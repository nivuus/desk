//! Where each window stands: detected, waiting for its viewport, waiting
//! for its output, live.
//!
//! Pure logic without side effects: the table creates nothing, kills nothing,
//! talks to no one. It returns a list of `Effet`s that `superviseur.rs`
//! executes. It is what makes it exercisable without Windows, without a driver and without
//! a browser — and it is where the rules live that, badly written, would
//! leak a virtual output or duplicate the sound.

use std::collections::HashMap;

/// Opaque identifier of a Windows window. It is an `HWND` on the Windows side,
/// but this module knows nothing of it and has no need to know more.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IdFenetre(pub u64);

/// Session identifier, as signaling and the browser URL
/// carry it. Opaque on purpose: neither the `HWND` nor the title, which both
/// change during a window's life.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdSession(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Etat {
    /// Announced to the shell page; we wait for it to say the size of its
    /// browser window.
    AttendLeViewport,
    /// The viewport is known, the virtual output is requested.
    AttendLaSortie,
    /// L'enfant tourne.
    Vivante,
    /// The child is dead, but **the Windows window is still there**. The
    /// periodic check will offer it again.
    ///
    /// ❌ **"and the output was handed back" appeared here and has been wrong since
    /// sub-block D3** — contradicted by `enfant_mort` two hundred lines
    /// below, which says in so many words "the output is RETAINED, and it is
    /// fix §7.1 of sub-block D3". A falsehood predating D10, found
    /// by its cross-cutting review because it survived in a file the
    /// branch modified. Retention is **the** point of the fix: handing back
    /// the output would recreate an output at restart, and it is creation
    /// that makes all open duplications abandon the mutex.
    ///
    /// Without this state, `enfant_mort` purely removed the entry: nothing
    /// recalled the window except a chance `SHOW` from Windows, and the shell
    /// stayed empty in front of very much alive applications (acceptance run D1 §3.3).
    SansSession,
}

/// Restarts tolerated for the same window before abandonment.
///
/// The safeguard against the runaway found in acceptance run D1: a window whose
/// child dies systematically would otherwise produce `w-5, w-6, w-7, w-8…`
/// until exhausting the driver's pool of outputs.
///
/// **This counter never goes back down to zero**, including when a restart
/// reaches `Vivante`: it measures the deaths accumulated over the whole life of the
/// window, not consecutive failures. A window that lives ten minutes then
/// dies three times in a row later is abandoned at the third — not
/// "three failures in a row" in the strict sense. A conservative choice, inherited as
/// is from task 10's brief.
///
/// **Uncorrected corollary, only to be documented**: a `HIDE`/`SHOW` cycle
/// (window minimised then restored) makes it leave then rejoin the table through
/// `fenetre_disparue`/`fenetre_apparue`, hence restarts at `relances = 0`. The
/// safeguard stays bounded for each cycle taken in isolation (never more than 3
/// restarts per cycle), hence no leak — only a reset one
/// must be aware of if one sought to bound the total number of
/// deaths of a window over its whole life.
pub const RELANCES_MAX: u32 = 3;

/// Time tolerated, waiting for a viewport, before a restarted window
/// is abandoned.
///
/// **Regression this delay fixes**: before task 10, a dead child
/// freed its place in `entrees` immediately (`enfant_mort` removed
/// the entry). Since then, a restarted window stays `AttendLeViewport` — and if
/// the shell page never answers (pop-up blocked, shell disconnected:
/// `CLAUDE.md` documents this case by name for sub-block D1's acceptance run),
/// nothing makes this entry progress any more: neither `SansSession` (it is no longer
/// so), nor `Vivante` (it will never reach it). Without this delay, its place
/// would be lost for the supervisor's lifetime.
///
/// **An upper bound and not calibrated** — same admission as `DUREE_FENETRE_REPRISE`
/// (`agent/src/capture/reprise.rs`): no measurement has established how long
/// a shell page can legitimately take to answer. Thirty seconds
/// largely cover a page reload or a network reconnection, without
/// blocking a place indefinitely — all the more since `CAPACITE` only offers
/// **four** since D3's campaign.
///
/// **Scope: ALL entries waiting for a viewport**, since sub-block
/// D3. It was limited to restarted entries, because `fenetre_apparue`
/// is pure and receives no instant; the timestamp is now set
/// lazily by `relancer_les_orphelines`, which receives one. Consequence
/// to know: a window pre-existing at startup is abandoned if the
/// shell page has not connected within this delay.
pub const DELAI_ATTENTE_VIEWPORT_MAX: std::time::Duration = std::time::Duration::from_secs(30);

