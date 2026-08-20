# Sous-projet ③ Pont fichiers — sous-blocs F0 et F1 : résultats

**20 août 2026.** Plan : `docs/superpowers/plans/2026-08-19-pont-fichiers-f1.md`.
Conception : `docs/superpowers/specs/2026-08-19-pont-fichiers-design.md`.
Pièces : `docs/superpowers/plans/journaux-pont-fichiers/`, dont
`LISEZ-MOI.md` porte les **deux** familles de lecture (les `agent-*.log` bruts
demandent `sed 's/\x1b\[[0-9;]*m//g'` ; leurs jumeaux `-plat` non) et la
consigne `grep -a`.

**Ce document existe parce que la preuve d'une affirmation du dépôt ne doit
jamais vivre ailleurs que dans git.** Le sous-bloc D9 a perdu six constats de
revue en les laissant dans un espace de travail gitignoré. Au sortir de la
recette, toute l'analyse de F1 vivait dans un **message de commit** (`fa14e60`)
et dans un `LISEZ-MOI` : les pièces survivaient, l'analyse n'était lisible que
par `git log`. C'est cette page qui la porte désormais.

---

## 1. Le verdict, en trois faits qui ne se simplifient dans aucun sens

> ✅ **① UN LECTEUR EXISTE, ET IL LISTE JUSTE.** Sur la VM,
> `%USERPROFILE%\Mes Fichiers` montre **exactement** l'arborescence choisie
> dans le navigateur, **aux deux niveaux** : 6 entrées sur 6, ensemble de noms
> identique à celui de l'hôte, sous-répertoire et fichier imbriqué compris, nom
> accentué avec espace compris, et `gros.bin` annoncé à **12 582 912** octets
> exactement. **Trois exécutions sur trois** parmi celles dont la mesure VM a
> abouti (`mesure-exec{1,2,5}.txt` contre `noms-origine.txt`).

> ⚠️ **② LE CRITÈRE QUI DEVAIT DÉCIDER N'EST PAS ÉTABLI — et il n'est pas
> RÉFUTÉ non plus.** Le condensat SHA-256 du fichier de 12 Mio n'a jamais pu
> être comparé : **aucune copie n'est allée à son terme dans la fenêtre de
> mesure**, aux cinq exécutions nominales. La spec §8 en fait « le seul critère
> qui ne puisse pas être satisfait par accident » ; il n'a pas été satisfait du
> tout. ⚠️ **Ne pas lire cela comme « les octets sont faux »** : sur l'état
> rouge (i), la même lecture a rendu **12 582 912 octets en 1 937 ms**
> (`rouge-i-kill-2s.txt`), et le condensat OPFS calculé côté page a été relevé
> **égal** à celui de l'hôte (`LISEZ-MOI.md`). Ce qui manque est la
> **comparaison de bout en bout**, pas une preuve du contraire.

> ✅ **③ LA VIDÉO N'A JAMAIS BRONCHÉ, ET C'EST LA PIÈCE LA PLUS FORTE DU
> CHANTIER.** Sur `exec1`, le pont est resté **calé neuf minutes** — la mesure
> VM lancée à `01:28:08` n'a rendu la main qu'à `01:37:11`, tronquée — pendant
> que la session vidéo de la même VM continuait : `framesDecoded` **monotone
> sur six échantillons**, `packetsLost` **0**, **0** `clôture de session
> amorcée`, **0** `ERROR` dans tout le journal. Idem sur `exec2` (framesDecoded
> jusqu'à **51 935**) et `exec5`. **Un pont qui cale ne coûte rien à la
> vidéo** : c'est exactement ce que la décision D2 (un processus séparé) devait
> acheter, et c'est mesuré.

**Aucun taux n'est revendiqué nulle part.** Cinq exécutions du chemin nominal,
une du Step 3, deux de l'état rouge (i), une sonde de borne de lecture.

---

## 2. F0 — le préalable, avec son rouge d'avant

**F0 est REÇU**, et son critère est falsifiable parce que le **rouge d'avant est
versé** (`f0-avant.txt`) :

| | Avant (19 août, ROUGE) | Après (19 août, VERT, réexécuté par la commande) |
| --- | --- | --- |
| `Client-ProjFS` | `State : Disabled` | `State : Enabled` |
| `ProjectedFSLib.dll` | **`False`** | `True` |
| `PrjFlt.sys` | **`False`** | `True` |
| filtre chargé | non relevé | `PrjFlt` altitude **189800** (`fltmc filters`) **et** service `Running`/`Automatic` |

**Le fait le plus réutilisable** : l'activation passe par
`Enable-WindowsOptionalFeature -Online -FeatureName Client-ProjFS -NoRestart -All`,
et elle a rendu **`RestartNeeded : False`** — aucun redémarrage n'a été
nécessaire, aucun n'a eu lieu, alors que le catalogue annonçait
`RestartRequired : Possible` **avant** activation. *Ces deux champs ne disent
pas la même chose et ne se lisent pas l'un pour l'autre.*

⚠️ **`-All` est une divergence d'avec le plan** (qui prescrivait la commande
sans lui), déclarée dans `f0-activation.txt`. Le critère porte sur l'état
obtenu, pas sur la commande.

⚠️ **`Install-WindowsFeature Projected-File-System` « échoue en silence » est
une INFÉRENCE, pas une mesure.** Ce qui est relevé (spec §2) est que
`Get-WindowsFeature | Select -Expand Name` filtré sur `Proj` ne rend **aucune
correspondance** : le nom vit au catalogue des fonctionnalités facultatives, pas
au gestionnaire de rôles. **La commande elle-même n'a jamais été lancée.**

