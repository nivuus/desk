//! Le téléchargement, éprouvé **contre un vrai serveur TCP local** — jamais
//! contre un double.
//!
//! 🔴 C'EST CE QUE LA PORTABILITÉ DU MODULE ACHÈTE, et c'est la seule raison
//! d'avoir divergé du §6 de la spécification, qui le rangeait en
//! `#[cfg(windows)]` : la troisième vérification d'empreinte, la reprise par
//! `Range`, le refus du `chunked` et celui de `https` sont ici tous les
//! quatre, sur l'hôte, au lieu de dépendre d'une recette VM.
//!
//! ⚠️ UN DOUBLE N'AURAIT PAS SUFFI. Ce que ces cas éprouvent est précisément ce
//! qu'un faux client aurait décidé lui-même : où reprendre, quoi faire d'un
//! `200` qui répond à un `Range`, et ce qu'on écrit quand la connexion tombe au
//! milieu du corps.

use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::*;

/// Ce que le faux serveur fait d'une requête.
enum Reaction {
    /// Répond `200` avec ce corps, entier.
    Entier(Vec<u8>),
    /// Répond `200`, envoie `coupe_a` octets, puis FERME. La reprise doit
    /// suivre.
    Coupe { corps: Vec<u8>, coupe_a: usize },
    /// Répond `200` même à un `Range` — le cas que le client doit détecter.
    IgnoreLeRange(Vec<u8>),
    /// Une réponse brute, telle quelle.
    Brut(&'static str),
}

/// Le journal de ce que le serveur a VU : c'est lui qui rend l'offset
/// vérifiable.
#[derive(Default)]
struct Vu {
    ranges: Vec<Option<u64>>,
    autorisations: Vec<String>,
}

async fn serveur(reactions: Vec<Reaction>) -> (String, Arc<Mutex<Vu>>) {
    let ecouteur = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = ecouteur.local_addr().expect("addr").port();
    let vu = Arc::new(Mutex::new(Vu::default()));
    let journal = Arc::clone(&vu);

    tokio::spawn(async move {
        for reaction in reactions {
            let Ok((mut socket, _)) = ecouteur.accept().await else {
                return;
            };
            let mut brut = Vec::new();
            let mut tampon = [0u8; 4096];
            loop {
                let Ok(n) = socket.read(&mut tampon).await else {
                    return;
                };
                if n == 0 {
                    break;
                }
                brut.extend_from_slice(&tampon[..n]);
                if brut.windows(4).any(|f| f == b"\r\n\r\n") {
                    break;
                }
            }
            let texte = String::from_utf8_lossy(&brut).to_string();
            {
                let mut j = journal.lock().expect("verrou");
                j.ranges.push(depuis_range(&texte));
                j.autorisations.push(
                    texte
                        .lines()
                        .find(|l| l.to_ascii_lowercase().starts_with("authorization:"))
                        .unwrap_or("")
                        .trim()
                        .to_string(),
                );
            }
            let debut = depuis_range(&texte).unwrap_or(0) as usize;
            match reaction {
                Reaction::Entier(corps) => {
                    let part = &corps[debut.min(corps.len())..];
                    let statut = if debut > 0 { 206 } else { 200 };
                    let _ = socket
                        .write_all(
                            format!(
                                "HTTP/1.1 {statut} OK\r\nContent-Length: {}\r\n\r\n",
                                part.len()
                            )
                            .as_bytes(),
                        )
                        .await;
                    let _ = socket.write_all(part).await;
                }
                Reaction::Coupe { corps, coupe_a } => {
                    let part = &corps[debut.min(corps.len())..];
                    let statut = if debut > 0 { 206 } else { 200 };
                    let _ = socket
                        .write_all(
                            format!(
                                "HTTP/1.1 {statut} OK\r\nContent-Length: {}\r\n\r\n",
                                part.len()
                            )
                            .as_bytes(),
                        )
                        .await;
                    let _ = socket.write_all(&part[..coupe_a.min(part.len())]).await;
                }
                Reaction::IgnoreLeRange(corps) => {
                    // 🔴 `200`, PAS `206`, ET LE CORPS ENTIER : c'est ce que
                    // fait un serveur qui ne gère pas les plages.
                    let _ = socket
                        .write_all(
                            format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", corps.len())
                                .as_bytes(),
                        )
                        .await;
                    let _ = socket.write_all(&corps).await;
                }
                Reaction::Brut(texte) => {
                    let _ = socket.write_all(texte.as_bytes()).await;
                }
            }
            let _ = socket.shutdown().await;
        }
    });

    (format!("http://127.0.0.1:{port}/t/c"), vu)
}

fn depuis_range(requete: &str) -> Option<u64> {
    let ligne = requete
        .lines()
        .find(|l| l.to_ascii_lowercase().starts_with("range:"))?;
    let valeur = ligne.split_once('=')?.1;
    valeur.trim_end_matches('-').trim().parse().ok()
}

fn corps_de(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i % 251) as u8).collect()
}

