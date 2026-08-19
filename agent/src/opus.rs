//! Codec Opus des deux pistes audio : l'ENCODEUR de la piste descendante
//! (chantier A, agent → navigateur) et le DÉCODEUR de la piste montante
//! (chantier E, navigateur → agent).
//!
//! Media Foundation n'expose aucun encodeur Opus, et aucun codec que Chrome
//! accepte en WebRTC n'est disponible nativement sous Windows : on passe donc
//! par libopus, dont la source C est vendorée dans `audiopus_sys` et bâtie par
//! cmake à la compilation (voir `scripts/build-agent.sh`).
//!
//! Ce module ne référence jamais le crate `windows` : il se compile et se teste
//! sous Linux.

use anyhow::{bail, Context, Result};
// Le `::` initial force la résolution du crate externe `opus`. Ce module s'appelle
// lui-même `opus` (édition 2021), donc sans le `::`, un `use opus::...` désignerait
// le module courant, pas le crate externe — d'où la compilation échouerait. Le `::` initial
// force le parcours de la racine du crate, d'où le crate externe.
use ::opus::{Application, Bitrate, Channels, Decoder, Encoder};

/// Fréquence d'échantillonnage de la piste audio, en hertz. C'est aussi la
/// fréquence d'horloge RTP du type de charge utile Opus.
pub const SAMPLE_RATE_HZ: u32 = 48_000;

/// Nombre de canaux transmis.
pub const CHANNELS: usize = 2;

/// Durée d'une trame, en millisecondes.
pub const FRAME_MS: u64 = 10;

/// Échantillons par canal dans une trame.
pub const FRAME_SAMPLES: usize = (SAMPLE_RATE_HZ as u64 * FRAME_MS / 1000) as usize;

/// Échantillons entrelacés dans une trame — la taille exacte que `encode`
/// exige.
pub const FRAME_INTERLEAVED: usize = FRAME_SAMPLES * CHANNELS;

/// Débit cible, en bits par seconde.
///
/// `pub` depuis la revue finale de branche (I5) : `transport.rs` la référence
/// pour le budget audio du contrôleur de congestion (`congestion::Config::
/// audio_bps`), plutôt que de dupliquer `128_000` en dur sans lien avec cette
/// constante.
pub const BITRATE_BPS: i32 = 128_000;

/// Borne haute d'un paquet encodé. Une trame de 10 ms à 128 kbps fait ~160
/// octets ; 4 000 laisse toute la marge nécessaire sans jamais tronquer.
const MAX_PACKET_BYTES: usize = 4_000;

/// Encodeur Opus configuré une fois pour toutes.
///
/// **`Application::Audio`** est choisi pour rendre possible le FEC in-band.
/// `OPUS_APPLICATION_RESTRICTED_LOWDELAY` force `MODE_CELT_ONLY`
/// (`opus/src/opus_encoder.c:1349`), dans lequel `decide_fec` retourne zéro
/// (`opus/src/opus_encoder.c:721`) — la redondance LBRR n'existe que dans SILK.
/// Le coût est un pré-délai qui passe de 120 échantillons (2,5 ms) en
/// `RESTRICTED_LOWDELAY` à 312 (6,5 ms) en `Audio`, mesure établie au
/// chantier A. Ces 4 ms supplémentaires se comparent aux ~48 ms de latence
/// vidéo médiane du pipeline — l'audio reste largement en avance sur l'image.
/// L'arbitrage a été tranché en faveur de la résilience réseau.
pub struct OpusEncoder {
    inner: Encoder,
}

impl OpusEncoder {
    pub fn new() -> Result<Self> {
        let mut inner = Encoder::new(SAMPLE_RATE_HZ, Channels::Stereo, Application::Audio)
            .context("création de l'encodeur Opus")?;
        inner
            .set_bitrate(Bitrate::Bits(BITRATE_BPS))
            .context("réglage du débit Opus")?;
        // FEC in-band : le décodeur peut reconstruire une trame perdue à
        // partir de la suivante. Sur un lien quelconque, c'est ce qui évite
        // les micro-coupures audibles.
        inner.set_inband_fec(true).context("activation du FEC in-band")?;
        // DTX : le silence numérique retombe à 1 octet par trame en régime
        // établi (mesuré). Sans lui, il coûterait 3 octets — l'encodage à
        // débit variable dépense déjà peu. Le gain est modeste, le coût nul.
        inner.set_dtx(true).context("activation du DTX")?;
        Ok(Self { inner })
    }

