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
use super::moniteurs::PiloteParIoctl;
use super::voies::VoieDeCapture;

/// Durée de chaque passe.
pub(super) const DUREE_PASSE: Duration = Duration::from_secs(10);
/// Cadence de journalisation des compteurs.
pub(super) const PERIODE_JOURNAL: Duration = Duration::from_secs(1);
/// Cadence de battement du chien de garde du pilote d'affichage virtuel.
///
/// Une seconde est le tiers de `delai = 3` lu en secondes, la lecture la plus
/// défavorable de ce champ d'unité inconnue (même raisonnement que
/// `montee::CADENCE_PING`). Le garde ne change pas de nature selon la passe qui
/// court : une seule valeur pour toutes.
const CADENCE_PING: Duration = Duration::from_secs(1);

/// Bat le chien de garde du pilote à cadence fixe — **et mesure ce qu'elle a
/// réellement battu**.
///
/// Le second rôle n'est pas décoratif. La ronde 1 a daté sur le journal un trou
/// de 11,1 s sans un seul ping (passe témoin et ouverture des duplications),
/// que rien dans le programme ne signalait : les pings n'étant pas tracés, un
/// trou ne se voyait qu'en recoupant à la main des horodatages de lignes
/// voisines. `intervalle_max` rend ce défaut OBSERVABLE d'un seul nombre, et
/// c'est ce nombre qui prouve — ou non — que le trou est fermé.
///
/// Un compteur agrégé, journalisé une fois en fin de mesure : tracer chaque
/// ping donnerait une ligne par seconde pour rien.
pub(super) struct Garde<'p> {
    pilote: &'p PiloteParIoctl,
    dernier: Instant,
    intervalle_max: Duration,
}

impl<'p> Garde<'p> {
    /// À construire juste après le dernier ping connu — typiquement au retour
    /// d'`attendre_en_pinguant`.
    ///
    /// **Précision d'énoncé, corrigée en revue finale** : `dernier` est posé à
    /// `Instant::now()` ICI, donc la couture entre le dernier ping réel et
    /// cette construction **échappe au compteur** — elle n'est pas *comptée*,
    /// elle est rendue *négligeable* par l'adjacence des deux appels (66 µs au
    /// relevé du chantier des duplications parallèles). Compter cette couture
    /// exigerait qu'`attendre_en_pinguant` rende l'instant de son dernier ping.
    pub(super) fn nouvelle(pilote: &'p PiloteParIoctl) -> Self {
        Self { pilote, dernier: Instant::now(), intervalle_max: Duration::ZERO }
    }

    /// Bat sans condition, et retient l'écart depuis le battement précédent.
    pub(super) fn battre(&mut self) -> Result<()> {
        let maintenant = Instant::now();
        self.intervalle_max = self.intervalle_max.max(maintenant - self.dernier);
        self.pilote.pinguer()?;
        self.dernier = maintenant;
        Ok(())
    }

    /// Bat si la cadence l'exige, sans rien faire sinon.
    pub(super) fn battre_si_du(&mut self) -> Result<()> {
        if self.dernier.elapsed() >= CADENCE_PING {
            self.battre()?;
        }
        Ok(())
    }

    /// Le plus grand écart entre deux battements, **y compris celui qui court
    /// depuis le dernier** : un trou ouvert à l'instant de la lecture compte
    /// autant qu'un trou refermé, sans quoi le dernier segment de la mesure
    /// échapperait au contrôle.
    pub(super) fn intervalle_max(&self) -> Duration {
        self.intervalle_max.max(self.dernier.elapsed())
    }
}

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
///
/// `garde` bat le chien de garde du pilote pendant la passe. `None` pour le
/// protocole mono-sortie (`banc.rs`), qui ne détient aucun pilote ; `Some` pour
/// le protocole multi-sorties (`paralleles.rs`), dont les N sorties vivent sous
/// un chien de garde d'unité INCONNUE — **la seconde n'est pas exclue**. Sans
/// ce battement, cette passe est un trou de dix secondes pendant lequel le
/// pilote peut reprendre ses sorties, et la mesure suivante capturerait du noir
/// sans que rien ne dise pourquoi (trou de 11,1 s daté au journal de la
/// ronde 1).
pub(super) fn passe_temoin(mires: &mut Mires, mut garde: Option<&mut Garde<'_>>) -> Result<()> {
    let debut = Instant::now();
    let mut trames = 0u64;
    while debut.elapsed() < DUREE_PASSE {
        mires.peindre()?;
        mires.pomper();
        trames += 1;
        if let Some(garde) = garde.as_mut() {
            garde.battre_si_du()?;
        }
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
