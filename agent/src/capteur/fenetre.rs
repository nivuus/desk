//! Le fil d'une fenêtre : il tient un `WindowsSource` complet et le sert à
//! l'enfant par le tube.
//!
//! **Il ne réécrit AUCUN code de capture ni d'encodage.** `WindowsSource` est
//! déjà exactement le couple `DesktopCapture` + `H264Encoder` derrière le
//! trait `VideoSource` : ce module ne fait qu'appeler ce trait et transporter
//! ses résultats. C'est la simplification centrale du sous-bloc D4.

#![cfg(windows)]

use std::io::Write;
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

pub fn servir_une_fenetre<E: Write>(
    commandes: std::sync::mpsc::Receiver<VersCapteur>,
    mut ecrivain: E,
    attache: VersCapteur,
) -> Result<()> {
    let VersCapteur::Attache { session, hwnd, sortie, fps, debit, origine_qpc } = attache else {
        bail!("le premier message d'un enfant doit être une attache");
    };

    let clock_origin = origine_depuis_qpc(
        origine_qpc,
        lire_qpc().context("lecture de QPC à l'attache")?,
        frequence_qpc().context("fréquence de QPC")?,
        Instant::now(),
    );

    let hwnd = HWND(hwnd as *mut core::ffi::c_void);
    let mut source = match WindowsSource::sur_sortie(hwnd, &sortie, fps, debit, clock_origin) {
        Ok(source) => source,
        Err(erreur) => {
            // Le refus est ANNONCÉ à l'enfant, jamais silencieux : sans ce
            // message il attendrait une image qui ne viendra pas.
            let _ = ecrire_json(
                &mut ecrivain,
                &DepuisCapteur::Refus { motif: format!("{erreur:#}") },
            );
            return Err(erreur).with_context(|| format!("attache de la session {session}"));
        }
    };

    let (largeur, hauteur) = source.dimensions();
    ecrire_json(&mut ecrivain, &DepuisCapteur::Attachee { largeur, hauteur })?;
    // **Ce `flush` n'est pas décoratif : sans lui l'attache ne se termine
    // jamais sur une fenêtre immobile.** `ecrivain` est un `BufWriter` ; la
    // réponse resterait dans son tampon jusqu'à la première écriture qui le
    // vide — c'est-à-dire jusqu'à la première IMAGE. Or Desktop Duplication ne
    // rend une image qu'au changement du bureau : devant un Bloc-notes
    // statique, il n'en vient aucune. L'enfant, lui, bloque dans `lire_trame`
    // en attendant cette réponse, donc n'atteint jamais le signaling et
    // n'établit aucune session WebRTC. Relevé en recette (tâche 9,
    // 2 août 2026) : deux fenêtres attachées côté capteur, zéro
    // « attaché au capteur » côté enfant, deux pages en `iceConnectionState
    // = "new"`. Toutes les autres écritures de ce fichier sont déjà suivies
    // d'un `flush` ; celle-ci était la seule à ne pas l'être.
    ecrivain.flush()?;
    tracing::info!(%session, %sortie, largeur, hauteur, "fenêtre attachée au capteur");

    let mut dernier_etat = (true, false, largeur, hauteur);
    let mut images = 0u64;
    let mut dernier_compte = Instant::now();

    loop {
        // 1. Les commandes en attente, s'il y en a. Elles sont rares.
        loop {
            match commandes.try_recv() {
                Ok(message) => {
                    let reponse = executer_commande(&mut source, message);
                    ecrire_json(&mut ecrivain, &reponse)?;
                    ecrivain.flush()?;
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
    };
    match resultat {
        Ok(()) => DepuisCapteur::Fait,
        Err(erreur) => DepuisCapteur::Erreur { motif: format!("{erreur:#}") },
    }
}
