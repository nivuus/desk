# Sous-bloc P2 — l'identité des humains, et la garde du signaling : plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Donner une identité aux humains — table `utilisateur`, mot de passe
par `scrypt`, jeton d'accès JWT court, chaîne de rafraîchissement hachée et
rotative avec détection de rejeu — puis **conditionner à cette identité la
poignée de main du rôle `client` et la délivrance d'`ice-config`**, qui sont
aujourd'hui ouvertes à quiconque atteint le port.

**Architecture:** Rien de neuf dans le transport. La garde est une **fonction
pure et synchrone** appelée par le relais avant `Appariement::declarer`, sur le
même chemin que la déclaration de rôle ; elle ne touche ni la base ni une
horloge réelle (les deux lui sont injectées). L'identité elle-même vit dans
quatre modules purs (`identite/mot-de-passe.ts`, `identite/jeton.ts`,
`identite/garde.ts`, `signaling/propriete.ts`) et deux dépôts
(`depot/utilisateur.ts`, `depot/jeton.ts`). Deux routes HTTP les exposent. Le
navigateur gagne un écran de connexion et porte son jeton dans la poignée de
main.

**Tech Stack:** inchangée — Node/TypeScript, Vitest, `ws`, `node:sqlite`, `pg`.
🔴 **P2 N'AJOUTE AUCUNE DÉPENDANCE**, et ce n'est pas une intention mais une
propriété **mesurée** (§« Contraintes globales », relevés ① et ②) : `scrypt` et
HMAC-SHA256 sont dans `node:crypto`, donc le JWT HS256 s'écrit sans
`jsonwebtoken`. Le contrôle `n'ajoute aucune dépendance de production hors
l'allow-list` (`plateforme/src/base/pilote.test.ts:38-50`) reste **inchangé** et
doit rester **vert** : c'est le témoin de cette propriété.

**Spec :** `docs/superpowers/specs/2026-08-19-plateforme-design.md` (commit
`217a765`), §3.5 et §4 « P2 ».
**Sous-bloc précédent :** `docs/superpowers/plans/2026-08-19-plateforme-p1.md`
et son document de résultats, **qui fait autorité sur l'état du code**.

**Ce plan ne couvre QUE P2.** Rien de l'identité des agents ni du canal
plateforme ↔ agent (P3), rien de l'orchestration ni de l'attribution de VM
(P4), rien du durcissement de production — TLS, coturn restreint, frein sur les
routes d'authentification, `/sante` (P5). Les tables `agent_enrole` et
`application` ne sont pas créées.

---

## Contraintes globales

### Ce qui a été relevé PAR LA COMMANDE avant d'écrire une ligne

Toutes les valeurs ci-dessous ont été obtenues le **19 août 2026**, sur cette
machine, en lançant réellement la commande. Aucune n'est recopiée d'un document
antérieur. **Ce qui n'a pas été mesuré est marqué comme tel partout ailleurs
dans ce plan.**

| # | Commande | Résultat relevé |
| --- | --- | --- |
| ① | `node -e "const c=require('node:crypto'); …"` | `scrypt function`, `scryptSync function`, `timingSafeEqual function`, `randomBytes function`, `createHmac function`, `randomUUID function` |
| ② | `scryptSync('mdp', sel16, 32, {N:16384,r:8,p:1})` | **`OK 29 ms`** |
| ③ | le même avec `N:32768` | 🔴 **`REFUSE -> Invalid scrypt params: error:030000AC:digital envelope routines::memory limit exceeded`** |
| ④ | `timingSafeEqual(Buffer.from('aa'), Buffer.from('aaa'))` | 🔴 **lève `Input buffers must have the same byte length`** |
| ⑤ | JWT HS256 composé à la main par `createHmac('sha256', …).digest('base64url')` | jeton produit, signature de **43** caractères, charge relue par `Buffer.from(…, 'base64url')` |
| ⑥ | `node --version` | `v24.9.0` |
| ⑦ | `cd plateforme && npm run test:sqlite` | `Test Files 12 passed (12)`, `Tests 64 passed (64)` |
| ⑧ | `cd plateforme && npm run test:postgres` | `Test Files 12 passed (12)`, `Tests 64 passed (64)` |
| ⑨ | `cd plateforme && npm run typecheck` | sortie **0** |
| ⑩ | `cd client && npm test` | `Test Files 12 passed (12)`, `Tests 107 passed (107)` |
| ⑪ | `docker compose -f docker-compose.plateforme.yml ps` | `guacamole-postgres-plateforme-1 … Up 19 minutes (healthy) 127.0.0.1:5433->5432/tcp` |
| ⑫ | la commande des 500 lignes de `CLAUDE.md` | **deux** fichiers seulement : `agent/src/encode.rs` **1536**, `agent/src/windows_source.rs` **630** |

⚠️ **Les relevés ⑦ à ⑩ sont les références de non-régression de P2.** Toute
tâche qui les fait baisser a cassé quelque chose ; toute tâche qui les fait
monter doit dire de combien et pourquoi.

⚠️ **Le relevé ③ est un piège de construction, pas une curiosité** : la limite
mémoire par défaut d'OpenSSL (32 MiB) est franchie dès `N = 32768` avec
`r = 8`, et l'erreur ne parle **pas** de `N` — elle parle d'« enveloppe
numérique ». Un successeur qui durcirait le paramètre sans lever `maxmem`
recevrait ce message et ne saurait pas d'où il vient. Il est écrit dans le code
par la tâche 1.

### La règle des 500 lignes : relevée, et AUCUNE extraction n'est requise dans `plateforme/`

**Relevé par la commande le 19 août 2026** (`find plateforme/src -name '*.ts'
-o -name '*.sql' | xargs wc -l | sort -rn`), pour les seuls fichiers que P2
modifie :

| Fichier | Lignes | Marge | Ce que P2 y ajoute | Porte |
| --- | --- | --- | --- | --- |
| `plateforme/src/signaling/server.test.ts` | **255** | 245 | un `const GARDE_OUVERTE` et un argument dans `beforeEach` (~10) | aucune |
| `plateforme/src/signaling/relais.ts` | **219** | 281 | l'appel de garde, le refus typé, la fermeture du socket (~35) | aucune |
| `plateforme/src/signaling/resilience.test.ts` | **181** | 319 | un jeton dans `connectTo`, une variable dans l'`env` de l'enfant (~10) | aucune |
| `plateforme/src/signaling/trace.test.ts` | **151** | 349 | l'appartenance persistée (~15) | aucune |
| `plateforme/src/base/sous-ensemble.test.ts` | **125** | 375 | rien — il lit le répertoire, `0002` y entre tout seul | aucune |
| `plateforme/src/http/serveur.test.ts` | **107** | 393 | un jeton signé dans le troisième test (~10) | aucune |
| `plateforme/src/depot/session.ts` | **89** | 411 | un paramètre `utilisateurId` (~5) | aucune |
| `plateforme/src/signaling/trace.ts` | **84** | 416 | le passage de l'appartenance (~5) | aucune |
| `plateforme/src/http/serveur.ts` | **82** | 418 | le branchement des routes et de la garde (~25) | aucune |
| `plateforme/src/config.ts` | **61** | 439 | deux variables neuves (~30) | aucune |

**Conclusion, et elle est mesurée** : aucun fichier de `plateforme/` n'approche
le plafond, et **aucune tâche d'extraction n'est nécessaire dans ce paquet**.
C'est l'effet direct du geste de P1 — `appariement.ts` a été extrait **avant**
l'addition d'aujourd'hui, exactement comme `CLAUDE.md` le prescrit depuis D9.

🔴 **UN SEUL fichier de tout le périmètre de P2 porte une marge dangereuse, et
il n'est pas dans `plateforme/` : `client/verify-webrtc.mjs` est à 497 lignes,
marge 3** (relevé par la commande le 19 août 2026 ; `CLAUDE.md` annonçait déjà
ce chiffre au relevé D10, et il n'a pas bougé). La tâche 17 y touche. **Sa porte
est écrite dans la tâche : l'addition doit être à somme NULLE ou NÉGATIVE**, et
si elle ne peut pas l'être, c'est une extraction — jamais une compression.
⚠️ **La divergence de convention que `CLAUDE.md` signale sans la trancher — le
§ « Portée » ne liste pas ce fichier, la commande l'attrape — n'est pas
tranchée ici non plus** : c'est une décision du propriétaire du dépôt, et P2
n'a pas à la prendre au passage. Le plan applique la contrainte la plus stricte
des deux, ce qui est sûr dans les deux lectures.

⚠️ **Porte chiffrée, à jouer après CHAQUE tâche qui touche un fichier existant** :
relancer `wc -l` sur les fichiers de la table ci-dessus. Si l'un d'eux dépasse
**450**, extraire **avant** de continuer, et non après. Le seuil est à 450 et
non à 500 parce que ce dépôt a franchi le plafond **trois fois** au sous-bloc
D10 en croyant avoir de la marge.

### Les règles de méthode, héritées et non négociables

- **Jamais `git add -A`** : nommer les fichiers, un par un. Un `git add -A` a
  déjà emporté le travail concurrent d'une autre tâche dans un commit qui ne
  compilait pas. 🔴 **Et un autre agent travaille dans le même arbre git en ce
  moment** — voir §« Périmètre concurrent » ci-dessous.
- **Aucune tâche de ce plan n'emploie la VM Windows**, ni `scripts/build-agent.sh`,
  ni `run-agent.sh`, ni `winrm.js`, ni `virsh`. Décision de la spec (§4), pas
  commodité. Les pairs `agent` et `client` sont **simulés par des sockets
  scriptés**.
- **Un contrôle qu'on n'a jamais vu rouge n'est pas un contrôle.** Chaque test
  neuf est exécuté **avant** l'implémentation et **vu échouer**. Chaque tâche
  nomme la mutation qui le rend rouge. ⚠️ **Ce plan est lui-même une source de
  contrôles vacueux** : D10 en a attrapé quatre, dont **trois écrits par son
  propre plan. Le doute porte sur ce document.**
- **Ne jamais fabriquer une sortie de commande.** D10 a attrapé **deux pièces
  fabriquées**, dont une inscrite dans `CLAUDE.md` à l'intérieur même d'une
  correction qui dénonçait une affirmation non étayée. Le mécanisme nommé par
  son auteur : *réutiliser la sortie d'une commande antérieure pour répondre à
  la question d'une AUTRE, sans la relancer.*
- **Un saut est un échec.** `npm run test:postgres` ne se saute pas quand
  l'instance manque : il échoue.
- **Aucun taux ne sera revendiqué.** Chaque énoncé de résultat porte son nombre
  d'exécutions. **Deux exécutions par critère de recette, pas une.**
- **Toute preuve d'une affirmation portée dans `CLAUDE.md` est versée dans git.**
  D10 a établi par la commande que l'espace de travail de D9 (`.superpowers/sdd/`,
  gitignoré) **a disparu**, emportant six constats de revue définitivement
  perdus. Une preuve qui vit dans un rapport gitignoré n'existe pas.

### Périmètre concurrent — à lire avant de toucher `client/`

Un agent conduit le sous-bloc **D11** dans le même arbre. Relevé par la commande
le 19 août 2026 (`grep -n "client/src" docs/superpowers/plans/2026-08-19-multifenetres-solder-les-legs.md`),
**il modifie `client/src/main.ts` et `client/src/resize.test.ts`**.

**P2 ne touche NI l'un NI l'autre**, et c'est une contrainte de conception, pas
un hasard — voir la divergence **E7**, qui explique comment `SessionOptions`
gagne son jeton sans que `main.ts` bouge d'une ligne. Les fichiers `client/` que
P2 touche sont : `src/webrtc.ts`, `src/shell-page.ts`, `vite.config.ts`,
`verify-webrtc.mjs`, `recette/paire-candidats.mjs`, `recette/harness.mjs`, plus
des fichiers neufs. ⚠️ **Aucun d'eux n'apparaît dans la liste « Modifiés » du
plan D11** (relevée aux lignes 187-191 de ce plan), mais **la vérification
appartient à l'exécutant** : relancer ce `grep` avant de commencer la famille 4,
parce que le plan D11 peut avoir changé depuis.

⛔ **Interdits absolus pour toute tâche de ce plan** : `agent/`, `proto/`,
`src/`, `web/`, `index.js`, `docker-compose.yml`, `scripts/run-agent.sh`,
`docs/superpowers/*multifenetres*`, et `client/src/main.ts`.

---

## Divergences relevées entre la spec, le code réel de P1, et ce que P2 doit faire

Onze points, trouvés en lisant le code que P2 modifie. Ils sont numérotés
**E1…E11** et non D7… : P1 a employé `D1…D6`, et réutiliser cette suite
mélangerait deux inventaires. Chacun est tranché ici, avec son coût, pour
qu'aucune tâche ne le redécouvre à l'exécution.

### E1 — 🔴 La garde CASSE trois fichiers de test livrés par P1, et il faut le dire d'avance

