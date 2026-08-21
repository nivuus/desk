//! Les quatre états d'`APPS_SURVEILLANCE`, et **aucun n'est une présence**.
//!
//! **PUR** : aucun `cfg`, aucune lecture d'environnement, aucune entrée-sortie.
//! La lecture de `std::env` vit chez l'appelant (`apps::brancher`) ; ce qui se
//! DÉCIDE vit ici, et c'est la seule part de la garde qu'on puisse voir rougir
//! sur l'hôte — `apps::desarme` porte déjà exactement cet argument.
//!
//! 🔴 UNE SEULE VARIABLE POUR QUATRE ÉTATS, ET NON TROIS BOOLÉENS. Deux raisons,
//! et la seconde est structurelle :
//!
//! 1. le piège de `scripts/run-agent.sh` a été payé CINQ fois par ce dépôt
//!    (`SUPERVISEUR` en D1, `MULTIFENETRE_REPRISE` en D2, `AUDIO` en D7…) : une
//!    ligne à transmettre au lieu de trois, c'est un tiers du risque ;
//! 2. **les quatre états sont mutuellement exclusifs par construction.** Trois
//!    booléens autoriseraient « ni surveillance ni réconciliation périodique »,
//!    c'est-à-dire un agent qui ne réconcilie JAMAIS — un état qui n'a aucun
//!    sens et qu'aucune recette ne veut.

/// Ce que `APPS_SURVEILLANCE` commande.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Variable absente : **le produit livré.** Surveillance armée, anti-rebond
    /// armé, réconciliation périodique armée.
    Armee,
    /// `0` : surveillance désarmée — **le comportement de G1, exactement.**
    /// C'est la ROUGE du critère ①.
    Desarmee,
    /// `sans-rebond` : surveillance armée, **anti-rebond neutralisé**. Toute
    /// notification rompt l'attente. C'est la ROUGE du critère ④.
    SansRebond,
    /// `seule` : surveillance armée, **réconciliation périodique DÉSARMÉE**.
    /// C'est la ROUGE du critère ③.
    ///
    /// ⚠️ **Variable de BANC, jamais une configuration livrée** : un agent qui
    /// ne réconcilie que sur notification perd tout ce qu'une notification
    /// manquée emporte — c'est-à-dire exactement la panne que la
    /// réconciliation périodique existe pour fermer.
    Seule,
}

impl Mode {
    /// Rend le mode **et**, le cas échéant, la valeur inconnue à journaliser.
    ///
    /// 🔴 CHAQUE ÉTAT EST UNE ÉGALITÉ EXACTE, JAMAIS UN `is_some()`. Tester la
    /// présence ACTIVERAIT le mécanisme en écrivant `APPS_SURVEILLANCE=0` POUR
    /// LE COUPER, et le couperait en écrivant `=1` pour l'activer. Les deux
    /// erreurs se compensent au point que personne ne les verrait sans le test
    /// qui les fixe. C'est la convention de `PLEIN_ECRAN`, `AUDIO`, `APPS`,
    /// `ICONES`, `PART_SONDAGE` et `PRESSE_PAPIER`, et `crate::apps::desarme`
    /// est **RÉUTILISÉ, pas recopié** — G2 a posé ce précédent pour `ICONES`,
    /// et le réemploi est ce qui préserve gratuitement le cas `"0 "`.
    ///
    /// 🔴 UNE VALEUR INCONNUE RETIENT LE COMPORTEMENT LIVRÉ, ET LA NOMME. Sans
    /// cela, une coquille dans une rouge — `seul` pour `seule` — ferait tourner
    /// le comportement VERT sous le nom du ROUGE, et la recette lirait un
    /// verdict faux. C'est le patron « un contrôle qui ne peut pas échouer »
    /// sous une forme neuve : le montage rendrait `156` là où il croit lire
    /// `156`, pour une raison qui n'est pas celle qu'il mesure.
    pub fn lire(valeur: Option<&str>) -> (Mode, Option<String>) {
        if crate::apps::desarme(valeur) {
            return (Mode::Desarmee, None);
        }
        match valeur {
            None => (Mode::Armee, None),
            Some("sans-rebond") => (Mode::SansRebond, None),
            Some("seule") => (Mode::Seule, None),
            Some(autre) => (Mode::Armee, Some(autre.to_string())),
        }
    }