⚠️ **La troisième condition (le filtre RÉELLEMENT chargé) n'est pas dans la
spec** : c'est le risque R1bis, ajouté par le plan. Un mini-filtre présent sur
le disque mais non chargé ferait échouer `PrjStartVirtualizing` très loin de
là, avec un `HRESULT` que personne ne rattacherait à F0.

⚠️ **Ce que F0 n'établit PAS** : aucun `PrjStartVirtualizing` n'a été appelé à
ce stade, et rien n'est dit de la persistance au redémarrage — `Automatic` la
rend plausible, pas certaine. **La provenance du rouge est déclarée** : il a été
pris par l'orchestrateur, pas par l'agent d'implémentation, qui ne pouvait plus
le reproduire ; c'est la seule pièce du sous-bloc qui ne soit pas de première
main, et `f0-avant.txt` le dit en toutes lettres.

---

## 3. L'instrument : `showDirectoryPicker()` est inutilisable ici, et c'est MESURÉ

**C'est la mesure qui décide de tout le montage de recette**
(`sonde-picker.txt`, Chrome 151.0.7922.169, hôte Linux) :

1. sans interception CDP, `showDirectoryPicker({mode:'read'})` rend
   `AbortError` et **aucun** `Page.fileChooserOpened` — il n'y a pas
   d'interface, Chrome avorte de lui-même ;
2. avec `Page.setInterceptFileChooserDialog({enabled:true})`, l'événement est
   bien émis, **et l'interception EST l'annulation** :
   `Intercepted by Page.setInterceptFileChooserDialog()` ;
3. **aucune commande CDP n'accepte un sélecteur** — `Page.handleFileChooser` et
   `Page.fileChooserAccepted` rendent tous deux `-32601 … wasn't found`, sur la
   cible PAGE où `setInterceptFileChooserDialog` répond pourtant `{}` : ce
   n'est donc pas un problème de cible ;
4. aucune interface graphique sur l'hôte — `DISPLAY` vide, `/tmp/.X11-unix`
   absent, `Xvfb` et `xdotool` introuvables.

**Parade retenue : OPFS.** `navigator.storage.getDirectory()` rend une **vraie**
`FileSystemDirectoryHandle`, sans sélecteur et sans geste, et
`globalThis.showDirectoryPicker` est surchargé pour la rendre. **Le point
d'injection est le plus bas possible** — `choisirDossier()` lit
`globalThis.showDirectoryPicker` *à l'appel* — donc tout le code produit tourne
**inchangé** derrière : `choisirDossier`, l'affectation
`const racine: Racine = poignee` qui est le contrôle de compatibilité
structurelle, `creerAdaptateur`, `creerServeur`, `connecterCanalFichiers`, et le
gestionnaire de clic de `shell-page.ts`.

