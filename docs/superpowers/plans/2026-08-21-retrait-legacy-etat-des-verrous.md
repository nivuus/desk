# Retrait du legacy — état des verrous au 21 août 2026

**Date du relevé** : 21 août 2026, entre 12:50 et 13:30 UTC.
**Objet** : évaluer, verrou par verrou, la spécification
`docs/superpowers/specs/2026-08-20-retrait-legacy-design.md` (« Retrait du
legacy Guacamole — conditions de mort de l'ancien produit »), écrite la veille.

⛔ **CE DOCUMENT NE RETIRE RIEN, NE MODIFIE RIEN, N'ARRÊTE RIEN.** C'est un
**relevé**. Aucun fichier du legacy n'a été touché, aucun `docker-compose` n'a
été modifié, aucun service n'a été démarré ni arrêté. Le retrait lui-même est
une décision du propriétaire du dépôt, et il n'était pas demandé.

⚠️ **Ce document DÉRIVE, exactement comme celui qu'il évalue.** Chaque nombre
porte sa commande. Ce dépôt a payé neuf fois le naufrage du « 487 » : **ne
recopier aucun chiffre d'ici sans relancer sa commande**.

⚠️ **Convention** : `…/` abrège `docs/superpowers/`.

---

## 0. Le geste d'ouverture rejoué — et il périme le §3 de la spec

La spec §13 prescrit de rejouer son inventaire avant toute chose, et donne
d'avance la conséquence : « **Si ① rend "aucun conteneur", le §3 entier est
périmé et L1 est sans objet** ». Rejoué :

| # | Commande | Relevé du 20 août (spec) | Relevé du 21 août (ici) |
| --- | --- | --- | --- |
| ① | `docker ps -a … \| grep -i guac` | `guacamole-web-1  Up 16 hours` | 🔴 **`guacamole-web-1  Exited (137) 31 hours ago`** |
| ① bis | `docker logs … \| grep -c "restarting due to changes"` | **1 838** en 16 h | **2 933** sur la vie entière du conteneur |
| ② | `curl … http://127.0.0.1:3445/apps` | `200`, corps `[]` | 🔴 **`curl: (7)`, connexion refusée** — `http=000 size=0` |
| ③ | `git log --all --oneline -- web/index.js index.js docker-compose.yml` | vide | **vide** — inchangé |
| ④ | `wc -l index.js src/*.js web/*.js assets/*.html test_winrm_*.js Dockerfile docker-compose.yml package.json` | **3 170** | **3 170** — inchangé, ligne à ligne |

**Le §3 de la spec — « 🔴 Le legacy TOURNE — et il coûte, aujourd'hui, au
nouveau produit » — est donc périmé dans sa prémisse.** Il reste vrai comme
histoire ; il ne décrit plus la machine.

### 0.1 Ce que l'arrêt est, et ce qu'il n'est pas

```
docker inspect guacamole-web-1 --format \
  'OOMKilled={{.State.OOMKilled}} started={{.State.StartedAt}} finished={{.State.FinishedAt}} restarts={{.RestartCount}}'
#   OOMKilled=false started=2026-08-19T08:32:40Z finished=2026-08-20T05:53:54Z restarts=0
```

`OOMKilled=false` et `restarts=0` : le **137** est un `docker stop`/`kill`, pas
une mort accidentelle. **C'est cohérent avec un arrêt volontaire**, et c'est
tout ce que la commande établit — elle ne nomme pas qui l'a demandé.

**Taux réel de redémarrage sur la vie du conteneur** : 2 933 relances en
21 h 21 min = **137,4 par heure**, contre les ~115/h que la spec calculait sur
une fenêtre partielle. Le fait qu'elle décrit était **plus lourd** qu'elle ne
le disait.

Ses derniers mots au journal corroborent ses points ① et ② mot pour mot :

```
docker logs guacamole-web-1 2>&1 | tail -6
#   path: '/media/vm/Users/guacamole/Desktop'      ← ENOENT, catalogue vide
#   guacd[20880]: ERROR: Unable to bind socket to any addresses.   ← guacd orphelin
```

### 0.2 🔴 L'arrêt N'EST PAS DURABLE, et c'est le seul fait neuf de ce §

```bash
docker inspect guacamole-web-1 --format 'RestartPolicy={{.HostConfig.RestartPolicy.Name}}'
#   RestartPolicy=always
grep -n "restart" docker-compose.yml
#   20:    restart: always
```

La politique est **`always`**, pas `unless-stopped`. La sémantique documentée
de Docker pour `always` est qu'un conteneur **arrêté à la main redémarre au
prochain démarrage du démon** — un redémarrage de l'hôte, un
`systemctl restart docker`, une mise à jour de Docker.

⚠️ **Je n'ai PAS éprouvé cette sémantique** : la vérifier demanderait de
redémarrer le démon Docker de cette machine, ce que ce relevé s'interdit. **Ce
qui est mesuré est la valeur de la politique ; sa conséquence est lue dans la
documentation de Docker, pas observée ici.**

**Et la VM est allumée** (`virsh list --all` → `Windows  en cours d'exécution`,
`ls -d /media/vm/Users/guacamole/Desktop` réussit) : si le conteneur revenait,
il trouverait le bureau, `fetchApps()` aboutirait, et les 137 relances/heure
reprendraient leur trafic WinRM vers la machine de recette de tous les
chantiers en cours.

🔵 **Conséquence opérationnelle, et elle est bon marché** : passer
`restart: always` à `restart: unless-stopped` rendrait l'arrêt durable. ⚠️ **Ce
serait une modification de `docker-compose.yml`, que le cadrage §11 interdit
(« l'ancien code n'est pas modifié ») et que la spec de retrait range en L1 au
mieux. Je le nomme ; je ne le fais pas.**

---

## 1. Le tableau des verrous, un par un

**Lecture des états** : `SATISFAIT` / `PARTIELLEMENT` / `NON SATISFAIT` /
`NON ÉVALUABLE`. 🔴 **Un verrou déclaré satisfait sans sa pièce ne compte pas.**
Chaque ligne porte donc son chemin, sa sortie de commande ou son commit.

### Vue d'ensemble

| # | Ce qui peut mourir | État | En une phrase |
| --- | --- | --- | --- |
| **C1** | `web/index.js`, `assets/app.tpl.html`, `dist/` | ✅ **SATISFAIT** | les six fonctions ont leur document de résultats ; aucune des trois clauses de rejet n'est déclenchée |
| **C2** | `src/file.js`, `fuse-native`, `ftp-srv` | ✅ **SATISFAIT sur ses trois clauses** | les huit verbes existent, F1-F4 sont versés, `ERROR_WRITE_PROTECT` est un repli — ⚠️ trois plafonds mesurés qu'aucune clause n'interroge |
| **C3** | `src/app.js`, `src/lnkParser.js`, `src/iconExtractor.js` | 🔴 **NON ÉVALUABLE** | la comparaison n'a **jamais** été jouée, et le service legacy est mort depuis 31 h |
| **C4** | `assets/index.html`, `web/home.js` | 🟡 **PARTIELLEMENT** | le hub existe et sait lancer ; **aucune recette ne le montre lançant une session** |
| **C5** | `src/asset.js`, `web/sw.js` | 🔴 **NON SATISFAIT** | **aucun service worker n'existe**, et le manifeste par application n'a pas de route |
| **C6** | `src/session.js`, `guacamole-lite`, image | 🔴 **NON SATISFAIT** | 31 h d'arrêt sur les 7 jours exigés, et l'arrêt n'est pas durable (§0.2) |
| **C7** | `docker-compose.yml` | 🟡 **PARTIELLEMENT** | le `config` réussit ; **le `up` réel n'a pas été joué**, et la spec dit qu'il ne s'y substitue pas |
| **C8** | `package.json`, `node_modules/` racine | ⚪ **NON ÉVALUABLE avant L5** | sa prémisse est **établie** — `nodejs-winrm` est le seul module externe requis par `scripts/` |
| **C9** | `test_winrm_*.js` | 🟡 **selon la lecture** | satisfaite sur son énoncé, **non satisfaite sur la lettre de sa clause de rejet** |
| **C10** | la connaissance de `CLAUDE.md` | 🔴 **NON SATISFAIT** | **aucun déplacement n'a eu lieu** — les douze blocs sont intacts |

**Compte** : **2 SATISFAITS** (C1, C2), **3 NON SATISFAITS** (C5, C6, C10),
**2 PARTIELS** (C4, C7), **2 NON ÉVALUABLES** (C3, C8), **1 dépendant de la
lecture** (C9).

---

### C1 — la fenêtre de session ✅ **SATISFAIT**

> « La fenêtre de session couvre vidéo, audio descendant, entrée,
> **presse-papier dans les DEUX sens**, et **micro jusqu'à une application
> Windows** »
> Rend NON : un sens du presse-papier non mesuré ; ou E2 non joué ; ou le
> préalable éliminatoire de P2 non levé.

| Fonction | État | Pièce |
| --- | --- | --- |
| vidéo | ✅ | D1→D11, mesuré ; `…/specs/2026-08-20-retrait-legacy-design.md:352` |
| audio descendant | ✅ | chantier A puis D7 ; idem `:353` |
| entrée | ✅ | chantier B, `client/src/input.ts` ; idem `:355` — ⚠️ **le tactile est une régression, voir §4** |
| presse-papier **VM → navigateur** | ✅ **au niveau 1** | `…/plans/2026-08-19-presse-papier-p1-resultats.md` — 4 critères TENUS, 2 exécutions par bras |
| presse-papier **navigateur → VM** | ✅ | `…/plans/2026-08-20-presse-papier-p2-resultats.md` — ①②③④ TENUS ; ⑤ **NON MESURÉ, déclaré** |
| micro **jusqu'à une application Windows** | ✅ | `…/plans/2026-08-20-micro-e2-resultats.md:53-64` |

**Les trois clauses de rejet, une par une :**

1. **« un sens du presse-papier non mesuré »** → **NON DÉCLENCHÉE.** Les deux
   sens ont leur document versé. Un troisième existe même
   (`…/plans/2026-08-21-presse-papier-p3-resultats.md`, N fenêtres, 4 critères
   TENUS), que C1 n'exigeait pas.
2. **« E2 non joué »** → **NON DÉCLENCHÉE.** Le juge
   `micro-ecoute-e2.ps1` ouvre `CABLE Output` par `IAudioClient` en mode
   partagé — **exactement ce que fait une application Windows qui choisit ce
   microphone** — et rend **440,0 Hz** aux trois passages (`e2-juge-v1.log`,
   `e2-juge-v2a.log`, `e2-juge-v2b.log`). 🔵 **Le rouge n'est pas vacueux** :
   sur le binaire d'avant E2, `AMPLITUDE=0,000000`, **et dans la MÊME
   exécution** un témoin joue 660 Hz sur le câble que le juge rend à
   **660,0 Hz**. *Sans cette dernière ligne, « E2 n'écrit pas » et « le juge est
   cassé » se liraient pareil.*
3. **« le préalable éliminatoire de P2 non levé »** → **NON DÉCLENCHÉE.**
   `…/plans/2026-08-20-presse-papier-p2.md:49` (« §0. LE PRÉALABLE
   ÉLIMINATOIRE EST MESURÉ, ET IL EST FAVORABLE »), et la spec porte
   l'annotation à `…/specs/2026-08-19-presse-papier-design.md:1271` : « ✅
   **LEVÉ le 20 août 2026** ». Deux exécutions, matrice de 24 cellules dans une
   seule session, **rouge d'instrument** et **témoin de mesurabilité** compris.

🔴 **LA MATRICE §4 DE LA SPEC EST PÉRIMÉE SUR DEUX DE SES LIGNES, ET UN LECTEUR
QUI S'Y FIERAIT CONCLURAIT L'INVERSE.** Sa ligne 3 porte encore « **E2 NON
COMMENCÉ** — 🔴 NON repris » (`:354`) et sa ligne 5 « 🔴 **NON repris, dans
AUCUN sens** », étayée par un `grep` à **0** (`:356`). Relancé aujourd'hui :

```bash
grep -rniE "clipboard|presse.papier" agent/src client/src plateforme/src proto/src | wc -l
#   586        (la spec relevait 0 le 20 août)
```

⚠️ **Contrôle du zéro** : le même balayage sur une chaîne inexistante
(`zzz_chaine_qui_nexiste_pas_zzz`) rend **0**, et les quatre répertoires
existent — la commande discrimine.

---

### C2 — le pont fichiers ✅ **SATISFAIT sur ses trois clauses**

> « Le pont fichiers **écrit** : les huit verbes de la table du périmètre v1,
> suppression comprise ».
> Rend NON : un verbe absent de `proto/src/fichiers.rs` ; ou F1 toujours sans
> document de résultats ; ou `ERROR_WRITE_PROTECT` décrit encore le régime
> nominal.

**Les huit verbes de la table** (`…/specs/2026-08-19-pont-fichiers-design.md`,
§ « Le périmètre v1, énoncé positivement », l. **419-431**) :

| Verbe | `TYPE_*` | Ligne de `proto/src/fichiers.rs` |
| --- | --- | --- |
| Lister | `TYPE_LISTER = 1` | `:76` |
| Attributs | `TYPE_ATTRIBUTS = 2` | `:77` |
| Lire | `TYPE_LIRE = 3` | `:78` |
| **Existence** | *aucun type dédié* | servie par `TYPE_ATTRIBUTS`, **par décision documentée** — `agent/src/pont/projfs/etat.rs:65-76` : « les deux posent la même question au navigateur, mais ProjFS n'attend pas la même chose en retour » |
| Créer | `TYPE_CREER = 5` | `:80` |
| Écrire | `TYPE_ECRIRE = 4` | `:79` |
| Renommer | `TYPE_RENOMMER = 7` | `:81` |
| Supprimer | `TYPE_SUPPRIMER = 8` | `:82` |

```bash
grep -c 'pub const TYPE_' proto/src/fichiers.rs   #  15
```

**Aucun verbe n'est absent.** ⚠️ **La citation de C2 a dérivé** : elle renvoie
à `…pont-fichiers-design.md:396-407`, qui porte aujourd'hui l'argumentaire
« Pourquoi ce n'est pas un confort » ; **la table est 23 lignes plus bas**
(419-431). Relevé, non corrigé — ce n'est pas mon fichier.

**F1 est clos** : `…/plans/2026-08-19-pont-fichiers-f1-resultats.md` existe.
**F2, F3 et F4 aussi.** Verdicts, avec leur nombre d'exécutions :

| Recette | Verdicts |
| --- | --- |
| **F1** | ① TENU (3 exéc.) ; ② **NON ÉTABLI, et non réfuté** (0 exéc. — le condensat SHA-256) ; ③ **contrôle VACUEUX** ; ④ TENU |
| **F2** | ①②③④⑤ TENUS (2 exéc. chacun) ; ⑥ **NON MESURABLE à ce montage** ; 🔴 une **fenêtre de 30 s** où une écriture reste sans réponse, **non corrigée** |
| **F3** | ①a TENU ; ①b ⛔ **NON LIVRABLE** (renommage de répertoire — ProjFS refuse avant de nous consulter) ; ② TENU ; ③ TENU ; ④ **8 sur 12** |
| **F4** | recette de **MESURE**, aucun critère numéroté. 🔴 **débit plafonné à 30-33 Kio/s**, **rien au-delà de 128 Kio** ; 🔴 **mur du listage** à ~3 150 entrées, mode d'échec = 20 s de gel puis erreur opaque |

**`ERROR_WRITE_PROTECT` ne décrit plus le régime nominal — le code tranche.**
`agent/src/pont/notifications.rs::decider`, l. 274-306 : les trois portes
`PRE_CONVERT_TO_FULL`, `PRE_RENAME` et `PRE_DELETE` **autorisent** en régime
nominal, et ne refusent que sur trois états de banc ou de panne (racine non
inscriptible, mutations désarmées, canal fermé). Les deux drapeaux sont **armés
par défaut**, convention `=0` désarme :

```rust
// agent/src/pont.rs:138 et :154
let ecriture_armee   = std::env::var("PONT_ECRITURE").as_deref() != Ok("0");
let mutations_armees = std::env::var("PONT_MUTATION").as_deref() != Ok("0");
```

⚠️ **MAIS DEUX COMMENTAIRES DISENT ENCORE LE CONTRAIRE, ET L'UN EST L'EN-TÊTE
DU MODULE DONT LE CORPS LES AUTORISE.**

- `agent/src/pont/notifications.rs:14-20` — « **Ce qui reste refusé, et c'est
  nommé** : `PRE_RENAME` et `PRE_DELETE`, parce que `Renommer` et `Supprimer`
  sont des livrables de **F3** ». Le corps du même fichier, l. 293-306, les
  **autorise**.
- `agent/src/pont/erreurs.rs:110-122` — dit que la variante couvre « un
  **renommage** ou une **suppression**, que F2 refuse inconditionnellement », et
  que « `PONT_ECRITURE=0` **ne la produit PAS** ». **Cette seconde clause est
  fausse** : `pont.rs:186` passe `ecriture_armee` en position `inscriptible`.

🔴 **Un évaluateur qui lirait ces en-têtes plutôt que le corps conclurait NON.**
C'est le patron exact que les revues transverses de ce dépôt traquent, et il est
ici **sur le chemin d'une décision de retrait**.

⚠️ **F5 est EN VOL** : `…/plans/2026-08-21-pont-fichiers-f5.md` existe, **aucun
`-resultats.md`**. Neuf commits `(f5)`, un critère sur quatre TENU (`a6de715`),
travail non commité dans l'arbre (`agent/src/pont/projfs/racine.rs` modifié).

🔴 **CE QUE LA GRILLE DE C2 NE VOIT PAS, ET QUI EST LOURD.** C2 mesure
l'**existence** de verbes, jamais leur **viabilité**. Trois plafonds mesurés
n'entrent dans aucune de ses trois clauses : **~33 Kio/s**, **rien au-delà de
128 Kio**, **mur du listage à ~3 150 entrées**. Et **rien dans le dépôt ne
compare les deux ponts** — F1 l'écrit : « Rien de l'ancien pont : ni modifié,
ni retiré, ni **comparé chiffre à chiffre** ».

---

### C3 — le catalogue 🔴 **NON ÉVALUABLE**

> « Le catalogue du nouveau produit rend **au moins autant d'applications** que
> le legacy sur la **même** VM, avec leurs icônes ». Évalué en comparant
> l'ensemble des **noms** à ceux de `curl http://127.0.0.1:3445/apps` **pris le
> même jour**.

**Le catalogue neuf EST mesuré, deux fois** :

- **G1** (`…/plans/2026-08-19-gestion-apps-g1-resultats.md`) —
  `catalogue reconcilie total=218 retenus=154 cles=154`, deux exécutions ;
  `GET /applications?vm=…` rend **154**. Les chiffres `218 / 167 / 154` de la
  spec, d'abord mesurés par `WScript.Shell`, sont **confirmés à l'unité par
  `IShellLinkW`**.
- **G5, tranche F** — catalogue plus récent : `total=220 retenus=169 cles=156
  icones=156 icones_echouees=0`, **156** applications en base.
- **Les icônes sont livrées ET mesurées** (G2) : **96/96** PNG en 256×256,
  **156** applications pour **98** empreintes, **58 téléversements évités**.

**Mais la comparaison exigée n'existe pas, et pour deux raisons superposées :**

1. **Elle n'a jamais été jouée.** Aucun des cinq documents de résultats G ne
   cite `3445` ni `/apps`. Ce n'est pas « elle a échoué », c'est « elle n'existe
   pas ».
2. **Elle ne peut plus l'être aujourd'hui.** Le service legacy est mort depuis
   31 h, le port 3445 est fermé, `curl` rend `(7)`. Et la spec anticipe
   exactement ce cas (`…retrait-legacy-design.md:477`) : « une comparaison
   contre `[]` réussirait toujours, et serait donc **vacueuse** ».

⚠️ **Non évalué n'est pas inévaluable en principe.** L'image
`guacamole-web:latest` (806 Mo) existe toujours, la VM est allumée : la
condition redeviendrait évaluable en relançant conteneur **et** VM le même
jour. **En l'état, prononcer un verdict serait l'inventer.**

---

### C4 — le hub 🟡 **PARTIELLEMENT SATISFAIT**

> « Un **hub** existe : il liste les applications, les lance, et il est servi
> par la plateforme ». Évalué par `ls client/*.html` **et** une recette qui le
> montre lançant une session.

**Première moitié : SATISFAITE.**

```bash
ls client/*.html
#   connexion.html  design.html  hub.html  index.html  primitives.html
#   probe-coalesced.html  shell.html
```

`client/hub.html` est né du commit **`d238413`**. Le module qui le sert est
`client/src/hub/page.ts`, épaulé par `catalogue.ts`, `manifeste.ts`,
`depot.ts`, `televersement.ts`. La page **liste**
(`<ul id="applications" class="hub__grille">`) et **sait lancer** :
`page.ts:181-192` construit un bouton « Lancer » qui appelle
`lancerApplication`, lequel fait `POST /application/:id/lancer`
(`catalogue.ts:230-235`). La liste **a été peuplée pour de vrai** en recette G5,
sur le catalogue réel de la VM, en origine croisée.

**Seconde moitié : NON SATISFAITE, et G5 le dit lui-même** — « **Rien du
lancement de bout en bout depuis une PWA** : la lecture de `?app=` par
`shell-page.ts` est **livrée, et exercée par aucun critère** »
(`…/plans/2026-08-21-gestion-apps-g5-resultats.md:414`, repris en legs n°9 à
`:527`). Le seul lancement mesuré est celui de **G1 ⑤**, par `curl` sur la
route et lecture du journal d'agent — **pas depuis le hub**.

⚠️ **Réserve sur « servi par la plateforme »** : `hub.html` est une entrée
**Vite du client**, servie en statique par nginx dans le profil `deploiement`
(`deploiement/nginx.conf`), pas par le service Node. Selon la lecture, la clause
est satisfaite (même origine derrière le proxy) ou non. **La spec ne le précise
pas, et je ne tranche pas.**

---

### C5 — PWA et service worker 🔴 **NON SATISFAIT**

> « Le nouveau produit sert un manifeste PWA par application **et** un service
> worker ».

**Le service worker : ABSENT, sans ambiguïté.**

```bash
grep -rn "serviceWorker.register|navigator.serviceWorker" client/src client/*.html
#   (aucune ligne)
find client -name 'sw*.ts' -o -name 'sw*.js' | grep -v node_modules
#   (aucun)
```

G5 le confirme deux fois : « **Aucun service worker, et donc rien du verrou C5
du retrait du legacy** » (`…g5-resultats.md:433`), et son legs n°7 nomme **ce
chantier-ci** comme destinataire.

**Le manifeste : livré, mais pas sous la forme que C5 sait évaluer.**

| | État |
| --- | --- |
| manifeste **du hub** | ✅ `client/dist/hub.webmanifest`, engendré au build par le greffon `guac-manifeste-hub` (`client/vite.config.ts:138`), injecté par `<link rel="manifest">` (`:153`), chargé et analysé par Chromium |
| manifeste **par application** | ⚠️ construit en mémoire par `page.ts` et publié en **`blob:`** — **aucune route à `curl`** |

🔴 **La raison est structurelle, et elle est écrite dans
`client/hub.html:22-26`** : « un `<link rel="manifest">` est allé chercher par
le navigateur **SANS en-tête `Authorization`**, et ⑤ ne pose **AUCUN cookie** ».
**La méthode d'évaluation que C5 prescrit — un `curl` sur une route — est donc
inapplicable**, non par négligence mais par une contrainte d'authentification
assumée. G5 mesure autrement (CDP, `Page.getInstallabilityErrors`), et le
résultat est bon (`installabilityErrors: []`, `icons: ["256x256"]`) — **par un
chemin que la condition ne prévoit pas**.

---

### C6 — plus aucune session RDP 🔴 **NON SATISFAIT**

> « Plus aucune session utilisateur n'emprunte le chemin RDP ». Évalué par
> `ss -ltnp | grep 4822` qui ne rend rien **après arrêt volontaire du conteneur
> pendant 7 jours, sans réclamation**.

| Clause | Relevé |
| --- | --- |
| `ss -ltn \| grep -c ':4822 '` | **0** ✅ |
| `ss -ltn \| grep -c ':3445 '` | **0** ✅ |
| processus `guacd` | **aucun** ✅ |
| arrêt volontaire | ✅ `OOMKilled=false`, `restarts=0` |
| **pendant 7 jours** | 🔴 **31,2 h**, soit **1,30 jour sur les 7 exigés** |
| sans réclamation | ⚪ **non observable depuis le dépôt** |

**La fenêtre d'observation est ouverte depuis le 20 août 2026 à 05:53:54 UTC.
Elle se refermerait le 27 août 2026 à 05:53:54 UTC**, si rien ne relance le
conteneur d'ici là.

🔴 **Et §0.2 montre que ce « si » n'est pas garanti** : `restart: always` fait
revenir le conteneur au prochain démarrage du démon Docker. **Une fenêtre
d'observation que le premier redémarrage de l'hôte remet à zéro n'est pas une
fenêtre d'observation.**

⚠️ **La spec le dit elle-même** : C6 « est la seule condition qui ne se réduit
pas à une commande […] La forme retenue est un **protocole**, pas une mesure.
Elle n'est pas pour autant une preuve d'absence d'usage. »

---

### C7 — coturn sans `docker-compose.yml` 🟡 **PARTIELLEMENT**

> « Le relais TURN démarre **sans** ce fichier ». Évalué par
> `docker compose -f docker-compose.coturn.yml up -d coturn` puis un `ps`
> montrant le service **Up**, **et** une session relayée établie.

```bash
docker compose -f docker-compose.coturn.yml config >/dev/null ; echo $?
#   0
```

**Le fichier coturn se parse seul** — mesuré, comme le 20 août. ⚠️ **Mais la
spec écrit noir sur blanc qu'« un `config` qui réussit n'est pas un `up` qui
réussit », et elle a raison.**

🔴 **JE N'AI PAS JOUÉ LE `up`, ET C'EST DÉLIBÉRÉ** : démarrer un conteneur est
une modification de l'état de la machine, que mon mandat exclut. **La moitié
qui décide de C7 n'est donc pas mesurée ici.** La seconde moitié — « une
session relayée est établie » — exige en outre la VM, tenue par le sous-bloc
F5.

**La dépendance documentaire, elle, tient toujours** — quatre places prescrivent
encore le fichier composé :

```bash
grep -rn -- "-f docker-compose.yml -f docker-compose.coturn.yml" CLAUDE.md docker-compose.coturn.yml
#   CLAUDE.md:2424, CLAUDE.md:2425
#   docker-compose.coturn.yml:9, docker-compose.coturn.yml:12
```

Elle « coûte une ligne de correction, pas une refonte » — mais elle n'a pas été
corrigée.

---

### C8 — `package.json` et `node_modules/` ⚪ **NON ÉVALUABLE avant L5**

> « `scripts/winrm.js` fonctionne sans les 20 dépendances legacy ». Évalué
> **après retrait des 20** par `node scripts/winrm.js 'echo ok'` **et**
> `scripts/verify-all.sh` vert.

**C'est une condition POST-retrait : son évaluation exige que le retrait ait
commencé.** Ce qui peut être établi aujourd'hui est sa **prémisse**, et elle
l'est :

```bash
grep -rnE "require\(['\"][^.'\"]" scripts/*.js
#   scripts/winrm.js:3:const winrm = require('nodejs-winrm');
```

**Un seul module externe est requis par tout `scripts/`, et c'est
`nodejs-winrm`.** Il se résout :

```bash
node -e "console.log(require.resolve('nodejs-winrm'))"
#   /home/mallanic/Projects/Guacamole/node_modules/nodejs-winrm/index.js
```

Et les trois sous-projets du nouveau produit ont **leurs propres**
`package.json` et `node_modules` — `client/` (41 paquets), `plateforme/` (56),
`proto/` (38) — donc aucun ne dépend du `node_modules` de la racine.

Les quatre scripts qui pilotent la VM en dépendent par `scripts/winrm.js` :
`build-agent.sh`, `run-agent.sh`, `stop-agent.sh`, `check-session.sh`.

⚠️ **`node scripts/winrm.js 'echo ok'` N'A PAS ÉTÉ LANCÉ** : la commande parle à
la VM, que F5 tient. ⚠️ **`scripts/verify-all.sh` N'A PAS ÉTÉ LANCÉ non plus** :
l'arbre porte du travail non commité de F5 (`agent/src/pont/projfs/racine.rs`
modifié, une vingtaine de journaux non suivis), si bien que son verdict
mesurerait l'état d'un chantier voisin et ne serait **attribuable à personne**.
C'est le piège que ce dépôt a payé en P2 et en G1.

