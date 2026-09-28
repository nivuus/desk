use super::*;
use std::cell::RefCell;

/// Fake driver: it counts the live outputs, it creates none.
struct PiloteFactice {
    vivantes: RefCell<Vec<IdSortie>>,
    next: RefCell<IdSortie>,
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
    fn with_ceiling(plafond: usize) -> Self {
        Self {
            vivantes: RefCell::new(Vec::new()),
            next: RefCell::new(1),
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
        Self::with_ceiling(usize::MAX)
    }
}

impl PiloteAffichageVirtuel for PiloteFactice {
    fn create(&self, _largeur: u32, _hauteur: u32, _hertz: u32) -> Result<IdSortie> {
        let mut vivantes = self.vivantes.borrow_mut();
        anyhow::ensure!(vivantes.len() < self.plafond, "fake driver's ceiling");
        let mut next = self.next.borrow_mut();
        let id = *next;
        *next += 1;
        vivantes.push(id);
        Ok(id)
    }

    fn detruire(&self, id: IdSortie) -> Result<()> {
        anyhow::ensure!(
            !*self.refuse_les_destructions.borrow(),
            "the fake driver refuses this destruction"
        );
        self.vivantes.borrow_mut().retain(|vivante| *vivante != id);
        self.detruites.borrow_mut().push(id);
        Ok(())
    }
}

#[test]
fn the_guard_destroys_everything_it_created() {
    let pilote = PiloteFactice::with_ceiling(8);
    {
        let mut sorties = Sorties::nouvelles(&pilote);
        sorties.create(1920, 1080, 60).unwrap();
        sorties.create(1920, 1080, 60).unwrap();
        sorties.create(1920, 1080, 60).unwrap();
        assert_eq!(sorties.count(), 3);
        assert_eq!(pilote.vivantes.borrow().len(), 3);
    }
    assert!(
        pilote.vivantes.borrow().is_empty(),
        "the guard left outputs behind"
    );
}

/// The case that justifies the guard: these APIs fail by crashing, and a
/// virtual monitor outlives the process.
#[test]
fn the_guard_destroys_even_when_the_thread_panics() {
    let pilote = PiloteFactice::with_ceiling(8);
    let issue = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut sorties = Sorties::nouvelles(&pilote);
        sorties.create(1920, 1080, 60).unwrap();
        sorties.create(1920, 1080, 60).unwrap();
        panic!("simulated panic in the middle of the ramp-up to N");
    }));
    assert!(issue.is_err(), "the panic should have propagated");
    assert!(
        pilote.vivantes.borrow().is_empty(),
        "ghost monitors survive a panic"
    );
}

#[test]
fn destroying_gives_the_output_back_to_the_driver_and_forgets_it() {
    let pilote = PiloteFactice::default();
    let mut sorties = Sorties::nouvelles(&pilote);
    let a = sorties.create(1280, 720, 60).unwrap();
    let b = sorties.create(1600, 900, 60).unwrap();

    sorties.detruire(a).unwrap();
    assert_eq!(*pilote.detruites.borrow(), vec![a]);
    assert_eq!(sorties.count(), 1);

    // The guard must not destroy `a` again: the driver would refuse, and the
    // log would accuse a due purge that does not exist.
    drop(sorties);
    assert_eq!(*pilote.detruites.borrow(), vec![a, b]);
}

#[test]
fn destroying_an_unknown_output_fails_without_touching_anything() {
    let pilote = PiloteFactice::default();
    let mut sorties = Sorties::nouvelles(&pilote);
    let a = sorties.create(1280, 720, 60).unwrap();

    assert!(sorties.detruire(a + 1000).is_err());
    assert!(pilote.detruites.borrow().is_empty());
    assert_eq!(sorties.count(), 1, "the legitimate output stays held");
}

#[test]
fn a_destruction_refused_by_the_driver_does_not_forget_the_output() {
    // The GUID is the project's only handle on this monitor: forgetting it
    // on failure would make it unrecoverable, and the guard would never retry
    // it.
    let pilote = PiloteFactice::default();
    let mut sorties = Sorties::nouvelles(&pilote);
    let a = sorties.create(1280, 720, 60).unwrap();
    *pilote.refuse_les_destructions.borrow_mut() = true;

    assert!(sorties.detruire(a).is_err());
    assert_eq!(
        sorties.count(),
        1,
        "the output stays due as long as it is not given back"
    );
}

#[test]
fn a_driver_refusal_does_not_lose_the_outputs_already_created() {
    let pilote = PiloteFactice::with_ceiling(2);
    {
        let mut sorties = Sorties::nouvelles(&pilote);
        sorties.create(1920, 1080, 60).unwrap();
        sorties.create(1920, 1080, 60).unwrap();
        assert!(
            sorties.create(1920, 1080, 60).is_err(),
            "the ceiling should have refused"
        );
        assert_eq!(sorties.count(), 2, "a refusal must not count as a creation");
    }
    assert!(pilote.vivantes.borrow().is_empty());
}

#[test]
fn identical_dimensions_give_a_unit_factor() {
    assert_eq!(
        facteur_echelle((2400, 1080), (2400, 1080)),
        Some((1.0, 1.0))
    );
}

/// The trap noted by the probe: output announced as 3413×960 by DXGI,
/// 5120×1440 by WMI — ratio 1.5, DPI scaling at 150 %.
#[test]
fn the_probe_dpi_trap_gives_a_factor_of_one_and_a_half() {
    let (horizontal, vertical) = facteur_echelle((3413, 960), (5120, 1440)).unwrap();
    assert!(
        (horizontal - 1.5).abs() < 0.001,
        "horizontal = {horizontal}"
    );
    assert!((vertical - 1.5).abs() < 0.001, "vertical = {vertical}");
}

#[test]
fn a_degenerate_announcement_gives_no_factor() {
    assert_eq!(facteur_echelle((0, 960), (5120, 1440)), None);
    assert_eq!(facteur_echelle((3413, 0), (5120, 1440)), None);
}

#[test]
fn without_scale_or_offset_the_region_does_not_move() {
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
fn an_offset_output_brings_the_region_back_to_its_texture_origin() {
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
fn the_dpi_factor_enlarges_the_region_and_its_origin() {
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
fn each_output_is_converted_with_its_own_factor() {
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
fn an_output_offset_in_the_virtual_desktop_starts_from_its_texture_origin() {
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
fn a_length_mismatch_is_refused() {
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
fn a_degenerate_output_is_refused_rather_than_assumed_at_unit_scale() {
    let sorties = vec![Rect {
        x: 0,
        y: 0,
        width: 0,
        height: 720,
    }];
    assert!(places_texture_par_sortie(&sorties, &[(1280, 720)]).is_err());
}
