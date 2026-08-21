# Sous-projet ③ Pont fichiers — sous-bloc F5 : la vie longue

**C'est le DERNIER sous-bloc du sous-projet ③.** Ce que F5 ne prend pas, personne
ne le prendra : il n'y a pas de F6, et les legs de ce plan sortiront de ce
sous-projet **sans destinataire**. Cette phrase n'est pas une formule de style —
elle est la règle d'arbitrage de tout le §0 : chaque fois qu'un travail voisin
est à trois lignes du périmètre, la question n'est pas « est-ce à moi ? » mais
« qui d'autre ? », et la réponse est **personne**.

Spécification : `docs/superpowers/specs/2026-08-19-pont-fichiers-design.md`,
§ « F5 — la vie longue » (l. 1202-1217), §6.4, §7.4, §4.2, §9, §10 R4 et R7.
Sous-blocs clos : **F0/F1** (le lecteur en lecture seule), **F2** (l'écriture et
le journal des dues), **F3** (renommage, suppression, table des HRESULT, contrôle
de flux), **F4** (la mesure que le cadrage réclame).

---

## Ce que F5 est, et ce qu'il n'est pas

**F5 EST** : le cache d'énumération et son `Rafraichir` ; la poignée de main
`Bonjour` qui décide s'il faut pousser les écritures dues ou les **retenir** ;
la mesure de l'occupation disque de la racine ; et la recette du journal à
travers un **redémarrage complet de la VM**.

**F5 N'EST PAS** : une politique d'éviction (spec §10 R4 — `PrjDeleteFile` reste
chargée et **sans appelant**, comme depuis F1) ; un changement de conception de
la boucle de `pont::transport` (legs n°1 de F4, **sans destinataire**) ; le
découpage d'une énumération en plusieurs trames (legs n°2 de F4, incrément de
`FICHIERS_VERSION`) ; ni le réexamen du sélecteur de fichiers Windows, qui
n'appartient pas à ce sous-projet.

⚠️ **Deux exceptions à ce « n'est pas », toutes deux argumentées au §0.6 et
§0.7**, et toutes deux prises **parce que F5 est le dernier** : le correctif de
moindre coût que F4 a nommé sur le mur de listage (D10), et l'épreuve par
mutation de `MORCEAUX_EN_VOL`, que F4 a laissée écrite et non jouée (D11).

---

## 0. Ce que F5 doit trancher AVANT d'écrire une ligne de code

### 0.1 🔴 LA CLAUSE ROUGE DE LA SPEC EST ÉCRITE À L'ENVERS, ET C'EST LE VERT QUI INVALIDE F4

C'est le point le plus important de ce plan, et il doit être réglé avant tout,
parce qu'il détermine ce que la recette de F5 **doit mesurer en plus** de ses
critères.

La spec écrit (l. 1214-1217) :

> **ROUGE** : le fichier apparaît **sans** `Rafraichir` — ce qui voudrait dire
> qu'aucun cache d'énumération ne fonctionne, donc que chaque listage paie un
> aller-retour, donc que la mesure de F4 portait sur autre chose que ce que le
> produit livre.

**La prémisse est juste, la conclusion est fausse, et c'est F4 lui-même qui la
réfute.** Son annotation D2 de la table du §8, écrite dans la spec, dit :

> ⛔ « cache d'énumération chaud » N'EST PAS MESURABLE TEL QUEL :
> `TTL_ENUMERATION` **n'existe nulle part** […] F4 mesure **le chaud que le
> produit A**. ⚠️ **Conséquence à ne pas perdre : F4 mesure un produit SANS
> cache d'énumération, donc une borne HAUTE du coût.**

D'où l'inversion, qui doit être écrite dans la recette **avant** de la jouer :

| Verdict de F5 | Ce qu'il dit de F4 |
| --- | --- |
| **ROUGE** — le fichier apparaît sans `Rafraichir` | **F4 reste EXACT et courant.** Ce n'est pas F4 qui a mesuré autre chose : c'est **F5 qui n'a pas livré le cache**. Les 6 s pour mille entrées demeurent le coût du produit livré |
| **VERT** — le fichier n'apparaît qu'après `Rafraichir` | **C'est ICI que la mesure de F4 cesse de décrire le produit livré**, et seulement pour la part qu'un cache sert. Sa borne haute devient une borne haute *dépassée* |

🔴 **Conséquence exécutable, et c'est une TÂCHE, pas une remarque** : un verdict
VERT oblige F5 à **remesurer la latence de listage aux rangs de F4, cache armé**,
sinon l'annotation de la spec resterait une réserve là où elle peut devenir un
nombre. C'est la tâche 15.

⚠️ **Et le mur ne bouge pas.** Le cache ne déplace **PAS** le plafond de
~3 150 entrées : celui-ci tient à la taille d'**un** message, pas à la répétition
(F4 §5, et l'annotation D1 de la spec). Un plan qui laisserait croire le
contraire ferait chercher le mur du mauvais côté.

### 0.2 🔵 LA PRÉDICTION QUE F5 ÉCRIT D'AVANCE, ET SA FALSIFICATION

F4 a relevé, **fait non expliqué** (son §6.2, legs n°4) :

> Chaque `Get-ChildItem` produit **DEUX** commandes `Lister`. `lister=n:2` par
> geste, systématiquement, et le navigateur renvoie la liste entière aux deux.
> **Le coût est doublé.** Fait relevé, mécanisme non expliqué.

Si le cache est consulté au bon endroit — **avant** l'inscription d'une commande
dans la table (§0.4) —, la **seconde** de ces deux commandes doit être servie
depuis le cache. **Prédiction écrite avant la mesure** :

1. `lister=n:2` par geste doit devenir **`lister=n:1`** ;
2. la latence mur-à-mur d'un `Get-ChildItem` unique doit **approximativement se
   diviser par deux** aux rangs 100 et 1 000 ;
3. un **second** `Get-ChildItem` sur le même répertoire, dans la fenêtre de
   `TTL_ENUMERATION`, doit coûter **`lister=n:0`**.

**Ce qui la falsifie** : si (1) ne se produit pas, ou bien les deux `Lister` ne
passent pas par le même chemin, ou bien le cache n'est pas consulté là où il faut.
Dans les deux cas, **c'est un défaut de F5**, pas une propriété de ProjFS, et il
faut le dire ainsi.

⚠️ **Ce que cette prédiction NE ferme PAS** : le **mécanisme** du doublement.
Même si (1) se vérifie, on saura que le cache absorbe la seconde commande ; on ne
saura toujours pas **pourquoi il y en a deux**. Le legs n°4 de F4 reste ouvert,
et le taire ferait lire un vert comme une explication.

### 0.3 🔴 UN `Rafraichir` REÇU AUJOURD'HUI SERAIT JETÉ EN SILENCE — c'est le bras fourre-tout, pour la SIXIÈME fois

**Relevé dans le code, pas supposé.** La chaîne, telle qu'elle est aujourd'hui :

- `agent/src/pont/transport.rs:233-259` — `Event::ChannelData` décode la trame,
  **n'en lit que la corrélation**, et envoie `DuNavigateur::Reponse { correlation,
  trame }`. **C'est la seule variante qui porte des octets** (`transport.rs:48-55`) ;
- `agent/src/pont/service.rs:131` — `traiter(&etat, correlation, &trame)` ;
- `agent/src/pont/service.rs:206-211` — `table.resoudre(correlation, …)` rend
  `None` pour une corrélation inconnue, et la trame est **jetée** :
  `tracing::debug!(correlation, "réponse tardive ou inconnue : jetée")`.

Un `Rafraichir` poussé par le navigateur n'a **aucune** corrélation en table. Il
tomberait donc dans ce `debug!` — invisible sous `RUST_LOG=info`, qui est le
réglage de `scripts/run-agent.sh` et la doctrine d'exploitation de ce dépôt.
**Le bouton ne ferait rien, et rien ne le dirait.**

C'est exactement le patron que ce dépôt a payé **cinq fois** sur
`agent/src/capteur/pont_media.rs` (D5 `Sommeil`, D6 `Part`, D7 `Audio`, D8
`PleinEcran`, P1 presse-papier), et une sixième sur
`agent/src/superviseur/signalisation.rs` (P3, le refus rangé avec `ice-config`).
**D2 le ferme d'avance.**

### 0.4 🔵 OÙ LE CACHE SE CONSULTE, ET POURQUOI CE POINT-LÀ ET PAS UN AUTRE

Le point d'insertion est **relevé**, `agent/src/pont/projfs/rappels/listage.rs`,
fonction `suite_enumeration` :

```rust
        let session = sessions.entry(id).or_default();
        if redemarrer { session.redemarrer(); }
        if session.chargee() {
            // ⚠️ Chemin SYNCHRONE, et il est le cas nominal. […]
            return crate::pont::service::remplir_session(etat, session, tampon);
        }
        drop(sessions);
        // ← C'EST ICI que le cache se consulte.
        let entete = …;
        let demandee = etat.demander(…, Attendue::Lister { … }, …);