⚠️ **CE QUE LA RECETTE N'EXERCE DONC PAS** : l'appel `showDirectoryPicker()`
lui-même, le modèle de permission des répertoires choisis par l'utilisateur
(`queryPermission`/`requestPermission`, que `choisirDossier` n'appelle pas), et
l'activation utilisateur transitoire.

---

## 4. Les quatre critères de F1, avec leur nombre d'exécutions

| # | Critère (spec §8) | Verdict | Exéc. |
| --- | --- | --- | --- |
| 1 | l'arborescence, **aux deux niveaux** | **TENU** — 6/6, ensembles de noms identiques | **3** (exec1, 2, 5) |
| 2 | même condensat SHA-256 pour le fichier > 10 Mio | **NON ÉTABLI** — aucune copie menée à terme. **Pas réfuté** | **0** |
| 3 | aucun dépassement des budgets du §5.3 | **NON DÉMONTRABLE, et le contrôle est VACUEUX** | 5 |
| 4 | la vidéo tourne et ne perd pas une image | **TENU** | **3** nominales + **2** rouges + **1** Step 3 |

**Critère 1, en détail.** Ensemble relevé côté VM, identique aux trois
exécutions : `sous-dossier` (répertoire), `casse.txt` (42), `Casse.txt` (42),
`gros.bin` (**12 582 912**), `éphémère été.txt` (25), `sous-dossier\imbrique.txt`
(43) — `noms_total=6`. Côté hôte, `noms-origine.txt` porte exactement les six
mêmes noms. **La comparaison est faite par ENSEMBLE DE NOMS, jamais par
cardinal** — piège maison : un compteur ne suffit pas quand un tiers agit sur le
système. Le nom accentué **et** l'espace passent par UTF-16 sans dommage ; les
tailles remontent justes, y compris celle du gros fichier, ce qui prouve que le
verbe `Attributs` fonctionne indépendamment de `Lire`.

**Critère 3, et pourquoi il ne vaut rien tel qu'il est écrit.** Le `grep`
prescrit (`ERROR_SEM_TIMEOUT\|delai depasse`) rend **0** partout — **et aucune
de ces deux chaînes n'est jamais émise par le produit**. Le témoin réel est
`commande expirée` (`agent/src/pont/service.rs`), et il vaut **0 aux cinq
exécutions nominales**. Ce zéro-là n'établit rien non plus, puisque **aucune
lecture de `gros.bin` n'y est allée à son terme** : on ne peut pas dépasser un
budget sur une opération qu'on n'a pas menée. *Un contrôle qui ne peut pas
échouer, pour la troisième fois dans ce dépôt.*

⚠️ **La seule occurrence de `commande expirée` de tout le corpus** est dans
`agent-dbg-plat.log`, **une** ligne à `01:07:57`, et elle tombe **après** que le
pilote a fermé le navigateur. *`borne-lecture.txt` écrit « à 01:07:57 **et
suivantes** » : le pluriel dépasse le relevé, il n'y en a qu'une.*

**Critère 4, en détail.** `packetsLost` vaut **0** aux six échantillons de
chacune des trois exécutions nominales, des deux rouges (i) et du Step 3 ;
`framesDecoded` croît de façon monotone partout ; `clôture de session amorcée`
et ` ERROR ` valent **0** dans **tous** les journaux d'agent versés.

⚠️ **« Vidéo intacte » ne s'étend PAS à l'application, et la recette le montre
plutôt qu'elle ne le contourne** : sur `exec1`, l'Explorateur et le script de
mesure sont restés bloqués neuf minutes pendant que les images continuaient
d'arriver. *Les images arrivent, le contenu ne bouge plus* (spec §5.2).
**`framesDecoded` ne dit rien de cela.**

---

## 5. Les trois rouges : UN SEUL a été provoqué

| Rouge (spec §8) | Provoqué ? | Ce qui est relevé |
| --- | --- | --- |
| (i) tuer le pont en pleine lecture | **OUI**, 2 exécutions | voir ci-dessous |
| (ii) fermer la page-shell pendant une copie | **NON** | — |
| (iii) démarrer le pont avant le choix du répertoire | **NON**, et **l'énoncé décrit un état que F1 ne peut pas atteindre par ce chemin** | voir ci-dessous |

**Rouge (i)** — `rouge-i-kill-2s.txt`, PID **relevé dans le journal d'agent** et
non deviné : 4 processus `agent` avant, le pont tué (`pont_pid_du_journal=10728`),
**4 après** avec un PID neuf (`16604`) — *le superviseur a relancé le pont*, et
`delai_apres_mort_ms=44`. La vidéo n'a pas bronché.

⚠️ **CE ROUGE EST FAIBLE, ET IL FAUT LE DIRE** : la lecture s'est terminée en
**1 937 ms**, donc **avant** la mise à mort programmée à 2 s. Ce qui est établi
est que le superviseur relance le pont ; ce qui ne l'est pas est que
« l'Explorateur rende une erreur d'E/S en moins de 5 s », puisqu'aucune lecture
n'était réellement en vol au moment du coup.

✅ **Un défaut d'instrument a été trouvé, versé et corrigé** : la première
version choisissait le PID « le plus jeune », heuristique **fausse** — l'ordre
de lancement est superviseur, capteur, pont, **puis** l'enfant de chaque
fenêtre. **C'est l'ENFANT qui avait été tué**
(`rouge-i-pid-mal-choisi.txt`, conservé parce que c'est le défaut, pas le
résultat). Le relevé fautif portait `delai_apres_mort_ms=11038` et un échec de
lecture : il aurait été lu comme un succès du rouge.

**Rouge (iii)** — l'énoncé de la spec (« la racine existe et toute lecture rend
`ERROR_IO_DEVICE` ») suppose que le pont monte sa racine à son démarrage. **Il
ne le fait pas** : `agent/src/pont.rs::executer` n'appelle
`Virtualisation::demarrer` qu'**après** avoir accepté l'offre SDP de la
page-shell. Relevés concordants : `racine presente apres nettoyage : False`
avant chaque exécution, et le Step 3 (DLL renommée) ne fait apparaître **aucune**
racine. Le seul chemin qui produirait cet état est une racine **survivante**
d'une exécution tuée brutalement — cas que `pont.rs` nomme déjà et que F1 ne
referme pas.

---

## 6. Le contrôle qui justifie la décision D1 tout entière — il PASSE

**C'est le contrôle qui prouve que la résolution à l'exécution
(`LoadLibraryW` + `GetProcAddress`, décision D1) fait ce pour quoi elle a été
choisie.** Sans lui, D1 est une intention.

`ProjectedFSLib.dll` renommée sur la VM (`RENAMED OK`, `dll presente : False`),
une exécution complète, puis restauration vérifiée
(`orchestrateur-step3.txt`, `agent-sans-projfs*.log`) :

| Relevé | Valeur |
| --- | --- |
| `capteur lancé` | **1** |
| `chargement de ProjectedFSLib.dll` | **366** |
| `Le module spécifié est introuvable. (0x8007007E)` | **366** |
| `superviseur::lanceur::pont: pont fichiers lancé` | **367** |
| `surveillance_pont: … lancé ou relancé` | **1** *(les suivantes sont silencieuses par conception)* |
| ` ERROR ` dans tout le journal | **0** |
| ` WARN ` | 34 |
| `framesDecoded` à t+90 s | **15 122**, monotone, `packetsLost` **0** |
| `#etat-fichiers` côté page | « n'a pas pu être monté : l'agent n'a pas répondu » |

**Le superviseur, le capteur et l'enfant démarrent normalement, la session vidéo
s'établit, et SEUL le pont échoue.** Et la panne est un **`warn!`, jamais un
`error!`** — c'est ce que dit le `0 ERROR` sur 3 064 lignes. **D1 est validée par
la mesure, pas par le raisonnement.**

⚠️ **La chaîne à grepper n'est PAS celle du plan** : `ERROR_MOD_NOT_FOUND`
n'apparaît nulle part dans `agent/src/` — elle ne vit que dans la spec et le
plan. **Grepper `ProjectedFSLib`.** Un opérateur suivant le plan aurait lu `0` et
conclu que le contrôle avait échoué.

---

## 7. Les TROIS défauts que seule la recette pouvait trouver

### 7.1 🔴 La casse rend le MAUVAIS FICHIER, en silence — pire que le legs annoncé

Le legs de la tâche 3 promettait `Introuvable`. **La mesure dit autre chose, et
c'est une autre nature de risque.** Relevé **identique aux trois exécutions
versées** (`mesure-exec{1,2,5}.txt`), avec `Casse.txt` sur le poste local :

```
casse|Casse.txt|OK|MAJUSCULE - ce fichier sappelle Casse.txt
casse|casse.txt|OK|MAJUSCULE - ce fichier sappelle Casse.txt
casse|CASSE.TXT|OK|MAJUSCULE - ce fichier sappelle Casse.txt
casse|GROS.BIN|ECHEC|… Le fichier '…\GROS.BIN' est introuvable.
```

