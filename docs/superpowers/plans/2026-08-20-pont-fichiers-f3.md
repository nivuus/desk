# Sous-projet ③ Pont fichiers — sous-bloc F3 : le renommage, la suppression, et la table d'erreurs qui cesse d'être décorative

**20 août 2026.**
Conception : `docs/superpowers/specs/2026-08-19-pont-fichiers-design.md` (commit
`06619fc`), §3.5, §3.5.1, §4.3, §5, §7.3 et §8 F3.
Sous-blocs antérieurs : `docs/superpowers/plans/2026-08-19-pont-fichiers-f1.md`
et `…-f1-resultats.md` (**livrés, et ils font autorité sur l'état réel partout où
ils contredisent la spec**) ; `docs/superpowers/plans/2026-08-20-pont-fichiers-f2.md`
(commit `832defe`, **planifié, NON implémenté au 20 août 2026** — voir §1.1).

> **Périmètre, énoncé négativement d'abord.** F3 ne fait **ni** le banc de
> latence (F4), **ni** `Rafraichir`, **ni** le cache d'énumération, **ni** la
> mesure d'occupation disque, **ni** la reprise à travers un redémarrage de VM,
> **ni** le retour avec un autre répertoire (tout cela est F5). Il ne calibre
> aucune constante. Il livre **quatre choses** : que renommer et supprimer dans
> la VM se répercute sur le poste local ; que la casse cesse de désigner le
> mauvais fichier **en lecture** ; que les douze codes d'erreur du §5 soient
> **observés** plutôt qu'écrits ; et que le canal ne soit plus la source de
> latence de tout le reste.

---

## 0. Ce que F3 doit trancher AVANT d'écrire une ligne de code

Six questions. Trois sont tranchées **contre** la lettre de la spec, et chacune
porte le relevé qui la fonde.

### 0.1 🔵 Par quoi renomme-t-on, puisque la File System Access API n'a pas de renommage ?

**Fait de plateforme, non mesuré ici et déclaré comme tel** : `FileSystemHandle.move()`
n'appartient pas à la norme du File System Access API ; c'est une extension
Chromium. La spec §3.5.1 le dit, et l'ancien pont s'en sert
(`web/index.js:628`, `:644` — **relevés, relus**).

**Décision de F3, en deux branches, les deux LIVRÉES et les deux TESTÉES sur
l'hôte** :