```

**Sur un succès de cache** : `session.poser(entrees_preparees)` puis
`remplir_session(…)`, et l'on **rend `S_OK`** — la commande n'est **jamais**
inscrite dans la table, donc **jamais complétée**.

🔴 **Ce détail est ce qui préserve l'invariant qui rend `pont/table.rs` pur**
(spec §7.1) : *« Un seul fil du pont complète les commandes, jamais un fil de
rappel. »* Remplir un tampon et rendre `S_OK` depuis un fil de rappel **n'est pas
compléter une commande** — c'est ne jamais en créer une. Appeler
`PrjCompleteCommand` depuis ce fil-là, en revanche, casserait l'invariant, et
c'est le geste qu'il ne faut pas faire.

⚠️ **Le cache stocke les entrées BRUTES, avant `enumeration::preparer`.** Le
filtrage par `searchExpression` et le tri par `PrjFileNameCompare` dépendent de
la requête (`dir *.txt` et `dir` n'ont pas le même résultat) et des comparateurs
ProjFS, qui ne sont pas disponibles sur l'hôte. Stocker le résultat préparé
ferait qu'un `dir *.txt` empoisonnerait le cache pour le `dir` suivant. **Le
cache est donc en amont de `preparer`, et `preparer` court à chaque chargement de
session** — c'est déjà ce que fait `service/reponses.rs:252`.

### 0.5 🔴 LE CACHE PEUT CASSER CE QUE F2 ET F3 ONT DÉJÀ RECETTÉ, ET C'EST LE RISQUE N°1 DE F5

Un cache d'énumération est **la seule addition de ce sous-projet qui puisse
rendre faux un comportement déjà mesuré**. Trois cas, et aucun n'est
hypothétique :

1. l'application crée un fichier dans la VM (F2, `Creer`), le liste ensuite :
   sans invalidation, **le fichier neuf est absent** ;
2. l'application renomme (F3, `Renommer`) : sans invalidation, l'ancien nom
   subsiste dans le listage **et le nouveau manque** ;
3. l'application supprime (F3, `Supprimer`) : sans invalidation, l'entrée
   supprimée **reste listée**.

**C'est le défaut de l'ancien pont**, que la spec §7.4 nomme : `src/file.js`
servait des octets depuis un cache **sans aucun TTL**, invalidé seulement par une
écriture passant par ce même pont — *« Une modification faite sur le poste local
n'était donc jamais vue, pour toujours. »*

**D3 pose l'invalidation par mutation**, et **la tâche 9 la joue en ROUGE** : le
cache armé **sans** invalidation, et les trois cas ci-dessus doivent tomber. Un
cache dont l'invalidation n'a jamais été vue manquer n'est pas un cache
invalidé — c'est un cache qu'on espère.

⚠️ **Une quatrième question, et elle est OUVERTE, donc elle est une PORTE (P3)** :
un fichier créé **localement dans la VM** (par l'application, sous ProjFS) est un
fichier réel sur le disque de la VM. **Le filtre le fusionne-t-il lui-même avec
ce que le fournisseur énumère, ou nous rappelle-t-il ?** Je ne le sais pas, et je
ne l'affirme pas. Si le filtre fusionne, l'invalidation du cas 1 est inutile mais
inoffensive ; s'il ne fusionne pas, elle est **indispensable**. **La porte P3 le
mesure avant que le code n'en dépende.**

### 0.6 🔵 CE QUE F5 PREND DES LEGS DE F4, ET SUR QUEL ARGUMENT

F4 laisse trois legs 🔴 **sans destinataire**. F5 est le dernier sous-bloc. Voici
ce qu'il en fait, un par un, avec l'argument de rattachement — parce qu'un
sous-bloc qui ramasse tout ce qui traîne n'est plus un sous-bloc.

| Legs de F4 | Décision de F5 | Argument |
| --- | --- | --- |
| **n°1** — la boucle plafonne à ~33 Kio/s, `ATTENTE_MAX` en est le terme dominant mais **pas le seul** | ⛔ **NON PRIS** | F4 l'écrit : « la parade est un **changement de conception de la boucle**, pas un réglage ». F5 ne touche pas `pont/transport.rs`. **Le prendre demanderait sa propre recette de débit, et F5 n'en a pas.** Reste sans destinataire |
| **n°2** — au-delà de ~3 150 entrées, **vingt secondes de gel puis une erreur opaque** | ✅ **PRIS, dans sa moitié de moindre coût** (D10) | **F5 touche le chemin du listage, et son cache le RENDRA PLUS DISCRET** : un listage servi par le cache masque le mur jusqu'à l'expiration du TTL. Un sous-bloc qui rend un mode d'échec plus difficile à voir doit laisser ce chemin capable de dire qu'il échoue. **Trois lignes et un test.** ⛔ Le découpage du protocole, lui, reste NON PRIS |
| **n°3** — `MORCEAUX_EN_VOL = 4` rend le produit **pire** à ce débit, **non éprouvé par mutation**, mutation **écrite et prête** | ✅ **ÉPROUVÉ, hors critère**, avec une règle d'admission écrite d'avance (D11) | F4 a abandonné le diagnostic quand un voisin a cassé le build (son §13.2). L'épreuve coûte **deux exécutions par bras** et rend lisible un rang qui échoue aujourd'hui. **Si la mesure ne montre pas ce que l'arithmétique annonce, la constante ne bouge pas** |

### 0.7 ⚠️ CE QUE F5 NE PREND PAS DES LEGS DES AUTRES, ET IL FAUT LE DIRE

- **Legs n°3 de F3** — *aucun éditeur réel n'a exercé l'idiome « fichier
  temporaire + renommage »*, que F4 nomme « la lacune la plus lourde du
  sous-projet ». ⛔ **NON PRIS** : il demande LibreOffice ou Word installés sur
  la VM et un geste d'interface, c'est-à-dire un montage que ce sous-projet n'a
  jamais eu. **Il sortira de ③ sans destinataire, et c'est la lacune la plus
  lourde que F5 laisse.**
- **Legs n°4 de F1** — *des lectures calent sans jamais expirer*. ⛔ **NON PRIS** :
  F4 a l'instrument (`plus_ancienne_ms`) et **n'a pas rencontré le symptôme** sur
  douze exécutions. On ne recette pas un symptôme qu'on ne sait pas provoquer.
- **`showDirectoryPicker()`, le modèle de permission, le mode `readwrite`** —
  legs de F1, reconduit par F2, F3 et F4. 🔴 **NON PRIS, ET C'EST GRAVE POUR F5
  EN PARTICULIER**, parce que le §6.4 cas 2 — *l'utilisateur revient avec un
  AUTRE répertoire* — **est** le modèle de permission. Voir §0.8.
- **Legs n°1 de F2** — la fenêtre de trente secondes. 🔵 **PAS PRIS COMME
  OBJECTIF, mais FERMÉ PAR CONSTRUCTION** par D6, et **mesuré comme
  corroboration** (tâche 16). Voir §0.9.
- **Legs n°6 de F1** — le condensat SHA-256 de bout en bout. ⛔ **NON PRIS.**
- **Legs n°6 de F4** — le coût de la canonicalisation de casse de F3. ⛔ **NON
  PRIS** : aucun geste de F5 ne l'exerce, comme aucun geste de F4.

### 0.8 🔴 CE QUE LE MONTAGE OPFS PEUT ET NE PEUT PAS ÉPROUVER DU §6.4 CAS 2

Le critère « le retour avec un répertoire différent **demande** à l'utilisateur au
lieu d'écrire » se décompose en deux moitiés, et **l'instrument n'en atteint
qu'une**.

| Moitié | Atteignable par OPFS ? |
| --- | --- |
| **La RÈGLE** — nom de racine différent ⇒ le pont **retient** au lieu de pousser, et l'annonce le dit | ✅ **OUI, entièrement.** L'instrument crée deux répertoires OPFS de noms différents et monte l'un puis l'autre. Le nom d'une racine OPFS est un `name` de `FileSystemDirectoryHandle` exactement comme celui d'un répertoire choisi par le sélecteur : **c'est la même valeur, lue au même endroit** |
| **LE MODÈLE DE PERMISSION** — `showDirectoryPicker()`, `queryPermission`, `requestPermission`, l'activation utilisateur transitoire, la persistance de l'autorisation entre deux visites | ⛔ **NON.** OPFS n'a **aucun** modèle de permission : `navigator.storage.getDirectory()` rend une poignée sans rien demander. **Rien de ce que F5 mesurera ne dit ce qu'un vrai retour d'utilisateur produit** |

🔴 **La conséquence est à écrire dans le document de résultats, pas à taire** :
`isSameEntry()` compare **deux poignées vivantes**, jamais une poignée à un
souvenir (spec §6.4 cas 2) — c'est pourquoi la v1 compare le **nom**, « un indice
et non une preuve ». **F5 mesure que la règle du nom fonctionne. Il ne mesure pas
qu'elle suffit**, et deux répertoires homonymes sur deux disques différents la
mettraient en défaut sans que rien ici ne le dise.

### 0.9 🔵 `Bonjour` FERME LA FENÊTRE DE TRENTE SECONDES DE F2 — par construction, et il faut le mesurer quand même

**Relevé dans le code** : `agent/src/pont/ecriture/fil.rs:170-194`, `Fil::demarrer`
appelle `fil.reprendre()` **au démarrage du fil**, c'est-à-dire au démarrage du
pont — sans savoir si un navigateur est là, ni lequel, ni sur quel répertoire.
**Aucun `Ordre` ne correspond à `CanalOuvert`** : l'énumération `Ordre`
(`fil.rs:106-115`) porte `Survenu`, `Fait` et `Echec`, et rien d'autre.

C'est exactement ce que F2 a mesuré (son §« défaut n°1 », **2 fois sur 2**) :
poussée du rejeu à `23:15:47.876`, montage annoncé par le navigateur à
`23:15:48.677` — **0,8 s APRÈS la poussée** —, et
`commande expirée … correlation=0` **+30,2 s** plus tard. *« L'indicateur qui
existe pour dénoncer la perte est MUET pendant trente secondes. »*

**D6 déplace la reprise de `Fil::demarrer` vers un `Ordre::Bonjour`.** Le pont ne
pousse alors **rien** avant que le navigateur n'ait dit qu'il est prêt — ce qui
est **littéralement le remède que F2 a nommé** : *« que le pont n'ouvre son canal
d'écriture qu'après un acquittement de l'écrivain »*.

⚠️ **Fermé par construction n'est pas mesuré.** La tâche 16 rejoue le scénario de
reprise de F2 et vérifie que `commande expirée … correlation=0` **n'apparaît
plus** et que le compteur du navigateur ne passe **jamais** par `dues: 0` alors
que le journal en porte. **Corroboration, pas critère** — parce que le remède
n'est pas l'objet de F5 et qu'un critère emprunté à un sous-bloc clos brouillerait
l'attribution.

---

## 1. Contraintes globales

### 1.1 Références d'entrée, RELEVÉES PAR LA COMMANDE le 21 août 2026

⚠️ **Chaque nombre ci-dessous a été mesuré au moment où il a été écrit.** Ce
dépôt a publié des chiffres faux à répétition — un plan en portait deux faux,
un autre sous-estimait ses croissances d'un facteur 2 à 4, deux messages de
commit ont porté des comptes d'exécutions faux **dans le commit qui les
corrigeait**. **Les remesurer à la tâche 1**, et ne recopier aucun.

**Arbre à `880548d`**, arbre de travail **propre** sur `agent/` et `proto/` au
moment du dernier relevé.

| Commande | Relevé |
| --- | --- |
| `cargo test -p agent` | **979 passed, 0 failed** |
| `cargo test -p proto` | **109 passed, 0 failed** |
| `cd client && npx vitest run` | **521 passed**, 45 fichiers |
| `cd client && npx vitest run --dir ../proto` | **298 passed**, 9 fichiers |
| `cargo check --target x86_64-pc-windows-gnu` | **exit 0, 34 avertissements**, tous famille `dead_code` |
| `cargo clippy --workspace` | **exit 0, 494 avertissements** sur `agent` |
| `stat -c '%F' /dev/null` | `character special file` ✅ |
| `virsh list --all` | `Windows | en cours d'exécution` |

⛔ **`./scripts/verify-all.sh` N'A PAS ÉTÉ LANCÉ, et je le dis plutôt que de
recopier son compte.** Il n'est **pas hermétique** : avec `TURN_URL`/`TURN_SECRET`
dans l'environnement — c'est-à-dire après le `set -a && source .env` que tout
travail sur la VM exige — six tests de `plateforme/src/signaling/server.test.ts`
échouent. **Le lancer depuis un shell propre, ou `env -u TURN_URL -u TURN_SECRET`.**
C'est à la tâche 1 de le faire, et de publier son compte **avec la commande
employée**.

🔴 **DEUX DIVERGENCES AVEC LE BRIEF DE CE PLAN, ET ELLES SONT MESURÉES.**

1. Le brief annonce « `clippy --workspace` porte **QUATRE** avertissements hors
   `dead_code` ». **Il y en a SIX** :
   `agent/src/capteur/sommeil.rs:46` (`unused imports: inscrire, retirer`),
   `agent/src/accent.rs:129` (`manual !RangeInclusive::contains`),
   `agent/src/capteur/vivier.rs:225` (`redundant closure`),
   `agent/src/mire.rs:48` (`manual .is_multiple_of()`),
   `agent/src/transport/piste_audio.rs:330` (`this map_or can be simplified`),
   plus un `very complex type`. **AUCUN dans `agent/src/pont/`.** Deux d'entre
   eux (`accent.rs`) sont d'un chantier voisin livré ces jours-ci ; le compte de
   quatre était probablement juste à sa date. **Le remesurer, jamais le
   recopier.**
2. `grep -c '^warning:' ` sur la sortie de `cargo check` rend **35**, et cargo
   écrit lui-même **34** : *le `grep` compte AUSSI la ligne de résumé*. Piège
   déjà payé par le sous-bloc P1 du presse-papier. **Lire le nombre que cargo
   annonce.**

✅ **Le tableau de dette de `CLAUDE.md` a DEUX lignes, et mon balayage le
confirme** — `agent/src/encode.rs` **1536**, `agent/src/windows_source.rs`
**630**, et **aucun autre fichier de code source au-dessus de 500**. ⚠️ Les deux
lignes `proto/src/plateforme/tests.rs` (561) et `proto/ts/plateforme.test.ts`
(512) qu'un état antérieur de ce fichier portait **ont disparu du balayage** :
`proto/ts/plateforme.test.ts` vaut **462** et l'autre n'apparaît plus. **Ce n'est
pas à F5 de corriger `CLAUDE.md` sur ce point** — la correction est d'un chantier
voisin, et trois chantiers ont déjà failli se l'attribuer.

### 1.2 🔴 PÉRIMÈTRE CONCURRENT — et la VM est un préalable EXTERNE, pas une dépendance de tâche

