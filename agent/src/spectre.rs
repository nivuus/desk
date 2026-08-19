//! Fréquence dominante d'un bloc d'échantillons — **pur, éprouvé sur l'hôte**.
//!
//! ## Pourquoi ce module existe
//!
//! Ce dépôt a établi, au sous-bloc D7, que **le bon instrument pour juger un
//! son est sa fréquence dominante, jamais un compte d'octets** : une piste
//! dont `bytesReceived` croît peut porter un spectre à −1000 dB. La sonde
//! `AUDIO_PROBE` (`diagnostics/audio.rs`) ne rendait qu'une **crête**, qui
//! distingue « du son » de « rien » mais jamais « MON son » de « un autre
//! son ». C'est exactement la distinction dont la correction « A-bis » a
//! besoin : elle doit montrer qu'un périphérique NOMMÉ porte la tonalité
//! qu'on y a jouée, pendant qu'un autre ne la porte pas.
//!
//! ## Goertzel, pas une FFT
//!
//! On ne cherche pas un spectre complet : on cherche **la** raie dominante
//! d'une tonalité pure, sur une grille de fréquences connue d'avance. Le
//! filtre de Goertzel évalue une seule fréquence en O(n) sans allouer, et
//! balayer quelques dizaines de points coûte moins qu'une FFT — et surtout
//! n'embarque aucune dépendance. La résolution du relevé est celle de la
//! grille, et elle est **rendue avec le résultat** plutôt que supposée.

/// Magnitude au carré d'une fréquence donnée, par le filtre de Goertzel.
///
/// `echantillons` est MONO (voir `mono` ci-dessous). Rend 0 sur un bloc vide,
/// ce qui évite à l'appelant un cas particulier : une magnitude nulle ne peut
/// jamais devenir un maximum strict.
pub fn magnitude(echantillons: &[f32], frequence_hz: f32, taux_hz: f32) -> f32 {
    if echantillons.is_empty() || taux_hz <= 0.0 {
        return 0.0;
    }
    let omega = 2.0 * std::f32::consts::PI * frequence_hz / taux_hz;
    let coefficient = 2.0 * omega.cos();
    let (mut s1, mut s2) = (0.0f32, 0.0f32);
    for &x in echantillons {
        let s0 = x + coefficient * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    // |X(k)|² sans le terme de phase, suffisant pour comparer des raies entre
    // elles — c'est un ARGMAX qu'on cherche, pas une valeur physique.
    s1 * s1 + s2 * s2 - coefficient * s1 * s2
}

/// Réduit un bloc entrelacé stéréo (ce que rend `LoopbackCapture::read`) à du
/// mono flottant. La moyenne des canaux, et non un seul : une tonalité jouée
/// sur un seul canal ne doit pas disparaître.
pub fn mono(entrelace: &[i16], canaux: usize) -> Vec<f32> {
    if canaux == 0 {
        return Vec::new();
    }
    entrelace
        .chunks_exact(canaux)
        .map(|trame| {
            let somme: f32 = trame.iter().map(|&v| v as f32).sum();
            somme / canaux as f32 / i16::MAX as f32
        })
        .collect()
}

/// Ce qu'un relevé de fréquence dominante rend.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dominante {
    /// La fréquence de la grille dont la magnitude est maximale.
    pub frequence_hz: f32,
    /// Sa magnitude, pour que l'appelant puisse juger du rapport au reste.
    pub magnitude: f32,
    /// Le pas de la grille : la résolution du relevé, **rendue plutôt que
    /// supposée**. Sans elle, « 441 Hz » ne dit pas s'il faut lire 440 ou 441.
    pub resolution_hz: f32,
}

