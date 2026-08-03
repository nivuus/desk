//! Traitement des événements que str0m remonte : changement d'état ICE,
//! ouverture de canal, données reçues, estimation de débit sortant.
//!
//! Ces fonctions s'exécutent PENDANT le drainage de `poll_output` : aucune ne
//! doit muter `Rtc`. Ce qui demande une reconfiguration (redimensionnement,
//! décision d'adaptation) est seulement mémorisé — `pending_resize`,
//! `pending_decision` — et la liste de priorités de `tick` l'applique au tour
//! suivant, comme une étape à part entière.

use std::time::Instant;

use proto::control::{AgentControl, ClientControl};
use proto::input::InputMessage;
use str0m::{Event, IceConnectionState};

use super::tick::Tick;
use super::Session;
use crate::congestion;

impl Session {
    pub(super) fn handle_event(
        &mut self,
        event: Event,
        on_input: &mut impl FnMut(InputMessage),
        on_control: &mut impl FnMut(ClientControl),
    ) -> Tick {
        match event {
            Event::IceConnectionStateChange(IceConnectionState::Disconnected) => {
                tracing::warn!("ICE déconnecté");
                return Tick::Disconnected;
            }
            Event::IceConnectionStateChange(state) => {
                tracing::info!(?state, écoulé = ?self.started.elapsed(), "état ICE");
            }
            Event::Closed => {
                // I1 : émis à la réception du close_notify DTLS — typiquement
                // quand l'utilisateur ferme l'onglet. Sans ce bras, cet
                // événement tombait dans `_ => {}` et l'agent continuait
                // d'émettre jusqu'à l'expiration ICE, bien après le départ
                // du pair.
                tracing::info!("connexion fermée par le pair (close_notify DTLS)");
                return Tick::Disconnected;
            }
            Event::MediaAdded(media) => {
                tracing::info!(mid = ?media.mid, kind = ?media.kind, "piste négociée");
                match media.kind {
                    str0m::media::MediaKind::Video => self.video_mid = Some(media.mid),
                    str0m::media::MediaKind::Audio => self.audio_mid = Some(media.mid),
                }
            }
            Event::ChannelOpen(id, label) => {
                tracing::info!(%label, "canal de données ouvert");
                if label == "control" {
                    self.control_channel = Some(id);
                    // I3 : envoyer `ready` ici, pas juste après la réponse
                    // SDP — à ce moment-là SCTP n'est pas encore ouvert,
                    // `control_channel` valait `None`, et le message était
                    // silencieusement perdu. Le client de la tâche 8 attend
                    // ce message pour effacer sa bannière de statut.
                    let (width, height) = self.dimensions;
                    self.queue_control(AgentControl::ready(width, height));
                }
            }
            Event::ChannelData(data) => {
                self.dispatch_channel_data(&data, on_input, on_control);
            }
            Event::KeyframeRequest(request) => {
                // Le navigateur demande une image clé, typiquement après une
                // perte de paquet détectée par le décodeur. Le groupe
                // d'images de l'encodeur matériel est ouvert (voir
                // `encode::configure_rate_control`) : sans ce relais, aucune
                // image clé n'est plus jamais produite après le démarrage, et
                // la perte corrompt la vidéo jusqu'à reconnexion. Ne mute pas
                // `Rtc` — seul l'encodeur (côté `VideoSource`) est affecté —
                // donc ce relais respecte l'invariant de drainage documenté
                // en tête de fichier même appelé depuis `handle_event`.
                tracing::debug!(mid = ?request.mid, "image clé demandée par le pair");
                if let Err(e) = self.source.request_keyframe() {
                    tracing::warn!(erreur = %e, mid = ?request.mid, "échec de la demande d'image clé");
                }
            }
            Event::EgressBitrateEstimate(kind) => {
                // Les deux variantes portent une estimation ; seule REMB
                // nomme en plus le `mid` concerné, dont on n'a pas l'usage
                // avec une piste vidéo unique.
                let bps = match kind {
                    str0m::bwe::BweKind::Twcc(b) => b.as_u64(),
                    str0m::bwe::BweKind::Remb(_, b) => b.as_u64(),
                    // `BweKind` est `#[non_exhaustive]` côté str0m : une
                    // variante future retomberait ici plutôt que d'empêcher
                    // la compilation. Rien à faire de mieux qu'ignorer une
                    // estimation qu'on ne sait pas encore interpréter.
                    _ => return Tick::Continue,
                };
                self.derniere_estimation_bps = Some((bps as u32, Instant::now()));
            }
            Event::MediaEgressStats(stats) => {
                // Seule la piste vidéo alimente la décision : l'audio a un
                // débit fixe et son budget est déjà retiré par le contrôleur.
                if Some(stats.mid) != self.video_mid {
                    return Tick::Continue;
                }
                // I4 (revue finale de branche) : une estimation reçue une
                // seule fois puis plus jamais (TWCC qui se tarit alors que la
                // session survit) est traitée comme absente au-delà
                // d'`EXPIRATION_ESTIMATION`, plutôt que d'être utilisée
                // indéfiniment — potentiellement la dernière valeur haute
                // avant l'incident, ce qui annoncerait « Bonne » sur un lien
                // mort.
                let now = Instant::now();
                let estimate_bps = self.estimation_fraiche(now);
                let observation = congestion::Observation {
                    estimate_bps,
                    rtt: stats.rtt,
                    loss: stats.loss,
                    at: now,
                };
                let absence = observation.estimate_bps.is_none();
                if absence && !self.absence_bwe_signalee {
                    self.absence_bwe_signalee = true;
                    tracing::warn!(
                        "aucune estimation de bande passante reçue : l'adaptation reste \
                         indisponible et le débit demeure au plafond configuré"
                    );
                }
                tracing::debug!(
                    estimation = ?observation.estimate_bps,
                    rtt = ?observation.rtt,
                    perte = ?observation.loss,
                    "observation réseau"
                );
                // `observer` DOIT être appelé avant de lire `courant()`
                // ci-dessous : c'est lui qui, dans sa branche sans
                // estimation, met `courant.adaptation` à jour vers
                // `Indisponible` (voir son commentaire). Lire `courant()`
                // avant cet appel rendrait un instantané périmé (encore
                // `Active`) sur la transition qui nous intéresse le plus.
                if let Some(decision) = self.congestion.observer(observation) {
                    // Mémorisée, pas appliquée : voir le commentaire du champ.
                    self.pending_decision = Some(decision);
                }
                if absence {
                    // I2 (revue finale de branche) : `Controleur::observer`
                    // ne produit JAMAIS de décision quand l'estimation
                    // manque (voir son commentaire, retour anticipé) — sans
                    // ce relais explicite, `Adaptation::Indisponible`
                    // n'atteint donc jamais le navigateur, alors que la spec
                    // l'exige nommément. On pose `self.congestion.courant()`,
                    // lu APRÈS l'appel ci-dessus : son champ `adaptation` est
                    // désormais à jour, et le reste (débit, taille) reflète
                    // la dernière décision réelle — la seule chose de sensé à
                    // annoncer tant qu'aucune nouvelle donnée n'arrive.
                    if !self.indisponibilite_annoncee {
                        self.indisponibilite_annoncee = true;
                        self.pending_decision = Some(self.congestion.courant());
                    }
                } else {
                    // Une estimation fraîche revient : une indisponibilité
                    // ultérieure redeviendra une information neuve.
                    self.indisponibilite_annoncee = false;
                }
            }
            _ => {}
        }
        Tick::Continue
    }

