use super::*;

#[test]
fn neuve_n_a_rien_a_relancer_ni_rien_a_stabiliser() {
    let etat = EtatRelance::neuve();
    assert_eq!(etat.tentative(), 0);
    assert_eq!(etat.espacement_ms(), ESPACEMENT_PLANCHER_MS);
}

/// `delai_de_repli(0)` est ÉGAL à `ESPACEMENT_PLANCHER_MS` — la propriété
/// que la doc du module affirme, éprouvée plutôt que crue.
#[test]
fn le_premier_espacement_egale_le_plancher() {
    assert_eq!(delai_de_repli(0), ESPACEMENT_PLANCHER_MS);
}

/// 🔴 LE TEST QUI ANCRE LE CORRECTIF DU ROUND 2 : `SEUIL_STABILITE_MS`
/// doit rester STRICTEMENT AU-DESSUS de `REPLI_MAX_MS`, sans quoi la
/// propriété que ce module existe pour garantir retombe le jour où l'un
/// des deux dérive sans que l'autre suive — même patron que
/// `frein.test.ts::FENETRE_REQUETES_MS_reste_plus_courte_que_FENETRE_MS`.
#[test]
fn le_seuil_de_stabilite_reste_strictement_au_dessus_du_plafond_de_repli() {
    assert!(
        SEUIL_STABILITE_MS > REPLI_MAX_MS,
        "SEUIL_STABILITE_MS = {SEUIL_STABILITE_MS} n'est pas > REPLI_MAX_MS = {REPLI_MAX_MS}"
    );
}

/// 🔴 L'INVARIANT DONT DÉPEND LE ROUND 3 : `reinitialiser_le_repli` doit
/// TOUJOURS avoir déjà agi avant que `stable` ne puisse rendre `true`,
/// sans quoi `stable` redeviendrait la seule remise à zéro possible de
/// facto, et la boucle de trace du round 2 redeviendrait atteignable par
/// un chemin détourné. Transitivement vrai par le test ci-dessus
/// (`SEUIL_STABILITE_MS > REPLI_MAX_MS > ESPACEMENT_PLANCHER_MS`), fixé
/// ici EXPLICITEMENT pour ne pas dépendre d'une transitivité qu'un
/// lecteur pressé pourrait manquer.
#[test]
fn le_plancher_reste_strictement_sous_le_seuil_de_stabilite() {
    assert!(
        ESPACEMENT_PLANCHER_MS < SEUIL_STABILITE_MS,
        "ESPACEMENT_PLANCHER_MS = {ESPACEMENT_PLANCHER_MS} n'est pas < \
         SEUIL_STABILITE_MS = {SEUIL_STABILITE_MS}"
    );
}

#[test]
fn doit_relancer_est_faux_juste_apres_une_tentative() {
    let mut etat = EtatRelance::neuve();
    etat.tentative_lancee();
    assert!(!etat.doit_relancer(0));
    assert!(!etat.doit_relancer(ESPACEMENT_PLANCHER_MS - 1));
}

#[test]
fn doit_relancer_devient_vrai_a_l_espacement_exact() {
    let etat = EtatRelance::neuve();
    assert!(etat.doit_relancer(ESPACEMENT_PLANCHER_MS));
}

#[test]
fn seul_le_premier_lancement_du_cycle_est_signale() {
    let mut etat = EtatRelance::neuve();
    assert!(etat.tentative_lancee(), "le premier lancement doit être signalé");
    assert!(!etat.tentative_lancee(), "le second, du MÊME cycle, ne doit plus l'être");
    assert!(!etat.tentative_lancee(), "ni le troisième");
    assert_eq!(etat.tentative(), 3, "le COMPTE, lui, continue de croître");
}

/// 🔴 LA ROUGE EXACTE DU DÉFAUT CORRIGÉ PAR LE ROUND 2, REJOUÉE ICI :
/// un processus REFUSÉ qui reste vivant jusqu'à `REPLI_MAX_MS` avant de
/// mourir (le sommeil de `honorer_retry_suggere`) ne doit JAMAIS être
/// déclaré stable pendant ce sommeil. Sondé à intervalles réguliers,
/// comme le ferait la boucle du superviseur à ~10 Hz. **C'est LA rouge
/// de la « boucle de trace »** que le round 3 devait préserver en
/// découplant les deux seuils : elle continue de tenir parce que
/// `stable` (seul appelé ici) n'a PAS changé de condition de garde, seul
/// son EFFET (ne plus toucher `tentative`) a changé.
#[test]
fn stable_pendant_un_sommeil_de_refus_ne_declare_jamais_stable() {
    let mut etat = EtatRelance::neuve();
    etat.tentative_lancee();
    let mut ecoule = 0u64;
    while ecoule < REPLI_MAX_MS {
        assert!(
            !etat.stable(ecoule),
            "déclaré stable à {ecoule} ms, alors que le sommeil de refus \
             peut durer jusqu'à {REPLI_MAX_MS} ms — c'est la boucle de \
             trace du round de correction 2"
        );
        ecoule += 97; // un pas non-rond, pour ne pas tomber sur un cas pile
    }
}

