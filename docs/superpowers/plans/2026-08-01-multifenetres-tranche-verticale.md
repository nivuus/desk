# Tranche verticale multi-fenêtres (chantier D, sous-bloc D1) — plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** qu'une deuxième fenêtre Windows ouvre une deuxième fenêtre navigateur qui l'affiche, avec entrée et son, chaque fenêtre ayant sa propre sortie virtuelle, sa propre duplication DXGI, son propre encodeur et sa propre connexion WebRTC.

**Architecture:** un processus superviseur détient le hook de détection des fenêtres, le pilote d'affichage virtuel et le lancement des enfants ; un processus enfant par fenêtre reprend l'agent actuel en capturant une sortie DXGI entière au lieu de recadrer une fenêtre ; une page-shell côté navigateur ouvre une fenêtre par fenêtre Windows et annonce les viewports. Le signaling ne gagne aucun rôle : la session de contrôle réutilise `agent`/`client` sur un `session_id` réservé.

**Tech Stack:** Rust (windows-rs 0.62, str0m, Media Foundation, DXGI Desktop Duplication), TypeScript (Vite, vitest), Node (ws).

**Spec:** `docs/superpowers/specs/2026-08-01-multifenetres-tranche-verticale-design.md`

## Global Constraints

- **Aucun nouveau fichier ne naît au-dessus de 500 lignes** (CLAUDE.md, § Conventions de code). Les frontières de fichiers de ce plan sont posées pour tenir cette règle ; si une tâche fait franchir le seuil, elle extrait au lieu de compresser.
- **`agent/src/encode/arret.rs` est à 500 lignes exactement, sa marge est NULLE.** Aucune tâche de ce plan ne doit y ajouter une ligne.
- **La logique pure ne va jamais sous `#[cfg(windows)]`.** Elle doit compiler et se tester sur l'hôte Linux — c'est la doctrine déjà appliquée à `moniteurs_virtuels.rs`, dont le commentaire de module explique pourquoi.
- **Tests Rust sur l'hôte** : `cargo test -p agent` depuis la racine. **Compilation Windows** : `scripts/build-agent.sh` (synchronise puis compile SUR la VM). **Tests TypeScript** : `npm test` dans `client/` et `signaling/`.
- **La VM n'est jamais supposée allumée.** Vérifier `virsh list --all` et démarrer si besoin ; attendre WinRM **puis** un accès réel à `/media/vm` (`ls /media/vm/dev`), le montage CIFS survivant à une VM éteinte.
- **Ne jamais utiliser `git add -A`** — nommer les fichiers. Un `git add -A agent/src` a déjà emporté le travail concurrent d'une autre tâche dans un commit qui ne compilait pas.
- **Ne jamais tracer par paquet ni par image dans une boucle de transport ou de capture.** Compter ou échantillonner.
- **Débit et résolution de référence** : 1280×720 à 60 Hz, 8 Mb/s — les valeurs des quatre rangs déjà mesurés.
- **Plafonds connus** : 10 sorties virtuelles simultanées (refus du pilote à la 11ᵉ), 8 encodeurs dans un processus unique (le comportement en multi-processus est inconnu et ce plan le relèvera).

---

## File Structure

**Créés :**

| Fichier | Responsabilité |
| --- | --- |
| `agent/src/superviseur.rs` | assemblage seul — il n'a ni boucle ni décision |
| `agent/src/superviseur/boucle.rs` | la boucle et l'exécution des effets rendus par la table |
| `agent/src/superviseur/lanceur.rs` | lancement et contrôle de vie d'un processus Windows |
| `agent/src/superviseur/signalisation.rs` | connexion à la session de contrôle du signaling |
| `agent/src/superviseur/fenetres.rs` | critère « Alt-Tab-able », logique pure |
| `agent/src/superviseur/table.rs` | machine à états fenêtre → sortie → enfant, logique pure |
| `agent/src/superviseur/hook.rs` | `SetWinEventHook` et pompe de messages, glue Windows |
| `agent/src/superviseur/enfants.rs` | lancement, surveillance, mise à mort bornée |
| `agent/src/superviseur/protocole.rs` | messages de la session « bureau » |
| `agent/src/superviseur/placement.rs` | poser une fenêtre sur une sortie et l'y maintenir |
| `agent/src/moniteurs_virtuels/pilote.rs` | glue IOCTL, **promue** depuis `diagnostics/multifenetre/moniteurs.rs` |
| `agent/src/moniteurs_virtuels/sudovda.rs` | constantes et structures du pilote, **promues** |
| `agent/src/moniteurs_virtuels/peripherique.rs` | énumération SetupAPI, **promue** |
| `agent/src/moniteurs_virtuels/purge.rs` | purge des sorties orphelines, **promue** |
| `agent/src/windows_source/sortie.rs` | construction d'une `WindowsSource` sur une sortie DXGI entière |
| `client/shell.html` | page-shell |
| `client/src/shell.ts` | logique de la page-shell |
| `client/src/shell.test.ts` | tests de la page-shell |

**Modifiés :**

| Fichier | Changement |
| --- | --- |
| `agent/src/main.rs` | déclaration des modules, champs de `Config` pour le mode superviseur et les arguments d'enfant |
| `agent/src/moniteurs_virtuels.rs` | devient le module parent ; garde sa logique pure et ses tests |
| `agent/src/demarrage.rs` | l'enfant prend sa fenêtre et sa sortie par configuration au lieu de chercher un titre |
| `agent/src/windows_source.rs` | expose la construction sur une sortie entière |
| `agent/src/diagnostics/multifenetre.rs` | pointe vers les modules promus au lieu des siens |
| `signaling/src/server.ts` | élargit les types relayés, mémorise la dernière offre |
| `signaling/src/server.test.ts` | tests des deux changements ci-dessus |
| `client/src/main.ts` | annonce le viewport à la page-shell |
| `client/vite.config.ts` | déclare la seconde page (`shell.html`) |

---

### Task 1: Confirmer que le bureau virtuel inclut une sortie virtuelle

Le seul risque de ce plan qui pourrait invalider l'architecture. `input.rs` est déjà préparé (`MOUSEEVENTF_VIRTUALDESK` l. 147, métriques du bureau virtuel l. 172-175) : il n'y a rien à écrire côté produit, seulement à établir que le pointeur atteint bien une sortie virtuelle. Si le résultat dément, tout le reste du plan change de forme — d'où sa place en tête.

**Files:**
- Create: `agent/src/diagnostics/multifenetre/pointeur_virtuel.rs`
- Modify: `agent/src/diagnostics/multifenetre.rs`

**Interfaces:**
- Consumes: `crate::moniteurs_virtuels::{PiloteAffichageVirtuel, Sorties}`, `super::moniteurs::ouvrir_pilote`, `crate::capture::enumerer_sorties`
- Produces: rien pour les tâches suivantes — c'est une sonde, son livrable est un journal et un verdict écrit

- [ ] **Step 1: Écrire la sonde**

Créer `agent/src/diagnostics/multifenetre/pointeur_virtuel.rs` :

```rust
//! Sonde : le bureau virtuel s'étend-il jusqu'à une sortie virtuelle, et le
//! pointeur y arrive-t-il ?
//!
//! `input.rs` pose déjà `MOUSEEVENTF_VIRTUALDESK` et calcule ses coordonnées
//! sur `SM_*VIRTUALSCREEN` : rien n'est à écrire côté produit. Ce qui n'est
//! établi par aucune lecture de code, c'est que Windows compte une sortie
//! virtuelle dans ces métriques. Cette sonde le tranche par une mesure.

use anyhow::{Context, Result};
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_MOVE,
    MOUSEEVENTF_VIRTUALDESK, MOUSEINPUT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
    SM_YVIRTUALSCREEN,
};

use super::moniteurs::ouvrir_pilote;
use crate::capture::enumerer_sorties;
use crate::moniteurs_virtuels::Sorties;

/// Rectangle du bureau virtuel, tel que Windows le déclare.
fn bureau_virtuel() -> (i32, i32, i32, i32) {
    unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    }
}

pub(super) fn sonder() -> Result<()> {
    let avant = bureau_virtuel();
    tracing::info!(
        x = avant.0, y = avant.1, largeur = avant.2, hauteur = avant.3,
        "bureau virtuel AVANT création de la sortie"
    );

    let pilote = ouvrir_pilote()?;
    let mut sorties = Sorties::nouvelles(&pilote);
    let id = sorties.creer(1280, 720, 60).context("création de la sortie virtuelle")?;

    // Le pilote crée la sortie de façon asynchrone du point de vue de
    // l'espace de bureau : Windows doit encore la rattacher. On laisse
    // le temps à la topologie de s'établir, puis on relit.
    std::thread::sleep(std::time::Duration::from_secs(3));
    pilote.pinguer()?;

    let apres = bureau_virtuel();
    tracing::info!(
        id, x = apres.0, y = apres.1, largeur = apres.2, hauteur = apres.3,
        elargi = (apres != avant),
        "bureau virtuel APRÈS création de la sortie"
    );

    // Retrouver la sortie virtuelle parmi les sorties DXGI : c'est son
    // rectangle qui donne la cible à viser. `GetDesc`/`DesktopCoordinates`
    // est la source de vérité — WMI ment (champ vu périmé de 68 s).
    let toutes = enumerer_sorties()?;
    for s in &toutes {
        tracing::info!(
            adaptateur = %s.adaptateur, nom = %s.nom_sortie,
            attachee = s.attachee_au_bureau,
            x = s.rect.x, y = s.rect.y, l = s.rect.width, h = s.rect.height,
            "sortie DXGI énumérée"
        );
    }
    let cible = toutes
        .iter()
        .filter(|s| s.attachee_au_bureau && s.rect.width == 1280 && s.rect.height == 720)
        .max_by_key(|s| s.rect.x)
        .context(
            "aucune sortie attachée de 1280x720 : la sortie virtuelle n'est pas \
             entrée dans la topologie du bureau",
        )?;

    // Centre de la sortie, en coordonnées du bureau virtuel, converti dans
    // l'espace normalisé 0..65535 que `MOUSEEVENTF_ABSOLUTE` attend — la
    // conversion exacte de `input.rs`.
    let vise_x = cible.rect.x + (cible.rect.width / 2) as i32;
    let vise_y = cible.rect.y + (cible.rect.height / 2) as i32;
    let normalise = |v: i32, origine: i32, etendue: i32| -> i32 {
        ((v - origine) as i64 * 65535 / etendue.max(1) as i64) as i32
    };
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: normalise(vise_x, apres.0, apres.2),
                dy: normalise(vise_y, apres.1, apres.3),
                mouseData: 0,
                dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let envoyes = unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };

    let mut ou = POINT::default();
    unsafe { GetCursorPos(&mut ou) }.context("GetCursorPos")?;

    let ecart = ((ou.x - vise_x).abs(), (ou.y - vise_y).abs());
    tracing::info!(
        envoyes,
        vise_x, vise_y, obtenu_x = ou.x, obtenu_y = ou.y,
        ecart_x = ecart.0, ecart_y = ecart.1,
        // Deux pixels de tolérance : la conversion normalisée n'est pas
        // exactement réversible, et ce n'est pas ce qu'on mesure ici.
        verdict = if ecart.0 <= 2 && ecart.1 <= 2 { "ATTEINTE" } else { "NON ATTEINTE" },
        "injection absolue vers le centre de la sortie virtuelle"
    );

    Ok(())
}
```

- [ ] **Step 2: Câbler la variable d'environnement**

Dans `agent/src/diagnostics/multifenetre.rs`, ajouter la déclaration de module auprès des autres (`pub(super) mod pointeur_virtuel;`), et dans `aiguiller()`, **après** la branche `MULTIFENETRE_VDD_PURGE` (une purge demandée ne doit jamais être supplantée) :

```rust
    if std::env::var("MULTIFENETRE_POINTEUR").is_ok() {
        pointeur_virtuel::sonder()?;
        return Ok(true);
    }
```

- [ ] **Step 3: Compiler sur la VM**

```bash
virsh list --all   # démarrer si « fermé », puis attendre WinRM puis `ls /media/vm/dev`
scripts/build-agent.sh 2>&1 | tee /tmp/build-tache1.log
```

Attendu : compilation sans erreur. **Verser le journal de compilation** — la fraîcheur du binaire mesuré est une pièce qui a manqué au chantier précédent.

- [ ] **Step 4: Exécuter la sonde en session interactive**

```bash
MULTIFENETRE_POINTEUR=1 scripts/run-agent.sh
```

La sonde doit tourner en **session interactive** (`scripts/run-agent.sh`, tâche planifiée) et non par WinRM : la session 0 de WinRM n'a ni pointeur ni bureau.

- [ ] **Step 5: Écrire le verdict et purger**

Relever dans le journal : le bureau virtuel s'est-il élargi (`elargi=true`) ? La sortie virtuelle est-elle énumérée et attachée ? Le verdict d'injection est-il `ATTEINTE` ?

Puis, **depuis un processus neuf** (le processus mesureur est juge et partie, et une sortie virtuelle lui survit) :

```bash
MULTIFENETRE_VDD_PURGE=1 scripts/run-agent.sh
MULTIFENETRE_DXGI=1 scripts/run-agent.sh   # contrôle : la topologie est-elle revenue ?
```

Consigner le verdict dans `docs/superpowers/plans/journaux-multifenetres-d1/pointeur-virtuel.log` et une ligne de conclusion dans le rapport de tâche.

**Si le verdict est NON ATTEINTE** : ne pas continuer le plan tel quel. L'injection devra passer en relatif (déjà implémenté au chantier B), ce qui change la tâche 12 et impose de le dire dans la spec.

- [ ] **Step 6: Commit**

```bash
git add agent/src/diagnostics/multifenetre/pointeur_virtuel.rs \
        agent/src/diagnostics/multifenetre.rs \
        docs/superpowers/plans/journaux-multifenetres-d1/pointeur-virtuel.log
git commit -m "mesure(d1): le bureau virtuel inclut-il une sortie virtuelle"
```

---

### Task 2: Le critère « Alt-Tab-able », en logique pure

**Files:**
- Create: `agent/src/superviseur/fenetres.rs`
- Create: `agent/src/superviseur.rs`
- Modify: `agent/src/main.rs`

**Interfaces:**
- Consumes: rien
- Produces: `superviseur::fenetres::{DescriptionFenetre, merite_une_fenetre}`. `DescriptionFenetre` est un enregistrement de faits **déjà relevés** — la tâche 6 le remplira depuis les API Windows ; ici il n'appelle rien.

- [ ] **Step 1: Écrire les tests qui échouent**

Créer `agent/src/superviseur/fenetres.rs` avec, **d'abord**, le module de tests seul :

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Une fenêtre ordinaire d'application : tout ce qu'il faut pour mériter
    /// une fenêtre navigateur.
    fn ordinaire() -> DescriptionFenetre {
        DescriptionFenetre {
            visible: true,
            a_un_proprietaire: false,
            tool_window: false,
            app_window: false,
            masquee_dwm: false,
            titre: "Bloc-notes".into(),
        }
    }

    #[test]
    fn une_fenetre_ordinaire_merite_une_fenetre_navigateur() {
        assert!(merite_une_fenetre(&ordinaire()));
    }

    #[test]
    fn une_fenetre_invisible_est_ecartee() {
        let d = DescriptionFenetre { visible: false, ..ordinaire() };
        assert!(!merite_une_fenetre(&d));
    }

    #[test]
    fn une_fenetre_possedee_est_ecartee() {
        // Dialogues modaux, palettes : elles restent composées dans leur
        // parente, qui a déjà sa fenêtre navigateur.
        let d = DescriptionFenetre { a_un_proprietaire: true, ..ordinaire() };
        assert!(!merite_une_fenetre(&d));
    }

    #[test]
    fn une_tool_window_est_ecartee() {
        let d = DescriptionFenetre { tool_window: true, ..ordinaire() };
        assert!(!merite_une_fenetre(&d));
    }

    #[test]
    fn une_tool_window_qui_est_aussi_app_window_est_gardee() {
        // WS_EX_APPWINDOW force la présence dans Alt-Tab : c'est la
        // dérogation exacte que le critère du cadrage prévoit.
        let d = DescriptionFenetre { tool_window: true, app_window: true, ..ordinaire() };
        assert!(merite_une_fenetre(&d));
    }

    #[test]
    fn une_fenetre_masquee_par_dwm_est_ecartee() {
        // Sans ce filtre on capte les fenêtres UWP fantômes, qui existent
        // sans jamais s'afficher.
        let d = DescriptionFenetre { masquee_dwm: true, ..ordinaire() };
        assert!(!merite_une_fenetre(&d));
    }

    #[test]
    fn une_fenetre_sans_titre_est_ecartee() {
        let d = DescriptionFenetre { titre: String::new(), ..ordinaire() };
        assert!(!merite_une_fenetre(&d));
    }

    #[test]
    fn le_masquage_dwm_prime_sur_app_window() {
        // Une fenêtre fantôme qui porterait WS_EX_APPWINDOW ne doit pas
        // ressortir par la dérogation : l'ordre des tests compte ici.
        let d = DescriptionFenetre {
            tool_window: true,
            app_window: true,
            masquee_dwm: true,
            ..ordinaire()
        };
        assert!(!merite_une_fenetre(&d));
    }
}
```

Créer `agent/src/superviseur.rs` :

```rust
//! Le superviseur : il détecte les fenêtres, leur donne une sortie virtuelle,
//! lance un processus enfant par fenêtre et parle à la page-shell.
//!
//! Ce fichier reste mince à dessein — il assemble, il ne décide pas. Les
//! décisions vivent dans `fenetres` (quelle fenêtre mérite d'exister côté
//! navigateur) et `table` (où en est chacune), tous deux en logique pure et
//! testés sur l'hôte.

pub mod fenetres;
```

Dans `agent/src/main.rs`, ajouter `mod superviseur;` auprès des autres déclarations **hors** `#[cfg(windows)]` — la logique pure doit se compiler et se tester sur Linux.

- [ ] **Step 2: Lancer les tests et vérifier qu'ils échouent**

Run: `cargo test -p agent superviseur::fenetres`
Expected: FAIL — `cannot find type DescriptionFenetre in this scope`, `cannot find function merite_une_fenetre in this scope`.

- [ ] **Step 3: Écrire l'implémentation minimale**

En tête de `agent/src/superviseur/fenetres.rs`, **avant** le module de tests :

