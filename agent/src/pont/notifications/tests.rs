use super::*;

/// 🔴 **LE contrôle du périmètre de F1.** « Toute tentative d'écriture rend
/// `ERROR_WRITE_PROTECT` (0x80070013). C'est un périmètre, pas une lacune »
/// (spec §8). ⚠️ *Cette phrase de la spec est FAUSSE pour un fichier créé de
/// toutes pièces, et la recette F1 l'a mesuré : la création RÉUSSIT. Ce test
/// n'en est pas affaibli — il porte sur les TROIS `PRE_`, qui sont bien les
/// seules refusables.* Si l'une de ces trois notifications cessait d'être refusée,
/// l'écriture RÉUSSIRAIT localement sur la VM et se perdrait en silence — la
/// perte silencieuse que le rappel `PRE_CONVERT_TO_FULL` existe pour empêcher.
#[test]
fn les_trois_notifications_d_ecriture_sont_refusees_en_protege_en_ecriture() {
    for code in [PRE_CONVERT_TO_FULL, PRE_RENAME, PRE_DELETE] {
        assert_eq!(
            decider(code),
            Reponse::Refuser(Erreur::ProtegeEnEcriture),
            "code {code} : ce n'est PAS un refus d'écriture"
        );
    }
}

/// Les liens durs n'ont aucun équivalent dans la File System Access API : ce
/// n'est pas un refus de lecture seule, c'est une opération qui n'existe pas de
/// l'autre côté (spec §3.5.2). La distinction est visible côté application et
/// au journal, ce qui est tout l'objet de `pont::erreurs`.
#[test]
fn les_liens_durs_sont_refuses_en_non_supporte() {
    for code in [PRE_SET_HARDLINK, HARDLINK_CREATED] {
        assert_eq!(decider(code), Reponse::Refuser(Erreur::NonSupporte), "code {code}");
    }
}

/// ⚠️ `NEW_FILE_CREATED` est une notification **POST** : elle ne se refuse pas.
/// Le fichier existe déjà sur la VM quand elle arrive. La seule chose à faire
/// est de la journaliser, pour que la recette CONSTATE la divergence plutôt que
/// de la découvrir.
#[test]
fn un_fichier_neuf_est_accepte_mais_signale() {
    assert_eq!(decider(NEW_FILE_CREATED), Reponse::AccepterEnSignalant);
}

/// 🔴 **Le garde d'exhaustivité, et c'est lui qui vaut le plus.** Chaque bit
/// que le masque demande doit avoir une décision NOMMÉE : un bit demandé qui
/// tomberait dans le bras fourre-tout serait accepté en silence, et une
/// écriture passerait.
#[test]
fn chaque_bit_du_masque_a_une_decision_nommee() {
    for bit in 0..32u32 {
        let drapeau = 1u32 << bit;
        if MASQUE & drapeau == 0 {
            continue;
        }
        let decision = decider(drapeau as i32);
        assert_ne!(
            decision,
            Reponse::AccepterSansAttendre,
            "le bit 0x{drapeau:X} est DEMANDÉ par le masque et retombe dans le bras \
             fourre-tout : il serait accepté en silence"
        );
    }
}

/// Le masque demande exactement cinq notifications, et pas une de plus. En
/// demander une sixième la ferait arriver sans décision ; en demander une de
/// moins ouvrirait un chemin d'écriture.
#[test]
fn le_masque_demande_exactement_les_cinq_notifications_de_f1() {
    assert_eq!(MASQUE.count_ones(), 5, "masque 0x{MASQUE:X}");
    assert_eq!(
        MASQUE,
        NOTIFY_FILE_PRE_CONVERT_TO_FULL
            | NOTIFY_PRE_RENAME
            | NOTIFY_PRE_DELETE
            | NOTIFY_PRE_SET_HARDLINK
            | NOTIFY_NEW_FILE_CREATED
    );
}

/// Une notification que le masque n'a pas demandée ne peut pas arriver — mais
/// si elle arrivait, l'accepter EN SILENCE ferait qu'un masque élargi par
/// erreur passerait inaperçu.
#[test]
fn une_notification_hors_masque_est_acceptee_sans_attendre() {
    // `FILE_OPENED` (mod.rs:338) : jamais demandée par F1.
    assert_eq!(decider(2), Reponse::AccepterSansAttendre);
}
