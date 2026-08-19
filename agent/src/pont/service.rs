//! **Le fil du pont** : il possède la table, complète les commandes ProjFS, et
//! balaie les expirations. C'est la catégorie 3 de la discipline de fil décrite
//! en tête de [`crate::pont::projfs`].
//!
//! Il ne possède **ni** le `Rtc` **ni** le socket — c'est
//! [`crate::pont::transport::tourner`] qui les tient, sur son propre fil, et
//! les deux se parlent par les deux `mpsc` que le plan définit. La raison de ce
//! découpage est écrite en tête de [`crate::pont::projfs`], sous la divergence
//! qu'elle constitue.
//!
//! # Ce que ce fil ne fait JAMAIS
//!
//! - **Il ne parcourt pas la racine.** Un `read_dir` sur la racine traverserait
//!   ProjFS, donc déclencherait nos propres rappels d'énumération, qui
//!   inscrivent une commande que **ce fil-ci** doit compléter : il
//!   s'attendrait lui-même. C'est pourquoi le relevé d'hydratation compte ce
//!   que le pont écrit au lieu de mesurer le disque.
//! - **Il ne rejoue jamais une commande expirée.** Une requête rejouée
//!   produirait une seconde réponse sans destinataire (spec §5.3).

mod verbes;

use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use windows::core::HRESULT;
use windows::Win32::Foundation::S_OK;

use crate::pont::entetes;
use crate::pont::enumeration::Session;
use crate::pont::erreurs::{hresult, Erreur};
use crate::pont::projfs::{ContexteProjFs, Etat, PERIODE_HYDRATATION};
use crate::pont::table::Attendue;
use crate::pont::transport::DuNavigateur;

/// Période du balayage des expirations.
///
/// ⚠️ **NON CALIBRÉE.** Posée, pas mesurée — c'est F4 qui donnera de quoi la
/// juger. Elle borne le retard avec lequel une commande échue est complétée :
/// une application attend donc au pire son budget plus cette période.
pub const PERIODE_BALAYAGE: Duration = Duration::from_millis(250);

/// La boucle du fil du pont. Rend quand le canal se ferme ou que le transport
/// s'arrête.
pub fn tourner(etat: Arc<Etat>, entrant: Receiver<DuNavigateur>) {
    let mut dernier_releve = Instant::now();
    loop {
        match entrant.recv_timeout(PERIODE_BALAYAGE) {
            Ok(DuNavigateur::CanalOuvert) => {
                tracing::info!("canal du pont ouvert : le navigateur peut servir les requêtes");
            }
            Ok(DuNavigateur::CanalFerme) => {
                tracing::warn!("canal du pont fermé : les commandes en vol sont abandonnées");
                tout_completer(&etat, Erreur::CanalFerme);
                return;
            }
            Ok(DuNavigateur::Reponse { correlation, trame }) => {
                traiter(&etat, correlation, &trame);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                tracing::info!("transport du pont arrêté : le fil du pont s'arrête");
                tout_completer(&etat, Erreur::CanalFerme);
                return;
            }
        }
        balayer(&etat);
        if dernier_releve.elapsed() >= PERIODE_HYDRATATION {
            etat.tracer_hydratation();
            dernier_releve = Instant::now();
        }
    }
}

/// Complète en erreur tout ce qui reste en vol. Appelée quand plus aucune
/// réponse ne peut arriver.
fn tout_completer(etat: &Etat, cause: Erreur) {
    let restantes = match etat.table.lock() {
        Ok(mut table) => table.vider(),
        Err(empoisonne) => empoisonne.into_inner().vider(),
    };
    for (commande, correlation) in restantes {
        oublier_contexte(etat, correlation);
        verbes::completer(etat, commande, HRESULT(hresult(cause)));
    }
}

/// Retire les commandes échues et les complète en délai dépassé.
///
/// **Rien n'est rejoué**, jamais : une commande expirée dont on rejouerait la
/// requête produirait une seconde réponse sans destinataire, et le navigateur
/// répondrait à une corrélation que la table ne connaît plus.
fn balayer(etat: &Etat) {
    let echues = match etat.table.lock() {
        Ok(mut table) => table.expirees(Instant::now()),
        Err(_) => return,
    };
    for (commande, correlation) in echues {
        tracing::warn!(commande, correlation, "commande expirée : le navigateur n'a pas répondu");
        oublier_contexte(etat, correlation);
        verbes::completer(etat, commande, HRESULT(hresult(Erreur::DelaiDepasse)));
    }
}

/// Retire le contexte ProjFS d'une corrélation, s'il en reste un.
fn oublier_contexte(etat: &Etat, correlation: u32) -> Option<ContexteProjFs> {
    etat.en_attente.lock().ok()?.remove(&correlation)
}