```rust
//! Le critère « Alt-Tab-able » : quelles fenêtres Windows méritent une fenêtre
//! navigateur.
//!
//! Logique pure, délibérément séparée de la glue Windows de `hook.rs` : c'est
//! la règle produit, celle qui décide de ce que l'utilisateur voit, et elle
//! doit être éprouvable sans Windows.

/// Ce qu'on a relevé d'une fenêtre. Aucun appel système ici : `hook.rs`
/// remplit cette structure, ce module la juge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptionFenetre {
    /// `WS_VISIBLE` et `IsWindowVisible`.
    pub visible: bool,
    /// `GetWindow(hwnd, GW_OWNER)` non nul.
    pub a_un_proprietaire: bool,
    /// `WS_EX_TOOLWINDOW`.
    pub tool_window: bool,
    /// `WS_EX_APPWINDOW`.
    pub app_window: bool,
    /// `DwmGetWindowAttribute` / `DWMWA_CLOAKED` non nul.
    pub masquee_dwm: bool,
    /// `GetWindowTextW`.
    pub titre: String,
}

/// Vrai si cette fenêtre mérite sa propre fenêtre navigateur.
///
/// Le critère est celui du cadrage (`specs/2026-07-28-support-jeux-design.md`
/// §4, « critère de filtrage retenu ») : tout ce qui n'est pas ici — menus
/// déroulants, infobulles, dialogues modaux, écrans de démarrage — reste
/// composé dans sa fenêtre parente et arrive donc par la capture de celle-ci.
pub fn merite_une_fenetre(d: &DescriptionFenetre) -> bool {
    if !d.visible || d.masquee_dwm || d.a_un_proprietaire {
        return false;
    }
    // Une fenêtre sans titre n'est présentable ni dans Alt-Tab ni dans la
    // page-shell : rien ne permettrait à l'utilisateur de la désigner.
    if d.titre.is_empty() {
        return false;
    }
    // `WS_EX_APPWINDOW` est la dérogation explicite : elle force la présence
    // dans Alt-Tab malgré `WS_EX_TOOLWINDOW`.
    !d.tool_window || d.app_window
}
```

- [ ] **Step 4: Lancer les tests et vérifier qu'ils passent**

Run: `cargo test -p agent superviseur::fenetres`
Expected: PASS, 8 tests.

- [ ] **Step 5: Commit**

```bash
git add agent/src/superviseur.rs agent/src/superviseur/fenetres.rs agent/src/main.rs
git commit -m "feat(d1): le critere Alt-Tab-able, en logique pure et teste"
```

---

### Task 3: La table du superviseur

Le cœur décisionnel : où en est chaque fenêtre, quelle sortie lui est attribuée, quel enfant tourne. Entièrement pure — le pilote et le lanceur sont des traits, injectés par la tâche 11.

**Files:**
- Create: `agent/src/superviseur/table.rs`
- Modify: `agent/src/superviseur.rs`

**Interfaces:**
- Consumes: `superviseur::fenetres::DescriptionFenetre`
- Produces:
  - `superviseur::table::{Table, IdFenetre, IdSession, Etat, Effet}`
  - `Table::nouvelle(capacite: usize) -> Table`
  - `Table::fenetre_apparue(&mut self, id: IdFenetre, titre: String) -> Vec<Effet>`
  - `Table::viewport_recu(&mut self, session: &IdSession, largeur: u32, hauteur: u32) -> Vec<Effet>`
  - `Table::sortie_creee(&mut self, session: &IdSession, sortie_pilote: u32, dxgi: (u32, u32)) -> Vec<Effet>`
  - `Table::sortie_dxgi_de(&self, session: &IdSession) -> Option<(u32, u32)>`
  - `Table::fenetre_disparue(&mut self, id: IdFenetre) -> Vec<Effet>`
  - `Table::enfant_mort(&mut self, session: &IdSession) -> Vec<Effet>`
  - `Table::etat(&self, session: &IdSession) -> Option<&Etat>`
  - `Table::sessions_vivantes(&self) -> Vec<IdSession>`
  - `Effet::{AnnoncerOuverture, CreerSortie, LancerEnfant, TuerEnfant, DetruireSortie, AnnoncerFermeture, AnnoncerRefus}` — `LancerEnfant` porte `index_adaptateur`/`index_sortie` (position DXGI, ce que l'enfant capture) et `DetruireSortie` porte `sortie_pilote` **et** `dxgi` (le pilote ne retire que par son propre identifiant)

- [ ] **Step 1: Écrire les tests qui échouent**

Créer `agent/src/superviseur/table.rs` avec, d'abord, ses tests :

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Table {
        Table::nouvelle(10)
    }

    /// Récupère l'identifiant de session attribué à la fenêtre, en lisant
    /// l'effet d'annonce — c'est la seule sortie publique qui le porte.
    fn session_annoncee(effets: &[Effet]) -> IdSession {
        effets
            .iter()
            .find_map(|e| match e {
                Effet::AnnoncerOuverture { session, .. } => Some(session.clone()),
                _ => None,
            })
            .expect("une ouverture doit être annoncée")
    }

    #[test]
    fn une_fenetre_qui_apparait_est_annoncee_et_rien_de_plus() {
        // Rien ne peut être créé avant de connaître le viewport : c'est lui
        // qui donne la taille de la sortie.
        let mut t = table();
        let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
        assert_eq!(effets.len(), 1);
        let session = session_annoncee(&effets);
        assert_eq!(t.etat(&session), Some(&Etat::AttendLeViewport));
    }

    #[test]
    fn le_viewport_declenche_la_creation_de_la_sortie() {
        let mut t = table();
        let session = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into()));
        let effets = t.viewport_recu(&session, 1600, 900);
        assert_eq!(
            effets,
            vec![Effet::CreerSortie { session: session.clone(), largeur: 1600, hauteur: 900 }]
        );
        assert_eq!(t.etat(&session), Some(&Etat::AttendLaSortie));
    }

    #[test]
    fn la_sortie_creee_declenche_le_lancement_de_l_enfant() {
        let mut t = table();
        let session = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into()));
        t.viewport_recu(&session, 1600, 900);
        // Deux identifiants distincts, et c'est le fond du sujet : `7` est
        // l'identifiant que le PILOTE a rendu, `(0, 4)` la position de la
        // même sortie dans l'énumération DXGI. Le pilote ne détruit que par
        // le premier ; l'enfant ne sait capturer que par le second.
        let effets = t.sortie_creee(&session, 7, (0, 4));
        assert_eq!(
            effets,
            vec![Effet::LancerEnfant {
                session: session.clone(),
                fenetre: IdFenetre(1),
                index_adaptateur: 0,
                index_sortie: 4,
                // La première fenêtre porte le son : le loopback WASAPI capte
                // toute la session Windows, deux porteurs feraient entendre
                // deux fois le même son.
                audio: true,
            }]
        );
        assert_eq!(t.etat(&session), Some(&Etat::Vivante));
        assert_eq!(t.sortie_dxgi_de(&session), Some((0, 4)));
    }

    #[test]
    fn seule_la_premiere_fenetre_porte_le_son() {
        let mut t = table();
        let a = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "A".into()));
        t.viewport_recu(&a, 1600, 900);
        t.sortie_creee(&a, 7, (0, 4));

        let b = session_annoncee(&t.fenetre_apparue(IdFenetre(2), "B".into()));
        t.viewport_recu(&b, 1280, 720);
        let effets = t.sortie_creee(&b, 8, (0, 5));
        assert_eq!(
            effets,
            vec![Effet::LancerEnfant {
                session: b,
                fenetre: IdFenetre(2),
                index_adaptateur: 0,
                index_sortie: 5,
                audio: false,
            }]
        );
    }

    #[test]
    fn une_fenetre_qui_disparait_tue_l_enfant_detruit_la_sortie_et_l_annonce() {
        let mut t = table();
        let session = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into()));
        t.viewport_recu(&session, 1600, 900);
        t.sortie_creee(&session, 7, (0, 4));

        let effets = t.fenetre_disparue(IdFenetre(1));
        assert_eq!(
            effets,
            vec![
                Effet::TuerEnfant { session: session.clone() },
                // L'identifiant du PILOTE, seul avec lequel il sait retirer.
                Effet::DetruireSortie { sortie_pilote: 7, dxgi: (0, 4) },
                Effet::AnnoncerFermeture { session: session.clone() },
            ]
        );
        assert_eq!(t.etat(&session), None, "la fenêtre doit avoir quitté la table");
    }

    #[test]
    fn un_enfant_qui_meurt_seul_libere_la_sortie_et_l_annonce_sans_le_tuer() {
        // C'est le bénéfice pour lequel le multi-processus a été choisi : la
        // mort d'un enfant ne doit rien emporter d'autre, mais elle ne doit
        // pas non plus laisser fuir sa sortie.
        let mut t = table();
        let session = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into()));
        t.viewport_recu(&session, 1600, 900);
        t.sortie_creee(&session, 7, (0, 4));

        let effets = t.enfant_mort(&session);
        assert_eq!(
            effets,
            vec![
                Effet::DetruireSortie { sortie_pilote: 7, dxgi: (0, 4) },
                Effet::AnnoncerFermeture { session: session.clone() },
            ]
        );
        assert_eq!(t.etat(&session), None);
    }

    #[test]
    fn une_fenetre_qui_disparait_avant_sa_sortie_ne_demande_aucune_destruction() {
        // Fermée pendant qu'on attendait son viewport : aucune sortie
        // n'existe, et demander d'en détruire une ferait échouer le pilote.
        let mut t = table();
        let session = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "A".into()));
        let effets = t.fenetre_disparue(IdFenetre(1));
        assert_eq!(
            effets,
            vec![
                Effet::TuerEnfant { session: session.clone() },
                Effet::AnnoncerFermeture { session },
            ]
        );
    }

    #[test]
    fn le_vivier_plein_refuse_la_fenetre_suivante_sans_rien_casser() {
        let mut t = Table::nouvelle(2);
        for n in 1..=2u64 {
            let s = session_annoncee(&t.fenetre_apparue(IdFenetre(n), format!("F{n}")));
            t.viewport_recu(&s, 1280, 720);
            t.sortie_creee(&s, n as u32, (0, n as u32));
        }
        let effets = t.fenetre_apparue(IdFenetre(3), "F3".into());
        assert_eq!(
            effets,
            vec![Effet::AnnoncerRefus {
                titre: "F3".into(),
                motif: "plus aucune sortie virtuelle disponible".into()
            }]
        );
    }

    #[test]
    fn une_sortie_liberee_rouvre_la_place() {
        let mut t = Table::nouvelle(1);
        let a = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "A".into()));
        t.viewport_recu(&a, 1280, 720);
        t.sortie_creee(&a, 7, (0, 4));
        assert!(matches!(
            t.fenetre_apparue(IdFenetre(2), "B".into()).as_slice(),
            [Effet::AnnoncerRefus { .. }]
        ));

        t.fenetre_disparue(IdFenetre(1));
        let effets = t.fenetre_apparue(IdFenetre(3), "C".into());
        assert!(matches!(effets.as_slice(), [Effet::AnnoncerOuverture { .. }]));
    }

    #[test]
    fn le_son_repasse_a_personne_tant_que_d2_ne_le_redesigne_pas() {
        // Limite assumée de D1, écrite en test pour qu'elle soit un choix
        // visible plutôt qu'un oubli : fermer la porteuse ne redésigne rien.
        let mut t = table();
        let a = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "A".into()));
        t.viewport_recu(&a, 1280, 720);
        t.sortie_creee(&a, 7, (0, 4));
        let b = session_annoncee(&t.fenetre_apparue(IdFenetre(2), "B".into()));
        t.viewport_recu(&b, 1280, 720);
        t.sortie_creee(&b, 8, (0, 5));

        t.fenetre_disparue(IdFenetre(1));
        let c = session_annoncee(&t.fenetre_apparue(IdFenetre(3), "C".into()));
        t.viewport_recu(&c, 1280, 720);
        let effets = t.sortie_creee(&c, 9, (0, 6));
        assert_eq!(
            effets,
            vec![Effet::LancerEnfant {
                session: c,
                fenetre: IdFenetre(3),
                index_adaptateur: 0,
                index_sortie: 6,
                audio: false,
            }],
            "aucune redésignation du son en D1"
        );
    }

    #[test]
    fn un_viewport_pour_une_session_inconnue_est_ignore() {
        // Le navigateur est une source externe : un message tardif ou rejoué
        // ne doit produire aucun effet.
        let mut t = table();
        let effets = t.viewport_recu(&IdSession("w-inconnue".into()), 800, 600);
        assert!(effets.is_empty());
    }

    #[test]
    fn un_second_viewport_pour_la_meme_session_est_ignore() {
        let mut t = table();
        let session = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "A".into()));
        t.viewport_recu(&session, 1600, 900);
        let effets = t.viewport_recu(&session, 800, 600);
        assert!(effets.is_empty(), "la sortie est déjà demandée à la première taille");
    }

    #[test]
    fn les_identifiants_de_session_sont_uniques() {
        let mut t = table();
        let a = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "A".into()));
        let b = session_annoncee(&t.fenetre_apparue(IdFenetre(2), "B".into()));
        assert_ne!(a, b);
    }

    #[test]
    fn la_fenetre_d_une_session_est_retrouvable() {
        // Le contrôle périodique de placement connaît la sortie par session
        // et doit remonter à la fenêtre pour la replacer.
        let mut t = table();
        let session = session_annoncee(&t.fenetre_apparue(IdFenetre(42), "A".into()));
        assert_eq!(t.fenetre_de(&session), Some(IdFenetre(42)));
        assert_eq!(t.fenetre_de(&IdSession("w-inconnue".into())), None);
    }
}
```

- [ ] **Step 2: Lancer les tests et vérifier qu'ils échouent**

Run: `cargo test -p agent superviseur::table`
Expected: FAIL — `cannot find type Table in this scope`.

- [ ] **Step 3: Écrire l'implémentation**

En tête de `agent/src/superviseur/table.rs` :

```rust
//! Où en est chaque fenêtre : détectée, en attente de son viewport, en attente
//! de sa sortie, vivante.
//!
//! Logique pure et sans effet de bord : la table ne crée rien, ne tue rien,
//! ne parle à personne. Elle rend une liste d'`Effet` que `superviseur.rs`
//! exécute. C'est ce qui la rend éprouvable sans Windows, sans pilote et sans
//! navigateur — et c'est là que vivent les règles qui, mal écrites, feraient
//! fuir une sortie virtuelle ou dédoubler le son.

use std::collections::HashMap;

/// Identifiant opaque d'une fenêtre Windows. C'est un `HWND` côté Windows,
/// mais ce module n'en sait rien et n'a pas à en savoir plus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IdFenetre(pub u64);

/// Identifiant de session, tel que le signaling et l'URL du navigateur le
/// portent. Opaque à dessein : ni le `HWND` ni le titre, qui changent tous
/// deux au cours de la vie d'une fenêtre.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdSession(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Etat {
    /// Annoncée à la page-shell ; on attend qu'elle dise la taille de sa
    /// fenêtre navigateur.
    AttendLeViewport,
    /// Le viewport est connu, la sortie virtuelle est demandée.
    AttendLaSortie,
    /// L'enfant tourne.
    Vivante,
}

/// Ce que la table demande au monde extérieur de faire. Le superviseur les
/// exécute dans l'ordre rendu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effet {
    AnnoncerOuverture { session: IdSession, titre: String },
    CreerSortie { session: IdSession, largeur: u32, hauteur: u32 },
    LancerEnfant {
        session: IdSession,
        fenetre: IdFenetre,
        index_adaptateur: u32,
        index_sortie: u32,
        audio: bool,
    },
    TuerEnfant { session: IdSession },
    /// `sortie_pilote` est **l'identifiant du PILOTE**, pas l'index DXGI : le
    /// pilote ne sait retirer une sortie que par ce qu'il a lui-même rendu à
    /// la création ; lui présenter un index DXGI ne détruirait rien, ou
    /// détruirait la sortie d'autrui. Les deux identifiants désignent la même
    /// sortie et n'ont aucune relation calculable — d'où les deux champs.
    ///
    /// `dxgi` accompagne la destruction parce que l'entrée a déjà quitté la
    /// table quand cet effet est rendu : sans lui, l'appelant ne pourrait
    /// plus savoir quelle place DXGI redevient libre.
    DetruireSortie { sortie_pilote: u32, dxgi: (u32, u32) },
    AnnoncerFermeture { session: IdSession },
    AnnoncerRefus { titre: String, motif: String },
}

#[derive(Debug)]
struct Entree {
    fenetre: IdFenetre,
    etat: Etat,
    /// Identifiant rendu par le pilote à la création, pour la destruction.
    sortie_pilote: Option<u32>,
    /// Position de la même sortie dans l'énumération DXGI, pour la capture
    /// et le placement.
    dxgi: Option<(u32, u32)>,
    audio: bool,
}

pub struct Table {
    /// Nombre de fenêtres simultanées que la table s'autorise. Le vivier de
    /// sorties du pilote vaut 10 (mesuré), mais Apollo puise au même : la
    /// capacité est un paramètre, pas une constante.
    capacite: usize,
    entrees: HashMap<IdSession, Entree>,
    /// Compteur des sessions attribuées. Croît sans jamais reculer : un
    /// identifiant réutilisé apparierait un message tardif du navigateur à la
    /// mauvaise fenêtre.
    compteur: u64,
    /// Vrai tant qu'aucune fenêtre ne porte le son.
    audio_libre: bool,
}

impl Table {
    pub fn nouvelle(capacite: usize) -> Self {
        Self {
            capacite,
            entrees: HashMap::new(),
            compteur: 0,
            audio_libre: true,
        }
    }

    pub fn etat(&self, session: &IdSession) -> Option<&Etat> {
        self.entrees.get(session).map(|e| &e.etat)
    }

    /// Fenêtre Windows associée à une session.
    ///
    /// Le superviseur en a besoin pour le contrôle périodique de placement :
    /// il connaît la sortie par session, mais c'est la fenêtre qu'il faut
    /// replacer.
    pub fn fenetre_de(&self, session: &IdSession) -> Option<IdFenetre> {
        self.entrees.get(session).map(|e| e.fenetre)
    }

