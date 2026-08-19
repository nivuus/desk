# Sous-bloc P2 — résultats : la moitié navigateur, et la garde éprouvée de bout en bout

**Plan :** `docs/superpowers/plans/2026-08-19-plateforme-p2.md` (commit `826a16d`).
**Spec :** `docs/superpowers/specs/2026-08-19-plateforme-design.md`, §3.5 et §4 « P2 ».
**Journaux :** `docs/superpowers/plans/journaux-plateforme-p2/` — **tous versés
dans git**, UTF-8, sans séquences ANSI, `grep`-ables à plat.

**Portée de ce document** : les §1 à §8 closent les **tâches 14 à 18** — la
moitié navigateur et la recette. Les §9 à §12, écrits à la clôture, closent les
**tâches 19, 20 et 21** : le témoin rejoué, la **revue transverse**, le sort des
onze divergences, et le contrôle que rien ne vit hors de git. Les tâches 1 à 13 —
tout le côté serveur — ont été livrées auparavant (dernier commit du lot :
`669b050`, puis `32c2eeb` et `5add07b`), et leurs décisions ne sont pas rejouées
ici.

**Commits de cette tranche** : `a4a9890` (14), `4be86ad` (15), `b229b98` (16),
`8879a4b` (17), `f9cc330` (18, la première rédaction de ce document), et la
clôture (19-21).

⚠️ **Rien du §1 au §8 n'a été RÉÉCRIT à la clôture** : ce sont les relevés de la
recette, et ils restent vrais à leur date. Ce que la clôture ajoute, elle
l'ajoute **à la suite** — notamment le §9, où le témoin `verify-all.sh`, rejoué,
**rend 0** là où le §1 le donne en échec pour une cause étrangère.


🔴 **AUCUN TAUX N'EST REVENDIQUÉ NULLE PART.** Chaque énoncé porte son nombre
d'exécutions.

---

## 1. Le verdict des quatre critères

| # | Critère | Verdict | Exéc. | Pièce |
| --- | --- | --- | --- | --- |
| ① | un pair `client` non authentifié est refusé **et ne reçoit aucun `ice-config`** | **TENU** | **2** | `critere-1-{1,2}.log`, `rouge-1B-service-p1.log` |
| ② | un jeton expiré est refusé, **horloge qui varie** | **TENU** | **2** | `critere-2-{1,2}.log` |
| ③ | un utilisateur ne peut pas rejoindre la session d'un autre, et l'appartenance est **enregistrée** | **TENU** | **2** (+1 sur le service vivant) | `critere-3-{1,2}.log`, `critere-3-appartenance-en-base.log` |
| ④ | un mot de passe n'est **jamais** journalisé ni renvoyé | **TENU** | **2** | `critere-4-{1,2}.log` |
| témoin | non-régression, `verify-all.sh` sortie 0 | ❌ **NON TENU, et la cause est ÉTRANGÈRE à P2** | **2** | `verify-all-{1,2}.log`, `verify-ts-{1,2}.log`, `temoin-cargo-arbre-propre.log` |

⚠️ **Les deux exécutions d'un critère diffèrent par le MOTEUR de base** —
sqlite pour la première, postgres pour la seconde. C'est une répétition plus
forte que deux passes identiques pour les critères qui touchent la base, et
strictement identique pour ceux qui sont purs. **Ce n'est pas ce que le plan
prescrivait** (il disait « deux exécutions », sans dire lesquelles) : la
décision est prise ici et déclarée dans l'en-tête de chaque journal.

### ⚠️ Le témoin ne rend pas 0, et il faut lire pourquoi avant de l'imputer à P2

`scripts/verify-all.sh` échoue à sa **première** étape, aux **deux**
exécutions, sur un test **Rust** :

```
transport::sonde_montante::str0m_expose_l_opus_montant_via_media_data
test result: FAILED. 467 passed; 1 failed
```

Ce test vit dans `agent/src/transport/sonde_montante.rs`, qui était au moment
des deux exécutions **un fichier non suivi par git** (`git ls-files` ne le
rendait pas, `git status` le donnait `??`), accompagné d'une modification non
commitée d'`agent/src/transport.rs`. C'est le travail du **chantier E
(microphone)**, conduit par un autre agent **dans le même arbre** ; il l'a
commité depuis, sous `18ab224`.

**Ce n'est pas une affirmation, c'est une mesure** : un `git worktree add` de
`8879a4b` — le dernier commit de P2, **avant** `18ab224` — et le même
`cargo test --workspace` rendent **`ok. 467 passed; 0 failed`**, exactement les
467 qui passent dans l'arbre sale, où le 468ᵉ est celui du voisin. Aucun des
quatre commits de P2 ne touche `agent/`, vérifié **commit par commit**
(`temoin-cargo-arbre-propre.log`).

**Les sept étapes TypeScript de `verify-all.sh` ont donc été jouées à part,
deux fois, et toutes passent** (`verify-ts-{1,2}.log`) :

| Étape | Relevé |
| --- | --- |
| `client : npm test` | **120 passed** (13 fichiers) |
| `client : npm run typecheck` | sortie 0 |
| `proto : npm test` | **35 passed** (2 fichiers) |
| `proto : npm run typecheck` | sortie 0 |
| `plateforme : npm run test:sqlite` | **137 passed** (22 fichiers) |
| `plateforme : npm run test:postgres` | **137 passed** (22 fichiers) |
| `plateforme : npm run typecheck` | sortie 0 |

⚠️ **`cd client && npx vitest run` NE COUVRE PAS `proto/ts/`** — la racine
Vitest est `client/`, et les 120 tests sont ceux de `client/src/` seuls. Les
35 tests de `proto/ts` exigent `--dir ../proto` (ou `cd proto && npm test`).
**Un test posé hors de `client/src/` ne tournerait pas, et personne ne le
verrait** ; les deux comptes sont donc relevés séparément partout dans ce
document.

**Référence de non-régression** : les comptes de départ, relevés par la
commande avant la première ligne écrite, étaient client **107**, proto **35**,
plateforme **137 / 137**. P2 tâches 14-18 ajoute **13 tests côté client**
(107 → 120) et **aucun** ailleurs.

---

## 2. Chaque ROUGE jouée, avec son message verbatim

**Le plan en annonçait sept. Il y en a HUIT**, et la huitième existe parce que
la prédiction du plan sur la première était **fausse** — voir ①A-bis. Après
chacune, la source est restaurée et l'identité vérifiée (`git status` vide, ou
`md5sum -c` pour les fichiers pas encore suivis).

