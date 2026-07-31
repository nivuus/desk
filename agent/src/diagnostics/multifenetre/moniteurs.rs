//! Mesure ① : le pilote d'affichage virtuel, commandé depuis NOTRE code.
//!
//! La spec §2 tranche : on ne relance pas Apollo pour obtenir cette mesure.
//! D'abord parce qu'elle dépendrait d'un second appareil client apparié que le
//! propriétaire du poste n'a pas — c'est ce qui a bloqué la sonde précédente.
//! Ensuite parce que le produit devra de toute façon se passer d'Apollo.
//!
//! Canal de contrôle relevé à la tâche 3 :
//! `docs/superpowers/plans/journaux-mesures-prealables/canal-de-controle.md`.
//! Son verdict est la **forme B** : le pilote SudoVDA n'exporte que
//! `FxDriverEntryUm` (le point d'entrée générique UMDF), donc aucune fonction
//! de contrôle appelable — l'hypothèse « charger la DLL et appeler
//! `AddVirtualDisplay` » est morte, et n'a pas été implémentée. On parle au
//! pilote comme le fait son client réel (`sunshine.exe`) : énumération de son
//! interface de périphérique par SetupAPI, `CreateFile`, `DeviceIoControl`.
//!
//! **Ce qui est établi et ce qui ne l'est pas.** Le GUID d'interface et deux
//! des six codes IOCTL ont été retrouvés octet pour octet dans le
//! `SudoVDA.dll` installé sur CETTE VM. Les quatre autres codes et la
//! disposition de TOUTES les structures ci-dessous viennent d'un en-tête amont
//! (`Apollo/third-party/sudovda`) dont la dernière modification connue précède
//! de onze mois le pilote installé (`DriverVer 07/14/2025, 1.10.9.289`). Rien
//! n'exclut qu'un champ ait été ajouté ou réordonné depuis. C'est la raison
//! d'être du module voisin `contrat.rs` : éprouver le contrat sur les deux
//! tampons les plus simples AVANT que la tâche suivante n'engage
//! `IOCTL_ADD_VIRTUAL_DISPLAY` et ses 56 octets d'entrée. Sans cela, un
//! contrat faux et une mesure ratée seraient indiscernables.
//!
//! **Le watchdog n'est pas armé ici.** Le pilote expose `IOCTL_DRIVER_PING` et
//! `IOCTL_GET_WATCHDOG` : un client qui cesse de pinguer voit ses sorties
//! retirées. Ce module ne pingue pas — il n'en a pas besoin, ne créant rien de
//! durable — mais la sonde de `contrat.rs` relève le délai réel, pour que la
//! tâche qui tiendra N sorties vivantes sache à quelle cadence pinguer.

// La création et la destruction de sorties n'ont PAS ENCORE de consommateur :
// la sonde de `contrat.rs` n'appelle délibérément que les deux IOCTL sans effet
// de bord, et c'est la tâche suivante (montée en N) qui exercera
// `PiloteAffichageVirtuel`. Sans cet `allow`, le cœur de ce module — les codes
// IOCTL d'ajout et de retrait, la table d'appariement, le gabarit de GUID —
// ressort en avertissements « never used » qui noieraient les vrais.
// **À RETIRER dès que la montée en N appelle `creer`/`detruire`** : à partir de
// là, un « never used » dans ce module redevient un signal.
#![allow(dead_code)]

use std::sync::Mutex;

use anyhow::{Context, Result};
use windows::core::{GUID, PCWSTR};
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
    SetupDiGetDeviceInterfaceDetailW, DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, HDEVINFO,
    SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W,
};
use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::IO::DeviceIoControl;

use crate::moniteurs_virtuels::{IdSortie, PiloteAffichageVirtuel};

/// Interface de périphérique de SudoVDA — **confirmée par présence d'octets**
/// dans le `SudoVDA.dll` installé sur cette VM (canal-de-controle.md §5.3).
/// À ne pas confondre avec le GUID de classe `{4D36E968-…}`, qui est la classe
/// `Display` standard de Windows et ne sert qu'à l'installation.
const INTERFACE_PILOTE: GUID = GUID::from_u128(0xe5bc_c234_1e0c_418a_a0d4_ef8b_7501_414d);

