//! Les **treize** entrées de `ProjectedFSLib.dll` : leur nom, leur rang, et la
//! résolution elle-même. **PUR** — aucun `cfg`, aucune dépendance à `windows`,
//! entièrement testé sur l'hôte par un résolveur injecté.
//!
//! ⚠️ **Pourquoi ce module existe séparément de `pont/projfs/chargement.rs`.**
//! Le plan de F1 (tâche 12, step 4a) fait porter la garde « `charger()` échoue
//! si **une seule** des treize entrées manque, et l'erreur **nomme**
//! l'entrée » par un test de fumée `#[cfg(windows)]`, dont il écrit lui-même
//! qu'on ne le rend rouge qu'« en injectant un quatorzième nom bidon dans la
//! liste, une fois, puis en le retirant » — c'est-à-dire par une manipulation
//! manuelle, sur la VM, qu'aucune exécution ultérieure ne rejoue. Ce dépôt a
//! attrapé cinq contrôles incapables d'échouer, dont trois écrits par un plan.
//! La décision — qui est une décision, et pas une commodité — vit donc ici,
//! **pure**, et son test balaie les treize entrées une à une, à chaque
//! `cargo test`. Ce qui reste dans `chargement.rs` est ce qu'aucun test d'hôte
//! ne peut atteindre : `LoadLibraryW`, `GetProcAddress`, et les treize
//! `transmute`.
//!
//! ⚠️ **Ce module ne dit RIEN de l'ABI.** Il vérifie que treize noms existent
//! et à quel rang ; il ne peut pas vérifier qu'une signature transcrite à la
//! main correspond à celle de la DLL. C'est le risque R7 de la spec, et il
//! n'est pas couvert ici — voir l'en-tête de `pont/projfs/chargement.rs`.

/// Le nombre d'entrées. Ce n'est pas une commodité : il type [`NOMS`] et le
/// tableau que rend [`resoudre`], donc en ajouter une quatorzième sans
/// l'inscrire dans [`NOMS`] ne compile pas.
pub const NOMBRE: usize = 13;

/// Les treize noms exportés, **dans l'ordre des rangs ci-dessous**.
///
/// La liste est celle de la spec §4.3, relevée présente dans
/// `windows-0.62.2/src/Windows/Win32/Storage/ProjectedFileSystem/mod.rs` le
/// 19 août 2026. `PrjWritePlaceholderInfo2` et `PrjFillDirEntryBuffer2` en sont
/// **délibérément absentes** : d'une génération ultérieure, non vérifiées
/// présentes sur cette machine (spec §2.2), et inutiles à la v1 — elles servent
/// les liens symboliques, hors périmètre (spec §3.5.2).
pub const NOMS: [&str; NOMBRE] = [
    "PrjAllocateAlignedBuffer",
    "PrjClearNegativePathCache",
    "PrjCompleteCommand",
    "PrjDeleteFile",
    "PrjFileNameCompare",
    "PrjFileNameMatch",
    "PrjFillDirEntryBuffer",
    "PrjFreeAlignedBuffer",
    "PrjMarkDirectoryAsPlaceholder",
    "PrjStartVirtualizing",
    "PrjStopVirtualizing",
    "PrjWriteFileData",
    "PrjWritePlaceholderInfo",
];

/// Les treize adresses, **nommées**.
///
/// ⚠️ **Pourquoi une structure à champs nommés et non un `[usize; 13]` plus
/// treize constantes de rang.** La première rédaction faisait exactement cela,
/// et une mutation jouée après le vert l'a réfutée : échanger deux rangs au
/// site de construction de `ProjFs` (`adresses[COMPARER_NOMS]` contre
/// `adresses[APPARIER_NOM]`) **survivait** — le test des rangs épinglait bien
/// `NOMS[RANG]`, mais rien n'épinglait l'appariement CHAMP ↔ RANG au site
/// d'appel, qui est précisément là où la faute se commet. Deux entrées
/// échangées, ce sont deux `transmute` vers la mauvaise signature.
///
/// Avec des champs nommés, le seul appariement positionnel restant est
/// celui-ci, dans un module **pur** — et `chaque_champ_recoit_l_adresse_de_son_entree`
/// le balaie, les treize.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Adresses {
    pub allouer_tampon_aligne: usize,
    pub vider_cache_negatif: usize,
    pub completer_commande: usize,
    pub supprimer_fichier: usize,
    pub comparer_noms: usize,
    pub apparier_nom: usize,
    pub remplir_tampon_entrees: usize,
    pub rendre_tampon_aligne: usize,
    pub marquer_racine: usize,
    pub demarrer_virtualisation: usize,
    pub arreter_virtualisation: usize,
    pub ecrire_donnees: usize,
    pub ecrire_info_marqueur: usize,
}

/// Une entrée que la bibliothèque n'exporte pas.
///
/// **Elle porte le NOM**, pas un rang ni un compte : c'est ce qui distingue
/// « cette VM n'a pas ProjFS du tout » de « cette VM a une génération de
/// ProjectedFSLib antérieure à celle que la v1 attend ». Un message qui dirait
/// seulement « une entrée manque » laisserait les deux indiscernables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntreeManquante {
    pub nom: &'static str,
}

impl std::fmt::Display for EntreeManquante {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ProjectedFSLib.dll n'exporte pas « {} »", self.nom)
    }
}

impl std::error::Error for EntreeManquante {}

/// Résout les treize entrées par le `resolveur` fourni, et rend leurs adresses
/// **à leur rang**.
///
/// Le résolveur rend `None` — ou `Some(0)` — pour une entrée absente : c'est le
/// contrat de `GetProcAddress`, dont le `NULL` n'est pas une adresse.
/// Confondre les deux ferait `transmute` d'un pointeur nul en pointeur de
/// fonction, et le premier appel sauterait à l'adresse 0.
///
/// **Échec immédiat à la première manquante** : la suivante n'apprendrait rien
/// de plus, et c'est la PREMIÈRE que le journal doit nommer — une DLL d'une
/// génération antérieure nommerait sinon sa dernière entrée absente, et le
/// diagnostic partirait du mauvais bout.
pub fn resoudre(
    mut resolveur: impl FnMut(&str) -> Option<usize>,
) -> Result<Adresses, EntreeManquante> {
    let mut a = [0usize; NOMBRE];
    for (rang, nom) in NOMS.iter().enumerate() {
        match resolveur(nom) {
            Some(adresse) if adresse != 0 => a[rang] = adresse,
            _ => return Err(EntreeManquante { nom }),
        }
    }
    // ⚠️ **L'UNIQUE appariement positionnel du pont**, et il est ici, pur et
    // balayé par un test d'hôte — plutôt qu'au site des `transmute`, où rien
    // ne pourrait l'éprouver. L'ordre doit suivre celui de [`NOMS`].
    Ok(Adresses {
        allouer_tampon_aligne: a[0],
        vider_cache_negatif: a[1],
        completer_commande: a[2],
        supprimer_fichier: a[3],
        comparer_noms: a[4],
        apparier_nom: a[5],
        remplir_tampon_entrees: a[6],
        rendre_tampon_aligne: a[7],
        marquer_racine: a[8],
        demarrer_virtualisation: a[9],
        arreter_virtualisation: a[10],
        ecrire_donnees: a[11],
        ecrire_info_marqueur: a[12],
    })
}

#[cfg(test)]
mod tests;
