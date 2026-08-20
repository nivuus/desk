# Sous-projet ③ Pont fichiers — sous-blocs F0 et F1 : plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Activer ProjFS sur la VM (**F0**), puis livrer la tranche verticale
minimale (**F1**) : un lecteur `%USERPROFILE%\Mes Fichiers` **en lecture seule**
dont le contenu est un répertoire du poste local ouvert par
`showDirectoryPicker({ mode: 'read' })` dans la page-shell.

**Architecture:** Un **troisième type de processus** `agent.exe` (`PONT=1`),
lancé et surveillé par le superviseur exactement comme le capteur l'est, tient
la racine de virtualisation ProjFS. Il parle à la page-shell par une
`RTCPeerConnection` **dédiée, sans média**, portant un unique data channel
`fichiers` fiable et ordonné. Les entrées `ProjectedFSLib.dll` sont résolues à
l'**exécution** (`LoadLibraryW` + `GetProcAddress`), jamais liées statiquement :
c'est ce qui fait qu'une VM sans ProjFS perd le pont et **rien d'autre**.

**Tech Stack:** Rust (crate `agent` et crate `proto`, cible réelle
`x86_64-pc-windows-msvc` sur la VM), TypeScript côté `client/` et `proto/ts/`,
PowerShell distant via `scripts/winrm.js`.

**Spec :** `docs/superpowers/specs/2026-08-19-pont-fichiers-design.md`
(commit `06619fc`).

**Périmètre strict de ce plan : F0 et F1, et rien d'autre.** F2 (écriture,
write-back, journal de reprise, `beforeunload`), F3 (renommage, suppression, la
table complète des HRESULT, les trois budgets de délai, le contrôle de flux),
F4 (le banc de latence `PONT_MESURE`) et F5 (`Rafraichir`, la vie longue) sont
**hors de ce plan**. Là où F1 doit poser une amorce pour eux, la tâche le dit et
s'arrête là.

---

## Global Constraints

### Références relevées, par la commande, le 19 août 2026

Tous les nombres ci-dessous ont été **exécutés**, pas recopiés. Ils sont la
référence d'entrée : tout écart en cours de branche est un défaut, pas une
dérive.

| Contrôle | Commande | Relevé |
| --- | --- | --- |
| Tests d'hôte Rust | `cd agent && cargo test -p agent` | **467 passed, 0 failed** |
| Compilation croisée Windows | `cd agent && cargo check --target x86_64-pc-windows-gnu` | sortie 0, **9 avertissements** (tous `dead_code`) |
| Tests client | `cd client && npx vitest run` | **12 fichiers, 107 passed** |
| Tests du protocole partagé | `cd client && npx vitest run --dir ../proto` | **2 fichiers, 35 passed** |
| Plafond de 500 lignes | la commande de `CLAUDE.md` § Conventions | **deux** fichiers au-dessus : `agent/src/encode.rs` **1536**, `agent/src/windows_source.rs` **630** |

> 🔴 **`cd client && npx vitest run` NE COUVRE PAS `proto/ts/`.** La racine
> Vitest est `client/`, et `proto/ts/*.test.ts` est en dehors : les 107 tests
> sont ceux de `client/src/` seuls (12 fichiers). Les 35 tests de
> `proto/ts/control.test.ts` et `input.test.ts` ne tournent que par
> `npx vitest run --dir ../proto`. **Un test ajouté à `proto/ts/fichiers.test.ts`
> ne serait donc jamais exécuté par la commande de référence du dépôt** — et un
> test qui ne tourne pas n'est pas un test. Les deux commandes sont exigées à
> chaque tâche qui touche `proto/ts/`. *(Relevé et vérifié par exécution ce
> jour ; aucun document du dépôt ne le signalait.)*
> `cd client && npx tsc --noEmit` couvre en revanche bien `proto/ts/`
> (`client/tsconfig.json`, `include: ["src/**/*.ts", "../proto/ts/**/*.ts"]`).

### Règles de travail

- **Plafond de 500 lignes** par fichier de code source écrit à la main.
  **Aucune extraction préalable n'est requise sur les fichiers existants
  touchés** — leurs marges sont relevées au § « Structure des fichiers » et la
  plus étroite est de 109 lignes. **Mais deux fichiers reçoivent leur addition
  dans un module ENFANT plutôt qu'en ligne**, et c'est une décision de ce plan,
  pas une réaction : `superviseur/lanceur.rs` (367) reçoit `lanceur/pont.rs`, et
  `superviseur/boucle.rs` (263) reçoit `boucle/surveillance_pont.rs`.
  L'extraction se place **avant** l'addition qui la rend nécessaire — c'est le
  seul geste qui ait fonctionné dans ce dépôt (D9, `capteur/serveur/instances.rs`,
  marge rendue de 10 à 65 ; les deux fichiers traités après coup en D9 ont été
  **compressés**, geste que `CLAUDE.md` interdit nommément, puis extraits quand
  même).
- **Convention de module enfant** (`CLAUDE.md`, § « Convention de module
  enfant… ») : **aucun module de ce plan n'en relève.** Elle ne s'applique
  qu'aux modules qu'on extrait d'un parent `#[cfg(windows)]` pour les faire
  compiler sur l'hôte, et qui doivent de ce fait devenir frères de premier
  niveau dans `main.rs`. Ici : `agent/src/pont.rs` n'est **pas** gaté, ses
  enfants purs (`table`, `decoupe`, `chemins`, `erreurs`, `transport`) se
  déclarent par un simple `mod` à l'intérieur de lui, et `pont/projfs.rs` est
  gaté sans rien avoir à hisser. `lanceur/pont.rs` est l'enfant ordinaire d'un
  `lanceur.rs` qui porte déjà `#![cfg(windows)]` (`agent/src/superviseur/lanceur.rs:36`)
  et le lui transmet. **Aucun `#[path]` n'est écrit par ce plan.**
- **Jamais `git add -A`** : nommer les fichiers. Un `git add -A` a déjà emporté
  le travail concurrent d'une autre tâche dans un commit qui ne compilait pas.
- **Un contrôle qu'on n'a jamais vu rouge n'est pas un contrôle.** Chaque test
  neuf doit être exécuté **avant** l'implémentation et vu échouer ; le message
  d'échec attendu est écrit dans la tâche. Ce plan est lui-même une source de
  contrôles vacueux — D10 en a attrapé quatre, **dont trois écrits par son
  propre plan**, et un cinquième a été trouvé en recette D11. Chaque contrôle
  ci-dessous porte donc explicitement son état rouge et **comment l'atteindre**.
- **`PONT` doit être ajoutée explicitement à `scripts/run-agent.sh`**, dans une
  **tâche dédiée placée avant celle qui en a besoin** — piège payé en D1
  (`SUPERVISEUR`), D2 (`MULTIFENETRE_REPRISE`) et D7 (`AUDIO`), évité en D3 et
  D6 par exactement ce geste. L'agent démarre sans la variable **et sans rien
  signaler**.
