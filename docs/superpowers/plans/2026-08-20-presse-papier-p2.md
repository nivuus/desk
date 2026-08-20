# Sous-projet ① « Divers » — presse-papier, sous-bloc **P2** : le navigateur colle dans la VM — plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to execute this plan.

> ⚠️ **ATTENTION AU NOM — il existe DEUX « P2 » dans ce dépôt, et ils n'ont
> rien à voir.**
>
> | Ce document | L'autre |
> | --- | --- |
> | **Presse-papier P2** — le navigateur colle dans la VM | **Plateforme P2** — un sous-bloc du sous-projet ⑤, **clos** |
> | Suite de `2026-08-19-presse-papier-p1.md`, **clos** | Suite de la plateforme, sans rapport |
>
> Le sous-bloc D-quelque-chose du chantier multi-fenêtres n'est pas non plus
> concerné. **Quand ce document écrit « P1 », il désigne toujours le
> presse-papier P1.**

**Goal:** livrer le sens **navigateur → VM** du presse-papier : l'utilisateur
copie sur sa machine locale, colle par `Ctrl+V` dans la fenêtre de session, et
le texte arrive dans l'application Windows.

**Architecture:**

1. **Le client n'intercepte plus tout le clavier.** `client/src/input.ts`
   appelle `event.preventDefault()` **sans condition** sur chaque `keydown`
   (`client/src/input.ts:95`), sur `window` (`:111`) : aucun événement `paste`
   ne peut naître dans la fenêtre de session. P2 y pose une **exception
   étroite**, et elle seule.
2. **Le collage voyage sur le canal de CONTRÔLE**, pas sur celui des entrées —
   parce que ce dernier est `ordered: false, maxRetransmits: 0`
   (`client/src/webrtc.ts:277-280`) quand le contrôle est `ordered: true`
   (`:281`), et qu'un collage exige un ORDRE : le presse-papier Windows
   d'abord, `Ctrl+V` ensuite.
3. **Le propriétaire écrit, l'enfant injecte.** Le capteur — seul propriétaire
   du presse-papier Windows depuis P1 (D1) — écrit le texte ; l'enfant attend
   son `Fait`, puis injecte `Ctrl+V` par son `InputInjector`, qui pose déjà
   `SetForegroundWindow` avant toute touche (`agent/src/input.rs:118-119`).
4. **Les trois gardes anti-écho naissent ici**, parce que c'est P2 qui rend la
   boucle possible — et **un sous-bloc ne livre pas un défaut qu'il crée**.

**Tech Stack:** Rust (agent, proto), TypeScript (client, proto), Win32
(`SetClipboardData`, `EmptyClipboard`, `GlobalAlloc`), CDP pour la recette.

**Spec :** `docs/superpowers/specs/2026-08-19-presse-papier-design.md`
(commit `a6b1b64`, 1108 lignes). P2 en dépend des §3.2 (R2), §4 (D1, D3, D4,
D5, D6, D7, D8), §6.2, §7.1, §10 (R5, R6, R7, R9), §11.

---

## 0. 🔵 LE PRÉALABLE ÉLIMINATOIRE EST MESURÉ, ET IL EST FAVORABLE

**C'est le premier fait de ce plan, et il commande tout le reste.** La spec
range en tête de P2 un contrôle qu'elle déclare capable de bloquer le sous-bloc
entier : *l'événement `paste` de confiance parvient-il quand le focus est sur
le `<video>` ?* Le relevé R2 (§3.2) avait été pris avec le focus sur un `body`,
et le §3.3 le dit lui-même. **La mesure a été prise le 20 août 2026, avant
l'écriture de ce plan.**

| Relevé | Valeur |
| --- | --- |
| Verdict | **FAVORABLE**, aux **deux** exécutions |
| Montage | `Chrome/151.0.7922.169`, `--headless=new`, sur l'**HÔTE** — la VM est tenue par un chantier concurrent |
| Cellule qui décide | focus sur `<video id="remote" tabindex="0">`, régime « exception étroite », `Ctrl+V` de confiance ⟹ **1 `paste`, `isTrusted: true`, `types: ["text/plain"]`, texte = le dernier copié, `e.target` = `VIDEO#remote`** |
| Permission | `clipboard-read` = `denied` (exéc. 1) / `prompt` (exéc. 2), `readText()` levant `NotAllowedError` **aux deux** |

**Pièces :** `docs/superpowers/plans/journaux-presse-papier-p2/` —
`p2-paste-video-{1,2}.{log,json}` et l'instrument
`instrument/sonde-p2-paste-video.mjs`. UTF-8, **sans séquence ANSI et sans
octet NUL** (relevé par `file`) : **une seule famille de lecture**, ils se
`grep`ent à plat — contrairement aux trois familles de P1.

### 0.1 🔴 Ce qui rend ce verdict FONDÉ, et pourquoi c'est la forme qui compte

La sonde P0 de P1 a rendu un **faux verdict éliminatoire** — trois zéros sur
une machine saine, parce que rien n'avait encore eu lieu
(`journaux-presse-papier-p1/p0-sonde-0-instrument-defectueux.log`). **Un
verdict négatif exige que la chose mesurée soit ABSENTE, pas seulement nulle.**

La sonde de P2 mesure donc une **matrice de 24 cellules dans une seule
session** — {`body`, `<video>`, conteneur `<div tabindex="0">`, `documentElement`}
× {`produit`, `etroit`, `aucun`} × {`Ctrl+V`, `Shift+Insert`} — et porte les
deux témoins qui manquaient à P0 :

- **la ROUGE DE L'INSTRUMENT** — le régime `produit`, qui recopie le
  `preventDefault()` inconditionnel de `client/src/input.ts:95`, rend **zéro
  `paste` sur ses huit cellules, aux deux exécutions**. L'instrument PEUT
  rendre zéro, et c'est bien `input.ts` qui bloque aujourd'hui ;
- **le TÉMOIN DE MESURABILITÉ** — `body` rend un `paste` juste dans la **même**
  session. Un `<video>` muet en aurait été discriminé ;
- et si **aucune** cellule n'avait rendu de `paste`, la sonde aurait conclu
  `NON MESURABLE` au lieu de `DEFAVORABLE` — la distinction est écrite dans son
  code, pas décidée après coup.

### 0.2 🔵 Trois faits neufs que P2 CONSOMME, et qu'aucun document ne portait

1. **L'exception peut être VRAIMENT étroite.** Dans la cellule qui décide, le
   `keydown` de **`ControlLeft` garde son `preventDefault`** (`dp: true`) et
   **seul `KeyV` passe** (`dp: false`). Il suffit donc de laisser passer le
   `keydown` du raccourci lui-même. **Le risque R5 de la spec — rendre le
   navigateur au clavier — se borne à une condition d'une ligne, mesurée et non
   supposée.**
2. **`Shift+Insert` produit le même `paste` de confiance**, aux quatre cellules
   où il est éprouvé, aux deux exécutions. D6 le nommait sans l'avoir mesuré.
3. **Le chemin ne dépend d'aucune permission, sur `<video>` comme sur `body`** :
   voir le tableau ci-dessus. Le critère ② de P2 en est **pré-étayé**, pas
   dispensé.

### 0.3 ⚠️ Une sonde ANNEXE lève une ambiguïté que la sonde principale créait

La sonde principale relève `writeText THROW:NotAllowedError` là où le §3.1 de
la spec relève `writeText : OK`. Publier ce THROW sans l'expliquer laisserait
un fait apparemment contradictoire avec R1, donc avec tout le sens VM →
navigateur que P1 a recetté.

`instrument/sonde-p2-annexe-writetext.mjs`, **deux exécutions**, relève :
`isActive:false` ⟹ **THROW**, puis après un `Ctrl+C` de confiance
`isActive:true` ⟹ **OK**. L'hypothèse que la spec pose elle-même (§3, portée :
R1 mesuré « dans un contexte où `userActivation.isActive` valait DÉJÀ `true` »)
est **CORROBORÉE**, et **NON ÉTABLIE** — le geste n'est pas la seule variable
qui a changé entre les deux appels. **Elle n'établit rien du préalable de P2**
et rien du sens VM → navigateur.

### 0.4 Ce que la mesure N'ÉTABLIT PAS

- **Rien d'un Chromium AVEC interface**, rien de Firefox, rien de Safari — même
  portée que le §3 de la spec, qui la déclare déjà.
- **Rien de `Ctrl+W` / `Ctrl+T` / `Ctrl+N`** (critère ⑤) : `--headless=new` n'a
  ni onglets ni fenêtres au sens de l'utilisateur, et `Input.dispatchKeyEvent`
  n'emprunte pas le chemin des raccourcis du **navigateur**. **Ce critère se
  joue à la recette, sur un navigateur réel** — voir la tâche 22.
- **Rien de la VM Windows**, et **aucun taux** : deux exécutions.

---

## 1. Ce plan ne couvre QUE P2

- **P1 est CLOS** (sens VM → navigateur). P2 n'y touche que là où il l'étend :
  le garde n°1 dans `Sondeur`, et l'écriture Win32 dans `presse_papier/win32.rs`.
- **P3 — les N fenêtres** (spec §6.3) : la règle « toutes reçoivent, seule la
  focalisée écrit localement » et sa mesure à trois fenêtres. **P2 recette à UNE
  fenêtre**, comme P1. ⚠️ Le legs n°3 de P1 — « une fenêtre attachée après une
  copie ne reçoit jamais ce contenu » — **reste à P3**.
- **A1 — la couleur d'accent** (spec §5, §6.4) : hors périmètre entier.
- **Images, fichiers, RTF, HTML** : hors périmètre v1 (spec §9).
- **Le propriétaire MONO-FENÊTRE** (legs n°1 de P1) reste **hors périmètre**, et
  ce plan écrit pourquoi (D-P2-9).

---

## 2. Contraintes globales

### 2.1 Ce qui a été relevé PAR LA COMMANDE avant d'écrire une ligne

| Commande | Résultat |
| --- | --- |
| `wc -l` sur les fichiers touchés | voir le tableau du §2.2 |
| `cargo test -p agent` | **671 passed, 0 failed** |
| `npx vitest run` (client) | **304 passed**, 33 fichiers |
| `grep -n createDataChannel client/src/webrtc.ts` | `:277` input `ordered:false, maxRetransmits:0` ; `:281` control `ordered:true` |
| `grep -n 'PRESSE_PAPIER' scripts/run-agent.sh` | `:37` — **déjà transmise par P1** |
| `grep -n 'ClientControl::' agent/src/transport/evenements.rs` | `:336`, `:339` — le `match` de `memoriser_controle` (`:334-343`) |

⚠️ **Ces chiffres dérivent, et un chantier concurrent travaille dans `agent/`
et `scripts/run-agent.sh`.** Les relever de nouveau avant chaque tâche qui en
dépend ; ne jamais recopier ceux-ci.

### 2.2 La règle des 500 lignes, budgétée d'avance

Relevé **par la commande**, le 20 août 2026 :

```bash
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>400'
```

| Fichier touché par P2 | Lignes | Marge | Budget |
| --- | --- | --- | --- |
| 🔴 `agent/src/demarrage.rs` | **491** | **9** | `Capabilities.clipboard` ⟹ **EXTRACTION PRÉALABLE OBLIGATOIRE** (tâche 3). C'est le legs n°1 de P1, qui l'exigeait déjà |
| 🔴 `agent/src/capteur/protocole.rs` | **464** | **36** | +1 variante `VersCapteur` **avec sa doc** ⟹ **EXTRACTION PRÉALABLE** de son bloc de tests (`:298-464`, 167 lignes ⟹ **297**), tâche 2 |
| 🔴 `agent/src/presse_papier.rs` | **428** | **72** | +garde n°1, +décision d'écriture, +tests ⟹ **EXTRACTION PRÉALABLE** de son bloc de tests (`:242-428`, 187 lignes ⟹ **241**), tâche 1 |
| `agent/src/capteur/distante.rs` | 446 | 54 | +1 méthode |
| 🔴 `agent/src/transport/tick.rs` | 421 | 79 | ❌ **BUDGET FAUX.** « +1 branche » suggère quelques lignes ; le corps de `a1octies` en fait **plus de soixante-dix**, car il porte tout le raisonnement de D6. Le fichier est monté à **493, MARGE 7**. Mesuré AVANT commit, extraction jouée : `transport/collage.rs` (neuf) le ramène à **441**. **Le plafond n'a jamais été franchi dans un commit** |
| `agent/src/source.rs` | 414 | 86 | +1 méthode de trait, à défaut inerte |
| `client/src/main.ts` | 414 | 86 | +câblage seul — **toute la logique va ailleurs** |
| `agent/src/transport/evenements.rs` | 363 | 137 | +1 bras dans un `match` exhaustif |
| `proto/src/control.rs` | 327 | 173 | +1 variante `ClientControl`, +1 champ `Capabilities` |
| `agent/src/input.rs` | 250 | 250 | l'injection |
| `agent/src/capteur/fenetre/commandes.rs` | 234 | 266 | +1 bras étage 0, +1 dans le bras miroir |
| `agent/src/capteur/sommeil/presse_papier.rs` | 193 | 307 | l'écriture par le propriétaire |
| `proto/ts/control.ts` | 184 | 316 | +1 interface, +1 encodeur, +1 témoin |
| `scripts/run-agent.sh` | 145 | — | +1 ligne (tâche 21) |
| `client/src/input.ts` | 123 | 377 | l'exception étroite |
| `client/src/presse-papier.ts` | 112 | 388 | le garde n°3 |
| `client/src/presse-papier-dom.ts` | 101 | 399 | l'écouteur `paste` |
| `agent/src/presse_papier/win32.rs` | 98 | 402 | `ecrire_texte` |
| `agent/src/transport/boucle.rs` | 93 | 407 | +le drainage de l'injection |
| 🔴 `agent/src/transport.rs` | **448** | **52** | ❌ **ABSENT DE CETTE TABLE, et c'est là que vivent les DEUX champs** (`pending_clipboard`, `collage_a_injecter`) — la tâche 14 le savait (« le champ dans la structure de session, il n'est pas dans ce fichier ») sans le budgéter. Réel : **479, marge 21**, soit +31/−0 dont **quatre lignes de code** et vingt-sept de commentaire. ⚠️ Ce fichier a franchi 501 **deux fois** (D10, puis F2 qui l'a ramené à 448 par extraction) : P2 reprend 31 des 52 regagnés. **Toute addition future y appelle une extraction**, point de chute `transport/initialisation.rs` |
| `agent/src/transport/collage.rs` | — | — | **NEUF**, né de l'extraction ci-dessus : les DEUX moitiés de l'ordre de D6 au même endroit |

⚠️ **Aucun fichier de la dette gelée n'est touché** : ni `encode.rs` (1536), ni
`windows_source.rs` (630), ni `encode/arret.rs` (500, marge 0), ni
`capture.rs` (492). Ni les deux entrées neuves du tableau de dette,
`proto/src/plateforme/tests.rs` (561) et `proto/ts/plateforme.test.ts` (512).

🔴 **Les trois extractions se jouent AVANT les additions qui les rendent
nécessaires, jamais après.** D9 a payé deux compressions pour l'avoir oublié ;
D10 a joué trois extractions préalables et n'en a payé aucune ; P1 en a joué
une (sa tâche 3) et n'en a payé aucune non plus.

⚠️ **Scinder un module de tests par `#[path]` est HORS de la portée de la
convention `#[path]`/racine nue** — `CLAUDE.md` l'écrit nommément (§ « Convention
de module enfant », clause « Est également hors de portée l'usage de `#[path]`
pour scinder un module de *tests* trop long »). Les tâches 1 et 2 emploient donc
`#[cfg(test)] #[path = "…/tests.rs"] mod tests;`, comme `superviseur/table.rs`.

