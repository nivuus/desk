//! Distribution des ordres audio — la branche de `capteur::audio::arbitrer`
//! sur le registre de `capteur::sommeil`.
//!
//! **Extrait de `sommeil.rs` et non ajouté dedans**, exactement comme
//! `parts.rs` : ce fichier-là est proche de son plafond, et la règle du dépôt
//! veut qu'une addition substantielle s'accompagne d'une extraction.
//!
//! Il ne s'appelle pas `audio` : ce nom est déjà pris par le module qui porte
//! la RÈGLE pure. Celui-ci ne porte que sa BRANCHE sur ce registre — même
//! distinction que `repartiteur` / `parts`.

use std::sync::MutexGuard;

use crate::capteur::audio::{arbitrer, FenetreAudio};

use super::file::Envoi;
use super::{oublier, Etat, Message};

/// Recalcule qui porte le son et n'envoie que ce qui a changé.
///
/// **Appelée APRÈS `distribuer_les_parts`**, en dernière position de tous les
/// chemins d'entrée du registre. L'ordre importe peu vis-à-vis des parts — le
/// son et le débit sont orthogonaux — mais un ordre unique et documenté vaut
/// mieux qu'un ordre qui dépend de l'appelant.
///
/// Un canal rompu ici est retiré par `oublier`, exactement comme dans
/// `distribuer` et `distribuer_les_parts` : c'est le point de passage unique du
/// registre, et le contourner laisserait des entrées fantômes au vivier.
pub(super) fn distribuer_l_audio(garde: &mut MutexGuard<'static, Etat>) {
    let fenetres: Vec<FenetreAudio> = garde
        .canaux
        .keys()
        .filter_map(|session| {
            // Une session sans PID ou sans rang d'arrivée connu n'existe pas
            // : `inscrire` pose les deux ensemble. Le `filter_map` est un
            // filet, pas un cas nominal.
            //
            // ⚠️ `arrivee` et `dernier_focus` ne sont PAS symétriques malgré
            // l'air qu'elles en ont : `0` est le sentinelle DOCUMENTÉ de
            // `dernier_focus` (« jamais focalisée », la priorité la plus
            // basse — voir `FenetreAudio`), donc `unwrap_or(0)` y est le bon
            // repli. Pour `arrivee`, `0` BAT toute fenêtre réelle de son
            // groupe de PID (`l_emporte` compare `candidat.arrivee <
            // actuel.arrivee`) : un repli à 0 y serait donc le pire choix
            // possible, pas un choix neutre. Aucun chemin vivant ne produit
            // ce cas — `inscrire` pose toujours `arrivees` avant tout appel
            // à `distribuer_l_audio` —, mais le rendre par `?` plutôt que par
            // un défaut le rend impossible à mal lire.
            let pid = *garde.pids.get(session)?;
            let arrivee = *garde.arrivees.get(session)?;
            Some(FenetreAudio {
                session: session.clone(),
                pid,
                arrivee,
                dernier_focus: garde.derniers_focus.get(session).copied().unwrap_or(0),
                inapte: garde.inaptes.contains_key(session),
            })
        })
        .collect();

    let decisions = arbitrer(&fenetres);

    let vivantes: std::collections::HashSet<&String> =
        decisions.iter().map(|(session, _)| session).collect();
    garde.derniers_audio.retain(|session, _| vivantes.contains(session));

    // Les ordres de SE TAIRE partent d'abord, les ordres de PORTER ensuite.
    // Cet ordre RÉDUIT la fenêtre de recouvrement, il ne la ferme pas : les
    // deux ordres empruntent deux canaux DISTINCTS (`file.rs` depuis le
    // 25 août 2026, `mpsc` avant lui), lus chacun par le
    // fil de SA fenêtre. L'ordre d'ENVOI est garanti, pas celui de
    // TRAITEMENT — si le fil qui doit se taire est déclassé par
    // l'ordonnanceur avant de lire son message, les deux fenêtres restent
    // audibles ensemble le temps qu'il reprenne la main. La borne réelle est
    // donc l'ordonnancement des deux fils, pas ce canal.
    let (a_porter, a_taire): (Vec<_>, Vec<_>) =
        decisions.into_iter().partition(|(_, actif)| *actif);

    // ⚠️ Un canal rompu ici n'est PAS ré-arbitré dans la même passe : si la
    // session rompue portait le son de son groupe, sa voisine ne le
    // reprendra qu'au TOUR DE ROUE SUIVANT — borne `PERIODE_REARBITRAGE`,
    // 250 ms. Même résidu, et même borne, que le chemin `rompus` de
    // `distribuer_les_parts` (voir la doc du `Message` juste au-dessus), mais
    // le symptôme n'est pas de même nature : là, un plancher de débit
    // transitoire ; ici, un silence PERCEPTIBLE par l'utilisateur pendant
    // jusqu'à 250 ms.
    let mut rompus = Vec::new();
    for (session, actif) in a_taire.into_iter().chain(a_porter) {
        // ⚠️ La remise à zéro vivait ICI jusqu'à D10, sur la DÉCISION
        // d'arbitrage — et la revue transverse de D9 a établi qu'elle ne
        // pouvait alors PAS mordre dans le cas majoritaire : pour une fenêtre
        // seule de son groupe de PID, la sortie de répit la rend
        // automatiquement porteuse, donc remet le compteur à zéro à chaque
        // tour. Le garde-fou était décoratif. Il repart désormais de
        // `sommeil::signaler_audio_vivant`, sur une PREUVE de son.
        if garde.derniers_audio.get(&session) == Some(&actif) {
            continue;
        }
        let issue = match garde.canaux.get(&session) {
            Some(canal) => canal.envoyer(Message::Audio { actif }),
            None => Envoi::Rompu,
        };
        match issue {
            Envoi::Depose(_) => {
                garde.derniers_audio.insert(session, actif);
            }
            // 🔴 REFUSÉ : ON NE MÉMORISE PAS. Même correctif que
            // `parts::distribuer_les_parts`, et **le plus coûteux des deux** :
            // mémoriser un ordre audio jamais parti fait juger l'état « déjà
            // livré » par le garde d'écrasement en tête de boucle, et la
            // fenêtre reste sur son état précédent — donc potentiellement
            // MUETTE, ou audible en même temps qu'une voisine, **sans borne**.
            // ⚠️ Le commentaire ci-dessus s'inquiète d'un silence perceptible
            // pendant jusqu'à 250 ms sur le chemin du canal rompu ; ce
            // résidu-ci n'était borné par rien du tout.
            //
            // ⚠️ **Et surtout PAS `rompus.push`** : la session est VIVANTE.
            Envoi::Refuse => {}
            Envoi::Rompu => rompus.push(session),
        }
    }

    let mut ordres_du_retrait = Vec::new();
    for session in rompus {
        ordres_du_retrait.extend(oublier(garde, &session));
    }
    if !ordres_du_retrait.is_empty() {
        super::distribuer(garde, ordres_du_retrait);
    }
}

