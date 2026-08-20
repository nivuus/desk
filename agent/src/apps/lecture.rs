//! Lecture d'un raccourci Windows par `IShellLinkW`, et découverte des quatre
//! racines par `SHGetKnownFolderPath`.
//!
//! 🔴 CE MODULE EST `#[cfg(windows)]` DANS SON ENTIER, ET IL NE PORTE AUCUNE
//! RÈGLE. Il rend un [`Brut`] et rien d'autre ; ce qui décide de le retenir
//! vit dans `apps::raccourci`, qui est pur et se juge sur l'hôte contre les
//! 218 raccourcis réels de la VM. La coupure est celle de la spec §6, pas un
//! arbitrage d'implémentation.
//!
//! ⚠️ IL N'EST VÉRIFIÉ QUE PAR `cargo check --target x86_64-pc-windows-gnu`,
//! qui couvre types, emprunts, visibilités et durées de vie — et PAS
//! l'édition de liens, la cible réelle étant `msvc`. Aucun test d'hôte ne peut
//! le couvrir.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use windows::core::{Interface, GUID, PCWSTR};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, IPersistFile, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED, STGM_READ,
};
use windows::Win32::System::Environment::ExpandEnvironmentStringsW;
use windows::Win32::UI::Shell::{
    IShellLinkW, SHGetKnownFolderPath, ShellLink, FOLDERID_CommonStartMenu, FOLDERID_Desktop,
    FOLDERID_PublicDesktop, FOLDERID_StartMenu, KF_FLAG_DEFAULT, SLGP_RAWPATH, SLGP_UNCPRIORITY,
};

use super::raccourci::Brut;

/// Les tampons de `IShellLinkW` : la documentation de l'interface ne borne
/// aucune de ces chaînes, mais un argument de raccourci ne peut de toute façon
/// pas dépasser la ligne de commande Windows (32 767 unités UTF-16). Le
/// tampon est donc dimensionné pour qu'aucune troncature silencieuse ne soit
/// possible, plutôt que sur `MAX_PATH` — que les chemins longs dépassent.
const TAMPON: usize = 32_768;

/// Rejoint l'appartement COM cloisonné du fil appelant.
///
/// 🔴 `APARTMENTTHREADED`, ET NON `MULTITHREADED` comme `wasapi.rs` : le Shell
/// et surtout `ShellExecuteExW` (`apps::lancement`) exigent une STA, et les
/// deux tournent sur le MÊME fil dédié. Choisir la MTA ici ferait échouer le
/// lancement plus tard, loin de sa cause.
///
/// 🔴 LE `HRESULT` EST CONTRÔLÉ, PAS IGNORÉ — précédent `wasapi.rs`. `S_OK`
/// (ce fil vient de rejoindre une STA) et `S_FALSE` (il en était déjà membre)
/// sont acceptables ; `RPC_E_CHANGED_MODE` signifie que ce fil appartient déjà
/// à un appartement d'un autre modèle, et poursuivre reviendrait à appeler des
/// vtables COM depuis le mauvais appartement sans marshaling — un comportement
/// indéfini qui « marche » la plupart du temps, donc qu'aucune exécution ne
/// révèle de façon fiable.
///
/// Pas de `CoUninitialize` en regard : ce fil vit aussi longtemps que la
/// boucle de réconciliation, et COM exige que la libération se fasse sur le
/// fil qui a initialisé — c'est le même raisonnement, et le même précédent,
/// qu'à `agent/src/wasapi.rs`.
pub fn initialiser_com() -> Result<()> {
    // SÉCURITÉ : appel FFI. Le seul contrat est que ce fil n'ait pas déjà
    // rejoint un appartement d'un autre modèle, ce que le contrôle ci-dessous
    // vérifie plutôt que de le supposer.
    let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    if hr.is_err() {
        bail!(
            "CoInitializeEx(APARTMENTTHREADED) refusé ({hr:?}) : le fil de découverte des \
             applications appartient déjà à un appartement COM d'un autre modèle. \
             `IShellLinkW` et `ShellExecuteExW` exigent tous deux une STA, et tournent \
             sur ce fil-ci."
        );
    }
    Ok(())
}