### 2.3 Les règles de méthode, héritées et non négociables

1. **Un contrôle qu'on n'a jamais vu ROUGE n'est pas un contrôle.** Pour chaque
   test prescrit, ce plan écrit ce qui le rend rouge — et **la tâche vérifie que
   cet état est atteignable** avant d'écrire le vert.
2. **Un plan est une SOURCE de contrôles vacueux, pas une protection contre
   eux.** Ce plan en a trouvé un dans la spec elle-même (E4) et l'a corrigé ;
   il peut en porter d'autres.
3. **Ne jamais fabriquer une sortie de commande.** Lancer la commande.
4. **Nommer la chose, jamais compter les lignes qui l'en séparent** : un
   déictique (« trois lignes plus haut ») vieillit à la première insertion. P1 a
   payé ce défaut deux fois.
5. **`git commit` valide TOUT L'INDEX, y compris avec `-F`.** Pathspec
   explicite, et vérification après coup. **Jamais `--amend`.**
6. **Compter les tests AVANT de les lire** : annoncer le nombre attendu, puis le
   comparer.
7. **Un journal de pilote se lit avec `grep -a`** (P1 : `grep -c` rend une
   sortie vide, `grep -ac` rend 17).
8. **Copier `agent.log` APRÈS la fin réelle de l'exécution**, pas à la fin du
   pilote.
9. **Appel BLOQUANT au premier plan** pour toute exécution longue : une commande
   backgroundée par le harnais ne survit pas à la fin du tour (D10, deux
   recettes y ont perdu une exécution).
10. **`Get-Process agent` revérifié après CHAQUE tentative, y compris échouée** :
    un superviseur survivant fait relire le journal de la tentative précédente.

### 2.4 Périmètre concurrent

⚠️ **Un chantier concurrent travaille dans `agent/` et `scripts/run-agent.sh`**,
et un autre tient la **VM Windows** (porte éliminatoire du chantier microphone).
`CLAUDE.md` est également sous périmètre concurrent.

- **Step 0 de toute tâche touchant `agent/`, `scripts/` ou `CLAUDE.md`** :
  `git log --oneline -5 -- <chemin>` et relire le diff.
- **La tâche 22 (recette) ne peut pas démarrer tant que la VM est tenue.**
  L'ordonnanceur doit l'enregistrer comme un préalable **externe**, pas comme
  une dépendance de tâche.
- **`git add` nominatif**, jamais `git add -A` : un `git add -A agent/src` a
  déjà emporté le travail concurrent d'une autre tâche dans un commit qui ne
  compilait pas.

---

## 3. Décisions tranchées

### D-P2-1 — L'ordre de D6 s'obtient SANS changer aucune signature

D6 exige un ordre : (a) le presse-papier Windows porte le bon texte, puis (b)
l'application reçoit `Ctrl+V`. Le problème est que les deux moitiés ne vivent
pas au même endroit :

| Chose | Où | Pièce |
| --- | --- | --- |
| la source distante, seule à savoir parler au capteur | dans `Session` | `agent/src/capteur/distante.rs:169` (`commander_simple`) |
| l'injecteur clavier | dans la fermeture `spawn_blocking` de l'enfant | `agent/src/demarrage.rs:311` puis `:315` |
| le callback qui reçoit les `ClientControl` | **ne trace que**, et n'a accès ni à l'un ni à l'autre | `agent/src/demarrage.rs:415` |

Et **`act_on_timeout` ne reçoit ni `on_input` ni `on_control`** —
`pub(super) fn act_on_timeout(&mut self, deadline: Instant) -> Result<Tick>`
(`agent/src/transport/tick.rs:108`) —, alors que **`run` reçoit les deux**
(`agent/src/transport/boucle.rs:45-49`) et appelle `act_on_timeout` en `:53`.

**Décision — trois pas, aucune signature modifiée :**

1. `memoriser_controle` (`agent/src/transport/evenements.rs:334-343`) pose
   `pending_clipboard`, comme il pose déjà `pending_resize` et
   `pending_visibility` ;
2. `act_on_timeout` gagne la branche **`a1octies`** : elle prend
   `pending_clipboard.take()`, appelle `self.source.ecrire_le_presse_papier(&texte)`
   — **synchrone, avec attente du `Fait`** —, et ne pose `self.collage_a_injecter`
   **que si l'écriture a réussi** ;
3. `run` (`boucle.rs`), **juste après** son appel à `act_on_timeout`, consomme
   `collage_a_injecter` et appelle `on_input(…)` avec les touches.

**L'ordre est garanti par construction** : l'écriture est synchrone et précède
la pose du drapeau, qui précède l'injection. Aucun ordonnancement de canal n'y
entre.

**Alternative écartée** : changer la signature d'`act_on_timeout` pour lui
passer `on_input`. Elle est appelée depuis `run` seul, mais elle porte huit
branches et un invariant de drainage documenté sur soixante lignes ; lui ajouter
un paramètre pour une branche est un coût permanent contre un gain nul.

⚠️ **Coût nommé, et il est réel** : l'écriture bloque la boucle de transport le
temps d'un aller-retour de tube. **Le précédent existe et il est exercé** —
`set_awake` commande le capteur depuis cette même boucle
(`agent/src/capteur/distante.rs:376`), et D4 a mesuré une attache en 34 µs. Mais
la borne du canal est **12 s**, et un capteur mort ferait attendre la boucle
jusque-là. **Ce chemin n'a jamais couru** (la borne de 12 s est déclarée « du
code jamais couru » depuis D4) : legs, pas remède.

### D-P2-2 — L'injection est AUTO-SUFFISANTE en modificateurs, et elle ne crée aucun code

**Décision** : l'agent injecte **quatre** `InputMessage::Key` — `Ctrl`↓,
`V`↓, `V`↑, `Ctrl`↑ — par le chemin existant `on_input`.

