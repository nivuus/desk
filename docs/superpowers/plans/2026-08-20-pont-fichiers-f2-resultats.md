# Sous-projet ③ Pont fichiers — sous-bloc F2 : résultats

**21 août 2026.** Document **permanent**, versé dans git — D9 a perdu **six**
constats de revue parce que sa preuve vivait dans un espace gitignoré.

Plan : `docs/superpowers/plans/2026-08-20-pont-fichiers-f2.md`.
Conception : `docs/superpowers/specs/2026-08-19-pont-fichiers-design.md`.
Journaux : `docs/superpowers/plans/journaux-pont-fichiers-f2/` — **44 fichiers
suivis par git** (36 au premier niveau, 8 sous `instrument/`), **DEUX familles de
lecture depuis la recette**, et c'est **mesuré, pas supposé** :

| Famille | État | Ce qu'il faut faire |
| --- | --- | --- |
| tout ce qui vient de l'hôte (`f2-*.txt`, `pilote-*.log`, `pilote-*.json`, `instrument/`) et les `agent-*-plat.log` | UTF-8, **aucune séquence ANSI** | rien : ils se `grep`ent à plat |
| les **sept** `agent-*.log` **bruts**, copiés de la VM | UTF-8, **CRLF**, **séquences ANSI de `tracing` PRÉSENTES** | `sed 's/\x1b\[[0-9;]*m//g'` — ou lire le `-plat` jumeau, **versé pour chacun** |

✅ **AUCUN fichier ne porte d'octet NUL**, relevé par la commande sur les **36** du premier niveau
après la dernière écriture — donc, contrairement aux journaux de pilote de P1 et
de D10, **`grep -a` n'est ici obligatoire nulle part**. *La précaution générale
tient pour tout journal copié de la VM ; elle ne décrit pas ceux-ci.*
⚠️ **Seize fichiers portent des CRLF** (les quatorze journaux d'agent et les deux
de la sonde d'idiome) : cela ne gêne aucun `grep`.

---

## 0. La recette sur VM — SIX exécutions, et ce qu'elles établissent

**Jouée le 21 août 2026**, une fois la VM rendue par le sous-projet ④ (G2).
Binaire rebâti depuis cet arbre après `cargo clean --release -p proto -p agent`
— **les DEUX crates**, la parade `-p agent` seule étant insuffisante quand
`proto` a franchi le partage réseau.

⚠️ **Une rédaction antérieure de ce §0 disait « CE DOCUMENT NE RAPPORTE AUCUNE
RECETTE SUR VM ».** Elle était exacte à sa date — la VM était tenue par un
chantier concurrent — et elle ne l'est plus. Les trois clauses qu'elle portait
sont reprises une à une ci-dessous.

⚠️ **AUCUN TAUX n'est revendiqué nulle part.** Deux exécutions par critère au
mieux ; les nombres d'exécutions sont dans chaque énoncé.

| Critère | Verdict | Exéc. | Le chiffre, **relevé** |
| --- | --- | --- | --- |
| ① les octets traversent, **à l'identique** | **TENU** | **2** | 1 572 869 octets, **25 morceaux**, SHA-256 `5d040545…` **des deux côtés**, 16 636 / 16 642 ms |
| ② `PRE_CONVERT_TO_FULL` est traversée et **acceptée** | **TENU** | **2** | `present_avant=true, taille_avant=52, ecriture=OK`, condensats égaux |
| ③ le journal est **rejoué** après une mort du pont | **TENU** | **2** | 5 dues poussées sur 6 ; la 6ᵉ reste, et c'est le bon comportement |
| ④ `PONT_ECRITURE=0` désarme | **TENU** | **1** + 1 côté produit | `dues=6` qui ne redescend **jamais** sur 59 échantillons |
| garde de casse, en conditions réelles | **TENU** | **2** | `ecriture due retenue … code=CasseAmbigue`, `Casse.txt` **intact** |
| ⑥ la vidéo ne perd pas une image | **NON MESURABLE** à ce montage | 2 | voir plus bas |

**Ce que les trois clauses de l'ancienne rédaction deviennent :**

- ✅ **DES OCTETS ONT TRAVERSÉ.** Le critère ① est **tenu** : un fichier de
  1 572 869 octets écrit *depuis la VM* est relu *depuis le navigateur* avec le
  **même condensat**, aux deux exécutions armées. S'y ajoutent, aux deux : un
  fichier **vide** (0 octet, et non « aucun morceau » — divergence 4), un
  **répertoire neuf**, et un fichier **dans** ce répertoire.
- ✅ **`PRE_CONVERT_TO_FULL` A ÉTÉ EXERCÉE**, et le legs 8 de F1 — « jamais
  exercé » — **tombe**. ⚠️ **La preuve est de CONDUITE, pas de trace** :
  `Reponse::Autoriser` **n'émet rien**, à dessein (il n'y a rien à faire). Ce qui
  l'établit est qu'un **marque-page ne peut pas devenir complet sans passer par
  cette porte**, et que l'écriture a réussi sur un fichier **projeté**
  (`present_avant=true`, `taille_avant=52`). *Un successeur qui voudrait une
  trace devra l'ajouter — c'est un legs.*