- **Convention de valeur des variables de mode** : `PONT=0` **désactive**, et le
  test est `matches!(std::env::var("PONT").as_deref(), Ok(v) if v != "0")` —
  **la forme exacte de `agent/src/main.rs:315` pour `CAPTEUR`**, citée par la
  spec §3.2. ⚠️ **La glose de la spec (« une simple présence n'active pas ») est
  inexacte pour cette forme** : une présence dont la valeur n'est pas `"0"`
  active bel et bien, y compris une valeur vide. Ce qu'elle interdit
  réellement — et c'est le piège d'exploitation que le dépôt nomme —, c'est
  `is_ok()`, qui ferait qu'écrire `PONT=0` pour couper le pont l'allumerait.
  **C'est le CODE de `main.rs:315` qui fait foi, pas la glose.**
- **La VM n'est pas démarrée automatiquement.** Avant toute tâche de recette :
  `virsh list --all`, `virsh start Windows`, attendre WinRM **puis** un accès
  réel à `/media/vm` (`until ls /media/vm/dev`), et `set -a && source .env && set +a`
  avant `scripts/build-agent.sh` — sans quoi il s'arrête **en silence** après
  « sources synchronisées ».
- **La VM est un état partagé.** Les tâches 1, 18 et toute reprise de recette
  sont **sérialisées entre elles**, quelle que soit leur indépendance logique.
- **Aucun taux ne sera revendiqué.** Chaque énoncé de résultat porte son nombre
  d'exécutions. **Deux exécutions par critère**, jamais une.
- **Toute preuve d'une affirmation portée dans `CLAUDE.md` est versée dans git**,
  sous `docs/superpowers/plans/journaux-pont-fichiers/`. D9 a perdu **six**
  constats de revue parce que sa preuve vivait dans `.superpowers/sdd/`,
  gitignoré et jamais commité : ils sont définitivement perdus.

---

## Structure des fichiers

### Créés

| Fichier | Nature | Visé | Responsabilité |
| --- | --- | --- | --- |
| `proto/src/fichiers.rs` | **PUR** | ≤ 400 | Trame binaire v1, `FICHIERS_VERSION`, types de messages |
| `proto/ts/fichiers.ts` | **PUR** | ≤ 250 | Le jumeau TypeScript |
| `proto/ts/fichiers.test.ts` | test | ≤ 200 | ⚠️ ne tourne que par `--dir ../proto` |
| `agent/src/pont.rs` | assemblage | ≤ 200 | Déclare les modules, `executer()` — il assemble, il ne décide pas |
| `agent/src/pont/chemins.rs` | **PUR** | ≤ 250 | Normalisation ProjFS → chemin logique |
| `agent/src/pont/erreurs.rs` | **PUR** | ≤ 150 | `Erreur → HRESULT` |
| `agent/src/pont/decoupe.rs` | **PUR** | ≤ 150 | Plage `(offset, length)` → trames bornées |
| `agent/src/pont/table.rs` | **PUR** | ≤ 350 | Commandes en vol, corrélation, expiration, annulation, sessions d'énumération |
| `agent/src/pont/transport.rs` | mixte, **sans Windows** | ≤ 300 | signaling + str0m **données seules** |
| `agent/src/pont/projfs.rs` | `#[cfg(windows)]` | ≤ 400 | Racine, virtualisation, les huit rappels |
| `agent/src/pont/projfs/chargement.rs` | `#[cfg(windows)]` | ≤ 200 | `LoadLibraryW` + les **treize** transcriptions |
| `agent/src/superviseur/lanceur/pont.rs` | `#[cfg(windows)]` hérité | ≤ 150 | `lancer_pont`, `pont_vivant`, `tuer_pont` |
| `agent/src/superviseur/boucle/surveillance_pont.rs` | `#[cfg(windows)]` hérité | ≤ 200 | Jumeau de `surveillance_capteur.rs` |
| `client/src/fichiers/protocole.ts` | **PUR** | ≤ 250 | Encodage/décodage, corrélation — sans DOM, sans FSA |
| `client/src/fichiers/adaptateur.ts` | adaptateur FSA | ≤ 300 | `FileSystemDirectoryHandle` → réponses ; injecté, donc testable par un faux système de fichiers |
| `client/src/fichiers/canal.ts` | données seules | ≤ 250 | `RTCPeerConnection` sans média, canal `fichiers` |
| `client/src/fichiers/protocole.test.ts`, `adaptateur.test.ts` | tests | ≤ 300 chacun | Vitest, faux système de fichiers en mémoire |
| `docs/superpowers/plans/2026-08-19-pont-fichiers-f1-resultats.md` | document | — | Le document de résultats permanent |
| `docs/superpowers/plans/journaux-pont-fichiers/` | journaux | — | Les pièces versées |

### Modifiés — tailles **relevées par la commande le 19 août 2026**

| Fichier | Lignes | Marge | Ce que ce plan y ajoute |
| --- | --- | --- | --- |
| `agent/Cargo.toml` | — | — | +1 fonctionnalité `Win32_Storage_ProjectedFileSystem` |
| `proto/src/lib.rs` | **4** | — | +1 `pub mod fichiers;` |
| `agent/src/main.rs` | **328** | 172 | +1 branche `PONT` (forme de `:315`), +1 `mod pont;` |
| `agent/src/superviseur.rs` | **77** | 423 | rien — `lanceur` et `boucle` déclarent leurs propres enfants |
| `agent/src/superviseur/lanceur.rs` | **367** | 133 | `mod pont;` + `env_remove("PONT")` en **deux** endroits (`:158`ss et `:229`ss). **L'essentiel va dans `lanceur/pont.rs`** |
| `agent/src/superviseur/boucle.rs` | **263** | 237 | `mod surveillance_pont;` + le câblage (≈ 15 lignes) |
| `agent/src/superviseur/protocole.rs` | **110** | 390 | +1 constante `SESSION_DU_PONT` |
| `agent/src/transport/evenements.rs` | ❌ ~~**391**~~ **279** | ❌ ~~109~~ 221 | tâche 17 seulement — l'aiguillage par canal, et ses tests. ⚠️ **Le 391 n'a JAMAIS été vrai** : le fichier en faisait 279 quand ce plan a été écrit, et 363 après la tâche 17. Le `wc -l … # attendu : 391` du Step 3 de la tâche 17 s'appuyait dessus |
| `client/src/shell.ts` | **93** | 407 | l'état « lecteur monté », injecté comme le reste |
| `client/src/shell-page.ts` | **71** | 429 | le bouton, le second WebSocket, le câblage |
| `client/shell.html` | **14** | — | +1 bouton, +1 zone d'état |
| `scripts/run-agent.sh` | **126** | — | +1 ligne `PONT` |
| `CLAUDE.md` | — | — | tâche 19 |

> ⚠️ **`agent/src/superviseur/lanceur.rs` (marge 133) est le fichier le plus
> exposé de ce plan.** `lancer_capteur` occupe `:156-188` et `capteur_vivant`
> `:198-…` : leurs jumeaux, écrits dans le style documentaire de ce fichier,
> pèsent 80 à 100 lignes. Les écrire en ligne mènerait à ~460, marge ~40 — la
> configuration exacte que ce dépôt a payée quatre fois (« la marge regagnée par
> une extraction se reperd à la ronde suivante si on la traite comme acquise »).
> **D'où `lanceur/pont.rs` dès la tâche 8, et non après.**

---

## Interfaces partagées

Ces signatures sont **fixées ici** : les tâches les consomment telles quelles.
Toute divergence constatée à l'implémentation est un défaut du plan **à
signaler, pas à recopier**.

```rust
// proto/src/fichiers.rs
pub const FICHIERS_VERSION: u8 = 1;
pub const TAILLE_TRAME_MAX: usize = 64 * 1024;   // NON CALIBRÉE — voir §4.2 de la spec

pub struct Trame<'a> {
    pub version: u8,
    pub type_message: u8,
    pub correlation: u32,
    pub entete: &'a [u8],      // JSON UTF-8
    pub charge: &'a [u8],      // octets bruts, JAMAIS encodés
}
pub fn encoder(type_message: u8, correlation: u32, entete: &str, charge: &[u8]) -> Vec<u8>;
pub fn decoder(octets: &[u8]) -> Result<Trame<'_>, ErreurTrame>;

// Requêtes pont → navigateur (v1)
pub const TYPE_LISTER: u8 = 1;
pub const TYPE_ATTRIBUTS: u8 = 2;
pub const TYPE_LIRE: u8 = 3;
// Réponses navigateur → pont (v1)
pub const TYPE_ENTREES: u8 = 64;
pub const TYPE_META: u8 = 65;
pub const TYPE_DONNEES: u8 = 66;
pub const TYPE_ECHEC: u8 = 127;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CodeEchec {
    Introuvable, CheminIntrouvable, AccesRefuse, ProtegeEnEcriture,
    NonSupporte, TropGrand, Interne,
}

// agent/src/pont/chemins.rs   — PUR
pub fn normaliser(brut: &str) -> Result<String, CheminRefuse>;
pub enum CheminRefuse { Remontee, FluxAlternatif, NomReserve, Absolu, Vide, NonUtf16Valide }

// agent/src/pont/erreurs.rs   — PUR
pub enum Erreur {
    Introuvable, CheminIntrouvable, AccesRefuse, CanalFerme, DelaiDepasse,
    Abandonnee, DisquePlein, NonSupporte, RepertoireNonVide, DejaPresent,
    ProtegeEnEcriture, Inattendue,
}
/// `HRESULT_FROM_WIN32(x) = 0x8007_0000 | x`. Rend un `i32` : c'est ce que
/// `windows_core::HRESULT` enveloppe, et le module reste PUR (aucun `windows`).
pub fn hresult(e: Erreur) -> i32;

// agent/src/pont/decoupe.rs   — PUR
pub struct Morceau { pub position: u64, pub longueur: u32 }
pub fn decouper(position: u64, longueur: u64, max: usize) -> Vec<Morceau>;

// agent/src/pont/table.rs     — PUR
pub struct Table { /* … */ }
pub enum Attendue { Attributs { chemin: String }, Lire { chemin: String, position: u64, longueur: u32 },
                    Lister { chemin: String, enumeration: [u8; 16] } }
impl Table {
    pub fn nouvelle() -> Self;
    /// Inscrit une commande et rend son identifiant de corrélation, monotone.
    pub fn inscrire(&mut self, command_id: i32, quoi: Attendue, echeance: std::time::Instant) -> u32;
    /// Rend `None` si la commande a été annulée, expirée, ou n'a jamais existé —
    /// la réponse tardive est alors JETÉE, jamais appliquée.
    pub fn resoudre(&mut self, correlation: u32) -> Option<(i32, Attendue)>;
    pub fn annuler(&mut self, command_id: i32) -> Option<u32>;
    /// Retire et rend tout ce qui est échu à `maintenant`.
    pub fn expirees(&mut self, maintenant: std::time::Instant) -> Vec<(i32, u32)>;
    /// Retire et rend TOUT : appelée avant `PrjStopVirtualizing`.
    pub fn vider(&mut self) -> Vec<(i32, u32)>;
    pub fn en_vol(&self) -> usize;
}

// agent/src/pont/transport.rs
pub enum VersNavigateur { Requete { correlation: u32, trame: Vec<u8> } }
pub enum DuNavigateur { Reponse { correlation: u32, trame: Vec<u8> }, CanalOuvert, CanalFerme }
/// Boucle de transport données seules. `sortant` porte les requêtes, `entrant`
/// rend les réponses. Ne connaît NI ProjFS NI Windows.
pub fn tourner(
    rtc: str0m::Rtc,
    socket: std::net::UdpSocket,
    sortant: std::sync::mpsc::Receiver<VersNavigateur>,
    entrant: std::sync::mpsc::Sender<DuNavigateur>,
) -> anyhow::Result<()>;
pub fn construire_rtc_donnees(local_ip: std::net::IpAddr)
    -> anyhow::Result<(std::net::UdpSocket, str0m::Rtc)>;

// agent/src/superviseur/protocole.rs
pub const SESSION_DU_PONT: &str = "fichiers";

// agent/src/superviseur/lanceur/pont.rs
impl super::LanceurDeProcessus {
    /// Même contrat ATOMIQUE que `lancer_capteur` : `Err` signifie qu'aucun
    /// processus ne tourne.
    pub fn lancer_pont(&self) -> anyhow::Result<u32>;
    pub fn pont_vivant(&self) -> bool;
}
```

```ts
// client/src/fichiers/protocole.ts        — PUR, sans DOM
export const FICHIERS_VERSION = 1;
export interface Trame { version: number; type: number; correlation: number;
                         entete: unknown; charge: Uint8Array }
export function encoder(type: number, correlation: number, entete: unknown,
                        charge?: Uint8Array): ArrayBuffer;
export function decoder(octets: ArrayBuffer): Trame;   // lève si version ≠ FICHIERS_VERSION

// client/src/fichiers/adaptateur.ts       — la FSA est INJECTÉE
export interface Racine { getDirectoryHandle(...): Promise<any>;
                          getFileHandle(...): Promise<any>;
                          values(): AsyncIterable<any> }
export function creerAdaptateur(racine: Racine): {
    lister(chemin: string): Promise<Entree[]>;
    attributs(chemin: string): Promise<Meta>;
    lire(chemin: string, position: number, longueur: number): Promise<Uint8Array>;
};

// client/src/fichiers/canal.ts
export function connecterCanalFichiers(options: {
    signalingUrl: string; sessionId: string;
    onStatus?: (m: string) => void;
}): Promise<{ pc: RTCPeerConnection; canal: RTCDataChannel; close(): void }>;
```

---

# Famille F0 — le préalable, avant tout code

### Task 1 : activer ProjFS sur la VM, en montrant d'abord le rouge

**Files:**
- Create: `docs/superpowers/plans/journaux-pont-fichiers/f0-avant.txt`,
  `f0-activation.txt`, `f0-apres.txt`

**Interfaces:**
- Consumes: rien.
- Produces: le verdict F0. **Aucun autre travail n'a de sens avant** (spec §10,
  R1 : « **Éliminatoire** […] Repli s'il échoue : **aucun** »).

⚠️ **Cette tâche est conduite par le propriétaire du dépôt ou l'orchestrateur,
avec son accord explicite — accord DONNÉ le 19 août 2026.** Elle modifie l'état
de la VM et peut la redémarrer. Aucun agent d'implémentation ne l'exécute de sa
propre initiative.

- [ ] **Step 1 : préparer la VM**

```bash
virsh list --all
virsh start Windows   # si « fermé »
until timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null; do sleep 5; done
until ls /media/vm/dev >/dev/null 2>&1; do sleep 5; done
set -a && source .env && set +a
```

- [ ] **Step 2 : relever l'état ROUGE, et le VERSER**

```bash
node scripts/winrm.js "\
  (Get-WindowsOptionalFeature -Online -FeatureName Client-ProjFS | \
     Format-List FeatureName,State,RestartRequired | Out-String); \
  'ProjectedFSLib.dll : ' + (Test-Path \$env:SystemRoot\\system32\\ProjectedFSLib.dll); \
  'PrjFlt.sys        : ' + (Test-Path \$env:SystemRoot\\system32\\drivers\\PrjFlt.sys); \
  (Get-Service PrjFlt -ErrorAction SilentlyContinue | Format-List Name,Status | Out-String)" \
  | tee docs/superpowers/plans/journaux-pont-fichiers/f0-avant.txt
```

**Attendu — et c'est l'ÉTAT ROUGE, constaté, pas supposé** (relevé du 19 août
2026, spec §2 et brief de ce plan) :

```
State           : Disabled
RestartRequired : Possible
ProjectedFSLib.dll : False
PrjFlt.sys        : False
```

⚠️ **C'est ce relevé, et lui seul, qui rend le critère de F0 falsifiable.** Sans
lui, le vert d'après ne prouve rien : un contrôle qu'on n'a jamais vu rouge
n'est pas un contrôle. **Le verser est obligatoire** — c'est la pièce, et elle
doit vivre dans git.

- [ ] **Step 3 : activer**

```bash
node scripts/winrm.js \
  "Enable-WindowsOptionalFeature -Online -FeatureName Client-ProjFS -NoRestart | \
   Format-List Path,Online,RestartNeeded | Out-String" \
  | tee docs/superpowers/plans/journaux-pont-fichiers/f0-activation.txt
```

⚠️ **Ne PAS employer `Install-WindowsFeature Projected-File-System`** : sur
Windows Server 2022 le nom vit au catalogue des fonctionnalités facultatives et
**pas** au gestionnaire de rôles (`Get-WindowsFeature` ne connaît aucun `Proj*`,
spec §2). Un tel script **échouerait en silence**.

⚠️ Élévation requise : le compte `WINDOWS_ADMIN_USERNAME` de `.env` en dispose
(élévation `True`, relevée au brief).

- [ ] **Step 4 : décider du redémarrage SUR LE RETOUR, pas d'avance**

Si `RestartNeeded : True` :

```bash
node scripts/winrm.js 'Restart-Computer -Force'
until timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null; do sleep 5; done
until ls /media/vm/dev >/dev/null 2>&1; do sleep 5; done
```

⚠️ **`RestartRequired : Possible` du Step 2 est ce que le CATALOGUE annonce
avant activation, pas ce que l'activation exigera** (spec §2, fait n°3). Ne rien
déduire de lui.

- [ ] **Step 5 : relever le VERT, depuis un état neuf**

```bash
node scripts/winrm.js "\
  (Get-WindowsOptionalFeature -Online -FeatureName Client-ProjFS | \
     Format-List FeatureName,State | Out-String); \
  'ProjectedFSLib.dll : ' + (Test-Path \$env:SystemRoot\\system32\\ProjectedFSLib.dll); \
  'PrjFlt.sys        : ' + (Test-Path \$env:SystemRoot\\system32\\drivers\\PrjFlt.sys); \
  (Get-Service PrjFlt | Format-List Name,Status,StartType | Out-String); \
  (fltmc filters | Out-String)" \
  | tee docs/superpowers/plans/journaux-pont-fichiers/f0-apres.txt
```

**Critère de réception F0, et il porte TROIS conditions, pas deux :**

1. `State : Enabled` ;
2. `ProjectedFSLib.dll : True` **et** `PrjFlt.sys : True` ;
3. **le filtre est réellement chargé** — `fltmc filters` mentionne `PrjFlt`, ou
   `Get-Service PrjFlt` rend `Running`.

⚠️ **La troisième condition n'est pas dans la spec, et elle est ajoutée ici avec
sa raison** : la spec §2.1 s'arrête à la présence du fichier, et §2 dit
lui-même que l'absence conjointe des deux fichiers est « une corroboration, pas
une preuve ». Un pilote de mini-filtre **présent sur le disque et non chargé**
ferait échouer `PrjStartVirtualizing` à l'exécution, très loin d'ici, avec un
`HRESULT` que personne ne rattacherait à F0. **Le coût de la condition est une
ligne ; le coût de son absence est une tâche 13 inexplicablement rouge.**

- [ ] **Step 6 : inscrire l'exigence dans le PROVISIONNEMENT, pas dans l'agent**

Porter la ligne d'activation là où l'image de base de la VM est décrite. **Un
agent qui activerait une fonctionnalité Windows et redémarrerait la machine sous
l'utilisateur est un comportement que rien dans le cadrage n'autorise** (spec
§2.1). Si aucun tel endroit n'existe encore dans le dépôt — c'est le sous-projet
⑤ —, **le dire dans le document de résultats et l'inscrire comme legs**, jamais
le cacher dans `pont.rs`.

- [ ] **Step 7 : si l'activation ÉCHOUE**

**Il n'y a aucun repli** (spec §10, R1). Le sous-projet ③ est alors **non
livrable**, et le geste correct est : verser le journal d'échec, écrire le
document de résultats avec ce seul verdict, **et s'arrêter**. Ne pas entamer les
tâches 2 et suivantes « en attendant » : elles produiraient du code que rien ne
pourrait jamais exercer.

- [ ] **Step 8 : commit**

```bash
git add docs/superpowers/plans/journaux-pont-fichiers/
git commit -m "recette(pont-fichiers): F0, ProjFS active, et le rouge d'avant verse" \
  -- docs/superpowers/plans/journaux-pont-fichiers/
```

---

# Famille A — le protocole et les modules purs

Ces cinq tâches sont **entièrement testables sur l'hôte Linux** et
**parallélisables entre elles** : aucune ne dépend d'une autre, aucune ne touche
un fichier commun sauf `proto/src/lib.rs` (tâche 2 seule).

### Task 2 : `proto/{src,ts}/fichiers` v1 — la trame, et sa version

**Files:**
- Create: `proto/src/fichiers.rs`, `proto/ts/fichiers.ts`, `proto/ts/fichiers.test.ts`
- Modify: `proto/src/lib.rs`

**Interfaces:**
- Consumes: rien.
- Produces: tout le bloc `proto/src/fichiers.rs` et `client/src/fichiers/protocole.ts`
  du § « Interfaces partagées ».

- [ ] **Step 1 : lire les DEUX précédents avant d'écrire le troisième**

```bash
sed -n '1,55p' proto/src/control.rs      # CONTROL_VERSION:14, verifie_version:41-52, la note :37-40
sed -n '1,20p;94,100p;143,150p' proto/src/input.rs   # PROTOCOL_VERSION:14, push :98, rejet :145
```

Le format retenu est celui d'**`input`** — binaire — et non celui de `control`.
La raison est un chiffre relevé sur l'ancien pont (spec §4.2) :
`src/file.js:264` sérialise les octets d'une écriture par
`Array.from(buffer.slice(0, length))`, soit ~4 octets transmis par octet utile ;
`web/index.js:653-657` fait pire au retour. **Un protocole de fichiers qui
encode les octets en JSON paie cet ordre de grandeur sur chaque octet de chaque
lecture.**

De `control.rs:37-40`, reprendre la doctrine **et sa note** : pas de `default`
sur la version — un message sans version doit être **rejeté**, jamais
silencieusement complété.

- [ ] **Step 2 : écrire les tests ROUGES, des deux côtés**

`proto/src/fichiers.rs`, module `#[cfg(test)]` :

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `une_trame_sans_version_est_refusee` | trame de 0 octet acceptée |
| `une_trame_de_version_2_est_refusee` | `FICHIERS_VERSION + 1` accepté |
| `un_entete_dont_la_longueur_deborde_la_trame_est_refuse` | longueur d'en-tête `u32::MAX` → panique par tranche hors bornes, ou `Ok` |
| `un_aller_retour_conserve_les_octets_bruts` | un octet `0x00` ou `0xFF` de la charge altéré |
| `une_charge_vide_et_un_entete_vide_sont_licites` | une trame minimale refusée |
| `la_charge_maximale_de_TAILLE_TRAME_MAX_passe` | refus au seuil exact |

`proto/ts/fichiers.test.ts` : les **mêmes six**, plus
`un_octet_encode_par_rust_se_decode_en_ts` sur un vecteur d'octets **écrit en
dur dans les deux fichiers** — c'est la seule façon de voir rouge une divergence
d'endianness entre les deux implémentations, et le format est petit-boutiste des
deux côtés.

**Les voir rouges :**

```bash
cd agent && cargo test -p proto 2>&1 | tail -5      # attendu : erreurs de compilation, `fichiers` n'existe pas
cd client && npx vitest run --dir ../proto 2>&1 | tail -5
```

- [ ] **Step 3 : implémenter, puis vérifier les TROIS commandes**

```bash
cd agent && cargo test -p proto && cargo test -p agent 2>&1 | tail -3   # attendu : 467 passed
cd client && npx vitest run 2>&1 | tail -3                              # attendu : 107 passed (inchangé)
cd client && npx vitest run --dir ../proto 2>&1 | tail -3               # attendu : 35 + les neufs
cd client && npx tsc --noEmit                                           # couvre proto/ts (tsconfig include)
```

⚠️ **La troisième commande est obligatoire** : sans elle, `fichiers.test.ts`
n'est jamais exécuté (voir les Global Constraints). **Un test qui ne tourne pas
n'est pas un test.**

- [ ] **Step 4 : relever la taille et commit**

```bash
wc -l proto/src/fichiers.rs proto/ts/fichiers.ts proto/ts/fichiers.test.ts
```
Si `proto/src/fichiers.rs` dépasse 400, **extraire les tests** vers
`proto/src/fichiers/tests.rs` par `#[path]` — précédent :
`superviseur/table.rs`, qui déclare ainsi ses deux modules de test. **Jamais
comprimer.**

```bash
git add proto/src/fichiers.rs proto/src/lib.rs proto/ts/fichiers.ts proto/ts/fichiers.test.ts
git commit -m "proto(pont-fichiers): la trame binaire v1, et sa version verifiee des deux cotes" \
  -- proto/
```

---

### Task 3 : `agent/src/pont/chemins.rs` — la normalisation, PURE

**Files:** Create: `agent/src/pont/chemins.rs`

**Interfaces:** Produces: `normaliser`, `CheminRefuse`.

- [ ] **Step 1 : les tests ROUGES d'abord**

ProjFS livre `PRJ_CALLBACK_DATA.FilePathName`
(`windows-0.62.2/…/ProjectedFileSystem/mod.rs:162`) : un chemin **relatif à la
racine**, en contre-obliques, sans lettre de lecteur. Ce module le transforme en
chemin logique pour la FSA, ou le refuse.

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `une_remontee_sort_de_la_racine_et_est_refusee` | `a\..\..\secret` accepté — **le test que la spec §4.4 nomme** |
| `une_remontee_deguisee_est_refusee` | `a\.\..\..\x` accepté |
| `un_flux_alternatif_est_refuse` | `fichier.txt:Zone.Identifier` accepté |
| `un_nom_reserve_est_refuse` | `CON`, `PRN`, `NUL`, `AUX`, `COM1`, `LPT1`, et `CON.txt` acceptés |
| `un_chemin_absolu_est_refuse` | `C:\x` ou `\\serveur\part` accepté |
| `la_racine_elle_meme_est_la_chaine_vide_et_est_licite` | `""` refusé — c'est le chemin de l'énumération de la racine |
| `les_contre_obliques_deviennent_des_barres` | `a\b\c` rendu tel quel |
| `la_casse_est_conservee_mais_la_comparaison_ne_l_est_pas` | deux entrées différant par la casse traitées comme distinctes |

⚠️ **La casse est le piège structurel de ce module** : Windows est insensible à
la casse, la File System Access API ne l'est **pas**. `getFileHandle("Rapport.txt")`
échoue là où NTFS aurait ouvert `rapport.txt`. **F1 ne le résout pas** — il le
**documente** et le fait remonter en `Introuvable`. Le remède (une table de
correspondance alimentée par l'énumération) appartient à F3 ou plus tard, et il
est inscrit comme legs. *Le déclarer résolu sans l'avoir mesuré serait exactement
le geste que ce dépôt reproche à ses constantes non calibrées.*

- [ ] **Step 2 : voir rouge, implémenter, revoir vert**

```bash
cd agent && cargo test -p agent chemins 2>&1 | tail -5
```
Rouge attendu au Step 1 : `error[E0433]: failed to resolve: use of undeclared crate or module 'chemins'`.

- [ ] **Step 3 : commit**

```bash
git add agent/src/pont/chemins.rs
git commit -m "pont(f1): la normalisation des chemins, pure et testee sur l'hote" -- agent/src/pont/chemins.rs
```

*(La déclaration `mod chemins;` arrive avec `pont.rs` en tâche 9 ; d'ici là le
fichier n'est compilé par rien. **C'est délibéré et c'est nommé** : ces trois
tâches sont parallélisables précisément parce qu'aucune ne touche le module
parent. La tâche 9 les déclare toutes d'un coup et **c'est elle qui les fait
passer au vert** ; `cargo test` d'ici là ne les voit pas. **Un implémenteur qui
verrait « 467 passed » après la tâche 3 n'aurait donc RIEN vérifié** — il doit
compiler le fichier en le déclarant temporairement, ou attendre la tâche 9. La
tâche 9 porte le contrôle qui le dénonce.)*

---

### Task 4 : `agent/src/pont/erreurs.rs` — la table des codes, PURE

**Files:** Create: `agent/src/pont/erreurs.rs`

**Interfaces:** Produces: `Erreur`, `hresult`.

- [ ] **Step 1 : les valeurs sont RELEVÉES, pas mémorisées**

```bash
grep -n 'ERROR_FILE_NOT_FOUND\|ERROR_PATH_NOT_FOUND\|ERROR_ACCESS_DENIED\|ERROR_IO_DEVICE\|ERROR_SEM_TIMEOUT\|ERROR_OPERATION_ABORTED\|ERROR_DISK_FULL\|ERROR_NOT_SUPPORTED\|ERROR_DIR_NOT_EMPTY\|ERROR_FILE_EXISTS\|ERROR_WRITE_PROTECT\|ERROR_IO_PENDING' \
  ~/.cargo/registry/src/*/windows-0.62.2/src/Windows/Win32/Foundation/mod.rs | head -20
```

⚠️ **Le module reste PUR : il ne dépend PAS du crate `windows`.** Les douze
valeurs y sont recopiées comme constantes `u32`, **avec le numéro de ligne de
`Foundation/mod.rs` en regard de chacune** — c'est le contrôle de revue que la
spec §3.1 impose aux transcriptions, appliqué ici aussi. Sans cela, `erreurs.rs`
serait gaté et perdrait sa testabilité d'hôte, ce qui est tout son intérêt.

- [ ] **Step 2 : les tests ROUGES**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `deux_causes_distinctes_ne_partagent_jamais_un_code` | deux variantes rendent le même `i32` — **le test que la spec §4.4 nomme**, et le contre-exemple est l'ancien pont, qui rendait `cb(-1)` = `EPERM` à **neuf** sites (`src/file.js:189,203,245,267,277,288,299,311,323`) |
| `aucun_code_rendu_n_est_un_succes` | un `hresult` avec le bit de sévérité à 0 |
| `le_canal_ferme_rend_bien_ERROR_IO_DEVICE` | autre chose que `0x8007045D` — c'est « l'erreur I/O standard » du cadrage §7 |
| `la_protection_en_ecriture_rend_0x80070013` | autre valeur — **F1 vit tout entier dans cet état** |

Le premier test s'écrit par balayage exhaustif des variantes (`Erreur` porte un
`const TOUTES: [Erreur; 12]`), pas par énumération à la main : sinon une
variante ajoutée demain échapperait au contrôle. **C'est ce qui empêche la table
d'être décorative** — même intention que le critère (4) de F3.

- [ ] **Step 3 : implémenter, vérifier, commit**

```bash
cd agent && cargo test -p agent erreurs 2>&1 | tail -5
git add agent/src/pont/erreurs.rs
git commit -m "pont(f1): un code distinct par cause, et le balayage qui l'exige" -- agent/src/pont/erreurs.rs
```

---

### Task 5 : `agent/src/pont/decoupe.rs` — la découpe des plages, PURE

**Files:** Create: `agent/src/pont/decoupe.rs`

**Interfaces:** Produces: `Morceau`, `decouper`.

⚠️ **C'est le module où vivent les erreurs d'unité, et c'est pour cela qu'il est
pur et testé isolément** (spec §7.3). Le critère (2) de la recette F1 — le
condensat SHA-256 — est exactement ce que ce module peut faire échouer : un
fichier tronqué, des plages dans le désordre, une dernière trame manquante.

- [ ] **Step 1 : les tests ROUGES — les cas limites que la spec §4.4 NOMME**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `une_plage_de_taille_exactement_TAILLE_TRAME_MAX_fait_un_seul_morceau` | deux morceaux, dont un vide |
| `une_plage_de_taille_nulle_ne_fait_aucun_morceau` | un morceau de longueur 0 émis |
| `la_somme_des_longueurs_egale_la_longueur_demandee` | somme ≠ longueur, sur cent tailles de 1 à 3·max |
| `les_morceaux_sont_contigus_et_croissants` | un trou ou un recouvrement |
| `le_premier_morceau_part_de_la_position_demandee` | position ignorée |
| `une_longueur_superieure_a_u32_MAX_se_decoupe_quand_meme` | débordement `u32` à la conversion — `PRJ_GET_FILE_DATA_CB` reçoit `length: u32` mais `byteoffset: u64`, et le fichier peut dépasser 4 Gio |

- [ ] **Step 2 : voir rouge, implémenter, revoir vert, commit**

```bash
cd agent && cargo test -p agent decoupe 2>&1 | tail -5
git add agent/src/pont/decoupe.rs
git commit -m "pont(f1): la decoupe des plages, avec ses trois cas limites" -- agent/src/pont/decoupe.rs
```

---

### Task 6 : `agent/src/pont/table.rs` — les commandes en vol, PURE

**Files:** Create: `agent/src/pont/table.rs`

**Interfaces:** Produces: tout le bloc `Table` du § « Interfaces partagées ».

C'est la réponse au défaut de l'ancien pont relevé par la spec §4.2 : **un
écouteur `message` par requête** (`src/file.js:155`), jamais retiré sur le
chemin d'erreur (`:126-129`), donc un écouteur à vie par opération en échec, et
tous les survivants ré-analysant chaque message suivant.

- [ ] **Step 1 : les tests ROUGES**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `une_reponse_arrivee_apres_annulation_est_jetee` | `resoudre` rend `Some` après `annuler` — **le test que la spec §4.4 nomme** |
| `une_commande_expiree_ne_reste_pas_en_table` | `en_vol()` non décrémenté après `expirees` |
| `une_reponse_arrivee_apres_expiration_est_jetee` | `resoudre` rend `Some` |
| `les_correlations_sont_monotones_et_ne_se_reutilisent_pas` | deux inscriptions rendent le même entier |
| `vider_rend_TOUT_et_laisse_la_table_vide` | une commande survit à `vider` — c'est ce qui précède `PrjStopVirtualizing` |
| `une_correlation_inconnue_rend_None_sans_paniquer` | panique |
| `deux_enumerations_du_meme_chemin_coexistent` | la seconde écrase la première — **la session d'énumération est indexée par le GUID d'énumération du rappel, PAS par le chemin**, qui n'est pas unique quand deux applications listent le même répertoire en même temps (spec §7.2) |
| `un_debordement_du_compteur_de_correlation_ne_reutilise_pas_une_correlation_en_vol` | collision après `u32::MAX` |

⚠️ **Le dernier test est le seul qui ne puisse pas s'écrire naïvement** :
faire tourner un `u32` demande 4 milliards d'inscriptions. Il s'écrit en
**injectant le compteur de départ** (`Table::nouvelle_depuis(u32::MAX - 2)`,
`#[cfg(test)]`), ce qui le rend atteignable en trois appels. **Sans cette
couture, le test serait vacueux** — il passerait sans rien exercer, et c'est
exactement le patron que D10 a attrapé quatre fois.

- [ ] **Step 2 : le budget de délai, POSÉ et déclaré non calibré**

F1 n'a besoin que de deux des trois budgets du §5.3 (le troisième, celui de
l'écriture, appartient à F2) :

```rust
/// ⚠️ NON CALIBRÉE. Posée, pas mesurée — c'est F4 qui donnera de quoi la
/// juger. Elle rejoint `BPP_MIN`, `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`,
/// `HYSTERESIS`, `REPIT_APRES_ECHEC`, `TAILLE_MAX_SORTIE`,
/// `REPIT_REARMEMENT_AUDIO` et `REARMEMENTS_MAX` dans la liste des constantes
/// de ce dépôt qu'aucune mesure n'a jugées.
///
/// L'ancien pont en avait UNE SEULE, 10 s, pour tout (`src/file.js:89`) :
/// d'où des lectures de gros blocs qui expiraient avant d'aboutir, et des
/// `getattr` qui figeaient l'Explorateur dix secondes sur un chemin
/// inexistant.
pub const DELAI_ATTRIBUTS: Duration = Duration::from_secs(2);
pub const DELAI_LIRE: Duration = Duration::from_secs(5);
pub const DELAI_LISTER: Duration = Duration::from_secs(20);
```

- [ ] **Step 3 : voir rouge, implémenter, revoir vert, commit**

```bash
cd agent && cargo test -p agent pont::table 2>&1 | tail -5
git add agent/src/pont/table.rs
git commit -m "pont(f1): la table des commandes en vol, et la reponse tardive qu'on jette" -- agent/src/pont/table.rs
```

---

# Famille B — le processus pont, et sa surveillance

### Task 7 : `scripts/run-agent.sh` transmet `PONT` — tâche DÉDIÉE

**Files:** Modify: `scripts/run-agent.sh`

**Interfaces:** Consumes: rien. Produces: rien de code.

⚠️ **Cette tâche ne fait qu'une ligne, et elle existe pour cela.** Le piège a été
payé en D1 (`SUPERVISEUR`), D2 (`MULTIFENETRE_REPRISE`) et D7 (`AUDIO` — où
implémenteur ET relecteur avaient vérifié la propriété **en traçant le code**,
le tracé étant juste et la valeur ne pouvant simplement pas atteindre le
processus). Il a été **évité** en D3 et D6 par une tâche dédiée placée avant
celle qui en a besoin. **On refait ce geste-là.**

- [ ] **Step 1 : relever la place exacte, ne jamais recopier un numéro de ligne**

```bash
grep -n 'CAPTEUR\|SUPERVISEUR' scripts/run-agent.sh
```
⚠️ `CLAUDE.md` porte l'avertissement explicite : « **Ne plus recopier de numéro
de ligne pour ce script : `grep -n` avant de s'y fier.** » Les numéros publiés
pour `PLEIN_ECRAN` y ont dérivé deux fois.

- [ ] **Step 2 : ajouter la ligne, à côté de `CAPTEUR`**

```sh
${PONT:+\$env:PONT = '$PONT'}
```

- [ ] **Step 3 : le contrôle qui peut échouer**

```bash
PONT=1 SUPERVISEUR=1 bash -n scripts/run-agent.sh && echo "syntaxe ok"
grep -c 'env:PONT' scripts/run-agent.sh    # attendu : 1
```
⚠️ **Le vrai contrôle est celui de la tâche 18** : `PONT` doit apparaître dans
`/media/vm/dev/run-agent.ps1` après un lancement. **Un `grep` sur le script bash
prouve qu'on a écrit la ligne, jamais qu'elle produit la variable** — c'est
précisément le raisonnement qui a échoué en D7.

- [ ] **Step 4 : commit**

```bash
git add scripts/run-agent.sh
git commit -m "outillage(f1): run-agent.sh transmet PONT, avant que quiconque en ait besoin" -- scripts/run-agent.sh
```

---

### Task 8 : `lanceur/pont.rs`, et la symétrie d'environnement

**Files:**
- Create: `agent/src/superviseur/lanceur/pont.rs`
- Modify: `agent/src/superviseur/lanceur.rs`, `agent/src/superviseur/protocole.rs`

**Interfaces:**
- Consumes: `LanceurDeProcessus`, `Enfant`, le job object.
- Produces: `lancer_pont`, `pont_vivant`, `SESSION_DU_PONT`.

- [ ] **Step 1 : relever la taille de départ et le modèle**

```bash
wc -l agent/src/superviseur/lanceur.rs      # attendu : 367
sed -n '150,215p' agent/src/superviseur/lanceur.rs   # lancer_capteur :156, capteur_vivant :198
grep -n 'env_remove' agent/src/superviseur/lanceur.rs
```
Attendu : `env_remove("SUPERVISEUR")` `:162`, `TEST_FILE` `:168`,
`WINDOW_TITLE` `:169` (dans `lancer_capteur`) ; `SUPERVISEUR` `:241`,
`TEST_FILE` `:265`, `WINDOW_TITLE` `:266`, `CAPTEUR` `:281` (dans `lancer`).

- [ ] **Step 2 : écrire l'enfant, en TRANSPOSANT `lancer_capteur`**

`lancer_pont` est la transposition littérale de `lancer_capteur`
(`lanceur.rs:156-188`), avec quatre différences, **et pas une de plus** :

```rust
//! Lancement et contrôle de vie du processus pont fichiers.
//!
//! Écrit dans un module enfant plutôt qu'en ligne dans `lanceur.rs`
//! (367 lignes, marge 133) : les jumeaux de `lancer_capteur` et
//! `capteur_vivant`, dans le style documentaire de ce fichier, y auraient pesé
//! 80 à 100 lignes et ramené la marge sous 45. Ce dépôt a payé quatre fois la
//! leçon « la marge regagnée par une extraction se reperd à la ronde suivante
//! si on la traite comme acquise » — ici la marge n'est pas prise du tout.
//!
//! **Même contrat ATOMIQUE que `lancer_capteur`** : `Err` signifie qu'aucun
//! processus ne tourne. Le rattachement au job est le seul post-traitement
//! faillible, et il tue donc lui-même le pont avant de rendre `Err`.

use super::*;

impl LanceurDeProcessus {
    pub fn lancer_pont(&self) -> Result<u32> {
        let mut pont = std::process::Command::new(&self.executable)
            .env("PONT", "1")
            .env("SESSION_ID", crate::superviseur::protocole::SESSION_DU_PONT)
            .env("SIGNALING_URL", &self.signaling_url)
            .env("LOCAL_IP", &self.local_ip)
            // Un pont qui hériterait de SUPERVISEUR se prendrait pour un
            // superviseur et lancerait ses propres enfants, indéfiniment.
            .env_remove("SUPERVISEUR")
            // Un pont qui hériterait de CAPTEUR se prendrait pour un CAPTEUR,
            // et non pour un pont : `main.rs:315` teste `CAPTEUR` AVANT tout le
            // reste, et rendrait la main à `capteur::executer`. Le pont ne
            // démarrerait jamais, sans qu'une seule ligne ne le dise.
            .env_remove("CAPTEUR")
            .env_remove("TEST_FILE")
            .env_remove("WINDOW_TITLE")
            .spawn()
            .context("lancement du pont fichiers")?;
        // … suite IDENTIQUE à lancer_capteur : PID, HANDLE, AssignProcessToJobObject,
        //   mise à mort en cas d'échec du rattachement, trace `pont lancé`, mémorisation.
    }
    pub fn pont_vivant(&self) -> bool { /* jumeau de capteur_vivant */ }
}
```

Les quatre différences : `PONT=1` au lieu de `CAPTEUR=1` ; `env_remove("CAPTEUR")`
en plus ; `SESSION_ID` / `SIGNALING_URL` / `LOCAL_IP` posés (le capteur n'en a
pas besoin, le pont si — il parle au signaling) ; le champ mémorisé est
`self.pont`, un `Mutex<Option<Enfant>>` de plus sur `LanceurDeProcessus`.

- [ ] **Step 3 : la symétrie INVERSE, dans `lanceur.rs`**

Ajouter `.env_remove("PONT")` **aux deux autres lanceurs** :
- dans `lancer_capteur` (près de `:162`) — un capteur qui hériterait de `PONT`
  ne deviendrait pas un pont (`CAPTEUR` est testé en premier), mais la
  dissymétrie est un piège dormant, et l'inverse est **fatal** ;
- dans `lancer` (près de `:281`, à côté de l'`env_remove("CAPTEUR")` existant) —
  **un enfant qui hériterait de `PONT` se prendrait pour un pont** et ne
  capturerait jamais rien.

⚠️ **La branche `PONT` de `main.rs` est placée APRÈS `CAPTEUR` (tâche 9)** :
c'est ce qui fait que l'oubli d'`env_remove("PONT")` dans `lancer` est le seul
des trois oublis qui casse le produit. **Cela ne dispense pas des deux autres** :
un ordre de test est une propriété qui change, un `env_remove` non.

- [ ] **Step 4 : la constante de session**

Dans `agent/src/superviseur/protocole.rs`, à côté de `SESSION_DE_CONTROLE`
(`:17`) :

```rust
/// Identifiant réservé de la session de signaling du pont fichiers.
///
/// DISTINCT de `SESSION_DE_CONTROLE` : le relais de signaling n'accepte qu'un
/// `agent` et un `client` par identifiant (`plateforme/src/signaling/appariement.ts:45-52`),
/// et la page-shell occupe déjà le rôle `client` de `bureau`. Deux
/// `PeerConnection` vers la même VM exigent donc deux identifiants.
///
/// ⚠️ Une seule VM, un seul utilisateur (spec §11) : cet identifiant n'est pas
/// namespacé par utilisateur, et il faudra qu'il le devienne le jour où la
/// plateforme servira plusieurs VM sur un même relais.
pub const SESSION_DU_PONT: &str = "fichiers";
```

- [ ] **Step 5 : le contrôle, et son rouge**

Test d'hôte impossible : `lanceur.rs` porte `#![cfg(windows)]` (`:36`).
Le contrôle est donc :

```bash
cd agent && cargo check --target x86_64-pc-windows-gnu 2>&1 | tail -3   # sortie 0, 9 avertissements
cd agent && cargo test -p agent 2>&1 | tail -3                          # 467 passed (inchangé)
wc -l agent/src/superviseur/lanceur.rs agent/src/superviseur/lanceur/pont.rs
```
⚠️ **Le rouge de la symétrie d'environnement n'est PAS atteignable ici** — il
l'est en recette (tâche 18, Step 4), par un relevé du nombre de processus et de
leur mode. **Le dire est plus honnête que d'inventer un test.**

- [ ] **Step 6 : commit**

```bash
git add agent/src/superviseur/lanceur.rs agent/src/superviseur/lanceur/pont.rs agent/src/superviseur/protocole.rs
git commit -m "pont(f1): lancer_pont, et la symetrie d'environnement dans les trois sens" \
  -- agent/src/superviseur/
```

---

### Task 9 : `agent/src/pont.rs` et la branche `PONT` de `main.rs`

**Files:**
- Create: `agent/src/pont.rs`
- Modify: `agent/src/main.rs`

**Interfaces:**
- Consumes: les modules purs des tâches 3 à 6.
- Produces: `pont::executer(config)`.

- [ ] **Step 1 : le modèle est `agent/src/capteur.rs`, 36 lignes**

```bash
cat agent/src/capteur.rs
```
Il assemble, il ne décide pas : les modules purs sont déclarés sans `cfg`, les
modules Windows avec. `executer()` existe en deux versions
(`#[cfg(windows)]` `:28`, `#[cfg(not(windows))]` `:34`, la seconde faisant
`bail!`). **`pont.rs` est bâti sur ce patron exactement.**

- [ ] **Step 2 : écrire `pont.rs`**

```rust
//! Le pont fichiers : un seul processus par VM, qui tient la racine de
//! virtualisation ProjFS et la sert depuis le répertoire local que la
//! page-shell a ouvert.
//!
//! Ce fichier reste mince à dessein — il assemble, il ne décide pas. Même
//! découpage que `capteur.rs` et `superviseur.rs` : la logique pure (chemins,
//! erreurs, découpe, table, transport) est hors `cfg` et se teste sur l'hôte ;
//! ce qui touche ProjFS est gaté.
//!
//! **Pourquoi un processus séparé** (spec §3.2) : les rappels ProjFS
//! s'exécutent sur des fils que LE SYSTÈME possède, où une panique Rust
//! devient un `abort` de processus. Les loger dans le superviseur ou le
//! capteur ferait du pont fichiers un risque pour la capture entière — c'est
//! la faute de l'ancien pont, transposée. Ici, le pire cas est la mort du
//! pont, que le superviseur relance.

pub mod chemins;
pub mod decoupe;
pub mod erreurs;
pub mod table;
pub mod transport;
#[cfg(windows)]
pub mod projfs;

#[cfg(windows)]
pub async fn executer(config: crate::Config) -> anyhow::Result<()> { /* tâche 11 puis 13 */ }

#[cfg(not(windows))]
pub async fn executer(_config: crate::Config) -> anyhow::Result<()> {
    anyhow::bail!("le mode pont n'existe que sur Windows")
}
```

- [ ] **Step 3 : la branche de `main.rs`, APRÈS `CAPTEUR`, AVANT `superviseur`**

```bash
sed -n '303,328p' agent/src/main.rs
```
Attendu : `diagnostics::aiguiller` `:307`, `CAPTEUR` `:315`,
`config.superviseur` `:323`, `demarrage::executer` `:327`.

```rust
    // Le mode pont ne capture rien et ne lance personne : il tient la racine
    // de virtualisation ProjFS et la sert depuis le répertoire que la
    // page-shell a ouvert. `PONT=0` DÉSACTIVE le mode, comme `CAPTEUR=0` et
    // `SUPERVISEUR=0` — même piège d'exploitation, même parade.
    //
    // Placée APRÈS `CAPTEUR` et AVANT `superviseur` : un pont qui hériterait
    // de `CAPTEUR` deviendrait un capteur, d'où l'`env_remove("CAPTEUR")` de
    // `lancer_pont`.
    if matches!(std::env::var("PONT").as_deref(), Ok(v) if v != "0") {
        return pont::executer(config).await;
    }
```
plus `mod pont;` dans la liste des modules (par ordre alphabétique, entre
`opus` `:26` et `pointer_settings` `:28` — **relever la place par `grep -n '^mod'`,
ne pas recopier ces numéros**).

- [ ] **Step 4 : LE contrôle de cette tâche, et il est double**

```bash
cd agent && cargo test -p agent 2>&1 | tail -3
```
**Attendu : 467 + le nombre de tests écrits aux tâches 3 à 6.** ⚠️ **C'est ici,
et nulle part avant, que les tests des modules purs entrent réellement dans la
suite** : jusqu'à cette tâche, aucun `mod` ne les déclarait et `cargo test`
rendait 467 sans les voir. **Un implémenteur qui aurait cru ses tâches 3 à 6
vertes sur un « 467 passed » n'aurait rien vérifié.** Compter, ne pas se
contenter de l'absence d'échec — un test perdu se lit comme un succès.

```bash
cd agent && cargo check --target x86_64-pc-windows-gnu 2>&1 | tail -3
```
⚠️ **Le second contrôle est la raison d'être de tout ce plan** : la
fonctionnalité `Win32_Storage_ProjectedFileSystem` n'est pas encore ajoutée
(tâche 12), et `pont.rs` ne doit **rien** importer d'elle. Sortie 0 attendue.

- [ ] **Step 5 : commit**

```bash
git add agent/src/pont.rs agent/src/main.rs
git commit -m "pont(f1): le troisieme mode, et les modules purs qui entrent enfin dans la suite" \
  -- agent/src/pont.rs agent/src/main.rs
```

---

### Task 10 : `boucle/surveillance_pont.rs` — le jumeau de `surveillance_capteur`

**Files:**
- Create: `agent/src/superviseur/boucle/surveillance_pont.rs`
- Modify: `agent/src/superviseur/boucle.rs`

**Interfaces:**
- Consumes: `lancer_pont`, `pont_vivant` (tâche 8).
- Produces: `EtatPont::demarrer`, `EtatPont::surveiller`.

- [ ] **Step 1 : lire le modèle EN ENTIER**

```bash
cat agent/src/superviseur/boucle/surveillance_capteur.rs   # 148 lignes
```
Il porte trois choses à transposer **et à comprendre**, pas seulement à copier :

1. `PERIODE_RELANCE_CAPTEUR_MIN = 500 ms` (`:32`) — sans elle, un pont qui meurt
   aussitôt après avoir été relancé ferait retenter un vrai `spawn` à la cadence
   de la boucle (~10 Hz) ;
2. `cycle_signale` (`:40-55`, correctif I3) — la période espace les `spawn`,
   **pas les LIGNES** : deux lignes par seconde sur un partage CIFS, soit
   ~170 000 par jour. Signaler la première fois, se taire tant que la situation
   se répète, redevenir bruyant au premier retour à la normale (`:103-108`) ;
3. la condition de durée du réarmement (`:95-102`) — exiger que le processus
   tienne au moins aussi longtemps que l'espacement des relances est ce qui
   distingue « il repart » de « il agonise en boucle ».

- [ ] **Step 2 : la DIFFÉRENCE d'avec le capteur, qui est la seule décision**

`EtatCapteur::demarrer` est **fatal** au superviseur en cas d'échec (`:63-66` :
« un échec de CET appel est fatal au superviseur, au même titre qu'un pilote ou
qu'un hook qui ne s'ouvre pas »).

**`EtatPont::demarrer` ne l'est PAS.** C'est le cadrage §4 principe 4 rendu
structurel : un pont qui ne démarre pas ne doit pas empêcher la capture de
tourner. Un échec est journalisé en `warn!` et **retenté indéfiniment** par
`surveiller`, exactement comme une relance.

```rust
/// Contrairement à `EtatCapteur::demarrer`, un échec ici n'est PAS fatal.
/// Le capteur sert le média à tout enfant qui se rattache : sans lui, chaque
/// enfant capturerait dans le vide. Le pont, lui, ne sert que le lecteur de
/// fichiers — le cadrage §4 principe 4 exige qu'une panne de ce côté ne
/// touche jamais le flux vidéo, et le rendre fatal ferait exactement
/// l'inverse.
pub(super) fn demarrer(lanceur: &LanceurDeProcessus) -> Self;
```

- [ ] **Step 3 : câbler dans `boucle.rs`**

```bash
grep -n 'surveillance_capteur\|EtatCapteur' agent/src/superviseur/boucle.rs
```
Ajouter `mod surveillance_pont;`, la construction d'`EtatPont` à côté de celle
d'`EtatCapteur`, et l'appel à `surveiller` dans le même tour de boucle.

- [ ] **Step 4 : contrôles**

```bash
cd agent && cargo check --target x86_64-pc-windows-gnu 2>&1 | tail -3
wc -l agent/src/superviseur/boucle.rs agent/src/superviseur/boucle/surveillance_pont.rs
```
Attendu : `boucle.rs` ≈ **280** (marge ~220), l'enfant ≈ **150**.
⚠️ **Le rouge est en recette** (tâche 18, Step 6) : tuer le pont par PID, le
voir relancé, et **la vidéo ne pas broncher**. Aucun test d'hôte ne peut le
voir : `boucle.rs` est `#[cfg(windows)]`.

- [ ] **Step 5 : commit**

```bash
git add agent/src/superviseur/boucle.rs agent/src/superviseur/boucle/surveillance_pont.rs
git commit -m "pont(f1): la surveillance du pont, et l'echec qui n'est pas fatal" -- agent/src/superviseur/
```

---

### Task 11 : `agent/src/pont/transport.rs` — WebRTC données seules

**Files:**
- Create: `agent/src/pont/transport.rs`
- Modify: `agent/src/pont.rs`

**Interfaces:**
- Consumes: `crate::signaling::run_signaling` (`agent/src/signaling.rs:53`), str0m.
- Produces: `construire_rtc_donnees`, `tourner`, `VersNavigateur`, `DuNavigateur`.

- [ ] **Step 1 : lire le modèle, et voir ce qui TOMBE**

```bash
cat agent/src/transport/initialisation.rs      # 80 lignes, construire_rtc :27
```
Pour un point d'accès **données seules**, tombent : `enable_h264` (`:58`),
`enable_opus` (`:59`), `enable_bwe` (`:65`), `set_stats_interval` (`:66`),
`set_desired_bitrate` (`:70`). Restent : le socket UDP non bloquant (`:29-36`,
avec sa raison — le délai de `set_read_timeout` déborde massivement sous
Windows), `str0m::crypto::from_feature_flags().install_process_default()`
(`:47`, idempotent), et le candidat hôte (`:75-77`).

⚠️ **`clear_codecs()` sans aucun `enable_*` doit être vérifié compilable et
fonctionnel** : si str0m 0.21 refuse un `Rtc` sans codec, la parade est de
laisser `enable_h264(true)` sans jamais négocier de piste — le navigateur
n'en offre aucune. **Signaler, ne pas contourner en silence.**

- [ ] **Step 2 : le canal `fichiers` — l'agent est RÉPONDANT, il ne le crée pas**

```bash
sed -n '45,64p' agent/src/transport/evenements.rs      # Event::ChannelOpen(id, label) :51
sed -n '105,115p' agent/src/transport/controle.rs      # self.rtc.channel(id).write(false, …) :109-110
```
C'est le navigateur qui appelle `createDataChannel('fichiers')`. Le pont reçoit
`Event::ChannelOpen(id, "fichiers")`, retient l'`id`, et écrit par
`self.rtc.channel(id).write(true, octets)` — **`binary = true`**, à l'inverse du
canal `control` qui écrit `false`.

⚠️ **Le pont aiguille SUR LE LABEL, dès le premier jour** : il retient l'`id`
associé au label `"fichiers"` et **refuse tout ChannelData venant d'un autre
id**, avec un `warn!` qui nomme l'id. C'est la leçon de
`transport/evenements.rs:168-189` (tâche 17) appliquée d'emblée plutôt que
repayée.

- [ ] **Step 3 : le test d'hôte, et son échafaudage PROPRE**

```bash
grep -n 'mod fixtures' agent/src/transport.rs                # :51-52, #[cfg(test)]
grep -n 'fn local_peer' agent/src/transport/fixtures.rs      # :42, pub(super)
```
⚠️ **`transport::fixtures` est `pub(super)` : `pont/` ne peut PAS l'employer.**
Ne pas le hisser — cela toucherait le chemin vidéo pour un besoin de test.
Écrire un échafaudage local `#[cfg(test)]` dans `pont/transport.rs`, minimal :
un second `Rtc` sur loopback qui `add_channel("fichiers")` et échange l'offre.

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `une_trame_emise_par_le_pont_arrive_au_pair_en_binaire` | `binary == false` reçu, ou rien reçu |
| `une_trame_du_pair_remonte_avec_sa_correlation` | corrélation altérée, ou trame perdue |
| `un_channeldata_venu_d_un_autre_canal_est_refuse_et_journalise` | la trame est traitée comme une réponse |
| `la_fermeture_du_canal_remonte_CanalFerme` | la boucle se termine en silence |

Le troisième test est celui qui **ne peut se voir rouge que si on l'écrit
avant** : il faut négocier **deux** canaux au pair (`fichiers` et un
`autre-canal`) et envoyer sur le mauvais. Sans ce second canal, le test est
vacueux.

- [ ] **Step 4 : contrôles**

```bash
cd agent && cargo test -p agent pont::transport 2>&1 | tail -5
cd agent && cargo check --target x86_64-pc-windows-gnu 2>&1 | tail -3
wc -l agent/src/pont/transport.rs
```

- [ ] **Step 5 : commit**

```bash
git add agent/src/pont/transport.rs agent/src/pont.rs
git commit -m "pont(f1): une PeerConnection sans media, et un aiguillage par label des le premier jour" \
  -- agent/src/pont/ agent/src/pont.rs
```

---

# Famille C — ProjFS

### Task 12 : `pont/projfs/chargement.rs` — les treize transcriptions

**Files:**
- Create: `agent/src/pont/projfs/chargement.rs`
- Modify: `agent/Cargo.toml`

**Interfaces:**
- Consumes: `windows` (types seuls), `LoadLibraryW`, `GetProcAddress`.
- Produces: `struct ProjFs` — les treize pointeurs, résolus une fois.

⚠️ **C'est la tâche qui porte le risque R7 de la spec** : treize signatures
transcrites à la main, **et le compilateur ne peut plus rien en dire**. Un
mauvais `transmute` de fonction est un défaut que rien n'attrape avant
l'exécution. Ce qui suit est ce qu'on peut faire — **ce n'est pas une
garantie**, et la tâche le dit dans son propre en-tête de module.

- [ ] **Step 1 : ajouter la fonctionnalité, et vérifier qu'elle n'émet AUCUN import**

```bash
grep -n 'Win32_Storage_ProjectedFileSystem' ~/.cargo/registry/src/*/windows-0.62.2/Cargo.toml
```
Attendu : `554:Win32_Storage_ProjectedFileSystem = ["Win32_Storage"]`.

Ajouter `"Win32_Storage_ProjectedFileSystem",` à `agent/Cargo.toml`, juste après
`"Win32_Storage_FileSystem",`.

⚠️ **La raison pour laquelle on prend les TYPES et pas les ENVELOPPES** est
relevable :

```bash
sed -n '1,30p' ~/.cargo/registry/src/*/windows-link-0.2.1/src/lib.rs
```
Le `link!` se développe en
`#[link(name = …, kind = "raw-dylib", modifiers = "+verbatim")]`, donc en
**import statique** dans le PE. ⚠️ **Précision relevée, que la spec §3.1 rate
d'un cran** : elle cite « `windows-link-0.2.1/src/lib.rs:9` », qui est la ligne
de la branche **`target_arch = "x86"`** (l. **5-15**, seule à porter
`import_name_type = "undecorated"`). La branche employée par notre cible x86_64
est la seconde (l. **18-28**, son `#[link]` à la **l. 22**), **sans** `import_name_type`. **La substance tient
pour les deux** — c'est bien un `raw-dylib` dans les deux cas —, mais le numéro
cité ne désigne pas l'arme qui nous concerne.

**Conséquence, et elle est la justification de tout D1** : `agent.exe` est **un
seul binaire pour les quatre modes** (`main.rs:307-327`). Un import non résolu
ne tuerait pas « le pont » : il tuerait **la capture, la vidéo et l'input** sur
toute VM sans ProjFS — l'état exact de la VM avant la tâche 1.

- [ ] **Step 2 : LE CONTRÔLE QUI PEUT ÉCHOUER, et qui est le cœur de la tâche**

```bash
cd agent && cargo build --release --target x86_64-pc-windows-gnu 2>&1 | tail -3
x86_64-w64-mingw32-objdump -x target/x86_64-pc-windows-gnu/release/agent.exe \
  | grep -i 'projectedfslib' || echo "AUCUN import ProjectedFSLib — c'est le vert attendu"
```

⚠️ **Ce contrôle DOIT être vu rouge avant d'être cru.** Pour le rendre rouge :
ajouter temporairement, dans n'importe quel `#[cfg(windows)]` du crate, un appel
à `windows::Win32::Storage::ProjectedFileSystem::PrjStopVirtualizing(ctx)`,
recompiler, et vérifier que `objdump` **trouve** alors `PROJECTEDFSLIB.dll` dans
la table d'import. **Puis le retirer.** Sans ce rouge, le vert ne prouve rien —
il pourrait aussi bien dire qu'`objdump` a échoué ou que le motif est mal écrit.
⚠️ Si `x86_64-w64-mingw32-objdump` n'est pas disponible, employer
`x86_64-w64-mingw32-nm -D` ou, à défaut, `strings agent.exe | grep -i projectedfs` —
**et déclarer l'outil réellement employé** dans le rapport.

> ❌ **CE CONTRÔLE, ÉCRIT TEL QUEL, PORTE DEUX DÉFAUTS QUI L'EMPÊCHENT D'ÊTRE
> ROUGE — les deux relevés à l'exécution** (`f1-tache12-objdump.txt`, versé) :
>
> 1. **le chemin est FAUX.** Le dépôt est un espace de travail cargo : la cible
>    vit à la RACINE, donc `../target/x86_64-pc-windows-gnu/release/agent.exe`
>    depuis `agent/`. Sur le chemin publié ci-dessus, `objdump` rend « pas de
>    tel fichier », et le `|| echo "AUCUN import…"` **traduit cette absence de
>    fichier en VERT** ;
> 2. **la recette du rouge est INSUFFISANTE.** Une `pub fn` qui appelle
>    l'enveloppe **sans aucun appelant** ne rend pas le contrôle rouge : `agent`
>    est un binaire, l'éditeur de liens élimine la fonction inatteignable, et
>    `raw-dylib` n'émet alors aucun import. **Mesuré : le rouge n'est pas
>    apparu.** L'appel doit être sur un chemin réellement atteignable depuis
>    `main` — placé dans `pont::executer`, le rouge est venu
>    (`Nom DLL: projectedfslib.dll`), puis le vert est revenu au retrait.

⚠️ **Portée exacte du contrôle** : il porte sur le binaire `-gnu` de l'hôte, pas
sur le binaire `msvc` de la VM. Il ne prouve pas l'absence d'import dans le
livrable. **Le contrôle qui porte sur le vrai binaire est en recette**
(tâche 18, Step 3) : renommer temporairement `ProjectedFSLib.dll` et vérifier
que le superviseur, le capteur et un enfant démarrent quand même. **Le dire
plutôt que de laisser croire que l'un vaut l'autre.**

- [ ] **Step 3 : transcrire les treize, avec leur numéro de ligne SOURCE**

Chaque transcription porte en commentaire le numéro de ligne du `link!` du
module `windows-rs`, qui reste la source de vérité (spec §3.1). **Relevé par la
commande le 19 août 2026** dans
`windows-0.62.2/src/Windows/Win32/Storage/ProjectedFileSystem/mod.rs`
(621 lignes, 19 entrées `Prj*`, 8 types `PRJ_*_CB`) :

| Entrée | Ligne du `pub unsafe fn` | Ligne du `link!` |
| --- | --- | --- |
| `PrjAllocateAlignedBuffer` | 2 | 3 |
| `PrjClearNegativePathCache` | 7 | 8 |
| `PrjCompleteCommand` | 12 | 13 |
| `PrjDeleteFile` | 17 | 21 |
| `PrjFileNameCompare` | 33 | (dans le corps) |
| `PrjFileNameMatch` | 42 | (dans le corps) |
| `PrjFillDirEntryBuffer` | 51 | 55 |
| `PrjFreeAlignedBuffer` | 67 | 68 |
| `PrjMarkDirectoryAsPlaceholder` | 88 | 93 |
| `PrjStartVirtualizing` | 97 | 101 |
| `PrjStopVirtualizing` | 108 | 109 |
| `PrjWriteFileData` | 121 | 122 |
| `PrjWritePlaceholderInfo` | 126 | 130 |

⚠️ **TROIS pièges de transcription, relevés dans le module et non supposés :**

1. **`PrjStartVirtualizing` a CINQ paramètres dans le `link!`, quatre dans
   l'enveloppe.** L'enveloppe cache un paramètre de sortie
   (`namespacevirtualizationcontext : *mut PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT`,
   l. 101) et le rend en `Result`. **Transcrire l'enveloppe plutôt que le
   `link!` produirait un appel dont le dernier argument manque** — corruption de
   pile, aucun diagnostic.
2. **`PrjStopVirtualizing` et `PrjFreeAlignedBuffer` ne rendent RIEN** (l. 109,
   68), pas un `HRESULT`. Leur donner un type de retour est un défaut d'ABI.
3. **`PrjAllocateAlignedBuffer` rend un `*mut c_void`** (l. 3), pas un
   `HRESULT` : `NULL` est l'échec.

⚠️ **`PrjWritePlaceholderInfo2` et `PrjFillDirEntryBuffer2` ne sont PAS
chargées**, bien que présentes (l. 59, 134) : elles sont d'une génération
ultérieure, leur présence sur cette machine n'a pas été vérifiée (spec §2.2), et
la v1 n'en a besoin pour rien (elles servent les liens symboliques, hors
périmètre §3.5.2).

