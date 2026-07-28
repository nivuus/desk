# Sondes préalables — chantier B « Input jeu »

Tâche 1 du chantier. Investigation, pas d'implémentation : ce document lève
les quatre inconnues du §11 de `docs/superpowers/specs/2026-07-28-input-jeu-design.md`
qui pouvaient invalider la conception du volet manette et du volet souris
avant que la tâche 10 (manette virtuelle) ou la tâche 12 (souris relative) ne
s'appuient dessus. L'inconnue n°3 du §11 (« le signal curseur masqué
suffit-il ? ») n'est pas traitée ici — elle relève d'une autre tâche du
chantier (T6, sondage du curseur).

VM utilisée : `Windows` (libvirt/QEMU), déjà démarrée au moment de cette
tâche. Build constaté : `10.0.20348.5386`.

---

## Inconnue 1 — ViGEmBus s'installe-t-il sur cette VM ?

### Commande exacte

```bash
virsh list --all
until timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null; do sleep 5; done
set -a && source .env && set +a
node scripts/winrm.js '$PSVersionTable.BuildVersion.ToString()'
```

### Sortie brute

```
 ID   Nom       État
--------------------------------------
 19   Windows   en cours d'exécution

10.0.20348.5386
```

### Installation

`winget` échoue avec une erreur de certificat sur la source `msstore` —
**indépendante de ViGEmBus**, c'est la configuration winget de cette VM qui
est en cause :

```bash
node scripts/winrm.js "winget install --id Nefarius.ViGEmBus --accept-source-agreements --accept-package-agreements 2>&1 | Out-String"
```

```
Échec de la tentative de mise à jour de la source : winget
Échec lors de la recherche de la sourceB : msstore
Une erreur inattendue s'est produite lors de l'exécution de la commande :
0x8a15005e : Le certificat de serveur ne correspond à aucune des valeurs attendues.

Aucun package n'a été trouvé parmi les sources de travail.
```

Repli sur le programme d'installation officiel, comme prévu par le brief —
**mais avec une correction** : l'URL `.../releases/latest/download/ViGEmBus_Setup_x64.exe`
du brief renvoie une 404. Le nom d'asset réel de la dernière release
(`v1.22.0`, vérifié via `gh api repos/nefarius/ViGEmBus/releases/latest`) est
`ViGEmBus_1.22.0_x64_x86_arm64.exe` :

```bash
node scripts/winrm.js "Invoke-WebRequest -Uri 'https://github.com/nefarius/ViGEmBus/releases/latest/download/ViGEmBus_1.22.0_x64_x86_arm64.exe' -OutFile 'C:\temp\ViGEmBus_Setup_x64.exe' -UseBasicParsing; Get-Item 'C:\temp\ViGEmBus_Setup_x64.exe' | Select-Object Name,Length | Format-List | Out-String"
```

```
Name   : ViGEmBus_Setup_x64.exe
Length : 6278576
```

```bash
node scripts/winrm.js "Start-Process -Wait -FilePath 'C:\temp\ViGEmBus_Setup_x64.exe' -ArgumentList '/quiet','/norestart'; 'installation terminee'"
```

```
installation terminee
```

### Vérification du pilote

La commande du brief (`Where-Object { $_.FriendlyName -like '*ViGEm*' }`) ne
renvoie **rien** — pas parce que le pilote est absent, mais parce que son nom
d'affichage réel ne contient pas la chaîne « ViGEm » :

```bash
node scripts/winrm.js "Get-PnpDevice | Where-Object { \$_.FriendlyName -like '*vigem*' -or \$_.FriendlyName -like '*Nefarius*' -or \$_.InstanceId -like '*vigem*' } | Select-Object FriendlyName,Status,Class,InstanceId | Format-List | Out-String"
```

```
FriendlyName : Nefarius Virtual Gamepad Emulation Bus
Status       : OK
Class        : System
InstanceId   : ROOT\SYSTEM\0001
```

