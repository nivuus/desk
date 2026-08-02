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

/// Un enfant suivi, et le peu d'état qu'il faut retenir sur lui.
struct Enfant {
    /// Retenu pour son HANDLE, pas pour son numéro — voir l'invariant 1 en
    /// tête de module.
    processus: std::process::Child,
    /// Vrai dès qu'un état illisible a été signalé pour cet enfant.
    ///
    /// **Correctif I2 de la revue finale.** `est_vivant` est appelé à CHAQUE
    /// tour de boucle du superviseur, via `Enfants::morts()` : une erreur
    /// persistante de `try_wait` y produisait une dizaine de lignes par
    /// seconde et PAR ENFANT, sur un partage CIFS. Un signalement unique
    /// suffit — l'état étant persistant par hypothèse, le répéter n'apprend
    /// rien de neuf. Remis à faux si l'état redevient lisible, pour qu'une
    /// seconde occurrence, elle, se voie.
    etat_illisible_signale: bool,
}

pub struct LanceurDeProcessus {
    executable: std::path::PathBuf,
    signaling_url: String,
    local_ip: String,
    /// Les enfants vivants, par PID.
    ///
    /// `Mutex` et non `RefCell` : `Lanceur` prend `&self`, et rien ne promet
    /// que ce lanceur restera consulté depuis un seul fil.
    enfants: Mutex<HashMap<u32, Enfant>>,
    /// Le capteur unique de capture mutualisée (sous-bloc D4), suivi à part
    /// des enfants : il n'a pas de session, donc pas sa place dans `enfants`.
    /// `None` tant qu'aucun capteur n'a encore été lancé, ou juste après que
    /// le précédent a été constaté mort par `capteur_vivant`.
    capteur: Mutex<Option<Enfant>>,
    /// Le job auquel tout enfant — et le capteur — est rattaché. Sa fermeture
    /// les tue tous.
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
            capteur: Mutex::new(None),
            job,
        })
    }

    /// Accès à la table sans paniquer sur un verrou empoisonné.
    ///
    /// Le verrou s'empoisonne dès qu'une panique traverse un porteur — et il y
    /// en a un : `lancer` peut paniquer entre le `spawn` et l'insertion. Après
    /// quoi un `.expect(…)` ferait paniquer **tous** les appels suivants,
    /// c'est-à-dire `est_vivant` et `tuer` : le superviseur perdrait d'un coup
    /// la capacité de constater une mort et celle de mettre à mort. Le job
    /// object rattraperait les enfants à la fin, mais bien plus tard et sans
    /// que rien ne l'explique. `into_inner` rend la table telle quelle : au
    /// pire une insertion interrompue y manque.
    ///
    /// (`Drop` ne passe PAS par ici : il ne touche que le handle de job.)
    fn enfants(&self) -> std::sync::MutexGuard<'_, HashMap<u32, Enfant>> {
        self.enfants.lock().unwrap_or_else(|empoisonne| empoisonne.into_inner())
    }

    /// Même raison que `enfants` ci-dessus : ne pas paniquer sur un verrou
    /// empoisonné, sans quoi une panique isolée dans `lancer_capteur` rendrait
    /// `capteur_vivant` inutilisable pour toujours, et le superviseur ne
    /// pourrait plus jamais constater ni relancer le capteur.
    fn capteur(&self) -> std::sync::MutexGuard<'_, Option<Enfant>> {
        self.capteur.lock().unwrap_or_else(|empoisonne| empoisonne.into_inner())
    }

    /// Lance le capteur unique de capture mutualisée (`agent/src/capteur.rs`) :
    /// même exécutable, `CAPTEUR=1`, **rattaché au même job object** que les
    /// enfants — sans quoi il survivrait au superviseur en tenant N
    /// duplications DXGI et N sorties virtuelles captives du vivier qui n'en
    /// compte que dix.
    ///
    /// **Même contrat atomique que `lancer`** (voir la doc du trait) : `Err`
    /// signifie qu'aucun processus ne tourne. Le rattachement au job est le
    /// seul post-traitement faillible, et il tue donc lui-même le capteur
    /// avant de rendre `Err`.
    pub fn lancer_capteur(&self) -> Result<u32> {
        let mut capteur = std::process::Command::new(&self.executable)
            .env("CAPTEUR", "1")
            // Même motif que pour un enfant (voir `lancer` ci-dessous) : un
            // capteur qui hériterait de `SUPERVISEUR` se prendrait pour un
            // superviseur et lancerait ses propres enfants, indéfiniment.
            .env_remove("SUPERVISEUR")
            // Même motif que pour un enfant : ces deux variables changent le
            // SENS d'une source (un fichier de test à diffuser, une fenêtre à
            // chercher par titre) — et le capteur n'a pas de source unique
            // désignée par elles, il en tient N, chacune décrite par l'enfant
            // qui s'y rattache via le tube nommé.
            .env_remove("TEST_FILE")
            .env_remove("WINDOW_TITLE")
            .spawn()
            .context("lancement du capteur")?;
        let pid = capteur.id();
        let handle = HANDLE(capteur.as_raw_handle() as *mut core::ffi::c_void);
        if let Err(erreur) = unsafe { AssignProcessToJobObject(self.job, handle) } {
            // Même contrat atomique que `lancer` : un capteur non rattaché au
            // job survivrait au superviseur EN TENANT N duplications.
            if let Err(mise_a_mort) = capteur.kill() {
                tracing::error!(pid, %mise_a_mort,
                    "capteur NON rattaché au job ET NON tué — il survivra au superviseur");
            }
            let _ = capteur.wait();
            return Err(anyhow::Error::new(erreur)
                .context(format!("rattachement du capteur {pid} au job object")));
        }
        tracing::info!(pid, "capteur lancé");
        *self.capteur() = Some(Enfant { processus: capteur, etat_illisible_signale: false });
        Ok(pid)
    }

    /// Vrai tant que le capteur lancé par le dernier `lancer_capteur` réussi
    /// est vivant. Rend `false` si aucun capteur n'a jamais été lancé, ou si
    /// le précédent est mort — aux appelants de rappeler `lancer_capteur`.
    ///
    /// Même logique qu'`est_vivant` (voir plus bas) : un état illisible n'est
    /// PAS traité comme une mort — le déclarer mort ferait relancer un
    /// capteur qui tourne peut-être encore, doublant les duplications DXGI —
    /// et n'est journalisé qu'une fois tant qu'il persiste.
    pub fn capteur_vivant(&self) -> bool {
        let mut capteur = self.capteur();
        let Some(en_cours) = capteur.as_mut() else { return false };
        match en_cours.processus.try_wait() {
            Ok(None) => {
                en_cours.etat_illisible_signale = false;
                true
            }
            Ok(Some(code)) => {
                tracing::info!(pid = en_cours.processus.id(), ?code, "capteur terminé");
                *capteur = None;
                false
            }
            Err(erreur) => {
                if !en_cours.etat_illisible_signale {
                    en_cours.etat_illisible_signale = true;
                    tracing::warn!(
                        %erreur,
                        "état du capteur illisible, tenu pour vivant \
                         (signalé une seule fois tant que l'état reste illisible)"
                    );
                }
                true
            }
        }
    }
}

