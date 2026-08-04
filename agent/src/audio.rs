//! Types de la piste audio et tampon de transfert entre fils.
//!
//! Ce module ne référence jamais le crate `windows` : il se compile et se teste
//! sous Linux.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Un paquet Opus horodaté, prêt à être écrit sur la piste audio.
#[derive(Debug, Clone)]
pub struct AudioPacket {
    /// Trame Opus encodée.
    pub data: Vec<u8>,
    /// Horodatage de présentation, en unités de 1/48000 s — c'est-à-dire un
    /// compte d'échantillons depuis l'origine d'horloge de la session.
    ///
    /// Contrairement à la vidéo, cette valeur n'est pas une lecture d'horloge
    /// mais un décompte exact : l'audio ne peut pas dériver de lui-même.
    pub pts_48k: u64,
    /// Instant réel auquel ces échantillons ont été captés.
    ///
    /// C'est cette valeur qui part dans `Writer::write` comme `wallclock`, et
    /// donc dans les RTCP Sender Reports : elle porte toute la synchro A/V.
    pub captured_at: Instant,
}

/// Producteur de paquets Opus, pendant audio de `VideoSource`.
pub trait AudioSource {
    /// Paquet suivant, ou `None` si aucun n'est prêt ce tour-ci.
    ///
    /// `None` est le cas courant : la boucle de transport tourne bien plus
    /// vite que les 100 paquets par seconde que produit la capture.
    fn next_packet(&mut self) -> Option<AudioPacket>;

    /// Déclare le taux de perte observé, pour que le FEC in-band Opus
    /// produise réellement de la redondance. Sans effet par défaut.
    fn set_packet_loss_perc(&mut self, _perc: i32) -> anyhow::Result<()> {
        Ok(())
    }

    /// Porte le son, ou se tait, sur ordre de l'arbitrage du capteur.
    ///
    /// Sans effet par défaut : une source qui n'est pas arbitrée émet
    /// toujours.
    fn set_actif(&mut self, _actif: bool) {}

    /// Vrai quand la capture a définitivement cessé et qu'aucun ordre ne la
    /// fera repartir.
    ///
    /// **Existe pour que les traces cessent de mentir.** `set_actif` réussit
    /// toujours — il n'écrit qu'un atomique —, et sans ce témoin
    /// `appliquer_audio` journaliserait `actif=true` pour une fenêtre qui ne
    /// produira plus jamais un paquet. Faux par défaut : une source qui n'a
    /// pas de fil de capture n'a rien qui puisse mourir.
    fn capture_morte(&self) -> bool {
        false
    }
}

/// Nombre d'erreurs de lecture consécutives tolérées par le fil de capture
/// avant qu'il n'abandonne définitivement.
///
/// **Une erreur isolée ne doit pas condamner tout un groupe de PID.** Le fil
/// de capture est le seul producteur de son de sa fenêtre, et sa mort est
/// sans retour : le capteur continue de tenir cette session pour porteuse de
/// son groupe, donc sa voisine reste muette et n'est jamais promue. Or les
/// causes connues d'un refus de lecture WASAPI — changement de périphérique,
/// redémarrage du service audio, changement de format — sont **transitoires**.
/// On retente donc, avec la temporisation croissante ci-dessous, et l'on
/// n'abandonne qu'après `LECTURES_ECHOUEES_MAX` échecs d'affilée.
pub const LECTURES_ECHOUEES_MAX: u32 = 10;

/// Bornes de la temporisation appliquée entre deux tentatives de lecture.
///
/// La base vaut l'intervalle de sondage du fil de capture : au premier échec,
/// retenter ne coûte pas plus cher qu'un tour de boucle normal. Le plafond
/// évite qu'une rafale d'erreurs rendues *immédiatement* — le cas qui compte,
/// `read()` n'attendant alors pas son délai — ne tourne en boucle serrée.
const REPRISE_LECTURE_BASE: std::time::Duration = std::time::Duration::from_millis(5);
const REPRISE_LECTURE_MAX: std::time::Duration = std::time::Duration::from_millis(200);

