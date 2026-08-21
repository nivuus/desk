//! UNE racine surveillée : l'ouvrir, armer une lecture, compléter, rouvrir.
//!
//! 🔴 `#[cfg(windows)]`, ET **AUCUN TEST D'HÔTE N'EST POSSIBLE** — comme
//! `apps/lecture.rs`, qui le dit de lui-même. La seule vérification disponible
//! est `cargo check --target x86_64-pc-windows-gnu`, qui couvre **types,
//! emprunts, visibilités et durées de vie**, et **PAS** l'édition de liens ni le
//! comportement. Tout ce qui DÉCIDE quelque chose vit donc ailleurs, dans les
//! quatre modules purs de ce répertoire.
//!
//! 🔴 **LE CONTENU DU TAMPON N'EST JAMAIS LU, ET C'EST LA SIMPLIFICATION
//! CENTRALE DU SOUS-BLOC.** Une réconciliation relit le disque ENTIER : il n'y a
//! donc rien à tirer du nom du fichier qui a bougé. Ce que cela retire du
//! produit :
//!
//! - aucune chaîne UTF-16 à décoder, aucune chaîne de `NextEntryOffset` à
//!   suivre, **aucun aliasing de tampon** — trois familles de défaut qui
//!   n'existeront pas ;
//! - **aucune tentation de filtrer sur `.lnk`**, laquelle serait de toute façon
//!   IMPOSSIBLE À TENIR : un débordement **jette le tampon entier**, donc le
//!   chemin « je ne sais pas ce qui a changé » doit exister quoi qu'il arrive.
//!   Écrire un filtre qui ne couvre pas ce cas serait écrire deux chemins pour
//!   en servir un.
//!
//! ⚠️ **LE COÛT, NOMMÉ** : n'importe quelle écriture sous les quatre
//! arborescences déclenche une réconciliation, y compris un fichier temporaire
//! qui n'a rien à voir avec un raccourci. C'est exactement ce que l'anti-rebond
//! borne, et c'est aussi ce qui rend mesurable — au lieu de théorique — la
//! question « une racine est-elle bruyante au repos ? ».

use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{
    CloseHandle, ERROR_NOTIFY_ENUM_DIR, ERROR_OPERATION_ABORTED, HANDLE,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, ReadDirectoryChangesW, FILE_FLAGS_AND_ATTRIBUTES, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_FLAG_OVERLAPPED, FILE_LIST_DIRECTORY, FILE_NOTIFY_CHANGE_DIR_NAME,
    FILE_NOTIFY_CHANGE_FILE_NAME, FILE_NOTIFY_CHANGE_LAST_WRITE, FILE_SHARE_DELETE,
    FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};
use windows::Win32::System::Threading::CreateEventW;

use super::faute::{self, Famille};
use super::TAMPON_NOTIFICATIONS;

/// Ce qu'une complétion nous apprend.
#[derive(Debug)]
pub(super) enum Issue {
    /// Quelque chose a bougé. **On ne sait pas quoi, et on ne veut pas le
    /// savoir** (voir l'en-tête).
    Notification,
    /// Le tampon a débordé : son contenu est perdu.
    ///
    /// ⚠️ **L'ÉVÉNEMENT, LUI, NE L'EST PAS** — la complétion s'est produite, et
    /// c'est elle qui déclenchera la réconciliation qui rattrape tout.
    Debordement,
    /// L'attente a été annulée : c'est l'arrêt, pas une panne.
    Annulee,
    /// La racine n'est plus surveillable en l'état.
    Perte(anyhow::Error),
    /// 🔵 **INJECTÉE, ET INJECTABLE UNIQUEMENT** : la complétion est AVALÉE —
    /// ni comptée, ni journalisée, ni déclenchante. Aucun chemin réel ne la
    /// produit, et c'est tout son objet : elle fabrique la seule panne que la
    /// réconciliation périodique achète réellement, celle d'une surveillance
    /// qui **cesse de délivrer SANS ERREUR**.
    ///
    /// ✅ **MESURÉE, ET C'EST LE SEUL MONTAGE DISCRIMINANT DU CRITÈRE ③**
    /// (deux exécutions par bras) : `notifications=0` des deux côtés — les
    /// complétions sont bien avalées —, et le catalogue passe à `cles=157` par
    /// `declencheur="periode"` quand la période est armée, contre `cles=156`
    /// **indéfiniment** sous `APPS_SURVEILLANCE=seule`.
    ///
    /// ⚠️ **ELLE ÉTABLIT QUE LE REMÈDE FONCTIONNE, JAMAIS QU'UNE CAUSE EXISTE.**
    Avalee,
}

