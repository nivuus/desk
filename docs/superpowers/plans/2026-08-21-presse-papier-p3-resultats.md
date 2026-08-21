# Sous-projet ① « Divers » — presse-papier, sous-bloc **P3** : les N fenêtres — résultats

> ⚠️ **TROIS « P3 » PEUVENT SE CONFONDRE DANS CE DÉPÔT.** Celui-ci est le
> **presse-papier**. Le sous-projet ⑤ (plateforme) a un « P3 » clos et sans
> rapport ; le chantier D a des sous-blocs `D1`…`D11`. Quand ce document écrit
> « P1 » ou « P2 », il désigne **toujours** le presse-papier.

Plan : `2026-08-21-presse-papier-p3.md`.
Conception : `../specs/2026-08-19-presse-papier-design.md` — **annotée par ce
sous-bloc**, à son §3.3, son §4.2 et son §6.3.
Journaux : `journaux-presse-papier-p3/`.

---

## 0. LE VERDICT D'ENSEMBLE : LES QUATRE CRITÈRES SONT TENUS

**Les treize tâches sont faites, recette comprise.** La VM, tenue par le
chantier F3 (pont fichiers) pendant tout le travail hôte, a été libérée à la fin
de la ronde ; la recette a été jouée ensuite. **Deux exécutions (`e1`, `e2`),
relevés IDENTIQUES sur les quatre critères.**

⚠️ **Aucun taux n'est revendiqué nulle part.** Deux exécutions établissent la
reproductibilité d'un mécanisme déterministe, **jamais une fréquence**.

| # | Critère | Verdict | Exéc. |
| --- | --- | --- | --- |
| ① | une copie dans la VM parvient aux **trois** fenêtres | **TENU** — 3/3 | **2** |
| ① bis | une **quatrième** fenêtre attachée **APRÈS** la copie reçoit le contenu courant | **TENU** | **2** |
| ② | un collage depuis B met le texte de B dans la VM, **et dans SA fenêtre** | **TENU, et ATTRIBUABLE** | **2** |
| ③ | deux collages quasi simultanés : ni interblocage, ni contenu mêlé, le dernier gagne | **TENU sur ses TROIS volets** | **2** |
| ④ | une fenêtre sans focus n'écrit pas localement, et écrit à la reprise | **TENU** | **2** |

**Le montage** : trois Bloc-notes pré-semés de marqueurs distincts, trois
sessions, **quatre** `fenêtre attachée au capteur` avec la tardive, **quatre
`pid` DISTINCTS** aux deux exécutions — c'est le contrôle du Step 4 de la
tâche 8, et il passe. **ZÉRO `ERROR`**, **zéro** `introuvable dans la
topologie` (aucune pollution de registre).

⚠️ **Le nombre de fenêtres est RELEVÉ, jamais exigé** (D-P3-8) : trois demandées,
trois attachées.

### 🔴 Le résultat le plus lourd : le produit a réfuté ma propre sonde

La sonde S1 avait conclu **« ④ NON MESURABLE en conditions de produit »**. La
recette relève l'inverse : **le focus discrimine parfaitement**, aux deux
exécutions et aux trois basculements.

**Cause, trouvée en RELISANT `client/src/shell-page.ts:117`** : il appelle
`window.open(url, nom)` — **DEUX arguments**. La sonde en passait **TROIS**,
avec `'width=800,height=600'`. **Avec une chaîne de caractéristiques Chrome
ouvre une POPUP, sans elle un ONGLET**, et c'est cela qui décide.

S1 est corrigée et rejouée avec **TROIS armes**, deux exécutions, relevés
identiques :

| arme | issue |
| --- | --- |
| `window.open(url, nom)` — **LE PRODUIT** | **MESURABLE** |
| `window.open(url, nom, 'width=…')` — une **POPUP** | NON MESURABLE |
| `Target.createTarget` | MESURABLE |

⚠️ **La « seconde arme » de S1 avait raison sur la CAUSE — le mode
d'ouverture — et TORT sur laquelle était celle du produit.** *Une sonde qui
croit reproduire un geste doit le RELIRE, pas s'en souvenir.*

🔵 **Corollaire : le régime de N ÉCRIVAINS CONCURRENTS ne s'est PAS présenté**,
puisque le focus discrimine. **Il reste non mesuré**, et c'est un legs.

### 🔴 Cinq défauts de mon instrument, tous trouvés en le jouant

**Sa première exécution a été son premier débogage**, comme il était prévu.
Trois exécutions sont versées **sous un nom qui le dit** —
`1-DISQUALIFIEE`, `2-DIAGNOSTIC`, `r1-DIAGNOSTIC` — plutôt que jetées : ce sont
les pièces des diagnostics.

**a) Le parse attrapait le SPAN, pas le champ.** `/session=(\S+)/` matchait
`fenetre{session=…}:` d'abord, d'où des noms portant `}:` et une attribution
**INUTILISABLE** — ② et ③ cessaient d'être jugeables. **C'est le piège de D8,
rejoué.** Ancré sur `session=… pid=…` **adjacents** : le span ne porte jamais de
`pid`, donc il s'exclut par construction.

