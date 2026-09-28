//! `Sondeur` — observing the VM's clipboard, and deciding what
//! to announce.
//!
//! **Extracted VERBATIM from `presse_papier.rs` in sub-block P3, BEFORE the addition
//! that made it necessary** (D-P3-6's second take, task 5). The parent
//! was at 441 lines for a ceiling of 500, and documenting a guard
//! costs thirty lines there for a six-line function — `apres_notre_ecriture`
//! carries twenty-nine. The repository's rule is to extract BEFORE adding, never
//! to compress after: D9 paid two compressions for forgetting it.
//!
//! ⚠️ **This module is declared by an ORDINARY `mod sondeur;` in its parent,
//! and `CLAUDE.md`'s "Child module convention" does not apply**:
//! it only targets modules extracted from a `#[cfg(windows)]` parent so
//! that their pure logic compiles on the host, and `presse_papier.rs` is not
//! gated. Nothing is hoisted to the crate root.
//!
//! **Pure, without any `cfg`** — except for `lire_la_plateforme` alone, which is
//! the platform switch and decides nothing.

use std::time::Instant;

#[cfg(windows)]
use super::win32;
use super::{actif, gardes_armes, normaliser, Annonce, PERIODE_PRESSE_PAPIER, PRESSE_PAPIER_MAX};

/// Observes the clipboard and decides what to announce.
///
/// It holds **no** Windows resource: it is the caller that gives it
/// the sequence number and the read closure.
#[derive(Debug, Default)]
pub struct Sondeur {
    /// The last sequence number for which a read **succeeded**.
    ///
    /// `None` at the first turn: the state read then serves as **reference** and is
    /// **not announced** — it is the pattern of `SuiviBordure`
    /// (`capteur/plein_ecran.rs`).
    ///
    /// ❌ **THIS DOC ADDED "a window that attaches therefore does not receive
    /// the content already present; it receives the first copy THAT FOLLOWS", AND
    /// SUB-BLOCK P3 REFUTED IT.** It was P1's legacy no. 3, and it is
    /// closed: the registry memorises the last announcement
    /// (`capteur/sommeil/registre.rs::Etat::dernier_presse_papier`) and
    /// emits it at registration on the new channel alone.
    ///
    /// ⚠️ **What stays TRUE is the property of THIS field**, and it is
    /// unchanged: the `Sondeur` still announces nothing at its first turn.
    /// What changed is elsewhere — it is the REGISTRY that replays, not it.
    reference: Option<u32>,
    /// The last content actually announced — **D5's guard no. 2**.
    dernier_emis: Option<String>,
    /// The last refused size, so as not to repeat the refusal.
    dernier_refus: Option<u32>,
    /// When `tour()` last read the counter.
    dernier_tour: Option<Instant>,
}

impl Sondeur {
    pub fn nouveau() -> Self {
        Self::default()
    }

    /// **The core, and it does not touch Windows.**
    ///
    /// `seq` is the sequence number read by the caller; `lire` is
    /// called **only** if that number moved. It is what makes the property
    /// "we do not open the clipboard for nothing" exercisable on the
    /// host, without a single `cfg` — opening is a contended resource
    /// under Windows, and opening it at each turn would starve applications.
    ///
    /// `lire` returns `None` when the read **failed** (another application
    /// holds the clipboard — a NORMAL case under Windows, not a failure) or
    /// when the clipboard carries no text. **The reference does NOT advance
    /// then**: otherwise the corresponding content would be lost forever,
    /// the next turn seeing an "unchanged" counter and retrying nothing.
    pub fn observer(&mut self, seq: u32, lire: impl FnOnce() -> Option<String>) -> Option<Annonce> {
        if self.reference == Some(seq) {
            return None;
        }
        let brut = lire()?;
        // The reference only advances after a SUCCESSFUL read (D-P1-5).
        let premier_tour = self.reference.is_none();
        self.reference = Some(seq);
        if premier_tour {
            // The state read at attach time serves as reference: we announce nothing.
            self.dernier_emis = Some(normaliser(&brut));
            return None;
        }
        // Normalise first, bound after (D-P1-2): the bound bears on what
        // we EMIT, never on what we read. A Windows text of 65,000
        // lines loses 65,000 bytes at normalisation; bounding first
        // would refuse a text that, once normalised, would fit.
        let texte = normaliser(&brut);
        let octets = texte.len();
        if octets > PRESSE_PAPIER_MAX {
            let octets = octets as u32;
            // A refusal repeated identically is only announced once: otherwise
            // a huge content left in the clipboard would make the banner
            // flicker at each neighbouring copy.
            if self.dernier_refus == Some(octets) {
                return None;
            }
            self.dernier_refus = Some(octets);
            return Some(Annonce::Refus { octets });
        }
        self.dernier_refus = None;
        // D5's guard no. 2: the counter moves on an identical rewrite
        // (measured, probe P0). Without this comparison, such a gesture would push
        // a message for nothing.
        if self.dernier_emis.as_deref() == Some(texte.as_str()) {
            return None;
        }
        self.dernier_emis = Some(texte.clone());
        Some(Annonce::Texte(texte))
    }

