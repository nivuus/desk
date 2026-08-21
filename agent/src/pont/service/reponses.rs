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
            // 🔴 **LE SUBSTITUT EST CRÉÉ SOUS LE NOM **STOCKÉ**, jamais sous
            // celui que l'application a tapé** — c'est la conséquence ① du
            // canonicaliseur de casse de F3.
            //
            // ⚠️ **On ne reconvertit le chemin QUE s'il y a quelque chose à
            // changer.** F1 s'était donné la propriété de ne jamais toucher les
            // octets que ProjFS a livrés — « un aller-retour où une casse ou un
            // séparateur pourrait se perdre » —, et
            // `chemins::avec_dernier_composant` rend `None` quand le nom
            // canonique est déjà celui du chemin.
            let projfs_texte = String::from_utf16_lossy(
                chemin_projfs.strip_suffix(&[0u16]).unwrap_or(chemin_projfs),
            );
            let canonique = crate::pont::chemins::avec_dernier_composant(&projfs_texte, &meta.nom);
            let Some(neuf) = canonique else {
                return Suite::Termine(verbes::ecrire_marqueur(
                    etat,
                    chemin_projfs,
                    meta.repertoire,
                    meta.taille,
                    meta.modifie,
                ));
            };
            tracing::debug!(
                demande = %projfs_texte,
                stocke = %neuf,
                "nom canonique : le substitut prend le nom du poste local"
            );
            let neuf_utf16: Vec<u16> = neuf.encode_utf16().chain(std::iter::once(0)).collect();
            let issue = verbes::ecrire_marqueur(
                etat,
                &neuf_utf16,
                meta.repertoire,
                meta.taille,
                meta.modifie,
            );
            if issue.is_ok() {
                return Suite::Termine(issue);
            }
            // 🔴 **LE REPLI, ET IL EST DÉCLARÉ.** Que ProjFS accepte un
            // substitut dont le nom diffère de celui demandé est la façon
            // documentée de corriger une casse — **mais cela n'a jamais été
            // MESURÉ sur cette VM**, et un refus rendrait le fichier
            // inouvrable alors qu'il s'ouvre aujourd'hui. On retente donc sous
            // le nom demandé plutôt que d'échouer, et **le journal dit
            // laquelle des deux voies a servi**.
            tracing::warn!(
                demande = %projfs_texte,
                stocke = %neuf,
                %issue,
                "PrjWritePlaceholderInfo a refuse le nom canonique : repli sur le nom demande"
            );
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
            Some(ContexteProjFs::Lecture { flux, fenetre }),
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
            // 🔴 **L'ORDRE EST VÉRIFIÉ AVANT L'ÉCRITURE, JAMAIS APRÈS.** Une
            // réponse hors d'ordre écrite puis dénoncée aurait déjà corrompu le
            // fichier, et **seul un condensat SHA-256 le dirait**.
            let mut garde = match fenetre.lock() {
                Ok(g) => g,
                Err(empoisonne) => empoisonne.into_inner(),
            };
            if let Err(hors) = garde.recu(position) {
                tracing::warn!(
                    chemin,
                    recue = hors.recue,
                    attendue = hors.attendue,
                    "reponse de lecture HORS D'ORDRE : jetee, RIEN n'est ecrit"
                );
                return Suite::Termine(HRESULT(etat.compteurs.rendre(Erreur::Inattendue)));
            }
            let issue = verbes::ecrire_donnees(etat, flux.0, position, trame.charge);
            if issue.is_err() {
                return Suite::Termine(issue);
            }
            etat.octets_hydrates.fetch_add(trame.charge.len() as u64, Ordering::Relaxed);

            // ✅ **LA FENÊTRE DE F3 REMPLACE « UN MORCEAU EN VOL À LA
            // FOIS ».** *(Ces lignes disaient : « UN morceau en vol à la fois
            // […] le contrôle de flux par `bufferedAmount` et `SEUIL_TAMPON`
            // est un livrable de F3, pas de F1. L'implémenter à moitié ici
            // serait pire que de ne pas l'implémenter. » F3 est arrivé, et il
            // n'en a PAS implémenté la moitié : la fenêtre du pont
            // (`pont::lecture`) ET la contre-pression du navigateur
            // (`client/src/fichiers/flux.ts`) sont livrées ensemble — avec un
            // seul morceau en vol, la règle de la spec §7.3 ne pourrait JAMAIS
            // mordre.)*
            //
            // 🔴 **L'INVARIANT D'ORDRE EST VÉRIFIÉ, JAMAIS CRU.** Le canal est
            // `ordered` et les morceaux sont demandés en positions
            // croissantes ; `Fenetre::recu` **dénonce** néanmoins une réponse
            // hors d'ordre au lieu de l'appliquer. Écrire une plage au mauvais
            // rang produirait un fichier dont **seul un condensat SHA-256**
            // dirait qu'il est faux — celui que F1 n'a JAMAIS établi.
            let lot = garde.a_demander();
            let terminee = garde.terminee();
            let en_vol_max = garde.en_vol_max();
            drop(garde);
            if terminee {
                etat.entrees_hydratees.fetch_add(1, Ordering::Relaxed);
                tracing::debug!(chemin, correlation, en_vol_max, "lecture complète");
                return Suite::Termine(S_OK);
            }
            // Une lecture porte TOUJOURS une commande ProjFS : c'est un rappel
            // `GetFileData` qui l'a inscrite.
            let Some(commande) = commande else {
                tracing::warn!(chemin, "lecture sans commande ProjFS : impossible");
                return Suite::Termine(HRESULT(etat.compteurs.rendre(Erreur::Inattendue)));
            };
            for morceau in lot {
                etat.demander_lecture(
                    commande,
                    &chemin,
                    morceau,
                    flux.0,
                    std::sync::Arc::clone(fenetre),
                );
            }
            // ⚠️ **`Poursuit` MÊME QUAND LE LOT EST VIDE** : d'autres
            // corrélations de la MÊME commande sont encore en vol, et
            // compléter ici les laisserait répondre à un rappel achevé.
            Suite::Poursuit
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
        // 🔴 **LA MUTATION (F3).** Elle ne complète AUCUN rappel — `command_id`
        // est `None` — et ne fait que relayer l'acquittement au fil, qui décide
        // s'il pousse la suivante.
        //
        // ⚠️ **Le contexte ProjFS est `None` ici, et ce n'est pas une anomalie**
        // : une mutation n'a ni tampon d'énumération, ni flux de données. Elle
        // naît d'une notification POST, qui a déjà rendu la main à
        // l'application.
        (Attendue::Muter { chemin, renommage }, None) => {
            if trame.type_message != proto::fichiers::TYPE_FAIT {
                tracing::warn!(
                    chemin, correlation, renommage, type_message = trame.type_message,
                    "reponse d'un type inattendu a une mutation : jetee"
                );
                return Suite::Termine(HRESULT(etat.compteurs.rendre(Erreur::Inattendue)));
            }
            tracing::debug!(chemin, correlation, renommage, "mutation acquittee");
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