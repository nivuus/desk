# Sous-projet ③ Pont fichiers — sous-bloc F4 : la mesure que le cadrage RÉCLAME

**Date** : 21 août 2026
**Spécification** : `docs/superpowers/specs/2026-08-19-pont-fichiers-design.md`, §8 « F4 »
(l. 1114-1150), et le §7.4 (les deux caches), le §5.3 (les trois budgets), le
§7.3 (le contrôle de flux), le §10 (R2, R5).
**Cadrage** : `docs/superpowers/specs/2026-07-27-refonte-produit-design.md` §5 ③
et son **amendement du 28/07/2026 sur le sélecteur de fichiers Windows**
(l. 133-166).
**Prédécesseurs** : F0/F1 (`2026-08-19-pont-fichiers-f1.md`), F2
(`2026-08-20-pont-fichiers-f2.md`), F3 (`2026-08-20-pont-fichiers-f3.md`).

---

## Ce que F4 est, et ce qu'il n'est pas

**F4 est le seul livrable de ce sous-projet que le cadrage nomme
explicitement.** L'amendement du 28/07/2026 pose « instrumenter la latence de
listage et de lecture de ProjFS » comme le **critère de réexamen** de la
décision « aucune interception du sélecteur de fichiers en v1 », et exige que
ce réexamen se prenne « **sur mesure, pas sur intuition** ».

**F4 mesure. Il ne décide pas.** Le réexamen appartient à un autre chantier
(spec §8 F4, §11), et la voie par hook d'API est **écartée définitivement** par
l'amendement lui-même — anti-cheat, antivirus, et sélecteurs propriétaires
(Office, Qt, Electron).

⚠️ **« Ne tranche rien » est une issue acceptable et prévue**, exactement comme
pour l'A/B de `set_desired_bitrate` en D9 et D10. Ce qui n'est **pas**
acceptable est un verdict sans son nombre d'exécutions.

🔴 **Et c'est un sous-bloc de MESURE, pas de fonctionnalité. La leçon constante
de ce dépôt sur ce terrain est que le coût est dans la discipline de l'ÉNONCÉ,
pas dans le code** : les onze rondes de correction de la sonde multi-fenêtres
ont « quasiment toutes porté sur des **rapports qui affirmaient au-delà de leur
relevé**, jamais sur des bugs ». Le §1.5 de ce plan pose la discipline qui en
découle, et elle s'applique à **chaque** tâche.

---

## 0. Ce que F4 doit trancher AVANT d'écrire une ligne de code

Sept points. Les trois premiers **retirent ou remplacent une ligne de la table
du §8 F4 de la spec**, sur mesure. Les quatre suivants fixent la méthode.

### 0.1 🔴 LE MUR DE 256 Kio — le rang « 10 000 entrées » de la spec n'est PAS atteignable, et le symptôme n'est pas une lenteur

La table de la spec demande une latence d'énumération aux rangs **10, 100,
1 000, 10 000 entrées**. **Trois faits mesurés le 21 août 2026 établissent que
le dernier rang ne peut pas aboutir avec le protocole livré**, et que le mur
tombe bien en dessous.

**Fait 1 — le poids d'une entrée sur le fil, mesuré en exécutant la forme réelle
d'`encodeEntrees` (`proto/ts/fichiers-entetes.ts:162-173`)** :

| Entrées | En-tête produit | Par entrée |
| --- | --- | --- |
| 10 | 863 o | 86,3 o |
| 100 | 8 513 o | 85,1 o |
| 1 000 | **85 013 o (83,0 Kio)** | 85,0 o |
| 10 000 | **850 013 o (830,1 Kio)** | 85,0 o |

*(noms de 18 caractères, `taille` à quatre chiffres, `modifie` à treize. Un jeu
de noms plus longs déplace le mur vers le bas, jamais vers le haut.)*

**Fait 2 — la liste entière part dans UN SEUL message, dans l'EN-TÊTE, et rien
ne borne cet en-tête.** `client/src/fichiers/protocole.ts:333-335` fait
`encoderTexte(TYPE_ENTREES, correlation, encodeEntrees(entrees))` ;
`proto/ts/fichiers.ts:158-175` écrit la longueur d'en-tête sur **4 octets** et
n'oppose **aucun contrôle de taille** ; et côté agent, `proto/src/fichiers.rs:236`
ne vérifie que `charge.len() > TAILLE_TRAME_MAX` — **la charge, pas l'en-tête**.
Le commentaire de `TAILLE_TRAME_MAX` le dit déjà en toutes lettres :
« le nom dit "trame", la valeur borne la CHARGE ».

**Fait 3 — str0m 0.21.0 annonce et impose 256 Kio.**
`str0m-0.21.0/src/sctp/mod.rs:33` : `pub const LOCAL_MAX_MESSAGE_SIZE: u32 = 256 * 1024;`
`src/change/sdp.rs:1456` la pose dans le SDP (`a=max-message-size:262144`) et
`src/sctp/mod.rs:355` en fait le `with_max_receive_message_size`.

**Le mur analytique tombe donc à ≈ 262 144 / 85 ≈ 3 080 entrées**, avec ces
noms-là.

🔴 **ET LE SYMPTÔME N'EST PAS UNE LENTEUR : c'est un GEL DE VINGT SECONDES SUIVI
D'UNE ERREUR D'E/S.** `client/src/fichiers/canal.ts:132` fait
`canal.send(reponse)` dans un `.then()` dont le `.catch()` (`:134`) se contente
d'un `console.warn('trame fichiers non traitée', e)` — **il ne répond rien**.
Un `send()` refusé par le navigateur ne produit donc **aucune** trame `Echec` :
la commande `Lister` reste en vol jusqu'à `DELAI_LISTER` (**20 s**,
`agent/src/pont/table.rs:42`), puis est complétée en `ERROR_SEM_TIMEOUT`.

⚠️ **CECI EST UNE PRÉDICTION TIRÉE DE TROIS FAITS MESURÉS, PAS UNE MESURE.**
Ce que je n'ai pas établi : que Chrome refuse effectivement ce `send()` (il
pourrait fragmenter, ou tronquer, ou lever un autre type d'erreur), ni où tombe
le mur réel. **C'est l'objet de la porte P0 (tâche 3), et elle est ÉLIMINATOIRE
pour le choix des rangs.**

**Décision D1 — les rangs d'énumération ne sont pas ceux de la spec : ils sont
DÉRIVÉS du mur mesuré.** La porte P0 trouve `N_max` par dichotomie, puis les
rangs sont **10, 100, 1 000, et les deux qui encadrent `N_max`** (le dernier qui
passe, le premier qui échoue). Le rang qui échoue **est une mesure**, et son
chiffre-juge est la durée jusqu'à l'erreur rendue à l'application, pas une
latence.

⚠️ **Ne PAS « corriger » le protocole dans F4.** Découper une énumération en
plusieurs trames est un changement de forme du protocole, donc un incrément de
`FICHIERS_VERSION` (le commentaire de `proto/src/fichiers.rs:37` le dit : « ce
qui l'incrémentera est un changement de FORME »). **F4 mesure et lègue** ; il ne
livre pas la parade, qui appartient au sous-bloc qui la décidera.

### 0.2 ⛔ `TTL_ENUMERATION` N'EXISTE PAS — la ligne « cache d'énumération chaud » de la spec n'est pas mesurable telle qu'elle est écrite

La table de la spec demande la latence de `GetPlaceholderInfo` « à froid, **et
cache d'énumération chaud** ».

**Relevé par la commande le 21 août 2026**
(`grep -rn 'TTL_ENUMERATION' agent/ proto/ client/src/`) : **une seule
occurrence dans tout le dépôt**, et c'est un commentaire qui dit que la chose
n'existe pas — `agent/src/pont/enumeration.rs:26` :

> « Ce n'est pas le cache d'énumération (`TTL_ENUMERATION`) de la spec §7.4,
> **qui n'est PAS livré en F1** : celui-là survivrait à la session, serait
> indexé par CHEMIN, et ne pourrait être vidé que par `Rafraichir` — un
> livrable de **F5**. »

C'est confirmé par le legs 4 de F3 et par le legs 10 de F2 (« `Rafraichir`,
sans lequel aucun cache d'énumération n'est possible »).

**Décision D2 — on mesure le CHAUD QUE LE PRODUIT A, et on le nomme
précisément.** Ce qui existe et se réchauffe est autre chose, et c'est
observable :

| Ce qui est chaud | Ce qui le rend chaud | Mesurable ? |
| --- | --- | --- |
| le **cache négatif** de ProjFS | `PRJ_FLAG_USE_NEGATIVE_PATH_CACHE`, posé (`agent/src/pont/projfs.rs:214`) | **oui**, par différentiel (§0.3) |
| les **substituts déjà écrits** sur le disque de la VM | `PrjWritePlaceholderInfo` d'une énumération antérieure | **oui** : une seconde énumération du même répertoire ne redemande pas les métadonnées d'une entrée déjà posée |
| le **cache d'énumération** (`TTL_ENUMERATION`) | rien — **il n'existe pas** | **non** |

**Ce que cela fait aux chiffres de F4, et il faut l'écrire d'avance** : F4
mesure un produit **sans** cache d'énumération, donc **une borne HAUTE du coût**
et une borne BASSE de la performance. L'asymétrie qui en découle est à porter
telle quelle dans le document de résultats :

- si le listage **ne dégrade pas** l'expérience sans le cache, il ne la
  dégradera pas davantage avec — la conclusion est **robuste** ;
- s'il la **dégrade**, F4 **ne peut pas dire** si le cache de F5 y remédierait.
  La conclusion est alors **conditionnelle**, et le dire n'est pas une réserve
  de style : c'est la différence entre « le sélecteur doit être réexaminé » et
  « il faut d'abord finir F5 ».

### 0.3 ⛔ LE « TAUX D'OCCUPATION DU CACHE NÉGATIF » N'EST PAS OBSERVABLE DEPUIS LE PONT — ce qui le remplace, et son témoin

La table de la spec demande le « taux d'occupation du cache négatif » à
l'ouverture d'un dossier dans l'Explorateur.

**Un taux est un rapport, et son numérateur est invisible.** Un *succès* du
cache négatif est, par définition, une requête qui **n'atteint jamais le
fournisseur** : ProjFS y répond seul. Le pont ne peut compter que les
**échecs** — les sondages de chemins absents qui lui parviennent, comptés par
`Erreur::Introuvable` et `Erreur::CheminIntrouvable` au recensement
(`agent/src/pont/compteurs.rs`). Il n'a **aucun** accès au dénominateur.

**Décision D3 — on mesure le DIFFÉRENTIEL, qui est observable, et on lui donne
un témoin qui retire la fonctionnalité suspecte.**

| Grandeur | Comment | Ce qu'elle dit |
| --- | --- | --- |
| `introuvable` + `chemin-introuvable` à la **1ʳᵉ** ouverture du dossier | recensement, avant/après | le coût des sondages de Windows, cache froid |
| les mêmes à la **2ᵉ** ouverture | idem | ce que le cache a absorbé |
| **le témoin** : les mêmes, `PRJ_FLAG_USE_NEGATIVE_PATH_CACHE` **retiré** | une mutation d'une ligne, `projfs.rs:214` | ce que coûterait l'absence de cache |

🔵 **Le témoin est la pièce qui donne son sens au différentiel**, et c'est
littéralement la méthode que ce dépôt a payé dix minutes pour apprendre au
chantier TURN : *une mesure témoin sur le chemin sans la fonctionnalité
suspecte coûte peu et évite une conclusion fausse*. Sans lui, un différentiel
nul serait indiscernable de « Windows ne sonde rien » et de « le cache ne sert
à rien ».

⚠️ **Et le témoin est le ROUGE de cette mesure** : si le retrait du drapeau ne
change **aucun** compte, ou bien Windows ne sonde pas de chemins absents sur ce
montage, ou bien le drapeau n'est pas appliqué. **Les deux se déclarent ; on
n'en choisit pas un.**

### 0.4 🔴 LE REPLI DE RENOMMAGE PAR COPIE N'A JAMAIS COURU — et sa moitié RÉPERTOIRE est inatteignable depuis la VM

La table de la spec demande le « coût du repli de renommage par copie (§3.5.1) »
à 1 Mio et 100 Mio. **Deux faits de F3 le contraignent.**

**Fait 1 — le repli existe et aucune de ses lignes n'a jamais couru.** F3 le
déclare (`client/src/fichiers/copie.ts`, livré), et sa trace
`renommage par copie « … » → « … »` (`client/src/shell-page.ts:232`) **est
absente de tous les journaux versés** — vérifié par F3 au `grep -a` sur ses sept
`-plat.log`. La raison est que `move()` **existe** sur un fichier OPFS (sonde S2
de F3, 2 exécutions), et `client/src/fichiers/mutation.ts:291` la détecte **à
l'appel** : `if (typeof poignee.move === 'function')`. Le repli n'est donc
jamais pris.

**Fait 2 — ProjFS REFUSE le renommage d'un RÉPERTOIRE avant de consulter le
fournisseur.** Mesuré par F3, 2 exécutions plus sa sonde S1 :
`Rename-Item` d'un répertoire rend « Cette demande n'est pas prise en charge »,
et le journal d'agent ne porte **aucune** notification `PRE_RENAME` pour ce
geste. C'est un fait de plateforme, et il rend le critère ① b de F3 non
livrable.

**Décision D4 — la ligne se scinde en deux moitiés qui n'ont pas le même
statut, et les deux sont mesurées en le disant.**

