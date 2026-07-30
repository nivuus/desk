//! La piste audio : négociation du payload type Opus, et écriture des
//! paquets vers str0m. L'audio passe AVANT la vidéo dans la liste de
//! priorités de `tick` — une coupure sonore s'entend, une image en retard
//! de 10 ms ne se voit pas.

use std::time::Duration;

use str0m::format::Codec;
use str0m::media::{Frequency, MediaTime, Mid, Pt};

use super::tick::Tick;
use super::Session;
use crate::audio::{AudioPacket, AudioSource};

/// Plafond d'attente quand une piste audio est négociée.
///
/// Les paquets audio arrivent d'un fil de capture indépendant : cette boucle
/// n'a aucun moyen de prévoir leur instant d'arrivée, elle ne peut que se
/// réveiller assez souvent pour ne pas les laisser vieillir. 2 ms pour une
/// cadence de trames de 10 ms — un cinquième de trame de retard au pire.
pub(super) const AUDIO_POLL_INTERVAL: Duration = Duration::from_millis(2);

impl Session {
    /// Fournit la source audio. Sans appel, la session reste muette et la
    /// vidéo fonctionne normalement.
    pub fn set_audio_source(&mut self, source: Box<dyn AudioSource + Send>) {
        self.audio_source = Some(source);
    }

    /// Plafond d'attente de la branche `c` : uniquement quand une source ET
    /// une piste audio existent, sinon rien ne justifie de se réveiller plus
    /// souvent.
    pub(super) fn audio_wait_cap(&self) -> Option<Duration> {
        (self.audio_source.is_some() && self.audio_mid.is_some() && !self.ending)
            .then_some(AUDIO_POLL_INTERVAL)
    }

    /// Branche `a3` de la liste de priorités (voir `tick`) : émet un paquet
    /// audio si la piste est négociée et qu'un paquet attend.
    ///
    /// Pas d'échéance à surveiller ici : le fil de capture dépose dans un
    /// tampon, il suffit de regarder s'il y a quelque chose. Le réveil
    /// régulier vient d'`AUDIO_POLL_INTERVAL`, appliqué en branche `c`.
    ///
    /// Rend `Some(Tick::Continue)` quand elle a conclu le tour — un paquet
    /// écrit est une mutation de `Rtc`, qui doit être suivie du drainage
    /// différé de la branche `a0`. Rend `None` quand il n'y avait rien à
    /// émettre, et n'a alors rien muté : la liste de priorités peut passer à
    /// la branche suivante sans rompre l'invariant de drainage.
    pub(super) fn brancher_audio(&mut self) -> Option<Tick> {
        let (Some(mid), false) = (self.audio_mid, self.ending) else {
            return None;
        };
        let paquet = self
            .audio_source
            .as_mut()
            .and_then(|source| source.next_packet())?;
        if self.write_audio(mid, paquet) {
            self.audio_write_pending_drain = true;
        }
        Some(Tick::Continue)
    }

    /// Sélectionne le type de charge utile Opus négocié pour `mid`.
    ///
    /// Appel séparé de `write_audio` pour que l'emprunt sur `self` via
    /// `Rtc::writer` se termine avant tout appel `&mut self` ultérieur — même
    /// raison que `select_negotiated_h264_pt`.
    fn select_negotiated_opus_pt(&mut self, mid: Mid) -> Option<Pt> {
        let writer = self.rtc.writer(mid)?;
        // Lié à une variable plutôt que renvoyé directement : le type anonyme
        // rendu par `payload_params()` (capturant la durée de vie de
        // `writer`, voir sa signature) resterait sinon un temporaire vivant
        // jusqu'à la fin du bloc, après la destruction de `writer` — rejeté
        // par l'emprunteur (« `writer` does not live long enough ») alors que
        // la valeur finale (`Option<Pt>`, `Copy`) n'emprunte plus rien.
        let pt = writer
            .payload_params()
            .find(|p| p.spec().codec == Codec::Opus)
            .map(|p| p.pt());
        pt
    }

