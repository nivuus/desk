//! Tests of the write thread, **on a REAL temporary directory**.
//!
//! 🔵 **They run on the Linux host, and that is the whole point of the module.**
//! After `FILE_HANDLE_CLOSED_FILE_MODIFIED`, the file is complete in the
//! root: reading it is an ordinary `File::open`. The thread is therefore exercised here
//! for real — not simulated — without a single line of ProjFS coming into play.
//!
//! ⚠️ **NO NEW DEPENDENCY**: `std::env::temp_dir()` and a unique name,
//! rather than `tempfile`. F2 made it a rule to add no
//! dependency, and `agent/Cargo.toml` has no `dev-dependencies` section.
//!
//! 🔵 **The tests drive [`Fil`] DIRECTLY, not [`super::tourner`].** The
//! public loop blocks on a `Receiver`; exercising it would require a thread and a
//! `sleep`, hence a test whose verdict depends on timing. Here each `traiter`
//! is a deterministic step — the property `pont::table` gave itself by
//! taking time as a parameter.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use super::*;
use crate::pont::table::Table;

static COMPTEUR: AtomicU32 = AtomicU32::new(0);

/// A sandbox: a root, a journal path, and a captured channel.
pub(super) struct Bac {
    pub(super) racine: PathBuf,
    journal: PathBuf,
    recu: Receiver<VersNavigateur>,
    _envoi: Sender<VersNavigateur>,
    pub(super) table: Arc<Mutex<Table>>,
}

impl Bac {
    pub(super) fn neuf() -> Self {
        let n = COMPTEUR.fetch_add(1, Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!("f2-fil-{}-{n}", std::process::id()));
        let racine = base.join("racine");
        std::fs::create_dir_all(&racine).expect("test root");
        let (envoi, recu) = std::sync::mpsc::channel();
        Self {
            racine,
            journal: base.join("ecritures.journal"),
            recu,
            _envoi: envoi.clone(),
            table: Arc::new(Mutex::new(Table::new())),
        }
    }

    pub(super) fn config(&self, armee: bool) -> Config {
        Config {
            racine: self.racine.clone(),
            chemin_journal: self.journal.clone(),
            table: Arc::clone(&self.table),
            vers_navigateur: self._envoi.clone(),
            armee,
        }
    }

    pub(super) fn poser(&self, nom: &str, octets: &[u8]) {
        std::fs::write(self.racine.join(nom), octets).expect("test file");
    }

    pub(super) fn journal_brut(&self) -> String {
        std::fs::read_to_string(&self.journal).unwrap_or_default()
    }

    /// The frames emitted since the last call, decoded as (type, correlation).
    pub(super) fn trames(&self) -> Vec<(u8, u32, Vec<u8>, Vec<u8>)> {
        let mut sorties = Vec::new();
        while let Ok(VersNavigateur::Requete { trame, .. }) = self.recu.try_recv() {
            let t = proto::files::decoder(&trame).expect("valid frame");
            sorties.push((
                t.type_message,
                t.correlation,
                t.entete.to_vec(),
                t.charge.to_vec(),
            ));
        }
        sorties
    }
}

pub(super) fn modified(chemin: &str) -> Ordre {
    Ordre::Survenu(Evenement::Modified {
        chemin: chemin.to_string(),
    })
}

/// 🔴 **THE JOURNAL IS WRITTEN BEFORE THE FIRST FRAME.**
///
/// An entry pushed before being journalled is an entry an abrupt
/// stop loses: the restarted bridge would not even know it existed.
#[test]
fn the_journal_is_written_before_the_first_frame() {
    let bac = Bac::neuf();
    bac.poser("note.txt", b"bonjour");
    let mut fil = Fil::start(bac.config(true));
    // No frame has been READ yet: we check the state of the DISK at the moment
    // the first frame has already gone. If the order were reversed, the
    // journal would be empty here.
    let before = bac.journal_brut();
    assert!(before.is_empty(), "nothing has arrived yet");
    fil.traiter(modified("note.txt"));
    assert!(
        bac.journal_brut().contains("note.txt"),
        "the journal must carry the entry AS SOON AS the frame has left"
    );
    let trames = bac.trames();
    // ⚠️ The FIRST frame is the dues announcement, the second the write: the
    // browser must know what is due before receiving the bytes.
    assert_eq!(trames[0].0, proto::files::TYPE_DUES);
    assert_eq!(trames[1].0, proto::files::TYPE_WRITE);
    assert_eq!(
        trames[1].3, b"bonjour",
        "the payload carries the bytes, never encoded"
    );
}

