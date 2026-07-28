//! Découpage du flux PCM capté en trames de 10 ms, sur horloge réelle.
//!
//! WASAPI livre des blocs de longueur quelconque, à des instants irréguliers,
//! et **rien du tout** quand aucune application ne joue. Opus, lui, exige des
//! trames de taille exacte, à cadence régulière. Ce module fait le pont.
//!
//! Le principe : l'horloge murale dicte la cadence, le tampon fournit le
//! contenu quand il en a, et du silence sinon. Aucune trame n'est jamais
//! sautée — c'est cette continuité qui rend les RTCP Sender Reports
//! exploitables et empêche le tampon de gigue du navigateur de s'affamer.
//!
//! Ce module ne référence jamais le crate `windows` : il se compile et se teste
//! sous Linux.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::opus::{FRAME_INTERLEAVED, FRAME_SAMPLES, SAMPLE_RATE_HZ};

/// Retard maximal toléré dans le tampon, en trames. Au-delà, les échantillons
/// les plus anciens sont jetés : si WASAPI livre durablement plus vite que le
/// temps réel, le tampon grossirait sans fin et la latence avec lui.
const MAX_BACKLOG_FRAMES: usize = 3;

/// Une trame PCM prête à encoder.
#[derive(Debug, Clone)]
pub struct Frame {
    /// `FRAME_INTERLEAVED` échantillons entrelacés (gauche, droite, ...).
    pub pcm: Vec<i16>,
    /// Horodatage de présentation, en échantillons depuis l'origine.
    pub pts_48k: u64,
    /// Instant réel correspondant à `pts_48k`.
    pub captured_at: Instant,
}

/// Assemble des blocs PCM irréguliers en trames régulières de 10 ms.
pub struct FrameAssembler {
    origin: Instant,
    tampon: VecDeque<i16>,
    /// Échantillons **par canal** déjà émis depuis l'origine. `None` tant que
    /// l'assembleur ne s'est pas ancré (voir `drain_due`).
    emis_par_canal: Option<u64>,
    complements: u64,
    echantillons_jetes: u64,
}

impl FrameAssembler {
    pub fn new(origin: Instant) -> Self {
        Self {
            origin,
            tampon: VecDeque::new(),
            emis_par_canal: None,
            complements: 0,
            echantillons_jetes: 0,
        }
    }

    /// Ajoute des échantillons entrelacés stéréo.
    ///
    /// Borne le retard accumulé : au-delà de `MAX_BACKLOG_FRAMES` trames en
    /// attente, les plus anciens échantillons sont jetés plutôt que de laisser
    /// la latence croître indéfiniment.
    pub fn push(&mut self, pcm: &[i16]) {
        self.tampon.extend(pcm.iter().copied());

        let plafond = MAX_BACKLOG_FRAMES * FRAME_INTERLEAVED;
        if self.tampon.len() > plafond {
            let excedent = self.tampon.len() - plafond;
            self.tampon.drain(..excedent);
            self.echantillons_jetes += excedent as u64;
        }
    }

    /// Rend toutes les trames dues à l'instant `maintenant`.
    ///
    /// Le **premier** appel ne rend rien : il ancre l'assembleur sur le temps
    /// déjà écoulé depuis l'origine. Sans cet ancrage, une initialisation
    /// WASAPI de 500 ms produirait d'un coup 50 trames de silence.
    pub fn drain_due(&mut self, maintenant: Instant) -> Vec<Frame> {
        let ecoules = Self::echantillons_ecoules(self.origin, maintenant);

        let emis = match self.emis_par_canal {
            Some(emis) => emis,
            None => {
                self.emis_par_canal = Some(ecoules);
                return Vec::new();
            }
        };

        let mut sorties = Vec::new();
        let mut position = emis;
        while position + FRAME_SAMPLES as u64 <= ecoules {
            sorties.push(self.former_trame(position));
            position += FRAME_SAMPLES as u64;
        }
        self.emis_par_canal = Some(position);
        sorties
    }

    /// Nombre de trames qui ont dû être complétées par du silence.
    pub fn complements(&self) -> u64 {
        self.complements
    }

    /// Nombre d'échantillons entrelacés jetés pour excès de retard.
    pub fn echantillons_jetes(&self) -> u64 {
        self.echantillons_jetes
    }