**L'application reçoit le contenu d'un fichier qu'elle n'a pas demandé, sans
erreur, sans moyen de le savoir. Et le comportement n'est pas cohérent avec
lui-même** : dans la même exécution, `GROS.BIN` rend bien « introuvable ».

⚠️ **Le mécanisme est une HYPOTHÈSE cohérente avec les pièces, pas une mesure.**
L'écart suit exactement l'**hydratation** : la trace
`racine hydratee … octets=42 entrees=1` dit qu'une seule entrée de 42 octets —
`Casse.txt`, lu par la sonde juste avant — vivait localement, et NTFS,
insensible à la casse, la retrouve alors **sans jamais atteindre le pont** ;
`gros.bin`, jamais hydraté, retombe sur le rappel, qui demande au navigateur une
casse qu'il ne connaît pas. **Rien ne l'établit** : il faudrait une exécution où
l'ordre d'hydratation est renversé.

**Observé, non corrigé.** Le remède (une table de correspondance alimentée par
l'énumération) reste du ressort de F3.

### 7.2 🔴 Le refus d'écriture ne refuse pas — mais c'est la SPEC qui promettait trop

Une création locale devait rendre `ERROR_WRITE_PROTECT`. Elle rend :

```
creation|CREEE (DIVERGENCE : attendu ERROR_WRITE_PROTECT)
creation_presente=True
```

**2 exécutions versées sur 2** qui atteignent cette phase (`mesure-exec2.txt`,
`mesure-exec5.txt` ; `mesure-exec1.txt` s'est arrêtée avant).

⚠️ **CE N'EST PAS UNE DIVERGENCE DU CODE, et c'est le point à retenir** :
`agent/src/pont/notifications.rs` **documentait déjà** que `NEW_FILE_CREATED`
est une notification **POST, donc irrefusable**, et que seuls les trois chemins
`PRE_` (convert-to-full, rename, delete) sont refusés. Un fichier neuf n'en
traverse **aucun**. Le produit a d'ailleurs émis exactement le `warn!` écrit
pour ce cas :

```
WARN agent::pont::projfs::rappels: fichier créé dans la racine du pont : il vit
sur la VM et ne sera JAMAIS poussé vers le poste local (F1 est en lecture seule ;
la notification NEW_FILE_CREATED est une POST, elle ne se refuse pas)
chemin="creation-interdite.txt"
```

**Ce sont la spec §8 (« toute tentative d'écriture rend `ERROR_WRITE_PROTECT` »)
et le plan (« Attendu : refusé ») qui sont faux.** La formulation juste : *écrire
dans un fichier PROJETÉ rend `ERROR_WRITE_PROTECT` ; un fichier créé de toutes
pièces vit sur la VM et n'est jamais poussé.* Les deux documents sont annotés.

⚠️ **Et le levier qui porte la moitié VRAIE de la phrase —
`PRE_CONVERT_TO_FULL`, l'écriture d'un fichier EXISTANT — n'a JAMAIS été
exercé.** Aucune pièce ne le confirme ni ne l'infirme.

### 7.3 🔴 Énumération vide par intermittence — et l'une des deux occurrences a un CONFONDEUR

Sur `exec3`, la mesure VM n'a rendu que **3 lignes** (`racine=`,
`racine_presente=True`, `=== NOMS DEBUT ===`) : l'énumération n'a rien produit
dans la fenêtre de 540 s, sur une racine neuve, sans erreur au journal.

⚠️ **MAIS `exec3` ET `exec4` ONT TOURNÉ EN MÊME TEMPS, et cela change la
lecture.** Horodatages, relevés dans les orchestrateurs : `exec3` démarre à
`02:09:20` et sa mesure court jusqu'à `02:18:42` ; **`exec4` démarre à
`02:17:19`** et sa première action est un nettoyage qui tue tous les `agent` de
la VM. Les deux exécutions se recouvrent de `02:17:19` à `02:19:42`.

**Conséquences, toutes deux vérifiables sur les pièces :**

- **le journal d'agent versé sous le nom `agent-exec3` N'EST PAS celui
  d'`exec3`.** `agent-exec3-plat.log` et `agent-exec4-plat.log` commencent tous
  deux à `2026-08-20T02:17:24.8616` — ce sont deux copies, l'une tronquée (131
  lignes) l'autre non (164), du **même** journal, celui de l'agent lancé par
  `exec4`. **Le journal propre d'`exec3` a été écrasé et il est perdu** ;
- **l'échec d'`exec4`** (« Le lecteur n'a pas pu être monté : premier message
  invalide : {role, session} attendu », **0** fenêtre après 90 s) s'explique
  par la session de signaling que l'agent d'`exec3`, encore vivant, tenait sur
  le **même** préfixe.

**Il reste donc UNE occurrence d'énumération vide sur laquelle un confondeur
pèse, et aucune preuve côté agent.** L'intermittence est **relevée, pas
caractérisée** ; sa cause reste ouverte. *L'hypothèse la plus tentante a été
écartée : `Session::default()` ne court-circuite pas la requête —
`agent/src/pont/enumeration.rs`, `entrees: Option<Vec<Entree>>` naît `None`.*

⚠️ **C'est le piège de D8 rejoué, dans l'autre sens** : là-bas un superviseur
survivant faisait relire le journal **périmé** de la tentative précédente ; ici
un pilote lancé trop tôt fait relire celui de la **suivante**. *`Get-Process
agent` se revérifie après chaque tentative — et deux exécutions ne se
chevauchent jamais.*

---

## 8. Ce que la recette a mesuré CONTRE son propre protocole

**Quatre contrôles écrits par le plan étaient incapables de rendre le verdict
qu'ils annonçaient.** Tous sont désormais annotés dans le plan.

| Contrôle | Défaut | Ce qu'il aurait fait croire |
| --- | --- | --- |
| `grep -c 'pont lancé'` (attendu : 1) | la chaîne **n'existe pas** ; la trace est `pont fichiers lancé`, et **deux** lignes distinctes la portent (`lanceur::pont` en `INFO`, `surveillance_pont` en `WARN`) — relevé **2** aux cinq exécutions | un pont absent, sur une exécution parfaite |
| `grep … ERROR_MOD_NOT_FOUND` | la chaîne **n'est émise nulle part** ; le journal porte `chargement de ProjectedFSLib.dll` + `0x8007007E` | le contrôle de D1 échoué, alors qu'il passe |
| `grep -c 'ERROR_SEM_TIMEOUT\|delai depasse'` → 0 | **aucune des deux chaînes n'est émise** ; le témoin réel est `commande expirée` | des budgets tenus, alors que rien ne les a exercés |
| `objdump` du Step 2 de la tâche 12 | **deux** défauts : le chemin publié est faux (espace de travail cargo → la cible est à la RACINE) et le `\|\| echo "AUCUN import"` **traduit un fichier absent en VERT** ; et la recette du rouge est insuffisante — une `pub fn` **sans appelant** est éliminée par l'éditeur de liens, `raw-dylib` n'émet alors aucun import, **le rouge n'est pas apparu** | un contrôle correct qui vient d'échouer à devenir rouge |

✅ **Le quatrième a été rendu rouge pour de bon** : l'appel placé dans
`pont::executer`, atteignable depuis `main`, fait apparaître
`Nom DLL: projectedfslib.dll` ; le vert revient au retrait
(`f1-tache12-objdump.txt`, les quatre états versés).

⚠️ **Portée de ce contrôle, à ne pas élargir** : il porte sur le binaire
`x86_64-pc-windows-gnu` de l'hôte, **jamais** sur le `msvc` livré. Le contrôle
qui porte sur le vrai binaire est le Step 3, et il est d'une autre nature.

---

## 9. La revue transverse de fin de branche

Elle a trouvé **cinq** défauts en D7, trois en D8, six en D9, douze en D10, sept
en D11, huit en P1, dix en P2, cinq en S1, neuf en E, douze en P3, douze en S2.
**Onze ici**, et tous ont la même forme : **corrects des deux côtés pris
séparément**, faux une fois la branche entière lue. *Une revue par tâche ne peut
structurellement pas les voir.*

### 9.1 Les affirmations de code que la BRANCHE elle-même a réfutées

| # | Où | Ce qu'elle disait | Ce qui l'a réfutée | Sort |
| --- | --- | --- | --- | --- |
| 1 | `agent/src/pont/chemins.rs:16` | « obtiendra donc `Introuvable` » | **la recette** (§7.1) | **CORRIGÉ**, avec le relevé et l'hypothèse de mécanisme |
| 2 | `client/src/fichiers/adaptateur.ts:16` | jumeau du précédent : « `getFileHandle` lèvera `NotFoundError` » | idem | **CORRIGÉ** |
| 3 | `agent/src/pont/notifications.rs:8` | citait « toute tentative d'écriture rend `ERROR_WRITE_PROTECT` » comme un absolu | **la recette** (§7.2) — et **le corps du même module** le réfutait cinquante lignes plus bas | **CORRIGÉ** |
| 4 | `agent/src/pont/erreurs/tests.rs:64` | « toute écriture, toute **création**, toute suppression y aboutit » | idem | **CORRIGÉ** |
| 5 | `agent/src/pont/erreurs.rs:82` et `notifications/tests.rs:3` | mêmes absolus, portés ailleurs | idem | **CORRIGÉS** |
| 6 | `agent/src/pont.rs:44` | « **Tâche 13** : la racine est montée et VIDE, aucune requête ne part vers le navigateur » | **la tâche 14** de la même branche (`3c0d84c`) | **CORRIGÉ** |
| 7 | `agent/src/pont/projfs/rappels.rs:192`, `:264`, `:293`, `:353` | quatre rappels documentés comme rendant `S_OK` vide ou `ERROR_FILE_NOT_FOUND` — la **ligne suivante** appelle `etat.demander(…)` et rend `EN_COURS` | idem | **CORRIGÉS** (4 places) |
| 8 | `agent/src/pont/transport.rs:206` | décrivait **au présent** le défaut d'aiguillage d'`evenements.rs` | **la tâche 17** (`df7fd4d`) | **CORRIGÉ** |
| 9 | `agent/src/transport.rs:23` | « Ce fichier ne porte plus que l'état de la session **et la boucle qui l'anime** » ; et la liste des sous-modules omettait `boucle` | **la tâche 17**, qui a extrait `Session::run` vers `transport/boucle.rs` | **CORRIGÉ** |
| 10 | `agent/src/pont/projfs.rs:64` | « L'application reçoit une E/S expirée » | **la recette** : ce chemin **n'a jamais été observé** | **ANNOTÉ** — voir la réserve ci-dessous |
| 11 | spec `:157` | « `agent.exe` est un seul binaire pour les **trois** modes », renvoi `main.rs:303-327` | **la tâche 9** : il y en a **quatre**, et `:303-327` désigne maintenant un `mod tests` | **ANNOTÉ** |

⚠️ **Le n°10 est classé « suspect fort », pas « établi ».** `commande expirée`
vaut 0 aux cinq exécutions nominales alors que `DELAI_LIRE` vaut 5 s et que des
lectures ont calé 540 s. **L'absence de trace établit que le balayage n'a rien
retiré ; elle ne dit pas OÙ le blocage se produit.** Un blocage **en amont** de
l'inscription en table laisserait ce paragraphe littéralement vrai tout en
décrivant un chemin que rien n'atteint. Trancher demanderait une trace à
l'inscription, **qui n'existe pas**. L'annotation dit cela, elle ne conclut pas.

### 9.2 Les documents annotés

- **spec §2** — un encadré dit que **F0 a rendu ce relevé historique** ; les
  phrases « l'état exact où se trouve la VM **aujourd'hui** », « **ROUGE
  aujourd'hui** » (§3.1, §8, §10) sont **datées, donc vraies comme histoire**,
  et conservées ; le mot « aujourd'hui » ne portait pas sa date, d'où l'encadré ;
- **spec §8** — les trois faussetés : l'absolu d'écriture, le rouge (iii)
  inatteignable, et « les cinq rappels **en mode asynchrone** » (sa propre table
  §4.3 dit que deux sont **synchrones** ; livré : cinq implémentés, **trois**
  asynchrones) ;
- **spec §3.4** — le « fait de code à connaître » sur `dispatch_channel_data`
  n'en est plus un, et sa plage `:168-189` n'a **jamais** désigné cette fonction
  (elle vivait en `:219-239`) ;
- **plan** — les quatre contrôles du §8 ci-dessus, plus deux nombres :
  `evenements.rs` annoncé à **391** lignes (il en faisait **279** à la rédaction,
  **363** aujourd'hui — le 391 n'a jamais été vrai), et la ligne « **toutes
  exactes** » du tableau des divergences, qui l'est d'au moins une de moins.

### 9.3 Ce que la revue a cherché SANS rien trouver

Pour que la couverture soit connue :

- **le déplacement d'`entetes.rs` vers `proto/`** — propre. `agent/src/pont/entetes.rs`
  porte sa propre annotation ✅ « LA DETTE DÉCLARÉE ICI EST SOLDÉE », les **trois**
  sites d'appel écrivent `use proto::fichiers::entetes;` (pas de ré-export, à
  dessein), et les deux mentions de l'ancien emplacement sont explicitement
  historiques ;
