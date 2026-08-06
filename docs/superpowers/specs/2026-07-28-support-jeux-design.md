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

### Conséquence technique majeure — sondée le 30/07/2026

La capture actuelle utilise **DXGI Desktop Duplication** : elle duplique l'écran
entier puis recadre la fenêtre. Avec plusieurs fenêtres, deux fenêtres qui se
chevauchent produisent un recadrage pollué par ce qui est au-dessus. **Mesuré** :
`verdicts_faux` strictement positif dès N=2 (449 / 449 / 536 à N = 2 / 4 / 8).

Cette section portait une **décision ouverte** entre deux voies. **Les deux
branches de l'alternative sont démenties par la mesure**, dont le détail, les
chiffres et les journaux sont dans
`docs/superpowers/plans/2026-07-30-sonde-capture-multifenetre-resultats.md`.

**Voie écartée n°1 — revenir à `Windows.Graphics.Capture` par fenêtre.**
C'était le choix initial de la spec produit, abandonné en tâche 9 du jalon 1
(commit `4493b24`). Raison mesurée de son écartement : sur cette VM,
`CreateForWindow` échoue en `0x800706BE` (`RPC_S_SERVER_UNAVAILABLE`), reproduit
trois fois à l'identique sans redémarrage. Les deux réparations bon marché sont
exclues par la même mesure : `IsSupported()` rend `true` (le `E_OUTOFMEMORY` du
jalon 1 ne se reproduit plus) **et** le service `CaptureService_5865d` existe et
tourne. Ni composant absent, ni service arrêté ; la cause profonde reste
inélucidée. La voie demeure architecturalement la bonne réponse — par fenêtre,
hors-écran, chemin GPU, sans acrobatie de topologie d'affichage — et mérite un
créneau **borné** de diagnostic RPC, mais elle n'est pas le chemin critique.

**Voie écartée n°2 — conserver Desktop Duplication en garantissant le
non-recouvrement** par une disposition en tuiles imposée par l'agent. Raison
mesurée de son écartement : **la garantie n'existe pas**. Un menu contextuel
ouvert près d'un bord de tuile déborde de ≈284 px à droite et ≈142 px en bas sur
les tuiles voisines (mesuré sur capture d'écran, journal joint). C'est Windows
qui place les menus, et il ne connaît que les frontières de moniteurs **réels** :
l'agent ne peut pas l'empêcher. S'y ajoute une contrainte de surface — 800×360
par fenêtre à 8 fenêtres sur le bureau réel 2400×1080, très en deçà de 1280×720.
Le tuilage peut survivre comme *heuristique de placement*, jamais comme garantie.

**Voie recommandée — un moniteur virtuel par fenêtre.** Le pilote d'affichage
indirect produit une sortie DXGI réelle et attachée, portée par l'adaptateur qui
possède NVENC (donc sans copie inter-périphérique), mesurée à 3413×960 — seule
mesure de ce document sans journal joint, prise pendant une session Apollo
éphémère non reproductible sans le propriétaire du poste (détail et réserve
dans le document de résultats). Une fenêtre par moniteur supprime le
recouvrement *par construction* au lieu de le discipliner, tout en préservant
le chemin GPU. Les **deux réserves bloquantes** qui interdisaient de la
spécifier **sont levées** (31/07/2026,
`plans/2026-07-31-mesures-prealables-chantier-d-resultats.md`) : (a) le
**plafond de sorties virtuelles simultanées vaut 10**, le pilote refusant la
11ᵉ création en `ERROR_TOO_MANY_NAMES`, preuve par identité des onze sorties
présentes au refus — pour une cible de 8 fenêtres, **la voie tient avec 2 de
marge**, et notre code commande désormais le pilote lui-même sans dépendre
d'Apollo ; (b) sa correction d'image **est mesurée** et non plus seulement
argumentée par construction — Windows compose bien des fenêtres sur un moniteur
virtuel sans écran physique, dont Desktop Duplication rend l'image exacte
(900/900 verdicts justes, zéro image noire, 90,0 i/s par fenêtre).

**La mesure qui restait due — N duplications DXGI de front sur N sorties
virtuelles, l'arrangement que la voie propose réellement — a été prise le
31/07/2026** (`plans/2026-07-31-duplications-paralleles-resultats.md`), **et la
voie est reçue** : 90,1 i/s par fenêtre en capture+encodage à N=8, zéro verdict
faux, contre un critère de 60 i/s posé d'avance. **Portée exacte** : une
exécution par rang donc aucun taux, et **rien au-delà de 8 sorties — 8 est ce
qui a été demandé et obtenu, pas une limite trouvée**. Détail et réserves en §5
et §6.

> ⚠️ **Réserve ajoutée le 1ᵉʳ août 2026, après la première exécution en
> conditions de produit (sous-bloc D1,
> `plans/2026-08-01-multifenetres-tranche-verticale-resultats.md`).** Cette
> mesure n'est **pas réfutée** — le banc créait ses N sorties virtuelles
> **avant** d'ouvrir la moindre duplication, et dans cet ordre-là tout tient.
> **Mais ce n'est pas l'ordre du produit.** En exploitation, une fenêtre
> s'ouvre alors que d'autres capturent déjà : la création de sa sortie fait
> alors abandonner le mutex des duplications ouvertes (`0x887A0026`) et **tue
> toutes les sessions en cours**. La voie tient donc à arrangement figé, et
> s'effondre dès qu'une fenêtre s'ouvre. À corriger avant de la déclarer
> viable en production.

> ✅ **Corrigé et démontré corrigé par le sous-bloc D2, le 1ᵉʳ août 2026**
> (`plans/2026-08-01-multifenetres-arrangement-dynamique-resultats.md`).
> **La phrase « s'effondre dès qu'une fenêtre s'ouvre » n'est plus vraie.**
> L'abandon du mutex se produit toujours — il n'est ni évité ni expliqué — mais
> il est **encaissé** : la duplication est relâchée puis rouverte dans une
> fenêtre de reprise bornée en durée. Relevé en conditions de produit, sur de
> vraies applications : **44 pertes d'accès `0x887A0026`, aucune session
> perdue**, montées 1→2, 2→3, 3→4 propres, une seule clôture de session et elle
> était sollicitée. La destruction d'une sortie abandonne le mutex elle aussi,
> et la reprise l'encaisse également.
>
> ⚠️ **Mais un plafond DISTINCT apparaît à quatre fenêtres simultanées**, que
> D2 n'a pas levé : la 5ᵉ duplication DXGI, **dans un 5ᵉ processus**, est refusée
> en `0x887A0022`, par une limite de **concurrence** qui résiste à trois secondes
> de patience explicite. **La couche qui l'impose n'est pas identifiée**, et
> **rien n'établit que 4 soit une borne du système**. Ce n'est ni le plafond de
> sorties virtuelles (10), ni celui des encodeurs NVENC (8) — la mort survient
> avant tout encodeur. Le rapprochement avec les 8 duplications d'un **seul**
> processus est une **inférence**. **C'est le point bloquant pour la cible de
> huit fenêtres de ce chantier.**
>
> 🔢 **L'inférence a été remplacée par une mesure le 2 août 2026 (sous-bloc
> D3)** : le plafond porte bien sur le nombre de **processus** concurrents
> tenant une duplication, et vaut **exactement 4** — un banc unique oppose 1, 2,
> 4 et 8 processus, huit duplications passent sur au plus quatre processus, et
> le rang qui échoue a **moins** de duplications ouvertes que ceux qui passent.
> ⚠️ **Les deux réserves ci-dessus TIENNENT** : la couche n'est **toujours pas**
> identifiée, et **rien n'établit toujours que 4 soit une borne du système**.
> **Le point reste bloquant pour la cible de huit fenêtres** — il ne se lève
> qu'en mutualisant la capture (un seul processus tenant les N duplications),
> voie **désignée pour D4** et non implémentée.