### Verdict

**ViGEmBus s'installe et se charge sur cette VM (build 20348), `Status: OK`.**
Deux corrections faites en cours de route : l'installateur se télécharge sous
un autre nom que celui du brief (release `v1.22.0`), et le motif de recherche
`Get-PnpDevice` doit chercher `Nefarius`/`vigem` en minuscule plutôt que
`ViGEm` — le nom d'affichage du pilote ne contient pas cette chaîne.

---

## Inconnue 2 — le rappel de vibration remonte-t-il réellement ?

### L'API réelle de `vigem-client` (livrable principal de cette sonde)

Avant toute compilation sur la VM, l'API a été vérifiée hors ligne via le
code source publié sur docs.rs (`vigem-client` 0.1.4 — `crates.io` refuse les
accès automatisés, `docs.rs/.../src/....rs.html` reste accessible). Le code du
brief s'est révélé **exact à un détail près** ; le voici tel que le
compilateur (sur la VM, `cargo build --release`) l'a accepté sans aucune
correction supplémentaire :

```toml
# Cargo.toml, sous [target.'cfg(windows)'.dependencies]
vigem-client = { version = "0.1", features = ["unstable_xtarget_notification"] }
```

**Écart n°1** : la fonctionnalité de crate `unstable_xtarget_notification`
est **obligatoire**. Sans elle, `Xbox360Wired::request_notification` n'existe
tout simplement pas — pas une erreur à l'exécution, une absence à la
compilation. Le brief ne la mentionnait pas.

Signatures vérifiées (fichier `x360.rs` de la crate) :

```rust
pub struct TargetId { pub vendor: u16, pub product: u16 }
impl TargetId {
    pub const XBOX360_WIRED: TargetId = TargetId { vendor: 0x045E, product: 0x028E };
}

pub struct Client { /* … */ }
impl Client {
    pub fn connect() -> Result<Client, Error>;
    pub fn try_clone(&self) -> Result<Client, Error>;
}

pub struct Xbox360Wired<CL: Borrow<Client>> { /* … */ }
impl<CL: Borrow<Client>> Xbox360Wired<CL> {
    pub fn new(client: CL, id: TargetId) -> Xbox360Wired<CL>;
    pub fn plugin(&mut self) -> Result<(), Error>;
    pub fn unplug(&mut self) -> Result<(), Error>;
    pub fn wait_ready(&mut self) -> Result<(), Error>;
    pub fn update(&mut self, gamepad: &XGamepad) -> Result<(), Error>;
    #[cfg(feature = "unstable_xtarget_notification")]
    pub fn request_notification(&mut self) -> Result<XRequestNotification, Error>;
}

#[repr(C)]
pub struct XGamepad {
    pub buttons: XButtons,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub thumb_lx: i16,
    pub thumb_ly: i16,
    pub thumb_rx: i16,
    pub thumb_ry: i16,
} // dérive Default : XGamepad { ..Default::default() } est valide

// Macro XButtons!(A) / XButtons!(UP|RIGHT|LB|A) — exactement comme le brief.

#[cfg(feature = "unstable_xtarget_notification")]
pub struct XNotification {
    pub large_motor: u8,
    pub small_motor: u8,
    pub led_number: u8,
} // les noms de champs supposés dans le brief (large_motor/small_motor) étaient corrects

#[cfg(feature = "unstable_xtarget_notification")]
pub struct XRequestNotification { /* … */ }
impl XRequestNotification {
    pub fn is_attached(&self) -> bool;
    pub fn spawn_thread<F: FnMut(&XRequestNotification, XNotification) + Send + 'static>(
        self, f: F,
    ) -> std::thread::JoinHandle<()>;
    // Bas niveau, non utilisé par la sonde : request(Pin<&mut Self>),
    // poll(Pin<&mut Self>, wait: bool) -> Result<Option<XNotification>, Error>
}
```

