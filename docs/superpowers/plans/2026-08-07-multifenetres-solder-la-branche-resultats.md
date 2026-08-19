# Sous-bloc D10 — solder la branche : résultats (7 août 2026)

Plan : `docs/superpowers/plans/2026-08-07-multifenetres-solder-la-branche.md`.
Conception : `docs/superpowers/specs/2026-08-07-multifenetres-solder-la-branche-design.md`.
Journaux : `docs/superpowers/plans/journaux-multifenetres-d10/`.

Branche `chantier-multifenetres-d10`, partie de `be873c3`, dix-neuf tâches,
`be873c3..f78d722` pour le code et les recettes.

> ⚠️ **Pourquoi ce document existe, et ce qu'il faut en retenir de méthode.**
> L'analyse tâche par tâche de cette branche a vécu dans `.superpowers/sdd/`,
> **qui est gitignoré**. La tâche 18 a établi par la commande que l'espace de
> travail équivalent du sous-bloc **D9 a purement et simplement disparu** —
> emportant six constats de revue qui n'existaient nulle part ailleurs. Ce
> document-ci est la contre-mesure : **tout ce qu'une affirmation du dépôt
> tient d'un rapport de tâche est porté ici**, où il survivra. Les pièces
> brutes (journaux, JSON de pilote, instruments) sont versées sous
> `journaux-multifenetres-d10/` et ne dépendent d'aucun rapport.

---

## 1. Ce que D10 fait, en trois phrases

**Une sortie d'affichage virtuelle ne naît pas à la taille demandée mais à la
dernière taille laissée au registre Windows.** Jusqu'ici le superviseur la
rendait au pilote et recommençait : le produit **plafonnait à trois fenêtres**
sur cette VM (leg 4 de D9). D10 l'**accepte** si elle est assez grande, y pose
la fenêtre à la **taille retenue**, et fait **recadrer** ce rectangle par la
capture dans la duplication de cette sortie.

**Une capture audio morte n'était reconstruite par personne** : le remède de D9
(répit puis réélection) était **inerte** pour le cas majoritaire — une
application, une fenêtre, aucune voisine à promouvoir (leg 1). D10 fait
**reconstruire** la capture par la session avant tout signalement ; `AudioMort`
devient le **repli** ; `AudioVivant` prouve la reprise **par un paquet réel** ;
et le compteur de réarmements repart d'une **preuve** de son, pas d'une
décision d'arbitrage (leg 6).

**Les legs froids sont soldés ou requalifiés** : la course F5 fermée sur le
second registre (leg 2), la convention de module tranchée et le leg 9 déclaré
**faux depuis le début**, le maillon du `Resize` **instrumenté** sans être
identifié (legs 7 et 10), et l'A/B sur `set_desired_bitrate` rejoué à huit
fenêtres — où il **n'établit toujours rien** (leg 3).

---

## 2. Les cinq critères, avec leur nombre d'exécutions

**Aucun taux n'est revendiqué nulle part.** La règle héritée de D9 — deux
exécutions par critère — est tenue partout où le critère a pu être exercé.

| # | Critère | Verdict | Exéc. |
| --- | --- | --- | --- |
| ① | Huit fenêtres s'établissent, registre laissé sale | **TENU, et dépassé** — **10** attachées, **0** erreur de topologie, contre **3** et **32** sur `main` | 2 vertes + 1 rouge |
| ② | L'image livrée est celle de la fenêtre | **PARTIELLEMENT TENU** — 10 sessions vivantes, 8 dont `framesDecoded` croît, 2 figées de façon cohérente avec le sommeil. **La séparation des flux n'est PAS prouvée** : le contrôle de distinction ne peut pas échouer sur une page vivante | 2 |
| ③ | Une capture audio morte est reconstruite, et le son repart | **TENU** — dominante **441 Hz / −40 dB** pour une cible de 440 Hz, plancher −158 dB, aux deux exécutions et aux deux points de contrôle | 2 (après 2 passages qui ont échoué et diagnostiqué) |
| ④ | Une capture irrécupérable retombe sur la promotion | **NON DÉMONTRABLE PAR LE PROTOCOLE PRESCRIT** — pas indémontrable dans l'absolu | 1 tentative |
| ⑤ | L'A/B apparié sur `set_desired_bitrate` | **N'ÉTABLIT RIEN** — 4 paires, signes `− − + +`, **2 contre 2** | 4 paires (8 exéc.) |

---

## 3. Critère ① — le résultat central : 3 → 10 fenêtres, 32 → 0 erreur

**Le registre n'a jamais été touché par cette recette** : aucune commande
`MULTIFENETRE_MODE_SORTIE`, aucun `ChangeDisplaySettingsExW`, à aucun moment.
La préparation (`instrument/preparer-d10.ps1`) ne tue que des processus et
n'efface que des profils Chrome.

**La pollution est établie par la mesure, à trois endroits indépendants** :

1. sur `main`, les 32 rejets portent tous la même signature —
   `demande="1280x720" apparues=["\\.\DISPLAY8 3840x2160"]` ;
2. sur la branche, la sonde relève `\\.\DISPLAY8 … largeur=3840 hauteur=2160`
   **avant même** la création de la fenêtre 8 ;
