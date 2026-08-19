# Sous-bloc P1 — résultats : le service naît, absorbe le signaling, et persiste

**Date de la recette :** 19 août 2026.
**Plan :** `docs/superpowers/plans/2026-08-19-plateforme-p1.md` (commit `0e335e5`).
**Spécification :** `docs/superpowers/specs/2026-08-19-plateforme-design.md` (commit `217a765`).
**Journaux :** `docs/superpowers/plans/journaux-plateforme-p1/` — **15 fichiers,
UTF-8, AUCUNE séquence ANSI** (Vitest ne colore pas quand sa sortie est
redirigée) : ils se `grep`ent à plat, sans `sed`. Ils portent
l'`ExperimentalWarning` de `node:sqlite`, **et c'est voulu** — il est la trace
visible de la décision §3.2 de la spec, et un contrôle du sous-bloc interdit de
l'éteindre.

⚠️ **Aucun taux n'est revendiqué nulle part.** Chaque énoncé porte son nombre
d'exécutions.

⛔ **Aucune tâche de ce sous-bloc n'a employé la VM Windows.**

---

## 1. Le verdict des quatre critères

| # | Critère | Verdict | Exéc. | Pièce |
| --- | --- | --- | --- | --- |
| ① | Le service apparie deux pairs simulés et le média négocie comme avant | **TENU** | 2 (+2 rouges) | `critere-1-empreintes.log`, `critere-1-rouges.log` |
| ② | Une session appariée laisse une trace en base | **TENU** | 2 (+2 rouges) | `critere-2-rouges.log`, `compatibilite-pairs-reels.log` |
| ③ | La même suite passe sur `node:sqlite` **et** sur Postgres | **TENU** | 2 par moteur (+1 rouge) | `sqlite-{1,2}.log`, `postgres-{1,2}.log`, `critere-3-double-passe.log` |
| ④ | L'écoute est bornée | **TENU, dans une portée étroite** | 2 (+1 rouge) | `critere-4-rouge.log` |

**Comptes relevés, deux exécutions chacun** : `Test Files 12 passed (12)`,
`Tests 64 passed (64)` sur **sqlite** comme sur **postgres** ; `typecheck`
sortie **0**.

`./scripts/verify-all.sh` : sortie **0**, « Toutes les vérifications sont
passées. » — cargo **467** + **32** + 0, client **107**, proto **35**,
plateforme **64** / **64**, typecheck 0 (`verify-all.log`).

---

## 2. Chaque ROUGE jouée, avec son message verbatim

C'est ce qui distingue un contrôle d'une formule. **Les huit ont été jouées
dans le tour qui écrit ce document**, et les sources ont été restaurées puis
comparées à l'identique après chacune.

| # | Ce qu'on casse | Message d'échec, verbatim |
| --- | --- | --- |
| ① A | `resilience.test.ts` l. 28 laissée sur `'..'` | `Error: spawn /home/…/plateforme/src/node_modules/.bin/tsx ENOENT` |
| ① B | l'annonce du port passe à `hote:port` | `Error: démarrage du process signaling expiré. stderr: …` |
| ② A | l'appel à `ouvrirSession` retiré | `aucune ligne session ouverte pour trace-1 en 2000 ms` |
| ② B | la ligne écrite dès la **déclaration**, non à l'appariement | `expected [ { …(7) } ] to have a length of +0 but got 1` |
| ③ | les horodatages repassés en `INTEGER` | `error: value "1700000000000" is out of range for type integer` |
| ④ | `PLATEFORME_HOTE ?? '0.0.0.0'` | `AssertionError: expected [Function] to throw an error` |
| lint A | un `_a INTEGER` dans une migration | `expected 'un horodatage \`_a\` en INTEGER : 4 oct…' to be ''` |
| lint B | la définition dupliquée de `schema_migration` diverge | `Expected: "…APPLIQUE_A BIGINT NOT NULL" / Received: "…APPLIQUE_A INTEGER NOT NULL"` |

