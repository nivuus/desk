# Sous-projet ③ Pont fichiers — sous-bloc F4 : résultats

**Date** : 21 août 2026
**Plan** : `docs/superpowers/plans/2026-08-21-pont-fichiers-f4.md` (`6608005`)
**Spécification** : `docs/superpowers/specs/2026-08-19-pont-fichiers-design.md`, §8 « F4 »
**Journaux et instrument** : `docs/superpowers/plans/journaux-pont-fichiers-f4/`

---

## 0. En une phrase

**Le pont fonctionne, et il est lent d'un facteur qui le rend inutilisable au-delà
de quelques centaines de kilo-octets** : son canal soutient **30 à 33 Kio/s**,
une lecture de plus de **128 Kio échoue**, un listage de plus de **~3 150
entrées échoue**, et les deux échecs sont **muets pour l'application, qui ne voit
qu'« une erreur interne » après vingt secondes de gel**. La cause du plafond de
débit est **établie par mutation** : la boucle de `pont::transport` lit un
datagramme par tour et dort jusqu'à `ATTENTE_MAX` avant chaque lecture.

---

## 1. Les familles de lecture des journaux — MESURÉES

Relevé par un balayage Python (`familles-de-lecture.txt`), **jamais par
`grep -qa $'\000'`, qui cherche la chaîne vide et matche tout** (piège relevé
par F2). **Trois familles, dont une seule demande un `sed`** :

| Famille | Fichiers | Ce qu'il faut faire |
| --- | --- | --- |
| tous les `*.json`, `*.log` d'analyse, `*-trace.txt`, `*.txt` | **70** | rien — se `grep`ent à plat |
| les `agent-*-plat.log` | **24** | rien (CRLF, sans conséquence) |
| les `agent-*.log` BRUTS | **22** | ⚠️ `sed 's/\x1b\[[0-9;]*m//g'` — ou lire le jumeau `-plat`, versé pour chacun |

✅ **AUCUN octet NUL dans aucun fichier** : `grep -a` n'est requis nulle part,
contrairement à F1 et D10. La raison est structurelle — les journaux de pilote
sont des sorties `node` sur l'HÔTE, jamais du PowerShell distant.

---

## 2. Le contrôle d'entrée (tâche 1)

`t1-controle-entree.txt`. Les cinq comptes de référence **relancés, pas
recopiés** : 916 / 109 / 466 / 296 / **22 avertissements tous `dead_code`**,
identiques à ceux du plan. `HEAD` valait `ae74ab9` et non `4cdab11`, mais les
trois commits d'écart ne touchent que `docs/` et `CLAUDE.md`.

**Les trois faits du §0 relancés et confirmés** : `TTL_ENUMERATION` n'a **une
seule occurrence** dans tout le dépôt et c'est le commentaire qui dit qu'il
n'est pas livré (**D2 tient**) ; `PONT_MESURE` n'existe **nulle part** (**E7
confirmée**) ; `LOCAL_MAX_MESSAGE_SIZE = 256 * 1024` (**fait 3 du §0.1 tient**).

⚠️ **Le tableau de dette de `CLAUDE.md` porte QUATRE lignes et l'arbre en a
DEUX** — `proto/src/plateforme/tests.rs` vaut **340** (publié 561) et
`proto/ts/plateforme.test.ts` **462** (publié 512). **Ce n'est ni le fait de F3
ni celui de F4** ; corrigé à la tâche 16, sans se l'attribuer.

---

## 2bis. 🔴 Le nombre d'exécutions — et un compte que J'AI PUBLIÉ FAUX

**VINGT-DEUX exécutions d'agent**, comptées par `ls pilote-*.json | wc -l` :
dix-neuf en M1, trois en M2. Chacune a son `pilote-<étiquette>.json`, son
`agent-<étiquette>.log` brut, son jumeau `-plat`, son `pilote-<étiquette>.log`
et sa trace de mesure.

> 🔴 **Le message du commit `d380bdd` annonce « TREIZE exécutions ». C'est
> FAUX.** Je l'avais écrit **de mémoire** au lieu de le compter — exactement le
> patron que D11 a inscrit dans `CLAUDE.md` (*« un message de commit est une
> pièce du dépôt, et personne ne le relit »*), commis par la ronde qui le cite,
> et sur le chiffre même que la discipline d'énoncé d'un chantier de mesure
> exige de porter dans chaque phrase. **Le commit n'est pas amendé** — l'arbre
> est partagé et un voisin committe —, et la correction vit ici, avec la
> commande qui l'établit. *Un compte se compte ; il ne se rappelle pas.*

---

## 3. 🔵 M1 est viable — le pont tourne SEUL

**C'est la première chose que ce sous-bloc établit, et elle conditionne tout le
reste.** Un agent lancé avec `PONT=1` et `SESSION_ID=<préfixe>:fichiers`,
**sans `SUPERVISEUR`**, monte sa racine et sert le navigateur **sans qu'aucune
ligne de client ne change** :

```
INFO agent::pont: racine du pont fichiers montée racine=C:\Users\Administrateur\Mes Fichiers
INFO agent::pont::service: canal du pont ouvert : le navigateur peut servir les requêtes
```

