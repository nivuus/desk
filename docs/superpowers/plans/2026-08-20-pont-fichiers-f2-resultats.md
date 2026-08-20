# Sous-projet ③ Pont fichiers — sous-bloc F2 : résultats

**21 août 2026.** Document **permanent**, versé dans git — D9 a perdu **six**
constats de revue parce que sa preuve vivait dans un espace gitignoré.

Plan : `docs/superpowers/plans/2026-08-20-pont-fichiers-f2.md`.
Conception : `docs/superpowers/specs/2026-08-19-pont-fichiers-design.md`.
Journaux : `docs/superpowers/plans/journaux-pont-fichiers-f2/` — **UNE seule
famille de lecture** : sorties `npm`/`cargo`/`node` sur l'**hôte**, aucune
séquence ANSI, aucun octet de contrôle. Elles se `grep`ent à plat.

---

## 0. 🔴 CE DOCUMENT NE RAPPORTE AUCUNE RECETTE SUR VM

**Les tâches 14 (sonde d'idiome) et 15 (recette) N'ONT PAS ÉTÉ JOUÉES.** La VM
Windows était tenue par un chantier concurrent (**sous-projet ④, sous-bloc G2**)
au moment de cette branche : elle tournait, son `agent.log` avait été écrit
**vingt secondes** avant le relevé, et la recette de F2 exige de **reconstruire
`agent.exe`**, ce qui aurait remplacé le binaire que le voisin mesurait.

**Conséquence, écrite plutôt que déduite :**

- 🔴 **AUCUN OCTET N'A TRAVERSÉ.** Tout ce que ce document rapporte est éprouvé
  par des tests d'hôte et par la compilation croisée. **Le critère ① de F2 — un
  fichier enregistré depuis la VM, relu depuis le navigateur, condensat contre
  condensat — n'est ni tenu ni réfuté : il n'a pas été essayé.**
- 🔴 **`PRE_CONVERT_TO_FULL` N'A TOUJOURS JAMAIS ÉTÉ EXERCÉ**, et c'est le legs
  8 de F1, reconduit **entier**. F2 en fait l'unique porte de refus d'une
  écriture ; **rien n'établit qu'elle existe pour de bon.**
- 🔴 **LE RISQUE R-F2-1 EST INTACT.** Si aucun outil d'écriture de cette VM
  n'écrit **en place**, F2 est **codé et non recevable** — le critère ① partirait
  en F3 avec le critère 2. La sonde qui tranche est **écrite et versée**
  (`instrument/sonde-idiome.ps1`), **jamais exécutée**, et son en-tête le dit.

*Ce paragraphe est le premier du document à dessein : un lecteur qui n'en lit
qu'un doit repartir en sachant que F2 est **livré, pas recetté**.*

---

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

- **Aucun taux, nulle part** — et surtout : **aucune exécution sur la VM** (§0).
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
- **Le journal n'est jamais relu APRÈS un arrêt brutal réel** : la reprise est
  éprouvée en posant un journal à la main, jamais en tuant un pont.
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

12. 🔴 **LA RECETTE DE F2 ELLE-MÊME** (§0) : les tâches 14 et 15, la sonde
    d'idiome et les six critères. **L'instrument de la sonde est versé et prêt.**
13. ⛔ **`PRE_CONVERT_TO_FULL` n'a jamais été exercé** — legs 8 de F1, reconduit.
14. ⛔ **L'inférence du §3.4 du plan — `createWritable()` est atomique par
    fichier — n'est pas mesurée.** C'est le critère ⑤, non joué.
15. ⛔ **L'inférence du fil PUR — lire un fichier hydraté ne ré-entre pas dans
    nos rappels — n'est pas mesurée** non plus. C'est le critère ④.