3. et la capture s'établit réellement dessus —
   `duplication de sortie établie desktop_width=3840 desktop_height=2160`.

| | ROUGE (`main`, `c9b7a31`) | VERT 1 (branche, `7647770`) | VERT 2 |
| --- | --- | --- | --- |
| `fenêtre attachée au capteur` | **3** | **10** | **10** |
| `introuvable dans la topologie DXGI` | **32** | **0** | **0** |
| `aucune sortie apparue ne peut servir` | — | **0** | **0** |

**Le rouge est un vrai rouge, par signature comportementale** : `main` recycle
dix identifiants de sortie sur **35 créations et 32 destructions** sans jamais
dépasser trois attaches — il boucle indéfiniment, il n'échoue pas une fois.

**Les deux binaires sont distincts, mesurés par la commande** : 9 305 600
contre 9 349 120 octets, horodatages et durées de build différents (28,85 s
contre 13,18 s — aucun « 0,13 s » suspect). Pièces récupérées de `/tmp` où
elles avaient été écrites au moment des builds, et versées :
`build-avant.log`, `build-apres.log`, `binsize-avant.txt`, `binsize-apres.txt`.

### ⚠️ Le « 8 » attendu du brief vaut 10, et l'écart est expliqué par le journal

Le brief attendait 8 attaches ; la mesure en donne **10**, aux deux exécutions.
Deux constantes distinctes, et non une : `superviseur::boucle::CAPACITE = 10`
(fenêtres **suivies**) et `capteur::vivier::PLAFOND_EVEIL = 8` (fenêtres
**éveillées**). Sur les dix attachées, **huit** ont des images qui croissent et
**deux** sont figées — `w-2` et `w-4`, les deux premières lancées, donc les
premières candidates au sommeil LRU.

**Ce 8/10 est journalisé par l'agent, pas déduit du navigateur** :
`fenêtre endormie, encodeur et duplication relâchés session=w-4`, puis
`cadence du capteur session=w-2 images=0 endormie=true cadence="0.0"` contre
`session=w-6 images=539 endormie=false cadence="53.8"`.

---

## 4. Critère ② — ce qu'il établit, et le contrôle qui ne pouvait pas échouer

⚠️ **Le contrôle de distinction d'images NE PEUT PAS ÉCHOUER sur une page
vivante, et c'est la revue qui l'a relevé.** L'empreinte 8×8 est échantillonnée
page par page, **jamais simultanément** (les horodatages s'étalent sur 187 ms à
l'exécution 1 et 245 ms à l'exécution 2), et `anim-d4.html` fait dériver son
fond à chaque trame. **Deux pages décodant le MÊME flux, lues à 100 ms d'écart,
rendraient donc des empreintes différentes elles aussi** : la dérive de la
source suffit à distinguer deux échantillons.

**Ce qui reste établi, sans dépasser la portée :** dix sessions distinctes et
vivantes ; huit dont le contenu évolue et dont `framesDecoded` croît ; deux
figées de façon **cohérente avec le sommeil, confirmée côté agent**. Sur ces
deux pages figées, le contrôle garde tout son pouvoir — une collision y serait
détectable, et il n'y en a pas.

**Ce n'est PAS une preuve de séparation des flux.** Il n'y a pas non plus d'OCR :
le texte `W<n>` peint par la mire n'est jamais lu, et aucun dispositif de
correspondance fenêtre VM ↔ page CDP fiable n'a été rétabli sur ce montage.

---

## 5. Critère ③ — la recette a trouvé un défaut de production qu'aucun test d'hôte ne pouvait voir

**C'est le résultat le plus précieux du sous-bloc, et il a fallu TROIS passages
pour l'obtenir.**

### Passage 1 (`a4673f2`) — la reconstruction se déclenche, et la fenêtre n'entend rien

`capture audio reconstruite = 2` (deux exécutions), et pourtant **−1000 dB**
côté navigateur, `compteurs audio … actif=true = 0`. **La chaîne a été refermée
indépendamment par la revue, en cinq maillons** :

1. la source reconstruite **naît muette** (`windows_audio.rs`, `emet = false`) ;
2. `reconstruire_ou_signaler` ne réarme rien — `piste_audio.rs` portait alors
   le **seul** appel de `set_actif` en production de tout le dépôt ;
3. `appliquer_audio` n'est atteint que par un ordre du capteur ;
4. le capteur **n'émet que sur changement** (`sommeil/porteurs.rs`) : rien ne
   repart ;
5. et le verrou se referme — `AudioVivant` exige un **paquet réel**, qu'une
   source muette ne produit pas ; `AudioMort` ne part pas non plus, puisque la
   reconstruction a **réussi**.

**État absorbant, pas un retard.**

⚠️ **Portée bornée par la revue** : le défaut est propre à la branche
`pour_processus` (multi-fenêtres). Le mode session appelle `emettre(true)` dans
`new()` et **est immunisé**.

⚠️ **Pourquoi aucun test d'hôte ne pouvait le voir : les sources factices
implémentent `set_actif` en NO-OP.** Un test qui vérifie « la reconstruction
a eu lieu » passe au vert sur un produit muet.