Toutes les mesures qui suivent sont donc prises **sans capteur, sans encodeur,
sans PeerConnection vidéo** — les confondeurs que F1, F2 et F3 portaient.

---

## 4. La rouge de câblage de `PONT_MESURE` (tâches 2 et 7)

**Trois bras, une exécution chacun, et le deuxième rend le troisième
discriminant.**

| Bras | `banc de latence` | `traversees` | Témoin |
| --- | --- | --- | --- |
| **sans** la variable | **0** | **0** | 🔵 **7 `codes rendus`** — la boucle de recensement TOURNE. Sans ce témoin, le zéro se lirait comme « le pont n'a pas tourné » |
| `=1`, aucun geste | 1 | **2, `n:0` PARTOUT** | c'est lui qui rend le suivant lisible |
| `=1`, des gestes | 1 | `lister=n:12`, `attributs=n:4` | |

**Les deux premiers temps du contrôle de la tâche 2** (la forme, et
l'atteignabilité de `${VAR:+…}`) ont été joués **par rendu du heredoc seul**,
sans toucher la VM. **R6** : la ligne retirée par numéro de ligne fait rendre
**0** au `grep` sur le `run-agent.ps1` rendu, la variable étant pourtant posée.

---

## 5. 🔴 LA PORTE P0 — le mur du listage

**La prédiction du §0.1 du plan tient dans TOUS ses termes.**

| Rang | Mur-à-mur | Entrées rendues | Issue |
| --- | --- | --- | --- |
| 10 | 79–126 ms | 10 | aboutit |
| 100 | 629–699 ms | 100 | aboutit |
| 1 000 | 5 961–6 196 ms | 1 000 | aboutit |
| 2 000 | 13 035 ms | 2 000 | aboutit |
| 3 000 | 18 652 / 18 670 ms | 3 000 | aboutit, **2 exécutions** |
| **3 150** | **18 189 / 22 013 / 23 843 ms** | **3 150** | **aboutit — `N_max`** |
| **3 200** | 20 086–20 132 ms | — | **ÉCHOUE — `N_mur`** |
| 4 000 / 6 000 / 10 000 | 20 117–20 159 ms | — | échouent |

🔵 **Le mur mesuré ENCADRE le mur calculé.** Le calcul du §0.1, relancé contre
l'**encodeur réel** (`encodeEntrees`) avec les noms de 18 caractères du
gabarit : **83,0 o par entrée**, et le **premier `N` dont l'en-tête dépasse
262 144 octets est 3 159**.

🔴 **LA FORME DE L'ÉCHEC EST CELLE QUI ÉTAIT PRÉDITE, ET ELLE EST ÉTABLIE PAR
PIÈCE, PAS PAR INFÉRENCE.** La console de la page-shell — capturée précisément
pour cela — porte à chaque échec :

```
warning  trame fichiers non traitée {message:Failed to execute 'send' on
'RTCDataChannel': Trying to send message larger than max-message-size, …}
```

Le `.catch()` de `client/src/fichiers/canal.ts:134` **ne répond rien** : la
commande `Lister` reste en vol jusqu'à `DELAI_LISTER` (**20 132 / 20 116 /
20 133 ms mesurés**), puis est soldée en `delai-depasse` au recensement. **Les
comptes concordent** : 2 échecs → 2 avertissements de console → `delai-depasse=2`.
L'application, elle, ne voit que « **Une erreur interne s'est produite.** ».

✅ **CE N'EST PAS UNE TRONCATURE SILENCIEUSE** — la troisième issue, la pire. Le
compte d'entrées est relevé à **chaque** rang, et tout rang qui aboutit rend
**son compte exact**.

⚠️ **Le témoin qui rend la porte discriminante** : le rang 10 aboutit. Un
montage où même 10 entrées échoueraient mesurerait une panne, pas un mur.

---

## 6. L'énumération, les attributs, l'Explorateur (tâches 9, 10, 13)

**Deux exécutions**, montage M1, repos de 25 s après chaque geste.

### 6.1 🔵 Le motif de l'Explorateur ne coûte PAS N interrogations

C'est **contraire à ce que la question du cadrage laisse attendre**, et c'est le
fait le plus rassurant du sous-bloc.

| Rang | listage nu | `\| ForEach { $_.Attributes }` | dont les N interrogations |
| --- | --- | --- | --- |
| 10 | 79,6 / 95,1 ms | 65,3 / 65,4 ms | **1,80 ms** |
| 100 | 699,3 / 659,6 ms | 613,0 / 707,7 ms | **0,08 ms** |
| 1 000 | 6 064,8 / 5 961,2 ms | 5 735,8 / 5 522,1 ms | **0,12 ms** |

**L'énumération porte déjà les attributs** : `$_.Attributes` sur mille entrées
coûte un dixième de milliseconde. Le couple « lister puis interroger chacun » ne
double pas le coût — **il ne l'augmente pas**.

### 6.2 Chaque `Get-ChildItem` produit DEUX commandes `Lister`

`lister=n:2` par geste, systématiquement, et le navigateur renvoie **la liste
entière** aux deux. **Le coût est doublé.** Fait relevé, mécanisme non expliqué.

### 6.3 Le résidu ProjFS est très faible — et il reste NOMMÉ

