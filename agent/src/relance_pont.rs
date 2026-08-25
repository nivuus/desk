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
//! 🔴 **CE ZÉRO N'EST PAS UNE FATALITÉ DU `#[cfg(windows)]` — C'EST UN
//! ARBITRAGE, ET LA REVUE DU ROUND DE CORRECTION 3 L'A RELEVÉ.** Le
//! paragraphe ci-dessus se lisait comme si aucun autre découpage n'était
//! possible ; il en existe un : `surveiller` (`surveillance_pont.rs`) est
//! une décision à TROIS branches — vivant-et-stable, vivant-mais-pas-encore-
//! stable, mort-donc-à-relancer — qui pourrait s'écrire comme une fonction
//! PURE rendant un VERDICT (`Decision::{RienAFaire, Stable, Relancer}`, ou
//! équivalent), ne laissant dans le fichier gaté qu'une quinzaine de lignes
//! d'E/S (lire `pont_vivant()`, lire l'horloge, appliquer le verdict,
//! tracer). **Non fait dans ce round** — la prescription portait sur le SENS
//! des seuils, pas sur la frontière d'extraction, et la déplacer aurait
//! élargi la portée sans y être invitée. **CE QUE CE CHOIX LAISSE NON
//! GARDÉ** : l'ORDRE dans lequel `surveiller` appelle ses deux méthodes
//! (`reinitialiser_le_repli` avant `stable`, ci-dessous — un ordre inversé
//! ne casserait rien AUJOURD'HUI vu l'invariant `ESPACEMENT_PLANCHER_MS <
//! SEUIL_STABILITE_MS`, testé plus bas, mais rien ne l'empêcherait de casser
//! un jour si l'un des deux seuils changeait de sens) et le CÂBLAGE
//! lui-même (quelle méthode est appelée, avec quel argument, dans quelle
//! branche) restent `#[cfg(windows)]`, donc non exercés par
//! `cargo test --workspace` — seule la DÉCISION, une fois les deux entrées
//! connues, est éprouvée ici.
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
//!
//! 🔴 **RAFFINÉ AU ROUND DE CORRECTION 3 : `SEUIL_STABILITE_MS` NE GOUVERNE
//! PLUS QUE LA TRACE, JAMAIS LA CADENCE — LA REVUE A MESURÉ LE DÉFAUT ET
//! DÉCIDÉ LE DÉCOUPLAGE.** Le round 2 (paragraphe ci-dessus) faisait porter
//! à `SEUIL_STABILITE_MS` DEUX décisions à la fois — quand la ligne
//! « stable » peut sortir, ET quand le repli exponentiel peut se réarmer
//! (`tentative = 0`). Mesuré au banc, sur un pont SAIN dont les sessions
//! durent 1 s, 5 s ou 20 s (donc SOUS ce seuil de 35 s) : le repli ne se
//! réarmait alors JAMAIS, et l'espacement restait CLOUÉ au plafond (30 s)
//! atteint par une panne PASSÉE et déjà résolue — jusqu'à 29 s d'attente
//! pour une reconnexion parfaitement saine, là où le comportement d'AVANT le
//! round 2 en coûtait 0. **Les deux seuils gouvernent désormais chacun UNE
//! question distincte** : `ESPACEMENT_PLANCHER_MS` (COURT) réarme le repli
//! ([`EtatRelance::reinitialiser_le_repli`]) dès qu'une vie NORMALE le
//! prouve — restaurant un argument que le round 2 avait supprimé sans le
//! relocaliser, voir la doc de cette méthode ; `SEUIL_STABILITE_MS` (LONG)
//! continue de gouverner SEULEMENT la trace ([`EtatRelance::stable`], qui ne
//! touche plus `tentative`). **Le découplage NE ROUVRE PAS la boucle de
//! trace** : `reinitialiser_le_repli` ne touche jamais `cycle_signale`, qui
//! reste l'UNIQUE porte des lignes « lancé »/« stable » — voir les deux
//! rouges rejouées, ci-dessous.

use crate::plateforme::repli::{delai_de_repli, REPLI_MAX_MS};

/// Espacement PLANCHER entre deux tentatives, ET valeur du premier terme de
/// `delai_de_repli` (`delai_de_repli(0) == ESPACEMENT_PLANCHER_MS`, éprouvé
/// ci-dessous). Reprise de l'ex-`PERIODE_RELANCE_PONT_MIN`.
///
/// 🔴 **RÉUTILISÉE DEPUIS LE ROUND DE CORRECTION 3 POUR UN SECOND RÔLE** :
/// le seuil COURT au-delà duquel [`EtatRelance::reinitialiser_le_repli`]
/// considère qu'une vie suffit à prouver qu'une panne passée est résolue.
/// Les deux rôles partagent la MÊME valeur par choix, pas par nécessité — la
/// cadence PLANCHER entre deux tentatives et la preuve de vie « suffisante »
/// pour réarmer le repli n'ont aucune raison structurelle de coïncider, mais
/// aucune mesure ne les distingue non plus. Les découpler resterait à faire
/// le jour où l'une des deux raisons se calibre indépendamment de l'autre.
pub const ESPACEMENT_PLANCHER_MS: u64 = 500;