**Écart n°2** (le seul qui invalide une hypothèse du brief) : **il n'existe
pas de `notification.wait_timeout(Duration)`.** `XRequestNotification` n'est
pas directement pollable au niveau applicatif : `request()`/`poll()` exigent
un `Pin<&mut Self>` (la structure porte un `PhantomPinned`). L'usage prévu par
la crate elle-même est `spawn_thread(self, f)`, qui fait tourner la boucle
requête/attente dans un fil dédié et rappelle `f` à chaque notification —
sans délai réglable non plus : elle bloque tant qu'aucune notification
n'arrive. La sonde (`agent/src/gamepad.rs`) relaie donc les notifications de
ce fil vers le fil appelant par un canal `std::sync::mpsc`, et applique le
délai via `rx.recv_timeout(...)` côté canal plutôt que côté crate. **C'est le
patron que la tâche 10 doit reprendre.**

### Écart supplémentaire trouvé à l'exécution : `wait_ready()` ne suffit pas

Premier essai (sonde sans repli) : `target.update(&etat)` échoue
systématiquement juste après `wait_ready()`, avec :

```
WARN agent::gamepad: update() a échoué — variante brute erreur=WinError(259)
```

`259` = `ERROR_NO_MORE_ITEMS`. Ce n'est **pas** une variante que
`vigem-client` traduit en `Error::TargetNotReady` (seul `ERROR_DEV_NOT_EXIST`
l'est, voir `x360.rs::update`) — elle remonte donc telle quelle. Constat :
`wait_ready()` peut rendre `Ok(())` avant que le bus USB virtuel ait fini son
énumération PnP côté Windows ; un `update()` immédiatement après peut échouer
une ou plusieurs fois. La sonde corrigée boucle jusqu'à 20 fois avec un repli
de 250 ms. Dans l'essai retenu, **une seule tentative supplémentaire a
suffi** :

```
WARN agent::gamepad: update() pas encore prêt, nouvelle tentative tentative=1 erreur=WinError(259)
INFO agent::gamepad: update() a fini par réussir après attente tentatives=1
```

**La tâche 10 doit prévoir cette même boucle de repli après `wait_ready()`,
avant le premier `update()`.**

### Commande de lancement et sortie brute

```bash
set -a && source .env && set +a
scripts/build-agent.sh
VIGEM_PROBE=1 VIGEM_PROBE_SECS=25 scripts/run-agent.sh
```

Compilation : `Finished \`release\` profile [optimized] target(s) in 4.50s`,
zéro erreur (les 3 warnings affichés sont préexistants, sans rapport avec ce
chantier — code mort dans `encode.rs`/`window.rs`/`windows_source.rs`).

Pendant la fenêtre de 25 s, vibration déclenchée depuis la VM via un petit
programme C# appelant `XInputSetState` (`L=40000, R=20000`, répété 40 fois à
400 ms d'intervalle) :

```bash
node scripts/winrm.js "powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\xinput-vibe.ps1 2>&1 | Out-String"
```

`XInputSetState` retourne `0` (succès) pour la quasi-totalité des appels — la
manette virtuelle est bien vue par XInput comme un contrôleur connecté —, puis
`1167` (`ERROR_DEVICE_NOT_CONNECTED`) sur les tout derniers appels, une fois
que la sonde a débranché la cible en fin de fenêtre.

Journal de l'agent (`/media/vm/dev/agent.log`, encodé en **UTF-16LE** —
`Tee-Object` PowerShell écrit dans cet encodage par défaut ; `iconv -f
UTF-16LE -t UTF-8` est nécessaire pour le lire depuis Linux) :

```
INFO agent::gamepad: vibration reçue grand=0 petit=0
INFO agent::gamepad: vibration reçue grand=0 petit=0
INFO agent::gamepad: vibration reçue grand=0 petit=0
INFO agent::gamepad: vibration reçue grand=0 petit=0
INFO agent::gamepad: vibration reçue grand=156 petit=78
INFO agent: sonde ViGEmBus rapport="branchement OK, 5 notification(s) de vibration reçue(s)"
```

