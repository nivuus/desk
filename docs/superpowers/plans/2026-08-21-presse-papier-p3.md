# Sous-projet ① « Divers » — presse-papier, sous-bloc **P3** : les N fenêtres — plan d'implémentation

> ⚠️ **TROIS « P3 » PEUVENT SE CONFONDRE DANS CE DÉPÔT.** Celui-ci est le
> **presse-papier**. Le sous-projet ⑤ (plateforme) a un « P3 » clos et sans
> rapport (l'identité des agents, le canal `/agent`) ; le chantier D a des
> sous-blocs `D1`…`D11` qui n'ont rien à voir non plus. Quand ce document écrit
> « P1 » ou « P2 », il désigne **toujours** le presse-papier.

Conception : `../specs/2026-08-19-presse-papier-design.md` — §6.3 « P3 — Les N
fenêtres », décision **D3**, et §3.3 « Ce que ces relevés N'établissent PAS ».
Sous-blocs précédents : `2026-08-19-presse-papier-p1.md` (+ `-resultats`),
`2026-08-20-presse-papier-p2.md` (+ `-resultats`).
Journaux à verser : `journaux-presse-papier-p3/`.

---

## 0. Ce que ce plan a MESURÉ avant d'écrire une ligne

🔴 **Chaque nombre de ce document a été relevé par la commande au moment où il a
été écrit, le 21 août 2026.** Aucun n'est recopié d'un document antérieur.
Ce dépôt a publié des plans portant des chiffres faux à répétition — celui de P5
en portait deux, celui de P2 sous-estimait ses croissances d'un facteur 2 à 4,
celui de G3 affirmait une purge de dette qui n'avait pas eu lieu.

### 0.1 Les comptes, à `586a436`

| Commande | Résultat relevé |
| --- | --- |
| `cargo test -p agent` | **904 passed; 0 failed** |
| `cargo test -p proto` | **109 passed; 0 failed** |
| `cd client && npx vitest run` | **462 passed**, **40** fichiers |
| `cd proto && npx vitest run` | **296 passed**, **9** fichiers |
| `grep -c '^ *etape "' scripts/verify-all.sh` | **10** |
| `grep -n 'PRESSE_PAPIER' scripts/run-agent.sh` | `:39` `PRESSE_PAPIER`, `:40` `PRESSE_PAPIER_GARDE`, `:104` `PRESSE_PAPIER_SONDE` — **les trois y sont déjà** |
| `grep -n 'CONTROL_VERSION' proto/src/control.rs` | `:14` — vaut **3** |

⚠️ **`verify-all.sh` compte DIX étapes dans le SCRIPT** ; le nombre d'en-têtes
`==>` d'une exécution complète est **différent** (P5 en relevait dix-huit, dont
huit venant de l'intérieur de `client : npm run design:verifier`). **Dire lequel
on compte** — P3 et P4 de la plateforme ont chacun payé une correction pour ne
pas l'avoir fait. Ce plan compte les **étapes du script**.

⚠️ **Ces chiffres dérivent, et un chantier concurrent (F3, le pont fichiers)
écrit dans `agent/` et `client/` au moment où ce plan est rédigé.** Les relever
de nouveau avant chaque tâche qui en dépend ; ne jamais recopier ceux-ci.

### 0.2 🔴 Le tableau de dette de `CLAUDE.md` porte QUATRE lignes, et DEUX sont périmées

```
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
```

rend **DEUX** lignes, et deux seulement :

```
   1536 agent/src/encode.rs
    630 agent/src/windows_source.rs
```

`proto/src/plateforme/tests.rs` vaut **340** (publié **561**) et
`proto/ts/plateforme.test.ts` vaut **462** (publié **512**) : les deux sont
retombées sous le plafond. ⚠️ **Ce n'est PAS P3 qui les a résorbées** — un
chantier voisin l'a fait, et l'attribuer à P3 serait s'attribuer le travail d'un
autre. **La correction du tableau appartient à la tâche de clôture** (§7,
tâche 12), qui remesurera à sa date : `CLAUDE.md` est un périmètre concurrent et
un relevé pris aujourd'hui serait faux à la clôture.

⚠️ **Marge NULLE neuve, hors périmètre de P3** :
`plateforme/src/http/routes-installation.ts` est à **500 pile**, comme
`agent/src/encode/arret.rs`. **P3 n'y touche pas** ; relevé pour que nul ne
l'impute à ce sous-bloc.

### 0.3 Ce que ce plan N'A PAS mesuré, et ne pouvait pas

- **Rien de la VM Windows.** Elle est tenue par le chantier F3 (voir §2.4). Tous
  les faits d'agent de ce document sont **des lectures de code**, jamais des
  mesures d'exécution — et ils sont étiquetés comme tels.
- **Rien du navigateur.** Les deux sondes du §7 (tâches 1 et 2) n'ont **pas** été
  jouées : ce document les **prescrit**. Leurs verdicts gouvernent le reste, et
  §3 écrit d'avance ce que chacun commande.
- **Aucune des rouges prescrites n'a été jouée.**

---

## 1. Ce plan ne couvre QUE P3

- **P1 est CLOS** (sens VM → navigateur, une fenêtre). P3 n'y touche que là où il
  le complète : l'émission de l'état courant à l'inscription (legs n°3 de P1) et
  la course de D-P3-6.
- **P2 est CLOS** (sens navigateur → VM, une fenêtre). P3 le **met sous
  concurrence** et le mesure ; il ne le réécrit pas.
- **A1 — la couleur d'accent** (spec §5, §6.4) : hors périmètre entier.
- **Images, fichiers, RTF, HTML** : hors périmètre v1 (spec §9).
- **Le propriétaire MONO-FENÊTRE** (legs n°1 de P1, rendu **bruyant** par P2)
  reste **hors périmètre**, et D-P3-14 écrit pourquoi. **Ne pas le combler en
  passant.**
- **Le canal `Message` non borné** (legs n°4 de P1, aggravé par P2) reste **non
  borné**, et D-P3-11 écrit pourquoi.

🔵 **P3 est d'abord un sous-bloc de MESURE.** La règle D3 est **déjà livrée** —
le fan-out par P1, le dépôt différé par P1 aussi. Ce que P3 construit tient en
quatre points, tous petits : les deux moitiés du legs n°3, une trace
d'attribution d'une ligne, et le correctif de D-P3-6. **Tout le reste est de
l'instrument et de la mesure.**

---

## 2. Contraintes globales

### 2.1 La règle des 500 lignes, budgétée d'avance

Relevé **par la commande**, le 21 août 2026, sur les fichiers que P3 touche ou
pourrait toucher :

| Fichier | Lignes | Marge | Budget prévu |
| --- | --- | --- | --- |
| 🔴 `agent/src/presse_papier.rs` | **441** | **59** | D-P3-6 y ajoute une seconde prise et sa documentation. ⚠️ **Le budget « +20 lignes » serait le même mensonge que celui que P2 s'est fait sur `tick.rs`** (« +1 branche » pour +72 réelles) : la documentation d'un garde dans ce fichier coûte **trente lignes pour une fonction de six** — `apres_notre_ecriture` en porte **29** (`:318-346`, mesurées). **EXTRACTION PRÉALABLE OBLIGATOIRE** (tâche 3) : `Sondeur` (`:234-402`, **169 lignes relevées**) part vers `agent/src/presse_papier/sondeur.rs`, le parent retombe autour de **270** |
| 🔴 `agent/src/capteur/sommeil/presse_papier.rs` | **379** | **121** | la mémoire `dernier_presse_papier` + son émission + la seconde prise + **leurs tests**. ⚠️ Son bloc de tests est **AU MILIEU du fichier** (`:95` à `:~330`), pas en queue — voir E12. **EXTRACTION PRÉALABLE** (tâche 4) : le bloc part vers `agent/src/capteur/sommeil/presse_papier/tests.rs`, le parent retombe autour de **145** |
| `agent/src/capteur/sommeil/registre.rs` | **417** | **83** | +1 appel dans `inscrire` (`:394-397`), +1 champ dans `Etat`, +leur documentation. **Sous le plafond, mais à surveiller** : c'est le troisième fichier le plus serré du périmètre |
| `agent/src/capteur/sommeil.rs` | 375 | 125 | rien, sauf si `Message` gagne un champ — il n'en gagne pas |
| `agent/src/capteur/sommeil/tests.rs` | 417 | 83 | les tests de l'émission à l'inscription vivent **ici** (c'est là que vivent ceux de `registre.rs`) ⟹ **à surveiller**, voir E13 |
| `agent/src/capteur/fenetre.rs` | 426 | 74 | **+1 champ** dans une trace existante (tâche 8) |
| `client/src/main.ts` | **445** | **55** | +la mémoire du dernier `clipboard`, sur le patron **exact** de `micAnnonce` (`:141`, `:164`, `:339`) — **DEUX lignes de câblage**, et rien d'autre (E15) |
| `client/src/presse-papier-dom.ts` | 176 | 324 | +1 paramètre facultatif `initial` et son rejeu au montage — **c'est là que vit la RÈGLE**, parce que `main.ts` n'a aucun test (E15) |
| `client/src/presse-papier-dom.test.ts` | 290 | 210 | les quatre tests de la tâche 7 |
| `agent/src/presse_papier/tests.rs` | 377 | 123 | les tests de D-P3-6 |
| `agent/src/presse_papier/win32.rs` | 161 | 339 | **rien** |
| `client/src/presse-papier.ts` | 158 | 342 | **rien** |
| `proto/src/control.rs` | 403 | 97 | 🔵 **RIEN — P3 n'ajoute aucune variante de protocole** (D-P3-13) |
| `proto/ts/control.ts` | 241 | 259 | 🔵 **RIEN**, même raison |
| `agent/src/capteur/protocole.rs` | 330 | 170 | 🔵 **RIEN** : `DepuisCapteur::PressePapier` existe déjà |
| `agent/src/transport/tick.rs` | 441 | 59 | 🔵 **RIEN** |
| `agent/src/transport.rs` | **482** | **18** | 🔵 **RIEN**, et c'est heureux : ce fichier a franchi 501 **deux fois** (D10, puis F2) et P2 lui a repris 31 des 52 regagnés. **Si une tâche de P3 croit devoir y écrire, elle s'est trompée de conception** — le relire avant |
| `scripts/run-agent.sh` | 160 | — | 🔵 **RIEN** : les trois variables presse-papier y sont déjà (`:39`, `:40`, `:104`) |

⚠️ **Aucun fichier de la dette gelée n'est touché** : ni `encode.rs` (1536), ni
`windows_source.rs` (630), ni `encode/arret.rs` (500, marge 0), ni
`capture.rs` (492).

🔴 **Les deux extractions se jouent AVANT les additions qui les rendent
nécessaires, jamais après.** D9 a payé **deux compressions** pour l'avoir
oublié — geste que `CLAUDE.md` interdit nommément ; D10 a joué trois extractions
préalables et n'en a payé aucune ; P1 et P2 en ont joué quatre et n'en ont payé
aucune non plus.

⚠️ **Scinder un module de tests par `#[path]` est HORS de la portée de la
convention `#[path]`/racine nue** de `CLAUDE.md` (clause « Est également hors de
portée l'usage de `#[path]` pour scinder un module de *tests* trop long »). La
tâche 4 emploie donc `#[cfg(test)] #[path = "presse_papier/tests.rs"] mod
tests;` **à l'intérieur** de son parent, comme `superviseur/table.rs`, et **ne
hisse rien à la racine**.

⚠️ **La tâche 3, elle, n'est PAS un module de tests** : `presse_papier/sondeur.rs`
est du code de production. Il se déclare par un `mod sondeur;` **ordinaire** à
l'intérieur de `presse_papier.rs`, avec un `pub use sondeur::Sondeur;` — la
convention `#[path]`/racine nue **ne s'applique pas** : elle ne vise que les
modules extraits d'un parent `#[cfg(windows)]` pour compiler sur l'hôte, et
`presse_papier.rs` n'est pas gaté.

🔴 **Un balayage par la commande est OBLIGATOIRE à la clôture** (tâche 12,
Step 1), et pas seulement aux endroits budgétés ci-dessus. G2 a franchi **trois**
plafonds sans les voir passer, P2 trois, G3 un — **tous rattrapés par des relevés
voisins, jamais par le budget**. Un budget prévoit ; seule la commande constate.

### 2.2 Les règles de méthode, héritées et non négociables

1. **Un contrôle qu'on n'a jamais vu ROUGE n'est pas un contrôle.** Pour chaque
   test et chaque critère, ce plan écrit ce qui le rend rouge — et **la tâche
   vérifie que cet état est atteignable** avant d'écrire le vert.
2. 🔴 **Une rouge doit rougir pour la BONNE raison.** S3 a dû refaire **quatre
   rouges sur seize** : deux rougissaient sur une clause préexistante, deux ne
   mutaient rien du tout. **Le harnais de rouge est prescrit au §6.3 et il est
   obligatoire** : entre la mutation et le contrôle, une preuve que
   `git diff --numstat` est **NON VIDE** ; et le relevé doit dire **quelle
   assertion** a rougi, jamais seulement le code de sortie.
3. 🔴 **Muter par NUMÉRO DE LIGNE, ou par un motif ancré sur la syntaxe — jamais
   par la seule sous-chaîne.** P4 de la plateforme a vu une rouge rester **verte**
   parce que la chaîne à muter apparaissait d'abord dans le **commentaire** qui la
   justifie. Ce dépôt commente abondamment ses invariants : **plus un invariant
   est documenté, plus sa rouge est fragile.**
4. **Un plan est une SOURCE de contrôles vacueux, pas une protection contre
   eux.** Celui-ci en a trouvé un dans la spec (E2) et l'a remplacé ; il peut en
   porter d'autres.
5. **Ne jamais fabriquer une sortie de commande.** Lancer la commande. D10 a vu
   **deux pièces fabriquées** — un fait vrai présenté sans sa preuve —, et c'est
   le mode de défaillance que ce dépôt juge le plus grave.
6. **Nommer la chose, jamais compter les lignes qui l'en séparent.** Un déictique
   (« trois lignes plus haut », « `CLAUDE.md:723` ») vieillit à la première
   insertion. P1 l'a payé deux fois, et sa revue transverse une troisième — sur
   un numéro de ligne qu'elle venait elle-même de décaler.
7. **`git commit` valide TOUT L'INDEX, y compris avec `-F`.** Pathspec explicite
   (`git commit -F <fichier> -- <chemins>`), puis `git show --name-only`.
   **Jamais `git add -A`, jamais `--amend`** : l'index porte le travail de F3.
8. **Compter les tests AVANT de les lire** : annoncer le nombre attendu, puis le
   comparer. D10 a perdu un test dans un `Write` et ne l'a vu que parce que le
   compte avait été annoncé d'avance.
9. 🔴 **Un ZÉRO se qualifie avant de se rapporter.** `grep` sans `-a` sur un
   journal à octets NUL rend une sortie **VIDE**, pas un zéro — les journaux de
   pilote de P1 en portent **33 à 36**, et `grep -c 'copie'` y rendait vide quand
   `grep -ac` rendait **17**. **`grep -a` partout sur les journaux de pilote.**
10. 🔴 **Un `grep` de recette se vérifie contre le CODE, jamais contre la spec.**
    Trois des quatre contrôles de F1 cherchaient des chaînes que le produit
    n'émet pas, **et deux auraient fait lire un succès comme un échec**.
11. **Copier `agent.log` APRÈS la fin réelle de l'exécution**, pas à la fin du
    pilote — et **jamais sous un nom déjà pris** : P2 a écrasé le journal qui
    portait sa fuite de presse-papier.
12. **Appel BLOQUANT au premier plan** pour toute exécution longue : une commande
    backgroundée par le harnais ne survit pas à la fin du tour (D10, deux
    recettes y ont perdu une exécution).
13. **`Get-Process agent` revérifié après CHAQUE tentative, y compris échouée** :
    un superviseur survivant fait relire le journal de la tentative précédente.
14. 🔴 **Deux exécutions de recette ne doivent JAMAIS se chevaucher.** F1 en a
    perdu une : la seconde a tué l'agent de la première, et **le journal versé
    sous le nom de la première était celui de la seconde**.
15. **`unset -f chpwd` avant tout relevé** : le hook du shell de l'hôte injecte
    un `ls` dans toute sortie dès qu'un `cd` court dans un sous-shell. S2 a dû
    reprendre une passe entière de journaux pour cela.

### 2.3 Périmètre concurrent

🔴 **Le chantier F3 (pont fichiers) travaille en ce moment, et il tient la VM
Windows.** Son périmètre, relevé par `git status --porcelain` le 21 août 2026 :
`agent/src/pont/` (`chemins.rs`, `ecriture/fil.rs`, `ecriture/fil/tests.rs`,
`erreurs.rs`, et deux fichiers non suivis), `client/src/fichiers/`
(`mutation.ts`, `copie.ts`), plus `proto/src/fichiers.*`.

**Ce qui est à P3, et rien d'autre** :
`agent/src/presse_papier*`, `agent/src/capteur/sommeil/presse_papier*`,
`agent/src/capteur/sommeil/registre.rs`, `agent/src/capteur/sommeil/tests.rs`,
`agent/src/capteur/fenetre.rs` (**une ligne**), `client/src/main.ts` (**trois
lignes**), `docs/superpowers/plans/journaux-presse-papier-p3/`, et à la clôture
`CLAUDE.md` et le document de résultats.

**Ce qui n'est PAS à P3, et qu'il ne faut pas toucher** : `agent/src/pont/`,
`proto/src/fichiers.*`, `client/src/fichiers/` (F3) ; `proto/src/plateforme.*`
et `plateforme/` (gestion d'apps) ; `proto/src/control.rs` et `proto/ts/control.ts`
(D-P3-13 : **P3 n'y a rien à faire**).

- **Step 0 de toute tâche touchant `agent/`, `client/` ou `CLAUDE.md`** :
  `git log --oneline -5 -- <chemin>` **et** `git status --porcelain <chemin>`,
  puis relire le diff.
- 🔴 **La tâche 10 (recette) ne peut pas démarrer tant que la VM est tenue.**
  L'ordonnanceur doit l'enregistrer comme un **préalable EXTERNE**, jamais comme
  une dépendance de tâche.
- ⚠️ **Ne jamais lancer `scripts/build-agent.sh` quand `proto/` ou `agent/`
  portent des modifications non commitées d'un chantier voisin** : il
  rsynchronise l'arbre entier et pousserait sur la VM du travail à demi fait.
  P2 l'a payé, et son remède est la bonne forme — **un worktree isolé au commit
  de la branche**.
- ⚠️ **`cargo clean --release -p proto -p agent`**, les DEUX crates, avant tout
  `build-agent.sh` qui suit un aller-retour de sources : purger `agent` seul
  laisse le rlib de `proto` que l'horloge de la VM fait paraître frais, et **le
  symptôme est une erreur de type sur une signature pourtant à jour**.

---

## 3. Décisions tranchées

### D-P3-1 — Le sens descendant est DÉJÀ un fan-out complet : P3 le MESURE, il ne le construit pas

`agent/src/capteur/sommeil/presse_papier.rs::distribuer` (`:54-93`) itère sur
**toutes** les clés de `garde.canaux`, sans aucun filtre — ni de focus, ni
d'éveil :

```rust
let sessions: Vec<String> = garde.canaux.keys().cloned().collect();
for session in sessions {
    let envoye = match garde.canaux.get(&session) {
        Some(canal) => canal
            .send(Message::PressePapier { texte: texte.clone(), octets })
            .is_ok(),
        None => false,
    };
    ...
}
```

Et un test d'hôte le tient déjà :
`une_annonce_de_texte_part_vers_toutes_les_sessions_inscrites` vérifie que la
fenêtre focalisée **et** la non-focalisée reçoivent.

**Conséquence pour le plan** : le critère ① est une **mesure**, pas une
construction. ⚠️ **Ce n'est pas une raison de le sauter** : un test d'hôte
éprouve `distribuer`, jamais la chaîne capteur → tube → enfant → WebRTC → page,
et c'est cette chaîne-là que ① mesure, **trois fois de suite**.

### D-P3-2 — 🔴 Le legs n°3 de P1 a DEUX moitiés, et P1 comme P2 n'en nommaient qu'une

Le legs dit : « une fenêtre attachée après une copie ne reçoit jamais ce
contenu ». **Vérifié par la commande** :
`grep -rn "presse_papier::distribuer" agent/src/` rend **une seule occurrence**,
`registre.rs:231`, dans le **tour de roue**. `registre.rs::inscrire` (`:349-399`)
appelle `distribuer(ordres)`, `parts::distribuer_les_parts`,
`porteurs::distribuer_l_audio` — **jamais** `presse_papier::distribuer`. Et le
`Sondeur` le dit de lui-même (la doc du champ `reference` de `Sondeur`).

🔵 **Mais il y a une SECONDE moitié, côté client, que rien ne nommait.**
`client/src/main.ts:267` fait `pressePapier?.recevoir(...)` — et `pressePapier`
n'est assigné qu'à `:344`, dans le `.then` de `connectSession`, alors que
`onControl` est câblé **avant** que cette promesse ne résolve (c'est la raison
écrite du hissage à `:125`). **Un message `clipboard` arrivé dans cet intervalle
est perdu en silence.** Le patron du remède est **dans le même fichier** :
`micAnnonce` (`:141` la mémoire, `:164` la pose, `:339` le rejeu).

⚠️ **Et l'émission à l'inscription tombe PRÉCISÉMENT dans cet intervalle.**
L'inscription a lieu quand l'enfant s'attache au capteur, très avant que le
navigateur ne se connecte. Côté agent le message attend — `queue_control`
(`transport/controle.rs:14`) le met dans `pending_control`, et
`brancher_controle_en_file` le **garde en file tant que le canal n'est pas
ouvert**. Donc il partira dès l'ouverture du canal de contrôle, **c'est-à-dire
possiblement avant que `main.ts` n'ait assigné `pressePapier`**.

🔴 **Corriger une seule moitié ferait paraître le défaut corrigé alors qu'il
resterait intermittent — et c'est pire qu'un défaut connu.** Les deux moitiés
sont donc livrées : tâche 6 (agent) et tâche 7 (client), et **le critère ① les
mesure ensemble, de bout en bout**.

**Ce que la moitié agent fait, exactement** :
- `Etat` (`registre.rs:29`) gagne `dernier_presse_papier: Option<Annonce>`,
  voisin de `dernieres_parts` (`:42`) et `derniers_audio` (`:58`) ;
- `presse_papier::distribuer` le pose à chaque annonce ;
- `inscrire` émet cet état, s'il existe, **sur le seul canal de la session qui
  vient de s'inscrire** — jamais un fan-out, qui rejouerait le contenu à toutes
  les fenêtres à chaque attache.

### D-P3-3 — ⚠️ AUCUNE purge à l'inscription pour le presse-papier, et NE PAS copier celle des parts par symétrie apparente

`inscrire` purge `dernieres_parts` et `derniers_audio` sur une ré-inscription
(`registre.rs:368` et `:371`), et la raison est écrite là : `distribuer_les_parts`
**FILTRE** sur `dernieres_parts` (`parts.rs:126-128`), donc sans purge une part
identique serait jugée déjà livrée sur un canal qui ne l'a jamais reçue.

**L'émission du presse-papier à l'inscription est INCONDITIONNELLE : il n'y a
donc rien à purger.** Ajouter une purge par symétrie de forme retirerait la
mémoire au moment précis où on veut s'en servir, et le remède ne remédierait à
rien. **C'est écrit ici parce que la symétrie est trompeuse à la lecture.**

### D-P3-4 — 🔴 La règle du dépôt différé n'est PAS seulement une parade à une contrainte Chromium : à N fenêtres, c'est l'ARBITRAGE qui rend « toutes reçoivent » sûr

C'est la contribution de conception de ce sous-bloc, et elle change ce que le
critère ④ commande.

D3 présente la règle comme la clôture du « problème du focus document (§3.3,
contrainte supposée) ». **À une fenêtre, c'est tout ce qu'elle fait.** À N,
elle fait autre chose, et personne ne l'avait écrit : le capteur pousse le
contenu à **toutes** les fenêtres, chacune a son propre `PressePapierLocal`
(`presse-papier-dom.ts:104`), et **si toutes écrivaient, N appels concurrents à
`navigator.clipboard.writeText` partiraient pour une seule copie, le dernier
gagnant arbitrairement**. Le test de focus est donc, à N, **l'élection qui
désigne l'unique écrivain** — et elle est correcte au sens de D3, puisque
l'utilisateur ne peut coller localement que depuis la fenêtre qu'il regarde.

🔴 **CONSÉQUENCE, ÉCRITE D'AVANCE POUR QUE PERSONNE N'ARBITRE SOUS LE COUP DU
RÉSULTAT :**

| Verdict de la sonde S2 (tâche 2) | Ce que P3 fait |
| --- | --- |
| **`writeText` ÉCHOUE sans focus** | la contrainte du §3.3 passe de **supposée** à **mesurée**. La règle reste, sa justification d'origine est confirmée, et **la seconde justification ci-dessus est ajoutée au commentaire** — elle vaut indépendamment |
| **`writeText` RÉUSSIT sans focus** | 🔴 **la contrainte du §3.3 est RÉFUTÉE, et P3 l'ÉCRIT** — dans le §3.3 de la spec, dans `client/src/presse-papier.ts:109-113` (dont le commentaire l'affirme aujourd'hui comme un fait), et dans `CLAUDE.md`. **La règle est GARDÉE**, sur la justification d'arbitrage, qui est la seule qui subsiste |

⚠️ **« Il faut l'écrire, pas la garder » (spec §6.3) suppose UNE fenêtre, et P3
est le sous-bloc qui montre pourquoi.** Retirer la règle signifierait « toutes
les fenêtres écrivent », c'est-à-dire N écrivains concurrents pour une copie —
**et rien ne mesure ce régime-là**. Retirer la règle est donc une **décision de
conception qui appartient au propriétaire du dépôt**, pas une conséquence
mécanique d'un verdict de sonde. **P3 ne la retire pas, et il écrit pourquoi.**

### D-P3-5 — 🔴 La mesure du §3.3 doit DISCRIMINER le FOCUS de l'ACTIVATION, sinon elle attribue au mauvais mécanisme

**P2 a déjà mesuré un `NotAllowedError` sur `writeText`, et ce n'était PAS le
focus.** Sa sonde annexe, versée
(`journaux-presse-papier-p2/p2-annexe-writetext-{1,2}.json`, **deux
exécutions**), relève :

```
avant : { isActive: false, hasBeenActive: false, hasFocus: true }  →  writeText THROW:NotAllowedError
après : { isActive: true,  hasBeenActive: true,  hasFocus: true }  →  writeText OK
```

`hasFocus` vaut **`true` des deux côtés** : ce que cette sonde mesure est
l'**activation utilisateur transitoire**, pas le focus. Et les deux mécanismes
lèvent la **même** exception.

🔴 **Une sonde de P3 qui se contenterait d'observer « fenêtre non focalisée ⟹
THROW » attribuerait au focus ce qui pourrait être l'activation, et le verdict
de D-P3-4 serait faux — dans les deux sens possibles.** La sonde S2 est donc
un **2×2** : `{hasFocus vrai, faux} × {userActivation.isActive vrai, faux}`,
quatre cellules, chacune avec son résultat de `writeText`.

⚠️ **`isActive` est TRANSITOIRE** (il retombe quelques secondes après le geste),
`hasBeenActive` est collant. La sonde relève **les deux** à chaque cellule, et
la cellule « activation vraie » se produit **immédiatement après** un geste
`Input.dispatchKeyEvent`, jamais des secondes plus tard.

⚠️ **Si la cellule `{hasFocus: false, isActive: true}` s'avère INATTEIGNABLE**
— une fenêtre qui n'a pas le focus ne peut peut-être pas recevoir de geste de
confiance —, **la sonde le DIT et le §3.3 reste supposé**. C'est un verdict
recevable ; en fabriquer un autre ne le serait pas.

### D-P3-6 — 🔴 Une COURSE entre `armer_les_gardes` et `tour()` — trouvée par LECTURE, non mesurée, et le critère ③ la provoquera

**L'entrelacement, lu dans le code et rapporté ici sans être mesuré :**

1. la fenêtre A colle → `ecrire_avec` (`sommeil/presse_papier.rs:349`) écrit,
   puis pose `etat().notre_ecriture = Some((seqA, textA))` ;
2. le tour de roue appelle `armer_les_gardes` (`registre.rs:219`) : il **prend**
   le couple et arme `reference = seqA`, `dernier_emis = textA` ;
3. la fenêtre B colle → `notre_ecriture = Some((seqB, textB))`, le presse-papier
   Windows porte maintenant `textB`, le compteur vaut `seqB` ;
4. `sondeur.tour()` (`registre.rs:221`) lit `seqB` ≠ `seqA` → le **garde n°1 ne
   mord pas** → lit `textB` → `dernier_emis == textA` ≠ `textB` → le **garde n°2
   ne mord pas non plus** → **`Annonce::Texte(textB)` part vers les N fenêtres** ;
5. au tour suivant, `armer_les_gardes` prend `(seqB, textB)` — trop tard.

🔴 **C'est exactement l'aller-retour par collage que les gardes de D5 existent
pour supprimer**, et la documentation existante ne le couvre pas :
`apres_notre_ecriture` (`presse_papier.rs:341-346`) traite le cas d'une **copie
TIERCE** intercalée, qu'elle déclare *voulu*. Le cas ci-dessus est **notre
propre seconde écriture**, et il n'est déclaré nulle part.

**Portée exacte, et il faut la dire pour ne pas gonfler le défaut** :
- **conséquence** : **UNE** annonce parasite, portant le texte que B vient de
  coller. La fenêtre focalisée l'écrit dans son presse-papier **local** — qui
  contient déjà ce texte, puisque l'utilisateur venait de l'y copier pour le
  coller. **L'utilisateur ne voit rien.** Ce qui se voit est **sur le fil** ;
- **ce n'est PAS un défaut créé par P3** : à une fenêtre, deux collages en moins
  de `PERIODE_PRESSE_PAPIER` (250 ms) le produisent aussi. P2 ne l'a pas
  rencontré — ses quatre collages étaient espacés de plusieurs secondes ;
- 🔵 **mais le critère ③ de P3 le PROVOQUERA par construction** : « deux collages
  quasi simultanés » est littéralement l'entrelacement ci-dessus.

**DÉCISION : P3 l'éprouve par un test d'hôte VU ROUGE, et le CORRIGE.** Trois
raisons, dans cet ordre :
1. **il est purement testable et déterministe** — `armer(armes, seq, texte)` et
   `observer(seq, lire)` prennent tous deux leur état et leur lecture **injectés**
   (c'est ce que P1 et P2 ont payé pour). Le test se réduit à : `armer(true,
   seqA, textA)` puis `observer(seqB, || Some(textB))`, et il rend `Some` là où
   il devrait rendre `None`. **Il ne peut pas être vacueux** ;
2. **le laisser rendrait le critère ③ AMBIGU** : un message `clipboard` de plus
   se lirait comme un « contenu mêlé », et un critère ambigu est pire qu'un
   défaut corrigé ;
3. **le correctif est petit et pur.**

**Le correctif, tranché ici** : une **seconde prise**, dans `registre.rs`, entre
`tour()` et `distribuer` —

```
armer_les_gardes(&mut sondeur);              // inchangé, avant le tour
let annonce = sondeur.tour();
let annonce = filtrer_nos_ecritures_tardives(&mut sondeur, annonce);
```

où `filtrer_nos_ecritures_tardives` prend `etat().notre_ecriture`, l'arme sur le
`Sondeur`, et **laisse tomber l'annonce si son texte est le nôtre**.

⚠️ **CE REMÈDE RÉTRÉCIT LA FENÊTRE, IL NE LA FERME PAS, et il faut l'écrire dans
le code.** `ecrire_avec` écrit le presse-papier **puis** pose `notre_ecriture`
(le verrou est délibérément pris après l'E/S Win32, `:357-362`) : si `tour()` lit
le texte dans ce court intervalle, la seconde prise ne trouvera rien. Le résidu
est de l'ordre d'une acquisition de mutex, et il est **du même genre** que celui
que `apres_notre_ecriture` déclare déjà accepté.

⚠️ **Ne PAS choisir le remède « faire de `notre_ecriture` une file »** : elle ne
ferme rien de plus — le presse-papier ne porte qu'une valeur, et c'est celle du
**dernier** écrivain qui compte — et elle ajoute un plafond à calibrer que rien
ne calibrerait.

### D-P3-7 — 🔴 L'attribution session ↔ fenêtre Windows n'est observable NULLE PART, et les critères ② et ③ en dépendent

**Relevé par la commande** : `enfant lancé` (`superviseur/enfants.rs:66-72`)
porte `session`, `pid` (celui de l'**enfant agent**) et `sortie` ;
`fenêtre attachée au capteur` (`capteur/fenetre.rs:190-196`) porte `session`,
`sortie`, `largeur`, `hauteur`. **Aucune trace du dépôt n'associe une `session`
au `hwnd` ni au PID de l'APPLICATION Windows**, et
`grep -rn "hwnd" agent/src/superviseur/` ne rend que deux constructions locales,
sans trace.

Or les critères ② et ③ demandent « le texte de B est arrivé dans **la** fenêtre
de B ». **Sans attribution, ils ne sont pas jugeables.**

**DÉCISION : une ligne.** `Fenetre` porte déjà `pid: u32` (`fenetre.rs:149`,
c'est celui qu'elle passe à `inscrire`, `:221`) et `Parametres.hwnd`
(`fenetre.rs:123`). La trace `fenêtre attachée au capteur` gagne **`pid`**.
Le pilote fait ensuite `pid → MainWindowHandle` par `Get-Process -Id`, puis lit
par `WM_GETTEXT` — le chemin exact que `journaux-presse-papier-p2/instrument/lecteur-pp.ps1`
emprunte déjà.

❌ **La voie « coller un nonce et regarder quel Bloc-notes a grandi » est
REJETÉE : elle est CIRCULAIRE.** Elle établirait l'attribution **par le
mécanisme même que ② et ③ mesurent. C'est le défaut que D8 a payé sur
`resoudreIdentite`, et que son propre rapport qualifie de « partiellement
circulaire ».

⚠️ **Tâche DÉDIÉE (tâche 8), et une seule ligne.** C'est le geste que ce dépôt
a payé quatre fois sous une autre forme (`SUPERVISEUR` en D1,
`MULTIFENETRE_REPRISE` en D2, `BUDGET_BPS` en D6, `AUDIO` en D7) : **ce dont
tout le reste dépend se fait dans sa propre tâche, avant que quiconque en ait
besoin.**

### D-P3-8 — Trois fenêtres, et le nombre est RELEVÉ, jamais exigé

La spec l'écrit : « il prend le nombre que la VM rend le jour de la recette et
**l'écrit** ». D9 a plafonné à **trois** par pollution de registre ; D10 l'a levé
sur mesure et a relevé **dix**.

**Ce que la recette fait, dans cet ordre** :
1. `MULTIFENETRE_VDD_PURGE=1` **dans un lancement séparé** (l'aiguillage de
   `diagnostics::multifenetre` **retourne après la première sonde reconnue** :
   deux variables dans le même lancement n'en enchaînent pas deux) ;
2. `grep -a 'sortie créée mais introuvable'` **avant de conclure à un plafond** —
   c'est la signature de la pollution de registre, et son remède opérationnel est
   `MULTIFENETRE_MODE_SORTIE=1280x720`, **encore un lancement à lui seul** ;
3. compter les `fenêtre attachée au capteur`, **jamais les fenêtres du
   navigateur** : « compter les fenêtres, jamais les lancements » est un piège
   payé trois fois (Paint en D2, Bloc-notes en D4, les popups en D9).

**Si la VM rend moins de trois** : à **deux**, ①②③④ restent tous jugeables et le
relevé écrit « deux, pas trois ». À **une**, **P3 n'est pas livrable** (RP3-1).

### D-P3-9 — Trois Bloc-notes sont indistinguables par leur titre : chacun est PRÉ-SEMÉ d'un marqueur

Un Bloc-notes non enregistré s'intitule « *Sans titre - Bloc-notes », les trois
à l'identique. **La lecture par `WM_GETTEXT` doit donc partir d'un état
distinguable** : chaque Bloc-notes reçoit, avant toute mesure, un marqueur
distinct (`AAA`, `BBB`, `CCC`), par le **copieur à demeure de P1** et non par
une tâche planifiée par geste (D-P3-10).

⚠️ **Ce marqueur ne remplace pas l'attribution de D-P3-7** : il rend le CONTENU
attribuable, il ne dit pas **quelle session** pilote quel Bloc-notes. Les deux
sont nécessaires, et les confondre referait la circularité que D-P3-7 rejette.

### D-P3-10 — L'instrument est celui de P1 et P2, RÉEMPLOYÉ, jamais réécrit

- 🔴 **Le copieur à demeure** (`journaux-presse-papier-p1/instrument/copieur-pp.ps1`) :
  une tâche planifiée **par copie** ouvre une console **éligible à la capture**,
  ce qui fait naître une session de plus, ce qui détruit la mesure. **Un seul
  processus, né AVANT le superviseur, piloté par fichiers**, et **son attente
  porte sur le FAIT** (un numéro d'ordre relu), jamais sur une durée.
- ⚠️ **Deux relevés identiques à 300 ms d'écart** avant de croire une lecture :
  CIFS et `Set-Content` ne sont pas atomiques, et P2 a lu une ligne **tronquée**
  qui commençait par le bon numéro d'ordre et passait donc son test.
- 🔴 **Le point d'observation des messages `clipboard` est un écouteur
  INDÉPENDANT du produit, POSÉ PAR FENÊTRE**, sur le canal de contrôle — jamais
  les appels à `writeText` : `PressePapierLocal` **dédoublonne lui-même**
  (`presse-papier.ts:117`), et compter les écritures mesurerait le garde du
  **client** au lieu de celui de l'**agent**. C'est la leçon la plus réutilisable
  de la recette de P1, reprise telle quelle par P2, **et P3 l'étend à N**.
- ⚠️ **`Target.getTargets` en plus de l'auto-attache** : P1 a perdu une exécution
  entière parce que l'auto-attache ne suffit pas, et **une cible ouverte par
  `window.open` s'attache avec une URL VIDE** — un instrument qui teste l'URL à
  `Target.attachedToTarget` ne peut jamais réussir, en silence.

### D-P3-11 — Le canal `Message` reste NON BORNÉ, et P3 ne le borne pas

`registre.rs:351` : `channel::<Message>()`, un `std::sync::mpsc::channel` — **la
capacité est illimitée**, et `commandes.rs:22-23` le dit du côté enfant. C'est le
legs n°4 de P1, aggravé par P2 (64 KiB dans les **deux** sens).

**P3 n'y touche pas**, et la raison n'est pas la paresse : le borner exige de
décider ce qu'on fait quand il est plein — jeter, bloquer, ou clore. **Bloquer
serait le pire** : le seul écrivain de ce canal est le tour de roue, qui écrit
**sous le verrou global du registre** (`presse_papier::distribuer` reçoit un
`MutexGuard`), et un `send` bloquant y gèlerait l'attache et le retrait de
**toutes** les fenêtres. C'est un changement de conception, pas un correctif.

⚠️ **P3 l'aggrave d'un message par attache** (D-P3-2), et pas davantage. Dit
plutôt que tu.

### D-P3-12 — Côté client, P3 ne touche que `main.ts` (deux lignes de câblage) et `presse-papier-dom.ts` (un paramètre facultatif)

`presse-papier.ts` est **correct tel quel pour N fenêtres**, et
`presse-papier-dom.ts` ne gagne qu'un paramètre **facultatif** (E15). **Il faut
dire pourquoi, parce que la lecture naïve conclut l'inverse** : chaque fenêtre de
session est une **fenêtre navigateur séparée**
(`shell-page.ts:117`, `window.open('/?session=…')`), donc un `document` séparé,
un `window` séparé, une instance de module séparée. `focalise: () =>
document.hasFocus()` (`main.ts:346`) est donc **par fenêtre**, `cible: window`
(`:347`) n'est partagé avec personne, et `collageArme` (`:135`) est propre à
chaque page.

🔴 **Une lecture qui suppose N attaches dans UNE page conclurait à quatre défauts
inexistants** (N écouteurs `focus` sur le même `window`, N `writeText`
concurrents, un `paste` émis N fois, un `collageArme` partagé). **Aucun n'existe
dans cette architecture.** C'est écrit ici pour qu'une tâche ne parte pas les
corriger.

### D-P3-13 — `CONTROL_VERSION` ne monte pas, et P3 ne touche NI `proto/src/control.rs` NI `proto/ts/control.ts`

P3 n'ajoute **aucune** variante de protocole : le sens descendant réemploie
`DepuisCapteur::PressePapier` (`capteur/protocole.rs:245`) et
`AgentControl::Clipboard` (`proto/src/control.rs:294`), le sens montant
`ClientControl::Clipboard` (`:132`). L'ajout de D-P3-7 est un **champ de trace**,
pas un champ de message.

🔵 **Conséquence à vérifier plutôt qu'à croire** : à la clôture,
`git diff --stat` de la branche **ne doit montrer aucun fichier de `proto/`**.
Si un fichier de `proto/` a bougé, une tâche s'est trompée de conception.

### D-P3-14 — Le mono-fenêtre reste hors périmètre, et P3 ne le comble pas en passant

Legs n°1 de P1, rendu **bruyant** par P2 (`Err` au lieu d'un silence). Point de
chute nommé : `agent/src/demarrage.rs` (**426** lignes aujourd'hui, marge 74 —
la tâche 3 de P2 la lui a rendue). **P3 y touche d'autant moins qu'il n'a rien à
y faire** : son sujet est N fenêtres, pas zéro capteur.

### D-P3-15 — La borne de 12 s de `commander` n'est pas provoquée, et P3 est le premier à pouvoir la faire courir

Legs n°5 de P2 : ce chemin **n'a jamais couru**. `SourceDistante::ecrire_le_presse_papier`
(`capteur/distante.rs:402-404`) est **synchrone** — elle bloque la boucle de
transport de l'enfant jusqu'au `Fait` du capteur. À trois enfants qui commandent
en même temps, la contention est réelle pour la première fois.

**P3 ne la provoque pas artificiellement** (il faudrait tuer le capteur en plein
vol, ce qui est une autre mesure) — **mais le critère ③ la nomme**, et la recette
doit **relever** toute occurrence : la ligne à chercher est
`aucune réponse du capteur` / `commande expirée`. **Un zéro s'y qualifie comme
partout ailleurs** : zéro sur des collages espacés de secondes ne dit rien.

---

## 4. Divergences entre la spec, les documents antérieurs, et le code réel

Toutes relevées **par la commande** le 21 août 2026, et tranchées **ici** plutôt
que laissées à l'implémenteur.

### E1 — La spec dit que P3 « livre la règle D3 ». Elle est DÉJÀ livrée.

Le fan-out l'a été par P1 (`sommeil/presse_papier.rs::distribuer`, avec son
test), le dépôt différé aussi (`presse-papier.ts:114-119`,
`presse-papier-dom.ts:124-125`, avec **quatre** tests d'hôte). **Ce que P3 livre
est la MESURE, plus deux trous que la spec ne nomme pas** : les deux moitiés du
legs n°3 (D-P3-2) et l'attribution non observable (D-P3-7). **Tranché : la spec
sera annotée à la clôture**, à son §6.3, sans être réécrite — c'est un relevé
daté qui reste vrai comme histoire.

### E2 — 🔴 La rouge du critère ① de la spec désigne un mécanisme qui n'existe pas

Elle dit : « une élection de porteuse le ferait tomber à un ». **Il n'y a aucune
élection de porteuse dans le presse-papier** — c'est le mécanisme de l'**AUDIO**
(`capteur/sommeil/porteurs.rs`), et `presse_papier::distribuer` n'en a jamais eu.
La rouge n'est donc pas jouable telle qu'écrite.

**Remplacée, et la mutation est nommée** : dans `distribuer`, remplacer la boucle
sur toutes les clés par un envoi à **la première clé seule**. Le compte tombe de
**3** à **1**.

⚠️ **Une mutation qui n'enverrait à PERSONNE serait moins bonne** : elle rendrait
**0**, et zéro est aussi ce que rendrait un mécanisme entièrement mort. **1 est
le bon rouge, parce que 1 ne peut venir que d'une distribution qui fonctionne et
qui a été restreinte.**

### E3 — 🔴 La contrainte du §3.3 et la mesure de P2 ne parlent PAS du même mécanisme

Détaillé en D-P3-5. §3.3 parle de **focus** ; l'annexe de P2, **versée et jouée
deux fois**, mesure l'**activation** (`hasFocus: true` dans les deux cellules).
Les deux lèvent `NotAllowedError`. **Une sonde qui ne discrimine pas les deux
attribue au mauvais mécanisme.**

### E4 — « Du coût pour rien » suppose UNE fenêtre

Détaillé en D-P3-4. À N, la règle du dépôt différé est l'**arbitrage** qui
désigne l'unique écrivain local. **La phrase de la spec reste vraie de ce qu'elle
observait ; elle cesse de commander ce qu'elle commandait.** Annotation à la
clôture, au §6.3 et au §3.3.

### E5 — Le legs n°3 de P1 est nommé côté AGENT seulement

Détaillé en D-P3-2. Sa seconde moitié est `client/src/main.ts:267`. **Ni P1 ni
P2 ne la nomment**, et le §8 du plan de P2 renvoie le legs entier à P3 en ne
décrivant que la moitié agent.

### E6 — Ne pas copier la purge de `dernieres_parts` par symétrie apparente

Détaillé en D-P3-3.

### E7 — Les budgets de la spec §7.2 ont TOUS vieilli

| Fichier | Spec §7.2 (19 août) | **Relevé, 21 août** |
| --- | --- | --- |
| `proto/src/control.rs` | 470, marge 30 | **403**, marge 97 — et **P3 n'y touche pas** |
| `client/src/main.ts` | 451, marge 49 | **445**, marge 55 |
| `agent/src/capteur/protocole.rs` | 419, marge 81 | **330**, marge 170 |
| `agent/src/transport/tick.rs` | 403, marge 97 | **441**, marge 59 |
| `agent/src/capteur/distante.rs` | 409, marge 91 | **472**, marge **28** |
| `agent/src/capteur/sommeil/registre.rs` | 346, marge 154 | **417**, marge 83 |
| `agent/src/input.rs` | 250, marge 250 | **287**, marge 213 |
| `proto/ts/control.ts` | 139, marge 361 | **241**, marge 259 |

⚠️ **`agent/src/capteur/distante.rs` est passé de 409 à 472 : marge 28.**
**P3 n'y touche pas** — et si une tâche croit devoir y écrire, elle doit s'arrêter
et le déclarer.

### E8 — Le tableau de dette de `CLAUDE.md` porte deux lignes périmées

Détaillé au §0.2. **Non corrigé par ce plan** (périmètre concurrent), à
remesurer et corriger par la tâche 12 — c'est la règle de `CLAUDE.md` lui-même
(« corriger le tableau dans le même mouvement »).

### E9 — 🔴 `Emulation.setFocusEmulationEnabled` rendrait le critère ④ vacueux EN SILENCE

Cette commande CDP existe précisément pour faire croire à une page qu'elle a le
focus. Si elle était activée, `document.hasFocus()` vaudrait `true` partout et
④ ne pourrait plus échouer. **`grep` sur les deux pilotes versés
(`pilote-pp-p1.mjs`, `pilote-pp-p2.mjs`) ne rend aucune occurrence** — l'état
d'aujourd'hui est donc sain. **Le témoin S1 (tâche 1) le contrôle explicitement**,
et le pilote de P3 ne doit ni l'activer, ni hériter d'un profil qui l'activerait.

### E10 — La borne de 12 s : P3 est le premier à pouvoir la faire courir

Détaillé en D-P3-15.

### E11 — `verify-all.sh` : DIX étapes dans le script, un autre nombre à l'écran

Détaillé au §0.1. **Dire lequel on compte.**

### E12 — Le bloc de tests de `sommeil/presse_papier.rs` est AU MILIEU du fichier

`#[cfg(test)] mod tests {` à `:95`, et le code de production **reprend à `:336`**
(`ecrire`, `ecrire_avec`, `armer_les_gardes`). L'extraction de la tâche 4 déplace
donc un bloc **central**, pas une queue : le `git diff` sera moins lisible qu'un
déplacement de fin de fichier, **et le Step « transposition caractère pour
caractère » n'en est que plus obligatoire**.

### E13 — Les tests de `registre.rs` ne vivent pas dans `registre.rs`

`grep -n "mod tests" agent/src/capteur/sommeil/registre.rs` rend **rien** :
`registre.rs` n'a pas de module de tests. Ils vivent dans
`agent/src/capteur/sommeil/tests.rs` (**417** lignes, marge 83), déclaré depuis
`sommeil.rs:375`. **Les tests de l'émission à l'inscription y vont**, et la
tâche 6 doit relever ce fichier **avant** d'y écrire : c'est le quatrième plus
serré du périmètre, et **aucun budget de ce plan ne le protège au-delà de
l'annoncer**.

### E14 — Le mono-fenêtre n'a pas de propriétaire, et le collage y rend `Err`

Sans capteur, `SourceDistante` n'existe pas et `ecrire_le_presse_papier` n'a
personne à commander. **P2 l'a rendu bruyant.** Nommé ici pour qu'une recette
lancée par mégarde **sans `SUPERVISEUR`** ne lise pas cet `Err` comme une
régression de P3.

### E15 — 🔴 `client/src/main.ts` N'A AUCUN TEST, et ce plan a failli y mettre une règle

`ls client/src/*.test.ts` rend **24** fichiers ; **`main.test.ts` n'en fait pas
partie**, et aucun autre ne couvre `main.ts` — module d'entrée, effets de bord au
premier niveau, non importable.

⚠️ **La première rédaction de ce plan prescrivait un test « voisin » pour la
moitié client du legs n°3.** Il n'existe pas et ne peut pas exister. **Tranché
en faveur de la testabilité** : la RÈGLE va dans `presse-papier-dom.ts`
(paramètre `initial`), qui a son fichier de tests ; `main.ts` ne garde que deux
lignes de **câblage**, **déclarées non testées** (§6.2, tâche 7).

🔵 **C'est aussi une divergence de ce plan avec LUI-MÊME** : son §5.3 déclarait
`presse-papier-dom.ts` intouché. **Il est corrigé, et l'écart est écrit plutôt
qu'effacé** — un plan qui dicte du code intestable cède devant sa propre
contrainte.

---

## 5. Structure des fichiers

### 5.1 Ce que P3 CRÉE

| Fichier | Nature |
| --- | --- |
| `agent/src/presse_papier/sondeur.rs` | **extraction préalable** (tâche 3), `Sondeur` verbatim, `mod sondeur;` ordinaire + `pub use sondeur::Sondeur;` chez le parent |
| `agent/src/capteur/sommeil/presse_papier/tests.rs` | **extraction préalable** (tâche 4), `#[cfg(test)] #[path = …] mod tests;` chez le parent |
| `docs/superpowers/plans/journaux-presse-papier-p3/` | les journaux d'agent **bruts ET leurs `-plat`**, les JSON de pilote, `familles-de-lecture.txt`, et l'instrument dans `instrument/` |
| `docs/superpowers/plans/2026-08-21-presse-papier-p3-resultats.md` | le document de résultats permanent (tâche 13) |

### 5.2 Ce que P3 MODIFIE

| Fichier | Ce qui change |
| --- | --- |
| `agent/src/presse_papier.rs` | l'extraction (t3), puis la seconde prise de D-P3-6 (t5) |
| `agent/src/presse_papier/tests.rs` | les tests de D-P3-6 (t5) |
| `agent/src/capteur/sommeil/presse_papier.rs` | l'extraction (t4), la mémoire `dernier_presse_papier` (t6), `filtrer_nos_ecritures_tardives` (t5) |
| `agent/src/capteur/sommeil/registre.rs` | `Etat.dernier_presse_papier`, l'émission dans `inscrire`, la seconde prise dans le tour de roue |
| `agent/src/capteur/sommeil/tests.rs` | les tests de l'émission à l'inscription (E13) |
| `agent/src/capteur/fenetre.rs` | **une ligne** : `pid` dans la trace `fenêtre attachée au capteur` (t8) |
| `client/src/main.ts` | **deux lignes de CÂBLAGE** : la mémoire du dernier `clipboard`, patron `micAnnonce` (t7). ⚠️ **Non testées, et déclarées telles** (E15) |
| `client/src/presse-papier-dom.ts` | le paramètre `initial?: Recu` et son rejeu au montage (t7) — **c'est là que vit la RÈGLE**, parce que c'est là qu'elle est testable |
| `client/src/presse-papier-dom.test.ts` | les quatre tests de la tâche 7 |
| `CLAUDE.md` | la section P3, le tableau des variables **inchangé** (aucune variable neuve), les tailles remesurées (t12) |
| `docs/superpowers/specs/2026-08-19-presse-papier-design.md` | annotations E1, E3, E4 (t12) |

### 5.3 Ce que P3 NE TOUCHE PAS, et le vérifier à la clôture

`proto/` **en entier** (D-P3-13), `client/src/presse-papier.ts`,
`client/src/input.ts`,
`client/src/raccourcis.ts`, `agent/src/presse_papier/win32.rs`,
`agent/src/transport/*`, `agent/src/capteur/protocole.rs`,
`agent/src/capteur/pont_media.rs`, `agent/src/capteur/distante.rs`,
`agent/src/input.rs`, `agent/src/demarrage.rs`, `scripts/run-agent.sh`,
et **tout `agent/src/pont/`, `proto/src/fichiers.*`, `client/src/fichiers/`**
(F3).

🔵 **Aucune variable d'environnement neuve.** Les trois variables presse-papier
sont déjà transmises par `scripts/run-agent.sh` (`:39`, `:40`, `:104`). Le piège
maison — « toute variable neuve doit y être ajoutée explicitement, sinon l'agent
démarre sans elle et **sans rien signaler** », payé en D1, D2, D6 et D7 — **ne
s'applique pas ici**, et c'est dit parce qu'une absence se déclare.

---

## 6. Interfaces, et le harnais qui juge les rouges

### 6.1 Côté agent — la mémoire de l'état courant

```rust
// registre.rs, dans Etat, voisin de `dernieres_parts` et `derniers_audio`
pub(super) dernier_presse_papier: Option<Annonce>,
```

- posé par `presse_papier::distribuer`, **à chaque annonce**, y compris un
  `Annonce::Refus` — une fenêtre qui s'attache après un refus doit voir le
  bandeau, sans quoi elle attendrait un contenu qui n'arrivera jamais ;
- lu par `registre::inscrire`, **après** `parts::distribuer_les_parts` et
  `porteurs::distribuer_l_audio`, et envoyé **sur le seul canal de la session qui
  vient de s'inscrire** ;
- **aucune purge** (D-P3-3).

### 6.2 Côté client — la mémoire d'avant-attache, et où elle NE va PAS

🔴 **`client/src/main.ts` N'A AUCUN TEST — relevé par la commande** :
`ls client/src/*.test.ts` rend **24** fichiers, et **`main.test.ts` n'en fait
pas partie**. `main.ts` est le module d'entrée, avec ses effets de bord au
premier niveau : il n'est pas importable dans un test, et **aucun sous-bloc de
ce dépôt n'a jamais réussi à le tester**.

**Conséquence, tranchée ici plutôt que découverte** : la **RÈGLE** ne va pas
dans `main.ts`. Elle va dans `client/src/presse-papier-dom.ts`, qui a son fichier
de tests (**290** lignes, **15** tests), sous la forme d'un paramètre
**facultatif** d'`OptionsPressePapier` :

```ts
/** Le dernier contenu reçu AVANT l'attache, à rejouer au montage. */
initial?: Recu;
```

`attacherPressePapierAuDOM` le passe à `etat.recevoir(initial)` **puis** appelle
`ecrireSiPossible()` — le chemin qui existe déjà, sans en créer un second.

**Ce qui reste dans `main.ts` est du CÂBLAGE, et deux lignes** — le `let` et son
affectation, aux places exactes du patron `micAnnonce` (`:141`, `:164`, `:339`) :

```ts
let dernierPressePapier: Recu | undefined;                                      // ~:141
...
dernierPressePapier = { texte: message.text, octets: message.bytes };           // ~:267
pressePapier?.recevoir(dernierPressePapier);
...
pressePapier = attacherPressePapierAuDOM({ ..., initial: dernierPressePapier }); // ~:344
```

⚠️ **La mémoire est posée AVANT le `?.`**, jamais dans une branche `else` : la
poser dans le `else` ferait diverger les deux chemins le jour où l'un changerait.

🔴 **CES DEUX LIGNES NE SERONT PAS TESTÉES, ET LE PLAN LE DIT plutôt que de
promettre un test qui ne peut pas exister.** Le critère qui départage vient de
P4 de la plateforme : *une condition est une RÈGLE si la changer change ce que le
produit décide ; elle est du CÂBLAGE si elle ne fait que router une décision déjà
prise ailleurs, et testée là-bas.* Ici la décision — « rejouer le mémorisé au
montage » — vit dans `presse-papier-dom.ts` et y est testée ; `main.ts` ne fait
que la router. **Et `micAnnonce` est le précédent exact**, également non testé,
également dans ce fichier.

⚠️ **Le seul contrôle de bout en bout de ces deux lignes est le critère ①** —
une fenêtre attachée **après** la copie. **Sans lui, la moitié client du legs
n°3 n'est éprouvée par rien.**

### 6.3 🔴 Le harnais de rouge — OBLIGATOIRE, et il refuse une rouge qui ne mute rien

Pour **chaque** rouge de ce plan, dans cet ordre, et le relevé porte les sept
lignes :

1. `sha256sum <fichier>` **avant** ;
2. la mutation, **par numéro de ligne ou par un motif ancré sur la syntaxe** —
   jamais par la seule sous-chaîne (§2.2 règle 3) ;
3. 🔴 **`git diff --numstat -- <fichier>` : la sortie doit être NON VIDE.**
   Une sortie vide **est un échec de la rouge**, jamais un succès du produit ;
4. la commande de contrôle, **et le relevé dit QUELLE assertion a rougi** — pas
   seulement le code de sortie ;
5. `git checkout -- <fichier>` ;
6. `sha256sum` **après**, égal au premier ;
7. `git status --porcelain <fichier>` **vide**.

⚠️ **Le blanchiment des commentaires est éprouvé DANS LES DEUX SENS** pour tout
contrôle qui cherche une sous-chaîne : ce dépôt commente ses invariants, et S1
comme S2 ont vu un garde **satisfait par le commentaire du fichier qu'il
analyse**.

### 6.4 Le point d'observation de la recette

**Par fenêtre**, un écouteur du canal de contrôle **indépendant du produit**
(D-P3-10). Il compte les messages `{type:'clipboard'}` **reçus**, et retient leur
`text`/`bytes`. **Jamais les appels à `writeText`.**

Et **un relevé de focus par fenêtre** à chaque phase :
`{ session, hasFocus: document.hasFocus(), isActive: navigator.userActivation.isActive, hasBeenActive: navigator.userActivation.hasBeenActive }`.
C'est ce triplet qui rend le critère ④ jugeable **et** qui permet de dire, si ④
échoue, **de quel côté**.

---

## 7. Ordre et parallélisme

| Famille | Tâches | Dépend de | Parallélisable |
| --- | --- | --- | --- |
| 0 — les deux sondes, **HORS VM** | 1, 2 | — | **oui, les deux** |
| 1 — extractions préalables | 3, 4 | — | **oui, les deux** |
| 2 — le code | 5, 6, 7, 8 | 5←3 ; 6←4 | **5‖6‖7‖8** |
| 3 — l'instrument et la recette | 9, 10 | 9←1 ; 10←tout | non |
| 4 — clôture | 11, 12, 13 | 11←10 ; 12←11 ; 13←11 | 12‖13 |

**Chemin critique** : 1 → 9 → 10 → 11 → {12, 13}.

⚠️ **Les tâches 1 et 2 gouvernent la RÉDACTION des autres, pas leur code** : leurs
verdicts décident de ce que la tâche 10 peut juger (④) et de ce que la tâche 12
écrit dans la spec (D-P3-4). **Elles se jouent en premier, et elles se jouent
HORS VM** — comme le §0 de P2, elles n'ont besoin que d'un Chrome sur l'hôte.

⚠️ **Les tâches 3 et 4 ne peuvent pas être fusionnées avec 5 et 6.** L'extraction
se joue **AVANT** l'addition qui la rend nécessaire, et c'est une règle du dépôt,
pas une préférence.

⚠️ **La tâche 8 est dédiée à une ligne**, et elle n'est pas fusionnée avec 6 ou 7
pour cette raison exacte : ce dont tout le reste dépend se fait à part.

---

# Famille 0 — les deux sondes, HORS VM, et leurs verdicts gouvernent le reste

### Task 1 : 🔴 SONDE S1 — la mesurabilité du FOCUS à N fenêtres. Elle PEUT échouer, et c'est ce qui en fait un témoin.

**Objet :** établir qu'un Chrome sans interface peut faire qu'**exactement une**
fenêtre sur trois rapporte `document.hasFocus() === true`. **Sans cela, le
critère ④ n'est pas mesurable, et il faut le dire avant de le tenter.**

**Files:**
- Create: `docs/superpowers/plans/journaux-presse-papier-p3/instrument/sonde-p3-focus.mjs`
- Create: `docs/superpowers/plans/journaux-presse-papier-p3/p3-focus-{1,2}.json` et `.log`
- Modify: aucun

- [ ] **Step 0 :** aucune dépendance de code. **Aucun agent, aucune VM, aucune
      session WebRTC** — la sonde ouvre trois pages **locales** (`about:blank`
      suffit, ou une page servie par `vite preview`) par `window.open`, comme
      `shell-page.ts:117`.
- [ ] **Step 1 : 🔴 CONTRÔLER que l'émulation de focus est ÉTEINTE.** Ne jamais
      appeler `Emulation.setFocusEmulationEnabled`, et **le relever dans le
      JSON** (E9). Une sonde qui la laisserait active rendrait `true` partout et
      **le témoin serait vacueux en silence**.
- [ ] **Step 2 :** pour chaque fenêtre *i* de 0 à 2 : `Page.bringToFront` sur *i*,
      puis relever, **sur les TROIS**, le triplet
      `{hasFocus, isActive, hasBeenActive}`. **Trois basculements, trois
      relevés de trois.**
- [ ] **Step 3 : le verdict.**
- [ ] **Step 4 : DEUX exécutions**, JSON versés, et **aucun taux revendiqué** :
      deux exécutions établissent la reproductibilité d'un mécanisme
      déterministe, jamais une fréquence.

| Relevé | Verdict | Ce qu'il commande |
| --- | --- | --- |
| **exactement un `true` à chaque basculement, et c'est bien celui qu'on a amené au premier plan** | ✅ **④ est MESURABLE** | la tâche 10 le joue tel quel |
| **`true` partout** | 🔴 **④ NON MESURABLE** | l'écrire, et **ne pas le maquiller**. Nommer `Xvfb` + `xdotool` — consentement donné en D8, **jamais suivi d'effet** — et ⚠️ **les mesures qui en sortiraient ne se compareraient à aucune campagne antérieure** |
| **`false` partout** | 🔴 **④ NON MESURABLE, et pire** : le produit lui-même n'écrirait jamais rien | l'écrire, et **relire ① avec cet œil** — si aucune fenêtre n'a le focus, aucune n'écrit localement, et le niveau 1 de ① mesure alors le message, pas l'écriture |
| **un `true`, mais pas sur la fenêtre amenée au premier plan** | ⚠️ **mesurable et TROMPEUR** | l'écrire, et **piloter ④ sur le relevé, jamais sur l'intention** |

🔴 **Ce témoin PEUT échouer sur trois de ses quatre issues. C'est ce qui en fait
un témoin, et c'est ce que le témoin de mesurabilité de P1 a fait — il a échoué
sur sa troisième branche.**

### Task 2 : 🔴 SONDE S2 — `writeText` × FOCUS × ACTIVATION, quatre cellules. C'est la mesure du §3.3.

**Objet :** trancher la contrainte que la spec déclare **supposée depuis le
28 juillet 2026**, et la trancher **sans l'attribuer au mauvais mécanisme**
(E3, D-P3-5).

**Files:**
- Create: `docs/superpowers/plans/journaux-presse-papier-p3/instrument/sonde-p3-writetext.mjs`
- Create: `.../p3-writetext-{1,2}.json` et `.log`
- Modify: aucun

- [ ] **Step 0 :** hors VM, hors agent. Deux fenêtres suffisent : une qu'on
      amène au premier plan, une qu'on y laisse pas.
- [ ] **Step 1 :** relever, pour chacune des **quatre** cellules, le triplet
      d'état **puis** le résultat de `navigator.clipboard.writeText(nonce)` —
      `OK`, ou `THROW:<name>: <message>` **verbatim, le message compris**. C'est
      le message qui distingue « Document is not focused » d'un refus
      d'activation, et **le nom seul (`NotAllowedError`) ne le distingue pas**.
- [ ] **Step 2 : l'activation est TRANSITOIRE.** La cellule « activation vraie »
      se joue **immédiatement après** un `Input.dispatchKeyEvent` de confiance,
      jamais des secondes plus tard. Relever `isActive` **au moment de l'appel**,
      dans le même `Runtime.evaluate`, pas avant.
- [ ] **Step 3 : la cellule `{hasFocus: false, isActive: true}` peut être
      INATTEIGNABLE** — une fenêtre sans focus ne reçoit peut-être pas de geste
      de confiance. **Si elle l'est, la sonde le DIT** et le §3.3 reste
      **supposé**. C'est un verdict recevable ; en fabriquer un autre ne le
      serait pas.
- [ ] **Step 4 : DEUX exécutions**, JSON versés.

| Cellule | Résultat attendu si le §3.3 est vrai | Résultat attendu si c'est l'activation seule |
| --- | --- | --- |
| focus ✔, activation ✔ | OK | OK |
| focus ✔, activation ✘ | OK | THROW *(c'est ce que P2 a mesuré)* |
| focus ✘, activation ✔ | **THROW** | **OK** ← 🔴 **la cellule qui tranche** |
| focus ✘, activation ✘ | THROW | THROW |

🔴 **Le verdict commande D-P3-4, et les deux branches y sont déjà écrites.**
La tâche 12 porte l'annotation dans la spec **et** dans
`client/src/presse-papier.ts:109-113`, dont le commentaire affirme aujourd'hui
la contrainte comme un fait.

⚠️ **Ce que cette sonde n'établira PAS, quel qu'en soit le verdict** : rien de
Firefox, rien de Safari, rien d'un Chromium **avec interface**, rien d'un humain.
Même portée que le §3 de la spec, qui la déclare déjà.

---

# Famille 1 — les extractions préalables, AVANT toute addition

### Task 3 : 🔴 EXTRACTION de `Sondeur` vers `agent/src/presse_papier/sondeur.rs`

**Objet :** rendre à `presse_papier.rs` la marge que la tâche 5 va lui prendre.
**Aucun changement de comportement, aucune ligne de logique touchée.**

**Files:**
- Create: `agent/src/presse_papier/sondeur.rs`
- Modify: `agent/src/presse_papier.rs`

**Relevé le 21 août 2026 : 441 lignes, marge 59. `pub struct Sondeur` commence
à `:238` (sa documentation à `:234`) et l'`impl` se ferme à `:402` — **remesuré**. L'extraction
doit rendre le parent autour de 270 — LE MESURER, pas le supposer.**

- [ ] **Step 0 :** `git log --oneline -5 -- agent/src/presse_papier.rs` **et**
      `git status --porcelain agent/src/` — périmètre concurrent (F3).
- [ ] **Step 1 : compter les tests AVANT.** `cargo test -p agent presse_papier`,
      **annoncer le nombre**, puis le comparer au Step 5.
- [ ] **Step 2 : déplacer VERBATIM** la doc, la `struct` et son `impl`.
      Déclaration chez le parent : `mod sondeur;` **ordinaire** + `pub use
      sondeur::Sondeur;`. ⚠️ **Ce n'est PAS le cas de la convention `#[path]`** —
      elle ne vise que les modules extraits d'un parent `#[cfg(windows)]`, et
      `presse_papier.rs` n'est pas gaté (§2.1).
- [ ] **Step 3 : les dépendances qui traversent.** `Sondeur` emploie
      `normaliser`, `PRESSE_PAPIER_MAX`, `Annonce`, `actif()`, `gardes_armes()`
      et `win32`. **`gardes_armes` est PRIVÉE** (`:134`) : elle devient
      `pub(super)`, **et rien d'autre ne change de visibilité**. Le
      `#[cfg(windows)] / #[cfg(not(windows))] lire_la_plateforme` part avec lui.
- [ ] **Step 4 : vérifier la transposition caractère pour caractère.**
      `git diff` ne doit montrer qu'un déplacement, la ligne de déclaration, le
      `pub use`, et le changement de visibilité du Step 3.
- [ ] **Step 5 : voir vert**, avec **le même compte qu'au Step 1**. Un compte qui
      baisse est un test perdu.
- [ ] **Step 6 : `cargo check --target x86_64-pc-windows-gnu`** — le chemin
      `#[cfg(windows)]` de `lire_la_plateforme` n'est vérifié que là.
- [ ] **Step 7 : `wc -l` sur les deux fichiers**, porté au rapport.

| Test | Ce qui le rend ROUGE |
| --- | --- |
| aucun test neuf | — **c'est une extraction, pas une addition** |

### Task 4 : 🔴 EXTRACTION du bloc de tests de `agent/src/capteur/sommeil/presse_papier.rs`

**Objet :** identique à la tâche 3, sur le fichier que la tâche 6 va faire
grossir.

**Files:**
- Create: `agent/src/capteur/sommeil/presse_papier/tests.rs`
- Modify: `agent/src/capteur/sommeil/presse_papier.rs`

**Relevé : 379 lignes, marge 121. ⚠️ Le bloc `#[cfg(test)] mod tests {` commence
à `:95` et le code de production REPREND à `:336` — c'est un bloc CENTRAL, pas
une queue (E12).**

- [ ] **Step 0 :** `git log --oneline -5` + `git status --porcelain`.
- [ ] **Step 1 : compter les tests AVANT**, et l'annoncer.
- [ ] **Step 2 : déplacer VERBATIM**, `#[cfg(test)] #[path =
      "presse_papier/tests.rs"] mod tests;` **à l'intérieur** du parent.
      ⚠️ **Cette utilisation de `#[path]` est HORS de la portée de la convention
      de module enfant de `CLAUDE.md`** — c'est le même mécanisme employé pour
      une autre raison (la règle des 500 lignes), et `superviseur/table.rs` en
      porte deux précédents. **Ne pas hisser ce module à la racine.**
- [ ] **Step 3 : la déclaration se pose EN FIN de parent**, pas à `:95` : c'est
      la place de tous les autres `#[path]` de tests du dépôt, et un lecteur qui
      cherche le code de production ne doit pas buter dessus.
- [ ] **Step 4 : les `use` du bloc déplacé.** Il emploie `super::*` et des
      voisins de `sommeil` : **relire le diff plutôt que compiler à l'aveugle**,
      un bloc central en emporte plus qu'une queue.
- [ ] **Step 5 : vert, même compte qu'au Step 1. Step 6 : `wc -l` des deux.**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| aucun test neuf | — extraction |

---

# Famille 2 — le code, et il tient en quatre points

### Task 5 : 🔴 La course entre `armer_les_gardes` et `tour()` — le test ROUGE D'ABORD, puis le correctif

**Objet :** D-P3-6. **Le test avant le remède**, et il doit être **vu rouge sur
l'arbre intact**, sans aucune mutation.

**Files:**
- Modify: `agent/src/presse_papier/sondeur.rs` *(né de la tâche 3)*
- Modify: `agent/src/presse_papier/tests.rs`
- Modify: `agent/src/capteur/sommeil/presse_papier.rs`
- Modify: `agent/src/capteur/sommeil/registre.rs`

- [ ] **Step 0 :** `git log --oneline -5` + `git status --porcelain` sur les
      quatre chemins.
- [ ] **Step 1 : 🔴 ÉCRIRE LE TEST, ET LE VOIR ROUGE SUR L'ARBRE INTACT.**
      C'est la forme la plus forte de rouge de ce dépôt : **elle ne mute rien**,
      donc elle ne peut ni rougir pour la mauvaise raison, ni être satisfaite par
      un commentaire. Le test :
      `armer(true, seq_a, "textA")` puis `observer(seq_b, || Some("textB"))`.
      **Il rend `Some(Annonce::Texte("textB"))` là où il doit rendre `None`.**
      ⚠️ **Si le test passe du premier coup, C'EST QUE LA LECTURE ÉTAIT
      FAUSSE** — s'arrêter, l'écrire dans le rapport, et **ne pas écrire le
      correctif**. D-P3-6 est une lecture, pas une mesure.
- [ ] **Step 2 : le correctif, tranché en D-P3-6.**
      `filtrer_nos_ecritures_tardives(&mut Sondeur, Option<Annonce>) ->
      Option<Annonce>` dans `sommeil/presse_papier.rs`, appelée depuis
      `registre.rs` **entre `sondeur.tour()` et `presse_papier::distribuer`**.
      Elle prend `etat().notre_ecriture`, l'arme sur le `Sondeur`, et laisse
      tomber l'annonce si son texte normalisé est le nôtre.
- [ ] **Step 3 : le verrou est pris et rendu, il ne couvre pas d'E/S.** Même
      discipline que `armer_les_gardes` (`sommeil/presse_papier.rs:372-379`) :
      la prise du verrou ne doit **jamais** encadrer un appel Win32.
- [ ] **Step 4 : 🔴 ÉCRIRE LE RÉSIDU DANS LE CODE.** `ecrire_avec` écrit **puis**
      pose `notre_ecriture` (`:357-362`, le verrou est délibérément pris après
      l'E/S) : un `tour()` qui lit dans cet intervalle ne trouvera rien. **Le
      remède rétrécit la fenêtre, il ne la ferme pas**, et le commentaire le dit
      — comme `apres_notre_ecriture` dit déjà le sien.
- [ ] **Step 5 : le test du Step 1 passe au VERT**, et le compte total monte
      **d'exactement ce qui a été annoncé**.
- [ ] **Step 6 : `cargo check --target x86_64-pc-windows-gnu`, sortie 0**, et
      **relever la NATURE des avertissements, jamais leur nombre** — il dérive
      avec la fraîcheur du build.

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `une_ecriture_notre_survenue_apres_l_armement_n_est_pas_annoncee` | **l'arbre intact** — voir le Step 1. Après remède : retirer l'appel à `filtrer_nos_ecritures_tardives` de `registre.rs` |
| `une_copie_TIERCE_survenue_apres_l_armement_est_TOUJOURS_annoncee` | 🔴 **le garde-fou du correctif** : filtrer trop large ferait taire une vraie copie. Rouge = faire filtrer sur le seul `seq` au lieu du texte |
| `le_filtre_ne_touche_pas_un_refus_de_taille` | rouge = laisser le filtre s'appliquer à `Annonce::Refus`, qui n'a pas de texte à comparer |

### Task 6 : L'état courant à l'inscription — la moitié AGENT du legs n°3 de P1

**Objet :** D-P3-2, moitié agent. **Cette tâche ne suffit pas à elle seule** : la
tâche 7 est l'autre moitié, et livrer celle-ci sans celle-là ferait **paraître**
le défaut corrigé.

**Files:**
- Modify: `agent/src/capteur/sommeil/registre.rs`
- Modify: `agent/src/capteur/sommeil/presse_papier.rs`
- Modify: `agent/src/capteur/sommeil/tests.rs` *(E13 — c'est là que vivent les tests de `registre.rs`)*

- [ ] **Step 0 :** `git log` + `git status`, **et `wc -l` sur
      `sommeil/tests.rs`** : **417 lignes, marge 83** au 21 août, et aucun budget
      de ce plan ne le protège au-delà de l'annoncer (E13). Si l'addition le
      porte au-delà de 480, **extraire avant d'écrire**, jamais comprimer.
- [ ] **Step 1 :** `Etat.dernier_presse_papier: Option<Annonce>`, posé par
      `presse_papier::distribuer` **à chaque annonce, refus compris** (§6.1).
- [ ] **Step 2 :** `inscrire` l'émet **sur le seul canal de la session qui vient
      de s'inscrire**, après `porteurs::distribuer_l_audio`. ⚠️ **Jamais un
      fan-out** : rejouer le contenu à toutes les fenêtres à chaque attache
      serait un aller-retour par attache.
- [ ] **Step 3 : 🔴 AUCUNE PURGE** (D-P3-3), et le commentaire dit **pourquoi**
      la symétrie avec `dernieres_parts` est trompeuse.
- [ ] **Step 4 :** vert, compte annoncé d'avance.

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `une_session_qui_s_inscrit_apres_une_copie_recoit_le_contenu_courant` | **l'arbre intact avant le Step 2** |
| `l_emission_a_l_inscription_ne_part_QUE_sur_le_canal_neuf` | rouge = appeler `distribuer` au lieu d'envoyer sur le seul canal : les voisines reçoivent aussi |
| `une_session_qui_s_inscrit_apres_un_REFUS_recoit_le_refus` | rouge = ne mémoriser que `Annonce::Texte` |
| `une_session_qui_s_inscrit_AVANT_toute_copie_ne_recoit_rien` | rouge = émettre un `Message::PressePapier` vide quand la mémoire est `None` |

### Task 7 : L'état courant à l'inscription — la moitié CLIENT, et la RÈGLE va où elle est testable

**Objet :** D-P3-2, moitié client. `client/src/main.ts:267` fait
`pressePapier?.recevoir(...)` alors que `pressePapier` n'est assigné qu'à `:344` :
**un message arrivé entre les deux est perdu en silence**, et l'émission de la
tâche 6 tombe précisément dans cet intervalle.

🔴 **`main.ts` N'A AUCUN TEST** (§6.2, relevé par la commande). **La règle va
donc dans `presse-papier-dom.ts`**, qui en a un ; `main.ts` ne garde que deux
lignes de câblage, **et le plan déclare qu'elles ne sont pas testées**.

**Files:**
- Modify: `client/src/presse-papier-dom.ts` *(le paramètre `initial`, et le rejeu au montage)*
- Modify: `client/src/presse-papier-dom.test.ts` *(les tests ci-dessous)*
- Modify: `client/src/main.ts` *(**deux lignes de câblage**, patron `micAnnonce`)*

⚠️ **Cette tâche est la SEULE de P3 à toucher `presse-papier-dom.ts`**, et le
§5.3 le disait intouché : **c'est une divergence de ce plan avec lui-même,
tranchée ici en faveur de la testabilité** (E15). Un plan qui dicte du code
intestable cède devant sa propre contrainte.

- [ ] **Step 0 :** `git log --oneline -5 -- client/src/main.ts client/src/presse-papier-dom.ts`
      **et `git status --porcelain client/`** — F3 écrit dans
      `client/src/fichiers/`. **Relever `wc -l` : `main.ts` 445 (marge 55),
      `presse-papier-dom.ts` 176 (marge 324), `presse-papier-dom.test.ts` 290
      (marge 210).**
- [ ] **Step 1 : le paramètre `initial?: Recu` sur `OptionsPressePapier`**, et
      le rejeu au montage **par le chemin qui existe déjà**
      (`etat.recevoir(...)` puis `ecrireSiPossible()`), jamais un second chemin.
- [ ] **Step 2 : les deux lignes de `main.ts`**, aux places du patron
      `micAnnonce`, **et rien d'autre**. La mémoire est posée **avant** le `?.`.
- [ ] **Step 3 : `cd client && npx vitest run`**, compte annoncé d'avance.
      ⚠️ **`cd client && npx vitest run` NE COUVRE PAS `proto/ts/`** : la racine
      Vitest est `client/`. Un test posé hors de `client/src/` **ne tournerait
      pas, et personne ne le verrait**.
- [ ] **Step 4 : `npm run typecheck`**, sortie 0. ⚠️ `client/` n'a pas
      `@types/node` : un test écrit avec `Buffer` passe sous Vitest et **casse le
      typecheck** — c'est la raison d'être de `verify-all.sh`, esbuild
      transpilant **sans vérifier les types**.
- [ ] **Step 5 : ÉCRIRE dans le rapport que les deux lignes de `main.ts` ne sont
      couvertes par aucun test**, et que leur seul contrôle est le critère ①.

| Test *(dans `presse-papier-dom.test.ts`)* | Ce qui le rend ROUGE |
| --- | --- |
| `un_initial_fourni_est_ecrit_au_montage_si_la_fenetre_a_le_focus` | **l'arbre intact avant le Step 1** : le paramètre n'existe pas |
| `un_initial_fourni_SANS_focus_n_est_pas_ecrit_au_montage` | rouge = appeler `ecrire` directement au montage au lieu de passer par `ecrireSiPossible` — **le dépôt différé doit rester le seul chemin**, y compris ici |
| `un_initial_ABSENT_ne_provoque_aucune_ecriture` | rouge = appeler `etat.recevoir` inconditionnellement avec un `undefined` coerçé |
| `un_initial_est_ECRASE_par_un_message_recu_ensuite` | rouge = rejouer `initial` à chaque `focus` au lieu de le confier à l'état, ce qui ferait revenir un contenu périmé |

### Task 8 : 🔴 TÂCHE DÉDIÉE — le `pid` dans la trace `fenêtre attachée au capteur`. UNE ligne.

**Objet :** D-P3-7. **Sans elle, les critères ② et ③ ne sont pas attribuables**,
et aucune autre tâche n'en dépend au compilateur — c'est exactement pourquoi elle
est dédiée. Ce dépôt a payé quatre fois pour ce geste sous une autre forme.

**Files:**
- Modify: `agent/src/capteur/fenetre.rs` *(**une ligne**, plus son commentaire)*

- [ ] **Step 0 :** `git log --oneline -5 -- agent/src/capteur/fenetre.rs`.
      **Relever `wc -l` : 426, marge 74.**
- [ ] **Step 1 :** ajouter `pid = self.pid` au `tracing::info!` de `:190-196`.
      Le champ est **déjà** sur `Fenetre` (`:149`) et **déjà** passé à `inscrire`
      (`:221`) : rien à faire remonter.
- [ ] **Step 2 : le commentaire dit à quoi il sert** — l'attribution
      session ↔ fenêtre Windows, **et le fait qu'aucune autre trace du dépôt ne
      la porte**. Sans cette phrase, un successeur le retirerait comme du bruit.
- [ ] **Step 3 : `cargo check --target x86_64-pc-windows-gnu`**, sortie 0. Ce
      fichier est `#[cfg(windows)]` : **aucun test d'hôte ne le couvre**, et la
      compilation croisée est la seule vérification disponible avant la VM.
- [ ] **Step 4 :** le contrôle qui vaut est **à la recette** :
      `grep -a 'fenêtre attachée au capteur' agent-plat.log` doit rendre **trois**
      lignes portant **trois `pid` distincts**. ⚠️ **Le mettre à plat d'abord** :
      les séquences ANSI de `tracing` séparent le nom du champ de sa valeur, et
      `grep 'pid='` ne matche jamais sur un journal brut.

| Test | Ce qui le rend ROUGE |
| --- | --- |
| aucun test d'hôte n'est possible | — `#[cfg(windows)]`. **Le contrôle est à la recette**, Step 4, et il est nommé |

---

# Famille 3 — l'instrument, puis la recette

### Task 9 : L'instrument multi-fenêtres

**Objet :** bâtir le pilote. **Aucune ligne de code de produit.** Il peut être
écrit **pendant que la VM est tenue** par F3 — c'est la seule tâche de la
famille 3 qui le puisse.

**Files:**
- Create: `docs/superpowers/plans/journaux-presse-papier-p3/instrument/pilote-pp-p3.mjs`
- Create: `docs/superpowers/plans/journaux-presse-papier-p3/instrument/lecteur-pp-n.ps1`
- Modify: aucun

- [ ] **Step 0 : RÉEMPLOYER, jamais réécrire.** Partir de
      `journaux-presse-papier-p2/instrument/pilote-pp-p2.mjs` et
      `lecteur-pp.ps1`, et du **copieur à demeure** de
      `journaux-presse-papier-p1/instrument/copieur-pp.ps1` (D-P3-10).
- [ ] **Step 1 : l'inventaire des cibles.** `Target.getTargets` **en plus** de
      l'auto-attache — P1 a perdu une exécution entière parce que l'auto-attache
      ne suffit pas. ⚠️ **Une cible ouverte par `window.open` s'attache avec une
      URL VIDE** ; l'URL n'arrive qu'au `targetInfoChanged` suivant, et un
      instrument qui teste l'URL à `Target.attachedToTarget` **ne peut jamais
      réussir, en silence**.
- [ ] **Step 2 : un écouteur de canal de contrôle PAR FENÊTRE**, indépendant du
      produit (§6.4). Il compte les `clipboard` **reçus** et retient
      `text`/`bytes`.
- [ ] **Step 3 : le relevé de focus par fenêtre et par phase** (§6.4) —
      `{hasFocus, isActive, hasBeenActive}`.
- [ ] **Step 4 : l'attribution.** Lire les `pid` dans `agent-plat.log` (tâche 8),
      les résoudre en `MainWindowHandle` par `Get-Process -Id`, et lire par
      `WM_GETTEXT`. ⚠️ **`FindWindowW('Notepad', $null)` rend ZÉRO sur une fenêtre
      présente** — PowerShell marshale `$null` en **chaîne vide** pour un
      paramètre `string` ; `[NullString]::Value` est obligatoire, et P2 a payé
      cette leçon. **`EnumWindows`, lui, la trouve** : c'est le contrôle croisé.
- [ ] **Step 5 : trois Bloc-notes PRÉ-SEMÉS** de marqueurs distincts (D-P3-9),
      et **deux relevés identiques à 300 ms d'écart** avant de croire une lecture
      (CIFS n'est pas atomique).
- [ ] **Step 6 : le pilote SORT.** Un `WebSocket` CDP tient la boucle
      d'événements après la mort de Chrome ; sans sortie explicite, le harnais
      bascule en arrière-plan alors que tout est fini, **et cela se lit comme une
      mesure interminable**.
- [ ] **Step 7 : ne JAMAIS activer `Emulation.setFocusEmulationEnabled`** (E9),
      et **le contrôler** dans le JSON de sortie.

### Task 10 : 🔴 La recette sur la VM — QUATRE critères, DEUX exécutions chacun

**Objet :** mesurer. **Aucune ligne de code de produit.**

**Files:**
- Create: `docs/superpowers/plans/journaux-presse-papier-p3/` — les journaux
  d'agent **bruts ET leurs `-plat`**, les JSON de pilote, et
  `familles-de-lecture.txt`
- Modify: aucun

🔴 **PRÉALABLE EXTERNE, hors dépendance de tâche : la VM Windows est tenue par
le chantier F3 au moment où ce plan est écrit.** Ne pas démarrer sans l'avoir
vérifié.

⚠️ **Contraintes de montage héritées, NON renégociables** — P1 en a perdu une
exécution entière sur la première, P2 deux tentatives :

- **l'agent doit s'ENRÔLER** : sans `AGENT_VM`/`AGENT_SECRET` le signaling le
  refuse, et **le symptôme se lit exactement comme une panne du produit** ;
- **une instance de plateforme au bon numéro de VERSION** : `version_emise=N
  version_recue=M` est un refus **bruyant et correct**, jamais un défaut du
  presse-papier ;
- **le jeton utilisateur semé dans `localStorage` avant toute navigation**, et
  **hors dépôt** ;
- **navigateur pilote sur l'HÔTE, jamais sur la VM** : sur la VM, la fenêtre de
  la page-shell est elle-même capturée, ce qui boucle en cascade d'ouvertures ;
- **le copieur à demeure**, né **avant** le superviseur ;
- **`unset -f chpwd`** avant tout relevé.

- [ ] **Step 0 : l'environnement.** VM démarrée et `/media/vm` **réellement
      accessible** (`ls /media/vm/dev`, pas `mountpoint` — l'entrée CIFS survit à
      une VM éteinte) ; `.env` sourcé **avant** `build-agent.sh` ;
      `cargo clean --release -p proto -p agent` ; **`Get-Process agent` vide** ;
      **taille du binaire relevée** (une compilation de 0,13 s est un aveu).
- [ ] **Step 1 : 🔴 LE NOMBRE DE FENÊTRES SE RELÈVE, IL NE S'EXIGE PAS**
      (D-P3-8). `MULTIFENETRE_VDD_PURGE=1` **dans un lancement séparé**, puis
      `grep -a 'sortie créée mais introuvable'` **avant de conclure à un
      plafond**, puis compter les `fenêtre attachée au capteur` — **jamais les
      fenêtres du navigateur**. **Écrire le nombre obtenu.** Moins de deux ⟹
      **P3 n'est pas livrable** (RP3-1).
- [ ] **Step 2 : 🔴 LE TÉMOIN DE FOCUS, joué AVANT tout critère**, sur les
      fenêtres RÉELLES du produit cette fois. C'est le rejeu de la tâche 1 en
      conditions de produit : exactement un `hasFocus === true`, et c'est celui
      qu'on a amené au premier plan. **Il peut échouer, et c'est ce qui en fait
      un témoin.** S'il échoue, **④ est déclaré NON MESURABLE et les trois
      autres critères se jouent quand même**.
- [ ] **Step 3 : le témoin de mesurabilité du sens montant**, repris de P2 :
      copier un nonce dans le presse-papier de l'HÔTE, coller dans un Bloc-notes
      de la VM, relire par `WM_GETTEXT` — **jamais par `Get-Clipboard` en
      WinRM**, qui tourne en session 0 et rend `-1` (sonde P0 de P1).
- [ ] **Step 4 : les quatre critères. DEUX exécutions chacun. Aucun taux.**

| # | Critère | Comment il est jugé | 🔴 Ce qui le rend ROUGE |
| --- | --- | --- | --- |
| ① | **Une copie dans la VM parvient aux TROIS fenêtres** | l'écouteur du §6.4 compte **un** message `clipboard` **par fenêtre**, portant le même texte. ⚠️ **Et la moitié qui manquait** : une **quatrième** fenêtre ouverte APRÈS la copie doit recevoir le contenu courant à son attache (D-P3-2) — si la VM n'en donne pas quatre, **fermer une fenêtre et la rouvrir**, et l'écrire | 🔴 **la rouge de la spec désigne un mécanisme inexistant** (E2). **Remplacée** : muter `distribuer` pour n'envoyer qu'à la **première** clé ⟹ le compte tombe à **1**, pas à 0. ⚠️ **Une mutation qui n'enverrait à personne serait moins bonne** : 0 est aussi ce que rend un mécanisme mort |
| ② | **Un collage depuis B met le texte de B dans la VM, pas celui de A** | deux textes distincts, deux fenêtres, **attribution par le `pid` de la tâche 8** puis `WM_GETTEXT` sur le `MainWindowHandle` correspondant | 🔴 la rouge est **structurelle et gratuite** : deux textes distincts. Si le texte de A apparaissait dans le Bloc-notes de B, ② tombe. ⚠️ **Sans la tâche 8, ce critère n'est pas ATTRIBUABLE** — et un relevé non attribuable n'est pas un verdict |
| ③ | **Deux collages quasi simultanés : ni interblocage, ni contenu mêlé ; le dernier gagne** | deux `Ctrl+V` à moins de 250 ms d'écart depuis deux fenêtres. **Trois choses se relèvent séparément** : (a) les deux commandes reçoivent leur réponse — `grep -a 'aucune réponse du capteur\|commande expirée'` doit rendre **0** ; (b) chaque Bloc-notes a reçu **un** texte entier, jamais un mélange de caractères ; (c) 🔵 **le compte de messages `clipboard` en RETOUR** — voir la ligne d'avertissement ci-dessous | 🔴 la rouge de l'interblocage **ne se fabrique pas** : elle est le relevé de la borne de 12 s (D-P3-15), et un zéro sur des collages **espacés** ne dirait rien. **La rouge qui vaut est celle de l'ordre** : coller T1 puis T2 et vérifier que le second Bloc-notes porte T2 — la rouge de P2 pour ③, **jamais jouée faute d'un second binaire** (legs n°1 de P2), reste due et **P3 ne la joue pas non plus** |
| ④ | **Une fenêtre sans focus n'écrit pas le presse-papier local, et l'écrit à la reprise du focus** | fenêtre B **non focalisée** au moment de la copie : son écouteur voit le message, **et `writeText` n'est pas appelée**. Puis `Page.bringToFront` sur B : **l'écriture a lieu**. ⚠️ **Le point d'observation est l'écouteur de canal ET un relevé du presse-papier local**, jamais un compte d'appels à `writeText` — le client dédoublonne | 🔴 **la rouge est aussi la MESURE du §3.3** : elle est **déjà jouée** par la tâche 2, hors VM, et **ne se rejoue pas ici — elle se CITE**. Ce que la recette rougit, c'est le produit : muter `aEcrire` pour ignorer `focalise` ⟹ B écrit sans focus |

⚠️ 🔵 **CE QUE ③ VA PROBABLEMENT EXHIBER, ET QU'IL NE FAUT PAS LIRE COMME UN
CONTENU MÊLÉ** : la course de D-P3-6 se déclenche exactement sur « deux collages
quasi simultanés ». **Si la tâche 5 a été jouée, le compte de messages en retour
doit rester à ZÉRO** ; s'il ne l'est pas, c'est que le résidu du Step 4 de la
tâche 5 a mordu — **et c'est un relevé, pas une réfutation de ③**. Le noter
séparément.

- [ ] **Step 5 : appel BLOQUANT au premier plan**, délai explicite couvrant la
      durée complète.
- [ ] **Step 6 : copier `agent.log` APRÈS la fin réelle**, sous un nom **jamais
      réutilisé** — P2 a écrasé le journal qui portait sa fuite.
- [ ] **Step 7 : `grep -a` partout**, `file` sur chaque journal, et
      **`familles-de-lecture.txt` écrit par la commande, APRÈS la dernière
      écriture**. ⚠️ **Écrire ses motifs par un heredoc entre quotes** : `echo`
      en zsh interprète `\x1b` et `\r`, et S3 a produit un fichier qui **se
      décrivait en se polluant lui-même**.
- [ ] **Step 8 : verser TOUS les journaux dans git**, bruts **et** `-plat`, et
      l'instrument dans son **état final**. 🔴 **Jamais dans un rapport
      gitignoré** : l'espace de travail de D9 a disparu avec six constats de
      revue, **définitivement perdus**.

---

# Famille 4 — la clôture

### Task 11 : 🔴 La revue transverse de fin de branche — OBLIGATOIRE

**Objet :** chercher les défauts qui **franchissent une frontière de tâche**, et
que les revues par tâche ne peuvent structurellement pas voir. Barème du dépôt :
**5** en D7, **3** en D8, **6** en D9, **douze** en D10, **sept** en D11, **huit**
en P1 de la plateforme, **dix** en P2, **cinq** en S1, **neuf** au chantier E,
**douze** en P3 de la plateforme, **douze** en S2, **onze** en F1, **huit** en
P4, **treize** en S3, **huit** en G1.

**Files:**
- Modify: les fichiers portant une affirmation devenue fausse
- Modify: `docs/superpowers/specs/2026-08-19-presse-papier-design.md` (E1, E3, E4)

- [ ] **Step 1 : la cible propre de cette revue est nommée d'avance** — les
      affirmations **rendues fausses par cette branche même**. Les candidates
      connues :
      - `client/src/presse-papier.ts:109-113`, qui affirme la contrainte du §3.3
        comme un **fait** ; la tâche 2 l'aura tranchée dans un sens ou l'autre ;
      - `agent/src/presse_papier/sondeur.rs`, doc d'`apres_notre_ecriture`
        (`:341-346` avant extraction) : elle déclare *voulu* le cas d'une copie
        tierce, et **ne dit rien de notre propre seconde écriture** — la tâche 5
        l'aura rendu incomplet ;
      - `agent/src/capteur/sommeil/registre.rs`, la doc du champ `reference` du
        `Sondeur` et celle d'`inscrire` : « une fenêtre qui s'attache ne reçoit
        donc pas le contenu déjà présent » (la doc du champ `reference` de `Sondeur`) —
        **la tâche 6 la réfute** ;
      - `client/src/presse-papier-dom.ts:80-87`, « un message arrivé avant
        l'attache est PERDU » — **la tâche 7 la réfute** ;
      - le plan de P2, §8 : « Aucune annonce de l'état courant à l'attache :
        c'est le legs n°3 de P1, et il appartient à P3 » — **fait**, et il n'en
        nommait qu'une moitié ;
      - `CLAUDE.md`, section presse-papier P1, legs n°3 et n°4.
- [ ] **Step 2 : 🔴 ÉNUMÉRER LES PLACES PAR `grep -n` AVANT D'ÉCRIRE, ET LES
      RELIRE PLACE PAR PLACE APRÈS.** « Corrigé à sa place » est une affirmation
      de **complétude**, et ce dépôt l'a payée **neuf fois** sous le nom de
      « naufrage du 487 » — dont une **à l'intérieur du commit qui la
      dénonçait**. ⚠️ **Une substitution qui ne dit pas combien d'occurrences
      elle a touchées est une affirmation de complétude non vérifiée.**
- [ ] **Step 3 : `grep -rniE` et non `grep -rn`** : deux des dix-sept places de
      la revue de S4 étaient écrites en MAJUSCULES et le `grep` sensible à la
      casse les manquait.
- [ ] **Step 4 : TRIER avant de corriger.** S4 a trouvé, sur dix-sept places,
      **trois citations en style direct qui devaient rester** et **neuf emplois
      du même mot qui nommaient autre chose**. Une substitution globale les
      aurait abîmés.
- [ ] **Step 5 : annoter les documents datés, ne pas les réécrire.** Un relevé de
      P1 ou de P2 reste **vrai comme histoire** ; le barrer le rendrait faux. Ce
      sont les **pronostics** qu'on reprend — « c'est P3 », « reste VRAI »,
      « aura besoin ».
- [ ] **Step 6 : ⚠️ LA REVUE TRANSVERSE EST ELLE-MÊME UNE SOURCE DE
      CROISSANCE.** S2 y a perdu 13 lignes de marge, S3 y a ajouté **+54 lignes**
      de commentaire, P2 y a failli annuler une extraction en ramenant
      `verify-webrtc.mjs` **exactement à son chiffre d'avant**. **Relever `wc -l`
      APRÈS les éditions de cette tâche**, et si une porte est approchée,
      **extraire — jamais comprimer**.

### Task 12 : `CLAUDE.md` gagne sa section, et ses tailles sont RELEVÉES

**Files:**
- Modify: `CLAUDE.md`
- Modify: `docs/superpowers/specs/2026-08-19-presse-papier-design.md` *(si la tâche 11 ne l'a pas fait)*

- [ ] **Step 0 :** `git log --oneline -5 -- CLAUDE.md` — **périmètre concurrent**,
      et il l'est en permanence.
- [ ] **Step 1 : 🔴 LE BALAYAGE COMPLET, PAR LA COMMANDE, APRÈS LA DERNIÈRE
      ÉDITION DE LA RONDE** — revue transverse comprise. Une table relevée en
      début de ronde est **fausse à la fin de la même ronde** : D8 a commis
      exactement cette erreur en croyant bien faire.
      ```bash
      { git ls-files; git ls-files --others --exclude-standard; } \
        | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
        | xargs wc -l 2>/dev/null | sort -rn | awk '$1>420'
      ```
- [ ] **Step 2 : CORRIGER LE TABLEAU DE DETTE dans le même mouvement**, comme
      `CLAUDE.md` l'exige de lui-même. Au 21 août il porte **QUATRE** lignes et
      **DEUX sont périmées** (§0.2) — **remesurer, ne pas recopier ce plan**, et
      **ne pas s'attribuer une résorption faite par un voisin**.
- [ ] **Step 3 : MESURER CHAQUE LIGNE DE LA TABLE AU MOMENT OÙ ON L'ÉCRIT.**
      D11 a écrit **cinq** chiffres sans les mesurer, et **les cinq étaient
      faux** — attrapés en relançant `wc -l` sur les tables entières, jamais en
      les relisant.
- [ ] **Step 4 : le tableau des variables d'environnement N'A RIEN À GAGNER** —
      P3 n'en introduit aucune (§5.3). **Le dire** : une absence se déclare.
- [ ] **Step 5 : la section porte le nombre d'exécutions dans chaque énoncé**,
      et **aucun taux**.
- [ ] **Step 6 : 🔴 CONTRÔLER QUE `proto/` N'A PAS BOUGÉ** (D-P3-13) :
      `git diff --stat <base>..HEAD -- proto/` doit être **VIDE**. S'il ne l'est
      pas, une tâche s'est trompée de conception, et **il faut l'écrire plutôt
      que de le laisser passer**.
- [ ] **Step 7 : les trois comptes de tests, RELANCÉS**, et le témoin
      `./scripts/verify-all.sh`. ⚠️ **Le lancer depuis un shell PROPRE**, ou
      `env -u TURN_URL -u TURN_SECRET` : avec `TURN_URL`/`TURN_SECRET` dans
      l'environnement — c'est-à-dire **après le `source .env` que tout travail
      sur la VM exige** — six tests de signaling échouent, **et la sensibilité
      est PRÉEXISTANTE** (relevée par G1). ⚠️ **Et le lire ÉTAPE PAR ÉTAPE** :
      l'arbre est partagé, une étape rouge peut appartenir à F3. **Le présenter
      comme vert serait faux ; comme rouge, tout autant.**

### Task 13 : Le document de résultats permanent

**Files:**
- Create: `docs/superpowers/plans/2026-08-21-presse-papier-p3-resultats.md`

- [ ] **Step 1 : la note de lecture des journaux, RELEVÉE et non supposée** —
      `file`, `grep -lP '\x1b\['`, un balayage `tr -dc '\000'`, **après la
      dernière écriture**. P1 avait **33 à 36 octets NUL** dans ses journaux de
      pilote, P2 zéro, D10 **558** : rien ne se déduit, tout se mesure.
- [ ] **Step 2 : le verdict critère par critère, avec le nombre d'exécutions DANS
      CHAQUE ÉNONCÉ.**
- [ ] **Step 3 : « Ce que P3 n'établit PAS »**, repris du §9 de ce plan et
      **complété de ce que la recette a découvert**.
- [ ] **Step 4 : « Ce que P3 lègue »**, repris du §10 et complété.
- [ ] **Step 5 : 🔴 LES PIÈCES VIVENT DANS GIT, PAS DANS UN RAPPORT DE TÂCHE.**
      L'espace de travail de D9 était gitignoré : il a disparu avec **six**
      constats de revue, **définitivement perdus**, et D9 laissait au dépôt la
      conclusion **sans sa preuve**.
- [ ] **Step 6 : ⚠️ RELIRE LES JOURNAUX CONTRE LE MESSAGE DE COMMIT, jamais
      l'inverse.** D11 a trouvé **neuf** écarts dans ses propres messages de
      commit, dont **deux affirmations fausses**, et **sept n'existaient que
      là** — un message de commit est une pièce du dépôt, et personne ne le
      relit. ⚠️ **Passer les messages longs par un fichier** (`git commit -F`) :
      des accents graves dans un `-m` se font interpréter comme des
      substitutions de commande, et D11 a mutilé trois phrases ainsi.

---

## 8. Ce que ce plan NE prescrit PAS, et pourquoi

- **Aucune élection de porteuse pour le presse-papier.** D3 l'exclut nommément,
  et pour une raison qui tient : l'utilisateur peut coller depuis n'importe
  laquelle de ses fenêtres, et rien ne permet de deviner laquelle.
- **Aucun presse-papier PAR FENÊTRE** (D3) : les applications Windows le
  partagent déjà.
- **Aucun bornage du canal `Message`** (D-P3-11).
- **Aucune file de collages** : l'écrasement du dernier reste assumé (D-P2-3 de
  P2, legs n°4), **et P3 ne le mesure pas non plus** — il faudrait deux collages
  dans un même tour de boucle d'un **même** enfant, ce que ③ ne produit pas
  (③ met deux enfants différents en concurrence).
- **Aucun propriétaire mono-fenêtre** (D-P3-14).
- **Aucun changement de `CONTROL_VERSION`, aucune variante de protocole, aucun
  fichier de `proto/`** (D-P3-13).
- **Aucune variable d'environnement neuve** (§5.3).
- **Aucun retrait de la règle du dépôt différé**, quel que soit le verdict de la
  sonde S2 (D-P3-4) — et le plan écrit pourquoi ce retrait serait une décision
  du propriétaire du dépôt, pas une conséquence mécanique.
- **Aucune mesure de latence de bout en bout**, qu'aucun sous-bloc de ce dépôt
  n'a jamais prise depuis D1.
- **Aucun rejeu de la rouge du critère ③ de P2** (le chemin naïf, `V` sur le
  canal d'entrées) : elle exige un **second binaire**, elle reste due, et **P3 ne
  la joue pas**. La justification de D6 reste donc **non éprouvée** — c'est le
  legs n°1 de P2, reconduit sans réduction.
- **Aucune mesure du critère ⑤ de P2** (`Ctrl+W`/`T`/`N`) : `--headless` n'a ni
  onglets ni fenêtres au sens de l'utilisateur, et `Xvfb` n'est toujours pas
  installé. Reconduit.

---

## 9. Ce que P3 n'établira PAS

- **Aucun taux, nulle part.** Deux exécutions par critère au mieux, une par
  rouge. **Deux exécutions établissent la reproductibilité, jamais une
  fréquence.**
- 🔴 **Rien du niveau 2 du sens VM → navigateur** : *personne n'aura vérifié
  qu'un humain peut coller.* Legs n°11 de P1, reconduit — `xclip` et `wl-paste`
  sont absents de l'hôte, `xsel` refuse en « Can't open display ».
- 🔴 **Si le témoin S1 échoue, le critère ④ n'est pas mesuré**, et c'est le
  centre du sous-bloc. **Il sera déclaré, jamais maquillé.**
- **Rien au-delà du nombre de fenêtres que la VM rend** ce jour-là — et ce nombre
  est un relevé, pas une propriété du produit.
- **Rien de la latence** d'un collage ni d'une copie.
- **Rien d'un navigateur autre que Chromium**, rien d'un Chromium **avec
  interface**, rien de Firefox, rien de Safari, **rien d'un humain**.
- **Rien du HiDPI** : `deviceScaleFactor = 1` partout, comme dans toutes les
  campagnes de ce dépôt depuis D1.
- **La visibilité et le focus restent IMPOSÉS par le pilote**, page par page —
  limite héritée de D5, qu'aucun sous-bloc n'a levée. ⚠️ **Elle touche ④ de
  plein fouet** : ce qui sera mesuré est le comportement du produit **sous un
  focus imposé par CDP**, jamais sous un clic humain. Un focus imposé n'est pas
  un focus obtenu, et **rien n'établit que les deux se valent**.
- **Rien de plus de deux collages concurrents** : ③ en met deux, pas trois.
- **Rien de l'interblocage de la borne de 12 s** : elle est **relevée**, jamais
  **provoquée** (D-P3-15).
- **Rien d'un texte non-ASCII ni multi-ligne à N fenêtres** : `normaliser` et
  `denormaliser` ne sont éprouvées que sur l'hôte, et P1 le déclarait déjà.
- **Rien du refus de taille à N fenêtres** : les deux bornes ne sont éprouvées
  que par leurs tests d'hôte, des deux côtés du canal (legs n°8 de P2).
- **`PRESSE_PAPIER_MAX` et `PERIODE_PRESSE_PAPIER` ne sont toujours pas
  calibrées**, et elles rejoignent la liste que ce dépôt tient depuis `BPP_MIN` :
  `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`, `HYSTERESIS`, `REPIT_APRES_ECHEC`,
  `TAILLE_MAX_SORTIE`, `REPIT_REARMEMENT_AUDIO`, `REARMEMENTS_MAX`.
  **Aucun jugement d'usage n'aura été porté sur aucune.**
- **Le mono-fenêtre n'a toujours pas de propriétaire** (D-P3-14).
- **Le presse-papier reste PARTAGÉ entre les fenêtres d'une même session** (D3)
  **et entre deux utilisateurs d'une même VM** — la seconde est une question de
  **confidentialité**, réelle, hors périmètre v1, et elle appartient au
  sous-projet ⑤.

---

## 10. Ce que P3 léguera

**Legs de P1 réglés** : n°3 (l'état courant à l'attache — **les DEUX moitiés**).
**Legs de P2 réglé** : n°6 (« deux collages concurrents depuis deux fenêtres sont
hors de ce qui est établi. **P3 les rencontrera** ») — **rencontrés et mesurés**.

**Ce qui reste dû :**

1. 🔴 **La rouge du critère ③ de P2 n'est toujours pas jouée**, et **D6 reste du
   coût dont rien n'établit la nécessité** (legs n°1 de P2). Elle exige un
   binaire distinct qui envoie la touche `V` sur le canal d'entrées. Point de
   chute : `client/src/input.ts`.
2. 🔴 **Le critère ⑤ de P2 reste NON MESURÉ** — `Xvfb` + `xdotool`, consentement
   donné en D8, jamais suivi d'effet. ⚠️ **Les mesures qui en sortiraient ne se
   compareraient à aucune campagne antérieure.**
3. 🔴 **Le niveau 2 du sens VM → navigateur reste NON MESURABLE** (legs n°11 de
   P1) : *personne n'a jamais vérifié qu'un humain peut coller.*
4. ⛔ **Le propriétaire MONO-FENÊTRE n'existe toujours pas** (legs n°1 de P1).
   Point de chute : `agent/src/demarrage.rs`, **426 lignes, marge 74**.
5. ⛔ **Le canal `Message` reste NON BORNÉ** (legs n°4 de P1), et P3 l'aggrave
   d'un message par attache. **Le borner est un changement de conception**
   (D-P3-11).
6. ⛔ **Le collage écrase le précédent, sans trace** (legs n°4 de P2), et **P3 ne
   l'exerce pas non plus**.
7. ⛔ **La borne de 12 s de `commander` n'a toujours pas couru** (legs n°5 de P2),
   même si P3 est le premier à l'avoir mise sous concurrence.
8. ⛔ **Le résidu de D-P3-6** : `ecrire_avec` pose `notre_ecriture` **après**
   l'E/S Win32, et le correctif de la tâche 5 ne ferme pas cet intervalle.
   Le fermer demanderait de tenir le verrou autour de l'E/S — **ce que
   `sommeil/presse_papier.rs:357-362` interdit nommément**, et pour une bonne
   raison. **Écrit, non fermé.**
9. ⛔ **Le retrait de la règle du dépôt différé**, si la sonde S2 réfute le §3.3 :
   **décision du propriétaire du dépôt** (D-P3-4), avec son coût nommé — N
   écrivains concurrents pour une copie, régime que rien ne mesure.
10. ⛔ **Un focus IMPOSÉ n'est pas un focus OBTENU**, et rien n'établit que les
    deux se valent. Limite héritée de D5, jamais levée.

---

## 11. Risques qui rendraient ce sous-bloc NON LIVRABLE

| # | Risque | Gravité | Ce qui le lève, ou le borne |
| --- | --- | --- | --- |
| **RP3-1** | 🔴 **La VM ne rend qu'UNE fenêtre** | **éliminatoire** : ①②③ deviennent tous non mesurables | tâche 10 Step 1 — purge du vivier **dans un lancement séparé**, `grep -a 'sortie créée mais introuvable'`, et `MULTIFENETRE_MODE_SORTIE=1280x720` **encore à part** si le registre est pollué. À **deux** fenêtres, les quatre critères restent jugeables et le relevé écrit « deux ». À **une**, **s'arrêter et le déclarer** |
| **RP3-2** | 🔴 **Le témoin S1 échoue : `--headless` ne distingue pas le focus** | ④ non mesuré, et c'est le centre du sous-bloc | tâche 1, jouée **la première et hors VM**. Les quatre issues sont écrites d'avance, **y compris les trois qui sont des échecs**. ⚠️ **Non éliminatoire pour ①②③** |
| **RP3-3** | 🔴 **L'attribution session ↔ fenêtre reste non observable** | ② et ③ non **attribuables**, donc non jugeables | tâche 8, **dédiée, une ligne**, et son contrôle est à la recette (trois `pid` distincts). ❌ **La voie « coller un nonce et voir qui a grandi » est CIRCULAIRE** et rejetée (D-P3-7) |
| **RP3-4** | ⚠️ **La VM est tenue par F3** | la recette entière | préalable **EXTERNE** nommé en tête de la tâche 10. Les tâches 1 à 9 n'en dépendent pas, **et la tâche 9 peut être écrite pendant ce temps** |
| **RP3-5** | ⚠️ **La sonde S2 ne peut pas atteindre `{focus: ✘, activation: ✔}`** | le §3.3 reste **supposé** | tâche 2 Step 3 : **le dire est un verdict recevable**. ⚠️ **Fabriquer un verdict ne le serait pas** |
| **RP3-6** | 🔴 **Une seule moitié du legs n°3 est livrée** | le défaut **paraît** corrigé et reste intermittent — pire qu'un défaut connu | D-P3-2 : les tâches 6 **et** 7, et le critère ① les mesure **ensemble, de bout en bout**, par une fenêtre attachée APRÈS la copie |
| **RP3-7** | 🔴 **`Emulation.setFocusEmulationEnabled` rend ④ vacueux EN SILENCE** | on croirait ④ tenu | E9 : `grep` sur les deux pilotes versés rend **zéro occurrence** aujourd'hui, et la tâche 1 Step 1 **le contrôle explicitement** dans le JSON |
| **RP3-8** | 🔴 **La course de D-P3-6 se déclenche pendant ③ et est lue comme un « contenu mêlé »** | ③ jugé faux sur un phénomène étranger | prédit **d'avance** en tâche 10, avec sa ligne d'avertissement, et **compté séparément** |
| **RP3-9** | ⚠️ **Le test de la tâche 5 passe du premier coup** | D-P3-6 serait une lecture fausse | tâche 5 Step 1 : **s'arrêter, l'écrire, ne PAS écrire le correctif**. C'est un résultat, pas un échec |
| **RP3-10** | ⚠️ **`presse_papier.rs` ne s'extrait pas proprement** | dette neuve sur un fichier à marge 59 | tâche 3, **avant** la tâche 5. Son Step 7 impose d'**écrire le chiffre obtenu** et de s'arrêter s'il ne suffit pas |
| **RP3-11** | ⚠️ **`sommeil/tests.rs` (417, marge 83) franchit sans être budgété** | plafond franchi hors table | E13 : la tâche 6 le relève **avant** d'y écrire, et **extrait** au-delà de 480 — jamais ne comprime |
| **RP3-12** | ⚠️ **Trois Bloc-notes indistinguables** | contenu mal attribué | D-P3-9 : pré-semer des marqueurs distincts, **et cela ne remplace pas la tâche 8** |
| **RP3-13** | ⚠️ **Deux exécutions de recette se chevauchent** | un journal versé sous le nom d'une exécution est celui d'une autre | F1 l'a payé : `Get-Process agent` **après chaque tentative, y compris échouée**, et un nom de journal **jamais réutilisé** |
| **RP3-14** | ⚠️ **Une rouge rougit pour la mauvaise raison, ou ne mute rien** | on croit un garde éprouvé | §6.3, harnais **obligatoire** : diff **non vide**, et le relevé dit **quelle assertion** a rougi. S3 a dû refaire **quatre rouges sur seize** |
| **RP3-15** | ⚠️ **Un `grep` de recette cherche une chaîne que le produit n'émet pas** | un succès se lit comme un échec | §2.2 règle 10 : **vérifier chaque motif contre le CODE**, et le voir rendre non-vide **sur le VERT** avant de conclure quoi que ce soit du rouge |
| **RP3-16** | ⚠️ **`verify-all.sh` échoue sur une étape de F3** | on croirait P3 rouge | tâche 12 Step 7 : shell **propre**, lecture **étape par étape**, et **nommer l'étape qui appartient à l'autre** |
| **RP3-17** | ⚠️ **Les deux lignes de câblage de `main.ts` ne sont couvertes par aucun test** | la moitié client du legs n°3 pourrait être fausse sans qu'un test le dise | E15 : **la RÈGLE est ailleurs et testée** (`presse-papier-dom.ts`), et le seul contrôle de ces deux lignes est le **critère ①** — une fenêtre attachée APRÈS la copie. 🔴 **Si ① n'est pas joué dans cette forme-là, elles ne sont éprouvées par rien**, et il faut l'écrire |

---

## 12. Ce que ce plan n'a pas fait, et qu'il déclare

- **Il n'a joué AUCUNE des rouges qu'il prescrit.**
- **Il n'a joué NI la sonde S1 NI la sonde S2** : leurs verdicts sont écrits
  d'avance sous forme de branches, pas de prédictions.
- **Il n'a rien mesuré sur la VM** : elle est tenue par F3, et tous les faits
  d'agent de ce document sont des **lectures de code**, étiquetées comme telles.
- **D-P3-6 est une LECTURE, jamais une mesure.** Le Step 1 de la tâche 5 est ce
  qui la transformerait en fait — **et il peut la réfuter**.
- 🔵 **Il s'est RÉFUTÉ LUI-MÊME une fois, et l'écart est écrit plutôt qu'effacé
  (E15).** Sa première rédaction prescrivait un test pour `client/src/main.ts` ;
  `ls client/src/*.test.ts` établit qu'il n'en existe aucun et qu'il ne peut pas
  en exister. **La règle a été déplacée là où elle est testable, et le §5.3 a été
  corrigé après coup.** Un plan qui dicte du code intestable cède devant sa
  propre contrainte.
- **Il n'a pas corrigé `CLAUDE.md`**, dont le tableau de dette porte deux lignes
  périmées (§0.2) : c'est un périmètre concurrent, et la correction appartient à
  la tâche 12, qui remesurera à sa date.
