//! Reconstruction-avec-repli d'une ressource dont la fabrication réussie
//! exige d'avoir d'abord relâché une éventuelle instance existante.
//!
//! Motivé par `WindowsSource::resize` (voir son commentaire) : DXGI
//! n'autorise qu'une seule instance vivante d'`IDXGIOutputDuplication` par
//! sortie et par processus à la fois, donc l'ancienne capture doit être
//! relâchée AVANT de tenter d'en construire une nouvelle. Si cette tentative
//! échoue, l'appelant ne doit pas rester sans rien — un `next_frame` appelé
//! juste après ne doit jamais heurter un état absent (c'est exactement le
//! bogue signalé en revue sur la première version du correctif : la ronde
//! précédente vidait le champ, tentait une reconstruction, et si celle-ci
//! échouait via `?`, le champ restait `None` pour de bon, faisant paniquer
//! le prochain appel à `next_frame`).
//!
//! Extrait dans un module sans dépendance Windows pour rester testable sur
//! Linux (voir les tests plus bas) : la logique de décision — essayer,
//! retomber sur un secours, ou déclarer une panne définitive — ne dépend
//! d'aucun type spécifique à `windows-rs`, `DesktopCapture` ou
//! `H264Encoder`.

use anyhow::{Error, Result};

/// Issue d'une tentative de reconstruction avec repli (voir le commentaire
/// de module).
pub enum RebuildOutcome<T, C> {
    /// La fabrique principale a réussi : l'appelant adopte `T` en entier.
    Rebuilt(T),
    /// La fabrique principale a échoué, mais la fabrique de secours a
    /// réussi : l'appelant doit adopter `C` (typiquement un sous-ensemble de
    /// ce que produit la fabrique principale — la capture seule, pas la
    /// région ni l'encodeur, qui restent ceux d'avant et demeurent valides)
    /// et garder inchangé le reste de son état précédent. L'erreur d'origine
    /// est conservée : le repli restaure un état exploitable, il ne doit pas
    /// faire disparaître l'erreur que l'appelant doit journaliser/renvoyer.
    Recovered(C, Error),
    /// Les deux fabriques ont échoué : aucun état exploitable n'a pu être
    /// obtenu. L'appelant ne doit conserver aucune ressource partielle et
    /// doit se déclarer définitivement épuisé plutôt que de laisser un appel
    /// suivant heurter une ressource absente.
    Fatal(Error),
}

/// Tente `primary`. En cas d'échec, tente `recovery` pour retomber sur un
/// état exploitable plutôt que de laisser l'appelant sans rien.
///
/// `recovery` n'est appelée QUE si `primary` échoue : à aucun moment les
/// deux fabriques ne produisent une ressource vivante simultanément, ce qui
/// est précisément la contrainte qui motive ce mécanisme.
pub fn rebuild_or_recover<T, C>(
    primary: impl FnOnce() -> Result<T>,
    recovery: impl FnOnce() -> Result<C>,
) -> RebuildOutcome<T, C> {
    match primary() {
        Ok(value) => RebuildOutcome::Rebuilt(value),
        Err(primary_error) => match recovery() {
            Ok(recovered) => RebuildOutcome::Recovered(recovered, primary_error),
            Err(_recovery_error) => RebuildOutcome::Fatal(primary_error),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;

    #[test]
    fn reussite_de_la_fabrique_principale_produit_rebuilt() {
        let outcome = rebuild_or_recover(|| Ok::<_, Error>(42), || Ok::<_, Error>(0));
        match outcome {
            RebuildOutcome::Rebuilt(v) => assert_eq!(v, 42),
            _ => panic!("attendu Rebuilt"),
        }
    }

    #[test]
    fn echec_principal_avec_secours_reussi_produit_recovered() {
        // Le cas motivant : la fabrique principale (nouvelle capture +
        // région + encodeur) échoue, mais une capture de secours seule
        // réussit — l'appelant doit pouvoir continuer à produire des images
        // avec ses anciens paramètres plutôt que de rester sans capture.
        let outcome = rebuild_or_recover(
            || Err::<i32, _>(anyhow!("échec principal")),
            || Ok::<_, Error>("secours"),
        );
        match outcome {
            RebuildOutcome::Recovered(v, e) => {
                assert_eq!(v, "secours");
                assert!(e.to_string().contains("échec principal"));
            }
            _ => panic!("attendu Recovered"),
        }
    }

    #[test]
    fn echec_des_deux_fabriques_produit_fatal() {
        let outcome = rebuild_or_recover(
            || Err::<i32, _>(anyhow!("échec principal")),
            || Err::<i32, _>(anyhow!("échec de secours")),
        );
        match outcome {
            RebuildOutcome::Fatal(e) => assert!(e.to_string().contains("échec principal")),
            _ => panic!("attendu Fatal"),
        }
    }

    #[test]
    fn la_fabrique_de_secours_n_est_jamais_appelee_si_la_principale_reussit() {
        // Preuve directe de la contrainte qui motive ce mécanisme : ne
        // jamais construire les deux ressources en même temps (DXGI
        // n'autorise qu'une seule instance vivante à la fois pour la sortie
        // dupliquée).
        let mut recovery_called = false;
        let outcome = rebuild_or_recover(
            || Ok::<_, Error>(1),
            || {
                recovery_called = true;
                Ok::<_, Error>(2)
            },
        );
        assert!(matches!(outcome, RebuildOutcome::Rebuilt(1)));
        assert!(!recovery_called);
    }
}
