# Résorption de la dette de taille — plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ramener `congestion.rs`, `transport.rs`, `gamepad.rs` et `main.rs` sous la limite de 500 lignes par fichier, sans changer un seul comportement.

**Architecture:** Chaque fichier devient un module répertoire (édition 2021 : `congestion.rs` + `congestion/`, sans `mod.rs`). Le code se répartit par responsabilité, et les tests suivent le code qu'ils testent. Un seul endroit demande plus qu'un déplacement : `Session::act_on_timeout`, 445 lignes, dont les corps de branches descendent dans les modules thématiques pendant que la liste de priorités reste à un seul endroit.

**Tech Stack:** Rust 2021 (`stable`), `cargo test --workspace`, `cargo clippy --workspace`, `scripts/verify-all.sh`, `scripts/build-agent.sh` (compilation sur la VM Windows).

Spécification de référence : `docs/superpowers/specs/2026-07-30-dette-taille-fichiers-design.md`.

## Global Constraints

- **Limite** : 500 lignes par fichier, **tests inline compris**. Aucun fichier produit par ce plan ne doit l'atteindre.
- **Aucun changement de comportement** : ni signature publique modifiée, ni logique retouchée, ni test supprimé, ni test renommé. **Aucun test ajouté non plus — sauf à la tâche 4**, seule tâche qui édite du code, et dans le périmètre étroit que son en-tête définit.
- **Visibilité au minimum strict** : `pub(super)` d'abord, `pub(crate)` seulement si `pub(super)` ne suffit pas. Jamais `pub`.
- **Ligne de base à préserver, mesurée le 30 juillet 2026** : `cargo test --workspace` → **142 passés, 0 échec**. `cargo clippy --workspace` → **33 avertissements**, pas un de plus. Tous de la famille « never used / never constructed » — la liste exacte est figée dans `.superpowers/sdd/2026-07-30-dette-taille-fichiers/clippy-baseline.txt`, à comparer par `diff` plutôt qu'à compter : ces avertissements sont précisément ceux qu'un déplacement de module fait bouger.

  Le commentaire de `scripts/verify-all.sh` annonce 31 : ce chiffre datait du chantier B et n'a pas été remesuré depuis. Ne pas le prendre pour référence.
- **Ordre imposé** : `congestion.rs` → `transport.rs` → `gamepad.rs` → `main.rs`. Croissant en risque de vérification (§3.2 de la spec).
- **Un fichier traité, au moins un commit.** Aucun commit ne laisse le dépôt rouge. La spec disait « un fichier, un commit » ; le plan s'en écarte sur un point, `transport.rs`, découpé en trois commits (tâches 2, 3, 4). Un commit unique de 2558 lignes réparties sur neuf fichiers ne serait pas relisable, et les tâches 2 et 3 sont des déplacements purs qu'il faut pouvoir approuver sans les mêler à la seule tâche qui édite du code.
- **Langue** : commentaires, noms de modules et messages de commit en français, comme tout le dépôt.

---

## Pourquoi ce plan n'est pas en TDD

Le cycle rouge → vert → commit ne s'applique pas : la spec interdit d'ajouter le moindre test (§6, hors périmètre). Le harnais existe déjà — ce sont les 142 tests actuels — et le cycle de chaque étape est **déplacer → `cargo test` → commit**. Un test rouge n'a donc qu'une seule lecture possible : le déplacement est fautif. C'est cette propriété, et elle seule, qui rend le chantier sûr ; l'entamer en écrivant des tests la détruirait.

## Deux faits Rust qui gouvernent tout le plan

**1. Un module enfant voit les items privés de ses ancêtres.** `transport/tick.rs` accède aux 31 champs privés de `struct Session` déclarée dans `transport.rs` sans qu'aucun ne change de visibilité. C'est ce qui rend ces découpages presque gratuits.

**2. Un module frère ne les voit pas.** Un item qui descend dans `transport/socket.rs` et sert depuis `transport/tick.rs` doit passer `pub(super)` — ce qui le rend visible dans `transport` **et tous ses descendants**, donc dans les frères. `pub(super)` suffit toujours ici ; `pub(crate)` n'est jamais nécessaire.

**Conséquence pratique** : un `impl Session { ... }` peut vivre dans n'importe quel fichier sous `transport/`, avec un simple `use super::Session;` en tête.

---

## Task 1 : `congestion.rs` (990) → cinq fichiers

**Files:**
- Modify: `agent/src/congestion.rs` (990 → ~90)
- Create: `agent/src/congestion/echelle.rs` (~190)
- Create: `agent/src/congestion/hysteresis.rs` (~220)
- Create: `agent/src/congestion/controleur.rs` (~460)
- Create: `agent/src/congestion/reconfiguration.rs` (~110)

**Interfaces:**
- Consumes : rien (première tâche).
- Produces : `congestion.rs` continue d'exposer exactement les mêmes items publics qu'aujourd'hui — `Barreau`, `Echelle`, `Hysteresis`, `Config`, `Observation`, `Qualite`, `Adaptation`, `Decision`, `Controleur` — par re-export. `transport.rs` fait `use crate::congestion;` et n'est pas modifié par cette tâche.

**Découpage source, bornes exactes dans le fichier actuel :**

| Destination | Lignes de code | Lignes de tests |
| --- | --- | --- |
| `congestion/echelle.rs` | 12-103 (`DIVISEURS`, `BPP_MIN`, `Barreau`, `Echelle`, `impl Echelle`) | 507-591 (3 tests) |
| `congestion/hysteresis.rs` | 104-219 (`use std::time`, `DELAI_DESCENTE`, `DELAI_REMONTEE`, `SEJOUR_MINIMAL`, `DELAI_AMORCAGE`, `Hysteresis`, `impl Hysteresis`) | 592-689 (`t0()` + 5 tests) |
| `congestion.rs` (reste) | 1-11 (doc), 220-287 (`MARGE`, `ECART_MINIMAL_DEBIT`, `PERTE_MAX_OPUS`, `Config`, `Observation`, `Qualite`, `Adaptation`, `Decision`) | — |
| `congestion/controleur.rs` | 288-342 (`Controleur`, `new`, `courant`), 370-502 (`observer`, `ecart_relatif`) | 690-922 (`config()`, `obs()` + 9 tests) |
| `congestion/reconfiguration.rs` | 343-369 (`changer_source`) | 923-989 (3 tests `changer_source_*`) |

`congestion/reconfiguration.rs` existe pour une raison arithmétique doublée d'une raison de fond : sans lui, `controleur.rs` pèserait ~526 lignes, et `changer_source` traite une reconfiguration à chaud là où `observer` traite l'asservissement continu. La spec (§4.2) avait anticipé cette séparation.

- [ ] **Step 1 : Créer `agent/src/congestion/echelle.rs`**

Créer le répertoire `agent/src/congestion/`, puis le fichier avec cet en-tête, suivi du contenu des lignes 12-103 de `congestion.rs` :

