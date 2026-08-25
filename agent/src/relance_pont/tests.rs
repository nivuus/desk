use super::*;

#[test]
fn neuve_n_a_rien_a_relancer_ni_rien_a_stabiliser() {
    let etat = EtatRelance::neuve();
    assert_eq!(etat.tentative(), 0);
    assert_eq!(etat.espacement_ms(), ESPACEMENT_PLANCHER_MS);
}

/// `delai_de_repli(0)` est ÉGAL à `ESPACEMENT_PLANCHER_MS` — la propriété
/// que la doc du module affirme, éprouvée plutôt que crue.
///
/// ⚠️ **C'EST AUSSI LA SOUDURE** qui empêche de relever
/// `ESPACEMENT_PLANCHER_MS` seule : ce test exige
/// `REPLI_MIN_MS == ESPACEMENT_PLANCHER_MS`, et la doc de la constante le
/// dit désormais.
#[test]
fn le_premier_espacement_egale_le_plancher() {
    assert_eq!(delai_de_repli(0), ESPACEMENT_PLANCHER_MS);
}

/// 🔴 LE TEST QUI ANCRE LE CORRECTIF DU ROUND 2 : `SEUIL_STABILITE_MS`
/// doit rester STRICTEMENT AU-DESSUS de `REPLI_MAX_MS`, sans quoi la
/// propriété que ce module existe pour garantir retombe le jour où l'un
/// des deux dérive sans que l'autre suive — même patron que
/// `frein.test.ts::FENETRE_REQUETES_MS_reste_plus_courte_que_FENETRE_MS`.
///
/// ⚠️ **SON JUMEAU A ÉTÉ RETIRÉ AU ROUND 4, ET C'EST DIT PLUTÔT QUE PASSÉ SOUS SILENCE** :
/// `le_plancher_reste_strictement_sous_le_seuil_de_stabilite` fixait
/// `ESPACEMENT_PLANCHER_MS < SEUIL_STABILITE_MS` pour garantir l'ORDRE dans
/// lequel `surveiller` appelait `reinitialiser_le_repli` puis `stable`. Cet
/// ordre n'existe plus : les deux méthodes vivent désormais dans des
/// BRANCHES DIFFÉRENTES de `surveiller` (mort contre vivant), et ne peuvent
/// plus s'exécuter au même tour. Garder ce test aurait été garder une
/// justification fausse — ce que ce dépôt paie plus cher qu'un test de
/// moins.
#[test]
fn le_seuil_de_stabilite_reste_strictement_au_dessus_du_plafond_de_repli() {
    assert!(
        SEUIL_STABILITE_MS > REPLI_MAX_MS,
        "SEUIL_STABILITE_MS = {SEUIL_STABILITE_MS} n'est pas > REPLI_MAX_MS = {REPLI_MAX_MS}"
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

/// 🔴 **`EtatObserve` N'A AUCUN CONSOMMATEUR SUR L'HÔTE** (les trois seuls
/// vivent dans `surveillance_pont.rs` et `lanceur/pont.rs`, tous deux
/// `#[cfg(windows)]` — voir la doc de tête du module). Sans ce test, son
/// ré-export ET l'enum elle-même sont du code mort pour `cargo check` côté
/// hôte : deux avertissements, dont un NEUF depuis l'extraction — la classe
/// même que `CLAUDE.md` nomme pour les extractions (« elle laisse ses
/// imports derrière elle »).
#[test]
fn etat_observe_distingue_mort_de_vivant_et_d_absent_par_son_issue() {
    let mort_propre = EtatObserve::Mort(IssueDeSortie::Propre);
    let mort_erreur = EtatObserve::Mort(IssueDeSortie::Erreur);
    assert_ne!(mort_propre, mort_erreur, "l'issue distingue deux morts");
    assert_ne!(EtatObserve::Vivant, EtatObserve::Absent);
    assert_ne!(EtatObserve::Vivant, mort_propre);
}

/// La traduction du code de sortie, dans les trois sens — y compris celui
/// qui ne court PAS sur la cible (voir la doc de `depuis_le_code`).
#[test]
fn le_code_de_sortie_se_traduit_dans_les_trois_sens() {
    assert_eq!(IssueDeSortie::depuis_le_code(Some(0)), IssueDeSortie::Propre);
    assert_eq!(IssueDeSortie::depuis_le_code(Some(1)), IssueDeSortie::Erreur);
    assert_eq!(IssueDeSortie::depuis_le_code(Some(101)), IssueDeSortie::Erreur);
    assert_eq!(IssueDeSortie::depuis_le_code(None), IssueDeSortie::Inconnue);
}

/// 🔴 **LA ROUGE DU ROUND DE CORRECTION 4, MOITIÉ « ON RÉARME »** — et elle
/// remplace `une_vie_normale_mais_pas_stable_reinitialise_quand_meme_le_
/// repli`, qui **assertait le défaut comme la propriété désirée** : le
/// défaut était scellé dans son propre contrôle.
///
/// Un pont qui a été refusé cinq fois, puis qui SERT une session et se
/// termine PROPREMENT, doit repartir du plancher — « un agent connecté
/// depuis trois jours qui perd son réseau une seconde doit reprendre en une
/// demi-seconde, pas en trente ». **Et la durée de cette session n'entre
/// nulle part** : le test le montre en n'en fournissant aucune.
#[test]
fn une_sortie_propre_rearme_le_repli_sans_qu_aucune_duree_n_intervienne() {
    let mut etat = EtatRelance::neuve();
    for _ in 0..5 {
        etat.tentative_lancee();
    }
    assert!(etat.espacement_ms() > ESPACEMENT_PLANCHER_MS, "le repli a bien grandi avant le test");
    etat.reinitialiser_le_repli(IssueDeSortie::Propre);
    assert_eq!(
        etat.espacement_ms(),
        ESPACEMENT_PLANCHER_MS,
        "une sortie PROPRE doit réarmer le repli : la panne passée est résolue"
    );
}

/// 🔴 **LA ROUGE DU ROUND DE CORRECTION 4, MOITIÉ « ON NE RÉARME PAS » —
/// C'EST ELLE QUI TIENT LE DÉFAUT MESURÉ.** Un pont REFUSÉ honore
/// `retryApresS`, reste vivant jusqu'à `REPLI_MAX_MS`, **puis meurt en
/// erreur** : sa longue vie ne prouve rien, et le repli doit continuer de
/// croître. Sous le round 3, la même situation réarmait le repli ~500 ms
/// après CHAQUE lancement, et la cadence montait à 100 connexions/minute
/// contre un budget partagé de 120.
///
/// 🔵 Les DEUX autres issues sont éprouvées dans le même test, parce
/// qu'elles partagent la conclusion : ni `Erreur` ni `Inconnue` ne réarme.
#[test]
fn ni_un_refus_ni_une_issue_inconnue_ne_rearment_le_repli() {
    for issue in [IssueDeSortie::Erreur, IssueDeSortie::Inconnue] {
        let mut etat = EtatRelance::neuve();
        for _ in 0..5 {
            etat.tentative_lancee();
        }
        let avant = etat.espacement_ms();
        assert!(avant > ESPACEMENT_PLANCHER_MS, "le repli a bien grandi avant le test");
        // Une vie TRÈS longue — plus longue que le sommeil de refus le plus
        // long possible — et pourtant aucun réarmement : la durée n'entre
        // pas dans la décision, c'est tout le round 4.
        etat.reinitialiser_le_repli(issue);
        assert_eq!(
            etat.espacement_ms(),
            avant,
            "{issue:?} ne doit RIEN réarmer, quelle qu'ait été la durée de vie"
        );
    }
}

/// 🔴 LA ROUGE EXACTE DU DÉFAUT CORRIGÉ PAR LE ROUND 2, REJOUÉE ICI :
/// un processus REFUSÉ qui reste vivant jusqu'à `REPLI_MAX_MS` avant de
/// mourir (le sommeil de `honorer_retry_suggere`) ne doit JAMAIS être
/// déclaré stable pendant ce sommeil. Sondé à intervalles réguliers,
/// comme le ferait la boucle du superviseur à ~10 Hz. **C'est LA rouge
/// de la « boucle de trace »**, que les rounds 3 et 4 devaient préserver :
/// elle continue de tenir parce que `stable` n'a jamais changé de
/// condition de garde.
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
/// stable, et une seule fois.
///
/// 🔴 **CE QU'IL TENAIT AU ROUND 4 EST DEVENU INATTEIGNABLE AU ROUND 5, ET
/// CE N'EST PAS UN TROU DE COUVERTURE — C'EST UNE MEILLEURE GARANTIE.** La
/// version précédente visait l'état `cycle_signale == false && tentative >
/// 0` (un pont déclaré stable puis terminé proprement, la garde du round 3
/// sur `reinitialiser_le_repli` bloquant alors le réarmement) en appelant
/// `reinitialiser_le_repli(Propre)` APRÈS `stable`. Depuis que `stable()`
/// remet ELLE-MÊME `tentative` à zéro dans sa branche vraie (voir sa doc),
/// cet appel ne pouvait plus rien prouver : `tentative` était déjà à zéro
/// AVANT lui, quoi que fasse `reinitialiser_le_repli` — prouvé par
/// mutation. La garantie réelle est l'INVARIANT qui rend cet état
/// inatteignable, tenu directement ci-dessous plutôt que déduit : `stable()
/// == true` remet `tentative` à zéro DANS LE MÊME GESTE qui fait retomber
/// `cycle_signale`, donc il n'existe plus d'instant où `cycle_signale ==
/// false` et `tentative > 0` à la fois.
#[test]
fn un_processus_reellement_stable_finit_par_etre_declare_stable_une_fois() {
    let mut etat = EtatRelance::neuve();
    etat.tentative_lancee();
    etat.tentative_lancee(); // le repli a grandi : tentative = 2
    assert!(!etat.stable(SEUIL_STABILITE_MS - 1));
    assert!(etat.stable(SEUIL_STABILITE_MS));
    assert_eq!(
        etat.tentative(),
        0,
        "l'invariant qui remplace la garantie visée au round 4 : `stable() \
         == true` remet `tentative` à zéro dans le MÊME geste qui fait \
         retomber `cycle_signale` — l'état que ce test visait avant \
         (cycle retombé, tentative encore positive) n'existe plus"
    );
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

/// 🔴 LA SIMULATION DE PLUSIEURS CYCLES DE REFUS — le test qui compte les
/// LIGNES QUI SERAIENT ÉMISES, pas les appels : c'est la forme que la
/// revue a demandée au round 2. Chaque cycle : une tentative lancée, un
/// sommeil de refus jusqu'à `REPLI_MAX_MS` sondé à ~10 Hz, puis la mort EN
/// ERREUR — l'issue que `bail!` produit — que `surveiller` constate une
/// fois avant de relancer.
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
/// première fois, se taire tant que la situation se répète ».
/// **Le round 4 ne change rien à cette propriété** : `reinitialiser_le_
/// repli` ne touche jamais `cycle_signale`, et l'issue `Erreur` ne réarme
/// même pas `tentative`.
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
            if etat.stable(ecoule) {
                lignes_stables += 1;
            }
            ecoule += PAS_MS;
        }
        // Le processus meurt ici, EN ERREUR (fin du sommeil de refus, puis
        // le `bail!` de `pont::executer`) : c'est ce que `surveiller`
        // constate, une seule fois, avant de relancer.
        etat.reinitialiser_le_repli(IssueDeSortie::Erreur);
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
         correction 2 ferme, et que ni le round 3 ni le round 4 ne rouvrent"
    );
    assert!(
        etat.espacement_ms() > REPLI_MAX_MS / 2,
        "après cinq refus, l'espacement doit avoir GRANDI — c'est le \
         défaut du round 3, où il retombait au plancher à chaque cycle"
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

/// 🔴 **LE TEST-JUGE DU LOT ENTIER : LA CADENCE DE CONNEXIONS `/signal`
/// D'UN PONT QUI MEURT EN BOUCLE.** C'est le chiffre que le legs des freins
/// manquants existe pour borner — `plateforme/src/securite/frein.ts` donne
/// `REQUETES_MAX_ADRESSE = 120` par fenêtre de 60 s, **partagée** avec la
/// session de contrôle du superviseur et chaque enfant de fenêtre.
///
/// ⚠️ **CE TEST RÉ-ÉCRIT LE CÂBLAGE** (`surveiller` + `tenter`), qui vit
/// derrière `#[cfg(windows)]` et reste hors de portée de
/// `cargo test --workspace` : il éprouve la DÉCISION sous une cadence
/// réaliste, jamais le câblage réel. C'est dit plutôt que supposé.
///
/// 🔵 **MESURÉ, PAS SUPPOSÉ** : sous le round 3, cette même simulation
/// rendait **100** lancements pour `VIE_MS = 600` (et 60 pour 1 s, 30 pour
/// 2 s, 12 pour 5 s) — 83 % du budget partagé consommé par le pont seul,
/// **sans qu'aucun `retryApresS` n'ait à intervenir**.
#[test]
fn un_pont_qui_meurt_en_erreur_apres_une_demi_seconde_ne_martele_pas() {
    const PAS_MS: u64 = 100;
    const DUREE_MS: u64 = 60_000;
    const VIE_MS: u64 = 600;
    // 🔴 LA VALEUR MESURÉE, PAS UNE BORNE LÂCHE. Une rédaction antérieure
    // assertait `<= 120 / 10`, soit 12, là où la mesure rend 6 : **une
    // régression qui DOUBLERAIT la cadence serait passée** (relevé par la
    // revue du round de correction 5). L'égalité exacte oblige à REMESURER
    // le jour où `REPLI_MIN_MS` ou `REPLI_MAX_MS` bougent, ce qui est
    // exactement ce qu'on veut d'un chiffre-juge.
    const LANCEMENTS_MESURES: u32 = 6;
    // Le budget PARTAGÉ de la plateforme, pour la lecture du message.
    const BUDGET_PARTAGE_PAR_MINUTE: u32 = 120;

    let mut relance = EtatRelance::neuve();
    let mut derniere_tentative_ms: i64 = -(ESPACEMENT_PLANCHER_MS as i64);
    let mut lance_a: Option<i64> = None;
    let mut lancements = 0u32;
    let mut horloge: i64 = 0;
    while horloge < DUREE_MS as i64 {
        let vivant = matches!(lance_a, Some(t) if horloge - t < VIE_MS as i64);
        let ecoule_ms = (horloge - derniere_tentative_ms) as u64;
        if vivant {
            let _ = relance.stable(ecoule_ms);
        } else {
            if lance_a.take().is_some() {
                // La mort est constatée UNE FOIS : `bail!` après le refus.
                relance.reinitialiser_le_repli(IssueDeSortie::Erreur);
            }
            if relance.doit_relancer(ecoule_ms) {
                derniere_tentative_ms = horloge;
                relance.tentative_lancee();
                lance_a = Some(horloge);
                lancements += 1;
            }
        }
        horloge += PAS_MS as i64;
    }
    assert_eq!(
        lancements, LANCEMENTS_MESURES,
        "{lancements} connexions /signal en une minute pour le pont SEUL, \
         contre un budget PARTAGÉ de {BUDGET_PARTAGE_PAR_MINUTE} et une \
         mesure de {LANCEMENTS_MESURES} : c'est le verrouillage de la VM que \
         ce lot existe pour fermer"
    );
}

/// 🔴 **LA ROUGE DU ROUND DE CORRECTION 5 — « LA CITATION QUE LE MODULE
/// PORTE DEPUIS LE ROUND 3 ÉTAIT FAUSSE POUR UN CAS ».** Un pont refusé six
/// fois (espacement au PLAFOND), puis relancé une septième fois et qui SERT
/// TROIS JOURS, puis qu'une coupure réseau tue — donc une mort EN ERREUR,
/// qu'aucune sortie propre ne vient racheter — doit voir son
/// `espacement_ms()` RETOMBER au plancher plutôt que rester au plafond
/// d'une panne déjà résolue. ⚠️ **CE N'EST PAS « reprendre en une
/// demi-seconde, pas en trente » : la revue finale a mesuré, au niveau
/// boucle, que la PREMIÈRE reprise après la coupure est immédiate dans les
/// DEUX cas** (`doit_relancer` compare l'écoulé depuis le LANCEMENT, et
/// trois jours de vie dépassent tout repli) — **c'est la RAMPE de
/// l'épisode SUIVANT qui repart du plancher**, et ce test l'éprouve au
/// niveau de l'état PUR, un cran avant la boucle.
///
/// 🔵 **CE QUI REND LA DURÉE LÉGITIME ICI, LÀ OÙ ELLE NE L'ÉTAIT PAS AU
/// ROUND 3** : le seuil franchi est `SEUIL_STABILITE_MS` (35 s), qu'un pont
/// refusé qui dort ne peut PAS atteindre — son sommeil est borné à
/// `REPLI_MAX_MS` (30 s). Les deux tests qui tiennent cet invariant sont
/// `le_seuil_de_stabilite_reste_strictement_au_dessus_du_plafond_de_repli`
/// et `stable_pendant_un_sommeil_de_refus_ne_declare_jamais_stable`.
#[test]
fn une_longue_vie_stable_puis_une_mort_en_erreur_reprend_au_plancher() {
    const TROIS_JOURS_MS: u64 = 3 * 24 * 60 * 60 * 1_000;
    let mut etat = EtatRelance::neuve();
    for _ in 0..6 {
        etat.tentative_lancee();
    }
    assert_eq!(etat.espacement_ms(), REPLI_MAX_MS, "six refus : le repli est au PLAFOND");

    // Septième lancement — celui-là tient, et longtemps.
    etat.tentative_lancee();
    assert!(etat.stable(TROIS_JOURS_MS), "trois jours de vie, c'est stable");

    // Puis la coupure réseau : `pont::executer` rend une `Err`, donc un code
    // de sortie non nul, donc `IssueDeSortie::Erreur` — que
    // `reinitialiser_le_repli` refuse à bon droit de tenir pour une preuve.
    etat.reinitialiser_le_repli(IssueDeSortie::Erreur);
    assert_eq!(
        etat.espacement_ms(),
        ESPACEMENT_PLANCHER_MS,
        "après trois jours de service, une coupure réseau doit retomber à \
         l'espacement PLANCHER et non rester au plafond de {REPLI_MAX_MS} ms \
         — la RAMPE de l'épisode de refus suivant, pas la première reprise \
         (immédiate dans les deux cas) : c'est la citation que \
         `reinitialiser_le_repli` porte, et que seule `stable` peut tenir \
         pour une mort EN ERREUR"
    );
    assert!(etat.doit_relancer(ESPACEMENT_PLANCHER_MS));
}
