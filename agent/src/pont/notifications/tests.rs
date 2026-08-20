use super::*;

/// L'état nominal de F2 : racine inscriptible, canal ouvert.
const OUVERT: Etat = Etat { inscriptible: true, canal_ouvert: true };
/// L'état de F1, que `PONT_ECRITURE` ne pose plus mais que le code sait tenir.
const LECTURE_SEULE: Etat = Etat { inscriptible: false, canal_ouvert: true };
const CANAL_FERME: Etat = Etat { inscriptible: true, canal_ouvert: false };

/// 🔴 **L'UNIQUE PORTE DE REFUS D'UNE ÉCRITURE, et elle porte sur un ÉTAT.**
///
/// Si `PRE_CONVERT_TO_FULL` cessait d'être refusée sur une racine non
/// inscriptible, une écriture RÉUSSIRAIT localement sur la VM sans que rien ne
/// la pousse — la perte silencieuse que tout ce sous-projet existe pour
/// interdire.
#[test]
fn une_ecriture_sur_racine_non_inscriptible_est_refusee() {
    assert_eq!(
        decider(PRE_CONVERT_TO_FULL, LECTURE_SEULE),
        Reponse::Refuser(Erreur::ProtegeEnEcriture)
    );
}

/// 🔴 **DEUX CAUSES NE PARTAGENT JAMAIS UN CODE** (spec §5.1).
///
/// Un canal fermé rend `ERROR_IO_DEVICE`, pas `ERROR_WRITE_PROTECT` : « ce
/// partage est en lecture seule » et « l'onglet est fermé » n'appellent pas le
/// même geste de l'utilisateur, et c'est le seul instant où on peut encore le
/// lui dire.
#[test]
fn une_ecriture_sur_canal_ferme_est_refusee_en_erreur_d_e_s() {
    assert_eq!(
        decider(PRE_CONVERT_TO_FULL, CANAL_FERME),
        Reponse::Refuser(Erreur::CanalFerme)
    );
    // …et les deux causes ne se confondent pas.
    assert_ne!(
        decider(PRE_CONVERT_TO_FULL, CANAL_FERME),
        decider(PRE_CONVERT_TO_FULL, LECTURE_SEULE)
    );
}

/// L'écriture est AUTORISÉE dans l'état nominal — c'est la seule ligne de F2
/// qui change ce qu'une application obtient.
#[test]
fn une_ecriture_est_autorisee_quand_la_racine_est_inscriptible_et_le_canal_ouvert() {
    assert_eq!(decider(PRE_CONVERT_TO_FULL, OUVERT), Reponse::Autoriser);
}

/// 🔴 **LE GARDE DE LA DÉCISION §0.2 DU PLAN.**
///
/// Renommage et suppression restent refusés, **quel que soit l'état** : ils
/// sont des livrables de F3. Les accepter sans pouvoir les pousser laisserait
/// le poste local sur l'ancien contenu, sans que rien ne le dise — et une
/// application qui emploie l'idiome écrire-temporaire / renommer / supprimer
/// doit échouer BRUYAMMENT plutôt que silencieusement.
#[test]
fn un_renommage_et_une_suppression_restent_refuses_en_f2() {
    for etat in [OUVERT, LECTURE_SEULE, CANAL_FERME] {
        for code in [PRE_RENAME, PRE_DELETE] {
            assert_eq!(
                decider(code, etat),
                Reponse::Refuser(Erreur::ProtegeEnEcriture),
                "code {code} dans l'état {etat:?}"
            );
        }
    }
}

/// Les liens durs n'ont aucun équivalent dans la File System Access API : ce
/// n'est pas un refus de lecture seule, c'est une opération qui n'existe pas de
/// l'autre côté (spec §3.5.2).
///
/// ⚠️ `HARDLINK_CREATED` est une **POST** : le refus n'empêche rien, il
/// journalise. Le test épingle la décision, pas un effet.
#[test]
fn les_liens_durs_sont_refuses_en_non_supporte() {
    for code in [PRE_SET_HARDLINK, HARDLINK_CREATED] {
        assert_eq!(decider(code, OUVERT), Reponse::Refuser(Erreur::NonSupporte), "code {code}");
    }
}

/// 🔴 **LES DEUX POST DE CONTENU DÉCLENCHENT UNE POUSSÉE.**
///
/// Oublier `FILE_OVERWRITTEN` ferait perdre en silence tout enregistrement qui
/// tronque à l'ouverture (`CREATE_ALWAYS`, `TRUNCATE_EXISTING`) sans jamais
/// refermer le handle sur une modification — c'est-à-dire une bonne part des
/// enregistrements « en place ».
#[test]
fn les_deux_post_de_contenu_declenchent_une_poussee() {
    for code in [FILE_OVERWRITTEN, FILE_HANDLE_CLOSED_FILE_MODIFIED] {
        assert_eq!(
            decider(code, OUVERT),
            Reponse::Pousser(Poussee::Contenu),
            "code {code}"
        );
    }
}