    /// Le fil de surveillance doit-il seulement démarrer ?
    pub fn surveille(&self) -> bool {
        !matches!(self, Mode::Desarmee)
    }

    /// L'anti-rebond doit-il différer la réconciliation ?
    pub fn rebond(&self) -> bool {
        !matches!(self, Mode::SansRebond)
    }

    /// La réconciliation périodique doit-elle courir ?
    ///
    /// ⚠️ **`Desarmee` rend `true`**, et ce n'est pas un oubli : `0` désarme la
    /// SURVEILLANCE, il ne coupe pas la source de vérité. C'est précisément ce
    /// qui fait de `APPS_SURVEILLANCE=0` le comportement de G1 à l'identique,
    /// donc une ROUGE de ① meilleure que celle que la spécification propose —
    /// même binaire, même corpus, même machine, une seule variable de
    /// différence.
    pub fn periodique(&self) -> bool {
        !matches!(self, Mode::Seule)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_quatre_etats_sont_des_egalites_exactes() {
        assert_eq!(Mode::lire(None), (Mode::Armee, None));
        assert_eq!(Mode::lire(Some("0")), (Mode::Desarmee, None));
        assert_eq!(Mode::lire(Some("sans-rebond")), (Mode::SansRebond, None));
        assert_eq!(Mode::lire(Some("seule")), (Mode::Seule, None));
    }

    /// 🔴 LA ROUGE DE LA CONVENTION : remplacer `crate::apps::desarme(valeur)`
    /// par `valeur.is_some()` fait tomber ce test sur sa PREMIÈRE assertion —
    /// `"sans-rebond"` deviendrait `Desarmee`, c'est-à-dire que demander la
    /// ROUGE du critère ④ rendrait celle du critère ①, et la recette lirait un
    /// verdict faux sans qu'aucune ligne ne le dise.
    #[test]
    fn une_presence_n_est_pas_un_desarmement() {
        assert_eq!(Mode::lire(Some("sans-rebond")).0, Mode::SansRebond);
        assert_eq!(Mode::lire(Some("seule")).0, Mode::Seule);
        assert_eq!(Mode::lire(Some("1")).0, Mode::Armee);
    }

    /// ⚠️ `"0 "` — un espace de trop dans un `.ps1` généré — NE DÉSARME PAS.
    /// Le test d'`apps.rs` le fixe déjà pour `APPS` ; le réemploi de `desarme`
    /// doit le préserver, et c'est ce que cette assertion vérifie.
    #[test]
    fn une_valeur_inconnue_retient_le_comportement_livre_et_se_nomme() {
        for valeur in ["seul", "SEULE", "", "00", "0 ", "sans_rebond", "false"] {
            let (mode, inconnue) = Mode::lire(Some(valeur));
            assert_eq!(mode, Mode::Armee, "{valeur:?} doit retenir le mode livré");
            assert_eq!(
                inconnue.as_deref(),
                Some(valeur),
                "{valeur:?} doit être NOMMÉE au journal"
            );
        }
    }

    /// Les trois prédicats, un par consommateur — et `Desarmee` laisse la
    /// réconciliation périodique courir, ce qui est ce qui rend `=0` égal à G1.
    #[test]
    fn chaque_predicat_ne_coupe_que_ce_qui_le_concerne() {
        assert!(Mode::Armee.surveille() && Mode::Armee.rebond() && Mode::Armee.periodique());
        assert!(!Mode::Desarmee.surveille());
        assert!(Mode::Desarmee.periodique(), "`0` ne coupe PAS la source de vérité");
        assert!(Mode::SansRebond.surveille() && !Mode::SansRebond.rebond());
        assert!(Mode::SansRebond.periodique());
        assert!(Mode::Seule.surveille() && Mode::Seule.rebond());
        assert!(!Mode::Seule.periodique());
    }
}
