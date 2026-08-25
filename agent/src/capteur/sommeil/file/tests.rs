//! Tests de `capteur::sommeil::file` — fichier voisin plutôt que module en
//! ligne.
//!
//! **Extraction jouée dans une tâche DÉDIÉE, AVANT celle qui ajoute** (round
//! de correction 1, 25 août 2026) : `file.rs` était à 445 lignes pour un
//! plafond de projet à 500, et les correctifs qui suivent y ajoutent du code
//! ET de la doc. La règle du dépôt est « extraire, jamais comprimer », et
//! « la marge regagnée se reperd si on la traite comme acquise » — payé six
//! fois.
//!
//! ⚠️ **`#[path]` chez le parent, et ce n'est PAS la convention
//! `<parent>_<enfant>`.** Celle-ci ne vise que les modules extraits d'un
//! parent `#[cfg(windows)]` pour compiler sur l'hôte ; ici le mécanisme Rust
//! est le même mais la raison est autre — scinder un module de TESTS trop
//! long à l'intérieur d'un fichier par ailleurs portable. `CLAUDE.md` met ce
//! cas explicitement HORS de la portée de cette convention, et nomme le
//! précédent : `superviseur/table.rs`, qui déclare de la même façon
//! `#[path = "table/tests.rs"] mod tests;` et
//! `#[path = "table/tests_relance.rs"] mod tests_relance;`.
//!
//! **Le chemin de module reste `file::tests`** : seul l'emplacement physique
//! du fichier change, aucune visibilité n'est touchée.

use super::*;
use crate::capteur::sommeil::{Message, Ordre};
use std::collections::VecDeque;

/// 🔴 LE CŒUR DE LA DÉCISION : deux `Part` sans lecture n'en laissent
/// qu'UNE, et c'est la DERNIÈRE valeur qui survit.
#[test]
fn deux_parts_se_coalescent_en_une_seule() {
    let mut f = VecDeque::new();
    assert!(matches!(deposer(&mut f, Message::Part { bps: 1 }), Depot::Empilee));
    assert!(matches!(deposer(&mut f, Message::Part { bps: 2 }), Depot::Coalescee));
    assert_eq!(f.len(), 1);
    assert!(matches!(f[0], Message::Part { bps: 2 }));
}

/// 🔴 LE CRITÈRE QUI DISTINGUE LA COALESCENCE EN PLACE DE CELLE EN QUEUE,
/// et c'est l'invariant que `sommeil.rs` écrit : au sein d'une session, le
/// canal garantit l'ORDRE DE LIVRAISON entre les variantes. Coalescer en
/// queue ferait franchir à la part un ordre de dormir déposé entre-temps.
#[test]
fn la_coalescence_conserve_la_position() {
    let mut f = VecDeque::new();
    deposer(&mut f, Message::Part { bps: 1 });
    deposer(&mut f, Message::Sommeil(Ordre::Reveiller));
    deposer(&mut f, Message::Part { bps: 2 });
    assert_eq!(f.len(), 2);
    assert!(matches!(f[0], Message::Part { bps: 2 }), "la part garde sa PLACE");
    assert!(matches!(f[1], Message::Sommeil(Ordre::Reveiller)));
}

/// Un ordre perdu laisse une fenêtre endormie ou éveillée à tort.
#[test]
fn un_ordre_de_sommeil_n_est_jamais_coalesce() {
    let mut f = VecDeque::new();
    deposer(&mut f, Message::Sommeil(Ordre::Reveiller));
    assert!(matches!(deposer(&mut f, Message::Sommeil(Ordre::Reveiller)), Depot::Empilee));
    assert_eq!(f.len(), 2);
}

/// Un presse-papier perdu, c'est la donnée de l'utilisateur.
#[test]
fn un_presse_papier_n_est_jamais_coalesce() {
    let mut f = VecDeque::new();
    deposer(&mut f, Message::PressePapier { texte: Some("a".into()), octets: 1 });
    let d = deposer(&mut f, Message::PressePapier { texte: Some("b".into()), octets: 1 });
    assert!(matches!(d, Depot::Empilee));
    assert_eq!(f.len(), 2);
}

/// La borne dure REFUSE, elle ne tronque pas en silence.
#[test]
fn au_dela_de_la_borne_le_depot_est_refuse() {
    let mut f = VecDeque::new();
    for _ in 0..PROFONDEUR_MAX {
        deposer(&mut f, Message::Sommeil(Ordre::Reveiller));
    }
    assert!(matches!(deposer(&mut f, Message::Sommeil(Ordre::Reveiller)), Depot::Refusee));
    assert_eq!(f.len(), PROFONDEUR_MAX, "la file n'a pas grossi");
}