    /// Forme une trame à partir du tampon, complétée par du silence si celui-ci
    /// n'a pas de quoi la remplir.
    ///
    /// Le silence est ajouté **en fin** de trame ; les échantillons qui
    /// arriveront ensuite reprennent à la trame suivante. Il en résulte une
    /// discontinuité minuscule, préférable de loin à un trou dans la ligne de
    /// temps.
    fn former_trame(&mut self, pts_par_canal: u64) -> Frame {
        let disponibles = self.tampon.len().min(FRAME_INTERLEAVED);
        let mut pcm: Vec<i16> = self.tampon.drain(..disponibles).collect();
        if pcm.len() < FRAME_INTERLEAVED {
            pcm.resize(FRAME_INTERLEAVED, 0);
            self.complements += 1;
        }
        Frame {
            pcm,
            pts_48k: pts_par_canal,
            captured_at: self.origin + Self::duree_de(pts_par_canal),
        }
    }

    /// Échantillons par canal écoulés entre `origin` et `maintenant`.
    fn echantillons_ecoules(origin: Instant, maintenant: Instant) -> u64 {
        let ecoule = maintenant.saturating_duration_since(origin);
        // En 128 bits : `as_nanos() * 48_000` déborderait un u64 au bout de
        // ~4 jours de session. Même précaution que `next_pts_90k` côté vidéo.
        (ecoule.as_nanos() * SAMPLE_RATE_HZ as u128 / 1_000_000_000) as u64
    }