- [ ] **Step 4 : les deux tests, et ce qui les rend rouges**

**(a) Test de fumée, `#[cfg(windows)]`, exécuté sur la VM seulement** —
`ProjFs::charger()` échoue si **une seule** des treize entrées manque, et
l'erreur **nomme l'entrée** :

```rust
// Rouge : une entrée absente donne `Ok`, ou une erreur qui ne dit pas laquelle.
// Rendu rouge en injectant un quatorzième nom bidon dans la liste, une fois,
// puis en le retirant.
```

**(b) Test d'hôte de comparaison de signatures** — pour les **huit** entrées
dont il existe un type `PRJ_*_CB` correspondant, une affectation qui ne compile
pas si l'ABI diverge :

```rust
#[test]
fn les_signatures_de_rappel_transcrites_valent_les_types_de_windows_rs() {
    // Compile ou ne compile pas — c'est tout ce qu'il y a à vérifier.
    let _: windows::Win32::Storage::ProjectedFileSystem::PRJ_GET_FILE_DATA_CB =
        Some(crate::pont::projfs::rappel_get_file_data);
    // … les sept autres.
}
```
⚠️ **Ce test n'est possible que sur `--target x86_64-pc-windows-gnu`, pas sur
l'hôte Linux nu** : les types `PRJ_*_CB` sont `#[cfg(windows)]`. Il est donc
couvert par `cargo check --target x86_64-pc-windows-gnu`, **pas** par
`cargo test -p agent`. **Le dire, et ne pas compter ce test dans le nombre de la
suite d'hôte.**
⚠️ **Et il ne couvre que les huit RAPPELS, pas les treize ENTRÉES** : il n'existe
aucun type `windows-rs` auquel comparer `PrjWriteFileData`. **Les cinq entrées
sans jumeau restent couvertes par la seule relecture** — c'est nommé, pas
contourné.