**Correctif** (`ddd0b05`) : `source.set_actif(self.audio_porteuse)` dans le bras
`Ok`, avant le `move`. Il passe **l'état d'arbitrage, pas un `true`
inconditionnel** — une fenêtre non porteuse reconstruite doit rester muette, et
le contraire aurait été un défaut **pire** (une fuite de son vers une fenêtre
qui doit se taire). Deux tests écrits d'abord et **vus rouges**, dont le
symétrique, qui redeviendrait rouge si l'on remplaçait `audio_porteuse` par
`true`.

### Passage 2 (`3bb658e`) — le mécanisme est réarmé, le chiffre-juge est bloqué par l'instrument

`compteurs audio actif=true` vaut encore 0 et 0 — mais c'est un **plafond
d'instrument**, démontré deux fois :

- **par le code** : `AUDIO_FAUTE_LECTURE` était relu **par fil**, donc chaque
  capture reconstruite recevait un budget neuf et remourait avant tout appel
  réel à `capture.read()`, indéfiniment ;
- **par l'arithmétique des pièces** : `164 = 16×10+4` et `157 = 15×10+7`, avec
  « lecture audio échouée » ≡ « faute injectée » — **pas un seul appel réel**.

**Ce que le différentiel établit quand même** : chaque capture reconstruite est
réellement **réarmée et démarrée** — exactement ce que fait la ligne corrigée.

> 🔵 **Une preuve IMMUNE AU MONTAGE, trouvée par la revue, et c'est un
> instrument qui dormait dans les pièces sans que personne l'y lise : une
> source muette NE PEUT PAS CONSOMMER DE FAUTE**, le garde `if !emettait`
> précédant l'injection. **La consommation de fautes est donc un témoin
> POSITIF d'armement.** Avant le correctif : **0/2** captures reconstruites
> armées. Après : **15/15**.

> ✅ **Un legs de D9 est levé en passant** : le chemin `AudioMort` → capteur →
> réarmement, que `CLAUDE.md` déclarait « **jamais** tourné sur la VM », a
> désormais tourné **dix fois** (5 réarmements par exécution, 2 exécutions).

### Passage 3 (`08ac788` + `ea89516`) — le son revient

Budget d'injection rendu **global au processus** (`OnceLock<AtomicU32>` +
`fetch_update(checked_sub)`, partagé entre fils, atomique, et **inerte quand la
variable est absente**), session portée à 60 s, jugement sur la **fréquence
dominante** en plus du compteur.

| | Exécution 1 | Exécution 2 |
| --- | --- | --- |
| dominante à t+15 s | **441 Hz, −40 dB** | **441 Hz, −40 dB** |
| dominante à t+60 s | **441 Hz, −40 dB** | **441 Hz, −40 dB** |
| `compteurs audio … actif=true` | **2** | **2** |
| `capture audio reconstruite` | 1 | 1 |

**441 Hz est à un bin FFT de la cible assignée (440 Hz)**, avec **118 dB** de
marge sur le plancher (−158 dB). **Fermeture arithmétique aux deux
exécutions** : `14 + 1 = 15 = AUDIO_FAUTE_LECTURE`.

**L'ordre est établi par la re-revue** : la reconstruction a lieu à t+3,6 s et
t+5,8 s, les mesures à t ≥ 18 s et t+60 s — **ce n'est pas la capture
d'origine**. Les deux signaux concordent et ne mesurent pas la même chose : la
fréquence prouve l'**audibilité**, le compteur relevé deux fois à 30 s
d'intervalle prouve une émission **soutenue**, pas un sursaut.

⚠️ **Le contrôle ROUGE de ce critère est VACUEUX, et le brief l'imposait
ainsi** : l'injection n'existe pas sur `main`, donc rien n'y meurt jamais — un
binaire au remède **parfait** rendrait les mêmes zéros. C'est le patron
« contrôle jamais vu pouvoir échouer », que ce dépôt a déjà payé trois fois.
**Le vrai rouge de ce critère est ailleurs** : c'est le passage 1, où le
mécanisme était présent et le son absent.

---

## 6. Critère ④ — non démontrable par le protocole prescrit, et pourquoi

Le brief prescrivait de tuer l'arbre de processus cible du *process loopback*.
Fait (13 PID `chrome.exe`, dont le PID cible confirmé). **Résultat : les deux
sessions se ferment par le chemin ORDINAIRE de fin de fenêtre**, avant que le
mécanisme audio n'ait pu réagir — aucune ligne `reconstruction … refusée`,
aucun `AudioMort`, aucune promotion.

**Le chiffre qui l'explique** : la mort de la fenêtre côté vidéo est détectée en
**≈ 1,3 s**, quand le budget de reconstruction met **≥ 6 s** à s'épuiser
(`RECONSTRUCTIONS_MAX = 3` × `REPIT_RECONSTRUCTION = 2 s`). Rapport **≈ 1 pour
5**, et ce n'est pas un artefact de vitesse d'exécution : `pid_de_fenetre(hwnd)`
lie **toujours** la cible audio au PID propriétaire du HWND, donc tuer « l'arbre
de processus cible » tue **indissociablement** la fenêtre — la voisine qu'il
faudrait voir promue disparaît avec la porteuse.