**b) `agent.log` lu par CIFS est EN RETARD** — l'attribution rendait `[]` cinq
secondes après que les lignes y étaient écrites. Attente sur le **FAIT**, bornée.

**c) 🔴 Mes motifs de ③(a) cherchaient des chaînes que le presse-papier n'émet
JAMAIS.** Le plan prescrivait `aucune réponse du capteur` et `commande
expirée` : la première **n'existe nulle part dans le dépôt**, la seconde
**n'existe que dans `agent/src/pont/`** — le **pont fichiers**. Leur zéro était
**VACUEUX** : il serait resté zéro alors même que la borne de 12 s aurait mordu.
C'est la règle 10 du §2.2 du plan, **payée sur le plan lui-même**. Les motifs
réels, relevés par `grep` dans `agent/src/` :

- côté **enfant**, le décisif — toute commande qui échoue y passe :
  `collage NON écrit : la touche V est perdue, pas reportée` ;
- côté **capteur**, la borne `DELAI_REPONSE_FENETRE` (12 s) :
  `aucune réponse du fil de fenêtre en …` et `le fil de fenêtre n'a pas
  répondu …`.

⚠️ **Et le zéro se qualifie** : un **contre-contrôle** vérifie que chacun des
quatre motifs matche sa propre ligne. **Les quatre rendent `true`.**

**d) L'amorce était posée trop tard sur la fenêtre tardive.** Elle enveloppe
`createDataChannel`, et le canal existait déjà : `① bis` rendait **`false`**
alors que l'agent **avait émis** — sa trace le porte, une fois, avec ses onze
octets. **Ce qui a tranché est une SECONDE ARME indépendante du navigateur : le
journal de l'agent.** Remède : guetter `Target.getTargets` dès le lancement du
Bloc-notes, la cible existant dès le `window.open` quand la session met des
secondes de plus.

**e) 🔴 `lire()` n'est pas concurrent, et je l'appelais en parallèle.** Le
protocole du lecteur tient dans **un seul** couple de fichiers : deux lectures
concurrentes s'écrasent l'ordre, et les perdants **expirent**. Deux Bloc-notes
sur trois rendaient `null`, ce qui se lit **exactement** comme « fenêtre
introuvable », c'est-à-dire comme un défaut du produit. **Ce qui a tranché** :
le pré-semage réussissait sur les **trois** (`ecrit longueur=13` × 3). Lecture
séquentielle.

🔵 **Les cinq ont en commun de rendre un verdict qui se lit comme un défaut du
PRODUIT.** Aucun n'a été classé : chacun a été diagnostiqué par une seconde
voie — le journal de l'agent, le pré-semage, le contre-contrôle des motifs.

### Ce que chaque critère a relevé, en détail

**② est ATTRIBUABLE, et c'est ce qui en fait un verdict.** Collage depuis w-1 →
`deux-A` dans le Bloc-notes de **w-1 seul** ; depuis w-2 → `deux-B` dans celui
de **w-2 seul** ; **w-3, jamais collée, garde son marqueur intact** — le témoin
négatif. L'attribution passe par le `pid` de la tâche 8, **jamais** par un nonce
collé dont on regarderait quel Bloc-notes a grandi : cette voie-là est
**circulaire** (D8, `resoudreIdentite`).

**③ sur ses trois volets** : (a) **zéro** échec sur les quatre motifs, tous
vérifiés discriminants ; (b) chaque Bloc-notes porte un texte **entier**, jamais
mêlé — w-1 = `deux-A` + `trois-T1` + `BBB`, w-2 = `deux-B` + `trois-T2` + `CCC`,
**aucun caractère de l'un dans l'autre** ; (c) **ZÉRO message en retour**
(1,1,1,1 → 1,1,1,1). Le presse-papier de la VM porte **T2** : **le dernier
gagne**.

🔵 **(c) EST LA MESURE DE D-P3-6.** « Deux collages quasi simultanés » est
littéralement l'entrelacement de la course, et la seconde prise l'absorbe.
**Témoin positif** : `presse-papier de la VM` compte **2** annonces sur toute
l'exécution — la copie de ① et celle de ④ — et **aucune** produite par les
collages. Un zéro seul aurait aussi été rendu par un mécanisme mort.

**① bis mesure le legs n°3 de bout en bout, ses DEUX moitiés ensemble**, et
c'est le **seul** contrôle des deux lignes de câblage de `client/src/main.ts`,
que rien ne teste. Côté agent : `etat courant du presse-papier emis a
l'inscription` **× 1 exactement** — **jamais un fan-out**.

**④** est observé en enveloppant `navigator.clipboard.writeText` **par page** :
lire le presse-papier local ne dirait pas **laquelle** a écrit, il est partagé
entre les pages d'un même navigateur.

