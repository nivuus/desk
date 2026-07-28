# Support du jeu vidéo et modèle multi-fenêtres — cadrage

**Date** : 28 juillet 2026
**Statut** : Cadrage validé, implémentation ajournée
**Portée** : Décisions produit et décomposition en chantiers. Ce document ne contient
pas de plan d'implémentation — chaque chantier fera l'objet de sa propre spec puis
de son propre plan.

> **Origine** : question « peut-on lancer des jeux via Steam et y jouer de façon
> fluide et réactive ? ». La réponse est oui, mais elle engage des chantiers qui
> dépassent le jeu. Ce document fige les décisions prises pour permettre une
> reprise ultérieure sans refaire le raisonnement.

---

## 1. Verdict de faisabilité

**Oui.** L'architecture de la refonte
(`2026-07-27-refonte-produit-design.md`) est déjà celle du jeu en streaming :
capture GPU → encodage H.264 matériel → WebRTC → décodage matériel navigateur.
C'est le pipeline de Parsec, Moonlight et GeForce Now. Aucun détournement n'est
nécessaire.

### Atouts déjà acquis

- **GPU réel sur la VM cible** : NVIDIA RTX 4070 → rendu 3D natif *et* NVENC.
- **Encodeur déjà en mode faible latence** : `CODECAPI_AVLowLatencyMode` et GOP
  piloté (`agent/src/encode.rs:1270-1282`).
- **Pipeline de bout en bout fonctionnel** : le jalon 1 streame une fenêtre
  Windows avec entrées souris et clavier.

### Écarts constatés dans le code actuel

| # | Écart | Preuve | Gravité |
|---|---|---|---|
| 1 | Aucune capture audio. `Opus` est déclaré comme payload type SDP mais aucune source ne l'alimente | `agent/src/transport.rs:937` | Bloquant |
| 2 | Souris en absolu uniquement — inutilisable pour tout jeu à caméra libre, qui lit la souris en relatif via Raw Input | `agent/src/input.rs:104` (`MOUSEEVENTF_ABSOLUTE`) | Bloquant |
| 3 | Aucune manette | — | Bloquant pour le jeu à la manette |
| 4 | Débit fixe par variable d'environnement, fps câblé à 60 | `agent/src/main.rs:653-658` | Fort sur réseau variable |
| 5 | PTS incrémentés d'un tick constant au lieu de suivre l'horloge de capture → dérive et judder | `agent/src/windows_source.rs:310` | Fort |
| 6 | Jitter buffer du navigateur non neutralisé (`playoutDelayHint`) | client | Moyen |

### Latence — ce que l'architecture permet

| Réseau | Click-to-photon visé | Verdict |
|---|---|---|
| LAN | 20-40 ms | Tout est jouable, FPS compris |
| Internet faible latence (même pays) | 40-70 ms | Confortable sauf FPS compétitif |
| Internet dégradé (RTT > 60 ms) | 90 ms+ | Solo, stratégie et indés restent bons ; FPS hors d'atteinte |

Ces valeurs sont celles **de l'architecture**, attestées par Parsec et Moonlight
qui l'emploient. Elles ne décrivent pas le pipeline actuel.

### Latence — ce que le pipeline mesure réellement (28/07/2026)

La recette du jalon 1 (`plans/2026-07-27-jalon1-recette.md`) donne les premières
mesures de bout en bout. Elles sont très loin du compte :

| Grandeur | Mesure | Cible jalon 1 | Écart |
|---|---|---|---|
| Débit d'images | ~29,5-30 i/s (6 mesures convergentes) | ≥ 55 i/s | facteur 2 |
| Latence bout en bout | min 54,1 ms · **médiane 276 ms** · max 1517 ms (12 essais) | < 50 ms | facteur 5 sur la médiane |
| Redimensionnement | 27/32 sous 500 ms | < 500 ms | non fiable |

Bilan de la recette : **2 critères sur 5 tenus**. L'étage désigné responsable est
l'encodage, le tampon de gigue aggravant la latence.