impl Lanceur for LanceurDeProcessus {
    fn lancer(&self, consigne: &Consigne) -> Result<u32> {
        let mut enfant = std::process::Command::new(&self.executable)
            .env("SESSION_ID", &consigne.session.0)
            .env("SIGNALING_URL", &self.signaling_url)
            .env("LOCAL_IP", &self.local_ip)
            .env("FENETRE_HWND", format!("{:#x}", consigne.fenetre))
            .env("SORTIE_DXGI", &consigne.nom_sortie)
            .env("AUDIO", if consigne.audio { "1" } else { "0" })
            // Surtout PAS `SUPERVISEUR` : un enfant qui hériterait de la
            // variable se prendrait pour un superviseur et lancerait ses
            // propres enfants, indéfiniment.
            .env_remove("SUPERVISEUR")
            // **Correctif I6 de la revue finale.** `SUPERVISEUR` n'était pas
            // la seule variable héritable qui change le SENS d'un enfant.
            // `scripts/run-agent.sh:32` pose `$env:TEST_FILE` dès que la
            // variable est définie dans l'environnement d'appel : un
            // superviseur lancé ainsi ferait que CHAQUE enfant diffuse le
            // fichier de test et ne capture rien (`Config::test_file`, lu par
            // `demarrage`), sans le moindre avertissement.
            //
            // `WINDOW_TITLE` par la même règle : la consigne impose la fenêtre
            // par `FENETRE_HWND`, et `demarrage::source` ne retombe sur la
            // recherche par titre que si celui-là manque. Inoffensive tant que
            // `FENETRE_HWND` est posé — ce que fait la ligne ci-dessus — mais
            // la laisser entretiendrait l'idée qu'un enfant peut chercher sa
            // fenêtre par titre, ce qui est faux par construction.
            //
            // Les modes diagnostic (`CAPTURE_TEST`, `AUDIO_PROBE`,
            // `MULTIFENETRE_*`…) ne sont volontairement PAS retirés : ils sont
            // aiguillés par `diagnostics::aiguiller()`, qui court AVANT la
            // branche superviseur de `main` — un superviseur qui en porterait
            // un ne serait jamais devenu superviseur, et n'aurait donc jamais
            // lancé d'enfant. `BITRATE`, `ENCODER_FPS` et `SOURCE_TRACE`
            // restent hérités à dessein : ce sont des réglages, pas des
            // changements de mode.
            .env_remove("TEST_FILE")
            .env_remove("WINDOW_TITLE")
            // **I7 de la revue finale de branche du sous-bloc D4**, par la
            // même règle de symétrie que le bloc I6 ci-dessus : `CAPTEUR` est
            // une variable héritable qui change le SENS d'un processus — un
            // enfant qui la porterait deviendrait un second capteur, ne
            // diffuserait rien, et se disputerait le tube nommé avec le vrai.
            //
            // Le cas est INATTEIGNABLE aujourd'hui : `main.rs` prend la
            // branche capteur AVANT la branche superviseur, donc un
            // superviseur portant `CAPTEUR` ne serait jamais devenu
            // superviseur et n'aurait jamais lancé d'enfant. On la retire
            // quand même, exactement comme `lancer_capteur` retire
            // `SUPERVISEUR` par ce même raisonnement : ce qui protège l'enfant
            // ne doit pas dépendre de l'ordre de deux `if` dans un autre
            // fichier.
            .env_remove("CAPTEUR")
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

        self.enfants()
            .insert(pid, Enfant { processus: enfant, etat_illisible_signale: false });
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
        match enfant.processus.try_wait() {
            Ok(None) => {
                enfant.etat_illisible_signale = false;
                true
            }
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
                //
                // Signalé UNE fois par enfant, et pas à chaque tour de boucle :
                // voir `Enfant::etat_illisible_signale`.
                if !enfant.etat_illisible_signale {
                    enfant.etat_illisible_signale = true;
                    tracing::warn!(
                        pid, %erreur,
                        "état de l'enfant illisible, tenu pour vivant \
                         (signalé une seule fois tant que l'état reste illisible)"
                    );
                }
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
        let issue = enfant.processus.kill();
        let _ = enfant.processus.wait();
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
