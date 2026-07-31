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

use super::sudovda::{
    en_champ_14, DemandeAjout, DemandeRetrait, SortieAjoutee, Veille, VersionProtocole,
    INTERFACE_PILOTE, IOCTL_AJOUTER_SORTIE, IOCTL_LIRE_VEILLE, IOCTL_LIRE_VERSION_PROTOCOLE,
    IOCTL_RETIRER_SORTIE,
};
use crate::moniteurs_virtuels::{IdSortie, PiloteAffichageVirtuel};

/// Gabarit du GUID que nous attribuons à chaque sortie créée : les 16 bits de
/// poids faible portent un compteur, le reste est une constante arbitraire
/// choisie ici. Le GUID n'a besoin que d'être unique et reconnaissable — s'il
/// traîne un jour dans l'état du pilote, on saura d'où il vient.
///
/// Le compteur repart de zéro à chaque exécution, et c'est un choix assumé :
/// deux exécutions attribuent donc les mêmes GUID. C'est ce déterminisme qui
/// donnera à la purge de la tâche 7 un motif reconnaissable pour retrouver nos
/// sorties orphelines, qu'aucune table en mémoire ne peut plus désigner.
const GABARIT_GUID_MONITEUR: u128 = 0x9c4a_1f6e_2b73_4d51_9e08_6775_4143_0000;

/// Tout ce que le pilote doit retenir entre deux appels, sous un verrou
/// unique.
///
/// Le compteur et la table ne servent qu'un seul invariant — « chaque sortie
/// vivante a un GUID connu, et deux sorties n'ont jamais le même » — donc un
/// seul verrou. Deux verrous pour un invariant seraient un piège gratuit.
#[derive(Default)]
struct EtatSorties {
    /// Le trait rend un `IdSortie` (`u32`) alors que le pilote retire par
    /// GUID : il faut donc retenir l'appariement.
    apparies: Vec<(IdSortie, GUID)>,
    /// Compteur des GUID attribués.
    compteur: u16,
}

pub(super) struct PiloteParIoctl {
    peripherique: HANDLE,
    /// Un `Mutex` et non un `RefCell` parce que `creer(&self, …)` doit rester
    /// utilisable depuis un contexte partagé. Voir `etat()` pour la seule
    /// subtilité qu'il introduit.
    etat: Mutex<EtatSorties>,
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
    Ok(PiloteParIoctl { peripherique, etat: Mutex::new(EtatSorties::default()) })
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
    /// Accès à l'état, **sans paniquer sur un verrou empoisonné**.
    ///
    /// `Sorties::drop` appelle `detruire` pendant le déroulement d'une panique
    /// et rattrape les `Err` — mais pas les paniques. Si la panique s'est
    /// produite alors que `creer` tenait ce verrou, celui-ci est empoisonné :
    /// un `.expect(…)` paniquerait ici, dans un `Drop`, ce qui abrège le
    /// processus (`abort`) et laisserait les sorties restantes non détruites.
    /// C'est précisément le scénario que ce module doit couvrir, pas
    /// aggraver. `into_inner` rend la table telle quelle : au pire, une
    /// insertion interrompue par la panique y manque.
    fn etat(&self) -> std::sync::MutexGuard<'_, EtatSorties> {
        self.etat.lock().unwrap_or_else(|empoisonne| empoisonne.into_inner())
    }

    /// Retire du pilote la sortie portant ce GUID.
    ///
    /// Extrait de `detruire` parce que `creer` doit pouvoir l'appeler aussi,
    /// sur son chemin d'échec — là où aucun `IdSortie` fiable n'existe.
    fn retirer_par_guid(&self, guid_moniteur: GUID, quoi: &str) -> Result<()> {
        let demande = DemandeRetrait { guid_moniteur };
        self.commander(
            IOCTL_RETIRER_SORTIE,
            Some((
                &demande as *const _ as *const _,
                std::mem::size_of::<DemandeRetrait>() as u32,
            )),
            None,
            quoi,
        )?;
        Ok(())
    }

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