| Moitié | Chemin | Statut de la mesure |
| --- | --- | --- |
| **fichier** | réel, de bout en bout depuis la VM | ⚠️ **mesure FORCÉE** : le repli n'est pris que si `move` est neutralisée dans la couche d'injection (`delete FileSystemFileHandle.prototype.move`). C'est légitime — l'injection est l'instrument, pas le produit — **et ce n'est pas le chemin nominal** |
| **répertoire** | **hors du produit** : une sonde côté navigateur seule, sur le modèle de `s2-move-casse.mjs` de F3 | ⛔ **le produit ne peut pas l'atteindre** : ProjFS ne demande jamais. C'est une mesure de **composant**, pas de produit, et elle se rapporte comme telle |

🔵 **Et le coût est presque entièrement CÔTÉ NAVIGATEUR, ce qui change ce qu'on
mesure.** `copie.ts` recopie **localement** — la trace du produit le dit :
« repli LOCAL (**zéro octet sur le canal**) ». Le pont ne voit qu'**un seul**
aller-retour, dont la durée **est** celle de la copie, et il est borné par
`DELAI_MUTATION` (**15 s**, `table.rs:82`). **Le chiffre-juge de cette ligne est
donc la durée de ce seul aller-retour**, et le rang 100 Mio y est d'abord une
question de savoir si 15 s suffisent.

### 0.5 🔵 DEUX INSTRUMENTS, PAS UN : le chronomètre externe est l'ARBITRE, le pont est la DÉCOMPOSITION, et leur différence est le résidu

La question du cadrage est « **le listage d'un dossier volumineux dégrade-t-il
réellement l'expérience ?** ». L'expérience est ce qu'un utilisateur **attend**,
et rien dans le pont ne le sait : les rappels ProjFS entrent sur des fils que le
système possède, et le pont ne voit que ce qui atteint sa table.

**Décision D5 — deux instruments, avec des rôles nommés et non
interchangeables :**

| Instrument | Ce qu'il mesure | Rôle |
| --- | --- | --- |
| **le chronomètre de la VM** (`System.Diagnostics.Stopwatch` en PowerShell, session 1) | le mur-à-mur d'un geste réel : `Get-ChildItem`, un `GetAttributes` par entrée, une lecture complète | 🔴 **l'ARBITRE.** C'est lui qui répond à la question du cadrage |
| **l'histogramme du pont** (`agent/src/pont/latence.rs`, neuf, PUR) | la traversée pont → navigateur → pont, par famille de commande | **la DÉCOMPOSITION.** Il répond à « où le temps passe » |

🔵 **Et leur DIFFÉRENCE est le troisième chiffre, obtenu sans troisième
instrument** : `mur-à-mur − Σ(traversées)` = tout ce qui n'est pas l'aller-retour
navigateur — l'entrée dans le rappel, l'inscription en table, le balayage à
`PERIODE_BALAYAGE` (**250 ms**, `service.rs:47`), `PrjCompleteCommand`, et le
retour de ProjFS à l'application.

⚠️ **Ce résidu est NOMMÉ, pas MESURÉ.** Il est une soustraction entre deux
grandeurs prises par deux horloges différentes sur deux machines différentes ;
il en porte les deux incertitudes. **Il se rapporte comme un ordre de grandeur
et jamais comme une latence.** Le dire d'avance évite qu'un lecteur le prenne
pour ce qu'il n'est pas.

🔴 **Pourquoi PAS un chronomètre du rappel à la complétion** (l'entrée du rappel
ProjFS jusqu'à `PrjCompleteCommand`), qui serait la mesure la plus fidèle de ce
que l'application attend : parce qu'il **dupliquerait l'arbitre avec un
instrument inférieur** — l'arbitre inclut déjà tout cela, et davantage — et
qu'il coûterait une horloge et un champ sur le chemin des rappels, c'est-à-dire
sur les fils que le système possède, où la discipline de fil de
`agent/src/pont/projfs/rappels.rs` interdit d'attendre quoi que ce soit. **La
soustraction est gratuite ; l'instrument ne l'est pas.**

### 0.6 🔵 LE FACTEUR ~120 DE F1 : ce que l'arithmétique en dit déjà, et ce que F4 peut en faire

Le legs 5 de F1 : « le débit varie d'un facteur ~120 entre deux exécutions
(**6,5 Mio/s** contre **52–55 Kio/s**), sans explication ».

**Trois relevés d'archives, pris le 21 août 2026 sur les pièces versées, et ils
resserrent la question sans la fermer :**

1. **Les deux nombres ne viennent pas du même chemin ni du même état.** Le
   6,5 Mio/s vient de `rouge-i-kill-2s.txt` (`issue|LECTURE|OK|1937|lus=12582912`),
   sur une exécution dont F1 dit lui-même que la lecture « s'est terminée
   **avant** la mise à mort ». Le 52–55 Kio/s est **dérivé** de la trace
   périodique `racine hydratee` d'`agent-dbg-plat.log` (`octets=42 → 3 145 770
   → 6 422 570` sur deux intervalles de 60 s), sur une exécution où la sonde de
   borne **a échoué deux fois** (`FileStream.Read(65536)` après 127 865 ms,
   `Read(65537)` après 10 263 ms). **Ce n'est pas un chronomètre**, et l'état
   n'est pas nominal.
2. **L'attribution par les journaux d'archives est IMPOSSIBLE, et pour une
   raison qui est elle-même un résultat.** J'ai cherché la trace
   `racine hydratee` dans les deux journaux du rouge (i) :
   `grep -ac 'racine hydratee' agent-rouge-i-plat.log` → **0**, idem pour
   `agent-rouge-i-b-plat.log`, sur des journaux qui couvrent respectivement
   96 s et 90 s. Elle est pourtant inconditionnelle et `info!`
   (`agent/src/pont/projfs/etat.rs:325`). Sa période est
   `PERIODE_HYDRATATION = 60 s` (`projfs.rs:122`), et le pont a été **tué puis
   relancé** en cours d'exécution (`pid_mort=3540 pid_neuf=1168` à
   `02:05:03.634`), ce qui remet son minuteur à zéro. 🔴 **Une période de 60 s
   ne peut pas attribuer une lecture de 1 937 ms** : l'instrument qui aurait
   tranché n'a pas la résolution pour le faire.
3. **L'arithmétique ne réfute ni ne confirme.** 12 582 912 octets à
   `TAILLE_TRAME_MAX = 64 Kio` font **192 morceaux** ; F1 et F2 n'avaient
   **qu'un morceau en vol** (`MORCEAUX_EN_VOL` est un livrable de F3). 1 937 ms
   / 192 = **10,1 ms par aller-retour**, ce qui est plausible sur un pont où le
   RTT mesuré est de 1 à 3 ms — **donc rien n'est impossible**, et l'hypothèse
   « cette lecture n'a jamais traversé le pont » n'est **pas** établie.
   ⚠️ *Une première rédaction de ce paragraphe la déclarait établie ; le calcul
   la réfute, et il est écrit ici plutôt que la conclusion.*

🔵 **Le point de comparaison le plus solide du dépôt n'est dans aucun document,
et il vient des journaux de F2** : les horodatages des lignes
`ecriture poussee` donnent **≈ 691 ms par aller-retour de 64 Kio** (25 poussées,
min 654 ms, max 839 ms), **reproduits à 1 ms près sur deux exécutions**, soit
≈ **92 Kio/s** — du côté de la borne BASSE. ⚠️ **Mais c'est le chemin
d'ÉCRITURE**, qui porte en plus un `write()` sur le disque du poste local et le
`close()` de committaison : il ne se transporte pas au chemin de lecture.

**Décision D6 — F4 ne « résout » pas le facteur 120 : il le rend caduc.** Il
prend **une** mesure de débit de lecture, **propre, répétée, et dont l'état est
déclaré** (froid / déjà hydraté), et il mesure séparément la question qui
sous-tend l'hypothèse — **une seconde lecture du même fichier coûte-t-elle
quelque chose au pont ?** — parce que c'est de toute façon une question de
produit. Ce que F4 rapportera du legs 5 est : *ce que le pont fait aujourd'hui,
et pourquoi les deux anciens nombres ne se comparaient pas*.

### 0.7 🔵 DEUX MONTAGES — le pont seul, et le pont sous une session vidéo — et leur DELTA mesure R5

**Le montage de F1, F2 et F3 lance `SUPERVISEUR=1`**, donc un capteur, des
fenêtres, des encodeurs et une ou plusieurs `PeerConnection` vidéo. Pour une
mesure de latence, ce sont autant de confondeurs — et le dépôt a payé pour
apprendre qu'une fenêtre parasite suffit à fausser une campagne (la console
PowerShell d'une tâche planifiée devenue une fenêtre éligible, P1
presse-papier).

**Relevé le 21 août 2026 : le pont peut tourner SEUL.**
`agent/src/main.rs:299` branche `PONT` **après** `CAPTEUR` et **avant** le
superviseur ; `agent/src/pont.rs:78-97` fait son **propre** signaling sur
`config.session_id` et répond lui-même à l'offre SDP. Le nom de session est
`<préfixe>:fichiers` — `agent/src/superviseur/protocole::session_du_pont`, dont
`client/src/fichiers/canal.ts:49` porte le miroir
(`NOM_SESSION_DU_PONT = 'fichiers'`, composé par `sessionDuPont()`). **Un agent
lancé avec `PONT=1` et `SESSION_ID=<préfixe>:fichiers`, sans `SUPERVISEUR`,
rencontre donc la page-shell existante sans qu'aucune ligne de client ne
change.**

**Décision D7 — deux montages, et la différence est une mesure :**

| Montage | Composition | Ce qu'il sert |
| --- | --- | --- |
| **M1** | `PONT=1` seul, aucun superviseur, aucune vidéo | **la référence propre.** Toutes les mesures des tâches 9 à 13 |
| **M2** | `SUPERVISEUR=1`, le montage de F3 **inchangé** | **le produit.** Deux points seulement (§tâche 14) |

🔵 **`M2 − M1` mesure R5 — « le pont concurrence la vidéo pour le lien réseau »,
risque nommé et non traité par la spec §10 — sans aucun instrument neuf.**

⚠️ **Portée exacte, à écrire dans le document de résultats** : le delta est
mesuré sur **deux points** (l'énumération au plus grand rang qui passe, et le
débit de lecture à 1 Mio), **deux exécutions chacun**. Il ne dit rien des autres
rangs, et il ne départage pas la contention **réseau** de la contention **CPU
de la VM** — les deux varient ensemble entre M1 et M2. **Nommer R5 n'est pas le
mesurer isolément.**

⚠️ **La porte P0 (tâche 3) court en M1.** Si le pont seul ne monte pas sa racine
ou ne sert pas, M1 tombe et **tout le sous-bloc bascule sur M2**, avec ses
confondeurs déclarés. C'est une issue prévue, pas un échec.

---

## 1. Contraintes globales

### 1.1 Références d'entrée, RELEVÉES PAR LA COMMANDE le 21 août 2026

**`HEAD = 4cdab11`**, arbre de travail portant un seul fichier non suivi
(`docs/superpowers/plans/journaux-presse-papier-p3/pilote-1.log`, du chantier
voisin).

| Commande | Relevé |
| --- | --- |
| `cd agent && cargo test -p agent` | **916 passed; 0 failed** |
| `cd agent && cargo test -p proto` | **109 passed; 0 failed** |
| `cd client && npx vitest run` | **466 passed**, 40 fichiers |
| `cd proto && npx vitest run` | **296 passed**, 9 fichiers |
| `cd agent && cargo check --target x86_64-pc-windows-gnu` | **sortie 0, 22 avertissements** |
| nature des 22 | **tous de la famille `dead_code`** — vérifié un à un (`never used` / `never read`), aucun d'une autre famille |

⚠️ **AUCUN de ces comptes n'est attribuable à F4, ni à F3.** F3 avait relevé
904 / 109 / 462 / 296 / **24** avertissements à sa clôture ; l'arbre a bougé
depuis sous les chantiers voisins. **Ce sont des références datées, pas des
propriétés.** Les mesurer de nouveau est le premier geste de la tâche 1.

⚠️ **Le tableau de dette de `CLAUDE.md` porte QUATRE lignes et il en a DEUX.**
Relevé par la commande de `CLAUDE.md` (§ « Vérifier l'état ») :

```
1536 agent/src/encode.rs
 630 agent/src/windows_source.rs
```

**et rien d'autre.** Les deux lignes que le sous-bloc P1 du presse-papier y
avait inscrites sont **résorbées** : `proto/src/plateforme/tests.rs` vaut **340**
(publié 561) et `proto/ts/plateforme.test.ts` vaut **462** (publié 512). ⚠️ **Ce
n'est ni le fait de F3 ni celui de F4** — F3 l'avait déjà relevé et l'impute à
G1/G3. **Ce plan le consigne ; il ne corrige pas `CLAUDE.md`**, qui n'est pas
son livrable. La tâche 16 le fera à la clôture.

### 1.2 🔴 PÉRIMÈTRE CONCURRENT — et la VM est un préalable EXTERNE, pas une dépendance de tâche

**Un chantier travaille en ce moment dans le même arbre : le presse-papier
P3**, qui tient :

- 🔴 **la VM Windows** (sa recette), et elle est **en cours d'exécution** au
  moment où ce plan est écrit (`virsh list --all` → `Windows | en cours
  d'exécution`, `/media/vm/dev` accessible) ;
- `agent/src/presse_papier*`, `agent/src/capteur/sommeil/`,
  `proto/src/control.*`, `client/src/presse-papier*`.

**Ce qui est à F4** : `agent/src/pont/` (et lui seul dans `agent/`),
`proto/src/fichiers*` et `proto/ts/fichiers*` (⚠️ **F4 n'a pas prévu d'y
toucher** — voir §2), `client/src/fichiers/` (⚠️ **idem**),
`scripts/run-agent.sh` (une ligne), et
`docs/superpowers/plans/journaux-pont-fichiers-f4/`.

**Ce qui n'est à personne de F4** : tout le reste, `CLAUDE.md` excepté à la
tâche 16.

🔴 **LA VM EST UN PRÉALABLE EXTERNE, ET C'EST LA FORMULATION QUI COMPTE.** Elle
n'est **pas** une dépendance de tâche que l'on attend : c'est une ressource
exclusive dont l'indisponibilité **arrête proprement** le sous-bloc au lieu
d'écraser le binaire d'un voisin. C'est la formulation qui a permis à P3 de
s'arrêter proprement, et elle se décline ainsi :

1. **Avant toute tâche qui touche la VM**, relever qui la tient :
   ```bash
   virsh list --all
   node scripts/winrm.js 'Get-Process agent -ErrorAction SilentlyContinue | Select-Object Id,StartTime'
   git log --oneline -5
   ```
2. **Si un agent y tourne et qu'aucune tâche de F4 ne l'a lancé, S'ARRÊTER** et
   le déclarer. **Ne jamais tuer un agent qu'on n'a pas lancé** : F1 a perdu une
   exécution entière parce que deux se sont recouvertes de 2 min 23 s, et *le
   journal versé sous le nom de la première était celui de la seconde*.
3. 🔴 **Ne JAMAIS lancer `scripts/build-agent.sh` quand `proto/` porte des
   modifications non commitées d'un voisin** : il rsynchronise les sources et
   pousserait sur la VM du travail à demi fait. Contrôle obligatoire :
   `git status --porcelain proto/ agent/` **vide** avant tout build.
4. **Le binaire est un état partagé.** `build-agent.sh` échoue en
   `Accès refusé (os error 5)` tant qu'un agent tourne, et l'y forcer écraserait
   le binaire que le voisin mesure. **Relever la taille du binaire après chaque
   build** — « une compilation de 0,13 s est un aveu » —, et
   `cargo clean --release -p proto -p agent`, **les deux crates**.

### 1.3 🔴 `cd client && npx vitest run` NE COUVRE PAS `proto/ts/`

La racine Vitest est `client/`. **Deux commandes, jamais une** :

```bash
cd client && npx vitest run     # client/src/ seul  → 466
cd proto  && npx vitest run     # proto/ts/         → 296
```

Repris de F1, F2 et F3, qui le portent tous trois. *Aucun document du dépôt ne
le disait avant F1.*

### 1.4 Règles de travail

**Commits.** `git add` **nominatif**, jamais `git add -A` : l'index porte du
travail qui n'est pas à nous. Message par fichier :
`git commit -F <fichier> -- <chemins>`, puis **`git show --name-only`** pour
vérifier ce qui est effectivement parti. *Un `git add -A agent/src` a emporté le
travail concurrent d'une autre tâche dans un commit qui ne compilait pas.*

**Le balayage des tailles, PAR LA COMMANDE, sur TOUT l'arbre, avant CHAQUE
commit.** C'est la leçon de la tâche 18 de F3, payée par un plafond **franchi
ET commité** (`agent/src/pont/ecriture/fil.rs` porté de 478 à **612** par une
tâche, le commit parti avec) :

```bash
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>460'
```

⚠️ **Le seuil est 460, pas 500** : c'est la marge dont on a besoin pour voir
venir. Un fichier de F4 qui y apparaît **s'extrait avant l'addition suivante**,
jamais après, et **jamais par compression** — geste que `CLAUDE.md` interdit
nommément.

**Le harnais de rouge.** Chaque mutation est jouée **seule**, et le harnais
refuse de compter une rouge qui n'a rien muté :

1. `cp <fichier> /tmp/f4-temoin/<fichier>` — **une COPIE NOMMÉE**, pas `HEAD` ;
2. muter **par NUMÉRO DE LIGNE**, jamais par sous-chaîne ;
3. `diff /tmp/f4-temoin/<fichier> <fichier>` — **s'il est VIDE, la rouge n'a pas
   eu lieu**, et c'est un échec du harnais, pas un succès du produit ;
4. jouer le contrôle, **lire QUELLE assertion tombe**, jamais seulement le code
   de sortie ;
5. restaurer par **`cp` SANS `-p`** ;
6. `diff` de nouveau — **vide** — et `sha256sum` identique ;
7. `git status --porcelain <chemins>` conforme à l'état d'avant.

🔴 **Les quatre raisons, toutes payées :**
- **`git checkout -- <fichier>` RESTAURE HEAD, PAS L'ÉTAT D'AVANT LA MUTATION** —
  il a effacé du travail non commité **deux fois** dans la branche F2 ;
- **`git diff --numstat` est non vide sur un fichier non commité même quand rien
  n'a bougé** : le harnais de F3 était **vacueux** pour cette raison ;
- **une substitution de chaîne frappe le COMMENTAIRE avant le code**, dans un
  dépôt qui commente ses invariants (piège P4, payé de nouveau en S1) ;
- **`cp -p` préserve la date, et cargo garde alors l'artefact de la version
  mutée** — « le pire cas est l'inverse : une suite VERTE exécutant encore le
  code muté ».

**Le shell de l'hôte.** `unset -f chpwd` avant tout relevé : un hook `chpwd`
injecte un `ls` dans toute sortie dès qu'un `cd` court dans un sous-shell (S2 a
dû reprendre une passe entière). Et **pas de backticks dans un `echo` de
journal** : ils exécutent une commande (S4).

