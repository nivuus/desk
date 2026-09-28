//! The priority list of a loop round.
//!
//! `act_on_timeout` decides the SINGLE action taken per round. The order
//! is not arbitrary:
//!
//! - the due drain comes with absolute priority: it is the only way to
//!   guarantee that no mutation chains on without a full pass through
//!   `poll_output()` in between, whatever the state of the other queues;
//! - control and adaptation come before media: reconfiguring
//!   the encoder with a frame in flight would cost that frame;
//! - audio comes before video: a sound dropout is heard, a frame
//!   10 ms late is not seen;
//! - waiting on the socket only comes last, when there is nothing to
//!   emit.
//!
//! The body of each branch lives in its thematic module; this file only
//! carries the order.

use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use proto::control::AgentControl;
use str0m::Input;

use super::Session;

/// Result of handling an event or an internal loop round.
pub(super) enum Tick {
    Continue,
    Disconnected,
}

/// Minimal interval between two checks of `source.is_alive()` in
/// `act_on_timeout`. This call costs a system call at each round on the
/// Windows side (window lookup); a closed window stays closed, no need
/// to recheck it at 60 Hz.
const ALIVE_CHECK_INTERVAL: Duration = Duration::from_secs(1);

impl Session {
    /// Reacts to `Output::Timeout`: decides and performs AT MOST ONE mutation
    /// of `Rtc` (deferred drain of an already written frame, pending control
    /// message, due video frame, or handling of an incoming packet
    /// / str0m deadline), then gives control back to `run()`, which immediately
    /// calls `poll_output` again — it is this structure that guarantees
    /// draining before any following mutation (C2 of the review): there
    /// is no code path that mutates `Rtc` without `run()`
    /// calling `poll_output` right after. The priority given to the deferred
    /// drain (see `video_write_pending_drain`) is what makes this
    /// guarantee true even right after writing a frame: without it,
    /// `write_frame` (one mutation) followed directly by `handle_input`
    /// (a second) would break the same rule.
    ///
    /// Thirteen additional branches (a0bis: draining a control
    /// message produced outside the loop into `pending_control`; a0ter:
    /// pending adaptation decision; a1: pending resize;
    /// a1bis: pending visibility; a1ter: announcing a sleep
    /// change; a1ter-bis: announcing a fullscreen change
    /// (sub-block D8); a1quater: budget share granted by the sensor
    /// (sub-block D6); a1quinquies: audio order decided by the sensor
    /// (sub-block D7); a1sexies: rebuilding a dead audio capture
    /// detected locally, `AudioMort` as a fallback if the attempt budget
    /// is exhausted, and announcing a recovery PROVEN by a real packet
    /// (sub-block D9, full remedy brought by D10); a1septies: announcing
    /// a change of the VM's clipboard (sub-block P1); a1octies:
    /// writing a paste from the browser into the VM's clipboard,
    /// then arming the `Ctrl+V` injection (sub-block P2); a1nonies:
    /// announcing a change of the window's accent colour — the dominant
    /// hue of its icon (sub-block A1); a2:
    /// window check) NEVER queue, before giving control
    /// back, a write that would remain to be drained — that is the invariant
    /// this enumeration exists to audit. **Twelve of them (all but
    /// a1quater) do not even touch `self.rtc`**: only `self.source`,
    /// `self.audio_source`, the audio rebuild budget (a1sexies
    /// only) and/or `self.pending_control`, at most by queueing a
    /// control message there (`queue_control`, which only pushes onto a `VecDeque`,
    /// with no effect on `Rtc` before the next round).
    ///
    /// **a1quater is an exception, and it must be stated precisely**:
    /// `rtc.bwe().set_desired_bitrate` (body in `part`) DOES mutate an internal
    /// field of `Rtc` — and reconfigures str0m's pacer
    /// (`configure_pacer`) if a bandwidth estimate already exists.
    /// But this call queues NO packet: the effect it schedules
    /// on the probing side (str0m's `ProbeControl`, which can bring forward the deadline of
    /// the next probe and cause padding to be emitted) is only evaluated at the
    /// NEXT handling of `Input::Timeout`, never during this call.
    /// It is this absence of queueing — not the absence of mutation of
    /// `Rtc` — that preserves the drain invariant for this branch.
    ///
    /// Each still gives control back immediately after its action rather
    /// than chaining on to the next branch in the same call: the
    /// resize rebuilds a whole encoding chain
    /// (potentially long, see `WindowsSource::resize`), and treating it
    /// as a step in its own right — just like the branches that
    /// really write or read packets on `Rtc` (a0, a3, b,
    /// c…) — keeps this function readable as a single priority list
    /// rather than mixing two different styles.
    ///
    /// **To whoever reads this after one more branch**: this count and this
    /// enumeration are the audit point of the invariant "none of these
    /// branches queues, before giving control back, a write that
    /// would remain to be drained" — **NOT** "none of these branches mutates
    /// `Rtc`": a1quater does mutate a field of it (see above, and do not
    /// let this wording be copied into a future addition
    /// without rechecking this distinction). An addition that forgets to confront
    /// this invariant is checked against an incomplete list. Update them
    /// in the same gesture as the branch.
    ///
    /// Does not take `on_input`/`on_control`: `handle_input` never produces
    /// an application event directly (the events that
    /// result only come out through a future `poll_output`, hence through
    /// `run()`, which dispatches them itself).
    pub(super) fn act_on_timeout(&mut self, deadline: Instant) -> Result<Tick> {
        // a0) Drain due after the last written video frame or audio
        // packet. Checked with absolute priority, before everything else:
        // it is the only way to guarantee that no mutation ever chains on
        // without a full pass through `poll_output()` in between,
        // whatever the state of the other queues (see the comment of the
        // field and fix round 1 of task 11).
        if self.video_write_pending_drain || self.audio_write_pending_drain {
            // A single `handle_input(Timeout)` pops `to_payload` for ALL
            // tracks: the two flags therefore fall back together. Keeping
            // them separate stays necessary upstream — it is what lets
            // `write_audio` and `write_frame` report independently
            // that a write did happen.
            self.video_write_pending_drain = false;
            self.audio_write_pending_drain = false;
            self.rtc
                .handle_input(Input::Timeout(Instant::now()))
                .map_err(|e| anyhow!("handle_input timeout (drainage média) : {e}"))?;
            return Ok(Tick::Continue);
        }

        // a0bis) A control message produced outside the loop is waiting.
        //        Body in `controle`.
        if let Some(tick) = self.drainer_controle_externe() {
            return Ok(tick);
        }

        // a) A control message is pending. Body in `controle`,
        //    emptiness test included: the branch only lets through (`None`)
        //    without having mutated `Rtc` — empty queue, or channel not yet
        //    open while the session is not closing, in which case the
        //    message stays queued.
        if let Some(tick) = self.brancher_controle_en_file()? {
            return Ok(tick);
        }

        if self.ending {
            // End message sent (queue emptied above): done.
            return Ok(Tick::Disconnected);
        }

        // a0ter) Pending adaptation decision. Handled before the video
        //        branch and before the resize: reconfiguring
        //        the encoder with a frame in flight would cost that frame.
        //        Never mutates `Rtc`. Body in `adaptation`.
        if let Some(decision) = self.pending_decision.take() {
            self.appliquer_decision(decision);
            return Ok(Tick::Continue);
        }

        // a1) Pending resize, to handle before the video
        //     branch. Never mutates `Rtc` either, but stays a potentially
        //     long operation — new window AND D3D11 device,
        //     see `WindowsSource::resize` — handled here as a step in
        //     its own right rather than mixed with others in the same call, like
        //     the other branches. Body in `redimensionnement`.
        if let Some((width, height)) = self.pending_resize.take() {
            self.appliquer_redimensionnement(width, height);
            return Ok(Tick::Continue);
        }

        // a1bis) Pending visibility. After the resize and before
        //        video, for the same reason as it: the decision may
        //        release an encoder on the sensor side, which is long, and
        //        never mutates `Rtc`.
        if let Some((visible, focalisee)) = self.pending_visibility.take() {
            if let Err(erreur) = self.source.set_awake(visible, focalisee) {
                // Not fatal: losing the arbitration is not losing the session.
                // `cause::chaine` and not `%erreur`: `set_awake` goes through
                // `commander_simple`, which stacks a context — `anyhow`'s plain
                // `Display` would only render that one. See `crate::cause`.
                tracing::warn!(
                    erreur = %crate::cause::chaine(&erreur),
                    visible,
                    focalisee,
                    "visibilité refusée par le capteur"
                );
            }
            return Ok(Tick::Continue);
        }

        // a1ter) A sleep change to announce to the browser. Queried
        //        at each round where a1bis did not fire (otherwise that one
        //        has already exited through an early return); but
        //        `sommeil_a_annoncer` consumes: no message is ever
        //        re-emitted, so this branch cannot flood the control
        //        channel even at ~100 Hz.
        if let Some((endormie, raison)) = self.source.sommeil_a_annoncer() {
            self.queue_control(AgentControl::asleep(endormie, &raison));
            return Ok(Tick::Continue);
        }

        // a1ter-bis) A fullscreen change to announce to the browser.
        //            Same regime as a1ter just above:
        //            `plein_ecran_a_annoncer` CONSUMES, so no message
        //            is ever re-emitted and this branch cannot flood
        //            the control channel even at ~100 Hz.
        if let Some(actif) = self.source.plein_ecran_a_annoncer() {
            self.queue_control(AgentControl::fullscreen(actif));
            return Ok(Tick::Continue);
        }

        // a1quater) A budget share granted by the sensor. After a1ter
        //           (which puts nothing to sleep: it ANNOUNCES to the browser a
        //           sleep already decided on the sensor side — the actual
        //           falling asleep happens on the sensor side, not here). Consistency
        //           between a sleep and the share that follows from it is NOT played
        //           in this local order: it is played UPSTREAM, on the
        //           sensor side, where `distribuer` (the sleep orders) precedes
        //           `distribuer_les_parts` on all entry paths of the
        //           registry (`inscrire`, `retirer`, `signaler`,
        //           `echec_de_reveil`, wheel round — see
        //           `capteur/sommeil.rs`).
        //
        //           ⚠️ **All BUT ONE, and it must not be kept quiet** (I2, final
        //           branch review). The `rompus` path of
        //           `distribuer_les_parts` sends the shares FIRST, then
        //           detects broken channels, removes their sessions from the
        //           pool, and only then relays the orders that this
        //           removal generates. A session WOKEN by the place
        //           a dead one frees therefore receives its `Reveiller` AFTER the
        //           sleeping share computed just before, and only gets its
        //           awake share at the next wheel round.
        //           **Bound: `PERIODE_REARBITRAGE`, that is 250 ms**, during
        //           which this window encodes at the
        //           `PART_DORMANTE_BPS` floor. The single channel guarantees the order of
        //           DELIVERY, never the order of COMPUTATION — it is this
        //           distinction the previous wording missed.
        //
        //           Handling a1quater right after a1ter stays the most
        //           readable choice: it respects the arrival order rather than
        //           inverting it for no reason.
        //           Queues no packet — see this function's header doc
        //           on what `set_desired_bitrate` really
        //           mutates — but sets a decision that branch
        //           a0ter will apply at the next round.
        //           `part_a_appliquer` CONSUMES: no re-emission, hence
        //           no reconfiguration looping at ~100 Hz.
        if let Some(bps) = self.source.part_a_appliquer() {
            self.appliquer_part(bps);
            return Ok(Tick::Continue);
        }

        // a1quinquies) Un ordre audio décidé par le capteur. Après a1quater,
        //              pour la même raison de lisibilité : on respecte l'ordre
        //              d'arrivée plutôt que de l'inverser sans raison.
        //
        //              Ne met AUCUN paquet en file : `appliquer_audio` (tâche
        //              7, voir `piste_audio.rs`) ne touche que la source
        //              audio (`AudioSource::set_actif`, qui écrit un booléen
        //              lu par le fil de capture) et le budget du contrôleur
        //              (`Controleur::changer_audio_bps`, qui n'écrit qu'un
        //              champ de `Config`) — ni l'un ni l'autre ne met de
        //              paquet en file. L'invariant de drainage de cette
        //              fonction est donc préservé.
        //
        //              `audio_a_appliquer` CONSOMME : un `Start()`/`Stop()` par
        //              tour à ~100 Hz est exactement ce que cette consommation
        //              empêche.
        if let Some(actif) = self.source.audio_a_appliquer() {
            self.appliquer_audio(actif);
            return Ok(Tick::Continue);
        }

        // a1sexies) Deux transitions détectées LOCALEMENT, jamais poussées par
        //           le capteur : la capture audio de cette fenêtre vient de
        //           mourir définitivement (`crate::audio::LECTURES_ECHOUEES_MAX`
        //           erreurs de lecture WASAPI consécutives, `windows_audio.rs`),
        //           ou elle vient d'apporter la PREUVE qu'elle est repartie
        //           (un paquet réel, posé par `brancher_audio` — voir
        //           `piste_audio`).
        //
        //           **D10 inverse l'ordre du remède** (D9 ne savait que
        //           signaler `AudioMort`, jamais reconstruire).
        //           `reconstruire_ou_signaler` (corps dans `piste_audio`)
        //           TENTE D'ABORD de refabriquer la source ; `AudioMort` n'est
        //           plus le premier geste mais le REPLI — celui du cas où le
        //           budget de tentatives est épuisé, ou où il n'existe aucun
        //           reconstructeur — et où seule la promotion d'une voisine
        //           par le capteur peut encore rendre du son au groupe.
        //
        //           ❌ **« Chemin mono-fenêtre » figurait dans cette
        //           parenthèse et c'est FAUX** (revue transverse de fin de
        //           branche, second tour) : `demarrage/audio.rs::brancher`
        //           pose un reconstructeur INCONDITIONNELLEMENT dans son bras
        //           `Ok`, branche `None` COMPRISE. Les seuls cas réellement
        //           sans reconstructeur sont `AUDIO=0`, `TEST_FILE`, et un
        //           échec d'ouverture initiale.
        //
        //           🔴 **Et l'erreur portait à conséquence ICI plus
        //           qu'ailleurs, parce que ce fichier est celui qui APPELLE
        //           `reconstruire_ou_signaler`** : elle donnait à qui
        //           reprendra le legs n°1 le modèle mental exactement
        //           INVERSE du vrai. En mono-fenêtre la source EST
        //           reconstruite — puis le réarmement la RENDAIT MUETTE,
        //           `audio_porteuse` valant alors toujours `false` faute de
        //           capteur pour l'écrire.
        //
        //           ✅ **CORRIGÉ AU SOUS-BLOC D11 (leg 4), et mesuré** :
        //           `demarrage/audio.rs::brancher` pose
        //           `audio_porteuse = true` dans sa seule branche
        //           mono-fenêtre, et la recette ① relève 441 Hz reçus au
        //           vert contre la sentinelle au rouge. Voir
        //           `Session::reconstruire_ou_signaler` (`piste_audio.rs`).
        //
        //           `appliquer_audio` (a1quinquies juste au-dessus) ne court
        //           qu'à l'ARRIVÉE d'un ordre, jamais périodiquement : sans ce
        //           contrôle au tick, une capture qui meurt (ou qui reprend)
        //           entre deux ordres ne serait jamais signalée. Un `load`
        //           atomique ou une tentative de reconstruction bornée par son
        //           propre répit sont, l'un comme l'autre, bon marché par
        //           tour.
        //
        //           Le verrou `audio_mort_signale` est ce qui empêche
        //           d'inonder le capteur d'`AudioMort` : une fois posé, il ne
        //           retombe QUE sur deux transitions — un rattachement (voir
        //           plus bas), ou une RÉÉLECTION par le capteur
        //           (`appliquer_audio`, a1quinquies), qui réapprovisionne
        //           aussi le budget de tentatives. Sans ce second point de
        //           chute, trouvé en revue de la tâche 12, le budget posé une
        //           fois à la construction de la `Session` n'aurait permis
        //           qu'un seul cycle mort → reconstruit → prouvé par session,
        //           jamais plusieurs échecs CONSÉCUTIFS — l'inverse de ce que
        //           `REARMEMENTS_MAX` (`capteur/sommeil.rs`) est censé
        //           compter. `audio_vivant_a_annoncer`, lui, se CONSOMME dès
        //           sa lecture (même régime que `sommeil_a_annoncer` /
        //           `part_a_appliquer`), donc ne peut pas non plus réémettre
        //           `AudioVivant` en boucle.
        //
        //           La remise à zéro du verrou (`rattachement_survenu`) N'EST
        //           PAS elle-même une action : elle ne mute ni `Rtc` ni la
        //           source, ne met rien en file, et ne casse donc pas
        //           l'invariant de drainage même sans `return` — même régime
        //           que `last_alive_check` en a2. Un rattachement (capteur
        //           relancé) fait perdre au capteur la mémoire de tout
        //           `AudioMort` signalé avant la rupture : sans cette remise à
        //           zéro, cette fenêtre ne le réinformerait jamais.
        if self.source.rattachement_survenu() {
            self.audio_mort_signale = false;
        }
        // D10 : on tente d'abord de RECONSTRUIRE. `AudioMort` n'est plus le
        // premier geste mais le repli — celui du cas où l'arbre de processus a
        // disparu, et où seule la promotion d'une voisine peut encore rendre
        // du son au groupe.
        if !self.audio_mort_signale && self.reconstruire_ou_signaler(Instant::now()) {
            self.audio_mort_signale = true;
            self.source.signaler_audio_mort();
            return Ok(Tick::Continue);
        }
        if self.audio_vivant_a_annoncer {
            self.audio_vivant_a_annoncer = false;
            self.source.signaler_audio_vivant();
            return Ok(Tick::Continue);
        }

        // a1septies) Le presse-papier de la VM a changé (sous-bloc P1). Même
        //            régime qu'a1ter-bis : `presse_papier_a_annoncer` CONSOMME,
        //            donc aucun message n'est jamais réémis et cette branche ne
        //            peut pas inonder le canal de contrôle même à ~100 Hz. Ce
        //            point compte davantage ici qu'ailleurs : le texte peut
        //            peser jusqu'à `presse_papier::PRESSE_PAPIER_MAX` (64 Kio),
        //            là où un `Asleep` ou un `Fullscreen` pèse quelques octets.
        //
        //            `texte` à `None` n'est PAS « rien à annoncer » : c'est un
        //            REFUS de taille, que le navigateur doit dire à
        //            l'utilisateur (D-P1-1). C'est le `Option` EXTÉRIEUR, celui
        //            que rend la méthode, qui porte « rien à annoncer ».
        if let Some((texte, octets)) = self.source.presse_papier_a_annoncer() {
            self.queue_control(AgentControl::clipboard(texte, octets));
            return Ok(Tick::Continue);
        }

        // a1octies) Le navigateur a collé (sous-bloc P2). 🔴 **C'EST ICI QUE
        //           L'ORDRE DE D6 EST PRODUIT** — écrire le presse-papier de la
        //           VM d'abord, n'armer l'injection de `Ctrl+V` qu'ensuite, et
        //           seulement si l'écriture a RÉUSSI. Corps dans `collage`, qui
        //           porte aussi la seconde moitié de cet ordre (l'injection
        //           elle-même, drainée par `boucle::run`) : les deux maillons
        //           se lisent au même endroit plutôt qu'à deux fichiers d'écart.
        //
        //           **Cette branche ne mute PAS `self.rtc`** — comme a1quater
        //           et a1quinquies. Elle touche `self.source` (par le tube du
        //           capteur) et deux champs propres, et ne met aucun paquet en
        //           file : l'invariant de drainage audité en tête de fonction
        //           est préservé.
        if let Some(texte) = self.pending_clipboard.take() {
            self.traiter_le_collage(&texte);
            return Ok(Tick::Continue);
        }

        // a1nonies) La couleur d'accent de la fenêtre a changé (sous-bloc A1).
        //           Même régime qu'a1ter-bis et a1septies :
        //           `accent_a_annoncer` CONSOMME, donc aucun message n'est
        //           jamais réémis et cette branche ne peut pas inonder le canal
        //           de contrôle même à ~100 Hz.
        //
        //           ⚠️ **Le CAPTEUR annonce déjà au seul changement** — c'est
        //           `accent::SuiviAccent`, sur le fil de fenêtre. La
        //           consommation ici est donc une SECONDE garde, sur un autre
        //           processus, et elle n'est pas redondante : rien dans l'enfant
        //           ne sait ce que le capteur a déjà émis, et la fenêtre de
        //           reprise d'une connexion média rompue peut faire arriver le
        //           même état deux fois.
        //
        //           **Cette branche ne mute PAS `self.rtc`** — comme a1quater,
        //           a1quinquies et a1octies. Elle lit `self.source` et met au
        //           plus un message en file dans `self.pending_control`, sans
        //           effet sur `Rtc` avant le tour suivant : l'invariant de
        //           drainage audité en tête de fonction est préservé.
        if let Some(couleur) = self.source.accent_a_annoncer() {
            self.queue_control(AgentControl::accent(couleur));
            return Ok(Tick::Continue);
        }

        // a2) La fenêtre capturée a-t-elle disparu ? Coûte un appel système
        // côté Windows (recherche de la fenêtre) : espacé par
        // `ALIVE_CHECK_INTERVAL` plutôt que vérifié à chaque tour de
        // boucle — une fenêtre fermée le reste.
        let now = Instant::now();
        if now.saturating_duration_since(self.last_alive_check) >= ALIVE_CHECK_INTERVAL {
            self.last_alive_check = now;
            if !self.source.is_alive() {
                self.begin_ending("fenêtre fermée");
                return Ok(Tick::Continue);
            }
        }

        // a3) Un paquet audio, si la piste est négociée et qu'un paquet
        //     attend. AVANT la vidéo : une coupure sonore s'entend, une image
        //     en retard de 10 ms ne se voit pas. L'audio a de plus une
        //     cadence dure de 10 ms, quand la vidéo est opportuniste par
        //     nature. Corps dans `piste_audio`.
        if let Some(tick) = self.brancher_audio() {
            return Ok(tick);
        }

        // b) Une image vidéo, si son échéance est atteinte et la piste
        //    négociée. Corps dans `piste_video`.
        if let Some(tick) = self.brancher_video() {
            return Ok(tick);
        }

        // b0) Requête TURN en attente d'émission (allocation, rafraîchissement
        //     du bail, permission, liaison de canal). Ne mute jamais `Rtc` :
        //     c'est un échange avec le serveur de relais, invisible de str0m.
        //     Placée juste avant l'attente pour que le rafraîchissement du
        //     bail ne dépende pas de l'arrivée d'un paquet. Corps dans
        //     `relais`.
        if let Some(tick) = self.emettre_requete_turn() {
            return Ok(tick);
        }

        // c) Rien à émettre : attendre un paquet entrant, borné à la fois
        //    par l'échéance de `Rtc` et par les prochaines échéances de
        //    média. Corps dans `socket`.
        self.brancher_attente(deadline)
    }
}

// Les tests vivent dans un fichier voisin : ce fichier-ci a franchi 500
// lignes en ajoutant la couverture des branches a1bis/a1ter (tâche 8,
// sous-bloc D5). Voir l'en-tête de `tick/tests.rs`.
#[cfg(test)]
mod tests;
