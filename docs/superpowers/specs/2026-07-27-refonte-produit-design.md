# Refonte produit — Streaming d'applications Windows dans le navigateur

**Date** : 27 juillet 2026
**Statut** : Spécification d'architecture validée
**Portée** : Refonte globale du projet « Guacamole » en produit multi-tenant

---

## 1. Contexte et motivation

Le système actuel streame des applications Windows dans le navigateur via Apache
Guacamole (guacd + RDP RemoteApp), avec un pont fichiers FUSE ↔ WebSocket et une
découverte d'apps par WinRM. Il fonctionne, mais atteint ses limites
structurelles :

- **Fiabilité** : crash récurrent `double free or corruption` dans la chaîne
  guacd/fuse-native (7 core dumps à la racine du projet en attestent), guacd
  unique partagé entre toutes les sessions, aucune reconnexion, `fuse-native`
  non maintenu.
- **Performance** : rendu par instructions Guacamole sur canvas (pas de décodage
  vidéo matériel), DPI forcé à 500, scaling client approximatif, pipeline de
  build Babel→ES5/Browserify daté.
- **Intégration** : RDP RemoteApp bride l'intégration fenêtre ; icônes basse
  résolution ; découverte d'apps par polling horaire.

## 2. Objectifs

1. **Rapide et performant** : latence de classe Parsec/Moonlight, décodage vidéo
   matériel dans le navigateur, 60 fps sur contenu animé.
2. **Fiable** : plus de composant natif fragile, isolation complète entre
   sessions, reconnexion transparente.
3. **Intégration navigateur parfaite** : la fenêtre de l'app Windows EST la
   fenêtre PWA ; technologies navigateur exploitées au maximum (WebRTC,
   WebCodecs, File System Access, Window Controls Overlay, PWA).
4. **Installation d'apps depuis le navigateur** par upload d'installeur.
5. **Produit multi-tenant** : une VM Windows dédiée par utilisateur.
6. **Refonte visuelle complète** : direction sobre « produit pro ».

## 3. Décisions actées

| Décision | Choix | Justification |
|---|---|---|
| Architecture | Agent custom + WebCodecs/WebRTC (remplace guacd/RDP/FUSE/WinRM) | Seule option atteignant les trois objectifs simultanément |
| Langage agent | **Rust** (windows-rs) | Pas de GC sur le chemin de capture, fiabilité mémoire (élimine la classe de bugs « double free »), binaire unique |
| Isolation | VM dédiée par utilisateur | Indispensable : exécution d'installeurs arbitraires uploadés |
| Installation d'apps | Upload `.exe`/`.msi` depuis le navigateur | Choix produit |
| Direction visuelle | Sobre « pro » (référence Linear/Vercel) | Crédibilité produit B2B |
| Client web | Vite + TypeScript | Build moderne, HMR, remplace Gulp/Babel/Browserify |
| Plateforme | Node.js/TypeScript, SQLite → Postgres | Continuité avec l'existant, état persistant (fini les sessions en mémoire) |

## 4. Architecture d'ensemble

```
┌─────────── Navigateur (PWA par app) ───────────┐
│  <video>/WebCodecs   Input   Clipboard   FSA   │
└──────────────────┬─────────────────────────────┘
                   │ WebRTC (piste vidéo + data channels)
┌──────────────────┴──────────────┐
│    Plateforme (Node.js/TS)      │  auth, catalogue, provisioning VMs,
│    signaling + orchestration    │  upload d'installeurs, TURN
└──────────────────┬──────────────┘
                   │ canal de contrôle (gRPC ou WS)
┌──────────────────┴──────────────┐
│   VM Windows dédiée (1/user)    │
│   Agent Rust (service) :        │
│   • Windows.Graphics.Capture    │  capture PAR FENÊTRE
│   • Media Foundation (H.264/AV1)│  encodage matériel
│   • SendInput                   │  injection input
│   • ProjFS                      │  lecteur virtuel « Mes Fichiers »
│   • Exécution installeurs       │
│   • Découverte apps + icônes HD │
└─────────────────────────────────┘
```

**Principes de fiabilité par conception** :

1. Un agent par VM, une VM par utilisateur — aucune ressource partagée entre
   sessions.
2. Zéro dépendance native non maintenue — FUSE et WinRM disparaissent ; un seul
   protocole versionné entre agent, plateforme et navigateur.
3. Reconnexion native — la session Windows survit à une coupure réseau ; la PWA
   se rattache au flux via renégociation WebRTC.
4. Une panne du canal fichiers ne touche jamais le flux vidéo (canaux
   indépendants).

## 5. Sous-projets

### ① Agent Windows (Rust)

- **Capture** : `Windows.Graphics.Capture` ciblée sur la fenêtre de
  l'application (et ses owned windows en v1 pour les popups/menus). Le resize de
  la fenêtre PWA pilote le redimensionnement de la fenêtre Windows.
- **Encodage** : Media Foundation, H.264 partout, AV1 si GPU compatible. Débit
  adaptatif piloté par les stats WebRTC.
- **Input** : injection via `SendInput` (souris, clavier, molette, touch),
  support IME correct.
- **Divers** : couleur d'accent de la fenêtre envoyée par data channel (plus de
  `getImageData` côté client), presse-papier bidirectionnel.
- **Service Windows** : démarrage automatique, watchdog, mise à jour pilotée par
  la plateforme.

### ② Client web (Vite + TypeScript)