**Un chantier travaille en ce moment dans le même arbre : la gestion d'apps G5**,
qui tient :

- 🔴 **la VM Windows** (sa recette) — elle est **en cours d'exécution** au moment
  où ce plan est écrit ;
- `plateforme/src/http/`, `client/src/hub/`, `agent/src/apps/`.

**Ce qui est à F5** : `agent/src/pont/` (et lui seul dans `agent/`),
`proto/src/fichiers*`, `proto/ts/fichiers*`, `proto/fichiers-vectors.json`,
`client/src/fichiers/`, `client/src/shell.ts`, `client/src/shell-page.ts`,
`client/shell.html`, `scripts/run-agent.sh` (**une ligne**), et
`docs/superpowers/plans/journaux-pont-fichiers-f5/`.

**Ce qui n'est à personne de F5** : tout le reste, `CLAUDE.md` excepté à la
dernière tâche.

🔴 **LA VM EST UN PRÉALABLE EXTERNE, ET C'EST LA FORMULATION QUI COMPTE.** Elle
n'est **pas** une dépendance de tâche que l'on attend : c'est une ressource
exclusive dont l'indisponibilité **arrête proprement** le sous-bloc au lieu
d'écraser le binaire d'un voisin.

1. **Avant toute tâche qui touche la VM**, relever qui la tient :
   ```bash
   virsh list --all
   node scripts/winrm.js 'Get-Process agent -ErrorAction SilentlyContinue | Select-Object Id,StartTime'
   git log --oneline -5
   ```
2. **Si un agent y tourne et qu'aucune tâche de F5 ne l'a lancé, S'ARRÊTER** et
   le déclarer. **Ne jamais tuer un agent qu'on n'a pas lancé** : F1 a perdu une
   exécution entière parce que deux se sont recouvertes de 2 min 23 s, et *le
   journal versé sous le nom de la première était celui de la seconde*.
3. 🔴 **`git status --porcelain proto/ agent/` doit être VIDE avant CHAQUE
   build — pas seulement avant le premier.** `sync-agent.sh` ne synchronise que
   ce que git suit : un voisin qui déclare un module dont le fichier n'est pas
   encore suivi fait échouer le build sur la VM. F4 a payé exactement cela (son
   §13.2) en ayant vérifié **avant le premier build et pas avant les suivants**.
   🔵 **J'AI VU CE PIÈGE SE REPRODUIRE EN ÉCRIVANT CE PLAN** : mon premier
   `cargo check --target x86_64-pc-windows-gnu` a **échoué** (`E0308`) pendant
   que G5 éditait `agent/src/apps/associations/` — fichier alors **non suivi** —,
   et la même commande relancée dix minutes plus tard rend **exit 0**. *Une
   compilation qui échoue dans un arbre partagé se REMESURE avant d'être
   attribuée à quiconque.*
4. **Le binaire est un état partagé.** `build-agent.sh` échoue en
   `Accès refusé (os error 5)` tant qu'un agent tourne. `cargo clean --release
   -p proto -p agent`, **les deux crates** — purger `agent` seul laisse le rlib
   de `proto` qui paraît plus récent que ses sources, l'horloge de la VM avançant
   sur celle de l'hôte.
5. 🔴 **La VM s'éteint toute seule, et le déclencheur EST identifié** :
   `/var/log/libvirt/qemu/Windows.log` porte `terminating on signal 15 from pid
   <N>`, et ce PID est **`/usr/sbin/libvirtd --timeout 120`** — le démon s'arrête
   sur inactivité et **emporte le domaine**. ⚠️ **CE N'EST PAS LE MÉCANISME DE
   D1** (hibernation initiée *dans* l'invité, Kernel-Power 187/42) : ici c'est
   **l'hôte** qui tue. Les confondre ferait chercher du mauvais côté.
   **Conséquence directe pour F5** : la tâche 14 **redémarre délibérément la VM**,
   et son instrument doit distinguer un redémarrage voulu d'une extinction subie.
6. **L'hôte porte un défaut non identifié** : `/dev/null` a été trouvé remplacé
   par un fichier ordinaire. Contrôle avant toute séquence VM :
   `stat -c '%F' /dev/null` → `character special file`. *(Relevé le 21 août 2026 :
   conforme.)*

### 1.3 🔴 `cd client && npx vitest run` NE COUVRE PAS `proto/ts/`

La racine Vitest est `client/`. **Deux commandes, jamais une** :

```bash
cd client && npx vitest run              # client/src/ seul  → 521
cd client && npx vitest run --dir ../proto   # proto/ts/     → 298
```

**F5 touche les DEUX**, contrairement à F4 : `proto/ts/fichiers.ts` et
`proto/ts/fichiers-entetes.ts` reçoivent les deux annonces neuves.

### 1.4 Règles de travail

**Commits.** `git add` **nominatif**, jamais `git add -A` : l'index porte du
travail qui n'est pas à nous — au moment où ce plan est écrit, il porte celui de
G5. Message par fichier : `git commit -F <fichier> -- <chemins>`, puis
**`git show --name-only`** pour vérifier ce qui est effectivement parti.

🔴 **CHAQUE COMMIT DOIT COMPILER, ET IL Y A UNE RÈGLE D'ORDRE QUI L'IMPOSE.**
*Un `match` exhaustif dans un crate aval rend toute addition de variante amont
CASSANTE : le bras part dans le MÊME commit que la variante.* Un chantier a cassé
l'arbre entre deux de ses commits parce qu'**aucun des deux ordres possibles ne
compilait**, et cela a coûté une mesure à un voisin.

Pour F5, cela vise nommément :

- `proto::fichiers::CodeEchec` et les `match` qui l'épuisent
  (`agent/src/pont/service.rs::cause_de`) ;
- toute variante ajoutée à `pont::ecriture::fil::Ordre` — `Fil::traiter`
  l'épuise ;
- toute variante ajoutée à `pont::transport::DuNavigateur` — `service.rs:95-137`
  l'épuise.

**Le balayage des tailles, PAR LA COMMANDE, sur TOUT l'arbre, avant CHAQUE
commit** — jamais seulement à la clôture. **Sept chantiers récents ont franchi
des plafonds sans les voir passer**, et F3 en a commité un
(`pont/ecriture/fil.rs` porté de 478 à **612**, commit parti avec) :

```bash
unset -f chpwd
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>460 && $2!="total"'
```

⚠️ **Le seuil est 460, pas 500** : c'est la marge dont on a besoin pour voir
venir. ⚠️ **Et documenter une extraction reprend une part de la marge qu'elle
rend** : S2 l'a payé deux fois dans le même fichier, et une revue transverse y a
ajouté 48 lignes de commentaire. **Compter l'en-tête du module extrait dans le
budget de l'extraction.**

⚠️ **UNE EXTRACTION RIGOUREUSEMENT VERBATIM NE COMPILE PAS.** Un élément privé
d'un module enfant n'est pas visible de son parent : `pub(super)` sur un champ
n'emporte pas son **type**, et `rustc` le dit par `private_interfaces` — *un
avertissement d'une autre famille que `dead_code`*, que ce dépôt vérifie par sa
nature à chaque clôture. `agent/src/pont/ecriture/fil.rs:138-142` porte déjà ce
constat, payé par l'extraction de `fil/mutations.rs`.

**Le harnais de rouge.** Chaque mutation est jouée **seule** :

1. `cp <fichier> /tmp/f5-temoin/<fichier>` — **une COPIE NOMMÉE, jamais `HEAD`** ;
2. muter **par NUMÉRO DE LIGNE**, jamais par sous-chaîne ;
3. `diff /tmp/f5-temoin/<fichier> <fichier>` — **s'il est VIDE, la rouge n'a pas
   eu lieu**, et c'est un échec du harnais, pas un succès du produit ;
4. jouer le contrôle, **lire QUELLE assertion tombe**, jamais seulement le code
   de sortie ;
5. restaurer par **`cp` SANS `-p`** ;
6. `diff` de nouveau — **vide** — et `sha256sum` identique ;
7. `git status --porcelain <chemins>` conforme à l'état d'avant.

🔴 **ROUGE 0, obligatoire, et elle est jouée AVANT toutes les autres** : une
mutation **vide** (`cp` du témoin sur lui-même, sans rien changer), passée au
harnais. **Le harnais doit la REFUSER à l'étape 3.** Un harnais qu'on n'a jamais
vu refuser ne refuse rien — et **cinq rouges sont restées vertes dans ce dépôt
ces derniers jours** pour des raisons voisines.

🔴 **UNE ROUGE RESTÉE VERTE SE DIAGNOSTIQUE, JAMAIS NE SE CLASSE.** L'une d'elles
a révélé que la consommation du vrai code n'était couverte par rien. Les deux
causes déjà payées, à vérifier dans cet ordre :

- **la ligne à muter existe DEUX FOIS** (la seconde dans un commentaire, ou dans
  un test) ;
- **le garde a DEUX sites**, et n'en muter qu'un laisse l'autre faire le travail.

**Le shell de l'hôte.** `unset -f chpwd` avant tout relevé : un hook `chpwd`
injecte un `ls` dans toute sortie dès qu'un `cd` court dans un sous-shell. **Pas
de backticks dans un `echo` de journal** : ils exécutent une commande. **Pas de
`echo` avec échappements en zsh** dans un fichier qui décrit ses propres motifs :
il les interprète, et le journal se pollue lui-même (S3).

### 1.5 🔴 LA DISCIPLINE D'ÉNONCÉ — elle s'applique à CHAQUE tâche, pas à la recette

1. **Nommer le nombre d'exécutions dans CHAQUE énoncé.** « Deux exécutions » et
   « une seule » ne se lisent pas pareil. **Aucun taux n'est revendiqué nulle
   part** : deux exécutions établissent la **reproductibilité**, jamais une
   fréquence.
2. **Un ZÉRO SE QUALIFIE AVANT DE SE RAPPORTER, et seul un TÉMOIN NÉGATIF le
   fait.** Un chantier vient d'interroger un binaire au mauvais chemin : ses
   trois chaînes discriminantes rendaient **0**, **et son témoin négatif aussi**.
   Sans ce témoin il aurait rejoué toute sa campagne. **Tout contrôle par chaîne
   de F5 porte son témoin négatif** — une chaîne dont on SAIT qu'elle est
   présente.
3. 🔴 **LE CONTRÔLE PAR LA TAILLE DU BINAIRE NE PROUVE RIEN, DANS LES DEUX
   SENS.** F4 l'a mesuré : un vert restauré pesait **exactement** autant que son
   rouge (10 708 480 o) sans être le même binaire, **et deux compilations de la
   même source** rendent 10 647 552 puis 10 708 480. **Ce qui tranche est une
   chaîne qu'on a soi-même posée** — et **par un `tracing::warn!`, jamais par un
   `const &str`**, que le compilateur élimine (F4 §12.4). ⚠️ **Et une chaîne
   COURTE ne prouve rien de son absence** : le compilateur inline les littéraux
   courts en constantes immédiates coupées aux frontières de mot machine. **Poser
   une chaîne d'au moins vingt caractères.**
4. **Ne rien affirmer au-delà de ce qui a été relevé.** Ce dépôt a vu deux pièces
   **fabriquées** — un fait vrai présenté sans sa preuve —, et c'est le mode de
   défaillance qu'il juge le plus grave.
5. **`grep` sur un identifiant de sous-bloc rend des occurrences qui ne sont pas
   les siennes.** « F5 » est aussi le cinquième défaut de la revue transverse de
   **D7** (l'identité d'une session par son seul nom). **Trier avant d'écrire.**
6. **`grep` sans `-a`** sur un journal à octets NUL rend une **sortie vide**, pas
   un zéro.
7. **Un palier de mesure doit être PLUSIEURS FOIS plus long que la temporisation
   du mécanisme qu'il observe** — et **lire les constantes du code AVANT de le
   dimensionner**. Pour F5 : `DELAI_LISTER` (**20 s**), `ATTENTE_MAX` (**20 ms**),
   `PERIODE_BALAYAGE` (**250 ms**), `PERIODE_RECENSEMENT` (**10 s**),
   `PERIODE_HYDRATATION` (**60 s**), `DELAI_ECRIRE` (**30 s**), et
   `TTL_ENUMERATION` que F5 pose lui-même. ⚠️ **Pour le critère ① c'est
   l'INVERSE** : la fenêtre d'observation doit être **PLUS COURTE** que le TTL —
   voir la règle d'admission de D4.
8. **L'instrument peut détruire ce qu'il mesure.** Compter ou échantillonner,
   jamais tracer par unité. `agent/src/pont/projfs/etat.rs:324-338` porte déjà ce
   raisonnement pour l'occupation disque, et **la porte P1 le remet à l'épreuve
   plutôt que de le recopier**.
9. **Un motif de recette se vérifie contre le CODE, jamais contre la spec** :
   trois des quatre contrôles de F1 cherchaient des chaînes inexistantes. **Et
   une sonde qui croit reproduire un geste doit le RELIRE, pas s'en souvenir.**
10. ⚠️ **Un instrument qui rend un chiffre plausible mais petit mesure souvent
    autre chose que ce qu'on croit** — l'un rendait *10 événements pour
    1 000 fichiers* : il comptait la pompe d'événements de PowerShell.

---

## 2. Structure des fichiers

### 2.1 Modules créés

| Fichier | Visé | Nature |
| --- | --- | --- |
| `agent/src/pont/cache.rs` | ≤ 220 | **PUR** — le cache d'énumération, indexé par chemin, **horloge INJECTÉE** ; aucune dépendance à `windows` |
| `agent/src/pont/cache/tests.rs` | ≤ 220 | tests d'hôte, `#[path]` non requis (parent non gaté) |
| `agent/src/pont/bonjour.rs` | ≤ 160 | **PUR** — la règle « pousser ou retenir » à partir du nom de racine mémorisé et du nom annoncé, plus la lecture/écriture du nom mémorisé rendue par valeur |
| `agent/src/pont/bonjour/tests.rs` | ≤ 160 | tests d'hôte |
| `agent/src/pont/ecriture/fil/contrat.rs` | ≤ 90 | **extraction préalable** — voir §2.3 |
| `client/src/fichiers/protocole.annonces.test.ts` | ≤ 260 | **extraction préalable** — voir §2.3 |

