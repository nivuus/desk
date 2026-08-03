//! Le fil d'une fenêtre : il tient un `WindowsSource` complet et le sert à
//! l'enfant par le tube.
//!
//! **Il ne réécrit AUCUN code de capture ni d'encodage.** `WindowsSource` est
//! déjà exactement le couple `DesktopCapture` + `H264Encoder` derrière le
//! trait `VideoSource` : ce module ne fait qu'appeler ce trait et transporter
//! ses résultats. C'est la simplification centrale du sous-bloc D4.

#![cfg(windows)]

use std::io::Write;
use std::sync::mpsc::{sync_channel, Receiver, Sender, SyncSender, TryRecvError, TrySendError};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use windows::Win32::Foundation::HWND;

use crate::capteur::horloge::{frequence_qpc, lire_qpc, origine_depuis_qpc};
use crate::capteur::protocole::{ecrire_image, ecrire_json, DepuisCapteur, VersCapteur};
use crate::h264::AccessUnit;
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

/// Profondeur de la file entre le fil de fenêtre et le fil écrivain de la
/// connexion média.
///
/// **Bornée à dessein** : une file libre laisserait s'accumuler sans limite des
/// unités d'accès qu'un enfant qui ne lit plus ne prendra jamais. C'est le
/// pendant exact de `CAPACITE_FILE` côté enfant, et la contre-pression continue
/// donc de remonter jusqu'à la capture — mais elle remonte désormais dans
/// `deposer`, qui sert les commandes à chaque tour d'attente.
const CAPACITE_ECRITURES: usize = 8;

/// Ce que le fil de fenêtre confie au fil écrivain de la connexion média.
enum AEcrire {
    Image(AccessUnit),
    Etat(DepuisCapteur),
}

/// Faut-il continuer la boucle de fenêtre, ou la clore — et pourquoi.
enum Fin {
    Continuer,
    Terminer(&'static str),
}

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
    /// `ecrivain` est la connexion **média**, et elle est confiée à un fil
    /// ÉCRIVAIN dédié : ce fil-ci ne touche plus aucun objet fichier. Les
    /// réponses aux commandes partent par `reponses`, vers le fil qui tient la
    /// connexion de commandes et qui les écrit lui-même. Aucun objet fichier
    /// ne porte donc jamais une lecture et une écriture concurrentes.
    ///
    /// **Pourquoi un fil écrivain plutôt qu'une écriture directe.** Une
    /// écriture bloquante ici bloquait le fil de fenêtre *après* son sondage
    /// des commandes, donc sans en servir aucune — et l'enfant, qui attend sa
    /// réponse sans délai depuis la tâche 10, ne pouvait plus jamais reprendre
    /// sa lecture du média : interblocage à six maillons, relevé par la revue
    /// de la tâche 10. La seule attente que ce fil peut encore subir est celle
    /// de `deposer`, **qui sert les commandes à chaque tour**.
    pub fn servir<E: Write + Send + 'static>(
        mut self,
        commandes: Receiver<VersCapteur>,
        reponses: Sender<DepuisCapteur>,
        ecrivain: E,
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

        let (ecritures, a_ecrire) = sync_channel::<AEcrire>(CAPACITE_ECRITURES);
        let session_ecrivain = session.clone();
        std::thread::spawn(move || ecrire_le_media(ecrivain, a_ecrire, &session_ecrivain));

        let source = &mut self.source;
        let mut dernier_etat = (true, false, self.largeur, self.hauteur);
        let mut images = 0u64;
        let mut dernier_compte = Instant::now();

