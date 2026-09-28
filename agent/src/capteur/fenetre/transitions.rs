//! The window thread's two transitions: releasing its source,
//! rebuilding it.
//!
//! **This module EXECUTES what `crate::capteur::sommeil` DECIDES.** It is
//! deliberately not named that: two `sommeil` modules in the same subtree would
//! be confused when reading, and the registry import from `fenetre.rs`
//! would collide with the child.
//!
//! **Extracted from `fenetre.rs` and not added into it**: sub-block D5 would have
//! taken the file beyond the project's 500-line cap. The repository has the
//! precedent (`vivier.rs` and `vivier/tests.rs`), and the rule imposing it is
//! "extraction, never compression".

use std::sync::mpsc::SyncSender;
use std::time::Instant;

use anyhow::{Context, Result};

use crate::capteur::protocole::DepuisCapteur;
use crate::capteur::sommeil::file::{ReceveurSession, VideOuFerme};
use crate::capteur::sommeil::Message;
use crate::capteur::vivier::Ordre;
use crate::source::VideoSource;
use crate::windows_source::WindowsSource;

use super::commandes::deposer;
use super::{AEcrire, Contexte, Fenetre, Fin};

impl Fenetre {
    /// Releases the encoder and the duplication. **On THIS thread**, never elsewhere:
    /// `Drop for H264Encoder` can freeze (risk observed, not attributed), and
    /// here it would only freeze this window.
    fn dormir(&mut self) {
        if self.source.take().is_some() {
            tracing::info!(
                session = %self.session,
                "fenêtre endormie, encodeur et duplication relâchés"
            );
        }
    }

    /// Builds the source from the kept parameters, then forces a
    /// key frame.
    ///
    /// **It is the ONLY place in the sensor that builds a `WindowsSource`**
    /// since sub-block D5: `Fenetre::ouvrir` no longer builds one, and a
    /// window is born asleep. This path therefore serves the first
    /// construction as well as all rebuilds — and that requires nothing
    /// special, `Parametres` carrying exactly what `ouvrir` knew.
    ///
    /// **Can fail, and it is the NOMINAL case** when the hardware cap
    /// of encoders is reached: the caller must then tell the pool (see
    /// `appliquer_les_ordres`).
    ///
    /// `sur_sortie` retries the duplication during `DUREE_FENETRE_OUVERTURE`:
    /// it is the resumption path of sub-block D2, and wake-up therefore takes it
    /// without having to relearn the same lesson — an output created by the
    /// supervisor at the same instant makes neighbouring duplications abandon
    /// the mutex.
    ///
    /// `duree_ms` is logged because the wake-up delay is one of the
    /// readings expected by the acceptance run, and there is no other place
    /// to take it on the agent side.
    fn reveiller(&mut self) -> Result<()> {
        if self.source.is_some() {
            return Ok(());
        }
        let debut = Instant::now();
        // `self.dimensions()`: the KEPT size, the one `ouvrir`
        // resolved — never that of the output, which may be larger
        // (polluted registry, D9 §9).
        //
        // ❌ **THIS PARAGRAPH SAID "`resize` NEVER updates it: it
        // is a no-op in `SortieEntiere` mode … a wake-up therefore always re-reads
        // the same value as the previous one", AND BATCH 33 MADE IT FALSE.**
        // `resize` now makes the crop and the window follow the
        // viewport (`ModeCapture::suit_le_viewport`), so `self.largeur` and
        // `self.hauteur` CAN change between two wake-ups. A window
        // resized then put to sleep would have woken up at its opening
        // size, erasing the resize without a trace — that is
        // why `boucler` now writes these two fields on state
        // change (see its point 3). **What `dimensions()` returns therefore remains
        // the freshest KEPT size**, which is exactly what
        // this wake-up needs; it is the REASON that changed, not the expected
        // value.
        let taille = self.dimensions();
        let p = &self.parametres;
        let mut source =
            WindowsSource::sur_sortie(p.hwnd, &p.sortie, taille, p.fps, p.debit, p.clock_origin)
                .with_context(|| format!("réveil de la session {}", self.session))?;
        // `sur_sortie` already requests one at construction. This second call
        // is a belt: without a key frame, the browser's decoder would have
        // no entry point into the new stream and would render a grey screen
        // until the next one — the hardware encoder's group of pictures is
        // open. Wake-up thus depends on no detail of `sur_sortie`.
        source.request_keyframe().context("image clé au réveil")?;
        let (largeur, hauteur) = source.dimensions();
        self.largeur = largeur;
        self.hauteur = hauteur;
        self.source = Some(source);
        tracing::info!(
            session = %self.session,
            largeur,
            hauteur,
            duree_ms = debut.elapsed().as_millis() as u64,
            "fenêtre réveillée"
        );
        Ok(())
    }

