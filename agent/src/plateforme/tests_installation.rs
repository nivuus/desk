//! Le canal de l'agent, côté INSTALLATION : la seconde file, et son routage.
//!
//! 🔴 EXTRAITS PARCE QUE `tests.rs` A FRANCHI 500 LIGNES — 501 —, et la
//! doctrine de `CLAUDE.md` est de rattraper par une EXTRACTION, jamais par une
//! compression. **Un seul franchissement de UNE ligne se rattrape aussi par une
//! extraction** : compresser pour un dépassement de un serait exactement le
//! geste que D9 a payé (`sommeil.rs` ramené à 499, puis extrait sur exigence de
//! revue).
//!
//! ⚠️ Le harnais reste chez le parent (`faux_canal_bidirectionnel`,
//! `attendre_message`) : le recopier produirait deux versions qui divergeraient
//! à la première correction portée sur une seule.
//!
//! ⚠️ TRANSPOSITION VERBATIM. Le contrôle est le COMPTE, annoncé avant d'être
//! mesuré : `cargo test -p agent` rendait 861 avant, il doit rendre 861 après.

use crate::plateforme::tests::{attendre_message, faux_canal_bidirectionnel};
use super::*;
use proto::plateforme::IssueLancement;
use std::time::Duration;

#[tokio::test]
async fn un_ordre_d_installation_arrive_dans_SA_file_et_ne_ferme_pas_la_session() {
    // 🔴 DEUX ROUGES EN UNE, exactement comme pour `Lancer` — et c'est la
    // CINQUIÈME fois que ce dépôt paie la leçon du bras manquant (D5 `Sommeil`,
    // D6 `Part`, D7 `Audio`, D8 `PleinEcran`, G2 `IconesManquantes`). Sans le
    // bras `Installer`, un ordre valide tomberait dans le bras `Err` (« message
    // illisible »), qui FERME la session : le canal se reprendrait à chaque
    // installation demandée, et la trace accuserait une divergence de version
    // qui n'existe pas.
    //
    // ⚠️ ET IL VÉRIFIE QUE L'ORDRE ARRIVE DANS **L'AUTRE** FILE. Un bras qui
    // l'aurait poussé dans `ordres` passerait un test qui ne regarde que « la
    // session survit » : la file des ordres est drainée par le fil COM de la
    // découverte, qui se figerait alors pendant tout le téléchargement.
    let (url, mut recus, ordres) = faux_canal_bidirectionnel("PPP").await;
    let mut canal = ouvrir(&url, "w1".into(), "chut".into());
    canal.attendre_identite().await.expect("enrôlement");
    let mut recu_ordres = canal.ordres().expect("la file d'ordres n'est prise qu'une fois");
    let mut recu_install = canal
        .installations()
        .expect("la file d'installations n'est prise qu'une fois");

    ordres
        .send(
            r#"{"type":"installer","v":4,"installation":"i-1","url":"http://h:8080/t/c","nom":"setup.exe","taille":42,"sha256":"ab"}"#
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
    // 🔴 ET LA FILE DES ORDRES N'A RIEN REÇU. C'est l'assertion qui distingue
    // « routé correctement » de « routé n'importe où ».
    assert!(recu_ordres.try_recv().is_err(), "l'installation ne doit PAS aller aux ordres");

    // La session vit toujours : l'émission suivante arrive.
    canal.emetteur().emettre(VersLaPlateforme::lancee("d-7", IssueLancement::Raccourci));
    let texte = attendre_message(&mut recus, |t| t.contains("lancee")).await;
    assert!(texte.contains(r#""demande":"d-7""#));
}

#[tokio::test]
async fn la_file_d_installations_ne_se_prend_qu_une_fois() {
    // Même propriété qu'`ordres()`, et pour la même raison : deux consommateurs
    // se voleraient les ordres l'un à l'autre, et le symptôme serait « une
    // installation sur deux ne part pas ».
    let (url, _recus, _ordres) = faux_canal_bidirectionnel("PPP").await;
    let mut canal = ouvrir(&url, "w1".into(), "chut".into());
    canal.attendre_identite().await.expect("enrôlement");
    assert!(canal.installations().is_some());
    assert!(canal.installations().is_none());
}