Et une neuvième, hors suite : `verify-all.sh` instance Postgres arrêtée →
`connect ECONNREFUSED 127.0.0.1:5433` puis
`ÉCHEC : plateforme : npm run test:postgres`, **et le script s'arrête là** —
l'étape `typecheck` n'est jamais atteinte. **Un saut aurait été un échec ; il
n'y a pas de saut** (`verify-all-rouge-postgres-absent.log`).

### ⚠️ Deux attentes du PLAN que la mesure corrige

1. 🔴 **La tâche 12 attend `server.test.ts` à l'empreinte `2a1304e0…`**, ce que
   le renommage `server.ts` → `relais.ts` prescrit par sa **propre tâche 4**
   rend impossible. L'empreinte réelle est **`5e90b854…`**. **Le contrôle
   décidable est ailleurs, et il a été joué** : `diff` contre la version
   d'avant le déménagement rend **une seule ligne, la 3**, celle de l'import.
   `ice.test.ts` reste **`0b32514d…`, identique au bit près**
   (`critere-1-empreintes.log`).
2. **La tâche 1 step 5 annonce « les DEUX premiers tests échouent »** quand on
   pose le défaut `'0.0.0.0'`. **UN SEUL échoue.** La chaîne vide n'est pas
   *nullish* : `env.PLATEFORME_HOTE ?? '0.0.0.0'` ne s'y applique pas, et le
   test « n'invente pas d'adresse quand la variable est vide » reste vert. Le
   contrôle tient — il **peut** échouer — mais pas pour la raison écrite.

### ⚠️ `resilience.test.ts` a TROIS hunks de différence, non deux

