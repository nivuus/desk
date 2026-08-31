//! Contrôle de placement : remet une fenêtre sur sa sortie DXGI si elle en
//! est partie.
//!
//! Extrait de `boucle.rs` (tâche 7 du sous-bloc D3) pour rester sous le
//! plafond de 500 lignes du projet — pas pour une raison de conception : ces
//! deux fonctions font partie de la boucle comme les autres, dans le même
//! module logique, juste dans un fichier voisin. Même schéma que
//! `superviseur/table/attribution.rs`.

use super::*;
use crate::geometry::Rect;

/// La borne d'une sortie : sa ZONE DE TRAVAIL quand Windows la donne, son
/// rectangle sinon.
///
/// 🔴 **C'EST LA MOITIÉ SUPERVISEUR DE L'ACCORD ENTRE LES DEUX PROCESSUS.** Le
/// capteur interroge le même moniteur par SA FENÊTRE
/// (`window::zones_du_moniteur_de`), le superviseur par l'ORIGINE de la sortie
/// — il connaît le rectangle DXGI avant même d'avoir posé la fenêtre. Même
/// `HMONITOR`, donc même borne, donc même `taille_pour_viewport` des deux
/// côtés : aucun message à échanger, et aucune bataille à 1 Hz.
///
/// ⚠️ **Le superviseur ne borne PAS par la texture de la duplication**, à la
/// différence du capteur : il n'ouvre aucune duplication, et `SortieDxgi.rect`
/// est déjà en coordonnées de bureau comme `rcWork`. Sur cette machine la
/// texture est PLUS grande que le rectangle (1860 contre 1428, lot 32T) : le
/// `min` du capteur est donc inerte ici, et les deux bornes coïncident. **Si
/// elles divergeaient**, le superviseur l'emporterait au tour suivant — un
/// désaccord borné à une seconde, jamais une oscillation.
pub(super) fn borne_de(sortie: &SortieDxgi) -> (u32, u32) {
    let moniteur = (sortie.rect.width, sortie.rect.height);
    match crate::window::zones_du_moniteur_au_point(sortie.rect.x, sortie.rect.y) {
        Ok((_, travail)) => crate::windows_source_sortie::borne_de_la_sortie(
            moniteur,
            Some((travail.width, travail.height)),
        ),
        Err(erreur) => {
            tracing::warn!(%erreur, nom = %sortie.nom_sortie, "zone de travail illisible : la sortie entière sert de borne");
            moniteur
        }
    }
}

/// Fait suivre au placement le viewport que le navigateur vient d'annoncer,
/// sur une session DÉJÀ vivante.
///
/// 🔴 **CE QUE CETTE FONCTION FAIT, ET CE QU'ELLE NE FAIT PAS.** Elle corrige
/// la taille RETENUE dans la table et repose la fenêtre — donc la moitié
/// SUPERVISEUR du remède. Elle ne touche ni au recadrage ni à l'encodeur, qui
/// vivent dans le capteur : c'est le `Resize` du canal de contrôle qui les
/// fait suivre (`WindowsSource::suivre_le_viewport`), depuis la même mesure du
/// même `ResizeObserver`, par la même règle pure.
///
/// ⚠️ **Si le `Resize` se perdait et que seul le `viewport` arrivait**, la
/// fenêtre changerait de taille sans que le recadrage suive : l'image
/// montrerait du bureau, jusqu'au `Resize` suivant. Ce n'est pas rattrapé ici,
/// et c'est dit plutôt que supposé impossible.
pub(super) fn suivre_le_viewport(
    table: &mut Table,
    session: &IdSession,
    largeur: u32,
    hauteur: u32,
) {
    let Some(nom) = table.nom_sortie_de(session).map(str::to_owned) else {
        return;
    };
    let toutes = enumerer_sorties_silencieux().unwrap_or_default();
    let Some(sortie) = toutes.iter().find(|s| s.nom_sortie == nom).cloned() else {
        // La sortie a disparu de la topologie entre l'annonce et ce tour : ne
        // rien écrire vaut mieux qu'écrire une taille calculée sur rien.
        return;
    };
    let retenue =
        crate::windows_source_sortie::taille_pour_viewport((largeur, hauteur), borne_de(&sortie));
    // Court-circuit : le `ResizeObserver` du client émet toutes les 200 ms
    // pendant qu'on tire un bord, et chaque passage relirait sinon la
    // topologie DXGI puis reposerait la fenêtre.
    if table.taille_sortie_de(session) == Some(retenue) {
        return;
    }
    tracing::info!(
        session = %session.0, nom_sortie = %nom,
        demande = format!("{largeur}x{hauteur}"),
        retenue = format!("{}x{}", retenue.0, retenue.1),
        "viewport suivi : la taille retenue change, la sortie ne bouge pas"
    );
    table.rafraichir_taille_sortie(session, retenue);
    replacer_si_besoin(table, session, &toutes);
}

