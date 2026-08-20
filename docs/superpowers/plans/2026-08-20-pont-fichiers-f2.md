# Sous-projet ③ Pont fichiers — sous-bloc F2 : l'écriture, et la fenêtre de perte

**20 août 2026.**
Conception : `docs/superpowers/specs/2026-08-19-pont-fichiers-design.md` (commit
`06619fc`), §3.5, §4.3, §5, §6 et §8 F2.
Sous-blocs livrés dont celui-ci hérite : `docs/superpowers/plans/2026-08-19-pont-fichiers-f1.md`
et son document de résultats `…-f1-resultats.md`, qui **font autorité sur l'état
réel** partout où ils contredisent la spec.

> **Périmètre, énoncé négativement d'abord.** F2 ne fait **ni** le renommage,
> **ni** la suppression, **ni** la table complète des douze `HRESULT` (F3) ;
> **ni** le banc de latence (F4) ; **ni** `Rafraichir`, ni la mesure
> d'occupation disque, ni la reprise à travers un redémarrage de VM (F5). Il
> livre **une seule chose** : que des octets écrits dans la VM arrivent sur le
> poste local, et que ce qui n'y arrive pas soit **nommé** plutôt que perdu en
> silence.

---

## 0. Ce que F2 doit trancher AVANT d'écrire une ligne de code

Quatre questions de la conception sont ouvertes, et deux d'entre elles sont
tranchées ici **contre** la lettre de la spec. Elles gouvernent tout le
découpage qui suit.

### 0.1 🔵 Par quelle notification une écriture se refuse-t-elle réellement ?

**Par une seule, et elle ne couvre pas tous les cas.** Relevé dans
`windows-0.62.2/src/Windows/Win32/Storage/ProjectedFileSystem/mod.rs` (fichier
de **621** lignes, relevé par la commande le 20 août 2026) et dans
`agent/src/pont/notifications.rs`, qui les recopie avec leur ligne source :

| Notification | Valeur | `mod.rs:` | Nature | Refusable ? |
| --- | --- | --- | --- | --- |
| `PRJ_NOTIFICATION_FILE_PRE_CONVERT_TO_FULL` | 4096 | **340** | **PRE** | **OUI** |
| `PRJ_NOTIFICATION_PRE_RENAME` | 32 | **378** | **PRE** | OUI |
| `PRJ_NOTIFICATION_PRE_DELETE` | 16 | **377** | **PRE** | OUI |
| `PRJ_NOTIFICATION_NEW_FILE_CREATED` | 4 | **349** | POST | **NON** |
| `PRJ_NOTIFICATION_FILE_OVERWRITTEN` | 8 | **339** | POST | **NON** |
| `PRJ_NOTIFICATION_FILE_HANDLE_CLOSED_FILE_MODIFIED` | 1024 | **336** | POST | **NON** |
| `PRJ_NOTIFICATION_FILE_HANDLE_CLOSED_FILE_DELETED` | 2048 | **335** | POST | NON |
| `PRJ_NOTIFICATION_FILE_HANDLE_CLOSED_NO_MODIFICATION` | 512 | **337** | POST | NON |

> ✅ **Les DOUZE numéros de ligne publiés par `agent/src/pont/notifications.rs:64-76`
> ont été REVÉRIFIÉS un à un dans `windows-0.62.2` le 20 août 2026, et ils sont
> **TOUS EXACTS** — 340, 378, 377, 379, 342, 349 pour les `PRJ_NOTIFICATION_*`,
> et 385, 391, 390, 392, 388 pour les `PRJ_NOTIFY_*`.
>
> ❌ **Une première rédaction de ce plan en déclarait TROIS faux, et c'était MA
> lecture qui l'était** : j'avais relevé `mod.rs:204-206` dans
> `windows-sys-0.61.2`, un crate voisin dont le module ProjFS porte les mêmes
> symboles à d'autres lignes. Le défaut a été trouvé en **relisant mes propres
> citations après les avoir écrites**, ce que ce dépôt exige précisément parce
> qu'une citation peut être fausse à l'instant même où on l'écrit. *Il est
> consigné ici plutôt que corrigé en silence.*
>
> ⚠️ **La leçon opératoire, pour F2 et pour tout successeur** : `windows` et
> `windows-sys` exposent **deux** modules `Win32/Storage/ProjectedFileSystem`
> aux symboles identiques et aux lignes différentes. Le crate qui fait foi est
> celui que `agent/Cargo.toml` déclare — **`windows`** —, et un
> `find … -name mod.rs` qui en trouve deux doit être désambiguïsé avant d'être
> lu. **La VALEUR fait foi, jamais le numéro de ligne.**

**Ce que cela impose, et c'est la charnière de F2 :**

1. **`PRE_CONVERT_TO_FULL` est le SEUL point de refus d'une écriture**, et il ne
   concerne que l'hydratation complète d'un fichier **déjà projeté**. C'est le
   levier que `agent/src/pont/notifications.rs:115` actionne aujourd'hui pour
   tenir la lecture seule de F1 — et **il n'a JAMAIS été exercé** (F1 résultats
   §7.2 et §11 : « aucune pièce ne le confirme ni ne l'infirme »). **La recette
   de F2 est la première occasion de savoir s'il existe pour de bon.**
2. **Un fichier créé de toutes pièces ne traverse AUCUNE `PRE_`.** Refuser une
   création est donc **impossible** — pas difficile : impossible. C'est ce que
   F1 a mesuré (`mesure-exec2.txt`, `mesure-exec5.txt` : `creation|CREEE`) et ce
   que `notifications.rs:123-128` documentait déjà avant la mesure.
3. **Il n'existe aucune notification par laquelle on ACCEPTE une écriture.**
   On accepte en **ne refusant pas** `PRE_CONVERT_TO_FULL` ; on apprend qu'il y
   a des octets à pousser par `FILE_HANDLE_CLOSED_FILE_MODIFIED` et
   `FILE_OVERWRITTEN`, **toutes deux POST**, c'est-à-dire **après** que
   l'application a refermé son handle et cru avoir enregistré.
4. **Corollaire, qui doit être écrit dans le code et dans le document de
   résultats : le chemin d'écriture de F2 n'a AUCUNE contre-pression.** Si le
   navigateur refuse la poussée — permission révoquée, disque plein, onglet
   fermé —, l'application a **déjà** reçu son succès. Aucun `HRESULT` ne peut
   plus l'atteindre. Les deux seuls leviers restants sont :
   - un refus **en amont** au `PRE_CONVERT_TO_FULL`, portant sur un **état**
     (canal fermé, racine montée en lecture seule) et jamais sur l'issue ;
   - une **dénonciation** après coup : journal de reprise, compteur d'écritures
     dues, `beforeunload`.

> ⚠️ **La spec §8 F2 écrit « toute tentative d'écriture rend
> `ERROR_WRITE_PROTECT` » comme la promesse de F1 ; F1 l'a réfutée pour la
> création. Ce plan n'hérite pas de la promesse — il hérite de la réfutation.**
> Aucune tâche ci-dessous ne prescrit « l'écriture est refusée » sans dire
> **laquelle** et **par quelle notification**.

### 0.2 ⛔ Le critère 2 de la spec §8 F2 est INATTEIGNABLE dans F2, et il faut le dire maintenant

La spec §8 F2 exige :

> « 2. un fichier enregistré depuis une application employant l'idiome
> **écrire-temporaire / renommer / supprimer** (LibreOffice ou Word) apparaît de
> même — **c'est ce critère, et lui seul, qui justifie D5** ».

Et la spec §8 F3 livre `Renommer` et `Supprimer`.

**Ce critère demande donc à F2 de démontrer ce que F3 livre.** Il est déplacé en
F3, et ce plan le déclare plutôt que de le jouer à moitié. La conséquence est
énoncée dans le code et dans la recette :

> 🔴 **F2 CONTINUE DE REFUSER `PRE_RENAME` ET `PRE_DELETE`**, comme F1
> (`notifications.rs:115`). Une application qui emploie l'idiome
> écrire-temporaire/renommer/supprimer **échouera bruyamment au renommage**,
> avec `ERROR_WRITE_PROTECT` (0x80070013), plutôt que de réussir sur la VM en
> laissant le poste local sur l'ancien contenu. **Le choix est délibéré** :
> accepter le renommage sans le pousser produirait exactement la « perte
> silencieuse » que l'en-tête de `notifications.rs:31-39` existe pour
> interdire.

⚠️ **Et cela porte un risque qui peut rendre F2 non livrable — voir §8, R-F2-1 :
si le Bloc-notes de CETTE VM emploie l'idiome temp+rename, le critère 1 de F2
échoue lui aussi.** C'est pourquoi la tâche 14 est une **sonde préalable** qui
mesure l'idiome **avant** que la recette ne choisisse son instrument, sur le
modèle de `sonde-picker.txt` en F1.

### 0.3 🔴 La fenêtre de perte : ce qu'elle est, quand elle s'ouvre, ce qu'elle coûte

**Elle ne peut pas être supprimée. Ce n'est pas un arbitrage de ce plan : c'est
le fonctionnement de ProjFS** (spec §6.1). Le fournisseur n'est **jamais** sur
le chemin de l'écriture ; l'application écrit sur un fichier NTFS local et le
fournisseur est prévenu **à la fermeture du handle**.

| | Instant | Ce qui est vrai à cet instant |
| --- | --- | --- |
| **Ouverture** | `CloseHandle` rend la main à l'application | l'application **croit avoir enregistré** ; les octets vivent **uniquement** sur le disque de la VM |
| **Fenêtre** | ↓ | le pont journalise, lit le fichier local, le découpe, pousse les trames, attend le `Fait` du dernier morceau |
| **Fermeture** | réception du `Fait` du dernier morceau | l'entrée sort du journal, **et pas une microseconde avant** |

**Ce qui se passe si la session meurt à l'intérieur — les quatre cas, tous
nommés :**

