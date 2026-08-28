//! Les tests de `capteur/sommeil/presse_papier.rs`.
//!
//! **Extraits VERBATIM au sous-bloc P3 (tâche 4), AVANT l'addition qui les a
//! rendus nécessaires** — la mémoire `dernier_presse_papier` de D-P3-2 et la
//! seconde prise de D-P3-6. Le parent était à 379 lignes pour un plafond de
//! 500. La règle du dépôt est d'extraire AVANT d'ajouter, jamais de comprimer
//! après.
//!
//! ⚠️ **Le bloc extrait était AU MILIEU du fichier**, pas en queue : le code de
//! production reprenait juste après (`ecrire`, `ecrire_avec`,
//! `armer_les_gardes`). Le `git diff` en est moins lisible qu'un déplacement de
//! fin de fichier, et le contrôle de transposition caractère pour caractère
//! n'en est que plus obligatoire — il a été joué, et la désindentation de
//! quatre espaces a été vérifiée RÉVERSIBLE.
//!
//! ⚠️ Cet emploi de `#[path]` est HORS de la portée de la « Convention de
//! module enfant » de `CLAUDE.md` : c'est le même mécanisme Rust employé pour
//! une autre raison — la règle des 500 lignes —, exactement comme
//! `superviseur/table.rs` et `presse_papier.rs`. Ce module ne se hisse PAS à la
//! racine du crate.

use crate::capteur::sommeil::file::ReceveurSession;
use crate::capteur::sommeil::tests::{premier_ordre, verrouiller_pour_le_test};
use crate::capteur::sommeil::{etat, inscrire, retirer, signaler, Message};
use crate::capteur::vivier::Ordre;
use crate::presse_papier::Annonce;
use crate::presse_papier::Sondeur;

/// Le dernier presse-papier reçu sur un canal, en vidant ce qui s'y
/// trouve : parts et ordres de sommeil s'y intercalent librement.
fn dernier_presse_papier(canal: &ReceveurSession) -> Option<(Option<String>, u32)> {
    canal
        .vider()
        .into_iter()
        .filter_map(|m| match m {
            Message::PressePapier { texte, octets } => Some((texte, octets)),
            _ => None,
        })
        .last()
}

/// **Toutes les fenêtres reçoivent, pas seulement la focalisée** : chaque
/// fenêtre navigateur a son propre presse-papier local, et c'est le client
/// qui décide s'il écrit maintenant ou au retour du focus.
#[test]
fn une_annonce_de_texte_part_vers_toutes_les_sessions_inscrites() {
    let _verrou = verrouiller_pour_le_test();
    let (canal_a, generation_a) = inscrire("pp-a", 7100);
    let (canal_b, generation_b) = inscrire("pp-b", 7101);
    signaler("pp-a", true, true);

    super::distribuer(&mut etat(), Annonce::Texte("bonjour".to_string()));

    assert_eq!(
        dernier_presse_papier(&canal_a),
        Some((Some("bonjour".to_string()), 7)),
        "la fenêtre focalisée doit recevoir le texte"
    );
    assert_eq!(
        dernier_presse_papier(&canal_b),
        Some((Some("bonjour".to_string()), 7)),
        "la fenêtre NON focalisée aussi : c'est le client qui décide d'écrire"
    );

    retirer("pp-a", generation_a);
    retirer("pp-b", generation_b);
}

/// Un refus voyage par la même variante, `texte` à `None` et `octets`
/// portant la taille refusée — c'est ce qui permet au bandeau du
/// navigateur de dire *combien* plutôt que « trop grand ».
#[test]
fn un_refus_part_sans_texte_mais_avec_sa_taille() {
    let _verrou = verrouiller_pour_le_test();
    let (canal, generation) = inscrire("pp-refus", 7200);

    super::distribuer(&mut etat(), Annonce::Refus { octets: 100_000 });

    assert_eq!(
        dernier_presse_papier(&canal),
        Some((None, 100_000)),
        "un refus doit partir, et porter sa taille"
    );

    retirer("pp-refus", generation);
}

