# Sous-bloc D11 — solder les legs : plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rendre le son au cas majoritaire — une application, une fenêtre —, et
prouver quatre choses que le chantier D n'a jamais prouvées : le repli sur la
promotion, la séparation des flux entre fenêtres, le coût de la duplication
surdimensionnée, et le maillon fautif du `Resize`.

**Architecture:** Aucune architecture neuve. D11 est un sous-bloc de **solde** :
deux lignes de code changent un comportement (`audio_porteuse` au branchement
mono-fenêtre, `{erreur:#}` au journal du refus), deux variables de **banc**
rendent atteignables des chemins que D10 n'a pas pu exercer, et le reste est de
l'instrument, de la mesure et du document.

**Tech Stack:** Rust (crate `agent`, cible réelle `x86_64-pc-windows-msvc` sur la
VM), TypeScript côté `client/`, PowerShell distant via `scripts/winrm.js`,
pilotes CDP en Node depuis l'hôte.

**Spec :** `docs/superpowers/specs/2026-08-19-multifenetres-solder-les-legs-design.md`
(commit `75f7ae7`).

---

## Divergences relevées entre la spec et le code, AVANT toute tâche

Relevées par la commande le 19 août 2026, sur `main` à `217a765`. **Aucune n'est
bloquante**, et chacune est reportée ici plutôt que découverte à l'exécution.

| Spec | Réalité relevée | Effet |
| --- | --- | --- |
| §3.1 point 6 : « `tick.rs:259` — l'unique appelant d'`appliquer_audio` est gardé par `self.source.audio_a_appliquer()` » | le **garde** est à `src/transport/tick.rs:258`, l'**appel** à `:259`. `grep -n "audio_a_appliquer()" src/transport/tick.rs` → `258` | cosmétique ; le fait est exact |
| §3.2 : « l'accesseur, posé auprès de ses jumeaux `set_audio_source` (`piste_audio.rs:26`) et `set_audio_reconstructeur` (`piste_audio.rs:227`) » | exact, vérifié : `grep -n "pub fn set_audio_source\|pub fn set_audio_reconstructeur" src/transport/piste_audio.rs` → `26` et `227` | aucun |
| §7.2 : `client/src/main.ts:386` porte l'invariant | exact : `grep -n "addEventListener('open', emettreSiPossible)" client/src/main.ts` → `386` | aucun |
| §9.1 : les six tailles de fichiers | **les six sont exactes**, relevées par `wc -l` : `piste_audio.rs` 403, `tick/tests/audio.rs` 403, `fil.rs` 339, `demarrage/audio.rs` 95, `transport.rs` 468, `run-agent.sh` 124 | aucun |
| §11 : `cargo test -p agent` = 458, `npx vitest run` = 107 | **exacts**, relancés le 19 août 2026 pour ce plan | aucun |
| §3.3 : six sites emploient `{erreur:#}` | **exact** : `grep -rn '{erreur:#}\|{e:#}' agent/src` rend **sept** lignes dont **une est le commentaire** de `plafond/sonde.rs:110`. Six sites réels | aucun |

**Et une divergence que la spec ne relève pas — un TREIZIÈME commentaire orphelin
de D10, que sa revue transverse a manqué.** `agent/src/transport.rs:278` dit du
champ `audio_porteuse` :

> **Sert UNIQUEMENT à détecter la TRANSITION vers `actif = true`** dans
> `appliquer_audio` (`piste_audio.rs`)

C'est **déjà faux aujourd'hui**, et depuis D10 lui-même. Le champ a **deux**
lecteurs : `piste_audio.rs:156` (`if actif && !self.audio_porteuse`, la
transition) **et** `piste_audio.rs:318` (`source.set_actif(self.audio_porteuse)`,
qui n'est pas une transition). Chronologie établie par la commande :

```
$ git blame -L 276,281 agent/src/transport.rs   → 264083d  (la doc)
$ git blame -L 318,318 agent/src/transport/piste_audio.rs → ddd0b05  (le 2e lecteur)
$ git merge-base --is-ancestor 264083d ddd0b05 → vrai
```

**La doc précède le second lecteur, et n'a pas été relue quand il est arrivé** —
exactement le patron que la revue transverse de D10 s'était donné pour cible, et
qui lui a échappé dans le fichier même qu'elle corrigeait. La tâche 3 la corrige,
et la tâche 15 en tire la conséquence de méthode.

---

## Global Constraints

- **Plafond de 500 lignes** par fichier de code source écrit à la main.
  **Aucune extraction n'est requise a priori** (les marges sont dans le tableau
  ci-dessous), **mais deux fichiers reçoivent une porte de contrôle chiffrée**
  et un **point de chute nommé d'avance** — voir « Budget de lignes » plus bas.
  La règle du dépôt est sans exception : **extraction d'abord, addition
  ensuite**, jamais une compression. D9 a joué la compression deux fois et la
  revue l'a fait défaire les deux fois ; D10 a franchi le plafond trois fois et
  l'a rattrapé trois fois par extraction.
- **Jamais `git add -A`** : nommer les fichiers. Un `git add -A` a déjà emporté
  le travail concurrent d'une autre tâche dans un commit qui ne compilait pas.
- **Vérification sur l'hôte avant toute compilation distante** :
  `cd agent && cargo check --target x86_64-pc-windows-gnu` (couvre types,
  emprunts, visibilités, durées de vie ; **pas l'édition de liens**).
- **Tests d'hôte** : `cd agent && cargo test -p agent`. Référence d'entrée
  **458 passed, 0 failed** (relancée pour ce plan le 19 août 2026). Côté client :
  `cd client && npx vitest run`, référence **107 passed** (idem).
- **Annoncer le compte de tests attendu AVANT de le mesurer.** D10 a récupéré un
  test écrasé par un `Write` uniquement parce que le compte est sorti à 452 au
  lieu des 453 annoncés d'avance.
- **Un contrôle qu'on n'a jamais vu rouge n'est pas un contrôle.** Chaque test
  neuf est exécuté **avant** l'implémentation et vu échouer, et **le message
  d'échec attendu est écrit dans ce plan**. Là où le rouge n'est pas atteignable
  sur l'hôte, le plan le **dit** et nomme l'unique pièce qui en tient lieu — il
  ne prétend pas le contraire. *(D10 a attrapé quatre contrôles vacueux, dont
  **trois écrits par le plan lui-même** : un plan n'immunise pas contre ce
  patron, il en est une source.)*
- **Toute variable d'environnement neuve doit être ajoutée explicitement à
  `scripts/run-agent.sh`** — piège payé en D1 (`SUPERVISEUR`), D2
  (`MULTIFENETRE_REPRISE`), D6 (`BUDGET_BPS`) et D7 (`AUDIO`). L'agent démarre
  sans elle **et sans rien signaler**.
- **Convention des variables de banc** : `PART_SONDAGE`/`PLEIN_ECRAN` désarment
  sur `=0` ; les deux variables de D11 sont **absentes = désarmées**, présentes
  et non nulles = armées. Ne jamais tester `is_ok()`. Trace `warn!` **seulement
  si armée**, et son texte dit « banc, jamais une configuration livrée ».
- **La VM n'est pas démarrée automatiquement.** Avant toute tâche de recette :
  `virsh list --all`, `virsh start Windows`, attendre WinRM **et** un accès réel
  à `/media/vm` (`until ls /media/vm/dev`), et `set -a && source .env && set +a`
  avant `scripts/build-agent.sh` — sans quoi il s'arrête **en silence**.
- **La VM est un état partagé** : deux recettes ne courent jamais ensemble. Les
  tâches 9 à 14 sont **strictement sérialisées entre elles**.
- **Appels à la VM bloquants au premier plan**, jamais backgroundés : une
  commande backgroundée automatiquement par le harnais **ne survit pas** à la fin
  du tour de l'agent qui l'a lancée, et D10 y a perdu deux exécutions — le
  symptôme est un journal **tronqué** copié depuis une VM où l'agent tourne
  encore.
- **`grep -a` sur tout journal copié pendant que Windows écrit encore** : une
  queue d'octets NUL fait classer le fichier « binaire », et `grep` rend alors
  **une sortie vide, pas zéro** — indiscernable d'un compte nul.
- **Aucun taux ne sera revendiqué.** Chaque énoncé de résultat porte son nombre
  d'exécutions. **Deux exécutions par critère** au mieux.
- **Aucune preuve ne vit dans un rapport gitignoré.** Le document de résultats
  permanent et les journaux versés sont la seule mémoire — le leg 3 de D10 est
  perdu pour n'avoir pas tenu cette règle.
- **Ne jamais fabriquer une sortie de commande.** D10 en a relevé **deux**,
  présentées comme des relevés, dont une inscrite dans `CLAUDE.md` **à
  l'intérieur d'une correction qui dénonçait une affirmation non étayée**. Le
  mécanisme a été nommé par son auteur : *réutiliser la sortie d'une commande
  antérieure pour répondre à la question d'une autre, sans la relancer.*

---

## Budget de lignes — relevé par la commande le 19 août 2026

```
$ wc -l agent/src/transport/piste_audio.rs agent/src/transport/tick/tests/audio.rs \
        agent/src/windows_audio/fil.rs agent/src/demarrage/audio.rs \
        agent/src/transport.rs agent/src/audio.rs scripts/run-agent.sh client/src/main.ts
   403 agent/src/transport/piste_audio.rs
   403 agent/src/transport/tick/tests/audio.rs
   339 agent/src/windows_audio/fil.rs
    95 agent/src/demarrage/audio.rs
   468 agent/src/transport.rs
   318 agent/src/audio.rs
   124 scripts/run-agent.sh
   392 client/src/main.ts
```

| Fichier | Lignes | Marge | Additions prévues | Porte |
| --- | --- | --- | --- | --- |
| `agent/src/transport/piste_audio.rs` | **403** | **97** | tâche 2 (~8), tâche 3 (~18), tâche 4 (~45) — **~71 cumulés** | ⚠️ **porte chiffrée** : relever `wc -l` **après chaque** des trois tâches ; si le total dépasse **480**, extraire **avant** de continuer |
| `agent/src/transport/tick/tests/audio.rs` | **403** | **97** | tâches 3 et 4 (~90 cumulés) | ⚠️ **même porte à 480** |
| `agent/src/transport.rs` | **468** | **32** | tâche 3 : réécriture de la doc de `audio_porteuse` | ⚠️ **marge la plus étroite du chemin.** La réécriture doit être **à somme nulle ou négative** ; si elle grossit, extraction, jamais compression |
| `agent/src/windows_audio/fil.rs` | **339** | **161** | tâche 5 (~20) | aucune |
| `agent/src/audio.rs` | **318** | **182** | tâche 5 : le prédicat pur + ses tests (~35) | aucune |
| `agent/src/demarrage/audio.rs` | **95** | **405** | tâche 3 (~6) | aucune |
| `scripts/run-agent.sh` | **124** | — | tâches 4 et 5 (2 lignes) | hors portée de la règle |

**Points de chute nommés d'avance**, à créer **seulement** si une porte se
déclenche — et alors **dans une tâche à part, placée AVANT l'addition** :

- `agent/src/transport/piste_audio/injection.rs` — l'injection de faute de
  reconstruction et son budget. Déclaré `mod injection;` **à l'intérieur** de
  `piste_audio.rs` : pas de frontière `#[cfg(windows)]` ici, donc la convention
  `#[path]` de `CLAUDE.md` **ne s'applique pas** (elle ne vise que les modules
  qu'on extrait d'un parent non portable pour les compiler sur l'hôte).
- `agent/src/transport/tick/tests/audio/injection.rs` — les tests de l'injection.
  Même mécanique, `mod injection;` dans `tests/audio.rs`, sur le modèle de
  `tick/tests.rs` qui déclare déjà `mod audio; mod reste;`.
- `agent/src/transport/piste_audio.rs` — point de chute des **champs audio** de
  `transport.rs` et de leur documentation, si la porte de `transport.rs` se
  déclenche. C'est là que vivent déjà leurs seuls écrivains.

⚠️ **Cette table est un relevé du 19 août 2026 et elle DÉRIVERA.** Relancer
`wc -l` avant de s'y fier — jamais recopier ces nombres.

---

## Structure des fichiers

**Créés :**

| Fichier | Responsabilité |
| --- | --- |
| `docs/superpowers/plans/journaux-multifenetres-d11/` | les journaux versés, avec leurs jumeaux `-plat` |
| `docs/superpowers/plans/journaux-multifenetres-d11/instrument/anim-d11.html` | la mire de D4/D10, **plus un marqueur d'identité invariant dans le temps** |
| `docs/superpowers/plans/journaux-multifenetres-d11/instrument/pilote-audio-d11.mjs` | le pilote des recettes ①, ② et ③ |
| `docs/superpowers/plans/journaux-multifenetres-d11/instrument/pilote-flux-d11.mjs` | le pilote de la recette ④ (séparation des flux) |
| `docs/superpowers/plans/journaux-multifenetres-d11/instrument/pilote-cout-d11.mjs` | le pilote de la recette ⑤ (coût de la duplication) |
| `docs/superpowers/plans/journaux-multifenetres-d11/instrument/pilote-resize-d11.mjs` | le pilote de la recette ⑥, **abonné à `Runtime.consoleAPICalled`** |
| `docs/superpowers/plans/2026-08-19-multifenetres-solder-les-legs-resultats.md` | le document de résultats permanent |

**Modifiés :** `agent/src/transport/piste_audio.rs`, `agent/src/transport.rs`,
`agent/src/demarrage/audio.rs`, `agent/src/transport/tick/tests/audio.rs`,
`agent/src/windows_audio/fil.rs`, `agent/src/audio.rs`,
`agent/src/windows_source/telemetrie.rs`, `client/src/resize.test.ts`,
`client/src/main.ts`, `scripts/run-agent.sh`,
`docs/superpowers/plans/2026-08-07-multifenetres-solder-la-branche-resultats.md`,
`CLAUDE.md`.

**Aucun fichier de code n'est touché par les legs 7 et 8** (coût de la
duplication, séparation des flux) : ce sont des mesures et de l'instrument.

