//! Lancement et contrôle de vie du processus **pont fichiers**.
//!
//! Écrit dans un module enfant plutôt qu'en ligne dans `lanceur.rs`
//! (367 lignes, marge 133) : les jumeaux de `lancer_capteur` et
//! `capteur_vivant`, dans le style documentaire de ce fichier, y auraient pesé
//! 80 à 100 lignes et ramené la marge sous 45. Ce dépôt a payé quatre fois la
//! leçon « la marge regagnée par une extraction se reperd à la ronde suivante
//! si on la traite comme acquise » — ici la marge n'est pas prise du tout.
//! L'extraction est placée **avant** l'addition qui la rendrait nécessaire,
//! seul geste qui ait fonctionné dans ce dépôt (D9, `capteur/serveur/instances.rs`,
//! marge rendue de 10 à 65 ; les deux fichiers traités après coup en D9 ont été
//! **compressés**, geste que `CLAUDE.md` interdit nommément, puis extraits
//! quand même).
//!
//! **Même contrat ATOMIQUE que `lancer_capteur`** : `Err` signifie qu'aucun
//! processus ne tourne. Le rattachement au job est le seul post-traitement
//! faillible, et il tue donc lui-même le pont avant de rendre `Err`.
//!
//! `#![cfg(windows)]` hérité de `lanceur.rs` (son `#![cfg(windows)]` porte sur
//! le module entier, enfants compris) : aucun `#[path]` n'est écrit ici, et la
//! « convention de module enfant » de `CLAUDE.md` ne s'applique pas — elle ne
//! vise que les modules qu'on extrait d'un parent gaté pour les faire compiler
//! sur l'hôte, ce que celui-ci n'a aucune raison d'être.

use super::*;

