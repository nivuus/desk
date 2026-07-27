# Jalon 1 — Tranche verticale : plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Streamer Firefox tournant sur une VM Windows dans un navigateur, à 60 fps et moins de 50 ms de latence, avec souris, clavier et suivi du redimensionnement.

**Architecture:** Un agent Rust sur la VM Windows capture la fenêtre (Windows.Graphics.Capture), l'encode en H.264 matériel (Media Foundation), et l'envoie en WebRTC via str0m. Un serveur de signaling Node minimal relaie l'offre/réponse SDP. Le client web affiche la piste dans un `<video>` (décodage matériel du navigateur) et renvoie les entrées sur un data channel binaire.

**Tech Stack:** Rust 1.8x + `windows` 0.62 + `str0m` 0.21 + `tokio` · Node 24 + TypeScript + `ws` · Vite + TypeScript

## Contraintes globales

Ces contraintes s'appliquent à **toutes** les tâches.

- **Spec de référence** : `docs/superpowers/specs/2026-07-27-jalon1-tranche-verticale-design.md`. Les critères d'acceptation du §1 de la spec sont le juge final.
- **Versions figées** : `str0m = "0.21"`, `windows = "0.62"`, `windows-future = "0.3"`, `tokio = "1"`, Node ≥ 24, TypeScript ≥ 5.5, Vite ≥ 6.
- **Machine de développement** : Linux (`/home/mallanic/Projects/Guacamole`). Rust n'y est pas installé — la tâche 1 l'installe.
- **Machine cible** : VM Windows `192.168.3.2`, GPU avec encodeur matériel. WinRM ouvert sur 5985 (`Administrator`). Pas de SSH. Son `C:` est monté en lecture-écriture sur `/media/vm`.
- **Le code source vit dans le dépôt Linux.** Le répertoire `agent/` est synchronisé vers `C:\dev\agent` (via `/media/vm/dev/agent`) et compilé sur Windows par WinRM. Ne jamais éditer directement sous `/media/vm`.
- **Nommage** : le nouveau client web s'appelle `client/` (le répertoire `web/` est occupé par l'ancien client Guacamole, qui reste intact).
- **Langue** : commentaires et messages de commit en français ; identifiants de code en anglais.
- **Commits** : un commit par tâche minimum, message conventionnel (`feat:`, `test:`, `chore:`).
- **Pièges d'API vérifiés** (issus de la recherche, ne pas les contredire) :
  1. `str0m::change::DirectApi::declare_media()` ne remplit pas `remote_pts` → `Writer::write()` ne trouverait aucun payload type. **On utilise `sdp_api()` + Frame API**, jamais `direct_api()` pour la vidéo.
  2. str0m 0.21 exige l'installation d'un crypto provider au démarrage du processus.
  3. Les MFT `MFT_ENUM_FLAG_HARDWARE` sont **asynchrones** : pilotage par `IMFMediaEventGenerator` (`METransformNeedInput`/`METransformHaveOutput`), pas par boucle `ProcessInput`/`ProcessOutput` synchrone.
  4. `MFTEnumEx` a une signature typée en 0.62 : `flags: MFT_ENUM_FLAG`, types `Option<*const MFT_REGISTER_TYPE_INFO>`.
  5. Règle str0m : après chaque mutation, drainer `poll_output()` jusqu'à `Output::Timeout` avant la mutation suivante.

## Structure de fichiers

```
proto/                       Protocole partagé (source de vérité unique)
  src/lib.rs                 Réexports
  src/input.rs               Codec binaire des entrées (Rust)
  src/control.rs             Messages de contrôle JSON (Rust, serde)
  vectors.json               Vecteurs de test partagés Rust ⇄ TypeScript
  ts/input.ts                Encodeur binaire des entrées (TypeScript)
  ts/control.ts              Types des messages de contrôle (TypeScript)
  ts/input.test.ts           Tests TS + round-trip sur vectors.json

signaling/                   Serveur de signaling Node/TS
  src/server.ts              Relais SDP par session
  src/server.test.ts

client/                      Client web Vite + TypeScript
  index.html
  src/main.ts                Orchestration de la session
  src/webrtc.ts              RTCPeerConnection, offre, piste vidéo
  src/input.ts               Capture des entrées → protocole binaire
  src/stats.ts               Overlay de mesure (fps, bitrate, RTT, latence)
  src/style.css

agent/                       Agent Rust (compilé sur Windows)
  Cargo.toml
  src/main.rs                Câblage, CLI
  src/signaling.rs           Client WebSocket vers signaling/
  src/transport.rs           Boucle str0m (ICE/DTLS/SRTP/SCTP), envoi des frames
  src/h264.rs                Découpage Annex-B en unités d'accès
  src/source.rs              Trait VideoSource + source fichier (dérisquage)
  src/capture.rs             D3D11 + Windows.Graphics.Capture (Windows)
  src/encode.rs              Encodeur H.264 matériel MFT async (Windows)
  src/input.rs               Injection SendInput + mapping de coordonnées
  src/window.rs              Repérage, resize, surveillance de la fenêtre

scripts/
  sync-agent.sh              Copie agent/ + proto/ vers /media/vm/dev/
  build-agent.sh             Compile sur Windows via WinRM
  winrm.js                   Exécution d'une commande PowerShell via WinRM
```

**Progression du plan.** Les tâches 1 à 7 se développent et se testent **entièrement sur Linux** — la tâche 7 affiche déjà une vidéo dans le navigateur via WebRTC, sans aucun code Windows. C'est le jalon de dérisquage : si str0m, le signaling et le décodage navigateur fonctionnent avec une source de test, il ne reste plus qu'à remplacer la source. Les tâches 8 à 13 ajoutent le code Windows.

---

### Task 1: Squelette du monorepo et chaîne de compilation Windows

**Files:**
- Create: `Cargo.toml` (workspace), `rust-toolchain.toml`, `proto/Cargo.toml`, `proto/src/lib.rs`, `agent/Cargo.toml`, `agent/src/main.rs`
- Create: `scripts/winrm.js`, `scripts/sync-agent.sh`, `scripts/build-agent.sh`
- Create: `package.json` (racine, pour les scripts de dev)
- Modify: `.gitignore`

**Interfaces:**
- Consumes: rien.
- Produces: workspace Cargo compilable ; `scripts/build-agent.sh` compile l'agent sur la VM Windows et affiche le résultat.

- [ ] **Step 1: Installer Rust sur la machine Linux**

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
source "$HOME/.cargo/env"
cargo --version   # attendu : cargo 1.8x.x
```

- [ ] **Step 2: Créer le workspace Cargo**

`Cargo.toml` (racine) :

```toml
[workspace]
resolver = "2"
members = ["proto", "agent"]

[workspace.package]
edition = "2021"
version = "0.1.0"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
anyhow = "1"
```

`rust-toolchain.toml` :

```toml
[toolchain]
channel = "stable"
```

- [ ] **Step 3: Créer la crate proto (vide pour l'instant)**

`proto/Cargo.toml` :

```toml
[package]
name = "proto"
edition.workspace = true
version.workspace = true

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
```

`proto/src/lib.rs` :

```rust
//! Protocole partagé entre l'agent, le client web et le signaling.

pub mod control;
pub mod input;
```

Créer `proto/src/input.rs` et `proto/src/control.rs` vides (contenu ajouté aux tâches 2 et 4).

- [ ] **Step 4: Créer la crate agent avec un main minimal**

`agent/Cargo.toml` :

```toml
[package]
name = "agent"
edition.workspace = true
version.workspace = true

[dependencies]
proto = { path = "../proto" }
anyhow = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

`agent/src/main.rs` :

```rust
fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();
    tracing::info!("agent démarré");
    Ok(())
}
```

- [ ] **Step 5: Vérifier que le workspace compile sur Linux**

Run: `cargo build`
Expected: compilation réussie des deux crates (`proto`, `agent`).

- [ ] **Step 6: Écrire le client WinRM**

`scripts/winrm.js` — exécute une commande PowerShell sur la VM et renvoie sa sortie :

```javascript
// Exécute une commande PowerShell sur la VM Windows via WinRM.
// Usage : node scripts/winrm.js "Get-ChildItem C:\\"
const winrm = require('nodejs-winrm');

const HOST = process.env.WINDOWS_HOSTNAME || '192.168.3.2';
const USER = process.env.WINDOWS_ADMIN_USERNAME || 'Administrator';
const PASS = process.env.WINDOWS_ADMIN_PASSWORD;

async function main() {
    const command = process.argv.slice(2).join(' ');
    if (!command) {
        console.error('usage : node scripts/winrm.js <commande powershell>');
        process.exit(2);
    }
    if (!PASS) {
        console.error('WINDOWS_ADMIN_PASSWORD non défini');
        process.exit(2);
    }
    const output = await winrm.runCommand(command, HOST, USER, PASS, 5985, true);
    process.stdout.write(output || '');
}

main().catch((e) => {
    console.error(e.message || e);
    process.exit(1);
});
```

`package.json` (racine) :

```json
{
  "name": "guacamole-monorepo",
  "private": true,
  "scripts": {
    "winrm": "node scripts/winrm.js"
  },
  "dependencies": {
    "nodejs-winrm": "^1.1.0"
  }
}
```

- [ ] **Step 7: Vérifier la connexion WinRM**

```bash
npm install
export WINDOWS_ADMIN_PASSWORD='<mot de passe>'
node scripts/winrm.js "\$PSVersionTable.PSVersion.Major"
```

Expected: un numéro de version (5 ou 7). Si échec, corriger avant d'aller plus loin — toute la suite en dépend.

- [ ] **Step 8: Installer Rust sur la VM Windows**

```bash
node scripts/winrm.js "if (-not (Test-Path 'C:\\dev')) { New-Item -ItemType Directory -Path 'C:\\dev' | Out-Null }; Invoke-WebRequest -Uri 'https://win.rustup.rs/x86_64' -OutFile 'C:\\dev\\rustup-init.exe'"
node scripts/winrm.js "C:\\dev\\rustup-init.exe -y --default-toolchain stable --default-host x86_64-pc-windows-msvc"
node scripts/winrm.js "\$env:Path += ';C:\\Users\\Administrator\\.cargo\\bin'; cargo --version"
```

Expected: `cargo 1.8x.x`. Si `cargo` compile mais que le lien échoue plus tard, installer les Build Tools MSVC :
`node scripts/winrm.js "winget install --id Microsoft.VisualStudio.2022.BuildTools --silent --override '--wait --quiet --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended'"`

- [ ] **Step 9: Écrire les scripts de synchronisation et de compilation**

`scripts/sync-agent.sh` :

```bash
#!/usr/bin/env bash
# Copie les sources Rust vers C:\dev (monté sur /media/vm) pour compilation sur Windows.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="/media/vm/dev"

if ! mountpoint -q /media/vm; then
    echo "erreur : /media/vm n'est pas monté" >&2
    exit 1
fi

mkdir -p "$DEST"
rsync -a --delete \
    --exclude 'target/' \
    "$ROOT/Cargo.toml" "$ROOT/rust-toolchain.toml" \
    "$ROOT/proto" "$ROOT/agent" \
    "$DEST/"

echo "sources synchronisées vers $DEST"
```

`scripts/build-agent.sh` :

```bash
#!/usr/bin/env bash
# Synchronise puis compile l'agent sur la VM Windows.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
"$ROOT/scripts/sync-agent.sh"

PROFILE="${1:-debug}"
FLAG=""
[ "$PROFILE" = "release" ] && FLAG="--release"

node "$ROOT/scripts/winrm.js" \
    "\$env:Path += ';C:\\Users\\Administrator\\.cargo\\bin'; Set-Location C:\\dev; cargo build $FLAG 2>&1 | Out-String"
```

```bash
chmod +x scripts/sync-agent.sh scripts/build-agent.sh
```

- [ ] **Step 10: Vérifier la compilation croisée de bout en bout**

Run: `./scripts/build-agent.sh`
Expected: sortie `cargo` se terminant par `Finished dev profile`. C'est la boucle de développement de tout le reste du plan.

- [ ] **Step 11: Compléter le .gitignore et committer**

Ajouter à `.gitignore` :

```
target/
client/dist/
signaling/dist/
*.264
```

```bash
git add Cargo.toml rust-toolchain.toml proto agent scripts package.json package-lock.json .gitignore
git commit -m "chore: squelette du monorepo et chaîne de compilation Windows par WinRM"
```

---

### Task 2: Codec binaire des entrées (Rust)

Le protocole d'entrée circule sur un data channel non fiable et non ordonné : chaque message doit être **autonome, de taille fixe, sans état**. Les coordonnées sont normalisées sur `0..=65535`, ce qui correspond exactement à l'espace attendu par `SendInput` en mode absolu et reste correct pendant un redimensionnement.

**Files:**
- Modify: `proto/src/input.rs`
- Test: `proto/src/input.rs` (module `#[cfg(test)]`)

**Interfaces:**
- Consumes: rien.
- Produces:
  - `enum InputMessage { MouseMove { x: u16, y: u16 }, MouseButton { button: MouseButton, pressed: bool, x: u16, y: u16 }, Wheel { delta_x: i16, delta_y: i16 }, Key { scancode: u16, pressed: bool, extended: bool } }`
  - `enum MouseButton { Left, Right, Middle }`
  - `InputMessage::encode(&self) -> Vec<u8>`
  - `InputMessage::decode(bytes: &[u8]) -> Result<InputMessage, DecodeError>`
  - `const PROTOCOL_VERSION: u8 = 1;`

- [ ] **Step 1: Écrire les tests d'abord**

