# Sous-bloc D8 — le plein écran et Keyboard Lock

**Date** : 4 août 2026
**Chantier** : D (multi-fenêtres), sous-bloc 8
**Prédécesseur** : D7 (l'audio par fenêtre) —
`docs/superpowers/specs/2026-08-03-multifenetres-audio-par-fenetre-design.md`
**Successeur annoncé** : aucun à ce jour

---

## 1. Objet

Le cadrage jeux (§4.1 de `2026-07-28-support-jeux-design.md`) a décidé que
**Windows est maître du plein écran, dans un sens unique** : quand une
application y passe, la fenêtre navigateur suit ; jamais l'inverse. Le chantier B
n'en a implémenté que le plein écran **demandé par l'utilisateur**, en laissant
explicitement le sens Windows→navigateur au chantier D.

D8 le livre — et livre avec lui les deux autres plans sans lesquels le plein
écran n'est pas utilisable :

| Plan | Ce qui manque aujourd'hui |
| --- | --- |
| **Déclenchement** | L'application passe en plein écran, la fenêtre navigateur ne suit pas |
| **Résolution** | `resize` est sans effet en mode `SortieEntiere` : le plein écran affiche du 720p mis à l'échelle |
| **Échap** | Keyboard Lock est codé et câblé depuis le chantier B, mais **jamais éprouvé** — or le §4.1 le nomme lui-même « condition de viabilité » |

**Hors périmètre, par décision** :

- le **plein écran exclusif D3D** — le cadrage impose déjà le plein écran
  fenêtré (`support-jeux-design.md:751`), et l'exclusif court-circuite le
  compositeur ;
- le sens **navigateur→Windows** : le navigateur ne force jamais l'état de la
  fenêtre Windows. C'est ce qui rend toute oscillation impossible, et c'est le
  principal mérite du choix du §4.1 ;
- les legs de D6 et D7 **autres que le critère ③** (l'audio d'une fenêtre
  endormie), ramassé ici parce que la recette de D8 endort des fenêtres par
  construction. Restent donc dus : le signal enfant→capteur de F3, l'identité de
  session par génération monotone (F5), les compteurs par session de D6, le
  critère ④ de D6 à palier long, et l'A/B différentiel sur
  `set_desired_bitrate`.

## 2. État des lieux, vérifié

| Fait | Pièce |
| --- | --- |
| Le plein écran client et Keyboard Lock existent et sont câblés | `client/src/fullscreen.ts`, bouton `#fullscreen` de `client/index.html:13`, `attachFullscreenAuDOM` à `client/src/main.ts:189` |
| La fenêtre Windows occupe **exactement** sa sortie virtuelle, et le superviseur le lui réimpose périodiquement | `agent/src/superviseur/placement.rs:112` (`poser`), `agent/src/superviseur/boucle.rs:218` (`controler_le_placement`) |
| `resize` est **sans effet** sur une source en mode `SortieEntiere` | `agent/src/capteur/fenetre/commandes.rs:144-146` |
| Le client émet **déjà** un `Resize` à l'entrée en plein écran | `client/src/main.ts:264` — le `ResizeObserver` observe l'élément vidéo, dont la taille change |
| Le pilote SudoVDA n'expose **aucun** changement de mode | `docs/superpowers/plans/journaux-mesures-prealables/canal-de-controle.md:230-235` — six IOCTL : `ADD`, `REMOVE`, `SET_RENDER_ADAPTER`, `GET_WATCHDOG`, `PING`, `GET_PROTOCOL_VERSION` |
| Le capteur pousse déjà des messages non sollicités à l'enfant, au changement seulement | `agent/src/capteur/protocole.rs:79-115` — `Sommeil`, `Part`, `Audio` |
| L'hôte de recette n'a **ni serveur X, ni `xdotool`, ni `Xvfb`** | Relevé le 4 août 2026 : `DISPLAY` vide, les trois binaires absents, seul `/usr/bin/google-chrome` présent |

### ❌ Le critère de détection du cadrage est structurellement mort

Le §4.1 prescrit la « comparaison de `GetWindowRect` avec le rect du moniteur ».
Dans l'architecture née de D1, **chaque fenêtre est seule sur sa propre sortie
virtuelle et l'occupe exactement** : « rect fenêtre == rect moniteur » est l'état
**nominal**, pas l'état plein écran. Ce critère ne peut rien distinguer.

Il n'est pas seulement inopérant : il est **toujours vrai**. Une implémentation
fidèle à la lettre du cadrage annoncerait donc le plein écran en permanence, pour
toutes les fenêtres. **Le signal survivant est la perte des styles de bordure**,
pas le rectangle.

### Ce que le troisième signal du cadrage ne peut pas faire

Le §4.1 cite aussi `SHQueryUserNotificationState()` rendant
`QUNS_RUNNING_D3D_FULL_SCREEN`. Il est **global à la session interactive, pas par
fenêtre** : à N fenêtres il ne dit pas *laquelle*, et il ne voit pas le
« borderless fullscreen » que la quasi-totalité des jeux modernes emploient.
Écarté.

## 3. Décisions actées

| Décision | Choix | Justification |
| --- | --- | --- |
| Périmètre | **Déclenchement + résolution + Échap** | Les trois plans du plein écran ; deux sur trois ne livrent rien d'observable |
| Signal de détection | **Perte des styles de bordure** | Par fenêtre donc non ambigu ; prédicat pur donc testable sur l'hôte ; couvre le borderless |
| Où vit la détection | **Le fil de fenêtre du capteur** | Il tient déjà le `HWND`, un span `tracing` porteur de `session` (D7), et un canal poussé vers l'enfant |
| Résolution | **Changement de mode de la sortie existante** | La sortie garde son nom, son attache et sa place dans la table : rien du protocole n'est à remanier, et la rétention acquise en D3 est préservée |
| Repli si le mode ne change pas | **Accepter l'upscale, et le documenter** | Coût nul, et cohérent avec la mesure de D6 : le goulot est le décodeur du navigateur |
| Voisines | **Rien de neuf — le vivier de D5 suffit** | Le plein écran les occulte, elles se déclarent cachées, le vivier les endort. Une seconde autorité sur le sommeil serait deux décideurs pour un même état |
| Déclenchement client | **Armement sur le prochain geste** | `requestFullscreen()` exige une activation transitoire qu'un message de canal de données ne fournit pas. Même mécanisme que celui validé pour `window.open()` |
| Instrument de recette | **Sondé avant d'être employé** | Ce dépôt a payé deux fois de suite, sur le seul `grep` de D7, la règle « vérifier qu'un contrôle **peut** échouer avant de s'y fier » |

## 4. Trois portes de mesure, avant toute ligne de code

Chacune a son repli **écrit d'avance**. P0 et P2 sont bon marché et se jouent en
premier ; P1 exige la VM et un binaire d'agent.

### P0 — le `grep` de D7, qui est la première mesure de F1

C'est le geste d'ouverture que D7 prescrit, et **la seule mesure jamais prise du
correctif F1** — lequel repose aujourd'hui sur un argument de flot de contrôle,
pas sur une observation.

```bash
sed 's/\x1b\[[0-9;]*m//g' agent.log > agent-plat.log
grep -c 'compteurs audio' agent-plat.log                      # A
grep 'compteurs audio' agent-plat.log | grep -c 'actif=true'  # B
```

**Et seulement sur une session vivante depuis au moins 30 s** (`REPORT_INTERVAL`
de `agent/src/windows_audio.rs`) : sous cette durée, `A = 0` ne dit rien.

| Relevé | Lecture |
| --- | --- |
| `A = 0` | Le contrôle n'a rien à dire : mesure non prise |
| `A > 0`, `B = 0` | Des fenêtres vivent, **aucune ne porte le son** — le défaut est là |
| `A > 0`, `B < A` | **F1 est confirmé** : une fenêtre muette rapporte bien `actif=false` |
| `B == A` avec plusieurs fenêtres d'un même PID | ⚠️ Signature exacte du code d'**avant** F1 : F1 est réfuté et se traite avant D8 |

### P1 — une sortie SudoVDA annonce-t-elle d'autres modes ?

`EnumDisplaySettingsExW` sur `\\.\DISPLAYn`, puis un `ChangeDisplaySettingsExW`
**réel** vers une taille supérieure, et relecture par `GetDesc`/
`DesktopCoordinates` — jamais WMI, dont le champ a été vu périmé de 68 s.

**L'énumération ne suffit pas** : un pilote peut annoncer un mode et le refuser,
comme il peut accepter un mode non énuméré. C'est le changement effectif qui
tranche.

- **Si oui** → le §6 s'applique.
- **Si non** → repli acté : `resize` reste un no-op, le plein écran affiche
  l'image native mise à l'échelle, et c'est une **limite documentée, pas
  corrigée** — comme le cadrage le fait déjà pour Keyboard Lock hors Chromium.
  Les critères ①, ③, ④ et ⑤ tiennent sans ②.

### P2 — l'instrument sait-il entrer en plein écran ?

Deux questions, sur l'hôte, hors de toute session d'agent :

1. Chrome sans interface, piloté par CDP, accorde-t-il l'activation utilisateur
   transitoire nécessaire à `requestFullscreen()` ? *(D1 a relevé qu'un clic CDP
   n'en fournissait pas une pour `requestPointerLock` — sans que la cause soit
   isolée.)*
2. Expose-t-il `navigator.keyboard.lock()` ?

- **Si oui** → la recette est entièrement automatisable, sur le montage des sept
  campagnes précédentes.
- **Si non** → deux voies, **et le choix se fait sur la mesure, pas d'avance** :
  installer `Xvfb` + `xdotool` (de vrais événements X11, donc une activation
  authentique par construction), ou déclarer le critère ③ non éprouvé.
  ⚠️ **Si `Xvfb` est installé, les mesures ne se comparent à AUCUNE campagne
  antérieure**, et il faudra le dire.

## 5. Détection et protocole

### 5.1 Le prédicat, pur

Module neuf `agent/src/capteur/plein_ecran.rs`, **sans aucun `cfg`**, testé sur
l'hôte — même forme que `capteur/audio.rs` (D7) et `capteur/repartiteur.rs` (D6) :

```rust
/// Vrai si le style ne porte ni barre de titre ni cadre redimensionnable.
pub fn est_sans_bordure(style: u32) -> bool
```

Seule la lecture `GetWindowLongPtrW(hwnd, GWL_STYLE)` est `#[cfg(windows)]`.

### 5.2 L'état de référence, et la garde contre les faux positifs

**L'état lu à l'attache fait référence, et on n'annonce que les *changements*.**
Une application née sans bordure n'annonce donc rien, et ne fait pas entrer sa
fenêtre navigateur en plein écran sans raison. La garde est gratuite et suffit :
le critère de sélection des fenêtres du superviseur (« Alt-Tab-able ») exclut
déjà en amont les fenêtres outils et les popups.

### 5.3 Cadence, et absence d'hystérésis

La lecture se fait sur le **fil de fenêtre**, **bridée par son propre minuteur** à
`PERIODE_STYLE = 250 ms`, et **jamais à l'image** : un appel local est bon
marché, pas gratuit à N × 90 i/s.

⚠️ **Ce n'est pas le tour de roue de `PERIODE_REARBITRAGE`**, qui vit sur le fil
de sommeil (`agent/src/capteur/sommeil.rs:34`) et n'a pas les `HWND`. La valeur
est du même ordre, délibérément, mais la constante est **propre à
`plein_ecran.rs`** : les faire suivre l'une l'autre coupleraient deux mécanismes
que rien ne lie.

**Pas d'hystérésis, et c'est un choix écrit** : une bascule de style est atomique,
aucun rebond n'est attendu. Si la recette en montre un, on l'ajoute **alors** —
pas avant.

### 5.4 Les deux messages

```rust
// agent/src/capteur/protocole.rs, à côté de Sommeil, Part et Audio
DepuisCapteur::PleinEcran { actif: bool }
```

Poussé **au changement seulement**, et **distinct d'`Etat`** pour la même raison
que ses trois voisins : `Etat` alimente un cache lu à chaque tour de la boucle de
transport, et y mêler une annonce ponctuelle passerait par un chemin conçu pour
un état permanent.

```rust
// proto/src/control.rs, relayé par l'enfant sur le canal de données
AgentControl::Fullscreen { v: u8, active: bool }
```

Exactement le trajet que `Sommeil` emprunte pour devenir `Asleep`.

## 6. La résolution qui suit

**Aucun message neuf n'est nécessaire.** Le client émet déjà un `Resize` en
entrant en plein écran (`ResizeObserver` sur l'élément vidéo), le message
descend déjà jusqu'à `capteur/fenetre/commandes.rs:154`, et il s'y arrête sur un
no-op. D8 rend ce no-op agissant, **en mode `SortieEntiere` seulement** :

1. `ChangeDisplaySettingsExW` sur `\\.\DISPLAYn` — la sortie **garde son nom**,
   son attache capteur→enfant et sa place dans la table ;
2. `placement::poser` remet la fenêtre à la nouvelle taille ;
   ✅ **Le superviseur ne combattra pas ce placement, et c'est vérifié** :
   `replacer_si_besoin` (`agent/src/superviseur/boucle/placement_periodique.rs:36`)
   **réénumère les sorties DXGI à chaque contrôle** et les résout par **nom**,
   puis pose sur le `rect` fraîchement lu — il n'existe aucun rectangle mémorisé
   qui deviendrait périmé. Le superviseur *réparerait* même le placement de
   lui-même, au pire `PERIODE_PLACEMENT` plus tard ; l'appel immédiat n'est là
   que pour ne pas laisser une fenêtre mal dimensionnée pendant ce délai ;
3. on rend la taille **obtenue** par `GetDesc`/`DesktopCoordinates` — jamais
   WMI, jamais la taille demandée. C'est la règle que `SourceDistante::resize`
   applique déjà : le pilote quantifie, et une fenêtre Windows impose des
   dimensions paires.

### Trois coûts, écrits plutôt que découverts

- **La duplication DXGI de cette sortie est invalidée** par le changement de
  mode. La reprise de D2 la rouvre ; son coût relevé ailleurs est de **+48 à
  +144 ms**.
- **Les voisines perdent-elles leur mutex ?** La *création* d'une sortie le
  provoquait ; d'un *changement de mode*, **rien n'est su**. La recette le
  **compte**, elle ne le suppose pas.
- **L'encodeur se reconstruit** (`set_encode_size`, qui détruit avant de
  construire depuis D5, donc sans franchir le plafond de 8) et l'échelle de
  congestion se recalibre par `Controleur::changer_source`, déjà câblé par
  `transport/redimensionnement.rs`.

### Un plafond explicite, et non calibré

```rust
/// Taille maximale qu'une sortie virtuelle prendra sur demande de viewport.
const TAILLE_MAX_SORTIE: (u32, u32) = (1920, 1080);
```

Un écran 4K donnerait sinon **9× les pixels de 720p** à un décodeur que D6 a
mesuré saturé dès huit fenêtres de 720p (18,03 % d'images jetées au barreau
plein).

⚠️ **Cette valeur n'est pas calibrée** : c'est un garde-fou posé par prudence,
sans qu'aucun jugement visuel ne l'ait jugée — exactement la lacune que `BPP_MIN`
traîne depuis le chantier C volet 1. Le dire vaut mieux que la faire passer pour
un résultat.

## 7. Le client

### 7.1 Armement, pas action

`requestFullscreen()` exige une activation utilisateur transitoire ; un message
reçu sur canal de données n'en est pas une, et l'appel serait rejeté.

- `Fullscreen { active: true }` → **on mémorise**, et on entre au premier
  `pointerdown` ou `keydown` qui suit. C'est le mécanisme du §4.1, et **le même
  que le spike multi-fenêtres a validé pour `window.open()`** : un seul
  mécanisme pour les deux besoins.
- `Fullscreen { active: false }` → sortie immédiate : `exitFullscreen()` n'exige
  aucune activation.

L'ajout vit dans `client/src/fullscreen.ts` (110 lignes, dépendances déjà
injectées, testable sans DOM), sous la forme d'une fonction `armerPleinEcran`
suivant les conventions du module.

