//! Les tests de `memoriser_controle` : ce que la réception d'un `ClientControl`
//! RETIENT, et ce qu'elle n'applique surtout pas sur-le-champ.
//!
//! **Extrait AVANT que le fichier voisin ne franchisse 500** (sous-bloc P2 du
//! chantier presse-papier) : il était monté à **493, marge 7**, les deux tests
//! du collage lui ayant pris quarante lignes. La règle du dépôt est d'extraire,
//! jamais de comprimer un commentaire pour repasser sous la ligne — et le
//! voisin le dit déjà de lui-même, en tête.
//!
//! La famille est cohérente : tous ces tests passent par
//! `dispatch_controle_de_test`, le point d'entrée `#[cfg(test)]` qui
//! court-circuite `ChannelData` (str0m interdit délibérément sa construction
//! hors de son crate) tout en exerçant exactement le même chemin de
//! mémorisation que `dispatch_channel_data`.

use std::time::Instant;

use super::super::*;
use crate::transport::fixtures;

/// `ClientControl::Visibility` reçu doit être mémorisé dans
/// `pending_visibility`, pas appliqué sur-le-champ.
///
/// Même raison que pour `Resize` : ce code court pendant le drainage de
/// `poll_output`, et relâcher un encodeur y romprait l'invariant d'une
/// seule mutation de `Rtc` par appel. `dispatch_controle_de_test` est un
/// point d'entrée `#[cfg(test)]` qui court-circuite `ChannelData` (str0m
/// interdit délibérément sa construction hors du crate) tout en exerçant
/// exactement le même chemin de mémorisation que `dispatch_channel_data`.
#[test]
fn un_message_de_visibilite_est_memorise_et_non_applique_sur_le_champ() {
    let source = Box::new(fixtures::video_test_source());
    let mut session = Session::new(source, fixtures::local_ip(), Instant::now(), 12_000_000)
        .expect("session");

    let json = r#"{"type":"visibility","v":3,"visible":false,"focused":false}"#;
    session.dispatch_controle_de_test(json);

    assert_eq!(session.pending_visibility, Some((false, false)));
}

/// `ClientControl::Clipboard` reçu doit être mémorisé dans
/// `pending_clipboard`, pas appliqué sur-le-champ — même raison que ses deux
/// voisins.
///
/// ROUGE si le bras posait le mauvais champ. Le bras ABSENT, lui, ne compile
/// pas : le `match` de `memoriser_controle` est exhaustif, et c'est le
/// compilateur qui l'a exigé au moment où la variante est née.
#[test]
fn un_message_de_collage_est_memorise_et_non_applique_sur_le_champ() {
    let source = Box::new(fixtures::video_test_source());
    let mut session = Session::new(source, fixtures::local_ip(), Instant::now(), 12_000_000)
        .expect("session");

    session.dispatch_controle_de_test(r#"{"type":"clipboard","v":3,"text":"bonjour"}"#);

    assert_eq!(session.pending_clipboard.as_deref(), Some("bonjour"));
    // Mémoriser n'injecte RIEN : l'écriture et l'injection vivent dans
    // `act_on_timeout`, hors du drainage de `poll_output`.
    assert!(!session.collage_a_injecter);
}

/// 🔴 **C'est la décision D-P2-3 rendue vérifiable, et c'est le seul endroit
/// où elle l'est.** Deux collages entre deux tours de boucle se réduisent au
/// SECOND ; le premier est perdu sans trace.
///
/// ROUGE si le champ accumulait (une file, un `Vec`) : le test lirait alors le
/// premier ou les deux. Ce test ne dit pas que l'écrasement est bon — il dit
/// que c'est bien ce que le produit fait, et la doc du champ en porte le coût.
#[test]
fn deux_collages_successifs_ne_laissent_que_le_second() {
    let source = Box::new(fixtures::video_test_source());
    let mut session = Session::new(source, fixtures::local_ip(), Instant::now(), 12_000_000)
        .expect("session");

    session.dispatch_controle_de_test(r#"{"type":"clipboard","v":3,"text":"premier"}"#);
    session.dispatch_controle_de_test(r#"{"type":"clipboard","v":3,"text":"second"}"#);

    assert_eq!(session.pending_clipboard.as_deref(), Some("second"));
}