`proto/src/input.rs` :

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(msg: InputMessage) {
        let encoded = msg.encode();
        let decoded = InputMessage::decode(&encoded).expect("décodage réussi");
        assert_eq!(msg, decoded);
    }

    #[test]
    fn round_trip_mouse_move() {
        round_trip(InputMessage::MouseMove { x: 0, y: 0 });
        round_trip(InputMessage::MouseMove { x: 65535, y: 32768 });
    }

    #[test]
    fn round_trip_mouse_button() {
        round_trip(InputMessage::MouseButton {
            button: MouseButton::Left,
            pressed: true,
            x: 100,
            y: 200,
        });
        round_trip(InputMessage::MouseButton {
            button: MouseButton::Middle,
            pressed: false,
            x: 65535,
            y: 65535,
        });
    }

    #[test]
    fn round_trip_wheel() {
        round_trip(InputMessage::Wheel { delta_x: 0, delta_y: 120 });
        round_trip(InputMessage::Wheel { delta_x: -240, delta_y: -120 });
    }

    #[test]
    fn round_trip_key() {
        round_trip(InputMessage::Key { scancode: 0x1E, pressed: true, extended: false });
        round_trip(InputMessage::Key { scancode: 0x48, pressed: false, extended: true });
    }

    #[test]
    fn encodage_petit_boutiste() {
        // MouseMove x=0x0201, y=0x0403 : version, type, puis octets faibles en tête.
        let encoded = InputMessage::MouseMove { x: 0x0201, y: 0x0403 }.encode();
        assert_eq!(encoded, vec![PROTOCOL_VERSION, 1, 0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn rejette_version_inconnue() {
        let err = InputMessage::decode(&[99, 1, 0, 0, 0, 0]).unwrap_err();
        assert!(matches!(err, DecodeError::UnsupportedVersion(99)));
    }

    #[test]
    fn rejette_type_inconnu() {
        let err = InputMessage::decode(&[PROTOCOL_VERSION, 42, 0, 0]).unwrap_err();
        assert!(matches!(err, DecodeError::UnknownType(42)));
    }

    #[test]
    fn rejette_message_tronque() {
        let err = InputMessage::decode(&[PROTOCOL_VERSION, 1, 0, 0]).unwrap_err();
        assert!(matches!(err, DecodeError::Truncated { .. }));
    }

    #[test]
    fn rejette_message_vide() {
        assert!(matches!(
            InputMessage::decode(&[]).unwrap_err(),
            DecodeError::Truncated { .. }
        ));
    }

    #[test]
    fn rejette_bouton_inconnu() {
        let err = InputMessage::decode(&[PROTOCOL_VERSION, 2, 9, 1, 0, 0, 0, 0]).unwrap_err();
        assert!(matches!(err, DecodeError::UnknownButton(9)));
    }
}
```

- [ ] **Step 2: Lancer les tests pour vérifier qu'ils échouent**

Run: `cargo test -p proto`
Expected: échec de compilation — `InputMessage` n'existe pas.

- [ ] **Step 3: Écrire l'implémentation**

En tête de `proto/src/input.rs`, avant le module de tests :

```rust
//! Codec binaire des messages d'entrée (souris, clavier).
//!
//! Format : `version: u8 | type: u8 | charge utile`, entiers en petit-boutiste.
//! Chaque message est autonome et de taille fixe : le canal de transport est
//! non fiable et non ordonné, aucun message ne dépend d'un autre.

/// Version du protocole d'entrée. Incrémenter à tout changement de format.
pub const PROTOCOL_VERSION: u8 = 1;

const TYPE_MOUSE_MOVE: u8 = 1;
const TYPE_MOUSE_BUTTON: u8 = 2;
const TYPE_WHEEL: u8 = 3;
const TYPE_KEY: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

impl MouseButton {
    fn to_u8(self) -> u8 {
        match self {
            MouseButton::Left => 0,
            MouseButton::Right => 1,
            MouseButton::Middle => 2,
        }
    }

    fn from_u8(v: u8) -> Result<Self, DecodeError> {
        match v {
            0 => Ok(MouseButton::Left),
            1 => Ok(MouseButton::Right),
            2 => Ok(MouseButton::Middle),
            other => Err(DecodeError::UnknownButton(other)),
        }
    }
}

/// Message d'entrée du client vers l'agent.
///
/// Les coordonnées `x`/`y` sont normalisées sur `0..=65535` par rapport à la
/// zone vidéo : cet espace est indépendant de la résolution courante et
/// correspond directement au mode absolu de `SendInput`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMessage {
    MouseMove { x: u16, y: u16 },
    MouseButton { button: MouseButton, pressed: bool, x: u16, y: u16 },
    Wheel { delta_x: i16, delta_y: i16 },
    Key { scancode: u16, pressed: bool, extended: bool },
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DecodeError {
    #[error("version de protocole non supportée : {0}")]
    UnsupportedVersion(u8),
    #[error("type de message inconnu : {0}")]
    UnknownType(u8),
    #[error("bouton de souris inconnu : {0}")]
    UnknownButton(u8),
    #[error("message tronqué : {actual} octets reçus, {expected} attendus")]
    Truncated { expected: usize, actual: usize },
}

impl InputMessage {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(8);
        out.push(PROTOCOL_VERSION);
        match *self {
            InputMessage::MouseMove { x, y } => {
                out.push(TYPE_MOUSE_MOVE);
                out.extend_from_slice(&x.to_le_bytes());
                out.extend_from_slice(&y.to_le_bytes());
            }
            InputMessage::MouseButton { button, pressed, x, y } => {
                out.push(TYPE_MOUSE_BUTTON);
                out.push(button.to_u8());
                out.push(pressed as u8);
                out.extend_from_slice(&x.to_le_bytes());
                out.extend_from_slice(&y.to_le_bytes());
            }
            InputMessage::Wheel { delta_x, delta_y } => {
                out.push(TYPE_WHEEL);
                out.extend_from_slice(&delta_x.to_le_bytes());
                out.extend_from_slice(&delta_y.to_le_bytes());
            }
            InputMessage::Key { scancode, pressed, extended } => {
                out.push(TYPE_KEY);
                out.extend_from_slice(&scancode.to_le_bytes());
                out.push(pressed as u8);
                out.push(extended as u8);
            }
        }
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let header = take(bytes, 0, 2)?;
        if header[0] != PROTOCOL_VERSION {
            return Err(DecodeError::UnsupportedVersion(header[0]));
        }
        match header[1] {
            TYPE_MOUSE_MOVE => {
                let p = take(bytes, 2, 4)?;
                Ok(InputMessage::MouseMove { x: le_u16(p, 0), y: le_u16(p, 2) })
            }
            TYPE_MOUSE_BUTTON => {
                let p = take(bytes, 2, 6)?;
                Ok(InputMessage::MouseButton {
                    button: MouseButton::from_u8(p[0])?,
                    pressed: p[1] != 0,
                    x: le_u16(p, 2),
                    y: le_u16(p, 4),
                })
            }
            TYPE_WHEEL => {
                let p = take(bytes, 2, 4)?;
                Ok(InputMessage::Wheel {
                    delta_x: le_u16(p, 0) as i16,
                    delta_y: le_u16(p, 2) as i16,
                })
            }
            TYPE_KEY => {
                let p = take(bytes, 2, 4)?;
                Ok(InputMessage::Key {
                    scancode: le_u16(p, 0),
                    pressed: p[2] != 0,
                    extended: p[3] != 0,
                })
            }
            other => Err(DecodeError::UnknownType(other)),
        }
    }
}

fn take(bytes: &[u8], offset: usize, len: usize) -> Result<&[u8], DecodeError> {
    bytes
        .get(offset..offset + len)
        .ok_or(DecodeError::Truncated { expected: offset + len, actual: bytes.len() })
}

fn le_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}
```

- [ ] **Step 4: Lancer les tests pour vérifier qu'ils passent**

Run: `cargo test -p proto`
Expected: 9 tests réussis.

- [ ] **Step 5: Committer**

```bash
git add proto/src/input.rs
git commit -m "feat: codec binaire des messages d'entrée"
```

---

### Task 3: Vecteurs de test partagés et encodeur TypeScript

Le client web doit produire exactement les octets que l'agent Rust décode. On fige donc des vecteurs de test dans un fichier lu par les deux langages : toute divergence de format casse un test des deux côtés.

**Files:**
- Create: `proto/vectors.json`, `proto/ts/input.ts`, `proto/ts/input.test.ts`, `proto/package.json`, `proto/tsconfig.json`
- Modify: `proto/src/input.rs` (test de conformité aux vecteurs)

**Interfaces:**
- Consumes: `InputMessage` (tâche 2).
- Produces (TypeScript) :
  - `encodeMouseMove(x: number, y: number): Uint8Array`
  - `encodeMouseButton(button: 0|1|2, pressed: boolean, x: number, y: number): Uint8Array`
  - `encodeWheel(deltaX: number, deltaY: number): Uint8Array`
  - `encodeKey(scancode: number, pressed: boolean, extended: boolean): Uint8Array`
  - `PROTOCOL_VERSION: 1`

- [ ] **Step 1: Écrire les vecteurs de test**

`proto/vectors.json` — chaque entrée décrit un message et sa forme binaire attendue :

```json
{
  "comment": "Vecteurs partagés Rust/TypeScript. Toute modification doit être répercutée des deux côtés.",
  "version": 1,
  "cases": [
    { "name": "mouse_move_origine", "kind": "mouse_move", "x": 0, "y": 0, "bytes": [1, 1, 0, 0, 0, 0] },
    { "name": "mouse_move_max", "kind": "mouse_move", "x": 65535, "y": 65535, "bytes": [1, 1, 255, 255, 255, 255] },
    { "name": "mouse_move_petit_boutiste", "kind": "mouse_move", "x": 513, "y": 1027, "bytes": [1, 1, 1, 2, 3, 4] },
    { "name": "bouton_gauche_enfonce", "kind": "mouse_button", "button": 0, "pressed": true, "x": 256, "y": 512, "bytes": [1, 2, 0, 1, 0, 1, 0, 2] },
    { "name": "bouton_milieu_relache", "kind": "mouse_button", "button": 2, "pressed": false, "x": 0, "y": 0, "bytes": [1, 2, 2, 0, 0, 0, 0, 0] },
    { "name": "molette_bas", "kind": "wheel", "delta_x": 0, "delta_y": 120, "bytes": [1, 3, 0, 0, 120, 0] },
    { "name": "molette_haut_negatif", "kind": "wheel", "delta_x": -120, "delta_y": -240, "bytes": [1, 3, 136, 255, 16, 255] },
    { "name": "touche_a_enfoncee", "kind": "key", "scancode": 30, "pressed": true, "extended": false, "bytes": [1, 4, 30, 0, 1, 0] },
    { "name": "fleche_haut_etendue", "kind": "key", "scancode": 72, "pressed": false, "extended": true, "bytes": [1, 4, 72, 0, 0, 1] }
  ]
}
```

- [ ] **Step 2: Ajouter le test Rust de conformité aux vecteurs**

Dans le module `tests` de `proto/src/input.rs` :

```rust
    #[test]
    fn conformite_aux_vecteurs_partages() {
        let raw = include_str!("../vectors.json");
        let doc: serde_json::Value = serde_json::from_str(raw).expect("vectors.json valide");
        let cases = doc["cases"].as_array().expect("tableau de cas");
        assert!(!cases.is_empty(), "au moins un vecteur attendu");

        for case in cases {
            let name = case["name"].as_str().unwrap();
            let expected: Vec<u8> = case["bytes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u8)
                .collect();

            let msg = match case["kind"].as_str().unwrap() {
                "mouse_move" => InputMessage::MouseMove {
                    x: case["x"].as_u64().unwrap() as u16,
                    y: case["y"].as_u64().unwrap() as u16,
                },
                "mouse_button" => InputMessage::MouseButton {
                    button: MouseButton::from_u8(case["button"].as_u64().unwrap() as u8).unwrap(),
                    pressed: case["pressed"].as_bool().unwrap(),
                    x: case["x"].as_u64().unwrap() as u16,
                    y: case["y"].as_u64().unwrap() as u16,
                },
                "wheel" => InputMessage::Wheel {
                    delta_x: case["delta_x"].as_i64().unwrap() as i16,
                    delta_y: case["delta_y"].as_i64().unwrap() as i16,
                },
                "key" => InputMessage::Key {
                    scancode: case["scancode"].as_u64().unwrap() as u16,
                    pressed: case["pressed"].as_bool().unwrap(),
                    extended: case["extended"].as_bool().unwrap(),
                },
                other => panic!("type de vecteur inconnu : {other}"),
            };

            assert_eq!(msg.encode(), expected, "encodage du vecteur « {name} »");
            assert_eq!(
                InputMessage::decode(&expected).expect("décodage du vecteur"),
                msg,
                "décodage du vecteur « {name} »"
            );
        }
    }
```

- [ ] **Step 3: Lancer le test Rust**

Run: `cargo test -p proto conformite`
Expected: PASS. En cas d'échec, c'est `vectors.json` qui fait foi seulement si la spec du format le confirme — sinon corriger le vecteur.

- [ ] **Step 4: Initialiser le paquet TypeScript de proto**

`proto/package.json` :

```json
{
  "name": "@guacamole/proto",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "main": "./ts/input.ts",
  "scripts": {
    "test": "vitest run"
  },
  "devDependencies": {
    "typescript": "^5.5.0",
    "vitest": "^2.0.0"
  }
}
```

`proto/tsconfig.json` :

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true,
    "resolveJsonModule": true,
    "noEmit": true,
    "lib": ["ES2022", "DOM"]
  },
  "include": ["ts/**/*.ts"]
}
```

```bash
cd proto && npm install && cd ..
```

- [ ] **Step 5: Écrire le test TypeScript d'abord**

`proto/ts/input.test.ts` :

```typescript
import { describe, expect, it } from 'vitest';
import vectors from '../vectors.json';
import {
    PROTOCOL_VERSION,
    encodeKey,
    encodeMouseButton,
    encodeMouseMove,
    encodeWheel,
} from './input';

describe('encodeur du protocole d\'entrée', () => {
    it('déclare la même version que les vecteurs', () => {
        expect(PROTOCOL_VERSION).toBe(vectors.version);
    });

    it.each(vectors.cases)('produit les octets attendus pour « $name »', (testCase) => {
        let actual: Uint8Array;
        switch (testCase.kind) {
            case 'mouse_move':
                actual = encodeMouseMove(testCase.x!, testCase.y!);
                break;
            case 'mouse_button':
                actual = encodeMouseButton(
                    testCase.button! as 0 | 1 | 2,
                    testCase.pressed!,
                    testCase.x!,
                    testCase.y!,
                );
                break;
            case 'wheel':
                actual = encodeWheel(testCase.delta_x!, testCase.delta_y!);
                break;
            case 'key':
                actual = encodeKey(testCase.scancode!, testCase.pressed!, testCase.extended!);
                break;
            default:
                throw new Error(`type de vecteur inconnu : ${testCase.kind}`);
        }
        expect(Array.from(actual)).toEqual(testCase.bytes);
    });

    it('borne les coordonnées hors plage', () => {
        expect(Array.from(encodeMouseMove(-10, 99999))).toEqual([1, 1, 0, 0, 255, 255]);
    });

    it('borne les deltas de molette hors plage', () => {
        expect(Array.from(encodeWheel(-40000, 40000))).toEqual([1, 3, 0, 128, 255, 127]);
    });
});
```

- [ ] **Step 6: Lancer le test pour vérifier qu'il échoue**

Run: `cd proto && npx vitest run`
Expected: échec — le module `./input` n'existe pas.

- [ ] **Step 7: Écrire l'encodeur TypeScript**

`proto/ts/input.ts` :

```typescript
// Encodeur binaire des messages d'entrée. Doit rester strictement aligné sur
// proto/src/input.rs — les vecteurs de vectors.json vérifient les deux côtés.

export const PROTOCOL_VERSION = 1;

const TYPE_MOUSE_MOVE = 1;
const TYPE_MOUSE_BUTTON = 2;
const TYPE_WHEEL = 3;
const TYPE_KEY = 4;

export type MouseButtonCode = 0 | 1 | 2; // gauche, droit, milieu

function clampU16(value: number): number {
    return Math.max(0, Math.min(65535, Math.round(value)));
}

function clampI16(value: number): number {
    return Math.max(-32768, Math.min(32767, Math.round(value)));
}

export function encodeMouseMove(x: number, y: number): Uint8Array {
    const buffer = new Uint8Array(6);
    const view = new DataView(buffer.buffer);
    buffer[0] = PROTOCOL_VERSION;
    buffer[1] = TYPE_MOUSE_MOVE;
    view.setUint16(2, clampU16(x), true);
    view.setUint16(4, clampU16(y), true);
    return buffer;
}

export function encodeMouseButton(
    button: MouseButtonCode,
    pressed: boolean,
    x: number,
    y: number,
): Uint8Array {
    const buffer = new Uint8Array(8);
    const view = new DataView(buffer.buffer);
    buffer[0] = PROTOCOL_VERSION;
    buffer[1] = TYPE_MOUSE_BUTTON;
    buffer[2] = button;
    buffer[3] = pressed ? 1 : 0;
    view.setUint16(4, clampU16(x), true);
    view.setUint16(6, clampU16(y), true);
    return buffer;
}

export function encodeWheel(deltaX: number, deltaY: number): Uint8Array {
    const buffer = new Uint8Array(6);
    const view = new DataView(buffer.buffer);
    buffer[0] = PROTOCOL_VERSION;
    buffer[1] = TYPE_WHEEL;
    view.setInt16(2, clampI16(deltaX), true);
    view.setInt16(4, clampI16(deltaY), true);
    return buffer;
}

export function encodeKey(scancode: number, pressed: boolean, extended: boolean): Uint8Array {
    const buffer = new Uint8Array(6);
    const view = new DataView(buffer.buffer);
    buffer[0] = PROTOCOL_VERSION;
    buffer[1] = TYPE_KEY;
    view.setUint16(2, clampU16(scancode), true);
    buffer[4] = pressed ? 1 : 0;
    buffer[5] = extended ? 1 : 0;
    return buffer;
}
```

- [ ] **Step 8: Lancer les tests TypeScript**

Run: `cd proto && npx vitest run`
Expected: 12 tests réussis (1 version + 9 vecteurs + 2 bornes).

- [ ] **Step 9: Committer**

```bash
git add proto/vectors.json proto/ts proto/package.json proto/tsconfig.json proto/package-lock.json proto/src/input.rs
git commit -m "feat: vecteurs de test partagés et encodeur d'entrées TypeScript"
```

---

### Task 4: Protocole de contrôle (JSON versionné)

Le canal de contrôle est fiable et ordonné, à faible débit : JSON versionné, lisible dans les journaux. Il porte le redimensionnement (client → agent) et les événements de session (agent → client).

**Files:**
- Modify: `proto/src/control.rs`
- Create: `proto/ts/control.ts`

**Interfaces:**
- Consumes: rien.
- Produces:
  - Rust : `enum ClientControl { Resize { width: u32, height: u32 } }`, `enum AgentControl { Ready { width: u32, height: u32 }, SessionEnd { reason: String } }`, sérialisés avec `{"v":1,"type":"...","...":...}`.
  - TypeScript : types `ClientControl`, `AgentControl`, fonctions `encodeResize(width, height): string`, `parseAgentControl(raw: string): AgentControl`.

- [ ] **Step 1: Écrire les tests Rust d'abord**

`proto/src/control.rs` :

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialise_le_redimensionnement() {
        let json = serde_json::to_string(&ClientControl::Resize { width: 1280, height: 720 })
            .expect("sérialisation");
        assert_eq!(json, r#"{"v":1,"type":"resize","width":1280,"height":720}"#);
    }

    #[test]
    fn deserialise_le_redimensionnement() {
        let msg: ClientControl =
            serde_json::from_str(r#"{"v":1,"type":"resize","width":800,"height":600}"#)
                .expect("désérialisation");
        assert_eq!(msg, ClientControl::Resize { width: 800, height: 600 });
    }

    #[test]
    fn serialise_ready_et_session_end() {
        assert_eq!(
            serde_json::to_string(&AgentControl::Ready { width: 1920, height: 1080 }).unwrap(),
            r#"{"v":1,"type":"ready","width":1920,"height":1080}"#
        );
        assert_eq!(
            serde_json::to_string(&AgentControl::SessionEnd {
                reason: "fenêtre fermée".into()
            })
            .unwrap(),
            r#"{"v":1,"type":"session-end","reason":"fenêtre fermée"}"#
        );
    }

    #[test]
    fn rejette_un_type_inconnu() {
        let err = serde_json::from_str::<ClientControl>(r#"{"v":1,"type":"vol","x":1}"#);
        assert!(err.is_err());
    }

    #[test]
    fn rejette_une_version_absente() {
        let err = serde_json::from_str::<ClientControl>(r#"{"type":"resize","width":1,"height":1}"#);
        assert!(err.is_err());
    }
}
```

- [ ] **Step 2: Lancer les tests pour vérifier qu'ils échouent**

Run: `cargo test -p proto control`
Expected: échec de compilation — `ClientControl` n'existe pas.

- [ ] **Step 3: Écrire l'implémentation**

En tête de `proto/src/control.rs` :

```rust
//! Messages du canal de contrôle (fiable, ordonné, faible débit).
//!
//! Format JSON versionné : `{"v":1,"type":"...","...":...}`. Le champ `v` est
//! obligatoire et vérifié à la désérialisation.

use serde::{Deserialize, Serialize};

/// Version du protocole de contrôle. Incrémenter à tout changement de format.
pub const CONTROL_VERSION: u8 = 1;

fn version() -> u8 {
    CONTROL_VERSION
}

fn verifie_version<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = u8::deserialize(deserializer)?;
    if v != CONTROL_VERSION {
        return Err(serde::de::Error::custom(format!(
            "version de contrôle non supportée : {v}"
        )));
    }
    Ok(v)
}

/// Message du client web vers l'agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ClientControl {
    Resize {
        #[serde(rename = "v", default = "version", deserialize_with = "verifie_version")]
        version: u8,
        width: u32,
        height: u32,
    },
}

/// Message de l'agent vers le client web.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum AgentControl {
    Ready {
        #[serde(rename = "v", default = "version", deserialize_with = "verifie_version")]
        version: u8,
        width: u32,
        height: u32,
    },
    SessionEnd {
        #[serde(rename = "v", default = "version", deserialize_with = "verifie_version")]
        version: u8,
        reason: String,
    },
}
```

Note : `serde` sérialise les champs dans l'ordre de déclaration, mais `tag` est émis en premier. Si l'ordre obtenu diffère de celui attendu par les tests de l'étape 1, ajuster les chaînes attendues dans les tests plutôt que de forcer l'ordre — seul le contenu compte pour l'interopérabilité.

Les tests de l'étape 1 utilisent la forme abrégée `ClientControl::Resize { width, height }` : pour que cela compile, ajouter des constructeurs et adapter les tests aux champs `version` :

```rust
impl ClientControl {
    pub fn resize(width: u32, height: u32) -> Self {
        ClientControl::Resize { version: CONTROL_VERSION, width, height }
    }
}

impl AgentControl {
    pub fn ready(width: u32, height: u32) -> Self {
        AgentControl::Ready { version: CONTROL_VERSION, width, height }
    }

    pub fn session_end(reason: impl Into<String>) -> Self {
        AgentControl::SessionEnd { version: CONTROL_VERSION, reason: reason.into() }
    }
}
```

Remplacer dans les tests `ClientControl::Resize { width: 1280, height: 720 }` par `ClientControl::resize(1280, 720)`, `AgentControl::Ready { .. }` par `AgentControl::ready(1920, 1080)` et `AgentControl::SessionEnd { .. }` par `AgentControl::session_end("fenêtre fermée")`. Le test de désérialisation compare à `ClientControl::resize(800, 600)`.

- [ ] **Step 4: Lancer les tests pour vérifier qu'ils passent**

Run: `cargo test -p proto`
Expected: tous les tests de `proto` réussis (input + control).

- [ ] **Step 5: Écrire le miroir TypeScript**

`proto/ts/control.ts` :

```typescript
// Messages du canal de contrôle. Doit rester aligné sur proto/src/control.rs.

export const CONTROL_VERSION = 1;

export interface ResizeMessage {
    v: number;
    type: 'resize';
    width: number;
    height: number;
}

export type ClientControl = ResizeMessage;

export interface ReadyMessage {
    v: number;
    type: 'ready';
    width: number;
    height: number;
}

export interface SessionEndMessage {
    v: number;
    type: 'session-end';
    reason: string;
}

export type AgentControl = ReadyMessage | SessionEndMessage;

export function encodeResize(width: number, height: number): string {
    const message: ResizeMessage = {
        v: CONTROL_VERSION,
        type: 'resize',
        width: Math.max(1, Math.round(width)),
        height: Math.max(1, Math.round(height)),
    };
    return JSON.stringify(message);
}

export function parseAgentControl(raw: string): AgentControl {
    const parsed = JSON.parse(raw) as Partial<AgentControl>;
    if (parsed.v !== CONTROL_VERSION) {
        throw new Error(`version de contrôle non supportée : ${parsed.v}`);
    }
    if (parsed.type !== 'ready' && parsed.type !== 'session-end') {
        throw new Error(`type de contrôle inconnu : ${parsed.type}`);
    }
    return parsed as AgentControl;
}
```

- [ ] **Step 6: Ajouter les tests TypeScript**

`proto/ts/control.test.ts` :

```typescript
import { describe, expect, it } from 'vitest';
import { CONTROL_VERSION, encodeResize, parseAgentControl } from './control';

describe('protocole de contrôle', () => {
    it('encode un redimensionnement', () => {
        expect(JSON.parse(encodeResize(1280, 720))).toEqual({
            v: CONTROL_VERSION,
            type: 'resize',
            width: 1280,
            height: 720,
        });
    });

    it('arrondit et borne les dimensions', () => {
        expect(JSON.parse(encodeResize(0, 719.6))).toEqual({
            v: CONTROL_VERSION,
            type: 'resize',
            width: 1,
            height: 720,
        });
    });

    it('analyse un message ready', () => {
        const msg = parseAgentControl('{"v":1,"type":"ready","width":800,"height":600}');
        expect(msg).toEqual({ v: 1, type: 'ready', width: 800, height: 600 });
    });

    it('analyse une fin de session', () => {
        const msg = parseAgentControl('{"v":1,"type":"session-end","reason":"fermée"}');
        expect(msg.type).toBe('session-end');
    });

    it('rejette une version inconnue', () => {
        expect(() => parseAgentControl('{"v":9,"type":"ready","width":1,"height":1}')).toThrow(
            /version de contrôle/,
        );
    });

    it('rejette un type inconnu', () => {
        expect(() => parseAgentControl('{"v":1,"type":"autre"}')).toThrow(/type de contrôle/);
    });
});
```

- [ ] **Step 7: Lancer les tests TypeScript**

Run: `cd proto && npx vitest run`
Expected: tous les tests réussis (input + control).

- [ ] **Step 8: Committer**

```bash
git add proto/src/control.rs proto/ts/control.ts proto/ts/control.test.ts
git commit -m "feat: protocole de contrôle JSON versionné"
```

---

### Task 5: Serveur de signaling

Rôle unique : mettre en relation un agent et un client par identifiant de session, et relayer l'offre et la réponse SDP. Pas de trickle ICE — le réseau est local et les candidats hôtes voyagent dans le SDP.

**Files:**
- Create: `signaling/package.json`, `signaling/tsconfig.json`, `signaling/src/server.ts`, `signaling/src/server.test.ts`

**Interfaces:**
- Consumes: rien.
- Produces:
  - `createSignalingServer(port: number): { close(): Promise<void>, port: number }`
  - Protocole WebSocket (JSON) : `{role:'agent'|'client', session:string}` en premier message, puis `{type:'offer', sdp:string}` (client → agent) et `{type:'answer', sdp:string}` (agent → client). Le serveur émet `{type:'peer-gone'}` quand le pair se déconnecte, `{type:'error', reason:string}` en cas de protocole invalide.

- [ ] **Step 1: Initialiser le paquet**

`signaling/package.json` :

```json
{
  "name": "@guacamole/signaling",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "start": "tsx src/index.ts",
    "test": "vitest run"
  },
  "dependencies": {
    "ws": "^8.18.0"
  },
  "devDependencies": {
    "@types/ws": "^8.5.12",
    "tsx": "^4.19.0",
    "typescript": "^5.5.0",
    "vitest": "^2.0.0"
  }
}
```

`signaling/tsconfig.json` :

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true,
    "noEmit": true,
    "types": ["node"],
    "lib": ["ES2022"]
  },
  "include": ["src/**/*.ts"]
}
```

```bash
cd signaling && npm install && cd ..
```

- [ ] **Step 2: Écrire les tests d'abord**

`signaling/src/server.test.ts` :

```typescript
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { WebSocket } from 'ws';
import { createSignalingServer } from './server';

let server: ReturnType<typeof createSignalingServer>;

function connect(role: 'agent' | 'client', session: string): Promise<WebSocket> {
    return new Promise((resolve, reject) => {
        const ws = new WebSocket(`ws://127.0.0.1:${server.port}`);
        ws.on('error', reject);
        ws.on('open', () => {
            ws.send(JSON.stringify({ role, session }));
            resolve(ws);
        });
    });
}