- ✅ **R-F2-1 EST LEVÉ** (tâche 14, sonde d'idiome, **2 exécutions**, session 0
  **et** session 1) : les **cinq** outils éprouvés écrivent **EN PLACE** —
  `WriteAllText`, `Add-Content`, `cmd >`, `Set-Content`, et `notepad.exe` en
  session interactive. ⚠️ **Portée exacte** : mesuré sur un répertoire NTFS
  ordinaire, **pas** dans une racine ProjFS, et il ne dit **rien** de LibreOffice
  ni de Word — dont l'idiome « écrire un temporaire, renommer, supprimer » est
  précisément l'objet de **F3**.

### Le rejeu, et ce qu'il a traversé

`reprise-1` et `reprise-2` relancent le pont sur un journal de **six** dues
laissé par le bras désarmé. Aux deux : la **première poussée part AVANT toute
écriture neuve** (`ecriture poussee … correlation=0` à `23:15:47.876`, quand la
mesure VM ne commence qu'à `23:15:56`), les **cinq** poussables sont acquittées,
et la sixième reste au journal.

🔵 **`reprise-1` a en outre traversé une HIBERNATION COMPLÈTE de la VM** — la
machine s'est éteinte d'elle-même à `23:11:11` (piège documenté depuis D1,
déclencheur toujours non identifié) entre le bras désarmé et le rejeu. **Le
journal a survécu à l'extinction de la machine, octet pour octet** (196, contenu
identique relevé avant et après). Ce n'était pas prévu au protocole ; c'est une
épreuve plus forte que celle qui l'était.

⚠️ **Le journal ne redevient PAS vide, et c'est le comportement juste.** Il
retient exactement `CASSE.TXT`, que l'écrivain refuse pour ambiguïté de casse —
une écriture refusée **doit** rester due. Le compteur final le dit en toutes
lettres : `1 fichier … n'a pas encore été recopié sur ce poste : « CASSE.TXT »
(casse-ambigue)`. *Le critère ③ du plan écrit « le journal redevient vide » : il
est tenu de ses cinq entrées poussables, et sa formulation ignorait le cas du
refus légitime.*

### 🔴 Le défaut n°1 trouvé par la recette : une fenêtre de 30 s où tout est perdu, compteur compris

**Reproduit 2 fois sur 2.** Une écriture poussée **entre l'ouverture du canal et
l'installation de l'écrivain côté navigateur** n'obtient **aucune réponse**, et
n'est rattrapée qu'au bout de `DELAI_ECRIRE`.

| | `reprise-1` | `reprise-2` |
| --- | --- | --- |
| poussée du rejeu | `23:15:47.876` | `23:31:59.278` |
| `commande expirée … correlation=0` | `23:16:18.092` (**+30,2 s**) | `23:32:29` (**+30,0 s**) |
| montage annoncé par le navigateur | `23:15:48.677` — **0,8 s APRÈS la poussée** | idem |
| issue | rejeu **réussi**, 5 acquittements | idem |

🔵 **LES OCTETS NE SONT JAMAIS PERDUS** — c'est exactement ce pour quoi le
journal existe, et il fait son travail. **C'est la LATENCE qui l'est**, et c'est
assez grave pour être un legs.

🔴 **Et le pire est l'indicateur, pas la latence** : l'annonce `Dues` part dans
la **même** fenêtre, donc le compteur du navigateur affiche **`dues: 0`** pendant
que **six** écritures attendent. Aux deux rejeux, `compteur AVANT` vaut
`{"dues":0,"vues":0}` alors que le journal en portait six. **L'indicateur qui
existe pour dénoncer la perte est MUET pendant trente secondes.**

