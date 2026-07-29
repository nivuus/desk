# Chantier B — Input jeu

**Date** : 28 juillet 2026
**Statut** : Implémenté, recette conduite le 29 juillet 2026 — voir
`docs/superpowers/plans/2026-07-28-input-jeu-resultats.md` (sur les 5 mesures
instrumentées : 2 atteintes sans réserve, 2 atteintes avec une réserve
méthodologique déclarée, 1 partielle ; Steam/TF2/Dota 2 bloqués par une
authentification à deux facteurs hors de portée d'un agent)
**Portée** : Rendre les entrées utilisables par un jeu — souris relative, manette
avec vibration, clavier complet en plein écran — et faire apparaître le curseur,
aujourd'hui absent du flux.

> **Origine** : chantier B de `2026-07-28-support-jeux-design.md` §5. Les
> chantiers 0 (débit et latence) et A (audio) sont terminés ; leurs cibles sont
> atteintes. Ce chantier lève les écarts n°2 et n°3 du tableau §1 du cadrage.

---

## 1. Objectif

Le pipeline streame aujourd'hui une fenêtre Windows avec une souris **absolue**
(`agent/src/input.rs:104`, `MOUSEEVENTF_ABSOLUTE`), un clavier partiel, aucune
manette et aucun curseur. Tout jeu à caméra libre lit la souris en relatif par
Raw Input : il est donc injouable. Tout jeu à la manette l'est aussi.

À la fin de ce chantier, un jeu est jouable et mesurable en conditions réelles :

- la souris bascule automatiquement en relatif quand un jeu le demande, avec une
  visée linéaire 1:1 ;
- une manette Xbox 360 virtuelle est vue par les jeux, vibration comprise ;
- Échap et les raccourcis navigateur atteignent le jeu en plein écran ;
- le curseur est visible hors jeu — ce qu'aucune session ne permet aujourd'hui.

### Prérequis levés

| Prérequis | État |
|---|---|
| Pipeline à ses cibles (60 i/s, < 50 ms LAN) | **Acquis** — 62,6 i/s, médiane 47,9 ms (`plans/2026-07-28-debit-latence.md`) |
| Audio | **Acquis** — chantier A mergé (`0e0679f`) |
| Session interactive sur la VM | **Acquis** — contrainte du jalon 1 |

---

## 2. Décisions actées

| Décision | Choix | Justification |
|---|---|---|
| Périmètre | **Souris relative + manette + clavier/plein écran + curseur** | Seul périmètre qui rend un jeu réellement jouable à la fin du chantier |
| Bascule absolu ⇄ relatif | **L'agent décide**, le client obéit | L'utilisateur ne configure rien ; une seule source de vérité sur un canal non ordonné |
| Signal de bascule | **Curseur système masqué** | Un jeu à caméra libre masque le curseur parce qu'il lit la souris en brut |
| Curseur | **Forme seule, position locale** | Latence nulle par construction, aucun overlay à composer |
| Accélération pointeur | **L'agent neutralise au démarrage, sans persister** | Impossible à oublier lors d'un réapprovisionnement de la VM |
| Manette | **Une Xbox 360 (XInput), avec vibration** | XInput est le mapping le plus universellement reconnu ; la vibration porte le ressenti AAA |
| Zones mortes | **Aucune**, ni client ni agent | Les jeux appliquent la leur ; en ajouter une la creuserait deux fois |
| Déclencheur du plein écran | **Demande de l'utilisateur** | Le hook Windows du §4.1 du cadrage appartient au chantier D ; ce sens-ci ne le contredit pas et ne sera pas défait |
| Recette | **Sondes instrumentées, puis jeu réel** | Les sondes prouvent, le jeu confirme |

### Ce que ce chantier ne décide pas

Le §4.1 du cadrage fait de Windows le maître du plein écran, dans un sens unique.
Ce chantier n'implémente que le plein écran **demandé par l'utilisateur**, qui ne
touche pas à l'état de la fenêtre Windows. Le chantier D ajoutera le sens
Windows → navigateur par-dessus, sans rien défaire ici.

---

## 3. Découpage en modules

### Agent

| Module | Rôle | Testable sous Linux |
|---|---|---|
| `agent/src/cursor.rs` | Sondage `GetCursorInfo`, hystérésis de bascule, classification de forme, émission de `Pointer` | La machine à états d'hystérésis, oui — extraite du code Windows |
| `agent/src/gamepad.rs` | Cible ViGEmBus, application des états, rappel de vibration limité en débit | La comparaison de séquence et la limitation de débit, oui |
| `agent/src/pointer_settings.rs` | `SPI_SETMOUSE` / `SPI_SETMOUSESPEED` au démarrage, relecture journalisée | Non — appel système pur |
| `agent/src/input.rs` (étendu) | Injection relative, saut du repositionnement en mode relatif | Non — déjà `#[cfg(windows)]` |

`cursor.rs` et `gamepad.rs` ne connaissent pas WebRTC : ils produisent des
messages. C'est ce qui rend leur logique testable sans Windows ni réseau.

### Client

| Module | Rôle |
|---|---|
| `client/src/pointer.ts` | Pointer Lock, armement sur clic, sommation des deltas, application de la forme CSS |
| `client/src/gamepad.ts` | Sondage à 250 Hz, détection de changement, rafraîchissement, vibration |
| `client/src/fullscreen.ts` | Bouton plein écran, Keyboard Lock, repli silencieux |
| `client/src/scancodes.ts` | Table `KeyboardEvent.code` → scancode, **extraite** d'`input.ts` et complétée |

`input.ts` fait 215 lignes dont 90 de table. L'extraction n'est pas cosmétique :
elle isole la seule partie du fichier qui se relit et se complète.

---

## 4. Protocole

### Canal binaire — protocole d'entrée **v2**

Deux types s'ajoutent aux quatre existants. Le commentaire de
`proto/src/input.rs` impose d'incrémenter `PROTOCOL_VERSION` à tout changement
de format ; ajouter un type en est un. Agent et client étant déployés ensemble,
le rejet mutuel des versions est le comportement souhaitable.

```
TYPE_MOUSE_MOVE_RELATIVE = 5 : dx: i16 | dy: i16                       (6 octets)
TYPE_GAMEPAD_STATE       = 6 : seq: u16 | buttons: u16 | left_trigger: u8
                             | right_trigger: u8 | lx: i16 | ly: i16
                             | rx: i16 | ry: i16                      (16 octets)
```

`buttons` reprend le masque `XINPUT_GAMEPAD.wButtons`, les axes ses `i16`, les
gâchettes ses `u8` : aucune conversion côté agent, donc aucune occasion de se
tromper de convention.

### Canal de contrôle — **v2**

Trois messages agent → client s'ajoutent :

```json
{"type":"pointer","v":2,"visible":false,"shape":"default"}
{"type":"rumble","v":2,"left":128,"right":64}
{"type":"capabilities","v":2,"gamepad":true}
```

`capabilities` est émis **une seule fois par session**, mais PAS après `ready`
contrairement à ce qu'une lecture naturelle suggérerait : `agent/src/main.rs`
le pousse dans le `mpsc` de contrôle dès le démarrage du transport, avant
l'ouverture du canal de données, alors que `ready` n'est ajouté qu'à
`Event::ChannelOpen`. L'ordre réellement observé est `capabilities`,
éventuellement un premier `pointer`, puis `ready`. Sans conséquence ici — le
client (`client/src/main.ts`) traite les types de message indépendamment —
mais **un client ne doit pas gater son initialisation sur `ready`** : il
perdrait `capabilities` et le premier `pointer`, émis avant.

`shape` prend l'une des valeurs CSS suivantes, et rien d'autre : `default`,
`text`, `wait`, `progress`, `crosshair`, `pointer`, `move`, `not-allowed`,
`help`, `ns-resize`, `ew-resize`, `nwse-resize`, `nesw-resize`.

**Un seul message porte la visibilité et la forme** parce que c'est une seule
observation : `visible: false` signifie à la fois « verrouille le pointeur » et
« n'affiche aucun curseur ».

### Chemin agent → client

`Session::run()` possède la boucle et n'émet aujourd'hui des `AgentControl` que
depuis l'intérieur (`ready`, `begin_ending`). Or le sondage du curseur tourne sur
son propre fil et le rappel de vibration de ViGEmBus s'exécute sur un fil du
pilote : ni l'un ni l'autre ne peut toucher la `Session`.

`run()` reçoit donc un `mpsc::Receiver<AgentControl>`, drainé dans
`act_on_timeout` **à raison d'une mutation par tour**, dans la discipline
documentée à `transport.rs:569-579` — celle qui garantit qu'aucun chemin de code
ne mute `Rtc` sans que `run()` ne rappelle `poll_output` juste après. Un seul
mécanisme sert les deux producteurs.

---

## 5. Souris relative

### Détection

Un fil sonde `GetCursorInfo` toutes les 50 ms. L'absence du drapeau
`CURSOR_SHOWING` est le signal de mode relatif.

Un changement n'est retenu qu'après **trois sondages consécutifs cohérents**.
Sans cette hystérésis, les clignotements du curseur pendant les transitions
d'écran verrouilleraient et déverrouilleraient le pointeur en boucle. Latence de
bascule : ~150 ms, imperceptible puisqu'elle accompagne un changement de scène.

**Limite assumée.** Un jeu qui masque le curseur tout en lisant sa position
absolue — certains RTS — basculera en relatif à tort. Il reste jouable :
`SendInput` relatif déplace le curseur système, donc `GetCursorPos` suit. Mais le
curseur butera sur les bords de l'écran sans retour visuel. Si la recette révèle
ce cas, le second critère à ajouter est `GetClipCursor` (curseur confiné). On ne
l'ajoute pas d'emblée : il produit ses propres faux positifs, et rien ne dit
encore lesquels dominent. La mesure n°5 et Dota 2 sont là pour trancher.

### Verrouillage

`requestPointerLock()` exige une activation utilisateur transitoire ; un message
reçu sur data channel n'en est pas une. Le client **s'arme donc sur le prochain
clic** — le mécanisme que le §4.1 du cadrage retient déjà pour le plein écran et
que le spike multi-fenêtres a validé. Troisième usage d'un mécanisme unique, ce
qui est un argument de conception à part entière : il n'y a qu'un comportement à
expliquer à l'utilisateur.

Le déverrouillage ne demande rien : `exitPointerLock()` à réception de
`Pointer { visible: true }`.

### Transport des deltas

Un message par événement `pointermove`, portant la **somme** des `movementX/Y`
des événements coalescés (`getCoalescedEvents`). La somme détermine la visée ; la
granularité intra-trame n'apporte rien à un jeu qui intègre les deltas, et
coûterait un paquet par échantillon à 1000 Hz.

Débordement de `i16` clampé, **reste reporté sur le message suivant** : la somme
transmise reste exacte, aucun mouvement ne se perd.

### Le piège des clics

`InputMessage::MouseButton` porte des coordonnées absolues, et l'agent
repositionne le curseur avant de cliquer (`input.rs:48-49`, « le canal n'est pas
ordonné, le déplacement correspondant a pu se perdre »). En mode relatif, ce
repositionnement **téléporterait le curseur à chaque tir**.

Résolution : l'`InputInjector` porte l'état de mode que le sondage lui donne, et
saute le repositionnement quand il est en relatif. Le format des messages ne
change pas, le client n'a rien à savoir, et il n'existe **qu'une source de
vérité** — ce qui est la seule construction correcte sur un canal non ordonné.

### Neutralisation de l'accélération

Au démarrage de l'agent :

- `SystemParametersInfo(SPI_SETMOUSE, …)` avec les trois seuils à zéro
  (accélération désactivée) ;
- `SystemParametersInfo(SPI_SETMOUSESPEED, …)` à 10, le cran 1:1 ;
- **sans** `SPIF_UPDATEINIFILE` : le réglage vaut pour la session courante et
  disparaît à la déconnexion ;
- relecture immédiate (`SPI_GETMOUSE`, `SPI_GETMOUSESPEED`) et journalisation des
  valeurs obtenues.

Sans cette neutralisation, la visée est non linéaire — le « piège majeur » du
cadrage. Aucun effet de bord sur la souris absolue, qui ignore déjà ces réglages.

---

## 6. Manette

### Agent

Crate `vigem-client` (Rust pur, dialogue avec le pilote par IOCTL — aucun SDK C à
embarquer). **ViGEmBus est un pilote noyau à installer sur la VM : c'est une
inconnue à sonder, pas un acquis**, au même titre que la forme exacte de son API
de notification de vibration (§11, sondes 1 et 2).

La cible Xbox 360 est branchée à la **première réception** d'un `GamepadState` et
débranchée en fin de session. Brancher une manette en permanence perturberait les
applications qui réagissent à sa seule présence ; la débrancher en cours de
session ne rendrait service à personne.

### Client

Sondage à **250 Hz** (`setInterval` à 4 ms, le plancher navigateur).
`requestAnimationFrame` est exclu : 60 Hz, et suspendu en arrière-plan.

Émission **sur changement d'état, plus un rafraîchissement toutes les 100 ms**.
Sur un canal non fiable, un état complet et idempotent se répare de lui-même au
message suivant, là où des événements différentiels laisseraient une touche
coincée jusqu'à la fin de la partie.

Le champ `seq` fait rejeter les états réordonnés. La comparaison se fait en
**différence signée** (`(a - b) as i16 > 0`) pour traverser correctement le
bouclage de l'`u16`.

Conversions : axes des flottants `-1..1` de la Gamepad API vers les `i16`
d'XInput ; gâchettes de `buttons[6]`/`buttons[7]` (`value` en `0..1`) vers `u8`.
**Aucune zone morte n'est appliquée.**

La Gamepad API n'expose aucune manette avant un appui sur celle-ci. L'UI le dit
explicitement — « appuyez sur un bouton de votre manette » — plutôt que de
laisser conclure à une panne. Le mapping `standard` n'est pas garanti selon les
modèles : on suppose le mapping standard et on documente la limite.

### Vibration

Le rappel de notification de `vigem-client` s'exécute sur un fil du pilote et
pousse un `AgentControl::Rumble` dans le `mpsc` du §4. Émission **sur changement
et au plus une toutes les 20 ms** : le canal de contrôle est fiable, l'inonder
lui ferait accumuler du retard exactement quand le jeu en produit le plus.

Côté client, `gamepad.vibrationActuator.playEffect('dual-rumble', …)` avec une
durée de **200 ms remplacée à chaque message** : si l'agent se tait, la vibration
s'éteint d'elle-même plutôt que de rester bloquée. Absence de
`vibrationActuator` : ignoré silencieusement.

---

## 7. Clavier et plein écran

Un bouton met la page en plein écran. L'événement `fullscreenchange` appelle
alors `navigator.keyboard.lock()` **sans argument** : c'est le mode jeu, toutes
les touches vont au jeu, et l'utilisateur sort par appui long sur Échap —
comportement prévu par l'API.

C'est la condition de viabilité posée au §4.1 du cadrage : Échap est à la fois la
touche de sortie du plein écran navigateur et la touche de menu pause de presque
tous les jeux.

**Repli** : sur Firefox et Safari, `navigator.keyboard` est absent. On n'appelle
rien, Échap casse le plein écran. Limite documentée et non corrigée, conformément
au §6.1 du cadrage.

### Table de scancodes

`client/src/scancodes.ts` reprend la table existante et la complète du **pavé
numérique, aujourd'hui entièrement absent** :

| Touche | Scancode | Étendu |
|---|---|---|
| `Numpad0`…`Numpad9` | 0x52, 0x4F-0x51, 0x4B-0x4D, 0x47-0x49 | non |
| `NumpadMultiply` | 0x37 | non |
| `NumpadSubtract` | 0x4A | non |
| `NumpadAdd` | 0x4E | non |
| `NumpadDecimal` | 0x53 | non |
| `NumLock` | 0x45 | non |
| `ScrollLock` | 0x46 | non |
| `PrintScreen` | 0x37 | oui |

`NumpadDecimal` (0x53 non étendu) et `Delete` (0x53 étendu) partagent leur
scancode : c'est exactement la distinction que le drapeau `extended` porte, et la
table existante la respecte déjà pour les touches de navigation.

**`Pause` reste absente, délibérément.** Elle n'émet pas un scancode simple mais
la séquence `E1 1D 45`, préfixée `E1` et non `E0` — que le drapeau booléen
`extended` du protocole ne sait pas représenter. La coder exigerait un troisième
état de préfixe pour une touche qu'aucun jeu n'utilise. Limite documentée.

Ce manque pénalise déjà la bureautique, pas seulement le jeu.

---

## 8. Curseur

Le sondage du §5 lit aussi `CURSORINFO.hCursor` et le compare aux curseurs
système obtenus par `LoadCursorW(None, IDC_*)` : `IDC_ARROW`, `IDC_IBEAM`,
`IDC_WAIT`, `IDC_APPSTARTING`, `IDC_CROSS`, `IDC_HAND`, `IDC_NO`, `IDC_HELP`,
`IDC_SIZENS`, `IDC_SIZEWE`, `IDC_SIZENWSE`, `IDC_SIZENESW`, `IDC_SIZEALL`. La
forme obtenue part dans `Pointer.shape` et le client la pose en propriété CSS
`cursor` sur l'élément vidéo.

Le navigateur dessine alors le curseur à la position réelle de la souris de
l'utilisateur : **latence nulle par construction**, aucun overlay, aucun bitmap à
décoder.

**Limite assumée** : un curseur applicatif custom ne correspond à aucun curseur
système et retombe sur `default`. C'est le prix du choix « forme seule ». La
seconde limite est la divergence de position quand l'application déplace le
curseur elle-même (`SetCursorPos`) — rare hors jeu, et sans objet en mode relatif
où le curseur est masqué.

Aujourd'hui, **aucun curseur n'est transmis** : Desktop Duplication le fournit
séparément (`PointerPosition`, `GetFramePointerShape`) et l'agent ne le lit nulle
part. Ce chantier comble donc un manque qui précède le jeu.

---

## 9. Erreurs et dégradation

**Rien de ce chantier ne doit pouvoir empêcher une session de démarrer.**

| Défaillance | Comportement |
|---|---|
| ViGEmBus absent ou non chargé | Avertissement au démarrage, `GamepadState` ignorés, `Capabilities { gamepad: false }` envoyé au client, qui l'affiche |
| `SPI_SETMOUSE` en échec | Avertissement, session poursuivie avec une visée dégradée |
| `GetCursorInfo` en échec | Avertissement unique, on reste en absolu — **jamais de bascule à l'aveugle** |
| `requestPointerLock` refusé | Réarmement sur le clic suivant ; bandeau via `status.ts` après deux échecs |
| Pointer Lock perdu (Échap) alors que l'agent est en relatif | Réarmement sur le clic suivant |
| `navigator.keyboard` absent | Aucun appel, plein écran sans Keyboard Lock |
| `vibrationActuator` absent | Vibrations ignorées silencieusement |
| **Manette débranchée côté client** | Envoi immédiat d'un **état neutre** — sans quoi une touche reste coincée dans le jeu |

Les deux dernières lignes du tableau traitent les seuls cas qui laisseraient
autrement une entrée bloquée côté Windows. Ils sont explicites parce qu'un
blocage d'entrée est le défaut le plus pénible et le moins diagnosticable du lot.

---

## 10. Tests

### Sous Linux, sans Windows

Trois logiques pures sont **extraites du code Windows précisément pour être
testables** :

1. **Hystérésis de bascule** (`cursor.rs`) — trois échantillons cohérents avant
   changement : séquences oscillantes, séquences stables, transitions.
2. **Comparaison de séquence** (`gamepad.rs`) — différence signée à travers le
   bouclage `u16` : `65535` puis `0` doit être accepté, `10` puis `9` rejeté.
3. **Limitation de débit de la vibration** — au plus un message par 20 ms, et le
   dernier état émis doit toujours être le vrai.

Le protocole se teste des deux côtés par les vecteurs partagés
`proto/vectors.json`, étendus aux deux nouveaux types, plus le rejet explicite
d'un message en version 1.

### Côté client (Vitest)

`audio.test.ts`, `webrtc.test.ts` et `status.test.ts` ont installé le patron.
S'ajoutent :

- `pointer.test.ts` — sommation des événements coalescés, clamp `i16` et report
  du reste ; armement et réarmement sur clic.
- `gamepad.test.ts` — conversion axes et gâchettes, détection de changement,
  rafraîchissement périodique, état neutre au débranchement.

### Recette mesurée sur la VM

Sur le modèle des sondes du chantier A (`AUDIO_PROBE`), pilotées par
`scripts/run-agent.sh` en session interactive.

| # | Mesure | Critère |
|---|---|---|
| 1 | **Linéarité de la visée** — sonde agent `INPUT_LINEARITY_PROBE` : injecter une somme connue de deltas, relire `GetCursorPos` avant/après | écart ≤ 1 px sur 1000, aux deux axes, **y compris à grands deltas** — c'est là que l'accélération se voit |
| 2 | **Manette** — sonde agent `GAMEPAD_PROBE` : brancher la cible, injecter un état, le relire par `XInputGetState` | chaque bouton et chaque axe conformes à ±1 LSB, pilote compris |
| 3 | **Vibration** — `XInputSetState` à magnitudes connues | le client reçoit le `Rumble` correspondant |
| 4 | **Échap en plein écran** | scancode 0x01 reçu par l'agent, navigateur toujours en plein écran |
| 5 | **Bascule de mode** | `Pointer { visible: false }` sous 250 ms après masquage, **zéro oscillation sur 60 s** |

La mesure n°1 se fait côté agent et non par le navigateur, délibérément : CDP ne
sait pas synthétiser un `movementX` digne de foi, et le trajet navigateur → delta
est déjà couvert par `pointer.test.ts`. La sonde mesure ce qui n'est mesurable
que là : `SendInput` relatif et la neutralisation SPI.

### Jeu réel

**Team Fortress 2** et **Dota 2** : gratuits tous les deux, même écosystème à
installer, et ils couvrent les deux modes opposés — relatif à caméra libre,
absolu à curseur visible.

Dota 2 vérifie ce qu'aucune sonde ne vérifie : que le mode relatif **ne se
déclenche pas** à tort.

Critères qualitatifs : viser une cible sans dérive perceptible ; tourner sur 360°
sans blocage ; ouvrir le menu pause par Échap sans sortir du plein écran ; manette
reconnue dans les options du jeu ; vibration ressentie.

---

## 11. Risques et sondes

À lever **avant** de planifier, pas à supposer :

| # | Inconnue | Sonde |
|---|---|---|
| 1 | ViGEmBus s'installe-t-il sur cette VM (Windows Server, build 20348) ? | Installation, puis `Get-PnpDevice` sur la classe système et création d'une cible par `vigem-client` |
| 2 | Le rappel de vibration remonte-t-il réellement ? | `XInputSetState` depuis un petit programme sur la VM, vérification du rappel côté agent |
| 3 | Le signal « curseur masqué » suffit-il ? | Mesure n°5 sur TF2 (doit basculer) et Dota 2 (ne doit pas) |
| 4 | `setInterval` à 4 ms tient-il 250 Hz sous charge ? | Comptage des tours sur 10 s pendant une session vidéo active |
| 5 | Les événements de `getCoalescedEvents()` portent-ils un `movementX/Y` exploitable sous Pointer Lock ? | Comparer la somme des deltas coalescés au `movementX` de l'événement principal sur 5 s de mouvement continu |

La sonde n°5 n'est pas théorique : `movementX` a longtemps valu zéro sur les
événements coalescés de Chromium. Si elle échoue, le repli est immédiat et sans
conséquence sur le reste du design — on prend le `movementX` de l'événement
principal, en perdant seulement la restitution des positions intermédiaires.

**La VM Windows n'est pas démarrée automatiquement** (`CLAUDE.md`, cycle de vie
de la VM). Toute sonde exige `virsh start Windows` puis l'attente de WinRM.