**Pourquoi auto-suffisante** : le canal d'entrées est `ordered: false,
maxRetransmits: 0` (`client/src/webrtc.ts:277-280`). L'état des modificateurs
côté VM au moment de l'injection **n'est pas connaissable** — le `Ctrl`↓ que le
client a envoyé peut être arrivé, perdu, ou arriver après. Poser soi-même les
quatre événements rend le geste indépendant de tout cela ; un `Ctrl`↑ de trop
est inoffensif.

**Pourquoi aucun code neuf** : le bras `InputMessage::Key` de `InputInjector`
appelle **déjà** `au_premier_plan()` (`agent/src/input.rs:118-119`, la fonction
étant en `:155`), lequel vérifie le retour de `SetForegroundWindow` et le
journalise **une fois par basculement**. Réutiliser `InputMessage::Key` hérite
de tout cela gratuitement.

⚠️ **La portée du dépôt sur `SetForegroundWindow` n'est PAS élargie par cette
décision, et il faut le dire** : ce qui est mesuré (D2) est « une frappe par
fenêtre, sonde séquentielle, **aucune frappe concurrente** », et
`agent/src/input.rs` écrit lui-même que `SendInput` est **global à la session
Windows** et que « deux sessions qui tapent en même temps se le disputent ».
**Deux collages simultanés depuis deux fenêtres restent hors de ce qui est
établi**, et P3 est le sous-bloc qui les rencontrera.

### D-P2-3 — Un collage est un ÉVÉNEMENT, et il est pourtant mémorisé comme un état

`memoriser_controle` porte une doctrine explicite : « Mémorise un
`ClientControl` reçu, **sans jamais l'appliquer sur-le-champ** », parce que le
code s'exécute pendant le drainage de `poll_output` et que str0m impose une
seule mutation de `Rtc` par appel.

**Décision : `pending_clipboard: Option<String>`, écrasement du dernier.**

⚠️ **Ce n'est pas gratuit, et le coût est écrit** : deux collages arrivés entre
deux tours de boucle se réduisent au second, le premier étant **perdu sans
trace**. C'est acceptable parce qu'un tour de boucle est borné par la cadence
vidéo et qu'un humain ne produit pas deux `Ctrl+V` dans cet intervalle — **mais
un client qui se conduirait mal, lui, le pourrait.** Une file bornée serait le
remède ; elle n'est pas prescrite ici. **Legs.**

### D-P2-4 — Le garde n°1 est exactement une ligne, et il vit dans le `Sondeur` PUR

`pub fn observer` (`agent/src/presse_papier.rs:158`) commence son corps par
`if self.reference == Some(seq) { return None; }` (`:163`). **Le garde n°1 de D5
consiste donc à poser `reference` sur le numéro de séquence relu juste après
notre propre écriture.**

**Décision** : une méthode **pure** `Sondeur::apres_notre_ecriture(&mut self,
seq: u32, texte: &str)`, qui pose `reference = Some(seq)` **et**
`dernier_emis = Some(texte.to_owned())`.

- `reference` **est** le garde n°1 : le tour suivant ne rouvre même pas le
  presse-papier ;
- `dernier_emis` **est** le garde n°2 armé sur notre écriture : il rattrape le
  cas où une écriture tierce s'intercalerait entre notre `SetClipboardData` et
  notre relecture du compteur — le cas exact que D5 donne comme raison d'être du
  n°2.

**Elle est pure et testable sur l'hôte** : `observer` prend son `seq` et sa
fermeture de lecture en paramètres, c'est tout le mérite du découpage de P1.

### D-P2-5 — 🔴 LA ROUGE DU CRITÈRE ④ DE LA SPEC EST VACUEUSE, ET CE PLAN LA REMPLACE

La spec écrit, critère ④ de P2 : *« rouge = désarmer le garde n°1 et compter
les messages sur 30 s. **Le compte doit croître sans borne** ; s'il reste à un,
le garde n°1 ne servait à rien et il faut le dire. »*

**Il reste à un. La spec avait prévu ce cas, et c'est bien celui qui se
produit.** La démonstration tient au code, pas à une mesure :

1. l'agent écrit `T` dans le presse-papier Windows ; le compteur bouge (mesuré
   par la sonde P0 de P1, `q3="bouge"`, deux exécutions) ;
2. le `Sondeur` lit `T`. Sans garde n°1 **ni** armement du n°2, `dernier_emis`
   ne vaut pas `T` : **un** message part vers les fenêtres ;
3. il pose alors `dernier_emis = Some(T)`, et `reference = Some(seq)`. **Au tour
   suivant, `observer` sort sur sa première ligne.** Le compte s'arrête à un ;
4. et **rien ne le relance** : le client n'émet vers l'agent que sur un
   événement `paste`, c'est-à-dire sur un **geste de l'utilisateur** — jamais à
   la réception d'un `clipboard`.

> 🔵 **Conséquence de conception, et elle contredit une phrase de la spec** :
> D5 écrit que la boucle « est réelle et ne s'arrête pas d'elle-même ». **Dans
> l'architecture livrée, aucune oscillation auto-entretenue n'est possible** —
> chaque tour exigerait un geste humain. Ce que les gardes suppriment est **un
> aller-retour par collage**, pas une divergence. ⚠️ **Je le déduis du code, pas
> d'une mesure** : `client/src/presse-papier-dom.ts` n'écrit que localement à la
> réception (`writeText`), et n'émet rien. **La condition qui rendrait la boucle
> réelle est nommée** : un client qui réémettrait vers l'agent ce qu'il reçoit.

**Décision — le critère ④ est reformulé, et sa rouge devient atteignable :**

| | Énoncé |
| --- | --- |
| **Critère** | après **k** collages, **aucun** message `clipboard` ne revient vers la fenêtre |
| **Rouge** | `PRESSE_PAPIER_GARDE=0` ⟹ **exactement k** messages, chacun portant le texte qu'on vient de coller |
| **Ce qui la rend atteignable** | la variable désarme `apres_notre_ecriture` **en entier** — n°1 *et* armement du n°2. Désarmer le seul n°1 rendrait **0 message aussi**, et la rouge serait vacueuse une seconde fois |

⚠️ **C'est le point le plus important de ce plan après le §0.** Une rouge qui ne
peut pas se déclencher est le mode de défaillance que ce dépôt paie depuis F1
(D7), et il vient d'être trouvé **dans la spec**, pas dans le code.

### D-P2-6 — Le prédicat de collage est PUR, il vit à part, et il est la seule défense contre R5

**Décision** : un module neuf `client/src/raccourcis.ts`, **pur, sans DOM**,
exportant `estUnRaccourciDeCollage(e: { ctrlKey, shiftKey, altKey, metaKey, code })`.

**Condition, mesurée et non supposée** (§0.2, fait n°1) :

```
(e.ctrlKey && !e.altKey && !e.metaKey && !e.shiftKey && e.code === 'KeyV')
||
(e.shiftKey && !e.altKey && !e.metaKey && !e.ctrlKey && e.code === 'Insert')
```

**Pourquoi les négations sont écrites** : sans elles, `Ctrl+Shift+V` (« coller
sans mise en forme » dans plusieurs applications) et `Ctrl+Alt+V` passeraient.
Élargir cette condition est **exactement** le risque R5 de la spec — `Ctrl+W`
fermerait la fenêtre de session —, et c'est pourquoi le prédicat est **un module
pur avec ses tests**, et non trois conditions inlinées dans un écouteur.

⚠️ **`ControlLeft` et `ShiftLeft` NE sont PAS des raccourcis de collage**, et
leur `preventDefault` est conservé : la sonde établit que cela n'empêche pas le
`paste` (`dp: true` sur `ControlLeft`, `dp: false` sur `KeyV`, `paste` reçu).

### D-P2-7 — Le client retient les scancodes du collage, mais LUI SEUL

D6 point 2 : les scancodes du raccourci « ne partent **pas** sur le canal
d'entrées ». **Décision** : `onKeyDown` **et** `onKeyUp` de `client/src/input.ts`
sortent sans `send` quand `estUnRaccourciDeCollage(event)` est vrai — mais
**sans `preventDefault` pour le `keydown`** (c'est ce qui laisse naître le
`paste`) et **avec** pour le `keyup` (qui n'a aucune raison de rendre la main au
navigateur).

⚠️ **`ControlLeft`/`ShiftLeft` partent normalement**, eux : les retenir ferait
perdre à la VM un modificateur que l'utilisateur tient peut-être pour autre
chose. L'injection étant auto-suffisante (D-P2-2), aucun doublon n'en résulte
qui ait un effet.

### D-P2-8 — `Capabilities` gagne `clipboard`, et l'enfant lit la variable

Le legs n°2 de P1 pose le problème : *« `Capabilities` est émis par l'ENFANT
(`agent/src/demarrage.rs:291`), qui ne lit pas `PRESSE_PAPIER` — la spec place
cette lecture dans le propriétaire, donc dans le capteur. »*

**Décision : l'enfant lit `PRESSE_PAPIER` lui aussi**, par le même
`presse_papier::actif()` que le capteur.

**Preuve que les deux lectures s'accordent** : `std::process::Command` hérite de
l'environnement du père — c'est ce que la revue transverse de P1 a établi (son
constat R1) —, et `lancer_capteur` (`agent/src/superviseur/lanceur.rs:249`)
n'efface que `SUPERVISEUR`, `PONT`, `TEST_FILE` et `WINDOW_TITLE`.
`PRESSE_PAPIER` est donc **la même valeur** dans le superviseur, le capteur et
chaque enfant.

⚠️ **Ce n'est pas « le capteur annonce sa capacité », c'est « les deux lisent la
même variable ».** La différence compte : si le capteur venait un jour à décider
autrement qu'à la lecture d'une variable d'environnement, l'annonce deviendrait
fausse **en silence**. **Le commentaire du champ doit porter cette condition.**

⚠️ **`capabilities` est émis TROIS fois** (`demarrage.rs:291`, `:363`, `:383`) :
une fois optimiste, deux fois en repli quand la manette échoue. **Le champ
`clipboard` doit valoir la même chose aux trois** — le repli concerne le gamepad
seul.

### D-P2-9 — Le mono-fenêtre reste hors périmètre, et P2 le rend VISIBLE au lieu de le taire

En mono-fenêtre (`main.rs` retourne vers `demarrage::executer`), il n'y a pas de
capteur : `SourceDistante` n'existe pas, la méthode de trait
`ecrire_le_presse_papier` retombe sur son **défaut**, et aucun texte n'est écrit.

**Décision** : le défaut du trait rend **`Err`**, jamais `Ok(())`.

**Pourquoi pas un défaut inerte comme `signaler_audio_mort`** : un `Ok(())`
ferait injecter `Ctrl+V` sur un presse-papier Windows **inchangé**, donc coller
le contenu **précédent** — le mode de défaillance silencieux que D6 existe
entièrement pour éviter. Un `Err` fait, lui, journaliser l'échec et **ne pas
injecter**, ce qui est le comportement que D6 prescrit en toutes lettres pour ce
cas (« si le presse-papier ne peut pas être écrit, la touche `V` est PERDUE, pas
reportée »).

⚠️ **Le mono-fenêtre n'a donc pas de collage, et il le DIT.** Le legs n°1 de P1
reste ouvert.

### D-P2-10 — Le texte entrant est borné et normalisé par le MÊME code que le sortant

**Décision** : le chemin entrant réutilise `presse_papier::normaliser` et
`PRESSE_PAPIER_MAX`, dans le sens inverse — `\n` ⇄ `\r\n` — et refuse au-delà de
la borne au lieu de tronquer.

⚠️ **`normaliser` ne fait aujourd'hui qu'un sens.** La tâche 4 doit donc ajouter
sa réciproque, et **son test doit voir rouge sur l'aller-retour** : un texte
Windows à `\r\n` qui traverse les deux sens doit revenir identique. C'est la
première chose qu'un test doit voir rouge (spec §7.1).

⚠️ **Où la borne s'applique, et pourquoi ce n'est pas symétrique** : côté
sortant elle protège le **canal de contrôle** (spec D4) ; côté entrant, le texte
a **déjà** traversé ce canal quand l'agent le voit. La borne entrante protège
donc le **tube capteur↔enfant** et la mémoire, pas le canal. **Le refus entrant
se journalise, il ne remonte aucun bandeau** — le client a déjà, lui, sa propre
borne à appliquer avant d'émettre.

---

## 4. Divergences entre la spec et le code réel

Toutes relevées par lecture directe de l'arbre courant, le 20 août 2026.

**E1 — 🔴 La rouge du critère ④ est vacueuse.** Traitée en D-P2-5. C'est la
divergence la plus lourde, et elle porte sur un contrôle, pas sur du code.

**E2 — La spec cite `client/src/webrtc.ts:265-268` et `:269` pour les deux
canaux ; les lignes ont DÉRIVÉ.** Les vraies sont `:277-280` (`input`,
`ordered:false, maxRetransmits:0`) et `:281` (`control`, `ordered:true`). **Le
fait cité est exact, seule sa localisation est fausse** — l'écart entre P1 et P2
suffit à décaler un fichier client. C'est la raison de la règle 4 du §2.3.

**E3 — La spec cite `agent/src/transport/evenements.rs:251-258` pour le `match`
exhaustif sur `ClientControl` ; il est en `:334-343`**, dans
`fn memoriser_controle` (`:334`). La propriété — **gardé par le compilateur** —
est vraie ; le numéro ne l'est plus.

**E4 — La spec dit que `TYPES_AGENT` est « un tableau écrit à la main, et rien
ne le confronte à l'union » (§2.2, R3). CE N'EST PLUS VRAI : P1 l'a corrigé.**
`proto/ts/control.ts:137` déclare `const TOUS_AGENT: Record<AgentControl['type'],
true>`, et `:153` en dérive `TYPES_AGENT`. **Le risque R3 est donc FERMÉ pour le
sens agent → client.** ⚠️ **Et il n'a pas de jumeau à ouvrir dans l'autre sens** :
`ClientControl` n'est jamais *parsé* côté TypeScript (le client encode), et il
est *désérialisé* côté Rust par `serde` avec un `match` exhaustif. **P2 n'a donc
aucun `TYPES_CLIENT` à écrire**, et prescrire un test d'exhaustivité côté client
serait un contrôle sans objet.

**E5 — La spec place `clipboard: bool` dans `Capabilities` en disant « calquée
sur `mic` » ; `mic` vit dans `Ready`** (`proto/src/control.rs:138`), pas dans
`Capabilities` (`:182-186`, dont le seul champ est `gamepad`, `:185`). Le
**raisonnement** que la spec reprend de `mic` (pas de bump, `#[serde(default)]`)
est exact et le commentaire de `mic` le porte déjà ; seul le voisinage diffère.
**Décision : suivre la spec** — `Capabilities`, parce que c'est là que vivent les
capacités, et que `Ready` porte la géométrie.

**E6 — La spec justifie `#[serde(default)]` par `deny_unknown_fields`, et les
deux mécanismes n'ont rien à voir.** `deny_unknown_fields` refuse un champ
**inconnu** ; c'est le défaut de `serde` qui refuse un champ **manquant**. Le
commentaire de `mic` (`proto/src/control.rs:126-129`) dit d'ailleurs la chose
juste : « `AgentControl` porte `deny_unknown_fields`, ce qui **n'empêche pas
d'AJOUTER** un champ, mais un champ MANQUANT reste une erreur ». **Le
`#[serde(default)]` reste requis** ; la raison à écrire est celle du commentaire
de `mic`, pas celle de la spec.

**E7 — La spec parle du « bras miroir de `commandes.rs:221-227` » ; le miroir
est en `:220-224`** (`VersCapteur::Attache { .. } | Identite | Visibilite |
AudioMort | AudioVivant`). La propriété est intacte : **les commandes d'étage 0
sont listées DEUX fois** dans ce fichier, et oublier la seconde ne compile pas.

**E8 — La spec dit que le `paste` « n'est pas mesuré sur un `<video>` focalisé »
(§3.3) et le range en préalable éliminatoire (§10, R1). IL EST MESURÉ, et
favorable** — §0 de ce document. La spec sera annotée par la tâche 25.

**E9 — Le nom `ClipboardClientMessage` est DÉJÀ réservé par P1**, avec sa raison
écrite dans `proto/ts/control.ts` au-dessus de `ClipboardAgentMessage` : « le
sous-bloc P2 ajoutera un `ClipboardClientMessage` portant le MÊME tag
`'clipboard'` dans l'autre sens ». **Rien à renommer.**

**E10 — Le legs n°4 de P1 s'aggrave, et P2 doit le dire.** Le canal `Message` du
registre est **non borné**, et P1 y fait circuler jusqu'à 64 KiB **par fenêtre
et par changement**. P2 n'y ajoute rien dans ce sens — l'écriture va de l'enfant
vers le capteur par le **tube**, pas par ce canal —, mais **le collage fait
maintenant circuler 64 KiB dans les DEUX sens** à chaque geste. Nommé, non
corrigé.

---

## 5. Structure des fichiers

```
agent/src/
  presse_papier.rs                 M  +apres_notre_ecriture (garde n°1), +denormaliser,
                                      +borner_entrant. PUR. Tests EXTRAITS d'abord (T1)
  presse_papier/tests.rs           C  extraction verbatim de :242-428 (T1)
  presse_papier/win32.rs           M  +ecrire_texte  #[cfg(windows)], aucune décision
  capteur/protocole.rs             M  +VersCapteur::PressePapierEcrire. Tests EXTRAITS (T2)
  capteur/protocole/tests.rs       C  extraction verbatim de :298-464 (T2)
  capteur/sommeil/presse_papier.rs M  +ecrire(), qui arme le garde n°1 sur le Sondeur
  capteur/sommeil/registre.rs      M  +l'accès au Sondeur pour l'écriture, SOUS verrou
  capteur/fenetre/commandes.rs     M  +1 bras étage 0, +1 dans le bras miroir (:220-224)
  capteur/distante.rs              M  +ecrire_le_presse_papier -> commander_simple
  source.rs                        M  +méthode de trait, défaut Err (D-P2-9)
  transport/evenements.rs          M  +pending_clipboard dans memoriser_controle
  transport/tick.rs                M  +branche a1octies
  transport/boucle.rs              M  +le drainage de collage_a_injecter vers on_input
  input.rs                         M  +TOUCHES_COLLAGE (les 4 InputMessage::Key)
  demarrage.rs                     M  +presse_papier::actif() aux 3 capabilities.
                                      EXTRACTION PRÉALABLE (T3)
  demarrage/entrees.rs             C  extraction verbatim (T3) — candidat mesuré, cf. T3

proto/src/control.rs               M  +ClientControl::Clipboard, +Capabilities.clipboard
proto/ts/control.ts                M  +ClipboardClientMessage, +encodeClipboard, +témoin

client/src/
  raccourcis.ts                    C  PUR, sans DOM — le prédicat de D-P2-6
  raccourcis.test.ts               C
  presse-papier.ts                 M  +le garde n°3
  presse-papier.test.ts            M
  presse-papier-dom.ts             M  +l'écouteur `paste`, l'émission
  presse-papier-dom.test.ts        M
  input.ts                         M  l'exception étroite (D-P2-7)
  input.test.ts                    C  s'il n'existe pas — sinon M
  main.ts                          M  câblage SEUL

scripts/run-agent.sh               M  +PRESSE_PAPIER_GARDE  (T21, TÂCHE DÉDIÉE)

docs/superpowers/plans/
  journaux-presse-papier-p2/       M  les journaux de recette s'ajoutent à ceux du §0
  2026-08-20-presse-papier-p2-resultats.md  C  (T25)
```

---

## 6. Interfaces partagées

```rust
// agent/src/presse_papier.rs — PUR
/// Arme les gardes n°1 et n°2 de D5 sur NOTRE PROPRE écriture.
///
/// `seq` est le numéro de séquence relu JUSTE APRÈS `SetClipboardData`.
/// Poser `reference` fait sortir `observer` sur sa première ligne au tour
/// suivant : le presse-papier n'est même pas rouvert (garde n°1, exact).
/// Poser `dernier_emis` couvre le cas où une écriture TIERCE se serait
/// intercalée entre notre écriture et cette relecture (garde n°2).
pub fn apres_notre_ecriture(&mut self, seq: u32, texte: &str);

/// `\n` -> `\r\n`, la réciproque de `normaliser`. Windows attend `\r\n`.
pub fn denormaliser(texte: &str) -> String;

/// Rend `None` au-delà de `PRESSE_PAPIER_MAX` : on REFUSE, on ne tronque pas.
pub fn borner_entrant(texte: &str) -> Option<String>;

// agent/src/presse_papier/win32.rs — #[cfg(windows)], AUCUNE décision
pub fn ecrire_texte(texte: &str) -> Result<u32>;   // rend le seq relu APRÈS

// agent/src/capteur/protocole.rs
VersCapteur::PressePapierEcrire { texte: String }  // se répond par `Fait`

// agent/src/source.rs — trait VideoSource
fn ecrire_le_presse_papier(&mut self, _texte: &str) -> anyhow::Result<()> {
    anyhow::bail!("aucun capteur : le presse-papier de la VM n'est pas accessible")
}
```