### ⚠️ Le défaut n°2, d'instrument : un `JSON.parse` qui tue une recette

`evalBorne` rend un **objet** (`{__timeout}` / `{__erreur}`), jamais une chaîne.
Un `JSON.parse` posé dessus reçoit « [object Object] » et **lève**. C'est ce qui
a tué le pilote de `reprise-1` à son 51ᵉ échantillon — **alors que le produit,
lui, poussait correctement, comme le journal d'agent le montre**. Corrigé :
`lireCompteur` **CONSERVE** l'échantillon illisible plutôt que de le sauter,
*un trou silencieux dans une série se lisant comme une série continue*.

### Ce qui est déclaré plutôt que tu

- **`desarme-2` est ABANDONNÉE côté pilote** : la page-shell a cessé de répondre
  aux `eval` après `23:21:31`, et les 90 échantillons se sont mis à expirer un
  par un. Le pilote a été arrêté. **Le côté PRODUIT est complet et versé** — 1
  `DESARMEE`, 0 poussée, 0 acquittement, et un journal **identique octet pour
  octet** (196) à celui de `desarme-1`. **Cause non établie.**
  `pilote-desarme-2.json` n'existe donc pas.
- **Le premier lancement de `reprise-1` a échoué AVANT de toucher la VM**
  (`/media/vm/dev/run-agent.ps1: Aucun fichier`) : la VM s'était hibernée seule.
  Elle a été redémarrée, les **deux** conditions attendues (port 5985, **puis**
  un **accès réel** à `/media/vm/dev` — la table de montage CIFS survit à la VM
  éteinte), et `Get-Process agent` relevé **à zéro** avant de reprendre.
- **Le critère ⑥ n'est pas mesurable à ce montage** : `window.__pc` est absent et
  l'unique fenêtre est `endormie=true images=0` (piège D5 du Chrome sans
  interface, jamais levé par aucun sous-bloc). **Ce qui EST établi** : **0**
  `clôture de session amorcée` sur toute la campagne d'écriture, poussée de
  16,6 s comprise.
- **Aucune exécution ne s'est chevauchée**, et le contrôle a été fait **avant
  chaque lancement** — F1 en a perdu une pour l'avoir omis. ⚠️ Le premier
  contrôle écrit, `pgrep -f jouer-f2.sh`, **matchait le shell qui le lançait**
  (piège maison, payé une fois de plus) : il porte désormais sur le **pilote**.

## 1. Ce que F2 livre

| Étage | Fichier | Nature |
| --- | --- | --- |
| le protocole | `proto/src/fichiers.rs` (219), `fichiers/entetes.rs` (172) + jumeaux TS | **PUR** — 4 types, 3 en-têtes, 3 codes d'échec, **9 formes épinglées** par `proto/fichiers-vectors.json`, lu des DEUX côtés |
| la décision | `agent/src/pont/notifications.rs` (227) | **PUR, aucun `cfg`**, 12 tests d'hôte — le masque passe de **cinq à sept** bits, et `decider` prend un **ÉTAT** |
| le journal de reprise | `agent/src/pont/journal.rs` (194) + tests (167) | **PUR** — ajout seul, chemins en JSON, dernière ligne tronquée tolérée |
| la file | `agent/src/pont/ecriture.rs` (207) + tests (178) | **PUR** — une poussée en vol, un rejeu jamais perdu, FIFO |
| le fil d'écriture | `agent/src/pont/ecriture/fil.rs` (438) + `fil/disque.rs` (103) + tests (398) | **PUR**, éprouvé **sur un répertoire temporaire RÉEL** de l'hôte |
| la table | `agent/src/pont/table.rs` (238) | **PUR** — `command_id: Option<i32>`, `DELAI_ECRIRE` |
| le rappel | `agent/src/pont/projfs/rappels/notification.rs` (121) | `#[cfg(windows)]` — **il pousse et rend la main**, aucune E/S |
| l'écrivain | `client/src/fichiers/ecriture.ts` (265) + tests (326) | **PUR** — la **garde de casse**, un flux par chemin |
| le protocole client | `client/src/fichiers/protocole.ts` (251) | **PUR** — la **troisième famille** de messages |
| le compteur | `client/src/shell.ts` (252), `shell-page.ts` (270), `shell.html` | `data-dues` **et** `data-vues`, `beforeunload` |

