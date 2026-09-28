//! The download, tested **against a real local TCP server** — never
//! against a double.
//!
//! 🔴 THIS IS WHAT THE MODULE'S PORTABILITY BUYS, and it is the only reason
//! for having diverged from §6 of the specification, which put it under
//! `#[cfg(windows)]`: the third fingerprint check, the `Range`
//! resumption, the refusal of `chunked` and that of `https` are all four
//! here, on the host, instead of depending on a VM acceptance run.
//!
//! ⚠️ A DOUBLE WOULD NOT HAVE BEEN ENOUGH. What these cases test is precisely what
//! a fake client would have decided by itself: where to resume, what to do with a
//! `200` answering a `Range`, and what is written when the connection drops in the
//! middle of the body.

use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::*;

/// What the fake server does with a request.
enum Reaction {
    /// Replies `200` with this body, whole.
    Entier(Vec<u8>),
    /// Replies `200`, sends `coupe_a` bytes, then CLOSES. The resumption must
    /// follow.
    Coupe { corps: Vec<u8>, coupe_a: usize },
    /// Replies `200` even to a `Range` — the case the client must detect.
    IgnoreLeRange(Vec<u8>),
    /// A raw response, as is.
    Brut(&'static str),
}

/// The log of what the server SAW: it is what makes the offset
/// checkable.
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
                    // 🔴 `200`, NOT `206`, AND THE WHOLE BODY: that is what
                    // a server that does not handle ranges does.
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
    let value = ligne.split_once('=')?.1;
    value.trim_end_matches('-').trim().parse().ok()
}

fn corps_de(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i % 251) as u8).collect()
}

fn empreinte(octets: &[u8]) -> String {
    crate::apps::sha256::hex(octets)
}

struct Bac {
    _dir: std::path::PathBuf,
    file: std::path::PathBuf,
}

fn bac(nom: &str) -> Bac {
    let dir = std::env::temp_dir().join(format!("g3-dl-{}-{}", nom, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("bac");
    Bac {
        file: dir.join("setup.exe"),
        _dir: dir,
    }
}

#[tokio::test]
async fn downloads_writes_and_verifies_the_digest() {
    let corps = corps_de(200_000);
    let (url, vu) = serveur(vec![Reaction::Entier(corps.clone())]).await;
    let b = bac("nominal");
    let ecrits = telecharger(
        Demande {
            url: &url,
            jeton: "jeton-d-agent",
            destination: &b.file,
            expected_size: corps.len() as u64,
            sha256_attendu: &empreinte(&corps),
        },
        |_, _| {},
    )
    .await
    .expect("téléchargement");
    assert_eq!(ecrits, corps.len() as u64);
    assert_eq!(std::fs::read(&b.file).expect("relecture"), corps);
    // The token does go out as `Authorization: Bearer` — without it the route
    // would refuse, and the symptom would be a 401 very far from here.
    assert_eq!(
        vu.lock().expect("verrou").autorisations[0],
        "Authorization: Bearer jeton-d-agent"
    );
}

/// 🔴 RED NO. 1, AND THE TEST CHECKS THE **REQUESTED OFFSET**, NOT ONLY THE
/// SUCCESS.
///
/// A resumption that asked for everything again from zero would pass a test that
/// looks only at the result — that is the lesson of criterion ③ of the specification,
/// and it is why the fake server LOGS the `Range` it receives.
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
            destination: &b.file,
            expected_size: corps.len() as u64,
            sha256_attendu: &empreinte(&corps),
        },
        |_, _| {},
    )
    .await
    .expect("téléchargement repris");

    assert_eq!(ecrits, corps.len() as u64);
    assert_eq!(std::fs::read(&b.file).expect("relecture"), corps);
    let ranges = vu.lock().expect("verrou").ranges.clone();
    assert_eq!(
        ranges,
        vec![None, Some(60_000)],
        "la seconde requête doit demander EXACTEMENT ce qui manque"
    );
}

/// 🔴 RED NO. 2: A `200` IN REPLY TO A `Range` MAKES IT START OVER FROM ZERO.
///
/// Concatenating would produce a file longer than its size and a wrong
/// fingerprint **without anyone knowing why**. The test compares the **SIZE**.
///
/// 🔴 AND IT COUNTS THE CONNECTIONS, which is the second half: the client
/// consumes **that very response**, it does not reopen a third connection to
/// ask again for what is already arriving. The first draft of this
/// module did, and on an 800 MB installer it would have pushed the
/// file TWICE over the link. **This test is what found it** — the fake
/// server offered only two reactions, and the third connection was
/// refused.
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
            destination: &b.file,
            expected_size: corps.len() as u64,
            sha256_attendu: &empreinte(&corps),
        },
        |_, _| {},
    )
    .await
    .expect("téléchargement recommencé");

    assert_eq!(ecrits, corps.len() as u64, "PAS de concaténation");
    assert_eq!(
        std::fs::metadata(&b.file).expect("stat").len(),
        corps.len() as u64
    );
    assert_eq!(std::fs::read(&b.file).expect("relecture"), corps);
    let ranges = vu.lock().expect("verrou").ranges.clone();
    assert_eq!(
        ranges,
        vec![None, Some(40_000)],
        "DEUX connexions, pas trois : la réponse au Range ignoré est CONSOMMÉE"
    );
}