/// Traite une réponse du navigateur.
fn traiter(etat: &Etat, correlation: u32, octets: &[u8]) {
    let trame = match proto::fichiers::decoder(octets) {
        Ok(trame) => trame,
        Err(erreur) => {
            tracing::warn!(correlation, %erreur, "réponse du navigateur illisible, jetée");
            return;
        }
    };
    // ⚠️ **`resoudre` rend `None` pour une corrélation annulée, expirée ou
    // inconnue, et la réponse est alors JETÉE.** Appliquer une réponse dont la
    // commande ProjFS a déjà été complétée écrirait dans un tampon que le
    // système a repris.
    let Some((commande, attendue)) = etat.table.lock().ok().and_then(|mut t| t.resoudre(correlation))
    else {
        tracing::debug!(correlation, "réponse tardive ou inconnue : jetée");
        return;
    };
    let contexte = oublier_contexte(etat, correlation);

    if trame.type_message == proto::fichiers::TYPE_ECHEC {
        let cause = match serde_json::from_slice::<entetes::Echec>(trame.entete) {
            Ok(echec) => cause_de(echec.code),
            Err(erreur) => {
                tracing::warn!(correlation, %erreur, "échec au code illisible");
                Erreur::Inattendue
            }
        };
        tracing::debug!(commande, correlation, ?cause, "le navigateur refuse");
        return terminer(etat, commande, contexte, HRESULT(hresult(cause)));
    }

    let issue = appliquer(etat, correlation, commande, attendue, &trame, contexte.as_ref());
    match issue {
        Suite::Termine(resultat) => terminer(etat, commande, contexte, resultat),
        // La lecture continue : la commande est déjà réinscrite, et son
        // contexte est resté en place — surtout ne pas la compléter.
        Suite::Poursuit => {}
    }
}

/// Ce qu'il reste à faire après avoir appliqué une réponse.
enum Suite {
    Termine(HRESULT),
    Poursuit,
}

/// Complète, en tenant compte du fait qu'une énumération exige des paramètres
/// étendus.
fn terminer(etat: &Etat, commande: i32, contexte: Option<ContexteProjFs>, resultat: HRESULT) {
    match contexte {
        Some(ContexteProjFs::Enumeration { tampon, .. }) => {
            verbes::completer_enumeration(etat, commande, tampon.0, resultat)
        }
        _ => verbes::completer(etat, commande, resultat),
    }
}