```rust
//! L'échelle de barreaux : les résolutions d'encodage disponibles pour une
//! taille de source donnée, et le débit minimal que chacune exige.
//!
//! `BPP_MIN` (0,05 bit par pixel et par image) est **reconduit faute de
//! preuve du contraire, pas confirmé**, et il est couplé au `fps` de
//! `Config` : les deux se recalibrent ensemble. Voir `CLAUDE.md`,
//! « Réglages du contrôleur ».
```

Puis y ajouter, à la fin, le module de tests :

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // ... contenu des lignes 507-591 de congestion.rs, sans le `mod tests {`
    // ni le `use super::*;` d'origine, qui sont déjà écrits ci-dessus.
}
```

- [ ] **Step 2 : Ajuster les imports d'`echelle.rs`**

`DIVISEURS` et `BPP_MIN` descendent **avec** `Echelle` : ce sont ses constantes de calcul. Vérifier ensuite que `echelle.rs` ne référence rien qui soit resté dans `congestion.rs`. Si `Echelle::new` ou une de ses méthodes utilise `Config`, ajouter `use super::Config;`.

- [ ] **Step 3 : Créer `agent/src/congestion/hysteresis.rs`**

En-tête :

```rust
//! Hystérésis : combien de temps une contrainte doit durer avant qu'on
//! descende d'un barreau, et combien avant qu'on remonte. Asymétrique à
//! dessein — voir `DELAI_REMONTEE`.

use std::time::{Duration, Instant};

use super::echelle::Barreau;
```

Puis le contenu des lignes 104-219 (moins le `use std::time` d'origine, déjà écrit) et les tests des lignes 592-689 dans un `#[cfg(test)] mod tests { use super::*; ... }`.

- [ ] **Step 4 : Créer `agent/src/congestion/controleur.rs`**

En-tête :

```rust
//! Le contrôleur : convertit les observations du pair en décisions de débit
//! et de résolution, en passant par l'échelle et l'hystérésis.

use std::time::Instant;

use super::echelle::{Barreau, Echelle};
use super::hysteresis::Hysteresis;
use super::{Adaptation, Config, Decision, Observation, Qualite};
use super::{ECART_MINIMAL_DEBIT, MARGE, PERTE_MAX_OPUS};
```

Puis les lignes 288-342 et 370-502, et les tests 690-922.

- [ ] **Step 5 : Créer `agent/src/congestion/reconfiguration.rs`**

En-tête :

```rust
//! Reconfiguration à chaud : ce qui se passe quand la source change de
//! taille en cours de session. L'échelle est reconstruite, et le barreau
//! courant reporté sur la nouvelle — borné si elle est plus courte.

use std::time::Instant;

use super::controleur::Controleur;
use super::Decision;

impl Controleur {
    // ... contenu des lignes 343-369 : `pub fn changer_source`
}

#[cfg(test)]
mod tests {
    // ... contenu des lignes 923-989
}
```

- [ ] **Step 6 : Réécrire `agent/src/congestion.rs`**

Le fichier ne garde que la doc de module (lignes 1-11), les constantes 220-230, les types de contrat 231-287, les déclarations de sous-modules et les re-exports :

```rust
//! Contrôleur de congestion : décide du débit et de la résolution d'encodage
//! à partir de ce que le pair rapporte.
//!
//! Aucune dépendance à Windows, à str0m ni au socket — c'est ce qui rend
//! toute la politique testable sur Linux, sans VM et sans réseau. Même
//! raison d'être que `geometry.rs`, `rebuild.rs` et `clock.rs`.
//!
//! Découpé en quatre sous-modules : `echelle` (les résolutions disponibles),
//! `hysteresis` (les délais avant changement), `controleur` (l'asservissement
//! continu) et `reconfiguration` (le changement de taille de source).

mod controleur;
mod echelle;
mod hysteresis;
mod reconfiguration;

pub use controleur::Controleur;
pub use echelle::{Barreau, Echelle};
pub use hysteresis::Hysteresis;

// ... lignes 220-287 d'origine : MARGE, ECART_MINIMAL_DEBIT, PERTE_MAX_OPUS,
// Config, Observation, Qualite, Adaptation, Decision
```

Les constantes `MARGE`, `ECART_MINIMAL_DEBIT` et `PERTE_MAX_OPUS` restent ici mais doivent devenir `pub(crate)` ou rester privées selon leur usage : `controleur.rs` les lit via `use super::{...}`, ce qui fonctionne sans changer leur visibilité (règle 1 : les enfants voient les privés des ancêtres). **Ne pas les rendre publiques.**

- [ ] **Step 7 : Compiler**

Run: `cd /home/mallanic/Projects/Guacamole/agent && cargo build`
Expected: succès. Si des erreurs `E0603 private` apparaissent, c'est qu'un item est utilisé par un **frère** et non par un descendant : lui poser `pub(super)`, jamais plus.

- [ ] **Step 8 : Vérifier la ligne de base des tests**

Run: `cd /home/mallanic/Projects/Guacamole/agent && cargo test --workspace 2>&1 | tail -5`
Expected: `test result: ok. 142 passed; 0 failed`

Si le nombre est inférieur à 142, un test a été perdu dans un déplacement. Le retrouver avec :
`cargo test --workspace -- --list | wc -l` puis comparaison avec `git stash` / `git stash pop`.

- [ ] **Step 9 : Vérifier clippy**

Run:
```bash
cd /home/mallanic/Projects/Guacamole/agent
cargo clippy --workspace 2>&1 | grep -E '^warning' | sed 's/^warning: //' | sort \
  | diff - ../.superpowers/sdd/2026-07-30-dette-taille-fichiers/clippy-baseline.txt
```
Expected: aucune différence. Tout avertissement nouveau s'explique — le plus probable est un « never used » sur un item devenu visible autrement. Ne pas le masquer par `#[allow]`.

- [ ] **Step 10 : Vérifier les tailles**

Run:
```bash
wc -l /home/mallanic/Projects/Guacamole/agent/src/congestion.rs \
      /home/mallanic/Projects/Guacamole/agent/src/congestion/*.rs
```
Expected: aucun fichier ≥ 500.

- [ ] **Step 11 : Commit**