    pub fn fenetre_apparue(&mut self, fenetre: IdFenetre, titre: String) -> Vec<Effet> {
        if self.entrees.len() >= self.capacite {
            return vec![Effet::AnnoncerRefus {
                titre,
                motif: "plus aucune sortie virtuelle disponible".into(),
            }];
        }
        self.compteur += 1;
        let session = IdSession(format!("w-{}", self.compteur));
        // Le son est réservé ici, à la détection, et non au lancement : deux
        // fenêtres détectées coup sur coup ne doivent pas se le voir attribuer
        // toutes les deux parce que aucune n'a encore été lancée.
        let audio = self.audio_libre;
        self.audio_libre = false;
        self.entrees.insert(
            session.clone(),
            Entree {
                fenetre,
                etat: Etat::AttendLeViewport,
                sortie_pilote: None,
                dxgi: None,
                audio,
            },
        );
        vec![Effet::AnnoncerOuverture { session, titre }]
    }

    pub fn viewport_recu(&mut self, session: &IdSession, largeur: u32, hauteur: u32) -> Vec<Effet> {
        // Un message du navigateur est une source externe : tardif, rejoué ou
        // inventé, il ne doit jamais faire avancer la machine deux fois.
        let Some(entree) = self.entrees.get_mut(session) else {
            return Vec::new();
        };
        if entree.etat != Etat::AttendLeViewport {
            return Vec::new();
        }
        entree.etat = Etat::AttendLaSortie;
        vec![Effet::CreerSortie { session: session.clone(), largeur, hauteur }]
    }

    /// `sortie_pilote` est ce que le pilote a rendu à la création (il ne sait
    /// détruire que par là) ; `dxgi` est la position de la même sortie dans
    /// l'énumération DXGI (l'enfant ne sait capturer que par là). Aucune
    /// relation calculable entre les deux : les deux sont retenus.
    pub fn sortie_creee(
        &mut self,
        session: &IdSession,
        sortie_pilote: u32,
        dxgi: (u32, u32),
    ) -> Vec<Effet> {
        let Some(entree) = self.entrees.get_mut(session) else {
            return Vec::new();
        };
        if entree.etat != Etat::AttendLaSortie {
            return Vec::new();
        }
        entree.etat = Etat::Vivante;
        entree.sortie_pilote = Some(sortie_pilote);
        entree.dxgi = Some(dxgi);
        vec![Effet::LancerEnfant {
            session: session.clone(),
            fenetre: entree.fenetre,
            index_adaptateur: dxgi.0,
            index_sortie: dxgi.1,
            audio: entree.audio,
        }]
    }

    /// Position DXGI de la sortie d'une session, pour le contrôle périodique
    /// de placement.
    pub fn sortie_dxgi_de(&self, session: &IdSession) -> Option<(u32, u32)> {
        self.entrees.get(session).and_then(|e| e.dxgi)
    }

    /// Sessions dont l'enfant tourne, pour le contrôle périodique de
    /// placement. Rendues par valeur : l'appelant mute la table pendant
    /// qu'il les parcourt.
    pub fn sessions_vivantes(&self) -> Vec<IdSession> {
        self.entrees
            .iter()
            .filter(|(_, e)| e.etat == Etat::Vivante)
            .map(|(s, _)| s.clone())
            .collect()
    }

    pub fn fenetre_disparue(&mut self, fenetre: IdFenetre) -> Vec<Effet> {
        let Some(session) = self
            .entrees
            .iter()
            .find(|(_, e)| e.fenetre == fenetre)
            .map(|(s, _)| s.clone())
        else {
            return Vec::new();
        };
        let entree = self.entrees.remove(&session).expect("trouvée à l'instant");
        let mut effets = vec![Effet::TuerEnfant { session: session.clone() }];
        // Rien à détruire si la fenêtre s'est fermée avant que sa sortie
        // n'existe : demander au pilote de retirer une sortie qu'il n'a
        // jamais créée ne ferait qu'une erreur de plus au journal.
        if let Some(sortie_pilote) = entree.sortie_pilote {
            effets.push(Effet::DetruireSortie {
                sortie_pilote,
                // `dxgi` est toujours renseigne quand `sortie_pilote` l'est :
                // `sortie_creee` pose les deux ensemble, jamais l'un sans
                // l'autre. Le repli n'est donc pas atteignable.
                dxgi: entree.dxgi.unwrap_or((0, 0)),
            });
        }
        effets.push(Effet::AnnoncerFermeture { session });
        effets
    }

    pub fn enfant_mort(&mut self, session: &IdSession) -> Vec<Effet> {
        // Pas de `TuerEnfant` : il est déjà mort. Mais sa sortie, elle, ne
        // s'est pas détruite toute seule — une sortie virtuelle survit au
        // processus qui l'a créée.
        let Some(entree) = self.entrees.remove(session) else {
            return Vec::new();
        };
        let mut effets = Vec::new();
        if let Some(sortie_pilote) = entree.sortie_pilote {
            effets.push(Effet::DetruireSortie {
                sortie_pilote,
                // `dxgi` est toujours renseigne quand `sortie_pilote` l'est :
                // `sortie_creee` pose les deux ensemble, jamais l'un sans
                // l'autre. Le repli n'est donc pas atteignable.
                dxgi: entree.dxgi.unwrap_or((0, 0)),
            });
        }
        effets.push(Effet::AnnoncerFermeture { session: session.clone() });
        effets
    }
}
```

Ajouter `pub mod table;` dans `agent/src/superviseur.rs`.

- [ ] **Step 4: Lancer les tests et vérifier qu'ils passent**

Run: `cargo test -p agent superviseur::table`
Expected: PASS, 13 tests.

- [ ] **Step 5: Commit**

```bash
git add agent/src/superviseur/table.rs agent/src/superviseur.rs
git commit -m "feat(d1): la table du superviseur, machine a etats pure et testee"
```

---

### Task 4: Promouvoir le pilote d'affichage virtuel hors de `diagnostics/`

Déplacement sans changement de comportement. Le code est déjà de qualité production ; ce qui ne l'est pas, c'est son emplacement — rien en exploitation ne doit dépendre de l'arbre `diagnostics/`.

**Files:**
- Create: `agent/src/moniteurs_virtuels/pilote.rs` (depuis `agent/src/diagnostics/multifenetre/moniteurs.rs`)
- Create: `agent/src/moniteurs_virtuels/sudovda.rs` (depuis `agent/src/diagnostics/multifenetre/sudovda.rs`)
- Create: `agent/src/moniteurs_virtuels/peripherique.rs` (depuis `agent/src/diagnostics/multifenetre/peripherique.rs`)
- Create: `agent/src/moniteurs_virtuels/purge.rs` (depuis `agent/src/diagnostics/multifenetre/purge.rs`)
- Modify: `agent/src/moniteurs_virtuels.rs`
- Modify: `agent/src/diagnostics/multifenetre.rs`
- Modify: tout appelant dans `agent/src/diagnostics/multifenetre/`

**Interfaces:**
- Consumes: rien de neuf
- Produces: `crate::moniteurs_virtuels::pilote::{ouvrir_pilote, PiloteParIoctl}` et `crate::moniteurs_virtuels::purge::purger` — mêmes signatures qu'avant, visibilité élargie de `pub(super)` à `pub(crate)`.

- [ ] **Step 1: Relever l'état de départ des tests**

```bash
cargo test -p agent 2>&1 | tail -5
```

Noter le nombre de tests qui passent. Cette tâche ne doit pas le changer — c'est le seul contrôle disponible pour un déplacement.

- [ ] **Step 2: Déplacer les quatre fichiers**

```bash
git mv agent/src/diagnostics/multifenetre/moniteurs.rs agent/src/moniteurs_virtuels/pilote.rs
git mv agent/src/diagnostics/multifenetre/sudovda.rs agent/src/moniteurs_virtuels/sudovda.rs
git mv agent/src/diagnostics/multifenetre/peripherique.rs agent/src/moniteurs_virtuels/peripherique.rs
git mv agent/src/diagnostics/multifenetre/purge.rs agent/src/moniteurs_virtuels/purge.rs
```

`git mv` et non `mv` : l'historique de ces fichiers porte le raisonnement sur le contrat IOCTL, et il doit suivre.

- [ ] **Step 3: Recâbler les modules**

Dans `agent/src/moniteurs_virtuels.rs`, ajouter en tête de fichier, sous le commentaire de module existant :

```rust
// Glue Windows du pilote SudoVDA, promue depuis `diagnostics/multifenetre/`
// au sous-bloc D1 : ce n'est plus de l'outillage de mesure, c'est le chemin
// par lequel le produit fait paraître ses sorties. Le module parent reste
// hors `#[cfg(windows)]` — c'est ce qui permet à sa garde `Sorties` d'avoir
// des tests, et cette raison n'a pas changé.
#[cfg(windows)]
pub mod peripherique;
#[cfg(windows)]
pub mod pilote;
#[cfg(windows)]
pub mod purge;
#[cfg(windows)]
pub mod sudovda;
```

Dans `agent/src/diagnostics/multifenetre.rs`, retirer les quatre `pub(super) mod` correspondants.

- [ ] **Step 4: Corriger les chemins d'import et les visibilités**

Dans les quatre fichiers déplacés, remplacer `use super::` par `use crate::moniteurs_virtuels::` pour les références entre eux, et `use crate::moniteurs_virtuels::{IdSortie, PiloteAffichageVirtuel}` devient `use super::{IdSortie, PiloteAffichageVirtuel}`. Élargir `pub(super)` en `pub(crate)` sur tout ce que `diagnostics/` appelle encore.

Dans les appelants restants de `diagnostics/multifenetre/` (`montee.rs`, `capture_virtuelle.rs`, `contrat.rs`, `nvenc.rs`, `paralleles.rs`, et le `pointeur_virtuel.rs` de la tâche 1), remplacer `super::moniteurs::` par `crate::moniteurs_virtuels::pilote::` et `super::purge::` par `crate::moniteurs_virtuels::purge::`.

Localiser exhaustivement :

```bash
grep -rn "super::moniteurs\|super::purge\|super::sudovda\|super::peripherique" agent/src/
```

- [ ] **Step 5: Vérifier que rien n'a bougé**

```bash
cargo test -p agent 2>&1 | tail -5          # même nombre qu'au step 1
scripts/build-agent.sh 2>&1 | tee /tmp/build-tache4.log
```

Expected: le même nombre de tests qu'au step 1, et une compilation Windows sans erreur. Un déplacement qui change un compte de tests n'est pas un déplacement.

- [ ] **Step 6: Commit du déplacement, seul**

```bash
git add agent/src/moniteurs_virtuels.rs agent/src/moniteurs_virtuels/ \
        agent/src/diagnostics/multifenetre.rs agent/src/diagnostics/multifenetre/
git commit -m "refactor(d1): promouvoir le pilote d'affichage virtuel hors de diagnostics"
```

Le déplacement est commité **seul**, avant l'ajout de comportement qui suit : c'est ce qui rend son absence d'effet relisible.

- [ ] **Step 7: Écrire le test de la libération à chaud**

`Sorties` ne sait aujourd'hui que créer et tout détruire à sa destruction. D1 a besoin de rendre une sortie **pendant** l'exécution : sans cela le vivier se consomme à chaque ouverture de fenêtre et non par fenêtre simultanée — ouvrir et fermer une application onze fois épuiserait les dix sorties du pilote, et plus aucune fenêtre ne pourrait s'ouvrir, avec un symptôme sans rapport visible avec sa cause.

Dans le module de tests existant de `agent/src/moniteurs_virtuels.rs`, ajouter :

```rust
    #[test]
    fn detruire_rend_la_sortie_au_pilote_et_l_oublie() {
        let pilote = PiloteFactice::default();
        let mut sorties = Sorties::nouvelles(&pilote);
        let a = sorties.creer(1280, 720, 60).unwrap();
        let b = sorties.creer(1600, 900, 60).unwrap();

        sorties.detruire(a).unwrap();
        assert_eq!(*pilote.detruites.borrow(), vec![a]);
        assert_eq!(sorties.nombre(), 1);

        // La garde ne doit pas redétruire `a` : le pilote refuserait, et le
        // journal accuserait une purge due qui n'existe pas.
        drop(sorties);
        assert_eq!(*pilote.detruites.borrow(), vec![a, b]);
    }

    #[test]
    fn detruire_une_sortie_inconnue_echoue_sans_rien_toucher() {
        let pilote = PiloteFactice::default();
        let mut sorties = Sorties::nouvelles(&pilote);
        let a = sorties.creer(1280, 720, 60).unwrap();

        assert!(sorties.detruire(a + 1000).is_err());
        assert!(pilote.detruites.borrow().is_empty());
        assert_eq!(sorties.nombre(), 1, "la sortie légitime reste tenue");
    }

    #[test]
    fn une_destruction_refusee_par_le_pilote_ne_fait_pas_oublier_la_sortie() {
        // Le GUID est la seule prise du projet sur ce moniteur : l'oublier
        // sur échec le rendrait irrécupérable, et la garde ne le retenterait
        // jamais.
        let pilote = PiloteFactice::default();
        let mut sorties = Sorties::nouvelles(&pilote);
        let a = sorties.creer(1280, 720, 60).unwrap();
        *pilote.refuse_les_destructions.borrow_mut() = true;

        assert!(sorties.detruire(a).is_err());
        assert_eq!(sorties.nombre(), 1, "la sortie reste due tant qu'elle n'est pas rendue");
    }
```

Le `PiloteFactice` du fichier doit gagner ce que ces tests lisent — s'il n'a pas déjà `detruites` et `refuse_les_destructions`, les ajouter en `RefCell`, sur le modèle de ses champs existants, et faire échouer `detruire` quand le drapeau est levé.

- [ ] **Step 8: Vérifier que les tests échouent**

Run: `cargo test -p agent moniteurs_virtuels`
Expected: FAIL — `no method named detruire found for struct Sorties`.

- [ ] **Step 9: Écrire la libération**

Dans `agent/src/moniteurs_virtuels.rs`, sur `impl Sorties` :

```rust
    /// Rend une sortie au pilote **pendant** l'exécution, et cesse de la
    /// tenir.
    ///
    /// Sans cette méthode, une sortie n'est rendue qu'à la destruction de la
    /// garde, c'est-à-dire à l'arrêt du superviseur : le vivier du pilote
    /// (dix sorties, mesuré) se consommerait alors à chaque OUVERTURE de
    /// fenêtre et non par fenêtre simultanée, et une dizaine
    /// d'ouvertures-fermetures suffirait à bloquer toute nouvelle fenêtre.
    ///
    /// Sur refus du pilote, la sortie **reste tenue** : elle est encore due,
    /// et la garde la retentera à la destruction. L'oublier ici la rendrait
    /// irrécupérable — le pilote ne retire que par un GUID dont lui seul et
    /// `PiloteParIoctl` gardent la trace.
    pub fn detruire(&mut self, id: IdSortie) -> Result<()> {
        let rang = self
            .creees
            .iter()
            .position(|connu| *connu == id)
            .with_context(|| format!("sortie {id} non tenue par cette garde — rien à rendre"))?;
        self.pilote.detruire(id)?;
        self.creees.remove(rang);
        Ok(())
    }
```

- [ ] **Step 10: Vérifier que les tests passent, et commiter**

```bash
cargo test -p agent moniteurs_virtuels
```

Expected: PASS, dont les trois nouveaux.

```bash
git add agent/src/moniteurs_virtuels.rs
git commit -m "feat(d1): rendre une sortie virtuelle au pilote pendant l'execution"
```

---

### Task 5: Une source vidéo sur une sortie DXGI entière

**Files:**
- Create: `agent/src/windows_source/sortie.rs`
- Modify: `agent/src/windows_source.rs`

**Interfaces:**
- Consumes: `crate::capture::{DesktopCapture, enumerer_sorties, SortieDxgi}`, `crate::geometry::Rect`
- Produces: `WindowsSource::sur_sortie(index_adaptateur: u32, index_sortie: u32, fps: u32, bitrate: u32, clock_origin: Instant) -> Result<WindowsSource>`

**Le point de vigilance de cette tâche.** `WindowsSource::new` calcule sa région par `crop_region(window_rect, dw, dh)` où `window_rect` est en coordonnées **écran** et `dw`/`dh` les dimensions du bureau capturé. Avec `DesktopCapture::sur_sortie`, la texture couvre **cette sortie seule**, dont l'origine dans le bureau virtuel n'est pas (0,0) : la région doit donc être exprimée **relativement à la sortie**, soit `Rect { x: 0, y: 0, width, height }` pour la sortie entière. Passer le rectangle en coordonnées de bureau virtuel donnerait une image décalée, ou vide.

- [ ] **Step 1: Écrire le test qui échoue**

Dans `agent/src/windows_source/sortie.rs`, le calcul de région est extrait en fonction pure pour être testable sans Windows :

```rust
//! Construction d'une `WindowsSource` sur une sortie DXGI entière.
//!
//! C'est le mode du sous-bloc D1 : une fenêtre par sortie virtuelle, donc
//! plus rien à recadrer — la sortie *est* la fenêtre.

use crate::geometry::Rect;

/// Région à capturer dans la texture d'une sortie dupliquée.
///
/// **Relative à la sortie, pas au bureau virtuel.** `DesktopCapture::sur_sortie`
/// rend une texture qui couvre cette sortie seule ; son origine dans l'espace
/// du bureau virtuel (par exemple x=2400 pour une sortie posée à droite du
/// bureau physique) n'y a aucun sens. Passer les coordonnées de bureau
/// donnerait une image décalée ou vide.
///
/// Les dimensions sont alignées sur des valeurs paires : l'encodeur NV12 les
/// exige, et une sortie virtuelle créée à une taille impaire par un viewport
/// impair est un cas réel.
pub fn region_de_sortie(largeur: u32, hauteur: u32) -> Option<Rect> {
    let largeur = largeur & !1;
    let hauteur = hauteur & !1;
    if largeur < 2 || hauteur < 2 {
        return None;
    }
    Some(Rect { x: 0, y: 0, width: largeur, height: hauteur })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_region_couvre_toute_la_sortie_a_partir_de_son_origine_propre() {
        assert_eq!(
            region_de_sortie(1600, 900),
            Some(Rect { x: 0, y: 0, width: 1600, height: 900 })
        );
    }

    #[test]
    fn les_dimensions_impaires_sont_alignees_vers_le_bas() {
        assert_eq!(
            region_de_sortie(1601, 901),
            Some(Rect { x: 0, y: 0, width: 1600, height: 900 })
        );
    }

    #[test]
    fn une_sortie_degeneree_ne_donne_aucune_region() {
        assert_eq!(region_de_sortie(1, 900), None);
        assert_eq!(region_de_sortie(0, 0), None);
    }
}
```

Ajouter `pub mod sortie;` en tête de `agent/src/windows_source.rs`.

- [ ] **Step 2: Lancer les tests et vérifier qu'ils échouent**

Run: `cargo test -p agent windows_source::sortie`
Expected: FAIL — le module n'est pas déclaré, ou `region_de_sortie` est introuvable.

Note : `windows_source.rs` est sous `#[cfg(windows)]` dans `main.rs`. Pour que ces tests tournent sur l'hôte, déplacer la déclaration `mod windows_source;` hors du bloc `#[cfg(windows)]` **n'est pas** la bonne réponse — le reste du fichier ne compile pas sur Linux. Déclarer plutôt le sous-module à part dans `main.rs`, hors cfg :

