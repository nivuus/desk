//! Le job object d'APPARTENANCE : à qui la fenêtre appartient-elle ?
//!
//! 🔴 **LA RÈGLE, TRANCHÉE PAR LE PROPRIÉTAIRE (30 août 2026) : `desk` n'adopte
//! que les fenêtres des applications qu'il a LUI-MÊME lancées, et de leur
//! descendance.** C'est une règle d'appartenance, pas une liste noire : Apollo,
//! Steam et le reste sont ignorés **sans être nommés nulle part**, ce qu'une
//! liste noire ne saurait pas faire sans vieillir.
//!
//! ⚠️ **CE QU'ELLE EMPORTE, ET C'EST ASSUMÉ, PAS UNE RÉGRESSION** : une fenêtre
//! **déjà ouverte avant `desk`** n'est plus reprise. Mesuré le 30 août 2026 :
//! `cmd.exe` et `Forza Horizon 6` quittent le hub, et c'est la contrepartie que
//! le propriétaire a acceptée en connaissance de cause.
//!
//! ## Pourquoi un JOB et non la chaîne de parenté
//!
//! Remonter les parents fonctionne — c'est ce que la mesure du 30 août a fait —
//! mais **une chaîne se casse quand un intermédiaire meurt**, ce qui est le cas
//! ordinaire d'un lanceur qui rend la main, et un PID parent est **réutilisable**
//! après la mort du processus. L'appartenance deviendrait **intermittente**,
//! c'est-à-dire la panne la plus coûteuse à diagnostiquer. Le job, lui, est une
//! propriété **portée par le processus**, héritée par ses descendants, et
//! insensible à la mort de qui l'a créé.
//!
//! ## 🔴 CE QUI A ÉTÉ MESURÉ AVANT D'ÉCRIRE UNE LIGNE — et qui a corrigé le produit
//!
//! `apps/lancement.rs` affirmait que le processus lancé « n'entre dans aucun
//! job object », **par une conséquence qu'il énonçait lui-même** : que
//! `superviseur/lanceur.rs` « n'assigne que ses ENFANTS et jamais lui-même ».
//! 🔴 **MESURÉ FAUX le 30 août 2026** : le superviseur **EST** dans un job —
//! celui du **Planificateur de tâches**, qui lance l'agent — et les applications
//! qu'il lance en **héritent** (`DANS_UN_JOB=True` pour le superviseur, ses
//! douze enfants, et le `notepad.exe` lancé par le catalogue).
//!
//! **Conséquence directe pour la conception** : `IsProcessInJob(p, NULL)` ne
//! discrimine rien du tout ici — tout est dans un job. La question qui vaut est
//! **« dans CE job-ci »**, avec notre poignée.
//!
//! Et la précondition qui restait, **mesurée elle aussi** :
//!
//! ```text
//! CIBLE notepad dans_NOTRE_job_avant=False ASSIGNATION=True err=0
//!               dans_NOTRE_job_apres=True
//! SURVIE_APRES_FERMETURE notepad = 1
//! ```
//!
//! **L'assignation IMBRIQUÉE réussit** sur un processus déjà dans le job du
//! Planificateur (Windows 10.0.26100), **et fermer notre job ne tue rien** —
//! parce qu'on ne pose PAS `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`. C'est ce
//! drapeau, et non le job, que `apps/lancement.rs` avait raison de refuser :
//! il tuerait les applications de l'utilisateur au premier redéploiement.
//!
//! ## ⚠️ La course, nommée et bornée
//!
//! `ShellExecuteExW` ne sait pas créer un processus **suspendu** : il n'y a pas
//! de patron « créer suspendu, assigner, reprendre » sur ce chemin. Entre le
//! retour de `ShellExecuteExW` et `AssignProcessToJobObject`, il s'écoule un
//! appel système — **des microsecondes** —, et un descendant créé dans cet
//! intervalle **n'hériterait pas** du job.
//!
//! **Pourquoi c'est acceptable** : le processus lancé, lui, est toujours
//! assigné (c'est son handle qu'on tient) ; seule une fenêtre créée par un
//! petit-enfant né dans cette fenêtre de quelques microsecondes serait écartée.
//! Elle le serait **bruyamment** — voir la trace de refus de
//! `superviseur::hook` —, jamais en silence. **Une course nommée et bornée est
//! un risque ; une course tue est un défaut.**