/// 🔵 TÉMOIN POSITIF : un processus RÉELLEMENT stable — vivant bien
/// au-delà du sommeil de refus le plus long possible — est bien déclaré
/// stable, et une seule fois. `tentative` est déjà retombée bien AVANT
/// (par `reinitialiser_le_repli`, jamais par `stable` depuis le round de
/// correction 3) : ce test appelle les deux méthodes dans l'ORDRE où
/// `surveiller` les appelle réellement, pour ne pas prouver une
/// propriété que le câblage réel ne tient pas.
#[test]
fn un_processus_reellement_stable_finit_par_etre_declare_stable_une_fois() {
    let mut etat = EtatRelance::neuve();
    etat.tentative_lancee();
    etat.reinitialiser_le_repli(SEUIL_STABILITE_MS - 1);
    assert_eq!(etat.tentative(), 0, "réarmé bien avant la VRAIE stabilité");
    assert!(!etat.stable(SEUIL_STABILITE_MS - 1));
    assert!(etat.stable(SEUIL_STABILITE_MS));
    // Un second appel, cycle déjà retombé : plus rien à signaler tant
    // qu'aucune tentative neuve n'a eu lieu.
    assert!(!etat.stable(SEUIL_STABILITE_MS * 2));
}

/// Sans cycle en cours (aucune tentative lancée depuis la dernière
/// stabilité), `stable` ne doit rien déclarer, quel que soit `ecoule_ms`
/// — un pont jamais relancé n'a rien à « redevenir » stable.
#[test]
fn sans_cycle_en_cours_stable_ne_declare_jamais_rien() {
    let mut etat = EtatRelance::neuve();
    assert!(!etat.stable(SEUIL_STABILITE_MS * 10));
}

/// 🔵 TÉMOIN NÉGATIF, symétrique du précédent : sans cycle en cours,
/// `reinitialiser_le_repli` ne fait rien non plus (`tentative` déjà à
/// zéro) — même garde que `stable`, pour la même raison.
#[test]
fn sans_cycle_en_cours_reinitialiser_le_repli_ne_fait_rien() {
    let mut etat = EtatRelance::neuve();
    etat.reinitialiser_le_repli(SEUIL_STABILITE_MS * 10);
    assert_eq!(etat.tentative(), 0);
}

/// Sous le plancher, `reinitialiser_le_repli` ne réarme rien : un pont
/// vu vivant depuis quelques millisecondes seulement ne prouve encore
/// rien.
#[test]
fn reinitialiser_le_repli_ne_fait_rien_avant_le_plancher() {
    let mut etat = EtatRelance::neuve();
    etat.tentative_lancee();
    etat.tentative_lancee();
    etat.tentative_lancee(); // tentative = 3, espacement > plancher
    assert!(etat.espacement_ms() > ESPACEMENT_PLANCHER_MS, "le repli a bien grandi");
    etat.reinitialiser_le_repli(ESPACEMENT_PLANCHER_MS - 1);
    assert!(
        etat.espacement_ms() > ESPACEMENT_PLANCHER_MS,
        "sous le plancher, aucun réarmement ne doit avoir lieu"
    );
}

/// 🔴 LA ROUGE NEUVE DU ROUND DE CORRECTION 3 — « LE RÉARMEMENT » :
/// un pont SAIN, dont les sessions durent 1 s, 5 s ou 20 s — donc BIEN
/// sous `SEUIL_STABILITE_MS` (35 s), jamais déclaré stable au sens de la
/// trace —, doit malgré tout revenir VITE (au plancher) après une panne
/// FUTURE sans rapport, jamais hériter du plafond d'une panne PASSÉE
/// déjà résolue. Avant ce round (seuil unique = `SEUIL_STABILITE_MS`
/// pour tout), cette assertion aurait rougi : `espacement_ms()` serait
/// resté au-dessus du plancher.
#[test]
fn une_vie_normale_mais_pas_stable_reinitialise_quand_meme_le_repli() {
    let mut etat = EtatRelance::neuve();
    for _ in 0..5 {
        etat.tentative_lancee();
    }
    assert!(etat.espacement_ms() > ESPACEMENT_PLANCHER_MS, "le repli a bien grandi avant le test");

    // Vie NORMALE — 20 s, bien sous les 35 s de `SEUIL_STABILITE_MS`.
    const VIE_NORMALE_MS: u64 = 20_000;
    etat.reinitialiser_le_repli(VIE_NORMALE_MS);
    assert_eq!(
        etat.espacement_ms(),
        ESPACEMENT_PLANCHER_MS,
        "une vie de {VIE_NORMALE_MS} ms doit réarmer le repli, même si \
         elle ne suffit pas à être déclarée STABLE"
    );
    // Et pourtant, pas de ligne « stable » : la trace reste gouvernée
    // par le seuil LONG, inchangé — c'est le découplage lui-même, et la
    // preuve qu'il ne rouvre pas la boucle de trace.
    assert!(
        !etat.stable(VIE_NORMALE_MS),
        "20 s ne suffit pas à la VRAIE stabilité (35 s) — la trace doit \
         rester silencieuse"
    );
}