fn appliquer(
    etat: &Etat,
    correlation: u32,
    commande: i32,
    attendue: Attendue,
    trame: &proto::fichiers::Trame<'_>,
    contexte: Option<&ContexteProjFs>,
) -> Suite {
    match (attendue, contexte) {
        (Attendue::Attributs { chemin }, Some(ContexteProjFs::Attributs { chemin_projfs })) => {
            let Ok(meta) = serde_json::from_slice::<entetes::Meta>(trame.entete) else {
                tracing::warn!(chemin, "en-tête Meta illisible");
                return Suite::Termine(HRESULT(hresult(Erreur::Inattendue)));
            };
            Suite::Termine(verbes::ecrire_marqueur(
                etat,
                chemin_projfs,
                meta.repertoire,
                meta.taille,
                meta.modifie,
            ))
        }
        // `QueryFileName` : le nom existe, et c'est TOUT ce que ProjFS attend.
        // Écrire un marqueur ici créerait un objet projeté pour un fichier que
        // personne n'ouvre. Un `TYPE_ECHEC` est traité en amont et rend
        // `ERROR_FILE_NOT_FOUND`, ce qui alimente le cache négatif.
        (Attendue::Attributs { .. }, Some(ContexteProjFs::Existence)) => Suite::Termine(S_OK),
        (
            Attendue::Lire { chemin, position, longueur },
            Some(ContexteProjFs::Lecture { flux, restants }),
        ) => {
            let Ok(entete) = serde_json::from_slice::<entetes::Donnees>(trame.entete) else {
                tracing::warn!(chemin, "en-tête Donnees illisible");
                return Suite::Termine(HRESULT(hresult(Erreur::Inattendue)));
            };
            // ⚠️ **L'en-tête et la charge doivent se corroborer.** Écrire dans
            // le tampon de ProjFS une quantité d'octets que l'émetteur ne
            // croyait pas envoyer est le genre de divergence qu'aucun contrôle
            // en aval ne rattrape : le fichier serait tronqué ou allongé, et
            // seul un condensat le dirait.
            if entete.longueur as usize != trame.charge.len()
                || entete.position != position
                || entete.longueur != longueur
            {
                tracing::warn!(
                    chemin, position, longueur,
                    recu_position = entete.position, recu_longueur = entete.longueur,
                    octets = trame.charge.len(),
                    "réponse Donnees incohérente avec la plage demandée : jetée"
                );
                return Suite::Termine(HRESULT(hresult(Erreur::Inattendue)));
            }
            let issue = verbes::ecrire_donnees(etat, flux.0, position, trame.charge);
            if issue.is_err() {
                return Suite::Termine(issue);
            }
            etat.octets_hydrates.fetch_add(trame.charge.len() as u64, Ordering::Relaxed);

            // ⚠️ **UN morceau en vol à la fois** : le morceau *n+1* n'est
            // demandé qu'après réception du morceau *n*. C'est le plus simple,
            // et c'est suffisant — le contrôle de flux par `bufferedAmount` et
            // `SEUIL_TAMPON` est un livrable de **F3** (spec §8), pas de F1.
            // L'implémenter à moitié ici serait pire que de ne pas
            // l'implémenter.
            let mut restants = restants.clone();
            match verbes::prochain_morceau(&mut restants) {
                Some(morceau) => {
                    etat.demander_lecture(commande, &chemin, morceau, flux.0, restants);
                    Suite::Poursuit
                }
                None => {
                    etat.entrees_hydratees.fetch_add(1, Ordering::Relaxed);
                    tracing::debug!(chemin, correlation, "lecture complète");
                    Suite::Termine(S_OK)
                }
            }
        }
        (
            Attendue::Lister { chemin, enumeration },
            Some(ContexteProjFs::Enumeration { tampon, expression, .. }),
        ) => {
            let Ok(entete) = serde_json::from_slice::<entetes::Entrees>(trame.entete) else {
                tracing::warn!(chemin, "en-tête Entrees illisible");
                return Suite::Termine(HRESULT(hresult(Erreur::Inattendue)));
            };
            let entrees = crate::pont::enumeration::preparer(
                verbes::entrees_depuis(entete.entrees),
                expression.as_deref(),
                |nom, motif| etat.apparier(nom, motif),
                |a, b| etat.comparer(a, b),
            );
            let mut sessions = match etat.sessions.lock() {
                Ok(sessions) => sessions,
                Err(_) => return Suite::Termine(HRESULT(hresult(Erreur::Inattendue))),
            };
            let session = sessions.entry(enumeration).or_insert_with(Session::nouvelle);
            session.poser(entrees);
            Suite::Termine(verbes::remplir(etat, session, tampon.0))
        }
        // Une réponse dont le type ne correspond pas à ce que la commande
        // attendait. Elle n'est pas appliquée « au mieux » : le navigateur et
        // le pont divergent, et deviner ferait écrire n'importe quoi dans le
        // tampon de ProjFS.
        (attendue, contexte) => {
            tracing::warn!(
                correlation,
                type_message = trame.type_message,
                ?attendue,
                contexte_present = contexte.is_some(),
                "réponse d'un type qui ne correspond pas à la commande : jetée"
            );
            Suite::Termine(HRESULT(hresult(Erreur::Inattendue)))
        }
    }
}

/// Le code d'échec du protocole → la cause locale.
///
/// `match` **exhaustif** : un code neuf du protocole ne peut pas tomber dans un
/// bras fourre-tout et hériter en silence de la cause d'un autre — c'est le
/// défaut que `pont::erreurs` existe pour ne pas rejouer.
fn cause_de(code: proto::fichiers::CodeEchec) -> Erreur {
    use proto::fichiers::CodeEchec;
    match code {
        CodeEchec::Introuvable => Erreur::Introuvable,
        CodeEchec::CheminIntrouvable => Erreur::CheminIntrouvable,
        CodeEchec::AccesRefuse => Erreur::AccesRefuse,
        CodeEchec::ProtegeEnEcriture => Erreur::ProtegeEnEcriture,
        CodeEchec::NonSupporte => Erreur::NonSupporte,
        // Une plage plus grande que ce que le navigateur peut rendre. Le pont
        // découpe déjà en `TAILLE_TRAME_MAX` ; recevoir ce code signale un
        // désaccord de constante entre les deux bouts, pas une condition
        // d'exécution.
        CodeEchec::TropGrand => Erreur::Inattendue,
        CodeEchec::Interne => Erreur::Inattendue,
    }
}

/// Remplit un tampon d'entrées depuis une session déjà chargée.
///
/// Exposée parce que le rappel `GetDirectoryEnumeration` l'emprunte
/// **directement** quand la session est déjà chargée : c'est le cas nominal
/// après le premier tour, il ne consulte pas le navigateur, et repasser par le
/// fil du pont ferait un aller-retour de fil pour une liste déjà en mémoire.
///
/// ⚠️ **C'est le seul travail qu'un rappel fait lui-même**, et il est borné :
/// une copie de noms dans un tampon que ProjFS a fourni, sous le verrou des
/// sessions, **sans aucune E/S**. La discipline de fil interdit l'attente, pas
/// le calcul.
pub fn remplir_session(
    etat: &Etat,
    session: &mut Session,
    tampon: windows::Win32::Storage::ProjectedFileSystem::PRJ_DIR_ENTRY_BUFFER_HANDLE,
) -> HRESULT {
    verbes::remplir(etat, session, tampon)
}