- **les promesses de F2–F5 dans `agent/src/pont/`** — que le plan désignait comme
  « le piège le plus probable de cette branche » : **aucune trouvaille**. Les dix
  occurrences balayées attribuent toutes explicitement le comportement à leur
  sous-bloc futur ;
- **le côté client de l'aiguillage** — `client/src/fichiers/protocole.ts` et son
  test décrivent l'ancien aiguillage à l'**imparfait** en nommant la tâche 17 :
  écrits en anticipation, devenus exacts ;
- **l'arbre de processus du superviseur** — `lanceur.rs`, `lanceur/pont.rs`,
  `main.rs:427-461`, la symétrie des `env_remove` dans les trois sens : à jour ;
- **les fonctions déplacées par l'extraction** (`accept_offer`, `run`,
  `drain_quietly`) — aucun renvoi textuel cassé ailleurs : les seules occurrences
  hors `boucle.rs` sont des **appels**, pas des localisations.

---

## 10. Les chiffres, RELEVÉS PAR LA COMMANDE après la dernière édition

| Vérification | Référence d'entrée (plan) | **Relevé** |
| --- | --- | --- |
| `cd agent && cargo test -p agent` | 467 | **614 passed, 0 failed** |
| `cd agent && cargo check --target x86_64-pc-windows-gnu` | 9 avertissements | **16**, tous de la famille `dead_code` |
| `cd client && npx vitest run` | 107 | **223 passed** (24 fichiers) |
| `cd client && npx vitest run --dir ../proto` | 35 | **111 passed** (5 fichiers) |

