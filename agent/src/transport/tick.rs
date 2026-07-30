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
    /// Quatre branches supplémentaires (a0bis : drainage d'un message de
    /// contrôle produit hors boucle vers `pending_control` ; a0ter :
    /// décision d'adaptation en attente ; a1, a2 : redimensionnement en
    /// attente et vérification de la fenêtre) ne mutent JAMAIS `Rtc` — elles
    /// ne touchent que `self.source`, `self.audio_source` et/ou
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

        // c) Rien à émettre : attendre un paquet entrant, borné à la fois
        //    par l'échéance de `Rtc` et par les prochaines échéances de
        //    média. Corps dans `socket`.
        self.brancher_attente(deadline)
    }
}

#[cfg(test)]
mod tests {
    use str0m::format::Codec;
    use str0m::{Event, Output};

    use super::*;
    use crate::audio::{AudioPacket, AudioSource};
    use crate::transport::fixtures;

    /// Preuve d'intégration pour C1 (cadence) et C2 (drainage) : les tests
    /// ci-dessus valident les fonctions pures, mais la revue demandait une
    /// mesure réelle de cadence. Sans navigateur disponible, on simule le
    /// pair offrant avec un second `Rtc` str0m en loopback UDP — exactement
    /// la forme que le brief attribue au navigateur (piste vidéo recvonly et
    /// deux canaux de données). `Session::run` tourne sur un thread dédié,
    /// comme en production, pendant que ce test pilote le pair et compte les
    /// images vidéo reçues sur une fenêtre fixe après connexion.
    ///
    /// Avant le correctif de C1, l'attente entre deux images valait jusqu'à
    /// l'échéance que réclame `Rtc` (jusqu'à 1 s, imposée par les timers
    /// RTCP/statistiques) au lieu d'être bornée par `next_frame_at` : ce test
    /// aurait alors mesuré environ 1 image/s au lieu de ~60.
    #[test]
    fn atteint_la_cadence_video_visee_avec_un_pair_local() {
        use std::thread;
        use str0m::change::SdpAnswer;
        use str0m::media::{Direction, MediaKind};

        /// Source audio de test : rend un paquet toutes les 10 ms au plus
        /// tôt, avec un `pts_48k` qui avance de 480 (une trame de 10 ms) à
        /// chaque paquet rendu — comme le ferait `WindowsAudioSource`
        /// (`PacketRing` alimenté par un fil de capture cadencé, jamais
        /// disponible en continu). La charge utile n'a pas besoin d'être un
        /// Opus valide : ce test vérifie que la `Session` ACHEMINE les
        /// paquets jusqu'au pair, pas ce qu'un décodeur en ferait.
        ///
        /// **Constaté pendant l'écriture de ce test (ronde de correction
        /// 1)** : une première version rendait un paquet à CHAQUE appel, sans
        /// pacage. La branche `a3` passant avant la branche `b` (par
        /// construction, voir plus haut), un flux audio en continu
        /// affamait totalement la vidéo — `video_count` retombait à 0 sur
        /// toute la fenêtre de mesure. Ce n'est pas un défaut de la source
        /// réelle (`PacketRing`, bornée à 10 paquets et alimentée par un fil
        /// séparé au rythme de la capture, ne peut pas rendre en continu),
        /// mais un artefact d'une source de test irréaliste. Le pacage à
        /// 10 ms ci-dessous restaure un comportement fidèle à
        /// `WindowsAudioSource` : la plupart des appels à `next_packet`
        /// rendent `None`, exactement comme en production.
        struct DummyAudioSource {
            next_pts_48k: u64,
            next_due: Instant,
        }

        impl AudioSource for DummyAudioSource {
            fn next_packet(&mut self) -> Option<AudioPacket> {
                let now = Instant::now();
                if now < self.next_due {
                    return None;
                }
                self.next_due += Duration::from_millis(10);
                let pts_48k = self.next_pts_48k;
                self.next_pts_48k += 480;
                Some(AudioPacket {
                    data: vec![0xF8, 0xFF, 0xFE],
                    pts_48k,
                    captured_at: now,
                })
            }
        }

        let local_ip = fixtures::local_ip();
        let source = Box::new(fixtures::video_test_source());

        let mut session = Session::new(source, local_ip, Instant::now(), 12_000_000).expect("session");

        // Pair « navigateur » minimal : un second `Rtc`, offrant, avec une
        // piste vidéo recvonly et les deux canaux de données.
        let (peer_socket, peer_addr, mut peer_rtc) = fixtures::local_peer(local_ip, true);

        let mut api = peer_rtc.sdp_api();
        api.add_media(MediaKind::Video, Direction::RecvOnly, None, None, None);
        // Non lié à une variable lue plus loin : le décompte plus bas
        // distingue audio et vidéo par codec, pas par `mid` (voir plus bas).
        api.add_media(MediaKind::Audio, Direction::RecvOnly, None, None, None);
        api.add_channel("control".to_string());
        api.add_channel("input".to_string());
        let (offer, pending) = api.apply().expect("offre non vide");

        let answer_sdp = session.accept_offer(&offer.to_sdp_string()).expect("offre acceptée");
        let answer = SdpAnswer::from_sdp_string(&answer_sdp).expect("réponse SDP valide");
        peer_rtc
            .sdp_api()
            .accept_answer(pending, answer)
            .expect("réponse acceptée par le pair");

        // Ronde de correction 1 (revue) : la première version de ce test
        // faisait écrire le PAIR lui-même sur `audio_mid`, ce qui ne passait
        // jamais par `Session::write_audio` ni par la branche `a3` — la
        // suppression pure et simple de cette branche aurait laissé ce test
        // vert (constaté, voir le rapport de tâche). Ce qui doit réellement
        // être prouvé : une `Session` munie d'une source audio
        // (`set_audio_source`) ÉMET des paquets Opus que le pair reçoit. Le
        // décompte, plus bas, distingue les paquets audio des paquets vidéo
        // par leur codec (`Codec::Opus` vs `Codec::H264`), pas par leur
        // `mid` : `audio_mid` n'a donc plus besoin d'être lu après la
        // négociation SDP.
        session.set_audio_source(Box::new(DummyAudioSource {
            next_pts_48k: 0,
            next_due: Instant::now(),
        }));

        // La session tourne sur un thread dédié, comme en production (voir
        // `main.rs` / `tokio::task::spawn_blocking`). Le thread n'est pas
        // rejoint : `Session::run` ne se termine qu'à la détection d'une
        // déconnexion ICE (délai de plusieurs secondes), ce qui ralentirait
        // ce test sans rien y ajouter. Le processus de test se termine de
        // toute façon en fin de suite ; le thread ne fuit pas au-delà.
        thread::spawn(move || {
            let mut on_input = |_| {};
            let mut on_control = |_| {};
            let _ = session.run(&mut on_input, &mut on_control);
        });

        // Boucle du pair : pilote son propre `Rtc` (STUN, ACKs DTLS...) et
        // compte les images vidéo ET les paquets audio reçus pendant une
        // fenêtre fixe démarrée à la connexion (pas avant : le temps de
        // poignée de main ICE/DTLS ne doit pas être compté contre la cadence
        // mesurée).
        //
        // Ronde de correction 1 (revue) : `media_count` comptait auparavant
        // tout `Event::MediaData` sous le nom d'« images vidéo ». Une fois la
        // source audio de test posée sur `Session` (ci-dessus), l'assertion
        // de cadence vidéo aurait aussi compté des paquets audio et serait
        // devenue fausse (silencieusement, sans jamais échouer pour la
        // mauvaise raison qu'un décompte trop haut). Les deux compteurs sont
        // désormais séparés par codec (`data.params.spec().codec`), pas par
        // `mid` — un paquet Opus reste un paquet Opus quel que soit le `mid`
        // qui le porte.
        let hard_deadline = Instant::now() + Duration::from_secs(10);
        let measure_window = Duration::from_secs(2);
        let mut connected_at: Option<Instant> = None;
        let mut video_count = 0usize;
        let mut audio_count = 0usize;

        loop {
            let now = Instant::now();
            if now >= hard_deadline {
                panic!("le pair local ne s'est jamais connecté dans le délai imparti");
            }
            if let Some(connected_at) = connected_at {
                if now >= connected_at + measure_window {
                    break;
                }
            }

            match peer_rtc.poll_output().expect("poll_output du pair") {
                Output::Timeout(t) => {
                    let cap = match connected_at {
                        Some(c) => hard_deadline.min(c + measure_window),
                        None => hard_deadline,
                    };
                    let wait = t.saturating_duration_since(now).min(cap.saturating_duration_since(now));
                    if fixtures::poll_peer_socket(&mut peer_rtc, &peer_socket, peer_addr, now, wait) {
                        continue;
                    }
                }
                Output::Transmit(t) => {
                    let _ = peer_socket.send_to(&t.contents, t.destination);
                }
                Output::Event(Event::Connected) => {
                    connected_at = Some(Instant::now());
                }
                Output::Event(Event::MediaData(data)) => {
                    if connected_at.is_some() {
                        match data.params.spec().codec {
                            Codec::Opus => audio_count += 1,
                            Codec::H264 => video_count += 1,
                            _ => {}
                        }
                    }
                }
                Output::Event(_) => {}
            }
        }

        let per_second = video_count as f64 / measure_window.as_secs_f64();
        eprintln!(
            "cadence mesurée : {video_count} images vidéo et {audio_count} paquets audio reçus en {measure_window:?} ({per_second:.1} images/s)"
        );

        // Preuve de C1 : au rythme voulu (~60 Hz), on attend nettement plus
        // de 10 images par seconde. Le bug de cadence corrigé n'en aurait
        // produit qu'environ une par seconde — le seuil ci-dessous exclut
        // sans ambiguïté ce rythme tout en restant robuste à une machine de
        // test lente ou une CI chargée.
        assert!(
            per_second > 10.0,
            "cadence trop basse : {per_second:.1} images/s (attendu très supérieur à 1/s, la marque du bug de cadence C1)"
        );

        // Preuve de la tâche 8 (ronde de correction 1) : une `Session` munie
        // d'une source audio (`set_audio_source`, plus haut) doit
        // effectivement émettre des paquets Opus que le pair reçoit — pas
        // seulement négocier la piste. Sans la branche `a3` d'`act_on_timeout`
        // (celle qui appelle `write_audio`), ce compteur resterait à zéro :
        // constaté en la retirant temporairement (voir le rapport de tâche).
        assert!(
            audio_count > 0,
            "aucun paquet audio reçu par le pair : la Session, munie d'une source audio, \
             n'a émis aucun paquet Opus (la branche a3 d'act_on_timeout est-elle bien avant b, \
             ou write_audio échoue-t-il silencieusement ?)"
        );
    }
}
