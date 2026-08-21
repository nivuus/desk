//! La fenêtre de comptage : ce que le catalogue a gagné **pendant** une
//! installation, et non « à la réconciliation qui la suit ».
//!
//! 🔴 **CE MODULE EXISTE PARCE QUE LA PHRASE DE LA SPÉCIFICATION, PRISE À LA
//! LETTRE, PRODUIT UN FAUX `sans_effet`.** Elle dit : *une installation est
//! réussie quand la réconciliation qui la SUIT rapporte au moins une
//! application apparue.* Or la réconciliation tourne toutes les
//! [`crate::apps::PERIODE_RECONCILIATION`] — trente secondes — **pendant** que
//! l'installeur travaille. Un installeur qui pose son raccourci à la trentième
//! seconde puis continue trois minutes verra ses applications apparaître dans
//! une réconciliation **intercalaire** ; celle qui suit sa sortie n'aura alors
//! plus rien de neuf à rapporter, et le verdict serait `sans_effet` pour une
//! installation parfaitement réussie.
//!
//! **D'où une fenêtre** : ouverte au lancement, nourrie par chaque
//! réconciliation, fermée après la sortie du processus.
//!
//! ⚠️ **[`Fenetres::ajouter`] AJOUTE À TOUTES LES FENÊTRES OUVERTES, et c'est
//! délibéré.** Deux installations concurrentes sont possibles, et chacune doit
//! compter ce qui est apparu pendant SA fenêtre — quitte à ce que les deux
//! comptent la même application. **Attribuer une apparition à une seule
//! installation serait DEVINER** : rien, dans un raccourci qui vient
//! d'apparaître, ne dit quel installeur l'a posé. Le double comptage rend au
//! pire deux `reussie` là où l'une des deux n'a rien fait ; l'attribution
//! devinée rendrait un `sans_effet` **faux**, c'est-à-dire exactement le
//! défaut que ce module existe pour empêcher.
//!
//! **Pur, aucun `cfg`, aucune horloge, aucune entrée-sortie** : ce module ne
//! sait ni ce qu'est une application, ni quand une réconciliation a lieu — il
//! reçoit un compte et le range.

use std::collections::BTreeMap;

/// Les fenêtres de comptage ouvertes, une par installation en vol.
///
/// La `BTreeMap` donne un ordre de parcours stable pour la même raison
/// qu'`apps::reconciliation` l'a choisie : deux exécutions identiques doivent
/// journaliser la même chose, sans quoi toute recette qui compare deux tours
/// devient indécidable.
#[derive(Debug, Default)]
pub struct Fenetres {
    ouvertes: BTreeMap<String, usize>,
}

impl Fenetres {
    pub fn nouvelles() -> Self {
        Self::default()
    }

    /// Ouvre la fenêtre d'une installation, à zéro.
    ///
    /// ⚠️ **ROUVRIR UN IDENTIFIANT VIVANT REMET LE COMPTEUR À ZÉRO**, et ce
    /// n'est pas un cas limite gratuit : un identifiant est unique par
    /// installation, donc une seconde ouverture signifie ou bien un ordre
    /// rejoué, ou bien un identifiant réemployé. Dans les deux cas, garder le
    /// compte précédent attribuerait à l'installation qui commence des
    /// apparitions **antérieures à son propre lancement** — c'est-à-dire la
    /// devinette que l'en-tête de ce module refuse. Repartir de zéro ne peut
    /// produire qu'un `sans_effet`, qui est un aveu ; l'autre choix produirait
    /// un `reussie`, qui est une affirmation.
    pub fn ouvrir(&mut self, id: &str) {
        self.ouvertes.insert(id.to_string(), 0);
    }

    /// Verse le compte d'une réconciliation à **toutes** les fenêtres ouvertes.
    ///
    /// 🔴 IL S'AGIT D'UNE ACCUMULATION, JAMAIS D'UNE AFFECTATION. Une
    /// réconciliation qui ne trouve rien — le cas nominal, tour après tour, sur
    /// un disque au repos — ne doit **rien effacer** de ce que les précédentes
    /// ont vu, sans quoi la fenêtre ne mesurerait que son dernier instant et
    /// rendrait le faux `sans_effet` qu'elle existe pour empêcher. C'est la
    /// rouge de ce module.
    pub fn ajouter(&mut self, apparues: usize) {
        for total in self.ouvertes.values_mut() {
            *total = total.saturating_add(apparues);
        }
    }

