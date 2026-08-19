# Sous-bloc D11 — solder les legs : résultats (19 août 2026)

Plan : `docs/superpowers/plans/2026-08-19-multifenetres-solder-les-legs.md`
(commit `0a172a9`).
Conception : `docs/superpowers/specs/2026-08-19-multifenetres-solder-les-legs-design.md`
(commit `75f7ae7`).
Journaux : `docs/superpowers/plans/journaux-multifenetres-d11/` — **113 fichiers
suivis par git** (92 au premier niveau, plus 21 sous `instrument/`).

Quinze tâches, de `75f7ae7` (la conception) à la présente. Le code de production
que ce sous-bloc change tient en **deux lignes** ; tout le reste est de
l'instrument, de la mesure et du document.

> ⚠️ **Pourquoi ce document existe.** Le leg n°3 de D10 est *perdu* — six
> constats de revue de D9 vivaient dans `.superpowers/sdd/`, gitignoré, et
> n'existent plus nulle part. La contre-mesure est celle que D10 a instaurée et
> que D11 tient : **toute affirmation que le dépôt tiendra d'une mesure de ce
> sous-bloc est portée ici**, où elle survivra, et les pièces brutes sont
> versées sous `journaux-multifenetres-d11/` sans dépendre d'aucun rapport de
> tâche.

---

## 0. Note de lecture des journaux — deux familles, relevées par la commande

`file`, `grep -acF` sur `ESC[`, et un balayage `tr -dc '\000'` sur les 92
fichiers du premier niveau :

| Famille | État relevé | Ce qu'il faut faire |
| --- | --- | --- |
| les 27 `agent-*.log` **bruts** | UTF-8, **CRLF**, **séquences ANSI PRÉSENTES** (89 lignes sur `agent-critere-1-1.log`, 1081 sur `agent-cout-3.log`) | `sed 's/\x1b\[[0-9;]*m//g'` — ou lire le `-plat` jumeau, **versé pour chacun** |
| tous les `*-plat.log`, les `*.json`, les `*.jsonl`, les `*-analyse.log` et le `.diff` | UTF-8, **ANSI déjà retirées** (0 ligne `ESC[`) ; les `-plat` gardent le CRLF, les autres non | rien |

✅ **AUCUN octet NUL dans aucun fichier**, vérifié par balayage sur les 92 — à
la différence de D10, dont `agent-ab-desarme-1.log` en portait 558 et faisait
rendre à `grep` une sortie **vide** indiscernable d'un zéro. `grep -a` reste
employé partout par prudence, il n'était pas nécessaire ici.

⚠️ **Contrôle positif du `ERROR = 0` qui revient partout dans ce document** :
`grep -al 'WARN' *-plat.log` rend **26** fichiers, `grep -al 'ERROR' *-plat.log`
en rend **0**, et la seule occurrence de la chaîne `ERROR` du répertoire est
`instrument/montage-d11.mjs:161` — **le compteur de l'instrument lui-même**, qui
aurait donc rapporté des `ERROR` s'il y en avait eu. **Le zéro est réel, pas un
artefact d'encodage.**

---

## 1. Ce que D11 fait, en trois phrases

**Le son revient au cas majoritaire.** D10 laissait son remède de
reconstruction audio **inerte en mono-fenêtre** — une application, une fenêtre,
le cas de très loin le plus fréquent : la capture était bien refabriquée, puis
le réarmement `set_actif(self.audio_porteuse)` la faisait taire, `audio_porteuse`
naissant `false` et n'ayant pour écrivain qu'un ordre du capteur qu'un agent
mono-fenêtre ne reçoit jamais. **Une ligne** — `set_audio_porteuse(true)` dans la
seule branche `None` de `demarrage/audio.rs::brancher` — referme le leg 4, et la
recette ① le mesure : **441 Hz reçus au vert contre la sentinelle au rouge**.

**Deux boutons de banc rendent atteignables des chemins que D10 n'a pas pu
exercer.** `AUDIO_FAUTE_RECONSTRUCTION` fait échouer les *n* prochaines
reconstructions ; `AUDIO_FAUTE_LECTURE_MS` borne dans le temps l'injection de
fautes de lecture existante. Ensemble, ils font tomber le critère ④ de D10 —
« une capture irrécupérable retombe sur la promotion d'une voisine » — que D10
déclarait *non démontrable par le protocole prescrit*.

**Deux legs de mesure tombent, un troisième pas.** La **séparation des flux**
entre fenêtres est prouvée par un marqueur d'identité **invariant dans le
temps**, avec son rouge joué sur la VM (leg 8) ; le **coût de la duplication
surdimensionnée** est mesuré et n'est **pas détectable** à ce montage (leg 7).
Le **maillon fautif du `Resize`** (leg 2), lui, **reste non identifié** : le
silence de D9 n'a pas été reproduit — issue que la conception déclarait possible
d'avance.

---

## 2. Les six critères, avec leur nombre d'exécutions

**Aucun taux n'est revendiqué nulle part.**

| # | Critère | Verdict | Exéc. |
| --- | --- | --- | --- |
| ① | En mono-fenêtre, une capture reconstruite redevient audible (leg 4) | **TENU** — 441 Hz / −39,8 à −39,9 dB au vert, sentinelle −1000 dB au rouge | 2 vertes + 2 rouges (⚠️ **1 seule rouge exploitable au spectre**, §3.3) |
| ② | À deux fenêtres d'un même PID, la voisine reconstruite se tait | **TENU sur ses deux moitiés** — mais **le ROUGE prescrit est VACUEUX, et c'est MESURÉ** | 2 vertes + 1 rouge |
| ③ | Une capture irrécupérable retombe sur la promotion (leg 5, et critère ④ de D10) | **TENU** — la voisine promue émet réellement | 2 vertes + 1 rouge + 2 de diagnostic |
| ④ | Deux fenêtres montrent deux flux distincts (leg 8) | **TENU** — 10 marqueurs distincts, 0 collision | 2 vertes + 1 rouge |
| ⑤ | Le coût de la duplication surdimensionnée (leg 7) | **A/B À DEUX BRAS NON PRIS** par sa propre règle d'admission ; **mesuré en apparié** — coût **non détectable** | 4 exploitées + 1 dont la sonde de pose a échoué |
| ⑥ | Le maillon fautif du `Resize` (leg 2) | **SILENCE NON REPRODUIT** — maillon **toujours non identifié** | 3 interprétées + 1 écartée + 1 contrôle rouge/vert |

---

## 3. Critère ① — en mono-fenêtre, le son revient (leg 4 de D10)

