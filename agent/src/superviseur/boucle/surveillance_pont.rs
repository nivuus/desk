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
//! `une_session`) — plutôt que d'en écrire un second. `EtatPont` compte
//! désormais ses tentatives CONSÉCUTIVES sans stabilité observée
//! (`tentative`), et l'espacement de la PROCHAINE relance en découle : 500 ms
//! à la première, doublant jusqu'au plafond de 30 s. Un pont qui meurt en
//! boucle finit donc par ne plus revenir qu'à 2 tentatives/minute — largement
//! sous le budget partagé — plutôt qu'à 120.

use super::*;

/// Espacement PLANCHER entre deux tentatives de relance du pont, et
/// SEUIL de stabilité (voir `surveiller`).
///
/// 🔴 **DEPUIS LE CORRECTIF DU LEGS DES FREINS MANQUANTS, CE N'EST PLUS
/// L'ESPACEMENT RÉELLEMENT APPLIQUÉ** — celui-ci vient désormais de
/// `plateforme::repli::delai_de_repli(tentative)`, dont cette constante n'est
/// que le premier terme (`delai_de_repli(0) == 500 ms`, exactement cette
/// valeur — voir `repli.rs::REPLI_MIN_MS`, à laquelle elle est ÉGALE mais
/// pas COUPLÉE : rien n'empêche les deux de diverger un jour). Elle reste
/// employée pour DEUX choses : le recul de la toute première tentative dans
/// `demarrer`, et le seuil au-delà duquel un pont resté vivant est déclaré
/// STABLE dans `surveiller` — cette seconde durée n'a pas à croître avec le
/// nombre de tentatives passées, elle mesure juste « a-t-il tenu au moins
/// aussi longtemps que l'espacement minimal ».
///
/// Même raison, au mot près, que `PERIODE_RELANCE_CAPTEUR_MIN` pour son rôle
/// de plancher : sans lui, un pont qui meurt AUSSITÔT après avoir été relancé
/// ferait retenter un vrai `Command::spawn` à la cadence de la boucle
/// (~10 Hz).
///
/// ⚠️ **Constante PROPRE à ce module, et délibérément pas un emprunt à celle
/// du capteur** : les deux valent 500 ms aujourd'hui, et rien n'exige qu'elles
/// restent égales. Les coupler ferait qu'un réglage du capteur — mesuré, un
/// jour, sur le coût de sa relance — déplacerait en silence celui du pont, qui
/// n'a ni les mêmes ressources à reprendre ni le même coût de démarrage.
/// C'est le raisonnement que le sous-bloc D8 a tenu pour `PERIODE_STYLE`
/// contre `PERIODE_REARBITRAGE`.
///
/// ⚠️ **NON CALIBRÉE**, comme sa jumelle : reprise telle quelle, jamais
/// mesurée.
const PERIODE_RELANCE_PONT_MIN: std::time::Duration = std::time::Duration::from_millis(500);

