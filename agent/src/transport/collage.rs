//! The paste coming from the browser: write the VM's clipboard, then
//! inject `Ctrl+V` — **in that order, and never the reverse**.
//!
//! **Extracted from `tick.rs` BEFORE branch `a1octies` fitted there**
//! (sub-block P2): that file was at 421 lines for a ceiling of 500, and
//! the branch's body is more than seventy — it carries all of
//! D6's reasoning. Inlining it would have taken it to 493, margin 7, on a
//! file that already counts eleven branches and sixty lines of invariant
//! audit. The repository's rule is to extract BEFORE adding.
//!
//! 🔴 **BOTH HALVES OF D6's ORDER LIVE HERE, and that is the point.**
//! `traiter_le_collage` writes and arms; `injecter_le_collage` consumes and
//! types. They are called from two different files —
//! `tick::act_on_timeout` for the first, `boucle::run` for the second —
//! because only `run` receives `on_input` (D-P2-1); reading them two files
//! apart would make the order invisible.

use proto::input::InputMessage;

use super::Session;

impl Session {
    /// Writes `texte` into the VM's clipboard, then **arms** the injection.
    ///
    /// Three steps, in this exact order and for this reason: write first —
    /// **SYNCHRONOUS**, `write_clipboard` waits for the sensor's `Fait` —,
    /// only arm AFTERWARDS, and only if the write **succeeded**. No
    /// channel ordering comes into it: the order is guaranteed by
    /// construction.
    ///
    /// 🔴 **On `Err`, we do NOT arm**, and that is the whole matter: a `Ctrl+V` on
    /// an unchanged clipboard would paste the **PREVIOUS** content, without
    /// anything telling the user. D6 literally prescribes the opposite —
    /// "if the clipboard cannot be written, the `V` key is LOST,
    /// not deferred".
    ///
    /// **The text is normalised, bounded, then denormalised, in THAT order** — the
    /// same as the outgoing direction (D-P1-2). Bounding first would refuse a text
    /// that, once `\r\n` are brought back to `\n`, would fit; bounding AFTER
    /// denormalisation would refuse a text the VM had just accepted in
    /// the other direction, a round trip having inflated it by one `\r` per line.
    ///
    /// **A refusal sends NO message back to the browser**: the client already has
    /// its own bound and its banner, and a second refusal for the same gesture
    /// would be noise. It is logged, never kept quiet.
    pub(super) fn traiter_le_collage(&mut self, texte: &str) {
        let normalise = crate::presse_papier::normaliser(texte);
        match crate::presse_papier::borner_entrant(&normalise) {
            None => tracing::warn!(
                session = %self.session_id,
                octets = normalise.len(),
                borne = crate::presse_papier::PRESSE_PAPIER_MAX,
                "collage refusé : au-dessus de la borne, il n'est ni tronqué ni écrit"
            ),
            Some(borne) => {
                let pour_windows = crate::presse_papier::denormaliser(&borne);
                match self.source.write_clipboard(&pour_windows) {
                    // ⚠️ **NEVER THE TEXT IN THE LOG** (D-P1-7): the
                    // clipboard content is a private resource, and
                    // a log committed to git is public to the repository. Only
                    // its SIZE is logged, and a single line — two
                    // traces at the same instant count as two
                    // events (D6's home-grown trap).
                    Ok(()) => {
                        tracing::debug!(
                            session = %self.session_id,
                            octets = borne.len(),
                            "collage écrit dans le presse-papier de la VM"
                        );
                        self.collage_a_injecter = true;
                    }
                    Err(error) => tracing::warn!(
                        session = %self.session_id,
                        error = %format!("{error:#}"),
                        "collage NON écrit : la touche V est perdue, pas reportée"
                    ),
                }
            }
        }
    }

    /// Injects the four keys of a paste if `traiter_le_collage` armed
    /// them. **Consumes the flag**: otherwise, `Ctrl+V` would go out at EVERY
    /// loop round, that is, at the video cadence.
    ///
    /// No `coller()` method is added to `InputInjector`: its
    /// `InputMessage::Key` arm **already** calls `au_premier_plan()`, which checks the
    /// return of `SetForegroundWindow` and logs it once per
    /// switch. Reusing `InputMessage::Key` inherits that for free;
    /// writing a second path would duplicate it.
    pub(super) fn injecter_le_collage(&mut self, on_input: &mut impl FnMut(InputMessage)) {
        if !self.collage_a_injecter {
            return;
        }
        self.collage_a_injecter = false;
        for touche in crate::input::TOUCHES_COLLAGE {
            on_input(touche);
        }
    }
}

#[cfg(test)]
#[path = "collage/tests.rs"]
mod tests;
