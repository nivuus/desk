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
| `agent/src/encode.rs` | 1536 | `#[cfg(windows)]`, aucun test |
| `agent/src/windows_source.rs` | 740 | `#[cfg(windows)]`, aucun test |
| `agent/src/wasapi.rs` | 543 | `#[cfg(windows)]`, aucun test |

> `encode.rs` est passé de 1502 à 1536 lignes le 31 juillet 2026 (correctif de
> libération des encodeurs). **Cette croissance de +34 est régulière au regard
> de la règle ci-dessus** : l'addition s'est accompagnée de son extraction —
> l'essentiel de la logique neuve vit dans `agent/src/encode/arret.rs`, et seul
> le câblage est resté dans `encode.rs`.
>
> ⚠️ **`arret.rs` est à 500 lignes exactement : sa marge est NULLE.** Il en
> faisait 457 à sa création ; la revue finale de branche l'a porté à 500 en y
> inscrivant la portée présente et la borne du pire cas de `Drop`, et a dû
> resserrer sa propre rédaction pour ne pas franchir le plafond. **Toute
> addition future à ce fichier appelle une extraction, jamais une compression
> supplémentaire** — la compression y a déjà été jouée, et elle ne l'est qu'une
> fois.

> ⚠️ **`agent/src/capture.rs` est à 485 lignes : sa marge est de 15 lignes**
> (1ᵉʳ août 2026, fin du sous-bloc D2). Il a atteint **exactement 500** en cours
> de sous-bloc ; une extraction vers `capture/ouverture.rs` l'a fait retomber à
> **453**, puis une ronde de correction lui a **rendu 32 lignes**. **Toute
> addition future à ce fichier appelle une extraction** — les modules enfants
> `capture/reprise.rs`, `capture/ouverture.rs` et `capture/types.rs` existent
> déjà et sont le bon endroit. **Leçon du chiffre : la marge regagnée par une
> extraction se reperd à la ronde suivante si on la traite comme acquise.**

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
# `mountpoint -q` ne suffit pas : /media/vm est un montage CIFS
# (//192.168.3.2/c) dont l'entrée persiste dans la table de montage même VM
# éteinte et connexion morte — il faut éprouver un ACCÈS réel, pas la seule
# présence de l'entrée.
until ls /media/vm/dev >/dev/null 2>&1; do sleep 5; done

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

Sonde `AUDIO_PROBE` (`agent/src/wasapi.rs` + `agent/src/diagnostics/audio.rs`), exécutée en
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

### Deux réserves connues, closes par le chantier de découpage des fichiers

Les deux réserves ci-dessous, ouvertes lors du chantier C volet 1, sont
**fermées** : le chantier de découpage de `transport.rs` (fichiers >500
lignes) a ajouté exactement deux tests à cet effet, au titre d'une exception
accordée pour cela — un chantier de découpage n'ajoute normalement pas de
comportement neuf, celui-ci ferme une dette de test constatée au passage.

- Les transitions de `Adaptation` (`Active` → `Indisponible`) et l'expiration
  de l'estimation BWE à 5 s sont désormais couvertes par
  `une_estimation_perimee_bascule_l_adaptation_en_indisponible_et_l_annonce`
  (`agent/src/transport/adaptation.rs`). Le test fait vieillir une estimation
  au-delà d'`EXPIRATION_ESTIMATION`, vérifie le basculement en
  `Adaptation::Indisponible`, que l'absence n'est journalisée qu'une fois, et
  que la décision est relayée au navigateur sous forme de message `Link`
  (`AgentControl::Link { adaptation: LinkAdaptation::Indisponible, .. }`) via
  `act_on_timeout` — puis qu'une estimation fraîche qui revient réarme
  l'annonce.
- Le câblage de `resize` (branche `a1` de `act_on_timeout`, désormais en
  `agent/src/transport/tick.rs`, qui délègue à
  `agent/src/transport/redimensionnement.rs`, laquelle appelle
  `Controleur::changer_source`) est désormais couvert par
  `un_redimensionnement_recalibre_le_controleur_sur_la_taille_obtenue`
  (`agent/src/transport/redimensionnement.rs`). Le test pose une source
  factice dont `resize` réussit mais impose un alignement pair (comme une
  vraie fenêtre Windows), pose un `pending_resize` sur une taille impaire, et
  vérifie que la session retient les dimensions RÉELLEMENT obtenues (pas
  celles demandées), que `encode_size_appliquee` suit cette taille obtenue, et
  qu'un refus de taille antérieur (`taille_refus_signalee`) est effacé par ce
  redimensionnement.

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

