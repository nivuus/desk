//! Les appels Win32 du presse-papier, et **rien d'autre**.
//!
//! Ce module ne décide rien : il lit le numéro de séquence, il lit le texte,
//! il écrit le texte. Toute la décision — normaliser, dénormaliser, borner,
//! refuser, comparer au dernier émis, armer les gardes — vit dans le parent,
//! qui est pur et se teste sur l'hôte.
//!
//! ❌ **Ce module disait « il n'ÉCRIT jamais le presse-papier ; le sens
//! navigateur → VM est le sous-bloc P2 ». Ce sous-bloc a eu lieu**, et
//! `ecrire_texte` vit désormais ici. La sonde `diagnostics/presse_papier.rs`
//! garde son propre `mod win` privé, à dessein : elle mesure, ses phases C et
//! D écrivent le presse-papier de la VM pour l'éprouver, et le produit ne doit
//! pas hériter d'un chemin de banc.

use anyhow::{Context, Result};
use windows::Win32::Foundation::HGLOBAL;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber, OpenClipboard,
    SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
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

/// Écrit `texte` dans le presse-papier de la VM, et rend le numéro de séquence
/// relu **APRÈS** la fermeture.
///
/// **Aucune décision ici.** Le texte arrive déjà normalisé, borné et
/// dénormalisé (`\r\n`) par le parent : ce module se contente de l'écrire.
///
/// 🔴 **Le numéro est relu APRÈS `CloseClipboard`, et cet ordre est
/// PORTANT.** Le relire avant la fermeture rendrait un compteur que la
/// fermeture peut encore faire bouger — le garde n°1 de D5 serait alors faux
/// d'un cran, c'est-à-dire **silencieusement inopérant** : aucune panne,
/// seulement un aller-retour parasite par collage, que rien ne signalerait.
///
/// - `Err` sur refus d'ouverture : **cas NORMAL sous Windows** (une autre
///   application tient le presse-papier — risque R7 de la spec), et non une
///   panne. **On ne boucle JAMAIS en attente** : l'appelant journalise et
///   n'injecte pas, la touche `V` étant perdue plutôt que reportée (D6).
/// - `EmptyClipboard` **précède** `SetClipboardData`, faute de quoi les
///   formats de l'application précédente survivraient dans d'autres
///   `CF_*` et le collage deviendrait imprévisible : une application qui
///   préfère `CF_RTF` ou `CF_HTML` collerait l'ancien contenu.
pub fn ecrire_texte(texte: &str) -> Result<u32> {
    // UTF-16 terminé par un `\0` : `CF_UNICODETEXT` l'exige, et un bloc non
    // terminé ferait lire au-delà par toute application qui colle.
    let mut unites: Vec<u16> = texte.encode_utf16().collect();
    unites.push(0);

    unsafe { OpenClipboard(None) }.context("OpenClipboard")?;
    // 🔴 LE GARDE EST CONSTRUIT IMMÉDIATEMENT APRÈS L'OUVERTURE, comme dans
    // `lire_texte` : à partir d'ici tous les chemins de sortie referment, la
    // panique comprise. Un presse-papier laissé ouvert bloque TOUTE la window
    // station, pas seulement l'agent.
    let ecriture = (|| unsafe {
        let _garde = PressePapierOuvert;
        EmptyClipboard().context("EmptyClipboard")?;
        let octets = unites.len() * std::mem::size_of::<u16>();
        let global = GlobalAlloc(GMEM_MOVEABLE, octets).context("GlobalAlloc")?;
        let pointeur = GlobalLock(global) as *mut u16;
        if pointeur.is_null() {
            anyhow::bail!("GlobalLock a rendu un pointeur nul");
        }
        std::ptr::copy_nonoverlapping(unites.as_ptr(), pointeur, unites.len());
        let _ = GlobalUnlock(global);
        // 🔴 Le presse-papier PREND POSSESSION du bloc : ne pas le libérer.
        // `GlobalFree` ici rendrait le presse-papier de la station pointant sur
        // de la mémoire rendue au tas — un défaut à effet différé, et global à
        // la session Windows.
        SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(global.0)))
            .context("SetClipboardData")?;
        Ok(())
    })();
    // Le garde a couru à la sortie de la fermeture ci-dessus : le
    // presse-papier est refermé, et c'est seulement maintenant que le compteur
    // est stable.
    ecriture?;
    Ok(numero_de_sequence())
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
