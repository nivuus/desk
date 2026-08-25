//! La décision PURE de relance/stabilité d'un processus supervisé mais **non
//! fatal** — extraite de `superviseur::boucle::surveillance_pont` (round de
//! correction 2, 25 août 2026) pour compiler et se tester **sur l'hôte
//! Linux** : ce dernier fichier vit derrière `boucle.rs::#![cfg(windows)]`,
//! et `EtatPont` y lisait `Instant::now()` en dur — aucun des trois tests
//! du round de correction 1 ne pouvait donc couvrir le CÂBLAGE, seulement la
//! fonction `honorer_retry_suggere` prise isolément. La revue l'a mesuré :
//! retirer `surveillance_pont.rs` au bug d'avant (constante fixe) ou retirer
//! les deux appels à `honorer_retry_suggere` laissaient les DEUX `cargo test
//! --workspace` et `cargo check --target x86_64-pc-windows-gnu` intacts.
//!
//! 🔴 **CONVENTION DE NOMMAGE (`CLAUDE.md`, « Convention de module enfant »),
//! APPLIQUÉE ICI, PAS DEVINÉE.** Ce module ne porte le préfixe d'AUCUN module
//! de premier niveau existant : `relance` n'est déclaré nulle part dans
//! `main.rs` (`grep -n '^mod \|^pub mod ' agent/src/main.rs` ne rend aucun
//! `mod relance;`), donc `relance_pont` ne satisfait la forme `<parent>_
//! <enfant>` pour AUCUN `<parent>` de premier niveau — y compris `pont`
//! lui-même, bien qu'il soit un préfixe TEXTUEL du nom : la règle exige que
//! le préfixe soit `<parent>_`, c'est-à-dire que le nom COMMENCE par
//! `pont_`, ce que `relance_pont` ne fait pas. Il vit donc à la RACINE NUE,
//! `mod relance_pont;` ordinaire dans `main.rs`, exactement comme
//! `survie_verdict` (extrait quatre niveaux plus bas, sans parent court et
//! unique à préfixer) et pour la MÊME raison que `surveillance_pont` porte
//! son propre nom plutôt que `pont` : les deux évitent la confusion que
//! l'en-tête de `surveillance_pont.rs` nomme explicitement — «&nbsp;deux
//! `pont` dans le même graphe de modules n'attendraient qu'un lecteur pressé
//! pour se confondre&nbsp;». Un `pont_relance` aurait, lui, satisfait la
//! forme et serait allé sous `pont/relance.rs` — délibérément écarté : ce
//! module ne décrit rien du PONT lui-même, il décrit une politique de
//! SUPERVISION générique (relance espacée, à seuil de stabilité), aussi
//! indifférente au pont qu'à n'importe quel autre processus non fatal
//! qu'un superviseur voudrait un jour suivre de la même façon.
//!
//! 🔴 **CE QUE CE MODULE CORRIGE, ET QUI A JUSTIFIÉ L'EXTRACTION** : avant ce
//! round, le seuil de STABILITÉ (« le pont a-t-il assez vécu pour qu'une
//! mort future soit une information neuve ? ») était LE MÊME que l'espacement
//! PLANCHER entre deux tentatives — 500 ms — un raccourci qui tenait tant
//! qu'un pont refusé mourait en quelques millisecondes. Le correctif du
//! critique ② du round de correction 1 (`agent::signaling::
//! honorer_retry_suggere`) a changé cette prémisse : un pont refusé reste
//! désormais **vivant** (`pont_vivant() == true`) pendant qu'il honore le
//! délai suggéré par le relais, jusqu'à `plateforme::repli::REPLI_MAX_MS`
//! (30 s). Un pont vivant depuis 500 ms peut donc être en train de MOURIR
//! LENTEMENT, pas d'être stable — et le confondre rouvre exactement la
//! boucle de trace que le commit `7a00fcb` de cette branche a payée une
//! première fois sur un mécanisme voisin (« mon propre remède a fait de la
//! trace une boucle ») : « pont de nouveau stable » suivi d'un « pont
//! relancé », en boucle, à chaque cycle de refus — environ 4 lignes/minute,
//! ~5 800 par jour sur un partage CIFS, l'exacte classe de dommage que ce
//! module de trace existe pour éviter (voir sa doc, `cycle_signale`, dans
//! `surveillance_pont.rs`).
//!
//! **Le remède : DEUX seuils temporels distincts, plus jamais un seul.**
//! `SEUIL_STABILITE_MS`, employé par [`EtatRelance::stable`], est
//! désormais STRICTEMENT SUPÉRIEUR à `REPLI_MAX_MS` — avec une marge pour le
//! temps qu'il faut au pont pour ATTEINDRE ce sommeil (poignée de main WS,
//! refus, lecture du message) — si bien qu'un pont qui meurt encore pendant
//! son sommeil d'attente n'est JAMAIS déclaré stable entre-temps.
//! `ESPACEMENT_PLANCHER_MS`, inchangé, continue de border la cadence des
//! VRAIES tentatives de relance (`delai_de_repli(0)`).

