//! L'échelle de barreaux : les résolutions d'encodage disponibles pour une
//! taille de source donnée, et le débit minimal que chacune exige.
//!
//! `BPP_MIN` (0,05 bit par pixel et par image) est **reconduit faute de
//! preuve du contraire, pas confirmé**, et il est couplé au `fps` de
//! `Config` : les deux se recalibrent ensemble. Voir `CLAUDE.md`,
//! « Réglages du contrôleur ».

/// Diviseurs successifs appliqués à la taille source pour former l'échelle.
///
/// Quatre barreaux, choisis pour que chaque descente soit visible sans être
/// brutale : de 1080p on passe à 864p, puis 720p, puis 540p.
const DIVISEURS: [f32; 4] = [1.0, 1.25, 1.5, 2.0];

/// Débit minimal, en bits par pixel et par image, en dessous duquel un barreau
/// devient laid.
///
/// **C'est LE réglage du contrôleur.** La valeur de départ est choisie pour
/// donner une échelle cohérente sous le plafond de 12 Mb/s en 1080p60 (6,2 →
/// 4,0 → 2,8 → 1,6 Mb/s), pas mesurée.
///
/// **Issue réelle (tâche 12, recette netem) :** RECONDUITE, faute de preuve
/// du contraire — pas confirmée par une inspection visuelle positive. Sous
/// `adsl` (8 Mb/s), le seuil du barreau plein pour la source captée valait
/// ≈1,11 Mb/s, largement sous le débit du lien : les descentes observées
/// venaient de l'instabilité de l'estimation BWE (voir `DELAI_REMONTEE`), pas
/// d'un seuil mal calibré. Mais le critère qui aurait permis de VALIDER cette
/// valeur (« l'image en pleine résolution était visiblement acceptable ou
/// dégradée ») suppose un jugement visuel qui n'a jamais été fait — aucune
/// capture d'écran n'a été comparée à l'œil. Voir
/// `docs/superpowers/plans/2026-07-29-reseau-adaptatif-resultats.md`, §5.
const BPP_MIN: f32 = 0.05;

/// Un barreau de l'échelle : une taille d'encodage et le débit en dessous
/// duquel elle cesse d'être regardable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Barreau {
    pub taille: (u32, u32),
    pub min_bps: u32,
}

/// Échelle de résolutions dérivée d'une taille source et d'une cadence.
///
/// L'échelle contient **au plus quatre barreaux**, strictement décroissants en
/// largeur. La troncature au pixel pair (H.264) peut faire converger plusieurs
/// diviseurs vers la même taille pour les sources minuscules — dans ce cas,
/// l'échelle les fusionne, tout en garantissant au moins un barreau (le
/// plancher), ce qui évite les préconditions inutiles en amont (qui ne
/// garantissent pas une taille source minimale).
///
/// Le premier barreau est toujours la taille source (diviseur 1.0).
#[derive(Debug, Clone)]
pub struct Echelle {
    barreaux: Vec<Barreau>,
}

impl Echelle {
    pub fn depuis(source: (u32, u32), fps: u32) -> Self {
        let (sw, sh) = source;
        let tous_barreaux: Vec<Barreau> = DIVISEURS
            .iter()
            .map(|d| {
                // `& !1` : H.264 exige des dimensions paires. La même
                // contrainte est déjà appliquée par `WindowsSource::resize`.
                // `.max(2)` empêche une source minuscule de produire une
                // dimension nulle, que Media Foundation refuserait.
                let w = (((sw as f32) / d) as u32 & !1).max(2);
                let h = (((sh as f32) / d) as u32 & !1).max(2);
                let pixels = w as u64 * h as u64;
                let min_bps = (pixels * fps as u64) as f32 * BPP_MIN;
                Barreau { taille: (w, h), min_bps: min_bps as u32 }
            })
            .collect();

        // Éliminer les barreaux dont la taille est identique au précédent.
        // Cela peut arriver pour les sources minuscules, en raison de la
        // troncature au pixel pair et du plancher `.max(2)`.
        let mut barreaux: Vec<Barreau> = Vec::new();
        for barreau in tous_barreaux {
            if barreaux.is_empty() || barreau.taille != barreaux.last().unwrap().taille {
                barreaux.push(barreau);
            }
        }

        Self { barreaux }
    }

    pub fn barreaux(&self) -> &[Barreau] {
        &self.barreaux
    }

