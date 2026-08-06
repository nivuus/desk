# Sous-bloc D9 — solder la dette du chantier D

**Date** : 6 août 2026
**Statut** : Spécification validée, prête à planifier
**Portée** : Fermer les douze legs ouverts au sortir de D8 — les six propres à
D8, les deux de D7 et les trois de D6 qu'aucun sous-bloc n'a traités, plus les
trois défauts d'instrument.

---

## 1. Objet, et pourquoi un seul bloc

D8 s'est terminé le 5 août 2026 en léguant **treize** points. Le n°11
(l'annotation du §4.1 du cadrage jeux) est fait au commit `2856f2a` ; **douze
restent**, et ils dorment depuis un à trois sous-blocs :

| Leg | Origine | Nature | Morde-t-il le produit **livré** ? |
| --- | --- | --- | --- |
| 1 — signal enfant→capteur quand une capture audio meurt | D7 (F3) | code | **oui** |
| 2 — identité de session par génération monotone | D7 (F5), préexistant | code | **oui** |
| 3 — `TICKS`/`CAPTURED`/`PRODUCED` par session | D6 n°1 | code | non |
| 4 — critère ④ de D6 à palier 45–60 s | D6 n°3 | mesure | non |
| 5 — A/B différentiel sur `set_desired_bitrate` | D6 n°4 | mesure | non |
| 6 — les trois inconnues du changement de mode | D8 | mesure | non (chemin désarmé) |
| 7 — défaut HiDPI côté client | D8 | code | non (chemin désarmé) |
| 8 — instrumenter la chaîne viewport → `Resize` | D8 | code + mesure | non |
| 9 — trois défauts de l'instrument de recette | D8 | outillage | non |
| 10 — pourquoi si peu de `Resize` | D8 | code + mesure | non |
| 12 — C1, la pollution du registre par le produit | D8 | code | non (chemin désarmé) |
| 13 — C2, la reprise D2 court-circuitée | D8 | code | non (chemin désarmé) |

**Décision, prise le 6 août 2026** : les douze en **un seul bloc**. Le coût est
écrit — trois familles sans lien technique entre elles, une sonde de banc et
trois recettes produit distinctes, sur une VM qui s'hiberne toute seule.

**Deux legs mordent sur le code livré aujourd'hui**, et c'est ce qui interdit
de reporter encore : une erreur de lecture WASAPI silence **définitivement** un
groupe de PID entier en se déclarant en bonne santé (leg 1), et l'identité
d'une session par son seul nom porte une course au `retirer` (leg 2).

## 2. Forme du bloc — l'éliminatoire en tête

Un seul des douze legs en conditionne cinq autres. La sonde qui le tranche
coûte un banc ; se tromper à sa place coûte une recette produit entière. Le
bloc s'ouvre donc sur une **phase P** — une mesure, aucun code de production —
dont le verdict tombe avant qu'une ligne de la famille ① ne s'écrive.

Les legs qui ne croisent pas le plein écran (1, 2, 3, 5, 9) **n'attendent pas
ce verdict** et peuvent s'écrire en parallèle.

```
        ┌─────────────┐
        │  Phase P    │  sonde de banc : l'éliminatoire + C1
        └──────┬──────┘
               │ verdict
     ┌─────────┴─────────┐
     ▼                   ▼
┌─────────────┐   ┌──────────────────────────────┐
│ Famille ①   │   │ Familles ② et ③              │
│ 6 12 13 7   │   │ 1 2  |  3 4 5 9              │
│ 8 10        │   │ (indépendantes de P)         │
└──────┬──────┘   └───────┬──────────────┬───────┘
       ▼                  ▼              ▼
   Recette ①          Recette ②      Recette ③
       └──────────────────┴──────────────┘
                          ▼
             Revue transverse de fin de branche
```

## 3. Phase P — la sonde éliminatoire

### 3.1 La question

**`ChangeDisplaySettingsExW` sur une sortie virtuelle dont une duplication DXGI
est ouverte et détenue.** C'est le cas du produit, et c'est exactement l'écart
que D8 a laissé béant : la sonde P1 n'ouvre **jamais** de `DuplicateOutput`.

### 3.2 Le montage

Extension de `agent/src/diagnostics/multifenetre/mode_sortie.rs` (421 lignes,
marge 79), pilotée par la variable existante `MULTIFENETRE_MODE_SORTIE` :

1. créer une sortie virtuelle ;
2. **ouvrir une `DesktopCapture` dessus et la tenir** pendant toute la
   tentative ;
3. tenter le changement de mode ;
4. **juger sur la relecture DXGI, jamais sur le code de retour.**

Le point 4 n'est pas une précaution de style : `mode-sortie-1728x1080.log`
montre l'idiome `CDS_UPDATEREGISTRY|CDS_NORESET` puis `CDS_RESET` **annonçant
`0`** sur une sortie qui n'a pas bougé d'un pixel.

### 3.3 La contre-épreuve, obligatoire

**C'est la leçon littérale de P1.** Le premier verdict de D8 était `P1 REÇU`
rendu par un critère qui **ne pouvait pas rendre l'autre valeur** — la sonde
demandait à la sortie la taille qu'elle avait déjà. Deux garanties, donc :

- la sonde **exclut structurellement la taille courante** des cibles
  candidates, et rend `NON MESURABLE` si aucun mode annoncé n'en diffère (acquis
  de la tâche 3bis, à conserver) ;
