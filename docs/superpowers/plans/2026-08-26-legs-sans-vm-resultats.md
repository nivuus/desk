# Legs sans VM — recette et revue transverse : résultats

**26 août 2026.** Tâche 10, la dernière du lot `legs-sans-vm` (commits
`6b61203`..`ec09c40` sur `finalisation`, HEAD mesuré `ec09c40`). Brief :
`.superpowers/sdd/2026-08-22-legs-sans-vm/task-10-brief.md`. Neuf tâches,
huit legs de `CLAUDE.md` fermés, chacune déjà revue à l'implémentation
(`.superpowers/sdd/2026-08-22-legs-sans-vm/progress.md` : sept tâches
« review clean », deux « arbitré » au plafond de rounds — 3 et 5). **Cette
tâche ne reprend pas ce travail** : elle cherche ce qu'une revue par tâche ne
peut structurellement pas voir — un défaut qui franchit une frontière de
tâche.

---

## 1. Verdict en une ligne

**Rien à corriger.** La suite entière est verte, les tailles sont inchangées
par rapport à avant le lot, aucun message de commit ne contredit son diff,
aucune affirmation devenue fausse par jonction n'a été trouvée au-delà de
celles déjà déclarées par les tâches elles-mêmes. Le seul travail de cette
tâche a été de **fermer les huit legs dans `CLAUDE.md`** — ils y étaient
encore décrits comme ouverts après que le code les a fermés — et d'écrire ce
document.

---

## 2. Tailles — relevées après la dernière édition de la ronde (la mienne comprise)

```
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
```

```
   1536 agent/src/encode.rs
    630 agent/src/windows_source.rs
```

**Identique à la liste d'avant le lot.** Le tableau de dette de `CLAUDE.md`
n'avait rien à corriger. Les neuf tâches ont chacune extrait AVANT d'ajouter
(`file.rs`, `parts.rs`, `registre.rs` → `registre/tables.rs` et
`registre/tour_de_roue.rs`, `relance_pont.rs` → `relance_pont/issue.rs`,
`tokens.css` → `tokens/couleurs.css` + `tokens/echelles.css`,
`serveur.ts` → `http/erreurs-socket.ts`) : aucun fichier neuf du lot n'a
franchi 500 lignes, et les deux seuls fichiers de la dette historique
(`#[cfg(windows)]`, sans test) sont inchangés par ce lot.

Spot-checks des tailles finales citées par les commits, contre `wc -l`
aujourd'hui — tous conformes : `agent/src/relance_pont.rs` 446,
`relance_pont/tests.rs` 410, `relance_pont/issue.rs` 94, `agent/src/
signaling.rs` 468, `agent/src/capteur/sommeil/file.rs` 420,
`plateforme/src/securite/frein.ts` 368, `plateforme/src/http/routes-vm.ts`
375, `client/src/design/tokens/couleurs.css` 234, `client/src/design/
tokens/echelles.css` 167, `client/outils/tokens-orphelins.mjs` 292.

---

## 3. Les messages de commit, relus contre leur diff

**30 commits** dans `6b61203..HEAD`, groupés en 9 tâches (T1–T9, chacune
1 à 9 commits selon le nombre de rounds de correction). Pour chaque commit :
`git show --stat`, puis diff complet pour les commits à claim forte
(« CE COMMIT NE FAIT QUE L'EXTRACTION », changements de comportement,
chiffres de taille).

**Aucun écart trouvé entre un message et son diff.** Notamment :

- Les quatre commits d'extraction déclarant « ne fait que l'extraction »
  (`88e96ff` file.rs→file/tests.rs, `6f92f14` parts.rs→parts/tests.rs,
  `803dccd` registre.rs→registre/tables.rs, `6645957`
  relance_pont.rs→relance_pont/issue.rs) le sont réellement : diff
  vérifié à la main sur `88e96ff` et `6645957`, le corps déplacé est
  verbatim (désindenté), les seuls ajouts de prose sont déclarés comme tels
  dans le message (`6645957` nomme explicitement les trois déictiques
  réparés au passage).