| # | Ce qu'on casse | Message d'échec, verbatim | Journal |
| --- | --- | --- | --- |
| **①A** | la garde neutralisée dans `relais.ts` (verdict forcé à `ok`) | `expected [ { type: 'ice-config', …(1) } ] to deep equally contain ObjectContaining{…}` — **5 tests sur 6 tombent** | `rouge-1A-garde-neutralisee.log` |
| **①A-bis** | 🔴 la garde refuse toujours, mais `ice-config` part **avant** elle | `expected [ 'ice-config', 'error' ] to not include 'ice-config'` | `rouge-1A-bis-fuite-ice-config.log` |
| **①B** | 🔴 le service **de P1** (`19f6409`), sorti tel quel d'un worktree | pas un test : un WebSocket nu reçoit de vrais identifiants TURN et **n'est jamais refusé** | `rouge-1B-service-p1.log` |
| **②** | l'`exp` ignoré dans `verifierJeton` | `expected { ok: true, sujet: 'utilisateur-42' } to deeply equal { ok: false, motif: 'expire' }` | `rouge-2-exp-ignore.log` |
| **③** | la revendication retirée de `relais.ts` | `expected undefined to be 'session-refusee'` | `rouge-3-revendication-retiree.log` |
| **④** | un `console.log(JSON.stringify(corps))` dans `/auth/connexion` | `expected '{"email":"ada@exemple.test","motdepas…' not to match /motdepasse\|mot_de_passe\|empreinte_mdp/i` | `rouge-4-mot-de-passe-journalise.log` |
| **E5** | la ligne **supprimée** à la rotation au lieu d'être marquée | `expected { ok: false, motif: 'inconnu' } to deeply equal { ok: false, motif: 'rejeu' }`, **sur les deux moteurs** | `rouge-E5-ligne-supprimee.log` |
| **lint** | un `expire_a INTEGER` dans `0002` | `expected 'un horodatage \`_a\` en INTEGER : 4 oct…' to be ''` | `rouge-lint-horodatage-integer.log` |

### 🔴 ①A-bis — une prédiction du plan, réfutée par la mesure

Le plan annonce que la rouge ①A fait tomber « **les DEUX** assertions du
critère ① ». **Elle n'en fait tomber qu'UNE.** `expect` interrompt le test à la
première : la seconde assertion — « aucun `ice-config` » — n'était donc pas
éprouvée, et **rien ne disait qu'elle pouvait échouer**. C'est exactement le
patron que ce dépôt a payé cinq fois : un contrôle qu'on n'a jamais vu rouge
n'est pas un contrôle.

La mutation ①A-bis l'éprouve **seule** : la garde refuse toujours (la première
assertion reste verte), mais la configuration ICE est envoyée **avant** la
vérification. C'est littéralement la fuite d'identifiants TURN de 24 h que
l'assertion existe pour attraper, et elle rougit :
`expected [ 'ice-config', 'error' ] to not include 'ice-config'`.

### 🔴 E5 — la première mutation n'éprouvait PAS ce qu'elle annonçait

Une première rédaction remplaçait l'`UPDATE` par un `DELETE … WHERE id = ? AND
? IS NOT NULL AND ? IS NOT NULL`, pour garder les trois paramètres. Les
paramètres étant `[maintenant, idNeuf, ligne.id]`, la clause devenait
`WHERE id = maintenant`, et **ne supprimait rien** : le rejeu était alors
**accepté** (`ok: true`, un jeton neuf délivré au voleur) plutôt que rendu
`inconnu`. C'était un défaut réel, mais **un autre**, décrit sous le nom de
celui-ci. Rejouée fidèlement (`DELETE … WHERE id = ?`, un seul paramètre), elle
rend bien le `inconnu` que le plan prédisait, sur les deux moteurs, et fait
tomber aussi le test de bout en bout de `routes-auth.test.ts`.

### La rouge ①B, décisive, et ce qu'elle montre exactement

Elle ne dépend d'**aucune** modification de notre part : `git worktree add
/tmp/p1-rouge 19f6409`, un lien symbolique de `node_modules` (le remède au
piège de D4 — un `npm start` depuis un worktree s'arrête **en silence** sans
lui), et le service de P1 démarre. Son arbre n'a même pas de répertoire
`plateforme/src/identite`. La **même** sonde, les **mêmes** variables TURN :

| | service de P1 (`19f6409`) | service de P2 (HEAD) |
| --- | --- | --- |
| reçoit `ice-config` | **oui** | **non** |
| identifiants TURN délivrés | `turn:127.0.0.1:3478`, `username: 1787233461:x`, `credential: TZ3Ink…` | aucun |
| refusé | **non** | **oui** — `{"type":"error","reason":"authentification requise","motif":"jeton-absent"}` |
| socket | **toujours ouvert** | **fermé, code 1008, motif `jeton-absent`** |

---

## 3. Ce que le code livre (tâches 14 à 17)

| Étage | Fichier | Lignes | Nature |
| --- | --- | --- | --- |
| le coffre | `client/src/jeton.ts` | **142** | **pur, aucun DOM** — `Coffre` injecté, `localStorage` touché seulement dans le défaut d'un argument |
| ses tests | `client/src/jeton.test.ts` | **129** | 10 tests, **4 mutations vues rouges** |
| l'écran | `client/connexion.html` + `client/src/connexion.ts` | **35** + **75** | câblage DOM seul, **aucune direction visuelle** (elle appartient au sous-projet ⑥) |
| l'entrée de build | `client/vite.config.ts` | **22** | +1 entrée `connexion` |
| la poignée de main | `client/src/webrtc.ts` | **300** | `jeton?` **facultatif** (E7), repli sur `jetonAcces()` |
| ses tests | `client/src/webrtc.test.ts` | **405** | +3 tests, **3 rouges vues** |
| la shell | `client/src/shell-page.ts` | **88** | le jeton, **et la seule redirection du client** |
| l'attente DevTools | `client/recette/devtools.mjs` | **27** | **neuf** — extraction de la copie triplicée |
| le jeton de recette | `client/recette/jeton-recette.mjs` | **121** | **neuf** — obtient de la plateforme, ne signe rien |

**Trois faits de conception qui survivront au code :**

1. **`SessionOptions.jeton` est FACULTATIF, et c'est une contrainte de
   conception, pas une commodité** (E7). Un champ requis obligerait à modifier
   `client/src/main.ts`, que le chantier D11 tient dans le même arbre. Le repli
   sur `jetonAcces()` est donc le chemin **nominal**, pas un secours — et il a
   son test.
2. **La redirection vers l'écran de connexion vit dans `shell-page.ts` SEUL.**
   Une page de session est toujours ouverte par la shell, sur la même origine,
   donc le jeton y est déjà ; rediriger depuis `webrtc.ts` donnerait à une
   bibliothèque un pouvoir sur la navigation de ses appelants. C'est écrit dans
   le code des deux côtés.
3. **Le champ `jeton` est AJOUTÉ, aucun n'est retiré** (spec §10.2) : un service
   de P1 ne lit que `role` et `session`, donc **ce client reste compatible avec
   un service antérieur à la garde**. La compatibilité ne va que dans ce sens, et
   un test garde cette moitié-là (`n'ajoute AUCUN champ hors 'jeton'`).