```rust
// Le calcul de région est pur et doit être testable sur l'hôte : il est donc
// déclaré indépendamment du reste de `windows_source`, qui ne compile que sur
// Windows.
#[path = "windows_source/sortie.rs"]
mod windows_source_sortie;
```

et dans `windows_source.rs`, sous Windows : `use crate::windows_source_sortie as sortie;`

- [ ] **Step 3: Écrire le constructeur**

Dans `agent/src/windows_source.rs`, à côté de `pub fn new` :

```rust
    /// Construit une source capturant une sortie DXGI **entière**.
    ///
    /// Mode du sous-bloc D1 : la fenêtre a sa propre sortie virtuelle, il n'y
    /// a donc plus rien à recadrer ni aucune fenêtre à suivre. `hwnd` reste
    /// renseigné — l'injection d'entrée et le contrôle de vie en ont besoin —
    /// mais il ne sert plus au calcul de la région.
    pub fn sur_sortie(
        hwnd: HWND,
        index_adaptateur: u32,
        index_sortie: u32,
        fps: u32,
        bitrate: u32,
        clock_origin: std::time::Instant,
    ) -> Result<Self> {
        let capture = DesktopCapture::sur_sortie(index_adaptateur, index_sortie)?;
        let (dw, dh) = capture.desktop_size();
        let region = sortie::region_de_sortie(dw, dh).with_context(|| {
            format!("sortie {index_adaptateur}:{index_sortie} de dimensions inexploitables ({dw}x{dh})")
        })?;
        let (width, height) = (region.width, region.height);
        let encoder = H264Encoder::new(capture.device(), width, height, fps, bitrate)?;
        Ok(Self::depuis_pieces(hwnd, capture, encoder, region, width, height, clock_origin))
    }
```

`Self::depuis_pieces` n'existe pas encore : c'est l'assemblage final de `new` — le `Ok(Self { hwnd, capture: Some(capture), encoder: Some(encoder), region, width, height, clock_origin, … })` et tout ce que `new` initialise après avoir construit sa capture et son encodeur. **Extraire ce bloc tel quel** en fonction privée, et faire appeler cette fonction par `new` comme par `sur_sortie` :

```rust
    /// Assemblage final, partagé par les deux constructeurs.
    ///
    /// Extrait pour que `new` (capture du bureau + recadrage de la fenêtre) et
    /// `sur_sortie` (capture d'une sortie entière) ne divergent pas sur
    /// l'initialisation des champs — ils ne diffèrent que par la façon
    /// d'obtenir la capture, l'encodeur et la région.
    fn depuis_pieces(
        hwnd: HWND,
        capture: DesktopCapture,
        encoder: H264Encoder,
        region: Rect,
        width: u32,
        height: u32,
        clock_origin: std::time::Instant,
    ) -> Self {
        // Corps : exactement le `Self { … }` que `new` construisait, sans
        // aucune modification de valeur. Si un champ de `WindowsSource` a
        // besoin d'une valeur qui n'est pas dans cette liste de paramètres,
        // l'ajouter en paramètre plutôt que le recalculer ici.
    }
```

Vérifier après extraction que `new` se termine bien par `Ok(Self::depuis_pieces(...))` et ne construit plus aucun champ directement : c'est le seul contrôle que l'extraction n'a rien changé.

- [ ] **Step 4: Lancer les tests et compiler**

```bash
cargo test -p agent windows_source_sortie   # PASS, 3 tests
scripts/build-agent.sh 2>&1 | tee /tmp/build-tache5.log
```

- [ ] **Step 5: Commit**

```bash
git add agent/src/windows_source.rs agent/src/windows_source/sortie.rs agent/src/main.rs
git commit -m "feat(d1): source video sur une sortie DXGI entiere"
```

---

### Task 6: Le hook de détection des fenêtres

**Files:**
- Create: `agent/src/superviseur/hook.rs`
- Modify: `agent/src/superviseur.rs`

**Interfaces:**
- Consumes: `superviseur::fenetres::{DescriptionFenetre, merite_une_fenetre}`, `superviseur::table::IdFenetre`
- Produces:
  - `superviseur::hook::EvenementFenetre` — `Apparue { fenetre: IdFenetre, titre: String }` ou `Disparue { fenetre: IdFenetre }`
  - `superviseur::hook::poser(tx: std::sync::mpsc::Sender<EvenementFenetre>) -> Result<Hook>` — pose le hook sur un fil dédié avec sa pompe de messages, et rend une garde qui le retire à la destruction
  - `superviseur::hook::decrire(hwnd: HWND) -> Option<DescriptionFenetre>`
  - `superviseur::hook::enumerer_existantes() -> Vec<(IdFenetre, String)>`

- [ ] **Step 1: Écrire le module**

```rust
//! Détection des fenêtres par `SetWinEventHook`, et sa pompe de messages.
//!
//! **Le hook `WINEVENT_OUTOFCONTEXT` n'appelle son rappel que depuis un fil
//! qui pompe des messages.** Sans `GetMessageW` en boucle, le hook se pose
//! sans erreur et ne se déclenche jamais — c'est le mode de défaillance
//! muet de cette API, et la raison du fil dédié.
//!
//! Le rappel ne fait qu'une chose : traduire et envoyer. Aucune décision n'est
//! prise ici (voir `fenetres`), aucun état n'y est tenu (voir `table`) : un
//! rappel de hook global s'exécute dans un contexte contraint, et tout ce
//! qu'on peut y faire de long retarde tout le bureau.

#![cfg(windows)]

use std::sync::mpsc::Sender;
use std::sync::Mutex;

use anyhow::{anyhow, Result};
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM, TRUE};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, EnumWindows, GetMessageW, GetWindow, GetWindowLongPtrW, GetWindowTextLengthW,
    GetWindowTextW, IsWindowVisible, PostThreadMessageW, TranslateMessage, EVENT_OBJECT_DESTROY,
    EVENT_OBJECT_HIDE, EVENT_OBJECT_SHOW, GWL_EXSTYLE, GW_OWNER, MSG, OBJID_WINDOW, WINEVENT_OUTOFCONTEXT,
    WM_QUIT, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
};

use super::fenetres::{merite_une_fenetre, DescriptionFenetre};
use super::table::IdFenetre;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvenementFenetre {
    Apparue { fenetre: IdFenetre, titre: String },
    Disparue { fenetre: IdFenetre },
}

/// Émetteur global du rappel.
///
/// Un rappel `extern "system"` ne porte aucune donnée utilisateur : Windows ne
/// passe rien qui nous appartienne. C'est la raison de ce global, et non un
/// choix de commodité. Il est écrit une fois par `poser` et lu par le rappel.
static EMETTEUR: Mutex<Option<Sender<EvenementFenetre>>> = Mutex::new(None);

/// Garde : retire le hook et arrête la pompe à la destruction.
pub struct Hook {
    hook: HWINEVENTHOOK,
    fil: Option<std::thread::JoinHandle<()>>,
    fil_id: u32,
}

impl Drop for Hook {
    fn drop(&mut self) {
        unsafe {
            let _ = UnhookWinEvent(self.hook);
            // Réveiller la pompe pour qu'elle sorte de `GetMessageW`, sans
            // quoi le fil ne se termine jamais et la jointure ci-dessous
            // bloquerait indéfiniment.
            let _ = PostThreadMessageW(self.fil_id, WM_QUIT, Default::default(), Default::default());
        }
        if let Some(fil) = self.fil.take() {
            let _ = fil.join();
        }
        *EMETTEUR.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

/// Relève l'état d'une fenêtre, pour le soumettre au critère de `fenetres`.
///
/// `None` si la fenêtre a déjà disparu entre l'événement et cet appel — cas
/// courant et normal, pas une erreur.
pub fn decrire(hwnd: HWND) -> Option<DescriptionFenetre> {
    unsafe {
        let longueur = GetWindowTextLengthW(hwnd);
        let titre = if longueur > 0 {
            let mut tampon = vec![0u16; longueur as usize + 1];
            let ecrits = GetWindowTextW(hwnd, &mut tampon);
            if ecrits > 0 {
                String::from_utf16_lossy(&tampon[..ecrits as usize])
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let styles = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let mut masquee: u32 = 0;
        let _ = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut masquee as *mut _ as *mut _,
            std::mem::size_of::<u32>() as u32,
        );

        Some(DescriptionFenetre {
            visible: IsWindowVisible(hwnd).as_bool(),
            a_un_proprietaire: GetWindow(hwnd, GW_OWNER).is_ok_and(|o| !o.is_invalid()),
            tool_window: styles & WS_EX_TOOLWINDOW.0 != 0,
            app_window: styles & WS_EX_APPWINDOW.0 != 0,
            masquee_dwm: masquee != 0,
            titre,
        })
    }
}

unsafe extern "system" fn rappel(
    _hook: HWINEVENTHOOK,
    evenement: u32,
    hwnd: HWND,
    id_objet: i32,
    _id_enfant: i32,
    _fil: u32,
    _instant: u32,
) {
    // `OBJID_WINDOW` seul : sans ce filtre, chaque contrôle enfant, chaque
    // barre de défilement et chaque curseur remontent ici.
    if id_objet != OBJID_WINDOW.0 || hwnd.is_invalid() {
        return;
    }
    let message = match evenement {
        EVENT_OBJECT_SHOW => {
            let description = match decrire(hwnd) {
                Some(d) if merite_une_fenetre(&d) => d,
                _ => return,
            };
            EvenementFenetre::Apparue {
                fenetre: IdFenetre(hwnd.0 as u64),
                titre: description.titre,
            }
        }
        // `HIDE` autant que `DESTROY` : une fenêtre masquée ne se distingue
        // pas d'une fenêtre fermée du point de vue de l'utilisateur, et une
        // application qui masque sa fenêtre principale au lieu de la détruire
        // (barre de notification) laisserait sinon un flux vivant sur une
        // fenêtre invisible.
        EVENT_OBJECT_HIDE | EVENT_OBJECT_DESTROY => {
            EvenementFenetre::Disparue { fenetre: IdFenetre(hwnd.0 as u64) }
        }
        _ => return,
    };
    if let Some(tx) = EMETTEUR.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        // L'échec d'envoi signifie que le superviseur s'arrête : rien à
        // journaliser depuis un rappel de hook global.
        let _ = tx.send(message);
    }
}

/// Énumère les fenêtres déjà ouvertes au démarrage du superviseur.
///
/// Le hook ne rapporte que les changements : sans cette énumération, les
/// fenêtres antérieures au superviseur n'existeraient jamais pour lui.
pub fn enumerer_existantes() -> Vec<(IdFenetre, String)> {
    let mut trouvees: Vec<(IdFenetre, String)> = Vec::new();
    unsafe {
        let _ = EnumWindows(
            Some(rappel_enumeration),
            LPARAM(&mut trouvees as *mut Vec<(IdFenetre, String)> as isize),
        );
    }
    trouvees
}

unsafe extern "system" fn rappel_enumeration(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let trouvees = &mut *(lparam.0 as *mut Vec<(IdFenetre, String)>);
    if let Some(d) = decrire(hwnd) {
        if merite_une_fenetre(&d) {
            trouvees.push((IdFenetre(hwnd.0 as u64), d.titre));
        }
    }
    TRUE
}

/// Pose le hook global et lance sa pompe de messages sur un fil dédié.
pub fn poser(tx: Sender<EvenementFenetre>) -> Result<Hook> {
    *EMETTEUR.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);

    let (prete, attendre) = std::sync::mpsc::channel::<Result<(isize, u32), String>>();
    let fil = std::thread::spawn(move || {
        let hook = unsafe {
            SetWinEventHook(
                EVENT_OBJECT_DESTROY,
                EVENT_OBJECT_SHOW,
                None,
                Some(rappel),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            )
        };
        if hook.is_invalid() {
            let _ = prete.send(Err("SetWinEventHook a échoué".into()));
            return;
        }
        let id = unsafe { windows::Win32::System::Threading::GetCurrentThreadId() };
        let _ = prete.send(Ok((hook.0 as isize, id)));

        // La pompe. `GetMessageW` rend 0 sur `WM_QUIT` : c'est ainsi que
        // `Hook::drop` fait sortir ce fil.
        let mut message = MSG::default();
        while unsafe { GetMessageW(&mut message, None, 0, 0) }.as_bool() {
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    });

    match attendre.recv() {
        Ok(Ok((hook, fil_id))) => Ok(Hook {
            hook: HWINEVENTHOOK(hook as *mut core::ffi::c_void),
            fil: Some(fil),
            fil_id,
        }),
        Ok(Err(e)) => Err(anyhow!(e)),
        Err(_) => Err(anyhow!("le fil du hook s'est terminé avant de rendre son état")),
    }
}
```

Ajouter dans `agent/src/superviseur.rs` :

```rust
#[cfg(windows)]
pub mod hook;
```

- [ ] **Step 2: Compiler sur la VM**

```bash
scripts/build-agent.sh 2>&1 | tee /tmp/build-tache6.log
```

Expected: compilation sans erreur.

- [ ] **Step 3: Éprouver le hook par une sonde**

Le hook ne se teste pas sur l'hôte. Ajouter dans `agent/src/diagnostics/multifenetre.rs`, à côté des autres branches :

```rust
    if std::env::var("SUPERVISEUR_HOOK").is_ok() {
        let (tx, rx) = std::sync::mpsc::channel();
        for (fenetre, titre) in crate::superviseur::hook::enumerer_existantes() {
            tracing::info!(id = fenetre.0, titre, "fenêtre déjà ouverte");
        }
        let _garde = crate::superviseur::hook::poser(tx)?;
        tracing::info!("hook posé — ouvrez et fermez des fenêtres pendant 60 s");
        let fin = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while std::time::Instant::now() < fin {
            match rx.recv_timeout(std::time::Duration::from_millis(500)) {
                Ok(evenement) => tracing::info!(?evenement, "événement de fenêtre"),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(e) => { tracing::warn!(erreur = %e, "canal du hook rompu"); break; }
            }
        }
        return Ok(true);
    }
```

Puis :

```bash
scripts/build-agent.sh && SUPERVISEUR_HOOK=1 scripts/run-agent.sh
```

Pendant les 60 s : ouvrir le Bloc-notes, ouvrir un menu contextuel, ouvrir une boîte de dialogue, fermer le Bloc-notes.

**Attendu** : `Apparue` pour le Bloc-notes, `Disparue` à sa fermeture, et **aucun événement pour le menu contextuel ni pour la boîte de dialogue** — c'est ce qui prouve que le filtrage fait son travail sur des fenêtres réelles et pas seulement sur les descriptions factices de la tâche 2.

Verser le journal dans `docs/superpowers/plans/journaux-multifenetres-d1/hook.log`.

- [ ] **Step 4: Commit**

```bash
git add agent/src/superviseur/hook.rs agent/src/superviseur.rs \
        agent/src/diagnostics/multifenetre.rs \
        docs/superpowers/plans/journaux-multifenetres-d1/hook.log
git commit -m "feat(d1): detection des fenetres par SetWinEventHook, avec sa pompe"
```

---

### Task 7: Poser une fenêtre sur sa sortie

**Files:**
- Create: `agent/src/superviseur/placement.rs`
- Modify: `agent/src/superviseur.rs`

**Interfaces:**
- Consumes: `crate::capture::{enumerer_sorties, SortieDxgi}`, `crate::geometry::Rect`
- Produces:
  - `superviseur::placement::sortie_par_dimensions(sorties: &[SortieDxgi], largeur: u32, hauteur: u32, deja_prises: &[(u32, u32)]) -> Option<SortieDxgi>` — pur, testé
  - `superviseur::placement::poser(hwnd: HWND, cible: &Rect) -> Result<()>` — glue Windows

**Le problème que cette tâche résout.** Le pilote rend un `IdSortie` qui lui est propre ; DXGI énumère des sorties par `(index_adaptateur, index_sortie)`. **Rien ne relie les deux.** L'appariement se fait donc par dimensions et par élimination : une sortie fraîchement créée à 1600×900 est la sortie attachée de 1600×900 qu'aucune fenêtre ne s'est encore vu attribuer. C'est fragile si deux fenêtres demandent le même viewport — d'où le paramètre `deja_prises`.

- [ ] **Step 1: Écrire les tests qui échouent**

