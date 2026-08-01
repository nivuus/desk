//! Lancement d'un processus agent par fenêtre, et contrôle de sa vie.
//!
//! Séparé de `boucle.rs` : c'est la seule partie du superviseur qui parle de
//! processus Windows, et elle satisfait un trait dont `enfants.rs` porte les
//! tests avec un lanceur factice.
//!
//! **Deux invariants tiennent ce fichier**, et ils se lisent tous deux dans le
//! `Child` retenu et dans le Job Object.
//!
//! 1. **Un PID seul ne désigne rien de durable.** Windows ne recycle le numéro
//!    d'un processus mort que lorsque le dernier handle sur l'objet processus
//!    est fermé — or `drop` d'un `std::process::Child` ferme ce handle.
//!    Relâcher le `Child` et ne garder que le PID, c'est accepter qu'un
//!    `est_vivant` rende `true` sur un processus étranger et, bien pire, qu'un
//!    `tuer` appelle `TerminateProcess` sur un tiers. On retient donc le
//!    `Child` : le handle reste ouvert, le numéro reste réservé, et toutes les
//!    opérations passent par ce handle-là.
//! 2. **Un superviseur mort ne doit pas laisser N agents derrière lui.**
//!    `Command::spawn` ne rattache l'enfant à rien : à la mort du superviseur,
//!    chaque agent continuerait de tourner avec sa duplication DXGI, son
//!    encodeur et son inscription au signaling. Le démarrage suivant
//!    attribuerait alors des identifiants de session qui entrent en collision
//!    avec les leurs (`Table::compteur` repart de zéro), et sa purge
//!    détruirait des sorties SOUS des enfants encore vivants. Le Job Object
//!    avec `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` ferme cela au niveau du noyau :
//!    la mort du superviseur ferme ses handles, donc le job, donc les enfants —
//!    y compris s'il a été tué net, cas qu'aucun code en espace utilisateur ne
//!    peut couvrir.
//!
//! **Le contrat de `Lanceur::lancer` est atomique** (voir sa documentation sur
//! le trait) : `Err` doit signifier qu'aucun processus ne tourne. Le
//! rattachement au job est le seul post-traitement faillible d'ici, et il tue
//! donc lui-même l'enfant avant de rendre `Err`. **Toute addition après le
//! `spawn` doit faire de même.**

#![cfg(windows)]

use std::collections::HashMap;
use std::os::windows::io::AsRawHandle;
use std::sync::Mutex;

use anyhow::{Context, Result};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

use super::enfants::{Consigne, Lanceur};

pub struct LanceurDeProcessus {
    executable: std::path::PathBuf,
    signaling_url: String,
    local_ip: String,
    /// Les enfants vivants, par PID. Retenus pour leur HANDLE, pas pour leur
    /// numéro — voir l'invariant 1 en tête de module.
    ///
    /// `Mutex` et non `RefCell` : `Lanceur` prend `&self`, et rien ne promet
    /// que ce lanceur restera consulté depuis un seul fil.
    enfants: Mutex<HashMap<u32, std::process::Child>>,
    /// Le job auquel tout enfant est rattaché. Sa fermeture les tue.
    job: HANDLE,
}

impl LanceurDeProcessus {
    pub fn nouveau(
        executable: std::path::PathBuf,
        signaling_url: String,
        local_ip: String,
    ) -> Result<Self> {
        let job =
            unsafe { CreateJobObjectW(None, None) }.context("création du job object des enfants")?;
        let limites = JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
            BasicLimitInformation: windows::Win32::System::JobObjects::JOBOBJECT_BASIC_LIMIT_INFORMATION {
                LimitFlags: JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                ..Default::default()
            },
            ..Default::default()
        };
        unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &limites as *const _ as *const core::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        }
        .context("pose de KILL_ON_JOB_CLOSE sur le job object des enfants")?;
        Ok(Self {
            executable,
            signaling_url,
            local_ip,
            enfants: Mutex::new(HashMap::new()),
            job,
        })
    }

    /// Accès à la table sans paniquer sur un verrou empoisonné : ce chemin
    /// court aussi depuis `Drop`, où une panique abrégerait le processus et
    /// laisserait les enfants sans personne pour les compter.
    fn enfants(&self) -> std::sync::MutexGuard<'_, HashMap<u32, std::process::Child>> {
        self.enfants.lock().unwrap_or_else(|empoisonne| empoisonne.into_inner())
    }
}

