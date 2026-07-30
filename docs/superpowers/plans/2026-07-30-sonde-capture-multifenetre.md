# Sonde de capture multi-fenêtres — plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Trancher par la mesure, avant de spécifier le chantier D, quelle voie de capture rend une image correcte par fenêtre quand les fenêtres se recouvrent, et à quel coût jusqu'à 8 fenêtres encodées.

**Architecture:** Deux temps. Le temps 1 éprouve la *viabilité* des quatre voies (WGC, un moniteur par fenêtre, tuilage disjoint, replis par fenêtre), **une voie par exécution du binaire** pour qu'un plantage n'emporte pas les autres. Le temps 2 passe au banc les seules survivantes : passe témoin (mires seules), capture seule, capture + encodage, de 1 à 8 fenêtres. Tout vit sous `agent/src/diagnostics/multifenetre/`, activé par variable d'environnement, sur le patron des sondes existantes (`CAPTURE_TEST`, `AUDIO_PROBE`).

**Tech Stack:** Rust 2021, `windows` 0.62 (Direct3D11, DXGI, WinRT Graphics.Capture), Media Foundation via `agent::encode`, exécution sur la VM Windows Server 2022 par `scripts/build-agent.sh` puis `scripts/run-agent.sh`.

**Spec:** `docs/superpowers/specs/2026-07-30-sonde-capture-multifenetre-design.md`

## Global Constraints

- **Cible : 8 fenêtres simultanées.** Assumé : la mesure touchera aussi le plafond NVENC.
- **Profondeur : capture + encodage.** Aucun transport, aucune `RTCPeerConnection`, aucune topologie N-sessions.
- **Porte éliminatoire n°1 — correction sous recouvrement.** Vérification par pixels dans le code, jamais à l'œil. Une voie qui échoue est éliminée **sans** mesure de cadence.
  - **Exception, la voie « tuilage disjoint ».** Elle n'évite pas le recouvrement : elle l'interdit par construction, en imposant une disposition. Lui appliquer cette porte l'éliminerait par définition, ce qui ne prouverait rien. Elle est jugée sur deux autres critères, établis en Task 7 : la surface que le bureau peut offrir (8 places d'une taille utile ?) et ce qui échappe à la disposition imposée (menus débordants, fenêtres déplacées par l'application elle-même).
- **Porte éliminatoire n°2 — chemin GPU.** Une voie qui rapatrie les pixels en mémoire centrale est écartée quelle que soit la qualité de son image.
- **Jamais de trace par trame.** Compteurs agrégés, journalisés à intervalle. Une trace par paquet a détruit une mesure au chantier NAT.
- **Session interactive obligatoire.** Ni la capture ni `SendInput` ne franchissent la session 0 de WinRM : tout passe par `scripts/run-agent.sh`.
- **Aucune installation de composant Windows sans accord humain.** Une fonctionnalité serveur implique un redémarrage de la VM ; la sonde constate et s'arrête.
- **Tout changement de configuration d'affichage est réversible** et remis en état en fin de sonde. La VM sert à d'autres travaux.
- **500 lignes maximum par fichier source.** Aucun fichier de ce plan ne naît au-dessus.
- **Français** pour les noms de modules, fonctions, tests et messages, comme le code récent du dépôt (`adaptation.rs`, `redimensionnement.rs`, `canaux.rs`).

## Ce qui est testé, et ce qui ne peut pas l'être

Les tâches 1 et 2 sont en **TDD strict** : elles produisent du code portable, exécuté par `cargo test -p agent` sur l'hôte Linux (163 tests y passent aujourd'hui). Ce sont précisément les deux morceaux dont une panne silencieuse rendrait tout le banc ininterprétable — un vérificateur cassé et un banc qui « ne trouve rien » se ressemblent trait pour trait.

Les tâches 3 à 10 sont `#[cfg(windows)]` et ne compilent que sur la VM : aucun test automatisé, comme `encode.rs`, `wasapi.rs` et les sondes existantes. Leur vérification est le relevé qu'elles produisent, journalisé et repris dans le document de résultats.

**Écart assumé à la spec §6.** La spec logeait `regions.rs` et `verification.rs` sous `diagnostics/multifenetre/`. Ils vivent en réalité à la racine `agent/src/`, en `disposition.rs` et `mire.rs`, aux côtés de `geometry.rs` : le module `multifenetre` est `#[cfg(windows)]`, et tout ce qu'il contient serait donc exclu de la compilation Linux — leurs tests ne tourneraient jamais. L'exigence de la spec (« portable, testé ») est respectée ; c'est son emplacement qui ne l'était pas.

## File Structure

| Fichier | Responsabilité | Testé |
| --- | --- | --- |
| `agent/src/mire.rs` | Couleur qu'une mire doit peindre, verdict sur un pixel lu | ✅ Linux |
| `agent/src/disposition.rs` | Découpe d'un bureau en tuiles disjointes | ✅ Linux |
| `agent/src/capture.rs` (modif.) | Énumération des sorties DXGI, capture sur une sortie choisie | ❌ VM |
| `agent/src/diagnostics/multifenetre.rs` | Aiguillage des sondes du chantier | ❌ VM |
| `agent/src/diagnostics/multifenetre/mires.rs` | N fenêtres D3D11 peintes et déplaçables | ❌ VM |
| `agent/src/diagnostics/multifenetre/disponibilite.rs` | Temps 1 : relevé DXGI et verdicts de viabilité | ❌ VM |
| `agent/src/diagnostics/multifenetre/wgc.rs` | Voie WGC : viabilité et implémentation de `VoieDeCapture` | ❌ VM |
| `agent/src/diagnostics/multifenetre/replis.rs` | `PrintWindow` et `DwmGetDxSharedSurface` | ❌ VM |
| `agent/src/diagnostics/multifenetre/voies.rs` | Trait `VoieDeCapture` et voie « duplication recadrée » | ❌ VM |
| `agent/src/diagnostics/multifenetre/banc.rs` | Temps 2 : passes témoin / capture / capture+encodage | ❌ VM |
| `agent/src/diagnostics/multifenetre/nvenc.rs` | Plafond d'encodeurs simultanés | ❌ VM |
| `scripts/sonde-multifenetre.sh` | Enchaînement des exécutions et récolte des journaux | ❌ |

---

### Task 1 : `mire.rs` — la couleur d'une mire et le verdict sur un pixel

**Files:**
- Create: `agent/src/mire.rs`
- Modify: `agent/src/main.rs:15` (ajouter `mod mire;` après `mod geometry;`)

**Interfaces:**
- Consumes: rien.
- Produces: `mire::MIRES_MAX: u8`, `mire::couleur_mire(id: u8, trame: u64) -> (u8, u8, u8)`, `mire::identifier(pixel: (u8, u8, u8)) -> Option<u8>`, `mire::Verdict` (variantes `Juste`, `Voisine(u8)`, `Noire`, `Inconnue`), `mire::verdict(attendu: u8, pixel: (u8, u8, u8)) -> Verdict`. Tâches 4, 6, 8, 9 en dépendent.

- [ ] **Step 1 : Écrire les tests qui échouent**

Créer `agent/src/mire.rs` avec, pour tout contenu, le bloc de tests :

```rust
//! Description des mires de la sonde multi-fenêtres : la couleur qu'une
//! fenêtre doit peindre, et le verdict rendu sur un pixel lu dans une image
//! capturée.
//!
//! Portable à dessein, comme `geometry.rs` : c'est le juge de la porte
//! éliminatoire du banc (« la fenêtre recouverte rend-elle toujours sa mire »).
//! Un juge cassé et un banc qui ne trouve rien produisent le même silence —
//! d'où les tests, exécutés sur l'hôte Linux.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_couleur_d_une_mire_identifie_sa_fenetre() {
        for id in 0..MIRES_MAX {
            assert_eq!(identifier(couleur_mire(id, 0)), Some(id));
            assert_eq!(identifier(couleur_mire(id, 1)), Some(id));
        }
    }

    #[test]
    fn l_alternance_de_trame_change_le_vert_sans_toucher_a_l_identite() {
        let paire = couleur_mire(3, 10);
        let impaire = couleur_mire(3, 11);
        assert_ne!(paire.1, impaire.1, "sans alternance visible, Desktop Duplication n'émet rien");
        assert_eq!(paire.0, impaire.0);
        assert_eq!(identifier(impaire), Some(3));
    }

    #[test]
    fn la_mire_d_une_fenetre_voisine_est_rejetee() {
        // Le cas exact que la porte éliminatoire doit attraper : la capture
        // d'une fenêtre recouverte rend le contenu de celle du dessus.
        assert_eq!(verdict(3, couleur_mire(4, 0)), Verdict::Voisine(4));
    }

    #[test]
    fn une_image_noire_est_rejetee() {
        // PrintWindow sur une fenêtre D3D rend typiquement du noir : c'est un
        // échec de voie, pas une mire inconnue.
        assert_eq!(verdict(0, (0, 0, 0)), Verdict::Noire);
    }

    #[test]
    fn un_ecart_de_lecture_dans_la_tolerance_reste_juste() {
        let (r, g, b) = couleur_mire(5, 0);
        assert_eq!(verdict(5, (r + 2, g + 2, b + 2)), Verdict::Juste);
    }

    #[test]
    fn une_couleur_etrangere_est_inconnue() {
        // Le fond du bureau, une console PowerShell : ni une mire, ni du noir.
        assert_eq!(verdict(0, (255, 255, 255)), Verdict::Inconnue);
        assert_eq!(identifier((1, 36, 86)), None);
    }
}
```

- [ ] **Step 2 : Lancer les tests pour vérifier qu'ils échouent**

Run: `cargo test -p agent mire:: 2>&1 | tail -20`
Expected: échec de compilation — `cannot find function couleur_mire`, `cannot find type Verdict`.

- [ ] **Step 3 : Écrire l'implémentation minimale**

Insérer avant le bloc `#[cfg(test)]` :

