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

/// Ce qu'il faut faire d'une trame reçue sur un canal de données.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Destination {
    Entree,
    Controle,
    /// Le canal n'est ni `input` ni `control` — ou celui-là n'est pas encore
    /// ouvert. La trame est **refusée**, jamais interprétée au hasard.
    Ignoree,
}

/// Décide où va une trame, **par son CANAL et non par son drapeau binaire**.
///
/// 🔴 `binaire` N'EST PLUS UN PARAMÈTRE, ET C'EST TOUT LE CORRECTIF. L'ancienne
/// version aiguillait sur lui seul : toute trame binaire, quel que soit son
/// canal, partait dans `InputMessage::decode`. Le label était pourtant
/// disponible dans `Event::ChannelOpen(id, label)` et simplement inutilisé.
/// Désormais chaque canal porte le contrat de sa charge — `input` du binaire,
/// `control` du JSON —, et une trame qui n'arrive par aucun des deux est
/// refusée plutôt que devinée.
///
/// ⚠️ **Aucune trame existante ne change de destination** : aujourd'hui
/// `control` écrit en `false` (`transport/controle.rs`) et `input` est le seul
/// canal binaire d'une `PeerConnection` d'enfant. Ce qui change, c'est ce qui
/// arrive à une trame d'un canal TIERS — hier une entrée souris décodée au
/// hasard, aujourd'hui un refus nommé.
///
/// 🔴 **GÉNÉRIQUE SUR L'IDENTIFIANT, et c'est ce qui la rend éprouvable.**
/// `str0m::channel::ChannelId` ne peut pas être construit hors du crate de
/// str0m (« Deliberately not Deref or From to avoid this Id being created
/// outside of this module ») : une signature qui l'exigerait ne laisserait
/// éprouver que les cas qu'une négociation SDP réelle sait produire. Or le cas
/// le plus important — une trame reçue AVANT le `ChannelOpen` de son canal,
/// c'est-à-dire l'état INITIAL de toute session — n'en fait pas partie.
/// ⚠️ **L'ORDRE DES DEUX COMPARAISONS EST INOBSERVABLE, et c'est MESURÉ, pas
/// supposé** : la mutation qui les intervertit a été jouée et a SURVÉCU aux
/// neuf tests. Elle est ÉQUIVALENTE, et la preuve tient en une phrase — un
/// `Event::ChannelOpen(id, label)` porte UN label, str0m donne un `ChannelId`
/// distinct par flux SCTP, donc `canal_entree` et `canal_controle` ne peuvent
/// jamais porter le même identifiant. Écrire un test qui figerait la priorité
/// épinglerait un comportement inatteignable ; c'est pourquoi il n'y en a pas.
pub(super) fn destination<T: PartialEq>(
    recu: T,
    canal_entree: Option<T>,
    canal_controle: Option<T>,
) -> Destination {
    if canal_entree.is_some_and(|c| c == recu) {
        Destination::Entree
    } else if canal_controle.is_some_and(|c| c == recu) {
        Destination::Controle
    } else {
        Destination::Ignoree
    }
}

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
                // ⚠️ Le champ `direction` est porté par la trace À DESSEIN :
                // avec DEUX m-lines audio, un journal de recette ne permet
                // pas de distinguer les deux pistes sans lui. C'est la leçon
                // « une trace non attribuable coûte une ré-imputation » (D6).
                tracing::info!(
                    mid = ?media.mid,
                    kind = ?media.kind,
                    direction = ?media.direction,
                    "piste négociée"
                );
                // ⚠️ **La direction discrimine, et sans elle ce `match` est un
                // défaut MUET.** Il posait `audio_mid` pour toute piste audio :
                // dès qu'une SECONDE m-line audio existe — le micro du chantier
                // E, `recvonly` de notre côté —, elle écrasait la première, et
                // le son descendant du chantier A partait sur une piste que
                // nous ne pouvons pas émettre. Aucune erreur, aucun `WARN`,
                // juste le silence. Le rouge sémantique qui l'exhibe est versé
                // dans `journaux-micro/tache-7-rouge.txt`.
                //
                // `MediaAdded::direction` est la direction LOCALE : str0m
                // inverse la direction distante à l'acceptation d'une offre
                // (`change/sdp.rs`, `let new_dir = m.direction().invert();`).
                // Vérifié par la MESURE et non par la seule lecture — la
                // sonde 1 (`transport::sonde_montante`) voit le récepteur
                // annoncer `RecvOnly` sur une piste offerte en `SendOnly`.
                use str0m::media::{Direction, MediaKind};
                match (media.kind, media.direction) {
                    (MediaKind::Video, _) => self.video_mid = Some(media.mid),
                    // Le son du chantier A : l'agent ÉMET.
                    (MediaKind::Audio, Direction::SendOnly | Direction::SendRecv) => {
                        self.audio_mid = Some(media.mid)
                    }
                    // Le micro du chantier E : l'agent REÇOIT.
                    (MediaKind::Audio, Direction::RecvOnly) => self.mic_mid = Some(media.mid),
                    // Éteinte par le pair : ni l'une ni l'autre. La ranger
                    // quelque part lui ferait prendre la place d'une piste
                    // vivante.
                    (MediaKind::Audio, Direction::Inactive) => {}
                }
            }
            Event::MediaData(data) => {
                if Some(data.mid) == self.mic_mid {
                    self.deposer_micro(&data);
                }
                // Tout autre `MediaData` est ignoré : l'agent ne reçoit aucun
                // autre média. Ce bras tombait dans le `_ => {}` catch-all ; il
                // est NOMMÉ ici pour que le prochain média entrant ne tombe pas
                // en silence.
                //
                // ⚠️ Un bras catch-all a déjà coûté QUATRE fois dans ce dépôt
                // (`capteur/pont_media.rs`, D5/D6/D7/D8). Celui-ci est bénin —
                // il ignore, il ne tue rien —, mais le nommer coûte trois
                // lignes et évite la cinquième.
            }
            Event::ChannelOpen(id, label) => {
                tracing::info!(%label, "canal de données ouvert");
                // 🔴 LE LABEL EST RETENU POUR LES DEUX CANAUX, plus seulement
                // pour `control`. Sans `input_channel`, `dispatch_channel_data`
                // n'a que le drapeau binaire pour décider, et prend toute trame
                // binaire pour une entrée souris.
                if label == "input" {
                    self.input_channel = Some(id);
                }
                if label == "control" {
                    self.control_channel = Some(id);
                    // I3 : envoyer `ready` ici, pas juste après la réponse
                    // SDP — à ce moment-là SCTP n'est pas encore ouvert,
                    // `control_channel` valait `None`, et le message était
                    // silencieusement perdu. Le client de la tâche 8 attend
                    // ce message pour effacer sa bannière de statut.
                    let (width, height) = self.dimensions;
                    // Le micro (chantier E) : disponible seulement si une
                    // piste montante a été négociée ET qu'un puits est là.
                    let mic = self.micro_disponible();
                    self.queue_control(AgentControl::ready(width, height, mic));
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

    /// La destination d'une trame reçue, décidée par le CANAL et non par le
    /// seul drapeau binaire.
    ///
    /// 🔴 Fonction LIBRE et PURE, et c'est ce qui la rend éprouvable : elle ne
    /// prend pas de `ChannelId`, que str0m interdit délibérément de construire
    /// hors de son crate (« Deliberately not Deref or From to avoid this Id
    /// being created outside of this module »). Sans elle, le cas « une trame
    /// binaire arrive AVANT tout `ChannelOpen` » — c'est-à-dire l'état INITIAL
    /// de toute session — ne serait couvert par rien : aucun montage à pair
    /// local ne peut le produire, str0m émettant toujours `ChannelOpen` en
    /// premier.
    fn dispatch_channel_data(
        &mut self,
        data: &str0m::channel::ChannelData,
        on_input: &mut impl FnMut(InputMessage),
        on_control: &mut impl FnMut(ClientControl),
    ) {
        match destination(data.id, self.input_channel, self.control_channel) {
            Destination::Entree => match InputMessage::decode(&data.data) {
                Ok(message) => on_input(message),
                Err(e) => tracing::warn!(erreur = %e, "message d'entrée invalide"),
            },
            Destination::Controle => {
                match std::str::from_utf8(&data.data).map(serde_json::from_str::<ClientControl>) {
                    Ok(Ok(message)) => {
                        self.memoriser_controle(&message);
                        on_control(message);
                    }
                    Ok(Err(e)) => tracing::warn!(erreur = %e, "message de contrôle invalide"),
                    Err(e) => tracing::warn!(erreur = %e, "contrôle non UTF-8"),
                }
            }
            Destination::Ignoree => {
                // ⚠️ `WARN` et non `debug` : c'est un canal que personne n'a
                // négocié pour ce transport, ou une trame arrivée avant son
                // `ChannelOpen`. Les deux méritent d'être vues.
                tracing::warn!(
                    binaire = data.binary,
                    octets = data.data.len(),
                    "trame reçue sur un canal ni `input` ni `control`, ignorée"
                );
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
            // Écrasement du dernier, comme les deux au-dessus — et pour ce
            // champ-ci le coût n'est PAS le même : un redimensionnement
            // intermédiaire n'a aucun intérêt, un collage perdu en a un. Voir
            // la doc du champ, qui porte le coût et le remède non livré.
            ClientControl::Clipboard { text, .. } => {
                self.pending_clipboard = Some(text.clone());
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
#[path = "evenements/tests.rs"]
mod tests;
