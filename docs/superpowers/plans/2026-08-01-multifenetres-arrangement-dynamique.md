# Sous-bloc D2 — arrangement dynamique multi-fenêtres : plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Qu'ouvrir une application de plus ne tue plus aucune session de capture en cours, et que la démonstration multi-fenêtres du sous-bloc D1 devienne rejouable jusqu'à cinq fenêtres.

**Architecture:** `DXGI_ERROR_ACCESS_LOST` cesse d'être une panne définitive. `DesktopCapture` retient de quoi se rouvrir, classe l'échec d'acquisition en trois branches au lieu de deux, et reconstruit **la seule** `IDXGIOutputDuplication` en conservant son périphérique D3D11 — la reprise est tentée dans `next_frame` elle-même, donc éprouvée par le banc autant que par la production. Autour de ce cœur : les sorties se désignent par leur nom `\\.\DISPLAYn` et non par un index positionnel, l'appariement d'une sortie fraîche devient tolérant et attend un fait plutôt qu'une durée, et la table du superviseur cesse d'oublier une fenêtre dont l'enfant est mort.

**Tech Stack:** Rust (`agent/`, windows-rs 0.62, `anyhow`, `tracing`), TypeScript (`client/`, vitest), pilote d'affichage virtuel SudoVDA piloté par IOCTL.

## Contraintes globales

Ces contraintes valent pour **toutes** les tâches. Elles ne sont pas répétées ensuite.

- **Spec de référence** : `docs/superpowers/specs/2026-08-01-multifenetres-arrangement-dynamique-design.md`. En cas de divergence entre ce plan et la spec, c'est un défaut du plan : le signaler, ne pas trancher seul.
- **Plafond de 500 lignes par fichier de code source** (`CLAUDE.md`). Aucun fichier neuf ne naît au-dessus ; un fichier déjà au-dessus ne grossit pas. `agent/src/windows_source.rs` (648 lignes) est en dette gelée : **toute addition y est interdite**, elle va dans un module enfant. Vérifier avec la commande de `CLAUDE.md`.
- **Tests sur l'hôte Linux** : `cargo test --manifest-path agent/Cargo.toml` (251 tests au départ de ce plan) et `cd client && npm test`. Tout code `#[cfg(windows)]` est hors de portée de ces suites : c'est pourquoi chaque tâche isole sa part pure.
- **Le code `#[cfg(windows)]` ne se compile QUE sur la VM** : `scripts/build-agent.sh`. Ne jamais déclarer compilable ce qui ne l'a pas été.
- **La VM n'est jamais supposée allumée.** `virsh list --all` avant toute séquence qui en dépend, `virsh start Windows`, puis attendre WinRM **puis** l'accès réel à `/media/vm` (`until ls /media/vm/dev`), pas seulement le port 5985.
- **Aucune trace par paquet ni par image.** Le dépôt a déjà payé deux fois pour cette faute (chantier TURN, correctif I2 de D1). Compter ou échantillonner.
- **Commits nominatifs** : `git add <chemins exacts>`, jamais `git add -A`. L'arbre est partagé.
- **Langue** : code, commentaires, messages de journal et messages de commit en français, sans accents dans les messages de commit (convention du dépôt).
- **Branche de travail** : `chantier-multifenetres-d2`, déjà créée, portant la spec au commit `881cbe1`.

---

## Structure des fichiers

**Créés :**

| Fichier | Responsabilité |
| --- | --- |
| `agent/src/capture/reprise.rs` | Classification d'un code d'erreur DXGI et budget de reprises. **Pur, testable sur l'hôte** — déclaré en module frère hors `#[cfg(windows)]`, comme `windows_source/sortie.rs`. |
| `agent/src/diagnostics/multifenetre/reprise.rs` | Le banc `MULTIFENETRE_REPRISE` : k duplications, création d'une sortie de plus, contrôle que les k reprennent. |
| `client/src/viewport.ts` | Arrondi pair du viewport annoncé. Pur, testable. |
| `client/src/viewport.test.ts` | Ses tests. |

**Modifiés :**

| Fichier | Nature du changement |
| --- | --- |
| `agent/src/capture.rs` (463 l.) | Retient sa cible, sait rouvrir, classe et reprend dans `next_frame`. Ouverture par nom de sortie. |
| `agent/src/main.rs` | Déclaration du module frère `capture_reprise` ; `SORTIE_DXGI` devient un nom. |
| `agent/src/windows_source.rs` (648 l., **dette gelée**) | **Uniquement** l'adaptation du `match` sur le nouveau type d'erreur, à coût de lignes nul ou négatif. |
| `agent/src/windows_source/sortie.rs` | `sur_sortie` prend un nom de sortie. |
| `agent/src/diagnostics/multifenetre/voies.rs` | Adaptation au nouveau type d'erreur. |
| `agent/src/diagnostics/multifenetre.rs` | Aiguillage de `MULTIFENETRE_REPRISE`. |
| `agent/src/superviseur/table.rs` (289 l.) | La sortie se désigne par son nom ; une fenêtre dont l'enfant meurt n'est plus oubliée. |
| `agent/src/superviseur/table/tests.rs` | Tests correspondants. |
| `agent/src/superviseur/boucle.rs` (423 l.) | Attente sur condition observable ; `prises` devient un ensemble de noms. |
| `agent/src/superviseur/placement.rs` (222 l.) | Appariement tolérant. |
| `agent/src/superviseur/enfants.rs` | `Consigne` porte le nom de sortie. |
| `agent/src/superviseur/lanceur.rs` | `SORTIE_DXGI` porte le nom. |
| `agent/src/input.rs` | Premier plan avant injection clavier. |
| `client/src/main.ts` | Emploie `viewport.ts`. |
| `scripts/` | Rien. Aucun script n'est modifié par ce plan. |

---

## Tâche 1 : Classification et budget de reprises — la part pure

**Files:**
- Create: `agent/src/capture/reprise.rs`
- Modify: `agent/src/main.rs` (bloc de déclarations de modules, l. 1-56)
- Test: dans `agent/src/capture/reprise.rs` (`#[cfg(test)] mod tests`, convention du dépôt)

**Interfaces:**
- Consumes: rien.
- Produces:
  - `pub const ACCES_PERDU: i32` = `0x887A0026u32 as i32` (`DXGI_ERROR_ACCESS_LOST`)
  - `pub const DEVICE_REMOVED: i32` = `0x887A0005u32 as i32`
  - `pub const ATTENTE_EXPIREE: i32` = `0x887A0027u32 as i32` (`DXGI_ERROR_WAIT_TIMEOUT`)
  - `pub const REPRISES_MAX: u32 = 3`
  - `pub fn est_acces_perdu(code: i32) -> bool`
  - `pub struct BudgetReprises` avec `pub fn nouveau() -> Self`, `pub fn consommer(&mut self) -> bool`, `pub fn succes(&mut self)`, `pub fn consommees(&self) -> u32`

- [ ] **Étape 1 : écrire le fichier avec ses tests, l'implémentation restant vide**

Créer `agent/src/capture/reprise.rs` :

```rust
//! Classer un échec d'acquisition DXGI, et borner les reprises.
//!
//! **Pur à dessein.** `capture.rs` est `#![cfg(windows)]` dans son ensemble :
//! un module ENFANT n'y serait pas compilable sur l'hôte Linux, donc pas
//! testable. Ce fichier est donc déclaré en module FRÈRE dans `main.rs`
//! (`#[path = "capture/reprise.rs"] mod capture_reprise;`), hors de tout
//! `cfg` — le même montage que `windows_source/sortie.rs`, et pour la même
//! raison.
//!
//! Il ne connaît que des `i32` : les codes DXGI nus. Aucun type `windows-rs`
//! ne franchit cette frontière, sans quoi elle ne tiendrait pas.

/// `DXGI_ERROR_ACCESS_LOST`. DXGI révoque l'accès à une duplication quand la
/// topologie d'affichage change — et **la création d'une sortie virtuelle en
/// est un cas**, relevé par le sous-bloc D1 sur trois exécutions sur trois.
/// La documentation Desktop Duplication décrit cet état comme récupérable :
/// relâcher l'`IDXGIOutputDuplication` et en créer une nouvelle.
pub const ACCES_PERDU: i32 = 0x887A0026u32 as i32;

/// `DXGI_ERROR_DEVICE_REMOVED`. Le périphérique lui-même est perdu : rouvrir
/// la seule duplication ne servirait à rien. Reste définitif.
pub const DEVICE_REMOVED: i32 = 0x887A0005u32 as i32;

/// `DXGI_ERROR_WAIT_TIMEOUT`. Pas un échec : le bureau n'a simplement pas
/// changé. Traité en amont de la classification, mais nommé ici pour que le
/// test puisse vérifier qu'il n'est PAS pris pour une perte d'accès.
pub const ATTENTE_EXPIREE: i32 = 0x887A0027u32 as i32;

/// Nombre de reprises CONSÉCUTIVES tolérées.
///
/// Trois, et non une : deux fenêtres ouvertes coup sur coup produisent deux
/// remaniements de topologie rapprochés, et une reprise interrompue par la
/// suivante ne doit pas condamner la session. Au-delà, ce n'est plus une
/// rafale mais un état durable, et insister ne ferait que masquer la cause.
pub const REPRISES_MAX: u32 = 3;

pub fn est_acces_perdu(code: i32) -> bool {
    todo!("étape 3")
}

/// Compte les reprises CONSÉCUTIVES. Une image qui passe remet le compteur à
/// zéro : ce budget mesure une rafale, pas une usure.
pub struct BudgetReprises {
    consommees: u32,
}

impl BudgetReprises {
    pub fn nouveau() -> Self {
        todo!("étape 3")
    }

    /// Consomme une reprise. Rend `false` si le budget est épuisé — auquel cas
    /// rien n'est consommé et l'appelant doit abandonner.
    pub fn consommer(&mut self) -> bool {
        todo!("étape 3")
    }

    /// Une image est passée : la rafale est finie.
    pub fn succes(&mut self) {
        todo!("étape 3")
    }

    pub fn consommees(&self) -> u32 {
        todo!("étape 3")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seule_la_perte_d_acces_est_recuperable() {
        assert!(est_acces_perdu(ACCES_PERDU));
        assert!(!est_acces_perdu(DEVICE_REMOVED), "périphérique perdu : rouvrir ne sert à rien");
        assert!(!est_acces_perdu(ATTENTE_EXPIREE), "attente expirée n'est même pas un échec");
        assert!(!est_acces_perdu(0), "S_OK");
        assert!(!est_acces_perdu(0x80070057u32 as i32), "E_INVALIDARG");
    }

    #[test]
    fn le_budget_se_consomme_puis_s_epuise() {
        let mut budget = BudgetReprises::nouveau();
        for attendu in 1..=REPRISES_MAX {
            assert!(budget.consommer(), "reprise {attendu} doit être accordée");
            assert_eq!(budget.consommees(), attendu);
        }
        assert!(!budget.consommer(), "au-delà du plafond, refus");
        assert_eq!(
            budget.consommees(),
            REPRISES_MAX,
            "un refus ne consomme rien"
        );
    }

    /// Le cœur du choix : le budget mesure une RAFALE. Une session longue qui
    /// reprend une fois par heure ne doit jamais s'épuiser.
    #[test]
    fn une_image_qui_passe_remet_le_budget_a_zero() {
        let mut budget = BudgetReprises::nouveau();
        assert!(budget.consommer());
        assert!(budget.consommer());
        budget.succes();
        assert_eq!(budget.consommees(), 0);
        for _ in 0..REPRISES_MAX {
            assert!(budget.consommer(), "le plafond entier est de nouveau disponible");
        }
    }
}
```

Puis, dans `agent/src/main.rs`, juste sous la déclaration de `windows_source_sortie` (l. 40-41), ajouter :

```rust
// Même montage, et pour la même raison : la classification des échecs
// d'acquisition et le budget de reprises sont purs et doivent se tester sur
// l'hôte, alors que `capture.rs` est `#![cfg(windows)]` dans son ensemble.
#[path = "capture/reprise.rs"]
mod capture_reprise;
```

- [ ] **Étape 2 : lancer les tests pour les voir échouer**

Run: `cargo test --manifest-path agent/Cargo.toml capture_reprise`
Expected: échec — les trois tests paniquent sur `todo!`.

- [ ] **Étape 3 : écrire l'implémentation minimale**

```rust
pub fn est_acces_perdu(code: i32) -> bool {
    code == ACCES_PERDU
}
```

```rust
impl BudgetReprises {
    pub fn nouveau() -> Self {
        Self { consommees: 0 }
    }

    pub fn consommer(&mut self) -> bool {
        if self.consommees >= REPRISES_MAX {
            return false;
        }
        self.consommees += 1;
        true
    }

    pub fn succes(&mut self) {
        self.consommees = 0;
    }

