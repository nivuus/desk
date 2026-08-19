//! Le PLAFOND de dissimulation : au-delà de tant de millisecondes dissimulées
//! d'affilée, on rend du SILENCE plutôt qu'une trame extrapolée.
//!
//! **PUR, aucun `cfg`, aucun décodeur.** La règle se teste sur l'hôte sans
//! libopus ni périphérique : elle ne connaît que des durées.
//!
//! # Pourquoi ce plafond existe — un défaut MESURÉ, pas une réserve
//!
//! La recette E1 (tâche 14, chantier E) a relevé, pendant **60 s de silence**
//! du navigateur — son DTX nominal, `packetsSent` figé —, une trace « micro
//! mesuré » qui portait seconde après seconde :
//!
//! ```text
//! plc = 50/s pendant 60 s        famines = 50/s
//! crete       0,53 à 0,67        (pas un silence)
//! frequence_hz  errant entre 308 et 393 Hz
//! ```
//!
//! Autrement dit : **la dissimulation fabriquait un bourdon continu.** Au bloc
//! E2 ce bourdon sortirait sur le câble virtuel, donc dans l'application
//! Windows — un utilisateur qui se tait ferait entendre un bourdonnement.
//! Pièce : `docs/superpowers/plans/journaux-micro/agent-silence-520-plat.log`.
//!
//! # Ce n'est PAS un défaut de libopus, et c'est ce qui rend le plafond nôtre
//!
//! Lu dans la source vendorée par `audiopus_sys` 0.2.2, et non de mémoire :
//!
//! - `opus/celt/celt_decoder.c:537` —
//!   `noise_based = loss_count >= 5 || start != 0 || st->skip_plc;` : dès la
//!   **6ᵉ perte consécutive**, CELT abandonne l'extrapolation par le pitch et
//!   bascule sur du **bruit**. Passé ce point, ce qu'on rend n'est plus une
//!   extrapolation de la voix du locuteur, c'est du bruit synthétisé.
//! - `celt_decoder.c:562` et `:566` — l'énergie décroît de 0,5 dB par trame
//!   perdue, **mais bornée par le bas** :
//!   `MAX16(backgroundLogE[...], oldBandE[...] - decay)`. Elle converge donc
//!   vers le **plancher de bruit de fond et s'y maintient**. C'est de la
//!   génération de bruit de confort, et c'est délibéré : **libopus ne s'arrête
//!   JAMAIS de lui-même.** C'est exactement ce que mesure la crête de 0,6 tenue
//!   soixante secondes.
//! - `celt_decoder.c:1149-1152` — au-delà de la **10ᵉ** perte consécutive,
//!   libopus change de régime et son commentaire nomme la situation : « *when
//!   we're in DTX* ». La bibliothèque cesse elle-même de traiter le trou comme
//!   une perte.
//! - `opus/silk/PLC.h:36` et `silk/PLC.c:250-254` — côté SILK, l'atténuation
//!   par trame est **saturée** dès la 2ᵉ perte (`NB_ATT = 2`,
//!   `silk_min_int(NB_ATT - 1, lossCnt)`).
//!
//! **Borner la durée dissimulée est donc à NOUS**, et à personne d'autre.
//!
//! # Perte réseau et silence de l'émetteur : la même borne, faute de pouvoir
//! # les distinguer
//!
//! Deux causes produisent la même trame manquante, et elles n'ont pas le même
//! sens :
//!
//! - une **perte réseau** est courte (quelques trames), et la dissimulation
//!   existe précisément pour elle ;
//! - un **silence de l'émetteur** (le DTX de Chrome) est potentiellement
//!   INFINI : le navigateur cesse simplement d'émettre.
//!
//! **Le code ne peut pas les distinguer, et c'est structurel.** Ce qui atteint
//! `TamponGigue` est l'absence d'un paquet RTP ; rien, dans ce que
//! `transport/piste_micro.rs` reçoit, ne dit « je me tais » — un émetteur en
//! DTX n'envoie pas un message, il n'envoie *rien*, et l'absence est la même
//! des deux côtés. Un marqueur RTP de reprise de parole, à supposer qu'on le
//! lise, n'arriverait qu'à la FIN du silence, jamais pendant.
//!
//! **La même borne sert donc les deux, et c'est le bon arbitrage** : sous le
//! plafond on dissimule, ce dont la perte réseau a besoin ; au-delà on se tait,
//! ce que le silence de l'émetteur exige. La cause n'a pas à être connue —
//! seule compte la durée pendant laquelle on extrapole sans rien pour
//! s'appuyer, et elle est la même quelle que soit la raison.

