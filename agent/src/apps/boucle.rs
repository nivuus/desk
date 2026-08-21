//! La boucle de réconciliation : lire, filtrer, comparer, émettre.
//!
//! 🔴 `#[cfg(windows)]` : elle ouvre COM et parcourt quatre arborescences
//! réelles. Tout ce qu'elle DÉCIDE vit pourtant dans `apps::raccourci` et
//! `apps::reconciliation`, qui sont purs et testés sur l'hôte — ce module ne
//! fait qu'orchestrer.

use std::time::Instant;

use proto::plateforme::{IssueLancement, VersLaPlateforme};
use tokio::sync::{mpsc, watch};

use super::icone;
use super::{lancement, lecture};
use crate::plateforme::{Identite, Ordre};

/// Ce que la boucle retient d'un tour à l'autre, et la réconciliation qui le
/// produit — **EXTRAIT VERBATIM de ce fichier, AVANT l'addition qui l'exigeait.**
///
/// `mod` ORDINAIRE, sans `#[path]` : les deux sont `#[cfg(windows)]`, et la
/// « Convention de module enfant » de `CLAUDE.md` réserve le `#[path]` aux
/// modules qui doivent franchir une frontière `#[cfg]` pour exister sur l'hôte.
mod memoire;

use memoire::{reconcilier, Memoire};

/// Honore un ordre de lancement, et rend son issue.
fn honorer(memoire: &Memoire, demande: &str, cle: &str) -> IssueLancement {
    let Some((chemin, montrer)) = memoire.lancables.get(cle) else {
        // ⚠️ `Inconnue` EST RENDUE ICI ET NULLE PART AILLEURS : seul cet étage
        // connaît le catalogue. `apps::lancement` ne sait rien des clés.
        tracing::warn!(demande, cle, "clé absente du catalogue de l'agent");
        return IssueLancement::Inconnue;
    };
    let cible = memoire
        .catalogue
        .iter()
        .find(|a| a.cle == cle)
        .map(|a| a.cible.as_str())
        .unwrap_or_default();
    let issue = lancement::lancer(chemin, cible, *montrer);
    tracing::info!(demande, cle, ?issue, "lancement");
    issue
}

/// Le corps de la boucle, sur son fil dédié.
///
/// 🔴 UN FIL BLOQUANT DÉDIÉ, ET COM INITIALISÉ UNE SEULE FOIS DESSUS.
/// `IShellLinkW` et `ShellExecuteExW` exigent tous deux un appartement, et un
/// appartement appartient à SON fil : les deux doivent donc courir ici, pas
/// sur un fil de pool que tokio pourrait changer entre deux tours.
pub fn tourner(
    canal_emission: impl Fn(VersLaPlateforme) + Send + 'static,
    mut ordres: mpsc::UnboundedReceiver<Ordre>,
    mut identite: watch::Receiver<Option<Identite>>,
    base_plateforme: String,
    periode: std::time::Duration,
    partage: crate::apps::installation::partage::Partage,
) {
    if let Err(erreur) = lecture::initialiser_com() {
        tracing::error!(%erreur, "decouverte d'applications abandonnee : COM indisponible");
        return;
    }

    let mut memoire = Memoire::default();
    // 🔴 LE PREMIER TOUR EST COMPLET, ET CHAQUE RÉENRÔLEMENT AUSSI. Un
    // `Catalogue` perdu pendant une coupure laisserait sinon la plateforme
    // divergente SANS TERME — le canal est un `push` sans garantie de
    // livraison, et sa file abandonne ce qu'elle ne peut pas remettre.
    let mut complet = true;
    // L'identité courante est marquée lue : ce qui suit ne réagit qu'aux
    // CHANGEMENTS, et le premier envoi est déjà complet par la ligne ci-dessus.
    identite.borrow_and_update();

    loop {
        let (diff, catalogue) = reconcilier(&mut memoire);
        // 🔴 LE COMPTE VA À TOUTES LES FENÊTRES OUVERTES, ET IL SE PREND ICI —
        // avant que `diff.apparues` ne soit consommé par la branche du delta.
        //
        // ⚠️ IL EST VERSÉ MÊME QUAND LE CATALOGUE PART COMPLET : `complet` ne
        // change que ce qui est ÉMIS, jamais ce que le diff contient. La
        // mémoire n'est pas remise à zéro à un réenrôlement, donc
        // `diff.apparues` reste la vraie nouveauté — le compter deux fois
        // serait le défaut, ne pas le compter en serait un autre.
        if !diff.apparues.is_empty() {
            if let Ok(mut f) = partage.fenetres.lock() {
                f.ajouter(diff.apparues.len());
            }
        }
        // ⚠️ LE DRAPEAU SE BAISSE APRÈS LA RÉCONCILIATION, PAS AVANT : le fil
        // d'installation attend `reconciliee`, et le lever trop tôt lui ferait
        // lire un compte pris avant que l'installeur n'ait fini d'écrire.
        if partage.reconcilier.swap(false, std::sync::atomic::Ordering::SeqCst) {
            partage.reconciliee.store(true, std::sync::atomic::Ordering::SeqCst);
        }
        if complet {
            canal_emission(VersLaPlateforme::catalogue(true, catalogue, Vec::new()));
            complet = false;
        } else if !diff.est_vide() {
            // ⚠️ RIEN N'EST ÉMIS QUAND RIEN N'A BOUGÉ, et c'est tout l'intérêt
            // du diff : sur un disque au repos, la boucle est muette sur le
            // canal comme dans le journal des écarts.
            let mut applications = diff.apparues;
            applications.extend(diff.modifiees);
            canal_emission(VersLaPlateforme::catalogue(false, applications, diff.disparues));
        }

        // Attendre la période, en restant réactif aux ordres et aux
        // réenrôlements : dormir bêtement trente secondes ferait attendre un
        // clic d'utilisateur jusqu'à une demi-minute.
        let echeance = Instant::now() + periode;
        loop {
            let reste = echeance.saturating_duration_since(Instant::now());
            if reste.is_zero() {
                break;
            }
            // 🔴 « RÉCONCILIE MAINTENANT » COURT-CIRCUITE L'ATTENTE. Sans
            // cela, le verdict d'une installation de dix secondes arriverait
            // jusqu'à trente secondes plus tard, et l'utilisateur verrait une
            // barre finie devant un état « en cours ».
            if partage.reconcilier.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }
            match ordres.try_recv() {
                Ok(Ordre::Lancer { demande, cle }) => {
                    let issue = honorer(&memoire, &demande, &cle);
                    canal_emission(VersLaPlateforme::lancee(demande, issue));
                    continue;
                }
                Ok(Ordre::IconesManquantes { empreintes }) => {
                    icone::televersement::honorer(
                        &memoire.icones,
                        &empreintes,
                        &base_plateforme,
                        &identite,
                    );
                    continue;
                }
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    tracing::warn!("canal /agent fermé : découverte d'applications arrêtée");
                    return;
                }
                Err(mpsc::error::TryRecvError::Empty) => {}
            }
            if identite.has_changed().unwrap_or(false) {
                identite.borrow_and_update();
                tracing::info!("reenrolement observe : le prochain catalogue sera COMPLET");
                complet = true;
            }
            std::thread::sleep(reste.min(std::time::Duration::from_millis(200)));
        }
    }
}