⚠️ **Convention de module enfant** (`CLAUDE.md`) : **aucun de ces modules n'en
relève.** Elle ne s'applique qu'aux modules extraits d'un parent
`#[cfg(windows)]` pour compiler sur l'hôte. `pont.rs` n'est pas gaté : `cache` et
`bonjour` sont des enfants ordinaires, déclarés par un simple `mod`, **aucun
`#[path]`**.

### 2.2 Modifiés — tailles **RELEVÉES PAR LA COMMANDE le 21 août 2026**

| Fichier | Lignes | Marge à 500 | Ce que F5 y met |
| --- | --- | --- | --- |
| 🔴 `client/src/fichiers/protocole.test.ts` | **484** | **16** | les deux annonces neuves — **extraction obligatoire d'abord** |
| 🔴 `agent/src/pont/ecriture/fil.rs` | **472** | **28** | `Ordre::Bonjour`, et le retrait de `reprendre()` — **extraction obligatoire d'abord** |
| ⚠️ `proto/ts/fichiers-entetes.ts` | **387** | 113 | deux en-têtes ; **~427 attendu**, au-dessus du seuil de balayage de 460 ? non — **à remesurer** |
| `agent/src/pont/projfs/etat.rs` | **354** | 146 | l'occupation disque |
| `client/src/fichiers/protocole.ts` | **354** | 146 | l'émission des deux annonces |
| `agent/src/pont/service.rs` | **341** | 159 | 🔴 **l'aiguillage AVANT `resoudre`** (D2) |
| `client/src/shell-page.ts` | **375** | 125 | les deux boutons, l'envoi de `Bonjour` |
| `client/src/shell.ts` | **309** | 191 | l'état « retenues », le libellé |
| `agent/src/pont.rs` | **265** | 235 | le câblage du cache et du nom de racine |
| `client/src/fichiers/canal.ts` | **262** | 238 | 🔴 le `send()` refusé rend un `Echec` (D10) |
| `proto/src/fichiers.rs` | **259** | 241 | `TYPE_BONJOUR`, `TYPE_RAFRAICHIR`, la **quatrième famille** |
| `proto/src/fichiers/entetes.rs` | **237** | 263 | `Bonjour`, `Rafraichir`, `Dues.retenues` |
| `proto/ts/fichiers.ts` | **226** | 274 | les deux constantes |
| `proto/fichiers-vectors.json` | **210** | — | les vecteurs des deux en-têtes |
| `agent/src/pont/projfs/rappels/listage.rs` | **165** | 335 | la consultation du cache |
| `agent/src/pont/enumeration.rs` | **140** | 360 | 🔵 **son en-tête réfute son propre pronostic** — voir §7 |
| `client/shell.html` | **110** | — | deux boutons |
| `agent/src/pont/ecriture/fil/reprise.rs` | **62** | 438 | la reprise devient conditionnelle |
| `scripts/run-agent.sh` | **164** | — | **une ligne**, `PONT_CACHE` |

⛔ **NON TOUCHÉS, ET C'EST UNE DÉCISION** : `agent/src/pont/transport.rs` (290) et
`agent/src/pont/transport/tests.rs` (**474**, marge **26**). D2 place l'aiguillage
dans `service.rs`, précisément pour que le transport reste une couche qui ne lit
qu'une corrélation. **Ne pas y toucher est aussi ce qui évite d'ouvrir une porte
à 26 lignes.**

### 2.3 🔴 LES TROIS PORTES, BUDGÉTÉES D'AVANCE, AVEC LEUR POINT DE CHUTE NOMMÉ

**Porte A — `agent/src/pont/ecriture/fil.rs`, 472, marge 28.**
`Ordre::Bonjour` ajoute une variante documentée (~10 lignes) et son bras dans
`Fil::traiter` (~8). **482 à 490 : trop serré pour la revue qui suivra.**
**Extraction préalable, obligatoire, dans une tâche DÉDIÉE jouée AVANT
l'addition** — le geste que D9 a inventé et que D10 a joué trois fois :
`enum Ordre` (l. 97-115) et `struct Config` (l. 116-127) partent **verbatim**
vers `agent/src/pont/ecriture/fil/contrat.rs`, avec un `pub use contrat::{Config,
Ordre};` dans `fil.rs` pour qu'**aucun site d'appel ne bouge**. Attendu : `fil.rs`
→ **~437**, marge **63**.
⚠️ **Les deux sont `pub`** : l'extraction compile. Ne **pas** y emporter `Fil` ni
`EnCours`, `pub(super)`, dont le type ne suivrait pas la visibilité.

**Porte B — `client/src/fichiers/protocole.test.ts`, 484, marge 16.**
**Extraction préalable, obligatoire** : les tests de `TYPE_DUES` (la famille
« annonce ») partent vers `client/src/fichiers/protocole.annonces.test.ts`, où
les tests des deux annonces neuves les rejoindront. Attendu : le parent
**≤ 400**.

**Porte C — `proto/ts/fichiers-entetes.ts`, 387.**
Deux en-têtes neufs et leurs gardes de forme : **~427 attendu**, sous 460 mais
**au-dessus du seuil de vigilance**. **Pas d'extraction préalable prescrite** ;
**le balayage de la tâche qui l'édite décide**, et son point de chute est nommé
d'avance : `proto/ts/fichiers-entetes-annonces.ts`.

⚠️ **Si une extraction se déclenche ailleurs, elle se joue AVANT l'addition, et
JAMAIS par compression** — `CLAUDE.md` l'interdit nommément, et D9 l'a fait deux
fois avant de devoir extraire quand même.

---

## 3. Les décisions, tranchées

### D1 — DEUX annonces neuves, une QUATRIÈME famille, et `FICHIERS_VERSION` reste à **1**

`proto/src/fichiers.rs` porte aujourd'hui **trois** familles, nommées dans le
fichier : requêtes pont → navigateur (1,2,3,4,5,7,8), **annonces** pont →
navigateur (6), réponses navigateur → pont (64,65,66,67,127).

**F5 en ouvre une quatrième : les annonces NAVIGATEUR → PONT.**

| Constante | Valeur | En-tête | Charge |
| --- | --- | --- | --- |
| `TYPE_BONJOUR` | **68** | `Bonjour { racine: String, forcer: bool }` | vide |
| `TYPE_RAFRAICHIR` | **69** | `{}` | vide |

**Elles n'attendent AUCUNE réponse, et leur corrélation est IGNORÉE.** La liste
de cette famille est **CLOSE** — même clause que la famille des annonces
pont → navigateur, et pour la même raison : *c'est ce qui l'empêche de devenir le
bras fourre-tout silencieux que ce dépôt a payé cinq fois sur `pont_media.rs`*.

**`FICHIERS_VERSION` reste à 1**, avec l'argument que F2 a écrit et que je
reprends parce qu'il tient : *la rupture est **ADDITIVE**, les deux bouts sont
livrés ensemble, et incrémenter casserait la compatibilité dans le seul sens où
elle n'a aucune valeur.*

🔴 **UNE RÉSERVE, ET ELLE EST RÉELLE : `Bonjour` n'est pas seulement additif.** Il
change le **moment** où la reprise se produit (D6). Un client d'**avant F5**
face à un pont **de F5** ne l'enverrait jamais, et les écritures dues ne
partiraient **plus jamais** — un silence, c'est-à-dire pire que les trente
secondes de F2. **D6 traite ce cas nommément, par une trace, et jamais par un
repli qui pousserait.**

### D2 — 🔴 L'AIGUILLAGE DES ANNONCES PRÉCÈDE `resoudre`, ET C'EST LA DÉCISION QUI PORTE TOUT LE SOUS-BLOC

Dans `agent/src/pont/service.rs::traiter`, **immédiatement après `decoder` et
AVANT `table.resoudre`** :

```rust
    // 🔴 LA QUATRIÈME FAMILLE N'A PAS DE CORRÉLATION, ET `resoudre` LA JETTERAIT
    // EN `debug!` — invisible sous RUST_LOG=info.
    match trame.type_message {
        proto::fichiers::TYPE_RAFRAICHIR => { … return; }
        proto::fichiers::TYPE_BONJOUR    => { … return; }
        _ => {}
    }
```

**Pourquoi ici et pas dans `transport.rs`** : le transport ne lit qu'une
corrélation et n'interprète aucun type (relevé, `transport.rs:245-250`). Y mettre
un aiguillage par type le ferait connaître le protocole, et il faudrait alors une
variante de plus dans `DuNavigateur` — **dont `service.rs:95-137` épuise le
`match`**, donc un commit qui casse (§1.4). **Un seul endroit, et c'est celui qui
connaît déjà les types.**

**La rouge**, jouée à la tâche 9 : déplacer ce bloc **après** `resoudre`.
Attendu : le bouton `Rafraichir` cesse d'agir, le fichier apparaît sans lui, et
**le journal ne porte rien d'autre** que le `debug!` — c'est-à-dire rien du tout
sous `RUST_LOG=info`. **C'est ce silence-là qu'il faut avoir vu une fois.**

### D3 — Le cache d'énumération : PUR, indexé par CHEMIN, horloge INJECTÉE, invalidé par TROIS voies

`agent/src/pont/cache.rs`, **aucun `cfg`**, testé sur l'hôte :

```rust
pub struct CacheEnumeration { /* chemin -> (Vec<Entree>, Instant) */ }
impl CacheEnumeration {
    pub fn lire(&mut self, chemin: &str, maintenant: Instant) -> Option<&[Entree]>;
    pub fn poser(&mut self, chemin: String, entrees: Vec<Entree>, maintenant: Instant);
    pub fn invalider(&mut self, chemin: &str);          // une mutation
    pub fn vider(&mut self);                            // Rafraichir
}
```