**Conséquence directe pour le jeu** : à 30 i/s et 276 ms de latence médiane,
aucune des quatre familles retenues n'est jouable — pas même la stratégie. Aucun
des chantiers A à D ne changera cela : ils ajoutent des fonctions à un pipeline
qui n'atteint pas encore sa propre cible. **Ramener le pipeline à 60 i/s et sous
50 ms sur LAN est le prérequis de tout le reste** (chantier 0, §8).

Le plafond ultime, lui, n'est pas technique mais économique : une VM à GPU dédié
par utilisateur ne se mutualise pas.

---

## 2. Décisions actées

| Décision | Choix | Justification |
|---|---|---|
| Statut du jeu | **Cas d'usage produit à part entière** | Décidé explicitement ; entre dans la roadmap au même titre que le pont fichiers |
| Familles visées | **Toutes** : stratégie/RPG iso/gestion, solo AAA à la manette, FPS/TPS à la souris, indés | Le FPS étant le plus exigeant, c'est lui qui dicte l'architecture |
| Réseau cible | **Internet quelconque, y compris dégradé** | Impose débit adaptatif, FEC et TURN dès la conception |
| Tension FPS ⇄ réseau dégradé | **Dégradation gracieuse assumée** | Voir §3 |
| Modèle de fenêtres | **Une fenêtre principale Windows = une fenêtre navigateur** | Voir §4 — remplace le modèle mono-fenêtre de la spec produit |
| Filtrage des fenêtres | **Seulement les fenêtres « Alt-Tab-ables »** | Menus, tooltips et dialogues restent composés dans leur fenêtre parente |
| Plein écran | **Windows maître, sens unique** | L'application décide, le navigateur suit ; le navigateur ne pilote jamais le plein écran côté Windows. Voir §4.1 |
| Intégration Steam | **Aucune intégration spécifique** | Le modèle multi-fenêtres rend Steam gratuit, et couvre aussi Epic, GOG, itch.io |

---

## 3. Tension assumée : FPS compétitif sur réseau dégradé

Ces deux exigences sont physiquement incompatibles. Le RTT réseau est un plancher
qu'aucune implémentation ne franchit : à 100 ms d'aller-retour, aucun produit au
monde ne rend un FPS compétitif confortable.

**Résolution retenue — dégradation gracieuse** :

1. Le pipeline vise l'excellence quand le réseau le permet.
2. Il se replie proprement quand il ne le permet pas : résolution réduite avant
   qualité écrasée, latence privilégiée sur fidélité d'image.
3. **Il le dit à l'utilisateur.** Un indicateur de qualité de connexion annonce
   explicitement quand le réseau ne permet pas le jeu nerveux, plutôt que de
   livrer une expérience molle et inexpliquée. C'est ce que fait GeForce Now.

Formulation de l'engagement produit : le FPS est confortable **sur bon réseau** ;
les autres familles sont bonnes partout.

---

## 4. Modèle multi-fenêtres — changement structurant

**Décision** : quand une nouvelle fenêtre principale apparaît dans Windows, une
nouvelle fenêtre s'ouvre dans le navigateur.

