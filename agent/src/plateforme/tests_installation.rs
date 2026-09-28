//! The agent's channel, INSTALLATION side: the second queue, and its routing.
//!
//! 🔴 EXTRACTED BECAUSE `tests.rs` CROSSED 500 LINES — 501 —, and the
//! doctrine of `CLAUDE.md` is to catch up through an EXTRACTION, never through a
//! compression. **A single crossing of ONE line is also caught up through an
//! extraction**: compressing for an overflow of one would be exactly the
//! gesture D9 paid for (`sommeil.rs` brought back to 499, then extracted on review
//! demand).
//!
//! ⚠️ The harness stays in the parent (`faux_canal_bidirectionnel`,
//! `attendre_message`): copying it would produce two versions that would diverge
//! at the first fix applied to only one.
//!
//! ⚠️ VERBATIM TRANSPOSITION. The check is the COUNT, announced before being
//! measured: `cargo test -p agent` returned 861 before, it must return 861 after.

use super::*;
use crate::plateforme::tests::{attendre_message, faux_canal_bidirectionnel};
use proto::plateforme::IssueLancement;
use std::time::Duration;

#[tokio::test]
async fn un_ordre_d_installation_arrive_dans_sa_file_et_ne_ferme_pas_la_session() {
    // 🔴 TWO REDS IN ONE, exactly as for `Lancer` — and it is the
    // FIFTH time this repository pays the lesson of the missing arm (D5 `Sommeil`,
    // D6 `Part`, D7 `Audio`, D8 `PleinEcran`, G2 `IconesManquantes`). Without the
    // `Installer` arm, a valid order would fall into the `Err` arm ("unreadable
    // message"), which CLOSES the session: the channel would reconnect at each
    // requested installation, and the trace would accuse a version divergence
    // that does not exist.
    //
    // ⚠️ AND IT CHECKS THAT THE ORDER ARRIVES IN **THE OTHER** QUEUE. An arm that
    // had pushed it into `ordres` would pass a test that only looks at "the
    // session survives": the order queue is drained by the COM thread of
    // discovery, which would then freeze during the whole download.
    let (url, mut recus, ordres) = faux_canal_bidirectionnel("PPP").await;
    let mut canal = ouvrir(&url, "w1".into(), "chut".into());
    canal.attendre_identite().await.expect("enrôlement");
    let mut recu_ordres = canal
        .ordres()
        .expect("la file d'ordres n'est prise qu'une fois");
    let mut recu_install = canal
        .installations()
        .expect("la file d'installations n'est prise qu'une fois");

    ordres
        .send(
            r#"{"type":"installer","v":5,"installation":"i-1","url":"http://h:8080/t/c","nom":"setup.exe","taille":42,"sha256":"ab"}"#
                .into(),
        )
        .expect("envoi de l'ordre");

    let ordre = tokio::time::timeout(Duration::from_secs(5), recu_install.recv())
        .await
        .expect("aucune installation reçue en 5 s")
        .expect("la file d'installations est fermée");
    assert_eq!(
        ordre,
        crate::plateforme::Installation {
            id: "i-1".into(),
            url: "http://h:8080/t/c".into(),
            nom: "setup.exe".into(),
            taille: 42,
            sha256: "ab".into(),
        }
    );
    // 🔴 AND THE ORDER QUEUE RECEIVED NOTHING. It is the assertion that distinguishes
    // "routed correctly" from "routed anywhere".
    assert!(
        recu_ordres.try_recv().is_err(),
        "l'installation ne doit PAS aller aux ordres"
    );

    // The session is still alive: the next emission arrives.
    canal
        .emetteur()
        .emettre(VersLaPlateforme::lancee("d-7", IssueLancement::Raccourci));
    let texte = attendre_message(&mut recus, |t| t.contains("lancee")).await;
    assert!(texte.contains(r#""demande":"d-7""#));
}

#[tokio::test]
async fn la_file_d_installations_ne_se_prend_qu_une_fois() {
    // Same property as `ordres()`, and for the same reason: two consumers
    // would steal orders from one another, and the symptom would be "one
    // installation out of two does not go".
    let (url, _recus, _ordres) = faux_canal_bidirectionnel("PPP").await;
    let mut canal = ouvrir(&url, "w1".into(), "chut".into());
    canal.attendre_identite().await.expect("enrôlement");
    assert!(canal.installations().is_some());
    assert!(canal.installations().is_none());
}