Dans `agent/src/superviseur/placement.rs` :

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Rect;

    fn sortie(a: u32, s: u32, x: i32, l: u32, h: u32, attachee: bool) -> SortieDxgi {
        SortieDxgi {
            index_adaptateur: a,
            index_sortie: s,
            adaptateur: "NVIDIA".into(),
            nom_sortie: format!("\\\\.\\DISPLAY{s}"),
            attachee_au_bureau: attachee,
            rect: Rect { x, y: 0, width: l, height: h },
        }
    }

    #[test]
    fn trouve_la_sortie_aux_dimensions_demandees() {
        let toutes = vec![
            sortie(0, 0, 0, 2400, 1080, true),
            sortie(0, 1, 2400, 1600, 900, true),
        ];
        let trouvee = sortie_par_dimensions(&toutes, 1600, 900, &[]).unwrap();
        assert_eq!((trouvee.index_adaptateur, trouvee.index_sortie), (0, 1));
    }

    #[test]
    fn ignore_une_sortie_non_attachee() {
        // Une sortie créée mais que Windows n'a pas encore rattachée ne peut
        // rien afficher : la prendre donnerait une capture noire.
        let toutes = vec![sortie(0, 1, 2400, 1600, 900, false)];
        assert!(sortie_par_dimensions(&toutes, 1600, 900, &[]).is_none());
    }

    #[test]
    fn ignore_une_sortie_deja_attribuee() {
        // Deux fenêtres au même viewport : sans ce filtre, la seconde se
        // verrait attribuer la sortie de la première, et les deux flux
        // montreraient la même image.
        let toutes = vec![
            sortie(0, 1, 2400, 1600, 900, true),
            sortie(0, 2, 4000, 1600, 900, true),
        ];
        let trouvee = sortie_par_dimensions(&toutes, 1600, 900, &[(0, 1)]).unwrap();
        assert_eq!((trouvee.index_adaptateur, trouvee.index_sortie), (0, 2));
    }

    #[test]
    fn ne_trouve_rien_quand_toutes_sont_prises() {
        let toutes = vec![sortie(0, 1, 2400, 1600, 900, true)];
        assert!(sortie_par_dimensions(&toutes, 1600, 900, &[(0, 1)]).is_none());
    }

    #[test]
    fn n_apparie_pas_une_sortie_aux_mauvaises_dimensions() {
        // Le facteur d'échelle DPI a déjà produit un écart de 1,5 sur ce
        // terrain (5120x1440 annoncé, 3413x960 mesuré) : un appariement
        // approximatif rendrait ce piège invisible.
        let toutes = vec![sortie(0, 1, 2400, 1067, 600, true)];
        assert!(sortie_par_dimensions(&toutes, 1600, 900, &[]).is_none());
    }

    #[test]
    fn une_fenetre_a_sa_place_n_est_pas_replacee() {
        let cible = Rect { x: 2400, y: 0, width: 1600, height: 900 };
        assert!(!doit_etre_replacee(&cible, &cible));
    }

    #[test]
    fn une_fenetre_deplacee_hors_de_sa_sortie_est_replacee() {
        let cible = Rect { x: 2400, y: 0, width: 1600, height: 900 };
        let ailleurs = Rect { x: 100, y: 50, width: 1600, height: 900 };
        assert!(doit_etre_replacee(&ailleurs, &cible));
    }

    #[test]
    fn une_fenetre_retaillee_par_l_application_est_replacee() {
        let cible = Rect { x: 2400, y: 0, width: 1600, height: 900 };
        let retaillee = Rect { x: 2400, y: 0, width: 800, height: 600 };
        assert!(doit_etre_replacee(&retaillee, &cible));
    }

    #[test]
    fn un_ecart_d_un_pixel_ne_declenche_pas_de_replacement() {
        // Les bordures invisibles de DWM décalent couramment le rectangle
        // rendu par `GetWindowRect` de un ou deux pixels. Sans tolérance, le
        // superviseur replacerait la fenêtre à chaque tour de boucle, en
        // boucle, et volerait le focus indéfiniment.
        let cible = Rect { x: 2400, y: 0, width: 1600, height: 900 };
        let presque = Rect { x: 2401, y: 1, width: 1599, height: 899 };
        assert!(!doit_etre_replacee(&presque, &cible));
    }
}
```

- [ ] **Step 2: Lancer les tests et vérifier qu'ils échouent**

Run: `cargo test -p agent superviseur::placement`
Expected: FAIL — `sortie_par_dimensions` introuvable.

- [ ] **Step 3: Écrire l'implémentation**

En tête de `agent/src/superviseur/placement.rs` :

```rust
//! Apparier une sortie virtuelle fraîchement créée à une sortie DXGI, puis y
//! poser la fenêtre.
//!
//! **Le pilote et DXGI ne parlent pas le même langage.** Le premier rend un
//! identifiant de cible qui lui appartient, le second énumère par
//! `(index_adaptateur, index_sortie)`. Aucune correspondance n'est exposée :
//! l'appariement se fait donc par dimensions et par élimination.
//!
//! **`GetDesc`/`DesktopCoordinates` est la source de vérité, jamais WMI** —
//! le champ WMI a été vu périmé de 68 s sur ce terrain, et la sortie virtuelle
//! y était annoncée 5120×1440 quand DXGI la mesurait 3413×960 (facteur DPI de
//! 1,5). Un placement calculé sur la valeur WMI serait décalé d'autant.

use anyhow::{Context, Result};

use crate::capture::SortieDxgi;
use crate::geometry::Rect;

/// Sortie DXGI correspondant à des dimensions demandées, parmi celles qui ne
/// sont pas déjà attribuées.
///
/// L'égalité des dimensions est **exacte** : un appariement approximatif
/// masquerait le piège du facteur d'échelle décrit en tête de module.
pub fn sortie_par_dimensions(
    sorties: &[SortieDxgi],
    largeur: u32,
    hauteur: u32,
    deja_prises: &[(u32, u32)],
) -> Option<SortieDxgi> {
    sorties
        .iter()
        .find(|s| {
            s.attachee_au_bureau
                && s.rect.width == largeur
                && s.rect.height == hauteur
                && !deja_prises.contains(&(s.index_adaptateur, s.index_sortie))
        })
        .cloned()
}

/// Tolérance de position et de taille, en pixels, avant de replacer.
///
/// Les bordures invisibles de DWM décalent couramment `GetWindowRect` de un ou
/// deux pixels par rapport à ce que `SetWindowPos` a demandé. Sans tolérance,
/// le superviseur replacerait la fenêtre à chaque tour de boucle.
const TOLERANCE_PX: i64 = 4;

/// Vrai si la fenêtre a quitté sa sortie ou changé de taille au point qu'il
/// faille la remettre en place.
///
/// C'est le cas que la spec §6 prévoit : une application peut se déplacer ou
/// se retailler d'elle-même, et une fenêtre qui déborde de sa sortie donne une
/// capture tronquée sans que rien ne le signale.
pub fn doit_etre_replacee(actuel: &Rect, cible: &Rect) -> bool {
    let ecart = |a: i64, b: i64| (a - b).abs() > TOLERANCE_PX;
    ecart(actuel.x as i64, cible.x as i64)
        || ecart(actuel.y as i64, cible.y as i64)
        || ecart(actuel.width as i64, cible.width as i64)
        || ecart(actuel.height as i64, cible.height as i64)
}
```

Puis la glue Windows, dans le même fichier :

```rust
#[cfg(windows)]
mod win {
    use super::*;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, ShowWindow, HWND_TOP, SM_CXSCREEN, SWP_NOACTIVATE, SW_SHOWNORMAL,
    };

    /// Pose la fenêtre sur la sortie et lui donne exactement sa taille.
    ///
    /// **Pas de maximisation.** `SW_MAXIMIZE` ferait adopter à la fenêtre la
    /// zone de travail du moniteur, barre des tâches déduite : l'image
    /// capturée ne remplirait alors pas la sortie, et le bas du flux serait
    /// une bande de bureau vide. On pose la taille exacte de la sortie.
    ///
    /// La fenêtre est d'abord restaurée : une fenêtre minimisée ou déjà
    /// maximisée ignore silencieusement `SetWindowPos`.
    pub fn poser(hwnd: HWND, cible: &Rect) -> Result<()> {
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
            SetWindowPos(
                hwnd,
                Some(HWND_TOP),
                cible.x,
                cible.y,
                cible.width as i32,
                cible.height as i32,
                // `SWP_NOACTIVATE` : poser une fenêtre ne doit pas voler le
                // premier plan à celle que l'utilisateur manipule.
                SWP_NOACTIVATE,
            )
            .context("SetWindowPos vers la sortie virtuelle")?;
        }
        Ok(())
    }

    /// Rectangle actuel de la fenêtre, en coordonnées du bureau virtuel.
    ///
    /// `GetWindowRect` et non `GetClientRect` : c'est la position dans
    /// l'espace du bureau qu'on compare à celle de la sortie, et
    /// `GetClientRect` rend un rectangle dont l'origine est toujours (0,0).
    pub fn rectangle_de(hwnd: HWND) -> Result<Rect> {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;
        let mut r = RECT::default();
        unsafe { GetWindowRect(hwnd, &mut r) }.context("GetWindowRect")?;
        Ok(Rect {
            x: r.left,
            y: r.top,
            width: (r.right - r.left).max(0) as u32,
            height: (r.bottom - r.top).max(0) as u32,
        })
    }
}

#[cfg(windows)]
pub use win::{poser, rectangle_de};
```

Ajouter `pub mod placement;` dans `agent/src/superviseur.rs`.

- [ ] **Step 4: Lancer les tests et compiler**

```bash
cargo test -p agent superviseur::placement    # PASS, 9 tests
scripts/build-agent.sh 2>&1 | tee /tmp/build-tache7.log
```

- [ ] **Step 5: Commit**

```bash
git add agent/src/superviseur/placement.rs agent/src/superviseur.rs
git commit -m "feat(d1): apparier une sortie virtuelle a DXGI et y poser la fenetre"
```

---

### Task 8: Lancer et surveiller les enfants

**Files:**
- Create: `agent/src/superviseur/enfants.rs`
- Modify: `agent/src/superviseur.rs`

**Interfaces:**
- Consumes: `superviseur::table::IdSession`
- Produces:
  - `superviseur::enfants::{Lanceur, Enfants, Consigne}`
  - `trait Lanceur { fn lancer(&self, consigne: &Consigne) -> Result<u32>; fn est_vivant(&self, pid: u32) -> bool; fn tuer(&self, pid: u32) -> Result<()>; }`
  - `Enfants::nouveaux(lanceur) -> Enfants`
  - `Enfants::lancer(&mut self, consigne: Consigne) -> Result<()>`
  - `Enfants::tuer(&mut self, session: &IdSession)`
  - `Enfants::morts(&mut self) -> Vec<IdSession>`

- [ ] **Step 1: Écrire les tests qui échouent**

Dans `agent/src/superviseur/enfants.rs` :

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// Lanceur factice : retient ce qu'on lui demande, sans lancer aucun
    /// processus. C'est ce qui rend cette machinerie éprouvable sur l'hôte.
    #[derive(Default)]
    struct LanceurFactice {
        lancees: RefCell<Vec<Consigne>>,
        tues: RefCell<Vec<u32>>,
        vivants: RefCell<Vec<u32>>,
        prochain_pid: RefCell<u32>,
    }

    impl Lanceur for LanceurFactice {
        fn lancer(&self, consigne: &Consigne) -> anyhow::Result<u32> {
            let mut pid = self.prochain_pid.borrow_mut();
            *pid += 1;
            self.lancees.borrow_mut().push(consigne.clone());
            self.vivants.borrow_mut().push(*pid);
            Ok(*pid)
        }
        fn est_vivant(&self, pid: u32) -> bool {
            self.vivants.borrow().contains(&pid)
        }
        fn tuer(&self, pid: u32) -> anyhow::Result<()> {
            self.tues.borrow_mut().push(pid);
            self.vivants.borrow_mut().retain(|p| *p != pid);
            Ok(())
        }
    }

    fn consigne(session: &str, audio: bool) -> Consigne {
        Consigne {
            session: IdSession(session.into()),
            fenetre: 0x1234,
            index_adaptateur: 0,
            index_sortie: 1,
            audio,
        }
    }

    #[test]
    fn lancer_transmet_la_consigne_au_lanceur() {
        let lanceur = LanceurFactice::default();
        let mut enfants = Enfants::nouveaux(&lanceur);
        enfants.lancer(consigne("w-1", true)).unwrap();
        assert_eq!(lanceur.lancees.borrow().len(), 1);
        assert!(lanceur.lancees.borrow()[0].audio);
    }

    #[test]
    fn tuer_demande_la_mise_a_mort_du_bon_processus() {
        let lanceur = LanceurFactice::default();
        let mut enfants = Enfants::nouveaux(&lanceur);
        enfants.lancer(consigne("w-1", true)).unwrap();
        enfants.lancer(consigne("w-2", false)).unwrap();
        enfants.tuer(&IdSession("w-1".into()));
        assert_eq!(*lanceur.tues.borrow(), vec![1]);
    }

    #[test]
    fn morts_rend_les_sessions_dont_le_processus_a_disparu() {
        let lanceur = LanceurFactice::default();
        let mut enfants = Enfants::nouveaux(&lanceur);
        enfants.lancer(consigne("w-1", true)).unwrap();
        enfants.lancer(consigne("w-2", false)).unwrap();
        lanceur.vivants.borrow_mut().retain(|p| *p != 1);

        assert_eq!(enfants.morts(), vec![IdSession("w-1".into())]);
    }

    #[test]
    fn une_session_morte_n_est_signalee_qu_une_fois() {
        // Sans cette garantie, le superviseur détruirait la sortie une
        // première fois puis en redemanderait la destruction à chaque tour
        // de boucle, et le journal se remplirait d'échecs.
        let lanceur = LanceurFactice::default();
        let mut enfants = Enfants::nouveaux(&lanceur);
        enfants.lancer(consigne("w-1", true)).unwrap();
        lanceur.vivants.borrow_mut().clear();

        assert_eq!(enfants.morts().len(), 1);
        assert!(enfants.morts().is_empty());
    }

    #[test]
    fn tuer_une_session_inconnue_ne_fait_rien() {
        let lanceur = LanceurFactice::default();
        let mut enfants = Enfants::nouveaux(&lanceur);
        enfants.tuer(&IdSession("w-jamais-lancee".into()));
        assert!(lanceur.tues.borrow().is_empty());
    }

    #[test]
    fn une_session_tuee_ne_ressort_pas_dans_les_morts() {
        // Elle a déjà été traitée par le chemin `fenetre_disparue` : la
        // signaler morte ferait détruire sa sortie une seconde fois.
        let lanceur = LanceurFactice::default();
        let mut enfants = Enfants::nouveaux(&lanceur);
        enfants.lancer(consigne("w-1", true)).unwrap();
        enfants.tuer(&IdSession("w-1".into()));
        assert!(enfants.morts().is_empty());
    }
}
```

- [ ] **Step 2: Lancer les tests et vérifier qu'ils échouent**

Run: `cargo test -p agent superviseur::enfants`
Expected: FAIL — `Enfants`, `Lanceur`, `Consigne` introuvables.

- [ ] **Step 3: Écrire l'implémentation**

En tête de `agent/src/superviseur/enfants.rs` :

```rust
//! Lancer un processus agent par fenêtre, et savoir lequel est mort.
//!
//! Le lancement passe par un trait : c'est ce qui rend la comptabilité — qui
//! tourne, qui vient de mourir, qui a déjà été signalé — éprouvable sur
//! l'hôte, alors qu'elle porte les erreurs qui feraient fuir une sortie
//! virtuelle.

use std::collections::HashMap;

use anyhow::Result;

use super::table::IdSession;

/// Ce qu'un enfant doit savoir pour démarrer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Consigne {
    pub session: IdSession,
    /// `HWND` de la fenêtre, sous forme d'adresse brute — un `HWND` n'est pas
    /// `Send` en windows-rs 0.62, et c'est un identifiant opaque, pas un
    /// pointeur déréférencé.
    pub fenetre: u64,
    pub index_adaptateur: u32,
    pub index_sortie: u32,
    /// Vrai pour la seule fenêtre porteuse du son.
    pub audio: bool,
}

pub trait Lanceur {
    fn lancer(&self, consigne: &Consigne) -> Result<u32>;
    fn est_vivant(&self, pid: u32) -> bool;
    fn tuer(&self, pid: u32) -> Result<()>;
}

pub struct Enfants<'l> {
    lanceur: &'l dyn Lanceur,
    vivants: HashMap<IdSession, u32>,
}

impl<'l> Enfants<'l> {
    pub fn nouveaux(lanceur: &'l dyn Lanceur) -> Self {
        Self { lanceur, vivants: HashMap::new() }
    }

    pub fn lancer(&mut self, consigne: Consigne) -> Result<()> {
        let pid = self.lanceur.lancer(&consigne)?;
        tracing::info!(
            session = %consigne.session.0,
            pid,
            sortie = format!("{}:{}", consigne.index_adaptateur, consigne.index_sortie),
            audio = consigne.audio,
            "enfant lancé"
        );
        self.vivants.insert(consigne.session, pid);
        Ok(())
    }

    /// Retire la session de la comptabilité **avant** de tuer : quoi qu'il
    /// advienne de la mise à mort, cette session ne doit plus ressortir comme
    /// « morte » et faire détruire sa sortie une seconde fois.
    ///
    /// **La mise à mort est immédiate, sans arrêt gracieux préalable, et c'est
    /// délibéré.** La spec parle d'un enfant « tué après un délai borné » : ce
    /// délai serait celui d'un arrêt propre qu'on attendrait. Or l'arrêt propre
    /// d'un agent passe par la destruction de son encodeur, où `IMFShutdown::
    /// Shutdown` n'est borné par rien et où un gel a été observé (1 fois sur 6
    /// à N=4, cause non attribuée). Attendre cet arrêt, c'est réintroduire dans
    /// le superviseur le risque même que le multi-processus écarte. La fenêtre
    /// Windows a déjà disparu quand on arrive ici : l'enfant n'a plus rien à
    /// sauvegarder, et le système récupère ses ressources.
    pub fn tuer(&mut self, session: &IdSession) {
        let Some(pid) = self.vivants.remove(session) else {
            return;
        };
        if let Err(erreur) = self.lanceur.tuer(pid) {
            tracing::warn!(session = %session.0, pid, %erreur, "mise à mort de l'enfant échouée");
        }
    }

    /// Sessions dont le processus a disparu depuis le dernier appel.
    ///
    /// Elles quittent la comptabilité au passage : une mort ne se signale
    /// qu'une fois.
    pub fn morts(&mut self) -> Vec<IdSession> {
        let morts: Vec<IdSession> = self
            .vivants
            .iter()
            .filter(|(_, pid)| !self.lanceur.est_vivant(**pid))
            .map(|(session, _)| session.clone())
            .collect();
        for session in &morts {
            let pid = self.vivants.remove(session);
            tracing::warn!(session = %session.0, ?pid, "enfant mort de lui-même");
        }
        morts
    }
}
```

Ajouter `pub mod enfants;` dans `agent/src/superviseur.rs`.

- [ ] **Step 4: Lancer les tests et vérifier qu'ils passent**

Run: `cargo test -p agent superviseur::enfants`
Expected: PASS, 6 tests.

- [ ] **Step 5: Commit**

```bash
git add agent/src/superviseur/enfants.rs agent/src/superviseur.rs
git commit -m "feat(d1): lancement et surveillance des processus enfants"
```

---

### Task 9: Le signaling — types relayés et mémorisation de l'offre

**Files:**
- Modify: `signaling/src/server.ts`
- Modify: `signaling/src/server.test.ts`

**Interfaces:**
- Produces: le serveur relaie désormais `fenetre-ouverte`, `fenetre-fermee`, `viewport`, `refus` en plus de `offer`/`answer`, et délivre à un agent qui se déclare la dernière offre reçue pour sa session.

- [ ] **Step 1: Écrire les tests qui échouent**

Dans `signaling/src/server.test.ts`, ajouter :

```typescript
    it("relaie les messages de la session de contrôle entre superviseur et shell", async () => {
        const serveur = createSignalingServer(0);
        const agent = await connecter(serveur.port, { role: 'agent', session: 'bureau' });
        const client = await connecter(serveur.port, { role: 'client', session: 'bureau' });

        agent.socket.send(JSON.stringify({
            type: 'fenetre-ouverte', session: 'w-1', titre: 'Bloc-notes',
        }));
        const recu = await prochainMessage(client, 'fenetre-ouverte');
        expect(recu).toMatchObject({ type: 'fenetre-ouverte', session: 'w-1', titre: 'Bloc-notes' });

        client.socket.send(JSON.stringify({
            type: 'viewport', session: 'w-1', largeur: 1600, hauteur: 900,
        }));
        const viewport = await prochainMessage(agent, 'viewport');
        expect(viewport).toMatchObject({ type: 'viewport', session: 'w-1', largeur: 1600, hauteur: 900 });

        await serveur.close();
    });

    it("délivre à l'agent l'offre arrivée avant lui", async () => {
        // Le cas de D1 : la page ouvre sa connexion et envoie son offre AVANT
        // que le superviseur n'ait lancé son enfant. Sans mémorisation,
        // l'offre tombait dans le vide et la session ne s'établissait jamais.
        const serveur = createSignalingServer(0);
        const client = await connecter(serveur.port, { role: 'client', session: 'w-tardive' });
        client.socket.send(JSON.stringify({ type: 'offer', sdp: 'v=0 offre-du-client' }));

        // L'agent arrive après coup.
        const agent = await connecter(serveur.port, { role: 'agent', session: 'w-tardive' });
        const offre = await prochainMessage(agent, 'offer');
        expect(offre).toMatchObject({ type: 'offer', sdp: 'v=0 offre-du-client' });

        await serveur.close();
    });

    it("ne délivre que la dernière offre, pas toutes celles reçues", async () => {
        const serveur = createSignalingServer(0);
        const client = await connecter(serveur.port, { role: 'client', session: 'w-rejeu' });
        client.socket.send(JSON.stringify({ type: 'offer', sdp: 'v=0 premiere' }));
        client.socket.send(JSON.stringify({ type: 'offer', sdp: 'v=0 seconde' }));

        const agent = await connecter(serveur.port, { role: 'agent', session: 'w-rejeu' });
        const offre = await prochainMessage(agent, 'offer');
        expect(offre).toMatchObject({ sdp: 'v=0 seconde' });

        await serveur.close();
    });

    it("oublie l'offre mémorisée quand la session se vide", async () => {
        // Sans cet oubli, un agent qui se reconnecterait sur un identifiant
        // réutilisé recevrait l'offre d'une session morte.
        const serveur = createSignalingServer(0);
        const client = await connecter(serveur.port, { role: 'client', session: 'w-videe' });
        client.socket.send(JSON.stringify({ type: 'offer', sdp: 'v=0 perimee' }));
        await fermerEtAttendre(client);

        const agent = await connecter(serveur.port, { role: 'agent', session: 'w-videe' });
        await expect(prochainMessage(agent, 'offer', 300)).rejects.toThrow();

        await serveur.close();
    });
