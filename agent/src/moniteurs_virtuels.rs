//! Mesure ① de la spec : combien de sorties virtuelles simultanées un pilote
//! d'affichage indirect accepte-t-il, et une fenêtre posée dessus est-elle
//! capturée correctement.
//!
//! Ce module ne contient QUE de la logique pure : le trait que doit remplir
//! un pilote, la garde qui détruit ce qui a été créé, et les conversions de
//! coordonnées. La glue Windows vit dans
//! `diagnostics/multifenetre/moniteurs.rs`.
//!
//! Il n'est PAS sous `#[cfg(windows)]`, délibérément : une sortie virtuelle
//! survit au processus, donc la garde ci-dessous est le seul rempart contre
//! une VM laissée avec des moniteurs fantômes — c'est exactement le genre de
//! code qui doit avoir des tests, et ils ne tourneraient pas sous
//! `#[cfg(windows)]`.

use anyhow::{Context, Result};

use crate::geometry::Rect;

/// Identifiant d'une sortie virtuelle, tel que le pilote le rend.
pub type IdSortie = u32;

/// Ce que ce bloc attend d'un pilote d'affichage virtuel, quel qu'il soit.
///
/// L'indirection existe pour deux raisons. La spec §6.4 acte un repli —
/// changer de pilote si celui de la VM résiste — et ce repli ne doit faire
/// réécrire ni la montée en N ni la garde. Et la garde ci-dessous doit
/// pouvoir être éprouvée sans Windows.
pub trait PiloteAffichageVirtuel {
    fn creer(&self, largeur: u32, hauteur: u32, hertz: u32) -> Result<IdSortie>;
    fn detruire(&self, id: IdSortie) -> Result<()>;
}

/// Détruit les sorties créées quoi qu'il arrive, y compris si le fil panique.
///
/// Sans elle, une sonde qui plante à la cinquième création laisse cinq
/// moniteurs derrière elle, et l'état survit au processus.
pub struct Sorties<'p> {
    pilote: &'p dyn PiloteAffichageVirtuel,
    creees: Vec<IdSortie>,
}

impl<'p> Sorties<'p> {
    pub fn nouvelles(pilote: &'p dyn PiloteAffichageVirtuel) -> Self {
        Self { pilote, creees: Vec::new() }
    }

    /// Un refus du pilote ressort tel quel et ne compte pas comme création :
    /// détruire un identifiant que le pilote n'a jamais rendu ferait au mieux
    /// une erreur de plus au journal, au pire détruirait la sortie d'autrui.
    pub fn creer(&mut self, largeur: u32, hauteur: u32, hertz: u32) -> Result<IdSortie> {
        let id = self.pilote.creer(largeur, hauteur, hertz)?;
        self.creees.push(id);
        Ok(id)
    }

    pub fn nombre(&self) -> usize {
        self.creees.len()
    }
}

impl Drop for Sorties<'_> {
    fn drop(&mut self) {
        // En ordre inverse de création : si le pilote a un état d'ordre, le
        // défaire dans l'ordre où il a été construit est le seul choix sûr.
        // `Drop` court aussi pendant le déroulement d'une panique — c'est
        // précisément le cas que la garde existe pour couvrir.
        for id in self.creees.drain(..).rev() {
            if let Err(erreur) = self.pilote.detruire(id) {
                tracing::error!(
                    id,
                    %erreur,
                    "sortie virtuelle NON détruite — purge manuelle requise"
                );
            }
        }
    }
}

/// Analyse une désignation de sortie « index_adaptateur:index_sortie », telle
/// que la portent les variables d'environnement du banc.
///
/// Les deux index sont ceux de `capture::enumerer_sorties`, et ce sont ceux
/// qu'attend `DesktopCapture::sur_sortie`.
pub fn analyser_designation(texte: &str) -> Result<(u32, u32)> {
    let (adaptateur, sortie) = texte
        .split_once(':')
        .with_context(|| format!("désignation « {texte} » : forme attendue « adaptateur:sortie »"))?;
    let adaptateur: u32 = adaptateur
        .parse()
        .with_context(|| format!("index d'adaptateur « {adaptateur} » n'est pas un entier"))?;
    let sortie: u32 = sortie
        .parse()
        .with_context(|| format!("index de sortie « {sortie} » n'est pas un entier"))?;
    Ok((adaptateur, sortie))
}