    pub fn consommees(&self) -> u32 {
        self.consommees
    }
}
```

- [ ] **Étape 4 : lancer les tests pour les voir passer**

Run: `cargo test --manifest-path agent/Cargo.toml capture_reprise`
Expected: 3 tests passés.

Puis la suite entière, pour vérifier qu'aucun avertissement neuf n'apparaît :
Run: `cargo test --manifest-path agent/Cargo.toml`
Expected: 254 tests passés (251 + 3).

- [ ] **Étape 5 : commit**

```bash
git add agent/src/capture/reprise.rs agent/src/main.rs
git commit -m "feat(d2): classer les echecs d'acquisition DXGI et borner les reprises

0x887A0026 est DXGI_ERROR_ACCESS_LOST, que la documentation Desktop
Duplication decrit comme recuperable. Ce module en pose la classification et
le budget de reprises consecutives, en pur, testable sur l'hote — capture.rs
etant #![cfg(windows)] dans son ensemble, il est declare en module FRERE,
comme windows_source/sortie.rs.

Le budget mesure une RAFALE et non une usure : une image qui passe le remet a
zero. Sans quoi une session longue finirait par s'epuiser sur des reprises
espacees et sans rapport entre elles.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 2 : `DesktopCapture` retient sa cible et sait rouvrir

**Files:**
- Modify: `agent/src/capture.rs` (`struct DesktopCapture` l. 50-63, `ouvrir` l. 80-166, `sur_sortie` l. 76-78, `ouvrir_sortie` l. 422-463)

**Interfaces:**
- Consumes: rien de la tâche 1 (elle n'est câblée qu'en tâche 3).
- Produces:
  - `pub enum CibleCapture { Bureau, Sortie(String) }` — la `String` est un `nom_sortie` (`\\.\DISPLAYn`)
  - `pub fn DesktopCapture::sur_sortie(nom: &str) -> Result<Self>` — **remplace** `sur_sortie(index_adaptateur: u32, index_sortie: u32)`
  - `pub fn DesktopCapture::rouvrir(&mut self) -> Result<()>`
  - `pub fn DesktopCapture::cible(&self) -> &CibleCapture`

> ⚠️ Cette tâche **ne compile que sur la VM**. Aucun test d'hôte ne la couvre : elle est éprouvée par la tâche 5 (le banc) et par la recette. C'est la règle de tout le code `#[cfg(windows)]` de ce dépôt, pas une exception prise ici.

- [ ] **Étape 1 : introduire `CibleCapture` et la faire porter par la structure**

Dans `agent/src/capture.rs`, au-dessus de `struct DesktopCapture` :

```rust
/// Ce que cette duplication couvre — et donc ce qu'il faut rouvrir après une
/// perte d'accès.
///
/// **Un nom de sortie, jamais un index.** `(index_adaptateur, index_sortie)`
/// est positionnel : il change dès qu'une sortie apparaît ou disparaît. Or
/// c'est exactement ce qui vient de se produire quand on rouvre. `\\.\DISPLAYn`
/// est stable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CibleCapture {
    /// La sortie qui compose le bureau, quelle qu'elle soit — comportement de
    /// `DesktopCapture::new()`. Résolue à chaque ouverture, donc une
    /// réouverture peut légitimement tomber sur une autre sortie.
    Bureau,
    /// Une sortie précise, désignée par son nom.
    Sortie(String),
}
```

Ajouter le champ à `DesktopCapture` :

```rust
pub struct DesktopCapture {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    duplication: IDXGIOutputDuplication,
    /// Ce qu'il faut rouvrir après une perte d'accès. Retenu à l'ouverture :
    /// à l'instant où l'accès est perdu, la topologie a déjà changé et rien
    /// dans les objets DXGI encore détenus ne dit ce qu'on capturait.
    cible: CibleCapture,
    desktop_width: u32,
    desktop_height: u32,
    target: Option<(ID3D11Texture2D, u32, u32)>,
    frame_held: bool,
    phase: Option<Arc<AtomicU64>>,
}
```

- [ ] **Étape 2 : faire résoudre `ouvrir_sortie` par nom**

Remplacer la signature et le corps du filtre de `ouvrir_sortie` (l. 422-463) :

```rust
/// Ouvre une sortie désignée par son nom, ou — pour `CibleCapture::Bureau` —
/// trouve et ouvre la sortie qui compose le bureau.
fn ouvrir_sortie(
    factory: &IDXGIFactory1,
    cible: &CibleCapture,
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
            let nom = String::from_utf16_lossy(&desc.DeviceName)
                .trim_end_matches('\0')
                .to_string();
            let retenue = match cible {
                CibleCapture::Sortie(vise) => &nom == vise,
                CibleCapture::Bureau => desc.AttachedToDesktop.as_bool(),
            };
            if retenue {
                let name = match unsafe { adapter.GetDesc1() } {
                    Ok(adapter_desc) => String::from_utf16_lossy(&adapter_desc.Description)
                        .trim_end_matches('\0')
                        .to_string(),
                    Err(_) => "<inconnu>".to_string(),
                };
                tracing::info!(
                    adaptateur = %name, index_adaptateur, index_sortie, nom_sortie = %nom,
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
        CibleCapture::Sortie(nom) => bail!("aucune sortie DXGI nommée {nom}"),
        CibleCapture::Bureau => {
            bail!("aucune sortie attachée au bureau : la session est-elle interactive ?")
        }
    }
}
```

- [ ] **Étape 3 : adapter les constructeurs et extraire la duplication seule**

`new` et `sur_sortie` :

```rust
    pub fn new() -> Result<Self> {
        Self::ouvrir(CibleCapture::Bureau)
    }

    /// Duplique une sortie DXGI précise, désignée par son nom
    /// (`\\.\DISPLAYn`, tel que `enumerer_sorties` le rend).
    ///
    /// **Par le nom et non par des index d'énumération** : ceux-ci sont
    /// positionnels et changent dès qu'une sortie apparaît ou disparaît — ce
    /// qui est le cas nominal en multi-fenêtres, où le superviseur crée une
    /// sortie par ouverture de fenêtre.
    pub fn sur_sortie(nom: &str) -> Result<Self> {
        Self::ouvrir(CibleCapture::Sortie(nom.to_string()))
    }
```

Dans `ouvrir`, remplacer `let (adapter, output) = ouvrir_sortie(&factory, cible)?;` par `ouvrir_sortie(&factory, &cible)?`, et ajouter `cible` au littéral `Self { … }`.

Extraire la fin de `ouvrir` — depuis `DuplicateOutput` jusqu'à la lecture de `GetDesc` — dans une fonction libre réutilisable par `rouvrir` :

```rust
/// Duplique la sortie et lit ses dimensions. Le seul morceau d'`ouvrir` que
/// `rouvrir` refait.
fn dupliquer(
    device: &ID3D11Device,
    output: &IDXGIOutput1,
) -> Result<(IDXGIOutputDuplication, u32, u32)> {
    let duplication =
        unsafe { output.DuplicateOutput(device) }.context("duplication de la sortie écran")?;
    let desc = unsafe { duplication.GetDesc() };
    Ok((duplication, desc.ModeDesc.Width, desc.ModeDesc.Height))
}
```

- [ ] **Étape 4 : écrire `rouvrir`**

```rust
    /// Reconstruit la duplication après une perte d'accès, **en conservant le
    /// périphérique D3D11**.
    ///
    /// Ce n'est pas une économie, c'est une nécessité. L'encodeur H.264 est lié
    /// à ce périphérique par l'`IMFDXGIDeviceManager` (`encode::share_device`) :
    /// en créer un neuf obligerait à détruire l'encodeur, donc à emprunter
    /// `Drop for H264Encoder`, dont le pire cas est borné à 8 s et où un gel a
    /// déjà été observé (`CLAUDE.md`). Une reprise censée passer inaperçue ne
    /// peut pas payer ce prix.
    ///
    /// La protection multifil posée sur le contexte à l'ouverture n'est pas
    /// rejouée : elle porte sur le contexte immédiat, qu'on conserve.
    pub fn rouvrir(&mut self) -> Result<()> {
        // Relâcher l'image éventuellement détenue AVANT de lâcher la
        // duplication : `release_frame` appelle `ReleaseFrame` sur l'objet
        // qu'on est en train de remplacer.
        self.release_frame();

        let factory: IDXGIFactory1 =
            unsafe { CreateDXGIFactory1() }.context("création de la fabrique DXGI (réouverture)")?;
        let (_adapter, output) = ouvrir_sortie(&factory, &self.cible)
            .context("résolution de la sortie à rouvrir")?;
        let (duplication, largeur, hauteur) = dupliquer(&self.device, &output)?;

        // Les dimensions peuvent avoir changé : la texture de destination est
        // dimensionnée sur la RÉGION demandée par l'appelant, pas sur celles-ci,
        // mais `desktop_size()` est lue ailleurs et doit rester juste.
        self.duplication = duplication;
        self.desktop_width = largeur;
        self.desktop_height = hauteur;
        Ok(())
    }

    pub fn cible(&self) -> &CibleCapture {
        &self.cible
    }
```

- [ ] **Étape 5 : réparer les appelants et vérifier que l'hôte compile encore**

`agent/src/windows_source/sortie.rs` : `sur_sortie` prend désormais `nom_sortie: &str` au lieu des deux index, et le passe à `DesktopCapture::sur_sortie`. Le message de contexte de `region_de_sortie` devient `format!("sortie {nom_sortie} de dimensions inexploitables ({dw}x{dh})")`.

`agent/src/diagnostics/multifenetre/voies.rs` l. 182-187 : `partagee_sur` prend `Option<&str>` et appelle `DesktopCapture::sur_sortie(nom)`. Ses appelants (`paralleles/passes.rs` l. 50-52, `capture_virtuelle.rs`, `banc.rs`) passent `Some(&sortie.nom_sortie)`.

`agent/src/main.rs` : `Config::sortie_dxgi` devient `Option<String>`, lue sans analyse d'index :

```rust
        // ABSENTE : mode mono-fenêtre légitime. PRÉSENTE MAIS VIDE : échec du
        // démarrage, jamais un repli muet — même règle que `FENETRE_HWND`.
        // Plus d'analyse `adaptateur:sortie` : c'est un NOM de sortie DXGI
        // (`\\.\DISPLAYn`), stable là où les index sont positionnels.
        sortie_dxgi: match std::env::var("SORTIE_DXGI") {
            Ok(brut) => {
                let nom = brut.trim().to_string();
                anyhow::ensure!(!nom.is_empty(), "SORTIE_DXGI est vide");
                Some(nom)
            }
            Err(_) => None,
        },
```

Adapter `agent/src/demarrage/source.rs:72` (`match config.sortie_dxgi`), qui construit la source : la branche `Some` passe désormais un `&str` à `WindowsSource::sur_sortie`.

Run: `cargo test --manifest-path agent/Cargo.toml`
Expected: 254 tests passés — la compilation de l'hôte doit tenir, seule la part Windows changeant réellement.

Run: `cargo check --manifest-path agent/Cargo.toml --target x86_64-pc-windows-msvc` **si et seulement si** la chaîne croisée est installée localement ; sinon cette vérification se fait à l'étape 6.

- [ ] **Étape 6 : compiler sur la VM**

```bash
virsh list --all   # démarrer si « fermé », puis attendre `ls /media/vm/dev`
set -a && source .env && set +a
scripts/build-agent.sh 2>&1 | tee /tmp/user/0/claude-0/-home-mallanic-Projects-Guacamole/*/scratchpad/build-t2.log
```

Expected: compilation sans erreur. **Conserver ce journal** : le chantier précédent s'est reproché de n'avoir aucune pièce attestant la fraîcheur du binaire mesuré.

- [ ] **Étape 7 : commit**

```bash
git add agent/src/capture.rs agent/src/main.rs agent/src/windows_source/sortie.rs \
        agent/src/demarrage/source.rs agent/src/diagnostics/multifenetre/voies.rs \
        agent/src/diagnostics/multifenetre/paralleles/passes.rs \
        agent/src/diagnostics/multifenetre/capture_virtuelle.rs \
        agent/src/diagnostics/multifenetre/banc.rs
git commit -m "feat(d2): DesktopCapture retient sa cible, sait rouvrir, et se designe par nom

Une duplication ne savait pas ce qu'elle couvrait : a l'instant ou l'acces est
perdu, la topologie a deja change et rien dans les objets DXGI detenus ne dit
ce qu'on capturait. `CibleCapture` le retient — par le NOM de sortie, jamais
par un index d'enumeration, celui-ci etant positionnel et changeant a chaque
apparition ou disparition de sortie, c'est-a-dire au cas nominal du
multi-fenetres.

`rouvrir` reconstruit la seule IDXGIOutputDuplication et CONSERVE le
peripherique D3D11 : l'encodeur H.264 y est lie par l'IMFDXGIDeviceManager, et
en creer un neuf obligerait a emprunter Drop for H264Encoder, dont le pire cas
est borne a 8 s et ou un gel a deja ete observe.

SORTIE_DXGI porte desormais un nom (\\\\.\\DISPLAYn) et non « adaptateur:sortie ».

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 3 : `next_frame` classe l'échec et reprend

**Files:**
- Modify: `agent/src/capture.rs` (`next_frame` l. 190-257)
- Modify: `agent/src/windows_source.rs` (`match captured` l. 552-580) — **dette gelée, solde de lignes ≤ 0**
- Modify: `agent/src/diagnostics/multifenetre/voies.rs` (l. 146)

**Interfaces:**
- Consumes: `capture_reprise::{est_acces_perdu, BudgetReprises}` (tâche 1) ; `DesktopCapture::rouvrir` (tâche 2).
- Produces:
  - `pub enum EchecAcquisition { AccesPerdu, Panne(anyhow::Error) }` dans `capture.rs`, avec `impl std::fmt::Display`
  - `pub fn DesktopCapture::next_frame(&mut self, region: Rect) -> std::result::Result<Option<CapturedFrame>, EchecAcquisition>`

- [ ] **Étape 1 : déclarer le type d'échec et le budget porté par la structure**

Dans `agent/src/capture.rs` :

```rust
/// Pourquoi une acquisition d'image a échoué, une fois les reprises épuisées.
///
/// `AccesPerdu` ne remonte pas à la première perte : `next_frame` tente de se
/// rouvrir d'abord (voir `BudgetReprises`). Le recevoir signifie « je n'ai pas
/// pu revenir », pas « l'accès vient d'être perdu ».
pub enum EchecAcquisition {
    AccesPerdu,
    Panne(anyhow::Error),
}

impl std::fmt::Display for EchecAcquisition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AccesPerdu => write!(
                f,
                "accès à la duplication perdu et non repris après {} tentatives",
                crate::capture_reprise::REPRISES_MAX
            ),
            Self::Panne(e) => write!(f, "{e:#}"),
        }
    }
}
```

Ajouter le champ `budget: crate::capture_reprise::BudgetReprises` à `DesktopCapture`, initialisé par `BudgetReprises::nouveau()` dans `ouvrir`.

- [ ] **Étape 2 : réécrire le corps de `next_frame`**

La fonction devient une enveloppe autour du corps actuel, renommé `tenter_acquisition` et rendant `std::result::Result<Option<CapturedFrame>, EchecAcquisition>` :

```rust
    /// Acquiert l'image suivante et la recadre sur `region`, **en se rouvrant
    /// si DXGI lui a révoqué l'accès**.
    ///
    /// La reprise est ici, et non chez l'appelant, à dessein : le banc
    /// multi-fenêtres capture par cette fonction sans passer par
    /// `WindowsSource` (`diagnostics/multifenetre/voies.rs`). Une reprise logée
    /// plus haut laisserait le banc hors du chemin de production, et la mesure
    /// qui doit valider cette voie ne vaudrait rien.
    pub fn next_frame(
        &mut self,
        region: Rect,
    ) -> std::result::Result<Option<CapturedFrame>, EchecAcquisition> {
        loop {
            match self.tenter_acquisition(region) {
                Ok(issue) => {
                    self.budget.succes();
                    return Ok(issue);
                }
                Err(EchecAcquisition::AccesPerdu) => {
                    if !self.budget.consommer() {
                        return Err(EchecAcquisition::AccesPerdu);
                    }
                    // `info!` et non `debug!` : l'exploitation tourne en
                    // RUST_LOG=info, et une mitigation muette n'en est pas une.
                    // Rare par construction — une reprise correspond à un
                    // remaniement de la topologie d'affichage.
                    tracing::info!(
                        tentative = self.budget.consommees(),
                        cible = ?self.cible,
                        "accès à la duplication perdu, réouverture"
                    );
                    if let Err(erreur) = self.rouvrir() {
                        return Err(EchecAcquisition::Panne(erreur));
                    }
                }
                Err(panne) => return Err(panne),
            }
        }
    }
```

Et dans `tenter_acquisition`, le bloc de classification remplace les l. 211-216 :

```rust
        if let Err(e) = acquired {
            if e.code() == DXGI_ERROR_WAIT_TIMEOUT {
                return Ok(None);
            }
            // Classer sur le CODE, jamais sur le texte du message : `CLAUDE.md`
            // porte le précédent d'un libellé Windows qui a fait attribuer un
            // refus au mauvais appel pendant tout un chantier.
            if crate::capture_reprise::est_acces_perdu(e.code().0) {
                return Err(EchecAcquisition::AccesPerdu);
            }
            return Err(EchecAcquisition::Panne(anyhow!("acquisition d'image : {e}")));
        }
```

Les deux autres sorties d'erreur de `tenter_acquisition` (l. 225 `ressource d'image absente`, l. 252-255 l'échec de `crop`) rendent `EchecAcquisition::Panne(...)`.

- [ ] **Étape 3 : adapter `windows_source.rs` sans ajouter une ligne**

Le `match captured` (l. 552-580) : la branche `Err(e)` devient

```rust
            Err(e) => {
                // Une perte d'accès est déjà passée par les reprises de
                // `next_frame` : la recevoir ici signifie qu'elles n'ont pas
                // suffi. Fin légitime dans les deux cas.
                tracing::error!(erreur = %e, "capture interrompue, source déclarée épuisée");
                self.fatal = true;
                return None;
            }
```

`%e` fonctionne grâce au `Display` de l'étape 1. Le commentaire de trois lignes existant (l. 573-575) est **remplacé**, pas complété : le solde de lignes de ce fichier doit rester nul ou négatif.

Vérifier : `wc -l agent/src/windows_source.rs` ≤ 648.

- [ ] **Étape 4 : adapter `voies.rs`**