---

### C9 — les essais WinRM 🟡 **selon la lecture, et les deux sont données**

> « Aucun document de recette en cours ne les nomme ».
> Rend NON : **une occurrence hors de la présente spec**.

```bash
grep -rn "test_winrm" docs/ scripts/ CLAUDE.md --include="*.md" --include="*.sh"
```

rend **13 lignes**, dont **10 dans la spec de retrait elle-même** et **3
ailleurs**, toutes trois dans `CLAUDE.md` :

| Place | Nature |
| --- | --- |
| `CLAUDE.md:1375` | index documentaire — « `test_winrm_nodejs.js` - NodeJS WinRM testing » |
| `CLAUDE.md:1376` | index documentaire — idem pour `test_winrm_fixed.js` |
| `CLAUDE.md:16208` | **une commande** — `docker exec -it guacamole-web-1 node test_winrm_nodejs.js`, dans « Commandes de Développement Essentielles » |

- **Sur l'ÉNONCÉ** (« aucun document de **recette en cours** ») : ✅
  **SATISFAITE** — aucune des trois n'est une recette ; ce sont des entrées
  d'index et une commande d'exploitation.
- **Sur la LETTRE de sa clause de rejet** (« une occurrence hors de la présente
  spec ») : 🔴 **NON SATISFAITE** — il y en a trois.

