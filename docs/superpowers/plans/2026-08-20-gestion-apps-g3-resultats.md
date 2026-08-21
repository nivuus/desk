# Sous-bloc G3 — le téléversement, l'exécution, et son issue : résultats

**Sous-projet ④ Gestion d'apps.** Plan :
`docs/superpowers/plans/2026-08-20-gestion-apps-g3.md`. Conception :
`docs/superpowers/specs/2026-08-19-gestion-apps-design.md`. Journaux :
`docs/superpowers/plans/journaux-gestion-apps-g3/`.

---

## §1 — 🔴 LA PORTE UAC : **NON JOUÉE**, et rien n'en présume

**C'est le §1 parce que c'est ce qu'on viendra y chercher.** Le plan fait
dépendre le périmètre de G3 du verdict de sa porte — l'élévation UAC est-elle
capturée par Desktop Duplication ? — et **cette sonde n'a pas été exécutée.**

**La raison, et elle est de disponibilité, pas de méthode** : la VM Windows est
une **ressource exclusive**, et elle a été tenue tout du long par le chantier
presse-papier P2, qui n'avait plus que sa recette à jouer. Elle s'est en outre
éteinte d'elle-même une fois pendant le sous-bloc (piège documenté depuis D1).

**Ce que cela laisse ouvert, nommément :**

| Observation | État |
| --- | --- |
| (a) le témoin positif — l'instrument sait-il observer ? | **NON PRISE** |
| (b) `consent.exe` vivant — une élévation a-t-elle eu lieu ? | **NON PRISE** |
| (c) la mesure — la boîte de dialogue est-elle capturée ? | **NON PRISE** |
| (d) `OpenInputDesktop` discrimine-t-il ? | **NON PRISE** |
| (e) `CreateProcess` rend-il `ERROR_ELEVATION_REQUIRED` (740) ? | **NON PRISE** |

🔴 **L'OBSERVATION (e) EST LA PLUS COÛTEUSE À LAISSER OUVERTE, et elle est
écrite dans le code plutôt que supposée.** Elle est la prémisse de la décision
D15 — `CreateProcessW` plutôt que `ShellExecuteExW` —, et c'est **une lecture de
la documentation Windows, pas une mesure de ce dépôt**. L'en-tête de
`agent/src/apps/installation/execution.rs` le dit en toutes lettres : si elle est
réfutée, la branche `elevation-requise` sera **inatteignable plutôt que fausse**,
et le refus typé devra être reconstruit sur un autre indice.

**C'est exactement ce que le libellé de `MF_E_UNSUPPORTED_D3D_TYPE` a coûté à ce
dépôt le 31 juillet 2026** : dix annotations de contexte pour découvrir que le
message parlait du type d'*entrée* quand l'appel refusé réglait le type de
*sortie*.

⚠️ **AUCUNE LIGNE DE CE SOUS-BLOC NE PRÉSUME DU VERDICT.** Le plan écrit
d'avance ce que chaque issue commande, précisément pour qu'on n'arbitre pas sous
le coup du résultat, et le code livré est **le même dans les quatre cas** — ce
qui change est le périmètre *déclaré* d'installeurs utilisables, et le §9 du
document que la recette écrira.

---

## §2 — Ce qui est LIVRÉ, et ce qui est MESURÉ

**Il faut distinguer les deux, et ce document ne les mélange nulle part.**

| | Livré | Éprouvé sur l'hôte | Éprouvé sur la VM |
| --- | --- | --- | --- |
| le protocole v4 | ✅ | ✅ vecteurs partagés, deux langages | ⛔ |
| le découpage en tranches, l'empreinte JS | ✅ | ✅ dont un oracle INDÉPENDANT | ⛔ |
| les six modules purs de l'agent | ✅ | ✅ | ⛔ |
| le téléchargement HTTP | ✅ | ✅ **contre un vrai `TcpListener`** | ⛔ |
| l'exécution Windows, le garde de job | ✅ | ⛔ `cargo check` seul | ⛔ |
| la trace du périphérique audio | ✅ | ⛔ `cargo check` seul | ⛔ |
| le câblage des deux fils | ✅ | ⛔ **aucun test ne le couvre** | ⛔ |
| la migration, les deux dépôts | ✅ | ✅ **double passe** | — |
| le magasin de tranches | ✅ | ✅ double passe | — |
| les routes | ✅ | ✅ double passe | ⛔ |
| la réémission à l'enrôlement | ✅ | ✅ | ⛔ |
| l'orchestration du navigateur | ✅ | ✅ sans DOM | ⛔ |
| le profil de déploiement | ✅ | ⛔ **AUCUN nginx**, mesuré | ⛔ |

