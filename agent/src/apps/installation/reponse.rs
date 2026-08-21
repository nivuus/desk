//! Lire une réponse HTTP/1.1 : son statut, sa longueur, et où commence le corps.
//!
//! 🔴 CE MODULE EST PUR : aucun `#[cfg]`, aucun socket, aucune lecture. Il
//! reçoit un `&[u8]` et rend une décision ; `installation::telechargement`
//! tient la connexion. C'est ce qui permet de juger l'analyse sur l'hôte
//! Linux, là où `execution.rs` n'a aucune épreuve possible.
//!
//! ⚠️ POURQUOI CE MODULE PLUTÔT QU'UN CRATE HTTP. L'agent n'a **aucun** client
//! HTTP et `tokio-tungstenite` y est verrouillé sans TLS : `reqwest`
//! apporterait une pile TLS entière et romprait l'invariant « aucune
//! dépendance de production » que G1 et G2 tiennent tous deux. Ce dépôt a déjà
//! écrit son client TURN, son codec STUN et son SHA-256 pour la même raison.
//!
//! 🔴 « PAS ENCORE COMPLET » EST UN ÉTAT À PART ENTIÈRE, ET NON UN REFUS. Le
//! tampon d'un appelant qui lit un socket porte un en-tête **coupé** une fois
//! sur deux : un analyseur qui ne saurait pas le dire conclurait sur
//! `Content-Len` et rendrait « longueur absente » sur une réponse parfaitement
//! valide, à laquelle il manquait quatre octets. Le refus serait bruyant,
//! typé, et **faux**.
//!
//! 🔴 REFUS TYPÉS, JAMAIS D'INTERPRÉTATION. *Un refus nommé se diagnostique en
//! une ligne de journal ; un analyseur qui devine se diagnostique en une
//! campagne.* Ce module ne réassemble aucune tranche, ne suit aucune
//! redirection et ne décompresse rien : il refuse, en disant quoi.

/// Le plafond de l'en-tête, en octets.
///
/// ⚠️ **NON CALIBRÉE**, elle rejoint la liste tenue depuis `BPP_MIN`. Elle
/// n'est pas un réglage fin : sans elle, un serveur qui n'enverrait **jamais**
/// son `\r\n\r\n` ferait grossir le tampon de l'appelant sans terme, et le
/// seul symptôme serait de la mémoire qui monte.
pub const ENTETE_MAX_OCTETS: usize = 16 * 1024;

/// Le seul codage de transfert que l'on sache lire : aucun.
const TRANSFERT_ACCEPTE: &str = "identity";

/// Les deux statuts qui portent un corps qu'on sache écrire : `200` la réponse
/// pleine, `206` celle à un `Range`. **Les deux voyagent**, l'appelant en
/// faisant deux choses opposées — un `200` reçu en réponse à un `Range` fait
/// **repartir de zéro**, jamais concaténer, ce qui produirait un fichier plus
/// long que sa taille et une empreinte fausse.
const STATUTS_RETENUS: &[u16] = &[200, 206];

/// Ce qui empêche de retenir une réponse.
///
/// ⚠️ CHAQUE VARIANTE PORTE DE QUOI LA DIAGNOSTIQUER SANS ROUVRIR LE PRODUIT :
/// le statut refusé, le codage refusé, la valeur illisible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refus {
    /// La première ligne n'est pas une ligne de statut HTTP/1.x.
    LigneDeStatut(String),
    /// Un statut hors `200` et `206` — il voyage tel quel.
    Statut(u16),
    /// Un `Transfer-Encoding` que l'on ne sait pas lire ; la valeur observée
    /// voyage, de sorte que le journal **nomme `chunked`**.
    TransfertCode(String),
    /// Aucun `Content-Length`. Le service en pose un ; qu'un intermédiaire
    /// puisse le retirer **n'a pas été mesuré**.
    LongueurAbsente,
    /// Un `Content-Length` qui n'est pas un nombre.
    LongueurIllisible(String),
    /// Deux `Content-Length` qui ne s'accordent pas — le vecteur classique de
    /// contrebande de requêtes. En retenir un serait choisir au hasard la
    /// lecture de l'un des deux intermédiaires.
    LongueurContradictoire { premiere: u64, seconde: u64 },
    /// L'en-tête n'est pas de l'UTF-8 — donc pas de l'ASCII, que la RFC impose.
    EnteteIllisible,
    /// L'en-tête dépasse `ENTETE_MAX_OCTETS` sans jamais se terminer.
    EnteteTropLongue(usize),
}

