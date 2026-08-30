//! Rendre LISIBLE la chaîne de causes d'une `anyhow::Error` dans une trace.
//!
//! 🔴 **Le défaut que ce module ferme est un défaut D'INSTRUMENT, et il a
//! coûté un lot entier.** Le lot 25 a mesuré 74 puis 52 `réveil refusé, la
//! fenêtre reste endormie` d'affilée sans que personne ne puisse dire
//! POURQUOI : le site de trace écrivait `%erreur` sur une `anyhow::Error`,
//! or le `Display` SIMPLE d'`anyhow` ne rend **que la couche la plus
//! EXTERNE** — ici le `with_context` posé par `transitions.rs::reveiller`,
//! c'est-à-dire « réveil de la session …:w-24 », qui ne dit rien de plus que
//! « ça a raté ». Le `HRESULT` de la couche Windows, seule donnée qui
//! réponde à la question, restait dans les causes, jetée à l'écriture.
//!
//! **Pourquoi `{:#}` et non `{:?}`** — le choix est écrit ici pour qu'on ne
//! le repose pas :
//!
//! - `{}` (donc `%erreur` en `tracing`) : la couche externe SEULE. C'est le
//!   défaut qu'on corrige.
//! - `{:#}` : la chaîne COMPLÈTE, des causes séparées par `: `, **sur une
//!   seule ligne**. C'est ce qu'on retient.
//! - `{:?}` : la chaîne complète **plus** une trace d'exécution, sur
//!   PLUSIEURS lignes, et seulement si `RUST_BACKTRACE` est posée — que
//!   `scripts/run-agent.sh` ne pose pas. Écarté pour deux raisons : la trace
//!   serait vide en pratique, et le multi-ligne casserait le seul instrument
//!   dont ce dépôt dispose sur `agent.log`, qui est le comptage de lignes par
//!   `grep -c` (un `réveil refusé` compterait pour trois).
//!
//! Le dépôt avait déjà tranché ainsi, deux fois, sans jamais poser la règle
//! au même endroit : `transport/piste_audio.rs` (leg 6 de D10, un `HRESULT`
//! perdu de la même façon) et `diagnostics/multifenetre/plafond/sonde.rs`.
//! **Ce module est cet endroit** ; il vit à la racine nue parce que son nom
//! se comprend sans référence à un parent (convention de `CLAUDE.md`,
//! précédents `geometry`, `sortie_dxgi`, `survie_verdict`).

/// Rend la chaîne de causes complète d'une erreur, sur une seule ligne.
///
/// À employer partout où l'on journalisait `%erreur` sur une `anyhow::Error` :
/// `tracing::warn!(erreur = %crate::cause::chaine(&erreur), "…")`.
///
/// Prend une référence et **ne consomme pas** l'erreur : plusieurs sites
/// corrigés la journalisent puis la propagent.
pub fn chaine(erreur: &anyhow::Error) -> String {
    format!("{erreur:#}")
}

#[cfg(test)]
mod tests {
    use anyhow::{anyhow, Context};

    /// Le contrôle qui vaut : il doit ROUGIR si l'on revient à `{}`. Les deux
    /// assertions sont donc dissymétriques à dessein — la première dit ce que
    /// `{:#}` ajoute, la seconde dit ce que `{}` PERD, et sans elle le test
    /// passerait encore avec un `format!("{erreur}")` dans `chaine`.
    #[test]
    fn la_chaine_porte_la_cause_profonde_que_le_display_simple_jette() {
        let profonde = anyhow!("0x88890004");
        let erreur = Err::<(), _>(profonde)
            .context("activation de l'encodeur H.264 matériel (ActivateObject)")
            .context("réveil de la session prefixe:w-24")
            .unwrap_err();

        let rendue = super::chaine(&erreur);
        assert!(rendue.contains("0x88890004"), "la cause profonde manque : {rendue}");
        assert!(
            rendue.contains("ActivateObject"),
            "la couche intermédiaire manque : {rendue}"
        );
        assert!(
            rendue.contains("réveil de la session"),
            "la couche externe manque : {rendue}"
        );

        // Le témoin négatif, dans le MÊME relevé : le `Display` simple, celui
        // que `%erreur` employait, ne rend QUE la couche externe.
        let simple = format!("{erreur}");
        assert_eq!(simple, "réveil de la session prefixe:w-24");
        assert!(!simple.contains("0x88890004"), "le témoin négatif est faux : {simple}");
    }

    /// Une erreur SANS contexte doit rester lisible telle quelle : la
    /// correction ne doit pas dégrader le cas simple, qui est le plus fréquent.
    #[test]
    fn une_erreur_sans_contexte_est_rendue_telle_quelle() {
        assert_eq!(super::chaine(&anyhow!("aucune sortie DXGI nommée 0:1")), "aucune sortie DXGI nommée 0:1");
    }
}
