//! Les deux appels Win32 du presse-papier, et **rien d'autre**.
//!
//! Ce module ne décide rien : il lit le numéro de séquence et il lit le
//! texte. Toute la décision — normaliser, borner, refuser, comparer au
//! dernier émis — vit dans le parent, qui est pur et se teste sur l'hôte.
//!
//! ⚠️ **Il n'ÉCRIT jamais le presse-papier.** Le sens navigateur → VM est le
//! sous-bloc P2. La seule écriture du dépôt à ce jour est celle de la sonde
//! `diagnostics/presse_papier.rs`, qui est un geste de mesure et le déclare.

use anyhow::{Context, Result};
use windows::Win32::Foundation::HGLOBAL;
use windows::Win32::System::DataExchange::{
    CloseClipboard, GetClipboardData, GetClipboardSequenceNumber, OpenClipboard,
};
use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::CF_UNICODETEXT;

/// Le compteur de séquence du presse-papier de la station de fenêtres.
///
/// ⚠️ **Zéro a DEUX causes**, et le sondeur n'a pas à les départager : ou bien
/// l'appel a échoué (pas d'accès `WINSTA_ACCESSCLIPBOARD`), ou bien rien n'a
/// jamais été copié depuis le démarrage de la station. Mesuré : une VM
/// fraîchement démarrée rend `0, 0, 0`, puis **53** cinq copies plus tard
/// (sonde P0, exécution n°1 du 20 août 2026, journal versé). Dans les deux
/// cas la conduite du `Sondeur` est la bonne — il prend `0` pour référence au
/// premier tour et n'annonce rien tant que le compteur ne bouge pas.
pub fn numero_de_sequence() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
}

/// Ouvre le presse-papier, lit `CF_UNICODETEXT`, referme.
///
/// - `Ok(None)` : le presse-papier ne porte pas de texte Unicode (une image,
///   par exemple). Ce n'est pas une erreur.
/// - `Err` : l'ouverture a été refusée — **cas NORMAL sous Windows**, une
///   autre application tient le presse-papier, et non une panne. L'appelant
///   n'avance alors pas sa référence et retentera au tour suivant.
///
/// `String::from_utf16_lossy` et non une conversion faillible : un substitut
/// isolé est possible (`CF_UNICODETEXT` n'est pas validé par Windows) et ne
/// doit ni faire échouer la lecture ni tuer un fil.
pub fn lire_texte() -> Result<Option<String>> {
    unsafe { OpenClipboard(None) }.context("OpenClipboard")?;
    // 🔴 LE GARDE EST CONSTRUIT IMMÉDIATEMENT APRÈS L'OUVERTURE, et rien ne
    // s'intercale : à partir d'ici, TOUS les chemins de sortie referment.
    let _garde = PressePapierOuvert;
    unsafe {
        let poignee = match GetClipboardData(CF_UNICODETEXT.0 as u32) {
            Ok(poignee) if !poignee.is_invalid() => poignee,
            // Pas de texte Unicode : une image, des fichiers. `Ok(None)`, pas
            // une erreur — le sondeur n'a rien à annoncer et n'avance pas sa
            // référence.
            _ => return Ok(None),
        };
        let global = HGLOBAL(poignee.0);
        let pointeur = GlobalLock(global) as *const u16;
        if pointeur.is_null() {
            anyhow::bail!("GlobalLock a rendu un pointeur nul");
        }
        let mut longueur = 0usize;
        while *pointeur.add(longueur) != 0 {
            longueur += 1;
        }
        // 🔴 LA DONNÉE EST COPIÉE AVANT TOUTE FERMETURE : le handle appartient
        // au presse-papier et n'est plus valide après `CloseClipboard`.
        let texte = String::from_utf16_lossy(std::slice::from_raw_parts(pointeur, longueur));
        let _ = GlobalUnlock(global);
        Ok(Some(texte))
    }
}

/// Le garde RAII qui referme le presse-papier — **sur TOUS les chemins de
/// sortie**, y compris un `?`, un `return` anticipé et une PANIQUE en cours de
/// déroulement de pile.
///
/// **Ce n'est pas une élégance, c'est la seule forme correcte ici.** Un
/// presse-papier laissé ouvert bloque **toute la window station**, pas
/// seulement l'agent : plus aucune application de la session Windows ne peut
/// copier ni coller tant que ce processus vit. La forme précédente — une
/// fermeture unique placée après un bloc de travail — couvrait bien le `?` et
/// le `return`, parce que le bloc rendait un `Result` au lieu de sortir de la
/// fonction ; elle ne couvrait **pas** la panique, qui déroule la pile sans
/// jamais atteindre la ligne de fermeture. Le plan (tâche 12, étape 2)
/// exigeait explicitement les trois, et le garde est ce qui les donne d'un
/// seul coup.
///
/// Le retour de `CloseClipboard` est délibérément ignoré : il n'existe aucune
/// conduite de rattrapage — ou le presse-papier était ouvert et il est fermé,
/// ou il ne l'était pas et il n'y avait rien à faire — et un `Drop` ne peut de
/// toute façon rien remonter à l'appelant.
struct PressePapierOuvert;

impl Drop for PressePapierOuvert {
    fn drop(&mut self) {
        let _ = unsafe { CloseClipboard() };
    }
}
