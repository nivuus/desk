//! The installation thread: download, run, close the window, report.
//!
//! 🔴 A SEPARATE THREAD, NOT THE DISCOVERY LOOP. That one runs on a
//! **real dedicated COM thread** — the COM apartment belongs to ITS thread, and
//! `IShellLinkW` as well as `ShellExecuteExW` must run on the one that called
//! `CoInitializeEx`. An installation downloads several hundred
//! megabytes then waits for a process for minutes: putting it there would freeze
//! the catalogue **for exactly the installation it is expected to report
//! on**.
//!
//! 🔴 THIS FILE IS THE ONLY POINT OF CONTACT BETWEEN `apps` AND `installation`,
//! and it has only two: the **counting window**, which the loop feeds,
//! and the "reconcile now" flag, which it observes. `apps::boucle` has
//! nothing else to know about installations.
//!
//! ⚠️ IT CARRIES A `cfg` BECAUSE IT CALLS `execution`. Everything that could leave it
//! has left: six pure modules, and that is where the coverage is.

use std::path::Path;
use std::sync::atomic::Ordering;

use proto::plateforme::{Phase, VersLaPlateforme};
use tokio::sync::{mpsc, watch};

use crate::plateforme::{Identite, Installation};

use super::cadence::doit_emettre;
use super::depot::{self, Etat};
use super::execution;
use super::journal::queue;
use super::partage::Partage;
use super::peripherique_audio::{self, Moment};
use super::telechargement::{self, Demande};
use super::verdict::{Issue, Motif};

/// 🔴 BENCH VARIABLE, NEVER A SHIPPED CONFIGURATION.
///
/// `INSTALLATION_FAUTE=empreinte` alters **one byte** of the file after writing
/// and before checking, which exercises the **third** fingerprint
/// check on the real path — otherwise unreachable without corrupting
/// something by hand during a transfer.
///
/// ⚠️ **VALUE CONVENTION, NOT A SWITCH**: the value NAMES a fault,
/// it disarms nothing. It is the shape of `AUDIO_PERIPHERIQUE` and
/// `AUDIO_FAUTE_LECTURE`, and **the opposite** of `PLEIN_ECRAN`/`AUDIO`/`APPS`,
/// where `=0` disarms. **Absent, everything is armed normally.**
const VARIABLE_FAUTE: &str = "INSTALLATION_FAUTE";

/// Runs the thread until the channel closes.
pub async fn tourner(
    mut installations: mpsc::UnboundedReceiver<Installation>,
    emettre: impl Fn(VersLaPlateforme) + Send + Sync + 'static,
    identite: watch::Receiver<Option<Identite>>,
    base: String,
    partage: Partage,
) {
    if let Ok(faute) = std::env::var(VARIABLE_FAUTE) {
        tracing::warn!(
            faute,
            "faute d'installation ARMEE (INSTALLATION_FAUTE) : banc, jamais une \
             configuration livrée"
        );
    }
    while let Some(ordre) = installations.recv().await {
        let jeton = identite
            .borrow()
            .as_ref()
            .map(|i| i.jeton.clone())
            .unwrap_or_default();
        honorer(&ordre, &emettre, &jeton, &base, &partage).await;
    }
    tracing::warn!("canal /agent fermé : fil d'installation arrêté");
}

