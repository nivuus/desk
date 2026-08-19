//! Le registre des attentes de connexion média — extrait de `serveur.rs`
//! (revue de la tâche 16, D10) : `serveur.rs` était à exactement 500 lignes,
//! et ce registre — la statique, ses accès, `attendre_le_media`, `oublier` —
//! est LITTÉRALEMENT la chose qu'on transpose depuis `sommeil/registre.rs`.
//! L'extraire ici rend ce parallèle structurel plutôt que seulement affirmé
//! dans un commentaire, et le remède du constat ci-dessous (le verrou unique)
//! en profite directement : c'est le même geste, dans le même mouvement.
//! Même montage que `sommeil.rs` → `sommeil/registre.rs`.
//!
//! **Sessions attachées sur leur connexion de commandes et attendant leur
//! connexion média.** Clé : l'identifiant de session ; valeur : le canal
//! d'envoi de la connexion média à venir, et la génération de cette attente.

use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::{Mutex, MutexGuard, OnceLock};

/// L'état complet du registre — le contenu ET le compteur de génération,
/// sous UN SEUL verrou.
///
/// ⚠️ **Ce n'est PAS la première forme de ce fichier.** La première version
/// (revue de la tâche 16) posait la génération dans un `AtomicU64` séparé du
/// `Mutex` de la carte, en arguant que « la monotonie d'un `fetch_add` ne
/// dépend d'aucune autre [donnée] ». **C'était le mauvais invariant.** Ce qui
/// compte n'est pas que le COMPTEUR soit monotone — il l'est toujours — mais
/// que la génération STOCKÉE DANS LA CARTE soit celle du DERNIER `insert`. Un
/// `fetch_add` et un `insert` dans deux verrous distincts sont deux sections
/// critiques distinctes, qui peuvent s'entrelacer :
///
/// ```text
/// T1 (attache A) : fetch_add -> 1
/// T2 (attache B) : fetch_add -> 2
/// T2 : verrou, insert (media2, 2)
/// T1 : verrou, insert (media1, 1)   // écrase la PLUS RÉCENTE par la 1
/// ```
///
/// Le registre porterait alors la génération 1 alors que 2 est l'attache la
/// plus tardive — une inversion que `sommeil/registre.rs` NE PEUT PAS
/// produire, parce que `prochaine_generation += 1` et l'`insert` y sont sous
/// le MÊME garde (`sommeil/registre.rs::inscrire`). La fenêtre est étroite —
/// il faut deux fils dans `attendre_le_media` pour la MÊME session, donc un
/// enfant qui se rattache pendant que son `ouvrir_les_commandes` précédent
/// tourne encore — et la conséquence n'est pas fatale (au pire une génération
/// périmée acceptée à tort), mais c'est une inversion que le patron
/// transposé n'a pas, et qui n'a donc pas de raison d'exister ici non plus.
///
/// **Le remède** : un seul verrou pour les deux, exactement comme
/// `sommeil::registre::Etat` (`canaux` et `prochaine_generation` y vivent
/// déjà côte à côte, sous le même `Mutex`).
struct Etat {
    attentes: HashMap<String, (Sender<std::fs::File>, u64)>,
    prochaine_generation: u64,
}

static ETAT: OnceLock<Mutex<Etat>> = OnceLock::new();

/// Verrouille l'état en survivant à un empoisonnement : un fil de fenêtre
/// qui panique ne doit pas emporter l'accueil de toutes les suivantes.
fn etat() -> MutexGuard<'static, Etat> {
    ETAT.get_or_init(|| {
        Mutex::new(Etat {
            attentes: HashMap::new(),
            prochaine_generation: 0,
        })
    })
    .lock()
    .unwrap_or_else(|empoisonne| empoisonne.into_inner())
}

