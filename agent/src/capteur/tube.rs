//! Le client de tube côté enfant : il se connecte au capteur, s'attache, et
//! rend une `SourceDistante` prête à servir la boucle de transport.
//!
//! **Deux connexions, un seul sens par bout.** La connexion de commandes (B)
//! est écrite puis lue par le seul fil appelant, en stricte alternance ; la
//! connexion média (A) ne porte qu'une trame d'identité à l'ouverture, puis
//! n'est plus que lue, par le seul fil répartiteur. Aucun objet fichier ne
//! porte donc jamais une lecture et une écriture concurrentes — voir la
//! tâche 10 du sous-bloc D4.

#![cfg(windows)]

use std::io::{BufReader, Write};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
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

/// Pas entre deux tentatives de connexion. Même valeur que le pas de reprise
/// de `capture/reprise.rs`, et pour la même raison : assez petit pour ne pas
/// retarder la reprise réelle, assez grand pour que la trace reste rare.
const PAS_CONNEXION: Duration = Duration::from_millis(150);

/// Fenêtre d'ouverture de la connexion média, une fois l'attache acceptée.
///
/// Le capteur ne recrée son instance d'écoute qu'après avoir accepté la
/// précédente : une ouverture immédiate peut tomber sur `ERROR_PIPE_BUSY`.
/// Une seconde couvre largement cette course sans rien retarder — l'attache
/// vient d'aboutir, donc le capteur est vivant et son serveur tourne.
const DUREE_OUVERTURE_MEDIA: Duration = Duration::from_secs(1);

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
    let commandes = ouvrir_dans(DUREE_FENETRE_CANAL)?;
    let attachee = attacher_sur(commandes, &signalement)?;
    let (largeur, hauteur) = (attachee.largeur, attachee.hauteur);
    Ok(SourceDistante::nouvelle(
        Box::new(CanalTube {
            commandes: Mutex::new(attachee.commandes),
            signalement,
        }),
        attachee.images,
        largeur,
        hauteur,
    ))
}

/// Attache l'enfant au capteur sur un tube de commandes déjà ouvert, puis
/// ouvre la connexion média. **Partagée par `connecter` et `rattacher`** : les
/// deux ne doivent pas porter deux copies de cette séquence.
///
/// L'ordre est imposé : les commandes d'abord (l'attache y rend les
/// dimensions), le média ensuite (l'identité y apparie la connexion à la
/// session déjà attachée). Le capteur ne saurait pas apparier l'inverse.
fn attacher_sur(mut commandes: std::fs::File, signalement: &Signalement) -> Result<Attachee> {
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

    // Écriture PUIS lecture, sur CE fil, sans aucun tampon d'écriture : c'est
    // déjà la discipline de `commander`, et l'attache l'inaugure.
    ecrire_json(
        &mut commandes,
        &VersCapteur::Attache {
            session: signalement.session.clone(),
            hwnd: signalement.hwnd,
            sortie: signalement.sortie.clone(),
            fps: signalement.fps,
            debit: signalement.debit,
            origine_qpc,
        },
    )?;
    commandes.flush()?;

    let (largeur, hauteur) = match lire_trame(&mut commandes).context("réponse à l'attache")? {
        Trame::Json(octets) => match serde_json::from_slice::<DepuisCapteur>(&octets)? {
            DepuisCapteur::Attachee { largeur, hauteur } => (largeur, hauteur),
            // Un refus fait échouer l'attache BRUYAMMENT : sans cela l'enfant
            // attendrait une image qui ne viendra jamais.
            DepuisCapteur::Refus { motif } => bail!("le capteur a refusé l'attache : {motif}"),
            autre => bail!("réponse inattendue à l'attache : {autre:?}"),
        },
        Trame::Image(_) => bail!("le capteur a répondu une image à l'attache"),
    };

    // La connexion MÉDIA. On y écrit son identité — la seule et unique trame
    // que ce bout y écrira jamais — puis on la confie au fil répartiteur, qui
    // ne fait que lire. C'est ce qui rend impossible qu'une lecture et une
    // écriture s'y croisent.
    let mut media = ouvrir_dans(DUREE_OUVERTURE_MEDIA).context("ouverture de la connexion média")?;
    ecrire_json(&mut media, &VersCapteur::Identite { session: signalement.session.clone() })?;
    media.flush()?;

    tracing::info!(
        session = %signalement.session,
        sortie = %signalement.sortie,
        largeur, hauteur,
        "attaché au capteur"
    );

    let (tx_images, rx_images) = sync_channel::<Recu>(CAPACITE_FILE);
    std::thread::spawn(move || lire_le_media(BufReader::new(media), tx_images));

    Ok(Attachee { commandes, images: rx_images, largeur, hauteur })
}