- [ ] **Step 5 : contrôles et commit**

```bash
cd agent && cargo check --target x86_64-pc-windows-gnu 2>&1 | tail -3
cd agent && cargo test -p agent 2>&1 | tail -3     # inchangé
wc -l agent/src/pont/projfs/chargement.rs
git add agent/Cargo.toml agent/src/pont/projfs/chargement.rs
git commit -m "pont(f1): treize entrees resolues a l'execution, et l'objdump qui le prouve" \
  -- agent/Cargo.toml agent/src/pont/projfs/
```

---

### Task 13 : `pont/projfs.rs` — la racine, la virtualisation, et les rappels MINIMAUX

**Files:**
- Create: `agent/src/pont/projfs.rs`
- Modify: `agent/src/pont.rs`

**Interfaces:**
- Consumes: `chargement::ProjFs`, `erreurs::hresult`.
- Produces: `Virtualisation::demarrer(racine) -> Result<Self>`, `Drop`.

**Objet** : le premier état VERT observable sur la VM — le dossier
`%USERPROFILE%\Mes Fichiers` **apparaît, vide**, et le pont s'arrête proprement.
Les rappels existent tous les **huit** mais ne demandent rien au navigateur :
c'est la tâche 14 qui les branche.

- [ ] **Step 1 : la racine, et ses TROIS contraintes**

