use super::*;

/// The nominal state: writable root, open channel, mutations armed.
const OUVERT: Etat = Etat {
    inscriptible: true,
    canal_ouvert: true,
    mutations_armees: true,
};
/// F1's state, which `PONT_ECRITURE` no longer sets but which the code can hold.
const LECTURE_SEULE: Etat = Etat {
    inscriptible: false,
    canal_ouvert: true,
    mutations_armees: true,
};
const CANAL_FERME: Etat = Etat {
    inscriptible: true,
    canal_ouvert: false,
    mutations_armees: true,
};
/// **F3** — `PONT_MUTATION=0`. Everything else is nominal.
const MUTATIONS_DESARMEES: Etat = Etat {
    inscriptible: true,
    canal_ouvert: true,
    mutations_armees: false,
};

/// The three states F1 and F2 swept, plus F3's.
const TOUS_LES_ETATS: [Etat; 4] = [OUVERT, LECTURE_SEULE, CANAL_FERME, MUTATIONS_DESARMEES];

/// 🔴 **THE ONLY REFUSAL GATE FOR A WRITE, and it bears on a STATE.**
///
/// If `PRE_CONVERT_TO_FULL` stopped being refused on a non-writable
/// root, a write WOULD SUCCEED locally on the VM with nothing
/// pushing it — the silent loss this whole sub-project exists to
/// forbid.
#[test]
fn une_ecriture_sur_racine_non_inscriptible_est_refusee() {
    assert_eq!(
        decider(PRE_CONVERT_TO_FULL, LECTURE_SEULE, Cible::SansObjet),
        Reponse::Refuser(Erreur::ProtegeEnEcriture)
    );
}

/// 🔴 **TWO CAUSES NEVER SHARE A CODE** (spec §5.1).
///
/// A closed channel returns `ERROR_IO_DEVICE`, not `ERROR_WRITE_PROTECT`: "this
/// share is read-only" and "the tab is closed" do not call for the
/// same gesture from the user, and it is the only moment we can still
/// tell them.
#[test]
fn une_ecriture_sur_canal_ferme_est_refusee_en_erreur_d_e_s() {
    assert_eq!(
        decider(PRE_CONVERT_TO_FULL, CANAL_FERME, Cible::SansObjet),
        Reponse::Refuser(Erreur::CanalFerme)
    );
    // …and the two causes are not confused.
    assert_ne!(
        decider(PRE_CONVERT_TO_FULL, CANAL_FERME, Cible::SansObjet),
        decider(PRE_CONVERT_TO_FULL, LECTURE_SEULE, Cible::SansObjet)
    );
}

/// Writing is ALLOWED in the nominal state — it is the only line of F2
/// that changes what an application gets.
#[test]
fn une_ecriture_est_autorisee_quand_la_racine_est_inscriptible_et_le_canal_ouvert() {
    assert_eq!(
        decider(PRE_CONVERT_TO_FULL, OUVERT, Cible::SansObjet),
        Reponse::Autoriser
    );
}