```rust
/// Nombre maximal de mires simultanées — la cible du banc.
pub const MIRES_MAX: u8 = 8;

/// Rouge de la mire n°0. Non nul : un rouge à 0 se confondrait avec du noir
/// sur une lecture bruitée.
const BASE_IDENTITE: u8 = 16;
/// Écart de rouge entre deux mires voisines. Très au-delà de la tolérance :
/// confondre deux mires ferait passer la porte éliminatoire à une voie qui
/// capture la mauvaise fenêtre, exactement le défaut recherché.
const PAS_IDENTITE: u8 = 24;
/// Bleu commun à toutes les mires : signe qu'on lit bien une mire.
const BLEU_MIRE: u8 = 96;
/// Vert des trames paires et impaires. L'alternance rend l'animation
/// détectable, et Desktop Duplication n'émet une image que si le bureau change.
const VERT_PAIR: u8 = 32;
const VERT_IMPAIR: u8 = 224;
/// Écart toléré par canal sur un pixel lu.
const TOLERANCE: i16 = 4;
/// En deçà, sur les trois canaux, l'image est tenue pour noire.
const SEUIL_NOIR: u8 = 12;

/// Ce qu'un pixel lu dit de la fenêtre attendue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// La mire attendue, à la tolérance près.
    Juste,
    /// La mire d'une AUTRE fenêtre : la voie capture le mauvais contenu.
    Voisine(u8),
    /// Image noire : la voie ne rend rien du contenu.
    Noire,
    /// Ni une mire, ni du noir.
    Inconnue,
}

/// Couleur que la mire `id` doit peindre à la trame `trame`, en `(r, g, b)`.
pub fn couleur_mire(id: u8, trame: u64) -> (u8, u8, u8) {
    debug_assert!(id < MIRES_MAX);
    let rouge = BASE_IDENTITE + id * PAS_IDENTITE;
    let vert = if trame % 2 == 0 { VERT_PAIR } else { VERT_IMPAIR };
    (rouge, vert, BLEU_MIRE)
}

fn proche(valeur: u8, attendu: u8) -> bool {
    (valeur as i16 - attendu as i16).abs() <= TOLERANCE
}

/// Identifie la fenêtre dont ce pixel porte la mire, s'il en porte une.
pub fn identifier(pixel: (u8, u8, u8)) -> Option<u8> {
    let (rouge, vert, bleu) = pixel;
    if !proche(bleu, BLEU_MIRE) {
        return None;
    }
    if !proche(vert, VERT_PAIR) && !proche(vert, VERT_IMPAIR) {
        return None;
    }
    let ecart = rouge as i16 - BASE_IDENTITE as i16;
    if ecart < 0 {
        return None;
    }
    let id = ecart / PAS_IDENTITE as i16;
    // Le reste doit tomber sur un multiple exact du pas, à la tolérance près :
    // sans cette vérification, toute nuance de rouge serait attribuée à une
    // mire par simple division.
    if (ecart - id * PAS_IDENTITE as i16).abs() > TOLERANCE || id >= MIRES_MAX as i16 {
        return None;
    }
    Some(id as u8)
}

/// Juge un pixel lu contre la mire attendue.
pub fn verdict(attendu: u8, pixel: (u8, u8, u8)) -> Verdict {
    let (rouge, vert, bleu) = pixel;
    if rouge < SEUIL_NOIR && vert < SEUIL_NOIR && bleu < SEUIL_NOIR {
        return Verdict::Noire;
    }
    match identifier(pixel) {
        Some(id) if id == attendu => Verdict::Juste,
        Some(id) => Verdict::Voisine(id),
        None => Verdict::Inconnue,
    }
}
```

- [ ] **Step 4 : Lancer les tests pour vérifier qu'ils passent**

Run: `cargo test -p agent mire:: 2>&1 | tail -10`
Expected: `test result: ok. 6 passed`

- [ ] **Step 5 : Déclarer le module et vérifier l'ensemble**

Ajouter dans `agent/src/main.rs`, après `mod geometry;` (ligne 15) :

```rust
mod mire;
```

Run: `cargo test -p agent 2>&1 | tail -5`
Expected: `169 passed; 0 failed`

- [ ] **Step 6 : Commit**

```bash
git add agent/src/mire.rs agent/src/main.rs
git commit -m "test(sonde): la mire dit quelle fenêtre a été capturée, et le prouve"
```

---

### Task 2 : `disposition.rs` — tuiles disjointes sur un bureau

**Files:**
- Create: `agent/src/disposition.rs`
- Modify: `agent/src/main.rs` (ajouter `mod disposition;`)

**Interfaces:**
- Consumes: `geometry::Rect`, `geometry::rects_overlap` (existants).
- Produces: `disposition::TUILE_MIN_LARGEUR: u32`, `disposition::TUILE_MIN_HAUTEUR: u32`, `disposition::tuiles(bureau: geometry::Rect, n: u32) -> Option<Vec<geometry::Rect>>`. Tâches 4 et 9 en dépendent.

- [ ] **Step 1 : Écrire les tests qui échouent**

Créer `agent/src/disposition.rs` :

```rust
//! Découpe d'un bureau en places disjointes, une par mire.
//!
//! Portable comme `geometry.rs`, et testé pour la même raison qu'y sont
//! testés les recadrages : une disposition dont deux places se recouvrent
//! ferait échouer la porte éliminatoire du banc sans qu'aucune voie de
//! capture soit en cause. Le banc accuserait la capture d'un défaut du banc.

use crate::geometry::Rect;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::rects_overlap;

    /// Le bureau relevé sur la VM le 30/07/2026.
    const BUREAU: Rect = Rect { x: 0, y: 0, width: 2400, height: 1080 };

    #[test]
    fn huit_places_ne_se_recouvrent_pas_et_tiennent_dans_le_bureau() {
        let places = tuiles(BUREAU, 8).expect("huit places sur 2400x1080");
        assert_eq!(places.len(), 8);
        for (i, a) in places.iter().enumerate() {
            assert!(a.x >= 0 && a.y >= 0);
            assert!(a.x as u32 + a.width <= BUREAU.width);
            assert!(a.y as u32 + a.height <= BUREAU.height);
            for b in places.iter().skip(i + 1) {
                assert!(!rects_overlap(*a, *b), "{a:?} recouvre {b:?}");
            }
        }
    }

    #[test]
    fn les_places_ont_des_dimensions_paires() {
        // L'encodeur H.264 refuse les dimensions impaires en 4:2:0, comme
        // le rappelle `geometry::crop_region`.
        for place in tuiles(BUREAU, 8).unwrap() {
            assert_eq!(place.width % 2, 0);
            assert_eq!(place.height % 2, 0);
        }
    }

    #[test]
    fn une_place_unique_couvre_presque_tout_le_bureau() {
        let places = tuiles(BUREAU, 1).unwrap();
        assert_eq!(places.len(), 1);
        assert_eq!(places[0].width, 2400);
        assert_eq!(places[0].height, 1080);
    }

    #[test]
    fn un_bureau_trop_petit_fait_refuser_la_disposition() {
        // Refuser franchement plutôt que rendre des places minuscules : une
        // mesure sur des fenêtres de 80x60 ne dirait rien du produit.
        let etroit = Rect { x: 0, y: 0, width: 640, height: 480 };
        assert_eq!(tuiles(etroit, 8), None);
    }

    #[test]
    fn le_nombre_de_places_demande_est_respecte_meme_si_la_grille_est_plus_large() {
        // Sept places tiennent dans une grille 3x3 : deux cases restent vides,
        // et on ne doit pas rendre neuf places pour autant.
        assert_eq!(tuiles(BUREAU, 7).unwrap().len(), 7);
    }
}
```

- [ ] **Step 2 : Lancer les tests pour vérifier qu'ils échouent**

Run: `cargo test -p agent disposition:: 2>&1 | tail -20`
Expected: échec de compilation — `cannot find function tuiles`.

- [ ] **Step 3 : Écrire l'implémentation minimale**

Insérer avant le bloc `#[cfg(test)]` :

```rust
/// Dimensions en deçà desquelles une place ne vaut pas la mesure.
pub const TUILE_MIN_LARGEUR: u32 = 320;
pub const TUILE_MIN_HAUTEUR: u32 = 240;

/// Découpe `bureau` en `n` places disjointes, en grille la plus carrée
/// possible.
///
/// Renvoie `None` si les places descendraient sous `TUILE_MIN_*` : mieux vaut
/// un refus net qu'une mesure sur des fenêtres trop petites pour représenter
/// quoi que ce soit du produit.
pub fn tuiles(bureau: Rect, n: u32) -> Option<Vec<Rect>> {
    if n == 0 {
        return None;
    }
    // Grille la plus carrée possible : `colonnes` est le plus petit entier
    // dont le carré atteint `n`. Calculé par boucle plutôt que par
    // `(n as f64).sqrt().ceil()`, dont l'arrondi flottant est faux pour
    // certains carrés parfaits selon la plateforme.
    let mut colonnes = 1u32;
    while colonnes * colonnes < n {
        colonnes += 1;
    }
    let lignes = n.div_ceil(colonnes);

    let largeur = (bureau.width / colonnes) & !1;
    let hauteur = (bureau.height / lignes) & !1;
    if largeur < TUILE_MIN_LARGEUR || hauteur < TUILE_MIN_HAUTEUR {
        return None;
    }

    let mut places = Vec::with_capacity(n as usize);
    for index in 0..n {
        let colonne = index % colonnes;
        let ligne = index / colonnes;
        places.push(Rect {
            x: bureau.x + (colonne * largeur) as i32,
            y: bureau.y + (ligne * hauteur) as i32,
            width: largeur,
            height: hauteur,
        });
    }
    Some(places)
}
```

- [ ] **Step 4 : Lancer les tests pour vérifier qu'ils passent**

Run: `cargo test -p agent disposition:: 2>&1 | tail -10`
Expected: `test result: ok. 5 passed`

Note : `une_place_unique_couvre_presque_tout_le_bureau` attend 2400×1080, ce que donne bien `colonnes = 1, lignes = 1` (2400 et 1080 sont pairs).

- [ ] **Step 5 : Déclarer le module et vérifier l'ensemble**

Ajouter `mod disposition;` dans `agent/src/main.rs`, en respectant l'ordre alphabétique des déclarations sans `cfg` (avant `mod frames;`).

Run: `cargo test -p agent 2>&1 | tail -5`
Expected: `174 passed; 0 failed`

- [ ] **Step 6 : Commit**

```bash
git add agent/src/disposition.rs agent/src/main.rs
git commit -m "test(sonde): places disjointes pour les mires, refus net si trop petites"
```

---

### Task 3 : `capture.rs` — énumérer les sorties DXGI et capturer celle qu'on choisit

**Files:**
- Modify: `agent/src/capture.rs:65-150` (`DesktopCapture::new`), `agent/src/capture.rs:312-337` (`find_desktop_output`)

**Interfaces:**
- Consumes: `geometry::Rect`.
- Produces: `capture::SortieDxgi { index_adaptateur: u32, index_sortie: u32, adaptateur: String, nom_sortie: String, attachee_au_bureau: bool, rect: geometry::Rect }`, `capture::enumerer_sorties() -> Result<Vec<SortieDxgi>>`, `DesktopCapture::sur_sortie(index_adaptateur: u32, index_sortie: u32) -> Result<DesktopCapture>`. Tâches 5, 7 et 9 en dépendent. `DesktopCapture::new()` garde son comportement actuel.

**Pourquoi cette modification est nécessaire** : la voie « un moniteur virtuel par fenêtre » exige de dupliquer une sortie *choisie*, alors que `find_desktop_output` retient aujourd'hui la **première** sortie attachée au bureau et n'expose rien d'autre. C'est la seule modification de ce plan touchant le chemin de production, et elle est additive : `new()` continue de déléguer au même choix.

- [ ] **Step 1 : Extraire l'énumération**

Remplacer `find_desktop_output` (lignes 312-337) par :