- elle **rejoue le même geste sans duplication ouverte** dans la même
  exécution. Sans ce témoin, un refus s'imputerait à la duplication alors qu'il
  viendrait du mode choisi.

### 3.4 La quatrième combinaison de drapeaux — le remède de C1

Les trois combinaisons éprouvées par D8
(`mode_sortie.rs:256-259`) portent **toutes** `CDS_UPDATEREGISTRY`. Le
changement de mode **dynamique non persisté** — `flags = 0` — n'a jamais été
tenté, ni par la sonde ni par le produit. Or c'est, par définition, l'appel qui
n'écrit pas au registre.

La phase P l'éprouve donc comme quatrième bras, croisé avec la duplication
ouverte.

- **S'il fait bouger la sortie**, **C1 disparaît entièrement** : plus
  d'écriture au registre, donc plus de pollution, donc plus de blocage des
  ouvertures de fenêtre ultérieures, et l'alternative des cinq GUID SudoVDA
  devient sans objet.
- **Sinon**, C1 exige un remède de repli — restaurer le registre à une taille
  de référence après coup — et le bloc l'écrira plutôt que de le supposer.

### 3.5 Ce que la phase P relève en plus, gratuitement

Les inconnues n°2 et n°3 de D8 se mesurent au même endroit et au même moment :

- le nombre de pertes d'accès `0x887a0026` infligées aux **voisines** ;
- la conservation du nom `\\.\DISPLAYn` de la sortie retaillée.

**Les trois inconnues tombent ensemble ou pas du tout.**

### 3.6 Règle de décision, écrite AVANT la mesure

C'est le geste que D3 avait fait et qui lui a évité un arbitrage à chaud.

> **Si le pilote refuse le changement de mode sur une sortie à duplication
> ouverte, `changer_mode_de_sortie` encadre son appel par relâcher la
> duplication → changer le mode → la rouvrir.**

C'est la mécanique de reprise de D2, éprouvée à **44** pertes d'accès
encaissées sans tuer une session — et c'est **aussi le remède de C2**, donc un
seul chemin et non deux.

**Coût écrit d'avance** : une interruption de capture bornée sur *cette*
fenêtre, et un abandon de mutex probablement infligé aux voisines à chaque
entrée en plein écran. La recette ① le relève ; elle ne le seuille pas, aucun
seuil acceptable n'étant connu à ce jour.

## 4. Famille ① — armer le plein écran (legs 6, 12, 13, 7, 8, 10)

### 4.1 C1 — la pollution du registre