**Repli mesuré — `PrintWindow(PW_RENDERFULLCONTENT)`.** Contre toute attente,
cette voie rend l'image **juste** d'une fenêtre D3D **recouverte** : c'est la
seule à avoir franchi la porte de correction par une mesure directe. Sa limite
est le **chemin CPU** (`GetDIBits` puis téléversement GPU), éliminatoire pour le
jeu — 29,1 i/s par fenêtre à deux fenêtres, **passe `capture` seule** (la passe
`capture+encodage` n'a pas été conclue à ce rang). N=4 et N=8 ont depuis été
mesurés : **17,6 puis 8,8 i/s par fenêtre**, sans palier — voir §5. La voie ne
tient donc pas la cible de 8 fenêtres ; elle reste acceptable pour les fenêtres
de productivité que le modèle multi-fenêtres doit porter à côté du jeu, et pour
les cas où la voie principale ne s'applique pas.

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

> ❌ **ARBITRÉ, ET DEUX DES TROIS SIGNAUX SONT ÉCARTÉS** — sous-bloc D8, 5 août
> 2026 (`docs/superpowers/specs/2026-08-04-multifenetres-plein-ecran-design.md`,
> `…/plans/2026-08-04-multifenetres-plein-ecran-resultats.md`). La prémisse de
> ce paragraphe — trois signaux à arbitrer — tient ; **la conclusion sur le
> premier est FAUSSE dans l'architecture née de D1** :
>
> - ⛔ **`GetWindowRect` contre le rect du moniteur ne distingue RIEN.** Chaque
>   fenêtre est seule sur sa propre sortie virtuelle et l'occupe **exactement**,
>   le superviseur le lui réimposant périodiquement
>   (`controler_le_placement`, `agent/src/superviseur/boucle/placement_periodique.rs:21`).
>   « rect fenêtre == rect moniteur » est donc l'état **NOMINAL**. Le critère
>   n'est pas seulement inopérant : **il est TOUJOURS VRAI**, et une
>   implémentation fidèle à cette page annoncerait le plein écran **en
>   permanence, pour toutes les fenêtres.**
> - ⛔ **`SHQueryUserNotificationState()` est global à la session interactive,
>   pas par fenêtre** : à N fenêtres il ne dit pas *laquelle*. Et il ne voit pas
>   le « borderless fullscreen », que la quasi-totalité des jeux modernes
>   emploient.
> - ✅ **Le signal survivant est le SECOND — la perte des styles de bordure**,
>   et c'est celui que D8 a implémenté (`agent/src/capteur/plein_ecran.rs`,
>   prédicat pur, 9 tests d'hôte), avec l'état lu à l'attache pour référence :
>   **on n'annonce que les CHANGEMENTS**, sans quoi une application née sans
>   bordure ferait entrer sa fenêtre en plein écran sans raison.

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
  multi-fenêtres. **Sondé le 28/07/2026 (tâche 10 du chantier A) :
  l'activation réussit**, mais c'est une réponse partielle à la question
  posée (spec du chantier A §11, sonde n°4 : « si l'activation réussit **et**
  si des données arrivent »). Après correction d'un bogue de corruption
  mémoire trouvé en revue (`PROPVARIANT` libéré via `CoTaskMemFree` sur une
  adresse de pile — voir `docs/superpowers/plans/2026-07-28-audio-resultats.md`
  §5 pour le détail), `agent::wasapi::probe_process_loopback` obtient un
  `IAudioClient` pour un PID donné sur cette VM (build 20348), reproduit deux
  fois. **Ce qui est acquis** : Windows accepte d'activer une interface
  `AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK` sur ce build — l'hypothèse
  d'une indisponibilité de l'API est écartée. **Ce qui reste ouvert** : la
  sonde n'initialise jamais ce client (`Initialize`, `GetService`,
  `IAudioCaptureClient::Start`) et ne tente aucune capture — savoir si un
  octet réel est capturable pour ce processus reste à vérifier **au chantier
  D**, qui devra construire au-delà de cette sonde d'activation.