```rust
/// Ce qu'on sait d'une sortie DXGI, sans en dupliquer quoi que ce soit.
#[derive(Debug, Clone)]
pub struct SortieDxgi {
    pub index_adaptateur: u32,
    pub index_sortie: u32,
    pub adaptateur: String,
    pub nom_sortie: String,
    pub attachee_au_bureau: bool,
    /// Position et dimensions dans les coordonnées du bureau virtuel.
    pub rect: crate::geometry::Rect,
}

/// Énumère toutes les sorties de tous les adaptateurs.
///
/// Sert au relevé du temps 1 de la sonde multi-fenêtres : deux documents du
/// dépôt se contredisent sur l'adaptateur qui pilote réellement le bureau
/// (`plans/fix-debit-socket-report.md:163` contre le commit `4493b24`), et
/// c'est ce relevé qui tranche.
pub fn enumerer_sorties() -> Result<Vec<SortieDxgi>> {
    let factory: IDXGIFactory1 =
        unsafe { CreateDXGIFactory1() }.context("création de la fabrique DXGI")?;
    let mut sorties = Vec::new();
    let mut index_adaptateur = 0u32;
    while let Ok(adapter) = unsafe { factory.EnumAdapters1(index_adaptateur) } {
        let adaptateur = match unsafe { adapter.GetDesc1() } {
            Ok(desc) => String::from_utf16_lossy(&desc.Description)
                .trim_end_matches('\0')
                .trim()
                .to_string(),
            Err(_) => "<inconnu>".to_string(),
        };
        let mut index_sortie = 0u32;
        while let Ok(output) = unsafe { adapter.EnumOutputs(index_sortie) } {
            if let Ok(desc) = unsafe { output.GetDesc() } {
                let r = desc.DesktopCoordinates;
                sorties.push(SortieDxgi {
                    index_adaptateur,
                    index_sortie,
                    adaptateur: adaptateur.clone(),
                    nom_sortie: String::from_utf16_lossy(&desc.DeviceName)
                        .trim_end_matches('\0')
                        .to_string(),
                    attachee_au_bureau: desc.AttachedToDesktop.as_bool(),
                    rect: crate::geometry::Rect {
                        x: r.left,
                        y: r.top,
                        width: (r.right - r.left).max(0) as u32,
                        height: (r.bottom - r.top).max(0) as u32,
                    },
                });
            }
            index_sortie += 1;
        }
        index_adaptateur += 1;
    }
    Ok(sorties)
}

/// Trouve l'adaptateur et la sortie qui composent le bureau.
fn find_desktop_output(factory: &IDXGIFactory1) -> Result<(IDXGIAdapter1, IDXGIOutput1)> {
    ouvrir_sortie(factory, None)
}

/// Ouvre une sortie précise, ou la première attachée au bureau si `cible` est
/// `None` (comportement historique de `find_desktop_output`).
fn ouvrir_sortie(
    factory: &IDXGIFactory1,
    cible: Option<(u32, u32)>,
) -> Result<(IDXGIAdapter1, IDXGIOutput1)> {
    let mut index_adaptateur = 0u32;
    while let Ok(adapter) = unsafe { factory.EnumAdapters1(index_adaptateur) } {
        let mut index_sortie = 0u32;
        while let Ok(output) = unsafe { adapter.EnumOutputs(index_sortie) } {
            let desc = match unsafe { output.GetDesc() } {
                Ok(desc) => desc,
                Err(_) => {
                    index_sortie += 1;
                    continue;
                }
            };
            let retenue = match cible {
                Some((a, s)) => a == index_adaptateur && s == index_sortie,
                None => desc.AttachedToDesktop.as_bool(),
            };
            if retenue {
                let name = match unsafe { adapter.GetDesc1() } {
                    Ok(adapter_desc) => String::from_utf16_lossy(&adapter_desc.Description)
                        .trim_end_matches('\0')
                        .to_string(),
                    Err(_) => "<inconnu>".to_string(),
                };
                tracing::info!(
                    adaptateur = %name, index_adaptateur, index_sortie,
                    attachee = desc.AttachedToDesktop.as_bool(),
                    "sortie retenue pour la duplication"
                );
                return Ok((adapter.clone(), output.cast()?));
            }
            index_sortie += 1;
        }
        index_adaptateur += 1;
    }
    match cible {
        Some((a, s)) => bail!("aucune sortie DXGI à l'index adaptateur {a}, sortie {s}"),
        None => bail!("aucune sortie attachée au bureau : la session est-elle interactive ?"),
    }
}
```

- [ ] **Step 2 : Ouvrir `DesktopCapture` sur une sortie choisie**

Dans `agent/src/capture.rs`, remplacer l'en-tête de `impl DesktopCapture` (ligne 65) par :

```rust
impl DesktopCapture {
    pub fn new() -> Result<Self> {
        Self::ouvrir(None)
    }

    /// Duplique une sortie DXGI précise, désignée par ses index
    /// d'énumération (voir `enumerer_sorties`).
    ///
    /// Ajouté pour la voie « un moniteur virtuel par fenêtre » de la sonde
    /// multi-fenêtres : elle duplique N sorties distinctes, là où `new()` ne
    /// sait ouvrir que la première attachée au bureau.
    pub fn sur_sortie(index_adaptateur: u32, index_sortie: u32) -> Result<Self> {
        Self::ouvrir(Some((index_adaptateur, index_sortie)))
    }

    fn ouvrir(cible: Option<(u32, u32)>) -> Result<Self> {
```

Puis, dans le corps déplacé, remplacer l'appel `let (adapter, output) = find_desktop_output(&factory)?;` par :

```rust
        let (adapter, output) = ouvrir_sortie(&factory, cible)?;
```

Le reste du corps (création du périphérique D3D11, `SetMultithreadProtected`, `DuplicateOutput`) est inchangé.

- [ ] **Step 3 : Vérifier que rien n'a cassé côté hôte**

Run: `cargo test -p agent 2>&1 | tail -5`
Expected: `174 passed; 0 failed` — le module est `#[cfg(windows)]`, l'hôte ne compile que le reste ; ce test vérifie l'absence de régression ailleurs.

- [ ] **Step 4 : Compiler sur la VM**

```bash
set -a && source .env && set +a
virsh list --all                       # « en cours d'exécution » attendu
scripts/build-agent.sh
```

Expected: compilation sans erreur. En cas d'échec sur `find_desktop_output` désormais inutilisée, la conserver telle quelle : elle documente le comportement historique et reste appelée par `new()`.

- [ ] **Step 5 : Commit**

```bash
git add agent/src/capture.rs
git commit -m "feat(sonde): énumérer les sorties DXGI et dupliquer celle qu'on choisit"
```

---

### Task 4 : `mires.rs` — N fenêtres D3D11 peintes, déplaçables, recouvrables

**Files:**
- Create: `agent/src/diagnostics/multifenetre.rs`, `agent/src/diagnostics/multifenetre/mires.rs`
- Modify: `agent/src/diagnostics.rs` (déclarer `#[cfg(windows)] mod multifenetre;`)

**Interfaces:**
- Consumes: `mire::couleur_mire`, `disposition::tuiles`, `geometry::Rect`.
- Produces: `mires::Mires` avec `ouvrir(device: &ID3D11Device, places: &[Rect]) -> Result<Mires>`, `peindre(&mut self) -> Result<()>`, `pomper(&self)`, `hwnd(&self, id: u8) -> Result<HWND>`, `place(&self, id: u8) -> Result<Rect>`, `recouvrir(&mut self, dessus: u8, dessous: u8) -> Result<()>`, `trame(&self) -> u64`, `nombre(&self) -> u8`. Tâches 6, 8, 9 en dépendent.

**Pourquoi D3D11 et pas GDI** : `PrintWindow` échoue précisément sur le contenu D3D. Une mire peinte en GDI validerait la voie des replis, qui s'effondrerait ensuite devant un vrai jeu.

- [ ] **Step 1 : Créer le module d'aiguillage**

Créer `agent/src/diagnostics/multifenetre.rs` :

```rust
//! Sonde préalable au chantier D : quelle voie de capture rend une image
//! correcte par fenêtre quand les fenêtres se recouvrent, et à quel coût.
//!
//! Spec : `docs/superpowers/specs/2026-07-30-sonde-capture-multifenetre-design.md`.
//!
//! Chaque voie s'active par SA PROPRE variable d'environnement, et chaque
//! exécution du binaire n'en éprouve qu'une : `captureservice.dll` plantait
//! en `0xc0000005` au jalon 1, et un plantage de ce genre emporte le
//! processus entier. Les éprouver ensemble ferait perdre les autres avec la
//! première.

pub(super) mod mires;

use anyhow::Result;

/// Renvoie `true` si une sonde de ce chantier a tourné.
pub(super) fn aiguiller() -> Result<bool> {
    Ok(false)
}
```

Ajouter dans `agent/src/diagnostics.rs`, auprès des autres déclarations de modules :

```rust
#[cfg(windows)]
mod multifenetre;
```

et, dans `aiguiller()`, juste avant le `Ok(false)` final :

```rust
    // Sondes du chantier D (capture multi-fenêtres). Placées en dernier :
    // elles créent leurs propres fenêtres et n'interfèrent avec aucune des
    // sondes ci-dessus, mais elles perturbent la disposition du bureau.
    #[cfg(windows)]
    if multifenetre::aiguiller()? {
        return Ok(true);
    }
```

- [ ] **Step 2 : Écrire les mires**

Créer `agent/src/diagnostics/multifenetre/mires.rs` :

