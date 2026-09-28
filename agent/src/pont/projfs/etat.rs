//! The state ProjFS callbacks share with the bridge thread, and the three
//! handle wrappers crossing that boundary.
//!
//! Extracted from [`super`] **before** task 14's addition made it
//! cross the ceiling, and not after: `projfs.rs` was at 537 lines. This repository
//! paid four times for the lesson "the margin regained by an extraction is lost again
//! the next round if treated as acquired", and the two files
//! sub-block D9 handled AFTER the fact were first **compressed**,
//! a gesture `CLAUDE.md` forbids by name.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Mutex;

use windows::core::PCWSTR;

use super::{chargement, Contexte};
use crate::pont::decoupe::Morceau;
use crate::pont::enumeration::Session;
use crate::pont::errors::Error;
use crate::pont::table::{Attendue, Table};
use crate::pont::transport::VersNavigateur;

/// The entry buffer handle of an enumeration.
///
/// # Safety
///
/// `PRJ_DIR_ENTRY_BUFFER_HANDLE` is an opaque `*mut c_void`. It is
/// `Send + Sync` **because ProjFS's ASYNCHRONOUS enumeration requires it**: the
/// callback returns `ERROR_IO_PENDING` and the completion — which takes this same
/// handle back in its extended parameters — necessarily happens from another
/// thread. We never dereference it; we only hand it back to
/// `PrjFillDirEntryBuffer` and `PrjCompleteCommand`.
#[derive(Clone, Copy)]
pub struct TamponEntrees(
    pub windows::Win32::Storage::ProjectedFileSystem::PRJ_DIR_ENTRY_BUFFER_HANDLE,
);
// SAFETY: see above.
unsafe impl Send for TamponEntrees {}
unsafe impl Sync for TamponEntrees {}

/// The GUID of a data stream, for `PrjWriteFileData`.
///
/// Wrapped for the same reason as [`TamponEntrees`]: it crosses threads, and
/// a bare `windows::core::GUID` is `Send`, but naming it here makes the intention
/// readable at the use site.
#[derive(Clone, Copy)]
pub struct DataStream(pub windows::core::GUID);

/// What [`crate::pont::table`] cannot carry **because it is PURE**:
/// the ProjFS handles of a command in flight.
///
/// ⚠️ **It is deliberately two structures and not one.** Merging would bring
/// `windows` into `pont::table`, which is the module whose concurrency
/// is exercised on the host — and exercising it is its whole point.
pub enum ContexteProjFs {
    Attributs {
        /// The path **as ProjFS delivered it** (UTF-16, null-terminated).
        ///
        /// The table's logical path is normalised with `/` for the File
        /// System Access API; `PrjWritePlaceholderInfo` wants ProjFS's.
        /// Keeping it as is avoids a reconversion, hence a round trip
        /// where a case or a separator could be lost.
        chemin_projfs: Vec<u16>,
    },
    /// `QueryFileName` — "does this name exist?", and **nothing more**.
    ///
    /// ⚠️ **Distinct from `Attributs`, and it is not a subtlety.** Both
    /// ask the browser the same question (`TYPE_ATTRIBUTS`), but ProjFS
    /// does not expect the same thing back: `GetPlaceholderInfo` wants a
    /// marker written by `PrjWritePlaceholderInfo`, `QueryFileName` wants
    /// **only** `S_OK` or `ERROR_FILE_NOT_FOUND`. Writing a marker
    /// from `QueryFileName` would create a projected object for a file
    /// no one opens — and the callback is only called, precisely, to
    /// feed the negative cache.
    Existence,
    Lecture {
        flux: DataStream,
        /// ✅ **F3'S WINDOW, SHARED BETWEEN THE *N* CORRELATIONS IN FLIGHT.**
        ///
        /// *(This field was `restants: VecDeque<Morceau>`, with "only one in
        /// flight at a time in F1; flow control through `bufferedAmount` is
        /// a deliverable of F3". F3 has arrived.)*
        ///
        /// 🔴 **AN `Arc<Mutex<…>>` AND NOT A COPY, and it is the window that
        /// imposes it**: the *N* entries of `en_attente` of the same read
        /// describe **A SINGLE** progress state. Cloning the window would mean
        /// each response would see its own copy, would request the same
        /// chunks again, and the file would be written *N* times — or truncated, depending on
        /// the order.
        ///
        /// ⚠️ **The lock is taken by the BRIDGE THREAD alone**, never by a
        /// callback: the threading discipline forbids waiting on a thread the
        /// system owns. There is therefore no contention.
        fenetre: std::sync::Arc<Mutex<crate::pont::lecture::Fenetre>>,
    },
    Enumeration {
        tampon: TamponEntrees,
        expression: Option<String>,
    },
}

