//! La correspondance entre l'identifiant de cible que rend le pilote et le
//! nom GDI (`\\.\DISPLAYn`) que DXGI énumère.
//!
//! 🔴 **CE MODULE EXISTE PARCE QU'UNE AFFIRMATION DE CE DÉPÔT ÉTAIT FAUSSE.**
//! `superviseur/placement.rs` écrivait depuis D1 : « Le premier rend un
//! identifiant de cible qui lui appartient, le second énumère par
//! `(index_adaptateur, index_sortie)`. **Aucune correspondance n'est
//! exposée** : l'appariement se fait donc par dimensions et par élimination. »
//! Une correspondance est exposée, par l'API CCD de Win32 (*Connecting and
//! Configuring Displays*), et c'est la conception entière de l'appariement
//! qui reposait sur cette phrase.
//!
//! **Le chaînage, en trois pas :**
//!
//! 1. `QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS)` rend les chemins actifs,
//!    chacun reliant une SOURCE (à qui appartient le nom GDI) à une CIBLE (le
//!    moniteur) ;
//! 2. la cible qui nous intéresse est celle dont `(adapterId, id)` est le
//!    couple que `SortieAjoutee` nous a rendu à la création ;
//! 3. `DisplayConfigGetDeviceInfo(GET_SOURCE_NAME)` sur la source de ce
//!    chemin rend `viewGdiDeviceName`, qui est littéralement `\\.\DISPLAYn` —
//!    le même nom que `DXGI_OUTPUT_DESC.DeviceName`, d'où
//!    `SortieDxgi::nom_sortie` est peuplé (`capture/enumeration.rs`).
//!
//! 🔴 **CE QUE CE MODULE SUPPOSE, ET QUI N'EST PAS CONFIRMÉ.** Que
//! `identifiant_cible` soit l'`id` de cible CCD sur l'adaptateur rendu.
//! `sudovda.rs` dit lui-même que la disposition de `VIRTUAL_DISPLAY_ADD_OUT`
//! est « non confirmée ». **Une pièce versionnée rend l'hypothèse crédible
//! sans l'établir** : le lot 22 a compté dix moniteurs fantômes
//! `DISPLAY\SMKD1CE\…UID256` à `UID265` en notant que « les identifiants du
//! pilote (256…265) tournent en rond », et le suffixe `UIDnnnn` d'un chemin
//! d'instance de moniteur EST l'`id` de cible CCD
//! (`docs/superpowers/plans/2026-08-30-lot22-hub-session-resultats.md`).
//! Les nombres coïncident ; que le LUID rendu soit celui qu'emploie CCD n'est
//! pas mesuré.
//!
//! 🔵 **Et si l'hypothèse est fausse, le coût est nul** : la recherche ne
//! trouve aucun chemin, `nom_gdi_de_la_cible` rend `None`, et l'appelant
//! retombe sur le repli qui est le produit d'avant le lot 32. C'est cette
//! propriété — et elle seule — qui a rendu ce module livrable avant d'être
//! mesuré sur la VM.
//!
//! Hors `#[cfg(windows)]`, comme le module parent et pour la même raison : la
//! RÈGLE doit avoir des tests, et ils ne tourneraient pas sous
//! `#[cfg(windows)]`. La moitié Win32 vit dans `mod win`, plus bas — même
//! patron que `superviseur/placement.rs`.

use crate::moniteurs_virtuels::Adaptateur;

/// Un chemin d'affichage actif, réduit à ce dont l'appariement a besoin.
///
/// Un type À NOUS, et non `DISPLAYCONFIG_PATH_INFO` : c'est ce qui permet à la
/// règle ci-dessous d'être pure, éprouvée sur l'hôte Linux, et de ne pas faire
/// entrer un type Win32 dans un module que le parent compile partout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheminActif {
    /// L'adaptateur de la CIBLE, jamais celui de la source : c'est celui que
    /// le pilote nous a rendu.
    pub adaptateur_cible: Adaptateur,
    /// L'identifiant de cible, tel que le système d'affichage le connaît.
    pub id_cible: u32,
    /// Le nom GDI de la SOURCE de ce chemin — `\\.\DISPLAYn`.
    pub nom_gdi: String,
}

