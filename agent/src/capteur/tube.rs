//! Le client de tube côté enfant : il se connecte au capteur, s'attache, et
//! rend une `SourceDistante` prête à servir la boucle de transport.

#![cfg(windows)]

use std::io::{BufReader, BufWriter, Write};
use std::sync::mpsc::{channel, sync_channel, Receiver, Sender, SyncSender};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};

use crate::capteur::distante::{Canal, Rattachee, Recu, SourceDistante};
use crate::capteur::horloge::lire_qpc;
use crate::capteur::protocole::{
    ecrire_json, lire_trame, DepuisCapteur, Trame, VersCapteur, NOM_TUBE,
};
use crate::capteur::reprise::DUREE_FENETRE_CANAL;

/// Profondeur de la file d'images entre le fil lecteur et `next_frame`.
///
/// **C'est elle qui exerce la contre-pression sur toute la chaîne** : file
/// pleine → le fil lecteur bloque → le tampon du tube se remplit → l'écriture
/// du capteur bloque, et son fil de fenêtre attend. Une unité d'accès ne peut
/// pas être jetée sans corrompre le flux, donc bloquer est la seule issue
/// correcte. 8 unités ≈ 90 ms de vidéo à 90 i/s : assez pour absorber un
/// à-coup d'ordonnancement, trop peu pour laisser une session dériver en
/// silence.
const CAPACITE_FILE: usize = 8;

/// Attente maximale d'une réponse à une commande.
///
/// **Majorant assumé** : `resize` peut reconstruire une chaîne d'encodage
/// complète, et `Drop for H264Encoder` porte une partie bornée de 8 s au pire
/// cas. Trop court, on déclarerait morte une fenêtre qui travaille.
const DELAI_COMMANDE: Duration = Duration::from_secs(10);

/// Pas entre deux tentatives de connexion. Même valeur que le pas de reprise
/// de `capture/reprise.rs`, et pour la même raison : assez petit pour ne pas
/// retarder la reprise réelle, assez grand pour que la trace reste rare.
const PAS_CONNEXION: Duration = Duration::from_millis(150);

pub fn connecter(
    session: &str,
    hwnd: u64,
    sortie: &str,
    fps: u32,
    debit: u32,
    clock_origin: Instant,
) -> Result<SourceDistante> {
    let signalement = Signalement {
        session: session.to_string(),
        hwnd,
        sortie: sortie.to_string(),
        fps,
        debit,
        clock_origin,
    };
    // La PREMIÈRE ouverture est patiente : l'enfant peut démarrer avant que le
    // capteur n'ait ouvert son tube. Les réouvertures de `rattacher`, elles,
    // ne le sont pas — elles courent depuis la boucle de transport.
    let attachee = attacher_sur(ouvrir_avec_patience()?, &signalement)?;
    let (largeur, hauteur) = (attachee.largeur, attachee.hauteur);
    Ok(SourceDistante::nouvelle(
        Box::new(CanalTube {
            ecrivain: Mutex::new(attachee.ecrivain),
            reponses: attachee.reponses,
            signalement,
        }),
        attachee.images,
        largeur,
        hauteur,
    ))
}

/// Envoie l'attache sur un tube déjà ouvert, lit la réponse, et démarre le fil
/// répartiteur. **Partagée par `connecter` et `rattacher`** : les deux ne
/// doivent pas porter deux copies de cette séquence.
fn attacher_sur(fichier: std::fs::File, signalement: &Signalement) -> Result<Attachee> {
    let mut ecrivain = BufWriter::new(fichier.try_clone().context("clone du tube en écriture")?);
    let mut lecteur = BufReader::new(fichier);

    // `clock_origin` a été créée par `demarrage.rs` AVANT cet appel — la
    // connexion peut avoir attendu le capteur plusieurs secondes, et un
    // rattachement survient bien plus tard encore. Lire QPC maintenant et
    // l'envoyer tel quel décalerait la vidéo de tout cet écart par rapport à
    // l'audio, qui partage `clock_origin`. On CORRIGE donc de l'écoulé, ce
    // qui rend l'origine exacte à chaque attache.
    let frequence = crate::capteur::horloge::frequence_qpc()?;
    let ecoule_tics = (signalement.clock_origin.elapsed().as_nanos() * frequence as u128
        / 1_000_000_000)
        .min(i64::MAX as u128) as i64;
    let origine_qpc = lire_qpc().context("lecture de QPC avant l'attache")? - ecoule_tics;

    ecrire_json(
        &mut ecrivain,
        &VersCapteur::Attache {
            session: signalement.session.clone(),
            hwnd: signalement.hwnd,
            sortie: signalement.sortie.clone(),
            fps: signalement.fps,
            debit: signalement.debit,
            origine_qpc,
        },
    )?;
    ecrivain.flush()?;

    let (largeur, hauteur) = match lire_trame(&mut lecteur).context("réponse à l'attache")? {
        Trame::Json(octets) => match serde_json::from_slice::<DepuisCapteur>(&octets)? {
            DepuisCapteur::Attachee { largeur, hauteur } => (largeur, hauteur),
            // Un refus fait échouer l'attache BRUYAMMENT : sans cela l'enfant
            // attendrait une image qui ne viendra jamais.
            DepuisCapteur::Refus { motif } => bail!("le capteur a refusé l'attache : {motif}"),
            autre => bail!("réponse inattendue à l'attache : {autre:?}"),
        },
        Trame::Image(_) => bail!("le capteur a répondu une image à l'attache"),
    };
    tracing::info!(
        session = %signalement.session,
        sortie = %signalement.sortie,
        largeur, hauteur,
        "attaché au capteur"
    );

    let (tx_images, rx_images) = sync_channel::<Recu>(CAPACITE_FILE);
    let (tx_reponses, rx_reponses) = channel::<DepuisCapteur>();
    std::thread::spawn(move || repartir_les_trames(lecteur, tx_images, tx_reponses));

    Ok(Attachee {
        ecrivain,
        reponses: rx_reponses,
        images: rx_images,
        largeur,
        hauteur,
    })
}