Traitée par la phase P (§3.4). Le produit adopte la combinaison de drapeaux qui
fait bouger la sortie **sans** écriture au registre si elle existe ; sinon il
restaure le registre après coup, et le document nomme ce repli pour ce qu'il
est — un pansement sur un comportement de pilote non expliqué.

### 4.2 C2 — la reprise court-circuitée

`reconstruire_sur_la_sortie` cesse de rendre `Err` sur un échec de réouverture
**retentable** : elle emprunte `est_ouverture_retentable` et la fenêtre de
reprise de D2. Si §3.6 s'applique, c'est le **même** code.

### 4.3 Leg 7 — HiDPI

L'asymétrie est littérale, et le code la nomme lui-même
(`windows_source/redimensionnement.rs:57-62`) : la sortie naît sur
`innerWidth`, le `Resize` arrive en `video.clientWidth × devicePixelRatio`
(`client/src/main.ts:284-285`). À `devicePixelRatio > 1` le court-circuit
anti-`Resize`-de-routine ne retient donc plus rien, et **chaque connexion de
chaque fenêtre déclencherait un changement de mode**, avec un écart de 25 à
100 %.

**Remède** : l'annonce de viewport passe dans la **même unité** que le
`Resize`, c'est-à-dire multipliée par le `devicePixelRatio`.

**Il n'y a qu'un `devicePixelRatio` en jeu, et c'est ce qui rend le remède
sûr** : `client/src/main.ts:41` montre que c'est **la page d'application
elle-même** qui annonce `viewportPair(window.innerWidth, window.innerHeight)`,
sur le `window` dont le `ResizeObserver` de la même page enverra plus tard
`video.clientWidth × window.devicePixelRatio`. Ce n'est pas le shell qui
annonce, et les deux valeurs ne peuvent donc pas venir d'écrans différents.

**Ordre des opérations** : multiplier par le dpr **puis** arrondir en pair —
`viewportPair` a un plancher à 2, et arrondir avant de multiplier laisserait
passer une hauteur impaire à dpr impair.

Les deux grandeurs redeviennent ainsi comparables à tout dpr, et le chemin
mono-fenêtre `FenetreRecadree` — qui a besoin de la valeur multipliée — la
reçoit inchangée.

**Mesurable, contrairement à ce que D8 pouvait faire** :
`Emulation.setDeviceMetricsOverride` accepte `deviceScaleFactor`, que le
montage de D8 n'a jamais employé (dpr = 1 partout, d'où un défaut
structurellement invisible).

### 4.4 Legs 8 et 10 — la chaîne viewport → `Resize`

**Découverte du code, non documentée jusqu'ici** : `client/src/main.ts:283`
abandonne **en silence** si `session.controlChannel.readyState !== 'open'` au
moment où la temporisation de 200 ms expire, et **ne réémet jamais** —
l'observateur ne se redéclenche que si l'élément change encore de taille.
C'est un candidat direct au leg 10 (deux `Resize` pour cinq sessions) et au
leg 8 (la chaîne qui casse au critère ②).

**Remèdes** :

- retenir la dernière taille observée et la **rejouer à l'ouverture du canal** ;
- tracer l'abandon côté client, au lieu du `return` muet ;
- **`agent/src/demarrage.rs:381` reçoit son champ `session`.** Sans lui,
  « 2 `Resize` pour 5 sessions » reste indécidable — c'est précisément ce que
  la correction I8 de D8 a établi.

⚠️ **Le transport n'est disculpé par aucune pièce.** La correction C3 de D8 a
réfuté l'argument qui le disculpait : la phase ② porte **141 s sans une seule
ligne `contrôle reçu`**, aux deux exécutions. Le `ResizeObserver` est **une
hypothèse, la mieux étayée**, pas un maillon désigné — le canal de contrôle
reste suspect, et l'instrumentation doit permettre de trancher entre les deux.

## 5. Famille ② — l'audio mort et l'identité de session (legs 1, 2)

### 5.1 Leg 1 — le signal enfant→capteur