⚠️ **Les 16 avertissements se décomposent, et la décomposition est vérifiée** :
**11** préexistants, **2** venus du chantier Microphone (`micro.rs:196`,
`micro/dissimulation.rs:146` — F0/F1 n'a touché ni l'un ni l'autre), et **5**
imputables à F1, tous délibérés : quatre dans `pont/erreurs.rs`
(les variantes `Abandonnee`/`DisquePlein`/`RepertoireNonVide`/`DejaPresent`
réservées à F2–F3, `NOMBRE`, `TOUTES`, `index`) et un dans `pont/table.rs`
(`en_vol`).

⚠️ **`cd client && npx vitest run` ne couvre PAS `proto/ts/`** — la racine
Vitest est `client/`. **Deux commandes, pas une.** Sans cela
`proto/ts/fichiers-entetes.test.ts` n'aurait jamais tourné, et le vecteur
partagé `proto/fichiers-vectors.json` n'aurait épinglé qu'un seul côté.

⚠️ **L'arbre est PARTAGÉ avec un agent concurrent** (sous-projet ⑤, `plateforme/`
et `client/`). Un premier passage de `npx vitest run` a rendu **4 échecs** sur
un fichier, disparus au passage suivant sans qu'aucune de mes éditions ne les
concerne : les 223 ci-dessus sont le relevé **après** ma dernière édition, et
ils comptent aussi les tests de l'autre chantier.

**Tailles > 440 lignes, relevées après la dernière édition** (la commande de
`CLAUDE.md`) :

| Fichier | Lignes |
| --- | --- |
| `agent/src/encode.rs` | **1536** *(dette gelée)* |
| `agent/src/windows_source.rs` | **630** *(dette gelée)* |
| `agent/src/encode/arret.rs` | **500** (marge 0) |
| `client/verify-webrtc.mjs` | **494** *(⚠️ `CLAUDE.md` le publiait à 497 ; corrigé)* |
| `agent/src/capture.rs` | 492 |
| `agent/src/demarrage.rs` | 491 |
| `agent/src/pont/projfs/rappels.rs` | **488** (marge **12**) |
| `agent/src/transport/socket.rs` | 481 |
| `agent/src/transport/piste_video.rs` | 477 |
| `agent/src/pont/transport/tests.rs` | 474 |
| `agent/src/transport.rs` | **448** |