```

Les auxiliaires `connecter`, `prochainMessage` et `fermerEtAttendre` doivent suivre les conventions déjà présentes dans ce fichier ; s'ils n'existent pas sous ces noms, les écrire en tête du fichier de test :

```typescript
interface Pair { socket: WebSocket; recus: unknown[]; }

async function connecter(port: number, declaration: object): Promise<Pair> {
    const socket = new WebSocket(`ws://127.0.0.1:${port}`);
    const pair: Pair = { socket, recus: [] };
    socket.on('message', (raw) => pair.recus.push(JSON.parse(raw.toString())));
    await new Promise((resolve) => socket.once('open', resolve));
    socket.send(JSON.stringify(declaration));
    return pair;
}

function prochainMessage(pair: Pair, type: string, delai = 1000): Promise<any> {
    return new Promise((resolve, reject) => {
        const echeance = Date.now() + delai;
        const verifier = () => {
            const trouve = pair.recus.find((m: any) => m?.type === type);
            if (trouve) return resolve(trouve);
            if (Date.now() > echeance) return reject(new Error(`aucun message ${type}`));
            setTimeout(verifier, 10);
        };
        verifier();
    });
}

function fermerEtAttendre(pair: Pair): Promise<void> {
    return new Promise((resolve) => {
        pair.socket.once('close', () => setTimeout(resolve, 50));
        pair.socket.close();
    });
}
```

- [ ] **Step 2: Lancer les tests et vérifier qu'ils échouent**

Run: `cd signaling && npm test`
Expected: FAIL sur les quatre nouveaux tests — les types sont rejetés (`type inconnu`), et l'offre antérieure n'est jamais délivrée.

- [ ] **Step 3: Écrire l'implémentation**

Dans `signaling/src/server.ts`, étendre l'interface `Session` :

```typescript
interface Session {
    agent?: WebSocket;
    client?: WebSocket;
    /// Dernière offre reçue du client, retenue tant qu'aucun agent n'est là
    /// pour la prendre.
    ///
    /// Le sous-bloc D1 renverse l'ordre d'arrivée : la page navigateur s'ouvre
    /// et envoie son offre AVANT que le superviseur n'ait lancé l'agent de
    /// cette fenêtre — c'est le viewport de cette page qui décide de la taille
    /// de la sortie virtuelle, donc rien ne peut être lancé plus tôt. Sans
    /// cette mémorisation, l'offre serait perdue en silence et la session ne
    /// s'établirait jamais.
    offreEnAttente?: string;
}
```

Ajouter la liste des types relayés, près du haut du fichier :

```typescript
// Types que le serveur relaie au pair. Tout le reste est refusé — un relais
// qui accepterait n'importe quoi deviendrait un canal de diffusion arbitraire
// sur un serveur sans authentification.
//
// `fenetre-ouverte`, `fenetre-fermee`, `refus` et `viewport` portent la
// session de contrôle du sous-bloc D1, entre le superviseur (rôle `agent`) et
// la page-shell (rôle `client`).
const TYPES_RELAYES = new Set([
    'offer',
    'answer',
    'fenetre-ouverte',
    'fenetre-fermee',
    'refus',
    'viewport',
]);
```

Dans le premier message, juste après `send(socket, { type: 'ice-config', ...ice })` / le `else`, délivrer l'offre en attente :

```typescript
                // Une offre arrivée avant cet agent l'attend : la lui remettre
                // maintenant, sinon elle ne partira jamais.
                if (declaredRole === 'agent' && session.offreEnAttente) {
                    send(socket, { type: 'offer', sdp: session.offreEnAttente });
                    session.offreEnAttente = undefined;
                }
                return;
```

Remplacer le bloc de relais :

```typescript
            if (TYPES_RELAYES.has(message.type as string)) {
                if (message.type === 'offer' && !peer) {
                    // Pas d'agent en face : on retient, plutôt que de perdre.
                    // La dernière écrase les précédentes — une offre périmée
                    // ne sert à rien, et en garder plusieurs n'aurait pas de
                    // destinataire distinct.
                    session.offreEnAttente = message.sdp as string;
                    return;
                }
                send(peer, message);
            } else {
                send(socket, { type: 'error', reason: `type inconnu : ${message.type}` });
            }
```

Noter que le relais transmet désormais `message` entier plutôt que `{type, sdp}` : les messages de contrôle portent d'autres champs (`session`, `titre`, `largeur`, `hauteur`).

Enfin, dans le `close`, la mémorisation doit disparaître avec la session — le `sessions.delete(sessionId)` existant s'en charge déjà, puisque `offreEnAttente` vit dans l'objet supprimé. **Vérifier que c'est bien le cas** et ne rien ajouter si oui.

- [ ] **Step 4: Lancer les tests et vérifier qu'ils passent**

Run: `cd signaling && npm test`
Expected: PASS, tous les tests dont les quatre nouveaux.

- [ ] **Step 5: Commit**

```bash
git add signaling/src/server.ts signaling/src/server.test.ts
git commit -m "feat(d1): session de controle et memorisation de l'offre dans le signaling"
```

---

### Task 10: La page-shell

**Files:**
- Create: `client/shell.html`
- Create: `client/src/shell.ts`
- Create: `client/src/shell.test.ts`
- Modify: `client/vite.config.ts`

**Interfaces:**
- Consumes: le protocole de la session « bureau » (tâche 9)
- Produces:
  - `client/src/shell.ts` exporte `creerBureau(options: OptionsBureau): Bureau`
  - `interface Bureau { fenetreOuverte(session: string, titre: string): void; fenetreFermee(session: string): void; refus(titre: string, motif: string): void; viewportRecu(session: string, largeur: number, hauteur: number): void; liste(): FenetreConnue[]; rouvrir(session: string): void; }`
  - `interface FenetreConnue { session: string; titre: string; ouverte: boolean; }`

La logique est séparée du DOM et du WebSocket : `creerBureau` reçoit ses effets par injection, exactement comme la table du superviseur côté Rust.

- [ ] **Step 1: Écrire les tests qui échouent**

Créer `client/src/shell.test.ts` :

```typescript
import { describe, it, expect, vi } from 'vitest';
import { creerBureau } from './shell';

function bureauDeTest() {
    const ouvertes = new Map<string, { closed: boolean; close: () => void }>();
    const envoyes: unknown[] = [];
    const bureau = creerBureau({
        ouvrirFenetre: (session) => {
            const f = { closed: false, close: () => { f.closed = true; } };
            ouvertes.set(session, f);
            return f as unknown as Window;
        },
        envoyer: (message) => { envoyes.push(message); },
        afficher: () => {},
    });
    return { bureau, ouvertes, envoyes };
}

describe('page-shell', () => {
    it('ouvre une fenêtre navigateur quand le superviseur annonce une fenêtre', () => {
        const { bureau, ouvertes } = bureauDeTest();
        bureau.fenetreOuverte('w-1', 'Bloc-notes');
        expect(ouvertes.has('w-1')).toBe(true);
        expect(bureau.liste()).toEqual([{ session: 'w-1', titre: 'Bloc-notes', ouverte: true }]);
    });

    it('transmet au superviseur le viewport que la page annonce', () => {
        const { bureau, envoyes } = bureauDeTest();
        bureau.fenetreOuverte('w-1', 'Bloc-notes');
        bureau.viewportRecu('w-1', 1600, 900);
        expect(envoyes).toContainEqual({
            type: 'viewport', session: 'w-1', largeur: 1600, hauteur: 900,
        });
    });

    it("ignore un viewport pour une session qu'elle n'a pas ouverte", () => {
        // Le message vient de `postMessage` : n'importe quelle page de même
        // origine peut en émettre un. Ne relayer que ce qu'on a demandé.
        const { bureau, envoyes } = bureauDeTest();
        bureau.viewportRecu('w-inventee', 800, 600);
        expect(envoyes).toHaveLength(0);
    });

    it('ferme la fenêtre navigateur quand la fenêtre Windows disparaît', () => {
        const { bureau, ouvertes } = bureauDeTest();
        bureau.fenetreOuverte('w-1', 'Bloc-notes');
        bureau.fenetreFermee('w-1');
        expect(ouvertes.get('w-1')!.closed).toBe(true);
        expect(bureau.liste()).toEqual([]);
    });

    it('garde la fenêtre dans sa liste quand seule la page a été fermée', () => {
        // Décision de D1 : fermer une page ne ferme pas l'application
        // Windows. La shell doit donc pouvoir la rouvrir.
        const { bureau, ouvertes } = bureauDeTest();
        bureau.fenetreOuverte('w-1', 'Bloc-notes');
        ouvertes.get('w-1')!.closed = true;
        expect(bureau.liste()).toEqual([{ session: 'w-1', titre: 'Bloc-notes', ouverte: false }]);
    });

    it('rouvre une fenêtre dont la page a été fermée', () => {
        const { bureau, ouvertes } = bureauDeTest();
        bureau.fenetreOuverte('w-1', 'Bloc-notes');
        ouvertes.get('w-1')!.closed = true;
        bureau.rouvrir('w-1');
        expect(ouvertes.get('w-1')!.closed).toBe(false);
        expect(bureau.liste()).toEqual([{ session: 'w-1', titre: 'Bloc-notes', ouverte: true }]);
    });

    it('affiche un refus sans rien ouvrir', () => {
        const affiche = vi.fn();
        const bureau = creerBureau({
            ouvrirFenetre: () => { throw new Error('rien ne doit être ouvert'); },
            envoyer: () => {},
            afficher: affiche,
        });
        bureau.refus('F9', 'plus aucune sortie virtuelle disponible');
        expect(affiche).toHaveBeenCalledWith(
            expect.stringContaining('plus aucune sortie virtuelle disponible'),
        );
    });

    it("signale un blocage de pop-up plutôt que de l'ignorer", () => {
        // `window.open` rend `null` quand le navigateur bloque : sans ce
        // traitement, l'utilisateur verrait une fenêtre listée « ouverte »
        // qui n'existe pas.
        const affiche = vi.fn();
        const bureau = creerBureau({
            ouvrirFenetre: () => null,
            envoyer: () => {},
            afficher: affiche,
        });
        bureau.fenetreOuverte('w-1', 'Bloc-notes');
        expect(affiche).toHaveBeenCalledWith(expect.stringContaining('pop-up'));
        expect(bureau.liste()).toEqual([{ session: 'w-1', titre: 'Bloc-notes', ouverte: false }]);
    });
});
```

- [ ] **Step 2: Lancer les tests et vérifier qu'ils échouent**

Run: `cd client && npm test -- shell`
Expected: FAIL — `Cannot find module './shell'`.

- [ ] **Step 3: Écrire la logique**

Créer `client/src/shell.ts` :

```typescript
// La page-shell : le bureau. C'est elle qui ouvre une fenêtre navigateur par
// fenêtre Windows, et elle seule — aucune page d'application n'a ce pouvoir.
//
// Pourquoi une page dédiée plutôt que la première page d'application : sans
// elle, fermer cette première page couperait la capacité d'ouvrir toutes les
// suivantes. Ici, aucune fenêtre d'application n'est spéciale.
//
// Toute la logique est ici, séparée du DOM et du WebSocket, pour être
// testable : `creerBureau` reçoit ses effets par injection.

export interface FenetreConnue {
    session: string;
    titre: string;
    ouverte: boolean;
}

export interface OptionsBureau {
    /// Rend `null` si le navigateur a bloqué l'ouverture.
    ouvrirFenetre(session: string, titre: string): Window | null;
    envoyer(message: unknown): void;
    afficher(message: string): void;
}

export interface Bureau {
    fenetreOuverte(session: string, titre: string): void;
    fenetreFermee(session: string): void;
    refus(titre: string, motif: string): void;
    viewportRecu(session: string, largeur: number, hauteur: number): void;
    liste(): FenetreConnue[];
    rouvrir(session: string): void;
}

interface Entree {
    titre: string;
    fenetre: Window | null;
}

export function creerBureau(options: OptionsBureau): Bureau {
    const connues = new Map<string, Entree>();

    function ouvrir(session: string, titre: string): void {
        const fenetre = options.ouvrirFenetre(session, titre);
        if (!fenetre) {
            options.afficher(
                `« ${titre} » n'a pas pu s'ouvrir : le navigateur a bloqué la pop-up. ` +
                `Autorisez les pop-ups pour ce site, puis rouvrez la fenêtre.`,
            );
        }
        connues.set(session, { titre, fenetre });
    }

    return {
        fenetreOuverte(session, titre) {
            ouvrir(session, titre);
        },

        fenetreFermee(session) {
            const entree = connues.get(session);
            if (!entree) return;
            // La fenêtre Windows a disparu : sa page n'a plus rien à montrer.
            entree.fenetre?.close();
            connues.delete(session);
        },

        refus(titre, motif) {
            options.afficher(`« ${titre} » n'a pas pu s'ouvrir : ${motif}.`);
        },

        viewportRecu(session, largeur, hauteur) {
            // Le message vient de `postMessage` : n'importe quelle page de
            // même origine peut en émettre un. On ne relaie que ce qu'on a
            // soi-même ouvert.
            if (!connues.has(session)) return;
            options.envoyer({ type: 'viewport', session, largeur, hauteur });
        },

        liste() {
            return [...connues.entries()].map(([session, e]) => ({
                session,
                titre: e.titre,
                // `closed` est la seule source de vérité : l'utilisateur peut
                // avoir fermé la page sans que personne ne nous prévienne.
                ouverte: e.fenetre !== null && !e.fenetre.closed,
            }));
        },

        rouvrir(session) {
            const entree = connues.get(session);
            if (!entree) return;
            ouvrir(session, entree.titre);
        },
    };
}
```

- [ ] **Step 4: Lancer les tests et vérifier qu'ils passent**

Run: `cd client && npm test -- shell`
Expected: PASS, 8 tests.

- [ ] **Step 5: Écrire la page et son câblage**

Créer `client/shell.html` :

```html
<!doctype html>
<html lang="fr">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Bureau</title>
  </head>
  <body>
    <h1>Bureau</h1>
    <div id="statut"></div>
    <ul id="fenetres"></ul>
    <script type="module" src="/src/shell-page.ts"></script>
  </body>
</html>
```

Créer `client/src/shell-page.ts`, qui câble la logique au WebSocket et au DOM :

```typescript
// Câblage de la page-shell : WebSocket du signaling d'un côté, DOM de
// l'autre. Aucune règle ici — elles sont dans `shell.ts`, qui est testé.

import { creerBureau } from './shell';

const params = new URLSearchParams(window.location.search);
const signalingUrl = params.get('signaling') ?? `ws://${window.location.hostname}:8080`;
// Identifiant réservé de la session de contrôle : le superviseur s'y déclare
// en `agent`, cette page en `client`.
const SESSION_DE_CONTROLE = 'bureau';

const statut = document.querySelector<HTMLDivElement>('#statut')!;
const liste = document.querySelector<HTMLUListElement>('#fenetres')!;