Recette de la tâche 9, commit `9511aef`. Montage **mono-fenêtre strict** : ni
`SUPERVISEUR`, ni `CAPTEUR`, ni `FENETRE_HWND` ; une fenêtre Chrome `--app`
jouant `ton.html?hz=440` ; `AUDIO_FAUTE_LECTURE=15`, soit plus que
`LECTURES_ECHOUEES_MAX = 10` ; session de 120 s ; points spectraux à t+30 s et
t+105 s. Binaires : **`18d9591`, 9 356 288 octets** (vert) et **`df03fc6`,
9 335 296 octets** (rouge, l'état de `main` avant le correctif).

### 3.1 Les comptes agent — relevés par `grep -ac` sur les `-plat.log`

| Journal | Bras | `capture audio reconstruite` | `compteurs audio` | dont `actif=true` | fautes consommées | `ERROR` |
| --- | --- | --- | --- | --- | --- | --- |
| `agent-critere-1-1-plat.log` | VERT | **1** | 4 | **4** | **15** | 0 |
| `agent-critere-1-2-plat.log` | VERT | **1** | 4 | **4** | **15** | 0 |
| `agent-rouge-1-plat.log` | ROUGE | **1** | 4 | **0** | **10** | 0 |
| `agent-rouge-2-plat.log` | ROUGE | **1** | 4 | **0** | **10** | 0 |

**Le rouge a la FORME prescrite** : le mécanisme se déclenche — `capture audio
reconstruite` vaut **1 des deux côtés** — et le son reste absent. Un rouge où la
reconstruction ne partirait pas serait **vacueux** : il rendrait les mêmes zéros
sur un binaire au remède parfait.

🔵 **Le compte de fautes est un témoin d'armement INDÉPENDANT du spectre**, et
c'est l'instrument que D10 avait trouvé en relisant son propre code : *une
source muette ne peut pas consommer de faute*, le garde `if !emettait` de
`windows_audio/fil.rs` précédant l'injection. **10 au rouge** — la source
reconstruite n'émet pas, elle s'arrête sur les dix fautes du fil d'origine
(`agent-rouge-1-plat.log:24`, `capture arrêtée définitivement … consecutives=10`).
**15 au vert** — elle émet, et brûle les cinq fautes restantes avant de lire pour
de vrai (`agent-critere-1-1-plat.log:42`, `consecutives=5`).

### 3.2 Le spectre reçu

| Journal | dominante | niveau (t+30 / t+105) | plancher | 660 Hz |
| --- | --- | --- | --- | --- |
| `critere-1-1.json` | **441 Hz** | −39,8 / −39,8 dB | −159,7 / −159,6 | −108,5 / −120,7 |
| `critere-1-2.json` | **441 Hz** | −39,9 / −39,9 dB | −159,4 / −159,5 | −114,2 / −119,9 |
| `rouge-2.json` | 0 Hz | **−1000 / −1000** (sentinelle) | −1000 | −1000, `muted=true`, `stats_audio: null` |

**L'ordre est établi, pas supposé.** Reconstruction à `11:01:42.628944Z`
(`agent-critere-1-1-plat.log:29`) et `11:04:28.744043Z`
(`agent-critere-1-2-plat.log:29`) ; premières mesures à `11:02:10.951Z` et
`11:04:57.078Z`. **Toutes les mesures sont postérieures à la reconstruction** :
ce n'est pas la capture d'origine qu'on entend.

### 3.3 ⚠️ Deux réserves que la vérification des journaux a trouvées

- 🔴 **La preuve SPECTRALE du bras rouge repose sur UNE exécution, pas deux.**
  `rouge-1.json` n'est pas une mesure exploitable : c'est l'exécution dont
  l'instrument était cassé. Elle ne porte pas un relevé ciblé mais **16 relevés**
  balayant toutes les cibles CDP — dont dix « aucune RTCPeerConnection », deux
  « Session with given id not found », deux `window is not defined` — et, pour la
  seule bonne URL, `dominante_hz: 0, niveau_db: null` : **la sentinelle −1000 est
  absente**, perdue par la sérialisation de `-Infinity`. Les **comptes agent**
  des deux rouges, eux, sont bien deux (tableau du §3.1). *Le message de commit
  dit « la sentinelle −1000 dB aux deux points rouges » : ce sont les deux points
  de contrôle d'**une** exécution, lecture correcte que le tableau à deux lignes
  invite à confondre.*
- 🟡 **Un compte d'anomalies d'instrument est inexact dans le message de
  commit** : « trois `window is not defined` sur huit relevés ». Mesuré :
  **2 sur 16**. Le fait rapporté — l'instrument évaluait aussi les cibles
  `worker` — est vrai et vérifiable ; **ni son numérateur ni son dénominateur ne
  tombent.**

### 3.4 Cinq défauts d'instrument trouvés PAR L'EXÉCUTION du bras rouge

Tous corrigés, et le premier **annulait la mesure** : `ctx.resume()` jamais
appelé ni `ctx.state` vérifié — *un contexte audio suspendu rend `-Infinity`
exactement comme un silence réel, donc le contrôle ne pouvait pas distinguer les
deux* ; aucun repli sur `<video>.srcObject` ; `-Infinity` sérialisé en `null`,
donc perdu du journal versé ; les niveaux aux fréquences **assignées** non
relevés ; l'évaluation lancée aussi sur les cibles `worker`.

✅ **L'instrument a ensuite été vu rendre LES DEUX valeurs, sur l'hôte et sans la
VM** : `controle-spectre-d11.html` à gain 0,25 rend 441 Hz / −39,9 dB, à gain 0
rend la sentinelle, `etat_ctx=running` aux deux.

---

## 4. Critère ② — à deux fenêtres d'un même PID, la voisine se tait

Recette de la tâche 10, commit `3f39656`. Deux fenêtres Chrome `--app`
partageant **un seul** `--user-data-dir`, `AUDIO_FAUTE_LECTURE=15`, session de
150 s, points spectraux à t+45 s et t+120 s **pris dans la même fenêtre de
temps** (horodatages versés).

**Le groupe de PID unique est relevé par deux voies indépendantes** : les deux
lignes `audio activé` d'une même exécution portent le **même** `pid` — **1880**
(`agent-critere-2-1-plat.log:47` et `:48`) puis **5872**
(`agent-critere-2-2-plat.log:50` et `:53`) — et `Get-Process chrome` sur la VM le
confirme.

| Journal | porteuse `w-2` (440 Hz) | voisine `w-1` (660 Hz) |
| --- | --- | --- |
| `critere-2-1.json` | **441 Hz à −39,9 / −39,8 dB**, 660 Hz à −40,2, `bytesReceived` 757 182 → 1 989 315 | **−1000 dB aux deux fréquences**, `muted=true` |
| `critere-2-2.json` | **441 Hz à −39,9 / −39,8 dB**, 660 Hz à −40,2, 720 898 → 1 952 929 | **−1000 dB**, `muted=true` |

⚠️ **La porteuse entend 440 ET 660**, et c'est le fait structurel de D7, pas un
défaut : `INCLUDE_TARGET_PROCESS_TREE` capture l'**arbre de processus**, jamais
la fenêtre. Deux fenêtres d'une même application ne peuvent pas avoir un son
séparé ; le focus ne choisit que **laquelle des deux reçoit le flux partagé**.

### 4.1 🔴 Le ROUGE prescrit par le plan est VACUEUX — et c'est MESURÉ, pas seulement raisonné

Un binaire **délibérément défectueux** a été bâti pour cette seule mesure et
**jamais fusionné** : `set_actif(true)` inconditionnel à la place de
`set_actif(self.audio_porteuse)`, une seule ligne, diff versé dans
`rouge-2-provenance.diff`. **Il rend EXACTEMENT le même relevé que le vert** —
voisine à −1000 dB, porteuse à 441 Hz, 1 reconstruction, 15 fautes, 5/10
`actif=true`, 0 `ERROR`.

**La raison était lisible dans le code avant la mesure** : le garde
`if !emettait { continue; }` de `windows_audio/fil.rs` **précède** l'injection.
Une voisine **muette** ne consomme donc aucune faute, ne meurt jamais, n'atteint
jamais `reconstruire_ou_signaler` — et la ligne que le rouge modifie **n'y court
jamais**. Avoir deux fenêtres ne suffit pas : il faudrait une session **non
porteuse au moment de sa reconstruction**, état qu'aucun réglage de ce montage ne
produit.

⛔ **Conséquence à ne pas perdre : la discrimination entre le correctif livré et
la version que le code déclare *pire* n'est portée QUE par le test d'hôte
`une_session_non_porteuse_reconstruite_reste_muette`.** C'est exactement la
lacune que D10 s'était reprochée à son §9, et **D11 ne la comble pas** : il
établit *pourquoi* elle ne se comble pas par ce montage.

### 4.2 Le couplage réélection / répit n'est pas exercé — par ②

`ordre audio appliqué … actif=true` apparaît **une** fois par exécution aux
trois exécutions de ②, donc **zéro réélection**. Le plan attendait de l'exercer
ici ; il ne l'est pas.

🔵 **Mais il l'est par ③, et aucun message de commit ne le relève** :
`agent-critere-3-1-plat.log` porte cinq réélections après répit — lignes 118,
131, 144, 156 et 164, toutes `ordre audio appliqué session=w-2 actif=true
capture_morte=true` — et `ordre audio appliqué` y vaut **26** par exécution
contre 2 en ②. **Le couplage est donc EXERCÉ par le sous-bloc**, ce qui n'était
pas acquis. ⚠️ Il reste **exercé, pas BORNÉ** : rien ne mesure le retard qu'il
inflige au fil de drainage.

### 4.3 ⚠️ Deux réserves de méthode

- 🔴 **`rouge-2-recette2.json` porte `assignations = []` aux deux points** : le
  hameçon qui lit les annonces `fenetre-ouverte {session, titre}` du produit n'a
  rien capturé, et le rôle porteuse/voisine y est donc attribué **par rang de
  nom** — la pratique exacte que le message de commit déclare avoir bannie
  (« jamais un rang de nom »). Le verdict « rouge vacueux » ne dépend pas de
  cette attribution (les comptes agent suffisent), **mais la garantie de méthode
  annoncée ne couvre pas cette exécution.**
- 🟡 **Les tailles de binaire du bras rouge (9 357 312 contre 9 356 288 octets)
  ne sont étayées par AUCUNE pièce du répertoire** : elles n'existent que dans le
  message de commit, et `rouge-2-provenance.diff` n'en porte pas. *La provenance
  du binaire défectueux est donc affirmée, pas versée.*

### 4.4 Deux défauts d'instrument corrigés en cours de route

`--fenetres=hz:profil` découpait sur **tous** les `:`, donc un chemin Windows
`C:\dev\…` donnait `profil = 'C'` et la fenêtre ne s'ouvrait pas — **sans autre
symptôme que l'absence d'annonce**. Et l'ordre shell → superviseur → fenêtres
faisait capturer la **console PowerShell de la tâche planifiée**
(`fenetre-ouverte w-1 powershell.EXE`) au lieu des fenêtres visées ; les fenêtres
s'ouvrent désormais **avant** le superviseur, qui les trouve par
`enumerer_existantes`.

---

## 5. Critère ③ — le repli sur la promotion (leg 5 de D10, et son critère ④)

Recette de la tâche 11, commit `8e81fb6`. **C'est le critère que D10 déclarait
« non démontrable par le protocole prescrit »** — son protocole tuait l'arbre de
processus cible, donc la fenêtre avec l'audio. `AUDIO_FAUTE_RECONSTRUCTION` est
la voie que D10 avait nommée sans la construire.

Réglages du vert : `AUDIO_FAUTE_LECTURE=15`, `AUDIO_FAUTE_LECTURE_MS=5000`,
`AUDIO_FAUTE_RECONSTRUCTION=30`.

| Journal | fautes RECONSTR. | reconstruite | réarmement programmé | abandon définitif | `compteurs` / `actif=true` | `ERROR` |
| --- | --- | --- | --- | --- | --- | --- |
| `agent-critere-3-1-plat.log` | **18** | **0** | **5** | **1** | 6 / 6 | 0 |
| `agent-critere-3-2-plat.log` | **18** | **0** | **5** | **1** | 6 / 6 | 0 |
| `agent-rouge-3-plat.log` (injection désarmée) | **0** | **1** | **0** | **0** | 12 / 6 | 0 |

**État final relevé** (`agent-critere-3-1-plat.log:172` puis `:173`/`:174`) :
`capture audio morte et abandon définitif : le groupe de PID restera muet
session=w-2 rearmements=5`, puis `ordre audio appliqué session=w-1 actif=true` et
`session=w-2 actif=false capture_morte=true`.

**Et la voisine promue ÉMET RÉELLEMENT** — c'est tout l'enjeu : `critere-3-1.json`
donne `w-1` à **441 Hz / −39,9 dB aux quatre points**, `bytesReceived` croissant
de **1 091 901 à 2 243 534**, pendant que `w-2` est à −1000 dB, `muted=true`.
Le rouge montre l'issue **inverse** : pas de promotion, la porteuse conserve le
son (441 Hz sur `w-2`, −1000 dB sur `w-1`).

### 5.1 🔴 DEUX valeurs du plan sont RÉFUTÉES, et par la mesure

**① `AUDIO_FAUTE_LECTURE_MS=3000` rend le critère INATTEIGNABLE.** Le plan
situait la mort de la porteuse à t+0,05 s ; elle survient bien plus tard, parce
que **le budget d'injection prend son origine au démarrage du fil, pas à
l'élection**, et que le premier arbitrage du capteur met ~2,7 s à arriver.
Mesuré sur `agent-critere-3-fenetre-3000-plat.log` : démarrage du fil à
`11:54:45.330941Z` (`:41`), première élection à `11:54:48.037742Z` (`:67`, soit
**+2,707 s**), élection de la porteuse à `11:54:48.336607Z` (`:95`, **+3,006 s**,
donc **6 ms après la fermeture de la fenêtre de 3000 ms**) — d'où `capture
arrêtée définitivement` = **0** et `réarmement programmé` = **0**. Sur le vert à
5000 ms : démarrage `11:44:53.667179Z`, première faute `11:44:56.257823Z`
(**+2,591 s**), mort `11:44:57.472982Z` (**+3,806 s**) — dans la fenêtre de 5 s,
hors de celle de 3 s. **Valeur retenue, dérivée de la mesure : 5000.**
⚠️ *Le message de commit situe la mort « à t+3,4 s » : ce chiffre est une
**interpolation** présentée comme un relevé. Les valeurs directement lisibles et
cohérentes avec lui sont +3,006 s et +3,806 s.*

**② `AUDIO_FAUTE_RECONSTRUCTION=5` ne tient pas la promotion**, et la fermeture
arithmétique « exactement 3 fautes » du plan ne vaut que pour le **premier**
cycle : le plan ignore le **réarmement de D9**, qui réapprovisionne le budget de
reconstruction après `REPIT_REARMEMENT_AUDIO`. Sur
`agent-critere-3-budget-5-plat.log` : promotion à `11:39:45.831348Z` (`:104`),
reprise par la porteuse à `11:39:50.851957Z` (`:110`) — **la promotion tient
5,020 s** —, puis `capture audio reconstruite restantes=0` à `11:39:54.856244Z`
(`:115`), à la sixième tentative.

**Le compte juste est `(REARMEMENTS_MAX + 1) × RECONSTRUCTIONS_MAX`**, et il est
vérifié **des deux côtés** : dans le code, `REARMEMENTS_MAX = 5`
(`agent/src/capteur/sommeil.rs:146`) et `RECONSTRUCTIONS_MAX = 3`
(`agent/src/audio.rs:83`), donc **6 × 3 = 18** ; et dans la mesure, **18
exactement** aux deux exécutions vertes. **L'injection est donc bien EN AVAL de
la garde de budget**, contrairement à ce que le plan redoutait.

### 5.2 `AUDIO_FAUTE_LECTURE_MS` fait exactement ce pour quoi elle a été écrite

La voisine avait déjà consommé 5 fautes pendant les ~80 ms où l'arbitrage l'a
effleurée à l'élection ; il lui en restait donc exactement
`LECTURES_ECHOUEES_MAX = 10`. **Sans la fenêtre de temps, elle serait morte à
l'instant même de sa promotion**, et le critère serait resté non démontrable.

### 5.3 🔴 Piège de relecture : `AudioMort` n'est journalisé NULLE PART

`grep -ac 'AudioMort' *-plat.log` rend **0 dans TOUS les journaux de D11**, y
compris les deux verts de ③. Le jeton du protocole n'est jamais écrit au journal.
Le compte réel se lit **côté capteur**, sur les lignes `agent::capteur::sommeil`
qui n'existent que sur réception du message : `capture audio morte` vaut **6** sur
chaque vert et **0** sur le rouge.

⚠️ **La séparation des issues que ③ revendique TIENT — 6 contre 0 — mais elle
n'est pas vérifiable par le `grep` que le message de commit suggère.** Quiconque
relira « `AudioMort` = 0 » et le vérifiera littéralement obtiendra 0 **sur le
vert aussi**, et conclura à tort que le rouge n'est pas discriminant.

### 5.4 ⚠️ Une réserve de méthode, la même qu'au §4.3

**`critere-3-2.json` porte `assignations = []` aux deux points** : la seconde des
deux exécutions vertes n'a **aucune preuve, issue du produit**, que `w-1` est la
fenêtre 660 Hz et `w-2` la 440 Hz. Seule la première (`critere-3-1.json`,
assignations pleines) est adossée au hameçon `fenetre-ouverte`. Le verdict ne
bascule pas — comptes agent et spectres concordent entre les deux — mais **la
garantie de méthode ne couvre qu'une exécution sur deux.**

---

## 6. Critère ④ — deux fenêtres montrent deux flux distincts (leg 8 de D10)

Recette de la tâche 12, commit `2b6452c`. D10 écrivait, de son propre contrôle
de séparation : *il ne peut pas échouer sur une page vivante* — l'échantillonnage
n'est pas simultané et la mire dérive à chaque trame, donc **deux pages décodant
le MÊME flux rendraient des empreintes différentes elles aussi**. D11 remplace
l'empreinte par un **marqueur d'identité invariant dans le temps**, porté par la
mire elle-même (tâche 7, commit `93052e7`).

### 6.1 Le ROUGE, joué EN PREMIER et sur la VM

`flux-rouge.json` : deux fenêtres portant la **même** mire (`"mires": ["3","3"]`)
rendent le **même** marqueur et le **même** RGB, **aux trois tours** —
`flux-analyse.log:7` (identique aux l. 11 et 15) :

```
COLLISION marqueur=0-15-5 sessions=9C6B9706,CEEF20AE
```

**Le prédicat PEUT donc échouer.** C'est exactement ce que le contrôle de D10 ne
pouvait pas.

🔵 **Et deux pièces d'instrument, jouées sur l'HÔTE avant la VM, réfutent le
contrôle de D10 directement** :
`instrument/controle-marqueur-rouge-sur-mire-d10.log:5,7,12` — sur la mire **de
D10** : « le marqueur DÉRIVE, il ne vaut rien », « distinction detectee : NON »,
`CONTROLE REFUSE : 3 echec(s)` ; contre
`instrument/controle-marqueur-d11.log:4-10` — sur la mire de D11 : invariance
OUI, collision OUI, distinction OUI, `CONTROLE RECU : le predicat discrimine.`

### 6.2 Le VERT, deux exécutions

| Journal | mires | `fenêtre attachée au capteur` | tours | marqueurs distincts / tour | collisions | `ERROR` |
| --- | --- | --- | --- | --- | --- | --- |
| `flux-1` | 1…10 | **10** | 3 | **10, 10, 10** | **0, 0, 0** | 0 |
| `flux-2` | 1…10 | **10** | 3 | **10, 10, 10** | **0, 0, 0** | 0 |

Et l'appariement par plus proche voisin RGB rend une **bijection** avec les dix
mires demandées, **aux six tours verts** : les `n=` valent `{1…10}` sans
répétition. Distances relevées sur les 60 relevés verts — **1ᵉʳ candidat 6 à 39,
2ᵉ candidat 41 à 182**. *Elles vivent dans `flux-analyse.log`, pas dans les
JSON, qui n'ont aucun champ d'appariement : c'est là qu'il faut chercher la
pièce.*

⚠️ **La chronologie du « rouge d'abord » est vérifiable** : `flux-rouge` court de
`12:05:11.801878Z` à `12:06:44.552303Z`, `flux-1` de `12:07:12.249516Z`, `flux-2`
de `12:10:48.152894Z`.

### 6.3 ⛔ Ce que ce critère établit, et ce qu'il n'établit PAS

Il établit que **deux pages ne décodent pas le même flux**. Il **n'établit pas**
que chaque page montre la fenêtre Windows qu'elle prétend montrer : la
correspondance page ↔ *fenêtre Windows* passe par l'appariement RGB au numéro de
mire, et non par un identifiant que la fenêtre porterait de bout en bout. **La
distinction est prouvée ; l'attribution ne l'est pas.**

*Observation annexe, sans effet sur ④ : dans les deux verts, neuf sessions
reçoivent `852x480` et une (`w-20`) `1024x576` — deux barreaux d'échelle
différents. Le marqueur est invariant à la taille.*

---

## 7. Critère ⑤ — le coût de la duplication surdimensionnée (leg 7 de D10)

Recette de la tâche 13, commit `7bcbb24`.

### 7.1 L'A/B à deux bras est NON PRIS, par sa propre règle d'admission

Aux quatre exécutions, les lignes `duplication de sortie établie` portent **deux**
valeurs de `desktop_width` — 1280 et 3840 — et jamais une seule :
`cout-analyse.log:6,23,40,57`, `ADMISSION du bras : NON OBTENU`. Il n'existe donc
ni bras « tout propre » ni bras « tout sale ».

**Et aucun bras n'est POSABLE**, ce qui est l'argument central : la combinaison
gagnante de `ChangeDisplaySettingsExW` sur cette VM est
`combinaison_gagnante=Some("aucun drapeau (dynamique, non persisté)")` — relevée
dans `agent-cout-sale-1-pose-plat.log` **et** `agent-cout-propre-1-pose-plat.log`
—, c'est-à-dire celle qui, **par construction, n'écrit pas le registre**.

### 7.2 🔵 LE FAIT NEUF : la pollution de registre est PAR GUID

C'est l'alternative que `CLAUDE.md` déclare **non tranchée depuis D8** — « ou le
mode registre est par GUID, ou une seule écriture empoisonne toutes les sorties
futures ». **Elle est tranchée : c'est par GUID.**

La taille **demandée** à la création est toujours `largeur=1280 hauteur=720`, et
c'est **toujours la sortie du 4ᵉ GUID du pilote** (`…677541430004`) qui naît en
3840×2160, les autres naissant en 1280×720. Chaîne complète sur
`cout-propre-1` :

```
agent-cout-propre-1-plat.log:200  sortie virtuelle créée id=261 … guid=…677541430004 largeur=1280 hauteur=720
agent-cout-propre-1-plat.log:218  enfant lancé session=w-8 …
agent-cout-propre-1-plat.log:259  fenetre{session=w-8}: duplication de sortie établie desktop_width=3840 desktop_height=2160
```

**Le fait se reproduit sur SEPT exécutions** — `flux-1`, `flux-2`,
`cout-sale-0-vivier-plein`, `cout-sale-1`, `cout-3`, `cout-4` **et
`cout-propre-1`** —, GUID énumérés à chaque fois de `…430001` à `…430008` (ou
`…43000A` à dix fenêtres), et **seul le quatrième** est touché.

⚠️ **Le message de commit `7bcbb24` en annonce SIX et omet `cout-propre-1`** —
qui est la plus probante des sept, sa sonde de pose ayant explicitement visé
`1280x720` et le 4ᵉ GUID étant **quand même** né en 3840×2160. **Le commit
sous-vend sa propre preuve** ; le compte juste est **sept**.

⚠️ **Ce que cela ne dit pas** : *pourquoi* le quatrième, ni pourquoi la taille
passée à la création est ignorée. Deux faits mesurés, **non expliqués** — comme
la non-persistance du changement de mode depuis D9.

### 7.3 Ce qui est mesuré à la place : un appariement plus serré que deux bras

Dans la **même** exécution, au **même** instant, sous la **même** charge : la
session dont la sortie est née 3840×2160 — **9 fois la surface** — contre les
sept nées 1280×720.

| Exécution | surdimensionnée (`w-8`) | 7 témoins (moy.) | étendue des témoins | écart |
| --- | --- | --- | --- | --- |
| `cout-sale-1` | 74,2 Hz | 72,1 | 69,7 – 73,5 | **+2,9 %** |
| `cout-propre-1` | 66,6 Hz | 64,8 | 63,1 – 66,4 | **+2,8 %** |
| `cout-3` | 67,2 Hz | 70,6 | 69,6 – 72,4 | **−4,8 %** |
| `cout-4` | 67,9 Hz | 67,2 | 64,6 – 71,5 | **+1,0 %** |

Pièces : `cout-analyse.log:16,33,50,67`. `packetsLost = 0` aux **32** pages
(8 pages × 4 exécutions, 64 échantillons aux deux bornes).

**Verdict : le coût n'est pas détectable à ce montage.**

### 7.4 🔴 MAIS L'ARGUMENT ÉCRIT DANS LE COMMIT EST FAUX — et ses propres chiffres le réfutent

`7bcbb24` conclut : « quatre executions, trois ecarts positifs et un negatif,
**TOUS a l'interieur de l'etendue des sept temoins** ». **Les écarts sont
exacts ; la phrase ne l'est pas.** Un seul des quatre est à l'intérieur —
`cout-4`. Les trois autres sont **hors bornes**, et les bornes sont imprimées
dans le message même : 74,2 > 73,5 ; 66,6 > 66,4 ; 67,2 < 69,6. **C'est 1 sur 4,
pas 4 sur 4.**

✅ **La conclusion survit, mais par une AUTRE raison, et il faut écrire la
bonne** : ce qui soutient « le coût n'est pas détectable » est l'**incohérence
de signe** — **trois exécutions sur quatre placent la session surdimensionnée
PLUS RAPIDE que ses témoins**, ce qu'un coût de duplication ne peut pas
produire. Un coût réel donnerait un signe **constamment négatif**. *L'argument
juste était là ; l'argument écrit ne l'était pas.*

### 7.5 ⚠️ Deux réserves, dont une portée par le commit et une qui ne l'était pas

- 🔴 **Confondeur, correctement porté par le commit** : le groupe surdimensionné
  ne compte **qu'une** session par exécution, et c'est **toujours `w-8`, la
  quatrième ouverte**. « Sortie surdimensionnée » et « quatrième session » sont
  **confondus**, et rien dans ces journaux ne les départage.
- ⚠️ **Énoncé plus large que le relevé** : « au MÊME barreau » est vrai de `w-8`
  contre six des sept témoins, **pas du septième** — `w-16` est à `1024x576`
  quand les autres sont à `852x480`. Effet mineur (`w-16` n'est pas la session
  sous test), mais l'énoncé dépasse la mesure.

### 7.6 Le défaut de protocole trouvé par l'exécution, et versé

`cout-sale-0-vivier-plein` : la première tentative a lancé la sonde de pose sur
un vivier **déjà plein** — dix sorties orphelines survivent à un
`Stop-Process` —, n'a pas pu créer sa sortie et **s'est arrêtée en 19 lignes sans
verdict**. `agent-cout-sale-0-vivier-plein-pose-plat.log:4` relève
`nombre=11 attachees=11`, et la l. 19 rend
`0x80070044` (`ERROR_TOO_MANY_NAMES`) — le plafond de dix du pilote, connu depuis
les mesures préalables du 31 juillet 2026. **La purge passe désormais AVANT la
pose** (`instrument/pilote-cout-d11.mjs:140-148`). *L'exécution **produit**
correspondante, elle, a bien tourné : 8 attachées, 0 `ERROR`.*

---

## 8. Critère ⑥ — le maillon fautif du `Resize` (leg 2 de D10)

Recette de la tâche 14, commit `2cfca1b`. L'instrumentation était **déjà en
place** depuis D10 ; il ne manquait qu'une lecture — et la lecture exigeait de
**collecter la console**, ce qu'aucun des trois pilotes de D10 ne faisait.

### 8.1 Le contrôle d'atteignabilité, joué EN PREMIER et VU ROUGE

| Bras | entrées de console | verdict |
| --- | --- | --- |
| `SANS_ABONNEMENT=1` (le montage de D10 : `Runtime.enable` **sans** abonnement) | **0** — `console-controle-rouge.jsonl` est **vide** | `controle-collecte-d11t14-rouge.log:6` — `CONTROLE DE COLLECTE REFUSE => RECETTE 6 ANNULEE` |
| avec `Runtime.consoleAPICalled` | **2**, chacune avec son `sessionId` | `controle-collecte-d11t14-vert.log:5` — `CONTROLE DE COLLECTE RECU` |

Puis la balise est reposée **dans chaque page d'application du run réel** :
**5/5 aux trois exécutions**. ✅ **Une absence de log est donc devenue une
information** — ce qu'elle n'était pas sous D10.

### 8.2 La grille, lue

La grille à trois issues vit dans `client/src/main.ts` (bloc de commentaire
précédant `console.debug('[instrumentation resize] declenchement
ResizeObserver', …)`) : ① aucun log ⟹ l'observateur n'est jamais armé ; ② log
présent et `clientWidth/Height` **suit** `innerWidth/Height` ⟹ ni l'observateur
ni la mise en page ; ③ log présent et ne suit pas ⟹ le CSS du `<video>`.

| Exécution | pages balisées | déclenchements | émissions | `Resize différé` | client vs inner | issue |
| --- | --- | --- | --- | --- | --- | --- |
| `resize-1` | 5/5 | 5 (1/session) | 5 | **0** | 1280×720 = 1280×720 | **②** ×5 |
| `resize-2` | 5/5 | 5 | 5 | **0** | 1280×720 = 1280×720 | **②** ×5 |
| `resize-sans-viewport` | 5/5 | 5 | 5 | **0** | **1280×633 = 1280×633** | **②** ×5 |
| `resize-0-args-opaques` | 5/5 | 5 | 5 | **0** | *illisible* | écartée |

**15 sessions, toutes en issue ②.** Côté agent, exactement **un**
`contrôle reçu … Resize` par session, portant la bonne session — par exemple
`agent-resize-1-plat.log:51` :
`contrôle reçu session=w-2 Resize { version: 3, width: 1280, height: 720 }`.
Sur les 18 `contrôle reçu` par exécution : 5 `Resize` et 13 `Visibility`.

### 8.3 ⛔ Le silence de D9 n'est PAS reproduit — le maillon reste NON IDENTIFIÉ

C'est une issue que la conception déclarait **possible d'avance**. Ce que la
recette ajoute, et c'est tout ce qu'elle ajoute : **les trois maillons suspects
— armement du `ResizeObserver`, mise en page CSS du `<video>`, canal de
contrôle — sont montrés FONCTIONNELS dans CETTE configuration**, chacun sur
pièce. ⚠️ **Cela ne les disculpe pas en général.**

### 8.4 Un bras de plus, non prévu par le plan

`SANS_VIEWPORT=1` retire l'imposition de viewport des pilotes de D10 — laquelle
est **elle-même un changement de taille**, et pouvait donc **provoquer** ce qu'on
observe. Même résultat : 5/5, issue ②, zéro différé, et un viewport **1280×633,
de hauteur IMPAIRE, toléré de bout en bout** (le client l'émet tel quel, l'agent
le reçoit tel quel). **Le montage n'est pas la cause.**

### 8.5 ⚠️ Une exclusion NON DÉCLARÉE, trouvée à la vérification des journaux

Les journaux de console portent, **en plus** des cinq pages d'application, des
pages `?session=w-impair` que le commit ne mentionne nulle part : **`w-7`** sur
`resize-1` ; **`w-1`, `w-5`, `w-7`** sur `resize-2` ; **`w-3`, `w-5`** sur
`resize-sans-viewport` et sur `resize-0-args-opaques`. Elles ont chargé
l'application (`[vite] connecting…` **et** `[vite] connected.`) et n'ont produit
**aucun** `declenchement ResizeObserver`, **aucune** émission, **aucune** balise.
**À la lettre de la grille, c'est la FORME de l'issue ①** — celle-là même que la
recette cherchait.

**L'exclusion est justifiée, et deux vérifications la fondent** :

1. **Côté agent, ces sessions n'existent pas du tout** — `grep -ac 'session=w-1\b'`,
   `w-3`, `w-5`, `w-7`, `w-9` sur `agent-resize-2-plat.log` rend **0** pour
   chacune ; seules les cinq paires (`w-2`…`w-10`) sont lancées et attachées
   (`enfant lancé` = 5, `fenêtre attachée au capteur` = 5, `ERROR` = 0 aux quatre
   journaux).
2. **Ces pages sont fermées avant l'étape de balisage** : le champ `.pages` des
   `resize-*.json` n'en liste que sept — `about:blank`, `shell.html` et les cinq
   paires —, et le pilote balise toute page non-shell et non-`about:blank`.

Sans session agent derrière elles, elles n'ont rien à observer ni à émettre :
**ce n'est pas le silence de D9.** Mais **le dénominateur « 5/5 » exclut
silencieusement une à trois pages d'application par exécution, et un lecteur du
seul commit ne peut pas le savoir.**

🔵 **Et c'est très vraisemblablement la trace du constat D9 survivant** — « le
shell réémet `fenetre-ouverte` pour une fenêtre déjà ouverte », requalifié par
D10 en **comportement du PRODUIT, mécanisme non élucidé**. Il reste non élucidé.

### 8.6 La piste, déclarée comme piste

Dans les **deux** exécutions de **D9**, la seule session muette à coup sûr est
`w-5`, la seule qui n'ait **jamais** annoncé `visible=true` ; `w-2` est muette
dans l'une et émet dans l'autre, et son basculement à `visible=false` tombe
140 ms après son chargement, **sous les 200 ms de lissage du `ResizeObserver`**.
⚠️ **Corrélation relevée sur les journaux de D9, mécanisme NON éprouvé** — et
ces journaux sont hors du répertoire D11, donc **hors de la vérification de ce
document**.

### 8.7 Le défaut d'instrument trouvé par l'exécution, et versé

`resize-0-args-opaques` : `Runtime.consoleAPICalled` rend la chaîne `"Object"`
pour **tout** argument objet — donc pour les **quatre nombres** dont la grille a
besoin. Le premier run ne tranchait que l'issue ①. Les champs se relèvent dans
`preview.properties`, et les trois runs retenus les portent bien résolus.

---

## 9. Leg 6 de D10 — la cause du refus de reconstruction, RÉPONDUE PAR UNE PIÈCE

D10 léguait : *« la cause du refus de reconstruction n'est pas identifiée »*. La
tâche 2 (commit `b087e0c`) a remplacé `%erreur` par `{erreur:#}` — **une ligne**,
qui rend la chaîne de causes **entière** au lieu de sa seule tête. C'est **la
première fois que `{erreur:#}` sert**, et il a servi :

`agent-critere-2-2-plat.log:99`

```
WARN agent::transport::piste_audio: reconstruction de la capture audio refusée
erreur="ouverture du process loopback du PID 5872: Initialize du client de
process loopback (format impose : 48 kHz, 2 canaux, 16 bits): Défaillance
irrémédiable (0x8000FFFF)" restantes=2
```

**Le répit se lit à la milliseconde** : refus à `11:28:50.298256Z`, `capture
audio reconstruite restantes=1` à `11:28:52.299329Z` (`:101`), soit **2,001 s** =
`REPIT_RECONSTRUCTION`. La tentative suivante **réussit**.

⚠️ **Le leg devient OBSERVABLE, pas EXPLIQUÉ** : `0x8000FFFF`
(`E_UNEXPECTED`) est le code le moins informatif de la famille, et rien ne dit
*pourquoi* `Initialize` le rend là. **Ce qui est acquis, c'est que la cause a
cessé d'être jetée à l'écriture.**

**Occurrences sur tout D11** (`grep -ac` sur les `-plat.log`) : 18 + 18 sur les
verts de ③ (la faute injectée emprunte le même `warn!`), 5 sur `budget-5`, et
**1 + 1 + 1** de cause réelle sur `agent-critere-2-2`, `agent-rouge-recette-2` et
`agent-rouge-3` — **44 au total**.

---

## 10. Les legs froids de D9 — n°11 et n°12, tombés du registre

Repris par la tâche 6 (commit `a1485ca`). Ils avaient disparu de **deux**
registres à la fois : ni le §11 du document de résultats de D10 ni la liste de
`CLAUDE.md` ne les portaient. Ils étaient **récupérables parce que leur preuve
est dans le CODE, pas dans un rapport gitignoré** — à la différence des six
constats du leg 3, définitivement perdus.

- **n°11, deux tests incapables de rendre l'autre valeur.**
  `windows_source/telemetrie.rs::une_telemetrie_neuve_est_a_zero` n'éprouvait que
  `#[derive(Default)]` ; `client/src/resize.test.ts` annonçait « quand le canal
  était fermé » un état que `RejeuResize` **ne peut pas atteindre**, n'ayant
  aucune notion de canal ni de `readyState`. ⚠️ **Et la précondition que le PLAN
  prescrivait pour le premier restait VERTE sous le sabotage que le plan
  prescrivait lui-même** — `capturee()` et `produite()` suffisaient à la
  satisfaire. **Resserrée au triplet exact, elle vire au rouge.** *Un plan
  n'immunise pas contre le contrôle vacueux : il en est une source, et c'est la
  cinquième fois que ce dépôt le paie.*
- **n°12, un invariant porteur ni écrit ni testé** : le rejeu du `Resize` ne
  tient que parce que le `.then()` qui pose
  `controlChannel.addEventListener('open', emettreSiPossible)` s'exécute
  **intégralement de façon synchrone**, sans `await` intercalé. Un `await` glissé
  là romprait le rejeu **en silence**. ⚠️ **Nommer le symbole, jamais la ligne** :
  ce site est passé de `:345` (D9) à `:386` sans qu'aucune de ses mentions ne
  bouge.

---

## 11. La revue transverse de fin de branche — SEPT affirmations, plus un leg de P1

Cinq défauts en D7, trois Critiques en D8, six en D9, **douze en D10** — et
**tous franchissent une frontière de tâche** : chacun est correct des deux côtés
pris séparément, et **une revue par tâche ne peut structurellement pas les
voir**. La cible propre de D11 était nommée d'avance par le plan : **les
commentaires qui décrivent le MONO-FENÊTRE**, que le correctif du leg 4 rend
faux.

**Six des sept portent la MÊME affirmation** — « en mono-fenêtre le remède de
reconstruction est INERTE » — devenue fausse au commit `5c0ce43` de cette branche
même. Commit correcteur : `0eadc02`.

| # | Fichier:ligne | Affirmation devenue fausse | Ce qui la rend fausse |
| --- | --- | --- | --- |
| 1 | `agent/src/windows_audio.rs:87` | « Un ordre externe atteint bien une source en mode session, **et il la fait taire** » | `audio_porteuse` vaut `true` en mono-fenêtre : le réarmement RÉÉMET. **L'atteignabilité reste vraie, sa conséquence ne l'est plus** |
| 2 | `agent/src/windows_audio.rs:136` | « la conséquence … est documentée … **et léguée** » | le leg est fermé, et mesuré |
| 3 | `agent/src/transport.rs:238` | « son défaut propre **est** que la source reconstruite **est réarmée à `false`** » | au présent, **dans le fichier même** dont la tâche 3 a corrigé les lignes 276-293. Le patron de D10 à l'identique |
| 4 | `agent/src/transport/tick.rs:293` | « puis le réarmement **la rend MUETTE** » | et **ce fichier est celui qui APPELLE `reconstruire_ou_signaler`**, donc celui qui donne le modèle mental à qui reprendra le sujet |
| 5 | `agent/src/capteur/sommeil.rs:106` | « le remède y est INERTE …, **léguée et non corrigée** » | idem |
| 6 | `agent/src/transport/tick/tests/audio.rs:223` | « son défaut propre … **n'est couvert par aucun test** » | **rendu faux par un test ajouté DEUX CENTS LIGNES PLUS BAS dans le même fichier, par la même tâche** |
| 7 | `agent/src/transport/tick/tests/audio.rs:421` | « `appliquer_audio`, **son unique écrivain** » | **faux dans le commit même qui ajoute le second** (`set_audio_porteuse`) |

**Le n°6 est le plus pur de la série**, et il bat le « septième commentaire
orphelin » de D10 : la distance entre l'affirmation et sa réfutation n'est pas
une tâche, ni un fichier — c'est **deux cents lignes du même fichier, dans le
même commit**.

**Plus un leg du sous-projet ⑤ soldé au passage** :
`agent/src/superviseur/protocole.rs:5` nommait `signaling/src/server.ts`,
**disparu** au sous-bloc P1 (le paquet vit désormais dans `plateforme/src/`).
`CLAUDE.md` l'enregistrait comme dette délibérée, `agent/` étant alors le
périmètre d'un travail concurrent. **La propriété énoncée reste VRAIE** —
`TYPES_RELAYES` porte toujours les mêmes six types (`offer`, `answer`,
`fenetre-ouverte`, `fenetre-fermee`, `refus`, `viewport`), **relus le 19 août
2026 dans `plateforme/src/signaling/relais.ts:37-44`** ; seul le chemin était
périmé.

⚠️ **Toutes les substitutions ont été faites en vérifiant leur COMPTE** — une
attendue, une obtenue, sept fois. *Une substitution qui ne dit pas combien
d'occurrences elle a touchées est une affirmation de complétude non vérifiée.*

---

## 12. 🔴 Ce que la vérification des journaux a trouvé DANS LES MESSAGES DE COMMIT

Les cinq messages de commit de recette sont la mémoire longue d'un sous-bloc au
même titre que ce document. Une relecture des journaux **indépendante des
rapports de tâche** y a trouvé **neuf** écarts. Aucun ne renverse un verdict ;
**deux sont des affirmations fausses**, et le premier est réfuté par les chiffres
que le message imprime lui-même.

| # | Recette | Écart | Gravité |
| --- | --- | --- | --- |
| 1 | ⑤ | « **TOUS** à l'intérieur de l'étendue des sept témoins » → **1 sur 4**. Corrigé au §7.4, où le bon argument est écrit | 🔴 **affirmation fausse** |
| 2 | ⑤ | Le fait « GUID …0004 » se reproduit sur **7** exécutions, pas 6 : `cout-propre-1` — la plus probante — est omise | ⚠️ **sous-vend sa propre preuve** |
| 3 | ① | La preuve **spectrale** du bras rouge repose sur **une** exécution : `rouge-1.json` est la mesure invalidée par l'instrument cassé. Les comptes agent, eux, sont bien deux | 🔴 réserve de portée |
| 4 | ① | « trois `window is not defined` sur huit relevés » → **2 sur 16**. Le fait est vrai, **ni son numérateur ni son dénominateur ne tombent** | 🟡 chiffres faux, fait vrai |
| 5 | ② | `rouge-2-recette2.json` porte `assignations = []` : identité **par rang de nom**, la pratique que le commit déclare bannir | 🔴 méthode annoncée non tenue |
| 6 | ③ | `critere-3-2.json` porte `assignations = []` : **un vert sur deux** sans provenance d'identité issue du produit | 🔴 idem |
| 7 | ③ | `AudioMort` **n'est journalisé nulle part** : un `grep 'AudioMort'` rend 0 **sur le vert aussi**. Le compte réel passe par `capture audio morte` côté capteur — 6 contre 0 | 🔴 piège de relecture |
| 8 | ⑥ | « 5/5 pages » exclut sans le dire 1 à 3 pages `?session=w-impair` par exécution. **Exclusion justifiée** (§8.5), **non déclarée** | ⚠️ énoncé incomplet |
| 9 | ② | Les tailles de binaire du bras rouge (9 357 312 / 9 356 288) ne sont étayées par **aucune pièce** du répertoire | ⚠️ affirmation sans pièce |

**Tout le reste concorde, y compris au chiffre près** : les huit chiffres du
tableau de ①, les quatre de ②, les six de ③, les distances RGB et la bijection de
④, les quatre cadences et écarts de ⑤ (un recalcul indépendant depuis les
journaux bruts reproduit `cout-analyse.log` à l'identique), les 15 sessions en
issue ② de ⑥, `packetsLost = 0` aux 32 pages, les 2,001 s de
`REPIT_RECONSTRUCTION`, les 5,020 s de promotion, et **18 = (5 + 1) × 3 vérifié à
la fois dans le code et dans la mesure**.

⚠️ **La leçon de méthode, et elle est neuve** : *un message de commit est une
pièce du dépôt, et il n'est relu par personne.* Sept des neuf écarts ci-dessus
n'existent que là — pas dans les journaux, qui sont justes. **La seule défense
est de relire les journaux contre le message, et non l'inverse.**

---

## 13. Ce que D11 n'établit PAS

- **Aucun taux, nulle part.** Deux exécutions par critère au mieux, une seule
  pour plusieurs bras.
- 🔴 **L'existence d'une cause NATURELLE de mort de capture audio reste
  inconnue.** Les quatre déclencheurs de D9 n'en produisent aucune, le cinquième
  (tuer le `chrome.exe` cible) tue la fenêtre avant l'audio, et **tout ce que
  D11 mesure l'est SOUS INJECTION DE FAUTE**. *L'injection établit que le remède
  fonctionne, jamais qu'une cause existe.*
- 🔴 **La discrimination entre `set_actif(self.audio_porteuse)` et
  `set_actif(true)` n'est portée QUE par le test d'hôte
  `une_session_non_porteuse_reconstruite_reste_muette`** — et §4.1 établit
  **pourquoi** aucun montage VM de cette forme ne peut la porter. **La lacune
  que D10 se reprochait n'est pas comblée** ; elle est expliquée.
- **Le leg 6 devient observable, pas expliqué** : `0x8000FFFF` est le code le
  moins informatif de sa famille, et rien ne dit pourquoi `Initialize` le rend.
- 🔴 **Le maillon fautif du `Resize` reste NON IDENTIFIÉ.** Les trois suspects
  sont montrés fonctionnels **dans cette configuration**, ce qui ne les disculpe
  pas en général — c'est exactement la nuance que la correction C3 de D8 avait
  payée.
- **Le coût de la duplication surdimensionnée n'est pas détectable à ce
  montage**, ce qui n'est pas « il n'y en a pas ». Et **le confondeur n'est pas
  levé** : la sortie surdimensionnée est toujours la quatrième session ouverte.
- **La séparation des flux est prouvée, l'ATTRIBUTION ne l'est pas** (§6.3) :
  rien n'établit que chaque page montre la fenêtre Windows qu'elle prétend
  montrer.
- **Pourquoi le QUATRIÈME GUID** — et pourquoi la taille passée à la création
  est ignorée — n'est pas expliqué. Deux faits mesurés, non expliqués.
- **Rien ne nettoie le registre**, et D11 n'y touche pas : la voie de D10 rend le
  produit *indifférent* à son état, elle ne le nettoie pas.
- **L'A/B sur `set_desired_bitrate` reste dû**, écarté par décision (§2.2 de la
  conception) avec sa **condition de réouverture**.
- **Les six constats parqués de D9 restent perdus.**
- **Le couplage réélection / répit est EXERCÉ, pas BORNÉ** : ③ le fait tourner
  cinq fois par exécution, rien ne mesure le retard qu'il inflige au fil de
  drainage.
- **Le plafond de 8 encodeurs au-delà de 720p** reste inconnu, et **les trois
  couches inconnues du chantier D** le restent : le plafond de 8 encodeurs, celui
  de 4 processus, et le mécanisme de l'abandon du mutex DXGI.
- **La latence de bout en bout**, qu'aucun sous-bloc du chantier D n'a jamais
  mesurée — D11 ne la mesure pas davantage.
- **Aucune constante n'est calibrée** par un jugement visuel ou d'écoute :
  `BPP_MIN`, `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`, `HYSTERESIS`,
  `REPIT_APRES_ECHEC`, `TAILLE_MAX_SORTIE`, `REPIT_REARMEMENT_AUDIO`,
  `REARMEMENTS_MAX`, `RECONSTRUCTIONS_MAX`, `REPIT_RECONSTRUCTION`. ⚠️ **Et
  `AUDIO_FAUTE_LECTURE_MS = 5000` n'est pas davantage calibrée** : c'est une
  valeur **dérivée d'une mesure de ce montage-ci**, pas une constante du produit
  — c'est une variable de banc.
- **La visibilité et le focus restent imposés par le pilote de recette**, page
  par page — limite héritée de D5, qu'aucun sous-bloc n'a levée.
- **Aucun client réel, aucun HiDPI réel** : `deviceScaleFactor = 1` partout, donc
  le legs HiDPI reste **inexercé**.
- **Le chemin d'extinction propre du superviseur** n'a toujours jamais été
  exercé, depuis D1.

---

## 14. Pièges neufs — à connaître avant de toucher à ce terrain

- ⚠️ **Un budget d'injection dont l'origine est prise au DÉMARRAGE DU FIL ne
  mesure pas ce qu'on croit.** `AUDIO_FAUTE_LECTURE_MS=3000` rendait le critère ③
  **inatteignable**, parce que le premier arbitrage du capteur arrive ~2,7 s après
  le démarrage du fil et que la fenêtre se refermait 6 ms avant l'élection de la
  porteuse. **Lire l'origine d'un compteur de banc avant de dimensionner sa
  fenêtre** — c'est la variante « injection » du piège de D6 sur les paliers de
  mesure.
- ⚠️ **Une fermeture arithmétique peut ne valoir que pour le PREMIER cycle.** Le
  plan calculait « exactement 3 fautes » en ignorant le **réarmement de D9**, qui
  réapprovisionne le budget de reconstruction. Le compte juste est
  `(REARMEMENTS_MAX + 1) × RECONSTRUCTIONS_MAX` = **18**. **Vérifier qu'un
  mécanisme voisin ne recharge pas le compteur qu'on croit épuiser.**
- 🔴 **Un ROUGE peut être VACUEUX parce que le chemin qu'il modifie ne court
  jamais dans ce montage.** Le rouge de ② rend **exactement** le même relevé que
  le vert : le garde `if !emettait` précède l'injection, donc une voisine muette
  n'atteint jamais la ligne modifiée. **Avant de bâtir un binaire défectueux,
  vérifier que la ligne qu'il change est ATTEINTE par le montage.**
- ⚠️ **Un jeton de protocole peut n'être journalisé nulle part.** `grep 'AudioMort'`
  rend **0 sur le vert comme sur le rouge** : le compte réel se lit sur l'effet
  côté capteur (`capture audio morte`), pas sur le nom du message. **Un `grep` de
  recette doit viser une trace qui EXISTE**, ce qui se vérifie sur le vert avant
  de conclure du rouge.
- 🔴 **Un message de commit est une pièce du dépôt, et personne ne le relit.**
  Sept des neuf écarts du §12 n'existent que là ; les journaux, eux, sont justes.
  **Relire les journaux contre le message, jamais l'inverse** — et se méfier des
  phrases d'interprétation, qui sont là où les deux affirmations fausses se
  trouvent.
- ⚠️ **Un dénominateur « 5/5 » peut exclure silencieusement ce que la recette
  cherchait.** Les pages `?session=w-impair` de ⑥ ont la **forme** de l'issue ① ;
  leur exclusion est justifiée, mais elle n'était écrite nulle part. **Énoncer la
  règle de sélection avant de compter** — le piège de D6, rejoué sur des pages.
- ⚠️ **Une provenance d'identité peut manquer sur une exécution et pas sur
  l'autre.** `assignations = []` sur `rouge-2-recette2.json` et `critere-3-2.json`
  : le hameçon n'a rien capturé, et l'identité y retombe **sur un rang de nom** —
  la pratique que le protocole déclarait bannie. **Contrôler que la garantie de
  méthode a effectivement produit sa pièce, exécution par exécution.**
- ⚠️ **Un contexte audio suspendu rend `-Infinity` exactement comme un silence
  réel.** Le contrôle spectral ne pouvait pas distinguer les deux tant que
  `ctx.resume()` n'était pas appelé ni `ctx.state` vérifié. Et **`-Infinity` se
  sérialise en `null`** : la sentinelle disparaît du journal versé.
- ⚠️ **Une sonde de pose lancée sur un vivier déjà plein s'arrête sans verdict**,
  en dix-neuf lignes (`0x80070044`). **La purge passe avant la pose.** Dix
  sorties orphelines survivent à un `Stop-Process`.
- ⚠️ **Un découpage sur `:` casse sur un chemin Windows.** `--fenetres=hz:profil`
  donnait `profil = 'C'` sur `C:\dev\…`, et **le seul symptôme était l'absence
  d'annonce**.
- ⚠️ **L'ordre shell → superviseur → fenêtres fait capturer la console PowerShell
  de la tâche planifiée.** Ouvrir les fenêtres **avant** le superviseur, qui les
  trouve par `enumerer_existantes`.
- ⚠️ **Une précondition prescrite par un plan peut rester VERTE sous le sabotage
  que le plan prescrit lui-même** (tâche 6, `a1485ca`). C'est le quatrième
  contrôle vacueux écrit par un plan dans ce dépôt en deux sous-blocs. **Un plan
  n'immunise pas contre ce patron : il en est une source.**
- ⚠️ **`Runtime.consoleAPICalled` rend la chaîne `"Object"` pour tout argument
  objet.** Les champs se relèvent dans `preview.properties` — sans quoi une
  grille qui a besoin de nombres ne tranche que son issue dégénérée.
- 🔴 **Un compte de tests n'est attribuable qu'assorti de son HEURE quand deux
  chantiers partagent l'arbre.** Le compte client est passé de **107 (12
  fichiers) à 15:16** à **120 (13 fichiers) à 15:38** sans qu'aucune tâche de
  D11 n'y touche — le sous-projet ⑤ committait sur `client/` en parallèle. Et
  le même partage a failli fausser une **marge de fichier** :
  `client/verify-webrtc.mjs` vaut **497 au dépôt commité** et **488 dans
  l'arbre de travail**. **Mesurer avec `git show HEAD:` quand l'arbre est
  partagé**, et dater tout compte.
- ⚠️ **Une commande `git commit -m` dont le message porte des accents graves
  perd des morceaux de phrase** : le shell les interprète comme des
  substitutions de commande. Trois phrases ont été mutilées ainsi dans ce
  sous-bloc, rattrapées par un `--amend -F fichier`. **Passer les messages
  longs par un fichier.**

---

## 15. Ce que D11 lègue

### Legs de D10 réglés

| Leg de D10 | Sort sous D11 |
| --- | --- |
| **4** 🔴 — le remède de reconstruction inerte en mono-fenêtre | ✅ **CORRIGÉ ET MESURÉ** — 2 vertes, 2 rouges, avec un témoin d'armement indépendant du spectre |
| **5** — construire `AUDIO_FAUTE_RECONSTRUCTION` | ✅ **FAIT**, budget **global au processus**, et il a servi : critère ③ tenu |
| **6** — la cause du refus de reconstruction | ✅ **RÉPONDUE PAR UNE PIÈCE** (`0x8000FFFF` sur `Initialize`) — **observable, pas expliquée** |
| **7** — le coût de la duplication surdimensionnée | ✅ **MESURÉ** en apparié (l'A/B à deux bras est **non posable**) — **coût non détectable à ce montage**, confondeur non levé |
| **8** — la séparation des flux | ✅ **PROUVÉE** au sens « deux pages ne décodent pas le même flux », rouge joué sur la VM. **L'attribution page ↔ fenêtre Windows ne l'est pas** |
| **D9 n°11** — deux tests faibles | ✅ **CORRIGÉS**, et la précondition du plan a dû être resserrée pour virer au rouge |
| **D9 n°12** — l'invariant non écrit du rejeu | ✅ **ÉCRIT**, avec sa grille de lecture |

### Ce qui reste dû

1. ⛔ **L'A/B sur `set_desired_bitrate`** (leg 1 de D10, n°3 de D9, n°4 de D6) —
   **écarté par décision**, joué deux fois sans rien trancher (+23,2 % contre
   +83,1 % de variance intra-bras en D9 ; 2 paires positives contre 2 négatives
   en D10). **Condition de réouverture, écrite pour ne pas se rejouer à
   l'aveugle** : un montage dont la charge d'hôte est **CONTRÔLÉE** — et non
   seulement appariée —, sur **au moins huit paires**. C'est un chantier de banc
   à part entière, pas une tâche de solde. ⚠️ *Cela n'affirme pas que l'appel est
   sans effet.*
2. ⛔ **Le maillon fautif du `Resize`** (leg 2 de D10) — **lu, et non identifié**.
   Le silence de D9 n'est pas reproduit ; les trois suspects sont fonctionnels
   **dans cette configuration**. La piste du §8.6 (`visible=true` jamais annoncé,
   et le lissage de 200 ms) est **une corrélation sur les journaux de D9, jamais
   éprouvée**.
3. ⛔ **Les SIX constats parqués de D9 restent PERDUS.** Inventer une liste serait
   pire que de l'admettre.

### Legs neufs de D11

4. ⛔ **Aucune cause NATURELLE de mort de capture audio n'est connue**, et le
   cinquième déclencheur reste le seul jamais essayé qui ne tue pas la fenêtre
   avec l'audio — il n'existe pas. **Tout le remède est éprouvé sous injection.**
5. ⛔ **La discrimination `audio_porteuse` contre `true` ne vit que dans un test
   d'hôte**, et §4.1 établit qu'aucun montage VM de cette forme ne peut la
   porter. **Il en faudrait un autre**, produisant une session **non porteuse au
   moment de sa reconstruction**.
6. ⛔ **Pourquoi le QUATRIÈME GUID** naît à la taille du registre quand les sept
   autres naissent à la taille demandée. Le fait est reproduit **sept fois** ; le
   mécanisme est inconnu.
7. ⛔ **Le confondeur de ⑤ n'est pas levé** : « sortie surdimensionnée » et
   « quatrième session ouverte » sont confondus. Les départager demande un
   montage où le rang d'ouverture et la taille de naissance varient
   indépendamment — ce que le §7.1 montre **non posable** par le levier de mode
   de sortie sur cette VM.
8. ⛔ **L'ATTRIBUTION page ↔ fenêtre Windows n'est pas prouvée** (§6.3) : ④ prouve
   la distinction, pas la correspondance. Il y faudrait un identifiant porté de
   bout en bout par la fenêtre, pas un appariement RGB.
9. ⛔ **Le shell réémet `fenetre-ouverte` pour une fenêtre déjà ouverte**
   (§8.5) — constat survivant de D9, **requalifié par D10 en comportement du
   PRODUIT**, et dont la trace se relit ici : une à trois pages d'application par
   exécution sans session agent derrière. **Mécanisme toujours non élucidé.**
10. ⛔ **Le couplage réélection / répit est exercé, pas borné** — rien ne mesure
    le retard qu'il inflige au fil de drainage.