### La porte des 500 lignes, tenue par une EXTRACTION

`client/verify-webrtc.mjs` était à **497 lignes, marge 3**, et la tâche 17
devait y ajouter du code. **La porte exigeait une somme nulle ou négative ; le
relevé est 497 → 488, somme −9.** Elle est tenue par une **extraction**, jamais
par une compression : `waitForDevtools` vivait à l'identique dans les **trois**
pilotes et vit désormais dans `client/recette/devtools.mjs`, que les trois
importent.

⚠️ **Un seul changement de comportement, déclaré** : `paire-candidats.mjs`
levait `devtools timeout` et lève désormais le message commun, qui nomme la
cause.

**Relevé par la commande à la clôture** : aucun fichier du périmètre de P2 ne
dépasse 450 lignes, hors `verify-webrtc.mjs` (**488**), et le tableau de dette
du dépôt est inchangé (`agent/src/encode.rs` **1536**,
`agent/src/windows_source.rs` **630**).

---

## 4. Le jeton de recette : obtenu, jamais forgé (E6)

Les trois pilotes CDP du chantier D — `verify-webrtc.mjs`,
`recette/paire-candidats.mjs`, `recette/harness.mjs` — cessaient de se
connecter dès que la garde mord. Ils sèment désormais une paire dans
`localStorage` par le `Page.addScriptToEvaluateOnNewDocument` qu'ils appelaient
**déjà**.

🔴 **Le jeton vient de `POST /auth/connexion`, il n'est jamais signé dans le
script.** Un script qui signerait lui-même porterait le secret du service, ce
qui **déplacerait** le trou au lieu de le fermer. Le mot de passe vient de
l'environnement (`RECETTE_EMAIL`, `RECETTE_MOTDEPASSE`), jamais d'un fichier
versionné.

**Différentiel joué** — même binaire, même service, seules les deux variables
changent (`tache-17-differentiel.log`) :

| | refus côté service pendant l'exécution |
| --- | --- |
| **sans** jeton semé | **1** — `poignée de main refusée : poignée de main sans jeton sur la session diff-sans` |
| **avec** jeton semé | **0** |

**Le refus d'authentification n'est donc plus la cause de l'échec de l'outil.**

⚠️ **`verify-webrtc.mjs` échoue toujours, et la raison est ÉCRITE plutôt que
tue** : aucun agent ne répond à l'offre — il n'y a **aucune VM dans le
périmètre de P2** —, d'où `ÉCHEC : dimensions rapportées non plausibles
(undefinedxundefined)` et `connectionState (final) : new`. **Cet outil n'est pas
un critère de P2.**

**Un garde contre la divergence silencieuse, vu lever** : les clés du coffre
sont écrites dans **deux langages** sans `import` possible entre eux.
`jeton-recette.mjs` relit `client/src/jeton.ts` et **refuse de semer** si elles
ont divergé — éprouvé en mutant la clé, message obtenu : « la clé CLE_ACCES de
ce module ne correspond plus à celle de client/src/jeton.ts ».

---

## 5. Deux mesures de bout en bout que les tests d'hôte ne pouvaient pas donner

### La rotation et le rejeu, sur le service vivant — **2 exécutions**

`rotation-et-rejeu-bout-en-bout.log`, identique aux deux :

```
1. connexion            -> 200  r0=…
2. rotation de r0       -> 200  r1=…   (r1 != r0 : true)
3. rotation de r1       -> 200  r2=…
4. REJEU de r0 (volé)   -> 401  {"refus":"rejeu"}
5. r2, le plus RÉCENT, après le rejeu -> 401  {"refus":"revoque"}
```

**La ligne 5 est celle qui compte** : ce n'est pas seulement le jeton rejoué qui
tombe, c'est **toute la famille**, y compris le jeton neuf que le voleur
détiendrait. C'est le point d'E5, et c'est ce chemin-là qui avait révélé, côté
serveur, que `/auth/rafraichir` ouvrait une famille **neuve** à chaque appel —
défaut qu'aucun test de dépôt ne pouvait attraper.

### L'appartenance ENREGISTRÉE, sur le chemin réel — **1 exécution**

`critere-3-appartenance-en-base.log` : un pair `agent` et un pair `client`
authentifié s'apparient sur le service vivant, et la ligne en base porte

```
{ "nom_session": "s-appartenance-reelle",
  "utilisateur_id": "a64cfa4b-77a7-4317-a901-d4357f257b1c", … }
```

soit exactement l'`id` de `recette@exemple.test`. C'est ce qui rend le mot
« enregistrée » du critère ③ littéralement vrai (E3), et c'est ce dont P4 aura
besoin.

### CORS et le message de refus — **1 exécution**

`cors-et-message-de-refus.log` : l'en-tête `Access-Control-Allow-Origin` sort
pour `http://localhost:5173` (l'origine autorisée) et **pour elle seule** —
aucun en-tête pour `http://mechant.exemple`, **jamais `*`**. Et le corps du
refus est **identique** pour un compte inexistant et pour un mot de passe faux
(`{"refus":"identifiants"}`), ce qui interdit l'énumération de comptes.

C'est la seule chose éprouvable de la **prémisse** de `connexion.ts`, qui n'est
pas testé unitairement.

---

## 6. Les divergences relevées entre le plan et le code réel

| # | Divergence | Sort |
| --- | --- | --- |
| a | Le plan annonce que ①A fait tomber **les deux** assertions du critère ①. Elle n'en fait tomber qu'**une** | **RÉFUTÉ**, et remplacé par une huitième rouge (①A-bis) qui éprouve la seconde seule |
| b | Le plan situe la rouge E5 sur « la ligne supprimée ». La première mutation écrite ne supprimait **rien** | **CORRIGÉ**, rejoué fidèlement, les deux versions décrites |
| c | `client/` n'a pas `@types/node` : un test écrit avec `Buffer` passe sous Vitest et **casse `npm run typecheck`** | Le test emploie `btoa`/`TextEncoder`. Relevé : `TS2580 Cannot find name 'Buffer'` |
| d | Le plan dit « deux exécutions » sans dire lesquelles | Tranché : exécution 1 = sqlite, exécution 2 = postgres, déclaré dans chaque journal |
| e | Le témoin `verify-all.sh` ne peut **pas** rendre 0 : un chantier voisin travaille dans le même arbre | Déclaré, mesuré sur arbre propre, et les sept étapes TypeScript jouées à part |