### Le binaire, et un contrôle qui n'était pas discriminant

Rebâti après `cargo clean --release -p proto -p agent`, `.env` sourcé.
⚠️ **Il pèse 10 658 816 octets — exactement le binaire de F3.** Le contrôle
prescrit — « vérifie la TAILLE » — se serait donc lu « la compilation n'a pas eu
lieu ». **Ce qui tranche est une CHAÎNE que moi seul ai ajoutée** :
`etat courant du presse-papier emis a l'inscription`, présente ×1 dans le
binaire. *Une taille identique n'est pas une preuve d'identité.*

⚠️ **`/dev/null` vérifié `character special file` avant la première tentative** :
aucune mesure de cette ronde n'a traversé la coupure connue de l'hôte.

⚠️ **Trois agents survivaient à la première tentative** : `Get-Process agent` a
été revérifié **avant et après chaque tentative**, y compris échouée.

### Note de lecture des journaux — RELEVÉE, pas supposée

Mesurée par `file`, `grep -lP '\x1b\['` et un balayage `tr -dc '\000'`, **après
la dernière écriture** :

| Famille | Fichiers | Ce qu'il faut faire |
| --- | --- | --- |
| les **cinq** `agent-*.log` **bruts** | ceux de la VM | `sed 's/\x1b\[[0-9;]*m//g'` — ou lire le `-plat` jumeau, **versé pour chacun** |
| **tout le reste** | les `-plat`, les JSON, les journaux de sondes et de pilote, l'`instrument/` | **rien** : ils se `grep`ent à plat |

**Dix fichiers portent des CRLF** — les cinq bruts **et** leurs cinq jumeaux,
tous venus de la VM. **Les CRLF ne gênent aucun `grep` ; seules les séquences
ANSI le font.**

✅ **AUCUN OCTET NUL, dans aucun fichier — mesuré, pas supposé.** Ce n'est PAS
la règle : les journaux de pilote de **P1** en portaient **33 à 36**, et `grep`
sans `-a` y rendait une sortie **VIDE**, pas un zéro. Ici `grep -a` n'est pas
nécessaire ; il ne nuit pas.

🔴 **ET METTRE À PLAT AVANT TOUT `grep` SUR UN CHAMP** : `tracing` sépare le nom
du champ de sa valeur par des séquences ANSI, donc `grep 'pid='` **ne matche
JAMAIS** sur un journal brut. C'est le piège de la recette d'entrée de D8 — et
c'est aussi ce qui a fait attraper le span au lieu du champ dans mon parse
(§0, défaut a).

---

## 1. La sonde S1, corrigée : c'est le TROISIÈME ARGUMENT de `window.open` qui décide

**Deux exécutions de la sonde corrigée, relevés identiques**
(`p3-focus-{1,2}.json`, `.log`). Trois armes, dans la **même exécution** et
avec le **même code de mesure** :

| arme | ce qu'elle relève |
| --- | --- |
| `window.open(url, nom)` — **LE GESTE DU PRODUIT**, relu dans `client/src/shell-page.ts:117` | **un seul `true`, et c'est la fenêtre amenée au premier plan** |
| `window.open(url, nom, 'width=800,height=600')` — une **POPUP** | `true` PARTOUT |
| `Target.createTarget` | un seul `true`, et c'est le bon |

**Avec une chaîne de caractéristiques, Chrome ouvre une POPUP ; sans, un
ONGLET.** Les popups rapportent toutes le focus, les onglets le discriminent.

❌ **LA PREMIÈRE RÉDACTION DE CETTE SONDE PASSAIT LE TROISIÈME ARGUMENT, QUE LE
PRODUIT NE PASSE PAS**, et elle a donc publié « ④ NON MESURABLE en conditions de
produit ». **C'est la recette sur la VM qui l'a réfutée** : le focus y
discrimine parfaitement, aux deux exécutions.

⚠️ **Sa « seconde arme » avait pourtant raison sur la CAUSE — le mode
d'ouverture — et tort sur laquelle était celle du produit.** Elle avait bien
empêché d'attribuer l'échec au `--headless`, ce qui aurait été faux aussi.
*Une sonde qui croit reproduire un geste doit le RELIRE, pas s'en souvenir.*

🔵 **Ce que la sonde établit malgré tout, et qui vaut** : le `--headless` n'est
pas en cause, et Xvfb/xdotool — relevés **absents** de l'hôte, consentement
donné en D8 et jamais suivi d'effet — **n'étaient pas nécessaires**. ④ est
mesurable avec l'outillage existant.

⚠️ **Corollaire, et il retire un fait annoncé** : le régime de **N écrivains
concurrents** ne s'est **PAS** présenté, puisque le focus discrimine. **Il reste
non mesuré**, et c'est un legs.

⚠️ **Ce que S1 n'établit toujours pas** : rien de Firefox, rien de Safari, rien
d'un Chromium **avec interface**, rien d'un humain.

## 2. 🔵 Le §3.3 est mesuré, et le verdict est plus fin que « vrai » ou « faux »