```bash
cd /home/mallanic/Projects/Guacamole
git add agent/src/congestion.rs agent/src/congestion/
git commit -m "refactor(congestion): découpe en échelle, hystérésis, contrôleur et reconfiguration

990 lignes en un fichier, dont 487 de tests. Découpé par responsabilité,
chaque sous-module portant ses propres tests. changer_source part dans son
propre module : il traite la reconfiguration à chaud, quand observer traite
l'asservissement continu — et sans cette séparation controleur.rs restait
au-dessus de la limite.

Aucun changement de comportement : 142 tests, identiques, verts.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâches 2 à 4 : pourquoi neuf fichiers et non six

La spec (§4.3) proposait six fichiers pour `transport.rs`. Le comptage des branches d'`act_on_timeout`, fait au moment d'écrire ce plan, en impose neuf. Deux raisons, toutes deux arithmétiques avant d'être esthétiques :

1. **Décomposer une fonction ne la raccourcit pas.** La spec supposait qu'extraire `act_on_timeout` en méthodes privées suffirait à faire tenir `tick.rs` sous la limite. C'est faux : les 445 lignes restent, augmentées des signatures. Pour passer sous 500, les corps de branches doivent **changer de fichier**, pas seulement de fonction — d'où `controle.rs` et `adaptation.rs`.
2. **`media.rs` aurait pesé ~610 lignes.** Vidéo et audio réunis totalisent 194 lignes de code et 259 de tests. D'où `piste_video.rs` et `piste_audio.rs`.

Le découpage reste celui que la spec décrit — par responsabilité, tests avec leur code. Seul le nombre de coupes change.

---

## Task 2 : `transport.rs` — extraire `socket.rs` et `fixtures.rs`

La partie la plus mécanique de `transport.rs` : des fonctions libres sans état et les échafaudages de test. La faire d'abord réduit le fichier de ~430 lignes et met en place le répertoire pour les tâches 3 et 4.

**Files:**
- Modify: `agent/src/transport.rs` (2558 → ~2130)
- Create: `agent/src/transport/socket.rs` (~280)
- Create: `agent/src/transport/fixtures.rs` (~250)

**Interfaces:**
- Consumes : rien de la tâche 1.
- Produces : `pub(super) fn classify_recv_error(kind: std::io::ErrorKind) -> RecvErrorAction`, `pub(super) fn recv_error_backoff(consecutive_errors: u32) -> Duration`, `pub(super) fn bounded_wait(now: Instant, deadline: Instant, next_frame_at: Option<Instant>, audio_cap: Option<Duration>) -> Duration`, `pub(super) struct TimerResolutionGuard`, `pub(super) enum RecvErrorAction`, et `pub(super) const RECV_POLL_INTERVAL: Duration`. Les tâches 3 et 4 les consomment via `use super::socket::...`.

**Vérifier la signature réelle de `bounded_wait`** dans le fichier actuel (lignes 352-371) avant de l'écrire dans le `use` — la reproduire à l'identique.

- [ ] **Step 1 : Créer `agent/src/transport/socket.rs`**

Y déplacer les lignes **194-371** de `transport.rs` : `RecvErrorAction` (194-231), `classify_recv_error` (232-252), `RECV_ERROR_BACKOFF_BASE`/`RECV_ERROR_BACKOFF_MAX` (253-255), `recv_error_backoff` (256-276), le bloc `#[link(name = "winmm")]` et `TimerResolutionGuard` dans ses deux variantes (277-351), `bounded_wait` (352-371). Déplacer aussi `RECV_POLL_INTERVAL` et son commentaire (lignes 105-141).

En-tête :

```rust
//! Le socket et l'attente : classification des erreurs de réception, recul
//! exponentiel, résolution du timer Windows, et calcul de l'attente bornée
//! entre deux tours de boucle.

use std::time::{Duration, Instant};
```

Poser `pub(super)` sur chacun des items déplacés — ils servent tous depuis `transport.rs` ou ses autres enfants.

- [ ] **Step 2 : Y déplacer les tests correspondants**

Depuis le `mod tests` de `transport.rs`, déplacer les lignes **1614-1678** (`connection_reset_est_transitoire`, `interrupted_est_transitoire`, `erreurs_non_reconnues_sont_fatales`, `backoff_croit_avec_le_nombre_d_erreurs_consecutives`, `backoff_reste_borne_meme_apres_une_tres_longue_rafale`, `backoff_est_non_nul_des_la_premiere_erreur`) et **1733-1758** (`attente_bornee_par_l_echeance_d_image_la_plus_proche`, `attente_bornee_par_l_echeance_rtc_si_plus_proche`, `attente_dictee_par_rtc_seul_sans_piste_video`), dans :

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // ... les 9 tests
}
```

`borne_l_attente_quand_l_audio_est_negocie` (1758-1788) **ne bouge pas ici** : il construit une `Session` et vérifie `audio_wait_cap`. Il ira dans `transport/piste_audio.rs` à la tâche 3.

- [ ] **Step 3 : Créer `agent/src/transport/fixtures.rs`**

Le test `atteint_la_cadence_video_visee_avec_un_pair_local` (1825-2089) contient ~230 lignes d'échafaudage — pair str0m local, source vidéo factice, source audio factice — que trois autres tests redéclarent partiellement. Extraire ces échafaudages ici.

```rust
//! Échafaudages partagés des tests de `transport` : un pair str0m local, une
//! source vidéo factice et une source audio factice.
//!
//! `#[cfg(test)]` : rien de ceci n'est compilé en `release`.

use super::*;

// ... les définitions de types factices et fonctions d'aide, extraites des
// tests des lignes 1825-2558. Chacune devient `pub(super)`.
```

Lire les quatre tests concernés — 1825-2089, 2090-2239, 2239-2384, 2384-2477 — et n'extraire que ce qui est **réellement commun**. Ne pas généraliser un échafaudage utilisé une seule fois : il reste avec son test.

- [ ] **Step 4 : Déclarer les sous-modules dans `transport.rs`**

Juste après le bloc de `use` (après la ligne 41) :

```rust
#[cfg(test)]
mod fixtures;
mod socket;

use socket::{bounded_wait, classify_recv_error, recv_error_backoff, RecvErrorAction};
use socket::{TimerResolutionGuard, RECV_POLL_INTERVAL};
```

- [ ] **Step 5 : Compiler et tester**

Run: `cd /home/mallanic/Projects/Guacamole/agent && cargo test --workspace 2>&1 | tail -5`
Expected: `test result: ok. 142 passed; 0 failed`

- [ ] **Step 6 : Commit**

```bash
cd /home/mallanic/Projects/Guacamole
git add agent/src/transport.rs agent/src/transport/
git commit -m "refactor(transport): sort le socket et les échafaudages de test

Les fonctions libres du socket — classification d'erreurs, recul, résolution
du timer, attente bornée — et les échafaudages de test partagés quittent le
fichier. Première étape d'un découpage en six, sans toucher à Session.

142 tests, identiques, verts.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Task 3 : `transport.rs` — extraire `piste_video.rs` et `piste_audio.rs`

**Files:**
- Modify: `agent/src/transport.rs` (~2130 → ~1560)
- Create: `agent/src/transport/piste_video.rs` (~370)
- Create: `agent/src/transport/piste_audio.rs` (~160)

Nommés `piste_video` / `piste_audio` et non `video` / `audio` : `agent/src/audio.rs` existe déjà et porte `AudioPacket`/`AudioSource`. Deux modules nommés `audio` dans le même arbre se liraient mal.

**Interfaces:**
- Consumes : `super::socket::bounded_wait` n'est pas utilisé ici.
- Produces : deux `impl Session` supplémentaires. Aucune signature publique nouvelle. `pub(super) const FRAME_INTERVAL` et `pub(super) const AUDIO_POLL_INTERVAL` sont consommés par `transport/tick.rs` à la tâche 4.

