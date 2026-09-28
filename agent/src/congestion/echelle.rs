//! The rung ladder: the encoding resolutions available for a
//! given source size, and the minimum bitrate each requires.
//!
//! `BPP_MIN` (0.05 bit per pixel per frame) is **kept for lack of
//! proof to the contrary, not confirmed**, and it is coupled to the `fps` of
//! `Config`: the two are recalibrated together. See `CLAUDE.md`,
//! "Controller settings".

/// Successive divisors applied to the source size to form the ladder.
///
/// Four rungs, chosen so that each descent is visible without being
/// abrupt: from 1080p we go to 864p, then 720p, then 540p.
const DIVISEURS: [f32; 4] = [1.0, 1.25, 1.5, 2.0];

/// Minimum bitrate, in bits per pixel per frame, below which a rung
/// becomes ugly.
///
/// **It is THE controller setting.** The starting value is chosen to
/// give a coherent ladder under the 12 Mb/s ceiling at 1080p60 (6.2 →
/// 4.0 → 2.8 → 1.6 Mb/s), not measured.
///
/// **Real outcome (task 12, netem acceptance run):** KEPT, for lack of proof
/// to the contrary — not confirmed by a positive visual inspection. Under
/// `adsl` (8 Mb/s), the full-rung threshold for the captured source was
/// ≈1.11 Mb/s, well under the link bitrate: the descents observed
/// came from the instability of the BWE estimate (see `DELAI_REMONTEE`), not
/// from a badly calibrated threshold. But the criterion that would have made it possible to VALIDATE this
/// value ("the full-resolution picture was visibly acceptable or
/// degraded") assumes a visual judgement that was never made — no
/// screenshot was compared by eye. See
/// `docs/superpowers/plans/2026-07-29-reseau-adaptatif-resultats.md`, §5.
const BPP_MIN: f32 = 0.05;

/// A rung of the ladder: an encoding size and the bitrate below
/// which it stops being watchable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Barreau {
    pub taille: (u32, u32),
    pub min_bps: u32,
}

/// Resolution ladder derived from a source size and a frame rate.
///
/// The ladder contains **at most four rungs**, strictly decreasing in
/// width. Truncation to an even pixel (H.264) can make several
/// divisors converge to the same size for tiny sources — in that case,
/// the ladder merges them, while guaranteeing at least one rung (the
/// floor), which avoids needless preconditions upstream (which do not
/// guarantee a minimum source size).
///
/// The first rung is always the source size (divisor 1.0).
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
                // `& !1`: H.264 requires even dimensions. The same
                // constraint is already applied by `WindowsSource::resize`.
                // `.max(2)` prevents a tiny source from producing a
                // zero dimension, which Media Foundation would refuse.
                let w = (((sw as f32) / d) as u32 & !1).max(2);
                let h = (((sh as f32) / d) as u32 & !1).max(2);
                let pixels = w as u64 * h as u64;
                let min_bps = (pixels * fps as u64) as f32 * BPP_MIN;
                Barreau {
                    taille: (w, h),
                    min_bps: min_bps as u32,
                }
            })
            .collect();

        // Remove the rungs whose size is identical to the previous one.
        // This can happen for tiny sources, due to the
        // even-pixel truncation and the `.max(2)` floor.
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

    /// Index of the highest rung that `disponible_bps` finances.
    ///
    /// Returns the last rung when nothing finances it: the ladder has no
    /// rung below. Observing the shortfall is the role of the
    /// controller, which alone knows we are at the floor.
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
        assert_eq!(
            tailles[0],
            (1920, 1080),
            "le premier barreau est la taille source"
        );
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
        // Sources where the even-pixel truncation can produce duplicates,
        // without this fix. Checks that the ladder removes duplicates and
        // stays strictly decreasing.

        // 8×8 source: the divisors 1.5 and 2.0 would fall back on (4, 4).
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

        // 2×2 source: the four divisors would all fall back on (2, 2).
        // The ladder must have only one rung, the floor, not four
        // duplicates.
        let echelle_2x2 = Echelle::depuis((2, 2), 60);
        let tailles_2x2: Vec<(u32, u32)> =
            echelle_2x2.barreaux().iter().map(|b| b.taille).collect();

        assert_eq!(
            tailles_2x2.len(),
            1,
            "source 2×2 : un seul barreau (plancher)"
        );
        assert_eq!(tailles_2x2[0], (2, 2), "le barreau est le plancher (2, 2)");
    }

    #[test]
    fn le_barreau_finance_est_le_plus_haut_que_le_debit_paie() {
        let echelle = Echelle::depuis((1920, 1080), 60);
        let barreaux = echelle.barreaux();

        // Very wide: rung 0.
        assert_eq!(echelle.barreau_finance(50_000_000), 0);

        // Right at the minimum of rung 0: still rung 0.
        assert_eq!(echelle.barreau_finance(barreaux[0].min_bps), 0);

        // One bit under the minimum of rung 0: we go down one notch.
        assert_eq!(echelle.barreau_finance(barreaux[0].min_bps - 1), 1);

        // Under the minimum of the last rung: we stay at the last one, it is the
        // floor. Declaring the shortfall is the role of the controller
        // (task 5), not that of the ladder.
        assert_eq!(echelle.barreau_finance(0), barreaux.len() - 1);
    }
}