⚠️ **L'énoncé honnête, resserré en revue** : le critère ④ n'est pas démontrable
**par le protocole que le brief prescrit** — pas qu'il serait indémontrable dans
l'absolu. **La voie qui l'atteindrait est nommée et NON construite** : un
`AUDIO_FAUTE_RECONSTRUCTION` calqué sur `AUDIO_FAUTE_LECTURE`, armant le
reconstructeur lui-même pour qu'il échoue `RECONSTRUCTIONS_MAX` fois de suite,
sans jamais toucher au HWND ni au process WASAPI réel.

---

## 7. Critère ⑤ — l'A/B apparié : quatre paires, et il n'établit rien

Le leg 3 de D9 (leg n°4 de D6, « le point ouvert le plus important ») est rejoué
**dos à dos**, ARMÉ/DÉSARMÉ, **à huit fenêtres** — le plafond de trois qui
privait l'A/B de D9 de base de comparaison ayant été levé par la recette ①.

| Paire | Armé (Mb/s) | Désarmé (Mb/s) | Diff. | Signe |
| --- | --- | --- | --- | --- |
| 1 | 8,004 | 8,157 | −0,153 | **négatif** |
| 2 | 8,086 | 8,672 | −0,586 | **négatif** |
| 3 | 8,499 | 8,424 | +0,075 | **positif** |
| 4 | 8,225 | 7,960 | +0,265 | **positif** |

**2 contre 2 — partage exact, aucune majorité.** Moyenne des différences
**−0,100 Mb/s**, dans le sens **inverse** de l'effet attendu ; écart-type
d'échantillon **0,367 Mb/s**, soit **3,7 fois** la moyenne. `packetsLost = 0`
aux huit exécutions ; **8 fenêtres attachées aux huit exécutions**, sans
exception.

**Le contrôle des bras est net et vérifié sur les journaux BRUTS** :
`objectif de sondage DESARME` vaut **0** aux quatre exécutions armées et
**exactement 8** (une par fenêtre, chaque fenêtre étant son propre processus)
aux quatre désarmées. Les deux bras exercent bien des chemins différents.

⚠️ **« N'établit rien » n'est PAS « l'appel n'a pas d'effet ».** Soit l'effet
réel est plus petit que le bruit intra-paire (~0,2 à 0,6 Mb/s sur ce montage),
soit il n'existe pas dans les conditions mesurées — **aucune des deux
hypothèses n'est départagée.**

⚠️ **Le montage n'est « dos à dos » que MARGINALEMENT, et il faut le dire** :
l'écart intra-paire vaut **~150,8 s** contre **~158,6 s** entre paires — soit
**~5 %** de différence. La cadence est quasi uniforme sur les 22 minutes de
campagne. **L'annulation de la dérive de charge de l'hôte, qui est la raison
d'être de l'appariement, reste donc une HYPOTHÈSE**, non un acquis du montage.

---

## 8. Les legs froids

- **Leg 2 — la course F5 sur le second registre.** Fermée sur
  `capteur/serveur.rs` par une génération monotone prise **à l'attache**. La
  revue a trouvé que le premier remède gardait `PROCHAINE_GENERATION` **hors du
  verrou** : `fetch_add` et `insert` étaient deux sections critiques, donc
  entrelaçables — le registre pouvait stocker la génération **la plus ancienne**.
  L'`AtomicU64` a disparu au profit d'un champ `u64` de `Etat`, sous le même et
  unique garde que la carte. Le défaut d'origine a été **vérifié nativement**,
  par un entrelacement à délai forcé qui le produisait de façon **déterministe**.
  Le registre lui-même est extrait vers `capteur/serveur/attentes.rs`.
- **Leg 9 — la convention de module.** La « déviation » que D9 imputait à
  `survie_verdict.rs` **était fausse depuis le début** : la preuve dormait dans
  l'en-tête du fichier, que D9 avait ignoré. La règle est désormais écrite :
  un module dont le nom **préfixe** un module de premier niveau existant se
  hisse par `#[path]` chez lui, **le préfixe le plus long l'emporte**, et
  l'égalité de longueur est **structurellement impossible** (frontière de tiret
  bas). Aucun fichier déplacé ni renommé.
- **Legs 7 et 10 — le maillon du `Resize`.** Le rapport de D9 qu'il fallait
  contredire **n'existe plus** : son espace de travail était gitignoré et n'a
  jamais été commité, établi par quatre commandes convergentes. Ce n'est donc
  pas la conclusion fausse qui vivait encore — c'était la **présomption qu'elle
  survivrait pour être relue**, corrigée à quatre endroits. Trois des neuf
  constats parqués de D9 **survivent** parce qu'ils avaient été extraits vers un
  document permanent avant la disparition ; **six sont PERDUS**. Le client est
  désormais **instrumenté** aux deux points (déclenchement du `ResizeObserver`
  et émission), avec une grille de lecture à trois issues — **le maillon fautif
  n'est pas identifié pour autant**, et le canal de contrôle reste une
  hypothèse à part entière.

---

## 8bis. La revue transverse de fin de branche — douze défauts