**Aucun fichier de code source ne dépasse 500 lignes** hors les deux entrées de
dette gelée. `rappels.rs` est le plus serré du sous-projet : il était à **489**,
et les corrections de la revue transverse lui ont rendu **une** ligne, la
réfutation écrite étant plus courte que l'annonce qu'elle remplace.

---

## 11. Ce que F0 et F1 n'établissent PAS

- **Aucun taux.** Cinq exécutions du chemin nominal, dont **trois** exploitables
  (`exec3` a un confondeur, `exec4` n'a pas monté) ; une du Step 3 ; deux du
  rouge (i) ; une sonde de borne. **Deux exécutions ne font pas une fréquence.**
- **Le condensat SHA-256 de bout en bout** — le critère 2, celui qui devait
  décider. Ni établi, ni réfuté.
- **La latence n'est mesurée par rien.** C'est l'objet de F4, et c'est le risque
  R2 : *le lecteur peut fonctionner et rester inutilisable.* La seule mesure de
  débit disponible est incohérente d'un facteur ~120 entre deux exécutions —
  **6,5 Mio/s** sur le rouge (i) contre **52 à 55 Kio/s** sur la sonde de borne.
  **Rien n'explique l'écart.**
- **Aucune constante n'est calibrée** : `TAILLE_TRAME_MAX`, `DELAI_ATTRIBUTS`,
  `DELAI_LIRE`, `DELAI_LISTER`. Elles rejoignent `BPP_MIN`, `FACTEUR_FOCUS`,
  `PART_DORMANTE_BPS`, `HYSTERESIS`, `REPIT_APRES_ECHEC`, `TAILLE_MAX_SORTIE`,
  `REPIT_REARMEMENT_AUDIO` et `REARMEMENTS_MAX`.
- **Aucun test d'hôte ne couvre `pont/projfs.rs`**, `#[cfg(windows)]` et appelé
  par le système. `cargo check --target x86_64-pc-windows-gnu` en vérifie les
  types, les emprunts et les durées de vie — **jamais le comportement**.
- **R7 reste OUVERT.** Les treize transcriptions ont tenu : sur les **sept**
  journaux qui portent `racine du pont fichiers montée` — `exec{1,2,5}`, les
  deux rouges (i), `dbg` et `verif`, relevé par la commande —, **aucun
  plantage de processus**. ⚠️ **Les ~367 relances du Step 3 ne comptent PAS ici**
  : la DLL y était renommée, `LoadLibraryW` échouait, et **aucune transcription
  n'a jamais été appelée**. ⚠️ **Et cela n'établit pas que le dépôt saurait
  attraper une transcription FAUSSE** : les mutations d'ABI jouées à la tâche 12
  avaient **survécu** aux tests d'hôte, et cinq entrées n'ont aucun jumeau
  `PRJ_*_CB`. Ce que la recette montre, c'est que **ces** transcriptions-ci sont
  bonnes.
- **`showDirectoryPicker()` n'a jamais été appelé**, ni le modèle de permission
  des répertoires choisis, ni l'activation utilisateur transitoire (§3).
- **`PRE_CONVERT_TO_FULL`** — l'écriture d'un fichier **existant** — n'a jamais
  été exercé.
- **Deux des trois rouges** (fermer la page-shell pendant une copie ; démarrer
  le pont avant le choix) **n'ont pas été provoqués**, et le troisième décrit un
  état inatteignable par ce chemin.
- **Aucun relais TURN pour le pont** — divergence assumée d'avec la session
  vidéo. Le pont ne traverse que ce que les candidats hôtes traversent.
- **Rien d'un client réel** : Chrome sans interface, décodage logiciel, sur
  l'hôte qui porte la VM. La File System Access API n'existe ni sur Firefox ni
  sur Safari — limite du **produit**.
- **Rien de plusieurs utilisateurs** : une VM, une racine, et `SESSION_DU_PONT`
  n'est pas namespacé.
- **Rien à travers une reconnexion WebRTC** : le chemin n'existe pas.
- **Rien de l'occupation disque au-delà de la trace** — aucune politique
  d'éviction (R4). L'hydratation observée n'a d'ailleurs jamais dépassé **42
  octets / 1 entrée** sur les exécutions nominales : le disque n'a pas grossi,
  parce que le gros fichier n'a jamais été copié.
- **Rien de l'ancien pont** : ni modifié, ni retiré, ni comparé chiffre à chiffre.
- **Le chemin d'extinction propre du superviseur** n'a toujours jamais été
  exercé, depuis D1.

---

## 12. Pièges neufs — à connaître avant de toucher à ce terrain

- ⚠️ **DEUX EXÉCUTIONS DE RECETTE NE DOIVENT JAMAIS SE CHEVAUCHER, et le
  symptôme n'est pas celui qu'on croit.** `exec3` et `exec4` se sont recouvertes
  de 2 min 23 s : la seconde a tué l'agent de la première **en pleine mesure**,
  la première a fait échouer la seconde au signaling, et **le journal d'agent
  versé sous le nom de la première est en réalité celui de la seconde** — même
  horodatage de début à la microseconde près. On lit alors une panne de produit
  là où il y a une collision de protocole. *`Get-Process agent` se revérifie
  après CHAQUE tentative, y compris échouée.*
- ⚠️ **Une heuristique de PID est un piège** : « le pont est le plus jeune des
  `agent` » est **faux** — l'ordre est superviseur, capteur, pont, **puis** les
  enfants. Le rouge (i) a d'abord tué un enfant et rendu un relevé qui se lisait
  comme un succès. **Relever le PID dans le journal d'agent, jamais le deviner.**
- ⚠️ **Une sonde peut ne pas borner ce que son nom annonce.** `borne-lecture.txt`
  le dit contre elle-même : la longueur passée à `FileStream.Read` **n'est pas**
  celle que ProjFS demande au rappel `GetFileData` — ProjFS choisit sa propre
  granularité. Les deux lignes de la sonde ne séparent donc **pas** « une trame »
  de « deux trames ».
- ⚠️ **Deux messages d'interface qui partagent une sous-chaîne font un instrument
  faux.** « Lecteur … **mont**é » et « n'a pas pu être **mont**é » : le pilote
  testait `includes('mont')` et a lancé une mesure de neuf minutes sur un pont
  **non monté** (`exec4`). Corrigé côté **pilote** par `f882b5a` ; **le produit
  garde l'ambiguïté**, et le prochain instrument tombera dedans.
- ⚠️ **Un `||` de repli transforme « fichier absent » en « contrôle vert ».** Le
  `objdump … || echo "AUCUN import"` du plan, sur un chemin faux, ne pouvait pas
  échouer pour la raison qu'il surveillait.
- ⚠️ **Une `pub fn` sans appelant ne rend pas un contrôle d'import rouge** :
  `agent` est un binaire, l'éditeur de liens élimine l'inatteignable, et
  `raw-dylib` n'émet alors aucun import. **Le rouge doit être sur un chemin
  atteignable depuis `main`.**
- ⚠️ **Le nom d'un `grep` de recette se vérifie contre le CODE, pas contre la
  spec.** Trois des quatre contrôles de F1 cherchaient des chaînes que le produit
  n'émet pas. **Deux d'entre eux auraient fait lire un succès comme un échec.**
- ⚠️ **`cd client && npx vitest run` ne couvre pas `proto/ts/`.** Aucun document
  du dépôt ne le disait avant F1.
- ⚠️ **Nettoyer `agent` seul ne suffit pas quand le chantier touche `proto`** :
  `cargo clean --release -p proto -p agent` avant `scripts/build-agent.sh`, et
  **vérifier la taille du binaire** — une compilation de 0,13 s est un aveu.

---

## 13. Ce que F1 lègue

**Défauts observés et NON corrigés :**

1. 🔴 **La casse rend le mauvais fichier en silence** (§7.1), et de façon
   **incohérente avec elle-même**. Le remède reste celui de F3 : une table de
   correspondance alimentée par l'énumération. ⚠️ **Le legs a changé de nature —
   ce n'est plus « on ne trouve pas », c'est « on rend autre chose ».**
2. 🔴 **Une création locale réussit** (§7.2). Le produit le journalise ; il ne
   l'empêche pas, et ne peut pas — c'est une notification POST. **F2.**
3. 🔴 **L'énumération vide par intermittence** (§7.3), une occurrence exploitable,
   **cause inconnue**, journal d'agent perdu.
4. 🔴 **Des lectures calent sans jamais expirer** : `commande expirée` reste à 0
   pendant qu'un `Get-ChildItem` ne rend pas la main en 540 s. **On ne sait pas
   où le blocage se produit**, faute d'une trace à l'inscription en table. C'est
   le legs le plus proche de rendre le lecteur inutilisable, et c'est l'objet
   de **F4**.
5. **Le débit varie d'un facteur ~120** entre deux exécutions (6,5 Mio/s contre
   52–55 Kio/s), sans explication.