`mur-à-mur − Σ(traversées)` : **0,3 % à 0,8 %** dès le rang 100, **16,6 %** au
rang 10 (coût fixe). En absolu : **4 à 65 ms**. Presque tout le temps est dans
la traversée pont → navigateur → pont.

⚠️ **C'est une soustraction entre deux grandeurs prises par deux horloges sur
deux machines. Elle se rapporte en ORDRE DE GRANDEUR, jamais comme une latence.**

### 6.4 🔴 `GetPlaceholderInfo` : froid contre chaud

| | Mur-à-mur | Traversées |
| --- | --- | --- |
| **froid** (entrée jamais touchée) | 75,4 / 88,5 ms | `attributs=n:3` |
| **chaud** (même entrée, substitut posé) | **0,4 ms** | **AUCUNE** |

⚠️ **Le « chaud » mesuré est celui que le produit A** — *les substituts déjà
posés*. **PAS** le cache d'énumération, **qui n'existe pas** (D2).

### 6.5 ⛔ Le cache négatif : le différentiel est NUL, et le témoin l'établit

`introuvable` et `chemin-introuvable` valent **ZÉRO** à la 1ʳᵉ ouverture de
l'Explorateur, à la 2ᵉ, aux deux exécutions vertes — **et au bras rouge R4**,
où `PRJ_FLAG_USE_NEGATIVE_PATH_CACHE` est retiré.

🔵 **Le plan (§0.3) prévoyait DEUX lectures de ce zéro ; le témoin en ÉCARTE
une.** Le drapeau était bien appliqué au vert et ne l'est plus au rouge — **le
binaire témoin le DIT lui-même dans le journal** (`BINAIRE TEMOIN R4 (F4) :
PRJ_FLAG_USE_NEGATIVE_PATH_CACHE RETIRE`) — et le résultat est identique. Reste
donc : **sur ce montage, l'Explorateur ne fait parvenir AUCUN sondage de chemin
absent au fournisseur.**

🔵 **Et ce zéro est LISIBLE parce qu'un contrôle le rend discriminant** : un
`Get-Item` sur un chemin **absent** rend `introuvable=1`. **Le compteur sait
compter.** Sans ce geste, le zéro aurait été indiscernable d'un compteur mort.

---

## 7. 🔴 LE PLAFOND DE LECTURE — le fait le plus lourd de F4

### 7.1 Le débit est plat à 30–33 Kio/s, et c'est la TAILLE du morceau

| Fichier | Morceaux | Exéc. 1 | Exéc. 2 | Débit |
| --- | --- | --- | --- | --- |
| 4 Kio | 1 | 189,9 ms | 233,8 ms | 17–21 Kio/s |
| 64 Kio | 1 | 2 012,3 ms | 2 075,8 ms | **30,8–31,8 Kio/s** |
| 128 Kio | 2 | 4 044,3 ms | 3 927,3 ms | 32,6 Kio/s |
| **256 Kio** | **4** | **ÉCHOUE** | **ÉCHOUE** | — |
| **1 Mio** | **16** | **ÉCHOUE**, `delai-depasse=16` | **ÉCHOUE** | — |

🔴 **La concurrence n'est PAS en cause** : **un seul** morceau de 64 Kio prend
déjà **1,97 s**. C'est la taille du morceau, et le débit est **linéaire**.

### 7.2 🔵 L'attribution est établie PAR MUTATION, pas par lecture

La boucle de `agent/src/pont/transport.rs` lit **un datagramme par tour**
(`socket.recv_from`, non bloquant) et **bloque jusqu'à `ATTENTE_MAX = 20 ms`**
dans `sortant.recv_timeout` **avant chaque lecture**. Arithmétique : 1 200 o par
tour / 35 ms ≈ 33 Kio/s.

**Mutation `ATTENTE_MAX` 20 ms → 1 ms** (binaire qui **s'identifie lui-même**
dans son journal), une exécution :

| Fichier | 20 ms (livré) | 1 ms (diag) | Rapport |
| --- | --- | --- | --- |
| 4 Kio | 189,9 ms | 101,1 ms | 1,9 |
| 64 Kio | 2 012,3 ms | 1 024,7 ms | 2,0 |
| 128 Kio | 4 044,3 ms | 1 966,6 ms | 2,1 |
| **256 Kio** | **ÉCHOUE** | **4 009,9 ms — ABOUTIT** | — |

**Le débit DOUBLE, et le rang qui échouait passe.**

⚠️ **MAIS LE FACTEUR EST 2, PAS 20.** À 1 ms le plafond est ~64 Kio/s.
`ATTENTE_MAX` est **le terme dominant, pas le seul** : le plafond vient de la
structure « un datagramme par tour de boucle », dont le coût fixe du tour domine
une fois l'attente réduite. **F4 mesure ; il ne corrige pas** — la parade est un
changement de conception de la boucle, pas un réglage.

### 7.3 🔴 `MORCEAUX_EN_VOL = 4` n'est JAMAIS atteint sur le binaire livré

`en_vol_max` (livré par F3, dont le legs n°2 dit « un maximum resté à 1 pendant
une lecture de 12 Mio est un échec du livrable ») :

