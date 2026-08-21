//! Ce que la boucle retient d'un tour à l'autre, et la réconciliation qui le
//! produit : lire le disque, filtrer, comparer, mesurer les icônes.
//!
//! 🔴 CE MODULE EST UNE EXTRACTION VERBATIM, JOUÉE **AVANT** L'ADDITION QUI LA
//! RENDAIT NÉCESSAIRE. `apps/boucle.rs` valait 385 lignes ; le sondage de la
//! surveillance, son déclencheur, son mode et leur documentation l'auraient
//! porté au-delà de la porte de 450 lignes que ce sous-bloc s'impose. La forme
//! forte — extraire d'abord, ajouter ensuite — a été inventée par le sous-bloc
//! D9 (tâche 6) et jouée trois fois par D10 ; ce dépôt a payé **cinq** fois la
//! forme faible, « on franchit puis on rattrape », dont deux fois par une
//! COMPRESSION que `CLAUDE.md` interdit nommément.
//!
//! ⚠️ **LA SEULE DIFFÉRENCE AVEC LE TEXTE D'ORIGINE EST CET EN-TÊTE, CES `use`,
//! ET CINQ QUALIFICATEURS `pub(super)`** — sur `Memoire`, sur `reconcilier`, et
//! sur les trois champs que `boucle.rs` lit (`catalogue`, `lancables`,
//! `icones`). ⚠️ *Cette ligne a d'abord annoncé QUATRE, et le diff du contrôle
//! l'a réfutée en montrant cinq hunks : un compte écrit de mémoire au lieu
//! d'être lu dans la sortie de la commande, dans le fichier même dont
//! l'en-tête promet la complétude.* Ils ne sont pas un embellissement : en Rust un item privé d'un
//! module ENFANT n'est PAS visible de son parent, et une extraction
//! rigoureusement verbatim ne compilerait donc pas. La divergence est déclarée
//! plutôt que passée en douce, et le contrôle qui l'accompagne compare le corps
//! ligne à ligne pour qu'il n'en subsiste aucune autre.
//!
//! ⚠️ `mesurer` et les champs `vues` / `ecartes` restent PRIVÉS : ils ne
//! traversent pas la frontière, et les élargir « pour uniformiser » ouvrirait
//! une surface que personne ne demande.
//!
//! `mod memoire;` ORDINAIRE dans `boucle.rs`, **aucun `#[path]`** : les deux
//! sont `#[cfg(windows)]`, et la « Convention de module enfant » de
//! `CLAUDE.md` écrit noir sur blanc qu'un module gaté qui n'a pas besoin
//! d'exister sur l'hôte reste un enfant normal.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use proto::plateforme::{Application, SourceMax};

use crate::apps::icone::{self, magasin::Magasin};
use crate::apps::{lecture, raccourci, reconciliation};

/// Ce que la boucle retient d'un tour à l'autre.
#[derive(Default)]
pub(super) struct Memoire {
    /// Le catalogue du tour précédent, pour le diff.
    pub(super) catalogue: Vec<Application>,
    /// Le chemin du `.lnk` et le `nShow` de chaque clé, pour le lancement.
    ///
    /// ⚠️ C'EST LE CATALOGUE DE L'AGENT QUI FAIT AUTORITÉ POUR LANCER, jamais
    /// celui de la plateforme : l'ordre ne porte qu'une clé, et la copie de la
    /// plateforme peut être vieille d'une réconciliation quand celle-ci vient
    /// d'être lue sur le disque.
    pub(super) lancables: BTreeMap<String, (String, i32)>,
    /// 🔴 LES ICÔNES DU CATALOGUE COURANT, ADRESSÉES PAR CONTENU. Sur ce
    /// corpus, **153 applications rendent 99 PNG distincts** — 54
    /// téléversements évités. Il est REMPLACÉ à chaque réconciliation, jamais
    /// accumulé : sur un processus qui vit des jours, fusionner le ferait
    /// croître sans terme.
    pub(super) icones: Magasin,
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
pub(super) fn reconcilier(memoire: &mut Memoire) -> (reconciliation::Diff, Vec<Application>) {
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
