//! Encodeur Opus pour la piste audio.
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
use ::opus::{Application, Bitrate, Channels, Encoder};

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
const BITRATE_BPS: i32 = 128_000;

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

#[cfg(test)]
mod tests {
    use super::*;

    /// Énergie du signal à `freq` hertz sur le canal gauche, par l'algorithme
    /// de Goertzel.
    ///
    /// Insensible au retard : le codec introduit une latence algorithmique
    /// (120 échantillons mesurés en `LowDelay`), donc une comparaison
    /// échantillon par échantillon avec l'entrée échouerait pour une raison
    /// qui n'a rien à voir avec la fidélité.
    fn energie_a(pcm: &[i16], freq: f64) -> f64 {
        let n = pcm.len() / CHANNELS;
        let w = 2.0 * std::f64::consts::PI * freq / SAMPLE_RATE_HZ as f64;
        let coeff = 2.0 * w.cos();
        let (mut s1, mut s2) = (0.0f64, 0.0f64);
        for i in 0..n {
            let s0 = pcm[i * CHANNELS] as f64 + coeff * s1 - s2;
            s2 = s1;
            s1 = s0;
        }
        (s1 * s1 + s2 * s2 - coeff * s1 * s2).sqrt() / n as f64
    }

    /// `n` échantillons entrelacés stéréo d'une sinusoïde à `freq` hertz,
    /// démarrant à l'échantillon `depuis` pour rester continue d'une trame à
    /// la suivante.
    fn ton(freq: f64, depuis: usize, n: usize) -> Vec<i16> {
        (0..n)
            .flat_map(|i| {
                let phase = (depuis + i) as f64 * 2.0 * std::f64::consts::PI * freq
                    / SAMPLE_RATE_HZ as f64;
                let v = (phase.sin() * 12_000.0) as i16;
                [v, v]
            })
            .collect()
    }

    #[test]
    fn une_trame_de_10_ms_vaut_480_echantillons_par_canal() {
        assert_eq!(FRAME_SAMPLES, 480);
        assert_eq!(FRAME_INTERLEAVED, 960);
    }

    #[test]
    fn refuse_une_trame_de_mauvaise_taille() {
        let mut encodeur = OpusEncoder::new().unwrap();
        let err = encodeur.encode(&vec![0i16; 1000]).unwrap_err();
        assert!(
            err.to_string().contains("960"),
            "le message doit nommer la taille attendue, obtenu : {err}"
        );
    }

    #[test]
    fn un_ton_encode_puis_decode_reste_le_meme_ton() {
        // Vérifier que l'encodeur rend des octets ne prouverait rien : du
        // bruit en rendrait tout autant. On décode en retour et on vérifie
        // que l'énergie reste concentrée sur la fréquence d'origine.
        let mut encodeur = OpusEncoder::new().unwrap();
        // Contrairement aux `use`, les expressions résolvent le chemin `opus::` en parcourant
        // d'abord l'arbre des modules du crate courant. N'y trouvant rien nommé `opus`,
        // elles remontent au prélude (crates externes), d'où le crate `opus`. Pas de `::` requis.
        let mut decodeur =
            opus::Decoder::new(SAMPLE_RATE_HZ, opus::Channels::Stereo).unwrap();

        let mut sortie: Vec<i16> = Vec::new();
        for t in 0..20 {
            let paquet = encodeur
                .encode(&ton(440.0, t * FRAME_SAMPLES, FRAME_SAMPLES))
                .unwrap();
            let mut trame = vec![0i16; FRAME_INTERLEAVED];
            decodeur.decode(&paquet, &mut trame, false).unwrap();
            sortie.extend_from_slice(&trame);
        }

        let a_440 = energie_a(&sortie, 440.0);
        let a_1500 = energie_a(&sortie, 1500.0);
        assert!(
            a_440 > 100.0 * a_1500,
            "l'énergie doit rester concentrée sur 440 Hz : 440 Hz = {a_440:.1}, 1500 Hz = {a_1500:.1}"
        );

        let entree = energie_a(&ton(440.0, 0, 20 * FRAME_SAMPLES), 440.0);
        let rapport = a_440 / entree;
        assert!(
            (0.8..=1.2).contains(&rapport),
            "l'amplitude restituée doit rester proche de l'originale, rapport = {rapport:.3}"
        );
    }

