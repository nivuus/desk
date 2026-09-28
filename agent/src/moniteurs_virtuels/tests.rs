use super::*;
use std::cell::RefCell;

/// Fake driver: it counts the live outputs, it creates none.
struct PiloteFactice {
    vivantes: RefCell<Vec<IdSortie>>,
    suivant: RefCell<IdSortie>,
    plafond: usize,
    /// Outputs actually returned by `detruire`, in order — what
    /// `Sorties::detruire` tests, distinctly from `vivantes` which only says
    /// what remains.
    detruites: RefCell<Vec<IdSortie>>,
    /// Makes any destruction fail as long as it is raised, without touching
    /// `vivantes`: it is the case where the driver refuses, and where the output
    /// must stay held by the guard.
    refuse_les_destructions: RefCell<bool>,
}

impl PiloteFactice {
    fn avec_plafond(plafond: usize) -> Self {
        Self {
            vivantes: RefCell::new(Vec::new()),
            suivant: RefCell::new(1),
            plafond,
            detruites: RefCell::new(Vec::new()),
            refuse_les_destructions: RefCell::new(false),
        }
    }
}

impl Default for PiloteFactice {
    /// No ceiling: the tests of `detruire` do not bear on the
    /// pool, `usize::MAX` keeps them from caring about it.
    fn default() -> Self {
        Self::avec_plafond(usize::MAX)
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
        anyhow::ensure!(
            !*self.refuse_les_destructions.borrow(),
            "le pilote factice refuse cette destruction"
        );
        self.vivantes.borrow_mut().retain(|vivante| *vivante != id);
        self.detruites.borrow_mut().push(id);
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

/// The case that justifies the guard: these APIs fail by crashing, and a
/// virtual monitor outlives the process.
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
fn detruire_rend_la_sortie_au_pilote_et_l_oublie() {
    let pilote = PiloteFactice::default();
    let mut sorties = Sorties::nouvelles(&pilote);
    let a = sorties.creer(1280, 720, 60).unwrap();
    let b = sorties.creer(1600, 900, 60).unwrap();

    sorties.detruire(a).unwrap();
    assert_eq!(*pilote.detruites.borrow(), vec![a]);
    assert_eq!(sorties.nombre(), 1);

    // The guard must not destroy `a` again: the driver would refuse, and the
    // log would accuse a due purge that does not exist.
    drop(sorties);
    assert_eq!(*pilote.detruites.borrow(), vec![a, b]);
}

#[test]
fn detruire_une_sortie_inconnue_echoue_sans_rien_toucher() {
    let pilote = PiloteFactice::default();
    let mut sorties = Sorties::nouvelles(&pilote);
    let a = sorties.creer(1280, 720, 60).unwrap();

    assert!(sorties.detruire(a + 1000).is_err());
    assert!(pilote.detruites.borrow().is_empty());
    assert_eq!(sorties.nombre(), 1, "la sortie légitime reste tenue");
}

#[test]
fn une_destruction_refusee_par_le_pilote_ne_fait_pas_oublier_la_sortie() {
    // The GUID is the project's only handle on this monitor: forgetting it
    // on failure would make it unrecoverable, and the guard would never retry
    // it.
    let pilote = PiloteFactice::default();
    let mut sorties = Sorties::nouvelles(&pilote);
    let a = sorties.creer(1280, 720, 60).unwrap();
    *pilote.refuse_les_destructions.borrow_mut() = true;

    assert!(sorties.detruire(a).is_err());
    assert_eq!(
        sorties.nombre(),
        1,
        "la sortie reste due tant qu'elle n'est pas rendue"
    );
}

#[test]
fn un_refus_du_pilote_ne_perd_pas_les_sorties_deja_creees() {
    let pilote = PiloteFactice::avec_plafond(2);
    {
        let mut sorties = Sorties::nouvelles(&pilote);
        sorties.creer(1920, 1080, 60).unwrap();
        sorties.creer(1920, 1080, 60).unwrap();
        assert!(
            sorties.creer(1920, 1080, 60).is_err(),
            "le plafond aurait dû refuser"
        );
        assert_eq!(
            sorties.nombre(),
            2,
            "un refus ne doit pas compter comme une création"
        );
    }
    assert!(pilote.vivantes.borrow().is_empty());
}

#[test]
fn des_dimensions_identiques_donnent_un_facteur_unite() {
    assert_eq!(
        facteur_echelle((2400, 1080), (2400, 1080)),
        Some((1.0, 1.0))
    );
}

/// The trap noted by the probe: output announced as 3413×960 by DXGI,
/// 5120×1440 by WMI — ratio 1.5, DPI scaling at 150 %.
#[test]
fn le_piege_dpi_de_la_sonde_donne_un_facteur_de_un_et_demi() {
    let (horizontal, vertical) = facteur_echelle((3413, 960), (5120, 1440)).unwrap();
    assert!(
        (horizontal - 1.5).abs() < 0.001,
        "horizontal = {horizontal}"
    );
    assert!((vertical - 1.5).abs() < 0.001, "vertical = {vertical}");
}

#[test]
fn une_annonce_degeneree_ne_donne_aucun_facteur() {
    assert_eq!(facteur_echelle((0, 960), (5120, 1440)), None);
    assert_eq!(facteur_echelle((3413, 0), (5120, 1440)), None);
}

#[test]
fn sans_echelle_ni_decalage_la_region_ne_bouge_pas() {
    let sortie = Rect {
        x: 0,
        y: 0,
        width: 2400,
        height: 1080,
    };
    let region = Rect {
        x: 100,
        y: 200,
        width: 300,
        height: 400,
    };
    assert_eq!(vers_texture(region, sortie, (1.0, 1.0)), region);
}

#[test]
fn une_sortie_decalee_ramene_la_region_a_l_origine_de_sa_texture() {
    let sortie = Rect {
        x: 2400,
        y: 0,
        width: 3413,
        height: 960,
    };
    let region = Rect {
        x: 2500,
        y: 100,
        width: 200,
        height: 200,
    };
    assert_eq!(
        vers_texture(region, sortie, (1.0, 1.0)),
        Rect {
            x: 100,
            y: 100,
            width: 200,
            height: 200
        }
    );
}

/// The case that matters: cropping on the announced rectangle while the
/// texture is at physical dimensions would shift everything by a factor of 1.5.
#[test]
fn le_facteur_dpi_agrandit_la_region_et_son_origine() {
    let sortie = Rect {
        x: 0,
        y: 0,
        width: 3413,
        height: 960,
    };
    let region = Rect {
        x: 100,
        y: 100,
        width: 200,
        height: 200,
    };
    assert_eq!(
        vers_texture(region, sortie, (1.5, 1.5)),
        Rect {
            x: 150,
            y: 150,
            width: 300,
            height: 300
        }
    );
}

/// The trap this function exists to avoid: applying to all
/// outputs the scale factor of the first. Two virtual outputs
/// can carry two different DPIs, and a crop computed with the wrong
/// factor is shifted without anything reporting it.
#[test]
fn chaque_sortie_est_convertie_avec_son_propre_facteur() {
    let sorties = vec![
        Rect {
            x: 0,
            y: 0,
            width: 1280,
            height: 720,
        },
        Rect {
            x: 1280,
            y: 0,
            width: 853,
            height: 480,
        },
    ];
    let textures = vec![(1280, 720), (1280, 720)];
    let places = places_texture_par_sortie(&sorties, &textures).unwrap();
    assert_eq!(
        places[0],
        Rect {
            x: 0,
            y: 0,
            width: 1280,
            height: 720
        }
    );
    // Factor 1280/853 ≈ 1.5: the second output covers its WHOLE texture.
    // It is the test that matters: with output 0's factor (unity),
    // one would get 853×480 in a corner of a 1280×720 texture.
    assert_eq!(
        places[1],
        Rect {
            x: 0,
            y: 0,
            width: 1280,
            height: 720
        }
    );
}

/// Each slot is brought back to the origin of ITS texture: it is what
/// distinguishes N outputs from N tiles on one output.
#[test]
fn une_sortie_decalee_dans_le_bureau_virtuel_part_de_l_origine_de_sa_texture() {
    let sorties = vec![Rect {
        x: 3840,
        y: 200,
        width: 1280,
        height: 720,
    }];
    let places = places_texture_par_sortie(&sorties, &[(1280, 720)]).unwrap();
    assert_eq!(
        places[0],
        Rect {
            x: 0,
            y: 0,
            width: 1280,
            height: 720
        }
    );
}

#[test]
fn un_desaccord_de_longueur_est_refuse() {
    let sorties = vec![Rect {
        x: 0,
        y: 0,
        width: 1280,
        height: 720,
    }];
    assert!(places_texture_par_sortie(&sorties, &[]).is_err());
    assert!(places_texture_par_sortie(&[], &[(1280, 720)]).is_err());
}

/// A degenerate announcement gives no factor (`facteur_echelle` returns
/// `None`): the refusal must come out, not a silent unity factor that
/// would shift all the crops of this output.
#[test]
fn une_sortie_degeneree_est_refusee_plutot_que_supposee_a_l_unite() {
    let sorties = vec![Rect {
        x: 0,
        y: 0,
        width: 0,
        height: 720,
    }];
    assert!(places_texture_par_sortie(&sorties, &[(1280, 720)]).is_err());
}