    /// Indice du barreau le plus haut que `disponible_bps` finance.
    ///
    /// Rend le dernier barreau quand rien ne le finance : l'échelle n'a pas
    /// de barreau en dessous. Constater l'insuffisance est le rôle du
    /// contrôleur, qui seul sait qu'on est au plancher.
    pub fn barreau_finance(&self, disponible_bps: u32) -> usize {
        self.barreaux
            .iter()
            .position(|b| disponible_bps >= b.min_bps)
            .unwrap_or(self.barreaux.len() - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_echelle_a_quatre_barreaux_decroissants_et_pairs() {
        let echelle = Echelle::depuis((1920, 1080), 60);
        let tailles: Vec<(u32, u32)> = echelle.barreaux().iter().map(|b| b.taille).collect();

        // Pour 1920×1080, on attend exactement 4 barreaux (cas nominal).
        assert_eq!(tailles.len(), 4, "quatre barreaux attendus pour 1920×1080");
        assert_eq!(tailles[0], (1920, 1080), "le premier barreau est la taille source");
        for (i, (w, h)) in tailles.iter().enumerate() {
            assert_eq!(w % 2, 0, "barreau {i} : largeur impaire, refusée par H.264");
            assert_eq!(h % 2, 0, "barreau {i} : hauteur impaire, refusée par H.264");
        }
        for i in 1..tailles.len() {
            assert!(
                tailles[i].0 < tailles[i - 1].0,
                "barreau {i} pas plus petit que le précédent : {:?}",
                tailles
            );
        }
    }

    #[test]
    fn echelle_minuscule_sans_doublons() {
        // Sources où la troncature au pixel pair peut produire des doublons,
        // sans cette correction. Vérifie que l'échelle élimine les doublons et
        // reste strictement décroissante.

        // Source 8×8 : les diviseurs 1.5 et 2.0 retomberaient sur (4, 4).
        let echelle_8x8 = Echelle::depuis((8, 8), 60);
        let tailles_8x8: Vec<(u32, u32)> =
            echelle_8x8.barreaux().iter().map(|b| b.taille).collect();

        assert!(tailles_8x8.len() <= 4, "au plus 4 barreaux pour source 8×8");
        assert_eq!(
            tailles_8x8[0],
            (8, 8),
            "le premier barreau est la taille source (8×8)"
        );
        for (i, (w, h)) in tailles_8x8.iter().enumerate() {
            assert_eq!(w % 2, 0, "barreau {i} : largeur impaire");
            assert_eq!(h % 2, 0, "barreau {i} : hauteur impaire");
        }
        for i in 1..tailles_8x8.len() {
            assert!(
                tailles_8x8[i].0 < tailles_8x8[i - 1].0,
                "barreau {i} pas plus petit que le précédent : {:?}",
                tailles_8x8
            );
        }

        // Source 2×2 : les quatre diviseurs retomberaient tous sur (2, 2).
        // L'échelle ne doit avoir qu'un seul barreau, le plancher, pas quatre
        // doublons.
        let echelle_2x2 = Echelle::depuis((2, 2), 60);
        let tailles_2x2: Vec<(u32, u32)> =
            echelle_2x2.barreaux().iter().map(|b| b.taille).collect();

        assert_eq!(tailles_2x2.len(), 1, "source 2×2 : un seul barreau (plancher)");
        assert_eq!(tailles_2x2[0], (2, 2), "le barreau est le plancher (2, 2)");
    }

    #[test]
    fn le_barreau_finance_est_le_plus_haut_que_le_debit_paie() {
        let echelle = Echelle::depuis((1920, 1080), 60);
        let barreaux = echelle.barreaux();

        // Très large : le barreau 0.
        assert_eq!(echelle.barreau_finance(50_000_000), 0);

        // Juste au minimum du barreau 0 : encore le barreau 0.
        assert_eq!(echelle.barreau_finance(barreaux[0].min_bps), 0);

        // Un bit sous le minimum du barreau 0 : on descend d'un cran.
        assert_eq!(echelle.barreau_finance(barreaux[0].min_bps - 1), 1);

        // Sous le minimum du dernier barreau : on reste au dernier, c'est le
        // plancher. Déclarer l'insuffisance est le rôle du contrôleur
        // (tâche 5), pas celui de l'échelle.
        assert_eq!(echelle.barreau_finance(0), barreaux.len() - 1);
    }
}
