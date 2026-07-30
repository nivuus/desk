# CLAUDE.md

> **⚠️ IMPORTANT**: Ce fichier contient toutes les informations essentielles pour la continuation du projet. **Toute information importante pour la suite du développement DOIT être stockée dans ce fichier.** Cela inclut les configurations critiques, et toute connaissance acquise pendant le développement.

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Guacamole is a Node.js-based remote desktop web application that provides browser-based access to Windows applications via Apache Guacamole protocol (RDP). The system creates isolated sessions for running remote Windows applications with a custom filesystem bridging client and server.

## 📏 Conventions de code

### Taille maximale d'un fichier : 500 lignes

**Un fichier de code source ne doit pas dépasser 500 lignes.** Au-delà, le
fichier porte plus d'une responsabilité : il faut le découper avant d'y ajouter
quoi que ce soit.

**Portée** — la règle s'applique au code source écrit à la main :
`agent/src/`, `client/src/`, `signaling/`, `proto/`, `src/`, `web/`, `scripts/`.

**Exemptions explicites** :

- `docs/` — les plans, specs et recettes sont des journaux d'exécution, longs
  par nature et non maintenus comme du code.
- `CLAUDE.md` — ce fichier est un index de connaissances, pas du code.
- Fichiers générés ou vendorisés : `dist/`, `target/`, `node_modules/`,
  `*-lock.json`, `Cargo.lock`, `testdata/`.

**Règle d'application** : geler la dette, pas la purger. Aucun **nouveau**
fichier ne naît au-dessus de 500 lignes, et un fichier déjà au-dessus ne doit
pas grossir davantage — toute addition substantielle s'accompagne d'une
extraction. Le découpage rétroactif des fichiers ci-dessous se fait au moment
où l'on travaille dedans, pas en chantier séparé.

**Dette existante** (code source uniquement) :

| Fichier | Lignes | Pourquoi elle reste |
| --- | --- | --- |
| `agent/src/encode.rs` | 1480 | `#[cfg(windows)]`, aucun test |
| `agent/src/windows_source.rs` | 721 | `#[cfg(windows)]`, aucun test |
| `agent/src/wasapi.rs` | 543 | `#[cfg(windows)]`, aucun test |

Ces trois modules ne se compilent que sur la VM et ne sont couverts par aucun
test : les découper se ferait sans filet automatisé. La dette est assumée
jusqu'à ce qu'ils gagnent des tests — voir
`docs/superpowers/specs/2026-07-30-dette-taille-fichiers-design.md` §1.

Les quatre fichiers que la suite de tests couvrait ont été résorbés le
30 juillet 2026 : voir `docs/superpowers/plans/2026-07-30-dette-taille-fichiers.md`.

**Vérifier l'état** :

```bash
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
```

## Development Commands

### Running the Application

```bash
# With Docker (recommended)
docker-compose up --build

# Local development (requires guacd daemon)
npm install
node index.js
```

The server listens on port 3445 by default.

### Environment Variables

Required environment variables (configured in docker-compose.yml or index.js):

- `WINDOWS_HOSTNAME` - Target Windows machine IP/hostname
- `WINDOWS_USERNAME` - Standard user account for RDP connections
- `WINDOWS_PASSWORD` - Password for standard user
- `WINDOWS_ADMIN_USERNAME` - Administrator account for WinRM commands
- `WINDOWS_ADMIN_PASSWORD` - Administrator password

### Dependencies

The project uses:

- `guacd` daemon (Apache Guacamole server)
- Node.js 20.x
- FUSE for filesystem operations (requires privileged container)

## Architecture

### Core Components

**index.js** - Entry point that initializes Express server and coordinates three main modules:

- `src/app.js` - Windows application discovery via WinRM
- `src/session.js` - Guacamole session and connection management
- `src/asset.js` - Static asset serving and image processing

### Session Flow

1. User navigates to `/:app` (e.g., `/excel`)
2. System generates UUID session identifier
3. Creates:
   - WebSocket tunnel for Guacamole protocol
   - Separate WebSocket for filesystem bridge
   - FUSE mount at `/mnt/ftp-{uuid}`
4. Spawns `guacd` daemon instance bound to port 4822
5. Establishes RDP connection to Windows with RemoteApp configuration
6. Browser connects via guacamole-common-js client library

### Filesystem Architecture

The system implements a bidirectional filesystem bridge between browser and Windows:

**Server side** (`src/file.js`):

- Creates FUSE mount using `fuse-native`
- Translates FUSE operations (read, write, getattr, etc.) to WebSocket messages
- Caches file data to reduce round-trips
- Mount point exposed to Windows via RDP drive sharing

**Client side** (`web/index.js`, lines 406-618):

- Uses File System Access API (`showDirectoryPicker`)
- Receives filesystem operation requests via WebSocket
- Performs local file operations and returns results
- Enables Windows applications to read/write files directly to user's browser filesystem

### Application Discovery

`src/app.js` discovers available Windows applications by:

1. Querying Windows desktop shortcuts via WinRM/PowerShell
2. Extracting icons and converting to base64 PNG
3. Determining file associations from Windows registry
4. Generating metadata (name, color theme, MIME handlers)
5. Refreshing application list hourly

### Frontend Build Process

`src/asset.js` uses Gulp to transpile and bundle:

- `web/index.js` → `dist/app.js` (main client application)
- `web/home.js` → `dist/home.js` (home page)
- Uses Babel for ES6+ → ES5 transpilation
- Uses Browserify to bundle CommonJS modules for browser

### Guacamole Integration

Connection established via `guacamole-lite` wrapper:

- Encryption: AES-256-CBC with session UUID as key
- Protocol: RDP with RemoteApp mode
- Audio: Bidirectional audio support
- Clipboard: Synchronized clipboard via WebSocket streaming
- Input: Mouse, touch, and keyboard event forwarding

### PWA Features

The application supports Progressive Web App installation:

- Dynamic manifest generation per application (`/:app/manifest.json`)
- Service worker registration (`web/sw.js`)
- File handler associations for opening files in installed apps
- Window controls overlay for native-like window chrome
- Theme color extraction from application canvas