---

## Interfaces partagées

Ces signatures sont **fixées ici** : les tâches les consomment telles quelles.

```rust
// agent/src/transport/piste_audio.rs — NEUF (tâche 3)
impl Session {
    /// Déclare que cette session porte le son sans qu'aucun capteur ne le lui
    /// dise. Réservé au mode MONO-FENÊTRE.
    pub fn set_audio_porteuse(&mut self, porteuse: bool);
}

// agent/src/audio.rs — NEUF (tâche 5), PUR, testé sur l'hôte
/// L'injection de fautes de lecture est-elle encore armée ?
///
/// `fenetre = None` : illimitée, donc toujours armée — c'est le comportement
/// de D10, strictement préservé quand `AUDIO_FAUTE_LECTURE_MS` est absente.
pub fn injection_encore_armee(
    depuis: std::time::Duration,
    fenetre: Option<std::time::Duration>,
) -> bool;

// agent/src/transport/piste_audio.rs — NEUF (tâche 4), interne au module
/// Le budget global au processus de fautes de reconstruction injectées.
/// `OnceLock` + `AtomicU32`, lu UNE fois dans l'environnement.
fn budget_faute_reconstruction() -> &'static std::sync::atomic::AtomicU32;
```

**Variables d'environnement neuves** — toutes deux **de BANC**, jamais une
configuration livrée :

| Variable | Lue dans | Effet | Absente |
| --- | --- | --- | --- |
| `AUDIO_FAUTE_RECONSTRUCTION=<n>` | `transport/piste_audio.rs` | fait échouer les *n* prochaines reconstructions, **budget global au processus** | désarmée |
| `AUDIO_FAUTE_LECTURE_MS=<ms>` | `windows_audio/fil.rs` (prédicat pur dans `audio.rs`) | borne dans le temps l'armement de `AUDIO_FAUTE_LECTURE` | **illimité** — comportement de D10 inchangé |

---

# Famille 0 — le document que deux sources ne racontent pas pareil

### Task 1 : réparer le §11 du document de résultats de D10

**Files:**
- Modify: `docs/superpowers/plans/2026-08-07-multifenetres-solder-la-branche-resultats.md`

**Interfaces:**
- Consumes: rien. **Aucune dépendance : parallélisable avec tout le reste.**
- Produces: un §11 qui porte les **huit** legs, et les deux de D9 qui n'en sont
  jamais sortis.

Aucun code. C'est le naufrage du « 487 » sous sa forme la plus pure : une liste
corrigée dans l'index (`CLAUDE.md`) et pas dans le document, **le document non
corrigé étant celui que le successeur ouvre quand il veut le détail**.

- [ ] **Step 1 : établir l'écart PAR LA COMMANDE, avant d'écrire**

```bash
R=docs/superpowers/plans/2026-08-07-multifenetres-solder-la-branche-resultats.md
grep -n "mono-fenêtre\|mono-fenetre" "$R"
grep -c "leg 11\|leg 12\|test faible\|invariant non écrit" "$R"
```

Relevé du 19 août 2026, à **reproduire** et non à recopier : le premier rend
**quatre** occurrences, aux lignes **336, 346, 352 et 360** — **toutes au §8bis**
(la revue transverse), **aucune au §11**. Le second rend **0**.

⚠️ **Ce contrôle PEUT échouer, et c'est ce qui le rend utile** : si une
occurrence apparaissait dans le §11, le leg n'aurait pas été omis et cette tâche
n'aurait pas lieu d'être. Vérifier le numéro de ligne du §11
(`grep -n '^## 11' "$R"`) et comparer.

- [ ] **Step 2 : réécrire le §11 avec HUIT legs et le rang de chacun**

Insérer le leg manquant **à son rang**, en gardant la numérotation de
`CLAUDE.md` (qui est l'index qu'on lit), et **renuméroter les six suivants** :

1. l'A/B sur `set_desired_bitrate` (leg 3 de D9) ;
2. le maillon fautif du `Resize` (leg 7 de D9) ;
3. les six constats parqués de D9 — **perdus** ;
4. 🔴 **la reconstruction audio est INERTE en mono-fenêtre** ← *le manquant* ;
5. construire `AUDIO_FAUTE_RECONSTRUCTION` ;
6. la cause du refus de reconstruction n'est pas identifiée ;
7. le coût de la duplication d'une sortie surdimensionnée ;
8. la séparation des flux entre fenêtres.

Le n°4 porte **son renvoi au §8bis**, où le diagnostic complet vit déjà — on ne
duplique pas l'analyse, on répare l'index.

- [ ] **Step 3 : rétablir les deux legs de D9 tombés du registre**

Ajouter au même §11 un encadré nommant **D9 n°11** (deux tests incapables de
rendre l'autre valeur : `windows_source/telemetrie.rs`,
`client/src/resize.test.ts`) et **D9 n°12** (l'invariant synchrone de
`client/src/main.ts`), avec la mention qu'ils sont **repris par D11** (tâche 6).