Spec §3.6 :
1. **marquer une seule fois** — `PrjMarkDirectoryAsPlaceholder` avec un GUID
   d'instance ; re-marquer une racine déjà marquée **échoue**. Le GUID est donc
   **persisté hors de la racine**, dans `%LOCALAPPDATA%\Guacamole\pont\instance.guid` ;
2. **la racine ne doit contenir aucune donnée au moment du marquage** ;
3. **rien de notre état ne vit dans la racine** — il serait lui-même un objet
   projeté, donc dépendant du pont pour être lu (circulaire), et il
   disparaîtrait avec la racine le jour où il faudrait la recréer, **c'est-à-dire
   exactement le jour où il sert**.

- [ ] **Step 2 : LA DISCIPLINE DE FIL, et elle est la décision centrale**

Écrite en tête de module, parce qu'elle est ce qu'aucun test d'hôte ne pourra
vérifier :

```rust
//! **Trois catégories de fils, et la frontière entre elles est stricte.**
//!
//! 1. **Les fils de RAPPEL, que le SYSTÈME possède.** ProjFS en tient un
//!    vivier dimensionné par `PRJ_STARTVIRTUALIZING_OPTIONS.PoolThreadCount` /
//!    `ConcurrentThreadCount` (windows-rs, ProjectedFileSystem/mod.rs:518-523).
//!    Un rappel qui s'y exécute :
//!      - enveloppe TOUT son corps dans `std::panic::catch_unwind` et rend
//!        `E_UNEXPECTED` — une panique Rust qui traverserait une frontière
//!        `extern "system"` est un ABANDON DE PROCESSUS ;
//!      - n'écrit QUE dans `pont::table`, sous verrou, et pousse sur un
//!        `mpsc::Sender` ;
//!      - n'appelle JAMAIS `PrjCompleteCommand`, ne touche JAMAIS le socket,
//!        ne tient JAMAIS un verrou pendant une E/S ;
//!      - rend `HRESULT_FROM_WIN32(ERROR_IO_PENDING)` (0x800703E5) et rend la
//!        main IMMÉDIATEMENT.
//!    Y attendre un aller-retour navigateur figerait l'APPLICATION qui lit le
//!    fichier — pas la vidéo : le flux continue de couler et la fenêtre montre
//!    une application gelée (spec §5.2). C'est la seule raison d'être de cette
//!    discipline.
//!
//! 2. **LE fil du pont, unique.** Il possède le `Rtc`, lit le socket, complète
//!    les commandes par `PrjCompleteCommand`, et balaie les expirations. C'est
//!    ce qui rend `pont::table` PUR et testable : la concurrence y est une
//!    propriété de la table, pas du système.
//!
//! 3. **Le fil principal**, qui n'a que `demarrer` et `Drop`.
//!
//! Le `PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT` est un pointeur brut partagé
//! entre 1 et 2 : il est enveloppé dans un type dont l'`unsafe impl Send +
//! Sync` porte sa justification en commentaire, jamais posé à la légère.
```

**Ce qui se passe si une commande dépasse son délai** — et il faut être précis
sur ce que « un rappel dépasse son délai » veut dire ici :

> **Un rappel ne dépasse jamais son délai : il rend en microsecondes.** Ce qui
> dépasse, c'est la **commande** qu'il a inscrite. Le balayage du fil du pont
> (période 250 ms) retire les échues de la table et les complète par
> `PrjCompleteCommand(command_id, HRESULT_FROM_WIN32(ERROR_SEM_TIMEOUT))`.
> L'application reçoit une E/S expirée ; **rien n'est rejoué**, jamais — une
> commande expirée dont on rejouerait la requête produirait une seconde réponse
> sans destinataire (spec §5.3). Si la réponse du navigateur arrive **après**,
> `Table::resoudre` rend `None` et la réponse est **jetée** : c'est le test
> `une_reponse_arrivee_apres_expiration_est_jetee` de la tâche 6.

- [ ] **Step 3 : les huit rappels, MINIMAUX mais TOUS PRÉSENTS**

`PRJ_CALLBACKS` a huit champs (mod.rs:143-152). En F1 :

| Champ | F1, tâche 13 |
| --- | --- |
| `StartDirectoryEnumerationCallback` | ouvre une session dans la table, `S_OK` **synchrone** |
| `EndDirectoryEnumerationCallback` | ferme la session, `S_OK` **synchrone** |
| `GetDirectoryEnumerationCallback` | `S_OK` avec un buffer vide (tâche 14 le branche) |
| `GetPlaceholderInfoCallback` | `ERROR_FILE_NOT_FOUND` (tâche 14) |
| `GetFileDataCallback` | `ERROR_FILE_NOT_FOUND` (tâche 14) |
| `QueryFileNameCallback` | `ERROR_FILE_NOT_FOUND` (tâche 14) |
| `NotificationCallback` | **le refus d'écriture — voir Step 4** |
| `CancelCommandCallback` | retire de la table ; **obligatoire dès F1** |

⚠️ **`CancelCommandCallback` n'est optionnel que pour un fournisseur
SYNCHRONE** (spec §4.3). Le nôtre ne l'est pas : une application qui abandonne
son E/S nous laisserait une commande orpheline. **Il est livré en F1, pas
différé.**

⚠️ **Précision sur l'énoncé de la spec** : F1 (§8) dit « les **cinq rappels
obligatoires** de ProjFS **en mode asynchrone** ». Les cinq obligatoires sont
`StartDirectoryEnumeration`, `GetDirectoryEnumeration`,
`EndDirectoryEnumeration`, `GetPlaceholderInfo`, `GetFileData`. **Mais seuls
TROIS se complètent en asynchrone** — la table de correspondance de la spec §4.3
dit elle-même que `StartDirectoryEnumeration` et `EndDirectoryEnumeration` sont
« synchrone, `S_OK` », puisqu'elles ne consultent jamais le navigateur.
**Cinq sont implémentés, trois sont asynchrones**, et c'est ainsi qu'il faut lire
le livrable. *(Divergence d'énoncé, pas de conception.)*

- [ ] **Step 4 : le refus d'écriture, qui est le PÉRIMÈTRE de F1**

« Toute tentative d'écriture rend `ERROR_WRITE_PROTECT` (0x80070013). C'est un
périmètre, pas une lacune » (spec §8, F1). **La spec ne dit pas COMMENT, et il
faut le dire ici, parce que le naïf ne marche pas** : avec
`showDirectoryPicker({ mode: 'read' })`, la FSA refuse bien l'écriture — mais
**côté VM, l'écriture réussit localement** : ProjFS *hydrate* le fichier et
l'application écrit sur le fichier NTFS local. Le fournisseur n'est prévenu
qu'**après coup**, à la fermeture du handle (spec §6.1). Une application
verrait donc son enregistrement **réussir**, et rien n'arriverait jamais côté
poste local. **C'est pire qu'une erreur : c'est une perte silencieuse.**

**Le levier existe, et il est relevé** : `PRJ_NOTIFY_FILE_PRE_CONVERT_TO_FULL`
(mod.rs:385, valeur `4096u32`) est la notification « une application est sur le
point d'écrire, il faut hydrater complètement ». **C'est une notification
`PRE_`, donc REFUSABLE** : le rappel rend `HRESULT_FROM_WIN32(ERROR_WRITE_PROTECT)`
et l'écriture échoue **avant** d'avoir commencé.

`PrjStartVirtualizing` reçoit donc un `PRJ_NOTIFICATION_MAPPING` (mod.rs:345-348)
sur la racine, avec le masque :

```
PRJ_NOTIFY_FILE_PRE_CONVERT_TO_FULL | PRJ_NOTIFY_PRE_RENAME
  | PRJ_NOTIFY_PRE_DELETE | PRJ_NOTIFY_PRE_SET_HARDLINK | PRJ_NOTIFY_NEW_FILE_CREATED
```
Les **quatre premières** sont refusées par `ERROR_WRITE_PROTECT` (la dernière des
quatre par `ERROR_NOT_SUPPORTED`, spec §3.5.2). **Ces quatre rappels sont
SYNCHRONES et ne consultent jamais le navigateur** (spec §4.3) : leur seul rôle
est d'autoriser ou de refuser, et la décision se prend sans quitter le fil.

⚠️ **`PRJ_NOTIFY_NEW_FILE_CREATED` (mod.rs:388) est une notification POST, elle
ne se refuse pas.** Un fichier créé de toutes pièces dans la racine existe donc
localement et **n'est jamais poussé** en F1. **Ce n'est pas une perte de donnée
de l'utilisateur** — son poste local ne l'a jamais eu — mais c'est une
divergence, et elle est **journalisée en `warn!` avec le chemin**, pour que la
recette puisse la constater plutôt que la découvrir. Refermer ce cas est du
ressort de F2.

- [ ] **Step 5 : la perte de données — lesquels des TROIS cas s'appliquent à F1**

Spec §6.4 nomme trois cas et n'en referme aucun. **En lecture seule :**

| Cas | En F1 |
| --- | --- |
| 1 — l'utilisateur ne revient jamais | **SANS OBJET** : aucune écriture n'est due, rien n'attend de sortir |
| 2 — l'utilisateur revient avec un AUTRE répertoire | **SANS OBJET** : il n'y a pas de journal à rejouer |
| 3 — la racine doit être recréée | **SANS OBJET pour la perte** : aucun octet de l'utilisateur ne vit dans la racine, seulement des copies hydratées de ce qui existe déjà côté poste local |
| 4 (« pas une perte, mais qui mord ») — **l'hydratation remplit le disque de la VM** | 🔴 **S'APPLIQUE PLEINEMENT, et F1 est le sous-bloc qui la crée** : chaque fichier LU est écrit en entier sur le disque de la VM et **il y reste** |

**Ce qui doit donc être instrumenté DÈS F1, et rien de plus** : une trace
périodique (période 60 s) de la taille occupée par la racine, avec le nombre
d'entrées hydratées.

```
racine hydratee octets=… entrees=…
```
⚠️ **Aucune politique d'éviction en F1** — `PrjDeleteFile` est chargée
(tâche 12) pour que la politique, quand elle viendra, n'ait pas à rouvrir la
couche. **Poser une politique sans mesure serait exactement le geste que ce
dépôt reproche à ses constantes non calibrées.** La mesure appartient à F5,
l'instrument est ici.

- [ ] **Step 6 : l'arrêt, et l'ordre qui n'est pas négociable**

```
1. cesser d'accepter de nouvelles commandes
2. Table::vider() → PrjCompleteCommand(id, ERROR_IO_DEVICE) pour CHACUNE
3. PrjStopVirtualizing(ctx)
```
⚠️ **L'ordre est le remède au dernier défaut de l'annexe §13** : l'arrêt forcé de
l'ancien pont n'appelait **pas** les callbacks FUSE en attente
(`src/file.js:359-365`) — le noyau n'obtenait jamais de réponse. Ici,
`PrjStopVirtualizing` est **précédé** de la complétion en erreur de tout ce qui
reste. Ceci vit dans `Drop`, **et `Drop` ne panique jamais**.

- [ ] **Step 7 : contrôles**

```bash
cd agent && cargo check --target x86_64-pc-windows-gnu 2>&1 | tail -3
cd agent && cargo test -p agent 2>&1 | tail -3      # inchangé : aucun test d'hôte ici
wc -l agent/src/pont/projfs.rs
```
⚠️ **`pont/projfs.rs` est `#[cfg(windows)]` et appelé par le système :
AUCUN test d'hôte n'est possible, et c'est déclaré, pas contourné** (spec §4.4).
La seule compensation est qu'il soit **mince** : il traduit, il ne décide pas.
**Toute décision qui pourrait vivre dans un module pur DOIT y vivre — c'est un
critère de revue de la tâche 19, pas un souhait.** Si le fichier approche 400,
les rappels d'énumération partent dans `projfs/enumeration.rs`, **jamais par
compression**.