Relevé par la commande (`grep -rn "role: 'client'\|role, session" plateforme/src`) :
**trois** fichiers déclarent un pair `client` **sans jeton**, et passent tous
par `createSignalingServer` :

| Fichier | Ligne | Ce qu'il fait |
| --- | --- | --- |
| `plateforme/src/signaling/server.test.ts` | 7-15 (`connect`), 39-40 (`beforeEach`) | les **12** tests de non-régression du critère ① de P1 |
| `plateforme/src/http/serveur.test.ts` | 91 | le troisième test, « sert le relais sur le chemin racine » |
| `plateforme/src/signaling/resilience.test.ts` | 85-93 (`connectTo`), 136/150/155 | le processus enfant réel |

**Aucun d'eux ne peut rester vert si la garde refuse un client sans jeton.**

**Ce qu'on pourrait faire et qu'on NE FERA PAS** : rendre la garde optionnelle,
avec pour défaut « accepter ». Les trois fichiers resteraient verts sans une
ligne de changement — et un service mal câblé n'authentifierait plus personne
**sans qu'aucun test ne rougisse**. C'est exactement la panne muette contre
laquelle `plateforme/src/http/serveur.ts:27-31` argumente déjà pour la base :
« `base` est REQUISE, jamais optionnelle ».

**Tranché** : la garde est un **paramètre REQUIS** de `createSignalingServer`,
posé en **deuxième** position (`(portOuWss, garde, trace?)`). Les trois fichiers
changent, et le critère de P1 est repris dans sa formulation la plus stricte qui
soit tenable, **celle que P1 a lui-même dû adopter en D1** :

> **Aucune ASSERTION ne change. Le harnais change du minimum strictement
> nécessaire, hunk par hunk, et chaque hunk est prouvé par un `git diff` versé,
> jamais par une impression.**

Les hunks prévus, à confronter au `diff` réel de la tâche 12 :

- `server.test.ts` : un `const GARDE_OUVERTE` local **au fichier de test**, et
  `createSignalingServer(0, GARDE_OUVERTE)` dans `beforeEach` ;
- `serveur.test.ts` : le troisième test signe un jeton et l'envoie ;
- `resilience.test.ts` : `PLATEFORME_SECRET_JETON` dans le bloc `env` de
  l'enfant (déjà explicite depuis P1, l. 47-53), et `connectTo` porte un jeton.

⚠️ **`GARDE_OUVERTE` vit dans le fichier de test et n'est JAMAIS exporté par du
code de production.** La garantie qu'aucune garde permissive n'existe en
production n'est pas déclarative : la seule fabrique de garde exige un secret,
et `PLATEFORME_SECRET_JETON` **n'a aucun défaut** (tâche 3) — donc il n'existe
aucun chemin qui produise une garde ouverte hors d'un test.

### E2 — 🔴 P2 ne ferme que LA MOITIÉ du trou `ice-config`, et le critère ① le dit littéralement

Le trou réel, tel que P1 le laisse : `plateforme/src/signaling/relais.ts:160-170`
envoie `ice-config` à **chaque pair dès qu'il se déclare, agent comme client**,
avec des identifiants valables `DUREE_SECONDES = 86_400` (`ice.ts:16`).

P2 refuse un **client** sans jeton. Il ne refuse **pas** un pair qui se déclare
`{"role":"agent","session":"n-importe-quoi"}` : l'agent Rust n'a pas d'identité
avant P3 (`agent/src/signaling.rs:59`,
`agent/src/superviseur/signalisation.rs:33-38`), et lui en exiger une casserait
le chantier D en cours.

**Tranché** : la fenêtre reste ouverte côté `agent`, **exactement comme la spec
l'annonce** (§4, P2 : « entre P2 et P3, le pair `agent` reste non
authentifié »). Ce plan ajoute deux exigences que la spec n'écrit pas :

1. **le libellé du critère ① est littéral** — « un pair **`client`** non
   authentifié » — et le document de résultats devra dire, sans détour, que
   `role:'agent'` demeure un chemin anonyme vers des identifiants TURN de 24 h ;
2. **le code le dit à l'endroit où on le lira** : le commentaire de tête de
   `relais.ts` porte aujourd'hui « “Aucune authentification” reste VRAI, et le
   restera jusqu'à P2 » — cette phrase devient **fausse en P2** et doit être
   remplacée par la moitié exacte qui reste vraie. C'est un point nommé de la
   revue transverse (tâche 19).

### E3 — « l'appartenance ENREGISTRÉE » de la spec ne dit pas OÙ elle est enregistrée

Le critère ③ de la spec dit : « tant que P3 n'a pas posé le préfixe, ce critère
porte sur l'**appartenance enregistrée** de la session, pas sur son nom ». Deux
lectures sont possibles — en mémoire, ou en base — et elles ne coûtent pas la
même chose.

**La lecture « en base » est impraticable sur ce chemin**, et c'est
démontrable par le code existant : le gestionnaire `message` de `ws` est
**synchrone** (`relais.ts:98`), et
`plateforme/src/signaling/trace.ts:16-25` explique en toutes lettres pourquoi
une promesse rejetée y **abat tout le process Node** — c'est la raison pour
laquelle P1 écrit sa trace **sans l'attendre**. Interroger la base pour décider
d'accepter un pair remettrait exactement ce que P1 a délibérément évité.

**Tranché, en deux étages** :

- **la DÉCISION** est prise par un registre **pur, synchrone, en mémoire** —
  `plateforme/src/signaling/propriete.ts`, sur le patron exact
  d'`appariement.ts` (générique, aucun socket, aucune horloge) ;
- **l'ENREGISTREMENT** est fait par la trace déjà existante : `session.utilisateur_id`,
  colonne créée **vide** par P1 (`0001-socle.sql:64`, commentée « renseignés par
  P2 et P3 »), est renseignée à l'appariement. C'est ce qui rend le mot
  « enregistrée » littéralement vrai, et c'est ce dont P4 aura besoin.

**Coût, nommé et non corrigé** : le registre en mémoire **ne survit pas à un
redémarrage du service**. Après un redémarrage, un nom de session libéré peut
être revendiqué par un autre utilisateur. Ce n'est pas rattrapable en P2 : la
spec §3.1 dit déjà qu'un WebSocket vit dans un processus et un seul, et le
même §6 dit que les sessions média **survivent** au redémarrage — donc l'état de
routage est perdu alors que le média continue. La véritable réponse est le
**préfixe opaque de P3** (spec §3.4), qui rend un nom de session non devinable.

### E4 — la spec ne dit RIEN de CORS, et le client vit sur une autre origine

Relevé : `client/vite.config.ts:4-7` sert le navigateur sur **5173** ;
`plateforme/src/config.ts:42` écoute sur **8080** par défaut. Un `POST` du
navigateur vers `/auth/connexion` est donc **cross-origin**, et sans en-tête
`Access-Control-Allow-Origin` le navigateur refusera de lire la réponse — sans
qu'aucun test côté serveur ne le voie, puisque les tests parlent en `fetch` Node.

**Tranché** : une variable `PLATEFORME_ORIGINE_CLIENT`, **facultative**, et une
fonction **pure** `entetesCors(origineDemandee, origineAutorisee)`.

- **absente → aucun en-tête CORS n'est émis**, donc le navigateur refuse. Le
  défaut est le refus, jamais l'ouverture ;
- **jamais `*`**, sous aucune condition — c'est une assertion de test, pas une
  intention ;
- l'origine demandée doit **égaler** l'origine autorisée pour que l'en-tête
  sorte : on ne renvoie pas l'origine du demandeur sans la comparer.

⚠️ **Elle est facultative, contrairement à `PLATEFORME_HOTE`, et la raison est
dans l'asymétrie des conséquences** : une `PLATEFORME_HOTE` absente produirait
une écoute universelle **silencieuse** (d'où le refus de démarrer) ; une
`PLATEFORME_ORIGINE_CLIENT` absente produit un refus **bruyant du navigateur**,
que l'opérateur voit immédiatement. Refuser de démarrer pour elle casserait le
déploiement de P5, où le proxy inverse met les deux sur la **même** origine et
où aucune valeur n'a de sens.

### E5 — 🔴 le schéma `jeton_rafraichissement` de la spec §5 ne permet PAS de détecter un rejeu

La spec §5 pose
`jeton_rafraichissement(id, utilisateur_id, empreinte, expire_a, revoque_a)` et
la spec §3.5 exige un rafraîchissement « rotatif à chaque emploi ». Ces deux
phrases ne suffisent pas : **rien, dans ces cinq colonnes, ne relie un jeton
tourné à son successeur.** Sans ce lien, la présentation d'un jeton déjà tourné
ne peut produire qu'une révocation **d'une seule ligne** — celle qui est déjà
révoquée —, ce qui ne protège rien : le voleur qui a tourné le premier garde son
jeton neuf.

**Tranché** : deux colonnes de plus, **`famille TEXT NOT NULL`** et
**`remplace_par TEXT NULL`**, et la règle de rejeu suivante :

> Présenter un jeton dont la ligne porte `revoque_a` non nul est un **REJEU** :
> toute la **famille** est révoquée d'un coup, et le refus est typé `rejeu`.

🔴 **Ces deux colonnes NAISSENT AVEC LA TABLE, elles ne sont pas ajoutées plus
tard, et c'est la leçon D3 de P1 appliquée** : `ALTER TABLE … ADD CONSTRAINT`
est refusé par SQLite (mesuré par P1 : `near "CONSTRAINT": syntax error`). Toute
contrainte de `jeton_rafraichissement` — dont sa clé étrangère vers
`utilisateur(id)` — doit être posée dans `0002`, ou n'exister jamais.

### E6 — 🔴 P2 rend TROIS outils de recette du chantier D incapables de se connecter

C'est la question posée explicitement au plan, et la réponse est **oui**.

Relevé par la commande le 19 août 2026 : `client/verify-webrtc.mjs:30` pilote un
navigateur vers `http://localhost:5173/?session=demo`, donc à travers
`client/src/main.ts` → `connectSession` → la poignée de main
`{role:'client', …}` de `client/src/webrtc.ts:193`. Après P2, **sans jeton, la
session est refusée et le socket fermé**. Les mêmes lignes existent dans
`client/recette/paire-candidats.mjs:187,201` et
`client/recette/harness.mjs:129,149`.

**Ce que nous refusons de faire** : ajouter un interrupteur permissif
(`PLATEFORME_AUTH=0`) pour les faire repasser. Ce serait un défaut d'écriture,
pas une commodité — le dépôt entier est écrit contre ce geste.

**Tranché** : les trois outils **sèment un jeton** dans `localStorage` avant
navigation, en réutilisant le `Page.addScriptToEvaluateOnNewDocument` **qu'ils
appellent déjà** (`verify-webrtc.mjs:186`, `paire-candidats.mjs:187`,
`harness.mjs:129`). C'est la tâche 17. ⚠️ **Le jeton est obtenu de la plateforme
par `/auth/connexion`, jamais forgé dans le script** : un script qui forgerait
un jeton porterait le secret de signature, ce qui déplacerait le trou au lieu de
le fermer.

⚠️ **`Page.addScriptToEvaluateOnNewDocument` ne court PAS sur une page ouverte
par `window.open`** — piège mesuré au sous-bloc D5. Les trois outils naviguent
directement, donc ils ne sont pas concernés ; **mais la page-shell ouvre bien ses
fenêtres par `window.open` (`client/src/shell-page.ts:19`)**, et c'est pourquoi
le jeton doit vivre dans `localStorage` (partagé entre onglets de même origine)
et non dans une variable injectée.

### E7 — `SessionOptions` ne peut pas gagner un champ requis sans toucher `main.ts`

`client/src/webrtc.ts:7-13` déclare `SessionOptions`, et son unique appelant
est `client/src/main.ts:113-115`. Ajouter un champ **requis** obligerait à
modifier `main.ts` — **fichier que le sous-bloc D11 concurrent modifie**.

**Tranché** : `SessionOptions` gagne `jeton?: string`, **facultatif**, et
`connectSession` retombe sur `jetonAcces()` de `client/src/jeton.ts` quand il
est absent. **`main.ts` n'est PAS touché par P2.**

Deux raisons, dans cet ordre : ① il n'existe alors qu'**un seul** lecteur du
stockage dans tout le client, ce qui est meilleur en soi ; ② la collision avec
D11 est évitée. **Le coût est nommé** : `webrtc.ts` gagne une dépendance à un
global de navigateur, ce qui l'éloigne un peu plus de la pureté — mais il
manipule déjà `WebSocket` et `RTCPeerConnection`, et `jeton.ts`, lui, reste pur
(son `Storage` est injecté).

⚠️ **Conséquence produit à assumer, et à ne pas découvrir** : une page de
session ouverte **sans** jeton n'est pas redirigée — elle affiche un refus. La
redirection vers l'écran de connexion vit dans **`shell-page.ts`**, qui est
l'entrée réelle de l'utilisateur. Une page de session est toujours ouverte par
la shell, sur la même origine, donc le jeton y est déjà.

### E8 — `scrypt` : le paramètre qu'on croirait meilleur est REFUSÉ par défaut

Mesuré le 19 août 2026 (relevés ② et ③ ci-dessus) : `N = 16384, r = 8, p = 1`
coûte **29 ms** et passe ; `N = 32768` **échoue** sur
`memory limit exceeded`, parce que `128 · N · r` franchit le `maxmem` par
défaut de 32 MiB.

**Tranché** : `N = 16384, r = 8, p = 1, longueur = 32`, `maxmem` **non
touché** — parce que le relever serait un réglage non mesuré de plus. ⚠️ **Ces
paramètres NE SONT PAS CALIBRÉS**, et ils rejoignent la liste déjà longue du
dépôt (`BPP_MIN`, `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`, `HYSTERESIS`,
`TAILLE_MAX_SORTIE`, `DUREE_SECONDES = 86 400`). **Les 29 ms sont une mesure sur
une machine, pas un objectif atteint** : aucun objectif n'a été posé.

C'est précisément pourquoi la colonne stocke `scrypt$N$r$p$sel$empreinte` : le
jour où ces paramètres seront calibrés, un re-hachage à la connexion suivante
suffira, **sans migration de données**.

### E9 — `timingSafeEqual` LÈVE sur des longueurs différentes

Mesuré (relevé ④) : `Input buffers must have the same byte length`.

C'est un piège réel et pas théorique : une empreinte tronquée en base — par une
colonne trop courte, une écriture partielle, ou un format d'une version
antérieure — ferait **lever** la vérification de mot de passe au lieu de rendre
`false`. L'appelant HTTP répondrait `500` là où il doit répondre `401`, et
l'écart de comportement serait à lui seul un oracle.

**Tranché** : partout où `timingSafeEqual` est employé (mot de passe **et**
signature JWT), les longueurs sont comparées **d'abord**, et une longueur qui
diffère rend un refus, jamais une exception. C'est une assertion de test dans
les tâches 1 et 2.

### E10 — trois modules de P2 sont absents de l'arborescence de la spec §5

La spec §5 prévoit `http/routes-auth.ts`, `depot/utilisateur.ts`,
`depot/jeton.ts`, `identite/mot-de-passe.ts`, `identite/jeton.ts`,
`identite/garde.ts`. **Elle ne prévoit pas** `signaling/propriete.ts` (E3),
`http/cors.ts` (E4), ni `admin/creer-utilisateur.ts` (E11).

**Tranché** : les trois sont créés, et la divergence est déclarée ici plutôt que
découverte. Aucun ne dépasse 200 lignes attendues ; aucun ne modifie une
décision de la spec.

### E11 — « création de compte par ligne de commande » n'a ni point d'entrée ni convention

La spec §4 P2 dit « création de compte **par ligne de commande
d'administration** (pas d'inscription publique en v1) » et s'arrête là.