Ce modèle dépasse largement le jeu. Il remplace l'hypothèse mono-fenêtre implicite
de `2026-07-27-refonte-produit-design.md` (§5①, « capture ciblée sur la fenêtre de
l'application ») et doit être répercuté dans cette spec.

### Ce qu'il apporte

- Steam devient un cas particulier gratuit : l'utilisateur ouvre Steam, lance un
  jeu, le jeu apparaît dans sa propre fenêtre navigateur, les deux coexistent.
- Fonctionne identiquement pour Epic, GOG, itch.io, ou tout logiciel
  multi-fenêtres (Photoshop, IDE, suites bureautiques).
- Rapproche le produit du comportement natif attendu.

### Critère de filtrage retenu

Une fenêtre mérite une fenêtre navigateur si elle est « Alt-Tab-able » :

- top-level et visible (`WS_VISIBLE`) ;
- sans propriétaire (`GetWindow(hwnd, GW_OWNER)` nul) ;
- sans `WS_EX_TOOLWINDOW`, ou bien avec `WS_EX_APPWINDOW` ;
- non masquée par DWM (`DwmGetWindowAttribute` / `DWMWA_CLOAKED`) — sans ce
  filtre on capte les fenêtres UWP fantômes.

Tout le reste (menus déroulants, tooltips, dialogues modaux, splash screens)
reste composé dans sa fenêtre parente.

### Conséquence technique majeure — décision ouverte

La capture actuelle utilise **DXGI Desktop Duplication** : elle duplique l'écran
entier puis recadre la fenêtre. Avec plusieurs fenêtres, deux fenêtres qui se
chevauchent produisent un recadrage pollué par ce qui est au-dessus.

Deux voies, à trancher lors de la spec du chantier D :

1. **Revenir à `Windows.Graphics.Capture`** par fenêtre, qui capture le contenu
   hors-écran indépendamment du chevauchement. C'était le choix initial de la
   spec produit, abandonné en tâche 9 du jalon 1 (commit `4493b24`) — il faudra
   comprendre pourquoi avant de revenir dessus.
2. **Conserver Desktop Duplication** et garantir que les fenêtres ne se
   chevauchent jamais sur le bureau virtuel (disposition imposée par l'agent).
   Plus simple mais fragile.

### 4.1 Plein écran

**Décision** : **Windows est maître, dans un sens unique.** Quand une application
passe en plein écran, la fenêtre navigateur correspondante suit. L'inverse n'existe
pas : le navigateur ne force jamais la fenêtre Windows en plein écran.

**Détection côté agent.** Trois signaux possibles, à arbitrer à l'implémentation :
comparaison de `GetWindowRect` avec le rect du moniteur (`MonitorFromWindow` +
`GetMonitorInfo`), perte des styles de bordure, ou `SHQueryUserNotificationState()`
qui renvoie `QUNS_RUNNING_D3D_FULL_SCREEN` — c'est le mécanisme par lequel Windows
supprime lui-même les notifications pendant un jeu. Le déclencheur est le
`SetWinEventHook` du chantier D (`EVENT_OBJECT_LOCATIONCHANGE`,
`EVENT_SYSTEM_FOREGROUND`). Nouveau message dans `AgentControl`
(`proto/src/control.rs:45`).

**Déclenchement côté client — obstacle identique à `window.open()`.**
`element.requestFullscreen()` exige une activation utilisateur transitoire ; un
message reçu sur data channel n'en est pas une, l'appel est rejeté. Solution
retenue : **armement sur le prochain clic**. Le client mémorise la demande et
entre en plein écran au premier événement pointeur — qui survient de toute façon,
puisqu'il faut cliquer pour jouer. Coût réel : un clic, imperceptible. Repli si
insuffisant : bandeau cliquable explicite.

**Conséquences assumées du choix « Windows maître »** :

- L'utilisateur ne peut pas réclamer le plein écran depuis le navigateur ; il
  passe par les options du jeu. C'est le comportement natif.
- S'il sort du plein écran côté navigateur (Échap, F11), l'agent ne réagit pas :
  l'application reste en plein écran Windows, affichée mise à l'échelle dans une
  fenêtre plus petite. Le `ResizeObserver` existant
  (`client/src/main.ts:47-53`) transmet malgré tout le nouveau viewport, donc la
  résolution d'encodage s'ajuste. Dégradation visuelle acceptable, pas de
  désynchronisation d'état.
- **Aucune boucle d'oscillation possible**, le sens étant unique. C'est le
  principal mérite de ce choix face à un miroir bidirectionnel.

**Condition de viabilité — la touche Échap.** En plein écran navigateur, Échap en
sort, et c'est non-interceptable par conception de la spec Fullscreen. Or Échap
ouvre le menu pause dans la quasi-totalité des jeux : sans traitement, le joueur
quitte le plein écran à chaque pause. `navigator.keyboard.lock(['Escape'])`
(Keyboard Lock) redirige Échap vers la page et impose un appui long pour sortir ;
l'API n'est disponible qu'en plein écran, cas d'usage pour lequel elle a été
conçue. Elle est **limitée à Chromium** : sur Firefox et Safari, Échap cassera le
plein écran. Limite à documenter, pas à corriger.

**Résolution.** Une fenêtre Windows en plein écran adopte la résolution du
moniteur virtuel. Le « SudoMaker Virtual Display Adapter » doit donc annoncer une
résolution et une fréquence cohérentes avec le viewport client, faute de quoi
l'image subit deux mises à l'échelle successives.

---

## 5. Décomposition en chantiers

Quatre chantiers indépendants. Chacun suit son propre cycle spec → plan →
implémentation.

### Chantier A — Audio

Bénéficie à tout le produit, pas seulement au jeu.

- **Capture** : WASAPI en mode loopback (`IAudioClient` +
  `AUDCLNT_STREAMFLAGS_LOOPBACK`) sur le périphérique de rendu par défaut.
- **Capture par processus** : Windows 10 build 19041+ expose le *process
  loopback* (`AUDIOCLIENT_ACTIVATION_PARAMS` / `PROCESS_LOOPBACK`), qui isole
  l'audio d'un seul processus. C'est exactement ce qu'exige le modèle
  multi-fenêtres. **Sondé le 28/07/2026 (tâche 10 du chantier A) : non
  déterminé.** Le code compile et lie sur la cible Windows réelle
  (`agent::wasapi::probe_process_loopback`, gestionnaire de complétion COM
  `IActivateAudioInterfaceCompletionHandler` via `#[implement]`), mais son
  exécution sur la VM (build 20348, donc éligible sur le papier) n'a produit
  ni journal exploitable ni erreur diagnostiquable dans le temps imparti :
  le processus sonde se termine avec un code de sortie 0 sans qu'aucune ligne
  de trace n'atteigne `agent.log`, et une invocation synchrone équivalente
  reste bloquée sans jamais rendre la main — aucun rapport de plantage
  Windows (WER) ne corrèle avec ces tentatives. Résultat non tranché ; voir
  `docs/superpowers/plans/2026-07-28-audio-resultats.md` pour le détail. **À
  reprendre au chantier D**, avec de meilleurs outils de diagnostic côté
  Windows (débogueur attaché, sortie vers un fichier dédié plutôt que
  `Tee-Object`/stdout à travers la tâche planifiée) — la valeur de cette
  sonde reste informative, pas structurante.
- **Encodage** : Media Foundation n'expose pas d'encodeur Opus. Passer par les
  bindings libopus (crate `opus` ou `audiopus`). 48 kHz stéréo, trames de 10 ms,
  ~128 kbps, mode `RESTRICTED_LOWDELAY`, FEC in-band activé.
- **Transport** : ajouter un media audio via `sdp_api()` de str0m. Le payload
  type 111 est déjà déclaré (`agent/src/transport.rs:937`).
- **Synchronisation A/V** : horodatage sur une horloge commune, RTCP Sender
  Reports.
- **Risque levé le 28/07/2026 (tâche 10 du chantier A)** : oui, la VM dispose
  d'un périphérique de rendu audio actif. `Get-CimInstance Win32_SoundDevice`
  et `Get-PnpDevice -Class AudioEndpoint` relèvent **deux endpoints actifs**
  (état `OK`) : « Haut-parleurs (Steam Streaming Speakers) », un périphérique
  **virtuel**, et « HDP-V104 (NVIDIA High Definition Audio) ». Aucun pilote
  audio virtuel supplémentaire n'a été nécessaire — celui de Steam suffit.
  Format de mixage réellement relevé par WASAPI (sonde de la tâche 5,
  reconfirmé à chaque session de la tâche 10) : **48 000 Hz, 2 canaux, 32
  bits flottant**, conforme à l'hypothèse de conception (aucun
  rééchantillonneur nécessaire).

### Chantier B — Input jeu

- **Souris relative** : `requestPointerLock()` côté navigateur, lecture de
  `movementX`/`movementY` ; nouveau type d'événement « mouvement relatif » dans
  le protocole binaire `proto/` ; côté agent, `SendInput` avec `MOUSEEVENTF_MOVE`
  **sans** `MOUSEEVENTF_ABSOLUTE`.
- **Piège majeur** : `SendInput` en relatif passe par la sensibilité et
  l'accélération pointeur de Windows. Sans neutralisation, la visée est déformée
  et non linéaire — inacceptable en FPS. Il faut désactiver « Améliorer la
  précision du pointeur » et fixer la sensibilité au facteur 1:1
  (`SystemParametersInfo` / `SPI_SETMOUSE`, `SPI_SETMOUSESPEED`) sur la VM.
- **Fidélité maximale** : Parsec utilise un pilote d'injection en mode noyau pour
  contourner entièrement cette couche. Hors périmètre v1, à garder en tête si la
  neutralisation logicielle s'avère insuffisante.
- **Manette** : pilote **ViGEmBus** à installer sur la VM + crate
  `vigem-client`, exposant une manette Xbox 360 (XInput est le plus universel).
  Côté navigateur, Gamepad API avec polling à 250 Hz minimum, envoi sur le data
  channel. Piège : la Gamepad API exige un geste utilisateur avant d'exposer les
  manettes, et le mapping `standard` n'est pas garanti selon les modèles.
- **Clavier** : les raccourcis réservés du navigateur (Ctrl+W, F11, Alt+Tab)
  n'atteignent pas le jeu. La Keyboard Lock API (`navigator.keyboard.lock()`)
  les libère, mais uniquement en plein écran et seulement sur Chromium.
  **Cette API ne relève pas du confort : elle conditionne la viabilité du plein
  écran**, Échap étant à la fois la touche de sortie du plein écran navigateur et
  la touche de menu pause de presque tous les jeux. Voir §4.1.
- **Curseur** : sous Pointer Lock, le jeu dessine son propre curseur, donc rien à
  faire. Hors Pointer Lock, transmettre la forme du curseur par data channel pour
  un rendu local sans latence.

### Chantier C — Adaptation réseau

Imposé par la cible « internet quelconque ». Bénéficie à tout le produit.

- ~~**Pacing des PTS sur horloge réelle** — corrige
  `agent/src/windows_source.rs:310`, qui incrémente un tick constant et
  dérive.~~ **Périmé : corrigé le 28/07/2026** (commit `857af65`, en dehors de
  ce chantier). Ce qui reste réellement à faire au chantier C n'est donc pas
  le pacing lui-même, mais l'exploitation de ses conséquences : **le débit
  adaptatif** ci-dessous (lire les RTCP Receiver Reports pour ajuster le
  débit d'encodage à chaud — rien de tel n'existe encore, le débit reste fixé
  par variable d'environnement). Par ailleurs, le chantier A (tâche 7,
  commit `733558e`) a corrigé un défaut voisin mais distinct : le
  `wallclock` des RTCP **Sender** Reports annonçait l'instant d'écriture du
  paquet plutôt que l'instant de capture, ce qui aurait fait annoncer l'audio
  en avance sur la vidéo de tout le délai d'encodage. Les deux corrections
  (pacing des PTS, wallclock des Sender Reports) sont indépendantes et
  toutes deux acquises ; seul le débit adaptatif à partir des **Receiver**
  Reports reste ouvert.
- **Débit adaptatif** : lire les RTCP Receiver Reports (perte, jitter) et
  l'estimation de bande passante côté str0m, puis ajuster
  `CODECAPI_AVEncCommonMeanBitRate` à chaud. Le support TWCC / BWE de str0m 0.21
  est à vérifier — c'est un prérequis, pas un acquis.
- **Résolution adaptative** : sous contrainte, réduire la résolution d'encodage
  plutôt que d'écraser la qualité. Reconfiguration de l'encodeur à chaud.
- **Résilience** : NACK + RTX pour la vidéo, FEC in-band Opus pour l'audio,
  keyframe sur demande (PLI/FIR → IDR forcé).
- **TURN** : coturn, déjà prévu par la spec produit.
- **Latence de restitution** : `playoutDelayHint = 0` sur le receiver (Chromium).
  Si insuffisant, basculer sur WebCodecs, option déjà envisagée par la spec
  produit (§5②).
- **Indicateur de qualité** : RTT, perte, débit affichés côté client, avec
  avertissement explicite quand le réseau ne permet pas le jeu nerveux (§3).

### Chantier D — Multi-fenêtres

Le plus structurant et le plus risqué. Refonte du modèle produit (§4).

- **Détection** : `SetWinEventHook` sur `EVENT_OBJECT_SHOW`, `EVENT_OBJECT_HIDE`
  et `EVENT_OBJECT_DESTROY`, filtré sur `idObject == OBJID_WINDOW`. Hook global
  `WINEVENT_OUTOFCONTEXT`, nécessitant une pompe de messages dans un thread
  dédié.
- **Filtrage** : critère « Alt-Tab-able » du §4.
- **Détection du plein écran** : le même hook alimente la bascule plein écran du
  §4.1 (`EVENT_OBJECT_LOCATIONCHANGE` + comparaison au rect du moniteur).
- **Topologie WebRTC** : une `RTCPeerConnection` par fenêtre navigateur plutôt
  qu'une session à N pistes — l'isolation évite qu'une fenêtre en panne
  n'affecte les autres, au prix de N négociations ICE.
- **Budget encodeurs** : les GPU GeForce récents plafonnent les sessions NVENC
  simultanées (ordre de grandeur : 8 sur Ada, à confirmer pour la RTX 4070).
  Suspendre l'encodage des fenêtres masquées, piloté par la Page Visibility API
  côté client — souhaitable en soi, pas seulement comme contournement.
- **Risque n°1 — popup blocker : LEVÉ le 28/07/2026.** Mesuré sur ChromeOS —
  résultats et protocole dans `plans/2026-07-28-spike-multifenetres-resultats.md`.
  Le modèle tient, mais l'hypothèse « une PWA installée a plus de latitude » est
  **fausse** : sans permission, une PWA installée est bloquée exactement comme un
  onglet (3 mesures concordantes). Ce qui débloque l'ouverture sans geste est la
  **permission pop-up du site**, accordée une fois comme les notifications.
  Mécanisme retenu : demander cette permission à l'installation (coût nul
  ensuite) ; replis validés, l'armement sur le prochain clic — déjà retenu au
  §4.1 pour le plein écran, donc **un seul mécanisme pour les deux besoins** —
  puis la notification cliquable, qui fonctionne fenêtre en arrière-plan.
  Reste à vérifier avant engagement sur poste desktop : Chrome sous Windows,
  macOS et Linux n'ont pas été testés, et ChromeOS est la plateforme au meilleur
  support multi-fenêtres — l'inférence ne va pas dans ce sens.