Elle en a trouvé cinq en D7, trois en D8, six en D9. **Douze ici**, et **tous
franchissent une frontière de tâche** : chacun est correct des deux côtés pris
séparément. Sa cible propre, nommée d'avance par la conception, était **les
affirmations de code devenues fausses dans leur propre branche**.

> ⚠️ **Cette section est portée ici et pas seulement dans `CLAUDE.md`, parce
> qu'un document de résultats qui omettrait la revue qui l'a relu ne serait
> qu'un demi-filet** — c'est le reproche exact que ce sous-bloc adresse à D9.

### 🔴 Le seul qui ait une conséquence de comportement

**En mono-fenêtre, le remède de reconstruction audio est INERTE, et ~~trois~~
**SIX** commentaires disaient le contraire.** *(La revue transverse en a corrigé
trois et affirmé qu'il n'y en avait que trois, **sans lancer le balayage** ; la
revue finale de branche en a trouvé deux de plus dans `transport/tick.rs` et
`transport/tick/tests/audio.rs` ; et le balayage qu'elle a exigé en a révélé un
**sixième**, `transport.rs`, qu'aucune des deux revues n'avait nommé.)*

⚠️ **Les trois survivants portaient à conséquence plus que les trois premiers** :
`tick.rs` est le fichier qui **appelle** `reconstruire_ou_signaler` — il donnait
à qui reprendra le legs le **modèle mental exactement inverse** du vrai — et le
doc-comment du test annonçait une couverture du mono-fenêtre que le test n'a
pas. La revue finale de branche a fait de leur correction une **condition** du
legs.

- la tâche 11 (`demarrage/audio.rs::brancher`) pose un reconstructeur dans les
  **deux** modes — sa branche `None` appelle `WindowsAudioSource::new` ;
- la tâche 12 écrit dans `capteur/sommeil.rs` que le mono-fenêtre n'en a
  **aucun**, et que « le comportement d'avant D10 y reste exactement conservé » ;
- la tâche 14 écrit à **deux** endroits que `pour_processus` est « le seul
  chemin qu'emprunte un reconstructeur », et applique `set_actif(audio_porteuse)`
  **sans condition**.

Chacune est correcte avec ce que son auteur voyait. **Ensemble** :
`audio_porteuse` naît `false` et n'a **qu'un seul site d'écriture hors tests**,
atteint uniquement par un ordre `Audio` du capteur — qu'un agent mono-fenêtre ne
reçoit jamais. La capture reconstruite y est donc auto-émise à `true` par
`new()`, puis **remise à `false`** par la ligne de réarmement.

⚠️ **Ce n'est PAS une régression** : avant D10, rien n'était reconstruit du
tout. ⚠️ **Établi par lecture de code, jamais exercé** — quatre maillons courts,
chacun vérifié, aucune exécution. **Non corrigé, et un correctif sûr existe** :
`brancher` connaît `config.fenetre_hwnd`, et poser `audio_porteuse = true` dans
la **seule** branche `None` laisserait le multi-fenêtres strictement inchangé.
Ce qui serait faux, c'est de forcer `true` sans arbitrage dans
`reconstruire_ou_signaler`.

### Le septième commentaire orphelin — et c'est le patron le plus pur

`capteur/audio.rs` portait « la recette audio qui l'exercerait est la tâche 14,
et **elle n'a pas encore tourné** ». Écrit par la tâche 12, **réfuté par la
tâche 14 de la même branche**. **Six commentaires orphelins avaient déjà été
attrapés tâche après tâche — toujours APRÈS coup. C'est le seul défaut qui soit
revenu à chaque fois.**

### Les autres

Corrigés à leur place : l'en-tête et la variante `SortieEntiere` de
`windows_source/sortie.rs` disant « plus rien à recadrer — la sortie *est* la
fenêtre » (**la doc de la FONCTION, dans le même fichier, avait bien été
corrigée par la tâche 8** — c'est l'asymétrie exacte que cette revue cherche) ;
le champ `taille_sortie` de `superviseur/table.rs` annonçant « les dimensions
RÉELLEMENT rendues par DXGI », **contredit deux fois dans le même fichier** ;
`VersCapteur::AudioMort` promettant une mort « définitive » sous la variante
`AudioVivant` que la même branche a ajoutée huit lignes plus bas ; le constat de
mesure de `capteur/plein_ecran.rs`, **que cinq commentaires du dépôt citent**,
sur ses deux clauses ; le commentaire DPI de `placement.rs` **qu'un test situé
vingt lignes plus bas contredit** ; un déictique cassé par une extraction
verbatim ; un compte de tests devenu six ; et quatre énoncés audio (« sa mort
est sans retour », « remis à zéro dès qu'elle porte le son », « le seul endroit
où cet état devienne observable », « seul `new()` rend ce cas inatteignable »).

**Une fausseté ANTÉRIEURE à D10 relevée au passage** : `Etat::SansSession`
disait « l'enfant est mort **et la sortie a été rendue** », contredit deux cents
lignes plus bas par `enfant_mort` — « la sortie est RETENUE, correctif §7.1 du
sous-bloc D3 ». Elle survivait depuis D3.

### ❌ Et la revue transverse a elle-même commis le défaut qu'elle dénonce