function nextMessage(ws: WebSocket): Promise<any> {
    return new Promise((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error('aucun message reçu')), 2000);
        ws.once('message', (raw) => {
            clearTimeout(timer);
            resolve(JSON.parse(raw.toString()));
        });
    });
}

beforeEach(() => {
    server = createSignalingServer(0);
});

afterEach(async () => {
    await server.close();
});

describe('serveur de signaling', () => {
    it('relaie une offre du client vers l\'agent', async () => {
        const agent = await connect('agent', 's1');
        const client = await connect('client', 's1');

        client.send(JSON.stringify({ type: 'offer', sdp: 'v=0 offre' }));
        expect(await nextMessage(agent)).toEqual({ type: 'offer', sdp: 'v=0 offre' });

        agent.close();
        client.close();
    });

    it('relaie une réponse de l\'agent vers le client', async () => {
        const agent = await connect('agent', 's2');
        const client = await connect('client', 's2');

        agent.send(JSON.stringify({ type: 'answer', sdp: 'v=0 reponse' }));
        expect(await nextMessage(client)).toEqual({ type: 'answer', sdp: 'v=0 reponse' });

        agent.close();
        client.close();
    });

    it('isole les sessions entre elles', async () => {
        const agentA = await connect('agent', 'sa');
        const clientB = await connect('client', 'sb');

        clientB.send(JSON.stringify({ type: 'offer', sdp: 'pour sb' }));
        await expect(nextMessage(agentA)).rejects.toThrow(/aucun message/);

        agentA.close();
        clientB.close();
    });

    it('signale la disparition du pair', async () => {
        const agent = await connect('agent', 's3');
        const client = await connect('client', 's3');

        agent.close();
        expect(await nextMessage(client)).toEqual({ type: 'peer-gone' });

        client.close();
    });

    it('rejette un premier message invalide', async () => {
        const ws = new WebSocket(`ws://127.0.0.1:${server.port}`);
        await new Promise((resolve) => ws.on('open', resolve));
        ws.send(JSON.stringify({ bonjour: true }));
        expect(await nextMessage(ws)).toEqual({
            type: 'error',
            reason: 'premier message invalide : {role, session} attendu',
        });
        ws.close();
    });

    it('rejette un second agent sur la même session', async () => {
        const first = await connect('agent', 's4');
        const second = await connect('agent', 's4');
        expect(await nextMessage(second)).toEqual({
            type: 'error',
            reason: 'un agent est déjà connecté à la session s4',
        });
        first.close();
        second.close();
    });
});
```

- [ ] **Step 3: Lancer les tests pour vérifier qu'ils échouent**

Run: `cd signaling && npx vitest run`
Expected: échec — le module `./server` n'existe pas.

- [ ] **Step 4: Écrire le serveur**

`signaling/src/server.ts` :

```typescript
// Serveur de signaling : met en relation un agent et un client par session et
// relaie l'offre et la réponse SDP. Aucun état persistant, aucune authentification
// (jalon 1, réseau local).

import { WebSocket, WebSocketServer } from 'ws';

type Role = 'agent' | 'client';

interface Session {
    agent?: WebSocket;
    client?: WebSocket;
}

export interface SignalingServer {
    port: number;
    close(): Promise<void>;
}

export function createSignalingServer(port: number): SignalingServer {
    const wss = new WebSocketServer({ port });
    const sessions = new Map<string, Session>();

    function send(socket: WebSocket | undefined, payload: unknown): void {
        if (socket && socket.readyState === WebSocket.OPEN) {
            socket.send(JSON.stringify(payload));
        }
    }

    wss.on('connection', (socket) => {
        let role: Role | undefined;
        let sessionId: string | undefined;

        socket.on('message', (raw) => {
            let message: any;
            try {
                message = JSON.parse(raw.toString());
            } catch {
                send(socket, { type: 'error', reason: 'JSON invalide' });
                return;
            }

            // Premier message : déclaration de rôle et de session.
            if (!role) {
                const declaredRole = message.role;
                const declaredSession = message.session;
                if (
                    (declaredRole !== 'agent' && declaredRole !== 'client') ||
                    typeof declaredSession !== 'string' ||
                    declaredSession.length === 0
                ) {
                    send(socket, {
                        type: 'error',
                        reason: 'premier message invalide : {role, session} attendu',
                    });
                    return;
                }

                const session = sessions.get(declaredSession) ?? {};
                if (session[declaredRole]) {
                    send(socket, {
                        type: 'error',
                        reason: `un ${declaredRole} est déjà connecté à la session ${declaredSession}`,
                    });
                    return;
                }

                role = declaredRole;
                sessionId = declaredSession;
                session[declaredRole] = socket;
                sessions.set(declaredSession, session);
                return;
            }

            // Messages suivants : relais vers le pair.
            const session = sessions.get(sessionId!);
            if (!session) return;
            const peer = role === 'client' ? session.agent : session.client;

            if (message.type === 'offer' || message.type === 'answer') {
                send(peer, { type: message.type, sdp: message.sdp });
            } else {
                send(socket, { type: 'error', reason: `type inconnu : ${message.type}` });
            }
        });

        socket.on('close', () => {
            if (!role || !sessionId) return;
            const session = sessions.get(sessionId);
            if (!session) return;

            delete session[role];
            const peer = role === 'client' ? session.agent : session.client;
            send(peer, { type: 'peer-gone' });

            if (!session.agent && !session.client) {
                sessions.delete(sessionId);
            }
        });
    });

    return {
        get port(): number {
            const address = wss.address();
            return typeof address === 'object' && address ? address.port : port;
        },
        close(): Promise<void> {
            return new Promise((resolve) => {
                for (const socket of wss.clients) socket.terminate();
                wss.close(() => resolve());
            });
        },
    };
}
```

- [ ] **Step 5: Ajouter le point d'entrée**

`signaling/src/index.ts` :

```typescript
import { createSignalingServer } from './server';

const port = Number(process.env.SIGNALING_PORT ?? 8080);
const server = createSignalingServer(port);
console.log(`signaling à l'écoute sur le port ${server.port}`);

process.on('SIGINT', async () => {
    await server.close();
    process.exit(0);
});
```

- [ ] **Step 6: Lancer les tests pour vérifier qu'ils passent**

Run: `cd signaling && npx vitest run`
Expected: 6 tests réussis.

Note : le test « rejette un second agent » attend le port dans le message d'erreur avec le rôle au singulier (`un agent est déjà connecté`). Le code produit exactement cette chaîne pour `declaredRole === 'agent'`.

- [ ] **Step 7: Committer**

```bash
git add signaling
git commit -m "feat: serveur de signaling avec relais SDP par session"
```

---

### Task 6: Découpage Annex-B et source vidéo abstraite

Deux besoins convergent ici. D'une part, l'encodeur Media Foundation produira un flux Annex-B qu'il faut découper en unités d'accès avant de le confier à str0m. D'autre part, pour dérisquer le transport sans écrire une ligne de code Windows, il faut une source vidéo de test : un fichier H.264 pré-encodé, rejoué en boucle. Le trait `VideoSource` sert les deux cas.

**Files:**
- Create: `agent/src/h264.rs`, `agent/src/source.rs`
- Modify: `agent/src/main.rs` (déclaration des modules), `agent/Cargo.toml`
- Test: modules `#[cfg(test)]` dans les deux fichiers

**Interfaces:**
- Consumes: rien.
- Produces:
  - `struct AccessUnit { pub data: Vec<u8>, pub is_keyframe: bool, pub pts_90k: u64 }`
  - `fn split_annex_b(stream: &[u8]) -> Vec<Vec<u8>>` — découpe un flux Annex-B en NAL (start codes retirés)
  - `fn is_keyframe(nals: &[Vec<u8>]) -> bool` — vrai si une NAL de type 5 (IDR) est présente
  - `fn group_access_units(stream: &[u8], fps: u32) -> Vec<AccessUnit>` — regroupe les NAL en unités d'accès horodatées
  - `trait VideoSource { fn next_frame(&mut self) -> Option<AccessUnit>; fn dimensions(&self) -> (u32, u32); }`
  - `struct FileSource` avec `FileSource::from_annex_b(data: Vec<u8>, width: u32, height: u32, fps: u32) -> anyhow::Result<FileSource>`

- [ ] **Step 1: Écrire les tests du découpage Annex-B**

`agent/src/h264.rs` :

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoupe_avec_start_code_de_quatre_octets() {
        let stream = [0, 0, 0, 1, 0x67, 0xAA, 0, 0, 0, 1, 0x68, 0xBB];
        let nals = split_annex_b(&stream);
        assert_eq!(nals, vec![vec![0x67, 0xAA], vec![0x68, 0xBB]]);
    }

    #[test]
    fn decoupe_avec_start_code_de_trois_octets() {
        let stream = [0, 0, 1, 0x67, 0xAA, 0, 0, 1, 0x65, 0xBB];
        let nals = split_annex_b(&stream);
        assert_eq!(nals, vec![vec![0x67, 0xAA], vec![0x65, 0xBB]]);
    }

    #[test]
    fn ignore_les_octets_avant_le_premier_start_code() {
        let stream = [0xFF, 0xFF, 0, 0, 0, 1, 0x65, 0x01];
        assert_eq!(split_annex_b(&stream), vec![vec![0x65, 0x01]]);
    }

    #[test]
    fn renvoie_rien_sans_start_code() {
        assert!(split_annex_b(&[0xFF, 0xFE, 0xFD]).is_empty());
        assert!(split_annex_b(&[]).is_empty());
    }

    #[test]
    fn ignore_les_nal_vides() {
        let stream = [0, 0, 0, 1, 0, 0, 0, 1, 0x65, 0x01];
        assert_eq!(split_annex_b(&stream), vec![vec![0x65, 0x01]]);
    }

    #[test]
    fn detecte_une_image_cle_sur_nal_idr() {
        // Type de NAL = 5 bits de poids faible du premier octet.
        assert!(is_keyframe(&[vec![0x65, 0x00]])); // 0x65 & 0x1F == 5 → IDR
        assert!(!is_keyframe(&[vec![0x41, 0x00]])); // 0x41 & 0x1F == 1 → non IDR
        assert!(is_keyframe(&[vec![0x67, 0x00], vec![0x65, 0x00]])); // SPS puis IDR
        assert!(!is_keyframe(&[]));
    }

    #[test]
    fn regroupe_en_unites_d_acces_horodatees() {
        // Deux images : SPS+PPS+IDR, puis une tranche non-IDR.
        let mut stream = Vec::new();
        for nal in [vec![0x67u8, 0x42], vec![0x68, 0xCE], vec![0x65, 0x88]] {
            stream.extend_from_slice(&[0, 0, 0, 1]);
            stream.extend_from_slice(&nal);
        }
        stream.extend_from_slice(&[0, 0, 0, 1]);
        stream.extend_from_slice(&[0x41, 0x9A]);

        let units = group_access_units(&stream, 60);
        assert_eq!(units.len(), 2);
        assert!(units[0].is_keyframe);
        assert!(!units[1].is_keyframe);
        assert_eq!(units[0].pts_90k, 0);
        assert_eq!(units[1].pts_90k, 1500); // 90000 / 60
        // La première unité contient les trois NAL avec leurs start codes.
        assert_eq!(units[0].data.len(), 3 * 4 + 2 + 2 + 2);
    }

    #[test]
    fn regroupe_un_flux_vide_sans_panique() {
        assert!(group_access_units(&[], 60).is_empty());
    }
}
```

- [ ] **Step 2: Lancer les tests pour vérifier qu'ils échouent**

Run: `cargo test -p agent h264`
Expected: échec de compilation — les fonctions n'existent pas.

- [ ] **Step 3: Écrire l'implémentation du découpage**

En tête de `agent/src/h264.rs` :

```rust
//! Manipulation de flux H.264 en format Annex-B.
//!
//! L'encodeur Media Foundation produit des NAL préfixées par des start codes
//! (`00 00 01` ou `00 00 00 01`). str0m attend des unités d'accès complètes,
//! une par image affichée.

/// Horloge RTP de la vidéo, en hertz. Valeur imposée par la RFC 3551.
pub const CLOCK_RATE_HZ: u64 = 90_000;

/// Une unité d'accès : toutes les NAL composant une image affichable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessUnit {
    /// Flux Annex-B complet de l'unité, start codes inclus.
    pub data: Vec<u8>,
    pub is_keyframe: bool,
    /// Horodatage de présentation, en unités de 1/90000 s.
    pub pts_90k: u64,
}

/// Découpe un flux Annex-B en NAL, start codes retirés.
pub fn split_annex_b(stream: &[u8]) -> Vec<Vec<u8>> {
    let mut nals = Vec::new();
    let mut start: Option<usize> = None;
    let mut i = 0;

    while i < stream.len() {
        let code_len = start_code_len(stream, i);
        if code_len > 0 {
            if let Some(begin) = start {
                push_nal(&mut nals, &stream[begin..i]);
            }
            i += code_len;
            start = Some(i);
        } else {
            i += 1;
        }
    }

    if let Some(begin) = start {
        push_nal(&mut nals, &stream[begin..]);
    }
    nals
}

fn push_nal(nals: &mut Vec<Vec<u8>>, nal: &[u8]) {
    if !nal.is_empty() {
        nals.push(nal.to_vec());
    }
}

/// Longueur du start code à la position donnée, ou 0 s'il n'y en a pas.
fn start_code_len(stream: &[u8], i: usize) -> usize {
    if stream[i..].starts_with(&[0, 0, 0, 1]) {
        4
    } else if stream[i..].starts_with(&[0, 0, 1]) {
        3
    } else {
        0
    }
}

const NAL_TYPE_IDR: u8 = 5;
const NAL_TYPE_NON_IDR: u8 = 1;
const NAL_TYPE_SPS: u8 = 7;

fn nal_type(nal: &[u8]) -> u8 {
    nal.first().map_or(0, |b| b & 0x1F)
}

/// Vrai si l'ensemble de NAL contient une image de référence instantanée.
pub fn is_keyframe(nals: &[Vec<u8>]) -> bool {
    nals.iter().any(|nal| nal_type(nal) == NAL_TYPE_IDR)
}

/// Regroupe les NAL d'un flux en unités d'accès, une par image affichable.
///
/// Une nouvelle unité commence à chaque NAL de tranche (IDR ou non-IDR) ; les
/// NAL de paramètres (SPS/PPS) qui la précèdent lui sont rattachées.
pub fn group_access_units(stream: &[u8], fps: u32) -> Vec<AccessUnit> {
    let nals = split_annex_b(stream);
    let tick = if fps == 0 { 0 } else { CLOCK_RATE_HZ / fps as u64 };

    let mut units: Vec<AccessUnit> = Vec::new();
    let mut current: Vec<Vec<u8>> = Vec::new();
    let mut slice_seen = false;

    for nal in nals {
        let kind = nal_type(&nal);
        let is_slice = kind == NAL_TYPE_IDR || kind == NAL_TYPE_NON_IDR;

        // Une NAL de paramètres après une tranche ouvre l'unité suivante.
        if slice_seen && (is_slice || kind == NAL_TYPE_SPS) {
            flush(&mut units, &mut current, tick);
            slice_seen = false;
        }

        if is_slice {
            slice_seen = true;
        }
        current.push(nal);
    }
    flush(&mut units, &mut current, tick);
    units
}

fn flush(units: &mut Vec<AccessUnit>, current: &mut Vec<Vec<u8>>, tick: u64) {
    if current.is_empty() {
        return;
    }
    let mut data = Vec::new();
    for nal in current.iter() {
        data.extend_from_slice(&[0, 0, 0, 1]);
        data.extend_from_slice(nal);
    }
    let index = units.len() as u64;
    units.push(AccessUnit {
        is_keyframe: is_keyframe(current),
        data,
        pts_90k: index * tick,
    });
    current.clear();
}
```

- [ ] **Step 4: Déclarer les modules et lancer les tests**

Ajouter en tête de `agent/src/main.rs` :

```rust
mod h264;
mod source;
```

Créer `agent/src/source.rs` vide pour l'instant.

Run: `cargo test -p agent h264`
Expected: 8 tests réussis.

- [ ] **Step 5: Écrire les tests de la source vidéo**

`agent/src/source.rs` :

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Construit un flux Annex-B de `frames` images, la première étant une IDR.
    fn flux_de_test(frames: usize) -> Vec<u8> {
        let mut stream = Vec::new();
        for i in 0..frames {
            let nal: Vec<u8> = if i == 0 { vec![0x65, 0x88] } else { vec![0x41, 0x9A] };
            stream.extend_from_slice(&[0, 0, 0, 1]);
            stream.extend_from_slice(&nal);
        }
        stream
    }

    #[test]
    fn expose_ses_dimensions() {
        let source = FileSource::from_annex_b(flux_de_test(2), 1280, 720, 60).unwrap();
        assert_eq!(source.dimensions(), (1280, 720));
    }

    #[test]
    fn rejette_un_flux_sans_image() {
        let err = FileSource::from_annex_b(vec![0xFF, 0xFE], 1280, 720, 60).unwrap_err();
        assert!(err.to_string().contains("aucune unité d'accès"));
    }

    #[test]
    fn rejette_un_flux_sans_image_cle() {
        let stream = {
            let mut s = Vec::new();
            s.extend_from_slice(&[0, 0, 0, 1]);
            s.extend_from_slice(&[0x41, 0x9A]);
            s
        };
        let err = FileSource::from_annex_b(stream, 1280, 720, 60).unwrap_err();
        assert!(err.to_string().contains("aucune image clé"));
    }

    #[test]
    fn rejoue_en_boucle_avec_des_horodatages_croissants() {
        let mut source = FileSource::from_annex_b(flux_de_test(3), 640, 480, 60).unwrap();
        let mut horodatages = Vec::new();
        for _ in 0..7 {
            horodatages.push(source.next_frame().unwrap().pts_90k);
        }
        // 1500 ticks par image à 60 fps ; les horodatages ne redémarrent jamais.
        assert_eq!(horodatages, vec![0, 1500, 3000, 4500, 6000, 7500, 9000]);
    }

    #[test]
    fn la_premiere_image_de_chaque_boucle_est_une_image_cle() {
        let mut source = FileSource::from_annex_b(flux_de_test(3), 640, 480, 60).unwrap();
        assert!(source.next_frame().unwrap().is_keyframe);
        assert!(!source.next_frame().unwrap().is_keyframe);
        assert!(!source.next_frame().unwrap().is_keyframe);
        assert!(source.next_frame().unwrap().is_keyframe); // début de la boucle suivante
    }
}
```

- [ ] **Step 6: Lancer les tests pour vérifier qu'ils échouent**

Run: `cargo test -p agent source`
Expected: échec de compilation — `FileSource` n'existe pas.

- [ ] **Step 7: Écrire la source vidéo**

En tête de `agent/src/source.rs` :

```rust
//! Sources vidéo produisant des unités d'accès H.264 prêtes à être envoyées.

use crate::h264::{group_access_units, AccessUnit, CLOCK_RATE_HZ};
use anyhow::{bail, Result};

/// Producteur d'unités d'accès H.264.
///
/// L'implémentation Windows (capture + encodage) et la source fichier de test
/// se substituent l'une à l'autre derrière ce trait.
pub trait VideoSource {
    /// Unité d'accès suivante, ou `None` si la source est épuisée.
    fn next_frame(&mut self) -> Option<AccessUnit>;
    /// Dimensions de la vidéo produite, en pixels.
    fn dimensions(&self) -> (u32, u32);
}

/// Source de test rejouant un fichier H.264 Annex-B en boucle.
///
/// Sert à valider le transport sans dépendre de Windows : le flux est découpé
/// une fois au chargement, puis rejoué indéfiniment avec des horodatages
/// strictement croissants (un décodeur rejetterait un retour en arrière).
pub struct FileSource {
    units: Vec<AccessUnit>,
    width: u32,
    height: u32,
    tick_90k: u64,
    index: usize,
    loops: u64,
}

impl FileSource {
    pub fn from_annex_b(data: Vec<u8>, width: u32, height: u32, fps: u32) -> Result<Self> {
        if fps == 0 {
            bail!("le nombre d'images par seconde doit être supérieur à zéro");
        }
        let units = group_access_units(&data, fps);
        if units.is_empty() {
            bail!("flux invalide : aucune unité d'accès trouvée");
        }
        if !units.iter().any(|u| u.is_keyframe) {
            bail!("flux invalide : aucune image clé trouvée");
        }
        Ok(Self {
            tick_90k: CLOCK_RATE_HZ / fps as u64,
            index: 0,
            loops: 0,
            units,
            width,
            height,
        })
    }

    /// Charge un fichier `.264` depuis le disque.
    pub fn from_path(path: &std::path::Path, width: u32, height: u32, fps: u32) -> Result<Self> {
        let data = std::fs::read(path)?;
        Self::from_annex_b(data, width, height, fps)
    }
}

impl VideoSource for FileSource {
    fn next_frame(&mut self) -> Option<AccessUnit> {
        let total = self.units.len() as u64;
        let mut unit = self.units[self.index].clone();
        unit.pts_90k = (self.loops * total + self.index as u64) * self.tick_90k;

        self.index += 1;
        if self.index >= self.units.len() {
            self.index = 0;
            self.loops += 1;
        }
        Some(unit)
    }

    fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}
```

- [ ] **Step 8: Lancer les tests pour vérifier qu'ils passent**