**État vérifié** : `capture_morte` est posé par `windows_audio.rs:342,406`
après `LECTURES_ECHOUEES_MAX = 10` échecs consécutifs, et lu **uniquement** par
`transport/piste_audio.rs:133`, pour une ligne de journal. **Le capteur ne le
voit jamais**, donc la fenêtre voisine n'est jamais promue et le groupe reste
muet sans retour.

**Message neuf** : `VersCapteur::AudioMort { session }`, sur le patron exact de
`Visibilite` — **poussé, non répondu par `Fait`**, parce que la décision peut
concerner une *autre* fenêtre que celle qui signale. Il part de là où l'enfant
relaie déjà la visibilité ; il est reçu par le serveur du capteur.

**Règle neuve**, dans `agent/src/capteur/audio.rs` — **pur, aucun `cfg`,
éprouvé sur l'hôte**, comme l'arbitrage de D7 qu'elle étend :

1. la fenêtre signalée cesse d'être porteuse et devient **inéligible** ;
2. si le groupe de PID a une voisine vivante, elle est élue — **activation
   *process loopback* neuve**, donc une chance réelle là où le client WASAPI
   périmé n'en avait plus ;
3. **sinon la même fenêtre redevient éligible après un répit borné**, ce qui
   construit elle aussi une activation neuve. Sans cette branche, le remède ne
   couvrirait que les applications multi-fenêtres, et laisserait muet le cas
   majoritaire — une application, une fenêtre ;
4. le cycle est **borné en nombre**, pour qu'un périphérique définitivement mort
   ne tourne pas sans fin.

Le réarmement réemploie la variante `DepuisCapteur::Audio { emet }`
**existante** : aucune variante neuve dans ce sens.

⚠️ **Deux constantes neuves, NON CALIBRÉES** — le répit et la borne de
réarmement. Elles rejoignent `BPP_MIN`, `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`,
`HYSTERESIS`, `REPIT_APRES_ECHEC` et `TAILLE_MAX_SORTIE` : nommées comme telles,
pas tues.

⚠️ **Le bras catch-all de `capteur/pont_media.rs`**, qui tue le fil
`lire_le_media` **en silence**, a été payé **quatre fois** (D5 `Sommeil`, D6
`Part`, D7 `Audio`, D8 `PleinEcran`). `AudioMort` allant dans `VersCapteur` et
non dans `DepuisCapteur`, il n'est pas sur le chemin — **il sera vérifié quand
même**, c'est moins cher que la cinquième.

### 5.2 Leg 2 — l'identité par génération monotone

`VersCapteur::Attache` porte une `generation: u64` frappée par le superviseur
au lancement. Le registre la retient, et **`retirer` devient sans effet si la
génération enregistrée est plus récente** que celle qu'on lui présente. Un
rattachement qui réinscrit le même nom ne se fait donc plus emporter par le
`retirer` de l'instance précédente.

Le registre (`capteur/sommeil.rs`, 327 lignes) garde son point de passage
unique `oublier`, acquis de la vague de correction de D6.

### 5.3 Le plafond de 500 lignes est sur ce chemin

**Relevé par la commande le 6 août 2026** :
`agent/src/capteur/serveur.rs` vaut **490 lignes, marge 10**. `CLAUDE.md` exige
pour ce fichier « une extraction, jamais une compression du commentaire de
`TAMPON` ». **La tâche qui pose `AudioMort` porte donc son extraction**, dans le
même mouvement — elle ne la remet pas au bloc suivant.

## 6. Famille ③ — la dette de mesure de D6, et l'instrument (legs 3, 4, 5, 9)

### 6.1 Leg 3 — la télémétrie par session

**État vérifié** : les trois statiques (`windows_source.rs:453-455`) sont
écrites aux lignes 313, 518 et 544 — donc dans le **capteur** depuis D4 — et
lues au seul `demarrage.rs:127-129`, donc dans l'**enfant**. `SOURCE_TRACE=1`
**est** transmis par `scripts/run-agent.sh:38` : la variable arrive bien, mais
n'affiche que des zéros, et les compteurs du capteur ne sont lus par personne.

