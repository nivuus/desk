# Dette de taille — ramener l'agent sous la limite de 500 lignes

**Date** : 30 juillet 2026
**Statut** : Spécification validée, prête à planifier
**Portée** : Découper les quatre fichiers Rust en dette que la suite de tests
Linux couvre — `congestion.rs`, `transport.rs`, `gamepad.rs`, `main.rs` — sans
changer un seul comportement

---

## 1. Objectif

`CLAUDE.md` fixe désormais une limite de **500 lignes par fichier de code
source**, tests inline compris. Sept fichiers la dépassent, tous dans
`agent/src/`, pour 8233 lignes cumulées :

| Fichier | Lignes | Dont tests | Testable sur Linux |
| --- | --- | --- | --- |
| `transport.rs` | 2558 | 976 | oui |
| `encode.rs` | 1480 | 0 | non (`#[cfg(windows)]`) |
| `main.rs` | 1342 | 78 | partiellement |
| `congestion.rs` | 990 | 487 | oui |
| `windows_source.rs` | 721 | 0 | non (`#[cfg(windows)]`) |
| `gamepad.rs` | 599 | ~99 | partiellement |
| `wasapi.rs` | 543 | 0 | non (`#[cfg(windows)]`) |

Ce chantier traite les **quatre fichiers que les tests couvrent** —
`transport.rs`, `main.rs`, `congestion.rs`, `gamepad.rs`, soit 5489 lignes. Les
trois modules Windows-only sans aucun test (`encode.rs`, `windows_source.rs`,
`wasapi.rs`, 2744 lignes) restent en **dette assumée** : les découper se ferait
sans filet automatisé, et le gain de conformité ne vaut pas le risque de
régression silencieuse dans la capture, l'encodage et l'audio.

---

## 2. Décisions actées

1. **Les tests inline comptent dans la limite.** Un fichier se juge à ce qu'on
   ouvre, pas à ce qui compile en `release`. Conséquence directe :
   `congestion.rs`, le fichier le mieux testé du dépôt (49 % de tests), est en
   dette alors que son code seul tient en 503 lignes.

2. **Périmètre restreint aux quatre fichiers testables.** Voir §1.

3. **Aucun changement de comportement.** Ni signature publique modifiée, ni
   logique retouchée, ni test ajouté ou supprimé. Seule dérogation admise : la
   visibilité d'un item devenu inter-modules, élargie au minimum strict
   (`pub(super)`, à défaut `pub(crate)`).

4. **Ordre par risque de vérification, non par taille** : `congestion.rs` →
   `transport.rs` → `gamepad.rs` → `main.rs`. Justifié au §5.

5. **Un fichier traité, un commit**, chacun vert au sens du §5.

---

## 3. Ce que la lecture du code a établi

### 3.1 La vérification croisée vers Windows depuis Linux est impossible

La cible `x86_64-pc-windows-msvc` est installée, mais
`cargo check --target x86_64-pc-windows-msvc` échoue à la compilation
d'`aws-lc-sys` — dépendance C de rustls, tirée par str0m — qui exige un
compilateur C ciblant Windows, absent de cette machine.

**Le code sous `#[cfg(windows)]` n'a donc qu'un seul vérificateur :
`scripts/build-agent.sh`, qui compile sur la VM.** Cette contrainte gouverne
l'ordre de traitement et la définition de « terminé ».

### 3.2 La couverture Linux ne suit pas la taille des fichiers

| Fichier | Code Windows-only | Vérifié par `cargo test` sur Linux |
| --- | --- | --- |
| `congestion.rs` | aucun | **100 %** |
| `transport.rs` | `TimerResolutionGuard` seul (~75 lignes) | **~97 %** |
| `gamepad.rs` | `mod win` + `probe` (~400 lignes) | ~33 % |
| `main.rs` | 32 blocs, dont `CAPTURE_TEST` et l'essentiel du démarrage | ~30 % |

Le plus gros fichier est presque intégralement couvert ; les deux plus petits
ne le sont presque pas. Classer par taille aurait conduit à traiter les deux
fichiers les plus opaques avant celui que les tests protègent.

### 3.3 `main()` est à 75 % du diagnostic

`async fn main()` fait 1024 lignes. Cinq modes de diagnostic activés par
variable d'environnement en occupent ~750 : `CAPTURE_TEST` (439 lignes à elle
seule), `AUDIO_PROBE`, `PROCESS_LOOPBACK_PROBE`, `VIGEM_PROBE`,
`INPUT_LINEARITY_PROBE`. La mise en route réelle de session tient dans les ~380
dernières lignes.

### 3.4 `act_on_timeout` est déjà écrite comme une liste de priorités

