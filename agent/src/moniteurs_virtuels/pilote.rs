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
//! **Le chien de garde est ici COMMANDABLE, pas armé.** Le pilote expose
//! `IOCTL_DRIVER_PING` et `IOCTL_GET_WATCHDOG` : un client qui cesse de
//! pinguer voit ses sorties retirées. Ce module rend les deux disponibles
//! (`pinguer`, `veille`) mais ne lance aucune cadence de lui-même — c'est
//! l'appelant qui tient des sorties vivantes, donc c'est à lui de battre. Voir
//! `montee.rs`, qui pingue et qui a mesuré ce que ce chien de garde fait
//! réellement.

use std::sync::Mutex;

use anyhow::{Context, Result};
use windows::core::{GUID, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::IO::DeviceIoControl;

use crate::moniteurs_virtuels::guid::{guid_pour, numero_de};
use crate::moniteurs_virtuels::peripherique::chemin_du_peripherique;
use crate::moniteurs_virtuels::sudovda::{
    en_champ_14, DemandeAjout, DemandeRetrait, SortieAjoutee, IOCTL_AJOUTER_SORTIE,
    IOCTL_RETIRER_SORTIE,
};
use super::{Adaptateur, IdSortie, PiloteAffichageVirtuel};

/// Les trois IOCTL sans effet de bord (version, ping, veille), extraites pour
/// tenir sous le plafond de 500 lignes — voir son commentaire de tête. Module
/// ENFANT : c'est ce qui lui laisse l'accès à `commander`, restée privée.
mod controle;

/// L'état retenu entre deux appels, extrait pour la même raison que
/// `controle` juste au-dessus — voir son commentaire de tête. Module ENFANT :
/// c'est ce qui laisse ses champs `pub(super)` lisibles ici, et nulle part
/// ailleurs.
///
/// ⚠️ Le module `etat` et la méthode `etat()` plus bas portent le même nom
/// dans deux espaces de noms différents : c'est légal, et c'est délibéré.
mod etat;
use etat::EtatSorties;

pub(crate) struct PiloteParIoctl {
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
pub(crate) fn ouvrir_pilote() -> Result<PiloteParIoctl> {
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
    ///
    /// Indexer par GUID suppose leur UNICITÉ, et c'est le distributeur de
    /// `numeros` qui la porte : deux sorties vivantes ne peuvent pas partager
    /// un numéro, puisqu'un numéro n'est rendu qu'après un retrait RÉUSSI.
    ///
    /// **N'appeler que sur une sortie réellement retirée.** C'est ici que le
    /// numéro repart au distributeur (correctif I1) : l'appeler sur un retrait
    /// en échec ferait réattribuer le GUID d'un moniteur encore vivant.
    ///
    /// `pub(super)` : `purge::rejouer_purge_due` l'appelle après un retrait
    /// réussi, pour la même raison que `detruire` l'appelle ici.
    pub(super) fn oublier(&self, guid_moniteur: GUID) {
        let mut etat = self.etat();
        etat.a_purger.retain(|connu| *connu != guid_moniteur);
        etat.apparies.retain(|(_, connu, _)| *connu != guid_moniteur);
        // `None` seulement pour un GUID qui ne vient pas de notre gabarit :
        // rien à rendre, et surtout rien à deviner (voir `guid::numero_de`).
        if let Some(numero) = numero_de(guid_moniteur) {
            etat.numeros.rendre(numero);
        }
    }

    /// Retire du pilote la sortie portant ce GUID.
    ///
    /// Extrait de `detruire` parce que `creer` doit pouvoir l'appeler aussi,
    /// sur son chemin d'échec — là où aucun `IdSortie` fiable n'existe.
    ///
    /// `pub(super)` : c'est aussi la porte par laquelle `purge.rs` retire des
    /// sorties que ce processus n'a jamais créées — un GUID régénéré par
    /// `guid_pour`, hors de `apparies` et `a_purger`.
    pub(super) fn retirer_par_guid(&self, guid_moniteur: GUID, quoi: &str) -> Result<()> {
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

    /// Instantané des GUID dont un retrait précédent a échoué et reste dû.
    ///
    /// `pub(super)` pour `purge::rejouer_purge_due`, qui referme la dette
    /// laissée par la tâche 5 : sans lecteur, `a_purger` ne servait qu'à
    /// journaliser un retrait raté, jamais à le retenter.
    pub(super) fn a_purger(&self) -> Vec<GUID> {
        self.etat().a_purger.clone()
    }

    /// L'adaptateur sur lequel une sortie appariée a été créée.
    ///
    /// `None` pour un identifiant que ce pilote n'a pas créé, ou dont le
    /// retrait a échoué : l'entrée a alors quitté `apparies`, à dessein (voir
    /// la doc d'`EtatSorties`). Un `None` fait retomber l'appelant sur son
    /// repli, jamais sur une devinette.
    ///
    /// ⚠️ **Le couple rendu ne sert QU'À DÉSIGNER une cible d'affichage,
    /// jamais à détruire** : le pilote ne retire que par GUID.
    pub(crate) fn adaptateur_de(&self, id: IdSortie) -> Option<Adaptateur> {
        self.etat()
            .apparies
            .iter()
            .find(|(connu, _, _)| *connu == id)
            .map(|(_, _, adaptateur)| *adaptateur)
    }
}

impl PiloteAffichageVirtuel for PiloteParIoctl {
    fn creer(&self, largeur: u32, hauteur: u32, hertz: u32) -> Result<IdSortie> {
        // Ce verrou est tenu pendant le `DeviceIoControl` d'ajout, un appel
        // noyau bloquant : sans conséquence tant que la montée en N reste
        // séquentielle, à revoir si elle cesse de l'être.
        let mut etat = self.etat();
        // Un refus ici est un refus de créer, et c'est voulu : au-delà du
        // plafond, le GUID attribué sortirait de la plage que la purge
        // inter-processus balaye, et la sortie deviendrait irrécupérable sans
        // redémarrage de la VM (voir `numeros`). Mieux vaut une fenêtre
        // refusée bruyamment — la table du superviseur sait déjà quoi faire
        // d'un refus de création — qu'un moniteur fantôme irrétirable.
        let numero = etat.numeros.attribuer()?;
        let guid_moniteur = guid_pour(numero);

        let demande = DemandeAjout {
            largeur,
            hauteur,
            hertz,
            guid_moniteur,
            nom_peripherique: en_champ_14("Guacamole"),
            numero_serie: en_champ_14(&format!("mesure{numero}")),
        };
        let mut ajoutee = SortieAjoutee::default();
        let rendus = match self.commander(
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
        ) {
            Ok(rendus) => rendus,
            Err(erreur) => {
                // AUCUNE sortie n'existe : le numéro n'est dû à personne et
                // repart au distributeur. Sans cela, une série de refus du
                // pilote — dont le vivier de dix est PLUS BAS que notre
                // plafond de seize, donc atteint le premier — consommerait des
                // numéros pour rien et finirait par faire refuser toute
                // création alors que le pilote, lui, aurait de la place.
                etat.numeros.rendre(numero);
                return Err(erreur);
            }
        };

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
        // 🔴 LES TROIS NOMBRES SONT RETENUS, PLUS SEULEMENT LE TROISIÈME.
        // L'adaptateur n'était que journalisé douze lignes plus bas, puis
        // jeté — et sans lui, `identifiant_cible` ne désigne rien : un
        // identifiant de cible n'est unique que PAR adaptateur. C'est ce
        // couple que `config_affichage` échange contre un nom GDI.
        let adaptateur: Adaptateur = (ajoutee.adaptateur_bas, ajoutee.adaptateur_haut);
        let mut etat = self.etat();
        etat.a_purger.retain(|connu| *connu != guid_moniteur);
        etat.apparies.push((id, guid_moniteur, adaptateur));
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
            .position(|(connu, _, _)| *connu == id)
            .with_context(|| format!("sortie {id} inconnue de ce pilote — rien à détruire"))?;
        let (_, guid_moniteur, _) = etat.apparies[rang];
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
                etat.apparies.retain(|(_, connu, _)| *connu != guid_moniteur);
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
        //
        // LES DEUX listes sont dues ici, pas seulement `a_purger`. La
        // distinction qui les sépare — « une entrée d'`apparies` reste
        // redemandable par identifiant » — cesse d'avoir un sens au moment
        // précis où le processus se termine : plus personne ne redemandera
        // rien. Un appelant qui emploie `creer` sans passer par la garde
        // `Sorties`, ou dont la garde a été neutralisée, laisserait sinon N
        // moniteurs derrière lui dans le silence total.
        //
        // Les deux origines sont distinguées parce qu'elles ne diagnostiquent
        // pas la même chose : un GUID d'`apparies` accuse un appelant qui n'a
        // pas utilisé la garde, un GUID d'`a_purger` accuse un retrait que le
        // pilote a refusé.
        let etat = self.etat();
        let apparies: Vec<GUID> = etat.apparies.iter().map(|(_, guid, _)| *guid).collect();
        let a_purger = etat.a_purger.clone();
        // Numéros attribués et non rendus : normalement égal au nombre de GUID
        // ci-dessous. Un écart signalerait une fuite du distributeur (un numéro
        // consommé par une création qui n'a rien créé), c'est-à-dire une place
        // perdue dans une plage volontairement étroite.
        let numeros_en_vol = etat.numeros.en_vol();
        drop(etat);
        if !apparies.is_empty() || !a_purger.is_empty() {
            tracing::error!(
                nombre = apparies.len() + a_purger.len(),
                numeros_en_vol,
                guids_jamais_detruits = ?apparies,
                guids_dont_le_retrait_a_echoue = ?a_purger,
                "sorties virtuelles créées et NON retirées — elles survivent à ce \
                 processus, purge requise"
            );
        }
        let _ = unsafe { CloseHandle(self.peripherique) };
    }
}