- **1** sur une lecture d'un morceau — attendu ;
- **2** sur une lecture de deux morceaux — **la fenêtre s'ouvre bien** ;
- **jamais 4**, parce que la seule lecture qui aurait quatre morceaux (256 Kio)
  **ÉCHOUE** : quatre morceaux concurrents se partagent 33 Kio/s, chacun dépasse
  alors `DELAI_LIRE = 5 s`, et ils expirent.
- **4** sur le binaire de diagnostic à `ATTENTE_MAX = 1 ms`, où 256 Kio aboutit.

🔴 **Le contrôle de flux que F3 livre n'a donc jamais eu l'occasion de servir en
exploitation.** Son legs n°2 est **fermé par la mesure**, et la réponse est que
le mécanisme fonctionne mais que le produit ne l'atteint pas.

### 7.4 🔵 SONDE A — l'hydratation ProjFS *EST* le cache de données

Première mesure de l'affirmation de la spec §6.4, **deux exécutions** :

| | Mur-à-mur | Traversées |
| --- | --- | --- |
| 1ʳᵉ lecture de 64 Kio | 2 143 / 2 076 ms | `lire=n:1` |
| **2ᵉ, 3ᵉ** | **0,5 / 0,6 ms** | **AUCUNE** |

**Le pont n'est PAS sur le chemin d'une relecture.**

### 7.5 ✅ Le facteur ~120 de F1 : expliqué, pas seulement rendu caduc

Le plan (D6) annonçait que F4 rendrait le facteur **caduc** sans l'expliquer.
**F4 fait mieux, et il faut dire de combien** : les deux moitiés du facteur sont
mesurées. La borne basse (52–55 Kio/s) est du même ordre que les **30–33 Kio/s**
mesurés proprement ; la borne haute (**6,5 Mio/s**) est **inatteignable à travers
le pont** sur ce montage — et une **relecture** rend, elle, **116 Mio/s**.
L'hypothèse résiduelle « la lecture de F1 n'a pas traversé le pont » devient
**cohérente avec deux mesures** au lieu d'une arithmétique.

⚠️ **Elle n'est pas ÉTABLIE pour autant** : les journaux d'archives de F1 ne
permettent pas de l'attribuer (§0.6 du plan — la trace qui aurait tranché a une
période de 60 s pour une lecture de 1 937 ms), et F4 n'a pas rejoué F1.

### 7.6 ⚠️ Le rang 100 Mio est ABANDONNÉ, et l'extrapolation est publiée

Règle d'admission du plan (tâche 11), écrite d'avance : au-delà de **dix
minutes** extrapolées, le rang est abandonné. À **30–33 Kio/s**, 100 Mio
prendraient **~52 minutes**. Et la question ne se pose de toute façon pas :
**1 Mio échoue, et même 256 Kio**. **Le rang maximal réellement lisible sur le
binaire livré est 128 Kio.**

---

## 8. Le repli de renommage par copie (tâche 12)

🔵 **IL A COURU POUR LA PREMIÈRE FOIS.** F3 l'a livré, et « aucune de ses lignes
n'a jamais couru » — sa trace est absente de tous les journaux versés par F3.

**Deux exécutions par bras.**

| Bras | Trace `renommage par copie` | 64 Kio | 1 Mio |
| --- | --- | --- | --- |
| **forcé** (`move` neutralisé par l'injection) | **2 par exécution** | 172,6 / 192,5 ms | 125,3 / 125,8 ms |
| **témoin R5** (`move` intact) | **0** | 159,2 / 161,5 ms | 125,6 / 125,8 ms |

La trace du produit dit elle-même pourquoi : « **repli LOCAL (zéro octet sur le
canal)** ». **Le coût du repli est NUL à ces rangs**, et les deux bras sont
indistinguables. Marge à `DELAI_MUTATION` (15 s) : **deux ordres de grandeur**.

⚠️ **C'est une mesure FORCÉE**, et ce n'est pas le chemin nominal.
⛔ **La moitié RÉPERTOIRE est hors du produit** : ProjFS refuse le renommage
d'un répertoire avant de consulter le fournisseur (F3). ⚠️ Elle n'a pas été
mesurée comme composant non plus — `FileSystemDirectoryHandle.prototype.move`
n'existe pas sur ce Chrome (l'injection ne neutralise que
`FileSystemFileHandle`).

---

## 9. 🔴 Le delta M2 − M1 va dans le sens INVERSE de R5 (tâche 14)

⚠️ **Une première tentative de M2 était DÉGÉNÉRÉE, et il faut le dire** :
`enfant lancé` = **0**, aucune fenêtre éligible sur la VM, donc aucune session
vidéo et **aucune contention** — le delta aurait été nul **par construction**,
et se serait lu comme « le pont ne concurrence pas la vidéo ». Un verbe
`application:` ouvre désormais un Bloc-notes avant la mesure.

| Point | M1 | M2 (avec fenêtre) | Verdict |
| --- | --- | --- | --- |
| lecture 64 Kio | 2 012 / 2 076 ms | **1 244 / 1 263 ms** | **delta NET, M2 ~1,6× PLUS RAPIDE** |
| listage 1 000 | 6 065 / 5 961 ms | 4 539 / 5 805 ms | **étendues qui SE RECOUVRENT — delta NON établi** |

