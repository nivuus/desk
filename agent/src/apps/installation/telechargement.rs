//! Tirer l'installeur par HTTP, l'écrire, et vérifier son empreinte PENDANT
//! l'écriture.
//!
//! 🔴 CE MODULE EST PORTABLE : aucun `#[cfg]`. `tokio::net::TcpStream`, un
//! analyseur d'en-têtes et une écriture de fichier compilent et tournent sur
//! l'hôte Linux. C'est une divergence DÉCLARÉE avec le §6 de la
//! spécification, qui le rangeait en `#[cfg(windows)]` — et c'est une
//! amélioration de couverture, pas un détail : la **troisième** vérification
//! d'empreinte, la reprise par `Range`, le refus du `chunked` et celui de
//! `https` se testent tous ici, **contre un vrai serveur TCP local**, au lieu
//! de dépendre d'une recette VM. Seule l'exécution reste Windows.
//!
//! ⚠️ POURQUOI UN CLIENT ÉCRIT À LA MAIN. L'agent n'a **aucun** client HTTP, et
//! `tokio-tungstenite` y est verrouillé **sans TLS** : `reqwest` apporterait
//! une pile TLS entière et romprait l'invariant « aucune dépendance de
//! production » que G1 et G2 tiennent tous deux. Ce dépôt a déjà écrit son
//! client TURN, son codec STUN et son SHA-256 pour la même raison.
//!
//! 🔴 CE QU'IL REFUSE BRUYAMMENT PLUTÔT QUE DE L'INTERPRÉTER : `https://`,
//! parce qu'il ne parle pas TLS ; `Transfer-Encoding` ; tout statut hors `200`
//! et `206`. *Un refus nommé se diagnostique en une ligne de journal ; un
//! analyseur qui devine se diagnostique en une campagne.*

use std::path::{Path, PathBuf};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::apps::sha256::{hex_de, Condensateur};

use super::reponse::{self, Etat};

/// Combien de fois on rouvre après une coupure en cours de transfert.
///
/// ⚠️ **NON CALIBRÉE**, elle rejoint la liste que ce dépôt tient depuis
/// `BPP_MIN`. Ce qu'elle borne est réel : sans elle, un serveur qui coupe à
/// chaque octet ferait boucler le téléchargement sans terme.
pub const RETABLISSEMENTS_MAX: u32 = 5;

/// La taille du tampon de lecture du socket.
const TAMPON: usize = 64 * 1024;

/// Pourquoi un téléchargement n'a pas abouti.
///
/// ⚠️ CHAQUE VARIANTE PORTE DE QUOI LA DIAGNOSTIQUER SANS ROUVRIR LE PRODUIT.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refus {
    /// L'URL n'est pas une URL `http://` que ce client sache lire.
    ///
    /// 🔴 `https://` ET `wss://` TOMBENT ICI, ET C'EST UN REFUS NOMMÉ — pas un
    /// « schéma inconnu ». Le canal `/agent` ne parle que `ws://` aujourd'hui,
    /// et l'URL de téléchargement se dérive de `SIGNALING_URL` exactement comme
    /// `url_du_canal` dérive la sienne : le jour où la plateforme passera en
    /// TLS, c'est ce refus-là qui le dira, et non une erreur d'analyse.
    ///
    /// ✅ `ws://` EST ACCEPTÉ, LUI, depuis que la recette a montré que **tout**
    /// ordre d'installation était refusé sans cela.
    Url(String),
    /// La connexion n'a pas pu s'ouvrir, ou s'est rompue au-delà du budget.
    Reseau(String),
    /// L'analyse de la réponse a refusé — le motif voyage tel quel.
    Reponse(reponse::Refus),
    /// L'écriture sur le disque a échoué.
    Disque(String),
    /// 🔴 LA TROISIÈME DES TROIS VÉRIFICATIONS D'EMPREINTE. Le navigateur peut
    /// mentir, le disque de la plateforme peut se corrompre, le transfert peut
    /// tronquer : **aucun saut ne fait confiance au précédent**.
    Empreinte { attendue: String, obtenue: String },
    /// Le corps reçu ne fait pas la taille annoncée par l'ordre.
    Taille { attendue: u64, obtenue: u64 },
    /// Le budget de rétablissements est épuisé.
    TropDeCoupures(u32),
}