**Sonde S2**, un **2×2** `{hasFocus} × {userActivation.isActive}`. **Deux
exécutions, relevés identiques** (`p3-writetext-{1,2}.json`, `.log`).

| cellule | état RELEVÉ | `writeText`, message **verbatim** |
| --- | --- | --- |
| focus ✔, activation ✘ | `{hasFocus:true, isActive:false}` | `NotAllowedError: Failed to execute 'writeText' on 'Clipboard': Write permission denied.` |
| focus ✔, activation ✔ | `{hasFocus:true, isActive:true}` | **OK** |
| focus ✘, activation ✘ | `{hasFocus:false, isActive:false}` | `NotAllowedError: Failed to execute 'writeText' on 'Clipboard': Document is not focused.` |
| **focus ✘, activation ✔** | — | 🔴 **INATTEIGNABLE** |

🔴 **LA CELLULE QUI TRANCHE EST INATTEIGNABLE**, et ce n'est pas faute d'avoir
essayé : le geste de confiance **REND le focus** à la fenêtre qui le reçoit, et
`Page.bringToFront` ne le lui reprend plus. **Deux moyens** ont été joués — le
geste ordinaire, puis un geste **qui ne focalise aucun élément** —, et l'attente
porte sur le **FAIT** (vingt relectures de `hasFocus`), jamais sur une durée.

**Le §3.3 reste donc SUPPOSÉ au sens strict**, et le dire est un verdict
recevable ; **en fabriquer un autre ne le serait pas** (RP3-5).

✅ **MAIS IL EST CORROBORÉ PAR UNE PIÈCE, et cela vaut mieux qu'un « non
tranché »** : les deux refus portent le **MÊME NOM** (`NotAllowedError`) et des
**MESSAGES DIFFÉRENTS**. **Un chemin de refus PROPRE AU FOCUS existe, et il se
nomme lui-même.** Ce qui reste non mesuré est **s'il survit à une activation**.

🔴 **POURQUOI UN 2×2 ET NON UNE SIMPLE OBSERVATION** — c'est D-P3-5, et il était
juste. P2 avait **déjà** mesuré un `NotAllowedError` sur `writeText`, et ce
n'était **pas** le focus : son annexe versée relève `hasFocus: true` **des deux
côtés**, et ce qu'elle mesurait était l'**ACTIVATION**. Les deux mécanismes
lèvent la même exception, et **le nom seul ne les distingue pas** — c'est le
message verbatim qui le fait.

🔴 **ET LA PREMIÈRE RÉDACTION DE S2 A RENDU LE VERDICT INVERSE — « le §3.3 est
RÉFUTÉ » — ALORS QUE SON PROPRE RELEVÉ LE RÉFUTAIT.** Elle jouait le geste **en
dernier**, obtenait `{hasFocus:true, isActive:true}` — c'est-à-dire la cellule
**précédente** sous une autre étiquette — et en tirait sa conclusion. **C'est
l'erreur d'attribution que D-P3-5 existe pour empêcher, commise par l'instrument
écrit pour l'empêcher.**

Deux remèdes, tous deux dans le fichier :
1. l'ordre **geste → retrait du focus → écriture** ;
2. 🔴 **le verdict calculé sur l'ÉTAT OBSERVÉ, jamais sur l'étiquette voulue** —
   une cellule dont l'état ne correspond pas à ce qu'elle prétend mesurer est
   **requalifiée INATTEIGNABLE**, et le relevé le dit.

### Ce que le verdict commande, et qui était écrit d'avance (D-P3-4)

C'est la branche « `writeText` échoue sans focus » qui est corroborée. **La
règle du dépôt différé RESTE**, sa justification d'origine est confirmée sans
être établie, et **la seconde justification vaut indépendamment** :

> **À N fenêtres, le test de focus n'est pas seulement une parade à un refus :
> c'est l'ARBITRAGE qui élit l'unique écrivain local.** Le capteur pousse le
> contenu à **toutes** les fenêtres (D3), chacune a son propre
> `PressePapierLocal`, et si toutes écrivaient, **N appels concurrents à
> `writeText` partiraient pour une seule copie**, le dernier gagnant
> arbitrairement.

⚠️ **« Du coût pour rien » (spec §6.3) suppose UNE fenêtre.** Retirer la règle
signifierait « toutes les fenêtres écrivent », **régime que rien ne mesure** :
son retrait est donc une **décision du propriétaire du dépôt**, jamais une
conséquence mécanique d'un verdict de sonde.

---

## 3. 🔴 Une course trouvée par LECTURE, puis MESURÉE, puis fermée

D-P3-6. Le plan la rapportait comme une **lecture, jamais une mesure**, et
armait explicitement le cas où elle serait fausse (RP3-9 : « s'arrêter, l'écrire,
et ne PAS écrire le correctif »).

