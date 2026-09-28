//! The file bridge: a single process per VM, which holds the ProjFS
//! virtualisation root and serves it from the local directory the
//! shell page opened.
//!
//! This file stays thin on purpose — **it assembles, it does not decide**. Same
//! split as `capteur.rs` and `superviseur.rs`: the pure logic (paths,
//! errors, splitting, table, transport, **and since F2 the journal of due
//! writes, the write queue and the thread that serves it**) is outside `cfg` and is tested
//! on the host;
//! what touches ProjFS is gated.
//!
//! **Why a separate process** (spec §3.2): ProjFS callbacks
//! run on threads **the system** owns, where a Rust panic
//! becomes a process `abort`. Housing them in the supervisor or in the
//! sensor would make the file bridge a risk for the whole capture — it is
//! the old bridge's fault, transposed. Here the worst case is the death of the bridge,
//! which the supervisor restarts, **without the video stream flinching**: it is
//! principle 4 of the scoping, and `boucle/surveillance_pont.rs` makes it
//! structural by refusing to treat a start-up failure as fatal.
//!
//! **Why the ProjFS entry points are resolved at RUNTIME** (decision D1):
//! the wrappers of the `windows` crate go through `raw-dylib`, hence through a
//! static import in the PE. Yet `agent.exe` is **a single binary for all
//! modes**: an unresolved import would not kill "the bridge", it would kill
//! capture, video and input on any VM without ProjFS. Hence
//! `LoadLibraryW` + `GetProcAddress`, in task 12 — and **nothing, in this
//! file or in its pure children, must import anything at all from
//! `Win32::Storage::ProjectedFileSystem`.**

pub mod bonjour;
pub mod cache;
pub mod chemins;
pub mod compteurs;
pub mod decoupe;
pub mod ecriture;
pub mod entetes;
pub mod enumeration;
pub mod errors;
pub mod journal;
pub mod latence;
pub mod lecture;
pub mod mutation;
pub mod notifications;
#[cfg(windows)]
pub mod projfs;
pub mod resolution;
#[cfg(windows)]
pub mod service;
pub mod table;
pub mod transport;