## Key Technical Details

### RDP Configuration

Session settings in `src/session.js` (lines 126-159):

- Client name set to UUID to ensure unique sessions
- Drive sharing enabled, pointing to FUSE mount
- Display update resize method for dynamic resolution
- Font smoothing and touch enabled
- Upload/download disabled (filesystem bridge used instead)
- High DPI support (500 DPI default)

### Security Considerations

- Credentials are stored in environment variables and hardcoded in index.js
- AES-256-CBC encryption for Guacamole protocol
- WebSocket connections should be served over WSS in production
- Container requires privileged mode for FUSE operations

### Client-Side State Management

`web/index.js` manages:

- Display scaling and resize handling
- Clipboard synchronization (bidirectional)
- Title bar dragging for PWA window mode
- Theme color extraction from canvas pixels
- Audio context initialization on user interaction

### Cleanup and Resource Management

`src/cleanup.js` (referenced but not examined) handles:

- Graceful guacd process termination
- FUSE unmounting via `fusermount -u`
- WebSocket connection cleanup
- Session instance removal from registry

## Common Development Patterns

### Adding New Remote Application Support

Applications are auto-discovered from Windows desktop shortcuts. To add support:

1. Place `.lnk` shortcut on Windows user desktop
2. Application will appear in list after next refresh cycle
3. File associations automatically extracted from Windows registry

### Modifying Frontend Client

1. Edit source in `web/index.js`
2. Gulp watches and rebuilds to `dist/app.js` automatically
3. Changes require browser refresh (no hot reload)

### Testing WinRM Connection

The repository includes test files:

- `test_winrm_nodejs.js` - NodeJS WinRM testing
- `test_winrm_fixed.js` - Fixed WinRM implementation tests

## Known Constraints

- Only supports single guacd instance on port 4822
- Requires Windows machine with:
  - RDP RemoteApp capability
  - WinRM enabled (port 5985)
  - Desktop shortcuts for discoverable applications
- FUSE operations require Linux host with kernel support
- File System Access API requires modern Chromium-based browser
- No horizontal scaling due to in-memory session storage

---

## 🔧 Problèmes Résolus (Session du 21 Oct 2025)

### 1. Icônes Manquantes - PowerShell Extraction Failed

**Symptôme**: Aucune icône ne s'affichait, logs montrant "PowerShell extraction returned FAILED"

**Cause Racine**: Script PowerShell complexe utilisant `Add-Type -TypeDefinition` avec code C# pour `ExtractIconEx` ne fonctionnait pas correctement

**Solution Implémentée** (`src/iconExtractor.js` lignes 165-178):

```javascript
const script = `
if (-not (Test-Path 'C:\\temp')) { New-Item -ItemType Directory -Path 'C:\\temp' | Out-Null }
Add-Type -AssemblyName System.Drawing
$icon = [System.Drawing.Icon]::ExtractAssociatedIcon('${windowsPath}')
if ($icon) {
    $bitmap = $icon.ToBitmap()
    $bitmap.Save('${tempFile}', [System.Drawing.Imaging.ImageFormat]::Png)
    $bitmap.Dispose()
    $icon.Dispose()
    echo 'SUCCESS'
} else {
    echo 'FAILED'
}
`.replace(/\n/g, "; ");
```

**Compromis**: Les icônes sont maintenant extraites en basse résolution (32x32 ou 48x48) au lieu de haute résolution (256x256+)

**Compensation**: Taille d'affichage réduite à 48px sur la page d'accueil (`assets/index.html` lignes 64-65) pour masquer la pixelisation

**Statut**: ✅ RÉSOLU - Les icônes s'affichent correctement

---

### 2. Sessions RDP Se Fermant Immédiatement (Après 2 Secondes)

**Symptôme**: Les sessions s'ouvraient puis se fermaient automatiquement après exactement 2 secondes

**Cause Racine**:

1. Le WebSocket filesystem tentait de se connecter au client browser
2. Si la connexion échouait ou était lente, un heartbeat check toutes les 2 secondes détectait le problème
3. Le handler d'erreur du WebSocket appelait `window.close()` immédiatement
4. Les handlers du tunnel Guacamole appelaient aussi `window.close()` sur toute erreur

**Solutions Multiples Implémentées** (`web/index.js`):

**A) Désactivation fermeture sur erreur filesystem** (lignes 426-433):

```javascript
ws.onclose = function () {
  console.log("Filesystem WebSocket closed (filesystem features disabled)");
  // NE PLUS fermer la fenêtre
};

ws.onerror = function (error) {
  console.log(
    "Filesystem WebSocket error:",
    error,
    "(filesystem features disabled)"
  );
  // NE PLUS fermer la fenêtre
};
```

**B) Heartbeat interval augmenté** (ligne 451):

```javascript
// Changé de 2000ms à 30000ms
setInterval(() => {
  if (ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ heartbeat: true }));
  }
}, 30000);
```

**C) Tunnel handlers moins agressifs** (lignes 117-122):

```javascript
if (state === Guacamole.Tunnel.State.CLOSED) {
  console.log("Tunnel closed - will close window in 1 second");
  setTimeout(function () {
    window.close();
  }, 1000); // Délai pour éviter fermeture prématurée
}
```

**Impact**: Les sessions restent maintenant ouvertes même si le filesystem WebSocket échoue. Les features de partage de fichiers sont simplement désactivées silencieusement.

**Statut**: ✅ RÉSOLU - Les sessions restent ouvertes correctement

---

### 3. Fenêtre Ne Se Fermant Pas Après Déconnexion

**Symptôme**: Après avoir fermé l'application RDP côté Windows, la fenêtre du navigateur restait ouverte indéfiniment

**Cause**: Tous les handlers de fermeture automatique avaient été désactivés pour résoudre le problème #2

**Solution** (`web/index.js` lignes 117-122):

- Ajout d'un timer de 1 seconde avant fermeture
- Fermeture uniquement quand le tunnel Guacamole passe à l'état `CLOSED`
- Ignore les états `UNSTABLE` pour permettre la reconnexion