**Extractions jouées** : `projfs/rappels/{listage,notification}.rs` **AVANT**
toute addition (`rappels.rs` 488 → 320, marge 12 → 180), puis
`ecriture/fil/disque.rs` à 494 pour une porte de 500. **Le plafond n'a été
franchi ni une fois, et aucune compression n'a été employée.**

---

## 2. Ce qui est mesuré, et en combien d'exécutions

⚠️ **Ce sont des contrôles DÉTERMINISTES.** « Combien de fois sur combien » ne se
pose pas ici, et n'est pas emprunté à une campagne qui, elle, le poserait.

| Contrôle | Référence d'entrée (relevée, non recopiée) | Après F2 |
| --- | --- | --- |
| `cargo test -p agent` | **735** | **778** |
| `cargo test -p proto` | **91** | **93** |
| `cargo check --target x86_64-pc-windows-gnu` | 0 erreur, **18** avertissements | 0 erreur, **19** |
| `cd client && npx vitest run` | **304** / 33 fichiers | **337** / 34 |
| `cd client && npx vitest run --dir ../proto` | **154** / 6 | **176** / 6 |
| `cd client && npx tsc --noEmit` | 0 | **0** |
| fichiers > 500 lignes | 5 | **2** — les deux de la dette gelée |

⚠️ **Les valeurs d'entrée du plan (614 / 223 / 111 / 16) sont celles de F1 et
ÉTAIENT toutes fausses** : l'arbre est partagé, et les sous-projets ④ et ⑤ y ont
commité depuis. C'est la divergence 10 du plan, confirmée par la mesure.

⚠️ **Les trois fichiers de plus de 500 lignes que le relevé d'entrée montrait
n'étaient PAS de F2** — `proto/src/plateforme.rs`, `proto/ts/plateforme.ts`,
`agent/src/plateforme.rs`, tous du sous-bloc **G2**, qui les a extraits pendant
cette branche. **Le tableau de dette est revenu à deux lignes sans que F2 y soit
pour rien**, et le dire évite de s'en attribuer le mérite.

⚠️ **Le 19ᵉ avertissement est de F2, et il est délibéré** : `en_vol`, `attend`,
`oublier` et `en_attente` de `pont::ecriture::File` n'ont pas d'appelant de
production. **Ce sont les trois points que le plan de F3 nomme** pour ses deux
règles d'entrelacement. **Tous les 19 restent de famille `dead_code`.**
✅ **Et les deux que le plan annonçait DISPARUS ont disparu** : `DisquePlein` et
`DejaPresent` ne figurent plus dans « variants … are never constructed ».

### Les états ROUGES joués

**Onze**, toutes au même harnais et versées :
`f2-tache1-rouges.txt`, `f2-tache2-pont-ecriture-transmise.txt`,
`f2-tache3-garde-abi.txt`, `f2-client-rouges.txt`.

Le harnais **refuse de compter une rouge dont le diff est vide**, mute **par
NUMÉRO DE LIGNE** — une substitution de chaîne frappe le **commentaire** avant le
code, dans un dépôt qui commente ses invariants (piège P4) — et restaure par une
**copie**.

🔴 **La plus importante : retirer la garde de casse.** Le faux système de
fichiers, **insensible à la casse par construction** comme un vrai poste local,
rend alors :

```
AssertionError: expected { nom: 'Casse.txt', contenu: [ 9 ] } to deeply equal { nom: 'Casse.txt', …(1) }
```

**`Casse.txt` est écrasé par la charge poussée vers `CASSE.TXT`.** C'est la perte
de données que F1 avait mesurée en lecture, transposée à l'écriture.

---

## 3. 🔴 Ce que F2 N'ÉTABLIT PAS

- **Aucun taux, nulle part.** Deux exécutions par critère au mieux (§0).
- **Rien du renommage ni de la suppression**, donc **rien de l'idiome
  d'enregistrement atomique**. Le critère 2 de la spec §8 F2 est **déplacé en
  F3**, comme le plan le décide.
- **Rien de la latence.** F2 journalise `duree_ms` par écriture ; il n'en tire
  aucune loi. La seule mesure de débit du dépôt est incohérente d'un **facteur
  ~120** sans explication (F1 §11).
