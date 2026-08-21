//! Le fil d'installation : télécharger, exécuter, fermer la fenêtre, rapporter.
//!
//! 🔴 UN FIL À PART, ET NON LA BOUCLE DE DÉCOUVERTE. Celle-ci tourne sur un
//! **vrai fil COM dédié** — l'appartement COM appartient à SON fil, et
//! `IShellLinkW` comme `ShellExecuteExW` doivent courir sur celui qui a appelé
//! `CoInitializeEx`. Une installation télécharge plusieurs centaines de
//! mégaoctets puis attend un processus des minutes durant : l'y poser figerait
//! le catalogue **pendant exactement l'installation dont on attend qu'il rende
//! compte**.
//!
//! 🔴 CE FICHIER EST LE SEUL POINT DE CONTACT ENTRE `apps` ET `installation`,
//! et il n'en a que deux : la **fenêtre de comptage**, que la boucle alimente,
//! et le drapeau « réconcilie maintenant », qu'elle observe. `apps::boucle` n'a
//! rien d'autre à connaître des installations.
//!
//! ⚠️ IL PORTE UN `cfg` PARCE QU'IL APPELLE `execution`. Tout ce qui pouvait en
//! sortir en est sorti : six modules purs, et c'est là qu'est la couverture.

use std::path::Path;
use std::sync::atomic::Ordering;

use proto::plateforme::{Phase, VersLaPlateforme};
use tokio::sync::{mpsc, watch};

use crate::plateforme::{Identite, Installation};

use super::cadence::{doit_emettre, PERIODE_PROGRESSION};
use super::depot::{self, Etat};
use super::execution;
use super::journal::queue;
use super::partage::Partage;
use super::peripherique_audio::{self, Moment};
use super::telechargement::{self, Demande};
use super::verdict::{Issue, Motif};

/// 🔴 VARIABLE DE BANC, JAMAIS UNE CONFIGURATION LIVRÉE.
///
/// `INSTALLATION_FAUTE=empreinte` altère **un octet** du fichier après écriture
/// et avant vérification, ce qui exerce la **troisième** vérification
/// d'empreinte sur le chemin réel — autrement inatteignable sans corrompre
/// quelque chose à la main pendant un transfert.
///
/// ⚠️ **CONVENTION DE VALEUR, PAS D'INTERRUPTEUR** : la valeur NOMME une faute,
/// elle ne désarme rien. C'est la figure d'`AUDIO_PERIPHERIQUE` et
/// d'`AUDIO_FAUTE_LECTURE`, et **l'inverse** de `PLEIN_ECRAN`/`AUDIO`/`APPS`,
/// où `=0` désarme. **Absente, tout est armé normalement.**
const VARIABLE_FAUTE: &str = "INSTALLATION_FAUTE";

/// Fait tourner le fil jusqu'à ce que le canal se ferme.
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
            // ⚠️ ON REFUSE PLUTÔT QUE DE RETOMBER SUR `%TEMP%` : Windows le
            // purge, y compris PENDANT une installation.
            return terminer(emettre, ordre, Issue::Refusee, Some(Motif::Disque), None, "", false);
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
        return terminer(emettre, ordre, Issue::Refusee, Some(Motif::Disque), None, "", false);
    };

    // 🔴 LA MÉMOIRE EST SUR LE DISQUE, ET C'EST LE RÉPERTOIRE LUI-MÊME. La
    // plateforme RÉÉMET à chaque enrôlement, et l'agent redémarre : une
    // déduplication en mémoire ne tiendrait que dans UN processus.
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
        return terminer(emettre, ordre, Issue::Refusee, Some(Motif::Disque), None, "", false);
    }

    // --- la fenêtre s'ouvre AVANT le lancement, pas après ---
    if let Ok(mut f) = partage.fenetres.lock() {
        f.ouvrir(&ordre.id);
    }

    // --- transfert ---
    let url = format!("{}{}", base.trim_end_matches('/'), &ordre.url);
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
                // ⚠️ LA DERNIÈRE EST TOUJOURS ÉMISE : sans cette clause, une
                // barre s'arrêterait à 97 % pour l'éternité.
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
    let _ = PERIODE_PROGRESSION;
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

    // 🔴 LA FAUTE DE BANC S'INJECTE ICI : après écriture, AVANT vérification —
    // c'est le seul endroit qui exerce la troisième vérification d'empreinte
    // sur le chemin réel. ⚠️ Le téléchargement l'a déjà vérifiée, donc la faute
    // doit passer par une seconde relecture ; c'est ce que fait `verifier`.
    if std::env::var(VARIABLE_FAUTE).as_deref() == Ok("empreinte") {
        if let Err(erreur) = alterer_un_octet(Path::new(&chemin)) {
            tracing::warn!(%erreur, "faute d'empreinte non injectée");
        } else if !verifier(Path::new(&chemin), &ordre.sha256) {
            tracing::error!("faute injectée : l'empreinte relue DIFFÈRE, installation refusée");
            let _ = std::fs::remove_file(&chemin);
            fermer(partage, &ordre.id);
            return terminer(
                emettre, ordre, Issue::Refusee, Some(Motif::Empreinte), None, "", false,
            );
        }
    }

    // --- exécution ---
    let _ = std::fs::write(repertoire.join(depot::MARQUEUR_COMMENCE), b"");
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
    // ⚠️ `spawn_blocking` : `executer` scrute et dort, elle bloquerait le
    // réacteur tokio des heures durant.
    let sortie = tokio::task::spawn_blocking(move || {
        execution::executer(Path::new(&chemin_exe), extension, &rep, maintenant_ms)
    })
    .await;
    peripherique_audio::tracer(&ordre.id, Moment::Apres);
    let _ = std::fs::write(repertoire.join(depot::MARQUEUR_TERMINE), b"");

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

    // --- réconciliation forcée, puis fermeture de la fenêtre ---
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

/// Lève le drapeau, et attend que la boucle l'ait honoré — **borné**.
///
/// ⚠️ ON ATTEND LE FAIT, JAMAIS UNE DURÉE, et l'attente est BORNÉE : une boucle
/// de découverte morte ne doit pas figer le fil d'installation, qui doit
/// pouvoir rapporter son issue de toute façon.
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
    // 🔴 CETTE BORNE PANIQUAIT. Elle faisait `&journal[journal.len() - N..]` sur
    // un `&str` sans vérifier la frontière de caractère — et le chemin était
    // ATTEIGNABLE, `from_utf8_lossy` agrandissant. Un installeur écrivant du
    // Latin-1 sur sa sortie standard aurait tué ce fil, qui serait mort SANS
    // RAPPORTER D'ISSUE : le hub aurait affiché « en cours » pour l'éternité.
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
    let mut f = std::fs::OpenOptions::new().read(true).write(true).open(chemin)?;
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
