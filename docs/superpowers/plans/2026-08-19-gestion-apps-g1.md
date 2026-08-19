# Sous-bloc G1 — le catalogue naît, et on peut lancer ce qu'il contient : plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** remplacer le balayage horaire du Bureau et le parseur `.lnk` maison par
une **réconciliation périodique** qui lit les quatre racines de raccourcis par
`IShellLinkW`, en dérive un catalogue identifié par le triplet
`(cible, arguments, répertoire)`, le pousse à la plateforme par le canal
`/agent`, et — parce qu'un catalogue qu'on ne peut pas lancer n'est pas un
livrable (spec D6) — accepte en retour un ordre de lancement honoré par
`ShellExecuteExW` sur le raccourci lui-même.

**Architecture:** quatre étages, et la coupure pur / `#[cfg(windows)]` est
décidée par la spec §6, pas à l'implémentation.

1. **Trois modules PURS dans l'agent** — la normalisation et la clé d'identité
   (D4 de la spec), la règle de filtrage (D3), et le **diff de réconciliation**
   (D1). Ils se testent sur l'hôte Linux, contre un **corpus versé** des
   raccourcis réels de la VM.
2. **Deux modules `#[cfg(windows)]`** — la lecture COM d'un raccourci, et le
   lancement. Ils ne se vérifient que par
   `cargo check --target x86_64-pc-windows-gnu`.
3. **Le canal `/agent` devient BIDIRECTIONNEL.** Il ne l'est pas aujourd'hui :
   `Canal` (`agent/src/plateforme.rs:59-65`) n'expose qu'`attendre_identite`, et
   aucun message entrant n'a de chemin vers le reste de l'agent (E2).
4. **La plateforme gagne un registre de sockets d'agent vivants**, une table
   `application` élargie, une fusion **pure** de catalogue, et deux routes HTTP.

**Tech Stack:** inchangée — Rust/`tokio-tungstenite` côté agent,
Node/TypeScript, Vitest, `ws`, `node:sqlite`, `pg` côté plateforme.
🔴 **G1 N'AJOUTE AUCUNE DÉPENDANCE de production**, ni en TypeScript ni en Rust.
Le contrôle `plateforme/src/base/pilote.test.ts:48-49`
(`expect(deps).toEqual(['pg', 'ws'])`) reste **inchangé** et doit rester
**vert** : c'est le témoin de cette propriété. Côté Rust, la seule addition à
`agent/Cargo.toml` est **une feature du crate `windows` déjà présent** (E10) —
aucune nouvelle ligne de dépendance.

**Spec :** `docs/superpowers/specs/2026-08-19-gestion-apps-design.md` (commit
`dded5b5`), §3, §4 (D1 à D4, D6, D10), §5 « G1 », §6, §7, §8.
**Amont qui fait autorité sur l'état du code :**
`docs/superpowers/plans/2026-08-19-plateforme-p3.md` et le code lui-même —
la spec ④ a été écrite pendant que P3 se terminait, et plusieurs de ses
affirmations sur l'existant sont à reprendre (voir « Divergences »).

**Ce plan ne couvre QUE G1.** Rien des icônes (G2) : aucune extraction
`IShellItemImageFactory`, aucun `source_max_px`, aucun `GRPICONDIR`, aucun
magasin adressé par contenu, aucun `IconesManquantes`. Rien du téléversement ni
de l'exécution d'un installeur (G3) : aucune route `/televersement`, aucun
`Installer`, `Progression` ni `Termine`, aucune table `installation`. Rien de
`ReadDirectoryChangesW` ni de l'anti-rebond (G4). Rien de la PWA par
application ni des `file_handlers` (G5).

---

## Contraintes globales

### Ce qui a été relevé PAR LA COMMANDE avant d'écrire une ligne

Toutes les valeurs ci-dessous ont été obtenues le **19 août 2026**, sur cette
machine, en lançant réellement la commande. Aucune n'est recopiée d'un document
antérieur. **Ce qui n'a pas été mesuré est marqué comme tel partout ailleurs
dans ce plan.**

| # | Commande | Résultat relevé |
| --- | --- | --- |
| ① | `cd proto && npm test` | `Test Files 3 passed (3)`, `Tests 70 passed (70)` |
| ② | `cd proto && npm run typecheck` | sortie **0** |
| ③ | `cd plateforme && npm run test:sqlite` | `Test Files 28 passed (28)`, `Tests 194 passed (194)` |
| ④ | `cd plateforme && npm run test:postgres` | `Test Files 28 passed (28)`, `Tests 194 passed (194)` |
| ⑤ | `cd plateforme && npm run typecheck` | sortie **0** |
| ⑥ | `cd client && npm test` | `Test Files 21 passed (21)`, `Tests 187 passed (187)` |
| ⑦ | `cd client && npm run typecheck` | sortie **0** |
| ⑧ | la commande des 500 lignes de `CLAUDE.md` | **deux** fichiers au-dessus du plafond : `agent/src/encode.rs` **1536**, `agent/src/windows_source.rs` **630** |
| ⑨ | `grep -c '^etape "' scripts/verify-all.sh` | **10** |

⚠️ **Les relevés ① à ⑦ sont les références de non-régression de G1.** Toute
tâche qui les fait baisser a cassé quelque chose ; toute tâche qui les fait
monter doit dire **de combien et pourquoi**, et **l'annoncer AVANT de lire le
compte** — c'est ainsi que D10 a rattrapé un test supprimé par un `Write`
d'écrasement.

🔴 **`cargo test --workspace` et `cargo clippy --workspace` N'ONT PAS ÉTÉ LANCÉS
pour établir cette référence**, et c'est déclaré plutôt que dissimulé : un
chantier concurrent conduisait la recette de P3 dans `agent/` et sur la VM au
moment de la rédaction, et un compte de tests Rust relevé alors ne serait
attribuable ni à lui ni à G1. **La première tâche qui touche `agent/` ou
`proto/src/` doit relever cette référence elle-même, sur un arbre dont elle
nomme le commit.** Piège hérité de D11 et repayé par P2
(`temoin-cargo-arbre-propre.log`).

⚠️ **`scripts/verify-all.sh` compte DIX étapes, pas neuf.** Le plan de P3 écrit
« ses neuf étapes » (§ « Structure des fichiers ») ; la commande ⑨ ci-dessus en
rend **dix** — `client : npm run design:verifier` (`scripts/verify-all.sh:73`)
s'est ajoutée avec le sous-projet ⑥. **`verify-all.sh` n'est PAS modifié par
G1** : ses dix étapes couvrent déjà `proto/` (⑥, ⑦), `plateforme/` (⑧, ⑨, ⑩),
`client/` (③, ④, ⑤) et `agent/` (①, ②). Relevé, pas supposé.

### Mesures de base de données prises pour ce plan, et elles décident du schéma

Lancées le 19 août 2026 sur **SQLite 3.50.4** (`node:sqlite`) et sur
**PostgreSQL 16.15** (l'instance de `docker-compose.plateforme.yml`, schéma
jetable), sur une table `application` de la forme exacte de
`plateforme/src/base/migrations/0003-agents.sql:52-58` :

| Geste | SQLite 3.50.4 | PostgreSQL 16.15 |
| --- | --- | --- |
| `ADD COLUMN TEXT NOT NULL` **sans DEFAUT**, table **VIDE** | **OK** | **OK** |
| `ADD COLUMN TEXT NOT NULL` **sans DEFAUT**, table **NON VIDE** | 🔴 **REFUSÉ** — `Cannot add a NOT NULL column with default value NULL` | non mesuré |
| `ADD COLUMN TEXT` (nullable) | OK | OK |
| `ADD COLUMN BIGINT` (nullable) | OK | OK |
| `ADD COLUMN INTEGER NOT NULL DEFAULT 0` | OK | non mesuré |
| `ADD COLUMN TEXT UNIQUE` | 🔴 **REFUSÉ** — `Cannot add a UNIQUE column` | **OK** |
| `CREATE UNIQUE INDEX … ON application(vm_id, cle)` | **OK** | **OK** |
| `DROP TABLE` puis `CREATE TABLE` | OK | OK |

**Ces deux refus commandent D5 et E7/E8**, et **aucun des deux n'était mesuré
par la spec**, dont le §3.3 ne relève que `ADD COLUMN nullable`,
`ADD COLUMN NOT NULL DEFAULT` et `ADD COLUMN avec REFERENCES`.

⚠️ **Une exécution de chaque, sur une base neuve.** Aucun taux.

### La règle des 500 lignes : UNE extraction est requise, et elle précède son addition

**Relevé par la commande le 19 août 2026** (`{ git ls-files; git ls-files
--others --exclude-standard; } | grep -vE
'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' |
xargs wc -l | sort -rn`), pour les seuls fichiers que G1 modifie ou qui bordent
son périmètre :

| Fichier | Lignes | Marge | Ce que G1 y ajoute | Porte |
| --- | --- | --- | --- | --- |
| 🔴 `proto/src/plateforme.rs` | **410** | **90** | trois variantes, deux types, leurs constructeurs, **un test de version par variante entrante**, et les cas de vecteurs | **EXTRACTION OBLIGATOIRE, tâche 1, AVANT la tâche 2** |
| `proto/ts/plateforme.test.ts` | **267** | 233 | les tests des variantes neuves (~120) | porte à **450**, tâche 3 |
| `plateforme/src/agents/canal.test.ts` | **361** | 139 | les branches neuves (~120) | 🔴 **franchira 450** — extraction prévue, tâche 17 |
| `plateforme/src/http/routes-auth.test.ts` | **215** | 285 | rien — G1 crée `routes-applications.test.ts` | aucune |
| `agent/src/plateforme.rs` | **277** | 223 | la file d'émission et la réception des ordres (~90) | porte à **450**, tâche 10 |
| `agent/src/main.rs` | **435** | **65** | le branchement d'`apps` (~20) | ⚠️ **à re-mesurer avant la tâche 11** ; si l'addition dépasse 15 lignes, elle descend dans `agent/src/apps.rs` |
| `plateforme/src/agents/canal.ts` | **198** | 302 | deux branches (~70) | aucune |
| `plateforme/src/http/serveur.ts` | **169** | 331 | le chaînage de la route (~20) | aucune |
| `plateforme/src/http/cors.ts` | **39** | 461 | `GET` (~2) | aucune |
| `plateforme/src/base/migrations/0003-agents.sql` | **58** | — | **rien** — G1 crée `0004` | aucune |
| `plateforme/src/base/pilotes.test.ts` | **141** | 359 | deux lignes réécrites (E9) | aucune |
| `agent/src/superviseur/lanceur.rs` | **367** | 133 | **rien** — G1 ne lance pas par le lanceur (D14) | aucune |
| `proto/src/control.rs` | **470** | **30** | **rien** — G1 ne touche pas le canal agent↔navigateur | aucune, et c'est une décision |
| `agent/src/transport.rs` | **495** | **5** | rien | aucune |
| `agent/src/encode/arret.rs` | **500** | **0** | rien | aucune |
| `agent/src/demarrage.rs` | **491** | **9** | rien | aucune |
| `client/verify-webrtc.mjs` | **494** | 6 | rien | aucune |

🔴 **`proto/src/plateforme.rs` est à 410 lignes, marge 90, et G1 doit y écrire
beaucoup.** La spec §6 le nomme déjà comme le fichier à surveiller et exige
« l'extraction précède l'addition, sans exception ». **La tâche 1 est donc une
tâche à part entière, sans addition d'aucune sorte, et elle précède la
tâche 2.** Une compression de commentaire pour repasser sous la ligne est
**interdite** — `CLAUDE.md` le dit nommément, et D9 l'a payée deux fois dans le
même sous-bloc.

⚠️ **Porte chiffrée, à jouer après CHAQUE tâche qui touche un fichier
existant** : relancer `wc -l` sur les fichiers de la table ci-dessus. Si l'un
d'eux dépasse **450**, extraire **avant** de continuer, jamais après. Le seuil
est à 450 et non à 500 pour la raison ci-dessus : D10 a franchi le plafond
**trois fois** en une branche, et n'a jamais été rattrapé que par une
extraction.

⚠️ **`plateforme/` EST DÉJÀ au § « Portée » de `CLAUDE.md`** — ligne **20**,
relue : `` `agent/src/`, `client/src/`, `plateforme/`, `proto/`, `src/`, `web/`,
`scripts/`. `` La spec §6 écrit qu'il « doit entrer » : **c'est fait**, et il n'y
a rien à faire de ce côté (E1).

### Les règles de méthode, héritées et non négociables

- **Jamais `git add -A`** : nommer les fichiers, un par un. Un `git add -A` a
  déjà emporté le travail concurrent d'une autre tâche dans un commit qui ne
  compilait pas. 🔴 **Et un autre chantier travaille dans le même arbre git.**
- **Jamais `git commit --amend`.** `git commit` valide **tout l'index** :
  pathspec explicite obligatoire.
- **Écrire le test d'abord, et le VOIR ROUGE.** Un contrôle qu'on n'a jamais vu
  rouge n'est pas un contrôle. Ce dépôt a payé quatre fois pour un contrôle
  incapable d'échouer — F1 de D7, la sonde P1 de D8, le confondeur de D9, le
  budget par fil de D10 — et **trois des quatre étaient écrits par un plan**.
  Pour chaque contrôle prescrit ci-dessous, la colonne « ce qui le rend ROUGE »
  nomme un état **atteignable**, et la tâche doit l'atteindre réellement.
- **Annoncer le compte de tests attendu AVANT de le lire.** C'est ainsi que D10
  a retrouvé un test écrasé par un `Write`.
- **Aucun numéro de ligne recopié sans être relu.** C'est le naufrage du « 487 »,
  payé neuf fois. Les numéros de ce plan ont été relus le 19 août 2026 ; **ils
  dériveront**.
- **Aucune sortie de commande fabriquée.** Deux pièces l'ont été en D10 et
  présentées comme des relevés ; le fait rapporté était vrai les deux fois, la
  preuve ne l'était pas.
- **Les valeurs d'horodatage des tests de base sont d'une magnitude d'ÉPOQUE**,
  jamais `1_000`. `plateforme/src/base/pilotes.test.ts:100-140` existe
  précisément parce que la double passe **n'écrivait que de petites valeurs**, et
  déclarait portable un schéma qui refuse toute écriture réelle. Constante à
  employer : **`1_787_136_773_742`**.
- **Une passe SQL qui manque N'EST PAS sautée.** `plateforme/src/base/harnais.ts:4-9`
  l'écrit : ni `it.skipIf`, ni `describe.skip`, ni `if (!disponible) return`.

### Périmètre concurrent — à lire avant de toucher `agent/`, `proto/` ou la VM

Au moment d'écrire ce plan, un chantier concurrent conduisait la **recette de
P3** : il travaillait dans `agent/`, rebâtissait le binaire, et **tenait la VM
Windows**. Conséquences opératoires :

- ⛔ **relancer `git log --oneline -5 -- proto/ agent/` avant de démarrer la
  famille 0 ou la famille 2** ;
- ⛔ **la tâche 20 (recette) est la SEULE qui emploie la VM** ; ne pas la démarrer
  tant qu'un autre chantier la tient. `virsh list --all` d'abord ;
- ⛔ **ne pas lancer `cargo test --workspace` en parallèle d'un `build-agent.sh`
  concurrent** : le verrou de cargo et le partage CIFS produisent des
  diagnostics sans rapport avec la cause.

---

## Décisions tranchées

### D1 — L'extraction de `proto/src/plateforme.rs` est celle de son MODULE DE TESTS, par `#[path]`

Le fichier fait **410** lignes, dont le module `#[cfg(test)] mod tests`
occupe **les lignes 167 à 410**, soit **244** — près de 60 % du fichier. Les 166
premières sont l'en-tête, `PLATEFORME_VERSION`, `verifie_version`, `MotifCanal`
et les deux énumérations avec leurs constructeurs.

**On ne peut pas scinder les énumérations** : `VersLaPlateforme` et
`DepuisLaPlateforme` sont chacune **une** `enum` avec `#[serde(tag = "type",
deny_unknown_fields)]`. Séparer « cycle de vie » et « gestion d'apps » en deux
modules exigerait deux énumérations, donc deux tags, donc deux protocoles —
c'est un changement de conception, pas une extraction.

