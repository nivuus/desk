# Sous-bloc D6 — partage de la capacité réseau entre N flux : plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Empêcher que N fenêtres visent chacune la capacité entière du même
lien, en donnant à chacune une part d'un budget de session arbitré par le
capteur, majorée pour la fenêtre que l'utilisateur regarde.

**Architecture:** Un module pur `agent/src/capteur/repartiteur.rs` calcule les
parts à partir de l'état des fenêtres. `capteur/sommeil.rs`, qui tient déjà le
verrou global du vivier de D5, l'appelle après chaque arbitrage et pousse les
parts changées sur le canal d'ordres existant. La part descend jusqu'à l'enfant
par `DepuisCapteur::Part` sur la connexion média, exactement comme
`DepuisCapteur::Sommeil` de D5. L'enfant l'applique en deux endroits :
`rtc.bwe().set_desired_bitrate` (qui arrête le sondage) et
`Controleur::changer_plafond` (qui borne la décision).

**Tech Stack:** Rust (agent), str0m (WebRTC), tubes nommés Windows,
`serde_json` sur le protocole capteur, `cargo test` sur l'hôte Linux,
`cargo check --target x86_64-pc-windows-gnu` pour le code `#[cfg(windows)]`.

## Global Constraints

- **Spec de référence** : `docs/superpowers/specs/2026-08-03-multifenetres-partage-capacite-design.md`.
  Toute divergence se signale, ne se décide pas seul.
- **Branche** : `chantier-multifenetres-d6`, déjà créée.
- **Plafond de taille de fichier : 500 lignes.** `agent/src/capteur/serveur.rs`
  est à **490** — ce plan ne le touche pas, et c'est délibéré (voir la doc de
  tête de `sommeil.rs`, qui en fait un objectif explicite). Vérifier par la
  commande du §8 de la spec avant chaque commit qui ajoute des lignes.
- **`git add` NOMINATIF, jamais `git add -A`** : l'arbre porte du travail non
  suivi (`src/`, `web/`, `assets/`, `Dockerfile`…) qui n'appartient pas à ce
  chantier, et un `git add -A` a déjà cassé un commit dans ce dépôt.
- **Chaque commit se termine par** `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`.
- **Sujets de commit sans accents** (convention du dépôt) ; le corps peut en
  porter.
- **Rien n'est déclaré fait sans la sortie de commande qui l'établit.**
- **Aucun nombre de ce plan ne se recopie dans un rapport sans être relevé à
  nouveau.**
- **Constantes neuves : `FACTEUR_FOCUS` et `PART_DORMANTE` sont NON CALIBRÉES**
  et leur documentation doit le dire, comme `BPP_MIN` et `HYSTERESIS` le font
  déjà.

## Structure des fichiers

| Fichier | Rôle | État |
| --- | --- | --- |
| `agent/src/capteur/repartiteur.rs` | **Neuf.** La règle de part, pure, sans `cfg`, sans COM, sans canal | Créé tâche 2 |
| `agent/src/capteur/repartiteur/tests.rs` | **Neuf.** Tests de la règle | Créé tâche 2 |
| `agent/src/capteur.rs` | Déclaration du module | Modifié tâche 2 |
| `agent/src/capteur/vivier.rs` (261) | Accesseur `eveillees()` | Modifié tâche 3 |
| `agent/src/capteur/sommeil.rs` (263) | `Message`, suivi du focus, distribution des parts | Modifié tâche 4 |
| `agent/src/capteur/fenetre.rs` (407) + `fenetre/transitions.rs` (159) | Consommer `Message::Part`, pousser `DepuisCapteur::Part` | Modifié tâche 5 |
| `agent/src/capteur/protocole.rs` (312) | `DepuisCapteur::Part` | Modifié tâche 5 |
| `agent/src/capteur/distante.rs` (235) + `distante/tests.rs` (385) | `Recu::Part`, `part_a_appliquer` | Modifié tâche 6 |
| `agent/src/source.rs` (295) | Méthode par défaut `part_a_appliquer` du trait `VideoSource` | Modifié tâche 6 |
| `agent/src/congestion/reconfiguration.rs` (142) | `Controleur::changer_plafond` | Modifié tâche 7 |
| `agent/src/transport/tick.rs` (212) | Branche `a1quater` : appliquer la part | Modifié tâche 8 |
| `agent/src/transport/adaptation.rs` (472, marge 28) | `Session::appliquer_part` | Modifié tâche 8 |
| `agent/src/capteur/serveur.rs` (490) | **NE PAS TOUCHER** | — |
| `scripts/run-agent.sh` (115) | Transmettre `BUDGET_BPS` | Modifié tâche 9 |

---

## Tâche 1 : La porte — mesurer le lien réel avant d'écrire une ligne

**Pourquoi elle est première.** D4 attribue au réseau l'écart 494,4 → 224,9 i/s
et le RTT 2 → 104 ms, mais **le déclare explicitement comme une inférence** :
« aucune mesure de charge du pont ». Tout ce plan repose dessus. Si le pont
porte plusieurs centaines de Mb/s, huit sondages visant 96 Mb/s ne l'ont pas
saturé et la cause était ailleurs.

**Files:**
- Create: `docs/superpowers/plans/journaux-multifenetres-d6/mesure-lien.log`
- Create: `docs/superpowers/plans/journaux-multifenetres-d6/mesure-lien-navigateur.json`
- Create: `docs/superpowers/plans/2026-08-03-multifenetres-partage-capacite-resultats.md` (section §1 seulement)

**Interfaces:**
- Consumes: rien.
- Produces: un nombre — la capacité du chemin VM → pont → navigateur hôte, en
  bits par seconde — et un **verdict sur la prémisse** que les tâches 2 à 11
  citeront.

- [ ] **Step 1: Démarrer la VM et attendre le partage de fichiers**

```bash
virsh list --all
virsh start Windows
until timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null; do sleep 5; done
until ls /media/vm/dev >/dev/null 2>&1; do sleep 5; done
```

`mountpoint -q` ne suffit pas : `/media/vm` est un montage CIFS dont l'entrée
persiste VM éteinte. Il faut éprouver un **accès réel**.

- [ ] **Step 2: Vérifier qu'aucun agent ne survit d'une exécution passée, et purger les sorties virtuelles**

```bash
set -a && source .env && set +a
node scripts/winrm.js 'Get-Process agent -ErrorAction SilentlyContinue | Format-List Id,StartTime | Out-String'
```

S'il en reste : `Stop-Process -Force`, puis purger, car les sorties virtuelles
**survivent** à une mort brutale (piège relevé en D5) :

```bash
MULTIFENETRE_VDD_PURGE=1 scripts/run-agent.sh
```

- [ ] **Step 3: Bâtir le binaire courant sur la VM**

```bash
set -a && source .env && set +a
scripts/build-agent.sh 2>&1 | tee /tmp/build-d6-t1.log
```

⚠️ Sans `.env` sourcé, ce script **s'arrête en silence** après « sources
synchronisées », et le symptôme se lit comme une compilation réussie. Vérifier
la **taille** du binaire dans la sortie ; une compilation de 0,13 s est un aveu
(`cargo clean --release -p agent` débloque).

- [ ] **Step 4: Lancer UNE fenêtre avec un plafond très haut, sur une source animée**

Une seule fenêtre, `BITRATE=100000000`, source animée à cadence connue
(`instrument/anim-d4.html` de D5, 90 Hz de rAF mesurés), avec un
`--user-data-dir` propre. Objectif : laisser le BWE monter librement et voir où
il bute.

- [ ] **Step 5: Relever, pendant au moins 60 s de régime établi, les quatre grandeurs**

| Grandeur | Où | Ce qu'elle dit |
| --- | --- | --- |
| Estimation BWE en régime | `agent.log`, lignes d'adaptation | Ce que l'agent croit du lien |
| Débit RTP réellement sortant | `agent.log` | Ce qu'il y met vraiment |
| `framesDecoded` / `framesDropped` | `getStats()` du navigateur | **Le témoin qui départage** |
| Charge CPU de l'hôte pendant la mesure | `top -b -n 3` sur l'hôte | L'autre cause possible |

Protocole obligatoire : pas de capture d'écran CDP pendant la mesure ; toute
évaluation CDP bornée ; copier `agent.log` **après la fin réelle**, pas à la
fin du pilote.