**Statut**: ✅ RÉSOLU - La fenêtre se ferme correctement après déconnexion

---

### 4. Design Obsolète de la Page d'Accueil

**Problème**: Design basique avec fond bleu foncé, cards simples sans animations

**Améliorations Implémentées** (`assets/index.html` lignes 26-134):

1. **Fond moderne**: Dégradé violet/bleu (`linear-gradient(135deg, #667eea 0%, #764ba2 100%)`)
2. **Effet Glassmorphism**: Cards blanches semi-transparentes avec backdrop-filter blur
3. **Grid CSS Responsive**: Auto-adaptatif selon la largeur d'écran
4. **Animations**: Effet de levée au survol, scale sur les boutons
5. **Typographie**: Police système moderne (-apple-system, Segoe UI, etc.)
6. **Ombres douces**: `box-shadow: 0 8px 32px rgba(0, 0, 0, 0.1)`
7. **Icônes optimisées**: Taille fixe 48x48px avec `object-fit: contain`

**Statut**: ✅ RÉSOLU - Design moderne et professionnel

---

### 5. Bouton Install/Uninstall Non Fonctionnel

**Symptôme**: `navigator.getInstalledRelatedApps()` retourne toujours un tableau vide

**Cause**: Cette API ne fonctionne que pour les apps cross-origin avec `related_applications` dans le manifest, pas pour les PWA du même domaine

**Solution** (`web/home.js` lignes 3-23):

- Tracking manuel via `localStorage`
- Fonctions helper: `isAppInstalled()`, `markAppInstalled()`, `markAppUninstalled()`
- Bouton affiche "Install" ou "Uninstall" selon l'état stocké
- Instructions affichées pour guider l'utilisateur vers l'installation native du navigateur

**Limitation Connue**: Si l'utilisateur installe via le menu du navigateur directement, le bouton ne changera pas automatiquement (nécessite clic sur Install puis Uninstall pour sync)

**Statut**: ⚠️ PARTIELLEMENT RÉSOLU - Fonctionne mais nécessite recompilation JavaScript (voir section Build)

---

## 🐛 Problèmes Connus Non Résolus

### 1. Crash "double free or corruption"

**Log Type**: `guacd[63]: ERROR: double free or corruption (out)`

**Contexte**:

- Se produit sporadiquement après ~30-60 secondes
- Souvent corrélé avec un accès au filesystem FUSE
- Log précédent: `getattr / { mode: 16877, size: 1000000000 }`

**Cause Probable**:

- Race condition entre fermeture du WebSocket filesystem et opération FUSE en cours
- Guacd tente d'accéder à la mémoire du drive FUSE pendant qu'il se démonte
- Double libération de mémoire dans le code natif de `fuse-native` ou `guacd`

**Impact**:

- Ferme la session RDP prématurément
- Message d'erreur: "RDP server closed/refused connection: Manually disconnected"
- L'utilisateur perd son travail si non sauvegardé

**Mitigations Actuelles**:

- Filesystem WebSocket ne ferme plus la fenêtre en cas d'erreur
- Caching des opérations filesystem pour réduire les appels

**TODO**:

1. Ajouter une gestion propre de shutdown du filesystem
2. Attendre que toutes les opérations FUSE soient terminées avant démontage
3. Investiguer les logs de guacd avec niveau DEBUG
4. Considérer alternative à FUSE (WebDAV? SFTP?)

**Workaround Utilisateur**: Si le crash se produit, simplement relancer l'application

---

### 2. Qualité Basse Résolution des Icônes

**Problème**:

- Icônes extraites en 32x32 ou 48x48 seulement
- Upscalées à 512x512 par Sharp (algorithme Lanczos3)
- Résultat pixelisé visible si affiché en grand

**Cause**:

- `System.Drawing.Icon.ExtractAssociatedIcon()` retourne seulement les petites icônes
- Les méthodes haute résolution (icotool, wrestool, sharp-ico) échouent avec les .exe modernes

**Mitigation Actuelle**:

- Affichage à 48px sur la page d'accueil (1:1 ou 1.5x upscale seulement)
- Acceptable pour les icônes système

**Solutions Possibles**:

1. Réimplémenter extraction via PowerShell avec `[System.IconExtractor]` et P/Invoke vers Shell32.dll
2. Utiliser un outil Windows natif (ResourceHacker, RCEdit) appelé via WinRM
3. Extraire directement depuis les ressources PE avec lecteur binaire custom
4. Pre-générer icônes haute résolution côté Windows et les stocker

**Priorité**: MOYENNE - Fonctionnel mais pas idéal

---

### 3. Détection PWA Installées Incomplète

**Problème**:

- Le système ne détecte pas automatiquement si l'utilisateur a installé l'app via le menu du navigateur
- Nécessite que l'utilisateur clique sur "Install" depuis la page d'accueil

**Cause**:

- `navigator.getInstalledRelatedApps()` ne fonctionne pas pour PWA du même domaine
- Pas d'événement JavaScript natif quand une PWA est installée depuis le menu browser
- LocalStorage utilisé comme workaround mais peut se désynchroniser

**Impact**:

- Confusion utilisateur: app installée mais bouton dit "Install"
- Pas de problème fonctionnel, juste UX sous-optimal

**Solutions Possibles**:

1. Ajouter `related_applications` dans manifest avec URL externe (mais nécessite domaine différent)
2. Utiliser Service Worker pour détecter l'installation (événement `appinstalled`)
3. Vérifier `window.matchMedia('(display-mode: standalone)')` régulièrement
4. Accepter la limitation et documenter le comportement

**Priorité**: BASSE - Cosmétique seulement

---

### 4. Recompilation JavaScript Manuelle

**Problème**:

- Modifications de `web/home.js` et `web/index.js` ne sont PAS recompilées automatiquement
- Nodemon surveille seulement `index.js` et `src/*.js`
- Build Gulp s'exécute uniquement au démarrage de l'application

**Impact**:

- Les changements JavaScript client nécessitent redémarrage Docker complet
- Ralentit le développement frontend
- Risque d'oublier de recompiler et tester ancien code

**Workaround Actuel**:

