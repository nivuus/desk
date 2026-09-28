//! The write thread: it reads the local file, splits it, pushes the frames,
//! waits for the `Fait`, keeps the journal, and announces the dues.
//!
//! # 🔵 Why this module is PURE, although it reads a file "from ProjFS"
//!
//! After `FILE_HANDLE_CLOSED_FILE_MODIFIED`, the file is **complete** in the
//! root: ProjFS only calls `GetFileData` on a **placeholder**. Reading it is
//! therefore an **ordinary** `std::fs::File::open`, portable, testable on Linux
//! with a real temporary directory.
//!
//! ⚠️ **It is an INFERENCE from ProjFS's model, not a measurement.** If it is
//! false, the reading re-enters our own callbacks. **It would not cause
//! a deadlock** — the commands thus created are completed by the
//! **bridge thread**, a distinct thread —, but the bridge would reread its own bytes
//! through the browser, which would be **visible in the log**: `Lire`s
//! on a path being written. Criterion ④ of the acceptance run exists to
//! decide.
//!
//! # 🔴 THE THREAD IS DEDICATED, AND NEVER THE BRIDGE'S
//!
//! `pont/service.rs` already writes it for the hydration survey: "a `read_dir`
//! on the root would go through ProjFS, hence would trigger our own enumeration
//! callbacks, which register a command that **this thread** must complete:
//! **it would wait for itself**". **The same sentence holds here, and it is the
//! raison d'être of this thread.**
//!
//! # The order of the sequence is NOT negotiable
//!
//! 1. the event arrives;
//! 2. **the journal is written AND FLUSHED (`sync_all`) BEFORE the first frame** —
//!    an entry pushed before being journalled is an entry that an abrupt
//!    stop loses;
//! 3. `TYPE_DUES` is announced;
//! 4. the local file is read and split;
//! 5. **one chunk in flight at a time** — *(these lines added "flow control
//!    through `bufferedAmount` / `SEUIL_TAMPON` is a deliverable of F3, and
//!    implementing it halfway here would be worse". **F3 has arrived, and it
//!    changes NOTHING here.**)*
//!
//!    ⚠️ **F3's window is that of READING, not writing, and the
//!    distinction is not a detail**: in reading, it is the BROWSER that
//!    emits the large messages, and the bridge's window serves not to leave it
//!    idle between two chunks. In writing, it is the BRIDGE that emits them —
//!    requesting several chunks in advance would make no sense, and pushing
//!    several would flood the SCTP queue, which F1 already decided to avoid. The
//!    `bufferedAmount` back-pressure, for its part, lives on the browser side
//!    (`client/src/fichiers/flux.ts`) and therefore does not cover this direction;
//! 6. on the `Fait` of the **last** chunk: `journal.retirer`, **then**
//!    `TYPE_DUES` announced again;
//! 7. on an `Echec` or an expiry: **the entry STAYS in the journal**, a
//!    `warn!` names the path and the code.

mod disque;
mod mutations;
mod reprise;
// What the thread emits: frames to the browser, journal lines,
// registrations in the shared table.
mod sorties;

use std::collections::VecDeque;
use std::sync::mpsc::Receiver;
use std::time::Instant;

use super::{Evenement, File};
use crate::pont::decoupe::{decouper, Morceau};
use crate::pont::journal::Journal;
use crate::pont::mutation::FileMutations;
use crate::pont::table::{Attendue, DELAI_ECRIRE};
use crate::pont::transport::VersNavigateur;
use proto::fichiers::{entetes, CodeEchec};

/// The correlation carried by an **announcement**.
///
/// ⚠️ **It identifies NOTHING**: an announcement waits for no response, and the
/// browser does not use it. It is only there for the transport's log,
/// which traces `correlation` on each emission.
///
/// ⚠️ **It is NOT reserved in [`crate::pont::table::Table`]**: making it step over it
/// would cost a special case in the distribution of correlations for a
/// gain in log readability. A collision would require 2^32 registrations
/// in a single run of the bridge.
const CORRELATION_ANNONCE: u32 = u32::MAX;

/// Beyond this size, a due write is logged at `warn!` and
/// **named** to the shell page.
///
/// 🔴 **IT IS NOT A REFUSAL CEILING, and the distinction is fundamental.** The
/// only place where a size refusal would be **visible to the application** is
/// `PRE_CONVERT_TO_FULL` — but there only the size **from BEFORE**
/// the write is known, which does not bound the size after. A ceiling applied to
/// write-back, for its part, would be **invisible**: the handle has been closed for
/// a long time. Spec §3.5.2 prescribed `TAILLE_MAX_FICHIER` with
/// `ERROR_DISK_FULL`; **that error code would reach no one.**
///
/// ⚠️ **NOT CALIBRATED.**
pub const TAILLE_ECRITURE_SIGNALEE: u64 = 64 * 1024 * 1024;

