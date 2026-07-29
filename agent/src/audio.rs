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