⚠️ **Les trois vivent dans des blocs que L4 et L5 retirent de toute façon.**
Ce verrou se referme mécaniquement avec C10, sans travail propre.

---

### C10 — la connaissance de `CLAUDE.md` 🔴 **NON SATISFAIT**

> « Ce qui est **vrai du domaine** a été déplacé hors des sections legacy ».
> Rend NON : un fait de la liste close du §6.2 n'a pas de nouvel emplacement.

**Aucun déplacement n'a eu lieu.** Les douze blocs du §6.2 sont **tous
présents**, aux mêmes titres :

```bash
grep -n '^## ' CLAUDE.md
```

| Bloc | Bornes actuelles | Lignes |
| --- | --- | --- |
| `## Project Overview` | 7-10 | 4 |
| `## Development Commands` → `## Known Constraints` (4 sections) | 1204-1390 | **187** |
| Problèmes Résolus | 1391-1537 | 147 |
| Problèmes Connus Non Résolus | 1538-1678 | 141 |
| Système de Build | 1679-1723 | 45 |
| Configuration Critique | 1724-1771 | 48 |
| Parser `.lnk` | 1772-1839 | 68 |
| Extraction d'icônes | 1840-1941 | 102 |
| Flux de lancement | 1942-2098 | 157 |
| Commandes de Développement | 16151-16219 | 69 |
| Checklist de Déploiement | 16220-16267 | 48 |
| Roadmap et TODO | 16268-16327 | 60 |
| Ressources et Références | 16328-16378 | 51 |
| **Total** | | **1 127** |