/// Ce qu'une réponse retenue apprend à l'appelant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entete {
    /// `200` ou `206`, et l'appelant en a besoin des deux : voir
    /// `STATUTS_RETENUS`.
    pub statut: u16,
    /// ⚠️ SUR UN `206`, C'EST LA LONGUEUR DE LA TRANCHE, PAS CELLE DU FICHIER.
    /// Un appelant qui la prendrait pour la taille finale déclarerait le
    /// téléchargement fini au premier octet de la reprise.
    pub longueur: u64,
    /// L'indice, dans le tampon analysé, du **premier octet du corps** —
    /// en-tête et début de corps arrivant dans la même lecture. Sans lui,
    /// l'appelant referait la recherche du séparateur, donc la referait
    /// différemment un jour.
    pub debut_du_corps: usize,
}

/// L'état d'une analyse, hors refus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Etat {
    /// Le séparateur `\r\n\r\n` n'est pas encore arrivé. **Ce n'est pas un
    /// refus** : il faut relire le socket et rappeler.
    Incomplet,
    Prete(Entete),
}

/// La règle entière, sur le tampon tel qu'il vient du socket.
pub fn analyser(tampon: &[u8]) -> Result<Etat, Refus> {
    // 🔴 LA COMPLÉTUDE SE TRANCHE AVANT TOUT LE RESTE, et c'est le seul ordre
    // qui tienne : chercher le séparateur sans vérifier qu'il est là ferait
    // conclure sur un en-tête partiel.
    let Some(fin) = position_du_separateur(tampon) else {
        return if tampon.len() > ENTETE_MAX_OCTETS {
            Err(Refus::EnteteTropLongue(tampon.len()))
        } else {
            Ok(Etat::Incomplet)
        };
    };

    let texte = std::str::from_utf8(&tampon[..fin]).map_err(|_| Refus::EnteteIllisible)?;
    let mut lignes = texte.split("\r\n");

    let statut = statut_de(lignes.next().unwrap_or_default())?;
    if !STATUTS_RETENUS.contains(&statut) {
        return Err(Refus::Statut(statut));
    }

    let mut longueur: Option<u64> = None;
    let mut transfert: Option<String> = None;
    for ligne in lignes {
        // Une ligne sans deux-points n'est pas un champ : les continuations
        // pliées de la RFC 7230 sont dépréciées, et aucun des trois champs que
        // l'on lit ne s'en sert.
        let Some((nom, valeur)) = ligne.split_once(':') else {
            continue;
        };
        let valeur = valeur.trim();
        // La RFC impose l'insensibilité à la casse sur le NOM ; la valeur d'un
        // codage de transfert est un jeton, donc repliée elle aussi.
        match nom.trim().to_ascii_lowercase().as_str() {
            "content-length" => {
                let lue = valeur
                    .parse::<u64>()
                    .map_err(|_| Refus::LongueurIllisible(valeur.to_string()))?;
                match longueur {
                    Some(premiere) if premiere != lue => {
                        return Err(Refus::LongueurContradictoire {
                            premiere,
                            seconde: lue,
                        });
                    }
                    _ => longueur = Some(lue),
                }
            }
            "transfer-encoding" => transfert = Some(valeur.to_ascii_lowercase()),
            _ => {}
        }
    }

    // 🔴 LE CODAGE DE TRANSFERT SE JUGE AVANT LA LONGUEUR, et c'est la RFC qui
    // l'impose : quand les deux sont présents, `Transfer-Encoding` gagne et le
    // `Content-Length` doit être ignoré. Juger la longueur d'abord rendrait
    // « longueur absente » sur une réponse en tranches, c'est-à-dire le mauvais
    // motif — celui qui envoie chercher un intermédiaire fautif au lieu du
    // codage qu'on ne sait pas lire.
    if let Some(code) = transfert {
        if code != TRANSFERT_ACCEPTE {
            return Err(Refus::TransfertCode(code));
        }
    }

    Ok(Etat::Prete(Entete {
        statut,
        longueur: longueur.ok_or(Refus::LongueurAbsente)?,
        debut_du_corps: fin + 4,
    }))
}

