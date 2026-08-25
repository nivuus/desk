//! Lancement et surveillance du **pont fichiers**.
//!
//! Jumeau de `surveillance_capteur.rs`, dont il transpose les trois
//! mécanismes — l'espacement DES relances, le signalement une fois par
//! cycle, et la condition de durée du réarmement. **Une seule chose diffère,
//! et c'est la seule décision de ce module : le démarrage n'est PAS fatal.**
//!
//! **`surveillance_pont` et non `pont`**, pour la raison exacte qui a fait
//! nommer son jumeau `surveillance_capteur` (I7 de la revue finale de branche
//! du sous-bloc D4) : `crate::pont` existe déjà et désigne AUTRE CHOSE — le
//! pont lui-même, c'est-à-dire le processus qui tient la racine de
//! virtualisation ProjFS. Ce module-ci n'en est que la supervision, vue du
//! superviseur ; il ne virtualise rien. Ce fichier fait `use super::*`, et
//! deux `pont` dans le même graphe de modules n'attendraient qu'un lecteur
//! pressé pour se confondre.
//!
//! 🔴 **CORRECTIF DU LEGS DES FREINS MANQUANTS (round de correction 1,
//! 25 août 2026), CRITIQUE ③.** L'espacement des relances était une
//! constante FIXE (`PERIODE_RELANCE_PONT_MIN`, 500 ms) : un PLANCHER entre
//! deux tentatives, jamais un PLAFOND. Chaîne MESURÉE : le pont ouvre
//! `/signal` → le budget « toute requête » de la plateforme
//! (`plateforme/src/securite/frein.ts::BUDGET_REQUETES`, 60/minute) est déjà
//! épuisé pour l'adresse de cette VM → le relais refuse et ferme en `1008`
//! → `agent/src/signaling.rs` voit son canal `offers` se fermer sans offre
//! → `pont::executer` rend une `Err` → le PROCESSUS meurt → ce module le
//! relance 500 ms plus tard → refusé de nouveau. **120 relances par minute
//! contre un budget de 60, sur une clé PARTAGÉE avec la session de contrôle
//! du superviseur ET chaque enfant de fenêtre** (même adresse source) :
//! aucune fenêtre neuve ne peut plus s'attacher tant que ce pont s'obstine.
//!
//! **Le remède RÉUTILISE `plateforme::repli::delai_de_repli`** — celui qui
//! protège déjà le canal `/agent` (`plateforme.rs`, boucle de reprise de
//! `une_session`) — plutôt que d'en écrire un second. L'état PUR qui compte
//! les tentatives et décide de la stabilité vit désormais dans
//! [`crate::relance_pont::EtatRelance`] (round de correction 2, voir sa doc
//! pour la raison de l'extraction et la convention de nommage appliquée) :
//! un pont qui meurt en boucle finit donc par ne plus revenir qu'à
//! 2 tentatives/minute — largement sous le budget partagé — plutôt qu'à 120.
//!
//! 🔴 **LES DEUX MOITIÉS DE CE REMÈDE NE S'ADDITIONNENT PAS — ET NE SE
//! SUBSTITUENT PAS NON PLUS AU SENS OÙ L'UNE REMPLACERAIT L'AUTRE : ELLES SE
//! COMPOSENT PAR UN MAXIMUM**, et une formulation antérieure de ce
//! paragraphe (round 2) affirmait à tort que le repli exponentiel d'ici
//! restait « la SEULE chose » à borner la cadence — corrigé à son tour,
//! pour ne pas remplacer une formulation trop optimiste par une autre.
//!
//! `agent::signaling::honorer_retry_suggere` (round 1, critique ②) fait
//! dormir le pont refusé jusqu'au délai que le relais a suggéré (borné à
//! `REPLI_MAX_MS`, 30 s) ; le pont ne meurt qu'APRÈS ce sommeil. Le prochain
//! `tenter()` compare alors le temps RÉELLEMENT écoulé depuis le dernier
//! lancement — qui inclut ce sommeil — à `EtatRelance::espacement_ms()`
//! (`delai_de_repli(tentative)`), et ne relance QUE si le premier dépasse le
//! second : **le délai effectif entre deux lancements est donc le PLUS
//! GRAND des deux**, jamais leur somme.
//!
//! **Les deux plafonnent à LA MÊME VALEUR** (`REPLI_MAX_MS`, 30 s) —
//! `delai_de_repli` par construction (`repli.rs`), le sommeil de
//! `honorer_retry_suggere` par la borne qu'il s'impose sur `retryApresS` —
//! si bien que ni l'un ni l'autre ne peut jamais DÉPASSER durablement
//! l'autre : `delai_de_repli(tentative)` atteint 30 s pile à `tentative = 6`
//! (`500 × 2⁶ = 32 000`, ÉCRÊTÉ à `30 000`, jamais 32 000) et y reste.
//! Sur le scénario RÉELLEMENT mesuré — un budget de volume saturé qui
//! suggère un `retryApresS` proche de 60 s, donc un sommeil constamment
//! écrêté à 30 s — le sommeil DOMINE (est la valeur RÉELLEMENT contraignante
//! du `max`) pour les six premières tentatives consécutives, où `delai_de_
//! repli` reste sous 30 s : le repli d'ici n'y fait alors QUE constater que
//! l'espacement est déjà satisfait, sans jamais l'imposer. À partir de la
//! sixième, les deux valent 30 s et deviennent indiscernables l'un de
//! l'autre — ni ne « domine » l'autre, ils coïncident. **Le round de
//! correction 1 documentait donc un plafond de cadence correct dans sa
//! conclusion (« 2 tentatives/minute »), mais pour une raison INCOMPLÈTE** :
//! sur ce scénario précis, ce plafond vient du sommeil de `honorer_retry_
//! suggere` au moins autant que du repli exponentiel, jamais de ce dernier
//! SEUL. Un `retryApresS` plus court (fenêtre moins saturée) inverserait
//! l'équilibre en faveur du repli — non mesuré non plus.

