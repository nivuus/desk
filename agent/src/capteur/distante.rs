//! `SourceDistante` — la source vidéo d'un enfant, alimentée par le capteur.
//!
//! **Pas de `#[cfg(windows)]`** : le tube réel est gaté (`capteur/tube.rs`),
//! mais la décision de clore ou non une session vit ici, et c'est la pièce la
//! plus coûteuse à se tromper. Elle est donc écrite contre un `Canal`
//! injecté et un `Receiver`, tous deux triviaux à simuler sur l'hôte.

use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Instant;

use anyhow::{bail, Result};

use crate::capteur::protocole::{DepuisCapteur, VersCapteur};
use crate::capteur::reprise::FenetreCanal;
use crate::h264::AccessUnit;
use crate::source::VideoSource;

/// Ce que l'enfant peut demander au capteur, et le moyen de s'y rattacher
/// quand le canal se rompt.
pub trait Canal {
    fn commander(&mut self, message: VersCapteur) -> Result<DepuisCapteur>;
    /// Rouvre un canal vers le capteur et s'y réattache. L'implémentation
    /// remplace son propre état interne d'écriture ; elle rend la file
    /// d'images neuve et les dimensions annoncées à l'attache.
    fn rattacher(&mut self) -> Result<Rattachee>;
}

/// Le fruit d'un rattachement réussi.
pub struct Rattachee {
    pub images: Receiver<Recu>,
    pub largeur: u32,
    pub hauteur: u32,
}

/// Ce que le capteur pousse, non sollicité.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recu {
    Image(AccessUnit),
    Etat { vivante: bool, epuisee: bool, largeur: u32, hauteur: u32 },
}

pub struct SourceDistante {
    canal: Box<dyn Canal + Send>,
    images: Receiver<Recu>,
    largeur: u32,
    hauteur: u32,
    vivante: bool,
    epuisee: bool,
    /// Rupture du canal en cours. Une rupture n'épuise pas la source tant que
    /// cette fenêtre n'a pas expiré : c'est ce qui fait survivre les sessions
    /// à une relance du capteur.
    fenetre: FenetreCanal,
}

impl SourceDistante {
    pub fn nouvelle(
        canal: Box<dyn Canal + Send>,
        images: Receiver<Recu>,
        largeur: u32,
        hauteur: u32,
    ) -> Self {
        Self {
            canal,
            images,
            largeur,
            hauteur,
            vivante: true,
            epuisee: false,
            fenetre: FenetreCanal::nouvelle(),
        }
    }

    /// Émet une commande et n'accepte que `Fait` comme succès.
    fn commander_simple(&mut self, message: VersCapteur) -> Result<()> {
        match self.canal.commander(message)? {
            DepuisCapteur::Fait => Ok(()),
            DepuisCapteur::Erreur { motif } => bail!("le capteur a refusé : {motif}"),
            autre => bail!("réponse inattendue du capteur : {autre:?}"),
        }
    }

    /// Fait vieillir la fenêtre de reprise, pour les seuls tests : sans elle,
    /// éprouver l'expiration exigerait d'attendre réellement 15 secondes.
    #[cfg(test)]
    pub fn vieillir_pour_test(&mut self, ecart: std::time::Duration) {
        self.fenetre.vieillir_pour_test(ecart);
    }
}

impl VideoSource for SourceDistante {
    fn next_frame(&mut self) -> Option<AccessUnit> {
        loop {
            match self.images.try_recv() {
                Ok(Recu::Image(unite)) => {
                    self.fenetre.succes();
                    return Some(unite);
                }
                Ok(Recu::Etat { vivante, epuisee, largeur, hauteur }) => {
                    self.vivante = vivante;
                    self.epuisee = epuisee;
                    self.largeur = largeur;
                    self.hauteur = hauteur;
                }
                // Le cas COURANT et normal : rien de neuf ce tour-ci. La
                // boucle de transport interroge à 100 Hz une source qui
                // produit à ~90 i/s.
                Err(TryRecvError::Empty) => {
                    self.fenetre.succes();
                    return None;
                }
                // Le canal est rompu. La fenêtre de reprise borne combien de
                // temps la source reste vivante en attendant un rattachement.
                Err(TryRecvError::Disconnected) => {
                    let maintenant = Instant::now();
                    if self.fenetre.rupture(maintenant) {
                        // La fenêtre est expirée : l'épuisement est acquis.
                        self.epuisee = true;
                        return None;
                    }
                    if self.fenetre.peut_reessayer(maintenant) {
                        match self.canal.rattacher() {
                            Ok(Rattachee { images, largeur, hauteur }) => {
                                tracing::info!(largeur, hauteur, "canal rattaché au capteur");
                                self.images = images;
                                self.largeur = largeur;
                                self.hauteur = hauteur;
                                self.vivante = true;
                                self.epuisee = false;
                                self.fenetre.succes();
                            }
                            // Journalisé en `debug!` et non `info!` : au pas
                            // de 250 ms sur une fenêtre de 15 s, un capteur
                            // durablement absent produirait 60 lignes par
                            // fenêtre et par session.
                            Err(erreur) => tracing::debug!(%erreur, "rattachement refusé"),
                        }
                    }
                    return None;
                }
            }
        }
    }

    fn dimensions(&self) -> (u32, u32) {
        (self.largeur, self.hauteur)
    }

    fn is_exhausted(&self) -> bool {
        self.epuisee
    }

    fn is_alive(&self) -> bool {
        self.vivante
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        match self
            .canal
            .commander(VersCapteur::Redimensionner { largeur: width, hauteur: height })?
        {
            // La taille RETENUE est celle obtenue, jamais celle demandée : le
            // pilote quantifie, et une fenêtre Windows impose des dimensions
            // paires. Même règle qu'en mono-fenêtre.
            DepuisCapteur::Taille { largeur, hauteur } => {
                self.largeur = largeur;
                self.hauteur = hauteur;
                Ok(())
            }
            DepuisCapteur::Erreur { motif } => bail!("le capteur a refusé : {motif}"),
            autre => bail!("réponse inattendue du capteur : {autre:?}"),
        }
    }

    fn set_bitrate(&mut self, bitrate: u32) -> Result<()> {
        self.commander_simple(VersCapteur::Debit { bps: bitrate })
    }

    fn set_encode_size(&mut self, width: u32, height: u32) -> Result<()> {
        self.commander_simple(VersCapteur::TailleEncodage { largeur: width, hauteur: height })
    }

    fn request_keyframe(&mut self) -> Result<()> {
        self.commander_simple(VersCapteur::ImageCle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capteur::reprise::DUREE_FENETRE_CANAL;
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
    #[test]
    fn un_rattachement_qui_echoue_jusqu_a_expiration_epuise_la_source() {
        let (mut source, tx, _, _, _) = source_rattachable(vec![None]);
        drop(tx);
        assert!(source.next_frame().is_none());
        source.vieillir_pour_test(DUREE_FENETRE_CANAL + std::time::Duration::from_millis(1));
        assert!(source.next_frame().is_none());
        assert!(source.is_exhausted());
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
}
