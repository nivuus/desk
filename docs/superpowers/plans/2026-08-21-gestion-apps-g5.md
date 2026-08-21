# Sous-projet ④ — Gestion d'apps, sous-bloc **G5** : la PWA par application, et les types installeur du hub

**Date** : 21 août 2026
**Conception** : `docs/superpowers/specs/2026-08-19-gestion-apps-design.md`, §5 « G5 »
(l. 850-873), §6 (l'arborescence), §10 (hors périmètre v1)
**Cadrage parent** : `2026-07-27-refonte-produit-design.md` §5 ② (« PWA par
application : manifest dynamique, file handlers, Window Controls Overlay,
service worker »), §5 ④, et son **amendement du 28/07/2026** (l. 177-215),
transcrit verbatim au §3.3 de ce plan
**Amont mesuré** : G1 (le catalogue), G2 (les icônes 256), G3 (le téléversement
et l'installation), G4 (la surveillance), **A1** (la couleur d'accent, sous-projet
① « Divers »), **S1→S4** (le socle de design de ⑥, clos)
**Statut** : plan, aucune ligne de code écrite

> 🔴 **CE PLAN EST UN DOCUMENT, ET SON AUTEUR N'A MODIFIÉ AUCUN FICHIER DE CODE.**
> Tout ce qui suit est soit une **mesure** prise le 21 août 2026 sur l'arbre à
> `HEAD = afd8196`, soit une **décision** tranchée ici pour n'être pas découverte
> à l'exécution. Chaque nombre porte l'indication de qui l'a produit. Les
> affirmations que je n'ai pas vérifiées sont écrites comme telles.

---

## 0. Ce qu'il faut lire en premier, si l'on ne lit qu'une chose

**G5 se heurte, dès sa première ligne, à un obstacle que le sous-bloc G2 a nommé
sans le trancher, et qui est PLUS LARGE que ce que G2 en disait.**

G2 l'a écrit dans le code, et le voici **verbatim**
(`plateforme/src/http/routes-icone.ts:20-26`) :

```
// 🔴 CONSÉQUENCE NOMMÉE ICI PLUTÔT QUE DÉCOUVERTE PLUS TARD, ET ELLE APPARTIENT
// AU SOUS-BLOC G5 : **un `<img src>` NE PORTE PAS D'EN-TÊTE `Authorization`.**
// Une page qui afficherait ces icônes devra les chercher par `fetch()` puis
// `URL.createObjectURL`, et **un manifeste PWA — dont le navigateur va chercher
// les icônes tout seul, sans en-tête — NE POURRA PAS pointer cette route en
// l'état**. G2 ne le tranche pas : le trancher demanderait de décider si une
// icône peut être servie sans jeton, ce qui est une décision de sécurité.
```

🔴 **L'obstacle est d'un cran plus haut que l'icône : LE MANIFESTE LUI-MÊME NE
PEUT PAS VIVRE DERRIÈRE `Authorization`.** Un `<link rel="manifest" href="…">`
est allé chercher par le navigateur **sans en-tête d'autorisation**, exactement
comme les icônes qu'il nomme. Le seul mécanisme du Web qui ferait porter une
identité à ces requêtes est le **cookie** (`crossorigin="use-credentials"`), et
le sous-projet ⑤ **n'en pose aucun** — c'est un legs écrit et non levé
(P5 : « ⛔ **« ni cookies » RESTE VRAI** »). **Le porteur de ⑤ vit dans
`localStorage`, et `localStorage` ne voyage sur aucune requête que le navigateur
émet de lui-même.**

Il n'y a donc pas trois moitiés de problème mais **une seule question**, et ce
plan la pose en **porte éliminatoire, avant toute écriture** (§3) :

> **Peut-on faire installer une PWA par un navigateur sans qu'aucune route
> authentifiée ne s'ouvre ?**

La réponse candidate est **oui**, par un manifeste que la page **construit
elle-même** et publie en `blob:`, dont les icônes sont des `data:` — les deux
étant produits à partir de `fetch()` **authentifiés**. Ce chemin ne change
**aucune** route, ne retire **aucun** contrôle de porteur, et **n'appelle donc
aucune décision de sécurité**. Il n'est pas acquis : **il se mesure**, et la
porte P0 le mesure avant que quoi que ce soit ne soit écrit.

**Si la porte P0 refuse**, alors la seule voie restante ouvre une route
aujourd'hui authentifiée, **c'est-à-dire une décision de sécurité qui appartient
au propriétaire du dépôt et que ce plan REFUSE de prendre à sa place** (§4, D2).
Le sous-bloc reste livrable dans ce cas — ses critères ② et ③ n'en dépendent
pas — et le critère ① est déclaré **BLOQUÉ SUR UNE DÉCISION DE SÉCURITÉ**,
jamais « échoué ».

---

## 1. Le contrôle d'entrée — tâche 1, et rien avant elle

⚠️ **Ce dépôt a publié des chiffres faux à répétition, et il l'a payé neuf fois
au moins** : le naufrage du « 487 », un plan qui portait **deux** nombres faux
dans sa table de tailles, un autre dont les cinq croissances annoncées étaient
sous-estimées d'un facteur 2 à 4, un troisième qui affirmait une purge de dette
qui n'avait pas eu lieu, un quatrième qui **a publié un compte faux dans le
message de commit qui le corrigeait**, un cinquième qui a écrit « treize
exécutions » pour vingt-deux. **La tâche 1 remesure TOUT, et son journal est la
seule ligne de base.**

### 1.1 Ce que la tâche 1 relève, par la commande

```bash
unset -f chpwd 2>/dev/null   # le hook du shell hôte injecte un `ls` dès qu'un `cd` court en sous-shell

# ① la dette de taille — la SEULE source de vérité
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>450'

# ② les fichiers que G5 touchera, nommément
wc -l client/vite.config.ts client/src/design/tokens.css client/src/style.css \
      client/src/style.test.ts client/src/hub/televersement.ts \
      client/outils/tokens-orphelins/attente.mjs \
      plateforme/src/http/routes-applications.ts plateforme/src/http/routes-icone.ts \
      plateforme/src/http/serveur.ts plateforme/src/depot/application.ts \
      proto/src/plateforme.rs proto/ts/plateforme.ts \
      agent/src/apps/icone/extraction.rs agent/src/accent.rs

# ③ les comptes de tests — QUATRE commandes, jamais une
cd agent && cargo test -p agent 2>&1 | tail -3;            cd ..
cd client && npx vitest run --reporter=dot 2>&1 | tail -5; cd ..
cd proto  && npx vitest run --reporter=dot 2>&1 | tail -5; cd ..
cd plateforme && npm run test:sqlite 2>&1 | tail -5;       cd ..

# ④ le socle de ⑥ — les sept scripts, PLUS les deux tests unitaires
cd client && npm run design:verifier; cd ..
cd client && npx vitest run src/design/theme.test.ts src/design/longueurs.test.ts src/style.test.ts; cd ..

# ⑤ la liste d'attente de §7.6, et les sous-blocs clos
sed -n '219p' client/outils/tokens-orphelins/attente.mjs
grep -n 'SOUS_BLOCS_CLOS = ' client/outils/tokens-orphelins/sous-blocs-clos.mjs

# ⑥ l'état des voisins — G4 est-il clos ? A1 est-il clos ?
git log --oneline -12
git status --porcelain

# ⑦ le filet du dépôt, DEPUIS UN SHELL PROPRE
env -u TURN_URL -u TURN_SECRET ./scripts/verify-all.sh
```

### 1.2 Ce que j'ai mesuré le 21 août 2026, et que la tâche 1 doit RETROUVER ou CORRIGER

⚠️ **Ces nombres sont datés et ils DÉRIVERONT** : deux chantiers voisins écrivent
dans le même arbre. Ils sont donnés pour que la tâche 1 sache ce qui a bougé, pas
pour être recopiés.

| Grandeur | Relevé le 21 août 2026, par la commande | Par qui |
| --- | --- | --- |
| `HEAD` | `afd8196` | moi |
| Tableau de dette (> 500 lignes) | **DEUX lignes** : `agent/src/encode.rs` **1536**, `agent/src/windows_source.rs` **630** | moi |
| `plateforme/src/http/routes-installation.ts` | **500** — 🔴 **marge 0, ex æquo avec `encode/arret.rs`** | moi |
| `agent/src/encode/arret.rs` | **500** — marge 0 | moi |
| `client/verify-webrtc.mjs` | **494** — marge 6. ⚠️ `CLAUDE.md` le publie encore à **497** en quatre endroits datés | moi |
| `plateforme/src/http/routes-installation.test.ts` | **485** | moi |
| `client/src/main.ts` | **483** | moi |
| `plateforme/src/http/routes-applications.test.ts` | **448** | moi |
| `plateforme/src/http/serveur.ts` | **432** | moi |
| `plateforme/src/http/routes-applications.ts` | **334** | moi |
| `plateforme/src/http/routes-icone.ts` | **312** | moi |
| 🔴 `client/src/design/tokens.css` | **300**, pour une porte de **300** — **marge NULLE** | moi |
| `client/src/hub/televersement.ts` | **300** | moi |
| `client/src/design/tokens.ts` | **253** | enquête |
| `plateforme/src/depot/application.ts` | **233** | moi |
| `client/outils/tokens-orphelins/attente.mjs` | **221**, seuil d'extraction conditionnel **240** | enquête |
| `client/src/style.test.ts` | **219** | moi |
| `client/src/style.css` | **194** | moi |
| `client/vite.config.ts` | **112** | moi |
| `cd client && npx vitest run` | **479 tests, 42 fichiers** | moi |
| `cd proto && npx vitest run` | **298 tests, 9 fichiers** | moi |
| `npm run design:verifier` | **7/7 vert**, `exit 0` | moi |
| §7.1 | **53 paires, 0 échec, minimum 3,16** | moi |
| §7.6 | **52 déclarés / 52 employés / 0 orphelin**, liste d'attente **VIDE** | moi |
| §7.7 | **8 616 octets / plafond 12 288 — marge 3 672** | moi |
| `SOUS_BLOCS_CLOS` | `new Set(['S1','S2','S3','S4'])` — 🔵 **ni `A1`, ni `G5`** | moi |
| `EN_ATTENTE_D_APPELANT` | `new Map([])` — **VIDE depuis S4 tâche 6** | moi |
| `PLATEFORME_VERSION` | **4** (`proto/src/plateforme.rs:122`) | moi |
| Entrées Vite | **CINQ** : `index`, `shell`, `connexion`, `design`, `primitives` | moi |
| `client/public/` | 🔴 **N'EXISTE PAS** | moi |
| Manifeste PWA dans `client/` | 🔴 **AUCUN**, nulle part dans le produit neuf | moi + enquête |
| Service worker dans `client/` | 🔴 **AUCUN** | moi |
| Page de hub | 🔴 **AUCUNE** — `client/src/hub/` ne contient que `televersement.ts` et son test, **sans un seul appelant dans le produit** | moi |
| `scripts/verify-all.sh` | **DIX** appels `etape` | moi |
| `COULEURS_HORS_THEME` | **SEPT** tokens (`tokens.ts:199-207`) — ⚠️ `CLAUDE.md` en publie encore **six** | moi |
| Lecteurs de `tokens.css` **par son chemin** | **NEUF**, énumérés au D3 — ⚠️ `CLAUDE.md` en publie **sept** ; A1 en a ajouté deux (`accent.test.ts:30`, `accent-dom.test.ts:15`) | moi |
| Corpus de la VM | **220 raccourcis / 156 clés** — ⚠️ et **non** 218/154 | enquête |

🔴 **CE QUE JE N'AI PAS MESURÉ, et que la tâche 1 doit produire** :
`cargo test -p agent`, `cargo test --workspace`, `cargo clippy --workspace`, les
suites `plateforme` (sqlite et postgres), et `verify-all.sh`. **Je ne publie
donc aucun de ces comptes**, et un plan qui en publierait sans les avoir lancés
fabriquerait une pièce — le mode de défaillance que ce dépôt juge le plus grave,
et qu'il a constaté **deux fois**.

⚠️ **`cargo clippy --workspace` porte QUATRE avertissements HORS `dead_code`**
(`sommeil.rs`, `vivier.rs`, `mire.rs`, `piste_audio.rs`), tous **préexistants**
— relevé par A1 dans son journal d'entrée, **que je n'ai pas rejoué**. **La
formule habituelle « tous `dead_code` » est FAUSSE et ne doit pas être
reprise.** Et `grep -c '^warning'` **compte aussi la ligne de résumé** : lire le
compte que `cargo` écrit lui-même.

⚠️ **`verify-all.sh` n'est pas hermétique** : avec `TURN_URL`/`TURN_SECRET` dans
l'environnement — c'est-à-dire après le `set -a && source .env` que tout travail
sur la VM exige — **six** tests de signaling échouent, sensibilité **préexistante
et mesurée par G1**. Lancer depuis un shell propre, ou
`env -u TURN_URL -u TURN_SECRET`.

⚠️ **`cd client && npx vitest run` NE COUVRE PAS `proto/ts/`** : la racine Vitest
est `client/`. **Deux commandes, jamais une.**

⚠️ **`client/src/design/longueurs.test.ts` et `client/src/style.test.ts` doivent
être lancés DEPUIS `client/`** : lancés de la racine, la racine Vite change,
`test: { css: true }` n'est plus retenu, **le CSS est court-circuité EN SILENCE**
et les feuilles valent la chaîne vide — les assertions d'absence passent au vert
en ne mesurant rien. Deux rouges de S4 ont d'abord été jouées ainsi.

### 1.3 Ce qui invalide la ligne de base

- un `git status --porcelain` **non vide sur autre chose que `docs/`** : l'arbre
  porte alors du travail d'un voisin, et tout compte est **daté à l'heure**, pas
  au commit. Le relevé le dit, il ne l'ignore pas ;
- une étape de `verify-all.sh` en échec : la tâche 1 **nomme laquelle et à qui
  elle appartient**, par `git log -S` sur le symbole fautif. **Présenter le filet
  comme vert serait faux ; le présenter comme rouge le serait tout autant** — le
  chantier E a payé cette leçon.

---

## 2. Ce que G5 livre, et ce qu'il ne livre pas

### 2.1 Ce que la conception demande

Spec §5 « G5 », l. 850-855, **verbatim** :

> **Livre** : le manifeste dynamique par application (icône 256, couleur
> d'accent) ; les `file_handlers` alimentés par les **vraies** associations lues
> par l'agent, sur le modèle de `src/asset.js:82-98` ; et l'ajout des types
> installeur au manifeste du **hub**, conformément à l'amendement du 28/07/2026.

### 2.2 Les six tranches, et laquelle sert quel critère

| Tranche | Ce qu'elle livre | Critère servi | Sans elle ? |
| --- | --- | --- | --- |
| **A** | La **porte P0** — la mesure qui décide de tout (§3) | — | G5 est aveugle |
| **B** | La **page de hub**, sixième entrée Vite : liste, icônes, lancement, **zone de dépôt** câblée sur `hub/televersement.ts`, `launchQueue` | **②**, **③** | ② et ③ sont indémontrables |
| **C** | Le **manifeste du hub**, statique, engendré au build depuis `tokens.ts`, avec ses `file_handlers` installeur | **③** | ③ est indémontrable |
| **D** | Le **manifeste par application**, construit côté client (voie de la porte P0) | **①** | ① est indémontrable |
| **E** | Le **Window Controls Overlay** — `display_override` posé, et le legs de S4 exercé ou déclaré non mesurable | *(aucun — legs de ⑥)* | le legs de S4 reste entier |
| **F** | La **couleur d'accent** et les **associations de fichiers** : agent → `proto` v5 → schéma → manifeste | *(aucun)* | la clause « Livre » de la spec n'est tenue qu'à moitié |

🔴 **LA TRANCHE F NE SERT AUCUN CRITÈRE, ET C'EST UN FAIT, PAS UN ARBITRAGE.**
Les trois critères de la spec portent sur l'**installabilité** (①), le
**glisser-déposer** (②) et le **test empirique** (③). Ni `theme_color`, ni les
`file_handlers` **par application** n'y figurent. La tranche F est donc placée
**en dernier**, après la recette de ①②③, **pour que le sous-bloc soit déjà
livrable si elle doit être retirée**. Sa porte est écrite au §6.6.

### 2.3 Ce que G5 NE livre PAS, et pourquoi — décidé ici, pas découvert

- ⛔ **Aucun service worker.** Le cadrage §5 ② le liste, et la spec du retrait du
  legacy le range nommément sous G5 (`2026-08-20-retrait-legacy-design.md:364`,
  ligne 13 de son inventaire, et son verrou **C5**). **La conception de ④ ne le
  mentionne à aucune ligne de son §« G5 ».** Trois raisons de ne pas l'inventer :
  ① la conception qui gouverne ce sous-bloc ne le demande pas ; ② un service
  worker sans doctrine de cache **sert une coquille périmée**, c'est-à-dire un
  mode de panne muet, et aucun des neuf contrôles de ⑥ n'en voit un ; ③ la porte
  P0 dira si Chromium en **exige** un pour l'installabilité — et si elle dit oui,
  ce n'est plus un choix mais une dépendance, et la tranche D le déclare bloqué
  plutôt que d'improviser. **Legs nommé, destinataire nommé : le chantier de
  retrait du legacy, verrou C5.**
- ⛔ **Aucune direction visuelle du hub au-delà du socle de ⑥.** La spec §10 dit
  « ④ livre un hub **fonctionnel** ; ⑥ l'habille » — **et ⑥ est CLOS**, son legs
  n°2 disant « Le hub n'existe pas ; son contenu dépend de ④ ». **Les deux se
  renvoient la balle, et ce plan le constate plutôt que de le taire** : G5 livre
  un hub qui emploie les **primitives** de S2 et les **tokens** de S1, sans
  inventer une direction visuelle. **Aucun jugement visuel ne sera porté** —
  personne n'a jamais ouvert une page de ⑥ dans un navigateur, d'un bout à
  l'autre de S1 à S4, et G5 n'invente aucun critère qui aurait l'air de le
  couvrir.
- ⛔ **Aucun classement, aucune recherche, aucune catégorie** du catalogue
  (spec §10).
- ⛔ **Aucune désinstallation depuis le hub** (spec §10).
- ⛔ **Aucune modification du produit historique** (`src/`, `web/`, `index.js`) —
  cadrage §11. `src/asset.js:82-100` est **lu comme modèle**, jamais touché.
- ⛔ **`--accent-fenetre` n'est ni déclaré, ni peint.** Décision D3, §4.

---

## 3. La porte P0 — éliminatoire, jouée AVANT toute écriture de produit

### 3.1 La question, et pourquoi elle est éliminatoire

**Un navigateur va chercher un manifeste, et les icônes qu'il nomme, SANS
en-tête `Authorization`.** Or `plateforme/src/http/routes-icone.ts:228` fait de
la lecture du porteur **le tout premier contrôle** de la route d'icône, avant
même la lecture de `?e=`, et **toutes** les routes du service exigent un porteur
sauf `/sante`. Un manifeste qui pointerait ces routes ferait, au mieux, une PWA
sans icône ; au pire, un manifeste que le navigateur refuse de charger.

⚠️ **Je qualifie cette porte d'ÉLIMINATOIRE, et ce mot est de moi** : ni la
conception ni le cadrage ne le portent. Il se dérive du fait que le critère ① est
**sans objet** tant qu'elle n'est pas tranchée. C'est un durcissement raisonné,
et il est signalé comme tel — exactement comme D8 avait durci l'inconnue du
changement de mode.

### 3.2 Les trois voies, avec leur coût mesuré

| Voie | Ce qu'elle fait | Décision de sécurité ? | Coût |
| --- | --- | --- | --- |
| **V1 — le manifeste `blob:`** | La page **authentifiée** appelle `GET /applications` et `GET /application/:id/icone?e=…` avec son porteur, construit l'objet de manifeste en mémoire, **encode l'icône en `data:`**, publie le tout en `blob:` et pose `<link rel="manifest" href="blob:…">` | 🔵 **AUCUNE.** Pas une route ne change, pas un contrôle de porteur ne saute | La faisabilité **n'est pas acquise** : c'est ce que P0 mesure |
| **V2 — la route s'ouvre** | `GET /application/:id/icone` et une route de manifeste cessent d'exiger le porteur ; l'URL **est** la capacité, l'`id` étant un **UUID engendré par la plateforme** (`depot/application.ts:145-148`), donc non devinable | 🔴 **OUI, et elle appartient au propriétaire du dépôt** | Retire un contrôle de porteur ; nécessite deux `location` nginx |
| **V3 — une capacité de courte durée** | Un appel authentifié frappe une URL signée et expirante, que le navigateur suit sans en-tête | 🔴 **OUI** — c'est un identifiant dans une URL, qui atterrit dans les journaux et l'historique | Plus l'expiration : **une PWA installée re-cherche son manifeste plus tard**, et une URL expirée le lui refuse |

🔵 **V2 n'est pas une aberration : c'est un motif que ⑤ emploie DÉJÀ deux fois**
— les identifiants TURN (`expiration:session` sous HMAC) et le **préfixe opaque**
de P3. **Mais l'employer ici retire un contrôle existant**, ce qui n'est pas la
même chose que de choisir la forme d'un mécanisme neuf. **Ce plan ne le prend
pas.**

### 3.3 Ce que la porte P0 mesure, exactement

**Instrument** : un Chromium sans interface piloté par CDP, servant une page
locale, **sans agent, sans VM, sans plateforme** — l'objet mesuré est le
**navigateur**, pas le produit.

| Sonde | Ce qu'elle pose | Ce qu'elle relève | Exéc. |
| --- | --- | --- | --- |
| **P0-a** | Une page avec `<link rel="manifest" href="{blob:…}">`, manifeste minimal valide, icône **512×512 en `data:image/png;base64`** | `Page.getAppManifest` — l'**URL** retenue, le **`data`** analysé, et **la liste `errors` verbatim** | **2** |
| **P0-b** | La même, icône **128×128** (sous le seuil) | la liste `errors` — l'erreur d'icône doit **apparaître** | **2** |
| **P0-c** | La même que P0-a, mais manifeste servi par **HTTP ordinaire** (témoin) | les deux relevés se comparent : ce qui diffère est imputable au `blob:` **et à rien d'autre** | **2** |
| **P0-d** | La page P0-a, **sans service worker** | `errors` contient-elle une exigence de service worker ? | **2** |
| **P0-e** | `beforeinstallprompt` : l'événement se déclenche-t-il en `--headless=new` ? | oui / non, écrit tel quel | **2** |

🔴 **P0-b EST LA MOITIÉ QUI COMPTE, ET SANS ELLE P0 NE MESURE RIEN.** Une sonde
qui ne verrait que le cas favorable ne saurait pas dire si `errors` **peut** se
remplir : elle serait verte en ne mesurant rien. C'est le patron que ce dépôt a
payé **six fois** — le F1 de D7, les quatre contrôles vacueux de D10, le témoin
de mesurabilité de P1. **P0-b est le contrôle du contrôle**, et elle est jouée
**en premier**.

### 3.4 Les verdicts, ÉCRITS D'AVANCE

| Relevé de P0 | Verdict | Ce que G5 fait alors |
| --- | --- | --- |
| P0-b remplit `errors`, **et** P0-a la laisse vide | **V1 REÇUE** | tranche D par V1 ; **aucune décision de sécurité n'est demandée** ; G5 est livrable seul |
| P0-b remplit `errors`, **mais** P0-a la remplit aussi (le `blob:` est refusé) | **V1 RÉFUTÉE** | 🔴 tranche D **BLOQUÉE SUR UNE DÉCISION DE SÉCURITÉ** ; le critère ① est déclaré tel, jamais « échoué » ; V2 et V3 sont **présentées au propriétaire du dépôt** avec ce §3.2, et G5 livre B, C, E et F |
| P0-b laisse `errors` vide | 🔴 **P0 NE MESURE RIEN** — l'instrument est faux | l'instrument est repris **avant** toute conclusion. Un relevé qui ne peut pas se remplir n'est pas un relevé |
| P0-d exige un service worker | **la décision 2.3 change de nature** | ce n'est plus un choix : le service worker devient une dépendance de ①, et la tranche D le déclare bloqué plutôt que d'improviser un cache |
| P0-e ne se déclenche pas | **attendu, et sans conséquence** | le critère ① se juge sur `errors`, jamais sur l'invite (§9) |

⚠️ **Portée de P0, écrite d'avance** : **un seul navigateur** (le Chromium de
l'hôte), **sans interface**, **sans profil installé**. Elle ne dit rien de
Firefox, de Safari, d'Edge, ni de ChromeOS. Elle ne dit rien non plus de ce que
fait le **système d'exploitation** avec un handler enregistré : voir §9, critère ③.

---

## 4. Les décisions, tranchées ici

> Chaque décision porte le relevé qui la fonde, ou dit qu'elle n'en a pas.

### D1 — Le manifeste par application est construit CÔTÉ CLIENT, jamais servi par une route

**Voie V1, sous réserve de la porte P0.** Conséquences, toutes favorables et
toutes mesurées :

- **aucune route neuve dans `plateforme/src/http/`** — ce qui compte, parce que
  `routes-installation.ts` est à **500 lignes pour une porte de 500** et
  `serveur.ts` à **432** ;
- **aucun `location` nginx neuf** — et c'est un écueil **mesuré** que G3 a payé :
  `deploiement/nginx.conf:255` termine son `location /` par
  `try_files $uri $uri/ /index.html`, si bien qu'**une route non déclarée rend la
  PAGE en 200**, ce qui est **pire qu'un 404** ;
- **aucune ligne de la plateforme ne change** pour la tranche D.

⚠️ **Ce que V1 coûte, et il faut le dire** : le manifeste n'existe que dans
l'onglet qui l'a construit. Une PWA installée qui re-cherche son manifeste plus
tard trouvera une URL `blob:` morte. **Le comportement de Chromium dans ce cas
n'est pas mesuré par ce plan**, et c'est un legs (§10).

### D2 — 🔴 Ouvrir une route est une DÉCISION DE SÉCURITÉ, et ce plan ne la prend pas

Si P0 réfute V1, **le plan s'arrête sur ce point et le SIGNALE** : il présente
V2 et V3 avec leur coût (§3.2), et **n'en implémente aucune**. La raison n'est
pas la prudence : c'est que G2 a explicitement laissé cette décision ouverte en
écrivant qu'elle « appartient au propriétaire du dépôt », et qu'un sous-bloc
suivant qui la prendrait en silence **la rendrait invisible**.

🔵 **Ce que l'analyse ajoute à ce que G2 en disait, et qui aidera la décision** :
si V2 était prise, ce qu'elle exposerait à un porteur d'UUID est **le `nom` et le
PNG de l'icône, et rien d'autre** — `cible`, `arguments`, `repertoire` et
`chemin` **ne traversent jamais** vers le navigateur (`routes-applications.ts:250-255`),
et l'`id` est un UUID engendré par la plateforme, non dérivé du contenu. **Le
« qui possède l'UUID » est exactement « qui a été authentifié ».** Ce n'est pas
un argument pour prendre la décision ; c'est ce qu'il faut savoir pour la prendre.

### D3 — 🔴 `--accent-fenetre` n'est NI déclaré NI peint par G5, et voici les quatre raisons

A1 lègue à G5 deux choses qu'il déclare **indissociables** : le token n'est pas
déclaré, et rien ne le peint. Son résultat écrit :

> « le jour où G5 écrira `var(--accent-fenetre)` dans une feuille, §7.6 rougira
> sur sa **première** inclusion et le forcera à déclarer le token dans les trois
> blocs — **et il paiera alors la scission de `tokens.css`**, à 300/300. »

**J'ai vérifié les deux moitiés de cette prédiction, et elles sont exactes.**

1. `client/src/design/tokens.ts:112-118` — `tokensReferences` emploie
   `/var\(\s*(--[\w-]+)/g`. **Le repli ne sauve pas** : `var(--accent-fenetre,
   var(--accent))` capture bien `--accent-fenetre`, qui n'est pas déclaré, donc
   l'inclusion ① de §7.6 rougit. *La phrase d'A1 « toute référence future doit
   porter un repli ou déclarer le token » laisse croire que le repli suffit : il
   ne suffit pas.*
2. `client/outils/tokens-orphelins.mjs:147-150` — le périmètre « employé » est
   **toute feuille `.css` sous `client/src/`** plus **les entrées Vite**, moins
   `EXCLUS`. Une feuille de G5 y entre **automatiquement**.

**Les quatre raisons de ne pas le faire** :

- 🔴 **La spec ne le demande pas.** Ce qu'elle demande est « le manifeste
  dynamique par application (icône 256, **couleur d'accent**) », c'est-à-dire le
  `theme_color` d'un manifeste — **une valeur JSON, statique, PAR APPLICATION,
  servie avant toute session**. `--accent-fenetre` est **par FENÊTRE**, arrive
  **en cours de session** sur le canal de contrôle WebRTC, et n'existe ni en base
  ni sur aucune route HTTP. **Ce sont deux choses différentes**, et les
  confondre ferait peindre la mauvaise.
- 🔴 **Le garde WCO de S4 interdit le mécanisme le plus naturel.** Son assertion ②
  (`client/src/style.test.ts:150-157`) refuse **toute**
  `@media (display-mode: window-controls-overlay)` dans `client/src/**/*.css`, et
  sa raison est écrite : « Un repli neutralise un `env()` ; **RIEN ne neutralise
  un bloc `@media`** ». Peindre une bande de titre seulement sous WCO est
  précisément ce qu'un tel bloc exprimerait. *Il reste possible de s'en passer —
  une bande dont la hauteur vaut `env(titlebar-area-height, 0px)` est invisible
  hors WCO — mais alors le coût de la ligne suivante s'applique en entier.*
- 🔴 **Le coût est la scission de `tokens.css`, et il est plus lourd que ce que
  ⑥ en disait** : `CLAUDE.md` annonce **sept** lecteurs qui le nomment par son
  chemin ; l'enquête en compte **NEUF** — les deux de l'écart (`accent.test.ts:30`,
  `accent-dom.test.ts:15`) ayant été **posés par A1 lui-même**, postérieurement à
  S4. **Les neuf, relevés par `grep -rn 'tokens\.css' client/ --include='*.ts'
  --include='*.mjs' --include='*.js'`, `dist/` exclu** — quatre outils de ⑥ et
  cinq `?raw` : `client/outils/contraste.mjs:24`,
  `client/outils/blocs-de-theme.mjs:42`, `client/outils/tokens-orphelins.mjs:74`
  (`const SOURCE`), `client/outils/couleurs-litterales.mjs:205` (le chemin
  d'**exclusion** — il le nomme sans l'ouvrir), `client/src/design/galerie.ts:34`,
  `client/src/design/tokens.test.ts:9`, `client/src/design/reprise.test.ts:4`,
  `client/src/accent.test.ts:30`, `client/src/accent-dom.test.ts:15`.
  🔴 **Scinder le fichier fait rendre ZÉRO déclaration à TROIS des neuf
  contrôles — §7.1, §7.4 et §7.6 —, c'est-à-dire les éteint SANS QU'AUCUN NE
  ROUGISSE**, ce qui est le pire mode de panne de ce socle. Le quatrième outil,
  §7.2, rougirait au contraire bruyamment (son exclusion cesserait de désigner
  quoi que ce soit) : **c'est le seul des quatre qui préviendrait**, et il ne
  faut donc pas compter sur lui pour les trois autres.
- ⚠️ **Et §7.4 imposerait une contrepartie claire** : `tokens.ts:246-251`
  (clause ③, ajoutée par S3) exige que toute **couleur** de la racine soit
  redéclarée dans les **deux** blocs clairs, sauf les **sept** tokens de
  `COULEURS_HORS_THEME`. Déclarer `--accent-fenetre` obligerait à choisir entre
  une contrepartie claire (qui n'a pas de sens pour une couleur venue d'une
  icône) et une huitième entrée hors thème.

**Ce que G5 fait à la place** : rien, et il l'écrit. Le legs n°1 de A1 **reste
ouvert**, reformulé (§10) plutôt que coché.

🔵 **Une tâche de CONTINGENCE est cependant budgétée (§6.7, tâche 20), non jouée**,
pour qu'un successeur n'ait pas à redécouvrir le chemin : elle énumère les neuf
lecteurs, nomme la variante moins coûteuse (extraire la **doctrine** de
`tokens.css` plutôt que ses déclarations, ce qui est une **extraction** et non une
compression), et dit ce qu'elle ne résout pas.

### D4 — Le `theme_color` du hub est ENGENDRÉ depuis `tokens.css`, jamais écrit en dur

`client/public/` **n'existe pas** (mesuré). Un fichier `.webmanifest` posé n'importe
où **échapperait à §7.2**, dont le périmètre est `client/src/**` en `.css`/`.ts`
plus les **entrées Vite** : une couleur littérale y passerait sans qu'aucun
contrôle ne la voie, et le dépôt aurait **deux sources de vérité pour une
couleur**.

**Décision** : un greffon Vite engendre `hub.webmanifest` dans `dist/` en lisant
`--fond-0` et `--accent` par `client/src/design/tokens.ts`. C'est le même
mécanisme, et la même raison, que le greffon `guac-amorce-theme` de
`client/vite.config.ts:52-60` : **Node v24.9.0 importe un `.ts` nativement**, ce
que trois contrôles de ⑥ exploitent déjà.

⚠️ **Deux contraintes mesurées à respecter** :
- `client/vite.config.ts` **n'est pas typechecké** (`:34-40`) — une erreur y est
  un **échec de build**, jamais une erreur `tsc` ;
- tout module de `client/src/design/` importé par un outil doit rester
  **« effaçable »** : pas d'`enum`, pas de `namespace`, pas de décorateur. Le
  retrait de types de Node ne typecheck pas et **refuse** le TypeScript non
  effaçable.

### D5 — La page de hub est la SIXIÈME entrée Vite, et sa feuille vit sous `client/src/`

`client/vite.config.ts:69-73` porte l'avertissement, **mesuré sur `connexion.html`** :
« une page absente de cette liste **ne sort pas du build, et rien ne le dit** ».
`hub.html` est donc **déclarée**, ce qui la fait entrer **automatiquement** dans
§7.2, §7.3, §7.6 et §7.9 ① (leurs périmètres sont dérivés de ce fichier).

**Le style du hub vit dans `client/src/hub/hub.css`, jamais dans un `<style>` en
ligne.** Raison mesurée : §7.7 ne pèse que `client/dist/assets/*.css`
(`poids-css.mjs:60-63`) — **un `<style>` en ligne échapperait au plafond**, et
§7.10 (`longueurs.test.ts:90`) ne balaie que `client/src/**/*.css` hors
`design/`. Mettre le style au bon endroit, c'est **se soumettre à deux contrôles
de plus**, délibérément.

### D6 — `hub.html` n'entre PAS dans `SURFACES_PRODUIT` de §7.9 ② A

`client/outils/classes-employees.mjs:95` porte une liste **en dur** —
`['client/index.html','client/shell.html','client/connexion.html']` — et c'est
**le seul endroit du socle** où une page neuve n'entre pas toute seule.

**Décision : ne pas y toucher.** L'assertion ② A est un **plancher** (« **au
moins une** famille atteint une surface du PRODUIT »), déjà satisfait par trois
surfaces. L'y ajouter **ne changerait aucun verdict**, et ferait franchir à ④ la
frontière d'un outil de ⑥, qui est clos. **Legs nommé** (§10), plutôt qu'une
modification cosmétique à risque.

### D7 — La recette de ①②③ se joue contre une base ENSEMENCÉE, sans la VM

Les trois critères de G5 mesurent **un navigateur** face à **un manifeste**. Rien
n'y exige qu'un agent réel ait produit le catalogue. **Décision** : la base est
peuplée en écrivant par les modules du dépôt eux-mêmes (`depot/application.ts`,
`apps/icones.ts`) — **le code que le chemin de l'agent emprunte**, jamais une
insertion SQL parallèle qui n'éprouverait qu'elle-même.

🔵 **Trois gains, tous mesurables** : la VM est **hors du chemin critique** de
G5 — elle est tenue par G4 et attendue par A1 ; la recette est **rejouable sans
matériel** ; et le corpus est **choisi**, donc l'icône sous le seuil de 192 px du
ROUGE de ① est **posable** (voir D8).

⚠️ **Ce que cela n'établit pas, et qui sera écrit dans le verdict** : aucun agent
réel n'a produit le catalogue de ces exécutions. **Seule la tranche F a besoin
de la VM.**

### D8 — Le ROUGE du critère ① se joue en POSANT une icône sous le seuil, jamais en redimensionnant

La conception écrit : « retirer l'icône du manifeste, ou **la servir en dessous
de 192 px** ». Or le magasin ne connaît **qu'une seule taille** :
`agent/src/apps/icone/extraction.rs:45` — `const COTE: i32 = 256;` — et l'icône
est adressée par le **sha256 de ces octets précis**.

Deux voies étaient possibles ; l'une est **refusée nommément par le dépôt** :
redimensionner côté plateforme exigerait une dépendance de décodage PNG en
TypeScript, que `plateforme/src/base/migrations/0005-icones.sql:43-47` écarte.

**Décision** : la base ensemencée de D7 porte, à côté du corpus normal, **une
application témoin dont l'icône est un PNG 128×128 versé en `testdata`**. Le
ROUGE consiste à bâtir le manifeste de **cette** application. Aucune
transformation d'image, aucune dépendance neuve, et le ROUGE est **jouable à
volonté**.

### D9 — `start_url` pointe la page-shell paramétrée, et la conséquence est déclarée

Un manifeste doit nommer un `start_url` **même-origine et dans son `scope`**.
Trois candidats existent, et aucun n'est parfait :

| Candidat | Ce qu'il donne | Ce qu'il coûte |
| --- | --- | --- |
| `/hub.html?app=<uuid>` | la PWA ouvre le hub, focalisé sur l'application | un clic de plus ; la barre de titre superposée s'applique **au hub**, pas à la fenêtre de session — donc **le legs de S4 n'est pas exercé** |
| `/shell.html?app=<uuid>` | la page-shell établit le contrôle, **lance** l'application, et sa fenêtre de session s'ouvre | la session s'ouvre par `window.open`, donc **dans une SECONDE fenêtre** de la PWA |
| une route de session directe | une seule fenêtre | **impossible** : un `start_url` est **statique**, et un identifiant de session est engendré à l'exécution (`<préfixe>:w-1`) |

**Décision : `/shell.html?app=<uuid>`, `scope: "/"`, et un membre `id` distinct
par application.** Deux raisons :
- c'est le seul candidat où **la fenêtre de session** tourne dans une fenêtre de
  PWA, donc le seul où le legs de S4 — « c'est à SA recette de regarder **la
  fenêtre de session** sous une barre superposée » — puisse être exercé ;
- `scope: "/"` étant partagé, c'est le membre **`id`** qui porte l'identité de
  l'application. ⚠️ **Que deux manifestes de même `scope` et d'`id` distincts
  produisent deux applications distinctes n'est PAS mesurable par ce montage** —
  il faudrait en installer deux —, et cela est écrit dans le verdict.

⚠️ **La conséquence des deux fenêtres est DÉCLARÉE, pas maquillée.** L'alternative
— héberger la session **dans** la page-shell — est une refonte du client, hors
périmètre, et elle est nommée en legs.

⚠️ **La lecture de `?app=` par `client/src/shell-page.ts` est une addition de
quelques lignes** : lire le paramètre, et après l'établissement du contrôle
appeler `POST /application/:id/lancer`. **Elle n'est exercée de bout en bout par
aucun critère**, et le §10 le dit.

### D10 — Les deux chemins de dépôt d'installeur CONVERGENT sur une seule fonction

C'est ce qui rend le critère ② **décidable**. `client/src/hub/televersement.ts`
est **pur, sans DOM, toutes dépendances injectées** (`:4-9`) et **n'a aucun
appelant dans le produit** — mesuré. G5 lui en donne **un seul**, et les deux
sources d'un fichier l'atteignent par lui :

- le **glisser-déposer** (`dragover` / `drop`, `DataTransfer.files`) — et
  `showOpenFilePicker` par un bouton ;
- la **file de lancement** (`launchQueue.setConsumer`), qui n'existe que si le
  navigateur a honoré `file_handlers`.

🔴 **Le ROUGE du critère ② est le retrait du second**, jamais du premier : si le
dépôt cesse de fonctionner quand `launchQueue` disparaît, **la décision de
l'amendement est violée**, et c'est exactement ce que le critère existe pour dire.

⚠️ **`launchQueue` n'existe pas hors PWA installée.** Le code doit donc tester sa
présence, et **le test ne doit pas être un `try`** : un `if ('launchQueue' in
window)` explicite, sans quoi une exception silencieuse rendrait les deux chemins
indiscernables.

### D11 — Les types installeur du manifeste du hub : `.msi`, `.exe`, `.bat`, et ce que Chromium en fait est MESURÉ, pas supposé

L'amendement nomme les trois. `file_handlers[].accept` est une carte
**type MIME → liste d'extensions**. La carte retenue :

| MIME | Extensions |
| --- | --- |
| `application/x-msi` | `.msi` |
| `application/vnd.microsoft.portable-executable` | `.exe` |
| `application/x-bat` | `.bat` |

⚠️ **Ces types MIME sont un CHOIX, pas un standard** — `.msi` et `.bat` n'ont pas
d'enregistrement IANA univoque. Ce qui compte pour le critère ③ n'est pas leur
justesse mais **ce que Chromium répond**, et c'est ce qui est relevé.

### D12 — Les associations par application viennent d'une comparaison d'IDENTITÉ, jamais d'une sous-chaîne de nom

Le modèle que la spec cite, `src/app.js:15-37`, apparie le ProgID à
l'application **par sous-chaîne sur son nom**, après avoir retiré les chiffres —
la même heuristique que G1 a déjà remplacée pour les identifiants, et pour la
même raison (« Nsight 2020.3 » et « Nsight 2024.6 » y produisaient la même clé).

**Décision, pour la tranche F** : l'agent lit, pour chaque extension,
`HKCU\…\FileExts\<ext>\UserChoice\ProgId` en premier (le choix réel de
l'utilisateur), à défaut la valeur par défaut de `HKCR\.<ext>` ; puis
`HKCR\<ProgID>\shell\open\command` ; il en **extrait le chemin d'exécutable**, le
**normalise avec la normalisation déjà écrite** dans `agent/src/apps/raccourci.rs`,
et le compare à `application.cible`. **Une égalité de chemin normalisé, jamais
une sous-chaîne de nom.**

🔵 **La règle de découpage d'une ligne de commande et la comparaison sont PURES**,
donc testables sur l'hôte avec des `testdata`. Seule la lecture du registre est
`#[cfg(windows)]`. C'est la coupure que G2 a déjà établie deux fois
(`apps/icone/ressource.rs`, `apps/installation`), et son intérêt est identique :
sans elle, la partie qui décide n'aurait aucun test d'hôte.

### D13 — Les associations voyagent comme des EXTENSIONS ; le MIME est posé à la génération du manifeste

Faire voyager le MIME depuis l'agent doublerait la table (Rust **et**
TypeScript) pour une donnée qui n'est **pas** une propriété de la VM. **Décision**
: `proto` porte `associations: Vec<String>` (des extensions, minuscules, avec le
point) ; la carte extension → MIME vit **une seule fois**, côté plateforme, à
l'endroit qui écrit le manifeste.

### D14 — Une colonne ajoutée à `application` serait NULLABLE ; les associations vont donc dans une TABLE NEUVE

Fait mesuré et écrit dans le dépôt
(`plateforme/src/base/migrations/0004-applications.sql:4-19`) : sur SQLite 3.50.4,
un `ALTER TABLE … ADD COLUMN … NOT NULL` **sans défaut** est refusé dès que la
table porte une ligne, et `rendreMarqueurs` **interdit tout SQL portant une
apostrophe**, donc tout défaut littéral. La table `application` **n'est plus
vide** (156 clés sur la VM).

**Conséquence, sans alternative** : `accent` (la couleur, tranche F) sera une
**colonne nullable** — ce qui est de toute façon juste, une icône pouvant ne pas
en donner. Les **associations**, elles, vont dans une **table neuve**
`application_association`, qui **peut** naître avec ses `NOT NULL` et sa clé
étrangère.

### D15 — `PLATEFORME_VERSION` passe de 4 à 5, et G5 en est le SEUL propriétaire pendant sa branche

`proto/src/plateforme.rs:122` vaut **4** (mesuré). La structure `Application`
porte `#[serde(deny_unknown_fields)]` : **tout champ neuf est cassant**.

🔴 **La collision est un fait mesuré de ce dépôt, pas une hypothèse** : la
corroboration de P4 a été perdue parce que G1 avait monté la version dans le même
arbre, et l'agent déployé s'est mis à boucler **sans pouvoir lire le refus**.
**La tranche F ne commence pas tant que la tâche 1 n'a pas établi que G4 est clos
et qu'aucun voisin ne touche `proto/`.**

⚠️ **Agent et plateforme se déploient au MÊME commit** — assumé depuis P3 D5, et
repayé par G1.

### D16 — Un `match` exhaustif en aval rend toute variante amont CASSANTE : le bras part dans LE MÊME COMMIT

🔴 **Règle d'ordre qu'aucun plan de ce dépôt ne portait, et que A1 vient de
payer** : A1 a cassé l'arbre entre deux de ses commits parce qu'**aucun des deux
ordres possibles ne compilait**, et cela a coûté une mesure à un voisin.

**Pour G5** : le champ neuf de `Application` (tranche F) et **tous** ses
consommateurs — le miroir TypeScript, les vecteurs partagés, la fusion de
catalogue, le dépôt — **partent dans un seul commit**. **Chaque commit de G5
compile et passe les suites.** Le contrôle est mécanique et il est écrit dans la
tâche : `env -u TURN_URL -u TURN_SECRET ./scripts/verify-all.sh` **après chaque
commit qui touche `proto/`**.

---

## 5. Les divergences entre la conception et le code réel

> Trouvées par la lecture, avant toute écriture. Chacune est **tranchée ici**.

| # | Ce que la conception dit | Ce que le code rend | Tranché |
| --- | --- | --- | --- |
| **E1** | « le manifeste dynamique par application (icône 256, **couleur d'accent**) » | 🔴 **La couleur d'accent n'existe nulle part dans ④** : aucune colonne, aucun champ de protocole, aucun calcul. Celle d'A1 est **par fenêtre**, sur le canal WebRTC | tranche **F** la produit (D3, D14) ; si F est retirée, le manifeste **omet** `theme_color` et le dit |
| **E2** | « les `file_handlers` alimentés par les **vraies** associations lues par l'agent » | 🔴 **L'agent ne lit AUCUNE association** : `grep -rn 'HKEY_CLASSES\|RegOpenKey' agent/src/` rend **zéro**. Le verbe « alimentés » suppose un tuyau qui n'a jamais existé | tranche **F** construit la chaîne entière (D12, D13, D14) — et elle **ne sert aucun critère**, ce qui la place en dernier |
| **E3** | « son icône 256 servie » (critère ①) | 🔴 **La route d'icône exige `Authorization`** (`routes-icone.ts:228`), et un manifeste est chargé **sans en-tête** | porte **P0** (§3) ; V1 par défaut, V2/V3 **signalées, non prises** (D1, D2) |
| **E4** | « la servir en dessous de 192 px » (ROUGE de ①) | ⚠️ **Une seule taille existe**, 256 (`extraction.rs:45`), adressée par le sha256 de **ces octets** ; redimensionner en TS est **refusé** par `0005-icones.sql:43-47` | **D8** : une application témoin à icône 128×128, versée en `testdata` |
| **E5** | « l'ajout des types installeur au manifeste du **hub** » | 🔴 **Il n'existe aucun hub** : ni page, ni entrée Vite, ni manifeste. `client/src/hub/televersement.ts` est **sans appelant** | tranche **B** livre la page ; tranche **C** le manifeste |
| **E6** | critère ② : « le glisser-déposer d'un installeur fonctionne » | ⚠️ L'orchestration existe et est **pure**, mais **n'a jamais tourné dans un navigateur** (legs G3 n°4) | tranche **B**, **D10** : un seul appelant, deux sources |
| **E7** | §10 : « ④ livre un hub fonctionnel ; **⑥ l'habille** » | 🔴 **⑥ est CLOS**, et son legs n°2 dit « le hub n'existe pas ; son contenu dépend de ④ ». **Les deux se renvoient la balle** | **2.3** : G5 emploie le socle et les primitives, **n'invente aucune direction visuelle**, et **ne porte aucun jugement visuel** |
| **E8** | (absent de la conception de ④) | ⚠️ **Le WCO de S4 est légué NOMMÉMENT « à la recette du sous-bloc G5 »** (`style.test.ts:73-77`), et la conception de ④ ne le mentionne à aucune ligne | tranche **E**, avec son verdict écrit d'avance (§9) |
| **E9** | (absent de la conception de ④) | ⚠️ La spec du retrait du legacy range le **service worker** sous G5 (`2026-08-20-retrait-legacy-design.md:364`, verrou **C5**) | **2.3** : hors périmètre, **déclaré**, legs nommé — sauf si P0-d le rend obligatoire |
| **E10** | « ChromeOS est le meilleur candidat de ③, et il n'est pas disponible » | ⚠️ Toujours vrai, **et pire** : l'hôte n'a ni `DISPLAY`, ni `Xvfb`, ni `xdotool` (mesuré par F1 et D8), donc **aucune invite d'installation ne peut être montrée** | §9 : ① se juge sur `Page.getAppManifest().errors`, jamais sur l'invite |
| **E11** | Les legs de G1, G3 et G4 renvoient tous à G5 la divergence `403 vm-etrangere` / `404 vm-inconnue` | ✅ **ELLE EST DÉJÀ TRANCHÉE** : `routes-applications.ts:22-36` porte « LE PROPRIÉTAIRE DU DÉPÔT A RETENU LE REFUS INDISTINGUABLE », et `vm-etrangere` a disparu du fil | **G5 ne la reprend pas.** Les trois legs sont **périmés**, et ce plan le dit plutôt que de les recopier |
| **E12** | Le corpus est de « 218 raccourcis / 154 applications » | ⚠️ Il vaut **220 / 156** depuis G2, qui a laissé **deux raccourcis témoins** sur le Bureau — **à ne pas supprimer**, un de ses critères en dépend | tout énoncé de G5 qui compte **compte des applications, jamais des raccourcis** : le rapport n'est pas de 1 |

---

## 6. Les tranches, les tâches, l'ordre et le parallélisme

### 6.0 Périmètre concurrent, et la VM comme préalable EXTERNE

| Chantier | Ce qu'il tient | État au 21 août 2026 |
| --- | --- | --- |
| **G4** (gestion d'apps) | **la VM**, `agent/src/apps/`, `scripts/run-agent.sh` | 🔄 **NON CLOS** : sa porte S1 est jouée, sa recette tourne, **douze journaux non commités** ; restent le document de résultats, la revue transverse et la section `CLAUDE.md` |
| **A1** (couleur d'accent) | `agent/src/capteur/fenetre*`, `agent/src/transport/tick.rs`, `proto/src/control.*`, `client/src/design/`, `agent/src/accent*` | 🔄 **NON CLOS** : travail hôte fini, **tâches 13 et 14 (instrument et recette) NON FAITES**, en attente de la VM |
| **G5** | `client/hub.html`, `client/src/hub/`, `client/vite.config.ts`, et — **tranche F seulement** — `proto/`, `agent/src/apps/`, `plateforme/src/{depot,apps,base}` | ce plan |

🔴 **LA VM EST UN PRÉALABLE EXTERNE, PAS UNE DÉPENDANCE DE TÂCHE.** C'est la
formulation qui a permis à quatre chantiers de s'arrêter proprement au lieu de
s'écraser le binaire. **Les tranches A à E n'en ont AUCUN besoin** (D7). **Seule
la tranche F en a besoin**, et elle ne commence pas avant que la tâche 1 ait
établi que G4 est clos.

⚠️ **Ne jamais lancer `scripts/build-agent.sh` quand `proto/` porte des
modifications non commitées d'un voisin** : il rsynchronise les sources et
pousserait sur la VM du travail à demi fait.

⚠️ **Purger les DEUX crates** : `cargo clean --release -p proto -p agent`.
Purger `agent` seul ne suffit pas — l'horloge de la VM avance sur celle de
l'hôte, et cargo **saute** le rlib de `proto`. Le symptôme est **une erreur de
type sur une signature de `proto` qui est pourtant à jour dans le fichier qu'on
vient de lire**. **Vérifier la TAILLE du binaire ensuite** : une compilation de
0,13 s est un aveu.

⚠️ **Deux défauts d'hôte, à contrôler avant toute séquence longue** :
- **la VM s'éteint seule.** Déclencheur **identifié** : `libvirtd --timeout 120`
  emporte le domaine. 🔴 **Ce n'est PAS le mécanisme de D1** (l'hibernation côté
  invité, Kernel-Power 187/42) : **les confondre ferait chercher du mauvais
  côté**. Contrôle : `virsh list --all` ;
- **`/dev/null` a été trouvé remplacé par un fichier ordinaire**, cause non
  identifiée. Contrôle : `stat -c '%F' /dev/null`.

### 6.1 Tranche A — la porte P0 (tâches 1 à 3)

| # | Tâche | Livre | Dépend de |
| --- | --- | --- | --- |
| 1 | **Contrôle d'entrée** (§1) | un journal, et rien d'autre | — |
| 2 | **L'instrument de P0** : un pilote CDP, une page témoin, `Page.getAppManifest` | `journaux-gestion-apps-g5/instrument/porte-p0.mjs` | 1 |
| 3 | **P0 jouée**, **P0-b EN PREMIER**, cinq sondes × 2 exécutions, verdict écrit selon la table du §3.4 | les journaux, et le verdict | 2 |

### 6.2 Tranche B — la page de hub (tâches 4 à 8)

| # | Tâche | Livre | Dépend de |
| --- | --- | --- | --- |
| 4 | `client/hub.html` + `client/src/hub/hub.css` + **la sixième entrée Vite** ; la page emploie **socle et primitives**, aucun style en ligne | la surface | 1 |
| 5 | `client/src/hub/catalogue.ts` — **pur, `fetch` injecté** : lit `GET /applications?vm=`, rend une liste ; et l'icône **par `fetch` authentifié puis `createObjectURL`**, seule voie possible (G2) | le module et ses tests | 4 |
| 6 | `client/src/hub/page.ts` — l'orchestration DOM : rendu de la liste, bouton de lancement (`POST /application/:id/lancer`) | le câblage | 5 |
| 7 | **La zone de dépôt**, câblée sur `hub/televersement.ts` — **son premier appelant de produit** | le dépôt par glisser | 6 |
| 8 | **`launchQueue`**, convergeant sur **la même fonction** que la tâche 7 (D10), sous un `if ('launchQueue' in window)` explicite | le second chemin | 7 |

### 6.3 Tranche C — le manifeste du hub (tâches 9 et 10)

| # | Tâche | Livre | Dépend de |
| --- | --- | --- | --- |
| 9 | Le **greffon Vite** qui engendre `hub.webmanifest` en lisant `tokens.ts` (D4), et le `<link rel="manifest">` de `hub.html` | le manifeste du hub | 4 |
| 10 | Les **`file_handlers`** installeur (D11), et le `display_override` du hub | la déclaration | 9 |

### 6.4 Tranche D — le manifeste par application (tâches 11 et 12)

| # | Tâche | Livre | Dépend de |
| --- | --- | --- | --- |
| 11 | `client/src/hub/manifeste.ts` — **PUR** : prend `{nom, id, icônePNG}` et rend l'objet de manifeste ; **aucun DOM, aucun `fetch`** | la règle, testable sur l'hôte | 3, 5 |
| 12 | La publication : `data:` pour l'icône, `blob:` pour le manifeste, `<link rel="manifest">` échangé quand `?app=` est présent (D1, D9) | le câblage | 11 |

⚠️ **La tâche 12 ne s'écrit QUE si P0 a reçu V1.** Sinon elle est **remplacée**
par une tâche de rédaction : le §3.2 est porté au propriétaire du dépôt, et le
critère ① est déclaré bloqué.

### 6.5 Tranche E — le Window Controls Overlay (tâche 13)

| # | Tâche | Livre | Dépend de |
| --- | --- | --- | --- |
| 13 | `display_override: ["window-controls-overlay"]` sur le manifeste par application ; **la lecture de `?app=` par `shell-page.ts`** (D9) ; et **le contrôle que le garde de S4 reste vert** | le legs de S4, exercé ou déclaré non mesurable | 12 |

🔴 **La tâche 13 NE MODIFIE NI `client/src/style.css` NI `client/src/style.test.ts`.**
Elle **pose la condition** qui rend la règle de S4 atteignable, et **relève**. Le
garde de S4 interdit toute `@media (display-mode: window-controls-overlay)`, et
G5 n'en écrit aucune (D3).

### 6.6 Recette de ①②③ (tâches 14 à 16)

| # | Tâche | Livre |
| --- | --- | --- |
| 14 | L'**ensemencement** de la base (D7) et l'**application témoin à icône 128×128** (D8), versée en `testdata` |
| 15 | Le **pilote de recette** — un seul, qui joue les trois critères et le WCO |
| 16 | **La recette**, deux exécutions par critère, verdicts selon le §9 |

### 6.7 Tranche F — l'accent et les associations (tâches 17 à 19), et la contingence (tâche 20)

🔴 **PORTE DE LA TRANCHE F, à évaluer AVANT la tâche 17, et son verdict est écrit
d'avance :**

| Relevé | Verdict |
| --- | --- |
| G4 **clos**, `proto/` sans modification d'un voisin, VM disponible | **F est jouée** |
| G4 **non clos**, ou `proto/` tenu par un voisin | 🔴 **F est RETIRÉE**, et la clause « Livre » de la conception est **annotée** : le manifeste **omet** `theme_color` et ses `file_handlers` par application, et le dit. **G5 reste livrable** — F ne sert aucun critère |
| A1 **non clos** | ⚠️ `agent/src/accent.rs` est à lui : la tâche 17 est **retirée**, la tâche 18 (les associations) peut rester |

| # | Tâche | Livre |
| --- | --- | --- |
| 17 | La **couleur dominante par application** : les pixels RGBA sont lus **avant** l'encodage PNG dans `agent/src/apps/icone/extraction.rs`, et passés à `agent::accent::dominante(rgba, l, h)`. ⚠️ **La conversion BGRA→RGBA existe déjà dans `agent/src/accent/win32.rs` et n'est éprouvée par rien** (legs A1 RA1-6) : **l'extraire en fonction PURE testée**, jamais en écrire une seconde |
| 18 | Les **associations** : le module **PUR** de découpage de ligne de commande et de comparaison d'identité (D12), sa moitié registre `#[cfg(windows)]`, et ses `testdata` |
| 19 | **UN SEUL COMMIT** (D16) : `PLATEFORME_VERSION` 4 → 5, les deux champs de `Application`, le miroir TypeScript, les **vecteurs partagés**, la migration `0007`, `apps/catalogue.ts`, `depot/application.ts`, et la carte extension → MIME (D13) |
| 20 | ⛔ **CONTINGENCE, NON JOUÉE** : la note de scission de `tokens.css` (D3) — les **neuf** lecteurs énumérés par la commande, la variante moins coûteuse nommée, et ce qu'elle ne résout pas |

### 6.8 Clôture (tâches 21 et 22)

| # | Tâche |
| --- | --- |
| 21 | **Revue transverse** : les affirmations devenues fausses **dans la branche elle-même**. Cible propre de G5 : les **cinq** commentaires du dépôt qui disent « il n'existe aucun manifeste » (`style.css:132-142`, `style.test.ts:63,76`, `session/etat-terminal.css:52`, `accent-dom.ts:51`), et la phrase de A1 « toute référence future doit porter un repli **ou** déclarer le token » — **mesurée fausse** (D3, point 1) |
| 22 | **Clôture** : relevé de tailles **APRÈS la dernière édition**, `verify-all.sh` depuis un shell propre, la section `CLAUDE.md`, le document de résultats |

🔴 **Le relevé de tailles se prend APRÈS la dernière édition de la ronde, revue
transverse comprise.** D8 a commis l'erreur inverse en croyant bien faire : une
table mesurée en début de ronde est **fausse à la fin de la même ronde**. Et
⚠️ **la revue transverse est elle-même une source de croissance** — mesurée
**+54 lignes** en S3, **+13** en A1 sur une extraction qui en rendait 23.

### 6.9 Parallélisme

- **1 → 2 → 3** est une chaîne : rien ne commence avant le contrôle d'entrée.
- **B (4→8)** et **C (9→10)** sont indépendantes de **A** au-delà de la tâche 1 :
  elles peuvent courir **pendant** que P0 se joue.
- **D (11→12)** attend **le verdict de P0**.
- **F** attend la recette, et sa porte.

---

## 7. Les portes de taille, budgétées d'avance

| Fichier | Aujourd'hui | Porte | Ce que G5 y ajoute | Marge après |
| --- | --- | --- | --- | --- |
| `client/vite.config.ts` | **112** | 300 | la 6ᵉ entrée (+3) et le greffon de manifeste (~40) | ~145 |
| `client/hub.html` | — | 300 | neuf | — |
| `client/src/hub/hub.css` | — | 300 | neuf | — |
| `client/src/hub/catalogue.ts` | — | 450 | neuf | — |
| `client/src/hub/page.ts` | — | 450 | neuf | — |
| `client/src/hub/manifeste.ts` | — | 450 | neuf | — |
| `client/src/hub/televersement.ts` | **300** | 450 | **rien** — G5 lui donne un appelant, pas des lignes | 150 |
| `client/src/shell-page.ts` | *(à mesurer)* | 450 | la lecture de `?app=` (~15) | — |
| `agent/src/apps/icone/extraction.rs` | **233** | 450 | la lecture RGBA (~35) | ~180 |
| `agent/src/accent.rs` | **214** | 450 | l'extraction pure de la conversion (~30) | ~205 |
| `proto/src/plateforme.rs` | *(à mesurer)* | 450 | deux champs, deux constructeurs, deux cas de vecteurs | — |

🔴 **CE QUE G5 NE TOUCHE PAS, ET POURQUOI** :

- `plateforme/src/http/routes-installation.ts` — **500 pour une porte de 500,
  marge NULLE**. La voie V1 (D1) fait qu'aucune route neuve n'est écrite ;
- `client/src/design/tokens.css` — **300/300, marge NULLE** (D3) ;
- `client/verify-webrtc.mjs` — **494**, marge 6, intouché par tous les chantiers
  de ⑥ et par ④.

⚠️ **Un balayage par la commande à la CLÔTURE, et pas seulement au budget.**
**Sept chantiers récents ont franchi des plafonds sans les voir passer** : D9
deux fois (rattrapés par une **compression** que ce dépôt interdit), D10 trois
fois, le chantier E deux fois. **La règle du dépôt est l'extraction AVANT
l'addition** — la forme forte est une tâche dédiée jouée avant celle qui ajoute
(D9 tâche 6, D10 tâches 1 à 3, P3 tâche 17) ; la forme in-commit suffit quand
l'addition et son extraction tiennent dans une tâche.

---

## 8. Les rouges — le harnais, la ROUGE 0, et une par une

### 8.1 Le harnais, et pourquoi il ne peut pas employer `git diff`

🔴 **Le contrôle « la mutation a-t-elle changé quelque chose ? » NE PEUT PAS être
`git diff --numstat`**, qui compare à **HEAD** et reste donc non vide tant qu'un
travail non commité vit dans le fichier — **même quand la mutation est nulle**.
Mesuré par P3, repayé par A1.

**Le harnais, en huit étapes** :

1. `sha256` du fichier → **une COPIE NOMMÉE**, hors de l'arbre ;
2. `assert` que l'ancre existe **exactement une fois** :
   🔴 **`t.count(ancre) == 1`, jamais une relecture.** A1 a mesuré une ancre
   présente **cinq fois** ; S3 a vu **quatre rouges sur seize** rester vertes ou
   rougir pour la mauvaise raison ; P4 a vu une mutation frapper le
   **commentaire** qui justifie la ligne au lieu de la ligne. **Muter par numéro
   de ligne, ou par un motif ancré sur la syntaxe — jamais par la seule
   sous-chaîne** ;
3. la mutation ;
4. **preuve que le diff CONTRE LA COPIE est non vide** — sinon le harnais
   **REFUSE la rouge** ;
5. le contrôle, **avec `--reporter=verbose`** : 🔴 **le reporter par défaut de
   Vitest DÉDUPLIQUE les échecs et n'en montre qu'un**, si bien qu'un relevé qui
   doit dire **quelle** assertion a rougi ne peut pas s'en contenter ;
6. restauration **depuis la copie** — 🔴 **jamais `git checkout --`**, qui
   restaure à HEAD et a **effacé du travail non commité deux fois** dans ce
   dépôt ;
7. `sha256` **égal** ;
8. ⚠️ `git status --porcelain` sur un fichier **NEUF** rend `??` et non le vide :
   **la preuve de restauration qui vaut est l'étape 7**.

🔴 **UNE « ROUGE 0 » — une mutation qui ne mute rien — est jouée EN PREMIER
devant chaque famille, et le harnais DOIT la refuser.** C'est « un contrôle qu'on
n'a jamais vu rouge n'est pas un contrôle », appliqué **au harnais lui-même**.

### 8.2 Les rouges, une par une

| # | Ce qu'elle mute | L'assertion qui doit tomber | Ce qui la rendrait vacueuse |
| --- | --- | --- | --- |
| **R0** | rien | — | le harnais **refuse** : `DIFF VIDE` |
| **R1** | l'icône du manifeste témoin, 512 → **128** (D8) | `Page.getAppManifest().errors` **se remplit** | si `errors` reste vide, **P0 ne mesure rien** et l'instrument est repris |
| **R2** | retirer entièrement `icons` du manifeste | `errors` nomme l'absence d'icône | idem R1 |
| **R3** | retirer `launchQueue.setConsumer` (D10) | **le glisser-déposer reste VERT** — c'est le critère ② | 🔴 si le dépôt tombe aussi, **l'amendement est violé** et c'est le résultat, pas un échec de rouge |
| **R4** | retirer le `<link rel="manifest">` du hub | le manifeste du hub n'est plus lu ; `file_handlers` disparaît de `getAppManifest` | si `getAppManifest` rend quand même un manifeste, l'instrument lit **une autre page** |
| **R5** | dans `hub/manifeste.ts`, remplacer l'`id` par une constante | le test pur **tombe** : deux applications rendent le même `id` | si le test ne compare qu'un seul manifeste, **il ne peut pas échouer** |
| **R6** | poser `env(titlebar-area-height, 8px)` dans `style.css` | 🔴 le garde de S4 **rougit**, et sa rouge a une **conséquence RÉELLE aujourd'hui** : le bandeau descend de 8 px sur **toutes** les sessions | — |
| **R7** | poser `@media (display-mode: window-controls-overlay)` dans `hub.css` | l'assertion ② du garde de S4 **rougit** | 🔴 **cette rouge éprouve que G5 n'a pas le droit de peindre par `@media`** (D3) |
| **R8** | écrire `var(--accent-fenetre)` dans `hub.css` | §7.6 inclusion ① **rougit** : `var() non déclaré` | 🔴 **c'est la mesure qui FONDE D3**, et elle est jouée **une fois**, puis restaurée |
| **R9** | *(tranche F)* dans le module pur de D12, comparer par **sous-chaîne de nom** au lieu du chemin normalisé | le test d'identité **tombe** sur le cas « Nsight 2020.3 / 2024.6 » | si le `testdata` n'a qu'une application, **le test ne peut pas échouer** |
| **R10** | *(tranche F)* omettre `verifie_version` sur **une seule** variante | **un seul** test tombe — ce qui établit que la vérification est branchée **variante par variante** | si tous tombent, la mutation a frappé autre chose |

⚠️ **R8 est une rouge de DOCUMENTATION** : elle n'accompagne aucune addition.
Elle est jouée pour que la décision D3 repose sur une mesure et non sur une
lecture de code, et son journal est versé.

---

## 9. Les critères, et les verdicts écrits d'avance

⚠️ **Deux exécutions établissent la REPRODUCTIBILITÉ, jamais un TAUX.** Les
contrôles d'hôte de ce dépôt sont déterministes ; la question « combien de fois
sur combien » **ne se pose pas ici** et ne doit pas être empruntée à une campagne
qui, elle, la posait. **Aucun taux n'est prescrit nulle part dans ce plan.**

### Critère ① — une application découverte est installable en PWA

**Jugé sur** `Page.getAppManifest()` : l'URL retenue, le `data` analysé, et
**la liste `errors` verbatim**. **2 exécutions**, plus **2** du ROUGE R1.

| Relevé | Verdict |
| --- | --- |
| `errors` **vide** au vert, **remplie** au rouge R1 | **TENU** |
| `errors` remplie au vert | **NON TENU**, et le message est transcrit verbatim |
| `errors` vide **aux deux** | 🔴 **NON MESURABLE** — l'instrument ne discrimine pas |
| P0 a réfuté V1 | 🔴 **BLOQUÉ SUR UNE DÉCISION DE SÉCURITÉ** (D2), jamais « échoué » |

🔴 **« Le navigateur propose l'installation » N'EST PAS MESURÉ, et le verdict le
dit.** L'hôte n'a ni `DISPLAY`, ni `Xvfb`, ni `xdotool` (mesuré par F1 et D8) :
**aucune invite ne peut être montrée à personne**. Ce qui est mesuré est
l'**absence d'erreur d'installabilité**, ce qui est moins. `beforeinstallprompt`
est relevé pour mémoire (P0-e), sans qu'aucun verdict n'en dépende.

### Critère ② — le glisser-déposer fonctionne sans aucun file handler

**Jugé sur** : un fichier déposé par `DataTransfer` traverse jusqu'à un
scellement accepté, **sur un binaire où `launchQueue` est retiré** (R3).
**2 exécutions par bras.**

| Relevé | Verdict |
| --- | --- |
| le dépôt aboutit **avec et sans** `launchQueue` | **TENU** — l'amendement est gardé |
| le dépôt n'aboutit que **avec** | 🔴 **VIOLÉ** — et c'est le résultat que le critère existe pour produire, pas un échec de recette |

⚠️ **Un `DataTransfer` synthétique par CDP n'est pas un glisser humain.**
`Input.dispatchDragEvent` existe ; **qu'il produise un `drop` porteur de fichiers
n'est pas acquis**, et le plan le dit. Repli déclaré : `showOpenFilePicker` est
lui aussi hors d'atteinte d'un Chromium sans interface (**mesuré par F1** :
aucune commande CDP n'accepte un sélecteur de fichiers). **Le repli qui reste est
d'appeler la fonction de convergence de D10 avec un `File` construit en page** —
ce qui éprouve **le produit**, mais **pas le geste**. La portée est écrite dans
le verdict.

### Critère ③ — le test empirique de l'amendement est joué, et son résultat écrit

La conception est explicite : « **le résultat n'a pas à être positif** ; ne pas
jouer le test, ou n'en écrire que la moitié, est l'échec. »

**Ce qui est mesuré** : Chromium **analyse-t-il** `file_handlers` avec `.msi`,
`.exe`, `.bat`, et que dit-il ? Relevé sur `Page.getAppManifest` : le `data`
retenu (les entrées survivent-elles, toutes, en partie, aucune ?) et les
`errors`/avertissements **verbatim**. **2 exécutions.**

🔴 **CE QUI N'EST PAS MESURÉ, ET QUI EST LA MOITIÉ DU PROBLÈME.** L'amendement
nomme **trois** obstacles ; ce montage n'en éprouve **qu'un** :

| Obstacle de l'amendement | Éprouvé ? |
| --- | --- |
| Chromium accepte-t-il un handler pour un type exécutable ? | ✅ **partiellement** — l'analyse du manifeste, jamais l'enregistrement d'une PWA installée |
| Sur Windows, `.exe` n'est pas une association ; `.msi`/`.bat` sont protégés par le hash `UserChoice` | ❌ **non éprouvé** — il faudrait un poste Windows client |
| Sur Linux/macOS, `.exe` n'a aucune association | ❌ **non éprouvé** |
| ChromeOS, « le meilleur candidat » | ❌ **indisponible** |

**La portée du relevé le dit**, comme la conception l'exige. **Le critère ③ est
TENU dès lors que le test est joué et que son résultat, y compris négatif, est
écrit avec sa portée.** Il ne serait NON TENU que si le test n'était pas joué, ou
si une seule de ses moitiés était écrite.

### Le legs de S4 — le Window Controls Overlay

**Verdict écrit d'avance, et il est probablement défavorable** : le WCO n'existe
que dans une **fenêtre de PWA installée**, et un Chromium sans interface **n'en
installe aucune**.

| Relevé | Verdict |
| --- | --- |
| `Page.getAppManifest` rend `display_override: ["window-controls-overlay"]` **sans erreur** | ✅ **la déclaration est posée** — c'est ce que G5 apporte, et il ne prétend pas à plus |
| `env(titlebar-area-height)` prend une valeur non nulle dans une fenêtre observée | ✅ **le legs est EXERCÉ**, et le dire serait un fait neuf pour ce dépôt |
| aucune fenêtre à barre superposée n'est atteignable | 🔴 **NON MESURABLE PAR CE MONTAGE**, et le legs de S4 **reste entier**, transmis avec sa raison |

🔴 **Un verdict « non mesurable » écrit d'avance vaut mieux qu'un critère
vacueux.** S4 l'a dit en livrant le WCO sans critère : « un critère vacueux est
pire qu'un critère absent : il se lit comme une preuve ». G3 a rendu un
`NON MESURABLE` définitif sur sa porte UAC, G4 sur son critère ② — **et rien de
ce qu'ils avaient écrit n'en présumait.**

---

## 10. Ce que G5 n'établira PAS

- **Aucun taux, nulle part.** Deux exécutions par critère au mieux.
- 🔴 **Rien du WCO en fonctionnement**, sauf si une fenêtre à barre superposée
  devient atteignable — ce que ce plan **n'attend pas**.
- 🔴 **Rien de l'invite d'installation** : l'hôte n'a pas d'interface graphique.
- 🔴 **Rien de l'enregistrement d'un `file_handler` par le SYSTÈME
  D'EXPLOITATION** : ni le hash `UserChoice` de Windows, ni ChromeOS, ni
  macOS/Linux. **Trois des quatre obstacles de l'amendement restent
  non éprouvés.**
- 🔴 **Aucun jugement visuel**, sur aucune page. **Personne n'a jamais ouvert une
  page de ⑥ dans un navigateur, de S1 à S4**, et G5 ne le fera pas davantage. Les
  **vingt-cinq** jugements humains de ⑥ restent entiers, et G5 en ajoute
  — **à compter par `git blame` à la clôture, jamais de mémoire** : S4 en
  prévoyait sept et en a produit dix.
- 🔴 **Rien d'un navigateur autre que Chromium** : ni Firefox, ni Safari. **La
  File System Access API n'existe pas chez eux** — limite du **produit**, pas de
  la recette.
- **Rien du HiDPI** : `deviceScaleFactor = 1`, limite héritée de D5 qu'aucun
  sous-bloc n'a levée.
- **Rien d'une PWA réellement installée** : ni son cycle de vie, ni la
  re-recherche de son manifeste, ni le comportement d'une URL `blob:` morte
  (D1).
- **Rien de deux applications installées côte à côte** : que des `id` distincts
  sous un `scope` partagé donnent deux applications distinctes **n'est pas
  mesurable ici** (D9).
- **Rien du lancement de bout en bout depuis une PWA** : la lecture de `?app=`
  par `shell-page.ts` est livrée, **et exercée par aucun critère** (D9).
- **Aucun service worker**, et donc rien du verrou C5 du retrait du legacy (2.3).
- **`--accent-fenetre` reste sans appelant et sans déclaration**, et le legs n°1
  de A1 **reste ouvert** (D3).
- **Aucune constante calibrée.** G5 en ajoute — les types MIME de D11, les seuils
  d'icône du manifeste — et **aucune ne l'est**. Elles rejoignent la liste que ce
  dépôt tient depuis `BPP_MIN` : `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`,
  `HYSTERESIS`, `TAILLE_MAX_SORTIE`, `ICONE_MAX_OCTETS`,
  `PERIODE_RECONCILIATION`, et les sept d'A1.
- **Aucun audit de sécurité** : le modèle de menace reste celui de ⑤.
- **Rien de la latence**, qu'aucun sous-bloc du chantier D ni du chantier E n'a
  jamais mesurée, depuis D1.
- **Aucun agent réel n'a produit le catalogue** des recettes ①②③ (D7).
- **Le chemin d'extinction propre du superviseur** n'a toujours jamais été
  exercé, depuis D1 — et G5 ne l'exerce pas.

---

## 11. Ce qui rendrait G5 NON LIVRABLE

| # | Ce qui arriverait | Effet | Ce qu'on fait |
| --- | --- | --- | --- |
| **1** | **P0-b ne remplit jamais `errors`** | 🔴 l'instrument ne discrimine pas : **aucun verdict de ① n'est possible**, ni positif ni négatif | l'instrument est repris. Un plan qui conclurait de P0-a seule **fabriquerait une pièce** |
| **2** | **P0 réfute V1** | le critère ① est **bloqué sur une décision de sécurité** | G5 livre B, C, E, F et **déclare** ① bloqué. **Le sous-bloc reste livrable** |
| **3** | **P0-d exige un service worker** | la décision 2.3 cesse d'être un choix | ① est déclaré bloqué sur une dépendance, jamais improvisé |
| **4** | Le glisser-déposer **ne fonctionne que par `launchQueue`** | 🔴 **l'amendement du 28/07/2026 est violé** | **c'est un RÉSULTAT**, écrit tel quel, et il appartient au propriétaire du cadrage |
| **5** | `hub.html` fait rougir un des neuf contrôles de ⑥ | ⑥ est clos ; ④ n'a pas juridiction sur ses outils | **corriger la page**, jamais le contrôle. **Satisfaire un contrôle en le vidant est le geste que ce dépôt combat** |
| **6** | La tranche F ferait franchir 500 à un fichier | dette de taille sur un fichier neuf | **extraction AVANT addition**, sans exception |
| **7** | Un voisin monte `PLATEFORME_VERSION` pendant la branche | 🔴 l'agent déployé **boucle sans terme, sans pouvoir lire le refus** — mesuré par G1 | **F est retirée** (porte du §6.7). Il n'y a pas de demi-mesure |
| **8** | La VM est indisponible | seule **F** en dépend | A à E se jouent quand même (D7) |

---

## 12. Les pièges à ne pas repayer

**Déjà payés par ce dépôt, et qui visent G5 :**

- 🔴 **Une page absente de `vite.config.ts` ne sort pas du build, et RIEN ne le
  dit** (`vite.config.ts:69-73`, mesuré sur `connexion.html`).
- 🔴 **`client/src/design/amorce-theme.js` part VERBATIM dans chaque page bâtie,
  commentaires compris.** Une page neuve **le reçoit sans rien faire** ; et y
  corriger un mot change les octets de **toutes** les pages de `dist/`. **Aucun
  des neuf contrôles ne le dirait** — §7.7 ne pèse que le CSS.
- 🔴 **Un `<style>` en ligne échappe à §7.7 et à §7.10** — d'où D5.
- 🔴 **`test: { css: true }` est obligatoire**, et il n'y a **délibérément pas**
  de `client/vitest.config.ts` : sans lui, un `import css from './x.css?raw'`
  rend la **chaîne vide** et un test qui la parse **passe au vert en ne mesurant
  rien**.
- 🔴 **`grep` sans `-a` sur un journal à queue d'octets NUL rend une sortie
  VIDE, pas un zéro** — indiscernable d'un compte nul (D10).
- 🔴 **Le reporter par défaut de Vitest DÉDUPLIQUE les échecs** : `--reporter=verbose`.
- 🔴 **ZSH ne découpe pas les variables en mots** : une commande passée par `$VAR`
  arrive **entière comme nom de fichier**, code **127** — c'est-à-dire **un
  contrôle qui n'a pas tourné**, indiscernable d'un échec. **Écrire les commandes
  littéralement.** Et `git add $FICHIERS` passe la liste comme **un seul chemin**.
- 🔴 **Des accents graves dans un `echo` ou dans `git commit -m` exécutent une
  commande** — trois phrases mutilées en D11, un script tué au milieu en A1.
  **Passer les messages longs par un fichier** : `git commit -F <fichier>`.
- ⚠️ **Le hook `chpwd` du shell hôte injecte un `ls`** dans toute sortie dès
  qu'un `cd` court en sous-shell : **`unset -f chpwd` avant tout relevé.**
- ⚠️ **`"$var:chemin"` en zsh mange le `:c`** : écrire `"${c}:chemin"`.
- ⚠️ **Un port « libre par convention » ne l'est pas** : relever `ss -ltn` avant
  de choisir. G3 et P4 ont chacun perdu une exécution ainsi.
- ⚠️ **`grep` sur un identifiant de sous-bloc rend des occurrences qui ne sont
  pas les vôtres** : sur neuf occurrences de « G4 », **huit** nommaient les
  gardes G1–G7 du design system. **Trier avant d'écrire.**
- ⚠️ **`git add` NOMINATIF** : l'index porte du travail qui n'est pas le vôtre.
  `git commit -F <fichier> -- <chemins>`, puis **`git show --name-only`**.
- ⚠️ **Un compte de tests n'est attribuable qu'assorti de son HEURE** quand deux
  chantiers partagent l'arbre. A1 l'a payé **trois fois en une journée** :
  944/0, puis 942/2, puis 939/15, puis 954/0 **à la relance sans aucune
  modification**.

**Neufs, propres à ce terrain :**

- 🔴 **Un manifeste n'est pas chargé avec les en-têtes de la page qui le nomme.**
  C'est l'obstacle entier de ce sous-bloc, et il ne se voit dans aucun test de
  Node : une suite entièrement verte peut décrire un manifeste que le navigateur
  refuse. **C'est la classe « ce qu'un navigateur exige et qu'un test serveur ne
  voit pas », que P4 a rencontrée sur CORS et qui n'a TOUJOURS aucun garde
  automatique dans ce dépôt.**
- ⚠️ **`Page.getAppManifest` rend un manifeste même quand la page n'en a pas** :
  vérifier l'**URL** qu'il retourne, et pas seulement le `data`. Sinon la rouge
  R4 lit une autre page et se croit verte.
- ⚠️ **Les `errors` de `getAppManifest` sont des chaînes localisées** : les
  transcrire **verbatim**, ne jamais les apparier par sous-chaîne traduite.

---

## 13. Ce que G5 léguera, quoi qu'il arrive

1. ⛔ **`--accent-fenetre` reste sans déclaration et sans appelant** (D3). Le legs
   n°1 de A1 est **reformulé, pas coché** : sa prédiction est exacte, et sa clause
   « un repli suffit » est **mesurée fausse**.
2. ⛔ **La scission de `client/src/design/tokens.css`** — 300/300, **neuf**
   lecteurs et non sept — reste due au premier chantier qui devra déclarer un
   token. La tâche 20 en verse la note.
3. ⛔ **Le service worker** — destinataire nommé : le chantier de retrait du
   legacy, verrou C5.
4. ⛔ **`hub.html` n'est pas dans `SURFACES_PRODUIT` de §7.9 ② A** (D6).
5. ⛔ **Trois des quatre obstacles de l'amendement du 28/07/2026 restent non
   éprouvés** (§9, critère ③).
6. ⛔ **Le WCO, si la tranche E rend `NON MESURABLE`** : le legs de S4 est
   transmis **avec la raison**, pas éteint.
7. ⛔ **Si P0 réfute V1** : la décision de sécurité V2/V3, portée au propriétaire
   du dépôt avec le §3.2.
8. ⛔ **Si la porte de F se ferme** : `theme_color` et les `file_handlers` par
   application, avec la clause « Livre » de la conception **annotée**.
9. ⚠️ **`CLAUDE.md` porte au moins cinq chiffres périmés sur ⑥ et sur ④** —
   §7.1 **53** paires et non 52, `COULEURS_HORS_THEME` **sept** et non six,
   lecteurs de `tokens.css` **neuf** et non sept, `verify-webrtc.mjs` **494** et
   non 497 (en quatre places datées), corpus **220/156** et non 218/154. **La
   tâche 22 les corrige à LEUR PLACE**, après les avoir énumérées par
   `grep -n` — et **relues place par place APRÈS l'édition**, parce que
   « corrigé à sa place » est une affirmation de **complétude** et qu'une
   substitution qui ne dit pas combien d'occurrences elle a touchées n'en est
   pas une preuve.