// `CTL_CODE(FILE_DEVICE_UNKNOWN = 0x22, fonction, METHOD_BUFFERED = 0,
// FILE_ANY_ACCESS = 0)` = `(0x22 << 16) | (fonction << 2)`. Les deux codes
// marqués « confirmé » ont été retrouvés en octets dans la DLL installée ; les
// autres proviennent de la même macro appliquée au même en-tête amont.
//
// Les deux codes que ce module n'emploie pas (`IOCTL_SET_RENDER_ADAPTER`
// `0x0022_2008`, `IOCTL_DRIVER_PING` `0x0022_2220`) ne sont volontairement pas
// déclarés : une constante inutilisée est un avertissement de compilation, et
// une constante non employée n'est de toute façon éprouvée par rien.

/// Confirmé par octets (offset 16316 de la DLL locale).
const IOCTL_AJOUTER_SORTIE: u32 = 0x0022_2000;
/// Non confirmé par octets — même macro, même en-tête amont.
const IOCTL_RETIRER_SORTIE: u32 = 0x0022_2004;
/// Confirmé par octets (offset 16284 de la DLL locale).
const IOCTL_LIRE_VEILLE: u32 = 0x0022_200C;
/// Non confirmé par octets — c'est le tampon le plus simple des six, donc le
/// premier que `valider_contrat()` éprouve.
const IOCTL_LIRE_VERSION_PROTOCOLE: u32 = 0x0022_23FC;

/// Tampon d'entrée de `IOCTL_AJOUTER_SORTIE` (`VIRTUAL_DISPLAY_ADD_PARAMS`).
///
/// Aucun `#pragma pack` en amont : alignement naturel MSVC/x64, soit 4 ici.
/// Offsets attendus : 0, 4, 8, 12, 28, 42 — total 56 octets, vérifié par
/// l'assertion de compilation plus bas.
///
/// Aucun de ces champs n'est jamais relu depuis Rust —
/// le seul lecteur est le pilote, à l'autre bout du `DeviceIoControl`. Les
/// retirer pour faire taire le lint reviendrait à changer la disposition du
/// tampon, c'est-à-dire à casser exactement ce que cette structure décrit.
#[repr(C)]
struct DemandeAjout {
    largeur: u32,
    hauteur: u32,
    hertz: u32,
    /// Choisi par NOUS, pas rendu par le pilote : c'est la clé de retrait.
    guid_moniteur: GUID,
    nom_peripherique: [u8; 14],
    numero_serie: [u8; 14],
}

/// Tampon de sortie de `IOCTL_AJOUTER_SORTIE` (`VIRTUAL_DISPLAY_ADD_OUT`).
///
/// `LUID` Win32 = `{ DWORD LowPart; LONG HighPart; }`, 8 octets alignés sur 4 —
/// écrit en deux champs plutôt qu'en `windows::Win32::Foundation::LUID` pour
/// que la disposition qu'on suppose soit lisible ici, là où elle est en jeu.
#[repr(C)]
#[derive(Default)]
struct SortieAjoutee {
    adaptateur_bas: u32,
    adaptateur_haut: i32,
    /// C'est lui qui devient l'`IdSortie` du trait.
    identifiant_cible: u32,
}

/// Tampon d'entrée de `IOCTL_RETIRER_SORTIE`
/// (`VIRTUAL_DISPLAY_REMOVE_PARAMS`) : le pilote retire par le GUID que le
/// client a choisi à l'ajout, pas par l'identifiant qu'il a rendu.
///
/// Ses champs ne sont pas davantage relus depuis Rust — même raison que
/// `DemandeAjout`.
#[repr(C)]
struct DemandeRetrait {
    guid_moniteur: GUID,
}

