//! Parmi quelles sorties DXGI le superviseur a le droit d'apparier une
//! fenêtre — et pourquoi la question ne se réduit pas à « laquelle vient
//! d'apparaître ».
//!
//! 🔴 **LE DÉFAUT QUE CE MODULE EXISTE POUR FERMER (lot 30, mesuré sur la
//! VM).** Sans le VGA de QEMU, la session 1 démarre avec zéro moniteur PnP
//! mais UN écran (`\\.\DISPLAY5`), dont la cible active porte
//! `statusFlags = 0x11` (`IN_USE | FORCED_AVAILABILITY_SYSTEM`) : une **cible
//! forcée**, que Windows fabrique quand il ne lui reste aucun affichage. Le
//! premier moniteur virtuel créé **REMPLACE cette cible forcée sur la MÊME
//! source** — il hérite donc du même nom GDI, et n'« apparaît » jamais. Le
//! produit n'appariant que parmi les sorties APPARUES, il refusait la fenêtre
//! (« aucune sortie d'affichage ne peut servir cette fenêtre ») et rendait au
//! pilote une sortie parfaitement utilisable. La boucle n'atteignait jamais
//! la deuxième fenêtre.
//!
//! ⚠️ **L'hypothèse « le registre pollué empêche l'attachement » a été
//! RÉFUTÉE** par le même lot : registre intact, 4 sorties créées → 4
//! attachées. Ce n'est pas une limite de Windows ni de SudoVDA, c'est un
//! défaut de code.
//!
//! **La règle, en deux chemins et dans cet ordre.** ① DÉSIGNER la sortie par
//! ce qu'on a donné au pilote (voir `moniteurs_virtuels::config_affichage`) ;
//! ② à défaut seulement, le REPLI historique — la différence d'ensembles.
//!
//! 🔴 **LE REPLI N'EST PAS MORT ET NE DOIT PAS ÊTRE RETIRÉ.** Il court dès
//! que la désignation ne rend rien : pilote sans adaptateur connu, CCD muette
//! ou en erreur, cible pas encore dans un chemin actif, paire ambiguë. C'est
//! lui qui garantit qu'une hypothèse fausse sur `identifiant_cible` — que
//! `sudovda.rs` déclare lui-même « non confirmée » — dégrade vers le
//! comportement CONNU au lieu de casser. Sans lui, la voie retenue n'aurait
//! pas été livrable avant d'avoir été mesurée sur la VM.
//!
//! Hors `#[cfg(windows)]`, dans son propre fichier plutôt que dans
//! `placement.rs` : ce dernier était à 441 lignes, et l'y poser l'aurait
//! amené à ~496 — la marge que ce dépôt a mesuré six fois se reperdre, une
//! fois le jour même dans la branche qui l'avait gagnée.

use std::sync::OnceLock;

use crate::sortie_dxgi::SortieDxgi;