/// Ce que la boucle retient du pont d'un tour à l'autre.
///
/// `pid` vaut `None` tant qu'aucun lancement n'a réussi — état qui n'existe
/// pas chez le capteur, dont le démarrage est fatal et qui a donc toujours un
/// PID dès sa construction.
pub(super) struct EtatPont {
    pid: Option<u32>,
    derniere_tentative: std::time::Instant,
    /// Vrai dès qu'un cycle de relance en cours a été signalé.
    ///
    /// Même motif que `EtatCapteur::cycle_signale` (correctif I3) et
    /// qu'`Enfant::etat_illisible_signale` : l'espacement des relances espace
    /// les `spawn`, **pas les LIGNES**. Un pont qui remeurt aussitôt après
    /// chaque relance produirait deux lignes par seconde, indéfiniment, sur un
    /// partage CIFS — environ 170 000 par jour. Signaler la première fois, se
    /// taire tant que la situation se répète, redevenir bruyant au premier
    /// retour à la normale.
    cycle_signale: bool,
    /// 🔴 **NEUF — CORRECTIF DU LEGS DES FREINS MANQUANTS.** Le nombre de
    /// tentatives de relance CONSÉCUTIVES depuis la dernière stabilité
    /// observée. `0` à la construction et après chaque retour à la stabilité
    /// (`surveiller`) ; incrémenté à CHAQUE tentative RÉELLEMENT lancée
    /// (`tenter`), qu'elle réussisse ou non À SE LANCER — voir sa doc : c'est
    /// le cas mesuré (`Command::spawn` réussit, le processus meurt aussitôt,
    /// refusé par `/signal`) qui doit faire croître ce compteur, pas
    /// seulement un `Command::spawn` en échec. C'est l'argument de
    /// `plateforme::repli::delai_de_repli`.
    tentative: u32,
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
            // Reculée d'une période : la toute première tentative doit avoir
            // lieu MAINTENANT, pas dans 500 ms. Sans ce recul, `tenter`
            // rendrait la main sans rien faire et le pont ne démarrerait qu'au
            // tour de boucle suivant la période — ce qui passerait inaperçu,
            // le pont n'étant pas sur le chemin critique.
            derniere_tentative: std::time::Instant::now() - PERIODE_RELANCE_PONT_MIN,
            cycle_signale: false,
            tentative: 0,
        };
        etat.tenter(lanceur, "lancement initial du pont fichiers échoué");
        etat
    }

    /// Relance le pont s'il est mort, espacé selon
    /// `plateforme::repli::delai_de_repli(self.tentative)`.
    ///
    /// **Ne ferme jamais aucune fenêtre, et ne touche à rien d'autre.** Une
    /// panne du pont est sans effet sur les sessions vidéo : c'est tout
    /// l'intérêt de l'avoir mis dans son propre processus, et y ajouter le
    /// moindre effet de bord sur la table ou sur les enfants annulerait cette
    /// propriété.
    pub(super) fn surveiller(&mut self, lanceur: &LanceurDeProcessus) {
        if lanceur.pont_vivant() {
            // Le pont a SURVÉCU au moins `PERIODE_RELANCE_PONT_MIN` depuis sa
            // dernière tentative : le cycle est rompu, une mort ultérieure
            // sera une information neuve. ⚠️ Ce seuil de STABILITÉ reste la
            // constante fixe, PAS l'espacement (potentiellement croissant) des
            // relances : confirmer qu'un pont tient est une question
            // indépendante de combien de fois il a fallu s'y reprendre avant.
            //
            // La condition de durée n'est pas décorative — même raisonnement
            // que chez le capteur : la boucle tourne à ~10 Hz, donc un pont
            // qui vivrait deux ou trois tours avant de mourir serait vu vivant
            // au moins une fois entre deux relances, ce qui réarmerait le
            // signalement à chaque cycle et rendrait la parade sans effet.
            // Exiger qu'il tienne au moins aussi longtemps que l'espacement
            // PLANCHER des relances est ce qui distingue « il repart » de
            // « il agonise en boucle ».
            if self.cycle_signale && self.derniere_tentative.elapsed() >= PERIODE_RELANCE_PONT_MIN {
                self.cycle_signale = false;
                // 🔴 REMISE À ZÉRO ICI, ET NULLE PART AILLEURS : c'est ce qui
                // fait qu'une panne FUTURE reparte de l'espacement minimal
                // (500 ms) plutôt que de rester bloquée au plafond de 30 s
                // atteint par une panne PASSÉE, déjà résolue. Même geste que
                // `plateforme.rs::une_session` : « la tentative repart de
                // ZÉRO après chaque [succès] … un agent connecté depuis trois
                // jours qui perd son réseau une seconde doit reprendre en une
                // demi-seconde, pas en trente ».
                self.tentative = 0;
                tracing::info!(pid = self.pid, "pont fichiers de nouveau stable");
            }
            return;
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
        // 🔴 L'ESPACEMENT VIENT DE `delai_de_repli`, PAS DE LA CONSTANTE FIXE
        // — c'est tout le correctif du critique ③. `delai_de_repli(0)` vaut
        // exactement `PERIODE_RELANCE_PONT_MIN` (500 ms) : un pont qui ne
        // meurt jamais deux fois de suite n'observe donc AUCUN changement de
        // comportement. Ce n'est qu'à partir de la deuxième tentative
        // consécutive sans stabilité que l'espacement s'allonge.
        let espacement =
            std::time::Duration::from_millis(crate::plateforme::repli::delai_de_repli(self.tentative));
        if self.derniere_tentative.elapsed() < espacement {
            return;
        }
        self.derniere_tentative = std::time::Instant::now();
        // 🔴 INCRÉMENTÉ ICI, PAS SEULEMENT DANS LA BRANCHE `Err` PLUS BAS —
        // et c'est le point qui rend ce correctif correct sur le cas
        // RÉELLEMENT mesuré : `lancer_pont()` RÉUSSIT (`Ok`), le processus se
        // lance, se fait refuser par `/signal`, et meurt en quelques
        // millisecondes. Ce cycle-là ne passe JAMAIS par la branche `Err` de
        // `lancer_pont()` — c'est un `Command::spawn` parfaitement réussi —,
        // et un compteur incrémenté seulement sur `Err` resterait bloqué à
        // `tentative = 0`, donc à un espacement de 500 ms, dans EXACTEMENT le
        // cas que ce lot doit corriger.
        self.tentative = self.tentative.saturating_add(1);
        // Lu AVANT la tentative, et armé quoi qu'il arrive : les deux issues
        // journalisent, et les deux doivent se taire au tour suivant si le
        // cycle se poursuit.
        let premier_du_cycle = !self.cycle_signale;
        self.cycle_signale = true;
        match lanceur.lancer_pont() {
            Ok(nouveau) => {
                if premier_du_cycle {
                    tracing::warn!(
                        pid_mort = self.pid,
                        pid_neuf = nouveau,
                        tentative = self.tentative,
                        espacement_ms = espacement.as_millis() as u64,
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
                        tentative = self.tentative,
                        espacement_ms = espacement.as_millis() as u64,
                        "{quoi} — la capture n'est PAS affectée ; retenté indéfiniment, à un \
                         espacement CROISSANT (plateforme::repli::delai_de_repli), et \
                         silencieusement tant que l'échec se répète"
                    );
                }
            }
        }
    }
}