    /// Déclare à l'encodeur le taux de perte observé sur le lien, en pour
    /// cent.
    ///
    /// **C'est ce réglage qui rend le FEC in-band opérant.** `set_inband_fec`
    /// seul ne fait qu'autoriser la redondance LBRR ; libopus ne l'émet que si
    /// une perte non nulle est déclarée. Sans cet appel, le FEC activé à la
    /// construction ne produit rien.
    ///
    /// La valeur est bornée à [0, 100] : libopus refuse le reste avec une
    /// erreur opaque, et l'appelant n'a pas à connaître cette borne.
    pub fn set_packet_loss_perc(&mut self, perc: i32) -> Result<()> {
        self.inner
            .set_packet_loss_perc(perc.clamp(0, 100))
            .context("réglage du taux de perte déclaré à Opus")
    }

    /// Encode exactement une trame de 10 ms.
    ///
    /// `pcm` doit contenir `FRAME_INTERLEAVED` échantillons entrelacés
    /// (gauche, droite, gauche, ...). libopus refuse toute autre taille avec
    /// `BadArg` ; on refuse ici plus tôt, avec un message qui nomme la taille
    /// attendue plutôt que de laisser remonter une erreur opaque.
    pub fn encode(&mut self, pcm: &[i16]) -> Result<Vec<u8>> {
        if pcm.len() != FRAME_INTERLEAVED {
            bail!(
                "trame de {} échantillons entrelacés, {FRAME_INTERLEAVED} attendus",
                pcm.len()
            );
        }
        self.inner
            .encode_vec(pcm, MAX_PACKET_BYTES)
            .context("encodage Opus")
    }
}

/// Échantillons PAR CANAL que porte un paquet Opus, lus de son en-tête (TOC).
///
/// Fonction LIBRE, et c'est délibéré : la boucle de transport a besoin de cette
/// lecture pour construire une `TrameMicro`, et elle n'a aucune raison de
/// posséder un décodeur pour cela — le décodeur vit dans `LecteurMicro`, sur le
/// fil qui décode. `OpusDecoder::echantillons_de` y délègue.
///
/// **Jamais supposé** (spec §7) : Chrome émet du 20 ms, le chantier A du
/// 10 ms, et rien n'oblige un pair à s'y tenir.
pub fn echantillons_de(paquet: &[u8]) -> Result<usize> {
    ::opus::packet::get_nb_samples(paquet, SAMPLE_RATE_HZ)
        .context("lecture de la durée d'un paquet Opus")
}

/// Décodeur Opus de la piste montante (chantier E).
///
/// **Toujours STÉRÉO**, quel que soit le nombre de canaux qu'a réellement
/// encodé le pair : Chrome encode le micro en mono, et libopus duplique alors
/// le canal unique sur les deux sorties. La spec §7 décrivait cette
/// conversion comme un travail à écrire ; elle est faite par la bibliothèque,
/// et `un_flux_mono_ressort_stereo_par_duplication` le VÉRIFIE plutôt que de
/// le supposer.
///
/// **Aucune durée de trame n'est supposée** (spec §7). Chrome émet du 20 ms,
/// le chantier A du 10 ms, et rien n'oblige un pair à s'y tenir : la durée se
/// LIT du paquet (`echantillons_de`) avant toute allocation, et celle du PLC
/// se lit de la dernière trame décodée (`derniere_duree`).
pub struct OpusDecoder {
    inner: Decoder,
}

impl OpusDecoder {
    pub fn new() -> Result<Self> {
        let inner = Decoder::new(SAMPLE_RATE_HZ, Channels::Stereo)
            .context("création du décodeur Opus")?;
        Ok(Self { inner })
    }