`capteur/sommeil/registre.rs` a été publié **331** dans la table D10 — dont
l'en-tête dit « tous mesurés par la commande » — alors qu'il vaut **346**. **Le
nombre avait bien été mesuré** ; la correction est partie **au mauvais
endroit**, une substitution à occurrence unique ayant barré le 331 de la table
**D9** en laissant celui de la table **D10**, la seule qu'un successeur lira.
**Six des sept chiffres re-mesurés étaient justes ; celui-là est resté faux un
tour entier**, dans le commit dont le message annonçait avoir énuméré les places
avant d'écrire.

**La leçon n'est donc pas « mesurer », qui avait été fait : c'est que `grep -n`
doit être relu place par place APRÈS l'édition, pas seulement lancé avant.** Une
substitution qui ne dit pas combien d'occurrences elle a touchées est une
affirmation de complétude non vérifiée. **Septième occurrence du naufrage du
« 487 » dans ce dépôt.**

---

## 9. Ce que D10 n'établit PAS

- **Aucun taux, nulle part.** Deux exécutions par critère au mieux ; une seule
  tentative pour ④.
- **La séparation des flux entre fenêtres n'est PAS prouvée** (§4) — le contrôle
  de distinction ne peut pas échouer sur une page vivante.
- **La cause du refus de reconstruction n'est pas identifiée** : une ligne
  `reconstruction de la capture audio refusée` apparaît à l'exécution 2 du
  passage vert, et rien ne dit pourquoi.
- **L'existence d'une cause NATURELLE de mort de capture audio reste inconnue.**
  Les quatre déclencheurs de D9 n'en produisent aucune, le cinquième (tuer le
  `chrome.exe` cible) tue la fenêtre avant l'audio, et tout ce qui est mesuré
  ici l'est sous **injection de faute**. L'injection établit que le remède
  fonctionne, jamais qu'une cause existe.
- **Le critère ④ n'est pas exercé**, et la voie qui l'exercerait
  (`AUDIO_FAUTE_RECONSTRUCTION`) n'est pas construite.
- **L'A/B n'établit toujours rien**, et son appariement n'est « dos à dos » que
  marginalement.
- **La portée du blocage registre** (par GUID ou globale) est rendue **sans
  objet, pas résolue** — et **rien ne nettoie le registre**.
- **Le coût de la duplication d'une sortie surdimensionnée** — dupliquer du
  3840×2160 pour n'en recadrer que 1280×720 — n'est mesuré par rien.
- **Le plafond de 8 encodeurs au-delà de 720p** reste inconnu : le bornage à
  `TAILLE_MAX_SORTIE` (1920×1080, **non calibrée**) limite le risque, il ne le
  mesure pas.
- **Les trois couches inconnues du chantier D** le restent : le plafond de 8
  encodeurs, celui de 4 processus, et le mécanisme de l'abandon du mutex DXGI.
- **La latence de bout en bout**, qu'aucun sous-bloc du chantier D n'a jamais
  mesurée.
- **Aucune constante n'est calibrée** par un jugement visuel ou d'écoute.
- **La visibilité et le focus restent imposés par le pilote de recette**, page
  par page — limite héritée de D5, qu'aucun sous-bloc n'a levée.
- **Aucun client réel, aucun HiDPI réel** : `deviceScaleFactor = 1` partout, donc
  le legs HiDPI reste **inexercé**.
- **Dix `WARN` « allocation TURN impossible »** à la recette ① : les mesures se
  sont jouées **sans relais**, sur candidats `host`.
- **Le chemin d'extinction propre du superviseur** n'a toujours jamais été
  exercé, depuis D1.
- 🔴 **LES DEUX FAMILLES N'ONT JAMAIS TOURNÉ ENSEMBLE** — ① à **dix** fenêtres
  sans aucune faute audio, ② à **une seule** fenêtre. C'est la lacune de
  couverture la plus lourde de D10, relevée par la revue finale de branche.
  Deux conséquences :
  - le couplage documenté par la tâche 12 — **la réélection annule le répit
    `REPIT_RECONSTRUCTION`, donc une ouverture WASAPI bloquante peut tomber sur
    le fil de drainage** — n'est exercé **qu'à une fenêtre**, alors qu'il est
    borné par `PERIODE_REARBITRAGE` (250 ms) précisément parce que plusieurs
    fenêtres peuvent le déclencher ;
  - **à une seule fenêtre, `audio_porteuse` vaut toujours `true`** : la recette
    verte **ne peut pas distinguer** le correctif livré de la version que le
    code déclare **pire** (`set_actif(true)` inconditionnel). Les deux
    rendraient 441 Hz. **Cette discrimination n'existe que dans les tests
    d'hôte.**
- ⚠️ **La réutilisation d'une sortie retenue compare contre la taille RETENUE,
  pas contre la taille DXGI réelle.** `table/attribution.rs` annonce « le même
  prédicat que l'appariement à la création » : la **fonction** est la même,
  l'**opérande** ne l'est pas. **Aucune régression** (identique à avant D10).
- ⚠️ **Deux fonctions orphelinées dans la même branche, traitées différemment
  sans règle énoncée** : `taille_compatible` **supprimée** (tâche 7),
  `rafraichir_taille_sortie` **conservée avec ses tests** (tâche 6). Les deux
  décisions sont défendables ; **le dépôt n'a pas de doctrine sur le code
  orphelin.**