**Elle est vraie.** Le test a été **vu ROUGE sur l'arbre intact, sans aucune
mutation** — la forme la plus forte de rouge de ce dépôt : elle ne mute rien,
donc elle ne peut ni rougir pour la mauvaise raison, ni être satisfaite par un
commentaire du fichier qu'elle analyse. Pièce :
`journaux-presse-papier-p3/rouge-t5-d-p3-6-arbre-intact.log`.

```
assertion `left == right` failed
  left: Some(Texte("textB"))
 right: None
```

**L'entrelacement**, à deux fenêtres : A colle → le tour de roue **prend** le
couple et arme `reference = seqA`, `dernier_emis = textA` → **B colle** → `tour()`
lit `seqB ≠ seqA` (le garde n°1 ne mord pas) puis `textB ≠ textA` (le n°2 non
plus) → **`Annonce::Texte(textB)` part vers les N fenêtres** → au tour suivant,
`armer_les_gardes` prend `(seqB, textB)` : **trop tard**.

**Le remède, en deux étages**, comme la doctrine du dépôt le veut : la **RÈGLE**
pure et injectée (`Sondeur::ecarter` / `ecarter_notre_ecriture`) et sa
**BRANCHE** sur le registre (`filtrer_nos_ecritures_tardives`), appelée depuis
`registre.rs` **entre `tour()` et `distribuer`**.

⚠️ **LE RÉSIDU EST ÉCRIT DANS LE CODE.** `ecrire_avec` écrit le presse-papier
**PUIS** pose `notre_ecriture` — le verrou y est délibérément pris **après**
l'E/S Win32, parce que le tenir autour d'`OpenClipboard` bloquerait l'attache et
le retrait de **toutes** les fenêtres. Si `tour()` lit dans cet intervalle, la
seconde prise ne trouve rien. **Le remède RÉTRÉCIT la fenêtre, il ne la ferme
pas** — du même genre que le résidu qu'`apres_notre_ecriture` déclare déjà
accepté.

⚠️ **Ce n'est PAS un défaut créé par P3** : à une fenêtre, deux collages en
moins de `PERIODE_PRESSE_PAPIER` (250 ms) le produisent aussi. P2 ne l'a pas
rencontré, ses quatre collages étant espacés de plusieurs secondes.

🔵 **`PRESSE_PAPIER_GARDE=0` désarme AUSSI cette prise**, et il le faut : ce
bras de banc existe pour rendre atteignable la rouge du critère ④ de P2, qui
compte les messages revenant vers la fenêtre après un collage. Une seconde prise
qui mordrait quand même le viderait de son sens. Un test le tient.

### 🔴 Un défaut du plan, signalé et corrigé plutôt que recopié

Son Step 1 prescrit un test de **deux lignes** — `armer(true, seqA, textA)` puis
`observer(seqB, || Some(textB))` — « vu ROUGE sur l'arbre intact ». **Il l'a
été.**

⚠️ **Mais ces deux lignes seules ne peuvent JAMAIS devenir vertes, et le plan ne
l'avait pas vu** : le remède qu'il tranche lui-même est un **POST-FILTRE**, qui
court **après** `tour()` sur son résultat. `observer` ne peut pas connaître une
écriture arrivée après lui ; exiger qu'il rende `None` serait exiger qu'il
devine.

**La lettre du test est conservée** — ses deux premières lignes sont celles qui
ont rougi — **et une ligne lui est ajoutée** : la seconde prise, appliquée au
résultat.

---

## 4. 🔴 Trois harnais pris en défaut — le résultat de méthode du sous-bloc

**a) `git checkout --` restaure à HEAD, pas à l'état d'avant la mutation.**
La première rouge a donc **EFFACÉ le correctif non commité** que les rouges
suivantes devaient éprouver ; celles-ci se sont arrêtées sur « ancre
introuvable », c'est-à-dire sur **le seul symptôme visible d'un travail perdu**.
Le contrôle de `sha256` a bien crié « DIVERGENT » — **mais après la perte**.
**Une rouge se restaure depuis une COPIE NOMMÉE.**

**b) L'étape « la mutation a-t-elle changé quelque chose ? » était VACUEUSE.**
Elle exigeait un `git diff --numstat` **non vide** — mais il compare à **HEAD**,
donc il reste non vide tant qu'un correctif non commité vit dans le fichier,
**quelle que soit la mutation, et même s'il n'y en a aucune**. Le contrôle censé
refuser une rouge qui ne mute rien **ne pouvait pas échouer**. Il compare
désormais à la copie nommée, et une **ROUGE 0** — une mutation qui ne mute
rien — est jouée **pour le voir refuser** : il refuse.

> C'est « un contrôle qu'on n'a jamais vu rouge n'est pas un contrôle »,
> appliqué **au contrôle lui-même**.