**L'horloge est un PARAMÈTRE, jamais `Instant::now()` à l'intérieur** — c'est ce
qui rend l'expiration testable **sans dormir**, exactement comme `pont::table`
l'a fait pour ses délais.

**Il stocke les entrées BRUTES**, avant `enumeration::preparer` (§0.4).

**Trois voies d'invalidation, et pas une de plus :**

1. **`Rafraichir`** → `vider()` **ET** `PrjClearNegativePathCache` (D9) ;
2. **l'expiration** → `TTL_ENUMERATION` (D4) ;
3. 🔴 **toute mutation que le pont connaît** → `invalider(parent(chemin))`, sur
   les quatre variantes de `pont::ecriture::Evenement` (`Modifie`, `Cree`,
   `Renomme` — **les deux parents**, source et destination —, `Supprime`), **et**
   sur les réponses `Fait` aux verbes `Creer`, `Renommer`, `Supprimer` que le
   pont a lui-même émis.

⚠️ **Les deux moitiés de (3) sont nécessaires et ne se recouvrent pas** : les
notifications ProjFS disent ce qui a changé **dans la VM**, les réponses `Fait`
disent ce que **le navigateur** a fait sur demande du pont. **La porte P3
mesurera si la première est même atteignable** (§0.5).

⚠️ **`invalider` prend le PARENT, jamais le chemin lui-même** : un cache
d'énumération est indexé par **répertoire**. Se tromper là est silencieux — le
cache ne serait jamais invalidé et rien ne le dirait. **Un test d'hôte le
verrouille.**

### D4 — `TTL_ENUMERATION = 30 s`, NON CALIBRÉE, et la règle d'admission qu'elle impose à la recette

**Valeur : 30 secondes.** Elle rejoint `BPP_MIN`, `FACTEUR_FOCUS`,
`PART_DORMANTE_BPS`, `HYSTERESIS`, `REPIT_APRES_ECHEC`, `TAILLE_MAX_SORTIE`,
`REPIT_REARMEMENT_AUDIO`, `REARMEMENTS_MAX`, `TAILLE_TRAME_MAX`, `DELAI_LISTER`,
`ATTENTE_MAX` et les treize seaux de `pont::latence` **dans la liste des
constantes de ce dépôt qu'aucune mesure n'a jugées**.

**Ce qui a présidé au choix, et qui n'est pas une calibration** :

- elle doit être **plus longue** qu'un geste de recette complet — F4 mesure un
  listage de 1 000 entrées à **~6 s**, et le protocole du critère ① enchaîne deux
  listages plus une addition de fichier ;
- elle doit être **plus courte** que le temps qu'un utilisateur accepte de voir
  un contenu périmé sans que rien ne le lui dise ;
- ⚠️ **elle ne doit PAS valoir 60 s**, qui est la valeur de `PERIODE_HYDRATATION`
  (relevée, `service.rs:74`). Deux constantes qui se recalibreraient séparément
  et qui portent le même nombre finissent par se croire liées — le dépôt l'écrit
  déjà de `PLAFOND_DISSIMULATION` et `micro::PLAFOND`.

🔴 **RÈGLE D'ADMISSION DE LA RECETTE, ÉCRITE D'AVANCE.** L'instrument du critère ①
**enregistre le temps écoulé** entre le listage d'amorçage et le listage de
vérification. **Si cet écart dépasse `TTL_ENUMERATION`, l'exécution est
DISQUALIFIÉE, pas rapportée.** Raison : le cache expiré rendrait le fichier
visible, donc un **FAUX ROUGE**, et l'imputer au produit serait une erreur
d'attribution. *C'est le pendant exact du piège « un palier doit être plusieurs
fois plus long que la temporisation observée », dans l'autre sens.*

### D5 — `PONT_CACHE=0` désarme le cache, et c'est le bras rouge du critère ①

| Variable | Convention | Lue dans |
| --- | --- | --- |
| `PONT_CACHE=0` | **`=0` DÉSARME ; une simple présence n'active pas** — celle de `PONT_ECRITURE`, `PONT_MUTATION`, `SUPERVISEUR`, `CAPTEUR`, `AUDIO`, `PLEIN_ECRAN`, `PART_SONDAGE`, `PRESSE_PAPIER` | `agent/src/pont.rs`, par `OnceLock`, comme `PONT_ECRITURE` (`pont.rs:136`) |

**Trace, émise SEULEMENT si désarmé**, sur le modèle exact de `PONT_ECRITURE` :
`cache d'enumeration DESARME (PONT_CACHE=0) : bras de banc, jamais une
configuration livree` (`warn!`).

🔵 **C'est le bras rouge du critère ① sur le PRODUIT lui-même** : cache désarmé,
le fichier ajouté localement **apparaît sans `Rafraichir`**, et le critère tombe.
Un rouge provoqué sur un binaire dont le mécanisme est **présent** et le résultat
**absent** — la forme que D10 a nommée après avoir produit un rouge **vacueux**.

🔴 **Tâche DÉDIÉE pour `scripts/run-agent.sh`, jouée AVANT que quiconque en ait
besoin, et le contrôle qui compte est le `run-agent.ps1` GÉNÉRÉ, jamais le tracé
du code.** Piège payé **six fois** — `SUPERVISEUR` (D1), `MULTIFENETRE_REPRISE`
(D2), `AUDIO` (D7), `BUDGET_BPS` (D6, évité par une tâche dédiée), `PONT_MESURE`
(F4), et une septième par l'instrument d'un chantier qui le connaissait : *trois
exécutions demandaient 1 000, 5 000 et 20 000 et les trois ont tourné à 1 000*.

### D6 — `Bonjour` : la reprise quitte `Fil::demarrer`, et le nom de racine est PERSISTÉ

**Le nom de racine mémorisé vit à côté du journal**,
`%LOCALAPPDATA%\Guacamole\pont\racine.nom` — `projfs::dossier_etat()`, déjà
`pub` depuis F2 (`racine.rs:47-58`). **Hors de la racine**, pour la raison que ce
fichier-là écrit trois fois : *un état qui vivrait DANS la racine serait lui-même
un objet projeté, donc dépendant du pont pour être lu — circulaire — et il
disparaîtrait avec elle exactement le jour où il sert.*

**Le changement, en trois gestes :**

1. `Fil::demarrer` **n'appelle plus `reprendre()`** ;
2. `Ordre::Bonjour { racine, forcer }` l'appelle, **conditionnellement** ;
3. `agent/src/pont/bonjour.rs` (**PUR**) porte la règle :

| Nom mémorisé | Nom annoncé | `forcer` | Décision |
| --- | --- | --- | --- |
| absent | quelconque | — | **POUSSER**, et mémoriser. *Premier montage : rien ne peut être mal placé, le journal étant vide ou né de ce même montage* |
| `X` | `X` | — | **POUSSER** |
| `X` | `Y ≠ X` | `false` | 🔴 **RETENIR** : annoncer les dues avec `retenues: true`, ne rien pousser, **et mémoriser `Y`** seulement si l'utilisateur confirme |
| `X` | `Y ≠ X` | `true` | **POUSSER**, et mémoriser `Y` |

⚠️ **Le journal N'EST PAS vidé quand on retient**, et c'est le point : *rejouer
aveuglément écrirait les fichiers d'une session dans le dossier d'une autre*
(spec §6.4 cas 2), mais les jeter perdrait la donnée. **On ne fait ni l'un ni
l'autre : on nomme.**

**Le cas « aucun `Bonjour` n'arrive »** : au terme de `DELAI_BONJOUR` (**10 s**,
**non calibrée**) après `CanalOuvert`, un `warn!` — `canal ouvert depuis
<n> s sans Bonjour : aucune ecriture due ne sera poussee` — **et rien d'autre**.
🔴 **Pas de repli qui pousserait** : un repli rouvrirait exactement le danger du
cas 2, celui pour lequel `Bonjour` existe.

### D7 — `Dues` gagne `retenues: bool`, additif

`proto::fichiers::entetes::Dues` gagne `#[serde(default)] pub retenues: bool`.
**Additif** : un lecteur d'avant F5 l'ignore. Le navigateur en tire le libellé et
l'affichage du bouton « Reprendre l'enregistrement ».

⚠️ **Le vecteur partagé `proto/fichiers-vectors.json` porte les DEUX formes** —
avec et sans le champ —, pour que le défaut par défaut soit épinglé et non
supposé.

### D8 — L'occupation disque : une PORTE (P1) avant toute ligne de code

La spec demande « la mesure de l'occupation disque de la racine et sa trace »
(§6.4, §10 R4). `agent/src/pont/projfs/etat.rs:324-338` **refuse déjà** la voie
évidente, et son argument tient :

> Mesurer la taille occupée par la racine demanderait de la parcourir — donc de
> traverser ProjFS, donc de déclencher nos propres rappels d'énumération […]
> **L'instrument détruirait ce qu'il mesure.**

🔴 **ET F5 LE RENFORCE** : un parcours de fond **empoisonnerait le cache que F5
vient de poser** avec des listages que l'utilisateur n'a pas demandés. La voie du
parcours est donc **fermée deux fois**.

**La porte P1 mesure trois candidates, et accepte `NON MESURABLE` :**

| # | Candidate | Ce qu'elle mesure vraiment | À vérifier |
| --- | --- | --- | --- |
| **a** | le compteur d'hydratation **PERSISTÉ** entre exécutions du pont | ce que **le pont a écrit**, cumulativement — ce qu'`etat.rs` déclare manquer aujourd'hui | rien à mesurer : c'est du code |
| **b** | `GetDiskFreeSpaceExW` sur le volume qui porte la racine | **le disque de la VM**, qui est la question littérale de R4 | 🔴 confondeur **non levable** : tout le reste de la VM y compte aussi |
| **c** | `std::fs::metadata` sur **les seuls chemins que le pont a hydratés** | la taille **logique** de ces fichiers-là | 🔴 **deux questions ouvertes** : (i) un `metadata` sur un substitut déjà posé traverse-t-il ? *F4 mesure `GetPlaceholderInfo` chaud à 0,4 ms, **AUCUNE traversée** — indice fort, pas preuve* ; (ii) `len()` est la taille **logique**, pas l'occupation ; un substitut non hydraté la porte quand même |

**Ce que la porte joue** : (i) mille `metadata` sur des chemins hydratés,
compteurs de traversées du pont **avant et après** — attendu **zéro** ; (ii) la
même chose sur des chemins **jamais touchés** — attendu **non nul**, et *c'est ce
témoin qui rend le zéro lisible* ; (iii) `GetDiskFreeSpaceExW` avant et après
l'hydratation d'un fichier connu, pour voir si le delta est du bon ordre.

**Issue admise d'avance** : si (c-i) traverse, **la candidate c tombe**, et F5
livre **a + b** avec le confondeur de b **nommé dans la trace elle-même**. Si
(c-i) ne traverse pas, F5 livre **a + b + c**, et c porte sa réserve (ii).

⚠️ **`NON MESURABLE` est une issue acceptable et prévue**, comme pour l'A/B de
`set_desired_bitrate` en D9/D10 et pour les portes de G3 et G4. **Ce qui n'est
pas acceptable est un chiffre sans dire ce qu'il mesure.**

### D9 — `PrjClearNegativePathCache` gagne enfin un appelant ; `PrjDeleteFile` n'en gagne pas

**Relevé** : `agent/src/pont/resolution.rs:39-41` et
`projfs/chargement.rs:246-256` **chargent** les deux entrées, et
`grep -rn "vider_cache_negatif\|supprimer_fichier" agent/src/` ne rend **aucun
site d'appel de production**. Ce sont deux des cinq entrées sans jumeau `PRJ_*_CB`
que **R7 tient pour ouvert depuis F1**.