/// Inscrit l'attente de connexion média d'une session, et rend la génération
/// qui vient de lui être attribuée.
///
/// **La génération est frappée ICI, à l'ATTACHE — pas transmise par le
/// protocole, pas prise au lancement du processus.** Même correction que
/// celle que la revue du sous-bloc D9 a imposée sur
/// `capteur::sommeil::registre::inscrire` (voir son commentaire, et celui-ci,
/// plus bas, de `oublier`) : la course F5 se joue entre deux attaches
/// successives de la MÊME session (un enfant qui se rattache après une
/// rupture de tube redit le même nom), jamais entre deux lancements de
/// processus — un enfant relancé par le superviseur reçoit un nom NEUF
/// (`Table::compteur`, `superviseur/table.rs`), donc aucune course. Seule une
/// génération frappée à l'attache distingue les deux inscriptions qui, elles,
/// PEUVENT se chevaucher.
///
/// L'appelant (`ouvrir_les_commandes`, dans `serveur.rs`) retient la valeur
/// rendue le temps du service de cette connexion et la redonne telle quelle
/// à `oublier`.
pub(super) fn attendre_le_media(session: &str, media: Sender<std::fs::File>) -> u64 {
    let mut garde = etat();
    // Frappée et insérée SOUS LE MÊME GARDE : c'est tout le remède au
    // constat ci-dessus, voir le commentaire d'`Etat`.
    garde.prochaine_generation += 1;
    let generation = garde.prochaine_generation;
    // Un remplacement se journalise : il signale un enfant qui se rattache
    // sans que la précédente attente ait été soldée. Laisser tomber l'ancien
    // émetteur réveille aussitôt le fil de fenêtre correspondant.
    if garde.attentes.insert(session.to_string(), (media, generation)).is_some() {
        tracing::warn!(%session, "attente de connexion média remplacée pour cette session");
    }
    generation
}

/// Retire l'attente de connexion média d'une session — **si et seulement si**
/// la génération présentée est bien la courante.
///
/// ✅ **C'est LITTÉRALEMENT la course F5 (D7), fermée ici sur le SECOND
/// registre (D10, leg 2).** Le sous-bloc D9 l'avait fermée sur le registre de
/// sommeil seul (`capteur/sommeil/registre.rs` : `inscrire` frappe une
/// génération monotone, `retirer` s'efface si l'enregistrée est plus
/// récente) — le brief de sa tâche 10 ne nommait que `sommeil`, et aucune
/// revue par tâche ne pouvait voir ce jumeau. Relevé par la revue transverse
/// de fin de branche D9, refermé ici avec le même patron.
///
/// Sans cette comparaison, un `remove` inconditionnel émis par un fil tardif
/// — un abandon qui expire à l'instant précis où la même session vient de se
/// réattacher — emporterait l'attente NEUVE : l'enfant qui vient de se
/// rattacher attendrait alors un média que plus personne ne lui délivrerait.
pub(super) fn oublier(session: &str, generation: u64) {
    let mut garde = etat();
    if garde.attentes.get(session).is_some_and(|(_, g)| *g != generation) {
        tracing::info!(%session, generation, "oubli périmé ignoré");
        return;
    }
    garde.attentes.remove(session);
}

/// Retire et rend l'émetteur média attendu par une session, **quelle que soit
/// sa génération** — appelé quand la connexion média elle-même arrive
/// (`accueillir`, branche `VersCapteur::Identite`, dans `serveur.rs`).
///
/// La génération n'a rien à dire ici : ce retrait apparie la connexion média
/// à l'attente COURANTE, quelle qu'elle soit — ce n'est PAS le chemin de la
/// course F5, qui ne joue qu'entre deux appels à `oublier` (voir son
/// commentaire). Une connexion média arrivant pour une génération périmée
/// n'est de toute façon plus attendue par personne : son `Receiver` a été
/// abandonné avec l'entrée remplacée par la réattache.
pub(super) fn retirer_pour_identite(session: &str) -> Option<Sender<std::fs::File>> {
    etat().attentes.remove(session).map(|(media, _generation)| media)
}

#[cfg(test)]
mod tests;