**Remède** : elles deviennent des **champs de `WindowsSource`** — un jeu par
fenêtre, la granularité voulue — et sortent vers
**`agent/src/windows_source/telemetrie.rs`**. Le lecteur devient le fil de
fenêtre du capteur, qui porte **déjà** son span `session` depuis D7.

**Effet de bord recherché** : `windows_source.rs` (638 lignes, dette gelée)
passe sous la barre que D6 lui avait fait franchir, et la condition « la
prochaine addition exige une extraction » tombe.

### 6.2 Leg 4 — le critère ④ à palier long

Le pilote de D6
(`plans/journaux-multifenetres-d6/instrument/pilote-recette-d6.mjs`) se
réemploie tel quel. Palier de **60 s** contre un `DELAI_REMONTEE` de 20 s, là
où D6 mesurait à 25 s — 25 % de marge, dans laquelle devaient encore tenir
l'annonce de focus, la redistribution des parts et la montée de l'estimation.
**Trois promotions sur quatorze étaient arrivées après la fin du palier.**

Question binaire : la promotion de focus est-elle **systématique** quand on lui
laisse le temps ?

### 6.3 Leg 5 — l'A/B différentiel sur `set_desired_bitrate`

Une variable de banc neutralise l'appel de `transport/part.rs:71`, et l'on
oppose les deux trafics cumulés à huit fenêtres.

⚠️ **Le témoin doit porter sur des fenêtres ÉVEILLÉES.** C'est précisément ce
qui a empêché la recette de D6 de servir : une endormie émet **0,000 Mb/s sur
30 s**, si bien que « sondage borné à la part » et « pas de sondage du tout » s'y
lisent à l'identique.

⚠️ **Ce que cet A/B établira, et rien de plus** : que l'appel a un effet
observable sur le trafic émis. Il n'établira **pas** qu'il est nécessaire — la
prémisse qui le disait « le plus important » est réfutée par D6 elle-même, le
pont portant ≥ 1,44 Gb/s pour `packetsLost = 0`.

⚠️ **La variable neuve va dans `scripts/run-agent.sh` dans la tâche même qui la
crée.** Ce piège a été payé en D1 (`SUPERVISEUR`), D2 (`MULTIFENETRE_REPRISE`)
et D6 (`BUDGET_BPS`).

### 6.4 Leg 9 — l'instrument

Trois correctifs à `pilote-recette-d8.mjs`, que la recette ① réemploie :

- le seuil `audio_survit` se compare à la **dominante**, non au plancher de
  bruit (−158 dB) : en l'état il ne peut quasiment pas échouer ;
- `verdict_partie_mesurable` cesse de contredire le verdict publié — il vaut
  `false` aux deux exécutions de D8 là où le texte dit ① CONFIRMÉ ;
- `window.__pleinEcran` se **borne par horodatage**, faute de quoi une balise
  d'identité antérieure produit un faux positif de « fuite ».

## 7. Recettes et critères de réception

Une sonde de banc, puis trois recettes produit.

| | Objet | Reçu si |
| --- | --- | --- |
| **P** (banc) | l'éliminatoire × quatre combinaisons de drapeaux, avec témoin sans duplication | les quatre bras ont tourné **et** la sonde a été vue rendre les **deux** valeurs |
| **①** | le plein écran armé — le critère ② de D8, enfin exercé | le flux suit le viewport ; la sortie garde son nom ; les pertes d'accès infligées aux voisines sont **relevées** ; à `deviceScaleFactor = 2`, **aucun** changement de mode parasite à la connexion |
| **②** | l'audio mort, deux montages ; non-régression du rattachement | la dominante revient dans les deux montages ; le contrôle a été vu **ROUGE** sans le remède |
| **③** | le critère ④ à palier 60 s ; l'A/B `set_desired_bitrate` | un taux de promotion sur N déplacements ; un écart de trafic cumulé entre les deux bras |

### 7.1 « Vu rouge » est une exigence, pas une formule