/// The state the callbacks share with the bridge thread.
///
/// It lives in an `Arc` of which **one copy is entrusted to ProjFS** as
/// `PrjStartVirtualizing`'s `instancecontext`, and taken back by
/// [`Virtualisation::drop`] **after** `PrjStopVirtualizing`.
pub struct Etat {
    /// The thirteen entry points. `pub`: the bridge thread calls them too.
    pub projfs: chargement::ProjFs,
    /// Set right after `PrjStartVirtualizing`.
    ///
    /// ⚠️ **It cannot be set before**: it is that very call that returns it.
    /// Yet ProjFS can call a callback **during** that call — hence the
    /// `Mutex<Option<…>>` rather than a bare field, and hence the fact that a callback
    /// must tolerate not seeing it yet.
    pub contexte: Mutex<Option<Contexte>>,
    /// The commands in flight. **PURE**, under lock.
    ///
    /// ⚠️ **An `Arc` since F2**, because the **write thread** registers its
    /// correlations there too — and it must take its own from THIS table.
    /// Two sources of correlations on a single channel would collide, and
    /// the collision would be **silent**: a response applied to the wrong
    /// command. See `Table::inscrire_sans_commande`.
    pub table: std::sync::Arc<Mutex<Table>>,
    /// The open enumeration sessions, by enumeration GUID.
    pub sessions: Mutex<HashMap<[u8; 16], Session>>,
    /// The ProjFS handles of the commands in flight, by correlation.
    pub en_attente: Mutex<HashMap<u32, ContexteProjFs>>,
    /// Where the callbacks push their requests towards the transport.
    pub sortant: Sender<VersNavigateur>,
    /// Where the notification callback pushes towards the **write thread**.
    ///
    /// 🔴 **A CHANNEL, AND NOTHING ELSE: the callback reads NO file.** It
    /// runs on a thread the system owns; opening the hydrated
    /// file there would do an I/O on that thread, which the discipline of
    /// [`super`] forbids — and the read would cross the root, hence our
    /// own callbacks.
    pub vers_ecriture: Sender<crate::pont::ecriture::fil::Ordre>,
    /// **F3** — are mutations armed? `PONT_MUTATION=0` disarms them.
    ///
    /// 🔴 **BENCH VARIABLE, never a shipped configuration.** Set once
    /// at startup, like `inscriptible`, and for the same reason: changing it
    /// along the way would mean a `PRE_` would allow what a POST would no longer
    /// push.
    pub mutations_armees: bool,
    /// Does the root accept writing? Set once at startup.
    ///
    /// ⚠️ **`false` GIVES EXACTLY F1'S BEHAVIOUR**: `PRE_CONVERT_TO_FULL`
    /// is then refused with `ERROR_WRITE_PROTECT`, and nothing is ever pushed.
    pub inscriptible: bool,
    /// Is the bridge's channel open?
    ///
    /// 🔴 **It is the ONLY moment an application can still learn that
    /// the browser has left**: refusing at `PRE_CONVERT_TO_FULL` returns
    /// `ERROR_IO_DEVICE` before the write starts. After, the handle is
    /// closed and no `HRESULT` reaches anyone any more.
    ///
    /// ⚠️ **An `AtomicBool` and not a bare field**: it is written by the bridge
    /// thread (on `CanalOuvert` / `CanalFerme`) and read by the system's
    /// callback threads.
    pub canal_ouvert: AtomicBool,
    /// 🔴 **THE COUNTER OF THE TWELVE CAUSES, AND THE ONLY PATH TO AN
    /// `HRESULT`.**
    ///
    /// It lives here, and not in a static, because it must die with the
    /// root: a process counter would outlive a restarted bridge and
    /// would make the census of the previous run be read — exactly the
    /// caveat [`Etat::tracer_hydratation`] writes about its own figures.
    ///
    /// ⚠️ **PURE, and lock-free**: it is incremented from the callback threads
    /// the SYSTEM owns, where the threading discipline forbids waiting.
    pub compteurs: crate::pont::compteurs::Compteurs,
    /// **F4** — the histogram of bridge → browser → bridge traversals.
    ///
    /// 🔴 **IT IS ALWAYS FED, and `PONT_MESURE` only arms
    /// EMISSION.** A mechanism that is armed only during its own measurement
    /// is a mechanism the product never exercises, hence one we will
    /// never see red. Always collecting costs three atomic operations on a
    /// path that already does a `HashMap::remove` under a `Mutex`, and makes
    /// F5 exercise the histogram without knowing it.
    ///
    /// ⚠️ *These lines said "F5 **and its successors**".* **F5 has no
    /// successor**: it is the last sub-block of sub-project ③. The bet
    /// holds anyway — F5 did exercise it without knowing —, but there is
    /// no one behind to extend it.
    ///
    /// ⚠️ **Here and not in a static**, like [`Etat::compteurs`] and for the
    /// same reason: it must die with the root, otherwise a restarted bridge
    /// would make the census of the previous run be read.
    pub latences: crate::pont::latence::Histogramme,
    /// What THIS process has hydrated since it started — see
    /// [`PERIODE_HYDRATATION`] and [`Etat::tracer_hydratation`].
    /// **F5** — what each directory contained, and since when.
    ///
    /// 🔴 **It is the only addition of the whole sub-project ③ that can make
    /// an already accepted behaviour WRONG**: without invalidation, a file created
    /// by F2, renamed or deleted by F3 would stop being seen. It is the defect
    /// spec §7.4 reproaches the old bridge for, whose data cache
    /// had **no TTL**. See [`crate::pont::cache`].
    pub cache: Mutex<crate::pont::cache::CacheEnumeration>,
    /// `PONT_CACHE=0`: `false` = the disarmed arm of criterion ①'s A/B.
    ///
    /// ⚠️ **Disarmed, the bridge behaves EXACTLY as before F5**: each
    /// listing pays its round trip. It is what makes criterion ① falsifiable
    /// **on the product itself**, and not through a source mutation.
    pub cache_arme: bool,
    pub octets_hydrates: AtomicU64,
    pub entrees_hydratees: AtomicU64,
}