Risque résiduel connu et accepté : la neutralisation logicielle de l'accélération
peut se révéler insuffisante pour un FPS compétitif. Le cadrage a déjà tranché —
le pilote d'injection en mode noyau à la Parsec est hors périmètre v1 (§9 du
cadrage). La mesure n°1 dira si l'écart est mesurable.

---

## 12. Critère de fin

Le chantier est terminé quand :

1. les cinq mesures du §10 passent leurs critères ;
2. TF2 se joue à la souris et à la manette, vibration comprise, Échap ouvrant le
   menu pause sans quitter le plein écran ;
3. Dota 2 se joue au curseur visible, **sans bascule en relatif** ;
4. les tests Linux et Vitest passent ;
5. une session bureautique ordinaire affiche un curseur qui change de forme.

Le point 5 n'est pas un bonus : c'est la vérification que ce chantier n'a pas
optimisé le jeu au détriment de l'usage courant.

---

## 13. Dette connue

La revue finale a identifié ces points comme réels, sans les faire bloquer la
fusion : ils sont consignés ici plutôt que corrigés dans l'urgence.

1. **La garde de Pointer Lock côté client n'est couverte par aucun test**
   (`client/src/input.ts:61`, `if (document.pointerLockElement === video)
   return;`). C'est pourtant la moitié du dispositif décrit au §5 « Le piège
   des clics » — l'autre moitié (`agent/src/input.rs`) étant, elle, hors de
   portée de Vitest de toute façon. `input.ts` est le seul module client à
   ne pas avoir reçu l'injection de dépendances appliquée à `pointer.ts`,
   `fullscreen.ts` et `gamepad.ts` : il lit `document` et `window`
   globalement, ce qui le rend intestable sans DOM réel. Deux lignes
   ajoutées à `InputOptions` (un `doc` injecté à la place de `document`)
   suffiraient à aligner ce module sur le patron des trois autres.