`agent/src/congestion/echelle.rs` : `BPP_MIN` (0,05 bit par pixel et par image) est
**reconduit faute de preuve du contraire, pas confirmé** — le critère qui
l'aurait validé suppose un jugement visuel qui n'a jamais été porté. Il est
**couplé au `fps` de `Config`** (fixé à 60, la cadence délivrée, et non à
`ENCODER_FPS` qui vaut 90 et n'est que la cadence de sollicitation) : les deux
doivent être recalibrés **ensemble**. `DELAI_REMONTEE` a été porté de 10 à 20 s
pendant la recette pour réduire une oscillation, améliorée sans être éliminée.

---

## 🌐 Chantier C volet 2 — traversée NAT (30 juillet 2026)

Recette complète : `docs/superpowers/plans/2026-07-29-traversee-nat-resultats.md`.

L'agent est un client TURN à part entière (`agent/src/turn/`) : il alloue un
relais **avant** de répondre à l'offre, publie le candidat relayé et le candidat
réflexif, encapsule en ChannelData ce qui doit passer par le relais, et
rafraîchit son bail. Le signaling délivre aux deux pairs des identifiants
éphémères (`signaling/src/ice.ts`) dérivés d'un secret qui ne quitte jamais le
serveur. Mesuré : le média traverse un relais réel pour **≈2 ms de RTT en plus**
(2,0 → 4,0 ms), sans perte de cadence.

### Lancer coturn : deux fichiers compose, pas un

`docker-compose.yml` est **gitignoré** (il porte les mots de passe Windows du
Guacamole historique en clair). Le relais vit donc dans un fichier séparé et
versionné, sans aucun secret :

```bash
docker compose -f docker-compose.yml -f docker-compose.coturn.yml up -d coturn
docker compose -f docker-compose.yml -f docker-compose.coturn.yml logs coturn
```

Variables à poser dans `.env` : `TURN_SECRET` (`openssl rand -hex 32`),
`TURN_REALM`, `TURN_EXTERNAL_IP`, et `TURN_URL` (lue par le **signaling**, sans
laquelle il n'annonce aucun relais et le journalise).

**coturn écoute sur toutes les interfaces de l'hôte, dont l'adresse publique** —
constaté (`UDP listener opened on: 90.87.35.18:3478`). L'accès est authentifié et
les identifiants expirent, mais c'est un relais joignable depuis Internet : à
restreindre (`--listening-ip` ou pare-feu) avant tout déploiement durable.

### Le signaling doit être relancé AVEC l'environnement

Piège rencontré : un serveur de signaling tournait depuis 36 h sans les variables
TURN, et les sessions ne recevaient donc aucune configuration ICE — sans que rien
ne le signale côté client. Vérifier l'environnement du processus **qui écoute
réellement**, pas de celui qu'on croit avoir lancé :

```bash
P=$(ss -ltnp | grep ':8080 ' | grep -o 'pid=[0-9]*' | cut -d= -f2 | head -1)
tr '\0' '\n' < /proc/$P/environ | grep ^TURN_URL=
```

### Relever PAR OÙ passe le flux, pas seulement qu'il passe

`client/verify-webrtc.mjs` prouve que le média traverse, jamais par quel chemin.
`client/recette/paire-candidats.mjs` relève la paire de candidats réellement
employée, le type des deux candidats, le RTT et le débit :

```bash
node client/recette/paire-candidats.mjs 'http://127.0.0.1:5174/?session=demo' 10000
FORCER_RELAIS=1 node client/recette/paire-candidats.mjs   # iceTransportPolicy 'relay'
```

`FORCER_RELAIS` intercepte le constructeur `RTCPeerConnection` dans la page :
aucune modification du code client à committer puis retirer.

### `relay ↔ relay` est inatteignable en laboratoire — ce n'est pas un défaut

Sur un pont où les deux pairs se voient, la paire nominée est toujours
`relay ↔ host` : pour que le relais de l'agent fonctionne, coturn doit pouvoir
joindre l'agent, et cette même joignabilité valide la paire `host`, prioritaire
en ICE. Des règles de pare-feu Windows bloquant l'UDP direct n'y changent rien
(le trafic relayé arrive depuis la plage de relais, donc autorisé). **Prouver le
chemin relayé côté agent pour le média exige deux réseaux réellement distincts.**
Le chemin d'encapsulation est en revanche exercé et prouvé pour les contrôles de
connectivité ICE (`CREATE_PERMISSION` et `CHANNEL_BIND` acceptés par coturn).

### Trois objets TURN expirent, pas un seul

Le bail de l'**allocation** (600 s) n'est pas le seul minuteur. Une
**permission** dure 300 s (RFC 5766 §8) et une **liaison de canal** 600 s (§11).
Passé ces délais, le serveur cesse de relayer **sans rien annoncer** — ni erreur,
ni message : une session relayée mourrait d'un silence au bout de 5 minutes.

Le plan du chantier ne prévoyait que le bail. `agent/src/turn/canaux.rs`
réaffirme désormais chaque liaison à 150 s (moitié de la permission, la plus
courte des trois durées). Toute évolution du client TURN doit préserver ces
**trois** rafraîchissements.

**Piège lié, qui a coûté une seconde mesure** : `handle_packet` reconduisait le
bail à *chaque* réponse de succès du serveur. Or ni `CreatePermission` ni
`ChannelBind` n'en portent — leurs réponses, arrivant toutes les 150 s,
repoussaient sans fin un rafraîchissement dû à 300 s, et l'allocation mourait à
600 s. Seules les réponses à `Allocate` et `Refresh` reconduisent un bail : la
méthode se lit avec `messages::methode_de`, qui défait l'entrelacement
classe/méthode de la RFC 5389 §6.

Trois traces `info` rendent tout cela observable (`état du client TURN` une fois
par minute, émission du bail, réaffirmation d'un canal) : c'est par elles que le
diagnostic a été fait, elles sont rares par construction — ne pas les retirer.

### Une page Chrome sans interface gèle au bout de 5 minutes

Piège de recette, coûteux : deux sessions de 11 minutes se sont interrompues à
331 s et 340 s — soit 300 s plus le délai de révocation du consentement ICE. Ce
n'était ni le produit ni le relais (la session **directe** tombait pareil), mais
Chrome qui gèle une page jamais mise au premier plan.

Toute mesure de plus de 5 minutes doit lancer Chrome avec
`--disable-background-timer-throttling`,
`--disable-backgrounding-occluded-windows` et
`--disable-renderer-backgrounding` (posées dans
`client/recette/paire-candidats.mjs` ; `client/verify-webrtc.mjs` ne les a pas,
ses mesures ne dépassant pas 25 s).

**Leçon de méthode :** le délai collait si bien au minuteur des permissions TURN
que la cause a d'abord été imputée au relais, à tort. Une mesure témoin sur le
chemin *sans* la fonctionnalité suspecte coûte dix minutes et évite une
conclusion fausse.

### Ne jamais tracer par paquet dans la boucle de transport

Une session relayée a échoué à s'établir uniquement parce que le binaire portait
encore une trace `tracing::info!` par `Transmit` — 18 619 lignes en quelques
secondes, écrites sur un partage CIFS depuis la boucle. **La mesure détruisait ce
qu'elle mesurait.** Compter ou échantillonner, jamais tracer par paquet.

### Réserve connue : pas d'appariement des transactions

`TurnClient::handle_packet` (`agent/src/turn/allocation.rs`) accepte la réponse
du serveur sans vérifier que son identifiant de transaction apparie la requête en
cours. Les `trans_id` sont retenus dans `Etat` — c'est là que l'appariement se
brancherait — mais jamais relus. Une réponse tardive ou rejouée fait donc avancer
la machine à états.

---

## 🪟 Sonde de capture multi-fenêtres (30 juillet 2026)

Résultats complets :
`docs/superpowers/plans/2026-07-30-sonde-capture-multifenetre-resultats.md`.
Journaux bruts : `docs/superpowers/plans/journaux-sonde-multifenetre/`, accents
corrompus en amont par la page de code PowerShell. **Encodages mixtes, pas
tous UTF-8** : `dxgi.log`, `wgc.log`, `replis.log` et `nvenc.log` sont en
**UTF-16LE** (`iconv -f UTF-16LE -t UTF-8 fichier.log` pour les lire ou les
`grep` — un `grep` direct dessus ne trouve rien) ; les journaux `banc-*.log`
sont en UTF-8 (avec BOM). **Cette conversion ne concerne QUE ce répertoire** :
`scripts/run-agent.sh` a été corrigé le 31 juillet 2026 et les journaux de
`journaux-mesures-prealables/` sont tous en UTF-8 sans BOM, accents intacts,
`grep`-ables tels quels.

Chantier de **mesure sans livrable produit** : trancher, avant de spécifier le
chantier D, quelle voie de capture rend une image correcte **par fenêtre** quand
les fenêtres se recouvrent. Le banc vit dans
`agent/src/diagnostics/multifenetre/`, piloté par variables d'environnement
(`MULTIFENETRE_DXGI`, `_WGC`, `_REPLIS`, `_BANC` + `MULTIFENETRE_N`).

### Ce qui est mesuré — aucune voie n'est simplement viable

| Voie | Verdict |
| --- | --- |
| `Windows.Graphics.Capture` | **ÉLIMINÉE** — `CreateForWindow` en `0x800706BE` (`RPC_S_SERVER_UNAVAILABLE`), 3 reproductions. `IsSupported()` rend `true` et le service `CaptureService_5865d` tourne : ni composant absent, ni service arrêté |
| Un moniteur virtuel par fenêtre | **CONDITIONNELLE** — mécanisme établi, sortie DXGI réelle 3413×960 sur l'adaptateur qui porte NVENC (relevé sans journal joint — session Apollo non reproductible sans le propriétaire du poste) ; **plafond non mesuré** (un seul client Apollo apparié) |
| Tuilage disjoint | **CONDITIONNELLE** — 800×360 à 8 fenêtres, et surtout : **les menus débordent** de ≈284 px sur la tuile voisine. La voie censée garantir le non-recouvrement ne le garantit pas |
| `PrintWindow(PW_RENDERFULLCONTENT)` | **CONDITIONNELLE** — rend l'image **juste** d'une fenêtre D3D **recouverte** (contre-intuitif, vérifié) ; limite = **chemin CPU**, 29,1 i/s/fenêtre à N=2 |

**La capture ne décroche pas jusqu'à 8 fenêtres** (voie `duplication`, cadence
par fenêtre : 80,2 / 99,4 / 98,8 / 107,5 i/s à N = 1 / 2 / 4 / 8, toutes égales
entre elles) — **mais à aire totale fixe** : `disposition::tuiles` découpe le
bureau, donc la surface par fenêtre décroît quand N croît (débit de pixels
quasi constant, ~208-258 MP/s, par construction). Ce montage ne dit rien du cas
où N fenêtres garderaient chacune sa résolution utile (8×1280×720 = 2,8× le
bureau mesuré) : le nombre de fenêtres n'est pas prouvé neutre en soi. Le banc
est validé par le fait que la voie de production **se pollue bien** sous
recouvrement (`verdicts_faux` = 449 / 449 / 536 à N = 2/4/8), ce qui **coupe la
passe d'encodage** — il n'existe donc aucune mesure d'encodage multi-fenêtres
par cette voie, par construction du protocole.

> ✅ **Note du 31 juillet 2026 — les deux réserves du paragraphe ci-dessus sont
> levées** par le chantier des duplications parallèles (section « N duplications
> DXGI de front » plus bas). **Le cas « N fenêtres gardant chacune leur
> résolution utile » a été exercé** : 8 sorties de 1280×720, aire totale
> multipliée par 8 (0,92 → 7,37 Mpx), cadence par fenêtre inchangée (90,1 i/s).
> Et **des mesures d'encodage multi-fenêtres existent désormais** — quatre rangs,
> jusqu'à N=8, zéro verdict faux. Ce montage-là n'a pas de recouvrement (une
> fenêtre par sortie), donc pas de porte éliminatoire : ce n'est pas le montage à
> aire fixe ci-dessus, et **les deux séries ne se comparent d'aucun chiffre**.

**Plafond d'encodage, formulation bornée, composant du refus non identifié** :
sur un périphérique D3D11 **unique et partagé**, à 720p/60/8 Mb/s, 8 instances
du pipeline Media Foundation réussissent, la 9ᵉ échoue à la **liaison du type
d'entrée** (`MF_E_UNSUPPORTED_D3D_TYPE`). Ce n'est **pas** « la limite NVENC de
cette carte » — le transform n°9 s'instancie sans peine. Deux appels
`SetInputType` s'enchaînent entre le succès et l'échec (encodeur H.264 puis
Video Processor MFT du convertisseur de couleur) et rien n'indiquait lequel
refusait — corrigé par un `.context()` distinct sur chacun
(`agent/src/encode.rs`), pour que la prochaine mesure tranche.

> ⚠️ **Corrigé le 31 juillet 2026 : ce n'était aucun de ces deux
> `SetInputType`.** La mesure ② du chantier de mesures préalables (rapport
> `task-9-report.md`, journaux `docs/superpowers/plans/journaux-mesures-prealables/nvenc-*.log`)
> a relevé une chaîne de causes **nue** avec ces deux contextes déjà en place :
> l'appel fautif était un `?` sans contexte, à savoir **`SetOutputType` de
> l'encodeur H.264**. Le libellé de `MF_E_UNSUPPORTED_D3D_TYPE` parle du type
> d'**entrée** alors que l'appel refusé règle le type de **sortie** — c'est très
> probablement ce libellé qui avait égaré l'attribution ci-dessus. **Ne pas se
> fier au texte de ce HRESULT pour désigner un appel.** Dix appels du chemin de
> construction portent désormais un contexte distinct.

**Le plafond ne vient pas du partage du périphérique** (mesure ② du même
chantier) : avec **un périphérique D3D11 neuf par encodeur**, le plafond reste
**8**, refus au même `SetOutputType`. Cette attribution causale porte une
réserve : **comparaison à deux variables confondues**, le mode séparé n'ouvrant
aucune duplication DXGI là où le mode partagé en ouvre une — il faudrait une
coïncidence pour que deux effets se compensent exactement, mais le témoin propre
(mode séparé *avec* duplication) n'a pas été exercé. Sous cette réserve, séparer
les périphériques ne fait gagner aucune fenêtre — et le coût correspondant
(partager des textures entre le périphérique de capture et ceux des encodeurs)
n'a donc pas à être payé. Ce que la mesure **ne** dit **pas** non plus : quelle
couche impose ce plafond (NVENC, pilote, Media Foundation, ou virtualisation),
s'il tient à d'autres résolutions ou débits, et si 8 encodeurs tiennent la
cadence *ensemble* — aucune image n'a été soumise, seule la **construction** est
mesurée. Enfin, détruire un encodeur n'a **pas** été montré libérer la place :
la mise en sommeil des fenêtres masquées reste à éprouver par une séquence
« créer 8 → en détruire 1 → tenter un 9ᵉ ».

> ✅ **Note du 31 juillet 2026 — « si 8 encodeurs tiennent la cadence ensemble »
> est répondu : ils la tiennent.** Huit encodeurs alimentés de front pendant
> 10 s rendent **90,1 i/s par fenêtre** en capture+encodage, zéro verdict faux
> (chantier des duplications parallèles, section plus bas). Portée exacte : sur
> **huit périphériques D3D11 distincts**, 1280×720 à 60 Hz et 8 Mb/s, **une**
> exécution par rang, unités H.264 comptées et jamais décodées. Le montage de la
> mesure ② — **périphérique unique partagé** — n'a, lui, toujours jamais été
> alimenté. **Les autres réserves de ce paragraphe restent entières** : la couche
> qui impose le plafond de 8 n'est pas identifiée, rien n'est su d'autres
> résolutions ou débits, et la mise en sommeil des fenêtres masquées demeure une
> conjecture — « créer 8 → en détruire 1 → tenter un 9ᵉ » n'a toujours pas été
> jouée.

**Voie recommandée** : le moniteur virtuel par fenêtre (seule voie qui préserve
le chemin GPU en supprimant le recouvrement par construction), avec `PrintWindow`
en repli mesuré pour les fenêtres non-jeu. Le relevé Apollo (3413×960) qui fonde
cette voie n'a pas de journal joint et n'est pas reproductible sans le
propriétaire du poste.

> ⚠️ **Recommandée, et éprouvée le 1ᵉʳ août 2026 en conditions de produit
> (sous-bloc D1) : elle tient à arrangement FIGÉ et s'effondre dès qu'une
> fenêtre s'ouvre.** Créer une sortie virtuelle fait abandonner le mutex des
> duplications DXGI déjà ouvertes, donc tue toutes les captures en cours. Voir
> la section « Sous-bloc D1 » plus bas.
>
> ✅ **CETTE PHRASE N'EST PLUS VRAIE — le sous-bloc D2, le même jour, a réparé
> ce défaut et l'a démontré réparé en conditions de produit.** L'abandon du
> mutex se produit toujours (il n'est ni évité ni expliqué), mais il est
> désormais **encaissé** : la duplication est relâchée puis rouverte dans une
> fenêtre de reprise, et **44 pertes d'accès n'ont tué aucune session**. La voie
> tient donc à arrangement **dynamique** — jusqu'à **quatre** fenêtres
> simultanées, où un **plafond distinct** apparaît (la 5ᵉ duplication DXGI, dans
> un 5ᵉ processus, est refusée en `0x887A0022` ; **la couche qui l'impose n'est
> pas identifiée**). Voir la section « Sous-bloc D2 » plus bas.

> ✅ **Les deux mesures que cette sonde déclarait bloquantes ont été prises le
> 31 juillet 2026** (plafond de sorties virtuelles, plafond d'encodage sur
> périphériques séparés) — voir la section suivante. Le chantier D est
> spécifiable. Le repli `PrintWindow`, lui, **recule** : mesuré à N=4 et N=8, il
> ne tient pas la cible de 8 fenêtres.

### Pièges — à connaître avant de toucher à ce terrain

- **Animer les mires.** Desktop Duplication n'émet une trame **qu'au changement
  du bureau**. Une mire immobile fait rendre `WAIT_TIMEOUT` à toutes les
  acquisitions : on mesure zéro image et on conclut à tort à une panne.
- **Peindre en D3D11, jamais en GDI.** `PrintWindow` sait faire redessiner une
  fenêtre par `WM_PRINT` : sur une mire GDI il rendrait toujours une image
  juste, et la voie 4 serait **validée à tort**. Le banc peint par chaîne
  d'échange D3D11 (`mires.rs`), et c'est ce qui donne son poids au résultat.
- **Un processus par voie.** Ces API échouent par **plantage du processus**
  (`0xc0000005` vu au jalon 1), pas par code d'erreur : une sonde monolithique
  perd toutes les mesures déjà faites.
- **DXGI n'autorise qu'UNE seule duplication ouverte par sortie** — exactement
  une, pas « un nombre très limité ». Un `DesktopCapture` provisoire laissé en
  vie fait échouer la suivante en `0x80070057`.
- **Ne pas refondre acquisition et recadrage dans le trait `VoieDeCapture`.**
  C'est la lacune du plan initial : la première voie du tour consommait
  l'`AcquireNextFrame`, les autres récoltaient `WAIT_TIMEOUT` — **famine dès
  deux fenêtres**, et le plan désignait ce trait comme la couture destinée au
  chantier D : la lacune serait allée en production. Corrigé : acquisition
  mutualisée une fois par tour (`SourceDuplication::amorcer`), **une texture de
  destination par voie** (sans quoi les voies s'écrasent mutuellement).
- **WMI ment sur la résolution** : champ vu périmé de 68 s. Source de vérité =
  `GetDesc`/`DesktopCoordinates`. Corollaire : la sortie virtuelle est annoncée
  5120×1440 par WMI mais mesurée **3413×960** (rapport 1,5 = DPI 150 %) — si un
  recadrage est calculé sur le rectangle virtualisé alors que la texture est aux
  dimensions physiques, il sera décalé d'un facteur 1,5.
- **Leçon de méthode** : les onze rondes de correction de ce chantier ont
  quasiment toutes porté sur des **rapports qui affirmaient au-delà de leur
  relevé**, jamais sur des bugs. Sur un chantier de mesure, le coût est dans la
  discipline de l'énoncé, pas dans le code.

---

## 📐 Mesures préalables au chantier D (31 juillet 2026)

Résultats complets :
`docs/superpowers/plans/2026-07-31-mesures-prealables-chantier-d-resultats.md`.
Journaux : `docs/superpowers/plans/journaux-mesures-prealables/` — **tous en
UTF-8 sans BOM, accents intacts, aucune conversion nécessaire** (contrairement
à ceux de la sonde ci-dessus). Le document de reconnaissance du canal de
contrôle du pilote y vit aussi : `canal-de-controle.md`.

Second **chantier de mesure sans livrable produit** : lever les quatre inconnues
que la sonde ci-dessus laissait ouvertes. **Les quatre sont levées, chacune avec
son journal versé** — la faiblesse que la sonde avait laissée sur son propre
chiffre fondateur. **Le chantier D est spécifiable.**

### Les quatre chiffres

| # | Résultat | Journal |
| --- | --- | --- |
| ① | **Plafond de sorties virtuelles = 10.** Refus du pilote à la 11ᵉ création (IOCTL `0x00222000`, `0x80070044` / `ERROR_TOO_MANY_NAMES`), **prouvé par identité** — les onze sorties sont énumérées nommément par la ligne de refus (liste mémorisée au relevé qui précède, ouvert 0,673 ms plus tôt, **pas** une relecture fraîche : le libellé du journal suggère le contraire). Cible du chantier D = 8 : la voie tient avec 2 de marge | `moniteurs-montee-en-n.log` |
| ② | **Plafond d'encodage = 8, inchangé sur périphériques D3D11 séparés.** 9 périphériques distincts et vivants côté mode `separe`, même rang refusé. **Le partage du périphérique n'était donc pas la contrainte** | `nvenc-partage-temoin.log`, `nvenc-separe.log` |
| ③ | **Windows compose bien sur un moniteur virtuel sans écran physique**, et Desktop Duplication en rend l'image exacte : 900/900 verdicts justes, **zéro image noire**, 90,0 i/s/fenêtre. L'hypothèse fondatrice de la voie recommandée tient | `moniteurs-capture.log`, `moniteurs-capture-n1-encodage.log` |
| ④ | **`PrintWindow` ne tient pas l'échelle** : 17,6 i/s/fenêtre à N=4, **8,8 à N=8** (17,2 et 8,8 avec l'encodage). Recollé aux deux rangs de la sonde, le débit de pixels décroît **sur les quatre rangs sans palier** : **116,64 → 75,43 → 45,62 → 20,28 MP/s**, à opposer aux 208–258 MP/s quasi constants de `duplication` | `printwindow-n4.log`, `printwindow-n8.log` ; N=1 et N=2 : `journaux-sonde-multifenetre/banc-printwindow-{1,2}.log` |

**Acquis d'outillage** : notre code **commande le pilote SudoVDA lui-même** (canal
IOCTL, `canal-de-controle.md`) — plus besoin d'une session Apollo pour faire
paraître une sortie virtuelle ; une **purge autonome** rattrape les sorties
orphelines, éprouvée sur un état réellement sale (8 orphelines, constat et
confirmation par des processus tiers).

### Ce que ces mesures NE disent pas

- **L'arrangement que la voie recommandée propose réellement n'est pas mesuré
  ICI** : N sorties virtuelles, **une fenêtre chacune**, donc **N duplications
  DXGI de front**. Le banc de ce chantier a posé N fenêtres sur **UNE** sortie.
  DXGI n'autorisant qu'une duplication par sortie, la question était réelle.
  ✅ **Elle a été mesurée le 31 juillet 2026 — voie reçue à N=8** : voir la
  section « N duplications DXGI de front » ci-dessous.
  ⚠️ **Mais dans un ORDRE que le produit n'a pas** : le banc créait ses N sorties
  avant d'ouvrir la moindre duplication. Le sous-bloc D1 a exercé l'ordre réel
  (une fenêtre s'ouvre pendant que d'autres capturent) et il échoue — voir la
  section « Sous-bloc D1 ».
  ✅ **L'ordre réel passe depuis le sous-bloc D2** (1ᵉʳ août 2026) : la reprise
  sur perte d'accès encaisse la perturbation, éprouvée au banc à k = 1, 2, 4
  puis en conditions de produit jusqu'à **quatre** fenêtres. **Rien au-delà de
  quatre** : un plafond distinct, sur le nombre de duplications DXGI
  simultanées **dans des processus distincts**, y arrête la montée — voir la
  section « Sous-bloc D2 ».
- **La comparaison des deux modes d'encodage porte sur DEUX variables
  confondues** : le mode `separe` n'ouvre aucune duplication DXGI là où
  `partage` en ouvre une. Le témoin propre n'a pas été exercé.
- **La couche qui impose le plafond de 8 n'est pas identifiée**, aucune image
  n'a été soumise (seule la *création* est mesurée), et **détruire un encodeur
  n'a pas été montré libérer la place** — la mise en sommeil des fenêtres
  masquées repose donc sur une conjecture.
  ✅ **Des images ont depuis été soumises** (31 juillet 2026) : 8 encodeurs
  alimentés ensemble tiennent 90,1 i/s par fenêtre — mais sur huit
  périphériques D3D11 **distincts**, pas sur le périphérique unique partagé de
  cette mesure-ci. **La couche du plafond et la mise en sommeil restent, elles,
  entièrement ouvertes.**
- **La cause du refus à la 11ᵉ sortie n'est pas isolée**, et on ignore si le
  vivier de 10 est global au pilote ou par client (Apollo pingue le même
  pilote). **L'unité du chien de garde (`delai = 3`) reste inconnue : aucune
  unité n'est exclue, pas même la seconde.**

### ⚠️ Défaut alors ouvert — depuis diagnostiqué et corrigé (31 juillet 2026)

> ✅ **Ce défaut est traité** : diagnostic, réfutation de `MFShutdown` et
> correctif au chantier « N duplications DXGI de front » ci-dessous, §7 de
> `docs/superpowers/plans/2026-07-31-duplications-paralleles-resultats.md`.
>
> ⚠️ **Et le bornage ci-dessous porte une erreur à ne pas reprendre : il laisse
> croire à un défaut DÉTERMINISTE.** « Les deux exécutions » ne dit pas combien
> avaient passé. Il est **intermittent** — 2 plantages sur 6 exécutions du cas
> comparable, et un rapport antérieur avait eu **quatre exécutions propres
> d'affilée sur un binaire non corrigé**. Un défaut intermittent qu'on croit
> déterministe se déclare « corrigé » à la première exécution qui passe.

**La passe d'encodage du banc tue le processus, sur la voie `duplication`.**
Bornage exact, à ne pas élargir :

- **à la SORTIE de la boucle, pas pendant** — les deux exécutions écrivent leur
  dixième et dernière ligne périodique à début + 10,00 s, et le bilan n'est
  jamais atteint. Cela désigne la **libération** du `Vec<H264Encoder>` et de la
  duplication, **pas** la soumission d'images ;
- **quelle que soit la sortie capturée** : le même banc sur le **bureau
  physique** meurt au même endroit — la sortie virtuelle est hors de cause ;
- **pas sur `printwindow`** : `printwindow-n4.log` et `printwindow-n8.log`
  portent tous deux leur ligne `passe terminée passe="capture+encodage"` ;
- **un encodage doit probablement avoir réellement eu lieu** : la mesure ②
  forme le couple duplication + encodeurs et **survit** à la destruction de ses
  8 encodeurs — mais sans avoir jamais soumis d'image.

Conséquence : la garde ne court pas, donc **la sortie virtuelle survit au
processus** (`MULTIFENETRE_VDD_PURGE=1` pour la retirer).

### Pièges neufs — à connaître avant de toucher à ce terrain

- **Un journal PowerShell lisible demande DEUX réglages, pas un.** Le mojibake
  des journaux de la sonde ne venait pas seulement de `Tee-Object` en UTF-16LE,
  mais AUSSI de `[Console]::OutputEncoding` resté sur la page de code OEM, qui
  abîmait les accents **en lisant** la sortie du processus enfant, avant même
  l'écriture. Le `StreamWriter` règle l'écriture, `[Console]::OutputEncoding` la
  lecture. Symptôme : un `grep` sur un mot accentué rend 0 quand le même `grep`
  sur sa partie ASCII rend 1. *(Ne pas remplacer `Tee-Object` par
  `Out-File -Encoding utf8` : il replie les lignes à la largeur de console.)*
- **Ne jamais se fier au texte d'un HRESULT pour désigner un appel.** Voir le
  bloc de correction d'attribution ci-dessus : dix annotations de contexte ont
  été nécessaires pour savoir quel appel refusait le 9ᵉ encodeur.
- **Un compteur ne suffit pas quand un tiers agit sur le système.** Apollo peut
  ajouter une sortie à tout instant : une addition externe compense exactement
  un retrait, et un contrôle par cardinal passe alors qu'une sortie a disparu.
  **Comparer des ensembles de noms, jamais des nombres.**
- **Le contrôle qui vaut se fait depuis un processus NEUF.** Le processus
  mesureur est juge et partie, et une sortie virtuelle lui survit.
- **Un plantage à la destruction se lit comme un plafond.** Instrumenter la
  sortie autant que l'entrée (deux traces encadrant le relâchement).
- **Modifier le banc rend les mesures antérieures non comparables** — et il faut
  le dire : la lecture de pixel a changé de portée en cours de chantier, les
  cadences `printwindow` viennent d'un banc qui relisait deux fois moins. Risque
  borné par un témoin, **borné n'est pas nul**.
- **Un compte de créations obtenu par minutage est un artefact de minutage.**
  L'épreuve de salissure a produit 8 sorties, pas 10, parce que le `sleep` de
  l'hôte ne mesure pas le temps de vie de l'agent. Le plafond reste 10.
- **Le mode de défaillance dominant reste l'énoncé, pas le code** — comme pour
  la sonde. Sur tout le chantier, **un seul** point a vu le code contredire son
  rapport ; tout le reste était des phrases qui affirmaient au-delà du relevé.

### Variables d'environnement du banc et des sondes

Un processus par voie (ces API échouent par **plantage du processus**, pas par
code d'erreur).

| Variable | Effet |
| --- | --- |
| `MULTIFENETRE_DXGI=1` | Relève la topologie DXGI et sort — le contrôle d'état depuis un processus neuf |
| `MULTIFENETRE_WGC=1` | Sonde `Windows.Graphics.Capture` (voie éliminée) |
| `MULTIFENETRE_REPLIS=1` | Sonde les voies de repli |
| `MULTIFENETRE_BANC=duplication\|printwindow` + `MULTIFENETRE_N=1..8` | Le banc de cadence |
| `MULTIFENETRE_SORTIE=<adaptateur:sortie>` | Force la sortie DXGI capturée par le banc |
| `MULTIFENETRE_CONTRAT=1` | Éprouve le contrat IOCTL du pilote (deux tampons simples, sans effet de bord) |
| `MULTIFENETRE_VDD=1` | **Mesure ①** — montée en N de sorties virtuelles jusqu'au refus |
| `MULTIFENETRE_VDD_VEILLE=<secondes>` | Épreuve du chien de garde : une sortie, aucun ping, relevé à 1 Hz |
| `MULTIFENETRE_VDD_PURGE=1` | **Purge autonome** des sorties orphelines |
| `MULTIFENETRE_VDD_CAPTURE=1` | **Mesure ③** — crée une sortie virtuelle et y lance le banc |
| `MULTIFENETRE_NVENC=partage\|separe` | **Mesure ②** — plafond d'encodeurs, périphérique D3D11 partagé ou un par encodeur |
| `MULTIFENETRE_VDD_PARALLELE=<1..8>` | **Chantier des duplications parallèles** — N sorties virtuelles × 1 fenêtre × 1 duplication DXGI × 1 encodeur, trois passes (témoin, capture, capture+encodage), contrôle d'image **en rotation**, chien de garde pingué à 1 Hz |
| `MULTIFENETRE_EPREUVE_FILE_MS=<ms>` | Bouche la file de travail sérialisée imposée à la MFT pendant la passe — c'est l'épreuve qui montre que la barrière n'est pas un placebo |
| `MULTIFENETRE_REPRISE=<k>` | **Sous-bloc D2** — *k* sorties virtuelles, *k* duplications, puis **une sortie de plus** créée en cours de capture : éprouve que les *k* duplications reprennent et rendent encore des images justes. Sonde post-mortem sur les voies mortes |
| `AGENT_TRACE_EXCEPTIONS=1` | Arme le filtre d'exception (pile symbolisable de la faute, journal séparé d'`agent.log`). Inerte sans la variable. `…_FICHIER` en change la destination ; `…_AUTOTEST=1` **tue délibérément le processus** pour éprouver l'instrument |

---

## 🖥️🖥️ N duplications DXGI de front sur N sorties virtuelles (31 juillet 2026)

Résultats complets :
`docs/superpowers/plans/2026-07-31-duplications-paralleles-resultats.md`.
Conception : `docs/superpowers/specs/2026-07-31-duplications-paralleles-design.md`.
Journaux : `docs/superpowers/plans/journaux-duplications-paralleles/` — **tous en
UTF-8**, accents `grep`-ables tels quels.

Troisième **chantier de mesure**, qui prend la seule mesure que les mesures
préalables laissaient due : **l'arrangement que la voie recommandée du chantier D
propose réellement** — une sortie virtuelle par fenêtre, une duplication DXGI par
sortie, un encodeur par sortie. Tout ce qui avait été mesuré jusque-là l'avait été
sur un montage qui n'est pas celui-là.

### Le résultat : **voie REÇUE**

> ⚠️ **Reçue par ce banc, et par lui seul.** Il crée ses N sorties virtuelles
> **avant** d'ouvrir la moindre duplication. Le sous-bloc D1 (1ᵉʳ août 2026) a
> exercé l'ordre du produit — une fenêtre s'ouvre alors que d'autres capturent —
> et **il échoue** : la création de la sortie fait abandonner le mutex des
> duplications ouvertes et tue toutes les sessions. Rien ci-dessous n'est
> réfuté ; c'est la portée qui est plus étroite qu'il n'y paraît.
>
> ✅ **L'ordre du produit passe depuis le sous-bloc D2 (1ᵉʳ août 2026)** : le
> mutex est toujours abandonné, mais la reprise l'encaisse et **aucune session
> n'en meurt**. ⚠️ **Ce qui reste plus étroit qu'il n'y paraît** : ce banc tient
> **8 duplications de front dans UN SEUL processus** ; en **processus distincts**
> — l'arrangement du produit — la **5ᵉ** est refusée (`0x887A0022`). Que la
> différence tienne au multi-processus est une **INFÉRENCE** : rien ne rapproche
> formellement les deux montages.

Le critère posé d'avance était : à N=8, **≥ 60 i/s par fenêtre en
capture+encodage et aucun verdict faux**.

| N | cadence/fenêtre, capture+encodage | verdicts faux | aire totale | débit de pixels **calculé** |
| --- | --- | --- | --- | --- |
| 1 | 90,1 i/s (`paralleles-n1.log:55`) | 0 | 0,92 Mpx | 83,0 MP/s |
| 2 | 90,1 i/s (`paralleles-n2.log:68`) | 0 | 1,84 Mpx | 166,1 MP/s |
| 4 | 90,1 i/s (`paralleles-n4.log:94`) | 0 | 3,69 Mpx | 332,1 MP/s |
| **8** | **90,1 i/s** (`paralleles-n8.log:146`) | **0** | **7,37 Mpx** | **664,3 MP/s** |

**90,1 i/s exactement sur les quinze voies des quatre rangs** en capture+encodage
(90,0–90,1 en capture nue), soit **1,50 fois le seuil** au rang du critère. Huit
duplications ouvertes de front (`paralleles-n8.log:78`), huit encodeurs matériels
NVENC construits sans refus (l. 93 à 114), huit périphériques D3D11 tenus pour
distincts (l. 47 à 75, tous `protection_precedente=false` — **inférence** sur la
sémantique de `SetMultithreadProtected`, aucun relevé de huit pointeurs
distincts), topologie rendue **nom pour nom** à son état de départ, zéro
`ERROR`, zéro `WARN`, **aucun rang rejoué**.

Deux réserves que ce tableau ne porte pas, et qu'il ne faut pas perdre en le
recopiant (§6 des résultats) :

- **la restauration « nom pour nom » n'est pas une preuve d'absence d'effet
  résiduel** — elle porte sur l'ensemble des **noms** de sorties attachées, et
  sur rien d'autre : ni mémoire, ni état du pilote, ni ressources DXGI ;
- **la fraîcheur du binaire mesuré n'est adossée à aucune pièce versée.** La
  sortie de `scripts/build-agent.sh` a été recopiée d'un terminal, jamais
  capturée dans un fichier. Vérifiable depuis le dépôt, et rien de plus :
  `git status --porcelain agent/ scripts/` vide à `db8cc85`, et la date du
  binaire sur la VM antérieure de 1 min 40 s à l'horodatage du commit —
  **cohérent** avec un binaire bâti sur ces sources, sans le prouver.

### Ce que ce montage a de neuf : l'aire croît avec N

**Tous les bancs antérieurs de ce projet mesuraient à aire totale fixe.**
`disposition::tuiles` découpe le bureau : le débit de pixels y est quasi constant
*par construction*, et le nombre de fenêtres n'y est **pas prouvé neutre en
soi**. Ici chaque fenêtre a sa sortie de 1280×720, facteur d'échelle 1 (pas de
piège DPI). **Le fait de ce chantier se lit entièrement dans sa propre série** :
l'aire totale est multipliée par 8 de N=1 à N=8 (0,92 → 7,37 Mpx, **relevé**) et
la cadence par fenêtre ne bouge pas (90,1 i/s aux quatre rangs, **relevé**) ; le
débit de pixels correspondant, **calculé**, va de 83,0 à 664,3 MP/s.

⚠️ **Ne rapprocher les deux séries d'AUCUN chiffre**, ni cadence ni débit — la
sonde partage *une* acquisition entre N recadrages d'aire fixe, celle-ci ouvre
*N* acquisitions sur N surfaces constantes. Un débit étant le produit d'une
cadence par une aire, deux séries incommensurables sur les cadences le restent
sur les débits. *(Une première rédaction de cette section affirmait « le débit
passe de 248–258 à 664 MP/s sans que la cadence bouge » : 248–258 vient de la
sonde et non de cette série, la cadence bouge bel et bien d'une série à l'autre
— 107,5 → 90,1 i/s —, et la borne basse de la sonde à N=1, 208 MP/s, était
écartée sans le dire.)*

### Le défaut de libération des encodeurs : corrigé, et deux risques assumés

**Il n'était pas déterministe mais intermittent** (2/6). **`MFShutdown` n'était
pas en cause** : retiré entièrement du chemin, la faute revient (1/5) — sa
présence dans les deux premières piles était fortuite. La faute réelle : **la MFT
NVIDIA a un élément de travail encore en vol** quand on relâche l'encodeur, et il
entre dans un verrou qui n'existe plus (`RtlEnterCriticalSection`, chemin
contendu, `DebugInfo` nul, sur un fil de pool `CSerialWorkQueue`).

