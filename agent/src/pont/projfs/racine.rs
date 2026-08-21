//! La racine de virtualisation : où elle vit, et comment elle est marquée
//! **une seule fois**.
//!
//! Extrait de [`super`] **avant** l'addition de la tâche 14, et non après :
//! `projfs.rs` était à 498 lignes, marge 2, et ce dépôt a payé quatre fois la
//! leçon « la marge regagnée par une extraction se reperd à la ronde suivante
//! si on la traite comme acquise ». Les deux fichiers que le sous-bloc D9 a
//! traités APRÈS coup ont été **compressés**, geste que `CLAUDE.md` interdit
//! nommément, puis extraits quand même.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use windows::core::{GUID, PCWSTR};

use super::chargement;

/// Le nom du dossier racine, dans le profil de l'utilisateur.
///
/// **ProjFS virtualise un RÉPERTOIRE, jamais un volume** : « lecteur Mes
/// Fichiers » est un nom d'usage, pas une lettre (décision D6). Aucune lettre
/// n'est attribuée — `subst`/`DefineDosDevice` auraient une durée de vie de
/// session d'ouverture, donc un mécanisme de plus à surveiller, à reposer après
/// un redémarrage et à retirer proprement, **pour zéro gain** : un dossier du
/// profil se navigue depuis un `IFileOpenDialog` exactement comme un volume, et
/// il paraît dans le volet de navigation de l'Explorateur sans travail.
const NOM_RACINE: &str = "Mes Fichiers";

/// Où le GUID d'instance est persisté, **hors de la racine**.
///
/// ⚠️ **Contrainte 3 de la spec §3.6, et elle n'est pas une préférence de
/// rangement** : un état qui vivrait dans la racine serait lui-même un objet
/// projeté — donc dépendant du pont pour être lu, ce qui est **circulaire** —
/// et il disparaîtrait avec la racine le jour où il faudrait la recréer,
/// c'est-à-dire **exactement le jour où il sert**.
const SOUS_DOSSIER_ETAT: &str = r"Guacamole\pont";
const FICHIER_GUID: &str = "instance.guid";


/// `%USERPROFILE%\Mes Fichiers`.
pub(super) fn racine() -> Result<PathBuf> {
    let profil = std::env::var("USERPROFILE")
        .context("USERPROFILE absent : impossible de situer la racine du pont fichiers")?;
    Ok(PathBuf::from(profil).join(NOM_RACINE))
}

/// `%LOCALAPPDATA%\Guacamole\pont`.
///
/// ⚠️ **`pub` depuis F2** : le **journal des écritures dues** y vit
/// aussi, pour la raison écrite en tête de [`SOUS_DOSSIER_ETAT`] — un état qui
/// vivrait DANS la racine serait lui-même un objet projeté, donc dépendant du
/// pont pour être lu, et il disparaîtrait avec la racine **exactement le jour
/// où il sert**.
pub fn dossier_etat() -> Result<PathBuf> {
    let local = std::env::var("LOCALAPPDATA")
        .context("LOCALAPPDATA absent : impossible de situer l'état du pont fichiers")?;
    Ok(PathBuf::from(local).join(SOUS_DOSSIER_ETAT))
}