**Tranché** : `plateforme/src/admin/creer-utilisateur.ts`, lancé par
`npm run admin:utilisateur -- --email <adresse>`, **le mot de passe étant lu sur
l'entrée standard et JAMAIS sur la ligne de commande** — `ps` expose l'argv de
tout processus à tout utilisateur de la machine. Un `--mot-de-passe` sur l'argv
est **refusé explicitement**, avec le motif ; c'est une assertion de test, et sa
rouge est de l'accepter.

---

## Structure des fichiers

**Créés :**

| Fichier | Responsabilité | Taille visée |
| --- | --- | --- |
| `plateforme/src/identite/mot-de-passe.ts` | `scrypt`, format `scrypt$N$r$p$sel$empreinte` — **PUR hors l'appel KDF** | ~110 |
| `plateforme/src/identite/mot-de-passe.test.ts` | dont la rouge de l'algorithme inconnu et celle de l'empreinte tronquée | ~120 |
| `plateforme/src/identite/jeton.ts` | JWT HS256 — **PUR, horloge injectée** | ~140 |
| `plateforme/src/identite/jeton.test.ts` | dont la rouge d'`alg:none` et celle de l'expiration | ~150 |
| `plateforme/src/identite/garde.ts` | la règle de poignée de main — **PURE, synchrone** | ~120 |
| `plateforme/src/identite/garde.test.ts` | les critères ①, ② et ③ au niveau de la règle | ~140 |
| `plateforme/src/signaling/propriete.ts` | le registre d'appartenance — **PUR**, patron d'`appariement.ts` | ~70 |
| `plateforme/src/signaling/propriete.test.ts` | | ~70 |
| `plateforme/src/depot/utilisateur.ts` | créer, lire par courriel, re-hacher | ~90 |
| `plateforme/src/depot/jeton.ts` | émettre, tourner, révoquer une famille — **transactionnel** | ~170 |
| `plateforme/src/depot/utilisateur.test.ts` | la suite de dépôt, jouée contre le pilote courant (double passe) | ~110 |
| `plateforme/src/depot/jeton.test.ts` | idem, dont la **détection de rejeu** | ~160 |
| `plateforme/src/http/cors.ts` | **PUR** — jamais `*` | ~60 |
| `plateforme/src/http/cors.test.ts` | | ~60 |
| `plateforme/src/http/routes-auth.ts` | `POST /auth/connexion`, `POST /auth/rafraichir` | ~180 |
| `plateforme/src/http/routes-auth.test.ts` | dont le critère ④ | ~200 |
| `plateforme/src/admin/creer-utilisateur.ts` | la ligne de commande, mot de passe par **stdin seul** | ~110 |
| `plateforme/src/admin/creer-utilisateur.test.ts` | dont la rouge du mot de passe en argv | ~80 |
| `plateforme/src/base/migrations/0002-identite.sql` | `jeton_rafraichissement` + index | ~55 |
| `client/src/jeton.ts` | stockage et fraîcheur du jeton — **PUR, `Storage` injecté, aucun DOM** | ~90 |
| `client/src/jeton.test.ts` | | ~110 |
| `client/src/connexion.ts` | l'écran de connexion, câblage DOM seul | ~90 |
| `client/connexion.html` | l'écran de connexion, **sans direction visuelle** (elle appartient à ⑥) | ~35 |
| `docs/superpowers/plans/2026-08-19-plateforme-p2-resultats.md` | le document de résultats, **versé dans git** | — |
| `docs/superpowers/plans/journaux-plateforme-p2/` | les journaux de recette, **versés dans git** | — |

**Modifiés :**

| Fichier | Ce qui change |
| --- | --- |
| `plateforme/src/config.ts` | `secretJeton` (**aucun défaut**), `origineClient` (facultative) |
| `plateforme/src/config.test.ts` | leurs rouges |
| `plateforme/src/signaling/relais.ts` | la garde en paramètre **requis**, le refus typé, la fermeture du socket |
| `plateforme/src/signaling/server.test.ts` | **harnais seul** — `GARDE_OUVERTE` et `beforeEach` (E1) |
| `plateforme/src/signaling/resilience.test.ts` | **harnais seul** — le secret dans l'`env`, le jeton dans `connectTo` (E1) |
| `plateforme/src/signaling/trace.ts` | l'appartenance passée au dépôt |
| `plateforme/src/signaling/trace.test.ts` | l'appartenance persistée |
| `plateforme/src/http/serveur.ts` | construction de la garde, branchement des routes et de CORS |
| `plateforme/src/http/serveur.test.ts` | le troisième test signe un jeton (E1) |
| `plateforme/src/depot/session.ts` | `ouvrirSession` gagne un `utilisateurId` facultatif |
| `plateforme/src/depot/session.test.ts` | la colonne renseignée |
| `plateforme/src/demarrage.ts` | rien, sauf si la construction de la garde s'y place (voir tâche 12) |
| `plateforme/package.json` | le script `admin:utilisateur`. **`dependencies` INCHANGÉES** |
| `client/src/webrtc.ts` | `jeton?` dans `SessionOptions`, envoyé dans la poignée de main |
| `client/src/shell-page.ts` | le jeton, et la redirection vers l'écran de connexion |
| `client/vite.config.ts` | l'entrée `connexion` |
| `client/verify-webrtc.mjs` | l'amorce sème le jeton (E6) — 🔴 **porte : somme nulle ou négative** |
| `client/recette/paire-candidats.mjs` | idem |
| `client/recette/harness.mjs` | idem |
| `CLAUDE.md` | la section P2 (tâche 20) |

**Jamais touchés par P2 :** `agent/`, `proto/`, `src/`, `web/`, `index.js`,
`docker-compose.yml`, `scripts/run-agent.sh`, `scripts/verify-all.sh` —
`plateforme` y est déjà couvert par ses trois étapes, il n'y a **aucune étape à
ajouter**, et l'écrire évite qu'un successeur en cherche une —, et
🔴 **`client/src/main.ts`** (E7, périmètre D11).

---

## Interfaces partagées

Ces signatures sont **fixées ici** ; les tâches les consomment telles quelles.

```ts
// plateforme/src/config.ts — DEUX champs de plus
export interface Config {
    hote: string;
    port: number;
    base: 'sqlite' | 'postgres';
    urlBase: string;
    /// PLATEFORME_SECRET_JETON — AUCUN défaut, et 32 caractères au moins.
    secretJeton: string;
    /// PLATEFORME_ORIGINE_CLIENT — facultative. Absente : aucun en-tête CORS.
    origineClient?: string;
}

// plateforme/src/identite/mot-de-passe.ts
export interface ParametresScrypt { N: number; r: number; p: number; }
export const PARAMETRES_COURANTS: ParametresScrypt;   // N=16384, r=8, p=1 (E8)
/// Rend `scrypt$N$r$p$sel$empreinte`, sel et empreinte en base64url.
export function hacher(motDePasse: string, params?: ParametresScrypt): Promise<string>;
/// `false` sur mot de passe faux OU empreinte malformée. LÈVE sur un
/// algorithme inconnu — un `false` silencieux y serait indiscernable d'un
/// mauvais mot de passe, et personne ne saurait diagnostiquer.
export function verifier(motDePasse: string, encode: string): Promise<boolean>;
/// Le format porte son algorithme et ses paramètres : c'est ce qui permettra
/// de passer à Argon2id par re-hachage, sans migration de données (spec §3.5).
export function doitEtreRehache(encode: string): boolean;
export function analyser(encode: string): { algo: string; params: ParametresScrypt; sel: Buffer; empreinte: Buffer };

// plateforme/src/identite/jeton.ts — PUR, horloge en PARAMÈTRE
export type MotifJeton = 'forme' | 'algorithme' | 'signature' | 'expire';
export type VerdictJeton = { ok: true; sujet: string } | { ok: false; motif: MotifJeton };
export const DUREE_JETON_ACCES_MS: number;     // 10 min — NON CALIBRÉE
export const LONGUEUR_SECRET_MIN: number;      // 32 — NON CALIBRÉE
export function signer(sujet: string, secret: string, maintenant: number, dureeMs?: number): string;
export function verifierJeton(jeton: unknown, secret: string, maintenant: number): VerdictJeton;

// plateforme/src/signaling/propriete.ts — PUR
export class ProprieteDeSession {
    proprietaire(session: string): string | undefined;
    revendiquer(session: string, utilisateurId: string): void;
    liberer(session: string): void;
}

// plateforme/src/identite/garde.ts — PURE, SYNCHRONE, sans base
export type MotifRefus =
    | 'jeton-absent' | 'jeton-invalide' | 'jeton-expire' | 'session-refusee';
export type Verdict =
    | { ok: true; utilisateurId?: string }
    | { ok: false; motif: MotifRefus; message: string };
export interface Garde {
    /// SANS EFFET DE BORD : elle décide, elle n'inscrit rien.
    verifier(poignee: { role: Role; session: string; jeton?: unknown }): Verdict;
    /// Appelée APRÈS que `Appariement::declarer` a accepté, et seulement alors.
    revendiquer(session: string, utilisateurId: string | undefined): void;
    liberer(session: string): void;
}
export function garde(
    secret: string,
    maintenant: () => number,
    proprietes: ProprieteDeSession,
): Garde;

// plateforme/src/depot/utilisateur.ts
export interface LigneUtilisateur { id: string; email: string; empreinte_mdp: string; cree_a: number; }
export function creerUtilisateur(p: Pilote, email: string, empreinteMdp: string, maintenant: number): Promise<string>;
export function lireParEmail(p: Pilote, email: string): Promise<LigneUtilisateur | undefined>;
export function remplacerEmpreinte(p: Pilote, id: string, empreinte: string): Promise<void>;

// plateforme/src/depot/jeton.ts
export type MotifRafraichissement = 'inconnu' | 'expire' | 'rejeu' | 'revoque';
export type IssueRotation =
    | { ok: true; clair: string; utilisateurId: string }
    | { ok: false; motif: MotifRafraichissement };
export const DUREE_RAFRAICHISSEMENT_MS: number;   // 30 j — NON CALIBRÉE
/// Ouvre une famille NEUVE (connexion) ; rend le jeton EN CLAIR, une seule fois.
export function emettre(p: Pilote, utilisateurId: string, maintenant: number): Promise<string>;
/// Tourne : révoque le présenté, en émet un neuf dans la MÊME famille.
/// Un jeton déjà révoqué est un REJEU : toute la famille est révoquée (E5).
export function tourner(p: Pilote, clair: string, maintenant: number): Promise<IssueRotation>;
export function revoquerFamille(p: Pilote, famille: string, maintenant: number): Promise<number>;

// plateforme/src/http/cors.ts — PUR
export function entetesCors(
    origineDemandee: string | undefined,
    origineAutorisee: string | undefined,
): Record<string, string> | undefined;

// plateforme/src/http/routes-auth.ts
export interface DependancesAuth {
    base: Pilote;
    secretJeton: string;
    origineClient?: string;
    maintenant: () => number;
}
/// Rend `true` si la requête a été servie, `false` si elle ne concerne pas
/// l'authentification — le serveur répond alors 404, comme aujourd'hui.
export function servirAuth(req: IncomingMessage, rep: ServerResponse, deps: DependancesAuth): Promise<boolean>;

// plateforme/src/signaling/relais.ts — la garde en DEUXIÈME position, REQUISE
export function createSignalingServer(port: number, garde: Garde, trace?: ObservateurDeSession): SignalingServer;
export function createSignalingServer(wss: WebSocketServer, garde: Garde, trace?: ObservateurDeSession): SignalingServer;

// plateforme/src/signaling/relais.ts — l'observateur porte l'appartenance
export interface ObservateurDeSession {
    apparie(nomSession: string, utilisateurId?: string): void;
    separe(nomSession: string): void;
}

// plateforme/src/depot/session.ts — un paramètre de plus, FACULTATIF
export function ouvrirSession(p: Pilote, nomSession: string, maintenant: number, utilisateurId?: string): Promise<string>;

// client/src/jeton.ts — PUR, aucun DOM, `Storage` injecté
export interface Coffre { getItem(c: string): string | null; setItem(c: string, v: string): void; removeItem(c: string): void; }
export interface Paire { acces: string; rafraichissement: string; }
export function poser(coffre: Coffre, paire: Paire): void;
export function vider(coffre: Coffre): void;
export function jetonAcces(coffre?: Coffre): string | undefined;
/// Lit `exp` de la charge SANS vérifier la signature — le navigateur ne
/// vérifie jamais, seul le serveur le fait. Documenté dans le fichier.
export function expireAvant(jeton: string, instant: number): boolean;
export function rafraichirSiNecessaire(
    coffre: Coffre,
    maintenant: number,
    margeMs: number,
    appel: (corps: unknown) => Promise<{ acces: string; rafraichissement: string } | undefined>,
): Promise<boolean>;
```

