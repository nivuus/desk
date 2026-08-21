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

use super::surveillance::mode::Mode;
use super::surveillance::partage::Veille;
use super::surveillance::rebond::Rebond;

/// Ce qui a fait partir CETTE réconciliation.
///
/// 🔴 IL EST PORTÉ SUR LA LIGNE `catalogue reconcilie`, ET C'EST CE QUI REND LE
/// CRITÈRE ① LISIBLE : sans lui, une réconciliation arrivée dans la seconde qui
/// suit la création d'un raccourci est indiscernable d'une réconciliation
/// périodique qui serait tombée là par hasard. **Un chiffre-juge doit dire d'où
/// il vient.**
///
/// ✅ Aucun instrument versé de ce dépôt ne lit cette trace — vérifié par
/// `grep -rn "catalogue reconcilie"` sur les `.mjs`, `.js`, `.ts`, `.sh` et
/// `.ps1`, qui rend AUCUNE ligne. Lui ajouter des champs ne casse donc rien.
#[derive(Debug, Clone, Copy)]
pub(super) enum Declencheur {
    /// Le tout premier tour. Il est `complet` par construction, et c'est lui
    /// qui rattrape tout ce qui a changé **agent arrêté**.
    Demarrage,
    /// L'échéance de `PERIODE_RECONCILIATION`. **La source de vérité.**
    Periode,
    /// La surveillance a vu bouger quelque chose. **Un ACCÉLÉRATEUR, et rien
    /// d'autre** : ce tour ne fait pas moins de travail qu'un tour périodique,
    /// il arrive plus tôt.
    Notification,
    /// La sortie d'un installeur a levé `partage.reconcilier` (sous-bloc G3).
    Installation,
}

impl Declencheur {
    fn mot(self) -> &'static str {
        match self {
            Declencheur::Demarrage => "demarrage",
            Declencheur::Periode => "periode",
            Declencheur::Notification => "notification",
            Declencheur::Installation => "installation",
        }
    }
}