- **Cycle de vie** : fenêtre Windows fermée → fenêtre navigateur fermée ;
  fenêtre navigateur fermée → `WM_CLOSE` sur la fenêtre Windows (à confirmer
  comme comportement souhaité).
- **Capture** : trancher la décision ouverte du §4 (WGC vs Desktop Duplication).

---

## 6. Prérequis propres aux jeux

- **Plein écran fenêtré imposé.** Le plein écran exclusif court-circuite le
  compositeur et complique la capture. Parsec impose la même contrainte.
- **Session interactive.** Steam et les jeux exigent la session 1 ; l'agent y
  tourne déjà (contrainte de frontière de session du jalon 1). Acquis.
- **Affichage virtuel.** Le « SudoMaker Virtual Display Adapter » doit annoncer
  les résolutions et fréquences de rafraîchissement visées.
- **Anti-triche.** Certains anti-triche en mode noyau (Riot Vanguard, Easy
  Anti-Cheat selon configuration) **refusent de s'exécuter en machine
  virtuelle**. Valorant est notamment inaccessible par construction. C'est une
  limite produit à documenter, pas un défaut à corriger.
- **DRM et overlay Steam.** L'overlay Steam s'injecte dans le processus du jeu ;
  son interaction avec la capture est à vérifier empiriquement.