- **`PrjClearNegativePathCache`** : appelée par `Rafraichir`. **R7 se referme
  d'UNE entrée sur cinq**, et il faut le dire ainsi — pas « R7 est fermé ».
  ⚠️ **Et son résultat est journalisé** : la fonction rend le nombre d'entrées
  purgées par son `totalentrynumber`. **Le tracer est ce qui rend le cache
  négatif observable pour la première fois** — F4 n'a pu en mesurer que le
  différentiel, **nul**, parce qu'aucun sondage de l'Explorateur n'atteignait le
  fournisseur.
  🔵 **C'est donc une mesure gratuite qui répond à une question que F4 a laissée
  ouverte** : *le cache négatif contient-il quoi que ce soit ?*
- **`PrjDeleteFile`** : ⛔ **aucun appelant, et c'est délibéré.** Aucune politique
  d'éviction en v1 (spec §10 R4), et **F5 n'en invente pas une sans mesure** —
  « poser une politique d'éviction sans mesure serait exactement le geste que ce
  dépôt reproche à ses constantes non calibrées » (spec §6.4). **Quatre entrées
  sur cinq restent sans jumeau.**

### D10 — Le mur de listage : le correctif de MOINDRE COÛT, et rien de plus

F4, legs n°2 : *« rien ne borne la taille de l'en-tête d'aucun côté, et le
`.catch()` de `canal.ts:134` ne répond rien — au minimum, un `Echec` renvoyé au
pont remplacerait vingt secondes de gel par une erreur immédiate. »*

**Relevé, `client/src/fichiers/canal.ts:133-139`** : le `.catch()` journalise en
`console.warn` et **ne renvoie rien**. Le `send()` refusé par SCTP y tombe.

**F5 livre** : le `.catch()` — et le chemin `readyState === 'open'` qui échoue —
renvoie au pont un `TYPE_ECHEC { code: Interne }` sur la **corrélation de la
requête**, avant de journaliser. **Vingt secondes de gel deviennent une erreur
immédiate**, et l'application reçoit un `HRESULT` que la table de la spec §5
porte déjà.

⚠️ **Ce que cela ne fait PAS, et il faut l'écrire dans le code** : **le mur ne
bouge pas.** Un listage de plus de ~3 150 entrées **échoue toujours** ; il échoue
seulement **vite et en le disant**. Le découpage d'une énumération en plusieurs
trames reste un incrément de `FICHIERS_VERSION`, **sans destinataire**.

⚠️ **Et la corrélation doit être capturée AVANT l'`await frein.avantEnvoi()`** :
c'est la trame entrante qui la porte, et le `catch` doit la connaître même si
l'attente a duré.

**La rouge** : retirer le renvoi d'`Echec`. Attendu — le listage au-delà du mur
regèle **20 s** (`DELAI_LISTER`), et la trace `commande expirée` réapparaît.
⚠️ **Cette rouge exige de franchir le mur**, donc un répertoire de plus de
3 200 entrées : **elle est un préalable de la tâche, pas un ornement**.

### D11 — `MORCEAUX_EN_VOL` : ÉPROUVÉ, avec une règle d'admission écrite d'avance

F4, legs n°3 : `MORCEAUX_EN_VOL = 4` rend le produit **pire** à ce débit, **par
arithmétique**, **non éprouvé par mutation**, mutation **écrite et prête**.

**F5 la joue, hors critère, et la règle d'admission est écrite AVANT :**

> La constante passe de **4 à 1** **si et seulement si** une lecture de 256 Kio
> **aboutit à `MORCEAUX_EN_VOL = 1`** et **échoue à 4**, **deux exécutions par
> bras**, sur des binaires qui **s'identifient eux-mêmes par un `tracing::warn!`**
> (§1.5 n°3). **Dans tout autre cas, la constante ne bouge pas**, et le legs
> ressort tel quel.

⚠️ **Ce n'est PAS un correctif du débit.** F4 l'a mesuré : à `ATTENTE_MAX = 1 ms`
le plafond n'est que ~64 Kio/s, *« le facteur est 2, pas 20 »*, et la parade
réelle est un changement de conception de la boucle. **Un morceau en vol au lieu
de quatre rend un rang lisible ; il ne rend pas le pont rapide.**

### D12 — Le redémarrage de la VM : un préalable EXTERNE, et une PORTE (P2)

Le critère « le journal survit à un redémarrage complet de la VM » se heurte à
une question que **personne n'a mesurée** : **la racine ProjFS survit-elle à un
redémarrage ?** Le marquage de `PrjMarkDirectoryAsPlaceholder` est censé être
persistant, et `racine.rs` persiste déjà le GUID d'instance hors de la racine
pour cette raison — **mais aucun sous-bloc n'a redémarré la VM**.

⚠️ **F2 s'en est approché par accident et c'est tout ce qu'on a** : son
`reprise-1` a traversé une **hibernation complète** de la VM, et *« le journal a
survécu à l'extinction de la machine, octet pour octet »* (196 octets, contenu
identique avant et après). **Une hibernation n'est pas un redémarrage** : elle
restaure la mémoire, donc l'état du pilote. **La porte P2 joue le vrai
redémarrage.**

**Issues, et leur conséquence :**

| Issue de P2 | Ce que F5 fait |
| --- | --- |
| la racine survit, marquée, et le pont la reprend | le critère ③ se joue tel qu'il est écrit |
| la racine survit mais le marquage est perdu | `projfs/racine.rs` la remarque ; **le critère se joue quand même**, et le cas 3 du §6.4 (« la racine doit être recréée ») devient **exercé pour la première fois** |
| la racine ne survit pas | 🔴 **c'est un défaut de produit, et il est plus lourd que F5** — il est rapporté, le critère ③ est rapporté **NON TENU avec sa cause**, et le journal reste testé isolément |

⚠️ **La VM est partagée avec G5.** Un redémarrage **doit** être annoncé et joué
quand personne d'autre ne la tient (§1.2 n°1-2).

---

## 4. Les tâches

**Vingt tâches.** Chacune finit par le balayage de tailles (§1.4) et un
`git show --name-only`.

### Task 1 — contrôle d'entrée, AUCUN code

Relever, **par la commande, et publier ce qui a été lancé** :

```bash
unset -f chpwd
git log --oneline -5 && git rev-parse --short HEAD
git status --porcelain
stat -c '%F' /dev/null
virsh list --all
cargo test -p agent 2>&1 | grep 'test result'
cargo test -p proto 2>&1 | grep 'test result'
cd client && npx vitest run 2>&1 | tail -4
cd client && npx vitest run --dir ../proto 2>&1 | tail -4
cd agent && cargo check --target x86_64-pc-windows-gnu 2>&1 | tail -2
env -u TURN_URL -u TURN_SECRET ./scripts/verify-all.sh 2>&1 | tail -5
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>460 && $2!="total"'
```

**Puis, sur le monde d'entrée, les TROIS faits du §0, vérifiés et non
recopiés :**

```bash
# (1) le Rafraichir serait jeté : la seule variante qui porte des octets
grep -n -A8 'pub enum DuNavigateur' agent/src/pont/transport.rs
grep -n 'réponse tardive ou inconnue' agent/src/pont/service.rs
# (2) la reprise est au démarrage du fil, pas à l'ouverture du canal
grep -n 'fil.reprendre()' agent/src/pont/ecriture/fil.rs
grep -n -A10 'pub enum Ordre' agent/src/pont/ecriture/fil.rs
# (3) les deux entrées ProjFS chargées et SANS appelant
grep -rn 'vider_cache_negatif\|supprimer_fichier' agent/src/ | grep -v tests
```

⚠️ **Si l'un des trois ne se vérifie pas, le plan est périmé sur ce point et il
faut le dire avant d'écrire.**

### Task 2 — `scripts/run-agent.sh` transmet `PONT_CACHE` — tâche DÉDIÉE

**Une ligne**, sur le modèle de `PONT_ECRITURE` (`run-agent.sh:34`).
**Le contrôle qui compte est le `run-agent.ps1` GÉNÉRÉ**, relu sur la VM :

```bash
PONT_CACHE=0 scripts/run-agent.sh …
node scripts/winrm.js 'Get-Content C:\dev\run-agent.ps1 | Select-String PONT_CACHE'
```

Attendu : `$env:PONT_CACHE = '0'`. **Le témoin négatif** : sans la variable, la
ligne est **absente** — et c'est ce qui rend la présence lisible.

### Task 3 — 🔴 PORTE P3 : un fichier créé DANS la VM apparaît-il sans nous rappeler ?

**Éliminatoire pour la portée de l'invalidation (D3, §0.5), et jouée AVANT tout
code de cache.** Sur le binaire **actuel** :

1. lister un répertoire de la racine (`Get-ChildItem`), relever `lister=n:?` ;
2. créer un fichier **dans la VM**, sous la racine (`Set-Content`) ;
3. relister **immédiatement**, relever `lister=n:?` et la présence du fichier.

**Deux exécutions.** Deux issues, toutes deux exploitables :

- `lister` **augmente** → le filtre nous rappelle → **l'invalidation par
  notification est indispensable** ;
- `lister` **n'augmente pas** et le fichier est **présent** → le filtre fusionne
  lui-même les entrées locales → **l'invalidation par notification est
  inoffensive mais inutile**, et il faut le dire dans le code plutôt que de
  laisser croire qu'elle sert.

⚠️ **Le témoin qui rend le relevé lisible** : un fichier créé **côté navigateur**
(hors ProjFS) doit, lui, **ne pas** apparaître sans un aller-retour. Sans ce
témoin, un « le fichier est là » ne distinguerait pas les deux mécanismes.

### Task 4 — 🔴 PORTE P1 : quelle grandeur d'occupation disque est lisible sans traverser ?

Les trois candidates de D8, **deux exécutions**. **`NON MESURABLE` est une issue
prévue.** Le journal porte, pour chaque candidate, le compteur de traversées du
pont **avant et après**, et **le témoin qui prouve que le compteur sait
compter** (un `metadata` sur un chemin jamais touché doit le faire bouger).

### Task 5 — 🔴 PORTE P2 : la racine ProjFS survit-elle à un redémarrage de la VM ?

**Une exécution suffit à trancher, deux sont jouées.** Marquer, redémarrer,
relancer le pont, relever : la racine existe-t-elle ? est-elle encore marquée ?
`PrjStartVirtualizing` réussit-il ? le GUID d'instance persisté est-il relu ?
**Le journal des dues est relevé octet pour octet avant et après** (comme F2 l'a
fait par accident).

⚠️ **Distinguer un redémarrage VOULU d'une extinction SUBIE** : relever
`/var/log/libvirt/qemu/Windows.log` et chercher `terminating on signal 15 from
pid <libvirtd>` (§1.2 n°5). *Une extinction subie au milieu de la porte rendrait
un relevé qui ressemble au sien.*

### Task 6 — extraction préalable A : `fil/contrat.rs`

**AVANT toute addition.** `enum Ordre` et `struct Config` partent **verbatim** de
`agent/src/pont/ecriture/fil.rs` (l. 97-127) vers
`agent/src/pont/ecriture/fil/contrat.rs`, avec `pub use contrat::{Config, Ordre};`
dans `fil.rs`. **Aucun site d'appel ne bouge**, et `cargo test -p agent` doit
rendre **exactement le même compte qu'à la tâche 1**.
**Attendu, à mesurer** : `fil.rs` ~437.

### Task 7 — extraction préalable B : `protocole.annonces.test.ts`

**AVANT toute addition.** Les tests de `TYPE_DUES` partent de
`client/src/fichiers/protocole.test.ts` (**484**) vers
`client/src/fichiers/protocole.annonces.test.ts`. Le compte total de
`cd client && npx vitest run` doit être **inchangé**.

### Task 8 — `proto` : les deux annonces, leurs en-têtes, leurs vecteurs

`proto/src/fichiers.rs` (les deux constantes et le paragraphe de la **quatrième
famille**), `proto/src/fichiers/entetes.rs` (`Bonjour`, `Rafraichir`,
`Dues.retenues`), `proto/fichiers-vectors.json`, `proto/ts/fichiers.ts`,
`proto/ts/fichiers-entetes.ts`.

