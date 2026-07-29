//! Contrôleur de congestion : décide du débit et de la résolution d'encodage
//! à partir de ce que le pair rapporte.
//!
//! Aucune dépendance à Windows, à str0m ni au socket — c'est ce qui rend
//! toute la politique testable sur Linux, sans VM et sans réseau. Même
//! raison d'être que `geometry.rs`, `rebuild.rs` et `clock.rs`.

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
/// 4,0 → 2,8 → 1,6 Mb/s), pas mesurée. La tâche 12 la confirme ou la corrige
/// sur le banc netem, et consigne l'ajustement.
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

use std::time::{Duration, Instant};

/// Durée pendant laquelle la condition doit tenir avant de DESCENDRE.
const DELAI_DESCENTE: Duration = Duration::from_secs(2);
/// Durée pendant laquelle la condition doit tenir avant de REMONTER.
///
/// Cinq fois plus long que la descente, et c'est délibéré : une estimation
/// qui oscille autour d'un seuil ferait sinon battre l'encodeur, et chaque
/// battement coûte une reconstruction du type de sortie et une image clé.
/// On dégrade vite pour rester fluide, on restaure lentement pour rester
/// stable.
const DELAI_REMONTEE: Duration = Duration::from_secs(10);
/// Durée minimale entre deux changements de barreau, quelle que soit la
/// condition. Filet contre un aller-retour rapide autour d'un seuil.
const SEJOUR_MINIMAL: Duration = Duration::from_secs(5);

/// Filtre temporel asymétrique sur un indice de barreau.
///
/// Rend `Some(nouvel_indice)` à l'instant précis où un changement est retenu,
/// et `None` sinon. L'appelant n'a rien à mémoriser.
///
/// **À ne pas confondre avec `cursor::Hysteresis`**, qui compte des
/// observations booléennes consécutives : ici le filtre est temporel,
/// asymétrique, et porte sur une échelle ordonnée.
pub struct Hysteresis {
    courant: usize,
    /// Barreau visé de façon continue depuis `vise_depuis`, s'il diffère du
    /// courant.
    vise: Option<(usize, Instant)>,
    /// Instant du dernier changement retenu.
    dernier_changement: Instant,
}

impl Hysteresis {
    pub fn new(barreau_initial: usize, now: Instant) -> Self {
        Self {
            courant: barreau_initial,
            vise: None,
            // Placé de façon à ce que le temps de séjour soit déjà écoulé au
            // démarrage : la toute première adaptation ne doit pas attendre
            // 5 s de plus que sa propre condition.
            dernier_changement: now - SEJOUR_MINIMAL,
        }
    }

    pub fn observer(&mut self, vise: usize, now: Instant) -> Option<usize> {
        if vise == self.courant {
            // Retour au barreau courant : toute intention de changement en
            // cours est annulée.
            self.vise = None;
            return None;
        }

        // Un barreau visé DIFFÉRENT de celui déjà en cours d'observation
        // redémarre le décompte : la condition n'a pas « tenu », elle a
        // changé de cible.
        let depuis = match self.vise {
            Some((precedent, depuis)) if precedent == vise => depuis,
            _ => {
                self.vise = Some((vise, now));
                now
            }
        };

        // Indices croissants = résolutions décroissantes : viser plus grand
        // que le courant, c'est descendre.
        let delai = if vise > self.courant { DELAI_DESCENTE } else { DELAI_REMONTEE };
        if now.duration_since(depuis) < delai {
            return None;
        }
        if now.duration_since(self.dernier_changement) < SEJOUR_MINIMAL {
            return None;
        }

        self.courant = vise;
        self.vise = None;
        self.dernier_changement = now;
        Some(vise)
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

    use std::time::{Duration, Instant};

    /// Instant de référence des tests. Placé loin dans le passé pour que
    /// toute soustraction de durée reste valide.
    fn t0() -> Instant {
        Instant::now() - Duration::from_secs(3600)
    }

    #[test]
    fn descendre_exige_deux_secondes_sous_le_barreau() {
        // Base liée UNE SEULE FOIS : `t0()` rend un instant neuf à chaque
        // appel, et des assertions posées sur des bornes exactes (2,000 s)
        // deviendraient instables à quelques microsecondes près.
        let base = t0();
        let mut h = Hysteresis::new(0, base);

        // Première observation du barreau 1 : le décompte DÉMARRE ici, il ne
        // s'est encore rien écoulé.
        assert_eq!(h.observer(1, base + Duration::from_millis(1900)), None);
        // 1,999 s après le début du décompte : pas encore.
        assert_eq!(h.observer(1, base + Duration::from_millis(3899)), None);
        // 2,000 s pile : on descend.
        assert_eq!(h.observer(1, base + Duration::from_millis(3900)), Some(1));
    }

    #[test]
    fn un_repit_remet_le_compteur_de_descente_a_zero() {
        let base = t0();
        let mut h = Hysteresis::new(0, base);

        assert_eq!(h.observer(1, base + Duration::from_millis(1900)), None);
        // Une seule observation revenue au barreau courant annule le décompte.
        assert_eq!(h.observer(0, base + Duration::from_millis(1950)), None);
        // Le décompte repart de zéro à 3000 ms.
        assert_eq!(h.observer(1, base + Duration::from_millis(3000)), None);
        // 1,9 s après ce nouveau départ : toujours pas.
        assert_eq!(h.observer(1, base + Duration::from_millis(4900)), None);
        // 2,0 s après : cette fois oui.
        assert_eq!(h.observer(1, base + Duration::from_millis(5000)), Some(1));
    }

    #[test]
    fn remonter_exige_dix_secondes_et_non_deux() {
        let base = t0();
        // Départ au barreau 1 : `new` place le dernier changement dans le
        // passé, donc le temps de séjour n'entrave pas ce test.
        let mut h = Hysteresis::new(1, base);

        assert_eq!(h.observer(0, base + Duration::from_millis(2000)), None);
        // 2,0 s pile après le début du décompte : une DESCENTE aurait basculé
        // ici, le seuil étant atteint. Une remontée, non — c'est tout l'objet
        // de ce test.
        assert_eq!(h.observer(0, base + Duration::from_millis(4000)), None);
        // 9,999 s : toujours pas.
        assert_eq!(h.observer(0, base + Duration::from_millis(11_999)), None);
        // 10,000 s pile : on remonte.
        assert_eq!(h.observer(0, base + Duration::from_millis(12_000)), Some(0));
    }

    #[test]
    fn le_temps_de_sejour_bloque_un_second_changement_trop_proche() {
        let base = t0();
        let mut h = Hysteresis::new(0, base);

        // Première descente : décompte démarré à 0, retenu à 2,0 s.
        assert_eq!(h.observer(1, base), None);
        assert_eq!(h.observer(1, base + Duration::from_millis(2000)), Some(1));

        // La condition de descente vers 2 est remplie 2 s plus tard, mais le
        // temps de séjour de 5 s depuis le dernier changement l'interdit.
        assert_eq!(h.observer(2, base + Duration::from_millis(2001)), None);
        assert_eq!(h.observer(2, base + Duration::from_millis(4001)), None);
        // À 7,000 s : 5,0 s de séjour écoulées ET la condition tient depuis
        // 4,999 s. Les deux verrous sont levés.
        assert_eq!(h.observer(2, base + Duration::from_millis(7000)), Some(2));
    }
}
