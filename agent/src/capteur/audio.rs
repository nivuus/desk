//! La règle : qui porte le son.
//!
//! **Pur, sans aucun `cfg`, sans COM, sans fenêtre** — comme `vivier.rs` et
//! `repartiteur.rs` avant lui. Il ne connaît ni `IAudioClient` ni `HWND` : il
//! reçoit des PID et rend des booléens.
//!
//! **Le sommeil n'entre PAS dans la règle**, et son absence de ce fichier est
//! le meilleur endroit pour le dire : `FenetreAudio` ne porte aucun champ
//! `eveillee`. Une fenêtre endormie (sous-bloc D5) a relâché son encodeur
//! vidéo ; son application peut parfaitement continuer à jouer de la musique,
//! et c'est précisément le cas où l'on veut du son sans image.

use std::cmp::Ordering;
use std::collections::HashMap;

/// Ce que le registre sait d'une fenêtre, du point de vue du son.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FenetreAudio {
    pub session: String,
    /// PID du processus propriétaire de la fenêtre Windows.
    pub pid: u32,
    /// Rang d'arrivée, strictement croissant. Départage deux fenêtres d'un
    /// même processus dont **aucune** n'a jamais été focalisée.
    pub arrivee: u64,
    /// Rang du dernier focus reçu, `0` si cette session n'a jamais été
    /// focalisée. **Un rang, pas un horodatage** : un `Instant` n'est pas
    /// comparable entre processus et n'apporterait rien ici.
    pub dernier_focus: u64,
    /// Cette fenêtre ne peut pas porter le son en ce moment.
    ///
    /// Vrai quand sa capture WASAPI est morte — dix erreurs de lecture
    /// consécutives (`crate::audio::LECTURES_ECHOUEES_MAX`) — et qu'elle
    /// observe son répit de réarmement.
    ///
    /// ⚠️ **Un `bool`, jamais un `Instant`.** L'expiration du répit vit dans le
    /// registre, qui a l'horloge ; ce module garde sa doctrine — « un rang, pas
    /// un horodatage » — et reste éprouvable sans horloge.
    pub inapte: bool,
}

/// Rend, pour chaque fenêtre, si elle porte le son.
///
/// **Une entrée par fenêtre, y compris les muettes** : le registre a besoin du
/// `false` pour ordonner de se taire à celle qui portait le son l'instant
/// d'avant.
pub fn arbitrer(fenetres: &[FenetreAudio]) -> Vec<(String, bool)> {
    let mut porteur: HashMap<u32, &FenetreAudio> = HashMap::new();
    for f in fenetres {
        // Une inapte n'est jamais CANDIDATE. Elle reçoit quand même son
        // verdict plus bas, qui vaudra `false` : c'est ce `false` qui ordonne
        // de se taire à celle qui portait le son l'instant d'avant.
        if f.inapte {
            continue;
        }
        match porteur.get(&f.pid) {
            Some(actuel) if !l_emporte(f, actuel) => {}
            _ => {
                porteur.insert(f.pid, f);
            }
        }
    }

    fenetres
        .iter()
        .map(|f| {
            let actif = porteur.get(&f.pid).is_some_and(|p| p.session == f.session);
            (f.session.clone(), actif)
        })
        .collect()
}

