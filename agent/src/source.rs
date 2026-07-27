//! Sources vidéo produisant des unités d'accès H.264 prêtes à être envoyées.

use crate::h264::{group_access_units, AccessUnit, CLOCK_RATE_HZ};
use anyhow::{bail, Result};

/// Producteur d'unités d'accès H.264.
///
/// L'implémentation Windows (capture + encodage) et la source fichier de test
/// se substituent l'une à l'autre derrière ce trait.
pub trait VideoSource {
    /// Unité d'accès suivante, ou `None` si la source est épuisée.
    fn next_frame(&mut self) -> Option<AccessUnit>;
    /// Dimensions de la vidéo produite, en pixels.
    fn dimensions(&self) -> (u32, u32);
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
}
