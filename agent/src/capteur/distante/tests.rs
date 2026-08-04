//! Tests de `SourceDistante` — tous sur l'hôte, sans aucun `#[cfg(windows)]`.
//!
//! Fichier voisin plutôt que module en ligne : `distante.rs` était à 487
//! lignes pour un plafond de projet à 500, et la garde d'épuisement de la
//! revue finale de branche (I2) plus son test l'auraient fait franchir.
//! Extraire plutôt que comprimer — même schéma que
//! `superviseur/table/tests_retention.rs`.

use super::*;
use crate::capteur::reprise::{DUREE_FENETRE_CANAL, PAS_RATTACHEMENT};
use std::sync::mpsc::sync_channel;

/// Ce que le canal factice rendra au prochain `rattacher`. `None` = échec.
/// Une file, pour que les tests enchaînent échecs puis succès.
type ProchainsRattachements = std::sync::Arc<std::sync::Mutex<Vec<Option<u32>>>>;

/// Canal factice : rend des réponses préparées et retient ce qui a été
/// demandé, pour que les tests vérifient le message ÉMIS et pas seulement
/// l'effet.
struct CanalFactice {
    reponses: Vec<anyhow::Result<DepuisCapteur>>,
    recus: std::sync::Arc<std::sync::Mutex<Vec<VersCapteur>>>,
    rattachements: ProchainsRattachements,
    essais: std::sync::Arc<std::sync::Mutex<u32>>,
}

impl Canal for CanalFactice {
    fn commander(&mut self, message: VersCapteur) -> anyhow::Result<DepuisCapteur> {
        self.recus.lock().unwrap().push(message);
        if self.reponses.is_empty() {
            Ok(DepuisCapteur::Fait)
        } else {
            self.reponses.remove(0)
        }
    }

    fn rattacher(&mut self) -> anyhow::Result<Rattachee> {
        *self.essais.lock().unwrap() += 1;
        let prochain = {
            let mut file = self.rattachements.lock().unwrap();
            if file.is_empty() { None } else { file.remove(0) }
        };
        match prochain {
            Some(largeur) => {
                let (tx, rx) = sync_channel(4);
                // Une image dans la file neuve : c'est elle qui prouvera
                // que la source lit bien le NOUVEAU canal.
                tx.send(Recu::Image(AccessUnit {
                    data: vec![7],
                    is_keyframe: true,
                    pts_90k: 700,
                }))
                .unwrap();
                Ok(Rattachee { images: rx, largeur, hauteur: 480 })
            }
            None => anyhow::bail!("aucun capteur"),
        }
    }
}

/// `_capacite` est conservée pour ne pas changer la signature appelée par
/// les tests existants, mais `source_rattachable` fixe la sienne à 4 —
/// ce qui couvre tous les usages actuels de `source_avec`.
fn source_avec(
    _capacite: usize,
) -> (
    SourceDistante,
    std::sync::mpsc::SyncSender<Recu>,
    std::sync::Arc<std::sync::Mutex<Vec<VersCapteur>>>,
) {
    let (source, tx, recus, _, _) = source_rattachable(Vec::new());
    (source, tx, recus)
}

#[allow(clippy::type_complexity)]
fn source_rattachable(
    rattachements: Vec<Option<u32>>,
) -> (
    SourceDistante,
    std::sync::mpsc::SyncSender<Recu>,
    std::sync::Arc<std::sync::Mutex<Vec<VersCapteur>>>,
    ProchainsRattachements,
    std::sync::Arc<std::sync::Mutex<u32>>,
) {
    let (tx, rx) = sync_channel(4);
    let recus = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let file = std::sync::Arc::new(std::sync::Mutex::new(rattachements));
    let essais = std::sync::Arc::new(std::sync::Mutex::new(0));
    let canal = CanalFactice {
        reponses: Vec::new(),
        recus: recus.clone(),
        rattachements: file.clone(),
        essais: essais.clone(),
    };
    (
        SourceDistante::nouvelle(Box::new(canal), rx, 1280, 720),
        tx,
        recus,
        file,
        essais,
    )
}

#[test]
fn une_image_poussee_est_rendue_par_next_frame() {
    let (mut source, tx, _) = source_avec(4);
    tx.send(Recu::Image(AccessUnit { data: vec![1, 2], is_keyframe: true, pts_90k: 42 }))
        .unwrap();
    let unite = source.next_frame().expect("une image était en file");
    assert_eq!(unite.pts_90k, 42);
    assert!(unite.is_keyframe);
}

