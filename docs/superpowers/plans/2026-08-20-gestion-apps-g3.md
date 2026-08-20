# Sous-bloc G3 — le téléversement, l'exécution, et son issue : plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal :** qu'un utilisateur dépose un installeur de plusieurs centaines de
mégaoctets, que ces octets arrivent **intacts** dans sa VM, qu'ils s'y exécutent
**sans que rien ne les tue**, et que le produit dise **ce qui s'est réellement
passé** — jamais ce qu'un code de retour prétend.

**Architecture :** quatre étages, et la coupure pur / portable / `#[cfg(windows)]`
est décidée **ici**, pas à l'implémentation.

1. **Le navigateur découpe, empreint et dépose.** Le découpage en tranches et
   l'empreinte incrémentale sont **purs, sans DOM**, et vivent dans `proto/ts/`
   parce qu'ils sont le **contrat partagé** avec la plateforme (D6) : deux
   arithmétiques de tranches qui divergeraient produiraient un scellement qui
   échoue sans que personne ne sache lequel des deux a tort.
2. **La plateforme range les tranches sur disque, et ne les assemble jamais**
   (D7). Le scellement est une **passe de flux** qui recalcule le SHA-256 ; la
   reprise est un **listage de répertoire**, jamais une comptabilité (spec D7).
3. **L'agent tire les octets par HTTP**, les recalcule après écriture, et
   exécute **hors du job object** (spec D8), en dehors du fil de réconciliation.
   Le téléchargement est **portable** — `tokio::net::TcpStream`, aucun `cfg` —
   donc il se teste sur l'hôte contre un vrai serveur (E4). Seule l'exécution
   est Windows.
4. **Le verdict se lit dans la RÉCONCILIATION** (spec D9), sur une **fenêtre**
   qui s'ouvre au lancement et se ferme après la sortie du processus — pas sur
   la seule réconciliation suivante, qui manquerait ce qu'une réconciliation
   intercalaire a déjà vu (D9).

**Tech Stack :** inchangée. 🔴 **G3 N'AJOUTE AUCUNE DÉPENDANCE de production**,
ni en TypeScript ni en Rust — et c'est ce qui décide de trois choses : le
SHA-256 incrémental de l'agent **étend** `agent/src/apps/sha256.rs` (déjà écrit
par G1 pour cette raison exacte), le SHA-256 du navigateur est écrit en JS
(M1 : l'API Web Crypto n'a **aucune** forme incrémentale), et le client HTTP de
l'agent est écrit à la main (M3 : il n'en existe aucun, et aucun crate HTTP
n'est dans l'arbre). Le témoin de l'invariant côté plateforme est
`plateforme/src/base/pilote.test.ts` (`expect(deps).toEqual(['pg','ws'])`,
cité par le plan de G2) : il doit rester **vert**.

**Spec :** `docs/superpowers/specs/2026-08-19-gestion-apps-design.md`
(commit `dded5b5`), §4 (D7, D8, D9, D10), §5 « G3 », §6, §7, §8, §9, §10, §11.
**Amont qui fait autorité sur l'état du code :**
`docs/superpowers/plans/2026-08-19-gestion-apps-g1.md` et son document de
résultats, la section G1 de `CLAUDE.md`, le plan de G2
(`docs/superpowers/plans/2026-08-20-gestion-apps-g2.md`, commit `e4eb5c2`,
**écrit et NON implémenté**), et **le code lui-même** — l'arbre a bougé depuis
la clôture de G1.

**Ce plan ne couvre QUE G3.** Rien des icônes ni de leur preuve par la ressource
(G2) : aucune extraction d'icône, aucun `IconesManquantes`, aucun magasin
d'images. Rien de `ReadDirectoryChangesW` ni de l'anti-rebond (G4) — et G3 est
écrit pour **ne pas en dépendre** (D9). Rien du manifeste PWA par application,
des `file_handlers` ni des types installeur du hub (G5). **Aucune page de hub** :
comme G1 (sa décision D12) et comme G2, la recette exerce les routes par `curl`
et par un pilote Node qui **appelle le code du produit**, ce qui les éprouve
davantage qu'une page (D14).

---

## Contraintes globales

### Ce qui a été relevé PAR LA COMMANDE avant d'écrire une ligne

Toutes les valeurs ci-dessous ont été obtenues le **20 août 2026**, sur cette
machine, en lançant réellement la commande. **Aucune n'est recopiée d'un
document.** Elles **DÉRIVERONT** : la seule source de vérité est la commande.

```
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>380'
```

| Fichier | Lignes | Ce que G3 en fait |
| --- | --- | --- |
| `agent/src/encode.rs` | **1536** | dette gelée, non touchée |
| `agent/src/windows_source.rs` | **630** | dette gelée, non touchée |
| 🔴 `proto/src/plateforme/tests.rs` | **561** | **dette inscrite** — G3 y ajoute des cas : extraction AVANT addition (D3) |
| 🔴 `proto/ts/plateforme.test.ts` | **512** | **dette inscrite** — idem |
| `agent/src/superviseur/lanceur.rs` | **488** | **LU, non modifié** — c'est lui qui pose le job object (D10) |
| `plateforme/src/http/routes-applications.test.ts` | **480** | **non touché par G3** — G3 crée ses propres fichiers de routes |
| `agent/src/plateforme.rs` | **453** (marge 47) | 🔴 **extraction AVANT addition** (D11) : G3 y ajoute une file, une branche descendante et deux montantes |
| `proto/src/plateforme.rs` | **433** (marge 67) | +5 variantes, +2 enums, le bump de version |
| `proto/ts/plateforme.ts` | **429** (marge 71) | le miroir |
| `plateforme/src/agents/canal.ts` | **423** (marge 77) | 🔴 **extraction AVANT addition** (D11) |
| `plateforme/src/http/serveur.ts` | **371** | +2 routeurs chaînés |
| `plateforme/src/http/routes-applications.ts` | **315** | **non touché** |
| `agent/src/apps/lecture.rs` | **281** | non touché |
| `agent/src/apps/boucle.rs` | **213** | la fenêtre de comptage, et l'aiguillage |
| `plateforme/src/depot/application.ts` | **186** | non touché |
| `plateforme/src/config.ts` | **183** | `PLATEFORME_TELEVERSEMENTS` |
| `agent/src/apps/sha256.rs` | **168** | **étendu** : l'API incrémentale |
| `agent/src/apps.rs` | **156** | `brancher` branche aussi l'installation |
| `scripts/run-agent.sh` | **145** | 🔴 **tâche dédiée** (D16) |
| `plateforme/src/apps/catalogue.ts` | **127** | non touché |

Autres relevés par la commande, le même jour :

- `proto/src/plateforme.rs:97` — `pub const PLATEFORME_VERSION: u8 = 2;` ;
  `proto/ts/plateforme.ts:16` — `export const PLATEFORME_VERSION = 2;` ;
  `proto/plateforme-vectors.json:3` — `"version": 2`, et **chaque** chaîne
  `json` du fichier porte `"v":2` en dur (189 lignes).
- **Comptes de tests de départ, à opposer aux comptes d'arrivée** :
  `cargo test -p proto` rend **83 passed** (plus une ligne de doc-tests à
  **0**) ; `cd proto && npm test` rend **5 fichiers, 142 tests**.
  ⚠️ **Dire lequel on annonce** — le piège des « dix » et « dix-sept » de P3.
- `plateforme/src/base/migrations/` porte **quatre** migrations
  (`0001-socle`, `0002-identite`, `0003-agents`, `0004-applications`), et
  `plateforme/src/base/pilotes.test.ts:33` fige
  `expect(suivi.map((l) => Number(l.version))).toEqual([1, 2, 3, 4]);`.
- `plateforme/src/base/migrations.ts` trie les migrations **numériquement**
  (`.sort((a, b) => a.version - b.version)`) et **n'exige aucune contiguïté** :
  un trou de numérotation ne casse rien (D2).
- `plateforme/src/base/pilote.ts:31-39` — `rendreMarqueurs` **REFUSE** tout SQL
  portant une apostrophe ou un guillemet : **aucun `DEFAULT` littéral n'est
  possible dans une migration**.
- `plateforme/src/base/pilote-sqlite.ts:54` — `base.exec('PRAGMA foreign_keys = ON')` :
  **les clés étrangères sont APPLIQUÉES des deux côtés**.
- `plateforme/src/base/migrations/0003-agents.sql` — `application` porte
  `vm_id TEXT NOT NULL REFERENCES vm(id)`, **sans `ON DELETE`** : une
  application orpheline **n'est insérable nulle part**. Les deux tables neuves
  de G3 naissent avec leurs clés étrangères, et pour la même raison (leg n°2 de
  P1, rappelé par `0004-applications.sql`).
- `plateforme/src/http/serveur.ts:221-231` — cinq routeurs chaînés, `/sante` en
  dernier ; `plateforme/src/http/entetes-routeurs.test.ts` porte **un `it()`
  par routeur** (l. 67, 77, 85, 92, 103) et son en-tête (l. 3) l'exige en
  toutes lettres.
- `plateforme/src/http/routes-auth.ts:72` — `const CORPS_MAX_OCTETS = 4 * 1024;`,
  et `lireCorps` (l. 103-119) **accumule dans une chaîne UTF-8** : il est
  inutilisable pour un corps binaire, et **il ne doit pas être relevé** (D8).
- `plateforme/src/http/porteur.ts` — `lirePorteur` refuse un jeton d'agent par
  `403 jeton-agent` (l. 80-86). **Il ne peut donc pas servir la route que
  l'agent appelle** (D12).
- `plateforme/src/depot/agent.ts:77` — `lireParPrefixe` existe déjà : c'est par
  elle qu'un jeton d'agent se résout en VM (D12).
- `plateforme/src/agents/fraicheur.ts:47` — `export const SEUIL_INJOIGNABLE_MS = 90_000;`
  ⚠️ **La spec de ④ et `CLAUDE.md` citent tous deux `fraicheur.ts:37`** : le
  numéro a dérivé, la valeur non (E9).
- `scripts/verify-all.sh` appelle `etape` **dix** fois (l. 67, 70, 82, 85, 95,
  98, 101, 104, 107, 110). ⚠️ **Une exécution complète en affiche DIX-SEPT** —
  les sept de plus viennent de l'intérieur de `client : npm run design:verifier`.
- `agent/src/main.rs:275` — `let _apps = apps::brancher(_canal_plateforme.as_mut());`,
  **avant** l'aiguillage `PONT` (l. 299) : le pont exécute donc lui aussi la
  boucle de découverte (leg n°8 de G1), et c'est ce qui rend D10 nécessaire.
- `agent/src/plateforme.rs:225` — `pub fn ordres(&mut self) -> Option<mpsc::UnboundedReceiver<(String, String)>>` :
  **un tuple, un seul consommateur**. G3 ne le change pas, il ajoute une
  **seconde** file (D11).
- `agent/src/apps.rs:34` — `PERIODE_RECONCILIATION: Duration = Duration::from_secs(30)`.
- `agent/Cargo.toml` — aucune dépendance HTTP, aucun `sha2`. `windows` est
  verrouillé à **0.62.2** dans `Cargo.lock` (l. 1325-1334) et
  `tokio-tungstenite` **0.24.0 n'y porte NI `rustls` NI `native-tls`**
  (ses quatre dépendances relevées : `futures-util`, `log`, `tokio`,
  `tungstenite`). ⚠️ **L'agent ne sait donc parler ni `wss` ni `https`
  aujourd'hui** — voir M3 et D13.

### 🔴 Les mesures prises POUR ce plan, transcrites VERBATIM

⚠️ **AUCUNE N'A ÉTÉ PRISE SUR LA VM WINDOWS** : elle redémarrait pendant la
rédaction et un chantier concurrent y conduit la recette du microphone. Les
quatre mesures ci-dessous sont prises **sur l'hôte**, et les deux relevés qui
n'ont pu être que des **lectures de code** sont nommés comme tels — pas comme
des mesures.

#### M1 — 🔴 L'API Web Crypto n'a AUCUNE forme incrémentale, et c'est ce qui décide de D5

```
$ node -e "const p = Object.getPrototypeOf(globalThis.crypto.subtle);
  console.log(Object.getOwnPropertyNames(p).sort().join(' ')); console.log(process.version)"
constructor decapsulateBits decapsulateKey decrypt deriveBits deriveKey digest
encapsulateBits encapsulateKey encrypt exportKey generateKey getPublicKey
importKey sign unwrapKey verify wrapKey
v24.9.0
```

**`digest` et rien d'autre** : pas de `createHash`, pas d'`update`, pas de
`Digest` à état. Empreindre un fichier de plusieurs centaines de mégaoctets par
`crypto.subtle.digest` exige donc de le tenir **entier en mémoire**, dans un
onglet. ⚠️ **PORTÉE EXACTE** : c'est l'implémentation de **Node 24.9.0** qui est
relevée ici, pas celle d'un navigateur. La spécification Web Crypto n'expose que
`digest`, et aucun navigateur ne s'en écarte à ma connaissance — mais **ce n'est
pas mesuré**, et un implémenteur qui voudrait s'en assurer le fera sur le
navigateur de la recette, pas sur Node.

#### M2 — Le prix d'un SHA-256 écrit en JS, mesuré contre le natif