### 6.1 Plateformes clientes

Les décisions prises engagent le support navigateur. Récapitulatif :

| Brique | Chromium desktop | **ChromeOS** | Firefox | Safari |
|---|---|---|---|---|
| Keyboard Lock — **condition du plein écran** (§4.1) | Oui | **Oui** | Non | Non |
| Pointer Lock / souris relative (chantier B) | Oui | **Oui** | Oui | Oui |
| Gamepad API (chantier B) | Oui | **Oui** | Oui | Partiel |
| Décodage H.264 matériel | Oui | **Oui** | Oui | Oui |
| PWA multi-fenêtres (chantier D) | Oui | **Meilleur support** | Non | Non |
| File System Access (pont fichiers) | Oui | **Oui** | Non | Non |
| `file_handlers` | Oui, mais bridé par l'OS | **Le moins bridé** | Non | Non |

**ChromeOS est la plateforme cliente privilégiée.** Un Chromebook en client léger
devant une VM à GPU dédié correspond exactement au modèle GeForce Now : toute la
puissance est distante, le client ne fait que décoder. Les limites y sont
matérielles et non logicielles — les modèles d'entrée de gamme plafonnent en
résolution de décodage, et le Wi-Fi est souvent le facteur limitant avant le CPU.

**Sur Firefox et Safari**, le produit reste utilisable pour les applications mais
le jeu se dégrade : absence de Keyboard Lock (Échap casse le plein écran à chaque
menu pause, §4.1) et absence de multi-fenêtres (§4). À documenter comme limite
assumée, pas à corriger.