    /// Arms D5's guards no. 1 and no. 2 **on OUR OWN write**.
    ///
    /// To be called right after writing the Windows clipboard ourselves
    /// (browser → VM direction, sub-block P2). `seq` is the sequence number
    /// reread **AFTER `CloseClipboard`** — rereading it before would return a counter
    /// that closing can still move, and guard no. 1 would be off
    /// by one, that is, silently inoperative.
    ///
    /// **Both** fields are set, and each is a distinct guard:
    ///
    /// - `reference` **is guard no. 1**: at the next turn, `observer` exits
    ///   on its first line and **does not even reopen** the clipboard;
    /// - `dernier_emis` **is guard no. 2 armed on our write**: it
    ///   catches the case where a THIRD-PARTY write slipped in between
    ///   our `SetClipboardData` and this reread of the counter. The reread
    ///   number is then no longer the current one, guard no. 1 does not bite, and
    ///   it is the content comparison that prevents the round trip.
    ///
    /// The text is **normalised** before being memorised, as is the one
    /// `observer` reads: otherwise guard no. 2 would compare a text with `\r\n`
    /// (what Windows will give back to us, since it is `denormaliser` that puts them there)
    /// to a text with `\n`, and would never recognise our own write.
    ///
    /// ⚠️ **What this method CANNOT do**, and it must be said: if
    /// another copy occurs between our write and the wheel turn that
    /// consumes this pair, setting `reference` on *our* `seq` does not mask it
    /// — the counter will have moved again, and the third-party copy will be announced.
    /// **It is the intended behaviour**: the guard stays exact in D5's sense, and
    /// a test checks it.
    ///
    /// ❌ **THIS CAVEAT WAS INCOMPLETE, AND SUB-BLOCK P3 MEASURED IT.**
    /// It only deals with the **THIRD-PARTY** copy, which it declares intended. The case
    /// of **OUR OWN SECOND WRITE** — a second window pasting
    /// after this arming and before the turn — was declared NOWHERE, and it
    /// gets past BOTH guards: no. 1 because the counter moved again, no.
    /// 2 because the memorised text is that of the PREVIOUS paste. A test
    /// saw it RED on the intact tree, without any mutation. The remedy is
    /// `ecarter_notre_ecriture`, further down in this same file.
    pub fn apres_notre_ecriture(&mut self, seq: u32, texte: &str) {
        self.armer(gardes_armes(), seq, texte);
    }

    /// The core of `apres_notre_ecriture`, with the guard state **injected**.
    ///
    /// 🔴 **It is what makes the disarmed arm EXERCISABLE ON THE HOST.**
    /// `gardes_armes()` is a `OnceLock`: a test can neither drive nor
    /// reset it, and a check written against it could therefore **not
    /// return the other value** — that is, not fail. Same pattern as
    /// `observer`'s read closure, and for the same reason.
    pub fn armer(&mut self, armes: bool, seq: u32, texte: &str) {
        // The disarmed arm of criterion ④: see `gardes_armes`, which says why
        // it disarms BOTH and not no. 1 alone.
        if !armes {
            return;
        }
        self.reference = Some(seq);
        self.dernier_emis = Some(normaliser(texte));
    }