### 7.2 Keyboard Lock : rien à écrire

`attachFullscreen` verrouille déjà sur `fullscreenchange`, **quelle que soit
l'origine de l'entrée**. C'est ce qui rend l'armement gratuit côté clavier — et
c'est aussi pourquoi Keyboard Lock est *mesuré* par D8 sans y être *codé*.

### 7.3 Deux entrées, un seul sens

Le bouton ⛶ reste. Ni lui ni l'armement ne touchent l'état de la fenêtre
Windows : **aucune oscillation n'est possible**, ce qui était le mérite
recherché par le §4.1.

Si l'utilisateur sort du plein écran navigateur alors que l'application reste en
plein écran Windows, l'agent ne réagit pas — le §4.1 l'assume. Le
`ResizeObserver` transmet malgré tout le viewport redevenu petit, donc la sortie
reprend sa taille fenêtrée par le chemin du §6.

## 8. Recette

**Montage** : deux applications distinctes, fenêtres Chrome `--app` sur pages
animées, **un `--user-data-dir` par fenêtre** (sans quoi Chrome rejoint son
instance existante et l'on compte des lancements au lieu de fenêtres — leçon de
D4). N = 3, pour laisser des voisines à observer.

| # | Critère | Ce qui le juge |
| --- | --- | --- |
| ① | **Le sens Windows→navigateur** — une application passe en plein écran, sa fenêtre navigateur y entre au geste suivant, **et elle seule** | `document.fullscreenElement` non nul sur la bonne page, nul sur les autres |
| ② | **La résolution suit** — le flux passe à la taille du viewport plein écran, bornée par `TAILLE_MAX_SORTIE` | `frameWidth`/`frameHeight` de `getStats()`, croisés avec `GetDesc` côté agent. **Et la sortie garde son nom `\\.\DISPLAYn`** — c'est ce qui prouve un changement de mode et non une recréation. ⚠️ Conditionné à **P1** |
| ③ | **Échap atteint le jeu** — appui bref, le plein écran tient et la touche arrive à Windows | `fullscreenElement` toujours non nul, **et** le titre de la fenêtre Windows relu par `WM_GETTEXT` (technique de D1), la page `--app` réécrivant son titre sur `keydown`. Réécrire un titre est sans effet sur le produit depuis D7 : l'identité d'une fenêtre vit dans son **nom de fichier**, pas dans son titre (`92ea675`). ⚠️ Conditionné à **P2** |
| ④ | **Les voisines s'endorment** par le chemin existant, sans code neuf | Les traces `Sommeil` du capteur, et `endormie=true` sur leurs parts. ⚠️ Conditionné au fait **non vérifié** que Chrome déclare `hidden` une fenêtre occultée |
| ⑤ | **L'audio d'une endormie survit** — le legs de D7 | La **fréquence dominante** par `AnalyserNode`, l'instrument de D7. **Jamais un compte d'octets** : D7 a relevé un `bytesReceived` qui croît sur un spectre à −1000 dB |