🔴 **AUCUNE LIGNE DE CE SOUS-BLOC N'A TOURNÉ SUR LA VM WINDOWS.** La recette
(tâche 34) et les deux sondes (tâches 1 et 2) restent entières.

---

## §3 — Les défauts que le travail a trouvés, et qui n'étaient pas cherchés

Ce sont eux qui valent d'être lus : ils ne figuraient dans aucune tâche.

### ① Une PANIQUE, dans le code que rien ne pouvait tester

La borne du journal d'installeur était écrite **deux fois** — sur des octets
dans `execution.rs`, sur une `String` dans `fil.rs`. La seconde faisait
`&texte[texte.len() - N..]` **sans vérifier la frontière de caractère UTF-8**.

**Le chemin était atteignable** : `String::from_utf8_lossy` **AGRANDIT** — un
octet invalide devient un U+FFFD de trois octets —, si bien que la première
borne pouvait rendre une chaîne plus longue que la borne, et la seconde coupait
alors au milieu d'un caractère. *Un installeur qui écrit du Latin-1 sur sa
sortie standard aurait suffi.*

**Ce n'est pas un échec d'assertion, c'est une panique** : le fil serait mort
**sans rapporter d'issue**, la ligne serait restée `en_cours` en base — donc
**même la réémission ne l'aurait pas reprise**, elle qui ne vise que les
`en_attente` — et le hub aurait affiché « en cours » pour l'éternité.

Rouge jouée, verbatim :
`start byte index 131072 is not a char boundary; it is inside '<U+FFFD>'`

⚠️ **La leçon n'est pas « attention à l'UTF-8 » : c'est qu'une borne écrite deux
fois est une borne qui diverge.**

### ② Une inefficacité que seul le test pouvait montrer

Recevant un `200` en réponse à un `Range`, ma première rédaction **fermait la
connexion et en rouvrait une troisième** pour redemander depuis zéro — alors que
le corps entier était déjà en train d'arriver. **Sur un installeur de 800 Mo,
cela aurait fait passer le fichier DEUX FOIS sur le lien.**

Le symptôme n'avait rien d'évident : le faux serveur n'offrait que deux
réactions, et la troisième connexion a été refusée. **Le test disait
« Connection refused » là où le défaut était « une connexion de trop ».**
Corrigé dans le PRODUIT ; le test compte désormais les connexions.

### ③ Un défaut que seul le typecheck de `client/` voyait

L'extraction de `plateforme-installation.ts` a laissé deux imports orphelins
dans `proto/ts/plateforme.ts`.

```
cd proto  && npm run typecheck  ->  0 erreur
cd client && npm run typecheck  ->  TS6192, DEUX fois, sur CE fichier
```

**Deux paquets typechèquent le même fichier avec deux sévérités, et seul le plus
sévère dit la vérité.** C'est aussi la raison pour laquelle `verify-all.sh`
lance trois `typecheck` et non un.

### ④ Un défaut que seule la passe Postgres voyait

`installation.test.ts` appelait `baseNeuve()` **sans argument**. Sous SQLite le
harnais ouvre `:memory:` et n'a que faire du nom ; sous Postgres il en dérive un
**nom de schéma** et lève sur `undefined`. **Les onze cas passaient d'un côté et
échouaient tous de l'autre.**

Ce n'est pas une divergence de dialecte — c'est le **harnais** qui diffère —,
mais la leçon est la même : *une suite qui ne tournerait que sur un moteur aurait
déclaré vert un fichier incapable de tourner sur l'autre.*