/// Le nom GDI de la sortie que le pilote vient de créer, désignée par ce qu'on
/// lui a DONNÉ plutôt que par une différence d'ensembles.
///
/// 🔴 **UNE AMBIGUÏTÉ REFUSE DE TRANCHER, elle ne prend pas le premier.**
/// C'est le précédent d'`AUDIO_PERIPHERIQUE` (`wasapi/peripherique.rs`, où
/// `Choix::Ambigu` refuse plutôt que de retomber sur un rang d'énumération par
/// la porte de derrière) et la leçon des index DXGI de D1, payée une fois.
/// Deux chemins actifs portant la même paire `(adaptateur, id)` est un état
/// que Windows ne devrait pas produire ; s'il le produit, l'appelant retombe
/// sur son repli — c'est-à-dire sur le produit d'avant le lot 32 — plutôt que
/// de désigner une sortie au hasard.
pub fn nom_gdi_de_la_cible(
    chemins: &[CheminActif],
    adaptateur: Adaptateur,
    id_cible: u32,
) -> Option<&str> {
    let mut trouves = chemins
        .iter()
        .filter(|c| c.adaptateur_cible == adaptateur && c.id_cible == id_cible);
    let premier = trouves.next()?;
    if trouves.next().is_some() {
        return None;
    }
    Some(&premier.nom_gdi)
}

#[cfg(windows)]
mod win {
    use anyhow::Result;
    use windows::Win32::Devices::Display::{
        DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, QueryDisplayConfig,
        DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_MODE_INFO,
        DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_SOURCE_DEVICE_NAME, QDC_ONLY_ACTIVE_PATHS,
    };
    use windows::Win32::Foundation::{ERROR_SUCCESS, WIN32_ERROR};

    use super::CheminActif;

    /// Les chemins d'affichage ACTIFS, tels que le système les voit.
    ///
    /// `QDC_ONLY_ACTIVE_PATHS` et non tous les chemins : une cible inactive
    /// n'a aucune source, donc aucun nom GDI, donc rien à apparier. Une sortie
    /// virtuelle qui vient d'être créée mais que Windows n'a pas encore
    /// attachée n'y figure simplement pas — l'appelant scrute, il n'échoue
    /// pas.
    ///
    /// ⚠️ **SILENCIEUSE, et c'est une contrainte, pas un oubli.** Elle est
    /// appelée dans une boucle de scrutation à 10 Hz
    /// (`creation_sortie::attendre_notre_sortie`), et ce dépôt a payé deux
    /// fois pour une trace émise à la cadence d'une boucle (chantier TURN,
    /// correctif I2 de D1 ; 18 619 lignes en quelques secondes sur un partage
    /// CIFS). L'appelant journalise une fois, sur son chemin d'échec.
    ///
    /// ⚠️ **La boucle `ERROR_INSUFFICIENT_BUFFER` est délibérée** : la
    /// configuration d'affichage peut changer ENTRE le dimensionnement et la
    /// lecture — c'est justement ce que fait une sortie virtuelle qui
    /// s'attache pendant qu'on scrute. Un seul essai rendrait une erreur
    /// transitoire indiscernable d'une panne.
    pub fn chemins_actifs() -> Result<Vec<CheminActif>> {
        const ESSAIS: u32 = 4;
        let mut derniere: Option<WIN32_ERROR> = None;
        for _ in 0..ESSAIS {
            let (mut n_chemins, mut n_modes) = (0u32, 0u32);
            let statut = unsafe {
                GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut n_chemins, &mut n_modes)
            };
            if statut != ERROR_SUCCESS {
                return Err(anyhow::anyhow!(
                    "GetDisplayConfigBufferSizes a rendu {:#010x}",
                    statut.0
                ));
            }
            let mut chemins = vec![DISPLAYCONFIG_PATH_INFO::default(); n_chemins as usize];
            let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); n_modes as usize];
            let statut = unsafe {
                QueryDisplayConfig(
                    QDC_ONLY_ACTIVE_PATHS,
                    &mut n_chemins,
                    chemins.as_mut_ptr(),
                    &mut n_modes,
                    modes.as_mut_ptr(),
                    None,
                )
            };
            if statut == ERROR_SUCCESS {
                chemins.truncate(n_chemins as usize);
                return Ok(chemins.iter().filter_map(traduire).collect());
            }
            derniere = Some(statut);
        }
        Err(anyhow::anyhow!(
            "QueryDisplayConfig a rendu {:#010x} après {ESSAIS} essais — la \
             configuration d'affichage change plus vite qu'on ne la lit",
            derniere.map(|e| e.0).unwrap_or(0)
        ))
    }

    /// Un chemin Win32 vers notre type, ou `None` si sa source n'a pas de nom.
    ///
    /// Un chemin sans nom de source n'est pas une erreur : il n'y a
    /// simplement rien à apparier avec, et le faire remonter en `Err`
    /// condamnerait tous les autres chemins du même relevé.
    fn traduire(chemin: &DISPLAYCONFIG_PATH_INFO) -> Option<CheminActif> {
        let mut nom = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
            header: windows::Win32::Devices::Display::DISPLAYCONFIG_DEVICE_INFO_HEADER {
                r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                size: std::mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
                adapterId: chemin.sourceInfo.adapterId,
                id: chemin.sourceInfo.id,
            },
            ..Default::default()
        };
        // Rend un `i32` brut (`ERROR_SUCCESS` vaut 0), et non un `WIN32_ERROR`
        // — la signature de `DisplayConfigGetDeviceInfo` diffère de celle de
        // ses deux voisines. Vérifié dans windows-0.62.2, pas supposé.
        if unsafe { DisplayConfigGetDeviceInfo(&mut nom.header) } != 0 {
            return None;
        }
        let fin = nom
            .viewGdiDeviceName
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(nom.viewGdiDeviceName.len());
        let nom_gdi = String::from_utf16_lossy(&nom.viewGdiDeviceName[..fin]);
        if nom_gdi.is_empty() {
            return None;
        }
        Some(CheminActif {
            adaptateur_cible: (
                chemin.targetInfo.adapterId.LowPart,
                chemin.targetInfo.adapterId.HighPart,
            ),
            id_cible: chemin.targetInfo.id,
            nom_gdi,
        })
    }
}