**Décision : déplacer le module de tests VERBATIM vers
`proto/src/plateforme/tests.rs`, déclaré par
`#[cfg(test)] #[path = "plateforme/tests.rs"] mod tests;`.** C'est exactement le
mécanisme qu'`agent/src/superviseur/table.rs` emploie déjà, et c'est le cas que
le § « Convention de module enfant » de `CLAUDE.md` range **explicitement hors
de sa portée** : « l'usage de `#[path]` pour scinder un module de *tests* trop
long À L'INTÉRIEUR d'un fichier par ailleurs portable […] c'est le même
mécanisme Rust, employé pour une raison différente (la règle des 500 lignes), et
il ne suit pas la convention ci-dessous. »

🔴 **UN DÉTAIL QUI CASSE LA COMPILATION SI ON LE MANQUE, et il est nommé
d'avance** : le test `conformite_aux_vecteurs_partages` porte
`include_str!("../plateforme-vectors.json")` (`proto/src/plateforme.rs`, dans le
module de tests). `include_str!` résout **relativement au fichier qui le
contient** : une fois le module descendu d'un niveau, le chemin devient
`"../../plateforme-vectors.json"`. **L'échec est bruyant, pas muet** — c'est une
erreur de compilation —, ce qui est la seule raison pour laquelle ce détail est
un avertissement et non un risque.

Après extraction : `plateforme.rs` ≈ **166** lignes (marge ≈ 334),
`plateforme/tests.rs` ≈ **244** (marge ≈ 256). La tâche 2 y ajoute
respectivement ~120 et ~200 : les deux restent sous 500. **Si `tests.rs`
dépassait 450**, il se scinde en `plateforme/tests_cycle.rs` et
`plateforme/tests_apps.rs`, deux `#[path]` de plus, sans rien compresser.

### D2 — TROIS variantes neuves, et pas une de plus

La spec D10 énumère sept messages pour tout le sous-projet ④ : `Catalogue`,
`Installer`, `Progression`, `Termine`, `Lancer`, `Lancee`,
`IconesManquantes`. **G1 n'en livre que trois** — `Catalogue` (montant),
`Lancer` (descendant), `Lancee` (montant). Les quatre autres appartiennent à G2
et G3, et les écrire d'avance produirait des variantes que rien n'exerce, donc
du code mort dans un protocole versionné.

⚠️ **Conséquence de coût, assumée** : G2 et G3 rebumperont `PLATEFORME_VERSION`
à 3 puis 4, et redéploieront agent et plateforme ensemble à chaque fois. C'est
le prix de `deny_unknown_fields` + vérification de version **par variante**
(`proto/src/plateforme.rs:42-53`), et il est préférable à un protocole dont les
trois quarts ne sont exercés par rien.

### D3 — Le catalogue voyage en DIFF, avec un drapeau `complet` posé à chaque (ré)enrôlement

La réconciliation produit un diff (spec D1). Le message porte donc les
**apparues et modifiées** et les **clés disparues**, jamais les 154 lignes à
chaque tour.

```
Catalogue { v, complet: bool, applications: Vec<Application>, disparues: Vec<String> }
```

**`complet = true` a une sémantique NOMMÉE, et c'est la seule qui rende l'état
de la plateforme reconstructible** : la plateforme marque `disparue_a` sur
**toute** ligne de cette VM absente de `applications`, et ignore `disparues`.
`complet = false` applique le delta.

🔴 **L'agent émet `complet = true` à chaque (ré)enrôlement**, c'est-à-dire à
chaque ouverture de session du canal (`agent/src/plateforme.rs:138`
`une_session`). **C'est ce qui rend la perte d'un message montant sans
conséquence** : le canal est un `push` WebSocket, sans garantie de livraison —
la spec D7 le dit du sens descendant, et c'est symétrique. Sans ce renvoi
complet, un `Catalogue` perdu pendant une coupure laisserait la plateforme
divergente **sans terme**.

⚠️ **Le coût est nommé** : un renvoi complet pèse 154 entrées. À ~300 octets
l'entrée, ~46 Kio — **calculé, jamais mesuré**, et à mesurer en recette
(tâche 20, relevé annexe).

### D4 — `Lancer { demande, cle }`, et l'issue NOMME le chemin emprunté

**L'ordre ne porte pas le chemin du raccourci.** Il porte la clé d'identité, et
l'agent la résout dans **son propre** catalogue — celui qu'il vient de lire sur
le disque. La copie de la plateforme peut être vieille d'une réconciliation ;
celle de l'agent ne l'est jamais.

```
Lancer { v, demande: String, cle: String }
Lancee { v, demande: String, issue: IssueLancement }
IssueLancement = Raccourci | Cible | Inconnue | Echec
```

🔴 **`Raccourci` contre `Cible` est ce qui rend le critère ⑤ DÉCIDABLE, et la
spec dit elle-même qu'il ne l'est pas sans cela** : « lancer par la cible
reconstruite au lieu du `.lnk` **passerait ce critère-ci** : c'est pourquoi ⑥
existe ». Le critère ⑥ (le répertoire de travail) reste, et ⑤ gagne en plus
l'assertion `issue = raccourci`. **Deux contrôles indépendants au lieu d'un
seul plus un contournement.**

`Inconnue` — la clé n'est dans aucun catalogue de l'agent. `Echec` — le
raccourci **et** la cible ont échoué ; les deux tentatives sont journalisées
(spec D6), et l'issue est typée, jamais un silence (spec §7).

⚠️ **`MotifCanal` n'est PAS réemployé pour cela.** Il décrit le canal lui-même,
et deux de ses valeurs **ferment le socket** (`plateforme/src/agents/canal.ts:73`
`MOTIFS_FERMANTS = ['enrolement', 'version']`). Un lancement raté ne doit
fermer aucun canal.

**La route HTTP attend `Lancee`, bornée.** `DELAI_LANCEMENT_MS` (5 000 proposé,
**NON CALIBRÉE**) : au-delà, la route rend `{ refus: 'delai' }`. Un
« tiré et oublié » qui rendrait `202` ferait passer le critère ⑤ sur un binaire
qui n'a rien lancé.

### D5 — `0004-applications.sql` procède par `ALTER TABLE ADD COLUMN` puis `CREATE UNIQUE INDEX`, jamais par `DROP`/`CREATE`

Les mesures du § « Mesures de base de données » commandent la forme :

- `ADD COLUMN … NOT NULL` **sans DEFAUT** passe des deux côtés **parce que la
  table est vide** — et `plateforme/src/base/migrations/0003-agents.sql:43-44`
  garantit qu'elle l'est : « `application` est creee par P3 et **RESTE VIDE** » ;
- un `DEFAULT` littéral est **impossible** pour une colonne TEXT :
  `plateforme/src/base/pilote.ts:32-39` (`rendreMarqueurs`) **lève** sur toute
  apostrophe ou guillemet, et `plateforme/src/base/sous-ensemble.test.ts:87`
  (`expect(sql).not.toMatch(/['"]/)`) le refuse statiquement. Un
  `DEFAULT ''` est donc rouge **avant** d'atteindre le moteur ;
- `ADD COLUMN … UNIQUE` est **refusé par SQLite** : l'unicité passe par un
  `CREATE UNIQUE INDEX`, accepté des deux côtés.

**L'alternative `DROP TABLE application; CREATE TABLE application (…)` a été
mesurée et fonctionne des deux côtés. Elle est ÉCARTÉE**, pour une raison et
une seule : elle rendrait le comportement de la migration dépendant du fait que
la table soit vide **au moment de l'exécution chez le lecteur**, et un `DROP`
sur une table peuplée détruirait un catalogue sans rien dire. `ADD COLUMN`, lui,
**échoue bruyamment** sur une table peuplée (mesuré : `Cannot add a NOT NULL
column with default value NULL`). **Entre deux gestes qui supposent la même
chose, on prend celui qui crie quand la supposition est fausse.**

**Colonnes ajoutées** (aucune clé étrangère — leg n°2 de P1 : une contrainte
naît avec sa table ou n'existe jamais) :

| Colonne | Type | Nullable | Pourquoi |
| --- | --- | --- | --- |
| `cle` | TEXT | NOT NULL | l'empreinte du triplet (spec D4) |
| `cible` | TEXT | NOT NULL | le chemin de la cible, normalisé |
| `arguments` | TEXT | NOT NULL | **bruts**, sensibles à la casse (spec D4). Vide = `''`, jamais NULL |
| `repertoire` | TEXT | NOT NULL | le répertoire de travail, normalisé |
| `apparue_a` | BIGINT | NOT NULL | première vue |
| `disparue_a` | BIGINT | NULL | spec D1 : **posée, jamais supprimée** |
| `masquee_a` | BIGINT | NULL | spec D3 : le geste explicite qui masque un désinstalleur |

plus `CREATE UNIQUE INDEX application_cle ON application(vm_id, cle)`.

🔴 **`apparue_a` est ÉCRITE PAR G1 ET LUE PAR PERSONNE avant G3**, et c'est
déclaré plutôt que découvert. Elle naît maintenant parce qu'une colonne
**NOT NULL** ne peut plus être ajoutée une fois la table peuplée — mesuré
ci-dessus —, et que le verdict d'installation de la spec D9 (« `reussie` quand la
réconciliation qui la suit rapporte au moins une application apparue ») en aura
besoin. Précédent explicite du dépôt : `plateforme/src/agents/fraicheur.ts:11-20`,
un module pur sans appelant de production, **acceptable parce que déclaré tel**.

⚠️ **`masquee_a` est écrite par personne en G1** : aucun geste de masquage
n'existe (aucun critère ne le juge, et le hub est G5). Elle naît pour la même
raison — sauf qu'elle est **nullable**, donc elle **pourrait** naître plus tard.
Elle naît quand même, pour ne pas fragmenter le schéma d'une même table en deux
migrations. **C'est le seul point de ce paragraphe qui soit une commodité et non
une contrainte, et il est marqué comme tel.**

⚠️ **Le lint `\b\w+_a\s+INTEGER\b` de `sous-ensemble.test.ts:46` couvre les
trois nouvelles colonnes `_a`** — elles sont BIGINT, il ne peut donc rougir. Il
ne couvrirait **pas** une colonne d'horodatage nommée autrement ; il n'y en a
aucune ici.

⚠️ **Le fichier `0004-applications.sql` ne peut porter ni apostrophe ni
guillemet, commentaires compris.** `0003-agents.sql` est **entièrement
désaccentué** pour cela — relu. Écrire les commentaires de `0004` de la même
façon.

### D6 — Les quatre racines viennent de `SHGetKnownFolderPath`, jamais de chemins littéraux

La spec §3.1 énumère quatre chemins mesurés sur **une** machine, dont
`C:\Users\Administrateur\Desktop` — le profil de l'administrateur de cette VM.
Les coder en dur rendrait la découverte fausse sur toute autre installation, et
muette à ce sujet.

**Décision :** les quatre racines sont résolues par `SHGetKnownFolderPath` sur
`FOLDERID_Desktop`, `FOLDERID_PublicDesktop`, `FOLDERID_StartMenu`,
`FOLDERID_CommonStartMenu`. **Les quatre constantes existent dans le crate
`windows` 0.62.2** — relues aux lignes **8692**, **8743**, **8786** et **8681**
de `Windows/Win32/UI/Shell/mod.rs`, et `SHGetKnownFolderPath` à la **3301**.

Parcours **récursif** (`bWatchSubtree` de G4 le sera aussi), extension `.lnk`,
insensible à la casse.

⚠️ **Une racine qui échoue à se résoudre est SAUTÉE AVEC SA TRACE, jamais
fatale** — spec §7 : « Une réconciliation qui échouerait en entier sur un
fichier ferait disparaître tout le catalogue. » Idem d'un répertoire absent :
`FOLDERID_StartMenu` peut ne pas exister sur un profil neuf.

### D7 — L'existence de la cible est un PRÉDICAT INJECTÉ, ce qui rend toute la règle de filtrage pure

La troisième règle de la spec D3 est « le fichier cible existe ». Écrite avec un
`std::path::Path::exists()` en dur, elle rendrait la règle **impure** et donc
non testable sur l'hôte — où aucune des 167 cibles n'existe.

**Décision :** `retenir(raccourci, existe: &dyn Fn(&str) -> bool) -> bool`.
C'est la même figure que l'horloge en paramètre de
`plateforme/src/agents/fraicheur.ts` et de `identite/jeton.ts`, et elle achète
exactement ce que la spec §8 exige : **la règle de filtrage se teste sur
l'hôte**.

🔴 **Et elle achète la rouge GRATUITE du critère ①.** La tâche 5 verse
`agent/testdata/gapps-corpus-vm.json` — le dépouillement des **218** raccourcis
de la VM, chacun avec son nom, sa cible, ses arguments, son répertoire et un
booléen `existe` **relevé sur la VM**. Le corpus est un `testdata/`, donc
**exempté de la règle des 500 lignes** par le § « Exemptions explicites » de
`CLAUDE.md`. Le test d'hôte assène alors les trois chiffres de la spec §3.1 :
**218 lus, 167 retenus, 154 clés distinctes, 104 par la cible seule.**

⚠️ **Le corpus DOIT être produit par l'agent lui-même**, pas par la sonde
PowerShell de la spec — sinon il ne mesure pas ce que le produit lit (E16). La
tâche 20 le produit ; la tâche 5 est **écrite contre les chiffres de la spec** et
**rejouée contre le corpus réel** en fin de branche. Si les deux divergent,
**l'écart est une mesure, pas un bug à aplatir**, et il se déclare.

### D8 — Le registre des sockets d'agent vivants est un objet de service, sur le modèle de `ProprieteDeSession`

`plateforme/src/agents/canal.ts` tient son état **dans la fermeture de la
connexion** (`vmId`, `prefixe`, l. 87-88). Il n'existe **aucun moyen de
retrouver le socket d'une VM donnée** (E4), et `POST /application/:id/lancer` en
a besoin.

**Décision :** `plateforme/src/agents/registre.ts` — une classe `RegistreAgents`
construite **une fois** dans `demarrerServeur` et passée à `servirLeCanalAgent`
et aux routes. C'est mot pour mot le patron de `ProprieteDeSession`
(`plateforme/src/signaling/propriete.ts`, 46 lignes, construite à
`plateforme/src/http/serveur.ts:97`), **y compris son coût, qui est le même et
doit être écrit au même endroit : il ne survit pas à un redémarrage.**

Il porte deux choses : la table `vmId → socket`, et les **demandes en vol**
`demande → resolveur`. Une connexion qui se ferme se retire du registre **et
rejette ses demandes en vol** avec `{ refus: 'agent-injoignable' }` — sans quoi
la route attendrait `DELAI_LANCEMENT_MS` pour rien.

⚠️ **Une VM peut apparaître deux fois** (un agent relancé avant que l'ancien
socket ne se ferme). Décision : **le dernier enrôlement gagne**, l'ancien socket
est fermé avec `FERMETURE_POLITIQUE`. La raison est celle de la clé primaire
d'`agent_enrole` (`0003-agents.sql:18-20`) : « une VM porte au plus un agent, et
deux lignes pour la meme VM n'auraient aucun sens — laquelle serait la bonne ? »

### D9 — Les deux routes exigent un jeton `utilisateur`, et l'isolation entre utilisateurs N'EXISTE PAS encore

Il n'existe **aucune authentification HTTP** dans la plateforme (E5) : les deux
routes de `routes-auth.ts` sont ouvertes par construction, et il n'y a ni
extraction de jeton porteur, ni cookie, nulle part.

**Décision :** les deux routes de G1 lisent un jeton dans
`Authorization: Bearer <jeton>` et appellent
`verifierJeton(jeton, secret, maintenant)`
(`plateforme/src/identite/jeton.ts:130`), puis exigent
`verdict.type === 'utilisateur'` — le claim `sty` de P3
(`plateforme/src/identite/jeton.ts:76`).

🔴 **La garde du relais (`identite/garde.ts`) N'EST PAS réutilisable ici**, et
c'est structurel : sa signature est `verifier({role, session, jeton})`
(`garde.ts:69`), taillée pour une poignée de main WebSocket, et elle porte le
registre d'appartenance. Le précédent d'un consommateur direct de
`verifierJeton` est `agents/canal.ts`, dont l'en-tête (l. 9-13) explique
pourquoi il ne partage rien avec la garde.

