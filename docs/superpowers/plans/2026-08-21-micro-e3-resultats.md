# Chantier E — Microphone, bloc E3 : le micro en multi-fenêtres — résultats

**Plan :** `docs/superpowers/plans/2026-08-21-micro-e3.md`
**Spécification :** `docs/superpowers/specs/2026-07-28-micro-design.md` — ⚠️ **écrite
le 28 juillet 2026, AVANT les chantiers A, B, C et D.** Le plan de E1 porte un
tableau de vieillissement, celui de E3 un second. **Ne recopier aucune
affirmation de la spec sans passer par l'un des deux.**
**Journaux :** `docs/superpowers/plans/journaux-micro-e3/` — la note de lecture,
relevée par la commande, vit dans `familles-de-lecture.txt`.

🔴 **AUCUN TAUX N'EST REVENDIQUÉ NULLE PART.** Le nombre d'exécutions est dans
chaque énoncé. Deux exécutions établissent la **reproductibilité**, jamais une
fréquence.

---

## 0. Où E3 s'arrête, et pourquoi — à lire avant tout le reste

Le plan porte **quatre** livrables. **Trois sont livrés et mesurés ; le
quatrième n'a pas été approché.**

| | Livrable | Sort |
| --- | --- | --- |
| ① | le refus d'exclusivité est **DIT au client** | ✅ **LIVRÉ ET MESURÉ**, 2 exécutions |
| ② | l'exclusivité **à DEUX ENFANTS** est exercée | ✅ **LIVRÉ ET MESURÉ**, 2 exécutions |
| ③ | l'occupation du tampon devient **observable** | ✅ **LIVRÉ ET MESURÉ**, 2 exécutions |
| ④ | **l'écho en multi-fenêtres** | ✅ **MESURÉ**, 2 exécutions par bras — voir le §4bis |

✅ **LE CONSENTEMENT A ÉTÉ DONNÉ, ET LE LIVRABLE ④ EST MESURÉ.** Il avait
d'abord été déclaré **non approché** : la Décision 5 du plan impose de créer un
nœud PipeWire dans le graphe audio de l'utilisateur de la machine hôte, et
l'autorisation manquait. Elle a été accordée, et la sonde S1 a été jouée.

🔴 **LA DÉCISION 7 DE E1 EST RÉFUTÉE, ET LA SPEC §4 EST ANNOTÉE** — ce que ce
document déclarait impossible faute de mesure. **La ligne de partage de l'AEC de
Chrome n'est ni l'onglet ni le périphérique : c'est L'INSTANCE DE NAVIGATEUR.**

⚠️ **Mais la Décision 3 tient sans changement** : le banc est **unilatéral**, il
**réduit** le soupçon et ne le **lève** pas, et **le protocole humain reste dû**.

⚠️ **Le protocole humain de l'écho reste dû dans TOUS les cas** — c'est la
Décision 3 du plan, et elle est asymétrique : le banc de salle émulée peut
**confirmer** le défaut, il ne peut pas le **lever**. Son texte intégral vit
dans le plan, § « Le protocole humain ».

⛔ **Le plan ne choisit AUCUNE des trois voies de correction de l'écho** (A —
rassembler la restitution, ce qui **défait D7** ; B — une annulation côté agent,
que la spec §14 exclut nommément ; C — le casque, dit à l'utilisateur). **Ce
choix appartient au propriétaire du dépôt**, et il ne se prend pas avant que le
défaut soit mesuré.

---

## 1. Le fait qui gouverne : à deux fenêtres, le bouton cessait de mentir

E2 léguait, en son legs n°2 : *« à deux fenêtres, le bouton micro de la perdante
s'allume et **rien ne sort** »*. E3 l'a confirmé par lecture du code avant
d'écrire une ligne — `micro_disponible()` vaut `mic_mid.is_some() &&
puits_micro.is_some()` et **ne consulte jamais le mutex** —, puis l'a fermé.

**Le remède est celui que E2 avait nommé** : une variante neuve
`AgentControl::MicState { granted }`, `type: "mic-state"`, donc `proto/`
**et** `client/`.

### Ce qui a été mesuré, et par quoi

Le **témoin de fil** enregistre les messages tels qu'ils **arrivent** sur le
canal de contrôle, par un `addEventListener` **passif** posé sur
`RTCDataChannel`. **Il est indépendant de notre code client**, et c'est ce qui
fait sa valeur : sans lui, « le bandeau s'affiche » et « le message est arrivé »
se liraient pareil.

| | exécution 1 | exécution 2 |
| --- | --- | --- |
| `w-2` (la gagnante) `granted:true` | **+51 ms** après son clic | **+84 ms** |
| `w-1` (la perdante) `granted:false` | **+83 ms** après son clic | **+88 ms** |
| `w-1` `granted:true` (la reprise) | **+19 443 ms** | **+19 244 ms** |
| messages en tout | **3** | **3** |
| avant le premier clic | **0** | **0** |

🔵 **LE TÉMOIN CRIE AUSSI QUAND LE MESSAGE PART TROP TÔT, ET IL NE CRIE PAS.**
Aucun message avant le premier clic, et chacun arrive **après** le sien. C'est
le mode de panne qu'un chantier voisin venait de payer — un message émis avant
l'ouverture du canal, donc un **silence** —, et c'est son propre `console.warn`
qui l'avait dénoncé.

🔴 **SUR TRANSITION, ET C'EST MESURÉ : TROIS messages pour ~4 500 dépôts.** Le
micro dépose une trame toutes les 20 ms. Sans le garde de transition, la file de
contrôle — bornée à 32 par `PLAFOND_CONTROLE_EN_FILE` — aurait débordé en moins
d'une seconde et **noyé le curseur, la vibration et le presse-papier**.

### Ce que le client affiche, et ce qu'il n'affiche pas

**L'état reste `'actif'` dans les DEUX fenêtres**, aux deux exécutions. Seul le
libellé change :

```
w-2 : "Microphone actif — cliquez pour couper"
w-1 : "Microphone actif — mais une autre fenêtre tient le micro de la VM :
       celle-ci n'y est pas entendue"
