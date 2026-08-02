//! `SourceDistante` — la source vidéo d'un enfant, alimentée par le capteur.
//!
//! **Pas de `#[cfg(windows)]`** : le tube réel est gaté (`capteur/tube.rs`),
//! mais la décision de clore ou non une session vit ici, et c'est la pièce la
//! plus coûteuse à se tromper. Elle est donc écrite contre un `Commandes`
//! injecté et un `Receiver`, tous deux triviaux à simuler sur l'hôte.

use std::sync::mpsc::{Receiver, TryRecvError};

use anyhow::{bail, Result};

use crate::capteur::protocole::{DepuisCapteur, VersCapteur};
use crate::h264::AccessUnit;
use crate::source::VideoSource;

/// Ce que l'enfant peut demander au capteur, en requête/réponse.
pub trait Commandes {
    fn commander(&mut self, message: VersCapteur) -> Result<DepuisCapteur>;
}

/// Ce que le capteur pousse, non sollicité.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recu {
    Image(AccessUnit),
    Etat { vivante: bool, epuisee: bool, largeur: u32, hauteur: u32 },
}

pub struct SourceDistante {
    commandes: Box<dyn Commandes + Send>,
    images: Receiver<Recu>,
    largeur: u32,
    hauteur: u32,
    vivante: bool,
    epuisee: bool,
}

impl SourceDistante {
    pub fn nouvelle(
        commandes: Box<dyn Commandes + Send>,
        images: Receiver<Recu>,
        largeur: u32,
        hauteur: u32,
    ) -> Self {
        Self { commandes, images, largeur, hauteur, vivante: true, epuisee: false }
    }

    /// Émet une commande et n'accepte que `Fait` comme succès.
    fn commander_simple(&mut self, message: VersCapteur) -> Result<()> {
        match self.commandes.commander(message)? {
            DepuisCapteur::Fait => Ok(()),
            DepuisCapteur::Erreur { motif } => bail!("le capteur a refusé : {motif}"),
            autre => bail!("réponse inattendue du capteur : {autre:?}"),
        }
    }
}

impl VideoSource for SourceDistante {
    fn next_frame(&mut self) -> Option<AccessUnit> {
        loop {
            match self.images.try_recv() {
                Ok(Recu::Image(unite)) => return Some(unite),
                Ok(Recu::Etat { vivante, epuisee, largeur, hauteur }) => {
                    self.vivante = vivante;
                    self.epuisee = epuisee;
                    self.largeur = largeur;
                    self.hauteur = hauteur;
                }
                // Le cas COURANT et normal : rien de neuf ce tour-ci. La
                // boucle de transport interroge à 100 Hz une source qui
                // produit à ~90 i/s.
                Err(TryRecvError::Empty) => return None,
                // Le canal est rompu. Ce n'est PAS traité ici comme un
                // épuisement : la tâche 3 y branche la fenêtre de reprise.
                Err(TryRecvError::Disconnected) => return None,
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
            .commandes
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
    use std::sync::mpsc::sync_channel;

    /// Canal factice : rend des réponses préparées et retient ce qui a été
    /// demandé, pour que les tests vérifient le message ÉMIS et pas seulement
    /// l'effet.
    struct CanalFactice {
        reponses: Vec<anyhow::Result<DepuisCapteur>>,
        recus: std::sync::Arc<std::sync::Mutex<Vec<VersCapteur>>>,
    }

    impl Commandes for CanalFactice {
        fn commander(&mut self, message: VersCapteur) -> anyhow::Result<DepuisCapteur> {
            self.recus.lock().unwrap().push(message);
            if self.reponses.is_empty() {
                Ok(DepuisCapteur::Fait)
            } else {
                self.reponses.remove(0)
            }
        }
    }

    fn source_avec(
        capacite: usize,
    ) -> (
        SourceDistante,
        std::sync::mpsc::SyncSender<Recu>,
        std::sync::Arc<std::sync::Mutex<Vec<VersCapteur>>>,
    ) {
        let (tx, rx) = sync_channel(capacite);
        let recus = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let canal = CanalFactice { reponses: Vec::new(), recus: recus.clone() };
        (SourceDistante::nouvelle(Box::new(canal), rx, 1280, 720), tx, recus)
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
        };
        let mut source = SourceDistante::nouvelle(Box::new(canal), rx, 1280, 720);
        let erreur = source.set_bitrate(1).unwrap_err().to_string();
        assert!(erreur.contains("encodeur perdu"), "message inattendu : {erreur}");
    }
}