/// Ce que l'appelant fournit, et ce qu'il observe.
pub struct Demande<'a> {
    /// L'URL absolue, `http://` seulement.
    pub url: &'a str,
    /// Le jeton d'agent, tel quel — il part en `Authorization: Bearer`.
    pub jeton: &'a str,
    /// Où écrire. Le répertoire parent doit exister.
    pub destination: &'a Path,
    pub taille_attendue: u64,
    /// En hexadécimal minuscule, 64 caractères.
    pub sha256_attendu: &'a str,
}

/// Une URL `http://hote:port/chemin` découpée.
///
/// ⚠️ MODULE-PRIVÉ ET TESTÉ : l'analyse d'URL est le seul endroit où une
/// erreur produirait une connexion vers un hôte que personne n'a demandé.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Cible {
    hote: String,
    port: u16,
    /// Ce qui part sur la ligne de requête. Toujours non vide, toujours
    /// commençant par `/`.
    chemin: String,
    /// L'en-tête `Host`, avec son port quand il n'est pas celui par défaut.
    entete_host: String,
}

fn decouper(url: &str) -> Result<Cible, Refus> {
    // 🔴 `https` EST REFUSÉ NOMMÉMENT, ET NON « INCONNU ». Le distinguer d'une
    // URL malformée est ce qui permet au journal de dire « cet agent ne parle
    // pas TLS » plutôt que « URL illisible », qui enverrait chercher une
    // coquille là où il y a une capacité manquante.
    //
    // ⚠️ `wss://` TOMBE ICI AUSSI, et pour la même raison : l'URL de
    // téléchargement se dérive de celle du canal, qui est un schéma WebSocket.
    // Un `wss://` refusé « schéma non reconnu » enverrait chercher une coquille
    // là où il y a, là encore, une capacité manquante.
    if url.starts_with("https://") || url.starts_with("wss://") {
        return Err(Refus::Url(format!(
            "TLS non pris en charge : cet agent ne parle ni https ni wss ({url})"
        )));
    }
    // 🔴 `ws://` EST ACCEPTÉ AU MÊME TITRE QUE `http://`, ET C'EST LA RECETTE
    // QUI L'A EXIGÉ. L'URL de l'installeur est **dérivée, pas configurée** :
    // `canal-apps.ts` envoie le chemin relatif `/televersement/:id/contenu`, et
    // l'agent le résout contre l'adresse de son PROPRE canal — laquelle est un
    // `ws://`, puisque c'est un WebSocket. Le client n'acceptant que `http://`,
    // **tout ordre d'installation était refusé** sur `schéma non reconnu :
    // ws://…`, mesuré sur la chaîne réelle.
    //
    // ⚠️ CE FICHIER PORTAIT DÉJÀ LE FAIT SANS PORTER LE REMÈDE : la doc de
    // `Refus::Url` dit, mot pour mot, que « le canal `/agent` lui-même ne parle
    // que `ws://` ». La lecture était juste et le code ne la suivait pas — un
    // écart qu'aucun test d'hôte ne pouvait voir, tous construisant leurs URL
    // en `http://` contre un `TcpListener` local.
    //
    // ✅ C'EST AUSSI LE PRÉCÉDENT DE G2, ET IL EST RÉEMPLOYÉ PLUTÔT QUE
    // RÉINVENTÉ : `apps/icone/televersement.rs` accepte exactement ces deux
    // schémas, par le même `strip_prefix(…).or_else(…)`. Deux modules qui
    // dérivent la même adresse doivent en accepter la même forme.
    let reste = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("ws://"))
        .ok_or_else(|| Refus::Url(format!("schéma non reconnu : {url}")))?;
    let (autorite, chemin) = match reste.find('/') {
        Some(i) => (&reste[..i], &reste[i..]),
        None => (reste, "/"),
    };
    if autorite.is_empty() {
        return Err(Refus::Url(format!("hôte vide : {url}")));
    }
    // ⚠️ PAS DE `rfind` SUR `:` SANS PRÉCAUTION : une adresse IPv6 littérale en
    // porte plusieurs. Ce client ne les prend pas en charge, et le DIT.
    if autorite.starts_with('[') {
        return Err(Refus::Url(format!(
            "adresse IPv6 littérale non prise en charge : {url}"
        )));
    }
    let (hote, port) = match autorite.split_once(':') {
        Some((h, p)) => {
            let port = p
                .parse::<u16>()
                .map_err(|_| Refus::Url(format!("port illisible : {url}")))?;
            (h.to_string(), port)
        }
        None => (autorite.to_string(), 80u16),
    };
    if hote.is_empty() {
        return Err(Refus::Url(format!("hôte vide : {url}")));
    }
    Ok(Cible {
        hote,
        port,
        chemin: chemin.to_string(),
        // ⚠️ LE PORT PAR DÉFAUT NE S'ÉCRIT PAS DANS `Host` : c'est ce que la
        // RFC 9110 §7.2 demande, et un proxy peut router dessus.
        entete_host: if port == 80 {
            autorite.split(':').next().unwrap_or(autorite).to_string()
        } else {
            autorite.to_string()
        },
    })
}