    /// Durée correspondant à `echantillons` échantillons par canal.
    fn duree_de(echantillons: u64) -> Duration {
        Duration::from_nanos(
            (echantillons as u128 * 1_000_000_000 / SAMPLE_RATE_HZ as u128) as u64,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use crate::opus::CHANNELS;

    /// Durée correspondant à `n` trames de 10 ms.
    fn trames(n: u64) -> Duration {
        Duration::from_millis(n * 10)
    }

    /// `n` échantillons entrelacés valant tous `v`.
    fn bloc(v: i16, n: usize) -> Vec<i16> {
        vec![v; n * CHANNELS]
    }

    #[test]
    fn n_emet_rien_avant_que_la_premiere_trame_ne_soit_due() {
        let origine = Instant::now();
        let mut a = FrameAssembler::new(origine);
        a.push(&bloc(100, FRAME_SAMPLES));
        // Ancrage au premier appel, puis 5 ms plus tard : une demi-trame.
        assert!(a.drain_due(origine + trames(1)).is_empty());
        assert!(a.drain_due(origine + trames(1) + Duration::from_millis(5)).is_empty());
    }

    #[test]
    fn emet_une_trame_pleine_quand_les_echantillons_sont_la() {
        let origine = Instant::now();
        let mut a = FrameAssembler::new(origine);
        a.drain_due(origine); // ancrage à 0
        a.push(&bloc(100, FRAME_SAMPLES));

        let sorties = a.drain_due(origine + trames(1));
        assert_eq!(sorties.len(), 1);
        assert_eq!(sorties[0].pcm.len(), FRAME_INTERLEAVED);
        assert!(sorties[0].pcm.iter().all(|&v| v == 100));
        assert_eq!(sorties[0].pts_48k, 0);
        assert_eq!(a.complements(), 0);
    }

    #[test]
    fn les_horodatages_avancent_de_480_sans_trou() {
        let origine = Instant::now();
        let mut a = FrameAssembler::new(origine);
        a.drain_due(origine);
        a.push(&bloc(100, FRAME_SAMPLES * 3));

        let sorties = a.drain_due(origine + trames(3));
        let pts: Vec<u64> = sorties.iter().map(|f| f.pts_48k).collect();
        assert_eq!(pts, vec![0, 480, 960]);
    }

    #[test]
    fn complete_par_du_silence_quand_rien_n_arrive_et_ne_saute_aucune_trame() {
        // C'est la propriété centrale : sans elle, un bureau silencieux
        // creuserait un trou dans la ligne de temps RTP, et le tampon de
        // gigue du navigateur s'affamerait avant de resynchroniser
        // brutalement — la signature exacte du défaut relevé en recette du
        // jalon 1 sur la vidéo.
        let origine = Instant::now();
        let mut a = FrameAssembler::new(origine);
        a.drain_due(origine);

        let sorties = a.drain_due(origine + trames(3));
        assert_eq!(sorties.len(), 3);
        assert!(sorties.iter().all(|f| f.pcm.iter().all(|&v| v == 0)));
        assert_eq!(
            sorties.iter().map(|f| f.pts_48k).collect::<Vec<_>>(),
            vec![0, 480, 960]
        );
        assert_eq!(a.complements(), 3);
    }

    #[test]
    fn une_trame_partielle_est_completee_par_du_silence_en_fin() {
        let origine = Instant::now();
        let mut a = FrameAssembler::new(origine);
        a.drain_due(origine);
        a.push(&bloc(100, 200)); // 200 échantillons sur 480

        let sorties = a.drain_due(origine + trames(1));
        assert_eq!(sorties.len(), 1);
        let pcm = &sorties[0].pcm;
        assert!(pcm[..200 * CHANNELS].iter().all(|&v| v == 100));
        assert!(pcm[200 * CHANNELS..].iter().all(|&v| v == 0));
        assert_eq!(a.complements(), 1);
    }

    #[test]
    fn l_instant_de_capture_se_deduit_de_l_horodatage() {
        let origine = Instant::now();
        let mut a = FrameAssembler::new(origine);
        a.drain_due(origine);
        a.push(&bloc(100, FRAME_SAMPLES * 2));

        let sorties = a.drain_due(origine + trames(2));
        assert_eq!(sorties[0].captured_at, origine);
        assert_eq!(sorties[1].captured_at, origine + Duration::from_millis(10));
    }

    #[test]
    fn s_ancre_sur_le_temps_ecoule_plutot_que_d_emettre_une_rafale() {
        // L'origine d'horloge est créée avant l'initialisation de WASAPI, qui
        // prend un temps non nul. Sans ancrage paresseux, le premier appel
        // produirait d'un coup toutes les trames écoulées depuis l'origine.
        let origine = Instant::now();
        let mut a = FrameAssembler::new(origine);

        let sorties = a.drain_due(origine + Duration::from_millis(500));
        assert!(
            sorties.is_empty(),
            "le premier appel ancre, il ne rattrape pas : {} trames émises",
            sorties.len()
        );

        // Et la ligne de temps repart bien de la position écoulée, pas de zéro.
        let suivantes = a.drain_due(origine + Duration::from_millis(510));
        assert_eq!(suivantes.len(), 1);
        assert_eq!(suivantes[0].pts_48k, 50 * FRAME_SAMPLES as u64);
    }

    #[test]
    fn jette_les_echantillons_les_plus_anciens_au_dela_du_retard_tolere() {
        let origine = Instant::now();
        let mut a = FrameAssembler::new(origine);
        a.drain_due(origine);

        // MAX_BACKLOG_FRAMES = 3, FRAME_INTERLEAVED = 960
        // Plafond = 2880

        // Première poussée : bien au-delà du plafond
        a.push(&bloc(100, 2000));
        // Tampon : 4000, plafond : 2880, excédent : 1120
        // Jetés : 1120
        assert_eq!(a.echantillons_jetes(), 1120,
            "première poussée jette 1120 échantillons (4000 - 2880)");

        // Deuxième poussée : de nouveau au-delà, avec valeur différente
        a.push(&bloc(-100, 1600));
        // Tampon avant: 2880 (100s), après extend: 6080
        // Excédent: 3200, jetés cumulativement: 1120 + 3200 = 4320
        // Tampon reste: 2880 (les -100s les plus récents)
        assert_eq!(a.echantillons_jetes(), 4320,
            "deuxième poussée jette les 3200 anciens (100s) du tampon");

        // La trame émise doit contenir les -100s (les plus récents conservés),
        // pas les 100s (les plus anciens, maintenant jetés).
        let sorties = a.drain_due(origine + trames(1));
        assert_eq!(sorties.len(), 1, "une seule trame est due après 10 ms");
        assert!(sorties[0].pcm.iter().all(|&v| v == -100),
            "la trame doit contenir les données les plus récentes (-100), pas les anciennes (100)");
    }
}
