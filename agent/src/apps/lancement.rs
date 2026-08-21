//! Lancer une application comme un double-clic le ferait.
//!
//! 🔴 CE MODULE EST `#[cfg(windows)]`, ET IL NE DÉCIDE DE RIEN. Il reçoit un
//! chemin de `.lnk`, une cible de repli et un `nShow`, et rend l'issue de ce
//! qu'il a tenté. `IssueLancement::Inconnue` n'est PAS rendue ici : seul
//! l'appelant connaît le catalogue, donc seul lui peut dire qu'une clé n'y est
//! pas.
//!
//! ⚠️ VÉRIFIÉ PAR `cargo check --target x86_64-pc-windows-gnu` SEUL, comme
//! `apps::lecture`. Aucun test d'hôte ne peut le couvrir.

use proto::plateforme::IssueLancement;
use windows::core::PCWSTR;
use windows::Win32::UI::Shell::{
    ShellExecuteExW, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW,
};

/// Lance le raccourci, et retombe sur la cible enregistrée s'il a disparu.
///
/// 🔴 AUCUNE LIGNE DE COMMANDE N'EST RECONSTRUITE, NULLE PART. Le `.lnk` est
/// passé tel quel à `ShellExecuteExW`, qui en tire lui-même la cible, les
/// arguments, le répertoire de travail et le verbe. Reconstruire, ce serait
/// réintroduire un analyseur de ligne de commande maison — c'est-à-dire
/// exactement le défaut qu'on retire à `src/lnkParser.js`, dont ce sous-bloc
/// est le remède. Le repli lui-même passe la cible comme `lpFile`, sans jamais
/// coller d'arguments derrière.
///
/// 🔴 LE PROCESSUS LANCÉ N'ENTRE DANS AUCUN JOB OBJECT, et le raisonnement du
/// superviseur NE SE TRANSPOSE PAS ICI. `agent/src/superviseur/lanceur.rs`
/// pose `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, dont l'objet est que la mort du
/// superviseur ne laisse pas N processus agent derrière lui. Une application
/// que l'UTILISATEUR vient de lancer n'est pas dans ce cas : l'y assigner la
/// tuerait avec l'agent, c'est-à-dire au premier redéploiement. `ShellExecuteEx`
/// crée son processus hors de tout job, et c'est ce qu'on veut.
///
/// ⚠️ **CETTE DERNIÈRE PHRASE N'A JAMAIS ÉTÉ MESURÉE, ET ELLE EST VRAIE POUR
/// UNE RAISON QU'ELLE NE DONNE PAS** (relevé par le sous-bloc G3, sa
/// divergence E8). En général, un processus créé par un processus assigné à un
/// job **est assigné au MÊME job** : ce n'est donc pas une propriété de
/// `ShellExecuteEx`, c'est une conséquence du fait que
/// `superviseur/lanceur.rs` **n'assigne que ses ENFANTS et jamais lui-même**.
/// La phrase cesserait d'être vraie le jour où ce lancement viendrait d'un
/// enfant — et le PONT, lui, EST dans le job.
///
/// 🔴 **G3 NE S'APPUIE PAS DESSUS : il MESURE et il REFUSE.**
/// `apps::installation::execution::dans_un_job` appelle `IsProcessInJob` avant
/// tout lancement d'installeur, journalise le booléen à chaque fois, et refuse
/// si la réponse est oui. **Rien de tel n'est fait ici**, et c'est assumé : un
/// installeur tué au milieu laisse une machine à moitié installée, une
/// application tuée ne laisse rien.
///
/// ⚠️ LES DEUX TENTATIVES SONT JOURNALISÉES, y compris celle qui réussit :
/// sans la trace du repli, `Cible` serait indiscernable de `Raccourci` dans un
/// journal, et c'est précisément la distinction que l'issue existe pour
/// rendre décidable.
pub fn lancer(chemin_lnk: &str, cible: &str, montrer: i32) -> IssueLancement {
    match executer(chemin_lnk, montrer) {
        Ok(()) => {
            tracing::info!(chemin = chemin_lnk, "raccourci lancé");
            return IssueLancement::Raccourci;
        }
        Err(erreur) => tracing::warn!(
            chemin = chemin_lnk,
            %erreur,
            "lancement du raccourci échoué, repli sur la cible enregistrée"
        ),
    }

    // ⚠️ LE REPLI N'EST PAS UNE ÉQUIVALENCE, et c'est pour cela qu'il porte une
    // issue distincte : la cible enregistrée n'emporte ni les arguments du
    // raccourci, ni son répertoire de travail. Une application lancée par ce
    // chemin-là peut donc démarrer différemment — c'est mieux que rien, ce
    // n'est pas la même chose, et l'appelant doit pouvoir le savoir.
    if cible.trim().is_empty() {
        tracing::warn!(chemin = chemin_lnk, "aucune cible de repli enregistrée");
        return IssueLancement::Echec;
    }
    match executer(cible, montrer) {
        Ok(()) => {
            tracing::warn!(chemin = chemin_lnk, cible, "lancé par la CIBLE, pas par le raccourci");
            IssueLancement::Cible
        }
        Err(erreur) => {
            tracing::error!(chemin = chemin_lnk, cible, %erreur, "lancement échoué des deux côtés");
            IssueLancement::Echec
        }
    }
}

fn executer(fichier: &str, montrer: i32) -> windows::core::Result<()> {
    let large: Vec<u16> = fichier.encode_utf16().chain(std::iter::once(0)).collect();
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        // ⚠️ `SEE_MASK_NOASYNC` EST REQUIS, et ce n'est PAS une mesure ici :
        // c'est une lecture de la documentation Windows, déclarée comme telle.
        // `ShellExecuteEx` peut rendre la main AVANT que le processus enfant
        // ne soit créé ; si le fil appelant se termine dans l'intervalle, le
        // lancement est perdu. Notre fil de réconciliation survit, mais
        // l'appel doit rester synchrone pour que l'issue rendue décrive
        // vraiment ce qui s'est passé — sans quoi `Raccourci` signifierait
        // « la demande a été acceptée », pas « l'application a démarré ».
        //
        // `SEE_MASK_FLAG_NO_UI` supprime les boîtes de dialogue d'erreur : la
        // session interactive de la VM n'a personne pour les fermer, et une
        // modale bloquerait l'appel jusqu'au prochain redémarrage.
        fMask: SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
        lpFile: PCWSTR(large.as_ptr()),
        // Le `nShow` du raccourci est rejoué tel quel, pour qu'un lancement
        // par l'agent et un double-clic dans l'Explorateur donnent la même
        // fenêtre.
        nShow: montrer,
        ..Default::default()
    };
    // SÉCURITÉ : appel FFI. `info` vit jusqu'à la fin de la fonction, et
    // `large` aussi — le pointeur de `lpFile` ne peut donc pas pendre pendant
    // l'appel, qui est synchrone par `SEE_MASK_NOASYNC`.
    unsafe { ShellExecuteExW(&mut info) }
}