Run: `cargo test -p agent`
Expected: 13 tests réussis (8 h264 + 5 source).

- [ ] **Step 9: Générer le flux H.264 de test**

Sur la machine Linux (installer `ffmpeg` s'il est absent) :

```bash
mkdir -p agent/testdata
ffmpeg -f lavfi -i testsrc=size=1280x720:rate=60 -t 5 \
    -c:v libx264 -profile:v baseline -level 3.1 -preset ultrafast \
    -tune zerolatency -g 60 -bf 0 -pix_fmt yuv420p \
    -f h264 agent/testdata/testsrc.264 -y
ls -lh agent/testdata/testsrc.264
```

Expected: un fichier de quelques mégaoctets. `-profile:v baseline -bf 0` garantit l'absence d'images B, donc un ordre de décodage identique à l'ordre d'affichage — indispensable puisque nos horodatages sont linéaires.

Retirer `*.264` du `.gitignore` pour ce fichier précis, ou l'ajouter en exception :

```
*.264
!agent/testdata/testsrc.264
```

- [ ] **Step 10: Vérifier que la source charge le vrai fichier**

Ajouter ce test à `agent/src/source.rs` :

```rust
    #[test]
    fn charge_le_flux_de_test_reel() {
        let path = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/testsrc.264"));
        let mut source = FileSource::from_path(path, 1280, 720, 60).expect("chargement du flux");
        assert_eq!(source.dimensions(), (1280, 720));

        let first = source.next_frame().unwrap();
        assert!(first.is_keyframe, "la première unité doit être une image clé");
        assert!(first.data.len() > 100, "une image clé réelle n'est pas minuscule");
    }
```

Run: `cargo test -p agent`
Expected: 14 tests réussis.

- [ ] **Step 11: Committer**

```bash
git add agent/src/h264.rs agent/src/source.rs agent/src/main.rs agent/testdata .gitignore
git commit -m "feat: découpage Annex-B et source vidéo de test rejouée en boucle"
```

---

### Task 7: Transport str0m et client de signaling

Cœur du transport. L'agent se connecte au signaling, attend l'offre du navigateur, y répond via `sdp_api()`, puis pousse les unités d'accès dans la boucle d'événements str0m. Le navigateur est l'offrant : il déclare la piste vidéo en réception seule et les deux data channels.

**Files:**
- Create: `agent/src/signaling.rs`, `agent/src/transport.rs`
- Modify: `agent/src/main.rs`, `agent/Cargo.toml`

**Interfaces:**
- Consumes: `VideoSource`, `AccessUnit` (tâche 6), `InputMessage`, `ClientControl`, `AgentControl` (tâches 2 et 4).
- Produces:
  - `async fn run_signaling(url: &str, session: &str) -> Result<SignalingHandle>` avec `SignalingHandle { offers: mpsc::Receiver<String>, answers: mpsc::Sender<String> }`
  - `struct Session` avec `Session::new(source: Box<dyn VideoSource + Send>, local_ip: IpAddr) -> Result<Session>`
  - `Session::accept_offer(&mut self, offer_sdp: &str) -> Result<String>`
  - `Session::send_next_frame(&mut self) -> Result<bool>`
  - `Session::send_control(&mut self, message: &AgentControl) -> Result<()>`
  - `Session::tick(&mut self, on_input: &mut impl FnMut(InputMessage), on_control: &mut impl FnMut(ClientControl)) -> Result<Tick>`
  - `enum Tick { Continue, Disconnected }`

- [ ] **Step 1: Ajouter les dépendances**

Dans `agent/Cargo.toml`, section `[dependencies]` :

```toml
str0m = "0.21"
tokio-tungstenite = "0.24"
futures-util = "0.3"
```

Run: `cargo build -p agent`
Expected: compilation réussie (les crates se téléchargent).

- [ ] **Step 2: Écrire le client de signaling**

`agent/src/signaling.rs` :

```rust
//! Client WebSocket vers le serveur de signaling.
//!
//! L'agent est toujours le répondant : il reçoit une offre SDP et renvoie une
//! réponse. Aucun trickle ICE — les candidats hôtes voyagent dans le SDP.

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

pub struct SignalingHandle {
    /// Offres SDP reçues du navigateur.
    pub offers: mpsc::Receiver<String>,
    /// Réponses SDP à renvoyer au navigateur.
    pub answers: mpsc::Sender<String>,
}

/// Se connecte au signaling et démarre la boucle d'échange en tâche de fond.
pub async fn run_signaling(url: &str, session: &str) -> Result<SignalingHandle> {
    let (stream, _) = tokio_tungstenite::connect_async(url)
        .await
        .with_context(|| format!("connexion au signaling {url}"))?;
    let (mut sink, mut source) = stream.split();

    let hello = serde_json::json!({ "role": "agent", "session": session });
    sink.send(Message::Text(hello.to_string())).await?;
    tracing::info!(session, "agent enregistré auprès du signaling");

    let (offer_tx, offers) = mpsc::channel::<String>(4);
    let (answers, mut answer_rx) = mpsc::channel::<String>(4);

    // Réception : offres et erreurs venant du signaling.
    tokio::spawn(async move {
        while let Some(message) = source.next().await {
            let text = match message {
                Ok(Message::Text(text)) => text,
                Ok(Message::Close(_)) | Err(_) => break,
                Ok(_) => continue,
            };
            let parsed: serde_json::Value = match serde_json::from_str(&text) {
                Ok(value) => value,
                Err(e) => {
                    tracing::warn!(erreur = %e, "message de signaling illisible");
                    continue;
                }
            };
            match parsed["type"].as_str() {
                Some("offer") => {
                    if let Some(sdp) = parsed["sdp"].as_str() {
                        if offer_tx.send(sdp.to_string()).await.is_err() {
                            break;
                        }
                    }
                }
                Some("peer-gone") => tracing::info!("le client s'est déconnecté"),
                Some("error") => {
                    tracing::error!(raison = %parsed["reason"], "erreur de signaling")
                }
                other => tracing::debug!(?other, "message de signaling ignoré"),
            }
        }
        tracing::info!("boucle de réception du signaling terminée");
    });

    // Émission : réponses SDP.
    tokio::spawn(async move {
        while let Some(sdp) = answer_rx.recv().await {
            let payload = serde_json::json!({ "type": "answer", "sdp": sdp });
            if sink.send(Message::Text(payload.to_string())).await.is_err() {
                break;
            }
        }
    });

    Ok(SignalingHandle { offers, answers })
}
```

- [ ] **Step 3: Écrire la boucle de transport str0m**

`agent/src/transport.rs` :

```rust
//! Boucle WebRTC : ICE, DTLS, SRTP et SCTP via str0m.
//!
//! str0m est une bibliothèque sans entrées-sorties : nous possédons le socket
//! UDP et la boucle d'événements. Règle impérative documentée par str0m : après
//! chaque mutation, drainer `poll_output` jusqu'à `Output::Timeout` avant la
//! mutation suivante.

use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::time::Instant;

use anyhow::{anyhow, bail, Context, Result};
use proto::control::{AgentControl, ClientControl};
use proto::input::InputMessage;
use str0m::channel::ChannelId;
use str0m::format::Codec;
use str0m::media::{MediaTime, Mid};
use str0m::net::{Protocol, Receive};
use str0m::{Candidate, Event, IceConnectionState, Input, Output, Rtc};

use crate::source::VideoSource;

/// Résultat d'un tour de boucle, pour piloter l'appelant.
pub enum Tick {
    Continue,
    Disconnected,
}

pub struct Session {
    rtc: Rtc,
    socket: UdpSocket,
    source: Box<dyn VideoSource + Send>,
    video_mid: Option<Mid>,
    control_channel: Option<ChannelId>,
    started: Instant,
}

impl Session {
    /// Prépare une session en attente d'offre.
    ///
    /// `local_ip` est l'adresse par laquelle le navigateur joindra l'agent.
    pub fn new(source: Box<dyn VideoSource + Send>, local_ip: IpAddr) -> Result<Self> {
        let socket = UdpSocket::bind(SocketAddr::new(local_ip, 0))
            .context("ouverture du socket UDP")?;
        let addr = socket.local_addr()?;
        tracing::info!(%addr, "socket UDP de l'agent");

        // str0m 0.21 exige un fournisseur cryptographique installé pour le processus.
        str0m::crypto::from_feature_flags().install_process_default();

        let mut rtc = Rtc::builder()
            .clear_codecs()
            .enable_h264(true)
            .set_stats_interval(Some(std::time::Duration::from_secs(1)))
            .build(Instant::now());

        rtc.add_local_candidate(
            Candidate::host(addr, "udp").map_err(|e| anyhow!("candidat hôte invalide : {e}"))?,
        )
        .map_err(|e| anyhow!("ajout du candidat hôte : {e}"))?;

        Ok(Self {
            rtc,
            socket,
            source,
            video_mid: None,
            control_channel: None,
            started: Instant::now(),
        })
    }

    /// Accepte l'offre du navigateur et produit la réponse SDP.
    pub fn accept_offer(&mut self, offer_sdp: &str) -> Result<String> {
        let offer = str0m::change::SdpOffer::from_sdp_string(offer_sdp)
            .map_err(|e| anyhow!("offre SDP illisible : {e}"))?;
        let answer = self
            .rtc
            .sdp_api()
            .accept_offer(offer)
            .map_err(|e| anyhow!("offre refusée : {e}"))?;
        Ok(answer.to_sdp_string())
    }

    /// Envoie l'unité d'accès suivante si la connexion est prête.
    ///
    /// `Ok(false)` signifie « rien envoyé, la piste n'est pas encore négociée ».
    pub fn send_next_frame(&mut self) -> Result<bool> {
        let Some(mid) = self.video_mid else {
            return Ok(false);
        };
        let Some(unit) = self.source.next_frame() else {
            bail!("source vidéo épuisée");
        };

        let Some(writer) = self.rtc.writer(mid) else {
            return Ok(false);
        };
        let Some(pt) = writer
            .payload_params()
            .find(|p| p.spec().codec == Codec::H264)
            .map(|p| p.pt())
        else {
            // La négociation n'a pas encore abouti sur un profil H.264 commun.
            return Ok(false);
        };

        writer
            .write(pt, Instant::now(), MediaTime::from_90khz(unit.pts_90k), unit.data)
            .map_err(|e| anyhow!("écriture de l'image : {e}"))?;
        Ok(true)
    }

    /// Envoie un message de contrôle au navigateur.
    pub fn send_control(&mut self, message: &AgentControl) -> Result<()> {
        let Some(id) = self.control_channel else {
            return Ok(());
        };
        let json = serde_json::to_string(message)?;
        if let Some(mut channel) = self.rtc.channel(id) {
            channel
                .write(false, json.as_bytes())
                .map_err(|e| anyhow!("écriture sur le canal de contrôle : {e}"))?;
        }
        Ok(())
    }

    /// Un tour de boucle : draine les sorties, puis attend un paquet ou l'échéance.
    pub fn tick(
        &mut self,
        on_input: &mut impl FnMut(InputMessage),
        on_control: &mut impl FnMut(ClientControl),
    ) -> Result<Tick> {
        // 1. Drainer toutes les sorties jusqu'à obtenir une échéance.
        let timeout = loop {
            match self.rtc.poll_output().map_err(|e| anyhow!("poll_output : {e}"))? {
                Output::Timeout(deadline) => break deadline,
                Output::Transmit(transmit) => {
                    self.socket.send_to(&transmit.contents, transmit.destination)?;
                }
                Output::Event(event) => {
                    if let Tick::Disconnected = self.handle_event(event, on_input, on_control) {
                        return Ok(Tick::Disconnected);
                    }
                }
            }
        };

        // 2. Attendre un paquet entrant jusqu'à l'échéance.
        let now = Instant::now();
        let wait = timeout.saturating_duration_since(now);
        if wait.is_zero() {
            self.rtc
                .handle_input(Input::Timeout(now))
                .map_err(|e| anyhow!("handle_input timeout : {e}"))?;
            return Ok(Tick::Continue);
        }

        self.socket.set_read_timeout(Some(wait))?;
        let mut buffer = vec![0u8; 2000];
        match self.socket.recv_from(&mut buffer) {
            Ok((n, source_addr)) => {
                buffer.truncate(n);
                let receive = Receive {
                    proto: Protocol::Udp,
                    source: source_addr,
                    destination: self.socket.local_addr()?,
                    contents: buffer
                        .as_slice()
                        .try_into()
                        .map_err(|e| anyhow!("paquet illisible : {e}"))?,
                };
                self.rtc
                    .handle_input(Input::Receive(Instant::now(), receive))
                    .map_err(|e| anyhow!("handle_input receive : {e}"))?;
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                self.rtc
                    .handle_input(Input::Timeout(Instant::now()))
                    .map_err(|e| anyhow!("handle_input timeout : {e}"))?;
            }
            Err(e) => return Err(e.into()),
        }
        Ok(Tick::Continue)
    }

    fn handle_event(
        &mut self,
        event: Event,
        on_input: &mut impl FnMut(InputMessage),
        on_control: &mut impl FnMut(ClientControl),
    ) -> Tick {
        match event {
            Event::IceConnectionStateChange(IceConnectionState::Disconnected) => {
                tracing::warn!("ICE déconnecté");
                return Tick::Disconnected;
            }
            Event::IceConnectionStateChange(state) => {
                tracing::info!(?state, écoulé = ?self.started.elapsed(), "état ICE");
            }
            Event::MediaAdded(media) => {
                tracing::info!(mid = ?media.mid, kind = ?media.kind, "piste négociée");
                if media.kind == str0m::media::MediaKind::Video {
                    self.video_mid = Some(media.mid);
                }
            }
            Event::ChannelOpen(id, label) => {
                tracing::info!(%label, "canal de données ouvert");
                if label == "control" {
                    self.control_channel = Some(id);
                }
            }
            Event::ChannelData(data) => {
                self.dispatch_channel_data(&data, on_input, on_control);
            }
            Event::KeyframeRequest(request) => {
                tracing::debug!(mid = ?request.mid, "image clé demandée");
            }
            _ => {}
        }
        Tick::Continue
    }

    fn dispatch_channel_data(
        &self,
        data: &str0m::channel::ChannelData,
        on_input: &mut impl FnMut(InputMessage),
        on_control: &mut impl FnMut(ClientControl),
    ) {
        if data.binary {
            match InputMessage::decode(&data.data) {
                Ok(message) => on_input(message),
                Err(e) => tracing::warn!(erreur = %e, "message d'entrée invalide"),
            }
        } else {
            match std::str::from_utf8(&data.data).map(serde_json::from_str::<ClientControl>) {
                Ok(Ok(message)) => on_control(message),
                Ok(Err(e)) => tracing::warn!(erreur = %e, "message de contrôle invalide"),
                Err(e) => tracing::warn!(erreur = %e, "contrôle non UTF-8"),
            }
        }
    }
}
```

- [ ] **Step 4: Vérifier la compilation et corriger les écarts d'API**

Run: `cargo build -p agent`

Ce fichier touche les points où l'API str0m a le plus évolué. Si la compilation échoue :

- `str0m::crypto::from_feature_flags` introuvable → consulter `cargo doc -p str0m --open`, section `crypto` ; la variante possible est `.set_crypto_provider(...)` sur le builder.
- `SdpOffer::from_sdp_string` / `to_sdp_string` → vérifier les noms exacts dans `str0m::change`.
- `writer.write(...)` consomme `writer` (signature `self`) : ne pas réutiliser la variable après.
- `Event::MediaAdded` porte une structure ; ajuster l'accès aux champs selon l'erreur du compilateur.

Expected: compilation réussie après ajustements. **Ne pas contourner un échec en supprimant une vérification** — l'objectif est un code qui compile ET respecte la sémantique décrite.

- [ ] **Step 5: Câbler le point d'entrée**

`agent/src/main.rs` :

```rust
mod h264;
mod signaling;
mod source;
mod transport;

use std::net::IpAddr;
use std::path::PathBuf;

use anyhow::{Context, Result};
use proto::control::AgentControl;

use crate::source::{FileSource, VideoSource};
use crate::transport::{Session, Tick};

/// Configuration de l'agent, lue depuis l'environnement.
struct Config {
    signaling_url: String,
    session_id: String,
    local_ip: IpAddr,
    test_file: Option<PathBuf>,
}

fn config() -> Result<Config> {
    Ok(Config {
        signaling_url: std::env::var("SIGNALING_URL")
            .unwrap_or_else(|_| "ws://127.0.0.1:8080".into()),
        session_id: std::env::var("SESSION_ID").unwrap_or_else(|_| "demo".into()),
        local_ip: std::env::var("LOCAL_IP")
            .unwrap_or_else(|_| "127.0.0.1".into())
            .parse()
            .context("LOCAL_IP n'est pas une adresse IP valide")?,
        test_file: std::env::var("TEST_FILE").ok().map(PathBuf::from),
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = config()?;
    let source: Box<dyn VideoSource + Send> = match &config.test_file {
        Some(path) => {
            tracing::info!(?path, "source de test");
            Box::new(FileSource::from_path(path, 1280, 720, 60)?)
        }
        None => anyhow::bail!("TEST_FILE non défini ; la capture Windows arrive à la tâche 9"),
    };

    let mut handle = signaling::run_signaling(&config.signaling_url, &config.session_id).await?;
    let mut session = Session::new(source, config.local_ip)?;

    let offer = handle
        .offers
        .recv()
        .await
        .context("le signaling s'est fermé avant l'offre")?;
    tracing::info!("offre reçue");
    let answer = session.accept_offer(&offer)?;
    handle.answers.send(answer).await?;
    tracing::info!("réponse envoyée");

    session.send_control(&AgentControl::ready(1280, 720)).ok();

    // Cadence d'envoi : une image toutes les 16,67 ms.
    let frame_interval = std::time::Duration::from_micros(16_667);
    let mut next_frame = std::time::Instant::now();
    let mut on_input = |message| tracing::debug!(?message, "entrée reçue");
    let mut on_control = |message| tracing::info!(?message, "contrôle reçu");

    loop {
        if let Tick::Disconnected = session.tick(&mut on_input, &mut on_control)? {
            tracing::info!("session terminée");
            break;
        }
        let now = std::time::Instant::now();
        if now >= next_frame {
            session.send_next_frame()?;
            next_frame = now + frame_interval;
        }
    }
    Ok(())
}
```

- [ ] **Step 6: Vérifier la compilation sur Linux et sur Windows**

```bash
cargo build -p agent
./scripts/build-agent.sh
```

Expected: les deux réussissent. Le code de cette tâche est portable — c'est ce qui permet de dérisquer le transport sans Windows.

- [ ] **Step 7: Committer**

```bash
git add agent/src/signaling.rs agent/src/transport.rs agent/src/main.rs agent/Cargo.toml Cargo.lock
git commit -m "feat: transport WebRTC str0m et client de signaling"
```

---

### Task 8: Client web et première vidéo de bout en bout

**Jalon de dérisquage.** À la fin de cette tâche, une mire vidéo s'affiche dans le navigateur, transmise en WebRTC depuis l'agent Rust — sans une ligne de code Windows. Si cette étape fonctionne, il ne reste plus qu'à remplacer la source vidéo. Si elle échoue, on l'apprend maintenant plutôt qu'après avoir écrit la capture et l'encodage.

**Files:**
- Create: `client/package.json`, `client/tsconfig.json`, `client/vite.config.ts`, `client/index.html`, `client/src/main.ts`, `client/src/webrtc.ts`, `client/src/style.css`

**Interfaces:**
- Consumes: le protocole de signaling (tâche 5), `parseAgentControl` (tâche 4).
- Produces:
  - `connectSession(options: SessionOptions): Promise<SessionHandle>` avec
    `SessionOptions { signalingUrl: string, sessionId: string, video: HTMLVideoElement }`
    et `SessionHandle { pc: RTCPeerConnection, inputChannel: RTCDataChannel, controlChannel: RTCDataChannel, close(): void }`

- [ ] **Step 1: Initialiser le projet Vite**

`client/package.json` :

```json
{
  "name": "@guacamole/client",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "preview": "vite preview"
  },
  "devDependencies": {
    "typescript": "^5.5.0",
    "vite": "^6.0.0"
  }
}
```

`client/tsconfig.json` :

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true,
    "noUnusedLocals": true,
    "noEmit": true,
    "lib": ["ES2022", "DOM", "DOM.Iterable"]
  },
  "include": ["src/**/*.ts", "../proto/ts/**/*.ts"]
}
```

`client/vite.config.ts` :

```typescript
import { defineConfig } from 'vite';

export default defineConfig({
    server: {
        host: '0.0.0.0',
        port: 5173,
    },
});
```

```bash
cd client && npm install && cd ..
```

- [ ] **Step 2: Écrire la page**

`client/index.html` :

```html
<!doctype html>
<html lang="fr">
    <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <title>Session distante</title>
        <link rel="stylesheet" href="/src/style.css" />
    </head>
    <body>
        <video id="remote" autoplay playsinline muted></video>
        <div id="status" role="status">Connexion…</div>
        <script type="module" src="/src/main.ts"></script>
    </body>
</html>
```

`client/src/style.css` :

```css
:root {
    color-scheme: dark;
    --surface: #0b0d10;
    --text: #e6e8eb;
}

* {
    box-sizing: border-box;
}

html,
body {
    margin: 0;
    height: 100%;
    overflow: hidden;
    background: var(--surface);
    color: var(--text);
    font: 14px/1.5 system-ui, -apple-system, "Segoe UI", sans-serif;
}