- [ ] **Step 8 : commit**

```bash
git add agent/src/pont/projfs.rs agent/src/pont.rs
git commit -m "pont(f1): la racine, la discipline de fil, et l'ecriture refusee avant d'avoir commence" \
  -- agent/src/pont/ agent/src/pont.rs
```

---

### Task 14 : les trois rappels ASYNCHRONES, branchés sur la table et le canal

**Files:** Modify: `agent/src/pont/projfs.rs`, `agent/src/pont.rs`

**Interfaces:** Consumes: tâches 5, 6, 11, 13.

- [ ] **Step 1 : `GetPlaceholderInfo` — le plus simple, donc le premier**

`Attributs { chemin }` → `PrjWritePlaceholderInfo` avec un `PRJ_FILE_BASIC_INFO`
(mod.rs:263-271 : `IsDirectory`, `FileSize`, `CreationTime`, `LastAccessTime`,
`LastWriteTime`, `ChangeTime`, `FileAttributes`).

⚠️ **`FILE_ATTRIBUTE_NORMAL` (ou `_DIRECTORY`) pour tout**, et c'est déclaré :
la FSA n'expose aucun attribut, il n'y a rien à transporter (spec §3.5.2).
Les horodatages viennent de `File.lastModified` ; **la FSA n'en donne qu'UN**,
et les quatre champs le portent tous. C'est une divergence, elle est
documentée dans le code.

- [ ] **Step 2 : `GetFileData` — le rappel qui décide du critère (2) de la recette**

`Lire { chemin, position, longueur }`, découpée par `pont::decoupe` en morceaux
de `TAILLE_TRAME_MAX`. **Chaque morceau est écrit par `PrjWriteFileData` dès son
arrivée**, dans un tampon obtenu par `PrjAllocateAlignedBuffer` et rendu par
`PrjFreeAlignedBuffer`.

⚠️ **`PrjAllocateAlignedBuffer` / `PrjFreeAlignedBuffer` sont la première source
de fuite mémoire d'un fournisseur ProjFS** (spec §4.3) : le couple est encapsulé
dans un garde `Drop`, **jamais appelé à la main**, et le garde a son test
d'intention (`le_tampon_est_rendu_meme_si_l_ecriture_echoue`) sous forme d'un
chemin d'erreur exercé.

⚠️ **Le fichier entier n'entre JAMAIS en mémoire.** C'est l'inverse exact de
l'ancien pont, dont chaque lecture faisait `getFile()` + `arrayBuffer()` +
`.slice(...)` (`web/index.js:562-564`) : une lecture séquentielle d'un fichier de
100 Mio par blocs de 128 Kio y relisait **100 Mio depuis le disque, huit cents
fois**.

⚠️ **Contrôle de flux, en F1 : UN morceau en vol à la fois.** Le pont ne demande
le morceau *n+1* qu'après avoir reçu le morceau *n*. C'est le plus simple, et
c'est suffisant : **le contrôle de flux par `bufferedAmount` et `SEUIL_TAMPON`
est un livrable de F3** (spec §8, F3), pas de F1. **Le dire plutôt que de
l'implémenter à moitié.**

- [ ] **Step 3 : `GetDirectoryEnumeration` — les deux pièges de ProjFS**

Spec §7.2, et ils sont tous deux **silencieux** :

1. **L'ordre est IMPOSÉ.** Les entrées doivent être remplies dans l'ordre de
   `PrjFileNameCompare` — qui n'est **ni** l'ordre lexicographique d'`OsStr`,
   **ni** `Ordering::cmp`. `dir.values()` de la FSA ne garantit **aucun** ordre.
   **Le pont trie lui-même, par `PrjFileNameCompare`** (chargée en tâche 12
   précisément pour cela).
2. **Le filtre `searchExpression` est FACULTATIF et il est FOURNI.**
   `PRJ_GET_DIRECTORY_ENUMERATION_CB` reçoit
   `searchexpression: PCWSTR` (mod.rs:315). **L'ignorer est une faute
   silencieuse** : un `dir /b *.txt` rendrait tout. Il s'applique par
   `PrjFileNameMatch`.
3. **`PRJ_CB_DATA_FLAG_ENUM_RESTART_SCAN`** (mod.rs:177, valeur `1i32`) doit être
   honoré : il redémarre l'énumération en cours.

La complétion se fait avec `PRJ_COMPLETE_COMMAND_EXTENDED_PARAMETERS`
(mod.rs:181-184) portant
`CommandType = PRJ_COMPLETE_COMMAND_TYPE_ENUMERATION` (mod.rs:214, valeur `2i32`)
et le `DirEntryBufferHandle` du rappel.

⚠️ La session d'énumération est indexée par le **GUID d'énumération** du rappel,
**pas par le chemin** — qui n'est pas unique quand deux applications listent le
même répertoire en même temps. C'est le test
`deux_enumerations_du_meme_chemin_coexistent` de la tâche 6.

- [ ] **Step 4 : le cache négatif, et pourquoi il est la première économie**

`PRJ_FLAG_USE_NEGATIVE_PATH_CACHE` (mod.rs:314, valeur `1i32`) au démarrage de la
virtualisation. Windows sonde en permanence des chemins qui n'existent pas —
`desktop.ini`, `Thumbs.db`, `folder.jpg`, les manifestes d'application — et
**chacun de ces sondages serait sinon un aller-retour navigateur** (spec §7.4).
`QueryFileName` ne consulte le navigateur que si le cache ne tranche pas.

⚠️ **Un cache d'énumération (`TTL_ENUMERATION`) est mentionné par la spec §7.4 et
n'est PAS livré en F1** : `Rafraichir`, qui est le seul moyen de l'invalider, est
un livrable de **F5**. Poser un cache dont rien ne peut vider le contenu ferait
qu'un fichier ajouté côté poste local n'apparaîtrait **jamais** — le défaut
exact de l'ancien pont, dont le cache de données n'avait **aucun TTL**
(`src/file.js:232-241`). **F1 ne met donc AUCUN cache d'énumération**, et le
critère ROUGE de F5 (« le fichier apparaît **sans** `Rafraichir` ») sera par
construction rouge tant que F5 n'existe pas. **C'est cohérent et il faut
l'écrire**, sans quoi F5 lira un vert qui ne veut rien dire.

- [ ] **Step 5 : contrôles**

```bash
cd agent && cargo check --target x86_64-pc-windows-gnu 2>&1 | tail -3
cd agent && cargo test -p agent 2>&1 | tail -3
wc -l agent/src/pont/projfs.rs
```

- [ ] **Step 6 : commit**

```bash
git add agent/src/pont/projfs.rs agent/src/pont.rs
git commit -m "pont(f1): trois rappels asynchrones, l'ordre de PrjFileNameCompare, et le filtre qu'on n'ignore pas" \
  -- agent/src/pont/
```

---

# Famille D — le client

### Task 15 : `client/src/fichiers/` — le protocole et l'adaptateur, PURS

**Files:**
- Create: `client/src/fichiers/protocole.ts`, `adaptateur.ts`,
  `protocole.test.ts`, `adaptateur.test.ts`

**Interfaces:** Consumes: `proto/ts/fichiers.ts` (tâche 2).

⚠️ **La FSA est INJECTÉE, jamais importée** (spec §4.4) : la logique de
résolution de chemin, de découpe et de corrélation est séparée du
`FileSystemDirectoryHandle` et testée par Vitest avec un **faux système de
fichiers en mémoire**. Sans cette couture, aucun de ces tests n'existerait —
Vitest tourne sous Node, qui n'a pas de FSA.

- [ ] **Step 1 : les tests ROUGES**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `lister_rend_les_entrees_avec_leur_nature` | un répertoire annoncé comme fichier |
| `lister_la_racine_prend_le_chemin_vide` | `""` refusé |
| `attributs_rend_la_taille_et_l_horodatage` | taille toujours nulle |
| `lire_une_plage_ne_lit_QUE_cette_plage` | le faux `File` compte ses appels : **rouge si `arrayBuffer()` est appelé au lieu de `slice(o, o+n).arrayBuffer()`** — c'est le défaut relevé de l'ancien pont (`web/index.js:562-564`), et le test le rend visible |
| `lire_au_dela_de_la_fin_rend_moins_d_octets_sans_lever` | exception |
| `un_chemin_inexistant_rend_le_code_Introuvable` | une exception opaque remonte |
| `une_erreur_de_la_FSA_devient_un_CODE_jamais_une_chaine` | `JSON.stringify(e)` d'une `Error` → `"{}"` — **le défaut exact de `web/index.js:669`, reconstruit en `new Error("{}")` par `src/file.js:127` : toute la cause était détruite à l'émission** |
| `une_reponse_a_une_correlation_inconnue_est_ignoree` | elle est traitée |

⚠️ Le quatrième test est le seul dont la valeur dépend du **faux** : il exige un
`File` factice qui **compte les appels** à `arrayBuffer()` et à `slice()`.
Un faux qui rendrait simplement les bons octets passerait dans les deux cas —
c'est-à-dire serait vacueux.

- [ ] **Step 2 : implémenter, contrôler**

```bash
cd client && npx vitest run 2>&1 | tail -3     # 107 + les neufs
cd client && npx tsc --noEmit
```

- [ ] **Step 3 : commit**

```bash
git add client/src/fichiers/
git commit -m "client(f1): la logique fichiers, pure, avec la FSA injectee" -- client/src/fichiers/
```

---

### Task 16 : le canal, le bouton, et l'état de la page-shell

**Files:**
- Create: `client/src/fichiers/canal.ts`
- Modify: `client/src/shell.ts`, `client/src/shell-page.ts`, `client/shell.html`

**Interfaces:** Consumes: tâche 15.

- [ ] **Step 1 : `connectSession` n'est PAS réutilisable, et il faut le dire**

```bash
sed -n '7,20p;180,215p' client/src/webrtc.ts
```
Relevé : `SessionOptions` exige `video: HTMLVideoElement` (`:10`), et
`connectSession` ajoute inconditionnellement `addTransceiver('video')` (`:202`)
et `addTransceiver('audio')` (`:206`), puis crée `input` (`:210`) et `control`
(`:214`).

⚠️ **La spec §3.4 montre ce qui tombe côté AGENT (`construire_rtc`), pas côté
CLIENT.** `canal.ts` est donc un **connecteur distinct**, pas un paramètre de
`connectSession`. Il **réemploie sans les copier** `parseSignalingMessage`
(`webrtc.ts:54`), `waitForAnswer` (`:84`) et `attendreConfigIce` (`:164`) —
les deux premières sont déjà exportées ; **si `attendreConfigIce` doit l'être,
c'est une modification de `webrtc.ts` à déclarer, pas à glisser.**

⚠️ **Ne PAS refactorer `connectSession`** pour rendre la vidéo optionnelle : ce
serait toucher le chemin vidéo de toutes les fenêtres pour un besoin qui a sa
propre fonction. Le coût est une trentaine de lignes dupliquées ; le coût
inverse est une régression sur le chemin critique.

Le canal :

```ts
const canal = pc.createDataChannel('fichiers', { ordered: true });
// fiabilité PAR DÉFAUT, donc fiable : une plage d'octets perdue est un fichier
// corrompu. C'est l'exact opposé du canal `input`
// (client/src/webrtc.ts:210-213, `maxRetransmits: 0`), et la raison inverse.
```

- [ ] **Step 2 : le bouton, dans la page-shell et nulle part ailleurs**

Spec §3.3 : la page-shell détient le répertoire, **et elle seule**. Trois
raisons, dont la troisième suffit : elle est déjà la page unique par utilisateur
(`client/src/shell.ts:1-6` le dit et donne sa raison) ; elle est le seul endroit
où le geste utilisateur a un sens ; **aucune fenêtre d'application n'est
spéciale**, et c'est l'invariant que la page-shell existe pour tenir.

```html
<!-- client/shell.html -->
<button id="choisir-dossier">Choisir mon dossier</button>
<div id="etat-fichiers"></div>
```

```ts
// showDirectoryPicker() EXIGE une activation utilisateur transitoire : il est
// appelé dans le gestionnaire de clic, jamais depuis un message de canal.
// `mode: 'read'` — F1 vit tout entier en lecture seule (spec §8).
const racine = await window.showDirectoryPicker({ mode: 'read' });
```

⚠️ **Le handle n'est PAS persisté** (spec §3.3) : il est sérialisable en
IndexedDB, mais au rechargement la permission doit être re-accordée par
`requestPermission()`, **qui exige à son tour une activation utilisateur**.
Persister n'économiserait que la traversée de l'arborescence dans le sélecteur,
jamais le geste — **un clic par chargement de la page-shell, et c'est tout**.

⚠️ **Le `beforeunload` de fermeture destructrice est un livrable de F2**, pas de
F1 (spec §3.3, §6.2) : en lecture seule, fermer la page-shell ne perd aucune
donnée. **Ne pas l'anticiper.**

- [ ] **Step 3 : l'état, dans `shell.ts`, injecté comme le reste**

`shell.ts` (93 lignes) est testé et **séparé du DOM et du WebSocket** : « toute
la logique est ici, séparée du DOM et du WebSocket, pour être testable :
`creerBureau` reçoit ses effets par injection » (`:8-9`). L'état du lecteur suit
la même règle : `OptionsBureau` gagne `afficherEtatFichiers(texte)`, et `Bureau`
gagne `lecteurMonte(nom)` / `lecteurDemonte()`.

Tests neufs dans `client/src/shell.test.ts` (97 lignes) :

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `monter_le_lecteur_affiche_le_nom_du_dossier` | rien n'est affiché |
| `demonter_le_lecteur_efface_l_etat` | le texte survit — **c'est le défaut relevé en D5** : le bandeau `#status` garde son `textContent` après `expirer()`, donc lire le texte prouve qu'un message est arrivé, **pas qu'il était affiché** |

- [ ] **Step 4 : contrôles**

```bash
cd client && npx vitest run 2>&1 | tail -3
cd client && npx tsc --noEmit
cd client && npm run build 2>&1 | tail -3      # vite.config.ts a DEUX entrées : main et shell
wc -l client/src/shell.ts client/src/shell-page.ts client/src/fichiers/canal.ts
```

- [ ] **Step 5 : commit**

```bash
git add client/src/fichiers/canal.ts client/src/shell.ts client/src/shell-page.ts client/src/shell.test.ts client/shell.html
git commit -m "client(f1): le bouton, le canal donnees seules, et l'etat du lecteur" -- client/
```

---

# Famille E — la dette voisine que F1 rend visible

### Task 17 : `transport/evenements.rs` aiguille par CANAL, plus par `data.binary`

**Files:** Modify: `agent/src/transport/evenements.rs`

**Interfaces:** Consumes: rien. Produces: rien de public.

- [ ] **Step 1 : le fait, RELU dans le code et non recopié de la spec**

```bash
sed -n '51,63p;168,189p' agent/src/transport/evenements.rs
```

**Vérifié le 19 août 2026 — la citation de la spec §3.4 est EXACTE, aux deux
bornes.** `dispatch_channel_data` occupe bien `:168-189`, et son corps aiguille
sur le **seul** `data.binary` (`:174`) : tout ce qui est binaire va à
`InputMessage::decode`. Le label est bien disponible `:51-63`
(`Event::ChannelOpen(id, label)`), et il est **simplement inutilisé** — seul
`"control"` est reconnu (`:53`), pour mémoriser son `ChannelId` (`:54`).

**Sort décidé, et il tient en deux phrases :**