use super::*;
use crate::relance_pont::{EtatObserve, EtatRelance};

/// Ce que la boucle retient du pont d'un tour à l'autre.
///
/// `pid` vaut `None` tant qu'aucun lancement n'a réussi — état qui n'existe
/// pas chez le capteur, dont le démarrage est fatal et qui a donc toujours un
/// PID dès sa construction.
pub(super) struct EtatPont {
    pid: Option<u32>,
    derniere_tentative: std::time::Instant,
    /// La décision PURE — tentatives, espacement, stabilité — vit dans
    /// [`EtatRelance`] depuis le round de correction 2 : ce champ ne porte
    /// plus lui-même ni compteur ni booléen, seulement l'horloge murale
    /// (`derniere_tentative`, ci-dessus) et l'identité du processus (`pid`),
    /// qui restent propres à CE module parce qu'ils touchent
    /// `std::time::Instant` et `LanceurDeProcessus` — non portables, donc
    /// non extraits.
    relance: EtatRelance,
}

impl EtatPont {
    /// Lance le pont fichiers.
    ///
    /// 🔴 **Contrairement à `EtatCapteur::demarrer`, un échec ici n'est PAS
    /// fatal, et c'est la seule décision de ce module.**
    ///
    /// Le capteur sert le média à tout enfant qui se rattache : sans lui,
    /// chaque enfant capturerait dans le vide, et le superviseur n'aurait plus
    /// rien à superviser — d'où son démarrage fatal, au même titre qu'un
    /// pilote ou qu'un hook qui ne s'ouvre pas. Le pont, lui, ne sert que le
    /// lecteur de fichiers. **Le cadrage §4, principe 4, exige qu'une panne de
    /// ce côté ne touche JAMAIS le flux vidéo** ; rendre son démarrage fatal
    /// ferait exactement l'inverse — une VM sans ProjFS, ou un pont qui refuse
    /// de démarrer pour n'importe quelle autre raison, y perdrait la capture
    /// entière.
    ///
    /// Ne rend donc pas de `Result` : il n'y a aucune erreur à propager. Un
    /// échec est journalisé et **retenté indéfiniment** par `surveiller`,
    /// exactement comme une relance — le pont peut donc apparaître en cours de
    /// session, sans redémarrage du superviseur.
    pub(super) fn demarrer(lanceur: &LanceurDeProcessus) -> Self {
        let mut etat = Self {
            pid: None,
            // Reculée d'un espacement plancher : la toute première tentative
            // doit avoir lieu MAINTENANT, pas dans 500 ms. Sans ce recul,
            // `tenter` rendrait la main sans rien faire et le pont ne
            // démarrerait qu'au tour de boucle suivant l'espacement — ce qui
            // passerait inaperçu, le pont n'étant pas sur le chemin critique.
            //
            // 🔴 **C'EST LE SECOND RÔLE DE `ESPACEMENT_PLANCHER_MS`, ET IL
            // N'ÉTAIT NOMMÉ NULLE PART AVANT LE ROUND DE CORRECTION 4** —
            // relevé par la revue : la doc de la constante en énumérait deux
            // quand le code en servait trois. Ce n'est pas une cadence, c'est
            // une AMORCE, et elle lie les deux fichiers : relever la
            // constante retarderait d'autant le démarrage du pont, ce qui ne
            // se lit pas depuis `relance_pont.rs`. La constante le dit
            // désormais de son côté, et ⚠️ elle est **soudée** à
            // `plateforme::repli::REPLI_MIN_MS` par le test
            // `le_premier_espacement_egale_le_plancher` : elle ne peut pas
            // être relevée seule.
            derniere_tentative: std::time::Instant::now()
                - std::time::Duration::from_millis(crate::relance_pont::ESPACEMENT_PLANCHER_MS),
            relance: EtatRelance::neuve(),
        };
        etat.tenter(lanceur, "lancement initial du pont fichiers échoué");
        etat
    }