**Correctif** (`agent/src/encode/arret.rs`) : une file de travail Media Foundation
**sérialisée par encodeur** imposée à la MFT (`IMFRealTimeClientEx::SetWorkQueueEx`),
puis dépôt d'une **sentinelle** attendue avant tout relâchement — une attente
**bornée sur une condition observable**, pas un délai. Que le travail de la MFT
transite bien par cette file est éprouvé : boucher la file 3 000 ms arrête
l'encodage net pendant exactement cette durée, la capture continuant.
**0 récidive sur 20 exécutions contre 2 sur 6** — *ce n'est pas une preuve
d'absence, et l'énoncé porte toujours son nombre d'exécutions.*

**Deux risques ouverts et assumés** :

- **`IMFShutdown::Shutdown` est non borné dans un `Drop`, et un gel y a été
  OBSERVÉ** (1 fois sur 6 à N=4, processus vivant treize minutes plus tard,
  `2ter-gel-n4-shutdown.log`). **Cause non attribuée** — l'exécution portait
  aussi une file au convertisseur, retirée depuis, et le départage n'a pas été
  fait. **Le retirer n'est pas une option** : sans lui la faute revient 2 fois
  sur 5, barrière pourtant franchie. Arrêt et barrière ne sont pas redondants.

  ⚠️ **Ce risque est PRÉSENT, pas réservé au chantier D.**
  `Drop for H264Encoder` court **déjà en production mono-fenêtre** : à chaque
  changement de barreau de l'adaptation réseau (`set_encode_size`) et à chaque
  redimensionnement (`resize`), sur le fil unique de `Session::run`
  (`spawn_blocking`) — un gel y figerait la session entière. **Borne du pire
  cas par destruction d'encodeur** : `2 × DELAI_BARRIERE + 2 × DELAI_ARRET_MFT`
  = **8 s** de partie bornée (6 s sur les machines éprouvées, le convertisseur
  n'exposant pas `IMFShutdown`), **et rien ne borne le total** — ni les quatre
  `ProcessMessage`, ni les deux `Shutdown()`. Nominal relevé : 0,5 ms par
  encodeur, 4,0 ms pour huit. Les deux traces qui encadrent l'appel sont en
  **`info!`** et non `debug!` — l'exploitation tourne en `RUST_LOG=info`, et une
  mitigation muette n'en est pas une. **Ne pas les redescendre.**
- **Le convertisseur de couleur n'est couvert par rien.** Sans effet tant qu'il
  retombe sur `CLSID_VideoProcessorMFT` (synchrone), mais **sur un hôte doté d'un
  Video Processor matériel ce serait une MFT matérielle sans barrière** —
  configuration qu'aucune machine éprouvée n'expose, donc **non mesurée**.

### Ce que cette mesure NE dit pas

- **Une exécution par rang, donc AUCUN taux** — ni fréquence d'échec, ni
  variabilité des cadences. Le gel de `Shutdown` vu 1/6 à N=4 n'est ni observé ni
  exclu par une exécution unique à N=4.
- **Rien au-delà de 8 sorties, rien entre 4 et 8** : **8 est ce qui a été demandé
  et obtenu, PAS une limite trouvée.** Le plafond de duplications simultanées
  n'est pas mesuré (le vivier de sorties est de 10 : un rang 9 ou 10 serait
  mesurable, il ne l'a pas été).
- **Rien de la latence**, rien d'autres résolutions ou débits (1280×720@60,
  8 Mb/s, passes de 10 s), rien sur une durée longue.
