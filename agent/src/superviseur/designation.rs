//! Among which DXGI outputs the supervisor is allowed to pair a
//! window — and why the question does not reduce to "which one just
//! appeared".
//!
//! 🔴 **THE DEFECT THIS MODULE EXISTS TO CLOSE (batch 30, measured on the
//! VM).** Without QEMU's VGA, session 1 starts with zero PnP monitors
//! but ONE display (`\\.\DISPLAY5`), whose active target carries
//! `statusFlags = 0x11` (`IN_USE | FORCED_AVAILABILITY_SYSTEM`): a **forced
//! target**, which Windows fabricates when it has no display left. The
//! first virtual monitor created **REPLACES this forced target on the SAME
//! source** — it therefore inherits the same GDI name, and never "appears". The
//! product only pairing among outputs that APPEARED, it refused the window
//! ("no display output can serve this window") and handed back to the
//! driver a perfectly usable output. The loop never reached
//! the second window.
//!
//! ⚠️ **The hypothesis "the polluted registry prevents attachment" was
//! REFUTED** by the same batch: intact registry, 4 outputs created → 4
//! attached. It is not a limit of Windows nor of SudoVDA, it is a
//! code defect.
//!
//! **The rule, in two paths and in this order.** ① DESIGNATE the output by
//! what we gave the driver (see `moniteurs_virtuels::config_affichage`);
//! ② only failing that, the historical FALLBACK — the set difference.
//!
//! 🔴 **THE FALLBACK IS NOT DEAD AND MUST NOT BE REMOVED.** It runs as soon
//! as designation returns nothing: driver without a known adapter, mute or failing
//! CCD, target not yet in an active path, ambiguous pair. It is
//! what guarantees that a wrong hypothesis about `identifiant_cible` — which
//! `sudovda.rs` itself declares "not confirmed" — degrades to the
//! KNOWN behaviour instead of breaking. Without it, the chosen path would
//! not have been shippable before being measured on the VM.
//!
//! Outside `#[cfg(windows)]`, in its own file rather than in
//! `placement.rs`: the latter was at 441 lines, and putting it there would have
//! brought it to ~496 — the margin this repository measured being lost again six times, once
//! the very same day in the branch that had gained it.

use std::sync::OnceLock;

use crate::sortie_dxgi::SortieDxgi;

/// Is path ① (DESIGNATE) armed?
///
/// **`SORTIE_DESIGNEE=0` DISARMS; mere PRESENCE does not enable** —
/// convention of `PLEIN_ECRAN`, `AUDIO`, `SUPERVISEUR`, `CAPTEUR`,
/// `PART_SONDAGE`, `PRESSE_PAPIER`, `APPS` and `PONT_ECRITURE`, and for the same
/// reason: testing `is_ok()` would arm the mechanism for whoever writes
/// `SORTIE_DESIGNEE=0` to cut it.
///
/// 🔴 **BENCH VARIABLE, NEVER A SHIPPED CONFIGURATION**, same status as
/// `PART_SONDAGE`, `PONT_ECRITURE`, `PONT_CACHE` and `PRESSE_PAPIER_GARDE`.
/// Disarmed, it gives **exactly the product from before batch 32**: it is the
/// acceptance run's RED arm, and it exists because "the red arm is the
/// earlier binary" is not reproducible — in three weeks that binary
/// no longer exists.
///
/// **The predicate is REUSED, not copied**: `crate::apps::desarme` already carries
/// exactly this argument, and the precedent is `ICONES` (G2) then
/// `APPS_SURVEILLANCE` (G4).
///
/// ⚠️ **Forced at supervisor STARTUP, not at the first pairing** —
/// else the trace would appear only with the first window, i.e. after the
/// first steps of a short acceptance run. `PONT_MESURE` taught this
/// lesson in F4.
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
/// `placement::sortie_pour_viewport`.
///
/// ❌ **CETTE DOC A DIT « la désignation resserre l'ensemble des candidates,
/// elle ne desserre aucun garde — c'est toute la différence avec un
/// relâchement de la règle d'appariement » JUSQU'AU 31 AOÛT 2026, ET C'EST
/// DEVENU FAUX.** Le nom désigné est désormais passé à
/// `placement::sortie_pour_viewport`, qui **exempte cette sortie-là du critère
/// de TAILLE** : le pilote ne fait pas naître la sortie à la taille demandée
/// (mesuré en production, 1614×1080 demandé, 1428×1080 rendu, huit refus en
/// boucle et plus aucune fenêtre servie). La désignation resserre toujours
/// l'ensemble ; elle desserre désormais **un** garde, nommément.
///
/// **`attachee_au_bureau` et `deja_prises` continuent de courir sur la sortie
/// désignée**, et le second est ce qui empêche deux fenêtres de montrer la
/// même image — voir la doc de `placement::sortie_pour_viewport`, qui porte
/// la mesure et le raisonnement.
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
        if let Some(notre) = toutes
            .iter()
            .find(|s| s.attachee_au_bureau && s.nom_sortie == nom)
        {
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
            rect: Rect {
                x: 0,
                y: 0,
                width: largeur,
                height: hauteur,
            },
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

    /// La désignation ne court-circuite pas le garde des PRISES — et c'est
    /// celui qui compte, depuis que le critère de taille, lui, est exempté
    /// pour la sortie désignée (31 août 2026). Sans ce test, l'exemption
    /// aurait pu s'étendre en silence à `deja_prises`, et deux fenêtres
    /// auraient montré la même image.
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
                None,
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
            assert!(
                !crate::apps::desarme(valeur),
                "{valeur:?} ne doit PAS désarmer"
            );
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