**1 127 lignes sur 16 378, soit 6,9 %.** La spec relevait **1 113 sur 9 932,
soit 11,2 %** : **l'absolu a crû de 14, le ratio a chuté parce que `CLAUDE.md`
a grossi de 6 446 lignes** en un jour de chantiers concurrents. **Les deux
nombres sont vrais à leur date ; c'est le ratio qui trompe.**

**Les deux blocs que le §6.2 marque « 🔴 CONSERVÉ — vrai du domaine »** — le
parseur `.lnk` (68 l.) et l'extraction d'icônes (102 l.) — **sont toujours là,
et nulle part ailleurs.**

⚠️ **Et le sous-projet ④ a rendu l'un des deux partiellement redondant sans
l'archiver** : G1 lit les raccourcis par `IShellLinkW` et G2 extrait les icônes
en 256×256, mais **la comparaison des cinq voies d'extraction avec leurs taux
de succès n'existe toujours que dans `CLAUDE.md` et dans
`src/iconExtractor.js`**.

---

## 2. Ce qui peut partir aujourd'hui, et ce qui ne le peut pas

### 2.1 Le tableau des blocs L1-L5

| Bloc | Condition d'entrée | État de la condition | Verdict |
| --- | --- | --- | --- |
| **L1** — cesser de nuire | aucune | — | 🟡 **première moitié SANS OBJET, seconde moitié DUE** |
| **L2** — archiver | L1 | L1 exécutable | 🟢 **EXÉCUTABLE — et il ne supprime rien** |
| **L3** — retirer la surface morte | C1 ∧ C2 ∧ C4 ∧ C5 ∧ C6 ∧ C7 | ✅ ✅ 🟡 🔴 🔴 🟡 | ⛔ **BLOQUÉ** — C5 et C6 franchement, C4 et C7 partiellement |
| **L4** — retirer la découverte | C3 ∧ C10 | 🔴 🔴 | ⛔ **BLOQUÉ** |
| **L5** — solder | C8 ∧ C9, et L3 ∧ L4 achevés | ⚪ 🟡, et L3/L4 bloqués | ⛔ **BLOQUÉ** |

### 2.2 🔴 Ce qui peut être SUPPRIMÉ aujourd'hui : **RIEN**

Et ce n'est pas une prudence de ma part, c'est la conclusion de la spec
elle-même, qui a **vérifié** ses deux candidats « évidents » et les a refusés
tous les deux (son §8) :

- **`web/fs.js`, 4 lignes**, sans aucun appelant — vérifié à nouveau
  aujourd'hui, toujours orphelin. Refusé : « le gain est nul, le précédent
  coûteux » ;