/// Temporisation à observer après `consecutives` erreurs de lecture d'affilée :
/// croissance exponentielle bornée (5, 10, 20, ... jusqu'à
/// `REPRISE_LECTURE_MAX`).
///
/// Même forme que `transport::socket::recv_error_backoff`, dont c'est le
/// précédent — à ceci près que cette fonction-ci vit dans un module sans
/// `cfg`, donc éprouvable sur l'hôte, là où son appelant
/// (`windows_audio.rs`) ne l'est pas.
///
/// `consecutives = 0` n'a pas de sens (aucune erreur, donc aucune attente) et
/// rend la base, comme `consecutives = 1`.
pub fn temporisation_de_reprise(consecutives: u32) -> std::time::Duration {
    // Borner l'exposant AVANT le décalage : `1u32 << 32` déborderait
    // silencieusement, et `saturating_mul` n'opère que sur la `Duration`, pas
    // sur l'opérande entier qu'on lui passe.
    let exposant = consecutives.saturating_sub(1).min(31);
    REPRISE_LECTURE_BASE
        .saturating_mul(1u32 << exposant)
        .min(REPRISE_LECTURE_MAX)
}

/// Tampon circulaire borné, partagé entre le fil de capture et la boucle de
/// transport.
///
/// **Pourquoi pas un `std::sync::mpsc::sync_channel`** : à saturation, son
/// `try_send` échoue, donc rejette le paquet **nouveau**. Sur une piste temps
/// réel c'est le mauvais bout — le paquet frais est celui qui a de la valeur,
/// le périmé n'en a plus. Ici, c'est le plus **ancien** qui part.
///
/// Le dépôt ne bloque jamais : un dépôt bloquant ferait de la boucle de
/// transport la contrainte du fil de capture, et un blocage côté transport
/// gèlerait la capture WASAPI, dont le tampon interne déborderait à son tour.
#[derive(Debug, Clone)]
pub struct PacketRing {
    file: Arc<Mutex<VecDeque<AudioPacket>>>,
    capacite: usize,
    rejetes: Arc<AtomicU64>,
}