```

🔴 **Éteindre le bouton d'une fenêtre qui capte réellement serait le mensonge
visuel que la spec §9 « Vie privée » qualifie d'inacceptable « sur cette
fonction précisément »** : la piste EST ouverte, le navigateur ÉMET, et
l'indicateur de capture de Chrome est allumé. Et ranger ce refus dans l'état
`'refuse'` confondrait deux causes qui appellent **deux gestes opposés** —
l'une se répare dans les réglages du navigateur, l'autre en fermant l'autre
fenêtre.

⚠️ **La formulation exacte est un JUGEMENT HUMAIN**, et **personne ne l'a lue à
l'écran**. Elle rejoint la liste que ce dépôt tient depuis `BPP_MIN`.

---

## 2. Le livrable ② : l'exclusivité à deux enfants, enfin le bon montage

E2 ne l'avait exercée que contre un **processus tiers** tenant le même mutex
nommé — donc le **chemin de code**, jamais le **montage**. Trois raisons y
étaient données ; **E3 en a trouvé une PÉRIMÉE** (« le registre reste pollué »
ne borne plus rien depuis D10, qui fait tolérer au superviseur une sortie née
trop grande : 3 → 10 fenêtres, 32 → 0 erreur, sur un registre laissé sale). Les
deux autres tenaient, et **le blocage était le périmètre, pas l'impossibilité**.

**Montage :** mode SUPERVISEUR, deux Bloc-notes ouverts en session 1 **avant**
le superviseur, deux enfants, deux fenêtres navigateur, une page-shell.

| Relevé | exécution 1 | exécution 2 |
| --- | --- | --- |
| `enfant lancé` | **3** (les deux du montage, plus la relance de la phase C) | **3** |
| `micro : une autre fenetre tient deja le cable` | **1**, sur `session=w-1` **seule** | **1**, idem |
| la gagnante en porte | **0** | **0** |
| `micro : cable acquis apres un refus` | présent, sur `w-1` | présent |
| juge sur CABLE Output, phases A / B / C | **440,0 Hz** aux trois | **440,0 Hz** |
| `ERROR` au journal d'agent | **0** | **0** |

⚠️ **`AUDIO_PERIPHERIQUE` N'A PAS ÉTÉ POSÉE, et c'est la Décision 8** : en
multi-fenêtres `loopback_de_session` vaut `config.audio &&
fenetre_hwnd.is_none()`, et `fenetre_hwnd` est `Some` dans un enfant — **la
garde de boucle locale est INERTE PAR CONSTRUCTION**. Toute la recette de E2 la
posait ; c'est un artefact du mono-fenêtre, et la poser ici aurait fait mesurer
autre chose que le produit livré.

⚠️ **REPLI DE LA DÉCISION 9 EMPLOYÉ, ET DÉCLARÉ : UNE SEULE TONALITÉ.**
`--use-file-for-fake-audio-capture` est un drapeau de **processus**, et la
page-shell ouvre ses N fenêtres par `window.open` dans **son** instance : les
deux portent nécessairement le même 440 Hz. **Le juge établit donc que
QUELQU'UN est entendu, jamais LEQUEL.** Le discriminant repose sur les deux
autres pièces que la Décision 9 nomme — la ligne de journal unique sur la
perdante, et l'état affiché par les deux clients —, et elles sont nettes.

**La reprise (R4 sur le chemin réel)** : l'enfant gagnant est tué **par son PID
relevé dans le journal**, jamais par `pkill -f` (qui tue son propre shell —
piège documenté depuis D2, rejoué cette semaine) et jamais par une heuristique
de rang de PID (« le plus jeune des `agent` » est **faux** : superviseur,
capteur, pont, puis les enfants). Après la mort, `w-1` reçoit un **second**
`mic-state` et son titre redevient nominal, aux deux exécutions.

---

## 3. Le livrable ③ : l'occupation du tampon, et ce qu'elle ne dit pas

E2 avait établi que l'occupation **n'est pas observable** sur le chemin de
production : les deux grandeurs ne vivaient que dans la trace du **puits de
mesure** (`MICRO_MESURE=1`), qui **ne peut pas coexister avec le câble**.
**Elles existaient déjà** — `LecteurMicro::occupation()` est publique depuis E1,
`CompteursMicro::famines` est un champ depuis E1 — et n'étaient lues par
personne : il n'y avait rien à calculer, seulement à tracer.

`instrument/controle-trace-cable.sh`, **2 exécutions** :

```
lignes 'micro ecrit sur le cable' : 166
  dont portant occupation_ms=     : 166
  dont portant famines=           : 166
