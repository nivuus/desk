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
//! une décision à TROIS branches — vivant, mort-avec-une-issue, absent — qui
//! pourrait s'écrire comme une fonction PURE rendant un VERDICT, ne laissant
//! dans le fichier gaté qu'une quinzaine de lignes d'E/S (lire l'état du
//! processus, lire l'horloge, appliquer le verdict, tracer). **Non fait, ni
//! au round 3 ni au round 4** — la prescription porte à chaque fois sur le
//! SENS de la décision, pas sur la frontière d'extraction. **CE QUE CE CHOIX
//! LAISSE NON GARDÉ, ET LE ROUND 4 EN AJOUTE UNE PIÈCE** : le CÂBLAGE
//! lui-même (quelle méthode est appelée, dans quelle branche, avec quel
//! argument) reste `#[cfg(windows)]`, donc non exercé par
//! `cargo test --workspace` — et il porte désormais une propriété dont TOUT
//! ce module dépend : **[`EtatObserve::Mort`] n'est rendu QU'UNE FOIS par
//! mort**. Elle tient au fait que `LanceurDeProcessus::etat_du_pont` pose
//! `*pont = None` dans la branche `Ok(Some(code))` de `try_wait`, si bien
//! qu'un second appel rend `Absent` et non une seconde `Mort` — rendre `Mort`
//! à chaque tour ferait réarmer le repli en boucle sur une sortie propre
//! ANCIENNE. Seule la DÉCISION, une fois l'observation connue, est éprouvée
//! ici.
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
//! 🔴 **CE QUE CE MODULE CORRIGE, ET QUI A JUSTIFIÉ L'EXTRACTION** : avant le
//! round 2, le seuil de STABILITÉ (« le pont a-t-il assez vécu pour qu'une
//! mort future soit une information neuve ? ») était LE MÊME que l'espacement
//! PLANCHER entre deux tentatives — 500 ms — un raccourci qui tenait tant
//! qu'un pont refusé mourait en quelques millisecondes. Le correctif du
//! critique ② du round de correction 1 (`agent::signaling::
//! honorer_retry_suggere`) a changé cette prémisse : un pont refusé reste
//! désormais **vivant** (le processus tourne) pendant qu'il honore le délai
//! suggéré par le relais, jusqu'à `plateforme::repli::REPLI_MAX_MS` (30 s).
//! **Un pont vivant depuis 500 ms peut donc être en train de MOURIR
//! LENTEMENT, pas d'être stable** — et le confondre rouvre exactement la
//! boucle de trace que le commit `7a00fcb` de cette branche a payée une
//! première fois sur un mécanisme voisin (« mon propre remède a fait de la
//! trace une boucle ») : « pont de nouveau stable » suivi d'un « pont
//! relancé », en boucle, à chaque cycle de refus.
//!
//! `SEUIL_STABILITE_MS`, employé par [`EtatRelance::stable`], est donc
//! STRICTEMENT SUPÉRIEUR à `REPLI_MAX_MS` — avec une marge pour le temps
//! qu'il faut au pont pour ATTEINDRE ce sommeil (poignée de main WS, refus,
//! lecture du message) — si bien qu'un pont qui meurt encore pendant son
//! sommeil d'attente n'est JAMAIS déclaré stable entre-temps.
//!
//! 🔴 **ROUND DE CORRECTION 4 : LE RÉARMEMENT DU REPLI NE SE JUGE PLUS SUR
//! UNE DURÉE, MAIS SUR L'ISSUE DE SORTIE — ET C'EST CE QUI FERME LES DEUX
//! DÉFAUTS À LA FOIS.** Le round 3 réarmait `tentative` dès qu'une vie
//! dépassait `ESPACEMENT_PLANCHER_MS` (500 ms), ce qui **contredisait le
//! paragraphe ci-dessus dans le même fichier** : une vie de 500 ms ne prouve
//! rien, puisque c'est précisément la forme d'un refus qui dort. `tentative`
//! retombait à zéro ~500 ms après CHAQUE lancement, et le repli ne pouvait
//! **structurellement plus croître** pour tout mode de panne où le pont vit
//! entre ~0,5 s et ~35 s — c'est-à-dire exactement le régime que le remède du
//! round 1 a créé. **Mesuré au banc** (connexions `/signal` par minute contre
//! le budget PARTAGÉ `REQUETES_MAX_ADRESSE`, 120 par 60 s) :
//!
//! | vie du pont | round 3 | round 4, mort en ERREUR | round 4, sortie PROPRE |
//! | --- | --- | --- | --- |
//! | 600 ms | **100** | 6 | **100** |
//! | 1 s | **60** | 6 | **60** |
//! | 2 s | **30** | 6 | **30** |
//! | 5 s | **12** | 6 | **12** |
//! | 10 s | 6 | 6 | 6 |
//!
//! ⚠️ **La ligne 600 ms s'atteignait SANS AUCUN `retryApresS`** : toute mort
//! répétée après une demi-seconde de vie (panne ProjFS, plantage) suffisait —
//! 100 connexions/minute, 83 % du budget partagé consommé par le pont seul.
//!
//! 🔴 **LA TROISIÈME COLONNE EST UNE RÉSERVE OUVERTE, ET ELLE VIT ICI PARCE
//! QU'UNE PREUVE QUI NE VIT QUE DANS UN RAPPORT GITIGNORÉ EST UNE PREUVE
//! PERDUE** (`CLAUDE.md` l'interdit nommément, après en avoir perdu six).
//! **Une sortie PROPRE réarme le repli sans aucune borne de cadence** : un
//! pont qui se terminerait proprement toutes les 600 ms rendrait toujours
//! 100 connexions/minute, et le plancher de 500 ms de
//! [`EtatRelance::doit_relancer`] serait alors la SEULE borne. Ce régime est
//! hors de portée de la boucle du superviseur seule — `pont::executer` ne
//! rend `Ok(())` qu'après la mort de son fil de transport, ce qui exige un
//! pair qui émette des offres, et ce pair consomme lui-même le budget
//! `/signal`. ⚠️ **Mais l'argument est plus faible qu'il n'y paraît** : ce
//! `Ok(())` court AUSSI sur un échec ICE survenu APRÈS l'envoi de la réponse
//! SDP, pas seulement après une session réellement établie. **Non mesuré,
//! non borné, et écrit ici plutôt que passé sous silence.**
//!
//! **Ce n'était PAS un réglage de seuil, et il ne faut pas y retourner** :
//! une session SAINE qui se termine occupe le MÊME intervalle (1 à 30 s)
//! qu'un pont refusé qui a dormi. Seuil LONG ⇒ le défaut que le round 3
//! corrigeait (un pont sain à sessions courtes ne réarme jamais, jusqu'à 29 s
//! d'indisponibilité pour une panne future sans rapport) ; seuil COURT ⇒ le
//! défaut ci-dessus.
//!
//! ⚠️ **UNE DURÉE *COURTE* NE DISCRIMINE PAS — ET LA PREMIÈRE RÉDACTION DE
//! CE PARAGRAPHE ÉCRIVAIT « la durée n'est pas le discriminant », CE QUI EST
//! TROP FORT** (relevé par la revue du round de correction 5). Une durée
//! **au-dessus de `REPLI_MAX_MS`** discrimine parfaitement, et ce module en
//! possède une : `SEUIL_STABILITE_MS` (35 s) est **structurellement
//! inatteignable par un pont refusé qui dort**, puisque ce sommeil est borné
//! à 30 s — c'est l'invariant que
//! `le_seuil_de_stabilite_reste_strictement_au_dessus_du_plafond_de_repli`
//! et `stable_pendant_un_sommeil_de_refus_ne_declare_jamais_stable` tiennent
//! déjà. Le round 4 avait retiré la durée **en bloc**, emportant le seul cas
//! qu'elle traitait juste ; le round 5 le lui rend, et **`stable()` remet
//! `tentative` à zéro** (voir sa doc).
//!
//! 🔵 **LE DISCRIMINANT ÉTAIT DÉJÀ LU, PUIS JETÉ.** `pont::executer` rend
//! `Ok(())` en fin normale et `bail!` sur refus — juste après avoir honoré
//! `retryApresS` —, et `main() -> Result<()>` traduit l'un en code de sortie
//! **0** et l'autre en code **non nul** ; l'ex-`pont_vivant()` recevait ce
//! code dans `Ok(Some(code))` **pour le journaliser et le laisser tomber**.
//! Il traverse désormais la frontière sous la forme d'une [`IssueDeSortie`],
//! et **c'est elle, jamais une durée, qui réarme le repli** : une session
//! saine qui se termine réarme, un pont refusé qui meurt en erreur ne réarme
//! pas, quelle qu'ait été sa durée de vie.
//!
//! 🔵 **CE QUE LE ROUND 4 REND À `cycle_signale` ET À `SEUIL_STABILITE_MS`.**
//! Le round 3 avait fait de `cycle_signale` une garde de
//! [`EtatRelance::reinitialiser_le_repli`], donc un gouverneur de CADENCE —
//! un TROISIÈME rôle que sa doc ne nommait pas, et la forme exacte du défaut
//! que ce round-là corrigeait. La garde a disparu avec la durée : elle
//! n'était plus seulement non nommée, elle était devenue **fausse**, un pont
//! déclaré stable puis mort proprement ayant `cycle_signale == false` et ne
//! réarmant donc rien. `cycle_signale` ne gouverne de nouveau QUE la trace.
//!
//! 🔴 **EN REVANCHE, `SEUIL_STABILITE_MS` GOUVERNE DE NOUVEAU UNE CADENCE
//! DEPUIS LE ROUND 5, ET C'EST ÉCRIT ICI PLUTÔT QUE DÉCOUVERT** — le round 4
//! affirmait à cet endroit qu'il ne gouvernait « que la trace, sans
//! réserve », et cette phrase serait devenue fausse en silence. La
//! différence avec le round 3, qui avait payé exactement ce mécanisme : le
//! seuil qui gouverne cette cadence-ci est **au-dessus de `REPLI_MAX_MS`**,
//! donc hors d'atteinte d'un refus endormi, là où celui du round 3 était le
//! plancher de 500 ms. Voir la doc de la constante.

