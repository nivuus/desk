# Sous-bloc D10 — solder la branche : plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Solder les dix legs que le sous-bloc D9 laisse, en commençant par celui
qui plafonne le produit à trois fenêtres.

**Architecture:** La sortie virtuelle cesse d'*être* la fenêtre. Une sortie née
à une taille imposée par le registre est **acceptée** ; la fenêtre y est posée à
la taille retenue, et la capture recadre **dans la duplication de cette sortie**
— jamais dans celle du bureau. En parallèle, la capture audio morte devient
reconstructible, et quatre legs froids sont soldés.

**Tech Stack:** Rust (crate `agent`, cible réelle `x86_64-pc-windows-msvc` sur la
VM), TypeScript côté `client/`, PowerShell distant via `scripts/winrm.js`.

**Spec :** `docs/superpowers/specs/2026-08-07-multifenetres-solder-la-branche-design.md`

---

## Global Constraints

- **Plafond de 500 lignes** par fichier de code source écrit à la main. Trois
  fichiers du chemin sont à marge étroite et reçoivent une **extraction dédiée
  AVANT** toute addition : `superviseur/boucle.rs` (**492**),
  `transport/tick/tests.rs` (**489**), `windows_audio.rs` (**479**).
  `superviseur/table.rs` (**494**, marge **6**) n'est pas sur le chemin prévu :
  s'il faut y toucher, extraction d'abord, sans exception.
- **Jamais `git add -A`** : nommer les fichiers. Un `git add -A` a déjà emporté
  le travail concurrent d'une autre tâche dans un commit qui ne compilait pas.