    /// Relance le pont s'il est mort, espacé selon
    /// `EtatRelance::espacement_ms` (`plateforme::repli::delai_de_repli`).
    ///
    /// **Ne ferme jamais aucune fenêtre, et ne touche à rien d'autre.** Une
    /// panne du pont est sans effet sur les sessions vidéo : c'est tout
    /// l'intérêt de l'avoir mis dans son propre processus, et y ajouter le
    /// moindre effet de bord sur la table ou sur les enfants annulerait cette
    /// propriété.
    pub(super) fn surveiller(&mut self, lanceur: &LanceurDeProcessus) {
        // 🔴 TROIS BRANCHES DEPUIS LE ROUND DE CORRECTION 4, ET LES DEUX
        // DÉCISIONS DE `EtatRelance` NE PEUVENT PLUS SE CROISER : `stable`
        // (seuil LONG, question de TRACE) ne se pose que sur un pont VIVANT ;
        // `reinitialiser_le_repli` (question de CADENCE) ne se pose que sur
        // une MORT, et sur son ISSUE — plus jamais sur une durée de vie.
        //
        // La revue du round 4 l'a mesuré : réarmer sur « vivant depuis
        // 500 ms » faisait retomber `tentative` à zéro après CHAQUE
        // lancement, et le repli ne pouvait plus croître pour tout mode de
        // panne où le pont vit entre ~0,5 s et ~35 s — jusqu'à 100
        // connexions `/signal` par minute contre un budget PARTAGÉ de 120,
        // sans qu'aucun `retryApresS` n'ait à intervenir. Une durée ne
        // pouvait pas trancher : une session SAINE qui se termine occupe le
        // même intervalle qu'un pont REFUSÉ qui a dormi. L'issue, elle,
        // tranche — voir la doc de tête de `crate::relance_pont`.
        match lanceur.etat_du_pont() {
            EtatObserve::Vivant => {
                let ecoule_ms = self.derniere_tentative.elapsed().as_millis() as u64;
                if self.relance.stable(ecoule_ms) {
                    tracing::info!(pid = self.pid, "pont fichiers de nouveau stable");
                }
                return;
            }
            // Une sortie PROPRE (`pont::executer` rend `Ok(())`) prouve
            // qu'une panne passée est résolue : le repli repart du plancher.
            // Un `bail!` — dont le refus du relais, honoré puis propagé —
            // ne prouve rien, et le repli continue de croître.
            EtatObserve::Mort(issue) => self.relance.reinitialiser_le_repli(issue),
            // ⚠️ AUCUN RÉARMEMENT ICI, ET C'EST LE POINT : `Absent` couvre
            // un `spawn` en échec répété, exactement le cas que le critique
            // ③ du round 1 a mesuré, dont le repli doit croître.
            EtatObserve::Absent => {}
        }
        self.tenter(lanceur, "relance du pont fichiers échouée");
    }