/// Le chemin ① (DÉSIGNER) est-il armé ?
///
/// **`SORTIE_DESIGNEE=0` DÉSARME ; une simple PRÉSENCE n'active pas** —
/// convention de `PLEIN_ECRAN`, `AUDIO`, `SUPERVISEUR`, `CAPTEUR`,
/// `PART_SONDAGE`, `PRESSE_PAPIER`, `APPS` et `PONT_ECRITURE`, et pour la même
/// raison : tester `is_ok()` armerait le mécanisme chez qui écrit
/// `SORTIE_DESIGNEE=0` pour le couper.
///
/// 🔴 **VARIABLE DE BANC, JAMAIS UNE CONFIGURATION LIVRÉE**, même statut que
/// `PART_SONDAGE`, `PONT_ECRITURE`, `PONT_CACHE` et `PRESSE_PAPIER_GARDE`.
/// Désarmée, elle rend **exactement le produit d'avant le lot 32** : c'est le
/// bras ROUGE de la recette, et il existe parce que « le bras rouge est le
/// binaire d'avant » n'est pas reproductible — dans trois semaines ce binaire
/// n'existe plus.
///
/// **Le prédicat est RÉUTILISÉ, pas recopié** : `crate::apps::desarme` porte
/// déjà exactement cet argument, et le précédent est `ICONES` (G2) puis
/// `APPS_SURVEILLANCE` (G4).
///
/// ⚠️ **Forcée au DÉMARRAGE du superviseur, pas au premier appariement** —
/// sans quoi la trace ne sortirait qu'à la première fenêtre, donc APRÈS les
/// premiers gestes d'une recette courte. C'est la leçon que `PONT_MESURE` a
/// payée en F4.
pub fn armee() -> bool {
    static ARMEE: OnceLock<bool> = OnceLock::new();
    *ARMEE.get_or_init(|| {
        let armee = !crate::apps::desarme(std::env::var("SORTIE_DESIGNEE").ok().as_deref());
        // Émise SEULEMENT si désarmé : une trace inconditionnelle ferait lire
        // un armement à qui n'en a aucun.
        //
        // 🔴 **ELLE PROUVE QUE LA VARIABLE A ATTEINT LE PROCESSUS, JAMAIS QUE
        // LE MÉCANISME EST COUPÉ** — leçon de P1 sur `PRESSE_PAPIER=0`. Ce qui
        // discrimine est le champ `designee` VIDE du refus, et la fenêtre non
        // servie.
        if !armee {
            tracing::warn!(
                "designation de sortie DESARMEE (SORTIE_DESIGNEE=0) : bras de banc, jamais une configuration livree"
            );
        }
        armee
    })
}