**Découpage :**

| Destination | Code | Tests |
| --- | --- | --- |
| `piste_video.rs` | `FRAME_INTERVAL` (68 + sa doc), `CandidatePt` + `select_h264_pt` (152-182), `next_frame_deadline` (183-193), `select_negotiated_h264_pt` (1201-1214), `capture_instant` (1215-1230), `write_frame` (1231-1276), `warn_negotiation_once` (1354-1363) | 1586-1601 (`pt()`), 1679-1708 (3 tests de paquetisation), 1709-1732 (2 tests de cadence), 1789-1824 (`la_session_ancre_l_instant_de_capture`), 2090-2238 (`write_frame_annonce_l_instant_de_capture`) |
| `piste_audio.rs` | `AUDIO_POLL_INTERVAL` (76 + sa doc), `set_audio_source` (1277-1282), `audio_wait_cap` (1290-1299), `select_negotiated_opus_pt` (1300-1322), `write_audio` (1323-1344), `warn_audio_negotiation_once` (1345-1353) | 1601-1613 (`la_frequence_rtp_audio`), 1758-1788 (`borne_l_attente_quand_l_audio_est_negocie`) |

`set_control_source` (1283-1289) **reste dans `transport.rs`** : il concerne le canal de contrôle, pas l'audio, malgré sa position dans le fichier.

- [ ] **Step 1 : Créer `agent/src/transport/piste_video.rs`**

```rust
//! La piste vidéo : négociation du payload type H.264, ancrage de l'instant
//! de capture sur l'origine d'horloge de la session, et écriture des unités
//! d'accès vers str0m.

use std::time::{Duration, Instant};

use str0m::format::Codec;
use str0m::media::{Frequency, MediaTime, Mid, Pt};

use super::Session;
use crate::clock::instant_from_pts;
use crate::h264::{AccessUnit, CLOCK_RATE_HZ};

// ... FRAME_INTERVAL, CandidatePt, select_h264_pt, next_frame_deadline
// (fonctions libres, `pub(super)`)

impl Session {
    // ... select_negotiated_h264_pt, capture_instant, write_frame,
    // warn_negotiation_once (méthodes privées : elles restent privées,
    // règle 1 — mais `write_frame` est appelée depuis `tick.rs`, donc
    // `pub(super)`)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::fixtures::*;

    // ... les tests listés dans le tableau
}
```

Ajuster les `use` d'après ce que le compilateur réclame — ne pas deviner la liste complète, la faire émerger par `cargo build`.

- [ ] **Step 2 : Créer `agent/src/transport/piste_audio.rs`**

```rust
//! La piste audio : négociation du payload type Opus, et écriture des
//! paquets vers str0m. L'audio passe AVANT la vidéo dans la liste de
//! priorités de `tick` — une coupure sonore s'entend, une image en retard
//! de 10 ms ne se voit pas.

use std::time::Duration;

use str0m::format::Codec;
use str0m::media::{Frequency, MediaTime, Mid, Pt};

use super::Session;
use crate::audio::{AudioPacket, AudioSource};

// ... AUDIO_POLL_INTERVAL (`pub(super)`)

impl Session {
    // ... set_audio_source (reste `pub`, c'est l'API appelée par main.rs),
    // audio_wait_cap, select_negotiated_opus_pt, write_audio,
    // warn_audio_negotiation_once
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::fixtures::*;

    // ... les deux tests listés
}
```

- [ ] **Step 3 : Déclarer les sous-modules**

Dans `transport.rs`, après `mod socket;` :

```rust
mod piste_audio;
mod piste_video;

use piste_audio::AUDIO_POLL_INTERVAL;
use piste_video::{next_frame_deadline, FRAME_INTERVAL};
```

- [ ] **Step 4 : Compiler et tester**

Run: `cd /home/mallanic/Projects/Guacamole/agent && cargo test --workspace 2>&1 | tail -5`
Expected: `test result: ok. 142 passed; 0 failed`

- [ ] **Step 5 : Commit**

```bash
cd /home/mallanic/Projects/Guacamole
git add agent/src/transport.rs agent/src/transport/
git commit -m "refactor(transport): sépare la piste vidéo de la piste audio

Négociation de payload type, écriture vers str0m et ancrage d'horloge
partent dans deux modules distincts, chacun avec ses tests. Nommés
piste_video/piste_audio pour ne pas entrer en collision de lecture avec
agent/src/audio.rs, qui porte AudioPacket et AudioSource.

142 tests, identiques, verts.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Task 4 : `transport.rs` — `evenements.rs`, `controle.rs`, `adaptation.rs`, `tick.rs`

**C'est la seule tâche du plan qui édite du code plutôt que de le déplacer.** `act_on_timeout` fait 445 lignes ; la déplacer telle quelle donnerait un fichier de ~465 sans place pour ses tests. Ses branches descendent donc dans les modules thématiques, et la liste de priorités reste à un seul endroit.

### Exception à l'interdiction d'ajouter des tests

Parce que cette tâche édite, elle est la seule autorisée à ajouter des tests — et seulement sur les branches extraites. Deux cibles précises, que `CLAUDE.md` consigne depuis le chantier C comme des réserves connues et non couvertes :

1. **Les transitions d'`Adaptation`** (`Active` → `Indisponible`) et l'expiration de l'estimation BWE à 5 s : « vérifiées par lecture de code », sans test. Elles vivent dans la branche a0ter, qui descend dans `adaptation.rs`.
2. **Le câblage de `resize`** (branche a1, qui appelle `Controleur::changer_source`) : « aucun verrou automatisé », le comportement étant prouvé par mesure sur la VM et non protégé contre régression. Cette branche descend aussi dans `adaptation.rs`.

Ajouter ces tests est autorisé, pas obligatoire — si l'échafaudage nécessaire s'avère disproportionné, le dire dans le rapport plutôt que de forcer. **Aucun autre test n'est autorisé nulle part ailleurs dans le plan.**

Conséquence sur la ligne de base : le compte de tests passe de 142 à 142 + N. Tout écart doit s'expliquer par les tests délibérément ajoutés, nommés dans le rapport. Un test qui disparaît reste une faute, à cette tâche comme aux autres.

**Files:**
- Modify: `agent/src/transport.rs` (~1560 → ~400)
- Modify: `agent/src/transport/piste_video.rs` (+45)
- Modify: `agent/src/transport/piste_audio.rs` (+23)
- Modify: `agent/src/transport/socket.rs` (+124)
- Create: `agent/src/transport/tick.rs` (~200)
- Create: `agent/src/transport/controle.rs` (~300)
- Create: `agent/src/transport/adaptation.rs` (~270)
- Create: `agent/src/transport/evenements.rs` (~340)

**Interfaces:**
- Consumes : `super::Session` et tous les items `pub(super)` posés aux tâches 2 et 3.
- Produces : `pub(super) fn act_on_timeout(&mut self, deadline: Instant) -> Result<Tick>` et `pub(super) enum Tick`, appelés depuis `Session::run` dans `transport.rs`.

**Bornes exactes des branches d'`act_on_timeout` (fichier d'origine) :**

| Branche | Lignes | Taille | Destination du corps |
| --- | --- | --- | --- |
| en-tête `fn` | 756 | 1 | `tick.rs` |
| a0) drainage dû | 757-776 | 20 | `tick.rs` (partagé vidéo/audio) |
| a0bis) contrôle hors boucle | 777-797 | 21 | `controle.rs` |
| a) contrôle en file, puis `ending` | 798-846 | 49 | `controle.rs` |
| a0ter) décision d'adaptation | 847-928 | 82 | `adaptation.rs` |
| a1) redimensionnement | 929-995 | 67 | `adaptation.rs` |
| a2) fenêtre disparue | 996-1008 | 13 | `tick.rs` |
| a3) paquet audio | 1009-1031 | 23 | `piste_audio.rs` |
| b) image vidéo | 1032-1076 | 45 | `piste_video.rs` |
| c) attente et réception | 1077-1200 | 124 | `socket.rs` |

- [ ] **Step 1 : Créer `agent/src/transport/tick.rs` avec la liste de priorités**

`act_on_timeout` devient un aiguillage. Chaque branche appelle une méthode privée définie dans son module thématique. Les commentaires détaillés **descendent avec leur corps** ; `tick.rs` garde le raisonnement d'ordonnancement, celui qui explique pourquoi cet ordre et pas un autre.

```rust
//! La liste de priorités d'un tour de boucle.
//!
//! `act_on_timeout` décide de la SEULE action entreprise par tour. L'ordre
//! n'est pas arbitraire :
//!
//! - le drainage dû passe en priorité absolue : c'est la seule façon de
//!   garantir qu'aucune mutation ne s'enchaîne sans un passage complet par
//!   `poll_output()` entre les deux, quel que soit l'état des autres files ;
//! - le contrôle et l'adaptation passent avant les médias : reconfigurer
//!   l'encodeur avec une image en vol coûterait cette image ;
//! - l'audio passe avant la vidéo : une coupure sonore s'entend, une image
//!   en retard de 10 ms ne se voit pas ;
//! - l'attente sur le socket ne vient qu'en dernier, quand il n'y a rien à
//!   émettre.
//!
//! Le corps de chaque branche vit dans son module thématique ; ce fichier
//! ne porte que l'ordre.