```ts
// client/src/raccourcis.ts — PUR, sans DOM
export interface ToucheObservee {
    ctrlKey: boolean; shiftKey: boolean; altKey: boolean; metaKey: boolean; code: string;
}
export function estUnRaccourciDeCollage(e: ToucheObservee): boolean;

// client/src/presse-papier.ts — le garde n°3
/** Rend le texte à émettre, ou `undefined` si c'est un écho de ce qu'on a reçu. */
aEmettre(texte: string): string | undefined;

// proto/ts/control.ts
export interface ClipboardClientMessage { v: number; type: 'clipboard'; text: string }
export function encodeClipboard(text: string): string;
```

```rust
// proto/src/control.rs
ClientControl::Clipboard { version: u8, text: String }
AgentControl::Capabilities { version: u8, gamepad: bool, clipboard: bool }
```

---

## 7. Ordre et parallélisme

| Famille | Tâches | Dépend de | Parallélisable |
| --- | --- | --- | --- |
| 1 — extractions préalables | 1, 2, 3 | — | **oui, les trois** |
| 2 — modules purs | 4, 5, 6 | 4←1 | oui |
| 3 — protocole | 7, 8, 9 | 9←2 | oui |
| 4 — agent | 10, 11, 12, 13, 14, 15, 16, 17 | 10←4 ; 11←{4,10} ; 12←9 ; 13←9 ; 14←7 ; 15←{13,14} ; 16←15 ; 17←{3,7} | 10‖12‖14, puis 11‖13, puis 15, puis 16‖17 |
| 5 — navigateur | 18, 19, 20 | 18←6 ; 19←{5,8} ; 20←{18,19} | 18‖19 |
| 6 — banc et clôture | 21, 22, 23, 24, 25 | 21←11 ; 22←tout ; 23←22 ; 24←23 ; 25←23 | 24‖25 |

**Chemin critique :** 1 → 4 → 10 → 11 → 21 → 22 → 23 → {24, 25}.

⚠️ **Les tâches 9 et 12 se compilent ensemble** : le bras de `commandes.rs` ne
compile pas sans la variante de `protocole.rs`. **Si elles sont dispatchées
séparément, 9 précède 12.** Idem pour 7 avant 14 et 17.

---

# Famille 1 — les extractions préalables, AVANT toute addition

### Task 1 : 🔴 EXTRACTION de `agent/src/presse_papier.rs`, avant d'y ajouter le garde n°1

**Objet :** rendre au fichier la marge que la tâche 4 va lui prendre. **Aucun
changement de comportement, aucune ligne de logique touchée.**

**Files:**
- Create: `agent/src/presse_papier/tests.rs`
- Modify: `agent/src/presse_papier.rs`

**Relevé : 428 lignes, marge 72. Le bloc de tests va de `:242` (`#[cfg(test)]`)
à `:428` — 187 lignes. L'extraction rend 241.**

- [ ] **Step 0 :** `git log --oneline -5 -- agent/src/presse_papier.rs` —
      périmètre concurrent.
- [ ] **Step 1 : compter les tests AVANT.** `cargo test -p agent presse_papier`
      et noter le nombre. **Annoncé avant d'être relu.**
- [ ] **Step 2 : déplacer le bloc VERBATIM.** `#[cfg(test)] #[path =
      "presse_papier/tests.rs"] mod tests;` dans le parent. ⚠️ **Cette
      utilisation de `#[path]` est HORS de la portée de la convention de module
      enfant de `CLAUDE.md`** — c'est le même mécanisme employé pour une autre
      raison (la règle des 500 lignes), et `superviseur/table.rs` en porte deux
      précédents. **Ne pas hisser ce module à la racine.**
- [ ] **Step 3 : vérifier la transposition caractère pour caractère.** `git
      diff` ne doit montrer qu'un déplacement plus la ligne de déclaration.
- [ ] **Step 4 : voir vert**, avec **le même compte qu'au Step 1**. Un compte
      qui baisse est un test perdu — D10 en a perdu un ainsi, et ne l'a vu que
      parce que le compte avait été annoncé d'avance.
- [ ] **Step 5 : relever `wc -l` sur les deux fichiers** et le porter au rapport.

| Test | Ce qui le rend ROUGE |
| --- | --- |
| aucun test neuf | — **c'est une extraction, pas une addition** |

### Task 2 : 🔴 EXTRACTION de `agent/src/capteur/protocole.rs`, avant d'y ajouter la variante

**Objet :** identique à la tâche 1, sur le fichier dont la marge est la plus
serrée des trois après `demarrage.rs`.

**Files:**
- Create: `agent/src/capteur/protocole/tests.rs`
- Modify: `agent/src/capteur/protocole.rs`

**Relevé : 464 lignes, marge 36. Le bloc de tests va de `:298` à `:464` — 167
lignes. L'extraction rend 297.**

⚠️ **Sans elle, la variante `PressePapierEcrire` et sa documentation
porteraient le fichier tout près du plafond** : la documentation d'une variante
y est copieuse — `AudioMort` (`:102`) en porte **27 lignes** à elle seule
(`:75-101`, relevé par la commande) —, et c'est exactement la situation que
`CLAUDE.md` décrit quatre fois sous « la marge regagnée par une extraction se
reperd à la ronde suivante ».

- [ ] **Step 0 :** `git log --oneline -5 -- agent/src/capteur/protocole.rs`.
- [ ] **Step 1 à 5 :** identiques à la tâche 1, `#[path = "protocole/tests.rs"]`.

### Task 3 : 🔴 EXTRACTION de `agent/src/demarrage.rs` — marge 9, et le legs n°1 de P1 l'exigeait déjà

**Objet :** ce fichier est à **491 lignes**. Le legs n°1 de P1 le nomme comme
point de chute du propriétaire mono-fenêtre **en écrivant qu'une extraction
préalable y sera requise**. P2 y ajoute moins que cela — la lecture de
`PRESSE_PAPIER` et un champ aux trois `capabilities` —, **et c'est déjà trop
pour une marge de 9.**

**Files:**
- Create: `agent/src/demarrage/entrees.rs`
- Modify: `agent/src/demarrage.rs`

**Candidat MESURÉ, et le découpage exact reste au jugement de l'implémenteur :**
la fermeture `on_input` et son voisinage manette vivent dans le bloc
`spawn_blocking` ouvert en `demarrage.rs:311` ; `on_input` est déclarée en
`:337` et `on_control` en `:415`. Le corps de la manette (branchement paresseux,
`pad_indisponible`, `connexion_manette`) est un ensemble cohérent, sans rapport
avec le presse-papier, et `demarrage/` porte déjà trois enfants (`audio.rs`,
`micro.rs`, `source.rs`).

- [ ] **Step 0 :** `git log --oneline -5 -- agent/src/demarrage.rs` — ⚠️ **ce
      fichier est sous périmètre concurrent, et sa marge peut avoir changé.
      Relever `wc -l` avant de décider quoi extraire.**
- [ ] **Step 1 : `cargo test -p agent`**, compte annoncé d'avance.
- [ ] **Step 2 : extraire VERBATIM**, sans réécrire une ligne de logique. Une
      extraction qui « améliore » au passage n'est plus vérifiable par `git diff`.
- [ ] **Step 3 : viser une marge d'au moins 60 lignes après extraction**, et
      **écrire le chiffre obtenu**. Si l'extraction rend moins que cela, elle
      n'a pas résolu le problème : le dire plutôt que de passer.
- [ ] **Step 4 : `cargo check --target x86_64-pc-windows-gnu`** — ce fichier
      porte du `#[cfg(windows)]`, et l'hôte ne le compile pas autrement.
- [ ] **Step 5 : voir vert, même compte.**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| aucun test neuf | — extraction |
| ⚠️ le contrôle qui vaut est `cargo check --target x86_64-pc-windows-gnu` | une extraction qui casse un `#[cfg(windows)]` **passe `cargo test` sur l'hôte** : c'est le seul contrôle qui puisse échouer ici |

---

# Famille 2 — les modules purs, et deux des trois gardes

### Task 4 : `agent/src/presse_papier.rs` — le garde n°1, et la réciproque de `normaliser`

**Objet :** la logique décisionnelle du sens entrant. **PUR, aucun `cfg`,
entièrement éprouvable sur l'hôte** — c'est tout le mérite du découpage de P1.

**Files:**
- Modify: `agent/src/presse_papier.rs`, `agent/src/presse_papier/tests.rs`

**Dépend de la tâche 1.**

**Interfaces :** `apres_notre_ecriture`, `denormaliser`, `borner_entrant` — §6.

- [ ] **Step 1 : 🔴 écrire les tests d'abord et les voir ROUGES.**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| 🔴 `apres_notre_ecriture(s, t)` puis `observer(s, …)` rend `None` **sans appeler la fermeture de lecture** | **c'est le garde n°1**. Le test passe une fermeture qui **panique** si elle est appelée : sans la pose de `reference`, `observer` la lit et le test explose. ⚠️ Un test qui se contenterait de vérifier `None` serait satisfait par le garde n°2 et **ne mesurerait pas le n°1** |
| 🔴 `apres_notre_ecriture(s, t)` puis `observer(s+1, ‖ t)` rend `None` | **c'est le garde n°2 armé sur notre écriture**, le cas « une écriture tierce s'est intercalée ». Rouge = ne poser que `reference` |
| 🔴 aller-retour `normaliser(denormaliser(x)) == x` sur un texte multi-ligne | rouge = une réciproque qui double les `\r`. **C'est ce que la spec §7.1 désigne comme « la première chose qu'un test doit voir rouge »** |
| `denormaliser` sur un texte SANS saut de ligne le rend inchangé | — |
| 🔴 `denormaliser` sur un texte portant DÉJÀ des `\r\n` ne les double pas | rouge = un `replace("\n", "\r\n")` naïf. **C'est le cas réel** : le texte vient d'un navigateur, mais rien ne garantit qu'il n'a pas de `\r\n` |
| `borner_entrant` rend `None` à `PRESSE_PAPIER_MAX + 1` octets et `Some` à `PRESSE_PAPIER_MAX` | rouge = un `>=` au lieu d'un `>` — une erreur de borne qui refuserait le cas limite exact |
| 🔴 `borner_entrant` compte des OCTETS UTF-8, pas des `char` | rouge = `texte.chars().count()`. Test avec un texte d'emojis dont le compte de `char` passe et celui d'octets non |

- [ ] **Step 2 : écrire le code.** `apres_notre_ecriture` pose les **deux**
      champs, et son commentaire dit **lequel est quel garde de D5**.
- [ ] **Step 3 : vérifier que les 13 tests existants de ce module restent
      verts** (`grep -c '#\[test\]' agent/src/presse_papier.rs` rend **13** au
      20 août 2026 — le relever de nouveau, la tâche 1 les aura déplacés).
- [ ] **Step 4 : voir vert.** `cargo test -p agent` — **671 + les tests neufs**,
      annoncé avant d'être lu. ⚠️ **Relever 671 de nouveau d'abord** : un
      chantier concurrent travaille dans `agent/`.

### Task 5 : `client/src/presse-papier.ts` — le garde n°3

**Objet :** la page ne réémet jamais vers l'agent un contenu qu'elle vient de
recevoir de lui. **PUR, sans DOM.**

**Files:**
- Modify: `client/src/presse-papier.ts`, `client/src/presse-papier.test.ts`

**Relevé : 112 lignes.** `PressePapierLocal` porte déjà `recevoir`, `aEcrire`,
`confirmer`, `echouer`, `refusADire` ; `aEmettre` s'y ajoute.

- [ ] **Step 1 : 🔴 les tests d'abord, ROUGES.**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| 🔴 `recevoir({texte: 'x'})` puis `aEmettre('x')` rend `undefined` | **c'est le garde n°3**. Rouge = `aEmettre` qui rend toujours son argument |
| `recevoir({texte: 'x'})` puis `aEmettre('y')` rend `'y'` | rouge = un garde qui bloquerait tout après une réception. **Sans ce test, un `aEmettre` qui rend toujours `undefined` passerait le test précédent** |
| 🔴 `aEmettre('x')` **avant toute réception** rend `'x'` | rouge = un état initial qui compare à la chaîne vide et bloquerait un collage de chaîne vide |
| 🔴 `recevoir({texte: 'x'})`, `aEmettre('x')` → `undefined`, puis `aEmettre('x')` **de nouveau** rend `'x'` | **le garde ne vaut que pour le PREMIER renvoi** : un utilisateur qui colle deux fois le même texte le veut deux fois. Rouge = un garde permanent |
| `recevoir({texte: null, octets: 100000})` (un refus) puis `aEmettre('x')` rend `'x'` | rouge = un garde qui prendrait `null` pour un contenu reçu |

- [ ] **Step 2 : écrire `aEmettre`.** Il consomme le témoin, comme
      `sommeil_a_annoncer` côté agent : c'est ce que le quatrième test impose.
- [ ] **Step 3 : voir vert.** `npx vitest run` → **304 + les tests neufs**,
      annoncé avant lecture.

### Task 6 : `client/src/raccourcis.ts` — le prédicat, et LA défense contre R5

**Objet :** décider si un `keydown` est un raccourci de collage. **PUR, sans
DOM, un module neuf** — parce que cette condition est la seule chose qui sépare
le produit du risque R5 (« élargir la condition rendrait le navigateur au
clavier »), et qu'une condition inlinée dans un écouteur n'est pas testable.

**Files:**
- Create: `client/src/raccourcis.ts`, `client/src/raccourcis.test.ts`
- Modify: aucun

**Interfaces :** `estUnRaccourciDeCollage` — §6. Condition exacte : D-P2-6.

- [ ] **Step 1 : 🔴 les tests d'abord, ROUGES. La table de vérité est le test.**

| Entrée | Attendu | Ce qui rend le test ROUGE |
| --- | --- | --- |
| `Ctrl+KeyV` | **vrai** | une condition qui ne reconnaît rien |
| `Shift+Insert` | **vrai** | oublier le second raccourci de D6 |
| 🔴 `Ctrl+KeyW` | **faux** | **le risque R5 lui-même** : rouge = tester `e.ctrlKey` seul. `Ctrl+W` ferme la fenêtre de session |
| 🔴 `Ctrl+KeyT`, `Ctrl+KeyN` | **faux** | idem |
| 🔴 `Ctrl+Shift+KeyV` | **faux** | rouge = omettre `!e.shiftKey`. C'est « coller sans mise en forme » dans plusieurs applications, et il **doit** rester au produit |
| 🔴 `Ctrl+Alt+KeyV` | **faux** | rouge = omettre `!e.altKey` |
| 🔴 `Meta+KeyV` | **faux** | rouge = omettre `!e.metaKey`. Sur macOS ce serait le collage natif ; **le produit ne le traite pas en v1, et ce test fige la décision** |
| `KeyV` seul | **faux** | rouge = tester le seul `code` |
| 🔴 `Ctrl+Shift+Insert` | **faux** | rouge = ne pas exiger `!e.ctrlKey` sur la branche `Insert` |