Le `impl Session` de `transport.rs` fait 1075 lignes, dont 445 pour la seule
`act_on_timeout`. Cette fonction est structurée en branches que ses propres
commentaires numérotent : a0 drainage dû, a0bis contrôle hors boucle, a message
en attente, a0ter décision d'adaptation, a1 redimensionnement, a2 fenêtre
disparue, a3 paquet audio, puis la vidéo. Le découpage suit cette structure
existante au lieu d'en inventer une.

### 3.5 Les tests d'intégration de `Session` ne peuvent pas sortir du module

`agent` est un binaire, pas une bibliothèque : le répertoire `tests/` de Cargo
ne peut atteindre aucun de ses modules. Les ~770 lignes de tests d'intégration
de `transport.rs` — pair local factice, sources factices — doivent rester dans
l'arborescence du module.

---

## 4. Le découpage

### 4.1 Convention

**Modules répertoire, sans `mod.rs`** (édition 2021) : `congestion.rs` cohabite
avec un répertoire `congestion/` frère. C'est une nouveauté pour ce dépôt —
`agent/src/` est plat aujourd'hui, 22 fichiers. Quatre répertoires y
apparaîtront.

**Les tests suivent le code qu'ils testent.** Pas d'extraction mécanique vers un
`*_tests.rs` : chaque sous-module garde son propre `#[cfg(test)] mod tests`
portant les tests de ses items. Deux raisons. Les tests conservent ainsi l'accès
aux items privés de leur module sans qu'on élargisse des visibilités « pour les
tests » — un `pub(crate)` de complaisance est une perte d'encapsulation réelle
en échange d'une conformité cosmétique. Et la répartition tombe juste d'elle-même :
les 976 lignes de tests de `transport.rs` se distribuent sur cinq sous-modules.

### 4.2 `congestion.rs` (990) → quatre fichiers

| Fichier | Contenu | ≈ |
| --- | --- | --- |
| `congestion.rs` | doc, types de contrat (`Config`, `Observation`, `Qualite`, `Adaptation`, `Decision`), re-exports | 90 |
| `congestion/echelle.rs` | `Barreau`, `Echelle` + 3 tests | 210 |
| `congestion/hysteresis.rs` | `Hysteresis` + 6 tests | 175 |
| `congestion/controleur.rs` | `Controleur`, `ecart_relatif` + 14 tests | ~515 |

Le dernier passe de peu au-dessus de la limite. Les trois tests de
`changer_source` (~80 lignes) sont le candidat naturel à séparer — ils portent
sur la reconfiguration à la volée, non sur l'asservissement. **À trancher au
moment du plan**, une fois les lignes réelles mesurées.

### 4.3 `transport.rs` (2558) → six fichiers