use std::time::Duration;

/// Durée maximale de dissimulation CONSÉCUTIVE. Au-delà, le puits rend du
/// silence jusqu'à ce qu'une vraie trame revienne.
///
/// ⚠️ **NON CALIBRÉE.** Aucun jugement d'écoute n'a été porté sur elle, et
/// elle rejoint en cela `CIBLE`, `BPP_MIN`, `FACTEUR_FOCUS`,
/// `PART_DORMANTE_BPS`, `TAILLE_MAX_SORTIE` et `AUDIO_PERIPHERIQUE` : ce dépôt
/// nomme ses constantes non calibrées plutôt que de laisser croire à un
/// réglage mesuré.
///
/// **Ce sur quoi l'ORDRE DE GRANDEUR s'appuie**, faute de calibration : le
/// repère `st->loss_count < 10` de `celt_decoder.c:1152`, où libopus nomme
/// lui-même la situation « DTX ». Chrome émet des trames de 20 ms ; dix
/// d'entre elles font **200 ms**, et c'est la valeur retenue. Sur des trames de
/// 10 ms — la cadence du chantier A — le même plafond en laisse passer vingt :
/// c'est pourquoi la constante est une **DURÉE et non un compte de trames**,
/// la durée d'une trame étant lue du paquet et jamais supposée (spec §7).
///
/// ⚠️ **Elle vaut le même nombre que `micro::PLAFOND`, et c'est une
/// COÏNCIDENCE, pas une dérivation.** Les deux répondent à des questions
/// différentes — combien d'audio on accepte d'accumuler, combien de temps on
/// accepte d'extrapoler — et **elles ne doivent pas être faites suivre l'une
/// l'autre**. Le dépôt a déjà écrit cette règle pour `PERIODE_STYLE` et
/// `PERIODE_REARBITRAGE`.
///
/// **La direction qu'une calibration future devrait explorer est le
/// RESSERREMENT** : le repère de `celt_decoder.c:537` — la 6ᵉ perte, soit
/// 120 ms à 20 ms par trame — est celui où la dissimulation cesse d'être une
/// extrapolation du signal pour devenir du bruit. Se taire dès là est
/// défendable ; c'est un jugement d'écoute qui doit trancher, pas un raisonnement.
pub const PLAFOND_DISSIMULATION: Duration = Duration::from_millis(200);

/// Ce qui reste de budget de dissimulation avant qu'on ne se taise.
///
/// **Un compteur de DURÉE, remis à zéro par toute vraie trame.** C'est la
/// règle entière, et elle tient en deux méthodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetDissimulation {
    plafond: Duration,
    consecutif: Duration,
}

impl BudgetDissimulation {
    pub fn new(plafond: Duration) -> Self {
        Self { plafond, consecutif: Duration::ZERO }
    }

    /// Peut-on encore dissimuler `duree` ? Rend `false` quand le plafond est
    /// atteint — à l'appelant de rendre du silence.
    ///
    /// La comparaison est faite **avant** l'ajout : le budget autorise donc
    /// exactement `plafond` de dissimulation cumulée, jamais une trame de plus.
    ///
    /// ⚠️ **Une durée NULLE est refusée**, et ce n'est pas un cas d'école : une
    /// durée nulle acceptée n'avancerait jamais le compteur et ce budget ne
    /// pourrait plus jamais atteindre son plafond — précisément le défaut sans
    /// borne qu'il existe pour fermer.
    pub fn consommer(&mut self, duree: Duration) -> bool {
        if duree.is_zero() || self.consecutif >= self.plafond {
            return false;
        }
        self.consecutif = self.consecutif.saturating_add(duree);
        true
    }