/// Lit un `.lnk` et rend ses cinq champs, plus le `nShow` que le lancement
/// réemploiera.
///
/// 🔴 `IShellLink::Resolve` N'EST APPELÉE NULLE PART, ET C'EST UNE DÉCISION.
/// Elle peut interroger le réseau, et surtout **déclencher l'installation à la
/// demande d'un raccourci MSI publié** — pendant une réconciliation de
/// routine, toutes les trente secondes, sans que personne ne l'ait demandé.
/// Le coût du refus est nommé : un raccourci dont la cible a bougé garde son
/// ancien chemin, et c'est le lancement qui le rattrape, par son repli.
pub fn lire(chemin: &Path) -> Result<Raccourci> {
    let chemin_w = vers_utf16(&chemin.to_string_lossy());

    // SÉCURITÉ : appels FFI. `CoCreateInstance` rend une interface comptée par
    // référence que `windows-rs` libère par `Drop`.
    let lien: IShellLinkW = unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }
        .context("CoCreateInstance(ShellLink)")?;
    let persistance: IPersistFile = lien.cast().context("IShellLinkW -> IPersistFile")?;
    unsafe { persistance.Load(PCWSTR(chemin_w.as_ptr()), STGM_READ) }
        .with_context(|| format!("IPersistFile::Load({})", chemin.display()))?;

    // `SLGP_RAWPATH` rend le chemin TEL QU'IL EST STOCKÉ, variables
    // d'environnement comprises et non développées : c'est ce qui évite
    // d'appeler `Resolve` implicitement. `SLGP_UNCPRIORITY` préfère le chemin
    // UNC au chemin de lecteur mappé, qui dépend de la session.
    //
    // ⚠️ Le troisième argument est un `u32` NU, alors que les constantes
    // vivent dans le newtype `SLGP_FLAGS(pub i32)` : la conversion est
    // explicite, et elle a été relue dans les bindings.
    let drapeaux = (SLGP_RAWPATH.0 | SLGP_UNCPRIORITY.0) as u32;
    let mut tampon = vec![0u16; TAMPON];
    // Le second argument peut être nul : nous ne voulons pas le
    // `WIN32_FIND_DATAW`, et le demander coûterait un accès disque par
    // raccourci — 218 par tour.
    let cible = match unsafe { lien.GetPath(&mut tampon, std::ptr::null_mut(), drapeaux) } {
        Ok(()) => developper(&depuis_utf16(&tampon)),
        // ⚠️ UNE CIBLE ILLISIBLE N'EST PAS UNE ERREUR : les cibles de l'espace
        // de noms Shell (Corbeille, « Ce PC », Panneau de configuration) n'ont
        // aucun chemin de fichier, et il y en a 7 sur cette VM. La règle de
        // filtrage les écarte par `Ecart::CibleVide` ; échouer ici les
        // transformerait en pannes de lecture.
        Err(_) => String::new(),
    };

    let mut tampon = vec![0u16; TAMPON];
    let arguments = match unsafe { lien.GetArguments(&mut tampon) } {
        Ok(()) => depuis_utf16(&tampon),
        Err(_) => String::new(),
    };

    let mut tampon = vec![0u16; TAMPON];
    let repertoire = match unsafe { lien.GetWorkingDirectory(&mut tampon) } {
        Ok(()) => developper(&depuis_utf16(&tampon)),
        Err(_) => String::new(),
    };

    // Le style de fenêtre que le raccourci demande — minimisé, maximisé,
    // normal. Le lancement le rejoue tel quel, pour qu'un double-clic dans
    // l'agent et un double-clic dans l'Explorateur donnent la même chose.
    let montrer = unsafe { lien.GetShowCmd() }.map(|c| c.0).unwrap_or(1);

    let nom = chemin
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    Ok(Raccourci {
        brut: Brut {
            nom,
            chemin: chemin.to_string_lossy().into_owned(),
            cible,
            arguments,
            repertoire,
        },
        montrer,
    })
}

/// Ce que la lecture rend : les cinq champs, plus le `nShow`.
///
/// `montrer` ne participe NI à l'identité NI au message : il ne sert qu'au
/// lancement, et le mettre dans [`Brut`] le ferait voyager sur le canal pour
/// rien.
pub struct Raccourci {
    pub brut: Brut,
    pub montrer: i32,
}

/// Les quatre racines de raccourcis, résolues par le système.
///
/// 🔴 JAMAIS DES CHEMINS LITTÉRAUX. La spec mesure
/// `C:\Users\Administrateur\Desktop` sur UNE machine : le coder rendrait la
/// découverte fausse sur toute autre installation, et muette à ce sujet.
///
/// ⚠️ UNE RACINE QUI ÉCHOUE À SE RÉSOUDRE EST SAUTÉE AVEC SA TRACE, JAMAIS
/// FATALE. `FOLDERID_StartMenu` peut ne pas exister sur un profil neuf, et une
/// réconciliation qui échouerait en entier sur une racine absente ferait
/// disparaître TOUT le catalogue — la spec §7 le dit nommément.
pub fn racines() -> Vec<PathBuf> {
    const RACINES: [(&str, GUID); 4] = [
        ("Bureau", FOLDERID_Desktop),
        ("Bureau public", FOLDERID_PublicDesktop),
        ("menu Démarrer", FOLDERID_StartMenu),
        ("menu Démarrer commun", FOLDERID_CommonStartMenu),
    ];
    let mut sortie = Vec::new();
    for (nom, id) in RACINES {
        match dossier_connu(&id) {
            Ok(chemin) if chemin.is_dir() => sortie.push(chemin),
            Ok(chemin) => tracing::warn!(
                racine = nom,
                chemin = %chemin.display(),
                "racine de raccourcis absente du disque, sautée"
            ),
            Err(erreur) => {
                tracing::warn!(racine = nom, %erreur, "racine de raccourcis non résolue, sautée")
            }
        }
    }
    sortie
}