use crate::plateforme::repli::{delai_de_repli, REPLI_MAX_MS};

/// Espacement PLANCHER entre deux tentatives, ET valeur du premier terme de
/// `delai_de_repli` (`delai_de_repli(0) == ESPACEMENT_PLANCHER_MS`, éprouvé
/// ci-dessous). Reprise de l'ex-`PERIODE_RELANCE_PONT_MIN`.
///
/// 🔴 **ELLE A DEUX RÔLES, ET LES VOICI TOUS LES DEUX — LE SECOND N'ÉTAIT PAS
/// NOMMÉ AVANT LE ROUND DE CORRECTION 4** (la revue l'a relevé : la doc en
/// énumérait deux et le code en servait trois, dont l'un — la garde du
/// réarmement du repli — a disparu avec le round 4, voir la doc de tête).
///
/// 1. **La cadence PLANCHER des tentatives** : `EtatRelance::doit_relancer`
///    ne rend vrai qu'au-delà de `delai_de_repli(tentative)`, dont c'est le
///    premier terme.
/// 2. **Le RECUL de `derniere_tentative` dans `EtatPont::demarrer`**
///    (`surveillance_pont.rs`) : l'horloge y est reculée d'exactement cette
///    valeur pour que la TOUTE PREMIÈRE tentative ait lieu maintenant et non
///    dans 500 ms. Ce rôle-là n'est pas une cadence, c'est une amorce — et
///    il lie les deux fichiers : relever cette constante retarderait le
///    démarrage du pont d'autant, ce qui ne se lit pas ici.
///
/// ⚠️ **ELLE EST SOUDÉE À `plateforme::repli::REPLI_MIN_MS` PAR UN TEST**
/// (`le_premier_espacement_egale_le_plancher`, qui exige
/// `delai_de_repli(0) == ESPACEMENT_PLANCHER_MS`, c'est-à-dire
/// `REPLI_MIN_MS == ESPACEMENT_PLANCHER_MS`) : **elle ne peut donc PAS être
/// relevée seule**. La relever exige de relever `REPLI_MIN_MS` — qui gouverne
/// aussi la reprise du canal `/agent` — ou de casser cette soudure
/// délibérément. Ni l'une ni l'autre n'est calibrée.
pub const ESPACEMENT_PLANCHER_MS: u64 = 500;

