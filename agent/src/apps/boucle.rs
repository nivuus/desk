//! La boucle de réconciliation : lire, filtrer, comparer, émettre.
//!
//! 🔴 `#[cfg(windows)]` : elle ouvre COM et parcourt quatre arborescences
//! réelles. Tout ce qu'elle DÉCIDE vit pourtant dans `apps::raccourci` et
//! `apps::reconciliation`, qui sont purs et testés sur l'hôte — ce module ne
//! fait qu'orchestrer.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use proto::plateforme::{Application, IssueLancement, SourceMax, VersLaPlateforme};
use tokio::sync::{mpsc, watch};

use super::icone::{self, magasin::Magasin};
use super::{lancement, lecture, raccourci, reconciliation};
use crate::plateforme::{Identite, Ordre};

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
    /// 🔴 LES ICÔNES DU CATALOGUE COURANT, ADRESSÉES PAR CONTENU. Sur ce
    /// corpus, **153 applications rendent 99 PNG distincts** — 54
    /// téléversements évités. Il est REMPLACÉ à chaque réconciliation, jamais
    /// accumulé : sur un processus qui vit des jours, fusionner le ferait
    /// croître sans terme.
    icones: Magasin,
    /// L'empreinte et la provenance déjà connues d'une clé, plus le `chemin`
    /// du `.lnk` au moment où on les a mesurées.
    ///
    /// 🔴 C'EST CE QUI ÉVITE DE RÉEXTRAIRE À CHAQUE TOUR. L'extraction coûte
    /// **2 298 ms pour 153 icônes** au premier tour (mesuré) : la refaire
    /// toutes les trente secondes ferait de la découverte d'applications le
    /// poste le plus cher de l'agent, pour un disque qui ne bouge pas.
    ///
    /// ⚠️ **TROU NOMMÉ, PAS OUBLIÉ** : une application qui se met à jour en
    /// réécrivant son `.exe` EN PLACE — même chemin, même icône déclarée,
    /// image différente — ne sera PAS revue. Le fermer demanderait un
    /// horodatage ou une empreinte de la source, donc un accès disque par
    /// application et par tour.
    vues: BTreeMap<String, (String, Option<String>, SourceMax)>,
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
    // 🔴 LE LEGS N°7 DE G1, FERMÉ ICI. Le champ `retenus` émis plus bas valait
    // `lancables.len()` — une table indexée par CLÉ, donc TOUJOURS égale à
    // `cles`. Les raccourcis réellement retenus n'étaient émis NULLE PART, et
    // le champ mentait sur son nom.
    //
    // **Mesuré le 21 août 2026 sur la VM de développement : `retenus=167` pour
    // `cles=154`.** DEUX NOMBRES DIFFÉRENTS, donc un contrôle qui peut
    // échouer — c'est tout ce qu'on lui demande.
    //
    // ⚠️ **Le même relevé rend `169`/`156` en fin de recette G2**, et l'écart
    // n'est pas une dérive : la recette a créé DEUX raccourcis témoins sur le
    // Bureau (`G2 Temoin 48.lnk`, `G2 Temoin 256.lnk`) pour le critère ②, et
    // **ils y sont RESTÉS** — c'est ce qui rend ce critère rejouable sans rien
    // remonter. Un chantier suivant qui compterait 154 sur cette VM les
    // cherchera : ils portent des arguments distincts (`--g2-temoin-48` et
    // `--g2-temoin-256`), sans quoi leur clé serait la même et le catalogue
    // n'en garderait qu'un.
    let mut retenus = 0usize;
    let mut icones = Magasin::new();
    let mut vues = BTreeMap::new();
    let mut extraites = 0usize;
    let mut echecs_icone = 0usize;
    for r in brutes {
        match raccourci::retenir(&r.brut, &existe) {
            Ok(()) => {
                retenus += 1;
                let chemin_lnk = r.brut.chemin.clone();
                let icone_location = r.icone.clone();
                let mut app = raccourci::depuis_brut(r.brut);
                lancables.insert(app.cle.clone(), (chemin_lnk.clone(), r.montrer));
                if vus.insert(app.cle.clone()) {
                    // 🔴 EXTRAIRE SEULEMENT SI LA CLÉ EST NEUVE OU SI LE `.lnk`
                    // A CHANGÉ DE PLACE — jamais à chaque tour.
                    match memoire.vues.get(&app.cle) {
                        Some((ancien, empreinte, source)) if *ancien == chemin_lnk => {
                            app.icone = empreinte.clone();
                            app.source_max = *source;
                            // Les octets restent nécessaires : le magasin est
                            // remplacé à chaque tour, et la plateforme peut
                            // redemander une icône qu'elle a perdue.
                            if let Some(e) = empreinte {
                                if let Some(o) = memoire.icones.octets(e) {
                                    icones.ajouter(o.to_vec());
                                }
                            }
                        }
                        _ => {
                            let (e, s) = mesurer(&chemin_lnk, &icone_location, &app.cible, &mut icones);
                            // 🔴 UN DÉSARMEMENT N'EST PAS UN ÉCHEC, et les
                            // compter ensemble ferait lire 156 pannes sur un
                            // agent parfaitement sain qu'on vient de couper
                            // avec `ICONES=0` — MESURÉ le 21 août 2026, avant
                            // cette ligne. C'est exactement le défaut que le
                            // legs n°7 de G1 portait sur `retenus` : un
                            // compteur qui ment sur son nom.
                            if e.is_some() {
                                extraites += 1;
                            } else if icone::armee() {
                                echecs_icone += 1;
                            }
                            app.icone = e;
                            app.source_max = s;
                        }
                    }
                    vues.insert(
                        app.cle.clone(),
                        (chemin_lnk, app.icone.clone(), app.source_max),
                    );
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
        retenus,
        cles = catalogue.len(),
        icones = extraites,
        icones_echouees = echecs_icone,
        icones_distinctes = icones.len(),
        apparues = diff.apparues.len(),
        modifiees = diff.modifiees.len(),
        disparues = diff.disparues.len(),
        duree_ms = depart.elapsed().as_millis(),
        "catalogue reconcilie"
    );

    memoire.catalogue = catalogue.clone();
    memoire.lancables = lancables;
    memoire.ecartes = ecartes;
    memoire.vues = vues;
    // 🔴 REMPLACER, JAMAIS FUSIONNER : le magasin porte le catalogue COURANT.
    memoire.icones.remplacer(icones);
    (diff, catalogue)
}

/// Extrait l'icône d'un raccourci, et mesure sa PROVENANCE.
///
/// 🔴 `source_max` VIENT DE LA RESSOURCE, JAMAIS DU PNG. Un code qui la
/// déduirait de la taille rendue donnerait `256` à TOUT — mesuré deux fois sur
/// deux témoins fabriqués, et c'est tout l'objet du sous-bloc.
///
/// ⚠️ **UN ÉCHEC N'EST PAS UNE APPLICATION PERDUE** : une application sans
/// icône vaut mieux qu'une application absente (spécification §7). L'échec est
/// journalisé, `icone` vaut `None`, et `source_max` vaut `NonMesuree`.
fn mesurer(
    lnk: &str,
    icone_location: &str,
    cible: &str,
    icones: &mut Magasin,
) -> (Option<String>, SourceMax) {
    if !icone::armee() {
        return (None, SourceMax::NonMesuree);
    }
    match icone::extraire(std::path::Path::new(lnk)) {
        Ok(png) => {
            let octets = png.len();
            let empreinte = icones.ajouter(png);
            // ⚠️ `debug!` ET NON `info!`, ET C'EST MESURÉ : cette ligne sort
            // une fois PAR APPLICATION au premier tour — 153 lignes sur ce
            // corpus —, dans un journal que le superviseur, le capteur et tous
            // les enfants partagent depuis D4. Elle ne sort ensuite QUE pour
            // les clés neuves, donc zéro sur un disque au repos.
            //
            // 🔴 C'EST ELLE QUI REND LA DÉTERMINATION DE WIC MESURABLE : deux
            // exécutions séparées de l'agent, deux journaux, et les empreintes
            // se comparent chemin par chemin. Sans elle, la seule chose
            // observable serait `icones_distinctes`, qui ne dirait RIEN d'une
            // empreinte qui change d'un processus à l'autre — le compte
            // resterait le même.
            tracing::debug!(lnk, %empreinte, octets, "icone extraite");
            // ⚠️ LA PROVENANCE EST MESURÉE MÊME QUAND ELLE EST INCONNUE : elle
            // rend `NonMesuree` sans erreur, et ce n'est pas une panne — 37 des
            // 153 applications de cette VM sont dans ce cas.
            (Some(empreinte), icone::provenance_de(icone_location, cible))
        }
        Err(erreur) => {
            tracing::warn!(lnk, %erreur, "extraction d'icone echouee : l'application reste au catalogue, sans icone");
            (None, SourceMax::NonMesuree)
        }
    }
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
    mut ordres: mpsc::UnboundedReceiver<Ordre>,
    mut identite: watch::Receiver<Option<Identite>>,
    base_plateforme: String,
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