- [ ] **Step 6: Écrire le verdict, avec son nombre d'exécutions**

Créer `2026-08-03-multifenetres-partage-capacite-resultats.md` avec un §1 qui
répond **explicitement** à : *le pont porte-t-il assez pour que 8 × 12 Mb/s
l'aient saturé ?*

- **Si oui** (capacité mesurée ≲ 96 Mb/s, ou `framesDropped` négligeable et CPU
  hôte non saturée) → la prémisse tient, `BUDGET_BPS` par défaut se dérive de
  cette capacité avec marge, et le critère ① reste tel que la spec l'écrit.
- **Si non** (capacité très supérieure, ou CPU hôte saturée / `framesDropped`
  massif) → **l'écrire sans l'atténuer**. Le répartiteur reste juste sur le
  fond, mais le critère ① retombe sur *le sondage cumulé ne dépasse plus le
  budget*, fait mesurable côté agent (§6.4 de la spec). **Signaler à l'humain
  avant de continuer** : c'est une divergence de prémisse, pas de détail.

- [ ] **Step 7: Contrôler que la VM a survécu**

```bash
grep -E "terminating on signal|shutting down" /var/log/libvirt/qemu/Windows.log | tail -4
```

- [ ] **Step 8: Commit**

```bash
git add docs/superpowers/plans/journaux-multifenetres-d6/ \
        docs/superpowers/plans/2026-08-03-multifenetres-partage-capacite-resultats.md
git commit -m "mesure(d6): capacite reelle du lien, et verdict sur la premisse de D4

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 2 : `repartiteur.rs` — la règle de part, pure et testée à froid

**Files:**
- Create: `agent/src/capteur/repartiteur.rs`
- Create: `agent/src/capteur/repartiteur/tests.rs`
- Modify: `agent/src/capteur.rs` (déclaration du module)

**Interfaces:**
- Consumes: rien.
- Produces:
  - `pub const FACTEUR_FOCUS: u32 = 2;`
  - `pub const PART_DORMANTE_BPS: u32 = 256_000;`
  - `pub struct Fenetre { pub session: String, pub eveillee: bool, pub focalisee: bool }`
  - `pub fn repartir(budget_bps: u32, fenetres: &[Fenetre]) -> Vec<(String, u32)>`

- [ ] **Step 1: Écrire les tests qui échouent**

Créer `agent/src/capteur/repartiteur/tests.rs` :

```rust
use super::*;

fn f(session: &str, eveillee: bool, focalisee: bool) -> Fenetre {
    Fenetre { session: session.to_string(), eveillee, focalisee }
}

fn part_de(parts: &[(String, u32)], session: &str) -> u32 {
    parts.iter().find(|(s, _)| s == session).map(|(_, bps)| *bps).expect("session absente")
}

#[test]
fn sans_aucune_fenetre_il_n_y_a_rien_a_repartir() {
    assert!(repartir(12_000_000, &[]).is_empty());
}

#[test]
fn une_seule_eveillee_recoit_tout_le_budget() {
    let parts = repartir(12_000_000, &[f("a", true, true)]);
    assert_eq!(part_de(&parts, "a"), 12_000_000);
}

#[test]
fn sans_focalisee_les_eveillees_se_partagent_a_parts_egales() {
    let parts = repartir(12_000_000, &[f("a", true, false), f("b", true, false)]);
    assert_eq!(part_de(&parts, "a"), 6_000_000);
    assert_eq!(part_de(&parts, "b"), 6_000_000);
}

#[test]
fn la_focalisee_recoit_le_facteur_de_majoration() {
    // 2 éveillées, dont une focalisée : diviseur = 1 + FACTEUR_FOCUS = 3.
    let parts = repartir(12_000_000, &[f("a", true, true), f("b", true, false)]);
    assert_eq!(part_de(&parts, "b"), 4_000_000);
    assert_eq!(part_de(&parts, "a"), 8_000_000);
    assert!(part_de(&parts, "a") > part_de(&parts, "b"), "la focalisée doit recevoir plus");
}

#[test]
fn une_endormie_recoit_le_plancher_et_ne_partage_pas_le_reste() {
    let parts = repartir(12_000_000, &[f("a", true, true), f("b", false, false)]);
    assert_eq!(part_de(&parts, "b"), PART_DORMANTE_BPS);
    assert_eq!(
        part_de(&parts, "a"),
        12_000_000 - PART_DORMANTE_BPS,
        "l'éveillée seule prend tout le reste"
    );
}

#[test]
fn toutes_endormies_recoivent_le_plancher_et_le_reste_n_est_donne_a_personne() {
    let parts = repartir(12_000_000, &[f("a", false, false), f("b", false, false)]);
    assert_eq!(part_de(&parts, "a"), PART_DORMANTE_BPS);
    assert_eq!(part_de(&parts, "b"), PART_DORMANTE_BPS);
}

/// Le cas limite que le produit rend atteignable : le client annonce le focus
/// sur une fenêtre que le vivier a ÉVINCÉE. Elle est endormie, donc au
/// plancher, et la majoration ne revient à personne.
#[test]
fn une_focalisee_endormie_reste_au_plancher_et_les_eveillees_se_partagent_egalement() {
    let parts =
        repartir(12_000_000, &[f("dormeuse", false, true), f("a", true, false), f("b", true, false)]);
    assert_eq!(part_de(&parts, "dormeuse"), PART_DORMANTE_BPS);
    let reste = 12_000_000 - PART_DORMANTE_BPS;
    assert_eq!(part_de(&parts, "a"), reste / 2);
    assert_eq!(part_de(&parts, "b"), reste / 2);
}

/// Plusieurs focalisées ne peuvent pas se produire durablement (le client
/// émet `blur`), mais la fonction doit rester TOTALE : une seule majoration
/// est accordée, à la première rencontrée, sinon l'invariant de budget saute.
#[test]
fn plusieurs_focalisees_ne_donnent_qu_une_seule_majoration() {
    let parts = repartir(12_000_000, &[f("a", true, true), f("b", true, true)]);
    let somme: u32 = parts.iter().map(|(_, bps)| bps).sum();
    assert!(somme <= 12_000_000, "somme {somme} au-dessus du budget");
}

/// **L'invariant central.** Il vaut à tous les rangs que le produit permet :
/// `vivier::PLAFOND_EVEIL` vaut 8, `superviseur::CAPACITE` vaut 10.
#[test]
fn la_somme_des_parts_ne_depasse_jamais_le_budget() {
    for eveillees in 0..=8usize {
        for endormies in 0..=(10 - eveillees) {
            let mut fenetres = Vec::new();
            for i in 0..eveillees {
                fenetres.push(f(&format!("e{i}"), true, i == 0));
            }
            for i in 0..endormies {
                fenetres.push(f(&format!("d{i}"), false, false));
            }
            let parts = repartir(12_000_000, &fenetres);
            let somme: u32 = parts.iter().map(|(_, bps)| bps).sum();
            assert!(
                somme <= 12_000_000,
                "{eveillees} éveillées et {endormies} endormies : somme {somme}"
            );
            assert_eq!(parts.len(), fenetres.len(), "chaque fenêtre doit recevoir une part");
            assert!(parts.iter().all(|(_, bps)| *bps > 0), "aucune part ne doit être nulle");
        }
    }
}

/// Un budget trop petit pour payer les planchers ne doit ni déborder ni
/// paniquer par soustraction en dessous de zéro.
#[test]
fn un_budget_inferieur_aux_planchers_ne_deborde_pas() {
    let fenetres: Vec<Fenetre> = (0..8).map(|i| f(&format!("d{i}"), false, false)).collect();
    let parts = repartir(100_000, &fenetres);
    assert!(parts.iter().all(|(_, bps)| *bps > 0));
}
```

- [ ] **Step 2: Lancer les tests pour vérifier qu'ils échouent**

```bash
cd agent && cargo test --lib repartiteur 2>&1 | tail -20
```

Attendu : ÉCHEC de compilation, `unresolved module or unlinked crate
'repartiteur'` ou `cannot find function 'repartir'`.

- [ ] **Step 3: Écrire l'implémentation minimale**

Créer `agent/src/capteur/repartiteur.rs` :

```rust
//! La règle de part : qui reçoit quelle fraction du budget de session.
//!
//! **Pas de `#[cfg(windows)]`, aucun objet COM, aucun canal.** Ce module ne
//! fait que décider ; l'application vit dans `capteur/sommeil.rs` et
//! `capteur/fenetre.rs`. C'est le patron posé par D4 pour `capteur/protocole.rs`
//! et par D5 pour `capteur/vivier.rs` : ce qui décide se teste sur l'hôte.
//!
//! **Le défaut que ce module existe pour corriger.** Depuis le sous-bloc D1,
//! chaque fenêtre est un processus portant sa propre `PeerConnection`, donc son
//! propre BWE, et chacune hérite `BITRATE` tel quel. À huit fenêtres, huit
//! `set_desired_bitrate` visent 96 Mb/s cumulés sur un lien unique, et le
//! sondage à la hausse de chacune est lu par les autres comme de la congestion.