⚠️ **`Garde.verifier` est SYNCHRONE et sans base, et ce n'est pas un détail de
confort** : le gestionnaire `message` de `ws` est synchrone
(`relais.ts:98`), et `trace.ts:16-25` explique pourquoi une promesse rejetée y
abat tout le process Node. **Une signature qui rendrait une promesse inviterait
un appelant à l'attendre**, exactement ce que P1 a interdit pour la trace.

⚠️ **La distinction `verifier` / `revendiquer` n'est pas cosmétique** : la
revendication ne doit avoir lieu **qu'après** que `Appariement::declarer` a
accepté, sinon un pair refusé pour cause de rôle déjà occupé laisserait une
appartenance derrière lui. Les deux appels sont dans le même bloc synchrone : il
n'y a pas de course entre eux, et c'est ce qui autorise à les séparer.

---

## Ordre et parallélisme

| Famille | Tâches | Dépend de | Parallélisable |
| --- | --- | --- | --- |
| 0 — le socle d'identité, **pur** | 1, 2, 3 | rien | **les trois entre elles** |
| 1 — la persistance | 4, 5, 6 | 4 ← rien ; 5 et 6 ← 4 | 5 et 6 entre elles |
| 2 — les routes HTTP | 7, 8, 9 | 7 ← rien ; 8 ← 1,2,3,5,6,7 ; 9 ← 1,5 | 7 avec la famille 0 ; 9 avec 8 |
| 3 — la garde du signaling | 10, 11, 12, 13 | 10 ← rien ; 11 ← 2,10 ; 12 ← 11,3 ; 13 ← 12 | 10 avec la famille 0 |
| 4 — le navigateur | 14, 15, 16, 17 | 14 ← rien ; 15 et 16 ← 14 ; 17 ← 8, 16 | 14 avec la famille 0 ; 15 et 16 entre elles |
| 5 — recette et clôture | 18, 19, 20, 21 | 18 ← tout ; 19, 20, 21 ← 18 | 19 et 20 entre elles |

**Chemin critique** : 2 → 11 → 12 → 13 → 18. **Cinq tâches purement isolées
peuvent démarrer ensemble au premier tour** : 1, 2, 7, 10, 14.

⚠️ **La tâche 12 est la seule qui touche des fichiers de test livrés par P1**
(E1). Elle ne se parallélise avec rien, et son `git diff` est une pièce du
document de résultats.

---

# Famille 0 — le socle d'identité, pur et sans réseau

### Task 1 : `scrypt`, et un format qui porte son propre algorithme

**Objet :** hacher et vérifier un mot de passe, dans un format préfixé par son
algorithme et ses paramètres, pour qu'un changement d'algorithme se fasse par
re-hachage et jamais par migration de données.

**Files:**
- Create: `plateforme/src/identite/mot-de-passe.ts`, `plateforme/src/identite/mot-de-passe.test.ts`
- Modify: aucun

**Interfaces:** Produces `hacher`, `verifier`, `doitEtreRehache`, `analyser`,
`PARAMETRES_COURANTS` (§« Interfaces partagées »). Consumes rien.

- [ ] **Step 1 : écrire les tests d'abord, et les voir ROUGES**

Sept tests, chacun avec l'état qui le rend rouge :

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `hacher` rend une chaîne de la forme `scrypt$16384$8$1$<sel>$<empreinte>` | rendre l'empreinte nue, sans préfixe : la forme ne correspond plus |
| deux hachages du **même** mot de passe **diffèrent** | fixer le sel au lieu de le tirer de `randomBytes` |
| `verifier` rend `true` sur le bon mot de passe | — (le test de base) |
| `verifier` rend `false` sur un mot de passe faux | comparer les longueurs seulement |
| 🔴 `verifier` rend `false`, **sans lever**, sur une empreinte tronquée | appeler `timingSafeEqual` sans comparer les longueurs d'abord : **mesuré, il lève `Input buffers must have the same byte length`** (relevé ④). C'est la rouge la plus utile du fichier |
| 🔴 `verifier` **LÈVE** sur `argon2id$…` (algorithme inconnu) | rendre `false` : le refus devient indiscernable d'un mot de passe faux, et personne ne diagnostique |
| `doitEtreRehache` rend `true` sur un `N` inférieur au courant, `false` sur le courant | rendre toujours `false` |

⚠️ **Le test de l'empreinte tronquée doit être écrit avec une empreinte
réellement plus courte, pas avec une chaîne vide** : une chaîne vide pourrait
être attrapée par une validation de forme en amont et ne jamais atteindre
`timingSafeEqual`. **Il ne mesurerait alors pas ce qu'il annonce.**

⚠️ **Les valeurs employées dans les tests sont RÉALISTES**, jamais commodes :
un mot de passe de longueur ordinaire, un sel de 16 octets, une empreinte de
32 octets. C'est la leçon la plus chère de P1 — *une suite qui n'écrit que des
`1_000` déclare portable un schéma qui refuse toute écriture réelle*.

- [ ] **Step 2 : implémenter**

`PARAMETRES_COURANTS = { N: 16384, r: 8, p: 1 }`, longueur de clé 32.

🔴 **Le commentaire de tête PORTE le relevé ③, verbatim**, parce qu'un
successeur qui voudra durcir `N` le rencontrera :

```
// Mesuré le 19 août 2026 sur ce Node (v24.9.0) :
//     N=16384 r=8 p=1 : OK 29 ms
//     N=32768 r=8 p=1 : REFUSE -> Invalid scrypt params:
//         error:030000AC:digital envelope routines::memory limit exceeded
// La limite est celle de `maxmem` (32 MiB par défaut), franchie dès que
// 128·N·r la dépasse. Le message ne parle PAS de N. Durcir le paramètre exige
// donc de lever `maxmem` explicitement — geste qui n'a PAS été fait ici, faute
// de mesure qui le justifie. Ces paramètres NE SONT PAS CALIBRÉS : les 29 ms
// sont une mesure, pas un objectif atteint.
```

`verifier` : `analyser` d'abord ; si `algo !== 'scrypt'` → `throw`. Puis
recalculer, **comparer les longueurs**, et seulement alors `timingSafeEqual`.

- [ ] **Step 3 : voir vert, et relever**

```bash
cd plateforme && npx vitest run src/identite/mot-de-passe.test.ts
```

Le compte de tests attendu est **7**. ⚠️ **L'annoncer AVANT de le lire** : c'est
ainsi que D10 a rattrapé un test supprimé par un `Write` d'écrasement.

---

### Task 2 : le JWT HS256, écrit avec `node:crypto` et rien d'autre

**Objet :** signer et vérifier un jeton d'accès court, sans aucune dépendance,
avec une horloge en paramètre.

**Files:**
- Create: `plateforme/src/identite/jeton.ts`, `plateforme/src/identite/jeton.test.ts`
- Modify: aucun

**Interfaces:** Produces `signer`, `verifierJeton`, `DUREE_JETON_ACCES_MS`,
`LONGUEUR_SECRET_MIN`.

- [ ] **Step 1 : les tests d'abord, ROUGES**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| un jeton signé se relit : `sujet` retrouvé | — |
| 🔴 `alg: 'none'` est **refusé** (`motif: 'algorithme'`) | lire l'`alg` de l'en-tête et s'y fier : le jeton forgé passe. **C'est la vulnérabilité JWT la plus classique, et elle est gratuite à éprouver** |
| 🔴 `alg: 'RS256'` est refusé de même | idem |
| une signature d'un octet modifié est refusée (`'signature'`) | comparer les chaînes avec `===` sans longueur, ou ne pas comparer du tout |
| 🔴 **l'expiration, avec une horloge qui VARIE** : signé à `t0` pour `d` ms, accepté à `t0 + d/2`, **refusé à `t0 + d`** et à `t0 + d + 1` | ignorer `exp` : les deux dernières assertions tombent. ⚠️ **Le test emploie TROIS instants distincts** — figer l'horloge le rendrait inerte, et c'est littéralement ce que la colonne ROUGE du critère ② de la spec interdit |
| une forme invalide (deux segments, base64 illisible, charge non-objet) est refusée (`'forme'`), **jamais levée** | laisser `JSON.parse` lever : l'appelant HTTP répond 500 au lieu de 401 |
| 🔴 `signer` **LÈVE** sur un secret de moins de `LONGUEUR_SECRET_MIN` | accepter un secret de 3 caractères : une plateforme se déploierait avec un secret devinable |

- [ ] **Step 2 : implémenter, sans dépendance**

La faisabilité est **mesurée** (relevé ⑤) : en-tête et charge en `base64url`,
signature `createHmac('sha256', secret).update(`${h}.${c}`).digest('base64url')`,
43 caractères.

Règles non négociables, écrites dans le fichier :

- l'`alg` de l'en-tête est **comparé à `'HS256'`**, jamais employé pour choisir
  un algorithme. On ne dérive pas un comportement d'une donnée non signée ;
- la comparaison de signature passe par `timingSafeEqual`, **après** comparaison
  des longueurs (E9, relevé ④) ;
- l'expiration est franche : `maintenant >= exp` **refuse**. La borne est écrite,
  pour que le test puisse l'assiéger des deux côtés ;
- `exp` est en **millisecondes**, comme tout horodatage de ce service — et non
  en secondes comme le veut la RFC 7519. ⚠️ **C'est une divergence délibérée
  avec le standard**, prise pour n'avoir qu'une seule unité de temps dans tout
  le paquet ; le jeton n'est lu que par ce service, jamais par un tiers. **Elle
  est écrite dans le fichier**, sans quoi le prochain lecteur croira à un bug.

- [ ] **Step 3 : vert, compte attendu 7**

---

### Task 3 : `config.ts` gagne deux variables, dont une SANS DÉFAUT

**Objet :** lire `PLATEFORME_SECRET_JETON` (obligatoire, longueur minimale) et
`PLATEFORME_ORIGINE_CLIENT` (facultative), au même endroit et une seule fois.

**Files:**
- Modify: `plateforme/src/config.ts` (61 l., marge 439), `plateforme/src/config.test.ts` (35 l.)

**Interfaces:** Produces `Config.secretJeton`, `Config.origineClient`.