use std::time::{Duration, Instant};

use anyhow::Result;

use super::Session;

pub(super) enum Tick {
    Continue,
    Disconnected,
}

pub(super) const ALIVE_CHECK_INTERVAL: Duration = Duration::from_secs(1);

impl Session {
    pub(super) fn act_on_timeout(&mut self, deadline: Instant) -> Result<Tick> {
        // a0) Drainage dû après la dernière image ou le dernier paquet audio.
        if self.video_write_pending_drain || self.audio_write_pending_drain {
            return self.brancher_drainage_du();
        }

        // a0bis) Un message de contrôle produit hors de la boucle attend.
        self.drainer_controle_externe();

        // a) Un message de contrôle est en attente.
        if !self.pending_control.is_empty() {
            return self.brancher_controle_en_file();
        }
        if self.ending {
            // Message de fin envoyé (file vidée ci-dessus) : terminé.
            return Ok(Tick::Disconnected);
        }

        // a0ter) Décision d'adaptation en attente.
        if let Some(decision) = self.pending_decision.take() {
            return self.brancher_decision(decision);
        }

        // a1) Redimensionnement en attente.
        if let Some((width, height)) = self.pending_resize.take() {
            return self.brancher_redimensionnement(width, height);
        }

        // a2) La fenêtre capturée a-t-elle disparu ?
        if let Some(tick) = self.brancher_fenetre_vivante()? {
            return Ok(tick);
        }

        // a3) Un paquet audio.
        if let Some(tick) = self.brancher_audio()? {
            return Ok(tick);
        }

        // b) Une image vidéo.
        if let Some(tick) = self.brancher_video()? {
            return Ok(tick);
        }

        // c) Rien à émettre : attendre un paquet entrant, borné.
        self.brancher_attente(deadline)
    }

    // ... brancher_drainage_du (lignes 757-776) et brancher_fenetre_vivante
    // (lignes 996-1008) : leurs corps restent ici.
}
```

**Ces signatures sont indicatives.** Les types de retour exacts se lisent dans le code d'origine ; chaque branche renvoie soit `Result<Tick>`, soit `Option<Tick>` selon qu'elle conclut le tour ou laisse passer. Reproduire fidèlement la sémantique de chaque `return` d'origine — c'est le seul endroit du plan où une inattention change un comportement.

- [ ] **Step 2 : Vérifier immédiatement après l'extraction de la liste**

Ne pas enchaîner les extractions avant d'avoir compilé. Extraire d'abord `tick.rs` avec **toutes** les branches encore dans son `impl Session`, puis compiler et tester :

Run: `cd /home/mallanic/Projects/Guacamole/agent && cargo test --workspace 2>&1 | tail -5`
Expected: `test result: ok. 142 passed; 0 failed`

À ce stade `tick.rs` pèse ~480 lignes : c'est attendu et transitoire. Ne pas commiter ici.

- [ ] **Step 3 : Déplacer `brancher_audio` vers `piste_audio.rs`**

Déplacer la méthode (corps issu des lignes 1009-1031) dans le bloc `impl Session` existant de `piste_audio.rs`, avec son commentaire d'origine. La déclarer `pub(super)`.

Run: `cargo test --workspace 2>&1 | tail -5` → 142 passés.

- [ ] **Step 4 : Déplacer `brancher_video` vers `piste_video.rs`**

Corps issu des lignes 1032-1076, commentaire compris — dont le long passage sur `to_payload`/`do_payload` de str0m, qui explique le drapeau de drainage. Ce commentaire est la justification de la branche a0 : le conserver intégralement.

Run: `cargo test --workspace 2>&1 | tail -5` → 142 passés.

- [ ] **Step 5 : Déplacer `brancher_attente` vers `socket.rs`**

Corps issu des lignes 1077-1200. C'est la plus grosse branche (124 lignes) et elle contient l'appel à `bounded_wait`, déjà dans ce module — la cohésion y gagne.

Run: `cargo test --workspace 2>&1 | tail -5` → 142 passés.

- [ ] **Step 6 : Créer `agent/src/transport/controle.rs`**

```rust
//! Le canal de contrôle : mise en file des messages sortants, drainage de
//! ceux produits hors de la boucle, et fin de session.

use anyhow::Result;
use proto::control::AgentControl;

use super::tick::Tick;
use super::Session;

impl Session {
    // ... queue_control (664-680), set_control_source (1283-1289),
    // begin_ending (1364-1376), drain_quietly (1377-1394),
    // drainer_controle_externe (777-797), brancher_controle_en_file (798-846)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::fixtures::*;