        loop {
            // 1. Les commandes en attente, s'il y en a. Elles sont rares.
            if let Fin::Terminer(motif) = servir_les_commandes(source, &commandes, &reponses) {
                // On sort par le haut, ce qui relâche `source` — donc la
                // duplication et l'encodeur — SUR CE FIL-CI, jamais sur la
                // boucle d'acceptation. `Drop for H264Encoder` peut geler
                // (risque observé, non attribué) ; ici il ne gèlerait que
                // cette fenêtre.
                tracing::info!(%session, images, motif, "fin de la fenêtre côté capteur");
                return Ok(());
            }

            // 2. Une image, s'il y en a une.
            match source.next_frame() {
                Some(unite) => {
                    images += 1;
                    // La file bornée EST la contre-pression : si l'enfant ne
                    // lit plus, ce fil finit par attendre — et il n'attend que
                    // pour SA fenêtre, sans jamais cesser de servir les
                    // commandes. Une unité d'accès ne peut pas être jetée sans
                    // corrompre le flux (les images P référencent les
                    // précédentes), d'où l'attente plutôt que l'abandon.
                    if let Fin::Terminer(motif) = deposer(
                        AEcrire::Image(unite),
                        &ecritures,
                        source,
                        &commandes,
                        &reponses,
                    ) {
                        tracing::info!(%session, images, motif, "fin de la fenêtre côté capteur");
                        return Ok(());
                    }
                }
                None => std::thread::sleep(PAS_A_VIDE),
            }

            // 3. L'état, au CHANGEMENT seulement.
            let (largeur, hauteur) = source.dimensions();
            let etat = (source.is_alive(), source.is_exhausted(), largeur, hauteur);
            if etat != dernier_etat {
                dernier_etat = etat;
                let message = DepuisCapteur::Etat {
                    vivante: etat.0,
                    epuisee: etat.1,
                    largeur: etat.2,
                    hauteur: etat.3,
                };
                if let Fin::Terminer(motif) =
                    deposer(AEcrire::Etat(message), &ecritures, source, &commandes, &reponses)
                {
                    tracing::info!(%session, images, motif, "fin de la fenêtre côté capteur");
                    return Ok(());
                }
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

/// Vide la file des commandes en attente et renvoie chaque réponse au fil de
/// commandes. **Ne bloque jamais** : `try_recv` d'un côté, `Sender` non borné
/// de l'autre.
fn servir_les_commandes(
    source: &mut WindowsSource,
    commandes: &Receiver<VersCapteur>,
    reponses: &Sender<DepuisCapteur>,
) -> Fin {
    loop {
        match commandes.try_recv() {
            Ok(message) => {
                let reponse = executer_commande(source, message);
                // La réponse repart par le canal, jamais par une écriture
                // directe : ce fil ne touche aucun objet fichier.
                if reponses.send(reponse).is_err() {
                    return Fin::Terminer("le fil de commandes est parti");
                }
            }
            Err(TryRecvError::Empty) => return Fin::Continuer,
            // L'enfant a fermé sa connexion de commandes : la fenêtre est finie.
            Err(TryRecvError::Disconnected) => return Fin::Terminer("l'enfant a fermé le canal"),
        }
    }
}

/// Dépose une charge pour le fil écrivain de la connexion média.
///
/// ⚠️ **C'est le SEUL point où le fil de fenêtre peut attendre, et c'est ce qui
/// garantit qu'il ne peut jamais attendre sans servir les commandes.** La file
/// est bornée pour que la contre-pression remonte jusqu'à la capture ; quand
/// elle est pleine, on ne bloque pas dessus — on sert les commandes, on souffle
/// un pas, et on réessaie. Un `send` bloquant ici recréerait exactement
/// l'interblocage que la tâche 10 devait supprimer : enfant figé dans
/// `commander` → file d'images de l'enfant pleine → tampon du tube plein →
/// écriture du capteur bloquée → commande jamais servie → enfant figé.
fn deposer(
    charge: AEcrire,
    ecritures: &SyncSender<AEcrire>,
    source: &mut WindowsSource,
    commandes: &Receiver<VersCapteur>,
    reponses: &Sender<DepuisCapteur>,
) -> Fin {
    let mut charge = charge;
    loop {
        match ecritures.try_send(charge) {
            Ok(()) => return Fin::Continuer,
            Err(TrySendError::Full(rendue)) => {
                charge = rendue;
                if let Fin::Terminer(motif) = servir_les_commandes(source, commandes, reponses) {
                    return Fin::Terminer(motif);
                }
                std::thread::sleep(PAS_A_VIDE);
            }
            // Le fil écrivain est mort : la connexion média est perdue.
            Err(TrySendError::Disconnected(_)) => {
                return Fin::Terminer("la connexion média est fermée")
            }
        }
    }
}

/// Le fil écrivain de la connexion média : il ne fait qu'écrire, et il est le
/// seul à toucher cet objet fichier. Personne ne le lit.
fn ecrire_le_media<E: Write>(mut ecrivain: E, charges: Receiver<AEcrire>, session: &str) {
    for charge in charges {
        let ecrit = match charge {
            AEcrire::Image(unite) => ecrire_image(&mut ecrivain, &unite),
            AEcrire::Etat(message) => ecrire_json(&mut ecrivain, &message),
        };
        // `flush` à chaque charge : devant une fenêtre immobile, la charge
        // suivante peut ne jamais venir, et l'enfant attendrait celle-ci dans
        // un tampon. Même leçon que la réponse d'attache de la tâche 9.
        if let Err(erreur) = ecrit.and_then(|()| ecrivain.flush()) {
            tracing::warn!(%session, %erreur, "écriture de la connexion média interrompue");
            return;
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
        // Ouvert pour l'exhaustivité du protocole (tâche 4), câblé par la
        // tâche 6 vers le registre de sommeil.
        VersCapteur::Visibilite { .. } => {
            return DepuisCapteur::Erreur {
                motif: "visibilité reçue avant que le vivier soit câblé".into(),
            }
        }
    };
    match resultat {
        Ok(()) => DepuisCapteur::Fait,
        Err(erreur) => DepuisCapteur::Erreur { motif: format!("{erreur:#}") },
    }
}
