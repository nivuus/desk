//! La boucle de transport, et les deux points de drainage qui la précèdent.
//!
//! **Extrait de `transport.rs`** (sous-bloc F1, tâche 17), qui avait franchi
//! les 500 lignes en gagnant le champ `input_channel` — celui qui permet à
//! `evenements::destination` d'aiguiller par CANAL plutôt que par le seul
//! drapeau binaire. C'est la deuxième extraction de ce fichier pour la même
//! raison : D10 en avait déjà sorti `initialisation.rs`.
//!
//! Les trois fonctions vont ensemble : `run` et `drain_quietly` sont le même
//! drainage à deux régimes — avec et sans rappels applicatifs —, et
//! `accept_offer` est l'un des deux points de mutation qui exigent le second.

use anyhow::{anyhow, Result};
use proto::control::ClientControl;
use proto::input::InputMessage;
use str0m::Output;

use super::tick::Tick;
use super::Session;

impl Session {
    /// Accepte l'offre du navigateur et produit la réponse SDP.
    pub fn accept_offer(&mut self, offer_sdp: &str) -> Result<String> {
        let offer = str0m::change::SdpOffer::from_sdp_string(offer_sdp)
            .map_err(|e| anyhow!("offre SDP illisible : {e}"))?;
        let answer = self
            .rtc
            .sdp_api()
            .accept_offer(offer)
            .map_err(|e| anyhow!("offre refusée : {e}"))?;

        // Même raisonnement que dans `new()` : `accept_offer` mute `Rtc`, on
        // draine avant de rendre la main, sans dépendre de ce que fera
        // l'appelant ensuite.
        self.drain_quietly()?;

        Ok(answer.to_sdp_string())
    }

    /// Boucle de transport : tourne jusqu'à déconnexion ou erreur fatale.
    ///
    /// Bloque volontairement (lecture UDP synchrone) — à appeler depuis un
    /// thread dédié (`tokio::task::spawn_blocking`), jamais depuis un
    /// ouvrier async de tokio (voir le commentaire de module, I6 de la revue).
    pub fn run(
        &mut self,
        on_input: &mut impl FnMut(InputMessage),
        on_control: &mut impl FnMut(ClientControl),
    ) -> Result<()> {
        loop {
            match self.rtc.poll_output().map_err(|e| anyhow!("poll_output : {e}"))? {
                Output::Timeout(deadline) => {
                    if let Tick::Disconnected = self.act_on_timeout(deadline)? {
                        return Ok(());
                    }
                    // 🔴 **JUSTE APRÈS `act_on_timeout`, ET C'EST LE SECOND
                    // MAILLON DE L'ORDRE DE D6** (sous-bloc P2). La branche
                    // `a1octies` vient peut-être d'écrire le presse-papier de
                    // la VM et d'armer ce drapeau ; l'injection de `Ctrl+V` ne
                    // peut donc pas précéder l'écriture.
                    //
                    // **Ici et pas dans `act_on_timeout`** : cette fonction-là
                    // ne reçoit ni `on_input` ni `on_control`, quand `run` les
                    // reçoit tous deux. Lui ajouter un paramètre pour une seule
                    // branche serait un coût permanent sur une fonction qui
                    // porte onze branches et soixante lignes d'audit
                    // d'invariant, contre un gain nul (D-P2-1). Corps dans
                    // `collage`, aux côtés de l'écriture qu'il suit.
                    self.injecter_le_collage(on_input);
                }
                Output::Transmit(transmit) => {
                    // Route vers le socket direct ou vers le relais TURN selon
                    // la source que str0m indique. Corps dans `relais`.
                    self.envoyer(&transmit);
                }
                Output::Event(event) => {
                    if let Tick::Disconnected = self.handle_event(event, on_input, on_control) {
                        return Ok(());
                    }
                }
            }
        }
    }

    /// Draine `poll_output` jusqu'à `Output::Timeout`, sans callbacks
    /// applicatifs. Utilisé uniquement aux points de mutation antérieurs à
    /// `run()` (`new`, `accept_offer`) : aucune piste ni canal ne peut
    /// encore produire de données applicatives à ce stade.
    pub(super) fn drain_quietly(&mut self) -> Result<()> {
        loop {
            match self.rtc.poll_output().map_err(|e| anyhow!("poll_output : {e}"))? {
                Output::Timeout(_) => return Ok(()),
                Output::Transmit(transmit) => {
                    // Même point d'émission unique que `run` : un paquet émis
                    // pendant un drainage doit passer par le relais si c'est
                    // par là qu'il doit sortir.
                    self.envoyer(&transmit);
                }
                Output::Event(event) => {
                    if let Tick::Disconnected = self.handle_event(event, &mut |_| {}, &mut |_| {}) {
                        return Ok(());
                    }
                }
            }
        }
    }
}
