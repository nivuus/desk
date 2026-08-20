use std::sync::{Arc, Mutex};
use std::time::Instant;

use proto::input::InputMessage;

use super::*;
use crate::h264::AccessUnit;
use crate::source::VideoSource;
use crate::transport::Session;

/// Trace ORDONNÉE partagée entre la source factice et le rappel d'entrée :
/// c'est elle qui rend l'ordre de D6 observable, et pas seulement ses deux
/// effets pris séparément.
type Trace = Arc<Mutex<Vec<String>>>;

/// Source factice qui enregistre chaque écriture de presse-papier et rend la
/// réponse préparée.
///
/// ⚠️ **Elle REDÉFINIT `ecrire_le_presse_papier`, et c'est voulu ici** : ce
/// fichier éprouve l'usage que `collage` fait de la méthode, pas son défaut.
/// Le défaut du trait — qui doit rendre `Err` et non `Ok(())`, précisément
/// pour que le piège de D10 ne se rejoue pas — est éprouvé à part, dans
/// `capteur/distante/tests_etats.rs`.
struct SourceQuiEcrit {
    inner: crate::source::FileSource,
    trace: Trace,
    accepte: bool,
}

impl VideoSource for SourceQuiEcrit {
    fn next_frame(&mut self) -> Option<AccessUnit> {
        self.inner.next_frame()
    }
    fn dimensions(&self) -> (u32, u32) {
        self.inner.dimensions()
    }
    fn ecrire_le_presse_papier(&mut self, texte: &str) -> anyhow::Result<()> {
        self.trace.lock().unwrap().push(format!("ecrire:{texte}"));
        if self.accepte {
            Ok(())
        } else {
            anyhow::bail!("le capteur a refusé : OpenClipboard")
        }
    }
}

fn session_qui_ecrit(accepte: bool) -> (Session, Trace) {
    let trace: Trace = Arc::new(Mutex::new(Vec::new()));
    let source_path =
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/testsrc.264"));
    let inner = crate::source::FileSource::from_path(source_path, 1280, 720, 60)
        .expect("chargement du flux de test");
    let source = Box::new(SourceQuiEcrit { inner, trace: trace.clone(), accepte });
    let session = Session::new(
        source,
        crate::transport::fixtures::local_ip(),
        Instant::now(),
        12_000_000,
    )
    .expect("session");
    (session, trace)
}

/// Vide le drapeau par le chemin du produit, en enregistrant les touches dans
/// la MÊME trace que les écritures.
fn injecter(session: &mut Session, trace: &Trace) {
    let t = trace.clone();
    let mut on_input = move |message: InputMessage| {
        t.lock().unwrap().push(format!("{message:?}"));
    };
    session.injecter_le_collage(&mut on_input);
}

/// 🔴 **L'ORDRE DE D6, OBSERVÉ SUR UNE SEULE TRACE.** L'écriture précède les
/// quatre touches — et le presse-papier reçoit le texte **DÉNORMALISÉ**,
/// puisque c'est Windows qui le lira.
///
/// ROUGE si l'injection précédait l'écriture, ou si le texte partait sans
/// dénormalisation.
#[test]
fn l_ecriture_precede_l_injection_et_le_texte_part_denormalise() {
    let (mut session, trace) = session_qui_ecrit(true);
    session.traiter_le_collage("une\ndeux");
    injecter(&mut session, &trace);

    let vue = trace.lock().unwrap().clone();
    assert_eq!(vue.len(), 5, "une écriture puis quatre touches : {vue:?}");
    assert_eq!(vue[0], "ecrire:une\r\ndeux", "le texte doit partir en \\r\\n");
    assert!(vue[1].starts_with("Key"), "les touches suivent l'écriture : {vue:?}");
}

/// 🔴 **EXACTEMENT QUATRE TOUCHES, DANS L'ORDRE Ctrl↓ V↓ V↑ Ctrl↑.**
///
/// ROUGE si l'on omet le `Ctrl`↑ : l'application resterait avec un
/// modificateur ENFONCÉ, et **toute frappe suivante deviendrait un
/// raccourci**. C'est le défaut le plus insidieux de ce chemin.
#[test]
fn un_collage_injecte_exactement_les_quatre_touches_dans_l_ordre() {
    let (mut session, trace) = session_qui_ecrit(true);
    session.traiter_le_collage("x");
    trace.lock().unwrap().clear();
    injecter(&mut session, &trace);

    let vue = trace.lock().unwrap().clone();
    assert_eq!(
        vue,
        vec![
            format!("{:?}", InputMessage::Key { scancode: 0x1d, pressed: true, extended: false }),
            format!("{:?}", InputMessage::Key { scancode: 0x2f, pressed: true, extended: false }),
            format!("{:?}", InputMessage::Key { scancode: 0x2f, pressed: false, extended: false }),
            format!("{:?}", InputMessage::Key { scancode: 0x1d, pressed: false, extended: false }),
        ]
    );
}