    #[test]
    fn le_silence_prolonge_retombe_a_quelques_octets_par_trame() {
        // DTX met plusieurs trames à converger : les cinq premières valent
        // encore 217 puis 161 octets. Mesurer trop tôt conclurait à tort que
        // DTX ne fonctionne pas. On regarde donc la QUEUE, pas le début.
        let mut encodeur = OpusEncoder::new().unwrap();
        let silence = vec![0i16; FRAME_INTERLEAVED];
        let tailles: Vec<usize> = (0..40)
            .map(|_| encodeur.encode(&silence).unwrap().len())
            .collect();

        let queue = &tailles[35..];
        assert!(
            queue.iter().all(|&t| t <= 8),
            "en régime établi, une trame de silence doit tenir en quelques octets, obtenu : {queue:?}"
        );
    }

    #[test]
    fn le_pourcentage_de_perte_est_borne_et_relu() {
        let mut enc = OpusEncoder::new().expect("encodeur");

        enc.set_packet_loss_perc(0).expect("0 accepté");
        enc.set_packet_loss_perc(25).expect("25 accepté");

        // Hors bornes : borné plutôt que refusé. Le contrôleur borne déjà,
        // mais cette fonction est publique et ne doit pas laisser passer une
        // valeur que libopus rejetterait avec une erreur opaque.
        enc.set_packet_loss_perc(-5).expect("valeur négative bornée");
        enc.set_packet_loss_perc(300).expect("valeur excessive bornée");
    }

    #[test]
    fn une_perte_declaree_change_reellement_l_encodage() {
        // Preuve que le FEC in-band n'est plus inerte.
        //
        // ATTENTION à la direction : ce test ne mesure PAS une augmentation de
        // taille. Sous un débit cible fixe, LBRR ne s'ajoute pas aux octets,
        // il les redistribue — `compute_silk_rate_for_hybrid`
        // (opus_encoder.c:751) emploie des tables de débit différentes selon
        // que le FEC est codé ou non. Les paquets peuvent donc RÉTRÉCIR.
        //
        // Ce qui fait preuve, c'est que la sortie DIFFÈRE : en mode CELT seul
        // (`Application::LowDelay`), elle était bit à bit identique, parce que
        // `decide_fec` (opus_encoder.c:721) rend 0 sans rien regarder d'autre.
        // En mode SILK/hybride, le seul chemin par lequel `packet_loss_perc`
        // influence l'encodage est `decide_fec` -> `LBRR_coded`.
        //
        // Un signal NON silencieux est indispensable : sous DTX, le silence
        // retombe à 1 octet par trame quoi qu'on déclare.
        let pcm: Vec<i16> = (0..FRAME_INTERLEAVED)
            .map(|i| ((i as f32 * 0.05).sin() * 8000.0) as i16)
            .collect();

        let mut sans = OpusEncoder::new().expect("encodeur");
        let mut avec = OpusEncoder::new().expect("encodeur");
        avec.set_packet_loss_perc(20).expect("perte déclarée");

        let mut total_sans = 0usize;
        let mut total_avec = 0usize;
        for _ in 0..100 {
            total_sans += sans.encode(&pcm).expect("encodage").len();
            total_avec += avec.encode(&pcm).expect("encodage").len();
        }

        assert_ne!(
            total_avec, total_sans,
            "sortie identique ({total_sans} octets des deux côtés) : \
             `decide_fec` a pris son retour anticipé, donc aucune redondance \
             LBRR n'est codée — c'est le symptôme du mode CELT seul"
        );
    }

