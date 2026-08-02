//! Le serveur de tube nommé du capteur.
//!
//! Chaque enfant s'y connecte et se décrit lui-même dans sa première trame :
//! il n'y a donc AUCUN canal superviseur → capteur, et aucune table d'état
//! partagée entre trois processus. La fermeture du tube EST le signal de fin
//! de vie d'une fenêtre.
//!
//! **Deux connexions par fenêtre, et un seul sens par bout.** Un enfant ouvre
//! d'abord la connexion de commandes (il y écrit son attache et y lit les
//! réponses), puis la connexion média (il y écrit sa seule `Identite`, et n'y
//! lit plus que des images). Aucun objet fichier ne porte donc jamais une
//! lecture et une écriture concurrentes — voir la tâche 10 du sous-bloc D4 :
//! la recette a établi qu'une écriture du capteur n'aboutissait pas tant
//! qu'une lecture bloquante était pendante sur la même instance de tube.

#![cfg(windows)]

use std::collections::HashMap;
use std::io::{BufReader, BufWriter, Write};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use windows::core::HRESULT;
use windows::Win32::Foundation::{CloseHandle, ERROR_PIPE_CONNECTED, HANDLE};
use windows::Win32::Storage::FileSystem::PIPE_ACCESS_DUPLEX;
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE,
    PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};

use crate::capteur::fenetre::Fenetre;
use crate::capteur::protocole::{
    ecrire_json, lire_trame, DepuisCapteur, Trame, VersCapteur, NOM_TUBE,
};
use crate::capteur::reprise::DUREE_FENETRE_CANAL;

/// Tampon de tube, dans les deux sens. Généreux à dessein : c'est lui qui
/// absorbe les à-coups avant que la contre-pression ne remonte jusqu'au fil
/// de capture.
const TAMPON: u32 = 1024 * 1024;

/// Attente maximale de la connexion média après une attache acceptée.
///
/// **Définie comme `DUREE_FENETRE_CANAL` et non comme une valeur libre** : un
/// enfant qui renonce à son canal et un capteur qui renonce à sa fenêtre
/// doivent se découvrir au même moment. Les laisser dériver ferait survivre
/// une fenêtre sans enfant, ou l'inverse.
const DELAI_CONNEXION_MEDIA: Duration = DUREE_FENETRE_CANAL;

/// Sessions attachées sur leur connexion de commandes et attendant leur
/// connexion média. Clé : l'identifiant de session.
///
/// `Mutex` et non `RefCell` : la boucle d'acceptation et les fils de commandes
/// y touchent tous deux.
static EN_ATTENTE_DE_MEDIA: OnceLock<Mutex<HashMap<String, Sender<std::fs::File>>>> =
    OnceLock::new();

fn registre() -> &'static Mutex<HashMap<String, Sender<std::fs::File>>> {
    EN_ATTENTE_DE_MEDIA.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Verrouille le registre en survivant à un empoisonnement : un fil de fenêtre
/// qui panique ne doit pas emporter l'accueil de toutes les suivantes.
fn registre_verrouille() -> std::sync::MutexGuard<'static, HashMap<String, Sender<std::fs::File>>> {
    registre()
        .lock()
        .unwrap_or_else(|empoisonne| empoisonne.into_inner())
}