```rust
//! Fenêtres de test de la sonde multi-fenêtres : N fenêtres sans bordure,
//! peintes par D3D11 d'une couleur qui encode leur identité et leur numéro
//! de trame (`crate::mire`).
//!
//! Le rendu passe par une swapchain D3D11 et non par GDI. `PrintWindow`
//! échoue précisément sur le contenu D3D : une mire peinte en GDI validerait
//! la voie des replis, qui s'effondrerait ensuite devant un vrai jeu.
//!
//! L'animation n'est pas décorative — Desktop Duplication n'émet une image
//! que lorsque le bureau change. Sans alternance de couleur, le banc mesure
//! une capture qui ne reçoit rien (piège déjà payé au jalon 1).

use anyhow::{anyhow, Context, Result};
use windows::core::{w, Interface, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Direct3D11::{
    ID3D11Device, ID3D11DeviceContext, ID3D11RenderTargetView, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    IDXGIDevice, IDXGIFactory2, IDXGISwapChain1, DXGI_SWAP_CHAIN_DESC1,
    DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL, DXGI_USAGE_RENDER_TARGET_OUTPUT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, PeekMessageW, RegisterClassW,
    SetWindowPos, ShowWindow, TranslateMessage, HWND_TOP, MSG, PM_REMOVE, SWP_NOACTIVATE,
    SW_SHOWNOACTIVATE, WNDCLASSW, WS_EX_NOACTIVATE, WS_POPUP, WS_VISIBLE,
};

use crate::geometry::Rect;
use crate::mire;

const CLASSE: PCWSTR = w!("SondeMultifenetreMire");

struct Fenetre {
    id: u8,
    hwnd: HWND,
    place: Rect,
    swapchain: IDXGISwapChain1,
    cible: ID3D11RenderTargetView,
}

pub(super) struct Mires {
    fenetres: Vec<Fenetre>,
    contexte: ID3D11DeviceContext,
    trame: u64,
}

impl Mires {
    /// Ouvre une fenêtre par place, peinte et visible, sans jamais prendre le
    /// focus (`WS_EX_NOACTIVATE`) : une mire qui volerait le premier plan
    /// changerait le recouvrement que le banc met en scène.
    pub(super) fn ouvrir(device: &ID3D11Device, places: &[Rect]) -> Result<Self> {
        anyhow::ensure!(
            places.len() <= mire::MIRES_MAX as usize,
            "au plus {} mires, {} demandées",
            mire::MIRES_MAX,
            places.len()
        );
        enregistrer_classe()?;

        let dxgi: IDXGIDevice = device.cast().context("IDXGIDevice depuis le périphérique D3D11")?;
        let adaptateur = unsafe { dxgi.GetAdapter() }.context("adaptateur DXGI")?;
        let fabrique: IDXGIFactory2 =
            unsafe { adaptateur.GetParent() }.context("fabrique DXGI depuis l'adaptateur")?;
        let contexte = unsafe { device.GetImmediateContext() }.context("contexte immédiat")?;

        let mut fenetres = Vec::with_capacity(places.len());
        for (index, place) in places.iter().enumerate() {
            let hwnd = unsafe {
                CreateWindowExW(
                    WS_EX_NOACTIVATE,
                    CLASSE,
                    PCWSTR::null(),
                    WS_POPUP | WS_VISIBLE,
                    place.x,
                    place.y,
                    place.width as i32,
                    place.height as i32,
                    None,
                    None,
                    None,
                    None,
                )
            }
            .context("création d'une fenêtre de mire")?;
            let _ = unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };

            let desc = DXGI_SWAP_CHAIN_DESC1 {
                Width: place.width,
                Height: place.height,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: 2,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                ..Default::default()
            };
            let swapchain = unsafe {
                fabrique.CreateSwapChainForHwnd(device, hwnd, &desc, None, None)
            }
            .context("création de la swapchain d'une mire")?;

            let arriere: ID3D11Texture2D =
                unsafe { swapchain.GetBuffer(0) }.context("tampon arrière de la swapchain")?;
            let mut cible = None;
            unsafe { device.CreateRenderTargetView(&arriere, None, Some(&mut cible)) }
                .context("vue de rendu d'une mire")?;

            fenetres.push(Fenetre {
                id: index as u8,
                hwnd,
                place: *place,
                swapchain,
                cible: cible.ok_or_else(|| anyhow!("vue de rendu absente"))?,
            });
        }

        Ok(Self { fenetres, contexte, trame: 0 })
    }

    /// Peint une trame sur toutes les mires et la présente.
    pub(super) fn peindre(&mut self) -> Result<()> {
        for fenetre in &self.fenetres {
            let (r, g, b) = mire::couleur_mire(fenetre.id, self.trame);
            // Format UNORM non-sRGB : la valeur flottante est écrite telle
            // quelle dans l'octet, donc `r/255` rend exactement `r` à la
            // lecture. Un format `_SRGB` imposerait une conversion et la
            // vérification par pixels échouerait sur une mire pourtant juste.
            let couleur = [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0];
            unsafe { self.contexte.ClearRenderTargetView(&fenetre.cible, &couleur) };
            unsafe { fenetre.swapchain.Present(0, Default::default()) }
                .ok()
                .context("présentation d'une mire")?;
        }
        self.trame += 1;
        Ok(())
    }

    /// Vide la file de messages des fenêtres. Sans cela Windows les tient
    /// pour figées et cesse de les composer.
    pub(super) fn pomper(&self) {
        let mut message = MSG::default();
        while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
            let _ = unsafe { TranslateMessage(&message) };
            unsafe { DispatchMessageW(&message) };
        }
    }

    /// Met la mire `dessus` par-dessus la mire `dessous`, en la déplaçant sur
    /// sa place et en la portant au premier plan. C'est la mise en scène de
    /// la porte éliminatoire : la mire recouverte doit rester capturable.
    pub(super) fn recouvrir(&mut self, dessus: u8, dessous: u8) -> Result<()> {
        let cible = self.place(dessous)?;
        let hwnd = self.hwnd(dessus)?;
        unsafe {
            SetWindowPos(
                hwnd,
                Some(HWND_TOP),
                cible.x,
                cible.y,
                cible.width as i32,
                cible.height as i32,
                SWP_NOACTIVATE,
            )
        }
        .context("déplacement d'une mire par-dessus une autre")?;
        Ok(())
    }

    pub(super) fn hwnd(&self, id: u8) -> Result<HWND> {
        self.fenetres
            .iter()
            .find(|f| f.id == id)
            .map(|f| f.hwnd)
            .ok_or_else(|| anyhow!("aucune mire n°{id}"))
    }

    pub(super) fn place(&self, id: u8) -> Result<Rect> {
        self.fenetres
            .iter()
            .find(|f| f.id == id)
            .map(|f| f.place)
            .ok_or_else(|| anyhow!("aucune mire n°{id}"))
    }

    pub(super) fn trame(&self) -> u64 {
        self.trame
    }

    pub(super) fn nombre(&self) -> u8 {
        self.fenetres.len() as u8
    }
}

impl Drop for Mires {
    fn drop(&mut self) {
        // Une mire orpheline fausserait la mesure suivante, et le banc est
        // lancé plusieurs fois de suite.
        for fenetre in &self.fenetres {
            let _ = unsafe { DestroyWindow(fenetre.hwnd) };
        }
    }
}

fn enregistrer_classe() -> Result<()> {
    use std::sync::Once;
    static UNE_FOIS: Once = Once::new();
    let mut resultat = Ok(());
    UNE_FOIS.call_once(|| {
        let classe = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            lpszClassName: CLASSE,
            ..Default::default()
        };
        // `RegisterClassW` rend 0 en cas d'échec. Un second enregistrement de
        // la même classe échouerait aussi — d'où le `Once`.
        if unsafe { RegisterClassW(&classe) } == 0 {
            resultat = Err(anyhow!(
                "enregistrement de la classe de fenêtre : {}",
                windows::core::Error::from_win32()
            ));
        }
    });
    resultat
}

unsafe extern "system" fn procedure(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    DefWindowProcW(hwnd, message, wparam, lparam)
}
```

- [ ] **Step 3 : Compiler sur la VM**

```bash
set -a && source .env && set +a
scripts/build-agent.sh
```

Expected: compilation sans erreur. Les écarts d'API `windows` 0.62 les plus probables : `ShowWindow`/`DestroyWindow`/`Present` rendent `BOOL` ou `HRESULT` selon la fonction — suivre les messages du compilateur, et documenter tout écart en commentaire à côté de l'appel, comme le fait déjà `window.rs`.

- [ ] **Step 4 : Commit**

```bash
git add agent/src/diagnostics.rs agent/src/diagnostics/multifenetre.rs agent/src/diagnostics/multifenetre/mires.rs
git commit -m "feat(sonde): mires D3D11, une fenêtre par place, recouvrables à la demande"
```

---

### Task 5 : Temps 1 — relever les sorties DXGI et lever l'écart documentaire

**Files:**
- Create: `agent/src/diagnostics/multifenetre/disponibilite.rs`, `scripts/sonde-multifenetre.sh`
- Modify: `agent/src/diagnostics/multifenetre.rs` (aiguillage), `scripts/run-agent.sh` (heredoc)

**Interfaces:**
- Consumes: `capture::enumerer_sorties`, `capture::SortieDxgi`.
- Produces: `disponibilite::relever_dxgi() -> Result<()>`, activée par `MULTIFENETRE_DXGI=1`.

- [ ] **Step 1 : Écrire le relevé**

Créer `agent/src/diagnostics/multifenetre/disponibilite.rs` :

```rust
//! Temps 1 de la sonde : la viabilité de chaque voie, une par exécution.

use anyhow::Result;

/// Relève toutes les sorties DXGI de la machine.
///
/// Premier acte de la sonde, parce que deux documents du dépôt se
/// contredisent sur l'adaptateur qui pilote le bureau
/// (`plans/fix-debit-socket-report.md:163` contre le commit `4493b24`) et que
/// tout le dimensionnement des voies 2 et 3 en dépend.
pub(super) fn relever_dxgi() -> Result<()> {
    let sorties = crate::capture::enumerer_sorties()?;
    tracing::info!(nombre = sorties.len(), "sorties DXGI relevées");
    for sortie in &sorties {
        tracing::info!(
            adaptateur = %sortie.adaptateur,
            index_adaptateur = sortie.index_adaptateur,
            index_sortie = sortie.index_sortie,
            nom = %sortie.nom_sortie,
            attachee = sortie.attachee_au_bureau,
            x = sortie.rect.x,
            y = sortie.rect.y,
            largeur = sortie.rect.width,
            hauteur = sortie.rect.height,
            "sortie"
        );
    }
    let attachees = sorties.iter().filter(|s| s.attachee_au_bureau).count();
    tracing::info!(
        attachees,
        "verdict : {} sortie(s) attachée(s) au bureau — la voie « un moniteur \
         par fenêtre » exige d'en obtenir 8",
        attachees
    );
    Ok(())
}
```

- [ ] **Step 2 : Aiguiller**

Dans `agent/src/diagnostics/multifenetre.rs`, remplacer le corps d'`aiguiller` :

```rust
pub(super) mod disponibilite;
pub(super) mod mires;

use anyhow::Result;

pub(super) fn aiguiller() -> Result<bool> {
    // Relevé DXGI : quelles sorties existent, laquelle porte le bureau.
    if std::env::var("MULTIFENETRE_DXGI").is_ok() {
        disponibilite::relever_dxgi()?;
        return Ok(true);
    }
    Ok(false)
}
```

- [ ] **Step 3 : Transmettre la variable à la session interactive**

Dans `scripts/run-agent.sh`, ajouter au heredoc, après la ligne `INPUT_LINEARITY_NEUTRALISER` :

```bash
${MULTIFENETRE_DXGI:+\$env:MULTIFENETRE_DXGI = '$MULTIFENETRE_DXGI'}
```

- [ ] **Step 4 : Écrire le script d'enchaînement**

Créer `scripts/sonde-multifenetre.sh` :

```bash
#!/usr/bin/env bash
# Enchaîne les sondes du chantier D, UNE PAR EXÉCUTION du binaire.
#
# Ce n'est pas une commodité : `captureservice.dll` plantait en 0xc0000005 au
# jalon 1, et un plantage de ce genre emporte le processus. Éprouver les
# quatre voies dans une même exécution ferait perdre les trois autres avec la
# première. Chaque voie tourne donc seule, et son journal est récolté avant
# la suivante.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
JOURNAUX="$ROOT/docs/superpowers/plans/journaux-sonde-multifenetre"
mkdir -p "$JOURNAUX"

# `run-agent.sh` exige les identifiants Windows ; les charger ici évite
# d'imposer un `set -a && source .env` à chaque invocation.
if [ -f "$ROOT/.env" ]; then
    set -a
    # shellcheck disable=SC1091
    . "$ROOT/.env"
    set +a
fi

executer() {
    local nom="$1"; shift
    # Lu à CHAQUE appel : le banc du temps 2 relève SONDE_SECS pour ses
    # propres passes, sans que les sondes courtes du temps 1 en héritent.
    local secs="${SONDE_SECS:-25}"
    echo "── sonde : $nom ─────────────────────────────"
    rm -f /media/vm/dev/agent.log
    env "$@" "$ROOT/scripts/run-agent.sh"
    sleep "$secs"
    cp /media/vm/dev/agent.log "$JOURNAUX/$nom.log" 2>/dev/null \
        || echo "AUCUN JOURNAL — la sonde a-t-elle planté au démarrage ?"
    tail -30 "$JOURNAUX/$nom.log" 2>/dev/null || true
}

executer dxgi MULTIFENETRE_DXGI=1
```