> **La `PeerConnection` dédiée de D4 rend ce défaut SANS OBJET POUR F1** : le
> canal `fichiers` vit dans une autre `PeerConnection`, dans un autre processus,
> servi par `pont/transport.rs`, qui aiguille sur le label dès son premier jour
> (tâche 11). Aucune trame `fichiers` ne peut atteindre `dispatch_channel_data`.
> **Mais elle ne le REFERME pas** : le défaut reste entier dans la
> `PeerConnection` de chaque enfant, dormant parce qu'aujourd'hui seul `input`
> y est binaire, et il est **le piège exact où tombera la première personne qui
> relira §3.4 et jugera la voie du « troisième canal » assez bon marché** — son
> seul symptôme serait un `WARN "message d'entrée invalide"` par trame.

**Il est donc corrigé ici, et pas légué**, pour trois raisons dont la
troisième décide : il est **testable sur l'hôte** (`transport/` n'est pas gaté),
donc le contrôle peut être vu rouge ; il coûte une dizaine de lignes ; et ce
dépôt a payé **quatre fois** le bras catch-all de `capteur/pont_media.rs`
(D5 `Sommeil`, D6 `Part`, D7 `Audio`, D8 `PleinEcran`) pour apprendre qu'un
aiguillage qui ne nomme pas ses cas se paie à chaque message neuf.

⚠️ **Si cette tâche est retirée du périmètre pour une raison quelconque, elle
DOIT être inscrite comme legs dans le document de résultats et dans `CLAUDE.md`,
avec sa citation `evenements.rs:168-189`.** Un défaut connu et non écrit est un
défaut perdu.

- [ ] **Step 2 : les tests ROUGES, écrits AVANT**

```bash
wc -l agent/src/transport/evenements.rs    # attendu : 391, marge 109
```

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `une_trame_binaire_du_canal_input_atteint_toujours_on_input` | l'entrée cesse d'arriver — **c'est le test de NON-RÉGRESSION, et c'est le plus important des trois** |
| `une_trame_binaire_d_un_canal_inconnu_est_refusee_et_le_WARN_nomme_le_canal` | elle est décodée comme une entrée souris (le comportement d'aujourd'hui) |
| `une_trame_binaire_arrivee_avant_tout_ChannelOpen_est_refusee` | elle est décodée |

Le rouge du deuxième s'obtient en négociant **deux** canaux binaires au pair de
test (`input` et `parasite`) et en écrivant sur le second : sur le code
d'aujourd'hui, `on_input` est appelé. **Sans ce second canal, le test est
vacueux.**

- [ ] **Step 3 : le remède, minimal**

`Event::ChannelOpen` mémorise `input_channel: Option<ChannelId>` quand
`label == "input"`, exactement comme `control_channel` l'est déjà pour
`"control"` (`:54`). `dispatch_channel_data` compare l'`id` avant de décoder.

⚠️ **Le risque est borné et il faut le mesurer, pas l'affirmer** : aujourd'hui
`control` écrit en `false` (`transport/controle.rs:110`) et `input` est le seul
canal binaire d'une `PeerConnection` d'enfant. Le remède ne change donc le
comportement d'aucune trame existante. **Le premier test le prouve sur l'hôte ;
la recette (tâche 18, critère 4) exige en plus que la SOURIS RÉPONDE pendant
toute la mesure.** Un test d'hôte seul aurait laissé passer une régression de
négociation.

- [ ] **Step 4 : contrôles et commit**

```bash
cd agent && cargo test -p agent transport::evenements 2>&1 | tail -5
cd agent && cargo test -p agent 2>&1 | tail -3
wc -l agent/src/transport/evenements.rs     # attendu ≈ 435, marge ≈ 65 — À RELEVER, pas à supposer
git add agent/src/transport/evenements.rs
git commit -m "corrige(f1): l'aiguillage regarde enfin le canal, et pas seulement le drapeau binaire" \
  -- agent/src/transport/evenements.rs
```

---

# Recette et clôture

### Task 18 : recette F1 sur la VM

**Files:**
- Create: `docs/superpowers/plans/journaux-pont-fichiers/` (journaux et jumeaux `-plat`)

**Interfaces:** Consumes: les tâches 1 à 17.

**Deux exécutions par critère. Aucun taux ne sera revendiqué.**

- [ ] **Step 1 : préparer, et vérifier ce qui tourne déjà**

```bash
virsh list --all && virsh start Windows
until timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null; do sleep 5; done
until ls /media/vm/dev >/dev/null 2>&1; do sleep 5; done
set -a && source .env && set +a
node scripts/winrm.js 'Get-Process agent -ErrorAction SilentlyContinue | Format-List Id,StartTime'
```
⚠️ **Un pilote qui laisse un superviseur vivant bloque SILENCIEUSEMENT la
tentative suivante — rencontré trois fois sur trois en D8** : le nouveau
`StreamWriter` ne peut pas ouvrir `agent.log` déjà tenu, et **la copie relue est
celle, périmée, de la tentative précédente**. `Get-Process agent` se revérifie
**après chaque tentative, y compris échouée**.

```bash
scripts/build-agent.sh
```
⚠️ **Vérifier la TAILLE du binaire** : une compilation de 0,13 s est un aveu
(un `rsync -a` qui remonte le temps fait garder le binaire précédent ; seul
`cargo clean --release -p agent` débloque).

- [ ] **Step 2 : le jeu de données, et ce qu'il doit contenir**