#remote {
    display: block;
    width: 100vw;
    height: 100vh;
    object-fit: contain;
    background: #000;
}

#status {
    position: fixed;
    inset-block-start: 12px;
    inset-inline-start: 12px;
    padding: 6px 12px;
    border-radius: 6px;
    background: rgb(0 0 0 / 0.72);
    font-variant-numeric: tabular-nums;
    transition: opacity 0.3s;
}

#status[data-hidden="true"] {
    opacity: 0;
    pointer-events: none;
}
```

- [ ] **Step 3: Écrire la couche WebRTC**

`client/src/webrtc.ts` :

```typescript
// Établissement de la session WebRTC. Le navigateur est l'offrant : il déclare
// la piste vidéo en réception seule et les deux canaux de données, puis attend
// la réponse de l'agent relayée par le signaling.

import { parseAgentControl, type AgentControl } from '../../proto/ts/control';

export interface SessionOptions {
    signalingUrl: string;
    sessionId: string;
    video: HTMLVideoElement;
    onControl?: (message: AgentControl) => void;
    onStatus?: (message: string) => void;
}

export interface SessionHandle {
    pc: RTCPeerConnection;
    inputChannel: RTCDataChannel;
    controlChannel: RTCDataChannel;
    close(): void;
}

/// Attend que la collecte ICE soit terminée : sans trickle, le SDP doit déjà
/// contenir tous les candidats.
function waitForIceGathering(pc: RTCPeerConnection): Promise<void> {
    if (pc.iceGatheringState === 'complete') return Promise.resolve();
    return new Promise((resolve) => {
        const check = () => {
            if (pc.iceGatheringState === 'complete') {
                pc.removeEventListener('icegatheringstatechange', check);
                resolve();
            }
        };
        pc.addEventListener('icegatheringstatechange', check);
        // Filet de sécurité : ne jamais bloquer indéfiniment sur un réseau lent.
        setTimeout(() => {
            pc.removeEventListener('icegatheringstatechange', check);
            resolve();
        }, 3000);
    });
}

export async function connectSession(options: SessionOptions): Promise<SessionHandle> {
    const status = options.onStatus ?? (() => {});
    // Réseau local : aucun serveur STUN/TURN nécessaire au jalon 1.
    const pc = new RTCPeerConnection({ iceServers: [] });

    pc.addTransceiver('video', { direction: 'recvonly' });

    // Entrées : non fiable et non ordonné — une position de souris périmée n'a
    // aucune valeur, mieux vaut la perdre que retarder les suivantes.
    const inputChannel = pc.createDataChannel('input', {
        ordered: false,
        maxRetransmits: 0,
    });
    const controlChannel = pc.createDataChannel('control', { ordered: true });

    controlChannel.addEventListener('message', (event) => {
        try {
            options.onControl?.(parseAgentControl(String(event.data)));
        } catch (error) {
            console.warn('message de contrôle invalide', error);
        }
    });

    pc.addEventListener('track', (event) => {
        options.video.srcObject = event.streams[0] ?? new MediaStream([event.track]);
        status('flux reçu');
    });

    pc.addEventListener('connectionstatechange', () => {
        status(`connexion : ${pc.connectionState}`);
    });

    const socket = new WebSocket(options.signalingUrl);
    await new Promise<void>((resolve, reject) => {
        socket.addEventListener('open', () => resolve(), { once: true });
        socket.addEventListener('error', () => reject(new Error('signaling injoignable')), {
            once: true,
        });
    });
    socket.send(JSON.stringify({ role: 'client', session: options.sessionId }));

    const answerReceived = new Promise<string>((resolve, reject) => {
        socket.addEventListener('message', (event) => {
            const message = JSON.parse(String(event.data));
            if (message.type === 'answer') resolve(message.sdp);
            else if (message.type === 'error') reject(new Error(message.reason));
            else if (message.type === 'peer-gone') reject(new Error('agent déconnecté'));
        });
    });

    const offer = await pc.createOffer();
    await pc.setLocalDescription(offer);
    await waitForIceGathering(pc);

    status('offre envoyée, attente de l\'agent…');
    socket.send(JSON.stringify({ type: 'offer', sdp: pc.localDescription!.sdp }));

    const answerSdp = await answerReceived;
    await pc.setRemoteDescription({ type: 'answer', sdp: answerSdp });
    status('réponse reçue');

    return {
        pc,
        inputChannel,
        controlChannel,
        close() {
            socket.close();
            pc.close();
        },
    };
}
```

- [ ] **Step 4: Écrire l'orchestration**

`client/src/main.ts` :

```typescript
import { connectSession } from './webrtc';

const video = document.querySelector<HTMLVideoElement>('#remote')!;
const statusElement = document.querySelector<HTMLDivElement>('#status')!;

function setStatus(message: string): void {
    statusElement.textContent = message;
    statusElement.dataset.hidden = 'false';
}

// La session et le signaling sont paramétrables par l'URL pour faciliter les
// essais : ?session=demo&signaling=ws://192.168.3.2:8080
const params = new URLSearchParams(window.location.search);
const sessionId = params.get('session') ?? 'demo';
const signalingUrl =
    params.get('signaling') ?? `ws://${window.location.hostname}:8080`;

connectSession({
    signalingUrl,
    sessionId,
    video,
    onStatus: setStatus,
    onControl(message) {
        if (message.type === 'ready') {
            setStatus(`prêt — ${message.width}×${message.height}`);
            setTimeout(() => {
                statusElement.dataset.hidden = 'true';
            }, 1500);
        } else if (message.type === 'session-end') {
            setStatus(`session terminée : ${message.reason}`);
        }
    },
}).catch((error: unknown) => {
    setStatus(`échec : ${error instanceof Error ? error.message : String(error)}`);
});
```

- [ ] **Step 5: Vérifier la compilation TypeScript**

Run: `cd client && npx tsc --noEmit`
Expected: aucune erreur.

- [ ] **Step 6: Lancer le test de bout en bout**

Trois terminaux sur la machine Linux :

```bash
# Terminal 1 — signaling
cd signaling && npx tsx src/index.ts

# Terminal 2 — agent avec la source de test
SIGNALING_URL=ws://127.0.0.1:8080 \
SESSION_ID=demo \
LOCAL_IP=127.0.0.1 \
TEST_FILE=agent/testdata/testsrc.264 \
RUST_LOG=info cargo run -p agent

# Terminal 3 — client web
cd client && npx vite
```

Ouvrir `http://localhost:5173/?session=demo` dans Chrome.

Expected: **la mire de test s'affiche et s'anime.** Le bandeau passe par « offre envoyée », « réponse reçue », « flux reçu », puis « prêt — 1280×720 » avant de s'effacer.

- [ ] **Step 7: Diagnostiquer si la vidéo ne s'affiche pas**

Ouvrir `chrome://webrtc-internals` et suivre cet ordre :

1. **`connectionState` reste `connecting`** → problème ICE. Vérifier que `LOCAL_IP` est l'adresse réellement joignable par le navigateur (`127.0.0.1` si tout est local) et que les journaux de l'agent affichent `socket UDP de l'agent`.
2. **ICE connecté mais `framesDecoded` reste à 0** → problème de format. C'est le point d'incertitude identifié : le paquetiseur H.264 de str0m attend peut-être les NAL sans start codes plutôt qu'en Annex-B. Consulter `cargo doc -p str0m` (module `packet`) ou le code source du paquetiseur H.264, puis, si nécessaire, modifier `flush()` dans `agent/src/h264.rs` pour émettre les NAL sans les préfixes `[0,0,0,1]`. Relancer.
3. **`framesDecoded` augmente mais l'image est noire** → le navigateur n'a pas reçu SPS/PPS. Vérifier que la première unité d'accès les contient (le test `charge_le_flux_de_test_reel` garantit une image clé, mais pas la présence des paramètres) ; au besoin régénérer le fichier avec `-x264-params repeat-headers=1`.
4. **`Erreur : offre refusée`** dans les journaux de l'agent → aucun profil H.264 commun. Consigner le SDP de l'offre et les `payload_params` de str0m, puis ajuster la configuration des codecs.

Consigner la cause et la correction dans le journal de la tâche : c'est l'information la plus précieuse de tout le jalon.

- [ ] **Step 8: Committer**

```bash
git add client
git commit -m "feat: client web WebRTC et première vidéo de bout en bout"
```

---

### Task 9: Repérage de fenêtre et capture Windows.Graphics.Capture

Premier code spécifique à Windows. Il ne compile et ne s'exécute que sur la VM — la boucle de développement passe désormais par `./scripts/build-agent.sh`. Les fonctions purement calculatoires (le mapping de coordonnées) restent testables sur Linux et sont testées à ce titre.

**Files:**
- Create: `agent/src/window.rs`, `agent/src/capture.rs`
- Modify: `agent/Cargo.toml`, `agent/src/main.rs`

**Interfaces:**
- Consumes: rien.
- Produces:
  - `fn find_window_by_title(fragment: &str) -> Result<HWND>`
  - `fn client_size(hwnd: HWND) -> Result<(u32, u32)>` — dimensions de la zone client, en pixels
  - `fn resize_window(hwnd: HWND, width: u32, height: u32) -> Result<()>`
  - `fn is_window_alive(hwnd: HWND) -> bool`
  - `struct WindowCapture` avec `WindowCapture::new(hwnd: HWND) -> Result<WindowCapture>`, `fn next_texture(&mut self) -> Result<Option<CapturedFrame>>`, `fn device(&self) -> &ID3D11Device`
  - `struct CapturedFrame { pub texture: ID3D11Texture2D, pub width: u32, pub height: u32 }`

- [ ] **Step 1: Ajouter les dépendances Windows**

Dans `agent/Cargo.toml` :

```toml
[target.'cfg(windows)'.dependencies]
windows = { version = "0.62", features = [
    "Foundation",
    "Graphics_Capture",
    "Graphics_DirectX_Direct3D11",
    "Win32_Foundation",
    "Win32_Graphics_Direct3D",
    "Win32_Graphics_Direct3D11",
    "Win32_Graphics_Dxgi",
    "Win32_Graphics_Dxgi_Common",
    "Win32_Graphics_Gdi",
    "Win32_Media_MediaFoundation",
    "Win32_System_Com",
    "Win32_System_Threading",
    "Win32_System_WinRT_Direct3D11",
    "Win32_System_WinRT_Graphics_Capture",
    "Win32_UI_HiDpi",
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_UI_WindowsAndMessaging",
] }
windows-future = "0.3"
```

Note : `Win32_Media_MediaFoundation` et `Win32_UI_Input_KeyboardAndMouse` sont déduits de la convention de nommage de windows-rs (namespace → feature). Si `cargo build` signale un symbole manquant, chercher le bon nom de feature avec :
`node scripts/winrm.js "Set-Location C:\\dev; cargo build 2>&1 | Select-String 'feature'"`

- [ ] **Step 2: Vérifier que la nouvelle dépendance compile sur Windows**

Run: `./scripts/build-agent.sh`
Expected: compilation réussie. Le premier build de `windows` est long (plusieurs minutes) — c'est normal.

- [ ] **Step 3: Écrire le module fenêtre**

`agent/src/window.rs` :

```rust
//! Repérage et pilotage de la fenêtre à capturer.

#![cfg(windows)]

use anyhow::{anyhow, bail, Result};
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT, TRUE};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClientRect, GetWindowTextLengthW, GetWindowTextW, IsWindow, IsWindowVisible,
    SetWindowPos, SWP_NOMOVE, SWP_NOZORDER,
};

struct SearchContext {
    fragment: String,
    found: Option<HWND>,
}

/// Cherche la première fenêtre visible dont le titre contient `fragment`.
///
/// La comparaison est insensible à la casse : les titres de navigateurs
/// changent au gré de la page affichée, on ne peut pas exiger un titre exact.
pub fn find_window_by_title(fragment: &str) -> Result<HWND> {
    let mut context = SearchContext {
        fragment: fragment.to_lowercase(),
        found: None,
    };

    unsafe {
        // EnumWindows renvoie une erreur si le rappel interrompt l'énumération,
        // ce qui est précisément ce que nous faisons en cas de succès.
        let _ = EnumWindows(
            Some(enum_callback),
            LPARAM(&mut context as *mut SearchContext as isize),
        );
    }

    context
        .found
        .ok_or_else(|| anyhow!("aucune fenêtre visible dont le titre contient « {fragment} »"))
}

unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let context = &mut *(lparam.0 as *mut SearchContext);

    if !IsWindowVisible(hwnd).as_bool() {
        return TRUE;
    }
    let length = GetWindowTextLengthW(hwnd);
    if length <= 0 {
        return TRUE;
    }

    let mut buffer = vec![0u16; length as usize + 1];
    let written = GetWindowTextW(hwnd, &mut buffer);
    if written <= 0 {
        return TRUE;
    }
    let title = String::from_utf16_lossy(&buffer[..written as usize]).to_lowercase();

    if title.contains(&context.fragment) {
        context.found = Some(hwnd);
        return BOOL(0); // interrompt l'énumération
    }
    TRUE
}

/// Dimensions de la zone client de la fenêtre, en pixels.
pub fn client_size(hwnd: HWND) -> Result<(u32, u32)> {
    let mut rect = RECT::default();
    unsafe { GetClientRect(hwnd, &mut rect)? };
    let width = (rect.right - rect.left).max(0) as u32;
    let height = (rect.bottom - rect.top).max(0) as u32;
    if width == 0 || height == 0 {
        bail!("la fenêtre a une zone client vide");
    }
    Ok((width, height))
}

/// Redimensionne la fenêtre sans la déplacer ni changer son ordre d'affichage.
pub fn resize_window(hwnd: HWND, width: u32, height: u32) -> Result<()> {
    // Les dimensions nulles font échouer la capture ; on impose un plancher.
    let width = width.max(160) as i32;
    let height = height.max(120) as i32;
    unsafe { SetWindowPos(hwnd, None, 0, 0, width, height, SWP_NOMOVE | SWP_NOZORDER)? };
    Ok(())
}

/// Vrai tant que la fenêtre existe.
pub fn is_window_alive(hwnd: HWND) -> bool {
    unsafe { IsWindow(Some(hwnd)).as_bool() }
}
```

- [ ] **Step 4: Écrire le module de capture**

`agent/src/capture.rs` :

```rust
//! Capture d'une fenêtre via Windows.Graphics.Capture.
//!
//! Les images restent sur le GPU : `next_texture` renvoie une `ID3D11Texture2D`
//! que l'encodeur consomme directement, sans aller-retour en mémoire centrale.

#![cfg(windows)]

use std::sync::mpsc::{channel, Receiver, TryRecvError};

use anyhow::{anyhow, Context, Result};
use windows::core::Interface;
use windows::Foundation::TypedEventHandler;
use windows::Graphics::Capture::{
    Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession,
};
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_0};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION,
};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;

/// Une image capturée, encore résidente sur le GPU.
pub struct CapturedFrame {
    pub texture: ID3D11Texture2D,
    pub width: u32,
    pub height: u32,
}

pub struct WindowCapture {
    device: ID3D11Device,
    _context: ID3D11DeviceContext,
    _session: GraphicsCaptureSession,
    frame_pool: Direct3D11CaptureFramePool,
    frames: Receiver<()>,
}

impl WindowCapture {
    pub fn new(hwnd: HWND) -> Result<Self> {
        let (device, context) = create_d3d_device()?;
        let direct3d_device = wrap_device_for_winrt(&device)?;

        // Interop WinRT : obtenir un GraphicsCaptureItem pour une HWND Win32.
        let interop =
            windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
                .context("fabrique IGraphicsCaptureItemInterop")?;
        let item: GraphicsCaptureItem =
            unsafe { interop.CreateForWindow(hwnd) }.context("capture de la fenêtre")?;

        let size = item.Size()?;
        // CreateFreeThreaded évite d'avoir à faire tourner une pompe de messages.
        let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &direct3d_device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            2, // deux tampons : capture et encodage se recouvrent
            size,
        )
        .context("création du pool d'images")?;

        // Le rappel signale seulement l'arrivée d'une image ; la texture est
        // récupérée dans next_texture, sur le fil de la boucle principale.
        let (notify, frames) = channel::<()>();
        frame_pool.FrameArrived(&TypedEventHandler::new(
            move |_pool: &Option<Direct3D11CaptureFramePool>, _| {
                let _ = notify.send(());
                Ok(())
            },
        ))?;

        let session = frame_pool
            .CreateCaptureSession(&item)
            .context("création de la session de capture")?;
        // Supprime la bordure jaune de capture sur Windows 11 ; échoue en silence
        // sur les versions antérieures, ce qui est acceptable.
        let _ = session.SetIsBorderRequired(false);
        session.StartCapture().context("démarrage de la capture")?;

        Ok(Self {
            device,
            _context: context,
            _session: session,
            frame_pool,
            frames,
        })
    }

    /// Récupère l'image la plus récente, ou `None` si aucune n'est disponible.
    ///
    /// Les images en retard sont volontairement écartées : en streaming, une
    /// image périmée n'a aucune valeur face à celle qui la suit.
    pub fn next_texture(&mut self) -> Result<Option<CapturedFrame>> {
        let mut available = false;
        loop {
            match self.frames.try_recv() {
                Ok(()) => available = true,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    return Err(anyhow!("la capture s'est arrêtée"))
                }
            }
        }
        if !available {
            return Ok(None);
        }

        let mut latest = None;
        while let Ok(frame) = self.frame_pool.TryGetNextFrame() {
            let surface = frame.Surface()?;
            let access: IDirect3DDxgiInterfaceAccess = surface.cast()?;
            let texture: ID3D11Texture2D = unsafe { access.GetInterface() }?;
            let size = frame.ContentSize()?;
            latest = Some(CapturedFrame {
                texture,
                width: size.Width.max(0) as u32,
                height: size.Height.max(0) as u32,
            });
        }
        Ok(latest)
    }

    pub fn device(&self) -> &ID3D11Device {
        &self.device
    }
}

fn create_d3d_device() -> Result<(ID3D11Device, ID3D11DeviceContext)> {
    let mut device: Option<ID3D11Device> = None;
    let mut context: Option<ID3D11DeviceContext> = None;
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            None,
            // BGRA_SUPPORT est obligatoire pour l'interopérabilité WinRT.
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&[D3D_FEATURE_LEVEL_11_0]),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
        .context("création du périphérique D3D11")?;
    }
    Ok((
        device.ok_or_else(|| anyhow!("périphérique D3D11 absent"))?,
        context.ok_or_else(|| anyhow!("contexte D3D11 absent"))?,
    ))
}

fn wrap_device_for_winrt(
    device: &ID3D11Device,
) -> Result<windows::Graphics::DirectX::Direct3D11::IDirect3DDevice> {
    let dxgi: IDXGIDevice = device.cast()?;
    let inspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi) }
        .context("conversion du périphérique pour WinRT")?;
    Ok(inspectable.cast()?)
}
```

- [ ] **Step 5: Déclarer les modules sous condition Windows**

Dans `agent/src/main.rs`, remplacer la liste des modules par :

```rust
mod h264;
mod signaling;
mod source;
mod transport;

#[cfg(windows)]
mod capture;
#[cfg(windows)]
mod window;
```

- [ ] **Step 6: Compiler sur Windows**

Run: `./scripts/build-agent.sh`
Expected: compilation réussie. Corriger les écarts d'API signalés par le compilateur — notamment le type exact du second paramètre de `SetWindowPos` et la forme de `EnumWindows` en 0.62.

- [ ] **Step 7: Écrire un essai manuel de capture**

Ajouter à `agent/src/main.rs` un mode de diagnostic, avant la logique de session :

```rust
    // Mode diagnostic : CAPTURE_TEST=firefox vérifie le repérage et la capture.
    #[cfg(windows)]
    if let Ok(fragment) = std::env::var("CAPTURE_TEST") {
        let hwnd = window::find_window_by_title(&fragment)?;
        let (w, h) = window::client_size(hwnd)?;
        tracing::info!(largeur = w, hauteur = h, "fenêtre trouvée");

        let mut capture = capture::WindowCapture::new(hwnd)?;
        let mut captured = 0;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while std::time::Instant::now() < deadline {
            if let Some(frame) = capture.next_texture()? {
                captured += 1;
                if captured == 1 {
                    tracing::info!(frame.width, frame.height, "première image capturée");
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        tracing::info!(captured, "images capturées en 3 s");
        return Ok(());
    }
```

- [ ] **Step 8: Exécuter l'essai sur la VM**

Lancer Firefox sur la VM (par RDP ou via WinRM), puis :

```bash
./scripts/build-agent.sh
node scripts/winrm.js "\$env:CAPTURE_TEST='firefox'; \$env:RUST_LOG='info'; C:\\dev\\target\\debug\\agent.exe 2>&1 | Out-String"
```

Expected: journal indiquant « fenêtre trouvée », « première image capturée » avec des dimensions plausibles, et un décompte d'images cohérent (plusieurs dizaines en 3 s). Un décompte de 0 signifie que `FrameArrived` ne se déclenche pas — vérifier que la fenêtre n'est pas minimisée.

- [ ] **Step 9: Committer**

```bash
git add agent/src/window.rs agent/src/capture.rs agent/src/main.rs agent/Cargo.toml Cargo.lock
git commit -m "feat: repérage de fenêtre et capture Windows.Graphics.Capture"
```

---

### Task 10: Encodeur H.264 matériel (MFT asynchrone)