```bash
chmod +x scripts/sonde-multifenetre.sh
```

- [ ] **Step 5 : Compiler, exécuter, lire**

```bash
set -a && source .env && set +a
virsh list --all
scripts/build-agent.sh
scripts/sonde-multifenetre.sh
```

Expected: le journal liste chaque sortie DXGI avec son adaptateur et son état d'attachement. **Consigner le relevé** : c'est lui qui tranche laquelle de `fix-debit-socket-report.md:163` ou du commit `4493b24` dit vrai.

- [ ] **Step 6 : Commit**

```bash
git add agent/src/diagnostics/multifenetre.rs agent/src/diagnostics/multifenetre/disponibilite.rs scripts/run-agent.sh scripts/sonde-multifenetre.sh
git commit -m "feat(sonde): relever les sorties DXGI, et trancher quel adaptateur porte le bureau"
```

---

### Task 6 : Temps 1 — WGC, re-test honnête

**Files:**
- Create: `agent/src/diagnostics/multifenetre/wgc.rs`
- Modify: `agent/Cargo.toml` (features `windows`), `agent/src/diagnostics/multifenetre.rs`, `scripts/run-agent.sh`, `scripts/sonde-multifenetre.sh`

**Interfaces:**
- Consumes: `mires::Mires`, `mire::verdict`, `diagnostics::pixels::read_pixel`, `capture::DesktopCapture` (pour le périphérique D3D11).
- Produces: `wgc::eprouver() -> Result<()>`, activée par `MULTIFENETRE_WGC=1`.

- [ ] **Step 1 : Ajouter les features WinRT**

Dans `agent/Cargo.toml`, section `[target.'cfg(windows)'.dependencies]`, ajouter à la liste `features` de `windows` :

```toml
    "Graphics_Capture",
    "Graphics_DirectX",
    "Graphics_DirectX_Direct3D11",
    "Win32_System_WinRT_Direct3D11",
    "Win32_System_WinRT_Graphics_Capture",
```

- [ ] **Step 2 : Écrire la sonde**

Créer `agent/src/diagnostics/multifenetre/wgc.rs` :

```rust
//! Voie 1 : `Windows.Graphics.Capture`, re-test honnête.
//!
//! Abandonnée au jalon 1 (commit `4493b24`) : `captureservice.dll` plantait
//! en `0xc0000005` de façon déterministe et `IsSupported()` levait
//! `E_OUTOFMEMORY` avec 13 Go libres. C'est pourtant la seule voie qui donne
//! la capture hors-écran gratuitement — l'écarter sans re-test coûterait cher
//! au chantier D.
//!
//! Cette sonde tourne SEULE dans son processus : si le service replante, elle
//! emporte ce processus et aucun autre.

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use windows::core::Interface;
use windows::Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;

use crate::disposition;
use crate::geometry::Rect;
use crate::mire;

use super::mires::Mires;

pub(super) fn eprouver() -> Result<()> {
    // `IsSupported` d'abord, et journalisé même en cas de succès : c'est
    // l'appel qui levait `E_OUTOFMEMORY` au jalon 1.
    let supporte = match GraphicsCaptureSession::IsSupported() {
        Ok(valeur) => valeur,
        Err(erreur) => {
            tracing::error!(%erreur, "verdict WGC : ÉLIMINÉE — IsSupported a échoué");
            return Ok(());
        }
    };
    tracing::info!(supporte, "GraphicsCaptureSession::IsSupported");
    if !supporte {
        tracing::error!("verdict WGC : ÉLIMINÉE — l'API se déclare non supportée");
        return Ok(());
    }

    // Deux mires côte à côte : celle du dessous est la fenêtre observée, celle
    // du dessus viendra la recouvrir. C'est le seul test qui distingue WGC
    // d'un recadrage de bureau.
    let capture = crate::capture::DesktopCapture::new()?;
    let (largeur, hauteur) = capture.desktop_size();
    let places = disposition::tuiles(
        Rect { x: 0, y: 0, width: largeur, height: hauteur },
        2,
    )
    .context("deux places sur ce bureau")?;
    let mut mires = Mires::ouvrir(capture.device(), &places)?;
    mires.peindre()?;
    mires.pomper();

    let dxgi: IDXGIDevice = capture.device().cast().context("IDXGIDevice")?;
    let winrt = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi) }
        .context("périphérique WinRT depuis le périphérique DXGI")?;
    let winrt: IDirect3DDevice = winrt.cast().context("IDirect3DDevice")?;

    let interop = windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
        .context("fabrique d'interop GraphicsCaptureItem")?;
    let item: GraphicsCaptureItem = unsafe { interop.CreateForWindow(mires.hwnd(0)?) }
        .context("CreateForWindow sur la mire observée")?;
    tracing::info!("CreateForWindow a réussi — le service de capture a répondu");

    let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
        &winrt,
        DirectXPixelFormat::B8G8R8A8UIntNormalized,
        2,
        item.Size()?,
    )
    .context("création du pool de trames")?;
    let session = pool.CreateCaptureSession(&item).context("session de capture")?;
    session.StartCapture().context("démarrage de la capture")?;

    // Recouvrement : la mire 1 passe par-dessus la mire 0. WGC doit continuer
    // de rendre la mire 0 — c'est toute la question.
    mires.recouvrir(1, 0)?;

    let mut recues = 0usize;
    let mut dernier_verdict = mire::Verdict::Inconnue;
    let echeance = Instant::now() + Duration::from_secs(8);
    while Instant::now() < echeance {
        mires.peindre()?;
        mires.pomper();
        if let Ok(trame) = pool.TryGetNextFrame() {
            let surface = trame.Surface()?;
            let acces: IDirect3DDxgiInterfaceAccess = surface.cast()?;
            let texture: ID3D11Texture2D = unsafe { acces.GetInterface() }?;
            let taille = trame.ContentSize()?;
            let (r, g, b, _a) = crate::diagnostics::pixels::read_pixel(
                capture.device(),
                &texture,
                taille.Width as u32,
                taille.Height as u32,
                taille.Width as u32 / 2,
                taille.Height as u32 / 2,
            )?;
            dernier_verdict = mire::verdict(0, (r, g, b));
            recues += 1;
        }
        std::thread::sleep(Duration::from_millis(8));
    }

    tracing::info!(recues, ?dernier_verdict, "trames WGC reçues sous recouvrement");
    match (recues > 0, dernier_verdict) {
        (true, mire::Verdict::Juste) => tracing::info!(
            "verdict WGC : VIABLE — la fenêtre recouverte reste capturée correctement"
        ),
        (true, autre) => tracing::error!(
            ?autre,
            "verdict WGC : ÉLIMINÉE — des trames arrivent mais pas le bon contenu"
        ),
        (false, _) => tracing::error!("verdict WGC : ÉLIMINÉE — aucune trame en 8 s"),
    }
    Ok(())
}
```

- [ ] **Step 3 : Rendre `read_pixel` visible depuis le chantier**

`pixels::read_pixel` est `pub(super)` dans `diagnostics/pixels.rs`, donc visible de `diagnostics` mais pas de `diagnostics::multifenetre::wgc`. Élargir sa visibilité :

```rust
pub(crate) fn read_pixel(
```

et, dans `agent/src/diagnostics.rs`, remplacer `mod pixels;` par `pub(crate) mod pixels;`.

- [ ] **Step 4 : Aiguiller et transmettre**

Dans `multifenetre.rs`, ajouter `pub(super) mod wgc;` et, dans `aiguiller()`, avant le `Ok(false)` :

```rust
    if std::env::var("MULTIFENETRE_WGC").is_ok() {
        wgc::eprouver()?;
        return Ok(true);
    }
```

Dans `scripts/run-agent.sh` :

```bash
${MULTIFENETRE_WGC:+\$env:MULTIFENETRE_WGC = '$MULTIFENETRE_WGC'}
```

Dans `scripts/sonde-multifenetre.sh`, après la ligne `executer dxgi …` :

```bash
executer wgc MULTIFENETRE_WGC=1
```

- [ ] **Step 5 : Exécuter et lire**

```bash
set -a && source .env && set +a
scripts/build-agent.sh
SONDE_SECS=30 scripts/sonde-multifenetre.sh
```

Expected: l'un des trois verdicts journalisés. Si le processus disparaît sans journal, c'est le plantage de `captureservice` — le noter tel quel : c'est un résultat, pas une panne du banc.

**Si l'échec vient d'un composant absent** : relever, sans installer —

```bash
node scripts/winrm.js "Get-WindowsFeature | Where-Object { \$_.Name -like '*App*Compat*' -or \$_.Name -like '*Desktop*Experience*' } | Format-List Name,InstallState | Out-String"
```

Consigner l'état et **s'arrêter là** : l'installation implique un redémarrage de la VM, donc une décision humaine.

- [ ] **Step 6 : Commit**

```bash
git add agent/Cargo.toml agent/src/diagnostics.rs agent/src/diagnostics/pixels.rs agent/src/diagnostics/multifenetre.rs agent/src/diagnostics/multifenetre/wgc.rs scripts/run-agent.sh scripts/sonde-multifenetre.sh
git commit -m "feat(sonde): re-test de WGC, verdict rendu sur une fenêtre recouverte"
```

---

### Task 7 : Temps 1 — ce que la configuration d'affichage permet (voies 2 et 3)

**Files:**
- Modify: `agent/src/diagnostics/multifenetre/disponibilite.rs`, `scripts/sonde-multifenetre.sh`
- Create: aucun code neuf attendu — cette tâche est une **investigation**, dont le livrable est un relevé.

**Interfaces:**
- Consumes: `capture::enumerer_sorties` (Task 3), `disposition::tuiles` (Task 2), `disponibilite::relever_dxgi` (Task 5).
- Produces: un relevé consigné, couvrant **deux voies** qui dépendent toutes deux des capacités du pilote d'affichage : la voie 2 (une sortie par fenêtre) et la voie 3 (tuilage disjoint sur un bureau agrandi).

**Pourquoi cette tâche n'a pas de code écrit d'avance** : le mécanisme de configuration du « SudoMaker Virtual Display Adapter » n'est pas documenté dans le dépôt. Les étapes ci-dessous sont des relevés à exécuter dans l'ordre ; le code éventuel se décide après.

- [ ] **Step 1 : Identifier le pilote et son mode de configuration**