- [ ] **Step 1 : les tests, ROUGES**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| 🔴 `lireConfig` **lève** sans `PLATEFORME_SECRET_JETON` | poser un défaut, fût-il aléatoire : un secret aléatoire au démarrage invaliderait tous les jetons à chaque redémarrage **et** passerait ce test. Le défaut doit être **l'absence de défaut** |
| `lireConfig` lève sur une valeur **vide** | employer `??`, qui ne s'applique pas à la chaîne vide. ⚠️ **P1 a payé exactement cette erreur** : sa tâche 1 annonçait deux tests rouges, un seul l'était, parce que `env.X ?? 'defaut'` ne rattrape pas `''` |
| 🔴 `lireConfig` lève sur un secret trop court | accepter n'importe quelle longueur |
| `origineClient` absente rend `undefined`, sans lever | lever : le déploiement de P5, où les deux origines sont la même, deviendrait impossible |

- [ ] **Step 2 : implémenter, puis constater le dommage collatéral**

⚠️ **Ajouter un champ requis à `Config` casse le typage de tous les littéraux
`Config` du paquet.** Relevé par la commande le 19 août 2026 : ils sont dans
`plateforme/src/http/serveur.test.ts:18-23` et
`plateforme/src/index.test.ts:41-47,70-75`. C'est **voulu** : `tsc --noEmit`
les désigne un par un, ce qu'aucune valeur par défaut n'aurait fait.

```bash
cd plateforme && npm run typecheck   # DOIT échouer, et nommer chaque site
```

Les compléter **avec un secret de test explicite**, jamais avec `''`.

- [ ] **Step 3 : `npm run typecheck` sortie 0, et `npm run test:sqlite` revenu à 64 + les neufs**

---

# Famille 1 — la persistance de l'identité

### Task 4 : la migration `0002`, et les DEUX gardes de P1 qui doivent la voir

**Objet :** créer `jeton_rafraichissement` avec **toutes** ses contraintes, et
vérifier qu'elle traverse le lint statique et les deux moteurs.

**Files:**
- Create: `plateforme/src/base/migrations/0002-identite.sql`
- Modify: aucun — 🔴 **`sous-ensemble.test.ts` lit le répertoire
  (`sous-ensemble.test.ts:61`) : le fichier neuf y entre TOUT SEUL.** Ne rien y
  ajouter serait un oubli si ce n'était pas vrai ; **c'est vrai, et vérifié par
  la commande.**

- [ ] **Step 1 : écrire la migration**

```sql
CREATE TABLE jeton_rafraichissement (
    id             TEXT PRIMARY KEY,
    utilisateur_id TEXT NOT NULL REFERENCES utilisateur(id),
    famille        TEXT NOT NULL,
    empreinte      TEXT NOT NULL,
    remplace_par   TEXT NULL,
    cree_a         BIGINT NOT NULL,
    expire_a       BIGINT NOT NULL,
    revoque_a      BIGINT NULL
);

CREATE UNIQUE INDEX jeton_empreinte ON jeton_rafraichissement(empreinte);
CREATE INDEX jeton_famille ON jeton_rafraichissement(famille);
```

🔴 **Quatre règles de P1 s'appliquent, et chacune est une erreur déjà payée** :

1. **`cree_a`, `expire_a` et `revoque_a` sont `BIGINT`, jamais `INTEGER`.**
   `INTEGER` vaut 4 octets sur Postgres, et `Date.now()` n'y tient pas — le
   service ne pouvait pas appliquer ses **propres** migrations
   (`value "1787136797072" is out of range for type integer`). Le lint refuse
   désormais tout `_a INTEGER` (`sous-ensemble.test.ts:45-48`) ;
2. **la convention de nommage `_a` est ce qui rend ce lint opérant** : une
   colonne d'horodatage nommée autrement y échapperait. Les trois la portent ;
3. **aucune chaîne littérale**, pas même une valeur par défaut
   (`sous-ensemble.test.ts:83-88`) — `rendreMarqueurs` lèverait côté Postgres ;