pub fn servir() -> Result<()> {
    loop {
        // Une instance NEUVE par client. `PIPE_UNLIMITED_INSTANCES` n'a de
        // sens que parce que cette boucle ne bloque JAMAIS sur la trame
        // d'attache d'un enfant (voir plus bas) : sans ce détachement, une
        // seule instance écoutait à la fois malgré son nom, et un enfant
        // connecté qui n'envoie jamais son attache bloquait l'accueil de
        // toutes les fenêtres suivantes.
        let tube = creer_instance().context("création d'une instance de tube")?;

        match connecter(tube) {
            // Connecté : déporter TOUT l'accueil — lecture de la première
            // trame comprise — sur son propre fil, DÉTACHÉ et jamais joint,
            // pour la même raison que le fil de fenêtre : le démontage d'un
            // `WindowsSource` peut geler, et la boucle d'acceptation ne doit
            // jamais pouvoir l'être. C'est aussi ce qui reboucle immédiatement
            // pour écouter l'instance suivante, au lieu d'attendre cet
            // enfant-ci — et chaque enfant ouvre désormais DEUX connexions.
            Ok(()) => {
                // `HANDLE` porte un pointeur brut et n'est donc pas `Send` —
                // il traverse la frontière de fil sous forme d'entier, sans
                // risque : cette instance de tube n'est plus touchée par la
                // boucle d'acceptation une fois le fil lancé, donc aucune
                // aliasing entre les deux fils.
                let brut = tube.0 as usize;
                std::thread::spawn(move || {
                    let tube = HANDLE(brut as *mut _);
                    // Une attache ratée ne fait PAS tomber le serveur : les
                    // autres fenêtres continuent. C'est tout l'intérêt
                    // d'avoir un capteur qui survit à ses fenêtres.
                    if let Err(erreur) = accueillir(tube) {
                        tracing::warn!(%erreur, "attache d'un enfant refusée");
                    }
                });
            }
            // Échec réel de connexion (pas la course bénigne isolée par
            // `connecter`) : le tube refusé est fermé pour ne pas fuir, et la
            // boucle recrée une instance neuve. Ne fait pas tomber le
            // serveur non plus.
            Err(erreur) => {
                if let Err(fermeture) = unsafe { CloseHandle(tube) } {
                    tracing::warn!(%fermeture, "fermeture d'un tube refusé également en échec");
                }
                tracing::warn!(%erreur, "connexion d'un enfant refusée");
            }
        }
    }
}

/// Bloque jusqu'à ce qu'un enfant se connecte à `tube`.
///
/// **`ERROR_PIPE_CONNECTED` est un SUCCÈS déguisé en erreur.** Il signale
/// qu'un enfant s'est connecté dans l'intervalle entre `CreateNamedPipeW` et
/// cet appel — une course banale, attendue sous `PIPE_UNLIMITED_INSTANCES` —
/// et non un échec. Le confondre avec un échec réel tuerait le processus
/// capteur entier (donc les N fenêtres avec lui) à la première course.
fn connecter(tube: HANDLE) -> Result<()> {
    match unsafe { ConnectNamedPipe(tube, None) } {
        Ok(()) => Ok(()),
        Err(erreur) if erreur.code() == HRESULT::from_win32(ERROR_PIPE_CONNECTED.0) => Ok(()),
        Err(erreur) => Err(erreur).context("attente d'un enfant"),
    }
}

fn creer_instance() -> Result<HANDLE> {
    let nom: Vec<u16> = NOM_TUBE.encode_utf16().chain(std::iter::once(0)).collect();
    let tube = unsafe {
        CreateNamedPipeW(
            windows::core::PCWSTR(nom.as_ptr()),
            PIPE_ACCESS_DUPLEX,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
            PIPE_UNLIMITED_INSTANCES,
            TAMPON,
            TAMPON,
            0,
            None,
        )
    };
    if tube.is_invalid() {
        bail!("CreateNamedPipeW a rendu un handle invalide");
    }
    Ok(tube)
}

/// Lit la première trame et AIGUILLE sur son type : une attache ouvre une
/// connexion de commandes, une identité apparie une connexion média à une
/// session déjà attachée.
fn accueillir(tube: HANDLE) -> Result<()> {
    // `std::fs::File` depuis le handle : il donne `Read`/`Write` sans écrire
    // d'enveloppe, et sa fermeture ferme le tube.
    use std::os::windows::io::FromRawHandle;
    let fichier = unsafe { std::fs::File::from_raw_handle(tube.0 as *mut _) };
    // Le lecteur travaille sur un handle DUPLIQUÉ : `fichier` reste entier et
    // peut être remis tel quel au fil de fenêtre s'il s'agit d'une connexion
    // média. Fermer le duplicata ne ferme pas l'instance de tube.
    let mut lecteur = BufReader::new(fichier.try_clone().context("clone du tube en lecture")?);

    let premiere = match lire_trame(&mut lecteur).context("première trame de l'enfant")? {
        Trame::Json(octets) => {
            serde_json::from_slice::<VersCapteur>(&octets).context("première trame illisible")?
        }
        Trame::Image(_) => bail!("le premier message d'un enfant ne peut pas être une image"),
    };

    match premiere {
        VersCapteur::Attache { .. } => ouvrir_les_commandes(lecteur, fichier, premiere),
        // La connexion média : ce fil-ci ne fait que la remettre au fil de
        // fenêtre déjà en attente. Aucune lecture ne restera pendante dessus,
        // et l'enfant n'y écrira plus rien — d'où le `lecteur` qu'on laisse
        // tomber juste après.
        VersCapteur::Identite { session } => {
            let attendue = registre_verrouille().remove(&session);
            match attendue {
                Some(media) => {
                    drop(lecteur);
                    if media.send(fichier).is_err() {
                        tracing::warn!(
                            %session,
                            "fil de fenêtre disparu avant sa connexion média, connexion abandonnée"
                        );
                    }
                }
                // Jamais un panic, jamais un silence : une identité orpheline
                // se nomme, et sa connexion se referme en sortant d'ici.
                None => tracing::warn!(
                    %session,
                    "connexion média pour une session inconnue, abandonnée"
                ),
            }
            Ok(())
        }
        autre => bail!("première trame inattendue d'un enfant : {autre:?}"),
    }
}