/// ❌ **`un_renommage_et_une_suppression_restent_refuses_en_f2` WAS DELETED,
/// AND ITS REASON IS WRITTEN HERE RATHER THAN LOST WITH IT.**
///
/// It required `PRE_RENAME` and `PRE_DELETE` to be refused **whatever
/// the state**, on the grounds that "accepting them without being able to push them would leave the
/// local workstation on the old content". **The grounds were right, and they stopped
/// being so**: F3 knows how to push them. Keeping it would have forced F3 to bypass it,
/// that is, to empty a guard rather than satisfy it.
///
/// What replaces it below is **more demanding**, not less: four
/// named refusal states, each with its own cause, and an acceptance that
/// is only possible in the nominal state.
///
/// 🔴 **A MUTATION IS REFUSED ON A STATE, AND THE FOUR STATES ARE
/// DISTINGUISHED.**
#[test]
fn une_mutation_est_refusee_sur_chacun_des_quatre_etats_qui_l_empechent() {
    for code in [PRE_RENAME, PRE_DELETE] {
        assert_eq!(
            decider(code, MUTATIONS_DESARMEES, Cible::DansLaRacine),
            Reponse::Refuser(Erreur::ProtegeEnEcriture),
            "PONT_MUTATION=0, code {code}"
        );
        assert_eq!(
            decider(code, LECTURE_SEULE, Cible::DansLaRacine),
            Reponse::Refuser(Erreur::ProtegeEnEcriture),
            "racine en lecture seule, code {code}"
        );
        // 🔴 **TWO CAUSES NEVER SHARE A CODE** (spec §5.1): a closed
        // channel returns `ERROR_IO_DEVICE`, not `ERROR_WRITE_PROTECT`. "The tab
        // is closed" and "this share is read-only" do not call for the
        // same gesture.
        assert_eq!(
            decider(code, CANAL_FERME, Cible::DansLaRacine),
            Reponse::Refuser(Erreur::CanalFerme),
            "canal fermé, code {code}"
        );
    }
    // The fourth state only applies to renaming: a deletion has no
    // destination.
    assert_eq!(
        decider(PRE_RENAME, OUVERT, Cible::HorsRacine),
        Reponse::Refuser(Erreur::NonSupporte),
        "une cible hors racine n'est pas un refus de DROIT"
    );
}

/// 🔴 **`NonSupporte` AND `ProtegeEnEcriture` ARE NOT CONFUSED.**
///
/// Red: return `ProtegeEnEcriture` on an out-of-root target. Two distinct
/// causes would then share `ERROR_WRITE_PROTECT`, which spec §5.1
/// forbids — and the user would look for a permission where there is
/// simply no handle.
#[test]
fn un_renommage_hors_racine_est_nonsupporte_et_pas_protegeenecriture() {
    let hors = decider(PRE_RENAME, OUVERT, Cible::HorsRacine);
    assert_eq!(hors, Reponse::Refuser(Erreur::NonSupporte));
    assert_ne!(hors, Reponse::Refuser(Erreur::ProtegeEnEcriture));
}

/// In the nominal state, both mutations are ALLOWED — and it is the only
/// line of F3 that changes what an application gets.
#[test]
fn une_mutation_est_autorisee_dans_l_etat_nominal() {
    assert_eq!(
        decider(PRE_RENAME, OUVERT, Cible::DansLaRacine),
        Reponse::Autoriser
    );
    assert_eq!(
        decider(PRE_DELETE, OUVERT, Cible::SansObjet),
        Reponse::Autoriser
    );
}

/// 🔴 **F3'S TWO POSTS TRIGGER A PUSH, AND TWO DISTINCT
/// PUSHES.**
///
/// Red: leave them as `AccepterSansAttendre`. They would fall back into the
/// catch-all arm — which `chaque_bit_du_masque_a_une_decision_nommee` catches
/// — and **nothing would ever be pushed**, on a product that appears to
/// work: the application sees its renaming succeed in the VM, and the local
/// workstation keeps the old name.
#[test]
fn les_deux_post_de_f3_declenchent_chacune_sa_poussee() {
    assert_eq!(
        decider(FILE_RENAMED, OUVERT, Cible::DansLaRacine),
        Reponse::Pousser(Poussee::Renommage)
    );
    assert_eq!(
        decider(FILE_HANDLE_CLOSED_FILE_DELETED, OUVERT, Cible::SansObjet),
        Reponse::Pousser(Poussee::Suppression)
    );
    // The four pushes are distinct: confusing a renaming and a
    // deletion would destroy in one direction or the other.
    assert_ne!(
        decider(FILE_RENAMED, OUVERT, Cible::DansLaRacine),
        decider(FILE_HANDLE_CLOSED_FILE_DELETED, OUVERT, Cible::SansObjet)
    );
}

/// ⚠️ **AN F3 POST GOES OUT EVEN WHEN THE `PRE_` WOULD HAVE REFUSED.**
///
/// It is not an inconsistency: it is the nature of a POST. If it arrives,
/// the gesture has happened in the VM — and pushing nothing would let the local workstation
/// diverge silently, which is worse than pushing.
#[test]
fn une_post_de_f3_part_quel_que_soit_l_etat() {
    for etat in TOUS_LES_ETATS {
        for code in [FILE_RENAMED, FILE_HANDLE_CLOSED_FILE_DELETED] {
            assert!(
                matches!(
                    decider(code, etat, Cible::DansLaRacine),
                    Reponse::Pousser(_)
                ),
                "code {code} dans l'état {etat:?}"
            );
        }
    }
}