**Témoin de non-régression**, dans le même protocole : une phase à N fenêtres
sans aucun plein écran, qui doit se comporter comme D7.

⚠️ **Si ④ est réfuté, on le relève et on le documente — on ne code pas une
seconde autorité sur le sommeil.** Deux décideurs pour un même état est
exactement le genre de couplage que ce dépôt a payé ailleurs.

### Contraintes de protocole héritées

- **Pas de capture d'écran CDP pendant une mesure** : elle provoque un `Resize`,
  donc un `SHOW`, donc une session et une sortie de plus (D1). Et toute
  évaluation CDP sur une page portant un flux WebRTC actif doit être **bornée**
  (D2).
- **Vérifier `Get-Process agent` avant chaque exécution** : un agent survit à
  l'hibernation de la VM, et `run-agent.sh` ne le tue pas.
- **Purger les sorties orphelines** (`MULTIFENETRE_VDD_PURGE=1`) entre deux
  exécutions : après un `Stop-Process -Force`, elles survivent (D5).
- **Copier `agent.log` après la fin réelle de l'exécution**, pas à la fin du
  pilote (D4).
- **Toute variable d'environnement neuve doit être ajoutée à
  `scripts/run-agent.sh`**, sinon l'agent démarre sans elle et sans rien
  signaler — piège payé en D1, D2, D4 et D7.