/// Ce que la ligne `catalogue reconcilie` porte en plus du catalogue lui-même.
pub(super) struct Contexte {
    pub(super) declencheur: Declencheur,
    /// Cumulé depuis le démarrage du fil de surveillance, jamais un delta :
    /// deux lignes successives se soustraient, un delta déjà pris ne se
    /// recompose pas.
    pub(super) notifications: u64,
    pub(super) debordements: u64,
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
    partage: crate::apps::installation::partage::Partage,
    veille: Veille,
    mode: Mode,
) {
    if let Err(erreur) = lecture::initialiser_com() {
        tracing::error!(%erreur, "decouverte d'applications abandonnee : COM indisponible");
        return;
    }

    let mut memoire = Memoire::default();
    // 🔴 LE PREMIER TOUR EST COMPLET, ET CHAQUE CHANGEMENT D'IDENTITÉ AUSSI. Un
    // `Catalogue` perdu pendant une coupure laisserait sinon la plateforme
    // divergente SANS TERME — le canal est un `push` sans garantie de
    // livraison, et sa file abandonne ce qu'elle ne peut pas remettre.
    //
    // ⚠️ **EN PRATIQUE, L'ENVOI COMPLET EST PÉRIODIQUE — MESURÉ, ET CE N'ÉTAIT
    // ÉCRIT NULLE PART.** Sur le journal d'un agent au repos, le 21 août 2026 :
    // ONZE lignes « le prochain catalogue sera COMPLET » pour DOUZE
    // réconciliations, et **aucun réenrôlement n'a eu lieu**. C'est le
    // rafraîchissement de jeton du battement qui fait bouger la `watch`, et
    // `PERIODE_BATTEMENT` vaut exactement `PERIODE_RECONCILIATION`. **Le
    // catalogue complet part donc sur le fil toutes les trente secondes, pour
    // toujours, sur un disque qui ne bouge pas.**
    //
    // 🔴 ET CE N'EST PAS CORRIGÉ, POUR UNE RAISON QUI EST L'INVERSE DE CE QU'ON
    // CROIRAIT. Cette boucle POURRAIT distinguer les deux — un réenrôlement
    // change le `prefixe`, un battement ne change que le `jeton` — et ne lever
    // `complet` que sur le premier. Mais ce serait **retirer une réparation
    // réelle** : la promesse écrite trois lignes plus haut, « un `Catalogue`
    // perdu ne laisse pas la plateforme divergente sans terme », n'a AUCUNE
    // autre implémentation que cet envoi complet périodique. Ce qui a l'air
    // d'un défaut est la seule chose qui tienne la garantie que ce commentaire
    // annonce.
    //
    // ⛔ **LEGS, ET IL EST NOMMÉ** : arbitrer entre ce filet et son coût demande
    // de MESURER LA TRAME SUR LE FIL, ce que G4 ne fait pas. Le seul chiffre
    // disponible est celui de G1 — **56 145 octets** pour **154** applications,
    // et **SANS** les champs `icone` et `source_max` que G2 a ajoutés. Celui
    // d'aujourd'hui porte **156** applications **avec** les deux : il est plus
    // gros, et **il n'est mesuré par rien**.
    let mut complet = true;
    // L'identité courante est marquée lue : ce qui suit ne réagit qu'aux
    // CHANGEMENTS, et le premier envoi est déjà complet par la ligne ci-dessus.
    identite.borrow_and_update();

    let mut rebond = Rebond::default();
    // 🔴 LA VALEUR RETENUE, ET NON UN DRAPEAU. `Veille::notifications` est
    // MONOTONE : la boucle compare au compte qu'elle avait la dernière fois.
    // Une notification survenue PENDANT une réconciliation est donc vue au
    // sondage suivant — ce qu'un booléen échangé perdrait, et c'est exactement
    // la notification qui compte, celle qui arrive quand on lit déjà le disque.
    let mut notifications_vues = veille.notifications();
    let mut declencheur = Declencheur::Demarrage;

    loop {
        // 🔴 E5 — LE DRAPEAU SE LIT ET SE BAISSE **AVANT** LA RÉCONCILIATION.
        //
        // ⚠️ CE COMMENTAIRE EN REMPLACE UN QUI NOMMAIT EXACTEMENT LE DÉFAUT QUE
        // SON PROPRE CODE PRODUISAIT. Il disait : « le drapeau se baisse APRÈS
        // la réconciliation, pas avant : le fil d'installation attend
        // `reconciliee`, et le lever trop tôt lui ferait lire un compte pris
        // avant que l'installeur n'ait fini d'écrire. » Le chemin NOMINAL était
        // correct — le drapeau se lève à la sortie de l'installeur, l'attente
        // est rompue, et la réconciliation qui suit a bien commencé APRÈS.
        //
        // 🔴 LE CHEMIN DE COURSE NE L'ÉTAIT PAS. Si une réconciliation
        // périodique était DÉJÀ EN COURS quand l'installeur sortait et levait
        // le drapeau, le `swap` posé après la voyait vrai et déclarait
        // `reconciliee` — **pour une réconciliation commencée AVANT que
        // l'installeur n'ait fini d'écrire**. `forcer_une_reconciliation`
        // rendait alors la main immédiatement, et le verdict se lisait sur une
        // fenêtre qui n'avait pas vu les derniers fichiers : un `sans-effet`
        // FAUX, c'est-à-dire précisément ce que l'ancien commentaire disait
        // vouloir empêcher.
        //
        // 🔴 POURQUOI C'EST DE G4 : la fenêtre de course valait « durée de la
        // réconciliation / période », soit ≈ 0,2 % des sorties d'installeur.
        // G4 rend les réconciliations bien plus fréquentes pendant une
        // installation — c'est tout son objet — et ÉLARGIT donc cette fenêtre
        // d'un ordre de grandeur.
        //
        // Lu avant, la demande SURVIT au tour en cours : la réconciliation
        // SUIVANTE l'honore, une période plus tard mais JUSTE. Aucun des trois
        // chemins ne perd la demande.
        //
        // ⚠️ SA SEULE PREUVE EST UN ARGUMENT DE FLOT DE CONTRÔLE. Ce fichier est
        // `#[cfg(windows)]`, aucun test d'hôte ne peut l'atteindre, et la
        // recette de G4 ne lance aucune installation. C'est la situation exacte
        // que le défaut F1 du sous-bloc D7 a payée. Le correctif est appliqué
        // parce qu'il est STRICTEMENT PLUS SÛR ; **la mesure est LÉGUÉE, et
        // déclarée manquante.**
        let demandee = partage
            .reconcilier
            .swap(false, std::sync::atomic::Ordering::SeqCst);
        let contexte = Contexte {
            declencheur,
            notifications: veille.notifications(),
            debordements: veille.debordements(),
        };
        let (diff, catalogue) = reconcilier(&mut memoire, contexte);
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
        // La demande lue AVANT le tour est honorée APRÈS lui : c'est bien
        // cette réconciliation-ci, commencée après la sortie de l'installeur,
        // que `reconciliee` annonce.
        if demandee {
            partage
                .reconciliee
                .store(true, std::sync::atomic::Ordering::SeqCst);
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

        // Attendre la période, en restant réactif aux ordres, aux
        // réenrôlements ET à la surveillance : dormir bêtement trente secondes
        // ferait attendre un clic d'utilisateur jusqu'à une demi-minute.
        //
        // 🔴 EN `Mode::Seule`, L'ÉCHÉANCE DE PÉRIODE N'EXISTE PAS. C'est un
        // `Option`, et non une période démesurément longue : une période de
        // mille ans serait un mensonge que le code porterait, et le jour où
        // quelqu'un la lirait il croirait à un réglage. **Variable de BANC, et
        // le seul montage qui rende le critère ③ discriminant** — un agent
        // purement événementiel perd tout ce qu'une notification manquée
        // emporte.
        let echeance = mode.periodique().then(|| Instant::now() + periode);
        loop {
            let maintenant = Instant::now();
            if let Some(echeance) = echeance {
                if echeance.saturating_duration_since(maintenant).is_zero() {
                    declencheur = Declencheur::Periode;
                    break;
                }
            }
            // 🔴 « RÉCONCILIE MAINTENANT » COURT-CIRCUITE L'ATTENTE. Sans
            // cela, le verdict d'une installation de dix secondes arriverait
            // jusqu'à trente secondes plus tard, et l'utilisateur verrait une
            // barre finie devant un état « en cours ».
            if partage.reconcilier.load(std::sync::atomic::Ordering::SeqCst) {
                declencheur = Declencheur::Installation;
                break;
            }
            // 🔴 LE SONDAGE DE LA SURVEILLANCE, DANS L'ATTENTE QUI EXISTAIT
            // DÉJÀ : la boucle ne gagne AUCUN fil. C'est ce qui fait de G4 une
            // accélération et non une architecture de plus.
            let notifications = veille.notifications();
            if notifications != notifications_vues {
                notifications_vues = notifications;
                if mode.rebond() {
                    rebond.notifier(maintenant);
                } else {
                    // `sans-rebond` : toute notification rompt l'attente.
                    // ⚠️ CE N'EST PAS « L'ANTI-REBOND CONTRE RIEN » : le
                    // sondage a une granularité de 200 ms, qui est déjà un
                    // anti-rebond faible. Le ROUGE du critère ④ mesure donc
                    // « anti-rebond contre 200 ms », et son énoncé doit le
                    // dire.
                    declencheur = Declencheur::Notification;
                    break;
                }
            }
            if rebond.du(maintenant) {
                rebond.consommer();
                declencheur = Declencheur::Notification;
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
                // 🔴 D7 — CETTE TRACE MENTAIT SUR SON NOM, ET C'EST LA
                // TROISIÈME FOIS DANS CE SOUS-PROJET. Elle disait
                // « reenrolement observe » ; **aucun réenrôlement n'a lieu**.
                // MESURÉ le 21 août 2026 sur le journal d'un agent au repos :
                // ONZE lignes pour DOUZE réconciliations. Ce qui fait bouger la
                // `watch`, c'est le RAFRAÎCHISSEMENT DE JETON du battement — et
                // `PERIODE_BATTEMENT` vaut exactement `PERIODE_RECONCILIATION`,
                // **par coïncidence et non par dérivation** : ce sont deux
                // constantes indépendantes, qui se recalibreraient séparément.
                //
                // Après `retenus` (leg n°7 de G1) et `icones_echouees` (défaut
                // ② de G2), c'est le troisième compteur de ④ à mentir sur son
                // nom. Elle dit désormais ce qu'elle OBSERVE, et déclare ce
                // qu'elle ne sait pas distinguer.
                tracing::info!(
                    "identité changée : le prochain catalogue sera COMPLET \
                     (rafraîchissement de jeton OU réenrôlement — cette boucle ne les distingue pas)"
                );
                complet = true;
            }
            // 🔴 LA GRANULARITÉ DU SONDAGE EST DE 200 ms, ET C'EST ELLE QUI
            // BORNE PAR LE BAS TOUT ANTI-REBOND — un délai plus court ne serait
            // pas observable, il serait absorbé ici. Elle est aussi le deuxième
            // terme du pire cas dérivé de `DELAI_ANTI_REBOND_MAX`.
            //
            // L'attente s'écourte sur l'échéance la plus proche des trois :
            // la période, le rebond, et ce pas. Sans le rebond dans ce `min`,
            // une échéance d'anti-rebond tombant juste après un réveil
            // attendrait 200 ms de plus — mesurable, et gratuit à éviter.
            let mut attente = std::time::Duration::from_millis(200);
            if let Some(echeance) = echeance {
                attente = attente.min(echeance.saturating_duration_since(maintenant));
            }
            if let Some(du) = rebond.echeance() {
                attente = attente.min(du.saturating_duration_since(maintenant));
            }
            // ⚠️ JAMAIS ZÉRO : une attente nulle ferait tourner ce fil à vide
            // sur un cœur entier. Les deux échéances ci-dessus sont testées en
            // tête de boucle, donc une attente nulle signifie « échue à l'instant
            // même », et une milliseconde suffit à rendre la main.
            std::thread::sleep(attente.max(std::time::Duration::from_millis(1)));
        }
    }
}