⚠️ **Une pièce a d'abord dit le contraire de sa légende, et c'est corrigé dans
le journal lui-même** : `temoin-cargo-arbre-propre.log` affichait
`git diff --stat a4a9890^..HEAD -- agent/` sous le titre « aucune ligne de P2 ne
touche `agent/` ». Depuis que `18ab224` est arrivé **pendant la recette**, cet
intervalle contient aussi le commit du voisin, et la commande affichait donc ses
178 lignes. **C'est le commit qu'il faut nommer, pas la plage** : la
vérification est refaite commit par commit, et les quatre sont vides.

---

## 7. Ce que P2 n'établit PAS

- 🔴 **Le rôle `agent` reste ANONYME**, et reçoit toujours des identifiants TURN
  valables 86 400 s. **Le trou n'est fermé qu'à moitié**, exactement comme le
  libellé du critère ① le dit — il porte sur un pair **`client`**. Ce n'est pas
  une inférence : c'est mesuré sur le service de HEAD
  (`e2-role-agent-toujours-anonyme.log`), où une sonde qui déclare
  `{"role":"agent","session":"x"}` sans rien reçoit
  `urls: turn:127.0.0.1:3478, username: 1787233483:x, credential: qbUbdLD…` et
  n'est **jamais** refusée. L'agent Rust n'a pas d'identité avant **P3**.
- **La garde ne s'applique qu'à la POIGNÉE DE MAIN.** Une session déjà ouverte
  n'est jamais revérifiée : un jeton qui expire en cours de session ne coupe
  rien. La spec §3.5 dit qu'un jeton court n'est pas révocable avant
  expiration ; **ici il ne l'est même pas après**, tant que le socket vit.
- **L'appartenance de session ne survit pas à un redémarrage** (E3) : le
  registre est en mémoire, et après un redémarrage un nom de session libéré peut
  être revendiqué par un autre utilisateur. La vraie réponse est le préfixe
  opaque de **P3**.
- **Aucune protection contre le rejeu du jeton d'ACCÈS** : il est porteur, et
  quiconque l'obtient peut ouvrir une session jusqu'à son expiration.
- **Aucune constante n'est calibrée** : `N`/`r`/`p` de `scrypt`,
  `DUREE_JETON_ACCES_MS`, `DUREE_RAFRAICHISSEMENT_MS`, `LONGUEUR_SECRET_MIN`, la
  marge de rafraîchissement du client, et `DUREE_SECONDES = 86 400` que P2 **ne
  recalibre pas**. Elles rejoignent la liste déjà longue du dépôt (`BPP_MIN`,
  `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`, `HYSTERESIS`, `TAILLE_MAX_SORTIE`).
- **L'égalisation temporelle du chemin de connexion n'est pas mesurée**, et ne
  le sera pas ici : un test de temporisation serait instable. Ce qui est testé
  est le **message identique**, qui est décidable.
- **`client/src/connexion.ts` n'est pas testé unitairement**, et c'est une
  convention déclarée (comme `shell-page.ts` et `main.ts`). Seule sa **prémisse**
  — CORS, message de refus — est éprouvée, et sur **une** exécution.
- **AUCUN navigateur réel n'a authentifié quoi que ce soit** hors la tâche 17,
  qui n'est pas un critère : `verify-webrtc.mjs` sème son jeton et n'est plus
  refusé, mais aucune session média ne s'établit — il n'y a pas de VM.
- **L'écran de connexion n'a jamais été employé par un humain** : aucun clic
  réel, aucun formulaire soumis depuis un navigateur. Le chemin
  `connexion.html` → `poser()` → `shell.html` est **raisonné et compilé**, pas
  observé.
- **`shell-page.ts` n'a jamais redirigé** dans une exécution mesurée : la
  redirection vers `connexion.html` est du code jamais couru dans cette recette.
- **Aucun frein sur les routes d'authentification** — c'est P5 ③.
- **Aucun audit par un tiers** : CSRF, fixation de session et attaques
  temporelles sont traités par des choix raisonnés, **non éprouvés** (spec §8).
- **Le jeton vit dans `localStorage`, donc il est lisible par tout script de la
  page.** C'est un arbitrage écrit dans `jeton.ts`, pas un oubli, et il se
  rouvrira en P5 avec les en-têtes de sécurité.
- **Aucun taux, nulle part.**

---

## 8. Renvoi vers chaque journal versé

| Journal | Ce qu'il porte |
| --- | --- |
| `critere-1-{1,2}.log` | critère ①, fichier `garde-fil.test.ts` **entier** (aucun filtre `-t`, donc aucun test sauté) |
| `critere-2-{1,2}.log` | critère ②, `jeton.test.ts` + `garde-fil.test.ts` |
| `critere-3-{1,2}.log` | critère ③, `garde-fil` + `depot/session` + `signaling/trace` |
| `critere-3-appartenance-en-base.log` | la ligne `session.utilisateur_id` sur le service vivant |
| `critere-4-{1,2}.log` | critère ④, `routes-auth.test.ts` |
| `verify-all-{1,2}.log` | le témoin, **qui échoue** — voir §1 |
| `verify-ts-{1,2}.log` | les sept étapes TypeScript de `verify-all.sh`, toutes vertes |
| `temoin-cargo-arbre-propre.log` | `cargo test` sur un worktree de `8879a4b` : 467 / 0 |
| `rouge-1A-garde-neutralisee.log` | la garde forcée à `ok` |
| `rouge-1A-bis-fuite-ice-config.log` | la seconde assertion du critère ①, éprouvée seule |
| `rouge-1B-service-p1.log` | le service de P1 face à la même sonde que HEAD |
| `rouge-2-exp-ignore.log` | l'expiration ignorée |
| `rouge-3-revendication-retiree.log` | l'appartenance jamais revendiquée |
| `rouge-4-mot-de-passe-journalise.log` | le corps de la requête journalisé |
| `rouge-E5-ligne-supprimee.log` | la rotation qui supprime au lieu de marquer, **deux moteurs** |
| `rouge-lint-horodatage-integer.log` | `expire_a INTEGER` dans `0002` |
| `e2-role-agent-toujours-anonyme.log` | la moitié du trou que P2 ne ferme pas |
| `rotation-et-rejeu-bout-en-bout.log` | la rotation et le rejeu sur le service vivant, 2 exécutions |
| `cors-et-message-de-refus.log` | CORS jamais `*`, et le refus qui n'énumère pas |
| `tache-17-differentiel.log` | avec / sans jeton semé : 0 refus contre 1 |
| `tache-17-verify-webrtc-{avec,sans}-jeton.log` | les deux sorties complètes de l'outil |
| `admin-creer-utilisateur.log` | (tâche 9, lot précédent) |

