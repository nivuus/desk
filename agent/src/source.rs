//! Sources vidéo produisant des unités d'accès H.264 prêtes à être envoyées.

use crate::h264::{group_access_units, AccessUnit, CLOCK_RATE_HZ};
use anyhow::{bail, Result};

/// Producteur d'unités d'accès H.264.
///
/// L'implémentation Windows (capture + encodage) et la source fichier de test
/// se substituent l'une à l'autre derrière ce trait.
pub trait VideoSource {
    /// Unité d'accès suivante, ou `None` si rien n'est prêt ce tour-ci.
    ///
    /// `None` ne signifie **pas** systématiquement « source épuisée » : pour
    /// une capture en direct, l'absence de nouvelle image est le cas courant
    /// et normal (rien n'a changé à l'écran depuis le dernier appel). C'est
    /// `is_exhausted()`, interrogée séparément par l'appelant après un
    /// `None`, qui distingue ce cas normal d'un arrêt définitif.
    fn next_frame(&mut self) -> Option<AccessUnit>;
    /// Dimensions de la vidéo produite, en pixels.
    fn dimensions(&self) -> (u32, u32);
    /// Vrai si la source ne produira plus jamais aucune image (périphérique
    /// disparu, erreur non récupérable...) et que la session doit se clore.
    ///
    /// Uniquement consultée après un `next_frame()` ayant renvoyé `None`.
    /// Par défaut, une source n'est jamais épuisée : c'est le cas exact de
    /// `FileSource`, qui boucle indéfiniment et ne renvoie jamais `None`, et
    /// le cas nominal de `WindowsSource` tant que la fenêtre capturée existe
    /// — un bureau immobile ne doit jamais, à lui seul, clore la session
    /// (voir `transport/piste_video.rs`).
    fn is_exhausted(&self) -> bool {
        false
    }

    /// Redimensionne la source, si elle le permet.
    ///
    /// Par défaut sans effet : une source fichier ignore la demande. La source
    /// Windows, elle, redimensionne la fenêtre et reconstruit sa chaîne.
    fn resize(&mut self, _width: u32, _height: u32) -> anyhow::Result<()> {
        Ok(())
    }

    /// Faux quand la source a définitivement disparu — fenêtre fermée, par
    /// exemple. À distinguer de `is_exhausted`, qui signale l'épuisement d'un
    /// flux fini.
    fn is_alive(&self) -> bool {
        true
    }

    /// Force la production d'une image clé sur la prochaine image produite.
    ///
    /// Le groupe d'images de l'encodeur matériel est ouvert (voir
    /// `encode::configure_rate_control`) : sans appel explicite ici, plus
    /// aucune image clé n'est jamais reproduite après le démarrage ou un
    /// redimensionnement, et la moindre perte de paquet corrompt la vidéo
    /// définitivement jusqu'à reconnexion. Câblé sur `Event::KeyframeRequest`
    /// de str0m dans `transport/evenements.rs`, qui relaie la demande du navigateur
    /// après une perte détectée côté décodeur.
    ///
    /// Par défaut sans effet : `FileSource` rejoue un flux pré-découpé où le
    /// bouclage lui-même repart déjà sur une image clé (voir
    /// `group_access_units`) ; une demande de plus n'aurait rien à changer.
    fn request_keyframe(&mut self) -> Result<()> {
        Ok(())
    }

    /// Change le débit d'encodage sans reconstruire quoi que ce soit.
    ///
    /// Sans effet par défaut : une source fichier n'encode rien.
    fn set_bitrate(&mut self, _bitrate: u32) -> anyhow::Result<()> {
        Ok(())
    }

    /// Change la taille RÉELLEMENT ENCODÉE, sans toucher à la fenêtre
    /// capturée.
    ///
    /// À ne pas confondre avec `resize`, qui redimensionne la vraie fenêtre
    /// Windows parce que l'utilisateur a tiré un bord. Ici la fenêtre ne
    /// bouge pas : seul le flux transporté maigrit, parce que le lien ne
    /// porte plus la pleine résolution.
    ///
    /// Sans effet par défaut.
    fn set_encode_size(&mut self, _width: u32, _height: u32) -> anyhow::Result<()> {
        Ok(())
    }
}

/// Source de test rejouant un fichier H.264 Annex-B en boucle.
///
/// Sert à valider le transport sans dépendre de Windows : le flux est découpé
/// une fois au chargement, puis rejoué indéfiniment avec des horodatages
/// strictement croissants (un décodeur rejetterait un retour en arrière).
#[derive(Debug)]
pub struct FileSource {
    units: Vec<AccessUnit>,
    width: u32,
    height: u32,
    tick_90k: u64,
    index: usize,
    loops: u64,
}

impl FileSource {
    pub fn from_annex_b(data: Vec<u8>, width: u32, height: u32, fps: u32) -> Result<Self> {
        if fps == 0 {
            bail!("le nombre d'images par seconde doit être supérieur à zéro");
        }
        let units = group_access_units(&data, fps);
        if units.is_empty() {
            bail!("flux invalide : aucune unité d'accès trouvée");
        }
        if !units.iter().any(|u| u.is_keyframe) {
            bail!("flux invalide : aucune image clé trouvée");
        }
        Ok(Self {
            tick_90k: CLOCK_RATE_HZ / fps as u64,
            index: 0,
            loops: 0,
            units,
            width,
            height,
        })
    }

    /// Charge un fichier `.264` depuis le disque.
    pub fn from_path(path: &std::path::Path, width: u32, height: u32, fps: u32) -> Result<Self> {
        let data = std::fs::read(path)?;
        Self::from_annex_b(data, width, height, fps)
    }
}