use crate::plateforme::repli::{delai_de_repli, REPLI_MAX_MS};

/// Espacement PLANCHER entre deux tentatives, ET valeur du premier terme de
/// `delai_de_repli` (`delai_de_repli(0) == ESPACEMENT_PLANCHER_MS`, éprouvé
/// ci-dessous). Reprise de l'ex-`PERIODE_RELANCE_PONT_MIN`.
pub const ESPACEMENT_PLANCHER_MS: u64 = 500;

/// Seuil de STABILITÉ — voir le commentaire de tête du module. **Distinct de
/// `ESPACEMENT_PLANCHER_MS` depuis ce round**, et c'est tout le correctif :
/// confondre les deux avec `REPLI_MAX_MS` en jeu rouvre la boucle de trace.
///
/// `REPLI_MAX_MS` couvre le SOMMEIL que `honorer_retry_suggere` s'impose ;
/// la marge couvre le temps qu'il faut pour l'ATTEINDRE (connexion WS,
/// refus, lecture du message d'erreur) — non mesuré, choisi large plutôt que
/// juste : un seuil de stabilité trop court coûte une boucle de trace
/// entière (ce correctif), un seuil trop long ne coûte qu'un `info!` en
/// retard, jamais un défaut fonctionnel (voir la doc de
/// [`EtatRelance::stable`]).
pub const SEUIL_STABILITE_MS: u64 = REPLI_MAX_MS + 5_000;

/// L'état, PUR, d'un processus supervisé et relancé avec repli exponentiel.
///
/// Ne connaît ni PID, ni horloge murale, ni `LanceurDeProcessus` : ces
/// horodatages et cette IO restent dans `surveillance_pont.rs`, qui reçoit
/// ses décisions d'ici sous forme de millisecondes ÉCOULÉES — c'est ce qui
/// rend cette structure éprouvable sur l'hôte Linux (`#[cfg(windows)]` ne
/// gate rien ici).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EtatRelance {
    /// Tentatives CONSÉCUTIVES sans stabilité observée. Voir `tentative()`.
    tentative: u32,
    /// Vrai dès qu'un cycle de relance en cours a été signalé — voir la doc
    /// de `cycle_signale` dans `surveillance_pont.rs`, qui reste la seule
    /// responsable de la trace elle-même (ce module ne journalise rien).
    cycle_signale: bool,
}

impl EtatRelance {
    pub fn neuve() -> Self {
        Self { tentative: 0, cycle_signale: false }
    }

    /// Le nombre de tentatives consécutives — pour l'annexer aux traces de
    /// l'appelant (`tentative = self.relance.tentative()`), jamais pour
    /// décider quoi que ce soit ici.
    pub fn tentative(&self) -> u32 {
        self.tentative
    }