/// Rapport entre les dimensions ANNONCÉES par la sortie
/// (`DXGI_OUTPUT_DESC::DesktopCoordinates`) et celles de la texture
/// RÉELLEMENT rendue par l'acquisition.
///
/// La sonde a relevé une sortie virtuelle annoncée 3413×960 par DXGI quand
/// WMI la disait 5120×1440 — rapport 1,5006, la mise à l'échelle DPI à 150 %.
/// Si un recadrage est calculé sur le rectangle annoncé alors que la texture
/// est aux dimensions physiques, il est décalé d'autant. Rend `None` si
/// l'annonce est dégénérée : un rapport n'y aurait aucun sens.
pub fn facteur_echelle(annonce: (u32, u32), texture: (u32, u32)) -> Option<(f64, f64)> {
    if annonce.0 == 0 || annonce.1 == 0 {
        return None;
    }
    Some((
        texture.0 as f64 / annonce.0 as f64,
        texture.1 as f64 / annonce.1 as f64,
    ))
}

/// Convertit un rectangle exprimé en coordonnées du bureau virtuel — celles
/// où vivent les fenêtres — vers les coordonnées de la texture rendue par
/// l'acquisition de `sortie`.
///
/// Deux corrections en une : le décalage de l'origine de la sortie dans le
/// bureau virtuel, et le facteur d'échelle de `facteur_echelle`.
pub fn vers_texture(region: Rect, sortie: Rect, facteur: (f64, f64)) -> Rect {
    let x = (region.x - sortie.x) as f64 * facteur.0;
    let y = (region.y - sortie.y) as f64 * facteur.1;
    Rect {
        x: x.round() as i32,
        y: y.round() as i32,
        width: (region.width as f64 * facteur.0).round() as u32,
        height: (region.height as f64 * facteur.1).round() as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// Pilote factice : il compte les sorties vivantes, il n'en crée aucune.
    struct PiloteFactice {
        vivantes: RefCell<Vec<IdSortie>>,
        suivant: RefCell<IdSortie>,
        plafond: usize,
    }

    impl PiloteFactice {
        fn avec_plafond(plafond: usize) -> Self {
            Self { vivantes: RefCell::new(Vec::new()), suivant: RefCell::new(1), plafond }
        }
    }

    impl PiloteAffichageVirtuel for PiloteFactice {
        fn creer(&self, _largeur: u32, _hauteur: u32, _hertz: u32) -> Result<IdSortie> {
            let mut vivantes = self.vivantes.borrow_mut();
            anyhow::ensure!(vivantes.len() < self.plafond, "plafond du pilote factice");
            let mut suivant = self.suivant.borrow_mut();
            let id = *suivant;
            *suivant += 1;
            vivantes.push(id);
            Ok(id)
        }

        fn detruire(&self, id: IdSortie) -> Result<()> {
            self.vivantes.borrow_mut().retain(|vivante| *vivante != id);
            Ok(())
        }
    }

    #[test]
    fn la_garde_detruit_tout_ce_qu_elle_a_cree() {
        let pilote = PiloteFactice::avec_plafond(8);
        {
            let mut sorties = Sorties::nouvelles(&pilote);
            sorties.creer(1920, 1080, 60).unwrap();
            sorties.creer(1920, 1080, 60).unwrap();
            sorties.creer(1920, 1080, 60).unwrap();
            assert_eq!(sorties.nombre(), 3);
            assert_eq!(pilote.vivantes.borrow().len(), 3);
        }
        assert!(
            pilote.vivantes.borrow().is_empty(),
            "la garde a laissé des sorties derrière elle"
        );
    }

    /// Le cas qui justifie la garde : ces API échouent par plantage, et un
    /// moniteur virtuel survit au processus.
    #[test]
    fn la_garde_detruit_meme_quand_le_fil_panique() {
        let pilote = PiloteFactice::avec_plafond(8);
        let issue = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut sorties = Sorties::nouvelles(&pilote);
            sorties.creer(1920, 1080, 60).unwrap();
            sorties.creer(1920, 1080, 60).unwrap();
            panic!("panique simulée au milieu de la montée en N");
        }));
        assert!(issue.is_err(), "la panique aurait dû se propager");
        assert!(
            pilote.vivantes.borrow().is_empty(),
            "des moniteurs fantômes survivent à une panique"
        );
    }

    #[test]
    fn un_refus_du_pilote_ne_perd_pas_les_sorties_deja_creees() {
        let pilote = PiloteFactice::avec_plafond(2);
        {
            let mut sorties = Sorties::nouvelles(&pilote);
            sorties.creer(1920, 1080, 60).unwrap();
            sorties.creer(1920, 1080, 60).unwrap();
            assert!(sorties.creer(1920, 1080, 60).is_err(), "le plafond aurait dû refuser");
            assert_eq!(sorties.nombre(), 2, "un refus ne doit pas compter comme une création");
        }
        assert!(pilote.vivantes.borrow().is_empty());
    }

    #[test]
    fn une_designation_bien_formee_donne_les_deux_index() {
        assert_eq!(analyser_designation("0:1").unwrap(), (0, 1));
        assert_eq!(analyser_designation("2:0").unwrap(), (2, 0));
    }

    #[test]
    fn une_designation_mal_formee_est_refusee() {
        assert!(analyser_designation("0").is_err(), "un seul index");
        assert!(analyser_designation("0:1:2").is_err(), "trois index");
        assert!(analyser_designation("a:b").is_err(), "pas des entiers");
        assert!(analyser_designation("").is_err(), "vide");
    }

    #[test]
    fn des_dimensions_identiques_donnent_un_facteur_unite() {
        assert_eq!(facteur_echelle((2400, 1080), (2400, 1080)), Some((1.0, 1.0)));
    }

    /// Le piège relevé par la sonde : sortie annoncée 3413×960 par DXGI,
    /// 5120×1440 par WMI — rapport 1,5, la mise à l'échelle DPI à 150 %.
    #[test]
    fn le_piege_dpi_de_la_sonde_donne_un_facteur_de_un_et_demi() {
        let (horizontal, vertical) = facteur_echelle((3413, 960), (5120, 1440)).unwrap();
        assert!((horizontal - 1.5).abs() < 0.001, "horizontal = {horizontal}");
        assert!((vertical - 1.5).abs() < 0.001, "vertical = {vertical}");
    }

    #[test]
    fn une_annonce_degeneree_ne_donne_aucun_facteur() {
        assert_eq!(facteur_echelle((0, 960), (5120, 1440)), None);
        assert_eq!(facteur_echelle((3413, 0), (5120, 1440)), None);
    }

    #[test]
    fn sans_echelle_ni_decalage_la_region_ne_bouge_pas() {
        let sortie = Rect { x: 0, y: 0, width: 2400, height: 1080 };
        let region = Rect { x: 100, y: 200, width: 300, height: 400 };
        assert_eq!(vers_texture(region, sortie, (1.0, 1.0)), region);
    }

    #[test]
    fn une_sortie_decalee_ramene_la_region_a_l_origine_de_sa_texture() {
        let sortie = Rect { x: 2400, y: 0, width: 3413, height: 960 };
        let region = Rect { x: 2500, y: 100, width: 200, height: 200 };
        assert_eq!(
            vers_texture(region, sortie, (1.0, 1.0)),
            Rect { x: 100, y: 100, width: 200, height: 200 }
        );
    }

    /// Le cas qui compte : recadrer sur le rectangle annoncé alors que la
    /// texture est aux dimensions physiques décalerait tout d'un facteur 1,5.
    #[test]
    fn le_facteur_dpi_agrandit_la_region_et_son_origine() {
        let sortie = Rect { x: 0, y: 0, width: 3413, height: 960 };
        let region = Rect { x: 100, y: 100, width: 200, height: 200 };
        assert_eq!(
            vers_texture(region, sortie, (1.5, 1.5)),
            Rect { x: 150, y: 150, width: 300, height: 300 }
        );
    }
}