/// Une racine ouverte, avec sa lecture en vol.
///
/// ⚠️ `OVERLAPPED` ET LE TAMPON SONT DES `Box` : le noyau écrit dedans pendant
/// que l'appel est en vol, et leurs adresses doivent donc rester stables. Un
/// champ par valeur bougerait avec la structure.
pub(super) struct Racine {
    chemin: PathBuf,
    repertoire: HANDLE,
    evenement: HANDLE,
    overlapped: Box<OVERLAPPED>,
    /// 🔴 UN `Vec<u32>` ET NON UN `Vec<u8>` : `FILE_NOTIFY_INFORMATION` doit
    /// être aligné sur une frontière de `DWORD`, et c'est un **contrat de
    /// l'appel**, pas une précaution. Un `Vec<u8>` n'est aligné que sur 1.
    ///
    /// ⚠️ Nous ne lisons jamais ce tampon (voir l'en-tête) — l'alignement est
    /// donc exigé pour ce que le NOYAU y écrit, pas pour ce que nous en
    /// ferions.
    tampon: Box<[u32]>,
    /// Le nombre d'échecs consécutifs, pour le repli exponentiel.
    echecs: u32,
    /// `Some` si la racine est en échec : l'instant de la prochaine tentative.
    reprise: Option<std::time::Instant>,
}

impl Racine {
    /// Ouvre une racine et arme sa première lecture.
    pub(super) fn ouvrir(chemin: PathBuf) -> Result<Self> {
        let (repertoire, evenement) = ouvrir_les_deux_handles(&chemin)?;
        let mut racine = Self {
            chemin,
            repertoire,
            evenement,
            overlapped: Box::new(OVERLAPPED {
                hEvent: evenement,
                ..Default::default()
            }),
            // `TAMPON_NOTIFICATIONS` est en OCTETS ; le `Vec` est en `u32`.
            tampon: vec![0u32; TAMPON_NOTIFICATIONS / 4].into_boxed_slice(),
            echecs: 0,
            reprise: None,
        };
        racine.armer()?;
        Ok(racine)
    }

    pub(super) fn chemin(&self) -> &Path {
        &self.chemin
    }

    /// L'événement à passer à `WaitForMultipleObjects`.
    pub(super) fn evenement(&self) -> HANDLE {
        self.evenement
    }

    /// Cette racine est-elle en échec ?
    pub(super) fn en_echec(&self) -> bool {
        self.reprise.is_some()
    }

    /// Le repli est-il échu ?
    pub(super) fn reprise_due(&self, maintenant: std::time::Instant) -> bool {
        self.reprise.is_some_and(|due| maintenant >= due)
    }

