//! La trace périodique des compteurs de capture (`SOURCE_TRACE=1`).
//!
//! **Extrait de `fenetre.rs`** (revue de la tâche 11, D9) pour la même raison
//! que `commandes.rs` et `transitions.rs` : le fichier parent est en marge
//! étroite sous le plafond de 500 lignes du projet — ce serait le troisième
//! module enfant sur le même patron. Transposé tel quel : ni les valeurs, ni
//! l'ordre des opérations n'ont changé, seul l'emplacement.

use std::sync::OnceLock;

use crate::windows_source::WindowsSource;

/// `SOURCE_TRACE=1` (présence, valeur quelconque) active la trace périodique
/// des compteurs de capture (`Telemetrie`, voir
/// `windows_source/telemetrie.rs`) — **présence ACTIVE**, à l'inverse de la
/// convention `=0` DÉSACTIVE de `PLEIN_ECRAN`/`AUDIO`/`SUPERVISEUR`/`CAPTEUR` :
/// c'est la convention déjà en vigueur pour cette variable précise avant son
/// déplacement ici (D9, tâche 11). Elle vivait dans `demarrage.rs`, côté
/// ENFANT, qui n'a plus de `WindowsSource` depuis D4 : elle n'y affichait que
/// des zéros, et rien ne lisait les compteurs du CAPTEUR, où ils sont
/// réellement écrits — voir la doc de module de `windows_source/telemetrie.rs`.
///
/// `OnceLock`, même raison que `plein_ecran::actif` : cette fonction est
/// consultée à chaque `PERIODE_COMPTEURS`, et l'environnement ne change pas en
/// cours de processus.
fn trace_source_active() -> bool {
    static ACTIF: OnceLock<bool> = OnceLock::new();
    *ACTIF.get_or_init(|| std::env::var("SOURCE_TRACE").is_ok())
}

/// Sous le span `fenetre{session=…}` posé par D7 : la trace est donc
/// attribuable sans champ supplémentaire, ce qui était impossible tant que
/// les compteurs vivaient dans trois statiques de processus. `None` : la
/// fenêtre dort, l'encodeur et la capture sont relâchés (voir le champ
/// `source` de `Fenetre`).
pub(super) fn tracer_les_compteurs(source: Option<&WindowsSource>) {
    if trace_source_active() {
        if let Some(source) = source {
            let (ticks, capturees, produites) = source.telemetrie.lire();
            tracing::info!(ticks, capturees, produites, "compteurs de capture");
        }
    }
}
