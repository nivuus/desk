//! La décision PURE de reconnexion de la **session de contrôle** du
//! superviseur : quand retenter, et quand le repli exponentiel a le droit de
//! repartir du plancher.
//!
//! 🔴 **LE DÉFAUT QU'ELLE CORRIGE, MESURÉ EN PRODUCTION.** Avant ce lot,
//! `superviseur/signalisation.rs` ouvrait son socket UNE FOIS et ne le
//! rouvrait JAMAIS. Après un redémarrage du service `desk-plateforme`, le
//! superviseur journalisait `émission vers la shell échouée
//! erreur=Trying to work with closed connection`, puis `connexion de contrôle
//! au signaling perdue`, et **restait ainsi jusqu'à sa propre mort** : aucune
//! fenêtre ne pouvait plus être annoncée ni RÉANNONCÉE, ce qui rendait
//! inopérante la correction du lot 17 (`pair-present`), qui a besoin de ce
//! socket pour être délivrée. Le seul remède connu était de relancer
//! l'agent — geste qui, depuis la règle d'appartenance du lot 32I,
//! **orpheline toutes les fenêtres du propriétaire**. Legs n°1 du lot 17,
//! fermé ici.
//!
//! 🔴 **CE N'EST PAS LE CAS DU PONT, ET LES CONFONDRE MÈNERAIT À RECOPIER LE
//! MAUVAIS REMÈDE.** `boucle/surveillance_pont.rs` relance un **PROCESSUS**
//! qu'un tiers (la boucle du superviseur) OBSERVE de l'extérieur : le
//! processus meurt, `LanceurDeProcessus::etat_du_pont` le constate au tour
//! suivant, et l'issue de sortie (`IssueDeSortie`) dit si la panne est
//! résolue. Ici il s'agit d'un **SOCKET dans un processus vivant** : personne
//! ne l'observe de l'extérieur, il n'y a **aucun code de sortie à lire**, et
//! la boucle de reprise doit donc vivre **dans la tâche qui possède le
//! socket**. C'est pourquoi ce module n'est PAS [`crate::relance_pont::
//! EtatRelance`] et n'en dérive pas :
//!
//! ① `EtatRelance::reinitialiser_le_repli` prend une [`crate::relance_pont::
//!    IssueDeSortie`] — un code de sortie de processus, qui n'existe pas ici ;
//! ② `EtatRelance::doit_relancer(ecoule_ms)` suppose un appelant qui SONDE à
//!    chaque tour d'horloge ; cette boucle-ci **dort** le délai voulu, elle ne
//!    sonde rien ;
//! ③ `EtatRelance::stable` est gardée par `cycle_signale`, un booléen de
//!    TRACE dont ce module n'a pas besoin — il journalise **chaque**
//!    tentative, voir plus bas.
//!
//! **Ce qui est RÉUTILISÉ, et non recopié**, c'est la primitive que les deux
//! partagent déjà avec le canal `/agent` : [`crate::plateforme::repli::
//! delai_de_repli`] et son plafond `REPLI_MAX_MS`. Le précédent le plus
//! proche du mécanisme entier n'est d'ailleurs pas le pont mais
//! **`plateforme.rs::ouvrir`** : ce même agent sait DÉJÀ rouvrir un socket
//! perdu — le canal `/agent` — avec ce même repli. La session de contrôle
//! était le seul de ses trois sockets à ne pas savoir le faire.
//!
//! 🔴 **CONVENTION DE MODULE (`CLAUDE.md`), APPLIQUÉE PLUTÔT QUE DEVINÉE.**
//! Ce fichier est un enfant ORDINAIRE de `superviseur.rs`, déclaré par un
//! `pub mod reprise_controle;` sans `#[path]`, et il n'a pas à se poser la
//! question du préfixe : cette question ne se pose QUE pour un module qu'on
//! extrait d'un parent `#[cfg(windows)]` et qui doit de ce fait devenir un
//! frère de premier niveau. `superviseur.rs`, lui, n'est PAS gaté (seuls
//! certains de ses enfants le sont), si bien qu'un enfant ordinaire compile
//! et se teste déjà sur l'hôte Linux — exactement comme ses voisins
//! `table.rs`, `fenetres.rs` et `reprise.rs`.
//!
//! ⚠️ **`reprise_controle` ET NON `reprise`** : `superviseur::reprise` existe
//! déjà (lot 32E) et désigne AUTRE CHOSE — le nombre de chances données à une
//! sortie virtuelle de s'attacher. Deux `reprise` dans le même graphe de
//! modules n'attendraient qu'un lecteur pressé pour se confondre, exactement
//! l'argument qui a fait nommer `surveillance_pont` et `relance_pont`.

