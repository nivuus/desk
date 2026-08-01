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
use crate::capture::enumerer_sorties;
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

/// Délai laissé à Windows pour rattacher une sortie fraîchement créée avant de
/// l'énumérer : elle n'apparaît pas instantanément dans la topologie DXGI.
const DELAI_RATTACHEMENT: std::time::Duration = std::time::Duration::from_millis(1500);

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
    // session -> sortie ; ceci n'est que l'ensemble des sorties occupées.
    let mut prises: Vec<(u32, u32)> = Vec::new();

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
                Effet::CreerSortie { session, largeur, hauteur } => {
                    effets.extend(creer_sortie(
                        pilote,
                        &mut sorties,
                        &mut table,
                        &mut prises,
                        &envoyer,
                        session,
                        largeur,
                        hauteur,
                    ));
                    // `creer_sortie` a battu le chien de garde pendant son
                    // attente de rattachement : ne pas le recompter en retard.
                    dernier_ping = std::time::Instant::now();
                }
                Effet::LancerEnfant {
                    session,
                    fenetre,
                    index_adaptateur,
                    index_sortie,
                    audio,
                } => {
                    if let Err(erreur) = enfants.lancer(Consigne {
                        session: session.clone(),
                        fenetre: fenetre.0,
                        index_adaptateur,
                        index_sortie,
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
                Effet::DetruireSortie { sortie_pilote, dxgi } => {
                    // Rendue MAINTENANT, pas à l'arrêt du superviseur : le
                    // vivier du pilote se consomme à chaque ouverture de
                    // fenêtre, et une dizaine d'ouvertures-fermetures
                    // suffirait sinon à bloquer toute nouvelle fenêtre.
                    rendre_la_sortie(&mut sorties, &mut prises, sortie_pilote, dxgi);
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
        }

        if effets.is_empty() {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
}

/// Crée la sortie virtuelle d'une session, l'apparie à sa place DXGI, y pose
/// la fenêtre, et rend les effets à enchaîner.
///
/// Extrait de la boucle pour une raison de fond : **tout chemin d'échec sous
/// la création doit défaire la sortie**. Une sortie créée que la topologie
/// DXGI ne rend pas resterait sinon tenue jusqu'à l'arrêt du superviseur, et
/// l'entrée de la table resterait éternellement en `AttendLaSortie` — une
/// fenêtre morte-vivante et une place perdue dans un vivier de dix.
fn creer_sortie(
    pilote: &PiloteParIoctl,
    sorties: &mut Sorties<'_>,
    table: &mut Table,
    prises: &mut Vec<(u32, u32)>,
    envoyer: &impl Fn(&VersLaShell),
    session: IdSession,
    largeur: u32,
    hauteur: u32,
) -> Vec<Effet> {
    let id_pilote = match sorties.creer(largeur, hauteur, 60) {
        Ok(id) => id,
        Err(erreur) => {
            tracing::error!(session = %session.0, %erreur, "création de sortie refusée");
            envoyer(&VersLaShell::Refus {
                titre: session.0.clone(),
                motif: format!("{erreur}"),
            });
            // La table garde son entrée en `AttendLaSortie` sans la sortie
            // qu'elle attend : la retirer, sinon la place reste comptée.
            return table.enfant_mort(&session);
        }
    };

    // Laisser Windows rattacher la sortie avant de l'énumérer — SANS cesser de
    // battre le chien de garde. Un `sleep` plat de 1,5 s serait le plus long
    // silence de tout le superviseur, et le délai du chien de garde vaut 3
    // dans une unité INCONNUE dont la seconde n'est pas exclue : un client qui
    // cesse de pinguer voit ses sorties retirées, y compris celle qu'on vient
    // de créer.
    let jusqua = std::time::Instant::now() + DELAI_RATTACHEMENT;
    while std::time::Instant::now() < jusqua {
        if let Err(erreur) = pilote.pinguer() {
            tracing::warn!(%erreur, "ping du chien de garde échoué pendant le rattachement");
        }
        std::thread::sleep(PERIODE_PING.min(jusqua.saturating_duration_since(std::time::Instant::now())));
    }

    // Une énumération qui échoue n'est PAS fatale au superviseur : les autres
    // fenêtres tournent, et rien ne dit que le prochain essai échouera aussi.
    let toutes = match enumerer_sorties() {
        Ok(toutes) => toutes,
        Err(erreur) => {
            tracing::error!(session = %session.0, %erreur, "topologie DXGI illisible");
            Vec::new()
        }
    };

    let Some(cible) = placement::sortie_par_dimensions(&toutes, largeur, hauteur, prises) else {
        tracing::error!(
            session = %session.0, largeur, hauteur,
            "sortie créée mais introuvable dans la topologie DXGI — elle est rendue au pilote"
        );
        // La rendre TOUT DE SUITE : personne ne la réclamera jamais, et le
        // vivier n'en compte que dix.
        if let Err(erreur) = sorties.detruire(id_pilote) {
            tracing::error!(id_pilote, %erreur, "sortie orpheline NON rendue — la garde la retentera");
        }
        return table.enfant_mort(&session);
    };

    prises.push((cible.index_adaptateur, cible.index_sortie));
    // Les DEUX identifiants : celui du pilote pour la destruction, la
    // position DXGI pour la capture. Aucune relation calculable entre eux.
    let suite = table.sortie_creee(
        &session,
        id_pilote,
        (cible.index_adaptateur, cible.index_sortie),
    );

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

/// Rend une sortie au pilote et libère sa place DXGI.
fn rendre_la_sortie(
    sorties: &mut Sorties<'_>,
    prises: &mut Vec<(u32, u32)>,
    sortie_pilote: u32,
    dxgi: (u32, u32),
) {
    match sorties.detruire(sortie_pilote) {
        Ok(()) => tracing::info!(sortie_pilote, "sortie virtuelle rendue au pilote"),
        Err(erreur) => tracing::error!(
            sortie_pilote, %erreur,
            "sortie virtuelle NON rendue — la garde la retentera à l'arrêt"
        ),
    }
    // La place DXGI se libère dans les deux cas : si le pilote a refusé, la
    // sortie ne sera de toute façon plus attribuée à personne, et
    // `sortie_par_dimensions` exigera qu'elle soit encore attachée.
    prises.retain(|p| *p != dxgi);
}

/// Remet sur sa sortie toute fenêtre qui en est partie.
fn controler_le_placement(table: &Table) {
    let toutes = enumerer_sorties().unwrap_or_default();
    for session in table.sessions_vivantes() {
        let Some((adaptateur, index)) = table.sortie_dxgi_de(&session) else {
            continue;
        };
        let Some(cible) = toutes
            .iter()
            .find(|s| s.index_adaptateur == adaptateur && s.index_sortie == index)
        else {
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