Programme jetable, écrit dans le bloc-notes de session, jamais versé (il
n'appartient pas au dépôt) : une compression SHA-256 en JS pur appliquée à
200 Mo, puis `node:crypto` sur les mêmes octets.

```
JS pur   : 200 Mo en 2.68 s -> 74.7 Mo/s
node natif: 200 Mo en 0.17 s -> 1160.6 Mo/s
```

**Un facteur 15,5**, et l'ordre de grandeur qui compte : **un installeur de
800 Mo coûte ≈ 11 s d'empreinte** côté navigateur. ⚠️ **UNE exécution, sur
CETTE machine, sous Node 24.9.0** : un navigateur n'a **pas** été mesuré, et le
chiffre n'a pas à être transporté ailleurs. Ce qu'il établit est suffisant pour
décider : **11 s est payable, 800 Mo en mémoire ne l'est pas** (D5).

#### M3 — LECTURE, PAS MESURE : l'agent n'a aucun client HTTP, et aucun TLS

```
$ grep -rln 'TcpStream|HTTP/1.1' agent/src/     ->  agent/src/plateforme.rs (seul)
$ grep -rn  'HTTP/1.1' agent/src/               ->  (aucune occurrence)
```

`agent/src/plateforme.rs` n'ouvre de `TcpStream` que **par**
`tokio_tungstenite::connect_async`. Aucun crate HTTP n'est déclaré dans
`agent/Cargo.toml`, et `tokio-tungstenite` est verrouillé sans aucune
fonctionnalité TLS (relevé ci-dessus). **C'est une lecture de l'arbre, pas une
mesure** : je n'ai lancé aucune connexion.

#### M4 — Le profil de déploiement ne route AUCUNE des routes de ④, et n'a aucun plafond de corps

```
$ grep -n 'location' deploiement/nginx.conf
167:        location / {
263:        location @relais {
275:        location = /agent {
293:        location /auth/  { proxy_pass http://plateforme; }
294:        location = /session { proxy_pass http://plateforme; }
295:        location = /vm      { proxy_pass http://plateforme; }
298:        location = /sante   { proxy_pass http://plateforme; }
$ grep -n 'client_max_body_size|proxy_request_buffering|client_body' deploiement/nginx.conf
(aucune occurrence)
```

Deux conséquences, et la seconde n'est écrite nulle part dans ce dépôt :

1. **`GET /applications` et `POST /application/:id/lancer` — les deux routes de
   G1 — ne sont routées par aucun `location`.** Elles tombent donc dans
   `location /`, dont la dernière ligne est
   `try_files $uri $uri/ /index.html;` (`deploiement/nginx.conf:257`) : derrière
   le profil livré, **elles rendent la PAGE, en 200, au lieu du JSON**. C'est
   pire qu'un 404 — un `fetch` du hub y lirait du HTML et échouerait à
   l'analyse. **Legs de G1 que personne n'a inscrit** (E1).
2. **Aucun `client_max_body_size` n'est posé.** La valeur par défaut de nginx
   est **1 m** d'après sa documentation — ⚠️ **je ne l'ai PAS mesurée**, et
   c'est précisément pourquoi la tâche 32 la pose **explicitement** plutôt que
   de compter sur un défaut : une tranche de 8 Mio serait refusée par le proxy
   **avant** d'atteindre le service, et le service n'en saurait rien.

#### M5 — LECTURE, PAS MESURE : qui est dans le job object, et qui ne l'est pas

`agent/src/superviseur/lanceur.rs` crée le job (l. 118-119) avec
`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` (l. 122) et n'y assigne **que ses enfants**
(`AssignProcessToJobObject` aux l. 277 et 412 — le capteur et les enfants de
fenêtre). **Le superviseur lui-même ne s'y assigne jamais.**

🔴 **Ce que cela implique pour le critère ⑦ de la spec, et qui n'y est pas
écrit** : si le processus qui lance l'installeur n'est **dans aucun job**, alors
« tuer l'agent ne tue pas l'installeur » est vrai **par construction**, et le
critère **ne peut pas échouer** — c'est un vert qui ne prouve rien. Et
l'inverse est vrai aussi : `agent/src/main.rs:275` branche la découverte
**avant** l'aiguillage `PONT` (l. 299), or **le pont, lui, EST dans le job**.
Sous le leg n°1 de G1 — le pont s'enrôle sous le même `vm_id` que son père et
les deux sockets s'évincent à ~1,5 Hz —, **l'ordre `Installer` peut atterrir
dans le pont**, qui lancerait l'installeur **depuis un processus assigné au
job**, donc tué à la mort du superviseur. **C'est exactement ce que la spec D8
interdit**, et cela ne se voit qu'en croisant trois fichiers.

**D'où la sonde U2** (tâche 2) et **D10**.

### La règle des 500 lignes : QUATRE extractions, toutes AVANT leur addition

**Aucune compression, sans exception** — la doctrine est écrite en tête de
`CLAUDE.md` et a été repayée en D9 (`sommeil.rs` ramené à 499 par compression,
puis extrait sur exigence de revue) puis en D10 (trois franchissements, trois
extractions).

| # | Fichier | Lignes | Pourquoi G3 doit l'extraire |
| --- | --- | --- | --- |
| 1 | `proto/src/plateforme/tests.rs` | **561** | 🔴 **DETTE INSCRITE** ; G3 y ajoute les cas des cinq variantes neuves |
| 2 | `proto/ts/plateforme.test.ts` | **512** | 🔴 idem, côté miroir |
| 3 | `agent/src/plateforme.rs` | **453**, marge **47** | G3 y ajoute une file, une branche descendante et deux montantes : il franchira |
| 4 | `plateforme/src/agents/canal.ts` | **423**, marge **77** | G3 y ajoute trois branches et la réémission à l'enrôlement |

🔴 **LES DEUX PREMIÈRES SONT LA DETTE QUE `CLAUDE.md` INSCRIT SANS POINT DE
CHUTE, ET LE PLAN DE G2 LES REVENDIQUE DÉJÀ** (sa décision D2, qui nomme
`tests_apps.rs` et `plateforme-apps.test.ts`). **G3 ne les revendique pas une
seconde fois** : voir D3, qui écrit la règle de coordination — *celui qui
arrive le premier extrait, celui qui arrive le second constate*. Ce qui est
interdit, c'est que les deux plans découpent la même dette **différemment**.

⚠️ **Les quatre extractions ne portent AUCUNE ligne de comportement neuve**, et
chacune se vérifie de la même façon : le compte de tests **avant** et **après**
est **identique**, annoncé avant d'être mesuré. *(D10 : un implémenteur a écrasé
un fichier de tests et supprimé un test antérieur — il l'a vu parce que le
compte est sorti à 452 au lieu des 453 annoncés d'avance.)*

### Les règles de méthode, héritées et non négociables

1. **Aucune pièce fabriquée.** Toute sortie de commande citée dans un rapport
   est **relancée**, jamais reconstituée de mémoire. D10 a trouvé deux pièces
   fabriquées dont les **faits étaient vrais** : c'est le mode de défaillance à
   surveiller, parce qu'il produit une conclusion juste **sans preuve**, donc
   invérifiable par le suivant.
2. **Un contrôle qu'on n'a jamais vu ROUGE n'est pas un contrôle.** Pour chaque
   contrôle prescrit ci-dessous, la ligne « ce qui le rend ROUGE » nomme
   l'état, **et cet état est atteignable** — quand il ne l'est pas, c'est dit
   (le critère ⑦ est le cas d'école, voir M5 et la tâche 2).
3. 🔴 **Un verdict NÉGATIF exige que la chose mesurée soit ABSENTE, pas
   seulement nulle.** Trois zéros sur une machine saine ne sont pas un verdict.
   Toute sonde de ce plan doit d'abord établir qu'elle **sait observer** ce
   qu'elle cherche — et la porte U1 est écrite tout entière autour de cette
   exigence.
4. **`fichier:ligne` relu après avoir été écrit, et l'ENTRÉE nommée**, pas
   seulement la ligne. ⚠️ **Vérifier la VERSION de ce qu'on lit** : un agent a
   déclaré fausses trois citations exactes parce qu'il lisait deux modules
   homonymes d'une autre version. Le crate `windows` de cet arbre est
   **0.62.2**.
5. **Deux exécutions par critère, jamais une.** Règle du sous-projet ⑤ et du
   chantier D. **Aucun taux ne sera revendiqué.**
6. 🔴 **Toute variable neuve de l'agent passe par `scripts/run-agent.sh`, dans
   une TÂCHE DÉDIÉE.** Piège payé **cinq fois** — `SUPERVISEUR` (D1),
   `MULTIFENETRE_REPRISE` (D2), `AUDIO` (D7) —, évité par une tâche dédiée en
   D6 (`BUDGET_BPS`), en G1 (`APPS`) et en E2 (les trois du micro). **Symptôme
   quand on l'oublie : un agent qui démarre sans la variable ET SANS RIEN
   SIGNALER.**
7. 🔴 **Un préfixe de route trop large ne se voit PAS dans un statut.** G1 a
   mesuré qu'un `startsWith('/application')` **laissait DIX-SEPT tests verts** :
   la route mangeait toute la famille et rendait **son propre 404 typé**,
   indiscernable du 404 générique tant qu'on ne lisait que le code. **Tout
   contrôle de chemin de ce plan compare le CORPS**, sur au moins quatre
   chemins déclinés.
8. **Nommer les fichiers dans `git add`, jamais `git add -A`** : l'arbre est
   partagé avec quatre chantiers actifs.
9. 🔴 **`git checkout` NE RESTAURE PAS un fichier non suivi et EFFACE un fichier
   suivi non commité.** Un agent y a perdu une implémentation entière. Toute
   mutation jouée pour voir une rouge se défait par une **copie de sauvegarde
   nommée**, jamais par `git checkout`.
10. **`git commit` valide TOUT L'INDEX, `-F message` compris.** Pathspec
    explicite obligatoire, et `git show --stat` après coup. **Jamais
    `--amend`.**
11. ⚠️ **`scripts/verify-all.sh` n'est pas hermétique** (leg n°5 de G1) : avec
    `TURN_URL`/`TURN_SECRET` dans l'environnement — c'est-à-dire après le
    `set -a && source .env` que tout travail sur la VM exige — **six** tests de
    `src/signaling/server.test.ts` échouent pour une raison étrangère. **Le
    lancer depuis un shell propre, ou `env -u TURN_URL -u TURN_SECRET`.**
12. ⚠️ **`docker compose config` imprime les secrets en clair**, et `${VAR:?}`
    engage **toutes** les sous-commandes. Aucune tâche de ce plan n'en a
    besoin ; c'est écrit pour qu'aucune ne l'invente.
13. ⚠️ **Un `cd` dans une commande de relevé fait injecter un `ls` par le hook
    `chpwd` du shell hôte** — rencontré pendant la rédaction de ce plan, et
    documenté par G1. `unset -f chpwd` avant tout relevé.
14. ⚠️ **Un `grep` sans `-a` rend une SORTIE VIDE, jamais un zéro**, sur un
    fichier que Windows réécrit encore (octets NUL) — P1 et D10.

### Périmètre concurrent — à lire avant de toucher `proto/`, `agent/` ou la VM

| Fichier | Qui d'autre | Ce que G3 fait |
| --- | --- | --- |
| `proto/src/plateforme.rs`, `proto/ts/plateforme.ts`, `proto/plateforme-vectors.json` | 🔴 **G2, planifié et non implémenté**, qui y monte `PLATEFORME_VERSION` à **3** | **G3 prend le SUIVANT, relevé à l'implémentation** — voir D1, qui n'écrit **aucun nombre en dur** |
| `proto/src/control.rs`, `proto/src/fichiers.rs` et leurs miroirs | presse-papier, pont fichiers | 🔵 **G3 N'Y TOUCHE PAS** |
| `agent/src/main.rs` | tous | 🔵 **G3 N'Y TOUCHE PAS** : l'installation se branche **dans** `apps::brancher` (D10), dont le site d'appel (l. 275) et la forme de liaison (`let _apps = …`) ne bougent pas |
| `agent/src/superviseur/lanceur.rs` | — | 🔵 **LU, jamais modifié.** G3 n'ajoute **aucun** drapeau au job object |
| `plateforme/src/http/serveur.ts` | P5 (en-têtes), P4 (routes VM), G2 (routeur d'icônes) | **deux lignes de chaînage**, sur le modèle de `servirApplications` (l. 223) |
| `plateforme/src/config.ts` | P5, G2 (`PLATEFORME_ICONES`) | **un champ**, `PLATEFORME_TELEVERSEMENTS` |
| `plateforme/src/base/migrations/` | G2 (`0005-icones.sql`, planifié) | **G3 prend le numéro libre suivant, relevé par `ls`** — D2 |
| `plateforme/src/http/porteur.ts` et un éventuel lecteur de jeton d'AGENT | G2 (`PUT /icone/:sha256`) | **si G2 a livré un lecteur de jeton d'agent, le RÉEMPLOYER** — une copie divergerait en silence (D12) |
| `scripts/run-agent.sh` | micro E2, presse-papier | **une ligne**, dans une tâche dédiée |
| 🔴 **la VM Windows** | **un chantier concurrent y conduit la recette du microphone** | **trois tâches l'emploient** — 2 (les deux sondes), 19 (une vérification de compilation distante) et 33 (la recette). Toutes trois vérifient `Get-Process agent` **avant et après, y compris après une tentative échouée** (piège de D8, payé trois fois sur trois) |

⚠️ **Douze fichiers `g2plan-*` inertes traînent dans `C:\dev\` de la VM** (sondes
du plan de G2, lecture seule). **Ne pas s'en étonner, ne pas les supprimer** :
ils n'appartiennent pas à G3.

---

## 🔴 LA PORTE — l'élévation UAC, et une sonde écrite pour ne pas pouvoir mentir

**C'est le risque n°1 du §11 de la spec, et il est déclaré NON MESURÉ.** Son
énoncé : *une élévation UAC ouvre une boîte de dialogue sur le **bureau
sécurisé**, que Desktop Duplication ne capture pas ; une installation qui
l'exige se présenterait à l'utilisateur comme un **écran figé**.* Si c'est
confirmé, **tout installeur exigeant une élévation est hors de portée de la
v1** — et c'est le cas de la plupart.

⚠️ **CETTE MESURE N'A PAS ÉTÉ PRISE PAR CE PLAN.** La VM était en cours de
redémarrage et un chantier concurrent y conduit une recette. La tâche 1 la
prescrit ; **le sous-bloc s'arrête sur son verdict** avant d'écrire la moitié
« exécution ».

### Pourquoi la forme naïve de cette sonde MENT, et dans quel sens

La sonde naïve est : *je déclenche une élévation, je capture, je ne vois pas la
boîte de dialogue, donc elle n'est pas capturée.* **Elle rend un verdict
ÉLIMINATOIRE à partir d'une ABSENCE**, et une absence a trois causes qu'elle ne
distingue pas :

1. la boîte de dialogue **est** sur le bureau sécurisé et n'est pas capturée —
   le fait cherché ;
2. **aucune boîte de dialogue n'a jamais été ouverte** — UAC désactivé
   (`EnableLUA = 0`), auto-élévation, ou processus déjà élevé. **Rien n'a eu
   lieu, et l'on conclut sur rien** ;
3. **l'instrument ne sait pas observer** — la capture ne tournait pas, la
   fenêtre n'était pas rattachée, le compteur lisait le mauvais objet.

**C'est le défaut que ce dépôt a payé aujourd'hui même** : une sonde a rendu un
**faux verdict éliminatoire** — trois zéros sur une machine saine, parce que
rien n'avait encore eu lieu —, et le planificateur de G2, averti, **a refusé son
propre verdict** et trouvé que la cause était l'instrument. C'est aussi F1 de
D7 (un contrôle dont la condition était insatisfiable), la sonde P1 de D8 (un
critère vrai avant toute tentative) et le confondeur de D9.

### La forme retenue : TROIS observations dans la MÊME exécution, et deux d'entre elles peuvent DISQUALIFIER le run

🔴 **La règle qui gouverne : le verdict « non capturé » n'est prononçable que si
les observations (a) et (b) sont TOUTES DEUX POSITIVES.** Sinon le verdict est
**NON MESURABLE**, et c'est ce qui est écrit — jamais « défavorable ».

| | Observation | Ce qu'elle établit | Ce qu'elle fait si elle est négative |
| --- | --- | --- | --- |
| **(a)** | **Le TÉMOIN POSITIF** — une fenêtre ordinaire, non élevée, au contenu reconnaissable, ouverte **dans la même exécution et sur le même chemin de capture**, EST vue | **l'instrument sait observer** | 🔴 **run DISQUALIFIÉ** : `NON MESURABLE — l'instrument n'a rien vu du tout` |
| **(b)** | **LA CHOSE A EU LIEU** — `consent.exe` est **relevé vivant** pendant la fenêtre d'observation (`Get-Process consent`, échantillonné à 1 Hz), et l'appel élévateur **attend** | **une élévation a réellement été demandée** | 🔴 **run DISQUALIFIÉ** : `NON MESURABLE — aucune élévation n'a eu lieu`, avec la configuration UAC relevée à l'appui |
| **(c)** | **LA MESURE** — le superviseur a-t-il **vu une fenêtre** pour `consent.exe`, et l'image de cette fenêtre est-elle **non vide** | le fait cherché | c'est le verdict, et lui seul |

**(b) est ce qui rend un zéro décidable**, et c'est le cœur de la porte : sans
`consent.exe` au relevé, un zéro d'image ne dit **rien**. Avec lui, un zéro dit
quelque chose.

⚠️ **`consent.exe` est le nom du processus qui porte la boîte de dialogue UAC
sur Windows — c'est une connaissance de plateforme, pas une mesure de ce
dépôt.** Si la sonde ne le trouve jamais alors que l'appel élévateur attend
manifestement, **l'observation (b) est indéterminée** et le run est disqualifié
de la même façon : on n'invente pas un second indice pour sauver un verdict.

### Ce que la sonde relève EN PLUS, et pourquoi chaque ligne compte

- **`EnableLUA` et `PromptOnSecureDesktop`** (`HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System`).
  🔵 **`PromptOnSecureDesktop = 0` déplace la boîte de dialogue sur le bureau
  ORDINAIRE**, donc dans le champ de la capture. Si le verdict est défavorable,
  **c'est le repli le moins cher qui existe** — et c'est une **décision de
  sécurité**, qui n'appartient ni à ce plan ni à son implémenteur : elle est
  **nommée, mesurée, et laissée au propriétaire du dépôt**. La sonde relève donc
  la valeur **avant** de conclure, sans quoi un verdict « capturé » pourrait
  n'être qu'un artefact de configuration.
- **Le compte de l'utilisateur interactif est-il administrateur** (`whoami /groups`) :
  s'il ne l'est pas, l'élévation demande **des identifiants**, pas un
  consentement — un cas strictement pire, et qui n'a pas le même repli.
- **`OpenInputDesktop` discrimine-t-il ?** Pendant que `consent.exe` vit, un
  appel à `OpenInputDesktop` depuis un processus ordinaire échoue-t-il, et
  réussit-il en dehors ? 🔵 **Si oui, le produit gagne un détecteur** — il peut
  dire *« une élévation est demandée et vous ne pouvez pas la voir »* au lieu de
  laisser un écran figé (D15). **Si non, c'est écrit, et le produit retombe sur
  la seule expiration.** ⚠️ Les deux issues sont acceptables ; **ne pas jouer la
  mesure ne l'est pas.**
- **`CreateProcess` sur le même exécutable rend-il `ERROR_ELEVATION_REQUIRED`
  (740) ?** 🔴 **C'est la prémisse de D15, et c'est une lecture de documentation
  tant que personne ne l'a jouée.** Elle ne disqualifie rien ; elle décide de la
  forme du refus typé de la tâche 19.

⚠️ **Ces deux dernières observations ne DÉCIDENT de rien** — seules (a), (b) et
(c) portent le verdict. Elles informent la conception, et c'est pourquoi elles
sont prises **dans la même exécution** plutôt que reportées à une sonde qu'on
n'écrira jamais.

### Ce que chaque verdict COMMANDE — écrit d'avance, pour que personne n'arbitre sous le coup du résultat

| Verdict | Ce qu'il commande |
| --- | --- |
| 🟢 **CAPTURÉE** (et `PromptOnSecureDesktop = 1`) | Le risque n°1 tombe. G3 se déroule tel quel ; le §9 du document de résultats l'écrit avec son nombre d'exécutions |
| 🟡 **CAPTURÉE, mais `PromptOnSecureDesktop = 0`** | Le risque tombe **pour cette VM et par sa configuration**, pas par le produit. **Écrire les deux moitiés de la phrase** — un successeur qui lirait la première seule croirait le problème résolu partout |
| 🔴 **NON CAPTURÉE** | **Le périmètre v1 se réduit, et il se nomme** — voir ci-dessous. G3 **reste livrable**, et c'est D15 qui le rend vrai |
| ⚪ **NON MESURABLE** | **Aucune conclusion.** Le sous-bloc ne se déclare ni gagné ni perdu sur ce point ; il journalise pourquoi et **rejoue** avec l'observation manquante réparée |

**Si le verdict est NON CAPTURÉE — ce qui survit, nommément :**

- **Les installeurs qui n'exigent aucune élévation** : les installeurs **par
  utilisateur** (`%LOCALAPPDATA%`, la forme de Chrome, de VS Code « User
  Setup », de la plupart des installeurs Squirrel/NSIS modernes), les
  exécutables **portables**, et les `.msi` **installés dans la portée
  utilisateur** quand le paquet l'autorise.
- **Le chemin entier de G3** — dépôt, tranches, reprise, empreintes, exécution,
  progression, verdict par réconciliation — **est le même dans les deux cas.**
  Rien de ce plan n'est à jeter.

**Ce qui devient hors v1, et doit être écrit en toutes lettres dans `CLAUDE.md`
plutôt que découvert en production :**

- **tout installeur qui exige une élévation au démarrage** — il est **refusé
  avec son motif** (`elevation-requise`), jamais lancé dans le vide (D15) ;
- **tout installeur qui s'élève LUI-MÊME en cours de route** — celui-là, **rien
  ne le refuse à l'avance**, et c'est la limite honnête du remède : il se
  présentera comme une exécution qui n'avance plus, bornée par
  `EXPIRATION_EXECUTION`, et signalée par le détecteur de bureau sécurisé **si
  et seulement si** l'observation `OpenInputDesktop` de la porte a discriminé.

**Les deux replis connus, NOMMÉS ET NON ARBITRÉS** — ce sont des décisions de
sécurité, et la spec §11 le dit déjà (« repli connu mais non arbitré : agent
élevé — décision de sécurité, hors de ce document ») :

1. **lancer l'agent élevé** : toute la surface du produit gagnerait alors les
   privilèges d'administrateur, y compris ce qu'un installeur arbitraire en
   ferait ;
2. **poser `PromptOnSecureDesktop = 0` sur la VM** : moins cher, et cela désarme
   une protection dont l'objet est précisément d'empêcher un programme de
   l'écran de contrefaire ou de piloter le consentement.

🔴 **Ni l'un ni l'autre n'est mis en œuvre par G3, et aucun ne doit l'être « en
passant ».**

---

## Décisions tranchées

### D1 — 🔴 `PLATEFORME_VERSION` prend le SUIVANT, et ce plan n'écrit AUCUN nombre en dur

**La question est posée par le brief, et deux montées concurrentes vers le même
numéro seraient une panne SILENCIEUSE.** Voici pourquoi, et ce n'est pas une
précaution de style.

G3 ajoute cinq variantes (`Installer` descendante, `Progression` et `Termine`
montantes, plus deux enums de charge). G2, planifié et non implémenté, en ajoute
d'autres et **écrit `3` en toutes lettres** (sa décision D1). Si les deux
branches posaient `3`, on obtiendrait deux protocoles **différents** portant le
**même** numéro : `verifie_version` (`proto/src/plateforme.rs:103-114`,
`fn verifie_version`) les laisserait passer tous les deux, et c'est
`#[serde(deny_unknown_fields)]` qui refuserait — donc un refus de motif
**`forme`**, pas `version`. Or `agent/src/plateforme.rs::sur_refus` rend
`Fin::Definitive` **pour le seul `MotifCanal::Version`** et `Fin::Reprenable`
pour tous les autres : **une incompatibilité de format se déguiserait en boucle
de reconnexion sans terme** — le mode de panne que la recette de G1 a mesuré
(10 reprises jusqu'au palier de 30 s) et que l'en-tête du protocole nomme
lui-même « le plus coûteux à diagnostiquer ».

**Décision, en trois clauses :**

1. **La tâche 5 RELÈVE la valeur courante par la commande**
   (`grep -n 'PLATEFORME_VERSION' proto/src/plateforme.rs proto/ts/plateforme.ts`)
   **et pose la suivante.** Au 20 août 2026 elle vaut **2** des deux côtés
   (relevé) : G3 poserait donc **3** si G2 n'a pas fusionné, et **4** s'il a
   fusionné. **Ce plan n'écrit pas le nombre**, parce qu'écrire « 3 » ici
   **serait la collision même que cette décision existe pour éviter**.
2. **La tâche ÉCHOUE si les trois lieux ne portent pas la même valeur avant**
   (`proto/src/plateforme.rs`, `proto/ts/plateforme.ts`,
   `proto/plateforme-vectors.json:3` plus **chaque** chaîne `json` du fichier)
   **et la même valeur après.** Un trou de numérotation — une version 3 qui
   n'aurait jamais existé sur le fil parce que G2 n'a pas été implémenté —
   **n'est pas un problème** : rien, nulle part, n'énumère les versions ; ce
   qui compte est qu'aucun couple ne partage un numéro avec deux formats.
3. **Agent et plateforme se déploient AU MÊME COMMIT.** C'est la reconduite
   explicite de D5 de P3, de D10 de la spec de ④ et de D1 de G1 ; la reprendre
   sans le dire serait la subir.

✅ **Une chose joue en notre faveur depuis G1** : le refus est **hors
versionnement** (correction du 20 août 2026, trois clauses en tête de
`proto/src/plateforme.rs`, l. 49-70). Un agent d'une version antérieure **lira**
le refus, journalisera, et **renoncera** au lieu de boucler. **La rupture reste
une rupture ; elle est devenue diagnosticable**, et le critère ⑨ de la recette
le prouve pour la seconde fois.

### D2 — Le numéro de migration se RELÈVE par `ls`, il ne se suppose pas

`plateforme/src/base/migrations/` porte quatre fichiers (relevé), et le plan de
G2 revendique `0005-icones.sql`. **La tâche 23 prend le premier numéro libre**,
relevé par `ls plateforme/src/base/migrations/`, et met à jour
`plateforme/src/base/pilotes.test.ts:33` avec la liste **exacte** qu'elle
observe. `migrations.ts` trie **numériquement** et n'exige **aucune contiguïté**
(relevé) : un `[1, 2, 3, 4, 6]` est parfaitement valide si G2 n'a pas fusionné.

### D3 — Les deux dettes de `proto/` : le PREMIER ARRIVÉ extrait, le second CONSTATE

`CLAUDE.md` inscrit `proto/src/plateforme/tests.rs` (**561**) et
`proto/ts/plateforme.test.ts` (**512**) sans point de chute ; **le plan de G2
les revendique et nomme leur point de chute** (`tests_apps.rs`,
`plateforme-apps.test.ts`, sa décision D2). G3 travaille dans les deux fichiers
lui aussi.

**Décision : G3 n'invente pas un second découpage.**

- **Si l'extraction de G2 a déjà eu lieu** (les fichiers font moins de 500 et
  `tests_apps.rs` existe) : G3 **constate**, ajoute ses cas **dans le fichier
  d'apps**, et les tâches 3 et 4 se réduisent à un relevé de `wc -l` versé dans
  le rapport.
- **Sinon** : G3 extrait **selon la frontière que G2 a nommée**, à l'identique —
  le cycle de vie reste (version, refus, enrôlement, battement), la gestion
  d'apps part. **Aucune autre frontière n'est admissible** : deux découpages
  concurrents du même fichier produiraient un conflit que personne ne saurait
  arbitrer.

⚠️ **La déclaration se fait par `#[path]` DEPUIS `plateforme.rs`**, comme le
`#[cfg(test)] #[path = "plateforme/tests.rs"] mod tests;` existant
(`proto/src/plateforme.rs`, tout en fin de fichier). Ce n'est **pas** la
« Convention de module enfant » de `CLAUDE.md`, qui vise les modules extraits
d'un parent `#[cfg(windows)]` : c'est le même mécanisme Rust employé pour la
règle des 500 lignes, et `CLAUDE.md` le nomme explicitement hors de portée de
cette convention.

### D4 — Le chemin des octets : sept routes, et le canal ne porte QUE des ordres

**Le canal `/agent` ne transporte jamais un installeur** (spec D7) : il est en
JSON, il porte le battement de cœur, et une tranche de 8 Mio y coûterait +33 %
en base64. La file montante de l'agent est en outre **bornée à 32 messages**
(`FILE_EMISSION`, `agent/src/plateforme.rs`) et **abandonne ce qui déborde**.

| # | Qui → qui | Quoi |
| --- | --- | --- |
| 1 | navigateur → plateforme | `POST /televersement` `{nom, taille, sha256}` → `{id, taille_tranche, tranches_presentes: []}` |
| 2 | navigateur → plateforme | `PUT /televersement/:id/tranche/:n`, `application/octet-stream`. **Idempotent** : redéposer `n` l'écrase |
| 3 | navigateur → plateforme | `GET /televersement/:id` → l'état, dont `tranches_presentes` **obtenu par LISTAGE** (D7) |
| 4 | navigateur → plateforme | `POST /televersement/:id/sceller` — 🔴 **la plateforme recalcule le SHA-256** en flux et refuse s'il diffère |
| 5 | navigateur → plateforme | `POST /installation` `{vm, televersement}` → `{id}` |
| 6 | plateforme → agent, sur `/agent` | `Installer { installation, url, sha256, nom, taille }` |
| 7 | agent → plateforme, en **HTTP** | `GET /televersement/:id/contenu`, **jeton d'AGENT** en en-tête. 🔴 **L'agent recalcule l'empreinte** après écriture |
| 8 | agent → plateforme, sur `/agent` | `Progression`, puis `Termine` |
| 9 | navigateur → plateforme | `GET /installation/:id` → état, phase, code de sortie, issue, queue de journal |

**Trois vérifications, une seule valeur, et aucun saut ne fait confiance au
précédent** (spec D7) : le navigateur peut mentir, le disque de la plateforme
peut se corrompre, le transfert vers la VM peut tronquer.

⚠️ **Le plafond de corps de `routes-auth.ts` (4 Kio, `CORPS_MAX_OCTETS:72`) ne
s'applique pas ici et NE DOIT SURTOUT PAS ÊTRE RELEVÉ** — la spec D7 l'écrit
déjà. Chaque route de G3 a **son propre** plafond (D8).

### D5 — L'empreinte du navigateur est ANNONCÉE À LA CRÉATION, et calculée en JS

**Décision : le SHA-256 du fichier entier, écrit en JS incrémental, annoncé à
l'étape 1.**

**Pourquoi en JS, et pas par l'API du navigateur** : M1 — `crypto.subtle`
n'expose que `digest`, sur un tampon **complet**. Empreindre 800 Mo par cette
voie exige de tenir 800 Mo dans un onglet. Le prix du remède est **mesuré** :
**74,7 Mo/s** en JS pur contre 1 160,6 Mo/s en natif (M2), soit **≈ 11 s pour
800 Mo** sur cette machine. **11 s est payable ; 800 Mo en mémoire ne l'est
pas.**

**Pourquoi à la création, et pas au scellement** — et c'est l'argument qui
décide : **c'est l'empreinte annoncée qui rend la REPRISE sûre.** Après un
rechargement d'onglet, l'utilisateur re-choisit un fichier ; rien, sinon
`{taille, sha256}`, ne dit que c'est **le même**. Sans elle, des tranches de
deux fichiers différents se mélangeraient et le scellement échouerait **sans
que rien ne dise pourquoi**. Avec elle, le client **refuse** un fichier qui ne
correspond pas, avec son motif.

⚠️ **Le coût est une passe de lecture COMPLÈTE avant le premier octet
téléversé**, et il est **affiché** : c'est la phase `empreinte` de la
progression, jamais un gel silencieux. L'implémentation cède la main entre deux
tranches (`await`), pour que l'onglet reste vivant.

⚠️ **Ce que l'on ne fait PAS** : une empreinte d'arbre sur les empreintes de
tranches, qui serait native et gratuite (`crypto.subtle.digest` par tranche de
8 Mio). Elle est **écartée délibérément** parce que la valeur cesserait d'être
le SHA-256 du fichier — donc impossible à comparer à un `sha256sum` par un
humain, et impossible à recalculer par l'agent sans connaître le découpage.
**Une seule valeur, comparable partout.**

### D6 — La règle de découpage vit dans `proto/ts/`, parce qu'elle est un CONTRAT

Le navigateur découpe, la plateforme vérifie que le découpage est complet.
**Deux arithmétiques indépendantes divergeraient un jour**, et le symptôme
serait un scellement qui refuse sans que l'on sache lequel des deux a tort.

**Décision : `proto/ts/tranches.ts`, PUR, sans DOM et sans `node:`**, consommé
par le navigateur **et** par la plateforme — qui importent déjà
`../../../proto/ts/plateforme` l'un et l'autre. Il rend, depuis
`(taille, taille_tranche)`, le nombre de tranches et la taille **exacte** de
chacune ; et, depuis la liste des tranches présentes avec leurs octets, un
verdict `complet` / `manquantes` / `incoherentes`.

🔴 **`incoherentes` n'est PAS `manquantes`, et les confondre serait une faute** :
une tranche présente **à la mauvaise taille** est une erreur de protocole, pas
un trou à recompléter — la redemander en boucle ne la réparerait jamais.

`proto/ts/sha256.ts` le rejoint, **pour la même raison et une de plus** : c'est
la valeur que les trois étages comparent, et la recette (D14) l'exécute depuis
Node **sans navigateur**, ce qui n'est possible que si elle vit dans un paquet
que Node sait charger.

### D7 — Les tranches ne sont JAMAIS assemblées

**Décision : le magasin est `<PLATEFORME_TELEVERSEMENTS>/<id>/<n>`, un fichier
par tranche, et il n'existe aucun fichier assemblé.**

- **Le scellement est une passe de FLUX** : il ouvre les tranches dans l'ordre,
  les fait passer par un `createHash('sha256')` de `node:crypto`, compare, et
  écrit `scelle_a`. Rien n'est recopié.
- **`GET …/contenu` sert la CONCATÉNATION en flux**, avec un `Content-Length`
  égal à la `taille` annoncée — vérifiée au scellement contre la somme des
  tranches.
- **La reprise est un LISTAGE de répertoire** (spec D7, mot pour mot : « jamais
  par une table de comptabilité qui pourrait diverger du disque »).

**Ce que cela évite** : le doublement de l'espace disque au scellement — 1,6 Go
pour un installeur de 800 Mo —, et une seconde source de vérité.

⚠️ **Le prix est nommé** : un `GET …/contenu` ouvre N descripteurs
successivement, et une tranche effacée sous les pieds du service donne une
réponse **tronquée**. C'est précisément ce que la troisième vérification
d'empreinte attrape, côté agent, et c'est la raison d'être de sa présence
(spec D7).

### D8 — Un corps binaire ne passe PAS par `lireCorps`, et chaque route porte son plafond

`routes-auth.ts::lireCorps` accumule dans une **chaîne UTF-8** (l. 103-119,
relu) : lui donner un corps binaire le corromprait, et lui donner 8 Mio le
mettrait en mémoire.

**Décision : `PUT …/tranche/:n` écrit en FLUX vers son fichier**, avec un
compteur d'octets et une **borne dure** — `TRANCHE_MAX_OCTETS = taille_tranche`
du téléversement, relue en base. Au franchissement : `req.destroy()`, `413`, et
**le fichier partiel est supprimé**. Rien n'est accumulé, jamais.

**Les plafonds, tous NON CALIBRÉS et déclarés tels :**

| Constante | Valeur | Ce qui la fonde |
| --- | --- | --- |
| `TAILLE_TRANCHE` | **8 Mio** | la valeur proposée par la spec D7. Majorante à vue |
| `TELEVERSEMENT_MAX_OCTETS` | **4 Gio** | une borne pour qu'un utilisateur authentifié ne remplisse pas le disque par accident. **Aucune mesure ne la fonde** |
| `TELEVERSEMENTS_EN_COURS_MAX` | **3** par utilisateur | idem |
| `EXPIRATION_TELEVERSEMENT` | **24 h** | valeur proposée par la spec |
| `EXPIRATION_INSTALLEUR` | **24 h** | idem, côté VM |
| `EXPIRATION_EXECUTION` | **2 h** | idem |
| `PERIODE_PROGRESSION` | **1 s** | voir D9 |
| `JOURNAL_MAX_OCTETS` | **64 Kio** | la queue bornée que la spec D9 nomme |

⚠️ **Le frein anti-force-brute n'est PAS posé sur ces routes**, et c'est un
choix, pas un oubli. Le frein (`securite/frein.ts`) existe pour les portes
**pré-authentifiées** — `/auth/connexion` et l'enrôlement `/agent` — où un pair
anonyme devine un secret. Les routes de G3 exigent toutes un jeton valide.
**Ce qui les protège est un QUOTA, pas un frein** : `TELEVERSEMENT_MAX_OCTETS`,
`TELEVERSEMENTS_EN_COURS_MAX` et le balayage d'âge. ⚠️ **Cela reconduit le legs
de ⑤** — `GET /vm` et `POST /session` ne sont pas freinées non plus — et **le
reconduire est une décision qui se déclare** : un utilisateur authentifié peut
faire travailler le disque du service, et rien ne l'en empêche au-delà de son
quota.

### D9 — 🔴 Le verdict se lit sur une FENÊTRE, pas sur « la réconciliation suivante »

La spec D9 tranche : *une installation est `reussie` quand la réconciliation qui
la suit rapporte au moins une application apparue.* **Prise à la lettre, cette
phrase produit un faux `sans_effet`**, et voici comment.

`PERIODE_RECONCILIATION` vaut **30 s** (`agent/src/apps.rs:34`), et la boucle
tourne **pendant** que l'installeur travaille. Un installeur qui pose son
raccourci à la trentième seconde puis continue trois minutes verra ses
applications **apparaître dans une réconciliation intercalaire** ; celle qui
suit sa **sortie** n'aura alors **plus rien de neuf à rapporter**, et le verdict
serait `sans_effet` pour une installation parfaitement réussie.

**Décision : une FENÊTRE de comptage, ouverte au lancement, fermée après la
sortie.** Chaque réconciliation ajoute `diff.apparues.len()` à toutes les
fenêtres ouvertes ; à la sortie du processus, l'agent **force une
réconciliation**, ferme la fenêtre, et lit le total.

- `installation/fenetre.rs` est **PUR** (ouvrir, ajouter, fermer) et se teste
  sur l'hôte ;
- `installation/verdict.rs` est **PUR** : `(code de sortie, apparues, expiré) →
  Issue`.

🔴 **Le code de sortie est RAPPORTÉ, jamais INTERPRÉTÉ** (spec D9) : `msiexec`
rend **3010** pour un succès qui demande un redémarrage, et beaucoup
d'installeurs rendent **0** après une annulation. C'est la doctrine que D8 a
payée sur un autre terrain — **juger sur la relecture, jamais sur le code de
retour**.

| Issue | Quand |
| --- | --- |
| `reussie` | la fenêtre a compté **au moins une** application apparue |
| `sans_effet` | la fenêtre s'est fermée à **zéro** |
| `issue_inconnue` | le code de sortie **n'a pas pu être recueilli** — agent mort, expiration |
| `refusee` | l'installation n'a jamais démarré : empreinte fausse, élévation requise, extension refusée, disque plein |

⚠️ **`refusee` est une addition à la spec, et elle est justifiée** : les quatre
cas qu'elle couvre ne sont ni un succès, ni un « sans effet », ni une ignorance
— **ce sont des refus, et ils portent leur motif.** Les fondre dans
`issue_inconnue` ferait lire « on ne sait pas » là où l'on sait très bien (E5).

**Progression** (cadrage §7) : phases `empreinte` (navigateur), `transfert`,
`execution`, `reconciliation`. 🔴 **La phase `execution` ne porte AUCUN
pourcentage** (spec D9) — un installeur n'en publie pas, en inventer un serait
mentir ; elle porte le **temps écoulé**, et l'interface affiche un état
indéterminé.

⚠️ **La progression est ÉCHANTILLONNÉE, à `PERIODE_PROGRESSION` (1 s), et c'est
une contrainte de sûreté, pas de confort** : la file montante est bornée à 32
et **journalise chaque abandon**. Une progression par tranche de 64 Kio la
saturerait et **noierait le journal partagé** — c'est la doctrine du chantier
TURN : *compter ou échantillonner, jamais tracer par paquet*.
`installation/cadence.rs` porte la règle, **horloge en paramètre**, et se teste
sur l'hôte.

⚠️ **Un journal d'installeur VIDE est le cas NORMAL** (spec D9) : la plupart des
installeurs Windows sont graphiques et n'écrivent rien sur les flux standard.
**L'interface ne doit pas le présenter comme un échec**, et le champ dit
explicitement `journal_vide` plutôt que de rendre une chaîne vide qu'un client
lirait comme une absence de réponse.

### D10 — 🔴 L'exécution hors job : on MESURE d'abord, et le pont est le vrai danger

La spec D8 tranche : **l'installeur n'est PAS assigné au job object du
superviseur**, parce qu'un redémarrage d'agent le tuerait au milieu d'une
écriture de registre et laisserait **la machine à moitié installée**.

**Lecture de l'arbre (M5)** : le superviseur crée le job et **n'y assigne que
ses enfants** ; **il ne s'y assigne pas lui-même**. Un processus qu'il crée
n'hérite donc d'aucun job, et **il n'y a rien à faire** — pas de
`CREATE_BREAKAWAY_FROM_JOB`, pas de `JOB_OBJECT_LIMIT_BREAKAWAY_OK`, aucun
drapeau ajouté à `lanceur.rs`.

🔴 **Mais deux choses rendent cette conclusion insuffisante, et ce sont elles
qui décident** :

1. **Le critère ⑦ serait VACUEUX.** « Tuer l'agent ne tue pas l'installeur » est
   vrai **par construction** si l'agent n'est dans aucun job : le contrôle ne
   peut pas échouer, et un vert ne prouve rien. **La sonde U2 (tâche 2) relève
   `IsProcessInJob` du processus agent lui-même**, et le produit **journalise ce
   relevé à chaque installation**. C'est cette ligne qui rend le critère
   décidable, et c'est elle que la recette lit.
2. **Le pont, lui, EST dans le job.** `apps::brancher` est appelée
   (`agent/src/main.rs:275`) **avant** l'aiguillage `PONT` (l. 299), et sous le
   leg n°1 de G1 le pont s'enrôle sous le même `vm_id` que son père : **l'ordre
   `Installer` peut lui échoir**. Il lancerait alors l'installeur depuis un
   processus assigné au job, **et l'installeur mourrait avec le superviseur**.

**Décision, en deux clauses :**

- **l'agent REFUSE d'installer depuis un processus assigné à un job**, avec un
  refus typé (`installation refusée : ce processus est assigné à un job object,
  l'installeur y mourrait avec lui`) et une issue `refusee`. **Un refus bruyant
  vaut mieux qu'une installation qu'un redéploiement tuera au milieu** ;
- **ce refus est un GARDE, pas le remède** : le remède est le leg n°1 de G1, qui
  n'appartient pas à G3 — trois décisions y sont possibles et aucune n'est
  tranchée. **G3 le nomme, s'en protège, et ne le referme pas.**

⚠️ **`IsProcessInJob` exige la fonctionnalité `Win32_System_JobObjects` du crate
`windows`** — déjà présente, puisque `lanceur.rs` l'emploie. **Aucune addition à
`agent/Cargo.toml`. À CONFIRMER par la compilation, pas à affirmer.**

### D11 — Deux files distinctes, et deux extractions avant addition

`Canal::ordres()` rend `mpsc::UnboundedReceiver<(String, String)>`
(`agent/src/plateforme.rs:225`) et **un seul consommateur peut la prendre** — sa
documentation le dit : « deux se voleraient les ordres l'un à l'autre, et le
symptôme serait *un lancement sur deux ne part pas* ».

**Décision : une SECONDE file, `Canal::installations()`, plutôt qu'un tuple
élargi ou un enum poussé dans la première.** Le canal aiguille à la source :
`Lancer` va aux ordres, `Installer` aux installations. Deux familles, deux
consommateurs, **chacune avec un seul**. La règle du consommateur unique est
respectée à la lettre, et **`apps/boucle.rs` n'a pas à connaître les
installations** — il ne fournit que le comptage de sa fenêtre.

🔴 **Les deux fichiers qui accueillent ces branches sont trop pleins, et
l'extraction PRÉCÈDE l'addition, sans exception** — la doctrine de ce dépôt
(D9, `serveur/instances.rs`) est qu'une marge traitée **avant** est rendue, et
qu'une marge traitée **après** est compressée :

| Fichier | Lignes | Point de chute nommé |
| --- | --- | --- |
| `agent/src/plateforme.rs` | **453** | `agent/src/plateforme/session.rs` — `une_session`, `connecter` et `sur_refus`, c'est-à-dire **une session du canal** ; `plateforme.rs` garde le type `Canal`, l'ouverture et la reprise |
| `plateforme/src/agents/canal.ts` | **423** | `plateforme/src/agents/canal-apps.ts` — les branches **montantes de ④** (`catalogue`, `lancee`, et les neuves `progression`, `termine`) ; `canal.ts` garde le **cycle de vie** (enrôlement, battement, refus, frein) |

⚠️ **La frontière est la MÊME que celle de D3 et que celle du plan de G2** :
cycle de vie d'un côté, gestion d'apps de l'autre. Trois fichiers, une seule
coupure — c'est ce qui la rend mémorisable.

### D12 — Le jeton d'AGENT a son propre lecteur, PUR, et la VM se résout par le PRÉFIXE

`lirePorteur` (`plateforme/src/http/porteur.ts`) **refuse** un jeton d'agent par
`403 jeton-agent` (l. 80-86) : il ne peut donc pas servir `GET …/contenu`, la
seule route que l'agent appelle.

**Décision : `plateforme/src/http/porteur-agent.ts`, PUR, symétrique**, qui
exige `type === 'agent'` et rend le **sujet**, c'est-à-dire le **préfixe de
session** — `agents/canal.ts` signe `jetonNeuf(verdict.prefixe)`, jamais
l'identifiant de VM. La VM se résout ensuite par
`depot/agent.ts::lireParPrefixe` (l. 77, existante).

🔴 **L'AUTORISATION N'EST PAS « un agent valide », C'EST « CET agent-LÀ »** :
la route compare la VM du jeton à `installation.vm_id`, et refuse sinon.
**Sans cette comparaison, n'importe quelle VM enrôlée pourrait télécharger
l'installeur de n'importe quelle autre** — c'est-à-dire le contenu que son
propriétaire a déposé pour lui seul. C'est la garde la plus importante de tout
le sous-bloc, et elle a sa mutation dédiée à la tâche 28.

⚠️ **Si G2 a déjà livré un lecteur de jeton d'agent** (son `PUT /icone/:sha256`
en a besoin), **le RÉEMPLOYER, ne pas en écrire un second** : « une copie
divergerait en silence », doctrine d'`agents/canal.ts`.

### D13 — Le client HTTP de l'agent est écrit à la main, et il refuse BRUYAMMENT ce qu'il ne sait pas faire

M3 : l'agent n'a **aucun** client HTTP, et `tokio-tungstenite` est verrouillé
**sans TLS**. Ajouter `reqwest` apporterait une pile TLS entière et romprait
l'invariant « aucune dépendance de production » que G1 et G2 tiennent tous deux.
Ce dépôt a déjà écrit son client TURN, son codec STUN et son SHA-256 pour la
même raison.

**Décision : `agent/src/installation/telechargement.rs`, portable** —
`tokio::net::TcpStream`, aucun `cfg` —, avec sa moitié pure
`installation/reponse.rs` (ligne de statut, en-têtes, `Content-Length`).

**Trois refus BRUYANTS, chacun avec son motif, plutôt qu'une interprétation :**

1. **`https://`** — l'agent ne parle pas TLS : refus typé nommant la capacité
   manquante. ⚠️ **Ce n'est PAS une régression de G3** : le canal `/agent`
   lui-même ne parle que `ws://` aujourd'hui. L'URL de téléchargement se dérive
   de `SIGNALING_URL` (`ws://h:p` → `http://h:p`), comme `url_du_canal` dérive
   déjà la sienne.
2. **`Transfer-Encoding: chunked`** — non pris en charge, refus typé. Le service
   pose un `Content-Length` ; un proxy pourrait le remplacer, et **cela n'a pas
   été mesuré** (M4 : le profil ne route même pas ces chemins). **Un refus
   nommé se diagnostique en une ligne de journal ; un analyseur qui devine se
   diagnostique en une campagne.**
3. **tout statut hors `200` et `206`** — refus typé portant le statut.

**La reprise** : sur coupure en cours de transfert, l'agent rouvre avec
`Range: bytes=<déjà écrit>-` et attend un `206`, **`RETABLISSEMENTS_MAX = 5`
fois** (NON CALIBRÉE). Un `200` en réponse à un `Range` fait **repartir de
zéro** — jamais concaténer, ce qui produirait un fichier plus long que sa
taille et une empreinte fausse **sans que l'on sache pourquoi**.

🔴 **L'empreinte se calcule PENDANT l'écriture, pas en relisant le fichier
après** — sauf sur un chemin de reprise, où l'agent **relit ce qu'il a déjà
écrit** pour réamorcer son état de condensation. Le dire ici évite qu'on
« optimise » cette relecture un jour : sans elle, une reprise donne une
empreinte de la seule fin du fichier.

### D14 — Aucune page de hub ; la recette EXÉCUTE le code du produit

G1 (sa décision D12) et G2 ont tous deux écarté la page, au motif que `curl`
éprouve une route davantage qu'une page. G3 va **un cran plus loin**, parce que
la vérification n°1 est *le navigateur calcule l'empreinte* : sans consommateur,
`tranches.ts` et `sha256.ts` seraient du **code orphelin**, et ce dépôt n'a
aucune doctrine sur le code orphelin (leg de D10).

**Décision :**

- `client/src/hub/televersement.ts` porte l'orchestration — **`fetch` et
  l'horloge sont des PARAMÈTRES**, il n'y a **aucun DOM** — et il se teste sur
  l'hôte avec un `fetch` factice ;
- **le pilote de recette IMPORTE ce module** et le lance sous Node (par `tsx`,
  déjà présent dans `plateforme/`) contre la plateforme réelle et un vrai
  fichier de plusieurs centaines de mégaoctets. **Le code qui téléverse pendant
  la recette EST le code du produit**, pas une réimplémentation `curl` qui
  n'éprouverait qu'elle-même ;
- **G5 lui accroche une page** (glisser-déposer, `showOpenFilePicker`, types
  installeur du hub) — c'est son objet, et l'amendement du 28/07 le lui donne.

⚠️ **Ce que cela ne prouve PAS, et qui doit être écrit dans les résultats** :
Node n'est pas un navigateur. Le débit de `sha256.ts` sous V8 côté Node est
mesuré (M2) ; **sous un moteur de navigateur, il ne l'est pas**, et `File.slice`
n'est pas `fs.read`. **La vérification n°1 est éprouvée sur son CODE, pas dans
son ENVIRONNEMENT.**

### D15 — 🔴 `CreateProcessW`, et non `ShellExecuteExW` : c'est ce qui transforme le risque n°1 en REFUS TYPÉ

G1 lance les applications par `ShellExecuteExW`
(`agent/src/apps/lancement.rs`), et c'est juste : un double-clic honore le
répertoire de travail, le verbe et le `nShow`.

**Pour un installeur, la décision est INVERSE.**

`ShellExecuteExW` **déclenche l'élévation** quand le manifeste de la cible la
demande : la boîte de dialogue s'ouvre — sur le bureau sécurisé, si la porte le
confirme — et l'appel **attend**. L'utilisateur voit un écran figé et le produit
ne sait rien dire. `CreateProcessW`, lui, **ne s'élève JAMAIS** : il échoue avec
**`ERROR_ELEVATION_REQUIRED` (740)**.

⚠️ **CETTE DERNIÈRE PHRASE EST UNE LECTURE DE LA DOCUMENTATION WINDOWS, PAS UNE
MESURE DE CE DÉPÔT**, et c'est la prémisse de tout le remède. **La porte la
confirme ou la réfute au passage** — observation (e) de la tâche 1, qui ne coûte
rien puisque l'exécutable élévateur est déjà là. **Si elle est réfutée, D15
tombe** et le refus typé doit être reconstruit sur un autre indice ; le dire ici
évite qu'on bâtisse une famille de refus sur une phrase que personne n'a
éprouvée — c'est exactement ce que le libellé de `MF_E_UNSUPPORTED_D3D_TYPE` a
coûté à ce dépôt le 31 juillet 2026.

**Décision : `CreateProcessW`, et `ERROR_ELEVATION_REQUIRED` devient
`issue = refusee, motif = elevation-requise`** — un message que le hub peut
afficher, au lieu d'une attente que personne ne comprend.

⚠️ **La portée exacte du remède, et sa limite** :

- il attrape les installeurs dont le **manifeste** demande l'élévation, et ceux
  que la **détection d'installeur** de Windows marque comme tels ;
- 🔴 **il n'attrape PAS un installeur qui s'élève LUI-MÊME en cours de route** —
  il démarre sans privilège puis appelle `ShellExecute … runas`. Celui-là
  provoquera la boîte de dialogue que la porte décrit, et **rien ne l'en
  empêche**. Il est borné par `EXPIRATION_EXECUTION` et, **si et seulement si**
  l'observation `OpenInputDesktop` de la porte a discriminé, signalé par une
  phase `elevation-attendue` ;
- **`msiexec /i <fichier>`** pour un paquet par machine passe par le service
  Windows Installer, qui demande son propre consentement : il tombe dans le
  second cas, pas dans le premier.

**Aucun mode silencieux n'est imposé** (spec D8) : `.exe` est exécuté tel quel,
`.msi` passe par `msiexec /i`. **Extensions acceptées : `.exe` et `.msi`.**
`.bat` est **refusé avec son motif** — c'est un script, dont l'interprète, le
répertoire de travail et la politique d'exécution appellent leurs propres
décisions. La règle est **pure** (`installation/depot.rs`) et testée sur l'hôte.

### D16 — Une seule variable neuve, et elle est de BANC

**`INSTALLATION_FAUTE=empreinte`** : l'agent altère **un octet** du fichier
**après écriture et avant vérification**, ce qui exerce la **troisième**
vérification d'empreinte sur le chemin réel — autrement inatteignable sans
corrompre quelque chose à la main pendant un transfert.

⚠️ **CONVENTION DIFFÉRENTE de `PLEIN_ECRAN`/`AUDIO`/`APPS`, et il faut le
dire** : ici la **valeur NOMME une faute**, elle ne désarme rien — même figure
qu'`AUDIO_PERIPHERIQUE` (qui nomme un périphérique) et que
`AUDIO_FAUTE_LECTURE`. **Absente, tout est armé normalement.** Trace émise
**seulement si posée**, en `warn!` : `faute d'installation ARMEE (INSTALLATION_FAUTE=…) : banc, jamais une configuration livrée`.

🔴 **`APPS=0` DÉSARME AUSSI L'INSTALLATION**, puisque `apps::brancher` retourne
avant tout (D10). **C'est déclaré ici plutôt que découvert** ; aucune variable
séparée n'est ajoutée pour désarmer la seule installation, faute de besoin
démontré.

**La tâche 22 est DÉDIÉE à `scripts/run-agent.sh`** et ne fait rien d'autre —
règle de méthode n°6, piège payé cinq fois.

### D17 — 🔴 Une installation ne s'exécute JAMAIS deux fois, même après un redémarrage de l'agent

La spec D7 (chute n°2) demande que la plateforme **réémette l'ordre à chaque
enrôlement**, et que **l'agent déduplique par `installation.id`**. Une
déduplication en mémoire tient dans **un** processus — et l'agent redémarre.

**Le cas qui mord** : l'agent meurt pendant l'exécution ; la plateforme n'a
jamais reçu `Termine` ; elle réémet au réenrôlement ; l'agent, neuf, ne se
souvient de rien et **relance l'installeur** — sur une machine peut-être déjà
installée.

**Décision : la mémoire est SUR LE DISQUE, et c'est le répertoire lui-même.**
`%ProgramData%\Guacamole\installeurs\<installation-id>\` porte un marqueur
`.commence` écrit **avant** `CreateProcessW`, et un `.termine` écrit après la
sortie. À la réception d'un ordre, l'agent lit ces deux marqueurs :

| État observé | Ce que l'agent fait |
| --- | --- |
| aucun répertoire | il télécharge et exécute |
| `.commence` seul | il **n'exécute pas**, et rapporte `issue_inconnue` — l'installeur a tourné, son issue est perdue (spec D7, chute n°3) |
| `.termine` | il ne fait rien et **réémet le `Termine` qu'il a conservé** |

🔴 **« Il n'exécute pas » est la clause qui compte** : rejouer serait rejouer un
installeur sur une machine à l'état inconnu. **La plateforme, elle, cesse de
réémettre dès que l'état de la ligne n'est plus `en_attente`** — deux ceintures,
et la seconde protège du cas où la première a perdu son disque.

### D18 — Le nettoyage : qui supprime quoi, et quand

| Quoi | Qui | Quand |
| --- | --- | --- |
| le fichier d'installeur, dans la VM | l'agent | **après** que le code de sortie a été rapporté **ET** que la réconciliation qui ferme la fenêtre a eu lieu (spec D7). ⚠️ **Jamais avant** : une archive auto-extractible relit son propre fichier |
| les répertoires d'installeurs oubliés | l'agent | balayage d'âge à chaque réconciliation, au-delà d'`EXPIRATION_INSTALLEUR` (24 h). ⚠️ **Les marqueurs `.commence`/`.termine` partent avec** — donc la mémoire de D17 est **bornée à 24 h**, et c'est écrit |
| les tranches d'un téléversement | la plateforme | balayage d'âge au-delà d'`EXPIRATION_TELEVERSEMENT` (24 h), **et à la fin d'une installation `reussie`** |
| la ligne `televersement` | la plateforme | **jamais** tant qu'une `installation` la référence : la clé étrangère l'interdit, et c'est voulu — l'historique d'une installation doit rester lisible |

🔴 **Le balayage est OPPORTUNISTE, jamais un minuteur** : il court à la création
d'un téléversement et au démarrage du service. **Ce service n'a aucun minuteur
d'entretien aujourd'hui**, et en introduire un est une décision d'exploitation
(qui l'observe ? que fait-il si le disque est plein ?) qui n'appartient pas à
G3. **Le prix est nommé** : une plateforme qui reçoit un gros téléversement puis
plus rien le garde indéfiniment.

### D19 — 🔴 Exécuter un binaire arbitraire dans la VM d'autrui : ce que G3 en dit, et ce qu'il n'en fait pas

**Le cadrage l'assume en toutes lettres** (§3, « Isolation ») : « Indispensable :
exécution d'installeurs arbitraires uploadés », et **la mitigation est la VM
dédiée par utilisateur**. ④ ne l'atténue pas davantage et ne prétend pas le
faire. **G3 est le sous-bloc qui rend cette phrase exécutable, et il doit donc
l'écrire sans l'adoucir.**

**Ce que cela implique, nommément :**

- **un installeur peut casser la VM** : installer un pilote, corrompre le
  registre, remplir le disque, désactiver le réseau ;
- **un installeur peut désinstaller ou remplacer l'agent lui-même** — rien ne
  l'en empêche, il tourne avec les mêmes privilèges. La plateforme le verra par
  `vu_a` (`SEUIL_INJOIGNABLE_MS = 90_000`, `agents/fraicheur.ts:47`) et
  annoncera la VM `injoignable`. **Rien ne le répare automatiquement** ;
- **un installeur peut changer le périphérique audio par défaut, et CELA S'EST
  PRODUIT LE 19 AOÛT 2026** : l'installation de VB-Cable a fait basculer le
  rendu par défaut sur un câble virtuel que rien n'alimente, et le loopback du
  chantier A s'est mis à capter du silence **sans qu'aucune ligne de journal ne
  dise pourquoi**. **C'est le cas nominal, pas un cas limite** : tout installeur
  audio le rejouera.

**Ce que G3 fait, et rien de plus :**

- **il journalise le périphérique de rendu par défaut AVANT et APRÈS chaque
  installation** (spec D8), en réemployant la règle pure existante
  (`agent/src/wasapi/peripherique.rs`). **La prochaine occurrence devient
  attribuable par un `grep`, au lieu d'une campagne** ;
- **il n'ajoute AUCUN garde-fou de restauration** : remettre d'autorité le
  périphérique d'avant serait décider à la place de l'utilisateur qui vient
  d'installer un périphérique audio exprès ;
- **il n'exécute jamais rien sans un `POST /installation` explicite** nommant la
  VM **et** le téléversement, authentifié, et vérifié contre le propriétaire de
  la VM.

🔴 **ET IL N'Y A AUCUN RETOUR ARRIÈRE.** Le backend d'orchestration v1 **refuse
explicitement** `instantane` — `plateforme/src/orchestration/inventaire-statique.ts:111-114`,
`async instantane(…)` rend `refuserNonSupporte('instantane')`, exposé en `501`
et journalisé. **Il n'existe donc, dans tout le produit, aucun moyen de revenir
à l'état d'avant une installation.** Ce n'est ni un oubli de G3 ni quelque chose
qu'il puisse réparer : c'est une propriété du sous-projet ⑤, et **G3 est le
premier sous-bloc dont les conséquences la rendent visible**. Elle doit être
écrite dans `CLAUDE.md` **à côté de G3**, pas seulement à côté de ⑤.

### D20 — Ce que G3 laisse à G4 et à G5, explicitement

**À G4 (la surveillance)** :

- **la latence du verdict.** À la sortie de l'installeur, l'agent force une
  réconciliation ; c'est G4 qui, par `ReadDirectoryChangesW`, la rendra
  **immédiate** au lieu de la déclencher à la main. 🔴 **G3 NE DÉPEND PAS DE
  G4** : sa fenêtre de comptage fonctionne à `PERIODE_RECONCILIATION = 30 s`, et
  c'est exactement la garantie que l'ordre des sous-blocs de la spec §5 existe
  pour préserver ;
- **l'anti-rebond.** Un installeur écrit des dizaines de fichiers ; sans
  anti-rebond, G4 déclencherait autant de réconciliations. Le problème naît avec
  G4, pas avec G3.

**À G5 (la PWA et le hub)** : la page de dépôt, le glisser-déposer, les
`file_handlers` pour `.exe`/`.msi`, l'affichage de la progression et du refus.
G3 livre **l'orchestration sans DOM** que cette page consommera (D14).

**À personne, et donc dû** : le leg n°1 de G1 (le pont hérite de l'identité de
son père), dont G3 se protège par un refus (D10) sans le refermer.

---

## Divergences relevées entre la spec, ce que G1 a livré, et l'état RÉEL de l'arbre

**Toutes sont relevées par la commande ou par lecture datée du 20 août 2026.**
Une divergence n'est ni une erreur de la spec ni une licence prise par ce
plan : c'est un écart **nommé avant** l'implémentation, pour qu'il ne soit pas
découvert pendant.

### E1 — 🔴 Le profil de déploiement ne route AUCUNE route de ④, et il rend la PAGE

Voir M4. **Legs de G1 que personne n'a inscrit** : `GET /applications` et
`POST /application/:id/lancer` tombent dans `location /` et sont servies par
`try_files … /index.html` (`deploiement/nginx.conf:257`). **Derrière le profil
livré, la première route de ④ rend du HTML en 200.**

**Décision : la tâche 32 pose les `location` de G3 ET celles de G1**, dans le
même geste, parce que le fichier est ouvert et qu'un hub qui téléverse sans
pouvoir lister n'a aucun sens. ⚠️ **C'est une addition au périmètre, et elle est
déclarée** : elle referme un legs de G1 dans un fichier de ⑤, sous-projet **clos**.

### E2 — Le téléchargement est PORTABLE, là où la spec §6 le range en `#[cfg(windows)]`

La spec §6 écrit `src/installation.rs NEUF — #[cfg(windows)] : téléchargement,
exécution hors job (G3)`. **Le téléchargement n'a aucune raison d'être
Windows** : `tokio::net::TcpStream`, un analyseur d'en-têtes et une écriture de
fichier compilent et tournent sur l'hôte.

**Conséquence, et c'est une amélioration de couverture, pas un détail** : la
**troisième vérification d'empreinte**, la reprise par `Range`, le refus du
`chunked` et le refus de `https` se testent **tous sur l'hôte, contre un vrai
serveur local**, au lieu de dépendre d'une recette VM. **Seule l'exécution
reste `#[cfg(windows)]`.**

### E3 — Les deux modules purs du téléversement vont dans `proto/ts/`, non dans `client/src/hub/`

La spec §6 range `client/src/hub/tranches.ts`. **G3 les met dans `proto/ts/`**
(D6) : ce sont des règles **partagées** navigateur ↔ plateforme, et les y mettre
est ce qui empêche deux arithmétiques de diverger — **et** ce qui rend la
recette exécutable sans navigateur (D14). `client/src/hub/` ne garde que
l'orchestration.

### E4 — La spec D9 a trois issues ; G3 en a QUATRE

`reussie` / `sans_effet` / `issue_inconnue` ne couvrent pas les cas où
l'installation **n'a jamais démarré** — empreinte fausse, élévation requise,
extension refusée, job object (D10), disque plein. Les ranger dans
`issue_inconnue` ferait dire « on ne sait pas » **là où l'on sait très bien**.
**`refusee` est ajoutée, avec un `motif` typé.**

### E5 — Il n'y a pas de « fichier réassemblé », donc rien à en recalculer

La spec D7 écrit « la plateforme recalcule le SHA-256 du fichier **réassemblé** ».
**G3 ne réassemble jamais** (D7) : le scellement est une passe de flux sur les
tranches. **La vérification est identique** — même valeur, même refus —, et elle
économise le doublement du disque. C'est l'unique mot de la spec que ce plan
contredit, et il est contredit **par une équivalence**, pas par un abandon.

### E6 — `SEUIL_INJOIGNABLE_MS` n'est pas à la ligne que deux documents citent

La spec de ④ (§3.3) et `CLAUDE.md` citent `plateforme/src/agents/fraicheur.ts:37`.
**Relevé : ligne 47.** La valeur (`90_000`) est inchangée. C'est le naufrage du
« 487 » sous sa forme la plus bénigne, et il est **corrigé dans ce plan** plutôt
que recopié.

### E7 — 🔴 Le critère ⑦ de la spec est VACUEUX tant qu'on n'a pas relevé le job

La spec écrit, pour ⑦ : « assigner l'installeur au job object : il meurt avec
l'agent. **À exercer, et c'est la ROUGE du choix D8** ». Mais si le processus
qui lance n'est **dans aucun job** — ce que M5 rend probable pour le
superviseur —, alors le vert est **vrai par construction**. **La sonde U2 (tâche
2) et la trace produit de D10 sont ce qui rend ce critère décidable.** Sans
elles, ⑦ est un contrôle qui ne peut pas échouer, exactement comme F1 de D7.

### E8 — Une affirmation de code de G1, NON MESURÉE, que G3 ne doit pas prendre au mot

`agent/src/apps/lancement.rs` affirme : « `ShellExecuteEx` crée son processus
**hors de tout job**, et c'est ce qu'on veut ». ⚠️ **En général, un processus
créé par un processus assigné à un job est assigné au MÊME job** ; l'affirmation
n'est vraie ici que parce que **le superviseur ne s'assigne pas lui-même**
(M5) — c'est-à-dire pour une raison que la phrase ne donne pas, et qui cesserait
d'être vraie le jour où le lancement viendrait d'un enfant. **G3 ne s'appuie pas
dessus** : il **mesure** (U2) et **refuse** (D10). La phrase est signalée à la
revue transverse (tâche 35), qui décidera de l'annoter.

### E9 — La coordination avec G2 porte sur TROIS objets, pas un seul

| Objet | G2 (planifié, non implémenté) | G3 |
| --- | --- | --- |
| `PLATEFORME_VERSION` | écrit **3** en dur | **le suivant, relevé** (D1) |
| les deux dettes de `proto/` | les extrait, et **nomme** le point de chute | **même frontière, premier arrivé** (D3) |
| le numéro de migration | revendique `0005` | **le premier libre, relevé** (D2) |

⚠️ **Un quatrième objet est possible et n'appartient à personne** : le **lecteur
de jeton d'agent**, dont G2 a besoin pour `PUT /icone/:sha256` et G3 pour
`GET …/contenu`. **Le premier arrivé l'écrit, le second le réemploie** (D12).

### E10 — `application.vm_id` porte `REFERENCES vm(id)` SANS `ON DELETE`

Relevé dans `0003-agents.sql`, et les clés étrangères sont **appliquées des deux
côtés** (`pilote-sqlite.ts:54`). **Une application orpheline est donc insérable
nulle part**, et supprimer une VM qui porte des applications serait refusé.

**Ce que G3 en tire** : ses deux tables naissent **avec** leurs clés étrangères
(`televersement.utilisateur_id → utilisateur(id)`,
`installation.vm_id → vm(id)`, `installation.televersement_id →
televersement(id)`), **sans `ON DELETE`** elles aussi — c'est le leg n°2 de P1,
qu'`0004-applications.sql` rappelle : *une clé étrangère naît avec sa table ou
n'existe jamais*, SQLite ne sachant pas l'ajouter par `ALTER TABLE`.

⚠️ **Et toute colonne `NOT NULL` dont un sous-bloc ULTÉRIEUR aura besoin sur ces
deux tables doit naître MAINTENANT** : `ADD COLUMN … NOT NULL` sans défaut est
refusé dès que la table porte une ligne (divergence E8 de G1), et un `DEFAULT`
littéral est impossible (`rendreMarqueurs` refuse les apostrophes).

### E11 — La « rouge gratuite » du critère ① dépend de l'endroit d'où l'on tire

La spec écrit : « sur le binaire de G1 aucune route de téléversement n'existe et
le `POST` rend `404` ». **Vrai en accès direct au service** (le 404 générique de
`serveur.ts`). **Faux derrière le profil de déploiement**, où E1 fait rendre la
**page**. La recette tire **en direct**, donc la rouge est bien gratuite ; **le
dire évite qu'un successeur la joue derrière nginx et conclue à un défaut du
service.**

### E12 — La spec nomme deux fichiers de plateforme ; G3 en livre quatre

La spec §6 nomme `depot/televersement.ts` et `routes-televersement.ts`. G3 y
ajoute `depot/installation.ts` et `routes-installation.ts` : **sept routes dans
un seul fichier le porteraient au-delà du plafond dès sa naissance**, et la
règle des 500 lignes interdit qu'un fichier neuf naisse au-dessus. Deux
routeurs, donc **deux `it()`** dans `entetes-routeurs.test.ts` (tâche 29).

### E13 — Contrôle qui PASSE : aucun message de G3 ne traverse un `match` catch-all

🔴 `agent/src/capteur/pont_media.rs:93` porte un bras `Ok(autre)` — le
catch-all du `match serde_json::from_slice::<DepuisCapteur>` — qui
**tue le fil `lire_le_media` EN SILENCE**, et ce dépôt l'a payé **quatre fois**
(D5 `Sommeil`, D6 `Part`, D7 `Audio`, D8 `PleinEcran`). **Vérifié : les messages
de G3 voyagent sur le canal `/agent` (`proto::plateforme`), qui n'a aucun
rapport avec le protocole capteur↔enfant (`agent/src/capteur/protocole.rs`).**
Aucun bras à ajouter là-bas. ⚠️ **En revanche `agent/src/transport/controle.rs`
porte un `match` EXHAUSTIF** sur `AgentControl` — sans rapport avec ④ non plus.
**Contrôle joué, résultat négatif, écrit pour qu'on ne le rejoue pas.**

---

## Structure des fichiers

```
proto/
  src/plateforme.rs               MODIFIÉ — 5 variantes, 2 enums, PLATEFORME_VERSION (D1)
  src/plateforme/tests_apps.rs    MODIFIÉ ou NEUF selon D3
  ts/plateforme.ts                MODIFIÉ — le miroir
  ts/plateforme-apps.test.ts      MODIFIÉ ou NEUF selon D3
  plateforme-vectors.json         MODIFIÉ — un cas par variante, version des DEUX côtés
  ts/tranches.ts                  NEUF — PUR, partagé navigateur/plateforme (D6)
  ts/tranches.test.ts             NEUF
  ts/sha256.ts                    NEUF — PUR, incrémental (D5)
  ts/sha256.test.ts               NEUF — croisé contre `crypto.subtle.digest`

agent/
  src/plateforme.rs               MODIFIÉ — la seconde file (D11), APRÈS extraction
  src/plateforme/session.rs       NEUF — extraction PURE de forme (D11)
  src/apps/sha256.rs              MODIFIÉ — l'API incrémentale, sans dépendance
  src/apps.rs                     MODIFIÉ — `brancher` branche aussi l'installation (D10)
  src/apps/boucle.rs              MODIFIÉ — alimente la fenêtre de comptage (D9)
  src/installation.rs             NEUF — declare SANS cfg ; l'assemblage porte le cfg
  src/installation/verdict.rs     NEUF — PUR : (code, apparues, expiré) -> Issue
  src/installation/fenetre.rs     NEUF — PUR : la fenêtre de comptage (D9)
  src/installation/reponse.rs     NEUF — PUR : statut + en-têtes HTTP/1.1 (D13)
  src/installation/depot.rs       NEUF — PUR : chemins, marqueurs, extensions, purge d'âge
  src/installation/cadence.rs     NEUF — PUR : l'échantillonnage de la progression
  src/installation/telechargement.rs NEUF — PORTABLE (E2) : le GET, la reprise, l'empreinte
  src/installation/execution.rs   NEUF — #[cfg(windows)] : CreateProcessW, job, journaux
  Cargo.toml                      INCHANGÉ — à CONFIRMER par la compilation (D10)

plateforme/
  src/base/migrations/000X-televersement.sql  NEUF — numéro relevé (D2)
  src/base/pilotes.test.ts        MODIFIÉ — la liste exacte des migrations
  src/depot/televersement.ts      NEUF
  src/depot/installation.ts       NEUF
  src/apps/magasin-tranches.ts    NEUF — le magasin de disque, listage et flux (D7)
  src/config.ts                   MODIFIÉ — PLATEFORME_TELEVERSEMENTS
  src/http/porteur-agent.ts       NEUF — PUR (D12), ou RÉEMPLOYÉ si G2 l'a livré
  src/http/routes-televersement.ts NEUF — les quatre routes du navigateur
  src/http/routes-installation.ts NEUF — l'ordre, l'état, et le contenu servi à l'agent
  src/http/serveur.ts             MODIFIÉ — deux lignes de chaînage
  src/http/entetes-routeurs.test.ts MODIFIÉ — DEUX `it()` (E12)
  src/agents/canal.ts             MODIFIÉ — la réémission à l'enrôlement, APRÈS extraction
  src/agents/canal-apps.ts        NEUF — extraction PURE de forme (D11)

client/
  src/hub/televersement.ts        NEUF — orchestration, `fetch` et horloge en PARAMÈTRES
  src/hub/televersement.test.ts   NEUF — sur l'hôte, sans DOM

deploiement/
  nginx.conf                      MODIFIÉ — les chemins de G3, et les DEUX de G1 (E1)
```

🔴 **`agent/src/installation.rs` est déclaré SANS `cfg`, et il est déclaré
depuis `apps.rs` — jamais depuis `main.rs`.** C'est la conséquence directe de
D10 : `apps::brancher` est le seul site d'appel, et `agent/src/main.rs` ne
bouge pas, ce qui tient l'engagement de périmètre pris envers quatre chantiers
concurrents. **Ses enfants purs existent donc sur l'hôte**, où leurs tests
courent, et **seul `execution.rs` porte le `#[cfg(windows)]`**.

C'est exactement la figure qu'`apps.rs` documente déjà en tête, et le texte est
cité verbatim : « la "Convention de module enfant" de `CLAUDE.md` n'est donc pas
mobilisée : aucun module ne franchit ici de frontière `#[cfg(windows)]` ».

---

## Interfaces partagées

**Les messages du canal** (`proto/src/plateforme.rs`, et son miroir) :

```rust
// Descendante.
Installer {
    v, installation: String, url: String, nom: String,
    taille: u64, sha256: String,
}

// Montantes.
Progression { v, installation: String, phase: Phase, octets_faits: u64, octets_total: u64, ecoule_ms: u64 }
Termine     { v, installation: String, issue: Issue, motif: Option<String>,
              code_sortie: Option<i32>, journal: String, journal_tronque: bool }

enum Phase { Transfert, Execution, Reconciliation }
enum Issue { Reussie, SansEffet, IssueInconnue, Refusee }
```

⚠️ **`Phase` et `Issue` ont chacune AU MOINS UNE VARIANTE DE DEUX MOTS**
(`SansEffet`, `IssueInconnue`), et c'est **délibéré** : G1 a mesuré qu'un
`rename_all` est **inobservable** sur un enum dont toutes les variantes sont
d'un seul mot — passer `kebab-case` à `snake_case` sur `IssueLancement` laisse
`cargo test -p proto` à **75 passed**. **Ces deux enums referment la lacune
d'eux-mêmes** (leg n°9 de G1), et le test des vecteurs le prouve.

⚠️ **`code_sortie` est `Option<i32>`, jamais `i32` avec un `-1` sentinelle** :
« pas de code » et « code −1 » sont deux faits différents, et une sentinelle les
confondrait exactement comme un `source_max_px` à `0` confondrait « inconnu » et
« nul » (spec D5).

**Les routes** : voir la table de D4. Toutes rendent
`{ refus: '<motif>' }` en cas d'échec, posent `ENTETES_SECURITE` **avant**
`cors` sur **toute** réponse y compris les refus, et servent `OPTIONS` — leurs
requêtes portent `Authorization`, donc elles ne sont **pas simples**, et un 404
sur l'`OPTIONS` ferait abandonner le navigateur **avant** la vraie requête.

**Le module pur partagé** (`proto/ts/tranches.ts`) :

```ts
export function plan(taille: number, tailleTranche: number): { n: number; octets: number }[];
export function verdict(
    taille: number, tailleTranche: number,
    presentes: { n: number; octets: number }[],
): { etat: 'complet' } | { etat: 'manquantes'; n: number[] } | { etat: 'incoherentes'; n: number[] };
```

**L'empreinte incrémentale** (`proto/ts/sha256.ts`) :

```ts
export class Sha256 { absorber(bloc: Uint8Array): void; terminer(): string; }
```

et son jumeau Rust, **ajouté à `agent/src/apps/sha256.rs` sans rien y casser** :

```rust
pub struct Condensateur { /* … */ }
impl Condensateur {
    pub fn neuf() -> Self;
    pub fn absorber(&mut self, bloc: &[u8]);
    pub fn terminer(self) -> [u8; 32];
}
// `condenser` (existante) devient un appel de commodité sur ce type.
```

---

## Ordre et parallélisme

| Famille | Tâches | Dépend de | Parallélisable | Paquets touchés |
| --- | --- | --- | --- | --- |
| U — la porte | 1, 2 | rien | **entre elles seulement** (une seule exécution VM) | 🔴 **la VM** |
| 0 — protocole et contrat partagé | 3, 4, 5, 6, 7, 8, 9 | 3 ← rien ; 4 ← rien ; 5 ← 3 ; 6 ← 4, 5 ; 7 ← 6 ; 8 ← rien ; 9 ← rien | **3, 4, 8 et 9 avec tout** | 🔴 `proto/` |
| 1 — l'agent, PUR | 10, 11, 12, 13, 14, 15 | toutes ← rien | **toutes entre elles** | 🔴 `agent/` |
| 2 — l'agent, portable | 16 | 10, 13, 14 | — | 🔴 `agent/` |
| 3 — l'agent, Windows | 17, 18, 19, 20, 21, 22 | 17 ← rien ; 18 ← 5, 17 ; 19 ← 1, 2, 14 ; 20 ← rien ; 21 ← 11, 12, 15, 16, 18, 19, 20 ; 22 ← rien | **17, 20 et 22 avec tout** | 🔴 `agent/`, `scripts/` |
| 4 — la plateforme, persistance | 23, 24, 25 | 23 ← rien ; 24 ← 23 ; 25 ← 8 | 23 et 25 avec tout | `plateforme/` |
| 5 — la plateforme, HTTP et canal | 26, 27, 28, 29, 30, 31, 32 | 26 ← rien ; 27 ← 24, 25, 26, 8, 9 ; 28 ← 24, 25, 26, 6 ; 29 ← 27, 28 ; 30 ← rien ; 31 ← 6, 24, 30 ; 32 ← rien | 26, 30 et 32 avec tout | `plateforme/`, `deploiement/` |
| 6 — le navigateur | 33 | 8, 9 | — | `client/` |
| 7 — recette et clôture | 34, 35, 36, 37 | 34 ← tout ; 35, 36, 37 ← 34 | 35 et 36 entre elles | — |

**Chemin critique** : **1 → 19 → 21 → 34.** ⚠️ **La porte est en tête du chemin
critique, et ce n'est pas une commodité de rédaction** : si son verdict est
défavorable, **la tâche 19 change** (le refus `elevation-requise` devient le
chemin nominal d'une famille entière d'installeurs) et **le §9 du document de
résultats aussi**. Un second brin de même poids passe par le protocole :
3 → 5 → 6 → 7 → 18 → 21.

🔴 **AUCUNE tâche de développement ne commence avant que la tâche 1 n'ait rendu
son verdict**, à deux exceptions près, et elles sont nommées : les familles 0 et
1 (protocole et modules purs) sont **indifférentes au verdict** — ni le
découpage en tranches, ni l'empreinte, ni le format des messages ne changent
selon que la boîte de dialogue est capturée ou non. **Elles peuvent donc courir
pendant que la VM est occupée**, ce qui est exactement la situation du
20 août 2026.

🔴 **Les tâches 3, 4, 5, 6 et 7 ne se parallélisent avec RIEN dans `proto/`** :
elles portent le bump de version, et le vert ne se lit qu'à la fin de la 7. Les
extractions **3 et 4 précèdent** la 5 et la 6 sans exception, et **n'ajoutent
aucune ligne de comportement**.

🔴 **Les tâches 17 et 30 sont des EXTRACTIONS, et elles précèdent leurs
additions** — la 18 et la 31 respectivement. Même règle, même interdit de
compression, et **aucune ligne de comportement** dans l'une ni dans l'autre.

🔴 **La tâche 22 (`scripts/run-agent.sh`) est dédiée et ne fait rien d'autre.**

⚠️ **Trois tâches emploient la VM** — 1 et 2 (une seule exécution, deux sondes),
19 (la seule compilation distante de développement) et 34 (la recette). **Toutes
vérifient `Get-Process agent` avant et après, y compris après une tentative
échouée** (piège de D8, payé trois fois sur trois : un superviseur resté vivant
empêche le nouveau journal de s'ouvrir, et l'on relit **le run d'avant en
croyant lire le sien**).

---

# Famille U — la porte, avant toute ligne de produit

### Task 1 : 🔴 LA PORTE — l'élévation UAC est-elle capturée ?

- [ ] **Relever d'abord la configuration**, et la verser telle quelle :
      `EnableLUA`, `ConsentPromptBehaviorAdmin` et `PromptOnSecureDesktop`
      (`HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System`), plus
      `whoami /groups` pour savoir si le compte interactif est administrateur.
      **Aucun verdict n'est prononçable sans ces quatre valeurs.**
- [ ] Lancer l'agent en mode `SUPERVISEUR`, page-shell ouverte **depuis l'hôte**
      (⚠️ **jamais depuis la VM** : la fenêtre de la page-shell y serait
      elle-même capturée, ce qui boucle en cascade d'ouvertures — piège de D8).
- [ ] **(a) LE TÉMOIN POSITIF, d'abord.** Ouvrir une fenêtre **ordinaire, non
      élevée**, au contenu reconnaissable, et vérifier **sur le même chemin de
      capture** qu'elle est vue : une session naît, `framesDecoded` croît.
      🔴 **Si le témoin n'est pas vu, le run est DISQUALIFIÉ** — verdict
      `NON MESURABLE — l'instrument n'a rien vu du tout`, et on répare
      l'instrument avant de recommencer.
- [ ] **(b) LA CHOSE A EU LIEU.** Déclencher une élévation par
      `Start-Process -Verb RunAs` sur un exécutable anodin, et **échantillonner
      `Get-Process consent` à 1 Hz** pendant toute la fenêtre d'observation.
      🔴 **Si `consent.exe` n'apparaît jamais, le run est DISQUALIFIÉ** —
      verdict `NON MESURABLE — aucune élévation n'a eu lieu`, accompagné des
      quatre valeurs de configuration. **On n'invente pas un second indice pour
      sauver un verdict.**
- [ ] **(c) LA MESURE.** Pendant que `consent.exe` vit : le superviseur
      journalise-t-il une fenêtre pour lui (`fenêtre détectée`, `enfant lancé`) ?
      Une session diffuse-t-elle une image **non vide** ? Relever les deux, et
      **verser une capture de la fenêtre navigateur correspondante**.
- [ ] **(d) L'OBSERVATION ANNEXE.** Appeler `OpenInputDesktop` depuis un
      processus ordinaire **pendant** que `consent.exe` vit, puis **en dehors**,
      et noter si les deux diffèrent. 🔵 **Si oui, D15 gagne son détecteur ; si
      non, c'est écrit et le produit retombe sur l'expiration.**
- [ ] **(e) LA PRÉMISSE DE D15, ÉPROUVÉE AU PASSAGE.** Sur le **même**
      exécutable élévateur, appeler `CreateProcess` (et non `ShellExecute`) et
      **relever le code d'erreur exact**. 🔴 **D15 tout entier repose sur
      `ERROR_ELEVATION_REQUIRED` (740), qui est une lecture de documentation et
      non une mesure de ce dépôt** : si l'appel réussit, ou échoue autrement, le
      refus typé de la tâche 19 doit être reconstruit sur un autre indice.
      **Cette observation ne disqualifie rien** — elle informe une décision de
      conception.
- [ ] Écrire le verdict **selon la table de la section « LA PORTE »**, et lui
      seul : `CAPTURÉE`, `CAPTURÉE mais PromptOnSecureDesktop=0`,
      `NON CAPTURÉE`, ou `NON MESURABLE`.

**Test :** aucun test automatisé — c'est une sonde. Son résultat est **versé
dans `docs/superpowers/plans/journaux-gestion-apps-g3/`**, journaux bruts **et**
`-plat` jumeaux.
**Ce qui la rend ROUGE :** ⚠️ **une sonde n'a pas de rouge, elle a des
DISQUALIFICATIONS** — les deux ci-dessus, (a) et (b), et elles sont exactement
ce qui distingue cette sonde de celle qui a rendu un faux verdict éliminatoire
aujourd'hui même. **Le contrôle qui vaut est : ai-je vu (a) POSITIF et (b)
POSITIF avant de prononcer quoi que ce soit ?**
**Nombre d'exécutions : DEUX**, comme partout ailleurs. Deux disqualifications
successives pour la même cause valent un verdict `NON MESURABLE` **définitif**
pour ce sous-bloc, et le §9 du document de résultats le dit.

### Task 2 : 🔴 La sonde du JOB OBJECT — sans elle, le critère ⑦ ne peut pas échouer

- [ ] Dans la **même exécution VM** que la tâche 1, relever `IsProcessInJob`
      pour **le processus superviseur** et pour **le pont**, s'il tourne. Le
      plus simple est un `.ps1` déposé sur `/media/vm/dev/` et invoqué par
      `powershell -ExecutionPolicy Bypass -File C:\dev\<nom>.ps1` (⚠️ **jamais
      en ligne de commande** : `nodejs-winrm` enveloppe dans
      `powershell -Command "& { … }"` et un guillemet interne y entre en
      collision — piège de D3).
- [ ] Relever si un processus **créé par** le superviseur hérite d'un job.
- [ ] Verser le relevé, et **en tirer la formulation du critère ⑦** : si le
      superviseur n'est dans aucun job, écrire noir sur blanc que **le vert de
      ⑦ est vrai par construction sur ce chemin**, et que la seule rouge
      disponible est la **mutation** de la tâche 34 (assigner délibérément
      l'installeur au job et le voir mourir).

**Test :** aucun — sonde.
**Ce qu'elle DISQUALIFIE :** rien ; elle ne peut pas mentir, elle rend un
booléen par processus. ⚠️ **Ce qu'elle empêche, en revanche, c'est qu'on
prononce ⑦ sans savoir ce qu'il vaut** — E7.

---

# Famille 0 — le protocole, et le contrat partagé

### Task 3 : EXTRACTION de `proto/src/plateforme/tests.rs`, ou CONSTAT

- [ ] `wc -l proto/src/plateforme/tests.rs`. **Si le fichier est sous 500 et que
      `tests_apps.rs` existe, G2 est passé : CONSTATER, verser le relevé, et ne
      rien découper** (D3).
- [ ] Sinon, découper **selon la frontière que G2 a nommée** : le cycle de vie
      reste (version, refus, enrôlement, battement), la gestion d'apps part vers
      `proto/src/plateforme/tests_apps.rs`. Déclaration **chez le parent** :
      `#[cfg(test)] #[path = "plateforme/tests_apps.rs"] mod tests_apps;`.
- [ ] **Transposition VERBATIM. Aucune ligne de comportement.**

**Test :** `cargo test -p proto`.
**Ce qui le rend ROUGE :** un test perdu au découpage. 🔴 **Le contrôle est le
COMPTE, ANNONCÉ AVANT D'ÊTRE MESURÉ** — relevé du 20 août 2026 : **83 passed**
(plus une ligne de doc-tests à 0). *(D10 : un implémenteur a écrasé un fichier
de tests et supprimé un test antérieur ; il l'a vu parce que le compte est sorti
à 452 au lieu des 453 annoncés d'avance.)*
**Contrôle de taille :** `wc -l` sur les **deux** fichiers, aucun au-dessus de
500. ⚠️ **Ils sont dans la table de dette de `CLAUDE.md` : la tâche 36 doit les
en RETIRER si G3 est celui qui les purge**, sans quoi la dette resterait écrite
après avoir été payée.

### Task 4 : EXTRACTION de `proto/ts/plateforme.test.ts`, ou CONSTAT

- [ ] Même règle, même frontière, même verbatim. Vitest découvre les
      `*.test.ts` : aucune déclaration à ajouter.

**Test :** `cd proto && npm test`.
**Ce qui le rend ROUGE :** le compte annoncé d'avance. Relevé du 20 août 2026 :
**5 fichiers, 142 tests**. ⚠️ **Dire lequel on annonce** — piège des « dix » et
« dix-sept » de P3.

### Task 5 : les cinq variantes, les deux enums, et `PLATEFORME_VERSION` (Rust)

- [ ] 🔴 **RELEVER la version courante des trois lieux, puis poser la
      suivante** (D1). **Ne pas écrire un nombre lu dans un plan.**
- [ ] `DepuisLaPlateforme::Installer { version, installation, url, nom, taille, sha256 }`.
- [ ] `VersLaPlateforme::Progression { … }` et `::Termine { … }`, avec `Phase`
      et `Issue` — **au moins une variante de deux mots dans chacun** (leg n°9
      de G1, voir « Interfaces partagées »).
- [ ] `code_sortie: Option<i32>`, jamais de sentinelle.
- [ ] Constructeurs `pub fn` pour chaque variante, comme les huit existants.
- [ ] Les cas de test vont dans le fichier **d'apps** (tâche 3).

**Test :** `cargo test -p proto`.
**Ce qui le rend ROUGE — quatre mutations, jouées APRÈS le vert :**
1. rendre `kebab-case` en `snake_case` sur `Phase` : `sans-effet` devient
   `sans_effet` et les vecteurs tombent. 🔴 **C'est la rouge qui prouve que la
   lacune de G1 est refermée** ; si elle survit, **une variante de deux mots
   manque** et il faut la relire, pas la contourner ;
2. `code_sortie: i32` avec `-1` : le test qui distingue « pas de code » de
   « code −1 » tombe ;
3. oublier le bump : le test de version tombe (tâche 7) ;
4. retirer `deny_unknown_fields` : le test du champ de trop tombe.

### Task 6 : le miroir TypeScript

- [ ] `proto/ts/plateforme.ts` : les mêmes variantes, les mêmes enums, la même
      constante, les encodeurs et le parseur.
- [ ] ⚠️ **Le parseur descendant est celui de l'AGENT et le montant celui de la
      PLATEFORME** : vérifier lequel gagne quelle branche, et ne pas en oublier
      une — un message non reconnu doit rendre `{ ok: false, motif: 'forme' }`,
      **jamais** être ignoré.

**Test :** `cd proto && npm test`, `npm run typecheck`.
**Ce qui le rend ROUGE :** une variante ajoutée d'un seul côté — la conformité
aux vecteurs (tâche 7) tombe alors du côté manquant.

### Task 7 : `proto/plateforme-vectors.json` — la version, des DEUX côtés

- [ ] Un cas par variante neuve, `sens` correct.
- [ ] 🔴 **Reprendre `"version"` (l. 3) ET CHAQUE chaîne `json` du fichier**,
      qui porte `"v":2` **en dur** — 189 lignes relevées.
- [ ] La clé `version` est **vérifiée des deux côtés** : c'est la lacune de
      `vectors.json` que P3 a corrigée pour ce fichier-ci, **et il ne faut pas
      la rouvrir**.

**Test :** `cargo test -p proto` **et** `cd proto && npm test`.
**Ce qui le rend ROUGE :** un `"v"` oublié dans une seule chaîne `json` — le
round-trip du cas concerné tombe **d'un seul côté**, ce qui est exactement le
symptôme qu'on veut voir.

### Task 8 : `proto/ts/tranches.ts` — la règle de découpage, PURE et PARTAGÉE

- [ ] `plan(taille, tailleTranche)` et `verdict(taille, tailleTranche, presentes)`
      (voir « Interfaces partagées »).
- [ ] 🔴 **`incoherentes` est distinct de `manquantes`** (D6) : une tranche
      présente à la mauvaise taille est une erreur de protocole, **pas un trou à
      recompléter**.
- [ ] Aucun import de `node:`, aucun DOM. **Ce module doit charger dans les deux
      environnements.**

**Test :** `proto/ts/tranches.test.ts`, `cd proto && npm test`.
**Ce qui le rend ROUGE :** les cas limites, tous à écrire — `taille = 0`
(zéro tranche, et **le dire** plutôt que rendre une tranche vide),
`taille` multiple exact de `tailleTranche` (⚠️ **pas de tranche finale de zéro
octet**), `taille < tailleTranche`, une tranche `n` hors bornes, une tranche
présente **plus grande** que prévue. **Mutation : remplacer `Math.ceil` par
`Math.floor`** — la dernière tranche disparaît et le verdict devient `complet`
sur un fichier **tronqué**. C'est la rouge qui compte.

### Task 9 : `proto/ts/sha256.ts` — l'empreinte incrémentale, croisée contre l'API du navigateur

- [ ] `class Sha256 { absorber(bloc); terminer(): string }`, sans dépendance.
- [ ] Les vecteurs de réponse connue de FIPS 180-4 : chaîne vide, `abc`, et le
      message de 448 bits qui force un second bloc — **les trois, comme
      `agent/src/apps/sha256.rs` les porte déjà**.
- [ ] 🔴 **Le contrôle qui vaut : comparer notre résultat à
      `crypto.subtle.digest('SHA-256', …)`** sur une dizaine d'entrées de
      tailles variées, **absorbées en blocs de tailles IRRÉGULIÈRES** (1, 63,
      64, 65, 1000 octets) — c'est le découpage irrégulier qui éprouve la
      gestion du tampon résiduel, et c'est là que vivent les fautes.

**Test :** `proto/ts/sha256.test.ts`, `cd proto && npm test`.
**Ce qui le rend ROUGE :** casser une constante de `K`, ou une rotation, ou le
remplissage final — **la comparaison à `crypto.subtle` tombe immédiatement**.
⚠️ **Cet état est atteignable et doit être JOUÉ** : c'est le seul test de tout
le sous-bloc dont l'oracle est **indépendant de notre propre code**.

---

# Famille 1 — l'agent, PUR

### Task 10 : `agent/src/apps/sha256.rs` — l'API incrémentale, sans dépendance

- [ ] Ajouter `Condensateur` (`neuf` / `absorber` / `terminer`) **au fichier
      existant** (168 lignes, marge confortable) ; `condenser` devient un appel
      de commodité dessus.
- [ ] ⚠️ **Ne pas toucher l'en-tête du module** : il explique pourquoi ce
      SHA-256 est écrit ici plutôt qu'emprunté, et **G3 reconduit exactement le
      même invariant** — aucune dépendance de production.
- [ ] Les trois vecteurs FIPS existants doivent passer **par les deux voies**,
      ce qui prouve que la commodité et l'incrémental s'accordent.

**Test :** `cargo test -p agent apps::sha256`.
**Ce qui le rend ROUGE :** absorber en blocs de 1 octet un message de 448 bits
et obtenir autre chose que le vecteur FIPS — c'est-à-dire un tampon résiduel
mal recopié. **Le test absorbe en tailles irrégulières**, exactement comme la
tâche 9.
**Contrôle :** `wc -l` — le fichier reste très en dessous de 500.

### Task 11 : `installation/verdict.rs` — l'issue, PURE

- [ ] `Issue::depuis(code_sortie: Option<i32>, apparues: usize, expire: bool, refus: Option<Motif>) -> Issue`.
- [ ] 🔴 **Le code de sortie n'est JAMAIS interprété** (spec D9, D9 de ce plan) :
      il est **rapporté**. Le seul rôle de `Option` est de distinguer
      « recueilli » de « perdu ».
- [ ] `refusee` l'emporte sur tout le reste ; `issue_inconnue` sur l'absence de
      code ou l'expiration ; puis `reussie` si `apparues > 0`, `sans_effet`
      sinon.

**Test :** `cargo test -p agent installation::verdict`.
**Ce qui le rend ROUGE :** 🔴 **le cas `3010`** — `msiexec` rend 3010 pour un
succès qui demande un redémarrage. Un code qui déciderait de l'issue rendrait
`sans_effet` (ou pire, un échec) sur `3010` avec `apparues = 2`. **Le test pose
`(Some(3010), 2)` et exige `reussie`**, et `(Some(0), 0)` et exige
`sans_effet` — ce dernier est **le témoin de la spec ⑥**, celui qu'un installeur
annulé produit.

### Task 12 : `installation/fenetre.rs` — la fenêtre de comptage, PURE

- [ ] `Fenetres` : `ouvrir(id)`, `ajouter(apparues: usize)` — qui ajoute à
      **toutes** les fenêtres ouvertes —, `fermer(id) -> usize`.
- [ ] ⚠️ **Plusieurs fenêtres peuvent être ouvertes en même temps** : deux
      installations concurrentes sont possibles, et **chacune doit compter ce
      qui est apparu pendant SA fenêtre**, quitte à ce que les deux comptent la
      même application. **Attribuer une apparition à une seule installation
      serait deviner.**

**Test :** `cargo test -p agent installation::fenetre`.
**Ce qui le rend ROUGE :** 🔴 **le scénario que D9 décrit** — ouvrir, ajouter 3,
ajouter 0, fermer : le total doit valoir **3**. Une implémentation qui ne
retiendrait que le **dernier** ajout rendrait `0` et produirait le faux
`sans_effet` que cette décision existe pour empêcher. **Cet état est
atteignable, et c'est la rouge à jouer.**

### Task 13 : `installation/reponse.rs` — la lecture d'une réponse HTTP/1.1, PURE

- [ ] Analyser la ligne de statut et les en-têtes depuis un `&[u8]` : rendre le
      statut, `Content-Length`, et **détecter `Transfer-Encoding`**.
- [ ] Noms d'en-têtes **insensibles à la casse** (la RFC l'impose), valeurs
      rognées.
- [ ] 🔴 **Refus TYPÉS, jamais d'interprétation** (D13) : `chunked` → refus
      nommé ; absence de `Content-Length` → refus nommé ; statut hors 200/206 →
      refus portant le statut.
- [ ] Rendre aussi la position du premier octet de corps — l'appelant en a
      besoin, l'en-tête et le corps arrivant dans le même tampon.

**Test :** `cargo test -p agent installation::reponse`.
**Ce qui le rend ROUGE :** un en-tête **coupé en deux lectures** (le tampon
s'arrête au milieu de `Content-Len|gth: 42`) — l'analyseur doit dire « pas
encore complet », **jamais** conclure. **Mutation : chercher `\r\n\r\n` sans
vérifier qu'il est présent** → l'analyseur conclut sur un en-tête partiel.

### Task 14 : `installation/depot.rs` — chemins, marqueurs, extensions, purge, PURS

- [ ] Le chemin d'atterrissage :
      `%ProgramData%\Guacamole\installeurs\<installation-id>\<nom>` (spec D7) —
      **pas `%TEMP%`** (Windows le purge, y compris pendant une installation),
      **pas le profil utilisateur** (un installeur élevé peut ne pas le voir).
- [ ] 🔴 **Le nom du fichier est ASSAINI, et la règle est ici** : le `nom` vient
      du navigateur. Rejeter tout séparateur, tout `..`, tout caractère interdit
      par Windows, et **borner la longueur**. ⚠️ **`..` est significatif sous
      Windows** — une correction de D3 l'avait introduit puis rattrapé.
- [ ] La règle d'extension : `.exe` et `.msi` acceptés, `.bat` **refusé avec son
      motif** (spec D8), tout le reste refusé.
- [ ] Les marqueurs de D17 : lire l'état d'un répertoire → `Neuf`, `Commence`,
      `Termine`.
- [ ] La règle de purge d'âge : depuis une liste `(id, age_ms)`, rendre ce qui
      est à supprimer au-delà d'`EXPIRATION_INSTALLEUR`.

**Test :** `cargo test -p agent installation::depot`.
**Ce qui le rend ROUGE :** 🔴 `nom = "..\\..\\Windows\\System32\\evil.exe"` doit
être **refusé**, pas assaini en silence — le test compare le refus **et son
motif**. Et `nom = "setup.bat"` doit rendre le refus d'extension, **pas** un
refus de nom : deux motifs distincts, deux assertions.

### Task 15 : `installation/cadence.rs` — l'échantillonnage de la progression, PUR

- [ ] `doit_emettre(dernier_ms, maintenant_ms) -> bool`, **horloge en
      paramètre** — comme `agents/fraicheur.ts` et `identite/jeton.ts` le font
      déjà côté plateforme.
- [ ] `PERIODE_PROGRESSION = 1 s`, **NON CALIBRÉE** et déclarée telle.
- [ ] 🔴 **La dernière progression d'une phase est TOUJOURS émise**, quel que
      soit le cadencement : sans cette clause, une barre s'arrêterait à 97 %
      pour l'éternité.

**Test :** `cargo test -p agent installation::cadence`.
**Ce qui le rend ROUGE :** cent appels dans la même milliseconde ne doivent
produire **qu'une** émission ; et la clause « dernière toujours émise » se
teste séparément, sinon elle serait vraie par hasard.

---

# Famille 2 — l'agent, portable

### Task 16 : `installation/telechargement.rs` — le GET, la reprise, l'empreinte

- [ ] `tokio::net::TcpStream`, requête `GET` avec `Host`, `Authorization: Bearer`,
      `Accept-Encoding: identity` et, en reprise, `Range: bytes=<n>-`.
- [ ] Écriture **en flux** vers le fichier, empreinte calculée **pendant**
      l'écriture (D13), progression cadencée (tâche 15).
- [ ] La reprise : `RETABLISSEMENTS_MAX = 5` (NON CALIBRÉE) ; un `200` en
      réponse à un `Range` fait **repartir de zéro**, jamais concaténer ; sur
      reprise, **relire ce qui est déjà écrit** pour réamorcer la condensation.
- [ ] Les trois refus typés de D13 (`https`, `chunked`, statut inattendu).
- [ ] ⚠️ **Aucun `#[cfg]`** : ce module compile et se teste sur l'hôte (E2).

**Test :** `cargo test -p agent installation::telechargement`, **contre un vrai
serveur TCP local** monté dans le test (`tokio::net::TcpListener`), pas contre
un double.
**Ce qui le rend ROUGE — cinq scénarios, tous atteignables sur l'hôte :**
1. le serveur ferme **au milieu** du corps → la reprise repart au bon offset et
   l'empreinte finale est **juste** ; ⚠️ **et le test vérifie l'OFFSET demandé**,
   pas seulement le succès : une reprise qui redemanderait tout depuis zéro
   passerait un test qui ne regarde que le résultat (c'est la leçon du critère
   ③ de la spec) ;
2. le serveur répond `200` à un `Range` → le fichier est **retéléchargé**, pas
   concaténé ; le test compare la **taille** ;
3. le serveur répond `Transfer-Encoding: chunked` → **refus typé**, et le motif
   nomme `chunked` ;
4. le serveur répond `500` → refus typé portant le statut ;
5. le serveur envoie un corps dont l'empreinte diffère → **refus `empreinte`**,
   et **le fichier partiel est supprimé** (spec §7 : « le fichier partiel est
   supprimé ; le téléversement reste reprenable »).

---

# Famille 3 — l'agent, Windows

### Task 17 : 🔴 EXTRACTION de `agent/src/plateforme.rs`, AVANT toute addition

- [ ] `agent/src/plateforme.rs` est à **453** lignes (relevé), marge **47**.
      Extraire `une_session`, `connecter` et `sur_refus` vers
      `agent/src/plateforme/session.rs`, déclaré par un `mod session;` ordinaire
      — ⚠️ **ce fichier n'est pas `#[cfg(windows)]`**, la « Convention de module
      enfant » ne s'applique donc pas.
- [ ] **Transposition VERBATIM.** Les visibilités passent à `pub(super)` là où
      il le faut, **et nulle part ailleurs**.

**Test :** `cargo test -p agent` **et**
`cargo check --target x86_64-pc-windows-gnu`.
**Ce qui le rend ROUGE :** le compte de tests, annoncé avant d'être mesuré.
**Contrôle de taille :** `wc -l` sur les deux fichiers, et **la marge de
`plateforme.rs` est reportée dans le rapport** — c'est elle que la tâche 18 va
consommer.

### Task 18 : le canal de l'agent — la seconde file, et les deux montantes

- [ ] `Canal::installations()` : une file `mpsc::UnboundedReceiver<Installation>`,
      prise **une seule fois** comme `ordres()` (D11) ; la boucle de session
      aiguille `Installer` vers elle.
- [ ] Les deux montantes passent par l'`Emetteur` existant.
- [ ] ⚠️ **La file montante est bornée à 32 et journalise ses abandons** : c'est
      la raison d'être du cadencement de la tâche 15, et le commentaire doit le
      dire **au point d'émission**, pas seulement dans `cadence.rs`.

**Test :** `cargo test -p agent plateforme`, plus
`cargo check --target x86_64-pc-windows-gnu`.
**Ce qui le rend ROUGE :** appeler `installations()` deux fois doit rendre
`None` au second appel — **la même propriété qu'`ordres()`, et pour la même
raison** : deux consommateurs se voleraient les ordres, et le symptôme serait
« une installation sur deux ne part pas ».

### Task 19 : `installation/execution.rs` — `CreateProcessW` hors job, et le refus d'élévation

- [ ] `CreateProcessW` — **jamais `ShellExecuteExW`** (D15) —, `.msi` via
      `msiexec /i`, stdout et stderr redirigés vers un fichier du répertoire
      d'installation.
- [ ] 🔴 `ERROR_ELEVATION_REQUIRED` (740) → `Issue::Refusee`,
      `motif = elevation-requise`. **C'est le remède qui rend G3 livrable même
      si la porte est défavorable.**
- [ ] 🔴 **Garde de job (D10)** : `IsProcessInJob(GetCurrentProcess())` avant
      tout lancement ; **si le processus est dans un job, REFUSER** avec son
      motif. Journaliser le booléen **à chaque installation**, que le refus ait
      lieu ou non — c'est cette ligne qui rend le critère ⑦ décidable (E7).
- [ ] Attente **non bloquante** : `WaitForSingleObject(handle, 0)` scruté, et
      `EXPIRATION_EXECUTION = 2 h` au terme de laquelle l'agent **cesse
      d'attendre sans TUER** (spec §7 : « tuer un installeur au milieu est
      pire »).
- [ ] La queue du journal, bornée à `JOURNAL_MAX_OCTETS` (64 Kio), avec son
      drapeau `journal_tronque` ; **un journal vide est le cas NORMAL** et le
      dit (D9).
- [ ] ⚠️ **Aucune addition à `agent/Cargo.toml` attendue**
      (`Win32_System_JobObjects` et `Win32_System_Threading` sont déjà employées
      par `lanceur.rs`) — 🔴 **à CONFIRMER par la compilation, pas à affirmer.**

**Test :** `cargo check --target x86_64-pc-windows-gnu` **seul** — aucun test
d'hôte ne peut couvrir ce module, comme `apps::lecture` et `apps::lancement`.
**Ce qui le rend ROUGE :** ⚠️ **rien, sur l'hôte, et il faut le dire.** Sa seule
épreuve est la recette (tâche 34), critères ⑥ et ⑦. **C'est pourquoi tout ce qui
pouvait en sortir en est sorti** — le verdict, la fenêtre, les chemins, les
extensions, la cadence sont **cinq modules purs**, et c'est là qu'est la
couverture.

### Task 20 : le périphérique de rendu par défaut, tracé AVANT et APRÈS

- [ ] Journaliser le nom du point de terminaison de rendu par défaut
      **immédiatement avant** le lancement et **immédiatement après** la sortie,
      en réemployant la règle existante (`agent/src/wasapi/peripherique.rs`).
- [ ] ⚠️ **Aucune restauration, aucun garde-fou** (D19) : remettre d'autorité le
      périphérique d'avant serait décider à la place de l'utilisateur qui vient
      d'installer un périphérique audio exprès.
- [ ] Le champ est nommé pour être `grep`able, et **les deux lignes portent le
      même nom de champ** avec un `moment=avant|apres`.

**Test :** `cargo check --target x86_64-pc-windows-gnu` ; la partie **pure** de
la lecture de périphérique est déjà testée par le chantier A.
**Ce qui le rend ROUGE :** critère ⑧ de la recette — **ne tracer qu'après**
rendrait un changement inattribuable, et c'est exactement ce qui a coûté une
campagne le 19 août 2026 avec VB-Cable.

### Task 21 : le câblage — `apps::brancher` branche aussi l'installation

- [ ] `apps::brancher` rend désormais **deux** poignées (une structure), et
      **`agent/src/main.rs` NE BOUGE PAS** : la liaison `let _apps = …` (l. 275)
      compile telle quelle (D10, périmètre concurrent).
- [ ] La boucle de découverte alimente `Fenetres::ajouter(diff.apparues.len())`
      à chaque réconciliation, et expose « réconcilie maintenant » — **c'est le
      seul point de contact entre `apps` et `installation`.**
- [ ] Le fil d'installation : télécharger (tokio), exécuter
      (`spawn_blocking`), fermer la fenêtre, émettre `Termine`.
- [ ] 🔴 **`APPS=0` désarme aussi l'installation**, et une ligne de journal le
      dit (D16).

**Test :** `cargo test -p agent`, `cargo check --target x86_64-pc-windows-gnu`,
`cargo clippy --workspace`.
**Ce qui le rend ROUGE :** ⚠️ **le câblage est le seul endroit qu'aucun test
d'hôte ne couvre**, et c'est reconnu ici plutôt que masqué : sa rouge est la
recette. **Le contrôle disponible tout de suite** est que `git diff --stat`
**ne montre AUCUNE ligne de `agent/src/main.rs`** — une affirmation de périmètre
qui se vérifie par la commande.

### Task 22 : 🔴 TÂCHE DÉDIÉE — `scripts/run-agent.sh` transmet `INSTALLATION_FAUTE`

- [ ] Une ligne, sur le modèle exact des voisines :
      `${INSTALLATION_FAUTE:+\$env:INSTALLATION_FAUTE = '$INSTALLATION_FAUTE'}`.
- [ ] **Rien d'autre dans cette tâche.**

**Test :** lancer l'agent avec la variable posée et **lire la TRACE**, pas le
script. 🔴 **Le contrôle qui vaut n'est pas la lecture du `.ps1` généré mais la
ligne d'agent** : `faute d'installation ARMEE (INSTALLATION_FAUTE=…)`.
**Ce qui le rend ROUGE :** retirer la ligne — l'agent démarre **sans la variable
ET SANS RIEN SIGNALER**, ce qui est le symptôme exact du piège payé cinq fois
(D1 `SUPERVISEUR`, D2 `MULTIFENETRE_REPRISE`, D7 `AUDIO`).

---

# Famille 4 — la plateforme, persistance

### Task 23 : la migration — deux tables, leurs clés étrangères, et TOUTES leurs colonnes

- [ ] 🔴 **Le numéro se relève par `ls plateforme/src/base/migrations/`** (D2),
      et `plateforme/src/base/pilotes.test.ts:33` reçoit la liste **exacte**
      observée — un trou n'est pas un défaut (`migrations.ts` trie
      numériquement et n'exige aucune contiguïté, relevé).
- [ ] `televersement(id, utilisateur_id → utilisateur(id), nom, taille,
      sha256, taille_tranche, cree_a, scelle_a NULL)`.
- [ ] `installation(id, vm_id → vm(id), televersement_id → televersement(id),
      demandee_a, etat, phase, octets_faits, octets_total, ecoule_ms,
      code_sortie NULL, issue NULL, motif NULL, journal NULL, journal_tronque,
      terminee_a NULL, maj_a)`.
- [ ] 🔴 **Les clés étrangères naissent AVEC les tables** — leg n°2 de P1,
      rappelé par `0004-applications.sql` : SQLite ne sait pas les ajouter par
      `ALTER TABLE`. **Sans `ON DELETE`**, comme `application.vm_id` (E10).
- [ ] 🔴 **TOUTE colonne `NOT NULL` dont un sous-bloc ultérieur aura besoin naît
      MAINTENANT** : `ADD COLUMN … NOT NULL` sans défaut est refusé dès que la
      table porte une ligne (divergence E8 de G1), et un `DEFAULT` littéral est
      **impossible** (`rendreMarqueurs` refuse les apostrophes, relevé).
- [ ] Horodatages en **`BIGINT`**, jamais `INTEGER` (4 octets sur Postgres, et
      un `Date.now()` n'y tient pas), et tous suffixés `_a` **sans quoi le lint
      de `sous-ensemble.test.ts` ne peut pas les voir**.
- [ ] Un index sur `installation(vm_id, etat)` — c'est la requête de la
      réémission à l'enrôlement (tâche 31).

**Test :** `npm run test:sqlite` **et** `npm run test:postgres`. ⚠️ **Un saut est
un échec** : si Postgres manque, la passe échoue, **elle ne se saute pas**
(règle de P1 §7.1).
**Ce qui le rend ROUGE :** trois mutations, toutes atteignables —
`ADD COLUMN … UNIQUE` (🔴 **refusé par SQLite, accepté par Postgres** : c'est
exactement ce que la double passe existe pour attraper) ; une clé étrangère
oubliée (l'insertion d'une installation pour une VM inexistante **passe** au
lieu d'échouer — `PRAGMA foreign_keys = ON`, relevé) ; un `INTEGER` au lieu d'un
`BIGINT` (Postgres déborde).

### Task 24 : `depot/televersement.ts` et `depot/installation.ts`

- [ ] Créer, lire par identifiant, sceller, lister les non scellés d'un
      utilisateur, compter ceux en cours ; créer une installation, avancer son
      état, la terminer, lister celles d'une VM en `en_attente`.
- [ ] ⚠️ **Aucune horloge lue ici** : `maintenant` est un **paramètre**, comme
      partout dans ce dépôt.
- [ ] 🔴 **Le dépôt ne décide de rien** — pas de règle de tranches, pas de
      verdict : il écrit ce qu'on lui donne. La règle vit dans `proto/ts/` (D6)
      et dans l'agent (D9).
- [ ] ⚠️ **`pg` rend tout entier long en TEXTE** : la conversion se fait **au
      PILOTE**, jamais par une rustine locale — c'est le legs de P1, et la
      moindre `Number(l.taille)` recopiée ici serait la rustine qu'il interdit.
      **Vérifier que le pilote couvre les colonnes neuves**, et l'étendre là si
      ce n'est pas le cas.

**Test :** `depot/televersement.test.ts`, `depot/installation.test.ts`, **double
passe**.
**Ce qui le rend ROUGE :** un `taille` de 3 Go relu comme une **chaîne** au lieu
d'un nombre sous Postgres — le test compare `typeof`, pas seulement la valeur.
**C'est la rouge du legs `pg`**, et elle ne se voit que sur la passe Postgres.

### Task 25 : le magasin de tranches, et `PLATEFORME_TELEVERSEMENTS`

- [ ] `plateforme/src/apps/magasin-tranches.ts` : écrire une tranche (en flux),
      **lister** celles présentes avec leurs tailles, ouvrir un flux de
      concaténation, supprimer un téléversement.
- [ ] 🔴 **La reprise est un LISTAGE, jamais une comptabilité** (spec D7, D7 de
      ce plan). Le listage rend `(n, octets)`, que `proto/ts/tranches.ts`
      transforme en verdict — **le magasin ne juge de rien**.
- [ ] 🔴 **Le nom de fichier d'une tranche est le nombre `n` VALIDÉ**, jamais un
      segment d'URL recopié : `/televersement/x/tranche/..%2f..%2fetc` ne doit
      pouvoir écrire nulle part.
- [ ] `PLATEFORME_TELEVERSEMENTS` dans `config.ts`, **facultative**, défaut
      `donnees/televersements`, **journalisée au démarrage**. ⚠️ **Asymétrie
      assumée avec `PLATEFORME_HOTE`**, qui n'a aucun défaut : là, un mauvais
      défaut **exposerait** le service ; ici il coûte **un retéléversement**,
      borné et visible. Un silence, en revanche, ne serait pas acceptable —
      d'où la ligne de journal. *(C'est le raisonnement que le plan de G2 tient
      déjà pour `PLATEFORME_ICONES` ; les deux répertoires sont frères.)*

**Test :** `apps/magasin-tranches.test.ts`, double passe (il ne touche pas la
base, mais il court dans la même suite).
**Ce qui le rend ROUGE :** un `n` non validé — le test tente
`n = '../../evil'` et exige un refus **avec son motif**, pas un fichier écrit
ailleurs. Et une tranche supprimée sous les pieds du flux de concaténation doit
donner une **erreur**, jamais un flux tronqué **silencieux**.

---

# Famille 5 — la plateforme, HTTP et canal

### Task 26 : `http/porteur-agent.ts` — le lecteur de jeton d'AGENT, PUR

- [ ] ⚠️ **Si G2 l'a déjà livré, le RÉEMPLOYER** (D12, E9) — une copie
      divergerait en silence.
- [ ] Sinon : symétrique de `porteur.ts`, exigeant `type === 'agent'`, rendant
      le **sujet** (le **préfixe de session**, jamais l'identifiant de VM).
- [ ] Mêmes refus typés, mêmes codes : `401 jeton-absent`, `401 jeton-invalide`,
      `401 jeton-expire`, et **`403 jeton-utilisateur`** — le symétrique exact
      du `403 jeton-agent` de `porteur.ts`, et pour la même raison : le jeton
      est **valide**, il n'est simplement pas celui d'un agent.

**Test :** `http/porteur-agent.test.ts`.
**Ce qui le rend ROUGE :** un jeton **d'utilisateur** accepté. 🔴 **Les deux
tests sont SYMÉTRIQUES et vont par paire** — `porteur.ts` refuse l'agent,
celui-ci refuse l'humain. Si l'un des deux passe dans le mauvais sens, les deux
identités deviennent interchangeables, et **elles sont signées par le même
secret** (`identite/jeton.ts` énumère les deux confusions et dit qu'elles sont
graves toutes les deux).

### Task 27 : `routes-televersement.ts` — les quatre routes du navigateur

- [ ] `POST /televersement`, `PUT /televersement/:id/tranche/:n`,
      `GET /televersement/:id`, `POST /televersement/:id/sceller` (D4).
- [ ] **Jeton porteur** par `lirePorteur`, **jamais une copie** — c'est ce que
      G1 a tranché en tête de `routes-applications.ts`.
- [ ] **Un téléversement appartient à un utilisateur** : toute route vérifie
      `televersement.utilisateur_id`, et rend **le même refus indistinguable**
      qu'une ressource inconnue. 🔴 **Le propriétaire du dépôt a tranché ce
      point pour G1** — le `403` distinct était un **oracle d'énumération** —,
      et G3 applique la décision **sans la rouvrir** : `404 { refus: 'televersement-inconnu' }`
      dans les deux cas, et **une ligne de journal** qui, elle, nomme le cas
      réel et **n'atteint jamais la réponse**.
- [ ] Le `PUT` écrit **en flux** avec sa borne dure (D8) ; `413` au
      franchissement, fichier partiel supprimé.
- [ ] Le scellement : passe de flux, `sha256` recalculé, `409 { refus: 'empreinte' }`
      s'il diffère, `409 { refus: 'tranches-manquantes' }` ou `'tranches-incoherentes'`
      selon le verdict de `proto/ts/tranches.ts`.
- [ ] 🔴 **Chemin découpé PAR SEGMENTS, jamais par `startsWith`** — `lancementDe`
      (`routes-applications.ts`) est le modèle, avec son commentaire.
- [ ] `OPTIONS` servi ; `ENTETES_SECURITE` étalés **avant** `cors`, sur **toute**
      réponse.

**Test :** `http/routes-televersement.test.ts`, **double passe**, contre de
vrais sockets.
🔴 **LE CONTRÔLE DE CHEMIN COMPARE LE CORPS, JAMAIS LE SEUL STATUT** : G1 a
mesuré qu'un `startsWith('/application')` **laissait DIX-SEPT tests verts** — la
route mangeait toute la famille et rendait **son propre 404 typé**. Le test
compare donc le corps à celui du 404 **générique** de `serveur.ts`, sur au moins
quatre chemins déclinés : `/televersementautre`, `/televersement/`,
`/televersement/x/tranche`, `/televersement/x/tranche/1/z`.
**Ce qui le rend ROUGE — six mutations, toutes après le vert :**
1. `startsWith` : ⚠️ **c'est celle qui a SURVÉCU en G1.** Si elle survit encore,
   **le test est faux, pas la mutation** ;
2. la vérification de propriétaire retirée : le téléversement d'autrui est lu ;
3. `403` au lieu de `404` : l'oracle que le propriétaire du dépôt vient de
   retirer — **le test compare le corps** ;
4. le scellement qui **fait confiance** au `sha256` annoncé au lieu de
   recalculer : le test dépose des tranches dont le contenu ne correspond pas et
   exige `409` ;
5. `verdict` remplacé par « le compte de tranches est bon » : le test dépose la
   bonne **quantité** de tranches dont **une** a la mauvaise taille, et exige
   `tranches-incoherentes` ;
6. la borne du `PUT` retirée : le test envoie `taille_tranche + 1` octets et
   exige `413` **et** l'absence du fichier partiel.

### Task 28 : `routes-installation.ts` — l'ordre, l'état, et le contenu servi à l'AGENT

- [ ] `POST /installation` (jeton porteur ; la VM doit appartenir au demandeur —
      même refus indistinguable, même ligne de journal) ; `GET /installation/:id`
      (idem) ; `GET /televersement/:id/contenu` (**jeton d'AGENT**, tâche 26).
- [ ] 🔴 **`GET …/contenu` compare la VM du jeton à celle de l'installation**
      (D12). **Sans cette comparaison, n'importe quelle VM enrôlée télécharge
      l'installeur de n'importe quelle autre.**
- [ ] Le contenu est servi **en flux** depuis le magasin (D7), avec
      `Content-Length` et `Content-Type: application/octet-stream`. ⚠️ **Aucun
      `Content-Disposition`, aucun nom de fichier** : l'agent connaît le nom, il
      l'a reçu dans l'ordre.
- [ ] Un téléversement **non scellé** n'est jamais servi : `409 { refus: 'non-scelle' }`.
- [ ] `POST /installation` refuse si le téléversement n'est pas scellé, si son
      extension n'est pas acceptée (la règle est **côté agent** — D15 —, mais le
      refus précoce évite un aller-retour), ou si la VM est **injoignable**
      (`503`, comme `routes-applications.ts` le fait déjà pour le lancement).

**Test :** `http/routes-installation.test.ts`, **double passe**.
**Ce qui le rend ROUGE — quatre mutations :**
1. 🔴 **la comparaison de VM retirée sur `/contenu`** : un jeton d'agent d'une
   VM `B` télécharge l'installeur destiné à `A`. **C'est la mutation la plus
   importante du sous-bloc**, et le test l'exige explicitement ;
2. le jeton **porteur** accepté sur `/contenu`, ou le jeton **d'agent** accepté
   sur `POST /installation` : les deux tests symétriques tombent ;
3. le `409 non-scelle` retiré : l'agent reçoit un fichier **partiel** dont
   l'empreinte échouera — et le test **exige le refus en amont**, parce qu'un
   refus au bon endroit vaut mieux qu'un refus au bon moment ;
4. `startsWith` (même famille que la tâche 27, même contrôle par le corps).

### Task 29 : chaîner les DEUX routeurs, et leurs DEUX `it()`

- [ ] `plateforme/src/http/serveur.ts` : deux lignes, après `servirApplications`
      (l. 223).
- [ ] 🔴 `plateforme/src/http/entetes-routeurs.test.ts` : **un `it()` par
      routeur neuf, donc DEUX** (E12). Son en-tête l'exige en toutes lettres
      (l. 3, relu) : « UN `it()` PAR ROUTEUR, ET JAMAIS UN TEST GLOBAL », et il
      nomme le précédent — **« G1 vient d'ajouter un routeur sans que personne
      ne s'en aperçoive côté P5 »**. ⚠️ **Ne pas rejouer le défaut que ce
      fichier existe pour empêcher.**
- [ ] ⚠️ **Vérifier que les jeux de chemins restent DISJOINTS** des cinq
      routeurs existants — `serveur.ts` s'appuie dessus explicitement (l. 226-231).

**Test :** `npm run test:sqlite`, `npm run test:postgres`, `npm run typecheck`.
**Ce qui le rend ROUGE :** retirer l'étalement d'`ENTETES_SECURITE` d'**un
seul** routeur neuf — **son** `it()` tombe, les six autres restent verts.

### Task 30 : 🔴 EXTRACTION de `plateforme/src/agents/canal.ts`, AVANT l'addition

- [ ] **423** lignes (relevé), marge **77**. Extraire les branches **montantes
      de ④** vers `plateforme/src/agents/canal-apps.ts` : `catalogue`, `lancee`,
      et **rien de neuf** — `canal.ts` garde le cycle de vie (enrôlement,
      battement, refus, frein).
- [ ] **Transposition VERBATIM. Aucune ligne de comportement.**

**Test :** `npm run test:sqlite`, `npm run test:postgres`.
**Ce qui le rend ROUGE :** le compte de tests, annoncé avant d'être mesuré.
**Contrôle de taille :** `wc -l` sur les deux fichiers, et **la marge rendue est
reportée** — c'est elle que la tâche 31 va consommer. ⚠️ **La marge regagnée par
une extraction se reperd à la ronde suivante si on la traite comme acquise** :
ce dépôt l'a payé **quatre fois**.

### Task 31 : le canal de la plateforme — pousser, recevoir, et RÉÉMETTRE

- [ ] `canal-apps.ts` gagne `progression` et `termine` (montantes), qui écrivent
      dans `installation` — ⚠️ **`void … .catch(…)`, jamais `await`** : une base
      momentanément indisponible ne doit pas abattre la connexion d'un agent qui
      va très bien, et une promesse rejetée sans `catch` **abat tout le process
      Node**. C'est la règle que `canal.ts` s'impose déjà pour `marquerVu` et
      pour le catalogue.
- [ ] 🔴 **La réémission à l'enrôlement** (spec D7, chute n°2) : après un
      enrôlement réussi, lire les installations `en_attente` de cette VM et
      pousser un `Installer` pour chacune. **Un `push` WebSocket n'a aucune
      garantie de livraison** : sans cette réémission, un ordre émis pendant une
      coupure serait perdu **sans terme**. C'est le même filet que
      `complet = true` du catalogue, et **la recette de G1 a vu ce filet
      fonctionner sur le chemin réel**.
- [ ] ⚠️ **Ni progression ni issue sans enrôlement** : `refuser('sequence')`,
      exactement comme `catalogue` et `lancee` — sans quoi un pair anonyme
      écrirait dans la table `installation` d'une VM qu'il n'a pas authentifiée.

**Test :** `agents/canal.test.ts` (et son harnais), **double passe**.
**Ce qui le rend ROUGE :** 🔴 **la réémission retirée** — le test enrôle, coupe
le socket **avant** que l'ordre ne parte, réenrôle, et exige de voir
l'`Installer` arriver. **C'est la rouge du critère ④ de la spec, et elle est
GRATUITE sur le binaire de G1** (qui n'a pas de variante `Installer` du tout).
Et : un `progression` reçu **sans enrôlement** doit rendre `sequence`.

### Task 32 : `deploiement/nginx.conf` — les chemins de G3, et les DEUX de G1

- [ ] Ajouter les `location` **énumérés** — jamais un motif large, la doctrine
      du fichier est explicite : « un `location /api/` n'existe pas ici, et un
      fourre-tout relaierait au service des chemins que personne n'a décidés ».
- [ ] 🔴 **Poser `client_max_body_size` explicitement** sur la route de tranche —
      **au moins `TAILLE_TRANCHE`** (M4 : aucun plafond n'est posé aujourd'hui,
      et le défaut nginx est **1 m** d'après sa documentation, ⚠️ **non mesuré
      ici**).
- [ ] Poser `proxy_request_buffering off` sur la route de tranche et
      `proxy_buffering off` sur la route de contenu : sans quoi le proxy
      tamponne des centaines de mégaoctets **sur son propre disque** avant de
      relayer. ⚠️ **Lecture de la documentation nginx, NON MESURÉE ici** — et
      c'est écrit comme tel.
- [ ] Allonger `proxy_read_timeout` sur la route de contenu : un téléchargement
      de plusieurs centaines de mégaoctets dépasse le défaut de 60 s.
- [ ] 🔴 **Ajouter AUSSI `location = /applications` et
      `location ~ ^/application/[^/]+/lancer$`** — les deux routes de **G1**, que
      le profil ne route pas (E1) et qui rendent aujourd'hui **la page en 200**.
      **Déclaré comme une addition au périmètre**, et justifié : un hub qui
      téléverse sans pouvoir lister n'a aucun sens.

**Test :** `nginx -t` **si un nginx est disponible** ; sinon, une relecture
attentive et **la mention explicite, dans le rapport, que le fichier n'a PAS été
validé par un nginx**. ⚠️ **`nginx -t` déclare « ok » une configuration qui rend
la page à un client WebSocket** — ce fichier le raconte lui-même (l. 172-179) :
**un `-t` vert ne prouve rien sur le routage.**
**Ce qui le rend ROUGE :** ⚠️ **rien d'automatisé, et il faut le dire.** Aucune
tâche de ce plan ne monte le profil de déploiement. **La seule épreuve honnête
serait un rang de recette derrière nginx, et il n'est pas au programme** — c'est
inscrit au §« Ce que G3 n'établira PAS ».

---

# Famille 6 — le navigateur, sans DOM

### Task 33 : `client/src/hub/televersement.ts` — l'orchestration

- [ ] `televerser(fichier, deps)` où `deps` porte **`fetch`, l'horloge et un
      `AbortSignal`** : aucune de ces trois choses n'est lue dans le module.
      **Aucun DOM.**
- [ ] La séquence : empreinte (phase `empreinte`, cadencée), création, dépôt des
      tranches **manquantes seulement**, scellement.
- [ ] 🔴 **La reprise vérifie l'identité du fichier avant de reprendre** (D5) :
      `{taille, sha256}` du téléversement contre ceux du fichier re-choisi ;
      **refus typé** si l'un des deux diffère. **Sans ce contrôle, les tranches
      de deux fichiers se mélangeraient et le scellement échouerait sans que
      rien ne dise pourquoi.**
- [ ] `File.slice` pour ne jamais tenir plus d'une tranche en mémoire, et un
      `await` entre deux tranches pour que l'onglet reste vivant.

**Test :** `client/src/hub/televersement.test.ts` (vitest, sans DOM), avec un
`fetch` factice.
**Ce qui le rend ROUGE — trois scénarios :**
1. le serveur annonce `tranches_presentes = [0, 1]` sur un fichier de 4
   tranches → **le test compte les octets envoyés** et exige **deux** tranches,
   pas quatre. 🔴 **C'est la rouge du critère ③ de la spec** : « recommencer à
   zéro passerait un test qui ne regarde que le résultat » ;
2. un fichier de taille différente présenté à une reprise → **refus typé**, et
   **aucune** requête `PUT` émise (le test compte les appels au `fetch`
   factice) ;
3. le scellement rend `409 empreinte` → l'erreur **remonte typée**, elle n'est
   ni avalée ni transformée en réessai infini.

---

# Famille 7 — recette, revue, et clôture

### Task 34 : 🔴 LA RECETTE, sur la VM Windows

⚠️ **Préalables, tous les trois, dans cet ordre** — le premier a été payé trois
fois sur trois en D8 :

1. `Get-Process agent` **avant** — un superviseur resté vivant empêche le
   nouveau journal de s'ouvrir, et **on relit le run d'avant en croyant lire le
   sien** ;
2. `virsh list --all`, puis attendre **`ls /media/vm/dev`**, jamais le seul port
   5985 (`mountpoint -q` ne suffit pas : l'entrée CIFS survit à une VM éteinte) ;
3. `set -a && source .env && set +a` — sans quoi `build-agent.sh` **s'arrête en
   silence** après « sources synchronisées » et l'on mesure le binaire précédent.

**Le montage** : superviseur sur la VM, plateforme et navigateur **sur l'hôte**,
un fichier d'installeur réel de **plusieurs centaines de mégaoctets**, et un
`.exe` témoin qui **rend 0 sans rien installer** (critère ⑥) — compilé, ou un
`cmd /c exit 0` renommé, **et le dire**.

**Les critères, avec le nombre d'exécutions attendu. DEUX par critère, jamais
une. Aucun taux ne sera revendiqué.**

| # | Critère | Comment il est jugé | Ce qui le rend ROUGE |
| --- | --- | --- | --- |
| ① | Un installeur réel de plusieurs centaines de Mo traverse et s'installe | l'application apparaît au catalogue après la réconciliation, et `GET /installation/:id` rend `reussie` | 🔴 **la ROUGE est GRATUITE** : sur le binaire de G1, `POST /televersement` rend **404** — ⚠️ **en accès direct**, pas derrière nginx (E11). **À JOUER, pas à supposer** |
| ② | Une empreinte fausse est refusée **aux trois étages** | trois épreuves : altérer une tranche déposée ; altérer une tranche **sur le disque de la plateforme** avant le scellement ; `INSTALLATION_FAUTE=empreinte` côté agent | 🔴 **retirer UNE vérification doit faire rougir SON test et LAISSER LES DEUX AUTRES VERTS** — sans quoi on ne sait pas laquelle protège. **Les trois mutations sont jouées séparément** |
| ③ | Une coupure en cours de téléversement **reprend là où elle en était** | couper après *k* tranches, relancer ; **compter les octets du second passage** | recommencer à zéro **passerait** un test qui ne regarde que le résultat. **Le compte d'octets est le critère**, pas le succès |
| ④ | Un ordre d'installation **survit à une chute du canal** | tuer le canal avant que l'agent ne télécharge ; la réémission à l'enrôlement le rejoue | 🔴 **ROUGE GRATUITE sur le binaire de G1** (aucune variante `Installer`), et **mutation disponible** : retirer la réémission de la tâche 31 |
| ⑤ | Une réémission **n'installe pas deux fois** — ni dans le même processus, **ni après un redémarrage de l'agent** | même `installation.id` réémis ; **un seul** processus lancé. Puis : tuer l'agent, le relancer, laisser la réémission arriver → **aucun second lancement** (D17) | retirer la déduplication mémoire : deux processus. Retirer le marqueur disque : **le second cas seul rougit** — et c'est celui que la spec ne demandait pas |
| ⑥ | Un code de sortie **0 sans effet** est rapporté `sans_effet`, jamais `reussie` | le `.exe` témoin | interpréter le code de sortie : il rend `reussie`. **La ROUGE est le témoin lui-même** |
| ⑦ | Tuer l'agent pendant l'installation **ne tue pas l'installeur** | le processus survit ; l'issue est `issue_inconnue` | ⚠️ **CE CRITÈRE EST VACUEUX SI L'AGENT N'EST DANS AUCUN JOB** (E7, tâche 2). **La rouge est une MUTATION** : assigner délibérément l'installeur au job, et le voir mourir. **Sans cette mutation jouée, ⑦ ne prouve rien**, et le rapport doit le dire |
| ⑧ | Le périphérique de rendu par défaut est tracé **avant et après** | deux lignes par installation, avec le nom du point de terminaison | ne tracer qu'après : un changement n'est plus attribuable. 🔵 **Bonus si l'installeur réel est VB-Cable** — le changement se produirait pour de vrai, et le critère cesserait d'être théorique |
| ⑨ | Un refus de version est **LISIBLE** | agent d'une version antérieure contre plateforme de la nouvelle : la ligne `la plateforme REFUSE la version` apparaît **avec `version_emise` et `version_recue`**, et l'agent **renonce** | 🔴 **c'est la SECONDE mesure de la correction du 20 août 2026** ; avant elle, G1 a mesuré **0** ligne de refus et **10** reprises sans terme |

- [ ] **Journaux versés dans git** sous
      `docs/superpowers/plans/journaux-gestion-apps-g3/`, bruts **et** `-plat`
      jumeaux, avec la **famille de lecture** annoncée (ANSI présentes ou non,
      octets NUL ou non) — le tableau de tête que D9, D10, P1 et G1 posent tous.
      ⚠️ **`grep -a` obligatoire** sur les journaux de pilote.
- [ ] **Copier `agent.log` APRÈS la fin réelle de l'exécution**, pas à la fin du
      pilote : les enfants meurent après, et leurs lignes de libération partent
      avec le journal suivant (piège de D4).
- [ ] `Get-Process agent` **après**, et purge des sorties virtuelles si le
      superviseur a été tué net.

### Task 35 : 🔴 LA REVUE TRANSVERSE DE FIN DE BRANCHE — obligatoire

**Barème, pour dire ce qu'on cherche** : **douze** défauts en D10, **treize** en
S3, **vingt-sept** en S4, **huit** en G1, **dix-sept** dans le presse-papier P1,
**neuf** en P5, **onze** en F1. **Une revue qui n'en trouve aucun n'a pas
cherché.**

- [ ] **Sa cible propre : les affirmations de code devenues fausses DANS LEUR
      PROPRE BRANCHE.** Aucune revue par tâche ne peut les voir — la tâche qui
      écrit la phrase et celle qui la réfute ne se relisent jamais.
- [ ] **Les candidats sont nommés d'avance**, et il faut les relire un par un :
      - `agent/src/apps/lancement.rs` — « `ShellExecuteEx` crée son processus
        hors de tout job » (E8) : **non mesuré, et vrai pour une raison que la
        phrase ne donne pas** ;
      - `agent/src/apps.rs` — la doc de `brancher` ne parle que de découverte,
        alors qu'elle branche désormais l'installation (D10) ;
      - `agent/src/plateforme.rs` — la doc de `Canal` énumère ce que « le
        lâcher » arrête : **il y a maintenant un troisième mécanisme** ;
      - `plateforme/src/agents/canal.ts` — son en-tête énumère les variantes
        de ④ et dit « `PLATEFORME_VERSION` est passée à 2 pour cela » ;
      - `proto/src/plateforme.rs` — la doc de `PLATEFORME_VERSION` liste v1 et
        v2 : **la version neuve doit y être décrite**, sinon la constante ment ;
      - `plateforme/src/base/migrations/0003-agents.sql` et `0004-applications.sql` —
        leurs encadrés parlent de la table `application` **et de ses voisines** ;
      - la **spec de ④** (§4 D7, §5 « G3 », §6) : E2, E3, E4, E5 et E12 sont
        des divergences **à annoter, pas à réécrire** — ce sont des relevés
        datés ;
      - `CLAUDE.md` : la citation `fraicheur.ts:37` (E6).
- [ ] 🔴 **Vérifier PAR LA COMMANDE, et non supposer**, qu'aucun fichier touché
      n'est sous `agent/src/capteur/`, `agent/src/superviseur/`, `src/`, `web/`
      ni `agent/src/main.rs` : `git diff --name-only` contre la base de branche.
- [ ] 🔴 **Une affirmation de COMPLÉTUDE se vérifie en énumérant ses places
      AVANT de l'écrire** : `grep -n '<le nombre>' CLAUDE.md`, puis **relire
      place par place APRÈS l'édition**. Une substitution qui ne dit pas combien
      d'occurrences elle a touchées est une affirmation non vérifiée — le
      naufrage du « 487 », **neuf fois** dans ce dépôt.

### Task 36 : `CLAUDE.md` — la section G3, et les tailles PAR LA COMMANDE

- [ ] La section G3 : le verdict de la porte **en premier** (c'est ce qu'on
      viendra y chercher), les critères avec leur **nombre d'exécutions**, la
      variable `INSTALLATION_FAUTE` dans le tableau des variables, les pièges
      neufs, ce que G3 n'établit pas, et les legs.
- [ ] 🔴 **Écrire l'absence de retour arrière** (D19) **à côté de G3**, pas
      seulement à côté de ⑤ : c'est G3 qui la rend visible.
- [ ] 🔴 **Relancer la commande des tailles APRÈS la dernière édition de la
      ronde**, revue transverse comprise — *une table relevée en début de ronde
      serait fausse à la fin de la même ronde*, erreur que D8 a commise en
      croyant bien faire.
- [ ] **Retirer de la table de dette** les deux fichiers de `proto/` **si G3 est
      celui qui les a purgés** (tâches 3 et 4) — une dette payée qui reste
      écrite est une affirmation devenue fausse.
- [ ] ⚠️ **Toucher une ligne d'un tableau de comptes OBLIGE à remesurer son
      compte**, même quand ce n'est pas l'objet de l'édition.

### Task 37 : le document de résultats

- [ ] `docs/superpowers/plans/2026-08-20-gestion-apps-g3-resultats.md`.
- [ ] Le §1 est **le verdict de la porte**, avec ses quatre valeurs de
      configuration, son témoin (a), sa preuve (b), et ce qu'il commande.
- [ ] Chaque affirmation porte **son nombre d'exécutions**, et **aucun taux
      n'est revendiqué**.
- [ ] 🔴 **Les pièces vivent SOUS `docs/`, jamais dans un rapport gitignoré** :
      D10 a établi que l'espace de travail de D9 a **disparu** avec **six**
      constats de revue que rien ne rattrape. **La preuve d'une affirmation de
      ce dépôt ne doit jamais vivre dans un fichier non suivi.**

---

## Ce que G3 n'établira PAS

- **Aucun taux, nulle part.** Deux exécutions par critère au mieux, et la porte
  n'en aura pas davantage.
- **Rien de la charge** : le nombre de téléversements simultanés qu'une
  plateforme soutient n'est pas mesuré, ni le débit du transfert vers la VM.
  ⚠️ **Le pont est connu porter ≥ 1,44 Gb/s** (D6 du chantier D), donc un
  téléchargement de plusieurs centaines de mégaoctets est **probablement** sans
  effet sur une session concurrente — *probablement* est le mot juste, et il
  n'est pas remplacé par un chiffre.
- **Rien d'un antivirus.** ⚠️ **L'état de celui de la VM n'a PAS été relevé** —
  ni par la spec, ni par G1, ni ici. L'analyse d'un installeur de plusieurs
  centaines de mégaoctets peut ajouter un délai, voire une **mise en quarantaine
  qui ferait échouer l'installation avec un code de sortie trompeur** : ni
  mesuré, ni prédit.
- **Rien d'un installeur qui s'élève LUI-MÊME en cours de route** (D15) : le
  refus typé ne l'attrape pas, et seule l'expiration le borne.
- **Rien du profil de déploiement.** Aucune tâche ne monte nginx : les
  directives de la tâche 32 sont **écrites, jamais éprouvées**, et
  ⚠️ **`nginx -t` ne prouve rien sur le routage** — ce fichier raconte
  lui-même une configuration déclarée « ok » qui rendait la page à un client
  WebSocket.
- **Rien du `Transfer-Encoding: chunked`** : il est **refusé**, pas géré, et
  personne n'a mesuré si un proxy le produirait (D13).
- **Rien de TLS** : l'agent ne parle ni `wss` ni `https` (M3), et G3 ne l'y
  amène pas.
- **La vérification n°1 est éprouvée sur son CODE, pas dans son
  ENVIRONNEMENT** : le pilote de recette exécute les modules du navigateur
  **sous Node** (D14). Le débit de `sha256.ts` sous un moteur de navigateur
  n'est pas mesuré, et `File.slice` n'est pas `fs.read`.
- **Aucune page de hub, aucun glisser-déposer, aucun `file_handler`** — G5.
- **Aucune surveillance de répertoire, aucun anti-rebond** — G4. La latence du
  verdict reste bornée par `PERIODE_RECONCILIATION` (30 s).
- **Aucune constante calibrée** : `TAILLE_TRANCHE`, `TELEVERSEMENT_MAX_OCTETS`,
  `TELEVERSEMENTS_EN_COURS_MAX`, `EXPIRATION_TELEVERSEMENT`,
  `EXPIRATION_INSTALLEUR`, `EXPIRATION_EXECUTION`, `PERIODE_PROGRESSION`,
  `JOURNAL_MAX_OCTETS`, `RETABLISSEMENTS_MAX`. Elles rejoignent `BPP_MIN`,
  `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`, `HYSTERESIS`, `REPIT_APRES_ECHEC`,
  `TAILLE_MAX_SORTIE`, `SEUIL_INJOIGNABLE_MS`, `DELAI_LANCEMENT_MS`,
  `PERIODE_RECONCILIATION`, `FILE_EMISSION`.
- **Aucune restauration après un installeur destructeur** (D19), et **aucun
  retour arrière** : l'orchestrateur v1 refuse `instantane`.
- **Aucun audit de sécurité.** Le modèle de menace reste celui de ⑤, borné à
  « un pair anonyme n'obtient rien ». **L'exécution d'un binaire arbitraire dans
  la VM de son propre auteur est une décision de produit déjà prise**, pas une
  vulnérabilité que G3 introduirait.
- **Le leg n°1 de G1 n'est pas refermé** : G3 s'en protège par un refus (D10),
  il ne le corrige pas. **Et tant qu'il vit, deux boucles de découverte
  tournent**, donc deux processus peuvent recevoir un ordre.
- **Aucune isolation entre utilisateurs sur les VMs non attribuées** : `vm.utilisateur_id`
  est NULL après `npm run admin:agent`, et **tout utilisateur authentifié voit
  toutes les VMs** — la ligne `vm non attribuee, acces accorde sans isolation…`
  le dit à chaque requête. **G3 hérite de cet état ; il ne l'aggrave pas et ne
  le répare pas.**
- **Rien d'un client réel, rien du HiDPI** : la recette reste un Chromium sans
  interface, limite héritée de D5 qu'aucun sous-bloc n'a levée.

---

## Risques, et ce qui rendrait G3 NON LIVRABLE

| Risque | Ce qu'il coûte | Mitigation, ou constat |
| --- | --- | --- |
| 🔴 **La porte rend NON CAPTURÉE** | tout installeur exigeant une élévation devient inutilisable ; l'utilisateur verrait un écran figé | **G3 reste livrable** : `CreateProcessW` transforme le cas en **refus typé** (D15), et le périmètre survivant est **nommé** (« LA PORTE »). ⚠️ **Ce qu'aucun remède n'attrape** : l'installeur qui s'élève lui-même |
| 🔴 **La porte rend NON MESURABLE deux fois** | on ne sait rien, et l'on ne peut pas décider | **le sous-bloc le DIT** et ne conclut pas. Il livre le reste, et le §1 des résultats porte `NON MESURABLE` avec ses deux disqualifications |
| 🔴 **Deux montées de `PLATEFORME_VERSION` vers le même numéro** | deux protocoles différents sous le même numéro, et une **boucle de reconnexion sans terme** au lieu d'un refus | **D1** : la valeur se **relève**, jamais ne s'écrit en dur ; la tâche 5 **échoue** si les trois lieux divergent |
| 🔴 **L'ordre `Installer` échoit au PONT** | l'installeur est lancé depuis un processus **assigné au job** et meurt avec le superviseur — exactement ce que la spec D8 interdit | **D10** : garde `IsProcessInJob`, refus typé. ⚠️ **C'est un GARDE, pas le remède** ; le remède est le leg n°1 de G1, qui n'appartient pas à G3 |
| **Un `PUT` de 8 Mio refusé par le proxy** | le téléversement échoue **derrière le profil de déploiement seulement**, et le service n'en sait rien | **tâche 32**, `client_max_body_size` posé explicitement. ⚠️ **Non éprouvé** : aucune tâche ne monte nginx |
| **Un proxy qui rechunke la réponse de contenu** | l'agent **refuse** et l'installation ne part jamais | **refus TYPÉ nommant `chunked`** (D13) : diagnosticable en une ligne de journal, au lieu d'un analyseur qui devine |
| **Le SHA-256 JS trop lent sur un vrai navigateur** | l'onglet paraît figé pendant la phase `empreinte` | mesuré **sous Node** à 74,7 Mo/s (M2) ; **non mesuré sous un navigateur**. La phase est **affichée**, et le module cède la main entre deux tranches |
| **Un installeur qui casse la VM ou l'agent** | la VM devient injoignable et **rien ne la répare** | **assumé par le cadrage** (§3). ④ ajoute la trace du périphérique audio (D19), **rien d'autre**, et **il n'y a aucun retour arrière** |
| **Le disque de la plateforme se remplit** | le service tombe pour tout le monde | quota par utilisateur, borne par téléversement, balayage d'âge **opportuniste** (D8, D18). ⚠️ **Aucun minuteur d'entretien** : une plateforme qui ne reçoit plus rien garde ce qu'elle a |
| **`agent/src/plateforme.rs` ou `agents/canal.ts` franchissent 500** | dette de taille sur des fichiers déjà étroits | **extraction AVANT addition**, tâches 17 et 30, **jamais une compression** |
| **La recette tourne sous le leg n°1 de G1** | toutes les mesures portent cette condition, comme celles de G1 | **le dire dans les résultats**, comme G1 l'a dit |

---

## Contrôle final, avant de déclarer G3 clos

🔴 **DEPUIS UN SHELL PROPRE, ou `env -u TURN_URL -u TURN_SECRET`** — sans quoi
six tests de signaling échouent pour une raison étrangère (leg n°5 de G1) :

```bash
scripts/verify-all.sh          # DIX etape(), DIX-SEPT lignes affichées — dire laquelle on compte
cd agent && cargo check --target x86_64-pc-windows-gnu
```

Puis, **et seulement ensuite** :

- [ ] `cargo test --workspace` — le compte **annoncé avant d'être mesuré** ;
- [ ] `cargo clippy --workspace` — ⚠️ **vérifier la NATURE des avertissements,
      jamais leur NOMBRE** : il dérive d'une exécution à l'autre selon la
      fraîcheur du build (99 puis 100 relevés par deux relecteurs en D3) ;
- [ ] `cd proto && npm test && npm run typecheck` ;
- [ ] `cd plateforme && npm run test:sqlite && npm run test:postgres && npm run typecheck`
      — ⚠️ **un saut est un échec** ;
- [ ] `cd client && npm test && npm run typecheck` ;
- [ ] la commande des tailles de `CLAUDE.md`, **relancée après la dernière
      édition de la ronde** ;
- [ ] `git diff --name-only <base>..HEAD` — **aucun fichier sous
      `agent/src/capteur/`, `agent/src/superviseur/`, `agent/src/main.rs`,
      `src/`, `web/`** ;
- [ ] les journaux de recette **versés** sous
      `docs/superpowers/plans/journaux-gestion-apps-g3/`, avec leur tableau de
      familles de lecture ;
- [ ] `git show --stat` après **chaque** commit — **pathspec explicite, jamais
      `git add -A`, jamais `--amend`.**