- **`test_winrm_*.js`, 246 lignes** — C9 satisfaite sur son énoncé. Refusé pour
  la même raison, le cadrage §11 interdisant de toucher l'ancien code.

**Je ne rouvre ni l'un ni l'autre.** Un relevé n'a pas mandat de renverser une
décision argumentée de la spec qu'il évalue.

### 2.3 🟢 Ce qui peut être FAIT aujourd'hui, et qui ne supprime aucune ligne

**① La seconde moitié de L1 — inscrire la dette de `web/index.js`.** Elle est
**due, et mesurée due** :

```bash
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
#   1536 agent/src/encode.rs
#    630 agent/src/windows_source.rs
```

**Deux entrées.** Or `CLAUDE.md:20` met `web/` **dans la portée** de la règle
des 500 lignes, et `web/index.js` fait **804 lignes**. Il n'y figure pas parce
que la commande énumère `git ls-files` et `git ls-files --others
--exclude-standard` : **un fichier IGNORÉ lui est structurellement invisible.**

```bash
{ git ls-files; git ls-files --others --exclude-standard; } | grep -c '^web/index.js$'
#   0
```

⚠️ **La table de dette a donc DEUX entrées là où le texte de sa propre portée en
impose TROIS.** Ce n'est pas une dette à purger — c'est un **constat à
inscrire**, et la spec le porte en L1 « parce qu'une portée qui ment est plus
coûteuse qu'un fichier trop long ».

**② La première moitié de L1 — borner `nodemon` — est SANS OBJET**, le
conteneur étant arrêté. `nodemon.json` n'existe pas, et il n'y a plus rien à
borner. ⚠️ **Elle redeviendrait due à la seconde où le conteneur reviendrait**
(§0.2).

**③ L2 tout entier — l'archivage.** `docs/legacy/` **n'existe pas**
(`ls -d docs/legacy` → absent). C'est le geste que la spec rend **bloquant
avant L3**, et il est exécutable dès aujourd'hui : il **ajoute** des fichiers,
il n'en retire aucun.

🔴 **Et le §3 ci-dessous montre que c'est le geste le plus urgent du chantier,
parce que le seul dont le report soit IRRÉVERSIBLE.**

---

## 3. 🔴 Les 890 lignes hors de git — le compte réel

### 3.1 Le fait, remesuré

```bash
git log --all --oneline -- web/index.js index.js docker-compose.yml
#   (vide — aucun commit, sur aucune branche)
git check-ignore -v index.js web/index.js docker-compose.yml
#   .gitignore:18:index.js          index.js
#   .gitignore:18:index.js          web/index.js
#   .gitignore:19:docker-compose.yml docker-compose.yml
```

**Vérification fichier par fichier**, par `git ls-files --error-unmatch` sur les
19 fichiers du legacy — **trois sont hors git, et trois seulement** :

| Fichier | Lignes | Suivi ? |
| --- | --- | --- |
| `web/index.js` | **804** | 🔴 **HORS GIT** |
| `index.js` | **61** | 🔴 **HORS GIT** |
| `docker-compose.yml` | **25** | 🔴 **HORS GIT** |
| **Total hors git** | **890** | |
| les 16 autres | 2 280 | suivis |
| **Total legacy** | **3 170** | |

**Le compte de la spec est EXACT, à la ligne près, et inchangé en un jour.**

⚠️ **Le motif `.gitignore:18` n'a pas d'ancre de début de chemin** : il attrape
`web/index.js` à n'importe quelle profondeur. Sa raison est écrite juste
au-dessus (`.gitignore:14-17`) et elle est **bonne** — `index.js` porte des mots
de passe Windows en clair. C'est sa **portée** qui est trop large.

### 3.2 Ce qui serait perdu — et ce n'est pas seulement du code

**Une suppression de `web/index.js` détruit définitivement 804 lignes.** Elles
sont le seul endroit du dépôt où vivent, à l'échelle d'un produit qui a servi :

1. le contournement `getPixelColor` pour la couleur de titre (`:767`) ;
2. l'usage de `windowControlsOverlay` (`:684-716`) ;
3. la lecture du presse-papier client sous permission (`:348-393`) ;
4. **le transport des formats riches du presse-papier** (`:330-345`), que le
   nouveau produit range hors périmètre v1 par décision ;
5. **les handlers tactiles vivants** (`:271`, `:282`, `:295`).

🔴 **Et elles sont CITÉES, avec leurs numéros de ligne, par des documents que le
dépôt conserve :**

```bash
# ⚠️ CE DOCUMENT-CI cite lui aussi `web/index.js` : sans l'exclure, la commande
#    mesurerait sa propre écriture. Les trois comptes sont pris SANS lui.
EXCL='2026-08-21-retrait-legacy-etat-des-verrous.md'
grep -rl 'web/index\.js' docs/superpowers/ --include='*.md' | grep -v "$EXCL" | wc -l   #  11
grep -r  'web/index\.js' docs/superpowers/ --include='*.md' -o | grep -v "$EXCL" | wc -l  #  77
grep -r  'web/index\.js:[0-9]*' docs/superpowers/ --include='*.md' -o | grep -v "$EXCL" \
  | sed 's/.*:\(web.*\)/\1/' | sort -u | wc -l                                         #  29
```

**77 occurrences dans 11 documents, dont 29 citations à numéro de ligne
distinct** — ce document-ci exclu. ⚠️ **Avec lui, les mêmes commandes rendent
102, 12 et 30** : *un document qui compte des citations en produit, et ne peut
pas se compter lui-même honnêtement.* Supprimer le fichier rend les 29 **invérifiables**. Les plus lourdes
vivent dans les specs, pas dans les journaux :

- `…/specs/2026-08-19-pont-fichiers-design.md` — **treize** citations, dont la
  table §« ce que l'ancien pont faisait mal » (`:1393-1405`), qui oppose ligne à
  ligne les deux ponts. Elle est **la seule pièce du dépôt qui documente le
  défaut de zone morte temporelle de `web/index.js:631`** ;
- `…/specs/2026-08-19-presse-papier-design.md` — **dix**, dont la description
  du geste de l'ancien produit que P2 refuse d'emprunter ;
- `…/specs/2026-08-19-plateforme-design.md:181` et
  `…/specs/2026-07-28-spike-multifenetres-design.md:55`.

⚠️ **`index.js` (62 occurrences) et `docker-compose.yml` (36) sont cités de
même**, et sont hors git au même titre.

### 3.3 Ce que le temps a changé aux deux « références vivantes » de la spec

Le §6.1 nommait deux sous-blocs pour lesquels `web/index.js` était **la
référence de travail**, pas seulement un passé. **Les deux ont eu lieu depuis :**

| Référence | État au 20 août (spec) | État au 21 août |
| --- | --- | --- |
| le défaut de renommage de répertoire, seule pièce empêchant **F2** de le réimplémenter | ouverte | ✅ **consommée** — F3 est clos, et son critère ①b déclare le renommage de répertoire ⛔ **NON LIVRABLE** |
| le repli `readText()` avec permission, seule implémentation de référence si le préalable de **P2** échouait | ouverte | ✅ **consommée** — le préalable est **LEVÉ**, le repli n'est **pas** emprunté |

🔵 **L'argument le plus opérationnel de la spec pour « L2 précède L3 » a donc
été soldé par les faits.** Ce qui reste est l'argument d'archive, et il est
entier : **77 citations vers un fichier qui n'existe dans aucun commit.**

### 3.4 🔴 La décision, et à qui elle appartient

**Elle appartient au propriétaire du dépôt, et je ne la prends pas.** Ce qui est
établi et qui la cadre :

- **« l'historique git reste » est FAUX pour 890 lignes sur 3 170** — mesuré,
  deux jours de suite ;
- le geste d'archivage (L2) **ajoute** des fichiers et n'en supprime aucun : il
  est compatible avec le cadrage §11, qui interdit de **modifier** l'ancien
  code ;
- il exige une **rédaction**, pas une copie : `index.js:2-4` et
  `docker-compose.yml:15,17` portent des mots de passe en clair, et l'archive
  doit porter la structure sans les valeurs — **et le dire** ;
- ⚠️ **cet archivage ne purge pas les secrets de la machine** : `.env` et le
  `docker-compose.yml` vivant les portent toujours ;
- ⚠️ **la mine du motif `index.js` n'a pas encore mordu** — aucun fichier du
  nouveau produit ne s'appelle `index.js` :

```bash
find . -name 'index.js' -not -path '*/node_modules/*' -not -path './target/*' -not -path './.claude/*'
#   ./index.js  ./web/index.js  ./src/dist/index.js  ./dist/index.js
find . -name 'index.ts' -not -path '*/node_modules/*' -not -path './target/*' -not -path './.claude/*'
#   ./plateforme/src/index.ts  ./spike-multifenetres/src/index.ts
```