    #[test]
    fn lbrr_est_reellement_decodable() {
        // Test que la redondance LBRR codée est réellement présente et
        // décodable. C'est la preuve sémantique que le FEC est opérant :
        // on encode en déclarant une perte, on prend un paquet en régime
        // établi, et on décode ce paquet avec le drapeau FEC sur un décodeur
        // neuf (sans historique). On mesure l'énergie reconstruite.
        //
        // La stratégie : encoder la même trame 100 fois pour atteindre le
        // régime établi. Prendre le paquet #99. Décoder ce paquet avec FEC
        // sur deux décodeurs neufs : un depuis l'encodeur avec perte déclarée
        // (attend la redondance LBRR), un depuis l'encodeur sans perte
        // (pas de redondance, seulement du bruit de reconstruction).
        //
        // Attendu : énergie reconstruite(avec FEC) >> énergie reconstruite(sans).
        let pcm: Vec<i16> = (0..FRAME_INTERLEAVED)
            .map(|i| ((i as f32 * 0.05).sin() * 8000.0) as i16)
            .collect();

        let mut enc_sans = OpusEncoder::new().expect("encodeur");
        let mut enc_avec = OpusEncoder::new().expect("encodeur");
        enc_avec
            .set_packet_loss_perc(20)
            .expect("perte déclarée");

        // Encoder 100 trames pour atteindre le régime établi.
        let mut paquets_sans = Vec::new();
        let mut paquets_avec = Vec::new();
        for _ in 0..100 {
            paquets_sans.push(enc_sans.encode(&pcm).expect("encodage sans"));
            paquets_avec.push(enc_avec.encode(&pcm).expect("encodage avec"));
        }

        // Prendre le dernier paquet en régime établi.
        let dernier_sans = &paquets_sans[99];
        let dernier_avec = &paquets_avec[99];

        // Décodeur neuf sans historique pour décoder le paquet comme une
        // trame FEC (le décodeur reconstruit à partir de la redondance du
        // paquet SUIVANT, ou simplement tente de masquer la perte).
        let mut dec_pour_sans =
            ::opus::Decoder::new(SAMPLE_RATE_HZ, ::opus::Channels::Stereo)
                .expect("décodeur");
        let mut dec_pour_avec =
            ::opus::Decoder::new(SAMPLE_RATE_HZ, ::opus::Channels::Stereo)
                .expect("décodeur");

        let mut sortie_sans = vec![0i16; FRAME_INTERLEAVED];
        let mut sortie_avec = vec![0i16; FRAME_INTERLEAVED];

        // Décoder avec le drapeau FEC (simule une trame perdue).
        dec_pour_sans
            .decode(dernier_sans, &mut sortie_sans, true)
            .expect("décodage sans avec FEC");
        dec_pour_avec
            .decode(dernier_avec, &mut sortie_avec, true)
            .expect("décodage avec avec FEC");

        // Mesurer l'énergie (somme des carrés normalisée).
        let energie_sans: f64 = sortie_sans
            .iter()
            .map(|&s| (s as f64) * (s as f64))
            .sum::<f64>()
            / (FRAME_INTERLEAVED as f64);
        let energie_avec: f64 = sortie_avec
            .iter()
            .map(|&s| (s as f64) * (s as f64))
            .sum::<f64>()
            / (FRAME_INTERLEAVED as f64);

        eprintln!(
            "Énergie reconstruite : sans FEC = {:.2}, avec FEC = {:.2}",
            energie_sans, energie_avec
        );

        // Attend que la redondance LBRR produise du signal significatif.
        // Si elle est présente, energie_avec >> energie_sans.
        assert!(
            energie_avec > energie_sans,
            "pas de redondance LBRR décodable : \
             énergie sans FEC = {:.2}, énergie avec FEC = {:.2} — \
             le FEC n'a rien apporté à la reconstruction",
            energie_sans, energie_avec
        );
    }
}