Ligne 146, `amorcer` : `self.dernier_bureau = self.capture.next_frame(self.bureau)?;` — le `?` ne compile plus, `EchecAcquisition` n'étant pas convertible en `anyhow::Error`. Remplacer par

```rust
        self.dernier_bureau = self
            .capture
            .next_frame(self.bureau)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
```

- [ ] **Étape 5 : vérifier l'hôte, puis compiler sur la VM**

Run: `cargo test --manifest-path agent/Cargo.toml`
Expected: 254 tests passés.

```bash
scripts/build-agent.sh
```
Expected: compilation sans erreur.

- [ ] **Étape 6 : commit**

```bash
git add agent/src/capture.rs agent/src/windows_source.rs \
        agent/src/diagnostics/multifenetre/voies.rs
git commit -m "feat(d2): next_frame classe l'echec d'acquisition et se rouvre

Toute erreur d'acquisition etait definitive : c'est ce qui faisait mourir
toutes les sessions de capture des qu'une sortie virtuelle etait creee, defaut
central du sous-bloc D1. Trois branches desormais au lieu de deux, classees sur
le CODE et jamais sur le texte du message.

La reprise est dans next_frame et non chez son appelant : le banc
multi-fenetres capture par cette fonction sans passer par WindowsSource, et une
reprise logee plus haut laisserait la mesure qui doit valider cette voie hors du
chemin de production.

windows_source.rs est en dette gelee : son solde de lignes reste negatif.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 4 : le superviseur désigne ses sorties par leur nom

**Files:**
- Modify: `agent/src/superviseur/table.rs` (`Effet::LancerEnfant` l. 46-52, `Effet::DetruireSortie` l. 55-64, `Entree` l. 66-81, `sortie_creee` l. 191-213, `sortie_dxgi_de` l. 217-219, `fenetre_disparue` l. 232-257, `enfant_mort` l. 259-278)
- Modify: `agent/src/superviseur/table/tests.rs`
- Modify: `agent/src/superviseur/enfants.rs` (`Consigne` l. 15-26)
- Modify: `agent/src/superviseur/lanceur.rs` (l. 139-142)
- Modify: `agent/src/superviseur/boucle.rs` (`prises` l. 62-65, l. 299-303, `rendre_la_sortie`)

**Interfaces:**
- Consumes: `SortieDxgi::nom_sortie` (existant).
- Produces:
  - `Effet::LancerEnfant { session, fenetre, nom_sortie: String, audio }` — les deux champs `index_*` disparaissent
  - `Effet::DetruireSortie { sortie_pilote: u32, nom_sortie: String }`
  - `Table::sortie_creee(&mut self, session: &IdSession, sortie_pilote: u32, nom_sortie: String) -> Vec<Effet>`
  - `Table::nom_sortie_de(&self, session: &IdSession) -> Option<&str>` — remplace `sortie_dxgi_de`
  - `Consigne { session, fenetre, nom_sortie: String, audio }`

- [ ] **Étape 1 : écrire le test qui échoue**

Dans `agent/src/superviseur/table/tests.rs`, ajouter :

```rust
/// Le défaut §3.3 bis de D1 : l'enfant recevait `(adaptateur, sortie)`, un
/// couple POSITIONNEL qu'il résolvait plus tard — après que d'autres sorties
/// avaient pu apparaître ou disparaître. D'où les `aucune sortie DXGI à
/// l'index adaptateur 0, sortie 5` relevés en recette.
#[test]
fn la_sortie_est_transmise_a_l_enfant_par_son_nom() {
    let mut t = Table::nouvelle(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("ouverture attendue, reçu {effets:?}");
    };
    let session = session.clone();
    t.viewport_recu(&session, 1280, 720);

    let effets = t.sortie_creee(&session, 42, "\\\\.\\DISPLAY7".into());

    assert_eq!(
        effets,
        vec![Effet::LancerEnfant {
            session: session.clone(),
            fenetre: IdFenetre(1),
            nom_sortie: "\\\\.\\DISPLAY7".into(),
            audio: true,
        }]
    );
    assert_eq!(t.nom_sortie_de(&session), Some("\\\\.\\DISPLAY7"));
}

/// La destruction porte les DEUX identifiants — celui du pilote pour retirer,
/// le nom DXGI pour libérer la place — et ils n'ont aucune relation calculable.
#[test]
fn la_destruction_porte_l_identifiant_pilote_et_le_nom_dxgi() {
    let mut t = Table::nouvelle(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("ouverture attendue, reçu {effets:?}");
    };
    let session = session.clone();
    t.viewport_recu(&session, 1280, 720);
    t.sortie_creee(&session, 42, "\\\\.\\DISPLAY7".into());

    let effets = t.fenetre_disparue(IdFenetre(1));

    assert!(effets.contains(&Effet::DetruireSortie {
        sortie_pilote: 42,
        nom_sortie: "\\\\.\\DISPLAY7".into(),
    }));
}
```

- [ ] **Étape 2 : lancer les tests pour les voir échouer**

Run: `cargo test --manifest-path agent/Cargo.toml superviseur::table`
Expected: échec de compilation — `Effet::LancerEnfant` n'a pas de champ `nom_sortie`, `nom_sortie_de` n'existe pas.

- [ ] **Étape 3 : porter le nom dans la table, la consigne et le lanceur**

`table.rs` — `Effet` :

```rust
    LancerEnfant {
        session: IdSession,
        fenetre: IdFenetre,
        /// Nom DXGI de la sortie (`\\.\DISPLAYn`), **et non un couple
        /// d'index** : ceux-ci sont positionnels, l'enfant les résout à son
        /// démarrage — donc plus tard — et une sortie apparue ou disparue
        /// entre-temps le fait capturer autre chose, ou échouer.
        nom_sortie: String,
        audio: bool,
    },
```

```rust
    /// `sortie_pilote` est **l'identifiant du PILOTE**, `nom_sortie` le nom
    /// DXGI de la même sortie. Le pilote ne sait retirer que par le premier ;
    /// la place ne se libère que par le second. Aucune relation calculable
    /// entre eux — d'où les deux champs.
    DetruireSortie { sortie_pilote: u32, nom_sortie: String },
```

`Entree` : le champ `dxgi: Option<(u32, u32)>` devient `nom_sortie: Option<String>`.

`sortie_creee` prend `nom_sortie: String`, le pose sur l'entrée, et le place dans `LancerEnfant`.

`sortie_dxgi_de` devient :

```rust
    /// Nom de la sortie d'une session, pour le contrôle périodique de
    /// placement.
    pub fn nom_sortie_de(&self, session: &IdSession) -> Option<&str> {
        self.entrees.get(session).and_then(|e| e.nom_sortie.as_deref())
    }
```

`fenetre_disparue` et `enfant_mort` : le repli `entree.dxgi.unwrap_or((0, 0))` devient `entree.nom_sortie.clone().unwrap_or_default()`, avec le même commentaire expliquant qu'il n'est pas atteignable.

`enfants.rs` — `Consigne` : les champs `index_adaptateur` et `index_sortie` sont remplacés par `pub nom_sortie: String`.

`lanceur.rs` l. 139-142 :

```rust
            .env("SORTIE_DXGI", &consigne.nom_sortie)
```

`boucle.rs` : `prises` devient `Vec<String>` ; l. 299-303 deviennent

```rust
    let nom = cible.nom_sortie.clone();
    prises.push(nom.clone());
    let suite = table.sortie_creee(&session, id_pilote, nom);
```

et les deux `prises.retain(|p| *p != place)` / la signature de `rendre_la_sortie` suivent. `controler_le_placement` emploie `nom_sortie_de` puis retrouve le rectangle par `enumerer_sorties_silencieux`, en appariant sur le nom.

- [ ] **Étape 4 : lancer les tests pour les voir passer**

Run: `cargo test --manifest-path agent/Cargo.toml`
Expected: 256 tests passés (254 + 2).

- [ ] **Étape 5 : compiler sur la VM**

```bash
scripts/build-agent.sh
```
Expected: compilation sans erreur.

- [ ] **Étape 6 : commit**

```bash
git add agent/src/superviseur/table.rs agent/src/superviseur/table/tests.rs \
        agent/src/superviseur/enfants.rs agent/src/superviseur/lanceur.rs \
        agent/src/superviseur/boucle.rs
git commit -m "feat(d2): designer les sorties par leur nom DXGI dans tout le superviseur

(index_adaptateur, index_sortie) est positionnel : il change des qu'une sortie
apparait ou disparait. Le superviseur le passait pourtant a l'enfant, qui le
resolvait a son demarrage — d'ou les « aucune sortie DXGI a l'index adaptateur
0, sortie 5 » de la recette D1. Le nom (\\\\.\\DISPLAYn) est stable, et le
superviseur l'avait deja sous la main.

DetruireSortie continue de porter les DEUX identifiants : le pilote ne sait
retirer que par le sien, la place ne se libere que par le nom, et rien ne se
calcule de l'un a l'autre.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 5 : le banc `MULTIFENETRE_REPRISE`

**Files:**
- Create: `agent/src/diagnostics/multifenetre/reprise.rs`
- Modify: `agent/src/diagnostics/multifenetre.rs` (aiguillage, après la ligne 149 `MULTIFENETRE_VDD_PARALLELE`)

**Interfaces:**
- Consumes: `montee::{attendre_en_pinguant, noms_attaches, relever_topologie, DELAI_TOPOLOGIE, RESOLUTION}` ; `voies::{VoieDeCapture, VoieDuplication}` ; `mires::Mires` ; `compteurs::{Compteurs, Garde}` ; `moniteurs_virtuels::Sorties` ; `mire::{voie_controlee, verdict, Verdict}` — toutes existantes, employées par `paralleles.rs` et `paralleles/passes.rs` qu'on prendra pour modèle.
- Produces: `pub(super) fn mesurer(nombre: u8) -> Result<()>`

> Ce banc est la **preuve de l'inférence** sur laquelle repose tout le sous-bloc. Il est un point d'arrêt : si les duplications ne reprennent pas, la voie A est réfutée et il faut basculer sur la sérialisation (§8 de la spec) avant d'aller plus loin.

- [ ] **Étape 1 : écrire le module, calqué sur `paralleles.rs`**

Créer `agent/src/diagnostics/multifenetre/reprise.rs`. Structure obligatoire, dans cet ordre :

1. **Relevé initial** : `relever_topologie("avant création")`, `noms_attaches`, `HashSet` des noms connus — comme `paralleles.rs` l. 68-72.
2. **Création de `nombre` sorties virtuelles** sous la garde `Sorties::nouvelles(&pilote)`, `attendre_en_pinguant(&pilote, DELAI_TOPOLOGIE)`, puis `designer_sorties_neuves` (réemployer celle de `paralleles.rs` en la rendant `pub(super)`, ne pas la recopier).
3. **Ouverture des `nombre` duplications** et pose des mires, par `VoieDuplication::partagee_sur(Some(&sortie.nom_sortie))` — même boucle que `paralleles/passes.rs::ouvrir_duplications`.
4. **Passe A, avant perturbation**, de durée `compteurs::DUREE_PASSE` (ne pas introduire une durée neuve) : capture en rotation, `Compteurs::nouveaux(nombre)`, verdicts par `mire::voie_controlee` et `compteurs::lire_verdict`, puis `compteurs::journaliser("avant perturbation", "duplication", nombre, &compteurs_a)`.
5. **La perturbation** : créer **une sortie de plus**, journalisée sans ambiguïté :

```rust
    tracing::info!(
        duplications_ouvertes = voies.len(),
        "création d'une sortie de PLUS pendant que les duplications tournent — c'est la perturbation mesurée"
    );
    let id_perturbatrice = sorties
        .creer(largeur, hauteur, hertz)
        .context("création de la sortie perturbatrice")?;
    tracing::info!(id = id_perturbatrice, "sortie perturbatrice créée");
```

6. **Passe B, après perturbation**, **identique à la passe A**, journalisée sous `"après perturbation"`.
7. **Bilan**, la seule sortie qui compte. `Compteurs` expose `images: Vec<u64>` (une entrée par voie) et `apres_recouvrement: Verdicts` avec `.faux()` — employer ces champs-là, ils existent :

```rust
    // Le chiffre décisif est le nombre de voies qui rendent ENCORE des images
    // après la perturbation, voie par voie : un total masquerait une voie
    // morte compensée par une autre.
    let vivantes_apres = compteurs_b.images.iter().filter(|n| **n > 0).count();
    tracing::info!(
        images_avant = ?compteurs_a.images,
        images_apres = ?compteurs_b.images,
        voies_vivantes_apres = vivantes_apres,
        voies_totales = voies.len(),
        verdicts_faux_avant = compteurs_a.apres_recouvrement.faux(),
        verdicts_faux_apres = compteurs_b.apres_recouvrement.faux(),
        "bilan de la reprise"
    );
```

Le nombre de reprises effectives ne se compte pas ici : il se lit au journal, sur les `info!` posées par `next_frame` en tâche 3 (`grep -c "accès à la duplication perdu, réouverture"`). Ne pas ajouter un compteur qui doublerait cette trace.

⚠️ **Ce banc n'a pas de recouvrement** — une fenêtre par sortie, rien ne peut en cacher une autre. Les verdicts se rangent donc tous dans `apres_recouvrement`, comme dans `paralleles`, et il n'y a **aucune porte éliminatoire** : ne pas transposer ici l'interprétation du banc à aire fixe.

8. **Restauration** : destruction de toutes les sorties par la garde, relevé final, comparaison **des ensembles de noms** — jamais des cardinaux — comme `paralleles.rs` l. 125-138.

Le commentaire de tête doit porter ce que ce banc **ne** dit **pas** :

```rust
//! # Ce que ce banc ne dit pas
//!
//! - **Une exécution par rang ne donne aucun taux.** Le sous-bloc D1 a
//!   reproduit son défaut trois fois sur trois ; une reprise qui marche une
//!   fois ne prouve pas qu'elle marche toujours.
//! - **Les mires ne sont pas des applications** : D3D11 plein cadre, sans
//!   occlusion ni interaction.
//! - **La justesse est ÉCHANTILLONNÉE** — une voie contrôlée par tour.
//! - **La DESTRUCTION d'une sortie n'est pas exercée ici**, pas plus qu'elle
//!   ne l'a été en D1.
```

- [ ] **Étape 2 : câbler l'aiguillage**

Dans `agent/src/diagnostics/multifenetre.rs`, après le bloc `MULTIFENETRE_VDD_PARALLELE` (l. 149) :

```rust
    // Crée des sorties, donc passe après `MULTIFENETRE_VDD_PURGE`.
    if let Ok(texte) = std::env::var("MULTIFENETRE_REPRISE") {
        let nombre: u8 = texte
            .parse()
            .context("MULTIFENETRE_REPRISE doit être un entier (nombre de duplications)")?;
        reprise::mesurer(nombre)?;
        return Ok(true);
    }
```

Et `mod reprise;` en tête du fichier, auprès des autres.

- [ ] **Étape 3 : vérifier la taille et compiler sur la VM**

```bash
wc -l agent/src/diagnostics/multifenetre/reprise.rs
```
Expected: ≤ 500. Au-delà, extraire la boucle de passes dans `reprise/passes.rs`, comme `paralleles` l'a fait.

Run: `cargo test --manifest-path agent/Cargo.toml`
Expected: 256 tests passés (le banc est `#[cfg(windows)]`, il n'ajoute aucun test d'hôte).

```bash
scripts/build-agent.sh
```
Expected: compilation sans erreur.

- [ ] **Étape 4 : commit**