- **Aucune constante calibrée** : `DELAI_ECRIRE` (30 s),
  `TAILLE_ECRITURE_SIGNALEE` (64 Mio), `TAILLE_JOURNAL_COMPACTAGE` (256 Kio),
  plus les quatre de F1 et les huit du chantier D.
- **Le défaut de casse n'est corrigé qu'en ÉCRITURE.** `casse.txt` continuera de
  rendre le contenu de `Casse.txt` en lecture.
- 🔴 **La garde de casse ne voit pas la NORMALISATION UNICODE.** macOS stocke en
  NFD, Windows en NFC : elle créerait un **doublon** au lieu d'écraser — moins
  grave que la perte, mais faux. **Non traité, déclaré ; c'est le canonicaliseur
  de F3.**
- **Aucun contrôle de flux** : un morceau en vol à la fois.
- **`showDirectoryPicker()` n'est toujours jamais appelé**, ni
  `queryPermission`/`requestPermission`, ni **le mode `readwrite`** que F2 pose :
  l'instrument de recette est OPFS, qui n'a aucun modèle de permission.
- ✅ ~~Le journal n'est jamais relu APRÈS un arrêt brutal réel~~ — **il l'est
  depuis la recette** : deux rejeux sur un pont tué, dont un **après une
  hibernation complète de la machine** (§0). ⚠️ **Ce qui reste vrai** : aucun
  arrêt *pendant* une poussée en vol n'a été provoqué — le pont a toujours été
  tué entre deux écritures, jamais au milieu d'un morceau.
- **`TAILLE_MAX_FICHIER` de la spec §3.5.2 n'est pas implémenté**, et la raison
  est de fond : un refus de taille serait **invisible** au write-back, et
  `ERROR_DISK_FULL` **n'atteindrait personne**. F2 pose un plafond de **journal**.
- **Aucun test d'hôte ne couvre `pont/projfs/`** ; la compilation croisée vérifie
  types, emprunts et durées de vie — **jamais le comportement**.
- **R7 reste OUVERT** : F2 **n'ajoute aucune entrée importée** (`resolution::NOMBRE`
  reste **13**), donc il n'aggrave rien — et ne réduit rien non plus.
- **L'énumération vide intermittente de F1 (legs 3) et les lectures qui calent
  sans expirer (legs 4)** ne sont ni expliquées ni refermées.
- **Le chemin d'extinction propre du superviseur** n'a toujours jamais été
  exercé, depuis D1.

---

## 4. Les divergences relevées AVEC LE PLAN, et pourquoi

**Sept**, toutes écrites dans le code à l'endroit où elles vivent.

| # | Ce que le plan prescrit | Ce qui est livré, et pourquoi |
| --- | --- | --- |
| 1 | `CasseAmbigue` est « la **NEUVIÈME** variante de `CodeEchec` » | **La DIXIÈME.** La tâche 1 en ajoute déjà deux aux sept de F1 ; le compte du plan s'arrête avant sa propre tâche 9 |
| 2 | `PRE_CONVERT_TO_FULL` autorisée rend `AccepterSansAttendre` | 🔴 **`Reponse::Autoriser`.** La prescription faisait retomber un bit **DEMANDÉ par le masque** dans le bras fourre-tout, donc **échouer `chaque_bit_du_masque_a_une_decision_nommee`** — et le remède évident, l'exclure du balayage, aurait **VIDÉ le garde**. *Un plan n'immunise pas contre le contrôle vacueux : il en est une source* |
| 3 | `Reponse::AccepterEnSignalant` est conservée | **Retirée** : plus aucun producteur une fois `NEW_FILE_CREATED` passé à `Pousser` |
| 4 | un fichier de taille nulle « produit une **création** et zéro morceau » | 🔴 **Un morceau VIDE.** Une création n'a **aucun effet** sur un fichier local existant : un fichier **tronqué à zéro** sur la VM garderait son ancien contenu sur le poste local — **corruption silencieuse** |
| 5 | `PONT_ECRITURE` : « `=0` désarme », **et** la forme `matches!(…, Ok(v) if v != "0")` | 🔴 **Le plan se contredit en une phrase.** La forme prescrite rend `false` **en l'absence** de la variable : prise à la lettre, elle aurait livré un pont **MUET PAR DÉFAUT**. Forme retenue : celle de `PLEIN_ECRAN` |
| 6 | `fil::tourner` prend **deux** `Receiver` | **Un seul canal.** Deux récepteurs sur un fil bloquant imposeraient un sondage alterné, donc un test dépendant du minutage |
| 7 | `TypeMismatchError` → `deja-present` | **Impossible** : il est **déjà** classé en absence, et c'est ce qui permet à `attributs` de retenter en fichier après avoir échoué en répertoire |