impl VideoSource for FileSource {
    fn next_frame(&mut self) -> Option<AccessUnit> {
        let total = self.units.len() as u64;
        let mut unit = self.units[self.index].clone();
        unit.pts_90k = (self.loops * total + self.index as u64) * self.tick_90k;

        self.index += 1;
        if self.index >= self.units.len() {
            self.index = 0;
            self.loops += 1;
        }
        Some(unit)
    }

    fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construit un flux Annex-B de `frames` images, la première étant une IDR.
    fn flux_de_test(frames: usize) -> Vec<u8> {
        let mut stream = Vec::new();
        for i in 0..frames {
            let nal: Vec<u8> = if i == 0 { vec![0x65, 0x88] } else { vec![0x41, 0x9A] };
            stream.extend_from_slice(&[0, 0, 0, 1]);
            stream.extend_from_slice(&nal);
        }
        stream
    }

    #[test]
    fn expose_ses_dimensions() {
        let source = FileSource::from_annex_b(flux_de_test(2), 1280, 720, 60).unwrap();
        assert_eq!(source.dimensions(), (1280, 720));
    }

    #[test]
    fn rejette_un_flux_sans_image() {
        let err = FileSource::from_annex_b(vec![0xFF, 0xFE], 1280, 720, 60).unwrap_err();
        assert!(err.to_string().contains("aucune unité d'accès"));
    }

    #[test]
    fn rejette_un_flux_sans_image_cle() {
        let stream = {
            let mut s = Vec::new();
            s.extend_from_slice(&[0, 0, 0, 1]);
            s.extend_from_slice(&[0x41, 0x9A]);
            s
        };
        let err = FileSource::from_annex_b(stream, 1280, 720, 60).unwrap_err();
        assert!(err.to_string().contains("aucune image clé"));
    }

    #[test]
    fn rejoue_en_boucle_avec_des_horodatages_croissants() {
        let mut source = FileSource::from_annex_b(flux_de_test(3), 640, 480, 60).unwrap();
        let mut horodatages = Vec::new();
        for _ in 0..7 {
            horodatages.push(source.next_frame().unwrap().pts_90k);
        }
        // 1500 ticks par image à 60 fps ; les horodatages ne redémarrent jamais.
        assert_eq!(horodatages, vec![0, 1500, 3000, 4500, 6000, 7500, 9000]);
    }

    #[test]
    fn la_premiere_image_de_chaque_boucle_est_une_image_cle() {
        let mut source = FileSource::from_annex_b(flux_de_test(3), 640, 480, 60).unwrap();
        assert!(source.next_frame().unwrap().is_keyframe);
        assert!(!source.next_frame().unwrap().is_keyframe);
        assert!(!source.next_frame().unwrap().is_keyframe);
        assert!(source.next_frame().unwrap().is_keyframe); // début de la boucle suivante
    }

    #[test]
    fn charge_le_flux_de_test_reel() {
        let path = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/testsrc.264"));
        let mut source = FileSource::from_path(path, 1280, 720, 60).expect("chargement du flux");
        assert_eq!(source.dimensions(), (1280, 720));

        let first = source.next_frame().unwrap();
        assert!(first.is_keyframe, "la première unité doit être une image clé");
        assert!(first.data.len() > 100, "une image clé réelle n'est pas minuscule");
    }

    #[test]
    fn boucle_avec_300_images_reelles_et_horodatages_strictement_croissants() {
        // Vérification de non-régression sur le bug de sur-découpage : le
        // fichier réel contient 300 images malgré ses 2400 tranches, donc le
        // compteur de boucles doit incrémenter après 300 appels, pas 2400.
        let path = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/testsrc.264"));
        let mut source = FileSource::from_path(path, 1280, 720, 60).expect("chargement du flux");

        // Une boucle complète (300 images), plus la première image du tour suivant.
        let mut horodatages = Vec::with_capacity(301);
        let mut keyframes = Vec::with_capacity(301);
        for _ in 0..301 {
            let unit = source.next_frame().unwrap();
            horodatages.push(unit.pts_90k);
            keyframes.push(unit.is_keyframe);
        }

        assert!(keyframes[0], "la première image du fichier est une IDR");
        assert!(
            keyframes[300],
            "la première image de la boucle suivante doit aussi être une IDR"
        );
        assert!(
            horodatages.windows(2).all(|w| w[0] < w[1]),
            "les horodatages doivent être strictement croissants, y compris au bouclage"
        );
        // 300 images à 1500 ticks (90000 / 60) : l'horodatage au bouclage
        // continue la progression linéaire au lieu de redémarrer à zéro.
        assert_eq!(horodatages[300], 300 * (CLOCK_RATE_HZ / 60));
    }

    #[test]
    fn resize_par_defaut_ignore_la_demande_et_ne_change_pas_les_dimensions() {
        // `FileSource` ne redéfinit pas `resize` : la méthode par défaut du
        // trait doit être un no-op qui réussit, sans jamais toucher aux
        // dimensions de la source fichier (tâche 13, source.rs).
        let mut source = FileSource::from_annex_b(flux_de_test(2), 640, 480, 60).unwrap();
        source.resize(1920, 1080).expect("le no-op par défaut ne doit jamais échouer");
        assert_eq!(source.dimensions(), (640, 480), "les dimensions ne doivent pas bouger");
    }

    #[test]
    fn is_alive_par_defaut_vaut_toujours_vrai() {
        // `FileSource` boucle indéfiniment et ne « meurt » jamais : la
        // méthode par défaut du trait doit refléter cela.
        let source = FileSource::from_annex_b(flux_de_test(2), 640, 480, 60).unwrap();
        assert!(source.is_alive());
    }
}