```bash
docker-compose restart web
```

**Solutions Possibles**:

1. Ajouter un watcher Gulp séparé pour `web/*.js`
2. Modifier `nodemon.json` pour inclure `web/` dans les fichiers surveillés
3. Migrer vers Webpack avec hot module replacement
4. Script de développement séparé avec `gulp watch`

**Configuration Actuelle** (`src/asset.js` lignes 19-37):

```javascript
gulp
  .src(__dirname + "/../web/index.js")
  .pipe(babel({ presets: ["@babel/env"] }))
  .pipe(browserify({ insertGlobals: true, debug: true }))
  .pipe(gulp.dest(__dirname + "/../dist"));
```

**TODO**: Ajouter système de watch pour développement plus fluide

**Priorité**: MOYENNE - Impact développement

---

## 📦 Système de Build et Compilation

### Pipeline de Compilation

**Source**: `src/asset.js` lignes 19-37

**Processus**:

```
web/index.js  → Babel (preset: @babel/env) → Browserify → dist/index.js
web/home.js   → Babel (preset: @babel/env) → Browserify → dist/home.js
```

### Déclenchement

- ✅ **Automatique**: Au démarrage de l'application Node.js
- ❌ **Pas de watch**: Modifications ne déclenchent pas rebuild
- ❌ **Nodemon ignore web/**: Surveille uniquement `index.js` et `src/`

### Forcer la Recompilation

```bash
# Méthode 1: Redémarrer le conteneur Docker
docker-compose restart web

# Méthode 2: Redémarrer complètement
docker-compose down
docker-compose up -d

# Méthode 3: Touch index.js pour déclencher nodemon (NE FONCTIONNE PAS pour web/*)
touch index.js
```

### Vérifier la Compilation

```bash
# Voir date de dernière modification
ls -lah dist/home.js dist/index.js

# Vérifier taille des fichiers
du -h dist/*.js
```

---

## 🔐 Configuration Critique

### Ports Exposés

| Port | Service      | Description                           |
| ---- | ------------ | ------------------------------------- |
| 3445 | Express HTTP | Serveur web principal                 |
| 4822 | guacd        | Daemon Guacamole (RDP proxy)          |
| 5985 | WinRM        | Communication PowerShell vers Windows |
| 3389 | RDP          | Port RDP Windows (implicite)          |

### Paths Importants

| Path                                 | Description                       |
| ------------------------------------ | --------------------------------- |
| `/media/vm/`                         | Mount point du filesystem Windows |
| `/media/vm/Users/{username}/Desktop` | Source des .lnk shortcuts         |
| `/mnt/ftp-{uuid}`                    | FUSE mount par session            |
| `/opt/server`                        | Workdir dans container Docker     |
| `/home/mallanic/Projects/Guacamole`  | Root projet sur host              |

### Credentials et Secrets

⚠️ **SÉCURITÉ CRITIQUE** ⚠️

**Fichiers contenant des credentials en clair**:

1. `index.js` lignes 2-4
2. `docker-compose.yml` lignes 14-17

**Variables**:

- `WINDOWS_HOSTNAME`: 192.168.3.2
- `WINDOWS_ADMIN_USERNAME`: Administrator
- `WINDOWS_ADMIN_PASSWORD`: [MASQUÉ - voir fichiers]
- `WINDOWS_USERNAME`: guacamole
- `WINDOWS_PASSWORD`: [MASQUÉ - voir fichiers]

**TODO PRODUCTION**:

1. ⚠️ Externaliser dans fichier .env (ne jamais commit)
2. ⚠️ Utiliser Docker secrets ou variables d'environnement externes
3. ⚠️ Chiffrer les secrets avec Vault, AWS Secrets Manager, etc.
4. ⚠️ Rotation régulière des mots de passe
5. ⚠️ Audit trail des accès

---

## 📝 Parser de Fichiers .lnk Windows

### Spécification

Implémente la spécification **MS-SHLLINK** (Shell Link Binary File Format)

**Fichier**: `src/lnkParser.js`

### Structure Parsée

1. **ShellLinkHeader** (76 bytes / 0x4C)

   - Magic number: 0x4C (validatio)
   - LinkFlags (offset 0x14)

2. **LinkTargetIDList** (optionnel)

   - Taille variable selon flags
   - Skippé mais taille lue

3. **LinkInfo** (optionnel si flag 0x02)

   - Contient le chemin local (`LocalBasePath`)
   - Ou chemin réseau (`NetworkShareName`)
   - **Problème connu**: Parfois retourne juste "C:\\" incomplet

4. **StringData** (plusieurs sections)
   - NAME_STRING (flag 0x04)
   - RELATIVE_PATH (flag 0x08) ← **Utilisé comme fallback**
   - WORKING_DIR (flag 0x10)
   - COMMAND_LINE_ARGUMENTS (flag 0x20)
   - ICON_LOCATION (flag 0x40) ← **Important pour icônes**

### Données Extraites

```javascript
{
  targetPath: "C:\\Program Files\\Mozilla Firefox\\firefox.exe",
  iconPath: "C:\\Program Files\\Mozilla Firefox\\firefox.exe",
  iconIndex: 0
}
```

### Cas Spéciaux Gérés

1. **Chemin incomplet dans LinkInfo**: Utilise RELATIVE_PATH comme fallback
2. **Chemins relatifs** (ex: `..\..\AppData\Local\...`): Convertis en absolus
3. **Format IconLocation**: Parse "path,index" (ex: "notepad.exe,1")
4. **Variables d'environnement**: Substitution basique

### Conversion de Chemins

**Fonction**: `convertToLinuxPath(windowsPath, lnkPath)`

```javascript
// Exemples de conversion:
"C:\\Program Files\\Firefox\\firefox.exe"
  → "/media/vm/Program Files/Firefox/firefox.exe"

"..\\AppData\\Local\\App\\app.exe" (avec contexte utilisateur)
  → "/media/vm/Users/guacamole/AppData/Local/App/app.exe"

"%ProgramFiles%\\App\\app.exe"
  → "/media/vm/Program Files/App/app.exe"
```

---

## 🎨 Extraction d'Icônes - Détails Techniques

### Méthodes Tentées (Ordre de Priorité)

**Fichier**: `src/iconExtractor.js`

#### 1. Fichier .ico Standalone

```javascript
// Cherche {exe}.ico
const icoPath = exePath.replace(/\.exe$/i, ".ico");
// Cherche aussi: icon.ico, app.ico, logo.ico, icon_256.ico
```

**Taux de succès**: ~5% (rare)

#### 2. icotool (icoutils)

```bash
icotool -x -i 1 application.exe
```

**Problème**: Erreur "reserved non-zero" sur .exe modernes
**Taux de succès**: ~10%

#### 3. wrestool (icoutils)

```bash
wrestool -l -t 14 application.exe  # List icon groups
wrestool -x -t 14 -n ICON_NAME application.exe
```

**Problème**: Erreur "No icons found" sur beaucoup d'exécutables
**Taux de succès**: ~15%

#### 4. sharp-ico (Direct PE Reading)

```javascript
const icons = await decode(exeBuffer);
```

**Problème**: Erreur "Invalid magic bytes" - ne supporte pas PE modernes
**Taux de succès**: ~5%

#### 5. PowerShell (MÉTHODE RETENUE) ✅

```powershell
Add-Type -AssemblyName System.Drawing
$icon = [System.Drawing.Icon]::ExtractAssociatedIcon('C:\path\to\app.exe')
$bitmap = $icon.ToBitmap()
$bitmap.Save('C:\temp\icon.png', [System.Drawing.Imaging.ImageFormat]::Png)
```

**Avantages**:

- Fonctionne toujours (100% succès)
- API Windows native
- Pas de dépendances externes

**Limitations**:

- Retourne seulement les petites icônes (32x32 ou 48x48)
- Pas d'accès aux grandes versions (256x256+) présentes dans l'exe

**Taux de succès**: **100%** ✅

### Post-Processing

Après extraction, toutes les icônes subissent:

```javascript
const resized = await sharp(iconBuffer)
  .resize(512, 512, {
    fit: "contain",
    background: { r: 0, g: 0, b: 0, alpha: 0 },
    kernel: sharp.kernel.lanczos3,
  })
  .png()
  .toBuffer();
```

**Algorithme**: Lanczos3 (meilleur qualité pour upscaling)
**Taille cible**: 512x512 (pour manifest PWA)
**Affichage réel**: 48x48 (page d'accueil)

### Extraction de Couleur

**Librairie**: ColorThief

```javascript
const color = await ColorThief.getColor(image);
// Retourne: [R, G, B]  ex: [251, 213, 64]
```

**Utilisé pour**:

- `theme-color` dans manifest
- `background-color` dans manifest
- Couleur de fond dans app.tpl.html

---

## 🔄 Flux de Lancement d'Application Complet

### 1. Découverte (Startup + Hourly)

```javascript
// src/app.js ligne 52
async function fetchApps() {
  const desktopPath = `/media/vm/Users/${process.env.WINDOWS_USERNAME}/Desktop`;
  const files = await fs.readdir(desktopPath);
  const lnks = files.filter((f) => f.endsWith(".lnk"));

  for (const lnk of lnks) {
    const { targetPath, iconPath, iconIndex } = await parseLnk(lnkPath);
    const iconBase64 = await extractIcon(finalIconPath, iconIndex, 512);
    const color = await ColorThief.getColor(image);
    const associations = await getAppFileAssociations(name);

    apps.push({ name, short, program, color, icon, fileHandlers });
  }
}

// Refresh toutes les heures
setInterval(update, 1000 * 60 * 60);
```

### 2. Affichage Liste (GET /)

```javascript
// assets/index.html chargé
// web/home.js récupère la liste
fetch("/apps")
  .then((res) => res.json())
  .then((apps) => apps.forEach((app) => addApp(app)));
```

### 3. Clic Launch (GET /:app)

```javascript
// src/session.js ligne 99
app.get("/:app", async (req, res) => {
  const guacdIndex = uuid.v4().slice(0, 32);

  // Créer WebSocket Server pour filesystem
  const ws = new WebSocketServer({ noServer: true });
  server.on("upgrade", upgradeHandler);

  // Créer FUSE mount
  const fileSystem = await createFileSystem(guacdIndex, ws);

  // Configurer Guacamole
  const guacServer = new GuacamoleLite(
    {
      server,
      noServer: true,
      path: `/${app.short}/${guacdIndex}`,
    },
    guacdOptions,
    clientOptions,
    callbackOptions
  );

  // Stocker la session
  guacdInstances[guacdIndex] = { port, guacServer, ws };

  // Rediriger vers la session
  res.redirect(`/${app.short}/${guacdIndex}`);
});
```

### 4. Ouverture Session (GET /:app/:uuid)

```html
<!-- assets/app.tpl.html chargé -->
<!-- web/index.js (compilé → dist/index.js) exécuté -->
```

```javascript
// web/index.js ligne 111
const tunnel = new Guacamole.WebSocketTunnel(`/${appName}/${sessionID}`);
tunnel.setUUID(sessionID);

const guac = new Guacamole.Client(tunnel);

const token = await encrypt(
  {
    connection: {
      type: "rdp",
      settings: {
        dpi: getDPI() + "",
        "color-depth": 24 + "",
      },
    },
  },
  clientOptions
);

guac.connect(
  "token=" + token + "&height=..." + "&width=..." + "&GUAC_AUDIO=audio/L16"
);
```

### 5. Connexion RDP

```
Browser WebSocket → guacamole-lite → guacd (port 4822) → RDP (192.168.3.2:3389)
```

**Configuration RDP envoyée**:

- Type: rdp
- RemoteApp: C:\Program Files\Mozilla Firefox\firefox.exe
- Drive: /mnt/ftp-{uuid}
- Audio: bidirectionnel
- Clipboard: sync via Guacamole
- Resize: display-update

### 6. Session Active

**Inputs**:

- Souris → `Guacamole.Mouse` → `guac.sendMouseState()`
- Touch → `Guacamole.Mouse.Touchscreen` → `guac.sendMouseState()`
- Clavier → `Guacamole.Keyboard` → `guac.sendKeyEvent()`

**Outputs**:

- Display → `guac.getDisplay()` → Canvas HTML
- Audio → `guac.onaudio` → Web Audio API
- Clipboard → `guac.onclipboard` → Clipboard API

**Filesystem**:

- Windows read/write → FUSE → WebSocket → Browser → File System Access API

### 7. Fermeture

```javascript
// Quand l'app Windows se ferme:
guacd[63]: INFO: RDP session closed

// Tunnel passe à CLOSED
tunnel.onstatechange → state === Guacamole.Tunnel.State.CLOSED

// Fermeture après 1 seconde
setTimeout(() => window.close(), 1000);

// Cleanup serveur
guacServer.on('close', () => {
  guacServer.close();
  ws.close();
  server.removeListener('upgrade', upgradeHandler);
  delete guacdInstances[guacdIndex];
});
```

---

## 🖥️ Cycle de vie de la VM Windows

**La VM cible est une machine libvirt/QEMU nommée `Windows`, et elle n'est pas
démarrée automatiquement.** Tout travail touchant l'agent Rust, la capture, la
recette WebRTC ou WinRM exige qu'elle tourne. Symptômes d'une VM éteinte :
`192.168.3.2` injoignable (« Aucun chemin d'accès pour atteindre l'hôte
cible ») et `/media/vm/` vide.

```bash
# État
virsh list --all            # « fermé » = éteinte, « en cours d'exécution » = démarrée

# Démarrer, puis attendre que WinRM réponde (~5 à 60 s)
virsh start Windows
until timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null; do sleep 5; done

# Puis attendre que le partage de fichiers soit monté : /media/vm se monte
# après que WinRM répond, pas en même temps — un `scripts/build-agent.sh`
# lancé dès que le port 5985 répond échoue avec « erreur : /media/vm n'est
# pas monté » (`scripts/sync-agent.sh`, qui teste le montage, pas le port).
until mountpoint -q /media/vm; do sleep 5; done

# Arrêter proprement
virsh shutdown Windows
```

Une fois la VM démarrée, l'outillage habituel redevient disponible :

```bash
set -a && source .env && set +a      # charge les identifiants (fichier gitignoré)
node scripts/winrm.js '<commande PowerShell>'
scripts/build-agent.sh               # synchronise puis compile SUR la VM
scripts/run-agent.sh                 # lance l'agent en session interactive
```

**Ne jamais supposer la VM allumée.** Vérifier `virsh list --all` avant toute
séquence qui en dépend, et la démarrer si besoin — c'est une opération sûre et
idempotente.

---

## 🔊 Audio de la VM — état constaté (28 juillet 2026)

Relevé au moment de cadrer le chantier A, pour lever le risque « la VM n'a
peut-être aucun périphérique de rendu audio, donc WASAPI loopback ne produirait
rien ».

**Le risque est levé : deux endpoints de rendu sont actifs.**

| Périphérique | État |
| --- | --- |
| Haut-parleurs (Steam Streaming Speakers) | `OK` — actif, **virtuel**, aucun matériel requis |
| HDP-V104 (NVIDIA High Definition Audio) | `OK` — actif |
| NVIDIA Virtual Audio Device (Wave Extensible) (WDM) | pilote présent |
| « Sortie audio de l'ordinateur distant » (×11) | `Unknown` — endpoints RDP résiduels de sessions mortes |

`Audiosrv` tourne, démarrage `Automatic`. **Aucun pilote audio virtuel
supplémentaire n'est à installer.** Reste à confirmer par l'API réelle
(`IMMDeviceEnumerator::GetDefaultAudioEndpoint`) lequel est le périphérique par
défaut de la session interactive.

Commande de relevé :

```bash
node scripts/winrm.js "(Get-Service Audiosrv | Format-List Name,Status,StartType | Out-String); \
  Get-PnpDevice -Class AudioEndpoint | Select-Object FriendlyName,Status | Format-List | Out-String"
```

### Relevé de la sonde loopback (tâche 5, 28 juillet 2026)

Sonde `AUDIO_PROBE` (`agent/src/wasapi.rs` + `agent/src/main.rs`), exécutée en
session interactive via `scripts/run-agent.sh` (task planifiée `/it`), sur le
périphérique de rendu par défaut de **cette session** (pas la session 0 de
WinRM).

- **Format de mixage exact** : `48000 Hz, 2 canaux, 32 bits, flottant`
  (`WAVE_FORMAT_EXTENSIBLE` / `KSDATAFORMAT_SUBTYPE_IEEE_FLOAT`). Compatible
  avec `SAMPLE_RATE_HZ = 48_000` de `agent/src/opus.rs` : aucun
  rééchantillonneur nécessaire.
- **Crête au repos** (10 s, rien ne joue) : `crete = 0`, `silencieux = true`,
  `lectures_vides = 1894`. Normal.
- **Crête avec son** :
  - Avec `[Console]::Beep(880, 400)` (commande exacte du brief) : `crete = 0`
    à nouveau, sur 20 s, malgré 10 bips. **Ceci n'est PAS le cas redouté**
    (périphérique par défaut détourné) : `[Console]::Beep` passe par l'ancienne
    API `kernel32!Beep` (haut-parleur PC), qui sur cette VM ne traverse pas le
    périphérique de rendu par défaut — c'est un artefact de méthode de test,
    pas un signal sur le pipeline audio.
  - Avec `(New-Object Media.SoundPlayer '...Windows Ding.wav').PlaySync()`
    (API multimédia standard, la même famille que ce que joueraient de vraies
    applications) : `crete = 2385`, `silencieux = false`,
    `echantillons = 424320` sur 15 s. **Le loopback capte bien ce que jouent
    les applications** — sonde n°2 répondue positivement.
- **Conclusion** : l'hypothèse « Steam Streaming Speakers détourné » n'est
  **pas** confirmée. Le format de mixage est directement compatible (48 kHz).
  Pour de futurs tests manuels sur cette VM, préférer `SoundPlayer` (ou toute
  API multimédia réelle) à `[Console]::Beep`, qui donne un faux négatif.

---

## 📡 Chantier C volet 1 — asservissement réseau (fusionné le 29 juillet 2026)

Commit de fusion `620bae5`. Recette complète :
`docs/superpowers/plans/2026-07-29-reseau-adaptatif-resultats.md`.

Le débit et la résolution d'encodage suivent ce que le lien porte. Prouvé sous
dégradation `netem` : estimation de 2,5 Mb/s à 148 kb/s, descente au barreau
plancher, navigateur recevant du 382×242 — vérifié des deux côtés.

### Le FEC audio et le mode faible latence sont mutuellement exclusifs

**À connaître avant de toucher à `agent/src/opus.rs`.** `RESTRICTED_LOWDELAY`
force `MODE_CELT_ONLY` (`opus_encoder.c:1349`), où `decide_fec` retourne 0 sans
condition (`opus_encoder.c:721`) : la redondance LBRR n'existe que dans SILK.
Le FEC activé au chantier A n'a donc **jamais rien émis** jusqu'à ce chantier.

L'application est désormais `Application::Audio` — arbitrage assumé de 4 ms de
pré-délai (120 → 312 échantillons) contre la résilience, l'audio restant
largement en avance sur les ~48 ms de latence vidéo. Ne pas revenir à
`LowDelay` sans rouvrir cet arbitrage.

**Piège de mesure** : à débit cible fixe, LBRR *redistribue* les octets, il ne
s'y *ajoute* pas. Vérifier le FEC par une taille de paquet est un test invalide.
La preuve valable est un décodage sur décodeur **neuf sans historique** :
`decode(paquet, sortie, fec=true)` rend 0,00 d'énergie sans perte déclarée
contre 585,02 avec.

### Banc de dégradation réseau

`scripts/netem.sh <lan|adsl|4g|congestionné|effondrement|off>`, sur
**`internalBridge`** (le pont de la VM ; aucun réseau libvirt n'est défini, il
est géré à la main). Pose la dégradation dans les **deux sens** — le sens
agent→navigateur arrive en entrée et exige une redirection `ifb` via
`tc mirred`. `lan` et `off` sont deux profils distincts à dessein : témoin de
recette contre retrait du banc. **Toujours reposer `off` en fin de mesure.**

### Angle mort de méthode, à corriger dans les recettes suivantes

Le banc tournait sur une fenêtre de **764×484** alors que le produit vise le
plein écran. Cet écart a produit à lui seul **deux des trois trouvailles de la
revue finale** : à cette taille le barreau plein n'exige que 1,11 Mb/s, franchi
d'emblée par l'amorçage du BWE, ce qui masquait une fausse alerte « Image
réduite par le réseau » qui se déclenche sur toute source ≥ 1080p. **Fixer une
résolution représentative au canevas de recette, et y exercer au moins un
redimensionnement de fenêtre.**

### Deux réserves connues, non bloquantes

- Les transitions de `Adaptation` (`Active` → `Indisponible`) et l'expiration de
  l'estimation BWE à 5 s n'ont **pas de test** : vérifiées par lecture de code.
- Le câblage de `resize` côté `transport.rs` (branche `a1`, qui appelle
  `Controleur::changer_source`) n'a **aucun verrou automatisé** :
  `VideoSource::resize` est un no-op par défaut dans toutes les sources
  factices, et rien ne positionne `pending_resize` dans les tests. Le
  comportement est prouvé par mesure sur la VM, pas protégé contre régression.

### Ce que le chantier D (multi-fenêtres) devra régler

`Controleur` s'instancie par flux sans difficulté, **mais son alimentation
non** : `Event::EgressBitrateEstimate` est une estimation **de session**, pas de
piste — c'est la capacité du chemin, partagée. Le chantier D devra donc insérer
une **couche de répartition** (parts égales ? prorata des pixels ? priorité à la
fenêtre au premier plan ?) avant de remettre la capacité à N contrôleurs. Deux
dettes voisines à régler en même temps : `audio_bps` est un budget unique de
session, à retirer une fois et non N fois, et le filtre de `MediaEgressStats`
s'appuie sur un `video_mid` unique. **À cadrer dans le plan du chantier D
plutôt qu'à découvrir à l'exécution.**

### Réglages du contrôleur, et lequel n'est pas calibré

`agent/src/congestion.rs` : `BPP_MIN` (0,05 bit par pixel et par image) est
**reconduit faute de preuve du contraire, pas confirmé** — le critère qui
l'aurait validé suppose un jugement visuel qui n'a jamais été porté. Il est
**couplé au `fps` de `Config`** (fixé à 60, la cadence délivrée, et non à
`ENCODER_FPS` qui vaut 90 et n'est que la cadence de sollicitation) : les deux
doivent être recalibrés **ensemble**. `DELAI_REMONTEE` a été porté de 10 à 20 s
pendant la recette pour réduire une oscillation, améliorée sans être éliminée.

---

## 🚀 Commandes de Développement Essentielles

### Build & Run

```bash
# Démarrage complet
docker-compose up -d

# Rebuild après changement Dockerfile/package.json
docker-compose up -d --build

# Redémarrer (recompile les assets)
docker-compose restart web

# Arrêter
docker-compose down
```

### Logs & Debugging

```bash
# Suivre les logs en temps réel
docker-compose logs -f web

# Logs guacd seulement
docker-compose logs web | grep guacd

# Logs JavaScript client (ouvrir DevTools navigateur)
# F12 → Console

# Vérifier la liste des apps découvertes
curl http://localhost:3445/apps | jq

# Vérifier une app spécifique
curl http://localhost:3445/apps | jq '.[] | select(.short=="firefox")'
```

### Filesystem & Permissions

```bash
# Vérifier le mount /media/vm
ls -lah /media/vm/

# Vérifier les .lnk shortcuts
ls -lah /media/vm/Users/guacamole/Desktop/*.lnk

# Permissions FUSE (après lancement session)
ls -lah /mnt/ftp-*/