/// Entry point of bridge mode: loads ProjFS, mounts the root, and holds it.
///
/// ❌ **THIS COMMENT WAS FALSE, AND IT IS THE SAME BRANCH THAT REFUTED IT.**
/// It announced "the root is mounted and EMPTY, no request goes to the
/// browser": that was the state of **task 13**, and **task 14**
/// wired the three asynchronous callbacks to it. The root shows the tree of the
/// local workstation — noted in the acceptance run, 6 entries out of 6, three runs
/// (`docs/superpowers/plans/2026-08-19-pont-fichiers-f1-resultats.md`). (policy: allow-fr, real file path)
/// *A per-task review could not see it: the task that writes the sentence
/// and the one that refutes it never reread each other.*
///
/// ⚠️ **"Stops cleanly" has an exact scope**: the `Drop` of
/// [`projfs::Virtualisation`] completes the in-flight commands then calls
/// `PrjStopVirtualizing`. It runs on a NORMAL exit of this function —
/// never on a `TerminateProcess`, which is what the supervisor's job object
/// inflicts on its children. **A root can therefore survive an
/// abrupt stop of the supervisor**, and nothing in F1 unmounts it then: it is
/// the exact counterpart of the virtual outputs that survive a
/// `Stop-Process -Force` (sub-block D5), and it is not closed here.
#[cfg(windows)]
pub async fn executer(config: crate::Config) -> anyhow::Result<()> {
    use anyhow::Context;

    // Load BEFORE touching the file system and BEFORE signaling:
    // a VM without ProjFS must fail here, with a message that names the missing
    // entry point, and not after having created an empty "Mes Fichiers" folder that (policy: allow-fr, real Windows folder name)
    // nothing would ever serve.
    let projfs = projfs::chargement::charger()?;

    // The socket and the **data-only** `Rtc`: no track, no codec, no BWE.
    let (socket, mut rtc) = transport::build_data_rtc(config.local_ip)?;

    let crate::signaling::SignalingHandle {
        mut offers,
        answers,
        closed,
        retry_apres_s,
        ..
    } = crate::signaling::run_signaling(
        &crate::signaling::url_du_relais(&config.signaling_url),
        &config.session_id,
        config.jeton.as_deref(),
    )
    .await?;

    // 🔴 FIX OF THE LEGACY OF MISSING BRAKES (fix round 1,
    // critical ②) — THE HALF THAT HONOURS `retryApresS`. A volume refusal
    // (`trop-de-requetes`) closes the `offers` channel without an offer: `.recv()`
    // returns `None`, and this process is going to die on the next line. BEFORE
    // returning, we wait for the delay the relay suggested (bounded,
    // see `honour_suggested_retry`): the time this process takes to
    // die counts in the spacing that `surveillance_pont.rs::EtatPont`
    // measures since its last LAUNCH, without any channel crossing
    // the process boundary.
    let Some(offre) = offers.recv().await else {
        crate::signaling::honour_suggested_retry(&retry_apres_s).await;
        anyhow::bail!("no SDP offer for the file bridge");
    };
    let offre = str0m::change::SdpOffer::from_sdp_string(&offre)
        .map_err(|e| anyhow::anyhow!("offre SDP illisible : {e}"))?;
    let reponse = rtc
        .sdp_api()
        .accept_offer(offre)
        .map_err(|e| anyhow::anyhow!("the bridge refuses the offer: {e}"))?;
    answers
        .send(reponse.to_sdp_string())
        .await
        .context("sending the bridge's SDP answer")?;
    tracing::info!("bridge SDP answer sent");

    // ⚠️ **No TURN relay for the bridge, and it is an assumed DIVERGENCE
    // from the video session**, which allocates one before its answer
    // (`demarrage.rs`). `pont::transport` — whose signature is set by the
    // plan and delivered since task 11 — exposes no allocation path.
    // The bridge therefore only crosses what host candidates cross.
    // **Not covered by F1**, to be reopened the day the shell page and the VM do not
    // see each other directly.

    // The request channel: the callbacks push into it, the transport emits them.
    let (vers_navigateur, requetes) = std::sync::mpsc::channel();
    // The response channel: the transport pushes into it, the bridge thread reads them.
    let (vers_pont, reponses) = std::sync::mpsc::channel();
    // The WRITE thread's channel: the notification callback pushes its
    // events into it, the bridge thread relays the acknowledgements into it.
    let (vers_ecriture, ordres_ecriture) = std::sync::mpsc::channel();

    // ⚠️ **`PONT_ECRITURE=0` DISARMS, and mere PRESENCE does not activate** —
    // the convention of `SUPERVISEUR`, `CAPTEUR`, `PONT`, `AUDIO`,
    // `PLEIN_ECRAN`, `PRESSE_PAPIER` and `APPS`, and for the same reason:
    // testing `is_ok()` **would arm** the mechanism when writing `PONT_ECRITURE=0`
    // to cut it.
    //
    // 🔴 **It is a BENCH variable, never a delivered configuration.** It
    // only exists to make RED the shell page's due-writes counter:
    // disarmed, the thread logs and announces, but never
    // pushes, and the counter rises without coming back down.
    //
    // ⚠️ **DECLARED DIVERGENCE FROM F2'S PLAN, which contradicts itself.**
    // It writes "`PONT_ECRITURE=0` **disarms**" — hence absence ARMS — and
    // prescribes in the same sentence the form
    // `matches!(std::env::var(…).as_deref(), Ok(v) if v != "0")`, which is that
    // of `CAPTEUR` and `PONT` and which returns **`false` when the
    // variable is absent**. Taken literally, it would have delivered a bridge **silent by
    // default**: no write pushed without setting a bench variable.
    // The retained form is that of `PLEIN_ECRAN` (`capteur/plein_ecran.rs`),
    // which is the convention actually described.
    let ecriture_armee = std::env::var("PONT_ECRITURE").as_deref() != Ok("0");

    // ⚠️ **`PONT_MUTATION=0` DISARMS, and mere PRESENCE does not activate** —
    // the convention of `SUPERVISEUR`, `CAPTEUR`, `PONT`, `PONT_ECRITURE`,
    // `AUDIO`, `PLEIN_ECRAN`, `PRESSE_PAPIER` and `APPS`.
    //
    // 🔴 **BENCH VARIABLE, never a delivered configuration.** It exists
    // to make RED criteria ① and ② of F3's acceptance run: disarmed, the
    // `PRE_RENAME` and the `PRE_DELETE` refuse, the application sees
    // `ERROR_WRITE_PROTECT`, and **the local workstation is unchanged**. It is a red
    // of the MECHANISM — the refusal is logged and the
    // `protege-en-ecriture` counter rises —, never a vacuous red.
    //
    // ⚠️ **DISTINCT from `PONT_ECRITURE`, and must stay so**: confusing them would mean
    // that a rename acceptance run would also cut writing, hence the
    // temp+rename idiom it precisely wants to exercise.
    let mutations_armees = std::env::var("PONT_MUTATION").as_deref() != Ok("0");
    if !mutations_armees {
        tracing::warn!(
            "mutations DISARMED (PONT_MUTATION=0): rename and delete refused \
             at PRE_, nothing is pushed"
        );
    }

    // ⚠️ **`PONT_CACHE=0` DISARMS, and mere PRESENCE does not activate** — the
    // convention of `SUPERVISEUR`, `CAPTEUR`, `PONT`, `PONT_ECRITURE`,
    // `PONT_MUTATION`, `AUDIO`, `PLEIN_ECRAN`, `PRESSE_PAPIER` and `APPS`, and
    // for the same reason: testing `is_ok()` **would arm** the mechanism when
    // writing `PONT_CACHE=0` to cut it.
    //
    // 🔴 **BENCH VARIABLE, never a delivered configuration.** It exists
    // to make RED criterion ① of F5's acceptance run **on the product
    // itself**: disarmed, the bridge pays for each listing, and the file added
    // on the browser side **appears without `Rafraichir`**. It is a red of the
    // MECHANISM — present and without effect —, never a vacuous red, and it is the
    // form D10 named after having produced the other.
    let cache_arme = std::env::var("PONT_CACHE").as_deref() != Ok("0");
    if !cache_arme {
        tracing::warn!(
            "enumeration cache DISARMED (PONT_CACHE=0): bench arm, never a \
             shipped configuration"
        );
    }

    let virtualisation = projfs::Virtualisation::start(
        projfs,
        vers_navigateur.clone(),
        vers_ecriture,
        ecriture_armee,
        mutations_armees,
        cache_arme,
    )?;
    let etat = virtualisation.etat();
    tracing::info!(racine = %virtualisation.racine().display(), "file bridge root mounted");

    // Thread 4 — **the WRITE thread**, and it is DEDICATED.
    //
    // 🔴 **It can be neither the bridge thread, nor a callback thread.** It reads
    // files of the root: `pont/service.rs` already writes why the bridge
    // thread must never do so — "it would wait for itself". And a
    // callback thread belongs to the system, where any I/O freezes the application reading.
    let chemin_journal = projfs::dossier_etat()?.join("ecritures.journal");
    let racine_du_fil = virtualisation.racine().to_path_buf();
    let table_du_fil = std::sync::Arc::clone(&etat.table);
    let ecriture = std::thread::Builder::new()
        .name("pont-ecriture".into())
        .spawn(move || {
            ecriture::fil::tourner(
                ecriture::fil::Config {
                    racine: racine_du_fil,
                    chemin_journal,
                    table: table_du_fil,
                    vers_navigateur,
                    armee: ecriture_armee,
                },
                ordres_ecriture,
            )
        })
        .context("launching the bridge write thread")?;

    // Thread 2 — the transport. It owns the `Rtc` and the socket, and **knows
    // neither ProjFS nor Windows**.
    let transport = std::thread::Builder::new()
        .name("pont-transport".into())
        .spawn(move || {
            if let Err(error) = transport::tourner(rtc, socket, requetes, vers_pont) {
                tracing::error!(%error, "bridge transport stopped on error");
            }
        })
        .context("launching the bridge transport thread")?;

    // I6, as for the video session: a loss of signaling after the initial
    // exchange must be visible rather than silent. No renegotiation
    // is possible, so we observe and log — but we do observe.
    let mut closed = closed;
    tokio::spawn(async move {
        if closed.changed().await.is_ok() && *closed.borrow() {
            tracing::warn!("bridge signaling connection lost (no renegotiation)");
        }
    });

    // Thread 3 — the bridge thread. It owns the table and completes the commands.
    // `spawn_blocking`: its loop is blocking and must not occupy a
    // tokio executor.
    let etat_du_fil = std::sync::Arc::clone(&etat);
    tokio::task::spawn_blocking(move || service::tourner(etat_du_fil, reponses))
        .await
        .context("the file bridge thread panicked")?;

    // 🔴 **THE LOCAL COPY OF `Arc<Etat>` MUST BE RELEASED HERE, AND F1 DID NOT
    // DO IT.**
    //
    // BOTH `Sender`s — the transport's (`sortant`) and the write
    // thread's (`vers_ecriture`) — live **in `Etat`**. A receiver only
    // disconnects when the **last** copy of its `Sender` is
    // gone: as long as this local variable holds an `Arc<Etat>`, both
    // threads run, and the `join`s below **never return**.
    //
    // ⚠️ **F1 already carried this latency, and its comment stated it
    // the wrong way round** — "the transport stops when `Etat`, HENCE `virtualisation`,
    // is released": `virtualisation` only holds one copy out of two.
    // It had no visible consequence there, `transport::tourner` being able to
    // return for another reason; **F2 would make it blocking**, by adding
    // a second `join` on a thread which, for its part, has no other reason to exit.
    drop(etat);

    // ⚠️ **THE STOP NOW HAS FOUR THREADS TO ORDER, and the order matters.**
    //
    // 0. **The WRITE thread first**: it is the only one able to push a
    //    frame after the table has been emptied. Letting it live would make it
    //    emit on a `Sender` whose other end is gone, and its journal entry
    //    would remain without anything saying so.
    // 1. the `Drop` of `virtualisation` empties the table and completes each
    //    command HAVING a `command_id`;
    // 2. `PrjStopVirtualizing`;
    // 3. the entrusted `Arc` is taken back.
    //
    // The write thread's channel closes when `Etat` — hence
    // `virtualisation` — is released; waiting for it BEFORE would deadlock.
    // We therefore release, then join.
    drop(virtualisation);
    let _ = ecriture.join();
    let _ = transport.join();
    tracing::info!("file bridge stopped");
    Ok(())
}

#[cfg(not(windows))]
pub async fn executer(_config: crate::Config) -> anyhow::Result<()> {
    anyhow::bail!("bridge mode only exists on Windows")
}