```bash
git add agent/src/diagnostics/multifenetre/reprise.rs \
        agent/src/diagnostics/multifenetre.rs \
        agent/src/diagnostics/multifenetre/paralleles.rs
git commit -m "feat(d2): banc MULTIFENETRE_REPRISE — k duplications, une sortie de plus

Le banc qui eprouve l'inference fondatrice du sous-bloc : que
DXGI_ERROR_ACCESS_LOST soit recuperable sur CE terrain, et pas seulement dans
la documentation. Sans navigateur ni signaling, donc sans rien qui puisse
masquer la cause.

Il capture par DesktopCapture::next_frame, c'est-a-dire par le chemin de
production : c'est la raison pour laquelle la reprise a ete logee la et non
dans WindowsSource.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 6 : recette étape 1 — éprouver la reprise, **point d'arrêt**

**Files:**
- Create: `docs/superpowers/plans/journaux-multifenetres-d2/reprise-n{1,2,4}.log`

**Interfaces:**
- Consumes: le binaire construit en tâche 5.
- Produces: le verdict qui autorise — ou non — la suite du plan.

> ⛔ **Si cette tâche échoue, ARRÊTER le plan** et rouvrir la conception sur la voie de repli (spec §8, sérialisation superviseur→enfants). Ne pas enchaîner les tâches 7 et suivantes sur une hypothèse réfutée.

- [ ] **Étape 1 : préparer la VM**

```bash
virsh list --all
# si « fermé » :
virsh start Windows
until timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null; do sleep 5; done
until ls /media/vm/dev >/dev/null 2>&1; do sleep 5; done
set -a && source .env && set +a
node scripts/winrm.js 'Get-Process agent -ErrorAction SilentlyContinue | Format-List Id,StartTime'
```

Expected: aucun processus `agent`. S'il y en a un, le tuer — sinon on mesurerait le processus précédent (piège relevé en D1).

Contrôler l'état des sorties **depuis un processus neuf**, et purger si nécessaire :

```bash
MULTIFENETRE_DXGI=1 scripts/run-agent.sh
# si des sorties orphelines apparaissent :
MULTIFENETRE_VDD_PURGE=1 scripts/run-agent.sh
```

- [ ] **Étape 2 : mesurer aux trois rangs**

```bash
MULTIFENETRE_REPRISE=1 scripts/run-agent.sh
MULTIFENETRE_REPRISE=2 scripts/run-agent.sh
MULTIFENETRE_REPRISE=4 scripts/run-agent.sh
```

Après **chaque** exécution, contrôler la topologie depuis un processus neuf (`MULTIFENETRE_DXGI=1`) : le processus mesureur est juge et partie, et une sortie virtuelle lui survit.

- [ ] **Étape 3 : verser les journaux et lire le verdict**

```bash
mkdir -p docs/superpowers/plans/journaux-multifenetres-d2
# récupérer chaque journal depuis la VM vers reprise-n<N>.log
grep -c "accès à la duplication perdu, réouverture" docs/superpowers/plans/journaux-multifenetres-d2/reprise-n4.log
grep "bilan de la reprise" docs/superpowers/plans/journaux-multifenetres-d2/reprise-n4.log
```

**Reçu si**, aux trois rangs : `voies_vivantes_apres` == `voies_totales`, `images_apres` > 0 sur **chaque** voie, `verdicts_faux_apres` == 0, et **au moins une** reprise observée — sans quoi la perturbation n'a rien perturbé et le banc ne mesure rien.

**Réfuté si** une voie meurt malgré la reprise, ou si `rouvrir` échoue. Dans ce cas : consigner le HRESULT nu et la chaîne de causes complète, **arrêter le plan**, et rouvrir la conception.

- [ ] **Étape 4 : commit des journaux**

```bash
git add docs/superpowers/plans/journaux-multifenetres-d2/
git commit -m "recette(d2): etape 1 — la reprise sur perte d'acces DXGI, eprouvee au banc

<Remplacer par le verdict REELLEMENT obtenu, chiffres a l'appui : rangs joues,
reprises observees, voies vivantes apres perturbation, verdicts faux. Une
execution par rang, donc AUCUN taux — le dire.>

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 6 bis : recalibrer la reprise en fenêtre de durée

> **Tâche née de la mesure.** La tâche 6 a déclenché le point d'arrêt : critère
> de réception non atteint, **hypothèse non réfutée**. Les sept sondes
> post-mortem sur sept rapportent que la sortie se rouvre et rend une image une
> fois la topologie stabilisée. Le défaut est un **calibrage**, et il est dans
> la spec — dont le §3.4 a été réécrit en conséquence.

**Files:**
- Modify: `agent/src/capture/reprise.rs` (`BudgetReprises` et ses tests)
- Modify: `agent/src/capture.rs` (boucle de `next_frame`, champ porté par `DesktopCapture`)
- Test: `agent/src/capture/reprise.rs`

**Interfaces:**
- Consumes: `est_acces_perdu` (inchangé), `DesktopCapture::rouvrir` (inchangé).
- Produces, en **remplacement** de `BudgetReprises` :
  - `pub const DUREE_FENETRE_REPRISE: Duration` = 8 s
  - `pub const PAS_REPRISE: Duration` = 150 ms
  - `pub struct FenetreDeReprise` avec :
    - `pub fn nouvelle() -> Self`
    - `pub fn tenter(&mut self, maintenant: Instant) -> Tentative`
    - `pub fn succes(&mut self)`
    - `pub fn tentatives(&self) -> u32`
  - `pub enum Tentative { Rouvrir, Patienter, Expiree }`

**Le point de conception à ne pas manquer** : `FenetreDeReprise` **ne lit aucune
horloge**. L'instant lui est passé en argument — c'est ce qui la rend testable
sur l'hôte Linux, où tout le reste de ce chemin est invisible.

**Ce que la sémantique doit être**, et c'est la seule chose qui compte :

- premier appel à `tenter` après un succès → la fenêtre s'ouvre à `maintenant`,
  et rend `Rouvrir` (on retente tout de suite, la reconstruction peut réussir
  du premier coup) ;
- appel suivant moins de `PAS_REPRISE` après la dernière tentative → `Patienter`.
  L'appelant rend alors `Ok(None)` **sans dormir** ;
- appel au-delà du pas, la fenêtre n'ayant pas expiré → `Rouvrir` ;
- appel au-delà de `DUREE_FENETRE_REPRISE` depuis l'**ouverture** → `Expiree` ;
- `succes()` referme la fenêtre : la prochaine perte d'accès rouvre une fenêtre
  pleine.

- [ ] **Étape 1 : écrire les tests qui échouent**

Dans `agent/src/capture/reprise.rs`, **remplacer** le module de tests de
`BudgetReprises` par :

```rust
    /// Une base d'instants qui ne lit pas l'horloge du système : le type sous
    /// test n'en lit aucune, c'est tout l'intérêt.
    fn t(base: std::time::Instant, ms: u64) -> std::time::Instant {
        base + std::time::Duration::from_millis(ms)
    }

    #[test]
    fn la_premiere_perte_fait_rouvrir_tout_de_suite() {
        let base = std::time::Instant::now();
        let mut fenetre = FenetreDeReprise::nouvelle();
        assert_eq!(fenetre.tenter(t(base, 0)), Tentative::Rouvrir);
        assert_eq!(fenetre.tentatives(), 1);
    }

    /// Le défaut que la mesure a relevé : trois tentatives sans délai étaient
    /// brûlées en 14 à 21 ms, alors que la topologie met jusqu'à 3 s à se
    /// stabiliser. Le pas d'attente est ce qui empêche cela.
    #[test]
    fn une_seconde_tentative_trop_proche_fait_patienter() {
        let base = std::time::Instant::now();
        let mut fenetre = FenetreDeReprise::nouvelle();
        fenetre.tenter(t(base, 0));
        assert_eq!(fenetre.tenter(t(base, 5)), Tentative::Patienter);
        assert_eq!(fenetre.tenter(t(base, 20)), Tentative::Patienter);
        assert_eq!(
            fenetre.tentatives(),
            1,
            "patienter n'est pas une tentative"
        );
    }

    #[test]
    fn le_pas_ecoule_fait_rouvrir_a_nouveau() {
        let base = std::time::Instant::now();
        let mut fenetre = FenetreDeReprise::nouvelle();
        fenetre.tenter(t(base, 0));
        let apres_le_pas = PAS_REPRISE.as_millis() as u64;
        assert_eq!(fenetre.tenter(t(base, apres_le_pas)), Tentative::Rouvrir);
        assert_eq!(fenetre.tentatives(), 2);
    }

    /// La fenêtre doit couvrir largement les 3 s que ce dépôt admet déjà pour
    /// qu'une topologie se stabilise (`DELAI_TOPOLOGIE`).
    #[test]
    fn la_fenetre_couvre_largement_la_stabilisation_de_la_topologie() {
        assert!(
            DUREE_FENETRE_REPRISE >= std::time::Duration::from_secs(6),
            "la sonde post-mortem a réussi à 3 s ; une fenêtre qui ne les \
             couvrirait pas au double reproduirait le défaut mesuré"
        );
        assert!(
            PAS_REPRISE < DUREE_FENETRE_REPRISE / 10,
            "un pas trop grand devant la fenêtre retarderait la reprise réelle"
        );
    }

    #[test]
    fn la_fenetre_expire_au_bout_de_sa_duree() {
        let base = std::time::Instant::now();
        let mut fenetre = FenetreDeReprise::nouvelle();
        fenetre.tenter(t(base, 0));
        let apres = DUREE_FENETRE_REPRISE.as_millis() as u64 + 1;
        assert_eq!(fenetre.tenter(t(base, apres)), Tentative::Expiree);
    }

    /// L'expiration se compte depuis l'OUVERTURE de la fenêtre, pas depuis la
    /// dernière tentative : sans quoi une reprise qui échoue indéfiniment ne
    /// finirait jamais.
    #[test]
    fn l_expiration_se_compte_depuis_l_ouverture_et_non_depuis_la_derniere_tentative() {
        let base = std::time::Instant::now();
        let mut fenetre = FenetreDeReprise::nouvelle();
        let pas = PAS_REPRISE.as_millis() as u64;
        let mut instant = 0;
        while instant < DUREE_FENETRE_REPRISE.as_millis() as u64 {
            fenetre.tenter(t(base, instant));
            instant += pas;
        }
        assert_eq!(fenetre.tenter(t(base, instant)), Tentative::Expiree);
    }

    /// La fenêtre se referme sur un succès d'acquisition, `Ok(None)` compris —
    /// c'est-à-dire dès que DXGI cesse de refuser, même sans image neuve. Un
    /// bureau immobile ne produit aucune image pendant de longues périodes, et
    /// une fenêtre qui ne se refermerait que sur une image livrée
    /// transformerait des pertes rares et sans rapport en une usure.
    #[test]
    fn un_succes_referme_la_fenetre_qui_rouvre_alors_pleine() {
        let base = std::time::Instant::now();
        let mut fenetre = FenetreDeReprise::nouvelle();
        fenetre.tenter(t(base, 0));
        fenetre.succes();
        assert_eq!(fenetre.tentatives(), 0);

        let tard = DUREE_FENETRE_REPRISE.as_millis() as u64 * 3;
        assert_eq!(
            fenetre.tenter(t(base, tard)),
            Tentative::Rouvrir,
            "une perte d'accès bien plus tard ouvre une fenêtre NEUVE"
        );
        assert_eq!(
            fenetre.tenter(t(base, tard + DUREE_FENETRE_REPRISE.as_millis() as u64 + 1)),
            Tentative::Expiree,
            "et cette fenêtre neuve court depuis SA propre ouverture"
        );
    }
```

- [ ] **Étape 2 : lancer les tests pour les voir échouer**

Run: `cargo test --manifest-path agent/Cargo.toml capture_reprise`
Expected: échec de compilation — `FenetreDeReprise` et `Tentative` n'existent pas.

- [ ] **Étape 3 : écrire l'implémentation**

Remplacer `REPRISES_MAX` et `BudgetReprises` par :

```rust
/// Durée pendant laquelle une perte d'accès est retentée avant d'être déclarée
/// définitive.
///
/// **Majorante et non calibrée, et il faut le dire.** La mesure du
/// 1ᵉʳ août 2026 (`plans/journaux-multifenetres-d2/`) établit deux points et
/// deux seulement : trois tentatives enchaînées sans délai, soit 14 à 21 ms,
/// **ne suffisent pas** ; et une réouverture tentée 3 s après le remaniement
/// **réussit**, sur 7 sondes sur 7. Le seuil réel est quelque part entre les
/// deux et n'a pas été cherché. Huit secondes le couvrent largement.
///
/// Ce qui borne le coût d'une valeur trop grande : la fenêtre ne bloque rien
/// (voir `Tentative::Patienter`), elle ne fait que retarder l'aveu d'échec
/// d'une source qui, de toute façon, ne rendrait plus d'image.
pub const DUREE_FENETRE_REPRISE: std::time::Duration = std::time::Duration::from_secs(8);

/// Intervalle minimal entre deux tentatives de réouverture.
///
/// Petit devant la fenêtre, pour ne pas retarder la reprise réelle ; assez
/// grand pour que la trace `info!` de chaque tentative reste rare — au plus
/// ~6 lignes par seconde et par source, contre une par appel de `next_frame`
/// (~90/s) si le pas n'existait pas. Le dépôt a déjà payé deux fois pour une
/// trace émise à la cadence de la boucle de capture.
pub const PAS_REPRISE: std::time::Duration = std::time::Duration::from_millis(150);

/// Ce que la fenêtre demande à l'appelant de faire, maintenant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tentative {
    /// Retenter la réouverture tout de suite.
    Rouvrir,
    /// Ne rien faire de ce tour-ci. L'appelant rend « rien de neuf » — **sans
    /// dormir** : c'est ce qui distingue cette forme d'une boucle de reprise
    /// bloquante, et ce qui laisse la boucle de session continuer de tourner.
    Patienter,
    /// La fenêtre est close : la perte d'accès est définitive.
    Expiree,
}

/// Fenêtre de reprise, ouverte à la première perte d'accès et refermée par le
/// premier succès.
///
/// **Elle ne lit aucune horloge** : l'instant lui est passé. C'est ce qui la
/// rend testable sur l'hôte Linux, où tout le reste de ce chemin est invisible.
///
/// **Elle a remplacé un budget en nombre de tentatives**, que la mesure a
/// réfuté : créer une sortie virtuelle rend `ACCESS_LOST`, la réouverture
/// réussit, et la duplication rouverte rend **aussitôt** `ACCESS_LOST` à son
/// tour tant que Windows n'a pas fini de reconfigurer sa topologie. Trois
/// tentatives sans délai étaient donc brûlées avant que le phénomène ne se
/// termine. Le compte de tentatives ne mesurait pas la bonne grandeur.
pub struct FenetreDeReprise {
    ouverte_a: Option<std::time::Instant>,
    derniere_tentative: Option<std::time::Instant>,
    tentatives: u32,
}

impl FenetreDeReprise {
    pub fn nouvelle() -> Self {
        Self { ouverte_a: None, derniere_tentative: None, tentatives: 0 }
    }

    pub fn tenter(&mut self, maintenant: std::time::Instant) -> Tentative {
        let ouverte_a = *self.ouverte_a.get_or_insert(maintenant);
        if maintenant.duration_since(ouverte_a) > DUREE_FENETRE_REPRISE {
            return Tentative::Expiree;
        }
        if let Some(derniere) = self.derniere_tentative {
            if maintenant.duration_since(derniere) < PAS_REPRISE {
                return Tentative::Patienter;
            }
        }
        self.derniere_tentative = Some(maintenant);
        self.tentatives += 1;
        Tentative::Rouvrir
    }

    /// Referme la fenêtre. Appelée sur tout succès d'acquisition, `Ok(None)`
    /// compris : dès que DXGI cesse de refuser, le remaniement est terminé.
    pub fn succes(&mut self) {
        self.ouverte_a = None;
        self.derniere_tentative = None;
        self.tentatives = 0;
    }

    pub fn tentatives(&self) -> u32 {
        self.tentatives
    }
}
```

