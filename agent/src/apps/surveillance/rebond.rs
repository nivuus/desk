//! L'anti-rebond : un train de notifications ne produit qu'UNE réconciliation.
//!
//! **PUR** : aucun `cfg`, aucune horloge propre, aucune entrée-sortie.
//! L'instant est un **paramètre**, exactement comme dans
//! `agent/src/pont/table.rs` et `agent/src/apps/installation/fenetre.rs` — et
//! c'est ce qui fait qu'**aucun test de ce fichier ne dort**.

use std::time::{Duration, Instant};

/// Toute notification repousse l'échéance de ce délai.
///
/// ⚠️ **NON CALIBRÉE.** Aucun jugement d'usage n'a été porté sur l'expérience
/// qu'elle produit. Ce qui la borne par le bas est mesuré, en revanche : le
/// sondage de `apps::boucle` a une granularité de **200 ms**
/// (`std::thread::sleep(reste.min(200 ms))`), donc un anti-rebond plus court
/// que cela ne serait pas observable — il serait absorbé par le sondage.
pub const DELAI_ANTI_REBOND: Duration = Duration::from_millis(750);

/// L'échéance ne peut jamais dépasser `première notification du train + ceci`.
///
/// 🔴 **CE PLAFOND N'EST PAS UN CONFORT, C'EST LE GARDE-FOU DU CRITÈRE ①** —
/// même rôle, et même précédent, que `REPLI_MAX_MS` dans
/// `agent/src/plateforme/repli.rs`. Sans lui, un flux CONTINU de notifications
/// ajourne la réconciliation **sans terme** : un installeur qui écrit pendant
/// deux minutes ne produirait aucun catalogue avant sa fin, et une racine
/// bruyante n'en produirait **jamais**.
///
/// 🔴 **QUATRE SECONDES, ET NON LES CINQ DE LA SPÉCIFICATION : c'est DÉRIVÉ, pas
/// recopié.** Le pire cas du critère ① — « un raccourci créé apparaît en moins
/// de cinq secondes » — vaut la somme de quatre termes :
///
/// ```text
/// pire cas = DELAI_ANTI_REBOND_MAX
///          + granularité du sondage    (200 ms, RELEVÉ dans apps/boucle.rs)
///          + coût d'une réconciliation (≈ 70 ms au repos, MESURÉ)
///          + 10 ms par icône neuve     (MESURÉ)
/// ```
///
/// À **5 s** : `5 000 + 200 + 70 + 10 = 5 280 ms` — **au-dessus** des cinq
/// secondes que le critère exige, dans le cas où le raccourci naît *à
/// l'intérieur* d'un flux continu. À **4 s** : `4 000 + 200 + 70 + 10 =
/// 4 280 ms`, et il reste **720 ms** de marge, soit **soixante-douze icônes
/// neuves** avant que le critère ne soit menacé.
///
/// ✅ **LA DÉRIVATION EST CORROBORÉE PAR LA MESURE, ET CE N'ÉTAIT PAS CHERCHÉ**
/// (critère ① de la recette, deux exécutions) : un raccourci créé apparaît en
/// **960 ms** puis **999 ms**, quand la somme prédit `750 + ≤200 + ~70` =
/// **960 à 1 020 ms**. Les deux mesures tombent dans l'intervalle.
///
/// 🔵 **DÉRIVÉE N'EST PAS CALIBRÉE, et les deux ne sont pas la même chose.**
/// Corroborer une somme n'est pas juger une expérience : personne n'a dit que
/// 960 ms « se sent bien ».
/// Personne n'a jugé que quatre secondes « se sentent bien » : elle rejoint
/// donc la liste des non calibrées de ce dépôt — `BPP_MIN`, `FACTEUR_FOCUS`,
/// `PART_DORMANTE_BPS`, `HYSTERESIS`, `TAILLE_MAX_SORTIE`,
/// `SEUIL_INJOIGNABLE_MS`, `PERIODE_RECONCILIATION`, `DELAI_LANCEMENT_MS`.
///
/// ⚠️ **LE PIRE CAS RESTE OUVERT À UN ENDROIT, NOMMÉ ET NON BORNÉ** : si *k*
/// applications apparaissent d'un coup, le terme d'icônes vaut `k × 10 ms` et
/// le critère ① tombe dès `k > 72`. C'est le prix de la voie « une seule
/// réconciliation pour tout un train », et il est assumé.
pub const DELAI_ANTI_REBOND_MAX: Duration = Duration::from_secs(4);

/// L'état d'un train de notifications en cours.
///
/// Deux instants suffisent : celui de la **première** notification du train
/// — qui borne l'ajournement — et celui de la **dernière** — qui le repousse.
#[derive(Default, Debug)]
pub struct Rebond {
    premiere: Option<Instant>,
    derniere: Option<Instant>,
}

impl Rebond {
    /// Une notification vient d'arriver.
    ///
    /// La première du train fixe le point d'ancrage du plafond ; les suivantes
    /// ne repoussent que l'échéance courte.
    pub fn notifier(&mut self, maintenant: Instant) {
        if self.premiere.is_none() {
            self.premiere = Some(maintenant);
        }
        self.derniere = Some(maintenant);
    }

