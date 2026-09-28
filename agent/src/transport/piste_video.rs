//! La piste vidéo : négociation du payload type H.264, ancrage de l'instant
//! de capture sur l'origine d'horloge de la session, et écriture des unités
//! d'accès vers str0m.

use std::time::{Duration, Instant};

use str0m::format::Codec;
use str0m::media::{MediaTime, Mid, Pt};

use super::tick::Tick;
use super::Session;
use crate::clock::instant_from_pts;
use crate::h264::{AccessUnit, CLOCK_RATE_HZ};

/// Cadence d'interrogation de la source vidéo : une toutes les 10 ms (100 Hz).
///
/// Ce n'est PAS la cadence d'émission : `VideoSource::next_frame` ne rend une
/// unité d'accès que s'il y en a une de prête, et rend `None` sinon (cas
/// courant et normal, voir `windows_source`). La cadence d'émission réelle est
/// donc celle de la source, bornée par celle-ci.
///
/// **Pourquoi 100 Hz et non 60 (28/07).** À 60 Hz, la capture ne récupérait
/// que 46 images/s d'un bureau qui, lui, se met à jour à 68,5 Hz — mesuré
/// directement par `DXGI_OUTDUPL_FRAME_INFO::AccumulatedFrames`
/// (`desktop_updates_hz` dans la trace `SOURCE_TRACE`). Chaque tour ne peut
/// remonter qu'une image, quel que soit le nombre de mises à jour que DXGI a
/// fusionnées entre-temps : interroger une source à 68,5 Hz seulement 60 fois
/// par seconde en perd mécaniquement une partie. Interroger plus souvent que
/// la source ne produit lève cette borne sans rien coûter quand il n'y a rien
/// à prendre — `AcquireNextFrame` est appelée avec un délai NUL, donc un tour
/// à vide se résume à un aller-retour DXGI immédiat.
///
/// Le plafond de 60 im/s visé par le jalon reste, lui, celui du contenu : rien
/// ici ne fabrique d'images qui n'existent pas.
///
/// **Vérifié le 28/07** : sonder 5× plus vite (2 ms) ne change rien au débit
/// — `produced_hz` reste à 47,5 pour un bureau à 68,5 Hz. La cadence de
/// sondage n'était donc pas le facteur limitant ; c'était le
/// `MF_MT_FRAME_RATE` annoncé aux MFT (voir `demarrage.rs`).
pub(super) const FRAME_INTERVAL: Duration = Duration::from_millis(10);

/// Vue minimale d'un profil de charge utile négocié, indépendante de str0m
/// pour rester testable sans session RTC réelle : les champs de
/// `str0m::format::PayloadParams` (dont `pt`) sont `pub(crate)` côté str0m,
/// donc impossibles à construire depuis ce crate pour un test.
#[derive(Debug, Clone, Copy)]
struct CandidatePt {
    codec: Codec,
    packetization_mode: Option<u8>,
    pt: Pt,
}

/// Sélectionne le type de charge utile à utiliser pour envoyer du H.264.
///
/// `enable_h264(true)` négocie sept profils (modes de paquetisation 0 et 1,
/// quatre profils de compatibilité) : prendre le premier de la liste, comme
/// le faisait la version initiale, ne garantit rien sur ce que produira
/// l'encodeur. On filtre explicitement sur le mode de paquetisation 1
/// (non-interleaved, RFC 6184 §6.2) — le seul que les tâches suivantes
/// produiront. Le profil exact (constrained-baseline, etc.) n'est pas
/// discriminé plus finement ici : ce n'est vérifiable qu'avec un navigateur
/// réel et un encodeur réel, pas avant les tâches 8/11.
fn select_h264_pt(candidates: impl Iterator<Item = CandidatePt>) -> Option<Pt> {
    candidates
        .filter(|p| p.codec == Codec::H264 && p.packetization_mode == Some(1))
        .map(|p| p.pt)
        .next()
}

