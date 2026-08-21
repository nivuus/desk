//! Le fil de surveillance : **un seul**, quatre racines, une attente.
//!
//! 🔴 `#[cfg(windows)]`, et **aucun test d'hôte n'est possible** — même statut
//! que `racine.rs`, et pour la même raison.
//!
//! **UN SEUL FIL, ET NON QUATRE.** Ce qui a été écarté, avec sa raison :
//!
//! - **quatre fils bloquants** (`ReadDirectoryChangesW` synchrone) : quatre
//!   fils pour attendre, et **aucun moyen de les arrêter proprement** — un fil
//!   bloqué dans un appel synchrone ne voit pas un drapeau d'arrêt ;
//! - **un port de complétion** : plus de machinerie que quatre handles n'en
//!   justifient, et une seconde façon d'attendre dans un processus qui en a
//!   déjà quatre ;
//! - **`SHChangeNotifyRegister`** : écarté par la spécification elle-même, et
//!   pour de bonnes raisons — un `HWND`, une pompe de messages, et des PIDL à
//!   re-résoudre.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Instant;

use windows::Win32::Foundation::{WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows::Win32::System::Threading::WaitForMultipleObjects;

use super::partage::Veille;
use super::racine::{Issue, Racine};

/// Le pas de l'attente.
///
/// 🔴 IL EXISTE **POUR QUE LE DRAPEAU D'ARRÊT SOIT VU**, et pour rendre la main
/// aux replis échus. **Il ne sonde rien** : les notifications arrivent par les
/// événements, jamais par ce délai. Le confondre avec un intervalle de sondage
/// ferait croire que le raccourcir accélère la détection — il ne ferait que
/// réveiller un fil qui n'a rien à faire.
const PAS_ATTENTE_MS: u32 = 1_000;

/// Le corps du fil.
pub(super) fn tourner(veille: Veille) {
    // 🔴 DÉDUPLICATION. `lecture::racines()` ne déduplique pas, et rien ne
    // garantit que les quatre dossiers connus soient distincts sur une
    // configuration inhabituelle. Ouvrir deux fois le même répertoire
    // gaspillerait 64 Kio de pool NON PAGINÉ pour compter chaque événement
    // deux fois.
    //
    // ⚠️ `racines()` N'EST PAS TOUCHÉE : elle est partagée avec la
    // réconciliation, où les doublons sont déjà inoffensifs
    // (`vus.insert(app.cle)`). La déduplication est un besoin de CE
    // consommateur, et elle vit chez lui.
    let distinctes: BTreeSet<PathBuf> = crate::apps::lecture::racines().into_iter().collect();
    if distinctes.is_empty() {
        tracing::warn!(
            "surveillance des raccourcis inactive : aucune racine résolue \
             (la réconciliation périodique, elle, continue)"
        );
        return;
    }

    let mut racines: Vec<Racine> = Vec::new();
    for chemin in distinctes {
        match Racine::ouvrir(chemin.clone()) {
            Ok(racine) => {
                tracing::info!(racine = %chemin.display(), "racine surveillée");
                racines.push(racine);
            }
            // ⚠️ TROU NOMMÉ, NON FERMÉ : une racine ABSENTE au démarrage n'a
            // pas de handle, donc pas d'erreur de complétion, donc aucune
            // tentative de réouverture. Si elle apparaît plus tard, **elle
            // n'est jamais surveillée**, et seule la réconciliation périodique
            // la voit. C'est de la LATENCE, jamais une perte — et le fermer
            // demanderait un minuteur de re-résolution que rien ne justifie
            // aujourd'hui.
            Err(erreur) => tracing::warn!(
                racine = %chemin.display(), %erreur,
                "racine non surveillée : la réconciliation périodique reste la source de vérité"
            ),
        }
    }
    if racines.is_empty() {
        tracing::warn!("surveillance des raccourcis inactive : aucune racine ouverte");
        return;
    }
    tracing::info!(racines = racines.len(), "surveillance des raccourcis armée");

    boucler(&mut racines, &veille);

    for racine in &mut racines {
        racine.fermer();
    }
    tracing::info!("surveillance des raccourcis arrêtée");
}

fn boucler(racines: &mut [Racine], veille: &Veille) {
    loop {
        if veille.arretee() {
            return;
        }
        // Les racines VIVANTES seulement : une racine en échec n'a plus de
        // handle valide, et l'inclure ferait rendre `WAIT_FAILED` à l'attente
        // entière — une racine morte emporterait les trois autres.
        let vivantes: Vec<usize> = (0..racines.len()).filter(|i| !racines[*i].en_echec()).collect();
        if vivantes.is_empty() {
            // Toutes en échec : il n'y a rien à attendre, mais il y a des
            // replis à faire échoir. Dormir le pas de l'attente est
            // exactement ce que ferait le `WaitForMultipleObjects` s'il
            // acceptait zéro handle — il ne l'accepte pas.
            std::thread::sleep(std::time::Duration::from_millis(u64::from(PAS_ATTENTE_MS)));
            reprendre_les_echues(racines);
            continue;
        }
        let handles: Vec<_> = vivantes.iter().map(|i| racines[*i].evenement()).collect();
        // SÉCURITÉ : appel FFI. `bWaitAll = false` : on veut la PREMIÈRE
        // racine signalée, pas les quatre.
        let issue = unsafe { WaitForMultipleObjects(&handles, false, PAS_ATTENTE_MS) };
        if issue == WAIT_TIMEOUT {
            reprendre_les_echues(racines);
            continue;
        }
        if issue == WAIT_FAILED {
            // 🔴 UNE SEULE LIGNE, PUIS ON QUITTE. Un fil qui boucle sur un
            // échec d'attente est un fil qui brûle un cœur EN SILENCE — et
            // dans un journal partagé par le superviseur, le capteur et tous
            // les enfants, il l'inonderait aussi.
            // SÉCURITÉ : appel FFI. `GetLastError` n'a aucune précondition ;
            // il est `unsafe` parce que sa valeur n'a de sens qu'ici, tout de
            // suite après l'appel qui a échoué.
            let code = unsafe { windows::Win32::Foundation::GetLastError() };
            tracing::error!(
                erreur = %windows::core::Error::from_hresult(code.to_hresult()),
                "attente de surveillance en échec : fil arrêté (la réconciliation périodique continue)"
            );
            return;
        }
        let rang = (issue.0 - WAIT_OBJECT_0.0) as usize;
        let Some(&indice) = vivantes.get(rang) else {
            tracing::error!(?issue, "attente de surveillance : rang hors des racines, fil arrêté");
            return;
        };
        servir(&mut racines[indice], veille);
    }
}

/// Complète une racine signalée, la réarme, et publie ce qu'elle a dit.
fn servir(racine: &mut Racine, veille: &Veille) {
    match racine.completer() {
        // 🔵 AVALÉE : ni comptée, ni journalisée, ni déclenchante — et la
        // racine est tout de même RÉARMÉE, sans quoi l'injection d'une seule
        // faute arrêterait la surveillance au lieu de lui faire manquer une
        // complétion.
        Issue::Avalee => rearmer(racine),
        Issue::Notification => {
            veille.signaler();
            rearmer(racine);
        }
        Issue::Debordement => {
            veille.signaler_debordement();
            // 🔴 PAS DE LIMITATION DE DÉBIT SUR CE `warn!`, ET C'EST RAISONNÉ.
            // Un débordement exige plus d'un millier d'événements entre deux
            // réarmements, c'est-à-dire dans les microsecondes qui les
            // séparent : il est RARE PAR CONSTRUCTION. Limiter son débit
            // cacherait exactement le cas pathologique qu'on voudrait voir.
            // ✅ **« RARE PAR CONSTRUCTION » EST DEVENU UNE MESURE** : sur
            // sept exécutions de la porte S1 (jusqu'à 60 000 fichiers à
            // 2 850/s) et deux rafales sur le produit (~96 000 notifications
            // réelles chacune), cette ligne n'est sortie **AUCUNE fois**. Le
            // seul relevé qui la montre est sous injection
            // (`APPS_FAUTE=debordement:3` : exactement trois lignes).
            // ⚠️ Si une recette relève un jour un déluge, c'est un RÉSULTAT :
            // il se consigne, et la limitation se lègue — elle ne s'ajoute pas
            // en catastrophe.
            tracing::warn!(
                racine = %racine.chemin().display(),
                debordements = veille.debordements(),
                "notifications perdues : tampon de surveillance débordé, \
                 la réconciliation qui suit relit le disque entier"
            );
            rearmer(racine);
        }
        Issue::Annulee => {}
        Issue::Perte(erreur) => {
            // 🔴 UNE LIGNE PAR TRANSITION, JAMAIS UNE PAR TENTATIVE. C'est le
            // patron de l'ensemble `ecartes` de `apps/boucle.rs`, dont le
            // commentaire chiffre ce qu'il évite : « sans cet ensemble, les
            // sept écarts de cette VM feraient 20 160 lignes par jour ».
            if !racine.en_echec() {
                tracing::warn!(
                    racine = %racine.chemin().display(), %erreur,
                    "racine de surveillance PERDUE : réouverture programmée \
                     (la réconciliation périodique reste la source de vérité)"
                );
            }
            racine.programmer_la_reprise(Instant::now());
        }
    }
}

fn rearmer(racine: &mut Racine) {
    if let Err(erreur) = racine.armer() {
        if !racine.en_echec() {
            tracing::warn!(
                racine = %racine.chemin().display(), %erreur,
                "réarmement de surveillance en échec : réouverture programmée"
            );
        }
        racine.programmer_la_reprise(Instant::now());
    }
}

/// Tente la réouverture des racines en échec **dont le repli est échu**, et
/// rien d'autre.
fn reprendre_les_echues(racines: &mut [Racine]) {
    let maintenant = Instant::now();
    for racine in racines.iter_mut() {
        if !racine.reprise_due(maintenant) {
            continue;
        }
        match racine.rouvrir() {
            Ok(()) => tracing::warn!(
                racine = %racine.chemin().display(),
                "racine de surveillance RÉTABLIE"
            ),
            // ⚠️ AUCUNE LIGNE ICI : la racine était déjà en échec, et son
            // entrée en échec a déjà été dite. Une ligne par tentative ferait
            // 2 880 lignes par jour et par racine au plafond de repli.
            Err(_) => racine.programmer_la_reprise(maintenant),
        }
    }
}