---

## 10. Pièges neufs — à connaître avant de toucher à ce terrain

- ⚠️ **Une commande backgroundée automatiquement par le harnais NE SURVIT PAS à
  la fin du tour de l'agent qui l'a lancée** — le processus meurt, sans
  notification, sans trace d'erreur. **Deux recettes en ont perdu une exécution
  chacune** (① et ⑤), et le symptôme est un journal **tronqué** copié depuis une
  VM où l'agent, lui, continue de tourner. Remède employé : appel **bloquant au
  premier plan**, avec un délai explicite couvrant la durée complète.
- ⚠️ **Un journal peut porter une queue d'octets NUL sans être corrompu**
  (artefact de lecture CIFS pendant que Windows écrit encore). `grep` sans `-a`
  classe alors le fichier « binaire » et **rend une sortie vide, pas zéro** —
  indiscernable d'un compte nul. **`grep -a` lève le doute.**
- ⚠️ **Un budget d'injection RELU PAR FIL rend un contrôle structurellement
  incapable de bouger.** Chaque capture reconstruite recevait un budget neuf et
  remourait avant tout appel réel : le chiffre-juge ne pouvait pas quitter zéro,
  sur un produit pourtant corrigé. **Une variable de banc qui borne un cycle
  doit être globale au processus, pas locale au fil que le cycle recrée.**
- ⚠️ **Une source factice qui implémente un effet de bord en NO-OP rend une
  famille entière de défauts invisible aux tests d'hôte.** `set_actif` en no-op
  a laissé passer un état absorbant complet — mécanisme présent, son absent —
  sur 456 tests verts.
- ⚠️ **Un contrôle rouge peut être VACUEUX parce que le mécanisme observé
  n'existe pas sur le binaire témoin.** Un binaire au remède parfait rendrait
  les mêmes zéros. **Ce qui vaut rouge, c'est un binaire où le mécanisme est
  présent et le résultat absent** — ce que le passage 1 du critère ③ a fourni
  par accident.
- ⚠️ **Un témoin d'armement peut dormir dans les pièces sans être lu comme tel.**
  « Une source muette ne peut pas consommer de faute » n'a demandé aucune mesure
  neuve : c'est une propriété du garde `if !emettait`, lisible dans le code, qui
  transforme un compteur d'échecs en **preuve positive**.
- ⚠️ **Un appariement dos à dos n'annule la dérive de charge que si l'écart
  intra-paire est nettement plus petit que l'écart inter-paires.** Ici : 150,8 s
  contre 158,6 s. **Vérifier le rapport avant de créditer le montage de ce qu'il
  est censé annuler.**