/// Majoration accordée à la fenêtre que l'utilisateur regarde.
///
/// ⚠️ **NON CALIBRÉE.** Le raisonnement qui la fonde n'est pas une mesure :
/// deux barreaux voisins de l'échelle sont dans un rapport de pixels de
/// 1,25² ≈ 1,56 (`DIVISEURS` de `congestion/echelle.rs`), donc un facteur 2
/// garantit plus d'un barreau d'écart en faveur de la fenêtre regardée. C'est
/// le critère ② de la recette qui la jugera, pas cette intuition.
pub const FACTEUR_FOCUS: u32 = 2;

/// Part laissée à une fenêtre endormie.
///
/// **Jamais zéro** : elle n'encode plus rien (D5 a relâché son encodeur) mais
/// sa `PeerConnection` vit, et `set_desired_bitrate(0)` n'est pas un réglage
/// que str0m est censé recevoir.
///
/// ⚠️ **NON CALIBRÉE**, et l'hypothèse qui la motive n'est pas vérifiée : on
/// ignore si str0m émet réellement du bourrage de sondage quand aucun média ne
/// part. Si oui, ce plancher empêche des fenêtres endormies de manger le lien
/// pour rien ; si non, il ne coûte que sa ligne. L'ordre de grandeur couvre
/// l'audio (`opus::BITRATE_BPS`, 128 kb/s) et laisse de la marge.
pub const PART_DORMANTE_BPS: u32 = 256_000;

/// L'état d'une fenêtre, tel que le répartiteur a besoin de le connaître.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fenetre {
    pub session: String,
    pub eveillee: bool,
    pub focalisee: bool,
}

/// Découpe `budget_bps` entre les fenêtres.
///
/// Chaque endormie reçoit `PART_DORMANTE_BPS` ; le reste se divise entre les
/// éveillées, la focalisée recevant `FACTEUR_FOCUS` parts au lieu d'une.
///
/// **Invariant** : la somme des parts ne dépasse jamais le budget, et aucune
/// part n'est nulle. **Fonction totale** : plusieurs focalisées, aucune
/// éveillée, ou un budget inférieur aux planchers ne la font ni déborder ni
/// paniquer.
///
/// **Aucun travail conservateur** : une fenêtre qui n'use pas sa part ne la
/// rend pas aux autres. Ce serait une seconde boucle de rétroaction dont la
/// stabilité devrait être éprouvée — hors périmètre de D6.
pub fn repartir(budget_bps: u32, fenetres: &[Fenetre]) -> Vec<(String, u32)> {
    let endormies = fenetres.iter().filter(|f| !f.eveillee).count() as u32;
    let eveillees = fenetres.iter().filter(|f| f.eveillee).count() as u32;

    // `saturating_sub` : un budget inférieur au total des planchers rend un
    // reste nul, jamais un débordement. Les endormies gardent alors leur
    // plancher et les éveillées reçoivent le minimum d'une part, ce qui fait
    // franchir le budget — cas dégénéré assumé, mais il ne panique pas.
    let reste = budget_bps.saturating_sub(endormies.saturating_mul(PART_DORMANTE_BPS));

    // Une seule majoration, à la PREMIÈRE focalisée éveillée rencontrée :
    // plusieurs focalisées ne durent pas (le client émet `blur`), mais en
    // accorder deux ferait sauter l'invariant de budget.
    let indice_focalisee =
        fenetres.iter().position(|f| f.eveillee && f.focalisee);

    let diviseur = match indice_focalisee {
        Some(_) => eveillees.saturating_sub(1) + FACTEUR_FOCUS,
        None => eveillees,
    };
    // `max(1)` : sans éveillée, le diviseur vaut 0 et la division paniquerait.
    // La valeur ne sert alors à personne — aucune fenêtre n'est éveillée.
    let part_base = reste / diviseur.max(1);

    fenetres
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let bps = if !f.eveillee {
                PART_DORMANTE_BPS
            } else if Some(i) == indice_focalisee {
                part_base.saturating_mul(FACTEUR_FOCUS)
            } else {
                part_base
            };
            // Aucune part nulle : un budget dérisoire ne doit pas produire un
            // `set_desired_bitrate(0)`.
            (f.session.clone(), bps.max(1))
        })
        .collect()
}

#[cfg(test)]
mod tests;
```

Ajouter dans `agent/src/capteur.rs`, auprès des autres déclarations de module :

```rust
pub mod repartiteur;
```

- [ ] **Step 4: Lancer les tests pour vérifier qu'ils passent**

```bash
cd agent && cargo test --lib repartiteur 2>&1 | tail -20
```

Attendu : `test result: ok.` avec 10 tests.

- [ ] **Step 5: Vérifier la taille des fichiers neufs**

```bash
wc -l agent/src/capteur/repartiteur.rs agent/src/capteur/repartiteur/tests.rs
```

Attendu : les deux sous 500. Si `tests.rs` approche, l'extraction est déjà
faite par construction (fichier séparé).

- [ ] **Step 6: Commit**

```bash
git add agent/src/capteur/repartiteur.rs agent/src/capteur/repartiteur/tests.rs agent/src/capteur.rs
git commit -m "feat(d6): la regle de part, pure et testee a froid

La somme des parts ne depasse jamais le budget, aucune part n'est nulle,
et la fonction reste totale sur les cas degeneres : plusieurs focalisees,
aucune eveillee, budget inferieur aux planchers.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 3 : `Vivier::eveillees` — l'accesseur dont le répartiteur a besoin

**Files:**
- Modify: `agent/src/capteur/vivier.rs` (261 lignes)
- Modify: `agent/src/capteur/vivier/tests.rs` (273 lignes)

**Interfaces:**
- Consumes: rien.
- Produces: `pub fn eveillees(&self) -> Vec<String>` sur `Vivier` — la liste
  des sessions actuellement éveillées, dans un ordre non spécifié.

**Pourquoi cet accesseur et pas un champ `focalisee` dans `Entree`.** Le
`Vivier` reçoit bien `focalisee` dans `signaler`, mais ne s'en sert que pour
rafraîchir `dernier_vu` et ne le retient pas. Le retenir mélangerait deux
responsabilités : le vivier arbitre des **places d'encodeur**, le répartiteur
des **parts de débit**. Le focus courant sera donc tenu par `sommeil::Etat`
(tâche 4), à côté des canaux qui doivent recevoir les parts.

- [ ] **Step 1: Écrire le test qui échoue**

Ajouter à la fin de `agent/src/capteur/vivier/tests.rs` :

```rust
#[test]
fn eveillees_rend_exactement_les_sessions_reveillees() {
    let base = t0();
    let mut vivier = Vivier::nouveau(2, Duration::from_secs(2));
    vivier.inscrire("a", base);
    vivier.inscrire("b", base);
    assert!(vivier.eveillees().is_empty(), "une fenêtre naît endormie");

    vivier.signaler("a", true, true, base);
    assert_eq!(vivier.eveillees(), vec!["a".to_string()]);

    vivier.signaler("b", true, false, base);
    let mut eveillees = vivier.eveillees();
    eveillees.sort();
    assert_eq!(eveillees, vec!["a".to_string(), "b".to_string()]);

    vivier.retirer("a", base);
    assert_eq!(vivier.eveillees(), vec!["b".to_string()]);
}
```

⚠️ Vérifier d'abord comment `vivier/tests.rs` construit son horloge de test et
importe `Vivier` (il existe un helper `t0()` dans `congestion/hysteresis.rs` ;
`vivier/tests.rs` a le sien). **Reprendre le helper du fichier, ne pas en
inventer un.**