#[cfg(test)]
mod tests {
    use crate::capteur::sommeil::file::{ReceveurSession, PROFONDEUR_MAX};
    use crate::capteur::sommeil::tests::verrouiller_pour_le_test;
    use crate::capteur::sommeil::{etat, inscrire, retirer, signaler, Message};
    use crate::capteur::vivier::Ordre;

    /// Dernier ordre audio reçu sur un canal, en vidant ce qui s'y trouve.
    ///
    /// **Vit ici, pas dans `sommeil::tests`** : `sommeil.rs` est proche de son
    /// plafond de 500 lignes, et cette suite couvre la branche de CE
    /// module — même montage que `parts::tests`, qui importe
    /// `verrouiller_pour_le_test` de la même façon plutôt que d'ajouter ses
    /// propres tests au fichier parent.
    fn dernier_audio(canal: &ReceveurSession) -> Option<bool> {
        canal
            .vider()
            .into_iter()
            .filter_map(|m| match m {
                Message::Audio { actif } => Some(actif),
                _ => None,
            })
            .last()
    }

    #[test]
    fn deux_fenetres_d_un_meme_pid_se_disputent_le_son_et_le_focus_tranche() {
        let _verrou = verrouiller_pour_le_test();
        let (a, generation_a) = inscrire("t9-a", 4242);
        let (b, generation_b) = inscrire("t9-b", 4242);

        // Aucune focalisée : la première arrivée porte le son.
        assert_eq!(dernier_audio(&a), Some(true), "la premiere arrivee porte le son");
        assert_eq!(dernier_audio(&b), Some(false), "la seconde se tait");

        // "b" prend le focus : le son bascule, et "a" reçoit l'ordre de se
        // taire — sans quoi les deux seraient audibles en même temps.
        signaler("t9-b", true, true);
        assert_eq!(dernier_audio(&b), Some(true), "la focalisee prend le son");
        assert_eq!(dernier_audio(&a), Some(false), "la precedente porteuse se tait");

        // "b" disparaît : "a" doit reprendre le son, sinon le groupe devient
        // definitivement muet.
        retirer("t9-b", generation_b);
        assert_eq!(dernier_audio(&a), Some(true), "le son revient a la survivante");

        retirer("t9-a", generation_a);
    }

