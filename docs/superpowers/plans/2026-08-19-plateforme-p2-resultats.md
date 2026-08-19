# Sous-bloc P2 — résultats : la moitié navigateur, et la garde éprouvée de bout en bout

**Plan :** `docs/superpowers/plans/2026-08-19-plateforme-p2.md` (commit `826a16d`).
**Spec :** `docs/superpowers/specs/2026-08-19-plateforme-design.md`, §3.5 et §4 « P2 ».
**Journaux :** `docs/superpowers/plans/journaux-plateforme-p2/` — **tous versés
dans git**, UTF-8, sans séquences ANSI, `grep`-ables à plat.

**Portée de ce document** : il clôt les **tâches 14 à 18**, c'est-à-dire la
moitié navigateur et la recette. Les tâches 1 à 13 — tout le côté serveur — ont
été livrées auparavant (dernier commit du lot : `669b050`, puis `32c2eeb` et
`5add07b`), et leurs décisions ne sont pas rejouées ici.

**Commits de cette tranche** : `a4a9890` (14), `4be86ad` (15), `b229b98` (16),
`8879a4b` (17), et le présent document (18).

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

## 9. Ce qui reste aux tâches 19 à 21

Ce document est la **première rédaction** exigée par la tâche 18. Restent dus,
et **non faits ici** :

- **tâche 19** — la revue transverse de fin de branche, qui cherche les
  affirmations de code devenues fausses dans leur propre branche. Le seul point
  nommé d'avance par le plan (E2) — le commentaire de tête de `relais.ts`, qui
  disait « “Aucune authentification” reste VRAI, et le restera jusqu'à P2 » — a
  **déjà été traité par le lot serveur**, vérifié en le relisant : il porte
  désormais « n'est PLUS VRAI depuis le sous-bloc P2, et n'est PAS DEVENU FAUX
  POUR AUTANT — voici la moitié exacte qui reste vraie ». **La revue transverse
  reste due pour tout le reste**, et notamment pour les fichiers `client/` que
  cette tranche a touchés.
- **tâche 20** — la section P2 de `CLAUDE.md`. ⚠️ **Ce fichier était édité par un
  agent concurrent pendant toute cette tranche, et n'a donc pas été touché.**
- **tâche 21** — la clôture de ce document au vu de la revue transverse.
