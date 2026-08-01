//! La boucle du superviseur : elle consomme les événements, fait avancer la
//! table, et exécute les effets que celle-ci rend.
//!
//! Aucune décision ici — la table décide, cette boucle agit. C'est ce qui
//! rend les règles éprouvables sans Windows, et ce fichier lisible.

#![cfg(windows)]

use anyhow::Result;

use super::enfants::{Consigne, Enfants, Lanceur};
use super::hook;
use super::placement;
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
use crate::moniteurs_virtuels::{pilote::PiloteParIoctl, Sorties};

/// Capacité retenue : le pilote refuse la 11ᵉ sortie (mesuré), et Apollo puise
/// au même vivier sans qu'on sache combien il en prend. Huit est la cible du
/// chantier, avec deux de marge assumée.
const CAPACITE: usize = 8;

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
    lanceur: &dyn Lanceur,
    rx_hook: std::sync::mpsc::Receiver<hook::EvenementFenetre>,
    rx_shell: std::sync::mpsc::Receiver<DepuisLaShell>,
    envoyer: impl Fn(&VersLaShell),
) -> Result<()> {
    let mut sorties = Sorties::nouvelles(pilote);
    let mut enfants = Enfants::nouveaux(lanceur);
    let mut table = Table::nouvelle(CAPACITE);
    // Sorties DXGI déjà attribuées, pour que deux fenêtres au même viewport ne
    // se voient pas donner la même. La table porte déjà la correspondance
    // session -> sortie ; ceci n'est que l'ensemble des sorties occupées, par
    // leur nom DXGI (stable), et non plus par un couple d'index (positionnel).
    let mut prises: Vec<String> = Vec::new();

    // Les fenêtres déjà ouvertes : le hook ne rapporte que les changements.
    let mut effets = Vec::new();
    for (fenetre, titre) in hook::enumerer_existantes() {
        effets.extend(table.fenetre_apparue(fenetre, titre));
    }

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
                Effet::LancerEnfant { session, fenetre, nom_sortie, audio } => {
                    if let Err(erreur) = enfants.lancer(Consigne {
                        session: session.clone(),
                        fenetre: fenetre.0,
                        nom_sortie,
                        audio,
                    }) {
                        tracing::error!(session = %session.0, %erreur, "lancement de l'enfant échoué");
                        // Le contrat du trait `Lanceur` est atomique : `Err`
                        // signifie qu'aucun processus ne tourne. Rien à tuer
                        // donc, mais la sortie, elle, existe — et
                        // `enfant_mort` est le seul chemin qui la rende.
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
            let DepuisLaShell::Viewport { session, largeur, hauteur } = message;
            effets.extend(table.viewport_recu(&IdSession(session), largeur, hauteur));
        }

        // 5. Enfants morts d'eux-mêmes.
        for session in enfants.morts() {
            effets.extend(table.enfant_mort(&session));
        }

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

/// Ce qu'une demande de sortie porte. Un `struct` plutôt que quatre
/// paramètres : le titre est venu s'ajouter (il est ce qu'un refus dit à
/// l'utilisateur) et la liste d'arguments passait le seuil du lisible.
struct Demande {
    session: IdSession,
    titre: String,
    largeur: u32,
    hauteur: u32,
}

/// Crée la sortie virtuelle d'une session, l'apparie à sa place DXGI, y pose
/// la fenêtre, et rend les effets à enchaîner.
///
/// Extrait de la boucle pour une raison de fond : **tout chemin d'échec sous
/// la création doit défaire la sortie**. Une sortie créée que la topologie
/// DXGI ne rend pas resterait sinon tenue jusqu'à l'arrêt du superviseur.
///
/// Chaque chemin d'échec appelle `table.enfant_mort`, qui sort l'entrée
/// d'`AttendLaSortie`. **Depuis la tâche 10, cet appel ne libère plus la
/// place dans la capacité** : l'entrée bascule en `Etat::SansSession` et le
/// contrôle périodique la relance, jusqu'à `RELANCES_MAX` fois (voir
/// `superviseur::table`). Conséquence à connaître : une fenêtre dont la
/// création de sortie échoue systématiquement fait donc envoyer jusqu'à
/// `RELANCES_MAX + 1` `VersLaShell::Refus` à la page-shell — un par tentative
/// avortée, plus l'abandon final — et non plus un seul comme avant cette
/// tâche.
fn creer_sortie(
    pilote: &PiloteParIoctl,
    sorties: &mut Sorties<'_>,
    table: &mut Table,
    prises: &mut Vec<String>,
    envoyer: &impl Fn(&VersLaShell),
    demande: Demande,
) -> Vec<Effet> {
    let Demande { session, titre, largeur, hauteur } = demande;

    // Relevé AVANT création, et c'est la pièce maîtresse de l'appariement.
    //
    // `sortie_par_dimensions` ne filtre que sur « attachée, aux bonnes
    // dimensions, pas déjà prise » : rien n'y exclut les sorties PRÉEXISTANTES.
    // Or le viewport annoncé par le navigateur peut parfaitement égaler la
    // résolution d'un moniteur physique — c'est même le cas banal en plein
    // écran. Sans ce relevé, la fenêtre serait posée sur l'écran RÉEL de la VM
    // et la sortie virtuelle qu'on vient de créer deviendrait orpheline. On
    // n'apparie donc que parmi les sorties APPARUES, et le dépôt a déjà écrit
    // la doctrine : comparer des ensembles de NOMS, jamais des nombres.
    let avant = match relever_topologie("avant création de sortie") {
        Ok(avant) => noms_attaches(&avant),
        Err(erreur) => {
            tracing::error!(session = %session.0, %erreur, "topologie DXGI illisible avant création");
            envoyer(&VersLaShell::Refus {
                titre: titre.clone(),
                motif: "topologie d'affichage illisible".into(),
            });
            // Aucune sortie n'a été créée : rien à rendre au pilote. Mais
            // l'entrée doit sortir d'`AttendLaSortie` — `enfant_mort` la
            // bascule en `SansSession` (sa place reste comptée, voir la doc
            // de cette fonction) plutôt que de la retirer : le contrôle
            // périodique la relancera.
            return table.enfant_mort(&session);
        }
    };

    let id_pilote = match sorties.creer(largeur, hauteur, 60) {
        Ok(id) => id,
        Err(erreur) => {
            tracing::error!(session = %session.0, %erreur, "création de sortie refusée");
            envoyer(&VersLaShell::Refus { titre, motif: format!("{erreur}") });
            return table.enfant_mort(&session);
        }
    };

    // Attend le FAIT — qu'une sortie neuve apparaisse dans la topologie DXGI —
    // plutôt qu'un délai plat, tout en continuant de battre le chien de garde
    // du pilote (voir la doc d'`attendre_une_sortie_neuve`).
    let apparues = attendre_une_sortie_neuve(pilote, &avant, LIMITE_RATTACHEMENT);

    let Some(cible) = placement::sortie_par_dimensions(&apparues, largeur, hauteur, prises) else {
        // Journaliser les CANDIDATS, pas seulement la demande. L'égalité de
        // dimensions est exacte à dessein, et `CLAUDE.md` documente une sortie
        // virtuelle déjà vue à un facteur DPI de 1,5 de ce qui était demandé :
        // si l'hôte applique une mise à l'échelle, AUCUNE fenêtre ne s'ouvrira
        // jamais, et un journal qui ne redirait que la demande laisserait ce
        // diagnostic entièrement à faire.
        tracing::error!(
            session = %session.0,
            demande = format!("{largeur}x{hauteur}"),
            apparues = ?apparues
                .iter()
                .map(|s| format!("{} {}x{}", s.nom_sortie, s.rect.width, s.rect.height))
                .collect::<Vec<_>>(),
            "sortie créée mais introuvable dans la topologie DXGI — elle est rendue au pilote"
        );
        rendre_sans_apparier(sorties, id_pilote);
        envoyer(&VersLaShell::Refus {
            titre,
            motif: "la sortie créée est introuvable dans la topologie d'affichage".into(),
        });
        return table.enfant_mort(&session);
    };

    let nom = cible.nom_sortie.clone();
    prises.push(nom.clone());
    // Les DEUX identifiants : celui du pilote pour la destruction, le nom
    // DXGI pour la capture. Aucune relation calculable entre eux.
    let suite = table.sortie_creee(&session, id_pilote, nom);

    // Une table qui n'a rien à dire de cette sortie ne la retient nulle part :
    // `id_pilote` ne serait plus connu de personne (ni de la table, ni d'un
    // effet à venir), une place perdue sur dix, et le nom resterait bloqué
    // dans `prises` à jamais. Le cas n'est pas atteignable avec l'ordonnancement
    // actuel de la boucle — mais cet ordonnancement n'est déclaré porteur nulle
    // part, et il suffira qu'une étape s'insère un jour.
    if suite.is_empty() {
        tracing::error!(
            session = %session.0, id_pilote,
            "la table n'attendait plus cette sortie — elle est rendue au pilote"
        );
        rendre_sans_apparier(sorties, id_pilote);
        prises.retain(|p| *p != cible.nom_sortie);
        return Vec::new();
    }

    // Poser la fenêtre dessus avant que l'enfant ne capture. On lit la fenêtre
    // dans les effets que la table VIENT de rendre, et non dans la file
    // globale : celle-ci peut porter le `LancerEnfant` d'une autre session,
    // et on placerait alors la mauvaise fenêtre.
    if let Some(Effet::LancerEnfant { fenetre, .. }) = suite.first() {
        let hwnd = windows::Win32::Foundation::HWND(fenetre.0 as *mut core::ffi::c_void);
        if let Err(erreur) = placement::poser(hwnd, &cible.rect) {
            tracing::warn!(session = %session.0, %erreur, "placement de la fenêtre échoué");
        }
    }
    suite
}

/// Rend au pilote une sortie qui n'a jamais été appariée à une session.
///
/// Rien à retirer de `prises` : par construction, aucun de ces chemins n'y a
/// inscrit quoi que ce soit — ou l'appelant s'en charge.
fn rendre_sans_apparier(sorties: &mut Sorties<'_>, id_pilote: u32) {
    if let Err(erreur) = sorties.detruire(id_pilote) {
        tracing::error!(
            id_pilote, %erreur,
            "sortie orpheline NON rendue — la garde la retentera à l'arrêt"
        );
    }
}

/// Attend qu'une sortie neuve apparaisse dans la topologie, sans cesser de
/// battre le chien de garde.
///
/// Le battement n'est pas un détail : le pilote retire les sorties d'un client
/// qui cesse de pinguer, **y compris celles qu'on vient de créer**, et l'étape
/// de ping de la boucle est hors du parcours des effets.
///
/// **`enumerer_sorties_silencieux`, jamais `relever_topologie`, DANS LA
/// SCRUTATION.** À 10 Hz, `relever_topologie` journaliserait une ligne par
/// sortie DXGI existante à chaque tour — le dépôt a déjà payé deux fois pour
/// une trace émise à la cadence d'une boucle (chantier TURN, correctif I2 de
/// D1). Le relevé nommé et journalisé reste fait une fois avant l'appel
/// (`creer_sortie`), et — depuis la relecture de cette fonction — une fois de
/// plus SEULEMENT si l'attente expire, juste avant de rendre le vecteur vide.
///
/// **Ce relevé d'expiration n'est pas cosmétique.** Sans lui, un échec ne
/// laisse au journal que le relevé d'AVANT création (qui ne peut par
/// construction pas montrer la sortie neuve) et le journal des « candidats »
/// de l'appelant, qui ne liste que les sorties déjà filtrées `attachee_au_
/// bureau && nouvelles` — vide par construction si la sortie n'a jamais été
/// attachée. Deux pannes distinctes se confondaient alors sous un même
/// journal : « la sortie est apparue mais Windows n'y a jamais rien composé »
/// (le refus que la sonde multi-fenêtres nomme déjà) contre « elle n'est
/// jamais apparue du tout ». Le relevé complet et nommé — toutes les sorties,
/// attachées et non attachées — tranche entre les deux, et ne coûte rien en
/// régime normal : il ne s'exécute que sur le chemin d'échec.
fn attendre_une_sortie_neuve(
    pilote: &PiloteParIoctl,
    avant: &[String],
    limite: std::time::Duration,
) -> Vec<SortieDxgi> {
    let echeance = std::time::Instant::now() + limite;
    loop {
        if let Err(erreur) = pilote.pinguer() {
            tracing::warn!(%erreur, "ping du chien de garde pendant l'attente de rattachement");
        }
        let toutes = enumerer_sorties_silencieux().unwrap_or_default();
        let apparues: Vec<_> = toutes
            .iter()
            .filter(|s| s.attachee_au_bureau && !avant.contains(&s.nom_sortie))
            .cloned()
            .collect();
        if !apparues.is_empty() {
            return apparues;
        }
        if std::time::Instant::now() >= echeance {
            tracing::error!(
                limite_ms = limite.as_millis() as u64,
                "aucune sortie neuve n'est apparue dans la limite"
            );
            // Relevé complet, nommé, UNE fois — sur ce seul chemin d'échec.
            // C'est ici, et seulement ici, que ce diagnostic vaut : voir la
            // doc de la fonction.
            if let Err(erreur) = relever_topologie("attente de rattachement expirée") {
                tracing::error!(%erreur, "topologie DXGI illisible au moment de l'expiration");
            }
            return Vec::new();
        }
        std::thread::sleep(PAS_RATTACHEMENT);
    }
}

/// Rend une sortie au pilote et libère sa place DXGI.
fn rendre_la_sortie(
    sorties: &mut Sorties<'_>,
    prises: &mut Vec<String>,
    sortie_pilote: u32,
    nom_sortie: String,
) {
    match sorties.detruire(sortie_pilote) {
        Ok(()) => {
            tracing::info!(sortie_pilote, "sortie virtuelle rendue au pilote");
            prises.retain(|p| *p != nom_sortie);
        }
        // La place DXGI reste RÉSERVÉE sur échec, et c'est le point de fond.
        //
        // Un refus de destruction signifie très probablement que la sortie
        // existe toujours — et qu'elle reste donc attachée au bureau. Libérer
        // sa place la rendrait à nouveau candidate : une fenêtre ultérieure de
        // mêmes dimensions pourrait s'y voir posée pendant que la table
        // retiendrait l'`id_pilote` de la sortie NEUVE, laquelle ne servirait
        // jamais et serait détruite à tort à la fermeture — l'ancienne restant
        // orpheline. Garder la place réservée coûte au pire une place DXGI
        // jusqu'à l'arrêt ; la libérer coûte une confusion d'identité.
        Err(erreur) => tracing::error!(
            sortie_pilote, %nom_sortie, %erreur,
            "sortie virtuelle NON rendue — la garde la retentera à l'arrêt, \
             et sa place DXGI reste réservée d'ici là"
        ),
    }
}

/// Remet sur sa sortie toute fenêtre qui en est partie.
fn controler_le_placement(table: &Table) {
    let toutes = enumerer_sorties_silencieux().unwrap_or_default();
    for session in table.sessions_vivantes() {
        let Some(nom) = table.nom_sortie_de(&session) else {
            continue;
        };
        let Some(cible) = toutes.iter().find(|s| s.nom_sortie == nom) else {
            continue;
        };
        let Some(fenetre) = table.fenetre_de(&session) else { continue };
        let hwnd = windows::Win32::Foundation::HWND(fenetre.0 as *mut core::ffi::c_void);
        let Ok(actuel) = placement::rectangle_de(hwnd) else { continue };
        if placement::doit_etre_replacee(&actuel, &cible.rect) {
            tracing::info!(
                session = %session.0,
                de = format!("{}x{}+{}+{}", actuel.width, actuel.height, actuel.x, actuel.y),
                vers = format!(
                    "{}x{}+{}+{}",
                    cible.rect.width, cible.rect.height, cible.rect.x, cible.rect.y
                ),
                "fenêtre sortie de sa sortie, replacement"
            );
            if let Err(erreur) = placement::poser(hwnd, &cible.rect) {
                tracing::warn!(session = %session.0, %erreur, "replacement échoué");
            }
        }
    }
}
