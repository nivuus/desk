//! La liste de priorités d'un tour de boucle.
//!
//! `act_on_timeout` décide de la SEULE action entreprise par tour. L'ordre
//! n'est pas arbitraire :
//!
//! - le drainage dû passe en priorité absolue : c'est la seule façon de
//!   garantir qu'aucune mutation ne s'enchaîne sans un passage complet par
//!   `poll_output()` entre les deux, quel que soit l'état des autres files ;
//! - le contrôle et l'adaptation passent avant les médias : reconfigurer
//!   l'encodeur avec une image en vol coûterait cette image ;
//! - l'audio passe avant la vidéo : une coupure sonore s'entend, une image
//!   en retard de 10 ms ne se voit pas ;
//! - l'attente sur le socket ne vient qu'en dernier, quand il n'y a rien à
//!   émettre.
//!
//! Le corps de chaque branche vit dans son module thématique ; ce fichier ne
//! porte que l'ordre.

use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use proto::control::AgentControl;
use str0m::Input;

use super::Session;

/// Résultat du traitement d'un événement ou d'un tour de boucle interne.
pub(super) enum Tick {
    Continue,
    Disconnected,
}

/// Intervalle minimal entre deux vérifications de `source.is_alive()` dans
/// `act_on_timeout`. Cet appel coûte un appel système à chaque tour côté
/// Windows (recherche de la fenêtre) ; une fenêtre fermée le reste, inutile
/// de le revérifier à 60 Hz.
const ALIVE_CHECK_INTERVAL: Duration = Duration::from_secs(1);