fn empreinte(octets: &[u8]) -> String {
    crate::apps::sha256::hex(octets)
}

struct Bac {
    _dir: std::path::PathBuf,
    fichier: std::path::PathBuf,
}

fn bac(nom: &str) -> Bac {
    let dir = std::env::temp_dir().join(format!("g3-dl-{}-{}", nom, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("bac");
    Bac {
        fichier: dir.join("setup.exe"),
        _dir: dir,
    }
}

#[tokio::test]
async fn telecharge_ecrit_et_verifie_l_empreinte() {
    let corps = corps_de(200_000);
    let (url, vu) = serveur(vec![Reaction::Entier(corps.clone())]).await;
    let b = bac("nominal");
    let ecrits = telecharger(
        Demande {
            url: &url,
            jeton: "jeton-d-agent",
            destination: &b.fichier,
            taille_attendue: corps.len() as u64,
            sha256_attendu: &empreinte(&corps),
        },
        |_, _| {},
    )
    .await
    .expect("téléchargement");
    assert_eq!(ecrits, corps.len() as u64);
    assert_eq!(std::fs::read(&b.fichier).expect("relecture"), corps);
    // Le jeton part bien en `Authorization: Bearer` — sans lui la route
    // refuserait, et le symptôme serait un 401 très loin d'ici.
    assert_eq!(
        vu.lock().expect("verrou").autorisations[0],
        "Authorization: Bearer jeton-d-agent"
    );
}

/// 🔴 LA ROUGE N°1, ET LE TEST VÉRIFIE L'**OFFSET DEMANDÉ**, PAS SEULEMENT LE
/// SUCCÈS.
///
/// Une reprise qui redemanderait tout depuis zéro passerait un test qui ne
/// regarde que le résultat — c'est la leçon du critère ③ de la spécification,
/// et c'est pourquoi le faux serveur JOURNALISE les `Range` qu'il reçoit.
#[tokio::test]
async fn une_coupure_reprend_au_bon_offset_et_l_empreinte_reste_juste() {
    let corps = corps_de(150_000);
    let (url, vu) = serveur(vec![
        Reaction::Coupe {
            corps: corps.clone(),
            coupe_a: 60_000,
        },
        Reaction::Entier(corps.clone()),
    ])
    .await;
    let b = bac("coupure");
    let ecrits = telecharger(
        Demande {
            url: &url,
            jeton: "j",
            destination: &b.fichier,
            taille_attendue: corps.len() as u64,
            sha256_attendu: &empreinte(&corps),
        },
        |_, _| {},
    )
    .await
    .expect("téléchargement repris");

    assert_eq!(ecrits, corps.len() as u64);
    assert_eq!(std::fs::read(&b.fichier).expect("relecture"), corps);
    let ranges = vu.lock().expect("verrou").ranges.clone();
    assert_eq!(
        ranges,
        vec![None, Some(60_000)],
        "la seconde requête doit demander EXACTEMENT ce qui manque"
    );
}

/// 🔴 LA ROUGE N°2 : UN `200` EN RÉPONSE À UN `Range` FAIT REPARTIR DE ZÉRO.
///
/// Concaténer produirait un fichier plus long que sa taille et une empreinte
/// fausse **sans que l'on sache pourquoi**. Le test compare la **TAILLE**.
///
/// 🔴 ET IL COMPTE LES CONNEXIONS, ce qui est la seconde moitié : le client
/// consomme **cette réponse-là**, il ne rouvre pas une troisième connexion pour
/// redemander ce qui est déjà en train d'arriver. La première rédaction de ce
/// module le faisait, et sur un installeur de 800 Mo cela aurait fait passer le
/// fichier DEUX FOIS sur le lien. **C'est ce test qui l'a trouvé** — le faux
/// serveur n'offrait que deux réactions, et la troisième connexion a été
/// refusée.
#[tokio::test]
async fn un_200_en_reponse_a_un_range_fait_repartir_de_zero() {
    let corps = corps_de(100_000);
    let (url, vu) = serveur(vec![
        Reaction::Coupe {
            corps: corps.clone(),
            coupe_a: 40_000,
        },
        Reaction::IgnoreLeRange(corps.clone()),
    ])
    .await;
    let b = bac("range-ignore");
    let ecrits = telecharger(
        Demande {
            url: &url,
            jeton: "j",
            destination: &b.fichier,
            taille_attendue: corps.len() as u64,
            sha256_attendu: &empreinte(&corps),
        },
        |_, _| {},
    )
    .await
    .expect("téléchargement recommencé");

    assert_eq!(ecrits, corps.len() as u64, "PAS de concaténation");
    assert_eq!(
        std::fs::metadata(&b.fichier).expect("stat").len(),
        corps.len() as u64
    );
    assert_eq!(std::fs::read(&b.fichier).expect("relecture"), corps);
    let ranges = vu.lock().expect("verrou").ranges.clone();
    assert_eq!(
        ranges,
        vec![None, Some(40_000)],
        "DEUX connexions, pas trois : la réponse au Range ignoré est CONSOMMÉE"
    );
}

/// 🔴 LA ROUGE N°3 : `chunked` EST REFUSÉ, ET LE MOTIF LE NOMME.
///
/// Le service pose un `Content-Length` ; qu'un proxy puisse le remplacer par un
/// codage par morceaux **n'a pas été mesuré**. Un refus nommé se diagnostique
/// en une ligne de journal ; un analyseur qui devine se diagnostique en une
/// campagne.
#[tokio::test]
async fn un_transfert_chunked_est_refuse_et_le_motif_le_nomme() {
    let (url, _) = serveur(vec![Reaction::Brut(
        "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n",
    )])
    .await;
    let b = bac("chunked");
    let refus = telecharger(
        Demande {
            url: &url,
            jeton: "j",
            destination: &b.fichier,
            taille_attendue: 1,
            sha256_attendu: &"0".repeat(64),
        },
        |_, _| {},
    )
    .await
    .expect_err("doit refuser");
    match refus {
        Refus::Reponse(reponse::Refus::TransfertCode(valeur)) => {
            assert!(valeur.contains("chunked"), "le motif doit NOMMER chunked");
        }
        autre => panic!("refus inattendu : {autre:?}"),
    }
}

/// 🔴 LA ROUGE N°4 : UN STATUT INATTENDU EST REFUSÉ, ET IL VOYAGE.
#[tokio::test]
async fn un_statut_inattendu_est_refuse_en_portant_son_statut() {
    let (url, _) = serveur(vec![Reaction::Brut(
        "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n",
    )])
    .await;
    let b = bac("cinq-cents");
    let refus = telecharger(
        Demande {
            url: &url,
            jeton: "j",
            destination: &b.fichier,
            taille_attendue: 1,
            sha256_attendu: &"0".repeat(64),
        },
        |_, _| {},
    )
    .await
    .expect_err("doit refuser");
    assert_eq!(refus, Refus::Reponse(reponse::Refus::Statut(500)));
}

/// 🔴 LA ROUGE N°5, ET C'EST LA TROISIÈME DES TROIS VÉRIFICATIONS
/// D'EMPREINTE : le corps arrive en entier, à la bonne taille, et son empreinte
/// diffère. **Le fichier partiel est supprimé**, et le téléversement reste
/// reprenable.
#[tokio::test]
async fn une_empreinte_fausse_est_refusee_et_le_fichier_partiel_disparait() {
    let corps = corps_de(50_000);
    let (url, _) = serveur(vec![Reaction::Entier(corps.clone())]).await;
    let b = bac("empreinte");
    let attendue = "f".repeat(64);
    let refus = telecharger(
        Demande {
            url: &url,
            jeton: "j",
            destination: &b.fichier,
            taille_attendue: corps.len() as u64,
            sha256_attendu: &attendue,
        },
        |_, _| {},
    )
    .await
    .expect_err("doit refuser");
    match refus {
        Refus::Empreinte { attendue: a, obtenue } => {
            assert_eq!(a, attendue);
            assert_eq!(obtenue, empreinte(&corps));
        }
        autre => panic!("refus inattendu : {autre:?}"),
    }
    assert!(
        !b.fichier.exists(),
        "le fichier partiel doit être SUPPRIMÉ : le garder inviterait un chemin \
         ultérieur à le prendre pour un installeur valide"
    );
}

/// 🔴 `https` EST REFUSÉ AVANT MÊME D'OUVRIR UN SOCKET, et le refus nomme la
/// capacité manquante — pas une coquille d'URL.
#[tokio::test]
async fn https_est_refuse_en_nommant_la_capacite_manquante() {
    let b = bac("https");
    let refus = telecharger(
        Demande {
            url: "https://exemple.invalide/t/c",
            jeton: "j",
            destination: &b.fichier,
            taille_attendue: 1,
            sha256_attendu: &"0".repeat(64),
        },
        |_, _| {},
    )
    .await
    .expect_err("doit refuser");
    match refus {
        Refus::Url(texte) => {
            assert!(texte.contains("TLS"), "le refus doit nommer TLS : {texte}");
        }
        autre => panic!("refus inattendu : {autre:?}"),
    }
    assert!(!b.fichier.exists(), "aucun fichier ne doit être créé");
}

/// Le budget de rétablissements est BORNÉ, et son épuisement est un refus
/// typé — jamais une boucle sans terme.
#[tokio::test]
async fn le_budget_de_retablissements_est_borne() {
    let corps = corps_de(10_000);
    // Sept coupures pour un budget de cinq : la sixième reprise doit rendre la
    // main.
    let reactions = (0..8)
        .map(|_| Reaction::Coupe {
            corps: corps.clone(),
            coupe_a: 10,
        })
        .collect();
    let (url, _) = serveur(reactions).await;
    let b = bac("budget");
    let refus = telecharger(
        Demande {
            url: &url,
            jeton: "j",
            destination: &b.fichier,
            taille_attendue: corps.len() as u64,
            sha256_attendu: &empreinte(&corps),
        },
        |_, _| {},
    )
    .await
    .expect_err("doit refuser");
    assert!(matches!(refus, Refus::TropDeCoupures(_)), "{refus:?}");
    assert!(!b.fichier.exists(), "le fichier partiel doit être supprimé");
}

#[test]
fn une_url_se_decoupe_et_le_port_par_defaut_ne_s_ecrit_pas_dans_host() {
    let c = decouper("http://plateforme.local/televersement/t-1/contenu").expect("url");
    assert_eq!(c.hote, "plateforme.local");
    assert_eq!(c.port, 80);
    assert_eq!(c.chemin, "/televersement/t-1/contenu");
    // ⚠️ RFC 9110 §7.2 : le port par défaut ne s'écrit pas, et un proxy peut
    // router dessus.
    assert_eq!(c.entete_host, "plateforme.local");

    let c = decouper("http://127.0.0.1:8080/t/c").expect("url");
    assert_eq!(c.port, 8080);
    assert_eq!(c.entete_host, "127.0.0.1:8080");

    // Sans chemin, la ligne de requête en porte quand même un.
    assert_eq!(decouper("http://h:9/").expect("url").chemin, "/");
    assert_eq!(decouper("http://h:9").expect("url").chemin, "/");

    // 🔴 `ws://` EST LU COMME `http://`, ET CE TEST DISAIT L'INVERSE. Il
    // assérait `Err(Refus::Url(_))` sur `ws://h/x` — écrit depuis la même
    // lecture fausse que le code qu'il gardait, et **il épinglait donc le
    // défaut au lieu de le prévenir**. La recette l'a réfuté sur la chaîne
    // réelle : l'URL de l'installeur étant DÉRIVÉE de celle du canal, elle
    // arrive toujours en `ws://`, et tout ordre d'installation était refusé.
    // Un test vert n'est une garde que si ce qu'il fixe est vrai.
    let ws = decouper("ws://h:9/x").expect("ws:// doit se lire comme http://");
    assert_eq!(ws.hote, "h");
    assert_eq!(ws.port, 9);
    assert_eq!(ws.chemin, "/x");
    // Le port par défaut d'un `ws://` est celui de `http://` — 80 —, et c'est
    // la conséquence directe de le traiter comme tel.
    assert_eq!(decouper("ws://h/x").expect("ws sans port").port, 80);

    // ⚠️ `wss://` RESTE REFUSÉ, ET NOMMÉMENT : accepter `ws://` ne dit rien de
    // TLS, que ce client ne parle pas plus qu'avant.
    assert!(matches!(decouper("wss://h/x"), Err(Refus::Url(_))));

    // Et les formes qu'on ne sait pas lire sont refusées NOMMÉMENT.
    assert!(matches!(decouper("http:///x"), Err(Refus::Url(_))));
    assert!(matches!(decouper("http://h:70000/x"), Err(Refus::Url(_))));
    assert!(matches!(decouper("http://[::1]:80/x"), Err(Refus::Url(_))));
}