- **Encodage** : Media Foundation n'expose pas d'encodeur Opus. Passer par les
  bindings libopus (crate `opus` ou `audiopus`). 48 kHz stéréo, trames de 10 ms,
  ~128 kbps, mode `RESTRICTED_LOWDELAY`, FEC in-band et DTX activés.
- **Transport** : le PT 111 n'apparaît nulle part en production — seulement
  dans deux fixtures de test (`agent/src/transport.rs`) — et la négociation ne
  passe pas par `sdp_api()`. C'est `Rtc::builder().enable_opus(true)` qui
  déclare le codec Opus ; sans cet appel, aucun type de charge utile Opus
  n'est proposé et la piste ne se négocie jamais. C'est exactement l'erreur
  que la spec du chantier A (§7) a corrigée pour elle-même — cette même
  affirmation fausse était restée ici, dans le document qui alimente ce
  chantier D.
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

> 🧪 **Sous-bloc D1 exécuté et éprouvé le 1ᵉʳ août 2026 — résultats :
> `plans/2026-08-01-multifenetres-tranche-verticale-resultats.md`.** Première
> exécution réelle du modèle, et donc **premier verdict qui ne vienne ni d'un
> banc ni du compilateur**.
>
> **Ce que D1 a réglé, et qui est vérifié en session réelle** : le superviseur
> détecte les fenêtres Windows, crée une sortie virtuelle par fenêtre **à la
> taille annoncée par le navigateur**, y pose la fenêtre, lance un enfant qui la
> capture, et la page-shell ouvre **une fenêtre navigateur par fenêtre
> Windows** ; chacune affiche **son** application et elle seule, plein cadre,
> à 1280×720. **Vérifié jusqu'à quatre fenêtres simultanées**, sur de vraies
> applications (Bloc-notes, Explorateur, Firefox) et non des mires — mais
> ⚠️ **ces quatre fenêtres PRÉEXISTAIENT au démarrage du superviseur** : le cas
> produit, un utilisateur qui ouvre une application, a été tenté deux fois et a
> échoué deux fois. L'audio est
> bien porté par **une seule** fenêtre. Aucune sortie virtuelle n'a fuité, sur
> trois contrôles depuis un processus neuf.
>
> **Ce qui reste à régler, et qui bloque la démonstration bout en bout** :
> **créer une sortie virtuelle fait abandonner le mutex des duplications DXGI
> déjà ouvertes** (`0x887A0026`), donc **toute nouvelle fenêtre tue toutes les
> sessions en cours** — reproduit sur **trois exécutions versées sur trois**,
> plus une quatrième dont les journaux ne sont pas joints. *(La destruction
> d'une sortie n'est, elle, pas mise en cause : le cas n'a pas été exercé.)* Trois défauts de
> moindre portée l'accompagnent : l'index `(adaptateur, sortie)` est positionnel
> et n'est pas un identifiant utilisable pour désigner une sortie à un enfant ;
> le chemin de redimensionnement de l'enfant ignore le mode « sortie DXGI
> entière » et retombe sur une capture du bureau ; une hauteur de viewport
> impaire — le cas banal — rend l'appariement impossible.
>
> **Ce que D1 n'a PAS relevé alors qu'il le devait** : le **plafond d'encodeurs
> en multi-processus** (voir la puce « Budget encodeurs » et le §9 de la
> conception D1). Il n'a pas été approché — 4 encodeurs NVENC construits de
> front dans 4 processus, aucun refus, 6 sorties virtuelles attachées
> simultanément sans refus du pilote non plus. **La question reste donc
> entièrement ouverte.** De même, l'injection **clavier** n'est ni démontrée ni
> réfutée, et rien de la latence ni de la cadence n'a été mesuré.

> 🔁 **Sous-bloc D2 exécuté le 1ᵉʳ août 2026 — résultats :
> `plans/2026-08-01-multifenetres-arrangement-dynamique-resultats.md`.** Il ne
> fait qu'une chose : lever ce qui empêchait D1 d'être reçu. **Son verdict est
> double, et les deux moitiés valent ensemble.**
>
> **① Le défaut bloquant ci-dessus est réparé, et démontré réparé en conditions
> de produit.** « Toute nouvelle fenêtre tue toutes les sessions en cours » **ne
> décrit plus le dépôt** : l'abandon du mutex se produit toujours — il n'est ni
> évité ni expliqué — mais la duplication est **relâchée puis rouverte** dans une
> fenêtre de reprise bornée en durée. Relevé : **44** pertes d'accès
> `0x887A0026` encaissées, **aucune session perdue**, montées 1→2, 2→3, 3→4
> propres, **une seule** clôture de session et elle était **sollicitée**.
> **La réserve « ces quatre fenêtres PRÉEXISTAIENT au démarrage du
> superviseur » est donc LEVÉE** : les fenêtres de D2 sont ouvertes pendant que
> d'autres capturent. Les
> **trois défauts de moindre portée** énumérés ci-dessus sont corrigés eux aussi
> (nom DXGI au lieu de l'index positionnel ; garde `sur_sortie` dans `resize` ;
> viewport arrondi en pair et appariement tolérant à 4 px). **Et la destruction
> d'une sortie a été exercée** : elle abandonne le mutex elle aussi, et la
> reprise l'encaisse — la parenthèse « le cas n'a pas été exercé » est caduque.
>
> ⚠️ **② Le critère de D2 exigeait cinq fenêtres simultanées ; on en atteint
> quatre.** La 5ᵉ duplication DXGI, **dans un 5ᵉ processus**, est refusée en
> `0x887A0022`, par une limite de **concurrence** qui **résiste à trois secondes
> de patience explicite**. **La couche qui l'impose n'est pas identifiée** et
> **rien n'établit que 4 soit une borne du système**. **D2 n'est donc pas reçu
> au sens de son critère**, et répare pourtant ce pour quoi il existait.
>
> **Ce que D2 n'a toujours pas relevé** : le **plafond d'encodeurs en
> multi-processus** reste entièrement ouvert — la mort survient à l'ouverture de
> la duplication, **avant tout encodeur**, et le maximum construit de front reste
> **4**, dans 4 processus. Rien de la latence ni de la cadence non plus.
> **L'injection clavier, elle, est démontrée** : `SetForegroundWindow` avant
> injection (4 succès, 0 refus) et quatre fenêtres recevant **chacune sa propre
> frappe** — ⚠️ portée exacte : une frappe par fenêtre, sonde **séquentielle**,
> aucune frappe concurrente, et **`SendInput` reste global à la session
> Windows**.
>
> **Une exécution par rang au banc, une exécution exploitée par configuration à
> la recette : aucun taux, nulle part.**

> 🔢 **Sous-bloc D3 exécuté le 2 août 2026 — résultats :
> `plans/2026-08-02-multifenetres-plafond-concurrence-resultats.md`.** Il prend
> trois des points que D2 laissait, et **ses deux critères sont tenus**.
>
> **① La sortie virtuelle est RETENUE d'une relance à l'autre.** Le superviseur
> ne détruit plus puis ne recrée plus une sortie à chaque relance d'enfant —
> c'était la vraie cause des réouvertures parasites infligées aux sessions
> **saines**. Relevé sur une fenêtre bornée, une fenêtre étant condamnée à
> répétition pendant que trois autres capturent : **0** réouverture imputable à
> une relance, **0** session saine perdue, **une seule** création et **une
> seule** destruction de sortie encadrant **quatre** lancements. La **fuite de
> capacité** (fenêtre neuve dont la page-shell ne répond jamais) est fermée par
> la même occasion.
>
> **② Le plafond de quatre est CARACTÉRISÉ : il porte sur le nombre de
> PROCESSUS concurrents tenant une duplication DXGI, et vaut exactement 4.**
> Campagne de 15 exécutions (5 rangs × 3), sondes minimales : **8** duplications
> passent, qu'elles soient réparties sur 1, 2 ou 4 processus ; le refus tombe au
> **5ᵉ processus**, en `0x887A0022`, 3/3. **Le fait décisif** : le rang qui
> échoue n'a que **4** duplications ouvertes au moment du refus, quand des rangs
> qui réussissent en ont **8** — le nombre de duplications est **positivement
> exclu** comme cause, et deux rangs créant le même nombre de sorties
> virtuelles ne diffèrent que par le nombre de processus.
> ⚠️ **La couche qui impose ce plafond n'est TOUJOURS pas identifiée** (Windows,
> DXGI, pilote NVIDIA, SudoVDA, virtualisation), et **rien n'établit que 4 soit
> une borne du système**.
>
> **Décision d'arrangement, prise selon une règle écrite AVANT la mesure** : la
> contrainte étant le nombre de processus, la **capture mutualisée** — un seul
> processus tenant les N duplications et distribuant les textures — est
> **DÉSIGNÉE pour D4** et **non implémentée** par D3. En conséquence, la
> capacité du superviseur passe de 8 à **4** : **valeur mesurée sur cette VM,
> non prouvée être une borne du système.** **La cible de huit fenêtres de ce
> chantier reste donc hors de portée tant que la capture n'est pas
> mutualisée.**
>
> **Ce que D3 n'a PAS relevé** : le **plafond d'encodeurs en multi-processus**
> — les sondes ne construisent **aucun** encodeur et ne capturent **aucune**
> image ; il **devient le risque n°1 de D4**. Rien de la latence, de la cadence
> ni de la durée. Aucune image comptée pendant la recette du critère 1, dont
> **une seule exécution** est retenue (la campagne, elle, répète 3 fois par
> rang).
>
> **Acquis d'outillage qui dépasse ce sous-bloc** : `cargo check --target
> x86_64-pc-windows-gnu` depuis `agent/` compile désormais le code
> `#[cfg(windows)]` sur l'hôte Linux (mingw-w64). ⚠️ Couvre types, emprunts,
> visibilités et durées de vie ; **ne couvre PAS l'édition de liens**, la cible
> réelle étant `msvc` sur la VM.

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
- **Budget encodeurs — 8, mesuré le 30/07/2026 puis reconfirmé le 31/07/2026,
  formulation bornée.** À 1280×720 / 60 i/s / 8 Mb/s (Baseline, CBR, NV12),
  **8 instances du pipeline Media Foundation complet réussissent, la 9ᵉ est
  refusée — que les encodeurs partagent un unique périphérique D3D11 ou qu'ils
  en aient chacun un neuf.** Séparer les périphériques ne fait donc gagner
  aucune fenêtre : **le partage n'était pas la contrainte**, et le coût
  correspondant (partager des textures entre le périphérique de capture et ceux
  des encodeurs) n'a pas à être payé.

  **Attribution corrigée** : le refus est rendu par **`SetOutputType` de la MFT
  NVIDIA** (`MF_E_UNSUPPORTED_D3D_TYPE`, `0xC00D6D76`), et non par « la liaison
  du type d'entrée » comme l'énonçait la version précédente de cette puce. Le
  libellé Windows de ce HRESULT parle du type d'**entrée** alors que l'appel
  refusé règle la **sortie** — **ne pas se fier au texte d'un HRESULT pour
  désigner un appel**.

  **Ce que la mesure n'établit pas**, et qu'il ne faut donc pas écrire : ce n'est
  pas « la limite de sessions NVENC de cette carte » (le transform matériel n°9
  s'instancie sans difficulté) ; **la couche qui impose le plafond n'est pas
  identifiée** (NVENC, pilote NVIDIA, Media Foundation, ou virtualisation) ; rien
  ne dit qu'il tienne à d'autres résolutions ou débits ; et **aucune image n'a
  été soumise** — seule la *construction* est mesurée, pas la tenue en cadence de
  8 flux ensemble.
  ✅ **Sur ce dernier point (31/07/2026) : 8 flux ensemble tiennent bien la
  cadence** — 90,1 i/s par fenêtre en capture+encodage, zéro verdict faux
  (`plans/2026-07-31-duplications-paralleles-resultats.md` §3.2), mais sur huit
  périphériques D3D11 **distincts** et à 1280×720/60/8 Mb/s, une exécution par
  rang. Le montage de la mesure ② — périphérique unique partagé — n'a, lui,
  toujours pas été alimenté, et **les autres réserves de ce paragraphe restent
  entières**. Réserve de méthode : la comparaison partagé/séparé porte sur
  **deux variables confondues**, le mode séparé n'ouvrant aucune duplication
  DXGI ; le témoin propre n'a pas été exercé.

  **Conséquence** : suspendre l'encodage des fenêtres masquées (Page Visibility
  API côté client) passe d'optimisation souhaitable à **condition de viabilité**
  au-delà de huit fenêtres. Le mécanisme reste à choisir : **que détruire un
  encodeur libère la place est une conjecture non éprouvée** — la séquence
  « créer 8 → en détruire 1 → tenter un 9ᵉ » n'a jamais été jouée.
  Journaux : `plans/journaux-mesures-prealables/nvenc-partage-temoin.log`,
  `nvenc-separe.log`.

  ⚠️ **Ce chiffre de 8 reste un chiffre de PROCESSUS UNIQUE.** Le sous-bloc D1,
  qui devait le relever en multi-processus (un encodeur par processus enfant),
  **n'y est pas parvenu** : ses sessions meurent avant d'atteindre le rang utile
  (voir l'encadré D1 en tête de ce chantier). Ce qui est observé en
  multi-processus le 1ᵉʳ août 2026 : **4 encodeurs NVENC construits de front
  dans 4 processus distincts, sans un seul refus**. **La question « par
  processus ou global ? » reste donc entièrement ouverte**, et rien n'autorise à
  transposer le 8 tel quel.
  ⚠️ **Toujours ouverte après le sous-bloc D2, mais pour une AUTRE raison** : les
  sessions ne meurent plus (le défaut central est réparé) ; c'est désormais la
  **5ᵉ duplication DXGI** qui est refusée, **avant tout encodeur**. Le maximum
  d'encodeurs construits de front reste donc **4**, dans 4 processus, sans refus
  — et **ce n'est toujours pas une limite d'encodeurs qui l'arrête**. Voir
  l'encadré D2 en tête de ce chantier.
  ⚠️ **Et TOUJOURS ouverte après le sous-bloc D3.** Ses sondes sont
  **minimales** : elles ouvrent des duplications nues et ne construisent
  **aucun** encodeur. D3 identifie ce qui bornait à quatre — le nombre de
  processus tenant une duplication —, ce qui **retire l'obstacle** qui empêchait
  d'atteindre le rang utile, mais **ne relève toujours pas ce plafond-ci**.
  La voie désignée pour D4 (mutualiser la capture) **déplace précisément le
  risque ici** : ce plafond **devient le risque n°1 de D4**.
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
- **Capture** : la décision ouverte du §4 a été sondée le 30/07/2026 — WGC et le
  non-recouvrement garanti sont **tous deux démentis par la mesure**, voie
  recommandée « un moniteur virtuel par fenêtre », repli mesuré `PrintWindow`.
  Voir §4 et `plans/2026-07-30-sonde-capture-multifenetre-resultats.md`.

  **Les deux mesures qui bloquaient la spécification de ce chantier sont
  prises** (31/07/2026, `plans/2026-07-31-mesures-prealables-chantier-d-resultats.md`) :

  - **Plafond de sorties virtuelles = 10**, refus du pilote à la 11ᵉ création
    (`ERROR_TOO_MANY_NAMES`), preuve par identité des onze sorties présentes au
    refus. Pour une cible de 8 fenêtres, **la voie tient avec 2 de marge** — mais
    le vivier est peut-être partagé avec Apollo, qui n'en consommait aucune
    pendant la mesure. Le produit **commande désormais le pilote lui-même** (IOCTL
    SudoVDA), sans dépendre du démarrage d'une session Apollo, et sait purger ses
    sorties orphelines.
  - **Plafond d'encodage sur périphériques séparés : inchangé, 8** — voir la puce
    « Budget encodeurs ».
  - **L'hypothèse fondatrice de la voie est vérifiée** : Windows compose bien des
    fenêtres sur un moniteur virtuel **sans écran physique**, et Desktop
    Duplication en rend l'image exacte (900/900 verdicts justes, zéro image
    noire, 90,0 i/s par fenêtre). Elle n'était jusqu'ici garantie que « par
    construction ».

  **La mesure qui restait due est prise** (31/07/2026,
  `plans/2026-07-31-duplications-paralleles-resultats.md`) : N duplications DXGI
  **de front** sur N sorties virtuelles — l'arrangement que la voie propose
  réellement, que rien n'avait exercé jusque-là (le banc posait N fenêtres sur
  **une** sortie, et DXGI n'autorise qu'une duplication par sortie).

  **Reçue.** Aux quatre rangs N = 1, 2, 4, 8, une sortie virtuelle par fenêtre à
  1280×720, une duplication et un encodeur par sortie : **90,1 i/s par fenêtre en
  capture+encodage**, identique aux quinze voies des quatre rangs, avec **zéro
  verdict faux**. À N=8 — le rang du critère de réception, qui exigeait ≥ 60 i/s
  et aucun verdict faux — c'est **1,50 fois le seuil**. Et, pour la première fois
  sur ce projet, **l'aire totale croît avec N** (0,92 → 7,37 Mpx de N=1 à N=8,
  contre une aire fixe sur tous les bancs antérieurs) **sans que la cadence par
  fenêtre bouge** — le débit de pixels correspondant, **calculé**, va de 83,0 à
  664,3 MP/s. *(Ce constat vaut à l'intérieur de cette série et n'est rapproché
  d'aucune autre : les bancs à aire fixe ne lui sont commensurables ni sur les
  cadences ni sur les débits, qui en dérivent.)*

  **Portée exacte, à ne pas élargir** : **une exécution par rang, donc aucun
  taux** ; **rien au-delà de 8 sorties — 8 est ce qui a été demandé et obtenu,
  pas une limite trouvée** ; rien de la latence ; justesse **échantillonnée**
  (contrôle en rotation, ~113 lectures par voie à N=8, pas 901) ; débits de
  pixels **calculés**, non relevés ; mires D3D11 plein cadre et **non des
  applications réelles** ; aucune unité H.264 décodée ; et rien du comportement
  quand Apollo consomme le même vivier de 10.

  **Le repli `PrintWindow` recule** : mesuré à N=4 et N=8, il rend 17,6 puis
  **8,8 i/s par fenêtre**. Recollé aux deux rangs de la sonde, son débit de
  pixels décroît **sur les quatre rangs sans palier** — 116,64 → 75,43 → 45,62
  → 20,28 MP/s, une division par 5,8 — là où `duplication` restait quasi
  constante. Il ne tient pas la cible de 8 fenêtres ; il reste un repli **pour
  deux à quatre fenêtres** et pour les cas où la voie principale ne s'applique
  pas. Réserve : le chiffre est un plancher de l'implémentation actuelle, qui
  réalloue ses ressources GDI à chaque image.

  **Le défaut de libération des encodeurs est diagnostiqué et corrigé**
  (31/07/2026, même document, §7). Il n'était **pas déterministe** comme les
  documents le décrivaient, mais **intermittent** : 2 plantages sur 6 exécutions
  du cas comparable. La faute : la MFT NVIDIA a encore un élément de travail en
  vol quand on relâche l'encodeur, et cet élément entre dans un verrou qui
  n'existe plus — pile symbolisée, deux fois identique, sur un fil de pool.
  `MFShutdown`, d'abord accusé, a été **réfuté par la mesure** : retiré
  entièrement du chemin, la faute revient. Correctif : une file de travail
  Media Foundation **sérialisée par encodeur** imposée à la MFT
  (`IMFRealTimeClientEx::SetWorkQueueEx`), avec dépôt d'une sentinelle avant
  tout relâchement — **0 récidive sur 20 exécutions contre 2 sur 6**, ce qui
  **n'est pas une preuve d'absence**.

  **Fait acquis, réutilisable par ce chantier — à ne pas redécouvrir** :
  `IMFShutdown::Shutdown` rendant `MFSHUTDOWN_COMPLETED` **ne prouve pas**
  l'absence d'élément de travail en vol concernant la MFT. Mesuré : la MFT rend
  cet état en `attente_ms=0` et la faute revient quand même (1 récidive sur 10).
  Toute logique de fermeture de fenêtre qui s'appuierait sur cette confirmation
  seule serait fausse — c'est le **couple** arrêt + barrière sur file sérialisée
  qui traite le cas, et chacun des deux retiré séparément laisse la faute
  revenir.

  **Deux risques restent ouverts et assumés** : `IMFShutdown::Shutdown` est non
  borné dans un `Drop` et un gel y a été **observé** (1 fois sur 6 à N=4, cause
  non attribuée) — le retirer n'est pas une option, sans lui la faute revient
  2 fois sur 5. ⚠️ **Et ce risque est PRÉSENT, pas propre à ce chantier** :
  `Drop for H264Encoder` court déjà en production mono-fenêtre, à chaque
  changement de barreau de l'adaptation réseau (`set_encode_size`) et à chaque
  redimensionnement (`resize`), sur le fil unique de `Session::run` — un gel y
  figerait la session entière. Partie bornée du pire cas : **8 s** par
  destruction d'encodeur (6 s sur les machines éprouvées) ; **le total n'est
  borné par rien**. La mitigation est l'observabilité : les deux traces qui
  encadrent l'appel sont en `info!`, **ne pas les redescendre en `debug!`**.
  Enfin, le convertisseur de couleur n'est couvert par rien, ce qui
  ne coûte rien tant qu'il retombe sur une MFT synchrone, mais **serait une MFT
  matérielle sans barrière sur un hôte doté d'un Video Processor matériel** —
  configuration qu'aucune machine éprouvée n'expose, donc non mesurée. Enfin,
  **le cas d'exploitation réel n'est pas couvert** : le banc détruit ses
  encodeurs d'affilée à la fin, jamais un seul pendant que les autres encodent —
  ce que fera pourtant la fermeture d'une fenêtre.

