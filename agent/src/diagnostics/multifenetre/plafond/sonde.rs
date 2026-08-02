//! La sonde minimale : D duplications DXGI, tenues, et rien d'autre.

use anyhow::Result;

// `pub(crate)` et non `pub(super)` : `plafond.rs` réexporte cette fonction
// pour que `multifenetre::aiguiller()` l'appelle en `plafond::sonder(..)`, et
// ce réexport franchit DEUX niveaux (sonde → plafond → multifenetre). Un
// `pub(super)` ici ne rendrait `sonder` visible que dans `plafond`, pas dans
// son propre parent — le réexport de `plafond.rs` échouerait à la
// compilation (« sonder is private, and cannot be re-exported »), constaté en
// caisse jetable avant d'écrire cette fonction ainsi.
pub(crate) fn sonder(_sorties: &[String]) -> Result<()> {
    anyhow::bail!("sonde non implémentée — voir la Task 9 du plan D3")
}