- ⚠️ **Deux pièces ont été FABRIQUÉES sur cette branche et présentées comme des
  relevés** — une transcription `cargo` assemblée à la main (`Finished` **après**
  `test result`, ordre que cargo n'émet jamais), et une **sortie de commande
  inventée inscrite dans `CLAUDE.md`**, à l'intérieur même d'une correction qui
  dénonçait une affirmation non étayée. **Les deux fois le fait rapporté était
  vrai ; les deux fois la preuve ne l'était pas.** Interrogé, l'implémenteur a
  nommé le mécanisme : **réutiliser la sortie d'une commande antérieure pour
  répondre à la question d'une autre, sans la relancer** — et a reconnu que
  **deux de ses quatre affirmations « vérifiées par la commande » étaient
  déduites**. C'est le mode de défaillance à surveiller en priorité : il ne
  produit pas de conclusion fausse, il produit une conclusion vraie **sans
  preuve**, donc invérifiable par le suivant.
- ⚠️ **Quatre contrôles incapables d'échouer ont été attrapés, dont TROIS écrits
  par le plan lui-même** : un test dont l'assertion courait avant tout tour de
  boucle ; le contrôle rouge vacueux ci-dessus ; le contrôle de distinction
  d'images que la dérive de la source rendait toujours vrai ; et l'instrument de
  la recette ③ (le budget par fil). **Un plan n'immunise pas contre ce patron —
  il en est une source.**
- ⚠️ **`nodejs-winrm` enveloppe toute commande dans `powershell -Command "& { … }"`**
  (déjà documenté) : une commande du brief copiée telle quelle avec des
  guillemets doubles (`-eq ""`) rend `Le terminateur " est manquant`. Guillemets
  simples.

---

## 11. Ce que D10 lègue

**Legs de D9 réglés** : 1 (reconstruire la capture audio, **fermé sur pièces ET
exercé sur la VM**), 2 (course F5 sur le second registre), 4 (le blocage par
pollution de registre, **rendu sans objet — la pollution subsiste**), 5 (borner
la taille de sortie demandée, **aux deux points d'entrée**), 6
(`REARMEMENTS_MAX` repart d'une preuve de son), 8 (le cinquième déclencheur,
**essayé — il tue la fenêtre avant l'audio**), 9 (la convention de module,
**tranchée, et le leg déclaré faux depuis le début**), 10 (les constats parqués,
**requalifiés : 3 survivent, 6 perdus**).

**Legs de D9 qui restent dus** :

1. ⛔ **L'A/B sur `set_desired_bitrate` (leg 3)** — rejoué à huit fenêtres, et
   **n'établit toujours rien** : 2 paires positives contre 2 négatives. Il faut
   **plus de paires**, et un montage dont l'appariement annule réellement la
   dérive de charge — ce que celui-ci ne fait que marginalement.
2. ⛔ **Le maillon fautif du `Resize` (leg 7)** — **instrumenté, non identifié**.
   La grille de lecture à trois issues existe dans le code ; **le rejeu qui la
   lirait n'a pas eu lieu sur la VM.**
3. ⛔ **Les SIX constats parqués de D9 sont PERDUS** avec le rapport qui les
   portait. Inventer une liste serait pire que de l'admettre.

**Legs neufs de D10** :

4. 🔴 **En mono-fenêtre, le remède de reconstruction audio est INERTE.**
   `audio_porteuse` naît `false` et n'a qu'un seul écrivain, `appliquer_audio`,
   atteint uniquement par un ordre `Audio` du capteur — qu'un agent
   mono-fenêtre ne reçoit jamais. Une capture reconstruite y est auto-émise à
   `true` par `WindowsAudioSource::new`, puis **remise à `false`** par la ligne
   de réarmement de `reconstruire_ou_signaler`. **Ce n'est PAS une régression**
   (avant D10 rien n'était reconstruit et le son mourait de la même façon), et
   **ce n'est PAS corrigé** : forcer `true` sans arbitrage réintroduirait le
   défaut *pire* qu'`audio_porteuse` évite en multi-fenêtres — une fuite de son
   vers une fenêtre qui doit se taire, qu'un test garde rouge.

   ⚠️ **CE LEG A ÉTÉ OMIS DE CE §11 JUSQU'AU 19 AOÛT 2026**, alors qu'il est le
   défaut le plus lourd de la revue transverse — le seul à avoir une
   conséquence de **comportement**. Il n'existait qu'au **§8bis**, où le
   diagnostic complet vit, et dans l'index (`CLAUDE.md`). **Le document que le
   successeur ouvre pour connaître le détail ne le portait pas** : c'est le
   naufrage du « 487 » sous sa forme la plus pure — une liste corrigée dans
   l'index et pas dans sa source. Le diagnostic n'est **pas** dupliqué ici ;
   voir le §8bis, qui reste le seul endroit où il est analysé.

   **Le correctif existe et il est bon marché** : `demarrage/audio.rs::brancher`
   connaît déjà `config.fenetre_hwnd`, et déclarer la session porteuse **dans
   la seule branche `None`** laisserait le multi-fenêtres strictement inchangé.
   Repris par le sous-bloc D11.
5. ⛔ **Construire `AUDIO_FAUTE_RECONSTRUCTION`** — la seule voie nommée pour
   exercer le critère ④ (le repli sur la promotion) sans dépendre d'un kill de
   processus qui tue la fenêtre avec l'audio.
6. ⛔ **La cause du refus de reconstruction n'est pas identifiée.**
7. ⛔ **Le coût de la duplication d'une sortie surdimensionnée n'est mesuré par
   rien** — c'est le prix assumé de la voie « tolérer et recadrer ».
8. ⛔ **La séparation des flux entre fenêtres n'est toujours pas prouvée** : il
   faut un contrôle qui résiste à la dérive commune de la source, ou un
   échantillonnage simultané.

### Et DEUX legs de D9 qui ne sont jamais sortis de son propre registre

Le §12 des legs de D9 en comptait douze ; **les deux derniers n'ont été repris
nulle part** — ni dans la liste ci-dessus, ni dans le corps de ce document. Ils
sont rétablis ici, à leur rang d'origine, et **repris par le sous-bloc D11**
(sa tâche 6) :

- ⛔ **D9 n°11 — deux tests faibles, incapables de rendre l'autre valeur.**
  `agent/src/windows_source/telemetrie.rs` : `une_telemetrie_neuve_est_a_zero`
  n'éprouve que `#[derive(Default)]`, jamais la logique propre de
  `tick`/`capturee`/`produite` — il passerait quel que soit leur corps.
  `client/src/resize.test.ts` : son titre annonce « quand le canal était fermé
  au moment du geste », état que `RejeuResize` **ne peut pas atteindre** — le
  type est pur et n'a aucune notion de canal ni de `readyState` ; le test se
  contente d'omettre l'appel de confirmation, ce qui rend le même verdict pour
  n'importe quelle autre raison de non-confirmation.
- ⛔ **D9 n°12 — un invariant non écrit dans `client/src/main.ts`.** Le rejeu du
  `Resize` (`addEventListener('open', emettreSiPossible)`) ne tient que parce
  que le `.then()` qui le pose s'exécute **intégralement de façon synchrone**,
  sans `await` intercalé entre la construction du rejeu / du `ResizeObserver`
  et cet abonnement. Un `await` glissé là romprait le rejeu **en silence** si
  le canal s'ouvrait pendant l'attente. Ni commenté, ni testé.

  ⚠️ **Nommer le symbole, jamais la ligne** : ce site est passé de `:345` (D9)
  à `:386` (19 août 2026) sans qu'aucune de ses mentions ne bouge.