🔵 **Le point d'entrée de la plateforme s'appelle `index.ts`, pas `index.js` :
il échappe au motif.** La mine est **armée et non déclenchée** — un futur
`client/src/index.js` serait ignoré **en silence**.

---

## 4. 🔴 Ce que le remplacement NE COUVRE PAS

**C'est la partie la plus utile de ce document.** Un retrait qui laisse un trou
non nommé est pire qu'un retrait différé. Ce qui suit rassemble, en une liste,
ce que le legacy faisait ou portait et qui **n'a aucun équivalent mesuré** dans
le nouveau produit.

⚠️ **Trois catégories, et il ne faut pas les confondre** :
**A — une fonction disparaît** ; **B — la fonction existe mais rien ne l'a
mesurée** ; **C — la fonction existe, est mesurée, et un plafond mesuré la borne
là où le legacy n'était pas borné (ou pas mesuré du tout)**.

### 4.1 Catégorie A — des fonctions qui disparaissent

| # | Ce qui part | Pièce | État |
| --- | --- | --- | --- |
| **A1** | 🔴 **Le service worker.** Le legacy en enregistrait un (`web/index.js:418-423`, scope `/${appName}/`) et le servait (`web/sw.js`, 45 l., `src/asset.js:106`) | `grep -rn "serviceWorker.register" client/src client/*.html` → **aucune ligne** ; `…g5-resultats.md:433` : « Aucun service worker, et donc rien du verrou C5 » | **AUCUN repreneur.** G5 en fait un legs **vers ce chantier-ci** |
| **A2** | 🔴 **Les formats riches du presse-papier** — images, fichiers, RTF, HTML. `web/index.js:330-345` construit `new ClipboardItem({ [mimetype]: blob })` **quel que soit le mimetype** | `…/specs/2026-08-19-presse-papier-design.md:1245-1248`, §9 « Hors périmètre v1, explicitement » | **RÉGRESSION DÉCIDÉE** (décision D4). La raison est le **canal**, pas l'API : le canal de contrôle est ordonné et partagé, une image y bloquerait le curseur en tête de file. Borne `PRESSE_PAPIER_MAX = 64 KiB`, **non calibrée**, avec **refus visible** |
| **A3** | ⚠️ **Le tactile.** `src/session.js:216` pose `enable-touch: true`, et **les handlers étaient VIVANTS** : `touch.onmousedown` (`web/index.js:271`), `touch.onmouseup` (`:282`), `touch.onmousemove` (`:295`) | voir 4.4 ci-dessous | **RÉGRESSION NON DÉCIDÉE — aucune spec du nouveau produit ne nomme le tactile** |
| **A4** | ⚠️ **L'impression.** `src/session.js:197` pose `enable-printing: true` | cadrage `…/specs/2026-07-27-refonte-produit-design.md:300` | **RÉGRESSION CONNUE, DATÉE ET ASSUMÉE** — rangée en v2+. ⚠️ Elle n'a jamais été rapprochée du fait que l'ancien produit l'avait |
| **A5** | ⚠️ **Le renommage de répertoire** | `…/plans/2026-08-20-pont-fichiers-f3-resultats.md`, critère ①b : ⛔ **NON LIVRABLE** — ProjFS refuse avant de consulter le fournisseur | 🔵 **PARITÉ, PAS RÉGRESSION** : l'ancien ne le faisait pas non plus (`web/index.js:631`, zone morte temporelle, `ReferenceError`). **Les deux échouent ; le nouveau bruyamment, l'ancien silencieusement** |
| **A6** | ⚠️ **`statfs`** — le legacy annonçait `1000000000` en dur (`web/index.js:517`) et `104857600` blocs (`src/file.js:173-185`) | `…/specs/2026-08-19-pont-fichiers-design.md:471` | 🔵 **AMÉLIORATION, PAS PERTE** : « des chiffres inventés » remplacés par le volume réel de la VM — « le chiffre est vrai, mais il décrit **le mauvais disque** : une limite, dite, pas un mensonge » |

### 4.2 Catégorie B — livré, jamais mesuré