---

## 7. État de départ

**Le jalon 1 est terminé** (tâches 1 à 14) et sa recette est conduite. Le
pipeline capture, encode, transporte et affiche une fenêtre Windows, avec souris,
clavier, molette et redimensionnement.

Mais la recette (`plans/2026-07-27-jalon1-recette.md`) conclut à **2 critères sur
5 tenus** : la capture par fenêtre et les entrées fonctionnent, le débit d'images,
la latence et la fiabilité du redimensionnement échouent. Les chiffres sont au §1.

Le jalon a donc rempli son rôle — valider le pari technique et **mesurer** — sans
atteindre ses cibles de performance. C'est ce constat, et non une intuition, qui
justifie le chantier 0 du §8.

---

## 8. Ordre recommandé

0. **Chantier 0 — ramener le pipeline à ses cibles** : 60 i/s et moins de 50 ms
   sur LAN. **Prérequis absolu.** Tant que la médiane est à 276 ms et le débit à
   30 i/s (§1), aucun jeu n'est jouable et tout chantier suivant enrichit un
   pipeline inutilisable. La recette désigne l'encodage comme étage responsable
   et le tampon de gigue comme aggravant ; ce chantier commence donc par
   confirmer ce diagnostic avant d'optimiser. Recouvre partiellement le chantier
   C (pacing des PTS sur horloge réelle, `playoutDelayHint`).