    fn dispatch_channel_data(
        &mut self,
        data: &str0m::channel::ChannelData,
        on_input: &mut impl FnMut(InputMessage),
        on_control: &mut impl FnMut(ClientControl),
    ) {
        if data.binary {
            match InputMessage::decode(&data.data) {
                Ok(message) => on_input(message),
                Err(e) => tracing::warn!(erreur = %e, "message d'entrée invalide"),
            }
        } else {
            match std::str::from_utf8(&data.data).map(serde_json::from_str::<ClientControl>) {
                Ok(Ok(message)) => {
                    self.memoriser_controle(&message);
                    on_control(message);
                }
                Ok(Err(e)) => tracing::warn!(erreur = %e, "message de contrôle invalide"),
                Err(e) => tracing::warn!(erreur = %e, "contrôle non UTF-8"),
            }
        }
    }

    /// Mémorise un `ClientControl` reçu, sans jamais l'appliquer sur-le-champ.
    ///
    /// Ce code s'exécute pendant le drainage de `poll_output` : reconstruire
    /// la chaîne d'encodage ou relâcher un encodeur y serait long et romprait
    /// l'invariant de drainage de str0m (une seule mutation de `Rtc` par
    /// appel). On mémorise seulement la demande la plus récente ;
    /// `act_on_timeout` l'applique à son tour, comme une étape à part
    /// entière.
    fn memoriser_controle(&mut self, message: &ClientControl) {
        match message {
            ClientControl::Resize { width, height, .. } => {
                self.pending_resize = Some((*width, *height));
            }
            ClientControl::Visibility { visible, focused, .. } => {
                self.pending_visibility = Some((*visible, *focused));
            }
        }
    }