---

## 5. Un défaut LATENT de F1, trouvé et corrigé

**L'exemplaire local d'`Arc<Etat>` n'était jamais relâché dans `pont::executer`.**
Les deux `Sender` — celui du transport et, depuis F2, celui du fil d'écriture —
vivent **dans `Etat`** ; un récepteur ne se déconnecte que lorsque le **dernier**
exemplaire de son `Sender` est parti.

Le commentaire de F1 l'énonçait **à l'envers** : « le transport s'arrête quand
`Etat` — **donc `virtualisation`** — est relâché ». `virtualisation` n'en détient
qu'un exemplaire sur deux. **Sans conséquence visible chez F1**, `transport::tourner`
pouvant rendre pour une autre raison ; **F2 l'aurait rendu BLOQUANT**, en
ajoutant un second `join` sur un fil qui n'a, lui, aucune autre raison de sortir.

---

## 6. La revue transverse — sept affirmations devenues fausses

Barème du dépôt : D7 **5**, D8 **3**, D9 **6**, D10 **douze**, D11 **sept**,
P1 **huit**, P2 **dix**, S1 **cinq**, E **neuf**, P3 **douze**, S2 **douze**,
F1 **onze**, P4 **huit**, S3 **treize**, G1 **huit**, S4 **vingt-sept**,
presse-papier **dix-sept**, P5 **neuf**. **Sept ici**, détaillées dans le commit
`revue(f2)` et corrigées **à leur place**.

La plus lourde : `erreurs.rs` — « **F1 vit tout entier dans cet état** : le
lecteur est en lecture seule ». La variante `ProtegeEnEcriture` existe toujours,
mais elle **signifie autre chose** depuis F2, et c'est écrit.

---

## 7. Les pièges neufs

- 🔴 **`cp -p` PRÉSERVE LA DATE, ET CARGO GARDE ALORS L'ARTEFACT DE LA VERSION
  MUTÉE.** Payé sur place : après une rouge sur `proto/src/fichiers.rs`, un test
  correct échouait sur `charge.len() > TAILLE_TRAME_MAX` avec `recu: 65536,
  max: 65536` — **une contradiction qu'aucune lecture de la source ne peut
  expliquer**. Le pire cas est l'inverse : une suite **VERTE** exécutant encore
  le code muté. Restaurer par `cp` **sans** `-p`.
- 🔴 **`git checkout -- <fichier>` RESTAURE HEAD, PAS L'ÉTAT D'AVANT LA
  MUTATION.** Il a effacé du travail non commité **deux fois** dans cette
  branche. Et son corollaire : un garde « le diff est-il vide ? » fondé sur
  `git diff` est **VACUEUX** sur tout fichier qui porte déjà du travail en cours.
- 🔴 **`expect` INTERROMPT UN TEST À SA PREMIÈRE ASSERTION EN ÉCHEC** — leçon
  ①A-bis de P2, repayée ici sur le rouge le plus important de F2. Les trois
  assertions de la garde de casse vivaient dans **un seul** test ; « `Casse.txt`
  est écrasé », celle qui porte la **perte de données**, n'était éprouvée par
  **rien**. Séparées, la rouge la montre.
- ⚠️ **`expect(x).toBe(y, 'message')` compile sous Vitest et IGNORE le second
  argument** — mais **`tsc --noEmit` le refuse**. Trois assertions de F2 en
  portaient un : c'est exactement la raison d'être de l'étape `typecheck` à côté
  de `vitest`, qui repose sur esbuild et ne vérifie aucun type.
- ⚠️ **Un `DIFFÈRE` qu'on ne lit pas ligne à ligne se conclut dans les deux
  sens.** Le contrôle « la transposition est-elle verbatim ? » a rendu `DIFFÈRE`
  sur les deux extractions : **le défaut était dans l'instrument** (un `sed '$d'`
  qui mangeait l'accolade fermante), pas dans la transposition.
- ⚠️ **Une mutation d'ABI doit être ISOLÉE.** Muter `*const GUID` en `*const u8`
  faisait aussi échouer un appel **interne** ; le contrôle en deux temps rendait
  alors deux erreurs au lieu de zéro, et se serait lu comme un échec du garde.
  Un **paramètre en trop**, que le corps n'emploie pas, l'isole.