/// 🔴 **THE ENTRY LEAVES THE JOURNAL AFTER THE LAST `Fait`, AND NOT BEFORE.**
///
/// Removing it at the first `Fait` would mean that a two-chunk file whose
/// second fails would leave the journal **having lost its bytes**.
#[test]
fn the_entry_leaves_the_journal_after_the_last_fact_and_not_before() {
    let bac = Bac::neuf();
    let gros = vec![7u8; proto::files::MAX_FRAME_SIZE + 1];
    bac.poser("gros.bin", &gros);
    let mut fil = Fil::start(bac.config(true));
    fil.traiter(modified("gros.bin"));

    let premier = bac.trames();
    let (_, c1, entete, charge) = premier.last().expect("un morceau parti").clone();
    let e: entetes::Write = serde_json::from_slice(&entete).expect("Write header");
    assert!(e.premier && !e.last, "the first of TWO chunks");
    assert_eq!(charge.len(), proto::files::MAX_FRAME_SIZE);

    fil.traiter(Ordre::Fait { correlation: c1 });
    assert_eq!(
        Journal::relire(&bac.journal_brut()).0.compte(),
        1,
        "at the FIRST Fait, the entry is STILL due"
    );

    let second = bac.trames();
    let (_, c2, entete, charge) = second.last().expect("the second chunk").clone();
    let e: entetes::Write = serde_json::from_slice(&entete).expect("Write header");
    assert!(!e.premier && e.last, "the second is the LAST");
    assert_eq!(charge.len(), 1);

    fil.traiter(Ordre::Fait { correlation: c2 });
    assert_eq!(
        Journal::relire(&bac.journal_brut()).0.compte(),
        0,
        "only at the LAST Fait does the entry leave"
    );
}

/// 🔴 **A FAILURE LEAVES THE ENTRY IN THE JOURNAL.**
///
/// Removing it would be **the data loss this module exists to
/// prevent**: the application already believed it had saved.
#[test]
fn a_failure_leaves_the_entry_in_the_journal() {
    let bac = Bac::neuf();
    bac.poser("note.txt", b"a");
    let mut fil = Fil::start(bac.config(true));
    fil.traiter(modified("note.txt"));
    let (_, c, _, _) = *bac.trames().last().expect("un morceau parti");
    fil.traiter(Ordre::Echec {
        correlation: c,
        code: CodeEchec::DisquePlein,
    });
    assert_eq!(Journal::relire(&bac.journal_brut()).0.compte(), 1);
    assert_eq!(
        Journal::relire(&bac.journal_brut()).0.dues()[0].0,
        "note.txt",
        "and the entry is NAMED"
    );
}