/// 🔴 LE TÉMOIN NÉGATIF, ET SA GARANTIE EXACTE : **une fois qu'une
/// occurrence de la variante est DÉJÀ en file**, un dépôt de cette
/// variante ne bute jamais sur la borne, quelle que soit la cadence — il
/// coalesce, donc il ne teste même pas la borne. Sans lui, « refusée »
/// au-dessus ne dirait pas que la coalescence borne réellement.
///
/// ⚠️ **LA GARANTIE N'EST PAS PLUS LARGE QUE CELA, et le nom d'origine
/// (`une_variante_coalescable_ne_bute_jamais_sur_la_borne`) SUR-AFFIRMAIT.**
/// Le PREMIER dépôt d'une variante coalescable, lui, s'empile comme les
/// autres et se heurte à la borne si la file est pleine d'incoalescables :
/// c'est le cas que mesure `un_premier_depot_coalescable_bute_bien_sur_la_borne`
/// juste en dessous. Ce n'est pas un défaut — le refus est explicite,
/// jamais une troncature — mais une sur-affirmation est la classe de
/// défaut que ce dépôt combat en premier.
#[test]
fn une_variante_deja_en_file_ne_bute_jamais_sur_la_borne() {
    let mut f = VecDeque::new();
    for i in 0..(PROFONDEUR_MAX * 10) {
        let d = deposer(&mut f, Message::Part { bps: i as u32 });
        assert!(!matches!(d, Depot::Refusee));
    }
    assert_eq!(f.len(), 1);
}

/// 🔴 CE QUE LA REVUE DE LA TÂCHE PRÉCÉDENTE A MESURÉ, et que le témoin
/// ci-dessus ne dit pas : le PREMIER `Part` déposé sur une file pleine de
/// variantes INCOALESCABLES n'a rien à remplacer, donc il s'empile — donc
/// il est REFUSÉ. **Ce n'est pas un défaut** : le refus est explicite et
/// compté, jamais une troncature silencieuse. C'est la borne de la
/// garantie, et elle est désormais éprouvée plutôt que supposée.
#[test]
fn un_premier_depot_coalescable_bute_bien_sur_la_borne() {
    let mut f = VecDeque::new();
    for _ in 0..PROFONDEUR_MAX {
        deposer(&mut f, Message::Sommeil(Ordre::Reveiller));
    }
    assert!(matches!(deposer(&mut f, Message::Part { bps: 1 }), Depot::Refusee));
    assert_eq!(f.len(), PROFONDEUR_MAX, "la file n'a pas grossi");
}

/// Le couple se comporte comme le canal qu'il remplace : ce qu'on dépose
/// se reçoit, dans l'ordre.
#[test]
fn ce_qui_est_depose_se_recoit_dans_l_ordre() {
    let (e, r) = canal_de_session();
    e.envoyer(Message::Sommeil(Ordre::Reveiller)).unwrap();
    e.envoyer(Message::PressePapier { texte: Some("a".into()), octets: 1 }).unwrap();
    assert!(matches!(r.essayer_recevoir(), Ok(Message::Sommeil(_))));
    assert!(matches!(r.essayer_recevoir(), Ok(Message::PressePapier { .. })));
    assert_eq!(r.essayer_recevoir(), Err(VideOuFerme::Vide));
}

/// 🔴 LE REFUS EST COMPTÉ. Un refus qui ne se compte pas est un refus
/// qu'aucune exploitation ne verra jamais.
#[test]
fn les_refus_se_comptent() {
    let (e, _r) = canal_de_session();
    for _ in 0..PROFONDEUR_MAX {
        e.envoyer(Message::Sommeil(Ordre::Reveiller)).unwrap();
    }
    assert_eq!(e.refuses(), 0, "aucun refus tant que la borne n'est pas atteinte");
    e.envoyer(Message::Sommeil(Ordre::Reveiller)).unwrap();
    assert_eq!(e.refuses(), 1);
}

/// L'émetteur sait que plus personne ne lit.
///
/// 🔴 C'EST LE TEST QUI TIENT LA PURGE DES SESSIONS MORTES du registre :
/// `distribuer` retire une session sur `envoyer(...).is_err()`, et rien
/// d'autre ne le fait sur ce chemin.
#[test]
fn un_receveur_tombe_ferme_l_emetteur() {
    let (e, r) = canal_de_session();
    drop(r);
    assert!(e.envoyer(Message::Sommeil(Ordre::Reveiller)).is_err());
}

/// Le receveur distingue « rien à lire » de « plus personne n'écrit ».
#[test]
fn un_emetteur_tombe_se_distingue_d_une_file_vide() {
    let (e, r) = canal_de_session();
    assert_eq!(r.essayer_recevoir(), Err(VideOuFerme::Vide));
    e.envoyer(Message::Sommeil(Ordre::Reveiller)).unwrap();
    drop(e);
    // Ce qui reste en file se lit ENCORE : la fermeture ne jette rien.
    assert!(matches!(r.essayer_recevoir(), Ok(Message::Sommeil(_))));
    assert_eq!(r.essayer_recevoir(), Err(VideOuFerme::Ferme));
}

/// `vider` rend ce qui attend, dans l'ordre, et laisse la file vide.
#[test]
fn vider_rend_tout_ce_qui_attend_dans_l_ordre() {
    let (e, r) = canal_de_session();
    e.envoyer(Message::Part { bps: 7 }).unwrap();
    e.envoyer(Message::Sommeil(Ordre::Reveiller)).unwrap();
    let recus = r.vider();
    assert_eq!(recus.len(), 2);
    assert!(matches!(recus[0], Message::Part { bps: 7 }));
    assert!(matches!(recus[1], Message::Sommeil(Ordre::Reveiller)));
    assert!(r.vider().is_empty());
}