    /// Une tentative de lancement, espacée et journalisée une fois par cycle.
    ///
    /// Partagée entre `demarrer` et `surveiller` : les deux chemins sont le
    /// même geste, et les écrire deux fois les ferait diverger — c'est
    /// précisément ce qui distingue ce module de son jumeau, où `demarrer` ne
    /// retente rien parce qu'il est fatal.
    fn tenter(&mut self, lanceur: &LanceurDeProcessus, quoi: &str) {
        // 🔴 L'ESPACEMENT VIENT DE `EtatRelance::doit_relancer`, PAS D'UNE
        // CONSTANTE FIXE — c'est tout le correctif du critique ③ de D1.
        // `espacement_ms()` vaut exactement `ESPACEMENT_PLANCHER_MS` (500 ms)
        // à la première tentative d'un cycle : un pont qui ne meurt jamais
        // deux fois de suite n'observe donc AUCUN changement de comportement.
        // Ce n'est qu'à partir de la deuxième tentative consécutive sans
        // stabilité que l'espacement s'allonge.
        let ecoule_ms = self.derniere_tentative.elapsed().as_millis() as u64;
        if !self.relance.doit_relancer(ecoule_ms) {
            return;
        }
        let espacement_ms = self.relance.espacement_ms();
        self.derniere_tentative = std::time::Instant::now();
        // 🔴 ENREGISTRÉ ICI, PAS SEULEMENT DANS LA BRANCHE `Err` PLUS BAS —
        // et c'est le point qui rend ce correctif correct sur le cas
        // RÉELLEMENT mesuré : `lancer_pont()` RÉUSSIT (`Ok`), le processus se
        // lance, se fait refuser par `/signal`, et meurt après avoir honoré
        // le délai suggéré. Ce cycle-là ne passe JAMAIS par la branche `Err`
        // de `lancer_pont()` — c'est un `Command::spawn` parfaitement
        // réussi —, et un compteur incrémenté seulement sur `Err` resterait
        // bloqué à `tentative = 0`, donc à un espacement de 500 ms, dans
        // EXACTEMENT le cas que ce lot doit corriger.
        let premier_du_cycle = self.relance.tentative_lancee();
        match lanceur.lancer_pont() {
            Ok(nouveau) => {
                if premier_du_cycle {
                    tracing::warn!(
                        pid_mort = self.pid,
                        pid_neuf = nouveau,
                        tentative = self.relance.tentative(),
                        espacement_ms,
                        "pont fichiers lancé ou relancé (tentatives suivantes silencieuses \
                         tant que le cycle se répète)"
                    );
                }
                self.pid = Some(nouveau);
            }
            Err(erreur) => {
                // `self.pid` n'est PAS mis à jour : il reste la dernière
                // valeur connue, pour que la prochaine relance réussie
                // journalise un « mort » exact — `lancer_pont` ne rend `Ok`
                // qu'en atomique, un échec n'a jamais fait vivre de processus.
                //
                // ⚠️ `warn!` et non `error!` : le pont est un service
                // FACULTATIF du produit, et un `error!` ferait qu'une VM sans
                // ProjFS — celle d'avant F0 — remplirait le journal d'erreurs
                // pour une capture qui, elle, fonctionne parfaitement. C'est
                // le principe 4 du cadrage jusque dans le niveau de trace.
                if premier_du_cycle {
                    tracing::warn!(
                        %erreur,
                        tentative = self.relance.tentative(),
                        espacement_ms,
                        "{quoi} — la capture n'est PAS affectée ; retenté indéfiniment, à un \
                         espacement CROISSANT (plateforme::repli::delai_de_repli), et \
                         silencieusement tant que l'échec se répète"
                    );
                }
            }
        }
    }
}