/// Échéance de la prochaine image, calculée à partir de l'échéance
/// *précédente* plutôt que de l'instant courant, pour ne pas accumuler de
/// dérive : un léger retard sur une image ne retarde pas systématiquement
/// toutes les suivantes. Borné à un intervalle de rattrapage : au-delà, on
/// abandonne le calcul fondé sur `previous` (qui produirait une rafale
/// d'images pour rattraper tout le retard d'un coup) et on repart d'un
/// intervalle après `now`.
pub(super) fn next_frame_deadline(previous: Instant, now: Instant, interval: Duration) -> Instant {
    let candidate = previous + interval;
    if now.saturating_duration_since(candidate) > interval {
        now + interval
    } else {
        candidate
    }
}

impl Session {
    /// Branche `b` de la liste de priorités (voir `tick`) : émet une image
    /// vidéo si son échéance est atteinte et la piste négociée.
    ///
    /// Rend `Some(Tick::Continue)` quand l'échéance était atteinte — le tour
    /// est alors conclu, qu'une image ait été écrite ou non : la tentative
    /// elle-même est l'action du tour, et une écriture réussie est une
    /// mutation de `Rtc` qui doit être suivie du drainage différé de la
    /// branche `a0`. Rend `None` quand l'échéance n'est pas atteinte ou que
    /// la piste n'est pas négociée, sans avoir rien muté.
    pub(super) fn brancher_video(&mut self) -> Option<Tick> {
        let mid = self.video_mid?;
        let now = Instant::now();
        if now < self.next_frame_at {
            return None;
        }
        self.next_frame_at = next_frame_deadline(self.next_frame_at, now, FRAME_INTERVAL);
        match self.source.next_frame() {
            Some(unit) => {
                // `writer.write()` ne fait qu'empiler l'image dans la file
                // interne `to_payload` de str0m — c'est
                // `Rtc::handle_input(Input::Timeout(..))` qui la dépile
                // réellement en paquets RTP (`do_payload`), jamais
                // `poll_output()` seul (voir `session.rs` de str0m).
                // L'appeler ICI serait une seconde mutation dans le même
                // appel à `act_on_timeout`, sans `poll_output` entre les
                // deux — exactement la violation que ce mécanisme doit
                // éviter (ronde de correction 1). On pose donc un drapeau :
                // la PROCHAINE invocation d'`act_on_timeout` le traite en
                // priorité absolue (branche `a0`). La file de charge non vide
                // fait renvoyer une échéance immédiate par `poll_output()`,
                // donc `run()` rappelle aussitôt.
                if self.write_frame(mid, unit) {
                    self.video_write_pending_drain = true;
                }
            }
            None => {
                // Ronde de correction 1 : l'absence de nouvelle image est le
                // cas courant et normal d'une capture en direct (bureau
                // immobile) — pas une fin de session. Seule une source
                // réellement épuisée (fenêtre fermée, erreur non
                // récupérable) le justifie, via `VideoSource::is_exhausted`.
                // `FileSource` ne renvoie jamais `None` et n'atteint donc
                // jamais ce chemin.
                if self.source.is_exhausted() {
                    self.begin_ending("source vidéo épuisée");
                }
            }
        }
        // Compteur de cadence côté enfant, le pendant de celui du capteur —
        // voir `cadence_video.rs` (extrait de ce fichier, tâche 8 : l'ajout
        // dépassait le plafond de 500 lignes de ce fichier).
        self.compter_la_cadence_video();
        Some(Tick::Continue)
    }

    /// Sélectionne le type de charge utile H.264 négocié pour `mid`, s'il y
    /// en a un. Appel séparé de `write_frame` pour que l'emprunt sur `self`
    /// via `Rtc::writer` se termine avant tout appel `&mut self` ultérieur
    /// (le journal d'avertissement, notamment).
    fn select_negotiated_h264_pt(&mut self, mid: Mid) -> Option<Pt> {
        let writer = self.rtc.writer(mid)?;
        select_h264_pt(writer.payload_params().map(|p| CandidatePt {
            codec: p.spec().codec,
            packetization_mode: p.spec().format.packetization_mode,
            pt: p.pt(),
        }))
    }

