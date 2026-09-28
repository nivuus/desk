//! Bridge transport tests: channel closing, and the label that decides which
//! channel is kept, apart from `tests.rs` to stay under 500 lines.

use super::tests::{canal_ouvert, echanger, monter};
use super::*;
use std::sync::mpsc::channel;
use std::time::Instant;
use str0m::channel::ChannelId;
use str0m::{Event, Rtc};

#[test]
fn la_fermeture_du_canal_seul_remonte_canal_ferme_et_seulement_pour_le_bon_id() {
    // Le cas où le navigateur ferme SON canal de données sans fermer la
    // connexion — distinct du close_notify du test suivant, et servi par un
    // autre bras de `traiter`. Né d'une MUTATION SURVIVANTE : retirer le bras
    // `ChannelClose` laissait tous les autres tests verts, aucun n'empruntant
    // ce chemin.
    //
    // ⚠️ **Il s'écrit sur des événements SYNTHÉTIQUES, et ce n'est pas un
    // raccourci de confort — c'est la seule voie.** Vérifié dans les sources de
    // str0m 0.21 plutôt que supposé : `DirectApi::close_data_channel` appelle
    // `RtcSctp::close_stream`, qui ne fait que poser `do_close = true` sur
    // l'entrée LOCALE (`sctp/mod.rs:516-520`) ; l'événement `SctpEvent::Close`
    // qui en découle (`:838-841`) est rendu au pair qui ferme, et **rien n'est
    // émis sur le fil**. Un pair str0m ne peut donc pas provoquer de
    // `Event::ChannelClose` chez nous. Un vrai navigateur, lui, émet un
    // stream-reset SCTP que str0m traite bien (`:740`, `:790`, `:808`, `:818`
    // posent `do_close` depuis l'entrée) — le bras est donc utile en
    // production, et seulement inatteignable depuis ce banc.
    //
    // Précédent de ce dépôt pour la forme : `transport/evenements/tests.rs`
    // remet des `MediaAdded` synthétiques à une session nue, pour exercer une
    // discrimination sans négociation.
    let mut faux = Rtc::builder().clear_codecs().build(Instant::now());
    let mut api = faux.sdp_api();
    let bon = api.add_channel(LABEL_FICHIERS.to_string());
    let autre = api.add_channel("autre-canal".to_string());

    let (tx, rx) = channel();
    let mut canal: Option<ChannelId> = None;

    assert!(traiter(
        Event::ChannelOpen(bon, LABEL_FICHIERS.to_string()),
        &mut canal,
        &tx
    )
    .is_none());
    assert_eq!(rx.try_recv(), Ok(DuNavigateur::CanalOuvert));
    assert_eq!(canal, Some(bon));

    // Une fermeture qui ne concerne PAS notre canal ne doit RIEN remonter :
    // sans cette moitié, le test passerait sur un bras qui envoie
    // `CanalFerme` à chaque fermeture, quelle qu'elle soit — et le pont
    // déclarerait mort un canal bien vivant.
    assert!(traiter(Event::ChannelClose(autre), &mut canal, &tx).is_none());
    assert!(
        rx.try_recv().is_err(),
        "la fermeture d'un autre canal ne remonte rien"
    );
    assert_eq!(canal, Some(bon), "et elle ne doit pas oublier le nôtre");

    assert!(traiter(Event::ChannelClose(bon), &mut canal, &tx).is_none());
    assert_eq!(rx.try_recv(), Ok(DuNavigateur::CanalFerme));
    assert_eq!(canal, None, "le canal fermé doit être oublié");
}

#[test]
fn un_canal_dont_le_label_n_est_pas_le_notre_n_est_jamais_retenu() {
    // Le pendant synthétique de `un_seul_canal_est_retenu_parmi_deux…`, et il
    // exerce ce que celui-là ne peut pas : l'ordre INVERSE d'ouverture, où le
    // canal étranger arrive EN DERNIER. Sans filtre sur le label, c'est lui qui
    // écraserait le nôtre.
    let mut faux = Rtc::builder().clear_codecs().build(Instant::now());
    let mut api = faux.sdp_api();
    let bon = api.add_channel(LABEL_FICHIERS.to_string());
    let autre = api.add_channel("autre-canal".to_string());

    let (tx, rx) = channel();
    let mut canal: Option<ChannelId> = None;
    traiter(
        Event::ChannelOpen(bon, LABEL_FICHIERS.to_string()),
        &mut canal,
        &tx,
    );
    let _ = rx.try_recv();
    traiter(
        Event::ChannelOpen(autre, "autre-canal".to_string()),
        &mut canal,
        &tx,
    );

    assert_eq!(
        canal,
        Some(bon),
        "un canal étranger ne doit jamais écraser le nôtre"
    );
    assert!(
        rx.try_recv().is_err(),
        "et il ne doit annoncer aucune ouverture"
    );
}

#[test]
fn la_fermeture_du_canal_remonte_canal_ferme() {
    let (mut pair, _sortant, entrant) = monter(&[LABEL_FICHIERS]);

    // Le pair s'en va — l'onglet se ferme. La boucle doit le DIRE, pas se
    // terminer en silence : sans ce message, les commandes en vol attendraient
    // leur délai plutôt que de rendre ERROR_IO_DEVICE tout de suite.
    let mut parti = false;
    let (remontees, _) = echanger(
        &mut pair,
        &entrant,
        |rtc, remontees| {
            if !parti && canal_ouvert(remontees) {
                parti = true;
                // ⚠️ `close()` et NON `disconnect()` — la différence est tout
                // le test. `disconnect()` ne fait que poser `alive = false`
                // LOCALEMENT, sans rien émettre : le pont ne l'apprendrait
                // qu'à l'expiration d'ICE, des dizaines de secondes plus tard.
                // `close()` envoie le close_notify DTLS, c'est-à-dire ce que
                // fait un navigateur dont on ferme l'onglet. Une première
                // rédaction employait `disconnect()` et échouait au budget —
                // elle ne mesurait pas ce qu'elle croyait.
                rtc.close().expect("close_notify émis");
            }
        },
        |remontees, _| remontees.contains(&DuNavigateur::CanalFerme),
        "que la fermeture ne remonte",
    );
    assert!(
        remontees.contains(&DuNavigateur::CanalOuvert),
        "le canal doit d'abord s'être ouvert, sinon le test ne mesure rien"
    );
    assert!(remontees.contains(&DuNavigateur::CanalFerme));
}