/// Hard links have no equivalent in the File System Access API: it
/// is not a read-only refusal, it is an operation that does not exist on
/// the other side (spec §3.5.2).
///
/// ⚠️ `HARDLINK_CREATED` is a **POST**: the refusal prevents nothing, it
/// logs. The test pins the decision, not an effect.
#[test]
fn les_liens_durs_sont_refuses_en_non_supporte() {
    for code in [PRE_SET_HARDLINK, HARDLINK_CREATED] {
        assert_eq!(
            decider(code, OUVERT, Cible::SansObjet),
            Reponse::Refuser(Erreur::NonSupporte),
            "code {code}"
        );
    }
}

/// 🔴 **THE TWO CONTENT POSTS TRIGGER A PUSH.**
///
/// Forgetting `FILE_OVERWRITTEN` would silently lose any save that
/// truncates at opening (`CREATE_ALWAYS`, `TRUNCATE_EXISTING`) without ever
/// closing the handle on a modification — that is, a good share of
/// "in place" saves.
#[test]
fn les_deux_post_de_contenu_declenchent_une_poussee() {
    for code in [FILE_OVERWRITTEN, FILE_HANDLE_CLOSED_FILE_MODIFIED] {
        assert_eq!(
            decider(code, OUVERT, Cible::SansObjet),
            Reponse::Pousser(Poussee::Contenu),
            "code {code}"
        );
    }
}

/// A creation is pushed, and **as a creation, not as content**:
/// a directory has no byte to read.
#[test]
fn un_fichier_neuf_est_pousse_comme_une_creation() {
    assert_eq!(
        decider(NEW_FILE_CREATED, OUVERT, Cible::SansObjet),
        Reponse::Pousser(Poussee::Creation)
    );
    assert_ne!(
        decider(NEW_FILE_CREATED, OUVERT, Cible::SansObjet),
        Reponse::Pousser(Poussee::Contenu),
        "une création n'est pas un contenu : un répertoire n'a rien à lire"
    );
}

/// ⚠️ **A POST IS NOT REFUSABLE, so the state does not change it.**
///
/// Saying so is the remedy to the trap this module paid for in F1: believing that a
/// refusal returned on a POST prevents anything. A push goes out even with the
/// channel closed — the write thread journals it and will hold it back.
#[test]
fn une_poussee_part_meme_canal_ferme_car_une_post_ne_se_refuse_pas() {
    for etat in TOUS_LES_ETATS {
        assert!(
            matches!(
                decider(FILE_HANDLE_CLOSED_FILE_MODIFIED, etat, Cible::SansObjet),
                Reponse::Pousser(_)
            ),
            "état {etat:?}"
        );
    }
}

/// 🔴 **The exhaustiveness guard, and it is the one worth the most.** Each bit
/// the mask requests must have a NAMED decision, **in all three states**:
/// a requested bit falling into the catch-all arm would be accepted
/// silently, and a write would be lost.
///
/// ⚠️ **It is this test that made F2 diverge from its plan.** The plan prescribed
/// `AccepterSansAttendre` for an allowed `PRE_CONVERT_TO_FULL`: the bit would
/// then have fallen back into the catch-all, and the obvious remedy — excluding it from the
/// sweep — would have EMPTIED this guard instead of satisfying it. Hence the
/// `Autoriser` variant, which names the acceptance.
#[test]
fn chaque_bit_du_masque_a_une_decision_nommee() {
    for etat in TOUS_LES_ETATS {
        for cible in [Cible::SansObjet, Cible::DansLaRacine, Cible::HorsRacine] {
            for bit in 0..32u32 {
                let drapeau = 1u32 << bit;
                if MASQUE & drapeau == 0 {
                    continue;
                }
                assert_ne!(
                    decider(drapeau as i32, etat, cible),
                    Reponse::AccepterSansAttendre,
                    "le bit 0x{drapeau:X} est DEMANDÉ par le masque et retombe dans le bras \
                 fourre-tout dans l'état {etat:?} / cible {cible:?} : il serait accepté \
                 en silence"
                );
            }
        }
    }
}