1. **Le pont meurt** (panique dans un rappel, `TerminateProcess` du job object).
   Le journal est sur le disque de la VM, **hors de la racine**
   (`%LOCALAPPDATA%\Guacamole\pont\`, `agent/src/pont/projfs/racine.rs:36-52`) ;
   le fichier hydraté est dans la racine. Le superviseur relance le pont
   (mesuré en F1 : `delai_apres_mort_ms=44`), qui relit le journal et repousse.
   **Récupérable.**
2. **La page-shell est fermée ou rechargée.** Le canal tombe, la poussée
   échoue, l'entrée **reste** au journal. Au remontage — qui exige un clic et un
   nouveau `showDirectoryPicker()` —, le pont repousse. **Récupérable si et
   seulement si l'utilisateur revient AVEC LE MÊME RÉPERTOIRE** (spec §6.4
   cas 2 : le pont ne peut pas en être certain, la File System Access API ne
   donnant aucun identifiant stable de répertoire).
3. **La VM redémarre.** Journal et fichier hydraté survivent tous deux au
   disque. **Récupérable** — mais la reprise à travers un redémarrage est un
   livrable de **F5**, pas de F2 : F2 relit le journal **au démarrage du pont**,
   ce qui couvre 1 et 2, et le cas 3 n'est ni exercé ni revendiqué ici.
4. **La racine doit être recréée**, ou la VM est détruite/réinitialisée par la
   plateforme, ou l'utilisateur ne revient jamais. **PERTE DE DONNÉES, définitive.**
   Le journal survit dans le cas de la racine recréée et **nomme** ce qui a été
   perdu. *Savoir ce qu'on a perdu n'est pas l'avoir* (spec §6.4).

**Ce que la fenêtre coûte, avec le seul chiffre que ce dépôt possède.** F1 n'a
mesuré aucune latence (c'est F4), et sa seule mesure de débit est **incohérente
d'un facteur ~120** : **6,5 Mio/s** sur le rouge (i) contre **52 à 55 Kio/s**
sur la sonde de borne, *sans explication* (F1 résultats §11). À la borne basse,
un fichier de 12 Mio reste **environ quatre minutes** dans la fenêtre. **F2 ne
publiera donc aucune borne de durée** ; il publiera, pour chaque écriture, sa
taille et son temps réel de traversée, et laissera F4 en faire une loi.

### 0.4 Ce que `beforeunload` garantit — et les trois choses qu'il ne garantit pas

La spec §6.2 demande « un `beforeunload` […] **avec un texte qui nomme les
fichiers** ». **Ce texte n'existe pas.**

- ⛔ **Le message personnalisé est ignoré par tous les navigateurs modernes.**
  `beforeunload` n'affiche qu'un libellé générique choisi par le navigateur.
  **Nommer les fichiers dans le dialogue est impossible** ; les nommer **dans la
  page**, à côté du compteur, l'est. *(Fait de plateforme, non mesuré ici, et
  déclaré comme tel.)*
- ⛔ **Il ne se déclenche pas du tout** si l'onglet est tué par le gestionnaire
  de tâches, si le navigateur plante, si la machine s'éteint, ou si l'onglet est
  écarté par le navigateur faute de mémoire.
- ⛔ **Il ne peut RIEN vider.** L'événement est synchrone, et un envoi sur un
  `RTCDataChannel` amorcé dedans n'a aucune garantie de partir. **`beforeunload`
  avertit ; il ne sauve pas.** Ce qui sauve est le journal, et lui seul.
- ⚠️ Il exige en outre une **activation utilisateur persistante** pour afficher
  son dialogue. Elle est acquise : monter le lecteur passe par un clic
  (`client/src/shell-page.ts:107-115`).

**Décision** : `beforeunload` pose `preventDefault()` tant que le compteur
d'écritures dues est non nul, **et rien d'autre**. La liste des fichiers dus
vit dans la page. Le code porte les trois limites ci-dessus en commentaire.

### 0.5 Le quota, l'espace, et ce que l'application en voit

- **Le disque de la VM** n'est pas le sujet : relevé par la commande le 20 août
  2026 sur `192.168.3.2`, `C:` porte **758,8 Go libres**.
- **L'espace du poste local est inconnaissable.** `navigator.storage.estimate()`
  décrit le quota de l'**origine** (OPFS, IndexedDB), **pas** le répertoire
  choisi par `showDirectoryPicker()`. Aucune API ne le donne. *(Fait de
  plateforme, déclaré, non mesuré.)*
- **Ce que le navigateur voit** quand l'écriture ne passe pas : une
  `DOMException` — `QuotaExceededError` sur `write()` ou `close()`, ou
  `NotAllowedError` si la permission a été révoquée entre-temps.
- **Ce que l'application Windows voit : RIEN.** Le handle est refermé depuis
  longtemps. `ERROR_DISK_FULL` (0x80070070, `agent/src/pont/erreurs.rs:75,152`)
  **n'atteint personne** — il n'y a plus de commande ProjFS à compléter.
  **Cette phrase doit figurer dans le code**, auprès de la variante
  `Erreur::DisquePlein`, sans quoi un successeur croira que le code d'erreur
  fait quelque chose.
- **Ce que l'utilisateur voit** : le compteur d'écritures dues qui ne redescend
  pas, la ligne du fichier en cause, et le motif. C'est la seule réponse
  possible, et c'est pourquoi le compteur est un livrable et non un confort.

⚠️ **Le plafond `TAILLE_MAX_FICHIER` de la spec §3.5.2 n'est PAS implémenté en
F2, et la raison est de fond** : le seul endroit où un refus de taille serait
**visible par l'application** est `PRE_CONVERT_TO_FULL` — mais on n'y connaît
que la taille **d'avant** l'écriture, qui ne borne pas celle d'après. Un plafond
appliqué au write-back, lui, est invisible. **F2 pose donc un plafond de
JOURNAL, pas de refus** : au-delà de `TAILLE_ECRITURE_SIGNALEE` la trace passe
en `warn!` et la page-shell nomme le fichier. La constante est **non calibrée**,
et son commentaire le dira.

### 0.6 La concurrence — quatre cas, quatre règles

| Cas | Règle de F2 | Où elle vit |
| --- | --- | --- |
| **Deux écritures sur le même chemin** (deux fermetures de handle rapprochées) | **Une seule poussée en vol par chemin.** Une notification qui arrive pendant une poussée marque l'entrée `a_rejouer` ; à la fin de la poussée en cours, le fichier est **relu depuis le début**. Jamais deux poussées concurrentes du même chemin, jamais une notification perdue | `pont/ecriture.rs`, **PUR**, testé |
| **Une écriture pendant une lecture du même fichier** | Aucune règle spéciale, **et c'est démontrable** : ProjFS n'appelle `GetFileData` que sur un **substitut** ; une fois le fichier converti en complet, les lectures ne nous atteignent plus. La conversion elle-même passe par nos `GetFileData` en vol, que **le fil du pont** complète — un fil distinct de celui qui écrit | discipline de fil, `pont/projfs.rs:11-52` |
| **Le fil d'écriture lit un fichier de la racine** | ⚠️ **Il ne doit JAMAIS courir sur le fil du pont.** `agent/src/pont/service.rs:13-17` l'écrit déjà : « il ne parcourt pas la racine […] il s'attendrait lui-même ». Un fil dédié, et la raison en commentaire | `pont.rs`, `pont/ecriture/fil.rs` |
| **Le poste local modifie le fichier pendant la poussée** | **Aucun verrou inter-machines** (spec §3.5.2). Non traité, nommé | doc |

---

## 1. Global Constraints

### 1.1 Références d'entrée, RELEVÉES PAR LA COMMANDE le 20 août 2026

Tous les nombres ci-dessous ont été **exécutés**. Ils sont la référence
d'entrée : tout écart en cours de branche est un défaut, pas une dérive.

| Contrôle | Commande | Relevé |
| --- | --- | --- |
| Plafond de 500 lignes | la commande de `CLAUDE.md` § Conventions | **deux** fichiers au-dessus, les deux entrées de dette gelée : `agent/src/encode.rs` **1536**, `agent/src/windows_source.rs` **630** |
| État de ProjFS sur la VM | `Get-WindowsOptionalFeature -Online -FeatureName Client-ProjFS` | `State : Enabled`, `ProjectedFSLib.dll : True` |
| Racine et état du pont sur la VM | `Test-Path` sur `C:\Users\guacamole\Mes Fichiers` et `…\AppData\Local\Guacamole\pont` | **les deux `False`**, `instance.guid` **absent** — *la racine ne survit à aucune exécution de F1* |
| Espace libre `C:` de la VM | `(Get-PSDrive C).Free` | **758,8 Go** |
| `PONT` transmise par le lanceur | `grep -n PONT scripts/run-agent.sh` | présente, **ligne 33** |

⚠️ **Les quatre suites de tests sont à relever par l'implémenteur de la tâche 1
et à inscrire dans le document de résultats.** Les valeurs de fin de F1
(**614** Rust, **223** client, **111** proto, **16** avertissements de
compilation croisée) sont recopiées de
`docs/superpowers/plans/2026-08-19-pont-fichiers-f1-resultats.md` §10 et **n'ont
pas été réexécutées ici** : l'arbre est partagé avec les sous-projets ④ et ⑤,
dont les commits postérieurs à F1 (`264c275` au moins) ont déjà fait bouger des
fichiers du périmètre. **Ne pas s'en servir comme référence sans les relancer.**

### 1.2 🔴 `cd client && npx vitest run` NE COUVRE PAS `proto/ts/`

Fait établi par F1, à rejouer à chaque tâche qui touche `proto/ts/` :

```bash
cd client && npx vitest run                # client/src/ seul
cd client && npx vitest run --dir ../proto # proto/ts/
cd client && npx tsc --noEmit              # couvre les DEUX (client/tsconfig.json)
```

**Un test ajouté à `proto/ts/fichiers.test.ts` et non lancé par la seconde
commande n'est pas un test.**

### 1.3 Règles de travail

- **Plafond de 500 lignes.** **Une extraction est OBLIGATOIRE et vient AVANT
  l'addition** — tâche 3 ci-dessous, et rien d'autre ne peut être écrit dans
  `agent/src/pont/projfs/rappels.rs` avant elle. Les marges sont au §2.
  **Jamais de compression** : D9 a compressé deux fichiers, geste que
  `CLAUDE.md` interdit nommément, puis a dû les extraire quand même.
- **Séparer le PUR du `#[cfg(windows)]`.** C'est ce qui a permis à F1 de jouer
  une quarantaine de mutations sur l'hôte. **Règle de ce plan, plus stricte que
  la spec §4.4** : *le write-back tout entier — journal, file, coalescence,
  découpe, lecture du fichier local — est PUR et compile sur Linux.* Seul le
  **déclenchement** (le rappel de notification) est gaté. La lecture d'un
  fichier NTFS ordinaire n'a rien de spécifique à Windows, et c'est
  précisément parce que ProjFS ne met le fournisseur nulle part sur le chemin
  de l'écriture.
- **Un contrôle qu'on n'a jamais vu rouge n'est pas un contrôle**, et
  **un plan est une source de contrôles vacueux** : D10 en a attrapé quatre
  dont **trois écrits par son propre plan**, F1 quatre de plus, dont deux qui
  auraient fait **lire un succès comme un échec**. Chaque tâche ci-dessous
  porte son état rouge **et la vérification que cet état est atteignable**.
- **Toute chaîne de `grep` prescrite se vérifie contre le CODE, jamais contre
  la spec ni contre ce plan.** F1 a prescrit `pont lancé` (la trace est
  `pont fichiers lancé`), `ERROR_MOD_NOT_FOUND` (jamais émise) et
  `ERROR_SEM_TIMEOUT\|delai depasse` (le témoin réel est `commande expirée`).
  **Chaque `grep` de la tâche 15 est accompagné du `grep -rn` sur `agent/src/`
  ou `client/src/` qui établit que la chaîne est émise.**
- **Jamais `git add -A`** : nommer les fichiers.
- **`PONT_ECRITURE` doit être ajoutée explicitement à `scripts/run-agent.sh`**,
  dans une **tâche dédiée placée avant celle qui en a besoin** (tâche 2). Piège
  payé en D1 (`SUPERVISEUR`), D2 (`MULTIFENETRE_REPRISE`), D7 (`AUDIO`) ; évité
  en D3, D6 et F1 par exactement ce geste.
- **Convention de valeur** : `PONT_ECRITURE=0` **désarme**, et le test est
  `matches!(std::env::var("PONT_ECRITURE").as_deref(), Ok(v) if v != "0")` — la
  forme exacte de `agent/src/main.rs` pour `CAPTEUR` et `PONT`. **C'est une
  variable de BANC, jamais une configuration livrée** : elle n'existe que pour
  rendre le compteur d'écritures dues rouge.
- **Convention de module enfant** (`CLAUDE.md`) : **aucun module de ce plan
  n'en relève.** Elle ne vise que les modules extraits d'un parent
  `#[cfg(windows)]` pour compiler sur l'hôte, et qui deviennent frères de
  premier niveau dans `main.rs`. Ici, `pont.rs` n'est pas gaté et ses enfants
  purs se déclarent par un simple `mod` ; `projfs/rappels/*` sont des enfants
  ordinaires d'un parent déjà gaté. **Aucun `#[path]` n'est écrit.**
- **La VM n'est pas démarrée automatiquement**, et **elle est un état
  partagé**. Avant toute tâche de recette : `virsh list --all`,
  `virsh start Windows`, attendre WinRM **puis** un accès réel à `/media/vm`
  (`until ls /media/vm/dev`), et `set -a && source .env && set +a` avant
  `scripts/build-agent.sh` — sans quoi il s'arrête **en silence**.
  ⚠️ **Le chantier touche `proto` : `cargo clean --release -p proto -p agent`
  avant la construction, et vérifier la TAILLE du binaire.** Une compilation de
  0,13 s est un aveu.
- 🔴 **DEUX EXÉCUTIONS DE RECETTE NE SE CHEVAUCHENT JAMAIS.** F1 en a perdu
  une : `exec3` et `exec4` se sont recouvertes de 2 min 23 s, la seconde a tué
  l'agent de la première **en pleine mesure**, et **le journal versé sous le nom
  de la première était celui de la seconde** — même horodatage de début à la
  microseconde près. `Get-Process agent` se revérifie **après chaque tentative,
  y compris échouée**, et chaque journal est copié **sous un nom unique
  immédiatement**.
- **Aucun taux ne sera revendiqué.** **Deux exécutions par critère**, jamais
  une, et chaque énoncé porte son nombre.
- **Toute preuve d'une affirmation portée dans `CLAUDE.md` est versée dans
  git**, sous `docs/superpowers/plans/journaux-pont-fichiers-f2/`. D9 a perdu
  **six** constats de revue parce que sa preuve vivait dans un espace gitignoré.

---

## 2. Structure des fichiers

### 2.1 Créés

| Fichier | Nature | Visé | Responsabilité |
| --- | --- | --- | --- |
| `agent/src/pont/projfs/rappels/listage.rs` | `#[cfg(windows)]` | ≤ 200 | **extraction** — les trois rappels d'énumération |
| `agent/src/pont/projfs/rappels/notification.rs` | `#[cfg(windows)]` | ≤ 250 | **extraction puis addition** — le rappel de notification et les POST de F2 |
| `agent/src/pont/journal.rs` | **PUR** | ≤ 300 | Le journal de reprise : ajout, retrait, relecture tolérante à la troncature |
| `agent/src/pont/journal/tests.rs` | test | ≤ 250 | |
| `agent/src/pont/ecriture.rs` | **PUR** | ≤ 350 | La file des écritures dues, la coalescence par chemin, l'état d'une poussée |
| `agent/src/pont/ecriture/tests.rs` | test | ≤ 300 | |
| `agent/src/pont/ecriture/fil.rs` | **PUR** | ≤ 300 | Le fil d'écriture : lit le fichier local, découpe, pousse, attend `Fait` |
| `agent/src/pont/ecriture/fil/tests.rs` | test | ≤ 250 | tests d'hôte sur un répertoire temporaire réel |
| `client/src/fichiers/ecriture.ts` | **PUR** | ≤ 300 | `ecrire`/`creer` côté navigateur, la garde de casse, les flux ouverts |
| `client/src/fichiers/ecriture.test.ts` | test | ≤ 300 | faux système de fichiers en mémoire |
| `docs/superpowers/plans/2026-08-20-pont-fichiers-f2-resultats.md` | document | — | Le document de résultats **permanent** |
| `docs/superpowers/plans/journaux-pont-fichiers-f2/` | journaux | — | Les pièces versées, avec leur `LISEZ-MOI.md` |

### 2.2 Modifiés — tailles **RELEVÉES PAR LA COMMANDE le 20 août 2026**

| Fichier | Lignes | Marge | Ce que F2 y ajoute |
| --- | --- | --- | --- |
| `agent/src/pont/projfs/rappels.rs` | 🔴 **488** | **12** | **RIEN avant la tâche 3.** Après extraction : ≈ **330**, marge ≈ 170 |
| `agent/src/pont/service.rs` | **325** | 175 | les bras `Fait` et `Echec` d'une écriture ; le câblage du fil d'écriture |
| `agent/src/pont/projfs/etat.rs` | **267** | 233 | `demander_avec_charge`, et le canal vers le fil d'écriture |
| `agent/src/pont/table.rs` | **174** | 326 | une inscription **sans** `command_id` ProjFS ; `DELAI_ECRIRE` |
| `agent/src/pont/erreurs.rs` | **163** | 337 | rien de neuf — `DisquePlein` et `DejaPresent` cessent d'être `dead_code` |
| `proto/src/fichiers.rs` | **157** | 343 | `TYPE_ECRIRE`, `TYPE_CREER`, `TYPE_DUES`, `TYPE_FAIT` ; deux `CodeEchec` |
| `agent/src/pont/notifications.rs` | **134** | 366 | le masque de F2, et la décision **par état** |
| `proto/src/fichiers/entetes.rs` | **109** | 391 | `Ecrire`, `Creer`, `Dues` |
| `agent/src/pont.rs` | **151** | 349 | le lancement du fil d'écriture, et la relecture du journal |
| `proto/ts/fichiers.ts` | **181** | 319 | le jumeau des quatre types |
| `proto/ts/fichiers-entetes.ts` | **208** | 292 | le jumeau des trois en-têtes |
| `proto/fichiers-vectors.json` | — | — | les vecteurs des trois formes neuves |
| `client/src/fichiers/adaptateur.ts` | **289** | 211 | **deux méthodes seulement**, qui délèguent à `ecriture.ts` |
| `client/src/fichiers/protocole.ts` | **128** | 372 | servir `TYPE_ECRIRE`/`TYPE_CREER`, **annoncer** `TYPE_DUES` |
| `client/src/fichiers/canal.ts` | **213** | 287 | `mode: 'readwrite'` (`:196`) |
| `client/src/shell.ts` | **154** | 346 | `ecrituresDues(n, chemins)`, `ecritureEchouee(chemin, motif)` |
| `client/src/shell-page.ts` | **221** | 279 | le câblage du compteur et le `beforeunload` |
| `client/shell.html` | — | — | la zone du compteur, **avec son `data-dues`** |
| `scripts/run-agent.sh` | **126**\* | — | +1 ligne `PONT_ECRITURE` |
| `CLAUDE.md` | — | — | tâche 16 |

\* *valeur du plan de F1, non réexécutée ; la ligne `PONT` y est relevée à 33.*

> 🔴 **`agent/src/pont/projfs/rappels.rs` À 488 LIGNES EST LE SEUL BLOCAGE DUR
> DE CE PLAN.** Marge **12**. Le rappel `notification` (`:407-438`) est
> exactement ce que F2 fait grossir : il doit lire `isdirectory`
> (`:409`, aujourd'hui `_est_repertoire`), l'union `PRJ_NOTIFICATION_PARAMETERS`
> (`:412`, aujourd'hui `_parametres`), le nom de destination, et aiguiller
> **cinq** notifications de plus. C'est ~80 lignes. **L'extraction se place
> AVANT** — c'est le seul geste qui ait fonctionné dans ce dépôt (D9,
> `capteur/serveur/instances.rs`, marge rendue de 10 à 65).

> ⚠️ **Marges étroites du VOISINAGE, relevées par la commande, qu'aucun document
> ne rapproche de ce sous-projet** : `agent/src/superviseur/lanceur.rs` **488**
> (marge 12 — *la spec §9 le publie à 367, marge 133 ; il a grossi de 121 lignes
> depuis, hors F1*), `agent/src/demarrage.rs` **491** (marge 9),
> `agent/src/pont/transport/tests.rs` **474** (marge 26). **F2 ne touche aucun
> des trois**, et la tâche 16 vérifiera qu'il ne les a pas touchés.

### 2.3 Où les extractions vont, et pourquoi ces noms

La spec §9 écrit : « s'il approche 500, **les rappels d'énumération partent dans
`projfs/enumeration.rs`**, jamais par compression ». **Deux choses ont changé
depuis** :

1. les rappels ne vivent plus dans `projfs.rs` mais dans `projfs/rappels.rs`
   (extrait par la tâche 13 de F1) : le point de chute est donc
   `projfs/rappels/…` ;
2. **`agent/src/pont/enumeration.rs` existe déjà**, et il est **PUR** (la
   session d'énumération, 140 lignes). Un `projfs/rappels/enumeration.rs`
   créerait deux modules homonymes dont l'un est pur et l'autre gaté — le genre
   d'homonymie qu'on ne remarque qu'en relisant un renvoi.

**D'où `projfs/rappels/listage.rs`**, et non `enumeration.rs`. La décision est
écrite dans l'en-tête du fichier extrait.

---

## 3. Interfaces partagées

**Fixées ici** ; les tâches les consomment telles quelles.

### 3.1 Le protocole — quatre types de plus, et une TROISIÈME famille

```
Requêtes pont → navigateur (attendent une réponse)
  TYPE_LISTER   = 1   (F1)
  TYPE_ATTRIBUTS= 2   (F1)
  TYPE_LIRE     = 3   (F1)
  TYPE_ECRIRE   = 4   (F2)   en-tête Ecrire, CHARGE = les octets
  TYPE_CREER    = 5   (F2)   en-tête Creer,  charge vide

Annonces pont → navigateur (n'attendent RIEN)
  TYPE_DUES     = 6   (F2)   en-tête Dues,   charge vide

Réponses navigateur → pont
  TYPE_ENTREES  = 64  (F1)
  TYPE_META     = 65  (F1)
  TYPE_DONNEES  = 66  (F1)
  TYPE_FAIT     = 67  (F2)   en-tête vide {}
  TYPE_ECHEC    = 127 (F1)
```

> ⚠️ **`TYPE_DUES` casse l'invariant que `client/src/fichiers/protocole.ts:17-21`
> énonce en majuscules — « UNE REQUÊTE REÇOIT TOUJOURS UNE RÉPONSE ».** C'est
> une **annonce**, pas une requête : aucune entrée de table ne lui correspond
> côté pont, et n'y répondre ne laisse donc rien en vol. **L'invariant doit être
> RÉÉCRIT, pas contourné** : « toute REQUÊTE reçoit une réponse ; une ANNONCE
> n'en reçoit aucune, et la liste des annonces est close ». Un bras qui rendrait
> `null` sans que le commentaire nomme la famille serait exactement le bras
> catch-all silencieux que ce dépôt a payé **quatre fois** sur
> `capteur/pont_media.rs`.

En-têtes (`proto/src/fichiers/entetes.rs` + `proto/ts/fichiers-entetes.ts`,
**épinglés par `proto/fichiers-vectors.json`, lu des DEUX côtés**) :

```rust
pub struct Ecrire {
    pub chemin: String,
    pub position: u64,
    pub longueur: u32,
    /// Premier morceau : le flux s'ouvre SANS `keepExistingData`.
    pub premier: bool,
    /// Dernier morceau : le flux se ferme, et c'est la COMMITTAISON.
    pub dernier: bool,
}
pub struct Creer { pub chemin: String, pub repertoire: bool }
pub struct Due   { pub chemin: String, pub octets: u64 }
pub struct Dues  { pub dues: Vec<Due> }
// TYPE_FAIT n'a pas d'en-tête propre : `{}`.
```

Deux `CodeEchec` de plus, **kebab-case sur le fil** :
`DisquePlein` → `"disque-plein"`, `DejaPresent` → `"deja-present"`.

> ⚠️ **`agent/src/pont/service.rs:291-306` (`cause_de`) est un `match`
> EXHAUSTIF** : ajouter deux variantes à `CodeEchec` **casse la compilation**
> tant qu'elles ne sont pas classées. C'est le garde, et il est gratuit.

### 3.2 Côté agent

```rust
// pont/journal.rs — PUR
pub struct Journal { /* … */ }
impl Journal {
    /// Relit un journal, en TOLÉRANT une dernière ligne tronquée.
    pub fn relire(contenu: &str) -> (Self, usize /* lignes ignorées */);
    pub fn inscrire(&mut self, chemin: &str, octets: u64) -> String; // la ligne à AJOUTER
    pub fn retirer(&mut self, chemin: &str) -> String;              // idem
    pub fn dues(&self) -> Vec<(String, u64)>;                        // ordre d'inscription
    pub fn compte(&self) -> usize;
}

// pont/ecriture.rs — PUR
pub enum Evenement { Modifie { chemin: String }, Cree { chemin: String, repertoire: bool } }
pub struct File { /* … */ }
impl File {
    pub fn signaler(&mut self, e: Evenement) -> Option<Evenement>; // None si déjà en vol → marque `a_rejouer`
    pub fn terminee(&mut self, chemin: &str) -> Option<Evenement>; // rend l'événement à rejouer, s'il y en a un
    pub fn en_vol(&self) -> Option<&str>;
}

// pont/ecriture/fil.rs — PUR
pub fn tourner(
    racine: std::path::PathBuf,
    journal_chemin: std::path::PathBuf,
    evenements: std::sync::mpsc::Receiver<Evenement>,
    vers_navigateur: std::sync::mpsc::Sender<crate::pont::transport::VersNavigateur>,
    faits: std::sync::mpsc::Receiver<Fait>,
    armee: bool,        // PONT_ECRITURE
);
pub enum Fait { Ok { correlation: u32 }, Echec { correlation: u32, code: proto::fichiers::CodeEchec } }
```

> ⚠️ **`VersNavigateur::Requete { correlation, trame }`
> (`agent/src/pont/transport.rs:42-44`) porte DÉJÀ une trame complète : le fil
> d'écriture n'a donc RIEN à ajouter au transport.** `agent/src/pont/transport.rs`
> (290) et `agent/src/pont/transport/tests.rs` (**474**, marge 26) **ne sont
> touchés par aucune tâche de ce plan**, et la tâche 16 le vérifie par
> `git diff --stat`.

### 3.3 `Table` — une inscription sans commande ProjFS

Une écriture n'est **la complétion d'aucun rappel** : elle naît d'une
notification POST, qui a déjà rendu. Il n'y a donc pas de `command_id` à
compléter, **mais il faut une corrélation, et elle doit venir de la même table**
— une seconde source de corrélations sur le même canal se collisionnerait avec
la première.

```rust
// EnVol.command_id : i32  →  Option<i32>
pub fn inscrire_sans_commande(&mut self, quoi: Attendue, echeance: Instant) -> u32;
pub fn resoudre(&mut self, c: u32) -> Option<(Option<i32>, Attendue)>;
pub fn expirees(&mut self, t: Instant) -> Vec<(Option<i32>, u32)>;
pub fn vider(&mut self) -> Vec<(Option<i32>, u32)>;
```

`service::terminer` et `verbes::completer` ignorent un `None` **en le
journalisant à `debug!`**, jamais en silence.

Et `DELAI_ECRIRE` rejoint les trois budgets de `agent/src/pont/table.rs:38-40` —
`table.rs:37` l'annonce déjà : « **Le quatrième budget, celui de l'écriture,
appartient à F2** ». Valeur proposée **30 s** par morceau, **NON CALIBRÉE**, et
sa documentation le dira, aux côtés de `TAILLE_TRAME_MAX`, `DELAI_ATTRIBUTS`,
`DELAI_LIRE`, `DELAI_LISTER`, `BPP_MIN`, `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`,
`HYSTERESIS`, `REPIT_APRES_ECHEC`, `TAILLE_MAX_SORTIE`,
`REPIT_REARMEMENT_AUDIO` et `REARMEMENTS_MAX`.

### 3.4 Côté navigateur

```ts
// client/src/fichiers/ecriture.ts — PUR
export interface Ecrivain {
  ecrire(chemin: string, position: number, octets: Uint8Array,
         premier: boolean, dernier: boolean): Promise<void>;
  creer(chemin: string, repertoire: boolean): Promise<void>;
  /** Ferme tout flux resté ouvert. Appelé à la fermeture du canal. */
  abandonner(): void;
}
export function creerEcrivain(racine: RacineInscriptible): Ecrivain;
```

`RacineInscriptible` étend `PoigneeRepertoire` de
`client/src/fichiers/adaptateur.ts:89-94` avec
`getDirectoryHandle(nom, {create})`, `getFileHandle(nom, {create})` et
`createWritable()`, **décrits comme un sous-ensemble structurel** — la vraie
poignée les satisfait sans conversion, et `canal.ts` le vérifie à la
compilation (`canal.ts:201-211`), exactement comme F1.

> 🔵 **PROPRIÉTÉ D'ATOMICITÉ, à écrire dans le code et à ne pas surestimer.**
> `createWritable()` écrit dans un fichier d'échange et **ne commet qu'au
> `close()`**. Une poussée interrompue en plein vol laisse donc le fichier local
> **inchangé** : le write-back est **tout ou rien par fichier**. C'est
> excellent — pas de fichier à moitié écrit chez l'utilisateur — et cela a un
> revers : *une interruption ne rend RIEN, pas même le début*. ⚠️ **C'est une
> inférence de la spécification de la File System Access API, non mesurée ici**,
> et le critère ⑤ de la recette est écrit pour l'éprouver.

---

# Famille A — le protocole, et le seul geste qui doit précéder tout

### Task 1 : `proto/{src,ts}/fichiers` — les quatre types, les trois en-têtes, les deux codes

**Objet** — donner au protocole les verbes d'écriture, et les épingler des deux
côtés par des vecteurs partagés.

**Fichiers** — `proto/src/fichiers.rs`, `proto/src/fichiers/entetes.rs`,
`proto/src/fichiers/tests.rs`, `proto/src/fichiers/entetes/tests.rs`,
`proto/ts/fichiers.ts`, `proto/ts/fichiers-entetes.ts`,
`proto/ts/fichiers.test.ts`, `proto/ts/fichiers-entetes.test.ts`,
`proto/fichiers-vectors.json`.

**Ce qui est écrit** — les quatre constantes de type du §3.1, les structures
`Ecrire`/`Creer`/`Due`/`Dues`, les deux variantes de `CodeEchec`, et **une
entrée par forme dans `proto/fichiers-vectors.json`**, avec sa chaîne JSON
**exacte**.

⚠️ **`FICHIERS_VERSION` reste à 1.** F1 l'a choisi ainsi *précisément* pour que
l'arrivée de ces verbes soit une rupture visible (F1 résultats §13, legs 13) —
mais la rupture est **additive** : un pont v1-lecture-seule et un client
v1-écriture s'entendent, le client ignorant simplement des types qu'il ne
recevra jamais. **Incrémenter à 2 casserait la compatibilité dans le seul sens
où elle n'a aucune valeur** (les deux bouts sont livrés ensemble) et ferait
échouer une session en cours de migration. **Décision : 1, et la raison écrite
dans le code.**

**Tests, et leur ROUGE :**

| Test | Rouge |
| --- | --- |
| `un_code_d_echec_a_une_forme_epinglee_sur_le_fil` (existant, étendu à 9) | renommer `DisquePlein` en `Plein` sans toucher le vecteur → le test échoue **côté Rust seul**, ce qui est tout l'intérêt du vecteur partagé |
| `les_neuf_formes_ont_leur_vecteur` (neuf, Rust **et** TS) | ajouter `Ecrire` sans son entrée dans `fichiers-vectors.json` → **les deux** rendent rouge |
| `un_type_de_message_neuf_est_dans_TYPES_CONNUS` (TS) | ajouter `TYPE_ECRIRE` à l'union `TypeMessage` sans l'ajouter à `TYPES_CONNUS` → **`tsc --noEmit` refuse**, `Record<TypeMessage, true>` exigeant une entrée par membre (`proto/ts/fichiers.ts:59`) |
| `une_trame_ecrire_pleine_passe` | charge de `TAILLE_TRAME_MAX` exactement + en-tête `Ecrire` → décodage juste. Rouge : borner la charge par `TAILLE_TRAME_MAX - taille_entete` |

**Vérification de l'atteignabilité du rouge** : les quatre états ci-dessus sont
des éditions d'une ligne, à faire puis défaire, et la sortie du test est versée
dans `journaux-pont-fichiers-f2/f2-tache1-rouges.txt`.

**Contrôles** :
```bash
cd agent  && cargo test -p proto
cd client && npx vitest run --dir ../proto   # OBLIGATOIRE, voir §1.2
cd client && npx tsc --noEmit
```

**Dépendances** — aucune. **Première tâche.**

---

### Task 2 : `scripts/run-agent.sh` transmet `PONT_ECRITURE` — tâche DÉDIÉE

**Objet** — la seule ligne sans laquelle le rouge du compteur d'écritures dues
est impossible à provoquer sur la VM.

**Fichiers** — `scripts/run-agent.sh` (une ligne, à côté de la ligne 33 qui
porte `PONT`).

**Pourquoi une tâche à elle seule** : `SUPERVISEUR` (D1),
`MULTIFENETRE_REPRISE` (D2) et `AUDIO` (D7) ont tous été oubliés, et **l'agent
démarre alors sans la variable et sans rien signaler**. D3, D6 et F1 ont évité
le piège par exactement ce geste.

**Preuve, versée** — le lancement d'un agent avec `PONT_ECRITURE=0` doit faire
apparaître au journal la trace de désarmement de la tâche 11. **Tant que la
tâche 11 n'existe pas, la preuve se fait par
`grep -n PONT_ECRITURE scripts/run-agent.sh` et par la relecture du `.ps1`
généré** (le script l'écrit sur `/media/vm/dev/`) : la ligne
`$env:PONT_ECRITURE = '0'` doit y figurer. **Rouge** : retirer la ligne du
script → elle n'apparaît pas dans le `.ps1`. Versé dans
`f2-tache2-pont-ecriture-transmise.txt`.

**Dépendances** — aucune. **À faire tôt, avant la tâche 11.**

---

# Famille B — l'extraction, avant toute addition

### Task 3 : extraire `listage.rs` et `notification.rs` de `rappels.rs` — AVANT d'y ajouter quoi que ce soit

**Objet** — rendre à `agent/src/pont/projfs/rappels.rs` (**488**, marge **12**)
la place que F2 va prendre, **par extraction et jamais par compression**.

**Fichiers créés** — `agent/src/pont/projfs/rappels/listage.rs`,
`agent/src/pont/projfs/rappels/notification.rs`.
**Fichier modifié** — `agent/src/pont/projfs/rappels.rs`.

**Ce qui part, exactement** (bornes relevées par la commande le 20 août 2026) :

| Vers | Ce qui bouge | Lignes actuelles |
| --- | --- | --- |
| `listage.rs` | `debut_enumeration` (`:153`), `fin_enumeration` (`:173`), `suite_enumeration` (`:194`), **et leurs trois `const _`** de `:61-63` | ≈ 115 |
| `notification.rs` | `notification` (`:407-438`), **et son `const _`** de `:67` | ≈ 50 |

🔴 **Les `const _: PRJ_*_CB = Some(…)` partent AVEC leur fonction.** C'est le
**seul garde d'ABI de ce dépôt** (`rappels.rs:47-68`), et le laisser derrière
en ferait une déclaration qui pointe ailleurs. D9 a posé la règle en extrayant
`capteur/serveur/instances.rs` : *le commentaire part avec sa constante*, et la
revue a comparé mot pour mot.

`garde`, `etat`, `chemins_de` et `identifiant` deviennent `pub(super)` et sont
importées par les deux enfants. `bloc()` (`:477-488`) reste dans le parent et
référence les fonctions des enfants.

**Ce que la tâche n'est PAS** : elle **n'ajoute aucun comportement**. La
transposition est **verbatim**, et la revue la compare caractère pour caractère.

**Contrôle, et son ROUGE :**

```bash
cd agent && cargo check --target x86_64-pc-windows-gnu   # sortie 0
wc -l agent/src/pont/projfs/rappels.rs                   # attendu : ≈ 330, à RELEVER
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
```

⚠️ **Le nombre attendu ci-dessus est une PRÉVISION, pas un relevé** : la tâche
inscrit dans son rapport le nombre **mesuré**, et si l'écart dépasse 20 lignes
c'est que la transposition n'a pas été verbatim.

**Rouge** — retirer un `const _` déplacé et changer un paramètre de la fonction
correspondante : `cargo check --target x86_64-pc-windows-gnu` doit **cesser** de
le voir (c'est ce qui prouve que le garde a bien suivi la fonction, et non
qu'il pointe encore sur l'ancienne). Puis le remettre. Versé dans
`f2-tache3-garde-abi.txt`.

**Dépendances** — aucune, mais **elle bloque les tâches 4 et 10**.

---

# Famille C — les modules PURS de l'agent

### Task 4 : `pont/notifications.rs` — le masque de F2, et la décision par ÉTAT

**Objet** — passer de « tout refuser » à « refuser sur un état, accepter le
reste, et nommer les POST qui déclenchent une poussée ».

**Fichiers** — `agent/src/pont/notifications.rs`,
`agent/src/pont/notifications/tests.rs`.

**Le masque, de cinq bits à huit** :

```rust
// Les DEUX constantes neuves, avec leur ligne source — relevées par la commande
// le 20 août 2026 dans windows-0.62.2/.../ProjectedFileSystem/mod.rs (621 lignes).
pub const FILE_OVERWRITTEN: i32 = 8;                    // mod.rs:339  (PRJ_NOTIFICATION)
pub const FILE_HANDLE_CLOSED_FILE_MODIFIED: i32 = 1024; // mod.rs:336  (PRJ_NOTIFICATION)
pub const NOTIFY_FILE_OVERWRITTEN: u32 = 8;                    // mod.rs:384 (PRJ_NOTIFY_TYPES)
pub const NOTIFY_FILE_HANDLE_CLOSED_FILE_MODIFIED: u32 = 1024; // mod.rs:381 (PRJ_NOTIFY_TYPES)

pub const MASQUE: u32 = NOTIFY_FILE_PRE_CONVERT_TO_FULL   // 4096, mod.rs:385 — gate REFUSABLE
    | NOTIFY_PRE_RENAME                                   //   32, mod.rs:391 — refusée (F3)
    | NOTIFY_PRE_DELETE                                   //   16, mod.rs:390 — refusée (F3)
    | NOTIFY_PRE_SET_HARDLINK                             //   64, mod.rs:392 — refusée (hors périmètre)
    | NOTIFY_NEW_FILE_CREATED                             //    4, mod.rs:388 — POST → Creer
    | NOTIFY_FILE_OVERWRITTEN                             //    8, mod.rs:384 — POST → poussée
    | NOTIFY_FILE_HANDLE_CLOSED_FILE_MODIFIED;            // 1024, mod.rs:381 — POST → poussée
```

⚠️ **Les deux familles ne sont PAS interchangeables**, et
`notifications.rs:54-59` le dit déjà : `PRJ_NOTIFICATION_*` (`i32`) est ce que
le rappel **reçoit**, `PRJ_NOTIFY_*` (`u32`) est ce que le **masque** demande.
Elles ont ici les mêmes valeurs numériques, mais ce sont deux types distincts
dans windows-rs, et rien ne garantit qu'elles resteront alignées.

⚠️ **`FILE_HANDLE_CLOSED_NO_MODIFICATION` (512) n'est PAS demandée**, et la
raison s'écrit : elle arriverait **à chaque fermeture de handle en lecture**,
c'est-à-dire sur le chemin le plus chaud du pont, pour n'apprendre que ce qu'on
sait déjà (rien à pousser). Le coût est certain, le gain nul. *Décision, pas
oubli.*

⚠️ **`HARDLINK_CREATED` (256) reste demandée et refusée**, comme en F1. Elle est
POST : le refus n'empêche rien, il **journalise**. Le dire, plutôt que de
laisser croire qu'un lien dur est empêché.

**La décision devient une fonction de l'état** :

```rust
pub struct Etat { pub inscriptible: bool, pub canal_ouvert: bool }
pub enum Reponse {
    Refuser(Erreur),
    Pousser(Poussee),         // NEUF — la notification déclenche un write-back
    AccepterEnSignalant,
    AccepterSansAttendre,
}
pub enum Poussee { Contenu, Creation }
pub fn decider(code: i32, etat: Etat) -> Reponse;
```

- `PRE_CONVERT_TO_FULL` : `Refuser(ProtegeEnEcriture)` si `!inscriptible` ;
  `Refuser(CanalFerme)` si `!canal_ouvert` ; **sinon `AccepterSansAttendre`**.
  🔵 **C'est l'unique porte de refus d'une écriture, et c'est la seule ligne de
  ce plan qui change ce que l'application obtient.**
- `PRE_RENAME`, `PRE_DELETE` : `Refuser(ProtegeEnEcriture)` **inconditionnel**
  — voir §0.2.
- `PRE_SET_HARDLINK`, `HARDLINK_CREATED` : `Refuser(NonSupporte)`, inchangé.
- `NEW_FILE_CREATED` : `Pousser(Creation)`.
- `FILE_OVERWRITTEN`, `FILE_HANDLE_CLOSED_FILE_MODIFIED` : `Pousser(Contenu)`.
- le reste : `AccepterSansAttendre`.

**Tests, et leur ROUGE** — les cinq existants sont **réécrits**, pas étendus,
et deux d'entre eux sont le garde de cette tâche :

| Test | Rouge, et pourquoi il est atteignable |
| --- | --- |
| `le_masque_demande_exactement_les_huit_notifications_de_f2` (existant `:67`, `count_ones() == 5` → `8`) | **il est ROUGE avant la modification** : le masque actuel a 5 bits. *Ce test ne peut pas être vacueux — il l'est déjà à l'inverse.* |
| `chaque_bit_du_masque_a_une_decision_nommee` (existant `:47`) | ajouter un bit au masque sans bras dans `decider` → le bit retombe dans le fourre-tout, `assert_ne!` échoue |
| `une_ecriture_sur_racine_non_inscriptible_est_refusee` | poser `inscriptible: true` → le refus disparaît |
| `une_ecriture_sur_canal_ferme_est_refusee_en_erreur_d_e_s` | rendre `ProtegeEnEcriture` au lieu de `CanalFerme` → l'assertion sur le code échoue. ⚠️ **Deux causes distinctes ne partagent jamais un code** (spec §5.1) |
| `un_renommage_reste_refuse_en_f2` | rendre `AccepterSansAttendre` → échoue. **C'est le garde de la décision §0.2** |
| `les_deux_post_de_contenu_declenchent_une_poussee` | oublier `FILE_OVERWRITTEN` → échoue |

**Dépendances** — tâche 3 (le rappel qui l'appelle vit désormais dans
`notification.rs`).

---

### Task 5 : `pont/table.rs` — une commande sans `command_id` ProjFS

**Objet** — permettre à une écriture, qui ne complète aucun rappel, d'obtenir
une corrélation **de la même table** que les lectures.

**Fichiers** — `agent/src/pont/table.rs`, `agent/src/pont/table/tests.rs`, et
les appelants : `agent/src/pont/service.rs`, `agent/src/pont/projfs/etat.rs`,
`agent/src/pont/projfs.rs` (le `Drop`), `agent/src/pont/service/verbes.rs`.

**Ce qui est écrit** — le §3.3, plus `Attendue::Ecrire { chemin, dernier }` et
`Attendue::Creer { chemin }`, plus `DELAI_ECRIRE`.

⚠️ **Pourquoi PAS une seconde source de corrélations** : le canal est unique et
la corrélation est un `u32` monotone avec recherche d'un libre
(`table.rs:110-118`). Deux compteurs indépendants sur le même canal se
collisionneraient, et **la collision serait silencieuse** — une réponse
appliquée à la mauvaise commande. C'est le défaut que `table.rs:104-109`
documente déjà contre le rebouclage.

**Tests, et leur ROUGE :**

| Test | Rouge |
| --- | --- |
| `une_inscription_sans_commande_recoit_une_correlation_unique` | faire rendre `0` à `inscrire_sans_commande` → collision avec une lecture en vol |
| `une_ecriture_et_une_lecture_ne_partagent_jamais_une_correlation` | inscrire les deux depuis deux compteurs → l'assertion d'unicité échoue |
| `vider_rend_les_ecritures_avec_un_command_id_absent` | rendre `Some(0)` au lieu de `None` → `verbes::completer` appellerait `PrjCompleteCommand(0)` |
| `une_ecriture_expiree_est_retiree_comme_les_autres` | exclure les `None` de `expirees` → l'écriture reste en table pour toujours |

**Dépendances** — aucune (module pur), mais **elle précède les tâches 11 et 12**.

---

### Task 6 : `pont/journal.rs` — le journal de reprise, PUR

**Objet** — l'ensemble des écritures dues, persisté **hors de la racine**, et
relu au démarrage.

**Fichiers** — `agent/src/pont/journal.rs`, `agent/src/pont/journal/tests.rs`,
`agent/src/pont.rs` (`pub mod journal;`).

**La forme, et pourquoi celle-là.** Un fichier **en AJOUT SEUL**, une ligne par
événement, encodée pour survivre à une troncature :

```
+<octets> <chemin JSON>\n     inscription
-<chemin JSON>\n              retrait
```

- **Le chemin est encodé en JSON** (`serde_json::to_string(&chemin)`) : il peut
  porter des espaces, des accents et — sur un poste local non-Windows — un
  saut de ligne. Un séparateur naïf casserait sur le nom accentué avec espace
  que F1 mesure déjà (`éphémère été.txt`).
- **Ajout seul, et jamais de réécriture en place** : une réécriture interrompue
  perdrait les entrées **antérieures**, ce que la spec §4.4 désigne
  nommément comme le rouge de ce module.
- **La dernière ligne peut être tronquée**, et `relire` la **jette en la
  comptant**. Une ligne partielle est le seul dommage qu'un arrêt brutal peut
  causer à un fichier en ajout.
- **Compactage** : quand le journal est vide (aucune due) **et** que le fichier
  dépasse `TAILLE_JOURNAL_COMPACTAGE`, il est **tronqué à zéro**. Jamais quand
  il reste une due. ⚠️ **Constante NON CALIBRÉE.**

⚠️ **Où il vit** — `%LOCALAPPDATA%\Guacamole\pont\ecritures.journal`. Le dossier
existe déjà : `agent/src/pont/projfs/racine.rs:36-37,48-52`. **`dossier_etat()`
y est privée** (`fn`, `:48`) et devient `pub(super)`, puis est ré-exportée par
`projfs`. **Le module `journal.rs` reste PUR** : il ne connaît qu'un
`&Path` qu'on lui donne, et c'est `pont::executer` — déjà `#[cfg(windows)]` —
qui le résout.

**Tests, et leur ROUGE :**

| Test | Rouge, atteignable comment |
| --- | --- |
| `une_derniere_ligne_tronquee_ne_fait_pas_perdre_les_precedentes` | relire un contenu se terminant par `+42 "note` → faire lever `relire` plutôt que jeter, ou faire perdre les entrées d'avant |
| `un_retrait_efface_l_inscription_et_pas_une_autre` | retirer par préfixe → deux chemins voisins s'effacent l'un l'autre |
| `l_ordre_d_inscription_est_conserve` | employer un `HashMap` sans ordre → `dues()` rend un ordre différent à chaque exécution |
| `un_chemin_a_saut_de_ligne_survit_a_un_aller_retour` | encoder le chemin brut → la relecture le coupe en deux lignes |
| `un_journal_vide_se_compacte_et_un_journal_non_vide_JAMAIS` | compacter inconditionnellement → une due est perdue **exactement quand elle sert** |

⚠️ **Le quatrième test n'est pas une coquetterie** : la File System Access API
tourne dans un navigateur qui peut être sur macOS ou Linux, où `\n` est un
caractère de nom de fichier licite.

**Dépendances** — aucune.

---

### Task 7 : `pont/ecriture.rs` — la file des écritures dues, PURE

**Objet** — décider *quoi* pousser, *dans quel ordre*, et *ce qu'on fait d'une
notification qui arrive pendant une poussée*.

**Fichiers** — `agent/src/pont/ecriture.rs`,
`agent/src/pont/ecriture/tests.rs`, `agent/src/pont.rs`.

**Les règles, toutes pures :**

1. **Une poussée en vol par chemin, au plus.** `signaler` d'un chemin déjà en
   vol rend `None` et pose `a_rejouer`.
2. **À la fin d'une poussée, `terminee` rend l'événement à rejouer** s'il y en
   a un : le fichier est **relu depuis le début**, jamais poussé en deux
   morceaux d'époques différentes.
3. **Ordre FIFO d'inscription** entre chemins distincts.
4. **Une création de RÉPERTOIRE ne porte aucun contenu** : `Creer` seul, aucune
   lecture, aucun morceau.
5. **Une création de FICHIER est suivie, en général, d'une notification de
   contenu.** F2 pousse donc `Creer` puis, à la fermeture du handle, le
   contenu. Un fichier créé et jamais écrit reste vide des deux côtés — ce qui
   est juste.

**Tests, et leur ROUGE :**

| Test | Rouge |
| --- | --- |
| `deux_notifications_du_meme_chemin_ne_font_qu_une_poussee_en_vol` | laisser passer la seconde → deux flux `createWritable` concurrents sur le même fichier |
| `une_notification_pendant_une_poussee_est_rejouee_apres` | jeter la seconde → **les derniers octets écrits par l'utilisateur sont perdus**, silencieusement |
| `un_repertoire_cree_ne_produit_aucun_morceau` | lui faire lire un fichier → `IsADirectory` |
| `l_ordre_entre_chemins_distincts_est_celui_d_inscription` | employer un `HashSet` |

**Dépendances** — aucune.

---

### Task 8 : `pont/ecriture/fil.rs` — le fil d'écriture, PUR et testé sur l'hôte

**Objet** — lire le fichier local, le découper, pousser les trames, attendre le
`Fait`, tenir le journal, et annoncer les dues.

**Fichiers** — `agent/src/pont/ecriture/fil.rs`,
`agent/src/pont/ecriture/fil/tests.rs`.

**🔵 Pourquoi ce module est PUR, alors qu'il lit un fichier « de ProjFS ».**
Après `FILE_HANDLE_CLOSED_FILE_MODIFIED`, le fichier est **complet** dans la
racine : ProjFS n'appelle `GetFileData` que sur un **substitut**. Le lire est
donc un `std::fs::File::open` **ordinaire**, portable, testable sur Linux avec
un répertoire temporaire réel. ⚠️ **C'est une INFÉRENCE du modèle de ProjFS, pas
une mesure** : si elle est fausse, la lecture ré-entre dans nos propres rappels.
**Elle ne provoquerait pas d'interblocage** — les commandes ainsi créées sont
complétées par le **fil du pont**, un fil distinct — mais le pont relirait ses
propres octets à travers le navigateur, ce qui serait visible au journal (des
`Lire` sur un chemin en cours d'écriture). **Le critère ④ de la recette existe
pour trancher.**

🔴 **Le fil est DÉDIÉ, et jamais celui du pont.**
`agent/src/pont/service.rs:13-17` l'écrit déjà pour le relevé d'hydratation :
« un `read_dir` sur la racine traverserait ProjFS, donc déclencherait nos
propres rappels d'énumération, qui inscrivent une commande que **ce fil-ci**
doit compléter : **il s'attendrait lui-même**. » **La même phrase vaut ici, et
c'est la raison d'être du fil.** Elle est recopiée en tête de `fil.rs`.

**La séquence, et l'ORDRE n'est pas négociable :**

1. `Evenement` reçu.
2. `journal.inscrire(chemin, octets)` — **écrit ET vidé (`sync_all`) sur le
   disque AVANT la première trame**. Une entrée poussée avant d'être journalisée
   est une entrée qu'un arrêt brutal perd.
3. `TYPE_DUES` annoncé au navigateur.
4. lecture du fichier local, `decoupe::decouper(0, taille, TAILLE_TRAME_MAX)`
   (`agent/src/pont/decoupe.rs:38`).
5. **un morceau en vol à la fois** — comme la lecture de F1
   (`agent/src/pont/service.rs:228-233`). Le contrôle de flux par
   `bufferedAmount`/`SEUIL_TAMPON` est un livrable de **F3**, et
   **l'implémenter à moitié ici serait pire que de ne pas l'implémenter**.
6. sur le `Fait` du **dernier** morceau : `journal.retirer(chemin)`, **puis**
   `TYPE_DUES` réannoncé.
7. sur un `Echec` ou une expiration : **l'entrée RESTE au journal**, un `warn!`
   nomme le chemin et le code, et la page-shell l'affiche.

**Au démarrage** : `journal.relire` ; s'il n'est pas vide, `TYPE_DUES` est
annoncé **avant** toute poussée, et les dues sont repoussées **dans l'ordre
d'inscription**. Une entrée dont le fichier local n'existe plus est **retirée
avec un `warn!` qui la nomme** : la racine a été recréée, le fichier est parti
avec elle (spec §6.4 cas 3).

**`PONT_ECRITURE=0`** : le fil **journalise et annonce** mais **ne pousse
jamais**. C'est le bras désarmé de l'A/B, et il rend le compteur rouge.
Trace, émise **seulement si désarmé** :
`warn!("poussee d'ecriture DESARMEE (PONT_ECRITURE=0) : bras de banc, jamais une configuration livrée")`.

**Tests d'hôte, et leur ROUGE** — sur un `tempfile::TempDir` réel :

| Test | Rouge |
| --- | --- |
| `le_journal_est_ecrit_avant_la_premiere_trame` | inverser les deux → le test observe une trame avant la ligne de journal |
| `l_entree_sort_du_journal_apres_le_dernier_fait_et_pas_avant` | retirer au premier `Fait` → un fichier de deux morceaux dont le second échoue sort du journal **en ayant perdu ses octets** |
| `un_echec_laisse_l_entree_au_journal` | la retirer → **c'est la perte de données que ce module existe pour empêcher** |
| `un_fichier_de_taille_nulle_produit_une_creation_et_zero_morceau` | `decouper` rend `[]` pour une longueur nulle (`decoupe.rs:29-31`) — sans le cas particulier, aucun `dernier` n'est jamais émis et l'entrée reste au journal **pour toujours** |
| `un_fichier_absent_au_redemarrage_sort_du_journal_en_le_nommant` | boucler sur le réessai → le pont repousse indéfiniment un fichier qui n'existe plus |
| `desarme_le_fil_journalise_mais_ne_pousse_rien` | ignorer la variable → le rouge du compteur devient impossible à provoquer |
| `un_morceau_en_vol_a_la_fois` | pousser tout d'un coup → la file SCTP est inondée, ce que F1 a déjà décidé d'éviter |

⚠️ **Le quatrième test est le contrôle le plus important de cette tâche** : un
fichier vide est le cas nominal d'un « nouveau document » enregistré aussitôt,
et `decouper` rend délibérément **zéro** morceau. Sans cas particulier, l'entrée
ne sortirait **jamais** du journal, le compteur ne redescendrait jamais, et
l'utilisateur verrait une alerte permanente pour un fichier correctement
transmis. *Un compteur qui ne redescend jamais est aussi faux qu'un compteur qui
ne monte jamais.*

**Dépendances** — tâches 1, 5, 6, 7.

---

# Famille D — le navigateur

### Task 9 : `client/src/fichiers/ecriture.ts` — écrire, créer, et la GARDE DE CASSE

**Objet** — poser les octets dans le répertoire du poste local, et **refuser
d'écrire dans un fichier qu'on n'a pas nommé**.

**Fichiers créés** — `client/src/fichiers/ecriture.ts`,
`client/src/fichiers/ecriture.test.ts`.
**Fichiers modifiés** — `client/src/fichiers/adaptateur.ts` (les deux méthodes
de l'interface, qui **délèguent**), `client/src/fichiers/adaptateur.test.ts`.

**🔴 CE QUE F2 FAIT DU DÉFAUT DE CASSE, devenu une PERTE DE DONNÉES.**

F1 a mesuré, **trois exécutions sur trois** : avec `Casse.txt` sur le poste
local, `casse.txt` **et** `CASSE.TXT` rendent le **contenu** de `Casse.txt`,
sans erreur — tandis que `GROS.BIN` rend « introuvable » dans la même exécution
(F1 résultats §7.1). En **lecture**, c'est un mauvais fichier rendu. **En
écriture, c'est un fichier écrasé.**

⚠️ **Le mécanisme est bien pire du côté navigateur que du côté VM**, et il faut
le voir précisément :

- Côté VM, l'écart de casse est **absorbé par NTFS** sur un fichier **déjà
  hydraté** : le pont reçoit alors la casse **réelle** de l'entrée locale, et la
  poussée est juste. *Ce chemin-là n'est pas dangereux.*
- Côté navigateur, `getFileHandle(nom, { create: true })` s'exécute sur le
  système de fichiers du **poste local**, qui est **insensible à la casse** sur
  Windows et sur macOS par défaut. Une poussée vers `CASSE.TXT` y ouvre donc
  **`Casse.txt`** et **l'écrase**. *C'est là que la perte se produit.*
- Le cas qui l'atteint : un fichier **jamais hydraté** créé dans la VM avec une
  casse différente d'une entrée locale existante. NTFS n'a rien à résoudre, la
  notification porte la casse de la VM, et la poussée écrase l'homonyme local.

**La règle de F2, PURE et testée :**

> **Avant toute écriture ou création, l'écrivain énumère le répertoire parent et
> cherche le nom demandé OCTET POUR OCTET.**
>
> - **nom exact trouvé** → on écrit dedans ;
> - **aucun nom trouvé, et aucun homonyme insensible à la casse** → on crée ;
> - **aucun nom exact, mais un homonyme qui ne diffère que par la casse** →
>   **`EchecFichiers('casse-ambigue')`, on n'écrit RIEN.**

⚠️ **`casse-ambigue` est une NEUVIÈME variante de `CodeEchec`**, et elle doit
être ajoutée à la tâche 1. Elle se traduit côté agent en
`Erreur::Inattendue` → `ERROR_GEN_FAILURE` — **et non en un code inventé** :
`agent/src/pont/erreurs.rs` n'a pas de variante pour cela, en créer une
appartiendrait à la table complète de **F3**, et la spec §5.1 interdit de faire
partager un code à deux causes distinctes. *L'application ne verra rien de
toute façon (§0.1 point 4)* ; ce qui compte est que **le journal et la
page-shell nomment le fichier et la cause**.

⚠️ **Coût, écrit** : une énumération du répertoire parent par écriture. Sur un
répertoire à mille entrées, c'est mille `getFile()` — le coût que
`adaptateur.ts:207-211` déclare déjà pour le listage. **Il est mesurable en F4,
pas ici.** L'alternative — la table de correspondance alimentée par
l'énumération, que F1 lègue à F3 — la supprimerait ; **F2 ne la construit pas**,
parce qu'un cache que rien n'invalide est le défaut de l'ancien pont
(`src/file.js`, cache **sans TTL**) et que `Rafraichir` est un livrable de F5.

**Ce que F2 NE fait PAS du défaut de casse** : il ne le **corrige** pas en
lecture. `casse.txt` continuera de rendre le contenu de `Casse.txt`. **La garde
ne protège que le sens ÉCRITURE**, qui est le seul où l'erreur détruit quelque
chose. Le remède complet reste **F3**.

**Le reste du module :**

- un **flux ouvert par chemin**, dans une `Map`, ouvert au morceau `premier`
  par `createWritable()` **sans `keepExistingData`** (spec §12 : l'ancien pont
  employait `keepExistingData: true` sans `truncate`, et **un fichier réécrit
  plus court conservait sa queue d'octets**), fermé au morceau `dernier` ;
- `abandonner()` ferme tout, appelé à la fermeture du canal
  (`canal.ts:104`) ;
- le classement des exceptions réemploie `classer` de `adaptateur.ts:130`,
  étendu : `QuotaExceededError` → `'disque-plein'`,
  `NotAllowedError`/`SecurityError` → `'acces-refuse'` (déjà),
  `InvalidModificationError`/`TypeMismatchError` → `'deja-present'`.

**Tests, et leur ROUGE** (faux système de fichiers en mémoire, **insensible à
la casse par construction**, pour reproduire le poste local) :

| Test | Rouge, atteignable comment |
| --- | --- |
| `ecrire_dans_un_homonyme_de_casse_est_refuse_et_n_ecrit_rien` | retirer la garde → **le faux montre `Casse.txt` écrasé**. *C'est le rouge le plus important de F2 : il doit être vu, et sa sortie versée.* |
| `ecrire_dans_le_nom_exact_reussit` | rendre la garde trop stricte → plus aucune écriture ne passe |
| `un_fichier_reecrit_plus_court_ne_garde_pas_sa_queue` | passer `keepExistingData: true` → la queue survit, exactement comme l'ancien pont |
| `un_flux_est_ouvert_une_fois_et_ferme_une_fois` | ouvrir par morceau → le faux compte deux ouvertures |
| `abandonner_ferme_les_flux_restes_ouverts` | ne rien faire → le faux voit un flux ouvert après |
| `un_quota_depasse_rend_disque_plein_et_pas_interne` | le laisser tomber dans le `default` de `classer` → `'interne'`, et le journal ne dit plus pourquoi |

**Dépendances** — tâche 1.

---

### Task 10 : `client/src/fichiers/protocole.ts` — servir `ECRIRE`/`CREER`, ANNONCER `DUES`

**Objet** — mettre les deux verbes sur le fil, et introduire la famille des
annonces **sans casser l'invariant, en le réécrivant**.

**Fichiers** — `client/src/fichiers/protocole.ts`,
`client/src/fichiers/protocole.test.ts`.

**Ce qui est écrit** — deux bras dans le `switch` de `:66-86`, deux appels dans
`servir` (`:103`), et un bras `TYPE_DUES` qui appelle un `onDues` **injecté** et
rend `null`.

🔴 **Le commentaire de `:17-21` est RÉÉCRIT** — voir §3.1. Un bras qui rendrait
`null` sans que la famille soit nommée serait le bras catch-all silencieux que
`capteur/pont_media.rs` a fait payer **quatre fois** à ce dépôt (D5 `Sommeil`,
D6 `Part`, D7 `Audio`, D8 `PleinEcran`).

**Tests, et leur ROUGE :**

| Test | Rouge |
| --- | --- |
| `une_ecriture_recoit_toujours_un_fait_ou_un_echec` | ne rien rendre sur le chemin d'erreur → la commande reste en vol côté agent jusqu'à `DELAI_ECRIRE` |
| `une_annonce_de_dues_ne_repond_rien_et_appelle_le_rappel` | rendre une trame → le pont recevrait une réponse à une corrélation qu'il ne connaît pas, et la jetterait en `debug!` — **silencieusement** |
| `le_code_d_echec_de_l_ecrivain_traverse` | rendre `'interne'` pour tout → la cause est détruite à l'émission, exactement comme `web/index.js:669` |

**Dépendances** — tâches 1, 9.

---

### Task 11 : la page-shell — le compteur, `beforeunload`, et `readwrite`

**Objet** — rendre la fenêtre de perte **visible**, et avertir avant qu'on ne
la referme par accident.

**Fichiers** — `client/src/shell.ts`, `client/src/shell.test.ts`,
`client/src/shell-page.ts`, `client/shell.html`,
`client/src/fichiers/canal.ts` (`:196`, `mode: 'read'` → `'readwrite'`).

**Ce qui est écrit :**

```ts
// shell.ts — LA RÈGLE, testée
ecrituresDues(dues: { chemin: string; octets: number }[]): void;
ecritureEchouee(chemin: string, motif: string): void;
```

🔴 **LE COMPTEUR EST UN NOMBRE DANS UN ATTRIBUT, PAS UNE PHRASE.**
`client/shell.html` porte
`<span id="ecritures-dues" data-dues="0" data-echecs="0"></span>`, et le
pilote de recette lit **`data-dues`**, jamais le texte.

⚠️ **C'est le remède direct au piège que F1 a payé neuf minutes** : « Lecteur
… **mont**é » (`client/src/shell.ts:124`) et « n'a pas pu être **mont**é »
(`:149`) partagent une sous-chaîne, le pilote testait `includes('mont')`, et
**une mesure entière a tourné sur un pont non monté**. Le produit garde cette
ambiguïté — **ce plan ne la rejoue pas sur le compteur.**

⚠️ **Et c'est aussi le remède au FAUX VERDICT ÉLIMINATOIRE** : `data-dues="0"`
sur une machine saine ne dit **rien**, puisque rien n'a encore eu lieu. Le
pilote lit donc **aussi** `data-vues`, un compteur **cumulatif et monotone** des
dues jamais remis à zéro. *Un verdict négatif exige que la chose mesurée soit
ABSENTE, pas seulement nulle* : `data-dues=0` **et** `data-vues>0` est un
succès ; `data-dues=0` **et** `data-vues=0` est une **mesure non prise**.

**`beforeunload`, et ses trois limites en commentaire** (§0.4) :

```ts
window.addEventListener('beforeunload', (e) => {
    if (dues.length === 0) return;
    e.preventDefault();   // le TEXTE est ignoré par tous les navigateurs modernes
});
```

**Le `mode: 'readwrite'`** de `canal.ts:196`. ⚠️ **Et il ne sera pas exercé** :
l'instrument de recette est **OPFS**, dont `navigator.storage.getDirectory()`
rend une vraie `FileSystemDirectoryHandle` **sans aucun modèle de permission**
(F1 résultats §3). `queryPermission`/`requestPermission` et l'activation
utilisateur transitoire restent **non couverts**, comme en F1. **Déclaré.**

**Tests, et leur ROUGE** (`shell.test.ts`, DOM injecté) :

| Test | Rouge |
| --- | --- |
| `le_compteur_affiche_le_nombre_de_dues_dans_data_dues` | l'écrire dans le texte → le pilote lit une phrase, et le piège de F1 se rejoue |
| `le_compteur_vu_est_monotone_et_ne_redescend_jamais` | le remettre à zéro → un verdict négatif devient indiscernable d'une mesure non prise |
| `un_echec_nomme_le_fichier_et_la_cause` | afficher « une écriture a échoué » → l'utilisateur ne sait pas quel fichier ouvrir |
| `zero_due_efface_le_bandeau_et_pose_le_ton_neutre` | garder le texte → **c'est le défaut de D5**, et `shell.ts:127-143` le documente déjà contre lui-même |
| `beforeunload_ne_previent_pas_quand_il_n_y_a_rien_a_perdre` | prévenir toujours → l'utilisateur apprend à ignorer l'avertissement |

⚠️ **`beforeunload` lui-même n'est PAS testé en Vitest** — il dépend du
navigateur. Ce qui est testé est le **prédicat** (`doitPrevenir(dues)`), pur ;
le câblage vit dans `shell-page.ts`, qui n'est pas testé, comme le reste de ce
fichier (`shell-page.ts:1-2`).

**Dépendances** — tâches 9, 10.

---

# Famille E — le câblage Windows

### Task 12 : le rappel de notification — POST, `isdirectory`, chemin de destination

**Objet** — traduire une notification ProjFS en un événement pour le fil
d'écriture, **sans jamais faire d'E/S sur le fil du système**.

**Fichiers** — `agent/src/pont/projfs/rappels/notification.rs` (créé par la
tâche 3), `agent/src/pont/projfs/etat.rs`.

**Ce qui est écrit :**

- lire `isdirectory` (`rappels.rs:409`, aujourd'hui `_est_repertoire`) ;
- lire le chemin par `chemins_de` — **la normalisation de
  `agent/src/pont/chemins.rs` reste la seule barrière** contre les `..`, les
  `:` (flux alternatifs NTFS) et les noms réservés, et elle est **PURE** ;
- appeler `notifications::decider(code, Etat { inscriptible, canal_ouvert })` ;
- sur `Pousser(_)`, **pousser un `Evenement` sur un `mpsc::Sender` et rendre
  `S_OK` immédiatement**. ⚠️ **Aucune lecture de fichier, aucun verrou tenu,
  aucune attente** — la discipline de `pont/projfs.rs:11-32`.

⚠️ **`PRJ_NOTIFICATION_PARAMETERS` (`mod.rs:352-356`, ses trois membres décrits
en `mod.rs:364-376`) est une UNION, et lire le mauvais membre est un
comportement indéfini.** F2 n'a besoin
d'**aucun** d'eux : `PostCreate.NotificationMask` et
`FileRenamed.NotificationMask` servent à **changer le masque** pour ce fichier,
ce que F2 ne fait pas, et `FileDeletedOnHandleClose.IsFileModified` concerne la
suppression, qui est **F3**. **Le paramètre reste `_parametres`, et le
commentaire dit POURQUOI** — sans quoi un successeur le lira comme un oubli.

**Contrôle, et son ROUGE** — aucun test d'hôte n'est possible ici
(`#[cfg(windows)]`, appelé par le système, spec §4.4). Ce qui existe :

```bash
cd agent && cargo check --target x86_64-pc-windows-gnu
```

et **le `const _: PRJ_NOTIFICATION_CB = Some(notification)`**, qui est vérifié
par cette compilation (`rappels.rs:52-59` explique pourquoi c'est un `const _`
et non un `#[test]` : *un `#[cfg(test)]` sur une cible qu'on ne teste jamais
n'est compilé par RIEN*). **Rouge** : changer `isdirectory: bool` en
`isdirectory: i32` → la compilation croisée refuse. Versé.

⚠️ **Le comportement, lui, n'est éprouvé que par la recette.** C'est déclaré,
pas contourné.

**Dépendances** — tâches 3, 4, 7.

---

### Task 13 : `pont.rs` et `pont/service.rs` — lancer le fil, et traiter le `Fait`

**Objet** — brancher les trois fils entre eux, et compléter proprement.

**Fichiers** — `agent/src/pont.rs`, `agent/src/pont/service.rs`,
`agent/src/pont/service/verbes.rs`, `agent/src/pont/projfs/etat.rs`,
`agent/src/pont/projfs.rs`.

**Ce qui est écrit :**

- `pont::executer` résout `dossier_etat()`, relit le journal, ouvre les deux
  `mpsc` du fil d'écriture, et **lance un QUATRIÈME fil** nommé
  `pont-ecriture` ;
- `Etat::demander_avec_charge`, jumeau de `demander` (`etat.rs:158-186`), qui
  passe une charge non vide à `proto::fichiers::encoder` ;
- `service::appliquer` gagne les bras `(Attendue::Ecrire{..}, _)` et
  `(Attendue::Creer{..}, _)` sur `TYPE_FAIT`, qui **relaient au fil
  d'écriture** et rendent `Suite::Termine(S_OK)` — sans jamais appeler
  `PrjCompleteCommand`, puisque `command_id` est `None` ;
- `service::terminer` et `verbes::completer` ignorent un `command_id` absent,
  **en le journalisant à `debug!`** ;
- `Virtualisation::drop` (`projfs.rs:266-320`) : les écritures en vol sont
  **vidées de la table comme les autres**, mais **NE SONT PAS retirées du
  journal** — c'est exactement le cas que le journal existe pour couvrir.

⚠️ **L'arrêt du pont a désormais QUATRE fils à ordonner**, et l'ordre du
`Drop` de `projfs.rs:248-252` est augmenté d'une étape, écrite :
0. **fermer le canal du fil d'écriture** et l'attendre — sans quoi il pousserait
   sur un `Sender` dont l'autre bout est parti, et son entrée de journal
   resterait sans que rien ne le dise ;
1. vider la table, compléter chaque commande **ayant un `command_id`** ;
2. `PrjStopVirtualizing` ;
3. reprendre l'`Arc` confié.

**Contrôles** :
```bash
cd agent && cargo test -p agent
cd agent && cargo check --target x86_64-pc-windows-gnu
grep -c 'dead_code' <(cargo check --target x86_64-pc-windows-gnu 2>&1)
```
⚠️ **Deux des cinq avertissements `dead_code` imputables à F1 doivent
DISPARAÎTRE** (`Erreur::DisquePlein`, `Erreur::DejaPresent`,
`pont/erreurs.rs:75,81`). **Si les seize avertissements restent seize, une
variante n'est pas employée là où ce plan croit qu'elle l'est** — c'est un
contrôle gratuit, et il peut échouer.

**Dépendances** — tâches 5, 8, 12.

---

# Famille F — recette et clôture

### Task 14 : SONDE PRÉALABLE — quel idiome d'enregistrement emploie CETTE VM ?

**Objet** — mesurer, **avant** d'écrire un seul critère, si l'instrument de la
recette peut exister. C'est le pendant exact de `sonde-picker.txt` en F1, et
c'est ce qui empêche d'imputer au produit un échec de protocole.

**Fichiers** — `docs/superpowers/plans/journaux-pont-fichiers-f2/instrument/sonde-idiome.ps1`
et son relevé versé.

**Ce qu'elle mesure**, sur un fichier ordinaire de `C:\dev\` (**pas** dans la
racine du pont, qui n'existe pas encore), en observant le répertoire pendant
l'enregistrement :

| Outil | Question |
| --- | --- |
| `notepad.exe` (piloté par frappes) | écrit-il **en place**, ou crée-t-il un fichier temporaire puis renomme-t-il ? |
| `[IO.File]::WriteAllText` | en place ? *(attendu : oui — `CreateFile`/`WriteFile`/`CloseHandle`)* |
| `Add-Content` | en place ? |
| `cmd /c echo … > fichier` | en place ? |

**Le verdict décide de l'instrument** :

- **au moins un outil écrit en place** → il devient l'instrument du critère ① ;
- **aucun n'écrit en place** → 🔴 **F2 n'est pas recevable sans F3**, et la
  tâche 15 le déclare au lieu de mesurer autre chose.

⚠️ **Cette sonde ne peut pas rendre un faux verdict par « trois zéros »** : son
critère est la **présence** d'un fichier temporaire pendant l'enregistrement,
pas son absence. Un répertoire observé sans qu'aucun enregistrement n'ait eu
lieu rend « **non mesuré** », jamais « en place ».

**Contrainte WinRM** : ⚠️ **aucun guillemet en ligne de commande** — le script
vit sur `/media/vm/dev/` et s'invoque par `-File`
(`nodejs-winrm` enveloppe toujours la commande dans
`powershell -Command "& { … }"`, et un guillemet entre en collision **sans
message clair**).

**Dépendances** — aucune. **À jouer TÔT**, parce qu'elle peut invalider la
tâche 15.

---

### Task 15 : recette F2 sur la VM

**Objet** — établir que les octets traversent, et **voir rouge** chacun des
contrôles.

**Préalables, non négociables** :

```bash
virsh list --all && virsh start Windows
until timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null; do sleep 5; done
until ls /media/vm/dev >/dev/null 2>&1; do sleep 5; done
set -a && source .env && set +a
cd agent && cargo clean --release -p proto -p agent
scripts/build-agent.sh && ls -l /media/vm/dev/target/release/agent.exe   # la TAILLE est relevée
```

⚠️ **`Get-Process agent` se revérifie AVANT et APRÈS chaque tentative, y compris
échouée** — piège de D8, rejoué en F1. ⚠️ **Chaque `agent.log` est copié sous un
nom unique immédiatement après la fin RÉELLE de l'exécution**, jamais à la fin
du pilote : les enfants meurent quand le navigateur se ferme, donc **après** la
copie (piège de D4, payé de nouveau en F1 avec `exec3`/`exec4`).

**L'instrument** : celui de F1 (`journaux-pont-fichiers/instrument/pilote-f1.mjs`),
**OPFS** en racine, `globalThis.showDirectoryPicker` surchargé au point
d'injection le plus bas. Le pilote gagne : la lecture de `data-dues`/`data-vues`
(**jamais du texte**), la relecture d'un fichier OPFS et son condensat SHA-256.

**Les six critères, DEUX exécutions chacun :**

| # | Critère | Ce qui le rend ROUGE, et comment le provoquer |
| --- | --- | --- |
| ① | Un fichier **enregistré depuis la VM** avec l'outil retenu par la tâche 14 arrive côté navigateur avec **le même condensat SHA-256**, sur un fichier d'au moins **1 Mio** (donc **plusieurs morceaux**) | **Provoqué** : `PONT_ECRITURE=0` → aucun fichier n'arrive. ⚠️ **C'est le critère 2 de F1, jamais établi, enfin joué — dans l'autre sens.** Un critère « le fichier apparaît » serait satisfait par un fichier tronqué, par des morceaux dans le désordre, et par un dernier morceau manquant : **les trois sont des défauts que `pont/decoupe.rs` et le fil d'écriture peuvent réellement produire** |
| ② | 🔵 **`PRE_CONVERT_TO_FULL` est réellement traversé** — la trace du rappel le nomme, **au moins une fois**, sur une écriture d'un fichier **projeté et non encore hydraté** | **Provoqué** : monter le lecteur, **lister sans lire**, puis écrire directement. **Rouge** : `PONT=…` avec la racine en lecture seule → la trace doit porter le refus. ⚠️ **Legs 8 de F1 : ce chemin n'a JAMAIS tourné.** S'il n'apparaît pas, il faut dire **lequel** de « pas exercé » ou « pas atteignable » |
| ③ | Le pont **tué entre l'écriture et l'acquittement**, puis relancé, **pousse l'écriture en attente**, et le journal redevient **vide** | **Provoqué** : `PONT_ECRITURE=0`, écrire, tuer le pont **par le PID relevé DANS LE JOURNAL** ⚠️ *(l'heuristique « le plus jeune » est FAUSSE : l'ordre est superviseur, capteur, pont, puis les enfants — F1 a tué un enfant et le relevé se lisait comme un succès)*, relancer avec `PONT_ECRITURE` armée. **Rouge** : retirer la relecture du journal au démarrage → rien n'est repoussé |
| ④ | Le compteur **monte** puis **redescend à zéro** | **Provoqué** : `PONT_ECRITURE=0` → `data-dues` monte et **ne redescend jamais**. ⚠️ **`data-dues=0` avec `data-vues=0` est une MESURE NON PRISE, pas un succès** — *un compteur qu'on n'a jamais vu monter n'est pas un compteur* |
| ⑤ | 🔵 **Une poussée interrompue laisse le fichier local INCHANGÉ** (l'atomicité de `createWritable`, §3.4) | **Provoqué** : écrire un fichier de plusieurs morceaux, fermer la page-shell au milieu, relire l'OPFS. **Rouge** : si le fichier est **partiellement** écrit, l'inférence du §3.4 est **réfutée**, et c'est un résultat, pas un échec. ⚠️ **C'est aussi le rouge (ii) que F1 n'a jamais provoqué** |
| ⑥ | 🔴 **La vidéo ne perd pas une image pendant toute la recette** | `framesDecoded` **monotone**, `packetsLost` **0**, **0** `clôture de session amorcée`. **Rouge** : c'est le critère 4 de F1, tenu 6 fois sur 6 ; s'il tombe, F2 a cassé la promesse structurelle de D2 |

**Les `grep` de la recette, avec leur ÉTABLISSEMENT** — ⚠️ **chaque chaîne est
d'abord prouvée émise par le produit**, parce que trois des quatre `grep` de F1
cherchaient des chaînes qui n'existent pas :

```bash
# 0. Mettre à plat, et TOUJOURS grep -a (queue d'octets NUL → sortie vide, pas zéro)
sed 's/\x1b\[[0-9;]*m//g' agent.log > agent-plat.log

# 1. Établir AVANT de compter — chaque chaîne doit apparaître dans le CODE
grep -rn "ecriture poussee"        agent/src/pont/       # doit rendre ≥ 1 ligne
grep -rn "ecriture acquittee"      agent/src/pont/
grep -rn "ecriture due retenue"    agent/src/pont/
grep -rn "PONT_ECRITURE"           agent/src/pont/ scripts/run-agent.sh
grep -rn "pont fichiers lancé"     agent/src/superviseur/   # la chaîne de F1, DEUX lignes la portent

# 2. Puis compter
grep -ac "ecriture poussee"     agent-plat.log
grep -ac "ecriture acquittee"   agent-plat.log
grep -ac "ecriture due retenue" agent-plat.log
grep -ac " ERROR "              agent-plat.log        # attendu 0
```

🔴 **Si l'étape 1 rend zéro ligne pour une chaîne, la recette S'ARRÊTE et la
chaîne est corrigée** — c'est la seule façon d'éviter qu'un `grep` à zéro se
lise comme un produit en panne.

**Contrôle d'un chemin réel plutôt que de cinq approximations** : le critère ①
n'est **pas** « les tests passent, le binaire se construit, le pont se lance ».
Il est **un fichier enregistré depuis une application Windows, relu depuis le
navigateur, condensat contre condensat**. *Cinq barrières vertes et un service
qui ne démarre pas* — c'est ce que le sous-projet ⑤ a mesuré le même jour.

**Journaux versés** — `docs/superpowers/plans/journaux-pont-fichiers-f2/`, avec
un `LISEZ-MOI.md` portant **les familles de lecture** (bruts avec ANSI /
`-plat` jumeaux / journaux de pilote en `grep -a`) et **toute chaîne de `grep`
qui s'est révélée fausse**.

**Dépendances** — tâches 1 à 14. **Sérialisée avec toute autre tâche employant
la VM.**

---

### Task 16 : revue transverse de fin de branche, résultats, et `CLAUDE.md`

**Objet** — trouver ce qu'aucune revue par tâche ne peut voir, et verser la
preuve dans git.

**Elle est OBLIGATOIRE.** Barème du dépôt : D7 **5**, D8 **3**, D9 **6**,
D10 **douze**, D11 **sept**, P1 **huit**, P2 **dix**, S1 **cinq**, E **neuf**,
P3 **douze**, S2 **douze**, F1 **onze**, P4 **huit**, S3 **treize**, G1 **huit**,
S4 **vingt-sept**, presse-papier **dix-sept**, P5 **neuf**.

**Ce qu'elle cherche en priorité**, parce que c'est ce que F2 rend probable :

1. **Les affirmations de F1 que F2 réfute.** Balayer `agent/src/pont/`,
   `client/src/fichiers/` et `proto/` sur les formules — « lecture seule »,
   « F1 vit tout entier dans cet état », « toute tentative d'écriture »,
   « appartient à F2 », « est un livrable de F2 ». ⚠️ **Chercher par le SENS,
   pas par la formule** : « pas / non / jamais / seul / uniquement ». Cibles
   connues d'avance : `agent/src/pont/erreurs.rs:82-85`,
   `agent/src/pont/notifications.rs` (l'en-tête entier),
   `agent/src/pont/table.rs:37`, `agent/src/pont/projfs/etat.rs:78-81`,
   `client/src/fichiers/adaptateur.ts:14-30`, `client/src/fichiers/canal.ts:168`.
2. **Les affirmations que F2 écrit et que F2 réfute lui-même.** C'est le patron
   le plus régulier du dépôt (D10 en a compté six, F1 cinq) : une tâche écrit
   une phrase, une tâche suivante la rend fausse, et les deux revues sont
   correctes séparément.
3. **Les nombres.** ⚠️ **Toute ligne de tableau que l'on ÉDITE oblige à
   REMESURER son compte** — le naufrage du « 487 » s'est rejoué **neuf** fois
   dans ce dépôt, dont une fois **dans la vague qui le corrigeait ailleurs**, et
   deux fois **sur la ligne même que la main éditait**. Et **« corrigé à sa
   place » est une affirmation de COMPLÉTUDE** : elle se vérifie en énumérant
   les places par `grep -n '<le nombre>' CLAUDE.md` **avant** d'écrire, et en
   **relisant place par place APRÈS** l'édition.
4. **Aucune sortie de commande n'est fabriquée.** D10 en a attrapé deux, dont
   une **inscrite dans `CLAUDE.md` à l'intérieur d'une correction qui dénonçait
   une affirmation non étayée**. Toute transcription est le copié-collé d'une
   exécution réelle.

**Ce qu'elle produit** :

- `docs/superpowers/plans/2026-08-20-pont-fichiers-f2-resultats.md` — le
  document **permanent**, portant l'analyse, les verdicts **avec leur nombre
  d'exécutions**, et « ce que F2 n'établit PAS » ;
- la section F2 de `CLAUDE.md`, **tailles relevées par la commande APRÈS la
  dernière édition** (une table relevée en début de ronde serait fausse à la
  fin de la même ronde — erreur commise en D8 en croyant bien faire) ;
- les quatre suites relancées et inscrites :
  ```bash
  cd agent  && cargo test -p agent
  cd agent  && cargo check --target x86_64-pc-windows-gnu
  cd client && npx vitest run
  cd client && npx vitest run --dir ../proto
  cd client && npx tsc --noEmit
  ```
- un `git diff --stat` vérifiant que **ni `agent/src/pont/transport.rs`, ni
  `agent/src/pont/transport/tests.rs`, ni `agent/src/superviseur/lanceur.rs`,
  ni `agent/src/demarrage.rs`** n'ont bougé.

**Dépendances** — toutes.

---

## 4. Ordre et dépendances

```
1  proto (types, en-têtes, vecteurs)  ─┬─────────────────────────────┐
2  run-agent.sh PONT_ECRITURE ────────┼──────────────┐              │
3  EXTRACTION rappels.rs ─────┬───────┼──────┐       │              │
                              │       │      │       │              │
4  notifications.rs (masque) ─┘       │      │       │              │
5  table.rs (sans command_id) ────────┤      │       │              │
6  journal.rs (PUR) ──────────┬───────┤      │       │              │
7  ecriture.rs (PUR) ─────────┴───────┤      │       │              │
8  ecriture/fil.rs (PUR) ─────────────┘      │       │              │
                                             │       │              │
9  client/fichiers/ecriture.ts ──────────────┼───────┼──────────────┘
10 client/fichiers/protocole.ts ─────────────┼───────┤
11 page-shell (compteur, beforeunload) ──────┼───────┘
                                             │
12 rappel de notification (#[cfg(windows)]) ─┤
13 pont.rs + service.rs (câblage) ───────────┘
                              │
14 SONDE idiome (VM) ─────────┤   ← indépendante, à jouer TÔT
                              ↓
15 RECETTE (VM) ──────────────→ 16 revue transverse + résultats + CLAUDE.md
```

- **3 bloque 4 et 12.** Aucune ligne n'entre dans `rappels.rs` avant.
- **2 précède 11 et 15.** La variable doit voyager avant qu'on en ait besoin.
- **14 peut invalider 15**, et se joue donc avant.
- **1, 5, 6, 7 sont indépendants** et peuvent être parallélisés — *mais un
  `git add` NOMINATIF, jamais `-A`.*
- **14 et 15 sont sérialisées avec toute tâche employant la VM**, y compris
  celles des autres sous-projets.

---

## 5. Divergences relevées entre la spec, F1 et le CODE RÉEL

Toutes relevées par la commande le 20 août 2026.

| # | Ce que dit le document | Ce que dit le code / la mesure |
| --- | --- | --- |
| 1 | spec §8 F2 : critère 2 (LibreOffice/Word, idiome temp+rename) | **inatteignable en F2** : `Renommer` et `Supprimer` sont livrés par F3. **Déplacé** — voir §0.2 |
| 2 | spec §6.2 : `beforeunload` « avec un texte qui nomme les fichiers » | **le texte personnalisé est ignoré par tous les navigateurs modernes.** Les fichiers sont nommés **dans la page** — §0.4 |
| 3 | spec §9 : « les rappels d'énumération partent dans `projfs/enumeration.rs` » | les rappels vivent dans `projfs/rappels.rs` depuis F1, et **`agent/src/pont/enumeration.rs` existe déjà** (PUR, 140). D'où **`projfs/rappels/listage.rs`** — §2.3 |
| 4 | spec §9 : `agent/src/superviseur/lanceur.rs` **367**, marge 133 | **488**, marge **12**. +121 lignes hors F1 (dernier commit `264c275`, sous-projet ⑤). **F2 ne le touche pas** |
| 5 | spec §9 : `client/src/shell-page.ts` **71**, `client/src/shell.ts` **93** | **221** et **154** |
| 6 | `CLAUDE.md` : `agent/src/superviseur/lanceur/pont.rs` **117** | **136** |
| 7 | Le brief de ce plan : la création locale réussit « **4 fois sur 4** » | **les pièces versées en portent DEUX** : `mesure-exec2.txt` et `mesure-exec5.txt` (`mesure-exec1.txt` s'arrête avant). Le document de résultats de F1 §7.2 écrit lui-même « **2 exécutions versées sur 2** ». ⚠️ **Ce plan retient 2/2** |
| 8 | ~~`agent/src/pont/notifications.rs:64-67` divergerait de `mod.rs`~~ | ❌ **AUCUNE DIVERGENCE : les douze numéros de ligne de `notifications.rs:64-76` sont EXACTS**, revérifiés un à un dans `windows-0.62.2`. C'est une première rédaction de CE plan qui les déclarait faux, en ayant lu `windows-sys-0.61.2` par erreur — voir l'encadré du §0.1, où le défaut est consigné plutôt qu'effacé |
| 9 | spec §3.5.2 : `TAILLE_MAX_FICHIER`, write-back refusé au-delà, `ERROR_DISK_FULL` | **le refus serait invisible** (§0.5). F2 pose un plafond de **journal**, pas de refus |
| 10 | F1 résultats §11 : suites à **614** / **223** / **111**, **16** avertissements | **non réexécutées ici** (arbre partagé, commits postérieurs). **À relever par la tâche 1** |

---

## 6. Ce que F2 n'établira PAS

Écrit d'avance, pour qu'aucun document de résultats n'ait à le découvrir.

- **Aucun taux.** Deux exécutions par critère est la règle ; deux exécutions ne
  font pas une fréquence.
- **Rien du renommage ni de la suppression**, donc **rien de l'idiome
  d'enregistrement atomique** que la spec §3.5 donne pour justifier D5. Le
  critère 2 de la spec §8 F2 est **déplacé en F3** et non joué.
- **Rien de la latence** : c'est F4, et c'est le risque R2 — *le lecteur peut
  fonctionner et rester inutilisable*. F2 publiera un temps de traversée par
  écriture ; il n'en tirera **aucune loi**, la seule mesure de débit du dépôt
  étant incohérente d'un **facteur ~120** sans explication.
- **Aucune constante calibrée** : `DELAI_ECRIRE`, `TAILLE_ECRITURE_SIGNALEE`,
  `TAILLE_JOURNAL_COMPACTAGE`, plus les quatre de F1 (`TAILLE_TRAME_MAX`,
  `DELAI_ATTRIBUTS`, `DELAI_LIRE`, `DELAI_LISTER`) et les huit du chantier D.
- **Le défaut de casse n'est corrigé qu'en ÉCRITURE** (§ tâche 9). `casse.txt`
  continuera de rendre le contenu de `Casse.txt` en lecture. La table de
  correspondance reste **F3**.
- **Aucun contrôle de flux** : un morceau en vol à la fois,
  `bufferedAmount`/`SEUIL_TAMPON` restent **F3**.
- **Aucun cache d'énumération** : `Rafraichir` est **F5**, et un cache que rien
  ne vide reproduirait le défaut de l'ancien pont.
- **Aucune politique d'éviction des hydratations.** Chaque fichier écrit reste
  sur le disque de la VM. La mesure est **F5**.
- **Aucun test d'hôte ne couvre `pont/projfs/`** ; `cargo check --target
  x86_64-pc-windows-gnu` en vérifie types, emprunts et durées de vie —
  **jamais le comportement**.
- **R7 reste OUVERT, et F2 n'y ajoute rien de neuf** — voir §7.
- **`showDirectoryPicker()` n'est toujours jamais appelé**, ni le modèle de
  permission (`queryPermission`/`requestPermission`), ni l'activation
  utilisateur transitoire, ni le **mode `readwrite`** : l'instrument est OPFS,
  et OPFS n'a pas de permission (F1 résultats §3).
- **Rien d'un client réel** : Chrome sans interface, décodage logiciel, sur
  l'hôte qui porte la VM. La File System Access API n'existe ni sur Firefox ni
  sur Safari.
- **Rien de la reprise à travers un redémarrage de la VM** (F5), ni du retour
  avec un **autre** répertoire (spec §6.4 cas 2, F5).
- **Rien de plusieurs utilisateurs**, rien de plusieurs applications écrivant
  le même fichier depuis la VM et le poste local (aucun verrou inter-machines).
- **L'énumération vide intermittente de F1 (legs 3) n'est ni expliquée ni
  refermée**, non plus que les lectures qui calent sans expirer (legs 4).
- **Le chemin d'extinction propre du superviseur** n'a toujours jamais été
  exercé, depuis D1.

---

## 7. Comment F2 réduit le risque des transcriptions d'API (R7)

**Le point de départ, mesuré et non craint** : les **treize** transcriptions de
`agent/src/pont/projfs/chargement.rs` sont écrites à la main, et **les trois
mutations d'ABI jouées en F1 ont TOUTES survécu** aux tests d'hôte et à
`cargo check --target x86_64-pc-windows-gnu`
(`journaux-pont-fichiers/f1-tache12-mutations.txt`). **Rien, dans ce dépôt, ne
peut attraper une transcription fautive** ; seule la relecture texte à texte et
l'exécution sur la VM en jugent.

**Ce que F2 fait, dans l'ordre du plus fort au plus faible :**

1. 🔵 **F2 N'AJOUTE AUCUNE ENTRÉE IMPORTÉE. Zéro.** C'est la réduction de risque
   la plus forte disponible, et elle est structurelle. Le write-back
   n'appelle **rien** de `ProjectedFSLib.dll` : les notifications **arrivent**
   par un rappel que **nous** écrivons, et la lecture du fichier local est un
   `std::fs::File::open` ordinaire. `resolution::NOMBRE` reste **13**, et
   `resolution::NOMS` est **inchangé** — un test existant le vérifie déjà.
   ⚠️ *`PrjUpdateFileIfNeeded` (`mod.rs:113`) et `PrjDeleteFile` — cette
   dernière déjà chargée — pourraient sembler nécessaires ; elles ne le sont
   pas : la première met à jour un **substitut**, la seconde appartient à F3 et
   à la politique d'éviction de F5.*
2. **Le seul rappel modifié est déjà couvert par le seul garde d'ABI du dépôt.**
   `const _: PRJ_NOTIFICATION_CB = Some(notification)` (`rappels.rs:67`) est
   vérifié **par la compilation croisée ordinaire, à chaque fois** —
   `rappels.rs:52-59` explique pourquoi c'est un `const _` et non un `#[test]`.
   La tâche 3 le **déplace avec sa fonction**, et la tâche 12 le rend **rouge
   une fois** (changer `isdirectory: bool` en `i32`) pour établir qu'il n'est
   pas décoratif.
3. **L'union `PRJ_NOTIFICATION_PARAMETERS` n'est PAS déréférencée** (§ tâche 12).
   Lire le mauvais membre d'une union est un comportement indéfini ; **ne pas la
   lire du tout est le seul moyen sûr**, et c'est possible parce qu'aucun de ses
   trois membres ne sert à F2.
4. **Un contrôle de revue** : toute constante ProjFS neuve porte sa **valeur** et
   son **numéro de ligne** dans `windows-0.62.2/.../mod.rs`, comme
   `notifications.rs:64-76` — dont les **douze** citations ont été revérifiées
   exactes le 20 août 2026. ⚠️ **Mais le contrôle porte sur un fichier qui a un
   HOMONYME** : `windows-sys` expose les mêmes symboles à d'autres lignes, et
   une première rédaction de ce plan s'y est trompée (§0.1). **Le commentaire
   dira désormais quel crate fait foi, et que la VALEUR prime sur la ligne.**

**Ce que F2 ne réduit pas** : le risque des **douze autres** entrées, que le
pont continue d'appeler sur le chemin de lecture. Il est **inchangé, pas
aggravé.**

---

## 8. Risques — et ce qui rendrait F2 NON LIVRABLE

| # | Risque | Ce qu'on en sait, et la parade |
| --- | --- | --- |
| **R-F2-1** | 🔴 **Aucun outil d'écriture de cette VM n'écrit EN PLACE** — tous emploient temp+rename, que F2 refuse par construction (§0.2) | **Éliminatoire pour la RECETTE, pas pour le code.** C'est la tâche 14 qui tranche, **avant** la tâche 15. Si c'est le cas : F2 est **codé et non recevable**, et le critère ① part en F3 avec le critère 2. **Il n'y a pas de repli** : accepter le renommage sans le pousser produirait la perte silencieuse que tout ce sous-projet existe pour interdire |
| **R-F2-2** | 🔴 **`PRE_CONVERT_TO_FULL` n'existe pas en pratique sur cette VM** — jamais émise, ou émise après l'hydratation | **Non tranché, et jamais exercé** (legs 8 de F1). Si elle n'arrive jamais, **F2 fonctionne quand même** (le write-back est déclenché par les POST) mais **perd son unique porte de refus** : plus aucune écriture ne peut être refusée, même canal fermé. **Dégrade, ne bloque pas.** C'est le critère ② |
| **R-F2-3** | **Lire le fichier hydraté ré-entre dans nos propres rappels** | **Inférence, non mesurée** (§ tâche 8). Ne provoque **pas** d'interblocage grâce au fil dédié, mais ferait relire nos octets à travers le navigateur. Visible au journal ; le critère ④ le montrerait |
| **R-F2-4** | **La fenêtre de perte est trop longue pour être acceptable** | **Non mesurable ici** : le seul chiffre du dépôt varie d'un facteur ~120. F2 l'instrumente ; **F4 la juge.** ⚠️ *Ne dégrade rien : elle existe de toute façon* |
| **R-F2-5** | **La garde de casse refuse des écritures légitimes** — un poste local **sensible** à la casse (Linux) porte licitement `note.txt` et `Note.txt` | **Elle refuserait alors les deux.** ⚠️ **Non résolu, et déclaré** : distinguer les deux exigerait la table de correspondance de F3. Le refus est **loud** (journal + page-shell), pas silencieux, et c'est le seul arbitrage disponible entre « refuser à tort » et « écraser le mauvais fichier » |
| **R-F2-6** | **Le journal grossit sans fin** si les poussées échouent en boucle | Compactage à vide seulement, et le compteur d'écritures dues **le rend visible**. Pas de purge automatique : purger une due, c'est perdre la donnée |
| **R-F2-7** | **Une panique dans le rappel de notification abat le pont** | `catch_unwind` à chaque frontière (`rappels.rs:76-88`), et **D2 fait que le pire cas est la mort du pont** : le superviseur le relance (mesuré, `delai_apres_mort_ms=44`), **le journal survit**, la vidéo ne bronche pas |
| **R-F2-8** | **Le pont concurrence la vidéo pour le lien** — `BUDGET_BPS` ne connaît pas ce trafic (R5 de la spec) | **Non traité, nommé.** Une poussée de gros fichier prend de la bande passante que le contrôleur de congestion vidéo lira comme une dégradation. Le critère ⑥ le verrait |

**Ce qui rendrait F2 NON LIVRABLE : R-F2-1 seul**, et il est mesuré **avant**
tout le reste par la tâche 14. Tous les autres dégradent, bornent ou reportent.

---

## 9. Résumé en une phrase

> **F2 ne peut pas empêcher une écriture d'être perdue ; il peut la rendre
> improbable, récupérable, et — quand elle l'est quand même — NOMMÉE.**