---

## 9. Le témoin `verify-all.sh`, REJOUÉ à la clôture — et il rend 0

⚠️ **Le §1 ci-dessus reste le relevé de la RECETTE (tâches 14-18), et il n'est
pas réécrit.** Le témoin y échoue, pour une cause **étrangère** à P2. La tâche
19 l'a **relancé elle-même**, plutôt que de reprendre ce constat de confiance.

**Relevé le 19 août 2026 à 14:00:26 UTC**, `HEAD = 988e2ee` — c'est-à-dire
**deux commits du chantier E (microphone) APRÈS le dernier commit de P2**
(`f9cc330`) —, arbre portant en outre `M agent/src/main.rs` et `?? agent/src/micro.rs`,
qui sont **du voisin** :

| Étape | Relevé | Verdict |
| --- | --- | --- |
| `cargo test --workspace` | `ok. 480 passed; 0 failed` (+ `32 passed`, + `0 passed`) | **PASSE** |
| `cargo clippy --workspace` | `Finished`, 194 avertissements sur le binaire `agent` | **PASSE** |
| `client : npm test` | **120 passed** (13 fichiers) | **PASSE** |
| `client : npm run typecheck` | sortie 0 | **PASSE** |
| `proto : npm test` | **35 passed** (2 fichiers) | **PASSE** |
| `proto : npm run typecheck` | sortie 0 | **PASSE** |
| `plateforme : npm run test:sqlite` | **137 passed** (22 fichiers) | **PASSE** |
| `plateforme : npm run test:postgres` | **137 passed** (22 fichiers) | **PASSE** |
| `plateforme : npm run typecheck` | sortie 0 | **PASSE** |

`Toutes les vérifications sont passées.` — **code de sortie 0**, une exécution.

**Ce que cela établit, et ce que cela n'établit pas :**

- ✅ **le témoin est vert aujourd'hui**, et la cause de son échec pendant la
  recette a bien disparu quand le voisin a commité son test (`18ab224`) ;
- ⚠️ **les 480 tests Rust ne sont PAS les 467 de la recette** : le chantier E en
  a ajouté treize, **P2 n'en a ajouté aucun** — aucun de ses commits ne touche
  `agent/`. Le nombre n'est donc **pas** un compte de non-régression de P2, et
  **il ne se compare pas** aux 467 du `temoin-cargo-arbre-propre.log` ;
- ⚠️ **les comptes TypeScript, eux, SONT attribuables** : les trois commits du
  chantier E ne touchent ni `client/`, ni `proto/`, ni `plateforme/` (vérifié
  par `git show --stat` sur les trois). **120 / 35 / 137 / 137 au commit
  `988e2ee`** ;
- ⚠️ **une exécution. Aucun taux.**

---

## 10. La revue transverse de fin de branche (tâche 19)

Sa cible propre : **les affirmations — commentaires, documents, `CLAUDE.md` —
que la branche P2 elle-même a rendues fausses.** Elle a trouvé **5** défauts en
D7, **3** en D8, **6** en D9, **douze** en D10, **huit** en P1, **sept** en D11.

**Elle trouve ici DIX affirmations distinctes, réparties sur VINGT-TROIS
PLACES**, plus **une** qui n'est pas imputable à P2 et le dit.

⚠️ **Ces deux comptes ne mesurent pas la même chose.** « Dix » est ce qu'un
relecteur a trouvé ; « vingt-trois » est ce qu'il a fallu éditer, et c'est le
second qui coûte — parce que *« corrigé à sa place » est une affirmation de
COMPLÉTUDE*, et que ce dépôt a payé **neuf fois** pour un nombre laissé dans une
place non balayée. Les barèmes ci-dessus comptent des **défauts**, pas des
places : **seule la colonne de gauche leur est comparable.**

🔵 **Le fait de forme le plus utile : UNE SEULE affirmation occupe SEPT des
vingt-trois places.** C'est « le port, s'il est atteint, délivre des identifiants
TURN à quiconque » — que P1 avait pris soin d'écrire **partout**, précisément
parce que c'était son trou connu. **Une branche qui ferme un trou rend fausses
toutes les phrases qui le décrivaient**, et leur nombre est proportionnel au
soin qu'on avait mis à le nommer. **Aucune n'était fausse quand elle a été
écrite.**

### 10.1 Le tableau, une ligne par PLACE