VERDICT : VERT
```

**Et les VALEURS ont un sens**, ce que la seule présence ne dirait pas :

| | `deposees` | `famines` | `occupation_ms` |
| --- | --- | --- | --- |
| micro éteint | 0 | **102** | 0 |
| micro allumé, exéc. 1 | 51 | **0** | **40** |
| micro allumé, exéc. 2 | 51 | **0** | **20** |

Les famines tombent à zéro **exactement** quand le micro alimente, et
l'occupation **diffère d'une exécution à l'autre** : c'est une mesure, pas une
constante.

⚠️ **Les deux lectures se font sous le MÊME verrou**, et ce n'est pas une
commodité : `occupation` est un **instantané**, et le lire à un second
verrouillage le daterait d'un autre moment que les compteurs, sur un tampon que
le fil de dépôt fait bouger toutes les 20 ms.

🔴 **CE QUE CELA NE DONNE PAS : la latence de bout en bout, que RIEN ne mesure
dans ce dépôt depuis D1.** C'est la **seconde** des deux composantes de la
latence ajoutée par l'agent, dont E2 n'avait que la première (`retards`). **LE
TROISIÈME CRITÈRE DE LA SPEC §13 RESTE NON JUGÉ.**

---

## 4. Les résidus de E2 — un fermé, un fermé, un nommé

### `MICRO_FAUTE_ECRITURE` : armée pour la PREMIÈRE fois, et elle mord (1 exécution)

| Relevé | Valeur |
| --- | --- |
| le `run-agent.ps1` **GÉNÉRÉ** | `$env:MICRO_FAUTE_ECRITURE = '3'` |
| `injection de fautes d'ecriture … ARMEE` | **2** — une par enfant : le budget est **global au PROCESSUS** |
| `micro : ecriture sur le cable echouee, fil de rendu arrete` | **2**, 27 µs après l'armement |
| `micro ecrit sur le cable` | **0** — le fil meurt avant d'avoir tracé |
| juge sur CABLE Output | **AMPLITUDE = 0,000000** |

🔴 **LE CONTRÔLE QUI COMPTE EST LE `run-agent.ps1` GÉNÉRÉ, jamais le tracé du
code** — piège payé six fois dans ce dépôt, dont une par l'instrument d'un
chantier qui le connaissait.

🔵 **ET CETTE EXÉCUTION EST LE TÉMOIN NÉGATIF DE TOUTE LA CAMPAGNE.** Le même
juge, sur le même point de terminaison, rend 440,0 Hz aux deux exécutions
nominales et 0,000000 ici. **Sans elle, « le juge entend 440 Hz » ne serait pas
discriminant d'un juge qui entendrait 440 Hz quoi qu'il arrive.** *Un zéro se
qualifie avant de se rapporter, et seul un témoin négatif le fait.*

🔴 **ELLE A RÉFUTÉ UNE AFFIRMATION DE E3 LUI-MÊME, ÉCRITE TROIS COMMITS PLUS
TÔT.** `MicStateMessage` documentait : « `granted: true` […] veut dire : ce que
ce micro capte **atteint la VM** ». **Faux, et mesuré** : le fil de rendu était
mort, le juge relevait 0,000000, et la fenêtre a reçu `granted: true` quand
même — le mutex vit dans `PuitsCable::deposer`, le fil de rendu est ailleurs,
et **rien ne les relie**. Corrigé des **deux** côtés : **ce champ est un verdict
d'EXCLUSIVITÉ, jamais un accusé de réception.** Un `false` est concluant ; un
`true` ne l'est pas.

### Le résidu de la sonde 3 de la spec §12 : fermé (1 exécution)

E1 avait relevé que CABLE Output est déjà le microphone par défaut, mais **en
session 0 (WinRM)**, et déclarait la concordance avec la session interactive
« plausible, **non mesurée** ». Elle est mesurée, par tâche planifiée `/it`, et
la ligne `SESSION=1` **prouve** la session au lieu de la supposer :

```
SESSION=1
CAPTURE_Default        = {0.0.1.00000000}.{5fae72b2-f885-4038-9827-0d91f862a7d5}
CAPTURE_Communications = le MÊME identifiant
ENDPOINT=CABLE Output (VB-Audio Virtual Cable) | …{5FAE72B2-…}
```

⚠️ **WinRT n'expose que DEUX rôles** (`Default` couvre `eConsole` et
`eMultimedia`) là où l'API COM en distingue trois : ce relevé ne les sépare pas,
et le dire vaut mieux que d'en annoncer trois.
⚠️ **La voie COM a échoué** — le transtypage d'un `__ComObject` vers une
interface `ComImport` rend `$null` **en silence** sous ce PowerShell, interfaces
imbriquées comme au niveau du namespace, ordre de vtable corrigé. Le symptôme,
« Impossible d'appeler une méthode dans une expression Null », ne désigne pas sa
cause. **La sonde COM est versée avec son échec.**

### Les deux replis jamais courus : nommés, et on passe