| Fichier | Contenu | ≈ |
| --- | --- | --- |
| `transport.rs` | doc de module (l'invariant str0m), constantes, `struct Session`, `new`, `accept_offer`, accesseurs, `run` | 300 |
| `transport/tick.rs` | `act_on_timeout` décomposée en méthodes privées par branche, `Tick`, `next_frame_deadline` + tests de cadence | 450 |
| `transport/media.rs` | `write_frame`, `write_audio`, sélection de payload type, `capture_instant`, `CandidatePt`, `select_h264_pt` + tests | 450 |
| `transport/evenements.rs` | `handle_event`, `dispatch_channel_data`, `queue_control`, `begin_ending`, `drain_quietly` + tests | 400 |
| `transport/socket.rs` | `RecvErrorAction`, `classify_recv_error`, `recv_error_backoff`, `TimerResolutionGuard`, `bounded_wait` + tests | 230 |
| `transport/fixtures.rs` | `#[cfg(test)]` : pair local factice et sources factices, aujourd'hui dupliqués entre tests | 250 |

**`transport/tick.rs` est le seul endroit où la règle « déplacement pur » se
tend.** Déplacer `act_on_timeout` telle quelle donnerait un fichier de ~490
lignes sans place pour ses tests : elle doit être décomposée en méthodes privées
nommées, une par branche. C'est une édition réelle et non un copier-coller —
d'où l'ordre du §5, qui la place derrière un cas d'échauffement et devant les
deux fichiers mal couverts.

### 4.4 `gamepad.rs` (599) → trois fichiers

L'en-tête du fichier annonce déjà ses trois natures ; il suffit de les acter.

| Fichier | Contenu | ≈ |
| --- | --- | --- |
| `gamepad.rs` | `plus_recent`, `LimiteurVibration`, `PERIODE_MIN` + tests | 200 |
| `gamepad/win.rs` | `VirtualPad`, `spawn_rumble` (`#[cfg(windows)]`) | 300 |
| `gamepad/probe.rs` | la sonde ViGEm (`#[cfg(windows)]`) | 105 |

### 4.5 `main.rs` (1342) → sept fichiers

| Fichier | Contenu | ≈ |
| --- | --- | --- |
| `main.rs` | déclarations de modules, `Config`/`config()`, `main()` réduite à : neutralisation pointeur, config, aiguillage, démarrage | 215 |
| `demarrage.rs` | l'assemblage réel : capture → encodeur → source → `Session` → signalisation, `watch_encoder` | 380 |
| `diagnostics.rs` | l'aiguillage des cinq sondes | 60 |
| `diagnostics/capture.rs` | `CAPTURE_TEST` | 440 |
| `diagnostics/pixels.rs` | `read_pixel`, `capture_center_pixel` | 95 |
| `diagnostics/entree.rs` | `INPUT_LINEARITY_PROBE`, `VIGEM_PROBE`, `point_depart_lineaire` + les 7 tests actuels de `main.rs` | 165 |
| `diagnostics/audio.rs` | `AUDIO_PROBE`, `PROCESS_LOOPBACK_PROBE` | 45 |

---

## 5. Ordre et vérification

### 5.1 Ordre

`congestion.rs` → `transport.rs` → `gamepad.rs` → `main.rs`.

Croissant en risque, au sens du §3.2 : on commence par le fichier que les tests
couvrent entièrement, on finit par ceux qu'ils ne voient presque pas. Effet de
bord recherché : `transport.rs` — le fichier que le chantier D devra rouvrir
pour la répartition de capacité entre flux — est traité tôt, et non en fin de
chantier.

### 5.2 Définition de « terminé », par commit

Le garde-fou est **`scripts/verify-all.sh`**, pas `cargo test` seul.

1. `cargo test --workspace` → **142 passés, 0 échec**, à l'identique de la ligne
   de base mesurée le 30 juillet 2026 (2,08 s).
2. `cargo clippy --workspace` → pas plus que les **31 avertissements
   `dead_code` préexistants**. Ce compteur mérite attention ici précisément :
   déplacer un item privé vers un autre module change sa visibilité, donc ce que
   clippy voit. Tout avertissement nouveau doit être expliqué, jamais absorbé.
3. `tsc --noEmit` (client puis proto) → inchangé. Ce chantier ne touche pas au
   TypeScript ; l'étape sert de témoin.
4. **Pour `gamepad.rs` et `main.rs` uniquement** : `scripts/build-agent.sh`
   réussit sur la VM. Ces deux commits ne sont pas terminés sans elle allumée —
   et elle ne démarre pas toute seule (`virsh start Windows`).

### 5.3 Définition de « terminé » pour le chantier

Les quatre fichiers, et tous les fichiers issus de leur découpage, sous 500
lignes. Vérifié par la commande consignée dans `CLAUDE.md` :

```bash
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
```

Sa sortie doit se réduire à trois lignes : `encode.rs`, `windows_source.rs`,
`wasapi.rs`. Le tableau de dette de `CLAUDE.md` est mis à jour en conséquence
dans le dernier commit.

---

## 6. Hors périmètre

Nommé ici pour que ça ne dérive pas dans le chantier :

- **Écrire les tests manquants** de `main.rs` et `transport.rs`. Ces fichiers
  sont sous-testés au regard de leur taille, et on aura le code sous les yeux —
  la tentation sera réelle. Mêler l'écriture de tests au déplacement ferait
  perdre la seule propriété qui rend ce découpage sûr : qu'un test rouge ne peut
  signifier qu'une chose, que le déplacement est fautif. Chantier légitime, mais
  séparé.
- **Les trois fichiers Windows-only** (`encode.rs`, `windows_source.rs`,
  `wasapi.rs`, 2744 lignes), pour la raison du §1.
- **Toute amélioration de la logique** rencontrée en chemin. Elle se note, elle
  ne se fait pas ici.

---

## 7. Risques

| Risque | Traitement |
| --- | --- |
| La décomposition d'`act_on_timeout` change un ordre de priorité entre branches | Les 142 tests, dont ceux de cadence et d'adaptation, tournent à chaque étape ; la structure suivie est celle que les commentaires du code décrivent déjà (§3.4) |
| Un item privé élargi en `pub(crate)` par facilité, érodant l'encapsulation | §2.3 : minimum strict, `pub(super)` d'abord ; à relire explicitement en revue |
| Le déplacement de code Windows-only casse la compilation sur la VM | §5.2.4 : `build-agent.sh` obligatoire pour `gamepad.rs` et `main.rs` |
| `congestion/controleur.rs` reste au-dessus de 500 après découpage | Anticipé au §4.2 ; séparation des tests de `changer_source` tenue en réserve |
| Nouveaux avertissements `dead_code` masquant une vraie régression | §5.2.2 : compteur de référence à 31, tout écart expliqué |