1. **Chantiers A + B** (audio, input jeu) — les deux manques fonctionnels
   bloquants, indépendants et peu risqués. À leur terme, un jeu est jouable et
   mesurable en conditions réelles.
2. **Chantier C** (adaptation réseau) — conçu à partir des mesures obtenues en
   1, plutôt qu'à l'aveugle.
3. **Chantier D** (multi-fenêtres) — le plus gros. Son risque n°1 (popup
   blocker) a été levé par un test isolé le 28/07/2026, avant les chantiers A et
   B comme prévu : le modèle produit est confirmé, par la permission pop-up du
   site et non par le statut de PWA installée (§5 D).

Cet ordre est une recommandation, pas un engagement. L'argument pour remonter D
en premier existe : il change la capture, et A/B/C construits sur une hypothèse
mono-fenêtre devraient être partiellement défaits.

---

## 9. Hors périmètre

- Intégration Steam spécifique (lecture de la bibliothèque, jaquettes,
  `steam://rungameid`) — le modèle multi-fenêtres rend cette intégration
  facultative. À reconsidérer comme confort produit ultérieur.
- Pilote d'injection souris en mode noyau.
- Multi-moniteurs, enregistrement de session — déjà hors périmètre de la spec
  produit.
- Encodage AV1, mentionné par la spec produit comme option GPU-dépendante, non
  traité ici.

---

## 10. Répercussions sur la spec produit

`2026-07-27-refonte-produit-design.md` doit être amendée sur trois points :

1. §5① « capture ciblée sur la fenêtre de l'application » → modèle
   multi-fenêtres (§4 du présent document).
2. §2 Objectifs → ajouter le jeu vidéo comme cas d'usage supporté, avec la
   nuance de dégradation gracieuse (§3).
3. §9 Ordre de construction → insérer les chantiers A à D.
