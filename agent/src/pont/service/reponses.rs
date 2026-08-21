//! **Ce qu'on fait d'une réponse du navigateur** : l'appariement de ce qu'on
//! attendait avec ce qui est arrivé, et l'écriture qui en découle.
//!
//! # Pourquoi cette extraction, et POURQUOI ELLE ARRIVE UNE ADDITION TROP TARD
//!
//! `service.rs` valait **436** lignes quand F3 l'a ouvert — et non les 325 que
//! le §2.2 du plan annonçait, F2 l'ayant fait grossir entre-temps. Le
//! recensement des codes de la tâche 5 l'a porté à **510** : **le plafond de
//! 500 a été FRANCHI**, et il est rattrapé par cette extraction — **jamais par
//! une compression**, que `CLAUDE.md` interdit nommément et que D9 a payée deux
//! fois avant de devoir extraire quand même.
//!
//! ⚠️ **Le geste juste aurait été d'extraire AVANT d'ajouter**, comme D9 l'a
//! inventé (`capteur/serveur/instances.rs`) et comme D10 l'a joué trois fois.
//! Ce n'est pas ce qui s'est passé : le plan budgétait une marge de 175 qui
//! n'existait plus, et le franchissement a été constaté par la commande après
//! l'addition. **Déclaré plutôt que dissimulé**, et les tâches 8 et 13, qui
//! ajoutent encore à ce fichier, disposent maintenant de la marge.
//!
//! # La ligne de partage
//!
//! [`super`] porte **la boucle** — recevoir, balayer, expirer, recenser,
//! compléter. Ce module porte **l'appariement** : `(ce qu'on attendait, le
//! contexte ProjFS retenu)` contre `(le type reçu, l'en-tête, la charge)`. Les
//! deux responsabilités se relisent séparément, et c'est la seule raison qui
//! vaille de scinder un fichier.
//!
//! ⚠️ **AUCUNE LIGNE DE CORPS N'EST MODIFIÉE** par l'extraction elle-même : la
//! transposition est VERBATIM, et le contrôle est une comparaison texte à
//! texte versée au journal de la tâche 5. Ce que la tâche 13 y ajoutera vient
//! dans un commit séparé, pour que la revue puisse comparer l'un et l'autre.

use std::sync::atomic::Ordering;

use windows::core::HRESULT;
use windows::Win32::Foundation::S_OK;

use proto::fichiers::entetes;
use crate::pont::ecriture::fil::Ordre;
use crate::pont::enumeration::Session;
use crate::pont::erreurs::Erreur;
use crate::pont::projfs::{ContexteProjFs, Etat};
use crate::pont::table::Attendue;

use super::verbes;

/// Ce qu'il reste à faire après avoir appliqué une réponse.
pub(super) enum Suite {
    Termine(HRESULT),
    Poursuit,
}

/// Complète, en tenant compte du fait qu'une énumération exige des paramètres
/// étendus.
pub(super) fn terminer(
    etat: &Etat,
    commande: Option<i32>,
    contexte: Option<ContexteProjFs>,
    resultat: HRESULT,
) {
    match contexte {
        Some(ContexteProjFs::Enumeration { tampon, .. }) => match commande {
            Some(commande) => verbes::completer_enumeration(etat, commande, tampon.0, resultat),
            // Un contexte d'énumération sans commande n'existe pas ; le dire
            // plutôt que de l'ignorer.
            None => tracing::warn!("contexte d'énumération sans commande ProjFS : ignoré"),
        },
        _ => verbes::completer(etat, commande, resultat),
    }
}

pub(super) fn appliquer(
    etat: &Etat,
    correlation: u32,
    commande: Option<i32>,
    attendue: Attendue,
    trame: &proto::fichiers::Trame<'_>,
    contexte: Option<&ContexteProjFs>,
) -> Suite {
    match (attendue, contexte) {
        (Attendue::Attributs { chemin }, Some(ContexteProjFs::Attributs { chemin_projfs })) => {
            let Ok(meta) = serde_json::from_slice::<entetes::Meta>(trame.entete) else {
                tracing::warn!(chemin, "en-tête Meta illisible");
                return Suite::Termine(HRESULT(etat.compteurs.rendre(Erreur::Inattendue)));
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
                return Suite::Termine(HRESULT(etat.compteurs.rendre(Erreur::Inattendue)));
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
                return Suite::Termine(HRESULT(etat.compteurs.rendre(Erreur::Inattendue)));
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
                    // Une lecture porte TOUJOURS une commande ProjFS : c'est un
                    // rappel `GetFileData` qui l'a inscrite.
                    let Some(commande) = commande else {
                        tracing::warn!(chemin, "lecture sans commande ProjFS : impossible");
                        return Suite::Termine(HRESULT(etat.compteurs.rendre(Erreur::Inattendue)));
                    };
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
                return Suite::Termine(HRESULT(etat.compteurs.rendre(Erreur::Inattendue)));
            };
            let entrees = crate::pont::enumeration::preparer(
                verbes::entrees_depuis(entete.entrees),
                expression.as_deref(),
                |nom, motif| etat.apparier(nom, motif),
                |a, b| etat.comparer(a, b),
            );
            let mut sessions = match etat.sessions.lock() {
                Ok(sessions) => sessions,
                Err(_) => return Suite::Termine(HRESULT(etat.compteurs.rendre(Erreur::Inattendue))),
            };
            let session = sessions.entry(enumeration).or_insert_with(Session::nouvelle);
            session.poser(entrees);
            Suite::Termine(verbes::remplir(etat, session, tampon.0))
        }
        // 🔴 **LES DEUX BRAS DE L'ÉCRITURE.** Ils ne complètent AUCUN rappel —
        // `command_id` est `None` — et ne font que relayer l'acquittement au
        // fil d'écriture, qui décide s'il pousse le morceau suivant ou retire
        // l'entrée du journal.
        //
        // ⚠️ **Le contexte ProjFS est `None` ici, et ce n'est pas une anomalie**
        // : une écriture n'a ni tampon d'énumération, ni flux de données.
        (Attendue::Ecrire { chemin, dernier }, None) => {
            if trame.type_message != proto::fichiers::TYPE_FAIT {
                tracing::warn!(
                    chemin, correlation, type_message = trame.type_message,
                    "réponse d'un type inattendu à une écriture : jetée"
                );
                return Suite::Termine(HRESULT(etat.compteurs.rendre(Erreur::Inattendue)));
            }
            tracing::debug!(chemin, correlation, dernier, "morceau d'écriture acquitté");
            let _ = etat.vers_ecriture.send(Ordre::Fait { correlation });
            Suite::Termine(S_OK)
        }
        (Attendue::Creer { chemin }, None) => {
            if trame.type_message != proto::fichiers::TYPE_FAIT {
                tracing::warn!(
                    chemin, correlation, type_message = trame.type_message,
                    "réponse d'un type inattendu à une création : jetée"
                );
                return Suite::Termine(HRESULT(etat.compteurs.rendre(Erreur::Inattendue)));
            }
            tracing::debug!(chemin, correlation, "création acquittée");
            let _ = etat.vers_ecriture.send(Ordre::Fait { correlation });
            Suite::Termine(S_OK)
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
            Suite::Termine(HRESULT(etat.compteurs.rendre(Erreur::Inattendue)))
        }
    }
}