// Extracted into a neighbouring file (task 9 of sub-block D10, at review):
// purely declarative, already heavily documented, and this file was at
// margin 1 before the extraction. See the header doc of `table/effets.rs`.
mod effets;
pub use effets::Effet;

#[derive(Debug)]
struct Entree {
    fenetre: IdFenetre,
    /// Kept for the sole reason that an output refusal must be told to a
    /// human (see `Effet::CreateOutput`). It is NOT an identifier: a
    /// window's title changes during its life, it is `IdSession` that
    /// designates.
    titre: String,
    etat: Etat,
    /// Identifier returned by the driver at creation, for destruction.
    sortie_pilote: Option<u32>,
    /// DXGI name (`\\.\DISPLAYn`) of the same output, for capture and
    /// placement. Stable, unlike an enumeration position.
    nom_sortie: Option<String>,
    /// ❌ **This field carried "the dimensions ACTUALLY returned by DXGI",
    /// and that is no longer true since sub-block D10** (found by the cross-cutting
    /// review: the file contradicted itself, `refresh_output_size`
    /// below and `boucle/placement_periodique.rs` both saying the
    /// opposite). It carries the **RETAINED size** — `min` axis by axis between the
    /// bounded viewport and the real DXGI size —, written by
    /// `boucle::creation_sortie::create_output` at creation and by
    /// `table::attribution::viewport_recu` at reuse. It is the size
    /// at which the window is put, and the one the capture crops in
    /// the output's duplication; it **no longer has any reason to equal** the
    /// raw DXGI size, since an output can be born larger than
    /// requested.
    ///
    /// The reasoning that justified the old semantics — the driver
    /// quantises (1280×632 requested returns 1280×720, measured in sub-block D2),
    /// so comparing a later viewport to the REQUEST would judge reusable
    /// an output that is not — **died with it**: reuse
    /// no longer compares an equality but `placement::sortie_assez_grande`.
    ///
    /// Set and cleared together with `sortie_pilote` and `nom_sortie`: the
    /// three designate the same output and are never separated.
    output_size: Option<(u32, u32)>,
    /// Number of times this window has already been restarted after its
    /// child's death. The safeguard of `relancer_les_orphelines` (`RELANCES_MAX`)
    /// relies on it to give up rather than restart endlessly.
    relances: u32,
    /// Instant from which this entry is in `AttendLeViewport` — set
    /// lazily by `relancer_les_orphelines` for an entry coming from
    /// `fenetre_apparue`, which stays pure and clockless. It is the safeguard
    /// against task 10's second capacity risk: without it, a window
    /// whose shell page never answers again would stay `AttendLeViewport`
    /// forever, neither `SansSession` nor `Vivante`, a place lost until
    /// supervisor shutdown. Cleared as soon as the viewport arrives
    /// (`viewport_recu`): past that point, the entry no longer waits for the
    /// browser.
    attente_depuis: Option<std::time::Instant>,
}