    /// L'instant auquel la réconciliation doit partir, ou `None` si aucun train
    /// n'est en cours.
    ///
    /// 🔴 `min` DES DEUX, ET C'EST TOUT L'ANTI-REBOND : la branche courte
    /// absorbe les rafales, la branche longue garantit qu'un flux continu
    /// finit quand même par produire un catalogue.
    pub fn echeance(&self) -> Option<Instant> {
        let premiere = self.premiere?;
        let derniere = self.derniere?;
        Some((derniere + DELAI_ANTI_REBOND).min(premiere + DELAI_ANTI_REBOND_MAX))
    }

    /// L'échéance est-elle atteinte ?
    ///
    /// ⚠️ **Rend `false` au repos**, et c'est délibéré : un anti-rebond qui
    /// échoirait sans notification déclencherait des réconciliations fantômes,
    /// c'est-à-dire précisément le coût que ce module existe pour éviter.
    pub fn du(&self, maintenant: Instant) -> bool {
        self.echeance().is_some_and(|echeance| maintenant >= echeance)
    }

    /// Le train est consommé : la réconciliation part.
    ///
    /// 🔴 SANS CETTE REMISE À ZÉRO, LA BORNE HAUTE MORDRAIT POUR TOUJOURS après
    /// le premier train — `premiere` resterait au tout premier instant, et
    /// `premiere + MAX` serait déjà dépassé à jamais : chaque notification
    /// ultérieure déclencherait immédiatement, c'est-à-dire que l'anti-rebond
    /// cesserait d'exister au bout d'un train.
    pub fn consommer(&mut self) {
        self.premiere = None;
        self.derniere = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⚠️ `Instant` NE SE FABRIQUE PAS, IL S'OFFSETTE. Tous les tests partent
    /// d'un `Instant::now()` et lui ajoutent des `Duration` : **aucun ne dort.**
    fn base() -> Instant {
        Instant::now()
    }

    #[test]
    fn une_notification_isolee_echoit_apres_le_delai_court() {
        let t = base();
        let mut r = Rebond::default();
        r.notifier(t);
        assert_eq!(r.echeance(), Some(t + DELAI_ANTI_REBOND));
        assert!(!r.du(t + DELAI_ANTI_REBOND - Duration::from_millis(1)));
        assert!(r.du(t + DELAI_ANTI_REBOND));
    }

    /// 🔴 C'EST *L'ANTI-REBOND*, PAS UN SIMPLE DÉLAI : la seconde notification
    /// REPOUSSE l'échéance au lieu de la laisser où la première l'avait mise.
    #[test]
    fn une_seconde_notification_repousse_l_echeance() {
        let t = base();
        let mut r = Rebond::default();
        r.notifier(t);
        r.notifier(t + Duration::from_millis(500));
        assert_eq!(
            r.echeance(),
            Some(t + Duration::from_millis(500) + DELAI_ANTI_REBOND)
        );
        assert!(
            !r.du(t + DELAI_ANTI_REBOND),
            "l'échéance de la PREMIÈRE ne doit plus valoir"
        );
    }

    /// 🔴 **LE TEST QUI COMPTE.** Sans lui, la borne haute pourrait être
    /// absente et les quatre autres passeraient : un train qui ne s'arrête
    /// jamais ferait fuir l'échéance indéfiniment, et aucune réconciliation ne
    /// partirait — la panne que `DELAI_ANTI_REBOND_MAX` existe pour fermer.
    #[test]
    fn un_train_continu_echoit_quand_meme_au_plafond() {
        let t = base();
        let mut r = Rebond::default();
        // Une notification toutes les 100 ms pendant dix secondes : la branche
        // courte ne peut JAMAIS échoir, `derniere + 750 ms` étant toujours
        // repoussé avant d'être atteint.
        for i in 0..100u32 {
            r.notifier(t + Duration::from_millis(100 * u64::from(i)));
        }
        assert_eq!(r.echeance(), Some(t + DELAI_ANTI_REBOND_MAX));
        assert!(r.du(t + DELAI_ANTI_REBOND_MAX));
    }

    #[test]
    fn consommer_remet_le_train_a_zero_et_le_suivant_repart_de_sa_premiere() {
        let t = base();
        let mut r = Rebond::default();
        r.notifier(t);
        r.consommer();
        assert_eq!(r.echeance(), None, "un train consommé n'a plus d'échéance");
        let t2 = t + Duration::from_secs(60);
        r.notifier(t2);
        assert_eq!(
            r.echeance(),
            Some(t2 + DELAI_ANTI_REBOND),
            "le train suivant repart de SA première, jamais de l'ancienne"
        );
    }

    #[test]
    fn aucune_notification_ne_produit_aucune_echeance() {
        let r = Rebond::default();
        assert_eq!(r.echeance(), None);
        assert!(!r.du(base()));
        assert!(!r.du(base() + Duration::from_secs(3600)));
    }

    /// La dérivation de D6 est vérifiée par le test, pas seulement écrite dans
    /// le commentaire : le pire cas doit tenir SOUS les cinq secondes que le
    /// critère ① exige, marge d'icônes comprise.
    #[test]
    fn le_pire_cas_derive_tient_sous_les_cinq_secondes_du_critere() {
        let granularite_sondage = Duration::from_millis(200);
        let cout_reconciliation = Duration::from_millis(70);
        let une_icone_neuve = Duration::from_millis(10);
        let pire = DELAI_ANTI_REBOND_MAX + granularite_sondage + cout_reconciliation + une_icone_neuve;
        assert!(
            pire < Duration::from_secs(5),
            "pire cas {pire:?} : le critère ① exige moins de cinq secondes"
        );
    }
}