impl LanceurDeProcessus {
    /// Lance le pont fichiers unique (`agent/src/pont.rs`) : même exécutable,
    /// `PONT=1`, **rattaché au même job object** que les enfants et le capteur.
    ///
    /// **Pourquoi il porte `SESSION_ID`, `SIGNALING_URL` et `LOCAL_IP` là où
    /// `lancer_capteur` n'en pose aucun** : le capteur ne parle à aucun
    /// signaling — il sert le média par tube nommé —, alors que le pont ouvre
    /// sa propre `PeerConnection` vers la page-shell.
    pub fn lancer_pont(&self) -> Result<u32> {
        // Composée ICI et non côté enfant, exactement comme la `Consigne`
        // d'un enfant l'est par `Table` : c'est le superviseur qui connaît le
        // préfixe de la VM. Le pont, lui, s'enrôlera pour son propre JETON —
        // le préfixe qu'il en tirera n'a pas à recomposer ce nom.
        let session = crate::superviseur::protocole::session_du_pont(&self.prefixe);
        let mut commande = std::process::Command::new(&self.executable);
        commande
            .env("PONT", "1")
            .env("SESSION_ID", &session)
            .env("SIGNALING_URL", &self.signaling_url)
            .env("LOCAL_IP", &self.local_ip)
            // Un pont qui hériterait de `SUPERVISEUR` se prendrait pour un
            // superviseur et lancerait ses propres enfants, indéfiniment.
            .env_remove("SUPERVISEUR")
            // Un pont qui hériterait de `CAPTEUR` se prendrait pour un
            // CAPTEUR, et non pour un pont : `main.rs` teste `CAPTEUR` AVANT
            // tout le reste et rendrait la main à `capteur::executer`. Le pont
            // ne démarrerait jamais, **sans qu'une seule ligne ne le dise** —
            // et le lecteur `Mes Fichiers` resterait absent sans cause
            // lisible.
            .env_remove("CAPTEUR")
            // Même motif que pour un enfant et pour le capteur : ces deux
            // variables changent le SENS d'une source. Le pont n'a pas de
            // source du tout, et `scripts/run-agent.sh` pose `TEST_FILE` dès
            // qu'elle est définie dans l'environnement d'appel.
            .env_remove("TEST_FILE")
            .env_remove("WINDOW_TITLE");
        // 🔴 **ET L'IDENTITÉ — C'EST LE DÉFAUT MESURÉ LE 20 AOÛT 2026, ET IL
        // ÉTAIT DANS CETTE LISTE-CI.** `AGENT_VM` et `AGENT_SECRET` n'y
        // figuraient pas : le pont s'enrôlait sous la MÊME identité que son
        // père, la plateforme n'admet qu'un socket par VM, et les deux
        // s'évinçaient sans terme — **95 enrôlements, 94 évictions en 64 s**,
        // à ~1,5 Hz, avec pour prix des messages incrémentaux perdus, des
        // réponses de lancement perdues, et deux boucles de découverte
        // d'applications au lieu d'une.
        //
        // ⚠️ **LE PONT A BIEN BESOIN D'UNE IDENTITÉ — mais d'un JETON, PAS
        // D'UN CANAL.** Il ouvre sa propre `PeerConnection` vers la page-shell
        // (c'est ce que dit `main.rs`, et c'est pourquoi il est placé APRÈS
        // l'enrôlement), donc il présente un jeton comme un enfant. Ce qu'il
        // ne fait JAMAIS du canal, en revanche : il ne bat pas le cœur de la
        // VM, ne pousse aucun catalogue, ne reçoit aucun ordre de lancement.
        // La question « identité propre, ou pas d'enrôlement du tout ? » se
        // tranche donc sur cet usage réel : **pas d'enrôlement**, et le jeton
        // du père par `AGENT_JETON`.
        self.identite_heritee(&mut commande);
        let mut pont = commande.spawn().context("lancement du pont fichiers")?;
        let pid = pont.id();
        let handle = HANDLE(pont.as_raw_handle() as *mut core::ffi::c_void);
        if let Err(erreur) = unsafe { AssignProcessToJobObject(self.job, handle) } {
            // Même contrat atomique que `lancer` et `lancer_capteur` : un pont
            // non rattaché au job survivrait au superviseur EN TENANT une
            // racine de virtualisation ProjFS, que rien ne démonterait.
            if let Err(mise_a_mort) = pont.kill() {
                tracing::error!(pid, %mise_a_mort,
                    "pont fichiers NON rattaché au job ET NON tué — il survivra au superviseur");
            }
            let _ = pont.wait();
            return Err(anyhow::Error::new(erreur)
                .context(format!("rattachement du pont fichiers {pid} au job object")));
        }
        tracing::info!(pid, session, "pont fichiers lancé");
        *self.pont() = Some(Enfant { processus: pont, etat_illisible_signale: false });
        Ok(pid)
    }

    /// Vrai tant que le pont lancé par le dernier `lancer_pont` réussi est
    /// vivant. Rend `false` si aucun pont n'a jamais été lancé, ou si le
    /// précédent est mort — aux appelants de rappeler `lancer_pont`.
    ///
    /// Même logique que `capteur_vivant` : un état illisible n'est **PAS**
    /// traité comme une mort — le déclarer mort ferait relancer un pont qui
    /// tourne peut-être encore, et deux ponts se disputeraient la même racine
    /// de virtualisation — et n'est journalisé qu'une fois tant qu'il persiste.
    pub fn pont_vivant(&self) -> bool {
        let mut pont = self.pont();
        let Some(en_cours) = pont.as_mut() else { return false };
        match en_cours.processus.try_wait() {
            Ok(None) => {
                en_cours.etat_illisible_signale = false;
                true
            }
            Ok(Some(code)) => {
                tracing::info!(pid = en_cours.processus.id(), ?code, "pont fichiers terminé");
                *pont = None;
                false
            }
            Err(erreur) => {
                if !en_cours.etat_illisible_signale {
                    en_cours.etat_illisible_signale = true;
                    tracing::warn!(
                        %erreur,
                        "état du pont fichiers illisible, tenu pour vivant \
                         (signalé une seule fois tant que l'état reste illisible)"
                    );
                }
                true
            }
        }
    }
}
