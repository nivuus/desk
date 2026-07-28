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

### Latence atteignable

| Réseau | Click-to-photon attendu | Verdict |
|---|---|---|
| LAN | 20-40 ms | Tout est jouable, FPS compris |
| Internet faible latence (même pays) | 40-70 ms | Confortable sauf FPS compétitif |
| Internet dégradé (RTT > 60 ms) | 90 ms+ | Solo, stratégie et indés restent bons ; FPS hors d'atteinte |

Le plafond réel n'est pas technique mais économique : une VM à GPU dédié par
utilisateur ne se mutualise pas.

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
  multi-fenêtres. La VM cible est en build 20348, donc éligible — à valider.
- **Encodage** : Media Foundation n'expose pas d'encodeur Opus. Passer par les
  bindings libopus (crate `opus` ou `audiopus`). 48 kHz stéréo, trames de 10 ms,
  ~128 kbps, mode `RESTRICTED_LOWDELAY`, FEC in-band activé.
- **Transport** : ajouter un media audio via `sdp_api()` de str0m. Le payload
  type 111 est déjà déclaré (`agent/src/transport.rs:937`).
- **Synchronisation A/V** : horodatage sur une horloge commune, RTCP Sender
  Reports.
- **Risque à lever** : la VM dispose-t-elle d'un périphérique de rendu audio ?
  Elle a un « SudoMaker Virtual Display Adapter » pour l'affichage, rien
  d'équivalent n'est connu pour le son. Sans périphérique de rendu actif, WASAPI
  loopback ne produit rien. Un pilote audio virtuel peut être nécessaire.

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
- **Curseur** : sous Pointer Lock, le jeu dessine son propre curseur, donc rien à
  faire. Hors Pointer Lock, transmettre la forme du curseur par data channel pour
  un rendu local sans latence.

### Chantier C — Adaptation réseau

Imposé par la cible « internet quelconque ». Bénéficie à tout le produit.

- **Pacing des PTS sur horloge réelle** — corrige
  `agent/src/windows_source.rs:310`, qui incrémente un tick constant et dérive.
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
- **Topologie WebRTC** : une `RTCPeerConnection` par fenêtre navigateur plutôt
  qu'une session à N pistes — l'isolation évite qu'une fenêtre en panne
  n'affecte les autres, au prix de N négociations ICE.
- **Budget encodeurs** : les GPU GeForce récents plafonnent les sessions NVENC
  simultanées (ordre de grandeur : 8 sur Ada, à confirmer pour la RTX 4070).
  Suspendre l'encodage des fenêtres masquées, piloté par la Page Visibility API
  côté client — souhaitable en soi, pas seulement comme contournement.
- **Risque n°1 — popup blocker** : `window.open()` déclenché sans geste
  utilisateur est bloqué par défaut dans tous les navigateurs. Une PWA installée
  a plus de latitude mais rien n'est garanti. Repli envisagé : notification
  cliquable (« Elden Ring est prêt → ouvrir ») ou barre des tâches dans le hub.
  **À valider en tout premier** : ce point peut invalider le modèle.
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

---

## 7. État de départ

Le jalon 1 (`2026-07-27-jalon1-tranche-verticale.md`) couvre les tâches 1 à 12
d'après l'historique git. Restent :

- **Tâche 13** — redimensionnement et fin de session.
- **Tâche 14** — instrumentation et recette du jalon.

La tâche 14 est un prérequis de fait pour tous les chantiers ci-dessus : elle
fournit la mesure de latence sans laquelle on optimise à l'aveugle.

---

## 8. Ordre recommandé

1. **Finir le jalon 1** (tâches 13 et 14) — établit la base de mesure.
2. **Chantiers A + B** (audio, input jeu) — les deux manques réellement
   bloquants, indépendants et peu risqués. À leur terme, un jeu est jouable et
   mesurable en conditions réelles.
3. **Chantier C** (adaptation réseau) — conçu à partir des mesures obtenues en
   2, plutôt qu'à l'aveugle.
4. **Chantier D** (multi-fenêtres) — le plus gros. Son risque n°1 (popup
   blocker) mérite cependant d'être levé par un test isolé **dès maintenant**,
   avant même les chantiers A et B : une réponse négative changerait le modèle
   produit.

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