4. **la clé étrangère naît avec la table**, ou n'existe jamais : SQLite ne sait
   pas l'ajouter par `ALTER TABLE` (mesuré par P1 : `near "CONSTRAINT": syntax
   error`). C'est aussi pourquoi `famille` et `remplace_par` sont là **dès
   maintenant** (E5) et pas au premier besoin.

⚠️ **`UUID` est un jeton INTERDIT par le lint** (`sous-ensemble.test.ts:29`).
Les commentaires sont dépouillés avant contrôle (`corps()`, l. 57-59), donc le
mot peut apparaître **dans un commentaire** — mais jamais dans le corps SQL.

- [ ] **Step 2 : jouer les DEUX rouges du lint, avant de continuer**

```bash
cd plateforme
# rouge A : un horodatage en INTEGER
sed -i 's/expire_a       BIGINT/expire_a       INTEGER/' src/base/migrations/0002-identite.sql
npx vitest run src/base/sous-ensemble.test.ts   # DOIT échouer
# rouge B : une chaîne littérale
# (poser un DEFAULT textuel, relancer, voir rouge)
```

**Restaurer la source, puis la comparer à l'identique** (`git diff` vide) avant
de passer à l'étape suivante. Verser la sortie des deux rouges.

- [ ] **Step 3 : la double passe, avec des VALEURS RÉALISTES**

```bash
cd plateforme && npm run test:sqlite && npm run test:postgres
```

⚠️ **Les deux passes doivent être jouées, et une seule ne dit rien.** Le
troisième garde de P1 — le **choix des valeurs** — est exercé par la tâche 6,
pas ici : cette tâche-ci ne crée que le schéma.

---

### Task 5 : le dépôt `utilisateur`

**Objet :** créer un compte, le lire par courriel, remplacer son empreinte.

**Files:**
- Create: `plateforme/src/depot/utilisateur.ts`, `plateforme/src/depot/utilisateur.test.ts`
- Modify: aucun

⚠️ **Un fichier de test par dépôt, et non un fichier commun** : les tâches 5 et
6 se parallélisent, et deux tâches qui créent le même fichier se l'écrasent
mutuellement. C'est le piège du `Write` d'écrasement attrapé en D10.

- [ ] **Step 1 : tests rouges, joués contre le pilote courant via `baseNeuve`**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| créer puis relire par courriel rend la même ligne | — |
| 🔴 deux comptes au **même** courriel : le second est **refusé** | l'index `UNIQUE` de `0001-socle.sql:32` existe déjà ; le retirer ferait passer les deux |
| la recherche d'un courriel inconnu rend `undefined`, **jamais une exception** | laisser `lignes[0]` indéfini puis accéder à `.id` |
| 🔴 `cree_a` relu vaut **exactement** la valeur d'époque écrite | employer une petite valeur : **le test ne mesurerait plus rien**. La valeur d'essai est une **époque réelle en millisecondes**, comme `INSTANT_MIGRATION` |
| `remplacerEmpreinte` change l'empreinte et **rien d'autre** | mettre à jour `email` au passage |

- [ ] **Step 2 : implémenter, sans une seule littérale dans le SQL**

Toute valeur en paramètre — c'est ce qui rend `rendreMarqueurs` sûr
(`pilote.ts:17-30`).

- [ ] **Step 3 : les deux passes vertes**

---

### Task 6 : le dépôt `jeton_rafraichissement`, et la DÉTECTION DE REJEU

**Objet :** émettre, tourner, et **traiter la réutilisation d'un jeton déjà
tourné comme une compromission** : révoquer toute la famille.

**Files:**
- Create: `plateforme/src/depot/jeton.ts`, `plateforme/src/depot/jeton.test.ts`

- [ ] **Step 1 : la règle, écrite avant le code**

Le jeton en clair est `randomBytes(32).toString('base64url')` — **256 bits
d'entropie**. Il est stocké **haché en SHA-256**, jamais en clair.

⚠️ **Pourquoi SHA-256 et non `scrypt`, alors que le mot de passe emploie
`scrypt`** : `scrypt` est lent **par conception**, pour rendre coûteuse
l'attaque par dictionnaire d'un secret à faible entropie. Un jeton de 256 bits
tiré au hasard n'a pas de dictionnaire. Le hachage sert ici à ce qu'une fuite de
la base ne rende pas les jetons utilisables, et SHA-256 y suffit. **Le coût est
nommé** : si un jour un jeton de rafraîchissement devenait dérivé d'un secret
humain, cette décision serait à rouvrir.

Rotation, **dans une transaction** (`Pilote.transaction`, éprouvée sur les deux
moteurs par `pilotes.test.ts:84-93`) :

| État de la ligne présentée | Issue |
| --- | --- |
| aucune ligne pour cette empreinte | `{ ok: false, motif: 'inconnu' }` |
| 🔴 `revoque_a` **non nul** | **REJEU** : toute la `famille` est révoquée, y compris le jeton neuf que le voleur détient. `{ ok: false, motif: 'rejeu' }` |
| `expire_a <= maintenant` | `{ ok: false, motif: 'expire' }` |
| sinon | `revoque_a = maintenant`, `remplace_par = <id neuf>`, insertion d'une ligne neuve **de la même famille**, `{ ok: true, clair, utilisateurId }` |

- [ ] **Step 2 : les tests, ROUGES d'abord**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `emettre` rend un clair **différent** à chaque appel, et la base ne porte jamais le clair | stocker le clair : une assertion cherche le clair dans la colonne `empreinte` et le trouve |
| tourner une fois rend un clair **neuf**, et l'ancien ne vaut plus | — |
| 🔴 **tourner DEUX fois le MÊME clair** : le second rend `motif: 'rejeu'` **et** toutes les lignes de la famille portent `revoque_a` | **supprimer** la ligne à la rotation au lieu de la marquer : le second appel rendrait `'inconnu'`, et **la famille survivrait**. C'est la rouge décisive de cette tâche, et les deux issues sont distinguables — c'est ce qui la rend non vacueuse |
| un jeton expiré rend `'expire'`, **avec une horloge qui varie** | figer l'horloge |
| une famille révoquée refuse **aussi** le jeton neuf | ne révoquer que la ligne présentée |
| 🔴 la transaction est annulée si l'insertion neuve échoue : **l'ancien reste valide** | écrire hors transaction — l'utilisateur perdrait sa session sur une panne partielle |
| 🔴 `expire_a` relu vaut **exactement** l'époque écrite, sur les DEUX moteurs | des petites valeurs : le test redevient aveugle au défaut `INTEGER`/`BIGINT` de P1 |

- [ ] **Step 3 : les deux passes**

```bash
cd plateforme && npm run test:sqlite && npm run test:postgres
```

⚠️ **Le test de rejeu DOIT être joué sur les deux moteurs**, et non seulement
sur SQLite : c'est la seule façon de savoir que la transaction se comporte
pareil des deux côtés. P1 a montré qu'une passe verte sur un moteur ne dit rien
de l'autre.

---

# Famille 2 — les routes HTTP

### Task 7 : CORS, et le refus par défaut

**Objet :** une fonction **pure** qui rend les en-têtes CORS, ou rien.

**Files:**
- Create: `plateforme/src/http/cors.ts`, `plateforme/src/http/cors.test.ts`

- [ ] **Step 1 : tests rouges**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| origine autorisée absente → **`undefined`** (aucun en-tête) | rendre `{'Access-Control-Allow-Origin': '*'}` quand rien n'est configuré |
| 🔴 la valeur `*` n'est **jamais** produite, quelle que soit l'entrée | renvoyer `origineDemandee` telle quelle : un attaquant choisit son en-tête |
| origine demandée **différente** de l'autorisée → `undefined` | comparer par `startsWith` ou par inclusion |
| origine demandée **égale** → l'en-tête porte l'origine, plus `Vary: Origin` | omettre `Vary` : un cache intermédiaire servirait la réponse d'une origine à une autre |
| origine demandée absente (requête non navigateur) → `undefined` | renvoyer l'origine autorisée à qui ne l'a pas demandée |

- [ ] **Step 2 : implémenter. Aucune expression régulière, une égalité de chaînes.**

---

### Task 8 : les deux routes d'authentification, et le critère ④

**Objet :** `POST /auth/connexion` et `POST /auth/rafraichir`, branchées dans le
serveur HTTP existant, **sans qu'un mot de passe n'apparaisse jamais dans une
trace ni dans une réponse**.

**Files:**
- Create: `plateforme/src/http/routes-auth.ts`, `plateforme/src/http/routes-auth.test.ts`
- Modify: `plateforme/src/http/serveur.ts` (82 l., marge 418)

**Interfaces:** Consumes `verifier`/`hacher` (T1), `signer` (T2), `Config` (T3),
`lireParEmail` (T5), `emettre`/`tourner` (T6), `entetesCors` (T7).

- [ ] **Step 1 : le contrat, écrit avant le code**

| Route | Corps | Issue |
| --- | --- | --- |
| `POST /auth/connexion` | `{email, motdepasse}` | `200 {acces, rafraichissement, expire_a}` ou `401 {refus:'identifiants'}` |
| `POST /auth/rafraichir` | `{rafraichissement}` | `200 {acces, rafraichissement, expire_a}` ou `401 {refus:<motif>}` |
| `OPTIONS` sur les deux | — | `204` + en-têtes CORS, ou `204` nu si aucune origine n'est autorisée |
| toute autre méthode | — | `405` |
| corps > 4 KiB | — | `413`, **connexion coupée sans lire la suite** |
| corps non-JSON | — | `400 {refus:'forme'}` |

🔴 **Le message de refus est IDENTIQUE pour « courriel inconnu » et « mot de
passe faux »** — `{refus:'identifiants'}`. Un message qui les distingue est un
oracle d'énumération de comptes. **C'est la même règle que le critère ② de P3**,
posée ici parce que le premier cas où elle mord est celui-ci.

⚠️ **Et le coût du chemin est égalisé aussi** : sur un courriel inconnu, la
route **hache quand même** un mot de passe leurre, pour que la durée de réponse
ne trahisse pas l'existence du compte. **Cette égalisation N'EST PAS MESURÉE**,
et ne le sera pas : un test de temporisation serait instable, et la spec §8 range
déjà les attaques temporelles parmi ce que ⑤ n'éprouve pas. **Ce qui EST testé
est le message identique**, qui est décidable.

- [ ] **Step 2 : les tests, ROUGES d'abord — et le critère ④ en fait partie**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| connexion valide rend un `acces` que `verifierJeton` accepte, et un `rafraichissement` utilisable | — |
| mot de passe faux → `401`, message **identique** à celui du courriel inconnu | distinguer les deux messages |
| rafraîchir tourne la paire ; l'ancien rafraîchissement ne vaut plus | — |
| 🔴 rafraîchir **deux fois** le même → `401`, et la famille est révoquée | voir la rouge de la tâche 6 |
| corps de 5 KiB → `413` | ne pas borner : un pair anonyme fait grossir la mémoire à volonté |
| `GET /auth/connexion` → `405` | router sur le chemin seul |
| 🔴 **critère ④** : après une connexion **réussie** et une connexion **échouée**, aucune trace capturée ne porte le **champ** `motdepasse`, `mot_de_passe` ni `empreinte_mdp`, et aucun corps de réponse n'en porte | ajouter un `console.log(JSON.stringify(corps))` dans la route : le test rougit. **C'est la rouge à jouer, et elle est gratuite** |

🔴 **Le critère ④ cherche LE CHAMP, pas la valeur, et c'est une exigence de la
spec.** Un test qui ne chercherait que la chaîne exacte du mot de passe
manquerait un journal du **corps entier** de la requête, où le mot de passe
apparaîtrait pourtant. Le test capture `console.log`, `console.warn` et
`console.error` pendant les deux appels et balaie leur concaténation sur
`/motdepasse|mot_de_passe|empreinte_mdp/i`.

⚠️ **Contrôle du contrôle, à faire une fois** : avant de croire ce test, ajouter
délibérément le `console.log` fautif et **le voir rougir**. Un test de balayage
qui ne capture pas réellement la console est vert quoi qu'il arrive — c'est
exactement le patron du contrôle vacueux, payé quatre fois par ce dépôt.

- [ ] **Step 3 : brancher dans `serveur.ts`**

Le gestionnaire HTTP de `serveur.ts:35-38` répond 404 à tout. Il appelle
désormais `servirAuth` d'abord ; si elle rend `false`, le 404 est conservé
**mot pour mot**. ⚠️ **Ne pas changer le corps du 404** : rien ne le teste
aujourd'hui, et le changer serait un effet de bord non déclaré.

- [ ] **Step 4 : mesurer le fichier**

```bash
wc -l plateforme/src/http/serveur.ts plateforme/src/http/routes-auth.ts
```

Porte à **450**. Attendu : ~110 et ~180.

---

### Task 9 : la création de compte en ligne de commande

**Objet :** créer un utilisateur sans inscription publique, **sans jamais
exposer le mot de passe dans l'argv**.

**Files:**
- Create: `plateforme/src/admin/creer-utilisateur.ts`, `plateforme/src/admin/creer-utilisateur.test.ts`
- Modify: `plateforme/package.json` — le script `admin:utilisateur`.
  🔴 **`dependencies` INCHANGÉES** : `pilote.test.ts:38-50` compare la liste à
  `['pg','ws']` et doit rester vert

- [ ] **Step 1 : tests rouges sur la partie PURE**

`analyserArguments(argv): {email} | {refus}` est pure et testée seule.

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `--email a@b.test` rend l'adresse | — |
| `--email` absent → refus nommé | rendre `undefined` et laisser l'appelant planter |
| 🔴 `--mot-de-passe secret` → **refus explicite**, avec le motif | l'accepter « pour la commodité » : `ps` expose l'argv de tout processus à tout utilisateur de la machine. **C'est la rouge de cette tâche** |
| un courriel vide → refus | accepter la chaîne vide |

- [ ] **Step 2 : le corps impur**

Lecture du mot de passe sur `stdin`, `hacher`, `creerUtilisateur`, puis
**afficher l'identifiant créé et rien d'autre**. Un courriel déjà pris rend un
message clair et un code de sortie non nul — ici l'oracle d'énumération n'existe
pas, l'appelant est l'administrateur.

- [ ] **Step 3 : lancer réellement la commande une fois, sur une base jetable, et verser la sortie**

C'est la seule preuve que le chemin impur tourne. Une sortie **recopiée à la
main serait une pièce fabriquée** — D10 en a attrapé deux.

---

# Famille 3 — la garde du signaling

### Task 10 : le registre d'appartenance, pur

**Objet :** décider, sans base et sans horloge, si un utilisateur peut rejoindre
un nom de session.

**Files:**
- Create: `plateforme/src/signaling/propriete.ts`, `plateforme/src/signaling/propriete.test.ts`

- [ ] **Step 1 : tests rouges**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| une session libre n'a pas de propriétaire | — |
| revendiquée par `u1`, elle rend `u1` | — |
| 🔴 `u2` n'est pas propriétaire d'une session revendiquée par `u1` | rendre `undefined` quoi qu'il arrive : plus personne n'est refusé |
| revendiquer deux fois par le **même** utilisateur est sans effet, jamais une erreur | lever : une reconnexion du même utilisateur casserait sa propre session |
| `liberer` rend la session à nouveau libre | ne jamais libérer : un nom de session serait perdu à vie |

- [ ] **Step 2 : implémenter sur le patron d'`appariement.ts`** — une `Map`, aucune importation de `ws`, aucun `Date`.

⚠️ **Le fichier porte en tête le coût de la décision E3** : l'appartenance ne
survit pas à un redémarrage du service, alors que le média, lui, survit
(spec §6). La vraie réponse est le préfixe opaque de P3.

---

### Task 11 : la garde, pure et synchrone

**Objet :** la règle complète de la poignée de main : qui passe, qui est refusé,
avec quel motif.

**Files:**
- Create: `plateforme/src/identite/garde.ts`, `plateforme/src/identite/garde.test.ts`

**Interfaces:** Consumes `verifierJeton` (T2), `ProprieteDeSession` (T10).

- [ ] **Step 1 : la table de décision, écrite avant le code**

| Poignée de main | Verdict |
| --- | --- |
| `role:'agent'`, quel que soit le jeton | **accepté**, `utilisateurId` absent. ⚠️ **E2 — c'est la fenêtre déclarée jusqu'à P3** |
| `role:'client'`, aucun jeton | refusé, `jeton-absent` |
| `role:'client'`, jeton mal formé ou mal signé | refusé, `jeton-invalide` |
| `role:'client'`, jeton expiré | refusé, `jeton-expire` |
| `role:'client'`, jeton valide, session libre ou déjà sienne | **accepté**, `utilisateurId` |
| `role:'client'`, jeton valide, session appartenant à un autre | refusé, `session-refusee` |

🔴 **Le message rendu SUR LE FIL pour `session-refusee` ne nomme pas le
propriétaire**, et ne dit pas si la session existe : `accès refusé à la session
demandée`. **Le journal, lui, porte le nom de session et l'identifiant du
demandeur** — c'est ce que le critère ③ de la spec exige (« refus typé,
journalisé, avec l'identifiant demandé »), et c'est ce qui sépare un diagnostic
d'un oracle.

- [ ] **Step 2 : les tests, six lignes de la table plus trois**

| Test supplémentaire | Ce qui le rend ROUGE |
| --- | --- |
| 🔴 **le critère ② vécu de bout en bout** : le même jeton est accepté à `t`, refusé à `t + duree` — **l'horloge injectée VARIE entre les deux appels** | figer l'horloge : le second appel passe |
| 🔴 `verifier` **n'a aucun effet de bord** : appelée deux fois sur une session libre, elle ne la revendique pas | y mettre la revendication : un pair refusé par `declarer` laisserait une appartenance fantôme |
| un `jeton` d'un type inattendu (nombre, objet, `null`) est refusé sans lever | passer la valeur à `String()` sans contrôle |

- [ ] **Step 3 : vert. Compte attendu : 9.**

---

### Task 12 : câbler la garde dans le relais — la seule tâche qui touche les tests de P1

**Objet :** brancher la garde, refuser proprement, **et prouver que rien
d'autre n'a bougé**.

**Files:**
- Modify: `plateforme/src/signaling/relais.ts` (219 l.),
  `plateforme/src/signaling/server.test.ts` (255 l.),
  `plateforme/src/signaling/resilience.test.ts` (181 l.),
  `plateforme/src/http/serveur.ts` (82 l.), `plateforme/src/http/serveur.test.ts` (107 l.)
- Create: `plateforme/src/signaling/garde-fil.test.ts` — les critères ①, ② et ③
  **au niveau du socket**, sur de vrais `WebSocket`

- [ ] **Step 1 : l'ordre dans le gestionnaire, écrit avant le code**

Dans le bloc « premier message » de `relais.ts:121-179`, **entre** le contrôle
de forme (l. 124-134) et `sessions.declarer` (l. 136) :

```
1. contrôle de forme          (inchangé, l. 124-134)
2. garde.verifier(...)        NEUF — si refus : send({type:'error', reason, motif})
                                     PUIS socket.close(1008, ...), et return