/// Remet sur sa sortie toute fenêtre qui en est partie.
///
/// **`&Table`, et non `&mut Table`.** Jusqu'au sous-bloc D10, cette fonction
/// rafraîchissait aussi `taille_sortie` depuis la taille DXGI brute de la
/// sortie (héritage d'IMPORTANT 5, revue de la tâche 9 de D8, qui tenait ce
/// champ à jour d'un changement de mode fait hors de cette table par
/// `WindowsSource::changer_mode_de_sortie`). Ce chemin a été retiré au
/// sous-bloc D9 ; le rafraîchissement, lui, avait survécu par précaution,
/// alors qu'il ne pouvait déjà plus rien faire dériver.
///
/// **D10 le rend carrément FAUX, et c'est pourquoi il a disparu plutôt que
/// d'être conservé.** Depuis `sortie_pour_viewport` (une sortie peut être
/// bien plus grande que le viewport, registre pollué oblige),
/// `Table::taille_sortie_de` porte la taille RETENUE — celle à laquelle la
/// fenêtre est posée et que la capture recadre —, qui n'a plus aucune raison
/// d'égaler `GetDesc`/`DesktopCoordinates` de la sortie DXGI. Rafraîchir
/// depuis cette dernière aurait donc écrasé la taille retenue par la taille
/// PLEINE de la sortie à chaque tour — reposant la fenêtre en grand une
/// seconde après que `creer_sortie` l'a posée à sa taille recadrée. La table
/// est désormais la seule source de vérité de cette taille, posée une fois à
/// la création (tâche 6) et à la réutilisation (tâche 7) : ce contrôle
/// périodique la relit, il ne la recalcule plus.
pub(super) fn controler_le_placement(table: &Table) {
    let toutes = enumerer_sorties_silencieux().unwrap_or_default();
    for session in table.sessions_vivantes() {
        replacer_si_besoin(table, &session, &toutes);
    }
}