```bash
set -a && source .env && set +a
node scripts/winrm.js "Get-PnpDevice -Class Display | Format-List FriendlyName,InstanceId,Status,Class | Out-String"
node scripts/winrm.js "Get-PnpDevice -Class Display | ForEach-Object { \$_.InstanceId } | ForEach-Object { 'HKLM:\SYSTEM\CurrentControlSet\Enum\' + \$_ } | ForEach-Object { Get-ItemProperty \$_ -ErrorAction SilentlyContinue | Select-Object FriendlyName,Service,Driver } | Format-List | Out-String"
```

Attendu : le nom du service pilote du SudoMaker. Les pilotes d'affichage indirects (IddCx) exposent en général leur nombre de moniteurs dans leur clé de service, sous `HKLM\SYSTEM\CurrentControlSet\Services\<service>`.

- [ ] **Step 2 : Lire les paramètres du service**

```bash
node scripts/winrm.js "Get-ChildItem 'HKLM:\SYSTEM\CurrentControlSet\Services' | Where-Object { \$_.PSChildName -like '*Sudo*' -or \$_.PSChildName -like '*Idd*' -or \$_.PSChildName -like '*Virtual*Display*' } | Format-List PSChildName | Out-String"
```

Puis, pour le service trouvé, lister ses valeurs :

```bash
node scripts/winrm.js "Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Services\<SERVICE>' | Format-List | Out-String"
```

Chercher une valeur ressemblant à un nombre de moniteurs.

- [ ] **Step 3 : Tenter d'obtenir une sortie supplémentaire**

Si une valeur de configuration existe, l'augmenter d'une unité, redémarrer le périphérique, puis **re-relever** :

```bash
node scripts/winrm.js "Get-PnpDevice -InstanceId '<INSTANCE>' | Disable-PnpDevice -Confirm:\$false; Start-Sleep 3; Get-PnpDevice -InstanceId '<INSTANCE>' | Enable-PnpDevice -Confirm:\$false"
scripts/run-agent.sh   # avec MULTIFENETRE_DXGI=1
```

**Toute modification est réversible et doit être remise en état** en fin de tâche, quelle que soit l'issue : la VM sert à d'autres travaux.

- [ ] **Step 4 : Rendre le verdict de la voie 2**

Consigner en trois lignes : mécanisme trouvé (ou absence), nombre maximal de sorties atteint, résolutions disponibles. Verdicts possibles :

- **VIABLE** — 8 sorties obtenues → la voie passe au temps 2 (Task 9, voie `sortie-dediee`).
- **CONDITIONNELLE** — quelques sorties seulement, ou un pilote tiers serait nécessaire → noter le plafond atteint.
- **ÉLIMINÉE** — aucune sortie supplémentaire possible.

- [ ] **Step 5 : Relever la surface disponible (voie 3)**

La voie « tuilage disjoint » ne se juge pas sur le recouvrement, qu'elle interdit par construction, mais sur la surface qu'elle peut offrir à 8 fenêtres. Relever les modes d'affichage réellement disponibles :

```bash
node scripts/winrm.js "Get-CimInstance -ClassName CIM_VideoControllerResolution | Select-Object HorizontalResolution,VerticalResolution,RefreshRate | Sort-Object HorizontalResolution -Descending | Select-Object -First 15 | Format-Table | Out-String"
```

Puis, pour le mode le plus large offert, calculer les places à 8 fenêtres. Le calcul est celui de `disposition::tuiles` : grille la plus carrée possible (3×3 pour 8), place = `(largeur/3) & !1` par `(hauteur/3) & !1`. Sur le bureau actuel (2400×1080) cela donne **800×360**.

- [ ] **Step 6 : Rendre le verdict de la voie 3**

Trois éléments à consigner, dans cet ordre :

1. **Surface** — dimension de place obtenue à 8 fenêtres sur le meilleur mode disponible. En deçà de 1280×720, noter que la voie ne permet pas une fenêtre de jeu à résolution utile.
2. **Ce qui échappe à la disposition** — vérifier au moins un cas : ouvrir un menu déroulant dans une fenêtre placée en bord de tuile (le menu du Bloc-notes suffit) et constater s'il déborde sur la tuile voisine. Un débordement pollue la capture du voisin, et l'agent ne peut pas l'empêcher.
3. **Verdict** — VIABLE, CONDITIONNELLE (avec la limite de surface chiffrée), ou ÉLIMINÉE.

- [ ] **Step 7 : Commit**

Si les étapes ci-dessus ont produit du code (par exemple une aide de relevé dans `disponibilite.rs`) :

```bash
git add agent/src/diagnostics/multifenetre/disponibilite.rs scripts/sonde-multifenetre.sh
git commit -m "feat(sonde): relever la capacité du pilote d'affichage virtuel en sorties"
```

Sinon, aucun commit : le relevé rejoint le document de résultats (Task 11).

---

### Task 8 : Temps 1 — les replis par fenêtre

**Files:**
- Create: `agent/src/diagnostics/multifenetre/replis.rs`
- Modify: `agent/src/diagnostics/multifenetre.rs`, `scripts/run-agent.sh`, `scripts/sonde-multifenetre.sh`

**Interfaces:**
- Consumes: `mires::Mires`, `mire::verdict`.
- Produces: `replis::eprouver() -> Result<()>`, activée par `MULTIFENETRE_REPLIS=1`.

- [ ] **Step 1 : Écrire la sonde**

Créer `agent/src/diagnostics/multifenetre/replis.rs` :

```rust
//! Voie 4 : les replis par fenêtre, sondés en dernier parce qu'ils sont les
//! moins prometteurs.
//!
//! `PrintWindow(PW_RENDERFULLCONTENT)` passe par GDI et rend typiquement du
//! noir sur une fenêtre D3D — c'est précisément ce que cette sonde vérifie,
//! sur une mire peinte en D3D11 et non en GDI. Il rapatrie de surcroît les
//! pixels en mémoire centrale : même correct, il tomberait sur la porte
//! « chemin GPU » de la spec §5. On le sonde pour le CONSIGNER, pas dans
//! l'espoir de le retenir.
//!
//! `DwmGetDxSharedSurface` n'est pas documentée : elle est résolue
//! dynamiquement dans user32.dll, et son absence est un résultat, pas une
//! erreur.

use anyhow::{Context, Result};
use windows::core::s;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetPixel,
    ReleaseDC, SelectObject,
};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};
use windows::Win32::UI::WindowsAndMessaging::{PrintWindow, PRINT_WINDOW_FLAGS};

use crate::disposition;
use crate::geometry::Rect;
use crate::mire;

use super::mires::Mires;

/// Non exposée par `windows` 0.62 : `PW_RENDERFULLCONTENT` vaut 2 (WinUser.h).
const PW_RENDERFULLCONTENT: PRINT_WINDOW_FLAGS = PRINT_WINDOW_FLAGS(2);

pub(super) fn eprouver() -> Result<()> {
    let capture = crate::capture::DesktopCapture::new()?;
    let (largeur, hauteur) = capture.desktop_size();
    let places =
        disposition::tuiles(Rect { x: 0, y: 0, width: largeur, height: hauteur }, 2)
            .context("deux places sur ce bureau")?;
    let mut mires = Mires::ouvrir(capture.device(), &places)?;
    mires.peindre()?;
    mires.pomper();
    mires.recouvrir(1, 0)?;
    mires.peindre()?;
    mires.pomper();

    eprouver_printwindow(&mires)?;
    eprouver_surface_dwm();
    Ok(())
}

fn eprouver_printwindow(mires: &Mires) -> Result<()> {
    let hwnd = mires.hwnd(0)?;
    let place = mires.place(0)?;
    let ecran = unsafe { GetDC(None) };
    let memoire = unsafe { CreateCompatibleDC(Some(ecran)) };
    let bitmap = unsafe { CreateCompatibleBitmap(ecran, place.width as i32, place.height as i32) };
    let ancien = unsafe { SelectObject(memoire, bitmap.into()) };

    let rendu = unsafe { PrintWindow(hwnd, memoire, PW_RENDERFULLCONTENT) }.as_bool();
    // `GetPixel` rend un COLORREF 0x00BBGGRR.
    let couleur = unsafe { GetPixel(memoire, place.width as i32 / 2, place.height as i32 / 2) };
    let brut = couleur.0;
    let pixel = ((brut & 0xFF) as u8, ((brut >> 8) & 0xFF) as u8, ((brut >> 16) & 0xFF) as u8);

    unsafe {
        SelectObject(memoire, ancien);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(memoire);
        ReleaseDC(None, ecran);
    }

    let verdict = mire::verdict(0, pixel);
    tracing::info!(rendu, ?pixel, ?verdict, "PrintWindow(PW_RENDERFULLCONTENT) sur une mire D3D");
    match verdict {
        mire::Verdict::Juste => tracing::info!(
            "verdict PrintWindow : CONDITIONNELLE — image correcte, mais chemin CPU \
             (porte « chemin GPU » de la spec §5)"
        ),
        autre => tracing::error!(?autre, "verdict PrintWindow : ÉLIMINÉE"),
    }
    Ok(())
}

fn eprouver_surface_dwm() {
    // Résolution dynamique : la fonction n'est pas documentée et peut être
    // absente. Son absence est un résultat.
    let module = match unsafe { LoadLibraryA(s!("user32.dll")) } {
        Ok(module) => module,
        Err(erreur) => {
            tracing::error!(%erreur, "verdict DwmGetDxSharedSurface : ÉLIMINÉE — user32 introuvable");
            return;
        }
    };
    let adresse = unsafe { GetProcAddress(module, s!("DwmGetDxSharedSurface")) };
    match adresse {
        Some(_) => tracing::info!(
            "DwmGetDxSharedSurface est exportée par user32 — voie CONDITIONNELLE, \
             à instrumenter seulement si les voies 1 à 3 tombent toutes"
        ),
        None => tracing::error!(
            "verdict DwmGetDxSharedSurface : ÉLIMINÉE — absente de user32 sur ce build"
        ),
    }
}
```

La sonde s'arrête à la **présence** du symbole : appeler une fonction non documentée, en déduire la disposition de ses six paramètres de sortie et ouvrir la surface partagée coûterait une demi-journée pour une voie qui n'est qu'un filet. Si les voies 1 à 3 tombent toutes, cette tâche se rouvre avec un plan à elle.

- [ ] **Step 2 : Aiguiller et transmettre**

Dans `multifenetre.rs` : `pub(super) mod replis;` et

```rust
    if std::env::var("MULTIFENETRE_REPLIS").is_ok() {
        replis::eprouver()?;
        return Ok(true);
    }
```

Dans `scripts/run-agent.sh` : `${MULTIFENETRE_REPLIS:+\$env:MULTIFENETRE_REPLIS = '$MULTIFENETRE_REPLIS'}`

Dans `scripts/sonde-multifenetre.sh` : `executer replis MULTIFENETRE_REPLIS=1`

- [ ] **Step 3 : Exécuter et lire**

```bash
set -a && source .env && set +a
scripts/build-agent.sh
scripts/sonde-multifenetre.sh
```

Expected: un verdict pour `PrintWindow` (attendu : `Noire`, donc ÉLIMINÉE) et un pour `DwmGetDxSharedSurface`.

- [ ] **Step 4 : Commit**