3. sessions.declarer(...)     (inchangé)
4. garde.revendiquer(...)     NEUF — seulement si (3) a accepté
5. trace / ice-config / offre en attente   (inchangés)
```

🔴 **L'ordre entre 2 et 3 n'est pas indifférent** : un pair refusé par la garde
ne doit **jamais** entrer dans la table d'appariement, sinon il occuperait le
rôle et empêcherait le pair légitime d'arriver — un déni de service ouvert à
l'anonyme, obtenu en refusant l'authentification.

🔴 **Le socket est FERMÉ après un refus de garde**, alors qu'il reste ouvert
après un message malformé (`relais.ts:107-118`, délibéré depuis le jalon 1).
C'est la spec §6 : « refus typé sur la poignée de main, connexion **fermée** —
contrairement au message malformé, que le relais laisse retenter à dessein ».
⚠️ **Envoyer PUIS fermer, jamais l'inverse** : un `terminate()` immédiat
tronquerait le message, et le pair verrait une fermeture sans motif. Un test
asserte l'ordre — message reçu **puis** `close`.

Et dans `serveur.ts` : construire la garde à partir de `config.secretJeton`,
avec `Date.now` comme horloge, et la passer au relais. `separe` appelle
`garde.liberer`.

- [ ] **Step 2 : le fichier de tests NEUF, ROUGE d'abord**

`garde-fil.test.ts`, sur de vrais sockets, à travers `demarrerServeur` :

| Test | Ce qui le rend ROUGE |
| --- | --- |
| 🔴 **critère ①, DEUX assertions distinctes** : un `client` sans jeton reçoit `{type:'error', motif:'jeton-absent'}`, **et** ne reçoit **aucun** message `ice-config` avant sa fermeture | neutraliser la garde : les deux assertions tombent. ⚠️ **Les deux sont exigées par la spec ; une seule ne suffirait pas** — un service qui refuserait après avoir envoyé `ice-config` passerait la première |
| le socket est **fermé** après le refus, et le message est arrivé **avant** | fermer avant d'envoyer |
| 🔴 **critère ②** : un jeton signé pour une durée écoulée est refusé `jeton-expire` — le service est démarré avec une horloge de test qui **avance** | figer l'horloge |
| 🔴 **critère ③** : `u1` prend `s-1`, puis `u2` la demande → refus `session-refusee`, **et le journal porte le nom de session** | ne pas révoquer l'appartenance : `u2` entre |
| après le départ des deux pairs, `u2` **peut** prendre `s-1` | ne jamais libérer |
| un pair `role:'agent'` **sans jeton** est toujours accepté | l'exiger : le chantier D casse. ⚠️ **Ce test EXISTE pour rendre E2 visible** : le jour où P3 l'inversera, il faudra le réécrire à dessein, pas par surprise |

⚠️ **Le service de test a besoin d'une horloge injectable.** Si `demarrerServeur`
ne l'accepte pas, ce test construit la garde lui-même et appelle
`createSignalingServer(wss, garde)` — c'est un choix d'implémentation, **à
trancher dans la tâche et à écrire**, pas à laisser flotter.

- [ ] **Step 3 : les trois harnais de P1, hunk par hunk (E1)**

Ne changer **aucune assertion**. Puis :

```bash
cd plateforme
git diff -- src/signaling/server.test.ts src/signaling/resilience.test.ts src/http/serveur.test.ts
```

**Verser ce `diff` dans les journaux**, et vérifier ligne à ligne qu'aucun
`expect(` n'y apparaît côté `+` autrement qu'inchangé. ⚠️ **C'est la preuve du
critère ① de P1, et elle est décidable — contrairement à « les tests sont
inchangés », que P1 a dû abandonner comme intenable.**

- [ ] **Step 4 : les deux passes, et le compte**

```bash
cd plateforme && npm run test:sqlite && npm run test:postgres && npm run typecheck
```

Les **64** tests de P1 doivent tous être verts, plus les neufs. **Annoncer le
total attendu avant de le lire.**

---

### Task 13 : l'appartenance ENREGISTRÉE en base

**Objet :** rendre littéralement vrai le mot « enregistrée » du critère ③ :
`session.utilisateur_id` cesse d'être toujours nul.

**Files:**
- Modify: `plateforme/src/depot/session.ts` (89 l.),
  `plateforme/src/depot/session.test.ts` (83 l.),
  `plateforme/src/signaling/trace.ts` (84 l.),
  `plateforme/src/signaling/trace.test.ts` (151 l.),
  `plateforme/src/signaling/relais.ts`

- [ ] **Step 1 : tests rouges**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `ouvrirSession` sans `utilisateurId` écrit `NULL` — **le comportement de P1 est intact** | rendre le paramètre requis : les appels de P1 cassent |
| une session appariée par un client authentifié porte son `utilisateur_id` en base | passer `undefined` : la colonne reste nulle |
| 🔴 une session appariée par un pair **`agent` seul** puis un client anonyme… **n'existe pas** : il n'y a pas de client anonyme après la tâche 12. Le test à écrire est celui de la session de contrôle `bureau`, où l'`agent` arrive seul et où le client **est** authentifié | écrire un test qui suppose un client anonyme : il serait **impossible à satisfaire**, et c'est ce qui le rendrait vacueux |

⚠️ **La troisième ligne est un piège que ce plan a failli poser lui-même** : la
tâche 12 rend un client anonyme inatteignable, donc tout test qui en suppose un
mesurerait un état que le produit ne peut plus produire.

- [ ] **Step 2 : implémenter** — `ObservateurDeSession.apparie` gagne un second
  paramètre facultatif ; `relais.ts` le lui passe depuis le verdict de garde.

- [ ] **Step 3 : les deux passes**

---

# Famille 4 — le navigateur

### Task 14 : `client/src/jeton.ts`, pur et sans DOM

**Objet :** un seul endroit du client qui sache où vit le jeton, et s'il est
encore frais.

**Files:**
- Create: `client/src/jeton.ts`, `client/src/jeton.test.ts`

⚠️ **Aucun DOM, et ce n'est pas un goût** : relevé par la commande le
19 août 2026, `client/` n'a **aucun** `vitest.config.*` — l'environnement de
test est donc le Node par défaut, sans `window` ni `localStorage`. Le `Coffre`
est **injecté** ; le module ne touche `globalThis.localStorage` que dans le
défaut d'un paramètre, jamais au chargement.

- [ ] **Step 1 : tests rouges, sur un `Coffre` factice de dix lignes**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `poser` puis `jetonAcces` rend l'accès | — |
| `vider` efface **les deux** clés | n'en effacer qu'une : la déconnexion laisserait un rafraîchissement utilisable |
| 🔴 `expireAvant` lit `exp` de la charge **sans vérifier la signature**, et le fichier le DIT | prétendre vérifier : le client n'a pas le secret, et croire qu'il vérifie est pire que savoir qu'il ne vérifie pas |
| `expireAvant` rend `true` sur un jeton mal formé | rendre `false` : un jeton illisible serait cru valable et la session échouerait plus tard, ailleurs |
| 🔴 `rafraichirSiNecessaire` **n'appelle pas** le réseau si le jeton est frais **au-delà de la marge** | appeler toujours : un aller-retour par ouverture de fenêtre |
| `rafraichirSiNecessaire` appelle, pose la paire neuve, rend `true` — l'appel est **injecté**, jamais `fetch` global | employer `fetch` : le test devient impossible sans réseau |
| un refus de l'appel **vide** le coffre et rend `false` | garder une paire morte : l'utilisateur boucle sur un refus sans jamais revoir l'écran de connexion |

⚠️ **Le stockage est `localStorage`, et le coût est nommé dans le fichier** :
un jeton en `localStorage` est lisible par tout script de la page, donc par une
injection de script. `sessionStorage` ne convient pas — la page-shell ouvre ses
fenêtres par `window.open` (`client/src/shell-page.ts:19`), et leur stockage de
session n'est pas garanti partagé. **C'est un arbitrage, pas un oubli**, et P5
est l'endroit où il se rouvrira avec les en-têtes de sécurité.

- [ ] **Step 2 : `cd client && npm test`** — attendu **107 + 7**.

---

### Task 15 : l'écran de connexion

**Objet :** une page où l'on entre un courriel et un mot de passe, qui pose la
paire de jetons et renvoie vers la shell. **Aucune direction visuelle** — elle
appartient au sous-projet ⑥.

**Files:**
- Create: `client/connexion.html`, `client/src/connexion.ts`
- Modify: `client/vite.config.ts` — l'entrée `connexion`

- [ ] **Step 1 : la page**

Un formulaire, un bouton, une zone de message. `POST` vers
`${plateformeUrl}/auth/connexion`, `poser()` en cas de succès, redirection vers
`shell.html`. En cas d'échec : **le message du serveur, tel quel** — qui ne
distingue pas courriel inconnu de mot de passe faux (tâche 8).

⚠️ **`connexion.ts` n'est PAS testé unitairement, et c'est déclaré** : c'est du
câblage DOM, comme `shell-page.ts` et `main.ts` qui ne le sont pas non plus
(`client/src/shell-page.ts:1-2` l'écrit : « Aucune règle ici — elles sont dans
`shell.ts`, qui est testé »). **Toute règle qu'il porterait doit descendre dans
`jeton.ts`.** Une revue de cette tâche vérifie qu'il ne reste que du câblage.

- [ ] **Step 2 : l'URL de la plateforme**

Elle se lit comme celle du signaling aujourd'hui : paramètre de requête, sinon
`window.location.hostname` et le port 8080 (`client/src/shell-page.ts:7`).
**Ne pas inventer une seconde convention** ; réemployer la même.

- [ ] **Step 3 : `npm run build` et `npm run typecheck` verts côté client**

Vérifier que `connexion.html` sort bien du build : une entrée `rollupOptions`
oubliée produit une page absente **sans aucune erreur**.

---

### Task 16 : le jeton dans la poignée de main du navigateur

**Objet :** les deux seuls sites d'émission côté navigateur portent le jeton, et
`client/src/main.ts` n'est **pas** touché (E7).

**Files:**
- Modify: `client/src/webrtc.ts` (l. 7-13 et 193), `client/src/shell-page.ts` (l. 45)

- [ ] **Step 1 : `webrtc.ts`**

`SessionOptions` gagne `jeton?: string`. `connectSession` envoie
`{role:'client', session: options.sessionId, jeton: options.jeton ?? jetonAcces()}`.

🔴 **Le champ est AJOUTÉ, aucun n'est retiré** — spec §10.2. Un service de P1
ignorerait simplement `jeton` (`relais.ts:122-123` ne lit que `role` et
`session`), donc le client de P2 reste compatible avec un service de P1. **La
compatibilité ne va que dans ce sens**, et c'est écrit dans le code.

- [ ] **Step 2 : `shell-page.ts`**

Même ajout, plus **la redirection** : si `jetonAcces()` est absent, aller à
`connexion.html` au lieu d'ouvrir le socket.

⚠️ **La page de session, elle, ne redirige pas** — elle affiche un refus (E7).
Elle est toujours ouverte par la shell, sur la même origine, donc le jeton y est
déjà. **Le dire dans le code de `webrtc.ts`**, sans quoi un successeur ajoutera
une redirection depuis une bibliothèque.

- [ ] **Step 3 : `cd client && npm test && npm run typecheck`**

---

### Task 17 : l'outillage de recette du chantier D continue de se connecter (E6)

**Objet :** les trois pilotes CDP sèment un jeton avant navigation, sinon ils
sont refusés — c'est une conséquence **déclarée** de P2, pas un effet de bord.

**Files:**
- Modify: `client/verify-webrtc.mjs` (**497 l., marge 3**),
  `client/recette/paire-candidats.mjs` (259 l.),
  `client/recette/harness.mjs` (380 l.)

🔴 **PORTE, à jouer avant ET après** :

```bash
wc -l client/verify-webrtc.mjs
```

**497 avant. L'addition doit être à somme NULLE ou NÉGATIVE sur ce fichier.**
S'il n'est pas possible d'y tenir, **extraire** — jamais compresser un
commentaire pour atteindre un compte, geste que `CLAUDE.md` interdit nommément
et que D9 a payé deux fois. ⚠️ **La divergence de convention que `CLAUDE.md`
signale sans la trancher — ce fichier est hors du § « Portée » mais dans la
commande — n'est PAS tranchée ici** : on applique la contrainte la plus stricte,
qui est sûre dans les deux lectures.

- [ ] **Step 1 : obtenir le jeton de la plateforme, jamais le forger**

Le pilote appelle `POST /auth/connexion` avec un compte de recette, et sème la
paire dans `localStorage` par le
`Page.addScriptToEvaluateOnNewDocument` que les trois scripts **appellent déjà**
(`verify-webrtc.mjs:186`, `paire-candidats.mjs:187`, `harness.mjs:129`).

🔴 **Ne jamais forger le jeton dans le script.** Un script qui signerait
lui-même porterait le secret de la plateforme, et déplacerait le trou au lieu de
le fermer.

⚠️ **Le compte de recette est créé par la ligne de commande de la tâche 9**, et
son mot de passe vient de l'environnement — **jamais écrit dans un fichier
versionné**. C'est le critère ④ de P5 par anticipation.

- [ ] **Step 2 : le contrôle qui dit que ça marche**

Lancer `node client/verify-webrtc.mjs` **une fois** et verser sa sortie.
⚠️ **Cet outil pilote un vrai Chrome et une vraie session ; il n'est PAS un
critère de P2** et son échec peut venir d'ailleurs (il n'y a pas de VM). **Ce
qui est exigé, c'est que le refus d'authentification ne soit plus la cause** :
la sortie ne doit plus porter le motif `jeton-absent`. Si l'outil échoue pour
une autre raison, **cette raison est ÉCRITE dans le document de résultats**,
jamais tue.

- [ ] **Step 3 : `wc -l` de nouveau, et la porte vérifiée**

---

# Famille 5 — recette, revue transverse, index

### Task 18 : la recette — quatre critères, DEUX exécutions chacun, rouges jouées

**Objet :** juger P2 sur les quatre critères de la spec §4, avec les pièces
versées dans git.

**Files:**
- Create: `docs/superpowers/plans/journaux-plateforme-p2/*` (tous versés),
  et la première rédaction de
  `docs/superpowers/plans/2026-08-19-plateforme-p2-resultats.md`

⛔ **Aucune VM Windows.** Prérequis : `docker compose -f
docker-compose.plateforme.yml up -d` (relevé ⑪ : l'instance tournait, saine, le
19 août 2026).

- [ ] **Step 1 : les quatre critères, deux exécutions chacun**

| # | Critère | Jugé par | Pièce |
| --- | --- | --- | --- |
| ① | un pair `client` non authentifié est refusé **et ne reçoit aucun `ice-config`** | `garde-fil.test.ts`, **deux assertions distinctes** | `critere-1-{1,2}.log` |
| ② | un jeton expiré est refusé, **horloge qui varie** | `jeton.test.ts` + `garde-fil.test.ts` | `critere-2-{1,2}.log` |
| ③ | un utilisateur ne peut pas rejoindre la session d'un autre | `garde-fil.test.ts` + la ligne `session.utilisateur_id` en base | `critere-3-{1,2}.log` |
| ④ | un mot de passe n'est **jamais** journalisé ni renvoyé | `routes-auth.test.ts`, balayage sur **le champ** | `critere-4-{1,2}.log` |
| témoin | non-régression | `verify-all.sh` sortie 0 | `verify-all-{1,2}.log` |

- [ ] **Step 2 : les ROUGES, jouées et versées**

**Sept au moins**, chacune avec son message d'échec **verbatim**, et la source
**restaurée puis comparée à l'identique** après chacune (`git diff` vide) :

| # | Ce qu'on casse | Attendu |
| --- | --- | --- |
| ① A | la garde neutralisée dans `relais.ts` (verdict forcé à `ok`) | les **deux** assertions du critère ① tombent |
| ① B | 🔴 **la rouge « gratuite » de la spec §7.2** : le service **de P1** délivre `ice-config` à quiconque | voir Step 3 |
| ② | l'`exp` ignoré dans `verifierJeton` | le jeton périmé est accepté |
| ③ | la revendication retirée de `relais.ts` | `u2` entre dans la session de `u1` |
| ④ | un `console.log(JSON.stringify(corps))` dans `/auth/connexion` | le balayage rougit |
| E5 | la ligne **supprimée** à la rotation au lieu d'être marquée | le rejeu rend `inconnu`, la famille survit |
| lint | un `expire_a INTEGER` dans `0002` | `sous-ensemble.test.ts` rougit |

- [ ] **Step 3 : la rouge ① B, DÉCISIVE, sur l'arbre de P1**

La spec §7.2 dit qu'elle est **gratuite** parce que le binaire du sous-bloc
précédent la porte déjà. Elle vaut mieux qu'une neutralisation, parce qu'elle ne
dépend d'aucune modification de notre part :

```bash
git worktree add /tmp/p1-rouge 19f6409
ln -s "$PWD/plateforme/node_modules" /tmp/p1-rouge/plateforme/node_modules
# démarrer le service de P1, y connecter un WebSocket nu déclarant
# {"role":"client","session":"x"} SANS jeton, et relever qu'il reçoit ice-config
# (TURN_URL et TURN_SECRET posés) et n'est jamais refusé.
```

⚠️ **Le lien symbolique de `node_modules` est le remède au piège de D4** — un
`build`/`test` lancé depuis un `git worktree` s'arrête **en silence** faute de
`node_modules`. **Si cette rouge n'aboutit pas pour une raison d'outillage, la
raison est ÉCRITE dans le document de résultats**, et la rouge ① A tient lieu de
preuve, en le disant. Une rouge annoncée et non jouée serait une affirmation
au-delà du relevé.

- [ ] **Step 4 : le document de résultats**

Sur le modèle de `2026-08-19-plateforme-p1-resultats.md` : verdict par critère
**avec son nombre d'exécutions**, chaque rouge avec son message verbatim, le
sort des onze divergences E1…E11, ce que le code livre, **ce que P2 n'établit
PAS**, et le renvoi vers chaque journal versé.

🔴 **La section « Ce que P2 n'établit PAS » porte au minimum** :

- **le rôle `agent` reste anonyme** et reçoit toujours `ice-config` valable
  86 400 s (E2) — **le trou n'est fermé qu'à moitié** ;
- **la garde ne s'applique qu'à la POIGNÉE DE MAIN** : une session déjà ouverte
  n'est jamais revérifiée, donc un jeton qui expire en cours de session ne coupe
  rien. La spec §3.5 dit qu'un jeton court n'est pas révocable avant expiration ;
  **ici il ne l'est même pas après**, tant que le socket vit ;
- **l'appartenance de session ne survit pas à un redémarrage** (E3) ;
- **aucune constante n'est calibrée** : `N`/`r`/`p` de `scrypt`,
  `DUREE_JETON_ACCES_MS`, `DUREE_RAFRAICHISSEMENT_MS`, `LONGUEUR_SECRET_MIN`, la
  marge de rafraîchissement du client, et `DUREE_SECONDES = 86_400` que P2 **ne
  recalibre pas** (spec §3.5) ;
- **l'égalisation temporelle du chemin de connexion n'est pas mesurée** ;
- **aucun navigateur réel n'a authentifié quoi que ce soit** hors la tâche 17,
  qui n'est pas un critère ;
- **aucune protection contre le rejeu du jeton d'ACCÈS** : il est porteur, et
  quiconque l'obtient peut ouvrir une session jusqu'à son expiration ;
- **aucun frein sur les routes d'authentification** — c'est P5 ③ ;
- **aucun audit par un tiers** : CSRF, fixation de session et attaques
  temporelles sont traités par des choix raisonnés, non éprouvés (spec §8) ;
- **aucun taux, nulle part.**

---

### Task 19 : la revue transverse de fin de branche — OBLIGATOIRE

**Objet :** trouver les affirmations — commentaires, documents, `CLAUDE.md` —
que **la branche elle-même** a rendues fausses.

**Files:** ceux que la revue désigne, plus le document de résultats.

⚠️ **Elle n'est jamais facultative.** Elle a trouvé **5** défauts en D7, **3**
en D8, **6** en D9, **douze** en D10, **huit** en P1. Ils ont tous la même
forme : **corrects des deux côtés pris séparément**, faux ensemble. **Une revue
par tâche ne peut structurellement pas les voir.**

- [ ] **Step 1 : les sites déjà connus, relevés par la commande le 19 août 2026**

Chacun **doit** être relu ; la liste n'est pas exhaustive, et le déclarer fait
partie de la tâche.

| Site | Ce qui devient faux |
| --- | --- |
| `plateforme/src/signaling/relais.ts:9-12` | « “Aucune authentification” reste VRAI, et le restera jusqu'à P2 » — **faux en P2**, et à remplacer par la **moitié** qui reste vraie (E2), pas à supprimer |
| `plateforme/src/config.ts:6-11` | « délivrait des identifiants TURN valables 24 h à quiconque » — encore vrai pour `role:'agent'`, plus pour `role:'client'`. La phrase doit dire **laquelle des deux moitiés** |
| `plateforme/src/http/serveur.ts:33-34` | « Toute route HTTP répond 404 : P2 et P4 en ajouteront, P1 n'en sert aucune » — P2 en sert deux |
| `plateforme/src/base/migrations/0001-socle.sql:23-24` | « `utilisateur` est créée par P1 et **RESTE VIDE** : P2 lui donne son comportement » — P2 l'a fait |
| `plateforme/src/base/migrations/0001-socle.sql:56-58` | « `utilisateur_id` et `vm_id` naissent NULL […] renseignés par P2 et P3 » — P2 renseigne le premier |
| `plateforme/src/signaling/appariement.ts:8-13` | « AVANT que P2 (garde d'authentification) […] n'y ajoutent quoi que ce soit » — P2 a ajouté, **ailleurs** : la phrase doit dire que la marge a servi |
| `plateforme/src/depot/session.ts:11-17` | l'avertissement sur ce que la table n'est pas — à relire à la lumière d'`utilisateur_id` |
| `plateforme/src/signaling/trace.ts:3-14` | « QUAND, exactement » — l'appartenance s'y ajoute |
| `plateforme/src/base/pilote.test.ts:38-50` | l'allow-list — **doit rester à `['pg','ws']`** ; si elle a bougé, P2 a ajouté une dépendance sans le dire |
| `docs/superpowers/plans/2026-08-19-plateforme-p1-resultats.md` §7 | « Aucune authentification. Le port, s'il est atteint, délivre toujours des identifiants TURN à quiconque. **C'est P2** » — à annoter, **jamais à réécrire** : c'est un relevé daté |
| `CLAUDE.md`, section P1, ⑧ | la même phrase, au même titre |
| `CLAUDE.md`, § « Chantier C volet 2 » | « le signaling doit être relancé AVEC l'environnement » — une variable de plus s'y ajoute, et **sans secret le service ne démarre pas du tout** |
| `docs/.../plateforme-design.md` §2.3 ③ | « Les identifiants TURN sont délivrés sans aucune authentification » — vrai **de moitié** après P2 |

- [ ] **Step 2 : le balayage, par le SENS et non par la formule**

`CLAUDE.md` en fait une doctrine : *une négation se dit de plusieurs façons, et
c'est celle qu'on n'a pas listée qui survit*. Balayer sur **la chose niée** :

```bash
grep -rn "authentification\|authentifi\|jeton\|sans jeton\|quiconque\|anonyme" \
  plateforme/src client/src CLAUDE.md docs/superpowers/plans/2026-08-19-plateforme-p1*.md \
  docs/superpowers/specs/2026-08-19-plateforme-design.md
```

- [ ] **Step 3 : annoter les documents DATÉS, ne jamais les réécrire**

Un relevé de P1 reste **vrai comme histoire**. Le barrer le rendrait faux. La
règle du dépôt : **corriger à sa place**, et « corrigé à sa place » est une
affirmation de **complétude** — donc elle se vérifie en **énumérant les places
AVANT d'écrire** (`grep -n`), puis en **relisant chaque place APRÈS l'édition**.
Ce dépôt a payé **neuf fois** pour ce geste manquant.

---

### Task 20 : `CLAUDE.md` gagne sa section P2

**Objet :** l'index de connaissance porte ce que P2 a établi, avec des chiffres
relevés **par la commande**.

**Files:** Modify `CLAUDE.md`.

- [ ] **Step 1 : la section, sur le modèle de celle de P1** (`CLAUDE.md:6296`)

En-tête (résultats, plan, conception, journaux et leur **famille de lecture**),
puis : le fait n°1 (la garde et **la moitié du trou qui reste ouverte**), les
**deux** variables d'environnement neuves, le format de mot de passe préfixé, la
détection de rejeu, les onze divergences, le verdict des quatre critères **avec
leur nombre d'exécutions**, « ce que P2 n'établit pas », et les pièges neufs.

**Pièges neufs à y porter, tous MESURÉS le 19 août 2026** :

- `scrypt` **refuse** `N = 32768` avec le `maxmem` par défaut, et le message ne
  parle pas de `N` (relevé ③) ;
- `timingSafeEqual` **lève** sur des longueurs différentes (relevé ④) ;
- un JWT HS256 s'écrit entièrement avec `node:crypto` : **aucune dépendance
  n'est nécessaire**, et le contrôle d'allow-list de P1 en est le témoin ;
- ⚠️ **`localStorage` est le seul stockage partagé entre une page et les
  fenêtres qu'elle ouvre par `window.open`** ; `Page.addScriptToEvaluateOnNewDocument`
  ne court **pas** sur ces fenêtres (piège D5), donc un pilote CDP ne peut pas y
  injecter un jeton après coup.

- [ ] **Step 2 : les tailles, RELEVÉES PAR LA COMMANDE, jamais recopiées**

```bash
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
find plateforme/src client/src -name '*.ts' | xargs wc -l | sort -rn | head -20
wc -l client/verify-webrtc.mjs
```

🔴 **Relevées APRÈS la dernière édition de la ronde, y compris celles de la
tâche 19.** Une table mesurée en début de ronde est fausse à la fin de la même
ronde — erreur que D8 a commise en croyant bien faire, et que D9 a dû corriger.

🔴 **Toucher une ligne d'un tableau de comptes oblige à remesurer son compte**,
même quand ce n'est pas l'objet de l'édition. C'est la doctrine que D10 a
ajoutée après deux occurrences où la main éditait la ligne même qui portait le
chiffre faux.

- [ ] **Step 3 : commit à pathspec explicite**

```bash
git commit -m "docs(p2): la section P2, et les chiffres releves par la commande" -- CLAUDE.md
```

---

### Task 21 : clore le document de résultats

**Objet :** finir ce que la tâche 18 a commencé, une fois la revue transverse
passée.

**Files:** Modify
`docs/superpowers/plans/2026-08-19-plateforme-p2-resultats.md`.

- [ ] Le sort des onze divergences, **une ligne chacune**.
- [ ] Le renvoi vers **chaque** journal versé, avec ce qu'il porte.
- [ ] Ce que la revue transverse a trouvé, **avec son compte** — et si elle n'a
      rien trouvé, le dire, ce qui serait une **première** dans ce dépôt et
      mériterait d'être regardé deux fois.
- [ ] 🔴 **Vérifier qu'aucune preuve d'une affirmation de `CLAUDE.md` ne vit
      hors de git.** D10 a établi par la commande que l'espace de travail de D9
      a disparu, emportant six constats définitivement perdus.

```bash
git status --porcelain docs/superpowers/plans/journaux-plateforme-p2/
```

Doit être **vide** : tout est suivi.

---

## Ce que ce plan ne prescrit PAS, et pourquoi

- **Aucune mesure de latence, de charge, ni de durée.** La cible « < 3 s si VM
  chaude » n'est jugée par aucun critère de ⑤ (spec §8).
- **Aucun frein sur les routes d'authentification** : c'est le critère ③ de P5.
  ⚠️ **Conséquence à assumer entre P2 et P5** : `/auth/connexion` est ouverte à
  la force brute, bornée seulement par les ~29 ms de `scrypt` et par l'écoute
  restreinte de `PLATEFORME_HOTE`. **C'est la même raison qui rend la fenêtre
  d'exposition d'E2 tolérable, et elle a la même fragilité.**
- **Aucune terminaison TLS**, aucun cookie, aucun en-tête de sécurité : P5.
- **Aucune inscription publique, aucune réinitialisation par courriel, aucun
  OIDC** : hors périmètre v1 (spec §9).
- **Aucune révocation immédiate d'un jeton d'accès** : impossible par
  construction (spec §3.5).
- **Aucun changement du protocole du fil hors l'ajout du champ `jeton`** : la
  spec §10.2 dit « P2 ajoute un champ, il n'en retire aucun ».
- **Aucune modification de l'agent Rust** : c'est P3, et P3 seul.