- **La justesse est ÉCHANTILLONNÉE** : contrôle en rotation, une voie par tour,
  soit **~113 lectures par voie à N=8**, pas 901. « Zéro verdict faux » vaut sur
  les 901 lectures effectuées, pas sur les 7 208 images capturées.
- **Les débits de pixels sont CALCULÉS**, pas relevés (le banc journalise des
  cadences et des images, jamais des pixels).
- **Les mires ne sont pas des applications** : D3D11 plein cadre, sans occlusion,
  sans interaction, sans redimensionnement.
- **Aucune unité H.264 n'a été décodée ni regardée.**
- **Ce qui borne la cadence à ~90 i/s n'est pas mesuré** — un plafond juste
  au-dessus et un plafond très au-dessus se liraient pareil ici.
- **Rien du comportement quand Apollo consomme le même vivier de 10.**
- **Le cas d'exploitation réel n'est pas couvert** : le banc crée ses N encodeurs
  d'un coup et les détruit d'affilée à la fin, **jamais un seul pendant que les
  autres encodent** — ce que fera pourtant la fermeture d'une fenêtre.
- **La mise en sommeil des fenêtres masquées reste une conjecture** : « créer 8 →
  en détruire 1 → tenter un 9ᵉ » n'a pas été jouée.

### Pièges neufs — à connaître avant de toucher à ce terrain

