# Sous-bloc G2 — les icônes 256, et la preuve que c'en est : RÉSULTATS

**Plan :** `docs/superpowers/plans/2026-08-20-gestion-apps-g2.md`.
**Conception :** `docs/superpowers/specs/2026-08-19-gestion-apps-design.md`.
**Journaux :** `docs/superpowers/plans/journaux-gestion-apps-g2/` — **26 fichiers
suivis par git**, relevé par la commande.

---

## 0. Les journaux : DEUX familles de lecture, MESURÉES

| Famille | Fichiers | Ce qu'il faut faire |
| --- | --- | --- |
| les **sept** journaux d'agent BRUTS (`agent-*.log`) | 7 | **séquences ANSI de `tracing` PRÉSENTES** : `sed 's/\x1b\[[0-9;]*m//g'` — ou lire le `-plat` jumeau, **versé pour chacun** |
| tout le reste — les `-plat`, les `.txt`, les `.json`, le journal de plateforme | 19 | rien : ils se `grep`ent à plat |

✅ **Relevé par la commande, pas supposé** : `grep -rlP '\x1b\['` rend **7**
fichiers, tous nommés ci-dessus ; **0 octet NUL** dans aucun fichier (à la
différence de D10, dont un journal en portait 558 et faisait rendre à `grep`
une sortie **vide** indiscernable d'un zéro) ; **0 fichier non-UTF-8**.
⚠️ **17 fichiers portent des CR** — ils viennent de la VM, et cela ne gêne
aucun `grep`. **Les CR et les séquences ANSI ne coïncident PAS**, et le dire
évite une règle fausse.

---

## 1. Le verdict, en faits qui ne se simplifient dans aucun sens

**① LES HUIT CRITÈRES SONT TENUS.** Le sous-bloc n'avait aucune porte
éliminatoire — le plan l'écrivait, mesures à l'appui — et il n'en a rencontré
aucune.

**② LA RECETTE A TROUVÉ DEUX DÉFAUTS DE PRODUIT QU'AUCUN TEST D'HÔTE NE
POUVAIT VOIR**, et le premier vidait de sa substance la mesure même que le
sous-bloc existe pour prendre.

**③ TROIS PLAFONDS DE 500 LIGNES ONT ÉTÉ FRANCHIS SANS QUE JE LES VOIE
PASSER**, et rattrapés par trois extractions au relevé de clôture. Ils
n'auraient pas dû l'être : la doctrine est d'extraire AVANT.

**Ces trois faits ne se compensent pas.** Le premier porte sur ce que le
produit fait ; le deuxième sur ce que la mesure a coûté ; le troisième sur une
discipline que ce sous-bloc a appliquée trois fois correctement (les trois
extractions prévues) et manquée trois fois.

---

## 2. Les huit critères. DEUX EXÉCUTIONS AU MIEUX. AUCUN TAUX.

| # | Critère | Verdict | Le chiffre, **relevé** |
| --- | --- | --- | --- |
| ① | 256×256 et **alpha non trivial** | **TENU** | **92 des 96** PNG distincts portent un alpha ni 0 ni 255 ; **96/96** en 256×256 |
| ② | `source_max` **distingue** un vrai 256 d'un agrandissement | **TENU** | témoin 48 → PNG **256×256**, `source_max = {"pixels":48}` ; témoin 256 → PNG **256×256**, `{"pixels":256}` |
| ③ | le corpus porte **les deux valeurs** | **TENU** | **69** à 256, **14** en dessous (48 : 9, 32 : 5), **71** `non-mesuree` |
| ④ | `NonMesuree` **jamais** un nombre | **TENU** | **71** `"non-mesuree"` sur le fil, **0** `{"pixels":0}`, **0** `"source_max":0` |
| ⑤ | une icône connue n'est **pas** retéléversée | **TENU** | magasin plein → **ZÉRO** inventaire poussé, **ZÉRO** octet ; `icones=0` aux tours 2 et 3 |
| ⑥ | la déduplication **paie** | **TENU** | **156** applications, **98** empreintes → **58 téléversements évités (37,2 %)** |
| ⑦ | le magasin **se reconstruit tout seul** | **TENU** | 98 fichiers → **0** → **98**, **même poids exact** (3 215 893 o) |
| ⑧ | un agent **v2** face à une plateforme **v3** RENONCE | **TENU** | **1** ligne de refus, `version_emise=2 version_recue=3`, et **0** reprise |

### Ce que le critère ① ne dit PAS, et il faut le dire

🔴 **IL NE VAUT RIEN SUR LA TAILLE.** Mesuré deux fois — par la spécification
le 19 août, par le plan le 20 — puis **une troisième fois sur le produit** : un
`.ico` ne contenant QU'UNE entrée 48×48 rend **256×256 32bpp**. Le `96/96 en
256×256` ci-dessus ne prouve donc **rien** de la provenance.

⚠️ **ET IL NE PEUT PAS EXIGER « TOUTES ».** **92 sur 96**, donc **quatre sans
alpha** — et la mesure M3 du plan relevait **149 sur 153**, soit **quatre**
également. Un critère écrit « toutes » serait faux par construction. Sa rouge
reste entière : `WICBitmapUseAlpha` → `WICBitmapIgnoreAlpha`, une **constante
nommée**, ferait tomber le compte à **zéro**.

### Le critère ⑧ vaut par son CONTRASTE, pas par son chiffre

Au moment de G1, ce même montage rendait **0 ligne de refus et 10 reprises**.
Il rend aujourd'hui **1 et 0**. C'est la correction du 20 août 2026 — le refus
sorti du versionnement — et **G2 est le premier bump depuis, donc le premier à
pouvoir le prouver**.

---

## 3. La tâche 10 — la détermination de WIC : VERDICT POSITIF

**Deux exécutions, DEUX PROCESSUS DISTINCTS**, 154 chemins de chaque côté :

```
chemins PRESENTS DANS LES DEUX : 154
  empreintes EGALES     : 154
  empreintes DIFFERENTES: 0
```

Le plan demandait **dix** icônes ; il y en a **154**.

🔴 **LA RÉDACTION DU VERDICT EST CONTRAINTE, ET ELLE A ÉTÉ RESPECTÉE.** Un
journal incomplet n'aurait **pas** été un verdict positif ; une empreinte
absente n'aurait **pas** été un verdict négatif — ce sont des mesures non
prises. Les deux journaux sont **complets** (154 = 154).

**Conséquence** : le repli nommé d'avance — n'empreindre que
`IHDR`/`PLTE`/`IDAT`/`IEND`, dans le module PUR — **n'a pas eu à être écrit**.

⚠️ **Portée** : une VM, un corpus, un jour, deux processus. **Rien** n'est
mesuré d'une autre machine, d'une autre version de Windows, ni d'un
redémarrage de la VM entre les deux.

---

## 4. 🔴 Les deux défauts que seule la VM pouvait trouver

### 4.1 Un nom nul n'est pas « la première ressource »

Une première rédaction de `lecture_pe.rs` passait `PCWSTR(null())` à
`FindResourceW` pour un index de 0, **en croyant demander le premier groupe
d'icônes**. `FindResourceW` cherche alors une ressource dont le NOM est nul, et
n'en trouve aucune.

**Mesuré : 148 des 154 applications** rendaient `aucune ressource RT_GROUP_ICON
dans ce module` — y compris des modules qui en portent manifestement, comme
`steam.exe`.

**Ce que cela coûtait** : le catalogue restait juste, les icônes étaient
servies, et **seule la PROVENANCE tombait à `NonMesuree`** — c'est-à-dire très
exactement la chose que tout le sous-bloc existe pour mesurer. Le produit
aurait eu l'air de fonctionner.

**Le remède était écrit dans le plan** : `EnumResourceNamesW` figure dans la
liste d'appels de sa tâche 9, et la sonde M1 l'employait. **Son omission est ce
qui a produit le défaut.** Après correction, l'histogramme du produit passe de
`4 mesurées / 149 non-mesurées` à **`69 / 9 / 5 / 71`**.

> 🔴 **ET CE DÉFAUT N'A ÉTÉ VU QUE PARCE QUE J'AI RE-MESURÉ AVEC LE BON
> `RUST_LOG`.** Un premier relevé rendait `ressource illisible : 0`, et **ce
> zéro ne voulait RIEN DIRE** : `RUST_LOG` n'activait `debug` que pour
> `apps::boucle`, pas pour `apps::icone`. **Un zéro rendu par une trace qu'on
> n'a pas allumée n'est pas une mesure** — c'est le piège maison « un contrôle
> qui ne peut pas échouer », sous une forme que je n'avais pas vue listée : le
> contrôle était bon, c'est son alimentation qui manquait.

### 4.2 Un désarmement n'est pas un échec

Sous `ICONES=0`, la boucle émettait **`icones_echouees=156`** : un exploitant
aurait lu 156 pannes sur un agent parfaitement sain **qu'il venait lui-même de
couper**.

C'est **exactement** le défaut que le legs n°7 de G1 portait sur `retenus` — un
compteur qui ment sur son nom — et que G2 venait de fermer. Après correction :
`icones=0 icones_echouees=0 icones_distinctes=0`, catalogue **complet**
(156 clés).

---

## 5. Ce que le sous-bloc livre, et trois faits de conception qui lui survivront

| Étage | Fichier | Nature |
| --- | --- | --- |
| la **preuve** | `agent/src/apps/icone/ressource.rs` (127) | **PUR, aucun `cfg`** — `GRPICONDIR` et `ICONDIR` depuis un `&[u8]`, 10 tests d'hôte contre **deux témoins versés** |
| les témoins | `agent/testdata/g2-temoin-{48,256}.ico` + `fabriquer-temoins-ico.py` | fabriqués **sur l'hôte, sans Windows**, et le script est versé **avec** eux |
| l'adressage par contenu | `agent/src/apps/icone/magasin.rs` (132) | **PUR** — aucune ligne de cryptographie neuve, `apps::sha256` réemployé tel quel |
| la provenance | `agent/src/apps/icone/source.rs` (90) | **PUR** — la règle « chemin vide ⇒ la CIBLE », qui vaut **92 applications sur 153** |
| l'extraction et WIC | `agent/src/apps/icone/extraction.rs` (233) | `#[cfg(windows)]` — `WICBitmapUseAlpha`, une **constante nommée** |
| le magasin de disque | `plateforme/src/apps/icones.ts` (140) | recalcule l'empreinte, écrit **atomiquement**, interroge le **DISQUE** |
| les deux routes | `plateforme/src/http/routes-icone.ts` (313) | `?e=` dans l'URL, découpage **par segments** |

**Trois faits de conception :**

1. 🔴 **LA PREUVE VIENT DE LA RESSOURCE, ET ELLE NE PEUT PAS VENIR D'AILLEURS.**
   C'est la coupure que la spécification §6 désigne comme « le point le plus
   important », et c'est elle qui donne au critère ② un test d'hôte. Sans elle,
   sa seule preuve serait un argument de flot de contrôle — la situation exacte
   que le défaut F1 du sous-bloc D7 a payée.
2. 🔴 **`immutable` N'EST HONNÊTE QUE PARCE QUE L'URL PORTE L'EMPREINTE.** La
   spécification écrivait les deux moitiés d'une contradiction ; le `?e=` la
   tranche, et un `e` périmé rend **404**. Sans ce refus, une vieille URL
   servirait l'icône **courante** sous un en-tête immuable, empoisonnant le
   cache **pour un an** avec une image qui n'est pas celle que l'URL nomme.
3. 🔴 **UN BLOB NE TRAVERSE PAS LA DOUBLE PASSE SANS MENTIR.** Postgres n'a pas
   de `BLOB` ; SQLite accepte n'importe quel nom de type par affinité. Écrire
   `BYTEA` passerait **les deux passes** en signifiant deux choses différentes —
   le piège `SERIAL` **à l'envers**, qu'aucun des deux gardes du dépôt
   n'attrape. D'où le disque.

---

## 6. Les rouges — TRENTE-SIX, chacune vue

Toutes jouées **une mutation à la fois**, avec **copie de sauvegarde nommée** et
**jamais `git checkout`**, `sha256` comparés après restauration.

| Famille | Nombre | La plus instructive |
| --- | --- | --- |
| protocole (Rust + TS + vecteurs) | 8 | **R8** — `typesDepuis()` non mis à jour fait tomber 3 tests **ET le typecheck** (`TS2741`) : la liste est DÉRIVÉE de l'union, remède structurel que P1 opposait à `TYPES_AGENT` |
| modules purs de l'agent | 11 | **6a** — `bWidth == 0` rendu comme `0` : 4 tests, et c'est la seule invisible sans les témoins |
| agent Windows + script | 2 | **9** — `Win32_Graphics_Imaging` retirée : `unresolved import`, la forme exacte sous laquelle G1 a découvert `Win32_System_Registry` |
| plateforme | 15 | **19a** — `startsWith` : **c'est la mutation qui a SURVÉCU en G1**, et elle rougit ici parce que le test compare le **CORPS** du 404 sur six chemins |

### 🔴 DEUX ROUGES ONT REFUSÉ DE ROUGIR, ET LES DEUX ÉTAIENT DES TROUS RÉELS

- **14b** — `NOT NULL` sur une table peuplée laissait **trois tests verts**.
  C'est **exactement** le piège que la divergence E8 du plan annonce : sur une
  base NEUVE la table est vide, et `NOT NULL` passe. Un test a été écrit pour
  l'atteindre.
- **17a** — `source_max` reconstruite en `Pixels(0)` sur `NULL` laissait
  **35 tests verts** : la chaîne `NULL → 'non-mesuree'` n'était gardée par
  **rien**. C'est le critère ④ « jusqu'au bout de la chaîne » qui n'avait aucun
  témoin. Deux tests écrits, dont l'aller-retour complet par la base.

**Les deux ont été rejouées après correction, et rougissent.**

---

## 7. Les défauts du PLAN, relevés et corrigés

| # | Ce que le plan affirme | Ce que la mesure rend |
| --- | --- | --- |
| 1 | D1 : un champ `icone` manquant est **refusé** (« aucun `default` ») | 🔴 **FAUX.** `serde_derive` traite tout `Option<T>` comme portant un `#[serde(default)]` IMPLICITE, et `deny_unknown_fields` n'y change rien — il regarde les champs EN TROP. **Mesuré** sur deux structures identiques à ce détail près. Remède : `icone_obligatoire`, un `deserialize_with` qui ne fait que déléguer |
| 2 | Tâche 8 : des cas « tirés du corpus versé » | ⚠️ Le corpus **ne porte aucun champ d'`IconLocation`** — six champs, et sa propre notice le dit. Les **cibles** en viennent ; les `IconLocation` sont les lignes **verbatim de M1** |
| 3 | Tâche 12 : un « `PUT` HTTP » | ⚠️ **L'agent n'a AUCUN client HTTP** — ni `reqwest`, ni `ureq`, ni `hyper`, relevé dans `Cargo.lock`. La requête est écrite à la main, et **aucune pile TLS n'existe** : un `wss://` est **refusé explicitement, avec sa trace** |
| 4 | Structure : `icone.rs` est `#[cfg(windows)]` **et** ses enfants purs sont déclarés dans `apps.rs` « ce qui évite le `#[path]` » | ⚠️ **Les deux ne tiennent pas ensemble** : on ne peut pas déclarer un petit-fils depuis le grand-parent sans `#[path]`. L'INTENTION est retenue par la seule construction qui la serve — parent libre de `cfg`, parties Windows gatées dedans |
| 5 | E10 : `routes-applications.test.ts` porte **dix-sept** tests | ⚠️ Il en porte **DIX-NEUF**. Le nombre a dérivé depuis G1 sans que personne ne le reprenne |
| 6 | E8 : la liste des migrations est figée en **un** endroit | ⚠️ Il y en a **DEUX** — `pilotes.test.ts` et `index.test.ts`, dont le commentaire nomme lui-même le `grep` qui les trouve |
| 7 | Tâche 17 : `catalogue.ts` doit faire voyager les deux champs | ✅ **Rien à changer** : il fait voyager des `Application` entières, donc les champs neufs traversent par leur type |

---

## 8. Ce que G2 n'établit PAS

- **Aucun taux, nulle part.** Deux exécutions au mieux par critère ; **une
  seule** pour plusieurs relevés annexes.
- 🔴 **AUCUNE ICÔNE N'A ÉTÉ REGARDÉE.** `source_max` dit **d'où vient l'image**,
  jamais si elle est bonne. C'est la lacune exacte que `BPP_MIN` traîne depuis
  le chantier C volet 1.
- **Aucune constante calibrée** : `ICONE_MAX_OCTETS` (majorante à vue, **aucune
  taille individuelle n'a été relevée**), le défaut de `PLATEFORME_ICONES`, le
  `DELAI` de 10 s du téléversement, et la règle « extraire quand la clé est
  neuve ou que le `chemin` a changé » — **dont le taux de réextraction inutile
  n'est mesuré par rien**.
- 🔴 **RIEN D'UNE ICÔNE QUI CHANGE SANS QUE LE RACCOURCI CHANGE.** Une mise à
  jour qui réécrit son `.exe` **en place** — même chemin, même icône déclarée,
  image différente — **ne sera pas revue**. Trou **nommé**, pas oublié.
- **Le téléversement ne fait pas de TLS**, et le refuse par son nom.
- **71 des 154 applications restent `NonMesuree`**, et **on ne sait pas
  pourquoi pour toutes** : `runcmdu.exe` (×26), `powershell.exe`, `odbcint.dll`
  y figurent. Qu'elles n'aient réellement aucun `RT_GROUP_ICON` est **plausible
  et non vérifié**.
- **Rien du HiDPI, rien d'un client réel** : la recette est `curl`, et **aucune
  page de hub n'existe**.
- **Rien d'une icône servie sans jeton**, donc **rien du manifeste PWA** : un
  `<img src>` ne porte pas d'`Authorization`, et G5 devra le trancher.
- **Rien de la charge** : une VM, un catalogue, un magasin.
- **Le nettoyage du magasin n'existe pas** : une icône dont plus aucune
  application ne porte l'empreinte **reste sur le disque, pour toujours**.
- **L'écart `153`/`154` de la divergence E4 n'est pas expliqué**, et la recette
  en ajoute un second : **156** après les deux témoins.
- **Aucun test d'hôte ne couvre `extraction.rs` ni `lecture_pe.rs`** — leur
  seule preuve est la recette, et c'est elle qui a trouvé leur défaut.

---

## 9. Pièges neufs — à connaître avant de toucher à ce terrain

- 🔴 **UN ZÉRO RENDU PAR UNE TRACE QU'ON N'A PAS ALLUMÉE N'EST PAS UNE MESURE.**
  `ressource illisible : 0` avec un `RUST_LOG` qui n'active `debug` que pour un
  AUTRE module. **Vérifier que la trace cherchée PEUT sortir avant de lire son
  compte.**
- 🔴 **DEUX RACCOURCIS À LA MÊME CIBLE SANS ARGUMENTS SONT LA MÊME
  APPLICATION.** Le premier montage du critère ② a perdu un témoin sur deux —
  ce n'était pas un défaut du produit, c'était la déduplication (spec D4)
  faisant son travail sur un montage qui l'ignorait. **Donner des arguments
  distincts.**
- 🔴 **`Cache-Control` ET `cache-control` SONT DEUX CLÉS D'OBJET DISTINCTES**, et
  `writeHead` émet **les deux** : le client lisait `no-store, private,
  max-age=…`, une réponse à la fois non stockable et immuable. **La casse doit
  être celle de l'objet qu'on écrase.**
- ⚠️ **UN `#[serde(default)]` PEUT ÊTRE IMPLICITE.** Tout champ `Option<T>` en
  porte un, et `deny_unknown_fields` ne le voit pas.
- 🔴 **`build-agent.sh` RSYNCHRONISE L'ARBRE ENTIER**, donc le travail **non
  commité et non compilant** d'un chantier voisin. Le remède est un
  `git worktree` à son propre commit — **avec `node_modules` lié**, sans quoi le
  script s'arrête en silence après « sources synchronisées ».
- ⚠️ **LA VM ÉTAIT ÉTEINTE** alors qu'un rapport la disait démarrée depuis
  1 j 11 h. **Vérifier `virsh list --all`, ne pas croire.**
- ⚠️ **Les tests écrivaient le magasin par défaut DANS LE DÉPÔT**
  (`plateforme/donnees/icones`). Fixtures pointées sur un répertoire
  temporaire, et le défaut de production gitignoré.
- ⚠️ **`zsh` ne découpe pas les variables en mots** : un `git add $FICHIERS` a
  échoué en passant la liste entière comme un seul chemin. Piège déjà écrit dans
  `CLAUDE.md`, repayé ici.

---

## 10. Les legs

1. ⛔ **71 applications restent `NonMesuree`, et la cause n'est établie pour
   aucune.** Le savoir demanderait d'énumérer les ressources de chaque module.
2. ⛔ **Aucune icône n'a été regardée** — la lacune de `BPP_MIN`, transposée.
3. 🔴 **Un `<img src>` ne porte pas d'`Authorization`** : **G5 ne pourra pas
   pointer cette route depuis un manifeste PWA**. Le trancher demande de
   décider si une icône se sert sans jeton — décision de sécurité, qui
   appartient au propriétaire du dépôt.
4. ⛔ **Une icône qui change sans que le raccourci change n'est jamais revue.**
5. ⛔ **Le magasin n'est jamais nettoyé.**
6. ⛔ **Le téléversement ne fait pas de TLS** — il le refuse par son nom, et le
   jour où l'agent devra franchir un lien non fiable, ce sera **une dépendance
   à décider, pas à glisser**.
7. ⛔ **`ICONE_MAX_OCTETS` n'est pas calibrée**, et **aucune taille individuelle
   d'icône n'a jamais été relevée**.
8. ⛔ **`magasin::manquantes` (agent) n'a aucun appelant de production** — c'est
   le jumeau hôte-testable de la règle de la plateforme, conservé et **déclaré**.
   ⚠️ Ce dépôt n'a **toujours pas** de doctrine sur le code orphelin.
9. ⚠️ **Les deux raccourcis témoins RESTENT sur le Bureau de la VM**
   (`G2 Temoin 48.lnk`, `G2 Temoin 256.lnk`, arguments distincts). C'est ce qui
   rend le critère ② rejouable — **et cela porte le corpus de 154 à 156**. Un
   chantier suivant qui compterait 154 les cherchera.
10. ⛔ **La lacune de nommage d'`IssueLancement` reste OUVERTE.** G2 ajoute un
    second témoin (`SourceMax::NonMesuree`, deux mots) sans refermer celle-là.

---

## 11. Le contrôle final

```
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
```

**Relevé APRÈS la dernière édition — le tableau de dette a DEUX lignes :**
`agent/src/encode.rs` **1536**, `agent/src/windows_source.rs` **630**, ni l'un
ni l'autre touché par G2. ✅ **Les deux fichiers de `proto/` que `CLAUDE.md`
inscrivait SANS point de chute en sont SORTIS.**

**Comptes de clôture** — `cargo test -p proto` **93**, `cargo test -p agent`
**778**, `proto` vitest **176**, `plateforme` **472** sur les **deux** moteurs,
`cargo check --target x86_64-pc-windows-gnu` **sortie 0**, et **AUCUN
avertissement hors de la famille `dead_code`** — vérifié par filtrage, pas
supposé. ⚠️ **Leur NOMBRE n'est PAS rapporté** : il vaut 18 dans un `git
worktree` au commit de G2 et 19 dans l'arbre partagé, et ce dépôt écrit depuis
D3 qu'on vérifie **la nature, jamais le nombre**, trois `typecheck` à 0.

⚠️ **`cargo test -p agent` rend 778 dans l'arbre PARTAGÉ et 735 dans un
`git worktree` à mon propre commit.** L'écart n'est **pas** de G2 : c'est le
travail non commité du chantier pont-fichiers. **Un compte de tests n'est
attribuable qu'assorti de son arbre** — leçon de D11, repayée ici.