/// Tampon de sortie de `IOCTL_LIRE_VEILLE`
/// (`VIRTUAL_DISPLAY_GET_WATCHDOG_OUT`).
///
/// L'en-tête amont ne documente AUCUNE unité pour ces deux `UINT` — ni le nom
/// des champs (`Timeout`, `Countdown`) ni un commentaire ne la donnent. On ne
/// la suppose donc pas ici : la sonde relève les nombres bruts, et c'est à la
/// tâche qui devra pinguer d'établir la cadence par la mesure.
#[repr(C)]
#[derive(Default)]
pub(super) struct Veille {
    pub(super) delai: u32,
    pub(super) decompte: u32,
}

/// Tampon de sortie de `IOCTL_LIRE_VERSION_PROTOCOLE`
/// (`SUVDA_PROTOCAL_VERSION`, orthographe d'origine). Quatre octets : le
/// `bool` MSVC en occupe un seul.
#[repr(C)]
#[derive(Default)]
pub(super) struct VersionProtocole {
    pub(super) majeure: u8,
    pub(super) mineure: u8,
    pub(super) increment: u8,
    pub(super) version_de_test: u8,
}

// Les tailles sont la seule partie du contrat amont qu'on puisse vérifier sans
// la VM. Un champ oublié ou un type mal traduit ferait échouer la compilation
// ici plutôt que de partir en tampon mal formé vers un pilote noyau.
const _: () = {
    assert!(std::mem::size_of::<DemandeAjout>() == 56);
    assert!(std::mem::size_of::<SortieAjoutee>() == 12);
    assert!(std::mem::size_of::<DemandeRetrait>() == 16);
    assert!(std::mem::size_of::<Veille>() == 8);
    assert!(std::mem::size_of::<VersionProtocole>() == 4);
};

/// Gabarit du GUID que nous attribuons à chaque sortie créée : les 16 bits de
/// poids faible portent un compteur, le reste est une constante arbitraire
/// choisie ici. Le GUID n'a besoin que d'être unique et reconnaissable — s'il
/// traîne un jour dans l'état du pilote, on saura d'où il vient.
const GABARIT_GUID_MONITEUR: u128 = 0x9c4a_1f6e_2b73_4d51_9e08_6775_4143_0000;

pub(super) struct PiloteParIoctl {
    peripherique: HANDLE,
    /// Le trait rend un `IdSortie` (`u32`) alors que le pilote retire par
    /// GUID : il faut donc retenir l'appariement. Un `Mutex` et non un
    /// `RefCell` parce que `creer(&self, …)` doit rester utilisable depuis un
    /// contexte partagé, et parce que la garde `Sorties` peut détruire depuis
    /// le déroulement d'une panique.
    apparies: Mutex<Vec<(IdSortie, GUID)>>,
    /// Compteur des GUID attribués. Sous le même `Mutex` que la table : deux
    /// verrous pour un seul invariant seraient un piège gratuit.
    compteur: Mutex<u16>,
}

