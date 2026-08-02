//! Le fil d'une fenêtre : il tient un `WindowsSource` complet et le sert à
//! l'enfant par le tube.
//!
//! **Il ne réécrit AUCUN code de capture ni d'encodage.** `WindowsSource` est
//! déjà exactement le couple `DesktopCapture` + `H264Encoder` derrière le
//! trait `VideoSource` : ce module ne fait qu'appeler ce trait et transporter
//! ses résultats. C'est la simplification centrale du sous-bloc D4.

#![cfg(windows)]

use std::io::Write;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use windows::Win32::Foundation::HWND;

use crate::capteur::horloge::{frequence_qpc, lire_qpc, origine_depuis_qpc};
use crate::capteur::protocole::{ecrire_image, ecrire_json, DepuisCapteur, VersCapteur};
use crate::source::VideoSource;
use crate::windows_source::WindowsSource;

/// Pas de sommeil quand la source n'a rien rendu.
///
/// 10 ms, la valeur exacte de `FRAME_INTERVAL` côté transport : cette boucle
/// prend la place de l'interrogation que faisait l'enfant, et il n'y a aucune
/// raison de changer la cadence de sondage en même temps que le reste. Le
/// commentaire de `transport/piste_video.rs` explique pourquoi 10 ms et non
/// 16 : interroger plus souvent que la source ne produit lève une borne sans
/// rien coûter quand il n'y a rien à prendre.
const PAS_A_VIDE: Duration = Duration::from_millis(10);

/// Période des lignes de compteurs. **Jamais de trace par image** : le projet
/// a déjà perdu une session entière à une trace par paquet.
const PERIODE_COMPTEURS: Duration = Duration::from_secs(10);

/// Une fenêtre servie par le capteur : sa source, et de quoi la nommer.
///
/// **Le `WindowsSource` ne quitte jamais le fil qui l'a construit.** Il porte
/// des objets COM et n'est pas `Sync` : `ouvrir` et `servir` sont appelées sur
/// le seul fil de fenêtre, et les commandes lui parviennent par `mpsc` depuis
/// le fil qui tient la connexion de commandes.
pub struct Fenetre {
    source: WindowsSource,
    session: String,
    sortie: String,
    largeur: u32,
    hauteur: u32,
}

impl Fenetre {
    /// Construit la source annoncée par une trame d'attache.
    ///
    /// La réponse à l'attache n'est PAS écrite ici : elle part sur la
    /// connexion de commandes, que ce fil ne touche jamais. L'appelant écrit
    /// `Attachee { largeur, hauteur }` en cas de succès, `Refus` sinon.
    pub fn ouvrir(attache: VersCapteur) -> Result<Fenetre> {
        let VersCapteur::Attache { session, hwnd, sortie, fps, debit, origine_qpc } = attache
        else {
            bail!("le premier message d'un enfant doit être une attache");
        };

        let clock_origin = origine_depuis_qpc(
            origine_qpc,
            lire_qpc().context("lecture de QPC à l'attache")?,
            frequence_qpc().context("fréquence de QPC")?,
            Instant::now(),
        );

        let hwnd = HWND(hwnd as *mut core::ffi::c_void);
        let source = WindowsSource::sur_sortie(hwnd, &sortie, fps, debit, clock_origin)
            .with_context(|| format!("attache de la session {session}"))?;
        let (largeur, hauteur) = source.dimensions();
        Ok(Fenetre { source, session, sortie, largeur, hauteur })
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (self.largeur, self.hauteur)
    }