**Contrôles et mesures dus :**

6. **Le critère 2 — le condensat de bout en bout** — reste à établir. C'est le
   premier geste de toute recette suivante.
7. **Les rouges (ii) et (iii)** ne sont pas provoqués, et **(iii) doit d'abord
   être RÉÉCRIT** : l'état qu'il décrit n'est atteignable que par une racine
   survivante d'un arrêt brutal.
8. **`PRE_CONVERT_TO_FULL` n'a jamais été exercé** — la moitié vraie de la
   promesse de lecture seule n'a aucun témoin.
9. **R7 reste ouvert** : cinq entrées sans jumeau `PRJ_*_CB`, et un mauvais
   `transmute` de fonction est un défaut que rien n'attrape avant l'exécution.
10. **Une racine peut survivre à un arrêt brutal du superviseur**, et rien dans
    F1 ne la démonte — pendant exact des sorties virtuelles de D5.

**Choix de conception assumés, à rouvrir le jour venu :**

11. **Aucun cache d'énumération en F1** — délibérément écarté (divergence 9 du
    plan) : `Rafraichir`, seul moyen de l'invalider, est un livrable de **F5**, et
    un cache que rien ne vide reproduirait le défaut de l'ancien pont
    (`src/file.js`, cache **sans TTL**).
12. **Un seul morceau en vol à la fois** — le contrôle de flux par
    `bufferedAmount`/`SEUIL_TAMPON` relève de **F3**. *Déclaré, pas implémenté à
    moitié.*
13. **Cinq verbes du protocole ne sont pas livrés** : `proto/src/fichiers.rs` ne
    définit que `TYPE_LISTER`, `TYPE_ATTRIBUTS` et `TYPE_LIRE` (plus les trois
    réponses et `TYPE_ECHEC`). `Ecrire`, `Creer`, `Renommer`, `Supprimer` et
    `Tronquer` — nommés par la spec §4.2 — **n'existent nulle part dans le
    code**, non plus que `Rafraichir`, qui est le sixième et va dans l'autre
    sens. Ils appartiennent à F2, F3 et F5, et `FICHIERS_VERSION` vaut **1**
    précisément pour que leur arrivée soit une rupture visible.
14. **Aucun relais TURN pour le pont**, à rouvrir le jour où la page-shell et la
    VM ne se voient pas directement.