- [ ] **Step 2 : écrire le prédicat**, avec en commentaire **le relevé du §0.2
      qui l'autorise à être si étroit** — `ControlLeft` garde son
      `preventDefault` et le `paste` arrive quand même, mesuré deux fois.
- [ ] **Step 3 : voir vert**, compte annoncé.

---

# Famille 3 — le protocole partagé

### Task 7 : `proto/src/control.rs` — `ClientControl::Clipboard` et `Capabilities.clipboard`

**Objet :** les deux additions au protocole, côté Rust. **`CONTROL_VERSION`
reste à 3** (D7 de la spec, et P1 l'a déjà tenu pour `AgentControl::Clipboard`).

**Files:**
- Modify: `proto/src/control.rs`, `proto/src/control/tests.rs`

**Relevé : 327 lignes, marge 173** — P1 a extrait les tests
(`proto/src/control/tests.rs`, **237** lignes, relevé par la commande)
précisément pour cela. **Aucune extraction n'est due ici.**

- [ ] **Step 1 : 🔴 les tests d'abord, ROUGES.**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| un `{"v":3,"type":"clipboard","text":"x"}` se désérialise en `ClientControl::Clipboard` | rouge = la variante absente |
| 🔴 un `{"v":2,…}` est **rejeté** | rouge = oublier `deserialize_with = "verifie_version"` sur le champ `version` — **c'est une ligne qu'on oublie en recopiant une variante voisine**, et rien d'autre ne le verrait |
| 🔴 un champ `bytes` en trop est **rejeté** | `deny_unknown_fields` (`proto/src/control.rs:86`). Rouge = poser la variante sur un autre enum |
| `Capabilities` **sans** champ `clipboard` se désérialise, `clipboard == false` | 🔴 rouge = oublier `#[serde(default)]`. ⚠️ **La raison à écrire est celle du commentaire de `mic` (`:126-129`), pas celle de la spec** — voir E6 |
| `Capabilities` sérialisé porte **les deux** champs | rouge = un `skip_serializing_if` |

- [ ] **Step 2 : la variante `ClientControl::Clipboard { version, text: String }`.**
      ⚠️ **`text` est `String`, pas `Option<String>`** — asymétrie voulue avec
      `AgentControl::Clipboard`, dont le `Option` **porte le refus** (spec D4).
      Le sens entrant n'a pas de refus à exprimer : le client borne avant
      d'émettre, et l'agent refuse en journalisant. **Écrire cette asymétrie en
      commentaire**, sans quoi un lecteur la prendra pour un oubli.
- [ ] **Step 3 : `Capabilities.clipboard: bool` avec `#[serde(default)]`**, et
      le constructeur `capabilities(gamepad, clipboard)`.
- [ ] **Step 4 : `AgentControl::Capabilities` porte un `match` exhaustif dans
      `agent/src/transport/controle.rs`** — ajouter un CHAMP ne le casse pas
      (le bras est `Capabilities { .. }`), donc **rien à faire là**, et le
      vérifier plutôt que de le supposer.
- [ ] **Step 5 : voir vert.** `cargo test -p proto`, compte annoncé.

### Task 8 : `proto/ts/control.ts` — `ClipboardClientMessage` et son encodeur

**Objet :** le miroir TypeScript. **Le nom est déjà réservé par P1** (E9).

**Files:**
- Modify: `proto/ts/control.ts`, et le test de `proto/ts/` qui couvre les
  encodeurs (le relever, ne pas le supposer)

**Relevé : 184 lignes, marge 316.**

- [ ] **Step 1 : 🔴 les tests d'abord.**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `encodeClipboard('x')` rend `{"v":3,"type":"clipboard","text":"x"}` | rouge = l'encodeur absent |
| 🔴 `CapabilitiesMessage.clipboard` est **optionnel** (`clipboard?: boolean`) | rouge = le rendre obligatoire — un agent d'avant P2 ne le porte pas, et `undefined` doit valoir `false` **gratuitement**, exactement comme `mic` (`proto/ts/control.ts`, doc de `ReadyMessage.mic`) |
| ❌ ~~`parseAgentControl` accepte un `capabilities` **sans** `clipboard`~~ **CE TEST N'A PAS ÉTÉ ÉCRIT** | ✅ **La réserve du plan se vérifie : `parseAgentControl` ne valide QUE `v` et `type`, puis CASTE** (`parsed as AgentControl`). Le test aurait été **incapable d'échouer**. Ce qui porte la propriété est le TYPAGE (`clipboard?: boolean`), et ce qui la mesure est une annotation explicite dans `control.test.ts` — dont la rouge est `tsc`, pas vitest : rendre le champ obligatoire fait sortir `TS2741`. Constat inscrit **dans le code**, au-dessus de `parseAgentControl` |

- [ ] **Step 2 : 🔴 NE PAS écrire de `TYPES_CLIENT`, et écrire pourquoi.**
      `ClientControl` n'est **jamais** parsé côté TypeScript — le client encode
      — et il est désérialisé côté Rust par `serde` avec un `match` exhaustif
      (E4). **Un témoin d'exhaustivité côté client serait un contrôle sans
      objet**, c'est-à-dire un contrôle incapable d'échouer. `TOUS_AGENT`
      (`proto/ts/control.ts:137`) couvre le sens agent → client, et P1 l'a déjà
      posé.
- [ ] **Step 3 : voir vert.**

### Task 9 : `agent/src/capteur/protocole.rs` — `VersCapteur::PressePapierEcrire`

**Objet :** la commande enfant → capteur. **Se répond par `Fait`**, à la
différence de `Visibilite`, `AudioMort` et `AudioVivant`, qui ne se répondent
pas — et la doc doit dire **pourquoi la différence** : ici l'appelant a besoin
de savoir que l'écriture a eu lieu **avant** d'injecter `Ctrl+V` (D6).

**Files:**
- Modify: `agent/src/capteur/protocole.rs`, `agent/src/capteur/protocole/tests.rs`

**Dépend de la tâche 2. Relevé après extraction : ~297 lignes.**

- [ ] **Step 1 : les tests d'abord.** Le fichier a un patron de tests de
      sérialisation aller-retour : le suivre.

| Test | Ce qui le rend ROUGE |
| --- | --- |
| aller-retour `PressePapierEcrire { texte }` | rouge = la variante absente |
| 🔴 un texte de 64 KiB traverse `ecrire_json`/`lire_trame` | rouge = un dépassement de `TAILLE_MAX` (8 MiB, `protocole.rs`). **Le test dit que la borne du canal capteur↔enfant n'est PAS le facteur contraignant**, ce que la spec D4 affirme sans l'avoir éprouvé |

- [ ] **Step 2 : écrire la variante**, avec sa doc, en disant **explicitement**
      qu'elle se répond par `Fait` et pourquoi.
- [ ] **Step 3 : voir vert**, et **relever `wc -l`** — le fichier doit rester
      loin de 500 après extraction ET addition.

---

# Famille 4 — le trajet dans l'agent

### Task 10 : `agent/src/presse_papier/win32.rs` — `ecrire_texte`, et AUCUNE décision

**Objet :** `OpenClipboard` / `EmptyClipboard` / `GlobalAlloc` /
`SetClipboardData` / `CloseClipboard`, puis **relire le numéro de séquence** et
le rendre. **Aucune décision** : le bornage et la normalisation sont déjà faits
par la tâche 4.

**Files:**
- Modify: `agent/src/presse_papier/win32.rs`

**Relevé : 98 lignes.** ⚠️ **Le chemin d'écriture existe DÉJÀ dans ce dépôt**,
mais dans la sonde : `agent/src/diagnostics/presse_papier.rs` porte un `mod win`
privé qui importe `SetClipboardData`, `EmptyClipboard` et `GlobalAlloc` pour ses
phases C et D. **Le relire avant d'écrire** — il a tourné sur la VM, deux
exécutions.

- [ ] **Step 0 :** `git log --oneline -5 -- agent/src/presse_papier/`.
- [ ] **Step 1 : lire `diagnostics/presse_papier.rs::mod win`** et reprendre ce
      qui y est éprouvé, sans le recopier aveuglément — la sonde n'a pas les
      mêmes contraintes de durée de vie que le produit.
- [ ] **Step 2 : écrire `ecrire_texte`.** Points de rigueur :
      - `CF_UNICODETEXT`, UTF-16 **terminé par un `\0`** ;
      - `GMEM_MOVEABLE`, et **le handle appartient au presse-papier après
        `SetClipboardData` : ne pas le libérer** ;
      - le garde RAII `PressePapierOuvert` **existe déjà** dans ce fichier
        (`impl Drop`) : le réutiliser, ne pas en écrire un second ;
      - **`EmptyClipboard` avant `SetClipboardData`**, faute de quoi les formats
        de l'application précédente survivent et le collage est imprévisible ;
      - **relire `numero_de_sequence()` APRÈS `CloseClipboard`**, et le rendre.
        ⚠️ Le relire **avant** la fermeture rendrait un compteur que la fermeture
        peut encore faire bouger, et **le garde n°1 serait faux d'un cran** —
        c'est-à-dire silencieusement inopérant.
- [ ] **Step 3 : `OpenClipboard` qui échoue est un cas NORMAL** (une autre
      application le tient — spec R7) : rendre `Err`, **jamais** boucler en
      attente. L'appelant journalise et n'injecte pas.
- [ ] **Step 4 : `cargo check --target x86_64-pc-windows-gnu`** — **c'est le
      SEUL contrôle possible sur l'hôte pour ce fichier**, il est
      `#[cfg(windows)]` et sans test. Le dire dans le rapport.

| Test | Ce qui le rend ROUGE |
| --- | --- |
| aucun | ⚠️ **`#[cfg(windows)]`, aucune décision, aucun test — comme `presse_papier/win32.rs` l'est déjà pour la lecture.** La seule épreuve est la recette (tâche 22, critère ②) |

### Task 11 : `agent/src/capteur/sommeil/presse_papier.rs` — le propriétaire écrit, et arme le garde n°1

**Objet :** l'écriture par le seul propriétaire (D1), et **l'armement des gardes
dans le même geste**.

**Files:**
- Modify: `agent/src/capteur/sommeil/presse_papier.rs`,
  `agent/src/capteur/sommeil/registre.rs`

**Dépend des tâches 4 et 10. Relevé : 193 et 378 lignes.**

🔴 **Le point qui décide de la correction de tout P2 :** le `Sondeur` vit dans
`registre.rs` (`let mut sondeur = crate::presse_papier::Sondeur::nouveau();`,
sur le fil du tour de roue), et **son `tour()` est appelé HORS du verrou**
pendant que `distribuer` s'exécute **sous** verrou. L'écriture, elle, arrive
d'un **autre fil** (le fil de fenêtre qui sert les commandes).

- [ ] **Step 1 : trancher où vit le `Sondeur`, et l'écrire.** Deux voies, et
      **une seule est sûre** :
      - ❌ laisser le `Sondeur` local au fil du tour de roue : le fil de fenêtre
        ne peut alors pas armer le garde n°1, et **il n'y a aucun moyen de le
        faire sans course** ;
      - ✅ **poser le numéro de séquence et le texte de notre écriture dans
        `Etat`** (le registre déjà protégé par `OnceLock<Mutex<Etat>>`), et
        laisser le tour de roue les consommer pour appeler
        `apres_notre_ecriture` **avant** son `tour()`.
      ⚠️ **La seconde voie déplace la course au lieu de la supprimer**, et il
      faut le dire : entre notre `SetClipboardData` et la consommation par le
      tour de roue, jusqu'à `PERIODE_PRESSE_PAPIER` (250 ms) peut s'écouler. Si
      une **autre** copie survient dans cet intervalle, poser `reference` sur
      *notre* `seq` **ne la masque pas** — le compteur aura encore bougé. **Le
      garde reste donc exact au sens de D5**, et c'est ce qu'il faut vérifier
      par un test.
- [ ] **Step 2 : 🔴 les tests d'abord.**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| 🔴 après une écriture posée dans `Etat`, le tour suivant n'émet **rien** | rouge = ne pas consommer, ou consommer **après** `tour()`. **L'ordre est le test** |
| 🔴 une copie TIERCE survenue après notre écriture est **quand même émise** | rouge = un garde qui poserait `reference` sur une valeur future ou qui absorberait tout après une écriture. **Sans ce test, un garde trop large passerait le précédent** |
| l'écriture échouée (`Err` de `win32`) **n'arme aucun garde** | rouge = armer avant de savoir. Le contenu resterait alors invisible à jamais |

- [ ] **Step 3 : `PRESSE_PAPIER=0` interdit AUSSI l'écriture.** `actif()` est
      déjà testé dans `Sondeur::tour()` avant toute lecture ; le chemin
      d'écriture doit le tester **aussi**, et la trace doit le dire. Rouge = une
      écriture qui passe alors que la variable désarme.
- [ ] **Step 4 : voir vert**, compte annoncé.

### Task 12 : `agent/src/capteur/fenetre/commandes.rs` — le bras étage 0, ET son miroir

**Objet :** router `PressePapierEcrire` vers le propriétaire. **C'est une
commande d'étage 0** — elle ne touche ni encodeur ni duplication —, comme
`Visibilite` (`:97`), `AudioMort` et `AudioVivant`.

**Files:**
- Modify: `agent/src/capteur/fenetre/commandes.rs`

**Dépend de la tâche 9. Relevé : 234 lignes, marge 266.**

🔴 **Les commandes d'étage 0 sont listées DEUX fois dans ce fichier** : le bras
qui les exécute, et le bras **miroir** de `:220-224` qui les énumère
(`VersCapteur::Attache { .. } | Identite | Visibilite | AudioMort |
AudioVivant`). **Oublier le second ne compile pas** — c'est un `match` exhaustif
—, mais le plan le nomme pour qu'on ne le découvre pas.

- [ ] **Step 1 : le bras étage 0**, qui appelle le propriétaire et rend
      `DepuisCapteur::Fait` — ou `DepuisCapteur::Erreur { motif }` sur échec.
- [ ] **Step 2 : le bras miroir de `:220-224`.**
- [ ] **Step 3 : vérifier que `cargo check --target x86_64-pc-windows-gnu`
      passe** avant de croire que le miroir est complet.

| Test | Ce qui le rend ROUGE |
| --- | --- |
| ⚠️ **le compilateur est le test** — les deux `match` sont exhaustifs | rouge = retirer un bras : `cargo check` échoue. **Ce contrôle-là ne peut pas être vacueux**, mais il ne prouve rien de la sémantique |

### Task 13 : `agent/src/source.rs` et `agent/src/capteur/distante.rs` — `ecrire_le_presse_papier`

**Objet :** la méthode de trait, et son unique implémentation réelle.

**Files:**
- Modify: `agent/src/source.rs`, `agent/src/capteur/distante.rs`

**Dépend de la tâche 9. Relevé : 414 et 446 lignes.**

- [ ] **Step 1 : la méthode de trait, à défaut `Err`** — **jamais `Ok(())`**
      (D-P2-9). Son commentaire porte la raison : un `Ok` ferait coller le
      contenu précédent, silencieusement.
      ⚠️ **C'est une rupture de patron dans ce fichier**, où
      `signaler_audio_mort`, `signaler_audio_vivant` et
      `presse_papier_a_annoncer` ont tous un défaut **inerte**. **L'écrire**,
      sans quoi un lecteur l'alignera sur ses voisines.
- [ ] **Step 2 : l'implémentation de `SourceDistante`** —
      `self.commander_simple(VersCapteur::PressePapierEcrire { texte })`, sur le
      patron exact de `set_awake` (`agent/src/capteur/distante.rs:376`).
- [ ] **Step 3 : 🔴 le test.**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| 🔴 une source factice **sans capteur** rend `Err` | rouge = le défaut inerte. ⚠️ **Attention au piège de D10** : les sources factices du dépôt implémentent des effets de bord en **no-op**, et 456 tests sont restés verts sur un produit muet. **Ce test doit porter sur le DÉFAUT DU TRAIT**, pas sur une factice qui l'aurait redéfini |
| `SourceDistante` transmet bien la commande et rend `Ok` sur `Fait` | rouge = `commander_simple` non appelé |
| 🔴 `SourceDistante` rend `Err` sur `DepuisCapteur::Erreur` | rouge = accepter n'importe quelle réponse. `commander_simple` le fait déjà (`:169-175`) : **le test vérifie qu'on l'a bien employé, lui, et pas `commander` nu** |

### Task 14 : `agent/src/transport/evenements.rs` — `pending_clipboard`

**Objet :** mémoriser, jamais appliquer sur-le-champ.

**Files:**
- Modify: `agent/src/transport/evenements.rs`, et le champ dans la structure de
  session (le relever, il n'est pas dans ce fichier)

**Dépend de la tâche 7. Relevé : 363 lignes, marge 137.**

- [ ] **Step 1 : le bras dans `memoriser_controle`** (`:334-343`), à côté de
      `Resize` (`:336`) et `Visibility` (`:339`).
- [ ] **Step 2 : écrire l'écrasement du dernier ET son coût** (D-P2-3), dans le
      commentaire du champ, pas seulement dans ce plan.
- [ ] **Step 3 : le test**, par `dispatch_controle_de_test` — le point d'entrée
      `#[cfg(test)]` que ce fichier expose déjà exactement pour cela.

| Test | Ce qui le rend ROUGE |
| --- | --- |
| un `{"v":3,"type":"clipboard","text":"x"}` pose `pending_clipboard` | rouge = le bras absent — **mais le `match` est exhaustif, donc cela ne compile pas** : la vraie rouge est un bras qui poserait le mauvais champ |
| 🔴 deux messages successifs laissent **le second** | rouge = une accumulation. **C'est la décision D-P2-3 rendue vérifiable**, et le seul endroit où elle l'est |

### Task 15 : `agent/src/transport/tick.rs` — la branche `a1octies`

**Objet :** écrire le presse-papier, **puis** armer l'injection. C'est le point
où l'ordre de D6 est produit.

**Files:**
- Modify: `agent/src/transport/tick.rs`

**Dépend des tâches 13 et 14. Relevé : 421 lignes, marge 79.**

- [ ] **Step 1 : la branche, après `a1septies`** (`:355`, la branche
      presse-papier de P1) — et **mettre à jour la liste d'audit de tête** du
      fichier, qui énumère les branches `a1…`. ⚠️ **Cette liste est un point de
      passage non gardé par le compilateur** : l'oublier laisse le fichier
      mentir sur lui-même, et c'est exactement le défaut que la revue transverse
      de D10 a trouvé douze fois.
- [ ] **Step 2 : l'ordre.** `pending_clipboard.take()` → `ecrire_le_presse_papier`
      → **si `Ok`** poser `collage_a_injecter`. **Sur `Err` : journaliser en
      `warn!` avec la `session`, et NE PAS armer** (D6 : la touche est perdue,
      pas reportée).
- [ ] **Step 3 : l'invariant de drainage.** Cette branche **ne mute pas
      `self.rtc`** — comme `a1quater` et `a1quinquies`. **Le vérifier et
      l'écrire** : le fichier porte soixante lignes d'audit sur ce point, et une
      affirmation fausse y a déjà coûté deux rondes (D6, consignation n°3).
- [ ] **Step 4 : le test.**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| 🔴 un `pending_clipboard` posé fait appeler `ecrire_le_presse_papier` **puis** armer le drapeau | rouge = armer avant d'écrire. **Le test doit observer l'ORDRE**, pas seulement les deux effets — une source factice qui enregistre une trace ordonnée |
| 🔴 une écriture qui rend `Err` **n'arme pas** le drapeau | rouge = armer inconditionnellement. **C'est le mode de défaillance que D6 existe pour éviter** : sans ce test, `Ctrl+V` collerait le contenu précédent |
| `pending_clipboard` est **consommé** (un second tour n'écrit rien) | rouge = un `.clone()` au lieu d'un `.take()` — le collage se répéterait à chaque tour de boucle |

### Task 16 : `agent/src/transport/boucle.rs` et `agent/src/input.rs` — l'injection

**Objet :** drainer le drapeau vers `on_input`, et poser les quatre touches.

**Files:**
- Modify: `agent/src/transport/boucle.rs`, `agent/src/input.rs`

**Dépend de la tâche 15. Relevé : 93 et 250 lignes.**

- [ ] **Step 1 : dans `run`** (`agent/src/transport/boucle.rs:45`), **juste
      après** `act_on_timeout` (`:53`), consommer le drapeau et appeler
      `on_input` quatre fois. ⚠️ **`on_input` est déjà un paramètre de `run`**
      (`:47`) : aucune signature ne change (D-P2-1).
- [ ] **Step 2 : les quatre `InputMessage::Key`**, par scancodes, **en constante
      nommée dans `agent/src/input.rs`** — Ctrl gauche `0x1d`, `V` `0x2f`,
      `extended: false` **aux deux** (relevé sur `client/src/scancodes.ts:41`
      et `:59`, la table que le client emploie déjà). **Ne pas les inliner dans `boucle.rs`** : un scancode
      inliné dans la boucle de transport est illisible et intestable.
- [ ] **Step 3 : ne rien ajouter à `InputInjector`.** Le bras
      `InputMessage::Key` (`agent/src/input.rs:118`) appelle **déjà**
      `au_premier_plan()` (`:119`, fonction en `:155`). **Le vérifier avant
      d'écrire une méthode `coller()` qui ferait double emploi.**
- [ ] **Step 4 : le test.**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| 🔴 le drapeau armé produit **exactement quatre** `InputMessage::Key`, dans l'ordre Ctrl↓ V↓ V↑ Ctrl↑ | rouge = omettre le `Ctrl`↑ — l'application resterait avec un modificateur enfoncé, et **toute frappe suivante deviendrait un raccourci**. C'est le défaut le plus insidieux de cette tâche |
| le drapeau est **consommé** : un second tour n'injecte rien | rouge = un booléen jamais remis à faux — `Ctrl+V` à chaque tour de boucle |
| drapeau non armé ⟹ **zéro** touche | rouge = injecter à chaque tour |

### Task 17 : `agent/src/demarrage.rs` — `Capabilities.clipboard`

**Objet :** annoncer la capacité au client, honnêtement.

**Files:**
- Modify: `agent/src/demarrage.rs`

**Dépend des tâches 3 et 7.**

- [ ] **Step 0 : relever `wc -l` après l'extraction de la tâche 3**, et
      **écrire le chiffre**. Si la marge est inférieure à 30, **s'arrêter et le
      dire** plutôt que d'ajouter.
- [ ] **Step 1 : les TROIS appels** — `:291`, `:363`, `:383` — passent
      `presse_papier::actif()`. ⚠️ **Les deux derniers sont des replis de
      MANETTE** : `clipboard` doit y valoir **la même chose** qu'au premier.
      Rouge = un `capabilities(false, false)` recopié qui éteindrait aussi le
      presse-papier quand la manette échoue.
- [ ] **Step 2 : écrire la condition de validité de l'annonce** (D-P2-8) : elle
      tient parce que capteur et enfant **lisent la même variable héritée**
      (`agent/src/superviseur/lanceur.rs:249` n'efface pas `PRESSE_PAPIER`), et
      **elle deviendrait fausse en silence** si le capteur décidait un jour
      autrement. **Le commentaire porte cette condition, pas seulement le fait.**
- [ ] **Step 3 : `cargo check --target x86_64-pc-windows-gnu`.**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| 🔴 le contrôle qui vaut est le `grep` de la tâche 22, pas un test d'hôte | ce fichier n'a pas de test unitaire de ce chemin. **Le dire**, plutôt que d'écrire un test qui n'exercerait que le constructeur |

---

# Famille 5 — le navigateur

### Task 18 : `client/src/input.ts` — l'exception étroite, et RIEN d'autre

**Objet :** la seule modification de ce fichier par tout le chantier ①.

**Files:**
- Modify: `client/src/input.ts`
- Create: `client/src/input.test.ts` **s'il n'existe pas** — le relever avant

**Dépend de la tâche 6. Relevé : 123 lignes, marge 377.**

- [ ] **Step 1 : 🔴 le test d'abord.** ⚠️ **`attachInput` prend un
      `HTMLVideoElement` et un `RTCDataChannel`** : le test emploie des doubles,
      sur le patron des tests DOM existants du client. **Si le montage s'avère
      trop lourd, dire pourquoi et déplacer la couverture sur `raccourcis.ts`
      (tâche 6) — mais ALORS le rapport doit écrire que la LIAISON entre le
      prédicat et l'écouteur n'est couverte par rien**, et c'est un legs.
      ✅ **CE REPLI N'A PAS ÉTÉ EMPRUNTÉ.** Le montage était bien impossible tel
      quel — la suite du client tourne en environnement `node`, sans DOM, et
      `window.addEventListener` y lève —, et le remède est celui que ce dépôt
      emploie partout ailleurs : **injecter la cible**. `CibleClavier` rejoint
      `CibleFocus` (`presse-papier-dom.ts`) et `CibleEcran` (`fullscreen.ts`),
      et `client/src/input.test.ts` (neuf, **10 tests**) couvre la liaison,
      **dont les trois cas du risque R5**. ⚠️ Ce que ce fichier ne couvre pas et
      le dit en tête : le pointeur, la molette et le menu contextuel, sans
      couverture avant P2 et qui le restent.

| Test | Ce qui le rend ROUGE |
| --- | --- |
| 🔴 `Ctrl+V` : `preventDefault` **PAS** appelé, et **aucun** octet envoyé | rouge = le `preventDefault()` inconditionnel de `:95`, celui d'aujourd'hui. **C'est la rouge centrale de P2, et le §0 établit qu'elle est atteignable** |
| 🔴 `Ctrl+W` : `preventDefault` **appelé**, et l'octet envoyé | rouge = une condition élargie. **C'est le risque R5** |
| `ControlLeft` seul : `preventDefault` appelé, octet envoyé | rouge = retenir le modificateur (D-P2-7) — la VM perdrait un `Ctrl` que l'utilisateur tient peut-être pour autre chose |
| 🔴 `keyup` de `KeyV` sous Ctrl : `preventDefault` **appelé**, **aucun** octet | rouge = laisser partir le relâchement seul. La VM verrait un `V`↑ sans `V`↓, ce qui peut débloquer une répétition |

- [ ] **Step 2 : écrire l'exception**, en appelant `estUnRaccourciDeCollage`
      (tâche 6) — **jamais une condition inlinée**.
- [ ] **Step 3 : commenter en citant le relevé du §0**, avec **le chemin du
      journal**, pour que le prochain lecteur n'ait pas à croire sur parole.

### Task 19 : `client/src/presse-papier-dom.ts` — l'écouteur `paste` et l'émission

**Objet :** capter le `paste`, borner, appliquer le garde n°3, émettre.

**Files:**
- Modify: `client/src/presse-papier-dom.ts`, `client/src/presse-papier-dom.test.ts`

**Dépend des tâches 5 et 8. Relevé : 101 lignes, marge 399.**

- [ ] **Step 1 : 🔴 les tests d'abord.**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| 🔴 un `paste` portant `text/plain` fait émettre `encodeClipboard(texte)` | rouge = l'écouteur absent |
| 🔴 un `paste` **vide** n'émet **rien** | rouge = émettre une chaîne vide — elle viderait le presse-papier de la VM sans que l'utilisateur l'ait demandé |
| 🔴 un `paste` **au-delà de la borne** n'émet rien et **dit** le refus | rouge = émettre 64 KiB + 1. ⚠️ **La borne côté client est OBLIGATOIRE** : sans elle, l'agent la ferait respecter mais le canal aurait déjà porté la charge, et le bandeau ne paraîtrait jamais |
| 🔴 un texte **qu'on vient de recevoir** n'est pas réémis | **c'est le garde n°3 câblé** (tâche 5). Rouge = ne pas appeler `aEmettre` |
| l'écouteur est bien **retiré** par `detacher()` | rouge = une fuite — `main.ts` détache déjà à la fin de session (`:177`), et un écouteur survivant émettrait pour une session morte |

- [ ] **Step 2 : l'écouteur va sur la CIBLE injectée** (`options.cible`, déjà
      `window` en production — `client/src/main.ts` le câble ainsi). ⚠️ **Le
      §0 établit que le `paste` a pour `target` le `<video>` lui-même** : un
      écouteur sur `window` le reçoit par remontée, ce que la sonde vérifie. **Ne
      pas l'attacher au `<video>`** — un `video.focus()` perdu le rendrait muet.
- [ ] **Step 3 : le message de refus réutilise `messageDeRefus`** (déjà exporté
      par `presse-papier.ts`) — **ne pas en écrire un second**.
- [ ] **Step 4 : voir vert**, compte annoncé.

### Task 20 : `client/src/main.ts` — le câblage, et lui seul

**Objet :** brancher l'émission, et gater sur `Capabilities.clipboard`.

**Files:**
- Modify: `client/src/main.ts`

**Dépend des tâches 18 et 19. Relevé : 414 lignes, marge 86.**

🔴 **Aucune logique ici.** P1 a livré `main.ts` avec zéro décision dans sa
branche `clipboard` (`:248`), et la revue transverse a **relevé la valeur de
ce choix**. Le tenir.

- [ ] **Step 1 : passer l'émetteur** à `attacherPressePapierAuDOM` — le canal de
      contrôle, comme le fait déjà `encodeResize`.
- [ ] **Step 2 : lire `clipboard` sur `capabilities`**, et **ne pas armer
      l'exception clavier** si l'agent ne l'annonce pas. ⚠️ **Sans ce gate,
      `PRESSE_PAPIER=0` produirait un `Ctrl+V` qui ne colle rien ET n'arrive pas
      à la VM** — la touche serait retenue par le client sans que personne ne
      l'injecte. **C'est le pire des deux mondes, et c'est ce que le gate évite.**
- [ ] **Step 3 : `Capabilities` arrive AVANT `Ready`**
      (`proto/src/control.rs`, doc de la variante). Le traitement doit être
      **indépendant de l'ordre**, comme le reste de ce fichier. Rouge = gater
      sur `Ready`.
- [ ] **Step 4 : `npm run typecheck` et `npx vitest run`.**

---

# Famille 6 — le banc, la recette, la clôture

### Task 21 : `PRESSE_PAPIER_GARDE` et `scripts/run-agent.sh` — TÂCHE DÉDIÉE

**Objet :** le bras désarmé de la rouge du critère ④, **et sa transmission**.
**Une tâche à elle seule**, parce que ce piège a été payé en D1 (`SUPERVISEUR`),
D2 (`MULTIFENETRE_REPRISE`), D6 (`BUDGET_BPS`) et D7 (`AUDIO`) : *un agent
démarre sans la variable et ne le signale pas*.

**Files:**
- Modify: `agent/src/presse_papier.rs`, `scripts/run-agent.sh`

**Dépend de la tâche 11. Relevé : `scripts/run-agent.sh` 145 lignes ;
`PRESSE_PAPIER` y est déjà transmise (`:37`), posée par P1.**

| | `PRESSE_PAPIER_GARDE=0` |
| --- | --- |
| Nature | **BANC, jamais une configuration livrée** — même statut que `PART_SONDAGE` |
| Convention | **`=0` DÉSARME ; une simple présence n'arme pas.** Le garde est armé par défaut |
| Effet | neutralise `Sondeur::apres_notre_ecriture` **EN ENTIER** — garde n°1 **et** armement du n°2 |
| Lue où | dans le **propriétaire** (le capteur), par `OnceLock`, comme `actif()` |
| Trace | **seulement si désarmé** : `warn!` — `garde anti-echo du presse-papier DESARME (PRESSE_PAPIER_GARDE=0) : bras de banc, jamais une configuration livree` |

🔴 **Pourquoi elle désarme les DEUX gardes, et non le seul n°1 :** D-P2-5.
Désarmer le n°1 seul rendrait **zéro message** aussi, et la rouge du critère ④
serait vacueuse **une seconde fois**.

- [ ] **Step 0 :** `git log --oneline -5 -- scripts/run-agent.sh` — ⚠️ **fichier
      sous périmètre concurrent.**
- [ ] **Step 1 : la variable et sa trace.**
- [ ] **Step 2 : la ligne dans `scripts/run-agent.sh`**, sur le patron exact de
      `:37`, **dans CETTE tâche**.
- [ ] **Step 3 : 🔴 le contrôle qui vaut est que la variable ATTEIGNE le
      processus, pas qu'elle soit écrite dans le script.** Le vérifier au
      lancement de la recette, par la présence de la trace `warn!` — pas par un
      `grep` sur le script. **C'est la leçon exacte du critère ④ de P1.**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| 🔴 `PRESSE_PAPIER_GARDE=0` ⟹ `apres_notre_ecriture` ne pose **ni** `reference` **ni** `dernier_emis` | rouge = ne désarmer que `reference`. **Le test doit vérifier les DEUX champs** — par l'observable : après désarmement, `observer` appelle bien la fermeture de lecture **et** rend `Some` |
| absence de la variable ⟹ les deux gardes armés | rouge = tester `is_ok()` : la variable armerait le désarmement |

### Task 22 : la recette sur la VM — CINQ critères, DEUX exécutions chacun

**Objet :** mesurer. **Aucune ligne de code de produit.**

**Files:**
- Create: `docs/superpowers/plans/journaux-presse-papier-p2/` — les journaux
  d'agent **bruts et leurs `-plat`**, les JSON de pilote, et l'instrument dans
  `journaux-presse-papier-p2/instrument/`
- Modify: aucun

⚠️ **PRÉALABLE EXTERNE, hors dépendance de tâche : la VM Windows est tenue par
un chantier concurrent au moment où ce plan est écrit.** Ne pas démarrer cette
tâche sans l'avoir vérifié.

⚠️ **Contraintes de montage héritées, NON renégociables** (P1 en a perdu une
exécution entière sur la première) :
- **l'agent doit s'enrôler** — sans `AGENT_VM`/`AGENT_SECRET` le signaling le
  refuse, et **le symptôme se lit exactement comme une panne du produit** ;
- **une seconde instance de plateforme sur le port 8090** (celle du 8080 refuse
  la version 2 du canal `/agent`) ;
- **le jeton utilisateur semé dans `localStorage` avant toute navigation**, et
  **hors dépôt** — ni secret ni mot de passe versé ;
- **navigateur pilote sur l'HÔTE**, jamais sur la VM ;
- **le copieur à demeure de P1** (`journaux-presse-papier-p1/instrument/copieur-pp.ps1`) :
  une tâche planifiée par copie ouvre une console **éligible à la capture**, ce
  qui détruit la mesure. **Le réemployer, ne pas le réécrire.**

- [ ] **Step 0 : l'environnement.** VM démarrée et `/media/vm` **réellement
      accessible** (`ls /media/vm/dev`, pas `mountpoint`) ; `.env` sourcé **avant**
      `build-agent.sh` ; `Get-Process agent` **vide** ;
      `MULTIFENETRE_VDD_PURGE=1` **dans un lancement séparé** ; et
      `grep -a 'sortie créée mais introuvable'` **avant de conclure à un plafond
      de fenêtres**.
- [ ] **Step 1 : 🔴 LE TÉMOIN DE MESURABILITÉ, joué AVANT tout critère.** Il
      diffère de celui de P1 parce que le sens est inverse : ici c'est **la VM**
      qui doit relire. Copier un nonce **dans le presse-papier de l'HÔTE** par
      le pilote, coller dans un Bloc-notes de la VM, et relire par
      `WM_GETTEXT` — **jamais par `Get-Clipboard` en WinRM**, qui tourne en
      session 0 et rend `-1` (sonde P0 de P1).
      - le nonce revient ⟹ le critère ② est mesurable ;
      - il ne revient pas ⟹ **écrire ce qui a échoué, et de quel côté**, avant
        de mesurer quoi que ce soit d'autre.
      🔴 **Ce témoin peut échouer, et c'est ce qui en fait un témoin.**
- [ ] **Step 2 : le point d'observation.** 🔴 **Les messages `clipboard`
      agent → client se comptent SUR LE CANAL DE CONTRÔLE, par un écouteur
      INDÉPENDANT du produit** — jamais sur les appels à `writeText`, parce que
      `PressePapierLocal` **dédoublonne lui aussi** et que le critère ④
      mesurerait alors le garde du client. C'est la leçon la plus réutilisable
      de la recette de P1 ; **la reprendre telle quelle.**
- [ ] **Step 3 : les cinq critères. DEUX exécutions chacun. Aucun taux n'est
      revendiqué.**

| # | Critère | Comment il est jugé | 🔴 Ce qui le rend ROUGE |
| --- | --- | --- | --- |
| ① | **Le `paste` parvient avec le focus sur le `<video>`** | ✅ **DÉJÀ MESURÉ hors VM, verdict FAVORABLE, deux exécutions** (§0). Sur la VM, il se rejoue **implicitement** par ② : un collage qui arrive prouve qu'un `paste` est né | la rouge est jouée et versée : régime `produit` ⟹ **0 `paste` sur 8 cellules**. **Ne pas la rejouer**, la citer |
| ② | **Coller dans le Bloc-notes de la VM depuis le presse-papier local FONCTIONNE, `clipboard-read` REFUSÉE** | le texte relu par `WM_GETTEXT` dans le Bloc-notes **est** celui copié sur l'hôte | 🔴 **accorder la permission `clipboard-read` et voir si cela change quelque chose : cela ne doit RIEN changer.** Si cela change quelque chose, **le chemin employé n'est pas celui qu'on croit** — et il faudrait chercher un `readText` qui n'a rien à faire là |
| ③ | **Le contenu collé est le DERNIER copié, jamais le précédent** | copier T1, coller ; copier T2, coller ; relire deux fois | 🔴 **la rouge est la justification ENTIÈRE de D6** : envoyer la touche `V` sur le canal d'entrées **au lieu de** l'injection, et coller deux textes à la suite. **Si elle ne se déclenche pas, D6 est du coût pour rien et doit être rouverte.** ⚠️ **Elle exige un binaire de rouge distinct** — le prévoir, ou déclarer le critère non joué |
| ④ | **Aucun aller-retour : k collages ⟹ ZÉRO message `clipboard` en retour** | l'écouteur du Step 2, sur k = 3 collages | 🔴 **`PRESSE_PAPIER_GARDE=0` ⟹ EXACTEMENT k messages**, portant chacun le texte collé. ⚠️ **La rouge de la spec — « le compte doit croître sans borne » — est VACUEUSE et remplacée** : voir D-P2-5, qui écrit pourquoi le compte s'arrête à un et pourquoi aucune oscillation n'est possible |
| ⑤ | **Un raccourci qui n'est PAS un collage garde son `preventDefault`** | frapper `Ctrl+W`, `Ctrl+T`, `Ctrl+N` dans la fenêtre de session | 🔴 **La fenêtre ne doit ni se fermer ni ouvrir d'onglet.** ⚠️ **CE CRITÈRE N'EST PAS MESURABLE EN `--headless`** — le §0.4 l'établit : pas d'onglets, et `Input.dispatchKeyEvent` n'emprunte pas le chemin des raccourcis du navigateur. **Deux issues, à trancher et à ÉCRIRE** : soit un Chrome **avec interface** est disponible sur l'hôte, soit le critère est **déclaré NON MESURÉ**, avec sa couverture de repli nommée — les tests purs de la tâche 6, **qui ne prouvent que le prédicat, jamais le comportement du navigateur** |

- [ ] **Step 4 : appel BLOQUANT au premier plan**, délai explicite couvrant la
      durée complète. Une commande backgroundée par le harnais **ne survit pas à
      la fin du tour**, et le symptôme est un journal **tronqué** copié depuis une
      VM où l'agent tourne encore.
- [ ] **Step 5 : copier `agent.log` APRÈS la fin réelle**, pas à la fin du
      pilote.
- [ ] **Step 6 : `grep -a` sur tout journal de pilote**, et relever l'encodage
      **par `file`** avant d'écrire la note de lecture.
- [ ] **Step 7 : le `grep` de la tâche 17.** `Capabilities.clipboard` atteint-il
      le client ? Le relever **dans la page**, pas dans le code.
- [ ] **Step 8 : verser TOUS les journaux dans git**, bruts **et** `-plat`, et
      l'instrument dans son **état final**. 🔴 **Jamais dans un rapport
      gitignoré** : l'espace de travail de D9 a disparu avec six constats de
      revue, définitivement perdus.

### Task 23 : la revue transverse de fin de branche — OBLIGATOIRE

**Objet :** chercher les défauts qui **franchissent une frontière de tâche**, et
que les revues par tâche ne peuvent structurellement pas voir.

**Barème du dépôt, à titre d'ordre de grandeur, JAMAIS de quota :** D7 **5**,
D8 **3**, D9 **6**, D10 **douze**, D11 **sept**, S1 **cinq**, S2 **douze**,
S3 **treize**, S4 **vingt-sept**, G1 **huit**, E **neuf**, plateforme P4 **huit**,
P5 **neuf**, **presse-papier P1 dix-sept**.

**Files:**
- Modify: ceux que la revue désigne, et eux seuls

- [ ] **Step 1 : la cible propre — les affirmations de code devenues fausses
      dans leur propre branche.** C'est le mode de défaillance dominant de ce
      dépôt : P1 en a trouvé **neuf** d'un coup, dont une (`presse-papier.ts`
      disant « c'est `main.ts` qui appelle `writeText` ») rendue fausse par une
      tâche **de la même branche**. **Balayer par le SENS, pas par la formule.**
- [ ] **Step 2 : les questions à poser explicitement**, chacune ayant déjà coûté
      une ronde ailleurs :
      1. **Un déictique de distance a-t-il vieilli ?** (« trois lignes plus
         haut », « ci-dessous ») — P1 en a trouvé un.
      2. **Un COMPTE écrit en toutes lettres est-il encore juste ?** (« les
         quatre détachements », « ces trois tests ») — P1 en a trouvé **trois**,
         dont un **faux aux deux endroits où il était écrit**.
      3. **Une variante d'enum promet-elle ce que sa voisine, ajoutée depuis,
         contredit ?** — c'est le cas `AudioMort`/`AudioVivant` de D10.
      4. **Un `fichier:ligne` cité pointe-t-il encore la bonne chose ?**
         **Ce plan en a corrigé TROIS dans la spec** (E2, E3, E7) : la même
         dérive frappera ce plan.
      5. **Le commentaire du bras catch-all de `pont_media.rs` (`:75`) compte
         les précédents** — P2 n'en ajoute aucun dans ce sens, **le vérifier**
         plutôt que le supposer.
      6. **Une tâche a-t-elle écrit « X n'a pas encore tourné » qu'une tâche
         ultérieure a fait tourner ?** C'est le défaut le plus pur trouvé par
         D10.
- [ ] **Step 3 : chercher les CONTRÔLES INCAPABLES D'ÉCHOUER, dans le code ET
      DANS CE PLAN.** 🔴 **Candidats désignés d'avance ici, parce qu'ils sont
      les plus fragiles :**
      - **le critère ⑤ de la tâche 22** — s'il a été « joué » en `--headless`,
        il n'a rien mesuré ;
      - **le premier test du garde n°1 (tâche 4)** — s'il vérifie `None` sans la
        fermeture qui panique, il est satisfait par le garde n°2 ;
      - **le test de `presse-papier-dom` sur le garde n°3** — s'il n'a pas son
        jumeau « un texte DIFFÉRENT passe », un `aEmettre` qui rend toujours
        `undefined` le satisfait ;
      - **le troisième test de la tâche 8** — il ne peut échouer que si
        `parseAgentControl` valide les champs ; s'il ne valide que `v` et
        `type`, le test est décoratif et **le rapport doit le dire** ;
      - **le critère ④ de la tâche 22** — vérifier que le bras désarmé a
        RÉELLEMENT produit ses k messages ; **zéro des deux côtés est un
        instrument mort, pas un succès.**
- [ ] **Step 4 : chercher les PIÈCES FABRIQUÉES.** D10 en a trouvé **deux**, et
      les deux rapportaient un fait **vrai** avec une preuve **inventée**.
      **Relancer au moins un `wc -l`, un `cargo test` et un `npx vitest run` de
      la branche, et les comparer aux nombres publiés.** ⚠️ **Le mécanisme est
      nommé** : réutiliser la sortie d'une commande antérieure pour répondre à
      la question d'une AUTRE, sans la relancer.
- [ ] **Step 5 : écrire les constats dans le document de RÉSULTATS**, jamais
      dans un rapport gitignoré.

### Task 24 : `CLAUDE.md` gagne sa section, et ses tailles sont RELEVÉES

**Objet :** l'index de connaissances. 🔴 **Toutes les tailles sont relevées par
la commande, APRÈS les dernières éditions de la branche, revue transverse
comprise** — une table relevée en début de ronde serait fausse à la fin de la
même ronde, erreur que D8 a commise en croyant bien faire.

**Files:**
- Modify: `CLAUDE.md`

⚠️ **`CLAUDE.md` est sous périmètre concurrent.** `git log --oneline -5 --
CLAUDE.md` **juste avant**, et relire le diff.

- [ ] **Step 1 : relancer la commande des 500 lignes** (§2.2), et **corriger le
      tableau de dette dans le même mouvement** s'il a dérivé.
- [ ] **Step 2 : 🔴 « corrigé à sa place » est une affirmation de COMPLÉTUDE, et
      elle se vérifie en énumérant les places AVANT d'écrire** :
      `grep -n '<le nombre>' CLAUDE.md`. **Puis RELIRE place par place APRÈS
      l'édition** — une substitution qui ne dit pas combien d'occurrences elle a
      touchées est une affirmation de complétude non vérifiée. ⚠️ **Ne barrer que
      ce qui est FAUX** : un énoncé **daté** reste vrai comme histoire.
      ⚠️ **Un numéro de ligne dans `CLAUDE.md` est faux dès qu'on écrit
      au-dessus, et on écrit toujours au-dessus** : **nommer l'entrée, jamais le
      numéro seul.**
- [ ] **Step 3 : la variable neuve au tableau du banc**, `PRESSE_PAPIER_GARDE=0`,
      **à côté de `PRESSE_PAPIER` et `PRESSE_PAPIER_SONDE`**, avec les **trois**
      conventions écrites côte à côte — deux `=0` désarme, une « présence =
      armée ».
- [ ] **Step 4 : la section du sous-bloc**, titrée **sans ambiguïté** (« presse-papier
      P2 », jamais « P2 » seul — le dépôt a deux P2).
- [ ] **Step 5 : elle porte, dans cet ordre** : **le verdict du préalable
      éliminatoire avec son nombre d'exécutions et sa rouge** ; les cinq
      critères avec leur verdict et leur nombre d'exécutions ; **ce que P2
      N'ÉTABLIT PAS** ; les pièges neufs ; et les legs.
- [ ] **Step 6 : 🔴 annoter la SPEC**, pas seulement `CLAUDE.md`. **Corriger une
      affirmation exige de la CHERCHER, pas de la corriger là où on nous l'a
      montrée** : « le préalable n'est pas mesuré » vit à **QUATRE** endroits,
      nommés ici plutôt que comptés — le §3.3 (« Ce que ces relevés
      n'établissent PAS », dernier tiret sur le `<video>`), le §6.2 (critère ①
      de P2), le §8 (« Ce que ce chantier n'établira PAS ») et le §10 (risque
      **R1**). *(Aux lignes `:319`, `:846`, `:1017` et `:1067` au 20 août 2026 —
      **revérifier par `grep`, un numéro de ligne dérive dès qu'on écrit
      au-dessus.**)* Et le critère ④ du §6.2 porte une rouge **vacueuse** (E1,
      D-P2-5) : il se corrige au même endroit.

### Task 25 : clore le document de résultats

**Objet :** le document permanent. 🔴 **La preuve d'une affirmation du dépôt ne
doit JAMAIS vivre dans un rapport gitignoré** — l'espace de travail de D9 a
disparu avec six constats, définitivement perdus.

**Files:**
- Create: `docs/superpowers/plans/2026-08-20-presse-papier-p2-resultats.md`

- [ ] **Step 1 : la note de lecture des journaux**, relevée **par `file`**, pas
      supposée. Les journaux du §0 sont **UTF-8 sans ANSI ni octet NUL** ; ceux
      de la recette (tâche 22) le seront **probablement** autrement — le relever.
- [ ] **Step 2 : chaque énoncé porte son NOMBRE D'EXÉCUTIONS.** Aucun taux.
- [ ] **Step 3 : « Ce que P2 n'établit PAS »**, au minimum :
      - aucun taux, aucun navigateur autre que Chromium, **aucun Chromium avec
        interface** sauf si le critère ⑤ en a exigé un ;
      - **rien au-delà d'UNE fenêtre** — P3 est le sous-bloc des N ;
      - **rien de deux collages concurrents** : `SendInput` reste global à la
        session Windows, et la portée mesurée du dépôt est « une frappe par
        fenêtre, sonde séquentielle » ;
      - **le chemin d'échec de `commander` (borne 12 s) n'a jamais couru** ;
      - **`PRESSE_PAPIER_MAX` et `PERIODE_PRESSE_PAPIER` ne sont pas calibrées**,
        et rejoignent la liste que ce dépôt tient depuis `BPP_MIN` ;
      - **aucun jugement d'usage** n'a été porté sur la latence d'un collage.
- [ ] **Step 4 : les legs**, numérotés, chacun avec son point de chute nommé.
- [ ] **Step 5 : `git status --porcelain` doit être vide** à la fin, et les
      journaux **versés**.

---

## 8. Ce que ce plan NE prescrit PAS, et pourquoi

- **Aucune file de collages.** L'écrasement du dernier est assumé (D-P2-3), avec
  son coût écrit. Une file bornée serait le remède si un client mal conduit
  perdait des collages ; **rien ne l'a mesuré**.
- **Aucun `readText()`, nulle part.** C'est le geste de l'ancien produit
  (`web/index.js`), il exige une permission, et le §0 établit qu'il est inutile.
  **Le nouveau produit ne demande aucune permission de presse-papier**, et c'est
  le meilleur résultat de ce chantier.
- **Aucun presse-papier par fenêtre** (spec D3) : les applications Windows le
  partagent déjà.
- **Aucune annonce de l'état courant à l'attache** : c'est le legs n°3 de P1, et
  il appartient à P3 — sans conséquence à une fenêtre.
- **Aucun propriétaire mono-fenêtre** (D-P2-9). P2 le rend **visible** (`Err` au
  lieu d'`Ok`), il ne le livre pas.
- **Aucun changement de `CONTROL_VERSION`** (spec D7, et P1 l'a déjà tenu).
- **Aucun `TYPES_CLIENT`** (E4) : il serait sans objet.
- **Aucune mesure de latence de bout en bout**, qu'aucun sous-bloc de ce dépôt
  n'a jamais prise.

---

## 9. Ce que P2 lègue

1. ⛔ **Le propriétaire MONO-FENÊTRE n'existe toujours pas** (legs n°1 de P1).
   P2 le rend **bruyant** au lieu de silencieux, il ne le comble pas. Point de
   chute : `agent/src/demarrage.rs`, **dont la tâche 3 aura rendu la marge**.
2. ⛔ **Le collage écrase le précédent** (D-P2-3) : deux collages dans un même
   tour de boucle se réduisent au second, **sans trace**. Remède nommé : une
   file bornée dans `evenements.rs`.
3. ⛔ **Le chemin d'échec de `commander` (borne 12 s) n'a jamais couru**, et P2
   l'emprunte désormais **depuis la boucle de transport**. Un capteur mort ferait
   attendre la boucle jusqu'à cette borne. **Déclaré, non mesuré.**
4. ⛔ **Deux collages concurrents depuis deux fenêtres sont hors de ce qui est
   établi** : `SendInput` est global à la session Windows, et la seule portée
   mesurée du dépôt est séquentielle. **P3 les rencontrera.**
5. ⛔ **Le legs n°4 de P1 s'aggrave** : le canal `Message` du registre reste non
   borné, et le presse-papier circule maintenant dans les **deux** sens à 64 KiB
   par geste.
6. ⛔ **`PRESSE_PAPIER_MAX` et `PERIODE_PRESSE_PAPIER` ne sont toujours pas
   calibrées**, et `PRESSE_PAPIER_GARDE` n'est pas une constante mais un bras de
   banc.
7. ⛔ **Rien de Firefox ni de Safari**, et **rien d'un Chromium avec interface**
   sauf si le critère ⑤ en a exigé un — auquel cas **les mesures qui en sortent
   ne se comparent à aucune campagne antérieure**, comme la conception de D8
   l'écrit déjà pour `Xvfb`.
8. ⛔ **Le niveau 2 de mesure du sens VM → navigateur reste NON MESURABLE**
   (legs n°11 de P1, `Xvfb` + `xdotool` absents). ⚠️ **P2 n'en dépend pas** : son
   sens est l'inverse, et son témoin de mesurabilité (tâche 22, Step 1) relit
   **côté VM**, où `WM_GETTEXT` fonctionne.

---

## 10. Risques qui rendraient ce sous-bloc NON LIVRABLE

| # | Risque | Gravité | Ce qui le lève, ou le borne |
| --- | --- | --- | --- |
| RP2-1 | ~~`paste` ne parvient pas sur un `<video>` focalisé~~ | ~~éliminatoire~~ | ✅ **LEVÉ** — §0, verdict FAVORABLE, deux exécutions, avec sa rouge et son témoin |
| RP2-2 | 🔴 **L'élargissement de la condition de D6 rend le navigateur au clavier** | `Ctrl+W` fermerait la fenêtre de session | prédicat **pur et isolé** (tâche 6) avec **neuf** cas de table de vérité dont **six rouges** ; critère ⑤ à la recette. ⚠️ **Le critère ⑤ peut être NON MESURABLE en `--headless`** — c'est alors le seul risque de ce tableau dont la mesure manque |
| RP2-3 | 🔴 **La rouge du critère ④ ne se déclenche pas** | on croirait les gardes utiles sans preuve | **traité d'avance** : D-P2-5 explique pourquoi la rouge de la spec est vacueuse et la remplace par une rouge atteignable (k messages contre zéro) |
| RP2-4 | 🔴 **La rouge du critère ③ n'est pas jouée** faute de binaire distinct | **D6 devient du coût pour rien**, et rien ne le dirait | la tâche 22 l'exige explicitement, ou déclare le critère **non joué**. ⚠️ **Ne pas confondre « le collage marche » avec « l'ordre est garanti »** : le chemin naïf marche aussi, la plupart du temps |
| RP2-5 | ⚠️ **La VM est tenue par un chantier concurrent** | la recette entière | préalable **externe** nommé en tête de la tâche 22 |
| RP2-6 | ⚠️ **`demarrage.rs` (491, marge 9) ne se laisse pas extraire proprement** | dette neuve sur un fichier déjà signalé par P1 | tâche 3, **avant** la tâche 17. Son Step 3 impose d'**écrire le chiffre obtenu** et de s'arrêter s'il ne suffit pas |
| RP2-7 | ⚠️ **`OpenClipboard` échoue parce qu'une autre application le tient** | un collage perdu | **cas NORMAL sous Windows** (spec R7) : `Err`, journalisé, **jamais** de boucle d'attente. L'injection n'a alors pas lieu |
| RP2-8 | ⚠️ **Le garde n°1 est armé d'un cran de retard** | un aller-retour parasite par collage | tâche 10 Step 2 : relire le compteur **APRÈS `CloseClipboard`**. ⚠️ **Silencieux si raté** — aucune panne, seulement du trafic |
| RP2-9 | ⚠️ **Le bras miroir de `commandes.rs:220-224` est oublié** | ne compile pas | gardé par le compilateur, **et nommé** en tâche 12 pour ne pas être découvert |
| RP2-10 | ⚠️ **`PRESSE_PAPIER_GARDE` n'est pas transmise par `run-agent.sh`** | l'agent démarre sans elle **et ne le dit pas** | payé en D1, D2, D6, D7 ; **tâche DÉDIÉE** (21), et le contrôle porte sur la trace, pas sur le script |
| RP2-11 | ⚠️ **Un `Ctrl`↑ manquant laisse un modificateur enfoncé dans la VM** | **toute frappe suivante devient un raccourci** | tâche 16, premier test : **exactement quatre** touches, dans l'ordre |
| RP2-12 | ⚠️ **`Capabilities.clipboard` ment** si le capteur cesse un jour de décider par la variable | le client armerait un `Ctrl+V` qui ne colle rien | D-P2-8 : la **condition** est écrite dans le commentaire du champ, pas seulement le fait |