```bash
git add agent/src/diagnostics/multifenetre.rs agent/src/diagnostics/multifenetre/replis.rs scripts/run-agent.sh scripts/sonde-multifenetre.sh
git commit -m "feat(sonde): éprouver les replis par fenêtre sur une mire D3D"
```

---

### Task 9 : Temps 2 — le banc

**Files:**
- Create: `agent/src/diagnostics/multifenetre/voies.rs`, `agent/src/diagnostics/multifenetre/banc.rs`
- Modify: `agent/src/diagnostics/multifenetre.rs`, `scripts/run-agent.sh`, `scripts/sonde-multifenetre.sh`

**Interfaces:**
- Consumes: `mires::Mires`, `mire::verdict`, `capture::DesktopCapture`, `capture::CapturedFrame`, `encode::H264Encoder`, `disposition::tuiles`.
- Produces: `voies::VoieDeCapture` (trait), `voies::VoieDuplication`, `banc::executer(voie: &str, n: u8) -> Result<()>`, activé par `MULTIFENETRE_BANC=<voie>` et `MULTIFENETRE_N=<1..8>`.

- [ ] **Step 1 : Écrire le trait et la voie « duplication recadrée »**

Créer `agent/src/diagnostics/multifenetre/voies.rs` :

```rust
//! Le trait que le banc mesure, et la voie de référence.
//!
//! Le banc est écrit UNE FOIS et exercé sur chaque voie : sans ce trait, on
//! l'écrirait une fois par voie et l'on ne comparerait plus les mêmes choses.
//! C'est aussi la couture dont le chantier D aura besoin pour rendre la
//! capture substituable.

use std::cell::RefCell;
use std::rc::Rc;

use anyhow::Result;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D11::ID3D11Device;

use crate::capture::{CapturedFrame, DesktopCapture};
use crate::geometry::Rect;

pub(super) trait VoieDeCapture {
    fn nom(&self) -> &'static str;
    /// Ouvre un flux sur une fenêtre. `region` est sa place à l'écran, dont
    /// les voies par recadrage ont besoin et que les voies par fenêtre
    /// ignorent.
    fn ouvrir(&mut self, hwnd: HWND, region: Rect) -> Result<()>;
    /// Rend l'image suivante, ou `None` si aucune n'est disponible.
    fn prochaine_image(&mut self) -> Result<Option<CapturedFrame>>;
    /// Périphérique D3D11 propriétaire des textures rendues par cette voie.
    /// Le banc en a besoin pour lire un pixel et pour créer l'encodeur : une
    /// texture ne se lit pas depuis un autre périphérique que le sien.
    fn device(&self) -> ID3D11Device;
}

/// Voie de référence : Desktop Duplication du bureau, recadrée sur la fenêtre.
/// C'est le comportement de production actuel — celui dont on sait déjà qu'il
/// se pollue au recouvrement. Il sert d'étalon : une voie qui ne fait pas
/// mieux que lui n'apporte rien, et s'il ne se polluait PAS au banc, ce
/// serait le banc qu'il faudrait suspecter.
///
/// **Une seule duplication pour toutes les fenêtres.** DXGI n'accorde qu'un
/// nombre très limité de duplications concurrentes d'une même sortie ;
/// en ouvrir une par fenêtre échouerait dès la deuxième. C'est d'ailleurs
/// fidèle à la production : une duplication, N recadrages.
pub(super) struct VoieDuplication {
    capture: Rc<RefCell<DesktopCapture>>,
    region: Rect,
}

impl VoieDuplication {
    /// Crée la duplication partagée, une fois pour tout le banc.
    pub(super) fn partagee() -> Result<Rc<RefCell<DesktopCapture>>> {
        Ok(Rc::new(RefCell::new(DesktopCapture::new()?)))
    }

    pub(super) fn nouvelle(capture: Rc<RefCell<DesktopCapture>>) -> Self {
        Self { capture, region: Rect { x: 0, y: 0, width: 0, height: 0 } }
    }
}

impl VoieDeCapture for VoieDuplication {
    fn nom(&self) -> &'static str {
        "duplication"
    }

    fn ouvrir(&mut self, _hwnd: HWND, region: Rect) -> Result<()> {
        self.region = region;
        Ok(())
    }

    fn prochaine_image(&mut self) -> Result<Option<CapturedFrame>> {
        self.capture.borrow_mut().next_frame(self.region)
    }

    fn device(&self) -> ID3D11Device {
        // `ID3D11Device` est un pointeur COM à comptage de références : le
        // cloner ne duplique pas le périphérique, il incrémente un compteur.
        // Rendre une valeur plutôt qu'une référence évite au banc de tenir un
        // emprunt sur la voie pendant qu'il l'appelle.
        self.capture.borrow().device().clone()
    }
}
```

- [ ] **Step 2 : Écrire le banc**

Créer `agent/src/diagnostics/multifenetre/banc.rs` :

```rust
//! Temps 2 : trois passes, de 1 à N fenêtres.
//!
//! 1. TÉMOIN — les mires peignent, rien ne capture. Sans cette passe, un
//!    décrochage à six fenêtres serait indiscernable d'un décrochage de la
//!    mire elle-même : huit swapchains à 60 Hz consomment du GPU, et cette
//!    charge entrerait sinon dans la mesure. Même rôle que le profil `lan`
//!    du banc `netem`.
//! 2. CAPTURE — les mires peignent et la voie capture.
//! 3. CAPTURE + ENCODAGE — un encodeur H.264 par fenêtre.
//!
//! À mi-parcours des passes 2 et 3, une mire vient en recouvrir une autre :
//! la fenêtre recouverte doit continuer de rendre sa mire. Un échec ici
//! élimine la voie SANS mesure de cadence — chiffrer la vitesse d'une image
//! fausse n'apprend rien.
//!
//! Aucune trace par trame : compteurs agrégés, journalisés à la seconde. Au
//! chantier NAT, une trace par paquet écrite sur le partage CIFS a détruit la
//! session qu'elle mesurait.

use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use crate::disposition;
use crate::geometry::Rect;
use crate::mire;

use super::mires::Mires;
use super::voies::{VoieDeCapture, VoieDuplication};

/// Durée de chaque passe.
const DUREE_PASSE: Duration = Duration::from_secs(10);
/// Cadence de journalisation des compteurs.
const PERIODE_JOURNAL: Duration = Duration::from_secs(1);

struct Compteurs {
    images: Vec<u64>,
    unites: Vec<u64>,
    verdicts_faux: u64,
}

pub(super) fn executer(nom_voie: &str, nombre: u8) -> Result<()> {
    anyhow::ensure!(
        nombre >= 1 && nombre <= mire::MIRES_MAX,
        "MULTIFENETRE_N doit valoir 1 à {}",
        mire::MIRES_MAX
    );

    let capture = crate::capture::DesktopCapture::new()?;
    let (largeur, hauteur) = capture.desktop_size();
    let places = disposition::tuiles(
        Rect { x: 0, y: 0, width: largeur, height: hauteur },
        nombre as u32,
    )
    .with_context(|| format!("{nombre} places sur un bureau {largeur}x{hauteur}"))?;
    tracing::info!(voie = nom_voie, nombre, ?places, "banc : disposition retenue");

    let mut mires = Mires::ouvrir(capture.device(), &places)?;

    passe_temoin(&mut mires)?;
    let mut voies = ouvrir_voies(nom_voie, nombre, &mires, &places)?;
    let compteurs = passe_capture(&mut mires, &mut voies, false)?;
    journaliser("capture", nom_voie, nombre, &compteurs);
    if compteurs.verdicts_faux > 0 {
        tracing::error!(
            voie = nom_voie,
            verdicts_faux = compteurs.verdicts_faux,
            "verdict : ÉLIMINÉE sous recouvrement — la passe d'encodage est sautée"
        );
        return Ok(());
    }
    let compteurs = passe_capture(&mut mires, &mut voies, true)?;
    journaliser("capture+encodage", nom_voie, nombre, &compteurs);
    Ok(())
}

fn ouvrir_voies(
    nom_voie: &str,
    nombre: u8,
    mires: &Mires,
    places: &[Rect],
) -> Result<Vec<Box<dyn VoieDeCapture>>> {
    // Ce que la voie partage entre ses N flux est décidé ICI, une fois : la
    // duplication n'accepte pas d'être ouverte N fois sur la même sortie.
    let partagee = match nom_voie {
        "duplication" => super::voies::VoieDuplication::partagee()?,
        autre => {
            anyhow::bail!("voie « {autre} » inconnue du banc — voies câblées : duplication")
        }
    };

    let mut voies: Vec<Box<dyn VoieDeCapture>> = Vec::new();
    for id in 0..nombre {
        let mut voie: Box<dyn VoieDeCapture> =
            Box::new(VoieDuplication::nouvelle(partagee.clone()));
        voie.ouvrir(mires.hwnd(id)?, places[id as usize])?;
        voies.push(voie);
    }
    Ok(voies)
}

/// Passe témoin : les mires peignent, rien ne capture.
fn passe_temoin(mires: &mut Mires) -> Result<()> {
    let debut = Instant::now();
    let mut trames = 0u64;
    while debut.elapsed() < DUREE_PASSE {
        mires.peindre()?;
        mires.pomper();
        trames += 1;
    }
    let secondes = debut.elapsed().as_secs_f64();
    tracing::info!(
        mires = mires.nombre(),
        trames,
        cadence = trames as f64 / secondes,
        "passe TÉMOIN — cadence de peinture sans capture"
    );
    Ok(())
}

fn passe_capture(
    mires: &mut Mires,
    voies: &mut [Box<dyn VoieDeCapture>],
    avec_encodage: bool,
) -> Result<Compteurs> {
    let nombre = voies.len();
    let mut compteurs = Compteurs {
        images: vec![0; nombre],
        unites: vec![0; nombre],
        verdicts_faux: 0,
    };
    let mut encodeurs: Vec<crate::encode::H264Encoder> = Vec::new();
    if avec_encodage {
        for id in 0..nombre {
            let place = mires.place(id as u8)?;
            // Un encodeur par fenêtre, sur le périphérique de SA voie : une
            // texture ne se soumet pas à un encodeur bâti sur un autre
            // périphérique D3D11.
            let appareil = voies[id].device();
            encodeurs.push(crate::encode::H264Encoder::new(
                &appareil,
                (place.width, place.height),
                (place.width, place.height),
                60,
                8_000_000,
            )?);
        }
    }

    let debut = Instant::now();
    let mi_parcours = debut + DUREE_PASSE / 2;
    let mut recouvert = false;
    let mut prochain_journal = debut + PERIODE_JOURNAL;
    let mut pts = vec![0u64; nombre];

    while debut.elapsed() < DUREE_PASSE {
        mires.peindre()?;
        mires.pomper();

        // Mise en scène de la porte éliminatoire : la dernière mire vient
        // recouvrir la première.
        if !recouvert && Instant::now() >= mi_parcours && nombre >= 2 {
            mires.recouvrir(nombre as u8 - 1, 0)?;
            recouvert = true;
            tracing::info!("recouvrement posé : la mire 0 est sous la mire {}", nombre - 1);
        }

        for (id, voie) in voies.iter_mut().enumerate() {
            let Some(image) = voie.prochaine_image()? else {
                continue;
            };
            compteurs.images[id] += 1;

            // La vérification ne porte QUE sur la fenêtre recouverte, et
            // seulement une fois le recouvrement posé : ailleurs elle
            // n'apprendrait rien et coûterait une copie CPU par image.
            if recouvert && id == 0 {
                let appareil = voie.device();
                let (r, g, b, _a) = crate::diagnostics::pixels::read_pixel(
                    &appareil,
                    &image.texture,
                    image.width,
                    image.height,
                    image.width / 2,
                    image.height / 2,
                )?;
                if mire::verdict(0, (r, g, b)) != mire::Verdict::Juste {
                    compteurs.verdicts_faux += 1;
                }
            }

            if avec_encodage {
                encodeurs[id].submit(&image, pts[id])?;
                pts[id] += 90_000 / 60;
                while let Some(_unite) = encodeurs[id].poll_output()? {
                    compteurs.unites[id] += 1;
                }
            }
        }

        if Instant::now() >= prochain_journal {
            tracing::info!(
                images = ?compteurs.images,
                unites = ?compteurs.unites,
                verdicts_faux = compteurs.verdicts_faux,
                "banc en cours"
            );
            prochain_journal += PERIODE_JOURNAL;
        }
    }
    Ok(compteurs)
}

fn journaliser(passe: &str, voie: &str, nombre: u8, compteurs: &Compteurs) {
    let secondes = DUREE_PASSE.as_secs_f64();
    let cadences: Vec<f64> = compteurs.images.iter().map(|n| *n as f64 / secondes).collect();
    tracing::info!(
        passe,
        voie,
        nombre,
        ?cadences,
        unites = ?compteurs.unites,
        verdicts_faux = compteurs.verdicts_faux,
        "passe terminée"
    );
}
```