    // ... relaie_au_pair_un_controle_pousse_depuis_l_exterieur_de_la_boucle
    // (lignes 2384-2477 d'origine)
}
```

La constante `PLAFOND_CONTROLE_EN_FILE` est déclarée **à l'intérieur** de la branche a0bis (ligne 788) : elle descend avec elle.

Run: `cargo test --workspace 2>&1 | tail -5` → 142 passés.

- [ ] **Step 7 : Créer `agent/src/transport/adaptation.rs`**

```rust
//! L'asservissement au réseau vu depuis la boucle : application d'une
//! décision du contrôleur de congestion, et redimensionnement de la source.

use std::time::{Duration, Instant};

use anyhow::Result;

use super::tick::Tick;
use super::Session;
use crate::congestion;

pub(super) const ESTIMATION_INITIALE_BPS: u32 = 2_500_000;
pub(super) const EXPIRATION_ESTIMATION: Duration = Duration::from_secs(5);

impl Session {
    // ... decision_courante (681-693), brancher_decision (847-928),
    // brancher_redimensionnement (929-995)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::fixtures::*;

    // ... un_refus_repete_de_set_encode_size_ne_remonte_pas_dans_decision_courante
    // (lignes 2477-2558 d'origine)
}
```

Reprendre les commentaires de doc d'origine des deux constantes (lignes 83-104) : ils portent le raisonnement sur l'amorçage du BWE et l'expiration à 5 s, que le chantier D devra relire.

Run: `cargo test --workspace 2>&1 | tail -5` → 142 passés.

- [ ] **Step 8 : Créer `agent/src/transport/evenements.rs`**

```rust
//! Traitement des événements que str0m remonte : changement d'état ICE,
//! ouverture de canal, données reçues, estimation de débit sortant.

use anyhow::Result;
use str0m::channel::ChannelId;
use str0m::{Event, IceConnectionState};

use proto::control::ClientControl;
use proto::input::InputMessage;

use super::Session;

impl Session {
    // ... handle_event (1395-1543), dispatch_channel_data (1544-1581)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::fixtures::*;