async fn honorer(
    ordre: &Installation,
    emettre: &(impl Fn(VersLaPlateforme) + Send + Sync + 'static),
    jeton: &str,
    base: &str,
    partage: &Partage,
) {
    let debut = maintenant_ms();
    let racine = match std::env::var("ProgramData") {
        Ok(r) => r,
        Err(_) => {
            // ⚠️ WE REFUSE RATHER THAN FALLING BACK ON `%TEMP%`: Windows
            // purges it, even DURING an installation.
            return terminer(
                emettre,
                ordre,
                Issue::Refusee,
                Some(Motif::Disque),
                None,
                "",
                false,
            );
        }
    };
    let racine = Path::new(&racine)
        .join(depot::RACINE_RELATIVE)
        .display()
        .to_string();

    let (chemin, extension) = match depot::chemin(&racine, &ordre.id, &ordre.nom) {
        Ok(c) => c,
        Err(refus) => {
            tracing::error!(?refus, nom = %ordre.nom, "installation refusée au dépôt");
            let motif = match refus {
                depot::Refus::Script(_) | depot::Refus::Extension(_) => Motif::Extension,
                _ => Motif::Disque,
            };
            return terminer(emettre, ordre, Issue::Refusee, Some(motif), None, "", false);
        }
    };
    let repertoire = Path::new(&chemin).parent().map(Path::to_path_buf);
    let Some(repertoire) = repertoire else {
        return terminer(
            emettre,
            ordre,
            Issue::Refusee,
            Some(Motif::Disque),
            None,
            "",
            false,
        );
    };

    // 🔴 THE MEMORY IS ON DISK, AND IT IS THE DIRECTORY ITSELF. The
    // platform RE-SENDS at every enrolment, and the agent restarts: an
    // in-memory deduplication would only hold within ONE process.
    let commence = repertoire.join(depot::MARQUEUR_COMMENCE).exists();
    let termine = repertoire.join(depot::MARQUEUR_TERMINE).exists();
    match depot::etat(commence, termine) {
        Etat::Commence => {
            tracing::warn!(
                installation = %ordre.id,
                "installation DÉJÀ COMMENCÉE : on n'exécute PAS. Rejouer serait \
                 rejouer un installeur sur une machine à l'état inconnu"
            );
            return terminer(emettre, ordre, Issue::IssueInconnue, None, None, "", false);
        }
        Etat::Termine => {
            tracing::info!(installation = %ordre.id, "installation déjà terminée, rien à faire");
            return terminer(emettre, ordre, Issue::IssueInconnue, None, None, "", false);
        }
        Etat::Neuf => {}
    }

    if let Err(erreur) = std::fs::create_dir_all(&repertoire) {
        tracing::error!(%erreur, "répertoire d'installation non créé");
        return terminer(
            emettre,
            ordre,
            Issue::Refusee,
            Some(Motif::Disque),
            None,
            "",
            false,
        );
    }

    // --- the window opens BEFORE the launch, not after ---
    if let Ok(mut f) = partage.fenetres.lock() {
        f.ouvrir(&ordre.id);
    }

    // --- transfert ---
    let url = format!("{}{}", base.trim_end_matches('/'), ordre.url);
    let mut dernier: Option<u64> = None;
    let telecharge = {
        let emettre = &emettre;
        let id = ordre.id.clone();
        telechargement::telecharger(
            Demande {
                url: &url,
                jeton,
                destination: Path::new(&chemin),
                taille_attendue: ordre.taille,
                sha256_attendu: &ordre.sha256,
            },
            move |faits, total| {
                let ms = maintenant_ms();
                // ⚠️ THE LAST ONE IS ALWAYS EMITTED: without this clause, a
                // bar would stop at 97 % forever.
                if doit_emettre(dernier, ms, faits >= total) {
                    dernier = Some(ms);
                    emettre(VersLaPlateforme::progression(
                        id.clone(),
                        Phase::Transfert,
                        faits,
                        total,
                        ms.saturating_sub(debut),
                    ));
                }
            },
        )
        .await
    };
    if let Err(refus) = telecharge {
        tracing::error!(?refus, url, "téléchargement de l'installeur refusé");
        fermer(partage, &ordre.id);
        let motif = match refus {
            telechargement::Refus::Empreinte { .. } => Motif::Empreinte,
            telechargement::Refus::Disque(_) => Motif::DisquePlein,
            _ => Motif::LancementImpossible,
        };
        return terminer(emettre, ordre, Issue::Refusee, Some(motif), None, "", false);
    }

    // 🔴 THE BENCH FAULT IS INJECTED HERE: after writing, BEFORE checking —
    // it is the only place that exercises the third fingerprint check
    // on the real path. ⚠️ The download has already checked it, so the fault
    // must go through a second re-read; that is what `verifier` does.
    if std::env::var(VARIABLE_FAUTE).as_deref() == Ok("empreinte") {
        if let Err(erreur) = alterer_un_octet(Path::new(&chemin)) {
            tracing::warn!(%erreur, "faute d'empreinte non injectée");
        } else if !verifier(Path::new(&chemin), &ordre.sha256) {
            tracing::error!("faute injectée : l'empreinte relue DIFFÈRE, installation refusée");
            if let Err(erreur) = std::fs::remove_file(&chemin) {
                // ⚠️ KEEPING IT WOULD BE WORSE THAN NOT HAVING IT: a later
                // path could take it for a valid installer.
                tracing::warn!(%erreur, chemin, "fichier corrompu NON supprimé");
            }
            fermer(partage, &ordre.id);
            return terminer(
                emettre,
                ordre,
                Issue::Refusee,
                Some(Motif::Empreinte),
                None,
                "",
                false,
            );
        }
    }

    // --- execution ---
    // 🔴 FAILING TO WRITE THIS MARKER IS A REFUSAL, NOT A WARNING, and it was
    // a `let _ =` until the end-of-branch review. It is **the only
    // memory that survives an agent restart**: the platform re-sends
    // at every enrolment, and an in-memory deduplication only holds within ONE
    // process. Without it, an agent restarted during execution **would relaunch
    // the installer on a machine in an unknown state** — the only thing this
    // sub-block cannot get out of.
    //
    // ⚠️ AND IT IS WRITTEN BEFORE `CreateProcessW`, NEVER AFTER: between the two,
    // there is exactly the window it exists to cover.
    if let Err(erreur) = std::fs::write(repertoire.join(depot::MARQUEUR_COMMENCE), b"") {
        tracing::error!(
            %erreur,
            installation = %ordre.id,
            "marqueur .commence non écrit : on REFUSE d'exécuter. Sans lui, un \
             redémarrage de l'agent relancerait l'installeur sur une machine à \
             l'état inconnu"
        );
        fermer(partage, &ordre.id);
        return terminer(
            emettre,
            ordre,
            Issue::Refusee,
            Some(Motif::Disque),
            None,
            "",
            false,
        );
    }
    peripherique_audio::tracer(&ordre.id, Moment::Avant);
    emettre(VersLaPlateforme::progression(
        ordre.id.clone(),
        Phase::Execution,
        0,
        0,
        maintenant_ms().saturating_sub(debut),
    ));

    let chemin_exe = chemin.clone();
    let rep = repertoire.clone();
    // ⚠️ `spawn_blocking`: `executer` polls and sleeps, it would block the
    // tokio reactor for hours.
    let sortie = tokio::task::spawn_blocking(move || {
        execution::executer(Path::new(&chemin_exe), extension, &rep, maintenant_ms)
    })
    .await;
    peripherique_audio::tracer(&ordre.id, Moment::Apres);
    // ⚠️ THIS ONE, ON THE OTHER HAND, IS ONLY A WARNING, and the asymmetry is
    // deliberate: its absence makes a FINISHED installation read as `Commence`,
    // hence report `issue_inconnue` on a re-enrolment. It is **cautious in
    // the right direction** — nothing is replayed —, but it is wrong, and one must be able
    // to see it in the log rather than deduce it from a surprising outcome.
    if let Err(erreur) = std::fs::write(repertoire.join(depot::MARQUEUR_TERMINE), b"") {
        tracing::warn!(
            %erreur,
            installation = %ordre.id,
            "marqueur .termine non écrit : une réémission rapporterait \
             issue_inconnue sur une installation pourtant finie"
        );
    }

    let sortie = match sortie {
        Ok(Ok(s)) => s,
        Ok(Err(motif)) => {
            fermer(partage, &ordre.id);
            return terminer(emettre, ordre, Issue::Refusee, Some(motif), None, "", false);
        }
        Err(erreur) => {
            tracing::error!(%erreur, "fil d'exécution perdu");
            fermer(partage, &ordre.id);
            return terminer(emettre, ordre, Issue::IssueInconnue, None, None, "", false);
        }
    };

    // --- forced reconciliation, then closing the window ---
    emettre(VersLaPlateforme::progression(
        ordre.id.clone(),
        Phase::Reconciliation,
        0,
        0,
        maintenant_ms().saturating_sub(debut),
    ));
    forcer_une_reconciliation(partage).await;

    let apparues = fermer(partage, &ordre.id).unwrap_or(0);
    let issue = Issue::depuis(sortie.code, apparues, sortie.expire, None);
    tracing::info!(
        installation = %ordre.id,
        ?issue,
        code_sortie = ?sortie.code,
        apparues,
        expire = sortie.expire,
        journal_tronque = sortie.journal_tronque,
        "installation terminée"
    );
    terminer(
        emettre,
        ordre,
        issue,
        None,
        sortie.code,
        &sortie.journal,
        sortie.journal_tronque,
    );
}

/// Raises the flag, and waits for the loop to have honoured it — **bounded**.
///
/// ⚠️ WE WAIT FOR THE FACT, NEVER A DURATION, and the wait is BOUNDED: a dead
/// discovery loop must not freeze the installation thread, which must
/// be able to report its outcome anyway.
async fn forcer_une_reconciliation(partage: &Partage) {
    partage.reconciliee.store(false, Ordering::SeqCst);
    partage.reconcilier.store(true, Ordering::SeqCst);
    for _ in 0..600 {
        if partage.reconciliee.load(Ordering::SeqCst) {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    tracing::warn!(
        "aucune réconciliation forcée en 60 s : l'issue sera calculée sur ce qui \
         a déjà été compté"
    );
}

fn fermer(partage: &Partage, id: &str) -> Option<usize> {
    partage.fenetres.lock().ok().and_then(|mut f| f.fermer(id))
}

#[allow(clippy::too_many_arguments)]
fn terminer(
    emettre: &(impl Fn(VersLaPlateforme) + Send + Sync + 'static),
    ordre: &Installation,
    issue: Issue,
    motif: Option<Motif>,
    code: Option<i32>,
    journal: &str,
    tronque: bool,
) {
    // 🔴 THIS BOUND PANICKED. It did `&journal[journal.len() - N..]` on
    // a `&str` without checking the character boundary — and the path was
    // REACHABLE, `from_utf8_lossy` enlarging. An installer writing
    // Latin-1 on its standard output would have killed this thread, which would have died WITHOUT
    // REPORTING AN OUTCOME: the hub would have shown "in progress" forever.
    let (queue, deja_tronque) = queue(journal);
    let tronque = tronque || deja_tronque;
    emettre(VersLaPlateforme::termine(
        ordre.id.clone(),
        issue.vers_protocole(),
        motif.map(|m| m.mot().to_string()),
        code,
        queue,
        tronque,
    ));
}

fn maintenant_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn alterer_un_octet(chemin: &Path) -> std::io::Result<()> {
    use std::io::{Read, Seek, SeekFrom, Write};
    let mut f = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(chemin)?;
    let mut octet = [0u8; 1];
    f.read_exact(&mut octet)?;
    f.seek(SeekFrom::Start(0))?;
    f.write_all(&[octet[0] ^ 0xFF])
}

fn verifier(chemin: &Path, attendu: &str) -> bool {
    use std::io::Read;
    let Ok(mut f) = std::fs::File::open(chemin) else {
        return false;
    };
    let mut c = crate::apps::sha256::Condensateur::neuf();
    let mut tampon = vec![0u8; 64 * 1024];
    loop {
        match f.read(&mut tampon) {
            Ok(0) => break,
            Ok(n) => c.absorber(&tampon[..n]),
            Err(_) => return false,
        }
    }
    crate::apps::sha256::hex_de(c.terminer()) == attendu
}