    /// Arme — ou réarme — une lecture.
    ///
    /// ⚠️ **`lpBytesReturned` EST `None` ICI, ET C'EST OBLIGATOIRE** : sur un
    /// handle chevauchant, ce paramètre est *undefined* et le lire serait une
    /// course. Le compte réel se prend à la complétion, par
    /// `GetOverlappedResult`.
    pub(super) fn armer(&mut self) -> Result<()> {
        let octets = std::mem::size_of_val(&*self.tampon) as u32;
        // SÉCURITÉ : appel FFI. `repertoire` vient d'un `CreateFileW` réussi ;
        // `tampon` et `overlapped` sont des `Box`, donc d'adresse stable pour
        // toute la durée de vie de `self`, ce que le noyau exige d'une lecture
        // en vol.
        unsafe {
            ReadDirectoryChangesW(
                self.repertoire,
                self.tampon.as_mut_ptr().cast(),
                octets,
                // `bWatchSubtree` : le menu Démarrer porte l'immense majorité
                // des raccourcis de cette VM, presque tous dans des
                // sous-dossiers par éditeur. Un guet à plat n'en verrait
                // quasiment aucun — même raison que le parcours récursif de
                // `lecture::lnk_sous`.
                true,
                FILE_NOTIFY_CHANGE_FILE_NAME
                    | FILE_NOTIFY_CHANGE_DIR_NAME
                    | FILE_NOTIFY_CHANGE_LAST_WRITE,
                None,
                Some(&mut *self.overlapped),
                None,
            )
        }
        .with_context(|| format!("ReadDirectoryChangesW sur {}", self.chemin.display()))
    }

    /// Relève ce que la complétion dit, **sans jamais lire le tampon**.
    ///
    /// 🔴 L'INJECTION EST CONSULTÉE **AVANT** LE CLASSEMENT RÉEL, et dans cet
    /// ordre : `Muette` d'abord, puisqu'elle doit court-circuiter jusqu'au
    /// comptage. Le budget est **global au processus** (voir `faute.rs`), donc
    /// une racine qui se rouvre ne le réarme pas — c'est la panne de mesure que
    /// le sous-bloc D10 a payée sur `AUDIO_FAUTE_LECTURE`.
    pub(super) fn completer(&mut self) -> Issue {
        if faute::consommer(Famille::Muette) {
            return Issue::Avalee;
        }
        if faute::consommer(Famille::Debordement) {
            return Issue::Debordement;
        }
        if faute::consommer(Famille::Perte) {
            return Issue::Perte(anyhow::anyhow!("faute injectée (APPS_FAUTE=perte)"));
        }
        let mut octets: u32 = 0;
        // SÉCURITÉ : appel FFI. `bWait = false` : l'événement est déjà signalé
        // quand on arrive ici, et attendre bloquerait le fil qui sert les trois
        // autres racines.
        let issue = unsafe {
            GetOverlappedResult(self.repertoire, &*self.overlapped, &mut octets, false)
        };
        match issue {
            // 🔴 ZÉRO OCTET EST UN DÉBORDEMENT, PAS UNE COMPLÉTION VIDE. C'est
            // la façon dont le noyau dit « le tampon n'a pas suffi, je l'ai
            // jeté » quand il ne rend pas `ERROR_NOTIFY_ENUM_DIR`.
            Ok(()) if octets == 0 => Issue::Debordement,
            Ok(()) => Issue::Notification,
            Err(erreur) if erreur.code() == ERROR_NOTIFY_ENUM_DIR.to_hresult() => {
                Issue::Debordement
            }
            // L'annulation est ce que `CancelIoEx` provoque à l'arrêt : la
            // classer en perte ferait journaliser une panne à chaque
            // extinction propre.
            Err(erreur) if erreur.code() == ERROR_OPERATION_ABORTED.to_hresult() => Issue::Annulee,
            Err(erreur) => Issue::Perte(anyhow::Error::new(erreur).context(format!(
                "GetOverlappedResult sur {}",
                self.chemin.display()
            ))),
        }
    }

    /// Marque la racine en échec et programme sa reprise.
    ///
    /// ⚠️ `delai_de_repli` est **RÉUTILISÉ, PAS RECOPIÉ** : il est déjà testé,
    /// et déjà protégé contre le débordement de décalage (`checked_shl`, sans
    /// quoi la treizième heure d'attente devient un `panic` en `debug`).
    pub(super) fn programmer_la_reprise(&mut self, maintenant: std::time::Instant) {
        let delai = crate::plateforme::repli::delai_de_repli(self.echecs);
        self.echecs = self.echecs.saturating_add(1);
        self.reprise = Some(maintenant + std::time::Duration::from_millis(delai));
    }