    /// Une vraie trame a été rendue : le budget repart entier.
    pub fn trame_reelle(&mut self) {
        self.consecutif = Duration::ZERO;
    }

    /// Durée dissimulée d'affilée depuis la dernière vraie trame.
    pub fn consecutif(&self) -> Duration {
        self.consecutif
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRAME: Duration = Duration::from_millis(20);

    /// Le cas NOMINAL, et il ne doit pas changer : une perte réseau courte
    /// reste dissimulée exactement comme avant le plafond.
    #[test]
    fn une_perte_courte_est_dissimulee_sans_reserve() {
        let mut b = BudgetDissimulation::new(PLAFOND_DISSIMULATION);
        for i in 0..3 {
            assert!(b.consommer(TRAME), "la {}ᵉ trame perdue a été refusée", i + 1);
        }
        assert_eq!(b.consecutif(), Duration::from_millis(60));
    }

    /// Le plafond mord, et il mord EXACTEMENT à sa valeur : dix trames de
    /// 20 ms font les 200 ms autorisées, la onzième est refusée.
    #[test]
    fn au_dela_du_plafond_la_dissimulation_est_refusee() {
        let mut b = BudgetDissimulation::new(PLAFOND_DISSIMULATION);
        for i in 0..10 {
            assert!(b.consommer(TRAME), "la {}ᵉ trame a été refusée trop tôt", i + 1);
        }
        assert_eq!(b.consecutif(), PLAFOND_DISSIMULATION);
        assert!(!b.consommer(TRAME), "la 11ᵉ trame a été dissimulée : le plafond ne mord pas");
        // Et le refus est DURABLE : il ne se lève pas de lui-même au tour
        // suivant. Sans cette ligne, un plafond qui n'interdirait qu'une trame
        // sur deux passerait l'assertion ci-dessus.
        for _ in 0..100 {
            assert!(!b.consommer(TRAME), "le refus s'est levé tout seul");
        }
    }

    /// ⚠️ **LE test de la remise à zéro.** Sans elle, un plafond atteint une
    /// fois condamnerait la session au silence pour toujours : le puits ne
    /// dissimulerait plus jamais la moindre perte réseau.
    #[test]
    fn une_vraie_trame_rend_le_budget_entier() {
        let mut b = BudgetDissimulation::new(PLAFOND_DISSIMULATION);
        while b.consommer(TRAME) {}
        assert!(!b.consommer(TRAME));

        b.trame_reelle();
        assert_eq!(b.consecutif(), Duration::ZERO);
        assert!(
            b.consommer(TRAME),
            "après le retour d'une vraie trame, le budget reste fermé"
        );
    }

    /// La borne est une DURÉE, pas un compte de trames : à 10 ms par trame il
    /// en passe deux fois plus qu'à 20 ms, pour la même durée dissimulée.
    ///
    /// C'est ce qui rend la constante juste quelle que soit la cadence du pair
    /// — Chrome émet du 20 ms, le chantier A du 10 ms, et rien n'oblige un
    /// pair à s'y tenir (spec §7).
    #[test]
    fn le_plafond_est_une_duree_et_non_un_compte_de_trames() {
        let compte = |trame: Duration| {
            let mut b = BudgetDissimulation::new(PLAFOND_DISSIMULATION);
            let mut n = 0;
            while b.consommer(trame) {
                n += 1;
            }
            (n, b.consecutif())
        };
        assert_eq!(compte(Duration::from_millis(20)), (10, PLAFOND_DISSIMULATION));
        assert_eq!(compte(Duration::from_millis(10)), (20, PLAFOND_DISSIMULATION));
    }

    /// Une durée nulle ne peut pas faire tourner le budget sans fin — ce
    /// serait le défaut sans borne rejoué à l'intérieur de son propre remède.
    #[test]
    fn une_duree_nulle_est_refusee_plutot_que_de_ne_jamais_epuiser_le_budget() {
        let mut b = BudgetDissimulation::new(PLAFOND_DISSIMULATION);
        assert!(!b.consommer(Duration::ZERO));
        assert_eq!(b.consecutif(), Duration::ZERO);
    }
}