`Local\` (mutex) et `Reveil::Echeance` restent du code jamais couru. **Décision
du plan, tenue : ne PAS écrire un levier pour l'occasion** — ce serait du code
de banc livré pour une recette, et le legs est plus honnête que l'artefact.

---

## 4bis. La sonde S1 — la prémisse du bloc, enfin mesurée

**Elle ne demande ni la VM, ni l'agent, ni le câble virtuel** (Décision 4) : la
question « Chrome annule-t-il ce qu'une AUTRE fenêtre restitue ? » se tranche
avec deux pages, une sortie, une entrée et un `AnalyserNode`. ⚠️ **Le prix est
nommé : ce banc mesure CHROME, pas LE PRODUIT.**

### Le verdict, et il tient en une ligne

🔴 **« L'AEC de Chrome n'annule que ce que SON PROPRE ONGLET restitue » (E1,
Décision 7) est FAUX DES DEUX CÔTÉS.** L'AEC couvre **plus** que l'onglet et
**moins** que le périphérique : son périmètre est **l'INSTANCE DE NAVIGATEUR**.

| Montage | Profondeur d'annulation du son de l'**autre** fenêtre | Issue | Exéc. |
| --- | --- | --- | --- |
| **même** instance de Chrome (S1bis) | **+81,9** et **+89,1 dB** | **B** | 2 |
| **deux** instances de Chrome (S1ter) | **−13,6** et **−13,2 dB** | **A** | 2 |

⚠️ **Le nombre négatif n'est pas une annulation manquée : c'est une
AMPLIFICATION.** L'`autoGainControl` monte le gain puisque le signal utile a
disparu, et **la dominante captée AVEC l'AEC devient 662,1 Hz à −29,1 dB** — le
son de l'autre instance **domine** ce que la fenêtre envoie.

🔵 **CONSÉQUENCE PRODUIT, ET ELLE EST FAVORABLE.** La page-shell ouvre ses N
fenêtres par `window.open`, **dans SA propre instance** : le montage du produit
est celui de la première ligne. **Le défaut que E1 redoutait n'existe pas entre
deux fenêtres du produit.**

🔴 **CE QUI RESTE, ET QUI EST MESURÉ** : le son d'une **autre application** — un
lecteur, une autre visioconférence, un autre navigateur — n'est **pas** annulé.
C'est un défaut réel ; simplement **pas celui qu'on avait nommé**.

🔴 **ET LE BANC NE LÈVE RIEN (Décision 3, écrite deux fois dans le plan).** Une
salle émulée n'est pas une pièce : pas de réponse de salle, pas de retard de
propagation, **pas de distorsion non linéaire de haut-parleur** — et c'est
précisément la non-linéarité qui met une annulation d'écho en défaut. **Issue A
⇒ le défaut est établi. Issue B ⇒ RIEN n'est levé.** Le protocole humain reste
dû **quel que soit ce verdict**.

### Trois témoins, et il en fallait trois

1. **TÉMOIN POSITIF**, joué **avant toute conclusion** (step 4 du plan) : la
   fenêtre qui capte joue *sa* tonalité dans la salle. **L'AEC en retire 65,5 /
   70,6 / 78,7 / 81,2 / 81,3 dB** selon l'exécution. Sans lui, « l'AEC ne couvre
   pas B » et « l'AEC ne fait rien » se liraient pareil — c'est l'**issue C**, et
   elle est **écartée par mesure**.
2. **TÉMOIN DE PRÉSENCE** : `aec:false` relève 660 Hz à **−42,7 dB** dans toutes
   les exécutions. **B arrive bien dans la salle** ; sans cela, « annulée » et
   « jamais arrivée » se liraient pareil.
3. **TÉMOIN NÉGATIF — le bras « avec casque »** : B cesse de rendre, et 660 Hz
   **chute de 120,1 dB** (−42,7 → −162,8).

Et **le juge lui-même a ses deux témoins** : `dominante.mjs` rend **440,0 Hz**
sur un ton connu et **−1000 dB** sur le silence.

### 🔴 Le banc est DIFFÉRENTIEL, et il a fallu une mesure pour le comprendre

Sa première rédaction jugeait le résidu **contre le plancher**, et concluait
« l'AEC ne fait rien » **sur une AEC qui retirait 65 dB**. La salle est du
silence **numérique** : son plancher est à **−151 dB**, et une annulation
parfaitement efficace y laisse encore un résidu **55 dB au-dessus**. Ce qui se
juge est l'**écart entre `aec:false` et `aec:true` sur le MÊME son**.

⚠️ **LA MÊME LEÇON, DEUX FOIS** : le bras casque portait le même seuil de
plancher et rendait « B toujours présente » alors qu'elle avait chuté de 114 dB.
Corrigé de la même façon. ***Ce qui juge est la CHUTE, jamais la hauteur.***

### Ce qui a débloqué le banc, et qui ne ressemblait à rien

Chrome et `pactl` tournent en **root**, et la bibliothèque PulseAudio **refuse**
de se connecter quand `XDG_RUNTIME_DIR` ne lui appartient pas. **Le symptôme
côté Chrome n'y ressemble en rien** : il retombe sur son dorsal **ALSA**,
`enumerateDevices()` rend des noms de cartes brutes, **`audioinput` est VIDE**,
et `getUserMedia` échoue en `NotFoundError: Requested device not found` — ce qui
se lit comme « la salle n'existe pas » alors qu'elle existe.
`PULSE_SERVER=unix:/run/user/1000/pulse/native` contourne l'heuristique.

🔵 **Et la première cause d'échec attendue par le plan NE SE PRODUIT PAS** :
**Chrome sans interface rend bien du son dans un nœud PipeWire** — 440,0 Hz à
−15,1 dB, mesuré par `pw-record`. **`Xvfb` était prêt et n'a pas eu à servir** ;
le banc sait s'en servir, par `--xvfb`.

### Le graphe audio de l'hôte est rendu intact

**Relevé AVANT, refait ce jour et non recopié du plan** : **un** seul
`Audio/Sink` (`auto_null`), **aucun** `Audio/Source`, aucun flux. *Rien de réel
à perturber.*

⚠️ **Le risque n°1 de la Décision 5 S'EST PRODUIT** : wireplumber **élit la
salle comme défaut tout seul**. Il est bénin ici, **et il se défait tout seul à
la mort du processus** — mesuré. Les défauts sont malgré tout posés
explicitement et **restaurés à leur valeur RELEVÉE** dans un `trap`, jamais à
une valeur supposée.

**Relevé APRÈS la campagne entière** : `Audio/Sink auto_null`, aucun
`Audio/Source`, défauts `auto_null` / `auto_null.monitor`, **zéro processus
`pw-loopback` survivant**. Le graphe est comparé **par ensemble de noms**,
jamais par nombre, et `diff` rend **AUCUN ÉCART** aux sept exécutions. Aucune
écriture dans `~mallanic`, aucune configuration persistante, aucun `systemctl`.

---

## 5. Les rouges — sept, dont la ROUGE 0 que le harnais refuse

**Doctrine appliquée sans exception** : copie **nommée** et jamais `HEAD` (un
contrôle fondé sur `git diff` contre `HEAD` ne distingue pas « la mutation n'a
rien changé » de « le fichier était déjà modifié ») ; restauration **depuis la
copie**, `git checkout --` restaurant `HEAD` et non l'état d'avant ; empreinte
vérifiée après ; comptage des occurrences **avant** toute substitution.

| # | Ce qu'elle mute | Ce qui tombe |
| --- | --- | --- |
| **R0** | **rien** (commentaire réécrit à l'identique) | **le HARNAIS**, qui REFUSE de jouer (exit 3, « mutation VIDE ») |
| R1 | `verifie_version` retiré de la **seule** variante `MicState` | **1 test sur 114** — la vérification est branchée variante par variante |
| R2a | la clé `type` → `micstate`, côté Rust | 2 tests sur 114 |
| R2b | la même, côté TypeScript | 4 tests sur 308 |
| R3 | l'émission déplacée vers **chaque dépôt** | 2 tests sur 1007 |
| R4 | l'annonce ne suit que la **première** transition | **1 test sur 1007** |
| R5 | `annoncerExclusivite` rendue inerte | **3** tests, tous dans la famille exclusivité |
| R6 | `occupation_ms` retiré de la trace | ⛔ **NON JOUÉE** — voir ci-dessous |

⚠️ **R2a fait tomber DEUX tests, et le second se DIAGNOSTIQUE plutôt qu'il ne se
classe** : le test de version analyse `{"type":"mic-state",…}`, qui ne résout
plus aucune variante, donc l'erreur change de texte. Les deux assertions
épinglent bien la clé de fil.

⚠️ **R2a mute par `#[serde(rename)]` et NON par un renommage de l'identifiant
Rust** : le renommer casserait `redaction.rs` et `controle.rs`, et la rouge
rougirait alors sur une **erreur de compilation** — c'est-à-dire pour la
mauvaise raison.