- [ ] **Étape 4 : câbler dans `next_frame`**

Le champ `budget` de `DesktopCapture` devient `fenetre: FenetreDeReprise`, et la
boucle de `next_frame` devient :

```rust
    pub fn next_frame(
        &mut self,
        region: Rect,
    ) -> std::result::Result<Option<CapturedFrame>, EchecAcquisition> {
        match self.tenter_acquisition(region) {
            Ok(issue) => {
                self.fenetre.succes();
                Ok(issue)
            }
            Err(EchecAcquisition::AccesPerdu) => {
                // Pas de boucle interne, et c'est le point de conception :
                // cette fonction est appelée depuis la boucle de
                // `Session::run`, et y dormir plusieurs secondes suspendrait
                // du même coup les demandes de keyframe, les changements de
                // barreau de l'adaptation réseau et les redimensionnements.
                // La reprise s'étale donc sur plusieurs appels.
                match self.fenetre.tenter(std::time::Instant::now()) {
                    crate::capture_reprise::Tentative::Rouvrir => {
                        tracing::info!(
                            tentative = self.fenetre.tentatives(),
                            cible = ?self.cible,
                            "accès à la duplication perdu, réouverture"
                        );
                        if let Err(erreur) = self.rouvrir() {
                            // Un échec de réouverture n'est PAS définitif : la
                            // sortie peut n'être pas encore réapparue dans la
                            // topologie. On le dit et on laisse la fenêtre
                            // courir — c'est elle qui tranchera.
                            tracing::info!(
                                erreur = %erreur,
                                cible = ?self.cible,
                                "réouverture de la duplication échouée, la fenêtre de reprise court toujours"
                            );
                        }
                        Ok(None)
                    }
                    crate::capture_reprise::Tentative::Patienter => Ok(None),
                    crate::capture_reprise::Tentative::Expiree => {
                        Err(EchecAcquisition::AccesPerdu)
                    }
                }
            }
            Err(panne) => Err(panne),
        }
    }
```

⚠️ **Le `Display` de `EchecAcquisition` mentionne `REPRISES_MAX`** : il doit
citer `DUREE_FENETRE_REPRISE` à la place, sous une formulation qui parle de
durée et non de tentatives.

⚠️ **Un échec de `rouvrir` ne sort plus de la boucle.** C'est un changement de
comportement voulu et mesuré : la tâche 6 a relevé que la réouverture peut
légitimement échouer le temps que la sortie réapparaisse dans la topologie.
Seule l'expiration de la fenêtre est définitive.

- [ ] **Étape 5 : vérifier l'hôte, puis compiler sur la VM**

Run: `cargo test --manifest-path agent/Cargo.toml`
Expected: 258 tests passés (254 − 3 de `BudgetReprises` + 7 de `FenetreDeReprise`).

```bash
git add agent/src/capture/reprise.rs agent/src/capture.rs
scripts/build-agent.sh
```
Expected: compilation sans erreur.

- [ ] **Étape 6 : commit**

```bash
git add agent/src/capture/reprise.rs agent/src/capture.rs
git commit -m "fix(d2): la reprise se borne en DUREE, et ne bloque plus la boucle de session

Le point d'arret de la tache 6 a refute le CALIBRAGE sans refuter la voie : 7
sondes post-mortem sur 7 rapportent que la sortie se rouvre et rend une image
une fois la topologie stabilisee. La chaine relevee : creation de sortie ->
ACCESS_LOST -> rouvrir() REUSSIT -> la duplication rouverte rend aussitot
ACCESS_LOST a son tour -> les 3 tentatives sans delai sont brulees en 14 a
21 ms, la ou le depot admet 3 s pour qu'une topologie se stabilise.

Le budget comptait des TENTATIVES la ou le phenomene a une DUREE. Il devient
une fenetre de 8 s, avec un pas de 150 ms entre deux reouvertures.

Non bloquante, et c'est le point : next_frame rend « rien de neuf » pendant la
fenetre au lieu de dormir. L'y bloquer aurait suspendu les demandes de
keyframe, l'adaptation reseau et les redimensionnements, next_frame etant
appelee depuis la boucle de Session::run.

Un echec de rouvrir n'est plus definitif : la sortie peut n'etre pas encore
reapparue dans la topologie. Seule l'expiration de la fenetre l'est.

La spec §3.4 est reecrite : le defaut etait dans la conception.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 6 ter : rejouer la mesure du point d'arrêt

Rejouer **intégralement** le protocole de la tâche 6 — mêmes rangs 1, 2 et 4,
même hygiène de topologie depuis un processus neuf, mêmes clés de lecture — sur
le binaire recalibré.

Les journaux sont versés sous `reprise-recalibree-n{1,2,4}.log`, **à côté** des
premiers et sans les remplacer : les deux séries se lisent ensemble, et la
première est la preuve du défaut que la seconde corrige.

Le critère de réception est **inchangé, et il n'est pas déplacé après coup** :
aux trois rangs, chaque voie rend encore des images après la perturbation, aucun
verdict faux après perturbation, et au moins une reprise observée.

⚠️ **Un chiffre neuf à relever, parce qu'il n'existait pas avant** : le nombre
de tentatives de réouverture réellement consommées par voie, et le délai entre
la perturbation et la première image de la passe B. C'est ce dernier qui dira si
la fenêtre de 8 s est très surdimensionnée ou juste suffisante — sans le
mesurer, on ne saura toujours pas où est le seuil.

---

## Tâche 6 quater : le témoin qui départage — relâcher avant d'acquérir

> **Tâche née de la seconde réfutation.** La tâche 6 ter a montré que la fenêtre
> de 8 s est consommée entière — 54 tentatives par voie, le maximum théorique —
> sans qu'une seule acquisition passe, aux trois rangs, avec **0 échec de
> `rouvrir()` sur 378 tentatives**. Le délai est donc écarté. Trois variables
> séparaient encore la reprise (qui échoue) de la sonde post-mortem (qui
> réussit 7/7). Cette tâche en isole une.

### Ce que la relecture du code a trouvé, et pourquoi c'est le suspect n°1

`agent/src/capture.rs:235` appelle `dupliquer(&self.device, &output)` **alors que
`self.duplication` détient encore l'ancienne duplication** : elle n'est remplacée
qu'à la ligne 240, donc relâchée seulement à cet instant.

Or **DXGI n'autorise qu'une duplication par sortie** — le dépôt le sait et
`CLAUDE.md` le porte noir sur blanc : « *DXGI n'autorise qu'UNE seule duplication
ouverte par sortie — exactement une, pas « un nombre très limité ». Un
`DesktopCapture` provisoire laissé en vie fait échouer la suivante.* »

On demande donc une seconde duplication de la même sortie sans avoir relâché la
première. Le relevé est cohérent avec cette lecture : l'appel **réussit** (378
sur 378) et ce qu'il rend est **mort-né**.

C'est une hypothèse, **pas un fait**. Mais elle a trois propriétés qui la font
passer devant celle du périphérique :

1. elle explique le relevé sans rien supposer d'autre ;
2. **relâcher avant d'acquérir est correct indépendamment du résultat** — la
   version actuelle viole une contrainte documentée de DXGI ;
3. elle ne coûte rien, là où reconstruire le périphérique obligerait à détruire
   l'encodeur qui y est lié, dont le pire cas est borné à 8 s avec un gel
   observé 1 fois sur 6.

**Files:**
- Modify: `agent/src/capture.rs` (champ `duplication`, `rouvrir`, `next_frame`)
- Modify: `agent/src/capture/ouverture.rs` si le constructeur en dépend

**Interfaces:**
- Consumes: `FenetreDeReprise`, `Tentative`, `est_acces_perdu` (inchangés).
- Produces : aucune interface publique neuve. `DesktopCapture::rouvrir` garde
  sa signature ; c'est son ordre d'opérations qui change.

- [ ] **Étape 1 : relâcher explicitement avant d'acquérir**

Le champ devient `duplication: Option<IDXGIOutputDuplication>`, seul moyen en
Rust de **relâcher** un objet COM détenu par un champ sans le remplacer par un
autre au même instant.

```rust
    /// Reconstruit la duplication après une perte d'accès, en conservant le
    /// périphérique D3D11 (voir la doc de cette méthode pour le pourquoi).
    ///
    /// **L'ancienne duplication est relâchée AVANT que la neuve ne soit
    /// demandée, et l'ordre est le fond de cette méthode.** DXGI n'autorise
    /// qu'**une** duplication par sortie. La version précédente appelait
    /// `dupliquer()` alors que `self.duplication` détenait encore l'objet
    /// périmé : l'appel réussissait — 378 fois sur 378 au relevé du
    /// 1ᵉʳ août 2026 — et rendait une duplication **mort-née**, qui refusait
    /// aussitôt toute acquisition. Huit secondes de réessais toutes les 150 ms
    /// n'en sortaient jamais.
    pub fn rouvrir(&mut self) -> Result<()> {
        // L'image détenue d'abord : `release_frame` appelle `ReleaseFrame` sur
        // la duplication qu'on s'apprête à relâcher.
        self.release_frame();

        // PUIS la duplication elle-même, et c'est cette ligne qui compte.
        // `None` la fait relâcher ici, pas à l'affectation d'après.
        self.duplication = None;

        let factory: IDXGIFactory1 =
            unsafe { CreateDXGIFactory1() }.context("création de la fabrique DXGI (réouverture)")?;
        let (_adapter, output) = ouvrir_sortie(&factory, &self.cible)
            .context("résolution de la sortie à rouvrir")?;
        let (duplication, largeur, hauteur) = dupliquer(&self.device, &output)?;

        self.duplication = Some(duplication);
        self.desktop_width = largeur;
        self.desktop_height = hauteur;
        Ok(())
    }
```

⚠️ **Le champ devenant `Option`, tous ses usages doivent le déballer.** Un
`DesktopCapture` dont la duplication vaut `None` n'existe que le temps de
`rouvrir` : un accès qui la trouve absente est un défaut de programmation, pas
un cas d'exploitation. Emploie une aide privée qui le dit :

```rust
    /// La duplication courante. Absente seulement pendant `rouvrir`, entre le
    /// relâchement et l'acquisition — état qui ne s'échappe jamais de cette
    /// méthode.
    fn duplication(&self) -> Result<&IDXGIOutputDuplication> {
        self.duplication
            .as_ref()
            .ok_or_else(|| anyhow!("duplication absente hors d'une réouverture"))
    }
```

- [ ] **Étape 2 : journaliser le HRESULT NU de l'échec d'acquisition**

**Le journal ne l'imprime nulle part**, et le rapport de la tâche 6 ter a dû
**inférer** que l'erreur était `0x887A0026` en relisant le code. Sur une mesure
qui décide de la poursuite d'un chantier, une chaîne de causes inférée ne vaut
pas une chaîne relevée.

Sur le chemin `Tentative::Rouvrir` de `next_frame`, la trace existante gagne le
code nu de l'échec qui a motivé la réouverture :

```rust
                        tracing::info!(
                            tentative = self.fenetre.tentatives(),
                            cible = ?self.cible,
                            hresult = format!("{:#010x}", code_perdu),
                            "accès à la duplication perdu, réouverture"
                        );
```

où `code_perdu` est le `i32` rendu par `e.code().0` dans `tenter_acquisition`,
remonté avec le variant. Fais-le porter par `EchecAcquisition::AccesPerdu(i32)`
plutôt que par un champ de la structure : l'information appartient à l'échec.

⚠️ **Le `Display` de `EchecAcquisition` doit alors montrer ce code lui aussi** —
c'est lui que `windows_source` journalise en déclarant la source épuisée, et
c'est la dernière ligne qu'un lecteur verra.

- [ ] **Étape 3 : vérifier l'hôte, puis compiler sur la VM**

Run: `cargo test --manifest-path agent/Cargo.toml`
Expected: 259 tests passés. Les tests de `FenetreDeReprise` sont purs et ne
voient pas ce changement ; si l'un d'eux casse, c'est que le variant a changé de
forme sans que son test suive.

```bash
git add agent/src/capture.rs agent/src/capture/ouverture.rs
scripts/build-agent.sh
```
Expected: compilation sans erreur.

- [ ] **Étape 4 : commit**

```bash
git add agent/src/capture.rs agent/src/capture/ouverture.rs
git commit -m "fix(d2): relacher la duplication AVANT d'en demander une neuve

DXGI n'autorise qu'UNE duplication par sortie. `rouvrir` appelait pourtant
`dupliquer()` alors que le champ detenait encore l'objet perime, relache
seulement a l'affectation suivante. L'appel reussissait — 378 fois sur 378 au
releve du 1er aout 2026 — et rendait une duplication MORT-NEE, qui refusait
aussitot toute acquisition : huit secondes de reessais toutes les 150 ms n'en
sortaient jamais, aux trois rangs.

C'est l'explication la plus economique des deux refutations, et elle vaut
correction independamment du resultat : acquerir sans avoir relache viole une
contrainte documentee de DXGI que ce depot porte deja dans CLAUDE.md.

Le champ devient Option, seul moyen en Rust de relacher un objet COM detenu par
un champ sans le remplacer au meme instant. L'etat None ne s'echappe jamais de
`rouvrir`.

Au passage, le HRESULT nu de l'echec d'acquisition est journalise : le rapport
de la mesure precedente a du l'INFERER en relisant le code, faute qu'il soit
ecrit nulle part.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 6 quinquies : rejouer la mesure, et lire le témoin

Rejouer **intégralement** le protocole de la tâche 6, rangs 1, 2 et 4, sur le
binaire corrigé. Journaux versés sous `reprise-relachee-n{1,2,4}.log`, **à côté**
des deux séries précédentes et sans les remplacer : les trois se lisent
ensemble.

Le critère de réception est **inchangé et non déplaçable** : aux trois rangs,
chaque voie rend encore des images après la perturbation, aucun verdict faux
après perturbation, au moins une reprise observée. Et `verdicts_faux = 0` ne
compte que **s'il y a eu des lectures** — le piège s'est réalisé deux fois.

**Ce que ce tirage doit relever, et qui n'existait dans aucun des deux
précédents :**

- **le HRESULT nu** de chaque échec d'acquisition ayant motivé une réouverture,
  désormais journalisé. Si le code n'est **pas** `0x887A0026`, tout ce qui
  précède est à relire : les deux rapports antérieurs l'avaient inféré ;
- **le nombre de tentatives par voie**. Au tirage précédent il valait 54, le
  maximum théorique, sur les sept voies. **Un nombre petit — une ou deux — est
  le signe attendu si l'hypothèse porte** ;
- **le délai entre la perturbation et la première image de la passe B**, qui
  n'a jamais pu être mesuré, faute d'image. C'est lui qui dira si la fenêtre de
  8 s est très surdimensionnée.

**Trois issues, et il faut les distinguer dans le rapport :**

1. **Reçu** — l'hypothèse du relâchement portait. Le dire, et dire du même coup
   que la piste du périphérique D3D11 devient **sans objet**, non pas réfutée.
2. **Refusé, mais les tentatives par voie ont chuté** — l'hypothèse porte en
   partie ; il reste une seconde cause, et la piste du périphérique redevient
   la suivante à départager.
3. **Refusé à l'identique** — 54 tentatives par voie de nouveau : l'hypothèse ne
   portait pas, et c'est le périphérique conservé qu'il faut mettre en cause.
   ⚠️ **Ne pas enchaîner sur cette correction sans en référer** : elle oblige à
   détruire l'encodeur lié au périphérique, dont le pire cas est borné à 8 s
   avec un gel déjà observé.