    /// Instant réel auquel l'image d'horodatage `pts_90k` a été capturée.
    ///
    /// C'est cette valeur que `write_frame` annonce à str0m comme `wallclock`.
    /// Extraite en méthode pour être vérifiable directement : l'écriture
    /// elle-même exige une session négociée, la conversion non.
    fn capture_instant(&self, pts_90k: u64) -> Instant {
        instant_from_pts(self.clock_origin, pts_90k, CLOCK_RATE_HZ as u32)
    }

    /// Écrit une unité d'accès sur la piste vidéo. Mutation émise depuis
    /// l'intérieur de la boucle de `run()` (voir `act_on_timeout`), donc
    /// suivie d'un retour immédiat à `poll_output` — conforme à la règle de
    /// drainage de str0m.
    ///
    /// Renvoie `true` si `writer.write()` a réellement été appelée et a
    /// réussi (donc qu'une entrée a bien été empilée dans `to_payload` et
    /// nécessite le drainage différé — voir `video_write_pending_drain`),
    /// `false` si l'écriture n'a pas eu lieu (négociation incomplète,
    /// piste indisponible) ou a échoué : dans ces deux cas, aucune entrée
    /// n'a été ajoutée à `to_payload`, poser le drapeau de drainage serait
    /// à tort et provoquerait un `handle_input(Timeout)` inutile.
    pub(super) fn write_frame(&mut self, mid: Mid, unit: AccessUnit) -> bool {
        let Some(pt) = self.select_negotiated_h264_pt(mid) else {
            // I4 : négociation incomplète (aucun profil H.264 en mode de
            // paquetisation 1) — sans ce journal, l'image est jetée
            // silencieusement, produisant un écran noir muet indéfiniment
            // sans le moindre indice dans les journaux.
            self.warn_negotiation_once(
                "aucun type de charge utile H.264 négocié (mode de paquetisation 1) : images jetées",
            );
            return false;
        };
        // Le `wallclock` de str0m est « the real world time that corresponds
        // to the MediaTime » — l'instant de CAPTURE, pas celui de l'écriture.
        // Passer `Instant::now()` ici encapsulait tout le délai de capture et
        // d'encodage matériel dans la correspondance annoncée, ce qui restait
        // invisible tant que la vidéo était seule. Avec une piste audio, dont
        // le chemin est bien plus court, l'audio devancerait la vidéo de tout
        // ce délai et la synchro labiale serait fausse par construction.
        //
        // L'horodatage fait l'aller-retour par Media Foundation sans perte
        // (`encode.rs`), donc l'instant de capture se reconstruit exactement
        // depuis l'origine partagée. Calculé avant l'emprunt de `writer` :
        // celui-ci retient `&mut self.rtc`, incompatible avec l'emprunt
        // immuable de `self.clock_origin` qu'exige `capture_instant`.
        let capture_at = self.capture_instant(unit.pts_90k);
        let Some(writer) = self.rtc.writer(mid) else {
            self.warn_negotiation_once("piste vidéo plus accessible en écriture : images jetées");
            return false;
        };
        match writer.write(
            pt,
            capture_at,
            MediaTime::from_90khz(unit.pts_90k),
            unit.data,
        ) {
            Ok(()) => {
                // C'est ICI, et seulement ici, que l'écriture a réellement
                // eu lieu — voir `compter_la_cadence_video`, qui journalise
                // ce compte, jamais un tour de boucle ni un `next_frame` à
                // vide.
                self.unites_video_ecrites += 1;
                true
            }
            Err(e) => {
                // Échec d'écriture applicatif (ex. RID inconnu) : on clôt la
                // session plutôt que de faire remonter l'erreur jusqu'au
                // processus. Seules `Session::new` et `accept_offer` — avant
                // qu'une session n'existe vraiment — justifient de tuer le
                // processus entier.
                tracing::warn!(erreur = %e, "échec d'écriture de l'image, fin de session");
                self.begin_ending("échec d'écriture vidéo");
                false
            }
        }
    }

    fn warn_negotiation_once(&mut self, message: &str) {
        if !self.warned_negotiation {
            self.warned_negotiation = true;
            tracing::warn!(
                message,
                "négociation vidéo incomplète (avertissement unique)"
            );
        }
    }
}

#[cfg(test)]
mod tests;
