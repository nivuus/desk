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
        let envoye = match garde.canaux.get(&session) {
            Some(canal) => canal.send(Message::Part { bps }).is_ok(),
            None => false,
        };
        if envoye {
            garde.dernieres_parts.insert(session, bps);
        } else {
            rompus.push(session);
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

#[cfg(test)]
mod tests {
    use std::sync::mpsc::Receiver;

    use crate::capteur::sommeil::tests::{premier_ordre, verrouiller_pour_le_test};
    use crate::capteur::sommeil::{inscrire, retirer, signaler, Message};
    use crate::capteur::vivier::Ordre;

    /// Dernière part reçue sur un canal, en vidant ce qui s'y trouve.
    fn derniere_part(canal: &Receiver<Message>) -> Option<u32> {
        canal
            .try_iter()
            .filter_map(|m| match m {
                Message::Part { bps } => Some(bps),
                _ => None,
            })
            .last()
    }

    #[test]
    fn une_session_qui_s_eveille_recoit_une_part_apres_son_ordre_de_reveil() {
        let _verrou = verrouiller_pour_le_test();
        let messages = inscrire("t6-a", 6001);
        signaler("t6-a", true, true);

        let recus: Vec<Message> = messages.try_iter().collect();
        let position_reveil = recus
            .iter()
            .position(|m| matches!(m, Message::Sommeil(Ordre::Reveiller)))
            .expect("l'ordre de réveil doit être présent");
        // La recherche part de `position_reveil`, pas du début : l'inscription
        // elle-même envoie déjà une PREMIÈRE part (le plancher endormi, avant
        // tout ordre — voir la doc de `inscrire`), et c'est légitime. La
        // recherche depuis le début confondrait cette part-là, envoyée AVANT
        // le réveil, avec celle que ce test veut vérifier : celle qui décrit
        // la fenêtre ÉVEILLÉE, et qui doit suivre son ordre.
        let position_part = recus[position_reveil..]
            .iter()
            .position(|m| matches!(m, Message::Part { .. }))
            .map(|i| i + position_reveil)
            .expect("une part doit suivre le réveil");
        assert!(
            position_reveil < position_part,
            "la part suit l'ordre, jamais l'inverse : une fenêtre encore endormie \
             recevrait sinon une part d'éveillée"
        );
        retirer("t6-a");
    }

    #[test]
    fn une_part_inchangee_n_est_pas_reemise() {
        let _verrou = verrouiller_pour_le_test();
        let messages = inscrire("t6-b", 6002);
        signaler("t6-b", true, true);
        let _ = messages.try_iter().count();

        // Même signal, donc même état, donc même part : rien ne doit partir.
        signaler("t6-b", true, true);
        let parts: Vec<Message> = messages
            .try_iter()
            .filter(|m| matches!(m, Message::Part { .. }))
            .collect();
        assert!(parts.is_empty(), "une part inchangée ne se réémet pas : {parts:?}");
        retirer("t6-b");
    }

    #[test]
    fn l_arrivee_d_une_seconde_fenetre_reduit_la_part_de_la_premiere() {
        let _verrou = verrouiller_pour_le_test();
        let a = inscrire("t6-c", 6003);
        signaler("t6-c", true, true);
        let premiere = derniere_part(&a).expect("la première doit avoir une part");

        let b = inscrire("t6-d", 6004);
        signaler("t6-d", true, false);
        let apres = derniere_part(&a).expect("la première doit être ré-servie");
        assert!(
            apres < premiere,
            "part de la première : {premiere} puis {apres} — elle doit baisser"
        );
        assert!(derniere_part(&b).is_some(), "la seconde doit recevoir une part");

        retirer("t6-c");
        retirer("t6-d");
    }

    /// Le test qui couvre le défaut trouvé en revue : un canal rompu détecté
    /// PENDANT la distribution des PARTS (pas pendant celle des ordres) doit
    /// libérer sa place au vivier, pas seulement dans `canaux`. Avant le
    /// remède, l'entrée y survivait pour toute la vie du processus dès qu'un
    /// fil de fenêtre mourait sans passer par `retirer` — le cas nominal
    /// d'une panique, court-circuitant le point de passage unique de
    /// `Fenetre::servir`.
    #[test]
    fn un_canal_rompu_detecte_par_les_parts_est_retire_du_vivier() {
        let _verrou = verrouiller_pour_le_test();
        // Sature les PLAFOND_EVEIL (8) places.
        let mut recepteurs_pleins = Vec::new();
        for i in 0..8 {
            let nom = format!("t7-plein-{i}");
            let ordres = inscrire(&nom, 6100 + i as u32);
            signaler(&nom, true, false);
            assert_eq!(
                premier_ordre(&ordres),
                Some(Ordre::Reveiller),
                "{nom} devrait s'eveiller"
            );
            recepteurs_pleins.push((nom, ordres));
        }

        // Le fil de la premiere "meurt" : son recepteur est jete SANS passer
        // par `retirer`, exactement ce qui arrive quand un fil de fenetre
        // panique avant d'atteindre son point de retrait unique. `canaux`
        // garde donc une entree dont plus personne ne lit.
        let (session_morte, recepteur_mort) = recepteurs_pleins.remove(0);
        drop(recepteur_mort);

        // "t7-attend" arrive. La simple INSCRIPTION d'une session neuve
        // (endormie) fait deja varier le calcul des parts des huit eveillees
        // existantes : chaque endormie retranche son `PART_DORMANTE_BPS` du
        // budget partage AVANT que le reste ne soit divise (voir `repartir`),
        // donc `reste` change, donc la part de CHAQUE eveillee change — y
        // compris celle de la session morte. La tentative d'envoi qui en
        // resulte sur son canal rompu declenche le remede : elle est retiree
        // du VIVIER (et pas seulement de `canaux`), ce qui libere sa place.
        let ordres_attend = inscrire("t7-attend", 6200);

        // Se signaler visible suffit desormais : la place est deja libre.
        // Sans le remede (retrait du vivier en plus de `canaux`), la session
        // morte y resterait comptee comme eveillee pour toujours, la place ne
        // se libererait jamais, et "t7-attend" resterait endormie ici.
        signaler("t7-attend", true, false);
        assert_eq!(
            premier_ordre(&ordres_attend),
            Some(Ordre::Reveiller),
            "le retrait de la session morte, detecte par la distribution des parts, \
             doit liberer sa place au vivier"
        );

        // Nettoyage. `retirer` sur la session deja retiree par le remede est
        // un no-op sur une cle deja absente, aussi bien pour `Vivier::retirer`
        // (HashMap::remove) que pour `canaux` — pas un double retrait.
        retirer("t7-attend");
        retirer(&session_morte);
        for (nom, _) in recepteurs_pleins {
            retirer(&nom);
        }
    }

    /// Le défaut trouvé en revue de la tâche 6 : `sommeil::inscrire` remplace
    /// le canal d'une session déjà connue (rattachement après rupture de
    /// tube) sans purger `dernieres_parts`. Si la topologie n'a pas changé
    /// entre les deux inscriptions, la part recalculée est identique à celle
    /// déjà mémorisée, le filtre d'écrasement de `distribuer_les_parts` la
    /// juge donc déjà livrée, et le canal NEUF ne reçoit jamais rien — le
    /// plafond de débit de cet enfant reste périmé sans terme.
    #[test]
    fn un_rattachement_a_topologie_inchangee_renvoie_une_part_sur_le_canal_neuf() {
        let _verrou = verrouiller_pour_le_test();

        // Premier canal : inscription seule, aucune autre fenêtre, aucun
        // signal — la fenêtre naît endormie et reçoit tout de même la part
        // plancher à l'inscription (voir la doc de `inscrire`).
        let premier_canal = inscrire("t8-rattache", 6300);
        let premiere_part = derniere_part(&premier_canal)
            .expect("une première part doit partir à l'inscription initiale");

        // Le tube se rompt et l'enfant se rattache : MÊME session, rien
        // d'autre dans la topologie n'a bougé (aucune autre fenêtre, aucun
        // signal de visibilité entre-temps). `inscrire` détecte le
        // remplacement (elle journalise « canal d'ordres remplacé pour
        // cette session ») et rend un canal neuf.
        let canal_neuf = inscrire("t8-rattache", 6300);

        // Sans le remède, la part recalculée est identique à `premiere_part`
        // : `dernieres_parts` la juge déjà livrée (elle l'était, mais sur
        // L'ANCIEN canal, disparu avec la rupture) et rien ne part sur le
        // canal neuf, qui reste muet pour toujours tant que la topologie ne
        // change pas.
        assert_eq!(
            derniere_part(&canal_neuf),
            Some(premiere_part),
            "le canal neuf doit recevoir sa part même si elle est identique à celle \
             déjà envoyée sur l'ancien canal : dernieres_parts doit être purgée pour \
             cette session au moment où son canal est remplacé"
        );

        retirer("t8-rattache");
    }
}