---

## Tâche 7 : appariement tolérant et attente sur condition observable

**Files:**
- Modify: `agent/src/superviseur/placement.rs` (`sortie_par_dimensions` l. 21-41)
- Modify: `agent/src/superviseur/boucle.rs` (`DELAI_RATTACHEMENT` l. 48-50, `creer_sortie` l. 251-297)

**Interfaces:**
- Consumes: `TOLERANCE_PX` (existant, `placement.rs:54`) ; `noms_attaches`, `relever_topologie`, `attendre_en_pinguant` (existants).
- Produces:
  - `pub fn sortie_par_dimensions(sorties: &[SortieDxgi], largeur: u32, hauteur: u32, deja_prises: &[String]) -> Option<SortieDxgi>` — tolérante, et `deja_prises` porte des noms (tâche 4)
  - `fn attendre_une_sortie_neuve(pilote: &PiloteParIoctl, avant: &[String], limite: Duration) -> Result<Vec<SortieDxgi>>` dans `boucle.rs`

- [ ] **Étape 1 : écrire les tests qui échouent**

Dans `agent/src/superviseur/placement.rs`, section `#[cfg(test)]` :

```rust
    /// §3.1 de la recette D1 : une sortie créée à 1280×713 a été rendue par
    /// DXGI à 1280×720 une fois, puis à 1280×713 l'essai suivant. L'égalité
    /// stricte rendait alors l'ouverture de la fenêtre impossible.
    #[test]
    fn un_ecart_dans_la_tolerance_apparie_quand_meme() {
        let sorties = vec![sortie("\\\\.\\DISPLAY7", 1280, 717)];
        let trouvee = sortie_par_dimensions(&sorties, 1280, 720, &[]);
        assert_eq!(trouvee.map(|s| s.nom_sortie), Some("\\\\.\\DISPLAY7".into()));
    }

    /// La tolérance ne doit pas avaler le facteur DPI de 1,5 que le dépôt a
    /// relevé sur une sortie virtuelle : c'est le piège que l'égalité stricte
    /// protégeait, et qu'il faut continuer de voir échouer.
    #[test]
    fn un_facteur_d_echelle_n_apparie_pas() {
        let sorties = vec![sortie("\\\\.\\DISPLAY7", 1920, 1080)];
        assert!(sortie_par_dimensions(&sorties, 1280, 720, &[]).is_none());
    }

    #[test]
    fn une_sortie_deja_prise_est_ignoree() {
        let sorties = vec![
            sortie("\\\\.\\DISPLAY7", 1280, 720),
            sortie("\\\\.\\DISPLAY8", 1280, 720),
        ];
        let trouvee = sortie_par_dimensions(&sorties, 1280, 720, &["\\\\.\\DISPLAY7".to_string()]);
        assert_eq!(trouvee.map(|s| s.nom_sortie), Some("\\\\.\\DISPLAY8".into()));
    }

    fn sortie(nom: &str, largeur: u32, hauteur: u32) -> SortieDxgi {
        SortieDxgi {
            index_adaptateur: 0,
            index_sortie: 0,
            adaptateur: "essai".into(),
            nom_sortie: nom.into(),
            attachee_au_bureau: true,
            rect: Rect { x: 0, y: 0, width: largeur, height: hauteur },
        }
    }
```

- [ ] **Étape 2 : lancer les tests pour les voir échouer**

Run: `cargo test --manifest-path agent/Cargo.toml superviseur::placement`
Expected: `un_ecart_dans_la_tolerance_apparie_quand_meme` échoue (`None` au lieu de `Some`) ; les deux autres échouent à la compilation sur le type de `deja_prises`.

- [ ] **Étape 3 : rendre l'appariement tolérant**

```rust
/// Sortie DXGI correspondant à des dimensions demandées, parmi celles qui ne
/// sont pas déjà attribuées.
///
/// **Tolérante de `TOLERANCE_PX`, et pas davantage.** L'égalité stricte était
/// le choix initial, pour ne pas masquer le facteur d'échelle décrit en tête de
/// module ; la recette D1 a montré qu'elle rendait l'ouverture impossible sur
/// une course de rattachement de quelques pixels (1280×713 rendue 1280×720).
/// La tolérance retenue est celle du replacement — quatre pixels — très loin
/// du facteur 1,5 qui reste, lui, refusé.
pub fn sortie_par_dimensions(
    sorties: &[SortieDxgi],
    largeur: u32,
    hauteur: u32,
    deja_prises: &[String],
) -> Option<SortieDxgi> {
    let proche = |a: u32, b: u32| (a as i64 - b as i64).abs() <= TOLERANCE_PX;
    sorties
        .iter()
        .find(|s| {
            s.attachee_au_bureau
                && proche(s.rect.width, largeur)
                && proche(s.rect.height, hauteur)
                && !deja_prises.contains(&s.nom_sortie)
        })
        .cloned()
}
```

`TOLERANCE_PX` doit être déclarée **avant** cette fonction dans le fichier.

- [ ] **Étape 4 : remplacer le délai plat par une attente sur condition**

Dans `boucle.rs`, remplacer la constante l. 48-50 :

```rust
/// Temps maximal laissé à Windows pour rattacher une sortie fraîchement créée.
///
/// **Une borne, pas une durée d'attente.** La version précédente dormait 1500 ms
/// plats, et la recette D1 a montré que ce n'était pas toujours assez : la
/// sortie n'était pas encore dans la topologie quand on l'y cherchait, et la
/// fenêtre ne s'ouvrait jamais. On attend désormais le FAIT — qu'une sortie
/// neuve apparaisse — et cette constante ne fait qu'empêcher d'attendre
/// indéfiniment.
const LIMITE_RATTACHEMENT: std::time::Duration = std::time::Duration::from_secs(5);

/// Pas de scrutation plus serrée : chaque tour énumère toutes les sorties DXGI,
/// ce qui n'est pas gratuit.
const PAS_RATTACHEMENT: std::time::Duration = std::time::Duration::from_millis(100);
```

Et écrire l'attente :

```rust
/// Attend qu'une sortie neuve apparaisse dans la topologie, sans cesser de
/// battre le chien de garde.
///
/// Le battement n'est pas un détail : le pilote retire les sorties d'un client
/// qui cesse de pinguer, **y compris celles qu'on vient de créer**, et l'étape
/// de ping de la boucle est hors du parcours des effets.
fn attendre_une_sortie_neuve(
    pilote: &PiloteParIoctl,
    avant: &[String],
    limite: std::time::Duration,
) -> Vec<SortieDxgi> {
    let echeance = std::time::Instant::now() + limite;
    loop {
        if let Err(erreur) = pilote.pinguer() {
            tracing::warn!(%erreur, "ping du chien de garde pendant l'attente de rattachement");
        }
        let toutes = relever_topologie("attente de rattachement").unwrap_or_default();
        let apparues: Vec<SortieDxgi> = toutes
            .iter()
            .filter(|s| s.attachee_au_bureau && !avant.contains(&s.nom_sortie))
            .cloned()
            .collect();
        if !apparues.is_empty() {
            return apparues;
        }
        if std::time::Instant::now() >= echeance {
            tracing::error!(
                limite_ms = limite.as_millis() as u64,
                "aucune sortie neuve n'est apparue dans la limite"
            );
            return Vec::new();
        }
        std::thread::sleep(PAS_RATTACHEMENT);
    }
}
```

Dans `creer_sortie`, remplacer les l. 251-273 (l'appel à `attendre_en_pinguant`, `relever_topologie` et le calcul d'`apparues`) par un unique `let apparues = attendre_une_sortie_neuve(pilote, &avant, LIMITE_RATTACHEMENT);`. Le reste — le `else` de `sortie_par_dimensions` avec son journal des candidats — est **inchangé** : il continue de couvrir le cas « des sorties sont apparues, mais aucune aux bonnes dimensions ».

`relever_topologie` étant appelée en boucle ici, employer sa variante silencieuse si elle journalise par sortie ; sinon, l'entourer d'une garde pour ne journaliser qu'au premier tour. **Ne pas laisser une boucle à 10 Hz écrire dans le journal.**

- [ ] **Étape 5 : lancer les tests pour les voir passer**

Run: `cargo test --manifest-path agent/Cargo.toml`
Expected: 259 tests passés (256 + 3).

```bash
scripts/build-agent.sh
```
Expected: compilation sans erreur.

- [ ] **Étape 6 : commit**

```bash
git add agent/src/superviseur/placement.rs agent/src/superviseur/boucle.rs
git commit -m "fix(d2): apparier une sortie fraiche a la tolerance, et attendre un fait

L'egalite stricte des dimensions rendait l'ouverture impossible des qu'une
course de rattachement decalait la sortie de quelques pixels : creee a
1280x713, rendue 1280x720 une fois puis 1280x713 l'essai suivant (recette D1
§3.1). La tolerance retenue est celle du replacement — quatre pixels —, tres
loin du facteur d'echelle de 1,5 que l'egalite stricte protegeait et qui reste
refuse, un test le fixe.

DELAI_RATTACHEMENT dormait 1500 ms plats, parfois insuffisants. On attend
desormais que la sortie APPARAISSE, sous une borne de 5 s, en continuant de
battre le chien de garde du pilote — qui retire les sorties d'un client
silencieux, y compris celle qu'on vient de creer.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 8 : le viewport annoncé est pair

**Files:**
- Create: `client/src/viewport.ts`, `client/src/viewport.test.ts`
- Modify: `client/src/main.ts` (l. 38-47)

**Interfaces:**
- Consumes: rien.
- Produces: `export function viewportPair(largeur: number, hauteur: number): { largeur: number; hauteur: number }`

- [ ] **Étape 1 : écrire le test qui échoue**

`client/src/viewport.test.ts` :

```typescript
import { describe, expect, it } from 'vitest';
import { viewportPair } from './viewport';

describe('viewportPair', () => {
    it('laisse des dimensions déjà paires intactes', () => {
        expect(viewportPair(1280, 720)).toEqual({ largeur: 1280, hauteur: 720 });
    });

    // Le cas banal : un pop-up de navigateur annonce couramment une hauteur
    // impaire. 1280×713 a été mesuré en recette D1, et rendait l'ouverture de
    // la fenêtre impossible — l'agent aligne ses régions en pair pour NV12,
    // la taille demandée ne pouvait donc jamais égaler celle obtenue.
    it('arrondit vers le bas des dimensions impaires', () => {
        expect(viewportPair(1281, 713)).toEqual({ largeur: 1280, hauteur: 712 });
    });

    it('arrondit les fractions avant de rendre pair', () => {
        expect(viewportPair(1280.6, 713.4)).toEqual({ largeur: 1280, hauteur: 712 });
    });

    // Une sortie virtuelle de dimension nulle n'est pas capturable : mieux vaut
    // un plancher qu'un moniteur 0×0 dont le défaut se manifesterait bien plus
    // loin, sur la chaîne d'échange d'une mire ou sur le facteur d'échelle.
    it('ne descend jamais sous un plancher de deux pixels', () => {
        expect(viewportPair(1, 0)).toEqual({ largeur: 2, hauteur: 2 });
    });
});
```

- [ ] **Étape 2 : lancer le test pour le voir échouer**

Run: `cd client && npx vitest run src/viewport.test.ts`
Expected: échec — `./viewport` n'existe pas.

- [ ] **Étape 3 : écrire l'implémentation**

`client/src/viewport.ts` :

```typescript
/**
 * Arrondit un viewport à des dimensions paires, plancher à 2.
 *
 * C'est cette taille qui décide de la résolution de la sortie virtuelle créée
 * côté agent. Or l'agent aligne ses régions de capture sur des valeurs paires
 * — l'encodeur NV12 l'exige — et apparie la sortie créée à la taille demandée.
 * Une hauteur impaire rendait donc l'appariement impossible, et la fenêtre ne
 * s'ouvrait jamais (recette D1 §3.1, 1280×713 mesuré sur un pop-up Chrome).
 *
 * Arrondi vers le BAS : agrandir demanderait une sortie plus grande que la
 * zone où l'image sera affichée, donc une image rognée.
 */
export function viewportPair(
    largeur: number,
    hauteur: number,
): { largeur: number; hauteur: number } {
    const pair = (valeur: number) => Math.max(2, Math.round(valeur) & ~1);
    return { largeur: pair(largeur), hauteur: pair(hauteur) };
}
```

- [ ] **Étape 4 : lancer les tests pour les voir passer**

Run: `cd client && npm test`
Expected: la suite entière passe, dont les 4 tests neufs.

Run: `cd client && npm run typecheck`
Expected: aucune erreur.

- [ ] **Étape 5 : câbler dans `main.ts`**

```typescript
if (window.opener && !window.opener.closed) {
    const { largeur, hauteur } = viewportPair(window.innerWidth, window.innerHeight);
    window.opener.postMessage(
        { type: 'viewport', session: sessionId, largeur, hauteur },
        window.location.origin,
    );
}
```

avec l'import correspondant en tête de fichier.

- [ ] **Étape 6 : commit**

```bash
git add client/src/viewport.ts client/src/viewport.test.ts client/src/main.ts
git commit -m "fix(d2): annoncer un viewport aux dimensions paires

Un pop-up de navigateur annonce couramment une hauteur impaire — 1280x713
mesure en recette D1. L'agent aligne ses regions en pair (NV12 l'exige) et
apparie la sortie creee a la taille demandee : la fenetre ne pouvait alors
jamais s'ouvrir. Arrondi vers le bas, plancher a deux pixels.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 9 : donner le premier plan avant d'injecter du clavier

**Files:**
- Modify: `agent/src/input.rs` (branche `InputMessage::Key`, l. 110-120)

**Interfaces:**
- Consumes: `InputInjector::hwnd` (existant).
- Produces: rien de public. Comportement seul.

> Cette tâche est une **tentative mesurée**, pas une garantie. `SendInput` reste global à la session Windows : deux fenêtres qui reçoivent des frappes simultanées se disputeront le premier plan. La limite est déclarée, pas résolue.

- [ ] **Étape 1 : implémenter**

Dans `agent/src/input.rs`, ajouter à `InputInjector` :

```rust
        /// Porte la fenêtre de cette session au premier plan avant d'injecter
        /// du clavier.
        ///
        /// **Nécessaire et probablement pas suffisant.** `SendInput` est global
        /// à la session Windows : il n'adresse personne, il alimente la file
        /// d'entrée de la fenêtre active. Sans cet appel, toutes les sessions
        /// tapent dans la même fenêtre — celle qui se trouve au premier plan.
        /// Avec, deux sessions qui tapent en même temps se le disputent. La
        /// réponse structurelle est ailleurs (injection ciblée par messages, ou
        /// un pilote) et reste hors périmètre du sous-bloc D2.
        ///
        /// **Le retour est vérifié.** `SetForegroundWindow` échoue
        /// silencieusement quand le processus appelant n'a pas le droit de
        /// voler le focus : sans cette trace, on ne saurait pas distinguer « le
        /// premier plan n'a pas suffi » de « le premier plan n'a jamais été
        /// donné ». Journalisé une fois par basculement et non par frappe — un
        /// journal par touche noierait le canal.
        fn au_premier_plan(&mut self) {
            if unsafe { GetForegroundWindow() } == self.hwnd {
                return;
            }
            let obtenu = unsafe { SetForegroundWindow(self.hwnd) }.as_bool();
            if obtenu != self.premier_plan_obtenu {
                if obtenu {
                    tracing::info!(hwnd = ?self.hwnd, "premier plan obtenu avant injection clavier");
                } else {
                    tracing::warn!(
                        hwnd = ?self.hwnd,
                        "SetForegroundWindow refusé — le clavier ira à la fenêtre active"
                    );
                }
                self.premier_plan_obtenu = obtenu;
            }
        }
```