- [ ] **Step 2: Lancer le test pour vérifier qu'il échoue**

```bash
cd agent && cargo test --lib vivier::tests::eveillees 2>&1 | tail -15
```

Attendu : ÉCHEC, `no method named 'eveillees' found for struct 'Vivier'`.

- [ ] **Step 3: Écrire l'implémentation minimale**

Dans `agent/src/capteur/vivier.rs`, dans `impl Vivier`, auprès des autres
méthodes publiques :

```rust
    /// Les sessions actuellement éveillées, dans un ordre non spécifié.
    ///
    /// Lu par `sommeil.rs` pour alimenter le répartiteur de débit (D6) : la
    /// part d'une fenêtre dépend de son éveil, et le vivier est la seule
    /// source de vérité sur ce point.
    pub fn eveillees(&self) -> Vec<String> {
        self.entrees
            .iter()
            .filter(|(_, entree)| entree.eveillee)
            .map(|(session, _)| session.clone())
            .collect()
    }
```

- [ ] **Step 4: Lancer les tests pour vérifier qu'ils passent**

```bash
cd agent && cargo test --lib vivier 2>&1 | tail -15
```

Attendu : `test result: ok.`, aucun test existant cassé.

- [ ] **Step 5: Commit**

```bash
git add agent/src/capteur/vivier.rs agent/src/capteur/vivier/tests.rs
git commit -m "feat(d6): le vivier dit qui est eveillee

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 4 : `sommeil.rs` — brancher le répartiteur sur le verrou existant

**Files:**
- Modify: `agent/src/capteur/sommeil.rs` (263 lignes)

**Interfaces:**
- Consumes: `repartiteur::{repartir, Fenetre}`, `Vivier::eveillees`.
- Produces:
  - `pub enum Message { Sommeil(Ordre), Part { bps: u32 } }`
  - `pub fn inscrire(session: &str) -> Receiver<Message>` (**signature changée**,
    l'ancienne rendait `Receiver<Ordre>`)
  - `fn budget_bps() -> u32` — **privé au module** : lit `BUDGET_BPS` une fois,
    replie sur 12 000 000. Personne d'autre n'a besoin du budget, seul le
    répartiteur le consomme.

**Le chemin dégradé « enfant pas encore attaché » (§5 de la spec) est tenu par
l'ordre des appels dans `Fenetre::servir`**, et il faut le vérifier plutôt que
le supposer : `servir` crée le canal d'écritures et lance son fil écrivain
**avant** d'appeler `sommeil::inscrire`. Comme `inscrire` se termine désormais
par `distribuer_les_parts`, la toute première part part à l'inscription, avant
la moindre image. La fenêtre d'exposition à `BITRATE` hérité est donc ramenée à
la durée d'un tour de boucle.

**Le point qui compte le plus dans cette tâche.** Le tour de roue de D5
(`PERIODE_REARBITRAGE`, 250 ms) rappelle l'arbitrage quatre fois par seconde.
Sans détection de changement, huit fenêtres recevraient 32 messages `Part` par
seconde à vie. Les parts se diffusent **au changement seulement**, exactement
comme `Etat` et `Sommeil`.

- [ ] **Step 1: Écrire les tests qui échouent**

Ajouter dans le `mod tests` de `agent/src/capteur/sommeil.rs` (il existe déjà,
avec son `VERROU_TESTS` — **le reprendre**, sans quoi les tests se privent
mutuellement des places du vivier partagé) :

```rust
    #[test]
    fn une_session_qui_s_eveille_recoit_une_part_apres_son_ordre_de_reveil() {
        let _verrou = verrouiller_pour_le_test();
        let messages = inscrire("t6-a");
        signaler("t6-a", true, true);

        let recus: Vec<Message> = messages.try_iter().collect();
        let position_reveil = recus
            .iter()
            .position(|m| matches!(m, Message::Sommeil(Ordre::Reveiller)))
            .expect("l'ordre de réveil doit être présent");
        let position_part = recus
            .iter()
            .position(|m| matches!(m, Message::Part { .. }))
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
        let messages = inscrire("t6-b");
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
        let a = inscrire("t6-c");
        signaler("t6-c", true, true);
        let premiere = derniere_part(&a).expect("la première doit avoir une part");

        let b = inscrire("t6-d");
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
```

- [ ] **Step 2: Lancer les tests pour vérifier qu'ils échouent**

```bash
cd agent && cargo test --lib sommeil 2>&1 | tail -20
```

Attendu : ÉCHEC de compilation, `cannot find type 'Message'`.

- [ ] **Step 3: Écrire l'implémentation**

Dans `agent/src/capteur/sommeil.rs` :

```rust
use crate::capteur::repartiteur::{self, Fenetre};

/// Budget de débit de la session entière, en bits par seconde.
///
/// **De session, pas par fenêtre** — c'est tout le sujet du sous-bloc D6.
/// Lu une seule fois : le changer en cours de vie n'aurait aucun sens tant
/// que le lien ne change pas.
///
/// ⚠️ Le repli de 12 Mb/s est l'ancien `BITRATE` par fenêtre, repris faute de
/// mieux ; la valeur réelle se dérive de la mesure de la tâche 1.
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

/// Ce qu'une fenêtre reçoit du registre global.
///
/// **Un seul canal pour les deux**, et non deux canaux parallèles : l'ordre
/// entre un endormissement et la part qui en découle est ainsi garanti par
/// construction. Une fenêtre qui recevrait sa part d'endormie avant l'ordre de
/// dormir serait momentanément décrite comme endormie alors qu'elle encode
/// encore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Sommeil(Ordre),
    Part { bps: u32 },
}
```

`Etat` gagne deux champs, et `canaux` change de type :

```rust
struct Etat {
    vivier: Vivier,
    canaux: HashMap<String, Sender<Message>>,
    /// La session que le client déclare focalisée, si elle existe encore.
    ///
    /// Tenue ici et non dans `Vivier` : le vivier arbitre des places
    /// d'encodeur, le répartiteur des parts de débit. Le client émet `blur`
    /// aussi bien que `focus` (`client/src/visibilite.ts`), donc ce champ se
    /// vide bien quand la fenêtre perd le focus.
    focalisee: Option<String>,
    /// Dernière part envoyée à chaque session. **Le seul rempart contre une
    /// inondation** : le tour de roue ré-arbitre toutes les 250 ms, et sans
    /// cette mémoire huit fenêtres recevraient 32 messages par seconde à vie.
    dernieres_parts: HashMap<String, u32>,
}
```

`distribuer` enveloppe désormais les ordres, sans autre changement de logique :

```rust
            let rompu = match garde.canaux.get(&session) {
                Some(canal) => canal.send(Message::Sommeil(ordre)).is_err(),
                None => false,
            };
```

Puis une fonction neuve, appelée **après** `distribuer` dans chacun des cinq
points d'entrée (`inscrire`, `retirer`, `signaler`, `echec_de_reveil`, et le
tour de roue) :

```rust
/// Recalcule les parts et n'envoie que celles qui ont changé.
///
/// **Appelée APRÈS `distribuer`**, jamais avant : les ordres de sommeil
/// changent l'éveil, et une part calculée avant eux décrirait l'état
/// précédent.
///
/// Une session dont le canal est rompu est retirée, comme dans `distribuer` —
/// mais sans boucle de reprise : contrairement à un ordre de sommeil, une part
/// perdue n'engendre aucun ordre supplémentaire, et la prochaine passe la
/// rattrapera.
fn distribuer_les_parts(garde: &mut MutexGuard<'static, Etat>) {
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
    for session in rompus {
        garde.canaux.remove(&session);
        garde.dernieres_parts.remove(&session);
    }
}
```

`signaler` retient le focus avant d'arbitrer :

```rust
pub fn signaler(session: &str, visible: bool, focalisee: bool) {
    let mut garde = etat();
    if focalisee {
        garde.focalisee = Some(session.to_string());
    } else if garde.focalisee.as_deref() == Some(session) {
        garde.focalisee = None;
    }
    let ordres = garde.vivier.signaler(session, visible, focalisee, Instant::now());
    distribuer(&mut garde, ordres);
    distribuer_les_parts(&mut garde);
}
```

`retirer` oublie le focus de la session partie :

```rust
pub fn retirer(session: &str) {
    let mut garde = etat();
    garde.canaux.remove(session);
    if garde.focalisee.as_deref() == Some(session) {
        garde.focalisee = None;
    }
    let ordres = garde.vivier.retirer(session, Instant::now());
    distribuer(&mut garde, ordres);
    distribuer_les_parts(&mut garde);
}
```

`inscrire`, `echec_de_reveil` et le tour de roue reçoivent chacun l'appel
`distribuer_les_parts(&mut garde);` après leur `distribuer`. L'initialisation
de `Etat` dans `etat()` gagne `focalisee: None` et
`dernieres_parts: HashMap::new()`.

- [ ] **Step 4: Lancer les tests pour vérifier qu'ils passent**

```bash
cd agent && cargo test --lib sommeil 2>&1 | tail -20
```

Attendu : `test result: ok.` — les tests de D5 **et** les trois neufs.

⚠️ `fenetre/transitions.rs` ne compile plus (le type du `Receiver` a changé).
C'est attendu : la tâche 5 le répare. Pour valider **cette** tâche isolément,
`cargo test --lib sommeil` suffit si le module fautif est `#[cfg(windows)]` ;
sinon, enchaîner directement sur la tâche 5 avant de commiter.