- **Ne jamais se fier à la pile du fil principal pour désigner une cause.** Elle
  montrait `MFShutdown` dans les deux vidages ; coïncidence de minutage. Ce qui
  tranche est de **retirer la variable suspecte et de voir si le symptôme
  survit** — cela a coûté une campagne entière.
- **`MFSHUTDOWN_COMPLETED` ne veut pas dire « plus rien en vol ».** Une MFT rend
  cet état en `attente_ms=0` et fait planter le processus quelques instants plus
  tard. Fait acquis, réutilisable.
- **Un défaut intermittent qu'on croit déterministe se déclare corrigé à la
  première exécution qui passe.** Mesurer le taux **avant** de corriger, et lui
  opposer une campagne d'un ordre de grandeur au-dessus.
- **Ne pas totaliser des exécutions qui n'exercent pas la même chose** : seule la
  ligne comparable s'oppose à la référence ; un agrégat est un nombre sans
  référent.
- **Ne pas utiliser `git add -A` dans un arbre partagé** — un `git add -A
  agent/src` a emporté dans un commit le travail concurrent d'une autre tâche,
  sans sa déclaration de module : le commit ne compilait pas. **Nommer les
  fichiers.**
- **Un banc à aire fixe et un banc à aire croissante ne se comparent pas.** Les
  deux séries existent désormais dans ce dépôt.
