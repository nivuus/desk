use super::*;
use std::cell::RefCell;

/// Pilote factice : il compte les sorties vivantes, il n'en crée aucune.
struct PiloteFactice {
    vivantes: RefCell<Vec<IdSortie>>,
    suivant: RefCell<IdSortie>,
    plafond: usize,
    /// Sorties effectivement rendues par `detruire`, dans l'ordre — ce que
    /// `Sorties::detruire` teste, distinctement de `vivantes` qui ne dit
    /// que ce qui reste.
    detruites: RefCell<Vec<IdSortie>>,
    /// Fait échouer toute destruction tant que levé, sans toucher
    /// `vivantes` : c'est le cas où le pilote refuse, et où la sortie
    /// doit rester tenue par la garde.
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
    /// Aucun plafond : les tests de `detruire` ne portent pas sur le
    /// vivier, `usize::MAX` évite qu'ils s'en soucient.
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
fn detruire_rend_la_sortie_au_pilote_et_l_oublie() {
    let pilote = PiloteFactice::default();
    let mut sorties = Sorties::nouvelles(&pilote);
    let a = sorties.creer(1280, 720, 60).unwrap();
    let b = sorties.creer(1600, 900, 60).unwrap();

    sorties.detruire(a).unwrap();
    assert_eq!(*pilote.detruites.borrow(), vec![a]);
    assert_eq!(sorties.nombre(), 1);

    // La garde ne doit pas redétruire `a` : le pilote refuserait, et le
    // journal accuserait une purge due qui n'existe pas.
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
    // Le GUID est la seule prise du projet sur ce moniteur : l'oublier
    // sur échec le rendrait irrécupérable, et la garde ne le retenterait
    // jamais.
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

/// Le piège relevé par la sonde : sortie annoncée 3413×960 par DXGI,
/// 5120×1440 par WMI — rapport 1,5, la mise à l'échelle DPI à 150 %.
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

/// Le cas qui compte : recadrer sur le rectangle annoncé alors que la
/// texture est aux dimensions physiques décalerait tout d'un facteur 1,5.
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

/// Le piège que cette fonction existe pour éviter : appliquer à toutes les
/// sorties le facteur d'échelle de la première. Deux sorties virtuelles
/// peuvent porter deux DPI différents, et un recadrage calculé au mauvais
/// facteur est décalé sans que rien ne le signale.
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
    // Facteur 1280/853 ≈ 1,5 : la seconde sortie couvre TOUTE sa texture.
    // C'est le test qui compte : avec le facteur de la sortie 0 (l'unité),
    // on obtiendrait 853×480 dans un coin d'une texture 1280×720.
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

/// Chaque place est ramenée à l'origine de SA texture : c'est ce qui
/// distingue N sorties de N tuiles sur une sortie.
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

/// Une annonce dégénérée ne donne aucun facteur (`facteur_echelle` rend
/// `None`) : le refus doit ressortir, pas un facteur unité silencieux qui
/// décalerait tous les recadrages de cette sortie.
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