impl Lanceur for LanceurDeProcessus {
    fn lancer(&self, consigne: &Consigne) -> Result<u32> {
        let mut enfant = std::process::Command::new(&self.executable)
            .env("SESSION_ID", &consigne.session.0)
            .env("SIGNALING_URL", &self.signaling_url)
            .env("LOCAL_IP", &self.local_ip)
            .env("FENETRE_HWND", format!("{:#x}", consigne.fenetre))
            .env(
                "SORTIE_DXGI",
                format!("{}:{}", consigne.index_adaptateur, consigne.index_sortie),
            )
            .env("AUDIO", if consigne.audio { "1" } else { "0" })
            // Surtout PAS `SUPERVISEUR` : un enfant qui hériterait de la
            // variable se prendrait pour un superviseur et lancerait ses
            // propres enfants, indéfiniment.
            .env_remove("SUPERVISEUR")
            .spawn()
            .with_context(|| format!("lancement de l'enfant {}", consigne.session.0))?;
        let pid = enfant.id();

        // Le seul post-traitement faillible de cette fonction. Le contrat
        // atomique du trait exige donc qu'il tue lui-même l'enfant avant de
        // rendre `Err` — sans quoi celui-ci tournerait sans être suivi et sa
        // sortie virtuelle resterait captive du vivier de dix.
        let handle = HANDLE(enfant.as_raw_handle() as *mut core::ffi::c_void);
        if let Err(erreur) = unsafe { AssignProcessToJobObject(self.job, handle) } {
            if let Err(mise_a_mort) = enfant.kill() {
                tracing::error!(
                    pid, %mise_a_mort,
                    "enfant NON rattaché au job ET NON tué — il survivra au superviseur"
                );
            }
            let _ = enfant.wait();
            return Err(anyhow::Error::new(erreur)
                .context(format!("rattachement de l'enfant {pid} au job object")));
        }

        self.enfants().insert(pid, enfant);
        Ok(pid)
    }

    fn est_vivant(&self, pid: u32) -> bool {
        let mut enfants = self.enfants();
        let Some(enfant) = enfants.get_mut(&pid) else {
            // Inconnu de ce lanceur : jamais lancé par nous, ou mort déjà
            // constatée. Dans les deux cas il n'est pas vivant *pour nous*, et
            // c'est la seule question posée.
            return false;
        };
        match enfant.try_wait() {
            Ok(None) => true,
            Ok(Some(code)) => {
                tracing::info!(pid, ?code, "enfant terminé");
                // Le `Child` part avec son handle : le PID redevient
                // recyclable, mais plus personne ne s'en sert.
                enfants.remove(&pid);
                false
            }
            Err(erreur) => {
                // Un état illisible n'est PAS une mort : le déclarer mort ferait
                // détruire la sortie d'un enfant qui capture encore.
                tracing::warn!(pid, %erreur, "état de l'enfant illisible, tenu pour vivant");
                true
            }
        }
    }

    fn tuer(&self, pid: u32) -> Result<()> {
        let mut enfant = self
            .enfants()
            .remove(&pid)
            .with_context(|| format!("processus {pid} inconnu de ce lanceur — rien à tuer"))?;
        // `Child::kill` passe par le HANDLE retenu, jamais par le numéro : même
        // si Windows avait recyclé ce PID, aucun tiers ne peut être visé.
        let issue = enfant.kill();
        let _ = enfant.wait();
        issue.with_context(|| format!("terminaison du processus {pid}"))
    }
}

impl Drop for LanceurDeProcessus {
    fn drop(&mut self) {
        // Fermer le job tue ce qu'il contient (`KILL_ON_JOB_CLOSE`). Les
        // `Child` restants partent avec la table et leurs handles se ferment
        // aussi ; l'ordre n'importe pas, le job est la garantie de fond.
        let _ = unsafe { CloseHandle(self.job) };
    }
}