- **Corriger une affirmation réfutée exige de la CHERCHER, pas de la corriger là
  où on nous l'a montrée.** Les documents longs ont un sommaire, et c'est lui
  qu'on lit : traiter le chapitre de détail en laissant le sommaire intact laisse
  le lecteur repartir avec une tâche déjà faite. Balayer sur les formules
  (« encore due », « non diagnostiqué », « jamais expliqué »…).
  **Corollaire, payé une ronde plus tard : chercher par le SENS, pas par la
  formule.** Ce balayage cherchait « **non** diagnostiqué » ; la phrase qui a
  survécu disait « **pas** diagnostiqué », et c'était la conclusion d'une section
  entière, contredisant l'encadré posé soixante lignes plus haut. Une négation se
  dit de plusieurs façons, et **c'est celle qu'on n'a pas listée qui survit** :
  balayer sur la *chose niée* (un diagnostic, une mesure, une explication) en
  énumérant les tournures — « pas / non / jamais / seulement localisé / sans
  explication / reste ouvert / n'est établi par rien ». Et **annoter
  l'affirmation elle-même, pas sa voisine**.
- **Une clé de lecture posée dans le code doit être vérifiée contre le journal
  avant d'être recopiée.** Le commentaire de `passes.rs` expliquait le rapport
  `unites`/`images` par un ratio 90/60 qui prédisait 600 unités là où le journal
  en montrait **450** — la fermeture arithmétique était juste
  (`images = unites + nv12_ecartees + au plus 1 en vol`, vérifiée aux quatre
  rangs), **l'attribution causale ne l'était pas**. Reformulée en constat ; la
  cause du rapport d'un demi **n'est pas établie**.
- **Corollaire à retenir pour le chantier D** : les 90,1 i/s sont une cadence de
  **capture**, pas d'unités H.264 délivrées — ce montage rend **45 unités par
  seconde et par fenêtre**.

---

## 🪟🌐 Sous-bloc D1 — tranche verticale multi-fenêtres (1ᵉʳ août 2026)

> ✅ **À LIRE AVANT CETTE SECTION — le sous-bloc D2, le même jour, a réparé le
> défaut bloquant de D1, et les SEPT points de suite de son §9 sont clos**
> (cinq par D2, deux par le correctif final de branche `e9691eb`). Les
> affirmations ci-dessous restent le relevé **de D1**, mais celles qui portent
> sur ce qui est possible aujourd'hui sont annotées une à une. Verdict à jour :
> section « Sous-bloc D2 » plus bas.

Résultats complets :
`docs/superpowers/plans/2026-08-01-multifenetres-tranche-verticale-resultats.md`.
Conception : `docs/superpowers/specs/2026-08-01-multifenetres-tranche-verticale-design.md`.
Journaux : `docs/superpowers/plans/journaux-multifenetres-d1/` — **UTF-8 sans
BOM**, avec les séquences ANSI de `tracing` comme les journaux des chantiers
précédents (`sed 's/\x1b\[[0-9;]*m//g'` pour les lire à plat). Le répertoire porte
aussi les pièces qui ne sont pas des journaux d'agent : les captures d'écran des
fenêtres navigateur, la relecture `WM_GETTEXT` des Bloc-notes, l'environnement
du signaling, le relevé des veilles prolongées, et **l'instrument lui-même**
(`pilote-recette.mjs`, versé dans son état final, celui de la dernière
exécution).

**Premier chantier de PRODUIT du modèle multi-fenêtres**, et première exécution
réelle : jusqu'ici la seule preuve était le compilateur et les tests des parties
pures. Le superviseur (`agent/src/superviseur/`) détecte les fenêtres, leur donne
une sortie virtuelle, y pose la fenêtre, lance un enfant par fenêtre, et parle à
une page-shell (`client/src/shell.ts`) qui ouvre une fenêtre navigateur par
fenêtre Windows.

### Le verdict : ça marche, et ça ne tient pas

**Acquis, vérifié en session réelle** : une fenêtre navigateur par fenêtre
Windows, chacune sur sa sortie virtuelle, chacune capturée et encodée par **son
propre processus**, chacune montrant **son** application et elle seule, plein
cadre à 1280×720, RTT 1–3 ms — **jusqu'à quatre simultanées**, sur de vraies
applications (Bloc-notes, Explorateur, Firefox) et non des mires.
⚠️ **Ces quatre fenêtres PRÉEXISTAIENT au démarrage du superviseur** : ce sont
celles que l'énumération initiale trouve. **Le cas produit — un utilisateur
ouvre une application — a été tenté deux fois et a échoué deux fois.** D1 sait
éclater un bureau tel qu'il est ; il ne sait pas en accueillir une de plus.

> ✅ **D2 sait en accueillir une de plus** : montées 1→2, 2→3 et 3→4 propres, en
> conditions de produit, sur de vraies applications. La restriction « fenêtres
> préexistantes » est **levée**. Ce qui la remplace est un plafond de **quatre**
> fenêtres simultanées, d'une autre nature (§ « Sous-bloc D2 »).