- Le commentaire de `plateforme/src/http/serveur.ts` que `1457569` dit avoir
  corrigé (« SEUL servirAuth LE LIT » → énumération complète) est bien celui
  qui est aujourd'hui en place, et j'ai revérifié la commande qu'il cite
  (`grep -ln 'deps\.frein' plateforme/src/http/routes-*.ts`) : elle rend
  exactement `routes-auth.ts`, `routes-vm.ts`, `routes-session.ts`, comme
  annoncé — et `signaling/relais.ts`, cité comme lecteur hors de ce grep
  parce qu'il reçoit `frein` par paramètre positionnel plutôt que via
  l'objet `deps`, est bien câblé ainsi.
- La constante `REQUETES_MAX_ADRESSE` vaut aujourd'hui `120`
  (`plateforme/src/securite/frein.ts:217`), conforme à la valeur que
  `bafd4de` dit avoir reconsidérée depuis le `60` initial de `1457569`, et
  identique à la valeur citée dans le commentaire de
  `docker-compose.plateforme.yml`.
- Chaque commit « round de correction N » cité (`0ce22fa`, `1976577`,
  `f6bf11e`, `275ac30`, `6c84a91` côté pont ; `af8a643`, `b977af4` côté
  apps ; `025ea2e` côté tokens ; `c49c5ec` côté accent) documente lui-même
  les affirmations qu'il corrige, avec la citation exacte de ce qui était
  faux — un patron répété dans ce lot qui rend la revue transverse plus
  facile que d'habitude : les tâches se sont largement corrigées
  elles-mêmes, round après round, avant d'atteindre HEAD.

**Écarts de commit-message : 0.**

---

## 4. Affirmations devenues fausses, cherchées par le sens

Recherche par script (Python, patterns joignant les lignes sur les fichiers
source des cinq racines de code) sur les tournures classiques de ce dépôt —
« n'est appelé par personne », « aucun appelant », « SEUL X LE LIT » — pour
capter une phrase coupée par un retour à la ligne. **Tous les résultats
étaient soit déjà déclarés comme un legs volontaire (code défensif
inatteignable, fonction sans appelant nommée comme telle), soit vérifiés
exacts** (le cas `serveur.ts` du §3 ci-dessus).

Recherche ciblée en plus sur `CLAUDE.md` lui-même : c'est là qu'ont été
trouvées les seules affirmations réellement devenues fausses par ce lot —
voir §5. Elles ne viennent pas d'un défaut du code, mais du fait que
`CLAUDE.md` décrivait encore les huit legs comme ouverts après que le code
les a fermés : une revue par tâche ne touche pas l'index, c'est précisément
le travail de cette tâche 10.

**Affirmations devenues fausses trouvées et corrigées : 8** (les huit
lignes de `CLAUDE.md` recensées au §5 — aucune dans le code source lui-même).

---

## 5. Défauts de jonction entre tâches