---

## 6. Prérequis propres aux jeux

- **Plein écran fenêtré imposé.** Le plein écran exclusif court-circuite le
  compositeur et complique la capture. Parsec impose la même contrainte.
- **Session interactive.** Steam et les jeux exigent la session 1 ; l'agent y
  tourne déjà (contrainte de frontière de session du jalon 1). Acquis.
- **Affichage virtuel.** Le « SudoMaker Virtual Display Adapter » doit annoncer
  les résolutions et fréquences de rafraîchissement visées. **Relevé le
  30/07/2026** : le pilote existe et est sain (`ROOT\DISPLAY\0003`) mais
  **n'expose aucune sortie DXGI au repos** — il n'en produit une (`\\.\DISPLAY5`,
  3413×960 mesurés — relevé sans journal joint, session Apollo non
  reproductible sans le propriétaire du poste) que pendant une session de
  streaming Apollo, et **en remplacement** de l'écran physique
  (`dd_configuration_option = ensure_only_display`). Piège : le champ de
  résolution de WMI s'est révélé périmé de 68 s ; la source de vérité est
  `GetDesc`/`DesktopCoordinates`.

  **Mis à jour le 31/07/2026 — la dépendance à Apollo est levée.** Le canal de
  contrôle du pilote est identifié (IOCTL sur le GUID d'interface SudoVDA,
  `plans/journaux-mesures-prealables/canal-de-controle.md`) et notre code crée,
  détruit et purge ses propres sorties : **dix créations et dix destructions
  éprouvées**, sorties DXGI réelles, attachées, portées par l'adaptateur qui
  possède NVENC. La sortie créée par notre code à 1280×720 ne présente **aucun
  facteur d'échelle** (1,0, journal `moniteurs-capture.log`) — mais le piège
  DPI 1,5 s'est bien présenté sur cette VM sur la sortie 5120×1440 d'Apollo
  (**le même relevé sans journal joint que ci-dessus**, non reproductible sans
  le propriétaire du poste) : la borne est donc *cette résolution-là*, pas
  *cette VM*, et le chemin de conversion reste nécessaire.
  Une sortie virtuelle **survit au processus qui l'a créée** :
  une purge autonome est nécessaire, elle existe, et elle a déjà servi en
  conditions réelles. Le pilote porte un **chien de garde d'unité inconnue**
  (`delai = 3`) : aucune unité n'est exclue, pas même la seconde — une
  exploitation durable devra le pinguer.
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
   **Son sous-bloc D1 (tranche verticale) a été construit et exécuté le
   1ᵉʳ août 2026** : une fenêtre navigateur par fenêtre Windows, sur sa propre
   sortie virtuelle, dans son propre processus — **acquis jusqu'à quatre
   fenêtres simultanées, mais seulement pour des fenêtres qui PRÉEXISTAIENT au
   démarrage du superviseur** : **l'arrangement ne survit pas à l'ouverture
   d'une fenêtre de plus** (tenté deux fois, échoué deux fois), et le plafond d'encodeurs en multi-processus n'a pas
   pu être relevé. Détail et suite à donner : encadré D1 du §5 D et
   `plans/2026-08-01-multifenetres-tranche-verticale-resultats.md`.
   ✅ **Son sous-bloc D2 a levé cette restriction le même jour**
   (`plans/2026-08-01-multifenetres-arrangement-dynamique-resultats.md`) :
   **l'arrangement survit désormais à l'ouverture d'une fenêtre de plus**,
   démontré en conditions de produit — 44 pertes d'accès encaissées, aucune
   session perdue, montées 1→2→3→4 propres. **La phrase ci-dessus ne décrit plus
   l'état du dépôt.** ⚠️ **Deux réserves qui, elles, tiennent** : un plafond
   **distinct** arrête la montée à **quatre** fenêtres simultanées (5ᵉ
   duplication DXGI refusée en `0x887A0022` ; **couche non identifiée**, et
   **4 n'est pas prouvé être une borne du système**), et **le plafond
   d'encodeurs en multi-processus n'a toujours pas pu être relevé** — la mort
   survient avant tout encodeur.
   🔢 **Son sous-bloc D3 a caractérisé ce plafond le 2 août 2026**
   (`plans/2026-08-02-multifenetres-plafond-concurrence-resultats.md`) : il
   porte sur le **nombre de processus** concurrents tenant une duplication, et
   vaut **exactement 4** — huit duplications tiennent dès lors qu'elles sont
   réparties sur au plus quatre processus, et le rang qui échoue en a **moins**
   que ceux qui passent. D3 ferme par ailleurs la recréation de sortie à chaque
   relance et la fuite de capacité, et porte la capacité du superviseur de 8 à
   **4**. **Décision d'arrangement** : la **capture mutualisée** (un seul
   processus tenant les N duplications) est **désignée pour D4**, non
   implémentée — **c'est elle qui conditionne la cible de huit fenêtres**.
   ⚠️ **Les deux réserves ci-dessus TIENNENT** : la couche du plafond reste
   **non identifiée**, **4 n'est toujours pas prouvé être une borne du
   système**, et le **plafond d'encodeurs en multi-processus n'a toujours pas
   été relevé** — les sondes de D3 ne construisent aucun encodeur. Il **devient
   le risque n°1 de D4**.

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