mod contrat;

pub use contrat::{Config, Ordre};

/// The thread's loop. Returns when the order channel closes.
pub fn tourner(config: Config, ordres: Receiver<Ordre>) {
    let mut fil = Fil::demarrer(config);
    while let Ok(ordre) = ordres.recv() {
        fil.traiter(ordre);
    }
    tracing::info!("fil d'ecriture du pont arrete");
}

// ⚠️ `pub(super)` BECAUSE THE FIELD CARRYING IT IS, and not the reverse.
// The extraction of `fil/mutations.rs` made `Fil::en_cours` visible to the parent
// module without hoisting its TYPE with it: `rustc` says so through
// `private_interfaces` — **a warning of a family other than
// `dead_code`**, and this repository checks its warnings by their NATURE at
// each closing. An extraction moves visibility as much as code.
pub(super) struct EnCours {
    chemin: String,
    restants: VecDeque<Morceau>,
    correlation: u32,
    dernier_envoye: bool,
    octets: u64,
    debut: Instant,
}

pub(super) struct Fil {
    pub(super) config: Config,
    pub(super) journal: Journal,
    pub(super) file: File,
    pub(super) en_cours: Option<EnCours>,
    /// The mutations in flight and waiting (F3).
    ///
    /// ⚠️ **DISTINCT from the write queue, and staying so is the point.** A
    /// mutation carries no byte, is not registered in the dues journal, and
    /// **does not coalesce**: `a`→`b` then `b`→`c` are two gestures whose
    /// order is the meaning.
    pub(super) mutations: FileMutations,
    /// The correlation of the mutation in flight, if there is one.
    pub(super) mutation_en_vol: Option<u32>,
    /// **F5** — the thread has due writes and **refuses to push them**.
    ///
    /// 🔴 **THIS FLAG EMPTIES NOTHING, AND THAT IS ITS WHOLE POINT.** Holding back is
    /// neither pushing nor throwing away: pushing would write the files of one session into
    /// the folder of another (spec §6.4 case 2), throwing away would lose the data. **We
    /// do neither one nor the other: we NAME it**, by carrying the state up to the
    /// browser through the `retenues` field of the `Dues` announcement.
    pub(super) retenues: bool,
}

impl Fil {
    fn demarrer(config: Config) -> Self {
        if !config.armee {
            tracing::warn!(
                "poussee d'ecriture DESARMEE (PONT_ECRITURE=0) : bras de banc, jamais une \
                 configuration livree"
            );
        }
        let contenu = std::fs::read_to_string(&config.chemin_journal).unwrap_or_default();
        let (journal, ignorees) = Journal::relire(&contenu);
        if ignorees > 0 {
            // A partial line is the only damage an abrupt stop can
            // cause to an append-only file. Counting it makes it visible;
            // keeping quiet about it would suggest an intact journal.
            tracing::warn!(
                ignorees,
                "lignes illisibles jetees au rechargement du journal"
            );
        }
        let fil = Self {
            config,
            journal,
            file: File::nouvelle(),
            en_cours: None,
            mutations: FileMutations::nouvelle(),
            mutation_en_vol: None,
            retenues: false,
        };
        // 🔴 **F5 — `reprendre()` IS NO LONGER CALLED HERE, AND THAT IS THE REMEDY.**
        //
        // *This line was `fil.reprendre();`.* The thread starts with the BRIDGE,
        // that is **before** any browser is there: F2
        // measured the replay's push **0.8 s BEFORE** the mount announcement, and
        // the `correlation=0` expiry **+30.2 s** later. Thirty seconds
        // during which the indicator that exists to denounce the loss
        // was **SILENT**.
        //
        // Resumption now waits for [`Ordre::Bonjour`], which alone says that a
        // browser is there **and on which directory**. ⚠️ **The case "no
        // `Bonjour` arrives" has NO fallback that would push**: a fallback
        // would reopen exactly the danger of §6.4 case 2, the one for which
        // `Bonjour` exists. It has a `warn!`, and nothing else.
        fil
    }