/// Le cas COURANT : rien de neuf. Il doit être gratuit et ne surtout pas
/// passer pour un épuisement — la boucle de transport interroge à 100 Hz.
#[test]
fn une_file_vide_rend_none_sans_epuiser_la_source() {
    let (mut source, _tx, _) = source_avec(4);
    assert!(source.next_frame().is_none());
    assert!(!source.is_exhausted());
    assert!(source.is_alive());
}

#[test]
fn les_images_sortent_dans_l_ordre_d_arrivee() {
    let (mut source, tx, _) = source_avec(4);
    for pts in [1, 2, 3] {
        tx.send(Recu::Image(AccessUnit { data: vec![], is_keyframe: false, pts_90k: pts }))
            .unwrap();
    }
    let rendus: Vec<u64> =
        (0..3).map(|_| source.next_frame().unwrap().pts_90k).collect();
    assert_eq!(rendus, vec![1, 2, 3]);
}

/// `Etat` n'est pas une image : il met à jour le cache et la lecture
/// continue, sans consommer le tour.
#[test]
fn un_etat_intercale_met_a_jour_le_cache_sans_masquer_l_image_suivante() {
    let (mut source, tx, _) = source_avec(4);
    tx.send(Recu::Etat { vivante: true, epuisee: false, largeur: 800, hauteur: 600 })
        .unwrap();
    tx.send(Recu::Image(AccessUnit { data: vec![], is_keyframe: false, pts_90k: 5 }))
        .unwrap();
    assert_eq!(source.next_frame().unwrap().pts_90k, 5);
    assert_eq!(source.dimensions(), (800, 600));
}

#[test]
fn une_fenetre_disparue_rend_la_source_non_vivante_et_epuisee() {
    let (mut source, tx, _) = source_avec(4);
    tx.send(Recu::Etat { vivante: false, epuisee: true, largeur: 1280, hauteur: 720 })
        .unwrap();
    assert!(source.next_frame().is_none());
    assert!(!source.is_alive());
    assert!(source.is_exhausted());
}

/// **I2 de la revue finale de branche du sous-bloc D4.** Le test ci-dessus
/// garde `tx` vivant, donc n'atteint JAMAIS `Disconnected` : c'est celui-ci
/// qui éprouve la suite réelle des événements à la fermeture normale d'une
/// fenêtre — le capteur pousse `Etat { epuisee: true }`, PUIS ferme le tube.
///
/// L'enfant doit alors conclure, et surtout pas rattacher : un rattachement
/// ferait rouvrir au capteur une duplication DXGI et un encodeur sur une
/// sortie que le superviseur détruit au même instant, et s'il aboutissait il
/// remettrait `epuisee` à faux — la session qui devait se clore ne se
/// clorait pas.
///
/// La file de rattachements porte un succès À DESSEIN : sans la garde, le
/// rattachement ne se contenterait pas d'être tenté, il RÉUSSIRAIT.
#[test]
fn un_epuisement_autoritaire_interdit_tout_rattachement() {
    let (mut source, tx, _, _, essais) = source_rattachable(vec![Some(1600)]);
    tx.send(Recu::Etat { vivante: false, epuisee: true, largeur: 1280, hauteur: 720 })
        .unwrap();
    assert!(source.next_frame().is_none());
    assert!(source.is_exhausted(), "l'état autoritaire épuise la source");

    // Le capteur ferme le tube : le tour suivant voit `Disconnected`.
    drop(tx);
    assert!(source.next_frame().is_none());
    assert_eq!(
        *essais.lock().unwrap(),
        0,
        "aucun rattachement ne doit être tenté après un épuisement autoritaire"
    );
    assert!(source.is_exhausted(), "l'épuisement reste acquis");
    assert!(!source.is_alive());
}

#[test]
fn les_commandes_partent_sous_la_forme_attendue() {
    let (mut source, _tx, recus) = source_avec(4);
    source.set_bitrate(3_000_000).unwrap();
    source.set_encode_size(640, 360).unwrap();
    source.request_keyframe().unwrap();
    let recus = recus.lock().unwrap();
    assert_eq!(
        *recus,
        vec![
            VersCapteur::Debit { bps: 3_000_000 },
            VersCapteur::TailleEncodage { largeur: 640, hauteur: 360 },
            VersCapteur::ImageCle,
        ]
    );
}