**c) 🔴 Une rouge est restée VERTE, et elle a été DIAGNOSTIQUÉE plutôt que
classée.** Elle remplaçait `take()` par `clone()` dans
`filtrer_nos_ecritures_tardives` : le diff n'était pas vide, et le test restait
vert. Cause, trouvée en **rejouant la mutation à la main** (`sed`, qui remplace
toutes les occurrences) : **cette ligne existe DEUX FOIS dans le fichier** —
`armer_les_gardes` la porte aussi, et elle vient **en premier** —, et le
`replace(…, 1)` a frappé la mauvaise fonction. Rejouée **ancrée sur la
SIGNATURE**, elle rougit.

C'est le §2.2 règle 3 du plan payé **sur une ligne qui n'avait rien d'un
commentaire** : la seule **duplication** suffit.

🔴 **LA LEÇON EST NEUVE : UNE ROUGE QUI RESTE VERTE SE DIAGNOSTIQUE, ELLE NE SE
CLASSE PAS.** Sans le rejeu manuel, ce relevé se serait lu comme « le test ne
peut pas échouer » — **l'inverse exact de la vérité**.

**d) Un piège mineur, relevé pendant la revue transverse** : `grep -c` avec des
alternatives `\|` compte des **lignes**, pas des motifs. Il a rendu « 3 » pour
**six** éditions bel et bien présentes. Les places ont donc été relues **une par
une, chacune avec son propre motif**.

---

## 5. Les douze rouges, et ce qu'elles tiennent

Chacune nomme **l'assertion qui a rougi et ce qu'elle a rendu**, jamais le seul
code de sortie. Journaux : `rouge-t5-…`, `rouges-t5.log`, `rouges-t6.log`,
`rouges-t7.log`.

| # | Ce qu'elle mute | Ce qui tombe |
| --- | --- | --- |
| — | **rien : l'arbre intact** | la course de D-P3-6 elle-même |
| 0 | **rien** (contrôle du harnais) | le harnais REFUSE, comme il doit |
| A | le filtre porte sur le `seq` et non le texte | une copie **TIERCE** cesserait d'être annoncée |
| B | le filtre s'applique aussi à `Annonce::Refus` | l'utilisateur perdrait le bandeau |
| C | le garde `if !armes` est retiré | `PRESSE_PAPIER_GARDE=0` cesserait de désarmer |
| D | la branche **lit** le couple sans le **consommer** | il serait rejoué au tour suivant |
| E | l'émission à l'inscription n'existe pas | **c'est l'arbre d'avant, donc le legs n°3 lui-même** |
| F | l'émission fait un **fan-out** | un aller-retour par attache |
| G | on ne mémorise que les `Texte` | un refus ne serait jamais rejoué |
| H | on émet un message **VIDE** quand la mémoire est `None` | le client écrirait une chaîne vide à chaque attache |
| I | on **purge** la mémoire à la ré-inscription | 🔵 **D-P3-3** — le rattachement ne rejouerait rien |
| J | le paramètre `initial` n'existe pas | **3 tests** tombent : c'est l'arbre d'avant P3 |
| K | le montage écrit **directement**, hors du dépôt différé | **2** tombent, dont le refus sans bandeau |
| L | un `Recu` vide est rejoué quand `initial` est absent | le comportement d'avant P3 ne serait plus préservé |

🔵 **LA ROUGE I EST CELLE QUI ENSEIGNE LE PLUS** : elle établit que D-P3-3
n'était pas un scrupule théorique. Copier la purge de `dernieres_parts` **par
symétrie de forme** fait tomber le rattachement — **le cas où le rejeu est le
plus utile**. La symétrie est trompeuse parce que ces deux-là se purgent pour
une raison que le presse-papier n'a pas : leur distribution **FILTRE** sur eux,
la sienne est **inconditionnelle**.

---

## 6. Ce que le code livre

| Étage | Fichier | Nature |
| --- | --- | --- |
| la règle de la seconde prise | `agent/src/presse_papier/sondeur.rs` | **pure, aucun `cfg`**, état du garde **injecté** — c'est ce qui rend le bras désarmé éprouvable sur l'hôte |
| sa branche | `agent/src/capteur/sommeil/presse_papier.rs` | verrou pris et rendu, **ne couvre aucune E/S** |
| la mémoire de l'état courant | `agent/src/capteur/sommeil/registre.rs` | `Etat::dernier_presse_papier`, **aucune purge** (D-P3-3) |
| son émission | `emettre_l_etat_courant` | **le seul canal neuf**, jamais un fan-out |
| l'attribution | `agent/src/capteur/fenetre.rs` | **une ligne** : `pid` dans `fenêtre attachée au capteur` |
| la règle du rejeu au montage | `client/src/presse-papier-dom.ts` | paramètre **facultatif** `initial?: Recu`, rejoué **par le chemin qui existe déjà** |
| le câblage | `client/src/main.ts` | **deux lignes**, patron `micAnnonce` — 🔴 **non testées, et déclarées telles** |

**Deux extractions préalables**, jouées **AVANT** les additions qui les rendent
nécessaires : `presse_papier.rs` **441 → 278** et
`capteur/sommeil/presse_papier.rs` **379 → 241**. Transpositions **vérifiées
caractère pour caractère**, et la désindentation de quatre espaces vérifiée
**réversible**.

