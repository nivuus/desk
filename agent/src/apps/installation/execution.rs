//! Lancer l'installeur, l'attendre sans le tuer, et rapporter ce qui s'est
//! passé.
//!
//! 🔴 CE MODULE EST LE SEUL DE `apps::installation` À PORTER UN `cfg`, ET IL
//! N'A, SUR L'HÔTE, AUCUNE ÉPREUVE POSSIBLE hors
//! `cargo check --target x86_64-pc-windows-gnu`. C'est pour cela que tout ce
//! qui pouvait en sortir en est sorti : le verdict, la fenêtre de comptage, les
//! chemins, les extensions, la cadence et l'analyse des en-têtes HTTP sont six
//! modules PURS, et c'est là qu'est la couverture. Sa seule autre épreuve est
//! la recette sur VM.
//!
//! 🔴 `CreateProcessW`, ET NON `ShellExecuteExW` — L'INVERSE DE `apps::lancement`.
//! Pour une application, un double-clic est ce qu'on veut : `ShellExecuteEx`
//! honore le verbe, le répertoire de travail et le `nShow`. Pour un installeur,
//! la décision s'inverse, et voici pourquoi.
//!
//! `ShellExecuteExW` **DÉCLENCHE l'élévation** quand le manifeste de la cible
//! la demande : la boîte de dialogue s'ouvre — sur le bureau sécurisé, que
//! Desktop Duplication ne capture pas — et l'appel **ATTEND**. L'utilisateur
//! voit un écran figé et le produit ne sait rien dire. `CreateProcessW`, lui,
//! ne s'élève **jamais** : il échoue avec `ERROR_ELEVATION_REQUIRED` (740), ce
//! qui transforme un écran figé en **refus typé que le hub peut afficher**.
//!
//! ⚠️ **CETTE DERNIÈRE PHRASE EST UNE LECTURE DE LA DOCUMENTATION WINDOWS, PAS
//! UNE MESURE DE CE DÉPÔT**, et c'est la prémisse de tout le remède. La porte
//! du sous-bloc (son observation (e)) doit la confirmer ou la réfuter sur la
//! VM. **À la date où ce fichier est écrit, elle n'a pas été jouée** : la VM
//! était tenue par un chantier concurrent. Si elle est réfutée — si l'appel
//! réussit, ou échoue autrement —, le refus typé ci-dessous doit être
//! reconstruit sur un autre indice, et **la branche `elevation-requise` sera
//! simplement inatteignable** plutôt que fausse. Le dire évite qu'on bâtisse
//! une famille de refus sur une phrase que personne n'a éprouvée — c'est
//! exactement ce que le libellé de `MF_E_UNSUPPORTED_D3D_TYPE` a coûté à ce
//! dépôt le 31 juillet 2026.
//!
//! ⚠️ **CE QUE LE REMÈDE N'ATTRAPE PAS** : un installeur qui s'élève LUI-MÊME
//! en cours de route — il démarre sans privilège puis appelle
//! `ShellExecute … runas`. Celui-là provoquera la boîte de dialogue que la
//! porte décrit, et **rien ne l'en empêche** : il est borné par
//! `EXPIRATION_EXECUTION`, et rien d'autre.

use std::path::Path;

