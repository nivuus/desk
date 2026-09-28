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
            // La taille RETENUE (`Consigne::taille`), pas celle de la
            // sortie — voir sa doc. Lue par `demarrage`, redite au capteur à
            // l'attache (tâche 9 du sous-bloc D10).
            .env(
                "TAILLE_FENETRE",
                format!("{}x{}", consigne.taille.0, consigne.taille.1),
            )
            // Surtout PAS `SUPERVISEUR` : un enfant qui hériterait de la
            // variable se prendrait pour un superviseur et lancerait ses
            // propres enfants, indéfiniment.
            .env_remove("SUPERVISEUR")
            // **Correctif I6 de la revue finale.** `SUPERVISEUR` n'était pas
            // la seule variable héritable qui change le SENS d'un enfant.
            // `scripts/run-agent.sh:32` pose `$env:TEST_FILE` dès que la
            // variable est définie dans l'environnement d'appel : un
            // superviseur lancé ainsi ferait que CHAQUE enfant diffuse le
            // fichier de test et ne capture rien (`Config::test_file`, lu par
            // `demarrage`), sans le moindre avertissement.
            //
            // `WINDOW_TITLE` par la même règle : la consigne impose la fenêtre
            // par `FENETRE_HWND`, et `demarrage::source` ne retombe sur la
            // recherche par titre que si celui-là manque. Inoffensive tant que
            // `FENETRE_HWND` est posé — ce que fait la ligne ci-dessus — mais
            // la laisser entretiendrait l'idée qu'un enfant peut chercher sa
            // fenêtre par titre, ce qui est faux par construction.
            //
            // Les modes diagnostic (`CAPTURE_TEST`, `AUDIO_PROBE`,
            // `MULTIFENETRE_*`…) ne sont volontairement PAS retirés : ils sont
            // aiguillés par `diagnostics::aiguiller()`, qui court AVANT la
            // branche superviseur de `main` — un superviseur qui en porterait
            // un ne serait jamais devenu superviseur, et n'aurait donc jamais
            // lancé d'enfant. `BITRATE`, `ENCODER_FPS` et `SOURCE_TRACE`
            // restent hérités à dessein : ce sont des réglages, pas des
            // changements de mode.
            .env_remove("TEST_FILE")
            .env_remove("WINDOW_TITLE")
            // **I7 de la revue finale de branche du sous-bloc D4**, par la
            // même règle de symétrie que le bloc I6 ci-dessus : `CAPTEUR` est
            // une variable héritable qui change le SENS d'un processus — un
            // enfant qui la porterait deviendrait un second capteur, ne
            // diffuserait rien, et se disputerait le tube nommé avec le vrai.
            //
            // Le cas est INATTEIGNABLE aujourd'hui : `main.rs` prend la
            // branche capteur AVANT la branche superviseur, donc un
            // superviseur portant `CAPTEUR` ne serait jamais devenu
            // superviseur et n'aurait jamais lancé d'enfant. On la retire
            // quand même, exactement comme `lancer_capteur` retire
            // `SUPERVISEUR` par ce même raisonnement : ce qui protège l'enfant
            // ne doit pas dépendre de l'ordre de deux `if` dans un autre
            // fichier.
            .env_remove("CAPTEUR")
            // 🔴 **Et `PONT` — le seul des trois oublis qui casserait le
            // produit.** La branche `PONT` de `main.rs` est placée APRÈS
            // `CAPTEUR`, mais AVANT `config.superviseur` : un enfant qui
            // hériterait de `PONT` se prendrait donc pour un pont, tiendrait
            // une racine de virtualisation ProjFS, et ne capturerait JAMAIS
            // rien. Contrairement aux deux autres, ce cas-là est atteignable
            // dès aujourd'hui — il suffit qu'un superviseur soit lancé avec
            // `PONT` dans son environnement, ce que `scripts/run-agent.sh`
            // rend possible d'une variable.
            .env_remove("PONT");
        // 🔴 ET L'IDENTITÉ, dont l'oubli est le défaut du 20 août 2026. Un
        // enfant traverse l'enrôlement de `main.rs` exactement comme le pont :
        // la recette G1 ne l'a pas vu parce qu'aucune fenêtre n'était ouverte,
        // donc aucun enfant lancé — le défaut y était invisible sur cette
        // moitié-là, et il aurait mordu à la première fenêtre.
        self.identite_heritee(&mut commande);
        let mut enfant = commande
            .spawn()
            .with_context(|| format!("lancement de l'enfant {}", consigne.session.0))?;
        let pid = enfant.id();

        // Le seul post-traitement faillible de cette fonction. Le contrat
        // atomique du trait exige donc qu'il tue lui-même l'enfant avant de
        // rendre `Err` — sans quoi celui-ci tournerait sans être suivi et sa
        // sortie virtuelle resterait captive du vivier de dix.
        let handle = HANDLE(enfant.as_raw_handle());
        if let Err(erreur) = unsafe { AssignProcessToJobObject(self.job, handle) } {
            if let Err(mise_a_mort) = enfant.kill() {
                tracing::error!(
                    pid, %mise_a_mort,
                    "enfant NON rattaché au job ET NON tué — il survivra au superviseur"
                );
            }
            let _ = enfant.wait();
            return Err(anyhow::Error::new(erreur)
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
            // Inconnu de ce lanceur : jamais lancé par nous, ou mort déjà
            // constatée. Dans les deux cas il n'est pas vivant *pour nous*, et
            // c'est la seule question posée.
            return false;
        };
        match enfant.processus.try_wait() {
            Ok(None) => {
                enfant.etat_illisible_signale = false;
                true
            }
            Ok(Some(code)) => {
                tracing::info!(pid, ?code, "enfant terminé");
                // Le `Child` part avec son handle : le PID redevient
                // recyclable, mais plus personne ne s'en sert.
                enfants.remove(&pid);
                false
            }
            Err(erreur) => {
                // Un état illisible n'est PAS une mort : le déclarer mort ferait
                // détruire la sortie d'un enfant qui capture encore.
                //
                // Signalé UNE fois par enfant, et pas à chaque tour de boucle :
                // voir `Enfant::etat_illisible_signale`.
                if !enfant.etat_illisible_signale {
                    enfant.etat_illisible_signale = true;
                    tracing::warn!(
                        pid, %erreur,
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
        // `Child::kill` passe par le HANDLE retenu, jamais par le numéro : même
        // si Windows avait recyclé ce PID, aucun tiers ne peut être visé.
        let issue = enfant.processus.kill();
        let _ = enfant.processus.wait();
        issue.with_context(|| format!("terminaison du processus {pid}"))
    }
}