| # | Place | L'affirmation | Sort |
| --- | --- | --- | --- |
| 1 | `plateforme/src/signaling/relais.ts` — bloc `TYPES_RELAYES` (annotation l. **33**) | « un canal de diffusion arbitraire **sur un serveur sans authentification** » | **CORRIGÉE** — fausse de moitié. Le bornage des **types** garde son sens : il borne ce qu'un `agent` anonyme fait transiter, **et** ce qu'un `client` authentifié diffuse. *Une identité n'est pas une autorisation de relayer n'importe quoi.* |
| 2 | `plateforme/src/signaling/relais.ts` — bloc `isJsonObject` (annotation l. **69**) | « un WebSocket **exposé sans authentification** » | **PRÉCISÉE — ELLE RESTE VRAIE**, et il fallait dire pourquoi, sans quoi un successeur la croirait périmée et desserrerait la garde de type. Le contrôle court sur le **premier** message, donc **avant** la garde : vérifié par la commande, `isJsonObject` est appelé l. **149**, `garde.verifier` l. **180** |
| 3 | `plateforme/src/signaling/resilience.test.ts:3-4` (annotation l. **6**) | « un déni de service en une trame, **sans authentification requise** » | **PRÉCISÉE — RESTE VRAIE**, pour la raison de la ligne 2, **et** parce que le rôle `agent` reste anonyme. **Le déni de service en une trame est toujours ouvert**, et c'est pourquoi ce fichier existe encore |
| 4 | `plateforme/src/signaling/appariement.ts:8-13` (annotation l. **14**) | « extrait **AVANT** que P2 (garde d'authentification), P3 et P4 n'y ajoutent quoi que ce soit » | **CORRIGÉE** — P2 a eu lieu, **et la marge a servi AILLEURS** : ce module n'a pas gagné une ligne, c'est `relais.ts` qui a grossi. **L'extraction a rendu sa marge au fichier qui en avait besoin** |
| 5 | `plateforme/src/config.ts:6-11` (annotation l. **13**) | « délivrait des identifiants TURN valables 24 h **à quiconque** atteignait le port » | **PRÉCISÉE** — vraie au passé, et **la moitié `agent` a survécu**. L'argument pour `PLATEFORME_HOTE` n'a rien perdu ; il a gagné, la fenêtre restant ouverte plus longtemps que P1 ne le prévoyait |
| 6 | `plateforme/src/base/migrations/0001-socle.sql:23-24` (annotation l. **26**) | « `utilisateur` est créée par P1 et **RESTE VIDE** : P2 lui donne son comportement, pas sa table » | **ANNOTÉE : FAIT.** La table n'est plus vide, **et elle n'a pas bougé d'une ligne** — exactement ce que la phrase promettait, et ce qui rendait la contrainte D3 payante |
| 7 | `…/0001-socle.sql:56-58` (annotation l. **70**) | « `utilisateur_id` et `vm_id` naissent NULL : en P1 il n'y a ni utilisateur ni VM » | **ANNOTÉE** — P2 renseigne le premier, et la colonne **reste nullable pour une raison qui n'est pas de la dette** : une session appariée par un agent seul (`bureau`) n'a personne à inscrire. **`NOT NULL` serait FAUX, pas seulement coûteux.** `vm_id` reste vide : c'est P3 |
| 8 | `client/verify-webrtc.mjs` — bloc `Usage:` (annotation l. **16**) | l'invocation complète de l'outil | **COMPLÉTÉE** — voir 10.2 |
| 9 | `client/recette/paire-candidats.mjs` — bloc `Usage:` (annotation l. **16**) | idem | **COMPLÉTÉE** |
| 10 | `client/recette/harness.mjs` — bloc `Usage:` (annotation l. **24**) | idem | **COMPLÉTÉE** |
| 11 | `docs/…/2026-08-19-plateforme-p1-resultats.md` §7 (annotation l. **241**) | « **Aucune authentification.** Le port … à quiconque. **C'est P2** » | **ANNOTÉE, JAMAIS RÉÉCRITE** — c'est un relevé daté, vrai comme histoire ; le barrer le rendrait faux |
| 12 | `docs/…/specs/2026-08-19-plateforme-design.md` §2.3 ③ (annotation l. **117**) | « Les identifiants TURN sont délivrés **sans aucune authentification** » | **ANNOTÉE** — vraie de moitié. L'annotation note aussi que la ROUGE gratuite du §7.2 (« le service de P1 délivre `ice-config` à quiconque ») **a été JOUÉE** |
| 13 | `CLAUDE.md` § « Chantier C volet 2 » (l. **2015**) | « le service lit désormais **quatre** variables de plus …, dont **la première** n'a aucun défaut » | **CORRIGÉE** — **six**, et **deux** sans défaut (`PLATEFORME_HOTE`, `PLATEFORME_SECRET_JETON`). ⚠️ **Ce paragraphe existe PRÉCISÉMENT pour prévenir un lancement à environnement incomplet** : il l'était lui-même. Un constat neuf y est ajouté — **`.env` ne porte AUCUNE `PLATEFORME_*`** (vérifié : `grep -c 'PLATEFORME_' .env` rend **0**) |
| 14 | `CLAUDE.md` section P1 ① (annotation l. **6905**) | « l'agent ne lit aucune des **quatre** variables neuves » | **ANNOTÉE** — **le fond reste vrai** (les deux variables de P2 sont côté serveur, `run-agent.sh` n'est pas touché), **seul le compte a vieilli** |
| 15 | `CLAUDE.md` section P1 ② — titre et tableau (annotation l. **6913**) | « Les **quatre** variables d'environnement neuves » | **ANNOTÉE** — les quatre lignes ne sont pas fausses, elles sont **incomplètes**, et c'est le tableau qu'un opérateur lit en premier |
| 16 | `CLAUDE.md` section P1 ⑧ (annotation l. **7096**) | « **Aucune authentification.** … **C'est P2** » | **ANNOTÉE** — relevé daté, vraie de moitié |
| 17 | `CLAUDE.md` section P1 ⑨ (l. **7137**) | « `resilience.test.ts` fixe donc les **quatre** variables » | **CORRIGÉE** — **cinq** (relevé par la commande : `PLATEFORME_SECRET_JETON` s'y ajoute l. 63) |
| 18 | `CLAUDE.md` section P1 ⑩ — tableau de tailles | quatre chiffres périmés par P2 | **ANNOTÉE ET REMESURÉE** — voir 10.3 |
| 19 | `CLAUDE.md` section P1 ⑪ — ligne `relais.ts:2` (l. **7195**) | « ⚠️ « Aucune authentification » **reste VRAI** et n'est pas touché » | **ANNOTÉE** — **le pronostic n'était juste que pour UN sous-bloc**, ce qui est la durée de vie ordinaire d'un « reste VRAI » |
| 20 | `CLAUDE.md` section P1 ⑫ leg 1 (l. **7229**) | « ⛔ **P2 — l'authentification** … `utilisateur` **existe et est vide** » | **ANNOTÉE : fait, et de moitié seulement** |
| 21 | `CLAUDE.md` section P1 ⑫ leg 2 (l. **7236**) | « ⛔ **P2 et P4 — toute contrainte doit naître avec sa table** » | **ANNOTÉE : APPLIQUÉ par `0002-identite.sql`** (`famille`, `remplace_par`, et la clé étrangère vers `utilisateur(id)`, toutes trois nées avec la table). **La consigne reste entière pour P4** |
| 22 | `CLAUDE.md` relevé D11 | `client/verify-webrtc.mjs` **497**, marge **3**, « la **deuxième** la plus serrée du dépôt », « **inchangée depuis D10, donc elle dérive toujours** » | **ANNOTÉE** — voir 10.3 |
| 23 | `CLAUDE.md` piège D11 | « **497 au dépôt commité** et **488 dans l'arbre de travail** » | **ANNOTÉE** — les 488 **étaient** la modification non commitée de P2 ; **elle est commitée depuis**, et le fichier vaut désormais autre chose. *La leçon du piège — dater tout compte quand l'arbre est partagé — n'a rien perdu ; elle vient au contraire d'être payée une fois de plus.* |

### 10.2 Ce que les trois blocs `Usage:` avaient perdu, et la leçon neuve

La tâche 17 a ajouté `semerJeton(cdp)` **dans le corps** des trois pilotes CDP,
et a écrit toute la doctrine dans `client/recette/jeton-recette.mjs`. **Elle n'a
pas touché leurs blocs `Usage:`**, trois à vingt lignes plus haut, qui
continuaient donc de promettre une invocation complète sans nommer
`RECETTE_EMAIL`, `RECETTE_MOTDEPASSE` ni `PLATEFORME_URL`. Ce n'est pas un
défaut d'exécution : c'est **exactement la forme que ce dépôt attribue à la
revue transverse** — correct des deux côtés pris séparément.

⚠️ **Le symptôme aurait été bénin et trompeur** : sans les variables,
`semerJeton` **avertit et rend `false`**, l'outil continue, et la session est
refusée par la garde — l'opérateur lirait un échec WebRTC là où il manque deux
variables d'environnement.

🔵 **LEÇON NEUVE, ET ELLE EST PETITE : une addition de commentaire peut annuler
une extraction.** La première rédaction de la correction n°8 faisait
**+9 lignes** sur `client/verify-webrtc.mjs` et le ramenait **exactement à
497** — c'est-à-dire au chiffre d'avant l'extraction que la tâche 17 venait de
payer pour tenir la porte des 500 (497 → 488, somme −9). **Le même jour, la même
branche, le gain rendu par une extraction repris par un commentaire.** Elle a
été **resserrée sur place**, la doctrine restant dans `jeton-recette.mjs` où
elle vit déjà : **488 → 494**, somme toujours négative face à 497, marge **6**.
**C'est déclaré, pas découvert** — et c'est la quatrième fois que ce dépôt écrit
que *la marge regagnée par une extraction se reperd si on la traite comme
acquise*.

### 10.3 Les chiffres, RELEVÉS PAR LA COMMANDE après la dernière édition

🔴 **Relevés APRÈS la dernière édition de la ronde, y compris celles de la revue
transverse.** Une table mesurée en début de ronde est fausse à la fin de la même
ronde.

🔴 **ET AVEC LEUR COMMIT.** L'arbre est partagé avec le chantier E (microphone),
qui a commité **trois fois pendant cette seule tâche de clôture** : `HEAD` est
passé de `f9cc330` → `988e2ee` → **`85ed23a`**. Le relevé ci-dessous est celui
de **`85ed23a`**, arbre portant en outre les éditions non commitées de P2 et un
`M agent/src/opus.rs` du voisin.

**Le dépôt entier ne porte que DEUX fichiers de plus de 500 lignes** — la dette
gelée, `agent/src/encode.rs` **1536** et `agent/src/windows_source.rs` **630**,
**ni l'un ni l'autre touché par P2**. **Aucun fichier de `plateforme/` ni de
`client/src/` ne dépasse 500**, ni ne s'en approche.

| Fichier du périmètre P2 | Lignes | Marge |
| --- | --- | --- |
| `client/src/main.ts` | **408** | 92 — *et P2 n'y a pas touché (E7)* |
| `client/src/webrtc.test.ts` | **405** | 95 |
| `plateforme/src/signaling/relais.ts` | **310** | 190 |
| `client/src/webrtc.ts` | **300** | 200 |
| `plateforme/src/signaling/server.test.ts` | **272** | 228 |
| `plateforme/src/http/routes-auth.ts` | **241** | 259 |
| `plateforme/src/signaling/garde-fil.test.ts` | **215** | 285 |
| `plateforme/src/http/routes-auth.test.ts` | **215** | 285 |
| `plateforme/src/signaling/resilience.test.ts` | **208** | 292 |
| `plateforme/src/signaling/trace.test.ts` | **194** | 306 |
| `plateforme/src/depot/jeton.test.ts` | **165** | 335 |
| `plateforme/src/depot/jeton.ts` | **155** | 345 |
| `plateforme/src/identite/mot-de-passe.ts` | **153** | 347 |
| `plateforme/src/base/pilotes.test.ts` | **141** | 359 |
| `client/src/jeton.ts` | **142** | 358 |
| `client/src/jeton.test.ts` | **129** | 371 |

**Les marges les plus serrées du DÉPÔT à cette date** : `agent/src/encode/arret.rs`
**500** (marge **0**), **`agent/src/micro/tests.rs` 497 (marge 3)** — fichier
**neuf du chantier E**, commité pendant cette clôture, **rien à voir avec P2** —,
`client/verify-webrtc.mjs` **494** (marge **6**), `agent/src/superviseur/table.rs`
**492** (8), `agent/src/capture.rs` **492** (8).

**`client/verify-webrtc.mjs` : 497 → 488 → 494.** Les trois chiffres comptent :
497 est le point de départ (marge 3) où la tâche 17 devait ajouter du code ; 488
est ce que l'**extraction** de `waitForDevtools` vers `client/recette/devtools.mjs`
(**27** lignes, importé par les **trois** pilotes) a rendu, somme **−9** ; 494
est ce que la revue transverse y a remis, **100 % commentaire, déclaré**. La
somme de la branche reste **négative** face à 497.

⚠️ **Le tableau de tailles de la section P1 de `CLAUDE.md` portait CINQ chiffres
périmés par P2 sur SEPT**, tous barrés à leur place : `server.test.ts` 255 →
**272**, `relais.ts` 219 → **310**, `resilience.test.ts` 181 → **208**,
`trace.test.ts` 151 → **194**, `pilotes.test.ts` 135 → **141**. Le plus gros
mouvement, **+91 sur `relais.ts`**, est exactement ce que P1 avait annoncé — « le
seul que P2, P3 et P4 feront grossir » — et **sa marge reste de 190**, ce que
l'extraction d'`appariement.ts` par P1 est ce qui permet.


### 10.4 Une affirmation trouvée qui N'EST PAS imputable à P2, et qui le dit

`CLAUDE.md`, section P1 ⑪, ligne `agent/src/superviseur/protocole.rs:5` :
« ❌ **NON CORRIGÉ, et c'est délibéré** … Dette d'une ligne, à reprendre par qui
touchera ce fichier ».

**Elle est soldée — par D11, pas par P2.** Le fichier nomme aujourd'hui
`plateforme/src/signaling/relais.ts`, et ses lignes 8-15 disent que l'ancien
chemin n'existe plus. **P2 n'a modifié aucune ligne d'`agent/`** ; la ligne a
donc été annotée en nommant son véritable auteur.

⚠️ **Ce qui mérite d'être retenu, c'est que `CLAUDE.md` se contredisait
lui-même depuis D11** : son tableau de tête enregistrait bien
« `superviseur/protocole.rs` 101 → 110 — le chemin `signaling/` disparu », à
six mille lignes du ❌ qui disait l'inverse. **Personne n'avait rapproché les
deux**, et c'est précisément ce qu'une revue transverse est là pour faire.

### 10.5 Ce que la revue transverse N'A PAS fait

- ⛔ **Elle n'a rien corrigé dans `agent/`, `proto/`, ni `scripts/run-agent.sh`** —
  périmètre d'un chantier concurrent (microphone, E1) au moment de la clôture.
  **Relu quand même** : `grep -rn "authentif\|anonyme\|sans jeton" agent/src proto`
  ne rend **aucune** affirmation périmée par P2 (les occurrences d'`agent/`
  parlent de traces anonymes, de types anonymes et de l'authentification TURN).
- ⛔ **Elle n'a PAS repris les chemins `signaling/src/…` périmés de la SPEC ni
  des plans anciens.** Ils sont nombreux — le §2 de
  `2026-08-19-plateforme-design.md` cite une dizaine de `server.ts:NNN` et
  `ice.ts:NNN` qui ne désignent plus rien —, mais **ils ont été rendus faux par
  P1, pas par P2**, et ce sont des relevés **datés**. Seul le §2.3 ③, que le
  plan de P2 nomme, est annoté. **Déclaré, pas oublié.**
- ⛔ **Elle n'a pas relu les journaux de la recette** : ce sont des pièces
  datées, non maintenues.

---

## 11. Aucune preuve ne vit hors de git (tâche 21)

```
$ git status --porcelain docs/superpowers/plans/journaux-plateforme-p2/
(vide)
$ git ls-files docs/superpowers/plans/journaux-plateforme-p2/ | wc -l
29
```

**Vide, et 29 fichiers suivis.** Aucune pièce de ce sous-bloc ne vit dans un
espace de travail gitignoré.

⚠️ **C'est la contre-mesure de D10**, qui avait établi par la commande que
l'espace de travail de D9 (`.superpowers/sdd/`) **avait disparu**, emportant six
constats de revue définitivement perdus. **Toute affirmation de ce document est
adossée soit à une pièce de ce répertoire, soit à une commande relancée à la
clôture et citée avec son relevé.**

⚠️ **Une réserve, et elle est réelle** : les **rapports de tâche** de P2
(`.superpowers/sdd/…`) ne sont **pas** versés, exactement comme ceux de D9. Ce
qu'ils portaient d'essentiel a été transporté ici et dans les journaux **au
moment où il a été produit**, ce qui est la seule défense possible — mais **rien
ne garantit qu'il n'en reste rien**, et le prétendre serait affirmer au-delà du
relevé.

---

## 12. Le sort des ONZE divergences E1…E11

Elles avaient été tranchées **avant** d'écrire une ligne (§« Divergences » du
plan). Voici ce que le code livré en a fait — **une ligne chacune**.

| # | Ce qu'elle posait | Sort dans le code livré |
| --- | --- | --- |
| **E1** | la garde casse trois fichiers de test livrés par P1 | **TENUE À LA LETTRE** : garde **REQUISE** en 2ᵉ position de `createSignalingServer`, **aucune assertion changée** dans les trois fichiers, `GARDE_OUVERTE` confiné au fichier de test et **jamais exporté** par du code de production |
| **E2** | P2 ne ferme que la moitié `client` du trou `ice-config` | **TENUE, ET MESURÉE** : le rôle `agent` reste anonyme, relevé sur le service de HEAD (**1 exécution**, `e2-role-agent-toujours-anonyme.log`). C'est le **legs n°1 vers P3** |
| **E3** | « appartenance enregistrée » ne dit pas OÙ | **TENUE EN DEUX ÉTAGES** : la **décision** par `signaling/propriete.ts` (pur, synchrone) ; l'**enregistrement** dans `session.utilisateur_id`, vérifié sur le service vivant (**1 exécution**). ⚠️ Le registre **ne survit pas à un redémarrage** — coût nommé, non corrigé, réponse en P3 |
| **E4** | la spec ne dit rien de CORS | **TENUE** : `PLATEFORME_ORIGINE_CLIENT` facultative, **défaut = refus**, **jamais `*`**, l'origine demandée doit **égaler** l'autorisée. Éprouvée sur le service vivant (**1 exécution**, `cors-et-message-de-refus.log`) |
| **E5** | le schéma de la spec §5 ne permet pas de détecter un rejeu | **TENUE, ET ELLE A PAYÉ** : `famille` et `remplace_par` naissent avec la table ; c'est en l'éprouvant de bout en bout qu'on a trouvé que `/auth/rafraichir` ouvrait une famille **neuve** à chaque appel — **défaut réel, corrigé** (§5) |
| **E6** | P2 casse trois outils de recette du chantier D | **TENUE, SANS INTERRUPTEUR PERMISSIF** : les trois **sèment** un jeton **obtenu** de `POST /auth/connexion`, jamais forgé. Différentiel joué : 1 refus sans, 0 avec |
| **E7** | `SessionOptions` ne peut pas gagner un champ requis | **TENUE** : `jeton?` facultatif, repli sur `jetonAcces()`, **`main.ts` PAS touché** — et le repli est le chemin **nominal**, avec son test |
| **E8** | `scrypt` refuse le paramètre qu'on croirait meilleur | **TENUE** : `N = 16384, r = 8, p = 1`, `maxmem` **non touché**, et le relevé du refus de `N = 32768` est **inscrit verbatim** dans le fichier |
| **E9** | `timingSafeEqual` lève sur des longueurs différentes | **TENUE AUX DEUX ENDROITS** : mot de passe **et** signature JWT comparent les longueurs d'abord. C'est **la rouge la plus utile** de la tâche 1 |
| **E10** | trois modules absents de l'arborescence de la spec §5 | **TENUE** : `signaling/propriete.ts`, `http/cors.ts`, `admin/creer-utilisateur.ts` créés, **divergence déclarée plutôt que découverte**, aucune décision de la spec modifiée |
| **E11** | « création de compte par ligne de commande » n'a ni entrée ni convention | **TENUE** : `npm run admin:utilisateur -- --email <adresse>`, mot de passe sur **stdin SEUL**, `--mot-de-passe` **refusé explicitement** avec son motif (assertion de test) |

**Aucune des onze n'a été abandonnée en cours de route**, et **une seule a
révélé un défaut à l'exécution** (E5). Les cinq divergences trouvées **pendant**
l'exécution, elles, vivent au §6.