    /// Échantillons PAR CANAL que porte ce paquet, lus de son en-tête (TOC).
    ///
    /// **Jamais supposé** : c'est cette fonction qui dimensionne le tampon de
    /// sortie, et une constante à sa place tronquerait toute trame plus
    /// longue que celle qu'on aurait devinée.
    pub fn echantillons_de(&self, paquet: &[u8]) -> Result<usize> {
        echantillons_de(paquet)
    }

    /// Décode une trame normale. Rend le nombre d'échantillons PAR CANAL
    /// écrits ; `sortie` doit en contenir au moins autant fois `CHANNELS`.
    pub fn decoder(&mut self, paquet: &[u8], sortie: &mut [i16]) -> Result<usize> {
        self.inner
            .decode(paquet, sortie, false)
            .context("décodage Opus")
    }

    /// Reconstruit la trame PRÉCÉDENTE à partir de la redondance LBRR portée
    /// par `suivante`.
    ///
    /// C'est le sens du FEC in-band, et il est contre-intuitif : on ne
    /// reconstruit jamais une trame depuis elle-même — on la reconstruit
    /// depuis celle qui la SUIT. D'où la règle du tampon de gigue
    /// (`micro.rs`) : le FEC ne sert que si la suivante est DÉJÀ arrivée.
    pub fn decoder_fec(&mut self, suivante: &[u8], sortie: &mut [i16]) -> Result<usize> {
        self.inner
            .decode(suivante, sortie, true)
            .context("décodage Opus par reconstruction FEC")
    }

    /// Dissimulation de perte : aucun paquet n'est disponible, et pas même sa
    /// suivante. libopus extrapole depuis son état interne.
    ///
    /// **La durée produite est celle de la DERNIÈRE TRAME DÉCODÉE, et c'est
    /// NOUS qui l'imposons — pas libopus.** Le geste n'est pas cosmétique :
    /// `opus_decode` appelé avec un paquet vide prend pour `frame_size` la
    /// TAILLE DU TAMPON qu'on lui tend, et produit donc autant de PLC qu'on
    /// lui offre de place, sans aucun rapport avec ce qui a été décodé avant.
    /// Un appelant qui tendrait un tampon de 40 ms après une trame de 10 ms
    /// obtiendrait 40 ms de dissimulation, et la ligne de temps de `micro.rs`
    /// dériverait de 30 ms à chaque perte.
    ///
    /// Ce défaut a été trouvé par la MUTATION de
    /// `la_dissimulation_rend_la_duree_de_la_derniere_trame` (tâche 3,
    /// step 2) : la première rédaction déléguait la durée à libopus, et le
    /// test passait encore quand on faisait précéder le PLC d'une trame de
    /// 10 ms au lieu de 40. Il ne mesurait rien.
    ///
    /// Rend `Ok(0)` sans rien écrire tant qu'aucune trame n'a été décodée :
    /// il n'y a alors pas de durée à dissimuler, et l'appelant doit rendre du
    /// silence.
    pub fn dissimuler(&mut self, sortie: &mut [i16]) -> Result<usize> {
        let par_canal = self.derniere_duree()?;
        if par_canal == 0 {
            return Ok(0);
        }
        let voulu = par_canal * CHANNELS;
        if sortie.len() < voulu {
            bail!(
                "tampon de dissimulation de {} échantillons, {voulu} attendus                  (durée de la dernière trame décodée : {par_canal} par canal)",
                sortie.len()
            );
        }
        self.inner
            .decode(&[], &mut sortie[..voulu], false)
            .context("dissimulation de perte Opus")
    }

    /// Durée de la dernière trame décodée, en échantillons PAR CANAL.
    ///
    /// Vaut 0 tant que rien n'a été décodé : il n'y a alors pas de durée à
    /// dissimuler, et l'appelant doit rendre du silence plutôt que d'appeler
    /// `dissimuler`.
    pub fn derniere_duree(&mut self) -> Result<usize> {
        let n = self
            .inner
            .get_last_packet_duration()
            .context("lecture de la durée de la dernière trame Opus")?;
        Ok(n as usize)
    }
}

#[cfg(test)]
#[path = "opus/tests.rs"]
mod tests;