Ce dépôt a payé **trois fois d'affilée** un contrôle qui ne pouvait pas
échouer : F1 de D7 (la trace `compteurs audio` dont le champ `actif` n'avait
qu'une valeur atteignable), le premier verdict P1 de D8, et le parseur de
session du pilote de D8. **Chaque critère de D9 se joue d'abord sans son
remède**, et le document verse la pièce du rouge.

### 7.2 Deux exécutions par critère, pas une

Tous les sous-blocs de D1 à D8 se terminent sur « aucun taux, nulle part ».
Deux exécutions n'en font pas un, mais elles distinguent un fait d'un tirage —
et c'est ce qui a manqué à D6, dont une même mesure variait d'un **facteur 19**
à binaire, budget et protocole identiques.

C'est le poste de coût principal du bloc. **Survie de la VM contrôlée après
chaque rang** : elle s'hiberne d'elle-même, déclencheur non identifié, et deux
mesures ont déjà été perdues ainsi.

### 7.3 Montages, tous acquis des sous-blocs précédents

- **Provoquer la mort de capture audio** : `Restart-Service Audiosrv` est
  nommément l'une des causes transitoires que `agent/src/audio.rs:69-70` invoque
  pour justifier ses dix réessais.
- **Deux fenêtres d'un même groupe de PID** : deux fenêtres Chrome `--app`
  partageant un `--user-data-dir`, donc **un seul `chrome.exe`** — montage établi
  par D8, PID vérifié par deux voies indépendantes. ⚠️ **Deux `notepad.exe` sont
  deux PID distincts** : le piège a été trouvé avant dispatch en D8, ne pas le
  rejouer.
- **Le verdict audio se lit à la fréquence dominante**, jamais à un compte
  d'octets : D7 a mesuré `bytesReceived` croissant sur un spectre à −1000 dB.
- **Une session WebRTC vivante est requise** pour que toute mesure audio dise
  quelque chose : `Session::run()` est la seule boucle qui consomme l'ordre
  audio du capteur.
- **Le navigateur pilote tourne sur l'HÔTE**, jamais sur la VM, où la fenêtre
  de la page-shell serait elle-même capturée.

### 7.4 Revue transverse de fin de branche — obligatoire

Elle a trouvé **cinq** défauts en D7 et **trois** Critiques en D8, tous
franchissant une frontière de tâche. Chacun était correct des deux côtés pris
séparément : **une revue par tâche ne peut structurellement pas les voir.**

## 8. Le plafond de 500 lignes — budgété d'avance

Relevé **par la commande** le 6 août 2026, sur les fichiers que D9 touchera :

| Fichier | Lignes | Conséquence pour D9 |
| --- | --- | --- |
| `agent/src/windows_source.rs` | **638** | dette gelée. Le leg 3 l'**allège** — c'est l'extraction que sa condition exige |
| `agent/src/capteur/serveur.rs` | **490** (marge 10) | ⚠️ le leg 1 y ajoute un bras : **extraction dans la même tâche** |
| `agent/src/diagnostics/multifenetre/mode_sortie.rs` | **421** (marge 79) | la phase P y ajoute un bras et un témoin |
| `agent/src/capteur/protocole.rs` | **369** (marge 131) | `AudioMort` et `generation` |
| `agent/src/windows_source/redimensionnement.rs` | **345** (marge 155) | C2 et le garde |
| `agent/src/capteur/sommeil.rs` | **327** (marge 173) | la génération au registre |
| `client/src/main.ts` | **295** | legs 7, 8, 10 |
| `agent/src/capteur/audio.rs` | **152** | la règle d'arbitrage étendue |

Aucun de ces nombres n'est recopié d'un document : ils dérivent, et
`CLAUDE.md` porte la trace de **cinq** naufrages successifs pour l'avoir oublié.
**Les relever de nouveau en fin de bloc, et corriger les tableaux de `CLAUDE.md`
à toutes leurs places** — « corrigé à sa place » est une affirmation de
complétude, et une affirmation de complétude se vérifie en énumérant les places
avant de l'écrire (`grep -n '<le nombre>' CLAUDE.md`).

## 9. Gestion des erreurs

| Panne | Comportement attendu |
| --- | --- |
| La phase P rend `NON MESURABLE` | Le bloc s'arrête sur la famille ① et le dit ; les familles ② et ③ continuent |
| Le pilote refuse le changement de mode sur duplication ouverte | §3.6 s'applique — relâcher / changer / rouvrir |
| `flags = 0` ne fait pas bouger la sortie | C1 prend son remède de repli, nommé comme pansement |
| Une réouverture de duplication échoue de façon **retentable** | La fenêtre de reprise de D2 l'encaisse (c'est C2) |
| Une réouverture échoue de façon **définitive** | La session meurt proprement, comme aujourd'hui |
| La capture audio meurt et aucune voisine n'existe | Réarmement après répit, borné en nombre ; au-delà, silence **journalisé**, jamais muet |
| Le canal de contrôle n'est pas ouvert au moment d'un `Resize` | La taille est retenue et rejouée à l'ouverture ; l'abandon est tracé |
| La VM s'hiberne en pleine mesure | Contrôle de survie après chaque rang ; le rang est rejoué, jamais recollé |