**Recherché et non trouvé** : aucune convention posée en cours de lot n'est
restée non appliquée rétroactivement à ses propres commits antérieurs (le
lot applique lui-même cette discipline — voir `357707b`, `770402d` etc., qui
corrigent leurs propres affirmations round après round) ; aucune décision de
T5 (apps) ne contredit T3 (freins) ; le vocabulaire (« FERMÉ », « legs
déclaré plutôt que corrigé », « barré plutôt qu'effacé », « copie nommée »)
est homogène d'un fichier à l'autre.

**Le seul défaut de jonction réel de ce lot était dans `CLAUDE.md`, pas dans
le code** : les tables et paragraphes de la section « Legs ouverts »
décrivaient encore, à HEAD, huit choses que le code venait de fermer :

| Ligne de `CLAUDE.md` | Fermée par |
| --- | --- |
| « ONZE PILOTES DE RECETTE VISENT UNE RACINE QUE CE CHANTIER A FERMÉE » | T8 (`e8fa436`) |
| « LE CONTRAT DE `SIGNALING_URL` N'EST FIGÉ PAR AUCUN TEST » | T4 (`3c97dac`) |
| ligne ④ gestion d'apps : « le magasin n'est jamais nettoyé » | T5 (`aa7ff65`…`b977af4`) |
| ligne ⑤ plateforme : « `GET /vm` et `POST /session` ne sont pas freinées […] le relais […] non plus » | T3 (`1457569`…`0ce22fa`) |
| ligne ⑥ design system : « `--accent-fenetre` n'est déclaré nulle part et peint par rien » | T7 (`b99c989`, `c49c5ec`) |
| ligne ⑥ design system : « les galeries n'ont aucun test » | T9 (`ec09c40`) |
| ligne ① divers : « le canal `Message` du registre est non borné » | T1+T2 (`d1d5dbd`…`3da9d1c`) |
| tokens.css 300/300 (déjà partiellement tenue à jour par T6 lui-même) | T6 (`0106cd5`…`0b11b48`) |

Les sept premières lignes ont été barrées (`~~…~~`) et suivies d'une note
« FERMÉ (lot `legs-sans-vm`) » nommant le mécanisme réel, sans recopier
aucun chiffre ni numéro de ligne — conformément à la règle du fichier. La
ligne WCO a été enrichie de sa raison de rester ouverte (« écarté par
décision : inatteignable tant que le hub n'est pas installable, une
décision de sécurité qui appartient au propriétaire »), **elle reste
ouverte** — ce lot ne l'a jamais prétendu fermer.

Diff exact : `git show -- CLAUDE.md` sur le commit de cette tâche.

---

## 6. La suite entière, depuis un shell propre

Annoncé : **les 10 étapes de `verify-all.sh`** (le script affiche 18
en-têtes `==>`, dont 8 viennent de `client : npm run design:verifier`
appelé en sous-commande — comptés à part, jamais confondus avec les 10
étapes).

```
env -u TURN_URL -u TURN_SECRET ./scripts/verify-all.sh
```
→ **« Les 10 étapes sont passées. »** Repères notables dans le journal :
cargo test 1052+114 (workspace), cargo clippy (avertissements existants,
aucune nouvelle famille), client `npm test` 572/572 (49 fichiers), design
7/7 (§7.1 contrastes 53 paires 0 échec, §7.2 0 couleur littérale, §7.4 0
écart de thème, §7.6 0 orphelin — dont `--accent-fenetre` posé par le JS
désormais reconnu, §7.9 0 écart de classe, §7.3 build servi, §7.7 poids CSS
10 368/12 288 octets), proto `npm test` (vert), plateforme `test:sqlite` et
`test:postgres` 65 fichiers / 717 tests chacun, `npm run typecheck`
(plateforme et client) propres. Les seules lignes `Error:` du journal sont
des fautes INJECTÉES par les tests eux-mêmes (« base injoignable »,
« EMFILE (simulé) », « pool after calling end ») — pas des échecs.

```
cargo test --workspace
```
→ **vert** : `1052 passed` (agent) + `114 passed` (proto) + `0` doctest,
0 échec.

```
cargo check --target x86_64-pc-windows-gnu
```
→ **vert**, exit 0. 24 avertissements, tous de la famille `dead_code`
(nature vérifiée, pas seulement comptée) — conforme au relevé que plusieurs
commits du lot citent déjà.

```
cd plateforme && npm run typecheck && npm run test:sqlite && npm run test:postgres
```
→ couverts par `verify-all.sh` ci-dessus (mêmes commandes) : **verts**.
Postgres vérifié en conteneur `guacamole-postgres-plateforme-1` (healthy, up
6 jours).

```
cd client && npm run typecheck && npx vitest run && npx vitest run --dir ../proto && npm run build
```
→ couverts par `verify-all.sh` (mêmes commandes, plus le build) : **verts**,
572 tests client, 308 tests proto, build Vite propre (6 pages, poids
inchangé).

**Verdict global : 10/10, aucune rouge.**

---

## 7. Ce que ce lot n'établit PAS

- **Aucun des douze pilotes de recette n'est rejoué.** La VM Windows est
  hors périmètre de ce lot (comme de la tâche 10 elle-même — voir le brief
  du lot). T8 a réparé onze des douze vers `/signal` et vérifié chaque
  fichier par `node --check` seul ; aucun n'a tourné contre une plateforme
  réelle, et le comportement métier de chaque recette (accent-a1, micro-e2,
  micro-e3, pont-fichiers f1–f5, presse-papier p1–p3) reste non rejoué.
- **Aucun jugement visuel n'est porté.** Les vingt-cinq jugements humains
  du sous-projet ⑥ (S1→S4, jamais rendus dans un navigateur par un humain)
  attendent toujours un œil ; T9 fige des LISTES de cas (tokens déclarés,
  primitives balisées), explicitement pas un rendu, et le dit dans son
  propre en-tête.
- **Aucune constante introduite par ce lot n'est calibrée** :
  `BUDGET_REQUETES`/`FENETRE_REQUETES_MS`/`REQUETES_MAX_ADRESSE` (T3, dits
  NON CALIBRÉS dans leur propre commit), les âges d'éviction des deux
  magasins (180 j icônes, 30 j tranches, T5), `PROFONDEUR_MAX = 64` du canal
  du capteur (T1/T2), `SEUIL_STABILITE_MS` du pont (T3, dérivé de
  `REPLI_MAX_MS`, pas mesuré indépendamment).
- **Rien n'a tourné sur la VM.** La trace de refus du capteur
  (`capteur/sommeil/file.rs`, palier en puissances de deux) n'a **jamais
  été vue sortir** dans un `agent.log` réel — c'est un contrôle qu'on n'a
  jamais vu rouge en conditions réelles, et T2 le déclare déjà lui-même :
  « il faudra un bras qui bouche réellement une file pour l'établir ». Le
  comportement en charge réelle de cette file relève d'un lot ultérieur
  (le lot 3 cité par le brief de cette tâche).
- **Les legs parqués nommément par chaque tâche restent dus**, notamment :
  T2/T3 — un `Sommeil` refusé reste PERDU (tracé, non réémis ; le remède
  casserait la preuve de terminaison de la boucle actuelle, à cadrer
  séparément) ; T5 — la course stat→rm entre lecture d'âge et éviction sur
  les deux magasins (rétrécie d'un défaut déterministe à une course, jamais
  fermée ; fenêtre de chevauchement entre deux tours d'éviction, sans
  conséquence mesurée en production mais non gardée) ; T5 — le câblage de
  `referencees` pour un futur magasin ne couvre que icônes et tranches, pas
  un troisième magasin hypothétique ; T7 — `tokens-orphelins.mjs` à 292/300
  lignes, découpage nommé (séparer le rapport de la collecte) mais non fait ;
  T3 — le plafond de sommeil de l'agent (30 s) reste plus court que ce que
  le serveur peut suggérer (jusqu'à 60 s), et un enfant de fenêtre dort
  désormais jusqu'à 30 s avant de mourir sur un échec d'attache.

---

## 8. Ce que ce document ferme, et ce qu'il ne ferme pas

**Ferme** : la description des huit legs dans `CLAUDE.md` comme ouverts,
alors que le code les a fermés entre le 25 et le 26 août 2026.

**Ne ferme pas** : le WCO (décision du propriétaire, hors périmètre) ; les
legs parqués nommément au §7 ; le rejeu des douze pilotes sur la VM ; le
jugement visuel des vingt-cinq cas de ⑥ ; la calibration de toute
constante — tous restent dus, et `CLAUDE.md` continue de le dire.