### ⑤ Le profil de déploiement ne routait AUCUNE route de ④

Ni celles de G1, ni celles de G2. Elles tombaient dans `location /`, dont la
dernière ligne est `try_files $uri $uri/ /index.html` : **derrière le profil
livré, elles rendaient LA PAGE, en 200, au lieu du JSON.** C'est pire qu'un
404 — un `fetch` du hub y lit du HTML et échoue à l'analyse, **sur un service
parfaitement sain**.

---

## §4 — Les défauts du PLAN, et ce qu'ils ont coûté

### D'abord une contradiction interne, dans son propre texte

Le plan écrit `enum Phase { Transfert, Execution, Reconciliation }` — **que des
mots simples** — puis affirme quatre lignes plus bas que « `Phase` et `Issue` ont
chacune **au moins une variante de deux mots** », et sa tâche 5 prescrit la
rouge du `rename_all` **sur `Phase`**, en citant `sans-effet`, **qui est une
variante d'`Issue`**.

🔴 **La rouge prescrite ne pouvait donc pas rougir**, et c'est exactement la
lacune que G1 a MESURÉE (son leg n°9) : un `rename_all` est inobservable sur un
enum dont toutes les variantes tiennent en un mot.

**Corrigé** : la rouge est jouée sur `Issue`, qui porte `SansEffet` et
`IssueInconnue`, et elle rougit. **On n'a PAS inventé une quatrième phase** pour
rendre une mutation observable — ce serait ajouter au produit un état qu'il n'a
pas —, et l'en-tête de `Phase` dit que son `rename_all` est inobservable plutôt
que de laisser croire l'inverse.

### Une divergence du plan elle-même inexacte

Sa divergence **E6** écrit que « la spec de ④ **et `CLAUDE.md`** » citent
`fraicheur.ts:37`. **Mesuré : `CLAUDE.md` n'en porte AUCUNE occurrence, la spec
en porte DEUX.** C'est le naufrage du « 487 » commis **dans la ligne qui le
dénonce**.

### Trois budgets périmés, parce que G2 a fusionné entre-temps

| Fichier | Le plan (20 août) | Réel (21 août) |
| --- | --- | --- |
| `proto/src/plateforme.rs` | 433, marge 67 | **468, marge 32** |
| `proto/ts/plateforme.ts` | 429, marge 71 | **475, marge 25** |
| `agent/src/plateforme.rs` | 453, marge 47 | **490, marge 10** |

Le plan ne budgétait **aucune** extraction pour les deux premiers. **G3 en a
joué cinq de plus** — toutes AVANT leur addition, jamais une compression.

### Une rouge prescrite NON APPLICABLE

La tâche 23 prescrit `ADD COLUMN … UNIQUE` — « refusé par SQLite, accepté par
Postgres ». **La migration `0006` ne contient AUCUN `ALTER TABLE`** : elle crée
deux tables neuves, où un `UNIQUE` en ligne est accepté des deux côtés. **La
jouer aurait été une mise en scène** ; ce qui la remplace est la même contrainte
SQLite sous son autre face — les clés étrangères nées avec leurs tables —, et la
rouge B l'éprouve.

### Une décision dont la prémisse avait changé

**D11** argumentait contre « un tuple élargi », `Canal::ordres()` rendant alors
un `(String, String)`. **G2 l'a remplacé par un enum entre-temps.** La
conclusion tient — deux files —, mais **pour une autre raison** : ce n'est pas
la forme du message qui décide, c'est que le consommateur n'est pas le même.

---

## §5 — Les chiffres, et leur nombre d'exécutions

⚠️ **AUCUN TAUX N'EST REVENDIQUÉ NULLE PART.** Les contrôles de ce sous-bloc
sont **déterministes** : une exécution y établit un fait, pas une fréquence.

