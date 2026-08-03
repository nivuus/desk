//! `WindowsSource::set_encode_size` : changer la résolution encodée sans
//! toucher ni à la fenêtre, ni à la capture.
//!
//! **Module ENFANT de `windows_source`**, comme `redimensionnement` et pour la
//! même raison : c'est ce qui lui donne accès aux champs privés de
//! `WindowsSource` sans qu'aucun ait à être ouvert en `pub(crate)` (voir le
//! commentaire des champs dans `windows_source.rs`). Extrait de ce fichier-là
//! au sous-bloc D5, parce qu'il est en dette de taille gelée (`CLAUDE.md`) et
//! que le remède du défaut C2 y ajoutait une quinzaine de lignes : l'addition
//! s'accompagne de son extraction, comme la règle l'exige.
//!
//! Rien n'a changé au déplacement hors du remède lui-même, décrit en tête de
//! fonction.

use anyhow::{Context, Result};

use super::WindowsSource;
use crate::encode::H264Encoder;

impl WindowsSource {
    /// Reconstruit l'encodeur à une nouvelle taille de sortie, **sans toucher
    /// à la capture ni à la fenêtre**.
    ///
    /// Media Foundation n'autorise pas le changement de résolution en cours
    /// de route : il faut un encodeur neuf (même contrainte que `resize`, voir
    /// son commentaire). Mais contrairement à `resize`, la capture DXGI reste
    /// vivante — c'est l'entrée du convertisseur, elle n'a pas changé. Aucune
    /// contrainte de duplication DXGI ici, donc aucun besoin de
    /// `rebuild_or_recover`.
    ///
    /// L'horodatage n'est pas réinitialisé : `last_pts_90k` est conservé, le
    /// décodeur du navigateur rejetterait un retour en arrière.
    pub fn set_encode_size(&mut self, width: u32, height: u32) -> Result<()> {
        // La source est définitivement épuisée : `capture` peut valoir `None`
        // pour de bon (voir le commentaire du champ), et `capture_mut()`
        // paniquerait. Une panique ici traverserait `spawn_blocking` et
        // emporterait tout le processus — le transport, lui, sait quoi faire
        // d'une erreur : il garde le barreau courant et poursuit la session
        // jusqu'à sa clôture normale.
        if self.fatal {
            anyhow::bail!("source épuisée : taille d'encodage inchangée");
        }

        // Borne haute ajoutée en revue finale de branche (C1) : la
        // justification qui la rendait jusqu'ici inutile (« l'appelant ne
        // produit jamais de taille supérieure à la source ») était fausse —
        // voir le commentaire de `resize`. Un filet, pas LE correctif : c'est
        // `changer_source` côté contrôleur qui évite normalement de viser une
        // taille trop grande, mais un appelant futur (ou un bug de calibration
        // de l'échelle) ne doit pas pouvoir demander à Media Foundation une
        // sortie plus grande que son entrée.
        let (width, height) = (width.max(2) & !1, height.max(2) & !1);
        let (width, height) = (width.min(self.width), height.min(self.height));
        // Court-circuit AVANT toute destruction, et il n'est pas cosmétique :
        // c'est lui qui évite de détruire puis reconstruire un encodeur pour
        // une taille qu'il sert déjà — notamment à la remontée de barreau qui
        // suit un réveil, où le contrôleur revise la même valeur.
        if (width, height) == self.encoder_mut()?.encode_size() {
            return Ok(());
        }

        let device = self.capture_mut().device().clone();

        // **Détruire AVANT de construire**, et c'est le remède du seul défaut
        // que le sous-bloc D4 avait laissé ouvert. Reconstruire en gardant
        // l'ancien vivant demandait un encodeur de plus le temps de la
        // bascule : à `vivier::PLAFOND_EVEIL` (8) encodeurs vivants, ce
        // transitoire-là est refusé au `SetOutputType` de la MFT NVIDIA
        // (`MF_E_UNSUPPORTED_D3D_TYPE`, `0xC00D6D76`) — **18 refus sur 18**
        // relevés à la seconde recette de D4 à huit fenêtres, contre 3 succès
        // sur 3 à deux fenêtres. L'adaptation par la résolution était donc
        // morte au rang maximal, ne laissant que le débit pour répondre à la
        // congestion. Que détruire libère réellement la place est mesuré :
        // 10 recyclages réussis sur 10, sur quatre exécutions indépendantes
        // (mesure pivot du sous-bloc D5).
        //
        // ⚠️ **Le prix est réel et assumé** : si la construction du neuf
        // échoue, l'ancien n'est plus là. L'ancien comportement gardait alors
        // le barreau courant et la session continuait de diffuser ; celui-ci
        // lui fait perdre sa vidéo. D'où le `self.fatal` ci-dessous, qui fait
        // clore la session par son chemin normal plutôt que de laisser une
        // source muette — et surtout, jamais de panique : elle traverserait
        // `spawn_blocking` et emporterait tout le processus, donc toutes les
        // autres fenêtres avec.
        drop(self.encoder.take());
        let neuf = H264Encoder::new(
            &device,
            (self.width, self.height),
            (width, height),
            self.fps,
            self.bitrate,
        );
        let mut encoder = match neuf {
            Ok(encoder) => encoder,
            Err(erreur) => {
                self.fatal = true;
                return Err(erreur)
                    .context("encodeur neuf refusé après destruction de l'ancien : source épuisée");
            }
        };
        // Un encodeur neuf doit commencer par une image clé : sans elle, le
        // décodeur du navigateur n'a aucun point d'entrée dans le nouveau
        // flux et rend un écran gris jusqu'à la prochaine. Un échec ici laisse
        // `self.encoder` à `None` s'il n'est pas rattrapé : même traitement
        // que ci-dessus, et pour la même raison.
        if let Err(erreur) = encoder.request_keyframe() {
            self.fatal = true;
            return Err(erreur)
                .context("image clé refusée par l'encodeur neuf : source épuisée");
        }

        self.encoder = Some(encoder);
        // L'encodeur neuf n'a rien produit : le budget de sondage de
        // démarrage doit repartir, comme après `resize`.
        self.encoder_warmed_up = false;
        tracing::info!(width, height, "taille d'encodage changée sans toucher à la fenêtre");
        Ok(())
    }
}
