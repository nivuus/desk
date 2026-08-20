//! La boucle de réconciliation : lire, filtrer, comparer, émettre.
//!
//! 🔴 `#[cfg(windows)]` : elle ouvre COM et parcourt quatre arborescences
//! réelles. Tout ce qu'elle DÉCIDE vit pourtant dans `apps::raccourci` et
//! `apps::reconciliation`, qui sont purs et testés sur l'hôte — ce module ne
//! fait qu'orchestrer.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use proto::plateforme::{Application, IssueLancement, VersLaPlateforme};
use tokio::sync::{mpsc, watch};

use super::{lancement, lecture, raccourci, reconciliation};
use crate::plateforme::Identite;

/// Ce que la boucle retient d'un tour à l'autre.
#[derive(Default)]
struct Memoire {
    /// Le catalogue du tour précédent, pour le diff.
    catalogue: Vec<Application>,
    /// Le chemin du `.lnk` et le `nShow` de chaque clé, pour le lancement.
    ///
    /// ⚠️ C'EST LE CATALOGUE DE L'AGENT QUI FAIT AUTORITÉ POUR LANCER, jamais
    /// celui de la plateforme : l'ordre ne porte qu'une clé, et la copie de la
    /// plateforme peut être vieille d'une réconciliation quand celle-ci vient
    /// d'être lue sur le disque.
    lancables: BTreeMap<String, (String, i32)>,
    /// Les chemins déjà signalés écartés.
    ///
    /// 🔴 SANS CET ENSEMBLE, LES SEPT ÉCARTS DE CETTE VM FERAIENT 20 160
    /// LIGNES PAR JOUR pour des fichiers qui ne changent pas — dans un journal
    /// partagé par le superviseur, le capteur et tous les enfants depuis D4.
    /// Une ligne est émise quand un chemin ENTRE dans cet ensemble, une autre
    /// quand il en SORT.
    ///
    /// ⚠️ CONSÉQUENCE POUR TOUTE RECETTE : le critère « sept lignes nommant
    /// chacune leur fichier » se mesure sur la PREMIÈRE réconciliation, dans
    /// une fenêtre temporelle explicite. Jamais sur un total de fichier.
    ecartes: BTreeSet<String>,
}

/// Une réconciliation complète : lire le disque, décider, rendre le diff.
///
/// Rend aussi le catalogue entier, parce que l'appelant en a besoin pour le
/// renvoi `complet` d'un réenrôlement.
fn reconcilier(memoire: &mut Memoire) -> (reconciliation::Diff, Vec<Application>) {
    let depart = Instant::now();
    let mut brutes = Vec::new();
    let mut total = 0usize;
    for racine in lecture::racines() {
        for chemin in lecture::lnk_sous(&racine) {
            total += 1;
            match lecture::lire(&chemin) {
                Ok(r) => brutes.push(r),
                // ⚠️ UN `.lnk` ILLISIBLE EST SAUTÉ AVEC SA TRACE. Une
                // réconciliation qui échouerait en entier sur un fichier
                // ferait disparaître TOUT le catalogue — un octet corrompu sur
                // le Bureau viderait la liste des applications.
                Err(erreur) => tracing::warn!(
                    chemin = %chemin.display(), %erreur, "raccourci illisible, sauté"
                ),
            }
        }
    }

    let existe = |c: &str| std::path::Path::new(c).is_file();
    let mut vus = BTreeSet::new();
    let mut catalogue = Vec::new();
    let mut lancables = BTreeMap::new();
    let mut ecartes = BTreeSet::new();
    for r in brutes {
        match raccourci::retenir(&r.brut, &existe) {
            Ok(()) => {
                let chemin_lnk = r.brut.chemin.clone();
                let app = raccourci::depuis_brut(r.brut);
                lancables.insert(app.cle.clone(), (chemin_lnk, r.montrer));
                if vus.insert(app.cle.clone()) {
                    catalogue.push(app);
                }
            }
            Err(motif) => {
                let chemin = r.brut.chemin.clone();
                if !memoire.ecartes.contains(&chemin) {
                    match &motif {
                        raccourci::Ecart::CibleVide => tracing::info!(
                            chemin = %chemin, motif = "cible-vide", "raccourci ecarte"
                        ),
                        raccourci::Ecart::Extension(e) => tracing::info!(
                            chemin = %chemin, motif = "extension", extension = %e,
                            "raccourci ecarte"
                        ),
                        raccourci::Ecart::CibleAbsente => tracing::info!(
                            chemin = %chemin, motif = "cible-absente", "raccourci ecarte"
                        ),
                    }
                }
                ecartes.insert(chemin);
            }
        }
    }
    for parti in memoire.ecartes.difference(&ecartes) {
        tracing::info!(chemin = %parti, "raccourci reintegre");
    }

    let diff = reconciliation::diff(&memoire.catalogue, &catalogue);
    tracing::info!(
        total,
        retenus = lancables.len(),
        cles = catalogue.len(),
        apparues = diff.apparues.len(),
        modifiees = diff.modifiees.len(),
        disparues = diff.disparues.len(),
        duree_ms = depart.elapsed().as_millis(),
        "catalogue reconcilie"
    );

    memoire.catalogue = catalogue.clone();
    memoire.lancables = lancables;
    memoire.ecartes = ecartes;
    (diff, catalogue)
}

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
    mut ordres: mpsc::UnboundedReceiver<(String, String)>,
    mut identite: watch::Receiver<Option<Identite>>,
    periode: std::time::Duration,
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
            match ordres.try_recv() {
                Ok((demande, cle)) => {
                    let issue = honorer(&memoire, &demande, &cle);
                    canal_emission(VersLaPlateforme::lancee(demande, issue));
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