/// L'indice du `\r\n\r\n`, s'il est arrivé.
fn position_du_separateur(tampon: &[u8]) -> Option<usize> {
    tampon.windows(4).position(|f| f == b"\r\n\r\n")
}

/// `HTTP/1.1 200 OK` → `200`.
fn statut_de(ligne: &str) -> Result<u16, Refus> {
    let mut morceaux = ligne.split(' ');
    let version = morceaux.next().unwrap_or_default();
    let code = morceaux.next().unwrap_or_default();
    if !version.starts_with("HTTP/1.") || code.len() != 3 {
        return Err(Refus::LigneDeStatut(ligne.to_string()));
    }
    code.parse::<u16>()
        .map_err(|_| Refus::LigneDeStatut(ligne.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lire(brut: &str) -> Result<Etat, Refus> {
        analyser(brut.as_bytes())
    }

    fn prete(brut: &str) -> Entete {
        match lire(brut) {
            Ok(Etat::Prete(entete)) => entete,
            autre => panic!("attendu une réponse retenue, obtenu {autre:?}"),
        }
    }

    #[test]
    fn une_reponse_complete_rend_son_statut_sa_longueur_et_le_debut_du_corps() {
        let brut = "HTTP/1.1 200 OK\r\nContent-Length: 42\r\n\r\nabc";
        let entete = prete(brut);
        assert_eq!((entete.statut, entete.longueur), (200, 42));
        // Le corps commence APRÈS le séparateur, et le tampon en porte déjà
        // trois octets : c'est le cas nominal d'une lecture de socket.
        assert_eq!(&brut.as_bytes()[entete.debut_du_corps..], b"abc");
    }

    /// 🔴 LA ROUGE DE CETTE TÂCHE. Le tampon s'arrête au milieu du nom de
    /// l'en-tête ; l'analyseur doit dire « pas encore », **jamais** conclure.
    #[test]
    fn un_entete_coupe_en_deux_lectures_dit_incomplet_et_ne_conclut_pas() {
        assert_eq!(lire("HTTP/1.1 200 OK\r\nContent-Len"), Ok(Etat::Incomplet));
        assert_eq!(lire("HTTP/1.1 200 OK\r\n"), Ok(Etat::Incomplet));
        assert_eq!(lire(""), Ok(Etat::Incomplet));
        // Et la seconde lecture, elle, conclut — sans quoi « incomplet »
        // serait rendu par un analyseur entièrement mort.
        assert_eq!(prete("HTTP/1.1 200 OK\r\nContent-Length: 42\r\n\r\n").longueur, 42);
    }

    #[test]
    fn le_206_est_retenu_et_son_statut_voyage_pour_que_l_appelant_le_distingue() {
        assert_eq!(prete("HTTP/1.1 206 Partial Content\r\nContent-Length: 7\r\n\r\n").statut, 206);
    }

    #[test]
    fn un_statut_inattendu_est_refuse_en_portant_son_nombre() {
        for code in [302u16, 404, 500] {
            let brut = format!("HTTP/1.1 {code} X\r\nContent-Length: 0\r\n\r\n");
            assert_eq!(lire(&brut), Err(Refus::Statut(code)));
        }
    }

    /// Le codage se juge AVANT la longueur : sans `Content-Length`, une
    /// réponse en tranches doit dénoncer le codage, pas la longueur absente —
    /// le second motif enverrait chercher un intermédiaire fautif.
    #[test]
    fn chunked_est_refuse_le_motif_le_nomme_et_il_prime_sur_la_longueur() {
        for brut in [
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Length: 9\r\n\r\n",
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n",
        ] {
            assert_eq!(lire(brut), Err(Refus::TransfertCode("chunked".into())));
        }
        let gzip = "HTTP/1.1 200 OK\r\nTransfer-Encoding: gzip\r\n\r\n";
        assert_eq!(lire(gzip), Err(Refus::TransfertCode("gzip".into())));
        // `identity` ne code rien : il passe, et la casse ne compte pas.
        let brut = "HTTP/1.1 200 OK\r\nTransfer-Encoding: IDENTITY\r\nContent-Length: 3\r\n\r\n";
        assert_eq!(prete(brut).longueur, 3);
    }

    #[test]
    fn l_absence_de_longueur_est_un_refus_nomme_et_non_une_longueur_nulle() {
        assert_eq!(lire("HTTP/1.1 200 OK\r\nServer: x\r\n\r\n"), Err(Refus::LongueurAbsente));
        // Une longueur nulle est une réponse retenue, une longueur illisible
        // porte ce qu'on a lu : trois motifs, jamais confondus.
        assert_eq!(prete("HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n").longueur, 0);
        let flou = "HTTP/1.1 200 OK\r\nContent-Length: beaucoup\r\n\r\n";
        assert_eq!(lire(flou), Err(Refus::LongueurIllisible("beaucoup".into())));
    }

    #[test]
    fn les_noms_sont_insensibles_a_la_casse_et_les_valeurs_rognees() {
        for champ in ["CONTENT-LENGTH:   42  ", "content-length:42", "Content-Length:\t42"] {
            assert_eq!(prete(&format!("HTTP/1.1 200 OK\r\n{champ}\r\n\r\n")).longueur, 42);
        }
    }

    #[test]
    fn deux_longueurs_contradictoires_sont_refusees_et_deux_identiques_passent() {
        let deux = |a, b| format!("HTTP/1.1 200 OK\r\nContent-Length: {a}\r\nContent-Length: {b}\r\n\r\n");
        assert_eq!(
            lire(&deux(42, 9)),
            Err(Refus::LongueurContradictoire { premiere: 42, seconde: 9 })
        );
        assert_eq!(prete(&deux(42, 42)).longueur, 42);
    }

    #[test]
    fn une_premiere_ligne_qui_n_est_pas_du_http_est_refusee_en_la_citant() {
        for ligne in ["BONJOUR", "HTTP/1.1 OK"] {
            let brut = format!("{ligne}\r\nContent-Length: 1\r\n\r\n");
            assert_eq!(lire(&brut), Err(Refus::LigneDeStatut(ligne.into())));
        }
    }

    /// Un en-tête malformé se refuse, il ne se devine pas — et sans borne, le
    /// tampon de l'appelant grossirait sans terme.
    #[test]
    fn un_entete_sans_fin_est_borne_et_un_entete_non_ascii_est_refuse() {
        let trop = "HTTP/1.1 200 OK\r\n".to_string() + &"X: y\r\n".repeat(ENTETE_MAX_OCTETS / 4);
        assert!(matches!(lire(&trop), Err(Refus::EnteteTropLongue(_))));
        // Sous le plafond, le même en-tête inachevé reste « incomplet ».
        assert_eq!(lire("HTTP/1.1 200 OK\r\nX: y\r\n"), Ok(Etat::Incomplet));
        let brut = b"HTTP/1.1 200 OK\r\nX: \xff\r\nContent-Length: 1\r\n\r\n";
        assert_eq!(analyser(brut), Err(Refus::EnteteIllisible));
    }
}
