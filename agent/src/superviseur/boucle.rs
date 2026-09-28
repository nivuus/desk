//! The supervisor loop: it consumes events, advances the
//! table, and executes the effects it returns.
//!
//! No decision here — the table decides, this loop acts. It is what
//! makes the rules exercisable without Windows, and this file readable.

#![cfg(windows)]

use anyhow::Result;

use super::designation;
use super::enfants::{Consigne, Enfants};
use super::hook;
use super::lanceur::LanceurDeProcessus;
use super::placement;
use super::protocole::{DepuisLaShell, VersLaShell};
use super::reprise;
use super::table::{Effet, IdSession, Table};
use crate::capture::{enumerer_sorties_silencieux, SortieDxgi};
// `relever_topologie` rather than `enumerer_sorties` on the creation path:
// it logs the topology output by output, and it is this survey that makes
// a failing pairing diagnosable. The periodic placement check,
// for its part, uses `enumerer_sorties_silencieux` — it runs every
// second and must log nothing.
//
// **Fix I2 of the final review**: this last sentence was wrong.
// `enumerer_sorties` carries an unconditional `tracing::info!` per adapter
// without output, i.e. two lines per second indefinitely on this VM,
// written to a CIFS share. The silent variant exists for this single
// caller; any new periodic loop must use it too.
//
// **Nuance added in task 7**: the creation path now has
// a third step, the polling of `attendre_une_sortie_neuve`
// — and IT uses `enumerer_sorties_silencieux`, not `relever_topologie`,
// although it stays on the creation path. It is not a breach of the
// rule above: this step runs at 10 Hz, for up to 5 s, and
// `relever_topologie` logging one line per output at EACH call, it
// would be the same defect fix I2 corrected, replayed at a
// worse cadence. The named and logged survey is still done once before
// creation, and once more if the wait expires (see the doc
// of `attendre_une_sortie_neuve`) — never at each polling turn.
use crate::diagnostics::multifenetre::montee::{noms_attaches, relever_topologie};
use crate::moniteurs_virtuels::{config_affichage, pilote::PiloteParIoctl, Sorties};

/// Maximum number of windows served simultaneously.
///
/// **10, i.e. the driver's pool of virtual outputs** (measurement ① of
/// July 31st, 2026: refusal at the 11th creation, `ERROR_TOO_MANY_NAMES`). It
/// is no longer the ENCODER ceiling, and that is D5's change: until now
/// the two coincided at 8, since one could not open more windows than one
/// could encode. The pool (`capteur::vivier`) separates them — at most
/// `vivier::PLAFOND_EVEIL` (8) windows are awake at once, the others
/// sleep while keeping their virtual output and their session.
///
/// The two ceilings therefore no longer come from the same layer: this one from the
/// **virtual output driver**, `PLAFOND_EVEIL` from the **encoding
/// hardware**. Making them follow each other would be a mistake.
///
/// ⚠️ **Value measured on this VM, not proven to be a system bound** —
/// and the cause of the refusal at the 11th creation is not isolated (we do not even know whether the
/// pool of 10 is global to the driver or per client: Apollo pings the same one).
const CAPACITE: usize = 10;

/// Cadence of the driver watchdog's heartbeat. The driver removes the
/// outputs of a client that stops pinging; the unit of its delay is NOT
/// known (none is excluded, not even the second), hence a heartbeat
/// clearly faster than any plausible unit.
const PERIODE_PING: std::time::Duration = std::time::Duration::from_millis(500);

/// Cadence of the "is each window still on its output" check.
/// A second of delay on a move is imperceptible; on the other hand
/// this check enumerates DXGI outputs, which is not free — it must not
/// run at each loop turn.
const PERIODE_PLACEMENT: std::time::Duration = std::time::Duration::from_secs(1);

/// Maximum time left to Windows to attach a freshly created output.
///
/// **A bound, not a waiting time.** The previous version slept a flat 1500 ms,
/// and acceptance run D1 showed it was not always enough: the
/// output was not yet in the topology when we looked for it there, and the
/// window never opened. We now wait for the FACT — that a new output
/// appears — and this constant only prevents waiting
/// indefinitely.
const LIMITE_RATTACHEMENT: std::time::Duration = std::time::Duration::from_secs(5);

/// No tighter polling: each turn enumerates all DXGI outputs,
/// which is not free.
const PAS_RATTACHEMENT: std::time::Duration = std::time::Duration::from_millis(100);