/// 🔴 **A ZERO-SIZE FILE PRODUCES AN EMPTY CHUNK, AND THE ENTRY LEAVES.**
///
/// It is the most important check of this module. An empty file is the
/// nominal case of a "new document" saved straight away, and `decouper` deliberately
/// returns **zero** chunks for a zero length. Without the special
/// case, no `last` would ever be emitted, the entry would **never** leave
/// the journal, and the user would see a permanent alert for
/// a correctly transmitted file.
///
/// ⚠️ **And the empty chunk is NOT a creation**, against the letter of the plan:
/// a creation would have no effect on an existing local file, so
/// that a file TRUNCATED TO ZERO on the VM would keep its old content on the
/// local workstation. The test checks it on BOTH flags.
#[test]
fn a_zero_size_file_produces_an_empty_piece_and_leaves_the_journal() {
    let bac = Bac::neuf();
    bac.poser("vide.txt", b"");
    let mut fil = Fil::start(bac.config(true));
    fil.traiter(modified("vide.txt"));
    let trames = bac.trames();
    let (type_message, c, entete, charge) = trames.last().expect("one frame").clone();
    assert_eq!(
        type_message,
        proto::files::TYPE_WRITE,
        "a chunk, NOT a creation"
    );
    let e: entetes::Write = serde_json::from_slice(&entete).expect("Write header");
    assert_eq!((e.premier, e.last, e.length), (true, true, 0));
    assert!(charge.is_empty());
    fil.traiter(Ordre::Fait { correlation: c });
    assert_eq!(
        Journal::relire(&bac.journal_brut()).0.compte(),
        0,
        "without the special case, the entry would stay due FOREVER"
    );
}

/// 🔴 **A FILE ABSENT AT RESTART LEAVES THE JOURNAL WHILE BEING NAMED.**
///
/// The root was recreated, and the file went away with it (spec §6.4
/// case 3). Looping on retry would push indefinitely a file that
/// no longer exists.
///
/// ⚠️ **F5 MOVED THE MOMENT, NOT THE RULE.** *This test called
/// `Fil::start` and expected nothing else: resumption ran at the thread's
/// start.* It now waits for `Ordre::Bonjour` — without which the bridge
/// would push before knowing on which directory (§6.4 case 2). **The journal
/// is therefore no longer touched by `start` alone, and it is checked here before
/// the announcement**: without this half, the test would also pass on a product that
/// had kept the old moment.
#[test]
fn a_file_missing_at_restart_leaves_the_journal_naming_it() {
    let bac = Bac::neuf();
    bac.poser("survivant.txt", b"ok");
    // An earlier bridge left two dues, one of whose files has disappeared.
    let mut j = Journal::new();
    let mut brut = String::new();
    brut.push_str(&j.inscrire("disparu.txt", 42));
    brut.push_str(&j.inscrire("survivant.txt", 2));
    std::fs::write(&bac.journal, &brut).expect("journal de test");

    let mut fil = Fil::start(bac.config(true));
    // The half that makes this test able to see F5's move.
    let (before, _) = Journal::relire(&bac.journal_brut());
    assert_eq!(
        before.compte(),
        2,
        "start resumes NOTHING: it waits for Bonjour"
    );

    fil.traiter(Ordre::Bonjour {
        racine: "Documents".into(),
        forcer: false,
    });
    let (relu, _) = Journal::relire(&bac.journal_brut());
    let restants: Vec<&str> = relu.dues().iter().map(|(c, _)| c.as_str()).collect();
    assert_eq!(
        restants,
        ["survivant.txt"],
        "only the vanished one was supposed to go out"
    );
}

/// On resumption, the dues are **announced before** any push.
///
/// ⚠️ *This test was called `…_au_demarrage`, and it called `Fil::start` with
/// nothing else.* **F5 moved the moment**: resumption waits for `Bonjour`.
#[test]
fn pending_ones_are_announced_before_any_push_on_resume() {
    let bac = Bac::neuf();
    bac.poser("repris.txt", b"abc");
    let mut j = Journal::new();
    std::fs::write(&bac.journal, j.inscrire("repris.txt", 3)).expect("journal de test");

    let mut fil = Fil::start(bac.config(true));
    // 🔴 **NOTHING IS EMITTED BEFORE `Bonjour`**, and it is the half that measures the
    // remedy: F2 recorded the replay's push 0.8 s BEFORE the browser
    // announced its mount, then an expiry 30.2 s later.
    assert!(bac.trames().is_empty(), "no frame before Bonjour");

    fil.traiter(Ordre::Bonjour {
        racine: "Documents".into(),
        forcer: false,
    });
    let trames = bac.trames();
    assert_eq!(trames[0].0, proto::files::TYPE_DUES, "l'annonce d'abord");
    assert!(
        trames.iter().any(|(t, ..)| *t == proto::files::TYPE_WRITE),
        "then the resume"
    );
}