| Suite | Avant G3 | Après |
| --- | --- | --- |
| `cargo test -p agent` | **808** | **861** |
| `cargo test -p proto` | **99** | **106** |
| `cd proto && npm test` | **179** (6 fichiers) | **279** (9) |
| `cd client && npx vitest run` | **378** (36) | **400** (37) |
| `plateforme`, **les deux moteurs** | **474** (51) | **564** (58) |

⚠️ **Ces comptes sont attribuables à leur commit et à lui seul** : quatre
chantiers ont écrit dans cet arbre pendant le sous-bloc, et deux comptes ont
déjà bougé sous nous.

---

## §5bis — Les sept routes, et les deux lignes qui les font vivre

`POST /televersement` · `PUT /televersement/:id/tranche/:n` ·
`GET /televersement/:id` · `POST /televersement/:id/sceller` ·
`POST /installation` · `GET /installation/:id` ·
`GET /televersement/:id/contenu` (**l'agent, et lui seul**).

🔴 **LE CHAÎNAGE EST LA MOITIÉ DU TRAVAIL.** Sans ses **deux lignes** dans
`serveur.ts::servirTout`, les sept routes tombent dans le 404 générique — la
panne la plus discrète qui soit, puisque le service répond, écoute, et sert
correctement les six autres routeurs. **Deux rouges le prouvent, et elles
discriminent** : retirer UNE ligne fait tomber **son** test et lui seul,
`1 failed | 10 passed` dans les deux sens, restauration vérifiée identique.

`entetes-routeurs.test.ts` gagne donc **un `it()` par routeur**, comme son
en-tête l'exige nommément. Les nôtres sont le **septième** et le **huitième** ;
ce fichier n'existe que parce que G1 avait ajouté le cinquième « sans que
personne ne s'en aperçoive côté P5 », et G2 le sixième.

### Une collision de clé, attrapée par `tsc` **et par chance**

Les deux routeurs avaient nommé leur dépendance `magasin` — nom **déjà pris**
dans `serveur.ts` par le magasin d'**icônes** de G2. `tsc` l'a vue parce que les
deux types diffèrent. ⚠️ **Le jour où deux magasins auront la même forme, le
service servirait des icônes à la place des tranches sans qu'aucun contrôle ne
bronche.** Renommée `tranches`, aux deux bouts.

### 🔴 Le `PUT` que le navigateur exige et qu'aucun test serveur ne voit

`Access-Control-Allow-Methods` annonçait `GET, POST, OPTIONS`.
`PUT /televersement/:id/tranche/:n` est la **première route `PUT` de tout le
service**, et son appelant **est le navigateur**. Portant `Authorization`, elle
est non simple : le navigateur envoie une préalable portant
`Access-Control-Request-Method: PUT` et **abandonne sans jamais envoyer la vraie
requête**. Sans effet en origine unique (profil `deploiement`) ; **mordant en
développement**, où `vite` sert sur 5173 et le service sur 8080 — donc **là où on
le met au point, et nulle part ailleurs**.

⚠️ **TROISIÈME FOIS QUE CETTE CLASSE MORD, ET P4 L'AVAIT NOMMÉE EN LA DÉCLARANT
SANS GARDE AUTOMATIQUE.** Elle a mordu deux fois chez P4 (`Authorization` non
permis, préalable non traitée) et une troisième ici, **sur la méthode**. Aucun
`fetch` de Node n'applique la politique d'origine : **aucun test de bout en bout
ne peut rendre ce défaut rouge**, pas même un qui enverrait un vrai `PUT`,
puisqu'il aboutirait. Le seul garde est une assertion sur la **valeur**. Rouge
vue : `2 failed | 6 passed`.

### 🔴 Une fuite de disque, mesurée puis supprimée à sa source

`magasin-tranches.ts` affirmait que « **le fichier partiel est supprimé, quelle
que soit la cause** ». C'était faux, et c'est ce qui faisait tomber un test **par
intermittence sous la charge de la suite complète, jamais isolé**.

**Course** : `createWriteStream(chemin)` ouvre le fichier de façon
**asynchrone** ; sur un dépassement notre générateur lève **avant** que
l'`open(2)` n'aboutisse, le `rmSync` du `catch` courait donc sur un fichier
inexistant — `{ force: true }` avalant le `ENOENT` **en silence** — et
l'ouverture le créait juste après.

✅ **Le remède n'est pas un réessai mais la suppression de la course** : le
descripteur est ouvert par `openSync` **avant** le `pipeline`, si bien que
l'inode existe déjà quand le `catch` supprime. Un réessai temporisé aurait
réduit la fenêtre sans la fermer.

| Sonde directe sur `ecrire`, hors HTTP | Répertoires non vides |
| --- | --- |
| première mesure (agent qui a trouvé le défaut) | **42 / 400** |
| seconde mesure, autre charge | **100 / 400** |
| **après le correctif** | **0 / 400** |

⚠️ **AUCUN TAUX N'EST REVENDIQUÉ** : les deux premiers chiffres diffèrent d'un
facteur deux et demi selon la charge, ce qui est le propre d'une course. Ce qui
est établi est **l'existence du défaut puis sa disparition**, jamais une
fréquence. ⚠️ **Ce n'était PAS un trou de protocole** — `lister` ignore les noms
non numériques, donc aucune fausse tranche n'a jamais été comptée. C'était une
**fuite de disque**, sur un service qui accepte 4 Gio.

### Quatre extractions, aucune compression

Le plafond de 500 a été **franchi** (`routes-televersement.test.ts`, **514**) et
**frôlé** (sa production, **497**, marge 3) — **relevé par la commande, et le
plan ne budgétait ces extractions nulle part**.

| Fichier | Après | |
| --- | --- | --- |
| `televersement-regles.ts` | **66** | règles pures ; production 497 → **443** |
| `routes-televersement-tranches.test.ts` | **109** | la famille « déposer », **verbatim** |
| `routes-televersement-harnais.ts` | **117** | fixtures — **pas** un `.test.ts`, sinon l'importer rejouerait ses `it()` |
| `routes-televersement.test.ts` | **362** | (**514** avant) |

**20 tests avant, 20 après**, aucune réfutation raccourcie.

---

## §5ter — 🔴 La dette `proto/` : le brief la disait close, la mesure dit l'inverse

Le brief annonçait « la dette proto **déjà extraite par G2** → constate ».
**Mesuré au parent du premier commit de G3** (`70eb794~1`), plutôt que pris :

| Fichier | Avant G3 | Aujourd'hui |
| --- | --- | --- |
| `proto/src/plateforme/tests.rs` | **561** | **340** |
| `proto/ts/plateforme.test.ts` | **512** | **462** |

Ce sont **exactement** les deux nombres que la table de dette de `CLAUDE.md`
porte, inscrits par le sous-bloc P1 du presse-papier. **G2 n'y avait pas
touché** : c'est G3 qui les purge, par ses propres extractions. Le sous-bloc ne
*constate* donc pas une dette close — **il la solde**, et la table de dette
revient à ses **deux** lignes gelées.

---

### La divergence `403` / `404` : G1 est désormais seul

⚠️ **Relevé, non tranché — la décision appartient au propriétaire du dépôt.**
G3 n'émet que des refus **indistinguables** (`vm-inconnue`,
`televersement-inconnu`, `installation-inconnue`), et l'autorisation passe
**avant** le `409 non-scelle`, sans quoi le refus d'état deviendrait l'oracle
que le refus d'accès ferme. `grep` sur `vm-etrangere` : **deux occurrences, dans
un seul fichier — celui de G1**. La divergence que P4 a signalée oppose donc
maintenant **G1 seul à P4, G2 et G3**.