/// Ce qu'une passe de transfert a fait.
struct Passe {
    /// Le total écrit DEPUIS LE DÉBUT DU FICHIER à la fin de cette passe.
    ///
    /// ⚠️ UN TOTAL, PAS UN DELTA, et c'est ce qui rend le redémarrage
    /// exprimable : une passe qui repart de zéro rend le total qu'elle a
    /// réellement écrit, et l'appelant n'a rien à retrancher.
    total: u64,
    /// `true` si la connexion s'est rompue avant la fin annoncée.
    coupee: bool,
}

/// Télécharge, écrit, et vérifie. Rend les octets écrits.
///
/// 🔴 L'EMPREINTE SE CALCULE PENDANT L'ÉCRITURE, PAS EN RELISANT LE FICHIER
/// APRÈS — sauf sur un chemin de reprise, où l'on **relit ce qui est déjà
/// écrit** pour réamorcer l'état de condensation. Le dire ici évite qu'on
/// « optimise » cette relecture un jour : sans elle, une reprise donnerait
/// l'empreinte de la seule FIN du fichier.
pub async fn telecharger<F>(demande: Demande<'_>, mut progres: F) -> Result<u64, Refus>
where
    F: FnMut(u64, u64),
{
    let cible = decouper(demande.url)?;
    let mut coupures = 0u32;
    let mut deja = 0u64;
    let mut condensateur = Condensateur::neuf();

    loop {
        let passe = une_passe(&cible, &demande, deja, &mut condensateur, &mut progres).await?;
        deja = passe.total;
        if !passe.coupee {
            break;
        }

        coupures = coupures.saturating_add(1);
        if coupures > RETABLISSEMENTS_MAX {
            let _ = tokio::fs::remove_file(demande.destination).await;
            return Err(Refus::TropDeCoupures(coupures));
        }
        tracing::warn!(
            url = demande.url,
            deja,
            coupures,
            "transfert coupé, reprise par Range"
        );
    }

    if deja != demande.taille_attendue {
        let _ = tokio::fs::remove_file(demande.destination).await;
        return Err(Refus::Taille {
            attendue: demande.taille_attendue,
            obtenue: deja,
        });
    }

    let obtenue = hex_de(condensateur.terminer());
    if obtenue != demande.sha256_attendu {
        // 🔴 LE FICHIER PARTIEL EST SUPPRIMÉ, ET LE TÉLÉVERSEMENT RESTE
        // REPRENABLE. Le garder inviterait un chemin ultérieur à le prendre
        // pour un installeur valide.
        let _ = tokio::fs::remove_file(demande.destination).await;
        return Err(Refus::Empreinte {
            attendue: demande.sha256_attendu.to_string(),
            obtenue,
        });
    }
    Ok(deja)
}

async fn une_passe<F>(
    cible: &Cible,
    demande: &Demande<'_>,
    deja: u64,
    condensateur: &mut Condensateur,
    progres: &mut F,
) -> Result<Passe, Refus>
where
    F: FnMut(u64, u64),
{
    let mut socket = TcpStream::connect((cible.hote.as_str(), cible.port))
        .await
        .map_err(|e| Refus::Reseau(format!("connexion à {}:{} : {e}", cible.hote, cible.port)))?;

    let mut requete = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\n\
         Accept-Encoding: identity\r\nConnection: close\r\n",
        cible.chemin, cible.entete_host, demande.jeton
    );
    if deja > 0 {
        requete.push_str(&format!("Range: bytes={deja}-\r\n"));
    }
    requete.push_str("\r\n");
    socket
        .write_all(requete.as_bytes())
        .await
        .map_err(|e| Refus::Reseau(format!("envoi de la requête : {e}")))?;

    // --- l'en-tête, relu jusqu'à être complet ---
    let mut tampon: Vec<u8> = Vec::with_capacity(TAMPON);
    let mut lecture = [0u8; TAMPON];
    let entete = loop {
        match reponse::analyser(&tampon) {
            Ok(Etat::Prete(e)) => break e,
            Ok(Etat::Incomplet) => {}
            Err(refus) => return Err(Refus::Reponse(refus)),
        }
        let n = socket
            .read(&mut lecture)
            .await
            .map_err(|e| Refus::Reseau(format!("lecture de l'en-tête : {e}")))?;
        if n == 0 {
            return Err(Refus::Reseau(
                "connexion fermée avant la fin de l'en-tête".into(),
            ));
        }
        tampon.extend_from_slice(&lecture[..n]);
    };

    // 🔴 UN `200` ALORS QU'ON A DEMANDÉ UN `Range` : LE SERVEUR A IGNORÉ LA
    // PLAGE et renvoie le fichier ENTIER. On repart de zéro — jamais on ne
    // concatène, ce qui produirait un fichier plus long que sa taille et une
    // empreinte fausse **sans que l'on sache pourquoi**.
    //
    // 🔴 ET ON CONSOMME **CETTE** RÉPONSE, on ne rouvre pas une connexion. Le
    // corps entier est déjà en train d'arriver : le jeter pour le redemander
    // ferait passer deux fois plusieurs centaines de mégaoctets sur le lien,
    // pour rien. Le fichier est tronqué et la condensation réamorcée, ce qui
    // est exactement ce que « repartir de zéro » veut dire.
    let deja = if deja > 0 && entete.statut == 200 {
        tracing::warn!(
            url = demande.url,
            deja,
            "le serveur a ignoré le Range : le téléchargement REPART DE ZÉRO,              sur cette réponse même"
        );
        *condensateur = Condensateur::neuf();
        0
    } else {
        deja
    };

    // --- le corps ---
    let mut fichier = ouvrir(demande.destination, deja).await?;
    let mut ecrits = 0u64;
    let debut = &tampon[entete.debut_du_corps..];
    if !debut.is_empty() {
        ecrire(&mut fichier, condensateur, debut).await?;
        ecrits += debut.len() as u64;
        progres(deja + ecrits, demande.taille_attendue);
    }

    while ecrits < entete.longueur {
        let n = socket
            .read(&mut lecture)
            .await
            .map_err(|e| Refus::Reseau(format!("lecture du corps : {e}")))?;
        if n == 0 {
            // ⚠️ FERMETURE AVANT LA FIN ANNONCÉE : c'est une coupure, pas une
            // fin. L'appelant reprendra par `Range`.
            fichier
                .flush()
                .await
                .map_err(|e| Refus::Disque(format!("vidage : {e}")))?;
            return Ok(Passe {
                total: deja + ecrits,
                coupee: true,
            });
        }
        // ⚠️ ON N'ÉCRIT JAMAIS AU-DELÀ DE CE QUI EST ANNONCÉ : un serveur qui
        // enverrait trop ferait sinon grossir le fichier sans terme.
        let reste = (entete.longueur - ecrits) as usize;
        let utile = &lecture[..n.min(reste)];
        ecrire(&mut fichier, condensateur, utile).await?;
        ecrits += utile.len() as u64;
        progres(deja + ecrits, demande.taille_attendue);
    }
    fichier
        .flush()
        .await
        .map_err(|e| Refus::Disque(format!("vidage : {e}")))?;

    Ok(Passe {
        total: deja + ecrits,
        coupee: false,
    })
}