    fn traiter(&mut self, ordre: Ordre) {
        match ordre {
            Ordre::Bonjour { racine, forcer } => self.bonjour(&racine, forcer),
            Ordre::Survenu(evenement) if evenement.est_mutation() => {
                // 🔴 **A MUTATION IS NOT A WRITE, AND CONFUSING THEM
                // WOULD DESTROY.** Without this arm, `commencer` would fall onto the
                // CONTENT path: `disque::taille_de` would return 0 on a
                // source that no longer exists — renamed, or erased —, `decouper`
                // would return zero chunks, and the fallback empty chunk
                // **WOULD TRUNCATE THE LOCAL FILE TO ZERO** or **WOULD RECREATE
                // EMPTY** what the user has just erased.
                //
                // ⚠️ **It is the silent catch-all arm this repository paid for
                // FIVE times on `capteur/pont_media.rs`** (D5 `Sommeil`, D6
                // `Part`, D7 `Audio`, D8 `PleinEcran`, clipboard P1), in
                // a worse form: there the message was lost, here it would have
                // been applied to the wrong verb.
                //
                // ⚠️ **A mutation therefore goes NEITHER through the journal of
                // due writes, NOR through the content queue**: it carries
                // no byte, and registering it would make the shell page's counter rise
                // for a gesture that has nothing to transfer.
                self.mutation(evenement);
            }
            Ordre::Survenu(evenement) => {
                let chemin = evenement.chemin().to_string();
                let octets = disque::taille_de(&self.config.racine, &chemin);
                // STEP 2: the journal BEFORE the first frame.
                let ligne = self.journal.inscrire(&chemin, octets);
                self.ecrire_journal(&ligne);
                if octets > TAILLE_ECRITURE_SIGNALEE {
                    tracing::warn!(
                        chemin, octets, seuil = TAILLE_ECRITURE_SIGNALEE,
                        "ecriture due volumineuse : elle restera longtemps dans la fenetre de perte"
                    );
                }
                // STEP 3.
                self.annoncer_les_dues();
                if let Some(a_pousser) = self.file.signaler(evenement) {
                    self.commencer(a_pousser);
                }
            }
            Ordre::Fait { correlation } => self.acquitte(correlation),
            Ordre::Echec { correlation, code } => self.refuse(correlation, code),
        }
    }

    pub(super) fn commencer(&mut self, evenement: Evenement) {
        let chemin = evenement.chemin().to_string();
        if !self.config.armee {
            // The DISARMED arm: we log and announce, we NEVER
            // push. The entry therefore stays due, and the shell page's counter
            // rises without ever coming down — it is what makes it red.
            if let Some(suivant) = self.file.terminee(&chemin) {
                self.commencer(suivant);
            }
            return;
        }
        if evenement.est_repertoire() {
            // Rule 4: a directory carries NO content.
            self.pousser_creation(&chemin, true);
            return;
        }
        if matches!(evenement, Evenement::Cree { .. }) {
            self.pousser_creation(&chemin, false);
            return;
        }
        let octets = disque::taille_de(&self.config.racine, &chemin);
        let mut morceaux: VecDeque<Morceau> =
            decouper(0, octets, proto::fichiers::TAILLE_TRAME_MAX).into();
        if morceaux.is_empty() {
            // 🔴 **AN EMPTY FILE IS THE NOMINAL CASE OF A "NEW DOCUMENT"
            // SAVED STRAIGHT AWAY**, and `decouper` deliberately returns ZERO
            // chunks for a zero length. Without this special case, no
            // `dernier` would ever be emitted, the entry would NEVER leave the
            // journal, and the user would see a permanent alert for a
            // correctly transmitted file. *A counter that never comes
            // down is as wrong as a counter that never rises.*
            //
            // ⚠️ **AND IT IS AN EMPTY CHUNK, NOT A CREATION**, against the
            // letter of F2's plan ("a zero-size file produces a
            // creation and zero chunks"). A creation has no effect on a
            // local file that already exists: a file TRUNCATED TO ZERO on the
            // VM would keep its old content on the local workstation, which is
            // a silent corruption. The empty chunk, for its part, opens the stream
            // without `keepExistingData` and closes it: the local file becomes
            // empty, which is what it must be.
            morceaux.push_back(Morceau {
                position: 0,
                longueur: 0,
            });
        }
        self.en_cours = Some(EnCours {
            chemin,
            restants: morceaux,
            correlation: 0,
            dernier_envoye: false,
            octets,
            debut: Instant::now(),
        });
        self.pousser_morceau(true);
    }

    fn pousser_creation(&mut self, chemin: &str, repertoire: bool) {
        let entete = serde_json::to_string(&entetes::Creer {
            chemin: chemin.to_string(),
            repertoire,
        })
        .expect("un en-tete Creer se serialise toujours");
        let correlation = self.inscrire(Attendue::Creer {
            chemin: chemin.to_string(),
        });
        self.en_cours = Some(EnCours {
            chemin: chemin.to_string(),
            restants: VecDeque::new(),
            correlation,
            dernier_envoye: true,
            octets: 0,
            debut: Instant::now(),
        });
        self.emettre(proto::fichiers::TYPE_CREER, correlation, &entete, &[]);
        tracing::debug!(chemin, repertoire, correlation, "creation poussee");
    }