🔴 **UN PLAFOND A ÉTÉ FRANCHI QUAND MÊME**, et c'est mon erreur :
`agent/src/capteur/sommeil/tests.rs` est monté de **417 à 562**. E13 du plan
l'annonçait au-delà de 480, et **la mesure n'a pas été prise d'avance**.
**L'extraction est jouée, jamais une compression**, et le fichier retombe à
**417** — sa taille exacte d'avant. Le point de chute n'est pas de commodité :
ces tests exercent `emettre_l_etat_courant`, ils vont donc auprès d'elle.

Comptes, **annoncés avant d'être lus** : `cargo test -p agent` **904 → 916**,
`cargo test -p proto` **109** (inchangé), `client` **462 → 466** / 40 fichiers,
`proto/ts` **296** (inchangé). ⚠️ **`cd client && npx vitest run` NE COUVRE PAS
`proto/ts/`.** `./scripts/verify-all.sh` depuis un shell **propre** :
**sortie 0**, **dix** étapes du script, **dix-huit** en-têtes `==>` à l'écran.

✅ **`proto/` n'a pas bougé d'une ligne** (D-P3-13, vérifié par
`git diff --stat … -- proto/`, sortie **VIDE**), et **aucune variable
d'environnement n'est introduite** — `scripts/run-agent.sh` n'est pas modifié.

---

## 7. La revue transverse — huit affirmations, neuf places

Barème du dépôt : 5 en D7, 3 en D8, 6 en D9, douze en D10, sept en D11, huit en
P1 de la plateforme, dix en P2, cinq en S1, neuf au chantier E, douze en P3 de
la plateforme, douze en S2, onze en F1, huit en P4, treize en S3, huit en G1.

Les places ont été **énumérées par `grep -rniE` avant toute édition, et relues
place par place après**. Le détail vit dans le message du commit `9ab9b8f` ; les
trois qui enseignent :

- `agent/src/presse_papier/sondeur.rs` et `…/tests.rs` — « une fenêtre qui
  s'attache ne reçoit donc pas le contenu déjà présent », **réfutée par la
  tâche 6**, à **deux** places. ⚠️ Ce qui reste vrai est la propriété **du
  champ** : le `Sondeur` n'annonce toujours rien à son premier tour ; c'est le
  **registre** qui rejoue ;
- `apres_notre_ecriture` — sa réserve était **incomplète** : elle ne traite que
  la copie **tierce**, qu'elle déclare voulue, et le cas de **notre propre
  seconde écriture** n'était déclaré nulle part ;
- le plan de P2 §8 — « c'est le legs n°3 de P1, et il appartient à P3 » : **fait**,
  et il n'en nommait **qu'une moitié**.

Les documents datés (plans de P1 et P2, résultats de P2) sont **annotés, jamais
réécrits** : un relevé daté reste vrai comme histoire, et ce sont les
**pronostics** qu'on reprend.

⚠️ **La revue transverse est elle-même une source de croissance, et elle l'a
été** : `agent/src/presse_papier/tests.rs` passe à **477**, marge **23**.
Déclaré ; aucune porte n'est franchie, et rien n'est comprimé.

---

## 8. Ce que P3 n'établit PAS

- **Aucun taux, nulle part.** Deux exécutions par critère, deux par sonde, une
  par rouge.
- ⚠️ **Les deux lignes de câblage de `client/src/main.ts` ne sont couvertes par
  AUCUN test d'hôte**, et ne peuvent pas l'être. Leur seul contrôle est le
  critère ① bis — une fenêtre attachée APRÈS la copie —, **et il a été joué,
  deux fois** : RP3-17 est **écarté**, mais la couverture reste une mesure de
  bout en bout, jamais un test.
- 🔴 **Rien au-delà de TROIS fenêtres** — c'est le nombre que la VM a rendu ce
  jour-là, relevé et non exigé.
- 🔴 **Le site d'appel de `registre.rs`** n'est couvert par aucun test d'hôte :
  il vit dans le fil du tour de roue.
- 🔴 **Le régime de N ÉCRIVAINS CONCURRENTS ne s'est PAS présenté**, puisque le
  focus discrimine : **il reste non mesuré**, et c'était l'inquiétude que S1
  avait fait naître à tort.
- **Le §3.3 reste SUPPOSÉ au sens strict** — corroboré, non établi.
- **Le régime de N écrivains concurrents n'est mesuré par rien.**
- **La borne de 12 s de `commander` n'a toujours pas couru** (legs n°5 de P2) :
  P3 est le premier à pouvoir la mettre sous contention réelle, **et il ne l'a
  pas fait**.
- ⚠️ **Le pilote a porté CINQ défauts, tous trouvés en le jouant** (§0), et sa
  première exécution a bien été son premier débogage. **Trois exécutions sont
  versées comme DISQUALIFIÉE ou DIAGNOSTIC** : elles ne comptent pour aucun
  verdict, et elles sont les pièces des diagnostics.