/// Même remède, et pour la même raison, que
/// `un_canal_rompu_detecte_par_les_parts_est_retire_du_vivier` : un fil de
/// fenêtre qui meurt sans passer par `retirer` (une panique court-circuite
/// le point de passage unique de `Fenetre::servir`) laisse une entrée dans
/// `canaux` ET dans le vivier, où elle occuperait une place d'encodeur
/// pour toute la vie du processus. `oublier` — jamais un `remove` direct —
/// est ce qui retire les deux.
///
/// ⚠️ **Ce test observe le vivier DIRECTEMENT, et c'est ce qui le rend
/// discriminant.** Une première rédaction jugeait sur « une session neuve
/// arrive-t-elle à s'éveiller » — et elle passait AVEC UN DISTRIBUTEUR
/// VIDE : `inscrire` et `signaler` appellent tous deux
/// `parts::distribuer_les_parts`, qui détecte la même rupture par son
/// propre chemin et libère la place à la place de celui-ci. Le contrôle ne
/// pouvait donc pas échouer — exactement le patron que ce dépôt paie
/// depuis D6. Ici, rien ne s'intercale entre la rupture et l'observation.
#[test]
fn un_canal_rompu_detecte_par_le_presse_papier_est_retire_du_vivier() {
    let _verrou = verrouiller_pour_le_test();
    let (canal_mort, generation_morte) = inscrire("pp-mort", 7300);
    signaler("pp-mort", true, false);
    assert_eq!(
        premier_ordre(&canal_mort),
        Some(Ordre::Reveiller),
        "pp-mort devrait s'éveiller avant qu'on ne tue son fil"
    );

    // Le fil « meurt » : son récepteur est jeté SANS passer par `retirer`.
    drop(canal_mort);

    // Aucun appel public entre la rupture et l'observation : ni `inscrire`
    // ni `signaler`, qui détecteraient la rupture par le chemin des parts.
    super::distribuer(&mut etat(), Annonce::Texte("bonjour".to_string()));

    assert!(
        !etat().vivier.eveillees().iter().any(|s| s == "pp-mort"),
        "la session dont le canal est rompu doit être retirée du VIVIER, \
         pas seulement de `canaux` : sa place d'encodeur resterait sinon \
         occupée pour toute la vie du processus"
    );

    retirer("pp-mort", generation_morte);
}

// -----------------------------------------------------------------------
// Sous-bloc P2 — l'écriture par le propriétaire, et l'armement des gardes.
// -----------------------------------------------------------------------

/// Un `Sondeur` qui a déjà pris sa référence : c'est l'état nominal après
/// le premier tour, et le seul dans lequel les gardes se jugent.
fn sondeur_amorce() -> Sondeur {
    let mut sondeur = Sondeur::nouveau();
    assert_eq!(sondeur.observer(1, || Some(String::from("etat-initial"))), None);
    sondeur
}

/// Le fil de fenêtre pose notre écriture dans `Etat` ; `armer_les_gardes`
/// la consomme et arme le `Sondeur`. Le témoin n'est pas que rien ne soit
/// annoncé — ce serait aussi vrai du garde n°2 — mais que le presse-papier
/// ne soit **même pas rouvert** : la fermeture de lecture PANIQUE si elle
/// est appelée.
///
/// ROUGE si `armer_les_gardes` ne consomme rien, ou ne pose pas
/// `reference`.
///
/// 🔴 **CE QUE CE TEST NE COUVRE PAS, ET C'EST MESURÉ, PAS SUPPOSÉ.** Il
/// établit que le mécanisme est juste **quand on l'appelle avant
/// `tour()`** ; il n'établit **pas** que le tour de roue l'appelle bien
/// dans cet ordre. Vérifié par mutation le 21 août 2026 : intervertir les
/// deux lignes de `registre.rs::demarrer_le_tour_de_roue` laisse **les sept
/// tests de ce module VERTS**. Le corps du tour de roue est une boucle
/// infinie dans un `thread::spawn`, qu'aucun test d'hôte n'atteint —
/// l'ordre y est un fait de LECTURE, et sa seule épreuve est la recette
/// (critère ④, qui compte les messages de retour).
#[test]
fn armer_les_gardes_empeche_de_relire_notre_ecriture() {
    let _verrou = verrouiller_pour_le_test();
    etat().notre_ecriture = Some((42, String::from("colle")));

    let mut sondeur = sondeur_amorce();
    super::armer_les_gardes(&mut sondeur);

    assert_eq!(
        sondeur.observer(42, || panic!("le garde n°1 a laissé rouvrir le presse-papier")),
        None
    );
    // Consommée : un second armement ne trouve plus rien.
    assert!(etat().notre_ecriture.is_none());
}