    /// Applies the pending pool orders.
    ///
    /// **Called at the head of the round, before everything else**: sleeping frees an
    /// encoder, and there is no reason to ask for one more when
    /// the order to give it back is already there.
    ///
    /// ⚠️ The pool's contract "all `Dormir` precede any `Reveiller`"
    /// only holds at EMISSION: window threads are independent and
    /// consume distinct channels, so nothing orders the HANDLING between
    /// two windows. No decision here assumes that a neighbouring sleep has
    /// already happened; the safety net is `echec_de_reveil`, which makes a
    /// wake-up that arrived too early be proposed again.
    pub(super) fn appliquer_les_ordres(
        &mut self,
        ordres: &ReceveurSession,
        ecritures: &SyncSender<AEcrire>,
        ctx: &Contexte,
    ) -> Fin {
        loop {
            match ordres.essayer_recevoir() {
                Ok(Message::Sommeil(Ordre::Dormir(raison))) => {
                    self.dormir();
                    let raison = crate::capteur::sommeil::raison_en_texte(raison);
                    let etat = DepuisCapteur::Sommeil {
                        endormie: true,
                        raison: raison.into(),
                    };
                    // `deposer` and not a blocking `send`: the queue may be
                    // full, and waiting on it without serving the commands
                    // would recreate the six-link deadlock of task 10
                    // of sub-block D4.
                    if let Fin::Terminer(motif) =
                        deposer(AEcrire::Etat(etat), ecritures, self.source.as_mut(), ctx)
                    {
                        return Fin::Terminer(motif);
                    }
                }
                Ok(Message::Sommeil(Ordre::Reveiller)) => {
                    if let Err(erreur) = self.reveiller() {
                        tracing::warn!(
                            session = %ctx.session,
                            // `cause::chaine` and NOT `%erreur`: `anyhow`'s plain
                            // `Display` only rendered the
                            // `with_context` set fifteen lines above
                            // ("réveil de la session …"), and threw away the
                            // cause — hence the HRESULT. Batch 25 counted 74
                            // then 52 refusals in a row without being able to say
                            // why. See `crate::cause`.
                            erreur = %crate::cause::chaine(&erreur),
                            "réveil refusé, la fenêtre reste endormie"
                        );
                        // **Indispensable, and nothing else replaces it.** The
                        // pool sets `eveillee = true` BEFORE the wake-up has
                        // happened: without this return path it would believe the window
                        // awake forever, would never give back its place and
                        // would never propose it again — window lost
                        // for good, for a refusal that is the nominal case
                        // when the hardware cap is reached. The call is
                        // short and outside any borrow of `self.source`: the
                        // registry takes a global lock.
                        crate::capteur::sommeil::echec_de_reveil(ctx.session);
                        continue;
                    }
                    let etat = DepuisCapteur::Sommeil {
                        endormie: false,
                        raison: String::new(),
                    };
                    if let Fin::Terminer(motif) =
                        deposer(AEcrire::Etat(etat), ecritures, self.source.as_mut(), ctx)
                    {
                        return Fin::Terminer(motif);
                    }
                }
                Ok(Message::Audio { actif }) => {
                    // Nothing to do locally: the sensor does not capture sound.
                    // It is only the postman here, as for the shares.
                    let message = DepuisCapteur::Audio { actif };
                    if let Fin::Terminer(motif) =
                        deposer(AEcrire::Etat(message), ecritures, self.source.as_mut(), ctx)
                    {
                        return Fin::Terminer(motif);
                    }
                }
                Ok(Message::Part { bps }) => {
                    // Nothing to do locally: the sensor does NOT tune its
                    // encoder on this share. It is the child that decides
                    // its encoding bitrate (it has the BWE), and the share is
                    // only a bound passed on to it. The sensor is only
                    // the postman here.
                    let message = DepuisCapteur::Part { bps };
                    if let Fin::Terminer(motif) =
                        deposer(AEcrire::Etat(message), ecritures, self.source.as_mut(), ctx)
                    {
                        return Fin::Terminer(motif);
                    }
                }
                Ok(Message::PressePapier { texte, octets }) => {
                    // Nothing to do locally — same regime as `Audio` and
                    // `Part` above: the sensor HOLDS the clipboard
                    // (it is the only one polling it and carrying the
                    // anti-echo guard, D1), but it has nothing to do with it for
                    // itself. It is only the postman here.
                    let message = DepuisCapteur::PressePapier { texte, octets };
                    // `deposer` and not a blocking `send`: it is the fifth
                    // ingredient of the pattern, and it matters more here
                    // than elsewhere — this message may weigh up to 64 KiB, where
                    // `Sommeil`, `Part` and `Audio` weigh a few bytes,
                    // so it fills the queue faster. Waiting on a full queue
                    // without serving the commands would recreate
                    // the six-link deadlock of task 10 of
                    // sub-block D4. See the comment on the `Dormir` arm
                    // above, which says so in so many words.
                    if let Fin::Terminer(motif) =
                        deposer(AEcrire::Etat(message), ecritures, self.source.as_mut(), ctx)
                    {
                        return Fin::Terminer(motif);
                    }
                }
                Err(VideOuFerme::Vide) => return Fin::Continuer,
                // The registry dropped our sender: the window is no longer
                // arbitrated. We keep serving rather than closing —
                // losing arbitration is not losing the session.
                Err(VideOuFerme::Ferme) => return Fin::Continuer,
            }
        }
    }
}