    /// Ferme la fenêtre et rend son total, ou `None` si elle n'était pas
    /// ouverte.
    ///
    /// 🔴 `None` N'EST PAS `Some(0)`, et les confondre serait la même faute
    /// qu'un `code_sortie` à `-1` en guise de sentinelle : « fermée à zéro » et
    /// « jamais ouverte » sont **deux faits différents**. Le premier dit que
    /// l'installeur a tourné sans rien poser — c'est [`super::verdict::Issue::SansEffet`] ;
    /// le second dit que nous avons perdu la trace de l'installation, et c'est
    /// [`super::verdict::Issue::IssueInconnue`]. Rendre `0` dans les deux cas
    /// ferait déclarer « annulée » une installation dont nous ne savons rien.
    pub fn fermer(&mut self, id: &str) -> Option<usize> {
        self.ouvertes.remove(id)
    }

    /// Combien d'installations sont en vol. L'appelant s'en sert pour ne pas
    /// forcer de réconciliation quand il n'y a personne à servir.
    pub fn en_vol(&self) -> usize {
        self.ouvertes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 🔴 LE SCÉNARIO QUE LA DÉCISION D9 DÉCRIT, et la rouge de ce module.
    /// Une implémentation qui ne retiendrait que le DERNIER versement rendrait
    /// `0` ici, et produirait le faux `sans_effet` pour une installation qui a
    /// bel et bien posé trois applications.
    #[test]
    fn une_reconciliation_vide_n_efface_pas_ce_que_la_precedente_a_compte() {
        let mut fenetres = Fenetres::nouvelles();
        fenetres.ouvrir("inst-1");
        fenetres.ajouter(3);
        fenetres.ajouter(0);
        assert_eq!(fenetres.fermer("inst-1"), Some(3));
    }

    #[test]
    fn les_versements_s_accumulent_sur_toute_la_duree_de_la_fenetre() {
        let mut fenetres = Fenetres::nouvelles();
        fenetres.ouvrir("inst-1");
        for apparues in [0, 2, 0, 0, 1, 0] {
            fenetres.ajouter(apparues);
        }
        assert_eq!(fenetres.fermer("inst-1"), Some(3));
    }

    /// Deux installations concurrentes comptent la MÊME apparition, et c'est
    /// le comportement voulu : rien ne dit laquelle a posé le raccourci.
    #[test]
    fn deux_fenetres_concurrentes_comptent_chacune_ce_qui_passe_pendant_la_sienne() {
        let mut fenetres = Fenetres::nouvelles();
        fenetres.ouvrir("inst-1");
        fenetres.ajouter(1);
        // La seconde s'ouvre en cours de route : elle ne doit RIEN hériter du
        // versement d'avant son lancement.
        fenetres.ouvrir("inst-2");
        fenetres.ajouter(2);
        assert_eq!(fenetres.en_vol(), 2);
        assert_eq!(fenetres.fermer("inst-1"), Some(3));
        assert_eq!(fenetres.fermer("inst-2"), Some(2));
        assert_eq!(fenetres.en_vol(), 0);
    }

    /// Une fenêtre fermée cesse de compter — sans quoi une installation
    /// achevée gonflerait le total de la suivante.
    #[test]
    fn une_fenetre_fermee_ne_compte_plus() {
        let mut fenetres = Fenetres::nouvelles();
        fenetres.ouvrir("inst-1");
        fenetres.ouvrir("inst-2");
        assert_eq!(fenetres.fermer("inst-1"), Some(0));
        fenetres.ajouter(5);
        assert_eq!(fenetres.fermer("inst-2"), Some(5));
    }

    /// 🔴 « Jamais ouverte » et « fermée à zéro » ne se confondent pas.
    #[test]
    fn fermer_une_fenetre_inconnue_rend_none_et_non_zero() {
        let mut fenetres = Fenetres::nouvelles();
        assert_eq!(fenetres.fermer("jamais-ouverte"), None);
        fenetres.ouvrir("inst-1");
        assert_eq!(fenetres.fermer("inst-1"), Some(0));
        // Et une seconde fermeture du même identifiant est, elle aussi, une
        // fenêtre inconnue : le total ne se relit pas deux fois.
        assert_eq!(fenetres.fermer("inst-1"), None);
    }

    #[test]
    fn rouvrir_un_identifiant_vivant_repart_de_zero() {
        let mut fenetres = Fenetres::nouvelles();
        fenetres.ouvrir("inst-1");
        fenetres.ajouter(4);
        fenetres.ouvrir("inst-1");
        assert_eq!(fenetres.en_vol(), 1, "l'identifiant reste unique");
        assert_eq!(fenetres.fermer("inst-1"), Some(0));
    }

    /// Sans fenêtre ouverte, `ajouter` est inerte : la boucle de
    /// réconciliation l'appelle à chaque tour, installation ou non.
    #[test]
    fn ajouter_sans_fenetre_ouverte_ne_fait_rien() {
        let mut fenetres = Fenetres::nouvelles();
        fenetres.ajouter(9);
        assert_eq!(fenetres.en_vol(), 0);
        fenetres.ouvrir("inst-1");
        assert_eq!(fenetres.fermer("inst-1"), Some(0));
    }
}