/// Seuil de STABILITÉ — voir le commentaire de tête du module. **Distinct de
/// `ESPACEMENT_PLANCHER_MS` depuis le round 2**, et c'est tout ce correctif :
/// confondre les deux avec `REPLI_MAX_MS` en jeu rouvre la boucle de trace.
///
/// `REPLI_MAX_MS` couvre le SOMMEIL que `honorer_retry_suggere` s'impose ;
/// la marge couvre le temps qu'il faut pour l'ATTEINDRE (connexion WS,
/// refus, lecture du message d'erreur) — non mesuré, choisi large plutôt que
/// juste.
///
/// 🔴 **CE SEUIL GOUVERNE TROIS CHOSES, ET LA TROISIÈME EST UNE CADENCE.**
/// Les rounds 3 et 4 ont écrit tour à tour qu'il ne gouvernait « que la
/// trace » — les deux fois, c'était faux, et pas de la même façon. Voici les
/// trois, énumérées plutôt que découvertes :
///
/// 1. **TRACE** — le moment où la ligne « pont de nouveau stable » peut
///    sortir ;
/// 2. **TRACE** — le moment où `cycle_signale` retombe, donc où un épisode
///    de martèlement redevient bruyant sur son PROCHAIN lancement ;
/// 3. 🔴 **CADENCE, depuis le round de correction 5** — [`EtatRelance::
///    stable`] remet `tentative` à zéro dans sa branche vraie. C'est ce qui
///    fait qu'un pont vivant **trois jours**, puis coupé par une panne
///    réseau (donc mort EN ERREUR, que `reinitialiser_le_repli` refuse à bon
///    droit de tenir pour une preuve), reprenne en une demi-seconde et non
///    en trente — la propriété que la citation de `reinitialiser_le_repli`
///    revendique, et que le round 4 avait laissée fausse pour ce cas.
///
/// ⚠️ **POURQUOI CE N'EST PAS LE DÉFAUT DU ROUND 3, QUI ÉTAIT EXACTEMENT CE
/// MÉCANISME.** Là-bas, la cadence était gouvernée par un seuil de 500 ms,
/// qu'un pont refusé atteint TOUJOURS avant de mourir : le réarmement était
/// donc systématique et le repli ne pouvait plus croître. Ici, le seuil est
/// `REPLI_MAX_MS + 5 s` = 35 s, et le sommeil de `honorer_retry_suggere` est
/// borné à 30 s : **un pont refusé ne peut STRUCTURELLEMENT pas l'atteindre**
/// — c'est l'invariant que
/// `le_seuil_de_stabilite_reste_strictement_au_dessus_du_plafond_de_repli`
/// fixe, et que `stable_pendant_un_sommeil_de_refus_ne_declare_jamais_stable`
/// éprouve tick par tick. **Coût mesuré au banc : NUL** — la table de
/// martèlement de la doc de tête ne bouge d'aucune unité, pour toute durée de
/// vie inférieure ou égale à `REPLI_MAX_MS`.
///
/// ⚠️ **CONSÉQUENCE : « un seuil de trace trop long ne coûte qu'un `info!` en
/// retard » REDEVIENT FAUX.** Le relever au-delà de la durée de vie réelle
/// des sessions saines rendrait le rôle ③ inatteignable, et la reprise après
/// une panne réseau repasserait à 30 s.
pub const SEUIL_STABILITE_MS: u64 = REPLI_MAX_MS + 5_000;