- **Rien de la latence** d'un collage ni d'une copie.
- **Rien d'un texte non-ASCII ni multi-ligne à N fenêtres** : les textes de la
  recette sont ASCII et d'une seule ligne. Legs n°10 de P1, reconduit.
- **Rien du refus de taille à N fenêtres** : les deux bornes ne sont éprouvées
  que par leurs tests d'hôte (legs n°8 de P2).
- **La borne de 12 s de `commander` n'a toujours pas couru** : ses motifs sont
  désormais les BONS et vérifiés discriminants, et ils rendent **zéro** — mais
  un zéro sur des collages qui aboutissent tous ne dit rien de la borne.
- **Aucun taux, nulle part.** Deux exécutions par sonde, une par rouge. **Deux
  exécutions établissent la reproductibilité d'un mécanisme déterministe, jamais
  une fréquence.**
- **Rien d'un navigateur autre que Chromium**, rien avec interface, **rien d'un
  humain**, rien du HiDPI, rien de la latence.
- **Le niveau 2 du sens VM → navigateur reste NON MESURABLE** (legs n°11 de P1) :
  `xclip` et `wl-paste` sont **absents** de l'hôte, `xsel` refuse en « Can't
  open display ».
- **`PRESSE_PAPIER_MAX` et `PERIODE_PRESSE_PAPIER` ne sont toujours pas
  calibrées**, et elles rejoignent la liste que ce dépôt tient depuis `BPP_MIN`.
- **Aucun texte non-ASCII ni multi-ligne n'a traversé la chaîne réelle** — legs
  n°10 de P1, reconduit.

---

## 9. Ce que P3 lègue

**Legs de P1 réglé** : n°3 (l'état courant à l'attache — **les DEUX moitiés**),
🔴 **fermé SUR PIÈCES et jamais mesuré de bout en bout**.

**Ce qui reste dû :**

1. 🔴 **Le régime de N ÉCRIVAINS CONCURRENTS n'est mesuré par rien**, et il ne
   peut PAS l'être tant que le focus discrimine — c'est-à-dire tant que la
   page-shell ouvre des ONGLETS. Le provoquer exigerait de lui faire ouvrir des
   POPUPS, c'est-à-dire de changer le produit pour mesurer un régime qu'il n'a
   pas. **Nommé, non mesuré.**
2. 🔴 **Rien au-delà de TROIS fenêtres**, et rien de la latence.
3. 🔴 **Le résidu de D-P3-6** : `ecrire_avec` pose `notre_ecriture` **après**
   l'E/S Win32, et la seconde prise ne ferme pas cet intervalle. Le fermer
   demanderait de tenir le verrou autour de l'E/S — ce que
   `capteur/sommeil/presse_papier.rs` **interdit nommément**, et pour une bonne
   raison. **Écrit, non fermé.**
4. ⛔ **Le retrait de la règle du dépôt différé**, si le §3.3 devait être réfuté
   un jour : **décision du propriétaire du dépôt** (D-P3-4), avec son coût
   nommé — un régime que rien ne mesure.
5. ⛔ **Le canal `Message` reste NON BORNÉ**, et P3 l'aggrave **d'un message par
   attache**. Le borner est un **changement de conception** : bloquer serait le
   pire, le seul écrivain étant le tour de roue, **sous le verrou global**
   (D-P3-11).
6. ⛔ **Le propriétaire MONO-FENÊTRE n'existe toujours pas** (legs n°1 de P1).
   Point de chute : `agent/src/demarrage.rs`, **426 lignes, marge 74**.
7. ⛔ **La borne de 12 s de `commander` n'a toujours pas couru** (legs n°5 de P2).
8. ⛔ **La rouge du critère ③ de P2 n'est toujours pas jouée** — elle exige un
   second binaire, et **P3 ne la joue pas non plus** (legs n°1 de P2). La
   justification de D6 reste donc **non éprouvée**.
9. ⛔ **Le critère ⑤ de P2 reste NON MESURÉ**, et **le niveau 2 du sens
   VM → navigateur avec lui** : `Xvfb` + `xdotool`, consentement donné en D8,
   jamais suivi d'effet.
10. ⚠️ **`agent/src/presse_papier/tests.rs` est à 477, marge 23** : toute
    addition y appelle une **EXTRACTION**, jamais une compression.

**Et pour A1 — la couleur d'accent — P3 ne lègue RIEN**, parce qu'il n'y a
touché à rien : A1 est hors périmètre entier (spec §5, §6.4), il n'a pas de
plan, et **aucun fichier de P3 ne l'approche**. ⚠️ Ce qui le concerne
indirectement est le §2 ci-dessus : `Capabilities` et le canal de contrôle sont
les chemins qu'A1 empruntera, et **P3 n'y a ajouté aucune variante de
protocole** — `CONTROL_VERSION` ne bouge pas, `proto/` n'a pas bougé d'une
ligne.