#[cfg(windows)]
pub use win::chemins_actifs;

#[cfg(test)]
mod tests {
    use super::*;

    fn chemin(adaptateur: Adaptateur, id: u32, nom: &str) -> CheminActif {
        CheminActif { adaptateur_cible: adaptateur, id_cible: id, nom_gdi: nom.to_string() }
    }

    #[test]
    fn la_cible_creee_donne_le_nom_gdi_de_sa_source() {
        let chemins = vec![
            chemin((7, 0), 4096, "\\\\.\\DISPLAY1"),
            chemin((9, 0), 256, "\\\\.\\DISPLAY5"),
        ];
        assert_eq!(nom_gdi_de_la_cible(&chemins, (9, 0), 256), Some("\\\\.\\DISPLAY5"));
    }

    #[test]
    fn un_identifiant_de_cible_ne_suffit_pas_sans_son_adaptateur() {
        // Le MÊME identifiant de cible sur DEUX adaptateurs : c'est le cas que
        // le couple existe pour trancher, et la raison pour laquelle le lot 32
        // a cessé de jeter le LUID.
        let chemins = vec![
            chemin((7, 0), 256, "\\\\.\\DISPLAY1"),
            chemin((9, 0), 256, "\\\\.\\DISPLAY5"),
        ];
        assert_eq!(nom_gdi_de_la_cible(&chemins, (7, 0), 256), Some("\\\\.\\DISPLAY1"));
        assert_eq!(nom_gdi_de_la_cible(&chemins, (9, 0), 256), Some("\\\\.\\DISPLAY5"));
    }

    #[test]
    fn une_cible_absente_rend_none_et_non_un_choix_au_hasard() {
        let chemins = vec![chemin((7, 0), 4096, "\\\\.\\DISPLAY1")];
        assert_eq!(nom_gdi_de_la_cible(&chemins, (9, 0), 256), None);
    }

    #[test]
    fn une_paire_ambigue_refuse_de_trancher() {
        // Deux chemins pour la même paire : Windows ne devrait pas produire
        // cet état. On rend `None` — donc le repli de l'appelant — plutôt que
        // de désigner le premier, qui serait un rang d'énumération déguisé.
        let chemins = vec![
            chemin((9, 0), 256, "\\\\.\\DISPLAY5"),
            chemin((9, 0), 256, "\\\\.\\DISPLAY6"),
        ];
        assert_eq!(nom_gdi_de_la_cible(&chemins, (9, 0), 256), None);
    }
}