    /// Écrit un paquet Opus sur la piste audio.
    ///
    /// Renvoie `true` si `writer.write()` a réellement empilé le paquet — donc
    /// qu'un drainage différé est nécessaire.
    ///
    /// Contrairement à `write_frame`, un échec d'écriture ne clôt **pas** la
    /// session : un défaut audio ne doit jamais tuer une session vidéo qui
    /// fonctionne.
    pub(super) fn write_audio(&mut self, mid: Mid, packet: AudioPacket) -> bool {
        let Some(pt) = self.select_negotiated_opus_pt(mid) else {
            self.warn_audio_negotiation_once();
            return false;
        };
        let Some(writer) = self.rtc.writer(mid) else {
            self.warn_audio_negotiation_once();
            return false;
        };

        // `captured_at` est l'instant réel correspondant à `pts_48k` : c'est
        // lui qui part dans les RTCP Sender Reports et porte la synchro A/V.
        let rtp_time = MediaTime::new(packet.pts_48k, Frequency::FORTY_EIGHT_KHZ);
        match writer.write(pt, packet.captured_at, rtp_time, packet.data) {
            Ok(()) => true,
            Err(e) => {
                tracing::warn!(erreur = %e, "échec d'écriture audio, paquet abandonné");
                false
            }
        }
    }

    fn warn_audio_negotiation_once(&mut self) {
        if !self.warned_audio_negotiation {
            self.warned_audio_negotiation = true;
            tracing::warn!(
                "aucun type de charge utile Opus négocié : paquets audio jetés (avertissement unique)"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- horloge RTP audio --------------------------------------------------
    //
    // `write_audio` construit `MediaTime::new(pts_48k, Frequency::FORTY_EIGHT_KHZ)` :
    // la fréquence d'horloge RTP du type de charge utile Opus est câblée en
    // dur ici, séparément de `opus::SAMPLE_RATE_HZ`, qui documente pourtant
    // explicitement être « la fréquence d'horloge RTP du type de charge
    // utile Opus ». Rien ne lie ces deux constantes : modifier l'une sans
    // l'autre compilerait sans avertissement et produirait des horodatages
    // RTP faux d'un facteur constant — un défaut de synchronisation
    // silencieux. Ce test échoue si elles divergent.
    #[test]
    fn la_frequence_rtp_audio_correspond_au_taux_d_echantillonnage_opus() {
        assert_eq!(Frequency::FORTY_EIGHT_KHZ.get(), crate::opus::SAMPLE_RATE_HZ);
    }

    #[test]
    fn borne_l_attente_quand_l_audio_est_negocie() {
        // Sans ce plafond, la branche d'attente dormirait jusqu'à l'échéance
        // que réclame `Rtc` — jusqu'à la seconde entière — et traverserait
        // ainsi une centaine de paquets audio dus. C'est le même défaut que
        // C1 côté vidéo, transposé.
        use std::time::Instant;

        use crate::transport::socket::bounded_wait;

        let maintenant = Instant::now();
        let echeance_rtc = maintenant + Duration::from_secs(1);

        let sans_audio = bounded_wait(maintenant, echeance_rtc, None, None);
        assert_eq!(sans_audio, Duration::from_secs(1));

        let avec_audio = bounded_wait(maintenant, echeance_rtc, None, Some(AUDIO_POLL_INTERVAL));
        assert_eq!(avec_audio, AUDIO_POLL_INTERVAL);

        // Le plafond ne doit jamais ALLONGER une attente déjà plus courte.
        let echeance_proche = maintenant + Duration::from_micros(200);
        let court = bounded_wait(
            maintenant,
            echeance_proche,
            None,
            Some(AUDIO_POLL_INTERVAL),
        );
        assert_eq!(court, Duration::from_micros(200));
    }
}
