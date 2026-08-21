use super::*;

/// L'état nominal : racine inscriptible, canal ouvert, mutations armées.
const OUVERT: Etat = Etat { inscriptible: true, canal_ouvert: true, mutations_armees: true };
/// L'état de F1, que `PONT_ECRITURE` ne pose plus mais que le code sait tenir.
const LECTURE_SEULE: Etat =
    Etat { inscriptible: false, canal_ouvert: true, mutations_armees: true };
const CANAL_FERME: Etat =
    Etat { inscriptible: true, canal_ouvert: false, mutations_armees: true };
/// **F3** — `PONT_MUTATION=0`. Tout le reste est nominal.
const MUTATIONS_DESARMEES: Etat =
    Etat { inscriptible: true, canal_ouvert: true, mutations_armees: false };

/// Les trois états que F1 et F2 balayaient, plus celui de F3.
const TOUS_LES_ETATS: [Etat; 4] = [OUVERT, LECTURE_SEULE, CANAL_FERME, MUTATIONS_DESARMEES];

/// 🔴 **L'UNIQUE PORTE DE REFUS D'UNE ÉCRITURE, et elle porte sur un ÉTAT.**
///
/// Si `PRE_CONVERT_TO_FULL` cessait d'être refusée sur une racine non
/// inscriptible, une écriture RÉUSSIRAIT localement sur la VM sans que rien ne
/// la pousse — la perte silencieuse que tout ce sous-projet existe pour
/// interdire.
#[test]
fn une_ecriture_sur_racine_non_inscriptible_est_refusee() {
    assert_eq!(
        decider(PRE_CONVERT_TO_FULL, LECTURE_SEULE, Cible::SansObjet),
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
        decider(PRE_CONVERT_TO_FULL, CANAL_FERME, Cible::SansObjet),
        Reponse::Refuser(Erreur::CanalFerme)
    );
    // …et les deux causes ne se confondent pas.
    assert_ne!(
        decider(PRE_CONVERT_TO_FULL, CANAL_FERME, Cible::SansObjet),
        decider(PRE_CONVERT_TO_FULL, LECTURE_SEULE, Cible::SansObjet)
    );
}

/// L'écriture est AUTORISÉE dans l'état nominal — c'est la seule ligne de F2
/// qui change ce qu'une application obtient.
#[test]
fn une_ecriture_est_autorisee_quand_la_racine_est_inscriptible_et_le_canal_ouvert() {
    assert_eq!(decider(PRE_CONVERT_TO_FULL, OUVERT, Cible::SansObjet), Reponse::Autoriser);
}

/// ❌ **`un_renommage_et_une_suppression_restent_refuses_en_f2` A ÉTÉ SUPPRIMÉ,
/// ET SA RAISON EST ÉCRITE ICI PLUTÔT QUE PERDUE AVEC LUI.**
///
/// Il exigeait que `PRE_RENAME` et `PRE_DELETE` soient refusés **quel que soit
/// l'état**, au motif que « les accepter sans pouvoir les pousser laisserait le
/// poste local sur l'ancien contenu ». **Le motif était juste, et il a cessé de
/// l'être** : F3 sait les pousser. Le garder aurait obligé F3 à le contourner,
/// c'est-à-dire à vider un garde plutôt qu'à le satisfaire.
///
/// Ce qui le remplace ci-dessous est **plus exigeant**, pas moins : quatre
/// états de refus nommés, chacun avec sa cause propre, et une acceptation qui
/// n'est possible que dans l'état nominal.
///
/// 🔴 **UNE MUTATION EST REFUSÉE SUR UN ÉTAT, ET LES QUATRE ÉTATS SONT
/// DISTINGUÉS.**
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
        // 🔴 **DEUX CAUSES NE PARTAGENT JAMAIS UN CODE** (spec §5.1) : un canal
        // fermé rend `ERROR_IO_DEVICE`, pas `ERROR_WRITE_PROTECT`. « L'onglet
        // est fermé » et « ce partage est en lecture seule » n'appellent pas le
        // même geste.
        assert_eq!(
            decider(code, CANAL_FERME, Cible::DansLaRacine),
            Reponse::Refuser(Erreur::CanalFerme),
            "canal fermé, code {code}"
        );
    }
    // Le quatrième état ne vaut que pour le renommage : une suppression n'a pas
    // de destination.
    assert_eq!(
        decider(PRE_RENAME, OUVERT, Cible::HorsRacine),
        Reponse::Refuser(Erreur::NonSupporte),
        "une cible hors racine n'est pas un refus de DROIT"
    );
}

/// 🔴 **`NonSupporte` ET `ProtegeEnEcriture` NE SE CONFONDENT PAS.**
///
/// Rouge : rendre `ProtegeEnEcriture` sur une cible hors racine. Deux causes
/// distinctes partageraient alors `ERROR_WRITE_PROTECT`, ce que la spec §5.1
/// interdit — et l'utilisateur chercherait une permission là où il n'y a
/// simplement pas de poignée.
#[test]
fn un_renommage_hors_racine_est_NonSupporte_et_pas_ProtegeEnEcriture() {
    let hors = decider(PRE_RENAME, OUVERT, Cible::HorsRacine);
    assert_eq!(hors, Reponse::Refuser(Erreur::NonSupporte));
    assert_ne!(hors, Reponse::Refuser(Erreur::ProtegeEnEcriture));
}

