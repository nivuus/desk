//! `impl Lanceur for LanceurDeProcessus`: start a window agent from its
//! instruction, tell whether it is alive, kill it.
//!
//! Split out of `lanceur.rs` when `cargo fmt` pushed that file past 500 lines.
//! `#![cfg(windows)]` is inherited from `lanceur.rs`, as for `pont.rs`.

use super::*;

impl Lanceur for LanceurDeProcessus {
    fn lancer(&self, consigne: &Consigne) -> Result<u32> {
        let mut commande = std::process::Command::new(&self.executable);
        commande
            .env("SESSION_ID", &consigne.session.0)
            .env("SIGNALING_URL", &self.signaling_url)
            .env("LOCAL_IP", &self.local_ip)
            .env("FENETRE_HWND", format!("{:#x}", consigne.fenetre))
            .env("SORTIE_DXGI", &consigne.nom_sortie)
            // The RETAINED size (`Consigne::size`), not the output's
            // — see its doc. Read by `demarrage`, repeated to the capturer at
            // attach time (task 9 of sub-block D10).
            .env(
                "TAILLE_FENETRE",
                format!("{}x{}", consigne.size.0, consigne.size.1),
            )
            // Above all NOT `SUPERVISEUR`: a child inheriting the
            // variable would take itself for a supervisor and launch its
            // own children, indefinitely.
            .env_remove("SUPERVISEUR")
            // **Fix I6 of the final review.** `SUPERVISEUR` was not
            // the only inheritable variable changing a child's MEANING.
            // `scripts/run-agent.sh:32` sets `$env:TEST_FILE` as soon as the
            // variable is defined in the calling environment: a
            // supervisor launched that way would make EACH child broadcast the
            // test file and capture nothing (`Config::test_file`, read by
            // `demarrage`), without the slightest warning.
            //
            // `WINDOW_TITLE` by the same rule: the instruction imposes the window
            // through `FENETRE_HWND`, and `demarrage::source` only falls back on
            // lookup by title if that one is missing. Harmless as long as
            // `FENETRE_HWND` is set — which the line above does — but
            // leaving it would maintain the idea that a child can look up its
            // window by title, which is false by construction.
            //
            // The diagnostic modes (`CAPTURE_TEST`, `AUDIO_PROBE`,
            // `MULTIFENETRE_*`…) are deliberately NOT removed: they are
            // routed by `diagnostics::aiguiller()`, which runs BEFORE the
            // supervisor branch of `main` — a supervisor carrying
            // one would never have become a supervisor, and would therefore never have
            // launched a child. `BITRATE`, `ENCODER_FPS` and `SOURCE_TRACE`
            // stay inherited on purpose: they are settings, not
            // mode changes.
            .env_remove("TEST_FILE")
            .env_remove("WINDOW_TITLE")
            // **I7 of the branch's final review of sub-block D4**, by the
            // same symmetry rule as block I6 above: `CAPTEUR` is
            // an inheritable variable changing a process's MEANING — a
            // child carrying it would become a second capturer, would
            // broadcast nothing, and would contend for the named pipe with the real one.
            //
            // The case is UNREACHABLE today: `main.rs` takes the
            // capturer branch BEFORE the supervisor branch, so a
            // supervisor carrying `CAPTEUR` would never have become a
            // supervisor and would never have launched a child. We remove it
            // anyway, exactly as `lancer_capteur` removes
            // `SUPERVISEUR` by this same reasoning: what protects the child
            // must not depend on the order of two `if`s in another
            // file.
            .env_remove("CAPTEUR")
            // 🔴 **And `PONT` — the only one of the three omissions that would break the
            // product.** The `PONT` branch of `main.rs` is placed AFTER
            // `CAPTEUR`, but BEFORE `config.superviseur`: a child
            // inheriting `PONT` would therefore take itself for a bridge, would hold
            // a ProjFS virtualisation root, and would NEVER capture
            // anything. Unlike the two others, this case is reachable
            // today — it is enough for a supervisor to be launched with
            // `PONT` in its environment, which `scripts/run-agent.sh`
            // makes possible with one variable.
            .env_remove("PONT");
        // 🔴 AND THE IDENTITY, whose omission is the defect of August 20th, 2026. A
        // child goes through `main.rs`'s enrolment exactly like the bridge:
        // acceptance run G1 did not see it because no window was open,
        // hence no child launched — the defect was invisible there on that
        // half, and it would have bitten at the first window.
        self.identite_heritee(&mut commande);
        let mut enfant = commande
            .spawn()
            .with_context(|| format!("lancement de l'enfant {}", consigne.session.0))?;
        let pid = enfant.id();

        // This function's only fallible post-processing. The trait's atomic
        // contract therefore requires it to kill the child itself before
        // returning `Err` — otherwise the child would run untracked and its
        // virtual output would stay captive from the pool of ten.
        let handle = HANDLE(enfant.as_raw_handle());
        if let Err(error) = unsafe { AssignProcessToJobObject(self.job, handle) } {
            if let Err(mise_a_mort) = enfant.kill() {
                tracing::error!(
                    pid, %mise_a_mort,
                    "enfant NON rattaché au job ET NON tué — il survivra au superviseur"
                );
            }
            let _ = enfant.wait();
            return Err(anyhow::Error::new(error)
                .context(format!("rattachement de l'enfant {pid} au job object")));
        }

        self.enfants().insert(
            pid,
            Enfant {
                processus: enfant,
                etat_illisible_signale: false,
            },
        );
        Ok(pid)
    }

    fn est_vivant(&self, pid: u32) -> bool {
        let mut enfants = self.enfants();
        let Some(enfant) = enfants.get_mut(&pid) else {
            // Unknown to this launcher: never launched by us, or death already
            // observed. In both cases it is not alive *for us*, and
            // that is the only question asked.
            return false;
        };
        match enfant.processus.try_wait() {
            Ok(None) => {
                enfant.etat_illisible_signale = false;
                true
            }
            Ok(Some(code)) => {
                tracing::info!(pid, ?code, "enfant terminé");
                // The `Child` goes with its handle: the PID becomes
                // recyclable again, but no one uses it any more.
                enfants.remove(&pid);
                false
            }
            Err(error) => {
                // An unreadable state is NOT a death: declaring it dead would
                // destroy the output of a child still capturing.
                //
                // Signalled ONCE per child, and not at each loop turn:
                // see `Enfant::etat_illisible_signale`.
                if !enfant.etat_illisible_signale {
                    enfant.etat_illisible_signale = true;
                    tracing::warn!(
                        pid, %error,
                        "état de l'enfant illisible, tenu pour vivant \
                         (signalé une seule fois tant que l'état reste illisible)"
                    );
                }
                true
            }
        }
    }

    fn tuer(&self, pid: u32) -> Result<()> {
        let mut enfant = self
            .enfants()
            .remove(&pid)
            .with_context(|| format!("processus {pid} inconnu de ce lanceur — rien à tuer"))?;
        // `Child::kill` goes through the retained HANDLE, never through the number: even
        // if Windows had recycled this PID, no third party can be targeted.
        let issue = enfant.processus.kill();
        let _ = enfant.processus.wait();
        issue.with_context(|| format!("terminaison du processus {pid}"))
    }
}