# Démontage manuel FUSE si blocké
sudo fusermount -u /mnt/ftp-<UUID>
```

### Tests WinRM

```bash
# Depuis le container
docker exec -it guacamole-web-1 node test_winrm_nodejs.js

# Tester PowerShell command
docker exec -it guacamole-web-1 node -e "
const winrm = require('nodejs-winrm');
winrm.runCommand('Get-ChildItem C:\\', '192.168.3.2', 'Administrator', 'PASSWORD', 5985, true)
  .then(console.log);
"
```

---

## 📋 Checklist de Déploiement Production

### Sécurité

- [ ] Externaliser tous les credentials (Docker secrets, AWS Secrets Manager)
- [ ] Activer HTTPS/WSS pour tous les WebSockets
- [ ] Ajouter authentification utilisateur
- [ ] Rate limiting sur les endpoints
- [ ] CSP headers correctement configurés
- [ ] Audit logging des connexions RDP
- [ ] Rotation régulière des mots de passe
- [ ] Firewall: whitelist IPs autorisées
- [ ] Disable debug mode in Browserify
- [ ] Review et remove console.log statements

### Performance

- [ ] Activer compression gzip/brotli
- [ ] Mettre en place CDN pour assets statiques
- [ ] Caching HTTP headers appropriés
- [ ] Optimiser taille images (WebP pour icônes?)
- [ ] Minifier JavaScript (uglify, terser)
- [ ] Lazy loading pour liste apps si >50 apps
- [ ] Connection pooling pour WinRM
- [ ] Redis pour session storage (scale horizontal)

### Monitoring

- [ ] Prometheus metrics (connexions actives, latence RDP)
- [ ] Alerting sur crash guacd
- [ ] Logs centralisés (ELK, CloudWatch)
- [ ] Healthcheck endpoint (`/health`)
- [ ] Uptime monitoring
- [ ] Dashboard sessions actives
- [ ] Tracking erreurs client (Sentry)

### Infrastructure

- [ ] Multi-instance guacd avec load balancing
- [ ] Auto-scaling basé sur nombre de sessions
- [ ] Backup régulier de la configuration
- [ ] Disaster recovery plan
- [ ] Documentation ops (runbooks)
- [ ] CI/CD pipeline
- [ ] Environnements staging/prod séparés

---

## 🎯 Roadmap et TODO

### Priorité P0 (Critique - À faire immédiatement)

1. ✅ ~~Fixer sessions qui se ferment après 2 secondes~~ (RÉSOLU)
2. ⬜ **Résoudre crash "double free or corruption"**
   - Investiguer avec guacd en mode DEBUG
   - Améliorer shutdown propre du filesystem FUSE
   - Considérer alternatives (WebDAV, SFTP)
3. ⬜ **Externaliser credentials de production**
   - Créer .env.example
   - Docker secrets pour prod
   - Documentation mise à jour

### Priorité P1 (Haute - Cette semaine)

4. ⬜ **Fix recompilation automatique**
   - Ajouter gulp watch task
   - Ou migrer vers Webpack avec HMR
   - Update README avec workflow dev
5. ⬜ **Améliorer qualité icônes**
   - Implémenter extraction haute-res PowerShell
   - Ou pre-générer côté Windows
   - Fallback gracieux si échec
6. ✅ ~~Améliorer design page d'accueil~~ (RÉSOLU)

### Priorité P2 (Moyenne - Ce mois)

7. ⬜ **Gestion erreurs filesystem**
   - Indicateur visuel si WebSocket fail
   - Message informatif utilisateur
   - Retry logic avec backoff
8. ⬜ **Logging structuré**
   - Winston ou Pino
   - Niveaux: ERROR, WARN, INFO, DEBUG
   - Rotation logs (max size)
9. ⬜ **Détection PWA installées**
   - Écouter événement `appinstalled`
   - Vérifier `display-mode: standalone`
   - Sync avec localStorage
10. ⬜ **Authentification basique**
    - Username/password simple
    - Sessions avec express-session
    - Redirection /login si non auth

### Priorité P3 (Basse - Nice to have)

11. ⬜ Tests unitaires et intégration
12. ⬜ Documentation API complète
13. ⬜ Mode développement avec hot reload
14. ⬜ Support multi-utilisateurs simultanés
15. ⬜ Dashboard admin (stats, sessions actives)
16. ⬜ Enregistrement sessions (recording)
17. ⬜ Clipboard avancé (images, fichiers)
18. ⬜ Print to PDF depuis RemoteApp
19. ⬜ Raccourcis clavier personnalisables
20. ⬜ Thèmes clairs/sombres

---

## 📚 Ressources et Références

### Documentation Officielle

- [Apache Guacamole](https://guacamole.apache.org/doc/gug/)
- [guacamole-lite](https://www.npmjs.com/package/guacamole-lite)
- [MS-SHLLINK Specification](https://docs.microsoft.com/en-us/openspecs/windows_protocols/ms-shllink/)
- [File System Access API](https://developer.mozilla.org/en-US/docs/Web/API/File_System_Access_API)
- [PWA Documentation](https://web.dev/progressive-web-apps/)

### Librairies Utilisées

- [Sharp](https://sharp.pixelplumbing.com/) - Image processing
- [FUSE Native](https://github.com/fuse-friends/fuse-native) - Filesystem
- [WinRM Node.js](https://github.com/mshock/nodejs-winrm) - PowerShell remote
- [ColorThief](https://lokeshdhakar.com/projects/color-thief/) - Color extraction

### Outils de Debug

- Chrome DevTools Protocol pour debugging RDP canvas
- Wireshark pour analyser trafic RDP (port 3389)
- `fusermount -u` pour démontage FUSE propre
- `docker exec -it <container> bash` pour debug interne

---

**Dernière mise à jour**: 21 octobre 2025 (Session de bugfixing complète)

**Contributeurs**:

- Développement initial: [Original dev name]
- Documentation et bugfixes: Session Claude (21 Oct 2025)

**Version du projet**: 1.1 (post-bugfixes)

---

> 💡 **Rappel Important**: Toute nouvelle découverte, bug résolu, configuration importante, ou décision architecturale DOIT être ajoutée à ce fichier pour référence future.