Point le plus délicat du jalon. Les MFT matérielles sont **asynchrones** : on ne peut pas boucler sur `ProcessInput`/`ProcessOutput`, il faut suivre les événements `METransformNeedInput` et `METransformHaveOutput` émis par `IMFMediaEventGenerator`. Cette contrainte est structurante et non contournable.

**Files:**
- Create: `agent/src/encode.rs`
- Modify: `agent/src/main.rs`

**Interfaces:**
- Consumes: `CapturedFrame` (tâche 9), `AccessUnit` (tâche 6).
- Produces:
  - `struct H264Encoder` avec `H264Encoder::new(device: &ID3D11Device, width: u32, height: u32, fps: u32, bitrate: u32) -> Result<H264Encoder>`
  - `fn submit(&mut self, frame: &CapturedFrame, pts_90k: u64) -> Result<()>`
  - `fn poll_output(&mut self) -> Result<Option<AccessUnit>>`
  - `fn request_keyframe(&mut self) -> Result<()>`

- [ ] **Step 1: Vérifier la présence d'un encodeur matériel sur la VM**

```bash
node scripts/winrm.js "Get-CimInstance Win32_VideoController | Select-Object Name, DriverVersion | Format-List"
```

Consigner le modèle de GPU : il détermine le nom de la MFT attendue (NVIDIA `NVIDIA H.264 Encoder MFT`, Intel `Intel® Quick Sync Video H.264 Encoder MFT`, AMD `AMD H.264 Hardware MFT Encoder`).

- [ ] **Step 2: Écrire l'énumération et la configuration de l'encodeur**

`agent/src/encode.rs` :

```rust
//! Encodeur H.264 matériel via Media Foundation.
//!
//! Les MFT matérielles sont asynchrones : le pilotage se fait par événements
//! (`METransformNeedInput` / `METransformHaveOutput`) et non par une boucle
//! `ProcessInput`/`ProcessOutput` synchrone. Cette contrainte est imposée par
//! Media Foundation, pas par un choix de conception.

#![cfg(windows)]

use anyhow::{anyhow, bail, Context, Result};
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D11::ID3D11Device;
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::CoTaskMemFree;

use crate::capture::CapturedFrame;
use crate::h264::{group_access_units, AccessUnit};

/// Identifiants d'événements des MFT asynchrones.
const ME_TRANSFORM_NEED_INPUT: u32 = 601;
const ME_TRANSFORM_HAVE_OUTPUT: u32 = 602;

pub struct H264Encoder {
    transform: IMFTransform,
    events: IMFMediaEventGenerator,
    device_manager: IMFDXGIDeviceManager,
    width: u32,
    height: u32,
    fps: u32,
    /// Nombre de demandes d'entrée non encore satisfaites.
    pending_input_requests: u32,
    /// Nombre d'images prêtes à être récupérées.
    pending_outputs: u32,
}

impl H264Encoder {
    pub fn new(
        device: &ID3D11Device,
        width: u32,
        height: u32,
        fps: u32,
        bitrate: u32,
    ) -> Result<Self> {
        unsafe {
            MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET).context("démarrage de Media Foundation")?;
        }

        let transform = find_hardware_encoder()?;
        let attributes = unsafe { transform.GetAttributes() }?;

        // Débloquer le mode asynchrone : obligatoire pour toute MFT matérielle.
        let is_async = unsafe { attributes.GetUINT32(&MF_TRANSFORM_ASYNC) }.unwrap_or(0);
        if is_async == 0 {
            bail!("l'encodeur trouvé n'est pas asynchrone : configuration inattendue");
        }
        unsafe { attributes.SetUINT32(&MF_TRANSFORM_ASYNC_UNLOCK, 1) }?;
        // Mode faible latence : pas de mise en tampon multi-images.
        unsafe { attributes.SetUINT32(&MF_LOW_LATENCY, 1) }?;

        // Partager le périphérique D3D11 pour recevoir des textures GPU.
        let device_manager = share_device(device)?;
        unsafe {
            transform.ProcessMessage(
                MFT_MESSAGE_SET_D3D_MANAGER,
                device_manager.as_raw() as usize,
            )
        }
        .context("partage du périphérique D3D avec l'encodeur")?;

        configure_output(&transform, width, height, fps, bitrate)?;
        configure_input(&transform, width, height, fps)?;
        configure_rate_control(&transform, bitrate)?;

        let events: IMFMediaEventGenerator = transform.cast()?;

        unsafe {
            transform.ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0)?;
            transform.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)?;
            transform.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0)?;
        }

        Ok(Self {
            transform,
            events,
            device_manager,
            width,
            height,
            fps,
            pending_input_requests: 0,
            pending_outputs: 0,
        })
    }

    /// Draine les événements disponibles sans bloquer.
    fn drain_events(&mut self) -> Result<()> {
        loop {
            // MF_EVENT_FLAG_NO_WAIT : renvoie immédiatement s'il n'y a rien.
            let event = match unsafe { self.events.GetEvent(MF_EVENT_FLAG_NO_WAIT) } {
                Ok(event) => event,
                Err(_) => break, // file vide
            };
            let kind = unsafe { event.GetType() }?;
            match kind {
                ME_TRANSFORM_NEED_INPUT => self.pending_input_requests += 1,
                ME_TRANSFORM_HAVE_OUTPUT => self.pending_outputs += 1,
                _ => {}
            }
        }
        Ok(())
    }

    /// Soumet une image capturée si l'encodeur en réclame une.
    ///
    /// Si aucune demande n'est en attente, l'image est ignorée : l'encodeur est
    /// saturé et une image de plus ne ferait qu'ajouter de la latence.
    pub fn submit(&mut self, frame: &CapturedFrame, pts_90k: u64) -> Result<()> {
        self.drain_events()?;
        if self.pending_input_requests == 0 {
            return Ok(());
        }

        let sample = unsafe { MFCreateSample() }?;
        let buffer = unsafe {
            MFCreateDXGISurfaceBuffer(&ID3D11Texture2D::IID, &frame.texture, 0, false)
        }
        .context("enveloppement de la texture dans un tampon Media Foundation")?;
        unsafe {
            sample.AddBuffer(&buffer)?;
            // Media Foundation compte en unités de 100 ns ; nos horodatages sont
            // en 1/90000 s. 90000 Hz → 10 000 000 Hz : facteur 1000/9.
            sample.SetSampleTime((pts_90k as i64) * 1000 / 9)?;
            sample.SetSampleDuration(10_000_000 / self.fps.max(1) as i64)?;
            self.transform.ProcessInput(0, &sample, 0)?;
        }
        self.pending_input_requests -= 1;
        Ok(())
    }

    /// Récupère une unité d'accès encodée si elle est disponible.
    pub fn poll_output(&mut self) -> Result<Option<AccessUnit>> {
        self.drain_events()?;
        if self.pending_outputs == 0 {
            return Ok(None);
        }
        self.pending_outputs -= 1;

        // Les MFT matérielles allouent elles-mêmes leurs échantillons de sortie.
        let mut buffers = [MFT_OUTPUT_DATA_BUFFER {
            dwStreamID: 0,
            pSample: std::mem::ManuallyDrop::new(None),
            dwStatus: 0,
            pEvents: std::mem::ManuallyDrop::new(None),
        }];
        let mut status = 0u32;

        unsafe { self.transform.ProcessOutput(0, &mut buffers, &mut status) }
            .context("récupération de l'image encodée")?;

        let sample = buffers[0]
            .pSample
            .as_ref()
            .cloned()
            .ok_or_else(|| anyhow!("échantillon de sortie absent"))?;

        let media_buffer = unsafe { sample.ConvertToContiguousBuffer() }?;
        let mut data_ptr: *mut u8 = std::ptr::null_mut();
        let mut length = 0u32;
        unsafe { media_buffer.Lock(&mut data_ptr, None, Some(&mut length))? };
        let bytes = unsafe { std::slice::from_raw_parts(data_ptr, length as usize) }.to_vec();
        unsafe { media_buffer.Unlock()? };

        // La sortie est en Annex-B ; on la repasse par le regroupement pour
        // obtenir l'indicateur d'image clé de façon cohérente avec le reste.
        let mut units = group_access_units(&bytes, self.fps.max(1));
        if units.is_empty() {
            return Ok(None);
        }
        let mut unit = units.remove(0);
        // L'horodatage vient de l'échantillon, pas de la position dans le flux.
        let sample_time = unsafe { sample.GetSampleTime() }.unwrap_or(0);
        unit.pts_90k = (sample_time.max(0) as u64) * 9 / 1000;
        Ok(Some(unit))
    }

    /// Force la production d'une image clé sur l'image suivante.
    pub fn request_keyframe(&mut self) -> Result<()> {
        let codec: ICodecAPI = self.transform.cast()?;
        let value = windows::Win32::System::Variant::VARIANT::from(true);
        unsafe { codec.SetValue(&CODECAPI_AVEncVideoForceKeyFrame, &value) }?;
        Ok(())
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

impl Drop for H264Encoder {
    fn drop(&mut self) {
        unsafe {
            let _ = self.transform.ProcessMessage(MFT_MESSAGE_NOTIFY_END_OF_STREAM, 0);
            let _ = self.transform.ProcessMessage(MFT_MESSAGE_NOTIFY_END_STREAMING, 0);
            let _ = MFShutdown();
        }
        let _ = &self.device_manager;
    }
}

/// Énumère les encodeurs H.264 matériels et active le premier.
fn find_hardware_encoder() -> Result<IMFTransform> {
    let input_info = MFT_REGISTER_TYPE_INFO {
        guidMajorType: MFMediaType_Video,
        guidSubtype: MFVideoFormat_NV12,
    };
    let output_info = MFT_REGISTER_TYPE_INFO {
        guidMajorType: MFMediaType_Video,
        guidSubtype: MFVideoFormat_H264,
    };

    let mut activates: *mut Option<IMFActivate> = std::ptr::null_mut();
    let mut count: u32 = 0;

    unsafe {
        MFTEnumEx(
            MFT_CATEGORY_VIDEO_ENCODER,
            MFT_ENUM_FLAG_HARDWARE | MFT_ENUM_FLAG_SORTANDFILTER,
            Some(&input_info),
            Some(&output_info),
            &mut activates,
            &mut count,
        )
        .context("énumération des encodeurs H.264 matériels")?;
    }

    if count == 0 {
        bail!(
            "aucun encodeur H.264 matériel trouvé sur cette machine. \
             Vérifier le pilote GPU ; le jalon 1 n'a pas de repli logiciel."
        );
    }

    // Récupérer les objets AVANT de libérer le tableau alloué par CoTaskMemAlloc.
    let slice = unsafe { std::slice::from_raw_parts(activates, count as usize) };
    let first = slice
        .first()
        .and_then(|a| a.clone())
        .ok_or_else(|| anyhow!("activateur d'encodeur absent"))?;

    if let Ok(name) = unsafe { first.GetStringAlloc(&MFT_FRIENDLY_NAME_Attribute) } {
        tracing::info!(encodeur = %unsafe { name.to_string() }.unwrap_or_default(), "encodeur retenu");
    }
    let transform: IMFTransform = unsafe { first.ActivateObject() }?;
    unsafe { CoTaskMemFree(Some(activates as *const _)) };
    Ok(transform)
}

fn configure_output(
    transform: &IMFTransform,
    width: u32,
    height: u32,
    fps: u32,
    bitrate: u32,
) -> Result<()> {
    let media_type = unsafe { MFCreateMediaType() }?;
    unsafe {
        media_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        media_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)?;
        media_type.SetUINT32(&MF_MT_AVG_BITRATE, bitrate)?;
        media_type.SetUINT64(&MF_MT_FRAME_SIZE, pack_u64(width, height))?;
        media_type.SetUINT64(&MF_MT_FRAME_RATE, pack_u64(fps, 1))?;
        media_type.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack_u64(1, 1))?;
        media_type.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        // Baseline évite les images B : ordre de décodage = ordre d'affichage.
        media_type.SetUINT32(&MF_MT_MPEG2_PROFILE, eAVEncH264VProfile_Base.0 as u32)?;
        transform.SetOutputType(0, &media_type, 0)?;
    }
    Ok(())
}

fn configure_input(transform: &IMFTransform, width: u32, height: u32, fps: u32) -> Result<()> {
    let media_type = unsafe { MFCreateMediaType() }?;
    unsafe {
        media_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        media_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
        media_type.SetUINT64(&MF_MT_FRAME_SIZE, pack_u64(width, height))?;
        media_type.SetUINT64(&MF_MT_FRAME_RATE, pack_u64(fps, 1))?;
        media_type.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        transform.SetInputType(0, &media_type, 0)?;
    }
    Ok(())
}

fn configure_rate_control(transform: &IMFTransform, bitrate: u32) -> Result<()> {
    let codec: ICodecAPI = transform.cast()?;
    unsafe {
        use windows::Win32::System::Variant::VARIANT;
        // Débit constant : latence prévisible, indispensable en interactif.
        let mode = VARIANT::from(eAVEncCommonRateControlMode_CBR.0);
        codec.SetValue(&CODECAPI_AVEncCommonRateControlMode, &mode)?;
        let rate = VARIANT::from(bitrate);
        codec.SetValue(&CODECAPI_AVEncCommonMeanBitRate, &rate)?;
        // Pas de groupe d'images fermé : on demande les images clés à la volée.
        let gop = VARIANT::from(0u32);
        let _ = codec.SetValue(&CODECAPI_AVEncMPVGOPSize, &gop);
        let low_latency = VARIANT::from(true);
        let _ = codec.SetValue(&CODECAPI_AVLowLatencyMode, &low_latency);
    }
    Ok(())
}

/// Empaquette deux entiers 32 bits dans l'attribut 64 bits attendu par MF.
fn pack_u64(high: u32, low: u32) -> u64 {
    ((high as u64) << 32) | low as u64
}

fn share_device(device: &ID3D11Device) -> Result<IMFDXGIDeviceManager> {
    let mut token = 0u32;
    let mut manager: Option<IMFDXGIDeviceManager> = None;
    unsafe {
        MFCreateDXGIDeviceManager(&mut token, &mut manager)?;
    }
    let manager = manager.ok_or_else(|| anyhow!("gestionnaire DXGI absent"))?;
    unsafe { manager.ResetDevice(device, token)? };
    Ok(manager)
}
```

- [ ] **Step 3: Compiler et corriger les écarts d'API**

Ajouter `#[cfg(windows)] mod encode;` dans `agent/src/main.rs`, puis :

Run: `./scripts/build-agent.sh`

C'est le fichier le plus exposé aux évolutions d'API. Points de friction attendus, avec la marche à suivre :

- `MFT_OUTPUT_DATA_BUFFER` contient des `ManuallyDrop` : si l'initialisation échoue, utiliser `MFT_OUTPUT_DATA_BUFFER::default()` puis affecter `dwStreamID`.
- `VARIANT::from(...)` : vérifier les conversions disponibles dans `windows::Win32::System::Variant` ; à défaut, construire la `VARIANT` manuellement en positionnant `vt` et le champ d'union.
- `GetStringAlloc` peut s'appeler `GetAllocatedString` selon la version — suivre l'erreur du compilateur.
- `MFCreateDXGISurfaceBuffer` attend un `*const c_void` pour la surface ; ajuster le passage de `&frame.texture`.
- Les constantes `ME_TRANSFORM_NEED_INPUT`/`HAVE_OUTPUT` existent peut-être déjà dans le crate (`METransformNeedInput`) : préférer la constante fournie si elle existe.

Expected: compilation réussie.

- [ ] **Step 4: Vérifier l'encodeur par un essai isolé**

Ajouter au mode diagnostic de `agent/src/main.rs`, après la boucle de capture :

```rust
        // Encodage de vérification : ENCODE_TEST=1 encode 120 images capturées.
        if std::env::var("ENCODE_TEST").is_ok() {
            let (w, h) = window::client_size(hwnd)?;
            let mut encoder = encode::H264Encoder::new(capture.device(), w, h, 60, 8_000_000)?;
            encoder.request_keyframe()?;

            let mut encoded = 0usize;
            let mut keyframes = 0usize;
            let mut pts = 0u64;
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while std::time::Instant::now() < deadline && encoded < 120 {
                if let Some(frame) = capture.next_texture()? {
                    encoder.submit(&frame, pts)?;
                    pts += 1500; // 90000 / 60
                }
                while let Some(unit) = encoder.poll_output()? {
                    encoded += 1;
                    if unit.is_keyframe {
                        keyframes += 1;
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            tracing::info!(encoded, keyframes, "images encodées");
            anyhow::ensure!(encoded > 0, "aucune image encodée");
            anyhow::ensure!(keyframes > 0, "aucune image clé produite");
            return Ok(());
        }
```

- [ ] **Step 5: Exécuter l'essai d'encodage**

```bash
./scripts/build-agent.sh
node scripts/winrm.js "\$env:CAPTURE_TEST='firefox'; \$env:ENCODE_TEST='1'; \$env:RUST_LOG='info'; C:\\dev\\target\\debug\\agent.exe 2>&1 | Out-String"
```

Expected: le nom de l'encodeur matériel retenu dans les journaux, puis « images encodées » avec `encoded` proche de 120 et `keyframes ≥ 1`.

Si l'énumération ne trouve aucun encodeur, relire l'étape 1 : le pilote GPU est peut-être absent dans la VM alors que le matériel est présent.

- [ ] **Step 6: Committer**

```bash
git add agent/src/encode.rs agent/src/main.rs
git commit -m "feat: encodeur H.264 matériel Media Foundation en mode asynchrone"
```

---

### Task 11: Source Windows et streaming de Firefox

Assemblage : la capture et l'encodage sont réunis derrière le trait `VideoSource` de la tâche 6. La boucle de session ne change pas — elle ignore l'origine des unités d'accès qu'elle transmet.

**Files:**
- Create: `agent/src/windows_source.rs`
- Modify: `agent/src/main.rs`

**Interfaces:**
- Consumes: `WindowCapture`, `CapturedFrame` (tâche 9), `H264Encoder` (tâche 10), `VideoSource`, `AccessUnit` (tâche 6).
- Produces:
  - `struct WindowsSource` avec `WindowsSource::new(hwnd: HWND, fps: u32, bitrate: u32) -> Result<WindowsSource>`, implémentant `VideoSource`, plus `fn resize(&mut self, width: u32, height: u32) -> Result<()>` et `fn hwnd(&self) -> HWND`.

- [ ] **Step 1: Écrire la source Windows**

`agent/src/windows_source.rs` :

```rust
//! Source vidéo réunissant la capture de fenêtre et l'encodage matériel.

#![cfg(windows)]

use anyhow::Result;
use windows::Win32::Foundation::HWND;

use crate::capture::WindowCapture;
use crate::encode::H264Encoder;
use crate::h264::{AccessUnit, CLOCK_RATE_HZ};
use crate::source::VideoSource;
use crate::window;

pub struct WindowsSource {
    hwnd: HWND,
    capture: WindowCapture,
    encoder: H264Encoder,
    width: u32,
    height: u32,
    fps: u32,
    bitrate: u32,
    /// Horodatage attribué à la prochaine image soumise.
    next_pts_90k: u64,
}

impl WindowsSource {
    pub fn new(hwnd: HWND, fps: u32, bitrate: u32) -> Result<Self> {
        let (width, height) = window::client_size(hwnd)?;
        // L'encodeur H.264 exige des dimensions paires.
        let (width, height) = (width & !1, height & !1);

        let capture = WindowCapture::new(hwnd)?;
        let mut encoder = H264Encoder::new(capture.device(), width, height, fps, bitrate)?;
        encoder.request_keyframe()?;

        Ok(Self {
            hwnd,
            capture,
            encoder,
            width,
            height,
            fps,
            bitrate,
            next_pts_90k: 0,
        })
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Redimensionne la fenêtre et reconstruit la chaîne d'encodage.
    ///
    /// Media Foundation n'autorise pas le changement de résolution en cours de
    /// route : il faut repartir d'un encodeur neuf. L'horodatage, lui, reste
    /// continu — le décodeur du navigateur rejetterait un retour en arrière.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        let (width, height) = (width.max(160) & !1, height.max(120) & !1);
        if (width, height) == (self.width, self.height) {
            return Ok(());
        }

        window::resize_window(self.hwnd, width, height)?;
        // Laisser la fenêtre atteindre sa nouvelle taille avant de recapturer.
        std::thread::sleep(std::time::Duration::from_millis(50));
        let (actual_width, actual_height) = window::client_size(self.hwnd)?;
        let (actual_width, actual_height) = (actual_width & !1, actual_height & !1);

        self.capture = WindowCapture::new(self.hwnd)?;
        self.encoder =
            H264Encoder::new(self.capture.device(), actual_width, actual_height, self.fps, self.bitrate)?;
        self.encoder.request_keyframe()?;
        self.width = actual_width;
        self.height = actual_height;

        tracing::info!(self.width, self.height, "chaîne d'encodage reconstruite");
        Ok(())
    }

    /// Vrai tant que la fenêtre capturée existe.
    pub fn is_alive(&self) -> bool {
        window::is_window_alive(self.hwnd)
    }

    /// Demande une image clé, par exemple sur requête du navigateur.
    pub fn request_keyframe(&mut self) -> Result<()> {
        self.encoder.request_keyframe()
    }
}

impl VideoSource for WindowsSource {
    fn next_frame(&mut self) -> Option<AccessUnit> {
        // Alimenter l'encodeur avec l'image la plus récente, s'il en réclame une.
        match self.capture.next_texture() {
            Ok(Some(frame)) => {
                let pts = self.next_pts_90k;
                if let Err(e) = self.encoder.submit(&frame, pts) {
                    tracing::warn!(erreur = %e, "soumission à l'encodeur échouée");
                } else {
                    self.next_pts_90k += CLOCK_RATE_HZ / self.fps.max(1) as u64;
                }
            }
            Ok(None) => {}
            Err(e) => {
                tracing::error!(erreur = %e, "capture interrompue");
                return None;
            }
        }

        match self.encoder.poll_output() {
            Ok(unit) => unit,
            Err(e) => {
                tracing::warn!(erreur = %e, "récupération de l'image encodée échouée");
                None
            }
        }
    }

    fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}
```