🔴 **R6 N'EST PAS JOUÉE, ET C'EST DÉCLARÉ.** Une trace `tracing` ne s'assère
pas, et `windows_micro.rs` est `#[cfg(windows)]` : aucun test d'hôte ne peut
l'exécuter, et « le champ est dans la source » n'est **pas** « le champ sort
dans le journal ». La seule rouge qui vaille est de retirer le champ, rebâtir,
relancer et constater que le `grep` retombe à zéro — soit un aller-retour
complet sur la VM pour une propriété que le contrôle vert établit déjà sur
166 lignes. **Le plan la signalait d'avance comme la plus faible ; ce document
ne prétend pas qu'elle vaut les six autres.**

🔴 **R2a ET R2b ÉTABLISSENT QUE CHAQUE CÔTÉ ÉPINGLE *SA* FORME. ELLES
N'ÉTABLISSENT PAS QUE LES DEUX S'ACCORDENT.** C'est la divergence **V1**,
relevée par E3 et **léguée** : il n'existe **aucun** fichier de vecteurs partagé
pour `AgentControl`. **Un renommage de clé appliqué d'un seul côté resterait
VERT DES DEUX CÔTÉS.** La lacune est **préexistante et générale** ; E3 est le
premier à la nommer.

---

## 6. Sept défauts d'instrument, tous trouvés par l'EXÉCUTION

**Aucun n'a été trouvé par relecture.**

1. 🔵 **`controle-trace-cable.sh` NE SE PARSAIT MÊME PAS.** Il portait
   `${1:?journal d'agent}` : bash **analyse** le mot d'une expansion
   `${par:?mot}` même entre guillemets doubles, et l'apostrophe y ouvrait une
   quote — « EOF prématurée », signalée **trente lignes plus bas**. **Il avait
   été versé « PRÊT, pas VERT » — et il ne l'était même pas.** *Un contrôle de
   recette s'exécute avant d'être prescrit.*