pub struct Table {
    /// Number of simultaneous windows the table allows itself. The driver's
    /// output pool is 10 (measured), but Apollo draws from the same one: the
    /// capacity is a parameter, not a constant.
    capacite: usize,
    entrees: HashMap<IdSession, Entree>,
    /// Counter of assigned sessions. Grows without ever going back: a
    /// reused identifier would pair a late browser message with the
    /// wrong window.
    compteur: u64,
    /// The VM's prefix, delivered by the platform at enrolment
    /// (sub-block P3). **Empty when no enrolment took place**, and the session
    /// name is then exactly the one from before P3.
    ///
    /// ⚠️ It is received at CONSTRUCTION and never set afterwards, on purpose:
    /// a prefix changing along the way would make two
    /// namespaces coexist in the same table, and the only way to stay
    /// honest there would be not to reuse the identifiers already emitted —
    /// that is, above all NOT to reset the counter to zero. Making
    /// the state impossible beats keeping it correct.
    prefixe: String,
}

/// Destruction effect for an output retained by an entry being removed.
///
/// **Three paths remove an entry from the table, and since §7.1 of
/// sub-block D3 all three can carry one**: `fenetre_disparue`, and the
/// two abandonments of `relancer_les_orphelines`. Before D3, `enfant_mort` had
/// always handed back the output and the case did not exist. An output forgotten here
/// would consume the driver's pool of ten until supervisor shutdown,
/// without any trace saying so.
fn rendre_la_sortie_de(entree: &Entree) -> Option<Effet> {
    entree
        .sortie_pilote
        .map(|sortie_pilote| Effet::DetruireSortie {
            sortie_pilote,
            // `nom_sortie` is always filled when `sortie_pilote` is:
            // `sortie_creee` sets the three fields together, never one without the
            // others, and `viewport_recu` clears them together. The fallback is therefore
            // not reachable — if it were, it would produce a `nom_sortie: ""`.
            nom_sortie: entree.nom_sortie.clone().unwrap_or_default(),
        })
}

impl Table {
    /// A table without prefix: sessions are called `w-1`, `w-2`, …
    /// exactly as before sub-block P3.
    ///
    /// ⚠️ **No PRODUCTION caller any more since P3** (`boucle.rs` goes
    /// through `with_prefix`), and the lint says so on the Windows build.
    /// Kept because it is the witness of the behaviour from before P3 —
    /// it is what the twenty or so tests of this module use, and it is through
    /// it that "empty prefix = today's name" stays exercised.
    #[cfg(test)]
    pub fn new(capacite: usize) -> Self {
        Self::with_prefix(capacite, String::new())
    }

    /// A table all of whose sessions carry their VM's prefix.
    pub fn with_prefix(capacite: usize, prefixe: String) -> Self {
        Self {
            capacite,
            entrees: HashMap::new(),
            compteur: 0,
            prefixe,
        }
    }

    /// The identifier of the next session, prefix included.
    ///
    /// The TWO sites assigning an identifier go through here
    /// (`fenetre_apparue` and the restart in `orphelines.rs`): a third
    /// composing by hand would give a session the prefix
    /// does not reach, and the platform's guard would refuse it.
    fn prochaine_session(&mut self) -> IdSession {
        self.compteur += 1;
        IdSession(super::protocole::composer(
            &self.prefixe,
            &format!("w-{}", self.compteur),
        ))
    }

    pub fn etat(&self, session: &IdSession) -> Option<&Etat> {
        self.entrees.get(session).map(|e| &e.etat)
    }

    /// Windows window associated with a session.
    ///
    /// The supervisor needs it for the periodic placement check:
    /// it knows the output per session, but it is the window that must be
    /// placed again.
    pub fn fenetre_de(&self, session: &IdSession) -> Option<IdFenetre> {
        self.entrees.get(session).map(|e| e.fenetre)
    }