impl Etat {
    /// The virtualisation context, if already set.
    ///
    /// ⚠️ **A callback can occur DURING `PrjStartVirtualizing`**, hence before
    /// the context is known: returning `Option` rather than assuming
    /// is what prevents a panic in a callback, that is, a process
    /// abort.
    pub fn contexte(&self) -> Option<Contexte> {
        self.contexte.lock().ok().and_then(|c| *c)
    }

    /// `PrjFileNameCompare` — **the order ProjFS imposes** on an enumeration.
    /// Neither `OsStr`'s lexicographic order, nor `Ordering::cmp`.
    pub fn comparer(&self, a: &str, b: &str) -> std::cmp::Ordering {
        let (a, b) = (utf16_nul(a), utf16_nul(b));
        // SAFETY: both strings are null-terminated and live
        // until the end of the function. Transcription of the `link!` at
        // `mod.rs:38` — `i32` return.
        let rang = unsafe { (self.projfs.comparer_noms)(PCWSTR(a.as_ptr()), PCWSTR(b.as_ptr())) };
        rang.cmp(&0)
    }

    /// `PrjFileNameMatch` — the `searchExpression` filter, **optional but
    /// provided**. Ignoring it would make a `dir /b *.txt` return everything.
    pub fn apparier(&self, nom: &str, motif: &str) -> bool {
        let (nom, motif) = (utf16_nul(nom), utf16_nul(motif));
        // SAFETY: same. Transcription of the `link!` at `mod.rs:47` — `bool`
        // return, one byte, like Win32's `BOOLEAN`.
        unsafe { (self.projfs.apparier_nom)(PCWSTR(nom.as_ptr()), PCWSTR(motif.as_ptr())) }
    }

    /// Registers a command, keeps its ProjFS context, and pushes the request.
    ///
    /// ⚠️ **The table lock is released BEFORE sending**, and it is not
    /// elegance: the threading discipline forbids holding a lock during
    /// an I/O, and a callback blocking here would freeze the application reading
    /// the file.
    ///
    /// Returns `false` if the transport channel is gone — the caller then returns
    /// an error rather than waiting for a delay.
    pub fn demander(
        &self,
        commande: i32,
        quoi: Attendue,
        echeance: std::time::Instant,
        contexte: ContexteProjFs,
        type_message: u8,
        entete: &str,
    ) -> bool {
        let Ok(mut table) = self.table.lock() else {
            return false;
        };
        let correlation = table.inscrire(commande, quoi, echeance);
        drop(table);
        if let Ok(mut attente) = self.en_attente.lock() {
            attente.insert(correlation, contexte);
        }
        let trame = proto::files::encoder(type_message, correlation, entete, &[]);
        if self
            .sortant
            .send(VersNavigateur::Requete { correlation, trame })
            .is_err()
        {
            // The transport is gone: remove what we just registered,
            // otherwise the command would wait its whole budget for nothing.
            if let Ok(mut table) = self.table.lock() {
                // ⚠️ **No latency observation here**: the transport left
                // BEFORE the request left. There was no traversal, and
                // counting one of zero duration would pull the mean down
                // at each broken channel.
                table.resoudre(correlation, std::time::Instant::now());
            }
            if let Ok(mut attente) = self.en_attente.lock() {
                attente.remove(&correlation);
            }
            return false;
        }
        true
    }