2. **Le pilote cliquait quatre secondes après l'ouverture des fenêtres**, et
   `#micro` était encore `hidden`. ⚠️ **`dataset.etat` à `null` PROUVE que
   `attacherBoutonMicro` n'avait pas couru** — il est écrit à l'attache. Ce
   n'était donc pas `ready.mic: false`, et les deux se lisent pourtant pareil
   sur un bouton caché. Remède : **attendre le FAIT**.
3. 🔴 **Les pages d'application restaient à « Connexion… » indéfiniment, sans
   une ligne de console.** `shell-page.ts` ouvre `/?session=<id>` **sans**
   `signaling`, et `adresseSignaling` retombe sur `ws://${location.host}` —
   c'est-à-dire, en recette, le serveur `vite` (5173) et non la plateforme
   (8080). ⚠️ **CE N'EST PAS UN DÉFAUT DU PRODUIT** : depuis le sous-bloc P5 de
   la plateforme, la page et l'API sont servies par la **même origine** derrière
   nginx, et le repli est alors exactement juste. C'est le montage à deux ports
   qui les sépare. L'injection ajoute donc le paramètre aux fenêtres que la
   shell ouvre — **compensation de montage, déclarée, jamais un correctif**.
4. **Le pilote cherchait `micro : cable acquis` pour identifier la gagnante.**
   **Cette ligne n'existe PAS pour le premier acquéreur** : `Issue::Accepte` ne
   journalise rien, seul `AccepteApresRefus` a sa trace. La gagnante se déduit
   de la **perdante**. ⚠️ Et le pilote a déclaré « phase C NON JOUÉE » au lieu
   d'inventer un PID : *un motif de recette se vérifie contre le CODE, et sur le
   VERT.*
5. **`evalBorne` rend un OBJET quand elle échoue**, jamais une chaîne, et un
   `JSON.parse` posé dessus reçoit « [object Object] ». La page de l'enfant tué
   n'existe plus en phase C — ce qui est pourtant **l'observation attendue** à
   cet instant. Piège prescrit par le plan, et rencontré quand même.
6. **La sonde de périphérique par défaut : `Add-Type` réussissait et le type
   restait introuvable** — PowerShell veut `+` pour un type imbriqué
   (`E3Def+IMMDeviceEnumerator`), puis le transtypage COM rendait `$null` en
   silence. Réécrite en WinRT.
7. **Un `>` dans le `/tr` d'une tâche planifiée n'est pas interprété** (il
   faudrait `cmd /c`), et l'absence de journal se lit alors comme une sonde qui
   n'a pas tourné.

---

## 7. La VM est morte une fois, et son compteur l'a dit

**Compteur libvirt 352 → 354** à la fin de l'essai 3 :

```
qemu-system-x86_64: terminating on signal 15 from pid 3139865 (<unknown process>)
shutting down, reason=shutdown
```

C'est **`libvirtd --timeout 120`** qui emporte le domaine. ⚠️ **CE N'EST PAS le
mécanisme d'hibernation de D1**, qui voyait un `shutdown.exe` **invité**
(Kernel-Power 187/42) : ici c'est **l'hôte** qui tue QEMU, et les confondre
ferait chercher du mauvais côté.

Le lanceur porte désormais le compteur **avant et après**, et redémarre la VM en
attendant **l'accès RÉEL** à `/media/vm` — jamais le seul port 5985, ni la seule
présence du montage CIFS, dont l'entrée persiste VM éteinte. **Le compteur vaut
354 avant ET après les trois exécutions retenues.**

⚠️ **Une exécution a été perdue** à cette cause (`essai-4`), et son journal est
versé.

---

## 8. Le binaire mesuré, et pourquoi sa taille ne prouve rien

Rebâti après **`cargo clean --release -p proto -p agent`** — E3 modifie `proto`,
et purger le seul crate qu'on compile ne suffit pas quand une dépendance interne
a franchi le même partage réseau.

🔴 **LA TAILLE NE PROUVE RIEN, ET DANS LES DEUX SENS : le binaire d'E3 est PLUS
PETIT que celui qu'il remplace** — **10 772 992** contre **10 790 400** octets —
alors qu'il ajoute du code. Le contrôle qui vaut est une **chaîne posée
soi-même**, cherchée **sur le chemin que `run-agent.sh` lance** :

| Contrôle | AVANT | APRÈS |
| --- | --- | --- |
| témoin **positif** — `micro ecrit sur le cable` | 1 | 1 |
| témoin **négatif** — une chaîne impossible | 0 | 0 |
| discriminant — `mic-state` | **0** | **3** |
| discriminant — le `warn!` neuf de `windows_micro` | **0** | **1** |

⚠️ **`occupation_ms` a été ÉCARTÉ comme discriminant : il rend déjà 1 sur le
binaire d'AVANT** — c'est un champ de la trace du **puits de mesure** de E1.
*« Vérifie le chemin sur lequel tu la cherches » vaut aussi pour la chaîne
elle-même.*

Et la purge avait de toute façon **supprimé** le binaire : `ls` rendait « Aucun
fichier ou dossier de ce nom ».

**Les WAV** du périphérique factice sont régénérés par le générateur
déterministe de E2 et vérifiés **bit pour bit** contre `wav-empreintes.txt`
(4/4 « Réussi »). Ils ne sont pas versés, et n'ont pas à l'être.