const socket = new WebSocket(signalingUrl);

const bureau = creerBureau({
    ouvrirFenetre(session) {
        return window.open(`/?session=${encodeURIComponent(session)}`, `guac-${session}`);
    },
    envoyer(message) {
        if (socket.readyState === WebSocket.OPEN) socket.send(JSON.stringify(message));
    },
    afficher(message) {
        statut.textContent = message;
    },
});

function redessiner(): void {
    liste.replaceChildren();
    for (const f of bureau.liste()) {
        const item = document.createElement('li');
        item.textContent = `${f.titre} — ${f.ouverte ? 'ouverte' : 'fermée'} `;
        if (!f.ouverte) {
            const bouton = document.createElement('button');
            bouton.textContent = 'Rouvrir';
            bouton.addEventListener('click', () => { bureau.rouvrir(f.session); redessiner(); });
            item.append(bouton);
        }
        liste.append(item);
    }
}

socket.addEventListener('open', () => {
    socket.send(JSON.stringify({ role: 'client', session: SESSION_DE_CONTROLE }));
    statut.textContent = 'bureau connecté';
});

socket.addEventListener('message', (evenement) => {
    const message = JSON.parse(evenement.data);
    if (message.type === 'fenetre-ouverte') bureau.fenetreOuverte(message.session, message.titre);
    else if (message.type === 'fenetre-fermee') bureau.fenetreFermee(message.session);
    else if (message.type === 'refus') bureau.refus(message.titre, message.motif);
    redessiner();
});

// Les pages d'application annoncent leur viewport par `postMessage` sur leur
// ouvreuse — c'est-à-dire ici.
window.addEventListener('message', (evenement) => {
    // Même origine seulement : cette page ouvre des fenêtres, elle ne doit
    // pas relayer ce que n'importe quel site lui enverrait.
    if (evenement.origin !== window.location.origin) return;
    const message = evenement.data;
    if (message?.type === 'viewport') {
        bureau.viewportRecu(message.session, message.largeur, message.hauteur);
    }
});

// La fermeture d'une page par l'utilisateur ne prévient personne : on relit
// l'état périodiquement plutôt que d'attendre un événement qui n'existe pas.
setInterval(redessiner, 1000);
```

Dans `client/vite.config.ts`, déclarer la seconde page :

```typescript
    build: {
        rollupOptions: {
            input: {
                main: 'index.html',
                shell: 'shell.html',
            },
        },
    },
```

- [ ] **Step 6: Vérifier types et compilation**

```bash
cd client && npm run typecheck && npm test && npm run build
```

Expected: aucune erreur de type, tous les tests passent, la construction produit deux entrées.

- [ ] **Step 7: Commit**

```bash
git add client/shell.html client/src/shell.ts client/src/shell-page.ts \
        client/src/shell.test.ts client/vite.config.ts
git commit -m "feat(d1): la page-shell, qui ouvre une fenetre par fenetre Windows"
```

---

### Task 11: La page d'application annonce son viewport

**Files:**
- Modify: `client/src/main.ts`

**Interfaces:**
- Consumes: `window.opener`
- Produces: un `postMessage` `{ type: 'viewport', session, largeur, hauteur }` vers la page-shell

- [ ] **Step 1: Écrire le code**

Dans `client/src/main.ts`, juste après la lecture de `sessionId` :

```typescript
// Annonce du viewport à la page-shell qui nous a ouverts.
//
// C'est cette taille qui décide de la résolution de la sortie virtuelle, donc
// de la résolution native du flux : rien ne peut être créé côté agent avant
// qu'elle soit connue. L'annonce part donc AVANT toute connexion WebRTC.
//
// `window.opener` est nul quand la page est ouverte à la main (essais,
// rechargement direct) : dans ce cas l'agent tourne déjà et il n'y a rien à
// demander — on ne fait rien plutôt que d'échouer.
if (window.opener && !window.opener.closed) {
    window.opener.postMessage(
        {
            type: 'viewport',
            session: sessionId,
            largeur: Math.round(window.innerWidth),
            hauteur: Math.round(window.innerHeight),
        },
        window.location.origin,
    );
}
```

- [ ] **Step 2: Vérifier types et tests**

```bash
cd client && npm run typecheck && npm test
```

Expected: aucune erreur, aucun test en régression.

- [ ] **Step 3: Commit**

```bash
git add client/src/main.ts
git commit -m "feat(d1): la page d'application annonce son viewport a la shell"
```

---

### Task 12: Câbler le superviseur, et l'enfant qui prend sa fenêtre par configuration

**Files:**
- Modify: `agent/src/superviseur.rs`
- Modify: `agent/src/main.rs`
- Modify: `agent/src/demarrage.rs`
- Create: `agent/src/superviseur/protocole.rs`

**Interfaces:**
- Consumes: tout ce qui précède
- Produces: le mode `--superviseur` (variable `SUPERVISEUR=1`) et les variables d'enfant `FENETRE_HWND`, `SORTIE_DXGI`, `AUDIO`

- [ ] **Step 1: Étendre la configuration**

Dans `agent/src/main.rs`, ajouter à `Config` :

```rust
    /// Vrai en mode superviseur : ce processus ne capture rien, il détecte les
    /// fenêtres et lance un enfant par fenêtre.
    superviseur: bool,
    /// `HWND` de la fenêtre à capturer, en décimal ou hexadécimal préfixé
    /// `0x`. Posé par le superviseur sur ses enfants ; absent, l'agent
    /// retombe sur la recherche par titre (`WINDOW_TITLE`), c'est-à-dire sur
    /// le comportement mono-fenêtre d'avant ce sous-bloc.
    fenetre_hwnd: Option<u64>,
    /// Sortie DXGI à capturer, sous la forme `adaptateur:sortie`. Absente,
    /// l'agent capture le bureau et recadre la fenêtre.
    sortie_dxgi: Option<(u32, u32)>,
    /// Faux sur les enfants qui ne portent pas le son.
    audio: bool,
```

et dans `config()` :

```rust
        superviseur: std::env::var("SUPERVISEUR").is_ok(),
        fenetre_hwnd: std::env::var("FENETRE_HWND").ok().and_then(|v| {
            let v = v.trim();
            match v.strip_prefix("0x") {
                Some(hexa) => u64::from_str_radix(hexa, 16).ok(),
                None => v.parse().ok(),
            }
        }),
        sortie_dxgi: std::env::var("SORTIE_DXGI").ok().and_then(|v| {
            let (a, s) = v.split_once(':')?;
            Some((a.trim().parse().ok()?, s.trim().parse().ok()?))
        }),
        // Le son est actif par défaut : c'est le comportement mono-fenêtre
        // d'avant ce sous-bloc, qu'un agent lancé à la main doit retrouver.
        // Seul le superviseur le coupe, sur les enfants non porteurs.
        audio: std::env::var("AUDIO").as_deref() != Ok("0"),
```

et dans `main()`, avant `demarrage::executer` :

```rust
    if config.superviseur {
        return superviseur::executer(config).await;
    }