- [ ] **Step 3 : Prévoir le câblage des voies survivantes**

Le banc ne connaît que `duplication` — l'étalon, disponible quoi qu'il arrive. Chaque voie déclarée VIABLE au temps 1 s'ajoute **ensuite**, en deux endroits seulement :

1. une implémentation de `VoieDeCapture` dans son propre fichier (`wgc.rs` en a déjà la matière : `eprouver` fait tout le travail d'ouverture, il reste à le ranger derrière le trait ; la voie « une sortie par fenêtre » appelle `DesktopCapture::sur_sortie` de la Task 3) ;
2. une branche dans le `match nom_voie` d'`ouvrir_voies`.

Ne rien câbler d'avance : le temps 1 peut éliminer les trois autres voies, et ce serait du code écrit pour rien. Consigner cette étape dans le document de résultats si elle est faite, avec son verdict.

- [ ] **Step 4 : Aiguiller et transmettre**

Dans `multifenetre.rs` : `pub(super) mod banc; pub(super) mod voies;` et

```rust
    if let Ok(voie) = std::env::var("MULTIFENETRE_BANC") {
        let nombre: u8 = std::env::var("MULTIFENETRE_N")
            .unwrap_or_else(|_| "8".to_string())
            .parse()
            .context("MULTIFENETRE_N doit être un entier")?;
        banc::executer(&voie, nombre)?;
        return Ok(true);
    }
```

Dans `scripts/run-agent.sh` :

```bash
${MULTIFENETRE_BANC:+\$env:MULTIFENETRE_BANC = '$MULTIFENETRE_BANC'}
${MULTIFENETRE_N:+\$env:MULTIFENETRE_N = '$MULTIFENETRE_N'}
```

Dans `scripts/sonde-multifenetre.sh`, ajouter une boucle en fin de fichier :

```bash
# Temps 2 : le banc, sur les voies déclarées survivantes par le temps 1.
# `VOIES` est posée à la main d'après les verdicts — le script n'infère rien.
for voie in ${VOIES:-}; do
    for n in 1 2 4 8; do
        SONDE_SECS=45 executer "banc-$voie-$n" "MULTIFENETRE_BANC=$voie" "MULTIFENETRE_N=$n"
    done
done
```

- [ ] **Step 5 : Exécuter la voie de référence**

```bash
set -a && source .env && set +a
scripts/build-agent.sh
VOIES=duplication scripts/sonde-multifenetre.sh
```

Expected: à N ≥ 2, `verdicts_faux > 0` sur la voie `duplication` — c'est le **résultat attendu**, et il valide le banc : la voie de production se pollue bien au recouvrement, comme la spec l'affirme. Si `verdicts_faux` restait à 0 à N ≥ 2, c'est le banc qu'il faut suspecter, pas la capture.

- [ ] **Step 6 : Commit**

```bash
git add agent/src/diagnostics/multifenetre.rs agent/src/diagnostics/multifenetre/voies.rs agent/src/diagnostics/multifenetre/banc.rs scripts/run-agent.sh scripts/sonde-multifenetre.sh
git commit -m "feat(sonde): banc à trois passes, la voie de production servant d'étalon"
```

---

### Task 10 : Le plafond NVENC

**Files:**
- Create: `agent/src/diagnostics/multifenetre/nvenc.rs`
- Modify: `agent/src/diagnostics/multifenetre.rs`, `scripts/run-agent.sh`, `scripts/sonde-multifenetre.sh`

**Interfaces:**
- Consumes: `capture::DesktopCapture`, `encode::H264Encoder`.
- Produces: `nvenc::plafond() -> Result<()>`, activée par `MULTIFENETRE_NVENC=1`.

- [ ] **Step 1 : Écrire la sonde**

Créer `agent/src/diagnostics/multifenetre/nvenc.rs` :

```rust
//! Combien d'encodeurs H.264 matériels cette RTX 4070 accepte-t-elle en
//! parallèle ?
//!
//! L'ordre de grandeur admis (8 sessions sur Ada) est une rumeur de
//! spécification, pas une mesure sur cette carte et ce pilote. Le chantier D
//! en dépend directement : si le plafond est bas, suspendre l'encodage des
//! fenêtres masquées cesse d'être « souhaitable en soi » pour devenir une
//! condition de viabilité.

use anyhow::Result;

/// Au-delà, on cesse de chercher : le résultat serait déjà largement
/// suffisant pour le chantier D.
const PLAFOND_RECHERCHE: usize = 16;

pub(super) fn plafond() -> Result<()> {
    let capture = crate::capture::DesktopCapture::new()?;
    let mut encodeurs = Vec::new();
    for rang in 1..=PLAFOND_RECHERCHE {
        match crate::encode::H264Encoder::new(capture.device(), (1280, 720), (1280, 720), 60, 8_000_000)
        {
            Ok(encodeur) => {
                encodeurs.push(encodeur);
                tracing::info!(rang, "encodeur créé");
            }
            Err(erreur) => {
                tracing::info!(
                    plafond = rang - 1,
                    %erreur,
                    "plafond NVENC atteint — création du suivant refusée"
                );
                return Ok(());
            }
        }
    }
    tracing::info!(
        plafond_recherche = PLAFOND_RECHERCHE,
        "aucun plafond atteint sous {PLAFOND_RECHERCHE} encodeurs"
    );
    Ok(())
}
```

- [ ] **Step 2 : Aiguiller et transmettre**

Dans `multifenetre.rs` : `pub(super) mod nvenc;` et

```rust
    if std::env::var("MULTIFENETRE_NVENC").is_ok() {
        nvenc::plafond()?;
        return Ok(true);
    }
```

Dans `scripts/run-agent.sh` : `${MULTIFENETRE_NVENC:+\$env:MULTIFENETRE_NVENC = '$MULTIFENETRE_NVENC'}`

Dans `scripts/sonde-multifenetre.sh`, après la ligne `executer replis …` : `executer nvenc MULTIFENETRE_NVENC=1`

- [ ] **Step 3 : Exécuter et lire**

```bash
set -a && source .env && set +a
scripts/build-agent.sh
scripts/sonde-multifenetre.sh
```

Expected: une ligne `plafond NVENC atteint` avec un nombre, ou l'absence de plafond sous 16.

- [ ] **Step 4 : Commit**

```bash
git add agent/src/diagnostics/multifenetre.rs agent/src/diagnostics/multifenetre/nvenc.rs scripts/run-agent.sh scripts/sonde-multifenetre.sh
git commit -m "feat(sonde): mesurer le plafond d'encodeurs simultanés de cette carte"
```

---

### Task 11 : Clôture — résultats, amendement de la spec, CLAUDE.md

**Files:**
- Create: `docs/superpowers/plans/2026-07-30-sonde-capture-multifenetre-resultats.md`
- Modify: `docs/superpowers/specs/2026-07-28-support-jeux-design.md` (§4, décision ouverte), `CLAUDE.md`

- [ ] **Step 1 : Écrire le document de résultats**

Structure imposée — un lecteur doit pouvoir décider sans relire les journaux :

```markdown
# Sonde de capture multi-fenêtres — résultats

## Ce qui est acquis
## Ce qui reste ouvert
## Relevé DXGI (l'écart documentaire, tranché)
## Verdict par voie
| Voie | Verdict | Correction sous recouvrement | Chemin GPU | Cadence à N | Notes |
## Plafond NVENC mesuré
## Pièges rencontrés
## Recommandation pour le chantier D
```

Règle : chaque chiffre cité vient d'un journal joint dans
`docs/superpowers/plans/journaux-sonde-multifenetre/`. Aucun chiffre reconstitué de mémoire.

- [ ] **Step 2 : Trancher la décision ouverte de la spec**

Dans `docs/superpowers/specs/2026-07-28-support-jeux-design.md` §4, sous « Conséquence technique majeure — décision ouverte », remplacer l'énoncé des deux voies par la décision prise, en citant le document de résultats. Conserver l'historique : la voie écartée reste décrite, avec la raison mesurée de son écartement.

- [ ] **Step 3 : Consigner dans CLAUDE.md**

Ajouter une section « 🪟 Sonde de capture multi-fenêtres (30 juillet 2026) » sur le modèle des sections existantes : ce qui a été mesuré, la voie retenue, les pièges (au moins : la nécessité d'animer les mires, l'obligation de peindre en D3D11 et non en GDI, l'isolation par processus des voies).

- [ ] **Step 4 : Vérifier l'état du dépôt**

```bash
cargo test -p agent 2>&1 | tail -5
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
```

Expected: tous les tests passent ; aucun fichier neuf au-dessus de 500 lignes (seuls les trois fichiers de dette connus doivent apparaître).

- [ ] **Step 5 : Commit**

```bash
git add docs/ CLAUDE.md
git commit -m "docs(sonde): résultats de la sonde de capture, décision de capture tranchée"
```

---

## Ce que ce plan ne fait pas

- Aucun `SetWinEventHook`, aucune détection dynamique de fenêtres, aucun filtrage « Alt-Tab-able ».
- Aucune topologie N `RTCPeerConnection`, aucune répartition du débit entre flux.
- Aucun audio par processus.
- Aucun cycle de vie fenêtre Windows ↔ fenêtre navigateur, aucun plein écran Windows → navigateur.
- Aucune installation de composant Windows : la sonde constate, l'humain décide.

Tout cela relève du chantier D, qui sera spécifié **après** ces résultats.
