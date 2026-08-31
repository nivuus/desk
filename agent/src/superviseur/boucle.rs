//! La boucle du superviseur : elle consomme les événements, fait avancer la
//! table, et exécute les effets que celle-ci rend.
//!
//! Aucune décision ici — la table décide, cette boucle agit. C'est ce qui
//! rend les règles éprouvables sans Windows, et ce fichier lisible.

#![cfg(windows)]

use anyhow::Result;

use super::enfants::{Consigne, Enfants};
use super::hook;
use super::lanceur::LanceurDeProcessus;
use super::designation;
use super::placement;
use super::reprise;
use super::protocole::{DepuisLaShell, VersLaShell};
use super::table::{Effet, IdSession, Table};
use crate::capture::{enumerer_sorties_silencieux, SortieDxgi};
// `relever_topologie` plutôt qu'`enumerer_sorties` sur le chemin de création :
// elle journalise la topologie sortie par sortie, et c'est ce relevé qui rend
// diagnosticable un appariement qui échoue. Le contrôle périodique de
// placement, lui, emploie `enumerer_sorties_silencieux` — il court chaque
// seconde et ne doit rien journaliser.
//
// **Correctif I2 de la revue finale** : cette dernière phrase était fausse.
// `enumerer_sorties` porte un `tracing::info!` inconditionnel par adaptateur
// dépourvu de sortie, soit deux lignes par seconde indéfiniment sur cette VM,
// écrites sur un partage CIFS. La variante silencieuse existe pour ce seul
// appelant ; toute nouvelle boucle périodique doit l'employer aussi.
//
// **Nuance apportée à la tâche 7** : le chemin de création comporte
// maintenant une troisième étape, la scrutation d'`attendre_une_sortie_neuve`
// — et ELLE emploie `enumerer_sorties_silencieux`, pas `relever_topologie`,
// bien qu'elle reste sur le chemin de création. Ce n'est pas une entorse à la
// règle ci-dessus : cette étape tourne à 10 Hz, jusqu'à 5 s, et
// `relever_topologie` journalisant une ligne par sortie à CHAQUE appel, ce
// serait le même défaut que celui que le correctif I2 a corrigé, rejoué à une
// cadence pire. Le relevé nommé et journalisé reste fait une fois avant la
// création, et une fois de plus si l'attente expire (voir la doc
// d'`attendre_une_sortie_neuve`) — jamais à chaque tour de la scrutation.
use crate::diagnostics::multifenetre::montee::{noms_attaches, relever_topologie};
use crate::moniteurs_virtuels::{config_affichage, pilote::PiloteParIoctl, Sorties};

/// Nombre maximal de fenêtres servies simultanément.
///
/// **10, soit le vivier de sorties virtuelles du pilote** (mesure ① du
/// 31 juillet 2026 : refus à la 11ᵉ création, `ERROR_TOO_MANY_NAMES`). Ce
/// n'est plus le plafond d'ENCODEURS, et c'est le changement de D5 : jusqu'ici
/// les deux se confondaient à 8, faute de pouvoir ouvrir plus de fenêtres qu'on
/// ne pouvait en encoder. Le vivier (`capteur::vivier`) les sépare — au plus
/// `vivier::PLAFOND_EVEIL` (8) fenêtres sont éveillées à la fois, les autres
/// dorment en gardant leur sortie virtuelle et leur session.
///
/// Les deux plafonds ne viennent donc plus de la même couche : celui-ci du
/// **pilote de sorties virtuelles**, `PLAFOND_EVEIL` du **matériel
/// d'encodage**. Les faire suivre l'un l'autre serait une erreur.
///
/// ⚠️ **Valeur mesurée sur cette VM, non prouvée être une borne du système** —
/// et la cause du refus à la 11ᵉ création n'est pas isolée (on ignore même si le
/// vivier de 10 est global au pilote ou par client : Apollo pingue le même).
const CAPACITE: usize = 10;

/// Cadence du battement du chien de garde du pilote. Le pilote retire les
/// sorties d'un client qui cesse de pinguer ; l'unité de son délai n'est PAS
/// connue (aucune n'est exclue, pas même la seconde), d'où un battement
/// franchement plus rapide que toute unité plausible.
const PERIODE_PING: std::time::Duration = std::time::Duration::from_millis(500);

/// Cadence du contrôle « chaque fenêtre est-elle encore sur sa sortie ».
/// Une seconde de retard sur un déplacement est imperceptible ; en revanche
/// ce contrôle énumère les sorties DXGI, ce qui n'est pas gratuit — il ne
/// doit pas courir à chaque tour de boucle.
const PERIODE_PLACEMENT: std::time::Duration = std::time::Duration::from_secs(1);