/// `resize` doit retenir la taille RÉELLEMENT obtenue, pas celle demandée
/// — même règle qu'en mono-fenêtre (`transport/redimensionnement.rs`).
/// Le test utilise des dimensions initiales DIFFÉRENTES de la réponse
/// pour vérifier que la réponse est réellement adoptée (et pas ignorée).
#[test]
fn un_redimensionnement_retient_la_taille_obtenue() {
    let (tx_img, rx) = sync_channel(4);
    let recus = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let canal = CanalFactice {
        reponses: vec![Ok(DepuisCapteur::Taille { largeur: 1280, hauteur: 720 })],
        recus: recus.clone(),
        rattachements: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        essais: std::sync::Arc::new(std::sync::Mutex::new(0)),
    };
    let mut source = SourceDistante::nouvelle(Box::new(canal), rx, 640, 480);
    drop(tx_img);
    source.resize(1281, 713).unwrap();
    assert_eq!(source.dimensions(), (1280, 720));
}

#[test]
fn une_erreur_du_capteur_remonte_en_erreur() {
    let (_tx, rx) = sync_channel(4);
    let canal = CanalFactice {
        reponses: vec![Ok(DepuisCapteur::Erreur { motif: "encodeur perdu".into() })],
        recus: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        rattachements: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        essais: std::sync::Arc::new(std::sync::Mutex::new(0)),
    };
    let mut source = SourceDistante::nouvelle(Box::new(canal), rx, 1280, 720);
    let erreur = source.set_bitrate(1).unwrap_err().to_string();
    assert!(erreur.contains("encodeur perdu"), "message inattendu : {erreur}");
}

/// Le cœur du critère 2 : tuer le capteur ferme le tube, donc rompt le
/// canal — et cela ne doit PAS clore la session, sans quoi
/// `brancher_video` appelle `begin_ending("source vidéo épuisée")`.
#[test]
fn un_canal_rompu_n_epuise_pas_la_source_dans_la_fenetre() {
    let (mut source, tx, _) = source_avec(4);
    drop(tx);
    assert!(source.next_frame().is_none());
    assert!(!source.is_exhausted(), "une rupture de canal n'est pas un épuisement");
}

/// Mais une rupture qui dure l'est : sans cela, une session morte
/// resterait ouverte indéfiniment sur une image figée.
#[test]
fn un_canal_rompu_au_dela_de_la_fenetre_epuise_la_source() {
    let (mut source, tx, _) = source_avec(4);
    drop(tx);
    assert!(source.next_frame().is_none());
    source.vieillir_pour_test(crate::capteur::reprise::DUREE_FENETRE_CANAL + std::time::Duration::from_millis(1));
    assert!(source.next_frame().is_none());
    assert!(source.is_exhausted());
}

/// Le cœur du critère 2 : le capteur meurt, il est relancé, et la session
/// reprend — même file neuve, mêmes dimensions annoncées par le capteur.
#[test]
fn un_rattachement_reussi_fait_revivre_la_source_et_reprend_ses_dimensions() {
    let (mut source, tx, _, rattachements, essais) = source_rattachable(vec![Some(1600)]);
    drop(tx);
    // Premier tour : rupture constatée, rattachement tenté et réussi.
    assert!(source.next_frame().is_none(), "le tour de la rupture ne rend pas d'image");
    assert_eq!(*essais.lock().unwrap(), 1);
    assert!(rattachements.lock().unwrap().is_empty());
    // Tour suivant : l'image vient de la file NEUVE.
    let unite = source.next_frame().expect("la file neuve porte une image");
    assert_eq!(unite.pts_90k, 700);
    assert_eq!(source.dimensions(), (1600, 480), "les dimensions du capteur relancé");
    assert!(!source.is_exhausted());
    assert!(source.is_alive());
}

/// Un rattachement qui échoue ne conclut rien : la fenêtre court encore.
#[test]
fn un_rattachement_qui_echoue_laisse_la_source_en_attente_sans_l_epuiser() {
    let (mut source, tx, _, _, essais) = source_rattachable(vec![None]);
    drop(tx);
    assert!(source.next_frame().is_none());
    assert_eq!(*essais.lock().unwrap(), 1);
    assert!(!source.is_exhausted(), "un échec de rattachement n'épuise pas");
}