`grand=156, petit=78` correspond exactement à la conversion 16 bits → 8 bits
de `L=40000, R=20000` (`40000/65535*255 ≈ 156`, `20000/65535*255 ≈ 78`). Les 4
notifications `0/0` précèdent l'envoi de vibration (bruit d'énumération
initiale). **Une seule notification est arrivée pour les 40 appels
`XInputSetState`** : ViGEmBus ne notifie que sur *changement* de valeur, pas à
chaque appel — cohérent avec l'émission « sur changement » prévue au §6 de la
spec côté client, à répliquer côté agent pour la vibration.

### Verdict

**Le rappel de vibration remonte réellement et restitue les bonnes
magnitudes.** L'API exacte à utiliser dans la tâche 10 est celle documentée
ci-dessus (`spawn_thread` + canal, pas de `wait_timeout`), avec impérativement
un repli après `wait_ready()` avant le premier `update()`.

---

## Inconnue 3 — `movementX` sur les événements coalescés (à mesurer manuellement)

Cette sonde exige une souris physique et un humain devant l'écran : un
mouvement de souris synthétique ne prouverait rien sur `movementX` sous
Pointer Lock. **Non mesurée dans cette tâche.** Le fichier de sonde est créé
(`client/probe-coalesced.html`), prêt à l'emploi.

### Mode opératoire exact à suivre pour mesurer

```bash
cd client && npx vite --host 0.0.0.0
# ouvrir http://<hôte>:5173/probe-coalesced.html dans Chrome
```

1. Cliquer dans la zone grise pour verrouiller le pointeur (`requestPointerLock`).
2. Bouger la souris **continûment** pendant ~5 secondes.
3. Relever le verdict affiché : « coalescés exploitables » ou « COALESCÉS
   INUTILISABLES » (ce dernier si la somme des `movementX` des événements
   coalescés reste à 0 alors que des événements coalescés existent bien).

### Verdict

**À MESURER MANUELLEMENT.** Sans cette mesure, la tâche 12 doit soit
attendre cette mesure, soit prévoir d'emblée le repli documenté par la spec
(§11) : prendre le `movementX` de l'événement principal seul, en perdant la
restitution des positions intermédiaires — repli « immédiat et sans
conséquence sur le reste du design » selon la spec elle-même.

---

## Inconnue 4 — cadence réelle de `setInterval(4ms)` (à mesurer manuellement)

Sonde nécessitant elle aussi un navigateur réellement ouvert, idéalement
pendant une session vidéo active pour charger le thread principal comme en
conditions réelles. **Non mesurée dans cette tâche.**

### Mode opératoire exact à suivre pour mesurer

Dans la console DevTools de l'onglet servant `client/probe-coalesced.html`
(ou toute page de session active) :

```js
let n = 0; const t0 = performance.now();
const id = setInterval(() => { n++; }, 4);
setTimeout(() => { clearInterval(id); console.log('Hz réels :', (n / ((performance.now() - t0) / 1000)).toFixed(1)); }, 10000);
```

Relever la valeur affichée après 10 secondes.

### Verdict

**À MESURER MANUELLEMENT.** Seuil attendu : ≥ 200 Hz. En dessous, la tâche 14
devra accepter une cadence moindre plutôt que de prétendre 250 Hz — la spec
(§6) prévoit un sondage nominal à 250 Hz, plancher navigateur de
`setInterval`.

---

## Fichiers créés ou modifiés