/// The mask requests exactly NINE notifications, and not one more.
///
/// ⚠️ **RENAMED, never silently extended** — from
/// `..._les_sept_notifications_de_f2`. A name that lies about its count is a
/// name one stops reading, and this module has already changed count once
/// (five in F1, seven in F2).
///
/// ⚠️ **This test was RED BEFORE the modification**, and it therefore cannot
/// be vacuous: F2's mask carried seven. It is the only test of this
/// module whose reachability cost nothing to demonstrate.
///
/// 🔴 **Forgetting `NOTIFY_FILE_RENAMED` would mean the notification WOULD NEVER
/// ARRIVE, and NOTHING would say so**: `PRE_RENAME` would allow, the application
/// would see its renaming succeed, and the local workstation would keep the old name
/// forever. It is the mute failure this test exists to forbid.
#[test]
fn le_masque_demande_exactement_les_neuf_notifications_de_f3() {
    assert_eq!(MASQUE.count_ones(), 9, "masque 0x{MASQUE:X}");
    assert_eq!(
        MASQUE,
        NOTIFY_FILE_PRE_CONVERT_TO_FULL
            | NOTIFY_PRE_RENAME
            | NOTIFY_PRE_DELETE
            | NOTIFY_PRE_SET_HARDLINK
            | NOTIFY_NEW_FILE_CREATED
            | NOTIFY_FILE_OVERWRITTEN
            | NOTIFY_FILE_HANDLE_CLOSED_FILE_MODIFIED
            | NOTIFY_FILE_RENAMED
            | NOTIFY_FILE_HANDLE_CLOSED_FILE_DELETED
    );
}

/// 🔴 **THE TWO FAMILIES OF CONSTANTS HAVE THE SAME VALUE, AND IT IS NOT
/// GUARANTEED.**
///
/// `PRJ_NOTIFICATION_*` (`i32`, what the callback RECEIVES) and `PRJ_NOTIFY_*`
/// (`u32`, what the MASK requests) carry names alike enough to
/// mislead. The module header says so; this test checks it, for the
/// two pairs F3 adds — the only ones whose divergence would produce a
/// notification requested and never recognised.
#[test]
fn les_deux_familles_de_constantes_de_f3_s_accordent() {
    assert_eq!(FILE_RENAMED as u32, NOTIFY_FILE_RENAMED);
    assert_eq!(
        FILE_HANDLE_CLOSED_FILE_DELETED as u32,
        NOTIFY_FILE_HANDLE_CLOSED_FILE_DELETED
    );
}

/// ⚠️ **`FILE_HANDLE_CLOSED_NO_MODIFICATION` IS NOT REQUESTED, and it is a
/// DECISION.** It would arrive at each closing of a read handle, on the
/// bridge's hottest path, only to learn what we already know.
/// Without this test, adding it to the mask "to complete the family" would pass
/// for progress.
#[test]
fn la_fermeture_sans_modification_n_est_pas_demandee() {
    // 512, `mod.rs:382` on the PRJ_NOTIFY side.
    assert_eq!(MASQUE & 512, 0, "masque 0x{MASQUE:X}");
    // And if it arrived anyway, it would fall back into the catch-all,
    // which logs it.
    assert_eq!(
        decider(512, OUVERT, Cible::SansObjet),
        Reponse::AccepterSansAttendre
    );
}

/// A notification the mask did not request cannot arrive — but
/// if it did, accepting it SILENTLY would let a mask widened by
/// mistake go unnoticed.
#[test]
fn une_notification_hors_masque_est_acceptee_sans_attendre() {
    // `FILE_OPENED` (mod.rs:338): never requested.
    assert_eq!(
        decider(2, OUVERT, Cible::SansObjet),
        Reponse::AccepterSansAttendre
    );
}