use crate::plateforme::repli::{delai_de_repli, REPLI_MAX_MS};

/// Durée de vie au-delà de laquelle une connexion est réputée avoir
/// **SERVI**, et où le repli exponentiel repart donc du plancher.
///
/// 🔴 **STRICTEMENT SUPÉRIEUR À `REPLI_MAX_MS`, ET C'EST CE QUI REND LE
/// RÉARMEMENT INATTEIGNABLE PAR UN REFUS.** Un refus de poignée de main de la
/// plateforme (`signaling/relais.ts` : jeton absent, expiré, rôle déjà
/// occupé, budget de volume épuisé) arrive en **millisecondes** — le relais
/// envoie `{"type":"error",…}` puis `close(1008)` dans le même geste. Une
/// connexion refusée ne peut donc structurellement pas atteindre ce seuil, et
/// le repli continue de croître jusqu'à son plafond au lieu de marteler un
/// service qui vient de dire non. C'est la leçon que `relance_pont.rs` a
/// payée en cinq rounds de correction : un seuil COURT (500 ms) rendait le
/// réarmement systématique et le repli **structurellement incapable de
/// croître**.
///
/// ⚠️ **NON CALIBRÉ** — même réserve que `REPLI_MIN_MS` et `REPLI_MAX_MS` :
/// aucune mesure de ce dépôt ne dit combien de temps dure une coupure réelle.
/// La marge de 5 s au-dessus du plafond couvre le temps qu'il faut à un refus
/// pour ARRIVER (poignée de main WebSocket, aller-retour, lecture du
/// message), non mesuré, choisi large plutôt que juste. La valeur suit celle
/// de `relance_pont::SEUIL_STABILITE_MS` par la même arithmétique — sans
/// l'importer : ce seuil-là parle d'un PROCESSUS qui dort avant de mourir,
/// celui-ci d'un SOCKET refusé, et les souder ferait qu'un recalibrage de
/// l'un déplacerait l'autre sans qu'aucune mesure ne le demande.
pub const SEUIL_CONNEXION_UTILE_MS: u64 = REPLI_MAX_MS + 5_000;

/// L'état, PUR, de la reprise de la session de contrôle.
///
/// Ne connaît ni socket, ni horloge murale, ni tokio : elle reçoit des
/// millisecondes ÉCOULÉES et rend des millisecondes À ATTENDRE. C'est ce qui
/// la rend éprouvable sur l'hôte Linux, là où `signalisation.rs` vit derrière
/// un `#![cfg(windows)]` que `cargo test --workspace` ne compile jamais.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Reprise {
    /// Tentatives de reconnexion CONSÉCUTIVES depuis le dernier réarmement.
    tentative: u32,
}

impl Reprise {
    pub fn neuve() -> Self {
        Self { tentative: 0 }
    }

    /// Le nombre de tentatives consécutives — pour l'annexer aux traces de
    /// l'appelant, jamais pour décider quoi que ce soit ici.
    pub fn tentative(&self) -> u32 {
        self.tentative
    }

    /// Combien de millisecondes dormir AVANT la prochaine tentative — pur
    /// ré-emballage de `delai_de_repli(self.tentative)`.
    pub fn delai_ms(&self) -> u64 {
        delai_de_repli(self.tentative)
    }