/// Réessaie la connexion dans une fenêtre bornée : l'enfant peut démarrer
/// avant que le capteur n'ait ouvert son tube — au tout premier lancement, ou
/// pendant une relance du capteur.
fn ouvrir_avec_patience() -> Result<std::fs::File> {
    let debut = Instant::now();
    let mut derniere = None;
    while debut.elapsed() <= DUREE_FENETRE_CANAL {
        match std::fs::OpenOptions::new().read(true).write(true).open(NOM_TUBE) {
            Ok(fichier) => return Ok(fichier),
            Err(erreur) => {
                derniere = Some(erreur);
                std::thread::sleep(PAS_CONNEXION);
            }
        }
    }
    Err(anyhow::Error::from(derniere.expect("au moins une tentative"))
        .context(format!("aucun capteur sur {NOM_TUBE} après {DUREE_FENETRE_CANAL:?}")))
}

/// Aiguille chaque trame reçue : les images vers la file bornée, les réponses
/// de commande vers leur propre canal. Les deux ne doivent PAS partager une
/// file — une réponse coincée derrière huit images bloquerait `commander`.
fn repartir_les_trames<R: std::io::Read>(
    mut lecteur: R,
    images: SyncSender<Recu>,
    reponses: Sender<DepuisCapteur>,
) {
    loop {
        let trame = match lire_trame(&mut lecteur) {
            Ok(trame) => trame,
            // Fin de tube : le capteur est parti. Laisser tomber les deux
            // émetteurs fait rendre `Disconnected` à `SourceDistante`, qui
            // OUVRE SA FENÊTRE DE REPRISE au lieu de clore la session.
            Err(_) => return,
        };
        let envoi = match trame {
            Trame::Image(unite) => images.send(Recu::Image(unite)).is_ok(),
            Trame::Json(octets) => match serde_json::from_slice::<DepuisCapteur>(&octets) {
                Ok(DepuisCapteur::Etat { vivante, epuisee, largeur, hauteur }) => images
                    .send(Recu::Etat { vivante, epuisee, largeur, hauteur })
                    .is_ok(),
                Ok(reponse) => reponses.send(reponse).is_ok(),
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

struct CanalTube {
    /// `Mutex` et non `&mut` : `Canal::commander` prend `&mut self`, mais
    /// l'écrivain est aussi le seul point d'écriture du tube et rien ne promet
    /// qu'il restera consulté depuis un seul fil.
    ecrivain: Mutex<BufWriter<std::fs::File>>,
    reponses: Receiver<DepuisCapteur>,
    /// De quoi se réattacher à un capteur relancé. Retenu à la connexion :
    /// au moment de la rupture, plus rien d'autre ne porte ces valeurs.
    signalement: Signalement,
}

/// Ce qu'il faut redire au capteur pour se réattacher.
///
/// `clock_origin` est retenue et NON figée en tics QPC : chaque attache
/// recalcule `origine_qpc` à partir d'elle, de sorte que l'origine reste
/// exacte quel que soit le temps écoulé depuis le démarrage de l'enfant.
struct Signalement {
    session: String,
    hwnd: u64,
    sortie: String,
    fps: u32,
    debit: u32,
    clock_origin: Instant,
}

/// Le fruit d'une attache réussie, côté enfant.
struct Attachee {
    ecrivain: BufWriter<std::fs::File>,
    reponses: Receiver<DepuisCapteur>,
    images: Receiver<Recu>,
    largeur: u32,
    hauteur: u32,
}

impl Canal for CanalTube {
    /// Rouvre un tube vers le capteur (relancé par le superviseur) et
    /// réémet l'attache. Remplace l'écrivain et le canal de réponses de
    /// CE `CanalTube`, et rend la file d'images neuve.
    ///
    /// **Sans cette méthode, la fenêtre de reprise de `SourceDistante` ne
    /// ferait que retarder la mort des sessions de 15 s** : rien d'autre
    /// n'ouvre jamais un second tube. Voir la tâche 3bis.
    ///
    /// Une seule tentative, sans patience interne : c'est `SourceDistante`
    /// qui tient le budget et l'espacement (`PAS_RATTACHEMENT`). Ouvrir le
    /// tube directement par `OpenOptions`, PAS par `ouvrir_avec_patience`,
    /// qui bloquerait la boucle de transport jusqu'à 15 s.
    fn rattacher(&mut self) -> Result<Rattachee> {
        let fichier = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(NOM_TUBE)
            .context("réouverture du tube du capteur")?;
        let attachee = attacher_sur(fichier, &self.signalement)?;
        // Remplacer l'état d'écriture de CE canal : l'ancien pointe sur un
        // tube mort, et `commander` l'emploierait encore.
        self.ecrivain = Mutex::new(attachee.ecrivain);
        self.reponses = attachee.reponses;
        Ok(Rattachee {
            images: attachee.images,
            largeur: attachee.largeur,
            hauteur: attachee.hauteur,
        })
    }

    fn commander(&mut self, message: VersCapteur) -> Result<DepuisCapteur> {
        {
            let mut ecrivain = self
                .ecrivain
                .lock()
                .unwrap_or_else(|empoisonne| empoisonne.into_inner());
            ecrire_json(&mut *ecrivain, &message)?;
            ecrivain.flush()?;
        }
        self.reponses
            .recv_timeout(DELAI_COMMANDE)
            .with_context(|| format!("aucune réponse du capteur à {message:?}"))
    }
}