    pub fn fenetre_apparue(&mut self, fenetre: IdFenetre, titre: String) -> Vec<Effet> {
        // Idempotence per window, and this guard comes BEFORE the capacity check
        // (see below): Windows can announce the same
        // HWND twice — the supervisor's startup enumeration and the
        // `EVENT_OBJECT_SHOW` hook overlap on a window appearing
        // during the enumeration. Without this guard, a second entry would be
        // created for the same window: two places consumed, and if the
        // second reaches `sortie_creee`, two real outputs opened at
        // the driver. Yet `fenetre_disparue` only finds ONE entry by
        // linear search on `fenetre`, and Windows only emits ONE
        // closing event per HWND: the second entry would become
        // unreachable, its output would never be destroyed, and the driver's
        // output pool would silently empty — until no
        // window could open any more, tens of minutes
        // later, with no visible link to the cause.
        //
        // If the guard came after the capacity check, a re-announcement
        // on a full table would wrongly produce an `AnnoncerRefus` for a
        // window... already open.
        if self.entrees.values().any(|e| e.fenetre == fenetre) {
            return Vec::new();
        }
        if self.entrees.len() >= self.capacite {
            return vec![Effet::AnnoncerRefus {
                titre,
                motif: "plus aucune sortie virtuelle disponible".into(),
            }];
        }
        let session = self.prochaine_session();
        self.entrees.insert(
            session.clone(),
            Entree {
                fenetre,
                titre: titre.clone(),
                etat: Etat::AttendLeViewport,
                sortie_pilote: None,
                nom_sortie: None,
                output_size: None,
                relances: 0,
                attente_depuis: None,
            },
        );
        vec![Effet::AnnoncerOuverture { session, titre }]
    }

    /// Name of a session's output, for the periodic placement
    /// check.
    pub fn nom_sortie_de(&self, session: &IdSession) -> Option<&str> {
        self.entrees
            .get(session)
            .and_then(|e| e.nom_sortie.as_deref())
    }

    /// Dimensions of the output retained by a session, if there is one.
    pub fn output_size_of(&self, session: &IdSession) -> Option<(u32, u32)> {
        self.entrees.get(session).and_then(|e| e.output_size)
    }

    /// Updates the RETAINED size of an already assigned output, from a
    /// fresh DXGI read. Writes NOTHING if the session has no retained
    /// output (yet): this method only corrects an existing
    /// record, never creates one — `sortie_creee` stays the only point that
    /// sets `nom_sortie` and `output_size` together.
    ///
    /// Former IMPORTANT 5 (review of task 9): closes the gap D8 had
    /// opened — `WindowsSource::changer_mode_de_sortie` resized a virtual
    /// output without going through this table, hence without it knowing,
    /// and `output_size` stayed frozen at the CREATION size. **That path
    /// was removed in sub-block D9**, with measurement to back it (see the finding at
    /// the head of `capteur/plein_ecran.rs`): nothing, in production, any longer
    /// resizes an output after its creation. **No caller any more since
    /// sub-block D10**: `output_size` now carries the RETAINED size
    /// (`sortie_pour_viewport` accepts an output larger than the
    /// viewport), which no longer has any reason to equal the raw DXGI size — the
    /// periodic refresh would therefore have overwritten it, and the call was
    /// removed. No safety net is wired for a future resize
    /// outside this table.
    pub fn refresh_output_size(&mut self, session: &IdSession, size: (u32, u32)) {
        if let Some(entree) = self.entrees.get_mut(session) {
            if entree.output_size.is_some() {
                entree.output_size = Some(size);
            }
        }
    }

    /// Sessions whose child is running, for the periodic placement
    /// check. Returned by value, but no longer out of borrowing necessity
    /// since sub-block D10: its only caller
    /// (`placement_periodique.rs::controler_le_placement`) now only holds
    /// a `&Table`, and mutates nothing while walking them.
    pub fn sessions_vivantes(&self) -> Vec<IdSession> {
        self.entrees
            .iter()
            .filter(|(_, e)| e.etat == Etat::Vivante)
            .map(|(s, _)| s.clone())
            .collect()
    }