    /// L'espacement attendu avant la PROCHAINE tentative, en millisecondes —
    /// pur ré-emballage de `delai_de_repli(self.tentative)`.
    pub fn espacement_ms(&self) -> u64 {
        delai_de_repli(self.tentative)
    }

    /// Est-il temps de relancer, sachant que `ecoule_ms` millisecondes se
    /// sont écoulées depuis la dernière tentative ?
    pub fn doit_relancer(&self, ecoule_ms: u64) -> bool {
        ecoule_ms >= self.espacement_ms()
    }

    /// Enregistre une tentative RÉELLEMENT lancée (un `Command::spawn`
    /// effectué, qu'il réussisse ou non — voir la doc de `tentative` dans
    /// `surveillance_pont.rs` pour la raison : c'est le cas où `spawn`
    /// réussit et le processus meurt aussitôt qui doit faire croître ce
    /// compteur). Rend `true` si c'est le PREMIER lancement du cycle en
    /// cours — c'est ce qui doit gouverner l'émission d'une ligne de trace
    /// chez l'appelant, jamais un second lancement du même cycle.
    pub fn tentative_lancee(&mut self) -> bool {
        self.tentative = self.tentative.saturating_add(1);
        let premier_du_cycle = !self.cycle_signale;
        self.cycle_signale = true;
        premier_du_cycle
    }