pub fn tourner(
    pilote: &PiloteParIoctl,
    lanceur: &LanceurDeProcessus,
    rx_hook: std::sync::mpsc::Receiver<hook::EvenementFenetre>,
    rx_shell: std::sync::mpsc::Receiver<DepuisLaShell>,
    envoyer: impl Fn(&VersLaShell),
    // The VM's prefix, delivered by the platform (sub-block P3). Empty
    // when no enrolment took place — sessions then keep
    // exactly the name they had before P3.
    prefixe: String,
) -> Result<()> {
    // Forced HERE, and not at the first pairing: the disarming trace must
    // come out BEFORE the first window, otherwise a short acceptance run ends
    // without it. Lesson paid for by `PONT_MESURE` in sub-block F4.
    let _ = designation::armee();

    let mut sorties = Sorties::nouvelles(pilote);
    let mut enfants = Enfants::nouveaux(lanceur);
    let mut table = Table::with_prefix(CAPACITE, prefixe);

    // The capturer, before any window — `surveillance_capteur::EtatCapteur`.
    let mut etat_capteur = surveillance_capteur::EtatCapteur::start(lanceur)?;
    // The files bridge, right after — and its startup IS NOT FATAL, unlike
    // the capturer's: no `?` here, and it is not an
    // oversight. Framing §4 principle 4 requires that a failure on the files side
    // never touches the video stream; `EtatPont::start` therefore returns no
    // `Result`, and retries indefinitely from `surveiller`.
    let mut etat_pont = surveillance_pont::EtatPont::start(lanceur);
    // DXGI outputs already assigned, so that two windows with the same viewport are not
    // given the same one. The table already carries the
    // session -> output mapping; this is only the set of occupied outputs, by
    // their (stable) DXGI name, and no longer by a (positional) pair of indexes.
    let mut prises: Vec<String> = Vec::new();
    // Windows on probation: see `superviseur::sursis`.
    let mut sursis = super::sursis::Sursis::new();

    // Windows already open: the hook only reports changes.
    let mut effets = recenser_les_fenetres_existantes(&mut table);

    let mut last_ping = std::time::Instant::now();
    let mut last_placement_check = std::time::Instant::now();
    loop {
        // 1. Execute pending effects.
        let a_faire = std::mem::take(&mut effets);
        for effet in a_faire {
            match effet {
                Effet::AnnoncerOuverture { session, titre } => {
                    envoyer(&VersLaShell::FenetreOuverte {
                        session: session.0.clone(),
                        titre,
                    });
                }
                Effet::CreateOutput {
                    session,
                    titre,
                    largeur,
                    hauteur,
                } => {
                    effets.extend(create_output(
                        pilote,
                        &mut sorties,
                        &mut table,
                        &mut prises,
                        &envoyer,
                        Demande {
                            session,
                            titre,
                            largeur,
                            hauteur,
                        },
                    ));
                    // `create_output` beat the watchdog during its
                    // attach wait: do not count it again as late.
                    last_ping = std::time::Instant::now();
                }
                Effet::LancerEnfant {
                    session,
                    fenetre,
                    nom_sortie,
                    size,
                } => {
                    // The path reusing a retained output does not go
                    // through `create_output`, so the window was not
                    // placed again. A single enumeration, on this arm only.
                    let all = enumerer_sorties_silencieux().unwrap_or_default();
                    replacer_si_besoin(&table, &session, &all);
                    if let Err(error) = enfants.lancer(Consigne {
                        session: session.clone(),
                        fenetre: fenetre.0,
                        nom_sortie,
                        size,
                    }) {
                        tracing::error!(session = %session.0, %error, "child launch failed");
                        // The `Lanceur` trait's contract is atomic: `Err`
                        // means no process is running. Nothing to kill
                        // then; the output, for its part, is RETAINED by `enfant_mort`
                        // (§7.1 of D3), and released by an abandonment path.
                        effets.extend(table.enfant_mort(&session));
                    }
                }
                Effet::TuerEnfant { session } => enfants.tuer(&session),
                Effet::DetruireSortie {
                    sortie_pilote,
                    nom_sortie,
                } => {
                    // Released NOW, not at supervisor shutdown: the
                    // driver's pool is consumed at each window
                    // opening, and a dozen open-close cycles
                    // would otherwise be enough to block any new window.
                    rendre_la_sortie(&mut sorties, &mut prises, sortie_pilote, nom_sortie);
                }
                Effet::SuivreLeViewport {
                    session,
                    largeur,
                    hauteur,
                } => {
                    suivre_le_viewport(&mut table, &session, largeur, hauteur);
                }
                Effet::AnnoncerFermeture { session } => {
                    envoyer(&VersLaShell::FenetreFermee { session: session.0 });
                }
                Effet::AnnoncerRefus { titre, motif } => {
                    envoyer(&VersLaShell::Refus { titre, motif });
                }
            }
        }

        // 2. Beat the driver's watchdog.
        if last_ping.elapsed() >= PERIODE_PING {
            if let Err(error) = pilote.pinguer() {
                tracing::warn!(%error, "driver watchdog ping failed");
            }
            last_ping = std::time::Instant::now();
        }

        // 3. Window events.
        //
        // 🔴 **A WINDOW IS NO LONGER ANNOUNCED AT BIRTH: IT GOES THROUGH
        // PROBATION.** A single Steam launch served 23 windows in
        // three minutes, six of which died within 110 to 150 ms — each having opened
        // a pop-up that, for its part, outlives the Windows window. See
        // `superviseur::sursis`, which carries the measurement and the reasoning, and
        // notably WHY hardening the static criterion would have been the wrong
        // fix (25 of Steam's 26 windows are already set aside by it).
        while let Ok(evenement) = rx_hook.try_recv() {
            match evenement {
                hook::EvenementFenetre::Apparue { fenetre, titre } => {
                    sursis.deposer(fenetre, titre, std::time::Instant::now());
                }
                hook::EvenementFenetre::Disparue { fenetre } => {
                    // Removed from probation AND signalled to the table: both, since
                    // a window can disappear before its deadline (the
                    // first bites) or after having been announced (the
                    // second). `retirer` returns false in that case, and therefore does not lie
                    // about the avoided tab.
                    if sursis.retirer(fenetre) {
                        tracing::info!(
                            hwnd = format!("{:#x}", fenetre.0),
                            "window vanished during its grace period: no tab was opened"
                        );
                    }
                    effets.extend(table.fenetre_disparue(fenetre));
                }
            }
        }

        // 3 bis. Windows that have proven they last.
        //
        // ⚠️ **`murs` is NOT enough to announce**: it establishes that a window has
        // LASTED, never that it is still presentable. `merite_encore` is the
        // second half, and without it probation would only be a delay.
        for (fenetre, titre) in sursis.murs(std::time::Instant::now()) {
            if hook::merite_encore(fenetre) {
                effets.extend(table.fenetre_apparue(fenetre, titre));
            } else {
                // 🔴 THE BRANCH TAKEN, NAMED. Without this trace, a legitimate
                // window wrongly set aside by probation would be indistinguishable
                // from a window that never appeared — and the symptom
                // would be "my application does not open", without a line
                // to say so.
                tracing::info!(
                    hwnd = format!("{:#x}", fenetre.0),
                    %titre,
                    "window DISCARDED when its grace period expired: it no longer deserves a tab"
                );
            }
        }

        // 4. Messages from the shell.
        //
        // The `session` field comes from the browser and is not trustworthy: the
        // signaling relays whole control messages, a peer can
        // write whatever it wants there. It is `viewport_recu` that guards — it ignores
        // an unknown session, and a session no longer waiting for its viewport.
        while let Ok(message) = rx_shell.try_recv() {
            match message {
                DepuisLaShell::Viewport {
                    session,
                    largeur,
                    hauteur,
                } => {
                    // 🔴 **THE MOST UPSTREAM POINT, AND IT IS UNCONDITIONAL.**
                    // It distinguishes "the message NEVER arrives" (nothing here) from
                    // "it arrives and the table does nothing with it" (line here,
                    // `effets=0`). Without it, both read the same, and
                    // it is what blocked the diagnosis of the first send.
                    let session = IdSession(session);
                    let suite = table.viewport_recu(&session, largeur, hauteur);
                    tracing::info!(
                        session = %session.0,
                        demande = format!("{largeur}x{hauteur}"),
                        effets = suite.len(),
                        etat = ?table.etat(&session),
                        "viewport received from the shell page"
                    );
                    effets.extend(suite);
                }
                // 🔴 A SHELL PAGE HAS JUST JOINED THE CONTROL
                // SESSION. Everything the supervisor announced before this
                // instant is LOST — the relay drops without a trace
                // what it has no one to deliver to — and it is the defect
                // measured in production on August 30th, 2026: the agent had been running
                // for several minutes, its three windows had been
                // announced at t = 12 s then refused at t = 43 s, and
                // the user, held back by the proxy's authentication,
                // never saw anything.
                //
                // 🔴 THE ORDER OF THE TWO GESTURES IS THE FIX, NOT A
                // DETAIL:
                //   ① repeat the entries STILL pending
                //      (`reannoncer_les_attentes`), which also resets their
                //      countdown to zero — the 30 s clock restarts
                //      from the moment a shell is there, which is what the
                //      constant claims to measure;
                //   ② replay the startup enumeration, whose
                //      `fenetre_apparue` is idempotent by `HWND`: it
                //      therefore only catches up the windows ABANDONED in the
                //      meantime, which are no longer in the table.
                // Reversing the two would announce TWICE an entry still
                // pending, and the shell page would reload
                // (`window.open(url, "guac-<session>")` targets a NAMED
                // window) the window it has just opened.
                DepuisLaShell::PairPresent => {
                    tracing::info!(
                        "a shell page joined the control session: the windows are announced again"
                    );
                    effets.extend(table.reannoncer_les_attentes(std::time::Instant::now()));
                    effets.extend(recenser_les_fenetres_existantes(&mut table));
                }
            }
        }

        // 5. Children dead by themselves.
        for session in enfants.morts() {
            effets.extend(table.enfant_mort(&session));
        }

        // 5bis. The capturer, same turn as the children — `EtatCapteur::surveiller`.
        etat_capteur.surveiller(lanceur);

        // 5ter. The files bridge, same turn — `EtatPont::surveiller`. Touches
        // neither the table, nor the children, nor the outputs: a bridge
        // failure must stay without effect on video sessions.
        etat_pont.surveiller(lanceur);

        // 6. Are the windows still on their output?
        //
        // An application can move or resize itself, and a
        // window overflowing its output gives a truncated capture without
        // anything signalling it. The check is PERIODIC and not hooked to
        // `EVENT_OBJECT_LOCATIONCHANGE`: that event fires at each
        // pixel of movement, on all desktop windows, and would drown
        // the hook's channel for a need that tolerates a second of delay
        // very well.
        if last_placement_check.elapsed() >= PERIODE_PLACEMENT {
            last_placement_check = std::time::Instant::now();
            controler_le_placement(&table);
            // 7. Windows whose child is dead but which still exist
            // on the Windows side: we offer them again rather than let them
            // disappear from the shell (see `Etat::SansSession`).
            effets.extend(table.relancer_les_orphelines(std::time::Instant::now()));
        }

        if effets.is_empty() {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
}

/// Brings into the table all the already open Windows windows.
///
/// Called at TWO moments, and that is why it exists rather than
/// being copied: at supervisor startup (the hook only reports
/// CHANGES, hence nothing that existed before it), and when a
/// shell page arrives, to catch up the windows the waiting delay
/// abandoned in the meantime.
///
/// ⚠️ **It duplicates nothing**: `Table::fenetre_apparue` is idempotent
/// by `HWND` and returns an EMPTY vector for an already known window, whatever
/// its state. It is this idempotence — set for a completely different reason
/// (the overlap between the enumeration and the hook) — that makes the second
/// call free.
fn recenser_les_fenetres_existantes(table: &mut Table) -> Vec<Effet> {
    let mut effets = Vec::new();
    for (fenetre, titre) in hook::enumerer_existantes() {
        effets.extend(table.fenetre_apparue(fenetre, titre));
    }
    effets
}

/// What an output request carries. A `struct` rather than four
/// parameters: the title was added (it is what a refusal tells
/// the user) and the argument list crossed the readability threshold.
struct Demande {
    session: IdSession,
    titre: String,
    largeur: u32,
    hauteur: u32,
}

// Periodic placement check (`controler_le_placement`,
// `replacer_si_besoin`): extracted on the production side, to stay under the
// project's 500-line ceiling — task 7 of sub-block D3 made this file
// cross that ceiling. Extract rather than compress, same
// reason and same scheme as `superviseur/table/attribution.rs`.
mod placement_periodique;
use placement_periodique::{controler_le_placement, replacer_si_besoin, suivre_le_viewport};

// Launching and supervising the capturer (task 7 of sub-block D4): extracted
// on the production side, for the same reason and with the same scheme as
// `placement_periodique` above. Named `surveillance_capteur` and not
// `capteur` — see this file's header (I7).
mod surveillance_capteur;

// Launching and supervising the files bridge (task 10 of sub-block F1):
// twin of the module above, extracted for the same reason and with the same scheme.
// Named `surveillance_pont` and not `pont` — `crate::pont` designates the process
// itself, and this file does `use super::*`.
mod surveillance_pont;

// Creating a virtual output and handing it back to the driver (task 1 of
// sub-block D10): extracted on the production side, for the same reason and with the same
// scheme as the two modules above, and before the addition that would
// otherwise have made it cross the ceiling.
mod creation_sortie;
use creation_sortie::{create_output, rendre_la_sortie};