- [ ] **Step 2: Adapter la boucle principale**

Dans `agent/src/main.rs`, remplacer la construction de la source par une sélection selon la plateforme et l'environnement :

```rust
#[cfg(windows)]
mod windows_source;

// … dans main(), à la place du bloc `let source = match &config.test_file`

let source: Box<dyn VideoSource + Send> = match &config.test_file {
    Some(path) => {
        tracing::info!(?path, "source de test");
        Box::new(FileSource::from_path(path, 1280, 720, 60)?)
    }
    None => {
        #[cfg(windows)]
        {
            let title = std::env::var("WINDOW_TITLE").unwrap_or_else(|_| "firefox".into());
            let hwnd = window::find_window_by_title(&title)?;
            let bitrate: u32 = std::env::var("BITRATE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(12_000_000);
            tracing::info!(title, bitrate, "capture de la fenêtre Windows");
            Box::new(windows_source::WindowsSource::new(hwnd, 60, bitrate)?)
        }
        #[cfg(not(windows))]
        {
            anyhow::bail!("TEST_FILE est requis hors Windows")
        }
    }
};
```

Adapter également l'annonce des dimensions au navigateur, qui était figée à 1280×720 :

```rust
let (width, height) = session_dimensions;   // relevé avant de déplacer la source
session.send_control(&AgentControl::ready(width, height)).ok();
```

Relever les dimensions avant de transférer la source à `Session::new` :

```rust
let session_dimensions = source.dimensions();
let mut session = Session::new(source, config.local_ip)?;
```

- [ ] **Step 3: Compiler**

Run: `./scripts/build-agent.sh`
Expected: compilation réussie.

- [ ] **Step 4: Lancer le streaming de Firefox**

Trois éléments à lancer :

```bash
# Sur Linux — signaling accessible depuis la VM
cd signaling && SIGNALING_PORT=8080 npx tsx src/index.ts

# Sur Linux — client web
cd client && npx vite

# Sur la VM — Firefox puis l'agent
node scripts/winrm.js "Start-Process 'C:\\Program Files\\Mozilla Firefox\\firefox.exe'"
node scripts/winrm.js "\$env:SIGNALING_URL='ws://192.168.3.1:8080'; \$env:SESSION_ID='demo'; \$env:LOCAL_IP='192.168.3.2'; \$env:WINDOW_TITLE='firefox'; \$env:RUST_LOG='info'; C:\\dev\\target\\debug\\agent.exe 2>&1 | Out-String"
```

Remplacer `192.168.3.1` par l'adresse de la machine Linux sur le réseau de la VM (`ip -4 addr show | grep 192.168.3` pour la trouver).

Ouvrir `http://localhost:5173/?session=demo`.

Expected: **Firefox s'affiche en direct dans le navigateur.** Faire défiler une page dans Firefox par RDP et vérifier que le mouvement suit dans l'onglet.

- [ ] **Step 5: Relever une première mesure**

Dans `chrome://webrtc-internals`, noter pour la piste vidéo entrante : `framesPerSecond`, `frameWidth`/`frameHeight`, `bytesReceived` (débit), `jitterBufferDelay`. Consigner ces valeurs — elles servent de point de comparaison pour la tâche 13.

Si les images par seconde plafonnent nettement sous 60, ne pas optimiser à l'aveugle : la tâche 13 fournit l'instrumentation nécessaire pour identifier l'étage responsable.

- [ ] **Step 6: Committer**

```bash
git add agent/src/windows_source.rs agent/src/main.rs
git commit -m "feat: streaming de la fenêtre Windows capturée et encodée"
```

---

### Task 12: Injection des entrées

La souris et le clavier. Deux subtilités méritent l'attention : `SendInput` en mode absolu raisonne en coordonnées du bureau virtuel normalisées sur `0..=65535`, pas en coordonnées de fenêtre ; et le clavier passe par des scancodes, seule façon d'obtenir un comportement correct quelle que soit la disposition du clavier.

**Files:**
- Create: `agent/src/input.rs`
- Modify: `agent/src/main.rs`, `client/src/input.ts`, `client/src/main.ts`

**Interfaces:**
- Consumes: `InputMessage`, `MouseButton` (tâche 2), `window::client_size` (tâche 9).
- Produces:
  - Rust : `fn to_virtual_desktop(x: u16, y: u16, window: Rect, desktop: Rect) -> (i32, i32)` (testable partout), `struct InputInjector` avec `InputInjector::new(hwnd: HWND) -> InputInjector` et `fn inject(&mut self, message: InputMessage) -> Result<()>`
  - TypeScript : `attachInput(options: InputOptions): () => void` avec `InputOptions { video: HTMLVideoElement, channel: RTCDataChannel }`

- [ ] **Step 1: Écrire les tests du mapping de coordonnées**

Ce calcul est pur : il se teste sur Linux, sans Windows. Créer `agent/src/input.rs` avec, d'abord, la partie portable et ses tests :

```rust
//! Injection des entrées du navigateur dans la session Windows.

/// Rectangle en coordonnées écran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Convertit une coordonnée normalisée sur la fenêtre en coordonnée normalisée
/// sur le bureau virtuel, seule forme acceptée par `SendInput` en mode absolu.
///
/// `x`/`y` sont dans `0..=65535` relativement à la zone client de `window`.
/// Le résultat est dans `0..=65535` relativement à `desktop`.
pub fn to_virtual_desktop(x: u16, y: u16, window: Rect, desktop: Rect) -> (i32, i32) {
    // Position en pixels écran, au centre du pixel visé.
    let screen_x = window.x as f64 + (x as f64 / 65535.0) * window.width as f64;
    let screen_y = window.y as f64 + (y as f64 / 65535.0) * window.height as f64;

    let width = desktop.width.max(1) as f64;
    let height = desktop.height.max(1) as f64;
    let normalized_x = ((screen_x - desktop.x as f64) / width * 65535.0).round() as i32;
    let normalized_y = ((screen_y - desktop.y as f64) / height * 65535.0).round() as i32;

    (normalized_x.clamp(0, 65535), normalized_y.clamp(0, 65535))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DESKTOP: Rect = Rect { x: 0, y: 0, width: 1920, height: 1080 };

    #[test]
    fn coin_superieur_gauche_d_une_fenetre_a_l_origine() {
        let window = Rect { x: 0, y: 0, width: 1920, height: 1080 };
        assert_eq!(to_virtual_desktop(0, 0, window, DESKTOP), (0, 0));
    }

    #[test]
    fn coin_inferieur_droit_d_une_fenetre_plein_ecran() {
        let window = Rect { x: 0, y: 0, width: 1920, height: 1080 };
        assert_eq!(to_virtual_desktop(65535, 65535, window, DESKTOP), (65535, 65535));
    }

    #[test]
    fn centre_d_une_fenetre_decalee() {
        // Fenêtre de 960×540 placée au centre : son centre est celui de l'écran.
        let window = Rect { x: 480, y: 270, width: 960, height: 540 };
        let (x, y) = to_virtual_desktop(32768, 32768, window, DESKTOP);
        assert!((x - 32768).abs() <= 40, "x = {x}");
        assert!((y - 32768).abs() <= 40, "y = {y}");
    }

    #[test]
    fn origine_d_une_fenetre_decalee() {
        let window = Rect { x: 960, y: 540, width: 960, height: 540 };
        let (x, y) = to_virtual_desktop(0, 0, window, DESKTOP);
        assert_eq!((x, y), (32768, 32768));
    }

    #[test]
    fn borne_les_debordements_sur_un_bureau_multi_ecrans() {
        // Bureau virtuel commençant en coordonnées négatives (écran à gauche).
        let desktop = Rect { x: -1920, y: 0, width: 3840, height: 1080 };
        let window = Rect { x: -1920, y: 0, width: 1920, height: 1080 };
        assert_eq!(to_virtual_desktop(0, 0, window, desktop), (0, 0));
        let (x, _) = to_virtual_desktop(65535, 0, window, desktop);
        assert!((x - 32768).abs() <= 40, "x = {x}");
    }

    #[test]
    fn ne_divise_jamais_par_zero() {
        let degenerate = Rect { x: 0, y: 0, width: 0, height: 0 };
        let (x, y) = to_virtual_desktop(32768, 32768, degenerate, degenerate);
        assert!((0..=65535).contains(&x) && (0..=65535).contains(&y));
    }
}
```

- [ ] **Step 2: Lancer les tests**

Run: `cargo test -p agent input`
Expected: 6 tests réussis, sur Linux.

- [ ] **Step 3: Écrire l'injection Windows**

Ajouter à `agent/src/input.rs`, après la partie portable :

```rust
#[cfg(windows)]
mod win {
    use super::{to_virtual_desktop, Rect};
    use anyhow::{anyhow, Result};
    use proto::input::{InputMessage, MouseButton};
    use windows::Win32::Foundation::{HWND, POINT, RECT};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY,
        KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_HWHEEL,
        MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP,
        MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_VIRTUALDESK,
        MOUSEEVENTF_WHEEL, MOUSEINPUT,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        ClientToScreen, GetClientRect, GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
        SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
    };

    pub struct InputInjector {
        hwnd: HWND,
    }

    impl InputInjector {
        pub fn new(hwnd: HWND) -> Self {
            Self { hwnd }
        }

        pub fn inject(&mut self, message: InputMessage) -> Result<()> {
            match message {
                InputMessage::MouseMove { x, y } => self.move_mouse(x, y),
                InputMessage::MouseButton { button, pressed, x, y } => {
                    // Toujours positionner avant de cliquer : le canal n'est pas
                    // ordonné, le déplacement correspondant a pu se perdre.
                    self.move_mouse(x, y)?;
                    let flags = match (button, pressed) {
                        (MouseButton::Left, true) => MOUSEEVENTF_LEFTDOWN,
                        (MouseButton::Left, false) => MOUSEEVENTF_LEFTUP,
                        (MouseButton::Right, true) => MOUSEEVENTF_RIGHTDOWN,
                        (MouseButton::Right, false) => MOUSEEVENTF_RIGHTUP,
                        (MouseButton::Middle, true) => MOUSEEVENTF_MIDDLEDOWN,
                        (MouseButton::Middle, false) => MOUSEEVENTF_MIDDLEUP,
                    };
                    send_mouse(MOUSEINPUT { dwFlags: flags, ..Default::default() })
                }
                InputMessage::Wheel { delta_x, delta_y } => {
                    if delta_y != 0 {
                        send_mouse(MOUSEINPUT {
                            mouseData: delta_y as i32 as u32,
                            dwFlags: MOUSEEVENTF_WHEEL,
                            ..Default::default()
                        })?;
                    }
                    if delta_x != 0 {
                        send_mouse(MOUSEINPUT {
                            mouseData: delta_x as i32 as u32,
                            dwFlags: MOUSEEVENTF_HWHEEL,
                            ..Default::default()
                        })?;
                    }
                    Ok(())
                }
                InputMessage::Key { scancode, pressed, extended } => {
                    let mut flags = KEYEVENTF_SCANCODE;
                    if !pressed {
                        flags |= KEYEVENTF_KEYUP;
                    }
                    if extended {
                        flags |= KEYEVENTF_EXTENDEDKEY;
                    }
                    send_keyboard(KEYBDINPUT {
                        wVk: Default::default(),
                        wScan: scancode,
                        dwFlags: flags,
                        time: 0,
                        dwExtraInfo: 0,
                    })
                }
            }
        }

        fn move_mouse(&self, x: u16, y: u16) -> Result<()> {
            let window = self.client_rect_on_screen()?;
            let desktop = virtual_desktop();
            let (absolute_x, absolute_y) = to_virtual_desktop(x, y, window, desktop);
            send_mouse(MOUSEINPUT {
                dx: absolute_x,
                dy: absolute_y,
                dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                ..Default::default()
            })
        }

        /// Zone client de la fenêtre, exprimée en coordonnées écran.
        fn client_rect_on_screen(&self) -> Result<Rect> {
            let mut rect = RECT::default();
            unsafe { GetClientRect(self.hwnd, &mut rect)? };
            let mut origin = POINT { x: rect.left, y: rect.top };
            unsafe { ClientToScreen(self.hwnd, &mut origin).ok()? };
            Ok(Rect {
                x: origin.x,
                y: origin.y,
                width: rect.right - rect.left,
                height: rect.bottom - rect.top,
            })
        }
    }

    fn virtual_desktop() -> Rect {
        unsafe {
            Rect {
                x: GetSystemMetrics(SM_XVIRTUALSCREEN),
                y: GetSystemMetrics(SM_YVIRTUALSCREEN),
                width: GetSystemMetrics(SM_CXVIRTUALSCREEN),
                height: GetSystemMetrics(SM_CYVIRTUALSCREEN),
            }
        }
    }

    fn send_mouse(mouse: MOUSEINPUT) -> Result<()> {
        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 { mi: mouse },
        };
        dispatch(&[input])
    }

    fn send_keyboard(keyboard: KEYBDINPUT) -> Result<()> {
        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 { ki: keyboard },
        };
        dispatch(&[input])
    }

    fn dispatch(inputs: &[INPUT]) -> Result<()> {
        let sent = unsafe { SendInput(inputs, std::mem::size_of::<INPUT>() as i32) };
        if sent as usize != inputs.len() {
            return Err(anyhow!("SendInput a refusé l'entrée (session verrouillée ?)"));
        }
        Ok(())
    }
}

#[cfg(windows)]
pub use win::InputInjector;
```

- [ ] **Step 4: Brancher l'injection dans la boucle**

Dans `agent/src/main.rs`, remplacer le rappel `on_input` de démonstration :

```rust
#[cfg(windows)]
let mut injector = window_hwnd.map(input::InputInjector::new);

let mut on_input = |message: proto::input::InputMessage| {
    #[cfg(windows)]
    if let Some(injector) = injector.as_mut() {
        if let Err(e) = injector.inject(message) {
            tracing::warn!(erreur = %e, "injection d'entrée échouée");
        }
    }
    #[cfg(not(windows))]
    tracing::debug!(?message, "entrée reçue");
};
```

`window_hwnd` est un `Option<HWND>` renseigné lors de la création de la source Windows ; le déclarer près de la construction de la source :

```rust
#[cfg(windows)]
let mut window_hwnd: Option<windows::Win32::Foundation::HWND> = None;
```

et l'affecter dans la branche Windows : `window_hwnd = Some(hwnd);`

Ajouter `mod input;` à la liste des modules (sans `#[cfg(windows)]` : la partie mapping est portable et testée sur Linux).

- [ ] **Step 5: Écrire la capture d'entrées côté navigateur**

`client/src/input.ts` :

```typescript
// Capture des entrées et envoi sur le canal binaire.
//
// Les coordonnées sont normalisées sur 0..65535 par rapport à la zone d'image
// réellement affichée : `object-fit: contain` laisse des bandes noires qu'il
// faut exclure, sans quoi le pointeur dérive.

import {
    encodeKey,
    encodeMouseButton,
    encodeMouseMove,
    encodeWheel,
    type MouseButtonCode,
} from '../../proto/ts/input';

export interface InputOptions {
    video: HTMLVideoElement;
    channel: RTCDataChannel;
}

/// Zone occupée par l'image dans l'élément vidéo, bandes noires exclues.
function contentRect(video: HTMLVideoElement): DOMRect {
    const element = video.getBoundingClientRect();
    const sourceWidth = video.videoWidth;
    const sourceHeight = video.videoHeight;
    if (!sourceWidth || !sourceHeight) return element;

    const scale = Math.min(element.width / sourceWidth, element.height / sourceHeight);
    const width = sourceWidth * scale;
    const height = sourceHeight * scale;
    return new DOMRect(
        element.x + (element.width - width) / 2,
        element.y + (element.height - height) / 2,
        width,
        height,
    );
}

function normalize(video: HTMLVideoElement, clientX: number, clientY: number): [number, number] {
    const rect = contentRect(video);
    const x = ((clientX - rect.x) / Math.max(1, rect.width)) * 65535;
    const y = ((clientY - rect.y) / Math.max(1, rect.height)) * 65535;
    return [x, y];
}

const BUTTON_MAP: Record<number, MouseButtonCode> = { 0: 0, 1: 2, 2: 1 };

export function attachInput({ video, channel }: InputOptions): () => void {
    const send = (payload: Uint8Array): void => {
        if (channel.readyState === 'open') channel.send(payload);
    };

    const onPointerMove = (event: PointerEvent): void => {
        // getCoalescedEvents restitue les positions intermédiaires que le
        // navigateur a regroupées : le tracé reste fidèle à haute fréquence.
        const events = event.getCoalescedEvents?.() ?? [event];
        for (const sample of events) {
            const [x, y] = normalize(video, sample.clientX, sample.clientY);
            send(encodeMouseMove(x, y));
        }
    };

    const onPointerDown = (event: PointerEvent): void => {
        video.setPointerCapture(event.pointerId);
        const [x, y] = normalize(video, event.clientX, event.clientY);
        send(encodeMouseButton(BUTTON_MAP[event.button] ?? 0, true, x, y));
    };

    const onPointerUp = (event: PointerEvent): void => {
        const [x, y] = normalize(video, event.clientX, event.clientY);
        send(encodeMouseButton(BUTTON_MAP[event.button] ?? 0, false, x, y));
    };

    const onWheel = (event: WheelEvent): void => {
        event.preventDefault();
        // Windows compte 120 unités par cran ; deltaMode 0 est en pixels.
        const factor = event.deltaMode === 0 ? -120 / 100 : -120;
        send(encodeWheel(event.deltaX * -factor, event.deltaY * factor));
    };

    const onContextMenu = (event: Event): void => event.preventDefault();

    const onKeyDown = (event: KeyboardEvent): void => {
        event.preventDefault();
        const mapped = SCANCODES[event.code];
        if (mapped) send(encodeKey(mapped.scancode, true, mapped.extended));
    };

    const onKeyUp = (event: KeyboardEvent): void => {
        event.preventDefault();
        const mapped = SCANCODES[event.code];
        if (mapped) send(encodeKey(mapped.scancode, false, mapped.extended));
    };

    video.addEventListener('pointermove', onPointerMove);
    video.addEventListener('pointerdown', onPointerDown);
    video.addEventListener('pointerup', onPointerUp);
    video.addEventListener('wheel', onWheel, { passive: false });
    video.addEventListener('contextmenu', onContextMenu);
    window.addEventListener('keydown', onKeyDown);
    window.addEventListener('keyup', onKeyUp);

    return () => {
        video.removeEventListener('pointermove', onPointerMove);
        video.removeEventListener('pointerdown', onPointerDown);
        video.removeEventListener('pointerup', onPointerUp);
        video.removeEventListener('wheel', onWheel);
        video.removeEventListener('contextmenu', onContextMenu);
        window.removeEventListener('keydown', onKeyDown);
        window.removeEventListener('keyup', onKeyUp);
    };
}

/// Correspondance `KeyboardEvent.code` → scancode PS/2 (jeu 1).
///
/// On passe par les scancodes plutôt que par les codes de touches virtuelles :
/// c'est la position physique de la touche qui est transmise, donc la
/// disposition configurée côté Windows s'applique correctement.
const SCANCODES: Record<string, { scancode: number; extended: boolean }> = {
    Escape: { scancode: 0x01, extended: false },
    Digit1: { scancode: 0x02, extended: false },
    Digit2: { scancode: 0x03, extended: false },
    Digit3: { scancode: 0x04, extended: false },
    Digit4: { scancode: 0x05, extended: false },
    Digit5: { scancode: 0x06, extended: false },
    Digit6: { scancode: 0x07, extended: false },
    Digit7: { scancode: 0x08, extended: false },
    Digit8: { scancode: 0x09, extended: false },
    Digit9: { scancode: 0x0a, extended: false },
    Digit0: { scancode: 0x0b, extended: false },
    Minus: { scancode: 0x0c, extended: false },
    Equal: { scancode: 0x0d, extended: false },
    Backspace: { scancode: 0x0e, extended: false },
    Tab: { scancode: 0x0f, extended: false },
    KeyQ: { scancode: 0x10, extended: false },
    KeyW: { scancode: 0x11, extended: false },
    KeyE: { scancode: 0x12, extended: false },
    KeyR: { scancode: 0x13, extended: false },
    KeyT: { scancode: 0x14, extended: false },
    KeyY: { scancode: 0x15, extended: false },
    KeyU: { scancode: 0x16, extended: false },
    KeyI: { scancode: 0x17, extended: false },
    KeyO: { scancode: 0x18, extended: false },
    KeyP: { scancode: 0x19, extended: false },
    BracketLeft: { scancode: 0x1a, extended: false },
    BracketRight: { scancode: 0x1b, extended: false },
    Enter: { scancode: 0x1c, extended: false },
    ControlLeft: { scancode: 0x1d, extended: false },
    KeyA: { scancode: 0x1e, extended: false },
    KeyS: { scancode: 0x1f, extended: false },
    KeyD: { scancode: 0x20, extended: false },
    KeyF: { scancode: 0x21, extended: false },
    KeyG: { scancode: 0x22, extended: false },
    KeyH: { scancode: 0x23, extended: false },
    KeyJ: { scancode: 0x24, extended: false },
    KeyK: { scancode: 0x25, extended: false },
    KeyL: { scancode: 0x26, extended: false },
    Semicolon: { scancode: 0x27, extended: false },
    Quote: { scancode: 0x28, extended: false },
    Backquote: { scancode: 0x29, extended: false },
    ShiftLeft: { scancode: 0x2a, extended: false },
    Backslash: { scancode: 0x2b, extended: false },
    KeyZ: { scancode: 0x2c, extended: false },
    KeyX: { scancode: 0x2d, extended: false },
    KeyC: { scancode: 0x2e, extended: false },
    KeyV: { scancode: 0x2f, extended: false },
    KeyB: { scancode: 0x30, extended: false },
    KeyN: { scancode: 0x31, extended: false },
    KeyM: { scancode: 0x32, extended: false },
    Comma: { scancode: 0x33, extended: false },
    Period: { scancode: 0x34, extended: false },
    Slash: { scancode: 0x35, extended: false },
    ShiftRight: { scancode: 0x36, extended: false },
    AltLeft: { scancode: 0x38, extended: false },
    Space: { scancode: 0x39, extended: false },
    CapsLock: { scancode: 0x3a, extended: false },
    F1: { scancode: 0x3b, extended: false },
    F2: { scancode: 0x3c, extended: false },
    F3: { scancode: 0x3d, extended: false },
    F4: { scancode: 0x3e, extended: false },
    F5: { scancode: 0x3f, extended: false },
    F6: { scancode: 0x40, extended: false },
    F7: { scancode: 0x41, extended: false },
    F8: { scancode: 0x42, extended: false },
    F9: { scancode: 0x43, extended: false },
    F10: { scancode: 0x44, extended: false },
    F11: { scancode: 0x57, extended: false },
    F12: { scancode: 0x58, extended: false },
    IntlBackslash: { scancode: 0x56, extended: false },
    // Touches étendues : préfixe 0xE0 côté matériel, indicateur `extended` ici.
    ControlRight: { scancode: 0x1d, extended: true },
    AltRight: { scancode: 0x38, extended: true },
    NumpadEnter: { scancode: 0x1c, extended: true },
    NumpadDivide: { scancode: 0x35, extended: true },
    Home: { scancode: 0x47, extended: true },
    ArrowUp: { scancode: 0x48, extended: true },
    PageUp: { scancode: 0x49, extended: true },
    ArrowLeft: { scancode: 0x4b, extended: true },
    ArrowRight: { scancode: 0x4d, extended: true },
    End: { scancode: 0x4f, extended: true },
    ArrowDown: { scancode: 0x50, extended: true },
    PageDown: { scancode: 0x51, extended: true },
    Insert: { scancode: 0x52, extended: true },
    Delete: { scancode: 0x53, extended: true },
    MetaLeft: { scancode: 0x5b, extended: true },
    MetaRight: { scancode: 0x5c, extended: true },
};
```