/// Temps maximal laissé à Windows pour rattacher une sortie fraîchement créée.
///
/// **Une borne, pas une durée d'attente.** La version précédente dormait 1500 ms
/// plats, et la recette D1 a montré que ce n'était pas toujours assez : la
/// sortie n'était pas encore dans la topologie quand on l'y cherchait, et la
/// fenêtre ne s'ouvrait jamais. On attend désormais le FAIT — qu'une sortie
/// neuve apparaisse — et cette constante ne fait qu'empêcher d'attendre
/// indéfiniment.
const LIMITE_RATTACHEMENT: std::time::Duration = std::time::Duration::from_secs(5);

/// Pas de scrutation plus serrée : chaque tour énumère toutes les sorties DXGI,
/// ce qui n'est pas gratuit.
const PAS_RATTACHEMENT: std::time::Duration = std::time::Duration::from_millis(100);

pub fn tourner(
    pilote: &PiloteParIoctl,
    lanceur: &LanceurDeProcessus,
    rx_hook: std::sync::mpsc::Receiver<hook::EvenementFenetre>,
    rx_shell: std::sync::mpsc::Receiver<DepuisLaShell>,
    envoyer: impl Fn(&VersLaShell),
    // Le préfixe de la VM, délivré par la plateforme (sous-bloc P3). Vide
    // quand aucun enrôlement n'a eu lieu — les sessions gardent alors
    // exactement le nom qu'elles avaient avant P3.
    prefixe: String,
) -> Result<()> {
    // Forcé ICI, et non au premier appariement : la trace de désarmement doit
    // sortir AVANT la première fenêtre, sinon une recette courte se termine
    // sans elle. Leçon payée par `PONT_MESURE` au sous-bloc F4.
    let _ = designation::armee();

    let mut sorties = Sorties::nouvelles(pilote);
    let mut enfants = Enfants::nouveaux(lanceur);
    let mut table = Table::avec_prefixe(CAPACITE, prefixe);

    // Le capteur, avant la moindre fenêtre — `surveillance_capteur::EtatCapteur`.
    let mut etat_capteur = surveillance_capteur::EtatCapteur::demarrer(lanceur)?;
    // Le pont fichiers, juste après — et son démarrage N'EST PAS FATAL, à la
    // différence de celui du capteur : pas de `?` ici, et ce n'est pas un
    // oubli. Le cadrage §4 principe 4 exige qu'une panne du côté fichiers ne
    // touche jamais le flux vidéo ; `EtatPont::demarrer` ne rend donc aucun
    // `Result`, et retente indéfiniment depuis `surveiller`.
    let mut etat_pont = surveillance_pont::EtatPont::demarrer(lanceur);
    // Sorties DXGI déjà attribuées, pour que deux fenêtres au même viewport ne
    // se voient pas donner la même. La table porte déjà la correspondance
    // session -> sortie ; ceci n'est que l'ensemble des sorties occupées, par
    // leur nom DXGI (stable), et non plus par un couple d'index (positionnel).
    let mut prises: Vec<String> = Vec::new();

    // Les fenêtres déjà ouvertes : le hook ne rapporte que les changements.
    let mut effets = recenser_les_fenetres_existantes(&mut table);

    let mut dernier_ping = std::time::Instant::now();
    let mut dernier_controle_placement = std::time::Instant::now();
    loop {
        // 1. Exécuter les effets en attente.
        let a_faire = std::mem::take(&mut effets);
        for effet in a_faire {
            match effet {
                Effet::AnnoncerOuverture { session, titre } => {
                    envoyer(&VersLaShell::FenetreOuverte { session: session.0.clone(), titre });
                }
                Effet::CreerSortie { session, titre, largeur, hauteur } => {
                    effets.extend(creer_sortie(
                        pilote,
                        &mut sorties,
                        &mut table,
                        &mut prises,
                        &envoyer,
                        Demande { session, titre, largeur, hauteur },
                    ));
                    // `creer_sortie` a battu le chien de garde pendant son
                    // attente de rattachement : ne pas le recompter en retard.
                    dernier_ping = std::time::Instant::now();
                }
                Effet::LancerEnfant { session, fenetre, nom_sortie, taille } => {
                    // Le chemin de réutilisation d'une sortie retenue ne passe
                    // pas par `creer_sortie`, donc la fenêtre n'a pas été
                    // reposée. Une seule énumération, sur ce seul bras.
                    let toutes = enumerer_sorties_silencieux().unwrap_or_default();
                    replacer_si_besoin(&table, &session, &toutes);
                    if let Err(erreur) = enfants.lancer(Consigne {
                        session: session.clone(),
                        fenetre: fenetre.0,
                        nom_sortie,
                        taille,
                    }) {
                        tracing::error!(session = %session.0, %erreur, "lancement de l'enfant échoué");
                        // Le contrat du trait `Lanceur` est atomique : `Err`
                        // signifie qu'aucun processus ne tourne. Rien à tuer
                        // donc ; la sortie, elle, est RETENUE par `enfant_mort`
                        // (§7.1 de D3), et rendue par un chemin d'abandon.
                        effets.extend(table.enfant_mort(&session));
                    }
                }
                Effet::TuerEnfant { session } => enfants.tuer(&session),
                Effet::DetruireSortie { sortie_pilote, nom_sortie } => {
                    // Rendue MAINTENANT, pas à l'arrêt du superviseur : le
                    // vivier du pilote se consomme à chaque ouverture de
                    // fenêtre, et une dizaine d'ouvertures-fermetures
                    // suffirait sinon à bloquer toute nouvelle fenêtre.
                    rendre_la_sortie(&mut sorties, &mut prises, sortie_pilote, nom_sortie);
                }
                Effet::SuivreLeViewport { session, largeur, hauteur } => {
                    suivre_le_viewport(&mut table, &session, largeur, hauteur);
                }
                Effet::AnnoncerFermeture { session } => {
                    envoyer(&VersLaShell::FenetreFermee { session: session.0 });
                }
                Effet::AnnoncerRefus { titre, motif } => {
                    envoyer(&VersLaShell::Refus { titre, motif });
                }
            }
        }

        // 2. Battre le chien de garde du pilote.
        if dernier_ping.elapsed() >= PERIODE_PING {
            if let Err(erreur) = pilote.pinguer() {
                tracing::warn!(%erreur, "ping du chien de garde du pilote échoué");
            }
            dernier_ping = std::time::Instant::now();
        }

        // 3. Événements de fenêtres.
        while let Ok(evenement) = rx_hook.try_recv() {
            effets.extend(match evenement {
                hook::EvenementFenetre::Apparue { fenetre, titre } => {
                    table.fenetre_apparue(fenetre, titre)
                }
                hook::EvenementFenetre::Disparue { fenetre } => table.fenetre_disparue(fenetre),
            });
        }

        // 4. Messages de la shell.
        //
        // Le champ `session` vient du navigateur et n'est pas fiable : le
        // signaling relaie les messages de contrôle entiers, un pair peut y
        // écrire ce qu'il veut. C'est `viewport_recu` qui garde — elle ignore
        // une session inconnue, et une session qui n'attend plus son viewport.
        while let Ok(message) = rx_shell.try_recv() {
            match message {
                DepuisLaShell::Viewport { session, largeur, hauteur } => {
                    effets.extend(table.viewport_recu(&IdSession(session), largeur, hauteur));
                }
                // 🔴 UNE PAGE-SHELL VIENT DE REJOINDRE LA SESSION DE
                // CONTRÔLE. Tout ce que le superviseur a annoncé avant cet
                // instant est PERDU — le relais laisse tomber sans une trace
                // ce qu'il n'a personne à qui remettre — et c'est le défaut
                // mesuré en production le 30 août 2026 : l'agent tournait
                // depuis plusieurs minutes, ses trois fenêtres avaient été
                // annoncées à t = 12 s puis refusées à t = 43 s, et
                // l'utilisateur, retenu par l'authentification du proxy,
                // n'a jamais rien vu.
                //
                // 🔴 L'ORDRE DES DEUX GESTES EST LA CORRECTION, PAS UN
                // DÉTAIL :
                //   ① redire les entrées ENCORE en attente
                //      (`reannoncer_les_attentes`), qui remet aussi leur
                //      compte à rebours à zéro — l'horloge des 30 s repart
                //      du moment où une shell est là, ce qui est ce que la
                //      constante prétend mesurer ;
                //   ② rejouer l'énumération de démarrage, dont
                //      `fenetre_apparue` est idempotente par `HWND` : elle
                //      ne rattrape donc que les fenêtres ABANDONNÉES entre
                //      temps, qui ne sont plus dans la table.
                // Inverser les deux annoncerait DEUX fois une entrée encore
                // en attente, et la page-shell rechargerait
                // (`window.open(url, "guac-<session>")` vise une fenêtre
                // NOMMÉE) la fenêtre qu'elle vient d'ouvrir.
                DepuisLaShell::PairPresent => {
                    tracing::info!(
                        "une page-shell a rejoint la session de contrôle : les fenêtres sont réannoncées"
                    );
                    effets.extend(table.reannoncer_les_attentes(std::time::Instant::now()));
                    effets.extend(recenser_les_fenetres_existantes(&mut table));
                }
            }
        }

        // 5. Enfants morts d'eux-mêmes.
        for session in enfants.morts() {
            effets.extend(table.enfant_mort(&session));
        }

        // 5bis. Le capteur, même tour que les enfants — `EtatCapteur::surveiller`.
        etat_capteur.surveiller(lanceur);

        // 5ter. Le pont fichiers, même tour — `EtatPont::surveiller`. Ne
        // touche ni à la table, ni aux enfants, ni aux sorties : une panne du
        // pont doit rester sans effet sur les sessions vidéo.
        etat_pont.surveiller(lanceur);

        // 6. Les fenêtres sont-elles encore sur leur sortie ?
        //
        // Une application peut se déplacer ou se retailler d'elle-même, et une
        // fenêtre qui déborde de sa sortie donne une capture tronquée sans que
        // rien ne le signale. Le contrôle est PÉRIODIQUE et non branché sur
        // `EVENT_OBJECT_LOCATIONCHANGE` : cet événement se déclenche à chaque
        // pixel de déplacement, sur toutes les fenêtres du bureau, et noierait
        // le canal du hook pour un besoin qui tolère très bien une seconde de
        // retard.
        if dernier_controle_placement.elapsed() >= PERIODE_PLACEMENT {
            dernier_controle_placement = std::time::Instant::now();
            controler_le_placement(&table);
            // 7. Fenêtres dont l'enfant est mort mais qui existent toujours
            // côté Windows : on les repropose plutôt que de les laisser
            // disparaître de la shell (voir `Etat::SansSession`).
            effets.extend(table.relancer_les_orphelines(std::time::Instant::now()));
        }

        if effets.is_empty() {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
}

/// Fait entrer dans la table toutes les fenêtres Windows déjà ouvertes.
///
/// Appelée à DEUX moments, et c'est ce qui lui vaut d'exister plutôt que
/// d'être recopiée : au démarrage du superviseur (le hook ne rapporte que
/// les CHANGEMENTS, donc rien de ce qui existait avant lui), et à l'arrivée
/// d'une page-shell, pour rattraper les fenêtres que le délai d'attente a
/// abandonnées entre temps.
///
/// ⚠️ **Elle ne dédouble rien** : `Table::fenetre_apparue` est idempotente
/// par `HWND` et rend un vecteur VIDE pour une fenêtre déjà connue, quel que
/// soit son état. C'est cette idempotence — posée pour une tout autre raison
/// (le recouvrement entre l'énumération et le hook) — qui rend le second
/// appel gratuit.
fn recenser_les_fenetres_existantes(table: &mut Table) -> Vec<Effet> {
    let mut effets = Vec::new();
    for (fenetre, titre) in hook::enumerer_existantes() {
        effets.extend(table.fenetre_apparue(fenetre, titre));
    }
    effets
}

/// Ce qu'une demande de sortie porte. Un `struct` plutôt que quatre
/// paramètres : le titre est venu s'ajouter (il est ce qu'un refus dit à
/// l'utilisateur) et la liste d'arguments passait le seuil du lisible.
struct Demande {
    session: IdSession,
    titre: String,
    largeur: u32,
    hauteur: u32,
}

// Contrôle périodique de placement (`controler_le_placement`,
// `replacer_si_besoin`) : extrait côté production, pour rester sous le
// plafond de 500 lignes du projet — la tâche 7 du sous-bloc D3 a fait
// franchir ce plafond à ce fichier. Extraire plutôt que compresser, même
// raison et même schéma que `superviseur/table/attribution.rs`.
mod placement_periodique;
use placement_periodique::{controler_le_placement, replacer_si_besoin, suivre_le_viewport};

// Lancement et surveillance du capteur (tâche 7 du sous-bloc D4) : extrait
// côté production, pour la même raison et le même schéma que
// `placement_periodique` ci-dessus. Nommé `surveillance_capteur` et non
// `capteur` — voir l'en-tête de ce fichier (I7).
mod surveillance_capteur;

// Lancement et surveillance du pont fichiers (tâche 10 du sous-bloc F1) :
// jumeau du module ci-dessus, extrait pour la même raison et le même schéma.
// Nommé `surveillance_pont` et non `pont` — `crate::pont` désigne le processus
// lui-même, et ce fichier fait `use super::*`.
mod surveillance_pont;

// Création d'une sortie virtuelle et restitution au pilote (tâche 1 du
// sous-bloc D10) : extrait côté production, pour la même raison et le même
// schéma que les deux modules ci-dessus, et avant l'addition qui l'aurait
// autrement fait franchir le plafond.
mod creation_sortie;
use creation_sortie::{creer_sortie, rendre_la_sortie};