    // ... relaie_une_demande_d_image_cle_du_pair_vers_la_source
    // (lignes 2239-2384 d'origine)
}
```

Run: `cargo test --workspace 2>&1 | tail -5` → 142 passés.

- [ ] **Step 9 : Vérifier ce qui reste dans `transport.rs`**

Le fichier ne doit plus porter que : la doc de module (1-21, à compléter d'un paragraphe listant les sous-modules), les `use`, les déclarations de sous-modules, `struct Session` (372-506), `new` (507-643), `accept_offer` (644-663), `run` (694-755).

Run: `wc -l /home/mallanic/Projects/Guacamole/agent/src/transport.rs /home/mallanic/Projects/Guacamole/agent/src/transport/*.rs`
Expected: aucun fichier ≥ 500. Si `transport.rs` dépasse encore, le candidat suivant est `Session::new` (137 lignes) vers un `transport/init.rs`.

- [ ] **Step 10 : Vérification complète**

Run: `cd /home/mallanic/Projects/Guacamole && ./scripts/verify-all.sh`
Expected: toutes les étapes passent.

- [ ] **Step 11 : Commit**

```bash
cd /home/mallanic/Projects/Guacamole
git add agent/src/transport.rs agent/src/transport/
git commit -m "refactor(transport): éclate act_on_timeout par branche thématique

445 lignes en une fonction, structurées en liste de priorités que ses propres
commentaires numérotaient déjà. La liste reste à un seul endroit — tick.rs,
avec le raisonnement d'ordonnancement — et le corps de chaque branche descend
dans son module : contrôle, adaptation, piste audio, piste vidéo, socket.
handle_event et dispatch_channel_data partent dans evenements.rs.

Seule tâche du chantier qui édite plutôt que déplace. 142 tests, identiques,
verts à chaque extraction intermédiaire.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Task 5 : `gamepad.rs` (599) → trois fichiers

**Cette tâche exige la VM.** Deux des trois fichiers produits sont `#[cfg(windows)]` et ne sont vérifiés par aucun test Linux. Vérifier d'abord que la VM tourne :

```bash
virsh list --all    # « en cours d'exécution » attendu
virsh start Windows # si « fermé »
until timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null; do sleep 5; done
```

**Files:**
- Modify: `agent/src/gamepad.rs` (599 → ~200)
- Create: `agent/src/gamepad/win.rs` (~265)
- Create: `agent/src/gamepad/probe.rs` (~145)

**Interfaces:**
- Produces : `gamepad.rs` continue d'exposer `plus_recent`, `LimiteurVibration`, `PERIODE_MIN`, et de re-exporter `spawn_connect`, `spawn_rumble`, `VirtualPad` sous `#[cfg(windows)]`, plus `probe`. `main.rs` (tâche 6) appelle `gamepad::probe` et `gamepad::spawn_connect` sans changement.

**Découpage :**

| Destination | Lignes d'origine |
| --- | --- |
| `gamepad.rs` | 1-16 (doc), 18-97 (`PERIODE_MIN`, `plus_recent`, `LimiteurVibration`), 98-195 (tests) |
| `gamepad/win.rs` | 197-457 (le contenu de `mod win { ... }`, sans les accolades du module) |
| `gamepad/probe.rs` | 459-599 (le commentaire de doc de `probe` puis `probe`) |

- [ ] **Step 1 : Créer `agent/src/gamepad/win.rs`**

Prendre le contenu **intérieur** de `mod win { ... }` (lignes 198-456), c'est-à-dire sans la ligne `mod win {` ni son accolade fermante. Désindenter d'un niveau.

Le `use super::{plus_recent, LimiteurVibration, PERIODE_MIN};` d'origine reste **valide tel quel** : `gamepad/win.rs` est un enfant de `gamepad`, donc `super` désigne toujours `gamepad`.

En-tête à ajouter :

```rust
//! Le module ViGEmBus définitif : `VirtualPad` branche une manette Xbox 360
//! virtuelle et lui applique les états reçus, `spawn_rumble` relaie ses
//! notifications de vibration vers le client via le canal de contrôle.
```

- [ ] **Step 2 : Créer `agent/src/gamepad/probe.rs`**

Y déplacer les lignes 459-599, en gardant le long commentaire de doc qui précède `probe` — il documente la reprise sur `ERROR_NO_MORE_ITEMS`, un comportement caractérisé une seule fois et donc précieux.

Retirer le `#[cfg(windows)]` posé sur `pub fn probe` : il devient redondant, le module entier étant déclaré sous `#[cfg(windows)]` au Step 3.

- [ ] **Step 3 : Réécrire la fin de `agent/src/gamepad.rs`**

Après les tests (ligne 195), remplacer tout le reste par :

```rust
#[cfg(windows)]
mod probe;
#[cfg(windows)]
mod win;

#[cfg(windows)]
pub use probe::probe;
#[cfg(windows)]
pub use win::{spawn_connect, spawn_rumble, VirtualPad};
```

Mettre à jour la doc de module (lignes 1-16) : les trois natures qu'elle décrit sont désormais trois fichiers, le dire.

- [ ] **Step 4 : Tester sur Linux**

Run: `cd /home/mallanic/Projects/Guacamole/agent && cargo test --workspace 2>&1 | tail -5`
Expected: `test result: ok. 142 passed; 0 failed`

Attention : ce résultat ne prouve rien sur `win.rs` ni `probe.rs`, invisibles au compilateur Linux. Le Step 5 est obligatoire.

- [ ] **Step 5 : Compiler sur la VM**

Run: `cd /home/mallanic/Projects/Guacamole && ./scripts/build-agent.sh`
Expected: compilation réussie.

C'est **la seule vérification** de `gamepad/win.rs` et `gamepad/probe.rs`. Sans elle, la tâche n'est pas terminée. Si la compilation échoue sur un `use super::` devenu invalide, c'est que le fichier n'est pas placé où le plan le dit.

- [ ] **Step 6 : Vérifier les tailles et commiter**

```bash
wc -l /home/mallanic/Projects/Guacamole/agent/src/gamepad.rs \
      /home/mallanic/Projects/Guacamole/agent/src/gamepad/*.rs
cd /home/mallanic/Projects/Guacamole
git add agent/src/gamepad.rs agent/src/gamepad/
git commit -m "refactor(gamepad): acte les trois natures que l'en-tête annonçait

Le fichier documentait lui-même qu'il portait trois choses distinctes : la
logique pure testable sur Linux, le module ViGEmBus définitif, et la sonde du
chantier B. Elles deviennent trois fichiers.

Vérifié par cargo test sur Linux pour la logique pure, et par build-agent.sh
sur la VM pour les deux modules Windows — que nul test ne couvre.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Task 6 : `main.rs` (1342) → sept fichiers

**Cette tâche exige la VM** (même préambule qu'à la tâche 5) : ~70 % de `main.rs` est sous `#[cfg(windows)]`.

**Files:**
- Modify: `agent/src/main.rs` (1342 → ~135)
- Create: `agent/src/demarrage.rs` (~445)
- Create: `agent/src/diagnostics.rs` (~60)
- Create: `agent/src/diagnostics/capture.rs` (~450)
- Create: `agent/src/diagnostics/pixels.rs` (~95)
- Create: `agent/src/diagnostics/entree.rs` (~195)
- Create: `agent/src/diagnostics/audio.rs` (~50)

**Interfaces:**
- Produces : `pub async fn demarrage::executer(config: Config) -> Result<()>` et `pub fn diagnostics::aiguiller() -> Result<bool>`, qui renvoie `true` si une sonde a tourné et que `main` doit s'arrêter là. `aiguiller` ne prend **pas** la `Config` : les cinq sondes lisent leur configuration depuis l'environnement, comme aujourd'hui.

**Découpage, bornes exactes :**

| Destination | Lignes d'origine | Taille |
| --- | --- | --- |
| `diagnostics/pixels.rs` | 46-135 (`read_pixel`, `capture_center_pixel`) | 90 |
| `demarrage.rs` | 136-183 (`watch_encoder`), 879-1261 (l'assemblage de session) | 431 |
| `diagnostics/entree.rs` | 184-216 (`point_depart_lineaire`), 784-795 (`VIGEM_PROBE`), 809-878 (`INPUT_LINEARITY_PROBE`), 1264-1342 (les 7 tests) | 194 |
| `diagnostics/capture.rs` | 279-718 (`CAPTURE_TEST`) | 440 |
| `diagnostics/audio.rs` | 724-759 (`AUDIO_PROBE`), 767-777 (`PROCESS_LOOPBACK_PROBE`) | 46 |
| `main.rs` | 1-45 (mods, uses), 219-237 (`Config`, `config`), 240-279 (début de `main`) | ~135 |

Les 7 tests actuels de `main.rs` portent tous sur `point_depart_lineaire` : ils suivent la fonction dans `diagnostics/entree.rs`.

- [ ] **Step 1 : Créer `agent/src/diagnostics/pixels.rs`**

```rust
//! Lecture de pixels d'une texture GPU, pour les modes diagnostic.
//!
//! Copie vers une texture « staging » accessible au CPU
//! (`D3D11_USAGE_STAGING`) : c'est ce qui permet de prouver que le recadrage
//! capture bien le contenu de la fenêtre, et pas seulement des dimensions
//! qui auraient l'air correctes sans l'être.

// ... lignes 46-135, `read_pixel` et `capture_center_pixel` en `pub(super)`
```

Conserver le commentaire de doc d'origine des lignes 46-53, qui explique précisément cela.

- [ ] **Step 2 : Créer `agent/src/diagnostics/capture.rs`**

Y déplacer le corps du bloc `if let Ok(fragment) = std::env::var("CAPTURE_TEST")` (lignes 280-718), transformé en fonction :

```rust
//! Mode diagnostic `CAPTURE_TEST` : vérifie le repérage d'une fenêtre par
//! fragment de titre, puis que la capture en restitue bien le contenu.

use anyhow::Result;

use super::pixels::{capture_center_pixel, read_pixel};

pub(super) fn executer(fragment: &str) -> Result<()> {
    // ... corps des lignes 281-717
}
```

- [ ] **Step 3 : Créer `agent/src/diagnostics/audio.rs` et `agent/src/diagnostics/entree.rs`**

Même transformation : chaque bloc `if std::env::var(...)` devient une fonction `pub(super) fn executer_*`.

`diagnostics/audio.rs` :
```rust
//! Sondes audio du chantier A : périphérique de rendu par défaut de la
//! session et format de mixage (`AUDIO_PROBE`), et isolation de l'audio
//! d'un seul processus (`PROCESS_LOOPBACK_PROBE`, requis par le chantier D).
```

`diagnostics/entree.rs` :
```rust
//! Sondes d'entrée du chantier B : linéarité de la visée
//! (`INPUT_LINEARITY_PROBE`) et disponibilité de ViGEmBus (`VIGEM_PROBE`).
```

`point_depart_lineaire` et ses 7 tests vont dans `entree.rs`.

- [ ] **Step 4 : Créer `agent/src/diagnostics.rs`**

L'aiguillage. Il renvoie `Ok(true)` si une sonde a tourné, auquel cas `main` s'arrête.

```rust
//! Aiguillage des modes diagnostic, tous activés par variable
//! d'environnement. Aucun n'injecte d'entrée ni n'ouvre de session : ils
//! observent et consignent.

use anyhow::Result;

#[cfg(windows)]
mod audio;
mod entree;
#[cfg(windows)]
mod capture;
#[cfg(windows)]
mod pixels;

/// Renvoie `true` si une sonde a tourné — `main` doit alors s'arrêter là.
pub fn aiguiller() -> Result<bool> {
    #[cfg(windows)]
    if let Ok(fragment) = std::env::var("CAPTURE_TEST") {
        capture::executer(&fragment)?;
        return Ok(true);
    }
    // ... AUDIO_PROBE, PROCESS_LOOPBACK_PROBE, VIGEM_PROBE,
    // INPUT_LINEARITY_PROBE, dans cet ordre — l'ordre d'origine
    Ok(false)
}
```

**Respecter l'ordre d'origine des cinq blocs.** Il n'est pas indifférent : `INPUT_LINEARITY_PROBE` lit `INPUT_LINEARITY_NEUTRALISER`, dont l'effet dépend de ce que `main` a déjà fait au démarrage.

- [ ] **Step 5 : Créer `agent/src/demarrage.rs`**

Y déplacer `watch_encoder` (136-183) et le corps de l'assemblage de session (879-1261) :

```rust
//! Mise en route d'une session : capture, encodeur, source, `Session`
//! WebRTC et signalisation, assemblés dans cet ordre.

use anyhow::Result;

use crate::source::{FileSource, VideoSource};
use crate::transport::Session;
use crate::Config;

pub async fn executer(config: Config) -> Result<()> {
    // ... corps des lignes 879-1261
}

// ... watch_encoder, `#[cfg(windows)]`
```

Attention aux variables définies avant la ligne 879 et utilisées après — notamment `window_hwnd_addr` (655) et `clock_origin` (661), déclarées dans `main()` entre les sondes et l'assemblage. Elles descendent dans `demarrage::executer`. Relire les lignes 640-878 pour n'en oublier aucune.

- [ ] **Step 6 : Réécrire `agent/src/main.rs`**

```rust
// ... lignes 1-35 : déclarations de modules, plus les deux nouvelles
mod demarrage;
mod diagnostics;

// ... lignes 37-45 : uses

// ... lignes 219-237 : struct Config et fn config()

#[tokio::main]
async fn main() -> Result<()> {
    // ... lignes 241-278 : neutralisation du pointeur, puis config()

    if diagnostics::aiguiller()? {
        return Ok(());
    }

    demarrage::executer(config).await
}
```

Conserver intégralement le commentaire des lignes 241-262 sur `INPUT_LINEARITY_NEUTRALISER=0` : il documente un bogue réel corrigé en ronde de revue 1, où la neutralisation SPI faussait la mesure de la sonde par construction.

- [ ] **Step 7 : Tester sur Linux**

Run: `cd /home/mallanic/Projects/Guacamole/agent && cargo test --workspace 2>&1 | tail -5`
Expected: `test result: ok. 142 passed; 0 failed`

- [ ] **Step 8 : Compiler sur la VM**

Run: `cd /home/mallanic/Projects/Guacamole && ./scripts/build-agent.sh`
Expected: compilation réussie. **Obligatoire** : ~70 % du code déplacé est invisible au compilateur Linux.

- [ ] **Step 9 : Vérifier qu'une sonde fonctionne encore**

Une compilation réussie ne prouve pas que l'aiguillage est correct. Lancer une sonde bon marché :

Run: `cd /home/mallanic/Projects/Guacamole && VIGEM_PROBE=1 ./scripts/run-agent.sh`
Expected: la sonde s'exécute et rend son verdict, comme avant le découpage.

- [ ] **Step 10 : Commit**

```bash
cd /home/mallanic/Projects/Guacamole
git add agent/src/main.rs agent/src/demarrage.rs agent/src/diagnostics.rs agent/src/diagnostics/
git commit -m "refactor(main): sépare les sondes de diagnostic du démarrage

main() faisait 1024 lignes, dont ~750 de modes diagnostic activés par
variable d'environnement. Les cinq sondes partent sous diagnostics/, la mise
en route de session dans demarrage.rs, et main() se réduit à : neutraliser le
pointeur, lire la config, aiguiller, démarrer.

L'ordre des cinq sondes est conservé : INPUT_LINEARITY_PROBE dépend de ce que
main a déjà fait au démarrage.

Vérifié par cargo test sur Linux, build-agent.sh sur la VM, et une exécution
réelle de VIGEM_PROBE.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Task 7 : Clôture — `CLAUDE.md` et vérification finale

**Files:**
- Modify: `CLAUDE.md` (section « Conventions de code », tableau de dette)

- [ ] **Step 1 : Vérifier qu'aucun fichier du périmètre ne dépasse**

Run:
```bash
cd /home/mallanic/Projects/Guacamole
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
```
Expected: exactement trois lignes — `agent/src/encode.rs` (1480), `agent/src/windows_source.rs` (721), `agent/src/wasapi.rs` (543).

Si un fichier produit par ce plan apparaît, il n'est pas terminé : le redécouper avant de continuer.

- [ ] **Step 2 : Vérification complète**

Run: `cd /home/mallanic/Projects/Guacamole && ./scripts/verify-all.sh`
Expected: toutes les étapes passent, 142 tests.

- [ ] **Step 3 : Mettre à jour le tableau de dette de `CLAUDE.md`**

Dans la section « Conventions de code », remplacer le tableau « Dette existante au 30 juillet 2026 » par les trois fichiers restants, et remplacer le paragraphe sur `transport.rs` — qui n'est plus en dette — par une note sur ce qui reste :

```markdown
**Dette existante** (code source uniquement) :

| Fichier | Lignes | Pourquoi elle reste |
| --- | --- | --- |
| `agent/src/encode.rs` | 1480 | `#[cfg(windows)]`, aucun test |
| `agent/src/windows_source.rs` | 721 | `#[cfg(windows)]`, aucun test |
| `agent/src/wasapi.rs` | 543 | `#[cfg(windows)]`, aucun test |

Ces trois modules ne se compilent que sur la VM et ne sont couverts par aucun
test : les découper se ferait sans filet automatisé. La dette est assumée
jusqu'à ce qu'ils gagnent des tests — voir
`docs/superpowers/specs/2026-07-30-dette-taille-fichiers-design.md` §1.

Les quatre fichiers que la suite de tests couvrait ont été résorbés le
30 juillet 2026 : voir `docs/superpowers/plans/2026-07-30-dette-taille-fichiers.md`.
```

- [ ] **Step 4 : Commit**

```bash
cd /home/mallanic/Projects/Guacamole
git add CLAUDE.md
git commit -m "docs: la dette de taille se réduit aux trois modules Windows sans test

congestion.rs, transport.rs, gamepad.rs et main.rs sont passés sous la limite.
Restent encode.rs, windows_source.rs et wasapi.rs — dette assumée : ils ne se
compilent que sur la VM et aucun test ne les couvre.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Ce que ce plan ne fait pas

Rappel de la spec §6, à relire si la tentation se présente en cours de route :

- **Aucun test écrit, hors l'exception étroite de la tâche 4.** `main.rs` et `transport.rs` sont sous-testés et leur code va défiler sous les yeux de l'exécutant. Y toucher, hors des deux réserves nommées à la tâche 4, ferait perdre la seule propriété qui rend le chantier sûr : qu'un test rouge ne peut signifier qu'une chose.
- **Aucune amélioration de logique** rencontrée en chemin. Elle se note, elle ne se fait pas ici.
- **`encode.rs`, `windows_source.rs`, `wasapi.rs`** restent tels quels.