---

## 8. Ce que F2 lègue

**À F3, et son plan les attend déjà :**

1. ✅ **La file d'écritures dues est livrée et INDEXÉE PAR CHEMIN**, avec les
   trois points que le §0.3 de F3 exige : `File::attend`, `File::oublier`,
   `File::en_vol`. Ils sont **`dead_code` aujourd'hui**, et c'est déclaré.
2. ✅ **Les numéros de verbe 7 et 8 sont RÉSERVÉS**, et le code le dit.
3. ✅ **L'extraction de `rappels.rs` est faite, sous les noms que F3 nomme** —
   `listage.rs` et `notification.rs`. **Sa tâche 3 n'a rien à faire**, et elle
   doit le déclarer avec la sortie de `ls`.
4. ⛔ **La casse en LECTURE**, et le **canonicaliseur** — qui devra aussi porter
   la normalisation Unicode (§3).
5. ⛔ **Le contrôle de flux** (`bufferedAmount` / `SEUIL_TAMPON`).
6. ⛔ **La table complète des douze `HRESULT`**, et une variante propre pour
   `casse-ambigue`, qui partage aujourd'hui `Inattendue` avec `TropGrand`.

**À F4 :**

7. ⛔ **Le coût de la garde de casse** : une énumération du répertoire parent
   **par écriture**.
8. ⛔ **La durée de la fenêtre de perte.** F2 l'instrumente (`duree_ms`,
   `octets`) ; il n'en tire aucune loi.

**À F5 :**

9. ⛔ **La reprise à travers un redémarrage de VM**, et le retour avec un
   **autre** répertoire (spec §6.4 cas 2).
10. ⛔ **`Rafraichir`**, sans lequel aucun cache d'énumération n'est possible.
11. ⛔ **La politique d'éviction** : chaque fichier écrit reste sur le disque de
    la VM.

**Sans destinataire :**

12. ✅ ~~LA RECETTE DE F2 ELLE-MÊME~~ — **JOUÉE le 21 août 2026**, six
    exécutions, quatre critères tenus, R-F2-1 levé (§0).
13. ✅ ~~`PRE_CONVERT_TO_FULL` n'a jamais été exercé~~ — **EXERCÉ et ACCEPTÉ**,
    2 exécutions. ⚠️ **La preuve est de conduite, pas de trace** : la voie
    `Autoriser` n'émet rien. **Legs neuf** : lui donner un `debug!`, sans quoi
    F3 — qui refusera renommage et suppression à cette même porte — n'aura
    aucun moyen d'observer ses propres décisions.
14. ⛔ **L'inférence du §3.4 du plan — `createWritable()` est atomique par
    fichier — n'est pas mesurée.** C'est le critère ⑤, non joué.
15. ⛔ **L'inférence du fil PUR — lire un fichier hydraté ne ré-entre pas dans
    nos rappels — n'est pas mesurée** non plus. C'est le critère ④.

**Legs NEUFS, nés de la recette :**

16. 🔴 **LA FENÊTRE DE TRENTE SECONDES** (§0) : ce qui est poussé entre
    l'ouverture du canal et l'installation de l'écrivain côté navigateur est
    perdu, et le **compteur affiche `dues: 0` pendant que six écritures
    attendent**. Les octets, eux, arrivent. Remède nommé : que le pont
    n'ouvre son canal d'écriture qu'**après** un acquittement de l'écrivain,
    plutôt qu'à l'ouverture du canal de données.
17. ⛔ **Le critère ⑥ n'est pas mesurable au montage de recette** : `window.__pc`
    absent, fenêtre `endormie=true`. Limite héritée de **D5**, qu'aucun
    sous-bloc n'a levée.
18. ⛔ **La page-shell a cessé de répondre aux `eval` pendant `desarme-2`**,
    cause non établie. C'est la seule exécution abandonnée de la campagne.
19. ⛔ **`showDirectoryPicker()` n'est toujours jamais appelé**, ni le mode
    `readwrite` que F2 pose : l'instrument est OPFS, qui n'a **aucun modèle de
    permission**. La voie qui le lèverait — `Xvfb` + `xdotool` — a son
    consentement donné **en D8** et jamais suivi d'effet.