Ajouter le champ `premier_plan_obtenu: bool` à la structure (initialisé à `false` dans `new`), les imports `GetForegroundWindow` et `SetForegroundWindow`, et appeler `self.au_premier_plan();` en tête de la branche `InputMessage::Key`.

- [ ] **Étape 2 : vérifier l'hôte et compiler sur la VM**

Run: `cargo test --manifest-path agent/Cargo.toml`
Expected: 259 tests passés (`input.rs` est `#[cfg(windows)]`, aucun test neuf).

```bash
scripts/build-agent.sh
```
Expected: compilation sans erreur.

- [ ] **Étape 3 : commit**

```bash
git add agent/src/input.rs
git commit -m "feat(d2): porter la fenetre au premier plan avant d'injecter du clavier

SendInput est global a la session Windows : il n'adresse personne, il alimente
la file d'entree de la fenetre ACTIVE. En multi-fenetres, toutes les sessions
tapaient donc dans la meme. C'est le second suspect de l'injection clavier non
demontree du sous-bloc D1, et le seul qui soit structurel.

Necessaire, probablement pas suffisant : deux sessions qui tapent en meme temps
se disputeront le premier plan. Le retour de SetForegroundWindow est verifie et
journalise au basculement — sans quoi on ne saurait pas distinguer « le premier
plan n'a pas suffi » de « il n'a jamais ete donne ».

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 10 : une fenêtre vivante n'est plus oubliée

**Files:**
- Modify: `agent/src/superviseur/table.rs` (`Etat`, `Entree`, `enfant_mort`, `fenetre_apparue`)
- Modify: `agent/src/superviseur/table/tests.rs`
- Modify: `agent/src/superviseur/boucle.rs` (contrôle périodique)

**Interfaces:**
- Consumes: `Table` (tâche 4).
- Produces:
  - `Etat::SansSession` — nouvelle variante
  - `pub const RELANCES_MAX: u32 = 3` dans `table.rs`
  - `pub fn Table::relancer_les_orphelines(&mut self) -> Vec<Effet>`

- [ ] **Étape 1 : écrire les tests qui échouent**

Dans `agent/src/superviseur/table/tests.rs` :

```rust
/// Le second défaut de conception du §3.3 de D1 : `enfant_mort` retirait
/// l'entrée, et plus rien ne rappelait la fenêtre — sauf un `SHOW` fortuit de
/// Windows. Une fenêtre bien vivante disparaissait de la shell pour toujours,
/// et c'est ce qui laissait la page-shell vide alors que les quatre
/// applications tournaient encore.
#[test]
fn une_fenetre_dont_l_enfant_meurt_est_reproposee() {
    let mut t = Table::nouvelle(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("ouverture attendue, reçu {effets:?}");
    };
    let session = session.clone();
    t.viewport_recu(&session, 1280, 720);
    t.sortie_creee(&session, 42, "\\\\.\\DISPLAY7".into());

    let effets = t.enfant_mort(&session);
    assert!(
        effets.contains(&Effet::DetruireSortie {
            sortie_pilote: 42,
            nom_sortie: "\\\\.\\DISPLAY7".into()
        }),
        "la sortie doit toujours être rendue au pilote"
    );

    // La fenêtre, elle, n'est pas oubliée : le contrôle périodique la
    // repropose sous une session NEUVE.
    let effets = t.relancer_les_orphelines();
    let Some(Effet::AnnoncerOuverture { session: neuve, titre }) = effets.first() else {
        panic!("réouverture attendue, reçu {effets:?}");
    };
    assert_ne!(*neuve, session, "un identifiant réutilisé apparierait un message tardif");
    assert_eq!(titre, "Bloc-notes");
}

/// Le garde-fou que l'emballement de D1 rend obligatoire : sans lui, une
/// fenêtre dont l'enfant meurt systématiquement produit la boucle
/// `w-5, w-6, w-7, w-8…` observée en recette.
#[test]
fn une_fenetre_qui_echoue_sans_fin_finit_par_etre_abandonnee() {
    let mut t = Table::nouvelle(4);
    let mut effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    for _ in 0..RELANCES_MAX {
        let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
            panic!("ouverture attendue, reçu {effets:?}");
        };
        let session = session.clone();
        t.enfant_mort(&session);
        effets = t.relancer_les_orphelines();
    }
    assert!(
        matches!(effets.first(), Some(Effet::AnnoncerRefus { titre, .. }) if titre == "Bloc-notes"),
        "au-delà du plafond, un refus annoncé et non une relance de plus, reçu {effets:?}"
    );
    assert!(
        t.relancer_les_orphelines().is_empty(),
        "une fenêtre abandonnée ne doit plus rien produire"
    );
}

/// Une fenêtre qui se ferme pour de bon quitte la table, orpheline ou non :
/// sans quoi `relancer_les_orphelines` la ressusciterait indéfiniment.
#[test]
fn une_fenetre_orpheline_qui_se_ferme_quitte_la_table() {
    let mut t = Table::nouvelle(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("ouverture attendue, reçu {effets:?}");
    };
    t.enfant_mort(&session.clone());
    t.fenetre_disparue(IdFenetre(1));
    assert!(t.relancer_les_orphelines().is_empty());
}
```

- [ ] **Étape 2 : lancer les tests pour les voir échouer**

Run: `cargo test --manifest-path agent/Cargo.toml superviseur::table`
Expected: échec de compilation — `relancer_les_orphelines` et `RELANCES_MAX` n'existent pas.

- [ ] **Étape 3 : implémenter**

Ajouter à `Etat` :

```rust
    /// L'enfant est mort et la sortie a été rendue, mais **la fenêtre Windows
    /// est toujours là**. Le contrôle périodique la reproposera.
    ///
    /// Sans cet état, `enfant_mort` retirait purement l'entrée : plus rien ne
    /// rappelait la fenêtre sauf un `SHOW` fortuit de Windows, et la shell
    /// restait vide devant des applications bien vivantes (recette D1 §3.3).
    SansSession,
```

Ajouter le plafond et le compteur :

```rust
/// Relances tolérées pour une même fenêtre avant abandon.
///
/// Le garde-fou de l'emballement relevé en recette D1 : une fenêtre dont
/// l'enfant meurt systématiquement produirait sinon `w-5, w-6, w-7, w-8…`
/// jusqu'à épuiser le vivier de sorties du pilote.
pub const RELANCES_MAX: u32 = 3;
```

`Entree` gagne `relances: u32`. `enfant_mort` bascule l'entrée en `SansSession` — en **rendant toujours** `DetruireSortie` et `AnnoncerFermeture`, et en effaçant `sortie_pilote` et `nom_sortie` pour qu'une seconde mort ne demande pas deux fois la même destruction — au lieu de `self.entrees.remove(session)`.

⚠️ L'entrée changeant d'identifiant de session à chaque relance, `relancer_les_orphelines` **retire** l'entrée `SansSession` et en réinsère une sous une session neuve, en reportant `relances + 1` et `audio`. Le compteur `self.compteur` continue de croître sans jamais reculer.

```rust
    /// Repropose les fenêtres dont la session est morte mais qui existent
    /// toujours côté Windows. Appelée par le contrôle périodique du
    /// superviseur.
    pub fn relancer_les_orphelines(&mut self) -> Vec<Effet> {
        let orphelines: Vec<IdSession> = self
            .entrees
            .iter()
            .filter(|(_, e)| e.etat == Etat::SansSession)
            .map(|(s, _)| s.clone())
            .collect();
        let mut effets = Vec::new();
        for ancienne in orphelines {
            let entree = self.entrees.remove(&ancienne).expect("relevée à l'instant");
            if entree.relances >= RELANCES_MAX {
                effets.push(Effet::AnnoncerRefus {
                    titre: entree.titre,
                    motif: format!("la session n'a pas tenu après {RELANCES_MAX} tentatives"),
                });
                continue;
            }
            self.compteur += 1;
            let session = IdSession(format!("w-{}", self.compteur));
            self.entrees.insert(
                session.clone(),
                Entree {
                    fenetre: entree.fenetre,
                    titre: entree.titre.clone(),
                    etat: Etat::AttendLeViewport,
                    sortie_pilote: None,
                    nom_sortie: None,
                    audio: entree.audio,
                    relances: entree.relances + 1,
                },
            );
            effets.push(Effet::AnnoncerOuverture { session, titre: entree.titre });
        }
        effets
    }
```

`fenetre_apparue` : la garde d'idempotence (l. 139-141) trouve déjà les entrées `SansSession` par leur `fenetre`, ce qui est le comportement voulu — un `SHOW` fortuit ne doit pas doubler une entrée en attente de relance. **Ne pas la modifier.**

Dans `boucle.rs`, à l'étape 6 (contrôle périodique, l. 178-181) :

```rust
            effets.extend(table.relancer_les_orphelines());
```

- [ ] **Étape 4 : lancer les tests pour les voir passer**

Run: `cargo test --manifest-path agent/Cargo.toml`
Expected: 262 tests passés (259 + 3).

```bash
wc -l agent/src/superviseur/table.rs
```
Expected: ≤ 500. Au-delà, extraire — le fichier était à 289 lignes.

```bash
scripts/build-agent.sh
```
Expected: compilation sans erreur.

- [ ] **Étape 5 : commit**

```bash
git add agent/src/superviseur/table.rs agent/src/superviseur/table/tests.rs \
        agent/src/superviseur/boucle.rs
git commit -m "fix(d2): ne plus oublier une fenetre dont la session est morte

enfant_mort retirait l'entree de la table, et plus rien ne rappelait la fenetre
sauf un SHOW fortuit de Windows : une application bien vivante disparaissait de
la page-shell pour toujours. C'est ce qui laissait la shell vide devant quatre
applications ouvertes, en recette D1.

L'entree bascule desormais en SansSession et le controle periodique la
repropose. Avec le garde-fou que l'emballement de D1 rend obligatoire : au-dela
de trois relances, un refus annonce plutot qu'une relance de plus — sans quoi
une fenetre qui echoue sans fin epuiserait le vivier de sorties du pilote.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 11 : recette étape 2 — la démonstration bout en bout

**Files:**
- Create: `docs/superpowers/plans/journaux-multifenetres-d2/demonstration-*.log`, captures d'écran, et l'instrument employé.

**Interfaces:**
- Consumes: tout ce qui précède.
- Produces: le verdict de réception du sous-bloc.

- [ ] **Étape 1 : préparer l'environnement, dans cet ordre exact**

```bash
virsh list --all                      # démarrer si besoin, attendre `ls /media/vm/dev`
set -a && source .env && set +a
node scripts/winrm.js 'Get-Process agent -ErrorAction SilentlyContinue'   # doit être vide
MULTIFENETRE_DXGI=1 scripts/run-agent.sh                                  # topologie propre ?
```

Vérifier que le signaling tourne **avec** son environnement — vérifier le processus qui écoute réellement, pas celui qu'on croit avoir lancé :

```bash
P=$(ss -ltnp | grep ':8080 ' | grep -o 'pid=[0-9]*' | cut -d= -f2 | head -1)
tr '\0' '\n' < /proc/$P/environ | grep ^TURN_URL=
```

**Lancer le navigateur AVANT le superviseur** — le signaling ne mémorise que les offres SDP, et les annonces `fenetre-ouverte` émises avant que la page-shell soit connectée sont perdues sans trace. Chrome avec `--disable-popup-blocking`, `--disable-background-timer-throttling`, `--disable-backgrounding-occluded-windows`, `--disable-renderer-backgrounding`, et **vérifier qu'on parle bien à cette instance-là** : un port de débogage qui répond ne prouve pas que c'est le bon navigateur.

- [ ] **Étape 2 : jouer la séquence de réception**

Démarrer le superviseur (`SUPERVISEUR=1 scripts/run-agent.sh`) avec *k*=1 fenêtre préexistante, puis **ouvrir une application supplémentaire par WinRM**, une à la fois, jusqu'à cinq fenêtres. Entre chaque ouverture, laisser au moins 10 s.

⚠️ **Aucune capture d'écran CDP pendant la séquence.** En D1, c'est elle qui déclenchait l'effondrement : elle provoque un `Resize`, qui rétrécit la fenêtre Windows, qui engendre un `SHOW`, qui crée une session, qui crée une sortie. **L'instrument détruisait ce qu'il mesurait.** Captures à la fin seulement.

- [ ] **Étape 3 : lire le verdict**

```bash
J=docs/superpowers/plans/journaux-multifenetres-d2/demonstration.log
sed 's/\x1b\[[0-9;]*m//g' "$J" > "$J.plat"
grep -c "clôture de session amorcée" "$J.plat"
grep -c "accès à la duplication perdu, réouverture" "$J.plat"
grep -c "0x887A0026" "$J.plat"
```

**Reçu si** : aucune `clôture de session amorcée` non sollicitée — c'est-à-dire aucune qui ne corresponde à une fenêtre réellement fermée — sur toute la séquence, et cinq fenêtres navigateur affichant chacune son application à la fin.

Contrôler ensuite, **depuis un processus neuf**, que la topologie est revenue à son état de départ **nom pour nom** (`MULTIFENETRE_DXGI=1`), et que la VM n'a pas hiberné :

```bash
virsh list --all
grep -E "terminating on signal|shutting down" /var/log/libvirt/qemu/Windows.log | tail -4
```

- [ ] **Étape 4 : verser les pièces et committer**

Verser les journaux, les captures d'écran, **et l'instrument dans son état final** — celui de la dernière exécution, comme D1 l'a fait pour `pilote-recette.mjs`.