**L'hôte porte un défaut non identifié** : `/dev/null` a été trouvé remplacé par
un fichier ordinaire, ce qui empêchait toute VM de démarrer. Réparé, cause
inconnue, **peut se reproduire**. Contrôle, une ligne, à jouer avant toute
séquence VM : `stat -c '%F' /dev/null` → doit rendre `character special file`.
*(Relevé le 21 août 2026 : conforme.)*

### 1.5 🔴 LA DISCIPLINE D'ÉNONCÉ D'UN CHANTIER DE MESURE — elle s'applique à CHAQUE tâche

Ce n'est pas un préambule : c'est la partie du travail où ce dépôt échoue.

1. **Le nombre d'exécutions est DANS l'énoncé, jamais en note.** « 47 ms » est
   faux ; « 47 ms, une exécution » est vrai. **Deux exécutions ne font pas une
   fréquence** : sur un mécanisme déterministe elles établissent la
   reproductibilité, et rien de plus. **Aucun taux n'est revendiqué nulle part.**
2. **Ce que la mesure établit ET ce qu'elle n'établit pas s'écrivent AVANT de la
   jouer**, dans la tâche, et se recopient dans le document de résultats.
3. **Un témoin par mesure** : le chemin **sans** la fonctionnalité suspecte. Ce
   dépôt a imputé une panne au relais TURN pendant dix minutes avant qu'un
   témoin ne montre que le chemin **direct** tombait pareil.
4. 🔴 **Un contrôle qu'on n'a jamais vu ÉCHOUER n'est pas un contrôle.** Payé au
   moins six fois, dont **trois sur l'instrument écrit pour l'éviter** (F1 : le
   `grep` de D8 rejoué ; F3 : le montage S1 auto-destructeur ; S2 : la rouge
   satisfaite par son propre commentaire). Pour chaque contrôle de F4 : jouer
   l'état qu'il doit dénoncer, et vérifier qu'il le dénonce.
5. 🔴 **L'INSTRUMENT PEUT DÉTRUIRE CE QU'IL MESURE**, et ce dépôt en a **quatre**
   formes : la trace par paquet du chantier TURN (18 619 lignes en quelques
   secondes sur un partage CIFS, la session en est morte) ; la capture d'écran
   CDP de D1 (elle provoquait un `Resize`, donc un `SHOW`, donc une session de
   plus) ; l'`execFileSync` du pilote de F2 (il bloquait la boucle d'événements
   de Node, donc la lecture de la WebSocket CDP, donc **le rendu de la
   page-shell** — vingt `commande expirée` en découlent) ; et le parcours de la
   racine, que `etat.rs:307-323` refuse pour cette raison exacte.
   **Conséquences pour F4, non négociables** : *compter ou échantillonner,
   jamais tracer par unité* ; **aucune** capture d'écran CDP pendant une
   mesure ; **aucun** appel synchrone bloquant dans le pilote ; et **aucune**
   traversée de la racine par l'instrument.
6. ⚠️ **UN PALIER DE MESURE DOIT ÊTRE PLUSIEURS FOIS PLUS LONG QUE LA
   TEMPORISATION DU MÉCANISME QU'IL OBSERVE.** D6 a imputé au produit trois
   échecs qui venaient du protocole, pour un palier de 25 s sur un mécanisme
   temporisé à 20 s. **Les constantes de temporisation de ce terrain sont lues
   AVANT de dimensionner quoi que ce soit :**

   | Constante | Valeur | Fichier |
   | --- | --- | --- |
   | `PERIODE_BALAYAGE` | **250 ms** | `agent/src/pont/service.rs:47` |
   | `PERIODE_RECENSEMENT` | **10 s** | `service.rs:65` |
   | `PERIODE_HYDRATATION` | **60 s** | `agent/src/pont/projfs.rs:122` |
   | `DELAI_ATTRIBUTS` | **2 s** | `agent/src/pont/table.rs:40` |
   | `DELAI_LIRE` | **5 s** | `table.rs:41` |
   | `DELAI_LISTER` | **20 s** | `table.rs:42` |
   | `DELAI_ECRIRE` | **30 s** | `table.rs:58` |
   | `DELAI_MUTATION` | **15 s** | `table.rs:82` |
   | `ATTENTE_MAX` (boucle de transport) | **20 ms** | `agent/src/pont/transport.rs:38` |

   **Règle qui en découle : tout palier de F4 dure au moins 60 s** — six fois
   `PERIODE_RECENSEMENT`, trois fois `DELAI_LISTER`. Et **les mesures se lisent
   sur au moins deux recensements**, jamais sur un.
7. ⚠️ **Une observation CENSURÉE se déclare, elle ne se moyenne pas.** Un rang
   qui atteint `DELAI_LISTER` n'a pas une latence de 20 s : il a une latence
   **supérieure à 20 s**, et le budget est le plafond de l'instrument. La
   colonne du tableau porte alors `> 20 000 (censuré par DELAI_LISTER)`.
8. 🔴 **Ne JAMAIS fabriquer une pièce.** D10 a produit deux fois « un fait vrai
   sans sa preuve » — une transcription `cargo` assemblée à la main, et une
   sortie de commande inventée inscrite dans `CLAUDE.md` **à l'intérieur d'une
   correction qui dénonçait une affirmation non étayée**. Le mécanisme nommé par
   l'implémenteur : *réutiliser la sortie d'une commande antérieure pour
   répondre à la question d'une AUTRE, sans la relancer.* **Chaque chiffre
   publié est relancé au moment où il est écrit.**
9. ⚠️ **Un `grep` de recette vise une chaîne qui EXISTE**, prouvée dans le code
   **ET** dans un journal d'un bras vert avant d'être prescrite. **Trois des
   quatre `grep` de F1 cherchaient des chaînes que le produit n'émet pas, et
   deux auraient fait lire un succès comme un échec.**
10. ⚠️ **`grep -a` partout** sur les journaux : une queue d'octets NUL les classe
    « binaire » et `grep` rend alors une **sortie vide, pas un zéro**.

---

## 2. Structure des fichiers

### 2.1 🔵 L'EMPREINTE PRODUIT DE F4 EST MINUSCULE, et c'est une propriété, pas une chance

Un sous-bloc de mesure qui livre beaucoup de code produit mesure son propre
code. **F4 livre un module pur, une ligne de journal, et une variable.** Tout le
reste est de l'instrument, et l'instrument vit sous `docs/`, **exempt de la
règle des 500 lignes**.

🔵 **Et F4 ne touche AUCUN fichier de `client/src/` ni de `proto/`.** Le gabarit
(les répertoires de N entrées, les fichiers de 4 Kio / 1 Mio / 100 Mio) est
peuplé par la **couche d'injection** (`injection-f4.js`, sous `docs/`), qui
appelle l'API OPFS directement : `client/src/fichiers/adaptateur.ts` liste ce
qu'OPFS porte, sans savoir qui l'a peuplé. **Conséquence heureuse et à
déclarer** : `client/src/fichiers/protocole.test.ts`, à **484 lignes pour une
porte à 500 — marge 16**, la plus étroite du périmètre, **n'est pas approché**.

### 2.2 Créés

| Fichier | Visé | Nature |
| --- | --- | --- |
| `agent/src/pont/latence.rs` | ≤ 180 | **PUR** — aucun `cfg`, aucune E/S, aucune horloge lue en interne (le temps est un paramètre, comme dans `table.rs`). Buckets, compte, somme, max, par famille |
| `agent/src/pont/latence/tests.rs` | ≤ 220 | tests d'hôte |
| `docs/…/journaux-pont-fichiers-f4/instrument/*` | — | **exempt** (`^docs/` est filtré par la commande) |

### 2.3 Modifiés — tailles **RELEVÉES PAR LA COMMANDE le 21 août 2026**

| Fichier | Lignes | Marge | Ce que F4 y met | Budget |
| --- | --- | --- | --- | --- |
| `agent/src/pont/service.rs` | **413** | **87** | la lecture de `PONT_MESURE`, et **une** ligne de recensement de plus | ⚠️ **+30 visé, PORTE À 460** — voir §2.4 |
| `agent/src/pont/table.rs` | **344** | 156 | `resoudre` rend l'âge de la commande | +25 |
| `agent/src/pont.rs` | **264** | 236 | le câblage de l'histogramme dans `Etat` | +15 |
| `agent/src/pont/projfs/etat.rs` | **337** | 163 | le champ `latences` sur `Etat` | +10 |
| `scripts/run-agent.sh` | **160** | — | **une ligne**, tâche DÉDIÉE | +1 |