```

- [ ] **Step 2: Faire prendre à l'enfant sa fenêtre et sa sortie**

Dans `agent/src/demarrage.rs`, remplacer la construction de la source Windows :

```rust
            #[cfg(windows)]
            {
                // Fenêtre imposée par le superviseur, ou recherche par titre
                // pour un agent lancé à la main.
                let hwnd = match config.fenetre_hwnd {
                    Some(brut) => {
                        let hwnd = windows::Win32::Foundation::HWND(
                            brut as *mut core::ffi::c_void,
                        );
                        anyhow::ensure!(
                            window::is_window_alive(hwnd),
                            "la fenêtre {brut:#x} imposée par le superviseur n'existe plus"
                        );
                        hwnd
                    }
                    None => {
                        let title =
                            std::env::var("WINDOW_TITLE").unwrap_or_else(|_| "firefox".into());
                        window::find_window_by_title(&title)?
                    }
                };
                window_hwnd_addr = Some(hwnd.0 as isize);
                bitrate = std::env::var("BITRATE")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(12_000_000);
                let fps: u32 = std::env::var("ENCODER_FPS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(90);

                match config.sortie_dxgi {
                    Some((adaptateur, sortie)) => {
                        tracing::info!(
                            adaptateur, sortie, bitrate, fps,
                            "capture d'une sortie DXGI entière (mode multi-fenêtres)"
                        );
                        Box::new(windows_source::WindowsSource::sur_sortie(
                            hwnd, adaptateur, sortie, fps, bitrate, clock_origin,
                        )?)
                    }
                    None => {
                        tracing::info!(bitrate, fps, "capture de la fenêtre Windows (recadrage)");
                        Box::new(windows_source::WindowsSource::new(
                            hwnd, fps, bitrate, clock_origin,
                        )?)
                    }
                }
            }
```

et conditionner l'ouverture de l'audio :

```rust
    #[cfg(windows)]
    if config.test_file.is_none() && config.audio {
```

en ajoutant, dans la branche `else` correspondante :

```rust
    #[cfg(windows)]
    if !config.audio {
        tracing::info!("son désactivé sur cet enfant : une seule fenêtre le porte");
    }
```

- [ ] **Step 3: Écrire le protocole de la session « bureau »**

Créer `agent/src/superviseur/protocole.rs` :

```rust
//! Messages de la session de contrôle, entre le superviseur et la page-shell.
//!
//! Le signaling ne fait que relayer : c'est ici que la forme des messages est
//! décidée, et elle doit correspondre exactement à ce que `client/src/shell.ts`
//! attend.

use serde::{Deserialize, Serialize};

/// Identifiant réservé de la session de contrôle. Le superviseur s'y déclare
/// en `agent`, la page-shell en `client`.
pub const SESSION_DE_CONTROLE: &str = "bureau";

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum VersLaShell {
    #[serde(rename = "fenetre-ouverte")]
    FenetreOuverte { session: String, titre: String },
    #[serde(rename = "fenetre-fermee")]
    FenetreFermee { session: String },
    #[serde(rename = "refus")]
    Refus { titre: String, motif: String },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum DepuisLaShell {
    #[serde(rename = "viewport")]
    Viewport { session: String, largeur: u32, hauteur: u32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_ouverture_se_serialise_comme_la_shell_l_attend() {
        let json = serde_json::to_string(&VersLaShell::FenetreOuverte {
            session: "w-1".into(),
            titre: "Bloc-notes".into(),
        })
        .unwrap();
        assert_eq!(
            json,
            r#"{"type":"fenetre-ouverte","session":"w-1","titre":"Bloc-notes"}"#
        );
    }

    #[test]
    fn un_viewport_de_la_shell_se_lit() {
        let message: DepuisLaShell = serde_json::from_str(
            r#"{"type":"viewport","session":"w-1","largeur":1600,"hauteur":900}"#,
        )
        .unwrap();
        let DepuisLaShell::Viewport { session, largeur, hauteur } = message;
        assert_eq!((session.as_str(), largeur, hauteur), ("w-1", 1600, 900));
    }

    #[test]
    fn un_message_inconnu_de_la_shell_est_refuse_plutot_qu_ignore() {
        let resultat: Result<DepuisLaShell, _> =
            serde_json::from_str(r#"{"type":"autre-chose"}"#);
        assert!(resultat.is_err());
    }
}
```

- [ ] **Step 4: Écrire le lanceur de processus**

**Trois fichiers, pas un.** La boucle, le lanceur et l'assemblage font ensemble bien plus de 500 lignes, et la contrainte globale interdit qu'un nouveau fichier naisse au-dessus de ce plafond. Les frontières sont donc posées ici, à l'écriture — pas après coup.

Créer `agent/src/superviseur/lanceur.rs` :

```rust
//! Lancement d'un processus agent par fenêtre, et contrôle de sa vie.
//!
//! Séparé de `boucle.rs` : c'est la seule partie du superviseur qui parle de
//! processus Windows, et elle satisfait un trait dont `enfants.rs` porte les
//! tests avec un lanceur factice.

#![cfg(windows)]

use anyhow::{Context, Result};

use super::enfants::{Consigne, Lanceur};

pub struct LanceurDeProcessus {
    pub executable: std::path::PathBuf,
    pub signaling_url: String,
    pub local_ip: String,
}

impl Lanceur for LanceurDeProcessus {
    fn lancer(&self, consigne: &Consigne) -> Result<u32> {
        let enfant = std::process::Command::new(&self.executable)
            .env("SESSION_ID", &consigne.session.0)
            .env("SIGNALING_URL", &self.signaling_url)
            .env("LOCAL_IP", &self.local_ip)
            .env("FENETRE_HWND", format!("{:#x}", consigne.fenetre))
            .env(
                "SORTIE_DXGI",
                format!("{}:{}", consigne.index_adaptateur, consigne.index_sortie),
            )
            .env("AUDIO", if consigne.audio { "1" } else { "0" })
            // Surtout PAS `SUPERVISEUR` : un enfant qui hériterait de la
            // variable se prendrait pour un superviseur et lancerait ses
            // propres enfants, indéfiniment.
            .env_remove("SUPERVISEUR")
            .spawn()
            .with_context(|| format!("lancement de l'enfant {}", consigne.session.0))?;
        Ok(enfant.id())
    }

    fn est_vivant(&self, pid: u32) -> bool {
        // `OpenProcess` sur un PID mort échoue : c'est le contrôle le moins
        // cher qui ne dépende pas d'avoir gardé le `Child`.
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{
            GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };
        unsafe {
            let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
                return false;
            };
            let mut code = 0u32;
            let vivant = GetExitCodeProcess(handle, &mut code).is_ok()
                // 259 = STILL_ACTIVE.
                && code == 259;
            let _ = CloseHandle(handle);
            vivant
        }
    }

    fn tuer(&self, pid: u32) -> Result<()> {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
        unsafe {
            let handle = OpenProcess(PROCESS_TERMINATE, false, pid)
                .with_context(|| format!("ouverture du processus {pid} pour le terminer"))?;
            let issue = TerminateProcess(handle, 1);
            let _ = CloseHandle(handle);
            issue.with_context(|| format!("terminaison du processus {pid}"))?;
        }
        Ok(())
    }
}
```

- [ ] **Step 4bis: Écrire l'assemblage**

Dans `agent/src/superviseur.rs`, sous les déclarations de modules — et **rien de plus** : ce fichier assemble, il ne décide pas.

```rust
#[cfg(windows)]
pub mod boucle;
#[cfg(windows)]
pub mod lanceur;

#[cfg(windows)]
pub async fn executer(config: crate::Config) -> anyhow::Result<()> {
    use anyhow::Context;

    // Purger AVANT tout : une exécution précédente tuée net a pu laisser des
    // sorties, et elles occupent le vivier de dix.
    if let Err(erreur) = crate::moniteurs_virtuels::purge::purger() {
        tracing::warn!(%erreur, "purge des sorties orphelines incomplète au démarrage");
    }

    let pilote = crate::moniteurs_virtuels::pilote::ouvrir_pilote()
        .context("ouverture du pilote d'affichage virtuel")?;
    let lanceur = lanceur::LanceurDeProcessus {
        executable: std::env::current_exe().context("chemin de l'exécutable")?,
        signaling_url: config.signaling_url.clone(),
        local_ip: config.local_ip.to_string(),
    };
    let (rx_shell, envoyer) = signalisation::connecter(
        &config.signaling_url,
        protocole::SESSION_DE_CONTROLE,
    )
    .await?;

    let (tx_hook, rx_hook) = std::sync::mpsc::channel();
    let _garde_hook = hook::poser(tx_hook).context("pose du hook de détection")?;

    boucle::tourner(&pilote, &lanceur, rx_hook, rx_shell, envoyer)
}

#[cfg(not(windows))]
pub async fn executer(_config: crate::Config) -> anyhow::Result<()> {
    anyhow::bail!("le mode superviseur n'existe que sur Windows")
}
```

- [ ] **Step 4ter: Écrire la boucle**

Créer `agent/src/superviseur/boucle.rs` :

```rust
//! La boucle du superviseur : elle consomme les événements, fait avancer la
//! table, et exécute les effets que celle-ci rend.
//!
//! Aucune décision ici — la table décide, cette boucle agit. C'est ce qui
//! rend les règles éprouvables sans Windows, et ce fichier lisible.

#![cfg(windows)]

use anyhow::{Context, Result};

use super::enfants::{Consigne, Enfants, Lanceur};
use super::protocole::{DepuisLaShell, VersLaShell};
use super::table::{Effet, IdSession, Table};
use super::placement;
use crate::capture::enumerer_sorties;
use crate::moniteurs_virtuels::{pilote::PiloteParIoctl, Sorties};

/// Capacité retenue : le pilote refuse la 11ᵉ sortie (mesuré), et Apollo puise
/// au même vivier sans qu'on sache combien il en prend. Huit est la cible du
/// chantier, avec deux de marge assumée.
const CAPACITE: usize = 8;

    /// Cadence du battement du chien de garde du pilote. Le pilote retire les
    /// sorties d'un client qui cesse de pinguer ; l'unité de son délai n'est
    /// PAS connue (aucune n'est exclue, pas même la seconde), d'où un
    /// battement franchement plus rapide que toute unité plausible.
    const PERIODE_PING: std::time::Duration = std::time::Duration::from_millis(500);

    /// Cadence du contrôle « chaque fenêtre est-elle encore sur sa sortie ».
    /// Une seconde de retard sur un déplacement est imperceptible ; en
    /// revanche ce contrôle énumère les sorties DXGI, ce qui n'est pas
    /// gratuit — il ne doit pas courir à chaque tour de boucle.
    const PERIODE_PLACEMENT: std::time::Duration = std::time::Duration::from_secs(1);

pub fn tourner(
    pilote: &PiloteParIoctl,
    lanceur: &dyn Lanceur,
    rx_hook: std::sync::mpsc::Receiver<super::hook::EvenementFenetre>,
    rx_shell: std::sync::mpsc::Receiver<DepuisLaShell>,
    envoyer: impl Fn(&VersLaShell),
) -> Result<()> {
    let mut sorties = Sorties::nouvelles(pilote);
    let mut enfants = Enfants::nouveaux(lanceur);
    let mut table = Table::nouvelle(CAPACITE);
    // Sorties DXGI deja attribuees, pour que deux fenetres au meme viewport
    // ne se voient pas donner la meme. La table porte deja la correspondance
    // session -> sortie ; ceci n'est que l'ensemble des sorties occupees.
    let mut prises: Vec<(u32, u32)> = Vec::new();

    // Les fenetres deja ouvertes : le hook ne rapporte que les changements.
    let mut effets = Vec::new();
    for (fenetre, titre) in super::hook::enumerer_existantes() {
        effets.extend(table.fenetre_apparue(fenetre, titre));
    }

    let mut dernier_ping = std::time::Instant::now();
    let mut dernier_controle_placement = std::time::Instant::now();
    loop {
        // 1. Exécuter les effets en attente.
        let a_faire = std::mem::take(&mut effets);
        for effet in a_faire {
            match effet {
                Effet::AnnoncerOuverture { session, titre } => {
                    envoyer(&VersLaShell::FenetreOuverte {
                        session: session.0.clone(),
                        titre,
                    });
                }
                Effet::CreerSortie { session, largeur, hauteur } => {
                    match sorties.creer(largeur, hauteur, 60) {
                        Ok(id_pilote) => {
                            // Laisser Windows rattacher la sortie avant
                            // de l'énumérer : elle n'apparaît pas
                            // instantanément dans la topologie DXGI.
                            std::thread::sleep(std::time::Duration::from_millis(1500));
                            let toutes = enumerer_sorties()?;
                            match placement::sortie_par_dimensions(
                                &toutes, largeur, hauteur, &prises,
                            ) {
                                Some(cible) => {
                                    prises.push((cible.index_adaptateur, cible.index_sortie));
                                    // Les DEUX identifiants : celui du
                                    // pilote pour la destruction, la
                                    // position DXGI pour la capture.
                                    effets.extend(table.sortie_creee(
                                        &session,
                                        id_pilote,
                                        (cible.index_adaptateur, cible.index_sortie),
                                    ));
                                    // Poser la fenêtre dessus avant que
                                    // l'enfant ne capture.
                                    if let Some(Effet::LancerEnfant { fenetre, .. }) =
                                        effets.last().cloned()
                                    {
                                        let hwnd = windows::Win32::Foundation::HWND(
                                            fenetre.0 as *mut core::ffi::c_void,
                                        );
                                        if let Err(erreur) =
                                            placement::poser(hwnd, &cible.rect)
                                        {
                                            tracing::warn!(%erreur, "placement de la fenêtre échoué");
                                        }
                                    }
                                }
                                None => {
                                    tracing::error!(
                                        session = %session.0, largeur, hauteur,
                                        "sortie créée mais introuvable dans la topologie DXGI"
                                    );
                                    envoyer(&VersLaShell::FenetreFermee {
                                        session: session.0.clone(),
                                    });
                                }
                            }
                        }
                        Err(erreur) => {
                            tracing::error!(session = %session.0, %erreur, "création de sortie refusée");
                            envoyer(&VersLaShell::Refus {
                                titre: session.0.clone(),
                                motif: format!("{erreur}"),
                            });
                        }
                    }
                }
                Effet::LancerEnfant {
                    session,
                    fenetre,
                    index_adaptateur,
                    index_sortie,
                    audio,
                } => {
                    if let Err(erreur) = enfants.lancer(Consigne {
                        session: session.clone(),
                        fenetre: fenetre.0,
                        index_adaptateur,
                        index_sortie,
                        audio,
                    }) {
                        tracing::error!(session = %session.0, %erreur, "lancement de l'enfant échoué");
                        effets.extend(table.enfant_mort(&session));
                    }
                }
                Effet::TuerEnfant { session } => enfants.tuer(&session),
                Effet::DetruireSortie { sortie_pilote, dxgi } => {
                    // Rendue MAINTENANT, pas à l'arrêt du superviseur : le
                    // vivier du pilote se consomme à chaque ouverture de
                    // fenêtre, et une dizaine d'ouvertures-fermetures
                    // suffirait sinon à bloquer toute nouvelle fenêtre.
                    match sorties.detruire(sortie_pilote) {
                        Ok(()) => tracing::info!(sortie_pilote, "sortie virtuelle rendue au pilote"),
                        Err(erreur) => tracing::error!(
                            sortie_pilote, %erreur,
                            "sortie virtuelle NON rendue — la garde la retentera à l'arrêt"
                        ),
                    }
                    // La place DXGI se libère dans les deux cas : si le
                    // pilote a refusé, la sortie ne sera de toute façon plus
                    // attribuée à personne, et `sortie_par_dimensions`
                    // exigera qu'elle soit encore attachée.
                    prises.retain(|p| *p != dxgi);
                }
                Effet::AnnoncerFermeture { session } => {
                    envoyer(&VersLaShell::FenetreFermee { session: session.0 });
                }
                Effet::AnnoncerRefus { titre, motif } => {
                    envoyer(&VersLaShell::Refus { titre, motif });
                }
            }
        }

        // 2. Battre le chien de garde du pilote.
        if dernier_ping.elapsed() >= PERIODE_PING {
            if let Err(erreur) = pilote.pinguer() {
                tracing::warn!(%erreur, "ping du chien de garde du pilote échoué");
            }
            dernier_ping = std::time::Instant::now();
        }

        // 3. Événements de fenêtres.
        while let Ok(evenement) = rx_hook.try_recv() {
            effets.extend(match evenement {
                hook::EvenementFenetre::Apparue { fenetre, titre } => {
                    table.fenetre_apparue(fenetre, titre)
                }
                hook::EvenementFenetre::Disparue { fenetre } => {
                    table.fenetre_disparue(fenetre)
                }
            });
        }

        // 4. Messages de la shell.
        while let Ok(message) = rx_shell.try_recv() {
            let DepuisLaShell::Viewport { session, largeur, hauteur } = message;
            effets.extend(table.viewport_recu(&IdSession(session), largeur, hauteur));
        }

        // 5. Enfants morts d'eux-mêmes.
        for session in enfants.morts() {
            effets.extend(table.enfant_mort(&session));
        }

        // 6. Les fenêtres sont-elles encore sur leur sortie ?
        //
        // Une application peut se déplacer ou se retailler d'elle-même,
        // et une fenêtre qui déborde de sa sortie donne une capture
        // tronquée sans que rien ne le signale. Le contrôle est
        // PÉRIODIQUE et non branché sur `EVENT_OBJECT_LOCATIONCHANGE` :
        // cet événement se déclenche à chaque pixel de déplacement, sur
        // toutes les fenêtres du bureau, et noierait le canal du hook
        // pour un besoin qui tolère très bien une seconde de retard.
        if dernier_controle_placement.elapsed() >= PERIODE_PLACEMENT {
            dernier_controle_placement = std::time::Instant::now();
            let toutes = enumerer_sorties().unwrap_or_default();
            for session in table.sessions_vivantes() {
                let Some((adaptateur, index)) = table.sortie_dxgi_de(&session) else {
                    continue;
                };
                let Some(cible) = toutes
                    .iter()
                    .find(|s| s.index_adaptateur == adaptateur && s.index_sortie == index)
                else {
                    continue;
                };
                let Some(fenetre) = table.fenetre_de(&session) else { continue };
                let hwnd = windows::Win32::Foundation::HWND(
                    fenetre.0 as *mut core::ffi::c_void,
                );
                let Ok(actuel) = placement::rectangle_de(hwnd) else { continue };
                if placement::doit_etre_replacee(&actuel, &cible.rect) {
                    tracing::info!(
                        session = %session.0,
                        de = format!("{}x{}+{}+{}", actuel.width, actuel.height, actuel.x, actuel.y),
                        vers = format!("{}x{}+{}+{}", cible.rect.width, cible.rect.height, cible.rect.x, cible.rect.y),
                        "fenêtre sortie de sa sortie, replacement"
                    );
                    if let Err(erreur) = placement::poser(hwnd, &cible.rect) {
                        tracing::warn!(session = %session.0, %erreur, "replacement échoué");
                    }
                }
            }
        }

        if effets.is_empty() {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
}
```

- [ ] **Step 4bis: Écrire la connexion à la session de contrôle**

`agent/src/signaling.rs` ouvre une connexion pour une session **média** : il ne relaie que des offres et des réponses SDP. La session de contrôle a besoin de messages arbitraires. Créer `agent/src/superviseur/signalisation.rs` :

```rust
//! Connexion du superviseur à la session de contrôle du signaling.
//!
//! Distincte de `crate::signaling`, qui ne connaît que les offres et réponses
//! SDP d'une session média. Ici on envoie et reçoit des messages de contrôle,
//! et il n'y a jamais de négociation WebRTC.
//!
//! Le récepteur rendu est un `std::sync::mpsc::Receiver` et non un canal
//! tokio : la boucle du superviseur est synchrone (elle appelle des API
//! Windows bloquantes) et le sonde par `try_recv`.

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

use super::protocole::{DepuisLaShell, VersLaShell};

/// Ouvre la connexion, se déclare comme `agent` sur la session donnée, et rend
/// de quoi envoyer et recevoir.
pub async fn connecter(
    url: &str,
    session: &str,
) -> Result<(
    std::sync::mpsc::Receiver<DepuisLaShell>,
    impl Fn(&VersLaShell) + Send + Sync + 'static,
)> {
    let (stream, _) = tokio_tungstenite::connect_async(url)
        .await
        .with_context(|| format!("connexion au signaling {url}"))?;
    let (mut sortant, mut entrant) = stream.split();

    sortant
        .send(Message::Text(
            serde_json::json!({ "role": "agent", "session": session }).to_string(),
        ))
        .await
        .context("déclaration du superviseur au signaling")?;

    // Émission : une tâche tokio consomme une file, pour que l'envoi reste
    // appelable depuis la boucle synchrone du superviseur.
    let (tx_sortant, mut rx_sortant) = tokio::sync::mpsc::unbounded_channel::<String>();
    tokio::spawn(async move {
        while let Some(texte) = rx_sortant.recv().await {
            if let Err(erreur) = sortant.send(Message::Text(texte)).await {
                tracing::warn!(%erreur, "émission vers la shell échouée");
                break;
            }
        }
    });

    // Réception : les messages que la shell nous adresse.
    let (tx_entrant, rx_entrant) = std::sync::mpsc::channel();
    tokio::spawn(async move {
        while let Some(recu) = entrant.next().await {
            let Ok(Message::Text(texte)) = recu else { continue };
            match serde_json::from_str::<DepuisLaShell>(&texte) {
                Ok(message) => {
                    if tx_entrant.send(message).is_err() {
                        break; // le superviseur s'arrête
                    }
                }
                // Le signaling envoie aussi `ice-config` et `peer-gone`, qui
                // ne concernent pas la session de contrôle : les ignorer est
                // le comportement voulu, pas un défaut.
                Err(_) => tracing::debug!(texte, "message ignoré sur la session de contrôle"),
            }
        }
        tracing::warn!("connexion de contrôle au signaling perdue");
    });

    let envoyer = move |message: &VersLaShell| {
        match serde_json::to_string(message) {
            Ok(texte) => {
                let _ = tx_sortant.send(texte);
            }
            Err(erreur) => tracing::error!(%erreur, "sérialisation d'un message de contrôle"),
        }
    };

    Ok((rx_entrant, envoyer))
}
```

Ajouter `#[cfg(windows)] pub mod signalisation;` dans `agent/src/superviseur.rs`, et corriger l'appel dans la boucle — il rend deux valeurs, pas trois :

```rust
        let (rx_shell, envoyer) =
            super::signalisation::connecter(&config.signaling_url, SESSION_DE_CONTROLE).await?;
```

- [ ] **Step 5: Compiler**

```bash
cargo test -p agent                                  # les tests purs restent verts
scripts/build-agent.sh 2>&1 | tee /tmp/build-tache12.log
```

- [ ] **Step 6: Commit**

```bash
git add agent/src/superviseur.rs agent/src/superviseur/protocole.rs \
        agent/src/superviseur/signalisation.rs agent/src/main.rs agent/src/demarrage.rs
git commit -m "feat(d1): cabler le superviseur et l'enfant pilote par configuration"
```

---

### Task 13: Démonstration bout en bout et rapport

**Files:**
- Create: `docs/superpowers/plans/2026-08-01-multifenetres-tranche-verticale-resultats.md`
- Create: `docs/superpowers/plans/journaux-multifenetres-d1/demonstration.log`

- [ ] **Step 1: Préparer la VM et les serveurs**

```bash
virsh list --all                      # démarrer si « fermé »
until timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null; do sleep 5; done
until ls /media/vm/dev >/dev/null 2>&1; do sleep 5; done

cd signaling && npm start &           # relancer AVEC l'environnement TURN si voulu
cd client && npm run dev &
```

**Vérifier l'environnement du processus qui écoute réellement**, pas de celui qu'on croit avoir lancé :

```bash
P=$(ss -ltnp | grep ':8080 ' | grep -o 'pid=[0-9]*' | cut -d= -f2 | head -1)
tr '\0' '\n' < /proc/$P/environ | grep -E '^TURN_URL=|^SIGNALING' || echo "(aucune variable TURN)"
```

- [ ] **Step 2: Lancer le superviseur en session interactive**

```bash
scripts/build-agent.sh 2>&1 | tee /tmp/build-demo.log
SUPERVISEUR=1 scripts/run-agent.sh
```

Le superviseur doit tourner en **session interactive** : la session 0 de WinRM n'a ni bureau ni fenêtres.

- [ ] **Step 3: Dérouler la démonstration**

Ouvrir `http://<hôte>:5173/shell.html` dans Chrome, **avec les pop-ups autorisées pour ce site**.

Séquence à dérouler, en notant l'issue de chaque point :

1. La page-shell liste les fenêtres déjà ouvertes et en ouvre une par fenêtre.
2. Ouvrir le Bloc-notes sur la VM → une nouvelle fenêtre navigateur s'ouvre, montrant le Bloc-notes et lui seul.
3. Taper au clavier dans la fenêtre navigateur du Bloc-notes → le texte apparaît dans le Bloc-notes, **et pas dans l'autre application**.
4. Cliquer et déplacer la souris dans chaque fenêtre → le pointeur agit dans la bonne.
5. Jouer un son sur la VM (`(New-Object Media.SoundPlayer 'C:\Windows\Media\Windows Ding.wav').PlaySync()` — **pas** `[Console]::Beep`, qui donne un faux négatif documenté) → le son sort **d'une seule** fenêtre navigateur.
6. Fermer le Bloc-notes sur la VM → sa fenêtre navigateur se ferme.
7. Fermer une page navigateur → **l'application Windows reste ouverte**, la shell la liste comme « fermée », le bouton « Rouvrir » la ramène.
8. Déplacer une fenêtre Windows à la souris hors de sa sortie → le superviseur la replace dans la seconde (`fenêtre sortie de sa sortie, replacement` au journal), et l'image reste juste. Répéter en la **retaillant** : même issue attendue.

- [ ] **Step 4: Contrôler l'absence de fuite, depuis un processus neuf**

Arrêter le superviseur, puis :

```bash
MULTIFENETRE_DXGI=1 scripts/run-agent.sh     # la topologie est-elle revenue à son état de départ ?
MULTIFENETRE_VDD_PURGE=1 scripts/run-agent.sh
```

**Comparer des ensembles de NOMS de sorties, jamais des nombres** : Apollo peut ajouter une sortie à tout instant, et une addition externe compenserait exactement un retrait manquant.

- [ ] **Step 5: Écrire le rapport**

Créer `docs/superpowers/plans/2026-08-01-multifenetres-tranche-verticale-resultats.md`, avec :

- le déroulé point par point de la démonstration, chacun avec son issue **réellement observée** ;
- ce qui a échoué ou surpris, sans arrondi ;
- **le plafond d'encodeurs en multi-processus** — combien de fenêtres ont pu être ouvertes avant refus, et par quel appel ce refus est arrivé. C'est la première occasion de le relever, et la spec le désigne comme inconnu ;
- les limites connues et assumées de D1, reprises de la spec §8 : lien sur-souscrit à plusieurs fenêtres actives (aucun partage de la capacité), son perdu si la porteuse se ferme, pas de redimensionnement d'une fenêtre déjà ouverte, sorties réellement rendues au pilote seulement à l'arrêt du superviseur ;
- **ce que la démonstration n'établit pas** : aucune mesure de cadence ni de latence, une seule exécution, deux applications seulement, aucune durée longue.

Verser le journal brut dans `docs/superpowers/plans/journaux-multifenetres-d1/demonstration.log`, **en UTF-8 sans BOM** — `scripts/run-agent.sh` a été corrigé le 31 juillet 2026 et produit désormais des journaux `grep`-ables tels quels.

- [ ] **Step 6: Mettre à jour les documents qui concluent**

Dans `CLAUDE.md`, ajouter une section pour ce sous-bloc, sur le modèle des précédentes : ce qui est acquis, les pièges rencontrés, ce que la démonstration n'établit pas.

Dans `docs/superpowers/specs/2026-07-28-support-jeux-design.md` §5 D, annoter ce que D1 a réglé et ce qui reste à D2-D5. **Chercher par le sens, pas par la formule** — une négation se dit de plusieurs façons, et c'est celle qu'on n'a pas listée qui survit. Vérifier aussi le sommaire du document, pas seulement le chapitre de détail.

- [ ] **Step 7: Commit**

```bash
git add docs/superpowers/plans/2026-08-01-multifenetres-tranche-verticale-resultats.md \
        docs/superpowers/plans/journaux-multifenetres-d1/ \
        CLAUDE.md docs/superpowers/specs/2026-07-28-support-jeux-design.md
git commit -m "docs(d1): resultats de la tranche verticale multi-fenetres"
```

---

## Ce que ce plan ne couvre pas

Repris de la spec §8, pour qu'aucune tâche ne les traite par inadvertance :

- **Plein écran** (D4) et **mise en sommeil des fenêtres masquées** (D5).
- **Partage de la capacité réseau** (D3) : chaque enfant garde son contrôleur de congestion et estime sa propre part. À plusieurs fenêtres actives, **le lien est sur-souscrit** — limite à écrire dans le rapport, pas défaut à découvrir.
- **Audio par fenêtre** (D3), par loopback de processus.
- **Redimensionnement d'une fenêtre déjà ouverte** : le pilote n'expose aucun changement de mode.
- **Redésignation de la porteuse audio** quand sa fenêtre se ferme (D2).
- **Libération d'une sortie virtuelle en cours d'exécution** : la tâche 12 la marque libre mais ne la rend au pilote qu'à l'arrêt du superviseur (D2).
- **Le repli `PrintWindow`** pour les fenêtres auxquelles la voie principale ne s'applique pas.