Le son est porté par **une seule** fenêtre (+59 710 octets RTP audio en 9,4 s sur elle
seule, les trois autres sessions n'ayant aucune piste audio). Aucune sortie n'a
fuité : ensemble des **noms** de sorties identique au départ aux trois contrôles
depuis un processus neuf, superviseur pourtant tué net à chaque fois.

**Bloquant** : **créer une sortie virtuelle fait abandonner le mutex des
duplications DXGI déjà ouvertes** (`0x887A0026`, « Le mutex indexé a été
abandonné »). Toute nouvelle fenêtre tue donc **toutes** les sessions en cours,
et l'emballement qui suit vide la page-shell alors que les applications Windows
sont toujours là. **Reproduit sur trois exécutions versées sur trois, plus une
quatrième dont les journaux ne sont pas joints.** La correspondance est exacte : sur les **17**
créations de sortie des trois journaux, **13 n'ont aucune duplication ouverte
→ 0 erreur** (9 parce qu'aucun enfant n'a encore été lancé, 4 entre deux
vagues), et **4 en ont → 9 erreurs**, soit `1, 1, 3, 4`. Ce `1, 1, 3, 4` est le
nombre d'**enfants qui capturent**, PAS le nombre de lignes `duplication de
sortie établie` — dans F il y en a douze pour quatre enfants et quatre erreurs. ⚠️ **La DESTRUCTION d'une
sortie n'est pas mise en cause : le cas n'a jamais été exercé** — les
dix-sept destructions des trois journaux tombent toutes hors de toute
duplication ouverte. D1 **n'est pas reçu**.

> ✅ **Corrigé par D2, et la destruction a depuis été exercée.** L'abandon du
> mutex se produit toujours — il n'est ni évité ni expliqué — mais il est
> **encaissé** : la duplication est relâchée puis rouverte dans une fenêtre de
> reprise, et 44 pertes d'accès n'ont tué aucune session. **La destruction d'une
> sortie abandonne le mutex elle aussi** (relevé sous duplication ouverte,
> aucune création intercalée), et la reprise l'encaisse également. Ce paragraphe
> reste le relevé exact de D1 ; il ne décrit plus le comportement du dépôt.

### Quatre défauts à connaître avant de toucher à ce terrain

> ✅ **Les quatre sont corrigés par D2** (nom DXGI partout ; garde `sur_sortie`
> en tête de `resize` ; viewport arrondi en pair côté client ; appariement
> tolérant à 4 px et attente **sur condition observable** au lieu d'un délai
> plat). Le diagnostic ci-dessous garde sa valeur — c'est pourquoi il reste —
> mais **ne pas repartir de ces quatre points comme s'ils étaient ouverts**.
> ⚠️ Un piège s'y est ajouté depuis : **le pilote QUANTIFIE la résolution
> demandée** (1280×632 demandé → sortie 1280×720), ce que 4 px de tolérance ne
> rattrapent pas.

- **`(index_adaptateur, index_sortie)` n'est PAS un identifiant de sortie.** Il
  est positionnel et change dès qu'une sortie apparaît ou disparaît. Le
  superviseur le passe pourtant à l'enfant, qui le résout plus tard : d'où des
  `Error: aucune sortie DXGI à l'index adaptateur 0, sortie 5`. Le `nom_sortie`
  (`\\.\DISPLAYn`) est le seul identifiant stable, et le superviseur l'a déjà.
- **`WindowsSource::resize` ignore le mode `sur_sortie`.** Il redimensionne la
  fenêtre Windows et reconstruit une duplication du **bureau** : hors écran,
  donc `la fenêtre est hors de l'écran`, capture de secours sur le bureau
  physique, et `soumission à l'encodeur échouée … NV12` **une fois par image**.
  En mode « une sortie par fenêtre », il n'y a rien à redimensionner.
- **Une hauteur de viewport impaire rend toute fenêtre impossible.** Le pop-up
  d'un navigateur annonce couramment une hauteur impaire (1280×**713** mesuré).
  `resize` force les dimensions paires (`& !1`) : la taille demandée ne peut
  alors jamais égaler celle de la source. **Arrondir le viewport avant de créer
  la sortie.**
- **`DELAI_RATTACHEMENT = 1500 ms` n'est pas toujours suffisant**, et
  l'appariement par égalité stricte de dimensions échoue alors : sortie créée à
  1280×713, rendue par DXGI à 1280×720 une fois, à 1280×713 l'essai suivant.
  **Ce n'est pas le facteur DPI de 1,5** que les documents redoutaient, c'est une
  course.

### Ce que D1 devait relever et n'a PAS relevé

- **Le plafond d'encodeurs en multi-processus.** Pas approché : **4 encodeurs
  NVENC construits de front dans 4 processus, aucun refus** ; 6 sorties
  virtuelles attachées simultanément, aucun refus du pilote non plus. Le 8
  connu reste un chiffre de **processus unique**.
- **L'injection clavier**, ni démontrée ni réfutée. Deux causes possibles,
  non départagées : côté navigateur le premier clic est consommé par
  `requestPointerLock` (activation utilisateur, qu'un clic CDP ne fournit pas
  sans interface) ; côté agent **`SendInput` est global à la session Windows** —
  le clavier va à la fenêtre au premier plan, et aucun `SetForegroundWindow`
  n'est fait. Le second point est **structurel** et vaudra quel que soit le
  premier.
  ✅ **D2 l'a démontrée** : `SetForegroundWindow` est désormais posé avant
  injection (retour vérifié : 4 succès, 0 refus), et les quatre Bloc-notes qui
  ont une session ont reçu **chacun sa propre frappe**, pas le cumul. **Portée
  exacte** : une frappe par fenêtre, sonde **séquentielle**, aucune frappe
  concurrente — `SendInput` **reste global à la session Windows**, et ce relevé
  ne dit rien de deux utilisateurs frappant en même temps.
- Rien de la latence ni de la cadence, 3 applications seulement, session vivante
  la plus longue ≈ **50 s**, une exécution exploitée par configuration.
  ⚠️ **Toujours vrai après D2** — latence et cadence n'ont pas davantage été
  mesurées, et le **plafond d'encodeurs en multi-processus** n'a pas été
  approché non plus (4 encodeurs de front dans 4 processus, la mort survenant
  avant tout encodeur au 5ᵉ).

### ⚠️ La VM se met en veille prolongée toute seule — deux mesures perdues

**Deux horodatages, et l'écart entre eux est réel** : l'invité amorce la
transition à **09:40:04 et 10:40:04 UTC** (Kernel-Power 187/42), QEMU n'est
terminé qu'à **09:40:09 et 10:40:10** — les ~5 s d'écriture de l'image
d'hibernation. Ce n'est pas une minuterie d'inactivité (`STANDBYIDLE` et
`HIBERNATEIDLE` sont à 0) : le journal Windows nomme l'initiateur,
`\Windows\System32\shutdown.exe` (Kernel-Power **187**), pour une transition de
type hibernation (Kernel-Power **42**). Relevé versé :
`journaux-multifenetres-d1/veille-prolongee-vm.txt`.
⚠️ **Le motif horaire n'est PAS établi** : le journal libvirt porte **quatre**
extinctions sur la journée — 08:29:55, 09:40:09, 10:40:10 et 11:30:27 UTC —
soit des intervalles de **70, 60 puis 50 minutes**, dont deux seulement sur la
minute :40. **Le déclencheur exact n'est pas identifié** — la
seule tâche planifiée appelant `shutdown` est désactivée depuis avril 2025 ;
`sunshine`/`sunshinesvc` tournent et sont des suspects **non éprouvés**. La
seule règle prudente : **vérifier que la VM a survécu après toute séquence
longue**, plutôt que de se fier à une fenêtre horaire.

```bash
# Symptômes : /media/vm répond « L'hôte cible est arrêté ou en panne »,
# virsh list --all dit « fermé », run-agent.sh échoue en écrivant son .ps1.
grep -E "terminating on signal|shutting down" /var/log/libvirt/qemu/Windows.log | tail -4
```

### Pièges d'outillage rencontrés

- **`scripts/run-agent.sh` ne transmettait pas `SUPERVISEUR`** — corrigé. Sans
  cette ligne, l'agent démarre en mode mono-fenêtre sans rien signaler.
- **Un agent SURVIT à l'hibernation de la VM**, et `run-agent.sh` ne tue pas
  l'agent existant : il recrée la tâche et la lance. **Vérifier
  `Get-Process agent` avant chaque exécution**, sinon on mesure le processus
  précédent.
- **Sans `--disable-popup-blocking`, la démonstration est vide et muette** : la
  shell ouvre ses fenêtres hors geste utilisateur, Chrome les refuse toutes, et
  la seule trace est un message dans la page-shell.
- **Chrome sans interface survit à la mort de son pilote.** Une exécution
  entière a été perdue parce que le pilote s'est attaché à une instance
  résiduelle, sur un port réutilisé, dont les pages périmées ne recevaient plus
  rien. **Un port de débogage qui répond ne prouve pas que c'est le bon
  navigateur.**
