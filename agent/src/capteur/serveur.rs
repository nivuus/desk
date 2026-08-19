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

use std::io::{BufReader, BufWriter, Write};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use windows::Win32::Foundation::{CloseHandle, HANDLE};

use crate::capteur::fenetre::Fenetre;
use crate::capteur::protocole::{ecrire_json, lire_trame, DepuisCapteur, Trame, VersCapteur};
use crate::capteur::tube::DUREE_OUVERTURE_MEDIA;

mod attentes;
use attentes::{attendre_le_media, oublier};

mod instances;
use instances::{connecter, creer_instance, SOUFFLE_CREATION_INSTANCE};

/// Attente maximale de la connexion média après une attache acceptée.
///
/// **Alignée sur la patience de l'enfant (`tube::DUREE_OUVERTURE_MEDIA`), et
/// non sur `DUREE_FENETRE_CANAL`.** La tâche 10 prescrivait 15 s « pour qu'un
/// enfant qui abandonne et un capteur qui renonce se découvrent au même
/// moment » ; mais sur CETTE connexion, ce qui gouverne l'abandon de l'enfant
/// est `DUREE_OUVERTURE_MEDIA`, pas la fenêtre de reprise d'une session déjà
/// établie. Passé son propre budget, l'enfant échoue et meurt : attendre plus
/// longtemps ne rattraperait personne.
///
/// ⚠️ **Et attendre coûte cher ici** : à ce point la `Fenetre` est DÉJÀ
/// construite, donc la duplication DXGI de cette sortie est déjà prise. Or DXGI
/// n'en autorise qu'une par sortie : chaque seconde d'attente inutile est une
/// seconde pendant laquelle toute relance de cette fenêtre serait refusée en
/// `0x80070057` — et avec `RELANCES_MAX = 3` une fenêtre peut brûler ses quatre
/// tentatives dans un tel trou. D'où la marge d'une seconde, et pas davantage.
const DELAI_CONNEXION_MEDIA: Duration =
    DUREE_OUVERTURE_MEDIA.saturating_add(Duration::from_secs(1));

/// Attente maximale d'une réponse du fil de fenêtre — à l'attache comme à
/// chaque commande.
///
/// **Majorant assumé, strictement supérieur au pire cas documenté.** `resize`
/// peut reconstruire une chaîne d'encodage complète, et `Drop for H264Encoder`
/// porte une partie bornée de 8 s au pire cas (`2 × DELAI_BARRIERE +
/// 2 × DELAI_ARRET_MFT`) ; 12 s laissent 4 s pour la reconstruction. Trop
/// court, on déclarerait morte une fenêtre qui travaille.
///
/// **C'est la ceinture, pas la bretelle.** Le remède de l'interblocage relevé
/// par la revue de la tâche 10 est structurel (le fil écrivain de
/// `fenetre.rs`) ; cette borne existe pour qu'un fil de fenêtre muet pour une
/// raison non prévue rende à l'enfant une **erreur** — que la boucle de
/// transport absorbe déjà par un `warn!` — au lieu de le figer pour toujours.
/// L'enfant, lui, n'a plus aucun délai sur sa lecture.
const DELAI_REPONSE_FENETRE: Duration = Duration::from_secs(12);