    /// Sert la fenêtre jusqu'à la fin de sa vie.
    ///
    /// `ecrivain` est la connexion **média**, et c'est le **seul** objet
    /// fichier que ce fil écrit : images et état. Les réponses aux commandes
    /// partent par `reponses`, vers le fil qui tient la connexion de
    /// commandes et qui les écrit lui-même. Aucun objet fichier ne porte donc
    /// jamais une lecture et une écriture concurrentes.
    pub fn servir<E: Write>(
        mut self,
        commandes: Receiver<VersCapteur>,
        reponses: Sender<DepuisCapteur>,
        mut ecrivain: E,
    ) -> Result<()> {
        // Copiée une fois : les traces la citent à chaque tour, et `source`
        // emprunte l'autre champ pendant tout ce temps.
        let session = self.session.clone();
        tracing::info!(
            %session,
            sortie = %self.sortie,
            largeur = self.largeur,
            hauteur = self.hauteur,
            "fenêtre attachée au capteur"
        );
        let source = &mut self.source;

        let mut dernier_etat = (true, false, self.largeur, self.hauteur);
        let mut images = 0u64;
        let mut dernier_compte = Instant::now();

        loop {
            // 1. Les commandes en attente, s'il y en a. Elles sont rares.
            loop {
                match commandes.try_recv() {
                    Ok(message) => {
                        let reponse = executer_commande(source, message);
                        // La réponse repart par le canal, jamais par une
                        // écriture directe : ce fil n'écrit que le média.
                        if reponses.send(reponse).is_err() {
                            tracing::info!(%session, images, "le fil de commandes est parti");
                            return Ok(());
                        }
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    // L'enfant a fermé le tube. On sort par le haut, ce qui
                    // relâche `source` — donc la duplication et l'encodeur — SUR
                    // CE FIL-CI, jamais sur la boucle d'acceptation. `Drop for
                    // H264Encoder` peut geler (risque observé, non attribué) ;
                    // ici il ne gèlerait que cette fenêtre.
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        tracing::info!(%session, images, "l'enfant a fermé le canal");
                        return Ok(());
                    }
                }
            }

            // 2. Une image, s'il y en a une.
            match source.next_frame() {
                Some(unite) => {
                    images += 1;
                    // Une écriture bloquante EST la contre-pression : si l'enfant
                    // ne lit plus, ce fil attend — et il n'attend que pour SA
                    // fenêtre. Une unité d'accès ne peut pas être jetée sans
                    // corrompre le flux (les images P référencent les
                    // précédentes).
                    ecrire_image(&mut ecrivain, &unite)?;
                    ecrivain.flush()?;
                }
                None => std::thread::sleep(PAS_A_VIDE),
            }

            // 3. L'état, au CHANGEMENT seulement.
            let (largeur, hauteur) = source.dimensions();
            let etat = (source.is_alive(), source.is_exhausted(), largeur, hauteur);
            if etat != dernier_etat {
                dernier_etat = etat;
                ecrire_json(
                    &mut ecrivain,
                    &DepuisCapteur::Etat {
                        vivante: etat.0,
                        epuisee: etat.1,
                        largeur: etat.2,
                        hauteur: etat.3,
                    },
                )?;
                ecrivain.flush()?;
                if !etat.0 || etat.1 {
                    tracing::info!(%session, vivante = etat.0, epuisee = etat.1, images,
                        "fin de la fenêtre côté capteur");
                    return Ok(());
                }
            }

            if dernier_compte.elapsed() >= PERIODE_COMPTEURS {
                let ecoule = dernier_compte.elapsed().as_secs_f64();
                tracing::info!(
                    %session,
                    images,
                    cadence = format!("{:.1}", images as f64 / ecoule),
                    "cadence du capteur"
                );
                images = 0;
                dernier_compte = Instant::now();
            }
        }
    }
}

fn executer_commande(source: &mut WindowsSource, message: VersCapteur) -> DepuisCapteur {
    let resultat = match message {
        VersCapteur::Redimensionner { largeur, hauteur } => {
            return match source.resize(largeur, hauteur) {
                Ok(()) => {
                    let (largeur, hauteur) = source.dimensions();
                    DepuisCapteur::Taille { largeur, hauteur }
                }
                Err(erreur) => DepuisCapteur::Erreur { motif: format!("{erreur:#}") },
            }
        }
        VersCapteur::TailleEncodage { largeur, hauteur } => source.set_encode_size(largeur, hauteur),
        VersCapteur::Debit { bps } => source.set_bitrate(bps),
        VersCapteur::ImageCle => source.request_keyframe(),
        VersCapteur::Attache { .. } => {
            return DepuisCapteur::Erreur { motif: "seconde attache sur un canal déjà attaché".into() }
        }
        // `Identite` n'appartient qu'à la connexion média, où elle est la
        // première et unique trame : la voir ici signale un enfant qui
        // confond ses deux connexions.
        VersCapteur::Identite { session } => {
            return DepuisCapteur::Erreur {
                motif: format!("identité de {session} sur la connexion de commandes"),
            }
        }
    };
    match resultat {
        Ok(()) => DepuisCapteur::Fait,
        Err(erreur) => DepuisCapteur::Erreur { motif: format!("{erreur:#}") },
    }
}