/// 🔴 **Le garde reste EXACT au sens de D5, et c'est ce que ce test
/// mesure.** Poser `reference` sur *notre* `seq` ne masque pas une copie
/// TIERCE survenue depuis : le compteur a encore bougé, et cette copie doit
/// être annoncée.
///
/// ROUGE si l'armement posait un état « on se tait désormais ». Sans ce
/// test, un garde trop large passerait le précédent.
#[test]
fn une_copie_tierce_survenue_apres_notre_ecriture_est_quand_meme_annoncee() {
    let _verrou = verrouiller_pour_le_test();
    etat().notre_ecriture = Some((42, String::from("colle")));

    let mut sondeur = sondeur_amorce();
    super::armer_les_gardes(&mut sondeur);

    assert_eq!(
        sondeur.observer(43, || Some(String::from("autre chose"))),
        Some(Annonce::Texte(String::from("autre chose")))
    );
}

/// 🔴 **Une écriture ÉCHOUÉE n'arme AUCUN garde**, et c'est le cas le plus
/// dangereux : armer avant de savoir ferait sortir de l'observation un
/// contenu qui n'a jamais atteint le presse-papier. Ce contenu deviendrait
/// alors invisible **à jamais** — le tour suivant ne le verrait pas comme
/// un changement.
///
/// L'écrivain est INJECTÉ, exactement comme la fermeture de lecture de
/// `Sondeur::observer` : c'est ce qui rend ce chemin éprouvable sur l'hôte
/// sans le moindre `cfg`.
///
/// ROUGE si `ecrire_avec` posait `notre_ecriture` avant d'appeler
/// l'écrivain, ou si elle ignorait son `Err`.
#[test]
fn une_ecriture_echouee_n_arme_aucun_garde() {
    let _verrou = verrouiller_pour_le_test();
    etat().notre_ecriture = None;

    let resultat = super::ecrire_avec("colle", |_| anyhow::bail!("OpenClipboard refusé"));

    assert!(resultat.is_err(), "l'échec doit remonter à l'appelant");
    assert!(
        etat().notre_ecriture.is_none(),
        "rien ne doit être posé quand l'écriture a échoué"
    );
}

/// Le pendant : une écriture RÉUSSIE pose bien le couple, avec le numéro
/// que l'écrivain a rendu — celui relu APRÈS `CloseClipboard`.
///
/// ROUGE si `ecrire_avec` posait un numéro fabriqué au lieu de celui de
/// l'écrivain : le garde n°1 serait alors faux d'un cran, c'est-à-dire
/// silencieusement inopérant.
#[test]
fn une_ecriture_reussie_pose_le_numero_rendu_par_l_ecrivain() {
    let _verrou = verrouiller_pour_le_test();
    etat().notre_ecriture = None;

    super::ecrire_avec("colle", |texte| {
        assert_eq!(texte, "colle", "le texte doit arriver tel quel à Win32");
        Ok(1234)
    })
    .expect("écriture");

    assert_eq!(
        etat().notre_ecriture.clone(),
        Some((1234, String::from("colle")))
    );
    etat().notre_ecriture = None;
}