## 10. Stratégie de test

- **Pur, sur l'hôte** : la règle d'arbitrage audio étendue (`capteur/audio.rs`),
  la génération au registre (`capteur/sommeil.rs`), la télémétrie extraite
  (`windows_source/telemetrie.rs`). Ces trois-là **doivent** être testables sans
  `cfg` — c'est la discipline que D6 et D7 ont établie en sortant les règles
  pures de leurs modules Windows.
- **Compilation croisée** : `cargo check --target x86_64-pc-windows-gnu` depuis
  `agent/` avant toute compilation distante. ⚠️ Couvre types, emprunts,
  visibilités et durées de vie ; **ne couvre PAS l'édition de liens**.
- **Client** : `client/src/*.test.ts` pour le rejeu de `Resize` à l'ouverture du
  canal et la normalisation d'unité du viewport.
- **VM** : les quatre passes du §7. Rien n'est compté fermé sans sa pièce.

## 11. Ce que D9 n'établira PAS

Écrit d'avance, pour qu'aucun rapport ne l'affirme :

- **La latence de bout en bout**, qu'aucun sous-bloc du chantier D n'a mesurée
  et que D9 n'ajoute pas.
- **Les trois couches inconnues** : le plafond de 8 encodeurs, celui de 4
  processus, et le mécanisme de l'abandon du mutex DXGI.
- **La course du leg 2**, non provocable à la main de façon fiable : le
  correctif se prouve par tests d'hôte sur le registre pur ; la VM n'établira
  que la non-régression du rattachement.
- **Le répit et la borne de réarmement audio**, non calibrés — comme `BPP_MIN`,
  `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`, `HYSTERESIS`, `REPIT_APRES_ECHEC` et
  `TAILLE_MAX_SORTIE` avant eux.
- **Aucun jugement visuel** n'est porté, sur aucune constante.
- **Le chemin d'extinction propre du superviseur**, jamais exercé depuis D1.
- **Rien au-delà des rangs joués**, et rien d'un client réel : le montage reste
  un Chrome sans interface, à décodage logiciel, sur l'hôte qui porte la VM.
- **La visibilité et le focus restent IMPOSÉS par le pilote de recette**, page
  par page — limite héritée de D5, la plus lourde du montage, qu'aucun sous-bloc
  n'a levée.

## 12. Hors périmètre

- Le chantier E (microphone), spécifié le 28 juillet 2026 et jamais implémenté.
- Tout comportement neuf du produit : D9 ferme ce qui est ouvert, il n'ouvre
  rien. La seule exception assumée est la règle de réarmement audio du §5.1,
  qui est un comportement neuf **parce que le leg 1 en exige un** — promouvoir
  une voisine ne couvre pas le cas majoritaire.