impl PacketRing {
    pub fn new(capacite: usize) -> Self {
        Self {
            file: Arc::new(Mutex::new(VecDeque::with_capacity(capacite.max(1)))),
            capacite: capacite.max(1),
            rejetes: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Dépose un paquet. Ne bloque jamais. À saturation, le paquet le plus
    /// ancien est jeté et le compteur de rejets incrémenté.
    pub fn push(&self, packet: AudioPacket) {
        let mut file = self.verrou();
        while file.len() >= self.capacite {
            file.pop_front();
            self.rejetes.fetch_add(1, Ordering::Relaxed);
        }
        file.push_back(packet);
    }

    /// Retire le paquet le plus ancien, ou `None` si le tampon est vide.
    pub fn pop(&self) -> Option<AudioPacket> {
        self.verrou().pop_front()
    }

    /// Nombre cumulé de paquets jetés faute de place.
    pub fn rejetes(&self) -> u64 {
        self.rejetes.load(Ordering::Relaxed)
    }

    /// Verrou tolérant à l'empoisonnement.
    ///
    /// Un `unwrap()` ici ferait paniquer la boucle de transport parce qu'un
    /// autre fil a paniqué ailleurs — une session vidéo parfaitement saine
    /// mourrait d'un incident audio, ce que les contraintes globales
    /// interdisent. La file reste exploitable dans tous les cas : au pire un
    /// paquet est incomplet, et un paquet audio incomplet ne casse rien.
    fn verrou(&self) -> std::sync::MutexGuard<'_, VecDeque<AudioPacket>> {
        self.file.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- temporisation de reprise de lecture --------------------------------

    #[test]
    fn la_temporisation_de_reprise_croit_puis_se_borne() {
        // Sans croissance, une rafale d'erreurs rendues immédiatement
        // tournerait en boucle serrée ; sans borne, la dixième tentative
        // arriverait des secondes trop tard — et c'est elle qui décide de la
        // mort du fil.
        let un = temporisation_de_reprise(1);
        let deux = temporisation_de_reprise(2);
        let trois = temporisation_de_reprise(3);
        assert!(un < deux && deux < trois, "{un:?} {deux:?} {trois:?}");
        assert_eq!(un, REPRISE_LECTURE_BASE);
        assert_eq!(temporisation_de_reprise(0), REPRISE_LECTURE_BASE);

        assert_eq!(temporisation_de_reprise(u32::MAX), REPRISE_LECTURE_MAX);
        assert!(temporisation_de_reprise(LECTURES_ECHOUEES_MAX) <= REPRISE_LECTURE_MAX);
    }

    #[test]
    fn la_tolerance_totale_reste_de_l_ordre_de_la_seconde() {
        // La borne qui compte n'est pas le nombre d'essais mais le temps
        // qu'ils prennent : trop court, un redémarrage du service audio tue
        // la fenêtre ; trop long, une capture morte reste annoncée vivante.
        let totale: std::time::Duration =
            (1..=LECTURES_ECHOUEES_MAX).map(temporisation_de_reprise).sum();
        assert!(
            totale >= std::time::Duration::from_millis(500)
                && totale <= std::time::Duration::from_secs(3),
            "tolérance totale hors bornes : {totale:?}"
        );
    }

    fn paquet(pts_48k: u64) -> AudioPacket {
        AudioPacket {
            data: vec![0xAA],
            pts_48k,
            captured_at: Instant::now(),
        }
    }

    #[test]
    fn rend_les_paquets_dans_l_ordre_de_depot() {
        let ring = PacketRing::new(4);
        ring.push(paquet(0));
        ring.push(paquet(480));
        assert_eq!(ring.pop().unwrap().pts_48k, 0);
        assert_eq!(ring.pop().unwrap().pts_48k, 480);
        assert!(ring.pop().is_none());
    }

    #[test]
    fn a_saturation_jette_le_plus_ancien_pas_le_plus_recent() {
        // C'est l'arbitrage du §5 de la spec, et l'inverser passerait
        // inaperçu sans ce test : les deux comportements « perdent un
        // paquet », mais l'un fait croître la latence et l'autre non.
        let ring = PacketRing::new(2);
        ring.push(paquet(0));
        ring.push(paquet(480));
        ring.push(paquet(960));

        assert_eq!(
            ring.pop().unwrap().pts_48k,
            480,
            "le paquet le plus ancien (pts 0) doit avoir été jeté"
        );
        assert_eq!(ring.pop().unwrap().pts_48k, 960);
        assert!(ring.pop().is_none());
    }

    #[test]
    fn compte_les_paquets_rejetes() {
        let ring = PacketRing::new(1);
        assert_eq!(ring.rejetes(), 0);
        ring.push(paquet(0));
        assert_eq!(ring.rejetes(), 0);
        ring.push(paquet(480));
        ring.push(paquet(960));
        assert_eq!(ring.rejetes(), 2);
    }

    #[test]
    fn le_depot_ne_bloque_jamais_meme_saturee() {
        // Un dépôt bloquant ferait de la boucle de transport la contrainte du
        // fil de capture ; la capture WASAPI déborderait à son tour. Ce test
        // échouerait par expiration du délai global de cargo test si `push`
        // venait à bloquer.
        let ring = PacketRing::new(2);
        for i in 0..1000 {
            ring.push(paquet(i * 480));
        }
        assert_eq!(ring.rejetes(), 998);
    }

    #[test]
    fn une_copie_partage_le_meme_tampon() {
        // Le fil de capture et la boucle de transport en détiennent chacun
        // une copie : elles doivent voir la même file, pas deux files
        // indépendantes.
        let ring = PacketRing::new(4);
        let copie = ring.clone();
        ring.push(paquet(0));
        assert_eq!(copie.pop().unwrap().pts_48k, 0);
    }
}