/// Ouvre le périphérique du pilote d'affichage virtuel.
///
/// Type concret et non `impl Trait` : les tâches suivantes en prennent une
/// référence, que Rust coerce vers `&dyn PiloteAffichageVirtuel`.
pub(super) fn ouvrir_pilote() -> Result<PiloteParIoctl> {
    let chemin = chemin_du_peripherique()?;
    // POURQUOI PAS `FILE_FLAG_OVERLAPPED`, contrairement au client amont.
    //
    // `sunshine.exe` ouvre ce périphérique avec `FILE_FLAG_NO_BUFFERING |
    // FILE_FLAG_OVERLAPPED | FILE_FLAG_WRITE_THROUGH`, puis appelle
    // `DeviceIoControl` avec un `lpOverlapped` nul. La documentation Win32 est
    // pourtant explicite : sur un handle chevauchant, ce paramètre ne peut pas
    // être `NULL` — l'appel peut alors rendre la main avant que le tampon de
    // sortie soit rempli, et lire ce tampon est une course. Que cela « marche »
    // chez le client amont ne prouve rien : ce serait vrai de tout pilote qui
    // termine ses requêtes synchronement, jusqu'au jour où il n'en termine plus
    // une. On ouvre donc en mode synchrone (ni `OVERLAPPED`, ni les deux autres
    // drapeaux, qui ne concernent que le cache de fichier et n'ont aucun sens
    // pour des IOCTL `METHOD_BUFFERED`) : c'est le seul mode où un
    // `lpOverlapped` nul est correct, et tous nos appels sont synchrones.
    //
    // `GENERIC_READ | GENERIC_WRITE` et non `FILE_GENERIC_*` : le descripteur
    // de sécurité posé par l'INF n'accorde au monde que `GRGW`
    // (`(A;;GRGW;;;WD)`) — ce sont exactement ces droits génériques-là qu'il
    // faut demander.
    let peripherique = unsafe {
        CreateFileW(
            PCWSTR(chemin.as_ptr()),
            (GENERIC_READ | GENERIC_WRITE).0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
    }
    .context("ouverture du périphérique du pilote d'affichage virtuel (SudoVDA)")?;
    Ok(PiloteParIoctl {
        peripherique,
        apparies: Mutex::new(Vec::new()),
        compteur: Mutex::new(0),
    })
}

/// Libère la liste d'informations de périphériques sur TOUS les chemins, y
/// compris les sorties en erreur — SetupAPI ne pardonne pas les fuites de
/// `HDEVINFO`, et il y a quatre `?` entre son ouverture et sa fermeture.
struct ListeDePeripheriques(HDEVINFO);

impl Drop for ListeDePeripheriques {
    fn drop(&mut self) {
        let _ = unsafe { SetupDiDestroyDeviceInfoList(self.0) };
    }
}

/// Résout le chemin `\\?\…` du périphérique qui expose `INTERFACE_PILOTE`.
fn chemin_du_peripherique() -> Result<Vec<u16>> {
    let liste = ListeDePeripheriques(
        unsafe {
            SetupDiGetClassDevsW(
                Some(&INTERFACE_PILOTE),
                PCWSTR::null(),
                None,
                DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
            )
        }
        .context("énumération des périphériques exposant l'interface SudoVDA")?,
    );

    // Index 0 : le pilote n'expose qu'une instance de cette interface (un seul
    // device node `ROOT\DISPLAY\0003`). Si un jour il y en avait plusieurs, ce
    // serait un fait à relever avant de choisir — pas à trancher en silence.
    let mut interface = SP_DEVICE_INTERFACE_DATA {
        cbSize: std::mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
        ..Default::default()
    };
    unsafe { SetupDiEnumDeviceInterfaces(liste.0, None, &INTERFACE_PILOTE, 0, &mut interface) }
        .context(
            "aucun périphérique ne présente l'interface SudoVDA — pilote absent, \
             désactivé, ou device node non créé",
        )?;

    // Patron imposé par SetupAPI : un premier appel pour la taille (qui échoue
    // toujours en `ERROR_INSUFFICIENT_BUFFER`, d'où l'erreur ignorée), un
    // second pour le contenu.
    let mut requis = 0u32;
    let _ = unsafe {
        SetupDiGetDeviceInterfaceDetailW(liste.0, &interface, None, 0, Some(&mut requis), None)
    };
    let entete = std::mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
    anyhow::ensure!(
        requis as usize > entete,
        "taille de détail d'interface aberrante ({requis} octets)"
    );

    // Tampon en `u32` et non en `u8` : `SP_DEVICE_INTERFACE_DETAIL_DATA_W`
    // s'aligne sur 4, et `Vec<u8>` ne garantit que 1. Le `cbSize` à écrire est
    // celui de l'en-tête seul (8 sur x64), jamais celui du tampon — c'est la
    // convention de SetupAPI, contre-intuitive et source classique de
    // `ERROR_INVALID_USER_BUFFER`.
    let mut tampon = vec![0u32; requis.div_ceil(4) as usize];
    let detail = tampon.as_mut_ptr() as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;
    unsafe { (*detail).cbSize = entete as u32 };
    unsafe {
        SetupDiGetDeviceInterfaceDetailW(liste.0, &interface, Some(detail), requis, None, None)
    }
    .context("lecture du chemin du périphérique SudoVDA")?;

    // `DevicePath` est déclaré `[u16; 1]` mais se prolonge jusqu'au NUL au-delà
    // de la fin nominale de la structure : c'est un tableau de longueur
    // variable à la mode C, il faut le lire à la main.
    //
    // Le nombre d'unités lisibles se compte depuis l'OFFSET de `DevicePath`
    // (4 octets, juste après `cbSize`) et NON depuis `entete` (8 octets, qui
    // inclut le remplissage d'alignement de fin de structure). L'écart est
    // d'exactement deux octets, soit une unité UTF-16 : partir d'`entete`
    // amputait le chemin de son terminateur nul et faisait échouer la
    // résolution — constaté à la première exécution réelle de la sonde.
    let offset_chemin = std::mem::offset_of!(SP_DEVICE_INTERFACE_DETAIL_DATA_W, DevicePath);
    let debut = unsafe { (*detail).DevicePath.as_ptr() };
    let maximum = (requis as usize - offset_chemin) / 2;
    let mut chemin = Vec::with_capacity(maximum);
    for decalage in 0..maximum {
        let unite = unsafe { *debut.add(decalage) };
        chemin.push(unite);
        if unite == 0 {
            return Ok(chemin);
        }
    }
    anyhow::bail!("chemin de périphérique SudoVDA sans terminateur nul");
}

impl PiloteParIoctl {
    /// Un appel `DeviceIoControl` synchrone, avec vérification du nombre
    /// d'octets rendus.
    ///
    /// Cette vérification n'est pas de la ceinture-et-bretelles : c'est le seul
    /// signal disponible qu'une structure de sortie a bien la taille qu'on lui
    /// suppose. Un pilote qui aurait gagné un champ depuis l'en-tête amont
    /// rendrait un compte différent, et on veut le voir plutôt que lire un
    /// tampon partiellement rempli.
    fn commander(
        &self,
        code: u32,
        entree: Option<(*const std::ffi::c_void, u32)>,
        sortie: Option<(*mut std::ffi::c_void, u32)>,
        quoi: &str,
    ) -> Result<u32> {
        let (ptr_entree, taille_entree) = entree.map_or((None, 0), |(p, t)| (Some(p), t));
        let (ptr_sortie, taille_sortie) = sortie.map_or((None, 0), |(p, t)| (Some(p), t));
        let mut rendus = 0u32;
        unsafe {
            DeviceIoControl(
                self.peripherique,
                code,
                ptr_entree,
                taille_entree,
                ptr_sortie,
                taille_sortie,
                Some(&mut rendus),
                // Nul, et légitimement : le handle est ouvert en mode
                // synchrone (voir `ouvrir_pilote`).
                None,
            )
        }
        .with_context(|| format!("{quoi} (IOCTL {code:#010x})"))?;
        Ok(rendus)
    }

    /// Version de protocole annoncée par le pilote installé. Sans effet de
    /// bord — le tampon le plus simple des six.
    pub(super) fn version_protocole(&self) -> Result<(VersionProtocole, u32)> {
        let mut version = VersionProtocole::default();
        let rendus = self.commander(
            IOCTL_LIRE_VERSION_PROTOCOLE,
            None,
            Some((
                &mut version as *mut _ as *mut _,
                std::mem::size_of::<VersionProtocole>() as u32,
            )),
            "lecture de la version de protocole du pilote",
        )?;
        Ok((version, rendus))
    }

    /// Délai et décompte du watchdog du pilote. Sans effet de bord.
    pub(super) fn veille(&self) -> Result<(Veille, u32)> {
        let mut veille = Veille::default();
        let rendus = self.commander(
            IOCTL_LIRE_VEILLE,
            None,
            Some((&mut veille as *mut _ as *mut _, std::mem::size_of::<Veille>() as u32)),
            "lecture du watchdog du pilote",
        )?;
        Ok((veille, rendus))
    }
}

/// Copie une désignation ASCII dans un champ `CHAR[14]`, terminée par un NUL.
///
/// Tronque à 13 caractères utiles plutôt que de refuser : ces deux champs sont
/// cosmétiques (Apollo y met le nom et l'identifiant du client), et faire
/// échouer une création de moniteur pour un nom trop long serait absurde.
fn en_champ_14(texte: &str) -> [u8; 14] {
    let mut champ = [0u8; 14];
    for (place, octet) in champ.iter_mut().zip(texte.bytes()).take(13) {
        *place = octet;
    }
    champ
}

impl PiloteAffichageVirtuel for PiloteParIoctl {
    fn creer(&self, largeur: u32, hauteur: u32, hertz: u32) -> Result<IdSortie> {
        let mut apparies = self.apparies.lock().expect("table des sorties empoisonnée");
        let mut compteur = self.compteur.lock().expect("compteur des sorties empoisonné");
        *compteur = compteur.wrapping_add(1);
        let numero = *compteur;
        let guid_moniteur = GUID::from_u128(GABARIT_GUID_MONITEUR | u128::from(numero));

        let demande = DemandeAjout {
            largeur,
            hauteur,
            hertz,
            guid_moniteur,
            nom_peripherique: en_champ_14("Guacamole"),
            numero_serie: en_champ_14(&format!("mesure{numero}")),
        };
        let mut ajoutee = SortieAjoutee::default();
        let rendus = self.commander(
            IOCTL_AJOUTER_SORTIE,
            Some((
                &demande as *const _ as *const _,
                std::mem::size_of::<DemandeAjout>() as u32,
            )),
            Some((
                &mut ajoutee as *mut _ as *mut _,
                std::mem::size_of::<SortieAjoutee>() as u32,
            )),
            &format!("création d'une sortie {largeur}x{hauteur}@{hertz}"),
        )?;
        anyhow::ensure!(
            rendus as usize == std::mem::size_of::<SortieAjoutee>(),
            "le pilote a rendu {rendus} octets pour une sortie créée, {} attendus — \
             la disposition supposée de VIRTUAL_DISPLAY_ADD_OUT est fausse",
            std::mem::size_of::<SortieAjoutee>()
        );

        let id = ajoutee.identifiant_cible;
        tracing::info!(
            id,
            adaptateur_bas = ajoutee.adaptateur_bas,
            adaptateur_haut = ajoutee.adaptateur_haut,
            guid = ?guid_moniteur,
            largeur,
            hauteur,
            hertz,
            "sortie virtuelle créée"
        );
        apparies.push((id, guid_moniteur));
        Ok(id)
    }

    fn detruire(&self, id: IdSortie) -> Result<()> {
        let mut apparies = self.apparies.lock().expect("table des sorties empoisonnée");
        // Refuser plutôt que deviner : le pilote retire par GUID, et fabriquer
        // un GUID au jugé détruirait au mieux rien, au pire la sortie d'un
        // autre client (Apollo en attribue aussi).
        let rang = apparies
            .iter()
            .position(|(connu, _)| *connu == id)
            .with_context(|| format!("sortie {id} inconnue de ce pilote — rien à détruire"))?;
        let (_, guid_moniteur) = apparies.remove(rang);
        drop(apparies);

        let demande = DemandeRetrait { guid_moniteur };
        self.commander(
            IOCTL_RETIRER_SORTIE,
            Some((
                &demande as *const _ as *const _,
                std::mem::size_of::<DemandeRetrait>() as u32,
            )),
            None,
            &format!("destruction de la sortie {id}"),
        )?;
        tracing::info!(id, "sortie virtuelle détruite");
        Ok(())
    }
}

impl Drop for PiloteParIoctl {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.peripherique) };
    }
}
