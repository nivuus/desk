# Sous-projet ① « Divers » — presse-papier, sous-bloc **P1** : la VM copie, le navigateur colle — résultats

> ⚠️ **Ce P1 n'est PAS celui de la plateforme.** Le sous-projet ⑤ a aussi un
> sous-bloc « P1 », clos et sans rapport. Celui-ci est le premier sous-bloc du
> chantier **presse-papier**.

Plan : `docs/superpowers/plans/2026-08-19-presse-papier-p1.md`.
Conception : `docs/superpowers/specs/2026-08-19-presse-papier-design.md`.
Journaux et instrument : `docs/superpowers/plans/journaux-presse-papier-p1/`.

**Aucun taux n'est revendiqué nulle part.** Chaque énoncé de ce document porte
son nombre d'exécutions.

---

## 0. Note de lecture des journaux — TROIS familles, relevées par `file`

**Relevée par la commande, jamais supposée** (`file`, `tr -cd '\0' | wc -c`, et
un `grep` d'essai) :

| Famille | État relevé | Ce qu'il faut faire |
| --- | --- | --- |
| `agent-*.log` (bruts) | UTF-8, **CRLF**, **séquences ANSI PRÉSENTES** (191 sur `agent-arme-1.log`), **0 octet NUL** | `sed 's/\x1b\[[0-9;]*m//g'` — ou lire le `-plat` jumeau, versé pour chacun |
| `agent-*-plat.log` | UTF-8, CRLF, **ANSI retirées**, 0 octet NUL | rien |
| `critere-*.log` (journaux de **PILOTE**) | classés **« data »** par `file` — **33 à 36 octets NUL** par fichier | 🔴 **`grep -a` OBLIGATOIRE** |
| `pp-*.json`, `temoin-mesurabilite.*` | JSON / UTF-8 propre | rien |

🔴 **La troisième ligne n'est pas une précaution de principe : elle est
mesurée.** Sur `critere-arme-1.log` :

```
grep  -c 'copie' critere-arme-1.log   →  (sortie VIDE)
grep -ac 'copie' critere-arme-1.log   →  17
```

C'est le piège de D10 rejoué à l'identique : **`grep` sans `-a` rend une sortie
VIDE, pas zéro**, et une sortie vide est indiscernable d'un compte nul pour qui
lit trop vite. ⚠️ **La cause des octets NUL n'est PAS établie** — ils arrivent
avec les comptes rendus que le copieur écrit sur le partage CIFS, et la lecture
concurrente d'un fichier que Windows réécrit est l'hypothèse la plus proche ;
elle n'a pas été éprouvée.

---

## 1. La porte d'entrée : la sonde P0 (tâche 2), rappel

Verdict **V0 — FAVORABLE**, **deux exécutions** (`p0-sonde-1.log`,
`p0-sonde-2.log`) : compteur **stable au repos**, **bouge à chaque copie**
(5 mouvements / 5 lectures / 0 échec d'ouverture), **bouge sur une réécriture
identique** (`q2="bouge"`) et **bouge sur notre propre écriture**
(`q3="bouge"`).

Deux conséquences que la suite emploie :

1. **le critère ② est MESURABLE** — c'est la condition que la spec §6 posait
   explicitement (« si aucun message ne part même sans le garde, le critère est
   NON MESURABLE et doit le dire ») ;
2. **le garde n°2 a quelque chose à absorber**, et ce n'est pas une boucle
   d'écho : P1 n'écrit jamais le presse-papier. C'est le faux positif du
   compteur.

⚠️ **L'instrument de cette sonde a rendu un FAUX verdict éliminatoire à sa
première exécution** — trois zéros lus sur une VM saine, parce que rien n'avait
été copié depuis le démarrage de la station de fenêtres. Journal conservé :
`p0-sonde-0-instrument-defectueux.log`. **Un verdict négatif exige que la chose
mesurée soit ABSENTE, pas seulement nulle.**

---

## 2. Étape 0 — le témoin de mesurabilité (E11) : **NIVEAU 2 NON MESURABLE**

Joué **AVANT tout critère**, **une exécution**
(`temoin-mesurabilite.log`, `temoin-mesurabilite.json`,
instrument `instrument/temoin-mesurabilite-pp.mjs`). Il ne touche ni la VM, ni
l'agent, ni le produit.

| Ce qui est relevé | Résultat |
| --- | --- |
| `navigator.clipboard.writeText(nonce)` **résout** | **oui** (`{"resolue":true}`) |
| la page se relit elle-même (`readText`) | **oui** — elle rend le nonce exact |
| l'**HÔTE** relit le nonce | **NON** : `xclip` **absent**, `wl-paste` **absent**, `xsel` présent et refusé — `xsel: Can't open display: (null)` |

**Verdict : le niveau 2 est NON MESURABLE sur ce montage.** Aucun serveur X
n'est joignable depuis ce compte (`DISPLAY` vide, `/tmp/.X11-unix` inexistant).

🔴 **Ce n'est PAS un échec du produit, c'est une mesure non prise**, et le
critère ① tombe donc au **niveau 1**, avec sa réserve écrite en toutes lettres
plus bas. **Ce témoin POUVAIT échouer, et c'est ce qui en fait un témoin** — sa
première branche (`writeText` résout) et sa deuxième (`readText` rend le nonce)
ont, elles, réussi : l'échec est localisé, et sa cause est nommée.

⚠️ **Le relevé distingue TROIS choses et ne les confond pas.** Que la page se
relise elle-même prouve que le presse-papier **du navigateur** porte le texte ;
un presse-papier interne au processus Chromium suffirait à le satisfaire. **Ce
n'est pas le niveau 2**, et l'appeler ainsi serait exactement l'abus que E11
interdit.

---

## 3. La recette : quatre critères, **deux exécutions par bras**

Binaire mesuré : rebâti le 20 août 2026 après
`cargo clean --release -p proto -p agent` — **10 088 448 octets**,
`agent.exe` du 20 août 10:04 (le précédent valait 10 075 136). Fraîcheur
corroborée par la présence des marqueurs du sous-bloc dans le binaire :
`grep -ac 'presse-papier de la VM'` → **2**,
`grep -ac 'presse-papier DESARME'` → **1**.

**Quatre exécutions du produit** : `arme-1`, `arme-2`, `desarme-1`, `desarme-2`.
Une fenêtre, une session, un enfant.

### 3.1 Le protocole, et ce que chaque geste sert

Les copies sont faites dans la **session 1** de la VM — `Get-Clipboard` par
WinRM (session 0) rend `-1`, mesuré à la sonde P0 —, **après** que la session
est établie (D-P1-4 : l'état lu au premier sondage fait référence et n'est
jamais annoncé ; copier avant mesurerait zéro sur un produit correct).

| Geste | Ce qu'il sert |
| --- | --- |
| **C1** copie neuve `alpha-<nonce>` | critère ① |
| **C2** **la même**, à l'identique | critère ② |
| **C3** copie différente `beta-<nonce>` | 🔴 **ce qui rend ② DISCRIMINANT** : sans elle, « aucun message » serait aussi le relevé d'un mécanisme mort |
| **C4** 100 KiB | critère ③ |
| **C5** **vraie copie Bloc-notes** (`SendKeys` Ctrl+A, Ctrl+C) | critère ① avec le geste que la spec nomme, pas un `Set-Clipboard` de plus |

### 3.2 Le point d'observation, et pourquoi celui-là

🔴 **Les messages sont comptés SUR LE CANAL DE CONTRÔLE, par un écouteur
indépendant du produit, jamais sur les appels à `writeText`.**

La raison est que le critère ② serait sinon **incapable d'échouer** :
`PressePapierLocal.aEcrire` dédoublonne LUI AUSSI (`enAttente === ecrit` rend
`undefined`), donc deux messages identiques ne produiraient qu'une écriture
même si le garde de l'AGENT était retiré. Compter les écritures mesurerait le
garde du client, pas celui qu'on veut juger. C'est le patron que ce dépôt paie
depuis D6, et il a été payé une fois de plus **à l'intérieur de ce sous-bloc**
(tâche 13 : un test du canal rompu passait avec un distributeur vide, parce
qu'un appel intercalé détectait la même rupture).

Les écritures **sont** relevées, mais pour le critère ① au niveau 1 : le texte
réellement passé à `writeText`, et la **résolution** de sa promesse.

### 3.3 Les relevés, phase par phase — **`arme-1` et `arme-2`, identiques**

Compte cumulé lu dans la page à chaque phase (`pp-arme-1.json`,
`pp-arme-2.json`) :

| Phase | messages | écritures | `#status.textContent` | `dataset.hidden` | `readText` de la page |
| --- | --- | --- | --- | --- | --- |
| avant toute copie | 0 | 0 | `780×492, …` | `true` | `""` |
| après **C1** | **1** | **1** | — | `true` | **`alpha-<nonce>`** |
| après **C2** (identique) | **1** | **1** | — | `true` | `alpha-<nonce>` |
| après **C3** (différente) | **2** | **2** | — | `true` | **`beta-<nonce>`** |
| après **C4** (100 KiB) | **3** | **2** | **« copie trop volumineuse (100 Kio) — elle n'a pas été recopiée ici, réduisez la sélection »** | **`false`** | **`beta-<nonce>`** (inchangé) |
| après **C5** (Bloc-notes) | **4** | **3** | — | `true` | **`gamma-<nonce>`** |

Détail des quatre messages reçus, aux **deux** exécutions :

```
{texte_present:true,  longueur:19, bytes:19}      alpha-<nonce>
{texte_present:true,  longueur:18, bytes:18}      beta-<nonce>
{texte_present:false, longueur:0,  bytes:102400}  (refus)
{texte_present:true,  longueur:19, bytes:19}      gamma-<nonce>
```

Et côté **agent**, la trace `presse-papier de la VM` — **4 annonces pour
5 copies**, aux deux exécutions, dont **1** portant `refus=true` :

```
08:35:06.657 octets=19     refus=false     (C1)
                                            ← C2 : AUCUNE annonce
08:35:19.933 octets=18     refus=false     (C3)
08:35:26.198 octets=102400 refus=true      (C4)
08:35:38.974 octets=19     refus=false     (C5)
```

**Les deux bouts concordent exactement** : 4 annonces côté agent, 4 messages
côté page, aux deux exécutions.

### 3.4 Les verdicts

| # | Critère | Verdict | Exéc. |
| --- | --- | --- | --- |
| ① | Copier rend collable | **TENU au NIVEAU 1**, avec sa réserve (§3.5) | **2** |
| ② | Un texte inchangé ne produit **aucun** message | **TENU**, et **MESURABLE** (P0, `q2="bouge"`) | **2** |
| ③ | Au-dessus de la borne : **refusé et dit**, jamais tronqué | **TENU** | **2** |
| ④ | `PRESSE_PAPIER=0` désarme | **TENU** | **2 + 2** |

**④ en détail, et le contrôle qui vaut est le ZÉRO** : bras désarmé,
`desarme-1` et `desarme-2`, **0 message** dans la page et **0 annonce** côté
agent, sur les **cinq mêmes copies** — dont la vraie copie Bloc-notes. La trace
`presse-papier DESARME (PRESSE_PAPIER=0) : le contenu copie dans la VM n'est
plus pousse au navigateur` est présente **1 fois** dans chacune. ⚠️ **La trace
prouve que la variable a atteint le processus, elle ne prouve pas que le
mécanisme est coupé** : c'est le bras ARMÉ, avec ses 4 messages, qui rend le
zéro discriminant.

**Zéro `ERROR` aux quatre exécutions.** **Zéro** `sortie créée mais introuvable
dans la topologie DXGI` — le blocage par pollution du registre qui avait plafonné
D9 à trois fenêtres ne mord pas ici (contrôle exigé par l'étape 2 de la tâche 16).

### 3.5 🔴 La réserve du critère ①, écrite en toutes lettres

Ce qui est **établi** : le texte réellement passé à
`navigator.clipboard.writeText` est **exactement** celui copié dans la VM ; sa
promesse a **résolu** ; et le presse-papier **du navigateur** le porte, relu par
`readText` dans la page même.

Ce qui **n'est PAS établi** : qu'un humain puisse le coller dans une application
locale. **Personne n'a jamais vérifié cela**, ni dans ce sous-bloc ni ailleurs.
Le témoin de l'étape 0 dit pourquoi : il n'y a pas de serveur X sur ce montage.
La voie qui le permettrait est nommée — `Xvfb` + `xdotool`, dont le consentement
d'installation a été **donné en D8 et jamais suivi d'effet** —, et ⚠️ **les
mesures qui en sortiraient ne se compareraient à aucune campagne antérieure**.

---

## 4. 🔴 Le câblage du tour de roue est MESURÉ BRANCHÉ, et il ne l'est que par ici

**Aucun test d'hôte ne couvre ce câblage** : le fil dort
`PERIODE_REARBITRAGE`, et `Sondeur::lire_la_plateforme` rend `None` hors
Windows — le retirer laisserait les trois tests de
`capteur/sommeil/presse_papier.rs` verts. **Cette recette, et elle seule, établit
que le mécanisme est réellement branché.**

L'observation est **directe**, pas une lecture de journal isolée :

1. la trace `presse-papier de la VM octets=… refus=…` ne sort que de
   `agent/src/capteur/sommeil/presse_papier.rs::distribuer` ;
2. `distribuer` n'a qu'un appelant : `registre.rs::demarrer_le_tour_de_roue`,
   par `sondeur.tour()` — vérifié par
   `grep -rn 'presse_papier::distribuer' agent/src/` ;
3. et **le navigateur reçoit le message correspondant**, compté sur le canal de
   contrôle par un écouteur qui n'est pas le produit, avec le **même contenu**
   et la **même taille**.

C'est la chaîne entière, de `GetClipboardSequenceNumber` à
`navigator.clipboard.writeText`, exercée quatre fois par exécution.

⚠️ **Ce que cela n'établit pas** : ni la période réelle du sondage, ni qu'il
lise **hors du verrou** (D-P1-3 / E3). La lecture hors verrou est une propriété
du code, lisible et commentée ; **aucune mesure de ce sous-bloc ne la
démontre**, et aucune n'a exercé la contention qu'elle existe pour éviter.

---

## 5. Ce que la recette a trouvé — **deux défauts d'INSTRUMENT**, aucun du produit

Aucun défaut de produit n'a été trouvé par cette recette. Les deux défauts
ci-dessous sont ceux de l'instrument, et ils sont versés parce qu'ils sont
réutilisables.

### 5.1 🔴 Une tâche planifiée par copie détruisait la mesure

La première version copiait par un `vm-it.sh` à chaque geste. Relevé :
**une page navigateur de plus s'ouvrait 0,5 s après CHAQUE appel** — cinq
appels, cinq pages —, la fenêtre de session perdait le focus, sa page était
renavigée, la session tombait, et le superviseur relançait un enfant (`w-1` →
`w-3` → `w-8` sur une seule exécution). Le compte de messages lu dans la page
valait **0** alors que l'agent avait bel et bien annoncé
(`presse-papier de la VM octets=19 refus=false`, journal de la tentative).

**Cause : la console PowerShell d'une tâche planifiée est ÉLIGIBLE à la
capture**, même en `-WindowStyle Hidden` — c'est le piège que D11 avait déjà
écrit, rencontré ici sous une forme neuve (pendant la mesure, et non au
montage). **L'instrument détruisait ce qu'il mesurait**, comme la capture
d'écran CDP de D1 et la trace par paquet du chantier TURN.

**Remède** : un **copieur à demeure** (`instrument/copieur-pp.ps1`), un seul
processus né **avant** le superviseur, piloté par fichiers sur le partage, dont
l'attente porte sur le **FAIT** — le numéro d'ordre relu — et jamais sur une
durée. Plus rien ne s'ouvre pendant la mesure.

⚠️ **La vraie copie Bloc-notes (C5) ouvre, elle, une fenêtre éligible, et c'est
inhérent au geste** : elle crée une session de plus. Elle vient donc **en
dernier**, après toutes les autres mesures. Les journaux portent **2** lignes
`fenêtre attachée au capteur` par exécution : la mire, puis le Bloc-notes.

### 5.2 Le pilote ne sortait pas

Le pilote écrivait son relevé, copiait son journal — puis **ne sortait
jamais** : le `WebSocket` CDP tient la boucle d'événements après la mort de
Chrome. Le harnais l'a basculé en arrière-plan au bout de dix minutes alors que
tout était fini depuis deux, et le symptôme se lit comme une mesure
interminable. Sortie explicite ajoutée.

### 5.3 Deux corrections de méthode, l'une et l'autre payées à l'exécution

- Le repli d'ouverture directe de la page d'application cherchait `session=`
  dans un journal **BRUT**, où `tracing` intercale des séquences ANSI entre le
  nom du champ et sa valeur : **le `grep` de la recette d'entrée de D8, rejoué**.
  Mis à plat avant de chercher.
- **L'auto-attache CDP ne suffit pas** : une exécution entière n'a vu **aucune**
  cible d'application — deux cibles attachées en tout — alors que le viewport
  était bien arrivé (`enfant lancé … largeur=780 hauteur=492`). La fenêtre
  s'était donc ouverte **puis refermée**. ⚠️ **La cause n'est pas établie** ; le
  balayage explicite de `Target.getTargets` et l'ouverture directe de la page
  l'encaissent, ils ne l'expliquent pas.

---

## 6. Le montage, et ce qu'il a fallu poser pour qu'il existe

⚠️ **Depuis le sous-bloc P3 de la PLATEFORME, l'agent doit s'enrôler.** Sans
`AGENT_VM`/`AGENT_SECRET`, le signaling le refuse
(`session de contrôle REFUSÉE … motif=jeton-absent`), aucune session ne
s'établit, et **le symptôme — « aucune page d'application attachée » — se lit
exactement comme une panne du produit**. Une exécution y a été perdue.

Trois gestes de montage, **hors du dépôt** (aucun secret n'est versé) :

1. un compte de recette et une VM enrôlée, attribuée à ce compte ;
2. le jeton obtenu de la plateforme par `/auth/connexion`, jamais forgé, semé
   dans `localStorage` **avant** toute navigation ;
3. 🔴 **une SECONDE instance de la plateforme, sur le port 8090.** Celle du port
   8080 appartient au chantier concurrent et tourne depuis avant le commit
   `e4671e6` : elle refuse la version 2 du canal `/agent`
   (`version_emise=2 version_recue=1`). La redémarrer aurait invalidé les jetons
   de son propriétaire. **Ce n'est pas une propriété du produit, c'est une
   contrainte de cohabitation**, et elle est déclarée.

Conséquence de (3), déclarée comme geste d'instrument : `shell-page.ts` ouvre
`/?session=<id>` **sans** paramètre `signaling`, et `main.ts` retombe alors sur
`ws://<hôte>:8080` en dur. L'amorce du pilote enveloppe donc `window.open` dans
la page-shell pour y ajouter le paramètre. **Cela ne change rien au produit** —
`main.ts:30-35` documente lui-même ce paramètre comme prévu « pour faciliter les
essais ».

⚠️ **Une première version de ce détour renavigait la cible depuis CDP** sur
`Target.attachedToTarget`. Elle ne pouvait pas marcher : une cible ouverte par
`window.open` s'attache avec une **URL VIDE**, l'URL n'arrivant qu'au
`targetInfoChanged` suivant — le test d'URL était donc toujours faux, **en
silence**. Un contrôle de plus qui ne pouvait pas réussir.

---

## 7. RP11 — la confidentialité, vérifiée par la commande

`D-P1-7` interdit qu'un texte de presse-papier atterrisse dans un journal versé.
**Relevé, pas supposé** : le texte copié apparaît dans **0** journal d'agent
(`grep -al 'alpha-arme\|beta-arme\|gamma-arme\|alpha-desarme' agent-*.log` →
aucun fichier), et les 100 KiB de remplissage n'apparaissent nulle part
(**0** fichier). Les journaux d'agent ne portent que `octets` et `refus`.

Les nonces figurent en revanche dans les journaux de **pilote** et les JSON :
ils sont **choisis par la recette**, et les verser est sans conséquence.

⚠️ **Le secret d'enrôlement, le mot de passe du compte de recette et le JWT
n'apparaissent dans AUCUNE pièce versée** — vérifié par recherche littérale sur
tout le répertoire : 0, 0 et 0.

---

## 8. Ce que P1 n'établit PAS

- **Aucun taux.** Deux exécutions par bras au mieux ; une seule pour le témoin
  de mesurabilité.
- 🔴 **Le NIVEAU 2 du critère ① n'a jamais été atteint** : *personne n'a vérifié
  qu'un humain peut coller* (§3.5). Le critère est tenu au niveau 1.
- **Rien d'un navigateur autre que Chromium**, rien d'un Chromium **avec
  interface**. **Aucune fenêtre de dialogue de permission n'a jamais été montrée
  à un humain** — et il se trouve que P1 n'en demande aucune, `readText`
  n'étant appelée nulle part.
- **Rien au-delà d'UNE fenêtre** — c'est P3. (Le Bloc-notes de C5 en crée une
  seconde, sans qu'aucune mesure ne porte sur elle.)
- **Le sens navigateur → VM n'est pas touché** — c'est P2, et son préalable
  éliminatoire (`paste` sur un `<video>` focalisé) **n'est toujours pas
  mesuré** : R2 a été relevé avec le focus sur un `body`. Si la réponse est non,
  P2 est bloqué et la conception se replie sur `readText()` avec permission,
  c'est-à-dire sur l'ancien produit.
- **`writeText` depuis une fenêtre NON focalisée n'est pas mesuré** : les quatre
  exécutions ont toutes `document.hasFocus() === true` à chaque phase. La règle
  du dépôt différé est livrée sur une contrainte **supposée**, que le critère ④
  de P3 peut **réfuter**.
- **`PRESSE_PAPIER_MAX` (64 KiB) et `PERIODE_PRESSE_PAPIER` (250 ms) ne sont pas
  calibrées** — elles rejoignent `BPP_MIN`, `FACTEUR_FOCUS`,
  `PART_DORMANTE_BPS`, `HYSTERESIS`, `REPIT_APRES_ECHEC`, `TAILLE_MAX_SORTIE`,
  `REPIT_REARMEMENT_AUDIO`, `REARMEMENTS_MAX`. **Aucun jugement d'usage n'a été
  porté sur aucune d'elles.**
- **Le propriétaire MONO-FENÊTRE n'existe pas** (E6, leg n°1) : un agent sans
  `SUPERVISEUR` ni `CAPTEUR` n'a **aucun** presse-papier. Ce n'est pas une
  régression — il n'en avait pas non plus.
- **La lecture HORS VERROU n'est pas démontrée par une mesure** (§4) : c'est une
  propriété du code, et la contention qu'elle évite n'a jamais été exercée.
- **Le refus d'ouverture du presse-papier n'a jamais eu lieu** : `echecs_open=0`
  à la sonde P0, et aucune trace d'échec pendant la recette. **Le chemin
  D-P1-5** — une lecture échouée n'avance pas la référence — **est couvert par
  un test d'hôte et n'a jamais couru en conditions de produit.**
- **Aucun texte non-ASCII, aucune fin de ligne Windows n'a traversé la chaîne
  réelle.** `normaliser` est couverte par trois tests d'hôte ; les cinq copies
  de la recette sont toutes ASCII et sans saut de ligne.
- **Rien de la latence de bout en bout**, qu'aucun sous-bloc du chantier D n'a
  jamais mesurée.
- **Le presse-papier reste PARTAGÉ entre les fenêtres d'une même session** (D3),
  et **entre deux utilisateurs d'une même VM** (R11) — la seconde est une
  question de **confidentialité**, elle est réelle, elle est hors périmètre v1,
  et elle appartient à ⑤.
- **La cause des octets NUL des journaux de pilote n'est pas établie** (§0).
- **La cause de la fenêtre qui s'ouvre puis se referme n'est pas établie**
  (§5.3).

---

## 8bis. La revue transverse de fin de branche (tâche 17)

**Sa cible propre** : les affirmations de code ou de document devenues **fausses
dans leur propre branche** — le mode de défaillance dominant de ce dépôt (D10 en
a trouvé sept de suite, D9 trois). Barème du dépôt, à titre d'ordre de grandeur
et jamais de quota : D7 5, D8 3, D9 6, D10 douze, D11 sept, P1 (plateforme) 8,
P2 10, S1 5, E 9, P3 12, S2 12, F1 11, P4 8, S3 13, G1 8, S4 27.

### 8bis.1 Les six questions que le plan posait, vérifiées une à une

| # | Question | Verdict, **vérifié par la commande** |
| --- | --- | --- |
| 1 | `tick.rs` énumère les branches qui « ne touchent même pas `self.rtc` » — `a1septies` y est-elle ? | ✅ **OUI.** « Onze branches supplémentaires » et onze sont listées, `a1septies` comprise ; « Dix d'entre elles (toutes sauf a1quater) » — le compte est juste. *D6 avait payé DEUX rondes pour avoir laissé cet énoncé faux, aux deux endroits du même fichier.* |
| 2 | Le doc-comment d'`AgentControl::Clipboard` promet-il ce que le refus de taille contredit ? | ✅ **NON** : il dit `text = None` sur refus, « refusé, jamais tronqué », et pourquoi `text` doit rester présent plutôt qu'omissible. |
| 3 | Le commentaire d'`Ok(autre)` de `pont_media.rs` dit-il le bon nombre de précédents ? **Il en faut cinq.** | ✅ **OUI** — « la CINQUIÈME fois », et les quatre précédents sont nommés (`Sommeil` D5, `Part` D6, `Audio` D7, `PleinEcran` D8). |
| 4 | Le commentaire du `Sondeur` dit-il **pourquoi** le garde n°2 est livré, et non « pour fermer la boucle » qui serait faux ? | ✅ **OUI** : « il ne ferme aucune boucle (il n'y en a pas), il absorbe le faux positif du compteur ». |
| 5 | Le tableau de `CLAUDE.md` porte-t-il **les deux** variables, avec leurs conventions **inverses** ? | ✅ **Fait par la tâche 18**, côte à côte. |
| 6 | Un commentaire dit-il « la recette qui l'exercerait n'a pas encore tourné » alors qu'elle a tourné ? | ✅ **Traité avant la revue** : `capteur/protocole.rs` portait « CETTE VARIANTE N'EST PAS ENCORE RELIÉE » et « le contrôle doit rendre UNE ligne » ; les deux étaient devenus faux et ont été corrigés par la tâche qui les réfutait. **Le contrôle publié aujourd'hui rend quatre lignes — re-lancé et vérifié.** |

### 8bis.2 Ce que la revue a trouvé, et qui n'était pas dans sa liste

**R1 — 🔴 La divergence E10 du plan porte une menace FAUSSE.** Le plan annonce
qu'un `PRESSE_PAPIER_SONDE` hérité ferait exécuter la sonde « par le processus
CAPTEUR, qui s'arrêterait aussitôt — et le superviseur le relancerait en
boucle ». **Cela supposerait que l'enfant porte la variable et pas son père.**
`superviseur/lanceur.rs` lance ses enfants par `std::process::Command`, qui
hérite de l'environnement : si le capteur la porte, le superviseur la portait
déjà, et il s'est arrêté à `main.rs:172` **avant d'avoir lancé quoi que ce
soit**. La **décision** du plan (lancer la sonde seule) est inchangée ; sa
**raison** est autre, et elle est désormais écrite aux deux endroits — dans
`agent/src/diagnostics.rs`, où un lecteur la rencontrera, et **dans le plan
lui-même**, qui ne portait pas encore l'annotation.

**R2 — La spec §6 P1 écrit « Aucun garde anti-écho n'est nécessaire ».** La
prémisse est juste (P1 n'écrit jamais), la conclusion sur la boucle d'écho
aussi — **mais P1 livre pourtant le garde n°2**, pour une raison que cette phrase
ne pouvait pas anticiper et que la sonde P0 a mesurée. Un lecteur qui repartirait
de cette ligne le retirerait. **Annoté dans la spec.**

**R3 — La spec §6 dit du critère ② « fait déclaré non mesuré, §8 ».** Il **est**
mesuré (`q2="bouge"`, deux exécutions), et le critère est donc **mesurable**.
**Annoté.**

**R4 — La spec §8 déclare « Rien du presse-papier Windows n'est MESURÉ à ce
jour ».** C'est la seule ligne de ce §8 que le chantier a déjà réfutée. ⚠️ **La
moitié `AddClipboardFormatListener` de cette ligne, elle, TIENT** : D2 l'écarte
et la sonde ne l'a pas éprouvée. **Barrée pour moitié, pas en bloc.**

**R5 — La spec §6 exige que ① se juge « sur le CONTENU collé localement ».**
Cette exigence **n'a pas pu être tenue** (§2 et §3.5) et le tableau ne le disait
pas. **Annoté à sa place**, dans la spec, sous le tableau des critères.

**R6 — 🔴 Le plan prescrit (tâche 18, étape 2) de corriger deux nombres de
`CLAUDE.md` « à leur place »… et les DEUX corrections avaient déjà été faites
par un chantier ultérieur.** Le tableau de clôture de **F1** publie déjà
`agent/src/transport.rs` **448** et `client/verify-webrtc.mjs` **494**, et
énumère nommément les quatre places datées où le 497 subsiste, avec la raison de
ne pas les réécrire. **Pire, le nombre que le plan avait lui-même MESURÉ le
19 août a vieilli en une journée** : `transport.rs` valait 495 pour le plan, il
vaut **448**. *Le naufrage du « 487 » sous sa forme la plus brève : un nombre
relevé le 19 était faux le 20.* **Annoté dans le plan, aux deux endroits où il
publie ce 495** — énumérés par `grep -n '495' ` avant d'écrire.

**R7 — Le legs n°1 du plan cite `agent/src/main.rs:434`.** Le fichier a été
extrait **en cours de sous-bloc** et ne fait plus que **300** lignes : cette
ligne n'existe pas. La bonne est **`:299`**. **Corrigé dans le plan.** *C'est
une affirmation devenue fausse dans sa propre branche, et par une tâche de cette
branche même.*

**R8 — Un compte publié en amont vaut 17 là où `cargo` en écrit 16.**
`cargo check --target x86_64-pc-windows-gnu` rend **16** avertissements — cargo
l'écrit lui-même. Un `grep -c '^warning'` rend 17 parce qu'il compte AUSSI la
ligne de résumé. **Le chiffre de cargo fait foi** (§9).

### 8bis.3 Les contrôles incapables d'échouer — les trois candidats désignés d'avance

Le plan (tâche 17, étape 3) en nommait trois. **Les trois ont été éprouvés, et
aucun n'est vacueux** :

- **le témoin de mesurabilité** — il **A** échoué, sur sa troisième branche, en
  rendant `NIVEAU 2 NON MESURABLE` ; ses deux premières branches, elles, ont
  réussi. Un contrôle qu'on a vu rouge n'est plus une conjecture ;
- **le critère ④** — le zéro du bras désarmé n'est discriminant que parce que le
  bras **armé** rend 4 sur le même protocole. Les deux bras ont été joués, deux
  fois chacun ;
- **le test `TYPES_AGENT`** — les neuf valeurs sont écrites **à la main**, donc
  indépendantes de la table jugée : une clé de trop le fait tomber, une clé
  manquante fait d'abord tomber `tsc`.

⚠️ **Et un quatrième, non désigné, a été trouvé — dans l'instrument** : le détour
de signaling renavigait la cible sur `Target.attachedToTarget`, où l'URL est
**vide**. Le test d'URL était donc **toujours faux, en silence** (§6).

### 8bis.4 Les pièces fabriquées — étape 4, relancée

Aucune pièce fabriquée n'a été trouvée. **Les comptes publiés en amont ont été
RELANCÉS**, pas recopiés (§9) : `cargo test -p agent` **671** ✅,
`proto` Rust **83** ✅, `proto` TS **142** ✅, client **304** (publié 295 —
le chantier concurrent a ajouté des tests), avertissements **16** (publié 17 —
R8). Le contrôle publié dans `capteur/protocole.rs`
(`grep -n 'DepuisCapteur::PressePapier' agent/src/capteur/pont_media.rs` rend
« quatre » lignes) a été **relancé** : il rend bien **4**.

---

## 9. Les comptes, **relevés le 20 août 2026, après la dernière édition**

| Commande | Résultat |
| --- | --- |
| `cargo test -p agent` | **671 passed, 0 failed** |
| `cargo test -p proto` | **83 passed, 0 failed** |
| `cd proto && npx vitest run` | **142 passed**, 5 fichiers |
| `cd client && npx vitest run` | **304 passed**, 33 fichiers |
| `cd agent && cargo check --target x86_64-pc-windows-gnu` | **sortie 0**, **16 avertissements** |

⚠️ **Le compte client vaut 304 et non 295** : le chantier concurrent (sous-bloc
P5 de la plateforme) a ajouté des tests à `client/` entre l'écriture de ce plan
et sa clôture. **Aucun test de P1 n'a disparu.**

⚠️ **Le compte d'avertissements est 16, pas 17.** `cargo` l'écrit lui-même
(« generated 16 warnings ») ; un `grep -c '^warning'` rend **17** parce qu'il
compte AUSSI la ligne de résumé. C'est le chiffre de cargo qui fait foi.

---

## 10. Ce que P1 lègue

1. ⛔ **Le propriétaire MONO-FENÊTRE n'existe pas** (E6, D-P1-6). Point de
   chute : `agent/src/demarrage.rs` — **491 lignes, marge 9**, relevé le 20 août
   2026 : 🔴 **une extraction préalable y sera requise.**
2. ⛔ **`Capabilities.clipboard`** (D7, E6) — reporté à P2, où il sert.
3. ⛔ **Une fenêtre attachée après une copie ne reçoit jamais ce contenu** (E5).
   Sans conséquence à une fenêtre, **réel en P3**. Remède nommé : émettre l'état
   courant à l'inscription, comme `parts::distribuer_les_parts` le fait déjà
   depuis `inscrire`.
4. ⛔ **Le canal `Message` du registre est non borné** (E9), et P1 y fait
   circuler jusqu'à 64 KiB par fenêtre et par changement. Nommé, non corrigé.
5. ⛔ **Le préalable éliminatoire de P2 n'est toujours pas mesuré** : l'événement
   `paste` parvient-il quand le focus est sur le `<video>` ? **Si la réponse est
   non, P2 est bloqué.**
6. ⛔ **Le niveau 2 du critère ① n'a jamais été atteint** (E11). La voie est
   nommée : `Xvfb` + `xdotool`.
7. ⛔ **`PRESSE_PAPIER_MAX` et `PERIODE_PRESSE_PAPIER` ne sont pas calibrées.**
8. ⛔ **Le presse-papier reste partagé entre les fenêtres d'une session** (D3) et
   **entre deux utilisateurs d'une même VM** (R11).

**Legs neufs de P1 :**

9. ⛔ **La lecture hors verrou n'est démontrée par aucune mesure**, et la
   contention qu'elle évite n'a jamais été exercée (§4).
10. ⛔ **Le chemin D-P1-5 (échec d'ouverture) n'a jamais couru en production** :
    `echecs_open=0` partout. C'est un chemin de code livré et jamais emprunté —
    la forme exacte de `borner_a_la_taille_max`, restée sans appelant un
    sous-bloc entier.
11. ⛔ **Aucun texte non-ASCII ni multi-ligne n'a traversé la chaîne réelle.**
    `normaliser` est le cœur de D-P1-2 et elle n'est éprouvée que sur l'hôte.
12. ⛔ **`agent/src/main.rs` a franchi 500 lignes AVANT P1, sans que rien ne le
    déclare** — 470 → **505** au commit `264c275` (« corrige(identite) : un seul
    canal /agent par VM »), hors de tout tableau de dette. P1 l'a **résorbé** par
    l'extraction de `agent/src/configuration.rs` : **300 + 244** aujourd'hui,
    relevés par la commande. **Le legs n'est pas la dette — elle est purgée —
    c'est que personne ne l'avait vue.**
13. ⛔ **Deux fichiers dépassent 500 lignes hors du tableau de dette** :
    `proto/src/plateforme/tests.rs` (**561**) et `proto/ts/plateforme.test.ts`
    (**512**), nés du sous-bloc **G1** et **absents de `CLAUDE.md`**. P1 n'y
    touche pas ; ils y sont inscrits par la tâche 18, parce qu'**une dette non
    écrite est une dette qu'on découvre**.