/// 🔴 RED NO. 3: `chunked` IS REFUSED, AND THE REASON NAMES IT.
///
/// The service sets a `Content-Length`; that a proxy might replace it with a
/// chunked coding **has not been measured**. A named refusal is diagnosed
/// in one log line; a parser that guesses is diagnosed in a
/// campaign.
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
            destination: &b.file,
            expected_size: 1,
            sha256_attendu: &"0".repeat(64),
        },
        |_, _| {},
    )
    .await
    .expect_err("doit refuser");
    match refus {
        Refus::Reponse(reponse::Refus::TransfertCode(value)) => {
            assert!(value.contains("chunked"), "le motif doit NOMMER chunked");
        }
        autre => panic!("refus inattendu : {autre:?}"),
    }
}

/// 🔴 RED NO. 4: AN UNEXPECTED STATUS IS REFUSED, AND IT TRAVELS.
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
            destination: &b.file,
            expected_size: 1,
            sha256_attendu: &"0".repeat(64),
        },
        |_, _| {},
    )
    .await
    .expect_err("doit refuser");
    assert_eq!(refus, Refus::Reponse(reponse::Refus::Statut(500)));
}

/// 🔴 RED NO. 5, AND IT IS THE THIRD OF THE THREE FINGERPRINT
/// CHECKS: the body arrives whole, at the right size, and its fingerprint
/// differs. **The partial file is deleted**, and the upload remains
/// resumable.
#[tokio::test]
async fn a_wrong_digest_is_refused_and_the_partial_file_disappears() {
    let corps = corps_de(50_000);
    let (url, _) = serveur(vec![Reaction::Entier(corps.clone())]).await;
    let b = bac("empreinte");
    let attendue = "f".repeat(64);
    let refus = telecharger(
        Demande {
            url: &url,
            jeton: "j",
            destination: &b.file,
            expected_size: corps.len() as u64,
            sha256_attendu: &attendue,
        },
        |_, _| {},
    )
    .await
    .expect_err("doit refuser");
    match refus {
        Refus::Empreinte {
            attendue: a,
            obtenue,
        } => {
            assert_eq!(a, attendue);
            assert_eq!(obtenue, empreinte(&corps));
        }
        autre => panic!("refus inattendu : {autre:?}"),
    }
    assert!(
        !b.file.exists(),
        "le fichier partiel doit être SUPPRIMÉ : le garder inviterait un chemin \
         ultérieur à le prendre pour un installeur valide"
    );
}

/// 🔴 `https` IS REFUSED BEFORE EVEN OPENING A SOCKET, and the refusal names the
/// missing capability — not a URL typo.
#[tokio::test]
async fn https_est_refuse_en_nommant_la_capacite_manquante() {
    let b = bac("https");
    let refus = telecharger(
        Demande {
            url: "https://exemple.invalide/t/c",
            jeton: "j",
            destination: &b.file,
            expected_size: 1,
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
    assert!(!b.file.exists(), "aucun fichier ne doit être créé");
}

/// The recovery budget is BOUNDED, and exhausting it is a typed
/// refusal — never an endless loop.
#[tokio::test]
async fn le_budget_de_retablissements_est_borne() {
    let corps = corps_de(10_000);
    // Seven cuts for a budget of five: the sixth resumption must give
    // up.
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
            destination: &b.file,
            expected_size: corps.len() as u64,
            sha256_attendu: &empreinte(&corps),
        },
        |_, _| {},
    )
    .await
    .expect_err("doit refuser");
    assert!(matches!(refus, Refus::TropDeCoupures(_)), "{refus:?}");
    assert!(!b.file.exists(), "le fichier partiel doit être supprimé");
}

#[test]
fn a_url_is_split_and_the_default_port_is_not_written_in_host() {
    let c = decouper("http://plateforme.local/televersement/t-1/contenu").expect("url");
    assert_eq!(c.hote, "plateforme.local");
    assert_eq!(c.port, 80);
    assert_eq!(c.chemin, "/televersement/t-1/contenu");
    // ⚠️ RFC 9110 §7.2: the default port is not written, and a proxy may
    // route on it.
    assert_eq!(c.entete_host, "plateforme.local");

    let c = decouper("http://127.0.0.1:8080/t/c").expect("url");
    assert_eq!(c.port, 8080);
    assert_eq!(c.entete_host, "127.0.0.1:8080");

    // Without a path, the request line still carries one.
    assert_eq!(decouper("http://h:9/").expect("url").chemin, "/");
    assert_eq!(decouper("http://h:9").expect("url").chemin, "/");

    // 🔴 `ws://` IS READ LIKE `http://`, AND THIS TEST SAID THE OPPOSITE. It
    // asserted `Err(Refus::Url(_))` on `ws://h/x` — written from the same
    // wrong reading as the code it guarded, and **so it pinned the
    // defect instead of preventing it**. The acceptance run refuted it on the real
    // chain: the installer URL being DERIVED from the channel's, it
    // always arrives as `ws://`, and every installation order was refused.
    // A green test is a guard only if what it pins is true.
    let ws = decouper("ws://h:9/x").expect("ws:// doit se lire comme http://");
    assert_eq!(ws.hote, "h");
    assert_eq!(ws.port, 9);
    assert_eq!(ws.chemin, "/x");
    // The default port of a `ws://` is that of `http://` — 80 —, and that is
    // the direct consequence of treating it as such.
    assert_eq!(decouper("ws://h/x").expect("ws sans port").port, 80);

    // ⚠️ `wss://` STAYS REFUSED, AND BY NAME: accepting `ws://` says nothing about
    // TLS, which this client speaks no more than before.
    assert!(matches!(decouper("wss://h/x"), Err(Refus::Url(_))));

    // And the forms we cannot read are refused BY NAME.
    assert!(matches!(decouper("http:///x"), Err(Refus::Url(_))));
    assert!(matches!(decouper("http://h:70000/x"), Err(Refus::Url(_))));
    assert!(matches!(decouper("http://[::1]:80/x"), Err(Refus::Url(_))));
}