---

## 9. La revue transverse de fin de branche

Barème du dépôt : 5 en D7, 3 en D8, 6 en D9, douze en D10, sept en D11, huit en
P1, dix en P2, cinq en S1, neuf en E1, douze en P3, douze en S2, treize en S3,
huit en P4, huit en G1, vingt-sept en S4, dix-sept au presse-papier P1, huit en
E2. **Neuf en E3 avant la recette VM, sur dix places** — plus **six** que la
recette a rendues fausses ensuite, dont **une de E3 lui-même**.

Les places ont été **énumérées par `grep -n` avant d'écrire** et **relues une
par une après**. Le détail vit dans les messages de commit ; les trois qui
enseignent :

- 🔴 **« Le refus est une condition PERMANENTE — une autre fenêtre tient le
  câble pour la vie de son processus »** (`piste_micro.rs`, **deux** places).
  **Faux depuis la Décision 2 de E2**, qui a rendu la tentative d'acquisition
  **non collante**, et **jamais repris** : `git log` ne rend qu'un seul commit
  sur ce fichier avant E3, et la revue transverse de E2 ne le liste pas. *C'est
  le patron exact que la revue transverse existe pour attraper, et il lui avait
  échappé.*
- ⚠️ **Sa seconde moitié — « le refus n'est PAS dit au client » — est devenue
  fausse par E3 lui-même.** **Les deux sont corrigées, pas l'une des deux.**
- ⚠️ **« ce fichier est à TROIS lignes de son plafond »** (`transport.rs`) :
  **jamais trouvé vrai**. Au commit qui l'a écrit, le fichier faisait **491**
  lignes, marge **9**. *Rien n'établit qu'il ait été faux à l'INSTANT de
  l'écriture ; il l'était au commit, seul état vérifiable.* **Et le nombre n'est
  pas remplacé par un autre nombre** — un compte de lignes recopié dans un
  commentaire vieillit à la première insertion.

🔴 **LA REVUE TRANSVERSE A FRANCHI DEUX PLAFONDS ELLE-MÊME**, et c'est le piège
que ce dépôt paie depuis S2 : *documenter une extraction reprend une part de la
marge qu'elle rend*, et **la ronde qui dénonce la dérive la produit**.
`proto/src/control.rs` 475 → **496 (marge 4)**, rattrapé par l'**extraction** de
`LinkQuality`/`LinkAdaptation` vers `control/lien.rs` ; `agent/src/transport.rs`
488 → **495 (marge 5)**, rattrapé par un **PLACEMENT** — la réfutation complète
va dans `piste_micro.rs` (marge 269), « auprès du code qui les emploie », que le
commentaire d'origine revendiquait déjà pour lui-même. ⚠️ *Déplacer un texte
vers le module qui possède le raisonnement n'est pas le raccourcir ; raccourcir
une réfutation pour atteindre un compte de lignes est ce que ce dépôt interdit
nommément.*

🔴 **UN MESSAGE DE COMMIT A PORTÉ DEUX NOMBRES FAUX, ÉCRITS DE MÉMOIRE — et
l'un d'eux était littéralement « 487 », le nombre dont ce dépôt a fait le nom de
sa propre erreur.** Attrapés par `wc -l` avant que le commit ne soit figé.
*Un message de commit est une pièce du dépôt, et personne ne le relit.*

---

## 10. Ce que E3 n'établit PAS

- **Aucun taux, nulle part.** Deux exécutions par critère au mieux, une pour
  l'injection de faute et une pour la sonde de périphérique par défaut.
- 🔴 **L'ÉCHO EST MESURÉ SUR UN BANC NUMÉRIQUE, ET CE BANC NE LÈVE RIEN.** La
  Décision 7 de E1 est **réfutée**, mais une salle émulée n'a **ni réponse de
  salle, ni retard de propagation, ni distorsion non linéaire de
  haut-parleur** — et c'est la non-linéarité qui met une AEC en défaut. **Issue
  B ⇒ RIEN n'est levé** (Décision 3).
- 🔴 **Le banc mesure CHROME 151, pas LE PRODUIT** (Décision 4). Que le produit
  n'en souffre pas SUIT du fait que ses N fenêtres vivent dans une seule
  instance — **mais cela reste une inférence**, et aucune session réelle ne l'a
  montré.