    /// The state [`crate::pont::notifications::decider`] expects.
    pub fn etat_de_notification(&self) -> crate::pont::notifications::Etat {
        crate::pont::notifications::Etat {
            inscriptible: self.inscriptible,
            canal_ouvert: self.canal_ouvert.load(Ordering::Relaxed),
            mutations_armees: self.mutations_armees,
        }
    }

    /// Requests the next chunk of a read already started.
    ///
    /// Re-registers the command **under a new correlation**: the old one was
    /// resolved by the response we just applied, and reusing it would mean
    /// a late response to that old correlation would be applied to the
    /// next chunk.
    pub fn demander_lecture(
        &self,
        commande: i32,
        chemin: &str,
        morceau: Morceau,
        flux: windows::core::GUID,
        fenetre: std::sync::Arc<Mutex<crate::pont::lecture::Fenetre>>,
    ) {
        let entete = serde_json::to_string(&proto::files::entetes::Lire {
            chemin: chemin.to_string(),
            position: morceau.position,
            length: morceau.length,
        })
        .expect("a Lire header always serializes");
        let poursuivie = self.demander(
            commande,
            Attendue::Lire {
                chemin: chemin.to_string(),
                position: morceau.position,
                length: morceau.length,
            },
            std::time::Instant::now() + crate::pont::table::DELAI_LIRE,
            ContexteProjFs::Lecture {
                flux: DataStream(flux),
                fenetre,
            },
            proto::files::TYPE_LIRE,
            &entete,
        );
        if !poursuivie {
            tracing::warn!(
                chemin,
                commande,
                "read interrupted: the bridge channel is gone"
            );
            let contexte = self.contexte();
            if let Some(Contexte(contexte)) = contexte {
                // SAFETY: valid context, no extended parameter.
                let _ = unsafe {
                    (self.projfs.completer_commande)(
                        contexte,
                        commande,
                        windows::core::HRESULT(self.compteurs.rendre(Error::CanalFerme)),
                        std::ptr::null(),
                    )
                };
            }
        }
    }

    /// The hydration survey, at the [`PERIODE_HYDRATATION`] period.
    ///
    /// ⚠️ **It COUNTS what the bridge wrote; it does not MEASURE the disk, and
    /// it is a decision, not a comfortable approximation.** Measuring the
    /// size occupied by the root would require walking it — hence
    /// crossing ProjFS, hence triggering our own enumeration
    /// callbacks, which register a command that **the bridge thread** must
    /// complete. A walk launched from that thread would wait for itself;
    /// launched from another, it would produce a real browser round trip at
    /// each period. **The instrument would destroy what it measures** — the lesson
    /// this repository paid for twice (the per-packet trace of the TURN work item,
    /// the CDP screenshot of sub-block D1).
    ///
    /// **What the figure says exactly**: the bytes and entries
    /// THIS run of the bridge has hydrated. Not what the root carries
    /// cumulatively from earlier runs.
    ///
    /// ⚠️ **THIS SENTENCE WAS HALF WRONG, AND IT IS THE DANGEROUS HALF
    /// THAT STAYS TRUE.** *It said: "the background measurement belongs to F5,
    /// with the eviction policy it will serve".* **The measurement has
    /// arrived** — F5's gate P1 surveyed the three candidates, and kept
    /// `GetDiskFreeSpaceEx` (a confounder that **cannot be lifted**: everything else on the
    /// VM counts in it) plus the sum of the lengths of hydrated files, read
    /// **without any bridge traversal**. ⛔ **THE EVICTION POLICY HAS NOT
    /// ARRIVED**, `PrjDeleteFile` still has no caller, and **③
    /// closes behind F5 without leaving it one**. *Correcting this sentence by a
    /// single word would suggest eviction has come.*
    pub fn tracer_hydratation(&self) {
        tracing::info!(
            bytes = self.octets_hydrates.load(Ordering::Relaxed),
            entries = self.entrees_hydratees.load(Ordering::Relaxed),
            "root hydrated (by THIS bridge run, not by the disk)"
        );
    }
}

/// A null-terminated UTF-16 string, for a `PCWSTR`.
fn utf16_nul(texte: &str) -> Vec<u16> {
    texte.encode_utf16().chain(std::iter::once(0)).collect()
}