use std::sync::OnceLock;

/// La variable de banc qui DÉSARME la règle d'appartenance.
///
/// ⚠️ **`=0` DÉSARME ; une simple PRÉSENCE n'active pas** — convention de
/// `PLEIN_ECRAN`, `AUDIO`, `SUPERVISEUR`, `CAPTEUR`, `PART_SONDAGE`,
/// `PRESSE_PAPIER`, `APPS`, `PONT_ECRITURE` et `SORTIE_DESIGNEE`. Le prédicat
/// est **RÉUTILISÉ, pas recopié** : `crate::apps::desarme`.
///
/// Désarmée, le produit adopte de nouveau toute fenêtre qui passe le critère —
/// **c'est le bras ROUGE de la recette**, et il survit au binaire d'avant.
pub fn armee() -> bool {
    static ARMEE: OnceLock<bool> = OnceLock::new();
    *ARMEE.get_or_init(|| {
        let armee = !crate::apps::desarme(std::env::var("APPARTENANCE").ok().as_deref());
        if !armee {
            tracing::warn!(
                "appartenance DESARMEE (APPARTENANCE=0) : desk adopte de nouveau les \
                 fenetres qu'il n'a pas lancees — bras de banc, jamais une configuration livree"
            );
        }
        armee
    })
}

#[cfg(windows)]
mod win {
    use std::sync::OnceLock;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, IsProcessInJob,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    /// Le job d'appartenance du processus. **Aucune limite n'y est posée**, et
    /// c'est tout le point : il ne sert qu'à répondre « ce processus est-il des
    /// nôtres ? ». Voir l'en-tête du module.
    struct Job(HANDLE);
    // SAFETY : un HANDLE de job est un objet noyau global au processus, sans
    // affinité de fil. Il n'est jamais fermé — le fermer ne tue rien (mesuré),
    // et sa durée de vie est celle du processus.
    unsafe impl Send for Job {}
    unsafe impl Sync for Job {}

    static JOB: OnceLock<Option<Job>> = OnceLock::new();

    fn job() -> Option<HANDLE> {
        JOB.get_or_init(|| match unsafe { CreateJobObjectW(None, None) } {
            Ok(h) if !h.is_invalid() => {
                tracing::info!("job d'appartenance créé (aucune limite : il ne tue rien)");
                Some(Job(h))
            }
            Ok(_) => {
                tracing::error!("job d'appartenance : poignée invalide, la règle sera INERTE");
                None
            }
            Err(erreur) => {
                tracing::error!(%erreur, "job d'appartenance NON créé — la règle sera INERTE");
                None
            }
        })
        .as_ref()
        .map(|j| j.0)
    }

    /// Inscrit un processus que NOUS venons de lancer dans le job.
    ///
    /// Journalise dans les deux sens : sans la trace d'échec, une application
    /// qui ne paraîtrait jamais serait indiscernable d'une application qui
    /// n'a pas démarré.
    pub fn adopter(processus: HANDLE) {
        let Some(job) = job() else { return };
        match unsafe { AssignProcessToJobObject(job, processus) } {
            Ok(()) => tracing::info!("processus lancé inscrit au job d'appartenance"),
            Err(erreur) => tracing::error!(
                %erreur,
                "processus lancé NON inscrit au job d'appartenance — ses fenêtres \
                 seront ÉCARTÉES, et c'est une panne, pas un refus normal"
            ),
        }
    }

    /// Ce processus est-il des nôtres ?
    ///
    /// ⚠️ **`Some(false)` et `None` ne sont pas la même chose** : le premier est
    /// une réponse (« ce n'est pas à nous »), le second un échec de la question.
    /// L'appelant les distingue, et n'écarte JAMAIS sur un échec de question —
    /// refuser faute d'avoir su demander serait la panne que ce garde existe
    /// pour éviter.
    pub fn est_des_notres(pid: u32) -> Option<bool> {
        let job = job()?;
        let processus = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
        let mut dedans = windows::core::BOOL(0);
        let issue = unsafe { IsProcessInJob(processus, Some(job), &mut dedans) };
        let _ = unsafe { CloseHandle(processus) };
        issue.ok().map(|()| dedans.as_bool())
    }
}

#[cfg(windows)]
pub use win::{adopter, est_des_notres};