/// Mais un échec qui dure au-delà de la fenêtre, si : sans cela une
/// session morte resterait ouverte indéfiniment sur une image figée.
///
/// ⚠️ Ce test n'exerce PAS un second essai de rattachement : une fois
/// `DUREE_FENETRE_CANAL` dépassée, `rupture()` court-circuite et rend
/// `true` avant même d'atteindre `peut_reessayer` (branche `Disconnected`
/// de `next_frame`) — que `vieillir_pour_test` fasse vieillir
/// `dernier_essai` ou non est donc sans effet ICI. Cette bande-là
/// (vieillir au-delà du seul pas d'espacement, en restant dans la
/// fenêtre) est celle qu'éprouve `un_vieillissement_du_pas_seul_relance_un_essai`.
#[test]
fn un_rattachement_qui_echoue_jusqu_a_expiration_epuise_la_source() {
    let (mut source, tx, _, _, _) = source_rattachable(vec![None]);
    drop(tx);
    assert!(source.next_frame().is_none());
    source.vieillir_pour_test(DUREE_FENETRE_CANAL + std::time::Duration::from_millis(1));
    assert!(source.next_frame().is_none());
    assert!(source.is_exhausted());
}

/// Vieillir DANS la fenêtre, au-delà du seul pas d'espacement : un second
/// essai doit partir. Ce test-ci est le seul à éprouver que
/// `vieillir_pour_test` fait bien vieillir `dernier_essai` — celui de
/// l'expiration ne l'atteint jamais, `rupture` court-circuitant avant.
#[test]
fn un_vieillissement_du_pas_seul_relance_un_essai() {
    let (mut source, tx, _, _, essais) = source_rattachable(vec![None, None]);
    drop(tx);
    assert!(source.next_frame().is_none());
    assert_eq!(*essais.lock().unwrap(), 1);
    source.vieillir_pour_test(PAS_RATTACHEMENT + std::time::Duration::from_millis(1));
    assert!(source.next_frame().is_none());
    assert_eq!(*essais.lock().unwrap(), 2, "le pas écoulé autorise un second essai");
    assert!(!source.is_exhausted(), "on est encore dans la fenêtre");
}

/// Sans espacement, une rupture provoquerait ~100 tentatives par seconde.
#[test]
fn une_rafale_d_interrogations_ne_produit_qu_un_seul_essai() {
    let (mut source, tx, _, _, essais) = source_rattachable(vec![None, None, None, None]);
    drop(tx);
    for _ in 0..10 {
        assert!(source.next_frame().is_none());
    }
    assert_eq!(*essais.lock().unwrap(), 1, "un seul essai dans la rafale");
}

/// `set_awake` relaie la visibilité telle quelle au capteur : c'est lui qui
/// arbitre globalement (tâche 7). `source_avec` sert ici de canal espion, par
/// son troisième élément (`recus`), pour vérifier le message ÉMIS.
#[test]
fn set_awake_transmet_la_visibilite_au_capteur() {
    let (mut source, _tx, recus) = source_avec(4);
    source.set_awake(false, false).expect("le capteur accepte");
    assert_eq!(
        recus.lock().unwrap().as_slice(),
        &[VersCapteur::Visibilite { visible: false, focalisee: false }]
    );
}

/// Un `Sommeil` poussé par le capteur est retenu, pas ignoré : c'est
/// `sommeil_a_annoncer` qui le rend disponible à la boucle de transport, et
/// une seule fois — la réémettre à chaque tour inonderait le canal de
/// contrôle vers le navigateur. `source_avec` sert ici de file injectable par
/// son deuxième élément (`tx`).
#[test]
fn un_sommeil_pousse_par_le_capteur_est_retenu_pour_le_client() {
    let (mut source, tx, _recus) = source_avec(4);
    tx.send(Recu::Sommeil { endormie: true, raison: "evincee".into() }).expect("dépôt");
    // Le sommeil est consommé par le tour de boucle qui cherche une image.
    assert!(source.next_frame().is_none());
    assert_eq!(source.sommeil_a_annoncer(), Some((true, "evincee".to_string())));
    assert_eq!(source.sommeil_a_annoncer(), None, "une annonce ne se répète pas");
}

/// `sommeil` est un état COURANT, pas un historique : deux `Sommeil` reçus
/// avant toute lecture s'écrasent, et seul le dernier doit survivre — sans
/// quoi la boucle de transport annoncerait au navigateur un état déjà
/// périmé, ou pire, une file d'annonces grandirait sans jamais se vider.
#[test]
fn deux_sommeils_consecutifs_ne_retiennent_que_le_dernier() {
    let (mut source, tx, _recus) = source_avec(4);
    tx.send(Recu::Sommeil { endormie: true, raison: "masquee".into() }).expect("dépôt");
    tx.send(Recu::Sommeil { endormie: true, raison: "evincee".into() }).expect("dépôt");
    assert!(source.next_frame().is_none());
    assert_eq!(
        source.sommeil_a_annoncer(),
        Some((true, "evincee".to_string())),
        "seul le dernier sommeil reçu doit survivre"
    );
    assert_eq!(source.sommeil_a_annoncer(), None);
}