/// Ouvre la destination, et **réamorce la condensation** sur une reprise.
async fn ouvrir(destination: &Path, deja: u64) -> Result<tokio::fs::File, Refus> {
    if deja == 0 {
        return tokio::fs::File::create(destination)
            .await
            .map_err(|e| Refus::Disque(format!("création de {} : {e}", destination.display())));
    }
    tokio::fs::OpenOptions::new()
        .append(true)
        .open(destination)
        .await
        .map_err(|e| Refus::Disque(format!("réouverture de {} : {e}", destination.display())))
}

async fn ecrire(
    fichier: &mut tokio::fs::File,
    condensateur: &mut Condensateur,
    bloc: &[u8],
) -> Result<(), Refus> {
    fichier
        .write_all(bloc)
        .await
        .map_err(|e| Refus::Disque(format!("écriture : {e}")))?;
    condensateur.absorber(bloc);
    Ok(())
}

/// Le chemin d'un installeur, tel que l'appelant le compose.
///
/// ⚠️ IL N'EST PAS ASSAINI ICI : la règle vit dans `installation::depot`, PURE
/// et testée, et l'appelant l'a déjà appliquée. La dupliquer ferait diverger
/// les deux le jour où l'une des deux changerait.
pub fn destination(repertoire: &str, nom: &str) -> PathBuf {
    Path::new(repertoire).join(nom)
}

#[cfg(test)]
#[path = "telechargement/tests.rs"]
mod tests;