| Fichier | Nature |
|---|---|
| `agent/Cargo.toml` | Ajout de la dépendance `vigem-client` (fonctionnalité `unstable_xtarget_notification`) |
| `agent/src/main.rs` | Déclaration `mod gamepad` + bloc `VIGEM_PROBE` |
| `agent/src/gamepad.rs` | **Nouveau, jetable** — sonde ViGEmBus, remplacée intégralement par la tâche 10 |
| `scripts/run-agent.sh` | Transmission de `VIGEM_PROBE`/`VIGEM_PROBE_SECS` |
| `client/probe-coalesced.html` | **Nouveau** — sonde navigateur `movementX`, prête mais non exécutée |
| `Cargo.lock` | Mis à jour côté VM par `cargo build` (ajout de `vigem-client`, `winapi` et ses deux crates de liaison), rapatrié tel quel |
| `docs/superpowers/plans/2026-07-28-input-jeu-sondes.md` | Ce document |

## Auto-revue

- Compilation vérifiée deux fois sur la VM (`cargo build --release`), zéro
  erreur, zéro warning imputable au nouveau code.
- La sonde a réellement été exécutée en session interactive (tâche planifiée
  `/it`, comme `run-agent.sh` le fait pour toutes les sondes de ce projet) et
  a produit un rapport de vibration cohérent avec les valeurs envoyées.
- Le repli de 250 ms sur `update()` a été découvert **par l'échec réel**, pas
  supposé a priori — je l'ai ajouté après avoir vu `WinError(259)` en pratique,
  et revérifié que la seconde tentative suffisait.
- `agent/src/gamepad.rs` reste volontairement une sonde : pas de gestion
  d'erreur soignée au-delà de ce qu'il faut pour produire un rapport, pas de
  configuration, pas de test unitaire — la tâche 9 (logiques pures) et la
  tâche 10 (module définitif) prennent le relais.
- Je n'ai **pas** exécuté les sondes 9 et 10 du brief (navigateur) : elles
  exigent un humain et une souris physique, une mesure automatisée aurait été
  fabriquée, pas observée. Les fichiers/instructions nécessaires sont en
  place ; les verdicts sont marqués « à mesurer manuellement » plutôt
  qu'inventés.
- `agent/src/main.rs` reproduit le bloc du brief à l'identique (aucune
  correction nécessaire, contrairement à `gamepad.rs`).

## Doutes et réserves

- **`WinError(259)` n'est pas expliqué en profondeur** : je documente le
  contournement (repli 250 ms × jusqu'à 20 tentatives) qui a fonctionné une
  fois, mais je n'ai pas caractérisé son comportement statistique (combien de
  tentatives dans le pire cas, est-ce stable dans le temps). La tâche 10
  devra probablement prévoir une marge plus large que « 1 tentative » observée
  ici, ou consulter les retours de la communauté `vigem-client`/ViGEmBus sur
  ce code d'erreur précis.
- **Une tâche planifiée `boucle-agent.ps1` tourne en arrière-plan sur la VM**
  (héritée d'une session antérieure, hors du périmètre de ce chantier) et
  échoue en boucle avec « aucune fenêtre visible dont le titre contient
  « firefox » », en écrivant dans le même `C:\dev\agent.log`. Elle a pollué la
  lecture du journal pendant cette tâche (nécessité de `grep`/`iconv` ciblés)
  mais n'a pas faussé les résultats rapportés ci-dessus, qui proviennent tous
  de lignes explicitement attribuées à `agent::gamepad` ou `agent` avec le
  message `sonde ViGEmBus`. Signalé ici sans y toucher — hors périmètre de
  cette tâche.
- **Inconnues 3 et 4 non mesurées** (voir ci-dessus, décision assumée conforme
  à la résolution d'ambiguïté donnée pour cette tâche) : la tâche 12/14 ne
  doit pas les considérer comme validées.
- **Le `Cargo.lock` rapatrié** vient d'un `cargo build --release` exécuté sur
  la VM avec un index crates.io déjà partiellement chaud ; je n'ai pas
  vérifié qu'un `cargo build` à froid (cache vide) produirait un lock
  identique — improbable que ça change quoi que ce soit vu que les versions
  sont épinglées, mais je le note par prudence.
