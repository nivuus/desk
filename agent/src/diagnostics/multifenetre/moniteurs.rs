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
use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::IO::DeviceIoControl;

use super::peripherique::chemin_du_peripherique;
use super::sudovda::{
    en_champ_14, DemandeAjout, DemandeRetrait, SortieAjoutee, Veille, VersionProtocole,
    IOCTL_AJOUTER_SORTIE, IOCTL_LIRE_VEILLE, IOCTL_LIRE_VERSION_PROTOCOLE, IOCTL_RETIRER_SORTIE,
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
/// unique — le compteur et les deux listes ne servent qu'un seul invariant
/// (« toute sortie créée a un GUID connu tant qu'elle n'est pas retirée »), et
/// deux verrous pour un invariant seraient un piège gratuit.
///
/// **Deux listes en revanche, et non une**, parce que ce sont deux rôles et
/// deux durées de vie.
///
/// Une entrée d'`apparies` sert à **traduire un identifiant en GUID**. Une
/// entrée d'`a_purger` sert à **se souvenir d'un retrait dû**. Confondre les
/// deux fait qu'une sortie dont le retrait a échoué reste indexée par un
/// identifiant que le pilote peut réattribuer : un `detruire` ultérieur
/// apparierait alors l'entrée périmée, enverrait le mauvais GUID, et la sortie
/// vivante ne serait jamais détruite. Un identifiant qu'on ne peut plus honorer
/// doit donc quitter `apparies`, sans que le GUID soit perdu pour autant.
#[derive(Default)]
struct EtatSorties {
    /// Sorties vivantes dont l'identifiant est FIABLE — c'est-à-dire rendu par
    /// un tampon de sortie de la bonne taille. Le trait rend un `IdSortie`
    /// (`u32`) alors que le pilote retire par GUID : c'est ici que se fait la
    /// traduction, et rien d'autre n'a le droit d'y figurer.
    apparies: Vec<(IdSortie, GUID)>,
    /// GUID de sorties dont la création a réussi et dont le retrait est DÛ,
    /// sans qu'aucun identifiant fiable ne permette de les redemander. Jamais
    /// consultée par `detruire` : elle ne sert qu'à ne pas perdre la trace de
    /// ce qui doit être purgé — voir la tâche 7.
    a_purger: Vec<GUID>,
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

    /// Efface toute trace de ce GUID : la sortie n'existe plus, ni retrait dû
    /// ni appariement ne doivent lui survivre.
    fn oublier(&self, guid_moniteur: GUID) {
        let mut etat = self.etat();
        etat.a_purger.retain(|connu| *connu != guid_moniteur);
        etat.apparies.retain(|(_, connu)| *connu != guid_moniteur);
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
        // Ce verrou est tenu pendant le `DeviceIoControl` d'ajout, un appel
        // noyau bloquant : sans conséquence tant que la montée en N reste
        // séquentielle, à revoir si elle cesse de l'être.
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
        // code du projet ne puisse le retirer.
        //
        // Le GUID est retenu AVANT toute vérification, et dans `a_purger` et
        // non `apparies` : à cet instant on sait qu'une sortie existe, mais on
        // ne sait pas encore la DÉSIGNER — `identifiant_cible` ne vaut quelque
        // chose que si le tampon de sortie fait la taille attendue. Toute
        // sortie créée entre donc d'abord par la liste des retraits dus, et
        // n'en sort que pour être appariée à un identifiant fiable, ou parce
        // qu'elle a été retirée.
        etat.a_purger.push(guid_moniteur);
        drop(etat);

        // Le chemin d'échec le plus probable de ce module : `VIRTUAL_DISPLAY_ADD_OUT`
        // est justement la structure que la reconnaissance déclare non
        // confirmée. Si son compte d'octets diffère, `identifiant_cible` peut
        // valoir n'importe quoi — l'appelant ne pourra donc jamais nous
        // redemander cette sortie par son identifiant, et la garde `Sorties`
        // ne l'enregistrera pas non plus puisque nous rendons `Err`. On la
        // retire donc NOUS-MÊMES, tant que le GUID est connu.
        let attendus = std::mem::size_of::<SortieAjoutee>();
        if rendus as usize != attendus {
            let retrait = self.retirer_par_guid(
                guid_moniteur,
                "retrait de la sortie créée avec un tampon de sortie illisible",
            );
            match retrait {
                Ok(()) => {
                    self.oublier(guid_moniteur);
                    anyhow::bail!(
                        "le pilote a rendu {rendus} octets pour une sortie créée, \
                         {attendus} attendus — la disposition supposée de \
                         VIRTUAL_DISPLAY_ADD_OUT est fausse ; la sortie a été retirée"
                    );
                }
                Err(erreur) => {
                    // Le GUID reste dans `a_purger` à dessein : c'est la seule
                    // trace de ce qu'il faut retirer. Il n'entre PAS dans
                    // `apparies` — un identifiant douteux qui y figurerait
                    // pourrait apparier un `detruire` ultérieur et lui faire
                    // retirer la mauvaise sortie.
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

        // Le compte d'octets est bon : l'identifiant est fiable. La sortie
        // passe de « retrait dû » à « appariée ».
        let id = ajoutee.identifiant_cible;
        let mut etat = self.etat();
        etat.a_purger.retain(|connu| *connu != guid_moniteur);
        etat.apparies.push((id, guid_moniteur));
        drop(etat);

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

        // L'appariement n'est retiré qu'APRÈS l'appel, jamais avant : sur
        // échec, le GUID est la seule prise que le projet ait sur ce moniteur,
        // et l'oublier le rendrait irrécupérable.
        match self.retirer_par_guid(guid_moniteur, &format!("destruction de la sortie {id}")) {
            Ok(()) => {
                self.oublier(guid_moniteur);
                tracing::info!(id, "sortie virtuelle détruite");
                Ok(())
            }
            Err(erreur) => {
                // Le GUID change de liste plutôt que d'être oublié ou laissé
                // en place. Le laisser dans `apparies` serait le vrai danger :
                // un pilote d'affichage réattribue couramment ses identifiants
                // de cible, et cette entrée périmée apparierait alors un
                // `detruire` ultérieur portant le même identifiant — le
                // mauvais GUID partirait au pilote, et la sortie vivante ne
                // serait jamais détruite. Le retrait reste dû, il n'est
                // simplement plus adressable par identifiant.
                let mut etat = self.etat();
                etat.apparies.retain(|(_, connu)| *connu != guid_moniteur);
                etat.a_purger.push(guid_moniteur);
                drop(etat);
                tracing::error!(
                    id,
                    guid = ?guid_moniteur,
                    %erreur,
                    "sortie virtuelle NON détruite — purge manuelle requise"
                );
                Err(erreur)
            }
        }
    }
}

impl Drop for PiloteParIoctl {
    fn drop(&mut self) {
        // Dernière occasion de dire ce qui reste dû. Fermer le périphérique ne
        // retire rien : une sortie virtuelle survit au processus. Ces GUID sont
        // ce qu'une purge — celle de la tâche 7, ou un humain — devra viser.
        let restants = self.etat().a_purger.clone();
        if !restants.is_empty() {
            tracing::error!(
                nombre = restants.len(),
                guids = ?restants,
                "sorties virtuelles créées et NON retirées — elles survivent à ce \
                 processus, purge requise"
            );
        }
        let _ = unsafe { CloseHandle(self.peripherique) };
    }
}