1. **`move(parentDestination, nom)` quand il existe**, détecté par
   `typeof poignee.move === 'function'` — jamais supposé, jamais capturé au
   chargement du module (une détection faite une fois pour toutes serait fausse
   le jour où l'on injecterait un autre système de fichiers).
2. **Sinon : copie puis suppression, ENTIÈREMENT DANS LE NAVIGATEUR.**
   `getFile()` sur la source, `createWritable()` sur la destination, les octets
   passent de l'un à l'autre **sans jamais sortir de la page**, puis
   `parent.removeEntry(nom)`. Récursif pour un répertoire : on recrée l'arbre
   par `getDirectoryHandle(n, { create: true })` et on copie chaque feuille.

> ❌ **LE CHIFFRE DE COÛT DE LA SPEC §3.5.1 EST FAUX POUR CE MONTAGE, et c'est
> la divergence la plus utile de ce plan.** La spec écrit : « le repli […] fait
> transiter **tout le contenu du fichier deux fois** sur le canal. Renommer un
> fichier de 1 Gio sur le chemin de repli coûte donc 2 Gio de canal ». **Cela
> n'est vrai que si le PONT orchestre la copie**, par une suite de `Lire` et
> d'`Ecrire`. F3 ne l'orchestre pas : le renommage est **un seul message**
> (`Renommer { de, vers }`), et la copie de repli se fait entre deux poignées
> qui vivent toutes deux dans le navigateur, sur le disque du poste local.
> **Coût du repli sur le canal : ZÉRO octet, dans les deux branches.**

**Ce que le repli coûte quand même, et qu'il ne faut pas effacer avec le
chiffre ci-dessus** :

- **il n'est pas atomique** — une coupure au milieu laisse deux copies, dont
  l'une porte le nom cible et est partielle. La spec le dit ; c'est toujours
  vrai, et ce n'est pas réparable ici ;
- il **double transitoirement l'occupation disque du poste local** ;
- il est en **O(taille)** en temps et, pour un répertoire, en **O(nombre
  d'entrées)** appels FSA — sur un répertoire profond, cela peut être long, et
  **rien ici ne le borne** ;
- **la mesure de F4 reste due et reste pertinente** : la spec la prévoit
  (« Coût du repli de renommage par copie (§3.5.1) | 1 Mio, 100 Mio »). Elle
  mesurera un **temps local**, pas un débit de canal. **Le repli est instrumenté
  comme la spec l'exige** — une trace `renommage par copie` portant la taille et
  le nombre d'entrées.

**Le cas particulier qui détruit, et que ni la spec ni l'ancien pont ne
traitent** : renommer `a.txt` en `A.txt`. Sur un poste local **insensible à la
casse** (Windows, macOS par défaut), la destination « existe déjà » — et c'est
la source elle-même. Une implémentation naïve refuse (`deja-present`) ou, pire,
écrase. **Règle de F3** : si l'unique homonyme insensible à la casse de la
destination **est la source**, c'est un **renommage de casse pure**, il est
licite, et le repli passe par un **nom intermédiaire** (deux mouvements), jamais
par un écrasement. Testé sur l'hôte avec le faux insensible à la casse.

**Ce que l'ancien pont enseigne, relu ligne à ligne** :

| Relevé | Ce que F3 en fait |
| --- | --- |
| `web/index.js:631` — `const newDir = await newDir.getDirectoryHandle(...)` **à l'intérieur du bloc où `newDir` est le paramètre** : zone morte temporelle, `ReferenceError`. **Le renommage d'un répertoire contenant un sous-répertoire échoue donc toujours** | C'est le critère (1) de F3, écrit pour exercer exactement ce cas |
| `web/index.js:628` — l'ancien pont appelle `entry.move(...)` sur des **fichiers** seulement, et recrée les répertoires à la main | **Indice, pas preuve** : rien n'établit que `move()` existe sur un répertoire. C'est la question ② de la sonde S2 (tâche 16) |
| `web/index.js:605`, `:612`, `:639` — `handle.remove()`, **non standard** | F3 emploie `parent.removeEntry(nom)`, **standard** |

### 0.2 🔴 Ce que F3 fait du défaut de casse en LECTURE — et une relecture du legs 1 de F1

**Le relevé de F1, tel qu'il est** (`…-f1-resultats.md` §7.1, trois exécutions
sur trois) : avec `Casse.txt` sur le poste local, `casse.txt` **et** `CASSE.TXT`
rendent le **contenu** de `Casse.txt` sans erreur, tandis que `GROS.BIN` rend
« introuvable » **dans la même exécution**. F1 en fait un legs pour F3 : « une
table de correspondance alimentée par l'énumération ».

**Le code, relevé** : la casse n'est repliée **qu'à un seul endroit** de tout le
périmètre — `agent/src/pont/chemins.rs:139`,
`NOMS_RESERVES.iter().any(|r| r.eq_ignore_ascii_case(base))`, pour reconnaître
les noms de périphérique réservés. Partout ailleurs elle est conservée telle
quelle. Côté navigateur, `client/src/fichiers/adaptateur.ts` résout **par appel
direct** : `getDirectoryHandle(parts[i])` (`:171`),
`getFileHandle(dernier)` (`:249`), `getFileHandle(...)` (`:272`).
**`values()` n'est employé que pour énumérer un contenu** (`:199`), jamais pour
résoudre un nom.

> ⚠️ **RELECTURE DU LEGS 1, et c'est une lecture, pas une mesure.** Les deux
> moitiés du phénomène n'ont pas la même gravité, et la moitié que F1 a
> **mesurée** est probablement la bénigne :
>
> - **La moitié VM.** `Casse.txt` était **hydraté** (F1 relève
>   `racine hydratee … octets=42 entrees=1`). NTFS, insensible à la casse,
>   résout `casse.txt` sur le fichier local **sans jamais atteindre le pont**.
>   L'application obtient le bon contenu du bon fichier. **C'est le
>   comportement normal de Windows, pas un défaut** — et rien n'est écrit nulle
>   part sous un mauvais nom, puisque aucun substitut n'est créé.
> - **La moitié NAVIGATEUR.** Sur un poste local **insensible à la casse**,
>   `getFileHandle('CASSE.TXT')` ouvre `Casse.txt`. **C'est là que le mauvais
>   fichier est rendu**, et c'est là qu'une écriture écraserait. **Cette
>   moitié-là n'a JAMAIS été observée** : l'instrument de recette est OPFS
>   (F1 §3), et si OPFS est sensible à la casse — question ③ de la sonde S2 —
>   elle ne peut pas se produire sur ce montage.
> - **L'incohérence que F1 relève** (`casse.txt` passe, `GROS.BIN` échoue) se
>   lit alors sans mystère : le premier est résolu par NTFS sans nous, le second
>   atteint le pont et bute sur un OPFS sensible à la casse.
>
> **Ce qui trancherait** : une exécution où le fichier demandé avec une autre
> casse n'a **jamais** été hydraté, **et** où le système de fichiers du « poste
> local » est insensible à la casse. **Aucune des deux conditions n'est
> disponible sur ce montage.** C'est déclaré ici, et le §7 le redit.

**La règle de F3, PURE et testée sur l'hôte** — un canonicaliseur de nom,
`client/src/fichiers/noms.ts` :

> Pour résoudre un composant de chemin dans un répertoire parent, on **énumère
> le parent** et on compare **octet pour octet** :
>
> - **un nom exact trouvé** → c'est lui, et c'est le nom canonique ;
> - **aucun nom exact, exactement UN homonyme insensible à la casse** → c'est
>   lui, et **le nom canonique est le nom STOCKÉ**, pas le nom demandé ;
> - **aucun nom exact, PLUSIEURS homonymes** (poste local sensible à la casse
>   portant `note.txt` et `Note.txt`) → **`casse-ambigue`, on ne rend rien** ;
> - **rien du tout** → `introuvable`.

**Trois conséquences, toutes voulues :**

1. **Le pont rend le nom canonique au système**, et le substitut est créé sous
   ce nom-là : `PrjWritePlaceholderInfo` reçoit désormais le nom **rendu par le
   navigateur**, jamais celui que l'application a tapé. La racine ne peut donc
   plus montrer un nom qui n'existe pas sur le poste local.
2. **L'incohérence disparaît** : `GROS.BIN` sur un `gros.bin` local rend
   désormais le fichier, sous son nom `gros.bin`, exactement comme NTFS le
   ferait. **C'est mesurable sur l'instrument OPFS**, et c'est le rouge/vert de
   la tâche 9.
3. **Le renommage cesse d'être un couteau** : `de` et `vers` passent tous deux
   par le canonicaliseur, et la règle du renommage de casse pure (§0.1) est la
   seule exception.

⚠️ **AUCUN CACHE. Le canonicaliseur énumère le parent à CHAQUE résolution.**
C'est cher — `adaptateur.ts:207-211` déclare déjà le coût d'un `getFile()` par
entrée au listage —, et c'est délibéré : un cache que rien n'invalide est
exactement le défaut de l'ancien pont (`src/file.js`, cache **sans TTL**), et
le seul moyen de le vider (`Rafraichir`) est un livrable de **F5**.
**F3 échange donc de la latence contre une correction, et c'est F4 qui dira ce
que l'échange coûte.** L'optimisation évidente — court-circuiter l'énumération
quand `poignee.name` rend déjà le nom stocké — **n'est pas écrite**, parce que
la question ③ de la sonde S2 ne peut pas être répondue sur ce montage : elle est
**léguée**, pas implémentée à moitié.

### 0.3 🔴 L'entrelacement avec les écritures dues de F2 — c'est ici que l'idiome temp+rename se joue

**C'est la seule règle de ce plan dont l'oubli produit une perte de données, et
elle n'est écrite dans aucun document.**

L'idiome d'enregistrement que la spec §3.5 donne pour justifier D5 est :
*écrire un fichier temporaire, renommer, supprimer l'ancien*. Les trois gestes
arrivent en rafale, sur le même répertoire. Or F2 pousse les écritures **après
coup**, à la fermeture du handle, dans une fenêtre dont il déclare lui-même
qu'elle n'est pas bornée en durée (`…-f2.md` §0.3). Donc :

- **une poussée d'écriture encore due sur `de` au moment où `Renommer` part
  arriverait APRÈS le renommage, sur un chemin qui n'existe plus** — le
  navigateur recréerait le fichier temporaire, et l'enregistrement serait
  perdu ;
- **une poussée d'écriture due sur un chemin que l'on vient de supprimer
  recréerait le fichier** que l'utilisateur a effacé.

**Les deux règles de F3, PURES et testées :**

| Geste | Règle |
| --- | --- |
| `Renommer { de, vers }` | **Les écritures dues sur `de` sont poussées AVANT**, et le renommage attend leur `Fait`. Si le drainage échoue, **le renommage n'est pas poussé** : l'entrée reste due, le fichier est **nommé** à l'utilisateur par le compteur de F2, et le journal porte `renommage suspendu : écriture due sur la source` |
| `Supprimer { chemin }` | **Les écritures dues sur `chemin` sont RETIRÉES du journal**, avec une trace `écriture due abandonnée : le chemin a été supprimé`. Les pousser recréerait ce que l'utilisateur efface |

⚠️ **Ces deux règles supposent que F2 a livré une file d'écritures dues
indexée par chemin.** Son plan la nomme `agent/src/pont/ecriture.rs`, PURE, avec
« une seule poussée en vol par chemin » (§0.6). **La tâche 1 relève les noms
réels** ; si F2 n'a pas été implémenté au moment où F3 démarre, le §1.1 dit ce
qui se passe.

### 0.4 ⛔ « Le contrôle de flux » est VACUEUX tant qu'un seul morceau est en vol

La spec §7.3 pose : « le pont ne demande pas le morceau *n+1* tant que le canal
a plus de `SEUIL_TAMPON` octets en attente ». **Relevé dans le code** : il n'y a
**qu'un morceau en vol à la fois** — le suivant n'est demandé qu'à réception du
précédent (`agent/src/pont/service.rs:228-238`,
`agent/src/pont/projfs/rappels.rs:303-306`,
`agent/src/pont/projfs/etat.rs:76-82`), et **trois commentaires du code
déclarent explicitement que la fenêtre est un livrable de F3**. Le plan de F2 le
redit (« un morceau en vol à la fois, `bufferedAmount`/`SEUIL_TAMPON` restent
F3 »).

> **Avec un seul morceau en vol, la règle de la spec ne peut jamais mordre : le
> pont n'est jamais en avance.** Livrer `SEUIL_TAMPON` sans la fenêtre serait
> livrer un mécanisme incapable de se déclencher — c'est-à-dire un contrôle
> qu'on ne verra jamais rouge, appliqué cette fois à un mécanisme de produit.
> **F3 livre donc les DEUX, ou aucun.**

**Décision** :

- **une fenêtre de lecture bornée**, `MORCEAUX_EN_VOL`, dans un module **PUR**
  (`agent/src/pont/lecture.rs`) : jusqu'à *k* morceaux demandés d'avance ;
- **la contre-pression est CÔTÉ NAVIGATEUR**, parce que c'est lui qui émet les
  gros messages et que `bufferedAmount` est une propriété de **son** canal. Le
  pont ne la voit pas et ne peut pas la voir. `client/src/fichiers/flux.ts`
  (**PUR**) attend `bufferedamountlow` avant de servir la requête suivante ;
  `client/src/fichiers/canal.ts` pose `bufferedAmountLowThreshold` — **il ne le
  pose pas aujourd'hui**, alors que la spec §3.4 l'exige (« `bufferedAmountLowThreshold` |
  posé ») : relevé, `canal.ts:95` ne passe que `{ ordered: true }` et `:118`
  envoie sans regarder quoi que ce soit ;
- **l'invariant qui rend la fenêtre sûre est écrit et testé** : le canal est
  `ordered` (`canal.ts:95`), les morceaux sont demandés dans l'ordre croissant
  des positions, donc les réponses arrivent dans cet ordre — et
  `PrjWriteFileData` est appelé dans cet ordre. Le module pur **vérifie** que la
  réponse reçue est bien celle attendue et journalise un `warn!` sinon, plutôt
  que de faire confiance à SCTP.

⚠️ **`MORCEAUX_EN_VOL` et `SEUIL_TAMPON` ne sont PAS calibrées.** Elles
rejoignent la liste. **Et une valeur de 1 rendrait la fenêtre inerte** : la
recette relève `morceaux_en_vol_max` au recensement (§0.6), et un maximum resté
à 1 pendant une lecture de 12 Mio est un **échec du livrable**, pas un détail.

⚠️ **F3 ne revendique AUCUN gain de débit.** La seule mesure de débit du dépôt
est incohérente d'un facteur ~120 (6,5 Mio/s contre 52–55 Kio/s, F1 §11), sans
explication. C'est F4 qui jugera ; F3 livre le mécanisme et le rend observable.

### 0.5 🔴 Les douze codes du §5 : la table est DÉJÀ écrite, ce qui manque est de l'EXERCER

**Relevé** : `agent/src/pont/erreurs.rs` porte déjà les **douze** variantes
d'`Erreur` (`:60-89`) et les douze `HRESULT` (`:144-160`), avec `NOMBRE = 12`
(`:100`) et un garde structurel à deux étages (`:91-99`). **La spec F3 dit
« livre […] la table des HRESULT du §5 dans son intégralité » ; elle est là
depuis F1.** Ce que F3 livre réellement, c'est le critère (4) : *chacun des
douze est observé au moins une fois au journal*.

**Et ce critère est aujourd'hui INSATISFIABLE, pour une raison relevée dans le
code** : la seule trace qui nomme la cause d'un refus est
`agent/src/pont/service.rs:140`,
`tracing::debug!(commande, correlation, ?cause, "le navigateur refuse")` —
un **`debug!`**, alors que `scripts/run-agent.sh:29` pose
`RUST_LOG = 'info'` par défaut et que la doctrine du dépôt est que
l'exploitation tourne en `info`. **Sur une recette ordinaire, aucune cause n'est
observable.** Monter tout le pont en `debug` inonderait le journal d'une ligne
par rappel — le piège « ne jamais tracer par paquet » du chantier TURN.

**Décision : un COMPTEUR par variante, pur, et une ligne de recensement.**
`agent/src/pont/compteurs.rs` (**PUR**, testé) tient un compteur par variante
d'`Erreur`, incrémenté au **seul** point où une cause devient un `HRESULT`, et
le fil du pont émet périodiquement **une** ligne `info!` :

```
codes rendus introuvable=3 chemin-introuvable=1 acces-refuse=0 canal-ferme=1 …
```

**Le critère (4) devient alors décidable par un `grep` sur une ligne**, il ne
peut pas être satisfait par accident (un code jamais produit affiche `0`), et
il coûte une ligne toutes les *N* secondes. C'est exactement l'intention que la
spec écrit — « empêcher la table du §5 d'être décorative » — rendue mesurable.

**Comment chacun des douze est atteint, et par quoi.** ⚠️ **Une injection prouve
que la table n'est pas décorative ; elle ne prouve PAS que la cause est
atteignable en exploitation.** Les deux colonnes sont donc distinguées, et le
document de résultats les gardera distinctes.

| Cause | HRESULT | Comment on l'atteint |
| --- | --- | --- |
| `Introuvable` | 0x80070002 | **geste réel** — `type` d'un chemin absent |
| `CheminIntrouvable` | 0x80070003 | **geste réel** — un chemin sous un répertoire absent |
| `AccesRefuse` | 0x80070005 | **injection** — OPFS n'a aucun modèle de permission (F1 §3) |
| `CanalFerme` | 0x8007045D | **geste réel** — fermer la page-shell en pleine lecture (c'est le critère 3) |
| `DelaiDepasse` | 0x80070079 | **injection** — un chemin auquel le navigateur ne répond jamais |
| `Abandonnee` | 0x800703E3 | **geste réel** — tuer le lecteur en pleine copie (`CancelCommand`) |
| `DisquePlein` | 0x80070070 | **injection** — `QuotaExceededError` n'est pas provocable sur OPFS |
| `NonSupporte` | 0x80070032 | **geste réel** — `mklink /H` dans la racine |
| `RepertoireNonVide` | 0x80070091 | **geste réel** — voir §0.6, c'est la suppression NON récursive qui le rend atteignable |
| `DejaPresent` | 0x80070050 | **geste réel** — renommer sur un nom qui existe déjà |
| `ProtegeEnEcriture` | 0x80070013 | **geste réel** — `PONT_MUTATION=0` refuse `PRE_RENAME`/`PRE_DELETE` |
| `Inattendue` | 0x8007001F | **geste réel** — `casse-ambigue`, ou un code d'échec illisible |

**Le mécanisme d'injection** : le canonicaliseur du navigateur reconnaît un
premier composant de chemin `.faute-<code>` et lève l'échec correspondant.
**Il est DÉSARMÉ par défaut** et n'est armé que par un paramètre d'URL de la
page-shell (`?faute-fichiers=1`), lu une fois et **passé en argument** au module
pur — jamais lu depuis le module lui-même, sans quoi il ne serait pas testable.
Le geste de recette est alors un simple `dir` dans la VM. ⚠️ *Ce n'est pas une
variable d'environnement de l'agent : il n'y a rien à ajouter à
`scripts/run-agent.sh` pour elle.*

> ⚠️ **DIVERGENCE RELEVÉE, non corrigée, et sa raison est écrite.**
> `agent/src/pont/service.rs:303-304` fait retomber **deux** codes du fil
> distincts — `CodeEchec::TropGrand` et `CodeEchec::Interne` — sur la **même**
> cause `Erreur::Inattendue`, donc le même `HRESULT`. La spec §5.1 énonce
> pourtant : « **deux causes distinctes ne partagent jamais un code** ».
> **F3 ne crée pas une treizième variante** : du point de vue de l'application,
> les deux sont « un défaut de notre côté », et inventer un `HRESULT` pour
> distinguer nos propres bogues ferait grossir une table que le critère (4)
> oblige ensuite à exercer. **Ce que F3 corrige, c'est la perte au JOURNAL** :
> la trace de refus porte désormais **le code du fil ET la cause**, et elle
> passe en `warn!`. La distinction survit là où elle sert — le diagnostic —, et
> la ligne du §5.1 est respectée dans son intention (l'ancien pont rendait
> `EPERM` à neuf sites) sans l'être dans sa lettre. **Relevé, tranché, déclaré.**

### 0.6 🔵 Ce que les `PRE_` achètent, que F2 n'avait pas — et pourquoi la suppression n'est PAS récursive

**F2 déclare que le chemin d'écriture n'a aucune contre-pression** (`…-f2.md`
§0.1 point 4) : les notifications qui l'informent sont des POST, l'application a
déjà reçu son succès. **F3 est dans une situation meilleure, et il faut le
dire** : `PRE_RENAME` (32) et `PRE_DELETE` (16) sont des **PRE**, donc
**refusables**, et le code les refuse déjà aujourd'hui
(`agent/src/pont/notifications.rs:115`). F3 peut donc refuser un renommage ou
une suppression **avant** qu'ils n'aient lieu.

⚠️ **Mais le refus ne peut porter que sur un ÉTAT, jamais sur une issue.** Les
`PRE_` sont **synchrones** et ne consultent jamais le navigateur (spec §4.3, et
`rappels.rs:403-406` le redit). Les états sur lesquels F3 refuse, et rien
d'autre :

| État | Décision au `PRE_` |
| --- | --- |
| canal `fichiers` fermé | **refuser** — `CanalFerme` → `ERROR_IO_DEVICE` |
| `PONT_MUTATION=0` | **refuser** — `ProtegeEnEcriture` |
| racine montée en lecture seule | **refuser** — `ProtegeEnEcriture` |
| la cible du renommage sort de la racine | **refuser** — `NonSupporte` (spec §4.3 : « accepte, sauf si la cible sort de la racine ») |
| sinon | **accepter** ; c'est la POST qui poussera |

**Et la suppression n'est PAS récursive côté navigateur.**

> ⚠️ **DIVERGENCE avec la spec §3.5**, qui écrit
> `dir.removeEntry(nom, { recursive })`. **F3 appelle `removeEntry(nom)` sans
> `recursive`.** Raison : `recursive: true` transforme **un** geste dans la VM
> en **destruction récursive** sur le disque du poste local, sur la foi d'un
> miroir qu'aucune preuve ne dit à jour. Windows, lui, ne supprime jamais un
> répertoire non vide en un geste : l'Explorateur et `rd /s` effacent les
> enfants un à un, et **chaque enfant produit sa propre notification**. Le
> miroir non récursif suit donc Windows pas à pas.
>
> 🔵 **Bénéfice second, et il n'est pas décoratif** : si le navigateur répond
> que le répertoire n'est pas vide, cela veut dire que **le miroir a dérivé** —
> et `Erreur::RepertoireNonVide` (`ERROR_DIR_NOT_EMPTY`) devient une cause
> **réelle et diagnostique** au lieu d'un code jamais produit. C'est ce qui la
> rend atteignable par un geste au tableau du §0.5.
>
> ⚠️ **Ce que cela suppose, et qui n'est pas mesuré** : que ProjFS émette bien
> une notification de suppression **par enfant**, y compris pour des enfants
> jamais énumérés ni hydratés. **C'est la question ③ de la sonde S1** (tâche
> 15). Si la réponse est non, la suppression d'un répertoire non vide laissera
> les enfants sur le poste local, et le critère (2) tombe pour sa seconde
> moitié — **dégrade, ne bloque pas**, et le repli est nommé au §9.

---

## 1. Contraintes globales

### 1.1 🔴 F2 n'est pas implémenté : ce que la tâche 1 relève, et les deux mondes

**Relevé par la commande le 20 août 2026** : `agent/src/pont/projfs/rappels.rs`
vaut **488** lignes, `proto/src/fichiers.rs` **157**, et
`proto/src/fichiers.rs:52-59` ne définit que `TYPE_LISTER`, `TYPE_ATTRIBUTS`,
`TYPE_LIRE`, `TYPE_ENTREES`, `TYPE_META`, `TYPE_DONNEES`, `TYPE_ECHEC`.
**Aucun verbe d'écriture n'existe, et l'extraction de `rappels.rs` prévue par la
tâche 3 de F2 n'a pas eu lieu.**

**F3 est planifié pour s'exécuter APRÈS F2.** La tâche 1 relève l'état réel, et
deux cas seulement :

| État relevé | Ce que F3 fait |
| --- | --- |
| **F2 fusionné** (`projfs/rappels/notification.rs` existe, `TYPE_ECRIRE` existe, `pont/ecriture.rs` existe) | Le cas nominal. La tâche 3 de F3 **ne fait rien** et le déclare ; les tâches 12 et 13 se branchent sur les modules de F2 |
| **F2 non fusionné** | **La tâche 3 de F2 devient la tâche 3 de F3, à l'identique et sous les mêmes noms de fichiers** (`projfs/rappels/listage.rs`, `projfs/rappels/notification.rs`) — pour qu'il n'existe pas un troisième découpage de ce fichier. Et **les règles du §0.3 deviennent sans objet** (il n'y a pas d'écriture due), ce qui doit être **écrit** dans le code et dans le document de résultats plutôt que laissé au silence |

⚠️ **Ce que F3 ne fait dans AUCUN des deux cas : implémenter l'écriture.** Si F2
n'est pas là, F3 livre le renommage et la suppression **au-dessus d'un pont en
lecture seule**, ce qui reste cohérent (renommer et supprimer ne font transiter
aucun octet) mais rend le critère (2) de F2 — l'idiome temp+rename — toujours
inatteignable. **C'est à dire, pas à découvrir.**

### 1.2 Références d'entrée, RELEVÉES PAR LA COMMANDE le 20 août 2026

| Contrôle | Commande | Relevé |
| --- | --- | --- |
| Plafond de 500 lignes | la commande de `CLAUDE.md` § Conventions | **deux** fichiers au-dessus, les deux entrées de dette gelée : `agent/src/encode.rs` **1536**, `agent/src/windows_source.rs` **630** |
| `agent/src/pont/projfs/rappels.rs` | `wc -l` | 🔴 **488**, marge **12** |
| `agent/src/pont/transport/tests.rs` | `wc -l` | ⚠️ **474**, marge 26 |
| `agent/src/superviseur/lanceur.rs` | `wc -l` | ⚠️ **488**, marge 12 — **F3 n'y touche pas** |
| `agent/src/demarrage.rs` | `wc -l` | ⚠️ **491**, marge 9 — **F3 n'y touche pas** |
| `scripts/run-agent.sh` | `wc -l`, `grep -n PONT` | **155** lignes ; `PONT` transmise **ligne 33** ; `RUST_LOG` par défaut `info` **ligne 29** |
| Tests du périmètre | `cargo test -p agent pont::` | **60 passed** |
| | `cargo test -p proto fichiers` | **11 passed** |
| | `cd client && npx vitest run src/fichiers` | **19 passed** |
| | `cd client && npx vitest run --dir ../proto ts/fichiers` | **41 passed** |

⚠️ **Les quatre suites COMPLÈTES sont à relancer par la tâche 1 et à inscrire au
document de résultats.** Les valeurs de fin de F1 (**614** Rust, **223** client,
**111** proto, **16** avertissements de compilation croisée) sont recopiées de
`…-f1-resultats.md` §10 et **n'ont pas été réexécutées ici** : l'arbre est
partagé avec les sous-projets ④ et ⑤, dont les commits postérieurs ont déjà fait
bouger des fichiers. **Ne pas s'en servir comme référence sans les relancer.**

### 1.3 🔴 `cd client && npx vitest run` NE COUVRE PAS `proto/ts/`

Fait établi par F1, à rejouer à chaque tâche qui touche `proto/ts/` :

```bash
cd client && npx vitest run                # client/src/ seul
cd client && npx vitest run --dir ../proto # proto/ts/
cd client && npx tsc --noEmit              # couvre les DEUX
```

**Un test ajouté à `proto/ts/fichiers-entetes.test.ts` et non lancé par la
seconde commande n'est pas un test.**

### 1.4 Règles de travail

- **Plafond de 500 lignes.** **L'extraction vient AVANT l'addition** — tâche 3,
  et rien n'entre dans `rappels.rs` avant elle. **Jamais de compression** : D9
  a compressé deux fichiers, geste que `CLAUDE.md` interdit nommément, puis a dû
  les extraire quand même.
- **Séparer le PUR du `#[cfg(windows)]`.** C'est ce qui a permis à F1 de jouer
  une quarantaine de mutations sur l'hôte. **Règle de ce plan, plus stricte que
  la spec §4.4** : *la fenêtre de lecture, la décision de mutation, le
  compteur de codes et le canonicaliseur de noms sont tous PURS.* Seul le
  déclenchement (le rappel de notification) est gaté.
- **Un contrôle qu'on n'a jamais vu rouge n'est pas un contrôle**, et **un plan
  est une source de contrôles vacueux** : D10 en a attrapé quatre dont **trois
  écrits par son propre plan**, F1 quatre de plus dont **deux qui auraient fait
  lire un succès comme un échec**. Chaque tâche porte son état rouge **et la
  vérification que cet état est atteignable**.
- **Toute chaîne de `grep` prescrite se vérifie contre le CODE, jamais contre la
  spec ni contre ce plan.** F1 a prescrit `pont lancé` (la trace est `pont
  fichiers lancé`), `ERROR_MOD_NOT_FOUND` (jamais émise) et
  `ERROR_SEM_TIMEOUT\|delai depasse` (le témoin réel est `commande expirée`).
  **Chaque `grep` de la tâche 17 est accompagné du `grep -rn` sur `agent/src/`
  ou `client/src/` qui établit que la chaîne est émise par le code livré.**
- ⚠️ **Vérifier la version de la bibliothèque qu'on lit.** `windows` et
  `windows-sys` exposent **deux** modules `Win32/Storage/ProjectedFileSystem`
  aux symboles identiques et aux lignes différentes ; l'auteur du plan de F2 a
  déclaré fausses trois citations exactes pour l'avoir oublié, et a consigné
  l'erratum plutôt que de l'effacer. **Le crate qui fait foi est celui que
  `agent/Cargo.toml` déclare — `windows` (0.62.2).** **La VALEUR fait foi,
  jamais le numéro de ligne.**
- **Jamais `git add -A`** : nommer les fichiers. `git commit` valide **tout
  l'index**, y compris avec `-F` ; **pathspec explicite**, et
  `git show --name-only` après coup. **Jamais `--amend`.**
- **`PONT_MUTATION` est ajoutée à `scripts/run-agent.sh` dans une tâche DÉDIÉE
  placée AVANT celle qui en a besoin** (tâche 2). Piège payé en D1
  (`SUPERVISEUR`), D2 (`MULTIFENETRE_REPRISE`), D7 (`AUDIO`) ; évité en D3, D6,
  F1 par exactement ce geste.
- **Convention de valeur** : `PONT_MUTATION=0` **désarme**, et le test est
  `matches!(std::env::var("PONT_MUTATION").as_deref(), Ok(v) if v != "0")` — la
  forme de `agent/src/main.rs` pour `CAPTEUR` et `PONT`. **Variable de BANC,
  jamais une configuration livrée.**
- **Convention de module enfant** (`CLAUDE.md`) : **aucun module de ce plan n'en
  relève.** Elle ne vise que les modules extraits d'un parent `#[cfg(windows)]`
  pour compiler sur l'hôte et devenus frères de premier niveau dans `main.rs`.
  `pont.rs` n'est pas gaté et ses enfants purs se déclarent par un simple `mod` ;
  `projfs/rappels/*` sont des enfants ordinaires d'un parent déjà gaté.
  **Aucun `#[path]` n'est écrit.**
- **La VM n'est pas démarrée automatiquement**, et **elle est un état partagé** :
  `virsh list --all`, `virsh start Windows`, attendre WinRM **puis** un accès
  réel à `/media/vm` (`until ls /media/vm/dev`), et `set -a && source .env &&
  set +a` avant `scripts/build-agent.sh` — sans quoi il s'arrête **en silence**.
  ⚠️ **Le chantier touche `proto` : `cargo clean --release -p proto -p agent`
  avant la construction, et vérifier la TAILLE du binaire.** Une compilation de
  0,13 s est un aveu.
- 🔴 **DEUX EXÉCUTIONS DE RECETTE NE SE CHEVAUCHENT JAMAIS.** F1 en a perdu
  une : deux se sont recouvertes de 2 min 23 s, la seconde a tué l'agent de la
  première **en pleine mesure**, et **le journal versé sous le nom de la
  première était celui de la seconde** — même horodatage de début à la
  microseconde près. `Get-Process agent` se revérifie **après chaque tentative,
  y compris échouée**, et chaque journal est copié **sous un nom unique
  immédiatement**.
- ⚠️ **Relever le PID du pont dans `agent.log`, jamais le deviner** : « le pont
  est le plus jeune des `agent` » est faux, l'ordre est superviseur, capteur,
  pont, **puis** les enfants.
- ⚠️ **Ne jamais tester une sous-chaîne d'un message d'interface** : « Lecteur …
  **mont**é » et « n'a pas pu être **mont**é » partagent `mont`, et un pilote de
  F1 a lancé une mesure de neuf minutes sur un pont non monté. **Le produit
  garde l'ambiguïté** ; l'instrument de F3 compare des messages **entiers**.
- ⚠️ **Toujours `grep -a`** sur les journaux : un fichier à queue d'octets NUL
  est classé « binaire » et `grep` rend une sortie **vide**, indiscernable d'un
  compte nul.
- ⚠️ **Un `|| echo` de repli transforme « fichier absent » en « contrôle
  vert ».** Aucun contrôle de ce plan n'en porte.
- **Aucun taux ne sera revendiqué.** **Deux exécutions par critère**, jamais
  une, et chaque énoncé porte son nombre.
- **Toute preuve d'une affirmation portée dans `CLAUDE.md` est versée dans
  git**, sous `docs/superpowers/plans/journaux-pont-fichiers-f3/`. D9 a perdu
  **six** constats de revue parce que sa preuve vivait dans un espace gitignoré,
  et la seule chose qu'on puisse en dire aujourd'hui est qu'ils sont perdus.

---

## 2. Structure des fichiers

### 2.1 Créés

| Fichier | Nature | Visé | Responsabilité |
| --- | --- | --- | --- |
| `agent/src/pont/projfs/rappels/listage.rs` | `#[cfg(windows)]` | ≤ 200 | **extraction** (si F2 ne l'a pas faite) — les trois rappels d'énumération |
| `agent/src/pont/projfs/rappels/notification.rs` | `#[cfg(windows)]` | ≤ 300 | **extraction puis addition** — le rappel de notification, et les deux notifications de F3 |
| `agent/src/pont/compteurs.rs` | **PUR** | ≤ 200 | Le compteur par variante d'`Erreur`, et l'unique point où une cause devient un `HRESULT` |
| `agent/src/pont/compteurs/tests.rs` | test | ≤ 150 | |
| `agent/src/pont/mutation.rs` | **PUR** | ≤ 350 | La file des mutations dues, l'entrelacement avec les écritures dues (§0.3), la décision d'un `PRE_` par ÉTAT |
| `agent/src/pont/mutation/tests.rs` | test | ≤ 300 | |
| `agent/src/pont/lecture.rs` | **PUR** | ≤ 250 | La fenêtre de morceaux en vol, et l'invariant d'ordre |
| `agent/src/pont/lecture/tests.rs` | test | ≤ 250 | |
| `client/src/fichiers/noms.ts` | **PUR** | ≤ 250 | Le canonicaliseur de nom — le remède de casse en LECTURE, et l'injection de faute |
| `client/src/fichiers/noms.test.ts` | test | ≤ 300 | deux faux : un sensible, un **insensible** à la casse |
| `client/src/fichiers/mutation.ts` | **PUR** | ≤ 300 | `renommer` (`move()` et le repli local), `supprimer` |
| `client/src/fichiers/mutation.test.ts` | test | ≤ 300 | faux **avec** et **sans** `move` — les deux branches sont testées |
| `client/src/fichiers/flux.ts` | **PUR** | ≤ 150 | La contre-pression `bufferedAmount` |
| `client/src/fichiers/flux.test.ts` | test | ≤ 150 | |
| `docs/superpowers/plans/2026-08-20-pont-fichiers-f3-resultats.md` | document | — | Le document de résultats **permanent** |
| `docs/superpowers/plans/journaux-pont-fichiers-f3/` | journaux | — | Les pièces versées, avec leur `LISEZ-MOI.md` |

### 2.2 Modifiés — tailles **RELEVÉES PAR LA COMMANDE le 20 août 2026**

| Fichier | Lignes | Marge | Ce que F3 y ajoute |
| --- | --- | --- | --- |
| `agent/src/pont/projfs/rappels.rs` | 🔴 **488** | **12** | **RIEN avant la tâche 3.** Après extraction : ≈ **300**, marge ≈ 200 |
| `agent/src/pont/notifications.rs` | **134** | 366 | `FILE_RENAMED` (128), `FILE_HANDLE_CLOSED_FILE_DELETED` (2048), le masque étendu, et `decider` qui prend un **état** |
| `agent/src/pont/erreurs.rs` | **163** | 337 | **rien de neuf** — `RepertoireNonVide` et `DejaPresent` cessent d'être `dead_code` |
| `agent/src/pont/service.rs` | **325** | 175 | les bras de réponse d'une mutation ; le recensement ; la trace de refus en `warn!` avec le code du fil |
| `agent/src/pont/service/verbes.rs` | **265** | 235 | l'émission de `Renommer` et `Supprimer` |
| `agent/src/pont/table.rs` | **174** | 326 | `DELAI_MUTATION` ; l'inventaire pour le recensement |
| `agent/src/pont/projfs/etat.rs` | **267** | 233 | la fenêtre de lecture branchée ; le nom **canonique** rendu à `PrjWritePlaceholderInfo` |
| `agent/src/pont/projfs.rs` | **323** | 177 | le masque étendu au `PRJ_NOTIFICATION_MAPPING` (`:187-189`) |
| `agent/src/pont.rs` | **151** | 349 | la lecture de `PONT_MUTATION`, le câblage du compteur |
| `proto/src/fichiers.rs` | **157** | 343 | `TYPE_RENOMMER`, `TYPE_SUPPRIMER` ; `CodeEchec::RepertoireNonVide` |
| `proto/src/fichiers/entetes.rs` | **109** | 391 | `Renommer`, `Supprimer` |
| `proto/ts/fichiers.ts` | **181** | 319 | les jumeaux |
| `proto/ts/fichiers-entetes.ts` | **208** | 292 | les jumeaux |
| `proto/fichiers-vectors.json` | **88** | — | les vecteurs des deux formes neuves, et du code d'échec neuf |
| `client/src/fichiers/adaptateur.ts` | **289** | 211 | branche le canonicaliseur ; étend les poignées injectées (`removeEntry`, `move?`) ; **délègue** les mutations |
| `client/src/fichiers/protocole.ts` | **128** | 372 | servir `RENOMMER` et `SUPPRIMER` |
| `client/src/fichiers/canal.ts` | **213** | 287 | `bufferedAmountLowThreshold`, et la contre-pression avant `send` (`:118`) |
| `client/src/shell.ts` | **154** | 346 | la ligne « mutation en échec », avec le chemin et le motif |
| `client/src/shell-page.ts` | **221** | 279 | l'armement `?faute-fichiers=1`, et l'affichage |
| `scripts/run-agent.sh` | **155** | — | +1 ligne `PONT_MUTATION` |
| `CLAUDE.md` | — | — | tâche 18 |

> 🔴 **`agent/src/pont/projfs/rappels.rs` À 488 LIGNES EST LE SEUL BLOCAGE DUR
> DE CE PLAN, et il l'était déjà pour F2.** Marge **12**. Le rappel
> `notification` (`:407-438`) est exactement ce que F3 fait grossir : il doit
> lire `isdirectory` (`:409`, aujourd'hui `_est_repertoire`), lire
> `destinationfilename` (`:411`, aujourd'hui `_destination`), et aiguiller deux
> notifications de plus. **L'extraction se place AVANT** — c'est le seul geste
> qui ait fonctionné dans ce dépôt (D9, `capteur/serveur/instances.rs`, marge
> rendue de 10 à 65).

> ⚠️ **Marges étroites du VOISINAGE, relevées par la commande, que F3 ne touche
> pas et que la tâche 18 vérifiera qu'il n'a pas touchées** :
> `agent/src/superviseur/lanceur.rs` **488** (marge 12),
> `agent/src/demarrage.rs` **491** (marge 9),
> `agent/src/pont/transport/tests.rs` **474** (marge 26),
> `agent/src/encode/arret.rs` **500** (marge 0),
> `client/verify-webrtc.mjs` **494**.

### 2.3 Où l'union `PRJ_NOTIFICATION_PARAMETERS` n'est TOUJOURS pas déréférencée

**Relevé dans `windows-0.62.2/src/Windows/Win32/Storage/ProjectedFileSystem/mod.rs`
(621 lignes, relevé par la commande le 20 août 2026, chaque ligne relue par
`sed -n`)** :

| Symbole | Valeur | `mod.rs:` |
| --- | --- | --- |
| `PRJ_NOTIFICATION_CB` (la signature du rappel) | — | **334** |
| `PRJ_NOTIFICATION_FILE_RENAMED` | 128 | **341** |
| `PRJ_NOTIFICATION_FILE_HANDLE_CLOSED_FILE_DELETED` | 2048 | **335** |
| `PRJ_NOTIFY_FILE_RENAMED` | 128 | **386** |
| `PRJ_NOTIFY_FILE_HANDLE_CLOSED_FILE_DELETED` | 2048 | **380** |
| `PRJ_NOTIFICATION_PARAMETERS` (l'union) | — | **352** |
| `PRJ_NOTIFICATION_PARAMETERS_1 { NotificationMask }` | — | **369-371** |
| `PRJ_NOTIFICATION_PARAMETERS_2 { IsFileModified }` | — | **364-366** |

🔵 **Le nom de destination d'un renommage est un PARAMÈTRE DIRECT du rappel,
pas un membre de l'union** : `destinationfilename: windows_core::PCWSTR`
(`mod.rs:334`). **F3 n'a donc AUCUNE raison de déréférencer l'union**, et il ne
le fait pas — lire le mauvais membre d'une union est un comportement indéfini, et
ne pas la lire du tout est le seul moyen sûr. C'est la même décision que la
tâche 12 de F2, prise pour la même raison, et **elle survit à l'addition de F3**.

⚠️ **Aucun des trois membres de l'union ne sert à F3** : `FileRenamed.NotificationMask`
et `PostCreate.NotificationMask` servent à **changer** le masque d'une entrée en
cours de route, ce que F3 ne fait pas ; `FileDeletedOnHandleClose.IsFileModified`
ne dit rien qu'on ne sache déjà.

---

## 3. Interfaces partagées

**Fixées ici** ; les tâches les consomment telles quelles.

### 3.1 Le protocole — deux types de plus, un code d'échec de plus

```
Requêtes pont → navigateur (attendent une réponse)
  TYPE_LISTER    = 1   (F1)
  TYPE_ATTRIBUTS = 2   (F1)
  TYPE_LIRE      = 3   (F1)
  TYPE_ECRIRE    = 4   (F2)
  TYPE_CREER     = 5   (F2)
  TYPE_RENOMMER  = 7   (F3)   en-tête Renommer, charge vide
  TYPE_SUPPRIMER = 8   (F3)   en-tête Supprimer, charge vide

Annonces pont → navigateur (n'attendent RIEN)
  TYPE_DUES      = 6   (F2)

Réponses navigateur → pont
  TYPE_ENTREES   = 64  (F1)
  TYPE_META      = 65  (F1)
  TYPE_DONNEES   = 66  (F1)
  TYPE_FAIT      = 67  (F2)
  TYPE_ECHEC     = 127 (F1)
```

⚠️ **`TYPE_RENOMMER` vaut 7 et non 6** : F2 réserve 6 pour `TYPE_DUES`. Si F2
n'a pas été fusionné (§1.1), **les valeurs 4, 5, 6 restent réservées et ne sont
PAS réutilisées** — un trou dans la numérotation coûte moins qu'une valeur qui
change de sens entre deux branches. La tâche 4 pose une note à cet endroit et
un test qui épingle les valeurs.

En-têtes (`proto/src/fichiers/entetes.rs` + `proto/ts/fichiers-entetes.ts`,
**épinglés par `proto/fichiers-vectors.json`, lu des DEUX côtés** —
`proto/src/fichiers/entetes/tests.rs:21` et
`proto/ts/fichiers-entetes.test.ts:2`) :

```rust
pub struct Renommer {
    pub de: String,
    pub vers: String,
    /// `true` si l'entrée renommée est un répertoire — c'est `isdirectory` du
    /// rappel, transporté tel quel : le navigateur n'a pas à le redécouvrir.
    pub repertoire: bool,
}

pub struct Supprimer {
    pub chemin: String,
    pub repertoire: bool,
}
```

`CodeEchec` gagne une variante, **la seule de F3** :

```rust
RepertoireNonVide,   // sur le fil : "repertoire-non-vide"
```

⚠️ **Elle doit être ajoutée AUX DEUX BOUTS et à `CODES_ECHEC`
(`proto/ts/fichiers.ts:80-88`), et son octet exact épinglé par un vecteur.** Ce
dépôt a laissé passer une variante `battement-recu` verte sur cinquante tests
parce que rien n'épinglait ses octets ; `proto/src/fichiers.rs:63-67` porte
l'avertissement, et `repertoire-non-vide` est **à trois mots**, donc de la
famille exacte qui casse en silence.

⚠️ **`casse-ambigue` appartient à F2** (sa tâche 9). Si F2 n'est pas fusionné,
**F3 l'ajoute** — le canonicaliseur de la tâche 9 de F3 en a besoin —, et la
tâche 4 le déclare plutôt que de le supposer présent.

### 3.2 Côté agent

```rust
// agent/src/pont/compteurs.rs — PUR
pub struct Compteurs { /* [u64; erreurs::NOMBRE] */ }
impl Compteurs {
    /// Le SEUL point où une cause devient un HRESULT.
    pub fn rendre(&self, cause: Erreur) -> i32;
    /// Une ligne de recensement, ordre = `Erreur::TOUTES`.
    pub fn recensement(&self) -> String;
    pub fn total(&self) -> u64;
    pub fn manquants(&self) -> Vec<Erreur>;   // ceux à zéro — le critère (4)
}

// agent/src/pont/mutation.rs — PUR
pub enum Mutation { Renommer { de: String, vers: String, repertoire: bool },
                    Supprimer { chemin: String, repertoire: bool } }

/// Ce que le rappel PRE_ doit répondre, à partir d'un ÉTAT seul.
pub enum Prealable { Accepter, Refuser(Erreur) }
pub fn decider_prealable(etat: &EtatMutation, quoi: &Mutation) -> Prealable;

/// L'entrelacement du §0.3 : ce qu'il faut faire AVANT de pousser.
pub enum Ordonnancement {
    Pousser,
    AttendreEcrituresDues { chemins: Vec<String> },
    AbandonnerEcrituresDues { chemins: Vec<String> },
}
pub fn ordonnancer(dues: &[String], quoi: &Mutation) -> Ordonnancement;

// agent/src/pont/lecture.rs — PUR
pub struct Fenetre { /* morceaux restants, en vol, prochain attendu */ }
impl Fenetre {
    pub const MORCEAUX_EN_VOL: usize;      // ⚠️ NON CALIBRÉE
    pub fn a_demander(&mut self) -> Vec<Morceau>;   // au plus MORCEAUX_EN_VOL
    pub fn recu(&mut self, position: u64) -> Result<(), HorsOrdre>;
    pub fn en_vol_max(&self) -> usize;     // relevé au recensement
}
```

### 3.3 Côté navigateur

```ts
// client/src/fichiers/noms.ts — PUR
export type Resolution =
    | { sorte: 'trouve'; nom: string }        // le nom CANONIQUE, stocké
    | { sorte: 'absent' }
    | { sorte: 'ambigu'; noms: string[] };

export async function canoniser(
    parent: PoigneeRepertoire,
    demande: string,
): Promise<Resolution>;

// client/src/fichiers/mutation.ts — PUR
export async function renommer(racine, de, vers, repertoire): Promise<void>;
export async function supprimer(racine, chemin, repertoire): Promise<void>;

// client/src/fichiers/flux.ts — PUR
export function contrePression(canal: CanalSortant): {
    avantEnvoi(): Promise<void>;
};
```

⚠️ **Les poignées injectées d'`adaptateur.ts:89-94` gagnent deux membres**, et
c'est ce qui rend les DEUX branches du renommage testables sur l'hôte :

```ts
interface PoigneeRepertoire {
    getDirectoryHandle(nom: string, options?: { create?: boolean }): Promise<PoigneeRepertoire>;
    getFileHandle(nom: string, options?: { create?: boolean }): Promise<PoigneeFichier>;
    values(): AsyncIterable<PoigneeBase>;
    removeEntry(nom: string): Promise<void>;          // ⚠️ SANS `recursive` (§0.6)
    move?(parent: PoigneeRepertoire, nom: string): Promise<void>;   // NON STANDARD
}
```

🔵 **`move?` est OPTIONNEL dans le type, et c'est ce qui permet d'écrire deux
faux — l'un qui l'expose, l'autre non — et de voir les deux branches vertes sur
l'hôte.** Un type qui l'imposerait rendrait le repli inatteignable par un test.

---

## 4. Les tâches

### Task 1 : contrôle d'entrée et relevé de référence — AUCUN code

**Objet** — savoir dans lequel des deux mondes du §1.1 on se trouve, et fixer
les références d'entrée.

**Fichiers** — aucun. Le relevé va au document de résultats (tâche 18).

**À relever, par la commande, et à inscrire** :

```bash
# Dans quel monde sommes-nous ? (§1.1)
ls agent/src/pont/projfs/rappels/ 2>/dev/null || echo "ABSENT : F2 non fusionné"
grep -n 'TYPE_ECRIRE\|TYPE_CREER\|TYPE_DUES\|TYPE_FAIT' proto/src/fichiers.rs
ls agent/src/pont/ecriture.rs agent/src/pont/journal.rs 2>/dev/null
grep -n 'casse-ambigue' proto/src/fichiers.rs proto/ts/fichiers.ts

# Les quatre suites, COMPLÈTES
cd agent && cargo test -p agent 2>&1 | tail -3
cd agent && cargo test -p proto 2>&1 | tail -3
cd client && npx vitest run 2>&1 | tail -3
cd client && npx vitest run --dir ../proto 2>&1 | tail -3
cd agent && cargo check --target x86_64-pc-windows-gnu 2>&1 | tail -3

# Le plafond
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>430'
```

⚠️ **Ce n'est pas un contrôle et ce plan ne le déguise pas en contrôle** : c'est
un relevé. Il n'a pas d'état rouge, il a des valeurs. Ce qui serait un défaut,
c'est de **ne pas le faire** et de recopier les chiffres de F1.

**Dépendances** — aucune. **Première tâche.**

---

### Task 2 : `scripts/run-agent.sh` transmet `PONT_MUTATION` — tâche DÉDIÉE

**Objet** — que la variable puisse **atteindre le processus** avant que
quiconque en ait besoin.

**Fichiers modifiés** — `scripts/run-agent.sh` (**155** lignes ; `PONT` y est
transmise **ligne 33**, `RUST_LOG` **ligne 29**).

**Une ligne, de la forme exacte des voisines** :

```sh
${PONT_MUTATION:+\$env:PONT_MUTATION = '$PONT_MUTATION'}
```

**Rouge, et il est atteignable** : retirer la ligne, lancer avec
`PONT_MUTATION=0`, et vérifier au journal que le pont **n'annonce pas** son
désarmement. **Le contrôle qui vaut n'est pas de relire le script mais de lire
la trace côté VM** — c'est précisément ce que D7 a appris en découvrant qu'`AUDIO`
avait été vérifiée « en traçant le code », que le tracé était juste, et que la
valeur ne pouvait simplement pas atteindre le processus.

**Trace de contrôle**, émise **seulement si désarmé** (convention `PART_SONDAGE`
de D9) :

```
mutations DESARMEES (PONT_MUTATION=0) : renommage et suppression refusés au PRE_, rien n'est poussé
```

**Dépendances** — aucune. **À jouer tôt.**

---

### Task 3 : EXTRAIRE `projfs/rappels.rs` — AVANT d'y ajouter quoi que ce soit

**Objet** — rendre de la marge à un fichier qui n'en a que **12**.

**Conditionnelle** : si la tâche 3 de F2 a déjà eu lieu (`rappels/listage.rs` et
`rappels/notification.rs` existent), **cette tâche ne fait rien et le déclare
dans son rapport, avec la sortie de `ls`**. Sinon elle est **la tâche 3 de F2,
jouée à l'identique**, sous les mêmes noms.

**Fichiers créés** — `agent/src/pont/projfs/rappels/listage.rs` (les trois
rappels d'énumération : `debut_enumeration` `:153`, `fin_enumeration` `:173`,
`suite_enumeration` `:194`), `agent/src/pont/projfs/rappels/notification.rs`
(`notification` `:407-438`).
**Fichiers modifiés** — `agent/src/pont/projfs/rappels.rs`.

⚠️ **Pourquoi `listage.rs` et non `enumeration.rs`** : `agent/src/pont/enumeration.rs`
existe déjà, il est **PUR** (140 lignes), et deux modules homonymes dont l'un est
pur et l'autre gaté est une homonymie qu'on ne remarque qu'en relisant un renvoi.
La décision est écrite dans l'en-tête du fichier extrait. *(Reprise verbatim du
§2.3 de F2, pour qu'il n'y ait qu'une seule raison écrite dans le dépôt.)*

⚠️ **Le garde d'ABI part AVEC sa fonction.** `rappels.rs:61-68` porte huit
`const _: PRJ_*_CB = Some(...)` ; celui du rappel de notification suit
`notification` dans `notification.rs`. C'est le **seul** garde d'ABI du dépôt, et
`rappels.rs:52-59` explique pourquoi c'est un `const _` et non un `#[test]`.
**Le laisser derrière serait le désarmer.**

**Transposition VERBATIM** : aucune ligne de corps modifiée. **Le contrôle est
une comparaison texte à texte**, `git show HEAD:…rappels.rs | sed -n '153,260p'`
contre le fichier extrait, versée au journal.

**Rouge** — `wc -l agent/src/pont/projfs/rappels.rs` doit rendre **≈ 300**, pas
488. Un fichier resté à 488 est l'aveu que l'extraction n'a pas eu lieu.

**Dépendances** — aucune. **Elle BLOQUE les tâches 12 et 13.**

---

### Task 4 : `proto/{src,ts}/fichiers` — deux types, deux en-têtes, un code d'échec

**Objet** — mettre le renommage et la suppression sur le fil, épinglés des deux
côtés par les vecteurs partagés.

**Fichiers modifiés** — `proto/src/fichiers.rs`, `proto/src/fichiers/entetes.rs`,
`proto/src/fichiers/tests.rs`, `proto/src/fichiers/entetes/tests.rs`,
`proto/ts/fichiers.ts`, `proto/ts/fichiers-entetes.ts`,
`proto/ts/fichiers.test.ts`, `proto/ts/fichiers-entetes.test.ts`,
`proto/fichiers-vectors.json`.

**Contenu** — §3.1. `TYPE_RENOMMER = 7`, `TYPE_SUPPRIMER = 8`,
`CodeEchec::RepertoireNonVide`, et (si F2 n'est pas fusionné)
`CodeEchec::CasseAmbigue`. `FICHIERS_VERSION` **reste 1** : les types neufs sont
additifs et un pair ancien les rejette déjà par son aiguillage nommé
(`client/src/fichiers/protocole.ts:80-86` journalise et rend `null`).

**Vecteurs neufs dans `proto/fichiers-vectors.json`** (il en porte **12**
aujourd'hui, `version: 1`) : `renommer`, `renommer_repertoire`,
`renommer_casse_pure`, `supprimer`, `echec_repertoire_non_vide`.

**Tests, et leur ROUGE** :

| Test | Rouge, atteignable comment |
| --- | --- |
| `un_type_de_message_a_une_valeur_epinglee` | changer `TYPE_RENOMMER` de 7 à 6 → collision avec `TYPE_DUES`, le test nomme la valeur |
| `conformite_aux_vecteurs_partages` (Rust, `entetes/tests.rs:19`) | renommer le champ `vers` en `destination` d'**un seul côté** → le vecteur casse le côté fautif |
| l'`it.each(cas)` TS (`fichiers-entetes.test.ts:96`, `:100`) | idem, côté TS, **et le vecteur nomme le cas** |
| `un_code_d_echec_a_une_forme_epinglee_sur_le_fil` | écrire `repertoireNonVide` au lieu de `repertoire-non-vide` → l'octet exact est comparé |

⚠️ **Les deux commandes Vitest, pas une** (§1.3). Un vecteur ajouté et non lu
par `--dir ../proto` n'épingle **qu'une implémentation sur deux**, ce qui est
exactement l'inverse de ce à quoi il sert.

**Dépendances** — tâche 1 (pour savoir si `casse-ambigue` est là).

---

### Task 5 : `agent/src/pont/compteurs.rs` — l'instrument du critère (4)

**Objet** — rendre les douze codes **observables au journal en `RUST_LOG=info`**,
et faire du critère (4) un `grep` sur une ligne.

**Fichiers créés** — `agent/src/pont/compteurs.rs`, `compteurs/tests.rs`.
**Fichiers modifiés** — `agent/src/pont.rs` (déclaration du module et du
compteur), `agent/src/pont/service.rs` (l'émission périodique), tous les sites
d'appel de `erreurs::hresult`.

**L'invariant, et c'est lui le livrable** :

> **`erreurs::hresult` n'a plus qu'UN seul appelant hors tests :
> `compteurs::rendre`.** Tous les autres sites passent par le compteur.

**Le contrôle qui l'établit** est écrit dans le plan **et vérifié atteignable**,
parce qu'un contrôle de ce genre est précisément ce qui se met à mentir :

```bash
grep -rn --include='*.rs' 'hresult(' agent/src/pont/ \
  | grep -v '/tests\.rs:\|/tests/\|erreurs\.rs:\|compteurs\.rs:' \
  | grep -v ':[0-9]*: *//'
```

**Doit rendre ZÉRO ligne.** ⚠️ **Rouge, et il l'est aujourd'hui — la commande a
été LANCÉE le 20 août 2026 et rend 23 lignes de code** : `service.rs`
(`:86`, `:103`, `:141`, `:182`, `:203`, `:220`, `:253`, `:263`, `:281`),
`projfs.rs:283`, `projfs/rappels.rs` (`:206`, `:256`, `:268`, `:285`, `:301`,
`:343`, `:357`, `:381`, `:415`), `projfs/etat.rs:229`, `service/verbes.rs`
(`:156`, `:195`, `:199`). **Le contrôle est donc vérifié capable de dénoncer
l'état qu'il existe pour dénoncer, avant même que la tâche ne commence** — et sa
sortie d'avant est versée au journal.

⚠️ **Le second `grep -v` n'est pas une coquetterie** : sans lui la commande
attrape `notifications.rs:97`, une ligne de **doc-comment** (« Le rappel rend
`hresult(cause)` »), et le contrôle rendrait donc `1` pour toujours — un
contrôle qui ne peut **jamais redevenir vert**. *C'est le pendant exact du
`|| echo` de F1, qui rendait un contrôle incapable de devenir ROUGE.*

**La ligne de recensement**, émise par le fil du pont toutes les
`PERIODE_RECENSEMENT` (**non calibrée**, proposée à 10 s) et **une dernière fois
à l'arrêt** :

```
codes rendus total=17 introuvable=3 chemin-introuvable=1 acces-refuse=0 canal-ferme=1 delai-depasse=2 abandonnee=0 disque-plein=0 non-supporte=1 repertoire-non-vide=0 deja-present=1 protege-en-ecriture=8 inattendue=0
```

⚠️ **L'ordre est celui d'`Erreur::TOUTES` (`erreurs.rs:104-117`), et un test
l'épingle** — un recensement dont l'ordre dériverait ferait lire un compteur
pour un autre, ce qui est le piège des « deux messages qui partagent une
sous-chaîne » sous une autre forme.

**Tests, et leur ROUGE** :

| Test | Rouge, atteignable comment |
| --- | --- |
| `chaque_variante_a_son_compteur` | ajouter une variante à `Erreur` sans l'ajouter au recensement → `NOMBRE` fait échouer la compilation, puis le test nomme la manquante |
| `un_recensement_neuf_est_a_zero_partout` | initialiser à 1 → le test le voit |
| `rendre_incremente_la_bonne_case_et_elle_seule` | incrémenter `index(e)+1` → deux compteurs faux, et le test les nomme tous les deux |
| `manquants_rend_exactement_les_causes_a_zero` | rendre `Vec::new()` inconditionnellement → **c'est le rouge le plus important : un `manquants()` toujours vide ferait déclarer le critère (4) TENU sur une exécution où rien n'a été exercé** |
| `l_ordre_du_recensement_est_celui_de_TOUTES` | permuter deux noms → le test compare la chaîne entière |

**Dépendances** — aucune (module pur). Elle **précède** les tâches 12, 13, 17.

---

### Task 6 : `agent/src/pont/notifications.rs` — le masque étendu, et la décision par ÉTAT

**Objet** — accepter le renommage et la suppression, et savoir **quand les
refuser**, sans jamais consulter le navigateur.

**Fichiers modifiés** — `agent/src/pont/notifications.rs` (**134**, marge 366),
`agent/src/pont/notifications/tests.rs`, `agent/src/pont/projfs.rs:183-191` (le
`PRJ_NOTIFICATION_MAPPING`).

**Constantes neuves, avec leur ligne source relevée le 20 août 2026 dans
`windows-0.62.2` — et la VALEUR fait foi, pas la ligne** (§1.4) :

```rust
pub const FILE_RENAMED: i32 = 128;                       // mod.rs:341
pub const FILE_HANDLE_CLOSED_FILE_DELETED: i32 = 2048;   // mod.rs:335
pub const NOTIFY_FILE_RENAMED: u32 = 128;                // mod.rs:386
pub const NOTIFY_FILE_HANDLE_CLOSED_FILE_DELETED: u32 = 2048; // mod.rs:380
```

**`MASQUE` passe de cinq à sept bits.** Le fichier porte déjà deux gardes que
cette addition doit faire passer :
`le_masque_demande_exactement_les_cinq_notifications_de_f1` — **à renommer, pas
à rallonger en silence** — et `chaque_bit_du_masque_a_une_decision_nommee`, qui
balaie les 32 bits.

**`decider` change de signature** : elle prend désormais un **état**, parce que
la décision d'un `PRE_` en dépend (§0.6).

```rust
pub struct EtatPont { pub canal_ouvert: bool, pub mutations_armees: bool,
                      pub lecture_seule: bool }
pub fn decider(etat: &EtatPont, code: i32, sort_de_la_racine: bool) -> Reponse;
```

⚠️ **`sort_de_la_racine` est calculé par `chemins::normaliser` sur la
destination, PAS par `decider`** : la spec §4.3 dit « accepte, sauf si la cible
sort de la racine », et `chemins.rs` est déjà le module qui refuse les `..`, les
`:` et les noms réservés (`:92-129`). Le dupliquer ici en ferait deux vérités.

**Tests, et leur ROUGE** :

| Test | Rouge, atteignable comment |
| --- | --- |
| `le_masque_demande_exactement_les_sept_notifications_de_f3` | oublier `NOTIFY_FILE_RENAMED` → la notification n'arrive jamais et **rien ne le dit** ; le test le dit |
| `chaque_bit_du_masque_a_une_decision_nommee` (existant) | ajouter un bit sans bras → il retombe dans le fourre-tout, le test le nomme |
| `un_pre_rename_est_refuse_canal_ferme` | rendre `Accepter` inconditionnellement → un renommage accepté alors que personne ne peut le pousser, **c'est-à-dire la perte silencieuse** |
| `un_pre_rename_est_refuse_si_les_mutations_sont_desarmees` | idem, sur `PONT_MUTATION=0` |
| `un_pre_rename_hors_racine_est_NonSupporte_et_pas_ProtegeEnEcriture` | rendre `ProtegeEnEcriture` → deux causes partagent un code, ce que §5.1 interdit |
| `un_file_renamed_est_AccepterEnPoussant` | le laisser en `AccepterSansAttendre` → la POST est avalée par le fourre-tout et **rien n'est jamais poussé**, sur un produit qui a l'air de marcher |

**Dépendances** — tâche 4 (pour `Mutation`, si l'on y met le type). **Pure,
parallélisable.**

---

### Task 7 : `agent/src/pont/mutation.rs` — l'entrelacement avec les écritures dues

**Objet** — que l'idiome *écrire-temporaire / renommer / supprimer* ne perde
pas l'enregistrement (§0.3).

**Fichiers créés** — `agent/src/pont/mutation.rs`, `mutation/tests.rs`.

**Contenu** — §3.2 : `ordonnancer(dues, quoi)`, la file des mutations dues, et
la sérialisation par chemin.

⚠️ **Si F2 n'est pas fusionné**, `dues` est toujours vide et `ordonnancer` rend
toujours `Pousser`. **Le module est écrit et testé quand même**, avec ses tests
sur des `dues` non vides — c'est du code qui ne court pas, et **le dire est
moins cher que de le réécrire quand F2 arrive**. La ligne « ce cas n'est pas
atteignable tant que F2 n'est pas fusionné » est écrite auprès de la fonction.

**Tests, et leur ROUGE** :

| Test | Rouge, atteignable comment |
| --- | --- |
| `un_renommage_attend_les_ecritures_dues_sur_la_source` | rendre `Pousser` → l'écriture arrive après, sur un chemin disparu, **et l'enregistrement de LibreOffice est perdu** |
| `une_suppression_abandonne_les_ecritures_dues_sur_le_chemin` | rendre `Pousser` → le fichier que l'utilisateur vient d'effacer **réapparaît** sur son poste |
| `une_ecriture_due_sur_un_AUTRE_chemin_ne_retarde_rien` | comparer par préfixe au lieu d'égalité → toute écriture bloque tout renommage |
| `une_ecriture_due_sur_un_ENFANT_du_repertoire_renomme_retarde` | comparer par égalité seule → le cas du répertoire passe à travers. ⚠️ *Les deux tests ci-dessus se contredisent si l'on confond fichier et répertoire : c'est `repertoire` qui tranche, et c'est pour cela qu'il est transporté* |
| `une_mutation_par_chemin_a_la_fois` | autoriser deux en vol → deux renommages du même chemin se croisent |

**Dépendances** — tâche 4. **Pure, parallélisable.**

---

### Task 8 : `agent/src/pont/lecture.rs` et `table.rs` — la fenêtre, et le recensement qui rend le legs 4 diagnosticable

**Objet** — livrer la moitié « pont » du contrôle de flux (§0.4), et **rendre
observable** ce que F1 lègue en n°4 : *des lectures calent sans jamais expirer,
`commande expirée` reste à 0 pendant 540 s, et on ne sait pas où le blocage se
produit, faute d'une trace à l'inscription en table.*

**Fichiers créés** — `agent/src/pont/lecture.rs`, `lecture/tests.rs`.
**Fichiers modifiés** — `agent/src/pont/table.rs` (**174**, marge 326) :
`DELAI_MUTATION` (⚠️ **un QUATRIÈME budget, là où la spec §5.3 en pose trois** —
divergence déclarée), et un inventaire pour le recensement.

**Le recensement, une ligne `info!` toutes les `PERIODE_RECENSEMENT`** :

```
pont en vol=2 plus_ancienne_ms=1840 sessions=1 morceaux_en_vol_max=4 completions=118
```

🔵 **C'est ce qui départage les quatre hypothèses du legs 4, et aucune n'était
départageable jusqu'ici** :

| Ce que le recensement montre | Ce que cela dit du blocage |
| --- | --- |
| `en vol=0` alors que l'application est figée | **rien n'a jamais été inscrit** : le blocage est dans le rappel, ou avant lui |
| `en vol=N` et `plus_ancienne_ms` qui croît **au-delà du budget** | la table ne balaie plus : le fil du pont est sorti de sa boucle |
| `en vol=N` et `plus_ancienne_ms` borné par le budget | l'inscription et l'expiration marchent : le blocage est ailleurs, `completions` le dira |
| plus aucune ligne de recensement | **le fil du pont est mort**, ce que rien ne disait |

⚠️ **Une ligne toutes les 10 s, pas une par rappel.** Le chantier TURN a payé
18 619 lignes en quelques secondes pour une trace par paquet, écrite sur un
partage CIFS depuis la boucle : **la mesure détruisait ce qu'elle mesurait.**
Compter et recenser, jamais tracer par événement.

**Tests, et leur ROUGE** :

| Test | Rouge, atteignable comment |
| --- | --- |
| `la_fenetre_ne_demande_jamais_plus_de_MORCEAUX_EN_VOL` | retirer la borne → la file SCTP se remplit et le canal devient la source de latence de tout le reste |
| `la_fenetre_atteint_reellement_MORCEAUX_EN_VOL_sur_une_lecture_longue` | ⚠️ **c'est le rouge du livrable lui-même** : avec `MORCEAUX_EN_VOL = 1` le mécanisme est **inerte**, et ce test le dénonce. Sans lui, F3 livrerait un contrôle de flux incapable de mordre |
| `une_reponse_hors_ordre_est_denoncee_et_pas_appliquee` | l'appliquer quand même → un fichier dont les plages sont dans le désordre, **et le condensat SHA-256 est le seul critère qui l'attraperait** |
| `un_morceau_de_taille_nulle_ne_fait_pas_avancer_la_fenetre` | le compter → la lecture se termine avant la fin du fichier |
| `le_recensement_nomme_la_plus_ancienne_en_vol` | rendre 0 → le legs 4 reste indiagnosticable, et c'est exactement l'état d'aujourd'hui |

**Dépendances** — tâche 5 (recensement au même endroit). **Pure.**

---

### Task 9 : `client/src/fichiers/noms.ts` — le canonicaliseur, et le remède de casse en LECTURE

**Objet** — que `casse.txt` cesse de désigner autre chose que ce qu'il nomme, et
que le nom rendu au système soit **celui du poste local**.

**Fichiers créés** — `client/src/fichiers/noms.ts`, `noms.test.ts`.

**Contenu** — la règle du §0.2, plus l'injection de faute du §0.5 (le premier
composant `.faute-<code>`, **armée par argument, jamais lue depuis le module**).

**Le faux qui rend les tests décisifs, et c'est le cœur de la tâche** : deux
systèmes de fichiers en mémoire, **l'un sensible à la casse** (comme OPFS et
comme Linux), **l'autre insensible** (comme Windows et macOS par défaut, donc
comme le poste local réel). **Le même canonicaliseur doit rendre la même
réponse sur les deux.**

| Test | Rouge, atteignable comment |
| --- | --- |
| `sur_un_faux_INSENSIBLE_casse_txt_ne_rend_pas_Casse_txt` | retirer le canonicaliseur → **le faux rend le contenu de `Casse.txt`**, c'est-à-dire **la moitié navigateur du défaut de F1, reproduite sur l'hôte**. ⚠️ *C'est le rouge le plus important de F3 : il doit être vu, et sa sortie versée* |
| `sur_un_faux_SENSIBLE_GROS_BIN_rend_gros_bin_sous_son_nom_stocke` | garder la résolution directe → `introuvable`, c'est-à-dire l'incohérence que F1 relève |
| `le_nom_rendu_est_le_nom_STOCKE_pas_le_nom_DEMANDE` | rendre `demande` → le substitut est créé sous un nom qui n'existe pas côté poste local, et une écriture ultérieure le crée pour de bon |
| `deux_homonymes_de_casse_rendent_ambigu_et_rien_d_autre` | choisir le premier → on écrase l'un des deux ; le refus est **bruyant**, pas silencieux |
| `un_nom_exact_court_circuite_l_ambiguite` | ne pas privilégier l'exact → `note.txt` deviendrait ambigu sur un poste qui porte aussi `Note.txt`, alors qu'il est parfaitement désigné |
| `l_injection_de_faute_est_INERTE_quand_elle_n_est_pas_armee` | la lire depuis le module → un utilisateur qui crée un dossier `.faute-disque-plein` casserait son propre pont |

⚠️ **Ce que ce module NE corrige PAS, et il faut l'écrire dans son en-tête** :
la moitié VM du phénomène (NTFS résout la casse sur un fichier **déjà hydraté**
sans jamais atteindre le pont). Elle est **bénigne** — l'application obtient le
bon fichier, aucun substitut n'est créé sous un mauvais nom — et **elle n'est
pas réparable ici** : quand NTFS répond, nous ne sommes pas consultés.

**Dépendances** — tâche 1 (pour `casse-ambigue`). **Pure, parallélisable.**

---

### Task 10 : `client/src/fichiers/mutation.ts` — `move()` et son repli LOCAL

**Objet** — renommer et supprimer côté poste local, dans les deux branches.

**Fichiers créés** — `client/src/fichiers/mutation.ts`, `mutation.test.ts`.
**Fichiers modifiés** — `client/src/fichiers/adaptateur.ts` (**289**, marge 211)
— **deux méthodes seulement, qui délèguent**, et l'extension des poignées
injectées (§3.3).

**Contenu** — §0.1 : détection de `move` à l'appel, repli copie+suppression
**entièrement dans le navigateur**, récursion pour un répertoire, règle du
renommage de casse pure, `removeEntry` **sans `recursive`** (§0.6).

**L'instrumentation que la spec exige** — une trace, rendue au pont dans
l'en-tête de la réponse `Fait` et journalisée par lui :

```
renommage par copie chemin="…" octets=… entrees=… duree_ms=…
```

**Tests, et leur ROUGE** :

| Test | Rouge, atteignable comment |
| --- | --- |
| `avec_move_un_renommage_est_UN_appel_et_ne_copie_rien` | appeler le repli quand même → le faux compte des lectures |
| `sans_move_un_repertoire_contenant_un_SOUS_REPERTOIRE_est_renomme` | ⚠️ **le rouge est le défaut de l'ancien pont, `web/index.js:631`** — masquer la variable de boucle par elle-même, et le faux voit le sous-répertoire manquant |
| `sans_move_aucun_octet_ne_passe_par_le_canal` | orchestrer la copie par `Lire`+`Ecrire` → le faux de canal compte des trames, et le coût de la spec §3.5.1 redevient vrai |
| `un_renommage_de_casse_pure_passe_par_un_nom_intermediaire` | renommer directement → sur le faux **insensible**, la source est écrasée par elle-même ou l'opération échoue en `deja-present` |
| `une_suppression_de_repertoire_NON_VIDE_rend_repertoire_non_vide` | passer `recursive: true` → **le sous-arbre du poste local disparaît**, et le test ne peut plus le voir |
| `une_copie_interrompue_laisse_la_source_intacte` | supprimer la source avant la fin de la copie → perte de données ; le test la constate |

**Dépendances** — tâches 4 et 9. **Pure.**

---

### Task 11 : `flux.ts`, `canal.ts`, `protocole.ts` — la contre-pression et les deux verbes

**Objet** — que le navigateur cesse d'inonder SCTP, et qu'il serve les deux
verbes neufs.

**Fichiers créés** — `client/src/fichiers/flux.ts`, `flux.test.ts`.
**Fichiers modifiés** — `client/src/fichiers/canal.ts` (**213**, marge 287) :
`bufferedAmountLowThreshold` posé à la création (`:95` ne passe aujourd'hui que
`{ ordered: true }`), et l'attente avant `canal.send(reponse)` (`:118`) ;
`client/src/fichiers/protocole.ts` (**128**, marge 372) : les bras
`TYPE_RENOMMER` et `TYPE_SUPPRIMER`.

⚠️ **`canal.ts` n'a AUCUN test**, et son en-tête (`:177-180`) le déclare. C'est
pour cela que toute la logique d'attente vit dans `flux.ts`, **pur**, injecté
avec `{ bufferedAmount, bufferedAmountLowThreshold, addEventListener }` :
`canal.ts` ne fait que câbler.

⚠️ **L'invariant de `protocole.ts:17-21` — « UNE REQUÊTE REÇOIT TOUJOURS UNE
RÉPONSE » — tient pour les deux verbes de F3** : ce sont des **requêtes**, elles
reçoivent `Fait` ou `Echec`. *(C'est F2 qui l'assouplit, pour son annonce
`TYPE_DUES`. F3 n'y touche pas et ne doit pas le réécrire.)*

**Tests, et leur ROUGE** :

| Test | Rouge, atteignable comment |
| --- | --- |
| `on_n_envoie_pas_tant_que_le_tampon_depasse_le_seuil` | envoyer quand même → le faux voit `bufferedAmount` croître sans borne |
| `l_evenement_bufferedamountlow_libere_l_attente` | ne pas s'y abonner → **l'attente ne se termine jamais, et le pont expire** — un blocage pire que celui qu'on répare |
| `un_canal_ferme_pendant_l_attente_ne_reste_pas_suspendu` | ne pas traiter `close` → une lecture en cours fige la page à la fermeture |
| `un_bras_RENOMMER_repond_toujours` | rendre `null` → la commande reste en vol côté pont **jusqu'à son expiration**, et l'Explorateur se fige sur une panne pourtant immédiate |
| `un_type_inconnu_est_journalise_et_ignore` (existant) | — |

**Dépendances** — tâches 4 et 10.

---

### Task 12 : le rappel de notification — `isdirectory`, `destinationfilename`, et l'union toujours pas lue

**Objet** — recevoir les deux notifications neuves et les traduire, sans jamais
déréférencer l'union.

**Fichiers modifiés** — `agent/src/pont/projfs/rappels/notification.rs` (issu de
la tâche 3).

**Contenu** :

- `_est_repertoire` (`rappels.rs:409`) devient `est_repertoire` et est
  **transporté** dans l'en-tête (`repertoire: bool`, §3.1) ;
- `_destination` (`:411`) devient `destination` et est lu par
  `to_string()` puis `chemins::normaliser` ;
- `_parametres` (`:412`) **reste `_parametres`** — §2.3 ;
- **deux invariants qui refusent plutôt que de deviner** :
  `Renommer` n'est poussé que si `vers` est **non vide** et **différent de
  `de`** ; sinon le pont journalise un `warn!` nommant les deux champs bruts et
  ne pousse rien. ⚠️ *C'est la parade au risque d'inversion des champs (§9,
  R-F3-1) : se tromper de sens détruirait le fichier.*

**Rouge — et il est vérifié atteignable par la compilation croisée** : changer
`isdirectory: bool` en `i32` dans la signature transcrite fait échouer
`const _: PRJ_NOTIFICATION_CB = Some(notification)`, à chaque
`cargo check --target x86_64-pc-windows-gnu`. **Ce rouge doit être VU une fois**
et sa sortie versée — c'est le **seul** garde d'ABI du dépôt, et F1 a mesuré que
**trois mutations d'ABI sur trois ont survécu** aux tests d'hôte
(`journaux-pont-fichiers/f1-tache12-mutations.txt`).

**Dépendances** — tâches 3, 5, 6. **BLOQUÉE par la tâche 3.**

---

### Task 13 : `pont.rs`, `service.rs`, `verbes.rs`, `etat.rs` — le câblage

**Objet** — pousser les mutations, traiter leurs réponses, brancher la fenêtre
et le nom canonique.

**Fichiers modifiés** — `agent/src/pont.rs` (**151**), `agent/src/pont/service.rs`
(**325**), `agent/src/pont/service/verbes.rs` (**265**),
`agent/src/pont/projfs/etat.rs` (**267**), `agent/src/pont/table.rs`.

**Contenu** :

1. `PONT_MUTATION` lue une fois, par `OnceLock`, dans `pont.rs`, avec la trace
   de désarmement de la tâche 2 ;
2. l'émission de `Renommer`/`Supprimer`, **passée par `mutation::ordonnancer`** ;
3. les bras de réponse : `Fait` retire la mutation, `Echec` la nomme à la
   page-shell **et au journal** ;
4. 🔵 **`PrjWritePlaceholderInfo` reçoit le nom CANONIQUE** rendu par le
   navigateur, jamais le chemin que l'application a tapé (§0.2 conséquence 1) ;
5. la trace de refus de `service.rs:140` passe de `debug!` à **`warn!`** et
   porte **le code du fil ET la cause** (§0.5) ;
6. la fenêtre de `lecture.rs` remplace le « un morceau à la fois » de
   `service.rs:228-238`, `rappels.rs:303-306` et `etat.rs:76-82` — **et les
   trois commentaires qui annoncent cette fenêtre comme un livrable de F3 sont
   réécrits au passé.** ⚠️ *Ne pas les réécrire produirait exactement le défaut
   que la revue transverse cherche : une affirmation de code réfutée par sa
   propre branche.*

**Rouge** : `RUST_LOG=info` sur une exécution où un chemin absent est demandé
doit faire apparaître la ligne de refus. **Aujourd'hui elle n'apparaît pas** —
`service.rs:140` est un `debug!`, `scripts/run-agent.sh:29` pose `info`. **Le
rouge est donc l'état actuel, constaté.**

**Dépendances** — tâches 3, 5, 6, 7, 8, 12.

---

### Task 14 : `client/src/shell.ts` et `shell-page.ts` — nommer ce qui a échoué

**Objet** — qu'une mutation en échec soit **nommée**, et non comptée.

**Fichiers modifiés** — `client/src/shell.ts` (**154**), `client/src/shell-page.ts`
(**221**), `client/shell.html` (**88**).

**Contenu** — une ligne par mutation en échec : le chemin, le motif, l'heure.
Et l'armement `?faute-fichiers=1`, **lu une fois dans `shell-page.ts` et passé
en argument** (tâche 9).

⚠️ **Aucune sous-chaîne partagée entre deux messages d'état.** F1 a perdu neuf
minutes de mesure sur « Lecteur … **mont**é » contre « n'a pas pu être
**mont**é » ; le produit garde l'ambiguïté, et **les messages neufs de F3 ne
l'aggravent pas** : un test épingle qu'aucun message d'état n'est préfixe d'un
autre.

**Rouge** : `un_message_d_etat_n_est_prefixe_d_aucun_autre` — ajouter « lecteur
monté » et « lecteur monté en lecture seule » le fait échouer.

**Dépendances** — tâches 9, 11.

---

### Task 15 : SONDE S1 (VM) — quelles notifications arrivent, et QUEL CHAMP PORTE QUOI

**Objet** — répondre à trois questions dont dépend la justesse du câblage, sans
qu'aucun octet ne parte vers le poste local.

**Fichiers** — aucun de production ; l'instrument et les journaux sont versés
sous `journaux-pont-fichiers-f3/`.

**Montage** : binaire de la tâche 13, lancé avec **`PONT_MUTATION=0`**, modifié
pour cette seule sonde afin de **journaliser intégralement chaque notification
sans rien pousser** (`code`, `isdirectory`, `FilePathName`, `destinationFileName`
en clair). ⚠️ **Le désarmement est le mécanisme de produit, pas un bricolage** :
c'est la même variable qui sert de rouge à la recette.

**Les trois questions** :

| # | Question | Pourquoi elle décide |
| --- | --- | --- |
| ① | ProjFS émet-il `FILE_RENAMED` (128) pour un fichier, et pour un **répertoire** ? | sans elle, `Renommer` n'est pas livrable |
| ② | **`FilePathName` porte-t-il la SOURCE et `destinationFileName` la DESTINATION**, pour `PRE_RENAME` comme pour `FILE_RENAMED` ? | 🔴 **s'y tromper renomme dans le mauvais sens et DÉTRUIT** |
| ③ | Une suppression de répertoire non vide produit-elle une notification **par enfant**, y compris pour des enfants jamais énumérés ? | c'est ce qui rend la suppression non récursive (§0.6) suffisante |

**LA PORTE — trois observations dans la même exécution, dont DEUX
DISQUALIFIENT.** Un verdict négatif (« ProjFS n'émet pas cette notification »)
n'est prononçable **que si (a) et (b) sont vertes et (c) vide** :

| | Observation | Ce qu'elle disqualifie |
| --- | --- | --- |
| **(a)** | **témoin positif** — la même exécution porte `racine du pont fichiers montée` **et** au moins une énumération servie (une réponse `TYPE_ENTREES` traitée) | ⚠️ **sans elle, l'absence de notification ne dit rien** : un pont non monté n'émet rien non plus, et F1 a mesuré neuf minutes de pont calé qui ressemblaient à un produit vivant |
| **(b)** | **preuve que l'événement a eu lieu** — le renommage a bien été exécuté par Windows : `Test-Path` de l'ancien nom rend `False` **et** du nouveau `True`, dans la racine, **relevé depuis un processus neuf** | ⚠️ **sans elle, on mesurerait l'absence d'un GESTE, pas l'absence d'une notification** — et une sonde du dépôt a déjà rendu un faux verdict éliminatoire sur une machine saine pour exactement cette raison |
| **(c)** | **la mesure** — les lignes de notification, avec leurs quatre champs | c'est le relevé |

**Deux exécutions.** **Aucun taux.**

⚠️ **Ce que S1 ne peut PAS mesurer** : ce que ferait un poste local insensible à
la casse (§0.2), ni le comportement de `move()` (c'est S2).

**Dépendances** — tâches 12, 13. **PRÉALABLE À LA TÂCHE 17.** Si ② contredit le
câblage, **la tâche 12 est reprise avant toute recette.**

---

### Task 16 : SONDE S2 (navigateur) — `move()`, et la sensibilité à la casse de l'instrument

**Objet** — savoir quelle branche du renommage la recette exerce réellement, et
ce que l'instrument peut prouver.

**Fichiers** — aucun de production. Un script d'évaluation CDP, versé.

**Trois questions, jouées dans la page-shell de la recette, sur le Chrome de la
recette, contre OPFS** :

| # | Question | Ce qu'on en fait |
| --- | --- | --- |
| ① | `typeof handle.move === 'function'` sur un **fichier** ? | décide quelle branche la recette exerce |
| ② | idem sur un **répertoire** ? et `move()` accepte-t-il un répertoire de destination différent ? | **l'ancien pont ne l'appelait que sur des fichiers** (`web/index.js:628`) : indice, pas preuve |
| ③ | OPFS est-il **sensible** à la casse ? (`getFileHandle('A.txt', {create:true})` puis `getFileHandle('a.txt')`) | ⚠️ **c'est ce qui dit si l'instrument PEUT reproduire le défaut de casse.** S'il est sensible, **il ne le peut pas**, et le §7 le déclare |

⚠️ **Ce n'est PAS une porte éliminatoire, et ce plan ne la déguise pas en
porte.** Les deux branches du renommage sont livrées et testées sur l'hôte
(tâche 10) : quelle que soit la réponse, F3 fonctionne. S2 dit **ce que la
recette démontre**, pas ce que le produit sait faire. *(Le planificateur de G2 a
refusé d'écrire une porte quand ses mesures fermaient les inconnues ; la même
retenue s'applique ici.)*

⚠️ **La question qui ne peut PAS être posée à cet instrument** : `handle.name`
reflète-t-il le nom **stocké** après une résolution insensible à la casse ? Elle
exige un système de fichiers insensible à la casse, donc un vrai
`showDirectoryPicker()`, que F1 a mesuré **inatteignable** sur cet hôte (aucune
commande CDP pour accepter un sélecteur, ni `DISPLAY`, ni `Xvfb`, ni `xdotool`).
**C'est pour cela que le canonicaliseur de la tâche 9 n'exploite pas
`handle.name` : la seule optimisation évidente repose sur un fait que ce
montage ne peut pas établir.** Léguée, pas implémentée à moitié.

**Dépendances** — aucune côté produit. **À jouer AVANT la tâche 17.**

---

### Task 17 : RECETTE F3 sur la VM

**Objet** — les quatre critères de la spec §8 F3, **deux exécutions chacun**,
avec leur rouge provoqué.

⚠️ **Préalables non négociables** : `virsh list --all` ; `Get-Process agent`
revérifié **après chaque tentative, y compris échouée** ; **aucun chevauchement**
entre deux exécutions ; `cargo clean --release -p proto -p agent` puis
`scripts/build-agent.sh` après `set -a && source .env && set +a` ; **taille du
binaire relevée** ; chaque journal copié **sous un nom unique immédiatement** ;
tous versés sous `journaux-pont-fichiers-f3/`, avec leur jumeau `-plat`.

| # | Critère | Geste | ROUGE, et comment il est provoqué |
| --- | --- | --- | --- |
| ① | renommer un fichier, puis un **répertoire contenant un sous-répertoire**, réussit des deux côtés | `ren` dans la racine ; relevé du poste local **depuis la page**, par condensat des noms | `PONT_MUTATION=0` → le `PRE_RENAME` refuse, l'application voit `ERROR_WRITE_PROTECT`, et **le poste local est inchangé**. ⚠️ *C'est un rouge du MÉCANISME, pas un rouge vacueux : le refus est journalisé et le compteur `protege-en-ecriture` monte* |
| ② | supprimer un fichier, puis un **répertoire non vide**, se répercute | `del`, puis `rd /s` | idem. Et si S1 ③ a répondu « pas de notification par enfant », **le critère tombe pour sa seconde moitié et on le dit** |
| ③ | canal coupé en pleine lecture → `ERROR_IO_DEVICE` **en moins du budget**, l'application ne se fige pas, la vidéo n'a pas perdu une image | copie d'un fichier > 10 Mio, puis **fermeture de la page-shell** ; `framesDecoded` relevé dans la page d'application | ne pas couper → l'application termine, et **rien n'est mesuré**. ⚠️ *Le rouge utile est l'inverse : avec le balayage désarmé, l'application doit se figer au-delà du budget — c'est le legs 4 de F1, et le recensement de la tâche 8 dit alors où* |
| ④ | **chacun des douze codes du §5 observé au moins une fois** | le tableau du §0.5 : six gestes réels, quatre injections, deux mixtes | ⚠️ **le rouge est `manquants()` NON VIDE**, et il est atteignable **par construction** : une exécution qui n'exerce rien le rend vide de rien. **Le contrôle est donc vérifié capable d'échouer avant d'être joué** |

⚠️ **Le critère ③ porte deux mesures qu'il ne faut pas confondre** : « la vidéo
n'a pas perdu une image » se lit sur `framesDecoded` **et ne dit rien de ce que
l'utilisateur voit**. F1 a mesuré neuf minutes d'Explorateur figé pendant que
`framesDecoded` croissait, `packetsLost` à 0, zéro `ERROR`. **La seconde
mesure — l'application ne se fige pas — est indépendante et se relève depuis la
VM**, par le temps de retour de la commande.

**Les `grep` de la recette, chacun accompagné du `grep -rn` qui établit que la
chaîne est émise par le code livré** (règle §1.4, payée trois fois en F1) :

```bash
# à écrire par l'implémenteur APRÈS avoir vérifié chaque chaîne :
grep -rn 'codes rendus' agent/src/            # établit l'émetteur
grep -a 'codes rendus' agent-f3-exec1-plat.log | tail -1
grep -rn 'pont en vol' agent/src/
grep -a 'pont en vol' agent-f3-exec1-plat.log | tail -3
grep -rn 'renommage par copie' agent/src/
```

⚠️ **Ne PAS prescrire ici les chaînes exactes** : elles n'existent pas encore, et
un plan qui les fige les fait diverger du code — c'est exactement le défaut de
F1, dont **deux** `grep` auraient fait lire un succès comme un échec.

**Journaux versés** : `agent-f3-exec{1,2}.log` + `-plat`,
`pilote-f3-exec{1,2}.{log,json}`, `s1-notifications-{1,2}.txt`,
`s2-move-casse.txt`, `sha256-{origine,apres}.txt`, `LISEZ-MOI.md`.

**Dépendances** — tâches 13, 14, 15, 16.

---

### Task 18 : revue transverse de fin de branche, résultats, `CLAUDE.md`

**Objet** — chercher les affirmations que la branche a rendues fausses, et
inscrire ce que F3 laisse.

**Barème du dépôt** : D7 5, D8 3, D9 6, **D10 douze**, D11 sept, P1 huit, P2 dix,
S1 cinq, S2 douze, **S3 treize**, **S4 vingt-sept**, G1 huit, presse-papier P1
**dix-sept**, P5 neuf, **F1 onze**. **La revue est obligatoire** : les défauts
qu'elle trouve franchissent tous une frontière de tâche, et **une revue par
tâche ne peut structurellement pas les voir**.

**Cibles nommées d'avance, parce que F3 les rend fausses par construction** :

- les **trois** commentaires qui annoncent la fenêtre de lecture comme un
  livrable de F3 (`service.rs:228-238`, `rappels.rs:303-306`,
  `etat.rs:76-82`) — ils doivent passer au passé ;
- `client/src/fichiers/adaptateur.ts:14-30` — « LA CASSE N'EST TRAITÉE NULLE
  PART, ET C'EST UN LEGS DÉCLARÉ » : F3 la traite ;
- `agent/src/pont/chemins.rs:12-40` — le même legs, du côté Rust ;
- `agent/src/pont/notifications.rs:78-92` — le masque « des cinq notifications
  de F1 », son nom de test compris ;
- `agent/src/pont/erreurs.rs:78-81` — « (F3) » et « (F2) » en regard de
  `RepertoireNonVide` et `DejaPresent`, à relire une fois qu'ils ont un
  appelant ;
- `client/src/fichiers/canal.ts:88-95` — la description du canal, qui ne
  mentionne aucun seuil de tampon.

**Tailles, RELEVÉES PAR LA COMMANDE APRÈS la dernière édition de la ronde** —
une table relevée en début de ronde serait fausse à la fin de la même ronde,
erreur que D8 a commise en croyant bien faire. Et **« corrigé à sa place » est
une affirmation de COMPLÉTUDE** : énumérer les places par `grep -n '<nombre>'
CLAUDE.md` **avant** d'écrire, puis **relire place par place APRÈS l'édition**.

**Le document de résultats** est **permanent** et vit dans `docs/` : D9 a perdu
**six** constats de revue parce que sa preuve vivait dans un espace gitignoré, et
tout ce qu'on peut en dire aujourd'hui est qu'ils sont perdus.

**Dépendances** — toutes.

---

## 5. Ordre et dépendances

```
1  contrôle d'entrée (aucun code) ──┬──────────────────────────────────┐
2  run-agent.sh PONT_MUTATION ──────┼───────────────────┐              │
3  EXTRACTION rappels.rs ───────────┼──────┬────────────┼──────────────┤
4  proto (types, en-têtes, vecteurs)┴──┬───┼──┬───┬─────┼──────────────┤
                                       │   │  │   │     │              │
5  compteurs.rs (PUR) ─────────────────┼───┤  │   │     │              │
6  notifications.rs (masque, état) ────┘   │  │   │     │              │
7  mutation.rs (PUR) ──────────────────────┤  │   │     │              │
8  lecture.rs + table.rs (PUR) ────────────┤  │   │     │              │
9  client noms.ts (PUR) ───────────────────┼──┤   │     │              │
10 client mutation.ts (PUR) ───────────────┼──┴───┤     │              │
11 client flux/canal/protocole ────────────┼──────┴──┐  │              │
                                           │         │  │              │
12 rappel notification (#[cfg(windows)]) ──┘         │  │              │
13 câblage pont/service/verbes/etat ─────────────────┴──┘              │
14 page-shell (échecs nommés, armement) ───────────────────────────────┘
                              │
15 SONDE S1 (VM) ─────────────┤   ← peut faire REPRENDRE la tâche 12
16 SONDE S2 (navigateur) ─────┤   ← indépendante, à jouer TÔT
                              ↓
17 RECETTE (VM) ──────────────→ 18 revue transverse + résultats + CLAUDE.md
```

- **1 précède tout** : elle dit dans quel monde on est (§1.1).
- **3 BLOQUE 12 et 13.** Aucune ligne n'entre dans `rappels.rs` avant.
- **2 précède 13, 15 et 17.** La variable voyage avant qu'on en ait besoin.
- **15 peut invalider 12**, et se joue donc **avant** 17.
- **16 est indépendante du produit** et peut se jouer dès que la page-shell
  monte un lecteur — donc très tôt.
- **4, 5, 7, 8, 9 sont indépendants** et parallélisables — *mais un `git add`
  NOMINATIF, jamais `-A`.*
- **15, 16 et 17 sont sérialisées avec toute tâche employant la VM**, y compris
  celles des autres sous-projets. **La VM est un état partagé.**

---

## 6. Divergences relevées entre la spec, F1, F2 et le CODE RÉEL

Toutes relevées par la commande le 20 août 2026, chaque citation relue par
`sed -n` après avoir été écrite.

| # | Ce que dit le document | Ce que dit le code / la mesure |
| --- | --- | --- |
| 1 | spec §3.5.1 : le repli de renommage « fait transiter tout le contenu **deux fois sur le canal** », « 2 Gio pour 1 Gio » | **Faux pour ce montage** : la copie se fait entre deux poignées du navigateur. **Zéro octet sur le canal**, dans les deux branches — §0.1 |
| 2 | spec §3.5 : `dir.removeEntry(nom, { recursive })` | **F3 n'emploie PAS `recursive`** : cela transformerait un geste local en destruction récursive du poste local, et cela rendrait `RepertoireNonVide` inatteignable — §0.6 |
| 3 | spec §8 F3 : « livre […] la table des HRESULT du §5 dans son intégralité » | **Elle est livrée depuis F1** : `erreurs.rs:60-89` et `:144-160`, douze variantes, `NOMBRE = 12`. Ce que F3 livre, c'est de quoi l'**exercer** — §0.5 |
| 4 | spec §5.1 : « deux causes distinctes ne partagent jamais un code » | **`service.rs:303-304` en fait partager un** à `TropGrand` et `Interne`. **Relevé, tranché, non corrigé** : la distinction est rendue au **journal**, pas au `HRESULT` — §0.5 |
| 5 | spec §5.3 : **trois** budgets de délai | F3 en ajoute un **quatrième**, `DELAI_MUTATION`. Déclaré, et **non calibré** comme les trois autres |
| 6 | spec §7.3 : « le pont ne demande pas le morceau *n+1* tant que … » | **Vacueux tant qu'un seul morceau est en vol** — et c'est l'état du code (`service.rs:228-238`, `rappels.rs:303-306`, `etat.rs:76-82`). F3 livre la **fenêtre** avec le seuil, ou rien — §0.4 |
| 7 | spec §3.4 : `bufferedAmountLowThreshold` « posé » | **Il ne l'est pas.** `canal.ts:95` ne passe que `{ ordered: true }`, `:118` envoie sans rien regarder |
| 8 | F1 legs 1 : « la casse rend le mauvais fichier » | **Relecture** : la moitié mesurée (NTFS sur un fichier hydraté) est le comportement **normal** de Windows ; la moitié destructrice (navigateur sur un poste insensible à la casse) **n'a jamais été observée** et ne peut pas l'être sur ce montage — §0.2 |
| 9 | F1 legs 4 : « des lectures calent sans jamais expirer, on ne sait pas où » | Le mécanisme d'expiration **existe et paraît correct** (`table.rs:142-153`, `service.rs:95-105`, balayage à 250 ms). Ce qui manque est l'**observabilité** : la tâche 8 la livre |
| 10 | spec §8 F3 critère (4) : « observé au moins une fois **au journal du pont** » | **Insatisfiable aujourd'hui** : la seule trace qui nomme une cause est un `debug!` (`service.rs:140`) et `run-agent.sh:29` pose `info` |
| 11 | plan de F2 §2.2 : `rappels.rs` **488**, extraction en tâche 3 | **Toujours 488 au 20 août 2026** : F2 n'est pas implémenté. §1.1 |
| 12 | spec §9 : `client/src/shell.ts` **93**, `shell-page.ts` **71** | **154** et **221** (déjà relevé par F2, toujours vrai) |
| 13 | F1 résultats §10 : suites à **614** / **223** / **111**, **16** avertissements | **Non réexécutées ici** (arbre partagé). **À relever par la tâche 1** |
| 14 | spec §4.3 : `PRE_RENAME` « accepte, sauf si la cible sort de la racine » | Complété : F3 refuse **aussi** sur canal fermé, `PONT_MUTATION=0` et racine en lecture seule. **Toujours sur un ÉTAT, jamais sur une issue** — §0.6 |

---

## 7. Ce que F3 n'établira PAS

Écrit d'avance, pour qu'aucun document de résultats n'ait à le découvrir.

- **Aucun taux.** Deux exécutions par critère est la règle ; deux exécutions ne
  font pas une fréquence.
- 🔴 **Le remède de casse ne sera PAS démontré de bout en bout.** Il exige un
  poste local **insensible** à la casse **et** un fichier **jamais hydraté**.
  L'instrument est OPFS (F1 §3), `showDirectoryPicker()` est inatteignable sur
  cet hôte, et la sensibilité d'OPFS est la question ③ de S2. **Ce qui sera
  démontré** : le comportement sur l'hôte, avec **deux** faux (sensible et
  insensible), et la disparition de l'incohérence `GROS.BIN` sur l'instrument.
- **Aucune constante calibrée** : `MORCEAUX_EN_VOL`, `SEUIL_TAMPON`,
  `DELAI_MUTATION`, `PERIODE_RECENSEMENT`, plus les quatre de F1
  (`TAILLE_TRAME_MAX`, `DELAI_ATTRIBUTS`, `DELAI_LIRE`, `DELAI_LISTER`), celles
  de F2, et les huit du chantier D.
- **Aucun gain de débit revendiqué.** La seule mesure de débit du dépôt varie
  d'un **facteur ~120** sans explication (F1 §11). C'est F4.
- **Rien de la latence** — c'est F4, et c'est le risque R2 de la spec : *le
  lecteur peut fonctionner et rester inutilisable*. ⚠️ **Et F3 en ajoute** : le
  canonicaliseur énumère le parent à chaque résolution (§0.2).
- **`Rafraichir`, le cache d'énumération, l'occupation disque, la reprise à
  travers un redémarrage de VM, le retour avec un autre répertoire** : F5.
- **Aucune politique d'éviction des hydratations.** `PrjDeleteFile` et
  `PrjClearNegativePathCache` restent chargées **sans appelant**, déclarées telles
  (`chargement.rs:239-246`, `:248-255`).
- **R7 reste OUVERT, et F3 n'y ajoute rien de neuf** — §8.
- **Aucun test d'hôte ne couvre `pont/projfs/`** ; `cargo check --target
  x86_64-pc-windows-gnu` en vérifie types, emprunts et durées de vie — **jamais
  le comportement**.
- **`showDirectoryPicker()` n'est toujours jamais appelé**, ni le modèle de
  permission (`queryPermission`/`requestPermission`), ni l'activation
  utilisateur transitoire, ni le mode `readwrite`.
- **Rien d'un client réel** : Chrome sans interface, décodage logiciel, sur
  l'hôte qui porte la VM. La File System Access API n'existe ni sur Firefox ni
  sur Safari — limite du **produit**.
- **Rien de plusieurs utilisateurs** : une VM, une racine, `SESSION_DU_PONT` non
  namespacé. **Rien à travers une reconnexion WebRTC** : le chemin n'existe pas.
- **Rien du verrouillage inter-machines** : une modification faite sur le poste
  local pendant qu'une application de la VM écrit produit une divergence que
  rien ne détecte.
- **L'énumération vide intermittente de F1 (legs 3) n'est ni expliquée ni
  refermée** ; le journal qui l'aurait établie est **perdu**.
- **L'atomicité du repli de renommage n'est pas rétablie** : une coupure au
  milieu laisse deux copies, dont l'une porte le nom cible.
- **Rien de l'ancien pont** : ni modifié, ni retiré, ni comparé chiffre à
  chiffre. Il est **lu**, pas mesuré.
- **Le chemin d'extinction propre du superviseur** n'a toujours jamais été
  exercé, depuis D1 — et **une racine ProjFS peut survivre à un arrêt brutal**,
  ce que rien ne démonte.

---

## 8. Comment F3 se tient vis-à-vis de R7 (les transcriptions d'API)

**Le point de départ, mesuré et non craint** : les **treize** transcriptions de
`agent/src/pont/projfs/chargement.rs` (338 lignes) sont écrites à la main, et
**les trois mutations d'ABI jouées en F1 ont TOUTES survécu** aux tests d'hôte et
à `cargo check --target x86_64-pc-windows-gnu`
(`journaux-pont-fichiers/f1-tache12-mutations.txt`). **Rien, dans ce dépôt, ne
peut attraper une transcription fautive** ; seules la relecture texte à texte et
l'exécution sur la VM en jugent.

1. 🔵 **F3 N'AJOUTE AUCUNE ENTRÉE IMPORTÉE. Zéro.** Vérifié en énumérant les
   besoins : le renommage et la suppression n'appellent **rien** de
   `ProjectedFSLib.dll` — ils **arrivent** par un rappel que nous écrivons, et
   le miroir est fait par le navigateur ; le nom canonique passe par
   `PrjWritePlaceholderInfo`, **déjà chargée** ; le tri d'énumération par
   `PrjFileNameCompare`/`PrjFileNameMatch`, **déjà chargées** ; la fenêtre de
   lecture par `PrjWriteFileData`, **déjà chargée**. **`resolution::NOMBRE`
   reste 13** et `resolution::NOMS` est **inchangé** — un test existant le
   vérifie déjà (`agent/src/pont/resolution/tests.rs`, 5 tests).
2. **Le seul rappel modifié est couvert par le seul garde d'ABI du dépôt.**
   `const _: PRJ_NOTIFICATION_CB = Some(notification)` (`rappels.rs:67`) est
   vérifié **par la compilation croisée ordinaire, à chaque fois**. La tâche 3 le
   déplace **avec sa fonction** ; la tâche 12 le rend **rouge une fois** et verse
   la sortie.
3. **L'union `PRJ_NOTIFICATION_PARAMETERS` n'est TOUJOURS pas déréférencée**
   (§2.3), et c'est vérifiable : le nom de destination est un **paramètre
   direct** du rappel (`mod.rs:334`).
4. **Contrôle de revue** : toute constante ProjFS neuve porte sa **valeur** et
   son **numéro de ligne** dans `windows-0.62.2`, comme `notifications.rs:64-76`.
   ⚠️ **Et le contrôle porte sur un fichier qui a un HOMONYME** : `windows-sys`
   expose les mêmes symboles à d'autres lignes, et une première rédaction du plan
   de F2 s'y est trompée. **La VALEUR prime sur la ligne.**

**Ce que F3 ne réduit pas** : le risque des douze autres entrées, que le pont
continue d'appeler. Il est **inchangé, pas aggravé**.

---

## 9. Risques — et ce qui rendrait F3 NON LIVRABLE

| # | Risque | Ce qu'on en sait, et la parade |
| --- | --- | --- |
| **R-F3-1** | 🔴 **L'appariement source/destination est inversé** : renommer dans le mauvais sens **DÉTRUIT** | **Deux parades, et la seconde ne dépend d'aucune mesure** : (i) la sonde S1 ② le relève sur pièces avant toute recette ; (ii) le pont **refuse de pousser** un `Renommer` dont `vers` est vide ou égal à `de`, avec un `warn!` qui nomme les deux champs bruts. **Non éliminatoire, mais c'est le risque le plus grave du sous-bloc** |
| **R-F3-2** | 🔴 **ProjFS n'émet pas `FILE_RENAMED` exploitable** (jamais émise, ou sans destination utilisable) | **Non tranché** — c'est la sonde S1 ①/②, jouée **avant** la recette. Si c'est le cas : **`Renommer` n'est PAS livrable**, le critère (1) tombe, et **il n'y a pas de repli** — sans le nom de destination on ne peut pas renommer côté navigateur. **Le repli dégradé est de ne pas régresser** : refuser `PRE_RENAME` comme F1 et F2 le font déjà, bruyamment. **C'est le seul risque qui rendrait la moitié « renommage » de F3 non livrable** |
| **R-F3-3** | **ProjFS n'émet pas de notification par ENFANT** à la suppression d'un répertoire non vide | La suppression non récursive (§0.6) laisserait les enfants sur le poste local. **Mesuré par S1 ③.** Repli nommé : passer `recursive: true` **et le déclarer comme une destruction récursive consentie**, ce que ce plan refuse par défaut. **Dégrade, ne bloque pas** |
| **R-F3-4** | **Le canonicaliseur refuse des lectures légitimes** sur un poste local **sensible** à la casse portant `note.txt` et `Note.txt` | **Il refuserait les deux**, en `casse-ambigue`. C'est le même arbitrage que la garde d'écriture de F2 (son R-F2-5), et le refus est **bruyant** (journal + page-shell), pas silencieux. **Non résolu, déclaré** |
| **R-F3-5** | **Le coût du canonicaliseur rend le lecteur inutilisable** — une énumération du parent par résolution, sur un répertoire de 10 000 entrées | **Non mesuré ici** : c'est F4, et c'est le risque R2 de la spec. ⚠️ **F3 aggrave R2 en connaissance de cause**, pour corriger un défaut qui rend le mauvais fichier. L'optimisation évidente (`handle.name`) est **léguée**, faute d'un montage capable de l'établir |
| **R-F3-6** | **La fenêtre de lecture casse une lecture** — réponses hors d'ordre, complétion partielle, annulation en vol | Le canal est `ordered` (`canal.ts:95`), les morceaux sont demandés en ordre croissant, et le module pur **dénonce** une réponse hors d'ordre au lieu de l'appliquer. ⚠️ **Le seul critère qui l'attraperait est le condensat SHA-256**, que F1 n'a **jamais** établi : la recette doit le faire, et c'est le premier geste que F1 lègue (son n°6) |
| **R-F3-7** | **Une écriture due de F2 arrive après un renommage** et recrée le fichier temporaire | §0.3, deux règles pures et testées. ⚠️ **Sans F2 fusionné, ce risque n'existe pas — et la règle n'est donc pas exercée non plus** |
| **R-F3-8** | **Une panique dans le rappel de notification abat le pont** | `catch_unwind` à chaque frontière (`rappels.rs:76-88`), et **D2 fait que le pire cas est la mort du pont** : le superviseur le relance (mesuré, `delai_apres_mort_ms=44`), la vidéo ne bronche pas (mesuré : sur `exec1`, **neuf minutes** de pont calé, `framesDecoded` **monotone sur six échantillons**, `packetsLost` **0**, **0** `ERROR` ; le **51 935** que l'on cite souvent est le `framesDecoded` d'`exec2`, **pas** celui de l'exécution calée — deux exécutions, deux chiffres) |
| **R-F3-9** | **L'arbre est PARTAGÉ** avec les sous-projets ④ et ⑤ ; un `build-agent.sh` concurrent recopie l'arbre sur la VM | **Sérialiser** toute tâche employant la VM. F1 a perdu une exécution pour l'avoir ignoré, et **le journal versé sous le nom de la première était celui de la seconde** |

**Ce qui rendrait F3 NON LIVRABLE : R-F3-2 seul**, et encore : il ne supprime que
la moitié « renommage ». La suppression, le canonicaliseur de casse, le contrôle
de flux, le compteur de codes et les budgets restent livrables sans lui. **Il
n'existe pas, pour F3, de risque de la nature de R1 (ProjFS absent) ou de
R-F2-1 (aucun outil d'écriture en place) : aucun risque de ce sous-bloc ne le
prive de tout son objet.** C'est pourquoi ce plan **n'écrit qu'une porte**,
celle de la sonde S1, et refuse d'en fabriquer d'autres par rituel.

---

## 10. Résumé en une phrase

> **F3 ne rend pas le pont rapide et ne prétend pas le faire ; il fait qu'un
> renommage et une suppression arrivent de l'autre côté, qu'un nom désigne le
> fichier qu'il nomme, et que les douze façons d'échouer soient COMPTÉES plutôt
> qu'écrites.**