/// Crée la racine si besoin, et la marque **une seule fois**.
///
/// ⚠️ **Les TROIS contraintes de la spec §3.6, et aucune n'est décorative :**
///
/// 1. **marquer une seule fois** — re-marquer une racine déjà marquée
///    **échoue**. Le GUID d'instance est donc persisté, et sa PRÉSENCE est ce
///    qui dit que la racine a déjà été marquée ;
/// 2. **la racine ne doit contenir aucune donnée au moment du marquage** — un
///    refus explicite vaut mieux qu'un `HRESULT` que personne ne saura lire ;
/// 3. **rien de notre état ne vit dans la racine** (voir [`SOUS_DOSSIER_ETAT`]).
pub(super) fn preparer(projfs: &chargement::ProjFs, racine: &Path) -> Result<()> {
    std::fs::create_dir_all(racine)
        .with_context(|| format!("création de la racine « {} »", racine.display()))?;
    let etat = dossier_etat()?;
    let empreinte = etat.join(FICHIER_GUID);

    let deja_marquee = empreinte.exists();
    let guid = match std::fs::read_to_string(&empreinte) {
        Ok(texte) => lire_guid(texte.trim()).with_context(|| {
            format!(
                "« {} » ne porte pas un GUID lisible : le retirer À LA MAIN ferait re-marquer \
                 une racine déjà marquée, ce que ProjFS refuse — il faut retirer la RACINE aussi",
                empreinte.display()
            )
        })?,
        // SÛRETÉ : `CoCreateGuid` n'a aucun préalable et ne prend aucun
        // pointeur ; elle ne peut échouer que par manque de ressource.
        Err(_) => unsafe { windows::Win32::System::Com::CoCreateGuid() }
            .context("génération du GUID d'instance de la racine")?,
    };

    if !deja_marquee {
        // Contrainte 2. Compté plutôt que supposé : une racine héritée d'une
        // exécution antérieure dont l'empreinte a été perdue contient
        // probablement des fichiers hydratés, et le marquage échouerait avec
        // un code que personne ne saurait rattacher à cette cause.
        let entrees = std::fs::read_dir(racine)
            .with_context(|| format!("lecture de la racine « {} »", racine.display()))?
            .count();
        if entrees != 0 {
            bail!(
                "la racine « {}» contient {entrees} entrée(s) : ProjFS refuse de marquer un \
                 répertoire non vide, et « {} » n'existe pas — vider la racine, ou restaurer \
                 l'empreinte de l'instance qui l'a marquée",
                racine.display(),
                empreinte.display()
            );
        }
    }

    let chemin = utf16(racine);
    // SÛRETÉ : `chemin` est valide et terminé par un nul ; les deux pointeurs
    // facultatifs sont nuls (aucun chemin cible, aucune information de
    // version) ; `guid` vit jusqu'à la fin de la fonction. Signature :
    // transcription du `link!` de `mod.rs:93`.
    let issue = unsafe {
        (projfs.marquer_racine)(PCWSTR(chemin.as_ptr()), PCWSTR::null(), std::ptr::null(), &guid)
    };
    if issue.is_err() {
        // ⚠️ **On ne teste PAS un `HRESULT` particulier, et c'est délibéré** :
        // le code exact que ProjFS rend pour « déjà marquée » n'a été mesuré
        // sur AUCUNE machine de ce dépôt, et l'inventer ferait exactement ce
        // que ce dépôt reproche à ses affirmations non relevées.
        //
        // ❌ **« Ce qui est connu, c'est que re-marquer ÉCHOUE » ÉTAIT FAUX, ET
        // F5 L'A MESURÉ (21 août 2026, porte P2).** Sur cette VM,
        // `PrjMarkDirectoryAsPlaceholder` **RÉUSSIT** sur une racine déjà
        // marquée dont l'empreinte existe : trois exécutions, dont une
        // (`p2-dues`) sans purge et **sans redémarrage entre les deux**, toutes
        // trois passées par la branche de succès.
        //
        // 🔴 **DEUX CONSÉQUENCES, et la seconde a coûté une mesure** :
        //
        // 1. **la branche ci-dessous n'a JAMAIS couru** — elle est tolérante et
        //    inoffensive, mais elle n'est pas éprouvée, et il ne faut pas la
        //    lire comme un chemin exercé ;
        // 2. **la trace de succès disait « marquée pour la PREMIÈRE fois » même
        //    quand la racine était déjà marquée**, puisqu'elle est le `else`
        //    d'un appel qui réussit toujours. La porte P2 de F5 lui a demandé
        //    si le marquage avait survécu à un redémarrage : *elle rendait la
        //    même phrase dans les trois cas*, donc elle ne pouvait pas
        //    répondre. Elle dit désormais ce qu'elle SAIT — si une empreinte
        //    existait déjà — plutôt que ce qu'elle suppose.
        //
        // ⚠️ **Le comportement n'est PAS changé** : tolérer l'échec quand
        // l'empreinte existe reste juste, et le rendre fatal sur la foi de
        // trois exécutions d'UNE machine serait exactement l'inverse de ce que
        // ce paragraphe reproche.
        if deja_marquee {
            tracing::info!(
                %issue,
                racine = %racine.display(),
                "marquage refusé sur une racine dont l'empreinte existe déjà : \
                 tenue pour déjà marquée (le code exact de « déjà marquée » n'est \
                 mesuré sur aucune machine de ce dépôt, donc il n'est pas testé)"
            );
        } else {
            bail!("PrjMarkDirectoryAsPlaceholder sur « {} » : {issue}", racine.display());
        }
    } else {
        std::fs::create_dir_all(&etat)
            .with_context(|| format!("création de « {} »", etat.display()))?;
        // Écrite APRÈS le marquage, jamais avant : une empreinte posée sur un
        // marquage qui échoue ferait tenir pour marquée une racine qui ne
        // l'est pas, et le démarrage suivant échouerait sans cause lisible.
        std::fs::write(&empreinte, format!("{guid:?}"))
            .with_context(|| format!("écriture de « {} »", empreinte.display()))?;
        // ⚠️ **`deja_marquee` EST RAPPORTÉ, et c'est ce qui rend cette trace
        // capable de répondre à une question.** Elle affirmait « première
        // fois » sans le savoir ; elle rend maintenant l'observation qui la
        // fonde, et l'interprétation reste au lecteur.
        tracing::info!(
            racine = %racine.display(),
            empreinte_preexistante = deja_marquee,
            "racine marquée (le marquage RÉUSSIT même sur une racine déjà marquée : mesuré, F5 P2)"
        );
    }
    Ok(())
}

/// Relit le GUID écrit par [`preparer`], au format `Debug` de
/// `windows::core::GUID`.
fn lire_guid(texte: &str) -> Result<GUID> {
    GUID::try_from(texte).with_context(|| format!("GUID « {texte} » illisible"))
}

/// Une chaîne UTF-16 terminée par un nul, pour un `PCWSTR`.
pub(super) fn utf16(chemin: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    chemin.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
}
