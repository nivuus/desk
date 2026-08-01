//! Lancement d'un processus agent par fenêtre, et contrôle de sa vie.
//!
//! Séparé de `boucle.rs` : c'est la seule partie du superviseur qui parle de
//! processus Windows, et elle satisfait un trait dont `enfants.rs` porte les
//! tests avec un lanceur factice.
//!
//! **Le contrat de `Lanceur::lancer` est atomique** (voir sa documentation sur
//! le trait) : `Err` doit signifier qu'aucun processus ne tourne. Il est tenu
//! ici par construction — `spawn` est le dernier appel, et rien ne s'exécute
//! après lui qui puisse échouer. **Ne rien ajouter après ce `spawn`** : tout
//! post-traitement faillible (attente d'un signal de disponibilité, écriture
//! d'un fichier d'état…) devrait alors tuer lui-même l'enfant avant de rendre
//! `Err`, faute de quoi celui-ci deviendrait intraçable et sa sortie virtuelle
//! captive.

#![cfg(windows)]

use anyhow::{Context, Result};

use super::enfants::{Consigne, Lanceur};

pub struct LanceurDeProcessus {
    pub executable: std::path::PathBuf,
    pub signaling_url: String,
    pub local_ip: String,
}

impl Lanceur for LanceurDeProcessus {
    fn lancer(&self, consigne: &Consigne) -> Result<u32> {
        let enfant = std::process::Command::new(&self.executable)
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
        Ok(enfant.id())
    }

    fn est_vivant(&self, pid: u32) -> bool {
        // `OpenProcess` sur un PID mort échoue : c'est le contrôle le moins
        // cher qui ne dépende pas d'avoir gardé le `Child`.
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{
            GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };
        unsafe {
            let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
                return false;
            };
            let mut code = 0u32;
            let vivant = GetExitCodeProcess(handle, &mut code).is_ok()
                // 259 = STILL_ACTIVE.
                && code == 259;
            let _ = CloseHandle(handle);
            vivant
        }
    }

    fn tuer(&self, pid: u32) -> Result<()> {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
        unsafe {
            let handle = OpenProcess(PROCESS_TERMINATE, false, pid)
                .with_context(|| format!("ouverture du processus {pid} pour le terminer"))?;
            let issue = TerminateProcess(handle, 1);
            let _ = CloseHandle(handle);
            issue.with_context(|| format!("terminaison du processus {pid}"))?;
        }
        Ok(())
    }
}