⚠️ **Ce que le delta établit** : sur la **lecture**, une session vidéo
concurrente **ne dégrade pas** le pont — elle coïncide avec un pont plus rapide.
⚠️ **Ce qu'il n'établit PAS** : il **ne départage toujours ni le réseau ni le
CPU** (le plan le disait d'avance), et **la cause du sens inverse n'est pas
établie**. Une hypothèse plausible et **non éprouvée** : un processus multimédia
élève la résolution du timer système, ce qui raccourcit les attentes de la
boucle du pont — cohérent avec l'effet de `ATTENTE_MAX`, **et rien de plus**.
⚠️ **Confondeur déclaré** : `m2-vrai-2` avait **deux** fenêtres, `m2-vrai-1` une.

⚠️ **LE TÉMOIN VIDÉO EST NON MESURABLE** : `window.__pc` est absent de la page
d'application, exactement comme au critère ⑥ de F2. **Je le dis ; je ne le
remplace pas par un raisonnement.** Ce qui est établi est qu'une session existe
(`enfant lancé` = 1 puis 2, `fenêtre attachée au capteur` idem).

---

## 10. La conclusion sur la question du cadrage (tâche 15)

**La question, mot pour mot** : « le bypass ne se justifie que si le listage
d'un dossier volumineux **dégrade réellement l'expérience** — décision à
trancher sur mesure, pas sur intuition ».

### 🔴 L'issue est la TROISIÈME du plan, la plus lourde : au-delà de `N_mur`, le listage NE FONCTIONNE PAS

Ce n'est plus une question de confort. **Le rapport la porte comme un défaut de
produit**, avec sa cause, qui est établie :

- **jusqu'à ~100 entrées**, le listage est **immédiat** (< 0,7 s) ;
- **à 1 000 entrées**, il coûte **~6 s** — deux exécutions, reproduit à 2 % près.
  ⚠️ *Personne n'a dit si 6 s est acceptable : c'est un jugement d'usage, et F4
  n'en porte aucun* ;
- **à ~3 150 entrées**, il coûte **18 à 24 s** ;
- **au-delà de ~3 150 entrées, il ÉCHOUE**, et le mode d'échec est le pire
  possible : **vingt secondes de gel, puis « une erreur interne s'est
  produite »**, sans qu'aucune ligne de journal du côté application ne dise
  pourquoi.

**Ce qui, en revanche, ne dégrade rien** — et le cadrage l'attendait :
l'interrogation des attributs entrée par entrée (0,12 ms pour mille), les
sondages de chemins absents de l'Explorateur (aucun n'atteint le fournisseur),
et le résidu ProjFS (0,3 à 0,8 %).

### ⚠️ Les deux réserves qui bordent cette conclusion

1. **F4 mesure un produit SANS cache d'énumération** (`TTL_ENUMERATION` n'existe
   pas, `Rafraichir` est un livrable de F5). C'est donc une **borne HAUTE du
   coût**. Puisque F4 conclut que le listage **dégrade**, la conclusion est
   **CONDITIONNELLE** au sens du §0.2 : *F4 ne peut pas dire si le cache de F5 y
   remédierait*. ⚠️ **Mais un cache ne déplace PAS le mur** : celui-ci tient à la
   taille d'un message unique, pas à la répétition.
2. **Le sélecteur de fichiers Windows n'est ni ouvert ni mesuré.**
   `Get-ChildItem`, `GetAttributes` et `explorer.exe` en sont des
   **approximations**, et elles omettent l'extraction d'icônes et les vignettes.

### Ce que F4 alimente, et ce qu'il ne décide pas

**Le réexamen de « aucune interception du sélecteur en v1 » n'appartient pas à
F4** (spec §8 F4, §11), et la voie par hook d'API reste **écartée
définitivement** par l'amendement. Ce que F4 verse au réexamen est ci-dessus.

---

## 11. Ce que F4 N'ÉTABLIT PAS

- **Aucun taux, nulle part.** Deux exécutions par mesure au mieux, une par rouge.
- ⛔ **Aucun jugement d'usage** : personne n'a dit si le lecteur est *agréable*,
  ni si 6 s pour mille entrées est acceptable.