🔴 **Les vecteurs sont lus par les DEUX implémentations** — c'est la propriété
que F1 a posée : *un renommage n'a qu'un seul côté à casser pour être vu rouge*.
**Rouge à jouer** : renommer un champ côté Rust seul ; `cargo test -p proto`
**et** `npx vitest run --dir ../proto` doivent rougir **tous les deux**.

⚠️ **Le vecteur de `Dues` porte les DEUX formes**, avec et sans `retenues` (D7).

### Task 9 — 🔴 l'aiguillage AVANT `resoudre`, et sa rouge

`agent/src/pont/service.rs` (D2). **Rouge** : déplacer le bloc **après**
`resoudre`. Attendu — la trame est jetée, et **le seul témoin est un `debug!`**,
donc **rien** sous `RUST_LOG=info`.

⚠️ **Cette rouge se joue sur l'HÔTE si un test de `service` le permet, et sur la
VM sinon.** *Si elle ne peut être jouée que sur la VM, elle attend la tâche 14 et
c'est dit — pas jouée à moitié.*

### Task 10 — `agent/src/pont/cache.rs` — le module PUR, ses tests, sa rouge

D3, D4. **Horloge injectée.** Tests d'hôte : pose/lecture, expiration **sans
dormir**, `invalider` sur le **parent**, `vider`, et le cas d'un chemin absent.

🔴 **Rouge obligatoire** : faire prendre à `invalider` le chemin **lui-même** au
lieu de son parent. **Un test doit tomber.** Sans elle, l'erreur la plus probable
de ce module est silencieuse.

### Task 11 — la consultation du cache dans `suite_enumeration`