/// Les sorties parmi lesquelles `placement::sortie_pour_viewport` a le droit
/// de choisir.
///
/// `notre_nom` est ce que la désignation a rendu, `None` si elle n'a rien
/// rendu — et **`None` EST le produit d'avant le lot 32, ligne pour ligne**.
/// C'est cette propriété qui rend la rouge du premier test jouable sans avoir
/// à conserver l'ancien binaire.
///
/// ⚠️ **Ce que cette fonction NE fait PAS, à dessein : filtrer sur la taille
/// ou sur `deja_prises`.** Ces deux filtres restent chez
/// `placement::sortie_pour_viewport`, et ils continuent de courir sur la
/// sortie désignée. La désignation **resserre** l'ensemble des candidates,
/// elle ne desserre aucun garde — c'est toute la différence avec un
/// relâchement de la règle d'appariement.
pub fn candidates(
    toutes: &[SortieDxgi],
    notre_nom: Option<&str>,
    avant: &[String],
) -> Vec<SortieDxgi> {
    // ① DÉSIGNER — par ce qu'on a DONNÉ au pilote, jamais par ce qui a changé
    // autour. Insensible au remplacement d'une cible forcée, et accessoirement
    // à toute course : une sortie créée par un tiers (Apollo pilote aussi la
    // configuration d'affichage de cette VM) ne peut plus être prise pour la
    // nôtre, ce que la différence d'ensembles ne garantit pas.
    if let Some(nom) = notre_nom {
        if let Some(notre) = toutes.iter().find(|s| s.attachee_au_bureau && s.nom_sortie == nom) {
            return vec![notre.clone()];
        }
    }

    // ② REPLI — le produit d'hier, mot pour mot. Voir l'en-tête du module :
    // il n'est pas mort, il est ce qui rend une hypothèse fausse inoffensive.
    toutes
        .iter()
        .filter(|s| s.attachee_au_bureau && !avant.contains(&s.nom_sortie))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Rect;

    const NOTRE: &str = "\\\\.\\DISPLAY5";

    fn sortie(nom: &str, largeur: u32, hauteur: u32) -> SortieDxgi {
        SortieDxgi {
            index_adaptateur: 0,
            index_sortie: 0,
            adaptateur: "SudoVDA".into(),
            nom_sortie: nom.to_string(),
            attachee_au_bureau: true,
            rect: Rect { x: 0, y: 0, width: largeur, height: hauteur },
        }
    }

    /// Le montage du lot 30, à la lettre : la cible FORCÉE portait déjà
    /// `\\.\DISPLAY5`, et notre sortie neuve l'a remplacée sur la même source,
    /// donc sous le MÊME nom.
    fn montage_du_lot_30() -> (Vec<SortieDxgi>, Vec<String>) {
        (vec![sortie(NOTRE, 1860, 1080)], vec![NOTRE.to_string()])
    }

    #[test]
    fn la_sortie_qui_remplace_une_cible_forcee_est_retenue() {
        let (toutes, avant) = montage_du_lot_30();
        assert_eq!(candidates(&toutes, Some(NOTRE), &avant).len(), 1);
    }

    /// 🔴 **LA ROUGE, et elle vit dans SON PROPRE test.** Deux assertions dans
    /// un même test ne prouvent que la première — `assert` s'arrête au premier
    /// échec, et ce dépôt a payé qu'une assertion en seconde position n'était
    /// éprouvée par rien.
    ///
    /// Passer `None` **EST** le produit d'avant le lot 32 : il n'avait pas de
    /// paramètre `notre_nom` et courait toujours le repli. Ce test est donc le
    /// TÉMOIN NÉGATIF qui rend le précédent interprétable — il montre, dans le
    /// même relevé, que le montage est bien celui qui échouait, et non un
    /// montage où tout aurait passé de toute façon.
    #[test]
    fn le_produit_d_avant_le_lot_32_rend_un_ensemble_vide_sur_ce_meme_montage() {
        let (toutes, avant) = montage_du_lot_30();
        assert!(
            candidates(&toutes, None, &avant).is_empty(),
            "la différence d'ensembles rend un vecteur VIDE sur une sortie \
             parfaitement utilisable — c'est le défaut mesuré par le lot 30"
        );
    }

    /// La désignation ne court-circuite AUCUN garde de `placement` : elle
    /// choisit parmi quoi chercher, elle ne décide pas du résultat.
    #[test]
    fn la_designation_ne_court_circuite_pas_le_filtre_des_prises() {
        let (toutes, avant) = montage_du_lot_30();
        let candidates = candidates(&toutes, Some(NOTRE), &avant);
        assert!(
            crate::superviseur::placement::sortie_pour_viewport(
                &candidates,
                1860,
                1080,
                &[NOTRE.to_string()],
            )
            .is_none(),
            "une sortie déjà attribuée à une session vivante reste refusée"
        );
    }

    /// Ce que le REPLI protège encore, et que la voie « assouplir la règle »
    /// aurait relâché : un écran PHYSIQUE préexistant ne doit jamais être
    /// choisi. L'inégalité de `sortie_assez_grande` (D10) rend ce risque plus
    /// grand, pas moins — un moniteur 4K convient à n'importe quel viewport.
    #[test]
    fn sans_designation_un_ecran_preexistant_reste_refuse() {
        let physique = "\\\\.\\DISPLAY1";
        let toutes = vec![sortie(physique, 3840, 2160)];
        let avant = vec![physique.to_string()];
        assert!(candidates(&toutes, None, &avant).is_empty());
    }

    /// Une sortie DÉTACHÉE ne peut pas être désignée : le nom peut être exact
    /// et la sortie inutilisable. Sans ce garde, la scrutation rendrait la
    /// main sur une sortie que Windows n'a pas encore attachée, et
    /// `sortie_pour_viewport` refuserait — en consommant l'attente.
    /// Le prédicat de `SORTIE_DESIGNEE`, éprouvé sur le PRÉDICAT et non sur la
    /// variable : `armee()` porte un `OnceLock` que deux tests du même
    /// processus ne pourraient pas réinitialiser, et un test qui pose une
    /// variable d'environnement empoisonnerait ses voisins.
    #[test]
    fn seul_le_zero_desarme_la_designation() {
        assert!(crate::apps::desarme(Some("0")));
        for valeur in [None, Some(""), Some("1"), Some("0 "), Some("oui")] {
            assert!(!crate::apps::desarme(valeur), "{valeur:?} ne doit PAS désarmer");
        }
    }

    #[test]
    fn une_sortie_detachee_n_est_pas_designee() {
        let mut detachee = sortie(NOTRE, 1860, 1080);
        detachee.attachee_au_bureau = false;
        let avant = vec![NOTRE.to_string()];
        assert!(candidates(&[detachee], Some(NOTRE), &avant).is_empty());
    }
}