2. **La décision symétrique côté agent** (`agent/src/input.rs:65`, le saut du
   repositionnement en mode relatif) est du contrôle de flux pur, enfermé
   dans `#[cfg(windows)]`. C'est le seul comportement de ce chantier dont une
   régression serait invisible sur Linux (pas de code à compiler), invisible
   en Vitest (côté client, rien ne dépend de cette décision), et non
   détectée par la recette (qui mesure la linéarité, pas la présence du
   saut). Une extraction en logique pure, sur le modèle de `cursor.rs` et
   `gamepad.rs`, la rendrait testable.
3. **La discipline « extraire pour tester » a été appliquée à ce qui
   calcule, pas à ce qui décide.** Sept extractions de ce chantier portent
   sur de l'arithmétique pure (hystérésis, comparaison de séquence,
   limitation de débit, clamp, sommation de deltas...), aucune sur une
   décision de branchement comme les deux ci-dessus. C'est la remarque à
   porter au chantier suivant : une décision de contrôle de flux mérite la
   même extraction qu'un calcul, précisément parce qu'elle est plus facile à
   casser silencieusement qu'un calcul qui produirait un résultat visiblement
   faux.

**Point de conception non écrit ailleurs, signalé par la revue** : le clic
qui arme le Pointer Lock est aussi transmis à Windows comme un vrai clic.
`input.ts:76` (`onPointerDown`) envoie le bouton pressé quoi qu'il arrive ;
`pointer.ts:95` (`onClick`) ne verrouille qu'ensuite, sur le même événement.
Dans un FPS, le clic qui reprend la souris **tire aussi**. Ce n'est pas un
défaut à corriger — l'armement sur clic est le bon mécanisme, faute
d'activation utilisateur transitoire sur un message reçu par data channel
(voir §5) — mais le prochain qui touchera à l'un de ces deux fichiers doit
savoir que les deux effets partent du même geste.

---

## 14. Hors périmètre

- **Pilote d'injection souris en mode noyau** — déjà hors périmètre du cadrage.
- **Plein écran piloté par Windows** (§4.1 du cadrage) — chantier D, qui apporte
  le hook `SetWinEventHook`.
- **Curseur applicatif custom** (bitmap transmis, overlay composé) — écarté au
  profit de la forme seule.
- **Plusieurs manettes** — le modèle produit est un utilisateur, un navigateur.
- **Manette DualShock/DualSense** — ViGEmBus l'émule, mais de nombreux jeux
  Windows ne reconnaissent que XInput.
- **Zones mortes et courbes de sensibilité configurables** — les jeux les
  fournissent.
- **Micro** — chantier E, spécifié séparément (`2026-07-28-micro-design.md`).