impl PiloteAffichageVirtuel for PiloteParIoctl {
    fn creer(&self, largeur: u32, hauteur: u32, hertz: u32) -> Result<IdSortie> {
        let mut etat = self.etat();
        etat.compteur = etat.compteur.wrapping_add(1);
        let numero = etat.compteur;
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

        // À PARTIR D'ICI LA SORTIE EXISTE. Tout chemin d'échec sous cette ligne
        // doit donc défaire ce qui vient d'être fait, ou au minimum laisser le
        // GUID connu — sans quoi le moniteur survit au processus sans qu'aucun
        // code du projet ne puisse le retirer. L'appariement est enregistré
        // AVANT toute vérification pour cette raison : le GUID est le nôtre,
        // il est valide même quand le tampon de sortie est illisible.
        let id = ajoutee.identifiant_cible;
        etat.apparies.push((id, guid_moniteur));
        drop(etat);

        // Le chemin d'échec le plus probable de ce module, et il fuyait :
        // `VIRTUAL_DISPLAY_ADD_OUT` est justement la structure que la
        // reconnaissance déclare non confirmée. Si son compte d'octets diffère,
        // `identifiant_cible` peut valoir n'importe quoi — l'appelant ne pourra
        // donc jamais nous redemander cette sortie par son identifiant, et la
        // garde `Sorties` ne l'enregistrera pas non plus puisque nous rendons
        // `Err`. On la retire donc NOUS-MÊMES, tant que le GUID est encore
        // connu, plutôt que de la laisser derrière nous.
        let attendus = std::mem::size_of::<SortieAjoutee>();
        if rendus as usize != attendus {
            let retrait = self.retirer_par_guid(
                guid_moniteur,
                "retrait de la sortie créée avec un tampon de sortie illisible",
            );
            let mut etat = self.etat();
            match retrait {
                Ok(()) => {
                    etat.apparies.retain(|(_, connu)| *connu != guid_moniteur);
                    anyhow::bail!(
                        "le pilote a rendu {rendus} octets pour une sortie créée, \
                         {attendus} attendus — la disposition supposée de \
                         VIRTUAL_DISPLAY_ADD_OUT est fausse ; la sortie a été retirée"
                    );
                }
                Err(erreur) => {
                    // L'appariement reste en table à dessein : c'est la seule
                    // trace du GUID à retirer, et la purge de la tâche 7 en a
                    // besoin.
                    tracing::error!(
                        guid = ?guid_moniteur,
                        %erreur,
                        "sortie virtuelle NON retirée après un tampon illisible — \
                         purge manuelle requise"
                    );
                    anyhow::bail!(
                        "le pilote a rendu {rendus} octets pour une sortie créée, \
                         {attendus} attendus — la disposition supposée de \
                         VIRTUAL_DISPLAY_ADD_OUT est fausse, ET son retrait a \
                         échoué : {erreur}"
                    );
                }
            }
        }

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
        Ok(id)
    }

    fn detruire(&self, id: IdSortie) -> Result<()> {
        let etat = self.etat();
        // Refuser plutôt que deviner : le pilote retire par GUID, et fabriquer
        // un GUID au jugé détruirait au mieux rien, au pire la sortie d'un
        // autre client (Apollo en attribue aussi).
        let rang = etat
            .apparies
            .iter()
            .position(|(connu, _)| *connu == id)
            .with_context(|| format!("sortie {id} inconnue de ce pilote — rien à détruire"))?;
        let (_, guid_moniteur) = etat.apparies[rang];
        drop(etat);

        // L'appariement n'est retiré de la table qu'APRÈS un retrait réussi, et
        // non avant : sur échec, le GUID reste la seule prise que le projet ait
        // sur ce moniteur, et l'oublier le rendrait irrécupérable. Le garder
        // laisse une nouvelle tentative possible.
        self.retirer_par_guid(guid_moniteur, &format!("destruction de la sortie {id}"))?;
        self.etat().apparies.retain(|(_, connu)| *connu != guid_moniteur);
        tracing::info!(id, "sortie virtuelle détruite");
        Ok(())
    }
}

impl Drop for PiloteParIoctl {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.peripherique) };
    }
}