/// Dans l'état nominal, les deux mutations sont AUTORISÉES — et c'est la seule
/// ligne de F3 qui change ce qu'une application obtient.
#[test]
fn une_mutation_est_autorisee_dans_l_etat_nominal() {
    assert_eq!(decider(PRE_RENAME, OUVERT, Cible::DansLaRacine), Reponse::Autoriser);
    assert_eq!(decider(PRE_DELETE, OUVERT, Cible::SansObjet), Reponse::Autoriser);
}

/// 🔴 **LES DEUX POST DE F3 DÉCLENCHENT UNE POUSSÉE, ET DEUX POUSSÉES
/// DISTINCTES.**
///
/// Rouge : les laisser en `AccepterSansAttendre`. Elles retomberaient dans le
/// bras fourre-tout — que `chaque_bit_du_masque_a_une_decision_nommee` attrape
/// — et **rien ne serait jamais poussé**, sur un produit qui a l'air de
/// marcher : l'application voit son renommage réussir dans la VM, et le poste
/// local garde l'ancien nom.
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
    // Les quatre poussées sont distinctes : confondre un renommage et une
    // suppression détruirait dans un sens ou dans l'autre.
    assert_ne!(
        decider(FILE_RENAMED, OUVERT, Cible::DansLaRacine),
        decider(FILE_HANDLE_CLOSED_FILE_DELETED, OUVERT, Cible::SansObjet)
    );
}

/// ⚠️ **UNE POST DE F3 PART MÊME QUAND LE `PRE_` AURAIT REFUSÉ.**
///
/// Ce n'est pas une incohérence : c'est la nature d'une POST. Si elle arrive,
/// le geste a eu lieu dans la VM — et ne rien pousser laisserait le poste local
/// diverger en silence, ce qui est pire que de pousser.
#[test]
fn une_post_de_f3_part_quel_que_soit_l_etat() {
    for etat in TOUS_LES_ETATS {
        for code in [FILE_RENAMED, FILE_HANDLE_CLOSED_FILE_DELETED] {
            assert!(
                matches!(decider(code, etat, Cible::DansLaRacine), Reponse::Pousser(_)),
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
        assert_eq!(decider(code, OUVERT, Cible::SansObjet), Reponse::Refuser(Erreur::NonSupporte), "code {code}");
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
            decider(code, OUVERT, Cible::SansObjet),
            Reponse::Pousser(Poussee::Contenu),
            "code {code}"
        );
    }
}

/// Une création est poussée, et **comme une création, pas comme un contenu** :
/// un répertoire n'a aucun octet à lire.
#[test]
fn un_fichier_neuf_est_pousse_comme_une_creation() {
    assert_eq!(decider(NEW_FILE_CREATED, OUVERT, Cible::SansObjet), Reponse::Pousser(Poussee::Creation));
    assert_ne!(
        decider(NEW_FILE_CREATED, OUVERT, Cible::SansObjet),
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
    for etat in TOUS_LES_ETATS {
        assert!(
            matches!(decider(FILE_HANDLE_CLOSED_FILE_MODIFIED, etat, Cible::SansObjet), Reponse::Pousser(_)),
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

/// Le masque demande exactement NEUF notifications, et pas une de plus.
///
/// ⚠️ **RENOMMÉ, jamais rallongé en silence** — de
/// `..._les_sept_notifications_de_f2`. Un nom qui ment sur son compte est un
/// nom qu'on cesse de lire, et ce module a déjà changé de compte une fois
/// (cinq en F1, sept en F2).
///
/// ⚠️ **Ce test était ROUGE AVANT la modification**, et il ne peut donc pas
/// être vacueux : le masque de F2 en portait sept. C'est le seul test de ce
/// module dont l'atteignabilité n'a rien coûté à démontrer.
///
/// 🔴 **Oublier `NOTIFY_FILE_RENAMED` ferait que la notification N'ARRIVERAIT
/// JAMAIS, et RIEN ne le dirait** : le `PRE_RENAME` autoriserait, l'application
/// verrait son renommage réussir, et le poste local garderait l'ancien nom pour
/// toujours. C'est la panne muette que ce test existe pour interdire.
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

/// 🔴 **LES DEUX FAMILLES DE CONSTANTES ONT LA MÊME VALEUR, ET CE N'EST PAS
/// GARANTI.**
///
/// `PRJ_NOTIFICATION_*` (`i32`, ce que le rappel REÇOIT) et `PRJ_NOTIFY_*`
/// (`u32`, ce que le MASQUE demande) portent des noms qui se ressemblent au
/// point de tromper. L'en-tête du module le dit ; ce test le vérifie, pour les
/// deux paires que F3 ajoute — les seules dont une divergence produirait une
/// notification demandée et jamais reconnue.
#[test]
fn les_deux_familles_de_constantes_de_f3_s_accordent() {
    assert_eq!(FILE_RENAMED as u32, NOTIFY_FILE_RENAMED);
    assert_eq!(FILE_HANDLE_CLOSED_FILE_DELETED as u32, NOTIFY_FILE_HANDLE_CLOSED_FILE_DELETED);
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
    assert_eq!(decider(512, OUVERT, Cible::SansObjet), Reponse::AccepterSansAttendre);
}

/// Une notification que le masque n'a pas demandée ne peut pas arriver — mais
/// si elle arrivait, l'accepter EN SILENCE ferait qu'un masque élargi par
/// erreur passerait inaperçu.
#[test]
fn une_notification_hors_masque_est_acceptee_sans_attendre() {
    // `FILE_OPENED` (mod.rs:338) : jamais demandée.
    assert_eq!(decider(2, OUVERT, Cible::SansObjet), Reponse::AccepterSansAttendre);
}