/// Remet une fenêtre sur sa sortie si elle en est partie.
///
/// Appelée par le contrôle périodique, **et par le bras `LancerEnfant`** : sur
/// le chemin de réutilisation d'une sortie retenue (§7.1 du sous-bloc D3),
/// `creer_sortie` n'est pas appelée, donc `placement::poser` non plus. Entre
/// la mort de l'enfant et sa relance, l'application a pu déplacer ou retailler
/// sa fenêtre ; sans cet appel, l'enfant capturerait une fenêtre mal posée
/// jusqu'au prochain contrôle périodique — jusqu'à `PERIODE_PLACEMENT` plus
/// tard.
///
/// Idempotente : `doit_etre_replacee` garde l'appel, donc le chemin de
/// création — où la fenêtre vient d'être posée — n'émet **normalement** aucun
/// second `SetWindowPos`. « Normalement » et non « jamais » : si Windows a
/// clampé la taille demandée (taille minimale de la fenêtre, contrainte du DPI),
/// le rectangle obtenu diffère de la cible, `doit_etre_replacee` est vrai, et un
/// second `SetWindowPos` **est** émis — sans plus d'effet que le premier.
///
/// **La position vient de la sortie DXGI, la taille de la table** (sous-bloc
/// D10) : `sortie.rect` donne l'origine dans le bureau virtuel, mais
/// `Table::taille_sortie_de` donne la taille RETENUE — celle, éventuellement
/// bien plus petite que la sortie, à laquelle la fenêtre a été posée et que la
/// capture recadre. Le **TROISIÈME** `let Some` — celui de
/// `Table::taille_sortie_de` — ne peut, en pratique, jamais échouer une fois
/// le PREMIER passé (`nom_sortie_de`) : `sortie_creee` pose `nom_sortie` et
/// `taille_sortie` ensemble, jamais l'un sans l'autre (`table.rs`) — ce n'est
/// donc pas `Etat::Vivante` qui gouverne ici, mais cet invariant-là. Gardé
/// tel quel plutôt que supposé, pour ne rien devoir à un fichier voisin.
///
/// ❌ **Cette phrase disait « le second `let Some` », et elle désignait le
/// TROISIÈME** (constat de la revue de la tâche 6, différé puis repris à la
/// revue finale de branche). ⚠️ **L'erreur n'était pas seulement de comptage** :
/// le vrai second — la recherche DXGI par nom, `toutes.iter().find(...)` —
/// **PEUT** échouer après le premier, une sortie pouvant avoir disparu de la
/// topologie entre deux tours. Un lecteur qui comptait les `let Some`
/// attribuait donc la clause « ne peut jamais échouer » au **mauvais garde**,
/// celui pour lequel elle est fausse.
pub(super) fn replacer_si_besoin(table: &Table, session: &IdSession, toutes: &[SortieDxgi]) {
    let Some(nom) = table.nom_sortie_de(session) else {
        return;
    };
    let Some(sortie) = toutes.iter().find(|s| s.nom_sortie == nom) else {
        return;
    };
    let Some((largeur, hauteur)) = table.taille_sortie_de(session) else {
        return;
    };
    let cible = Rect { x: sortie.rect.x, y: sortie.rect.y, width: largeur, height: hauteur };
    let Some(fenetre) = table.fenetre_de(session) else { return };
    let hwnd = windows::Win32::Foundation::HWND(fenetre.0 as *mut core::ffi::c_void);
    let Ok(actuel) = placement::rectangle_de(hwnd) else { return };
    if placement::doit_etre_replacee(&actuel, &cible) {
        // 🔴 `hwnd` ET `fenetre_vivante` : sans eux, l'hypothèse « le
        // handle de la table est PÉRIMÉ » est INDÉCIDABLE, et le lot 32O l'a
        // payé — vingt minutes après la rafale, deux `hwnd` relevés dans le
        // journal rendaient `IsWindow=false`, ce qui ne prouvait RIEN : ces
        // fenêtres avaient simplement fermé depuis.
        //
        // Le phénomène est **intermittent et lié à une session** : sans une
        // trace qui porte la réponse À L'INSTANT du replacement, il faudrait
        // épier le journal en direct pour espérer le mesurer. **Une panne
        // qu'on ne peut pas diagnostiquer coûte plus qu'une trace de plus.**
        //
        // ⚠️ `fenetre_vivante = false` **expliquerait D'UN COUP** les deux
        // faits ouverts : un `SetWindowPos` qui « réussit » sans rien
        // déplacer, et un rectangle de fenêtre minimisée lu sur un objet qui
        // n'existe plus. `true` les laisserait tous deux entiers.
        let fenetre_vivante =
            unsafe { windows::Win32::UI::WindowsAndMessaging::IsWindow(Some(hwnd)) }.as_bool();
        tracing::info!(
            session = %session.0,
            hwnd = format!("{:#x}", fenetre.0),
            fenetre_vivante,
            de = format!("{}x{}+{}+{}", actuel.width, actuel.height, actuel.x, actuel.y),
            vers = format!("{}x{}+{}+{}", cible.width, cible.height, cible.x, cible.y),
            "fenêtre sortie de sa sortie, replacement"
        );
        if let Err(erreur) = placement::poser(hwnd, &cible) {
            tracing::warn!(session = %session.0, %erreur, "replacement échoué");
        }
    }
}
