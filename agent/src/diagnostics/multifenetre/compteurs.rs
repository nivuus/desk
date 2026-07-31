//! La métrologie du banc, partagée par ses deux protocoles.
//!
//! Extraite de `banc.rs` au moment où un second protocole est apparu (le
//! montage multi-sorties de `paralleles.rs`). Le partage n'est pas un confort
//! d'écriture : c'est ce qui rend les chiffres des deux bancs comparables. Un
//! second banc qui aurait sa propre boucle de comptage divergerait, et l'on ne
//! saurait plus si un écart de cadence vient de la voie mesurée ou du banc qui
//! la mesure.
//!
//! Ce qui n'est PAS ici : la mise en scène du recouvrement et la porte
//! éliminatoire (propres au protocole mono-sortie, restées dans `banc.rs`), et
//! la rotation du contrôle (propre au protocole multi-sorties, dans
//! `paralleles.rs`).

use std::time::{Duration, Instant};

use anyhow::Result;

use crate::capture::CapturedFrame;
use crate::mire;

use super::mires::Mires;
use super::voies::VoieDeCapture;

/// Durée de chaque passe.
pub(super) const DUREE_PASSE: Duration = Duration::from_secs(10);
/// Cadence de journalisation des compteurs.
pub(super) const PERIODE_JOURNAL: Duration = Duration::from_secs(1);

/// Décompte des verdicts rendus sur la mire 0, par NATURE et non en bloc.
///
/// Un simple compte de « faux » ne suffit pas à la mesure ③ : une image NOIRE
/// et une image portant la mire du DESSUS sont deux résultats opposés. La
/// première dirait que Windows ne compose pas une sortie virtuelle sans écran
/// attaché — et l'hypothèse fondatrice de la voie « un moniteur virtuel par
/// fenêtre » tomberait. La seconde dirait qu'il la compose parfaitement, et que
/// c'est la duplication qui ne sait pas défaire un recouvrement — ce qu'on
/// savait déjà du bureau physique. Les confondre sous un même compteur rendrait
/// la mesure ininterprétable.
#[derive(Default, Debug)]
pub(super) struct Verdicts {
    pub(super) justes: u64,
    pub(super) voisines: u64,
    pub(super) noires: u64,
    pub(super) inconnues: u64,
}

impl Verdicts {
    pub(super) fn compter(&mut self, verdict: mire::Verdict) {
        match verdict {
            mire::Verdict::Juste => self.justes += 1,
            mire::Verdict::Voisine(_) => self.voisines += 1,
            mire::Verdict::Noire => self.noires += 1,
            mire::Verdict::Inconnue => self.inconnues += 1,
        }
    }

    pub(super) fn faux(&self) -> u64 {
        self.voisines + self.noires + self.inconnues
    }
}

pub(super) struct Compteurs {
    pub(super) images: Vec<u64>,
    pub(super) unites: Vec<u64>,
    /// Verdicts rendus AVANT que le recouvrement ne soit posé : la mire 0 est
    /// alors dégagée, et c'est la seule fenêtre du banc où se lise « cette voie
    /// capture-t-elle simplement cette fenêtre ». Sur le bureau physique la
    /// réponse allait de soi ; sur une sortie virtuelle, c'est la question.
    pub(super) avant_recouvrement: Verdicts,
    /// Verdicts rendus une fois la mire 0 recouverte : la porte éliminatoire.
    pub(super) apres_recouvrement: Verdicts,
}

impl Compteurs {
    pub(super) fn nouveaux(nombre: usize) -> Self {
        Self {
            images: vec![0; nombre],
            unites: vec![0; nombre],
            avant_recouvrement: Verdicts::default(),
            apres_recouvrement: Verdicts::default(),
        }
    }
}

/// Passe témoin : les mires peignent, rien ne capture.
pub(super) fn passe_temoin(mires: &mut Mires) -> Result<()> {
    let debut = Instant::now();
    let mut trames = 0u64;
    while debut.elapsed() < DUREE_PASSE {
        mires.peindre()?;
        mires.pomper();
        trames += 1;
    }
    let secondes = debut.elapsed().as_secs_f64();
    tracing::info!(
        mires = mires.nombre(),
        trames,
        cadence = trames as f64 / secondes,
        "passe TÉMOIN — cadence de peinture sans capture"
    );
    Ok(())
}

pub(super) fn journaliser(passe: &str, voie: &str, nombre: u8, compteurs: &Compteurs) {
    let secondes = DUREE_PASSE.as_secs_f64();
    let cadences: Vec<f64> = compteurs.images.iter().map(|n| *n as f64 / secondes).collect();
    tracing::info!(
        passe,
        voie,
        nombre,
        ?cadences,
        unites = ?compteurs.unites,
        verdicts_faux = compteurs.apres_recouvrement.faux(),
        mire0_avant_recouvrement = ?compteurs.avant_recouvrement,
        mire0_apres_recouvrement = ?compteurs.apres_recouvrement,
        "passe terminée"
    );
}

/// Lit le centre de l'image et rend le verdict correspondant.
///
/// `attendu` est l'identité de la mire qui DOIT s'y trouver. La lecture se
/// fait sur le périphérique de la voie : une texture ne se lit pas depuis un
/// autre périphérique que le sien.
pub(super) fn lire_verdict(
    voie: &mut dyn VoieDeCapture,
    image: &CapturedFrame,
    attendu: u8,
) -> Result<mire::Verdict> {
    let appareil = voie.device();
    let (r, g, b, _a) = crate::diagnostics::pixels::read_pixel(
        &appareil,
        &image.texture,
        image.width,
        image.height,
        image.width / 2,
        image.height / 2,
    )?;
    Ok(mire::verdict(attendu, (r, g, b)))
}