    /// Ferme, **re-résout le chemin**, rouvre et réarme.
    ///
    /// ⚠️ LES HANDLES SONT REFERMÉS AVANT, sans quoi chaque tentative en
    /// fuirait deux — et une racine qui échoue est précisément celle qui
    /// réessaiera longtemps.
    pub(super) fn rouvrir(&mut self) -> Result<()> {
        self.fermer();
        let (repertoire, evenement) = ouvrir_les_deux_handles(&self.chemin)?;
        self.repertoire = repertoire;
        self.evenement = evenement;
        self.overlapped = Box::new(OVERLAPPED {
            hEvent: evenement,
            ..Default::default()
        });
        self.armer()?;
        self.echecs = 0;
        self.reprise = None;
        Ok(())
    }

    /// Annule la lecture en vol et ferme les deux handles.
    ///
    /// 🔴 `CancelIoEx` **AVANT** `CloseHandle`, et l'ordre n'est pas
    /// indifférent : fermer un handle dont une lecture est en vol laisse le
    /// noyau écrire dans un tampon que nous allons libérer.
    pub(super) fn fermer(&mut self) {
        if !self.repertoire.is_invalid() {
            // SÉCURITÉ : appels FFI. Les erreurs sont ignorées à dessein — il
            // n'y a rien à faire d'un échec d'annulation pendant une
            // extinction, et `ERROR_NOT_FOUND` est le cas nominal quand aucune
            // lecture n'est en vol.
            unsafe {
                let _ = CancelIoEx(self.repertoire, Some(&*self.overlapped));
                let _ = CloseHandle(self.repertoire);
            }
            self.repertoire = HANDLE::default();
        }
        if !self.evenement.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.evenement);
            }
            self.evenement = HANDLE::default();
        }
    }
}

impl Drop for Racine {
    fn drop(&mut self) {
        self.fermer();
    }
}

/// Ouvre le répertoire et son événement, ou n'en laisse AUCUN des deux ouvert.
///
/// 🔴 SI LE SECOND ÉCHOUE, LE PREMIER EST REFERMÉ. Sans cela, une racine dont
/// l'événement ne se crée pas fuirait un handle de répertoire **à chaque
/// tentative de reprise**, c'est-à-dire indéfiniment.
fn ouvrir_les_deux_handles(chemin: &Path) -> Result<(HANDLE, HANDLE)> {
    let large: Vec<u16> = chemin
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // SÉCURITÉ : appel FFI. `FILE_FLAG_BACKUP_SEMANTICS` est OBLIGATOIRE pour
    // ouvrir un RÉPERTOIRE ; `FILE_SHARE_DELETE` l'est en pratique, sans quoi
    // notre handle empêcherait quiconque de renommer ou supprimer la racine —
    // une surveillance qui gêne ce qu'elle observe.
    let repertoire = unsafe {
        CreateFileW(
            PCWSTR(large.as_ptr()),
            FILE_LIST_DIRECTORY.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(FILE_FLAG_BACKUP_SEMANTICS.0 | FILE_FLAG_OVERLAPPED.0),
            None,
        )
    }
    .with_context(|| format!("ouverture de la racine surveillée {}", chemin.display()))?;

    // `bManualReset = true` : l'événement est remis à l'état non signalé par
    // `ReadDirectoryChangesW` lui-même au moment où il met la lecture en file.
    // Un événement à réarmement automatique serait consommé par l'attente, ce
    // qui est correct aussi — mais le manuel rend l'état observable entre les
    // deux, et c'est ce qu'on veut d'un fil qui sert quatre racines.
    // SÉCURITÉ : appel FFI.
    match unsafe { CreateEventW(None, true, false, PCWSTR::null()) } {
        Ok(evenement) => Ok((repertoire, evenement)),
        Err(erreur) => {
            // SÉCURITÉ : appel FFI, sur un handle que nous venons d'ouvrir.
            unsafe {
                let _ = CloseHandle(repertoire);
            }
            Err(anyhow::Error::new(erreur).context(format!(
                "CreateEventW pour la racine surveillée {}",
                chemin.display()
            )))
        }
    }
}