    /// Point d'entrée `#[cfg(test)]` qui exerce `memoriser_controle` sans
    /// passer par un `str0m::channel::ChannelData` réel — str0m interdit
    /// délibérément sa construction hors de son propre crate (voir
    /// `ChannelId`, « Deliberately not Deref or From to avoid this Id being
    /// created outside of this module »). Les tests d'intégration existants
    /// de ce module contournent cela en montant un second `Rtc` str0m en
    /// pair local ; cette voie-ci est plus légère pour un test qui ne vérifie
    /// que la mémorisation elle-même.
    #[cfg(test)]
    pub(super) fn dispatch_controle_de_test(&mut self, json: &str) {
        let message: ClientControl =
            serde_json::from_str(json).expect("json de test valide dans dispatch_controle_de_test");
        self.memoriser_controle(&message);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use anyhow::Result;
    use str0m::Output;

    use super::*;
    use crate::h264::AccessUnit;
    use crate::source::VideoSource;
    use crate::transport::fixtures;

    /// `ClientControl::Visibility` reçu doit être mémorisé dans
    /// `pending_visibility`, pas appliqué sur-le-champ.
    ///
    /// Même raison que pour `Resize` : ce code court pendant le drainage de
    /// `poll_output`, et relâcher un encodeur y romprait l'invariant d'une
    /// seule mutation de `Rtc` par appel. `dispatch_controle_de_test` est un
    /// point d'entrée `#[cfg(test)]` qui court-circuite `ChannelData` (str0m
    /// interdit délibérément sa construction hors du crate) tout en exerçant
    /// exactement le même chemin de mémorisation que `dispatch_channel_data`.
    #[test]
    fn un_message_de_visibilite_est_memorise_et_non_applique_sur_le_champ() {
        let source = Box::new(fixtures::video_test_source());
        let mut session = Session::new(source, fixtures::local_ip(), Instant::now(), 12_000_000)
            .expect("session");

        let json = r#"{"type":"visibility","v":3,"visible":false,"focused":false}"#;
        session.dispatch_controle_de_test(json);

        assert_eq!(session.pending_visibility, Some((false, false)));
    }

    /// Preuve d'intégration que `Event::KeyframeRequest` (émis par str0m
    /// quand le pair envoie un PLI/FIR RTCP — ce que fait un navigateur après
    /// une perte de paquet détectée par son décodeur) est bien relayé jusqu'à
    /// `VideoSource::request_keyframe`, sans passer par un mock du trait
    /// `Event` : le pair local ici est un vrai second `Rtc` str0m, comme dans
    /// `atteint_la_cadence_video_visee_avec_un_pair_local`.
    ///
    /// N'exerce PAS le chemin `WindowsSource`/`H264Encoder::request_keyframe`
    /// réel (`#![cfg(windows)]`, indisponible sur la machine de compilation
    /// Linux) : seul le relais `handle_event` → `Session::source` est prouvé
    /// ici. Le câblage `WindowsSource::request_keyframe` →
    /// `H264Encoder::request_keyframe` (`SetValue` sur
    /// `CODECAPI_AVEncVideoForceKeyFrame`) reste vérifié par lecture et par
    /// la compilation croisée Windows, pas par un test automatisé.
    #[test]
    fn relaie_une_demande_d_image_cle_du_pair_vers_la_source() {
        use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
        use std::sync::Arc;
        use std::thread;
        use str0m::change::SdpAnswer;
        use str0m::media::{Direction, KeyframeRequestKind, MediaKind};

        /// Enveloppe `FileSource` en comptant les appels à
        /// `request_keyframe`, seule façon d'observer depuis ce test que le
        /// relais a bien eu lieu (le compteur est partagé via `Arc` avant que
        /// la source ne soit déplacée dans `Session`, qui la possède ensuite
        /// depuis le thread dédié de `Session::run`).
        struct CountingSource {
            inner: crate::source::FileSource,
            keyframe_requests: Arc<AtomicUsize>,
        }

        impl VideoSource for CountingSource {
            fn next_frame(&mut self) -> Option<AccessUnit> {
                self.inner.next_frame()
            }
            fn dimensions(&self) -> (u32, u32) {
                self.inner.dimensions()
            }
            fn request_keyframe(&mut self) -> Result<()> {
                self.keyframe_requests.fetch_add(1, AtomicOrdering::SeqCst);
                Ok(())
            }
        }

        let local_ip = fixtures::local_ip();
        let keyframe_requests = Arc::new(AtomicUsize::new(0));
        let source = Box::new(CountingSource {
            inner: fixtures::video_test_source(),
            keyframe_requests: keyframe_requests.clone(),
        });

        let mut session = Session::new(source, local_ip, Instant::now(), 12_000_000).expect("session");

        let (peer_socket, peer_addr, mut peer_rtc) = fixtures::local_peer(local_ip, false);

        let mut api = peer_rtc.sdp_api();
        // Recvonly côté pair == la piste vidéo que le navigateur reçoit
        // réellement de l'agent ; c'est sur ce `mid` que `writer(...)` émettra
        // le PLI plus bas (str0m nomme cet accès « writer » indépendamment du
        // sens du média — c'est l'API par laquelle la rétroaction RTCP sort).
        let video_mid = api.add_media(MediaKind::Video, Direction::RecvOnly, None, None, None);
        api.add_channel("control".to_string());
        api.add_channel("input".to_string());
        let (offer, pending) = api.apply().expect("offre non vide");

        let answer_sdp = session.accept_offer(&offer.to_sdp_string()).expect("offre acceptée");
        let answer = SdpAnswer::from_sdp_string(&answer_sdp).expect("réponse SDP valide");
        peer_rtc
            .sdp_api()
            .accept_answer(pending, answer)
            .expect("réponse acceptée par le pair");

        thread::spawn(move || {
            let mut on_input = |_| {};
            let mut on_control = |_| {};
            let _ = session.run(&mut on_input, &mut on_control);
        });

        let hard_deadline = Instant::now() + Duration::from_secs(10);
        let mut keyframe_requested_at_peer = false;

        loop {
            let now = Instant::now();
            if keyframe_requests.load(AtomicOrdering::SeqCst) > 0 {
                break; // Preuve faite : le relais a atteint la source.
            }
            if now >= hard_deadline {
                panic!(
                    "délai dépassé : le pair local ne s'est jamais connecté, ou \
                     Event::KeyframeRequest n'a jamais atteint VideoSource::request_keyframe \
                     (compteur toujours à 0)"
                );
            }

            match peer_rtc.poll_output().expect("poll_output du pair") {
                Output::Timeout(t) => {
                    let wait = t
                        .saturating_duration_since(now)
                        .min(hard_deadline.saturating_duration_since(now));
                    if fixtures::poll_peer_socket(&mut peer_rtc, &peer_socket, peer_addr, now, wait) {
                        continue;
                    }
                }
                Output::Transmit(t) => {
                    let _ = peer_socket.send_to(&t.contents, t.destination);
                }
                Output::Event(Event::Connected) => {
                    if !keyframe_requested_at_peer {
                        keyframe_requested_at_peer = true;
                        // Exactement ce que fait un navigateur après une
                        // perte de paquet détectée par son décodeur : demander
                        // une image clé via un PLI RTCP. `fb_pli` est vrai par
                        // défaut pour un codec vidéo dans str0m (voir
                        // `format::payload_params::PayloadParams::new`), donc
                        // cette négociation n'a rien de spécial à activer côté
                        // offre/réponse SDP.
                        let mut writer = peer_rtc.writer(video_mid).expect("writer vidéo");
                        writer
                            .request_keyframe(None, KeyframeRequestKind::Pli)
                            .expect("PLI négocié par défaut sur un codec vidéo (fb_pli)");
                    }
                }
                Output::Event(_) => {}
            }
        }

        assert!(
            keyframe_requests.load(AtomicOrdering::SeqCst) > 0,
            "Event::KeyframeRequest du pair n'a jamais atteint VideoSource::request_keyframe"
        );
    }
}