/// `candidat` l'emporte-t-il sur `actuel` au sein de leur groupe de PID ?
///
/// Le focus le plus RÉCENT prime ; à égalité — deux fenêtres jamais focalisées,
/// donc `dernier_focus == 0` toutes les deux — la PREMIÈRE arrivée. Le sommeil
/// n'entre pas dans la comparaison, et aucun champ ne le porte.
fn l_emporte(candidat: &FenetreAudio, actuel: &FenetreAudio) -> bool {
    match candidat.dernier_focus.cmp(&actuel.dernier_focus) {
        Ordering::Greater => true,
        Ordering::Less => false,
        Ordering::Equal => candidat.arrivee < actuel.arrivee,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fenetre(session: &str, pid: u32, arrivee: u64, dernier_focus: u64) -> FenetreAudio {
        FenetreAudio { session: session.into(), pid, arrivee, dernier_focus, inapte: false }
    }

    fn porteurs(fenetres: &[FenetreAudio]) -> Vec<String> {
        let mut noms: Vec<String> = arbitrer(fenetres)
            .into_iter()
            .filter(|(_, actif)| *actif)
            .map(|(session, _)| session)
            .collect();
        noms.sort();
        noms
    }

    #[test]
    fn une_fenetre_seule_de_son_processus_porte_le_son() {
        let f = vec![fenetre("a", 100, 1, 0)];
        assert_eq!(porteurs(&f), vec!["a".to_string()]);
    }

    #[test]
    fn deux_processus_distincts_portent_chacun_le_leur() {
        // Le cas nominal du produit : une application par fenêtre.
        let f = vec![fenetre("a", 100, 1, 0), fenetre("b", 200, 2, 0)];
        assert_eq!(porteurs(&f), vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn deux_fenetres_d_un_meme_processus_jamais_focalisees_la_premiere_arrivee_porte() {
        let f = vec![fenetre("a", 100, 1, 0), fenetre("b", 100, 2, 0)];
        assert_eq!(porteurs(&f), vec!["a".to_string()]);
    }

    #[test]
    fn le_focus_prend_le_son_a_sa_voisine_du_meme_processus() {
        // "b" arrive après "a" et prend le focus : le son bascule.
        let f = vec![fenetre("a", 100, 1, 0), fenetre("b", 100, 2, 7)];
        assert_eq!(porteurs(&f), vec!["b".to_string()]);
    }

    #[test]
    fn un_groupe_qui_perd_tout_focus_garde_son_son_sur_la_derniere_focalisee() {
        // C'est la règle 3 de la spec, et elle n'est pas cosmétique :
        // `focalisee` est GLOBAL — au plus une fenêtre focalisée sur toute la
        // session. Cliquer sur une fenêtre d'un AUTRE processus fait perdre le
        // focus à tout ce groupe, et sans cette règle son son se couperait.
        let f = vec![
            fenetre("a", 100, 1, 3),
            fenetre("b", 100, 2, 7),
            fenetre("etranger", 200, 3, 9),
        ];
        assert_eq!(porteurs(&f), vec!["b".to_string(), "etranger".to_string()]);
    }

    #[test]
    fn l_oubli_du_porteur_fait_passer_le_son_a_la_suivante_du_groupe() {
        // Le registre retire "b" (canal rompu, fermeture) et rappelle
        // `arbitrer` sur ce qui reste : "a" doit reprendre le son, sans quoi
        // le groupe deviendrait définitivement muet.
        let f = vec![fenetre("a", 100, 1, 3)];
        assert_eq!(porteurs(&f), vec!["a".to_string()]);
    }

    #[test]
    fn la_decision_est_rendue_pour_chaque_session_meme_muette() {
        // `arbitrer` rend une entrée par fenêtre, pas seulement pour les
        // porteuses : le registre a besoin du `false` pour envoyer l'ordre de
        // se taire à celle qui portait le son juste avant.
        let f = vec![fenetre("a", 100, 1, 0), fenetre("b", 100, 2, 0)];
        let decisions = arbitrer(&f);
        assert_eq!(decisions.len(), 2);
        assert!(decisions.contains(&("a".to_string(), true)));
        assert!(decisions.contains(&("b".to_string(), false)));
    }

    #[test]
    fn aucune_fenetre_rend_aucune_decision() {
        assert!(arbitrer(&[]).is_empty());
    }

    #[test]
    fn une_fenetre_inapte_ne_porte_jamais_le_son() {
        // Le cas du leg 1 : sa capture WASAPI est morte après dix échecs
        // consécutifs. Elle ne doit plus être élue, sans quoi le groupe entier
        // reste muet — c'est l'état d'avant D9.
        let fenetres = vec![
            FenetreAudio { session: "w-1".into(), pid: 42, arrivee: 1, dernier_focus: 9, inapte: true },
            FenetreAudio { session: "w-2".into(), pid: 42, arrivee: 2, dernier_focus: 0, inapte: false },
        ];
        let verdict = arbitrer(&fenetres);
        assert_eq!(verdict, vec![("w-1".into(), false), ("w-2".into(), true)]);
    }

    #[test]
    fn une_inapte_recoit_quand_meme_son_verdict_false() {
        // Le registre a besoin de ce `false` pour ordonner de se taire à celle
        // qui portait le son l'instant d'avant.
        let fenetres = vec![FenetreAudio {
            session: "w-1".into(), pid: 42, arrivee: 1, dernier_focus: 1, inapte: true,
        }];
        assert_eq!(arbitrer(&fenetres), vec![("w-1".into(), false)]);
    }

    #[test]
    fn un_groupe_entierement_inapte_reste_muet() {
        // Aucune voisine à promouvoir : c'est le cas MAJORITAIRE — une
        // application, une fenêtre. Le réarmement après répit est le seul remède,
        // et il vit dans le registre, pas ici.
        let fenetres = vec![
            FenetreAudio { session: "w-1".into(), pid: 42, arrivee: 1, dernier_focus: 0, inapte: true },
            FenetreAudio { session: "w-2".into(), pid: 42, arrivee: 2, dernier_focus: 0, inapte: true },
        ];
        assert_eq!(arbitrer(&fenetres), vec![("w-1".into(), false), ("w-2".into(), false)]);
    }

    #[test]
    fn l_inaptitude_d_un_groupe_ne_touche_pas_un_autre_pid() {
        let fenetres = vec![
            FenetreAudio { session: "w-1".into(), pid: 42, arrivee: 1, dernier_focus: 0, inapte: true },
            FenetreAudio { session: "w-2".into(), pid: 77, arrivee: 2, dernier_focus: 0, inapte: false },
        ];
        assert_eq!(arbitrer(&fenetres), vec![("w-1".into(), false), ("w-2".into(), true)]);
    }
}
