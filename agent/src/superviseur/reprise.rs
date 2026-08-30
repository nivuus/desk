//! Combien de fois réessayer d'attacher une sortie, et quand renoncer.
//!
//! 🔴 **LE FAIT QUI COMMANDE CETTE RÈGLE, MESURÉ LE 30 AOÛT 2026.** Une sortie
//! virtuelle TENUE s'attache en **moins d'une seconde** — relevé à 1 Hz,
//! `seconde=1 attachees=2 presente=Some(true)`. Dans la même heure, sur la même
//! machine, une autre sortie n'était **toujours pas** attachée après 3 s, et
//! le produit la refusait après 5 s. **L'attachement n'est donc pas LENT, il
//! est INTERMITTENT** — et la perturbation est corrélée à la sonde d'encodeur
//! qu'Apollo relance à chaque changement de topologie d'affichage.
//!
//! 🔴 **CE N'EST DONC PAS UNE CONSTANTE À RALLONGER.** Allonger l'attente d'un
//! seul tour ne fait qu'attendre plus longtemps DANS la même fenêtre de
//! perturbation ; ce qu'il faut est **plusieurs chances espacées**, pour qu'au
//! moins une tombe dans une accalmie. C'est pourquoi la règle porte un nombre
//! de TOURS et un RÉPIT, et non un délai plus grand.
//!
//! 🔴 **ET SURTOUT : ON NE RECRÉE PAS LA SORTIE ENTRE DEUX TOURS.** Détruire
//! puis recréer change la topologie, donc **redéclenche la sonde d'Apollo** —
//! la reprise nourrirait exactement ce qu'elle attend. C'est aussi ce qui
//! consomme le vivier : le relevé du bras rouge porte **9 sorties créées pour
//! 7 refus**. Le pilote a déjà accepté la création ; il n'y a rien à refaire.
//!
//! ⚠️ **CE QUE CETTE REPRISE COÛTE, ET IL FAUT LE DIRE.** `creer_sortie` court
//! DANS la boucle du superviseur, qui est mono-fil : pendant l'attente, rien
//! d'autre n'est traité. Le pire cas passe de 5 s à
//! `TOURS × LIMITE_RATTACHEMENT + (TOURS − 1) × REPIT`. C'est du temps d'attente
//! PUR avant que l'utilisateur voie son refus, quand la sortie ne viendra
//! jamais.
//!
//! ⚠️ **Hors de la portée de `DELAI_ATTENTE_VIEWPORT_MAX`** : cette borne de
//! 30 s ne filtre que `Etat::AttendLeViewport`
//! (`superviseur/table/orphelines.rs`), alors que `creer_sortie` court en
//! `Etat::AttendLaSortie` (`table/attribution.rs`). Vérifié en lisant les deux,
//! pas supposé — une reprise annulée par un garde-fou voisin serait un remède
//! qui ne remédie à rien.

use std::time::Duration;

/// Le nombre de chances données à une sortie de s'attacher.
///
/// **Trois, et la valeur vient de la mesure** : l'attachement réussit en
/// < 1 s, et la perturbation d'Apollo revient à une cadence de l'ordre de 5 s
/// (relevé de son journal : une sonde d'encodeur par changement de topologie,
/// à la cadence de nos propres tentatives). Trois tours espacés couvrent donc
/// plusieurs cycles de perturbation. ⚠️ **Ce n'est PAS une constante
/// calibrée** — aucune ne l'est dans ce dépôt, et celle-ci ne fait pas
/// exception : c'est une valeur DÉRIVÉE d'une mesure, ce qui n'est pas la
/// même chose.
pub const TOURS: u32 = 3;

/// Le temps rendu à la machine entre deux tours.
///
/// Sans répit, les trois tours seraient contigus et ne vaudraient qu'un seul
/// tour plus long — exactement ce que cette règle refuse de faire.
pub const REPIT: Duration = Duration::from_secs(1);