```bash
git add docs/superpowers/plans/journaux-multifenetres-d2/
git commit -m "recette(d2): etape 2 — la demonstration bout en bout

<Remplacer par le verdict REELLEMENT obtenu. Nombre de fenetres atteint,
cloture de sessions non sollicitees, reprises observees, etat de la topologie
au controle depuis un processus neuf. Ne rien affirmer au-dela du releve.>

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 11 bis : donner une reprise à l'ouverture initiale de la duplication

> **Tâche née de la démonstration.** La tâche 11 a démontré la réparation du
> défaut central de D1 en conditions de produit — 44 réouvertures, aucune
> session perdue — et révélé au passage un défaut que le banc ne pouvait pas
> voir : **l'ouverture initiale d'une duplication n'a aucune reprise**, alors
> que la reprise en cours de capture, elle, en a une.

### Ce que le relevé montre

Quand un enfant démarre pendant qu'une autre fenêtre s'ouvre, sa toute première
`DuplicateOutput` tombe en pleine reconfiguration de topologie et échoue. Il
meurt aussitôt. Le superviseur le compense en le relançant — ce qui **détruit
puis recrée sa sortie virtuelle**, donc abandonne à nouveau le mutex de toutes
les duplications déjà ouvertes.

**32 des 44 réouvertures du passage viennent de cette seule étape ratée.** La
compensation coûte plus qu'elle ne répare : elle transforme un échec d'ouverture
en perturbation pour les voisines.

Le HRESULT le dit lui-même : `DXGI_ERROR_NOT_CURRENTLY_AVAILABLE` signifie que
la ressource « pourra l'être ultérieurement ». On ne l'écoute pas.

⚠️ **Cette tâche ne lèvera probablement PAS le plafond de quatre fenêtres.** La
5ᵉ duplication est refusée alors que quatre sont tenues, et fermer une fenêtre
puis en rouvrir une réussit : cela désigne une limite de **concurrence**, que
nulle patience ne franchit. L'objet de cette tâche est de supprimer les
perturbations inutiles, pas de gagner une fenêtre. **Ne pas présenter le
résultat comme une tentative de lever le plafond.**

**Files:**
- Modify: `agent/src/capture.rs` (`ouvrir`)
- Modify: `agent/src/capture/reprise.rs` (constantes de l'ouverture)
- Test: `agent/src/capture/reprise.rs`

**Interfaces:**
- Consumes: `FenetreDeReprise`, `Tentative`, `est_acces_perdu`.
- Produces :
  - `pub const NON_DISPONIBLE: i32` = `0x887A0022u32 as i32` (`DXGI_ERROR_NOT_CURRENTLY_AVAILABLE`)
  - `pub const DUREE_FENETRE_OUVERTURE: std::time::Duration` = 3 s
  - `pub fn est_ouverture_retentable(code: i32) -> bool`

### Deux différences avec la reprise en cours de capture, à ne pas confondre

1. **Ici, bloquer est LÉGITIME.** `ouvrir` court au démarrage du processus
   enfant, avant toute boucle de capture : il n'y a ni keyframe à servir, ni
   adaptation réseau à traiter, ni redimensionnement en attente. On peut donc
   dormir entre deux tentatives, là où `next_frame` ne le pouvait pas. C'est
   l'inverse de l'arbitrage du §3.4 de la spec, et pour une raison qui tient au
   contexte d'appel, pas au goût.
2. **La fenêtre est plus courte** — 3 s contre 8 — parce qu'un échec durable
   doit se lire vite : quand la vraie cause est un plafond de concurrence,
   patienter huit secondes ne fait que retarder un diagnostic sans rien changer
   au résultat.

- [ ] **Étape 1 : écrire les tests qui échouent**

Dans `agent/src/capture/reprise.rs` :

```rust
    /// `DXGI_ERROR_NOT_CURRENTLY_AVAILABLE` dit dans son propre libellé que la
    /// ressource « pourra l'être ultérieurement ». C'est ce que rend une
    /// `DuplicateOutput` tentée pendant que Windows reconfigure sa topologie —
    /// le cas nominal quand une autre fenêtre s'ouvre au même instant.
    #[test]
    fn une_ouverture_est_retentable_sur_indisponibilite_ou_perte_d_acces() {
        assert!(est_ouverture_retentable(NON_DISPONIBLE));
        assert!(est_ouverture_retentable(ACCES_PERDU));
    }

    /// Un périphérique perdu ne reviendra pas, et un argument invalide n'est
    /// pas une question de patience : les retenter ne ferait que retarder le
    /// diagnostic de trois secondes.
    #[test]
    fn une_ouverture_n_est_pas_retentable_sur_une_panne_franche() {
        assert!(!est_ouverture_retentable(DEVICE_REMOVED));
        assert!(!est_ouverture_retentable(0x80070057u32 as i32), "E_INVALIDARG");
        assert!(!est_ouverture_retentable(0), "S_OK");
    }

    /// La fenêtre d'ouverture est plus COURTE que celle de la capture, et c'est
    /// délibéré : un échec durable à l'ouverture doit se lire vite, la vraie
    /// cause pouvant être un plafond de concurrence que nulle patience ne
    /// franchit.
    #[test]
    fn la_fenetre_d_ouverture_est_plus_courte_que_celle_de_la_capture() {
        assert!(DUREE_FENETRE_OUVERTURE < DUREE_FENETRE_REPRISE);
        assert!(DUREE_FENETRE_OUVERTURE >= std::time::Duration::from_secs(2));
    }
```

- [ ] **Étape 2 : lancer les tests pour les voir échouer**

Run: `cargo test --manifest-path agent/Cargo.toml capture_reprise`
Expected: échec de compilation — `NON_DISPONIBLE`, `DUREE_FENETRE_OUVERTURE` et `est_ouverture_retentable` n'existent pas.

- [ ] **Étape 3 : écrire l'implémentation**

```rust
/// `DXGI_ERROR_NOT_CURRENTLY_AVAILABLE`. Rendu par `DuplicateOutput` quand la
/// sortie ne peut pas être dupliquée **à cet instant**. Deux causes très
/// différentes se présentent sous ce même code, et le code ne les distingue
/// pas : une reconfiguration de topologie en cours — passagère —, et un plafond
/// de duplications concurrentes — durable. C'est pourquoi la fenêtre de
/// réessai est courte et son abandon bruyant.
pub const NON_DISPONIBLE: i32 = 0x887A0022u32 as i32;

/// Durée pendant laquelle l'ouverture d'une duplication est retentée.
///
/// **Plus courte que `DUREE_FENETRE_REPRISE`**, et pour une raison de
/// diagnostic : quand la cause est un plafond de concurrence, patienter
/// davantage ne change pas le résultat et retarde la lecture. Trois secondes
/// couvrent la reconfiguration de topologie que le dépôt admet par ailleurs
/// (`DELAI_TOPOLOGIE`).
///
/// **Majorante et non calibrée**, comme `DUREE_FENETRE_REPRISE`.
pub const DUREE_FENETRE_OUVERTURE: std::time::Duration = std::time::Duration::from_secs(3);

/// Vrai si un échec d'ouverture de duplication mérite d'être retenté.
pub fn est_ouverture_retentable(code: i32) -> bool {
    code == NON_DISPONIBLE || code == ACCES_PERDU
}
```

Dans `agent/src/capture.rs`, la construction retente l'appel à `dupliquer`, et
**lui seul** — pas la création du périphérique, ni la résolution de la sortie,
qui n'ont aucune raison d'être transitoires :

```rust
        // Retenter la SEULE duplication, et sur place.
        //
        // Bloquer est légitime ici, à la différence de `next_frame` : `ouvrir`
        // court au démarrage du processus enfant, avant toute boucle de
        // capture — ni keyframe à servir, ni adaptation réseau, ni
        // redimensionnement en attente.
        //
        // Sans ce réessai, une première `DuplicateOutput` tombée pendant que
        // Windows reconfigure sa topologie tuait l'enfant, que le superviseur
        // relançait en DÉTRUISANT puis RECRÉANT sa sortie — abandonnant du même
        // coup le mutex de toutes les duplications déjà ouvertes. La
        // compensation coûtait plus que la panne : 32 des 44 réouvertures du
        // relevé du 1ᵉʳ août 2026 venaient de cette seule étape.
        let debut = std::time::Instant::now();
        let (duplication, desktop_width, desktop_height) = loop {
            match dupliquer(&device, &output) {
                Ok(rendu) => break rendu,
                Err(erreur) => {
                    let code = erreur
                        .downcast_ref::<windows::core::Error>()
                        .map(|e| e.code().0);
                    let retentable =
                        code.is_some_and(crate::capture_reprise::est_ouverture_retentable);
                    if !retentable
                        || debut.elapsed() >= crate::capture_reprise::DUREE_FENETRE_OUVERTURE
                    {
                        // Bruyant à dessein : c'est ici que se lit un plafond
                        // de duplications concurrentes, indiscernable d'une
                        // reconfiguration par le seul HRESULT.
                        tracing::error!(
                            hresult = code.map(|c| format!("{c:#010x}")),
                            attendu_ms = debut.elapsed().as_millis() as u64,
                            cible = ?cible,
                            "ouverture de la duplication abandonnée"
                        );
                        return Err(erreur);
                    }
                    tracing::info!(
                        hresult = code.map(|c| format!("{c:#010x}")),
                        cible = ?cible,
                        "duplication indisponible à l'ouverture, nouvel essai"
                    );
                    std::thread::sleep(crate::capture_reprise::PAS_REPRISE);
                }
            }
        };
```

⚠️ **`dupliquer` rend une `anyhow::Error`** issue d'un `.context(…)` sur une
`windows::core::Error`. Le `downcast_ref` ci-dessus en dépend : si le code ne
peut pas être lu, `retentable` vaut `false` et l'ouverture échoue sans réessai —
**dégradation sûre, jamais une boucle**. Vérifie que le `downcast_ref` fonctionne
réellement sur la chaîne d'erreur telle qu'elle est construite ; s'il rend
toujours `None`, le réessai ne se déclencherait jamais et la tâche serait
inopérante **sans que rien ne le signale**. Dis dans ton rapport comment tu t'en
es assuré.

- [ ] **Étape 4 : vérifier l'hôte, puis compiler sur la VM**

Run: `cargo test --manifest-path agent/Cargo.toml`
Expected: 268 + 3 = **271** tests passés.

```bash
git add agent/src/capture.rs agent/src/capture/reprise.rs
scripts/build-agent.sh
```

- [ ] **Étape 5 : une exécution de vérification, courte**

Rejouer **une** montée jusqu'à quatre fenêtres, dans les conditions de la
tâche 11 (navigateur avant superviseur, `--disable-popup-blocking`, aucune
capture CDP pendant la séquence, `Get-Process agent` vérifié avant).

**Ce qui est attendu, et qui est le seul objet de cette exécution** : le nombre
de lignes `accès à la duplication perdu, réouverture` **chute nettement** — les
32 imputables aux ouvertures ratées disparaissent —, et **aucune session ne
meurt**. Relever aussi les lignes `duplication indisponible à l'ouverture,
nouvel essai` : elles sont la preuve que le réessai travaille.

⚠️ **Le plafond de quatre reste attendu.** S'il tombe encore en `0x887A0022`
après trois secondes de réessais, c'est la confirmation qu'il s'agit d'une
limite de concurrence et non d'un transitoire — **un résultat, pas un échec**.

Journal versé sous `docs/superpowers/plans/journaux-multifenetres-d2/demonstration-ouverture-retentee.log`.

- [ ] **Étape 6 : commit**

```bash
git add agent/src/capture.rs agent/src/capture/reprise.rs \
        docs/superpowers/plans/journaux-multifenetres-d2/
git commit -m "fix(d2): retenter l'ouverture initiale d'une duplication

La reprise en cours de capture existait ; l'ouverture INITIALE, non. Un enfant
qui demarrait pendant qu'une autre fenetre s'ouvrait voyait sa premiere
DuplicateOutput tomber en pleine reconfiguration de topologie, et mourait. Le
superviseur le compensait en le relancant, ce qui DETRUISAIT puis RECREAIT sa
sortie virtuelle — abandonnant a nouveau le mutex de toutes les duplications
deja ouvertes.

La compensation coutait plus que la panne : 32 des 44 reouvertures du releve du
1er aout 2026 venaient de cette seule etape.

DXGI_ERROR_NOT_CURRENTLY_AVAILABLE dit lui-meme que la ressource « pourra
l'etre ulterieurement ». On l'ecoute desormais, trois secondes durant.

Bloquer est legitime ici, a la difference de next_frame : `ouvrir` court au
demarrage de l'enfant, avant toute boucle de capture.

Ceci ne leve PAS le plafond de quatre fenetres : la 5e duplication est refusee
alors que quatre sont tenues, et fermer puis rouvrir reussit — c'est une limite
de concurrence, que nulle patience ne franchit.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 12 : le rapport, et les documents qu'il faut accorder

**Files:**
- Create: `docs/superpowers/plans/2026-08-01-multifenetres-arrangement-dynamique-resultats.md`
- Modify: `CLAUDE.md`, `docs/superpowers/specs/2026-07-28-support-jeux-design.md`

- [ ] **Étape 1 : écrire le rapport**

Plan imposé, celui des rapports de ce dépôt : le verdict en une phrase ; ce qui a été exécuté et ce qui a été écarté ; le déroulé avec son issue réellement observée ; ce qui a échoué ; **ce que la démonstration n'établit pas** ; les pièges rencontrés ; l'état de la VM à la fin ; ce qu'il reste à régler.

Trois exigences que les chantiers précédents ont payées cher :

- **ne rien affirmer au-delà du relevé.** Le mode de défaillance dominant de ce dépôt est l'énoncé, pas le code — onze rondes de correction sur la sonde multi-fenêtres, presque toutes sur des rapports qui affirmaient au-delà de leur mesure ;
- **dire le nombre d'exécutions** partout où un taux est suggéré. « Une exécution par configuration » n'est pas « ça marche » ;
- **distinguer relevé et calculé**, systématiquement.

- [ ] **Étape 2 : accorder `CLAUDE.md`**

Ajouter une section « Sous-bloc D2 » après celle de D1, et **annoter la section D1 existante** : son encadré « la voie recommandée … s'effondre dès qu'une fenêtre s'ouvre » doit renvoyer au verdict de D2.

**Balayer par le SENS et non par la formule** : chercher toutes les affirmations que D2 réfute ou amende, y compris dans les sommaires et les tables — un chapitre corrigé sous un sommaire intact laisse le lecteur repartir avec une tâche déjà faite. Les tournures à balayer : « pas / non / jamais / reste ouvert / n'est établi par rien / conjecture ».

- [ ] **Étape 3 : accorder la spec du chantier**

Dans `docs/superpowers/specs/2026-07-28-support-jeux-design.md`, l'encadré D1 du §5 D **et** le point 3 du §8 « Ordre recommandé » — les deux portent l'affirmation « l'arrangement ne survit pas à l'ouverture d'une fenêtre de plus ».

- [ ] **Étape 4 : vérifier la dette de taille de fichier**

```bash
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
```

Expected: les trois fichiers de dette connus (`encode.rs`, `windows_source.rs`, `wasapi.rs`), **et aucun de plus**. `windows_source.rs` ne doit pas avoir grossi.

- [ ] **Étape 5 : commit**

```bash
git add docs/superpowers/plans/2026-08-01-multifenetres-arrangement-dynamique-resultats.md \
        CLAUDE.md docs/superpowers/specs/2026-07-28-support-jeux-design.md
git commit -m "docs(d2): resultats du sous-bloc, et accord des documents amont

<Verdict en une ligne.> CLAUDE.md et la spec du chantier D portaient tous deux
l'affirmation « l'arrangement ne survit pas a l'ouverture d'une fenetre de
plus » : les deux sont annotees, sommaires compris.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Auto-relecture

**Couverture de la spec** — chaque section a sa tâche :

| Spec | Tâche |
| --- | --- |
| §3.2 classification | 1, 3 |
| §3.3 reconstruction étroite | 2 |
| §3.4 budget borné | 1, 3 |
| §3.5 où ce code vit | 1 (module frère), 5 et 10 (contrôles de taille) |
| §4 désignation par nom | 2 (capture), 4 (superviseur) |
| §5.1 a/b/c viewport, tolérance, attente | 8, 7, 7 |
| §5.2 clavier | 9 |
| §5.3 fenêtre non oubliée | 10 |
| §6.1 banc, point d'arrêt | 5 (le banc), 6 (l'exécution) |
| §6.2 démonstration | 11 |
| §7 tests | 1, 4, 7, 8, 10 |
| §8 voie écartée / repli | 6 (l'arrêt qui y renvoie) |
| §9 risques | 12 (le rapport les reprend) |

**Cohérence des types** — vérifiée d'une tâche à l'autre : `CibleCapture` (t. 2) est consommée par `rouvrir` (t. 2) et journalisée par `next_frame` (t. 3) ; `BudgetReprises` (t. 1) est portée par `DesktopCapture` (t. 3) ; `nom_sortie: String` traverse `Effet::LancerEnfant` → `Consigne` → `SORTIE_DXGI` → `Config::sortie_dxgi: Option<String>` → `DesktopCapture::sur_sortie(&str)` (t. 2 et 4) ; `deja_prises: &[String]` (t. 7) correspond à `prises: Vec<String>` (t. 4).

**Compte de tests attendu**, cumulé : 251 au départ → 254 (t. 1) → 256 (t. 4) → 259 (t. 7) → 259 + 4 côté client (t. 8) → 262 (t. 10).

**Une dépendance d'ordre à ne pas casser** : la tâche 6 est un **point d'arrêt**. Les tâches 7 à 10 sont indépendantes entre elles et peuvent se paralléliser, mais aucune ne doit être engagée avant que la tâche 6 ait rendu son verdict — c'est tout l'intérêt de la placer là.