- **Vérifier la survie de la VM après chaque rang** : elle s'hiberne d'elle-même,
  déclencheur non identifié.

## 9. Risques, chacun avec son repli

| Risque | Repli |
| --- | --- |
| **P1 négatif** — la sortie n'annonce ou n'accepte que son mode de création | Acté : on accepte l'upscale et on le documente. ①, ③, ④ et ⑤ tiennent sans ② |
| **P2 négatif** — l'instrument ne sait pas entrer en plein écran | `Xvfb` + `xdotool`, ou ③ déclaré non éprouvé. **Le choix se fait sur la mesure** |
| **P0 réfute F1** | F1 se traite avant D8 : c'est un défaut muet et total, qui ferait lire « tout va bien » dans l'état exact où rien ne va |
| Le changement de mode fait perdre le mutex aux voisines | Encaissé par la reprise de D2 — mais **compté**, pas supposé |
| `est_sans_bordure` faux positif | La garde de l'état de référence à l'attache (§5.2) |
| Une bascule de style rebondit | On ajoute l'hystérésis **alors**, sur la mesure (§5.3) |

## 10. Ce que D8 n'établira pas

- **Aucun taux, nulle part** — une exécution par point, comme les sept
  sous-blocs précédents.
- **Rien du plein écran exclusif D3D**, hors périmètre par décision du cadrage.
- **Rien de Keyboard Lock hors Chromium** : sur Firefox et Safari, Échap cassera
  le plein écran. Limite du §4.1, **à documenter, pas à corriger**.