- ⛔ **Le son d'une AUTRE application n'est pas annulé**, et c'est mesuré
  (−13 dB, c'est-à-dire amplifié). **Rien ne le corrige, et rien ne le dit à
  l'utilisateur.**
- 🔴 **Personne n'a écouté.** Le critère de fin de la spec §13 reste atteint par
  un **juge logiciel** seulement, et le protocole humain reste dû.
- 🔴 **La latence de bout en bout**, que rien ne mesure dans ce dépôt depuis D1.
  **Le troisième critère de la spec §13 reste NON JUGÉ.**
- **Une seule tonalité** : le juge dit que quelqu'un est entendu, jamais lequel.
- **Rien au-delà de DEUX fenêtres.**
- **Aucun microphone réel** : le périphérique factice de Chrome alimenté par un
  fichier exerce `getUserMedia`, la permission et les trois contraintes — il
  n'exerce ni un vrai micro, ni le DTX d'un vrai locuteur, ni l'AEC en
  conditions acoustiques.
- **Aucune durée longue** : la session la plus longue de cette recette est de
  l'ordre de deux minutes. E2 avait ses dix minutes ; E3 ne les rejoue pas.
- **Rien d'un autre navigateur, d'une autre version, d'un autre serveur audio.**
- **`granted: true` n'est PAS un accusé de réception**, et c'est mesuré (§4).
- **Aucune constante calibrée**, et **aucun jugement d'écoute n'a jamais été
  porté sur aucune constante de ce dépôt**. E3 en ajoute un **douzième** à la
  liste des jugements humains : **la formulation du bandeau d'exclusivité**.
- **La divergence V1** : `AgentControl` n'a aucun vecteur partagé, et un
  renommage de clé appliqué d'un seul côté resterait vert des deux côtés.
- **Le rééchantillonnage 48 000 → 44 100 du câble** n'est mesuré par personne.
- **La décroissance de 17,6 % de l'amplitude** relevée par E2 sur sept minutes
  reste inexpliquée.
- **Les deux replis `Local\` et `Reveil::Echeance`** restent du code jamais
  couru.
- **La licence VB-Audio** est personnelle seulement — **juridique**, avant mise
  sur le marché.
- **Les trois couches inconnues du chantier D le restent.**

---

## 11. Ce que E3 lègue

**Legs de E2 réglés** : n°2 (le refus dit au client — **fermé et exercé**), n°3
(l'exclusivité à deux enfants — **exercée**), n°7 **à moitié** (l'occupation est
observable ; la latence de bout en bout tient entièrement), n°9
(`MICRO_FAUTE_ECRITURE` — **armée, et elle mord**).
**Legs de E1 réglé** : n°7 (le périphérique par défaut de la session
interactive).

**Ce qui reste dû :**

1. ✅ **L'ÉCHO ENTRE DEUX FENÊTRES DU PRODUIT : MESURÉ, et le défaut que E1
   redoutait N'EXISTE PAS** — les N fenêtres vivent dans une seule instance de
   navigateur, où l'AEC de Chrome couvre tout. ⛔ **Ce qui reste dû est un
   défaut VOISIN et RÉEL, que personne n'avait nommé** : le son d'une **autre
   application** n'est pas annulé (**−13 dB**, c'est-à-dire amplifié). Rien ne
   le corrige, et **rien ne le dit à l'utilisateur** — c'est exactement ce que
   la **voie C** (le casque, dit au bon moment) livrerait par le mécanisme que
   E3 a déjà construit.
2. ⛔ **Le protocole humain de l'écho**, dû **quel que soit** le verdict de S1 —
   le banc de salle émulée peut confirmer, **jamais lever** (Décision 3). C'est
   aussi le seul protocole qui puisse fermer **deux autres critères que ce dépôt
   n'a jamais atteints** : « une application Windows entend » à l'oreille (spec
   §13) et « un correspondant en appel réel ne perçoit pas d'écho » (§11.2).
3. ⛔ **Le choix entre les voies A, B et C**, posé au propriétaire du dépôt —
   ⚠️ **et la mesure en a changé les termes.** La **voie A** (rassembler la
   restitution dans l'onglet qui capte, ce qui **défait D7**) **n'a plus rien à
   corriger entre fenêtres du produit** : elles sont déjà toutes couvertes. La
   **voie B** (une AEC côté agent, que la spec §14 exclut nommément) reste
   coûteuse. **La voie C — le casque, DIT AU BON MOMENT — est la seule dont le
   défaut mesuré ait encore besoin**, et c'est la seule livrable par le
   mécanisme que E3 a déjà construit : `MicState` porterait un second cas.
   ⚠️ **Non prescrite** : un avis qui s'affiche à tort est pire qu'un avis
   absent, et rien ne dit encore qu'une autre application joue du son.
4. ⛔ **La latence de bout en bout.**
5. ⛔ **L'écoute à l'oreille** (spec §13) et **l'appel réel sans écho** (§11.2).
6. ⛔ **La divergence V1** : ouvrir un fichier de vecteurs partagé pour
   `AgentControl`. Le remède a son précédent — `proto/plateforme-vectors.json`,
   créé par P3 pour cette raison exacte — et **il est général à `AgentControl`,
   pas propre au micro**.
7. ⛔ **R6**, la seule rouge non jouée.
8. ⛔ **Les deux replis jamais courus**, et **`Local\` en particulier** : sa
   garantie rétrécit à une session Windows, et personne n'a vu ce qu'elle vaut.
9. ⛔ **La qualité du rééchantillonnage du câble**, et la décroissance de 17,6 %
   de E2.
10. ⛔ **La licence VB-Audio**, personnelle seulement.
11. ⚠️ **`proto/src/control.rs` est à 487, marge 13.** Toute addition future y
    appelle une **extraction**, et son point de chute est nommé : la variante
    `Link` elle-même vers `control/lien.rs`, où vivent déjà ses deux enums.
12. ⚠️ **`agent/src/transport.rs` (488, marge 12)** et **`client/src/main.ts`
    (486, marge 14)**. Pour `main.ts`, le point de chute est nommé par le plan —
    `client/src/bandeaux.ts` — et **jamais la chaîne de dispatch**, qui capture
    une quinzaine de `let` de module : une extraction verbatim ne compilerait
    pas.
