//! Le **SURSIS** : une fenêtre doit SURVIVRE avant de mériter un onglet.
//!
//! 🔴 **MESURÉ SUR LE PRODUIT LE 31 AOÛT 2026, APRÈS QUE LE PROPRIÉTAIRE A
//! RAPPORTÉ « j'ai plein d'onglets qui se sont ouverts ».** Un seul lancement
//! de Steam a fait servir **23 fenêtres en trois minutes**, et le journal donne
//! leurs durées de vie :
//!
//! ```text
//! w-4  21:18:00.668 -> 21:18:00.776   0,11 s
//! w-12 21:18:08.101 -> 21:18:08.235   0,13 s
//! w-19 21:20:37.437 -> 21:20:37.589   0,15 s
//! w-20 21:20:37.587 -> 21:20:37.694   0,11 s
//! w-26 21:20:39.596 -> 21:20:39.703   0,11 s
//! w-27 21:20:40.480 -> 21:20:40.589   0,11 s
//! ```
//!
//! Ce sont les fenêtres transitoires du démarrage de Steam. Chacune a ouvert
//! une pop-up dans le navigateur, et la pop-up survit à la fenêtre Windows.
//!
//! 🔴 **CE N'EST PAS UN DÉFAUT DU CRITÈRE STATIQUE, ET LA MESURE L'A RÉFUTÉ
//! AVANT QU'ON NE LE CORRIGE À TORT.** Un inventaire des fenêtres de Steam en
//! session 1 (`GetWindowTextW`/`GetClassNameW`/styles, une fois Steam
//! stabilisé) rend **26 fenêtres de haut niveau dont UNE SEULE** satisfait
//! `fenetres::merite_une_fenetre` — les 25 autres sont déjà écartées, par
//! propriétaire, par `WS_EX_TOOLWINDOW`, par titre vide ou par invisibilité.
//! **Durcir ce critère aurait donc écarté des fenêtres légitimes sans toucher
//! la cause.** Ce qui distingue les fausses des vraies n'est pas ce qu'elles
//! SONT à l'instant où elles paraissent, c'est qu'elles **ne durent pas**.
//!
//! D'où la règle : on ne juge plus une fenêtre sur son seul instantané de
//! naissance, on lui laisse un sursis et on **redemande** ensuite. Précédent
//! du dépôt : l'anti-rebond d'`APPS_SURVEILLANCE` (G4).
//!
//! ⚠️ **`DUREE_SURSIS` N'EST PAS CALIBRÉE.** C'est un garde-fou de prudence,
//! choisi entre le plus long transitoire mesuré (0,15 s) et le délai qu'un
//! humain remarquerait à l'ouverture d'une fenêtre. **Aucun jugement d'usage
//! ne l'a jugée**, et elle rejoint la liste des constantes non calibrées de
//! `CLAUDE.md`.

use super::table::IdFenetre;
use std::time::{Duration, Instant};

/// Le temps qu'une fenêtre doit survivre avant qu'on lui ouvre un onglet.
///
/// ⚠️ **Non calibrée** — voir l'en-tête. Le coût de la choisir trop GRANDE est
/// une latence perçue à l'ouverture ; trop PETITE, des onglets fantômes.
pub const DUREE_SURSIS: Duration = Duration::from_millis(500);

/// Les fenêtres qui attendent de faire la preuve qu'elles durent.
#[derive(Debug, Default)]
pub struct Sursis {
    attentes: Vec<(IdFenetre, String, Instant)>,
}

impl Sursis {
    pub fn new() -> Self {
        Self::default()
    }

    /// Met une fenêtre en sursis.
    ///
    /// **Idempotente par `IdFenetre`**, comme `Table::fenetre_apparue` qu'elle
    /// précède : le hook peut réémettre `Apparue` pour un même `HWND`, et un
    /// second dépôt ne doit ni dédoubler l'onglet, ni **repousser l'échéance**
    /// du premier (ce qui laisserait une fenêtre bavarde en sursis pour
    /// toujours).
    pub fn deposer(&mut self, fenetre: IdFenetre, titre: String, maintenant: Instant) {
        if self.attentes.iter().any(|(f, _, _)| *f == fenetre) {
            return;
        }
        self.attentes.push((fenetre, titre, maintenant + DUREE_SURSIS));
    }

    /// Retire une fenêtre disparue avant son échéance.
    ///
    /// Rend `true` si elle était bien en sursis — c'est-à-dire **si un onglet
    /// vient d'être évité**, et c'est ce que l'appelant journalise.
    pub fn retirer(&mut self, fenetre: IdFenetre) -> bool {
        let avant = self.attentes.len();
        self.attentes.retain(|(f, _, _)| *f != fenetre);
        self.attentes.len() != avant
    }