    /// Enregistre une tentative RÉELLEMENT lancée (une connexion WebSocket
    /// effectivement tentée, qu'elle aboutisse ou non).
    ///
    /// ⚠️ Compter ici et non sur l'échec : une poignée de main ACCEPTÉE puis
    /// refusée trois millisecondes plus tard par la garde est un `Ok` côté
    /// TCP, et un compteur incrémenté seulement sur `Err` resterait bloqué à
    /// zéro — donc à un espacement de 500 ms — dans EXACTEMENT le cas qui
    /// martèle. C'est le défaut que `surveillance_pont.rs` a payé sur son
    /// `lancer_pont()` réussi.
    pub fn tentative_lancee(&mut self) {
        self.tentative = self.tentative.saturating_add(1);
    }

    /// Une connexion vient de se terminer après avoir vécu `vecu_ms`.
    ///
    /// Réarme le repli — remet le compteur à zéro — **si et seulement si**
    /// cette connexion a vécu au moins [`SEUIL_CONNEXION_UTILE_MS`], c'est-à-
    /// dire si elle a réellement servi. Rend `true` dans ce cas, pour que
    /// l'appelant puisse le dire dans sa trace.
    pub fn connexion_terminee(&mut self, vecu_ms: u64) -> bool {
        if vecu_ms < SEUIL_CONNEXION_UTILE_MS {
            return false;
        }
        self.tentative = 0;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plateforme::repli::REPLI_MIN_MS;

    #[test]
    fn la_premiere_reconnexion_attend_le_plancher() {
        // Une coupure d'une seconde ne doit pas coûter trente secondes de
        // bureau muet : la toute première reprise part du plancher partagé
        // avec le canal `/agent`.
        assert_eq!(Reprise::neuve().delai_ms(), REPLI_MIN_MS);
        assert_eq!(Reprise::neuve().tentative(), 0);
    }

    #[test]
    fn le_delai_croit_puis_plafonne() {
        let mut reprise = Reprise::neuve();
        let mut precedent = reprise.delai_ms();
        reprise.tentative_lancee();
        assert!(
            reprise.delai_ms() > precedent,
            "le délai doit croître : {} n'est pas > {precedent}",
            reprise.delai_ms()
        );
        for _ in 0..40 {
            precedent = reprise.delai_ms();
            reprise.tentative_lancee();
            assert!(
                reprise.delai_ms() >= precedent,
                "le délai ne doit jamais reculer sans réarmement"
            );
            assert!(
                reprise.delai_ms() <= REPLI_MAX_MS,
                "le délai {} dépasse le plafond {REPLI_MAX_MS}",
                reprise.delai_ms()
            );
        }
        assert_eq!(
            reprise.delai_ms(),
            REPLI_MAX_MS,
            "le plafond doit être ATTEINT, pas seulement respecté"
        );
    }

    /// 🔴 **LE TEST QUI REND LE PRÉCÉDENT STRUCTUREL.** Sans cette inégalité,
    /// un refus pourrait atteindre le seuil de réarmement et le repli
    /// deviendrait incapable de croître — le défaut exact que
    /// `relance_pont.rs` a payé à son round de correction 3.
    #[test]
    fn le_seuil_de_connexion_utile_reste_strictement_au_dessus_du_plafond_de_repli() {
        assert!(
            SEUIL_CONNEXION_UTILE_MS > REPLI_MAX_MS,
            "SEUIL_CONNEXION_UTILE_MS = {SEUIL_CONNEXION_UTILE_MS} doit être > REPLI_MAX_MS = {REPLI_MAX_MS}"
        );
    }

    /// Le scénario du refus en boucle : la plateforme accepte le TCP puis
    /// ferme aussitôt (jeton expiré, rôle déjà occupé, budget épuisé). Le
    /// repli doit croître jusqu'à son plafond, jamais repartir du plancher.
    #[test]
    fn une_connexion_refusee_ne_rearme_jamais_le_repli() {
        // 🔴 **LA DURÉE ÉPROUVÉE EST `REPLI_MAX_MS`, PAS LES 3 ms D'UN REFUS
        // RÉEL, ET C'EST UNE CORRECTION DE CE TEST LUI-MÊME.** Écrit d'abord
        // avec `3` — l'ordre de grandeur mesuré d'un `{"type":"error"}` suivi
        // d'un `close(1008)` —, il restait **VERT** sous la mutation qui fait
        // retomber `SEUIL_CONNEXION_UTILE_MS` à 500 ms, c'est-à-dire sous le
        // défaut exact que `relance_pont.rs` a payé à son round 3 : la seule
        // rouge venait alors du test d'invariant voisin. Une rouge restée
        // verte se DIAGNOSTIQUE, elle ne se classe pas.
        //
        // La durée retenue vient du PRODUIT — `REPLI_MAX_MS`, le plafond du
        // repli — et jamais d'un calcul sur ce qu'on juge : c'est le pire cas
        // qu'un épisode de refus puisse occuper, puisque rien dans cette
        // boucle n'attend plus longtemps que ce plafond avant de retenter.
        // Aucune vie de cette longueur ou moindre ne doit réarmer.
        let mut reprise = Reprise::neuve();
        for tour in 0..20 {
            reprise.tentative_lancee();
            assert!(
                !reprise.connexion_terminee(REPLI_MAX_MS),
                "un refus au tour {tour}, même long de REPLI_MAX_MS, ne doit JAMAIS réarmer le repli"
            );
        }
        assert_eq!(reprise.delai_ms(), REPLI_MAX_MS);
        assert_eq!(reprise.tentative(), 20);
        // …et le cas réellement mesuré reste couvert, lui aussi.
        assert!(!reprise.connexion_terminee(3));
    }

    /// Le symétrique : une connexion qui a réellement servi (le cas nominal —
    /// une session de contrôle vit des heures) rend son plancher au repli,
    /// pour que la coupure SUIVANTE ne reprenne pas au plafond d'une panne
    /// déjà résolue.
    #[test]
    fn une_connexion_qui_a_servi_rearme_le_repli() {
        let mut reprise = Reprise::neuve();
        for _ in 0..10 {
            reprise.tentative_lancee();
        }
        assert_eq!(reprise.delai_ms(), REPLI_MAX_MS);
        assert!(reprise.connexion_terminee(SEUIL_CONNEXION_UTILE_MS));
        assert_eq!(reprise.tentative(), 0);
        assert_eq!(reprise.delai_ms(), REPLI_MIN_MS);
    }

    /// La frontière, des deux côtés : une milliseconde de moins ne réarme
    /// pas, le seuil exact réarme. Sans ce test, une comparaison `>` au lieu
    /// de `>=` passerait inaperçue.
    #[test]
    fn la_frontiere_du_seuil_est_eprouvee_des_deux_cotes() {
        let mut juste_en_dessous = Reprise::neuve();
        juste_en_dessous.tentative_lancee();
        assert!(!juste_en_dessous.connexion_terminee(SEUIL_CONNEXION_UTILE_MS - 1));
        assert_eq!(juste_en_dessous.tentative(), 1);

        let mut au_seuil = Reprise::neuve();
        au_seuil.tentative_lancee();
        assert!(au_seuil.connexion_terminee(SEUIL_CONNEXION_UTILE_MS));
        assert_eq!(au_seuil.tentative(), 0);
    }

    /// Un agent qui vit des semaines derrière une plateforme morte ne doit ni
    /// paniquer sur un débordement, ni voir son délai retomber par
    /// enroulement. `saturating_add` et `delai_de_repli` couvrent les deux ;
    /// ce test le fige.
    #[test]
    fn le_compteur_ne_deborde_jamais() {
        let mut reprise = Reprise { tentative: u32::MAX - 1 };
        reprise.tentative_lancee();
        reprise.tentative_lancee();
        assert_eq!(reprise.tentative(), u32::MAX);
        assert_eq!(reprise.delai_ms(), REPLI_MAX_MS);
    }
}