- **`TAILLE_MAX_SORTIE` restera non calibrée**, et **aucun jugement visuel ne
  sera porté** — la lacune exacte de `BPP_MIN`.
- **Rien de la latence de bout en bout**, que ne mesure toujours aucun sous-bloc
  du chantier D.
- **Rien au-delà de la taille physique de l'écran de l'hôte de recette.**
- **Les trois couches inconnues le resteront** : le plafond de 8 encodeurs,
  celui de 4 processus, et le mécanisme de l'abandon du mutex DXGI.
- **Le chemin d'extinction propre du superviseur** n'aura toujours jamais été
  exercé.
- **Si `Xvfb` est installé, les mesures ne se comparent à aucune campagne
  antérieure.**

## 11. Ce que D8 lègue

Non traité ici, à porter dans le plan du sous-bloc suivant plutôt qu'à
redécouvrir :

1. **Le signal enfant→capteur quand une capture audio meurt** (F3 de D7, hors
   périmètre) : sans lui, la fenêtre voisine n'est jamais promue et le groupe
   reste muet.
2. **L'identité d'une session par génération monotone, pas par son seul nom**
   (F5 de D7, préexistant) : changement de conception du registre.
3. **Les compteurs `TICKS` / `CAPTURED` / `PRODUCED` par session** et leur
   extraction vers `agent/src/windows_source/telemetrie.rs` (D6 n°1) — ce qui
   lève la condition posée sur la dette gelée de `windows_source.rs`.
4. **Le critère ④ de D6 rejoué à palier de 45 à 60 s**, seule façon de savoir si
   la promotion de focus est systématique.
5. **L'A/B différentiel sur `set_desired_bitrate`** (D6 n°4), jamais joué.