---

## §6 — Ce que G3 n'établit PAS

- 🔴 **La porte UAC n'est pas jouée** (§1), et **rien de ce sous-bloc n'a tourné
  sur la VM Windows**.
- 🔴 **Le critère ⑦ reste indécidable tant que la sonde du job n'a pas tourné** :
  le garde est en place et journalise, mais personne n'a lu sa ligne.
- 🔴 **Le câblage n'est couvert par aucun test** — c'est reconnu plutôt que
  masqué. Le seul contrôle disponible tout de suite est que `git diff` ne montre
  aucune ligne de `main.rs`, et il a été joué.
- **`execution.rs` et `peripherique_audio.rs` n'ont sur l'hôte aucune épreuve**
  hors `cargo check --target x86_64-pc-windows-gnu`, qui vérifie types, emprunts
  et durées de vie — **jamais le comportement**.
- **Le profil de déploiement n'a été validé par AUCUN nginx** — `which nginx`
  rend « not found », mesuré. ⚠️ Et un `nginx -t` vert n'aurait rien prouvé du
  ROUTAGE : ce fichier raconte lui-même une configuration déclarée « ok » qui
  rendait la page à un client WebSocket.
- **Rien de la charge** : ni le nombre de téléversements simultanés qu'une
  plateforme soutient, ni le débit du transfert vers la VM.