/// Ce qu'il faut faire après un tour d'attente infructueux.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Suite {
    /// Réessayer, **sur la même sortie**, après ce répit.
    Reessayer { tour_suivant: u32, apres: Duration },
    /// Renoncer : la sortie est rendue au pilote et la fenêtre refusée.
    ///
    /// `tours_epuises` sert au JOURNAL : un refus après N tours ne se lit pas
    /// comme un refus immédiat, et sans ce nombre on ne saurait jamais lequel
    /// on lit.
    Renoncer { tours_epuises: u32 },
}

/// La règle, pure : après le tour `tour_acheve` (1-indexé), continue-t-on ?
///
/// 🔴 **BORNÉE PAR CONSTRUCTION.** Ce dépôt porte déjà une boucle de relance
/// non bornée comme défaut ouvert ; celle-ci ne peut pas en devenir une
/// seconde : `tour_acheve >= tours` rend toujours `Renoncer`, et `tours == 0`
/// aussi — un nombre de tours nul ne doit pas se lire comme « à l'infini ».
pub fn apres_un_tour(tour_acheve: u32, tours: u32, repit: Duration) -> Suite {
    if tours == 0 || tour_acheve >= tours {
        return Suite::Renoncer { tours_epuises: tour_acheve.max(1) };
    }
    Suite::Reessayer { tour_suivant: tour_acheve + 1, apres: repit }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_tours_intermediaires_reessaient_sur_la_meme_sortie() {
        assert_eq!(
            apres_un_tour(1, 3, REPIT),
            Suite::Reessayer { tour_suivant: 2, apres: REPIT }
        );
        assert_eq!(
            apres_un_tour(2, 3, REPIT),
            Suite::Reessayer { tour_suivant: 3, apres: REPIT }
        );
    }

    /// 🔴 La borne. Sans elle, la reprise deviendrait la seconde boucle de
    /// relance non bornée de ce dépôt.
    #[test]
    fn le_dernier_tour_renonce_et_dit_combien_il_en_a_faits() {
        assert_eq!(apres_un_tour(3, 3, REPIT), Suite::Renoncer { tours_epuises: 3 });
    }

    /// Un tour au-delà de la borne renonce aussi : aucune arithmétique ne peut
    /// faire repartir la boucle.
    #[test]
    fn au_dela_de_la_borne_on_renonce_encore() {
        assert_eq!(apres_un_tour(9, 3, REPIT), Suite::Renoncer { tours_epuises: 9 });
    }

    /// ⚠️ Le cas dégénéré : `tours = 0` ne doit pas se lire « sans borne ».
    #[test]
    fn zero_tour_renonce_immediatement_et_jamais_a_l_infini() {
        assert_eq!(apres_un_tour(1, 0, REPIT), Suite::Renoncer { tours_epuises: 1 });
        assert_eq!(apres_un_tour(0, 0, REPIT), Suite::Renoncer { tours_epuises: 1 });
    }

    /// Le produit d'AVANT ce remède, exprimé dans la même règle : un seul
    /// tour. C'est le TÉMOIN qui rend la reprise discriminante — il montre
    /// que la règle sait aussi ne rien réessayer.
    #[test]
    fn un_seul_tour_est_le_produit_d_avant_la_reprise() {
        assert_eq!(apres_un_tour(1, 1, REPIT), Suite::Renoncer { tours_epuises: 1 });
    }

    /// Le pire cas est BORNÉ et calculable : c'est ce que la boucle du
    /// superviseur peut passer sans rien traiter d'autre.
    #[test]
    fn le_pire_cas_est_borne_et_calculable() {
        let mut tours = 0;
        let mut attente = Duration::ZERO;
        let limite = Duration::from_secs(5);
        let mut tour = 1;
        loop {
            tours += 1;
            attente += limite;
            match apres_un_tour(tour, TOURS, REPIT) {
                Suite::Reessayer { tour_suivant, apres } => {
                    attente += apres;
                    tour = tour_suivant;
                }
                Suite::Renoncer { .. } => break,
            }
            assert!(tours <= TOURS, "la reprise doit être bornée");
        }
        assert_eq!(tours, TOURS);
        assert_eq!(attente, Duration::from_secs(17));
    }
}