/// Traite une connexion de **commandes** : construit la fenêtre, répond à
/// l'attache, puis boucle en stricte alternance lecture → exécution →
/// écriture. **Ce fil est le seul à toucher cette connexion.**
fn ouvrir_les_commandes(
    lecteur: BufReader<std::fs::File>,
    mut ecrivain: std::fs::File,
    attache: VersCapteur,
) -> Result<()> {
    let VersCapteur::Attache { ref session, .. } = attache else {
        bail!("le premier message d'un enfant doit être une attache");
    };
    let session = session.clone();

    let (media, attente_media) = channel::<std::fs::File>();
    // Un remplacement se journalise : il signale un enfant qui se rattache
    // sans que la précédente attente ait été soldée. Laisser tomber l'ancien
    // émetteur réveille aussitôt le fil de fenêtre correspondant.
    if registre_verrouille().insert(session.clone(), media).is_some() {
        tracing::warn!(%session, "attente de connexion média remplacée pour cette session");
    }

    let (commandes, receveur_commandes) = channel::<VersCapteur>();
    let (reponses, receveur_reponses) = channel::<DepuisCapteur>();

    // Fil de FENÊTRE : il tient le `WindowsSource`. DÉTACHÉ, jamais joint — la
    // boucle d'acceptation ne doit pas pouvoir être bloquée par un démontage
    // d'encodeur (`Drop for H264Encoder` peut geler).
    let session_fenetre = session.clone();
    std::thread::spawn(move || {
        if let Err(erreur) =
            tenir_la_fenetre(attache, session_fenetre, attente_media, receveur_commandes, reponses)
        {
            tracing::warn!(%erreur, "fil de fenêtre terminé sur erreur");
        }
    });

    // **La réponse à l'attache part AVANT toute lecture de commande.** L'enfant
    // la lit sur cette même connexion, et il n'ouvre sa connexion média
    // qu'après l'avoir lue : entrer directement dans la boucle de lecture le
    // laisserait bloqué à jamais. Bornée, pour qu'une construction de source
    // qui ne rendrait pas ne gèle pas ce fil.
    let premiere = match receveur_reponses.recv_timeout(DELAI_CONNEXION_MEDIA) {
        Ok(reponse) => reponse,
        Err(erreur) => {
            // L'expéditeur retire l'entrée qu'il a inscrite : si le fil de
            // fenêtre est resté dans sa construction de source, il n'atteindra
            // jamais sa propre attente, donc jamais son propre retrait.
            oublier(&session);
            tracing::warn!(%session, %erreur, "aucune réponse à l'attache, canal abandonné");
            return Ok(());
        }
    };
    // `ecrivain` est un `File` NU, sans tampon d'écriture : la recette de la
    // tâche 9 avait vu une réponse d'attache dormir dans un `BufWriter`
    // jusqu'à la première image — qui ne vient jamais devant une fenêtre
    // immobile. Le `flush` reste, mais c'est l'absence de tampon qui garantit.
    let refusee = matches!(premiere, DepuisCapteur::Refus { .. });
    ecrire_json(&mut ecrivain, &premiere).context("réponse à l'attache")?;
    ecrivain.flush().context("réponse à l'attache")?;
    if refusee {
        // Le refus est écrit, l'enfant le lira ; rien d'autre ne viendra sur
        // cette connexion. La fermer en sortant est la fin normale.
        return Ok(());
    }

    boucler_les_commandes(lecteur, ecrivain, &session, commandes, receveur_reponses);
    Ok(())
}