- ⛔ **Aucune constante calibrée.** F4 donne des distributions pour **cinq**
  budgets (et non trois : `DELAI_ECRIRE` et `DELAI_MUTATION` s'y sont ajoutés).
  `TAILLE_TRAME_MAX`, `SEUIL_TAMPON`, `MORCEAUX_EN_VOL`, `PERIODE_RECENSEMENT`,
  `PERIODE_HYDRATATION`, `PERIODE_BALAYAGE`, `ATTENTE_MAX`, **et les treize
  seaux de `pont::latence` que F4 introduit lui-même** restent non jugés.
- ⛔ **La cause du sens inverse du delta M2 − M1 n'est pas établie.**
- ⛔ **Le mécanisme des DEUX `Lister` par `Get-ChildItem` n'est pas expliqué.**
- ⛔ **Le coût de la canonicalisation de casse de F3 n'est PAS mesuré** : aucun
  geste de la campagne ne l'exerce. Les commentaires de `noms.ts` et
  `adaptateur.ts` qui l'annonçaient portent désormais ce constat.
- ⛔ **Le condensat SHA-256 de bout en bout** (legs n°6 de F1) n'est toujours pas
  établi. F4 ne relit aucun contenu.
- ⛔ **Le legs n°1 de F2 — la fenêtre de trente secondes — n'est ni corrigé ni
  mesuré.** Il n'a pas mordu : F4 n'écrit qu'en tâche 12, et aucun chiffre ne
  porte 30 s.
- ⛔ **Le legs n°4 de F1 — « des lectures calent sans jamais expirer »** — n'est
  pas refermé. F4 a l'instrument (`plus_ancienne_ms`) ; **il n'a pas rencontré le
  symptôme**, toutes ses lectures ayant soit abouti soit **expiré proprement**.
  *L'absence d'un symptôme sur douze exécutions n'est pas sa disparition.*
- ⛔ **Le legs n°3 de F3 — aucun éditeur réel n'a exercé « fichier temporaire +
  renommage »** — reste **entier**. F4 n'ouvre ni LibreOffice ni Word.
- ⛔ **`showDirectoryPicker()` n'est toujours jamais appelé**, ni le modèle de
  permission, ni le mode `readwrite`. L'instrument est OPFS.
- ⛔ **Rien d'un client réel** : Chrome sans interface, sur l'hôte qui porte la
  VM. Un seul navigateur.
- ⛔ **R7 reste ouvert** : cinq des treize entrées ProjFS n'ont aucun jumeau
  `PRJ_*_CB`.
- ⛔ **L'occupation disque (R4 de la spec) n'est pas mesurée**, et F4 la fait
  croître.

---

## 12. Les défauts d'instrument trouvés, et ce qu'ils enseignent

1. 🔴 **L'attente de l'hôte portait sur le COMPTE de gestes, satisfait AVANT le
   repos.** L'hôte reprenait la main, tuait Chrome, et le canal tombait **avant
   les deux recensements du repos** — c'est-à-dire avant la mesure elle-même.
   ⚠️ **Le symptôme n'avait rien d'une panne** : « 1 gestes en 5 s » et un code
   de sortie zéro. Remède : un marqueur `"fini":true` **posé après le dernier
   repos**.
2. 🔴 **Un résidu calculé contre une durée IMPOSÉE** (`explorer`, `application`)
   rendait **98 %** — un chiffre qui **se lirait comme du temps perdu dans
   ProjFS**.
3. 🔴 **Un résidu calculé sur des traversées CONCURRENTES** (`MORCEAUX_EN_VOL = 4`)
   rendait **−141 %**. La somme de traversées concurrentes n'est pas une durée
   écoulée. L'analyseur le refuse désormais nommément.
4. 🔴 **Le marqueur d'auto-identification du binaire de diagnostic était un
   `const &str` sans appelant : ÉLIMINÉ PAR LE COMPILATEUR**, absent du binaire,
   `grep` → 0. **Une constante morte n'identifie rien** ; il faut une référence
   vivante (un `tracing::warn!`).
5. 🔴 **LE CONTRÔLE PAR LA TAILLE DU BINAIRE A FAILLI RENDRE UN FAUX NÉGATIF,
   DANS LES DEUX SENS.** Le vert restauré pèse **exactement** autant que le rouge
   (10 708 480 o) sans être le même binaire, et deux compilations de la **même
   source** rendent **10 647 552** puis **10 708 480**. **La taille n'est ni
   nécessaire ni suffisante** ; ce qui tranche est une chaîne que l'on a
   soi-même posée.
6. ⚠️ **Et une chaîne COURTE ne prouve rien de son absence sur un binaire
   release** : `traversees` et `seaux_ms` rendent **zéro** alors qu'ils sont dans
   le binaire — le compilateur inline les littéraux courts en constantes
   immédiates coupées aux frontières de mot machine (`traverse` + `es`, vérifié
   par `dd`).

---

## 13. 🔴 Les deux incidents d'environnement, et leurs causes

### 13.1 La VM s'éteint toute seule — et le déclencheur EST identifié

**Deux fois pendant cette campagne** (06:52:12 et 07:32:12, ~40 min d'écart).
`/var/log/libvirt/qemu/Windows.log` porte `terminating on signal 15 from pid
<N>`, et ce PID est **`/usr/sbin/libvirtd --timeout 120`** : le démon s'arrête
sur inactivité et **emporte le domaine**.

⚠️ **CE N'EST PAS LE MÉCANISME DE D1**, et les confondre ferait chercher la
cause du mauvais côté. Celui de D1 était une **hibernation initiée DANS
l'invité** (Kernel-Power 187/42, `shutdown.exe`), QEMU se terminant ~5 s
**après**. Ici c'est **l'hôte** qui tue, et l'invité ne décide rien.

Parade posée dans `jouer-f4.sh` : relancer si besoin, et **attendre le partage
par un ACCÈS RÉEL** — l'entrée de montage CIFS persiste VM éteinte.

### 13.2 🔴 Un chantier voisin a écrit dans `agent/` pendant la campagne

Le chantier **A1 (couleur d'accent)** a modifié `agent/src/capteur/distante.rs`
pour y déclarer un module dont le fichier était **non suivi par git**.
`sync-agent.sh` ne synchronise que le suivi : **le build sur la VM a échoué**
sur `couldn't read agent\src\capteur\distante\video_source.rs`.

⚠️ **J'AI VIOLÉ LA RÈGLE §1.2 n°3 DU PLAN** : j'avais vérifié
`git status --porcelain proto/ agent/` **avant le premier build**, et pas avant
les suivants. **La règle est de le vérifier avant CHAQUE build**, parce qu'un
voisin peut commencer à écrire en cours de campagne.

🔵 **Ce qui a limité les dégâts** : le contrôle d'auto-identification du binaire
a rendu **0**, ce qui a immédiatement dit que la mesure allait tourner sur le
mauvais binaire. **Sans lui, j'aurais publié une mesure de `MORCEAUX_EN_VOL=1`
prise sur un binaire à 4.** Le diagnostic correspondant a été **abandonné**
plutôt que joué sur un binaire faux.

⚠️ **Aucune mesure publiée ici n'est affectée** : toutes ont tourné sur un
binaire dont la chaîne d'identification a été vérifiée.

---

## 14. Les comptes de clôture, avec leur ATTRIBUTION

⚠️ **L'arbre est partagé, et il a bougé de dix-sept commits pendant ce
sous-bloc** (chantiers **G4** et **A1**). Les comptes bruts ne sont donc **pas**
attribuables à F4, et l'attribution est faite **par un worktree** à mon dernier
commit.

| Commande | Entrée (`ae74ab9`) | Mon arbre (`8057f50`) | Aujourd'hui | Attribution |
| --- | --- | --- | --- | --- |
| `cargo test -p agent` | 916 | **928** | 954 | **F4 : +8**, vérifiés nommément (7 `pont::latence` + 1 `resoudre_rend_l_age`). **+4 de G4** intercalés, **+26 de A1/G4** depuis |
| `cargo test -p proto` | 109 | — | **109** | F4 : 0 |
| `client` vitest | 466 | — | **474** | F4 : 0 (aucun test client) |
| `proto` vitest | 296 | — | **296** | F4 : 0 |
| `cargo check --target x86_64-pc-windows-gnu` | 0, **22** | — | 0, **40** | **tous `dead_code`** (vérifié un à un ; `SuiviAccent is never constructed` est de A1). **F4 n'en ajoute aucun** |
| `client`/`proto` `tsc --noEmit` | — | — | **0 / 0** | |

**Tailles, relevées PAR LA COMMANDE après la dernière édition** — le tableau de
dette a **DEUX** lignes, `encode.rs` **1536** et `windows_source.rs` **630**,
**aucune touchée par F4**. Fichiers de F4 : `latence.rs` **226**,
`latence/tests.rs` **177**, `service.rs` **341**, `service/recensement.rs`
**160**, `table.rs` **376**, `pont.rs` **265**, `projfs/etat.rs` **354**.
**Aucun n'approche 460.**

✅ **La porte du §2.4 s'est déclenchée, et l'extraction a été jouée AVANT
l'addition qui l'imposait** : `service.rs` atteignait **458** pour une porte à
**460** ; `recenser`, `tout_completer` et `mesure_armee` en sont partis
**verbatim** vers `service/recensement.rs`. `service.rs` retombe à **332**
(**341** après la revue transverse).

⚠️ **`latence.rs` fait 226 lignes pour un budget « ≤ 180 » visé par le plan.**
Déclaré. Aucune porte n'est franchie ; l'écart est du commentaire.

---

## 15. La revue transverse (tâche 16 ①)

Barème du dépôt : 5 en D7, 3 en D8, 6 en D9, 12 en D10, 7 en D11, 8 en P1, 10 en
P2 (sur 23 places), 5 en S1, 9 sur le chantier E, 12 en P3, 12 en S2, 11 en F1,
8 en P4, 13 en S3, 8 en G1, 7 en F2, 4 en F3. **Neuf ici**, sur **quatorze
places**, toutes énumérées par `grep -n` **avant** l'édition et relues **après**.

| # | Place | Sort |
| --- | --- | --- |
| 1 | `agent/src/pont/lecture.rs:41` — « C'est F4 qui jugera » (facteur ~120) | **CORRIGÉ** — F4 a jugé, **et il explique** |
| 2 | `agent/src/pont/service.rs:47` — `PERIODE_BALAYAGE`, « c'est F4 qui donnera de quoi la juger » | **CORRIGÉ** — F4 a donné, et ce qu'il donne est qu'elle **ne mord pas** (résidu ≤ 0,8 %) |
| 3 | `agent/src/pont/table.rs:24` — les budgets | **CORRIGÉ** — deux des **cinq** mordent réellement |
| 4 | `proto/src/fichiers.rs:44` — `TAILLE_TRAME_MAX` | **CORRIGÉ**, **et un fait ajouté** : ce n'est PAS elle qui borne un listage |
| 5 | `client/src/fichiers/flux.ts:26` — « C'est F4 qui jugera » | **CORRIGÉ**, et la contre-pression est déclarée **jamais servie en exploitation** |
| 6 | `client/src/fichiers/mutation.ts:29` — « la mesure de F4 reste due » | **CORRIGÉ** — elle est faite, coût nul |
| 7 | `client/src/fichiers/adaptateur.ts:244` — « mille ouvertures, mesurable en F4 » | **CORRIGÉ** avec le chiffre, **et sa portée** : F4 mesure la traversée, jamais ce qui la compose |
| 8 | `client/src/fichiers/noms.ts:71` | 🔵 **ANNOTÉ « F4 NE L'A PAS DIT »** — la phrase reste VRAIE, et la taire ferait croire le contraire |
| 9 | `client/src/fichiers/adaptateur.ts:36` | 🔵 **idem** |

**Deux places relues et LAISSÉES INTACTES**, parce qu'elles restent vraies :
`agent/src/pont/enumeration.rs:26-37` (le critère rouge de F5 est par
construction rouge tant que F5 n'existe pas — **confirmé** par la tâche 1) et
`agent/src/pont/projfs/etat.rs:340` (« la mesure de fond appartient à F5 » —
F4 ne mesure pas le disque).

**La spécification est ANNOTÉE, jamais réécrite** — c'est un relevé daté du
19 août 2026 : §8 F4 (les quatre lignes de sa table remplacées ou restreintes,
D1 à D4), le §5.3 (« F4 donnera de quoi les calibrer » → **il a donné, sur
cinq**), R2 (**le risque se réalise ailleurs qu'où il était attendu**), et la
liste des non-calibrées (**`TTL_ENUMERATION` n'est pas « posé, pas mesuré » — il
N'EXISTE PAS**).

---

## 16. Ce que F4 lègue

**Legs fermés par F4** : le legs n°5 de F1 (le facteur ~120 — **expliqué**), le
legs n°2 de F3 (`en_vol_max` — **mesuré, et il révèle que la fenêtre n'est jamais
atteinte en exploitation**), et le coût du repli de renommage (**mesuré, nul**).

**Neufs, et le premier est de très loin le plus lourd :**

1. 🔴 **LE CANAL DU PONT PLAFONNE À ~33 Kio/s, ET AUCUN FICHIER DE PLUS DE
   128 Kio NE PEUT ÊTRE LU.** Cause établie par mutation : la boucle de
   `pont::transport` lit un datagramme par tour et dort jusqu'à `ATTENTE_MAX`
   avant chaque lecture. **La parade est un changement de conception de la
   boucle** (lire le socket en rafale jusqu'à `WouldBlock` avant de rendre la
   main), pas un réglage de constante — donc **elle n'appartient pas à un
   sous-bloc de mesure**. **Sans destinataire.**
2. 🔴 **AUCUN LISTAGE DE PLUS DE ~3 150 ENTRÉES N'ABOUTIT**, et le mode d'échec
   est vingt secondes de gel suivies d'une erreur opaque. La parade — découper
   une énumération en plusieurs trames — est un changement de **forme** du
   protocole, donc un incrément de `FICHIERS_VERSION`. **Sans destinataire.**
   ⚠️ **Et un correctif de moindre coût est nommé sans être livré** : rien ne
   borne la taille de l'en-tête **d'aucun côté**, et le `.catch()` de
   `canal.ts:134` **ne répond rien** — au minimum, un `Echec` renvoyé au pont
   remplacerait vingt secondes de gel par une erreur immédiate.
3. 🔴 **`MORCEAUX_EN_VOL = 4` REND LE PRODUIT PIRE À CE DÉBIT**, et c'est
   arithmétique : quatre morceaux concurrents à 33 Kio/s dépassent chacun
   `DELAI_LIRE`. Une lecture de 256 Kio aboutirait à un morceau en vol et échoue
   à quatre. ⚠️ **NON ÉPROUVÉ PAR MUTATION** : le diagnostic a été abandonné
   quand le voisin a cassé le build (§13.2). **La mutation est écrite et prête**
   (`diag-morceaux-en-vol.log`).
4. ⛔ **Le mécanisme des DEUX `Lister` par `Get-ChildItem`** — le coût est
   doublé, et personne ne sait pourquoi.
5. ⛔ **La cause du delta M2 − M1 inverse** : mesurée, non expliquée.
6. ⛔ **Le coût de la canonicalisation de casse de F3** reste dû.
7. ⛔ **F5 — `Rafraichir` et le cache d'énumération**, sans lequel la moitié
   « chaud » de la table de la spec n'a pas d'objet. ⚠️ **Un cache ne déplacerait
   PAS le mur de 3 150 entrées.**
8. ⛔ **F5 — l'occupation disque et la politique d'éviction**, que F4 fait
   croître sans la mesurer.
9. ⛔ **Sans destinataire — l'idiome fichier temporaire + renommage sur un
   éditeur réel** (legs n°3 de F3), la lacune la plus lourde du sous-projet.
10. ⛔ **Sans destinataire — la fenêtre de trente secondes de F2**, dont le
    remède est nommé et non appliqué.
11. ⛔ **Sans destinataire — `showDirectoryPicker()`**, le modèle de permission
    et le mode `readwrite`, qui exigent `Xvfb` + `xdotool`.
12. ⛔ **Un autre chantier — le réexamen du sélecteur de fichiers**, que F4
    alimente et ne conduit pas.
13. ⛔ **La calibration** des cinq budgets, des treize seaux de `pont::latence`
    et de tout le reste : **le jugement d'usage reste à porter.**