pub fn servir() -> Result<()> {
    // Vrai dès qu'un échec de `creer_instance` a été signalé — voir plus bas.
    let mut echec_signale = false;
    loop {
        // Une instance NEUVE par client. `PIPE_UNLIMITED_INSTANCES` n'a de
        // sens que parce que cette boucle ne bloque JAMAIS sur la trame
        // d'attache d'un enfant (voir plus bas) : sans ce détachement, une
        // seule instance écoutait à la fois malgré son nom, et un enfant
        // connecté qui n'envoie jamais son attache bloquait l'accueil de
        // toutes les fenêtres suivantes.
        // **Non fatal, comme les deux autres erreurs de cette boucle**
        // (correctif I5 de la revue finale de branche). Un `?` ici faisait
        // tomber le capteur ENTIER — donc les N sessions — pour un échec de
        // `CreateNamedPipeW` qui peut être transitoire (épuisement momentané
        // d'une ressource système), alors que la boucle sait déjà survivre à
        // un refus de connexion et à une attache ratée. Le souffle évite d'en
        // faire une boucle serrée si la cause, elle, persiste : sans lui, un
        // échec permanent produirait des milliers de lignes par seconde sur le
        // partage CIFS.
        //
        // La ligne est signalée une seule fois par série d'échecs, même motif
        // qu'`Enfant::etat_illisible_signale` (`superviseur/lanceur.rs`) : le
        // souffle seul ne suffirait pas à borner le journal si la cause dure.
        let tube = match creer_instance() {
            Ok(tube) => {
                if echec_signale {
                    echec_signale = false;
                    tracing::info!("création d'instances de tube rétablie");
                }
                tube
            }
            Err(erreur) => {
                if !echec_signale {
                    echec_signale = true;
                    tracing::warn!(
                        %erreur,
                        "création d'une instance de tube refusée, réessais \
                         (signalé une seule fois tant que l'échec se répète)"
                    );
                }
                std::thread::sleep(SOUFFLE_CREATION_INSTANCE);
                continue;
            }
        };

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
            // `retirer_pour_identite` ignore la génération : voir son
            // commentaire dans `attentes.rs`.
            let attendue = attentes::retirer_pour_identite(&session);
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
    // L'insertion et son journal de remplacement vivent désormais dans
    // `attendre_le_media`, qui rend la génération de CETTE inscription — la
    // valeur que ce fil doit redonner telle quelle à `oublier`.
    let generation = attendre_le_media(&session, media);

    let (commandes, receveur_commandes) = channel::<VersCapteur>();
    let (reponses, receveur_reponses) = channel::<DepuisCapteur>();

    // Fil de FENÊTRE : il tient le `WindowsSource`. DÉTACHÉ, jamais joint — la
    // boucle d'acceptation ne doit pas pouvoir être bloquée par un démontage
    // d'encodeur (`Drop for H264Encoder` peut geler).
    let session_fenetre = session.clone();
    std::thread::spawn(move || {
        if let Err(erreur) = tenir_la_fenetre(
            attache,
            session_fenetre,
            generation,
            attente_media,
            receveur_commandes,
            reponses,
        ) {
            tracing::warn!(%erreur, "fil de fenêtre terminé sur erreur");
        }
    });

    // **La réponse à l'attache part AVANT toute lecture de commande.** L'enfant
    // la lit sur cette même connexion, et il n'ouvre sa connexion média
    // qu'après l'avoir lue : entrer directement dans la boucle de lecture le
    // laisserait bloqué à jamais. Bornée, pour qu'une construction de source
    // qui ne rendrait pas ne gèle pas ce fil.
    let premiere = match receveur_reponses.recv_timeout(DELAI_REPONSE_FENETRE) {
        Ok(reponse) => reponse,
        Err(erreur) => {
            // L'expéditeur retire l'entrée qu'il a inscrite : si le fil de
            // fenêtre est resté dans sa construction de source, il n'atteindra
            // jamais sa propre attente, donc jamais son propre retrait.
            oublier(&session, generation);
            tracing::warn!(%session, %erreur, "aucune réponse à l'attache, canal abandonné");
            // Un REFUS explicite plutôt qu'une fermeture muette : l'enfant le
            // lit et échoue bruyamment, au lieu d'interpréter une fin de tube.
            let _ = ecrire_json(
                &mut ecrivain,
                &DepuisCapteur::Refus {
                    motif: format!("aucune réponse du fil de fenêtre en {DELAI_REPONSE_FENETRE:?}"),
                },
            );
            let _ = ecrivain.flush();
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
    generation: u64,
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
            oublier(&session, generation);
            return Err(erreur);
        }
    };
    let (largeur, hauteur) = fenetre.dimensions();
    if reponses.send(DepuisCapteur::Attachee { largeur, hauteur }).is_err() {
        oublier(&session, generation);
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
            oublier(&session, generation);
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
        // **Bornée, et c'est la ceinture de sécurité de tout le canal.**
        // L'enfant attend cette réponse sans aucun délai : sans borne ici, un
        // fil de fenêtre muet le figerait pour toujours. À l'expiration on lui
        // rend une `Erreur`, que `SourceDistante` remonte et que la boucle de
        // transport absorbe déjà par un `warn!` — puis on clôt la connexion.
        let reponse = match reponses.recv_timeout(DELAI_REPONSE_FENETRE) {
            Ok(reponse) => reponse,
            Err(RecvTimeoutError::Timeout) => {
                tracing::warn!(
                    %session,
                    delai = ?DELAI_REPONSE_FENETRE,
                    "le fil de fenêtre n'a pas répondu, canal clos"
                );
                let motif = format!("le fil de fenêtre n'a pas répondu en {DELAI_REPONSE_FENETRE:?}");
                let _ = ecrire_json(&mut ecrivain, &DepuisCapteur::Erreur { motif });
                let _ = ecrivain.flush();
                return;
            }
            // Le fil de fenêtre est parti : la fin de tube le dira à l'enfant.
            Err(RecvTimeoutError::Disconnected) => return,
        };
        if ecrire_json(&mut ecrivain, &reponse)
            .and_then(|()| ecrivain.flush())
            .is_err()
        {
            return;
        }
    }
}