Le plan promet « **1 ligne** (tâche 2) puis **2 lignes** (tâche 3) ». Le `diff`
relevé porte **trois** hunks : l. 28 (la racine du paquet), l. 38 (le point
d'entrée hissé), et l. 40 devenue un bloc `env` de treize lignes — parce que
`PLATEFORME_BASE` doit être **fixé** dans l'enfant, faute de quoi
`npm run test:postgres` lui transmettrait le moteur sans l'URL. **Aucune
assertion n'est touchée** : les trois hunks sont tous dans le harnais, avant le
premier `it(`. C'est la formulation la plus stricte qui soit tenable du
critère ①, et elle est **décidable**, contrairement à « les tests sont
inchangés ».

---

## 3. 🔴 Le défaut que cette recette a trouvé : `INTEGER` ne tient pas un `Date.now()`

**Le service ne pouvait pas démarrer du tout sur Postgres, et rien ne le
disait.**

`INTEGER` vaut jusqu'à 8 octets sur SQLite et **exactement 4** sur Postgres.
Toutes les colonnes d'horodatage du socle étaient `INTEGER`, et le service
n'écrit que des `Date.now()` (≈ 1,79 × 10¹²). Mesuré le 19 août 2026 sur
PostgreSQL **16.15** :

```
MIGRATIONS EN ECHEC -> error: value "1787136797072" is out of range for type integer
```

**Le service échouait sur ses PROPRES migrations.** SQLite l'acceptait sans un
mot.

**Pourquoi les deux gardes ne l'ont pas vu, et c'est le fait le plus
réutilisable de P1** : le lint statique est **lexical** — `INTEGER` est un type
parfaitement licite —, et la double passe d'exécution n'écrivait que de
**petites valeurs** (`1_000`), qui tiennent dans quatre octets. **Ce n'est ni le
lint ni la double passe qui manquaient : c'est le CHOIX DES VALEURS.** Une
suite qui n'écrit que des `1_000` déclare portable un schéma qui refuse toute
écriture réelle sur l'un des deux moteurs.

**Mesure de l'angle mort, sur l'arbre d'avant le correctif** (commit
`4183b7e~1`, harnais seul porté à une magnitude d'époque) :

| Moteur | Relevé |
| --- | --- |
| sqlite | `Test Files 11 passed (11)` / `Tests 57 passed (57)` |
| postgres | `Test Files 3 failed \| 8 passed (11)` / `Tests 13 failed \| 44 passed (57)` |

**Le remède est en trois pièces**, commit `4183b7e` :

- les horodatages passent en **`BIGINT`** — dans `0001-socle.sql` **et** dans la
  définition en dur de `migrations.ts` ;
- le harnais de test applique ses migrations à un instant de la **magnitude
  d'une époque** (`INSTANT_MIGRATION = 1_700_000_000_000`), de sorte que **toute
  la suite** exerce désormais la vraie magnitude ;
- **deux gardes neufs, tous deux vus rouges** : le lint refuse un
  `_a INTEGER` (sur la convention de nommage `_a` pour un horodatage), et la
  définition **dupliquée** de `schema_migration` est comparée entre
  `migrations.ts` et `0001-socle.sql`. Cette seconde affirmation — « les deux
  définitions sont à l'identique » — était portée par un commentaire **que rien
  ne vérifiait**, et elle aurait divergé en silence.

⚠️ **Conséquence sur la lecture du journal de critère ③** : la rouge y est
rejouée sur l'arbre d'**aujourd'hui**, qui porte déjà ces gardes ; SQLite y est
donc **rouge de 2 tests** lui aussi. La correction est portée dans le journal
lui-même. **Le schéma fautif est désormais attrapé par deux voies
indépendantes** : le lint, sur les deux moteurs, et l'exécution, sur Postgres
seul.

---

## 4. La compatibilité des pairs d'aujourd'hui, jugée sans VM

Le seul engagement de P1 qu'aucun test unitaire ne couvre est celui du §10.1 :
« l'agent et le client d'aujourd'hui fonctionnent **sans recompilation** ». Il
est jugé par des pairs scriptés qui imitent **les octets exacts** des pairs
réels — `{"role":"agent","session":"bureau"}` d'`agent/src/signaling.rs:59` avec
`SESSION_DE_CONTROLE` d'`agent/src/superviseur/protocole.rs:17`, et
`{"role":"client","session":"bureau"}` de `client/src/shell-page.ts:45` — sur le
**chemin racine**, sans composant de chemin (`compatibilite-pairs-reels.log`).

| Chemin visé | agent | client | offre relayée | lignes `session` |
| --- | --- | --- | --- | --- |
| `/` | ouvert | ouvert | `v=0 pair-reel` | **1** |
| `/signaling` | `ferme: Unexpected server response: 404` | idem | *(aucune)* | **0** |

**La rouge du routage est la seule preuve que le chemin racine est réellement
servi**, et elle est gratuite à jouer. La ligne à 1 sur `/` est par ailleurs une
démonstration **indépendante** du critère ②, avec les octets des pairs réels et
non ceux d'un test.

---

## 5. Les six divergences spec/code, et leur sort

| # | Divergence | Sort |
| --- | --- | --- |
| D1 | « les 483 lignes de tests restent INCHANGÉES » est intenable | **Tranchée, et la mesure la précise encore** : `ice.test.ts` identique au bit près ; `server.test.ts` à **une** ligne (l'import) ; `resilience.test.ts` à **trois** hunks de harnais, **aucune assertion** |
| D2 | renommer `server.ts` en `relais.ts` touche `server.test.ts` | **Tenue** — le renommage est isolé en tâche 4, une seule ligne. ⚠️ **Mais le plan ne l'a pas répercuté sur l'empreinte qu'il attend en tâche 12** (§2 ci-dessus) |
| D3 | `vm.utilisateur_id REFERENCES utilisateur(id)` force P1 à créer `utilisateur` | **Tenue** — la table naît en P1 et **reste vide**. SQLite ne sait pas ajouter une contrainte par `ALTER TABLE` : une clé étrangère naît avec sa table ou n'existe jamais |
| D4 | la table `session` de la spec n'a aucune colonne pour le nom de session | **Tenue** — `nom_session TEXT NOT NULL` ajoutée ; `utilisateur_id` et `vm_id` naissent `NULL` et **ne seront pas resserrés** |
| D5 | `SERIAL` traverse les deux moteurs sans erreur | **Tenue, et ÉLARGIE** : le §3 ci-dessus montre un **troisième** angle mort que ni le lint ni la double passe ne couvraient — le choix des valeurs |
| D6 | « quatre étapes » de `verify-all.sh` en désigne trois | **Tenue** — trois étapes, et le compte de trois est écrit plutôt que le compte de quatre |

---

## 6. Ce que le code livre

| Étage | Fichier | Nature |
| --- | --- | --- |
| lecture d'environnement | `plateforme/src/config.ts` | **PUR** — `PLATEFORME_HOTE` sans aucun défaut |
| serveur HTTP + routage | `plateforme/src/http/serveur.ts` | `noServer`, chemin racine, `listen(port, hote)` |
| la table des sessions | `plateforme/src/signaling/appariement.ts` | **PUR**, générique en `S`, extrait **AVANT** l'addition de P2 |
| le relais | `plateforme/src/signaling/relais.ts` | l'ex-`server.ts`, protocole du fil inchangé |
| la trace en base | `plateforme/src/signaling/trace.ts` | horloge injectée, écriture **jamais attendue** |
| l'interface SQL | `plateforme/src/base/pilote.ts` | **PUR** — `rendreMarqueurs` **lève** sur une littérale |
| les deux pilotes | `pilote-sqlite.ts`, `pilote-postgres.ts` | `node:sqlite`, `pg` — **aucune dépendance native** |
| les migrations | `base/migrations.ts`, `base/migrations/0001-socle.sql` | transactionnelles, tri **numérique** |
| le dépôt | `plateforme/src/depot/session.ts` | horloge injectée, clôture **idempotente** |
| la séquence | `plateforme/src/demarrage.ts` | **le port ne s'ouvre qu'après la base** |

**Tailles relevées PAR LA COMMANDE le 19 août 2026, après la dernière édition
de la ronde.** Le dépôt entier ne porte que **deux** fichiers de plus de
500 lignes, et ce sont les deux lignes de la dette gelée — `agent/src/encode.rs`
**1536** et `agent/src/windows_source.rs` **630**, ni l'un ni l'autre touché par
P1. Les plus gros fichiers de `plateforme/` :

| Fichier | Lignes | Marge |
| --- | --- | --- |
| `plateforme/src/signaling/server.test.ts` | 255 | 245 |
| `plateforme/src/signaling/relais.ts` | 219 | 281 |
| `plateforme/src/signaling/resilience.test.ts` | 181 | 319 |
| `plateforme/src/signaling/trace.test.ts` | 151 | 349 |
| `plateforme/src/base/pilotes.test.ts` | 135 | 365 |
| `plateforme/src/base/sous-ensemble.test.ts` | 125 | 375 |
| `plateforme/src/base/migrations.ts` | 118 | 382 |

**Aucun fichier de `plateforme/` n'approche le plafond.** `relais.ts`, le seul
fichier que P2, P3 et P4 feront grossir, dispose de **281** lignes de marge, et
sa table des sessions en est **déjà sortie**.

---

## 7. Ce que P1 n'établit PAS

- **Aucun taux.** Deux exécutions par critère, jamais une campagne.
- **Aucune latence, aucune charge.** La cible « < 3 s si VM chaude » n'est
  mesurée par aucun critère ; le nombre de sessions simultanées soutenues est
  inconnu.
- **Aucune exécution avec l'agent ou le navigateur réels.** Les pairs sont
  **simulés** — jusqu'aux octets, mais simulés. La corroboration sur VM est
  prévue en fin de P3, hors critère.
- **L'inaccessibilité du service depuis une autre interface.** Ce qui est
  établi : sans `PLATEFORME_HOTE` le service ne démarre pas, et avec, il écoute
  sur cette adresse. Ce qui ne l'est **pas** : qu'il soit injoignable ailleurs —
  sur une machine de développement, `127.0.0.1` et l'adresse de l'interface sont
  toutes deux locales, et la sonde exigerait une machine hors du réseau. **Le
  critère ④ ne porte donc que sur la moitié de son nom.**
- **Le comportement de Postgres sous charge, en concurrence, ou après
  redémarrage** : la passe `test:postgres` éprouve un **dialecte**, pas un
  déploiement. C'est le critère ① de P5.
- **Aucune authentification.** Le port, s'il est atteint, délivre toujours des
  identifiants TURN valables 86 400 s à quiconque. **C'est P2**, et le critère ④
  est ce qui rend cette fenêtre tolérable — raison pour laquelle il est en P1 et
  non en P5.
  > ⚠️ **ANNOTÉ le 19 août 2026, à la revue transverse du sous-bloc P2 — cet
  > énoncé reste VRAI COMME RELEVÉ DE P1, et il n'est PAS réécrit.** Il n'est
  > plus vrai que **de moitié** de l'état du dépôt : la garde de P2
  > (`plateforme/src/identite/garde.ts`) refuse un pair de rôle **`client`**
  > sans jeton — motif `jeton-absent`, socket fermé 1008 — et ne lui envoie
  > aucun `ice-config`. Un pair qui se déclare **`{"role":"agent"}` reste
  > accepté sans aucune identité** et reçoit ses identifiants TURN de 86 400 s,
  > l'agent Rust n'ayant pas d'identité avant **P3** ; c'est mesuré sur le
  > service de P2 (`journaux-plateforme-p2/e2-role-agent-toujours-anonyme.log`).
  > **Le critère ④ de P1 reste donc ce qui borne cette moitié-là de la fenêtre.**

- **La scalabilité horizontale** : la persistance ne la procure pas. Un
  WebSocket vit dans un processus et un seul.
- **Aucune constante calibrée** : ni `DUREE_SECONDES = 86_400`, ni le port par
  défaut, ni les bornes de temps des tests, ni `INSTANT_MIGRATION`.
- **Aucune cause NATURELLE de perte d'écriture n'a été observée.** L'écriture de
  la trace est délibérément non attendue, et son `.catch` ne journalise que ;
  **ce chemin d'échec n'a jamais couru** en recette.
- **Le balayage de démarrage MENT** sur les sessions qui ont réellement survécu
  à l'arrêt du service : le flux WebRTC ne dépend plus du signaling une fois
  l'offre et la réponse échangées. Limite nommée, non corrigée.
- **La course entre `apparie` et `separe`** est fermée par un enchaînement de
  promesses, **jamais éprouvée sous concurrence réelle**.
- **`ExperimentalWarning`** : `node:sqlite` est expérimental sur Node 24. Un
  changement de majeure impose de rejouer la suite avant tout autre travail.
- **La portée exacte de `BIGINT`** au-delà des cinq colonnes du socle : toute
  colonne d'horodatage future qui ne porterait pas un nom en `_a` échapperait au
  lint.

---

## 8. Renvoi vers chaque journal versé

| Fichier | Ce qu'il porte |
| --- | --- |
| `sqlite-1.log`, `sqlite-2.log` | les deux exécutions de `npm run test:sqlite` |
| `postgres-1.log`, `postgres-2.log` | les deux exécutions de `npm run test:postgres` |
| `typecheck-1.log`, `typecheck-2.log` | les deux exécutions de `tsc --noEmit` |
| `verify-all.log` | la vérification d'ensemble, sortie 0 |
| `verify-all-final.log` | la **même, rejouée APRÈS la dernière édition de la ronde** — c'est elle qui porte les chiffres de `CLAUDE.md` |
| `verify-all-rouge-postgres-absent.log` | la même, instance Postgres arrêtée — **elle ÉCHOUE** |
| `critere-1-empreintes.log` | empreintes, et les trois `diff` contre l'original |
| `critere-1-rouges.log` | les deux rouges du déménagement |
| `critere-2-rouges.log` | les deux rouges de la trace de session |
| `critere-3-double-passe.log` | la rouge de magnitude, **et sa propre correction** |
| `critere-4-rouge.log` | la rouge du défaut d'écoute, et le refus réel du service |
| `compatibilite-pairs-reels.log` | les octets des pairs réels, et la rouge du routage |