- [ ] **Step 6: Brancher les entrées dans le client**

Dans `client/src/main.ts`, après l'établissement de la session :

```typescript
import { attachInput } from './input';

// … remplacer l'appel connectSession par :
connectSession({ /* … options inchangées … */ })
    .then((session) => {
        attachInput({ video, channel: session.inputChannel });
        video.focus();
    })
    .catch((error: unknown) => {
        setStatus(`échec : ${error instanceof Error ? error.message : String(error)}`);
    });
```

Ajouter `tabindex="0"` à l'élément vidéo dans `client/index.html` pour qu'il puisse recevoir le focus :

```html
<video id="remote" autoplay playsinline muted tabindex="0"></video>
```

- [ ] **Step 7: Vérifier la compilation des deux côtés**

```bash
cargo test -p agent          # les 6 tests de mapping doivent passer
cd client && npx tsc --noEmit && cd ..
./scripts/build-agent.sh
```

Expected: tout réussit.

- [ ] **Step 8: Essai manuel des entrées**

Relancer le trio de la tâche 11, puis, depuis le navigateur :

1. Déplacer le pointeur sur un lien de la page Firefox — il doit se souligner au bon endroit, sans décalage.
2. Cliquer sur un lien — il doit s'ouvrir.
3. Faire défiler à la molette — le défilement doit suivre, dans le bon sens.
4. Cliquer dans la barre d'adresse et taper `test.example` — les caractères doivent apparaître.
5. Taper un caractère accenté (`é` sur clavier français) — il doit apparaître correctement. Sinon, vérifier que la disposition du clavier de la session Windows correspond à celle utilisée physiquement.
6. Utiliser les flèches et la touche Suppr — les touches étendues doivent fonctionner.

Consigner tout écart. Un décalage constant du pointeur signale une erreur de `contentRect` ou de DPI ; un décalage proportionnel signale une confusion entre zone client et zone fenêtre.

- [ ] **Step 9: Committer**

```bash
git add agent/src/input.rs agent/src/main.rs client/src/input.ts client/src/main.ts client/index.html
git commit -m "feat: injection des entrées souris et clavier par scancodes"
```

---

### Task 13: Redimensionnement et fin de session

Le dernier critère fonctionnel : redimensionner la fenêtre du navigateur redimensionne Firefox côté Windows. Le canal de contrôle transporte la demande ; l'agent la temporise, car un redimensionnement reconstruit toute la chaîne d'encodage et l'utilisateur produit des dizaines d'événements en tirant sur un bord.

**Files:**
- Modify: `agent/src/main.rs`, `client/src/main.ts`

**Interfaces:**
- Consumes: `ClientControl::Resize`, `AgentControl::SessionEnd` (tâche 4), `WindowsSource::resize`, `WindowsSource::is_alive` (tâche 11).
- Produces: aucune interface nouvelle — câblage de comportements existants.

- [ ] **Step 1: Émettre le redimensionnement depuis le navigateur**

Dans `client/src/main.ts`, après `attachInput` :

```typescript
import { encodeResize } from '../../proto/ts/control';

// … dans le .then(session => { … })

// Le redimensionnement reconstruit la chaîne d'encodage côté agent : on
// n'émet donc qu'une fois le geste terminé, pas à chaque pixel parcouru.
let resizeTimer: number | undefined;
const observer = new ResizeObserver(() => {
    window.clearTimeout(resizeTimer);
    resizeTimer = window.setTimeout(() => {
        if (session.controlChannel.readyState !== 'open') return;
        const width = Math.round(video.clientWidth * window.devicePixelRatio);
        const height = Math.round(video.clientHeight * window.devicePixelRatio);
        session.controlChannel.send(encodeResize(width, height));
    }, 200);
});
observer.observe(video);
```

- [ ] **Step 2: Traiter le redimensionnement dans l'agent**

Le rappel `on_control` de `main.rs` ne peut pas emprunter la source, déjà déplacée dans `Session`. On passe donc par une variable partagée que la boucle consulte.

Dans `agent/src/main.rs`, avant la boucle :

```rust
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

// Dimensions demandées par le client, empaquetées : largeur << 32 | hauteur.
// Zéro signifie « aucune demande en attente ».
let requested_size = Arc::new(AtomicU64::new(0));
let requested_size_for_callback = Arc::clone(&requested_size);

let mut on_control = move |message: proto::control::ClientControl| match message {
    proto::control::ClientControl::Resize { width, height, .. } => {
        tracing::info!(width, height, "redimensionnement demandé");
        requested_size_for_callback
            .store(((width as u64) << 32) | height as u64, Ordering::Relaxed);
    }
};
```

Et dans le corps de la boucle, après le `tick` :

```rust
        // Appliquer une éventuelle demande de redimensionnement.
        #[cfg(windows)]
        {
            let packed = requested_size.swap(0, Ordering::Relaxed);
            if packed != 0 {
                let (width, height) = ((packed >> 32) as u32, packed as u32);
                if let Err(e) = session.resize_source(width, height) {
                    tracing::warn!(erreur = %e, "redimensionnement échoué");
                } else if let Some((w, h)) = session.source_dimensions() {
                    session.send_control(&AgentControl::ready(w, h)).ok();
                }
            }

            // Fin de session si la fenêtre capturée a disparu.
            if !session.source_alive() {
                session
                    .send_control(&AgentControl::session_end("fenêtre fermée"))
                    .ok();
                // Laisser le message partir avant de couper.
                for _ in 0..50 {
                    let _ = session.tick(&mut on_input, &mut on_control);
                }
                tracing::info!("fenêtre fermée, session terminée");
                break;
            }
        }
```

- [ ] **Step 3: Exposer la source depuis la session**

`Session` détient la source ; il lui faut trois accès. Ajouter à `agent/src/transport.rs`, dans `impl Session` :

```rust
    /// Dimensions courantes de la source, si elle en déclare.
    pub fn source_dimensions(&self) -> Option<(u32, u32)> {
        Some(self.source.dimensions())
    }

    /// Redimensionne la source si elle le permet.
    #[cfg(windows)]
    pub fn resize_source(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        match self.source.as_resizable() {
            Some(source) => source.resize(width, height),
            None => Ok(()),
        }
    }

    /// Vrai si la source est toujours exploitable.
    #[cfg(windows)]
    pub fn source_alive(&self) -> bool {
        self.source.as_resizable().map_or(true, |s| s.is_alive())
    }
```

Cela suppose d'étendre le trait `VideoSource`. Dans `agent/src/source.rs` :

```rust
/// Source dont la géométrie suit celle demandée par le client.
#[cfg(windows)]
pub trait ResizableSource {
    fn resize(&mut self, width: u32, height: u32) -> anyhow::Result<()>;
    fn is_alive(&self) -> bool;
}

pub trait VideoSource {
    fn next_frame(&mut self) -> Option<AccessUnit>;
    fn dimensions(&self) -> (u32, u32);

    /// Vue redimensionnable de cette source, si elle en est capable.
    ///
    /// La source de test ne l'est pas et renvoie `None` — le reste du code n'a
    /// donc pas à distinguer les deux cas.
    #[cfg(windows)]
    fn as_resizable(&mut self) -> Option<&mut dyn ResizableSource> {
        None
    }
}
```

Et dans `agent/src/windows_source.rs`, implémenter le trait puis l'exposer :

```rust
impl crate::source::ResizableSource for WindowsSource {
    fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        WindowsSource::resize(self, width, height)
    }

    fn is_alive(&self) -> bool {
        WindowsSource::is_alive(self)
    }
}
```

Ajouter dans `impl VideoSource for WindowsSource` :

```rust
    fn as_resizable(&mut self) -> Option<&mut dyn crate::source::ResizableSource> {
        Some(self)
    }
```

- [ ] **Step 4: Compiler et tester**

```bash
cargo test -p agent
cd client && npx tsc --noEmit && cd ..
./scripts/build-agent.sh
```

Expected: tout réussit. Les tests existants doivent rester verts — l'ajout d'une méthode par défaut au trait ne casse pas `FileSource`.

- [ ] **Step 5: Essai manuel du redimensionnement**

Relancer le trio, puis redimensionner la fenêtre du navigateur.

Expected: après environ 200 ms d'immobilité, Firefox change de taille côté Windows et l'image se recadre. Chronométrer entre l'arrêt du geste et la stabilisation de l'image : le critère de la spec est **moins de 500 ms**. Consigner la mesure.

Fermer Firefox depuis la VM : le bandeau doit afficher « session terminée : fenêtre fermée ».

- [ ] **Step 6: Committer**

```bash
git add agent/src client/src
git commit -m "feat: redimensionnement de la fenêtre distante et fin de session"
```

---

### Task 14: Instrumentation et recette du jalon

Les cinq critères d'acceptation de la spec doivent être **mesurés**, pas ressentis. Cette tâche fournit l'instrument, puis conduit la recette et consigne les résultats.

**Files:**
- Create: `client/src/stats.ts`, `docs/superpowers/plans/2026-07-27-jalon1-recette.md`
- Modify: `client/src/main.ts`, `client/index.html`, `client/src/style.css`

**Interfaces:**
- Consumes: `RTCPeerConnection` (tâche 8).
- Produces: `attachStats(pc: RTCPeerConnection, element: HTMLElement): () => void`

- [ ] **Step 1: Écrire l'overlay de statistiques**

`client/src/stats.ts` :

```typescript
// Overlay de mesure. Les valeurs proviennent de getStats() : ce sont celles du
// navigateur lui-même, pas une estimation de notre part.

interface Snapshot {
    framesDecoded: number;
    bytesReceived: number;
    timestamp: number;
}

export function attachStats(pc: RTCPeerConnection, element: HTMLElement): () => void {
    let previous: Snapshot | undefined;

    const timer = window.setInterval(async () => {
        const report = await pc.getStats();
        let inbound: RTCInboundRtpStreamStats | undefined;
        let pair: RTCIceCandidatePairStats | undefined;

        report.forEach((stat) => {
            if (stat.type === 'inbound-rtp' && (stat as any).kind === 'video') {
                inbound = stat as RTCInboundRtpStreamStats;
            }
            if (stat.type === 'candidate-pair' && (stat as any).nominated) {
                pair = stat as RTCIceCandidatePairStats;
            }
        });
        if (!inbound) return;

        const current: Snapshot = {
            framesDecoded: (inbound as any).framesDecoded ?? 0,
            bytesReceived: (inbound as any).bytesReceived ?? 0,
            timestamp: inbound.timestamp,
        };

        let fps = 0;
        let mbps = 0;
        if (previous) {
            const seconds = (current.timestamp - previous.timestamp) / 1000;
            if (seconds > 0) {
                fps = (current.framesDecoded - previous.framesDecoded) / seconds;
                mbps =
                    ((current.bytesReceived - previous.bytesReceived) * 8) / seconds / 1_000_000;
            }
        }
        previous = current;

        const rttMs = (pair?.currentRoundTripTime ?? 0) * 1000;
        // Latence bout en bout approchée : la moitié de l'aller-retour réseau,
        // plus l'attente en tampon de gigue et le décodage côté navigateur.
        const jitterBufferMs = averageDelay(inbound);
        const glassToGlassMs = rttMs / 2 + jitterBufferMs;

        const width = (inbound as any).frameWidth ?? 0;
        const height = (inbound as any).frameHeight ?? 0;

        element.textContent = [
            `${fps.toFixed(1)} i/s`,
            `${width}×${height}`,
            `${mbps.toFixed(2)} Mb/s`,
            `RTT ${rttMs.toFixed(1)} ms`,
            `tampon ${jitterBufferMs.toFixed(1)} ms`,
            `≈ ${glassToGlassMs.toFixed(1)} ms`,
            `perdues ${(inbound as any).framesDropped ?? 0}`,
        ].join('  ·  ');
    }, 1000);

    return () => window.clearInterval(timer);
}

/// Délai moyen passé en tampon de gigue, par image émise.
function averageDelay(inbound: RTCInboundRtpStreamStats): number {
    const total = (inbound as any).jitterBufferDelay ?? 0;
    const count = (inbound as any).jitterBufferEmittedCount ?? 0;
    return count > 0 ? (total / count) * 1000 : 0;
}
```

- [ ] **Step 2: Afficher l'overlay**

Dans `client/index.html`, ajouter après l'élément `#status` :

```html
<div id="stats" role="status"></div>
```

Dans `client/src/style.css` :

```css
#stats {
    position: fixed;
    inset-block-end: 12px;
    inset-inline-start: 12px;
    padding: 6px 12px;
    border-radius: 6px;
    background: rgb(0 0 0 / 0.72);
    font-variant-numeric: tabular-nums;
    font-size: 12px;
    letter-spacing: 0.02em;
}
```

Dans `client/src/main.ts`, dans le `.then(session => …)` :

```typescript
import { attachStats } from './stats';

const statsElement = document.querySelector<HTMLDivElement>('#stats')!;
attachStats(session.pc, statsElement);
```

- [ ] **Step 3: Vérifier la compilation**

Run: `cd client && npx tsc --noEmit`
Expected: aucune erreur. Les statistiques WebRTC étant partiellement typées, les accès via `as any` sont volontaires et signalés comme tels.

- [ ] **Step 4: Conduire la recette**

Lancer le système complet (trio de la tâche 11), ouvrir Firefox sur une page riche — par exemple `https://www.wikipedia.org` puis un article long — et mesurer chaque critère de la spec :

| Critère | Mesure | Cible |
|---|---|---|
| 1. Firefox capturé par fenêtre | l'image montre Firefox seul, sans le bureau | oui |
| 2. Défilement à 60 i/s | overlay, pendant un défilement continu de 10 s | ≥ 55 i/s en moyenne |
| 3. Latence bout en bout | overlay, colonne `≈` | < 50 ms |
| 4. Souris et clavier | protocole de la tâche 12, étapes 1 à 6 | tous réussis |
| 5. Redimensionnement | chronomètre, tâche 13 étape 5 | < 500 ms |

- [ ] **Step 5: Consigner les résultats**

Créer `docs/superpowers/plans/2026-07-27-jalon1-recette.md` avec, pour chaque critère : la valeur mesurée, le verdict (atteint / non atteint), et pour tout critère non atteint, l'étage responsable identifié grâce au budget de latence de la spec (capture ≤ 5 ms, encodage ≤ 10 ms, réseau ≤ 5 ms, tampon de gigue et décodage ≤ 20 ms).

Y consigner également les écarts d'API rencontrés aux tâches 7, 9 et 10 : ce sont les informations les plus utiles pour la suite du produit, et elles disparaissent si elles ne sont pas écrites.

- [ ] **Step 6: Si le nombre d'images par seconde est insuffisant, diagnostiquer par étage**

Ne pas optimiser au hasard. Instrumenter dans cet ordre, en ajoutant des mesures temporaires dans `WindowsSource::next_frame` :

1. **Capture** — compter les images renvoyées par `capture.next_texture()` par seconde. Sous 60, le problème est en amont : la fenêtre est peut-être occluse ou le compositeur limite la cadence.
2. **Encodage** — compter les unités renvoyées par `encoder.poll_output()` par seconde. Un écart avec la capture signale que l'encodeur est saturé : baisser le débit cible, ou vérifier que la MFT matérielle est bien celle qui a été activée (le journal de la tâche 10 le nomme).
3. **Envoi** — compter les appels réussis à `send_next_frame`. Un écart avec l'encodage signale que la boucle `tick` n'a pas assez de temps de calcul : la cadence fixe de `main.rs` limite peut-être artificiellement l'envoi.
4. **Réception** — comparer `framesDecoded` du navigateur au nombre envoyé. Un écart signale de la perte réseau ou un tampon d'envoi saturé.

- [ ] **Step 7: Committer**

```bash
git add client/src/stats.ts client/src/main.ts client/index.html client/src/style.css docs/superpowers/plans/2026-07-27-jalon1-recette.md
git commit -m "feat: overlay de mesure et recette du jalon 1"
```

---

## Résumé de l'exécution

| Tâche | Livrable | Où cela s'exécute |
|---|---|---|
| 1 | Monorepo et compilation par WinRM | Linux + VM |
| 2 | Codec binaire des entrées | Linux |
| 3 | Vecteurs partagés et encodeur TypeScript | Linux |
| 4 | Protocole de contrôle | Linux |
| 5 | Serveur de signaling | Linux |
| 6 | Découpage Annex-B et source de test | Linux |
| 7 | Transport str0m | Linux |
| **8** | **Vidéo de bout en bout dans le navigateur** | **Linux — jalon de dérisquage** |
| 9 | Capture de fenêtre | VM |
| 10 | Encodeur H.264 matériel | VM |
| 11 | Firefox streamé | VM + Linux |
| 12 | Entrées souris et clavier | VM + Linux |
| 13 | Redimensionnement et fin de session | VM + Linux |
| 14 | Mesure et recette | VM + Linux |

Les tâches 1 à 8 ne dépendent d'aucune API Windows. Si la tâche 8 échoue, le pari technique est remis en cause avant tout investissement dans la capture et l'encodage — c'est l'intérêt de cet ordre.

## Couverture de la spécification

Les cinq critères d'acceptation du §1 de la spec sont couverts : capture par fenêtre (tâche 11), 60 images par seconde et latence (tâche 14), entrées (tâche 12), redimensionnement (tâche 13).

Le tableau de gestion des erreurs du §5 est couvert **sauf une ligne** : « coupure réseau → le navigateur relance le signaling et renégocie ; l'agent maintient capture et fenêtre 60 s avant nettoyage ». Aucune tâche ne l'implémente, et c'est délibéré :

- la reconnexion n'est pas un critère d'acceptation du jalon, qui vise à valider le pari technique du streaming ;
- elle demande une machine à états côté agent (session en attente de reprise plutôt que terminée) et une boucle de reprise côté navigateur, soit une tâche à part entière ;
- sur le réseau local de développement (0,43 ms, filaire), elle ne serait jamais exercée — donc jamais réellement testée.

À traiter en début de sous-projet suivant, avant toute exposition hors du réseau local. Les autres lignes du §5 sont couvertes : encodeur matériel absent (tâche 10, `find_hardware_encoder` échoue avec un diagnostic explicite), fenêtre cible fermée (tâche 13), agent absent à la connexion (tâche 8, `webrtc.ts` rejette sur `peer-gone`).