/// Une part reçue est retenue jusqu'à ce que la boucle de transport la
/// consomme, et ne se rend qu'une fois — même patron que
/// `un_sommeil_pousse_par_le_capteur_est_retenu_pour_le_client` plus haut.
#[test]
fn une_part_recue_est_rendue_une_seule_fois() {
    let (mut source, tx, _recus) = source_avec(4);
    tx.send(Recu::Part { bps: 4_000_000 }).expect("dépôt");
    // `next_frame` est ce qui draine le canal : sans lui, rien n'est lu.
    assert_eq!(source.next_frame(), None);
    assert_eq!(source.part_a_appliquer(), Some(4_000_000));
    assert_eq!(source.part_a_appliquer(), None, "une part ne se réapplique pas");
}

/// Deux parts arrivées entre deux lectures s'écrasent : c'est un état
/// courant, pas un historique — même régime que `Etat` et `Sommeil`.
#[test]
fn deux_parts_arrivees_avant_lecture_s_ecrasent() {
    let (mut source, tx, _recus) = source_avec(4);
    tx.send(Recu::Part { bps: 4_000_000 }).expect("dépôt");
    tx.send(Recu::Part { bps: 2_000_000 }).expect("dépôt");
    assert_eq!(source.next_frame(), None);
    assert_eq!(source.part_a_appliquer(), Some(2_000_000), "seule la dernière survit");
}

/// Un ordre audio reçu est retenu jusqu'à ce que la boucle de transport le
/// consomme, et ne se rend qu'une fois — même patron que
/// `une_part_recue_est_rendue_une_seule_fois` plus haut.
#[test]
fn un_ordre_audio_recu_est_rendu_une_seule_fois() {
    let (mut source, tx, _recus) = source_avec(4);
    tx.send(Recu::Audio { actif: true }).expect("dépôt");
    // `next_frame` est ce qui draine le canal : sans lui, rien n'est lu.
    assert_eq!(source.next_frame(), None);
    assert_eq!(source.audio_a_appliquer(), Some(true));
    assert_eq!(source.audio_a_appliquer(), None, "un ordre audio ne se réapplique pas");
}

/// **Au rattachement, l'enfant REDEVIENT MUET** (conception §4.4 ; F2, revue
/// finale de branche).
///
/// Ce que ce test attrape, et que rien n'attrapait : une porteuse dont le
/// canal casse gardait son drapeau `emet` d'avant la rupture, parce que le
/// rattachement ne remettait à zéro que l'ordre EN ATTENTE (`None`, « rien à
/// changer ») et jamais l'état de la source. Deux fenêtres d'un même PID
/// jouaient alors le même mix, désynchronisées, jusqu'à ce que l'ordre
/// d'extinction arrive — un écho audible.
#[test]
fn un_rattachement_remet_l_enfant_au_silence() {
    let (mut source, tx, _recus, _rattachements, _essais) = source_rattachable(vec![Some(1600)]);
    // La fenêtre porte le son, et la boucle de transport a consommé l'ordre :
    // il ne reste plus rien en attente, seul l'état réel de la source le sait.
    tx.send(Recu::Audio { actif: true }).expect("dépôt");
    assert_eq!(source.next_frame(), None);
    assert_eq!(source.audio_a_appliquer(), Some(true));

    // Le capteur meurt, l'enfant se rattache.
    drop(tx);
    assert_eq!(source.next_frame(), None, "le tour de la rupture ne rend pas d'image");

    assert_eq!(
        source.audio_a_appliquer(),
        Some(false),
        "un rattachement doit ORDONNER le silence, pas se taire sur la question"
    );
}

/// Deux ordres audio arrivés entre deux lectures s'écrasent : même régime
/// que `deux_parts_arrivees_avant_lecture_s_ecrasent` juste au-dessus.
#[test]
fn deux_ordres_audio_arrives_avant_lecture_s_ecrasent() {
    let (mut source, tx, _recus) = source_avec(4);
    tx.send(Recu::Audio { actif: true }).expect("dépôt");
    tx.send(Recu::Audio { actif: false }).expect("dépôt");
    assert_eq!(source.next_frame(), None);
    assert_eq!(source.audio_a_appliquer(), Some(false), "seul le dernier ordre survit");
}
