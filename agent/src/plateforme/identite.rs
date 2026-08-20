//! D'où un processus agent tient son identité de plateforme — la règle, PURE.
//!
//! 🔴 **UN SEUL CANAL `/agent` PAR VM, ET C'EST TOUT LE SUJET DE CE FICHIER.**
//! La plateforme tient sa table des agents joignables par `vm_id`
//! (`plateforme/src/agents/registre.ts`) : au plus UN socket par VM, et
//! **le dernier inscrit gagne, l'ancien étant fermé**. C'est la bonne
//! règle — deux sockets pour la même VM poseraient la question « lequel reçoit
//! l'ordre de lancement ? », à laquelle la clé primaire d'`agent_enrole`
//! répond déjà. Ce qui était faux, c'est que l'agent en présentait plusieurs.
//!
//! **Le défaut, MESURÉ sur la VM le 20 août 2026 (une exécution, 64 s) :**
//! `lancer_pont` retirait `SUPERVISEUR`, `CAPTEUR`, `TEST_FILE` et
//! `WINDOW_TITLE` de l'environnement du pont fichiers, **mais pas `AGENT_VM`
//! ni `AGENT_SECRET`**. Le pont s'enrôlait donc sous la même identité que son
//! père, chacun évinçait l'autre, l'évincé reprenait aussitôt — le repli
//! exponentiel repart de zéro après tout enrôlement réussi —, et le cycle
//! n'avait aucun terme : **95 enrôlements et 94 évictions en 64 s**, à ~1,5 Hz.
//! Ce que cela coûtait, relevé en recette G1 : le catalogue complet renvoyé à
//! chaque cycle, les messages incrémentaux perdus, les réponses de lancement
//! perdues (les `504 delai`), et **deux boucles de découverte** au lieu d'une.
//!
//! ⚠️ **LE PONT N'ÉTAIT PAS SEUL EN CAUSE, et c'est ce que la cartographie
//! montre** : `lancer` (les enfants de fenêtre) ne les retirait pas davantage,
//! et un enfant traverse lui aussi l'enrôlement de `main.rs`. La recette G1
//! n'avait aucune fenêtre ouverte, donc aucun enfant : le défaut y était
//! invisible sur cette moitié-là, et il aurait mordu à la première fenêtre.
//! Le **capteur**, lui, est hors de cause — `main.rs` lui rend la main AVANT
//! l'enrôlement — mais ses variables lui sont retirées quand même, par la
//! règle que `lanceur.rs` s'impose déjà pour `SUPERVISEUR`, `CAPTEUR` et
//! `PONT` : *un ordre de test est une propriété qui change, un `env_remove`
//! non.*
//!
//! 🔴 **CE QUI SE TRANSMET EST LE JETON, JAMAIS LE SECRET.** Un enfant et le
//! pont ont besoin d'une identité — ils ouvrent chacun leur propre
//! `PeerConnection`, et la garde de la plateforme refuse une poignée de main
//! sans jeton d'agent depuis le sous-bloc P3 —, mais ils n'ont besoin
//! d'AUCUN canal : ils ne battent pas le cœur de la VM, ne poussent aucun
//! catalogue et ne reçoivent aucun ordre de lancement. Le superviseur leur
//! passe donc `AGENT_JETON`, et retient `AGENT_VM`/`AGENT_SECRET`. Le secret
//! d'enrôlement ne quitte plus le processus qui tient le canal.

/// D'où ce processus tire son jeton d'agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceIdentite {
    /// `AGENT_JETON` : l'identité vient du père, qui tient le canal. Ce
    /// processus n'ouvre AUCUN canal `/agent`.
    Heritee(String),
    /// `AGENT_VM` + `AGENT_SECRET` : ce processus s'enrôle lui-même, et
    /// devient le socket joignable de la VM.
    Enrolement { vm: String, secret: String },
    /// Ni l'un ni l'autre : aucun jeton, donc aucune session ne s'établira.
    /// Ce n'est pas un mode de repli, c'est une panne annoncée.
    Aucune,
}

/// Tranche la source d'identité de ce processus.
///
/// 🔴 **LE JETON HÉRITÉ L'EMPORTE, MÊME SI LE COUPLE D'ENRÔLEMENT EST LÀ**, et
/// cette précédence est le cœur du remède plutôt qu'un détail. `lanceur.rs`
/// retire `AGENT_VM` et `AGENT_SECRET` de tout enfant, donc le cas ne devrait
/// pas se produire — mais « ne devrait pas » est exactement ce qu'on disait de
/// l'héritage lui-même avant de le mesurer. Si les deux arrivent quand même,
/// par une voie qu'on n'a pas prévue, la précédence garantit que le processus
/// n'ouvre PAS un second canal : le défaut redeviendrait au pire une identité
/// vieillissante, jamais une éviction mutuelle sans terme.
///
/// ⚠️ **UN COUPLE INCOMPLET REND `Aucune`, jamais un demi-enrôlement.** C'est
/// le contrat que `main.rs` portait déjà (« les deux ou aucun ») : présenter un
/// nom de VM sans secret ne peut qu'être refusé, et le faire quand même ne
/// produirait qu'un refus `enrolement` indistinct d'un secret faux.
pub fn source(
    jeton_herite: Option<&str>,
    vm: Option<&str>,
    secret: Option<&str>,
) -> SourceIdentite {
    if let Some(jeton) = jeton_herite {
        return SourceIdentite::Heritee(jeton.to_string());
    }
    match (vm, secret) {
        (Some(vm), Some(secret)) => SourceIdentite::Enrolement {
            vm: vm.to_string(),
            secret: secret.to_string(),
        },
        _ => SourceIdentite::Aucune,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_jeton_herite_dispense_de_tout_enrolement() {
        assert_eq!(
            source(Some("jwt.h"), None, None),
            SourceIdentite::Heritee("jwt.h".into())
        );
    }

    /// 🔴 LA PRÉCÉDENCE, ET C'EST ELLE QUI TIENT L'INVARIANT « UN SEUL CANAL
    /// PAR VM ». Mutation qui la rougit : tester le couple d'enrôlement en
    /// premier. Un processus recevant les trois variables ouvrirait alors son
    /// propre canal, et l'on retomberait sur les 95 enrôlements / 94 évictions
    /// mesurés le 20 août 2026.
    #[test]
    fn le_jeton_herite_l_emporte_sur_un_couple_d_enrolement_present() {
        assert_eq!(
            source(Some("jwt.h"), Some("vm-1"), Some("chut")),
            SourceIdentite::Heritee("jwt.h".into())
        );
    }

    #[test]
    fn sans_jeton_le_couple_complet_fait_s_enroler() {
        assert_eq!(
            source(None, Some("vm-1"), Some("chut")),
            SourceIdentite::Enrolement { vm: "vm-1".into(), secret: "chut".into() }
        );
    }

    /// Les deux moitiés du couple incomplet, et le cas vide — un test par
    /// forme, parce qu'un `assert!` qui interrompt au premier échec laisserait
    /// les suivantes non éprouvées.
    #[test]
    fn un_couple_incomplet_ne_produit_aucune_identite() {
        assert_eq!(source(None, Some("vm-1"), None), SourceIdentite::Aucune);
        assert_eq!(source(None, None, Some("chut")), SourceIdentite::Aucune);
        assert_eq!(source(None, None, None), SourceIdentite::Aucune);
    }
}