⚠️ **Ne recopier aucun numéro de ligne** dans ce texte : celui de `main.ts` a
déjà bougé de `:345` (D9) à `:386` (aujourd'hui). Nommer le symbole
(`addEventListener('open', emettreSiPossible)`), jamais la ligne.

- [ ] **Step 4 : contrôler la complétude AVANT de commiter**

```bash
grep -c "leg 11\|leg 12\|D9 n°11\|D9 n°12" "$R"     # attendu : > 0
grep -n "mono-fenêtre\|mono-fenetre" "$R"            # doit désormais toucher le §11
```

- [ ] **Step 5 : commit**

```bash
git add docs/superpowers/plans/2026-08-07-multifenetres-solder-la-branche-resultats.md
git commit -m "corrige(d10): le §11 comptait sept legs pour huit, et deux de D9 n'en etaient jamais sortis"
```

---

# Famille ① — rendre le son au cas majoritaire (legs 4 et 6)

### Task 2 : leg 6 — la cause du refus cesse d'être jetée à l'écriture

**Files:**
- Modify: `agent/src/transport/piste_audio.rs` (le `tracing::warn!` du bras `Err`)

**Interfaces:**
- Consumes: rien. **Parallélisable avec les tâches 1, 5, 6, 7.**
- Produces: un journal de refus qui porte la chaîne de causes.

- [ ] **Step 1 : établir que la cause EXISTE et qu'elle est perdue**

```bash
cd agent
grep -n '%erreur,' src/transport/piste_audio.rs
grep -n 'with_context' src/windows_audio.rs | head -3
```
Attendu (relevé du 19 août 2026) : `piste_audio.rs:325` porte `%erreur,` ; et
`windows_audio.rs:180` pose
`.with_context(|| format!("ouverture du process loopback du PID {pid}"))`.
**Le contexte externe existe, donc `%erreur` masque tout ce qui est en dessous.**

- [ ] **Step 2 : le ROUGE — et il est DÉJÀ VERSÉ, pas à provoquer**

```bash
grep -an "reconstruction de la capture audio refusée" \
  docs/superpowers/plans/journaux-multifenetres-d10/agent-critere-2-1-plat.log
```
Attendu, à la ligne **97** du journal (relevé du 19 août 2026) :

```
WARN agent::transport::piste_audio: reconstruction de la capture audio refusée
  erreur=ouverture du process loopback du PID 27544 restantes=2
```

**Aucune cause.** C'est la pièce rouge de cette tâche, et c'est une pièce
**historique versée dans git**, pas une reconstitution.

⚠️ **Honnêteté du contrôle, à ne pas maquiller** : il n'existe **aucun test
d'hôte capable de rendre ce site rouge**, le `warn!` n'étant pas observable sans
un collecteur `tracing`. Le vert de cette tâche **ne s'établira que sur la VM**,
et **seulement si un refus survient** pendant les recettes ② ou ③ — ce que rien
ne garantit. **C'est déclaré ici, à l'avance** : si aucun refus ne survient, le
leg 6 reste ouvert, mais **observable**, ce qu'il n'était pas.

- [ ] **Step 3 : le test qui vaut, et ce qu'il éprouve exactement**

Ajouter dans le module de tests de `piste_audio.rs` un test **du format**, pas du
site — et **dire dans son commentaire que c'est ce qu'il est** :

```rust
/// Éprouve le FORMAT, pas le site d'appel : `{erreur:#}` rend la chaîne de
/// causes là où `{erreur}` ne rend que le contexte le plus externe. Le site
/// lui-même n'est pas observable sur l'hôte (c'est un `warn!` de `tracing`) ;
/// sa preuve est le journal de recette, pas ce test.
#[test]
fn le_format_diese_rend_la_chaine_de_causes() {
    let cause = anyhow::anyhow!("0x88890004");
    let e = Err::<(), _>(cause)
        .context("ouverture du process loopback du PID 42")
        .unwrap_err();
    assert!(!format!("{e}").contains("0x88890004"), "le Display simple perd la cause");
    assert!(format!("{e:#}").contains("0x88890004"), "{{:#}} doit la rendre");
}
```

**Le voir ROUGE** est ici trivial et doit tout de même être fait : inverser
provisoirement les deux assertions, lancer, observer
`assertion failed: le Display simple perd la cause`, remettre.

- [ ] **Step 4 : le remède, une ligne, et sa raison auprès d'elle**

Remplacer `%erreur,` par `erreur = format!("{erreur:#}"),` et poser au-dessus un
commentaire disant **pourquoi** :

> `{erreur:#}` et non `%erreur` : le `Display` simple d'`anyhow` ne rend que le
> contexte le plus externe, et `windows_audio.rs` en pose justement un —
> le HRESULT, seule donnée qui réponde au leg 6 de D10, restait dans les causes.
> Même doctrine que `diagnostics/multifenetre/plafond/sonde.rs`, qui l'explique
> mot pour mot sur un HRESULT perdu de la même façon.

- [ ] **Step 5 : vérifier**

```bash
cd agent && cargo test -p agent 2>&1 | tail -3
```
Attendu : **459 passed, 0 failed** (458 + 1). **Annoncé avant d'être mesuré.**

```bash
cd agent && cargo check --target x86_64-pc-windows-gnu 2>&1 | tail -3
wc -l src/transport/piste_audio.rs        # porte à 480 : sous le seuil ?
```

- [ ] **Step 6 : commit**

```bash
git add agent/src/transport/piste_audio.rs
git commit -m "corrige(d11): la cause du refus de reconstruction cesse d'etre jetee a l'ecriture"
```

---

### Task 3 : leg 4 — le son revient au cas majoritaire

**Files:**
- Modify: `agent/src/transport/piste_audio.rs` (l'accesseur)
- Modify: `agent/src/demarrage/audio.rs` (l'appel, **branche `None` seule**)
- Modify: `agent/src/transport.rs` (la doc de `audio_porteuse`, **à somme nulle ou négative**)
- Modify: `agent/src/transport/tick/tests/audio.rs` (le test)

**Interfaces:**
- Consumes: la tâche 2 (même fichier — **séquentiel, pas parallèle**).
- Produces: `pub fn Session::set_audio_porteuse(&mut self, porteuse: bool)`.

- [ ] **Step 1 : relire la chaîne du défaut, dans le code et non dans la spec**

```bash
cd agent
grep -n "audio_porteuse" src/transport.rs src/transport/piste_audio.rs
grep -rn "fn audio_a_appliquer" src/
sed -n '33,45p' src/demarrage/audio.rs
```
Attendu (relevé du 19 août 2026), et **chaque maillon doit être revu** :
`transport.rs:293` déclare le champ **sans `pub`** ; `transport.rs:386` le naît
`false` ; `piste_audio.rs:183` est son **unique** écrivain (dans
`appliquer_audio`) ; `transport/tick.rs:258` garde l'unique appelant par
`self.source.audio_a_appliquer()` ; `source.rs:137` rend `None` par défaut et
`capteur/distante.rs:372` est **sa seule surcharge**. **Un agent mono-fenêtre n'a
pas de `SourceDistante`.**

- [ ] **Step 2 : écrire le test, et LE VOIR ROUGE**

Dans `tick/tests/audio.rs`, à côté de ses deux jumeaux existants
(`une_session_porteuse_reconstruite_recoit_set_actif_true` et
`une_session_non_porteuse_reconstruite_reste_muette`) :

```rust
/// Le leg 4 de D10 : en mono-fenêtre, `audio_porteuse` n'a AUCUN écrivain —
/// `appliquer_audio` n'est atteinte que par un ordre du capteur, qu'un agent
/// mono-fenêtre ne reçoit jamais. L'accesseur public est le seul chemin par
/// lequel `demarrage/audio.rs` (hors du module `transport`) peut le dire.
#[test]
fn l_accesseur_public_rend_une_session_porteuse_et_sa_reconstruction_audible() {
    let mut session = session_d_essai();
    session.set_audio_source(Box::new(SourceMorte::new()));
    let actif_recu = std::sync::Arc::new(std::sync::Mutex::new(None));
    let observe = actif_recu.clone();
    session.set_audio_reconstructeur(Box::new(move || {
        Ok(Box::new(SourceVivante::observant_actif(observe.clone()))
            as Box<dyn AudioSource + Send>)
    }));

    // Le geste du mode mono-fenêtre, par l'accesseur PUBLIC et non par le
    // champ privé — c'est cette voie-là que `demarrage/audio.rs` empruntera.
    session.set_audio_porteuse(true);

    session.reconstruire_ou_signaler(std::time::Instant::now());

    assert_eq!(*actif_recu.lock().unwrap(), Some(true));
}
```

**Le rouge attendu, avant l'implémentation** :

```
error[E0599]: no method named `set_audio_porteuse` found for struct `Session`
```

⚠️ **Un rouge de COMPILATION est le plus faible des rouges, et il faut le
dire.** Il prouve que l'accesseur n'existait pas, **pas** que le défaut existait.
**La preuve du défaut est ailleurs, et elle est double** : (a) la lecture de code
du Step 1, qui est un argument de flot de contrôle complet ; (b) le **ROUGE VM de
la recette ①** (tâche 9), joué sur le binaire de `main` (`df03fc6`), qui est le
seul contrôle capable de montrer le silence. Ne pas présenter ce test comme la
preuve du leg.

⚠️ **Et `une_session_non_porteuse_reconstruite_reste_muette` doit rester VERT
SANS ÊTRE TOUCHÉ.** C'est la vérification que le correctif **ne déborde pas** :
il n'écrit `audio_porteuse` qu'au branchement mono-fenêtre, jamais dans le chemin
de reconstruction. Si ce test devient rouge, **le correctif est faux** — c'est
le défaut *pire* que `piste_audio.rs:265-269` décrit (une fuite de son vers une
fenêtre qui doit se taire).

- [ ] **Step 3 : l'accesseur, auprès de ses jumeaux**

Dans `piste_audio.rs`, entre `set_audio_source` (l. 26) et
`set_audio_reconstructeur` (l. 227) — au plus près du second :

```rust
/// Déclare que cette session porte le son **sans qu'aucun capteur ne le lui
/// dise**. Réservé au mode MONO-FENÊTRE.
///
/// ⚠️ **Ne JAMAIS appeler depuis une session servie par un capteur.** Le
/// capteur arbitre qui porte le son entre les fenêtres d'un même groupe de
/// PID, et `appliquer_audio` est le seul chemin légitime dans ce mode. Poser
/// `true` ici sur une session arbitrée ferait parler une fenêtre qui doit se
/// taire — deux fenêtres joueraient alors le même mix désynchronisé, l'écho
/// audible que le défaut F2 du sous-bloc D7 décrit.
/// `une_session_non_porteuse_reconstruite_reste_muette` le garde rouge.
pub fn set_audio_porteuse(&mut self, porteuse: bool) {
    self.audio_porteuse = porteuse;
}
```

- [ ] **Step 4 : l'appel, dans la SEULE branche `None`**

Dans `demarrage/audio.rs::brancher`, après `session.set_audio_source(...)` et
**dans la branche qui sait qu'il n'y aura jamais de capteur** :

```rust
// Mode MONO-FENÊTRE : aucun capteur n'arbitrera jamais cette session, donc
// `appliquer_audio` — l'unique écrivain de `audio_porteuse` — n'y sera jamais
// appelée. Sans cette ligne, toute capture reconstruite est remise au silence
// par le réarmement de `reconstruire_ou_signaler`, et le remède de D10 est
// inerte dans le cas MAJORITAIRE (une application, une fenêtre). Leg 4 de D10.
if config.fenetre_hwnd.is_none() {
    session.set_audio_porteuse(true);
}
```

⚠️ **`config.fenetre_hwnd.is_none()` et non un `match` recopié** : le `match` du
choix de source est **au-dessus** et a déjà consommé la valeur ; refaire un
`match` ici dupliquerait un embranchement que le fichier a déjà payé deux fois
(voir son commentaire « Le MÊME choix de mode que ci-dessus, refait à
l'identique »).

- [ ] **Step 5 : le contrôle de non-débordement, exécutable**

```bash
cd agent && grep -rn "set_audio_porteuse" src/
```
Attendu : **exactement trois** lignes — la définition dans `piste_audio.rs`,
l'appel dans `demarrage/audio.rs`, et l'appel du test. **Toute quatrième
occurrence est un débordement à justifier ou à retirer.**

⚠️ **Ce contrôle PEUT échouer** : il suffirait d'un appel glissé dans
`transport/tick.rs` ou dans le chemin `pour_processus` pour le faire passer à
quatre. C'est ce qui le distingue d'un contrôle vacueux.

- [ ] **Step 6 : corriger la documentation de `transport.rs`, à somme nulle**

La doc du champ `audio_porteuse` (`transport.rs`, autour de la l. 276) dit
aujourd'hui deux choses **fausses après cette tâche**, et **l'une des deux est
déjà fausse aujourd'hui** :

1. « Vrai tant que **le capteur** nous demande de porter le son » — il y aura
   désormais un second écrivain, le branchement mono-fenêtre ;
2. « **Sert UNIQUEMENT à détecter la TRANSITION** vers `actif = true` » —
   **déjà faux depuis D10** : `piste_audio.rs:318` en est un second lecteur qui
   n'est pas une transition. Chronologie établie par `git blame` (voir la table
   des divergences en tête de ce plan). **C'est un treizième commentaire
   orphelin de D10, que sa revue transverse a manqué**, et il est corrigé ici.

⚠️ **`transport.rs` est à 468 lignes, marge 32.** La réécriture doit être **à
somme nulle ou négative** :

```bash
cd agent && wc -l src/transport.rs   # avant, puis après : ne doit PAS augmenter
```
Si elle augmente, **extraire** les champs audio et leur documentation vers
`transport/piste_audio.rs` dans une tâche à part, **avant** de récrire — jamais
compresser le commentaire pour tenir le compte. *Raccourcir une réfutation pour
atteindre un compte de lignes échangerait une vérité contre un nombre.*

- [ ] **Step 7 : vérifier**

```bash
cd agent && cargo test -p agent 2>&1 | tail -3
```
Attendu : **460 passed, 0 failed** (459 + 1). **Annoncé avant d'être mesuré.**

```bash
cd agent && cargo check --target x86_64-pc-windows-gnu 2>&1 | tail -3
wc -l src/transport/piste_audio.rs src/transport/tick/tests/audio.rs src/transport.rs
```

- [ ] **Step 8 : commit**

```bash
git add agent/src/transport/piste_audio.rs agent/src/demarrage/audio.rs \
        agent/src/transport.rs agent/src/transport/tick/tests/audio.rs
git commit -m "corrige(d11): en mono-fenetre, une capture reconstruite redevient porteuse"
```

---

# Famille ② — les deux boutons qui rendent le repli atteignable (leg 5)

### Task 4 : `AUDIO_FAUTE_RECONSTRUCTION`, budget global au processus

**Files:**
- Modify: `agent/src/transport/piste_audio.rs`
- Modify: `agent/src/transport/tick/tests/audio.rs`
- Modify: `scripts/run-agent.sh`

**Interfaces:**
- Consumes: les tâches 2 et 3 (même fichier — **séquentiel**).
- Produces: `AUDIO_FAUTE_RECONSTRUCTION`, et
  `fn budget_faute_reconstruction() -> &'static AtomicU32`.

**Pourquoi dans `transport/` et pas dans `demarrage/audio.rs`** :
`demarrage/audio.rs` porte `#![cfg(windows)]` (l. 10, relevé), donc toute
injection qui y vivrait serait **inéprouvable sur l'hôte**. `transport/` est
portable et ne connaît qu'un objet de trait et une fermeture : la chaîne
« injection → refus → épuisement → `AudioMort` » y est **entièrement éprouvable
sans VM**. Précédent de lecture d'environnement dans `transport/` :
`PART_SONDAGE` (`transport/part.rs`, l. 43 relevée).

- [ ] **Step 1 : lire le modèle qu'on copie, et la leçon qu'il porte**

```bash
cd agent && sed -n '95,120p;205,220p' src/windows_audio/fil.rs
```
`AUDIO_FAUTE_LECTURE` est un `OnceLock<AtomicU32>` **par processus**, décrémenté
par `fetch_update` avec `checked_sub(1)`. Son commentaire dit **pourquoi** : un
budget par appel « n'en est pas un » — chaque capture reconstruite recevrait un
budget neuf et remourrait, et le chiffre-juge ne pourrait pas quitter zéro.
**C'est la leçon que D10 a payée sur son deuxième passage de recette.**

- [ ] **Step 2 : écrire les tests, et LES VOIR ROUGES**

⚠️ **Danger de conception à traiter AVANT d'écrire, sans quoi les tests se
poisonnent l'un l'autre** : un budget global au processus est **partagé par tous
les tests du même binaire**, et `cargo test` est multi-fils par défaut. Deux
tests qui seedent le même `AtomicU32` se voleraient leurs fautes, de façon **non
déterministe**. Deux règles, toutes deux obligatoires :

1. les tests **ne touchent jamais l'environnement** — ils seedent directement
   `budget_faute_reconstruction().store(n, Ordering::Relaxed)` ;
2. tous les tests de l'injection prennent un **verrou de test partagé** :

```rust
static VERROU_INJECTION: std::sync::Mutex<()> = std::sync::Mutex::new(());
```

Trois tests :

```rust
#[test]
fn une_faute_injectee_fait_refuser_la_reconstruction() {
    let _g = VERROU_INJECTION.lock().unwrap();
    budget_faute_reconstruction().store(1, Ordering::Relaxed);
    // … session dont le reconstructeur rendrait Ok ; une tentative ;
    // attendu : la source n'est PAS remplacée, `reconstructions_restantes`
    // a décru, et le budget d'injection est retombé à 0.
}

#[test]
fn le_budget_epuise_laisse_la_reconstruction_reussir() {
    let _g = VERROU_INJECTION.lock().unwrap();
    budget_faute_reconstruction().store(0, Ordering::Relaxed);
    // attendu : la reconstruction réussit, comme sans injection.
}

#[test]
fn un_budget_superieur_a_RECONSTRUCTIONS_MAX_mene_a_AudioMort() {
    let _g = VERROU_INJECTION.lock().unwrap();
    budget_faute_reconstruction().store(crate::audio::RECONSTRUCTIONS_MAX + 2, Ordering::Relaxed);
    // avancer le temps de RECONSTRUCTIONS_MAX × REPIT_RECONSTRUCTION,
    // attendu : `reconstruire_ou_signaler` finit par rendre `true`
    // (AudioMort à signaler), et la source reste morte.
}
```

**Le rouge attendu, avant l'implémentation** :

```
error[E0425]: cannot find function `budget_faute_reconstruction` in this scope
```

Puis, **une fois la fonction posée mais avant de brancher l'injection dans
`reconstruire_ou_signaler`**, relancer : le premier test doit échouer sur
**l'assertion**, pas sur la compilation —

```
assertion failed: la reconstruction a REUSSI alors qu'une faute etait armee
```

⚠️ **Ce second rouge est celui qui compte** ; le rouge de compilation ne prouve
que l'absence du symbole. **Faire les deux, dans cet ordre.**

- [ ] **Step 3 : le budget, lu une fois dans l'environnement**

Dans `piste_audio.rs` :

```rust
/// Le budget global au processus de fautes de reconstruction injectées.
///
/// ⚠️ **Variable de BANC, jamais une configuration livrée** — même statut que
/// `PART_SONDAGE`. Absente : désarmée, et le binaire se comporte exactement
/// comme celui de D10.
///
/// **Global au processus, jamais par appel** : c'est la leçon que le sous-bloc
/// D10 a payée sur `AUDIO_FAUTE_LECTURE` (son §10) — un budget relu à chaque
/// tentative se réapprovisionne indéfiniment, et le contrôle qu'il sert devient
/// structurellement incapable de bouger.
fn budget_faute_reconstruction() -> &'static std::sync::atomic::AtomicU32 {
    static BUDGET: std::sync::OnceLock<std::sync::atomic::AtomicU32> =
        std::sync::OnceLock::new();
    BUDGET.get_or_init(|| {
        let n: u32 = std::env::var("AUDIO_FAUTE_RECONSTRUCTION")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        if n > 0 {
            tracing::warn!(
                fautes_a_injecter = n,
                "injection de fautes de RECONSTRUCTION audio ARMEE : banc, jamais une configuration livrée"
            );
        }
        std::sync::atomic::AtomicU32::new(n)
    })
}
```

- [ ] **Step 4 : brancher l'injection, AVANT l'appel au reconstructeur**

Dans `reconstruire_ou_signaler`, au point exact où le reconstructeur est
invoqué, sur le modèle de `fil.rs` :

```rust
let tentative = if budget_faute_reconstruction()
    .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1))
    .is_ok()
{
    Err(anyhow::anyhow!("faute injectée (AUDIO_FAUTE_RECONSTRUCTION)"))
} else {
    (reconstructeur)()
};
```

⚠️ **Le bras `Err` doit rester CELUI DE LA TÂCHE 2** : la faute injectée passe
par le même `tracing::warn!` avec `{erreur:#}`, et le journal de recette portera
donc `erreur=faute injectée (AUDIO_FAUTE_RECONSTRUCTION)`. **C'est aussi le
contrôle d'atteignabilité du leg 6** : si cette chaîne n'apparaît pas au journal
alors que l'injection est armée, c'est le format qui ne marche pas, pas la cause
qui manque.

- [ ] **Step 5 : ajouter la variable à `scripts/run-agent.sh`**

```bash
grep -c AUDIO_FAUTE_RECONSTRUCTION scripts/run-agent.sh   # AVANT : attendu 0
```
Poser la ligne **immédiatement après** celle de `AUDIO_FAUTE_LECTURE` (l. 42,
relevée) :

```sh
${AUDIO_FAUTE_RECONSTRUCTION:+\$env:AUDIO_FAUTE_RECONSTRUCTION = '$AUDIO_FAUTE_RECONSTRUCTION'}
```

```bash
grep -c AUDIO_FAUTE_RECONSTRUCTION scripts/run-agent.sh   # APRÈS : attendu 1
```
⚠️ **Ce contrôle passe de 0 à 1 : il peut donc échouer, et c'est ce qui en fait
un contrôle.** Le piège a été payé quatre fois (D1, D2, D6, D7) : sans cette
ligne, l'agent démarre sans la variable **et sans rien signaler**.

- [ ] **Step 6 : vérifier**

```bash
cd agent && cargo test -p agent 2>&1 | tail -3
```
Attendu : **463 passed, 0 failed** (460 + 3). **Annoncé avant d'être mesuré.**

```bash
cd agent && cargo check --target x86_64-pc-windows-gnu 2>&1 | tail -3
wc -l src/transport/piste_audio.rs src/transport/tick/tests/audio.rs
```
⚠️ **Porte à 480** sur les deux : si franchie, extraire vers les points de chute
nommés (`transport/piste_audio/injection.rs`,
`transport/tick/tests/audio/injection.rs`) **dans une tâche à part, avant de
continuer**.

- [ ] **Step 7 : commit**

```bash
git add agent/src/transport/piste_audio.rs agent/src/transport/tick/tests/audio.rs scripts/run-agent.sh
git commit -m "banc(d11): AUDIO_FAUTE_RECONSTRUCTION, budget global au processus"
```

---

### Task 5 : `AUDIO_FAUTE_LECTURE_MS`, et le prédicat pur qui la rend testable

**Files:**
- Modify: `agent/src/audio.rs` (le prédicat **pur** et ses tests)
- Modify: `agent/src/windows_audio/fil.rs` (la fenêtre de temps)
- Modify: `scripts/run-agent.sh`

**Interfaces:**
- Consumes: rien de la famille ① — **parallélisable avec les tâches 2 et 3**.
  Doit précéder la recette ③ (tâche 11).
- Produces: `pub fn audio::injection_encore_armee(...) -> bool`.

**Pourquoi cette variable existe** — l'arithmétique, à partir de constantes
**relevées** :

```bash
cd agent
grep -n "const POLL_INTERVAL\|const REPORT_INTERVAL" src/windows_audio.rs
grep -n "pub const LECTURES_ECHOUEES_MAX\|pub const RECONSTRUCTIONS_MAX\|pub const REPIT_RECONSTRUCTION" src/audio.rs
grep -n "const PERIODE_REARBITRAGE" src/capteur/sommeil.rs
```
Relevé du 19 août 2026 : `POLL_INTERVAL = 5 ms`, `REPORT_INTERVAL = 30 s`,
`LECTURES_ECHOUEES_MAX = 10`, `RECONSTRUCTIONS_MAX = 3`,
`REPIT_RECONSTRUCTION = 2 s`, `PERIODE_REARBITRAGE = 250 ms`.

Un enfant **hérite l'environnement** du superviseur — `superviseur/lanceur.rs`
ne retire que `SUPERVISEUR`, `TEST_FILE`, `WINDOW_TITLE` et `CAPTEUR` (vérifié :
`grep -n "env_remove" src/superviseur/lanceur.rs`). Chaque fenêtre étant son
propre processus, **chaque enfant reçoit un budget `AUDIO_FAUTE_LECTURE` neuf**.
Or seule la fenêtre **porteuse** consomme des fautes : le garde
`if !emettait { … continue; }` (`fil.rs:196`) fait qu'une source muette n'appelle
jamais `read()`. **La voisine reste donc intacte tant qu'elle se tait, et meurt
en ≈ 50 ms (10 × 5 ms) dès qu'elle est promue**, sur son budget resté plein —
soit **1/600ᵉ** de `REPORT_INTERVAL`. **Le critère ③ resterait non démontrable.**

Borner l'armement dans le temps referme cela **sans nommer aucune session** :
la porteuse consomme dans les premières millisecondes, la fenêtre se referme, et
la voisine — promue au plus tôt `RECONSTRUCTIONS_MAX × REPIT_RECONSTRUCTION`
= **6 s** plus tard, plus `PERIODE_REARBITRAGE` — lit pour de vrai.
**Aucune reconnaissance préalable, aucun appel Win32 de plus.**

- [ ] **Step 1 : écrire le prédicat PUR et ses tests, et les VOIR ROUGES**

Dans `audio.rs` (portable, déjà testé sur l'hôte), poser d'abord une version
**délibérément fausse** :

```rust
pub fn injection_encore_armee(_depuis: Duration, _fenetre: Option<Duration>) -> bool {
    true // stub : à remplacer, sert à voir les tests rouges
}
```

et ses tests :

```rust
#[test]
fn sans_fenetre_l_injection_reste_armee_indefiniment() {
    assert!(injection_encore_armee(Duration::from_secs(3600), None));
}
#[test]
fn dans_la_fenetre_l_injection_est_armee() {
    assert!(injection_encore_armee(Duration::from_millis(500), Some(Duration::from_secs(3))));
}
#[test]
fn passe_la_fenetre_l_injection_est_desarmee() {
    assert!(!injection_encore_armee(Duration::from_secs(6), Some(Duration::from_secs(3))));
}
#[test]
fn la_borne_de_la_fenetre_est_incluse_ou_exclue_mais_dite() {
    // Choix explicite : `depuis < fenetre`. À la borne exacte, DÉSARMÉE.
    assert!(!injection_encore_armee(Duration::from_secs(3), Some(Duration::from_secs(3))));
}
```

**Le rouge attendu, sur le stub** :

```
assertion failed: !injection_encore_armee(...)   (passe_la_fenetre…)
```
soit **deux tests rouges sur quatre** — les deux qui exigent le désarmement.
⚠️ **C'est la vérification d'atteignabilité** : un stub qui rendrait `false`
rendrait rouges les deux autres. Les quatre tests **ne peuvent pas être tous
verts sur un stub constant**, quelle que soit la constante.

Puis implémenter :

```rust
pub fn injection_encore_armee(depuis: Duration, fenetre: Option<Duration>) -> bool {
    match fenetre {
        None => true,
        Some(f) => depuis < f,
    }
}
```

- [ ] **Step 2 : câbler dans `fil.rs`**

Au voisinage du `OnceLock` de `AUDIO_FAUTE_LECTURE` (l. 107-118), poser :

- un `OnceLock<Option<Duration>>` lisant `AUDIO_FAUTE_LECTURE_MS` (absente =
  `None`, donc **illimité, comportement de D10 strictement préservé**) ;
- un `Instant` capturé **à l'initialisation du budget**, pas au premier `read()`
  — c'est ce qui rend l'origine identique pour la porteuse et pour la voisine,
  leurs deux processus démarrant ensemble ;
- au point d'injection (l. ~211), remplacer la condition seule par
  `crate::audio::injection_encore_armee(origine.elapsed(), fenetre) && fetch_update(...)`.

⚠️ **L'ordre des opérandes du `&&` compte** : le `fetch_update` **décrémente**.
Le mettre en second garantit qu'aucune faute n'est consommée une fois la fenêtre
fermée. Écrire ce pourquoi dans un commentaire auprès de la ligne.

⚠️ **Trace, seulement si armée** : ajouter `fenetre_ms` au `warn!` existant
d'armement de `AUDIO_FAUTE_LECTURE`, pour que le journal dise laquelle des deux
configurations tourne. **Ne pas créer un second `warn!`** : deux traces au même
instant se comptent comme deux événements (piège maison de D6).

- [ ] **Step 3 : ajouter la variable à `scripts/run-agent.sh`**

```bash
grep -c AUDIO_FAUTE_LECTURE_MS scripts/run-agent.sh   # AVANT : 0
```
⚠️ **Attention au préfixe commun** : `${AUDIO_FAUTE_LECTURE:+…}` et
`${AUDIO_FAUTE_LECTURE_MS:+…}` sont deux expansions distinctes et ne se
masquent pas — mais les **relire toutes deux** après édition, `grep -n
'AUDIO_FAUTE' scripts/run-agent.sh` devant rendre **trois** lignes (la variable
de D10, celle de la tâche 4, celle-ci).

- [ ] **Step 4 : vérifier**

```bash
cd agent && cargo test -p agent 2>&1 | tail -3
```
Attendu : **467 passed, 0 failed** (463 + 4). **Annoncé avant d'être mesuré.**

```bash
cd agent && cargo check --target x86_64-pc-windows-gnu 2>&1 | tail -3
wc -l src/audio.rs src/windows_audio/fil.rs
```

- [ ] **Step 5 : commit**

```bash
git add agent/src/audio.rs agent/src/windows_audio/fil.rs scripts/run-agent.sh
git commit -m "banc(d11): AUDIO_FAUTE_LECTURE_MS borne l'injection dans le temps"
```

---

# Famille ⑥ — les deux legs de D9 tombés du registre

### Task 6 : les deux tests faibles, et l'invariant non écrit

**Files:**
- Modify: `agent/src/windows_source/telemetrie.rs`
- Modify: `client/src/resize.test.ts`
- Modify: `client/src/main.ts` (commentaire seul)

**Interfaces:**
- Consumes: rien. **Entièrement parallélisable** avec les tâches 1, 2, 4, 5, 7.
- Produces: rien de public. Aucune VM.

- [ ] **Step 1 : D9 n°11 — `une_telemetrie_neuve_est_a_zero`**

```bash
cd agent && sed -n '55,72p' src/windows_source/telemetrie.rs
```
Relevé du 19 août 2026 : le test est
`assert_eq!(Telemetrie::default().lire(), (0, 0, 0));` — il n'éprouve que
`#[derive(Default)]`, jamais `tick`/`capturee`/`produite`. La logique propre est
déjà couverte par `deux_telemetries_ne_se_melangent_pas`, juste au-dessus.

**Décision, et elle est écrite dans le test lui-même** : le réécrire pour qu'il
éprouve **la remise à zéro APRÈS usage**, ce que rien ne couvre :

```rust
/// Une télémétrie remise à neuf repart de zéro — et le test le PROUVE en
/// l'ayant d'abord fait compter. La version d'avant (leg n°11 de D9)
/// n'éprouvait que `#[derive(Default)]`, et ne pouvait pas rendre l'autre
/// valeur : elle passait quelle que soit la logique de `tick`/`capturee`.
#[test]
fn une_telemetrie_remise_a_neuf_repart_de_zero() {
    let mut t = Telemetrie::default();
    t.tick();
    t.capturee();
    t.produite();
    assert_ne!(t.lire(), (0, 0, 0), "précondition : elle a bien compté");
    t = Telemetrie::default();
    assert_eq!(t.lire(), (0, 0, 0));
}
```

**Le voir ROUGE, et l'état à provoquer est nommé** : rendre `tick()` no-op
(commenter son corps), relancer — attendu :

```
assertion `left != right` failed: précondition : elle a bien compté
```
⚠️ **L'ancien test, lui, resterait VERT sous ce même sabotage** : c'est ce qui
établit qu'il ne pouvait pas rendre l'autre valeur. **Faire les deux mesures**,
l'ancienne et la neuve, sur le même sabotage — c'est la seule façon de prouver
la faiblesse plutôt que de l'affirmer. Puis restaurer `tick()`.

- [ ] **Step 2 : D9 n°11 — `client/src/resize.test.ts`**

```bash
grep -n "readyState\|channel\|canal" client/src/resize.ts
```
Relevé : **une seule** ligne, et c'est un commentaire de documentation.
`RejeuResize` **n'a aucune notion de canal**.

**Décision : changer le TITRE, pas le test.** `RejeuResize` est délibérément pur
et sans DOM ; lui donner une notion de canal serait un couplage neuf **pour un
test**. Le titre devient ce qu'il éprouve réellement :

```ts
it("rend la taille observée tant qu'aucune émission n'a été confirmée", () => {
    // ⚠️ Ce test n'exerce AUCUN état de canal : `RejeuResize` est pur et n'en
    // connaît aucun (leg n°11 de D9 — son titre annonçait « quand le canal
    // était fermé », état qu'il ne pouvait pas atteindre). L'état de canal
    // vit dans `main.ts`, chez `emettreSiPossible`, et c'est là qu'il se
    // teste — pas ici.
```

⚠️ **Le commentaire du corps du test dit déjà « le cas du leg 10 … `readyState
!== 'open'` a fait abandonner l'envoi »** : il est **conservé comme motivation**
du mécanisme, et **explicitement marqué comme motivation** — pas comme ce que le
test éprouve. La distinction est l'objet même de ce leg.

- [ ] **Step 3 : D9 n°12 — l'invariant synchrone de `main.ts`**

```bash
grep -n "addEventListener('open', emettreSiPossible)" client/src/main.ts
grep -n "^    .then((session)" client/src/main.ts
```
Relevé du 19 août 2026 : la ligne est à **386**, le `.then()` qui la pose à
**204**. ⚠️ **Ne recopier ni l'un ni l'autre dans le commentaire** — le premier a
déjà bougé de `:345` à `:386` depuis D9.

Poser au-dessus de la ligne un commentaire qui dit le **pourquoi de l'ordre**,
pas seulement le quoi :

```ts
// Le rejeu : à l'ouverture du canal, la taille retenue repart.
//
// ⚠️ INVARIANT NON ÉVIDENT (leg n°12 de D9) : ce `.then()` doit s'exécuter
// INTÉGRALEMENT DE FAÇON SYNCHRONE, sans `await` intercalé entre la
// construction de `rejeu` / du `ResizeObserver` ci-dessus et cet
// `addEventListener`. Un `await` glissé là rendrait la main à la boucle
// d'événements : si le canal s'ouvrait pendant l'attente, l'écouteur serait
// posé APRÈS l'événement `open`, il ne serait jamais appelé, et le rejeu
// serait rompu EN SILENCE — aucune erreur, aucun log, juste une taille
// perdue. C'est exactement le mode de défaillance que le rejeu existe pour
// réparer.
```

**Sur le test qui le garderait** : il exigerait de simuler `RTCDataChannel` et
tout le cycle de `createSession`. **Critère de décision, écrit ici** — un test
n'est ajouté que s'il peut être **vu rouge** en insérant un `await
Promise.resolve()` avant l'`addEventListener`, **sans** mocker la session
entière. Si ce rouge n'est pas obtenable à coût raisonnable, **le commentaire
seul est livré, et le plan le déclare** — un test qu'on ne peut pas voir rouge
n'aurait rien ajouté à ce que le commentaire dit déjà.

- [ ] **Step 4 : vérifier**

```bash
cd agent && cargo test -p agent 2>&1 | tail -3     # attendu : 467, inchangé
cd client && npx vitest run 2>&1 | tail -4          # attendu : 107, inchangé
```
⚠️ **Les deux comptes doivent être INCHANGÉS** : cette tâche remplace des tests,
elle n'en ajoute pas (sauf si le test optionnel du Step 3 est livré, auquel cas
le compte client passe à 108 et il faut le dire).

- [ ] **Step 5 : commit**

```bash
git add agent/src/windows_source/telemetrie.rs client/src/resize.test.ts client/src/main.ts
git commit -m "corrige(d11): deux tests incapables de rendre l'autre valeur, et l'invariant non ecrit du rejeu"
```

---

# Famille instrument — ce qui rend les recettes possibles

### Task 7 : la mire porte un marqueur d'identité invariant dans le temps

**Files:**
- Create: `docs/superpowers/plans/journaux-multifenetres-d11/instrument/anim-d11.html`

**Interfaces:**
- Consumes: rien. **Parallélisable.**
- Produces: la mire des recettes ① à ⑤, et le prédicat de distinction.

**Aucun fichier de code n'est touché.** Le répertoire `docs/` est exempté de la
règle des 500 lignes.

- [ ] **Step 1 : établir POURQUOI le contrôle de D10 ne peut pas échouer**

```bash
sed -n '188,208p' docs/superpowers/plans/journaux-multifenetres-d10/instrument/pilote-critere1-d10.mjs
grep -n "URLSearchParams" docs/superpowers/plans/journaux-multifenetres-d10/instrument/anim-d4.html
```
Relevé : l'empreinte est un sous-échantillonnage **8×8 de l'élément `<video>`
vivant**, relevé page par page par des allers-retours CDP indépendants
(étalement mesuré par D10 : 187 et 245 ms), sur une source dont le fond **dérive
à chaque trame**. **Deux pages décodant le MÊME flux rendraient donc des
empreintes différentes elles aussi** — le contrôle ne peut pas signaler une
collision.

Et `anim-d4.html:19` porte déjà `const n = new URLSearchParams(location.search).get('n') || '0'`.

- [ ] **Step 2 : partir d'`anim-d4.html`, VERBATIM, puis ajouter**

Copier `journaux-multifenetres-d10/instrument/anim-d4.html` sans en changer une
ligne, puis **ajouter seulement** :

- un **aplat de couleur à position fixe** — coin haut-gauche, 64×64 px CSS —
  dont la couleur est dérivée **déterministement de `n`** (par exemple
  `hsl(n × 47 mod 360, 100%, 50%)`, l'incrément 47 étant premier avec 360 pour
  écarter au maximum dix identités voisines) ;
- **il est repeint à chaque trame comme le reste**, mais **sa couleur ne dépend
  pas du temps** : c'est toute la propriété.

⚠️ **Le fond continue de dériver** : Desktop Duplication n'émet une trame qu'au
changement du bureau, et une mire immobile ferait mesurer zéro image. **Le piège
de la mire immobile n'est pas réintroduit** — il est nommé ici pour qu'aucune
« simplification » ne le réintroduise.

- [ ] **Step 3 : le prédicat de distinction, dans le pilote**

Le pilote n'échantillonne **que le patch** : lire le pixel au centre du carré
(coordonnées **dérivées de `videoWidth`/`videoHeight`**, pas codées en dur — le
flux peut être à n'importe quel barreau de l'échelle), et rendre son triplet RGB
quantifié. **Deux pages sur le même flux rendent alors le même marqueur, par
construction.**

- [ ] **Step 4 : le ROUGE, et il est déterministe et sans produit**

⚠️ **Il se joue AVANT toute recette, et sur l'hôte seul** : ouvrir **deux pages
locales** `anim-d11.html?n=3` et `anim-d11.html?n=3` — **même `n`** — et
vérifier que le prédicat **signale une collision**. Puis `n=3` et `n=4` :
distinct. **Ce rouge ne dépend d'aucun flux WebRTC, d'aucune VM et d'aucun
produit** : c'est un contrôle de l'instrument par lui-même, et il **doit** être
joué, faute de quoi la recette ④ mesurerait avec un prédicat non éprouvé.

Verser la transcription des deux tirages avec l'instrument.

- [ ] **Step 5 : commit**

```bash
git add docs/superpowers/plans/journaux-multifenetres-d11/instrument/
git commit -m "instrument(d11): la mire porte un marqueur d'identite invariant dans le temps"
```

---

### Task 8 : les pilotes, dont celui qui collecte enfin la console

**Files:**
- Create: `…/instrument/pilote-audio-d11.mjs`, `pilote-flux-d11.mjs`,
  `pilote-cout-d11.mjs`, `pilote-resize-d11.mjs`

**Interfaces:**
- Consumes: la tâche 7 (la mire).
- Produces: les quatre pilotes des recettes.

- [ ] **Step 1 : partir des pilotes de D10, et relever ce qui leur manque**

```bash
ls docs/superpowers/plans/journaux-multifenetres-d10/instrument/
grep -n "Runtime.enable\|Runtime.consoleAPICalled" docs/superpowers/plans/journaux-multifenetres-d10/instrument/*.mjs
```
Relevé du 19 août 2026 : les **trois** pilotes appellent `Runtime.enable`
(`pilote-critere1-d10.mjs:278`, `pilote-critere2-d10.mjs:317`,
`pilote-ab-d10.mjs:351`) et **aucun ne s'abonne à `Runtime.consoleAPICalled`** —
`grep` rend **zéro** occurrence. **Les `console.debug` de la page ne sont
collectés nulle part.**

- [ ] **Step 2 : l'abonnement, page par page**

Dans `pilote-resize-d11.mjs`, après `Runtime.enable` **et pour chaque
`sessionId`** :

```js
cdp.on('Runtime.consoleAPICalled', (p, sessionId) => {
    journalConsole.push({ t: Date.now(), sessionId, type: p.type,
        args: p.args.map(a => a.value ?? a.description ?? a.preview) });
});
```
Le flux de console est **versé avec les journaux**, un fichier par exécution.

- [ ] **Step 3 : le CONTRÔLE D'ATTEIGNABILITÉ de la collecte — obligatoire**

⚠️ **C'est la ligne la plus importante de cette tâche.** L'issue n°1 de la grille
de `main.ts` se lit sur une **absence de log** — et une absence de log est
exactement ce que produit aussi un pilote qui ne collecte pas la console.
**Les deux se lisent pareil**, et sans ce contrôle D11 rejouerait le naufrage de
F1 (D7), où un contrôle ne pouvait structurellement pas dénoncer ce qu'il
existait pour dénoncer.

Le contrôle, joué **sur l'hôte, sans la VM** :

```js
await cdp.send('Runtime.evaluate',
    { expression: "console.debug('[sonde collecte] balise', {a:1})" }, sessionId);
// puis : le journal de console DOIT porter cette ligne.
```
**Si la balise ne ressort pas, la recette ⑥ est ANNULÉE** — pas interprétée. Une
absence de log ne devient une information qu'une fois cette balise ressortie.

- [ ] **Step 4 : les invariants de montage, hérités et non renégociés**

Écrits dans l'en-tête de chaque pilote :

- Chrome `--app`, mire animée à **cadence connue et affichée par la mire
  elle-même** ; sans ce chiffre, une capture lente et une source lente se lisent
  pareil ;
- **un `--user-data-dir` par fenêtre** — **SAUF** pour les recettes ② et ③, qui
  exigent au contraire un `--user-data-dir` **partagé** pour obtenir un
  `chrome.exe` unique ;
- **relever les PID, jamais les supposer d'un nom d'exécutable** :
  `node scripts/winrm.js 'Get-Process chrome | Select-Object Id,MainWindowTitle | Format-List'`,
  et croiser avec les lignes `audio activé … pid=` de l'agent lui-même
  (deux voies indépendantes, comme D8) ;
- `--disable-popup-blocking`, `--disable-background-timer-throttling`,
  `--disable-backgrounding-occluded-windows`, `--disable-renderer-backgrounding` —
  sans les trois derniers, **une page jamais au premier plan gèle au bout de
  5 minutes** ;
- **navigateur pilote sur l'HÔTE, jamais sur la VM** : sur la VM la fenêtre de la
  page-shell est elle-même capturée par le superviseur, ce qui boucle en cascade
  d'ouvertures ;
- **aucune capture d'écran CDP pendant une mesure**, et **toute évaluation CDP
  sur une page portant un flux WebRTC actif doit être BORNÉE** — elle peut ne
  **jamais** rendre ;
- le verdict audio se juge à la **fréquence dominante** (`AnalyserNode`),
  **jamais au compte d'octets** : D7 a relevé `bytesReceived` en croissance sur
  un spectre à −1000 dB. Le seuil se compare **à la dominante**, jamais au
  plancher de bruit — le seuil de D10 comparait au plancher (−158 dB) et **ne
  pouvait quasiment pas échouer**.

- [ ] **Step 5 : commit**

```bash
git add docs/superpowers/plans/journaux-multifenetres-d11/instrument/
git commit -m "instrument(d11): quatre pilotes, dont celui qui collecte enfin la console"
```

---

# Les recettes — sur la VM, strictement sérialisées

**Préambule commun à toutes les tâches 9 à 14**, à jouer **à chaque fois** :

```bash
virsh list --all
virsh start Windows
until timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null; do sleep 5; done
until ls /media/vm/dev >/dev/null 2>&1; do sleep 5; done
set -a && source .env && set +a
node scripts/winrm.js 'Get-Process agent -ErrorAction SilentlyContinue | Format-List Id,StartTime'
```

⚠️ **`Get-Process agent` se revérifie APRÈS chaque tentative, y compris
échouée** : un superviseur resté vivant empêche le nouveau `StreamWriter`
d'ouvrir `agent.log`, et **la copie relue est celle, périmée, de la tentative
précédente** — on mesure le run d'avant en croyant lire le sien. Rencontré
**trois fois sur trois** en D8.

⚠️ **Vérifier la TAILLE du binaire après `scripts/build-agent.sh`** : une
compilation de 0,13 s est un aveu (un `rsync -a` qui remonte le temps fait garder
le binaire précédent ; seul `cargo clean --release -p agent` débloque).

⚠️ **Copier `agent.log` APRÈS la fin réelle de l'exécution, pas à la fin du
pilote** : les enfants meurent quand le navigateur se ferme, donc **après** la
copie. Verser chaque `agent-*.log` **avec son jumeau `-plat`**
(`sed 's/\x1b\[[0-9;]*m//g'`).

⚠️ **Purger les sorties virtuelles entre exécutions** (`MULTIFENETRE_VDD_PURGE=1`,
**dans un lancement à elle seule** — l'aiguillage de
`diagnostics/multifenetre.rs` retourne après la première sonde reconnue), sans
quoi la suivante démarre avec un vivier déjà entamé.

---

### Task 9 : recette ① — en mono-fenêtre, le son revient

**Files:**
- Create: journaux `agent-critere-1-{1,2}.log` + `-plat`, `agent-rouge-1.log` + `-plat`,
  les JSON du pilote, dans `journaux-multifenetres-d11/`

**Interfaces:**
- Consumes: les tâches 3, 4, 5, 8. **Première recette : elle ouvre la série.**
- Produces: le verdict du critère ①.

**Le critère** : en **mono-fenêtre** (aucun superviseur, aucun capteur), une
capture audio dont la lecture est tuée par injection est reconstruite **et
redevient audible**.

- [ ] **Step 1 : le ROUGE, sur le binaire de `main` — et il se joue EN PREMIER**

Compiler et mesurer **`df03fc6`** (le point de fusion de D10), avec exactement la
même injection et le même montage :

```
AUDIO_FAUTE_LECTURE=15    (> LECTURES_ECHOUEES_MAX = 10)
pas de SUPERVISEUR, pas de CAPTEUR, pas de FENETRE_HWND
session de 120 s
```

```bash
sed 's/\x1b\[[0-9;]*m//g' agent.log > agent-rouge-1-plat.log
grep -ac 'capture audio reconstruite' agent-rouge-1-plat.log        # attendu : ≥ 1
grep -a 'compteurs audio' agent-rouge-1-plat.log | grep -ac 'actif=true'   # attendu : 0
```

⚠️ **La FORME du rouge est ce qui compte, et elle est prescrite ici** : la
reconstruction **doit se déclencher** (`≥ 1`) **et** le son **doit rester absent**
(`actif=true` = **0**, dominante à −1000 dB). Un rouge où la reconstruction ne se
déclencherait pas serait **vacueux** — il se lirait pareil sur un binaire au
remède parfait dont l'injection n'aurait pas mordu. **C'est précisément le piège
que D10 a nommé** : *ce qui vaut rouge, c'est un binaire où le mécanisme est
PRÉSENT et le résultat ABSENT.*

- [ ] **Step 2 : le témoin d'armement, qui est immune au montage**

```bash
grep -ac 'faute injectée (AUDIO_FAUTE_LECTURE)' agent-rouge-1-plat.log
```
**Une source muette ne peut pas consommer de faute** — le garde `if !emettait`
précède l'injection. **La consommation de fautes est donc un témoin POSITIF
d'armement**, et il doit être **> 0** aux deux bras. C'est l'instrument que D10 a
trouvé dormant dans ses propres pièces.

- [ ] **Step 3 : le VERT, sur le binaire de D11**

Même montage, même injection, binaire de la branche. Attendu :

```bash
sed 's/\x1b\[[0-9;]*m//g' agent.log > agent-critere-1-1-plat.log
grep -ac 'capture audio reconstruite' agent-critere-1-1-plat.log            # ≥ 1
grep -a 'compteurs audio' agent-critere-1-1-plat.log | grep -ac 'actif=true'  # ≥ 2
```

**Le jugement est spectral, pas comptable** : la **fréquence dominante** reçue
doit être celle assignée à la source, plancher de bruit à l'appui.

⚠️ **Dimensionnement du palier, contre la constante RELEVÉE** :
`REPORT_INTERVAL = 30 s` (`windows_audio.rs:42`), et **le fil de capture est
recréé à la reconstruction — donc son compteur de période repart de zéro.** La
session dure **120 s**, et le verdict exige **au moins deux** lignes
`compteurs audio` **postérieures à l'horodatage de la reconstruction**. À 60 s de
session on n'en aurait qu'une, et une seule ligne ne distingue pas un son qui
tient d'un son qui reprend puis retombe. *(La leçon de D6 est exactement
celle-ci : un palier de mesure doit être plusieurs fois plus long que la
temporisation du mécanisme qu'il observe — 25 s pour un `DELAI_REMONTEE` de 20 s
avait fait imputer au produit trois échecs qui venaient du protocole.)*

Points de contrôle spectraux : **t+30 s** et **t+105 s**.

- [ ] **Step 4 : l'ordre est établi, pas supposé**

Relever l'horodatage de `capture audio reconstruite` et celui de chaque mesure
spectrale : **les mesures doivent être postérieures**. Sans cela, on mesurerait
la capture d'origine et on conclurait que le remède marche alors qu'il n'aurait
rien fait.

- [ ] **Step 5 : deuxième exécution, verser, commit**

```bash
git add docs/superpowers/plans/journaux-multifenetres-d11/
git commit -m "recette(d11): en mono-fenetre le son revient, avec son rouge sur df03fc6"
```

---

### Task 10 : recette ② — deux fenêtres d'un même PID, la voisine se tait

**Files:**
- Create: journaux `agent-critere-2-{1,2}.log` + `-plat`, `agent-rouge-2.log` + `-plat`

**Interfaces:**
- Consumes: la tâche 9 (VM sérialisée).
- Produces: le verdict du critère ②, **et la discrimination que D10 n'a pas pu
  faire**.

**Le critère** : à deux fenêtres d'un **même groupe de PID**, la porteuse
reconstruite **parle** et la voisine **se tait**, dans la **même** mesure.

**Pourquoi cette recette existe** : à une seule fenêtre, `audio_porteuse` vaut
toujours `true`, et la recette verte de D10 **ne pouvait pas distinguer** le
correctif livré (`set_actif(self.audio_porteuse)`) de la version que le code
déclare **pire** (`set_actif(true)` inconditionnel) — les deux rendraient
441 Hz. À deux fenêtres, la distinction existe sur la VM. **C'est aussi la
lacune 🔴 de couverture de D10 : « les deux familles n'ont jamais tourné
ensemble ».**

- [ ] **Step 1 : le montage, et son piège**

Deux fenêtres Chrome `--app` **partageant un `--user-data-dir`** → **un seul
`chrome.exe`**. ⚠️ **Deux `notepad.exe` seraient deux PID distincts** et le
contrôle ne pourrait alors pas échouer : c'est le défaut que le plan de D8 a
failli commettre. **Relever le PID par deux voies indépendantes** :
`Get-Process chrome` et les lignes `audio activé … pid=` de l'agent.

⚠️ **Une session WebRTC VIVANTE est requise** : `Session::run()` est la seule
boucle qui consomme l'ordre audio du capteur. Un répondeur de viewport nu fait
créer les sorties et lancer les enfants, **mais les deux fenêtres restent
`actif=false` à jamais** — faux négatif de méthode indiscernable du défaut.

Chaque fenêtre joue **sa** tonalité (par exemple 440 Hz et 660 Hz), assignée par
le pilote et **relevée dans le journal du pilote**.

- [ ] **Step 2 : le ROUGE — un binaire délibérément défectueux, jamais fusionné**

Bâtir un binaire portant **`set_actif(true)` inconditionnel** à la place de
`set_actif(self.audio_porteuse)`, **pour la seule mesure**.

```bash
cd agent && git diff --stat   # une seule ligne doit différer
scripts/build-agent.sh
ls -l /media/vm/…/agent.exe   # relever TAILLE et date
```
⚠️ **Verser sa provenance** — le hachage du commit temporaire (ou le `diff`
exact) **et** la taille du binaire, comme D10 l'a fait pour ses deux binaires.
**Ne jamais fusionner ce commit**, et le retirer de l'arbre après la mesure
(`git checkout -- agent/src/transport/piste_audio.rs`).

Attendu, sur ce binaire : **la voisine PARLE** — sa dominante est celle de la
porteuse (les deux fenêtres jouent le même mix, désynchronisé : c'est l'**écho
audible** que F2 de D7 décrit).

- [ ] **Step 3 : le VERT**

Sur le binaire de la branche, même montage, même injection
(`AUDIO_FAUTE_LECTURE=15`) :

- **porteuse** : dominante = sa tonalité assignée ;
- **voisine** : **aucune dominante** au-dessus du plancher, **dans la même
  mesure** (les deux relevés spectraux doivent être pris dans la même fenêtre de
  temps, et leurs horodatages versés).

⚠️ **Le seuil se compare à la DOMINANTE, jamais au plancher de bruit.** Le seuil
`audio_survit` de D10 comparait au plancher (−158 dB) et **ne pouvait quasiment
pas échouer** : sur son run initial, le niveau à la fréquence cible était
**79 dB sous** la dominante réelle et le seuil passait quand même.

- [ ] **Step 4 : le couplage réélection / répit, EXERCÉ et non borné**

Relever, sans en tirer de mesure : `PERIODE_REARBITRAGE = 250 ms`
(`capteur/sommeil.rs:52`, relevé) borne l'espacement des réélections **parce que
plusieurs fenêtres peuvent les déclencher** — ce que D10 n'avait jamais fait
tourner. Verser le compte de réélections et l'écart entre elles.
⚠️ **D11 l'EXERCE, il ne le BORNE pas** : le retard infligé au fil de drainage
n'est pas mesuré, et cela doit être écrit tel quel.

- [ ] **Step 5 : deuxième exécution, verser, commit**

```bash
git add docs/superpowers/plans/journaux-multifenetres-d11/
git commit -m "recette(d11): a deux fenetres d'un meme PID, la voisine reconstruite se tait"
```

---

### Task 11 : recette ③ — le repli sur la promotion

**Files:**
- Create: journaux `agent-critere-3-{1,2}.log` + `-plat`, `agent-rouge-3.log` + `-plat`

**Interfaces:**
- Consumes: les tâches 4, 5, 10 (VM sérialisée). **C'est la recette que le
  critère ④ de D10 n'a pas pu jouer.**
- Produces: le verdict du critère ③.

**Le critère** : une capture **irrécupérable** épuise son budget de
reconstruction, `AudioMort` part au capteur, et la **voisine du même groupe de
PID est promue et émet réellement**.

- [ ] **Step 1 : l'arithmétique du montage, dérivée des constantes RELEVÉES**

| Variable | Valeur | Pourquoi exactement celle-là |
| --- | --- | --- |
| `AUDIO_FAUTE_LECTURE` | **15** | `> LECTURES_ECHOUEES_MAX = 10` : la porteuse meurt |
| `AUDIO_FAUTE_LECTURE_MS` | **3000** | la fenêtre se referme à t+3 s, **avant** la promotion à t≈6,3 s |
| `AUDIO_FAUTE_RECONSTRUCTION` | **5** | `> RECONSTRUCTIONS_MAX = 3` : les trois tentatives sont refusées |

**La chronologie attendue, calculée** :

```
t+0      les deux processus démarrent (leur fil audio initialise son budget)
t+0,05 s la porteuse a consommé 10 fautes (10 × POLL_INTERVAL = 5 ms) → capture morte
t+2 s    tentative 1 de reconstruction → refusée (injection)   REPIT_RECONSTRUCTION
t+4 s    tentative 2 → refusée
t+6 s    tentative 3 → refusée, budget RECONSTRUCTIONS_MAX épuisé → AudioMort
t+6,25 s au plus tard, le capteur réarbitre (PERIODE_REARBITRAGE = 250 ms)
         → la voisine est promue
t+6,3 s  la voisine commence à émettre. Son budget AUDIO_FAUTE_LECTURE est
         INTACT (elle était muette, le garde `if !emettait` l'a protégée) —
         mais la fenêtre de 3 s est CLOSE : elle lit pour de vrai.
```

⚠️ **Sans `AUDIO_FAUTE_LECTURE_MS`, la voisine mourrait en ≈ 50 ms** sur son
budget resté plein, soit **1/600ᵉ** de `REPORT_INTERVAL` : elle disparaîtrait
avant d'avoir pu produire la moindre preuve d'écoute, et le critère resterait non
démontrable. **C'est l'incomplétude de la formulation de D10** (« un
`AUDIO_FAUTE_RECONSTRUCTION` calqué sur `AUDIO_FAUTE_LECTURE` »), et c'est
pourquoi il faut **deux** boutons.

- [ ] **Step 2 : la marge, vérifiée et non supposée**

L'origine de la fenêtre de 3 s est **l'initialisation du budget**, qui a lieu au
démarrage du fil, donc au démarrage du processus. **Les deux processus doivent
démarrer dans le même intervalle** : le pilote ouvre les deux fenêtres
d'affilée, et le journal doit le confirmer (écart entre les deux lignes
`audio activé` **< 1 s**). Si l'écart dépasse 1 s, **la marge de 3 s est à
recalculer**, ou la mesure est déclarée non prise.

- [ ] **Step 3 : le ROUGE — `AUDIO_FAUTE_RECONSTRUCTION` DÉSARMÉE**

Tout le reste identique, **la seule variable retirée**. Attendu : la
reconstruction **réussit**, `AudioMort` **n'est jamais émis**, **aucune promotion
n'a lieu**.

```bash
grep -ac 'AudioMort\|audio mort' agent-rouge-3-plat.log   # attendu : 0
grep -ac 'capture audio reconstruite' agent-rouge-3-plat.log  # attendu : ≥ 1
```
⚠️ **Ce contrôle a bien DEUX issues** : la présence d'`AudioMort` sépare le bras
armé du bras désarmé sans ambiguïté, et le second compte (`≥ 1`) écarte le rouge
vacueux — il établit que le mécanisme a bien tourné.

- [ ] **Step 4 : le VERT, et ce qu'on relève**

```bash
sed 's/\x1b\[[0-9;]*m//g' agent.log > agent-critere-3-1-plat.log
grep -ac 'faute injectée (AUDIO_FAUTE_RECONSTRUCTION)' agent-critere-3-1-plat.log  # attendu : 3
grep -a  'reconstruction de la capture audio refusée' agent-critere-3-1-plat.log   # la CAUSE doit être lisible (leg 6)
grep -ac 'AudioMort\|audio mort' agent-critere-3-1-plat.log                        # ≥ 1
grep -a  'audio activé' agent-critere-3-1-plat.log                                 # les deux PID
```
**Fermeture arithmétique à vérifier** : le nombre de fautes de reconstruction
consommées doit valoir **exactement `RECONSTRUCTIONS_MAX` = 3** (et non 5) — le
budget d'injection en garde 2, puisque le budget de reconstruction s'épuise le
premier. **Si ce compte vaut 5, le branchement de l'injection est en aval de la
garde de budget**, et c'est un défaut.

⚠️ **Le leg 6 se juge ICI** : le `warn!` de refus doit porter
`erreur=faute injectée (AUDIO_FAUTE_RECONSTRUCTION)` — chaîne complète, pas un
contexte tronqué. **C'est la première occasion de voir `{erreur:#}` à l'œuvre.**

Puis la voisine : **dominante = sa tonalité assignée**, sur au moins deux points
de contrôle postérieurs à sa promotion. **Session de 150 s** — la promotion à
t≈6,3 s, puis deux `compteurs audio` de la voisine à t≈36 s et t≈66 s, plus la
marge.

- [ ] **Step 5 : deuxième exécution, verser, commit**

```bash
git add docs/superpowers/plans/journaux-multifenetres-d11/
git commit -m "recette(d11): une capture irrecuperable retombe sur la promotion de la voisine"
```

---

### Task 12 : recette ④ — deux fenêtres montrent deux flux distincts

**Files:**
- Create: journaux `agent-critere-4-{1,2}.log` + `-plat`, `flux-{1,2}.json`,
  `flux-rouge.json`

**Interfaces:**
- Consumes: les tâches 7, 8, 11 (VM sérialisée).
- Produces: le verdict du critère ④ — **une propriété de CORRECTION du produit
  multi-fenêtres, jamais prouvée depuis D1**.

- [ ] **Step 1 : le ROUGE, joué EN PREMIER et sur la VM**

Lancer **deux fenêtres avec le MÊME `n`** (`anim-d11.html?n=3` × 2), tout le
reste identique. **Le prédicat doit signaler une collision.**

⚠️ **Ce rouge est déterministe et ne dépend pas du produit** : deux mires
identiques rendent le même marqueur, quel que soit le flux qui les porte. **Si
le prédicat ne signale rien ici, il est vacueux** et la recette est annulée —
c'est très exactement le défaut du contrôle de D10, qui ne pouvait pas échouer
sur une page vivante.

- [ ] **Step 2 : le VERT**

`n` distinct par fenêtre (`n = 1..N`). Relever le marqueur de **chaque** page, et
vérifier qu'ils sont **tous distincts deux à deux**.

⚠️ **N est ce que le produit atteint, pas ce qu'on espère** : `CAPACITE = 10`
(`superviseur/boucle.rs:60`, relevé) et `vivier::PLAFOND_EVEIL = 8`
(`capteur/vivier.rs:23`, relevé) sont **deux constantes distinctes** — jusqu'à
dix fenêtres **attachées**, dont **huit éveillées** au plus, les deux plus
anciennes dormant par LRU. **Les pages figées sont attendues et ne sont pas un
échec** : elles se relèvent au journal de l'agent
(`endormie=true cadence="0.0"`), jamais au navigateur.

Le marqueur d'une page **endormie** est celui de sa dernière trame décodée : il
reste valide pour la distinction, et c'est **le seul cas où le contrôle de D10
avait du pouvoir**. Le nôtre en a sur les deux.

- [ ] **Step 3 : ce que ce critère n'établit PAS, écrit d'avance**

Il établit que **deux pages ne décodent pas le même flux**. Il n'établit **pas**
que chaque page décode le flux de **la fenêtre Windows qu'elle prétend
montrer** — cela demanderait de corréler le marqueur au HWND, ce qui n'est pas
au programme. **Écrire cette limite dans le document de résultats**, pas la
laisser inférer.

- [ ] **Step 4 : deuxième exécution, verser, commit**

```bash
git add docs/superpowers/plans/journaux-multifenetres-d11/
git commit -m "recette(d11): deux fenetres montrent deux flux distincts, marqueur invariant a l'appui"
```

---

### Task 13 : recette ⑤ — le coût de la duplication surdimensionnée

**Files:**
- Create: journaux `agent-cout-{sale,propre}-{1,2}.log` + `-plat`, `cout-*.json`

**Interfaces:**
- Consumes: les tâches 8, 12 (VM sérialisée).
- Produces: le verdict du critère ⑤ — **ou « non pris »**.

**Un A/B à une seule variable** : même nombre de fenêtres, même taille retenue,
même taille d'encodage — **seule la taille de naissance des sorties change**.

| Bras | Registre | Comment il s'obtient | Comment il se VÉRIFIE |
| --- | --- | --- | --- |
| SALE | 3840×2160 | l'état courant de la VM | `duplication de sortie établie desktop_width=3840` au journal |
| PROPRE | 1280×720 | `MULTIFENETRE_MODE_SORTIE=1280x720`, **dans un lancement à elle seule** | `desktop_width=1280` |

- [ ] **Step 1 : la RÈGLE D'ADMISSION du bras PROPRE, écrite AVANT la mesure**

Le bras PROPRE **n'est réputé obtenu que si le journal porte `desktop_width=1280`
pour TOUTES les sorties du rang**.

```bash
grep -a 'duplication de sortie établie' agent-cout-propre-1-plat.log \
  | grep -o 'desktop_width=[0-9]*' | sort | uniq -c
```
Attendu : **une seule** valeur, `1280`. **À défaut, la mesure est DÉCLARÉE NON
PRISE** — et non pas approchée, ni interprétée. Mieux vaut « non pris, et voici
pourquoi » qu'un chiffre bâti sur un bras non établi.

⚠️ **Le bras PROPRE peut ne pas être obtenable, et c'est déclaré d'avance** : la
portée de la pollution de registre — **par GUID ou globale** — n'est pas
tranchée, et D10 l'a rendue *sans objet*, pas *résolue* ;
`journaux-multifenetres-d8/agent-recette.log` porte **cinq GUID SudoVDA
distincts**. Si la sonde ne déplace que le sien, les sorties suivantes peuvent
naître grandes malgré elle.

⚠️ **`MULTIFENETRE_MODE_SORTIE` doit être dans un lancement À ELLE SEULE** :
l'aiguillage de `diagnostics/multifenetre.rs` **retourne après la première sonde
reconnue** (`grep -c 'return Ok(true)' src/diagnostics/multifenetre.rs` rend
**18** — relevé du 19 août 2026), et un `MULTIFENETRE_VDD_PURGE=1` dans le même lancement
serait ignoré **en silence**.

- [ ] **Step 2 : le montage, identique aux deux bras**

Huit fenêtres, `BUDGET_BPS` à sa valeur livrée (**12 000 000**), mire animée à
cadence connue, **deux exécutions par bras**.

- [ ] **Step 3 : le palier, dimensionné contre les constantes RELEVÉES**

```bash
cd agent && grep -n "const DELAI_REMONTEE" src/congestion/hysteresis.rs
grep -n "pub const HYSTERESIS" src/capteur/vivier.rs
```
Relevé : `DELAI_REMONTEE = 20 s`, `HYSTERESIS = 2 s`.

**Palier de 90 s, dont les 30 premières SECONDES SONT ÉCARTÉES** comme temps
d'établissement — soit **60 s de mesure utile pour un `DELAI_REMONTEE` de 20 s**,
un rapport de **3**. *(D6 a perdu trois mesures pour un palier de 25 s face à
20 s : **25 % de marge**, où devaient encore tenir l'annonce de focus, la
redistribution des parts et la montée de l'estimation. Trois promotions sont
arrivées **après** la fin du palier, et l'échec s'imputait au produit alors qu'il
venait du protocole.)*

**Condition d'établissement, attendue en FAIT et non en durée** : **douze
secondes consécutives sans aucun changement de barreau** avant d'ouvrir la
fenêtre de mesure. Une exécution où l'échelle ne se pose pas est **disqualifiée**
— mélanger deux régimes est ce qui a rendu quatre exécutions de D6
incomparables.

- [ ] **Step 4 : la grandeur relevée, et son unité**

La **cadence par fenêtre** du capteur (`cadence du capteur … images=… cadence="…"`),
moyennée sur la fenêtre utile. Relever **aussi** :

- la taille d'encodage effective de chaque fenêtre (elle doit être **la même**
  aux deux bras — sinon la variable unique n'est pas unique) ;
- le `loadavg` de l'hôte au début et à la fin de chaque exécution : D6 a établi
  qu'il **covarie** avec les performances d'un facteur allant jusqu'à 3,6, et
  qu'un montage de recette « mesure son hôte au moins autant que le produit ».

- [ ] **Step 5 : ce que ce critère n'est PAS**

⚠️ **C'est une mesure, pas un contrôle** : il n'a pas de rouge. **Ce qui a un
rouge, c'est le contrôle du bras** (`desktop_width` vaut 3840 ou 1280, jamais les
deux), et il **peut** échouer. Ne pas présenter la mesure comme un contrôle.

- [ ] **Step 6 : verser, commit**

```bash
git add docs/superpowers/plans/journaux-multifenetres-d11/
git commit -m "recette(d11): le cout de la duplication surdimensionnee, deux bras, deux executions chacun"
```

---

### Task 14 : recette ⑥ — relire l'instrumentation du `Resize`

**Files:**
- Create: journaux `agent-resize-{1,2}.log` + `-plat`, `console-resize-{1,2}.jsonl`

**Interfaces:**
- Consumes: les tâches 8, 13 (VM sérialisée). **Dernière recette.**
- Produces: laquelle des trois issues de la grille se produit — **ou « non
  reproduit »**.

**Aucun code produit à écrire.** `client/src/main.ts` porte déjà la grille de
lecture à trois issues (posée par la tâche 18 de D10) et le `console.warn` du
report quand le canal n'est pas ouvert. **Ce qui manque est une exécution qui
lise ces logs sur la VM.**

- [ ] **Step 1 : LE CONTRÔLE D'ATTEIGNABILITÉ, avant toute conclusion**

⚠️ **C'est la ligne la plus importante de cette tâche, et elle passe avant la
mesure.** L'issue n°1 se lit sur une **absence de log**, et une absence de log
est aussi ce que produit un pilote qui ne collecte pas la console. **Les deux se
lisent pareil.**

Poser à la main un `console.debug('[sonde collecte] balise')` dans **chaque**
page, et vérifier qu'il **ressort** du pilote :

```bash
grep -c 'sonde collecte' console-resize-1.jsonl   # DOIT être ≥ 1 par page
```
**Si la balise ne ressort pas, la recette est ANNULÉE**, pas interprétée. Sans
ce contrôle, D11 rejouerait le naufrage de F1 (D7), où un contrôle écrit
précisément pour dénoncer un défaut muet **ne pouvait pas se déclencher**.

- [ ] **Step 2 : le montage, et l'issue déclarée d'avance**

Trois à cinq fenêtres, montage de la recette ① de D10. Relever, **par session** :

- les lignes `[instrumentation resize] declenchement ResizeObserver` avec
  `clientWidth`/`clientHeight`/`innerWidth`/`innerHeight` ;
- les `console.warn('Resize différé : canal de contrôle non ouvert')` ;
- côté agent, les lignes `contrôle reçu` **avec leur champ `session`** (posé par
  la tâche 5 de D9).

⚠️ **L'issue « on ne reproduit pas le silence » est POSSIBLE et déclarée
d'avance** : D9 a déjà relevé que le réseau local est trop rapide pour provoquer
la course, et que la **forcer par latence a cassé la reconnexion**. Elle serait
alors déclarée telle quelle, et le leg resterait ouvert.

- [ ] **Step 3 : lire la grille, dans SON ordre, et ne conclure que ce qu'elle donne**

1. **aucun** `declenchement ResizeObserver` pour une session qui n'émet jamais de
   `Resize` ⟹ le maillon est **en amont de la mise en page** ;
2. log présent et `clientWidth` **suit** `innerWidth` ⟹ ni l'observateur ni la
   mise en page ne sont en cause ;
3. log présent et `clientWidth` **ne suit pas** ⟹ la mise en page CSS de
   l'élément `<video>` est en cause.

⚠️ **Le canal de contrôle RESTE une hypothèse à part entière.** La correction C3
de D8 a établi qu'**aucune pièce ne le disculpe** — la pièce qui prétendait le
faire portait sur tout le run et non sur la phase, laquelle était **muette
141 s**. Le `console.warn` de la ligne 326 est précisément là pour l'attraper.
**Cette recette rend le maillon DÉCIDABLE ; elle ne l'identifie pas d'avance.**

- [ ] **Step 4 : deuxième exécution, verser, commit**

```bash
git add docs/superpowers/plans/journaux-multifenetres-d11/
git commit -m "recette(d11): le maillon du Resize, relu avec la console enfin collectee"
```

---

# Clôture

### Task 15 : document de résultats, revue transverse, et `CLAUDE.md`

**Files:**
- Create: `docs/superpowers/plans/2026-08-19-multifenetres-solder-les-legs-resultats.md`
- Modify: `CLAUDE.md`, et **tout fichier dont un commentaire est devenu faux**

**Interfaces:**
- Consumes: **toutes** les tâches. **Dernière, sans exception.**
- Produces: la mémoire permanente du sous-bloc.

- [ ] **Step 1 : le document de résultats permanent, AVANT `CLAUDE.md`**

Il porte : les six critères **avec leur nombre d'exécutions**, le verdict de
chacun, les pièces citées **par nom de fichier et numéro de ligne du journal**,
ce que D11 n'établit pas (§12 de la spec), les pièges neufs, et l'état des huit
legs de D10 plus les deux de D9.

⚠️ **Aucune preuve ne vit dans un rapport de tâche.** Le leg 3 de D10 est perdu
parce que l'analyse de D9 vivait dans `.superpowers/sdd/`, gitignoré. **Tout ce
qui doit survivre est dans ce document ou dans les journaux versés.**

- [ ] **Step 2 : la revue transverse — sa cible propre est NOMMÉE**

Cinq défauts en D7, trois Critiques en D8, six en D9, **douze en D10** — et
**tous franchissent une frontière de tâche** : chacun est correct des deux côtés
pris séparément. **Une revue par tâche ne peut structurellement pas les voir.**

**Cible propre de D11 : les commentaires qui décrivent le MONO-FENÊTRE.** Le
correctif du leg 4 change ce qu'ils décrivent.

```bash
grep -rn "mono-fenêtre\|mono-fenetre" agent/src client/src
```
⚠️ **Place par place, et RELU APRÈS ÉDITION, pas seulement avant.** Le leg 4 en a
déjà réfuté **six** en D10 : la revue transverse en avait corrigé **trois** en
affirmant qu'il n'y en avait que trois, la revue finale en a trouvé **deux** de
plus, et le balayage exigé par elle un **sixième**. *Une substitution qui ne dit
pas combien d'occurrences elle a touchées est une affirmation de complétude non
vérifiée.*

**Cible seconde, déjà identifiée par ce plan** : `transport.rs` disait
« **Sert UNIQUEMENT à détecter la TRANSITION** » du champ `audio_porteuse`,
faux depuis D10 lui-même (voir la table des divergences en tête). La tâche 3 l'a
corrigé — **vérifier qu'il n'en reste pas de jumeau** :

```bash
grep -rn "audio_porteuse" agent/src | grep -i "uniquement\|seule\|capteur nous demande"
```

- [ ] **Step 3 : balayer par le SENS, pas par la formule**

Leçon payée deux fois : un balayage sur « non diagnostiqué » a laissé survivre
« pas diagnostiqué ». Chercher la **chose niée** en énumérant les tournures —
« pas / non / jamais / seulement / sans explication / reste ouvert / n'est établi
par rien ». Et **annoter l'affirmation elle-même, pas sa voisine**.

- [ ] **Step 4 : relever les tailles PAR LA COMMANDE, APRÈS les dernières éditions**

```bash
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
```
⚠️ **Après** la dernière édition de la ronde, jamais avant : **une table relevée
en début de ronde est fausse à la fin de la même ronde** — erreur que D8 a
commise en croyant bien faire.

Relever aussi, nommément, les fichiers que D11 a fait bouger :

```bash
wc -l agent/src/transport/piste_audio.rs agent/src/transport/tick/tests/audio.rs \
      agent/src/transport.rs agent/src/windows_audio/fil.rs agent/src/audio.rs \
      agent/src/demarrage/audio.rs agent/src/windows_source/telemetrie.rs \
      client/src/main.ts client/src/resize.test.ts scripts/run-agent.sh
```
Et **la marge la plus serrée du dépôt**, qui n'est pas dans `agent/src` :
`client/verify-webrtc.mjs` (**497** au relevé de D10, marge 3) — **le remesurer**,
et ne pas recopier ce nombre.

- [ ] **Step 5 : « corrigé à sa place » est une affirmation de COMPLÉTUDE**

Pour chaque nombre corrigé dans `CLAUDE.md`, **énumérer ses places AVANT
d'écrire, et les RELIRE APRÈS** :

```bash
grep -n '<le nombre>' CLAUDE.md
```
Le naufrage du « 487 » s'est rejoué **neuf fois** dans ce dépôt, dont une dans la
vague même qui le corrigeait ailleurs, et une dans un commit dont le message
annonçait avoir énuméré les places avant d'écrire. **La leçon n'est pas
« mesurer », qui avait été fait — c'est que `grep -n` doit être relu place par
place APRÈS l'édition.** Et : **toucher une ligne d'un tableau de comptes oblige
à remesurer son compte**, même quand ce n'est pas l'objet de l'édition.

- [ ] **Step 6 : écrire la section « Sous-bloc D11 » de `CLAUDE.md`**

Sur le modèle des précédentes, et **elle doit porter au minimum** :

1. les **familles de lecture des journaux** de `journaux-multifenetres-d11/`
   (encodage, séquences ANSI, fins de ligne, octets NUL éventuels) — **relevées
   par `file` et `grep`, jamais supposées** ;
2. le verdict de **chaque** critère avec son **nombre d'exécutions** ;
3. **ce que D11 n'établit PAS** (§12 de la spec) ;
4. les **pièges neufs** ;
5. l'**état des legs**, y compris ceux qui restent dus — l'A/B sur
   `set_desired_bitrate` (écarté avec sa **condition de réouverture** : une
   charge d'hôte *contrôlée* et non seulement appariée, sur au moins huit
   paires) et les six constats perdus ;
6. les **deux variables de banc neuves** dans le tableau des variables
   d'environnement, avec leur convention (absente = désarmée) et leur trace ;
7. l'**annotation des legs de D10 devenus faux**, à leur place dans la section
   D10 **et** dans le récapitulatif de tête — **les deux places énumérées avant
   d'écrire**.

⚠️ **Ne pas déplacer le constat de mesure en tête de
`agent/src/capteur/plein_ecran.rs`** : **cinq commentaires du dépôt le citent**
comme point de référence.

- [ ] **Step 7 : les vérifications finales, les trois comptes**

```bash
cd agent && cargo test -p agent 2>&1 | tail -3
cd agent && cargo check --target x86_64-pc-windows-gnu 2>&1 | tail -3
cd client && npx vitest run 2>&1 | tail -4
```
**Annoncer les trois nombres attendus AVANT de lancer**, et reporter les trois
**relevés** dans le message de commit, comme D9 et D10. Attendu si toutes les
tâches sont livrées : **467 passed** côté agent (458 + 1 + 1 + 3 + 4, les tâches
de la famille ⑥ étant à somme nulle), **107 passed** côté client (108 si le test
optionnel de la tâche 6 Step 3 est livré), et `cargo check` à **sortie 0** avec
des avertissements **tous `dead_code`** — en vérifier la **nature**, jamais le
nombre, qui dérive avec la fraîcheur du build.

- [ ] **Step 8 : commit**

```bash
git add CLAUDE.md docs/superpowers/plans/ agent/src client/src
git commit -m "docs(d11): resultats du sous-bloc, revue transverse, et les chiffres releves par la commande"
```

---

## Ordre et dépendances

```
1   (document D10)          ─┐
2 → 3 → 4                    │  famille ①/② : MÊME FICHIER (piste_audio.rs),
                             │  strictement séquentielles entre elles
5   (fil.rs + audio.rs)     ─┤
6   (legs froids)           ─┤  parallélisables avec 1, 2, 5, 7
7   (mire)  →  8 (pilotes)  ─┘

                    └── 9 → 10 → 11 → 12 → 13 → 14   (recettes, VM SÉRIALISÉE)
                                                       └── 15  (clôture : dernière)
```

**Ce qui peut courir en parallèle** — quatre voies indépendantes :

| Voie | Tâches | Pourquoi elle est indépendante |
| --- | --- | --- |
| A | **1** | document seul, ne touche aucun code |
| B | **2 → 3 → 4** | les trois éditent `agent/src/transport/piste_audio.rs` : **jamais en parallèle entre elles** |
| C | **5** | `audio.rs` + `fil.rs` + `run-agent.sh` — aucun fichier commun avec B, **sauf `run-agent.sh` que la tâche 4 touche aussi** ⚠️ voir ci-dessous |
| D | **6** | `telemetrie.rs`, `resize.test.ts`, `main.ts` — aucun recouvrement |
| E | **7 → 8** | instrument seul, sous `docs/` |

⚠️ **Le seul recouvrement de fichier entre voies parallèles est
`scripts/run-agent.sh`**, que les tâches **4** et **5** modifient toutes deux.
**Les sérialiser sur ce fichier**, ou faire poser les **deux** lignes par la
tâche 4 et le vérifier à la tâche 5 (`grep -n 'AUDIO_FAUTE' scripts/run-agent.sh`
doit rendre **trois** lignes). **Ne jamais résoudre ce cas par `git add -A`.**

**Dépendances fortes** :

- **la famille ① (tâche 3) doit être livrée avant les recettes ① et ②**, qui en
  éprouvent le correctif ;
- **les tâches 4 et 5 doivent être livrées avant la recette ③**, qui n'existe que
  par leurs deux boutons ;
- **la tâche 7 avant la tâche 8**, et les deux avant toute recette ;
- **la tâche 15 est la dernière, sans exception** : une revue transverse jouée
  avant la fin de la branche ne peut pas voir les affirmations que la fin de la
  branche a rendues fausses.

⚠️ **La VM est un état partagé** : les tâches 9 à 14 sont **strictement
sérialisées**, quelle que soit leur indépendance logique. Aucune ne démarre sans
avoir revérifié `Get-Process agent` **et** purgé les sorties virtuelles.

---

## Extractions prévues — aucune a priori, deux portes armées

**Aucune extraction n'est ordonnancée**, contrairement à D10 qui en plaçait trois
en tête de plan : les quatre fichiers du chemin principal ont de la marge
(relevé du 19 août 2026 en tête de ce plan). **Mais deux portes sont armées**, et
si l'une se déclenche, **l'extraction devient une tâche à part, insérée AVANT
l'addition qui l'a déclenchée** :

| Porte | Se déclenche si | Extraction, et son point de chute |
| --- | --- | --- |
| `piste_audio.rs` | `wc -l` > **480** après la tâche 2, 3 ou 4 | `agent/src/transport/piste_audio/injection.rs` — `mod injection;` ordinaire, pas de `#[path]` (aucune frontière `#[cfg(windows)]` ici) |
| `tick/tests/audio.rs` | `wc -l` > **480** après la tâche 3 ou 4 | `agent/src/transport/tick/tests/audio/injection.rs`, sur le modèle de `tick/tests.rs` qui déclare déjà `mod audio; mod reste;` |
| `transport.rs` (marge **32**) | la réécriture de la doc **augmente** le compte | les champs audio **et leur documentation** vers `transport/piste_audio.rs`, où vivent déjà leurs seuls écrivains |

⚠️ **Jamais une compression.** D9 a joué la compression deux fois
(`capteur/sommeil.rs` ramené à 499, `capteur/fenetre.rs` à 496) et **la revue l'a
fait défaire les deux fois**. D10 a franchi le plafond trois fois et l'a rattrapé
**trois fois par extraction, jamais par une compression** — c'est le premier
sous-bloc à y être parvenu, et la barre ne redescend pas.

---

## Ce que ce plan ne prescrit PAS, et pourquoi

- **Rejouer l'A/B sur `set_desired_bitrate`** — écarté par la spec §2.2 avec sa
  condition de réouverture : le montage ne fait pas ce qu'on lui demande
  (appariement « dos à dos » à ~5 % près seulement), et la prémisse qui en
  faisait « le point ouvert le plus important » a été réfutée par D6 lui-même.
  ⚠️ **Cela n'affirme PAS que l'appel est sans effet.**
- **Reconstituer les six constats parqués de D9** — irrécupérables. **Inventer
  une liste serait pire que d'admettre qu'elle est perdue.**
- **Nettoyer le registre** : la voie de D10 rend le produit indifférent à son
  état, et y revenir réintroduirait l'écriture que D9 en a retirée. ⚠️ **La
  pollution subsiste, et rien ne nettoie derrière.**
- **Expliquer** la naissance d'une sortie à la taille du registre, ni la
  non-persistance du changement de mode, ni **identifier** la couche du plafond
  de 8 encodeurs ou de 4 processus.
- **Passer la reconstruction audio sur un fil** : `piste_audio.rs` nomme déjà la
  condition (« si la mesure montre qu'elle retarde le drainage »), et cette
  mesure n'est pas au programme.
- **La latence de bout en bout**, qu'aucun sous-bloc du chantier D n'a jamais
  mesurée — et D11 ne la mesurera pas davantage.
