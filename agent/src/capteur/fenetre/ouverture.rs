//! `Fenetre::ouvrir` : interprète le message d'attache et prépare la
//! `Fenetre`, **sans construire sa source**.
//!
//! Extrait de `fenetre.rs` (revue de la tâche 8 du sous-bloc D10) : les
//! tâches 8 et 9 avaient porté ce fichier à 505 lignes, au-dessus du plafond
//! de 500 (`CLAUDE.md`). `ouvrir` a une responsabilité nette — interpréter la
//! trame d'attache et construire une `Fenetre` endormie, sans ouvrir de
//! duplication DXGI ni construire d'encodeur — et rejoint le patron déjà
//! établi par les trois autres modules enfants de ce même fichier
//! (`commandes.rs`, `trace.rs`, `transitions.rs`), tous nés de la même raison
//! de plafond.
//!
//! Déplacée caractère pour caractère, commentaires compris : ce module
//! n'ajoute et ne retire aucun comportement, il ne fait que changer
//! d'adresse.

use std::time::Instant;

use anyhow::{bail, Context, Result};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

use crate::capteur::horloge::{frequence_qpc, lire_qpc, origine_depuis_qpc};
use crate::capteur::protocole::VersCapteur;

use super::{Fenetre, Parametres};

impl Fenetre {
    /// Prépare la fenêtre annoncée par une trame d'attache — **sans construire
    /// sa source**.
    ///
    /// **Une fenêtre que personne ne regarde ne consomme aucun encodeur.**
    /// Jusqu'au sous-bloc D5 cette fonction appelait `sur_sortie`, donc ouvrait
    /// une duplication DXGI et construisait un encodeur matériel, *avant* toute
    /// inscription au vivier : la 9ᵉ fenêtre échouait au plafond matériel comme
    /// si le vivier n'existait pas, et la comptabilité du vivier était fausse
    /// dès la naissance — il croyait la place libre alors qu'elle était déjà
    /// prise. La fenêtre naît désormais **endormie**, et c'est le premier
    /// `Ordre::Reveiller` qui construit la source, une fois la place acquise.
    ///
    /// ⚠️ **Cela DÉPLACE la détection des échecs, sans la perdre.** Un `hwnd`
    /// invalide ou une sortie inaccessible faisait jusqu'ici échouer l'attache,
    /// et l'enfant recevait un `Refus` immédiat. Désormais l'attache ne peut
    /// plus échouer que sur la résolution de la TAILLE ; une source impossible
    /// à construire ne se manifeste qu'au premier réveil, par un `warn!` et un
    /// `echec_de_reveil` qui la fait reproposer indéfiniment. Une fenêtre dont
    /// la sortie a disparu entre l'attache et le réveil boucle donc sur des
    /// réveils refusés au lieu de mourir — c'est le prix de l'acquisition
    /// préalable, et il est assumé.
    ///
    /// La réponse à l'attache n'est PAS écrite ici : elle part sur la
    /// connexion de commandes, que ce fil ne touche jamais. L'appelant écrit
    /// `Attachee { largeur, hauteur }` en cas de succès, `Refus` sinon.
    pub fn ouvrir(attache: VersCapteur) -> Result<Fenetre> {
        // Renommé à la destructuration : `Contexte.taille` — dans le module
        // PARENT, `capteur/fenetre.rs`, et non « plus bas dans ce fichier »
        // comme cette phrase le disait avant l'extraction du même sous-bloc
        // (le déplacement verbatim a conservé le texte et cassé le
        // déictique) — désigne la taille RÉSOLUE courante, sans rapport avec la
        // taille DEMANDÉE que l'attache apporte ici. Les deux cohabitent dans
        // ce module ; ne pas les confondre au premier coup d'œil.
        let VersCapteur::Attache {
            session,
            hwnd,
            sortie,
            fps,
            debit,
            taille: taille_demandee,
            origine_qpc,
        } = attache
        else {
            bail!("le premier message d'un enfant doit être une attache");
        };

        let clock_origin = origine_depuis_qpc(
            origine_qpc,
            lire_qpc().context("lecture de QPC à l'attache")?,
            frequence_qpc().context("fréquence de QPC")?,
            Instant::now(),
        );

        let hwnd = HWND(hwnd as *mut core::ffi::c_void);
        // Le PID ne circule pas sur le protocole : il se dérive du `hwnd` que
        // l'enfant a déjà envoyé. L'enfant fait de même de son côté, depuis son
        // `FENETRE_HWND`. Deux dérivations indépendantes du même identifiant
        // stable valent mieux qu'un champ de protocole à tenir cohérent.
        // Pré-initialisé à 0, et c'est CE 0 que le `ensure` ci-dessous
        // attrape en cas d'échec — pas une garantie de l'API Windows, qui ne
        // documente aucune écriture de `lpdwProcessId` en cas d'échec.
        let mut pid = 0u32;
        // SAFETY : `hwnd` vient d'un enfant vivant, et `&mut pid` est un
        // pointeur valide vers une variable initialisée pour toute la durée
        // de l'appel.
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        anyhow::ensure!(
            pid != 0,
            "impossible de dériver le PID de la fenêtre {hwnd:?} de la session {session}"
        );
        // La taille est la seule chose qu'il faut savoir avant d'avoir la
        // place : `taille_de_sortie` la lit sans ouvrir de duplication, donc
        // sans prendre le mutex de la sortie ni perturber aucune voisine.
        let sortie_taille = crate::capture::ouverture::taille_de_sortie(&sortie)
            .with_context(|| format!("attache de la session {session}"))?;
        // La sortie peut être plus grande que la fenêtre (registre pollué,
        // D9 §9). L'enfant a annoncé la taille que le superviseur lui a
        // donnée ; le capteur la borne à ce que la sortie offre réellement,
        // avec la MÊME fonction pure que le superviseur — deux calculs
        // déterministes sur les mêmes entrées, jamais deux règles.
        let (largeur, hauteur) =
            crate::superviseur::placement::taille_retenue(taille_demandee, sortie_taille);
        let parametres = Parametres { hwnd, sortie, fps, debit, clock_origin };
        Ok(Fenetre { source: None, parametres, session, largeur, hauteur, pid })
    }
}