fn dossier_connu(id: &GUID) -> Result<PathBuf> {
    // SÉCURITÉ : appel FFI. Le tampon rendu est alloué par le Shell, et c'est
    // à nous de le libérer par `CoTaskMemFree` — d'où la copie avant.
    let brut = unsafe { SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None) }
        .context("SHGetKnownFolderPath")?;
    if brut.is_null() {
        bail!("SHGetKnownFolderPath a rendu un pointeur nul");
    }
    let chemin = unsafe { brut.to_string() }.context("chemin de dossier connu non UTF-16 valide");
    unsafe { CoTaskMemFree(Some(brut.0 as *const _)) };
    Ok(PathBuf::from(chemin?))
}

/// Parcourt une racine RÉCURSIVEMENT et rend ses `.lnk`.
///
/// ⚠️ RÉCURSIF PARCE QUE LE MENU DÉMARRER EST ARBORESCENT : il porte 203 des
/// 218 raccourcis de cette VM, presque tous dans des sous-dossiers par
/// éditeur. Un parcours à plat n'en verrait quasiment aucun.
///
/// ⚠️ UN RÉPERTOIRE ILLISIBLE EST SAUTÉ AVEC SA TRACE, comme une racine.
pub fn lnk_sous(racine: &Path) -> Vec<PathBuf> {
    let mut sortie = Vec::new();
    let mut pile = vec![racine.to_path_buf()];
    while let Some(dossier) = pile.pop() {
        let entrees = match std::fs::read_dir(&dossier) {
            Ok(e) => e,
            Err(erreur) => {
                tracing::warn!(dossier = %dossier.display(), %erreur, "répertoire illisible, sauté");
                continue;
            }
        };
        for entree in entrees.flatten() {
            let chemin = entree.path();
            match entree.file_type() {
                // Les liens symboliques ne sont pas suivis : un lien qui
                // pointerait vers un ancêtre ferait boucler ce parcours sans
                // fin, toutes les trente secondes.
                Ok(t) if t.is_dir() => pile.push(chemin),
                Ok(t) if t.is_file() => {
                    let est_lnk = chemin
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("lnk"));
                    if est_lnk {
                        sortie.push(chemin);
                    }
                }
                _ => {}
            }
        }
    }
    sortie
}

/// Développe les variables d'environnement d'un chemin.
///
/// `SLGP_RAWPATH` rend le chemin tel qu'il est stocké — `%ProgramFiles%\…`
/// pour beaucoup de raccourcis d'installeurs. Sans ce développement, la règle
/// « le fichier cible existe » les écarterait tous, et la clé d'identité
/// dépendrait de la forme d'écriture plutôt que du fichier visé.
fn developper(valeur: &str) -> String {
    if !valeur.contains('%') {
        return valeur.to_string();
    }
    let source = vers_utf16(valeur);
    let mut tampon = vec![0u16; TAMPON];
    // SÉCURITÉ : appel FFI. Rend le nombre d'unités écrites, zéro en cas
    // d'échec — auquel cas on garde la valeur non développée plutôt que de
    // rendre une chaîne vide, qui se lirait comme « cible de l'espace de noms
    // Shell » et changerait le motif d'écart.
    let ecrit = unsafe { ExpandEnvironmentStringsW(PCWSTR(source.as_ptr()), Some(&mut tampon)) };
    if ecrit == 0 {
        return valeur.to_string();
    }
    depuis_utf16(&tampon)
}

fn vers_utf16(valeur: &str) -> Vec<u16> {
    valeur.encode_utf16().chain(std::iter::once(0)).collect()
}

/// ⚠️ S'ARRÊTE AU PREMIER NUL. Les tampons de `IShellLinkW` ne sont pas
/// remplis : ils portent une chaîne terminée par un nul suivie de 32 000
/// zéros, et une conversion naïve rendrait une chaîne de 32 768 caractères
/// dont l'égalité et l'empreinte seraient fausses.
fn depuis_utf16(tampon: &[u16]) -> String {
    let fin = tampon.iter().position(|&c| c == 0).unwrap_or(tampon.len());
    String::from_utf16_lossy(&tampon[..fin])
}