use windows::core::PWSTR;
use windows::Win32::Foundation::{
    CloseHandle, ERROR_ELEVATION_REQUIRED, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows::Win32::System::JobObjects::{
    IsProcessInJob, JobObjectExtendedLimitInformation, QueryInformationJobObject,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows::Win32::System::Threading::{
    CreateProcessW, GetCurrentProcess, GetExitCodeProcess, WaitForSingleObject,
    CREATE_NO_WINDOW, PROCESS_INFORMATION, STARTUPINFOW,
};

use super::depot::Extension;
use super::journal::queue_octets;
use super::verdict::Motif;

/// Au-delà, l'agent CESSE D'ATTENDRE — **il ne tue pas**.
///
/// 🔴 TUER UN INSTALLEUR AU MILIEU EST PIRE QUE DE CESSER DE L'ATTENDRE : il
/// laisserait la machine à moitié installée, registre écrit à demi, sans que
/// rien ne sache dans quel état. L'issue devient `issue-inconnue`, qui dit la
/// vérité : on ne sait pas.
///
/// ⚠️ **NON CALIBRÉE**, elle rejoint la liste que ce dépôt tient depuis
/// `BPP_MIN`.
pub const EXPIRATION_EXECUTION_MS: u64 = 2 * 60 * 60 * 1000;

/// Entre deux scrutations de la sortie du processus.
const PAS_DE_SCRUTATION_MS: u64 = 500;

/// Ce qu'une exécution a produit.
pub struct Sortie {
    /// `None` = **le code n'a pas pu être recueilli** — expiration, ou échec de
    /// lecture. Ce n'est pas un code, et une sentinelle `-1` les confondrait.
    pub code: Option<i32>,
    /// La queue du journal, bornée.
    pub journal: String,
    pub journal_tronque: bool,
    /// `true` si l'agent a cessé d'attendre sans que le processus soit sorti.
    pub expire: bool,
}

/// 🔴 LE GARDE DE JOB, ET C'EST LUI QUI REND LE CRITÈRE ⑦ DÉCIDABLE.
///
/// La spécification (D8) interdit que l'installeur soit assigné au job object
/// du superviseur : un redémarrage d'agent le tuerait au milieu d'une écriture
/// de registre et laisserait la machine à moitié installée.
///
/// ❌ **CE QUI SUIT ÉTAIT UNE LECTURE, ET LA MESURE L'A RÉFUTÉE.** Le plan la
/// donnait pour telle — son M5 s'intitule « LECTURE, PAS MESURE » —, et la
/// sonde de la tâche 2 existait pour la convertir. Elle l'a convertie, et
/// contre elle.
///
/// La lecture disait : « **le superviseur ne s'assigne pas lui-même** — relevé
/// dans `superviseur/lanceur.rs`, qui n'appelle `AssignProcessToJobObject` que
/// sur ses ENFANTS. Un processus qu'il crée n'hérite donc d'aucun job. » La
/// PRÉMISSE est exacte — `lanceur.rs` fait bien cela. **La CONCLUSION est
/// fausse**, parce qu'elle ignorait le mode de lancement : `run-agent.sh` passe
/// par le **planificateur de tâches**, qui place sa tâche dans un job. Mesuré
/// sur la VM : superviseur, capteur et pont sont **tous les trois** dans un
/// job, ainsi que le PowerShell de la sonde et l'enfant qu'il crée.
///
/// ⚠️ **On ne peut pas en sortir** : `CREATE_BREAKAWAY_FROM_JOB` est refusé
/// (`ERROR_ACCESS_DENIED`, 5) — le job ne porte pas `JOB_OBJECT_LIMIT_BREAKAWAY_OK`.
///
/// ✅ **Mais le job NE TUE PAS À LA FERMETURE**, et c'est ce qui sauve le
/// critère ⑦ : la tâche terminée et son lanceur mort, l'enfant direct survit.
/// **⑦ n'est donc ni vacueux ni perdu — il est TENU, pour une raison qu'aucune
/// lecture n'avait trouvée.**
///
/// 🔴 ET LE PONT, LUI, **EST** DANS LE JOB. `apps::brancher` est appelée avant
/// l'aiguillage `PONT` dans `main.rs` ; sous le leg n°1 de G1 — le pont
/// s'enrôlait sous le même `vm_id` que son père — l'ordre d'installation
/// pouvait lui échoir, et il aurait lancé l'installeur **depuis un processus
/// assigné au job**, donc tué à la mort du superviseur. C'est exactement ce que
/// la spec D8 interdit, et cela ne se voit qu'en croisant trois fichiers.
///
/// **D'où ce garde : l'agent REFUSE d'installer depuis un processus assigné à
/// un job.** Un refus bruyant vaut mieux qu'une installation qu'un
/// redéploiement tuera au milieu.
///
/// ⚠️ **C'EST UN GARDE, PAS LE REMÈDE.** Le remède est le leg n°1 de G1, qui
/// n'appartient pas à G3 : trois décisions y sont possibles et aucune n'est
/// tranchée. G3 le nomme, s'en protège, et ne le referme pas.
/// Le job de ce processus **tue-t-il ses membres à sa fermeture** ?
///
/// 🔴 **C'EST LA QUESTION QUI DÉCIDE, et elle a remplacé « suis-je dans un
/// job ? » sur la foi d'une MESURE** — voir l'en-tête du module. Le danger que
/// la spec D8 nomme n'est pas l'appartenance à un job : c'est
/// `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, qui fait mourir l'installeur avec
/// l'agent, au milieu d'une écriture de registre.
///
/// ⚠️ **`None` POUR LA POIGNÉE INTERROGE LE JOB DU PROCESSUS COURANT** — c'est
/// la sémantique de `QueryInformationJobObject`, et c'est la seule dont nous
/// disposions : nous n'avons pas la poignée du job que le planificateur de
/// tâches a créé, et nous n'avons aucun moyen de l'obtenir.
///
/// ⚠️ **HORS DE TOUT JOB, L'APPEL ÉCHOUE**, et c'est le cas nominal d'un agent
/// lancé à la main. On répond alors `false` : pas de job, pas de job qui tue.
pub fn job_tue_a_la_fermeture() -> bool {
    let mut infos = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    let taille = std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32;
    let ok = unsafe {
        QueryInformationJobObject(
            None,
            JobObjectExtendedLimitInformation,
            (&mut infos as *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            taille,
            None,
        )
    };
    match ok {
        Ok(()) => infos
            .BasicLimitInformation
            .LimitFlags
            .contains(JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE),
        Err(erreur) => {
            // ⚠️ UN ÉCHEC N'EST PAS UN OUI. Hors de tout job, l'appel échoue, et
            // c'est exactement l'état où il n'y a rien à craindre. Refuser
            // l'installation parce qu'on n'a pas su poser la question serait la
            // panne que ce garde vient précisément de cesser d'être.
            tracing::debug!(%erreur, "QueryInformationJobObject a échoué : aucun job, ou job non interrogeable");
            false
        }
    }
}

pub fn dans_un_job() -> bool {
    // ⚠️ `BOOL` VIT DANS `windows::core`, PAS DANS `Win32::Foundation` — écart
    // d'API de windows-rs 0.62, que `agent/src/window.rs` documente déjà. Le
    // crate est verrouillé à **0.62.2** dans `Cargo.lock` : lire une signature
    // d'une autre version enverrait chercher une erreur là où il n'y en a pas.
    let mut dedans = windows::core::BOOL(0);
    // ⚠️ `None` POUR LE JOB : la question est « dans UN job », pas « dans CE
    // job ». Nous n'avons pas la poignée du job du superviseur, et nous n'en
    // avons pas besoin.
    //
    // ❌ **CETTE FONCTION NE DÉCIDE PLUS DE RIEN, et la phrase qui la
    // justifiait — « n'importe quel job suffit à faire mourir l'installeur » —
    // EST RÉFUTÉE PAR LA MESURE** (voir l'en-tête du module). Elle reste parce
    // qu'elle est journalisée à chaque installation : savoir qu'on est dans un
    // job, sans en mourir, est précisément le fait que personne n'attendait.
    let ok = unsafe { IsProcessInJob(GetCurrentProcess(), None, &mut dedans) };
    match ok {
        Ok(()) => dedans.as_bool(),
        Err(erreur) => {
            // ⚠️ UN ÉCHEC DE LA QUESTION N'EST PAS UNE RÉPONSE. On journalise
            // et on répond `false` : refuser toute installation parce qu'on
            // n'a pas su poser la question serait une panne pire que le risque.
            tracing::warn!(%erreur, "IsProcessInJob a échoué : on suppose hors job");
            false
        }
    }
}

/// Lance l'installeur et l'attend, sans jamais le tuer.
///
/// `maintenant_ms` est un PARAMÈTRE : c'est l'horloge de l'appelant, comme
/// partout dans ce dépôt.
pub fn executer(
    chemin: &Path,
    extension: Extension,
    repertoire: &Path,
    maintenant_ms: impl Fn() -> u64,
) -> Result<Sortie, Motif> {
    // 🔴 LE GARDE PASSE AVANT TOUT, ET LES DEUX BOOLÉENS SONT JOURNALISÉS À
    // CHAQUE INSTALLATION — que le refus ait lieu ou non. Ce sont ces lignes,
    // et elles seules, qui rendent le critère ⑦ décidable.
    //
    // 🔴 LE GARDE PORTE SUR `job_tue_a_la_fermeture`, PAS SUR L'APPARTENANCE, ET
    // C'EST UNE MESURE QUI L'A CORRIGÉ. Il testait d'abord `dans_un_job()`, sur
    // la prémisse — écrite dans ce fichier — que « n'importe quel job suffit à
    // faire mourir l'installeur ». **La sonde de la tâche 2 l'a réfutée**, sur
    // la VM, une exécution :
    //
    //   - les TROIS processus de l'agent (superviseur, capteur, pont) sont
    //     dans un job — ce n'est pas `lanceur.rs` qui les y met, c'est le
    //     PLANIFICATEUR DE TÂCHES, par lequel `run-agent.sh` les lance ;
    //   - `CREATE_BREAKAWAY_FROM_JOB` y est REFUSÉ (`ERROR_ACCESS_DENIED`, 5) :
    //     on ne peut pas en sortir ;
    //   - et pourtant, la tâche une fois terminée — lanceur RÉELLEMENT mort,
    //     vérifié par l'absence de sa ligne de contrôle —, **l'enfant direct
    //     SURVIT**. Le job ne tue pas à la fermeture.
    //
    // Le garde d'appartenance aurait donc refusé **toute** installation dans le
    // mode de lancement normal du produit, pour un danger qui ne se matérialise
    // pas. Un refus qui ne peut jamais être levé n'est pas une protection :
    // c'est une panne. Le garde interroge désormais la propriété qui TUE, qui
    // est exactement celle que la spec D8 nomme.
    let dedans = dans_un_job();
    let tue = job_tue_a_la_fermeture();
    tracing::info!(
        dans_un_job = dedans,
        job_tue_a_la_fermeture = tue,
        "installation : le processus qui lance est-il dans un job, et ce job tue-t-il ?"
    );
    if tue {
        tracing::error!(
            "installation refusée : ce processus est dans un job qui TUE À LA FERMETURE, \
             l'installeur y mourrait avec lui"
        );
        return Err(Motif::JobObject);
    }

    let journal_chemin = repertoire.join("installeur.log");
    // ⚠️ LES DEUX FLUX VONT DANS LE MÊME FICHIER, du répertoire de
    // l'installation : il part avec lui au balayage d'âge, et il est là où
    // quelqu'un ira le chercher.
    let fichier = std::fs::File::create(&journal_chemin).map_err(|erreur| {
        tracing::error!(%erreur, chemin = %journal_chemin.display(), "journal d'installeur non créé");
        Motif::Disque
    })?;

    let mut ligne = ligne_de_commande(chemin, extension);
    let mut ligne_utf16: Vec<u16> = ligne.encode_utf16().chain(std::iter::once(0)).collect();
    ligne.clear();
    let repertoire_utf16: Vec<u16> = repertoire
        .as_os_str()
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let poignee = handle_de(&fichier);
    let depart = STARTUPINFOW {
        cb: u32::try_from(std::mem::size_of::<STARTUPINFOW>()).unwrap_or(0),
        dwFlags: windows::Win32::System::Threading::STARTF_USESTDHANDLES,
        hStdOutput: poignee,
        hStdError: poignee,
        ..Default::default()
    };
    let mut infos = PROCESS_INFORMATION::default();

    // 🔴 AUCUN DRAPEAU DE JOB, ET CE N'EST PLUS FAUTE D'EN AVOIR BESOIN : c'est
    // parce qu'il n'en existe pas d'utilisable. `CREATE_BREAKAWAY_FROM_JOB` a
    // été MESURÉ refusé sur ce chemin (`ERROR_ACCESS_DENIED`, 5) — le job du
    // planificateur de tâches ne porte pas `JOB_OBJECT_LIMIT_BREAKAWAY_OK`.
    // L'enfant hérite donc du job, et c'est sans conséquence : ce job ne tue
    // pas à la fermeture, mesuré lui aussi.
    //
    // ⚠️ Une rédaction antérieure disait ici que « le garde ci-dessus a déjà
    // établi que ce processus n'est dans aucun job ». C'était faux des deux
    // côtés : il est dans un job, et le garde ne teste plus cela.
    let resultat = unsafe {
        CreateProcessW(
            None,
            Some(PWSTR(ligne_utf16.as_mut_ptr())),
            None,
            None,
            true,
            CREATE_NO_WINDOW,
            None,
            windows::core::PCWSTR(repertoire_utf16.as_ptr()),
            &depart,
            &mut infos,
        )
    };

    if let Err(erreur) = resultat {
        // 🔴 `ERROR_ELEVATION_REQUIRED` (740) DEVIENT UN REFUS TYPÉ, et c'est
        // ce qui rend G3 livrable même si la porte est défavorable : un message
        // que le hub peut afficher, au lieu d'une attente que personne ne
        // comprend.
        //
        // ⚠️ LA PRÉMISSE EST UNE LECTURE DE DOCUMENTATION, PAS UNE MESURE DE CE
        // DÉPÔT — voir l'en-tête du module. Si la porte la réfute, cette
        // branche devient INATTEIGNABLE plutôt que fausse, et le refus typé
        // devra être reconstruit sur un autre indice.
        let code = erreur.code().0 as u32 & 0xFFFF;
        if code == ERROR_ELEVATION_REQUIRED.0 {
            tracing::error!(
                chemin = %chemin.display(),
                "installation refusée : l'installeur exige une élévation, que \
                 cet agent ne peut pas obtenir sans que l'utilisateur voie un \
                 écran figé"
            );
            return Err(Motif::ElevationRequise);
        }
        tracing::error!(%erreur, chemin = %chemin.display(), "CreateProcessW a échoué");
        return Err(Motif::LancementImpossible);
    }

    // ⚠️ LE FIL EST RELÂCHÉ TOUT DE SUITE : on n'en fait rien, et le garder
    // fuirait une poignée par installation.
    let _ = unsafe { CloseHandle(infos.hThread) };

    let debut = maintenant_ms();
    let mut expire = false;
    let code = loop {
        let attente = unsafe { WaitForSingleObject(infos.hProcess, 0) };
        if attente == WAIT_OBJECT_0 {
            let mut brut = 0u32;
            match unsafe { GetExitCodeProcess(infos.hProcess, &mut brut) } {
                // ⚠️ LE CODE EST RAPPORTÉ, JAMAIS INTERPRÉTÉ : `msiexec` rend
                // 3010 pour un succès qui demande un redémarrage, et beaucoup
                // d'installeurs rendent 0 après une annulation.
                Ok(()) => break Some(brut as i32),
                Err(erreur) => {
                    tracing::warn!(%erreur, "code de sortie illisible");
                    break None;
                }
            }
        }
        if attente != WAIT_TIMEOUT {
            tracing::warn!(?attente, "attente du processus en erreur");
            break None;
        }
        if maintenant_ms().saturating_sub(debut) >= EXPIRATION_EXECUTION_MS {
            // 🔴 ON CESSE D'ATTENDRE, ON NE TUE PAS.
            tracing::warn!(
                chemin = %chemin.display(),
                "installation expirée : l'agent CESSE D'ATTENDRE, il ne tue pas — \
                 tuer un installeur au milieu laisserait la machine à moitié installée"
            );
            expire = true;
            break None;
        }
        std::thread::sleep(std::time::Duration::from_millis(PAS_DE_SCRUTATION_MS));
    };
    let _ = unsafe { CloseHandle(infos.hProcess) };
    drop(fichier);

    // ⚠️ LA BORNE VIT DANS UN MODULE PUR, ÉCRITE UNE SEULE FOIS. Elle l'était
    // deux fois, et la seconde PANIQUAIT sur une frontière de caractère UTF-8.
    let (journal, journal_tronque) = match std::fs::read(&journal_chemin) {
        Ok(octets) => queue_octets(&octets),
        Err(_) => (String::new(), false),
    };
    Ok(Sortie {
        code,
        journal,
        journal_tronque,
        expire,
    })
}

/// ⚠️ **AUCUN MODE SILENCIEUX N'EST IMPOSÉ** (spec D8) : `.exe` est exécuté tel
/// quel, `.msi` passe par `msiexec /i`. Imposer `/qn` déciderait à la place de
/// l'utilisateur, et beaucoup d'installeurs refusent une installation
/// silencieuse qu'ils n'ont pas prévue.
fn ligne_de_commande(chemin: &Path, extension: Extension) -> String {
    let chemin = chemin.display().to_string();
    match extension {
        Extension::Exe => format!("\"{chemin}\""),
        Extension::Msi => format!("msiexec.exe /i \"{chemin}\""),
    }
}

fn handle_de(fichier: &std::fs::File) -> HANDLE {
    use std::os::windows::io::AsRawHandle;
    HANDLE(fichier.as_raw_handle() as _)
}