    /// Pushes the next chunk. `premier` is only true at the very first.
    fn pousser_morceau(&mut self, premier: bool) {
        let Some(en_cours) = self.en_cours.as_mut() else {
            return;
        };
        let Some(morceau) = en_cours.restants.pop_front() else {
            return;
        };
        let dernier = en_cours.restants.is_empty();
        let chemin = en_cours.chemin.clone();
        let entete = serde_json::to_string(&entetes::Ecrire {
            chemin: chemin.clone(),
            position: morceau.position,
            longueur: morceau.longueur,
            premier,
            dernier,
        })
        .expect("un en-tete Ecrire se serialise toujours");

        let octets = match disque::lire(&self.config.racine, &chemin, morceau) {
            Ok(octets) => octets,
            Err(erreur) => {
                tracing::warn!(chemin, %erreur, "lecture du fichier local echouee : ecriture due RETENUE");
                self.terminer(&chemin, false);
                return;
            }
        };
        let correlation = self.inscrire(Attendue::Ecrire {
            chemin: chemin.clone(),
            dernier,
        });
        if let Some(en_cours) = self.en_cours.as_mut() {
            en_cours.correlation = correlation;
            en_cours.dernier_envoye = dernier;
        }
        self.emettre(proto::fichiers::TYPE_ECRIRE, correlation, &entete, &octets);
        tracing::debug!(
            chemin,
            correlation,
            position = morceau.position,
            longueur = morceau.longueur,
            premier,
            dernier,
            "ecriture poussee"
        );
    }

    fn acquitte(&mut self, correlation: u32) {
        if self.mutation_en_vol == Some(correlation) {
            tracing::info!(correlation, "mutation acquittee : le poste local a suivi");
            self.terminer_mutation(true);
            return;
        }
        let Some(en_cours) = self.en_cours.as_ref() else {
            return;
        };
        if en_cours.correlation != correlation {
            // A late `Fait`, arrived after an expiry. Throwing it away is
            // the invariant of `Table::resoudre`, transposed.
            tracing::debug!(correlation, "acquittement tardif ou inconnu : jete");
            return;
        }
        if !en_cours.dernier_envoye {
            self.pousser_morceau(false);
            return;
        }
        let chemin = en_cours.chemin.clone();
        let octets = en_cours.octets;
        let duree_ms = en_cours.debut.elapsed().as_millis();
        // STEP 6: the journal, THEN the announcement.
        tracing::info!(
            chemin,
            octets,
            duree_ms,
            "ecriture acquittee : les octets sont sur le poste local"
        );
        self.terminer(&chemin, true);
    }

    fn refuse(&mut self, correlation: u32, code: CodeEchec) {
        if self.mutation_en_vol == Some(correlation) {
            // 🔴 **A FAILED MUTATION WILL NEVER BE REPLAYED**, and it is what
            // distinguishes it from a write: ProjFS sends no
            // notification again for a gesture already accomplished in the VM. The two
            // sides have DIVERGED, permanently, and the only remedy is human —
            // hence the `warn!` and the shell page's line.
            tracing::warn!(
                correlation,
                ?code,
                "MUTATION REFUSEE : le poste local n'a PAS suivi, et rien ne le rejouera"
            );
            self.terminer_mutation(false);
            return;
        }
        let Some(en_cours) = self.en_cours.as_ref() else {
            return;
        };
        if en_cours.correlation != correlation {
            return;
        }
        let chemin = en_cours.chemin.clone();
        // 🔴 **THE ENTRY STAYS IN THE JOURNAL.** Removing it would be the data
        // loss this module exists to prevent.
        tracing::warn!(
            chemin,
            ?code,
            "ecriture due retenue : le navigateur a refuse, l'entree reste au journal"
        );
        self.terminer(&chemin, false);
    }

    /// Closes the push in progress. `acquittee` decides whether the entry leaves the journal.
    fn terminer(&mut self, chemin: &str, acquittee: bool) {
        self.en_cours = None;
        if acquittee {
            let ligne = self.journal.retirer(chemin);
            self.ecrire_journal(&ligne);
        }
        self.annoncer_les_dues();
        if let Some(suivant) = self.file.terminee(chemin) {
            self.commencer(suivant);
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod tests_poussee;