/// Cherche la fréquence dominante entre `min_hz` et `max_hz`, sur une grille
/// de pas `pas_hz`.
///
/// Rend `None` quand le bloc est vide, la grille dégénérée, ou que **toute**
/// la bande est à magnitude nulle — un silence n'a pas de dominante, et en
/// inventer une serait exactement le genre de verdict qui ne peut pas échouer.
pub fn dominante(
    mono: &[f32],
    taux_hz: f32,
    min_hz: f32,
    max_hz: f32,
    pas_hz: f32,
) -> Option<Dominante> {
    if mono.is_empty() || pas_hz <= 0.0 || min_hz > max_hz || taux_hz <= 0.0 {
        return None;
    }
    let mut meilleure: Option<Dominante> = None;
    let mut frequence = min_hz;
    while frequence <= max_hz {
        let m = magnitude(mono, frequence, taux_hz);
        if m > meilleure.map_or(0.0, |d: Dominante| d.magnitude) {
            meilleure = Some(Dominante {
                frequence_hz: frequence,
                magnitude: m,
                resolution_hz: pas_hz,
            });
        }
        frequence += pas_hz;
    }
    meilleure
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fabrique une sinusoïde stéréo entrelacée de `frequence` Hz.
    fn sinus(frequence: f32, taux: f32, echantillons: usize, amplitude: f32) -> Vec<i16> {
        (0..echantillons)
            .flat_map(|n| {
                let v = (2.0 * std::f32::consts::PI * frequence * n as f32 / taux).sin();
                let e = (v * amplitude * i16::MAX as f32) as i16;
                [e, e]
            })
            .collect()
    }

    #[test]
    fn une_tonalite_pure_est_retrouvee_a_la_resolution_de_la_grille() {
        let bloc = sinus(440.0, 48_000.0, 24_000, 0.5);
        let d = dominante(&mono(&bloc, 2), 48_000.0, 100.0, 2_000.0, 1.0)
            .expect("une tonalité pure a une dominante");
        assert!(
            (d.frequence_hz - 440.0).abs() <= d.resolution_hz,
            "dominante relevée {} Hz, attendue 440 Hz à ±{} Hz",
            d.frequence_hz,
            d.resolution_hz
        );
    }

    /// **Le test qui rend l'instrument discriminant** : deux tonalités
    /// différentes ne doivent pas rendre le même verdict. Sans lui, une
    /// implémentation qui rendrait toujours `min_hz` passerait le test
    /// ci-dessus si la borne valait 440.
    #[test]
    fn deux_tonalites_differentes_rendent_deux_dominantes_differentes() {
        let grave = dominante(&mono(&sinus(440.0, 48_000.0, 24_000, 0.5), 2), 48_000.0, 100.0, 2_000.0, 1.0).unwrap();
        let aigu = dominante(&mono(&sinus(1_000.0, 48_000.0, 24_000, 0.5), 2), 48_000.0, 100.0, 2_000.0, 1.0).unwrap();
        assert!((grave.frequence_hz - 440.0).abs() <= 1.0);
        assert!((aigu.frequence_hz - 1_000.0).abs() <= 1.0);
        assert_ne!(grave.frequence_hz, aigu.frequence_hz);
    }

    /// Un silence n'a pas de dominante. C'est LE cas que la correction
    /// « A-bis » doit pouvoir distinguer : un périphérique qu'on ne joue pas.
    #[test]
    fn un_silence_n_a_aucune_dominante() {
        assert_eq!(dominante(&vec![0.0; 4_800], 48_000.0, 100.0, 2_000.0, 1.0), None);
        assert_eq!(dominante(&[], 48_000.0, 100.0, 2_000.0, 1.0), None);
    }

    /// La magnitude de la raie porteuse doit dominer NETTEMENT une raie
    /// voisine : sans écart, « dominante » ne voudrait rien dire.
    #[test]
    fn la_raie_porteuse_domine_nettement_ses_voisines() {
        let m = mono(&sinus(440.0, 48_000.0, 24_000, 0.5), 2);
        let porteuse = magnitude(&m, 440.0, 48_000.0);
        let voisine = magnitude(&m, 700.0, 48_000.0);
        assert!(
            porteuse > voisine * 100.0,
            "porteuse {porteuse} contre voisine {voisine} : rapport insuffisant"
        );
    }

    #[test]
    fn le_mono_moyenne_les_canaux_et_ignore_une_trame_incomplete() {
        // Trois valeurs pour deux canaux : la troisième est incomplète.
        let m = mono(&[i16::MAX, 0, 1234], 2);
        assert_eq!(m.len(), 1);
        assert!((m[0] - 0.5).abs() < 1e-3, "moyenne obtenue {}", m[0]);
        assert!(mono(&[1, 2, 3], 0).is_empty());
    }

    /// Une tonalité présente sur un SEUL canal ne doit pas être perdue par le
    /// repliement en mono — elle est seulement deux fois plus faible.
    #[test]
    fn une_tonalite_sur_un_seul_canal_survit_au_repliement() {
        let bloc: Vec<i16> = (0..24_000)
            .flat_map(|n| {
                let v = (2.0 * std::f32::consts::PI * 440.0 * n as f32 / 48_000.0).sin();
                [(v * 0.5 * i16::MAX as f32) as i16, 0]
            })
            .collect();
        let d = dominante(&mono(&bloc, 2), 48_000.0, 100.0, 2_000.0, 1.0).unwrap();
        assert!((d.frequence_hz - 440.0).abs() <= d.resolution_hz);
    }

    /// **Épingle la FORMULE, pas seulement l'argmax.** Une mutation qui
    /// amputait Goertzel de son terme croisé (`- coefficient·s1·s2`) est
    /// restée VERTE sur tous les autres tests : l'argmax survit à une
    /// magnitude fausse, parce qu'il ne dépend que de l'ordre des raies. La
    /// magnitude étant PUBLIÉE dans `Dominante`, un appelant qui la
    /// comparerait à un seuil lirait un nombre faux — d'où ce test.
    ///
    /// Valeur analytique : pour une sinusoïde réelle d'amplitude `A`
    /// exactement sur une raie, sur `N` échantillons, Goertzel rend
    /// `|X(k)|² = (A·N/2)²`. Ici `A = 0,5`, `N = 24 000` — soit 220 périodes
    /// entières de 440 Hz à 48 kHz, donc pas de fuite spectrale.
    #[test]
    fn la_magnitude_suit_la_valeur_analytique_de_goertzel() {
        let m = mono(&sinus(440.0, 48_000.0, 24_000, 0.5), 2);
        let mesuree = magnitude(&m, 440.0, 48_000.0);
        let attendue = (0.5 * 24_000.0 / 2.0f32).powi(2);
        let ecart = (mesuree - attendue).abs() / attendue;
        assert!(
            ecart < 0.02,
            "magnitude mesurée {mesuree}, analytique {attendue}, écart relatif {ecart}"
        );
    }

    #[test]
    fn une_grille_degeneree_ne_rend_rien_plutot_qu_un_verdict_invente() {
        let m = mono(&sinus(440.0, 48_000.0, 4_800, 0.5), 2);
        assert_eq!(dominante(&m, 48_000.0, 100.0, 2_000.0, 0.0), None);
        assert_eq!(dominante(&m, 48_000.0, 2_000.0, 100.0, 1.0), None);
        assert_eq!(dominante(&m, 0.0, 100.0, 2_000.0, 1.0), None);
    }
}