    /// Les fenêtres dont le sursis est écoulé, retirées de l'attente.
    ///
    /// ⚠️ **Les rendre ne suffit PAS à les annoncer** : l'appelant doit encore
    /// vérifier qu'elles méritent TOUJOURS une fenêtre (`hook::merite_encore`).
    /// Une fenêtre peut survivre au sursis et avoir entre-temps perdu son
    /// titre, été masquée par DWM, ou reçu un propriétaire.
    pub fn murs(&mut self, maintenant: Instant) -> Vec<(IdFenetre, String)> {
        let (murs, encore): (Vec<_>, Vec<_>) =
            self.attentes.drain(..).partition(|(_, _, echeance)| *echeance <= maintenant);
        self.attentes = encore;
        murs.into_iter().map(|(f, t, _)| (f, t)).collect()
    }

    /// Combien de fenêtres attendent — pour la trace, jamais pour décider.
    pub fn en_attente(&self) -> usize {
        self.attentes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(n: u64) -> IdFenetre {
        IdFenetre(n)
    }

    /// Le cas de production : une fenêtre morte en 110 ms n'atteint jamais
    /// `murs`, donc n'ouvre aucun onglet.
    #[test]
    fn une_fenetre_morte_avant_l_echeance_n_ouvre_aucun_onglet() {
        let t0 = Instant::now();
        let mut s = Sursis::new();
        s.deposer(f(1), "splash".into(), t0);
        assert!(s.retirer(f(1)), "elle était bien en sursis");
        assert!(s.murs(t0 + DUREE_SURSIS).is_empty());
    }

    /// 🔴 **LE TÉMOIN QUI REND LE TEST PRÉCÉDENT DISCRIMINANT** : sans lui,
    /// un `murs` structurellement vide passerait pour un anti-rebond qui
    /// marche.
    #[test]
    fn une_fenetre_qui_survit_est_bien_rendue() {
        let t0 = Instant::now();
        let mut s = Sursis::new();
        s.deposer(f(1), "Steam".into(), t0);
        let murs = s.murs(t0 + DUREE_SURSIS);
        assert_eq!(murs, vec![(f(1), "Steam".to_string())]);
    }

    /// Une seconde trop tôt, rien ne sort — et la fenêtre reste en attente,
    /// elle n'est pas perdue.
    #[test]
    fn avant_l_echeance_rien_ne_sort_et_rien_n_est_perdu() {
        let t0 = Instant::now();
        let mut s = Sursis::new();
        s.deposer(f(1), "Steam".into(), t0);
        assert!(s.murs(t0 + DUREE_SURSIS - Duration::from_millis(1)).is_empty());
        assert_eq!(s.en_attente(), 1);
        assert_eq!(s.murs(t0 + DUREE_SURSIS).len(), 1);
    }

    /// Idempotence, et surtout : **le second dépôt ne repousse pas
    /// l'échéance**. Sans cela, une fenêtre dont le hook réémet `Apparue`
    /// régulièrement resterait en sursis indéfiniment et n'apparaîtrait
    /// jamais.
    #[test]
    fn un_second_depot_ne_repousse_pas_l_echeance() {
        let t0 = Instant::now();
        let mut s = Sursis::new();
        s.deposer(f(1), "Steam".into(), t0);
        s.deposer(f(1), "Steam".into(), t0 + Duration::from_millis(400));
        assert_eq!(s.en_attente(), 1, "aucun doublon");
        assert_eq!(s.murs(t0 + DUREE_SURSIS).len(), 1, "l'échéance est celle du PREMIER dépôt");
    }

    /// Retirer une fenêtre qu'on n'attendait pas ne ment pas : c'est le cas
    /// d'une fenêtre déjà annoncée, dont la disparition regarde la `Table`.
    #[test]
    fn retirer_une_inconnue_rend_faux() {
        let mut s = Sursis::new();
        assert!(!s.retirer(f(42)));
    }

    /// Plusieurs fenêtres, des échéances distinctes : seules les mûres
    /// sortent, dans l'ordre où elles ont été déposées.
    #[test]
    fn seules_les_mures_sortent() {
        let t0 = Instant::now();
        let mut s = Sursis::new();
        s.deposer(f(1), "tot".into(), t0);
        s.deposer(f(2), "tard".into(), t0 + Duration::from_millis(300));
        let murs = s.murs(t0 + DUREE_SURSIS);
        assert_eq!(murs, vec![(f(1), "tot".to_string())]);
        assert_eq!(s.en_attente(), 1);
    }
}
