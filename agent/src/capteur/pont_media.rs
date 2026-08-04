//! Pont pur entre les trames de la connexion média du capteur et les `Recu`
//! consommés par `SourceDistante`.
//!
//! **Pas de `#[cfg(windows)]`, extrait de `tube.rs` à dessein** : cette
//! fonction ne connaît ni Windows ni le tube nommé qui la porte — elle prend
//! n'importe quel `R: std::io::Read` — et c'est justement de la logique
//! décisionnelle (traduire un flux d'octets en `Recu`, décider quand
//! abandonner le fil) que la doctrine du module (`capteur.rs`) réserve au
//! code hors `cfg`, testable sur l'hôte. `tube.rs` reste gaté : lui seul
//! ouvre le tube réel et lance le fil qui appelle `lire_le_media`.

use std::io::Read;
use std::sync::mpsc::SyncSender;

use crate::capteur::distante::Recu;
use crate::capteur::protocole::{lire_trame, DepuisCapteur, Trame};

/// Lit la connexion média — et **rien d'autre** : images et états. Une
/// réponse de commande n'y transite pas, elle est lue par `commander` sur la
/// connexion de commandes.
///
/// `pub(crate)`, pas `pub` : seul `tube.rs` l'appelle, depuis le même crate.
pub(crate) fn lire_le_media<R: Read>(mut lecteur: R, images: SyncSender<Recu>) {
    loop {
        let trame = match lire_trame(&mut lecteur) {
            Ok(trame) => trame,
            // Fin de tube : le capteur est parti. Laisser tomber l'émetteur
            // fait rendre `Disconnected` à `SourceDistante`, qui OUVRE SA
            // FENÊTRE DE REPRISE au lieu de clore la session.
            Err(_) => return,
        };
        let envoi = match trame {
            Trame::Image(unite) => images.send(Recu::Image(unite)).is_ok(),
            Trame::Json(octets) => match serde_json::from_slice::<DepuisCapteur>(&octets) {
                Ok(DepuisCapteur::Etat { vivante, epuisee, largeur, hauteur }) => images
                    .send(Recu::Etat { vivante, epuisee, largeur, hauteur })
                    .is_ok(),
                // Point de passage OBLIGÉ pour toute variante de `DepuisCapteur`
                // poussée sur la connexion média : l'oublier ici ne se signale
                // PAS par une erreur de compilation, mais par un fil qui meurt
                // en silence (branche `Ok(autre)` plus bas) au premier message
                // de ce type reçu — et ce fil est celui qui alimente
                // `SourceDistante`, donc la session tombe dans sa fenêtre de
                // reprise sans aucune panne réelle. C'est exactement l'omission
                // qui a échappé à la tâche 7 du sous-bloc D5 : `Sommeil` était
                // câblé de bout en bout côté capteur et côté `SourceDistante`,
                // mais jamais relié ici — et c'est cette absence de couverture
                // par test qui l'a laissée passer, d'où l'extraction de ce
                // fichier hors `#[cfg(windows)]`.
                Ok(DepuisCapteur::Sommeil { endormie, raison }) => {
                    images.send(Recu::Sommeil { endormie, raison }).is_ok()
                }
                // Relié par le correctif de la tâche 5/D6 : `DepuisCapteur::Part`
                // était déjà câblée côté capteur (protocole + fil de fenêtre)
                // mais jamais reliée ICI, exactement l'omission que le
                // commentaire ci-dessus signalait déjà pour `Sommeil` en D5.
                // Sans ce bras, la première part — envoyée par
                // `sommeil::inscrire` dès l'attache, avant la moindre image —
                // tombait dans `Ok(autre)` et tuait ce fil au tout premier
                // message reçu, en conditions de produit et sur toute session.
                Ok(DepuisCapteur::Part { bps }) => images.send(Recu::Part { bps }).is_ok(),
                // Tâche 6, sous-bloc D7 : même point de passage obligé que
                // `Sommeil` et `Part` juste au-dessus — l'oublier ici tuerait
                // ce fil en silence au premier ordre audio reçu.
                Ok(DepuisCapteur::Audio { actif }) => {
                    images.send(Recu::Audio { actif }).is_ok()
                }
                Ok(autre) => {
                    tracing::warn!(?autre, "trame inattendue sur la connexion média, abandonnée");
                    return;
                }
                Err(erreur) => {
                    tracing::warn!(%erreur, "trame illisible du capteur, canal abandonné");
                    return;
                }
            },
        };
        if !envoi {
            return; // la source est partie
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capteur::protocole::{ecrire_image, ecrire_json};
    use crate::h264::AccessUnit;
    use std::sync::mpsc::sync_channel;

    /// Le test qui aurait attrapé l'omission de la tâche 7 : un flux réel
    /// (sérialisé par les fonctions d'écriture de `protocole.rs`, pas des
    /// octets à la main) portant un `Etat`, une image, puis un `Sommeil` doit
    /// ressortir sous forme de trois `Recu`, DANS L'ORDRE, sans que le fil ne
    /// se soit abandonné avant la fin du tampon.
    #[test]
    fn lire_le_media_relaie_etat_image_et_sommeil_dans_l_ordre() {
        let mut tampon = Vec::new();
        ecrire_json(
            &mut tampon,
            &DepuisCapteur::Etat { vivante: true, epuisee: false, largeur: 1280, hauteur: 720 },
        )
        .unwrap();
        ecrire_image(&mut tampon, &AccessUnit { data: vec![1, 2, 3], is_keyframe: true, pts_90k: 42 })
            .unwrap();
        ecrire_json(
            &mut tampon,
            &DepuisCapteur::Sommeil { endormie: true, raison: "masquee".into() },
        )
        .unwrap();

        let (tx, rx) = sync_channel(8);
        // Appelé directement (pas dans un fil) : le tampon en mémoire est
        // épuisé après les trois trames, `lire_trame` y rend alors une erreur
        // de lecture, et la fonction retourne d'elle-même — aucun risque de
        // blocage à éprouver ici.
        lire_le_media(std::io::Cursor::new(tampon), tx);

        assert_eq!(
            rx.recv().unwrap(),
            Recu::Etat { vivante: true, epuisee: false, largeur: 1280, hauteur: 720 }
        );
        match rx.recv().unwrap() {
            Recu::Image(unite) => {
                assert_eq!(unite.pts_90k, 42);
                assert!(unite.is_keyframe);
            }
            autre => panic!("attendu une image, reçu {autre:?}"),
        }
        assert_eq!(
            rx.recv().unwrap(),
            Recu::Sommeil { endormie: true, raison: "masquee".to_string() }
        );
        assert!(
            rx.try_recv().is_err(),
            "aucune trame de plus après la fin du tampon : le fil n'a rien perdu ni rien inventé"
        );
    }

    /// Le test qui aurait attrapé le défaut critique relevé en revue du
    /// sous-bloc D6 : `DepuisCapteur::Part` est la toute première trame
    /// qu'une session reçoit en conditions de produit (`sommeil::inscrire`
    /// l'envoie dès l'attache, avant la moindre image). Avant ce correctif,
    /// elle tombait dans le bras `Ok(autre)` et abandonnait le fil — chaque
    /// session serait morte à la première trame reçue, sans qu'aucun test des
    /// tâches 4 ou 5 ne puisse le voir puisqu'aucune des deux ne pousse de
    /// trame jusqu'à ce fil-ci.
    #[test]
    fn lire_le_media_survit_a_une_part_et_la_transmet() {
        let mut tampon = Vec::new();
        ecrire_json(&mut tampon, &DepuisCapteur::Part { bps: 4_000_000 }).unwrap();
        // Une image APRÈS la part : si le fil s'était abandonné sur la part,
        // cette image ne serait jamais relayée non plus.
        ecrire_image(&mut tampon, &AccessUnit { data: vec![9, 9, 9], is_keyframe: true, pts_90k: 7 })
            .unwrap();

        let (tx, rx) = sync_channel(8);
        lire_le_media(std::io::Cursor::new(tampon), tx);

        assert_eq!(rx.recv().unwrap(), Recu::Part { bps: 4_000_000 });
        match rx.recv().unwrap() {
            Recu::Image(unite) => assert_eq!(unite.pts_90k, 7),
            autre => panic!("attendu une image après la part, reçu {autre:?}"),
        }
        assert!(
            rx.try_recv().is_err(),
            "aucune trame de plus après la fin du tampon : le fil n'a rien perdu ni rien inventé"
        );
    }

    /// Le même défaut, sur le même hop, pour la même raison — cette fois pour
    /// `DepuisCapteur::Audio` (tâche 6, sous-bloc D7) : câblé côté capteur
    /// (protocole + fil de fenêtre) mais, sans ce bras, il tomberait dans
    /// `Ok(autre)` et tuerait ce fil au tout premier ordre audio reçu, en
    /// conditions de produit et sur toute session.
    #[test]
    fn lire_le_media_survit_a_un_audio_et_le_transmet() {
        let mut tampon = Vec::new();
        ecrire_json(&mut tampon, &DepuisCapteur::Audio { actif: true }).unwrap();
        // Une image APRÈS l'ordre : si le fil s'était abandonné dessus, cette
        // image ne serait jamais relayée non plus.
        ecrire_image(
            &mut tampon,
            &AccessUnit { data: vec![4, 4, 4], is_keyframe: true, pts_90k: 11 },
        )
        .unwrap();

        let (tx, rx) = sync_channel(8);
        lire_le_media(std::io::Cursor::new(tampon), tx);

        assert_eq!(rx.recv().unwrap(), Recu::Audio { actif: true });
        match rx.recv().unwrap() {
            Recu::Image(unite) => assert_eq!(unite.pts_90k, 11),
            autre => panic!("attendu une image après l'ordre audio, reçu {autre:?}"),
        }
        assert!(
            rx.try_recv().is_err(),
            "aucune trame de plus après la fin du tampon : le fil n'a rien perdu ni rien inventé"
        );
    }

    /// Une trame de type inconnu sur la connexion média doit tuer le fil —
    /// pas la faire dériver silencieusement. Vérifie que la sévérité de
    /// `Ok(autre)` n'a pas été affaiblie par l'ajout du bras `Sommeil`.
    #[test]
    fn une_trame_de_commande_egaree_sur_le_media_abandonne_le_fil() {
        let mut tampon = Vec::new();
        // `Attachee` n'est JAMAIS censée transiter sur la connexion média :
        // c'est une réponse de commande. La recevoir ici doit abandonner.
        ecrire_json(&mut tampon, &DepuisCapteur::Attachee { largeur: 1280, hauteur: 720 }).unwrap();
        ecrire_json(
            &mut tampon,
            &DepuisCapteur::Etat { vivante: true, epuisee: false, largeur: 1280, hauteur: 720 },
        )
        .unwrap();

        let (tx, rx) = sync_channel(8);
        lire_le_media(std::io::Cursor::new(tampon), tx);

        assert!(
            rx.try_recv().is_err(),
            "le fil doit abandonner à la première trame inattendue, sans lire la suite"
        );
    }
}