/// 🔴 **UNE ÉCRITURE QUI ÉCHOUE N'ARME PAS L'INJECTION, ET C'EST LE TEST DE
/// L'ORDRE.** Le drapeau n'est posé que dans le bras `Ok` : une mutation qui
/// l'armerait inconditionnellement — ou AVANT le `match` — fait rougir ce
/// test-ci et lui seul.
///
/// Sans lui, `Ctrl+V` partirait sur un presse-papier inchangé et collerait le
/// contenu **PRÉCÉDENT** : le mode de défaillance silencieux que D6 existe
/// entièrement pour éviter, et le seul qui donne à l'utilisateur un résultat
/// FAUX plutôt qu'absent.
#[test]
fn une_ecriture_refusee_n_injecte_aucune_touche() {
    let (mut session, trace) = session_qui_ecrit(false);
    session.traiter_le_collage("x");
    assert_eq!(trace.lock().unwrap().len(), 1, "l'écriture a bien été tentée");
    trace.lock().unwrap().clear();

    injecter(&mut session, &trace);
    assert!(
        trace.lock().unwrap().is_empty(),
        "la touche V est PERDUE, pas reportée"
    );
}

/// Le drapeau se **consomme** : un second tour de boucle n'injecte rien.
///
/// ROUGE si le booléen n'était jamais remis à faux — `Ctrl+V` partirait à
/// chaque tour, c'est-à-dire à la cadence vidéo.
#[test]
fn le_drapeau_se_consomme() {
    let (mut session, trace) = session_qui_ecrit(true);
    session.traiter_le_collage("x");
    injecter(&mut session, &trace);
    trace.lock().unwrap().clear();

    injecter(&mut session, &trace);
    assert!(trace.lock().unwrap().is_empty(), "un second tour n'injecte rien");
}

/// Sans collage, aucun tour de boucle n'injecte quoi que ce soit.
///
/// ROUGE si `injecter_le_collage` frappait sans regarder le drapeau.
#[test]
fn sans_collage_aucune_touche_n_est_injectee() {
    let (mut session, trace) = session_qui_ecrit(true);
    injecter(&mut session, &trace);
    assert!(trace.lock().unwrap().is_empty());
}

/// 🔴 **AU-DESSUS DE LA BORNE : RIEN N'EST ÉCRIT, ET RIEN N'EST INJECTÉ.**
///
/// ROUGE si la borne n'était pas appliquée sur ce chemin : 64 Kio + 1 partirait
/// vers le tube capteur↔enfant, et la VM collerait un texte que le client avait
/// pourtant refusé d'émettre.
#[test]
fn un_texte_au_dessus_de_la_borne_n_est_ni_ecrit_ni_injecte() {
    let (mut session, trace) = session_qui_ecrit(true);
    let trop = "a".repeat(crate::presse_papier::PRESSE_PAPIER_MAX + 1);
    session.traiter_le_collage(&trop);

    assert!(
        trace.lock().unwrap().is_empty(),
        "aucune écriture ne doit être tentée"
    );
    injecter(&mut session, &trace);
    assert!(trace.lock().unwrap().is_empty(), "aucune touche non plus");
}

/// 🔴 **LA BORNE PORTE SUR LE TEXTE NORMALISÉ, JAMAIS SUR LE DÉNORMALISÉ**, et
/// ce test le mesure là où l'ordre des trois opérations pourrait sembler
/// indifférent.
///
/// Un texte de `PRESSE_PAPIER_MAX` octets fait entièrement de sauts de ligne
/// DOUBLE de taille à la dénormalisation. Borner après elle le refuserait —
/// alors que c'est exactement ce que la VM aurait pu émettre dans l'autre
/// sens, où la borne porte aussi sur la forme normalisée (D-P1-2). Un
/// aller-retour deviendrait impossible.
///
/// ROUGE si l'ordre est `denormaliser` puis `borner_entrant`.
#[test]
fn la_borne_porte_sur_la_forme_normalisee_pour_que_l_aller_retour_tienne() {
    let (mut session, trace) = session_qui_ecrit(true);
    let sauts = "\n".repeat(crate::presse_papier::PRESSE_PAPIER_MAX);
    session.traiter_le_collage(&sauts);

    let vue = trace.lock().unwrap().clone();
    assert_eq!(vue.len(), 1, "le texte devait être écrit : {}", vue.len());
    assert_eq!(
        vue[0].len() - "ecrire:".len(),
        crate::presse_papier::PRESSE_PAPIER_MAX * 2,
        "la dénormalisation double bien la taille, et n'est PAS bornée"
    );
}