/// Le fil de fenêtre, de bout en bout : construire la source, annoncer le
/// résultat au fil de commandes, attendre la connexion média, servir.
fn tenir_la_fenetre(
    attache: VersCapteur,
    session: String,
    attente_media: Receiver<std::fs::File>,
    commandes: Receiver<VersCapteur>,
    reponses: Sender<DepuisCapteur>,
) -> Result<()> {
    let fenetre = match Fenetre::ouvrir(attache) {
        Ok(fenetre) => fenetre,
        Err(erreur) => {
            // Le refus est ANNONCÉ à l'enfant, jamais silencieux : sans ce
            // message il attendrait une image qui ne viendra pas.
            let _ = reponses.send(DepuisCapteur::Refus { motif: format!("{erreur:#}") });
            oublier(&session);
            return Err(erreur);
        }
    };
    let (largeur, hauteur) = fenetre.dimensions();
    if reponses.send(DepuisCapteur::Attachee { largeur, hauteur }).is_err() {
        oublier(&session);
        bail!("le fil de commandes de {session} est parti avant la réponse à l'attache");
    }

    // L'enfant n'ouvre sa connexion média qu'APRÈS avoir lu `Attachee` : c'est
    // ce qui garantit qu'il connaît ses dimensions, et c'est pourquoi cette
    // attente vient ici et pas plus tôt.
    let media = match attente_media.recv_timeout(DELAI_CONNEXION_MEDIA) {
        Ok(media) => media,
        Err(RecvTimeoutError::Timeout) => {
            // Le receveur retire SA propre entrée : un enfant mort entre ses
            // deux connexions laisserait sinon une entrée éternelle.
            oublier(&session);
            tracing::warn!(
                %session,
                delai = ?DELAI_CONNEXION_MEDIA,
                "aucune connexion média, fenêtre abandonnée"
            );
            return Ok(());
        }
        // L'émetteur a été remplacé au registre (rattachement) ou laissé
        // tomber : l'entrée courante ne nous appartient plus, on ne la retire
        // surtout pas.
        Err(RecvTimeoutError::Disconnected) => {
            tracing::warn!(%session, "attente de connexion média rompue, fenêtre abandonnée");
            return Ok(());
        }
    };
    fenetre.servir(commandes, reponses, BufWriter::new(media))
}

/// Retire l'entrée de cette session du registre.
///
/// ⚠️ **Ne distingue pas deux attentes successives de la même session.** Un
/// abandon qui expire à l'instant précis où la même session vient de se
/// réinscrire retirerait l'entrée neuve ; l'enfant s'en remet par sa fenêtre
/// de reprise. Course jugée négligeable, pas inexistante.
fn oublier(session: &str) {
    registre_verrouille().remove(session);
}

/// La boucle de la connexion de commandes : lire une commande, la faire
/// exécuter par le fil de fenêtre, écrire sa réponse. **Stricte alternance sur
/// un seul fil** : jamais de lecture pendante pendant qu'on écrit.
fn boucler_les_commandes(
    mut lecteur: BufReader<std::fs::File>,
    mut ecrivain: std::fs::File,
    session: &str,
    commandes: Sender<VersCapteur>,
    reponses: Receiver<DepuisCapteur>,
) {
    loop {
        let message = match lire_trame(&mut lecteur) {
            Ok(Trame::Json(octets)) => match serde_json::from_slice::<VersCapteur>(&octets) {
                Ok(message) => message,
                Err(erreur) => {
                    tracing::warn!(%session, %erreur, "commande illisible, canal abandonné");
                    return;
                }
            },
            Ok(Trame::Image(_)) => {
                tracing::warn!(%session, "un enfant a envoyé une image, canal abandonné");
                return;
            }
            // Fin de tube : l'enfant est parti. Laisser tomber `commandes`
            // signale `Disconnected` au fil de fenêtre, qui démonte sa source.
            Err(_) => return,
        };
        if commandes.send(message).is_err() {
            return; // le fil de fenêtre est parti
        }
        // `recv` sans délai : le fil de fenêtre répond toujours, ou meurt — et
        // sa mort laisse tomber `reponses`, ce qui rend `Disconnected` ici.
        let Ok(reponse) = reponses.recv() else {
            return;
        };
        if ecrire_json(&mut ecrivain, &reponse)
            .and_then(|()| ecrivain.flush())
            .is_err()
        {
            return;
        }
    }
}