### 2.4 🔴 La seule porte de ce sous-bloc, et son point de chute est nommé D'AVANCE

**`agent/src/pont/service.rs` est à 413, marge 87.** C'est le seul fichier de
F4 dont la marge se compte en dizaines. Il porte déjà **deux** lignes de
recensement et une fonction `recenser` d'une soixantaine de lignes.

**Porte : si `service.rs` atteint 460 lignes, EXTRAIRE avant l'addition
suivante**, vers **`agent/src/pont/service/recensement.rs`** — `recenser`,
`tout_completer` et les deux `tracing::info!` y partent **verbatim**, le module
se déclarant par un `mod` ordinaire **à l'intérieur** de `service.rs` (aucune
frontière `#[cfg(windows)]` ici : la convention `#[path]` de `CLAUDE.md` **ne
s'applique pas**, comme pour les deux extractions de D11).

🔴 **Pourquoi budgéter une porte à 87 lignes de marge**, quand la règle n'en
demande pas : parce que **F3 a franchi le plafond ET commité** sur ce même
sous-projet, un fichier passant de 478 à 612 **en une tâche**, et que sa propre
leçon est que « le balayage des tailles doit se faire PAR LA COMMANDE, sur TOUT
l'arbre, avant CHAQUE commit ». **Quatre chantiers voisins de suite l'ont nommé
en premier** (G2 ×3, P2 ×3, G3 ×1, F3 ×1). Un budget écrit d'avance coûte trois
lignes.

⚠️ **Marges voisines relevées le même jour, à surveiller sans y toucher** :
`agent/src/pont/transport/tests.rs` **474** (26),
`agent/src/pont/ecriture/fil.rs` **472** (28),
`client/src/fichiers/protocole.test.ts` **484** (16),
`plateforme/src/http/routes-installation.ts` **500** (**0**, fichier de G3,
hors périmètre), `agent/src/encode/arret.rs` **500** (0),
`client/verify-webrtc.mjs` **494** (6). **F4 n'en touche aucun.**

---

## 3. Interfaces

### 3.1 `PONT_MESURE` — la variable, et sa convention

| Variable | Convention | Où |
| --- | --- | --- |
| `PONT_MESURE=1` | 🔴 **`=1` ARME ; l'ABSENCE désarme** — convention de `MICRO_MESURE`, **et NON celle de `PONT`/`PONT_ECRITURE`/`PONT_MUTATION`**, qui sont des variables de PRODUIT désarmées par `=0` | lue dans `agent/src/pont/service.rs`, par `OnceLock` |

🔴 **La convention est INVERSE de celle des trois autres `PONT_*`, et c'est
délibéré.** La règle du dépôt est : *on désarme sur `=0` ce qui est LIVRÉ, on
arme sur `=1` ce qui ne l'est pas.* Le pont, l'écriture et les mutations sont
livrés ; **la ligne de latence est un instrument de banc**, que la spec §8 F4
qualifie de « **variable de BANC, jamais une configuration livrée** ». C'est
exactement l'arbitrage écrit pour `PLEIN_ECRAN_MODE_SORTIE` en D8.

**Trace, émise SEULEMENT si armée**, une fois, au démarrage du fil du service :

```
banc de latence du pont ARME (PONT_MESURE=1) : instrument de banc, jamais une configuration livree
```

⚠️ **`warn!`**, comme `PART_SONDAGE` et `AUDIO_FAUTE_*` : c'est ce qui la rend
visible sous `RUST_LOG=info` et ce qui empêche de la confondre avec une
configuration ordinaire.

🔴 **CE QUE `PONT_MESURE` ARME EST L'ÉMISSION, PAS LA COLLECTE.** L'histogramme
est alimenté **toujours** — trois opérations atomiques sur un chemin qui fait
déjà un `HashMap::remove` sous un `Mutex`. La raison est celle que ce dépôt
répète : **un mécanisme qui n'est armé que pendant sa propre mesure est un
mécanisme que le produit n'exerce jamais**, donc qu'on ne verra jamais rouge.
Collecter toujours signifie que F5 et ses successeurs exercent l'histogramme
sans le savoir, et qu'une régression de sa forme tombera chez eux.

**Et la trace de contrôle vaut ce qu'elle vaut** : elle prouve que la variable a
atteint le processus, **jamais** que la mesure est juste. Le contrôle qui vaut
est le chiffre lui-même — voir le §4, tâche 2.

### 3.2 `agent/src/pont/latence.rs` — PUR, et ce qu'il n'est pas

```rust
/// Une famille de commande, telle qu'on la lit au recensement.
pub enum Famille { Attributs, Lister, Lire, Ecrire, Mutation }

pub struct Histogramme { /* par famille : compte, somme_us, max_us, seaux */ }

impl Histogramme {
    /// Enregistre une traversée ACHEVÉE. `duree` est calculée par l'appelant,
    /// depuis SON horloge : ce module n'en lit aucune, ce qui le rend testable
    /// sans dormir — c'est la discipline de `pont::table`.
    pub fn observer(&self, famille: Famille, duree: Duration);
    /// La ligne de recensement, dans l'ordre de `Famille::TOUTES`.
    pub fn recensement(&self) -> String;
}
```

**Les seaux** : `1, 2, 5, 10, 20, 50, 100, 200, 500, 1000, 2000, 5000, +∞`
millisecondes. Treize seaux, **choisis pour couvrir les trois budgets** (2 s,
5 s, 20 s) et le RTT du pont (1 à 3 ms mesurés en D1). ⚠️ **Non calibrés** — ils
rejoignent la liste que ce dépôt tient depuis `BPP_MIN`.

**Ce que le module n'est PAS, et il faut l'écrire dans son en-tête :**

- **pas une trace par commande.** *Compter ou échantillonner, jamais tracer par
  unité* — la trace par paquet du chantier TURN a tué une session avec
  18 619 lignes sur un partage CIFS ;
- **pas une mesure de ce que l'application attend.** Il mesure la traversée
  pont → navigateur → pont, et **rien d'autre** (§0.5). Le nom des champs le
  dit : `traversee_*`, jamais `latence_*` ;
- **pas un `Mutex`.** Les compteurs sont des `AtomicU64`, comme
  `pont::compteurs`, **et pour la même raison** : ils sont touchés depuis des
  fils que le système possède, où attendre un verrou ferait attendre
  l'application.

🔴 **Le garde structurel, sur le modèle des trois étages de `pont::compteurs`** :
un `match` exhaustif sur `Famille` pour le nom, un `NOMBRE` qui force
l'inscription dans `TOUTES`, et **un test qui épingle l'ORDRE de la ligne** —
« un recensement dont l'ordre dériverait ferait lire un compteur pour un autre ».

### 3.3 La ligne de recensement neuve

Émise **seulement si `PONT_MESURE=1`**, à `PERIODE_RECENSEMENT` (10 s), depuis
`recenser`, **en chaîne unique `nom=valeur`** — jamais en champs `tracing` :

```
traversees attributs=n:12 moy_us:4180 max_us:31002 lister=n:3 moy_us:9840 max_us:12003 lire=n:192 moy_us:10120 max_us:44001 ecrire=n:0 moy_us:0 max_us:0 mutation=n:1 moy_us:512004 max_us:512004 | seaux_ms lire=1:0,2:0,5:12,10:150,20:28,50:2,100:0,200:0,500:0,1000:0,2000:0,5000:0,inf:0
```

⚠️ **Chaîne unique, pour la raison que `service.rs:199-206` écrit déjà** : « un
champ `tracing` porterait des séquences ANSI entre son nom et sa valeur sur un
journal BRUT » — piège payé par la recette d'entrée de D8 et rejoué **trois
fois** par le `grep` de F1. Elle se lit **sans `sed`**.

⚠️ **Les compteurs sont CUMULATIFS depuis le démarrage du pont.** Une mesure se
lit par **différence entre deux recensements**, jamais sur une ligne isolée —
et c'est pourquoi tout palier dure au moins 60 s (§1.5 n°6).

### 3.4 Le gabarit, côté navigateur — dans l'INJECTION, jamais dans le produit