/// 🔴 LA SIMULATION DE PLUSIEURS CYCLES DE REFUS — le test qui compte les
/// LIGNES QUI SERAIENT ÉMISES, pas les appels : c'est la forme que la
/// revue a demandée au round 2. Chaque cycle : une tentative lancée, un
/// sommeil de refus jusqu'à `REPLI_MAX_MS` sondé à ~10 Hz — avec, à
/// CHAQUE tick, l'appel à `reinitialiser_le_repli` que `surveiller`
/// ferait réellement AVANT `stable` (round 3) —, puis la mort ; le cycle
/// suivant recommence.
///
/// 🔵 **CE QUE LA PREMIÈRE VERSION DE CE TEST CROYAIT À TORT** (rougie
/// avant correction, et c'est la preuve que la propriété n'était pas
/// supposée) : « une ligne "lancé" par cycle ». C'est FAUX, et c'est même
/// MEILLEUR que ça — parce que `stable()` ne réarme JAMAIS `cycle_signale`
/// tant qu'aucun cycle n'atteint la VRAIE stabilité, `tentative_lancee()`
/// rend `premier_du_cycle = false` pour TOUTE relance après la toute
/// première de l'épisode entier. La ligne « lancé » ne sort donc **qu'UNE
/// SEULE FOIS pour tout l'épisode de martèlement**, pas une fois par
/// cycle — exactement la promesse de tête du module : « signaler la
/// première fois, se taire tant que la situation se répète ». Le
/// correctif du round 2 ne fait pas que fermer la fausse ligne « stable » :
/// il restaure aussi le silence attendu sur « lancé », que le bug du
/// seuil unique avait rouvert en réarmant `cycle_signale` à chaque
/// fausse stabilité. **Le round 3 ne change rien à cette propriété** :
/// `reinitialiser_le_repli`, appelée à chaque tick, ne touche jamais
/// `cycle_signale` — voir l'assertion sur `lignes_stables` ci-dessous,
/// inchangée depuis le round 2.
#[test]
fn sur_plusieurs_cycles_de_refus_une_seule_ligne_lancee_et_aucune_ligne_stable() {
    const PAS_MS: u64 = 100; // ~10 Hz, la cadence réelle de la boucle
    let mut etat = EtatRelance::neuve();
    let mut lignes_lancees = 0u32;
    let mut lignes_stables = 0u32;
    for _cycle in 0..5 {
        if etat.tentative_lancee() {
            lignes_lancees += 1;
        }
        let mut ecoule = 0u64;
        while ecoule < REPLI_MAX_MS {
            // Ordre réel de `surveiller` (round 3) : le réarmement du
            // repli AVANT la question de stabilité.
            etat.reinitialiser_le_repli(ecoule);
            if etat.stable(ecoule) {
                lignes_stables += 1;
            }
            ecoule += PAS_MS;
        }
        // Le processus meurt ici (fin du sommeil de refus) : la boucle
        // suivante relancera, donc un cycle NEUF commence côté OS — mais
        // `cycle_signale` reste vrai côté `EtatRelance`, puisqu'aucune
        // stabilité RÉELLE n'a été observée. C'est exactement le
        // `EtatPont` réel entre deux tours de `surveiller`.
    }
    assert_eq!(
        lignes_lancees, 1,
        "UNE SEULE ligne « lancé » pour tout l'épisode — jamais une par \
         cycle : c'est la propriété de silence que le module promet déjà, \
         et que le bug du seuil unique cassait en réarmant `cycle_signale` \
         à chaque fausse stabilité"
    );
    assert_eq!(
        lignes_stables, 0,
        "AUCUNE ligne « stable » ne doit sortir tant qu'aucun cycle n'a \
         vraiment tenu : c'est exactement la boucle que le round de \
         correction 2 ferme, et que le round 3 ne rouvre pas malgré le \
         découplage"
    );
}

/// 🔵 TÉMOIN : si un cycle finit par tenir RÉELLEMENT (le pont cesse
/// d'être refusé), la ligne « stable » sort UNE fois, et le cycle SUIVANT
/// redevient bruyant sur son premier lancement — la moitié du mécanisme
/// que le test ci-dessus, à lui seul, ne peut pas prouver puisqu'aucun de
/// ses cycles n'atteint jamais la stabilité.
#[test]
fn apres_une_vraie_stabilite_le_cycle_suivant_redevient_bruyant() {
    let mut etat = EtatRelance::neuve();
    assert!(etat.tentative_lancee(), "premier lancement de l'épisode : bruyant");
    assert!(etat.stable(SEUIL_STABILITE_MS), "vraiment resté vivant assez longtemps");
    assert!(
        etat.tentative_lancee(),
        "un cycle NEUF, après une vraie stabilité, redevient bruyant"
    );
}