`agent/src/pont/projfs/rappels/listage.rs` (§0.4), `agent/src/pont.rs` (le
câblage et `PONT_CACHE`), `agent/src/pont/service/reponses.rs` (la pose au retour
d'`Entrees`), et l'invalidation par mutation aux deux moitiés de D3(3) — sous
réserve de ce que la porte P3 aura tranché.

🔴 **Rouge du §0.5** : cache armé, **invalidation retirée**. Les trois cas
(créer / renommer / supprimer depuis la VM, puis lister) doivent **tomber**.
*C'est la seule addition de F5 qui puisse rendre faux un comportement déjà
recetté par F2 et F3.*

### Task 12 — `Rafraichir` : le pont vide, et il DIT combien

Le bras `TYPE_RAFRAICHIR` : `cache.vider()`, puis `vider_cache_negatif` (D9), et
une trace `info!` portant **le nombre d'entrées purgées** que la fonction rend.

**Le témoin négatif** : sans `Rafraichir`, la trace est **absente**. **Le témoin
positif** : elle sort, avec un nombre — *et si ce nombre est toujours zéro, c'est
un fait, pas une panne, et il faut le rapporter comme F4 a rapporté son
différentiel nul.*

### Task 13 — `Bonjour` : la reprise devient conditionnelle

`agent/src/pont/bonjour.rs` (**PUR**, les quatre lignes de la table de D6, plus
la persistance du nom), `agent/src/pont/ecriture/fil.rs` et
`fil/reprise.rs`, `agent/src/pont/service.rs`, `agent/src/pont.rs`.

🔴 **Rouge n°1** : retirer la condition — le pont pousse quel que soit le nom.
Attendu : le test de la règle tombe, **et sur la VM les dues d'un répertoire
partent dans un autre**.
🔴 **Rouge n°2** : remettre `fil.reprendre()` dans `Fil::demarrer`. Attendu : la
fenêtre de trente secondes de F2 **réapparaît**, mesurable à la tâche 16.

### Task 14 — le côté navigateur : les deux boutons, l'envoi, l'état retenu

`client/src/fichiers/protocole.ts` (l'émission des deux annonces),
`client/src/shell-page.ts` (l'envoi de `Bonjour` **après** que l'écrivain, le
mutateur et l'adaptateur sont posés — §0.9), `client/src/shell.ts` (l'état
`retenues` et son libellé), `client/shell.html` (`#rafraichir`,
`#reprendre-enregistrement`).

⚠️ **`#reprendre-enregistrement` n'apparaît QUE si `retenues` est vrai**, et
disparaît sinon — un bouton toujours présent qui ne fait rien la plupart du temps
serait un piège à clic. **Test d'hôte.**

⚠️ **Les données d'instrument passent par `dataset`, jamais par le texte** — le
pilote lit `data-retenues`, comme il lit déjà `data-dues` et `data-vues`
(`shell-page.ts:135`).

### Task 15 — D10 : le `send()` refusé rend un `Echec`

`client/src/fichiers/canal.ts` (D10), avec la corrélation **capturée avant
l'`await`**. Rouge : retirer le renvoi ; le gel de 20 s revient.

### Task 16 — RECETTE, critères ① à ④, DEUX exécutions par bras

Voir §5. **La VM est un préalable externe** (§1.2).

### Task 17 — MESURE : la latence de listage, cache armé, aux rangs de F4

**C'est la tâche que le §0.1 impose au verdict VERT.** Rangs **10, 100,
1 000** — pas 3 150, *le mur ne bouge pas.* **Deux exécutions.** Et la prédiction
du §0.2 est **inscrite dans le journal avant la mesure**, avec sa falsification.

### Task 18 — MESURE hors critère : D11, `MORCEAUX_EN_VOL`

La règle d'admission de D11 est **recopiée en tête du journal avant la mesure**.
Binaires **auto-identifiés par un `tracing::warn!` d'au moins vingt caractères**.

### Task 19 — MESURE : l'occupation disque, selon ce que P1 aura tranché

Trace périodique, **à la période existante de `tracer_hydratation`**
(`PERIODE_HYDRATATION`, 60 s) — **aucune constante non calibrée de plus**.

### Task 20 — revue transverse, document de résultats, `CLAUDE.md`

Trois gestes distincts :

① **La revue transverse.** Sa cible propre : **les affirmations devenues fausses
dans la branche elle-même**. Barème du dépôt : 5 en D7, 3 en D8, 6 en D9, 12 en
D10, 7 en D11, 8 en P1, 10 en P2, 5 en S1, 9 sur E, 12 en P3, 12 en S2, 11 en F1,
8 en P4, 13 en S3, 8 en G1, 7 en F2, 4 en F3, 9 en F4.
**Les places sont énumérées par `grep -n` AVANT l'édition et relues APRÈS**, une
par une, **par numéro de ligne** — et ⚠️ *relire une citation avant d'écrire ne
suffit pas quand le chantier DÉPLACE la ligne citée*. **Les candidates connues
d'avance sont au §7.**

② **Le document de résultats**, permanent, sous
`docs/superpowers/plans/2026-08-21-pont-fichiers-f5-resultats.md`, et les pièces
sous `journaux-pont-fichiers-f5/`. 🔴 **La preuve d'une affirmation du dépôt ne
doit JAMAIS vivre dans un rapport gitignoré** : D9 a perdu six constats ainsi.

③ **`CLAUDE.md`** : la section F5, **et la clôture du sous-projet ③** — c'est le
dernier sous-bloc, donc la liste « ce que ③ laisse ouvert » doit être
**complète**, comme S4 l'a faite pour ⑥ et P5 pour ⑤.

⚠️ **Les tailles se relèvent APRÈS la dernière édition de la ronde, revue
transverse comprise.** Une table mesurée en début de ronde est fausse à la fin de
la même ronde — erreur commise par D8 en croyant bien faire.

---

## 5. Les critères de recette

**Deux exécutions par bras. Aucun taux n'est revendiqué.**

| # | Critère | Bras | Ce qui le rend ROUGE |
| --- | --- | --- | --- |
| ① | **Un fichier ajouté sur le poste local apparaît après `Rafraichir` et PAS avant** | vert (cache armé) / rouge (`PONT_CACHE=0`) | il apparaît **sans** `Rafraichir` |
| ② | **Le journal survit à un redémarrage COMPLET de la VM et se vide à la reconnexion** | vert / rouge (journal supprimé avant le redémarrage) | le journal est perdu, ou les dues ne repartent pas |
| ③ | **Le retour avec un répertoire DIFFÉRENT retient au lieu d'écrire, et le DIT** | vert / rouge (`Bonjour` sans condition, tâche 13 rouge n°1) | les dues partent dans le mauvais dossier, ou rien ne le dit |
| ④ | **Un `Rafraichir` ne casse rien** : les trois mutations de F2/F3 restent visibles après | vert / rouge (invalidation retirée, tâche 11) | une création, un renommage ou une suppression n'est plus vue |

**Le protocole du critère ①, dans l'ordre, et il n'est pas indifférent :**

1. monter le lecteur, envoyer `Bonjour` ;
2. **lister** le répertoire cible — c'est l'**amorçage** du cache, sans quoi il
   n'y a rien à servir et le critère est vacueux ;
3. **ajouter un fichier côté navigateur**, hors du pont ;
4. **relister** — le fichier doit être **ABSENT** ;
5. **`Rafraichir`** ;
6. **relister** — le fichier doit être **PRÉSENT**.

🔴 **RÈGLE D'ADMISSION (D4)** : l'instrument enregistre l'écart entre (2) et (4).
**S'il dépasse `TTL_ENUMERATION`, l'exécution est DISQUALIFIÉE**, parce qu'un
cache expiré rendrait un **faux rouge**.

🔴 **L'étape (4) est le seul point où le critère peut réussir pour une mauvaise
raison** : un listage qui **échoue** rend, lui aussi, « le fichier n'est pas là ».
**L'instrument vérifie que le listage a RÉUSSI et qu'il porte les entrées
d'amorçage**, pas seulement que le fichier neuf manque.

⚠️ **Le critère ② à travers un vrai redémarrage** : les dues sont inscrites
**avant** l'arrêt, le journal est relevé **octet pour octet** avant et après, et
la reconnexion est faite **par un `Bonjour` au même nom de racine** — sans quoi
D6 retiendrait, et le critère se lirait rouge pour la raison du critère ③.

---

## 6. Ordre, dépendances, et ce qui peut aller de front

```
T1 (contrôle d'entrée) ─┬─> T2 (run-agent.sh, DÉDIÉE)
                        ├─> T3 P3 ┐        [VM]
                        ├─> T4 P1 ┤ portes  [VM]
                        └─> T5 P2 ┘        [VM, redémarrage]

T6 (extraction A) ──┐
T7 (extraction B) ──┤  de front, aucun lien
T8 (proto) ─────────┘

T8 ──> T9 (aiguillage)  ──┐
T3 ──> T10 (cache pur) ───┼──> T11 (consultation + invalidation)
                          │
T6, T8 ──> T13 (Bonjour) ─┤
T8 ──> T12 (Rafraichir) ──┤
T7, T8 ──> T14 (client) ──┘

T11, T12, T13, T14, T15 ──> T16 (RECETTE)  [VM]
T16 ──> T17 (latence, si ① VERT)           [VM]
T4  ──> T19 (occupation)                   [VM]
T15 ──> T18 (MORCEAUX_EN_VOL, hors critère)[VM]
tout ──> T20 (revue transverse, résultats, CLAUDE.md)
```

**De front, sans risque** : T6, T7, T8 (trois arbres disjoints) ; T10 et T13
(deux modules purs) ; T12 et T14.

🔴 **JAMAIS de front** : deux tâches qui touchent la **VM**. C'est une ressource
exclusive, et le binaire est un état partagé (§1.2).

🔴 **T5 (redémarrage) est la tâche la plus intrusive du sous-bloc** : elle doit
être annoncée, et jouée quand G5 ne tient pas la VM.

---

## 7. Divergences relevées entre la spec, F1–F4 et le CODE RÉEL

| # | Divergence | Tranchée |
| --- | --- | --- |
| **E1** | 🔴 **La clause ROUGE de la spec (l. 1214-1217) tire une conclusion que F4 réfute** : c'est le VERT qui périme la mesure de F4, pas le rouge | §0.1. **La spec est ANNOTÉE, jamais réécrite** — c'est un relevé daté du 19 août 2026 |
| **E2** | La spec §4.2 écrit « **Une seule exception** [au sens unique], et elle est nécessaire : `Rafraichir` » | **Il y en a DEUX** : `Bonjour` est la seconde, et elle n'était pas prévue. La spec est annotée (D1, D6) |
| **E3** | La spec §7.4 dit « **Un bouton dans la page-shell, et rien de plus** » | **Il en faut DEUX** : `Rafraichir`, et « Reprendre l'enregistrement » du §6.4 cas 2. Les deux clauses de la spec sont dans deux § différents et ne se contredisent pas ; **leur somme n'était écrite nulle part** |
| **E4** | La spec §8 F5 dit « la reprise du journal à travers un redémarrage de la VM » comme d'un **livrable** | **C'est un livrable de F2** (`fil/reprise.rs`, existant). Ce que F5 livre est sa **RECETTE**, plus la condition de D6. *Confondre les deux ferait réécrire du code qui marche* |
| **E5** | `agent/src/pont/enumeration.rs:33-37` **pronostique** : « le critère ROUGE de F5 sera par construction rouge tant que F5 n'existe pas » | ✅ **JUSTE, et vérifié à la tâche 1.** Ce commentaire devra être **corrigé par la revue transverse** : il devient faux le jour où T11 est commitée. **Place n°1 de la revue** |
| **E6** | `agent/src/pont/chemins.rs:26` — « ce qui peut le vider, `Rafraichir`, est un livrable de F5 » | Même sort. **Place n°2** |
| **E7** | `agent/src/pont/projfs/chargement.rs:241` — « la seule à la solliciter est `Rafraichir`, un livrable de F5 » | Même sort. **Place n°3** |
| **E8** | `agent/src/pont/projfs/etat.rs:340` — « la mesure de fond appartient à F5, avec la politique d'éviction qu'elle servira » | 🔴 **À MOITIÉ FAUX APRÈS F5** : la mesure arrive, **la politique d'éviction NON** (D9). **Place n°4, et la plus dangereuse** — la corriger d'un mot ferait croire que l'éviction est venue |
| **E9** | La spec §9 budgète `agent/src/pont.rs` « ≤ 200 » | Il vaut **265**, et F5 le porte vers ~295. **Écart déclaré, aucune porte franchie.** *Un budget de spec n'est pas une porte* |
| **E10** | La spec §9 dit `agent/src/superviseur/lanceur.rs` **367** | Il vaut **488**, marge **12**. ⛔ **F5 n'y touche pas**, et ne le corrige pas : c'est la marge d'un voisin |
| **E11** | F4 §16 legs n°7 dit « F5 — `Rafraichir` et le cache d'énumération, **sans lequel la moitié "chaud" de la table de la spec n'a pas d'objet** » | ✅ **Juste**, et c'est la tâche 17 qui lui donne son objet |
| **E12** | La spec §8 F5 ne mentionne **aucune** variable d'environnement | F5 en ajoute **une**, `PONT_CACHE` (D5), **variable de banc**. Divergence déclarée, et elle est ce qui rend le critère ① falsifiable |

---

## 8. Ce que F5 n'établira PAS

- **Aucun taux, nulle part.** Deux exécutions par bras au mieux, une par rouge.
- ⛔ **Aucun jugement d'usage.** Personne n'aura dit si le lecteur est *agréable*,
  ni si 6 s pour mille entrées — ou 3 s avec le cache — est acceptable. **C'est
  la lacune que ce dépôt traîne depuis `BPP_MIN`**, et F5 ne la lève pas.
- ⛔ **Aucune constante calibrée.** F5 en ajoute **deux** non calibrées —
  `TTL_ENUMERATION` (30 s) et `DELAI_BONJOUR` (10 s) — à une liste déjà longue.
- 🔴 **Le MODÈLE DE PERMISSION n'est pas éprouvé** (§0.8) :
  `showDirectoryPicker()` n'est toujours jamais appelé, ni `queryPermission`, ni
  `requestPermission`, ni le mode `readwrite`. **F5 mesure que la règle du nom
  fonctionne ; il ne mesure pas qu'elle suffit.**
- 🔴 **Le mur de ~3 150 entrées n'est pas déplacé**, et un cache ne le déplacerait
  pas. F5 le rend seulement **immédiat au lieu de gelé** (D10).
- 🔴 **Le débit du canal reste à ~33 Kio/s**, et rien au-delà de 128 Kio ne se lit
  — sauf ce que D11 pourrait rendre lisible, sur **un rang**.
- ⛔ **Le mécanisme des DEUX `Lister` par `Get-ChildItem` ne sera pas expliqué**,
  même si le cache en absorbe un (§0.2).
- ⛔ **Aucune politique d'éviction** : `PrjDeleteFile` reste sans appelant, et
  **quatre des cinq entrées ProjFS sans jumeau `PRJ_*_CB` le restent** (R7).
- ⛔ **L'idiome « fichier temporaire + renommage » sur un éditeur réel** — legs
  n°3 de F3, que F4 nomme la lacune la plus lourde du sous-projet — **ne sera pas
  exercé**.
- ⛔ **Le condensat SHA-256 de bout en bout** (legs n°6 de F1) reste dû.
- ⛔ **Les lectures qui calent sans expirer** (legs n°4 de F1) : le symptôme n'est
  pas provocable.
- ⛔ **Le coût de la canonicalisation de casse de F3** reste non mesuré.
- ⛔ **L'occupation disque sera mesurée par ce que la porte P1 aura admis**, avec
  son confondeur nommé — **jamais la taille réelle de la racine**, que rien ne
  sait lire sans traverser.
- ⛔ **Rien d'un client réel** : Chrome sans interface, sur l'hôte qui porte la
  VM. **Un seul navigateur.** La File System Access API n'existe ni sur Firefox
  ni sur Safari — limite du **produit**.
- ⛔ **Rien de plusieurs utilisateurs** : une VM, une racine, `SESSION_DU_PONT`
  non namespacé.
- ⛔ **Le chemin d'extinction propre du superviseur** n'a toujours jamais été
  exercé, depuis D1.

---

## 9. Risques — et ce qui rendrait F5 NON LIVRABLE

| # | Risque | Parade, ou ce qu'il coûte |
| --- | --- | --- |
| **R-a** | 🔴 **Le cache casse F2/F3** (§0.5) — une création, un renommage ou une suppression cesse d'être vue | **C'est le risque n°1.** Parade : D3(3), la porte P3, le critère ④ et sa rouge. **Si le critère ④ tombe et qu'on ne sait pas pourquoi, `PONT_CACHE` est posé à `0` par défaut et le cache est livré DÉSARMÉ** — le critère ① devient alors NON TENU, et il faut le dire ainsi plutôt que livrer un cache qui ment |
| **R-b** | 🔴 **`Bonjour` fait qu'un client d'avant F5 ne repousse plus jamais** (D1) | Les deux bouts sont livrés ensemble ; la trace de `DELAI_BONJOUR` le dénonce. **Aucun repli qui pousserait** |
| **R-c** | 🔴 **La racine ProjFS ne survit pas au redémarrage** (porte P2) | Le critère ② est rapporté **NON TENU avec sa cause**, et c'est un défaut plus lourd que F5. **Ce n'est pas un blocage de livraison** : le journal reste testé isolément |
| **R-d** | **L'occupation disque n'est mesurable par aucune des trois candidates** (porte P1) | `NON MESURABLE` est **une issue prévue**. Le livrable devient la candidate (a) seule — le compteur persisté — avec ce qu'elle ne dit pas |
| **R-e** | **La VM est indisponible, ou tenue par G5** | **Le sous-bloc s'arrête proprement** (§1.2). Tout ce qui est pur — T6 à T15 — reste jouable sur l'hôte |
| **R-f** | ⚠️ **Le TTL rend le critère ① fragile** : trop court, faux rouge ; trop long, produit périmé | La règle d'admission de D4 **disqualifie** l'exécution plutôt que de la rapporter |
| **R-g** | ⚠️ **Un voisin écrit dans `agent/` pendant une campagne** — arrivé à F4, **et arrivé pendant l'écriture de ce plan** | `git status --porcelain proto/ agent/` avant **CHAQUE** build, et le contrôle d'auto-identification du binaire, qui a limité les dégâts à F4 |

🔴 **CE QUI RENDRAIT F5 NON LIVRABLE**, et il n'y a qu'un cas :

> **Si le cache ne peut pas être invalidé de façon fiable** — c'est-à-dire si le
> critère ④ tombe sans cause identifiée —, alors le cache **ne doit pas être
> livré armé**. Un cache qui fait disparaître un fichier que l'utilisateur vient
> de créer dans sa VM est **pire que l'absence de cache**, et c'est exactement le
> défaut que la spec §7.4 reproche à l'ancien pont. Dans ce cas, F5 livre
> `Rafraichir`, `Bonjour`, l'occupation disque et la recette du redémarrage,
> **et déclare son critère ① NON TENU.**

⚠️ **Tout le reste est livrable indépendamment.** `Bonjour` ne dépend pas du
cache, l'occupation disque ne dépend de rien, et le correctif D10 est de trois
lignes.

---

## 10. Résumé en une phrase

**F5 pose le cache d'énumération que F4 a mesuré absent, le bouton qui seul peut
le vider, la poignée de main qui empêche des écritures dues d'atterrir dans le
mauvais dossier, et la mesure de ce que la racine coûte au disque — puis il
remesure la latence de listage, parce que le verdict qui périme le chiffre de F4
est le VERT et non le rouge.**

---

## 11. Ce que F5 léguera, quoi qu'il mesure

**F5 est le dernier sous-bloc de ③. Tout ce qui suit sortira du sous-projet SANS
DESTINATAIRE**, et la liste sera reprise, complétée et déclarée close à la
tâche 20.

1. 🔴 **Le canal du pont plafonne à ~33 Kio/s**, la parade est un changement de
   conception de la boucle de `pont::transport` (F4 legs n°1).
2. 🔴 **Aucun listage de plus de ~3 150 entrées n'aboutit** ; F5 en fait une
   erreur immédiate, **il ne le déplace pas** (F4 legs n°2, moitié restante).
3. 🔴 **L'idiome « fichier temporaire + renommage » sur un éditeur réel** (F3
   legs n°3) — *le seul chemin par lequel une sauvegarde peut se perdre en
   silence*.
4. 🔴 **`showDirectoryPicker()`, le modèle de permission, le mode `readwrite`**,
   qui exigent `Xvfb` + `xdotool` — consentement donné **en D8**, jamais suivi
   d'effet.
5. ⛔ **Aucune politique d'éviction**, et le disque de la VM grossit —
   **F5 le mesure et le fait croître**.
6. ⛔ **Quatre des cinq entrées ProjFS sans jumeau `PRJ_*_CB`** (R7).
7. ⛔ **Le mécanisme des deux `Lister` par `Get-ChildItem`** (F4 legs n°4).
8. ⛔ **La cause du delta M2 − M1 inverse** (F4 legs n°5).
9. ⛔ **Le coût de la canonicalisation de casse** (F4 legs n°6).
10. ⛔ **Le condensat SHA-256 de bout en bout** (F1 legs n°6).
11. ⛔ **Les lectures qui calent sans expirer** (F1 legs n°4).
12. ⛔ **La calibration** : `TTL_ENUMERATION`, `DELAI_BONJOUR`, les cinq budgets
    de délai, `TAILLE_TRAME_MAX`, `SEUIL_TAMPON`, `MORCEAUX_EN_VOL`,
    `ATTENTE_MAX`, les treize seaux de `pont::latence`. **Le jugement d'usage
    reste à porter, et personne ne le portera dans ③.**
13. ⛔ **Le réexamen du sélecteur de fichiers Windows**, que F4 alimente, que F5
    n'approche pas, et qui appartient à un autre chantier.