/// 🔴 **THE CASE `Bonjour` EXISTS FOR: ANOTHER directory HOLDS BACK.**
///
/// The journal is **neither emptied nor pushed**, the announcement carries `retenues: true`, and
/// **no `TYPE_WRITE` goes out**. Without this, the files of one session
/// would land in the folder of another (spec §6.4 case 2).
#[test]
fn a_different_directory_holds_and_says_so() {
    let bac = Bac::neuf();
    bac.poser("repris.txt", b"abc");
    let mut j = Journal::new();
    std::fs::write(&bac.journal, j.inscrire("repris.txt", 3)).expect("journal de test");

    let mut fil = Fil::start(bac.config(true));
    fil.traiter(Ordre::Bonjour {
        racine: "Documents".into(),
        forcer: false,
    });
    assert!(
        bac.trames()
            .iter()
            .any(|(t, ..)| *t == proto::files::TYPE_WRITE),
        "the first mount pushes: nothing can be misplaced there"
    );

    // A second bridge, on ANOTHER directory, with the same state folder.
    let before = bac.trames().len();
    let mut fil2 = Fil::start(bac.config(true));
    fil2.traiter(Ordre::Bonjour {
        racine: "Telechargements".into(),
        forcer: false,
    });

    let neuves: Vec<_> = bac.trames().into_iter().skip(before).collect();
    // ① NO write goes out.
    assert!(
        !neuves.iter().any(|(t, ..)| *t == proto::files::TYPE_WRITE),
        "a different directory must push NOTHING"
    );
    // ② The announcement goes out, and it carries `retenues: true` — otherwise the
    //    browser would see a frozen counter without knowing why.
    let (_, _, entete, _) = neuves
        .iter()
        .find(|(t, ..)| *t == proto::files::TYPE_DUES)
        .expect("a due announcement must go out");
    let dues: proto::files::entetes::Dues =
        serde_json::from_slice(entete).expect("en-tete Dues lisible");
    assert!(
        dues.retenues,
        "the announcement must SAY that the dues are held"
    );
    assert_eq!(dues.dues.len(), 1, "and carry the due it holds");
    // ③ The journal survives: neither pushed nor thrown away.
    let (relu, _) = Journal::relire(&bac.journal_brut());
    assert_eq!(
        relu.compte(),
        1,
        "the journal is NEITHER empty NOR pushed: it is NAMED"
    );
}

/// 🔴 **DISARMED, THE THREAD JOURNALS AND ANNOUNCES, BUT PUSHES NOTHING.**
///
/// It is the disarmed arm of the A/B, and **it is what makes the due
/// writes counter RED**. Ignoring the variable would make this red impossible to
/// provoke, hence criterion ④ of the acceptance run unmeasurable.
#[test]
fn disarmed_the_thread_logs_but_pushes_nothing() {
    let bac = Bac::neuf();
    bac.poser("note.txt", b"bonjour");
    let mut fil = Fil::start(bac.config(false));
    fil.traiter(modified("note.txt"));
    assert_eq!(
        Journal::relire(&bac.journal_brut()).0.compte(),
        1,
        "the entry is due — that is what the counter will show"
    );
    let types: Vec<u8> = bac.trames().iter().map(|(t, ..)| *t).collect();
    assert!(
        types.iter().all(|t| *t == proto::files::TYPE_DUES),
        "ONLY announcements; no write goes out: {types:?}"
    );
    // …and the queue advances anyway: a second path is journalled too.
    bac.poser("autre.txt", b"x");
    fil.traiter(modified("autre.txt"));
    assert_eq!(Journal::relire(&bac.journal_brut()).0.compte(), 2);
}

impl Journal {
    /// Reading shortcut for tests: the number of dues in a raw journal.
    pub(super) fn compte_du_brut(brut: &str) -> usize {
        Journal::relire(brut).0.compte()
    }
}