`injection-f4.js` étend `injection-f2.js` (repris **par import, pas par copie** —
« une copie éprouverait la copie, pas l'instrument ») et peuple OPFS à la
demande du pilote :

| Gabarit | Contenu |
| --- | --- |
| `listage/<N>/` | N fichiers de 0 octet, nommés `f-00000.txt` … sur **18 caractères**, pour que le poids par entrée soit celui du §0.1 |
| `lecture/4k.bin`, `1m.bin`, `100m.bin` | 4 096 / 1 048 576 / 104 857 600 octets, contenu pseudo-aléatoire de graine fixe |
| `mutation/1m.bin`, `mutation/100m.bin` | idem, pour le repli de renommage |

🔴 **Le garde `EST_SHELL` de F2 est CONSERVÉ**, et il n'est pas optionnel : sans
lui, l'injection court aussi sur les fenêtres d'application ouvertes par
`window.open`, **qui repeuplent OPFS pendant que le pont y lit**. F2 l'a payé —
« la garde de casse n'avait pas failli : l'homonyme qu'elle cherche avait été
effacé sous elle par une autre page ».

⚠️ **En M1 il n'y a aucune fenêtre d'application** (§0.7), donc le garde n'a
rien à garder — **et il reste**, parce que M2 en a.

⚠️ **Le peuplement d'un gabarit se mesure lui-même et se journalise** : créer
10 000 entrées dans OPFS n'est pas gratuit, et une mesure lancée sur un gabarit
à demi écrit rendrait un chiffre qui ne veut rien dire. Le pilote attend le
**FAIT** — un relevé du nombre d'entrées réellement présentes —, jamais une
durée.

### 3.5 Le chronomètre de la VM — l'ARBITRE

`mesurer-f4.ps1`, transposé de `mesurer-f3.ps1`, dont il garde **les quatre
propriétés payées** :

1. **un chemin de relevé NEUF par exécution** (`mesure-f4-$HORO.json`) : « une
   tentative figée tient son fichier de relevé, et la suivante écrit dans le
   vide » — le verrou a survécu à **trois** tentatives en F3 ;
2. **des points de reprise**, le JSON étant écrit **après chaque famille de
   mesure** et pas seulement à la fin : « un instrument qui n'écrit qu'à la fin
   fait dépendre toute la mesure du geste le plus fragile », et F3 a perdu deux
   critères déjà mesurés pour cette raison ;
3. **la trace et le relevé dans DEUX fichiers distincts** : `Out-File` tronque à
   l'ouverture et tient la poignée, donc un `Set-Content` intercalaire serait
   perdu ;
4. **lancé par tâche planifiée `/it`**, en **session 1** : WinRM tourne en
   session 0, où la racine ProjFS d'un processus de la session 1 n'est pas la
   même chose, et où aucun bureau n'existe.

**Le chronomètre lui-même** : `System.Diagnostics.Stopwatch`, jamais
`Measure-Command` — celui-ci enveloppe un bloc de script et son propre coût
entre dans le chiffre.

🔴 **ET IL N'Y A AUCUNE BORNE DE JOB.** `Start-Job` + `Wait-Job -Timeout` rend
un **interblocage** sous tâche planifiée (`BlockedJobsDeadlockWithWaitJob`,
trace versée par F3). **La seule borne qui tient est celle du PRODUIT** — les
budgets de `table.rs` —, et c'est précisément ce que F4 mesure. Un geste dont
le produit ne borne pas la durée (§0.1 : le rang qui franchit le mur) doit donc
être **le dernier de sa famille**, après son point de reprise.

---

## 4. Les tâches

**Seize tâches.** Les tâches 1, 2, 4, 5 et 6 ne touchent pas la VM ; les tâches
3 et 7 à 15 la touchent et sont **strictement séquentielles** entre elles
(§1.2, §5).

---

### Task 1 — contrôle d'entrée et relevé de référence — AUCUN code

**Ne modifie aucun fichier.** Produit
`docs/…/journaux-pont-fichiers-f4/t1-controle-entree.txt`.

```bash
unset -f chpwd
cd /home/mallanic/Projects/Guacamole
stat -c '%F' /dev/null                 # doit rendre : character special file
git rev-parse --short HEAD && git status --porcelain | head -20
virsh list --all
node scripts/winrm.js 'Get-Process agent -ErrorAction SilentlyContinue | Select-Object Id,StartTime'

# Les QUATRE suites, complètes, DEUX commandes pour les tests TS
( cd agent  && cargo test -p agent )   ; ( cd agent && cargo test -p proto )
( cd client && npx vitest run )        ; ( cd proto && npx vitest run )
( cd agent  && cargo check --target x86_64-pc-windows-gnu 2>&1 | tail -3 )

# Le plafond, sur TOUT l'arbre, seuil 460
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>460'

# Ce que le monde d'entrée porte VRAIMENT (les trois faits du §0)
grep -rn 'TTL_ENUMERATION' agent/ proto/ client/src/ 2>/dev/null | grep -v node_modules
grep -rn 'PONT_MESURE' agent/ proto/ client/ scripts/ 2>/dev/null | grep -v node_modules
grep -n 'LOCAL_MAX_MESSAGE_SIZE' \
  ~/.cargo/registry/src/*/str0m-0.21.0/src/sctp/mod.rs
```

**Écrire le résultat, pas le recopier de ce plan.** Les références du §1.1 sont
datées du 21 août 2026 ; **l'arbre est partagé**, et P3 committe.

⚠️ **Si un compte diffère de plus de quelques unités, dire lequel et
l'attribuer par `git log --oneline -20`** — jamais l'imputer à F4 par défaut,
jamais le taire.

⚠️ **Si `grep 'TTL_ENUMERATION'` rend autre chose qu'un unique commentaire dans
`enumeration.rs`, la décision D2 est caduque et le §0.2 doit être rejoué**
avant toute autre tâche.

---

### Task 2 — `scripts/run-agent.sh` transmet `PONT_MESURE` — tâche DÉDIÉE

**Une ligne**, et rien d'autre. À placer auprès des trois autres `PONT_*` :

```sh
${PONT_MESURE:+\$env:PONT_MESURE = '$PONT_MESURE'}
```

🔴 **TÂCHE DÉDIÉE, ET JOUÉE AVANT CELLE QUI EN A BESOIN.** Ce dépôt a payé ce
piège **trois fois** — `SUPERVISEUR` en D1, `MULTIFENETRE_REPRISE` en D2,
`AUDIO` en D7, où *l'implémenteur et le relecteur avaient vérifié la propriété
en traçant le code : le tracé était juste, la valeur ne pouvait simplement pas
atteindre le processus*. F2 et F3 ont chacun consacré une tâche à leur variable
pour cette raison.

**Contrôle, en trois temps, et le troisième est le seul qui vaille :**

1. **la forme** : `PONT_MESURE=1 scripts/run-agent.sh` (ou un rendu du heredoc
   seul), puis `grep -n 'PONT_MESURE' /media/vm/dev/run-agent.ps1` → **une**
   ligne, `$env:PONT_MESURE = '1'` ;
2. **l'atteignabilité** : sans la variable, `grep` rend **zéro** ligne — la
   forme `${VAR:+…}` n'écrit rien, et c'est ce qui doit être vérifié plutôt que
   supposé ;
3. 🔴 **le FAIT** : l'agent lancé avec la variable **journalise**
   `banc de latence du pont ARME (PONT_MESURE=1)`, et **la ligne `traversees`
   apparaît au recensement**. Sans la variable, **ni l'une ni l'autre**.
   *(Ce troisième temps ne peut être joué qu'après la tâche 5 ; la tâche 2 pose
   les deux premiers et **inscrit le troisième comme dû**, avec le nom de la
   tâche qui le jouera.)*

⚠️ **La trace ne prouve pas la mesure.** Elle prouve que la variable a atteint
le processus. Le contrôle du mécanisme est le chiffre, et il est en tâche 5.

---

### Task 3 — 🔴 PORTE P0 : LE MUR DU LISTAGE — éliminatoire pour le choix des rangs

**C'est la première tâche qui touche la VM, et elle décide de ce que les autres
mesurent.** Montage **M1** (§0.7).

**Ce qu'elle établit** : à partir de combien d'entrées une énumération cesse
d'aboutir, **et sous quelle forme** elle cesse d'aboutir.
**Ce qu'elle n'établit pas** : pourquoi. La cause prédite (§0.1) est le
`max-message-size` de 256 Kio, mais un mur observé à une autre valeur, ou une
troncature silencieuse au lieu d'un refus, seraient des faits différents **et il
faudra le dire**.

**Étapes :**

1. **Éprouver M1 lui-même**, avant tout. Lancer l'agent avec
   `PONT=1`, `SESSION_ID=<préfixe>:fichiers`, **sans `SUPERVISEUR`**, la
   page-shell connectée d'abord. Attendre le **FAIT** : la ligne
   `racine du pont fichiers montée racine=…` **et** `canal du pont ouvert : le
   navigateur peut servir les requêtes`.
   ⛔ **Si M1 ne monte pas, tout le sous-bloc bascule sur M2** — le montage de
   F3 inchangé — et le §0.7 est annoté d'un constat, pas d'un renoncement.
2. **Dichotomie**, gabarits `listage/<N>/` peuplés par l'injection, avec
   `N ∈ {10, 100, 1 000, 2 000, 3 000, 4 000, 6 000, 10 000}` puis resserrement.
   Pour chaque N : un `Get-ChildItem` chronométré, et le relevé du nombre
   d'entrées rendues.
3. **Trois issues à distinguer, et la troisième est la pire :**

   | Issue | Signature |
   | --- | --- |
   | **aboutit** | le compte rendu par `Get-ChildItem` == N, durée bornée |
   | **échoue proprement** | erreur d'E/S, et `delai-depasse` **ou** un autre code monte au recensement |
   | 🔴 **tronque en SILENCE** | le compte rendu **< N**, **sans aucune erreur** — c'est-à-dire un répertoire qui paraît plus petit qu'il n'est. **Le contrôle qui l'attrape est le COMPTE, pas l'absence d'erreur**, et c'est pour cela que le compte est relevé à chaque rang |

4. **Relever, côté navigateur, ce que le `send()` a fait** : la trame
   `TYPE_ENTREES` est-elle partie ? `client/src/fichiers/canal.ts:134` ne
   journalise qu'un `console.warn('trame fichiers non traitée', e)` — **le
   pilote doit capter la console de la page-shell** (`Runtime.consoleAPICalled`)
   et verser son contenu. Sans cela, un refus du navigateur serait indiscernable
   d'un silence du produit.
   ⚠️ **`Runtime.consoleAPICalled` rend la chaîne `"Object"` pour tout argument
   objet** (piège S4) : les champs se relèvent dans `preview.properties`.

**Sortie** : `N_max` (le dernier rang qui aboutit), `N_mur` (le premier qui
échoue), la **forme** de l'échec, et la taille d'en-tête calculée à ces deux
rangs. **Deux exécutions.**

**Le témoin qui rend la porte discriminante** : le rang **10** doit aboutir.
Un montage où même 10 entrées échouent ne mesure pas un mur — il mesure une
panne, et le dire évite d'attribuer au protocole ce qui vient du gabarit.

---

### Task 4 — `agent/src/pont/latence.rs` — le module PUR, ses tests, et sa rouge

**Ne touche pas la VM.** Écrit `latence.rs` et `latence/tests.rs` selon §3.2.

**Les tests d'hôte, et ce que chacun doit pouvoir attraper :**

| Test | Ce qu'il dénonce |
| --- | --- |
| `un_histogramme_neuf_rend_des_zeros` | rien — c'est le témoin de départ |
| `une_traversee_tombe_dans_le_seau_qui_la_contient` | une borne de seau à l'envers (`<` contre `<=`) |
| `les_familles_ne_se_melangent_pas` | un `rang()` faux — le jumeau du garde de `pont::compteurs` |
| `le_max_est_le_max_et_la_moyenne_est_la_moyenne` | une somme et un compte échangés |
| `l_ordre_du_recensement_est_epingle` | 🔴 une dérive d'ordre, qui ferait **lire un compteur pour un autre** |
| `une_famille_neuve_ne_peut_pas_heriter_du_nom_d_une_autre` | le troisième étage du garde structurel |

🔴 **La rouge du module, et elle doit être VUE** : donner à `Famille::Lire` le
même rang qu'à `Famille::Lister` fait tomber
`les_familles_ne_se_melangent_pas` **et** `l_ordre_du_recensement_est_epingle`,
et **elles seules** — le compte `n failed | m passed` est la preuve que la
mutation n'a touché que ce qu'elle visait.

⚠️ **Un test qui ne vérifie qu'un `Default` n'est pas un test.** C'est le legs
n°11 de D9 (`une_telemetrie_neuve_est_a_zero`, « incapable de rendre l'autre
valeur ») : `un_histogramme_neuf_rend_des_zeros` **n'est gardé que parce qu'il
est le témoin de départ des autres**, et son en-tête doit le dire.

---

### Task 5 — `table.rs`, `etat.rs`, `service.rs`, `pont.rs` — le câblage, et le contrôle qui vaut

**Ne touche pas la VM.**

1. **`table.rs`** : `resoudre` rend l'âge. `EnVol` porte **déjà** `inscrite_a:
   Instant` (`table.rs:156`, posé par F3 pour `plus_ancienne`) — **rien à
   ajouter à la structure**, seulement à la valeur de retour :
   ```rust
   pub fn resoudre(&mut self, correlation: u32, maintenant: Instant)
       -> Option<(Option<i32>, Attendue, Duration)>
   ```
   ⚠️ **`maintenant` est un PARAMÈTRE**, comme partout dans ce module : « aucune
   horloge lue en interne — le temps est un paramètre, ce qui rend l'expiration
   testable sans dormir ».
2. **`etat.rs`** : un champ `latences: latence::Histogramme` sur `Etat`.
3. **`service.rs`** : `observer` à chaque résolution, la lecture de
   `PONT_MESURE` par `OnceLock`, et **une** ligne de recensement de plus.
4. **`pont.rs`** : la trace d'armement (§3.1).

🔴 **LE CONTRÔLE QUI VAUT N'EST PAS QUE LA LIGNE SORT — c'est qu'elle compte ce
qu'elle dit.** Un test d'hôte ne peut pas le donner (`service.rs` est sur le
chemin Windows), donc le contrôle est une **rouge de câblage**, jouée en
tâche 7 sur la VM :

| Bras | Attendu |
| --- | --- |
| `PONT_MESURE` absente | **0** ligne `banc de latence`, **0** ligne `traversees` |
| `PONT_MESURE=1`, aucun geste | ligne d'armement, ligne `traversees` avec **`n:0` partout** |
| `PONT_MESURE=1`, un `Get-ChildItem` | `lister=n:1` au recensement suivant, et `attributs=n:>0` |

⚠️ **Le deuxième bras est ce qui rend le troisième discriminant** : une ligne à
zéros prouve que l'émission ne dépend pas des gestes, donc qu'un `n:1` est bien
un geste et non l'apparition de la ligne.

**Porte de taille** : `service.rs` à **413** avant. **Le relever après**, et si
≥ 460, extraire vers `service/recensement.rs` **dans le même commit que
l'addition** (§2.4).

---

### Task 6 — l'instrument : `injection-f4.js`, `pilote-f4.mjs`, `mesurer-f4.ps1`, `jouer-f4.sh`

**Ne touche pas la VM** (elle écrit les fichiers, la tâche 7 les joue).
Tout sous `docs/…/journaux-pont-fichiers-f4/instrument/`.

**Réemploi par IMPORT, jamais par copie** — « une copie éprouverait la copie,
pas l'instrument » (F3) : `commun-f1.mjs` et la structure d'`injection-f2.js`
sont importées depuis leurs répertoires d'origine.

**`jouer-f4.sh`** est transposé de `jouer-f3.sh` et garde ses cinq gestes :
tuer les agents **avant** et recompter **après, y compris si l'exécution a
échoué** ; purger la racine ; **`rm -f` de TOUS les artefacts de l'exécution
précédente** (« un `pilote-arme-1.json` laissé par une tentative antérieure a
été lu comme le résultat de celle-ci ») ; copier `agent.log` **6 s après la fin
réelle** ; produire le jumeau `-plat`.

**Ce qui change** : `APRES_CONNEXION` pose **`PONT=1` et `SESSION_ID`** au lieu
de `SUPERVISEUR=1` (montage M1), et **`PONT_MESURE=1`**.

⚠️ **Les pièges d'instrument à reprendre tels quels, tous payés :**

- 🔴 **aucun appel synchrone bloquant dans le pilote** — `execFileSync` bloque
  la boucle d'événements de Node, donc la lecture de la WebSocket CDP, **donc le
  rendu de la page-shell** (F2 : vingt `commande expirée`, puis vingt `réponse
  tardive` **dans la même seconde**). `execFileAsync` partout ;
- 🔴 **aucune capture d'écran CDP** pendant une mesure (D1) ;
- 🔴 **toujours `Runtime.runIfWaitingForDebugger`** — « une cible laissée en
  attente de débogueur ne charge JAMAIS » ;
- 🔴 **re-poser l'injection ET l'attendre avant de naviguer** : l'auto-attache
  est asynchrone, et F2 a perdu une exécution dont le symptôme — `#etat-fichiers`
  à `null` — *se lit comme un lecteur non monté* ;
- 🔴 **`evalBorne` rend un OBJET** (`{__timeout}`, `{__erreur}`) : un
  `JSON.parse` dessus lève, et cela a tué le pilote de `reprise-1` **à son 51ᵉ
  échantillon** alors que le produit fonctionnait. **Conserver l'échantillon
  illisible plutôt que le sauter** — *un trou silencieux dans une série se lit
  comme une série continue* ;
- 🔴 **substituer par `replaceAll`**, et **ne nommer aucun marqueur dans un
  commentaire** ; un garde lève si un marqueur `__…__` survit ;
- ⚠️ **le jeton est OBTENU** par `POST /auth/connexion`, jamais forgé ;
- ⚠️ **`nodejs-winrm` enveloppe tout dans `powershell -Command "& { … }"`** :
  écrire le script sur le partage et l'invoquer par **`-File`**, sans quoi le
  symptôme est un script qui ne tourne jamais ;
- ⚠️ **la sortie WinRM mutile les accents** : `StreamWriter` UTF-8 sans BOM,
  **une seule voie de sortie nommée** ;
- ⚠️ **`Get-ChildItem | ForEach-Object { … }` émet dans le PIPELINE**, pas par
  `Write-Output` : surcharger `Write-Output` ne suffit pas, et F1 en a conclu
  **une énumération vide, c'est-à-dire une panne du produit**, alors que
  l'instrument ne regardait pas au bon endroit.

---

### Task 7 — 🔴 la rouge de câblage de `PONT_MESURE`, sur la VM

Joue les **trois bras** de la tâche 5, montage M1, **une exécution par bras**.
C'est le troisième temps du contrôle de la tâche 2, et **c'est lui qui ferme la
variable**.

⚠️ **Aucune mesure n'est prise ici** : le sous-bloc ne mesure rien avant que son
instrument ne soit démontré capable de compter zéro **et** de compter un.

---

### Task 8 — SONDE A : une seconde lecture traverse-t-elle le pont ?

**Ce qu'elle établit** : si le pont sert, ou non, une relecture d'un fichier
déjà hydraté.
**Ce qu'elle n'établit pas** : le facteur ~120 de F1. Elle **écarte ou retient
une hypothèse**, elle n'explique pas deux mesures anciennes prises sur deux
chemins et deux états différents (§0.6).

**Montage M1. Deux exécutions.** Sur `lecture/1m.bin` :

1. lire une fois, chronomètre VM + différentiel du recensement `lire=n:` ;
2. lire une **seconde** fois, idem ;
3. comparer.

| Issue | Lecture |
| --- | --- |
| `lire=n:` **n'augmente pas** au second passage | ProjFS sert depuis l'hydratation ; **le pont n'est pas sur le chemin d'une relecture**, et une mesure de débit qui ne déclare pas son état ne mesure pas le pont |
| `lire=n:` **augmente autant** | chaque lecture traverse ; l'hydratation ne dispense de rien, et **R4 (le disque de la VM se remplit) coûte sans rien rendre en latence** |

🔵 **C'est une question de PRODUIT avant d'être une question de méthode**, et
c'est ce qui justifie de la poser : le §6.4 de la spec écrit que « l'hydratation
ProjFS **est** le cache de données ». Cette sonde est la première mesure de
cette affirmation.

⚠️ **Le contrôle qui rend la sonde capable d'échouer** : la **première** lecture
doit faire monter `lire=n:` de 16 (1 Mio / 64 Kio). Si elle ne le fait pas, la
sonde ne mesure pas ce qu'elle croit, et le rang est écarté.

---

### Task 9 — MESURE 1 : l'énumération, aux rangs DÉRIVÉS de la porte P0

**Montage M1, deux exécutions, paliers ≥ 60 s.**

| Rang | Grandeurs relevées |
| --- | --- |
| 10, 100, 1 000, `N_max`, `N_mur` | **arbitre** : mur-à-mur d'un `Get-ChildItem`, chronomètre VM |
| | **décomposition** : différentiel `lister=n:`, `moy_us`, `max_us` |
| | **résidu** : arbitre − traversée (§0.5), **déclaré comme ordre de grandeur** |
| | le **compte d'entrées rendues**, à chaque rang (§tâche 3, issue « tronque en silence ») |

**Et le motif de l'Explorateur**, qui est celui que la spec §7.4 nomme : lister
**puis interroger chaque entrée**. Aux rangs 10, 100 et 1 000 :
`Get-ChildItem | ForEach-Object { $_.Attributes }`, chronométré à part.
🔵 **C'est ce couple qui répond réellement à la question du cadrage** : un
listage rapide suivi de N interrogations lentes dégrade l'expérience autant
qu'un listage lent.

⛔ **Ce que cette tâche NE mesure PAS, et il faut l'écrire dans le rapport** :
**le dialogue `IFileOpenDialog` lui-même n'est pas ouvert.** `ShowDialog()`
bloque en attendant un utilisateur, et le piloter demanderait
`Xvfb` + `xdotool` côté hôte — dont le consentement a été **donné en D8 et
jamais suivi d'effet**, et qu'un chantier vient de relever **absents**.
`Get-ChildItem` + `GetAttributes` en est une **approximation**, et elle omet ce
que le shell fait en plus : extraction d'icônes, vignettes, sondages de
`desktop.ini` et de `Thumbs.db` — dont la tâche 10 mesure au moins la part
absorbée par le cache négatif.

---

### Task 10 — MESURE 2 : `GetPlaceholderInfo` froid / chaud, et le différentiel du cache négatif

**Montage M1, deux exécutions.**

**Moitié A — `GetPlaceholderInfo`.** Le rappel `info_marqueur`
(`agent/src/pont/projfs/rappels.rs:150`) inscrit une commande `Attributs`
(budget `DELAI_ATTRIBUTS = 2 s`). Mesure : un `Get-Item` sur une entrée jamais
touchée (**froid**), puis sur la même (**chaud au sens de §0.2 : son substitut
est écrit**), différentiel `attributs=n:` et `moy_us`.

⚠️ **Le « chaud » mesuré est celui que le produit A**, et le rapport doit le
nommer ainsi : *substituts déjà posés*, **pas** cache d'énumération, qui
n'existe pas.

**Moitié B — le différentiel du cache négatif** (§0.3), et **son témoin** :

| Bras | Mutation | Attendu |
| --- | --- | --- |
| vert 1 | aucune | 1ʳᵉ ouverture : `introuvable + chemin-introuvable` = *a* ; 2ᵉ : *b* |
| vert 2 | aucune | reproduit |
| 🔴 **témoin** | `PRJ_FLAG_USE_NEGATIVE_PATH_CACHE` retiré (`projfs.rs:214`), **une ligne, par numéro de ligne** | *a′* et *b′*, et **`b′ ≈ a′`** si le cache était bien ce qui absorbait |

⚠️ **La mutation du témoin porte sur du code de PRODUIT et sur la VM** : elle
exige un rebâtissage (`cargo clean --release -p proto -p agent`, la taille du
binaire relevée), et la restauration se vérifie par `sha256sum` **et** par un
second rebâtissage. **Le binaire témoin doit s'identifier lui-même** — legs n°6
du chantier E : « le rouge ne se distingue du vert que par le nom de son
fichier, l'attribution n'est pas rejouable sur pièces ». **Verser la taille des
deux binaires.**

⚠️ **Si `a == b` sur les deux verts**, le cache n'absorbe rien de mesurable sur
ce montage — **et le témoin devient la seule pièce qui distingue « Windows ne
sonde pas ici » de « le drapeau n'est pas appliqué »**. Les deux se déclarent.

---

### Task 11 — MESURE 3 : première lecture et débit soutenu, aux rangs DÉRIVÉS

**Montage M1, deux exécutions.** Rangs **4 Kio, 1 Mio**, puis **100 Mio
conditionnel**.

🔴 **LE RANG 100 Mio EST DÉRIVÉ DE LA MESURE À 1 Mio, JAMAIS SUPPOSÉ.** F1 laisse
deux chiffres incompatibles d'un facteur 120 ; à la borne basse (52 Kio/s),
100 Mio prendraient **plus de trente minutes**, et F2 écrit que « à la borne
basse, un fichier de 12 Mio reste **environ quatre minutes** dans la fenêtre ».

**Règle d'admission, écrite avant de jouer** : si l'extrapolation depuis le
débit mesuré à 1 Mio dépasse **dix minutes**, le rang 100 Mio est **abandonné**,
et le rapport porte **l'extrapolation avec sa base** — pas un silence. Un rang
intermédiaire (10 Mio) le remplace.

**Grandeurs** : mur-à-mur VM ; différentiel `lire=n:`, `moy_us`, `max_us` ; les
seaux ; et **`en_vol_max`**, que F3 a livré — la borne vit dans
`agent/src/pont/lecture.rs`, **la trace qui la rend lisible est ailleurs** :
`agent/src/pont/service/reponses.rs:221`, `lecture complète … en_vol_max=N`,
niveau `DEBUG`.

🔵 **`en_vol_max` est le chiffre-juge du legs n°2 de F3** — « `SEUIL_TAMPON` et
`MORCEAUX_EN_VOL` ne sont pas calibrées, et le contrôle de flux n'a jamais été
exercé sous charge ». F3 l'écrit lui-même : *« un maximum resté à 1 pendant une
lecture de 12 Mio est un échec du livrable, pas un détail »*. **Sur une lecture
de 1 Mio (16 morceaux), `en_vol_max` doit valoir `MORCEAUX_EN_VOL = 4`.**

⚠️ **Cette lecture exige `RUST_LOG=info,agent::pont=debug`**, et **ce n'est pas
neutre** : F1 a payé « ne jamais tracer par paquet ». `agent::pont=debug` produit
une ligne par morceau poussé. **Le mesurer** : le rang 100 Mio en `debug`
produirait 1 600 lignes sur un partage CIFS. **Décision : `debug` seulement aux
rangs 4 Kio et 1 Mio ; les rangs supérieurs en `info`**, où `en_vol_max` n'est
pas lisible et où on le dit.

---

### Task 12 — MESURE 4 : le coût du repli de renommage par copie, ses DEUX moitiés

**Deux exécutions par moitié.** Voir la décision D4 (§0.4).

**Moitié FICHIER — chemin réel, mesure FORCÉE.** L'injection neutralise `move`
(`delete FileSystemFileHandle.prototype.move`) ; la VM renomme
`mutation/1m.bin` puis `mutation/100m.bin` *(si le rang 100 Mio a été admis en
tâche 11 ; sinon 10 Mio)*.

| Grandeur | Source |
| --- | --- |
| mur-à-mur du `Rename-Item` | chronomètre VM |
| la traversée | différentiel `mutation=n:`, `moy_us` |
| octets et entrées recopiés | trace de la page-shell, `renommage par copie « … » → « … » : N octets, M entree(s)` |
| **la marge au budget** | `DELAI_MUTATION = 15 s` |

🔴 **Le témoin est GRATUIT et il est obligatoire** : la même mutation **sans**
neutraliser `move`. La différence est le coût du repli, et **sans ce bras on
mesurerait un renommage sans savoir ce qu'il a coûté de plus**.

⚠️ **Le contrôle qui rend la mesure valide** : la trace `renommage par copie`
doit **apparaître** dans le bras forcé et **être absente** du témoin. F3 a
vérifié qu'elle est absente de **tous** ses journaux — c'est la ligne de base.

**Moitié RÉPERTOIRE — mesure de COMPOSANT, hors produit.** Une sonde côté
navigateur seule, sur le modèle de `s2-move-casse.mjs`, sur un arbre OPFS de
profondeur et de largeur connues. **Elle ne passe par aucun rappel ProjFS**,
parce que ProjFS refuse le renommage d'un répertoire avant de consulter le
fournisseur (F3 §2). **Le rapport la range dans une section séparée**, et ne
l'additionne à rien.

---

### Task 13 — MESURE 5 : l'ouverture d'un dossier par le shell Windows

**Montage M1, deux exécutions.** C'est la ligne « ouverture d'un dossier dans
l'Explorateur » de la table de la spec.

`explorer.exe "<racine>\listage\<N>"` en **session 1**, maintenu 30 s, puis
fermé ; différentiel des douze codes au recensement, et de `attributs=n:` /
`lister=n:`.

🔴 **En M1 il n'y a AUCUN superviseur, donc la fenêtre de l'Explorateur n'est
capturée par personne.** C'est la raison pour laquelle cette mesure est en M1 et
pas ailleurs : sous `SUPERVISEUR=1`, elle deviendrait une fenêtre éligible, donc
une session, donc une sortie virtuelle — *le piège que P1 presse-papier a payé
avec une console PowerShell devenue une fenêtre éligible en pleine mesure*.

⚠️ **Ce que cette mesure ajoute à la tâche 9** : le shell sonde des chemins que
`Get-ChildItem` ne sonde pas (`desktop.ini`, `Thumbs.db`, `folder.jpg`, les
manifestes). **C'est le seul point de F4 où le comportement réel du shell est
exercé**, et c'est ce qui donne son sens au différentiel du cache négatif de la
tâche 10.

⛔ **Ce qu'elle ne fait toujours pas** : ouvrir un `IFileOpenDialog`. Voir
tâche 9.

---

### Task 14 — MESURE 6 : le delta M2 − M1, qui nomme R5

**Deux points seulement, deux exécutions chacun** (§0.7) : l'énumération à
`N_max`, et le débit de lecture à 1 Mio. Montage **M2** = celui de F3,
**inchangé**, avec `PONT_MESURE=1`.

⚠️ **Ce que le delta établit** : que le pont mesuré sous une session vidéo rend
des chiffres différents, et de combien.
⚠️ **Ce qu'il n'établit PAS, et c'est la moitié qui compte** : il **ne
départage pas** la contention **réseau** (R5, le budget `BUDGET_BPS` qui ignore
ce trafic) de la contention **CPU de la VM** (l'encodeur, la capture). Les deux
varient ensemble entre M1 et M2. **Nommer R5 n'est pas le mesurer isolément**,
et un rapport qui écrirait « R5 vaut X % » serait faux.

🔵 Relever au passage `framesDecoded` et `packetsLost` : c'est le témoin
« vidéo intacte » du cadrage §7, et il est gratuit ici.
⚠️ **Il a déjà été NON MESURABLE une fois** — F2, critère ⑥ : `window.__pc`
absent, fenêtre `endormie=true images=0`, limite héritée de D5. **S'il l'est de
nouveau, le dire ; ne pas le remplacer par un raisonnement.**

---

### Task 15 — RECETTE : la conclusion sur la question du cadrage

**Aucune mesure neuve.** Elle assemble, et elle répond — ou déclare qu'elle ne
peut pas.

**La question, mot pour mot** : « le bypass ne se justifie que si le listage
d'un dossier volumineux **dégrade réellement l'expérience** — décision à
trancher sur mesure, pas sur intuition ».

**Les trois issues, toutes acceptables, et chacune avec ce qu'elle exige :**

| Issue | Ce que le rapport doit porter |
| --- | --- |
| **le listage ne dégrade pas** | les rangs, les durées, **le nombre d'exécutions**, et la réserve du §0.2 (mesuré **sans** cache d'énumération, donc borne haute du coût — la conclusion est **robuste**) |
| **le listage dégrade** | idem, **plus** la réserve inverse : F4 **ne peut pas dire** si le cache de F5 y remédierait. La conclusion est **conditionnelle** |
| 🔴 **le listage ne FONCTIONNE PAS au-delà de `N_mur`** | ce n'est plus une question de confort : c'est un **défaut de produit**, et il se rapporte comme tel, avec sa cause si elle est établie et sans elle si elle ne l'est pas |

⚠️ **Et une quatrième issue est possible : « ne tranche rien ».** Si les mesures
sont trop dispersées, ou si le montage a bougé entre deux exécutions, **le dire
est le résultat**. La spec l'écrit : « la spec promet un plan de mesure, pas un
verdict ».

**Ce que la recette doit AUSSI porter, et qui n'est pas la question du
cadrage :**

- 🔵 **de quoi calibrer les trois budgets du §5.3** — `DELAI_ATTRIBUTS`,
  `DELAI_LIRE`, `DELAI_LISTER` — que la spec §11 annonce comme le seul acquis
  de calibration de tout le sous-projet. **Les distributions, pas des valeurs
  proposées** : proposer un nombre serait décider, et calibrer une constante
  demande un jugement d'usage que F4 ne porte pas ;
- `en_vol_max` contre `MORCEAUX_EN_VOL`, pour le legs n°2 de F3 ;
- ce que la sonde A dit de l'hydratation, pour le legs n°5 de F1 ;
- **et le legs n°4 de F1** — « des lectures calent sans jamais expirer, on ne
  sait pas où » : `plus_ancienne_ms` et le tableau de lecture à quatre lignes de
  la doc de `table.rs::plus_ancienne` **existent depuis F3 et n'ont jamais eu de
  cas non trivial
  à départager** (toutes les lignes versées portent `en vol=0 …
  plus_ancienne_ms=0`). **Si F4 rencontre un calage, il a l'instrument** ; s'il
  n'en rencontre aucun, il doit le dire — *l'absence d'un symptôme sur deux
  exécutions n'est pas sa disparition*.

---

### Task 16 — revue transverse, document de résultats, `CLAUDE.md`

**Trois gestes, dans cet ordre.**

**① La revue transverse**, dont la cible propre est **les affirmations devenues
fausses dans la branche elle-même**. Barème : 5 en D7, 3 en D8, 6 en D9, 12 en
D10, 7 en D11, 8 en P1, 10 en P2 (sur **23 places**), 5 en S1, 9 sur le
chantier E, 12 en P3, 12 en S2, 11 en F1, 8 en P4, 8 en G1, 13 en S3.

**Les cibles nommées d'avance pour F4** — chacune est une phrase qu'une tâche de
cette branche rend fausse :

| Place | Ce qui devient faux |
| --- | --- |
| `agent/src/pont/table.rs:24-28` | « **c'est F4 qui donnera de quoi les juger** » — F4 l'a donné, ou ne l'a pas donné ; dans les deux cas la phrase au futur est à reprendre |
| `proto/src/fichiers.rs:43-46` | « c'est le sous-bloc **F4** (le banc de latence) qui donnera de quoi la juger » — idem pour `TAILLE_TRAME_MAX`, ⚠️ **et le §0.1 lui ajoute un fait : ce n'est pas elle qui borne un listage** |
| `agent/src/pont/lecture.rs:38-42` | « **C'est F4 qui jugera** » (le facteur ~120) — et §0.6 dit que F4 ne le juge pas, il le rend caduc |
| `agent/src/pont/journal.rs:56` | la liste des constantes non calibrées, si F4 en calibre |
| `agent/src/pont/enumeration.rs:26-37` | « le critère ROUGE de **F5** sera par construction rouge tant que F5 n'existe pas » — à relire, pas à changer |
| `agent/src/pont/projfs/etat.rs:307-323` | « la mesure de fond appartient à **F5** » — F4 mesure autre chose ; vérifier que la phrase reste vraie |
| spec §8 F4, l. 1114-1150 | **trois lignes de sa table** sont remplacées ou retirées sur mesure (D1, D2, D3). **Annoter, jamais réécrire** : c'est un relevé daté |

⚠️ **Le TRI compte autant que les corrections** (leçon de S4, qui a trouvé 17
places dont 3 justes et 9 parlant d'autre chose) : **énumérer par `grep -n`
AVANT d'éditer, corriger place par place PAR NUMÉRO DE LIGNE, relire chaque
place APRÈS**. *Une substitution qui ne dit pas combien d'occurrences elle a
touchées est une affirmation de complétude non vérifiée.*

🔴 **Et une citation `fichier:ligne` peut être rendue fausse par la branche
elle-même** : P3 l'a payé — la ligne citée avait été déplacée par un commit du
même sous-bloc. **Relire les citations APRÈS avoir exécuté ce qui les déplace**,
pas avant. `table.rs` et `service.rs` sont modifiés par F4 : **toute citation
vers eux est à revérifier en dernier**.

**② Le document de résultats**,
`docs/superpowers/plans/2026-08-21-pont-fichiers-f4-resultats.md`, **permanent
et commité**.

🔴 **La preuve d'une affirmation ne doit JAMAIS vivre dans un rapport
gitignoré.** L'espace de travail de D9 (`.superpowers/sdd/`) **a disparu**, et
**six** constats de revue avec lui, définitivement — établi par la commande en
D10. Toute pièce sur laquelle une phrase de F4 s'appuie est versée sous
`journaux-pont-fichiers-f4/`.

**③ `CLAUDE.md`** : une section « Sous-projet ③ Pont fichiers — sous-bloc F4 »,
les familles de lecture des journaux **mesurées** (`file`, `grep -lP '\x1b\['`,
un balayage d'octets NUL — ⚠️ **`grep -qa $'\000'` cherche la chaîne vide et
matche tout**, piège relevé par F2 ; le contrôle se fait en Python), la variable
`PONT_MESURE` au tableau des variables, **et le balayage des tailles PAR LA
COMMANDE, APRÈS la dernière édition de la ronde, revue transverse comprise** —
« une table relevée en début de ronde serait fausse à la fin de la même ronde »,
erreur commise par D8 en croyant bien faire.

⚠️ **Et deux corrections que F4 hérite sans en être l'auteur** : le tableau de
dette de `CLAUDE.md` porte **quatre** lignes et il en a **deux** (§1.1). La
tâche 16 les barre **en nommant qui les a résorbées** (G1/G3, d'après F3), et
**ne se les attribue pas**.

---

## 5. Ordre, dépendances, et ce qui peut aller de front

```
1 ─┬─ 2 ──────────────────────────────┐
   ├─ 4 ── 5 ──┐                      │
   └─ 6 ───────┴─ 3(P0) ─ 7 ─ 8 ─ 9 ─ 10 ─ 11 ─ 12 ─ 13 ─ 14 ─ 15 ─ 16
```

| Tâches | Régime |
| --- | --- |
| **1** | d'abord, seule |
| **2, 4, 6** | **de front** — aucune ne touche la VM, aucune ne partage un fichier |
| **5** | après 4 (elle câble `latence.rs`) |
| **3 (porte P0)** | après 5 et 6 — elle a besoin de l'instrument, pas de la mesure |
| **7 → 15** | 🔴 **STRICTEMENT SÉQUENTIELLES** |
| **16** | en dernier, après la **dernière** édition de code |

🔴 **POURQUOI 7 À 15 NE VONT JAMAIS DE FRONT.** La VM est une ressource
**exclusive** : une racine ProjFS, un binaire, un `agent.log`. F1 a perdu une
exécution entière parce que deux se sont recouvertes de 2 min 23 s — *le journal
versé sous le nom de la première était celui de la seconde*, et l'échec de la
seconde s'expliquait par la session de signaling que la première tenait sur le
même préfixe. **Deux exécutions ne se chevauchent jamais**, et `jouer-f4.sh` tue
avant **et recompte après, y compris si l'exécution a échoué**.

⚠️ **Le contrôle d'anti-chevauchement porte sur le PILOTE, pas sur le script** :
`pgrep -f jouer-f2.sh` matchait le shell qui le lançait (piège maison, payé une
fois de plus en F2).

⚠️ **La tâche 3 est une PORTE, pas une étape.** Son résultat (`N_max`, `N_mur`,
la forme de l'échec) est une **entrée** des tâches 9, 13 et 14. Elle échoue →
le §0.1 est réfuté, et les rangs se re-décident **avant** de continuer.

**Récapitulatif des rouges, une par ligne, chacune jouée SEULE :**

| # | Tâche | Mutation | Ce qui doit tomber |
| --- | --- | --- | --- |
| R1 | 4 | `Famille::Lire` prend le rang de `Famille::Lister` | 2 tests, et eux seuls |
| R2 | 4 | une borne de seau `<` → `<=` | `une_traversee_tombe_dans_le_seau_qui_la_contient` |
| R3 | 7 | *(aucune)* — le bras **sans** `PONT_MESURE` | 0 ligne d'armement, 0 ligne `traversees` |
| R4 | 10 | `PRJ_FLAG_USE_NEGATIVE_PATH_CACHE` retiré, **par numéro de ligne** | le différentiel du cache négatif s'effondre |
| R5 | 12 | *(aucune)* — le bras **sans** neutraliser `move` | `renommage par copie` absent du journal de page |
| R6 | 2 | la ligne de `run-agent.sh` retirée | `grep` sur `run-agent.ps1` rend 0 |

⚠️ **R3 et R5 ne mutent RIEN, et c'est ce qui en fait les meilleures** : elles
opposent deux bras du même binaire. **R4 mute du code de PRODUIT sur la VM** et
exige donc un rebâtissage, la taille des deux binaires versée, et un binaire
témoin **qui s'identifie lui-même** (legs n°6 du chantier E).

---

## 6. Divergences relevées entre la spec, F1/F2/F3 et le CODE RÉEL

Chacune est **tranchée dans ce plan**, et chacune porte le relevé qui la fonde.

| # | Divergence | Tranchée |
| --- | --- | --- |
| **E1** | La spec demande un rang « **10 000 entrées** ». **Mesuré** : 85 o par entrée, donc **830 Kio d'en-tête**, contre `LOCAL_MAX_MESSAGE_SIZE = 256 * 1024` (str0m 0.21.0, `sctp/mod.rs:33`). Le mur analytique est à **≈ 3 080 entrées** | **D1** : rangs **dérivés** de la porte P0, et le rang qui échoue **est** une mesure |
| **E2** | La spec demande le « **cache d'énumération chaud** ». **Mesuré** : `TTL_ENUMERATION` n'existe nulle part (une occurrence, un commentaire disant qu'il n'est pas livré) ; `Rafraichir` est un livrable de **F5** | **D2** : on mesure le chaud que le produit A (substituts posés, cache négatif), et l'asymétrie de conclusion est écrite d'avance |
| **E3** | La spec demande un « **taux d'occupation** du cache négatif ». Un succès du cache **n'atteint jamais le fournisseur** : le dénominateur est invisible | **D3** : le **différentiel** 1ʳᵉ/2ᵉ ouverture, **plus un témoin** qui retire le drapeau |
| **E4** | La spec demande le « coût du **repli de renommage par copie** ». **Relevé** : aucune de ses lignes n'a jamais couru (F3) ; `move()` existe sur un fichier OPFS ; et ProjFS **refuse** le renommage d'un répertoire avant de consulter le fournisseur | **D4** : deux moitiés de statut différent — fichier **forcé** sur le chemin réel, répertoire **hors produit** |
| **E5** | La spec §5.3 pose **trois** budgets. **Le code en a CINQ** : `DELAI_ECRIRE = 30 s` (F2) et `DELAI_MUTATION = 15 s` (F3) s'y sont ajoutés, tous deux déclarés comme divergences par leurs plans | F4 les mesure **tous les cinq** quand ses gestes les traversent, et le rapport dit lesquels ont été exercés |
| **E6** | La spec §11 : « **F4 donnera de quoi en calibrer trois** (les délais) ». ⚠️ Donner de quoi calibrer **n'est pas calibrer** — et un jugement d'usage n'a jamais été porté sur aucune constante de ce dépôt | F4 publie **des distributions**, jamais des valeurs proposées (tâche 15) |
| **E7** | Le plan de F1 annonce « **F4 (le banc de latence `PONT_MESURE`)** ». **Relevé** : `PONT_MESURE` n'existe nulle part dans le code | Créée en tâche 2, convention `=1` **ARME** (§3.1), **inverse** des trois autres `PONT_*` |
| **E8** | La spec §7.3 : « le pont ne demande pas le morceau *n+1* tant que le canal a plus de `SEUIL_TAMPON` octets en attente ». **Relevé** : `SEUIL_TAMPON = 64 Kio` vit **côté navigateur** (`client/src/fichiers/flux.ts:58`), et le pont ne le voit pas — c'est `MORCEAUX_EN_VOL = 4` (`agent/src/pont/lecture.rs:58`) qui borne son avance | F4 mesure **`en_vol_max`**, le seul des deux qui soit observable au pont (tâche 11) |
| **E9** | La spec §8 F4 attend « un banc, piloté par variable d'environnement ». **Le montage de F1/F2/F3 lance `SUPERVISEUR=1`**, donc capture, encodeurs et vidéo — autant de confondeurs pour une latence | **D7** : **M1** (pont seul, `PONT=1` + `SESSION_ID=<préfixe>:fichiers`) est la référence ; **M2** n'a que deux points |
| **E10** | Le cadrage parle de « la latence de listage **et de lecture de ProjFS** ». Le pont ne peut mesurer que la traversée **pont → navigateur → pont** ; ce que l'application attend inclut ProjFS, le rappel, la table et le balayage | **D5** : deux instruments, l'arbitre au mur-à-mur, et le **résidu** par soustraction — **nommé, pas mesuré** |
| **E11** | Le legs 5 de F1 attend une **explication** du facteur ~120. **Relevé** : les deux nombres viennent de deux chemins et deux états différents, l'un n'est pas un chronomètre, et l'attribution par les journaux d'archives est **impossible** — la trace qui aurait tranché a une période de **60 s** pour une lecture de **1 937 ms** | **D6** : F4 rend le facteur **caduc** par une mesure propre et répétée dont l'état est déclaré ; il ne l'explique pas |
| **E12** | La spec §8 F4 n'annonce **aucun** livrable de code produit | F4 en livre **un module pur, une ligne de journal, une variable** — §2.1 — et **ne touche ni `client/src/` ni `proto/`** |

⚠️ **E1, E2, E3 et E4 modifient la table de la spec §8 F4. Elles s'y annotent à
la tâche 16, jamais ne la réécrivent** : c'est un relevé daté du 19 août 2026,
et le barrer le rendrait faux comme histoire.

---

## 7. Ce que F4 n'établira PAS

Écrit d'avance, pour qu'aucun document de résultats n'ait à le découvrir.

- **Aucun taux, nulle part.** Deux exécutions par mesure au mieux ; une par
  rouge. **Deux exécutions ne font pas une fréquence.**
- ⛔ **Aucun jugement d'usage.** Personne n'aura dit si le lecteur est
  *agréable*. F4 mesure des latences, **pas une expérience** — même lacune que
  `BPP_MIN` traîne depuis le chantier C volet 1, et qu'aucun chantier de ce
  dépôt n'a jamais levée.
- ⛔ **Aucune constante ne sera CALIBRÉE.** F4 donne des distributions pour trois
  budgets (E6). Choisir un nombre demande un jugement d'usage qu'il ne porte
  pas. `TAILLE_TRAME_MAX`, `SEUIL_TAMPON`, `MORCEAUX_EN_VOL`,
  `PERIODE_RECENSEMENT`, `PERIODE_HYDRATATION`, `PERIODE_BALAYAGE`,
  `TAILLE_MAX_FICHIER` (non implémentée), **et les treize seaux de `latence.rs`
  que F4 lui-même introduit** restent non jugés.
- ⛔ **Le sélecteur de fichiers Windows n'est ni ouvert ni mesuré.**
  `IFileOpenDialog` bloque en attendant un utilisateur, et le piloter demanderait
  `Xvfb` + `xdotool` — **consentement donné en D8, jamais suivi d'effet**, et
  outils relevés **absents** de l'hôte. `Get-ChildItem` + `GetAttributes` +
  `explorer.exe` en sont des **approximations**, et elles omettent l'extraction
  d'icônes et les vignettes.
- ⛔ **Le réexamen de la décision « aucune interception en v1 » n'appartient pas
  à F4** (spec §8 F4, §11). Et la voie par hook d'API est **écartée
  définitivement** par l'amendement.
- ⛔ **Le résidu ProjFS n'est pas mesuré** : c'est une soustraction entre deux
  horloges sur deux machines, rapportée en **ordre de grandeur** (§0.5).
- ⛔ **La cause du mur de listage ne sera pas établie** si l'observation ne la
  donne pas. F4 mesure **où** il est et **sous quelle forme**, pas **pourquoi**
  — et il ne livre **aucune parade** (§0.1).
- ⛔ **R5 n'est pas mesuré isolément** : le delta M2 − M1 ne départage pas la
  contention réseau de la contention CPU (§tâche 14).
- ⛔ **Le legs n°3 de F3 — « aucun éditeur réel n'a exercé l'idiome fichier
  temporaire + renommage »** — reste **entier**. F4 mesure des durées ; il
  n'ouvre ni LibreOffice ni Word. *C'est « le seul chemin par lequel une
  sauvegarde peut se perdre en silence », et il traverse F4 sans être touché.*
- ⛔ **Le legs n°1 de F2 — la fenêtre de trente secondes** — n'est ni corrigé ni
  mesuré de nouveau. ⚠️ **Il peut MORDRE PENDANT F4** : une écriture poussée
  entre l'ouverture du canal et l'installation de l'écrivain attend
  `DELAI_ECRIRE`, et le compteur affiche `dues: 0` pendant ce temps. **F4 n'écrit
  pas**, sauf en tâche 12 (le repli) : si un chiffre de cette tâche porte 30 s,
  **c'est cette fenêtre, pas une latence**.
- ⛔ **Le legs n°4 de F1 — « des lectures calent sans jamais expirer »** — n'est
  pas refermé. F4 a l'instrument (`plus_ancienne_ms`, le tableau à quatre lignes
  de `table.rs::plus_ancienne`) ; **il n'a pas le symptôme**, qui ne s'est plus
  manifesté depuis. *L'absence d'un symptôme sur deux exécutions n'est pas sa
  disparition.*
- ⛔ **`showDirectoryPicker()` n'est toujours jamais appelé**, ni le modèle de
  permission (`queryPermission` / `requestPermission`), ni le mode `readwrite`.
  L'instrument est OPFS, **qui n'a aucun modèle de permission** — et qui est
  **sensible à la casse** là où un disque local ne l'est pas (sonde S2 de F3).
- ⛔ **Rien d'un client réel** : Chrome sans interface, décodage logiciel, sur
  l'hôte qui porte la VM. **Un seul navigateur** ; la File System Access API
  n'existe ni sur Firefox ni sur Safari — limite du **produit**, héritée du
  cadrage.
- ⛔ **Rien de plusieurs utilisateurs**, rien à travers une reconnexion WebRTC,
  **aucun relais TURN pour le pont** (divergence assumée depuis F1).
- ⛔ **Aucun test d'hôte ne couvrira `pont/projfs.rs` ni `pont/service.rs`.**
  `cargo check --target x86_64-pc-windows-gnu` en vérifie types, emprunts et
  durées de vie — **jamais le comportement**. C'est pourquoi le contrôle de
  `PONT_MESURE` est une rouge sur la VM (tâche 7), et non un test.
- ⛔ **R7 reste ouvert** : cinq des treize entrées ProjFS n'ont aucun jumeau
  `PRJ_*_CB`, et un mauvais `transmute` de fonction est un défaut que rien
  n'attrape avant l'exécution. F4 n'y touche pas.
- ⛔ **L'occupation disque de la racine (R4)** n'est pas mesurée : c'est F5.
  ⚠️ **Et F4 la fait CROÎTRE** — chaque rang de lecture hydrate son fichier.
  **Le gabarit 100 Mio laisse 100 Mio sur le disque de la VM**, et la purge
  entre exécutions est ce qui l'empêche de s'accumuler. **Relever l'espace libre
  avant la campagne.**

---

## 8. Risques — et ce qui rendrait F4 NON LIVRABLE

| # | Risque | Ce qu'on en sait, et la parade |
| --- | --- | --- |
| **R-F4-1** | 🔴 **La VM est tenue par un chantier voisin pendant toute la fenêtre de travail** | **C'est le risque n°1, et il est déjà réalisé au moment où ce plan est écrit** (P3, VM en cours d'exécution). Parade : préalable **externe** (§1.2) — relever, et **s'arrêter** plutôt qu'écraser. **Ne rend pas F4 non livrable ; le diffère** |
| **R-F4-2** | 🔴 **Le montage M1 ne monte pas** (pont seul sans superviseur) | **Éliminatoire pour M1, pas pour F4** : tout bascule sur M2, avec ses confondeurs **déclarés**. Éprouvé en tête de la porte P0, avant toute mesure |
| **R-F4-3** | 🔴 **Le mur du listage tombe si bas que les rangs de la spec perdent leur sens** (< 100 entrées) | **Ce serait un résultat, pas un échec** — et un résultat lourd : le produit ne servirait aucun dossier réel. Le rapport le porterait comme un **défaut de produit** (tâche 15, issue 3) |
| **R-F4-4** | **Le rang 100 Mio est hors de portée en temps** | Prévu : règle d'admission écrite d'avance, dérivée du débit mesuré à 1 Mio, avec **l'extrapolation publiée** et un rang de repli (tâche 11) |
| **R-F4-5** | 🔴 **L'instrument détruit ce qu'il mesure** — quatre formes connues dans ce dépôt (§1.5 n°5) | Parades **nommées et non négociables** : histogramme au lieu de trace par unité ; aucun `execFileSync` ; aucune capture CDP ; aucun parcours de la racine ; `agent::pont=debug` **borné aux petits rangs** (tâche 11) |
| **R-F4-6** | **Les chiffres sont trop dispersés pour conclure** | **« Ne tranche rien » est une issue prévue** (spec §8 F4). Ce qui serait fautif est un verdict sans son nombre d'exécutions |
| **R-F4-7** | **La mutation de produit de la tâche 10 (drapeau du cache négatif) laisse un binaire mutant sur la VM** | Harnais du §1.4 : copie nommée, restauration vérifiée par `sha256sum`, **second rebâtissage**, **taille des deux binaires versée**, binaire témoin qui **s'identifie lui-même** |
| **R-F4-8** | **La VM s'hiberne d'elle-même en pleine campagne** — déclencheur toujours non identifié, rencontré **deux fois** dans F3 | Symptôme trompeur : `run-agent.ps1: Aucun fichier ou dossier de ce nom`, *qui se lit comme une panne du script*. Parade : `virsh list --all` après **chaque** séquence longue, et une exécution interrompue est **rejouée**, jamais rapportée |
| **R-F4-9** | **Un journal versé n'est pas celui de son exécution** | Trois formes payées : chevauchement (F1), superviseur survivant tenant `agent.log` (D8), artefact d'une tentative antérieure relu (F3). Parade : `jouer-f4.sh` efface **tous** les artefacts avant, et le contrôle d'anti-chevauchement porte sur le **pilote** |
| **R-F4-10** | 🔴 **Les tâches 3 et 10 mutent ou éprouvent des états où le produit peut FIGER** — un `Get-ChildItem` non borné, un geste dont le produit ne borne pas la durée | **`Start-Job` + `Wait-Job` rend un INTERBLOCAGE sous tâche planifiée** (F3). La seule borne est celle du produit. Parade : **le geste non borné est le DERNIER de sa famille, après son point de reprise**, et le relevé partiel se rapporte **en le disant** |

**Ce qui rendrait F4 NON LIVRABLE** — et c'est court, parce que F4 mesure :

1. 🔴 **La VM reste indisponible pendant toute la fenêtre de travail.** Toutes
   les mesures de F4 sauf deux tâches (4, 6) la traversent. **Aucun repli** : ni
   la latence de ProjFS ni le comportement du shell Windows n'ont d'équivalent
   sur l'hôte. **C'est le seul risque qui supprime le sous-bloc** ; tous les
   autres bornent, dégradent ou reportent.
2. ⚠️ **Un second cas, moins net et qu'il faut nommer** : si la porte P0 montre
   que le listage **tronque en silence** (§tâche 3, troisième issue), F4 reste
   livrable — il aura mesuré un défaut de produit — **mais toutes ses mesures
   d'énumération deviennent des mesures d'un chemin faux**, et le rapport doit
   les présenter ainsi plutôt que comme des latences.

⚠️ **Ce qui NE rend PAS F4 non livrable, et qu'on pourrait croire tel** :
« aucune conclusion sur la question du cadrage ». La spec l'écrit noir sur
blanc — *« "Ne tranche rien" est une issue acceptable et prévue »*.

---

## 9. Résumé en une phrase

**F4 pose deux instruments — le chronomètre d'un geste réel dans la VM, qui
arbitre, et un histogramme pur dans le pont, qui décompose —, mesure avec eux
l'énumération, les attributs, la lecture et le repli de renommage à des rangs
DÉRIVÉS d'une porte éliminatoire plutôt que supposés, et répond à la question
que le cadrage pose depuis le 28 juillet 2026 — ou déclare, avec son nombre
d'exécutions, qu'il ne le peut pas.**

---

## 10. Ce que F4 léguera, quoi qu'il mesure

Écrit d'avance : ces legs ne dépendent d'aucun résultat.

1. ⛔ **F5** — `Rafraichir` et le cache d'énumération, **sans lequel la moitié
   « chaud » de la table de la spec n'a pas d'objet** (§0.2).
2. ⛔ **F5** — l'occupation disque et la politique d'éviction (R4), que F4 fait
   croître sans la mesurer.
3. ⛔ **Sans destinataire** — **la parade au mur de listage**, si la porte P0 le
   confirme. C'est un changement de **forme** du protocole, donc un incrément de
   `FICHIERS_VERSION`, donc une décision qui n'appartient pas à un sous-bloc de
   mesure.
4. ⛔ **Sans destinataire** — **l'idiome fichier temporaire + renommage sur un
   éditeur réel** (legs n°3 de F3), « la lacune la plus lourde » de ce
   sous-projet, que F4 ne touche pas.
5. ⛔ **Sans destinataire** — **la fenêtre de trente secondes de F2**, dont le
   remède est nommé (« que le pont n'ouvre son canal d'écriture qu'après un
   acquittement de l'écrivain ») et non appliqué.
6. ⛔ **Sans destinataire** — **`showDirectoryPicker()`, le modèle de permission
   et le mode `readwrite`**, qui exigent `Xvfb` + `xdotool`. ⚠️ **Et les mesures
   qui en sortiraient ne se compareraient à AUCUNE campagne antérieure.**
7. ⛔ **Sans destinataire** — **la calibration** des cinq budgets, des seaux de
   `latence.rs`, et de tout le reste : F4 donne des distributions, **le jugement
   d'usage reste à porter**.
8. ⛔ **Un autre chantier** — **le réexamen du sélecteur de fichiers**, que F4
   alimente et ne conduit pas, et dont la seule voie ouverte est la détection
   par classe de fenêtre `#32770`.