/// Une création est poussée, et **comme une création, pas comme un contenu** :
/// un répertoire n'a aucun octet à lire.
#[test]
fn un_fichier_neuf_est_pousse_comme_une_creation() {
    assert_eq!(decider(NEW_FILE_CREATED, OUVERT), Reponse::Pousser(Poussee::Creation));
    assert_ne!(
        decider(NEW_FILE_CREATED, OUVERT),
        Reponse::Pousser(Poussee::Contenu),
        "une création n'est pas un contenu : un répertoire n'a rien à lire"
    );
}

/// ⚠️ **UNE POST N'EST PAS REFUSABLE, donc l'état ne la change pas.**
///
/// Le dire est le remède au piège que ce module a payé en F1 : croire qu'un
/// refus rendu sur une POST empêche quoi que ce soit. Une poussée part même
/// canal fermé — le fil d'écriture la journalise et la retiendra.
#[test]
fn une_poussee_part_meme_canal_ferme_car_une_post_ne_se_refuse_pas() {
    for etat in [OUVERT, LECTURE_SEULE, CANAL_FERME] {
        assert!(
            matches!(decider(FILE_HANDLE_CLOSED_FILE_MODIFIED, etat), Reponse::Pousser(_)),
            "état {etat:?}"
        );
    }
}

/// 🔴 **Le garde d'exhaustivité, et c'est lui qui vaut le plus.** Chaque bit que
/// le masque demande doit avoir une décision NOMMÉE, **dans les trois états** :
/// un bit demandé qui tomberait dans le bras fourre-tout serait accepté en
/// silence, et une écriture se perdrait.
///
/// ⚠️ **C'est ce test qui a fait diverger F2 de son plan.** Celui-ci prescrivait
/// `AccepterSansAttendre` pour `PRE_CONVERT_TO_FULL` autorisée : le bit serait
/// alors retombé dans le fourre-tout, et le remède évident — l'exclure du
/// balayage — aurait VIDÉ ce garde au lieu de le satisfaire. D'où la variante
/// `Autoriser`, qui nomme l'acceptation.
#[test]
fn chaque_bit_du_masque_a_une_decision_nommee() {
    for etat in [OUVERT, LECTURE_SEULE, CANAL_FERME] {
        for bit in 0..32u32 {
            let drapeau = 1u32 << bit;
            if MASQUE & drapeau == 0 {
                continue;
            }
            assert_ne!(
                decider(drapeau as i32, etat),
                Reponse::AccepterSansAttendre,
                "le bit 0x{drapeau:X} est DEMANDÉ par le masque et retombe dans le bras \
                 fourre-tout dans l'état {etat:?} : il serait accepté en silence"
            );
        }
    }
}

/// Le masque demande exactement SEPT notifications, et pas une de plus.
///
/// ⚠️ **Ce test était ROUGE AVANT la modification**, et il ne peut donc pas être
/// vacueux : le masque de F1 en portait cinq. C'est le seul test de ce module
/// dont l'atteignabilité n'a rien coûté à démontrer.
#[test]
fn le_masque_demande_exactement_les_sept_notifications_de_f2() {
    assert_eq!(MASQUE.count_ones(), 7, "masque 0x{MASQUE:X}");
    assert_eq!(
        MASQUE,
        NOTIFY_FILE_PRE_CONVERT_TO_FULL
            | NOTIFY_PRE_RENAME
            | NOTIFY_PRE_DELETE
            | NOTIFY_PRE_SET_HARDLINK
            | NOTIFY_NEW_FILE_CREATED
            | NOTIFY_FILE_OVERWRITTEN
            | NOTIFY_FILE_HANDLE_CLOSED_FILE_MODIFIED
    );
}

/// ⚠️ **`FILE_HANDLE_CLOSED_NO_MODIFICATION` N'EST PAS DEMANDÉE, et c'est une
/// DÉCISION.** Elle arriverait à chaque fermeture de handle en lecture, sur le
/// chemin le plus chaud du pont, pour n'apprendre que ce qu'on sait déjà.
/// Sans ce test, l'ajouter au masque « pour compléter la famille » passerait
/// pour un progrès.
#[test]
fn la_fermeture_sans_modification_n_est_pas_demandee() {
    // 512, `mod.rs:382` côté PRJ_NOTIFY.
    assert_eq!(MASQUE & 512, 0, "masque 0x{MASQUE:X}");
    // Et si elle arrivait quand même, elle retomberait dans le fourre-tout,
    // qui la journalise.
    assert_eq!(decider(512, OUVERT), Reponse::AccepterSansAttendre);
}

/// Une notification que le masque n'a pas demandée ne peut pas arriver — mais
/// si elle arrivait, l'accepter EN SILENCE ferait qu'un masque élargi par
/// erreur passerait inaperçu.
#[test]
fn une_notification_hors_masque_est_acceptee_sans_attendre() {
    // `FILE_OPENED` (mod.rs:338) : jamais demandée.
    assert_eq!(decider(2, OUVERT), Reponse::AccepterSansAttendre);
}