    /// Le processus est vu VIVANT depuis `ecoule_ms` millisecondes écoulées
    /// depuis la dernière tentative. Rend `true` — et RÉARME l'état pour le
    /// prochain cycle (`tentative` à zéro, `cycle_signale` à faux) — SI ET
    /// SEULEMENT SI un cycle était en cours ET que `ecoule_ms` dépasse
    /// `SEUIL_STABILITE_MS`, **jamais** le seul `ESPACEMENT_PLANCHER_MS`.
    ///
    /// 🔴 **C'EST LA LIGNE QUI CORRIGE LE ROUND DE CORRECTION 2** : avant lui,
    /// le seuil ici était `ESPACEMENT_PLANCHER_MS` (500 ms), si bien qu'un
    /// processus refusé et endormi jusqu'à `REPLI_MAX_MS` (30 s) avant de
    /// mourir était déclaré stable dès 500 ms — see `stable_pendant_un_
    /// sommeil_de_refus_ne_declare_jamais_stable`, la rouge exacte de ce
    /// défaut, ci-dessous.
    pub fn stable(&mut self, ecoule_ms: u64) -> bool {
        if self.cycle_signale && ecoule_ms >= SEUIL_STABILITE_MS {
            self.tentative = 0;
            self.cycle_signale = false;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neuve_n_a_rien_a_relancer_ni_rien_a_stabiliser() {
        let etat = EtatRelance::neuve();
        assert_eq!(etat.tentative(), 0);
        assert_eq!(etat.espacement_ms(), ESPACEMENT_PLANCHER_MS);
    }

    /// `delai_de_repli(0)` est ÉGAL à `ESPACEMENT_PLANCHER_MS` — la propriété
    /// que la doc du module affirme, éprouvée plutôt que crue.
    #[test]
    fn le_premier_espacement_egale_le_plancher() {
        assert_eq!(delai_de_repli(0), ESPACEMENT_PLANCHER_MS);
    }

    /// 🔴 LE TEST QUI ANCRE LE CORRECTIF : `SEUIL_STABILITE_MS` doit rester
    /// STRICTEMENT AU-DESSUS de `REPLI_MAX_MS`, sans quoi la propriété que ce
    /// module existe pour garantir retombe le jour où l'un des deux dérive
    /// sans que l'autre suive — même patron que
    /// `frein.test.ts::FENETRE_REQUETES_MS_reste_plus_courte_que_FENETRE_MS`.
    #[test]
    fn le_seuil_de_stabilite_reste_strictement_au_dessus_du_plafond_de_repli() {
        assert!(
            SEUIL_STABILITE_MS > REPLI_MAX_MS,
            "SEUIL_STABILITE_MS = {SEUIL_STABILITE_MS} n'est pas > REPLI_MAX_MS = {REPLI_MAX_MS}"
        );
    }

    #[test]
    fn doit_relancer_est_faux_juste_apres_une_tentative() {
        let mut etat = EtatRelance::neuve();
        etat.tentative_lancee();
        assert!(!etat.doit_relancer(0));
        assert!(!etat.doit_relancer(ESPACEMENT_PLANCHER_MS - 1));
    }

    #[test]
    fn doit_relancer_devient_vrai_a_l_espacement_exact() {
        let etat = EtatRelance::neuve();
        assert!(etat.doit_relancer(ESPACEMENT_PLANCHER_MS));
    }

    #[test]
    fn seul_le_premier_lancement_du_cycle_est_signale() {
        let mut etat = EtatRelance::neuve();
        assert!(etat.tentative_lancee(), "le premier lancement doit être signalé");
        assert!(!etat.tentative_lancee(), "le second, du MÊME cycle, ne doit plus l'être");
        assert!(!etat.tentative_lancee(), "ni le troisième");
        assert_eq!(etat.tentative(), 3, "le COMPTE, lui, continue de croître");
    }

    /// 🔴 LA ROUGE EXACTE DU DÉFAUT CORRIGÉ PAR CE ROUND : un processus REFUSÉ
    /// qui reste vivant jusqu'à `REPLI_MAX_MS` avant de mourir (le sommeil de
    /// `honorer_retry_suggere`) ne doit JAMAIS être déclaré stable pendant ce
    /// sommeil. Sondé à intervalles réguliers, comme le ferait la boucle du
    /// superviseur à ~10 Hz.
    #[test]
    fn stable_pendant_un_sommeil_de_refus_ne_declare_jamais_stable() {
        let mut etat = EtatRelance::neuve();
        etat.tentative_lancee();
        let mut ecoule = 0u64;
        while ecoule < REPLI_MAX_MS {
            assert!(
                !etat.stable(ecoule),
                "déclaré stable à {ecoule} ms, alors que le sommeil de refus \
                 peut durer jusqu'à {REPLI_MAX_MS} ms — c'est la boucle de \
                 trace du round de correction 2"
            );
            ecoule += 97; // un pas non-rond, pour ne pas tomber sur un cas pile
        }
    }

    /// 🔵 TÉMOIN POSITIF : un processus RÉELLEMENT stable — vivant bien
    /// au-delà du sommeil de refus le plus long possible — est bien déclaré
    /// stable, et une seule fois.
    #[test]
    fn un_processus_reellement_stable_finit_par_etre_declare_stable_une_fois() {
        let mut etat = EtatRelance::neuve();
        etat.tentative_lancee();
        assert!(!etat.stable(SEUIL_STABILITE_MS - 1));
        assert!(etat.stable(SEUIL_STABILITE_MS));
        assert_eq!(etat.tentative(), 0, "réarmé");
        // Un second appel, cycle déjà retombé : plus rien à signaler tant
        // qu'aucune tentative neuve n'a eu lieu.
        assert!(!etat.stable(SEUIL_STABILITE_MS * 2));
    }

    /// Sans cycle en cours (aucune tentative lancée depuis la dernière
    /// stabilité), `stable` ne doit rien déclarer, quel que soit `ecoule_ms`
    /// — un pont jamais relancé n'a rien à « redevenir » stable.
    #[test]
    fn sans_cycle_en_cours_stable_ne_declare_jamais_rien() {
        let mut etat = EtatRelance::neuve();
        assert!(!etat.stable(SEUIL_STABILITE_MS * 10));
    }

    /// 🔴 LA SIMULATION DE PLUSIEURS CYCLES DE REFUS — le test qui compte les
    /// LIGNES QUI SERAIENT ÉMISES, pas les appels : c'est la forme que la
    /// revue a demandée. Chaque cycle : une tentative lancée, un sommeil de
    /// refus jusqu'à `REPLI_MAX_MS` sondé à ~10 Hz, puis la mort — le cycle
    /// suivant recommence.
    ///
    /// 🔵 **CE QUE LA PREMIÈRE VERSION DE CE TEST CROYAIT À TORT** (rougie
    /// avant correction, et c'est la preuve que la propriété n'était pas
    /// supposée) : « une ligne "lancé" par cycle ». C'est FAUX, et c'est même
    /// MEILLEUR que ça — parce que `stable()` ne rearme JAMAIS `cycle_signale`
    /// tant qu'aucun cycle n'atteint la VRAIE stabilité, `tentative_lancee()`
    /// rend `premier_du_cycle = false` pour TOUTE relance après la toute
    /// première de l'épisode entier. La ligne « lancé » ne sort donc **qu'UNE
    /// SEULE FOIS pour tout l'épisode de martèlement**, pas une fois par
    /// cycle — exactement la promesse de tête du module : « signaler la
    /// première fois, se taire tant que la situation se répète ». Le
    /// correctif du round 2 ne fait pas que fermer la fausse ligne « stable » :
    /// il restaure aussi le silence attendu sur « lancé », que le bug du
    /// seuil unique avait rouvert en réarmant `cycle_signale` à chaque
    /// fausse stabilité.
    #[test]
    fn sur_plusieurs_cycles_de_refus_une_seule_ligne_lancee_et_aucune_ligne_stable() {
        const PAS_MS: u64 = 100; // ~10 Hz, la cadence réelle de la boucle
        let mut etat = EtatRelance::neuve();
        let mut lignes_lancees = 0u32;
        let mut lignes_stables = 0u32;
        for _cycle in 0..5 {
            if etat.tentative_lancee() {
                lignes_lancees += 1;
            }
            let mut ecoule = 0u64;
            while ecoule < REPLI_MAX_MS {
                if etat.stable(ecoule) {
                    lignes_stables += 1;
                }
                ecoule += PAS_MS;
            }
            // Le processus meurt ici (fin du sommeil de refus) : la boucle
            // suivante relancera, donc un cycle NEUF commence côté OS — mais
            // `cycle_signale` reste vrai côté `EtatRelance`, puisqu'aucune
            // stabilité RÉELLE n'a été observée. C'est exactement le
            // `EtatPont` réel entre deux tours de `surveiller`.
        }
        assert_eq!(
            lignes_lancees, 1,
            "UNE SEULE ligne « lancé » pour tout l'épisode — jamais une par \
             cycle : c'est la propriété de silence que le module promet déjà, \
             et que le bug du seuil unique cassait en réarmant `cycle_signale` \
             à chaque fausse stabilité"
        );
        assert_eq!(
            lignes_stables, 0,
            "AUCUNE ligne « stable » ne doit sortir tant qu'aucun cycle n'a \
             vraiment tenu : c'est exactement la boucle que le round de \
             correction 2 ferme"
        );
    }

    /// 🔵 TÉMOIN : si un cycle finit par tenir RÉELLEMENT (le pont cesse
    /// d'être refusé), la ligne « stable » sort UNE fois, et le cycle SUIVANT
    /// redevient bruyant sur son premier lancement — la moitié du mécanisme
    /// que le test ci-dessus, à lui seul, ne peut pas prouver puisqu'aucun de
    /// ses cycles n'atteint jamais la stabilité.
    #[test]
    fn apres_une_vraie_stabilite_le_cycle_suivant_redevient_bruyant() {
        let mut etat = EtatRelance::neuve();
        assert!(etat.tentative_lancee(), "premier lancement de l'épisode : bruyant");
        assert!(etat.stable(SEUIL_STABILITE_MS), "vraiment resté vivant assez longtemps");
        assert!(
            etat.tentative_lancee(),
            "un cycle NEUF, après une vraie stabilité, redevient bruyant"
        );
    }
}