/// La SECONDE PRISE de D-P3-6, sur SA BRANCHE : elle doit CONSOMMER
/// `etat().notre_ecriture` et écarter l'annonce qui porte notre propre texte.
///
/// ⚠️ **Ce test couvre la lecture d'`etat()`, que la règle pure de
/// `Sondeur::ecarter` ne peut pas couvrir** — c'est le seul endroit où la
/// branche est éprouvable. Le SITE D'APPEL, lui (`registre.rs`, entre `tour()`
/// et `distribuer`), n'est couvert par aucun test d'hôte : il vit dans le fil
/// du tour de roue. Dit plutôt que tu.
///
/// ROUGE si `filtrer_nos_ecritures_tardives` ne prend pas le couple, ou s'il
/// le lit sans le consommer — la deuxième assertion tomberait.
#[test]
fn la_seconde_prise_consomme_notre_ecriture_et_ecarte_notre_texte() {
    let _verrou = verrouiller_pour_le_test();
    etat().notre_ecriture = Some((11, String::from("colle-par-B")));

    let mut sondeur = Sondeur::nouveau();
    let annonce = Some(Annonce::Texte(String::from("colle-par-B")));

    assert_eq!(
        super::filtrer_nos_ecritures_tardives(&mut sondeur, annonce),
        None,
        "notre propre texte ne doit pas repartir vers les N fenêtres"
    );
    assert_eq!(
        etat().notre_ecriture.clone(),
        None,
        "le couple doit être CONSOMMÉ : le laisser le ferait rejouer au tour suivant"
    );
}

/// Sans écriture de notre part, la seconde prise est transparente.
///
/// ROUGE si elle écartait tout : une copie faite dans la VM n'arriverait
/// jamais nulle part, et le sens descendant serait mort.
#[test]
fn la_seconde_prise_laisse_passer_une_copie_de_la_vm() {
    let _verrou = verrouiller_pour_le_test();
    etat().notre_ecriture = None;

    let mut sondeur = Sondeur::nouveau();
    let annonce = Some(Annonce::Texte(String::from("copie-dans-la-vm")));

    assert_eq!(
        super::filtrer_nos_ecritures_tardives(&mut sondeur, annonce.clone()),
        annonce
    );
}

// ── L'ÉTAT COURANT À L'INSCRIPTION — moitié AGENT du legs n°3 de P1 ──────
//
// 🔴 **CES TESTS ONT ÉTÉ ÉCRITS DANS `sommeil/tests.rs` PUIS DÉPLACÉS ICI,
// PARCE QU'ILS L'ONT FAIT FRANCHIR LE PLAFOND** — 417 → 562 lignes, pour une
// porte de 500. E13 du plan l'avait annoncé (« si l'addition le porte au-delà
// de 480, extraire AVANT d'écrire, jamais comprimer ») et la mesure n'a pas
// été prise d'avance. **L'extraction est jouée, jamais une compression** :
// c'est la règle du dépôt, que D9 a payée deux fois pour l'avoir oubliée.
//
// ⚠️ **Et ce n'est pas seulement un déménagement de commodité.** E13 range les
// tests de `registre.rs` dans `sommeil/tests.rs`, faute de module de tests chez
// lui ; mais ce que ces tests exercent est `emettre_l_etat_courant`, qui vit
// dans le PARENT de ce fichier-ci. Ils sont donc auprès de la fonction qu'ils
// éprouvent, et le helper `dernier_presse_papier` qui vit déjà ici leur sert
// tel quel — le jumeau écrit dans `sommeil/tests.rs` faisait double emploi et
// n'a pas suivi.

/// 🔴 ROUGE SUR L'ARBRE INTACT avant le remède : c'est le legs n°3 de P1 —
/// « une fenêtre attachée après une copie ne reçoit jamais ce contenu ».
#[test]
fn une_session_qui_s_inscrit_apres_une_copie_recoit_le_contenu_courant() {
    let _verrou = verrouiller_pour_le_test();
    etat().dernier_presse_papier = None;
    let (canal_present, generation_present) = inscrire("t11-present", 7300);
    super::distribuer(&mut etat(), crate::presse_papier::Annonce::Texte("deja-copie".into()));
    let _ = dernier_presse_papier(&canal_present);

    // La fenêtre s'attache APRÈS la copie.
    let (canal_tardif, generation_tardif) = inscrire("t11-tardif", 7301);

    assert_eq!(
        dernier_presse_papier(&canal_tardif),
        Some((Some("deja-copie".to_string()), 10)),
        "une fenêtre attachée après la copie doit recevoir le contenu courant"
    );

    etat().dernier_presse_papier = None;
    retirer("t11-present", generation_present);
    retirer("t11-tardif", generation_tardif);
    drop(canal_present);
    drop(canal_tardif);
}