    pub fn fenetre_disparue(&mut self, fenetre: IdFenetre) -> Vec<Effet> {
        let Some(session) = self
            .entrees
            .iter()
            .find(|(_, e)| e.fenetre == fenetre)
            .map(|(s, _)| s.clone())
        else {
            return Vec::new();
        };
        let entree = self.entrees.remove(&session).expect("trouvée à l'instant");
        let mut effets = vec![Effet::TuerEnfant {
            session: session.clone(),
        }];
        // Nothing to destroy if the window closed before its output
        // existed: `rendre_la_sortie_de` only returns `None` in that case, and
        // asking the driver to remove an output it never created would only
        // add one more error to the log. Same call as the two
        // abandonments of `relancer_les_orphelines`: the three paths that
        // hand back an output since fix §7.1 go through there.
        effets.extend(rendre_la_sortie_de(&entree));
        effets.push(Effet::AnnoncerFermeture { session });
        effets
    }

    pub fn enfant_mort(&mut self, session: &IdSession) -> Vec<Effet> {
        // No `TuerEnfant`: it is already dead. But its output did not
        // destroy itself — a virtual output outlives the
        // process that created it.
        //
        // The entry is NOT removed: the Windows window, for its part, is still
        // there (unless it coincides with its closing, handled elsewhere by
        // `fenetre_disparue`). It switches to `SansSession` so that the
        // periodic check finds it and offers it again via
        // `relancer_les_orphelines` — otherwise nothing recalled the
        // window any more, except a chance `SHOW` from Windows (acceptance run D1 §3.3).
        let Some(entree) = self.entrees.get_mut(session) else {
            return Vec::new();
        };
        entree.etat = Etat::SansSession;
        // **The output is RETAINED, and it is fix §7.1 of sub-block
        // D3.** It was until then handed back to the driver right here, and the restart
        // recreated one — yet it is the CREATION of an output that makes
        // all the already open DXGI duplications abandon the mutex
        // (`0x887A0026`). A single doomed window thus took the
        // reopening counter from 6 to 38 on perfectly healthy
        // sessions (acceptance run D2, step 4 of pass D).
        //
        // The counterpart is that three paths, and no longer one, must
        // hand back the output: `fenetre_disparue`, and the two abandonments of
        // `relancer_les_orphelines`. An output forgotten on one of them
        // would consume the pool of ten until supervisor shutdown.
        vec![Effet::AnnoncerFermeture {
            session: session.clone(),
        }]
    }
}

// Path assigning an output to a session (`viewport_recu`,
// `sortie_creee`): extracted on the PRODUCTION side, and not only the tests. This
// file was already brushing against the project's 500-line ceiling before the addition of the
// output reuse path of sub-block D3 (task 4) — adding it here
// would have crossed it. Extract rather than compress, same reason as the
// test modules below.
mod attribution;

// Periodic check of entries that no longer advance
// (`relancer_les_orphelines`): extracted on the PRODUCTION side, like
// `attribution.rs` above and for the same reason — this file was at 492
// lines, margin 8, when sub-block P3 needed to compose a session
// prefix in it (task 19). The extraction PRECEDES the addition, it is the order
// the plan imposes and the repository's doctrine: one does not compress a comment
// to get back under the line.
mod orphelines;

// Test module extracted into a neighbouring file: production alone
// already approaches the project's 500-line ceiling, and tests
// regularly add to it (the idempotence fix above added
// two). Extract rather than compress — compression has already been played
// elsewhere in this repository (`agent/src/encode/arret.rs`) and it is only played
// once.
#[cfg(test)]
#[path = "table/tests.rs"]
mod tests;

// Tests of task 10 (restart after a child's death), extracted into a
// second neighbouring file: adding them to `tests.rs` would have made it cross
// the project's 500-line ceiling. See the doc at the head of this file.
#[cfg(test)]
#[path = "table/tests_relance.rs"]
mod tests_relance;

#[cfg(test)]
mod tests_retention;
