# Jalon 1 — Tranche verticale : streaming d'une app Windows réelle

**Date** : 27 juillet 2026
**Statut** : Spécification validée
**Parent** : [Refonte produit](./2026-07-27-refonte-produit-design.md) — sous-projets ① (agent) et ② (client web)

---

## 1. Objectif et critères d'acceptation

Valider le pari technique central de la refonte : capture par fenêtre +
encodage matériel + WebRTC + décodage matériel navigateur, avec une application
réelle exigeante.

**Critères d'acceptation — mesurés, pas ressentis** :

1. Firefox (sur la VM Windows 192.168.3.2, GPU avec encodeur matériel confirmé)
   est streamé dans le navigateur, capturé **par fenêtre**.
2. Scroll soutenu dans Firefox à **60 fps** côté navigateur (stats WebRTC
   affichées par l'overlay).
3. Latence glass-to-glass **< 50 ms** sur LAN (mesurée par l'overlay).
4. Souris (mouvements, clics, molette) et clavier (y compris caractères
   accentués via scancodes) fonctionnels.
5. Redimensionner la fenêtre du navigateur redimensionne la fenêtre Firefox
   côté Windows en **< 500 ms**.

**Hors périmètre du jalon** : auth, presse-papier, fichiers, audio, PWA/manifest,
installation d'apps, service Windows, multi-sessions, STUN/TURN (LAN
uniquement), fallback encodage logiciel.

## 2. Architecture

```
web/ (Vite+TS)          platform/ (Node/TS)         agent/ (Rust)
┌──────────────┐  WS    ┌──────────────────┐  WS   ┌─────────────────────┐
│ RTCPeer-     │◄──────►│ Signaling minimal │◄────►│ signaling (client)  │
│ Connection   │  SDP   │ (relais SDP/ICE)  │       ├─────────────────────┤
├──────────────┤        └──────────────────┘       │ transport (str0m)   │
│ <video>      │◄═══════════ SRTP H.264 ══════════►│  socket UDP + boucle│
│ décodage HW  │                                    │  d'événements tokio │
├──────────────┤   data channel "input" (binaire)   ├─────────────────────┤
│ input capture│═══════════════════════════════════►│ input (SendInput)   │
├──────────────┤   data channel "control" (JSON)    ├─────────────────────┤
│ resize, stats│◄══════════════════════════════════►│ window (resize/vie) │
└──────────────┘                                    ├─────────────────────┤
                                                    │ capture (WGC)       │
                                                    │ encode (MF H.264 HW)│
                                                    └─────────────────────┘
```

Décision actée : pile WebRTC **str0m** (sans-IO) — contrôle total du socket,
du pacing et de la boucle d'événements, testabilité déterministe. Choix assumé
d'un jalon plus long que l'alternative webrtc-rs.

## 3. Composants

### 3.1 Agent Rust (`agent/`)

Exécutable console pour ce jalon (service Windows au jalon suivant). Crates
principales : `windows` (windows-rs), `str0m`, `tokio`, `serde`.

Six modules à responsabilité unique :

| Module | Rôle | Dépendances |
|---|---|---|
| `capture` | `Windows.Graphics.Capture` sur la fenêtre cible ; frame pool D3D11 ; textures BGRA restant sur GPU | windows-rs, D3D11 |
| `encode` | H.264 matériel via Media Foundation (`IMFTransform`), entrée texture D3D11 zéro-copie, `CODECAPI_AVLowLatencyMode`, débit adaptatif ; sortie Annex-B → paquetisation RTP | capture (textures) |
| `transport` | str0m : ICE (host candidates), DTLS, SRTP, SCTP ; nous possédons le socket UDP et la boucle tokio ; envoi RTP + réception data channels | encode (NAL), tokio |
| `input` | Injection `SendInput` ; coordonnées navigateur → fenêtre ; clavier par scancodes ; molette | transport (channel input) |
| `window` | Lancement/repérage de Firefox, resize, détection de fermeture | windows-rs |
| `signaling` | Client WebSocket vers platform/ ; échange SDP/ICE | tokio-tungstenite |

Data channels : `input` non fiable/non ordonné (latence minimale, perte
tolérée), `control` fiable/ordonné (resize, fin de session, couleur d'accent).

### 3.2 Protocole partagé (`proto/`)

- Messages **input** : binaire, taille fixe, little-endian —
  `type: u8 | version: u8 | payload`. Types : mouse-move, mouse-button,
  wheel, key. Pas de JSON sur le chemin chaud.
- Messages **control** : JSON versionné (`{v, type, ...}`) — resize,
  session-end, stats.
- Schéma défini une seule fois, générant les structs Rust (serde) et les types
  TypeScript ; tests de round-trip Rust ⇆ TS.

### 3.3 Client web (`web/`)

Vite + TypeScript, zéro framework pour ce jalon.

- `RTCPeerConnection` ; piste vidéo → `<video>` (décodage matériel du
  navigateur). `playoutDelayHint`/`jitterBufferTarget` au minimum.
- Input : Pointer Events avec `getCoalescedEvents()`, `KeyboardEvent.code`
  (scancodes), wheel — encodés en binaire sur le channel `input`.
- `ResizeObserver` → message resize sur `control`.
- **Overlay de stats** (instrument de validation) : fps décodés, bitrate, RTT,
  latence glass-to-glass estimée, frames perdues.

### 3.4 Signaling (`platform/`)

Node/TS + `ws`, volontairement minimal (~150 lignes) : sessions par ID, relais
des offres/réponses SDP et candidats ICE entre navigateur et agent. Embryon de
la future plateforme — aucune auth dans ce jalon.

## 4. Flux nominal

1. `platform/` démarre ; l'agent (lancé manuellement sur la VM) s'y connecte en
   WS et attend.
2. L'utilisateur ouvre la page web → création de session → offre SDP relayée.
3. str0m répond ; ICE direct en LAN ; DTLS/SRTP établis.
4. L'agent lance/repère Firefox, démarre capture + encodeur, pousse le flux.
5. Input navigateur → channel `input` → `SendInput`. Resize → channel
   `control` → `SetWindowPos` → la capture suit.
6. Fermeture de Firefox → message `session-end` → la page l'affiche.

## 5. Gestion des erreurs

| Cas | Comportement |
|---|---|
| Encodeur matériel absent | Erreur explicite au démarrage de l'agent avec diagnostic (énumération des encodeurs MF) ; pas de fallback dans ce jalon |
| Coupure réseau | Le navigateur relance signaling + renégociation ; l'agent maintient capture/fenêtre 60 s avant nettoyage |
| Fenêtre cible fermée | `session-end` propre sur `control` |
| Agent absent au moment de la connexion | La page web l'indique clairement |

## 6. Tests

- **Transport** : str0m sans-IO → boucle d'événements testée en déterministe
  (paquets simulés, pas de réseau ni de temps réel).
- **Unitaires agent** : mapping coordonnées, table scancodes, paquetiseur RTP,
  encodage/décodage du protocole binaire.
- **Protocole** : round-trip Rust ⇆ TypeScript sur vecteurs de test partagés.
- **Acceptation** : session manuelle instrumentée par l'overlay de stats ;
  chaque critère du §1 vérifié et consigné.
- CI Windows (capture/encodage sur VM réelle) : reportée après le jalon —
  validation manuelle documentée pour l'instant.

## 7. Structure du dépôt

Monorepo dans ce dépôt : `agent/` (cargo), `web/` (Vite), `platform/`
(Node/TS), `proto/` (schéma partagé). L'ancien code (racine, `src/`, `web/`
historique) reste intact jusqu'au remplacement complet.

Note : le répertoire historique `web/` (client Guacamole) et le nouveau `web/`
entrent en conflit de nom — le nouveau client web du monorepo s'appellera
`web/` seulement après suppression de l'ancien ; d'ici là il vit dans
`client/`.

## 8. Risques spécifiques au jalon

| Risque | Mitigation |
|---|---|
| str0m : assemblage manuel (ICE/pacing/boucle) plus long que prévu | C'est un choix assumé ; si le jalon s'enlise > quelques semaines sur le transport pur, bascule possible vers webrtc-rs derrière le même découpage de modules |
| Zéro-copie D3D11 → Media Foundation délicate | Étape intermédiaire acceptée : copie CPU d'abord (fonctionnel), optimisation zéro-copie ensuite ; le critère 60 fps reste le juge |
| Latence < 50 ms non atteinte | L'overlay identifie le maillon (capture/encode/réseau/décodage) ; budget par étage : capture ≤ 5 ms, encode ≤ 10 ms, réseau LAN ≤ 5 ms, jitter buffer + décodage ≤ 20 ms |
| Owned windows (menus Firefox) mal capturées | Périmètre : fenêtre principale seule pour ce jalon ; les menus natifs hors fenêtre sont un défaut connu et accepté, traité au sous-projet suivant |