impl Session {
    /// Réagit à `Output::Timeout` : décide et effectue AU PLUS UNE mutation
    /// de `Rtc` (drainage différé d'une image déjà écrite, message de
    /// contrôle en attente, image vidéo due, ou traitement d'un paquet
    /// entrant / échéance str0m), puis rend la main à `run()`, qui rappelle
    /// immédiatement `poll_output` — c'est cette structure qui garantit le
    /// drainage avant toute mutation suivante (C2 de la revue) : il
    /// n'existe aucun chemin de code qui mute `Rtc` sans que `run()` ne
    /// rappelle `poll_output` juste après. La priorité donnée au drainage
    /// différé (voir `video_write_pending_drain`) est ce qui rend cette
    /// garantie vraie même juste après l'écriture d'une image : sans elle,
    /// `write_frame` (une mutation) suivi directement de `handle_input`
    /// (une seconde) violerait la même règle.
    ///
    /// Sept branches supplémentaires (a0bis : drainage d'un message de
    /// contrôle produit hors boucle vers `pending_control` ; a0ter :
    /// décision d'adaptation en attente ; a1 : redimensionnement en attente ;
    /// a1bis : visibilité en attente ; a1ter : annonce d'un changement de
    /// sommeil ; a1quater : part de budget accordée par le capteur (sous-bloc
    /// D6) ; a2 : vérification de la fenêtre) ne mutent JAMAIS `Rtc` —
    /// elles ne touchent que `self.source`, `self.audio_source` et/ou
    /// `self.pending_control`, au plus en y mettant en file un message de
    /// contrôle (`queue_control`, qui n'empile qu'un `VecDeque`, sans effet
    /// sur `Rtc` avant le tour suivant). Chacune rend quand même la main
    /// immédiatement après son action plutôt que d'enchaîner sur la branche
    /// suivante dans le même appel : le redimensionnement reconstruit une
    /// chaîne d'encodage entière (potentiellement long, voir
    /// `WindowsSource::resize`), et le traiter comme une étape à part
    /// entière — au même titre que les branches qui, elles, mutent
    /// réellement `Rtc` — garde cette fonction lisible comme une seule
    /// liste de priorités plutôt que de mêler deux styles différents.
    ///
    /// **À qui lira ceci après une huitième branche** : ce compte et cette
    /// énumération sont le point d'audit de l'invariant « aucune de ces
    /// branches ne mute `Rtc` » — une addition qui l'oublie se vérifie sur une
    /// liste incomplète. Mets-les à jour dans le même geste que la branche.
    ///
    /// Ne prend pas `on_input`/`on_control` : `handle_input` ne produit
    /// jamais d'événement applicatif directement (les événements qui en
    /// résultent ne sortent que via un futur `poll_output`, donc via
    /// `run()`, qui les dispatche lui-même).
    pub(super) fn act_on_timeout(&mut self, deadline: Instant) -> Result<Tick> {
        // a0) Drainage dû après la dernière image vidéo ou le dernier paquet
        // audio écrit. Vérifié en priorité absolue, avant tout le reste :
        // c'est la seule façon de garantir qu'aucune mutation ne s'enchaîne
        // jamais sans un passage complet par `poll_output()` entre les deux,
        // quel que soit l'état des autres files (voir le commentaire du
        // champ et la ronde de correction 1 de la tâche 11).
        if self.video_write_pending_drain || self.audio_write_pending_drain {
            // Un seul `handle_input(Timeout)` dépile `to_payload` pour TOUTES
            // les pistes : les deux drapeaux retombent donc ensemble. Les
            // garder séparés reste nécessaire en amont — c'est ce qui permet
            // à `write_audio` et `write_frame` de signaler indépendamment
            // qu'une écriture a bien eu lieu.
            self.video_write_pending_drain = false;
            self.audio_write_pending_drain = false;
            self.rtc
                .handle_input(Input::Timeout(Instant::now()))
                .map_err(|e| anyhow!("handle_input timeout (drainage média) : {e}"))?;
            return Ok(Tick::Continue);
        }

        // a0bis) Un message de contrôle produit hors de la boucle attend.
        //        Corps dans `controle`.
        if let Some(tick) = self.drainer_controle_externe() {
            return Ok(tick);
        }

        // a) Un message de contrôle est en attente. Corps dans `controle`,
        //    test de vacuité compris : la branche ne laisse passer (`None`)
        //    que sans avoir muté `Rtc` — file vide, ou canal pas encore
        //    ouvert alors que la session n'est pas en clôture, auquel cas le
        //    message reste en file.
        if let Some(tick) = self.brancher_controle_en_file()? {
            return Ok(tick);
        }

        if self.ending {
            // Message de fin envoyé (file vidée ci-dessus) : terminé.
            return Ok(Tick::Disconnected);
        }

        // a0ter) Décision d'adaptation en attente. Traitée avant la branche
        //        vidéo et avant le redimensionnement : reconfigurer
        //        l'encodeur avec une image en vol coûterait cette image.
        //        Ne mute jamais `Rtc`. Corps dans `adaptation`.
        if let Some(decision) = self.pending_decision.take() {
            self.appliquer_decision(decision);
            return Ok(Tick::Continue);
        }

        // a1) Redimensionnement en attente, à traiter avant la branche
        //     vidéo. Ne mute jamais `Rtc` non plus, mais reste une opération
        //     potentiellement longue — fenêtre ET périphérique D3D11 neufs,
        //     voir `WindowsSource::resize` — traitée ici comme une étape à
        //     part entière plutôt que mêlée à d'autres dans le même appel, à
        //     l'image des autres branches. Corps dans `redimensionnement`.
        if let Some((width, height)) = self.pending_resize.take() {
            self.appliquer_redimensionnement(width, height);
            return Ok(Tick::Continue);
        }

        // a1bis) Visibilité en attente. Après le redimensionnement et avant la
        //        vidéo, pour la même raison que lui : la décision peut
        //        relâcher un encodeur côté capteur, ce qui est long, et ne
        //        mute jamais `Rtc`.
        if let Some((visible, focalisee)) = self.pending_visibility.take() {
            if let Err(erreur) = self.source.set_awake(visible, focalisee) {
                // Non fatal : perdre l'arbitrage n'est pas perdre la session.
                tracing::warn!(%erreur, visible, focalisee, "visibilité refusée par le capteur");
            }
            return Ok(Tick::Continue);
        }

        // a1ter) Un changement de sommeil à annoncer au navigateur. Interrogée
        //        à chaque tour où a1bis ne s'est pas déclenchée (sinon celle-ci
        //        est déjà sortie par un retour anticipé) ; mais
        //        `sommeil_a_annoncer` consomme : aucun message n'est jamais
        //        réémis, donc cette branche ne peut pas inonder le canal de
        //        contrôle même à ~100 Hz.
        if let Some((endormie, raison)) = self.source.sommeil_a_annoncer() {
            self.queue_control(AgentControl::asleep(endormie, &raison));
            return Ok(Tick::Continue);
        }

        // a1quater) Une part de budget accordée par le capteur. Après le
        //           sommeil, dont elle découle : une fenêtre qu'on vient
        //           d'endormir reçoit sa part d'endormie dans le même lot, et
        //           l'appliquer avant l'ordre décrirait l'état précédent.
        //           Ne mute pas `Rtc` au sens du drainage — `set_desired_bitrate`
        //           n'écrit aucun paquet —, mais pose une décision que la
        //           branche a0ter appliquera au tour suivant.
        //           `part_a_appliquer` CONSOMME : aucune réémission, donc
        //           aucune reconfiguration en boucle à ~100 Hz.
        if let Some(bps) = self.source.part_a_appliquer() {
            self.appliquer_part(bps);
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