- [ ] **Step 5: Vérifier la taille du fichier**

```bash
wc -l agent/src/capteur/sommeil.rs
```

Attendu : sous 500. S'il approche, extraire `distribuer_les_parts` et ses tests
vers `agent/src/capteur/sommeil/parts.rs` — **une extraction, jamais une
compression**.

- [ ] **Step 6: Commit**

```bash
git add agent/src/capteur/sommeil.rs
git commit -m "feat(d6): le registre repartit le budget et ne reemet que les changements

Le tour de roue rearbitre toutes les 250 ms : sans memoire des dernieres
parts, huit fenetres recevraient 32 messages par seconde a vie.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 5 : le protocole et le fil de fenêtre — pousser `Part` vers l'enfant

**Files:**
- Modify: `agent/src/capteur/protocole.rs` (312 lignes)
- Modify: `agent/src/capteur/fenetre.rs` (407 lignes) — signature de `boucler`
- Modify: `agent/src/capteur/fenetre/transitions.rs` (159 lignes)

**Interfaces:**
- Consumes: `sommeil::Message`.
- Produces: `DepuisCapteur::Part { bps: u32 }` sur la connexion média.

- [ ] **Step 1: Écrire le test qui échoue**

Dans le `mod tests` de `agent/src/capteur/protocole.rs`, à côté du test
existant qui fait l'aller-retour sur `DepuisCapteur::Taille` :

```rust
    #[test]
    fn une_part_traverse_l_encodage_json() {
        let mut tampon = Vec::new();
        ecrire_json(&mut tampon, &DepuisCapteur::Part { bps: 4_000_000 }).unwrap();
        let mut lecture = &tampon[..];
        let Trame::Json(corps) = lire_trame(&mut lecture).unwrap() else {
            panic!("une trame JSON était attendue");
        };
        let message: DepuisCapteur = serde_json::from_slice(&corps).unwrap();
        assert_eq!(message, DepuisCapteur::Part { bps: 4_000_000 });
    }
```

⚠️ **Reprendre les noms exacts** des fonctions de lecture/écriture du fichier
(`ecrire_json`, et la fonction de lecture de trame telle qu'elle s'appelle
réellement) : les lire avant d'écrire le test, ne pas les deviner.

- [ ] **Step 2: Lancer le test pour vérifier qu'il échoue**

```bash
cd agent && cargo test --lib protocole 2>&1 | tail -15
```

Attendu : ÉCHEC, `no variant named 'Part' found for enum 'DepuisCapteur'`.

- [ ] **Step 3: Ajouter la variante**

Dans `agent/src/capteur/protocole.rs`, à la fin de `enum DepuisCapteur` :

```rust
    /// Part du budget de débit de session accordée à cette fenêtre, poussée
    /// non sollicitée quand elle CHANGE.
    ///
    /// Distincte d'`Etat` pour la même raison que `Sommeil` : `Etat` alimente
    /// un cache lu à chaque tour de la boucle de transport, et y mêler une
    /// annonce ponctuelle passerait par un chemin conçu pour un état permanent.
    ///
    /// L'enfant l'applique en DEUX endroits, et le second est le plus
    /// important : `Controleur::changer_plafond` borne ce que l'encodeur
    /// produit, mais c'est `rtc.bwe().set_desired_bitrate` qui arrête le
    /// sondage à la hausse — la vraie cause de la congestion à N fenêtres.
    Part { bps: u32 },
