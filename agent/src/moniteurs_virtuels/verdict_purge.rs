//! Ce que la comparaison des deux relevés de topologie permet RÉELLEMENT
//! d'établir après une purge.
//!
//! Hors `#[cfg(windows)]`, comme le module parent et pour la même raison que
//! `superviseur::designation` et `superviseur::reprise` : **la règle doit
//! avoir des tests, et ils ne tourneraient pas sous `#[cfg(windows)]`.**
//! `purge.rs`, lui, est gaté — la première rédaction de ce verdict y vivait,
//! et `cargo test --workspace` filtrait ses quatre tests sans rien dire.

/// Ce que la comparaison des deux relevés permet RÉELLEMENT d'établir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Autant de sorties en moins que de retraits réussis.
    Conforme,
    /// **MOINS** de sorties qu'attendu : quelqu'un d'autre en a retiré une.
    /// Ce n'est **pas** notre échec, et le dénoncer noie les vrais.
    UnTiersAAussiRetire,
    /// **PLUS** de sorties qu'attendu : des retraits que le pilote a déclarés
    /// réussis n'ont rien retiré. C'est la seule anomalie que cette
    /// comparaison puisse imputer à la purge.
    RetraitsSansEffet,
}

/// 🔴 **CE VERDICT A CRIÉ À TORT À CHAQUE DÉMARRAGE, ET C'EST CE QU'ON CORRIGE.**
/// L'ancienne forme comparait `apres == avant - retirees` et rendait une
/// `ERROR` sur **toute** différence. Mesuré deux fois le 30 août 2026 :
/// `retirees=5 avant=7 apres=1 attendu=2` puis `retirees=7 avant=9 apres=1
/// attendu=2` — `apres` **INFÉRIEUR** à `attendu` dans les deux cas, c'est-à-dire
/// qu'il avait disparu PLUS de sorties que nous n'en avions retirées. La purge
/// avait pourtant parfaitement fonctionné (moniteurs SudoVDA : 8 → 0).
///
/// **L'égalité supposée est fausse dès qu'un tiers touche la topologie**, et
/// c'est le cas nominal sur cette VM : Apollo crée puis détruit une sortie
/// virtuelle temporaire pour sonder ses encodeurs.
///
/// ⚠️ **Un `ERROR` qui crie sans raison à chaque démarrage est un `ERROR` que
/// plus personne ne lit.** Ce dépôt a perdu plusieurs manches parce que la
/// trace qui disait la vérité était noyée.
///
/// **Ce que la comparaison peut encore établir, et qu'on garde** : s'il reste
/// PLUS de sorties qu'attendu, alors des retraits déclarés réussis n'ont rien
/// retiré — un handle ou un IOCTL cassé, c'est-à-dire le défaut que ce verdict
/// existait pour attraper.
pub fn verdict(avant: usize, apres: usize, retirees: usize) -> Verdict {
    let attendu = avant.saturating_sub(retirees);
    match apres.cmp(&attendu) {
        std::cmp::Ordering::Equal => Verdict::Conforme,
        std::cmp::Ordering::Less => Verdict::UnTiersAAussiRetire,
        std::cmp::Ordering::Greater => Verdict::RetraitsSansEffet,
    }
}

#[cfg(test)]
mod tests_verdict {
    use super::*;

    #[test]
    fn autant_de_moins_que_de_retraits_est_conforme() {
        assert_eq!(verdict(9, 2, 7), Verdict::Conforme);
        assert_eq!(verdict(0, 0, 0), Verdict::Conforme);
    }

    /// 🔴 LES DEUX RELEVÉS RÉELS DU 30 AOÛT 2026, qui rendaient une `ERROR`.
    /// Ils ne doivent plus en rendre : la purge avait fonctionné.
    #[test]
    fn les_deux_releves_qui_criaient_a_tort_ne_crient_plus() {
        assert_eq!(verdict(7, 1, 5), Verdict::UnTiersAAussiRetire);
        assert_eq!(verdict(9, 1, 7), Verdict::UnTiersAAussiRetire);
    }

    /// 🔴 LA ROUGE QUI RESTE, et c'est le défaut que le verdict existait pour
    /// attraper : le pilote déclare des retraits réussis, la topologie ne
    /// bouge pas.
    #[test]
    fn des_retraits_sans_effet_restent_denonces() {
        assert_eq!(verdict(9, 9, 7), Verdict::RetraitsSansEffet);
        assert_eq!(verdict(3, 3, 1), Verdict::RetraitsSansEffet);
    }

    /// Le cas que l'ancienne forme traitait déjà : aucun retrait, rien ne
    /// bouge, tout va bien.
    #[test]
    fn aucun_retrait_et_rien_ne_bouge_est_conforme() {
        assert_eq!(verdict(4, 4, 0), Verdict::Conforme);
    }
}