🔴 **L'ISOLATION ENTRE UTILISATEURS N'EST PAS LIVRÉE, et c'est déclaré plutôt
que dissimulé.** `vm.utilisateur_id` (`0001-socle.sql:49`) est **NULL après
`npm run admin:agent`** — relu : `enrolerLaVm` fait
`INSERT INTO vm(id, nom, adresse)` (`plateforme/src/admin/enroler-agent.ts`) et
ne passe jamais d'utilisateur. L'attribution d'une VM à un utilisateur est **P4**.

**Comportement retenu, qui se durcit tout seul le jour où P4 remplit la
colonne** :

| `vm.utilisateur_id` | Réponse |
| --- | --- |
| NULL | **servie**, et **journalisée** : `vm non attribuee, acces accorde sans isolation (attribution = sous-bloc P4)` |
| égal au sujet du jeton | servie |
| différent | `403 { refus: 'vm-etrangere' }` |

⚠️ **Ce n'est pas une isolation** : tant qu'aucune VM n'est attribuée, tout
utilisateur authentifié voit toutes les VMs. **La ligne de journal est ce qui
rend l'état visible**, et le legs le nomme.

### D10 — `PLATEFORME_VERSION = 2` : rupture assumée, et les trois tâches de la famille 0 sont INDIVISIBLES

C'est la décision D10 de la spec, et c'est la décision D5 de P3 rejouée
(« OUI, P3 CASSE l'agent déjà déployé, et c'est une décision »). Un agent v1 et
une plateforme v2 ne se parlent pas, et le refus `version` **ne se réessaie
pas** (`proto/src/plateforme.rs`, en-tête l. 24-28 ; `agent/src/plateforme.rs:251-267`
`sur_refus`).

**Ce que cela impose, opérationnellement** :

- **rebâtir et redéployer l'agent** avant toute recette —
  `set -a && source .env && set +a`, puis `scripts/build-agent.sh`, puis
  **vérifier la TAILLE du binaire** (une compilation de 0,13 s est un aveu) ;
- **relancer le service plateforme** au même commit ;
- 🔴 **vérifier la version des DEUX côtés**, et par les vecteurs : le test
  `conformite_aux_vecteurs_partages` compare `doc["version"]` à
  `PLATEFORME_VERSION` **en Rust**, et `proto/ts/plateforme.test.ts` fait de
  même en TypeScript. C'est la lacune d'`input.rs` que P3 a fermée pour ce
  fichier-ci, et **il ne faut pas la rouvrir**.

🔴 **Les tâches 2, 3 et 4 forment un GROUPE INDIVISIBLE, exécuté par le même
ouvrier, dans cet ordre, et le vert ne se lit qu'à la fin de la tâche 4.** Entre
elles, `cd proto && npm test` est **rouge par construction** : le Rust annonce 2
quand les vecteurs disent encore 1. **Un ouvrier qui lirait ce rouge comme un
défaut le corrigerait dans le mauvais sens.**

### D11 — `APPS=0` désarme la découverte, et `scripts/run-agent.sh` a sa TÂCHE DÉDIÉE

G1 ajoute à l'agent une boucle périodique qui ouvre COM, parcourt quatre
arborescences et résout 218 objets Shell, **toutes les 30 secondes, en
permanence**. Le dépôt a un précédent pour cela : `PLEIN_ECRAN=0`, `AUDIO=0`,
`PART_SONDAGE=0` — **`=0` désarme ; une simple présence n'active pas**, et la
raison est écrite dans `CLAUDE.md` : tester `is_ok()` activerait le mécanisme en
écrivant `APPS=0` pour le couper.

`PERIODE_RECONCILIATION = 30 s` — **NON CALIBRÉE**, valeur proposée par la spec
D1, autorisée par ses 113 ms mesurés.

🔴 **La ligne de `scripts/run-agent.sh` est une TÂCHE À ELLE SEULE.** Ce piège a
été payé en D1 (`SUPERVISEUR`), en D2 (`MULTIFENETRE_REPRISE`) et en D7
(`AUDIO`, dont implémenteur et relecteur avaient vérifié la propriété **en
traçant le code** — le tracé était juste, la valeur ne pouvait simplement pas
atteindre le processus). D3 et D6 l'ont évité par une tâche dédiée ; G1 fait de
même.

### D12 — G1 ne livre AUCUNE page de hub

La spec §6 range `client/src/hub/…` en « (G1, G3) ». **Aucun des six critères de
G1 ne juge une page**, et sa tranche verticale est décrite en termes de
`GET /applications` et de `POST … /lancer` — c'est-à-dire de HTTP.