- **Une capture d'écran CDP suffit à déclencher l'effondrement** : elle provoque
  un `Resize`, qui rétrécit la fenêtre Windows, qui engendre un `SHOW`, qui crée
  une session, qui crée une sortie, qui tue toutes les captures.
  **L'instrument détruisait ce qu'il mesurait** — même leçon que la trace par
  paquet du chantier TURN, sous une autre forme.
  ⚠️ **Depuis D2, le dernier maillon ne tue plus rien** (la reprise l'encaisse),
  mais **la chaîne demeure entière jusque-là** : une capture CDP provoque
  toujours un `Resize`, donc un `SHOW`, donc une session et une sortie de plus.
  **La contrainte de protocole tient** : pas de capture d'écran pendant une
  mesure. Et une autre raison s'y ajoute — toute évaluation CDP sur une page
  portant un flux WebRTC actif peut **ne jamais rendre** (voir la section D2).
- **Le signaling ne mémorise que les offres SDP** : les annonces
  `fenetre-ouverte` émises avant que la page-shell ne soit connectée sont perdues
  sans trace. **Lancer le navigateur AVANT le superviseur.**
- **`agent.log` mêle le superviseur et tous ses enfants** (stdout hérité), sans
  rien qui distingue l'émetteur hors le champ `session=` de certaines lignes.

---

## 🪟🔁 Sous-bloc D2 — arrangement multi-fenêtres dynamique (1ᵉʳ août 2026)

Résultats complets :
`docs/superpowers/plans/2026-08-01-multifenetres-arrangement-dynamique-resultats.md`.
Conception : `docs/superpowers/specs/2026-08-01-multifenetres-arrangement-dynamique-design.md`.
Journaux : `docs/superpowers/plans/journaux-multifenetres-d2/` — **UTF-8**, avec
les séquences ANSI de `tracing` (`sed 's/\x1b\[[0-9;]*m//g'` pour lire à plat).
**Une exception d'encodage** : `build-agent-11bis.log`, dont les lignes revenant
du PowerShell distant sont mutilées (voir les pièges plus bas).

D2 ne fait qu'une chose : lever ce qui empêchait D1 d'être reçu.

### Le verdict est DOUBLE — ne le simplifier dans aucun sens

**① Le défaut central de D1 est réparé, et démontré réparé en conditions de
produit.** Créer une sortie virtuelle ne tue plus les captures en cours : **44**
pertes d'accès `0x887A0026` encaissées sur le passage décisif, **aucune session
perdue**, montées 1→2, 2→3 et 3→4 propres, **une seule** ligne `clôture de
session amorcée` sur tout le passage — et elle est **sollicitée** (0,7 s après un
`WM_CLOSE` réel). Sur de vraies applications, pas des mires.

**② Le critère de réception exigeait CINQ fenêtres simultanées ; on en atteint
QUATRE.** La 5ᵉ duplication DXGI, **dans un 5ᵉ processus**, est refusée en
`0x887A0022` (`DXGI_ERROR_NOT_CURRENTLY_AVAILABLE`) par une limite de
**concurrence** qui **résiste à trois secondes de patience explicite** — ce
qu'aucune mesure antérieure n'avait éprouvé. **La couche qui l'impose n'est pas
identifiée**, et **rien n'établit que 4 soit une borne du système**.

**Donc D2 n'est pas reçu au sens de son critère, et il répare pourtant ce pour
quoi il existait.**

### La cause, trouvée au bout de TROIS mesures dont deux réfutations

`rouvrir()` demandait une seconde duplication de la même sortie **sans avoir
relâché la première**. DXGI n'autorise qu'une duplication ouverte par sortie
(doctrine déjà portée par ce fichier) : l'appel **réussissait** et rendait un
objet **mort-né**, qui reperdait son accès aussitôt.

| Tirage | Ce qui changeait | Tentatives/voie | `images_apres` | Verdict |
| --- | --- | --- | --- | --- |
| 1 | 3 tentatives sans délai | 3 (budget entier, brûlé en 14–21 ms) | 0 | RÉFUTÉ |
| 2 | fenêtre 8 s, pas 150 ms | **54** (le maximum théorique) | 0 | RÉFUTÉ |
| 3 | **relâche puis acquiert** | **1** | `[892]`/`[889,888]`/`[884,883,883,882]` | **REÇU** |

**Une exécution par rang à chaque tirage : aucun taux.** Entre le 2ᵉ et le 3ᵉ,
une seule variable de comportement a changé — l'attribution causale est propre
sous cette réserve.

Délai perturbation → réouverture, **relevé** : **+48 ms** (k=1), **+70/+81 ms**
(k=2), **+105 à +144 ms** (k=4). La fenêtre de 8 s consomme donc **1 tentative
sur 54** : elle est **très surdimensionnée pour le cas mesuré**, mais **aucun cas
lent n'a été observé** — **ne pas la réduire sur la foi de ce seul relevé.**

> **Leçon de méthode, chère :** deux réfutations coûteuses ont été closes non par
> un tirage de plus mais par la **relecture d'une ligne**. Quand deux mesures
> successives réfutent une hypothèse **sans que le symptôme change de forme**,
> relire le chemin avant de recalibrer.

### Trois acquis que D1 déclarait ouverts

- **Le clavier atteint chaque fenêtre séparément.** `SetForegroundWindow` avant
  injection, retour vérifié (4 succès, 0 refus) ; les quatre Bloc-notes qui ont
  une session ont reçu **chacun sa frappe**, la cinquième — sans session — rien.
  ⚠️ **Portée exacte** : une frappe par fenêtre, sonde séquentielle, aucune
  frappe concurrente. **`SendInput` reste global à la session Windows.**
- **La DESTRUCTION d'une sortie abandonne le mutex elle aussi**, et la reprise
  l'encaisse. D1 déclarait le cas « jamais exercé » ; il l'est (destruction,
  puis deux pertes d'accès à +22 et +25 ms, **aucune création intercalée**).
- **L'ordre `relâcher/acquérir` était la cause.** La piste du **périphérique
  D3D11 conservé**, formulée après la 2ᵉ réfutation, devient **SANS OBJET — et
  non pas « réfutée »** : elle n'a **jamais** été mise à l'épreuve, elle n'a
  simplement plus rien à expliquer. Elle reste disponible si un symptôme voisin
  réapparaissait.

### La suite à donner, nommée précisément

1. **Ne plus détruire puis recréer la sortie virtuelle à chaque relance
   d'enfant** — c'est **la vraie cause des 32 réouvertures parasites**. Une
   fenêtre condamnée fait passer le compteur de 6 à 38 à elle seule. Le réessai
   à l'ouverture ajouté pour cela n'en a supprimé **aucune** : **aucun de ces
   échecs n'était un transitoire** (quatre séquences, fenêtre entière courue,
   zéro reprise réussie). Aucun réessai ne peut rien contre cette cause-là.
   **C'est la première chose à corriger.**
   ⚠️ **Les « 44 avant / 44 après » sont des comptes ARRÊTÉS à la fin de la
   séquence de critère, pas des totaux de fichier.** Le journal versé de la
   seconde exécution va plus loin (phase clavier, captures, arrêt) et porte
   **50** réouvertures en fin de fichier ; le 44 comparable s'y relit par une
   borne temporelle explicite. Ne jamais opposer un total de fichier à un compte
   fenêtré.
2. **Identifier la couche du plafond de quatre.** Ce n'est ni le plafond de
   sorties virtuelles (10 : `création de sortie refusée` = **0** sur tout le
   passage — les neuf créations y sont **successives**, pas simultanées, et ne
   borneraient rien par elles-mêmes), ni celui des encodeurs NVENC (8 : la mort
   survient **avant** tout encodeur, et **aucune pièce de D2 ne compte
   d'encodeurs**). Le
   rapprochement avec les **8 duplications d'un seul processus** du 31 juillet
   est une **INFÉRENCE** — rien ici ne l'établit. Fermer une fenêtre puis en
   rouvrir une réussit : la place libérée suffit.
3. **Une fuite de capacité reste ouverte** pour une fenêtre **neuve** dont la
   page-shell ne répond **jamais** : ni relancée ni abandonnée, elle consomme sa
   place indéfiniment. **Défaut préexistant, pas introduit par D2** ; le
   garde-fou (`DELAI_ATTENTE_VIEWPORT_MAX = 30 s`, majorant non calibré) a été
   volontairement borné aux entrées **relancées**. Remède proposé par la revue :
   tamponner `attente_depuis` **paresseusement** au premier passage de
   `relancer_les_orphelines`, ce qui garde `Table` pure et ne bouge aucun
   appelant.

### Ce que D2 n'établit PAS

Une exécution par rang au banc, **une exécution exploitée par configuration** à
la recette : **aucun taux, nulle part**. Rien au-delà de 4 fenêtres. Le
**mécanisme de l'abandon du mutex reste inconnu** — on sait le traiter, pas
l'expliquer. Rien de la latence, de la cadence, de la durée (session la plus
longue ≈ 4 min 30 s). **Plafond d'encodeurs en multi-processus toujours pas
approché.** Aucun redimensionnement, aucun recouvrement, aucun déplacement de
fenêtre. Le chemin `resize` n'est **pas exercé** après le correctif
`new_sans_attente`. La branche `est_ouverture_retentable(ACCES_PERDU)` n'a
**jamais** été exercée à l'ouverture. **Le chemin d'extinction propre du
superviseur n'a jamais été exercé** (arrêt net par `schtasks /end`).

### Pièges neufs — à connaître avant de toucher à ce terrain

- **Le pilote de sortie virtuelle QUANTIFIE la résolution demandée** : 1280×632
  demandé rend une sortie **1280×720**, soit 88 px d'écart, très au-delà de la
  tolérance d'appariement de 4 px. **Aucune fenêtre ne s'ouvre alors, et rien ne
  le dit hors du journal d'agent.** Imposer au navigateur une taille que le
  pilote rend à l'identique.
- **Toute évaluation CDP sur une page portant un flux WebRTC actif doit être
  BORNÉE.** `Page.captureScreenshot` peut ne **jamais** rendre ; une relecture de
  `window.__console` s'y est figée de la même façon.
- **`scripts/run-agent.sh` ne transmettait pas `MULTIFENETRE_REPRISE`** — même
  piège que `SUPERVISEUR` en D1. **Toute variable neuve du banc doit y être
  ajoutée explicitement**, sinon l'agent démarre sans elle et sans rien signaler.
- **`build-agent.sh` ne pose pas `[Console]::OutputEncoding`** : les lignes
  revenant du PowerShell distant en reviennent mutilées (« Au caract⏎re
  Ligne:1 »). C'est le **défaut à deux réglages** déjà documenté plus haut. Le
  script est partagé : signalé, **non corrigé**.
- **Un journal d'agent s'écrase facilement**, et deux pièces ont été perdues
  ainsi dans ce sous-bloc — dont celle qui aurait étayé une affirmation qu'il a
  fallu retirer. **Copier le journal avant tout relevé qui écrit au même
  endroit.**
- **`pkill -f <motif>` depuis un shell dont la ligne de commande contient le
  motif tue le shell lui-même** (exit 144, la suite de la chaîne ne s'exécute
  pas). Tuer par PID relevé.
- **Paint ouvre DEUX fenêtres éligibles** (`Paint` + `UIRibbonWorkPane`) : sur
  une recette qui compte des fenêtres, n'employer que des applications à fenêtre
  unique — ou compter les fenêtres, jamais les lancements.
- **Un défaut du CODE fourni par un plan doit être signalé, pas recopié.** Une
  tâche a implémenté verbatim un code de brief qui rendait **muet le tout premier
  refus** — exactement le cas que la trace existait pour révéler. La clause
  « signaler un défaut du plan » vaut aussi pour un **bug** dans le code fourni,
  pas seulement pour une divergence de spécification.

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