/// Les deux types d'observation — [`IssueDeSortie`] et [`EtatObserve`] —
/// vivent dans leur propre fichier depuis le 25 août 2026 : voir son en-tête
/// pour la raison (règle des 500 lignes, extraction jouée AVANT l'addition
/// qui la rendait nécessaire). **Ré-exportés ici**, si bien qu'aucun
/// appelant ne change de chemin : `crate::relance_pont::IssueDeSortie` reste
/// valide.
mod issue;
pub use issue::{EtatObserve, IssueDeSortie};

/// L'état, PUR, d'un processus supervisé et relancé avec repli exponentiel.
///
/// Ne connaît ni PID, ni horloge murale, ni `LanceurDeProcessus` : ces
/// horodatages et cette IO restent dans `surveillance_pont.rs`, qui reçoit
/// ses décisions d'ici sous forme de millisecondes ÉCOULÉES et d'issues —
/// c'est ce qui rend cette structure éprouvable sur l'hôte Linux
/// (`#[cfg(windows)]` ne gate rien ici).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EtatRelance {
    /// Tentatives CONSÉCUTIVES sans repli réarmé. Voir `tentative()`.
    tentative: u32,
    /// Vrai dès qu'un cycle de relance en cours a été signalé — voir la doc
    /// de `cycle_signale` dans `surveillance_pont.rs`, qui reste la seule
    /// responsable de la trace elle-même (ce module ne journalise rien).
    ///
    /// ⚠️ **IL GARDE DE NOUVEAU UNE REMISE À ZÉRO DE `tentative` DEPUIS LE
    /// ROUND 5, ET C'EST NOMMÉ ICI PLUTÔT QUE DÉCOUVERT** — c'est le défaut
    /// de forme que la revue du round 4 avait relevé sur ce champ, et il ne
    /// sera pas payé deux fois. [`EtatRelance::stable`] exige `cycle_signale`
    /// AVANT de remettre `tentative` à zéro, donc ce booléen conditionne bel
    /// et bien une cadence. **Il ne peut cependant jamais la BLOQUER**, par
    /// un invariant qui se lit sur les trois seules écritures du champ :
    /// `tentative` ne croît QUE dans `tentative_lancee`, qui pose
    /// `cycle_signale = true` dans le même geste ; et les deux remises à zéro
    /// laissent `tentative` nul. Donc **`tentative > 0` implique
    /// `cycle_signale == true`**, et la garde ne peut refuser qu'un
    /// réarmement qui n'aurait rien à réarmer. Éprouvé par
    /// `un_processus_reellement_stable_finit_par_etre_declare_stable_une_fois`.
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

    /// Réarme le repli exponentiel — remet `tentative` à zéro — **si et
    /// seulement si l'issue du processus mort prouve qu'une panne passée est
    /// résolue**, c'est-à-dire sur une sortie PROPRE. Ne prend AUCUNE durée,
    /// et c'est tout le round de correction 4 : voir la doc de tête pour la
    /// mesure qui l'a exigé.
    ///
    /// 🔴 **RESTAURE UN ARGUMENT QUE LE ROUND DE CORRECTION 2 A SUPPRIMÉ SANS
    /// LE RELOCALISER** (relevé par la revue du round de correction 3).
    /// Avant l'extraction de ce module, ce texte vivait sur la remise à zéro
    /// de `EtatPont::tentative`, dans `surveillance_pont.rs` :
    ///
    /// > « REMISE À ZÉRO ICI, ET NULLE PART AILLEURS : c'est ce qui fait
    /// > qu'une panne FUTURE reparte de l'espacement minimal plutôt que de
    /// > rester bloquée au plafond atteint par une panne PASSÉE, déjà
    /// > résolue […] un agent connecté depuis trois jours qui perd son
    /// > réseau une seconde doit reprendre en une demi-seconde, pas en
    /// > trente. »
    ///
    /// 🔴 **CETTE MÉTHODE, SEULE, NE TIENT PAS CETTE CITATION — ET LE ROUND
    /// 4 AFFIRMAIT LE CONTRAIRE ICI MÊME** (« la propriété tient toujours,
    /// mais sa PREUVE a changé »). **Elle ne tenait pas** : « perdre son
    /// réseau » est une mort EN ERREUR, pas une sortie propre. Scénario
    /// simulé par la revue du round 5 : six refus (espacement au plafond),
    /// puis un septième lancement qui SERT TROIS JOURS, puis une coupure
    /// réseau — le repli restait à **30 000 ms**, exactement le défaut que le
    /// round 3 existait pour corriger, rouvert pour le cas mort-en-erreur.
    ///
    /// **La citation est désormais tenue par DEUX portes, et il en faut
    /// deux** : celle-ci, `IssueDeSortie::Propre`, pour une session courte
    /// qui se termine bien ; et [`Self::stable`], qui remet `tentative` à
    /// zéro dès qu'une vie dépasse `SEUIL_STABILITE_MS` (35 s), pour une
    /// longue vie qui finit MAL. **Ce n'est donc plus la seule remise à zéro
    /// du module**, et le dire faux coûtait un défaut.
    ///
    /// 🔴 **AUCUNE GARDE SUR `cycle_signale`, ET C'EST DÉLIBÉRÉ.** Le round 3
    /// en posait une ; elle serait devenue FAUSSE ici : un pont déclaré
    /// stable (donc `cycle_signale == false`) puis terminé proprement ne
    /// réarmerait rien, et la panne suivante hériterait d'un plafond. Elle
    /// faisait en outre de `cycle_signale` un gouverneur de CADENCE, ce que
    /// sa doc nie. ⚠️ **`SEUIL_STABILITE_MS`, LUI, EN GOUVERNE BIEN UNE
    /// DEPUIS LE ROUND 5** — mais par `stable`, sur un seuil hors d'atteinte
    /// d'un refus endormi, et sa doc l'énumère plutôt que de le nier.
    ///
    /// 🔴 **NE ROUVRE PAS LA BOUCLE DE TRACE** : cette méthode ne touche
    /// jamais `cycle_signale`, donc ne peut jamais faire rendre `true` à
    /// `tentative_lancee` prématurément — c'est `cycle_signale`, jamais
    /// `tentative`, qui gouverne le silence des traces.
    pub fn reinitialiser_le_repli(&mut self, issue: IssueDeSortie) {
        if issue.prouve_une_panne_resolue() {
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
    /// 🔴 **ET ELLE REMET `tentative` À ZÉRO, DEPUIS LE ROUND DE CORRECTION
    /// 5 — CETTE LIGNE FERME UN DÉFAUT AU LIEU DE LE DOCUMENTER.** Le round 3
    /// faisait cette remise à zéro sur un seuil de 500 ms (donc systématique,
    /// donc un repli qui ne pouvait plus croître) ; le round 4 l'a retirée
    /// **en bloc**, emportant avec elle le seul cas qu'elle traitait juste :
    /// **un pont vivant TROIS JOURS puis coupé par une panne réseau meurt EN
    /// ERREUR**, donc `reinitialiser_le_repli` refuse — à bon droit — d'y
    /// voir une preuve, et l'attente restait à `REPLI_MAX_MS` (30 s) si un
    /// épisode de refus l'avait précédée. Voir la citation de
    /// [`Self::reinitialiser_le_repli`], que ce round rend enfin vraie.
    ///
    /// ⚠️ **CE QUI REND CETTE REMISE À ZÉRO SÛRE, LÀ OÙ CELLE DU ROUND 3 NE
    /// L'ÉTAIT PAS** : le seuil vaut `REPLI_MAX_MS + 5 s`, qu'un pont refusé
    /// qui dort **ne peut pas atteindre** — son sommeil est borné à 30 s. La
    /// table de martèlement de la doc de tête ne bouge donc d'AUCUNE unité,
    /// et c'est mesuré, pas déduit. ⚠️ Cela redonne à `SEUIL_STABILITE_MS` un
    /// rôle de CADENCE : sa doc l'énumère, en troisième position.
    pub fn stable(&mut self, ecoule_ms: u64) -> bool {
        if self.cycle_signale && ecoule_ms >= SEUIL_STABILITE_MS {
            self.cycle_signale = false;
            // 🔴 LA LIGNE DU ROUND 5. Voir la doc ci-dessus : une vie qui
            // dépasse `SEUIL_STABILITE_MS` est une preuve de santé qu'aucun
            // refus endormi ne peut fabriquer, et c'est la SEULE porte par
            // laquelle une longue vie terminée EN ERREUR réarme le repli.
            self.tentative = 0;
            true
        } else {
            false
        }
    }
}

// Module de tests extrait dans son propre fichier — la règle des 500 lignes
// (`CLAUDE.md`) l'exige : les tests de ce module, avec leurs commentaires
// (chacun documente une propriété distincte, notamment les rouges rejouées
// aux rounds 3 et 4), pèsent à eux seuls plus que le fichier entier avant
// l'extraction. ⚠️ **AUCUN COMPTE N'EST ÉCRIT ICI, ET C'EST VOULU** : une
// rédaction antérieure annonçait « les onze tests de ce module » alors qu'il
// y en avait quinze — faux à l'octet où la phrase a été écrite. Le compte se
// relève, il ne se recopie pas :
// `grep -c '^#\[test\]' agent/src/relance_pont/tests.rs`.
// Même mécanisme que `superviseur/table.rs::#[path = "table/tests.rs"] mod
// tests;` — la clause de `CLAUDE.md` qui l'exempte de la convention de
// nommage des modules enfants le dit explicitement : « le même mécanisme
// Rust, employé pour une raison différente (la règle des 500 lignes) ».
#[cfg(test)]
#[path = "relance_pont/tests.rs"]
mod tests;
