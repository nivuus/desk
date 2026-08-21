//! Le tour d'accent du fil de fenêtre — sous-bloc **A1**.
//!
//! **Extrait de `fenetre.rs` et non ajouté dedans**, exactement comme
//! `transitions.rs` l'a été au sous-bloc D5 : l'addition l'aurait porté à
//! **500 lignes exactement**, c'est-à-dire à marge NULLE, et la règle du dépôt
//! est « extraction, jamais compression ». ⚠️ **Le plan de A1 avait chiffré
//! cette addition à ~18 lignes ; elle en pèse 55**, et c'est la mesure qui l'a
//! dit, pas la relecture. `fenetre.rs` a déjà franchi 500 deux fois — 508 en
//! D9, 505 en D10 —, et il en a été sauvé par une extraction les deux fois.
//!
//! 🔴 **AUCUN TEST D'HÔTE NE COUVRE CE FICHIER** : il est `#[cfg(windows)]` par
//! son appelant et par `accent::win32`. Son seul contrôle est le **critère ④**
//! de la recette — exactement UNE ligne `accent de la fenetre Windows` par
//! `session` sur un palier de 60 s.
//!
//! ⚠️ **La lecture vit ICI, sur le FIL DE FENÊTRE, et non sur le tour de roue
//! du registre**, contrairement à ce que la décision D9 de la spécification
//! prescrivait. La raison est mécanique et non esthétique : **le tour de roue
//! n'a pas le `hwnd`** — aucun des quinze champs d'`Etat`
//! (`capteur/sommeil/registre.rs`) ne le porte, et `inscrire(session, pid)` ne
//! le prend pas. Le presse-papier y vit parce qu'il est **global à la window
//! station** ; l'accent est **par fenêtre**, ce qui est justement la propriété
//! que D9 revendique. Le patron est le plein écran de D8, resté dans
//! `fenetre.rs` juste au-dessus de l'appel à ce module.

use std::sync::mpsc::SyncSender;
use std::time::Instant;

use super::commandes::deposer;
use super::{AEcrire, Contexte, Fin};
use crate::accent;
use crate::capteur::protocole::DepuisCapteur;
use crate::windows_source::WindowsSource;

/// Un tour d'accent : lit l'icône si le minuteur est échu, et dépose une
/// annonce si — et seulement si — la teinte a CHANGÉ.
///
/// Rend `Some(Fin::Terminer(motif))` si le dépôt a fait tomber la connexion
/// média, `None` dans tous les autres cas — **y compris quand il n'y a rien à
/// annoncer, et y compris quand l'icône est illisible.**
///
/// 🔴 **`accent::actif()` EST TESTÉ EN PREMIER** : `ACCENT=0` doit empêcher
/// jusqu'au `SendMessageTimeout`, pas seulement l'envoi. C'est ce que
/// `Sondeur::tour` fait pour le presse-papier, et pour la même raison — une
/// variable qui désarme un mécanisme doit désarmer sa **LECTURE**, sans quoi
/// elle n'économise rien et ne prouve rien.
///
/// ⚠️ **`accent::PERIODE_ACCENT` est une constante PROPRE à ce mécanisme** : ne
/// pas la coupler à `plein_ecran::PERIODE_STYLE`, qui borne une lecture
/// différente pour une raison différente.
///
/// ⚠️ **Une icône illisible n'est PAS une erreur** : `lire_icone` rend `None`
/// sur un délai dépassé, un `HICON` nul ou un `GetDIBits` en échec, et
/// `dominante` rend `None` sur une icône entièrement grise ou vide. Les deux se
/// traitent pareil — le tour ne produit aucune annonce, et il ne journalise
/// rien non plus : une trace par tour serait douze lignes par minute et par
/// fenêtre pour dire qu'il ne se passe rien.
#[cfg(windows)]
pub(super) fn tour(
    suivi: &mut accent::SuiviAccent,
    dernier: &mut Instant,
    hwnd: windows::Win32::Foundation::HWND,
    ecritures: &SyncSender<AEcrire>,
    source: Option<&mut WindowsSource>,
    ctx: &Contexte,
) -> Option<Fin> {
    if !accent::actif() || dernier.elapsed() < accent::PERIODE_ACCENT {
        return None;
    }
    *dernier = Instant::now();

    let (rgba, largeur, hauteur) = accent::win32::lire_icone(hwnd)?;
    let rgb = accent::dominante(&rgba, largeur, hauteur)?;
    let couleur = suivi.observer(&accent::en_hexa(rgb))?;

    // ⚠️ La trace porte `session` PARCE QUE TOUS LES ENFANTS PARTAGENT
    // `agent.log` DEPUIS D4 : une trace sans ce champ y est un nombre dans un
    // multiensemble anonyme (consignation n°2 de D6, payée en pleine recette).
    // Et elle ne sort QU'AU CHANGEMENT : c'est sur elle, et sur elle seule, que
    // le critère ④ se compte.
    tracing::info!(session = %ctx.session, couleur = %couleur, "accent de la fenetre Windows");

    match deposer(AEcrire::Etat(DepuisCapteur::Accent { couleur }), ecritures, source, ctx) {
        Fin::Terminer(motif) => Some(Fin::Terminer(motif)),
        Fin::Continuer => None,
    }
}