/// Seuil de STABILITÉ — voir le commentaire de tête du module. **Distinct de
/// `ESPACEMENT_PLANCHER_MS` depuis ce round**, et c'est tout le correctif :
/// confondre les deux avec `REPLI_MAX_MS` en jeu rouvre la boucle de trace.
///
/// `REPLI_MAX_MS` couvre le SOMMEIL que `honorer_retry_suggere` s'impose ;
/// la marge couvre le temps qu'il faut pour l'ATTEINDRE (connexion WS,
/// refus, lecture du message d'erreur) — non mesuré, choisi large plutôt que
/// juste.
///
/// 🔴 **CE QUE CE SEUIL NE GOUVERNE PLUS, DEPUIS LE ROUND DE CORRECTION 3 —
/// LA CORRECTION D'UNE AFFIRMATION FAUSSE, RELEVÉE PAR LA REVUE.** Ce
/// commentaire affirmait ici qu'« un seuil trop long ne coûte qu'un `info!`
/// en retard, jamais un défaut fonctionnel ». **C'était FAUX tant que ce
/// seuil gouvernait AUSSI la remise à zéro du repli** (`EtatRelance::
/// tentative`, via `stable`) : un pont sain dont les sessions durent 1 s,
/// 5 s ou 20 s — bien sous ce seuil — ne réarmait alors JAMAIS son repli, et
/// après quelques cycles l'espacement restait CLOUÉ à `REPLI_MAX_MS` (30 s)
/// même pour des pannes FUTURES sans aucun rapport avec la précédente,
/// mesuré au banc. **Depuis ce round, `stable` ne touche plus `tentative` :
/// seul [`EtatRelance::reinitialiser_le_repli`] le fait, sur un seuil COURT**
/// (`ESPACEMENT_PLANCHER_MS`, voir sa doc). `SEUIL_STABILITE_MS` ne gouverne
/// donc plus désormais que DEUX choses, toutes deux des questions de TRACE,
/// jamais de CADENCE : le moment où la ligne « pont de nouveau stable » peut
/// sortir, et le moment où `cycle_signale` retombe (donc où un épisode de
/// martèlement redevient bruyant sur son PROCHAIN lancement). **C'est
/// SEULEMENT sous cette portée réduite que l'affirmation redevient vraie** :
/// un seuil de TRACE trop long ne coûte qu'un `info!` en retard — la CADENCE,
/// elle, ne dépend plus de ce seuil du tout.
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
    /// Tentatives CONSÉCUTIVES sans repli réarmé. Voir `tentative()`.
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

    /// Réarme le repli exponentiel — remet `tentative` à zéro — dès qu'un
    /// cycle est en cours ET que le processus a été vu vivant au moins
    /// `ESPACEMENT_PLANCHER_MS` depuis sa dernière tentative. **Ne touche
    /// PAS `cycle_signale`** : la trace reste gouvernée par `stable` et son
    /// seuil LONG — voir la doc de tête du module pour la raison du
    /// découplage (round de correction 3).
    ///
    /// 🔴 **RESTAURE UN ARGUMENT QUE LE ROUND DE CORRECTION 2 A SUPPRIMÉ SANS
    /// LE RELOCALISER** (relevé par la revue du round de correction 3,
    /// `grep -rn 'REMISE À ZÉRO' agent/src` ne rendait plus rien). Avant
    /// l'extraction de ce module, ce texte vivait sur la remise à zéro de
    /// `EtatPont::tentative`, dans `surveillance_pont.rs` :
    ///
    /// > « REMISE À ZÉRO ICI, ET NULLE PART AILLEURS : c'est ce qui fait
    /// > qu'une panne FUTURE reparte de l'espacement minimal plutôt que de
    /// > rester bloquée au plafond atteint par une panne PASSÉE, déjà
    /// > résolue […] un agent connecté depuis trois jours qui perd son
    /// > réseau une seconde doit reprendre en une demi-seconde, pas en
    /// > trente. »
    ///
    /// Le round 2 a déplacé la remise à zéro dans `stable`, sur le seuil
    /// LONG (`SEUIL_STABILITE_MS`) — ce qui rendait l'argument FAUX : un
    /// pont sain dont les sessions durent 1 s, 5 s ou 20 s (donc SOUS ce
    /// seuil) ne réarmait plus JAMAIS le repli, et une panne future, sans
    /// rapport, héritait du plafond d'une panne passée déjà résolue.
    /// **CETTE MÉTHODE RESTAURE LA PROPRIÉTÉ, ICI, SUR LE SEUIL COURT.**
    /// C'est désormais la SEULE remise à zéro de `tentative` du module —
    /// `stable` ne le fait plus, voir sa doc — et elle a TOUJOURS déjà agi
    /// avant que `stable` ne puisse rendre vrai, puisque
    /// `ESPACEMENT_PLANCHER_MS < SEUIL_STABILITE_MS` (fixé par un test
    /// dédié, ci-dessous, plutôt que laissé à une transitivité qu'un
    /// lecteur pressé pourrait manquer).
    ///
    /// 🔴 **NE ROUVRE PAS LA BOUCLE DE TRACE** : cette méthode ne touche
    /// jamais `cycle_signale`, donc ne peut jamais faire rendre `true` à
    /// `tentative_lancee` prématurément — c'est `cycle_signale`, jamais
    /// `tentative`, qui gouverne le silence des traces. Un pont dont le
    /// repli vient d'être réarmé peut très bien mourir l'instant suivant :
    /// il relancera VITE (bon), mais SILENCIEUSEMENT tant que le cycle, lui,
    /// n'a pas atteint la VRAIE stabilité — voir
    /// `une_vie_normale_mais_pas_stable_reinitialise_quand_meme_le_repli`,
    /// ci-dessous, qui éprouve exactement cette distinction.
    pub fn reinitialiser_le_repli(&mut self, ecoule_ms: u64) {
        if self.cycle_signale && ecoule_ms >= ESPACEMENT_PLANCHER_MS {
            self.tentative = 0;
        }
    }

    /// Le processus est vu VIVANT depuis `ecoule_ms` millisecondes écoulées
    /// depuis la dernière tentative. Rend `true` — et RÉARME `cycle_signale`
    /// pour le prochain cycle (jamais `tentative`, voir plus bas) — SI ET
    /// SEULEMENT SI un cycle était en cours ET que `ecoule_ms` dépasse
    /// `SEUIL_STABILITE_MS`, **jamais** le seul `ESPACEMENT_PLANCHER_MS`.
    ///
    /// 🔴 **C'EST LA LIGNE QUI CORRIGE LE ROUND DE CORRECTION 2** : avant lui,
    /// le seuil ici était `ESPACEMENT_PLANCHER_MS` (500 ms), si bien qu'un
    /// processus refusé et endormi jusqu'à `REPLI_MAX_MS` (30 s) avant de
    /// mourir était déclaré stable dès 500 ms — voir
    /// `stable_pendant_un_sommeil_de_refus_ne_declare_jamais_stable`, la
    /// rouge exacte de ce défaut, ci-dessous.
    ///
    /// 🔴 **DEPUIS LE ROUND DE CORRECTION 3, CETTE MÉTHODE NE TOUCHE PLUS
    /// `tentative`** — seule [`Self::reinitialiser_le_repli`] le fait
    /// désormais, sur un seuil COURT. Un seuil UNIQUE pour les deux
    /// questions à la fois (CADENCE du repli et VISIBILITÉ de la trace)
    /// rendait forcément l'une des deux fausse, quel que soit le choix :
    /// trop court, la trace boucle (le défaut du round 2) ; trop long, le
    /// repli reste CLOUÉ pour des pannes FUTURES sans rapport (le défaut que
    /// ce round-ci corrige — voir la doc de `SEUIL_STABILITE_MS`).
    pub fn stable(&mut self, ecoule_ms: u64) -> bool {
        if self.cycle_signale && ecoule_ms >= SEUIL_STABILITE_MS {
            self.cycle_signale = false;
            true
        } else {
            false
        }
    }
}

// Module de tests extrait dans son propre fichier — la règle des 500 lignes
// (`CLAUDE.md`) l'exige : les onze tests de ce module, avec leurs
// commentaires (chacun documente une propriété distincte, notamment les
// deux rouges rejouées au round de correction 3), pesaient à eux seuls plus
// que le fichier entier avant l'extraction. Même mécanisme que
// `superviseur/table.rs::#[path = "table/tests.rs"] mod tests;` — la clause
// de `CLAUDE.md` qui l'exempte de la convention de nommage des modules
// enfants le dit explicitement : « le même mécanisme Rust, employé pour une
// raison différente (la règle des 500 lignes) ».
#[cfg(test)]
#[path = "relance_pont/tests.rs"]
mod tests;