/// ROUGE = appeler `distribuer` au lieu d'envoyer sur le seul canal neuf : les
/// voisines recevraient aussi, et le contenu serait rejoué à TOUTES les
/// fenêtres à chaque attache.
#[test]
fn l_emission_a_l_inscription_ne_part_que_sur_le_canal_neuf() {
    let _verrou = verrouiller_pour_le_test();
    etat().dernier_presse_papier = None;
    let (canal_present, generation_present) = inscrire("t12-present", 7310);
    super::distribuer(&mut etat(), crate::presse_papier::Annonce::Texte("copie".into()));
    // On vide ce que la voisine a légitimement reçu de `distribuer`.
    let _ = dernier_presse_papier(&canal_present);

    let (canal_neuf, generation_neuf) = inscrire("t12-neuf", 7311);

    assert_eq!(
        dernier_presse_papier(&canal_neuf),
        Some((Some("copie".to_string()), 5)),
        "le canal neuf reçoit"
    );
    assert_eq!(
        dernier_presse_papier(&canal_present),
        None,
        "la voisine NE DOIT RIEN recevoir de plus : ce serait un aller-retour par attache"
    );

    etat().dernier_presse_papier = None;
    retirer("t12-present", generation_present);
    retirer("t12-neuf", generation_neuf);
    drop(canal_present);
    drop(canal_neuf);
}

/// ROUGE = ne mémoriser que `Annonce::Texte` : la fenêtre attendrait alors un
/// contenu qui n'arrivera jamais, sans le bandeau qui lui dit pourquoi.
#[test]
fn une_session_qui_s_inscrit_apres_un_refus_recoit_le_refus() {
    let _verrou = verrouiller_pour_le_test();
    etat().dernier_presse_papier = None;
    super::distribuer(&mut etat(), crate::presse_papier::Annonce::Refus { octets: 123_456 });

    let (canal, generation) = inscrire("t13-refus", 7320);

    assert_eq!(
        dernier_presse_papier(&canal),
        Some((None, 123_456)),
        "le refus doit être rejoué à l'attache, avec sa taille"
    );

    etat().dernier_presse_papier = None;
    retirer("t13-refus", generation);
    drop(canal);
}

/// ROUGE = émettre un `Message::PressePapier` vide quand la mémoire est
/// `None` : le client écrirait alors une chaîne vide dans son presse-papier
/// local à chaque attache.
#[test]
fn une_session_qui_s_inscrit_avant_toute_copie_ne_recoit_rien() {
    let _verrou = verrouiller_pour_le_test();
    etat().dernier_presse_papier = None;

    let (canal, generation) = inscrire("t14-vierge", 7330);

    assert_eq!(dernier_presse_papier(&canal), None);

    retirer("t14-vierge", generation);
    drop(canal);
}

/// 🔴 AUCUNE PURGE À LA RÉ-INSCRIPTION (D-P3-3) — et la symétrie avec
/// `dernieres_parts` / `derniers_audio`, que `inscrire` purge quelques lignes
/// plus haut, est TROMPEUSE.
///
/// ROUGE = copier cette purge par symétrie de forme : la mémoire serait
/// retirée au moment précis où l'on veut s'en servir, et le rattachement — le
/// cas où le rejeu est le PLUS utile — ne rejouerait rien.
#[test]
fn un_rattachement_recoit_lui_aussi_le_contenu_courant() {
    let _verrou = verrouiller_pour_le_test();
    etat().dernier_presse_papier = None;
    let (premier_canal, premiere_generation) = inscrire("t15-rattache", 7340);
    super::distribuer(&mut etat(), crate::presse_papier::Annonce::Texte("avant-rupture".into()));
    let _ = dernier_presse_papier(&premier_canal);

    // Le rattachement, sous le MÊME nom.
    let (second_canal, seconde_generation) = inscrire("t15-rattache", 7340);

    assert_eq!(
        dernier_presse_papier(&second_canal),
        Some((Some("avant-rupture".to_string()), 13)),
        "un rattachement doit recevoir le contenu courant : rien n'est purgé"
    );

    etat().dernier_presse_papier = None;
    retirer("t15-rattache", seconde_generation);
    let _ = premiere_generation;
    drop(premier_canal);
    drop(second_canal);
}
