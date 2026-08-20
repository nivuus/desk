//! Distribution du presse-papier de la VM — la branche de
//! `crate::presse_papier::Sondeur` sur le registre de `capteur::sommeil`.
//!
//! **Extrait de `sommeil.rs` et non ajouté dedans**, exactement comme
//! `parts.rs` et `porteurs.rs` : ce fichier-là est proche de son plafond, et
//! la règle du dépôt veut qu'une addition substantielle s'accompagne d'une
//! extraction.
//!
//! Il ne s'appelle pas comme le module racine `crate::presse_papier` par
//! hasard, mais il n'en porte pas la même chose : celui-là porte la RÈGLE
//! pure (normaliser, borner, comparer au dernier émis) ; celui-ci ne porte que
//! sa BRANCHE sur ce registre. Même distinction que `repartiteur` / `parts`
//! et que `audio` / `porteurs`.

use std::sync::MutexGuard;

use crate::presse_papier::Annonce;

use super::{distribuer as distribuer_les_ordres, oublier, Etat, Message};

/// Pousse une annonce de presse-papier à **toutes** les fenêtres inscrites.
///
/// **Toutes, et non la seule focalisée** : le presse-papier est une ressource
/// GLOBALE à la session Windows, chaque fenêtre navigateur a son propre
/// presse-papier local à alimenter, et c'est le client qui décide s'il écrit
/// (`PressePapierLocal::aEcrire`, qui prend le focus en argument). Décider ici
/// priverait une fenêtre non focalisée d'un contenu qu'elle devra écrire dès
/// qu'elle reprendra le focus — le dépôt différé de D3.
///
/// **Aucun filtre d'écrasement ici**, à la différence de
/// `parts::distribuer_les_parts` : le `Sondeur` n'appelle cette fonction qu'au
/// CHANGEMENT — c'est lui qui porte le garde d'égalité de contenu (garde n°2
/// de D5) et le garde de refus répété. Refiltrer ici doublerait une décision
/// déjà prise, et la doublerait *mal* : le registre ne connaît pas le texte
/// précédemment émis, et un second garde par session divergerait du premier
/// dès qu'une fenêtre s'inscrit ou se retire.
///
/// Un canal rompu passe par `oublier` — **le point de passage unique du
/// registre**, jamais un `remove` direct : c'est la leçon de M1 (revue finale
/// de branche de D6), où `focalisee` avait été oublié par deux chemins qui
/// retiraient à la main.
pub(super) fn distribuer(garde: &mut MutexGuard<'static, Etat>, annonce: Annonce) {
    let (texte, octets) = match annonce {
        Annonce::Texte(texte) => {
            let octets = texte.len() as u32;
            (Some(texte), octets)
        }
        Annonce::Refus { octets } => (None, octets),
    };

    // ⚠️ **UNE SEULE TRACE, ET JAMAIS LE TEXTE** (D-P1-7). Le contenu du
    // presse-papier est une ressource privée, et un journal versé dans git est
    // public au dépôt : on ne journalise que sa TAILLE et le fait qu'il
    // s'agisse d'un refus. Une seule, parce que deux traces au même instant se
    // comptent comme deux événements — piège maison de D6, où chaque
    // changement de barreau produisait deux lignes au même horodatage et
    // faisait valoir le double à tous les compteurs de recette.
    tracing::info!(octets, refus = texte.is_none(), "presse-papier de la VM");

    let sessions: Vec<String> = garde.canaux.keys().cloned().collect();
    let mut rompus = Vec::new();
    for session in sessions {
        let envoye = match garde.canaux.get(&session) {
            Some(canal) => canal
                .send(Message::PressePapier { texte: texte.clone(), octets })
                .is_ok(),
            None => false,
        };
        if !envoye {
            rompus.push(session);
        }
    }

    let mut ordres_du_retrait = Vec::new();
    for session in rompus {
        ordres_du_retrait.extend(oublier(garde, &session));
    }
    if !ordres_du_retrait.is_empty() {
        distribuer_les_ordres(garde, ordres_du_retrait);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::Receiver;

    use crate::capteur::sommeil::tests::{premier_ordre, verrouiller_pour_le_test};
    use crate::capteur::sommeil::{etat, inscrire, retirer, signaler, Message};
    use crate::capteur::vivier::Ordre;
    use crate::presse_papier::Annonce;

    /// Le dernier presse-papier reçu sur un canal, en vidant ce qui s'y
    /// trouve : parts et ordres de sommeil s'y intercalent librement.
    fn dernier_presse_papier(canal: &Receiver<Message>) -> Option<(Option<String>, u32)> {
        canal
            .try_iter()
            .filter_map(|m| match m {
                Message::PressePapier { texte, octets } => Some((texte, octets)),
                _ => None,
            })
            .last()
    }

    /// **Toutes les fenêtres reçoivent, pas seulement la focalisée** : chaque
    /// fenêtre navigateur a son propre presse-papier local, et c'est le client
    /// qui décide s'il écrit maintenant ou au retour du focus.
    #[test]
    fn une_annonce_de_texte_part_vers_toutes_les_sessions_inscrites() {
        let _verrou = verrouiller_pour_le_test();
        let (canal_a, generation_a) = inscrire("pp-a", 7100);
        let (canal_b, generation_b) = inscrire("pp-b", 7101);
        signaler("pp-a", true, true);

        super::distribuer(&mut etat(), Annonce::Texte("bonjour".to_string()));

        assert_eq!(
            dernier_presse_papier(&canal_a),
            Some((Some("bonjour".to_string()), 7)),
            "la fenêtre focalisée doit recevoir le texte"
        );
        assert_eq!(
            dernier_presse_papier(&canal_b),
            Some((Some("bonjour".to_string()), 7)),
            "la fenêtre NON focalisée aussi : c'est le client qui décide d'écrire"
        );

        retirer("pp-a", generation_a);
        retirer("pp-b", generation_b);
    }

    /// Un refus voyage par la même variante, `texte` à `None` et `octets`
    /// portant la taille refusée — c'est ce qui permet au bandeau du
    /// navigateur de dire *combien* plutôt que « trop grand ».
    #[test]
    fn un_refus_part_sans_texte_mais_avec_sa_taille() {
        let _verrou = verrouiller_pour_le_test();
        let (canal, generation) = inscrire("pp-refus", 7200);

        super::distribuer(&mut etat(), Annonce::Refus { octets: 100_000 });

        assert_eq!(
            dernier_presse_papier(&canal),
            Some((None, 100_000)),
            "un refus doit partir, et porter sa taille"
        );

        retirer("pp-refus", generation);
    }

    /// Même remède, et pour la même raison, que
    /// `un_canal_rompu_detecte_par_les_parts_est_retire_du_vivier` : un fil de
    /// fenêtre qui meurt sans passer par `retirer` (une panique court-circuite
    /// le point de passage unique de `Fenetre::servir`) laisse une entrée dans
    /// `canaux` ET dans le vivier, où elle occuperait une place d'encodeur
    /// pour toute la vie du processus. `oublier` — jamais un `remove` direct —
    /// est ce qui retire les deux.
    ///
    /// ⚠️ **Ce test observe le vivier DIRECTEMENT, et c'est ce qui le rend
    /// discriminant.** Une première rédaction jugeait sur « une session neuve
    /// arrive-t-elle à s'éveiller » — et elle passait AVEC UN DISTRIBUTEUR
    /// VIDE : `inscrire` et `signaler` appellent tous deux
    /// `parts::distribuer_les_parts`, qui détecte la même rupture par son
    /// propre chemin et libère la place à la place de celui-ci. Le contrôle ne
    /// pouvait donc pas échouer — exactement le patron que ce dépôt paie
    /// depuis D6. Ici, rien ne s'intercale entre la rupture et l'observation.
    #[test]
    fn un_canal_rompu_detecte_par_le_presse_papier_est_retire_du_vivier() {
        let _verrou = verrouiller_pour_le_test();
        let (canal_mort, generation_morte) = inscrire("pp-mort", 7300);
        signaler("pp-mort", true, false);
        assert_eq!(
            premier_ordre(&canal_mort),
            Some(Ordre::Reveiller),
            "pp-mort devrait s'éveiller avant qu'on ne tue son fil"
        );

        // Le fil « meurt » : son récepteur est jeté SANS passer par `retirer`.
        drop(canal_mort);

        // Aucun appel public entre la rupture et l'observation : ni `inscrire`
        // ni `signaler`, qui détecteraient la rupture par le chemin des parts.
        super::distribuer(&mut etat(), Annonce::Texte("bonjour".to_string()));

        assert!(
            !etat().vivier.eveillees().iter().any(|s| s == "pp-mort"),
            "la session dont le canal est rompu doit être retirée du VIVIER, \
             pas seulement de `canaux` : sa place d'encodeur resterait sinon \
             occupée pour toute la vie du processus"
        );

        retirer("pp-mort", generation_morte);
    }
}
