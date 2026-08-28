//! Calcul et distribution des parts de débit — la branche de
//! `capteur::repartiteur::repartir` sur le registre de `capteur::sommeil`.
//!
//! **Extrait de `sommeil.rs` et non ajouté dedans** : le remède au canal
//! rompu détecté par cette voie (voir `distribuer_les_parts` plus bas) l'y
//! aurait porté au-delà du plafond de 500 lignes du projet. Même motif et
//! même montage que `fenetre.rs` / `fenetre/transitions.rs`.
//!
//! **Aucune visibilité `pub` en dehors du crate** : ce module est un
//! DESCENDANT de `sommeil`, et profite donc de plein droit de l'accès aux
//! items privés de `sommeil.rs` (`Etat`, `Message`, `distribuer`) — la même
//! règle de visibilité Rust qui permet à `fenetre::transitions` d'appeler les
//! méthodes privées de `Fenetre`.

use std::sync::{MutexGuard, OnceLock};

use crate::capteur::repartiteur::{self, Fenetre};

use super::file::Envoi;
use super::{distribuer, oublier, Etat, Message};

/// Budget de débit de la session entière, en bits par seconde.
///
/// **De session, pas par fenêtre** — c'est tout le sujet du sous-bloc D6.
/// Lu une seule fois : le changer en cours de vie n'aurait aucun sens tant
/// que le lien ne change pas.
///
/// **12 Mb/s est un CHOIX, pas une dérivation.** La tâche 1 a montré que le
/// lien porte ≥ 1,45 Gb/s : la capacité du chemin ne borne rien ici, et le
/// budget ne s'en dérive pas. Ce qui borne est ce que le CLIENT décode. La
/// tâche 1bis relève, à huit fenêtres : 18,03 % d'images jetées au barreau
/// plein, 7,99 % à 1024×576, 1,47 % à 640×360 — et surtout que réduire les
/// bits **sans** franchir de seuil de barreau ne sauve rien (23,08 % à
/// surface constante). **Le levier est la résolution, le débit n'en est que
/// la commande.**
///
/// 12 Mb/s conserve au cas mono-fenêtre exactement ce qu'il a aujourd'hui, et
/// donne 1,33 Mb/s par fenêtre à huit — soit le barreau 852×480.
///
/// ⚖️ **MESURÉ par la recette de la tâche 10 (3 août 2026), et la valeur est
/// RECONDUITE — mais l'arbitrage n'est PAS tranché par la mesure.** Le barreau
/// 852×480 à huit fenêtres, que personne n'avait mesuré, l'est : **3,94 %
/// d'images jetées** par le navigateur, contre un seuil de réception fixé à
/// 7,99 %. Le point de repli à 8 Mb/s a été mesuré dans la foulée : **1,46 %
/// et 3,94 %** sur deux exécutions, mais au barreau 640×360.
///
/// **Ce que la comparaison donne est un ARBITRAGE, pas une domination** :
/// 852×480 rend **196,4 MP/s** décodés contre 95,5 à 135,6 à 640×360, et
/// 3,94 % d'images jetées contre 1,46 à 3,94. Plus de pixels livrés, davantage
/// jetés. **Aucune des deux valeurs ne domine l'autre sur les deux grandeurs.**
/// Ce qui fait pencher pour 12 Mb/s tient en une seule raison qui, elle, ne se
/// discute pas : **elle ne coûte rien au cas mono-fenêtre**, là où 8 Mb/s lui
/// retirerait un tiers de son débit — et ce cas-là n'a jamais été mesuré à
/// 12 Mb/s (voir le §2.4 des résultats). **Le choix est donc assumé, pas
/// démontré.**
///
/// ⚠️ **La marge est une marge de LABORATOIRE, et elle est mince.** Le 3,94 %
/// vient d'**une seule** exécution. **Une seule des cinq exécutions à 12 Mb/s
/// passe le seuil** — et **une sur deux** si l'on ne retient que celles dont
/// l'échelle d'encodage s'était réellement posée, la seule population honnête.
/// Les autres relèvent 8,03 %, 16,70 %, 59,32 % et 75,47 %. La dégradation
/// covarie avec la charge de l'hôte de mesure **et** avec le non-établissement
/// de l'échelle ; **les deux ne sont pas départagées.** La grandeur qui
/// commande ici n'est ni le lien (`packetsLost` = 0 partout) ni le débit, mais
/// **ce que le client arrive à décoder** : sur une machine cliente plus lente,
/// 12 Mb/s décrocherait. **Le produit n'est pas démontré robuste** sous la
/// charge d'hôte réellement rencontrée pendant la campagne, et le repli à
/// 8 Mb/s n'a, lui, **jamais été éprouvé sous charge élevée** — qu'il y
/// résiste mieux n'est **pas établi**.
///
/// ⚠️ **`FACTEUR_FOCUS` ne départage PAS les deux valeurs.** Une première
/// rédaction de ce commentaire l'affirmait, sur un sous-ensemble de 8 des
/// 14 déplacements de focus relevés. Sur les 14 : **11 réussissent**, et
/// **8 sur 10 à 12 Mb/s** contre **3 sur 4 à 8 Mb/s** — les deux échecs à
/// 12 Mb/s se produisent avec 51 % de marge sur le seuil de barreau, donc
/// **sans explication arithmétique**. Voir le §3.4 ④ des résultats.
fn budget_bps() -> u32 {
    static BUDGET: OnceLock<u32> = OnceLock::new();
    *BUDGET.get_or_init(|| {
        let budget = std::env::var("BUDGET_BPS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(12_000_000);
        tracing::info!(budget_bps = budget, "budget de debit de la session");
        budget
    })
}

/// Recalcule les parts et n'envoie que celles qui ont changé.
///
/// **Appelée APRÈS `distribuer`**, jamais avant : les ordres de sommeil
/// changent l'éveil, et une part calculée avant eux décrirait l'état
/// précédent.
///
/// Un canal rompu ici est retiré du VIVIER, exactement comme dans
/// `distribuer` — pas seulement de `canaux` et `dernieres_parts`. Sans ce
/// retrait, l'entrée survivrait dans `Vivier::entrees` pour toute la vie du
/// processus : une fois hors de `canaux`, `distribuer` ne la redétecte plus
/// jamais (son bras `None => false` ne voit qu'une session déjà absente), et
/// elle resterait candidate à une place d'encodeur sans qu'aucun fil ne
/// l'occupe. Les ordres que ce retrait engendre (par exemple réveiller une
/// session qui attendait cette place) sont donc relayés à `distribuer`, sous
/// le même verrou — aucune nouvelle prise, `distribuer` reçoit le
/// `MutexGuard` déjà tenu ici.
pub(super) fn distribuer_les_parts(garde: &mut MutexGuard<'static, Etat>) {
    let eveillees = garde.vivier.eveillees();
    let focalisee = garde.focalisee.clone();
    let fenetres: Vec<Fenetre> = garde
        .canaux
        .keys()
        .map(|session| Fenetre {
            session: session.clone(),
            eveillee: eveillees.iter().any(|e| e == session),
            focalisee: focalisee.as_deref() == Some(session.as_str()),
        })
        .collect();

    let parts = repartiteur::repartir(budget_bps(), &fenetres);

    // Les sessions disparues ne doivent pas laisser leur part en mémoire.
    let vivantes: std::collections::HashSet<&String> =
        parts.iter().map(|(session, _)| session).collect();
    garde.dernieres_parts.retain(|session, _| vivantes.contains(session));

    let mut rompus = Vec::new();
    for (session, bps) in parts {
        if garde.dernieres_parts.get(&session) == Some(&bps) {
            continue;
        }
        // ⚠️ **`None` N'EST PAS UNE RUPTURE, et le round 3 a corrigé cette
        // rédaction** — même grief que `registre::distribuer` au round 2.
        // C'est inatteignable aujourd'hui (les sessions sortent de
        // `canaux.keys()` sous le MÊME verrou, quelques lignes plus haut),
        // donc sans conséquence ; mais ce lot s'était donné pour règle de ne
        // plus FABRIQUER d'issue, et l'écrire `Envoi::Rompu` ferait purger une
        // session sur un fait qui n'a pas eu lieu si cette invariance venait à
        // tomber. Un `Option` nomme la chose : il n'y a eu aucun envoi.
        let issue = match garde.canaux.get(&session) {
            Some(canal) => Some(canal.envoyer(Message::Part { bps })),
            None => None,
        };
        match issue {
            // Aucun canal : rien n'est parti, et il n'y a rien à purger — la
            // session n'est déjà plus dans `canaux`.
            None => {}
            // Livrée : on peut mémoriser, et le garde d'écrasement en tête de
            // boucle évitera de la réémettre tant qu'elle ne change pas.
            Some(Envoi::Depose(_)) => {
                garde.dernieres_parts.insert(session, bps);
            }
            // 🔴 REFUSÉE : ON NE MÉMORISE PAS, ET C'EST TOUT LE CORRECTIF DU
            // ROUND 1. La file de cette fenêtre était pleine : la part n'est
            // jamais partie. L'inscrire dans `dernieres_parts` ferait juger la
            // valeur « déjà livrée » par le garde d'écrasement ci-dessus, qui
            // supprimerait alors TOUTE réémission future de cette valeur — la
            // fenêtre resterait à son débit précédent tant que sa part
            // calculée ne change pas, sans borne. En ne mémorisant rien, le
            // tour de roue suivant la repropose de lui-même.
            //
            // ⚠️ **Et surtout PAS `rompus.push`** : la session est VIVANTE,
            // seulement en retard. La purger reviendrait à tuer l'arbitrage de
            // la fenêtre la plus en peine — exactement la mauvaise réaction.
            //
            // Le refus est déjà journalisé, au palier et avec le nom de la
            // session, par `EmetteurSession::journaliser_le_refus` : le
            // retracer ici doublerait la ligne sans rien ajouter.
            Some(Envoi::Refuse) => {}
            Some(Envoi::Rompu) => rompus.push(session),
        }
    }

    // Le remede : un canal rompu ICI n'est pas seulement une part perdue,
    // c'est le meme signal qu'un canal rompu dans `distribuer` — une fenetre
    // dont le fil est parti sans passer par `retirer` (voir `Fenetre::servir`,
    // point de passage unique cote fil, court-circuite par une panique). Sans
    // ce retrait du vivier, l'entree y survivrait pour toute la vie du
    // processus.
    //
    // `oublier` et non trois retraits écrits ici : c'est le point de passage
    // unique du registre, et il porte le champ que cette boucle omettait —
    // `focalisee` (M1, revue finale de branche). Voir sa doc.
    let mut ordres_du_retrait = Vec::new();
    for session in rompus {
        ordres_du_retrait.extend(oublier(garde, &session));
    }
    if !ordres_du_retrait.is_empty() {
        distribuer(garde, ordres_du_retrait);
    }
}

// Module de tests extrait dans un fichier voisin : ce fichier était à 488
// lignes pour un plafond de projet à 500, et le round de correction 2 y
// ajoute. Extraire, jamais comprimer — et dans une tâche DÉDIÉE, avant celle
// qui ajoute. Même idiome que `file/tests.rs` et `superviseur/table.rs` ;
// voir la doc en tête du fichier extrait.
#[cfg(test)]
#[path = "parts/tests.rs"]
mod tests;