    #[test]
    fn deux_pid_distincts_portent_chacun_leur_son() {
        let _verrou = verrouiller_pour_le_test();
        let (a, generation_a) = inscrire("t9-c", 111);
        let (b, generation_b) = inscrire("t9-d", 222);
        assert_eq!(dernier_audio(&a), Some(true));
        assert_eq!(dernier_audio(&b), Some(true));
        retirer("t9-c", generation_a);
        retirer("t9-d", generation_b);
    }

    #[test]
    fn un_ordre_audio_inchange_n_est_pas_reemis() {
        // Sans le filtre d'écrasement, le tour de roue (250 ms) enverrait
        // quatre ordres par seconde et par fenêtre, à vie. Même rempart que
        // `dernieres_parts`.
        let _verrou = verrouiller_pour_le_test();
        let (a, generation) = inscrire("t9-e", 333);
        let _ = a.vider();
        signaler("t9-e", true, true);
        let ordres: Vec<Message> = a
            .vider()
            .into_iter()
            .filter(|m| matches!(m, Message::Audio { .. }))
            .collect();
        assert!(ordres.is_empty(), "ordre audio inchange reemis : {ordres:?}");
        retirer("t9-e", generation);
    }
    /// 🔴 LA ROUGE DU CRITIQUE DU ROUND 1, CÔTÉ AUDIO — LE MÊME PATRON QUE
    /// `parts.rs`, ET LE PLUS COÛTEUX DES DEUX : une fenêtre pouvait rester
    /// MUETTE indéfiniment.
    ///
    /// `envoyer(...).is_ok()` valait « livré » sous `mpsc` ; depuis la file
    /// bornée il ne vaut plus que « pas déconnecté ». Un `Audio` refusé était
    /// donc écrit dans `derniers_audio`, et le garde d'écrasement en tête de
    /// boucle (`if derniers_audio.get(&session) == Some(&actif) { continue }`)
    /// supprimait toute réémission — la fenêtre restait sur son état audio
    /// précédent **sans borne**.
    ///
    /// ⚠️ **Le commentaire de `distribuer_l_audio` s'inquiète d'un silence
    /// perceptible pendant jusqu'à 250 ms sur un autre chemin ; ce résidu-ci
    /// n'était borné par rien.**
    ///
    /// **Ce test échoue sur sa DERNIÈRE assertion avant le correctif.**
    #[test]
    fn un_ordre_audio_refuse_n_est_pas_memorise_et_repart_au_tour_suivant() {
        let _verrou = verrouiller_pour_le_test();
        let (a, generation_a) = inscrire("t9-refus-a", 4300);
        // "a" est seule de son PID : elle porte le son, et `derniers_audio`
        // retient `true`.
        assert_eq!(dernier_audio(&a), Some(true), "précondition : la seule du PID porte le son");

        // Sature la file de "a" par des messages INCOALESCABLES.
        {
            let garde = etat();
            let emetteur = garde.canaux.get("t9-refus-a").expect("la session est inscrite");
            for _ in 0..PROFONDEUR_MAX {
                let _ = emetteur.envoyer(Message::Sommeil(Ordre::Reveiller));
            }
        }

        // "b" arrive sur le MÊME PID et prend le focus : "a" doit recevoir
        // l'ordre de se taire — qui est REFUSÉ, sa file étant pleine.
        let (b, generation_b) = inscrire("t9-refus-b", 4300);
        signaler("t9-refus-b", true, true);

        // "a" reprend sa lecture. Aucun ordre audio ne s'y trouve.
        let recus = a.vider();
        assert!(
            !recus.iter().any(|m| matches!(m, Message::Audio { .. })),
            "précondition : l'ordre de se taire n'a PAS été livré : {recus:?}"
        );

        // Le tour suivant : l'ordre doit repartir, sans quoi "a" reste
        // audible en même temps que "b", pour toujours.
        {
            let mut garde = etat();
            super::distribuer_l_audio(&mut garde);
        }
        assert_eq!(
            dernier_audio(&a),
            Some(false),
            "un ordre audio refusé doit être RÉÉMIS au tour suivant : il n'a jamais été livré"
        );

        retirer("t9-refus-b", generation_b);
        retirer("t9-refus-a", generation_a);
        drop(b);
    }
}