- **Rien d'un antivirus.** ⚠️ **L'état de celui de la VM n'a PAS été relevé** —
  ni par la spec, ni par G1, ni ici. Une mise en quarantaine ferait échouer une
  installation avec un code de sortie trompeur : ni mesuré, ni prédit.
- **Rien d'un installeur qui s'élève LUI-MÊME en cours de route** : le refus
  typé ne l'attrape pas, et seule l'expiration le borne.
- **Rien de TLS** : l'agent ne parle ni `wss` ni `https`, et G3 ne l'y amène pas.
- **Rien du `Transfer-Encoding: chunked`** : il est **refusé**, pas géré, et
  personne n'a mesuré si un proxy en produirait.
- **Aucune constante calibrée** : `TAILLE_TRANCHE`, `TELEVERSEMENT_MAX_OCTETS`,
  `TELEVERSEMENTS_EN_COURS_MAX`, `EXPIRATION_TELEVERSEMENT`,
  `EXPIRATION_INSTALLEUR`, `EXPIRATION_EXECUTION`, `PERIODE_PROGRESSION`,
  `JOURNAL_MAX_OCTETS`, `RETABLISSEMENTS_MAX`, `ENTETE_MAX_OCTETS`. Elles
  rejoignent la liste que ce dépôt tient depuis `BPP_MIN`.
- **Aucun retour arrière** — voir le §7.
- **Le leg n°1 de G1 n'est pas refermé** : G3 s'en protège par un refus, il ne le
  corrige pas.

---

## §7 — 🔴 Ce que G3 rend visible, et qui n'est pas de lui

**G3 est le premier sous-bloc dont les conséquences rendent visible qu'IL N'Y A
AUCUN RETOUR ARRIÈRE.** Le backend d'orchestration v1 refuse explicitement
`instantane` (`orchestration/inventaire-statique.ts`, exposé en `501` et
journalisé) : **il n'existe, dans tout le produit, aucun moyen de revenir à
l'état d'avant une installation.**

Ce n'est ni un oubli de G3 ni quelque chose qu'il puisse réparer — c'est une
propriété du sous-projet ⑤. Mais c'est G3 qui exécute des binaires arbitraires,
et donc G3 qui rend la phrase exécutable.

**Ce qu'un installeur peut faire, et que rien n'empêche** : installer un pilote,
corrompre le registre, remplir le disque, désactiver le réseau, **désinstaller
ou remplacer l'agent lui-même** — la plateforme le verrait par `vu_a` et
annoncerait la VM `injoignable`, et **rien ne le réparerait**.

**Et changer le périphérique audio par défaut, ce qui S'EST PRODUIT le 19 août
2026** : l'installation de VB-Cable a fait basculer le rendu par défaut sur un
câble virtuel que rien n'alimente, et le loopback du chantier A s'est mis à
capter du silence **sans qu'aucune ligne de journal ne dise pourquoi**. C'est le
cas **nominal**, pas un cas limite. G3 journalise le périphérique **avant et
après** chaque installation — la prochaine occurrence devient attribuable par un
`grep`, au lieu d'une campagne — et **n'ajoute aucun garde-fou de restauration** :
remettre d'autorité le périphérique d'avant serait décider à la place de
l'utilisateur qui vient d'installer une carte son exprès.