| # | Ce qui n'est pas mesuré | Pièce |
| --- | --- | --- |
| **B1** | 🔴 **Personne n'a jamais collé.** Le NIVEAU 2 du presse-papier VM → navigateur — « un humain peut-il coller dans une application locale ? » — est **NON MESURABLE** sur ce montage : `xclip` et `wl-paste` **absents**, `xsel` refusé en `Can't open display: (null)` | `…/plans/2026-08-19-presse-papier-p1-resultats.md:83`, reconduit par P3 |
| **B2** | 🔴 **Personne n'a jamais écouté le micro.** E2 est jugé par un juge **logiciel** ; le §13 de la spec (jugement humain) n'est pas atteint | `…/plans/2026-08-20-micro-e2-resultats.md:38-44` |
| **B3** | 🔴 **Le hub n'a jamais été montré lançant une session** — c'est la clause manquante de C4 | `…g5-resultats.md:414`, legs n°9 |
| **B4** | 🔴 **`showDirectoryPicker()` n'a JAMAIS été appelé.** Toutes les recettes du pont emploient **OPFS** : ni l'appel, ni le modèle de permission (`queryPermission`/`requestPermission`), ni le mode `readwrite`, ni l'activation utilisateur transitoire | F1, F2, F4 |
| **B5** | 🔴 **Aucun éditeur réel n'a exercé l'idiome temporaire + renommage + suppression** — « la lacune la plus lourde du sous-projet », **et c'est l'argument même par lequel la spec du pont justifie d'inclure la suppression au périmètre** | F3, reconduite par F4 |
| **B6** | 🔴 **L'écho micro en multi-fenêtres.** E3 est **bloqué sur deux choses distinctes** : la **VM**, tenue par F5, et un **consentement** sur le graphe audio de l'hôte qui n'a pas été donné | `…/plans/journaux-micro-e3/familles-de-lecture.txt` — le répertoire « ne contient AUCUN journal d'agent » |
| **B7** | ⚠️ **L'exclusivité micro à deux enfants** n'est **toujours pas** exercée | `…/plans/2026-08-20-micro-e2-resultats.md:472-476` |
| **B8** | ⚠️ **Le Window Controls Overlay n'a jamais été RENDU.** Chromium **accepte et retient** la déclaration (`displayOverrides: ["kWindowControlsOverlay", …]`), mais `matchMedia('(display-mode: window-controls-overlay)').matches` vaut **`false`** — les variables `env(titlebar-area-*)` ne sont définies que dans une PWA installée | `…g5-resultats.md:88-92` — **DEMI-MESURE**, et le legs de S4 reste entier |
| **B9** | ⚠️ **Les `file_handlers` ne sont jamais HONORÉS.** Chromium les **analyse** (établi par une sonde : une `action` hors `scope` fait rendre `FileHandler ignored`), mais **aucune PWA n'a été installée** — 🔴 **et le hub n'est PAS installable, faute d'icône** (`manifest-missing-suitable-icon`). « Un hub qu'on ne peut pas installer ne peut **jamais** voir ses `file_handlers` honorés » | `…g5-resultats.md` §6, legs n°1 |
| **B10** | 🔴 **La latence de bout en bout n'est mesurée par RIEN**, et ne l'a **jamais** été — ni par un sous-bloc du chantier D depuis D1, ni par E1/E2, ni par le pont | constat repris de section en section dans `CLAUDE.md` |
| **B11** | ⚠️ **Aucun jugement visuel n'a été porté sur ⑥, d'un bout à l'autre** — aucune page du design system n'a été ouverte dans un navigateur, ni en S1, ni en S2, ni en S3, ni en S4 | section S4 de `CLAUDE.md` |
| **B12** | ⚠️ **La porte UAC de G3 n'a pas été jouée** — la VM était tenue par le presse-papier P2. Cinq observations **NON PRISES** | `…/plans/2026-08-20-gestion-apps-g3-resultats.md:9-20` |

### 4.3 Catégorie C — mesuré, et borné là où le legacy ne l'était pas

| # | Plafond | Pièce |
| --- | --- | --- |
| **C-a** | 🔴 **Le pont plafonne à 30-33 Kio/s**, et **aucun fichier de plus de 128 Kio n'est lisible** sur le binaire livré — 256 Kio et 1 Mio **échouent**. Cause attribuée **par mutation** à `ATTENTE_MAX` : « le facteur est 2, pas 20 », terme dominant et non unique | `…/plans/2026-08-21-pont-fichiers-f4-resultats.md:220-231`, `:299-305`, `:233-256` |
| **C-b** | 🔴 **Mur du listage** : `N_max` = **3 150** entrées aboutit, `N_mur` = **3 200** échoue ; mode d'échec = **20 s de gel puis erreur opaque**. F4 le porte « comme un défaut de produit » | idem `:113-146`, `:369-382` |
| **C-c** | 🔴 **F2 laisse une fenêtre de 30 s** où une écriture reste sans réponse **et où le compteur affiche `dues: 0` alors que six attendent**. **Non corrigée**, legs n°16 | `…/plans/2026-08-20-pont-fichiers-f2-resultats.md:95-116` |
| **C-d** | ⚠️ **Le condensat SHA-256 de bout en bout du pont n'est NI établi NI réfuté** — le critère que F1 désigne comme « le seul qui ne puisse pas être satisfait par accident », **0 exécution** | `…/plans/2026-08-19-pont-fichiers-f1-resultats.md:138` |
| **C-e** | ⚠️ **Le manifeste par application est un `blob:` mort** : il n'existe que dans l'onglet qui l'a construit. Le comportement de Chromium quand une PWA installée re-cherche son manifeste **n'est mesuré par rien** | `…g5-resultats.md`, legs n°3 |
| **C-f** | ⚠️ **Le rééchantillonnage 48 000 → 44 100 de VB-Cable** subsiste, hors de notre code et hors de toute mesure — et son remède est **inapplicable** (il casse le point de terminaison, `0x88890008`) | `…/plans/2026-08-20-micro-e2-resultats.md` |
| **C-g** | 🔴 **RIEN NE COMPARE LES DEUX PRODUITS.** F1 l'écrit pour le pont : « Rien de l'ancien pont : ni modifié, ni retiré, ni **comparé chiffre à chiffre** ». Aucun sous-bloc, dans aucun sous-projet, n'oppose une mesure de l'ancien à une mesure du nouveau — **et le §10 de la spec de retrait l'annonçait** : « La matrice compare **du code lu** à **des recettes documentées**, jamais deux produits en fonctionnement » | — |

### 4.4 🔴 Le tactile : ce que j'ai relevé, et une erreur que j'ai failli écrire

**La spec §4.3 classait le tactile en « alerte, pas constat d'absence de
code ».** J'ai voulu la resserrer, et **j'ai d'abord conclu à l'envers.**

En lisant `web/index.js:181-235`, j'ai vu `var touch = new
Guacamole.Mouse.Touchscreen(...)` (`:183`) suivi de **trente lignes de handlers
`touch.*` COMMENTÉS** (`:204-219`), et j'allais écrire que *le tactile du legacy
était mort lui aussi*. **C'était faux** : le bloc commenté est une version
antérieure, et les handlers vivants sont **soixante lignes plus bas** :

```bash
grep -nE "^\s*touch\.on" web/index.js
#   271:    touch.onmousedown = function (touchState) {
#   282:    touch.onmouseup = function (touchState) {
#   295:    touch.onmousemove = function (touchState) {
```

**Le tactile du legacy était vivant.** *Lire une fenêtre et conclure est
exactement le geste que ce dépôt paie depuis D8 ; je l'ai commis et rattrapé par
un `grep` ancré.*

**Côté nouveau produit, mesuré :**

```bash
grep -rnE "touchstart|touchmove|touchend|TouchEvent|maxTouchPoints" client/src agent/src proto/
#   (aucune ligne)
grep -rn "pointerType" client/src | wc -l
#   0
```

⚠️ **Témoin positif** : la même forme de commande sur `pointerdown` rend trois
fichiers — **elle discrimine**.

**Ce que le nouveau produit a** : des `PointerEvent`
(`client/src/input.ts:95,109,115`, `client/src/pointer.ts`), qui **se
déclenchent aussi pour le tactile**. **Ce qu'il n'a pas** : aucune
discrimination `pointerType`, donc **aucun geste multi-doigts, aucun défilement,
aucune sémantique tactile** ; et le chemin de souris **relative** passe par
`document.pointerLockElement` (`input.ts:99`), **indisponible sur un appareil
tactile**.

🔵 **Formulation resserrée, à substituer à l'alerte de la spec** : *le legacy
avait un chemin tactile dédié et vivant ; le nouveau produit reçoit
incidemment les événements tactiles par `PointerEvent`, ne les distingue pas de
la souris, et son mode de souris relative leur est inaccessible. Aucune spec ne
nomme le tactile, et aucune recette ne l'a exercé.*

### 4.5 🔴 L'état persistant qui SURVIT au retrait

Le cadrage écrit « Migration des données de l'ancien système : aucune ». **C'est
exact pour l'état SERVEUR et inexact pour l'état CLIENT** — la spec de retrait
l'avait déjà relevé, et rien n'a changé :

| État | Où | Vérifié aujourd'hui |
| --- | --- | --- |
| montages FUSE | `/mnt/ftp-{uuid}` | ✅ **aucun résidu** — `ls -d /mnt/ftp-*` ne rend rien, `grep -c 'ftp-' /proc/mounts` → **0** |
| sessions, base, artefacts de build | en mémoire / régénérés | ✅ rien à migrer |
| 🔴 **`localStorage` du navigateur** | `web/home.js:5,14,22`, clé `installedApps` | 🔴 **SURVIT** — hors d'atteinte de toute suppression serveur |
| 🔴 **service worker enregistré** | `web/index.js:418-423`, scope `/${appName}/` | 🔴 **SURVIT** — un SW enregistré survit à la mort de son serveur |

⚠️ **Elles ne se migrent pas : elles se DÉSARMENT, et le seul moment où on peut
le faire est PENDANT QUE LE SERVEUR VIT ENCORE** — un service worker ne se
désenregistre que depuis une page servie sous son scope. **Le serveur est arrêté
depuis 31 h : cette fenêtre est déjà fermée**, sauf à le relancer exprès.

🔵 **Ce qui rend le risque faible aujourd'hui, et c'est mesuré** : le nouveau
produit **n'enregistre aucun service worker** (`grep` = 0, §4.1 A1), et il est
servi sur d'**autres origines** — plateforme sur `8080`, proxy nginx sur
`8443` dans le profil `deploiement` — quand le legacy servait sur `3445`.
**Origines différentes, donc pas d'interception.** ⚠️ **Le risque redeviendrait
réel si le nouveau hub était un jour servi sur le même hôte et le même port.**
Et **la gravité n'est toujours pas mesurée** : aucun navigateur porteur d'un SW
legacy n'a été inventorié.

---

## 5. Deux relevés qui m'étaient donnés, et ce que la mesure en dit

### 5.1 « Le conteneur legacy a été arrêté par décision du propriétaire »

**Confirmé quant au fait, non attribuable quant à l'auteur.**
`docker ps -a` rend `Exited (137) 31 hours ago` ; `OOMKilled=false` et
`restarts=0` établissent un arrêt **volontaire** (un `docker stop` dont le
délai de grâce a expiré, ou un `docker kill`). **La commande ne dit pas qui l'a
demandé** — je le rapporte comme reçu.

🔴 **Ce que la mesure ajoute, et qui n'était pas dans le relevé reçu** :
`restart: always` fait que **cet arrêt n'est pas durable** (§0.2). C'est le seul
fait de ce document qui appelle un geste, et il en appelle un petit.

### 5.2 « Le profil de déploiement de ⑤ remplace une partie de l'infrastructure »

**Confirmé, et il est en place :**

```bash
ls -1 deploiement/
#   nginx.conf  plateforme.env.exemple  postgres.env.exemple  README.md
grep -nE "^  [a-z-]+:|profiles:" docker-compose.plateforme.yml
#   132:  postgres-deploiement:   133:    profiles: [deploiement]
#   189:  plateforme:             190:    profiles: [deploiement]
#   254:  proxy:                  255:    profiles: [deploiement]  (nginx:alpine)
```

**Trois services sous `profiles: [deploiement]`** — Postgres, la plateforme,
et un proxy nginx qui termine TLS. **Ce que cela remplace du legacy** : le
service HTTP (`index.js`, port 3445) et son image `guacamole-web` (806 Mo).

⚠️ **Ce que cela NE remplace pas, et il faut le dire** : ni `guacd`, ni le
chemin RDP, ni le pont FUSE, ni la découverte d'applications. **Le profil
`deploiement` remplace l'enveloppe, pas le produit.**

⚠️ **Et il ne tourne pas en profil `deploiement` aujourd'hui** : ce qui tourne
est `guacamole-postgres-plateforme-1` (le Postgres de **test**, `Up 2 days`) et
un `tsx src/index.ts` de la plateforme sur le port **8080**, lancés à la main.
**Aucun conteneur du profil `deploiement` n'est démarré.**

✅ **Aucune collision de port entre l'ancien et le nouveau produit** —
`3445`, `4822` et `8080` sont distincts, et les deux premiers sont **fermés**.

---

## 6. Ce que ce relevé N'ÉTABLIT PAS

- **Il n'a lancé AUCUNE commande vers la VM Windows.** F5 la tient, E3 l'attend.
  Tout ce qui exige la VM — C3 (la comparaison des catalogues), la seconde
  moitié de C7 (une session relayée), la moitié `winrm` de C8 — est **non
  mesuré ici**.
- **Il n'a PAS lancé `scripts/verify-all.sh`.** L'arbre porte du travail non
  commité de F5 : son verdict mesurerait un chantier voisin et ne serait
  **attribuable à personne**. C'est le piège payé en P2 et en G1.
- **Il n'a PAS lancé `docker compose … up -d coturn`**, ni redémarré le démon
  Docker, ni relancé le conteneur legacy — démarrer un service est une
  modification de l'état de la machine que le mandat exclut.
- **Il ne mesure aucun taux.** Les commandes de ce document sont
  **déterministes** et ont été lancées **une fois chacune**, sauf indication
  contraire. Aucune fréquence n'est revendiquée nulle part.
- **Il n'a ouvert AUCUNE session** — ni RDP par le legacy, ni WebRTC par le
  nouveau produit. **La comparaison des deux produits en fonctionnement reste
  entière**, et le §10 de la spec l'annonçait.
- **Il n'a lu aucun des sept core dumps** (390 Mo). Le
  `double free or corruption` reste **non diagnostiqué**, exactement comme le
  20 août. La spec décide qu'ils partent **avec** le legacy et pas avant — je ne
  rouvre pas.
- **Il ne dit rien de `spike-multifenetres/`, de `target/`, ni des paquets
  non-legacy de `node_modules`.**
- **Il ne réécrit ni la spec, ni `CLAUDE.md`, ni aucun fichier du produit.** Les
  énoncés périmés qu'il relève (la matrice §4 de la spec, les deux commentaires
  de `agent/src/pont/`) sont **nommés, non corrigés** — ce sont les fichiers
  d'autres chantiers.
- **Il n'a pas vérifié la sémantique de `restart: always`** par l'expérience
  (§0.2) : elle est lue dans la documentation de Docker.

---

## 7. Ce que ce relevé a coûté en pièges, pour le suivant

- 🔴 **UN ZÉRO SE QUALIFIE AVANT DE SE RAPPORTER, ET J'AI FAILLI EN PUBLIER
  VINGT ET UN.** Un balayage des 21 dépendances de `package.json` a rendu
  « appelants hors legacy : 0 » **pour les 21**, avec un
  `(eval):9: bad math expression` noyé dans la sortie : le quotage zsh avait
  cassé le `grep`. **Les 21 zéros étaient des artefacts de commande.** Refait
  par un script en fichier, avec les deux colonnes (legacy / hors-legacy) côte à
  côte — et le résultat juste est qu'**un seul** module est partagé.
- 🔴 **ET LE MÊME PIÈGE UNE SECONDE FOIS, DANS LA VÉRIFICATION FINALE DE CE
  DOCUMENT.** Voulant contrôler que les treize bornes de `CLAUDE.md` n'avaient
  pas bougé sous un chantier voisin, j'ai écrit
  `grep -n '^## ' CLAUDE.md | grep -E ':(7|1204|…):'` — qui exige un `:` **avant**
  le numéro, quand `grep -n` le met en tête de ligne. **Il rend 0**, et un 0 dans
  ce contexte se lit « les bornes ont toutes bougé ». Refait par
  `awk -F: '{print $1}' | grep -xE …`, avec un **témoin** (`99999`, qui rend 0) :
  les treize sont intactes. **Deux zéros d'artefact dans un seul relevé, sur des
  commandes écrites par la même main qui venait d'écrire la règle.**
- 🔴 **LIRE UNE FENÊTRE ET CONCLURE** — le tactile (§4.4). Trente lignes de
  handlers commentés à `:204-219` cachaient trois handlers vivants à `:271-295`.
  **Un `grep` ancré (`^\s*touch\.on`) l'a rattrapé ; une lecture de plus ne
  l'aurait pas fait.**
- ⚠️ **UN TÉMOIN NÉGATIF À CÔTÉ DE CHAQUE CONTRÔLE PAR CHAÎNE.** Les trois
  balayages du §4.1 de la spec rendaient **0** le 20 août. Rejoués **à
  l'identique** : le premier (presse-papier) rend **586**, le deuxième
  (`.lnk`/catalogue) **409**, le troisième (`webmanifest|serviceWorker` sur
  `client/src client/*.html`) **2** — et **2 seulement**, parce que le
  manifeste PWA vit sous `client/src/hub/`, que ce motif-là n'atteint pas ; un
  balayage élargi y rend **26**. Pour être sûr qu'un zéro ne serait pas un
  chemin cassé, la même commande a été lancée sur une chaîne inexistante — elle
  rend **0** — et les quatre répertoires ont été listés.
- ⚠️ **`grep` SUR UN IDENTIFIANT REND CE QUI N'EST PAS À VOUS.** « P1 » et
  « P2 » désignent **à la fois** les sous-blocs de la plateforme (⑤) et ceux du
  presse-papier (①) ; « G1 » à « G5 » désignent la gestion d'apps (④), mais
  `grep 'G4'` rend surtout les gardes du design system (⑥). **Trier avant
  d'écrire.**
- ⚠️ **UNE CITATION `fichier:ligne` DÉRIVE.** C2 renvoie à
  `…pont-fichiers-design.md:396-407` pour la table des huit verbes ; ces lignes
  portent aujourd'hui autre chose, et la table est **23 lignes plus bas**.
- ⚠️ **UN RATIO TROMPE LÀ OÙ L'ABSOLU NE TROMPE PAS.** Les blocs legacy de
  `CLAUDE.md` passent de **11,2 %** à **6,9 %** en un jour — non parce qu'ils
  ont maigri (+14 lignes) mais parce que le fichier a grossi de **6 446
  lignes**. **Comparer les absolus.**
- ⚠️ **`unset -f chpwd` AVANT TOUT RELEVÉ**, sans exception : le shell de cet
  hôte injecte un `ls` dans la sortie dès qu'un `cd` court dans un sous-shell.

---

## 8. Synthèse

**Sur les dix verrous : deux sont satisfaits, trois ne le sont pas, deux le sont
partiellement, deux ne sont pas évaluables, et un dépend de la lecture qu'on
donne à sa clause de rejet.**

**Aucun bloc de retrait n'est débloqué.** L3 attend C4, C5, C6 et C7 ; L4 attend
C3 et C10 ; L5 attend les deux. **Rien ne peut être supprimé aujourd'hui**, et
c'est aussi la conclusion de la spec, qui avait déjà vérifié et refusé ses deux
candidats évidents.

**Deux gestes sont exécutables et ne suppriment aucune ligne** : inscrire au
tableau de dette de `CLAUDE.md` que `web/index.js` (804 l.) est **hors de portée
de la commande de vérification** — mesuré, la table a deux entrées là où la
portée en impose trois — et **archiver** sous `docs/legacy/`, qui n'existe pas.

🔴 **L'archivage est le seul geste dont le report soit IRRÉVERSIBLE.** Les
**890 lignes** hors de git sont confirmées à la ligne près, deux jours de suite,
et **77 citations réparties dans 11 documents** pointent vers `web/index.js`,
dont **29 avec un numéro de ligne**.

🔵 **Deux bonnes nouvelles que le temps a apportées, et qui n'étaient pas
acquises le 20 août** : les onze fonctions que la spec déclarait « reprises par
personne » sont **majoritairement reprises et mesurées** — le presse-papier dans
les deux sens et à N fenêtres, le micro jusqu'à une application Windows, le
catalogue, les icônes, le hub, le manifeste PWA. Et les **deux références
vivantes** de `web/index.js` — le défaut de renommage de répertoire pour F2, le
repli `readText()` pour P2 — ont été **consommées** par F3 et par le préalable
levé de P2.

🔴 **Trois trous ne sont repris par personne, et ils ne sont pas de même
nature** : le **service worker** (aucun code, aucun sous-bloc — G5 en fait un
legs vers ce chantier-ci), les **formats riches du presse-papier** (régression
**décidée**, pour une raison de canal), et le **tactile** (régression **non
décidée**, que personne n'a jamais nommée dans une spec du nouveau produit).

⚠️ **Et le remplaçant du pont fichiers porte trois plafonds mesurés — 33 Kio/s,
128 Kio, 3 150 entrées — qu'AUCUN verrou du retrait n'interroge.** C2 mesure
l'existence de verbes, jamais leur viabilité, et **rien dans ce dépôt ne compare
les deux ponts**.