**Décision : rien dans `client/`.** La recette exerce les deux routes par
`curl` / Node depuis l'hôte, ce qui les éprouve **davantage** qu'une page (elle
peut poser un jeton absent, un jeton d'agent, une VM étrangère). Le hub arrive
avec G3, qui en a réellement besoin — un dépôt de fichier ne se fait pas en
`curl` — et ⑥ l'habille.

⚠️ **Conséquence : `client/` n'est pas touché par G1**, donc les relevés ⑥ et ⑦
restent inchangés à l'unité près. C'est un contrôle, pas une commodité.

### D13 — Un raccourci écarté est journalisé AU CHANGEMENT, jamais à chaque tour

Le critère ④ de la spec exige « 7 lignes de trace nommant chacune son fichier ».
Sous une réconciliation **toutes les 30 s**, une trace par tour rendrait
**20 160 lignes par jour** pour sept fichiers qui ne changent pas — et le
journal est partagé par le superviseur, le capteur et tous les enfants depuis
D4.

**Décision :** l'ensemble des chemins écartés est retenu d'un tour à l'autre ;
une ligne est émise quand un chemin **entre** dans cet ensemble, une autre quand
il en **sort**. Le premier tour émet donc les sept, et les suivants zéro.

⚠️ **Le critère se mesure alors sur la PREMIÈRE réconciliation, bornée
temporellement** — jamais sur un total de fichier. C'est la leçon du « 44 avant
/ 44 après » de D2 : « Ne jamais opposer un total de fichier à un compte
fenêtré. »

Format retenu, pour que le `grep` du critère soit décidable :
`raccourci ecarte chemin="…" motif=cible-vide` — motifs `cible-vide`,
`extension` (avec l'extension observée), `cible-absente`.

### D14 — La boucle d'apps vit dans `main.rs`, après l'enrôlement, dans les DEUX modes, et JAMAIS dans le capteur

Relevé dans `agent/src/main.rs` :

- l'aiguillage `CAPTEUR` **retourne AVANT l'enrôlement** (l. 383-385) : un
  capteur n'ouvre aucun canal `/agent`, n'a ni jeton ni préfixe (E3) ;
- l'enrôlement est en l. 399-424, et lie le `Canal` à `_canal_plateforme`
  **uniquement pour le garder vivant** ;
- l'aiguillage `SUPERVISEUR` (l. 430-432) et le mono-fenêtre (l. 434) viennent
  **après**.

**Décision :** `apps::brancher(&canal)` est appelée **entre l'enrôlement et
l'aiguillage `SUPERVISEUR`**, et rend une `JoinHandle` gardée comme
`_canal_plateforme` l'est. Elle vit donc dans les **deux** modes, et dans aucun
capteur.

🔴 **Elle n'emprunte PAS `superviseur/lanceur.rs`.** Ce fichier lance
`current_exe()` (`superviseur.rs:67`) et **rien d'autre** — l'agent n'a jamais
lancé de binaire tiers. Son job object porte
`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` (`superviseur/lanceur.rs:98`) : y assigner
une application lancée par l'utilisateur la tuerait avec l'agent. C'est
exactement le raisonnement de la spec D8 pour l'installeur, et il vaut ici pour
les mêmes raisons. `ShellExecuteEx` crée son processus hors de tout job.

⚠️ **`ShellExecuteExW` exige COM initialisé sur le fil appelant**, en
appartement cloisonné. La boucle tourne sur un `spawn_blocking` dédié qui
appelle `CoInitializeEx` une fois — le contrôle strict du `HRESULT` a son
précédent à `agent/src/wasapi.rs:137-176`, et l'absence de `CoUninitialize` y est
déjà justifiée.

---

## Divergences relevées entre la spec, le code réel, et ce que G1 doit faire

Elles sont tranchées **avant** d'écrire une ligne, sur le modèle de P1 (six),
P2 (onze) et P3 (seize). Chacune porte le relevé qui la fonde.

### E1 — `plateforme/` est DÉJÀ au § « Portée » de `CLAUDE.md`

La spec §6 écrit : « ⚠️ **`plateforme/` doit entrer au § « Portée » de
`CLAUDE.md`** ». Relu ce jour, `CLAUDE.md:19-20` :

```
**Portée** — la règle s'applique au code source écrit à la main :
`agent/src/`, `client/src/`, `plateforme/`, `proto/`, `src/`, `web/`, `scripts/`.
```

**C'est fait.** Rien à faire, et la tâche 22 ne doit pas le refaire.

⚠️ **La divergence texte/commande que `CLAUDE.md` signale depuis D10 n'est pas
tranchée ici non plus** : le § « Portée » énumère des répertoires, la commande ne
filtre que `node_modules`, les verrous, `dist/`, `testdata/`, `docs/` et
`CLAUDE.md` — et attrape donc `client/verify-webrtc.mjs`, hors de `client/src/`.
G1 applique la contrainte la plus stricte des deux, sûre dans les deux lectures,
et **ne prend pas au passage une décision qui appartient au propriétaire du
dépôt**.

### E2 — 🔴 Le canal `/agent` n'a AUCUN chemin descendant vers le reste de l'agent

La spec suppose partout qu'un ordre `Lancer` arrive à l'agent. **Rien ne le
permet aujourd'hui.** Relevé :

- `Canal` (`agent/src/plateforme.rs:59-65`) n'a que deux champs : un
  `watch::Receiver<Option<Identite>>` et une `JoinHandle` ;
- sa seule méthode publique est `attendre_identite` (l. 124) ;
- la boucle `une_session` (l. 138-243) traite les **trois** variantes
  descendantes **en place**, et un message illisible ferme la session
  (l. 232-238) ;
- `main.rs:399` lie le `Canal` à `_canal_plateforme` et n'en fait rien d'autre :
  seuls les **champs** de l'`Identite` sont recopiés dans `Config`
  (`main.rs:412-413`).

**Il n'existe donc ni chemin montant hors des deux messages câblés en dur, ni
chemin descendant.** La tâche 10 les crée : une `mpsc` d'émission drainée dans
le `select!` de `une_session`, et une `mpsc` de réception des ordres.

⚠️ **Un message montant mis en file pendant que le socket est tombé est PERDU**,
et c'est acceptable **uniquement** parce que D3 fait renvoyer le catalogue
complet à chaque réenrôlement. **Le déclarer dans le code**, à côté de la file.

### E3 — Le mode `CAPTEUR` retourne AVANT l'enrôlement

`agent/src/main.rs:383-385` : `if matches!(env "CAPTEUR", Ok(v) if v != "0") {
return capteur::executer(); }`, et l'enrôlement est en l. 399. **Un capteur n'a
donc ni canal, ni jeton, ni préfixe** — et ne peut porter aucun catalogue. C'est
ce que D14 acte. La spec ne le dit pas ; il aurait été naturel de placer la
découverte « dans l'agent » sans distinguer les modes.

### E4 — 🔴 La plateforme n'a AUCUN registre de sockets d'agent vivants

`plateforme/src/agents/canal.ts:84-88` : l'état de la connexion (`vmId`,
`prefixe`) est **local à la fermeture** de `wss.on('connection', …)`. Rien, dans
tout `plateforme/src`, ne conserve la liste des sockets ouverts par VM.
`POST /application/:id/lancer` est **impossible** sans en créer un — c'est
l'objet de D8 et de la tâche 16.

### E5 — Il n'existe AUCUN routage HTTP paramétré ni AUCUNE authentification HTTP

- `plateforme/src/http/routes-auth.ts:46` :
  `const CHEMINS = new Set(['/auth/connexion', '/auth/rafraichir']);`, comparé
  par `CHEMINS.has(chemin)` (l. 99). **Aucune extraction de segment**, nulle
  part.
- **Aucune occurrence de `Authorization`, `Bearer` ou `cookie`** dans
  `plateforme/src` : les routes existantes sont ouvertes par construction
  (`routes-auth.ts:41`), et le jeton ne voyage aujourd'hui que dans la poignée
  de main WebSocket.

G1 introduit donc **les deux** : un routage à segment pour
`/application/:id/lancer`, et la première lecture d'un jeton porteur HTTP.
**Le motif d'extraction est ancré des deux bouts** (`^/application/([^/]+)/lancer$`),
jamais un `startsWith` — même raison que `serveur.ts:127-129` : « `/agentaire`
n'est pas `/agent`, et un préfixe ouvrirait une famille entière de chemins que
personne n'a décidés. »

### E6 — `cors.ts` n'autorise que `POST, OPTIONS`

`plateforme/src/http/cors.ts:36` pose
`Access-Control-Allow-Methods: 'POST, OPTIONS'`. Le hub sera servi par Vite sur
un autre port que la plateforme (`client/src/connexion.ts:23` vise déjà
`http://<hostname>:8080` depuis une page d'un autre port) : **un
`GET /applications` cross-origin est refusé par le préflight tant que cette
ligne n'a pas `GET`.** La tâche 19 l'ajoute, et `cors.test.ts` (57 lignes) fige
la nouvelle valeur.

### E7 — 🔴 `ALTER TABLE … ADD COLUMN … UNIQUE` est REFUSÉ par SQLite

Mesuré ce jour, SQLite 3.50.4 : `Cannot add a UNIQUE column`. PostgreSQL 16.15
l'accepte. **Deux moteurs, deux comportements, et le lint statique ne peut rien
contre** — c'est exactement le partage que
`plateforme/src/base/sous-ensemble.test.ts:12-14` décrit (« ce lint ne peut rien
contre une construction syntaxiquement licite des deux côtés mais de sémantique
divergente — c'est le rôle de la double passe »), sauf qu'ici c'est pire : la
construction n'est pas licite des deux côtés.

**La spec §3.3 n'a mesuré ni ce cas ni le suivant.** Remède : `CREATE UNIQUE
INDEX`, mesuré OK des deux côtés.

### E8 — 🔴 `ADD COLUMN … NOT NULL` sans DEFAUT ne passe QUE sur une table VIDE

Mesuré ce jour : SQLite l'accepte sur une table vide, et le **refuse** dès
qu'elle porte une ligne (`Cannot add a NOT NULL column with default value
NULL`). Et un `DEFAULT` littéral est impossible dans ce dépôt (D5).

**Conséquence qui dépasse G1, et qu'il faut inscrire :** toute colonne
**NOT NULL** dont un sous-bloc ultérieur aura besoin sur `application` doit
naître **maintenant**, tant que la table est vide. C'est pourquoi `apparue_a`
naît en G1 sans avoir d'appelant (D5). Les colonnes de G2 (`icone_sha256`,
`source_max_px`) sont **nullables par nature** — une application sans icône est
un cas nominal (spec §7) — et peuvent donc attendre.

### E9 — `pilotes.test.ts` fige la liste des migrations, et `0004` la fait rougir

`plateforme/src/base/pilotes.test.ts:33` :
`expect(suivi.map((l) => Number(l.version))).toEqual([1, 2, 3]);` et l. 38
`expect(apres).toHaveLength(3)`. Son commentaire (l. 28-32) dit que la liste est
**écrite en dur délibérément** — « une comparaison contre `readdirSync` serait
une tautologie » — et que le prix en est « une mise à jour CONSCIENTE de cette
ligne ».

**La tâche 13 les porte à `[1, 2, 3, 4]` et `4`.** 🔴 **C'est une ROUGE
gratuite et elle doit être VUE** : ajouter `0004` sans toucher ces deux lignes
fait tomber la double passe, sur les deux moteurs.

### E10 — `ExpandEnvironmentStringsW` exige une feature ABSENTE d'`agent/Cargo.toml`

La spec D2 prescrit `SLGP_RAWPATH` puis `ExpandEnvironmentStringsW`. Relevé
dans le crate `windows` 0.62.2 :
`Windows/Win32/System/Environment/mod.rs:76` — la fonction vit sous
`Win32_System_Environment`, et un `grep -n 'Win32_System_Environment'
agent/Cargo.toml` ne rend **rien**.

**La tâche 7 ajoute cette feature**, et **elle seule** — aucune nouvelle
dépendance.

✅ **Le reste est déjà accessible sans toucher au `Cargo.toml`**, relu dans les
bindings :

| Symbole | Où | Feature |
| --- | --- | --- |
| `IShellLinkW` | `Win32/UI/Shell/mod.rs:39947` | `Win32_UI_Shell`, **activée transitivement** par `Win32_UI_Shell_PropertiesSystem` (`windows-0.62.2/Cargo.toml:704` : `Win32_UI_Shell_PropertiesSystem = ["Win32_UI_Shell"]`), déjà déclarée |
| `ShellExecuteExW` | `Win32/UI/Shell/mod.rs:4362` | idem |
| `SHGetKnownFolderPath` | `Win32/UI/Shell/mod.rs:3301` | idem |
| `ShellLink` (le CLSID) | `Win32/UI/Shell/mod.rs:56159` | idem |
| `IPersistFile` | `Win32/System/Com/mod.rs:7169` | `Win32_System_Com`, déjà déclarée |
| `WIN32_FIND_DATAW` (2ᵉ argument de `GetPath`) | — | `Win32_Storage_FileSystem`, déjà déclarée |

⚠️ **Deux détails de signature, relus et non supposés** :
`IShellLinkW::GetPath(&self, pszfile: &mut [u16], pfd: *mut WIN32_FIND_DATAW,
fflags: u32)` — le troisième argument est un **`u32` nu**, alors que les
constantes vivent dans le newtype `SLGP_FLAGS(pub i32)`
(`Win32/UI/Shell/mod.rs:55259`) : la conversion est explicite. Et le second peut
être `null_mut()`.

🔴 **Rien de tout cela ne remplace `cargo check --target
x86_64-pc-windows-gnu`** — qui couvre types, emprunts, visibilités et durées de
vie, **et PAS l'édition de liens** (acquis de D3). Les tâches 7 et 8 le lancent.

### E11 — `TYPES_DEPUIS` et `TYPES_VERS` sont écrits à la main : c'est le jumeau de `TYPES_AGENT`

`proto/ts/plateforme.ts:53` (`TYPES_DEPUIS`) et `:83` (`TYPES_VERS`) sont deux
littéraux `as const` que **rien ne confronte aux unions** `DepuisLaPlateforme` /
`VersLaPlateforme`. C'est exactement l'état de `TYPES_AGENT`
(`proto/ts/control.ts:106`), dont l'oubli ne casse « ni compilation ni test ».

Ici l'oubli **casse au runtime** — `parseDepuisLaPlateforme` lèverait « type de
message de plateforme inconnu » — mais **uniquement si un test fait passer le
message par le parseur**. Un vecteur qui n'exercerait que l'encodeur ne le
verrait pas.

**Remède STRUCTUREL, pas un test de plus** : les deux listes sont **dérivées**
d'un enregistrement exhaustif typé par l'union.

```ts
const TOUS_DEPUIS: Record<DepuisLaPlateforme['type'], true> = {
    enrole: true, 'battement-recu': true, refus: true, lancer: true,
};
const TYPES_DEPUIS = Object.keys(TOUS_DEPUIS) as DepuisLaPlateforme['type'][];
```

🔴 **La ROUGE est ATTEIGNABLE ET VÉRIFIABLE** : ajouter une variante à l'union
sans ajouter sa clé fait échouer `cd proto && npm run typecheck` — un
`Record<K, true>` dont une clé manque est une erreur `tsc`. **La tâche 3 doit
jouer cette rouge**, pas seulement l'écrire.

### E12 — Le critère ④ de la spec n'est pas applicable tel quel sous une réconciliation périodique

« 7 lignes de trace nommant chacune son fichier » : sous
`PERIODE_RECONCILIATION = 30 s`, c'est 7 lignes **toutes les 30 secondes**, dans
un journal partagé par le superviseur, le capteur et tous les enfants depuis D4.
D13 tranche : trace **au changement**, comptée sur la **première**
réconciliation, dans une fenêtre temporelle explicite.

### E13 — Le critère ⑤ de la spec est NON DISCRIMINANT, la spec le dit, et D4 le répare

« lancer par la cible reconstruite au lieu du `.lnk` **passerait ce
critère-ci** ». `IssueLancement::Raccourci` vs `::Cible` le rend décidable — voir
D4. Le critère ⑥ reste, et les deux deviennent indépendants.

### E14 — La spec range `client/src/hub/…` en G1 ; aucun critère de G1 ne le juge

Voir D12. Décision de périmètre, déclarée.

### E15 — La spec fixe les quatre racines en littéral, dont un profil nommé

`C:\Users\Administrateur\Desktop` est le profil d'**une** machine.
`SHGetKnownFolderPath` (D6) les résout. La spec §3.1 mesure ces chemins ; elle
ne prescrit pas de les coder.

### E16 — Les 218 / 167 / 154 ont été mesurés par `WScript.Shell`, pas par `IShellLinkW`

La spec §3.1 le dit : « `WScript.Shell.CreateShortcut`, **qui est l'enveloppe COM
d'`IShellLinkW`** ». C'est très probablement équivalent, **et ce n'est pas
mesuré**. La tâche 20 relève les chiffres de l'agent lui-même. **Un écart est
une mesure**, à écrire dans le document de résultats, jamais à aplatir en
ajustant le corpus.

### E17 — Contrôle qui PASSE : le schéma de `application` cité par la spec est exact

La spec §3.3 donne `application(id, vm_id, nom, chemin, vue_a)` à
`plateforme/src/base/migrations/0003-agents.sql:52-58`. **Relu, exact**, aux
lignes exactes, `vue_a` comprise en `BIGINT NOT NULL`. De même
`agent/src/plateforme.rs` **277** lignes, `proto/src/plateforme.rs` **410**,
`plateforme/src/agents/canal.ts` **198**, `plateforme/src/agents/fraicheur.ts:37`
(`SEUIL_INJOIGNABLE_MS = 90_000`), `plateforme/src/http/routes-auth.ts:44`
(`CORPS_MAX_OCTETS = 4 * 1024`), `agent/src/superviseur/lanceur.rs:98`
(`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`), `src/app.js:152`, `src/app.js:67`,
`src/lnkParser.js:90`. **Dix citations de la spec vérifiées, dix exactes** —
c'est déclaré parce qu'un contrôle qui passe est une information autant qu'un
contrôle qui échoue.

### E18 — Le bras catch-all de `pont_media.rs` NE CONCERNE PAS G1, et il faut le vérifier plutôt que le supposer

`agent/src/capteur/pont_media.rs:75-77` porte bien le bras
`Ok(autre) => { warn!(…); return; }` qui tue le fil de lecture **en silence** —
défaut payé quatre fois (D5 `Sommeil`, D6 `Part`, D7 `Audio`, D8 `PleinEcran`).

**Aucun message de G1 ne passe par ce canal** : `Catalogue`, `Lancer` et
`Lancee` voyagent sur `/agent`, entre l'agent et la plateforme, et
`agent/src/capteur/protocole.rs` n'est pas touché — le superviseur ne parle
d'ailleurs pas au capteur (`capteur/protocole.rs:34-37` : « il n'existe **aucun
canal direct superviseur→capteur** »).

🔴 **La tâche 21 doit néanmoins le VÉRIFIER par la commande**
(`git diff --stat -- agent/src/capteur/`), pas le supposer : c'est le seul geste
qui distingue « nous n'y avons pas touché » de « nous croyons ne pas y avoir
touché ».

---

## Structure des fichiers

```
proto/
  src/plateforme.rs              MODIFIÉ — extraction des tests (T1), puis PLATEFORME_VERSION -> 2
                                 et trois variantes (T2)
  src/plateforme/tests.rs        NEUF — le module de tests, déplacé VERBATIM (T1)
  ts/plateforme.ts               MODIFIÉ — le miroir, listes blanches DÉRIVÉES de l'union (T3)
  ts/plateforme.test.ts          MODIFIÉ (T3)
  plateforme-vectors.json        MODIFIÉ — "version": 2, un cas par variante (T4)

agent/
  Cargo.toml                     MODIFIÉ — une feature : Win32_System_Environment (T7)
  src/main.rs                    MODIFIÉ — apps::brancher après l'enrôlement (T11)
  src/plateforme.rs              MODIFIÉ — file d'émission, réception des ordres (T10)
  src/apps.rs                    NEUF — declaré SANS cfg ; la boucle et l'assemblage (T9)
  src/apps/raccourci.rs          NEUF — PUR : normalisation, clé, règle de filtrage (T5)
  src/apps/reconciliation.rs     NEUF — PUR : le diff (T6)
  src/apps/lecture.rs            NEUF — #[cfg(windows)] : IShellLinkW, SHGetKnownFolderPath (T7)
  src/apps/lancement.rs          NEUF — #[cfg(windows)] : ShellExecuteExW (T8)
  testdata/gapps-corpus-vm.json  NEUF — les 218 raccourcis de la VM (T5, rejoué T20)

plateforme/
  src/base/migrations/0004-applications.sql  NEUF (T13)
  src/base/pilotes.test.ts       MODIFIÉ — deux lignes (E9, T13)
  src/depot/application.ts       NEUF (T14)
  src/depot/application.test.ts  NEUF (T14)
  src/apps/catalogue.ts          NEUF — PUR : la fusion (T15)
  src/apps/catalogue.test.ts     NEUF (T15)
  src/agents/registre.ts         NEUF — le registre des sockets vivants (T16)
  src/agents/registre.test.ts    NEUF (T16)
  src/agents/canal.ts            MODIFIÉ — deux branches, inscription au registre (T17)
  src/agents/canal.test.ts       MODIFIÉ, et SCINDÉ s'il franchit 450 (T17)
  src/http/routes-applications.ts       NEUF (T18)
  src/http/routes-applications.test.ts  NEUF (T18)
  src/http/cors.ts               MODIFIÉ — GET (T19)
  src/http/cors.test.ts          MODIFIÉ (T19)
  src/http/serveur.ts            MODIFIÉ — la route chaînée avant le 404 (T19)

scripts/run-agent.sh             MODIFIÉ — une ligne, tâche DÉDIÉE (T12)

docs/superpowers/plans/
  journaux-gestion-apps/         NEUF — les journaux de la recette (T20)
  2026-08-19-gestion-apps-g1-resultats.md   NEUF (T23)
```

⚠️ **`scripts/verify-all.sh` N'EST PAS MODIFIÉ** : ses **dix** étapes couvrent
déjà les quatre paquets. Relevé par la commande, pas supposé.

⚠️ **`agent/src/apps.rs` est déclaré `mod apps;` SANS `cfg` dans `main.rs`**, et
ce sont ses fonctions Windows qui portent le `#[cfg(windows)]`. C'est la
décision de la spec §6, et elle a une conséquence : l'arbre `apps::raccourci` et
`apps::reconciliation` **existe sur l'hôte**, sans quoi leurs tests n'y
courraient pas. La « Convention de module enfant » de `CLAUDE.md` **n'est donc
pas mobilisée** — aucun module ne franchit de frontière `#[cfg(windows)]`. Elle
est nommée pour dire qu'elle a été considérée.

---

## Interfaces partagées

```rust
// proto/src/plateforme.rs
pub const PLATEFORME_VERSION: u8 = 2;          // 🔴 rupture assumée (D10)

/// Une application telle que l'agent la découvre.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Application {
    pub cle: String,          // SHA-256 du triplet normalisé (spec D4)
    pub nom: String,          // le nom du .lnk, sans son extension
    pub chemin: String,       // le chemin du .lnk lui-même
    pub cible: String,        // normalisée : absolue, casse repliée
    pub arguments: String,    // BRUTS, sensibles à la casse
    pub repertoire: String,   // normalisé
}

pub enum VersLaPlateforme {
    Enroler { … }, Battement { … },                       // P3, inchangés
    Catalogue { v, complet: bool, applications: Vec<Application>, disparues: Vec<String> },
    Lancee    { v, demande: String, issue: IssueLancement },
}

pub enum DepuisLaPlateforme {
    Enrole { … }, BattementRecu { … }, Refus { … },       // P3, inchangés
    Lancer { v, demande: String, cle: String },
}

#[derive(Serialize, Deserialize)] #[serde(rename_all = "kebab-case")]
pub enum IssueLancement { Raccourci, Cible, Inconnue, Echec }
```

```rust
// agent/src/apps/raccourci.rs — PUR, aucun cfg
pub struct Brut { pub nom: String, pub chemin: String, pub cible: String,
                  pub arguments: String, pub repertoire: String }

/// La règle de filtrage de la spec D3. `existe` est INJECTÉ (D7).
pub fn retenir(brut: &Brut, existe: &dyn Fn(&str) -> bool) -> Result<(), Ecart>;
pub enum Ecart { CibleVide, Extension(String), CibleAbsente }

pub fn normaliser_chemin(chemin: &str) -> String;   // absolu, casse repliée
pub fn cle(cible: &str, arguments: &str, repertoire: &str) -> String;  // SHA-256 hex
pub fn depuis_brut(brut: Brut) -> proto::plateforme::Application;
```

```rust
// agent/src/apps/reconciliation.rs — PUR, aucun cfg
pub struct Diff { pub apparues: Vec<Application>, pub modifiees: Vec<Application>,
                  pub disparues: Vec<String> }
pub fn diff(hier: &[Application], aujourdhui: &[Application]) -> Diff;
```

```rust
// agent/src/apps.rs
pub const PERIODE_RECONCILIATION: Duration = Duration::from_secs(30); // NON CALIBRÉE
pub fn brancher(canal: &Canal) -> Option<tokio::task::JoinHandle<()>>; // None si APPS=0
```

```ts
// plateforme/src/apps/catalogue.ts — PUR, sans base, sans horloge lue
export interface Connue { id: string; cle: string; disparue_a: number | null; /* … */ }
export interface Fusion {
    aInserer: Application[];
    aMettreAJour: Array<{ id: string; app: Application }>;
    aMarquerDisparues: string[];   // des identifiants, jamais des clés
    aRessusciter: string[];        // disparue_a repassée à NULL
}
export function fusionner(connues: Connue[], message: CatalogueMessage): Fusion;
```

```ts
// plateforme/src/depot/application.ts
export interface LigneApplication {
    id: string; vm_id: string; nom: string; chemin: string; vue_a: number;
    cle: string; cible: string; arguments: string; repertoire: string;
    apparue_a: number; disparue_a: number | null; masquee_a: number | null;
}
export async function lireParVm(p: Pilote, vmId: string): Promise<LigneApplication[]>;
export async function lireParId(p: Pilote, id: string): Promise<LigneApplication | undefined>;
export async function appliquer(p: Pilote, vmId: string, fusion: Fusion, maintenant: number): Promise<void>;
```

```ts
// plateforme/src/agents/registre.ts
export class RegistreAgents {
    inscrire(vmId: string, socket: WebSocket): void;   // le DERNIER gagne (D8)
    retirer(vmId: string): void;                       // rejette les demandes en vol
    lancer(vmId: string, cle: string, demande: string): Promise<IssueLancement | 'agent-injoignable'>;
    resoudre(demande: string, issue: IssueLancement): void;
}
export const DELAI_LANCEMENT_MS = 5_000;               // NON CALIBRÉE
```

---

## Ordre et parallélisme

| Famille | Tâches | Dépend de | Parallélisable | Paquets touchés |
| --- | --- | --- | --- | --- |
| 0 — le protocole partagé | 1, 2, 3, 4 | 1 ← rien ; 2 ← 1 ; 3 ← 2 ; 4 ← 3 | **aucune entre elles** (D10) | 🔴 `proto/` |
| 1 — l'agent, **pur** | 5, 6 | 5 ← rien ; 6 ← 5 | avec 0, 4, 5 | 🔴 `agent/` |
| 2 — l'agent, Windows | 7, 8, 9 | 7 ← 5 ; 8 ← 5 ; 9 ← 5, 6, 7, 8, 10, F0 | 7 et 8 entre elles | 🔴 `agent/` |
| 3 — le canal côté agent | 10, 11, 12 | 10 ← F0 ; 11 ← 9, 10 ; 12 ← 9 | 10 avec 5, 7, 8 | 🔴 `agent/`, `scripts/` |
| 4 — la plateforme, persistance | 13, 14, 15 | 13 ← rien ; 14 ← 13 ; 15 ← F0 | 13 et 15 avec tout | `plateforme/` |
| 5 — la plateforme, canal et routes | 16, 17, 18, 19 | 16 ← rien ; 17 ← 14, 15, 16, F0 ; 18 ← 14, 16 ; 19 ← 18 | 16 avec tout ; 17 et 18 entre elles | `plateforme/` |
| 6 — recette et clôture | 20, 21, 22, 23 | 20 ← tout ; 21, 22, 23 ← 20 | 21 et 22 entre elles | — |

**Chemin critique** : 1 → 2 → 3 → 4 → 10 → 9 → 11 → 20.

**Quatre tâches purement isolées peuvent démarrer ensemble au premier tour** :
**1** (`proto/`), **5** (`agent/`, pur), **13** et **16** (`plateforme/`). Trois
paquets distincts, donc aucune ne bloque une autre sur un fichier.

🔴 **Les tâches 2, 3 et 4 ne se parallélisent avec RIEN, entre elles ni avec le
reste de `proto/`** : elles portent le bump de version, et le vert ne se lit
qu'à la fin de la 4 (D10).

🔴 **La tâche 1 (extraction) précède la tâche 2 sans exception**, et n'ajoute
**aucune** ligne de comportement.

🔴 **La tâche 12 (`scripts/run-agent.sh`) est dédiée et ne fait rien d'autre.**

⚠️ **La tâche 20 est la seule qui emploie la VM Windows.**

---

# Famille 0 — le protocole partagé

### Task 1 : 🔴 EXTRACTION de `proto/src/plateforme.rs`, AVANT toute addition

**Objet :** rendre au module sa marge **avant** que la tâche 2 n'y écrive.
**Aucune addition de comportement d'aucune sorte.**

**Files:**
- Create: `proto/src/plateforme/tests.rs`
- Modify: `proto/src/plateforme.rs`

- [ ] **Step 0 :** relever la référence Rust sur un arbre dont on **nomme le
      commit** : `git rev-parse HEAD`, `git status --porcelain -- agent/ proto/`,
      puis `cargo test --workspace` et `cargo clippy --workspace`. **Annoncer les
      comptes avant de les lire.** (C'est la référence que le § « relevés » ne
      porte pas, et la raison en est déclarée là-bas.)
- [ ] **Step 1 :** `wc -l proto/src/plateforme.rs` → attendu **410**. Si le
      nombre diffère, **c'est le nombre qui fait foi**, pas ce plan.
- [ ] **Step 2 :** déplacer le bloc `#[cfg(test)] mod tests { … }` **VERBATIM**
      vers `proto/src/plateforme/tests.rs`, en retirant l'enveloppe
      `mod tests { }` et en posant `use super::super::*;` — ou `use crate::plateforme::*;`,
      au choix, **le même dans tout le fichier**.
- [ ] **Step 3 :** déclarer dans `plateforme.rs` :
      `#[cfg(test)] #[path = "plateforme/tests.rs"] mod tests;`
- [ ] **Step 4 :** 🔴 **corriger `include_str!("../plateforme-vectors.json")` en
      `include_str!("../../plateforme-vectors.json")`** (D1). L'échec est une
      erreur de compilation, donc bruyant — mais le corriger *après* avoir vu
      l'erreur coûte un tour.
- [ ] **Step 5 : voir vert, et voir le MÊME compte qu'au step 0.** Une
      extraction qui change un compte de tests n'est pas une extraction.
      🔴 **Contrôle que la copie est verbatim** :
      `git show HEAD:proto/src/plateforme.rs | sed -n '167,410p'` comparé au
      nouveau fichier — **hors la ligne d'`include_str!` et l'en-tête de
      module**, aucune autre différence n'est admise.
- [ ] **Step 6 :** `wc -l proto/src/plateforme.rs proto/src/plateforme/tests.rs`,
      relevé et **écrit dans le rapport de tâche**.

**Ce qui rendrait cette tâche ROUGE :** rien — elle ne change aucun
comportement, et c'est sa définition. **Le contrôle est que tout reste
identique.**

---

### Task 2 : les trois variantes, et `PLATEFORME_VERSION = 2`

**Objet :** porter le canal à la version 2 avec `Catalogue`, `Lancer`, `Lancee`,
`Application` et `IssueLancement`.

**Files:**
- Modify: `proto/src/plateforme.rs`, `proto/src/plateforme/tests.rs`

**Interfaces:** produit `Application`, `IssueLancement`, et les trois variantes
(voir « Interfaces partagées »).

- [ ] **Step 1 : écrire les tests d'abord, et les voir ROUGES**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `Catalogue` sérialise en `{"type":"catalogue","v":2,"complet":true,"applications":[…],"disparues":[]}` | l'ordre des champs : serde émet le tag interne **en premier**, et le vecteur fige la chaîne exacte |
| `Lancer` sérialise en `{"type":"lancer","v":2,"demande":"…","cle":"…"}` | idem |
| `Lancee` sérialise en `{"type":"lancee","v":2,"demande":"…","issue":"raccourci"}` | 🔴 `IssueLancement` en `kebab-case` : `raccourci`, `cible`, `inconnue`, `echec` — **aucune n'a deux mots**, donc `snake_case` et `kebab-case` ne diffèrent sur aucune. **C'est une lacune ATTENDUE**, à écrire : le piège `battement-recu` de P3 ne se rejoue pas ici, et il faudra y penser au jour où une issue à deux mots apparaîtra |
| **un test de version par variante ENTRANTE, absente puis suivante** — `catalogue`, `lancee` côté `VersLaPlateforme` ; `lancer` côté `DepuisLaPlateforme` | 🔴 **omettre `verifie_version` sur UNE SEULE variante**. La convention est écrite à `proto/src/plateforme.rs` (« UN TEST DE VERSION PAR VARIANTE ENTRANTE, jamais un seul pour toutes ») : un test unique ne verrait pas ce trou-là |
| `deny_unknown_fields` refuse `{"type":"lancer","v":2,"demande":"d","cle":"c","bonus":1}` | retirer l'attribut |
| les cinq variantes **de P3** rejettent désormais `"v":1` | 🔴 **c'est la rouge du bump lui-même** : si `PLATEFORME_VERSION` restait à 1, ce test resterait vert et la rupture ne serait pas jouée |

- [ ] **Step 2 : implémenter.** `PLATEFORME_VERSION: u8 = 2`, la doc de la
      constante gagne sa ligne `v2 (sous-bloc G1) : catalogue, lancement`.
      **Mettre à jour les chaînes JSON écrites en dur dans les tests de P3** —
      elles portent toutes `"v":1`.
- [ ] **Step 3 :** ⚠️ **`conformite_aux_vecteurs_partages` sera ROUGE**
      (`la version des vecteurs a dérivé de PLATEFORME_VERSION`) jusqu'à la
      tâche 4. **C'est attendu, et c'est déclaré ici pour qu'aucun ouvrier ne le
      corrige dans le mauvais sens.**
- [ ] **Step 4 :** compte de tests attendu, **annoncé avant d'être lu** :
      les tests de P3 (inchangés en nombre) **+ 12**.
- [ ] **Step 5 :** `wc -l` des deux fichiers ; si `tests.rs` dépasse **450**,
      le scinder en `tests_cycle.rs` / `tests_apps.rs` **avant de continuer**.

---

### Task 3 : le miroir TypeScript, et les listes blanches DÉRIVÉES de l'union

**Objet :** répercuter les trois variantes dans `proto/ts/plateforme.ts`, et
**fermer structurellement** le trou d'E11.

**Files:**
- Modify: `proto/ts/plateforme.ts`, `proto/ts/plateforme.test.ts`

- [ ] **Step 1 : écrire les tests d'abord, et les voir ROUGES**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `encodeCatalogue`, `encodeLancer`, `encodeLancee` produisent **la chaîne exacte** du Rust | écrire `v` avant `type` : `JSON.stringify` respecte l'ordre d'insertion, serde émet le tag en premier — la divergence a été trouvée par le test en P3, pas par la relecture |
| `parseVersLaPlateforme` accepte `catalogue` et `lancee`, **et valide leurs champs** | 🔴 c'est le seul parseur du fichier dont les octets viennent d'un tiers (l. 100-110 du fichier) : un `applications` absent traverserait jusqu'à la requête SQL |
| `parseVersLaPlateforme` refuse un `catalogue` dont `applications` n'est pas un tableau, avec `motif: 'forme'` | rendre `enrolement` : le pair lirait « secret faux » pour un message bien authentifié |
| `parseDepuisLaPlateforme` accepte `lancer` | l'omettre de `TYPES_DEPUIS` |
| 🔴 **le typecheck échoue si une variante manque à `TOUS_DEPUIS` / `TOUS_VERS`** | **la rouge se joue en ajoutant une variante à l'union sans sa clé, et en lançant `cd proto && npm run typecheck`.** À JOUER, pas à supposer : c'est le remède d'E11, et un remède structurel qu'on n'a pas vu échouer n'en est pas un |
| `PLATEFORME_VERSION === 2` et un message `v: 1` est refusé | oublier le bump côté TS : les deux bouts divergeraient en silence |

- [ ] **Step 2 : implémenter.** Les deux listes blanches deviennent
      `Object.keys(TOUS_*)`, typées par l'union (voir E11). **Les commentaires
      de `TYPES_DEPUIS` et `TYPES_VERS` — qui expliquent pourquoi les types du
      sens inverse en sont absents — partent AVEC les constantes**, comme la
      règle des 500 lignes l'exige pour un commentaire attaché à sa valeur.
- [ ] **Step 3 :** ⚠️ `cd proto && npm test` reste **rouge** jusqu'à la tâche 4
      (les vecteurs disent encore 1). Attendu.
- [ ] **Step 4 :** compte de tests attendu, annoncé avant lecture : **+ 10**.
- [ ] **Step 5 :** `wc -l proto/ts/plateforme.ts proto/ts/plateforme.test.ts` ;
      porte à **450**.

---

### Task 4 : `proto/plateforme-vectors.json` — `"version": 2`, vérifié des DEUX côtés

**Objet :** figer les chaînes exactes des trois variantes neuves, et refermer le
groupe indivisible.

**Files:**
- Modify: `proto/plateforme-vectors.json`

- [ ] **Step 1 :** porter `"version"` à **2**, et réécrire les **neuf** cas
      existants avec `"v":2` — leur `json` est figé caractère pour caractère.
- [ ] **Step 2 :** ajouter au moins **six** cas : `catalogue_complet`,
      `catalogue_delta` (avec `disparues` non vide), `catalogue_vide`
      (`applications: []`, `complet: true` — **le cas d'une VM sans aucune
      application, qui doit vider le catalogue et non le laisser tel quel**),
      `lancer`, `lancee_raccourci`, `lancee_echec`. **Au moins un cas porte des
      accents** dans un nom d'application, sur le modèle de
      `enroler_a_nom_accentue`.
- [ ] **Step 3 : voir VERT des deux côtés** — `cargo test -p proto` et
      `cd proto && npm test`. 🔴 **C'est ici, et seulement ici, que le groupe
      indivisible se referme.**
- [ ] **Step 4 :** le compte de cas est **écrit en dur** dans les deux tests
      (`assert_eq!(vus, cases.len())` existe déjà côté Rust) : vérifier que
      **tous** les cas sont exercés, et que le `panic!`/`throw` sur un `kind`
      inconnu est **atteignable** (le tester en écrivant un `kind` bidon dans
      une copie locale du fichier, puis en la restaurant).
- [ ] **Step 5 :** `./scripts/verify-all.sh`, étapes ⑥ et ⑦ au minimum.

---

# Famille 1 — l'agent, PUR

### Task 5 : la normalisation, la clé d'identité, la règle de filtrage — et le corpus versé

**Objet :** livrer les trois règles pures de la spec D3 et D4, et le corpus qui
les juge contre les chiffres réels de la VM.

**Files:**
- Create: `agent/src/apps/raccourci.rs`, `agent/testdata/gapps-corpus-vm.json`
- Modify: `agent/src/main.rs` (une ligne : `mod apps;`), `agent/src/apps.rs`
  (créé ici, réduit à ses `mod`)

- [ ] **Step 1 : construire le corpus.** ⚠️ **Il ne peut pas encore venir de
      l'agent** (rien ne le lit). Il est donc **initialement dérivé du
      dépouillement de la spec §3.1**, et le fichier porte en tête un champ
      `"provenance": "spec 2026-08-19 §3.1, sonde WScript.Shell"`.
      🔴 **La tâche 20 le REMPLACE par un relevé de l'agent lui-même**, et
      change ce champ. **Un corpus dont la provenance n'est pas écrite est un
      corpus qu'on croira mesuré par le produit.**
      Le corpus porte, par entrée : `nom`, `chemin`, `cible`, `arguments`,
      `repertoire`, `existe` (booléen).
- [ ] **Step 2 : écrire les tests d'abord, et les voir ROUGES**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `retenir` écarte une cible **vide** avec `Ecart::CibleVide` | la règle 1 de la spec D3 ; sur le corpus, **7** entrées |
| `retenir` écarte `.msc`, `.url`, `.html`, `.pdf`, `.chm`, `.txt`, `.bat`, `.msi` avec `Ecart::Extension` | accepter `.msi` « puisqu'il s'installe » : la spec §10 le range **installable, pas lançable** |
| `retenir` écarte une cible dont `existe` rend `false`, avec `Ecart::CibleAbsente` | 🔴 **la rouge se joue en câblant `Path::exists()` en dur** : le test d'hôte devient inerte, puisque aucune cible n'existe sur l'hôte. C'est la raison d'être de D7 |
| `retenir` **n'écarte rien par le nom** : `maintenancetool.exe`, `Uninstall …` sont RETENUS | ajouter un motif « Uninstall » — dépendant de la langue, et il écarterait en silence des applications légitimes (spec D3) |
| `retenir` **n'écarte rien par le chemin** : une cible sous `C:\Windows` est RETENUE | un filtre système perdrait Bloc-notes et Paint (63 cibles concernées) |
| 🔴 **sur le corpus : 218 lus, 167 retenus** | l'un des trois écarts ci-dessus mal implémenté |
| 🔴 **sur le corpus : 154 clés distinctes par le TRIPLET** | — |
| 🔴 **sur le corpus : 104 clés distinctes par la CIBLE SEULE** | **c'est la ROUGE GRATUITE du critère ①, et elle est ici, sur l'hôte, sans VM.** Le test l'assène comme un fait, et un `cle()` qui ignorerait les arguments ferait tomber le test précédent en rendant 104 |
| la clé est **insensible à la casse sur la cible et le répertoire**, **sensible sur les arguments** | replier la casse des arguments : deux invocations distinctes fusionneraient (spec D4) |
| deux raccourcis **au même triplet, à des chemins de `.lnk` différents** rendent **une** application | l'identité par le chemin du `.lnk` : « un raccourci qui se déplace du Bureau vers le menu Démarrer resterait la même application » (spec D4) |
| deux applications **de même nom** à triplets différents restent **deux** | l'identité par le nom : c'est le défaut de `src/app.js:67`, relu |

- [ ] **Step 3 : implémenter.** SHA-256 par la crate déjà présente si elle
      l'est ; **sinon, aucune dépendance n'est ajoutée** — la clé devient une
      empreinte du dépôt, écrite à la main, et ce choix se déclare. ⚠️ **Relever
      d'abord** : `grep -n 'sha2\|sha-2\|ring\|blake' agent/Cargo.toml
      Cargo.toml`. Si rien, **trancher et écrire pourquoi** ; le contrôle
      `pilote.test.ts:48-49` ne porte que sur `plateforme/`, mais la discipline
      « aucune dépendance ajoutée » vaut pour tout G1.
- [ ] **Step 4 :** compte de tests attendu, annoncé avant lecture : **11**.
- [ ] **Step 5 :** `cargo test -p agent apps::raccourci` **et**
      `cargo check --target x86_64-pc-windows-gnu`.

---

### Task 6 : le diff de réconciliation, PUR

**Objet :** la fonction que la spec D1 appelle « le cœur du sous-bloc ».

**Files:**
- Create: `agent/src/apps/reconciliation.rs`
- Modify: `agent/src/apps.rs` (une ligne)

- [ ] **Step 1 : écrire les tests d'abord, et les voir ROUGES**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| deux catalogues **identiques** rendent un diff **entièrement vide** | comparer par ordre au lieu de par clé : un simple changement d'ordre de lecture du répertoire produirait un diff plein **à chaque tour**, donc un message par tour |
| une application neuve est dans `apparues`, et **pas** dans `modifiees` | — |
| une application dont **le nom** change (le `.lnk` renommé, même triplet) est dans `modifiees` | ne comparer que les clés : le nom affiché se figerait pour toujours |
| une application dont **le chemin du `.lnk`** change est dans `modifiees` | idem — et c'est ce chemin que le lancement emploie (spec D6) |
| une application absente d'aujourd'hui est dans `disparues`, **par sa clé** | rendre l'objet entier : la plateforme n'a besoin que de la clé, et l'objet ferait grossir le message |
| une application **qui revient** après avoir disparu est dans `apparues` | 🔴 « Une disparition n'est pas une suppression » (spec D1) : le diff la redonne, et c'est la plateforme qui saura qu'elle la connaît déjà (T15) |
| un catalogue d'hier **vide** rend tout en `apparues` et rien en `disparues` | c'est le premier tour, et il ne doit rien annoncer disparu |
| 🔴 le diff est **déterministe** : deux appels sur les mêmes entrées, dans un ordre d'entrée différent, rendent le même résultat **trié** | ne pas trier : deux réconciliations successives émettraient des messages différents pour un état identique, et rien ne le dirait |

- [ ] **Step 2 : implémenter.**
- [ ] **Step 3 :** compte de tests attendu, annoncé avant lecture : **8**.

---

# Famille 2 — l'agent, Windows

### Task 7 : la lecture d'un raccourci par `IShellLinkW`

**Objet :** les cinq champs, **sans jamais appeler `Resolve`** (spec D2).

**Files:**
- Create: `agent/src/apps/lecture.rs`
- Modify: `agent/src/apps.rs`, `agent/Cargo.toml` (une feature — E10)

- [ ] **Step 1 :** relever `git log --oneline -5 -- agent/` (périmètre
      concurrent), puis ajouter **`"Win32_System_Environment"`** à la liste des
      features du crate `windows`, **et rien d'autre**.
- [ ] **Step 2 : implémenter.**
      - `CoInitializeEx` en appartement cloisonné, `HRESULT` contrôlé
        strictement (précédent : `agent/src/wasapi.rs:137-176`) ;
      - `CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)` →
        `IShellLinkW`, puis `cast::<IPersistFile>()` et `Load(chemin, STGM_READ)` ;
      - `GetPath(&mut buf, null_mut(), (SLGP_RAWPATH | SLGP_UNCPRIORITY).0 as u32)`,
        `GetArguments`, `GetWorkingDirectory`, `GetIconLocation`, `GetShowCmd` ;
      - `ExpandEnvironmentStringsW` sur la cible et le répertoire ;
      - `SHGetKnownFolderPath` sur les quatre `FOLDERID` (D6), parcours récursif.
- [ ] **Step 3 :** 🔴 **`IShellLink::Resolve` n'apparaît NULLE PART**, et le
      fichier porte la raison **auprès de l'appel qu'il ne fait pas** : il peut
      interroger le réseau et **déclencher l'installation à la demande d'un
      raccourci MSI publié** pendant une réconciliation de routine (spec D2).
      **Contrôle prescrit et ATTEIGNABLE** :
      `grep -rn 'Resolve' agent/src/apps/` doit rendre **zéro** — hors le
      commentaire qui l'interdit. La rouge se joue en l'appelant.
- [ ] **Step 4 :** ⚠️ **Un `.lnk` illisible est SAUTÉ avec sa trace**, la
      réconciliation continue (spec §7). **Le test de cette propriété n'existe
      pas sur l'hôte** — le module est `#[cfg(windows)]` — et c'est déclaré :
      elle est éprouvée par la tâche 20, en posant un fichier `.lnk` de zéro
      octet sur le Bureau, et le catalogue doit rester complet.
- [ ] **Step 5 :** `cargo check --target x86_64-pc-windows-gnu`, sortie 0.
      **Relever le nombre d'avertissements et le comparer à la référence de la
      tâche 1** ; tout avertissement neuf est nommé.
- [ ] **Step 6 :** `wc -l agent/src/apps/lecture.rs` ; porte à 450.

---

### Task 8 : le lancement par `ShellExecuteExW`, et son repli

**Objet :** honorer un `Lancer` comme un double-clic le ferait (spec D6).

**Files:**
- Create: `agent/src/apps/lancement.rs`
- Modify: `agent/src/apps.rs`

- [ ] **Step 1 : implémenter.**
      - `SHELLEXECUTEINFOW { lpFile: <le chemin du .lnk>, nShow: <GetShowCmd>,
        fMask: SEE_MASK_NOASYNC, .. }` puis `ShellExecuteExW` ;
      - 🔴 **jamais de ligne de commande reconstruite** : « Reconstruire, c'est
        réintroduire un analyseur de ligne de commande maison, c'est-à-dire le
        défaut qu'on retire à `lnkParser.js` » (spec D6). Le commentaire porte
        cette phrase **auprès du code**, pas dans un rapport ;
      - repli : si le `.lnk` a disparu, tenter la **cible enregistrée**. Les
        **deux** tentatives sont journalisées.
- [ ] **Step 2 :** rendre `IssueLancement` : `Raccourci` si la première réussit,
      `Cible` si le repli réussit, `Echec` si les deux tombent. **`Inconnue` est
      rendue par l'appelant** (tâche 9), qui seul connaît le catalogue.
- [ ] **Step 3 :** 🔴 **Le processus lancé n'entre dans AUCUN job object**, et
      le fichier porte la raison **et le renvoi** :
      `agent/src/superviseur/lanceur.rs:98` pose
      `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, dont l'objet est que la mort du
      superviseur ne laisse pas N agents derrière lui — **et ce raisonnement ne
      se transpose pas** à une application de l'utilisateur (spec D8, D14).
      **Contrôle prescrit et atteignable** :
      `grep -rn 'AssignProcessToJobObject' agent/src/apps/` rend **zéro**.
- [ ] **Step 4 :** ⚠️ **`SEE_MASK_NOASYNC` est requis** parce que
      `ShellExecuteEx` peut rendre la main avant que le processus enfant ne soit
      créé, sur un fil qui se termine ensuite. **Non mesuré ici** — c'est une
      lecture de la documentation Windows, et elle est déclarée comme telle.
- [ ] **Step 5 :** `cargo check --target x86_64-pc-windows-gnu`, sortie 0.

---

### Task 9 : `agent/src/apps.rs` — la boucle, `PERIODE_RECONCILIATION`, `APPS=0`

**Objet :** assembler les quatre modules, et produire le `Catalogue`.

**Files:**
- Modify: `agent/src/apps.rs`

- [ ] **Step 1 : implémenter.**
      - `brancher(canal)` lit `APPS` : `Ok(v) if v == "0"` ⇒ `warn!` **et
        `None`**. 🔴 **`=0` désarme ; une simple présence n'active pas** (D11) —
        tester `is_ok()` activerait le mécanisme en écrivant `APPS=0` pour le
        couper ;
      - un `spawn_blocking` dédié, `CoInitializeEx` une fois, boucle
        `PERIODE_RECONCILIATION` ;
      - à chaque tour : lire, filtrer, journaliser **les changements** d'écart
        (D13), diffuser, **émettre** ;
      - **`complet = true` à chaque (ré)enrôlement** (D3) : la boucle observe le
        `watch::Receiver<Option<Identite>>` du canal — un changement d'identité
        **est** un réenrôlement ;
      - consommer les ordres `Lancer` : résoudre la clé dans le catalogue
        courant, appeler `lancement`, répondre `Lancee`. `Inconnue` si la clé
        est absente.
- [ ] **Step 2 :** traces de contrôle, **nommées d'avance parce que la recette
      les `grep`era** :
      - `catalogue reconcilie total=… retenus=… cles=… apparues=… disparues=… duree_ms=…`
      - `raccourci ecarte chemin="…" motif=cible-vide` (et `extension=…`,
        `cible-absente`)
      - `raccourci reintegre chemin="…"`
      - `decouverte d'applications DESARMEE (APPS=0)`
      - `lancement demande=… cle=… issue=raccourci`
- [ ] **Step 3 :** ⚠️ **Le journal est PARTAGÉ** par le superviseur, le capteur
      et tous les enfants depuis D4. **Toutes ces traces portent une cible
      identifiable** (`chemin=`, `cle=`), et la ligne de bilan est **unique par
      tour**. C'est la leçon de D6 : « une trace non attribuable coûte une
      ré-imputation ».
- [ ] **Step 4 :** `cargo check --target x86_64-pc-windows-gnu` ; `wc -l`, porte
      à 450.

---

# Famille 3 — le canal côté agent

### Task 10 : le canal `/agent` devient BIDIRECTIONNEL

**Objet :** fermer E2 — donner à `Canal` une file d'émission et un flux
d'ordres.

**Files:**
- Modify: `agent/src/plateforme.rs`, `agent/src/plateforme/tests.rs`

- [ ] **Step 1 : écrire les tests d'abord, et les voir ROUGES.** Le fichier de
      tests **tient déjà un vrai serveur WebSocket local** (196 lignes) : les
      cas neufs s'y greffent.

| Test | Ce qui le rend ROUGE |
| --- | --- |
| un message poussé dans la file **arrive** au serveur | ne pas drainer la file dans le `select!` : elle grossirait sans fin, et rien ne le dirait |
| un `Lancer` reçu **arrive** au consommateur d'ordres | l'oublier : il tomberait dans le bras `Err` (« message illisible »), qui **ferme la session** — donc une reprise en boucle sur un ordre parfaitement valide |
| 🔴 le `Lancer` **ne ferme pas** la session, et le battement continue après | traiter tout message non reconnu comme une divergence de version |
| un message mis en file pendant que le socket est **tombé** est **perdu**, et le canal ne meurt pas | 🔴 **c'est le comportement VOULU, et le test l'assène** : le rendre bloquant ferait de la file une fuite mémoire ; le rendre fatal tuerait le canal sur une coupure réseau ordinaire |
| l'identité **change** à chaque réenrôlement, et le `watch` l'annonce | ne pousser l'identité qu'une fois : la tâche 9 ne saurait pas qu'il faut renvoyer le catalogue complet |

- [ ] **Step 2 : implémenter.** Une `tokio::sync::mpsc` **bornée** pour
      l'émission — bornée et non illimitée, parce qu'une file illimitée sur un
      canal qui peut rester coupé est une fuite —, drainée dans le `select!` de
      `une_session` ; une `mpsc` non bornée pour les ordres reçus (le
      consommateur les traite en `spawn_blocking`, il ne doit pas bloquer la
      boucle du canal).
- [ ] **Step 3 :** ⚠️ **`Canal::tache` porte `#[allow(dead_code)]`**
      (`agent/src/plateforme.rs:63`) : vérifier que l'attribut reste **justifié**
      après la modification, ou le retirer. Un `allow` devenu inutile est une
      affirmation devenue fausse.
- [ ] **Step 4 :** `wc -l agent/src/plateforme.rs` ; porte à 450 — si franchie,
      extraire vers `agent/src/plateforme/file.rs`.
- [ ] **Step 5 :** `cargo test -p agent plateforme` **et**
      `cargo check --target x86_64-pc-windows-gnu`.

---

### Task 11 : le branchement dans `main.rs`

**Objet :** appeler `apps::brancher` entre l'enrôlement et l'aiguillage
`SUPERVISEUR` (D14).

**Files:**
- Modify: `agent/src/main.rs`

- [ ] **Step 1 :** `wc -l agent/src/main.rs` → attendu **435**, marge **65**.
      🔴 **Si l'addition dépasse 15 lignes, elle descend dans
      `agent/src/apps.rs`** : `main.rs` ne reçoit qu'un appel et la garde de sa
      `JoinHandle`.
- [ ] **Step 2 : implémenter.** L'appel se place **après** la ligne 424 (fin de
      l'enrôlement) et **avant** la ligne 430 (aiguillage `SUPERVISEUR`), et le
      résultat est lié comme `_apps` — même figure que `_canal_plateforme`
      (`main.rs:399`), et **avec le même commentaire disant pourquoi** : le
      lâcher arrêterait la boucle.
- [ ] **Step 3 :** ⚠️ **Sans `AGENT_VM`/`AGENT_SECRET`, il n'y a pas de canal**
      (`main.rs:417-421` journalise et rend `None`). **La découverte ne démarre
      alors pas non plus**, et cela se journalise : `decouverte d'applications
      inactive : aucun canal /agent`. **Un silence ici se lirait comme
      « `APPS=0` », qui est un état différent.**
- [ ] **Step 4 :** `cargo check --target x86_64-pc-windows-gnu`.

---

### Task 12 : 🔴 la ligne de `scripts/run-agent.sh` — TÂCHE DÉDIÉE

**Objet :** transmettre `APPS` à l'agent. **Cette tâche ne fait rien d'autre.**

**Files:**
- Modify: `scripts/run-agent.sh`

- [ ] **Step 1 :** `grep -n 'AGENT_VM' scripts/run-agent.sh` → **90**, et
      `AGENT_SECRET` → **91**. **Relever le nombre AVANT d'écrire ; ce plan ne
      fait pas foi.**
- [ ] **Step 2 :** ajouter, sur le modèle exact des lignes voisines :
      `${APPS:+\$env:APPS = '$APPS'}`
- [ ] **Step 3 :** 🔴 **Vérifier que la valeur ATTEINT le processus**, pas que
      le code la lirait si elle y était. C'est le piège d'`AUDIO` en D7 :
      implémenteur et relecteur avaient vérifié la propriété **en traçant le
      code**, le tracé était juste, et la valeur ne pouvait simplement pas
      arriver. **La vérification se fait à la tâche 20**, en lançant avec
      `APPS=0` et en cherchant la trace `decouverte d'applications DESARMEE`.
      **Elle est inscrite ici pour qu'on ne la déclare pas faite avant.**

---

# Famille 4 — la plateforme, persistance

### Task 13 : `0004-applications.sql`, et les deux gardes de P1 qui doivent la voir

**Objet :** élargir `application` par `ALTER TABLE` (D5), et faire rougir puis
verdir les deux gardes.

**Files:**
- Create: `plateforme/src/base/migrations/0004-applications.sql`
- Modify: `plateforme/src/base/pilotes.test.ts`

- [ ] **Step 1 : écrire la migration.** ⚠️ **Aucune apostrophe, aucun
      guillemet, commentaires compris** — `sous-ensemble.test.ts:87` les refuse
      et `rendreMarqueurs` lèverait côté Postgres. `0003-agents.sql` est
      **entièrement désaccentué** pour cela : faire de même.
      Le commentaire de tête porte, en clair : les colonnes NOT NULL naissent
      **maintenant** parce que SQLite refuse d'en ajouter à une table peuplée
      (E8, avec le message exact), et `apparue_a` **n'a aucun lecteur avant G3**
      (D5).
- [ ] **Step 2 : 🔴 voir les gardes ROUGES, dans cet ordre**, chacune restaurée
      après :

| Rouge à jouer | Message attendu |
| --- | --- |
| lancer `test:sqlite` **sans** toucher `pilotes.test.ts:33` | `expected [1,2,3,4] to equal [1,2,3]` — **c'est E9, la rouge gratuite** |
| écrire `ADD COLUMN cle TEXT UNIQUE` | `Cannot add a UNIQUE column` **sur SQLite seulement** — la passe Postgres reste VERTE. 🔴 **Deux passes, un seul rouge : c'est exactement ce que la double passe existe pour attraper**, et il faut le voir |
| écrire `ADD COLUMN cle TEXT NOT NULL DEFAULT ''` | `sous-ensemble.test.ts` : `0004-applications.sql ne porte aucune chaîne littérale` |
| écrire `disparue_a INTEGER` | `un horodatage \`_a\` en INTEGER : 4 octets sur Postgres…` |

- [ ] **Step 3 :** porter `pilotes.test.ts:33` à `[1, 2, 3, 4]` et `:38` à
      `toHaveLength(4)`.
- [ ] **Step 4 : ajouter un cas à `pilotes.test.ts`** qui écrit et relit une
      ligne `application` complète avec **`1_787_136_773_742`** dans `vue_a`,
      `apparue_a` et `disparue_a`, sur **les deux moteurs**. 🔴 **L'angle mort
      de la double passe est qu'elle n'écrivait que de petites valeurs**
      (`pilotes.test.ts:101-110`) : une valeur de `1_000` déclarerait portable un
      schéma qui refuse toute écriture réelle.
- [ ] **Step 5 : ajouter un cas** qui prouve que
      `CREATE UNIQUE INDEX application_cle` **mord** : deux insertions de même
      `(vm_id, cle)` sont refusées, et deux de même `cle` sur des `vm_id`
      **différents** passent. 🔴 **Sans le second, l'index pourrait être sur
      `cle` seule et le test resterait vert.**
- [ ] **Step 6 :** `npm run test:sqlite` **et** `npm run test:postgres`, verts
      tous les deux. Comptes annoncés avant lecture : **194 + 3**.

---

### Task 14 : le dépôt `application.ts`

**Files:**
- Create: `plateforme/src/depot/application.ts`, `…/application.test.ts`

- [ ] **Step 1 : écrire les tests d'abord.** Ils tournent sur les **deux**
      moteurs par le harnais.

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `lireParVm` ne rend **que** les applications de cette VM | omettre le `WHERE vm_id = ?` |
| `lireParVm` **exclut** les disparues et les masquées | les inclure : le catalogue affiché porterait des applications qui n'existent plus |
| `appliquer` insère, met à jour, et **pose `disparue_a` SANS SUPPRIMER LA LIGNE** | 🔴 **c'est le critère ③, et le test lit LES DEUX** : `disparue_a` non nul **et** la ligne toujours présente. Un `DELETE` ferait perdre son identifiant à la PWA installée (spec D1) |
| `appliquer` **ressuscite** une ligne disparue : `disparue_a` repasse à NULL, et **l'`id` ne change pas** | insérer une ligne neuve : c'est la même perte d'identifiant, par une autre porte |
| l'`id` est un `randomUUID()` **généré dans le dépôt** | le laisser venir de l'agent : deux VMs pourraient en produire le même |
| aucune valeur littérale dans aucune requête | 🔴 **cette moitié du lint ne lève que sous Postgres** (`rendreMarqueurs` n'est appelée que par `pilote-postgres.ts`) : **la rouge se joue sur les DEUX moteurs**, et n'est visible que sur un |

- [ ] **Step 2 : implémenter**, sur le modèle exact de
      `plateforme/src/depot/agent.ts` : `p: Pilote` en premier paramètre,
      horloge en paramètre, `COLONNES` factorisée, `undefined` sur ligne absente
      **jamais une exception**.
- [ ] **Step 3 :** comptes annoncés avant lecture ; les deux passes vertes.

---

### Task 15 : la fusion de catalogue, PURE

**Objet :** décider, sans base et sans horloge lue, ce qu'il faut écrire.

**Files:**
- Create: `plateforme/src/apps/catalogue.ts`, `…/catalogue.test.ts`

- [ ] **Step 1 : écrire les tests d'abord, et les voir ROUGES**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `complet: true` marque disparue **toute** ligne connue absente du message | ne traiter que `disparues` : une application supprimée pendant que le canal était coupé resterait au catalogue **pour toujours** |
| `complet: false` **n'invente aucune disparition** | 🔴 la rouge inverse : traiter un delta comme un état complet **viderait le catalogue** à chaque message qui ne porte qu'une apparition |
| `complet: true` avec `applications: []` **vide** le catalogue | le traiter comme « rien à faire » : une VM dont on désinstalle tout resterait pleine. **C'est le cas `catalogue_vide` des vecteurs (T4)** |
| une ligne connue **et** présente va dans `aMettreAJour`, pas dans `aInserer` | comparer sur l'`id` au lieu de la `cle` : l'agent ne connaît pas les `id` |
| une ligne connue **disparue** et présente va dans `aRessusciter` | l'insérer : perte d'identifiant (T14) |
| la fusion est **pure** : aucun `Date.now()`, aucun accès base | 🔴 **contrôle atteignable** : `grep -n 'Date.now\|Pilote' plateforme/src/apps/catalogue.ts` rend zéro |

- [ ] **Step 2 : implémenter.** Précédent de la figure : `agents/fraicheur.ts`.
- [ ] **Step 3 :** compte annoncé avant lecture : **6**.

---

# Famille 5 — la plateforme, canal et routes

### Task 16 : `registre.ts` — les sockets d'agent vivants

**Objet :** fermer E4.

**Files:**
- Create: `plateforme/src/agents/registre.ts`, `…/registre.test.ts`

- [ ] **Step 1 : écrire les tests d'abord, et les voir ROUGES**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `lancer` sur une VM **absente** rend `agent-injoignable` **immédiatement** | attendre `DELAI_LANCEMENT_MS` pour rien |
| `lancer` **encode et envoie** un `Lancer` sur le socket, avec la `demande` | recopier la forme du message au lieu d'appeler l'encodeur de `proto/ts/plateforme.ts` : « une copie divergerait en silence » (`agents/canal.ts:3-7`) |
| `resoudre(demande, issue)` **débloque** l'attente avec cette issue | apparier sur la `cle` : deux lancements concurrents de la même application se mélangeraient |
| une `demande` **inconnue** de `resoudre` est **ignorée avec sa trace**, jamais une exception | lever : une promesse rejetée dans un gestionnaire `ws` **abat tout le process Node** — mode de défaillance que `relais.ts`, `trace.ts` et `canal.ts` documentent tous les trois |
| 🔴 `retirer` **rejette les demandes en vol** de cette VM | ne rien faire : la route attendrait 5 s après une mort d'agent instantanément connue |
| 🔴 une seconde inscription de la **même** VM ferme la première et la remplace | garder les deux : « laquelle serait la bonne ? » (`0003-agents.sql:18-20`) |
| l'expiration à `DELAI_LANCEMENT_MS` rend `delai` — **horloge injectée**, et le test la fait AVANCER | figer l'horloge : le test devient inerte. **Le test doit VOIR la transition**, pas lire un état final — c'est la leçon du critère ④ de P3 |

- [ ] **Step 2 : implémenter.** Aucune dépendance ; `WebSocket` est déjà là.
- [ ] **Step 3 :** compte annoncé avant lecture : **7**.

---

### Task 17 : `canal.ts` — les deux branches neuves et l'inscription au registre

**Files:**
- Modify: `plateforme/src/agents/canal.ts`, `plateforme/src/agents/canal.test.ts`

- [ ] **Step 1 : écrire les tests d'abord, et les voir ROUGES**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| 🔴 un `catalogue` **avant tout enrôlement** est refusé `sequence` | l'accepter : un pair anonyme écrirait dans la table `application` d'une VM qu'il n'a pas authentifiée. **C'est le trou exact que le refus `sequence` du battement ferme déjà** (`canal.ts:121-130`), par une autre porte |
| un `lancee` avant tout enrôlement est refusé `sequence` | idem |
| un `catalogue` valide **écrit** par la fusion et le dépôt | — |
| l'écriture est **lancée sans être attendue, avec son `catch`** | 🔴 `await` dans le gestionnaire `message` : une base momentanément indisponible **abattrait la connexion d'un agent qui va très bien** (`canal.ts:133-137`). Et une promesse rejetée **sans `catch`** abat le process |
| l'enrôlement **inscrit** la VM au registre | l'oublier : `lancer` rendrait toujours `agent-injoignable` |
| la fermeture du socket **retire** la VM du registre | l'oublier : une VM morte resterait « joignable » jusqu'au prochain enrôlement |
| un `lancee` **résout** la demande | — |

- [ ] **Step 2 : implémenter.** `servirLeCanalAgent` gagne `registre` dans
      `OptionsCanal`.
- [ ] **Step 3 :** `wc -l plateforme/src/agents/canal.test.ts` → **361** au
      départ. 🔴 **Il franchira 450.** Extraire **AVANT** l'addition, pas après :
      `canal.test.ts` (cycle de vie, P3) et `canal-apps.test.ts` (G1). **C'est
      la seule extraction de cette famille, et elle précède son addition**,
      comme la tâche 1 précède la tâche 2.
- [ ] **Step 4 :** les deux passes vertes ; comptes annoncés avant lecture.

---

### Task 18 : `routes-applications.ts` — `GET /applications` et `POST /application/:id/lancer`

**Files:**
- Create: `plateforme/src/http/routes-applications.ts`, `…/routes-applications.test.ts`

- [ ] **Step 1 : écrire les tests d'abord, et les voir ROUGES**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| un chemin étranger rend `false` (le 404 du serveur suit) | rendre `true` : la route mangerait les 404 des autres |
| 🔴 le motif de `/application/:id/lancer` est **ancré des deux bouts** : `/application/x/lancer/y` n'est PAS servi | un `startsWith` : « un préfixe ouvrirait une famille entière de chemins que personne n'a décidés » (`serveur.ts:127-129`) |
| **sans** en-tête `Authorization` → `401 { refus: 'jeton' }` | omettre le contrôle : les deux routes de P1/P2 sont ouvertes par construction, et **rien** dans le dépôt n'authentifie une requête HTTP (E5) |
| avec un jeton **d'agent** (claim `sty = agent`) → `401` | 🔴 accepter tout jeton valide : agent et humain sont signés par **le même secret**, et sans le claim de type ils sont **interchangeables** — c'est E5 de P3, et ce serait le rouvrir |
| avec un jeton **expiré** → `401` | `verifierJeton` s'en charge ; le test le prouve avec une horloge **qui avance** |
| `GET /applications?vm=<id>` rend les applications de cette VM | — |
| VM attribuée à **un autre** utilisateur → `403 { refus: 'vm-etrangere' }` | l'omettre : tout utilisateur authentifié verrait tout. ⚠️ **Ce test se joue en posant `vm.utilisateur_id` à la main**, puisque rien ne le remplit avant P4 |
| VM **non attribuée** → servie, **et la ligne de journal est émise** | 🔴 servir en silence : l'absence d'isolation deviendrait invisible (D9). **Le test lit la trace**, pas seulement le code de réponse |
| `POST …/lancer` sur une application **inconnue** → `404` | — |
| `POST …/lancer` avec agent absent → `503 { refus: 'agent-injoignable' }` | rendre `200` : le hub afficherait un succès pour un lancement qui n'a pas eu lieu |
| `POST …/lancer` qui expire → `504 { refus: 'delai' }` | rendre `202` sans attendre : **le critère ⑤ passerait sur un binaire qui n'a rien lancé** (D4) |
| `POST …/lancer` réussi rend `{ issue: 'raccourci' }` | aplatir l'issue en booléen : **le critère ⑤ perd sa discrimination** (E13) |

- [ ] **Step 2 : implémenter**, sur le modèle exact de `servirAuth` :
      `Promise<boolean>`, dépendances injectées (`base`, `secretJeton`,
      `registre`, `maintenant`), refus toujours de la forme `{ refus: … }`, et
      les en-têtes CORS posés par `entetesCors` comme `routes-auth.ts:101`.
- [ ] **Step 3 :** ⚠️ **Le plafond de corps de `routes-auth.ts:44` (4 Kio) ne
      s'applique PAS ici** : ces routes n'ont pas de corps significatif. **Ne
      surtout pas relever celui-là** — c'est la consigne littérale de la spec D7,
      posée pour G3.
- [ ] **Step 4 :** comptes annoncés avant lecture ; deux passes vertes.

---

### Task 19 : `cors.ts` et le chaînage dans `serveur.ts`

**Files:**
- Modify: `plateforme/src/http/cors.ts`, `…/cors.test.ts`, `…/serveur.ts`,
  `…/serveur.test.ts`

- [ ] **Step 1 :** `cors.ts:36` → `'GET, POST, OPTIONS'`. `cors.test.ts` fige la
      **nouvelle** valeur. 🔴 **La rouge est gratuite** : sans `GET`, le
      préflight d'un `GET /applications` cross-origin est refusé, et le test le
      voit.
- [ ] **Step 2 :** chaîner `servirApplications` **avant** le 404 de
      `serveur.ts:64-65`. ⚠️ **Ne pas changer le corps du 404** : « rien ne le
      testait avant P2, et le changer serait un effet de bord non déclaré »
      (`serveur.ts:52-54`), et `routes-auth.test.ts` le fige désormais.
- [ ] **Step 3 :** construire le `RegistreAgents` **une fois** dans
      `demarrerServeur`, à côté de `gardeDuService` et de
      `new ProprieteDeSession()` (`serveur.ts:97`), et le passer aux deux
      consommateurs. **Écrire son coût au même endroit que celui de
      `ProprieteDeSession` : il ne survit pas à un redémarrage.**
- [ ] **Step 4 :** un test de `serveur.test.ts` qui prouve que la route est
      **réellement branchée** — 🔴 **c'est la même ligne que
      `serveur.ts:98-102` documente pour le canal** : « sans elle, le pair
      verrait une connexion réussie, puis un silence ». La rouge : retirer le
      chaînage, et lire `404 introuvable` là où on attend `401`.
- [ ] **Step 5 :** `./scripts/verify-all.sh` **en entier**, les dix étapes.
      ⚠️ **Il peut échouer pour une cause étrangère** — P2 l'a vécu, son étape 1
      tombant sur un test Rust du chantier voisin. Si c'est le cas, **le
      déclarer**, jouer les étapes TypeScript à part, et rejouer le témoin
      complet à la clôture sur un `git worktree` du dernier commit de G1.

---

# Famille 6 — recette et clôture

### Task 20 : la recette sur la VM — six critères, DEUX exécutions chacun

**Objet :** juger les six critères de la spec §5 « G1 », en conditions de
produit.

**Files:**
- Create: `docs/superpowers/plans/journaux-gestion-apps/*.log`, `*.json`
- Modify: `agent/testdata/gapps-corpus-vm.json` (remplacé par un relevé de
  l'agent — voir T5 step 1)

⚠️ **Cette tâche EMPLOIE la VM Windows** — la seule du plan. **Ne pas la
démarrer** tant qu'un autre chantier la tient.

- [ ] **Step 0 — la VM.** `virsh list --all` ; `virsh start Windows` si besoin ;
      attendre WinRM (`until timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985'`)
      **puis** le montage — `until ls /media/vm/dev >/dev/null 2>&1`, jamais
      `mountpoint -q` : l'entrée CIFS survit à une VM éteinte.
- [ ] **Step 1 — le binaire.** `set -a && source .env && set +a` **avant**
      `scripts/build-agent.sh` : sans cela le script **s'arrête en silence**
      après « sources synchronisées », et le symptôme se lit exactement comme
      une compilation réussie et muette. **Vérifier la TAILLE du binaire** — une
      compilation de 0,13 s est un aveu. `Get-Process agent` **avant** de lancer,
      et **après chaque tentative, y compris échouée** : un pilote qui laisse un
      superviseur vivant bloque silencieusement la suivante, et **la copie relue
      est celle, périmée, de la tentative précédente**.
- [ ] **Step 2 — la plateforme et l'enrôlement.** Service relancé **au même
      commit** ; `npm run admin:agent -- --vm g1 --adresse 192.168.3.2` ;
      `npm run admin:utilisateur` puis `POST /auth/connexion` pour obtenir un
      jeton d'utilisateur. `AGENT_VM` et `AGENT_SECRET` posés.
- [ ] **Step 3 — la MESURE DE VERSION, avant tout critère.** Lancer un agent
      **v1** (le binaire d'avant G1, conservé) contre la plateforme v2 : le
      journal doit porter `la plateforme REFUSE la version du canal /agent :
      aucune reprise` (`agent/src/plateforme.rs:257`). 🔴 **C'est la rouge de
      D10, et elle est gratuite.** Une exécution suffit ; **le dire.**

**Les six critères, DEUX exécutions chacun, journaux versés :**

| # | Critère | Comment il est jugé | La ROUGE, nommée et ATTEIGNABLE |
| --- | --- | --- | --- |
| ① | Le catalogue compte **154 applications pour 218 raccourcis** | `GET /applications?vm=…` compté, et la ligne `catalogue reconcilie total=… retenus=… cles=…` du journal. ⚠️ **Compté AVANT toute création de raccourci de recette** ; chaque création suivante décale les trois nombres, et le décalage attendu s'annonce avant d'être lu | 🔴 **Jouée sur l'HÔTE, sans VM** : la clé par la cible seule rend **104** sur le corpus versé (T5). **Écart déjà mesuré**, et le test le dénonce. ⚠️ **« Déjà mesuré » ne dispense PAS de jouer la rouge du TEST** : ce qui est mesuré est le fait du monde, ce qui reste à voir rouge est que le contrôle le dénonce |
| ② | Un raccourci **neuf** apparaît sans redémarrer l'agent | créer un `.lnk` sur le Bureau, attendre au plus `2 × PERIODE_RECONCILIATION`, relire ; l'horodatage `apparue_a` est postérieur au démarrage | figer le catalogue au démarrage : le compte ne bouge pas. **À exercer** |
| ③ | Un raccourci **retiré** quitte le catalogue **sans que sa ligne disparaisse** | `GET /applications` ne la rend plus **ET** `SELECT` direct montre la ligne avec `disparue_a` non nul. 🔴 **Le test lit LES DEUX** | supprimer la ligne : le second contrôle tombe. Rouge déjà jouée en T14 sur l'hôte ; **rejouée ici sur le chemin réel** |
| ④ | Les raccourcis **sans cible** sont **exclus ET journalisés** | `grep -c 'raccourci ecarte' ` sur la **première** réconciliation, **bornée temporellement** (D13) ; le compte attendu est **7**, annoncé avant lecture | les exclure en silence : le compte est 0. ⚠️ **Si le compte réel n'est pas 7, c'est le corpus de la spec qui a vieilli, et l'écart s'écrit** (E16) — il ne se corrige pas en ajustant le `grep` |
| ⑤ | Un lancement ouvre **la bonne fenêtre**, **par le raccourci** | `POST /application/:id/lancer` sur Bloc-notes ; la réponse porte `{ issue: 'raccourci' }` **ET** le superviseur journalise une fenêtre `notepad.exe` | 🔴 **Deux assertions, deux rouges.** Lancer par la cible reconstruite rend `issue: 'cible'` — c'est D4/E13, et c'est ce qui répare le critère que la spec déclarait non discriminant. Et un `202` sans attente ferait passer un binaire qui n'a rien lancé |
| ⑥ | Le lancement honore le **répertoire de travail** | un `.lnk` vers `cmd.exe /c cd > sortie.txt` avec un `WorkingDirectory` posé ; `sortie.txt` atterrit **là** et porte **ce** chemin | reconstruire la ligne de commande sans le répertoire : le fichier atterrit ailleurs. **À exercer** |

- [ ] **Step 4 — les relevés annexes**, qui ne sont pas des critères et qui se
      disent comme tels :
      - la **durée** d'une réconciliation complète (`duree_ms` du journal), à
        opposer aux **113 ms** de la spec §3.1 — **une autre voie de mesure, un
        autre chiffre attendu** ;
      - la **taille** d'un `Catalogue` complet sur le fil, à opposer aux
        ~46 Kio **calculés** de D3 ;
      - le **compte de l'agent** (218 / 167 / 154) contre celui de la sonde
        `WScript.Shell` de la spec. 🔴 **Un écart est une mesure** (E16) ;
      - un `.lnk` de **zéro octet** posé sur le Bureau : le catalogue reste
        complet, une trace le nomme (spec §7, T7 step 4) ;
      - un lancement avec **`APPS=0`** : la trace
        `decouverte d'applications DESARMEE` apparaît, et le catalogue reste
        vide. 🔴 **C'est la vérification de la tâche 12**, et elle ne peut se
        faire qu'ici.
- [ ] **Step 5 — remplacer le corpus versé** par le relevé de l'agent, et
      **changer son champ `provenance`**. Relancer `cargo test -p agent
      apps::raccourci` : si un chiffre bouge, **il s'écrit**, il ne se corrige
      pas.
- [ ] **Step 6 :** ⛔ **Ne PAS prendre de capture d'écran CDP pendant une
      mesure** — elle provoque un `Resize`, donc un `SHOW`, donc une session et
      une sortie de plus (D1/D2). Ici la contrainte est moindre (aucune session
      média n'est requise), **mais toute évaluation CDP sur une page portant un
      flux WebRTC actif peut ne JAMAIS rendre.**
- [ ] **Step 7 :** vérifier que la VM a survécu **après chaque rang** :
      `grep -E "terminating on signal|shutting down" /var/log/libvirt/qemu/Windows.log | tail -4`.
      **Elle s'hiberne toute seule**, deux mesures ont déjà été perdues ainsi.

---

### Task 21 : la revue transverse de fin de branche — OBLIGATOIRE

**Objet :** trouver **les affirmations — commentaires, documents, `CLAUDE.md` —
que la branche G1 elle-même a rendues fausses.**

**Elle n'est jamais facultative.** Barème : **5** défauts en D7, **3** en D8,
**6** en D9, **douze** en D10, **sept** en D11, **huit** en P1, **dix** en P2
(réparties sur **vingt-trois places**), **cinq** en S1, **neuf** dans le
chantier E.

⚠️ **Les deux comptes ne mesurent pas la même chose** : le nombre de **défauts**
est ce qu'un relecteur trouve ; le nombre de **places** est ce qu'il faut
éditer, et c'est le second qui coûte — parce que « corrigé à sa place » est une
affirmation de **COMPLÉTUDE**, et que ce dépôt a payé **neuf fois** pour un
nombre laissé dans une place non balayée.

**Cibles nommées d'avance** — toutes relues ce jour :

| Place | Ce qui devient faux, ou change de portée |
| --- | --- |
| `plateforme/src/base/migrations/0003-agents.sql:43-44` | « `application` est creee par P3 et **RESTE VIDE** : son chemin d'ecriture est le sous-projet ④ » — **④ l'a pris** |
| `plateforme/src/base/migrations/0003-agents.sql:46-51` | « Ce n'est donc PAS un oubli si aucun code ne l'ecrit » — **du code l'écrit** |
| `plateforme/src/agents/canal.ts:1-29` | l'en-tête décrit un canal à **deux** messages montants et **trois** descendants |
| `plateforme/src/agents/canal.ts:9-13` | « CE CANAL NE PARTAGE RIEN AVEC LE RELAIS — ni garde, ni **registre d'appartenance** » : il partage désormais **un autre** registre, à ne pas confondre |
| `proto/src/plateforme.rs`, en-tête | « v1 (sous-bloc P3) : enrôlement, battement de cœur, jeton d'agent » |
| `proto/ts/plateforme.ts:53` et `:79-83` | les commentaires de `TYPES_DEPUIS` / `TYPES_VERS`, si les listes deviennent dérivées |
| `agent/src/plateforme.rs:1-17` | l'en-tête : le canal « porte une IDENTITÉ » — il porte désormais aussi un catalogue et des ordres |
| `agent/src/plateforme.rs:57-65` | la doc de `Canal` : « Le lâcher arrête le battement de cœur » — il arrête aussi la découverte |
| `plateforme/src/agents/fraicheur.ts:11-20` | « ce qu'il decide n'est lu par personne tant qu'aucune vue ne liste les VMs, ce qui est le sujet de P4 » — **vérifier que G1 ne l'a pas rendu faux** |
| `client/src/prefixe.ts:11-17` | « P4 branchera la source » — **G1 n'y touche pas**, vérifier |
| `docs/…/specs/2026-08-19-gestion-apps-design.md` §3.3 | la mesure `ADD COLUMN` — **à annoter par E7 et E8**, jamais à réécrire : c'est un relevé daté |
| `docs/…/specs/2026-08-19-gestion-apps-design.md` §5 « G1 », critères ④ et ⑤ | E12 et E13 |
| `docs/…/specs/2026-08-19-gestion-apps-design.md` §6 | `plateforme/` au § Portée (E1), `client/src/hub/` en G1 (E14) |
| `docs/…/plans/2026-08-19-plateforme-p3.md` § « Structure des fichiers » | « ses **neuf** étapes » — `verify-all.sh` en compte **dix** |
| `CLAUDE.md` section P3, legs | « Aucune écriture dans `application` : ④ empruntera le canal » |
| `src/app.js`, `src/lnkParser.js`, `src/iconExtractor.js` | ⛔ **NE PAS TOUCHER** — le produit historique est hors périmètre (cadrage §11). Ils sont nommés ici pour dire qu'ils ne bougent pas |

- [ ] **Step 1 :** `grep -rn "sous-projet ④\|reste vide\|RESTE VIDE\|c'est ④\|avant ④\|G1\|P4 branchera" plateforme/src proto/src proto/ts agent/src client/src docs/superpowers/specs/2026-08-19-gestion-apps-design.md docs/superpowers/plans/2026-08-19-plateforme-p3.md CLAUDE.md`
      — **énumérer les places AVANT d'éditer**, et **relire `grep -n` place par
      place APRÈS l'édition**. Une substitution qui ne dit pas combien
      d'occurrences elle a touchées est une affirmation de complétude non
      vérifiée. **La leçon de D10 n'est pas « mesurer » — c'est que `grep -n`
      doit être relu APRÈS l'édition, pas seulement lancé avant.**
- [ ] **Step 2 :** 🔴 **Chercher par le SENS, pas par la formule.** Une négation
      se dit de plusieurs façons, et c'est celle qu'on n'a pas listée qui
      survit. Balayer sur la **chose** (le canal, la table, la version, le
      registre), pas sur la tournure.
- [ ] **Step 3 :** vérifier par la commande ce que G1 **n'a pas** touché :
      `git diff --stat -- agent/src/capteur/ agent/src/superviseur/ src/ web/ client/src/`
      (E18, D12). **Un « nous n'y avons pas touché » qui n'est pas mesuré est
      une croyance.**
- [ ] **Step 4 :** relever le nombre de **défauts** et le nombre de **places**,
      séparément.

---

### Task 22 : `CLAUDE.md` gagne sa section G1

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1 :** écrire la section sur le modèle des sections P1 à P3 : le
      fait n°1 (le catalogue naît et se lance), la variable neuve (`APPS`, avec
      sa convention `=0`), les six critères **avec leur nombre d'exécutions**,
      ce que G1 n'établit **pas**, les pièges neufs, les divergences E1…E18, la
      revue transverse, et les legs.
- [ ] **Step 2 :** ⚠️ **E1 est déjà vraie** : ne pas « ajouter `plateforme/` au
      § Portée », il y est (`CLAUDE.md:20`). **Le vérifier avant d'écrire.**
- [ ] **Step 3 : les tailles, RELEVÉES PAR LA COMMANDE, APRÈS la dernière
      édition de la ronde** — y compris celles de la revue transverse. **Une
      table mesurée en début de ronde est fausse à la fin de la même ronde**, et
      D8 a commis exactement cette erreur en croyant bien faire.
- [ ] **Step 4 : et il faut dire à quel COMMIT.** L'arbre est partagé.
- [ ] **Step 5 :** 🔴 `grep -n '<le nombre>' CLAUDE.md` pour **CHAQUE** chiffre
      corrigé, **avant et après**. C'est le seul geste qui ait jamais fermé le
      naufrage du « 487 ». ⚠️ **Et toucher une ligne d'un tableau de comptes
      oblige à remesurer son compte**, même quand ce n'est pas l'objet de
      l'édition : deux occurrences de D10 l'ont payé, la main sur la ligne même
      qui portait le chiffre faux.
- [ ] **Step 6 :** annoncer d'avance le compte de fichiers de
      `journaux-gestion-apps/`, puis le relever par `git ls-files`.

---

### Task 23 : clore le document de résultats

**Files:**
- Create: `docs/superpowers/plans/2026-08-19-gestion-apps-g1-resultats.md`

- [ ] Le verdict des six critères, **avec le nombre d'exécutions dans chaque
      énoncé**. **Aucun taux.**
- [ ] Chaque ROUGE jouée, **avec son message verbatim** — celles nommées dans
      les tâches 2, 3, 4, 5, 13, 16, 18, 19 et 20, plus celles qu'on n'attendait
      pas.
- [ ] Les divergences trouvées **pendant** l'exécution, en plus des E1…E18
      tranchées d'avance, et le sort de chacune des dix-huit.
- [ ] Les relevés annexes de la tâche 20 (durée, taille du message, écart de
      comptage `IShellLinkW` / `WScript.Shell`), **avec leur nombre
      d'exécutions**.
- [ ] Ce que G1 n'établit PAS (voir ci-dessous).
- [ ] Le renvoi vers chaque journal versé.
- [ ] 🔴 Un contrôle explicite qu'**aucune preuve ne vit hors de git** :
      `git ls-files docs/superpowers/plans/journaux-gestion-apps | wc -l`, et
      **le corpus `agent/testdata/gapps-corpus-vm.json` suivi par git**. D10 a
      établi par la commande que l'espace de travail de D9 a **disparu**,
      emportant six constats définitivement perdus — et la spec §11 dit que les
      sondes de ④ vivent dans `C:\dev\` sur la VM, « c'est-à-dire nulle part de
      durable ».

---

## Ce que ce plan NE prescrit PAS, et pourquoi

- **Aucune icône** : ni `IShellItemImageFactory`, ni `source_max_px`, ni lecture
  de `GRPICONDIR`, ni magasin adressé par contenu, ni `IconesManquantes`, ni
  `GET /application/:id/icone`, ni les deux fichiers `.ico` témoins. **C'est
  G2**, et le critère ② de G2 est celui qui décide de tout le §3.2 de la spec.
- **Aucun téléversement, aucune exécution d'installeur** : ni les quatre routes,
  ni les trois vérifications d'empreinte, ni `Installer`/`Progression`/`Termine`,
  ni les tables `televersement` et `installation`, ni la trace du périphérique
  de rendu, ni le verdict par réconciliation. **C'est G3** — et sa **sonde UAC
  éliminatoire** avec.
- **Aucune surveillance** : ni `ReadDirectoryChangesW`, ni anti-rebond, ni
  détection de débordement de tampon. **C'est G4**, et la spec §5 explique
  pourquoi il vient **après** G3 : la réconciliation étant la source de vérité,
  la construire en dernier garantit qu'aucun critère antérieur ne repose sur
  elle.
- **Aucune PWA, aucun `file_handler`, aucun manifeste** : **c'est G5**.
- **Aucune page de hub** (D12) : `client/` n'est pas touché.
- **Aucune isolation entre utilisateurs** (D9) : `vm.utilisateur_id` est NULL
  après `admin:agent`, et l'attribution est **P4**. Tout utilisateur
  authentifié voit toutes les VMs, **et une ligne de journal le dit à chaque
  requête**.
- **Aucun geste de masquage** : `masquee_a` naît et n'est écrite par personne.
  Le geste explicite qu'annonce la spec D3 appartient au hub.
- **Aucune calibration.** `PERIODE_RECONCILIATION`, `DELAI_LANCEMENT_MS`, la
  borne de la file d'émission, et toutes les constantes de P1 à P3 que G1 ne
  recalibre pas. Elles rejoignent la liste déjà longue de ce dépôt —
  `BPP_MIN`, `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`, `HYSTERESIS`,
  `REPIT_APRES_ECHEC`, `TAILLE_MAX_SORTIE`, `SEUIL_INJOIGNABLE_MS`,
  `REPLI_MIN_MS`, `REPLI_MAX_MS`, `OCTETS_PREFIXE`. **Aucun jugement visuel ni
  d'usage n'est porté sur aucune d'elles.**
- **Rien des raccourcis de l'espace de noms Shell** (les 7 à cible vide), **rien
  des applications UWP/MSIX**, **rien d'une application sans raccourci** — c'est
  le trou structurel de la voie retenue, et la spec §9 le nomme.
- **Rien de `.msc`, `.url`, `.bat`, `.msi` comme applications lançables** : seuls
  les `.exe` entrent au catalogue (spec D3, §10).
- **Rien du rapprochement d'une application avec sa version antérieure** quand
  son répertoire d'installation change : la spec D4 assume que c'est une
  application **neuve**.
- **Rien de la latence de bout en bout**, qu'aucun sous-bloc du chantier D n'a
  jamais mesurée, et que ⑤ ne mesure pas davantage.
- **Rien de la charge** : aucun test de plusieurs VMs enrôlées simultanément,
  aucun catalogue de plus de 218 raccourcis, aucune mesure du coût de la boucle
  sur la VM au-delà du `duree_ms` relevé.
- **Rien d'un antivirus.** ⚠️ L'état de l'antivirus de cette VM **n'a pas été
  relevé** — ni par la spec, ni ici : il n'est affirmé dans aucun sens.
- **Aucun audit de sécurité.** L'authentification HTTP introduite par D9 est la
  **première** du service ; elle réemploie `verifierJeton`, qui n'a pas été
  audité par un tiers (spec ⑤ §8).
- **Aucune modification du produit historique** (`src/`, `web/`, `index.js`) —
  cadrage §11. Il continue de tourner à côté, avec son balayage horaire.
- **Aucune revérification d'un jeton en cours de session** : les routes ne
  couvrent que la requête, et la garde du relais ne couvre que la poignée de
  main — legs n°9 de P2, reconduit.