- **Vérification sur l'hôte avant toute compilation distante** :
  `cd agent && cargo check --target x86_64-pc-windows-gnu` (couvre types,
  emprunts, visibilités, durées de vie ; **pas l'édition de liens**).
- **Tests d'hôte** : `cd agent && cargo test -p agent`. Référence d'entrée :
  **440 passed, 0 failed**. Côté client : `cd client && npx vitest run`,
  référence **107 passed**.
- **Un contrôle qu'on n'a jamais vu rouge n'est pas un contrôle.** Chaque test
  neuf doit être exécuté **avant** l'implémentation et vu échouer, et le message
  d'échec attendu est écrit dans le plan.
- **Toute variable d'environnement neuve doit être ajoutée explicitement à
  `scripts/run-agent.sh`** — piège payé en D1 (`SUPERVISEUR`), D2
  (`MULTIFENETRE_REPRISE`) et D7 (`AUDIO`). L'agent démarre sans elle et **sans
  rien signaler**.
- **Convention des variables de banc** (`PART_SONDAGE`, `PLEIN_ECRAN`) : `=0`
  désarme, une simple présence n'active pas. Ne jamais tester `is_ok()`.
- **La VM n'est pas démarrée automatiquement.** Avant toute tâche de recette :
  `virsh list --all`, `virsh start Windows`, puis attendre WinRM **et** un accès
  réel à `/media/vm` (`until ls /media/vm/dev`), et `set -a && source .env && set +a`
  avant `scripts/build-agent.sh` — sans quoi il s'arrête **en silence**.
- **Aucun taux ne sera revendiqué.** Chaque énoncé de résultat porte son nombre
  d'exécutions.

---

## Structure des fichiers

**Créés :**

| Fichier | Responsabilité |
| --- | --- |
| `agent/src/superviseur/boucle/creation_sortie.rs` | `creer_sortie` et ses aides, extraites de `boucle.rs` |
| `agent/src/transport/tick/tests/audio.rs` | les tests de tick touchant l'audio |
| `agent/src/transport/tick/tests/reste.rs` | le reste des tests de tick |
| `agent/src/windows_audio/fil.rs` | le corps du fil de capture WASAPI |

**Modifiés :** `agent/src/superviseur/placement.rs`,
`agent/src/superviseur/boucle.rs`, `agent/src/superviseur/table/attribution.rs`,
`agent/src/superviseur/enfants.rs`, `agent/src/superviseur/lanceur.rs`,
`agent/src/windows_source/sortie.rs`, `agent/src/capteur/protocole.rs`,
`agent/src/capteur/fenetre.rs`, `agent/src/capteur/serveur.rs`,
`agent/src/capteur/sommeil.rs`, `agent/src/capteur/sommeil/porteurs.rs`,
`agent/src/capteur/distante.rs`, `agent/src/source.rs`,
`agent/src/audio.rs`, `agent/src/windows_audio.rs`,
`agent/src/demarrage/audio.rs`, `agent/src/demarrage.rs`,
`agent/src/transport/piste_audio.rs`, `agent/src/transport/tick.rs`,
`agent/src/survie_verdict.rs`, `scripts/run-agent.sh`, `CLAUDE.md`.

---

## Interfaces partagées

Ces signatures sont **fixées ici** : les tâches les consomment telles quelles.

```rust
// agent/src/superviseur/placement.rs
pub fn sortie_assez_grande(sortie: (u32, u32), demandee: (u32, u32)) -> bool;
pub fn taille_retenue(demandee: (u32, u32), sortie: (u32, u32)) -> (u32, u32);
pub fn sortie_pour_viewport(
    sorties: &[SortieDxgi],
    largeur: u32,
    hauteur: u32,
    deja_prises: &[String],
) -> Option<SortieDxgi>;

// agent/src/windows_source/sortie.rs  (déjà présent, sans appelant)
pub fn borner_a_la_taille_max((l, h): (u32, u32)) -> (u32, u32);

// agent/src/windows_source/sortie.rs  (signature ÉLARGIE en tâche 7)
impl WindowsSource {
    pub fn sur_sortie(
        hwnd: HWND,
        nom_sortie: &str,
        taille: (u32, u32),
        fps: u32,
        bitrate: u32,
        clock_origin: std::time::Instant,
    ) -> Result<Self>;
}

// agent/src/audio.rs
pub type Reconstructeur =
    Box<dyn Fn() -> anyhow::Result<Box<dyn AudioSource + Send>> + Send>;
pub const RECONSTRUCTIONS_MAX: u32 = 3;
pub const REPIT_RECONSTRUCTION: std::time::Duration =
    std::time::Duration::from_secs(2);

// agent/src/transport/piste_audio.rs
impl Session {
    pub fn set_audio_reconstructeur(&mut self, r: crate::audio::Reconstructeur);
    pub(super) fn reconstruire_ou_signaler(&mut self, maintenant: std::time::Instant) -> bool;
}

// agent/src/source.rs — trait VideoSource
fn signaler_audio_vivant(&mut self) {}

// agent/src/capteur/protocole.rs
enum VersCapteur { /* … */ AudioVivant }
```

---

# Famille 0 — les extractions, avant toute addition

### Task 1 : extraire `creer_sortie` de `superviseur/boucle.rs`

**Files:**
- Create: `agent/src/superviseur/boucle/creation_sortie.rs`
- Modify: `agent/src/superviseur/boucle.rs` (retirer l. 261-378 et l. 414-484)

**Interfaces:**
- Consumes: rien.
- Produces: `pub(super) fn creer_sortie(...)`, `pub(super) fn rendre_la_sortie(...)`,
  signatures **inchangées** par rapport à leur forme actuelle dans `boucle.rs`.

- [ ] **Step 1 : relever la taille de départ**

```bash
cd agent && wc -l src/superviseur/boucle.rs
```
Attendu : `492`.

- [ ] **Step 2 : créer le fichier enfant avec son commentaire de tête**

```rust
//! La création d'une sortie virtuelle pour une session, et sa restitution au
//! pilote.
//!
//! Extrait de `boucle.rs` (tâche 1 du sous-bloc D10) pour rester sous le
//! plafond de 500 lignes du projet — **avant** l'addition qui l'aurait fait
//! franchir, et non après. C'est le seul geste qui a fonctionné en D9
//! (`capteur/serveur/instances.rs`) ; les deux fichiers traités après coup y
//! ont été compressés, geste que `CLAUDE.md` interdit, puis extraits quand
//! même.
//!
//! Aucune décision ici — la table décide, ce module agit —, exactement comme
//! le module parent.

use super::*;
```

- [ ] **Step 3 : déplacer les fonctions, sans en changer une ligne**

Couper de `boucle.rs` et coller dans `creation_sortie.rs`, **verbatim,
commentaires compris** : `fn creer_sortie` (l. 261-378), `fn rendre_sans_apparier`
(l. 379-413), `fn attendre_une_sortie_neuve` (l. 414-450), `fn rendre_la_sortie`
(l. 451-484). Changer leur visibilité en `pub(super)` pour `creer_sortie` et
`rendre_la_sortie` (appelées depuis `boucle.rs`) ; laisser les deux autres
privées au module.

Dans `boucle.rs`, ajouter à côté des deux `mod` existants (l. 485 et 492) :

```rust
mod creation_sortie;
use creation_sortie::{creer_sortie, rendre_la_sortie};
```

- [ ] **Step 4 : vérifier que rien n'a bougé**

```bash
cd agent && cargo check --target x86_64-pc-windows-gnu && cargo test -p agent
```
Attendu : sortie 0 ; **440 passed, 0 failed**. Une extraction ne change aucun
comportement : tout autre nombre est un défaut de déplacement.

- [ ] **Step 5 : relever la marge rendue**

```bash
cd agent && wc -l src/superviseur/boucle.rs src/superviseur/boucle/creation_sortie.rs
```
Attendu : `boucle.rs` autour de **380** (marge ~120), l'enfant autour de **120**.
Reporter les deux chiffres **relevés** dans le message de commit.

- [ ] **Step 6 : commit**

```bash
git add agent/src/superviseur/boucle.rs agent/src/superviseur/boucle/creation_sortie.rs
git commit -m "extrait(d10): creer_sortie sort de boucle.rs, avant l'addition"
```

---

### Task 2 : extraire les tests de `transport/tick/tests.rs`

**Files:**
- Create: `agent/src/transport/tick/tests/audio.rs`, `agent/src/transport/tick/tests/reste.rs`
- Modify: `agent/src/transport/tick/tests.rs`

**Interfaces:**
- Consumes: rien.
- Produces: rien de public — seul le découpage change.

- [ ] **Step 1 : relever la taille de départ**

```bash
cd agent && wc -l src/transport/tick/tests.rs
```
Attendu : `489` (marge 11).

- [ ] **Step 2 : inventorier les tests audio**

```bash
cd agent && grep -n "fn .*audio\|audio_mort" src/transport/tick/tests.rs
```
Tous les tests dont le nom porte `audio` partent dans `tests/audio.rs` ; les
autres dans `tests/reste.rs`. Les aides communes (constructeurs de source
factice, etc.) restent dans `tests.rs`, qui devient le module de tête.

- [ ] **Step 3 : réduire `tests.rs` à son rôle de tête**

```rust
//! Tests du tick de transport, répartis par famille de branche.
//!
//! Découpé à la tâche 2 du sous-bloc D10 : le fichier était à 489 lignes
//! (marge 11) et la tâche 11 y ajoute la couverture de la reconstruction
//! audio. L'extraction précède l'addition — voir `CLAUDE.md`.
//!
//! Les aides partagées restent ici ; les `use super::*;` des deux enfants les
//! atteignent.

mod audio;
mod reste;

use super::*;
// … les aides communes, déplacées telles quelles …
```

- [ ] **Step 4 : vérifier qu'aucun test n'a disparu**

```bash
cd agent && cargo test -p agent 2>&1 | tail -3
```
Attendu : **440 passed, 0 failed**. ⚠️ **Un test perdu dans un déplacement se lit
comme un succès** : comparer le nombre, pas seulement l'absence d'échec.

- [ ] **Step 5 : commit**

```bash
git add agent/src/transport/tick/tests.rs agent/src/transport/tick/tests/audio.rs agent/src/transport/tick/tests/reste.rs
git commit -m "extrait(d10): les tests du tick se separent par famille"
```

---

### Task 3 : extraire le fil de capture de `windows_audio.rs`

**Files:**
- Create: `agent/src/windows_audio/fil.rs`
- Modify: `agent/src/windows_audio.rs`

**Interfaces:**
- Consumes: rien.
- Produces: `pub(super) fn tourner(...)` — le corps du fil, dont la signature
  reprend **exactement** les captures actuelles de la fermeture passée à
  `std::thread::spawn` dans `WindowsAudioSource::pour_processus` / `new`.

- [ ] **Step 1 : relever la taille de départ**

```bash
cd agent && wc -l src/windows_audio.rs
```
Attendu : `479` (marge 21). `CLAUDE.md` nomme ce point de chute depuis D7 :
« toute addition future à ce fichier appelle une extraction, jamais une
compression ; son point de chute est `agent/src/windows_audio/`, en commençant
par le corps du fil de capture. »

- [ ] **Step 2 : créer l'enfant**

```rust
//! Le corps du fil de capture WASAPI.
//!
//! Extrait de `windows_audio.rs` à la tâche 3 du sous-bloc D10, au point de
//! chute que `CLAUDE.md` nomme depuis le sous-bloc D7. L'extraction précède
//! l'addition de la tâche 13 (`AUDIO_FAUTE_LECTURE`).
//!
//! `#![cfg(windows)]` comme son parent : aucun test d'hôte ne peut le couvrir,
//! et c'est précisément pourquoi la tâche 13 lui donne une injection de faute.

#![cfg(windows)]

use super::*;
```

- [ ] **Step 3 : déplacer le corps de la fermeture du fil**

Transformer la fermeture passée à `std::thread::spawn` en appel à
`fil::tourner(...)`, en passant explicitement **chaque valeur aujourd'hui
capturée** (dont `capture_morte_fil`, l'anneau de paquets, et le compteur
`lectures_echouees`). Ne changer **aucune** logique : les deux `return`
définitifs (l. 366 et 414 avant déplacement) partent tels quels.

- [ ] **Step 4 : vérifier**

```bash
cd agent && cargo check --target x86_64-pc-windows-gnu && cargo test -p agent
```
Attendu : sortie 0 ; 440 passed. ⚠️ Ce fichier est `#[cfg(windows)]` : sur
l'hôte, seul `cargo check --target …-gnu` le compile réellement. Un `cargo test`
vert **ne prouve rien** de ce déplacement.

- [ ] **Step 5 : relever, puis commit**

```bash
cd agent && wc -l src/windows_audio.rs src/windows_audio/fil.rs
git add agent/src/windows_audio.rs agent/src/windows_audio/fil.rs
git commit -m "extrait(d10): le fil de capture WASAPI sort de windows_audio.rs"
```

---

# Famille ① — la sortie cesse d'être la fenêtre (legs 4 et 5)

### Task 4 : la logique pure — `sortie_assez_grande` et `taille_retenue`

**Files:**
- Modify: `agent/src/superviseur/placement.rs`
- Test: `agent/src/superviseur/placement.rs` (module `tests_taille`)

**Interfaces:**
- Consumes: `TOLERANCE_PX` (privé au module, l. 36).
- Produces: `pub fn sortie_assez_grande(sortie: (u32,u32), demandee: (u32,u32)) -> bool`,
  `pub fn taille_retenue(demandee: (u32,u32), sortie: (u32,u32)) -> (u32,u32)`.

- [ ] **Step 1 : écrire les tests, qui doivent échouer**

À ajouter dans `mod tests_taille` :

```rust
    /// Le fait produit de D9 : sur cette VM, les sorties naissent à 3840×2160
    /// parce que le registre y est resté. `taille_compatible` refusait, et le
    /// produit plafonnait à trois fenêtres.
    #[test]
    fn une_sortie_nee_trop_grande_convient_desormais() {
        assert!(sortie_assez_grande((3840, 2160), (1280, 720)));
    }

    #[test]
    fn une_sortie_nee_trop_petite_ne_convient_pas() {
        assert!(!sortie_assez_grande((1024, 576), (1280, 720)));
    }

    /// La course de rattachement de D1 (1280×713 rendue 1280×720) reste
    /// couverte : quatre pixels de tolérance, comme le replacement.
    #[test]
    fn un_manque_de_quatre_pixels_reste_accepte() {
        assert!(sortie_assez_grande((1276, 716), (1280, 720)));
    }

    #[test]
    fn un_manque_de_sept_pixels_est_refuse() {
        assert!(!sortie_assez_grande((1280, 713), (1280, 720)));
    }

    #[test]
    fn la_taille_retenue_recadre_une_sortie_trop_grande() {
        assert_eq!(taille_retenue((1280, 720), (3840, 2160)), (1280, 720));
    }

    /// Née trop petite, la sortie est honorée à ce qu'elle offre : le client
    /// met à l'échelle. Aucun cas ne rend plus une sortie au pilote pour une
    /// question de taille.
    #[test]
    fn la_taille_retenue_se_borne_a_la_sortie_quand_celle_ci_est_plus_petite() {
        assert_eq!(taille_retenue((1280, 720), (1024, 576)), (1024, 576));
    }

    /// L'encodeur NV12 exige des dimensions paires, et une sortie née à une
    /// taille impaire est un cas réel (viewport impair, D1).
    #[test]
    fn la_taille_retenue_est_toujours_paire_et_jamais_nulle() {
        assert_eq!(taille_retenue((1281, 721), (3840, 2160)), (1280, 720));
        assert_eq!(taille_retenue((0, 0), (1280, 720)), (2, 2));
    }

    /// Les axes se bornent SÉPARÉMENT : on recadre, on ne met pas à
    /// l'échelle, donc il n'y a aucun rapport d'aspect à préserver ici —
    /// contrairement à `borner_a_la_taille_max`, qui, lui, redimensionne.
    #[test]
    fn les_deux_axes_se_bornent_separement() {
        assert_eq!(taille_retenue((1920, 720), (1280, 2160)), (1280, 720));
    }
```

- [ ] **Step 2 : les exécuter et les voir rouges**

```bash
cd agent && cargo test -p agent superviseur::placement 2>&1 | tail -20
```
Attendu : ÉCHEC de compilation, `cannot find function 'sortie_assez_grande' in this scope`
et `cannot find function 'taille_retenue' in this scope`.

- [ ] **Step 3 : implémenter**

À insérer après `taille_compatible` (qui reste : `attribution.rs` s'en sert
encore jusqu'à la tâche 6) :

```rust
/// Vrai si une sortie peut servir un viewport donné.
///
/// **Une inégalité, plus une égalité, et c'est tout le sous-bloc D10.** Une
/// sortie virtuelle ne naît PAS à la taille demandée : elle naît à la dernière
/// taille laissée au registre par un `CDS_UPDATEREGISTRY` antérieur (D8,
/// tâche 3bis — confirmé, reproduit, jamais expliqué). Sur cette VM le registre
/// est resté à 3840×2160, et l'égalité à quatre pixels près refusait donc
/// TOUTE sortie : le produit plafonnait à trois fenêtres, aux six exécutions
/// de la recette ③ de D9, sans exception.
///
/// Le produit n'écrit plus au registre depuis D9, mais **rien ne nettoie ce qui
/// y est déjà écrit** — et la portée du blocage (par GUID ou globale) reste
/// inconnue. D'où le choix de tolérer plutôt que de nettoyer : ainsi la
/// question devient **sans objet**, et non résolue.
///
/// La tolérance de `TOLERANCE_PX` est conservée dans le sens du MANQUE, pour la
/// course de rattachement relevée par la recette D1 (sortie créée à 1280×713,
/// rendue à 1280×720 un essai sur deux).
pub fn sortie_assez_grande(sortie: (u32, u32), demandee: (u32, u32)) -> bool {
    let assez = |s: u32, d: u32| s as i64 + TOLERANCE_PX >= d as i64;
    assez(sortie.0, demandee.0) && assez(sortie.1, demandee.1)
}

/// La taille à laquelle la fenêtre est posée, et que la capture recadre.
///
/// `min` axe par axe, **sans préserver le rapport d'aspect** : on recadre une
/// texture, on ne la met pas à l'échelle. C'est l'inverse de
/// `windows_source_sortie::borner_a_la_taille_max`, qui redimensionne et doit
/// donc, lui, préserver ce rapport.
///
/// Dimensions paires (l'encodeur NV12 les exige) et jamais nulles (une boîte
/// vidéo repliée émet `(0, 0)`, cas réel relevé en D8).
pub fn taille_retenue(demandee: (u32, u32), sortie: (u32, u32)) -> (u32, u32) {
    let retenir = |d: u32, s: u32| (d.min(s).max(2)) & !1;
    (retenir(demandee.0, sortie.0), retenir(demandee.1, sortie.1))
}
```

- [ ] **Step 4 : les exécuter et les voir verts**

```bash
cd agent && cargo test -p agent superviseur::placement 2>&1 | tail -5
```
Attendu : PASS pour les huit tests neufs.

- [ ] **Step 5 : commit**

```bash
git add agent/src/superviseur/placement.rs
git commit -m "ajoute(d10): sortie_assez_grande et taille_retenue, purs et testes"
```

---

### Task 5 : l'appariement accepte une sortie plus grande

**Files:**
- Modify: `agent/src/superviseur/placement.rs` (remplacer `sortie_par_dimensions`)
- Test: `agent/src/superviseur/placement.rs` (module `tests`)

**Interfaces:**
- Consumes: `sortie_assez_grande` (tâche 4).
- Produces: `pub fn sortie_pour_viewport(sorties: &[SortieDxgi], largeur: u32, hauteur: u32, deja_prises: &[String]) -> Option<SortieDxgi>`.
  `sortie_par_dimensions` **disparaît** ; son unique appelant est
  `boucle/creation_sortie.rs` (tâche 6).

- [ ] **Step 1 : écrire les tests, qui doivent échouer**

À ajouter dans `mod tests` :

```rust
    /// Le cas produit de D9 : la sortie naît à 3840×2160 pour un viewport de
    /// 1280×720, et doit désormais être appariée.
    #[test]
    fn apparie_une_sortie_nee_beaucoup_plus_grande() {
        let sorties = vec![sortie_nommee("\\\\.\\DISPLAY8", 3840, 2160)];
        let trouvee = sortie_pour_viewport(&sorties, 1280, 720, &[]);
        assert_eq!(trouvee.map(|s| s.nom_sortie), Some("\\\\.\\DISPLAY8".into()));
    }

    #[test]
    fn n_apparie_pas_une_sortie_trop_petite() {
        let sorties = vec![sortie_nommee("\\\\.\\DISPLAY8", 1024, 576)];
        assert!(sortie_pour_viewport(&sorties, 1280, 720, &[]).is_none());
    }

    /// Le filtre sur les sorties DÉJÀ PRISES devient plus important, pas
    /// moins : avec une inégalité, une même grande sortie conviendrait à
    /// toutes les fenêtres, et toutes montreraient la même image.
    #[test]
    fn une_grande_sortie_deja_prise_n_est_pas_reattribuee() {
        let sorties = vec![
            sortie_nommee("\\\\.\\DISPLAY8", 3840, 2160),
            sortie_nommee("\\\\.\\DISPLAY9", 3840, 2160),
        ];
        let trouvee =
            sortie_pour_viewport(&sorties, 1280, 720, &["\\\\.\\DISPLAY8".to_string()]);
        assert_eq!(trouvee.map(|s| s.nom_sortie), Some("\\\\.\\DISPLAY9".into()));
    }

    #[test]
    fn ignore_toujours_une_sortie_non_attachee() {
        // Une sortie que Windows n'a pas rattachée ne peut rien afficher :
        // la prendre donnerait une capture noire, quelle que soit sa taille.
        let toutes = vec![sortie(0, 1, 2400, 3840, 2160, false)];
        assert!(sortie_pour_viewport(&toutes, 1280, 720, &[]).is_none());
    }
```

⚠️ Le test `un_facteur_d_echelle_n_apparie_pas` (l. 230-234) devient **faux par
construction** : une sortie 1920×1080 pour un viewport 1280×720 est désormais
appariée, et c'est le comportement voulu. **Le remplacer**, ne pas le supprimer
en silence, par :

```rust
    /// Le facteur DPI de 1,5 (5120×1440 annoncé par WMI, 3413×960 mesuré par
    /// DXGI) n'est plus un motif de REFUS : une sortie plus grande est
    /// recadrée. Ce qui protégeait contre lui — poser la fenêtre sur une
    /// texture aux mauvaises dimensions — est désormais assuré par
    /// `taille_retenue`, pas par l'appariement.
    #[test]
    fn un_facteur_d_echelle_est_desormais_recadre_et_non_refuse() {
        let sorties = vec![sortie_nommee("\\\\.\\DISPLAY7", 1920, 1080)];
        assert!(sortie_pour_viewport(&sorties, 1280, 720, &[]).is_some());
        assert_eq!(taille_retenue((1280, 720), (1920, 1080)), (1280, 720));
    }
```

- [ ] **Step 2 : les exécuter et les voir rouges**

```bash
cd agent && cargo test -p agent superviseur::placement 2>&1 | tail -20
```
Attendu : `cannot find function 'sortie_pour_viewport' in this scope`.

- [ ] **Step 3 : implémenter**

Remplacer `sortie_par_dimensions` (l. 63-77) par :

```rust
/// Sortie DXGI capable de servir un viewport, parmi celles qui ne sont pas
/// déjà attribuées.
///
/// **`deja_prises` est ce qui empêche l'inégalité de tout casser.** Avec
/// l'égalité d'avant D10, deux fenêtres au même viewport se disputaient déjà
/// une sortie ; avec « au moins aussi grande », une seule grande sortie
/// conviendrait à TOUTES les fenêtres, et toutes montreraient la même image.
/// Le filtre désigne par NOM DXGI (`\\.\DISPLAYn`), stable, et non par un
/// couple d'index d'énumération, positionnel.
///
/// ⚠️ **L'appelant ne doit chercher QUE parmi les sorties APPARUES** (voir le
/// commentaire de `creation_sortie::creer_sortie`) : le viewport annoncé par le
/// navigateur peut égaler la résolution d'un moniteur PHYSIQUE, et l'inégalité
/// rend ce risque plus grand, pas moins — un moniteur 4K conviendrait
/// désormais à n'importe quel viewport.
pub fn sortie_pour_viewport(
    sorties: &[SortieDxgi],
    largeur: u32,
    hauteur: u32,
    deja_prises: &[String],
) -> Option<SortieDxgi> {
    sorties
        .iter()
        .find(|s| {
            s.attachee_au_bureau
                && sortie_assez_grande((s.rect.width, s.rect.height), (largeur, hauteur))
                && !deja_prises.contains(&s.nom_sortie)
        })
        .cloned()
}
```

Renommer les appels dans les tests existants (`trouve_la_sortie_aux_dimensions_demandees`,
`ignore_une_sortie_non_attachee`, `ignore_une_sortie_deja_attribuee`,
`ne_trouve_rien_quand_toutes_sont_prises`, `un_ecart_dans_la_tolerance_apparie_quand_meme`,
`une_sortie_deja_prise_est_ignoree`).

⚠️ `n_apparie_pas_une_sortie_aux_mauvaises_dimensions` (l. 208-215) attend
qu'une sortie **1067×600** ne serve pas un viewport 1600×900 : elle est plus
petite, donc le test reste vert **et garde son sens**. Ne pas y toucher.

- [ ] **Step 4 : les exécuter et les voir verts**

```bash
cd agent && cargo test -p agent superviseur::placement 2>&1 | tail -5
```

- [ ] **Step 5 : commit**

```bash
git add agent/src/superviseur/placement.rs
git commit -m "modifie(d10): l'appariement accepte une sortie plus grande, et la recadre"
```

---

### Task 6 : câbler la création — bornage, appariement, taille retenue

**Files:**
- Modify: `agent/src/superviseur/boucle/creation_sortie.rs`
- Modify: `agent/src/superviseur/boucle.rs` (le `use` de `placement`)

**Interfaces:**
- Consumes: `placement::sortie_pour_viewport`, `placement::taille_retenue`
  (tâches 4-5) ; `crate::windows_source_sortie::borner_a_la_taille_max`.
- Produces: `Table::sortie_creee` reçoit désormais **la taille retenue**, pas la
  taille de la sortie.

- [ ] **Step 1 : borner la demande — le leg 5**

Dans `creer_sortie`, **avant** le relevé de topologie, remplacer l'usage direct
de `(largeur, hauteur)` par :

```rust
    // LEG 5 de D9. `borner_a_la_taille_max` attendait son appelant depuis que
    // le changement de mode de sortie a été retiré : c'est ici.
    //
    // ⚠️ Le viewport arrive en PIXELS PÉRIPHÉRIQUES depuis la tâche 5 de D9
    // (`client/src/main.ts`, `innerWidth × devicePixelRatio`) : un client à
    // `devicePixelRatio = 2` demande 2560×1440 là où il demandait 1280×720,
    // soit quatre fois les pixels à capturer et à encoder. Et le plafond de
    // 8 encodeurs concurrents n'a JAMAIS été mesuré au-delà de 720p — NVENC
    // borne en macroblocs par seconde, pas en nombre de sessions.
    //
    // `TAILLE_MAX_SORTIE` (1920×1080) n'est PAS calibrée : c'est un garde-fou
    // de prudence, et aucun jugement visuel ne l'a jugée.
    let (largeur, hauteur) =
        crate::windows_source_sortie::borner_a_la_taille_max((largeur, hauteur));
```

- [ ] **Step 2 : changer l'appariement et calculer la taille retenue**

Remplacer le bloc `let Some(cible) = placement::sortie_par_dimensions(…) else { … }`
par :

```rust
    let Some(cible) = placement::sortie_pour_viewport(&apparues, largeur, hauteur, prises)
    else {
        // Ce refus ne peut plus venir d'une sortie née TROP GRANDE — c'est le
        // leg 4 de D9, qui plafonnait le produit à trois fenêtres sur une VM
        // au registre pollué. Il ne reste que deux causes : aucune sortie n'est
        // apparue du tout, ou celle qui est apparue est plus PETITE que la
        // demande de plus de `TOLERANCE_PX`.
        tracing::error!(
            session = %session.0,
            demande = format!("{largeur}x{hauteur}"),
            apparues = ?apparues
                .iter()
                .map(|s| format!("{} {}x{}", s.nom_sortie, s.rect.width, s.rect.height))
                .collect::<Vec<_>>(),
            "aucune sortie apparue ne peut servir ce viewport — elle est rendue au pilote"
        );
        rendre_sans_apparier(sorties, id_pilote);
        envoyer(&VersLaShell::Refus {
            titre,
            motif: "aucune sortie d'affichage ne peut servir cette fenêtre".into(),
        });
        return table.enfant_mort(&session);
    };

    // La sortie peut être bien plus grande que la fenêtre : c'est le cas
    // nominal sur une VM dont le registre a été pollué. La fenêtre est posée à
    // CETTE taille, à l'origine de la sortie, et la capture recadre le même
    // rectangle dans la duplication de CETTE sortie — jamais dans celle du
    // bureau, d'où l'absence du risque de fuite entre sessions que porte
    // `ModeCapture::FenetreRecadree` (voir l'en-tête de
    // `windows_source/sortie.rs`).
    let retenue = placement::taille_retenue((largeur, hauteur), (cible.rect.width, cible.rect.height));
```

Puis, à l'appel de `sortie_creee`, passer `retenue` **à la place** de
`(cible.rect.width, cible.rect.height)`.

- [ ] **Step 3 : poser la fenêtre à la taille retenue, pas à celle de la sortie**

Chercher l'appel à `placement::poser` (dans `boucle/placement_periodique.rs` et,
le cas échéant, dans le chemin de lancement) :

```bash
cd agent && grep -rn "placement::poser\|poser(" src/superviseur/
```

Le `Rect` cible doit valoir `{ x: cible.rect.x, y: cible.rect.y, width: retenue.0, height: retenue.1 }`.
La source de vérité de ce rectangle est la `taille_sortie` mémorisée par la
table, qui porte désormais la taille **retenue** : le contrôle périodique la
relit et n'a donc rien à recalculer.

- [ ] **Step 4 : vérifier**

```bash
cd agent && cargo check --target x86_64-pc-windows-gnu && cargo test -p agent
```
Attendu : sortie 0 ; 440 passed (+ les 12 tests des tâches 4-5).

- [ ] **Step 5 : commit**

```bash
git add agent/src/superviseur/boucle/creation_sortie.rs agent/src/superviseur/boucle.rs agent/src/superviseur/boucle/placement_periodique.rs
git commit -m "corrige(d10): une sortie nee trop grande est acceptee et recadree"
```

---

### Task 7 : la réutilisation d'une sortie retenue suit la même règle

**Files:**
- Modify: `agent/src/superviseur/table/attribution.rs:33`
- Test: `agent/src/superviseur/table/tests_retention.rs`

**Interfaces:**
- Consumes: `placement::sortie_assez_grande`, `placement::taille_retenue`.
- Produces: rien de neuf.

**Pourquoi cette tâche existe** : `viewport_recu` décide si la sortie qu'une
fenêtre a **gardée** de sa vie précédente peut resservir. Laissée sur
`taille_compatible` (±4 px), une sortie retenue de 3840×2160 serait détruite et
recréée à **chaque relance** — c'est-à-dire exactement la recréation que le
sous-bloc D3 existe pour supprimer, et la cause des 32 réouvertures parasites
qu'il a fermées.

- [ ] **Step 1 : écrire le test, qui doit échouer**

Dans `tests_retention.rs` :

L'aide `session_vivante` (l. 11-20) code en dur `(1280, 720)` à la ligne 18.
La généraliser d'abord, sans changer son comportement pour ses onze appelants :

```rust
fn session_vivante(t: &mut Table, fenetre: u64, titre: &str, sortie: u32, nom: &str) -> IdSession {
    session_vivante_de_taille(t, fenetre, titre, sortie, nom, (1280, 720))
}

/// Même amorce, mais la sortie naît à une taille imposée — le cas d'une VM
/// dont le registre a été pollué (D9 §9).
fn session_vivante_de_taille(
    t: &mut Table,
    fenetre: u64,
    titre: &str,
    sortie: u32,
    nom: &str,
    taille: (u32, u32),
) -> IdSession {
    let effets = t.fenetre_apparue(IdFenetre(fenetre), titre.into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("ouverture attendue, reçu {effets:?}");
    };
    let session = session.clone();
    t.viewport_recu(&session, 1280, 720);
    t.sortie_creee(&session, sortie, nom.into(), taille);
    session
}
```

Puis, sur le modèle exact de `une_sortie_retenue_compatible_est_reutilisee_sans_rien_creer`
(l. 128) :

```rust
    /// Une sortie retenue plus GRANDE que le viewport resservira : la
    /// détruire et la recréer ferait abandonner le mutex des duplications
    /// voisines à chaque relance — exactement la recréation que le sous-bloc
    /// D3 existe pour supprimer, et la cause de ses 32 réouvertures parasites.
    #[test]
    fn une_sortie_retenue_plus_grande_est_reutilisee() {
        let mut t = Table::nouvelle(4);
        let session = session_vivante_de_taille(
            &mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY8", (3840, 2160),
        );
        t.enfant_mort(&session);
        let effets = t.relancer_les_orphelines(std::time::Instant::now());
        let Some(Effet::AnnoncerOuverture { session: neuve, .. }) = effets.first() else {
            panic!("réouverture attendue, reçu {effets:?}");
        };
        let neuve = neuve.clone();

        let effets = t.viewport_recu(&neuve, 1280, 720);

        assert_eq!(
            effets,
            vec![Effet::LancerEnfant {
                session: neuve.clone(),
                fenetre: IdFenetre(1),
                nom_sortie: "\\\\.\\DISPLAY8".into(),
            }],
            "une sortie retenue assez grande ne doit être ni détruite ni recréée"
        );
    }
```

⚠️ **`une_sortie_retenue_incompatible_est_rendue_puis_remplacee` (l. 173) reste
VERT et garde son sens** : il demande 1920×1080 sur une sortie retenue de
1280×720, donc **trop petite**, seul cas qui justifie encore de rendre et
recréer. Le vérifier plutôt que le supposer.

- [ ] **Step 2 : les exécuter et voir le premier rouge**

```bash
cd agent && cargo test -p agent tests_retention 2>&1 | tail -20
```
Attendu : `une_sortie_retenue_plus_grande_est_reutilisee` ÉCHOUE avec
« une sortie retenue assez grande ne doit jamais être détruite ». Le second test
passe déjà — c'est le comportement actuel, et il doit le rester.

- [ ] **Step 3 : implémenter**

Dans `attribution.rs`, remplacer la ligne 33 :

```rust
            // Même prédicat que l'appariement à la création
            // (`placement::sortie_pour_viewport`), et c'est le point : deux
            // règles distinctes feraient détruire à la relance une sortie que
            // la création venait d'accepter. La sortie retenue est réutilisée
            // dès qu'elle est ASSEZ GRANDE ; elle n'est rendue que si elle est
            // trop petite, seul cas où la recréer peut apporter des pixels.
            if crate::superviseur::placement::sortie_assez_grande(taille, (largeur, hauteur)) {
```

Et, dans le même bloc, mémoriser la taille retenue avant de lancer l'enfant :

```rust
                entree.taille_sortie =
                    Some(crate::superviseur::placement::taille_retenue((largeur, hauteur), taille));
```

⚠️ **`largeur`/`hauteur` doivent avoir été bornés par
`borner_a_la_taille_max` avant d'arriver ici**, comme à la création. Le borner
dans `viewport_recu` plutôt que chez son appelant garde les deux chemins
symétriques.

- [ ] **Step 4 : les exécuter et les voir verts**

```bash
cd agent && cargo test -p agent tests_retention 2>&1 | tail -5
```

- [ ] **Step 5 : `taille_compatible` n'a-t-elle plus d'appelant ?**

```bash
cd agent && grep -rn "taille_compatible" src/ | grep -v "mod tests"
```
Si l'unique définition subsiste sans appelant hors tests, **la retirer avec ses
quatre tests** — un `dead_code` de plus dans un crate qui en compte 11 se perd.
Si un autre appelant existe, le laisser et le dire dans le commit.

- [ ] **Step 6 : commit**

```bash
git add agent/src/superviseur/table/attribution.rs agent/src/superviseur/table/tests_retention.rs agent/src/superviseur/placement.rs
git commit -m "corrige(d10): une sortie retenue assez grande n'est plus detruite a la relance"
```

---

### Task 8 : la capture recadre la taille retenue

**Files:**
- Modify: `agent/src/windows_source/sortie.rs` (`sur_sortie`)
- Modify: `agent/src/capteur/fenetre.rs` (`Fenetre::ouvrir`, appel à `sur_sortie`)
- Modify: `agent/src/capteur/fenetre/commandes.rs` (reconstruction au réveil)

**Interfaces:**
- Consumes: `placement::taille_retenue`, `region_de_sortie`.
- Produces: `WindowsSource::sur_sortie(hwnd, nom_sortie, taille, fps, bitrate, clock_origin)`
  — **un paramètre de plus**, en troisième position.

- [ ] **Step 1 : élargir `sur_sortie`**

```rust
    pub fn sur_sortie(
        hwnd: HWND,
        nom_sortie: &str,
        taille: (u32, u32),
        fps: u32,
        bitrate: u32,
        clock_origin: std::time::Instant,
    ) -> Result<Self> {
        let capture = DesktopCapture::sur_sortie(nom_sortie)?;
        let (dw, dh) = capture.desktop_size();
        // La sortie peut être PLUS GRANDE que la fenêtre depuis le sous-bloc
        // D10 : on recadre à l'origine de la sortie, là où le superviseur a
        // posé la fenêtre. `taille_retenue` garantit que la région tient dans
        // la texture — c'est elle qui borne, pas cette fonction.
        let (rl, rh) = crate::superviseur::placement::taille_retenue(taille, (dw, dh));
        let region = region_de_sortie(rl, rh).with_context(|| {
            format!("sortie {nom_sortie} de dimensions inexploitables ({dw}x{dh})")
        })?;
        let (width, height) = (region.width, region.height);
        // … le reste est inchangé, ModeCapture::SortieEntiere compris …
```

⚠️ **`ModeCapture::SortieEntiere` ne change pas de valeur.** Le mode
`FenetreRecadree` duplique **le bureau** et porte le risque de fuite entre
sessions décrit en tête de ce fichier ; ici on recadre dans la duplication **de
la sortie**. `redimensionne_la_fenetre()` doit continuer de rendre `false`.

- [ ] **Step 2 : mettre à jour le commentaire de `region_de_sortie`**

Sa doc affirme « rend une texture qui couvre cette sortie seule » et son nom
suggère la sortie entière. Ajouter :

```rust
/// ⚠️ **Depuis le sous-bloc D10, l'appelant lui passe la taille RETENUE, pas
/// celle de la sortie** : une sortie née trop grande (registre pollué) est
/// acceptée et recadrée à l'origine. La fonction elle-même est inchangée —
/// c'est son argument qui a changé de sens.
```

Et corriger le nom du test `la_region_couvre_toute_la_sortie_a_partir_de_son_origine_propre`
en `la_region_part_de_l_origine_de_la_sortie`, sans changer son corps.

- [ ] **Step 3 : la taille traverse jusqu'au capteur**

`Fenetre::ouvrir` (`capteur/fenetre.rs:234`) lit aujourd'hui la taille de la
sortie et la déclare comme celle de la fenêtre. Elle doit désormais **retenir**
la taille demandée :

```rust
        let (sortie_l, sortie_h) = crate::capture::ouverture::taille_de_sortie(&sortie)
            .with_context(|| format!("attache de la session {session}"))?;
        // La sortie peut être plus grande que la fenêtre (registre pollué,
        // D9 §9). L'enfant a annoncé la taille que le superviseur lui a
        // donnée ; le capteur la borne à ce que la sortie offre réellement,
        // avec la MÊME fonction pure que le superviseur — deux calculs
        // déterministes sur les mêmes entrées, jamais deux règles.
        let (largeur, hauteur) = crate::superviseur::placement::taille_retenue(
            (demande_l, demande_h),
            (sortie_l, sortie_h),
        );
```

où `(demande_l, demande_h)` vient du message d'attache (tâche 9).

- [ ] **Step 4 : l'appel de reconstruction au réveil**

```bash
cd agent && grep -rn "sur_sortie(" src/capteur/
```
Chaque appel reçoit `self.dimensions()` en troisième argument. ⚠️ Le commentaire
de `capteur/fenetre/commandes.rs:134` dit que le réveil « reconstruit la source
par `sur_sortie`, donc à la taille d'encodage » : **le relire et le corriger s'il
devient faux** — c'est exactement la classe de défaut que la revue transverse de
D9 a trouvée six fois.

- [ ] **Step 5 : vérifier**

```bash
cd agent && cargo check --target x86_64-pc-windows-gnu && cargo test -p agent
```

- [ ] **Step 6 : commit**

```bash
git add agent/src/windows_source/sortie.rs agent/src/capteur/fenetre.rs agent/src/capteur/fenetre/commandes.rs
git commit -m "corrige(d10): la capture recadre la taille retenue dans la duplication de sa sortie"
```

---

### Task 9 : la taille demandée voyage jusqu'au capteur

**Files:**
- Modify: `agent/src/superviseur/enfants.rs` (`Consigne`)
- Modify: `agent/src/superviseur/lanceur.rs:229-233`
- Modify: `agent/src/demarrage.rs` (`Config`)
- Modify: `agent/src/capteur/protocole.rs` (`VersCapteur::Attache`)
- Modify: `agent/src/capteur/distante.rs` (émission de l'attache)
- Modify: `scripts/run-agent.sh`

**Interfaces:**
- Consumes: rien.
- Produces: `VersCapteur::Attache { session, hwnd, sortie, taille: (u32, u32), fps, debit, origine_qpc }`
  — **un champ de plus**, nommé `taille`.

- [ ] **Step 1 : `Consigne` porte la taille, le lanceur la pose**

`Consigne` gagne `pub taille: (u32, u32)`, remplie depuis `Effet::LancerEnfant`
(qui la tient de `entree.taille_sortie`, posée aux tâches 6 et 7). Le lanceur, à
côté de `SORTIE_DXGI` :

```rust
            .env("TAILLE_FENETRE", format!("{}x{}", consigne.taille.0, consigne.taille.1))
```

- [ ] **Step 2 : l'enfant la lit**

Dans `demarrage.rs`, à côté de la lecture de `SORTIE_DXGI` :

```rust
    // Posée par le superviseur (`lanceur.rs`), jamais par un opérateur. Absente
    // — cas du chemin mono-fenêtre —, la taille reste celle de la sortie, et
    // `taille_retenue` la rendra telle quelle côté capteur.
    let taille_fenetre = std::env::var("TAILLE_FENETRE").ok().and_then(|v| {
        let (l, h) = v.split_once('x')?;
        Some((l.parse().ok()?, h.parse().ok()?))
    });
```

- [ ] **Step 3 : le message d'attache la transporte**

`VersCapteur::Attache` gagne `taille: (u32, u32)`. Côté enfant
(`capteur/distante.rs`), la remplir depuis la config ; **à défaut**, poser
`(u32::MAX, u32::MAX)` — `taille_retenue` la ramènera alors à la taille de la
sortie, ce qui reproduit exactement le comportement d'avant D10.

⚠️ **Vérifier le bras catch-all.** `capteur/pont_media.rs` porte
`Ok(autre) => return`, qui **tue le fil `lire_le_media` en silence** — payé
quatre fois (D5, D6, D7, D8). `Attache` circule enfant → capteur sur la
connexion de **commandes**, pas sur le média : le bras n'est donc pas concerné.
**Le vérifier explicitement plutôt que le supposer** :

```bash
cd agent && grep -n "Attache" src/capteur/pont_media.rs
```
Attendu : aucune ligne.

- [ ] **Step 4 : `scripts/run-agent.sh`**

Ajouter `TAILLE_FENETRE` à la liste des variables transmises, **et relever le
numéro de ligne par `grep -n` plutôt que de recopier celui d'un document** :
`CLAUDE.md` note que ces numéros ont déjà dérivé deux fois.

- [ ] **Step 5 : vérifier**

```bash
cd agent && cargo check --target x86_64-pc-windows-gnu && cargo test -p agent
```

- [ ] **Step 6 : commit**

```bash
git add agent/src/superviseur/enfants.rs agent/src/superviseur/lanceur.rs agent/src/demarrage.rs agent/src/capteur/protocole.rs agent/src/capteur/distante.rs scripts/run-agent.sh
git commit -m "ajoute(d10): la taille demandee voyage du superviseur au capteur"
```

---

### Task 10 : recette ① — huit fenêtres sur un registre laissé sale

**Files:**
- Create: `docs/superpowers/plans/journaux-multifenetres-d10/` (journaux + `-plat` jumeaux)

**Interfaces:**
- Consumes: les tâches 1 à 9.
- Produces: le verdict du critère ①.

- [ ] **Step 1 : préparer la VM, SANS nettoyer le registre**

```bash
virsh list --all
virsh start Windows
until timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null; do sleep 5; done
until ls /media/vm/dev >/dev/null 2>&1; do sleep 5; done
set -a && source .env && set +a
node scripts/winrm.js 'Get-Process agent -ErrorAction SilentlyContinue | Format-List Id,StartTime'
```

⚠️ **NE PAS lancer `MULTIFENETRE_MODE_SORTIE=1280x720`** : le registre sale
**est** l'état à mesurer. Le rétablir annulerait la mesure.

- [ ] **Step 2 : établir l'état de départ, qui doit être ROUGE**

Sur le binaire de `main` (avant D10), relever le plafond actuel :

```bash
sed 's/\x1b\[[0-9;]*m//g' agent.log > agent-avant-plat.log
grep -c 'fenêtre attachée au capteur' agent-avant-plat.log
grep -c 'introuvable dans la topologie DXGI' agent-avant-plat.log
```
Attendu : **3** attaches, et un nombre non nul d'erreurs. **C'est le contrôle vu
rouge** : sans ce relevé, le vert d'après ne prouve rien.

- [ ] **Step 3 : compiler et lancer D10**

```bash
scripts/build-agent.sh
```
⚠️ **Vérifier la TAILLE du binaire** : une compilation de 0,13 s est un aveu (un
`rsync -a` qui remonte le temps fait garder le binaire précédent ; seul
`cargo clean --release -p agent` débloque).

Pilote : dix fenêtres Chrome `--app` **animées à cadence connue**
(`instrument/anim-d4.html`), **un `--user-data-dir` par fenêtre**, navigateur
pilote sur l'**hôte**.

- [ ] **Step 4 : relever le critère ①**

```bash
sed 's/\x1b\[[0-9;]*m//g' agent.log > agent-critere-1-1-plat.log
grep -c 'fenêtre attachée au capteur' agent-critere-1-1-plat.log   # attendu : 8
grep -c 'introuvable dans la topologie DXGI' agent-critere-1-1-plat.log  # attendu : 0
grep -c 'aucune sortie apparue ne peut servir' agent-critere-1-1-plat.log # attendu : 0
grep 'sortie virtuelle créée' agent-critere-1-1-plat.log | head
```

- [ ] **Step 5 : relever le critère ② (l'image est la bonne)**

Sur chaque page : `framesDecoded` en croissance, et la fenêtre montre **son**
application seule. ⚠️ **Une source immobile ne produit aucune image** : Desktop
Duplication n'émet qu'au changement du bureau.

- [ ] **Step 6 : deuxième exécution, puis verser**

Rejouer intégralement. **Deux exécutions, et l'énoncé porte ce nombre.**
Verser chaque `agent-*.log` **avec son jumeau `-plat`**.

- [ ] **Step 7 : commit**

```bash
git add docs/superpowers/plans/journaux-multifenetres-d10/
git commit -m "recette(d10): huit fenetres sur un registre laisse sale, deux executions"
```

---

# Famille ② — l'audio mort (legs 1, 6, 8)

### Task 11 : le reconstructeur, construit et confié à la session

**Files:**
- Modify: `agent/src/audio.rs`, `agent/src/transport.rs` (champs de `Session`),
  `agent/src/transport/piste_audio.rs`, `agent/src/demarrage/audio.rs`
- Test: `agent/src/transport/tick/tests/audio.rs`

**Interfaces:**
- Consumes: `AudioSource`, `Session::set_audio_source`.
- Produces: `crate::audio::Reconstructeur`, `RECONSTRUCTIONS_MAX`,
  `REPIT_RECONSTRUCTION`, `Session::set_audio_reconstructeur`,
  `Session::reconstruire_ou_signaler(maintenant: Instant) -> bool`.

- [ ] **Step 1 : écrire le test, qui doit échouer**

Dans `tick/tests/audio.rs` :

```rust
    /// Le fait réparé (D9 §4.3) : une capture morte n'était JAMAIS
    /// reconstruite. `set_actif(true)` n'écrit qu'un booléen atomique que le
    /// fil mort ne relit jamais, et réélire la même session ne fait rien.
    #[test]
    fn une_capture_morte_est_reconstruite_avant_tout_signalement() {
        let mut session = session_d_essai();
        session.set_audio_source(Box::new(SourceMorte::new()));
        let essais = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let compte = std::sync::Arc::clone(&essais);
        session.set_audio_reconstructeur(Box::new(move || {
            compte.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(Box::new(SourceVivante::new()) as Box<dyn AudioSource + Send>)
        }));

        let t0 = std::time::Instant::now();
        assert!(!session.reconstruire_ou_signaler(t0), "rien à signaler : on reconstruit");
        assert_eq!(essais.load(std::sync::atomic::Ordering::Relaxed), 1);
        assert!(!session.capture_audio_morte(), "la source neuve est vivante");
    }

    /// Le budget épuisé fait retomber sur le signalement : c'est là que la
    /// promotion d'une voisine par le capteur reprend son rôle — la seule
    /// moitié de D9 qui fonctionnait.
    #[test]
    fn un_reconstructeur_qui_echoue_toujours_finit_par_signaler() {
        let mut session = session_d_essai();
        session.set_audio_source(Box::new(SourceMorte::new()));
        session.set_audio_reconstructeur(Box::new(|| anyhow::bail!("plus d'arbre de processus")));

        let mut t = std::time::Instant::now();
        for essai in 0..crate::audio::RECONSTRUCTIONS_MAX {
            assert!(!session.reconstruire_ou_signaler(t), "essai {essai} : budget restant");
            t += crate::audio::REPIT_RECONSTRUCTION;
        }
        assert!(session.reconstruire_ou_signaler(t), "budget épuisé : il faut signaler");
    }

    /// Le répit est respecté : sans lui, la boucle de tick tenterait une
    /// ouverture WASAPI à chaque tour.
    #[test]
    fn le_repit_espace_les_tentatives() {
        let mut session = session_d_essai();
        session.set_audio_source(Box::new(SourceMorte::new()));
        let essais = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let compte = std::sync::Arc::clone(&essais);
        session.set_audio_reconstructeur(Box::new(move || {
            compte.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            anyhow::bail!("pas encore")
        }));

        let t0 = std::time::Instant::now();
        session.reconstruire_ou_signaler(t0);
        session.reconstruire_ou_signaler(t0);
        assert_eq!(
            essais.load(std::sync::atomic::Ordering::Relaxed),
            1,
            "deux appels dans le même instant ne font qu'une tentative"
        );
    }

    /// Sans reconstructeur — chemin mono-fenêtre, ou `AUDIO=0` —, le
    /// comportement d'avant D10 doit être exactement conservé.
    #[test]
    fn sans_reconstructeur_on_signale_immediatement() {
        let mut session = session_d_essai();
        session.set_audio_source(Box::new(SourceMorte::new()));
        assert!(session.reconstruire_ou_signaler(std::time::Instant::now()));
    }
```

**Les trois sources factices, à écrire une fois dans `tick/tests/audio.rs`** —
la tâche 12 les réemploie telles quelles, ne pas en changer les noms :

```rust
    /// Capture morte : `capture_morte()` vrai, aucun paquet. C'est l'état dans
    /// lequel le fil de `windows_audio.rs` laisse la source après son `return`
    /// définitif.
    struct SourceMorte;
    impl SourceMorte {
        fn new() -> Self { Self }
    }
    impl AudioSource for SourceMorte {
        fn capture_morte(&self) -> bool { true }
        fn next_packet(&mut self) -> Option<AudioPacket> { None }
        fn set_actif(&mut self, _actif: bool) {}
    }

    /// Capture vivante, avec ou sans paquet en attente. `sans_paquet` sert à
    /// distinguer « reconstruite » de « entendue » — c'est toute la
    /// différence entre une décision et une preuve (leg 6).
    struct SourceVivante { paquets: u32 }
    impl SourceVivante {
        fn new() -> Self { Self::avec_un_paquet() }
        fn sans_paquet() -> Self { Self { paquets: 0 } }
        fn avec_un_paquet() -> Self { Self { paquets: 1 } }
    }
    impl AudioSource for SourceVivante {
        fn capture_morte(&self) -> bool { false }
        fn next_packet(&mut self) -> Option<AudioPacket> {
            (self.paquets > 0).then(|| { self.paquets -= 1; paquet_d_essai() })
        }
        fn set_actif(&mut self, _actif: bool) {}
    }
```

⚠️ **Compléter ces `impl` avec toute méthode que le trait `AudioSource` exige
sans valeur par défaut** — le relever, ne pas le supposer :

```bash
cd agent && grep -n "trait AudioSource" -A 30 src/audio.rs
```

`paquet_d_essai()` et `session_d_essai()` : réemployer les aides déjà présentes
dans `tick/tests.rs` (module de tête après la tâche 2) ; les y ajouter si elles
n'existent pas.

- [ ] **Step 2 : les exécuter et les voir rouges**

```bash
cd agent && cargo test -p agent tick::tests::audio 2>&1 | tail -20
```
Attendu : `no method named 'set_audio_reconstructeur'` et
`no method named 'reconstruire_ou_signaler'`.

- [ ] **Step 3 : implémenter — le type et les constantes**

Dans `audio.rs` :

```rust
/// De quoi refabriquer une source audio après la mort de sa capture.
///
/// **Le remède du leg 1 de D9.** Quand `capture.read()` échoue plus de
/// `LECTURES_ECHOUEES_MAX` fois d'affilée, le fil de `windows_audio.rs` pose
/// `capture_morte` et exécute un `return` DÉFINITIF. Rien, jusqu'à D10, ne
/// reconstruisait la source : une fenêtre seule de son groupe de PID — le cas
/// MAJORITAIRE, une application une fenêtre — perdait son son pour le restant
/// de la session, et la « réélection après répit » du capteur ne faisait
/// qu'écrire un booléen que ce fil mort ne relisait jamais.
///
/// Une fermeture plutôt qu'un trait : `transport/` ne doit rien connaître de
/// Windows, et c'est `demarrage/audio.rs` — seul détenteur de `Config` et du
/// `clock_origin` — qui sait refaire le bon choix de mode.
pub type Reconstructeur = Box<dyn Fn() -> anyhow::Result<Box<dyn AudioSource + Send>> + Send>;

/// Nombre de reconstructions tentées avant d'abandonner et de signaler.
///
/// ⚠️ **NON CALIBRÉE** — elle rejoint `BPP_MIN`, `FACTEUR_FOCUS`,
/// `PART_DORMANTE_BPS`, `HYSTERESIS`, `REPIT_APRES_ECHEC`, `TAILLE_MAX_SORTIE`,
/// `REPIT_REARMEMENT_AUDIO` et `REARMEMENTS_MAX` dans la liste des constantes
/// qu'aucun jugement d'écoute n'a jugées.
pub const RECONSTRUCTIONS_MAX: u32 = 3;

/// Délai entre deux tentatives de reconstruction.
///
/// ⚠️ **Ne PAS réemployer `temporisation_de_reprise`** : elle cadence les
/// relectures À L'INTÉRIEUR du fil de capture, pas les reconstructions de
/// source. Deux durées de sens différent qui divergeraient en silence le jour
/// où l'une changerait.
///
/// ⚠️ **NON CALIBRÉE** elle aussi.
pub const REPIT_RECONSTRUCTION: std::time::Duration = std::time::Duration::from_secs(2);
```

- [ ] **Step 4 : implémenter — les champs et la méthode**

`Session` gagne `audio_reconstructeur: Option<Reconstructeur>`,
`reconstructions_restantes: u32` (initialisé à `RECONSTRUCTIONS_MAX`),
`prochaine_reconstruction: Option<Instant>`,
`audio_reconstruit_sans_preuve: bool`.

Dans `piste_audio.rs` :

```rust
    /// Confie de quoi refabriquer la source audio après la mort de sa capture.
    pub fn set_audio_reconstructeur(&mut self, r: crate::audio::Reconstructeur) {
        self.audio_reconstructeur = Some(r);
    }

    /// Rend `true` s'il faut signaler `AudioMort` au capteur — c'est-à-dire
    /// quand il n'y a plus rien à reconstruire.
    ///
    /// **La reconstruction passe AVANT le signalement**, et c'est l'inversion
    /// que D10 apporte : le signal au capteur cesse d'être le premier geste
    /// pour devenir le repli. La promotion d'une voisine (la seule moitié de
    /// D9 qui fonctionnait) garde alors son rôle exact — celui du cas où
    /// l'arbre de processus a réellement disparu.
    ///
    /// ⚠️ **Cette méthode court sur le fil de `Session::run`**, et ouvrir une
    /// source WASAPI y est un appel bloquant de durée non bornée. D'où le
    /// répit : au plus une tentative par `REPIT_RECONSTRUCTION`. Si la mesure
    /// montre qu'elle retarde le drainage, elle passera sur un fil — même
    /// risque que `Drop for H264Encoder` porte déjà sur ce fil.
    pub(super) fn reconstruire_ou_signaler(&mut self, maintenant: std::time::Instant) -> bool {
        if !self.capture_audio_morte() {
            return false;
        }
        let Some(reconstructeur) = self.audio_reconstructeur.as_ref() else {
            return true;
        };
        if self.reconstructions_restantes == 0 {
            return true;
        }
        if self.prochaine_reconstruction.is_some_and(|t| maintenant < t) {
            return false;
        }
        self.reconstructions_restantes -= 1;
        self.prochaine_reconstruction = Some(maintenant + crate::audio::REPIT_RECONSTRUCTION);
        match reconstructeur() {
            Ok(source) => {
                tracing::info!(
                    restantes = self.reconstructions_restantes,
                    "capture audio reconstruite"
                );
                self.audio_source = Some(source);
                self.audio_reconstruit_sans_preuve = true;
                false
            }
            Err(erreur) => {
                tracing::warn!(
                    %erreur,
                    restantes = self.reconstructions_restantes,
                    "reconstruction de la capture audio refusée"
                );
                false
            }
        }
    }
```

⚠️ `self.audio_source = Some(source)` plutôt que `set_audio_source` : l'emprunt
de `reconstructeur` sur `self` est encore vivant. Si le compilateur s'en plaint,
prendre le reconstructeur par `Option::take` puis le remettre.

- [ ] **Step 5 : construire le reconstructeur**

Dans `demarrage/audio.rs::brancher`, après le `set_audio_source` réussi :

```rust
                // Le MÊME choix de mode que ci-dessus, refait à l'identique.
                // Le repli n'est JAMAIS le mix global : une fenêtre qui
                // entendrait toutes les autres sous couvert d'isolation est
                // l'arbitrage explicitement écarté au cadrage de D7.
                let hwnd = config.fenetre_hwnd;
                session.set_audio_reconstructeur(Box::new(move || {
                    let source = match hwnd {
                        Some(hwnd) => {
                            windows_audio::WindowsAudioSource::pour_processus(
                                pid_de_fenetre(hwnd)?,
                                clock_origin,
                            )?
                        }
                        None => windows_audio::WindowsAudioSource::new(clock_origin)?,
                    };
                    Ok(Box::new(source) as Box<dyn crate::audio::AudioSource + Send>)
                }));
```

- [ ] **Step 6 : les exécuter et les voir verts**

```bash
cd agent && cargo test -p agent tick::tests::audio 2>&1 | tail -5
cd agent && cargo check --target x86_64-pc-windows-gnu
```

- [ ] **Step 7 : commit**

```bash
git add agent/src/audio.rs agent/src/transport.rs agent/src/transport/piste_audio.rs agent/src/demarrage/audio.rs agent/src/transport/tick/tests/audio.rs
git commit -m "ajoute(d10): la capture audio morte se reconstruit avant d'etre signalee"
```

---

### Task 12 : a1sexies reconstruit, et `AudioVivant` prouve la reprise

**Files:**
- Modify: `agent/src/transport/tick.rs` (branche a1sexies)
- Modify: `agent/src/transport/piste_audio.rs` (`brancher_audio`)
- Modify: `agent/src/source.rs` (trait `VideoSource`)
- Modify: `agent/src/capteur/protocole.rs`, `agent/src/capteur/distante.rs`
- Test: `agent/src/transport/tick/tests/audio.rs`

**Interfaces:**
- Consumes: `reconstruire_ou_signaler` (tâche 11).
- Produces: `VideoSource::signaler_audio_vivant(&mut self)` (défaut **inerte**),
  `VersCapteur::AudioVivant`.

- [ ] **Step 1 : écrire le test, qui doit échouer**

```rust
    /// Le leg 6 : `REARMEMENTS_MAX` doit se remettre à zéro sur une PREUVE de
    /// son, pas sur une décision d'arbitrage. La preuve est le premier paquet
    /// qui repart après une reconstruction.
    #[test]
    fn audio_vivant_n_est_annonce_qu_apres_un_paquet_reel() {
        let mut session = session_d_essai();
        session.set_audio_source(Box::new(SourceMorte::new()));
        session.set_audio_reconstructeur(Box::new(|| {
            Ok(Box::new(SourceVivante::sans_paquet()) as Box<dyn AudioSource + Send>)
        }));
        session.reconstruire_ou_signaler(std::time::Instant::now());
        assert!(
            !annonces.load(std::sync::atomic::Ordering::Relaxed),
            "reconstruite n'est pas entendue : aucune preuve encore"
        );

        session.set_audio_source(Box::new(SourceVivante::avec_un_paquet()));
        session.brancher_audio(); // le paquet qui repart EST la preuve
        session.tick_pour_test();
        assert!(annonces.load(std::sync::atomic::Ordering::Relaxed));
    }
```

`annonces` est un `Arc<AtomicBool>` **confié à la source vidéo factice** de
`session_d_essai()` : son implémentation de `signaler_audio_vivant` le pose à
`true`. C'est la seule façon d'observer un appel sur un `Box<dyn VideoSource>`
sans downcast. Étendre la source factice existante du module de tests plutôt
que d'en écrire une seconde :

```bash
cd agent && grep -n "impl VideoSource for" src/transport/tick/tests.rs src/transport/tick/tests/*.rs
```

`tick_pour_test()` : réemployer le point d'entrée que les tests voisins
utilisent déjà pour faire tourner un tour de boucle (le repérer dans le même
`grep`) ; ne pas en introduire un second.

- [ ] **Step 2 : le voir rouge**

```bash
cd agent && cargo test -p agent tick::tests::audio 2>&1 | tail -20
```
Attendu : `no method named 'source_a_signale_audio_vivant'`.

- [ ] **Step 3 : implémenter — le trait et le protocole**

Dans `source.rs`, sur `VideoSource`, à côté de `signaler_audio_mort` :

```rust
    /// Annonce au capteur que la capture audio de cette fenêtre a repris.
    ///
    /// Défaut INERTE, comme les deux méthodes voisines : les sources qui ne
    /// parlent à aucun capteur (test, mono-fenêtre) n'ont rien à annoncer.
    fn signaler_audio_vivant(&mut self) {}
```

Dans `capteur/protocole.rs`, ajouter `AudioVivant` à `VersCapteur` — **poussé,
non répondu par `Fait`**, exactement comme `AudioMort`.

⚠️ **Vérifier le bras catch-all, plutôt que le supposer.**
`capteur/pont_media.rs` porte `Ok(autre) => return`, qui **tue le fil
`lire_le_media` EN SILENCE** — ce fichier documente ce défaut contre lui-même et
il a été payé **quatre fois** (D5 `Sommeil`, D6 `Part`, D7 `Audio`, D8
`PleinEcran`). `AudioVivant` circule enfant → capteur sur la connexion de
**commandes**, donc ne le traverse pas :

```bash
cd agent && grep -n "AudioVivant\|AudioMort" src/capteur/pont_media.rs
```
Attendu : aucune ligne. Et `transport/controle.rs` porte un `match` **exhaustif**
sur `AgentControl` qui, lui, se signale seul en refusant de compiler — c'est
l'autre trou du plan trouvé en cours d'exécution en D8.

- [ ] **Step 4 : implémenter — la preuve, puis l'annonce**

Dans `brancher_audio`, après un `next_packet()` réussi :

```rust
        // Le leg 6 de D9 : la remise à zéro du compteur de réarmements se fait
        // sur une PREUVE de son — ce paquet-ci —, jamais sur la décision
        // d'arbitrage qui, elle, ne peut pas mordre dans le cas majoritaire
        // (`sommeil/porteurs.rs`, une fenêtre seule de son groupe de PID
        // redevient porteuse automatiquement à la sortie de répit).
        if self.audio_reconstruit_sans_preuve {
            self.audio_reconstruit_sans_preuve = false;
            self.audio_vivant_a_annoncer = true;
        }
```

Dans `tick.rs`, remplacer la branche a1sexies (l. 285-292) :

```rust
        if self.source.rattachement_survenu() {
            self.audio_mort_signale = false;
        }
        // D10 : on tente d'abord de RECONSTRUIRE. `AudioMort` n'est plus le
        // premier geste mais le repli — celui du cas où l'arbre de processus a
        // disparu, et où seule la promotion d'une voisine peut encore rendre
        // du son au groupe.
        if !self.audio_mort_signale && self.reconstruire_ou_signaler(Instant::now()) {
            self.audio_mort_signale = true;
            self.source.signaler_audio_mort();
            return Ok(Tick::Continue);
        }
        if self.audio_vivant_a_annoncer {
            self.audio_vivant_a_annoncer = false;
            self.source.signaler_audio_vivant();
            return Ok(Tick::Continue);
        }
```

⚠️ **Ni `reconstruire_ou_signaler` ni `signaler_audio_vivant` ne doivent mettre
un paquet en file** : c'est l'invariant de drainage de ce fichier, et D6 a payé
deux rondes pour l'énoncer correctement. Les deux `return Ok(Tick::Continue)`
sont là pour cela.

- [ ] **Step 5 : le capteur remet à zéro sur la preuve**

Dans `capteur/sommeil.rs`, une fonction jumelle de celle qui traite `AudioMort` :

```rust
/// Le son de cette session est PROUVÉ revenu : le compteur de réarmements
/// repart de zéro.
pub fn signaler_audio_vivant(session: &str) {
    registre_verrouille().rearmements.remove(session);
}
```

Et **retirer** la remise à zéro de `sommeil/porteurs.rs:117`, en remplaçant son
commentaire par la raison du changement :

```rust
        // ⚠️ La remise à zéro vivait ICI jusqu'à D10, sur la DÉCISION
        // d'arbitrage — et la revue transverse de D9 a établi qu'elle ne
        // pouvait alors PAS mordre dans le cas majoritaire : pour une fenêtre
        // seule de son groupe de PID, la sortie de répit la rend
        // automatiquement porteuse, donc remet le compteur à zéro à chaque
        // tour. Le garde-fou était décoratif. Il repart désormais de
        // `sommeil::signaler_audio_vivant`, sur une PREUVE de son.
```

- [ ] **Step 6 : les exécuter et les voir verts**

```bash
cd agent && cargo test -p agent && cargo check --target x86_64-pc-windows-gnu
```

- [ ] **Step 7 : commit**

```bash
git add agent/src/transport/tick.rs agent/src/transport/piste_audio.rs agent/src/source.rs agent/src/capteur/protocole.rs agent/src/capteur/distante.rs agent/src/capteur/sommeil.rs agent/src/capteur/sommeil/porteurs.rs agent/src/transport/tick/tests/audio.rs
git commit -m "corrige(d10): AudioMort devient le repli, et AudioVivant prouve la reprise"
```

---

### Task 13 : `AUDIO_FAUTE_LECTURE`, l'injection qui rend le contrôle rouge

**Files:**
- Modify: `agent/src/windows_audio/fil.rs`, `scripts/run-agent.sh`

**Interfaces:**
- Consumes: la tâche 3 (le fil extrait).
- Produces: la variable de banc `AUDIO_FAUTE_LECTURE=<n>`.

- [ ] **Step 1 : implémenter**

Dans `fil.rs`, en tête de la boucle de lecture :

```rust
    // VARIABLE DE BANC, jamais une configuration livrée — même statut que
    // `PART_SONDAGE`. Elle existe parce qu'aucun déclencheur naturel de mort
    // de capture n'a pu être trouvé : les QUATRE de D9 (Restart-Service
    // Audiosrv, Stop/Start, Stop-Process audiodg, Disable/Enable-PnpDevice)
    // n'ont produit AUCUNE ligne `lecture audio échouée` sur neuf exécutions
    // versées — la capture *process loopback* suit l'ARBRE DE PROCESSUS, pas
    // le service ni le périphérique.
    //
    // ⚠️ Elle établit que le REMÈDE fonctionne, jamais qu'une cause naturelle
    // existe. Ne pas lire une recette qui l'emploie comme une preuve de
    // robustesse en production.
    let mut fautes_a_injecter: u32 = std::env::var("AUDIO_FAUTE_LECTURE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    if fautes_a_injecter > 0 {
        tracing::warn!(fautes_a_injecter, "injection de fautes de lecture audio ARMEE (banc)");
    }
```

et, à l'endroit exact où le résultat de `capture.read()` est examiné :

```rust
            let lecture = if fautes_a_injecter > 0 {
                fautes_a_injecter -= 1;
                Err(anyhow::anyhow!("faute injectée (AUDIO_FAUTE_LECTURE)"))
            } else {
                capture.read()
            };
```

⚠️ **Poser `n > LECTURES_ECHOUEES_MAX`** pour que la capture meure réellement :
en dessous, la tolérance de F3 (D7) l'absorbe et rien ne se passe — ce qui est
un faux négatif de méthode, indiscernable d'un remède qui marche.

- [ ] **Step 2 : `scripts/run-agent.sh`**

Ajouter `AUDIO_FAUTE_LECTURE` à la liste des variables transmises.
**Relever son numéro de ligne par `grep -n`**, ne pas recopier celui d'un
document : `CLAUDE.md` note que ces numéros ont dérivé deux fois.

- [ ] **Step 3 : vérifier**

```bash
cd agent && cargo check --target x86_64-pc-windows-gnu
grep -n "AUDIO_FAUTE_LECTURE" scripts/run-agent.sh
```

- [ ] **Step 4 : commit**

```bash
git add agent/src/windows_audio/fil.rs scripts/run-agent.sh
git commit -m "ajoute(d10): AUDIO_FAUTE_LECTURE, l'injection qui rend le controle rouge"
```

---

### Task 14 : recette ② — la reconstruction, et le déclencheur naturel

**Files:**
- Create: journaux dans `docs/superpowers/plans/journaux-multifenetres-d10/`

- [ ] **Step 1 : le montage — une session WebRTC VIVANTE est requise**

Découverte de méthode de D8, coûteuse : `Session::run()` est la seule boucle qui
consomme l'ordre audio du capteur. Un répondeur de viewport nu fait créer les
sorties et lancer les enfants, **mais aucune session ne s'établit et le son reste
`actif=false` à jamais** — faux négatif indiscernable du défaut.

Deux fenêtres Chrome `--app` **partageant un `--user-data-dir`** (un seul
`chrome.exe`, donc un seul groupe de PID). **Relever les PID avant de conclure** :

```bash
node scripts/winrm.js 'Get-Process chrome | Select-Object Id,MainWindowTitle | Format-List'
```

- [ ] **Step 2 : critère ③ — la reconstruction (injection de faute)**

Lancer avec `AUDIO_FAUTE_LECTURE=15` (> `LECTURES_ECHOUEES_MAX`), puis :

```bash
sed 's/\x1b\[[0-9;]*m//g' agent.log > agent-critere-2-1-plat.log
grep -c 'capture audio reconstruite' agent-critere-2-1-plat.log   # attendu : ≥ 1
grep -c 'compteurs audio' agent-critere-2-1-plat.log              # A
grep 'compteurs audio' agent-critere-2-1-plat.log | grep -c 'actif=true'  # B
```
⚠️ **`compteurs audio` est PÉRIODIQUE (30 s)** : sur une session plus courte,
`A = 0` ne veut pas dire « rien ne va mal », il veut dire « mesure non prise ».
Tenir la session au-delà de `REPORT_INTERVAL`.

Vérifier à l'oreille de l'instrument (`AnalyserNode`) que la **fréquence
dominante** reçue est bien celle assignée à cette fenêtre. ⚠️ **Ne jamais juger
au compte d'octets** : D7 a relevé `bytesReceived` en croissance sur un spectre
à −1000 dB.

- [ ] **Step 3 : le contrôle doit être vu ROUGE**

Rejouer **la même mesure sur le binaire de `main`** (avant D10), même
`AUDIO_FAUTE_LECTURE`. Attendu : **aucune** ligne `capture audio reconstruite`,
et le son ne revient jamais. Sans ce rouge, le vert du step 2 ne prouve rien.

- [ ] **Step 4 : critère ④ — le repli sur la promotion**

Reconstructeur voué à l'échec : tuer l'arbre de processus cible **après** avoir
armé l'injection. Attendu : `RECONSTRUCTIONS_MAX` refus, puis `AudioMort`, puis
la promotion de la voisine du même groupe.

- [ ] **Step 5 : le cinquième déclencheur — la tentative honnête**

```bash
node scripts/winrm.js 'Get-Process chrome | Where-Object { $_.MainWindowTitle -eq "" } | Select-Object -First 1 | Stop-Process -Force'
```
Tuer un **processus enfant** de l'arbre (un renderer, sans fenêtre principale),
qui laisse la fenêtre vivante. **Issue inconnue : elle peut ne rien produire.**
Relever `grep -c 'lecture audio échouée'` et **verser le résultat quel qu'il
soit** — un zéro est un relevé, pas un échec de tâche.

- [ ] **Step 6 : deuxième exécution, verser, commit**

```bash
git add docs/superpowers/plans/journaux-multifenetres-d10/
git commit -m "recette(d10): la capture audio se reconstruit, et le declencheur naturel reste a trouver"
```

---

# Famille ④ — l'A/B apparié (leg 3)

### Task 15 : rejouer l'A/B sur `set_desired_bitrate`, par paires

**Files:**
- Create: journaux dans `docs/superpowers/plans/journaux-multifenetres-d10/`

**Pourquoi le montage ne change pas, et le plan d'expérience si** : D9 a relevé
**+23,2 % entre bras** (moyennes 8,843 et 7,180 Mb/s cumulés) contre **+83,1 %
de variance intra-bras** (11,439 contre 6,247 sur le même bras **armé**). Le
bruit dépasse le signal, et le facteur dominant est la charge de l'hôte — D6
l'avait déjà nommé, sa recette ayant vu la performance varier d'un facteur 19 à
binaire et protocole identiques. **Ajouter des exécutions à un plan non apparié
ne convergera pas.**

- [ ] **Step 1 : le protocole**

Quatre **paires** consécutives, dans la même fenêtre de charge :
`ARMÉ, DÉSARMÉ, ARMÉ, DÉSARMÉ, …`, sans pause longue entre les deux membres
d'une paire. Bras désarmé : `PART_SONDAGE=0`.

⚠️ **À jouer APRÈS la tâche 10** : à trois fenêtres, la mesure ne se compare à
aucune campagne de D4 à D6, ce qui est exactement ce qui a privé D9 de sa base.
Relever le nombre de fenêtres attachées **avant** chaque exécution :

```bash
grep -c 'fenêtre attachée au capteur' agent-plat.log
```

- [ ] **Step 2 : relever, par paire**

Pour chaque exécution : trafic vidéo cumulé (Mb/s), `packetsLost`, et le nombre
de fenêtres. Puis, **par paire**, le signe de la différence `armé − désarmé`.

- [ ] **Step 3 : énoncer le verdict, qui peut être « n'établit rien »**

Le verdict porte **le nombre de paires et le signe de chacune**, jamais une
moyenne de bras seule. Quatre paires de même signe est un résultat ; trois sur
quatre n'en est pas un, et il faut le dire ainsi.

⚠️ **« N'établit rien » reste une issue acceptable** : la spec promet un plan
d'expérience, pas un verdict.

- [ ] **Step 4 : verser et committer**

```bash
git add docs/superpowers/plans/journaux-multifenetres-d10/
git commit -m "recette(d10): l'A/B set_desired_bitrate rejoue par paires appariees"
```

---

# Famille ③ — les legs froids

### Task 16 : leg 2 — la génération monotone sur le second registre

**Files:**
- Modify: `agent/src/capteur/serveur.rs` (registre `EN_ATTENTE_DE_MEDIA`, `oublier` l. 373)
- Test: `agent/src/capteur/serveur.rs` (module de tests) ou son enfant
  `serveur/instances.rs` si le registre y a été extrait

**Interfaces:**
- Consumes: le patron de `capteur/sommeil/registre.rs`.
- Produces: `fn oublier(session: &str, generation: u64)`.

- [ ] **Step 1 : écrire le test, qui doit échouer**

```rust
    /// La course F5, sur le SECOND registre. D9 l'a fermée sur le registre de
    /// sommeil seulement — le brief de sa tâche 10 ne nommait que `sommeil`.
    ///
    /// ⚠️ La génération se prend à l'ATTACHE, pas au lancement du processus :
    /// la première version du leg 2 en D9 frappait au lancement, et
    /// `retirer_est_perime` ne pouvait alors structurellement pas rendre
    /// `true` en production. Le défaut était dans la conception, pas dans
    /// l'exécution.
    #[test]
    fn un_oubli_perime_ne_retire_pas_l_attente_neuve() {
        let g1 = inscrire_attente("w-1", canal_d_essai());
        let g2 = inscrire_attente("w-1", canal_d_essai()); // réattache
        assert!(g2 > g1);
        oublier("w-1", g1); // l'ancien fil se réveille trop tard
        assert!(
            attente_existe("w-1"),
            "l'oubli de la génération 1 ne doit pas emporter la génération 2"
        );
        oublier("w-1", g2);
        assert!(!attente_existe("w-1"));
    }
```

- [ ] **Step 2 : le voir rouge**

```bash
cd agent && cargo test -p agent capteur::serveur 2>&1 | tail -20
```
Attendu : `this function takes 1 argument but 2 arguments were supplied`.

- [ ] **Step 3 : implémenter**

Transposer `sommeil/registre.rs` : la valeur du registre devient
`(Sender<std::fs::File>, u64)`, un compteur `prochaine_generation` monte à chaque
`insert`, et `oublier` compare avant de retirer :

```rust
/// Retire l'attente d'une session — **si et seulement si** la génération
/// présentée est bien la courante.
///
/// Sans cette comparaison, un `remove` inconditionnel émis par un fil tardif
/// emporte l'attente d'une RÉATTACHE arrivée entre-temps, et l'enfant neuf
/// attend un média que plus personne ne lui délivrera. Même course, même
/// remède et même patron que `capteur::sommeil::retirer`.
fn oublier(session: &str, generation: u64) {
    let mut garde = registre_verrouille();
    if garde.get(session).is_some_and(|(_, g)| *g != generation) {
        tracing::info!(%session, generation, "oubli périmé ignoré");
        return;
    }
    garde.remove(session);
}
```

Mettre à jour les appelants (l. 201 et 242) pour porter la génération.

- [ ] **Step 4 : vert, puis commit**

```bash
cd agent && cargo test -p agent && cargo check --target x86_64-pc-windows-gnu
git add agent/src/capteur/serveur.rs
git commit -m "corrige(d10): la course F5 est fermee sur le second registre"
```

---

### Task 17 : leg 9 — trancher la convention de module

**Files:**
- Modify: `agent/src/survie_verdict.rs` (et son parent), `CLAUDE.md`

- [ ] **Step 1 : relever les deux conventions**

```bash
cd agent && grep -rn '#\[path' src/ | head
ls src/*.rs
```
À la racine nue : `geometry.rs`, `sortie_dxgi.rs`, `survie_verdict.rs`.
Par `#[path]` chez le parent : `capture_reprise` (D2), `windows_source_sortie`
(D1), `windows_source_telemetrie` (D9).

- [ ] **Step 2 : choisir, et écrire la règle dans `CLAUDE.md`**

Règle retenue : **un module dont le nom porte celui de son parent
(`<parent>_<enfant>`) se déclare par `#[path]` chez ce parent** ; un module au
nom autonome (`geometry`, `sortie_dxgi`) vit à la racine. `survie_verdict` n'a
pas de parent dans son nom : **il reste à la racine**, et la déviation cesse
d'en être une.

⚠️ Si l'inspection montre que `survie_verdict` n'a qu'un seul appelant et que ce
nom devrait porter celui de ce parent, le renommer et le hisser — mais alors
**dire dans le commit lequel des deux cas s'applique**, jamais laisser le
lecteur deviner.

- [ ] **Step 3 : documenter dans le fichier lui-même**

L'en-tête de `survie_verdict.rs` cite déjà `geometry.rs` et `sortie_dxgi.rs`
comme précédents. Y ajouter la règle, pour que l'arbitrage ne se rejoue pas.

- [ ] **Step 4 : commit**

```bash
cd agent && cargo test -p agent
git add agent/src/survie_verdict.rs CLAUDE.md
git commit -m "tranche(d10): la convention de module enfant est ecrite une fois pour toutes"
```

---

### Task 18 : legs 7 et 10 — le maillon du `Resize`, et les constats parqués

**Files:**
- Modify: `docs/superpowers/plans/2026-08-06-multifenetres-solder-la-dette-resultats.md`
  et le rapport de la tâche 14 de D9
- Modify: `client/src/main.ts` (instrumentation)

- [ ] **Step 1 : corriger le rapport de la tâche 14 — AVANT toute mesure**

Il déclare le leg 10 de D8 **clos en sens inverse de ses propres journaux**, et
n'a pas été corrigé (interruption assumée de la ronde de D9). Le contredire est
le premier travail : sinon on repart de sa conclusion.

L'inventaire réel (`agent-critere-1-1.log` de D9) : `w-2` = 3 `Visibility` / **0
`Resize`** ; `w-3` = 3/3 ; `w-5` = 1/**0** ; `w-7` = 13/1.

⚠️ **Le canal vivant ÉCARTE « canal mort » pour ces deux sessions ; il ne
DÉSIGNE PAS le client.** C'est la nuance que la correction C3 de D8 a payée.

- [ ] **Step 2 : instrumenter la chaîne**

Dans `client/src/main.ts`, journaliser `video.clientWidth`/`clientHeight` à côté
de `window.innerWidth`/`innerHeight` à chaque déclenchement du `ResizeObserver`,
et à l'émission du `Resize`. C'est le maillon que D8 désignait sans l'avoir
mesuré.

- [ ] **Step 3 : rejouer et relever**

Sur une exécution à trois fenêtres au moins, compter par session :

```bash
grep 'contrôle reçu' agent-plat.log | grep -c 'Resize'
grep 'contrôle reçu' agent-plat.log | grep -o 'session=[^ ]*' | sort | uniq -c
```
La trace porte son `session` depuis D9 : l'attribution est enfin décidable.

- [ ] **Step 4 : reprendre les neuf constats parqués**

Les lire dans le rapport de la tâche 14 de D9 et, pour chacun, écrire **traité**
ou **requalifié** avec sa raison. ⚠️ L'un d'eux n'est pas un défaut d'instrument
mais un **comportement du produit** : la page-shell réémet `fenetre-ouverte`
pour une fenêtre déjà ouverte, mécanisme non élucidé — le dire tel quel.

- [ ] **Step 5 : commit**

```bash
cd client && npx vitest run
git add client/src/main.ts docs/superpowers/plans/
git commit -m "corrige(d10): le rapport de la tache 14 disait l'inverse de ses journaux"
```

---

### Task 19 : revue transverse de fin de branche, et `CLAUDE.md`

**Files:**
- Modify: `CLAUDE.md`, et tout fichier dont un commentaire est devenu faux

- [ ] **Step 1 : la cible propre de cette revue**

Elle a trouvé **cinq** défauts en D7, **trois** Critiques en D8, **six** en D9,
et **tous franchissaient une frontière de tâche**. En D10, sa cible nommée est
**les affirmations de code devenues fausses dans leur propre branche** — trois
des six défauts de D9 étaient de cette nature.

```bash
cd agent && grep -rn "la sortie \*\*est\*\* la fenêtre\|la sortie est la fenêtre\|plus rien à recadrer\|sortie DXGI entière" src/ | head -20
```
Chaque occurrence est relue contre le comportement d'après la famille ①.

- [ ] **Step 2 : balayer par le SENS, pas par la formule**

Leçon payée deux fois par ce dépôt : un balayage sur « non diagnostiqué » a
laissé survivre « pas diagnostiqué ». Chercher la **chose niée** en énumérant les
tournures — « pas / non / jamais / seulement / sans explication / reste ouvert ».

- [ ] **Step 3 : relever les tailles PAR LA COMMANDE, après les dernières éditions**

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
Le naufrage du « 487 » s'est rejoué **six fois** dans ce dépôt, dont une fois
dans la vague même qui le corrigeait ailleurs.

- [ ] **Step 5 : écrire la section D10 de `CLAUDE.md`**

Y porter : le verdict de chaque critère **avec son nombre d'exécutions**, ce que
D10 n'établit pas (§11 de la spec), les pièges neufs, et l'état des dix legs — y
compris ceux qui restent dus.

- [ ] **Step 6 : vérifications finales**

```bash
cd agent && cargo test -p agent 2>&1 | tail -3
cd agent && cargo check --target x86_64-pc-windows-gnu 2>&1 | tail -3
cd client && npx vitest run 2>&1 | tail -3
```
Reporter les **trois** nombres relevés dans le message de commit, comme D9.

- [ ] **Step 7 : commit**

```bash
git add CLAUDE.md agent/src docs/superpowers/plans/
git commit -m "docs(d10): resultats du sous-bloc, revue transverse, et les chiffres releves par la commande"
```

---

## Ordre et dépendances

```
1, 2, 3  (extractions, parallélisables entre elles)
   └── 4 → 5 → 6 → 7 → 8 → 9 → 10  (famille ①, strictement séquentielle)
                                      └── 15  (A/B : exige 10)
   └── 11 → 12 → 13 → 14            (famille ②, séquentielle ; 3 avant 13)
   └── 16, 17, 18                   (legs froids, indépendants)
                                      └── 19  (revue transverse : dernière)
```

⚠️ **La VM est un état partagé** : deux tâches de recette ne peuvent pas courir
en même temps. Les tâches 10, 14 et 15 sont **sérialisées entre elles**, quelle
que soit leur indépendance logique.