    /// Discards the announcement OUR OWN write has just produced, and arms
    /// the guards on it. **The SECOND TAKE of D-P3-6.**
    ///
    /// 🔴 **THE RACE THIS METHOD CLOSES, AND IT WAS MEASURED BEFORE
    /// BEING CLOSED** (sub-block P3, red on the intact tree, without any
    /// mutation — `journaux-presse-papier-p3/rouge-t5-d-p3-6-arbre-intact.log`).
    /// The interleaving, with two windows:
    ///
    /// 1. window A pastes → the write sets `notre_ecriture = (seqA, textA)`;
    /// 2. the wheel turn calls `armer_les_gardes`: it TAKES this pair and
    ///    arms `reference = seqA`, `dernier_emis = textA`;
    /// 3. window B pastes → `notre_ecriture = (seqB, textB)`, and the
    ///    Windows clipboard now carries `textB`;
    /// 4. `tour()` reads `seqB ≠ seqA` — guard no. 1 does not bite — then reads
    ///    `textB ≠ textA` — guard no. 2 does not bite either — and
    ///    **`Annonce::Texte(textB)` goes out to the N windows**;
    /// 5. at the next turn, `armer_les_gardes` takes `(seqB, textB)`: too late.
    ///
    /// It is exactly the per-paste round trip D5's guards
    /// exist to remove, and `apres_notre_ecriture` does not cover it:
    /// its written caveat deals with the case of an interleaved **THIRD-PARTY** copy,
    /// which it declares intended. The case above is **our own second
    /// write**, and it was declared nowhere.
    ///
    /// ⚠️ **THIS REMEDY NARROWS THE WINDOW, IT DOES NOT CLOSE IT.**
    /// `capteur/sommeil/presse_papier::ecrire_avec` writes the clipboard
    /// **THEN** sets `notre_ecriture` — the lock there is deliberately taken
    /// AFTER the Win32 I/O, because holding it around `OpenClipboard`
    /// would block the attaching and removal of ALL windows. If `tour()`
    /// reads the text in that short interval, the second take will find
    /// nothing. The residue is of the order of a mutex acquisition, and it is of the
    /// **same kind** as the one `apres_notre_ecriture` already declares accepted.
    ///
    /// ⚠️ **The filter bears on the TEXT, never on `seq` alone**, and it is
    /// a safeguard, not a detail: a THIRD-PARTY copy occurring after our
    /// write also carries a later `seq`, and filtering on the number
    /// would silence a real copy. A test holds it.
    ///
    /// ⚠️ **An `Annonce::Refus` is never discarded**: it carries no
    /// text to compare, and refusing it would deprive the user of the
    /// banner that tells them why nothing arrived.
    pub fn ecarter_notre_ecriture(
        &mut self,
        notre: Option<(u32, String)>,
        annonce: Option<Annonce>,
    ) -> Option<Annonce> {
        self.ecarter(gardes_armes(), notre, annonce)
    }

    /// The core of `ecarter_notre_ecriture`, with the guard state **injected** —
    /// same pattern, and for the same reason, as `armer` relative to
    /// `apres_notre_ecriture`: `gardes_armes()` is a `OnceLock` a test
    /// can neither drive nor reset, and a check written against it could
    /// therefore not return the other value, that is, not fail.
    ///
    /// 🔵 **`PRESSE_PAPIER_GARDE=0` ALSO disarms this take**, and it must:
    /// this bench variable exists to make reachable the red of
    /// P2's criterion ④, which counts the messages coming back to the window after
    /// a paste. A second take that discarded anyway would empty this
    /// arm of its meaning.
    pub fn ecarter(
        &mut self,
        armes: bool,
        notre: Option<(u32, String)>,
        annonce: Option<Annonce>,
    ) -> Option<Annonce> {
        let Some((seq, texte)) = notre else {
            return annonce;
        };
        if !armes {
            return annonce;
        }
        let notre_texte = normaliser(&texte);
        self.armer(armes, seq, &texte);
        match annonce {
            Some(Annonce::Texte(t)) if t == notre_texte => None,
            autre => autre,
        }
    }

    /// A complete polling turn, **outside any lock**.
    ///
    /// Returns `None` without reading anything as long as `PERIODE_PRESSE_PAPIER` has not
    /// elapsed, even if the caller comes more often.
    pub fn tour(&mut self) -> Option<Annonce> {
        let maintenant = Instant::now();
        if let Some(dernier) = self.dernier_tour {
            if maintenant.duration_since(dernier) < PERIODE_PRESSE_PAPIER {
                return None;
            }
        }
        self.dernier_tour = Some(maintenant);
        if !actif() {
            return None;
        }
        self.lire_la_plateforme()
    }

    #[cfg(windows)]
    fn lire_la_plateforme(&mut self) -> Option<Annonce> {
        let seq = win32::numero_de_sequence();
        self.observer(seq, || win32::lire_texte().ok().flatten())
    }

    /// Non-Windows fallback: there is no system clipboard to observe.
    ///
    /// **This stub is mandatory, not decorative**: the caller
    /// (`capteur/sommeil/registre.rs`) is not gated and must compile on
    /// the Linux host — unlike `plein_ecran::lire_style`, whose
    /// only caller is itself `#[cfg(windows)]`.
    #[cfg(not(windows))]
    fn lire_la_plateforme(&mut self) -> Option<Annonce> {
        None
    }
}