Côté poste local (l'hôte), un répertoire portant **au moins** :
- un sous-répertoire, lui-même contenant un fichier (les **deux niveaux** que le
  critère 1 exige) ;
- un fichier de **plus de 10 Mio**, dont on relève le condensat :

```bash
sha256sum /chemin/vers/gros.bin | tee docs/superpowers/plans/journaux-pont-fichiers/sha256-origine.txt
```
- un nom accentué et un nom avec espace (les chemins passent par UTF-16) ;
- **deux fichiers ne différant que par la casse**, pour que le legs de casse de
  la tâche 3 soit **observé**, pas conjecturé.

- [ ] **Step 3 : le contrôle qui justifie D1 tout entier, et il passe AVANT**

```bash
node scripts/winrm.js 'Rename-Item C:\Windows\System32\ProjectedFSLib.dll ProjectedFSLib.dll.absent'
# lancer superviseur + capteur + un enfant, SANS pont utilisable
node scripts/winrm.js 'Rename-Item C:\Windows\System32\ProjectedFSLib.dll.absent ProjectedFSLib.dll'
```
**Attendu** : le superviseur, le capteur et l'enfant démarrent **normalement**,
une session vidéo s'établit, et le journal du pont porte une erreur qui **nomme
la DLL** (`ERROR_MOD_NOT_FOUND`, spec §3.1).

> ❌ **`ERROR_MOD_NOT_FOUND` N'EST ÉMISE NULLE PART**, et ce `grep` rendrait
> donc `0` sur une exécution où le contrôle a parfaitement réussi. La chaîne
> ne vit que dans cette page et dans la spec. Ce que le journal porte
> réellement (`agent-sans-projfs-plat.log`, 20 août 2026) est le contexte
> `anyhow` **`chargement de ProjectedFSLib.dll`** suivi de
> **`Le module spécifié est introuvable. (0x8007007E)`** — **366** occurrences.
> **Grepper `ProjectedFSLib`.**

⚠️ **C'est le contrôle qui prouve que la résolution à l'exécution fait ce pour
quoi elle a été choisie**, et il est **atteignable et provoqué** — l'état est
littéralement celui de la VM avant la tâche 1. Sans lui, D1 est une intention.
⚠️ **Renommer une DLL système exige les droits et peut être refusé par WRP** : si
le renommage échoue, **le dire** et replier sur un contrôle plus faible
(`PONT=1` sur une machine sans ProjFS n'existe pas ici) — **jamais déclarer le
contrôle passé.**

- [ ] **Step 4 : la symétrie d'environnement, relevée depuis la VM**

```bash
node scripts/winrm.js 'Get-Process agent | Format-Table Id,StartTime -Auto | Out-String'
grep -a 'env:PONT' /media/vm/dev/run-agent.ps1
```
**Attendu** : **exactement trois** processus `agent` de plus que le nombre de
fenêtres — le superviseur, le capteur, le pont — et `PONT` présente dans le
script d'amorçage. ⚠️ **C'est le seul endroit où l'oubli d'un `env_remove`
devient visible** : un enfant qui aurait hérité de `PONT` ne capturerait rien, et
un pont qui aurait hérité de `CAPTEUR` serait un second capteur. Relever aussi
`grep -c 'pont lancé'` (attendu : 1) et `grep -c 'capteur lancé'` (attendu : 1).

> ❌ **`pont lancé` N'EXISTE PAS : la trace est `pont fichiers lancé`**, et
> l'attendu de **1** est faux lui aussi — **DEUX** lignes distinctes la portent
> à chaque lancement, l'une de `superviseur::lanceur::pont` (`INFO`, avec le
> `pid` et la session), l'autre de `superviseur::boucle::surveillance_pont`
> (`WARN`). Relevé : **2** aux cinq exécutions nominales. `capteur lancé`, lui,
> est exact et vaut **1**. *Un `grep` faux sur une chaîne inexistante aurait
> fait lire un succès comme un pont absent.*

- [ ] **Step 5 : les quatre critères de réception de F1** (spec §8)

```bash
sed 's/\x1b\[[0-9;]*m//g' agent.log > agent-f1-1-plat.log
```

| # | Critère | Comment on le relève |
| --- | --- | --- |
| 1 | `%USERPROFILE%\Mes Fichiers` montre **exactement** l'arborescence choisie, **aux deux niveaux** | `Get-ChildItem -Recurse` sur la VM, comparé **par ensemble de NOMS** à `find` côté hôte |
| 2 | le fichier de plus de 10 Mio, copié vers `C:\`, a **le même condensat SHA-256** | `Get-FileHash`, comparé à `sha256-origine.txt` |
| 3 | aucune opération de l'Explorateur ne dépasse les budgets du §5.3 | ❌ **CONTRÔLE VACUEUX** : `grep -c 'ERROR_SEM_TIMEOUT\|delai depasse'` → **0**, mais **aucune de ces deux chaînes n'est jamais émise par le produit**. Le témoin réel est `commande expirée` (`agent/src/pont/service.rs`), et il vaut **0 aux cinq exécutions nominales** — un zéro qui, lui non plus, n'établit rien, puisque **aucune lecture de `gros.bin` n'y est allée à son terme**. *Un contrôle qui ne peut pas échouer, pour la troisième fois dans ce dépôt.* |
| 4 | **une session vidéo tourne pendant TOUTE la recette et ne perd pas une image** | `framesDecoded` monotone dans la page d'application, **0** `clôture de session amorcée` dans `agent.log`, **et la souris répond** (tâche 17) |

⚠️ **Le critère 2 est le seul qui ne puisse pas être satisfait par accident**
(spec §8). Un critère qui se contenterait de « le fichier s'ouvre » serait
vérifié par un fichier tronqué, par un fichier dont les plages sont dans le
désordre, et par un fichier dont la dernière trame manque : **les trois sont des
défauts que `pont/decoupe.rs` peut réellement produire.**

⚠️ **Le critère 1 se compare par ENSEMBLE DE NOMS, jamais par cardinal** — piège
maison : un compteur ne suffit pas quand un tiers agit sur le système.

⚠️ **Le critère 4 se vérifie sur `framesDecoded` ET sur ce que l'utilisateur
voit** : « vidéo intacte » est **faux si on l'étend à l'application** — un rappel
ProjFS qui ne se terminerait pas figerait le fil de l'application qui lit le
fichier, le flux continuerait de couler, et la fenêtre montrerait une
application **gelée** ; *les images arrivent, le contenu ne bouge plus* (spec
§5.2). **`framesDecoded` ne dit rien de cela.** Relever une capture d'écran de
l'Explorateur à deux instants espacés.

- [ ] **Step 6 : les TROIS états rouges, PROVOQUÉS** (spec §8, F1)

| Rouge provoqué | Attendu |
| --- | --- |
| (i) tuer le processus pont **en pleine lecture** (par PID relevé) | l'Explorateur rend une **erreur d'E/S** en moins de 5 s, **jamais ne se fige** ; la vidéo continue ; le superviseur relance le pont (tâche 10) |
| (ii) fermer la page-shell **pendant une copie** | l'opération en vol se termine en `ERROR_IO_DEVICE` (0x8007045D) ; le lecteur se démonte ; **la vidéo ne bronche pas** |
| (iii) démarrer le pont **avant** que le répertoire n'ait été choisi | la racine existe et **toute lecture rend `ERROR_IO_DEVICE`**, pas un gel et pas un dossier fantôme |

⚠️ **Ces trois états sont ce qui rend le critère 4 non trivial** : un critère
« la vidéo n'a pas décroché » relevé sur une recette où rien n'a mal tourné ne
prouve rien. **Les provoquer est obligatoire.**

- [ ] **Step 7 : ce que la recette DOIT aussi observer, et consigner tel quel**

- **le legs de casse** (tâche 3) : les deux fichiers ne différant que par la
  casse — lequel s'ouvre, lequel rend `Introuvable` ;
- **l'hydratation** : la trace `racine hydratee octets=… entrees=…` (tâche 13),
  **avant** et **après** la copie du fichier de 10 Mio. Le disque de la VM a
  grossi d'au moins cette taille, **et il ne redescend pas** ;
- **la création locale** : créer un fichier dans la racine depuis l'Explorateur.
  ❌ **CET ATTENDU EST FAUX, et la mesure l'a montré** : le fichier est **CRÉÉ**
  (2 exécutions versées sur 2 qui atteignent cette phase). Ce n'est pas une
  divergence du code : `PRJ_NOTIFY_FILE_PRE_CONVERT_TO_FULL` ne concerne pas un
  fichier NEUF, et `agent/src/pont/notifications.rs` documentait déjà que
  `NEW_FILE_CREATED` est une **POST**, donc irrefusable. **C'est le plan et la
  spec §8 qui promettaient trop.** ~~Attendu : refusé~~ (`ERROR_WRITE_PROTECT`) par
  `PRJ_NOTIFY_FILE_PRE_CONVERT_TO_FULL`. Si Windows le crée quand même,
  relever le `warn!` de la tâche 13 Step 4 et **le consigner comme divergence**,
  pas comme succès.

- [ ] **Step 8 : deuxième exécution, intégrale, puis verser**

Rejouer **entièrement**. **Deux exécutions, et chaque énoncé du document de
résultats porte ce nombre.** Verser chaque `agent-*.log` **avec son jumeau
`-plat`**, plus les condensats, les captures d'écran et les relevés PowerShell.

⚠️ **Copier `agent.log` APRÈS la fin réelle de l'exécution, pas à la fin du
pilote** : les enfants meurent quand le navigateur se ferme, donc **après** la
copie, et leurs lignes de libération partent avec le journal suivant. Une pièce
a été perdue ainsi en D4.

⚠️ **Une commande backgroundée par le harnais ne survit PAS à la fin du tour de
l'agent qui l'a lancée** — le processus meurt sans notification, et le symptôme
est un journal **tronqué** copié depuis une VM où l'agent continue de tourner :
on croit à une panne du produit. **Appel bloquant au premier plan, avec un délai
explicite couvrant la durée complète.**

⚠️ **`grep` sans `-a` sur un journal portant une queue d'octets NUL rend une
sortie VIDE, pas zéro** — indiscernable d'un compte nul (D10).

- [ ] **Step 9 : commit**

```bash
git add docs/superpowers/plans/journaux-pont-fichiers/
git commit -m "recette(f1): un lecteur en lecture seule, condensat a l'appui, deux executions" \
  -- docs/superpowers/plans/journaux-pont-fichiers/
```

---

### Task 19 : revue transverse de fin de branche, résultats, et `CLAUDE.md`

**Files:**
- Create: `docs/superpowers/plans/2026-08-19-pont-fichiers-f1-resultats.md`
- Modify: `CLAUDE.md`, et tout fichier dont un commentaire est devenu faux

**Cette tâche n'est jamais facultative.**

- [ ] **Step 1 : la cible propre de cette revue**

Elle a trouvé **cinq** défauts en D7, **trois** Critiques en D8, **six** en D9,
**douze** en D10, **huit** en P1 — et **tous franchissaient une frontière de
tâche** : chacun est correct des deux côtés pris séparément. *Une revue par
tâche ne peut structurellement pas les voir ; ce n'est pas un défaut de rigueur,
c'est une propriété du découpage.*

Sa cible nommée ici : **les affirmations de code devenues fausses dans cette
branche même**. Trois candidates sont connues d'avance :

```bash
cd agent && grep -rn 'aiguille sur\|data.binary\|le label est\|inutilisé' src/transport/ | head
cd agent && grep -rn 'trois sortes de processus\|deux modes\|superviseur, capteur' src/ | head
grep -rn 'showDirectoryPicker\|lecture seule\|mode.*read' client/src/ | head
```

1. tout commentaire disant que l'agent a **trois** modes (il en a **quatre**
   depuis la tâche 9) — commencer par `agent/src/main.rs`, `capteur.rs`,
   `superviseur.rs`, `lanceur.rs` ;
2. tout commentaire décrivant l'aiguillage de `evenements.rs` d'avant la
   tâche 17 ;
3. tout commentaire de `pont/` promettant un comportement de F2 à F5 — **le
   piège le plus probable de cette branche** : ce plan cite abondamment des
   sections de spec qui décrivent le produit **fini**, et un commentaire qui les
   recopierait promettrait ce que F1 ne tient pas.

- [ ] **Step 2 : balayer par le SENS, pas par la formule**

Leçon payée deux fois : un balayage sur « non diagnostiqué » a laissé survivre
« pas diagnostiqué », et c'était la conclusion d'une section entière. Chercher la
**chose niée** en énumérant les tournures — « pas / non / jamais / seulement /
sans explication / reste ouvert / n'est établi par rien ». Et **annoter
l'affirmation elle-même, pas sa voisine**.

- [ ] **Step 3 : les tailles, RELEVÉES PAR LA COMMANDE, APRÈS la dernière édition**

```bash
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
```
⚠️ **Après** la dernière édition de la ronde, jamais avant : une table relevée en
début de ronde est fausse à la fin de la même ronde — erreur que D8 a commise en
croyant bien faire.

- [ ] **Step 4 : « corrigé à sa place » est une affirmation de COMPLÉTUDE**

Pour chaque nombre corrigé dans `CLAUDE.md`, **énumérer ses places avant
d'écrire** :

```bash
grep -n '<le nombre>' CLAUDE.md
```
puis **relire place par place APRÈS l'édition** : le naufrage du « 487 » s'est
rejoué **neuf fois** dans ce dépôt, dont une dans la vague même qui le
corrigeait ailleurs, et une dans un commit dont le message annonçait avoir
énuméré les places avant d'écrire. *Une substitution qui ne dit pas combien
d'occurrences elle a touchées est une affirmation de complétude non vérifiée.*
⚠️ **Toucher une ligne d'un tableau de comptes oblige à remesurer son compte**,
même quand ce n'est pas l'objet de l'édition.

- [ ] **Step 5 : écrire le document de RÉSULTATS, permanent et versé**

`docs/superpowers/plans/2026-08-19-pont-fichiers-f1-resultats.md`, portant :
le verdict de F0 avec son rouge d'avant ; les quatre critères de F1 **avec leur
nombre d'exécutions** ; les trois rouges provoqués ; ce que F1 **n'établit pas** ;
les pièges neufs ; l'état des legs.

⚠️ **Toute preuve d'une affirmation vit dans git.** D9 a perdu **six** constats
de revue parce que son espace de travail (`.superpowers/sdd/`) était gitignoré et
jamais commité : ils sont définitivement perdus, et D9 laissait au dépôt la
conclusion **sans sa preuve**, plus quatre phrases invitant à « relire » un
rapport qui n'existait plus.

⚠️ **Ne JAMAIS fabriquer une sortie de commande.** D10 en a attrapé deux, dont
une **inscrite dans `CLAUDE.md` et présentée comme « vérifiée par la commande »,
à l'intérieur même d'une correction qui dénonçait une affirmation non étayée** :
le relecteur a relancé la commande, elle rendait zéro. Les deux fois le fait
rapporté était vrai ; les deux fois la preuve ne l'était pas. **Le mécanisme
nommé par l'implémenteur : réutiliser la sortie d'une commande antérieure pour
répondre à la question d'une AUTRE, sans la relancer.**

- [ ] **Step 6 : écrire la section `CLAUDE.md` « Sous-projet ③ Pont fichiers — F0/F1 »**

Y porter, au minimum :
- **le fait le plus réutilisable** : ProjFS s'active par
  `Enable-WindowsOptionalFeature -Online -FeatureName Client-ProjFS`, et
  **`Install-WindowsFeature Projected-File-System` échoue en silence** sur
  Server 2022 ;
- 🔴 **`cd client && npx vitest run` ne couvre PAS `proto/ts/`** — deux
  commandes, pas une ;
- la convention de la variable `PONT` (**`=0` désarme**), et sa transmission par
  `scripts/run-agent.sh` ;
- **la discipline de fil des rappels ProjFS**, et pourquoi une panique y est un
  abandon de processus ;
- **les quatre modes d'`agent.exe`** — la phrase « trois sortes de processus »
  est désormais fausse partout où elle figure ;
- ce que F1 **n'établit pas**, et les legs.

- [ ] **Step 7 : vérifications finales, les QUATRE nombres**

```bash
cd agent && cargo test -p agent 2>&1 | tail -3
cd agent && cargo check --target x86_64-pc-windows-gnu 2>&1 | tail -3
cd client && npx vitest run 2>&1 | tail -3
cd client && npx vitest run --dir ../proto 2>&1 | tail -3
```
Reporter les **quatre** nombres relevés dans le message de commit, en regard des
références d'entrée (467 / 9 avertissements / 107 / 35).

- [ ] **Step 8 : commit**

```bash
git add CLAUDE.md agent/src client/src proto docs/superpowers/plans/
git commit -m "docs(f1): resultats du pont fichiers, revue transverse, et les chiffres releves par la commande"
```

---

## Ordre et dépendances

```
1  (F0 — ÉLIMINATOIRE, rien ne commence avant)
   │
   ├── 2, 3, 4, 5, 6      (protocole + modules purs — PARALLÉLISABLES)
   │      └── 9           (pont.rs + main.rs : c'est ELLE qui fait entrer 3-6 dans la suite)
   │             ├── 11   (transport données seules ; exige 2 et 6)
   │             └── 13 → 14   (ProjFS ; 13 exige 12, 14 exige 5, 6, 11, 13)
   │
   ├── 7  (run-agent.sh — dédiée, AVANT 8)
   │      └── 8  (lanceur/pont.rs)
   │             └── 10  (surveillance_pont.rs)
   │
   ├── 12 (chargement.rs — indépendante de tout sauf agent/Cargo.toml)
   │
   ├── 15 → 16            (client ; 15 exige 2)
   │
   └── 17                 (evenements.rs — INDÉPENDANTE, parallélisable partout)

   9, 10, 11, 14, 16, 17  ──> 18  (recette VM : exige TOUT)
                                    └── 19  (revue transverse : DERNIÈRE)
```

**Trois chemins parallèles après la tâche 1** : la famille A (2 à 6, puis 9, 11,
12, 13, 14), la famille B du superviseur (7, 8, 10), et la famille D du client
(15, 16). La tâche 17 est indépendante de tout.

⚠️ **La VM est un état partagé** : les tâches **1** et **18** ne peuvent pas
courir en même temps, ni avec une recette d'un autre chantier. La tâche 12
compile pour Windows mais **ne touche pas la VM** — elle est parallélisable.

⚠️ **Cinq tâches touchent `agent/src/pont.rs`** (9, 11, 13, 14) : elles sont
séquentielles entre elles. **Nommer les fichiers dans chaque `git add`** — un
`git add -A` a déjà emporté le travail concurrent d'une autre tâche dans un
commit qui ne compilait pas.

---

## Divergences relevées entre la spec et le code réel

**Relevées par la commande le 19 août 2026, chacune vérifiée en ouvrant le
fichier.** Aucune n'invalide la conception ; toutes changent quelque chose au
plan.

| # | Divergence | Effet sur ce plan |
| --- | --- | --- |
| 1 | **`cd client && npx vitest run` ne couvre PAS `proto/ts/`** (racine Vitest = `client/`). 107 tests = `client/src/` seuls ; `proto/ts` demande `--dir ../proto` (35 tests). Aucun document du dépôt ne le dit | Tâche 2 : **deux** commandes obligatoires. Sans cela `fichiers.test.ts` n'aurait jamais tourné |
| 2 | Le serveur de signaling a **déménagé** : `signaling/src/` n'existe plus, il est en **`plateforme/src/signaling/`** — périmètre concurrent | Aucune modification n'y est requise : `Appariement.declarer` (`:45-52`) accepte un identifiant de session **arbitraire** et n'admet qu'un `agent` + un `client` par identifiant. D'où `SESSION_DU_PONT = "fichiers"`, **distinct** de `"bureau"` |
| 3 | **`client/src/webrtc.ts::connectSession` est lié au média** : `SessionOptions.video: HTMLVideoElement` (`:10`), `addTransceiver('video')` (`:202`) et `('audio')` (`:206`) inconditionnels. La spec §3.4 ne montre ce qui tombe que côté **agent** | Tâche 16 : `canal.ts` est un **connecteur distinct**, pas un paramètre. **Ne pas refactorer `connectSession`** |
| 4 | **`transport::fixtures` est `pub(super)`** (`transport.rs:51-52`, `fixtures.rs:42`) : `pont/` ne peut pas l'employer | Tâche 11 : échafaudage local `#[cfg(test)]`. **Ne pas hisser** `fixtures` — cela toucherait le chemin vidéo pour un besoin de test |
| 5 | La spec §3.1 cite `windows-link-0.2.1/src/lib.rs:9` : c'est la branche **`target_arch = "x86"`** (l. **5-15**). La branche x86_64 est l. **18-28**, `#[link]` à la **l. 22**, **sans** `import_name_type` | Substance intacte (`raw-dylib` dans les deux cas). Tâche 12 cite la bonne ligne |
| 6 | La spec §3.2 dit « **`PONT=0` désactive, et une simple présence n'active pas** » tout en renvoyant à `main.rs:315`. **Les deux clauses se contredisent** : la forme de `:315` active pour toute valeur ≠ `"0"`, présence vide comprise | Le **code** de `:315` fait foi. Écrit dans les Global Constraints |
| 7 | La spec §8 dit « les **cinq** rappels obligatoires **en mode asynchrone** ». Sa propre table §4.3 dit que `StartDirectoryEnumeration` et `EndDirectoryEnumeration` sont **synchrones** | Tâche 13 : **cinq implémentés, trois asynchrones** |
| 8 | La spec §8 exige `ERROR_WRITE_PROTECT` sur toute écriture, **sans dire comment**. `mode: 'read'` côté navigateur ne suffit pas : ProjFS hydrate et l'écriture **réussit localement**, le fournisseur n'étant prévenu qu'à la fermeture du handle | Tâche 13 Step 4 : refus par `PRJ_NOTIFY_FILE_PRE_CONVERT_TO_FULL` (mod.rs:385), qui est une notification **`PRE_`, donc refusable** |
| 9 | La spec §7.4 pose un cache d'énumération dès F1, mais **`Rafraichir` — le seul moyen de l'invalider — est un livrable de F5** | Tâche 14 Step 4 : **aucun cache d'énumération en F1**. Un cache que rien ne vide reproduirait le défaut de l'ancien pont (`src/file.js:232-241`, cache **sans TTL**) |
| 10 | La spec §4.3 nomme le contrôle de flux par `bufferedAmount`/`SEUIL_TAMPON` sans dire de quel sous-bloc il relève ; le §8 le range en **F3** | Tâche 14 Step 2 : **un morceau en vol à la fois** en F1. Déclaré, pas implémenté à moitié |
| 11 | La spec ne prévoit rien pour la **casse** : Windows est insensible, la FSA ne l'est pas | Tâche 3 : documenté et remonté en `Introuvable` ; **legs**, exercé en recette (tâche 18 Step 7) |
| 12 | **Vérifications favorables** : les 13 entrées, les 8 types `PRJ_*_CB`, les 19 entrées `Prj*`, les 621 lignes du module, `Win32_Storage_ProjectedFileSystem` (Cargo.toml:554), `evenements.rs:168-189` et `:51-63`, `main.rs:315`, `lanceur.rs:156-188`, `proto/src/control.rs:14,37-40,41-52`, `input.rs:14,98,145`, `signaling.rs:53`, `initialisation.rs:27`, et les tailles du §9 (328 / 367 / 263 / 148 / 4 / 71 / 93) sont **toutes exactes** | ❌ **« TOUTES EXACTES » EST FAUX D'AU MOINS UNE** : `evenements.rs:168-189` ne désignait pas `dispatch_channel_data`, qui vivait en `:219-239` — relevé par la tâche 17 elle-même (message de `df7fd4d`), **sans que ce tableau soit corrigé**. Le FOND de la citation, lui, était exact. *Une ligne de tableau qui affirme la complétude d'une vérification est une affirmation de complétude comme une autre.* |

---

## Ce que F0 et F1 n'établiront PAS

Écrit d'avance, pour qu'aucun document de résultats n'ait à le découvrir.

- **Aucun taux.** Deux exécutions par critère ; deux exécutions ne font pas une
  fréquence.
- **Aucune constante n'est calibrée** : `TAILLE_TRAME_MAX`, `DELAI_ATTRIBUTS`,
  `DELAI_LIRE`, `DELAI_LISTER`. Elles rejoignent `BPP_MIN`, `FACTEUR_FOCUS`,
  `PART_DORMANTE_BPS`, `HYSTERESIS`, `REPIT_APRES_ECHEC`, `TAILLE_MAX_SORTIE`,
  `REPIT_REARMEMENT_AUDIO` et `REARMEMENTS_MAX`. **C'est F4 qui donnera de quoi
  calibrer les trois délais** ; jusque-là leur documentation le dit.
- **Aucune latence n'est mesurée** — c'est l'objet de F4, et c'est le risque R2
  de la spec : le lecteur peut fonctionner et rester inutilisable.
- **Aucun test d'hôte ne couvre `pont/projfs.rs`**, `#[cfg(windows)]` et appelé
  par le système. `cargo check --target x86_64-pc-windows-gnu` en vérifie les
  types, les emprunts et les durées de vie — **jamais le comportement**.
- **Les cinq entrées sans type `PRJ_*_CB` jumeau** (`PrjWriteFileData`,
  `PrjCompleteCommand`, `PrjFillDirEntryBuffer`, `PrjAllocateAlignedBuffer`,
  `PrjFreeAlignedBuffer`, entre autres) **ne sont couvertes que par la
  relecture** : R7 reste ouvert.
- **Rien de l'écriture** : F1 est en lecture seule par périmètre.
- **Rien d'un client réel** : Chrome sans interface, décodage logiciel, sur
  l'hôte qui porte la VM. La File System Access API n'existe ni sur Firefox ni
  sur Safari — limite du **produit**, héritée du cadrage.
- **Rien de plusieurs utilisateurs** : une VM, un utilisateur, une racine, et
  `SESSION_DU_PONT` n'est pas namespacé.
- **Rien du comportement à travers une reconnexion WebRTC** : le canal serait
  rétabli par une nouvelle `PeerConnection`, jamais par renégociation — le
  chemin n'existe pas plus ici que dans le reste du produit
  (`agent/src/demarrage.rs:252` journalise « aucune renégociation possible pour
  cette session »).
- **Rien de la casse** : le legs de la tâche 3 est **observé**, pas résolu.
- **Rien de l'occupation disque au-delà de la trace** : aucune politique
  d'éviction, et c'est déclaré (R4).
- **Rien de la concurrence pour le lien réseau** : `BUDGET_BPS` (D6) **ne
  connaît pas ce trafic-ci**, et le contrôle de flux de F3 bornera la file SCTP,
  **pas le débit** (R5).
- **Rien de l'ancien pont** : il n'est ni modifié, ni retiré, ni comparé chiffre
  à chiffre.

---

## Risques qui rendraient F1 non livrable

| # | Risque | Ce qu'on en sait, et la parade |
| --- | --- | --- |
| **R1** | 🔴 **ProjFS ne s'active pas, ou exige un redémarrage impossible à obtenir** | **Éliminatoire.** Traité en premier (tâche 1), et **son état rouge est l'état constaté d'aujourd'hui**. **Repli : AUCUN.** Si la tâche 1 échoue, le sous-projet est non livrable et les tâches 2 à 19 ne doivent pas être entamées |
| **R1bis** | 🔴 **Le pilote `PrjFlt` est présent sur le disque mais non chargé** | Non couvert par la spec. Ajouté au critère F0 (tâche 1, Step 5, condition 3). Coût : une ligne. Coût de son absence : une tâche 13 inexplicablement rouge |
| **R7** | **Les treize signatures transcrites divergent de l'ABI réelle** | Trois pièges nommés et relevés (tâche 12, Step 3), test de fumée nommant l'entrée manquante, comparaison compilée aux **huit** types `PRJ_*_CB`. **Ce n'est pas une garantie** : cinq entrées n'ont aucun jumeau, et un mauvais `transmute` de fonction est un défaut que rien n'attrape avant l'exécution |
| **R6** | **Une panique dans un rappel abat le processus pont** | `catch_unwind` à chaque frontière FFI, et **D2 fait que le pire cas est la mort du pont**, relancé par le superviseur (tâche 10), la vidéo intacte. C'est ce que la recette provoque (tâche 18, Step 6 (i)) |
| **R2** | **La latence rend le lecteur inutilisable en pratique** | Parades posées dès F1 : cache négatif, rappels qui ne bloquent jamais. **Non mesuré** — c'est F4. Si F4 conclut à l'inutilisabilité, F1 reste livré (le lecteur fonctionne) |
| **N1** | **`clear_codecs()` sans aucun `enable_*` pourrait être refusé par str0m 0.21** | Tâche 11, Step 1. Parade : laisser `enable_h264(true)` sans négocier de piste. **À signaler, pas à contourner en silence** |
| **N2** | **Le renommage de `ProjectedFSLib.dll` peut être refusé par Windows Resource Protection** | Tâche 18, Step 3. Si le renommage échoue, **le dire** et replier sur un contrôle plus faible — **jamais déclarer le contrôle passé** |
| **N3** | **Le plafond de `superviseur/lanceur.rs` (marge 133)** | Traité **avant** l'addition, par `lanceur/pont.rs` (tâche 8) : la marge n'est pas prise du tout |
| **N4** | **La VM se met en veille prolongée toute seule** | Piège documenté. **Vérifier que la VM a survécu après toute séquence longue** (`grep -E "terminating on signal" /var/log/libvirt/qemu/Windows.log`) |