- Réception du flux : piste vidéo WebRTC (décodage matériel natif) ; WebCodecs
  en option ultérieure si un contrôle plus fin de la latence s'avère nécessaire.
- Input capturé et envoyé sur data channel non-fiable/non-ordonné.
- PWA par application : manifest dynamique, file handlers, Window Controls
  Overlay, service worker.
- Deux surfaces : **hub** (catalogue, upload, compte) et **fenêtre de session**.

### ③ Pont fichiers

- Windows : lecteur virtuel via **ProjFS** exposé comme « Mes Fichiers ».
- Navigateur : File System Access API, avec lectures par plage et cache
  write-back flushé avant fin de session.
- Erreurs propres : canal coupé → erreur I/O standard côté Windows, session
  vidéo intacte.

### ④ Gestion d'apps

- **Upload d'installeur** : navigateur → plateforme → agent → exécution dans la
  VM de l'utilisateur, progression en temps réel.
- **Découverte** : l'agent surveille Desktop + Menu Démarrer (notifications de
  changement, plus de polling horaire ni de parsing `.lnk` maison).
- **Icônes** : extraction Shell native 256×256 par l'agent.
- Chaque app découverte devient une PWA installable.

### ⑤ Plateforme

- **Auth** : email + mot de passe (OIDC prévu), sessions JWT.
- **Orchestration VMs** : interface abstraite (start/stop/snapshot/assign) ;
  backend v1 = inventaire statique des VMs existantes ; extensible
  Proxmox/Hyper-V/cloud.
- **Signaling WebRTC** + serveur TURN pour les réseaux restrictifs.
- **Persistance** : SQLite en dev, Postgres en prod — utilisateurs, VMs, apps,
  sessions. Scalabilité horizontale possible (plus d'état en mémoire).

### ⑥ Design system

- Direction sobre « pro » : fond neutre, typographie soignée, accents discrets,
  densité d'information, thème clair/sombre.
- Tokens de design partagés entre hub et fenêtre de session.
- Appliqué en continu au fil des sous-projets.

## 6. Flux principaux

**Lancement d'app** : hub → clic sur l'app → la plateforme vérifie/démarre la VM
de l'utilisateur → l'agent lance l'app et démarre la capture → signaling WebRTC
→ la PWA affiche le flux. Cible : < 3 s si VM chaude.

**Installation d'app** : glisser-déposer de l'installeur dans le hub → upload →
transfert à l'agent → exécution → détection automatique des nouveaux raccourcis
→ l'app apparaît dans le hub en quelques secondes.

**Reconnexion** : coupure réseau → la session Windows et l'app restent vivantes
(délai de grâce configurable, 5 min par défaut) → la PWA retente et renégocie →
reprise sans perte d'état applicatif.

## 7. Gestion des erreurs

| Panne | Comportement |
|---|---|
| Agent crash | Watchdog service le redémarre ; la PWA affiche « reconnexion… » puis se rattache |
| Canal fichiers coupé | Erreurs I/O propres côté Windows ; vidéo intacte |
| VM injoignable | Le hub l'indique, propose redémarrage via l'orchestrateur |
| Installeur échoue | Code de sortie + logs remontés dans l'interface d'upload |
| Plateforme redémarre | Sessions persistées en base ; les PWA renégocient |

## 8. Stratégie de test

- **Agent** : tests unitaires Rust (protocole, état) ; tests d'intégration sur
  VM Windows réelle (capture, input, ProjFS) via CI auto-hébergée.
- **Client web** : tests unitaires (Vitest) ; tests E2E Playwright contre un
  agent simulé (mock server reproduisant le protocole).
- **Protocole** : schéma versionné partagé (source de vérité unique), tests de
  compatibilité générés.
- **Plateforme** : tests d'API ; tests de charge sur le signaling.

## 9. Ordre de construction

1. **Tranche verticale ① + ②** : une app Windows streamée dans le navigateur
   avec input, sur une VM existante, sans auth. C'est le jalon qui valide tout
   le pari technique (capture, encodage, WebRTC, latence).
2. **③ Pont fichiers** (ProjFS ⇆ FSA).
3. **④ Gestion d'apps** (upload, découverte, icônes).
4. **⑤ Plateforme** (auth, orchestration, persistance).
5. **⑥ Style** : tokens posés dès le début, appliqué en continu.

Chaque sous-projet suit son propre cycle spec → plan → implémentation.

## 10. Risques et mitigations

| Risque | Mitigation |
|---|---|
| Capture par fenêtre : popups/menus/dialogues mal couverts | v1 : fenêtre racine + owned windows ; fallback bascule en capture de bureau recadrée si nécessaire |
| Encodage matériel indisponible (VM sans GPU) | Fallback encodage logiciel (openh264/x264) à résolution/fps réduits |
| ProjFS complexe | Périmètre v1 réduit (read/write/list/rename) ; le reste ensuite |
| Compétence Rust/APIs Windows | La tranche verticale (jalon 1) valide ou invalide tôt ; l'existant Guacamole reste utilisable pendant la construction |
| WebRTC à travers NAT/firewalls | TURN inclus dès la v1 |

## 11. Hors périmètre (explicitement)

- Multi-moniteurs, enregistrement de session, collaboration multi-utilisateurs
  sur une même session, impression → v2+.
- Migration des données de l'ancien système : aucune (pas de données à
  migrer).
- L'ancien code (guacd/FUSE/WinRM) n'est pas modifié ; il reste fonctionnel
  jusqu'au remplacement, puis sera supprimé.