```

- [ ] **Step 4: Câbler le fil de fenêtre**

Dans `agent/src/capteur/fenetre.rs`, remplacer l'import et la signature :

```rust
use crate::capteur::sommeil::Message;
```

```rust
    fn boucler(
        &mut self,
        session: &str,
        ordres: &Receiver<Message>,
        ecritures: &SyncSender<AEcrire>,
        commandes: &Receiver<VersCapteur>,
        reponses: &Sender<DepuisCapteur>,
    ) -> Result<()> {
```

Dans `agent/src/capteur/fenetre/transitions.rs`, `appliquer_les_ordres` prend
`&Receiver<Message>` et enveloppe ses deux bras existants, plus le neuf :

```rust
        loop {
            match ordres.try_recv() {
                Ok(Message::Sommeil(Ordre::Dormir(raison))) => {
                    // … corps inchangé …
                }
                Ok(Message::Sommeil(Ordre::Reveiller)) => {
                    // … corps inchangé …
                }
                Ok(Message::Part { bps }) => {
                    // Rien à faire localement : le capteur ne règle PAS son
                    // encodeur sur cette part. C'est l'enfant qui décide de
                    // son débit d'encodage (il a le BWE), et la part n'est
                    // qu'une borne qu'on lui transmet. Le capteur n'est ici
                    // que le facteur.
                    let message = DepuisCapteur::Part { bps };
                    if let Fin::Terminer(motif) =
                        deposer(AEcrire::Etat(message), ecritures, self.source.as_mut(), ctx)
                    {
                        return Fin::Terminer(motif);
                    }
                }
                // … bras d'erreur inchangés …
            }
        }
```

⚠️ `AEcrire::Etat` porte un `DepuisCapteur` quelconque malgré son nom — c'est
déjà par lui que `Sommeil` transite. Ne pas ajouter de variante.

- [ ] **Step 5: Lancer les tests et la compilation croisée**

```bash
cd agent && cargo test --lib 2>&1 | tail -20
cd agent && cargo check --target x86_64-pc-windows-gnu 2>&1 | tail -25
```

Attendu : tests `ok.`, et `cargo check` en sortie 0. **Vérifier la NATURE des
avertissements, jamais leur nombre** (il dérive selon la fraîcheur du build) :
aucun ne doit viser les symboles neufs.

- [ ] **Step 6: Commit**

```bash
git add agent/src/capteur/protocole.rs agent/src/capteur/fenetre.rs agent/src/capteur/fenetre/transitions.rs
git commit -m "feat(d6): le capteur pousse la part sur la connexion media

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 6 : `SourceDistante` — recevoir la part et la tenir jusqu'à lecture

**Files:**
- Modify: `agent/src/capteur/distante.rs` (235 lignes)
- Modify: `agent/src/capteur/distante/tests.rs` (385 lignes)
- Modify: `agent/src/source.rs` (295 lignes)

**Interfaces:**
- Consumes: `DepuisCapteur::Part`.
- Produces:
  - `Recu::Part { bps: u32 }`
  - `VideoSource::part_a_appliquer(&mut self) -> Option<u32>` (défaut `None`)

- [ ] **Step 1: Écrire les tests qui échouent**

Dans `agent/src/capteur/distante/tests.rs`, à côté des tests de `Sommeil`
(qui montrent le patron exact à suivre — **les lire d'abord**) :

```rust
/// Une part reçue est retenue jusqu'à ce que la boucle de transport la
/// consomme, et ne se rend qu'une fois.
#[test]
fn une_part_recue_est_rendue_une_seule_fois() {
    let (emetteur, source_recue) = channel();
    let mut source = SourceDistante::nouvelle(
        Box::new(CanalFactice::nouveau()),
        source_recue,
        1280,
        720,
    );
    emetteur.send(Recu::Part { bps: 4_000_000 }).unwrap();
    // `next_frame` est ce qui draine le canal : sans lui, rien n'est lu.
    assert_eq!(source.next_frame(), None);
    assert_eq!(source.part_a_appliquer(), Some(4_000_000));
    assert_eq!(source.part_a_appliquer(), None, "une part ne se réapplique pas");
}

/// Deux parts arrivées entre deux lectures s'écrasent : c'est un état
/// courant, pas un historique — même régime que `Etat` et `Sommeil`.
#[test]
fn deux_parts_arrivees_avant_lecture_s_ecrasent() {
    let (emetteur, source_recue) = channel();
    let mut source = SourceDistante::nouvelle(
        Box::new(CanalFactice::nouveau()),
        source_recue,
        1280,
        720,
    );
    emetteur.send(Recu::Part { bps: 4_000_000 }).unwrap();
    emetteur.send(Recu::Part { bps: 2_000_000 }).unwrap();
    assert_eq!(source.next_frame(), None);
    assert_eq!(source.part_a_appliquer(), Some(2_000_000), "seule la dernière survit");
}
```

⚠️ **Reprendre le nom exact du canal factice** du fichier (`CanalFactice` ou
autre) et sa construction : les lire, ne pas les deviner.

- [ ] **Step 2: Lancer les tests pour vérifier qu'ils échouent**

```bash
cd agent && cargo test --lib distante 2>&1 | tail -15
```

Attendu : ÉCHEC, `no variant named 'Part' found for enum 'Recu'`.

- [ ] **Step 3: Écrire l'implémentation**

Dans `agent/src/source.rs`, dans le trait `VideoSource`, à la suite de
`sommeil_a_annoncer` :

```rust
    /// Rend la part de budget de débit en attente d'application, et la
    /// consomme.
    ///
    /// **État courant, pas un historique** : deux parts arrivées entre deux
    /// lectures s'écrasent — même régime que `sommeil_a_annoncer` juste
    /// au-dessus. Comme elle consomme, la branche de transport qui
    /// l'interroge à chaque tour ne peut pas reconfigurer en boucle.
    ///
    /// Par défaut sans effet : une source fichier ne partage le lien avec
    /// personne. Seule `SourceDistante` la redéfinit.
    fn part_a_appliquer(&mut self) -> Option<u32> {
        None
    }
```

Dans `agent/src/capteur/distante.rs`, la variante et le champ :

```rust
pub enum Recu {
    Image(AccessUnit),
    Etat { vivante: bool, epuisee: bool, largeur: u32, hauteur: u32 },
    Sommeil { endormie: bool, raison: String },
    /// Part du budget de débit de session, poussée par le capteur au
    /// changement. Retenue par `SourceDistante::part` jusqu'à ce que
    /// `part_a_appliquer` la consomme.
    Part { bps: u32 },
}
```

```rust
    /// Dernière part reçue du capteur, en attente d'application. Consommée par
    /// `part_a_appliquer`. Même régime d'écrasement que `sommeil`.
    part: Option<u32>,
```

`nouvelle` initialise `part: None`. Dans `next_frame`, à côté du bras
`Recu::Sommeil` :

```rust
                Ok(Recu::Part { bps }) => {
                    self.part = Some(bps);
                }
```

Et dans `impl VideoSource for SourceDistante`, à côté de `sommeil_a_annoncer` :

```rust
    /// Rend la part en attente, et la consomme.
    fn part_a_appliquer(&mut self) -> Option<u32> {
        self.part.take()
    }
```

⚠️ Il faut aussi que le **lecteur du tube** traduise
`DepuisCapteur::Part { bps }` en `Recu::Part { bps }`. Chercher où
`DepuisCapteur::Sommeil` est converti en `Recu::Sommeil` — c'est dans le
lecteur de `capteur/tube.rs` — et ajouter le bras symétrique **au même
endroit**.

```bash
grep -rn "Recu::Sommeil" agent/src --include='*.rs'
```

- [ ] **Step 4: Lancer les tests pour vérifier qu'ils passent**

```bash
cd agent && cargo test --lib distante 2>&1 | tail -15
cd agent && cargo check --target x86_64-pc-windows-gnu 2>&1 | tail -20
```

- [ ] **Step 5: Vérifier les tailles**

```bash
wc -l agent/src/capteur/distante.rs agent/src/capteur/distante/tests.rs agent/src/source.rs agent/src/capteur/tube.rs
```

Attendu : tous sous 500.

- [ ] **Step 6: Commit**

```bash
git add agent/src/capteur/distante.rs agent/src/capteur/distante/tests.rs \
        agent/src/source.rs agent/src/capteur/tube.rs
git commit -m "feat(d6): la source distante retient la part jusqu'a son application

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 7 : `Controleur::changer_plafond` — borner la décision sans toucher l'échelle

**Files:**
- Modify: `agent/src/congestion/reconfiguration.rs` (142 lignes)

**Interfaces:**
- Consumes: rien.
- Produces: `pub fn changer_plafond(&mut self, plafond_bps: u32) -> Decision`
  sur `Controleur`.

**Pourquoi elle ne reconstruit rien.** `Echelle::depuis(source, fps)` ne dépend
que de la taille source et de la cadence, **jamais du plafond**. Reconstruire
l'échelle ici perdrait le barreau courant pour rien.

- [ ] **Step 1: Écrire les tests qui échouent**

Dans le `mod tests` de `agent/src/congestion/reconfiguration.rs`, à côté des
tests de `changer_source` :

```rust
    #[test]
    fn changer_plafond_borne_la_decision_sans_toucher_l_echelle() {
        let base = t0();
        let mut c = Controleur::new(config(), base);
        let barreaux_avant = c.echelle.barreaux().to_vec();

        let decision = c.changer_plafond(3_000_000);
        assert_eq!(decision.video_bitrate_bps, 3_000_000, "le débit suit le nouveau plafond");
        assert_eq!(
            c.echelle.barreaux(),
            barreaux_avant.as_slice(),
            "l'échelle ne dépend pas du plafond"
        );
    }

    #[test]
    fn un_plafond_qui_remonte_ne_depasse_pas_l_estimation_courante() {
        let base = t0();
        let mut c = Controleur::new(config(), base);
        // Une estimation modeste, puis un plafond très haut : c'est
        // l'estimation qui doit continuer de commander.
        c.observer(super::super::Observation {
            estimate_bps: Some(2_000_000),
            rtt: None,
            loss: None,
            at: base + Duration::from_secs(1),
        });
        let decision = c.changer_plafond(50_000_000);
        assert!(
            decision.video_bitrate_bps <= 2_000_000,
            "le plafond ne doit jamais faire dépasser l'estimation : {}",
            decision.video_bitrate_bps
        );
    }
```

⚠️ **Lire d'abord** `controleur.rs` et `reconfiguration.rs` pour reprendre les
helpers réels (`config()`, `t0()`) et les champs réellement accessibles depuis
ce module frère (`pub(super)`).

- [ ] **Step 2: Lancer les tests pour vérifier qu'ils échouent**

```bash
cd agent && cargo test --lib congestion 2>&1 | tail -15
```

Attendu : ÉCHEC, `no method named 'changer_plafond'`.

- [ ] **Step 3: Écrire l'implémentation**

Dans `agent/src/congestion/reconfiguration.rs`, dans le même `impl Controleur` :

```rust
    /// Change la borne haute de débit, sans toucher à l'échelle.
    ///
    /// Appelée quand le capteur accorde une nouvelle part du budget de
    /// session (sous-bloc D6). **Ne reconstruit rien** : `Echelle::depuis` ne
    /// dépend que de la taille source et de la cadence, jamais du plafond —
    /// contrairement à `changer_source` juste au-dessus, qui doit reporter le
    /// barreau sur une échelle neuve.
    ///
    /// Le débit rendu reste borné par la dernière estimation de bande
    /// passante : un plafond qui remonte ne fait jamais dépasser ce que le
    /// lien porte, il lève seulement une borne qui l'emprisonnait.
    pub fn changer_plafond(&mut self, plafond_bps: u32) -> Decision {
        self.config.plafond_bps = plafond_bps;
        self.courant.video_bitrate_bps = self.courant.video_bitrate_bps.min(plafond_bps);
        self.courant
    }
```

⚠️ Le second test exige que le débit courant reste sous l'estimation quand le
plafond remonte. Si `changer_plafond` se contentait de `min`, un plafond qui
remonte ne rendrait rien de plus — ce qui **satisfait** le test. Si
l'implémentation choisie recalcule à la hausse, elle doit alors passer par le
chemin d'`observer` et son hystérésis, jamais écrire `plafond_bps` directement
dans `courant`. **Préférer la version ci-dessus** : la remontée arrive
naturellement à la prochaine observation, une par seconde.

- [ ] **Step 4: Lancer les tests pour vérifier qu'ils passent**

```bash
cd agent && cargo test --lib congestion 2>&1 | tail -15
```

- [ ] **Step 5: Commit**

```bash
git add agent/src/congestion/reconfiguration.rs
git commit -m "feat(d6): le controleur change de plafond sans reconstruire son echelle

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 8 : la boucle de transport applique la part

**Files:**
- Modify: `agent/src/transport/tick.rs` (212 lignes)
- Modify: `agent/src/transport/adaptation.rs` (472 lignes, **marge 28**)

**Interfaces:**
- Consumes: `VideoSource::part_a_appliquer`, `Controleur::changer_plafond`.
- Produces: `Session::appliquer_part(&mut self, bps: u32)`.

- [ ] **Step 1: Écrire le test qui échoue**

Dans le `mod tests` de `agent/src/transport/adaptation.rs` (il en existe déjà,
avec les fixtures de `transport/fixtures.rs` — **les lire d'abord**) :

Le `mod tests` du fichier construit déjà une `Session` réelle sans pair
distant — reprendre exactement ce patron :

```rust
    #[test]
    fn une_part_recue_borne_le_plafond_du_controleur() {
        let source = Box::new(fixtures::video_test_source());
        let mut session = Session::new(source, fixtures::local_ip(), Instant::now(), 12_000_000)
            .expect("session");
        // Avant la part, le plafond est celui de `Session::new`.
        assert_eq!(session.decision_courante().video_bitrate_bps, 12_000_000);

        session.appliquer_part(3_000_000);

        assert!(
            session.congestion.courant().video_bitrate_bps <= 3_000_000,
            "la décision du contrôleur doit être bornée par la part"
        );
        assert!(
            session.pending_decision.is_some(),
            "la part doit poser une décision que la branche a0ter appliquera"
        );
    }

    /// Une part qui remonte ne doit pas faire dépasser ce que le lien porte :
    /// elle lève une borne, elle n'en crée pas une nouvelle vers le haut.
    #[test]
    fn une_part_qui_remonte_ne_releve_pas_le_debit_au_dela_de_l_estimation() {
        let source = Box::new(fixtures::video_test_source());
        let mut session = Session::new(source, fixtures::local_ip(), Instant::now(), 12_000_000)
            .expect("session");
        session.appliquer_part(2_000_000);
        let apres_baisse = session.congestion.courant().video_bitrate_bps;

        session.appliquer_part(50_000_000);

        assert!(
            session.congestion.courant().video_bitrate_bps <= apres_baisse,
            "sans observation neuve, une part plus large ne remonte pas le débit d'elle-même"
        );
    }
```

⚠️ `decision_courante()` rend `bitrate_applique` et non la décision du
contrôleur (voir sa doc) : c'est `session.congestion.courant()` qu'il faut lire
pour juger `changer_plafond`. Les deux assertions ci-dessus le font
délibérément différemment — la première contrôle l'état de départ par
`decision_courante`, les suivantes le contrôleur lui-même.

- [ ] **Step 2: Lancer le test pour vérifier qu'il échoue**

```bash
cd agent && cargo test --lib adaptation 2>&1 | tail -15
```

- [ ] **Step 3: Écrire l'implémentation**

Dans `agent/src/transport/adaptation.rs`, dans `impl Session` :

```rust
    /// Applique une part du budget de session accordée par le capteur
    /// (sous-bloc D6).
    ///
    /// **Deux applications, et la seconde n'est pas la moins importante.**
    /// `changer_plafond` borne ce que le contrôleur décidera d'encoder ;
    /// `set_desired_bitrate` borne ce que le sous-système BWE **sonde**. Sans
    /// la seconde, N fenêtres continueraient de viser chacune le lien entier
    /// en injectant du trafic de sondage — la cause même du défaut que D6
    /// corrige — même si aucune n'encodait au-delà de sa part.
    pub(super) fn appliquer_part(&mut self, bps: u32) {
        let decision = self.congestion.changer_plafond(bps);
        self.rtc.bwe().set_desired_bitrate(Bitrate::bps(bps as u64));
        tracing::info!(part_bps = bps, "part de budget appliquee");
        self.pending_decision = Some(decision);
    }
```

⚠️ Vérifier le nom réel du champ du contrôleur dans `Session` (`self.congestion`
d'après `decision_courante`) et l'import de `Bitrate` (déjà présent dans
`transport.rs`, à ajouter ici si besoin).

Dans `agent/src/transport/tick.rs`, une branche **après `a1ter`** (le sommeil)
et **avant `a2`** :

```rust
        // a1quater) Une part de budget accordée par le capteur. Après le
        //           sommeil, dont elle découle : une fenêtre qu'on vient
        //           d'endormir reçoit sa part d'endormie dans le même lot, et
        //           l'appliquer avant l'ordre décrirait l'état précédent.
        //           Ne mute pas `Rtc` au sens du drainage — `set_desired_bitrate`
        //           n'écrit aucun paquet —, mais pose une décision que la
        //           branche a0ter appliquera au tour suivant.
        //           `part_a_appliquer` CONSOMME : aucune réémission, donc
        //           aucune reconfiguration en boucle à ~100 Hz.
        if let Some(bps) = self.source.part_a_appliquer() {
            self.appliquer_part(bps);
            return Ok(Tick::Continue);
        }
```

- [ ] **Step 4: Lancer les tests et la compilation croisée**

```bash
cd agent && cargo test --lib 2>&1 | tail -20
cd agent && cargo check --target x86_64-pc-windows-gnu 2>&1 | tail -20
```

- [ ] **Step 5: Vérifier la taille — la marge est de 28 lignes**

```bash
wc -l agent/src/transport/adaptation.rs agent/src/transport/tick.rs
```

Si `adaptation.rs` franchit 500, **extraire** `appliquer_part` et ses tests vers
`agent/src/transport/part.rs`, jamais compresser les commentaires.

- [ ] **Step 6: Commit**

```bash
git add agent/src/transport/tick.rs agent/src/transport/adaptation.rs
git commit -m "feat(d6): la boucle de transport applique sa part au controleur et au sondage

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 9 : `BUDGET_BPS` transmis jusqu'au capteur

**Files:**
- Modify: `scripts/run-agent.sh` (115 lignes)
- Modify: `agent/src/superviseur/lanceur.rs` (vérification seulement)

**Le piège que ce dépôt a payé trois fois** : `SUPERVISEUR` en D1,
`MULTIFENETRE_REPRISE` en D2, évité de justesse en D3. **Une variable neuve non
transmise fait démarrer l'agent sans elle et sans rien signaler.**

- [ ] **Step 1: Lire comment les variables existantes sont transmises**

```bash
grep -n "SUPERVISEUR\|CAPTEUR\|BITRATE\|MULTIFENETRE" scripts/run-agent.sh
```

- [ ] **Step 2: Ajouter `BUDGET_BPS` au même endroit**

Suivre exactement le patron du fichier pour `CAPTEUR` et `BITRATE`.

- [ ] **Step 3: Vérifier que le capteur hérite bien de l'environnement du superviseur**

```bash
grep -n "\.env(" agent/src/superviseur/lanceur.rs
```

Le capteur est lancé par `lancer_capteur`. Si l'environnement est **nettoyé
explicitement** (D3 a nettoyé celui des sondes pour éviter qu'un
`MULTIFENETRE_VDD_PURGE` résiduel ne détruise les sorties du porteur), alors
`BUDGET_BPS` doit être **ajouté explicitement** à la liste transmise. Le
vérifier, ne pas le supposer.

- [ ] **Step 4: Contrôler par une exécution que la valeur arrive**

Après le build de la tâche 10, chercher dans `agent.log` :

```
budget de debit de la session budget_bps=…
```

C'est la trace posée par `budget_bps()` en tâche 4. **Son absence signifie que
la variable n'est pas arrivée**, pas que le budget est absent.

- [ ] **Step 5: Commit**

```bash
git add scripts/run-agent.sh agent/src/superviseur/lanceur.rs
git commit -m "chore(d6): transmettre BUDGET_BPS jusqu'au capteur

Piege paye trois fois par ce depot : une variable neuve non transmise
fait demarrer l'agent sans elle et sans rien signaler.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 10 : recette — les critères ① et ②

**Files:**
- Create: `docs/superpowers/plans/journaux-multifenetres-d6/critere1-*.log`
- Create: `docs/superpowers/plans/journaux-multifenetres-d6/critere2-*.log`
- Create: `docs/superpowers/plans/journaux-multifenetres-d6/instrument/` (le pilote, versé dans son état final)
- Modify: `docs/superpowers/plans/2026-08-03-multifenetres-partage-capacite-resultats.md`

- [ ] **Step 1: Vérifier l'état de la VM, purger, bâtir**

```bash
virsh list --all
set -a && source .env && set +a
node scripts/winrm.js 'Get-Process agent -ErrorAction SilentlyContinue | Format-List Id | Out-String'
MULTIFENETRE_VDD_PURGE=1 scripts/run-agent.sh
scripts/build-agent.sh 2>&1 | tee /tmp/build-d6-t10.log
```

**Relever la taille du binaire** dans la sortie et la citer dans les résultats —
c'est la seule preuve de fraîcheur (D4 : un `rsync -a` qui remonte le temps fait
qu'un `cargo build` ne bâtit rien et le dit comme un succès ; seul
`cargo clean --release -p agent` débloque).

- [ ] **Step 2: Monter à 8 fenêtres, source animée, visibilité imposée**

Reprendre le pilote de D5. **Trois contraintes structurelles ici :**

1. **Source animée à cadence connue** (`instrument/anim-d4.html`) — sans quoi
   Desktop Duplication n'émet rien.
2. **Un `--user-data-dir` par fenêtre** — sans quoi Chrome rejoint son instance
   et l'on compte des lancements au lieu de fenêtres.
3. **Visibilité et focus IMPOSÉS page par page par le pilote.** Un Chrome sans
   interface rapporte `document.hidden = true` pour toute fenêtre
   d'arrière-plan, et `Page.addScriptToEvaluateOnNewDocument` ne court pas sur
   une page ouverte par `window.open`. **Ici c'est structurel et non
   cosmétique : la règle de part dépend du focus.** Une seule page doit être
   déclarée focalisée à la fois.

- [ ] **Step 3: Relever le critère ①**

Sur un palier d'au moins 30 s à 8 fenêtres :

| Grandeur | Source | Seuil |
| --- | --- | --- |
| i/s cumulées produites au capteur | `agent.log`, lignes de compteurs | — |
| i/s cumulées décodées au navigateur | `framesDecoded` par `getStats()` | **≥ 90 % du capteur** |
| RTT médian | `getStats()` et `agent.log` | **≤ 20 ms** |

**Ces deux seuils sont CHOISIS, pas mesurés** — à opposer aux 45 % et 104 ms de
D4, et à écrire comme tels.

Si la tâche 1 a réfuté la prémisse, ce critère devient : *la somme des
`set_desired_bitrate` de tous les enfants ne dépasse pas `BUDGET_BPS`*, lu dans
`agent.log` par les traces `part de budget appliquee`.

- [ ] **Step 4: Relever le critère ②**

- **Focus** : faire porter le focus successivement sur deux fenêtres, et
  vérifier dans `agent.log` que la taille d'encodage de la focalisée est
  **strictement** supérieure à celle de ses voisines éveillées. La grandeur qui
  tranche est `encode_size`, pas le débit demandé.
- **Endormie** : à 10 fenêtres ouvertes (`CAPACITE` vaut 10, `PLAFOND_EVEIL`
  vaut 8), vérifier que les deux endormies reçoivent `PART_DORMANTE_BPS` et
  que leurs octets RTP sortants restent au plancher.

- [ ] **Step 5: Contrôler la survie de la VM, et copier les journaux APRÈS la fin réelle**

```bash
grep -E "terminating on signal|shutting down" /var/log/libvirt/qemu/Windows.log | tail -4
```

Les enfants meurent quand le navigateur se ferme, **donc après** la fin du
pilote : copier `agent.log` ensuite, sans quoi les lignes de libération partent
avec le journal suivant (pièce perdue en D4).

- [ ] **Step 6: Purger les sorties virtuelles**

```bash
MULTIFENETRE_VDD_PURGE=1 scripts/run-agent.sh
```

- [ ] **Step 7: Commit**

```bash
git add docs/superpowers/plans/journaux-multifenetres-d6/
git commit -m "recette(d6): les criteres de la repartition du budget

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 11 : résultats et mise à jour de `CLAUDE.md`

**Files:**
- Modify: `docs/superpowers/plans/2026-08-03-multifenetres-partage-capacite-resultats.md`
- Modify: `CLAUDE.md`

- [ ] **Step 1: Compléter le document de résultats**

Sections obligatoires, sur le modèle des sous-blocs précédents :

1. **Le verdict**, critère par critère, avec **le nombre d'exécutions** dans
   chaque énoncé — « une exécution rapportée » n'est pas « un taux ».
2. **Ce que la mesure de la tâche 1 a établi**, et si elle a confirmé ou réfuté
   l'inférence de D4.
3. **Ce que D6 n'établit PAS** : les deux constantes non calibrées
   (`FACTEUR_FOCUS`, `PART_DORMANTE_BPS`) et le fait qu'aucune n'a été jugée
   par un jugement visuel ; l'absence de travail conservateur ; la latence de
   bout en bout toujours jamais mesurée ; les trois couches inconnues (plafond
   de 8 encodeurs, plafond de 4 processus, mécanisme de l'abandon du mutex
   DXGI) ; l'extinction propre du superviseur ; la mort d'un enfant pendant que
   les autres diffusent ; `BPP_MIN`, `HYSTERESIS` et `REPIT_APRES_ECHEC`
   toujours non calibrés.
4. **Pièges neufs** rencontrés pendant le chantier.
5. **Dette de taille**, relevée **par la commande** et non recopiée.

- [ ] **Step 2: Relever la dette par la commande**

```bash
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>440'
```

**Corriger le tableau de `CLAUDE.md` dans le même mouvement** si un chiffre a
dérivé — c'est la consigne explicite du fichier, et il porte déjà l'exemple d'un
nombre faux recopié trois fois (`distante.rs` annoncé 487 pour 235 réels).

- [ ] **Step 3: Écrire la section D6 de `CLAUDE.md`**

Une section `## 🔀 Sous-bloc D6 — partage de la capacité réseau (3 août 2026)`,
sur le modèle des cinq précédentes. **Et annoter les affirmations que D6
réfute ou complète**, une à une, là où elles vivent :

- la ligne « **D6** : partage de la capacité réseau entre N flux, et audio par
  fenêtre » du §D5 ⑥ — la renumérotation D6/D7/D8 ;
- « Ce que le chantier D (multi-fenêtres) devra régler » du chantier C volet 1 —
  la dette y est décrite pour une architecture qui n'existe plus (N pistes dans
  **une** session, alors que le produit a N sessions) ;
- « une fenêtre endormie continue de porter son propre contrôleur de
  congestion » (§D5 ⑥) — toujours vrai, mais son plafond est désormais le
  plancher d'endormie.

⚠️ **Chercher par le SENS, pas par la formule** : une négation se dit de
plusieurs façons, et c'est celle qu'on n'a pas listée qui survit. Balayer sur la
*chose niée*.

- [ ] **Step 4: Vérifier qu'aucune section du sommaire de `CLAUDE.md` ne contredit le détail**

Leçon payée au chantier des duplications parallèles : les documents longs ont un
sommaire, et c'est lui qu'on lit. Traiter le chapitre de détail en laissant le
sommaire intact laisse le lecteur repartir avec une tâche déjà faite.

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/plans/2026-08-03-multifenetres-partage-capacite-resultats.md CLAUDE.md
git commit -m "docs(d6): le partage du budget, et la renumerotation D6/D7/D8

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

- [ ] **Step 6: Revue finale de branche**

Invoquer `superpowers:requesting-code-review` sur la branche entière, puis
`superpowers:finishing-a-development-branch` pour décider de l'intégration.