/// Une seule tentative d'ouverture d'une instance du tube.
fn ouvrir_une_instance() -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new().read(true).write(true).open(NOM_TUBE)
}

/// Réessaie l'ouverture dans une fenêtre bornée : l'enfant peut démarrer avant
/// que le capteur n'ait ouvert son tube (au tout premier lancement, ou pendant
/// une relance du capteur), et une instance d'écoute peut être momentanément
/// occupée entre deux accueils.
fn ouvrir_dans(fenetre: Duration) -> Result<std::fs::File> {
    let debut = Instant::now();
    let mut derniere = None;
    while debut.elapsed() <= fenetre {
        match ouvrir_une_instance() {
            Ok(fichier) => return Ok(fichier),
            Err(erreur) => {
                derniere = Some(erreur);
                std::thread::sleep(PAS_CONNEXION);
            }
        }
    }
    Err(anyhow::Error::from(derniere.expect("au moins une tentative"))
        .context(format!("aucun capteur sur {NOM_TUBE} après {fenetre:?}")))
}

/// Lit la connexion média — et **rien d'autre** : images et états. Une réponse
/// de commande n'y transite pas, elle est lue par `commander` sur la connexion
/// de commandes.
fn lire_le_media<R: std::io::Read>(mut lecteur: R, images: SyncSender<Recu>) {
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

struct CanalTube {
    /// La connexion de commandes. `Mutex` et non `&mut` : `Canal::commander`
    /// prend `&mut self`, mais c'est l'alternance écriture → lecture qui doit
    /// rester indivisible, et rien ne promet que l'appelant sera toujours le
    /// même fil.
    commandes: Mutex<std::fs::File>,
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
    commandes: std::fs::File,
    images: Receiver<Recu>,
    largeur: u32,
    hauteur: u32,
}

impl Canal for CanalTube {
    /// Rouvre les DEUX connexions vers le capteur (relancé par le
    /// superviseur) et réémet l'attache. Remplace la connexion de commandes de
    /// CE `CanalTube`, et rend la file d'images neuve.
    ///
    /// **Sans cette méthode, la fenêtre de reprise de `SourceDistante` ne
    /// ferait que retarder la mort des sessions de 15 s** : rien d'autre
    /// n'ouvre jamais un second tube. Voir la tâche 3bis.
    ///
    /// Une seule tentative sur la connexion de commandes, sans patience
    /// interne : c'est `SourceDistante` qui tient le budget et l'espacement
    /// (`PAS_RATTACHEMENT`). Une ouverture patiente bloquerait la boucle de
    /// transport jusqu'à 15 s.
    fn rattacher(&mut self) -> Result<Rattachee> {
        let commandes = ouvrir_une_instance().context("réouverture du tube du capteur")?;
        let attachee = attacher_sur(commandes, &self.signalement)?;
        // Remplacer la connexion de CE canal : l'ancienne pointe sur un tube
        // mort, et `commander` l'emploierait encore.
        self.commandes = Mutex::new(attachee.commandes);
        Ok(Rattachee {
            images: attachee.images,
            largeur: attachee.largeur,
            hauteur: attachee.hauteur,
        })
    }

    /// ⚠️ **Cette lecture n'a AUCUN délai, et c'est assumé.** Le `recv_timeout`
    /// qui bornait autrefois l'attente a disparu avec le fil lecteur, et il
    /// n'existe pas d'équivalent de `SO_RCVTIMEO` pour un tube nommé
    /// synchrone. La parade retenue est que le capteur ferme ses tubes en
    /// mourant — le job object garantit sa mort, et la fermeture de ses
    /// handles avec —, ce qui fait rendre une **erreur** à cette lecture
    /// plutôt que de la suspendre. **À vérifier explicitement à la recette** :
    /// tuer le capteur pendant que des commandes circulent, et constater que
    /// `commander` rend une erreur.
    fn commander(&mut self, message: VersCapteur) -> Result<DepuisCapteur> {
        let mut commandes = self
            .commandes
            .lock()
            .unwrap_or_else(|empoisonne| empoisonne.into_inner());
        // Écriture PUIS lecture sur le même fil : c'est la discipline qui
        // rend le blocage impossible. Ne jamais introduire de fil lecteur
        // sur cette connexion — voir la tâche 10 du sous-bloc D4.
        ecrire_json(&mut *commandes, &message)?;
        commandes.flush()?;
        match lire_trame(&mut *commandes).context("réponse du capteur")? {
            Trame::Json(octets) => Ok(serde_json::from_slice(&octets)?),
            Trame::Image(_) => bail!("le capteur a répondu une image à une commande"),
        }
    }
}
