# Mesures préalables au chantier D — plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Lever les quatre inconnues qui empêchent de spécifier le chantier D — plafond de sorties virtuelles, plafond d'encodage sur périphériques D3D11 séparés, correction d'image sur sortie virtuelle, tenue de `PrintWindow` à N=4 et N=8.

**Architecture:** Chantier de mesure, aucun livrable produit. Le code neuf vit dans `agent/src/diagnostics/multifenetre/` (glue Windows, non testée) et `agent/src/moniteurs_virtuels.rs` (logique pure, testée sur Linux). Chaque mesure s'active par sa propre variable d'environnement et s'exécute dans son propre processus.

**Tech Stack:** Rust, `windows` 0.62, DXGI Desktop Duplication, Media Foundation, un pilote d'affichage indirect (IddCx). Mesures exécutées sur la VM Windows via `scripts/build-agent.sh` et `scripts/run-agent.sh`.

**Spec :** `docs/superpowers/specs/2026-07-31-mesures-prealables-chantier-d-design.md`

## Global Constraints

- **Ordre d'exécution imposé par la spec §2** : ④ → ① → ③ → ②. Les tâches de ce plan suivent cet ordre.
- **② n'est conditionnée par rien.** Si ① conclut « voie 2 hors d'atteinte » (spec §6.4), la tâche 8 (mesure ③) devient sans objet et se solde par ce constat au journal — mais **la tâche 9 (mesure ②) reste due**, c'est l'une des deux mesures bloquantes.
- **Aucun fichier source neuf au-dessus de 500 lignes** (`CLAUDE.md`, « Conventions de code »). `agent/src/capture.rs` est à 443 lignes : ne rien y ajouter.
- **Aucune trace par image ni par paquet.** Compteurs agrégés, journalisés à la seconde au plus.
- **Un processus par sonde.** Une variable d'environnement par mesure ; l'aiguillage de `agent/src/diagnostics/multifenetre.rs` retourne `Ok(true)` et `main()` s'arrête.
- **Nommage en français**, comme tout le code du dépôt (`sorties`, `pilote`, `facteur_echelle`).
- **Tests sur Linux** : `cargo test -p agent` (174 tests passent aujourd'hui en ~2 s). Le code sous `#[cfg(windows)]` n'y est pas compilé — c'est pourquoi la logique testable va dans `agent/src/moniteurs_virtuels.rs`, module **non gardé** au niveau du crate, comme `disposition.rs` et `mire.rs`.
- **La VM Windows est éteinte.** Avant toute tâche qui mesure (2, 3, 6, 7, 8, 9) :

```bash
virsh list --all                     # « fermé » = éteinte
virsh start Windows
until timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null; do sleep 5; done
until ls /media/vm/dev >/dev/null 2>&1; do sleep 5; done   # accès RÉEL, pas mountpoint
set -a && source .env && set +a
```

## File Structure

| Fichier | Responsabilité | État |
| --- | --- | --- |
| `scripts/run-agent.sh` | Lancement en session interactive, passage des variables | Modifié (tâches 1, 6, 7, 8, 9) |
| `agent/src/moniteurs_virtuels.rs` | Trait du pilote, garde de destruction, conversions de coordonnées — **logique pure, testée** | Créé (tâche 4) |
| `agent/src/main.rs` | Déclaration du module | Modifié (tâche 4) |
| `agent/src/diagnostics/multifenetre.rs` | Aiguillage des sondes, helper `causes` partagé | Modifié (tâches 6, 7, 8, 9) |
| `agent/src/diagnostics/multifenetre/moniteurs.rs` | Pilote concret + sondes ① et purge — **glue Windows** | Créé (tâches 5, 6, 7) |
| `agent/src/diagnostics/multifenetre/banc.rs` | Passes du banc, désignation de sortie | Modifié (tâche 8) |
| `agent/src/diagnostics/multifenetre/voies.rs` | Voies de capture, duplication sur sortie désignée | Modifié (tâche 8) |
| `agent/src/diagnostics/multifenetre/nvenc.rs` | Plafond d'encodeurs, partagé ou séparé | Modifié (tâche 9) |
| `docs/superpowers/plans/journaux-mesures-prealables/` | Journaux versés, en UTF-8 | Créé (tâche 2) |
| `docs/superpowers/plans/2026-07-31-mesures-prealables-chantier-d-resultats.md` | Document de résultats | Créé (tâche 10) |

---

### Task 1: Journaux en UTF-8

**Pourquoi.** Les journaux de la sonde précédente mêlent UTF-16LE et UTF-8 avec BOM, au point que `CLAUDE.md` doit expliquer comment les lire — un `grep` direct sur `dxgi.log`, `wgc.log`, `replis.log` et `nvenc.log` ne trouve rien. La cause est ici : `Tee-Object -FilePath` sous Windows PowerShell 5.1 écrit en UTF-16LE et n'accepte pas de paramètre `-Encoding`. Tous les journaux de ce bloc en dépendent, donc cette tâche vient en premier.

**Files:**
- Modify: `scripts/run-agent.sh:58` (la ligne `& '${AGENT_EXE}' *>&1 | Tee-Object ...`)

**Interfaces:**
- Consumes: rien.
- Produces: `/media/vm/dev/agent.log` en **UTF-8 sans BOM**, lignes non tronquées. Toutes les tâches de mesure lisent ce fichier.

- [ ] **Step 1: Remplacer l'écriture du journal**

Dans `scripts/run-agent.sh`, remplacer la dernière ligne du heredoc `PS1` :

```powershell
& '${AGENT_EXE}' *>&1 | Tee-Object -FilePath 'C:\dev\agent.log'
```

par :

```powershell
# UTF-8 SANS BOM, et sans le retour à la ligne que `Out-File` insère à la
# largeur de console : `Tee-Object` (PS 5.1) écrit en UTF-16LE et n'a pas de
# paramètre -Encoding, ce qui rendait les journaux de la sonde précédente
# illisibles au `grep`. `Out-File -Encoding utf8` corrigerait l'encodage mais
# reformaterait les lignes longues. Un StreamWriter explicite ne fait ni l'un
# ni l'autre.
\$flux = New-Object System.IO.StreamWriter('C:\dev\agent.log', \$false, (New-Object System.Text.UTF8Encoding(\$false)))
try {
  & '${AGENT_EXE}' *>&1 | ForEach-Object { \$flux.WriteLine([string]\$_); \$flux.Flush() }
} finally {
  \$flux.Close()
}
```

**Attention à l'échappement** : le heredoc `PS1` n'est **pas** entre quotes (il doit interpoler `${AGENT_EXE}`), donc chaque `$` PowerShell s'écrit `\$` — c'est déjà le cas des lignes `\$env:...` juste au-dessus.

- [ ] **Step 2: Corriger le commentaire devenu faux**

`scripts/run-agent.sh:67` affirme « toujours redirigée vers agent.log via Tee-Object ». Remplacer :

```
# n'affecte ni la session (toujours 1, toujours interactive) ni la sortie
# (toujours redirigée vers agent.log via Tee-Object).
```

par :

```
# n'affecte ni la session (toujours 1, toujours interactive) ni la sortie
# (toujours redirigée vers agent.log par le StreamWriter UTF-8 ci-dessus).
```

- [ ] **Step 3: Vérifier sur la VM**

Démarrer la VM (voir « Global Constraints »), puis :

```bash
scripts/build-agent.sh
MULTIFENETRE_DXGI=1 scripts/run-agent.sh
sleep 20
file /media/vm/dev/agent.log
```

Attendu : `UTF-8 Unicode text` (et **pas** `Unicode text, UTF-16, little-endian`, ni `with BOM`).

- [ ] **Step 4: Vérifier que `grep` mord et que les lignes ne sont pas coupées**

```bash
grep -c "sortie DXGI\|sorties DXGI relevées" /media/vm/dev/agent.log
awk '{ print length }' /media/vm/dev/agent.log | sort -rn | head -1
```

Attendu : au moins 1 correspondance ; la ligne la plus longue dépasse 120 caractères (preuve qu'aucun repli à la largeur de console n'a eu lieu).

- [ ] **Step 5: Commit**

```bash
git add scripts/run-agent.sh
git commit -m "fix(mesures): journaux d'agent en UTF-8 sans BOM, lignes entières

Tee-Object sous PowerShell 5.1 écrit en UTF-16LE et n'accepte pas
-Encoding : c'est ce qui rendait dxgi.log, wgc.log, replis.log et nvenc.log
illisibles au grep, au point que CLAUDE.md doive documenter la conversion.

Un StreamWriter explicite écrit en UTF-8 sans BOM et n'applique aucun
reformatage à la largeur de console, contrairement à Out-File.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 2: Mesure ④ — `PrintWindow` à N=4 et N=8

**Pourquoi en premier.** Elle ne coûte aucun code : le banc a déjà la voie `printwindow` (`voies.rs:312`) et le paramètre `MULTIFENETRE_N` (`banc.rs:43`, borné à `mire::MIRES_MAX = 8`). Elle solidifie le repli du chantier D au cas où la mesure ① s'effondre.

**Files:**
- Create: `docs/superpowers/plans/journaux-mesures-prealables/printwindow-n4.log`
- Create: `docs/superpowers/plans/journaux-mesures-prealables/printwindow-n8.log`

**Interfaces:**
- Consumes: le journal UTF-8 de la tâche 1.
- Produces: deux journaux versés, et deux couples (images/s par fenêtre, pixels/s total) repris par la tâche 10.

- [ ] **Step 1: Mesurer à N=4**

VM démarrée, puis :

```bash
scripts/build-agent.sh
MULTIFENETRE_BANC=printwindow MULTIFENETRE_N=4 scripts/run-agent.sh
sleep 60          # trois passes de 10 s, plus l'ouverture des mires
mkdir -p docs/superpowers/plans/journaux-mesures-prealables
cp /media/vm/dev/agent.log docs/superpowers/plans/journaux-mesures-prealables/printwindow-n4.log
```

- [ ] **Step 2: Vérifier que la mesure a bien eu lieu**

```bash
grep -E "disposition retenue|images|verdicts_faux" \
  docs/superpowers/plans/journaux-mesures-prealables/printwindow-n4.log
```

Attendu : une ligne `banc : disposition retenue` avec `nombre=4` et quatre places, puis des lignes de compteurs. Si `verdicts_faux` est non nul, la passe d'encodage a été sautée — c'est un résultat à consigner, pas un échec de la tâche.

- [ ] **Step 3: Mesurer à N=8**

```bash
MULTIFENETRE_BANC=printwindow MULTIFENETRE_N=8 scripts/run-agent.sh
sleep 60
cp /media/vm/dev/agent.log docs/superpowers/plans/journaux-mesures-prealables/printwindow-n8.log
grep -E "disposition retenue|images|verdicts_faux" \
  docs/superpowers/plans/journaux-mesures-prealables/printwindow-n8.log
```

- [ ] **Step 4: Convertir en pixels par seconde**

`disposition::tuiles` découpe le bureau : à aire totale fixe, la surface par fenêtre décroît quand N croît. Une cadence stable ne prouverait donc pas que le nombre de fenêtres est neutre. Pour chaque N, relever les dimensions de place dans la ligne `disposition retenue` et calculer :

```
pixels_par_seconde = images_par_seconde_par_fenetre × largeur_place × hauteur_place × N
```

Noter les deux chiffres (i/s **et** MP/s) pour la tâche 10. Repère de la sonde : la voie `duplication` tenait 208–258 MP/s ; `printwindow` valait 45,0 i/s à N=1 et 29,1 i/s/fenêtre à N=2.

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/plans/journaux-mesures-prealables/
git commit -m "mesure(printwindow): tenue du repli à N=4 et N=8

Quatrième mesure de la spec, la seule à ne coûter aucun code : le banc de la
sonde porte déjà la voie printwindow et le paramètre N.

La sonde n'avait mesuré ce repli qu'à N=1 et N=2.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 3: Reconnaissance du canal de contrôle du pilote

**Pourquoi.** Le dépôt ne sait pas comment on commande un pilote d'affichage indirect. La spec §6.1 pose deux hypothèses à éprouver ; cette tâche les tranche **avant** qu'une ligne de Rust soit écrite. Elle ne produit pas de code — elle produit un fait consigné.

**Files:**
- Create: `docs/superpowers/plans/journaux-mesures-prealables/canal-de-controle.md`

**Interfaces:**
- Consumes: rien.
- Produces: la désignation exacte du canal retenu, sous l'une de ces trois formes, que la tâche 5 implémente telle quelle :
  - **A** — nom du fichier DLL + noms exportés + signature (`extern "system"`, types des paramètres, type de retour) ;
  - **B** — GUID d'interface de périphérique + code IOCTL + disposition du tampon d'entrée ;
  - **C** — aucun canal trouvé → la spec §6.4 impose le repli « changer de pilote », qui devient l'objet de la tâche 5.

- [ ] **Step 1: Localiser les fichiers du pilote**

```bash
find /media/vm/Windows/System32/DriverStore/FileRepository -iname 'sudovda*' 2>/dev/null
find /media/vm/Windows/System32 -maxdepth 2 -iname '*vda*' -o -maxdepth 2 -iname '*virtualdisplay*' 2>/dev/null
find '/media/vm/Program Files' '/media/vm/Program Files (x86)' -maxdepth 3 -iname '*.dll' 2>/dev/null | grep -i -E 'apollo|sunshine|vda'
```

Consigner les chemins trouvés dans `canal-de-controle.md`.

- [ ] **Step 2: Lire l'INF — l'interface de périphérique y est déclarée**

```bash
INF=$(find /media/vm/Windows/System32/DriverStore/FileRepository -iname 'sudovda.inf' | head -1)
grep -i -n -E 'Interface|Guid|ClassGuid|AddReg|ServiceBinary' "$INF"
```

Un pilote qui se pilote par IOCTL déclare une **interface de périphérique** sous forme de GUID. Sa présence oriente vers l'hypothèse B ; son absence, vers A.

- [ ] **Step 3: Dumper les exports des DLL candidates**

`objdump` de binutils lit les exécutables PE :

```bash
for dll in <chemins trouvés à l'étape 1>; do
  echo "=== $dll"
  objdump -x "$dll" 2>/dev/null | sed -n '/Export Address Table/,/^$/p'
done
```

Chercher un nom de la famille `AddVirtualDisplay` / `RemoveVirtualDisplay`. **Un nom exporté ne donne pas sa signature** : si l'hypothèse A tient, il faut encore la trouver — voir l'étape suivante.

- [ ] **Step 4: Compléter par la source amont si les deux étapes précédentes ne suffisent pas**

Le pilote est publié en source ouverte par SudoMaker. Y lire l'en-tête de contrôle (GUID d'interface, codes IOCTL, structures d'entrée) ou la déclaration des exports. C'est la seule voie qui donne des **signatures** plutôt que des noms.

- [ ] **Step 5: Consigner le verdict**

Écrire `canal-de-controle.md` avec, selon le cas :

- **A** : `chemin de la DLL`, `nom exporté`, et la signature Rust correspondante, par exemple
  `type FnCreer = unsafe extern "system" fn(largeur: u32, hauteur: u32, hertz: u32) -> i32;`
- **B** : le GUID d'interface, le code IOCTL, et la structure du tampon d'entrée.
- **C** : ce qui a été cherché, et pourquoi rien n'a été trouvé.

Le document doit permettre à la tâche 5 d'écrire du code **sans rouvrir de recherche**.

- [ ] **Step 6: Commit**

```bash
git add docs/superpowers/plans/journaux-mesures-prealables/canal-de-controle.md
git commit -m "mesure(moniteurs): canal de contrôle du pilote d'affichage virtuel

Reconnaissance préalable exigée par la spec §6.1 : le dépôt ne savait pas
comment on commande ce pilote. Ce document tranche, et la tâche suivante
implémente sans rouvrir de recherche.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 4: Trait du pilote, garde de destruction et conversions — la partie testable

**Pourquoi un module non gardé.** `agent/src/diagnostics/multifenetre/` est sous `#[cfg(windows)]` : rien de ce qui y vit n'est compilé, donc testé, sur Linux. La garde de destruction et les conversions de coordonnées sont de la logique pure ; elles vont au niveau du crate, comme `disposition.rs` et `mire.rs`.

**Files:**
- Create: `agent/src/moniteurs_virtuels.rs`
- Modify: `agent/src/main.rs` (déclaration du module)

**Interfaces:**
- Consumes: `crate::geometry::Rect { x: i32, y: i32, width: u32, height: u32 }`.
- Produces, pour les tâches 5 à 8 :
  - `pub type IdSortie = u32`
  - `pub trait PiloteAffichageVirtuel { fn creer(&self, largeur: u32, hauteur: u32, hertz: u32) -> anyhow::Result<IdSortie>; fn detruire(&self, id: IdSortie) -> anyhow::Result<()>; }`
  - `pub struct Sorties<'p>` avec `Sorties::nouvelles(&'p dyn PiloteAffichageVirtuel) -> Sorties<'p>`, `sorties.creer(largeur: u32, hauteur: u32, hertz: u32) -> anyhow::Result<IdSortie>`, `sorties.nombre() -> usize`
  - `pub fn analyser_designation(texte: &str) -> anyhow::Result<(u32, u32)>`
  - `pub fn facteur_echelle(annonce: (u32, u32), texture: (u32, u32)) -> Option<(f64, f64)>`
  - `pub fn vers_texture(region: Rect, sortie: Rect, facteur: (f64, f64)) -> Rect`

- [ ] **Step 1: Écrire les tests qui échouent**

Créer `agent/src/moniteurs_virtuels.rs` avec **uniquement** le module de tests ci-dessous (le code de production vient à l'étape 3) :

```rust
//! Mesure ① de la spec : combien de sorties virtuelles simultanées un pilote
//! d'affichage indirect accepte-t-il, et une fenêtre posée dessus est-elle
//! capturée correctement.
//!
//! Ce module ne contient QUE de la logique pure : le trait que doit remplir
//! un pilote, la garde qui détruit ce qui a été créé, et les conversions de
//! coordonnées. La glue Windows vit dans
//! `diagnostics/multifenetre/moniteurs.rs`.
//!
//! Il n'est PAS sous `#[cfg(windows)]`, délibérément : une sortie virtuelle
//! survit au processus, donc la garde ci-dessous est le seul rempart contre
//! une VM laissée avec des moniteurs fantômes — c'est exactement le genre de
//! code qui doit avoir des tests, et ils ne tourneraient pas sous
//! `#[cfg(windows)]`.

use anyhow::{Context, Result};

use crate::geometry::Rect;

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// Pilote factice : il compte les sorties vivantes, il n'en crée aucune.
    struct PiloteFactice {
        vivantes: RefCell<Vec<IdSortie>>,
        suivant: RefCell<IdSortie>,
        plafond: usize,
    }

    impl PiloteFactice {
        fn avec_plafond(plafond: usize) -> Self {
            Self { vivantes: RefCell::new(Vec::new()), suivant: RefCell::new(1), plafond }
        }
    }

    impl PiloteAffichageVirtuel for PiloteFactice {
        fn creer(&self, _largeur: u32, _hauteur: u32, _hertz: u32) -> Result<IdSortie> {
            let mut vivantes = self.vivantes.borrow_mut();
            anyhow::ensure!(vivantes.len() < self.plafond, "plafond du pilote factice");
            let mut suivant = self.suivant.borrow_mut();
            let id = *suivant;
            *suivant += 1;
            vivantes.push(id);
            Ok(id)
        }

        fn detruire(&self, id: IdSortie) -> Result<()> {
            self.vivantes.borrow_mut().retain(|vivante| *vivante != id);
            Ok(())
        }
    }

    #[test]
    fn la_garde_detruit_tout_ce_qu_elle_a_cree() {
        let pilote = PiloteFactice::avec_plafond(8);
        {
            let mut sorties = Sorties::nouvelles(&pilote);
            sorties.creer(1920, 1080, 60).unwrap();
            sorties.creer(1920, 1080, 60).unwrap();
            sorties.creer(1920, 1080, 60).unwrap();
            assert_eq!(sorties.nombre(), 3);
            assert_eq!(pilote.vivantes.borrow().len(), 3);
        }
        assert!(
            pilote.vivantes.borrow().is_empty(),
            "la garde a laissé des sorties derrière elle"
        );
    }

    /// Le cas qui justifie la garde : ces API échouent par plantage, et un
    /// moniteur virtuel survit au processus.
    #[test]
    fn la_garde_detruit_meme_quand_le_fil_panique() {
        let pilote = PiloteFactice::avec_plafond(8);
        let issue = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut sorties = Sorties::nouvelles(&pilote);
            sorties.creer(1920, 1080, 60).unwrap();
            sorties.creer(1920, 1080, 60).unwrap();
            panic!("panique simulée au milieu de la montée en N");
        }));
        assert!(issue.is_err(), "la panique aurait dû se propager");
        assert!(
            pilote.vivantes.borrow().is_empty(),
            "des moniteurs fantômes survivent à une panique"
        );
    }

    #[test]
    fn un_refus_du_pilote_ne_perd_pas_les_sorties_deja_creees() {
        let pilote = PiloteFactice::avec_plafond(2);
        {
            let mut sorties = Sorties::nouvelles(&pilote);
            sorties.creer(1920, 1080, 60).unwrap();
            sorties.creer(1920, 1080, 60).unwrap();
            assert!(sorties.creer(1920, 1080, 60).is_err(), "le plafond aurait dû refuser");
            assert_eq!(sorties.nombre(), 2, "un refus ne doit pas compter comme une création");
        }
        assert!(pilote.vivantes.borrow().is_empty());
    }

    #[test]
    fn une_designation_bien_formee_donne_les_deux_index() {
        assert_eq!(analyser_designation("0:1").unwrap(), (0, 1));
        assert_eq!(analyser_designation("2:0").unwrap(), (2, 0));
    }

    #[test]
    fn une_designation_mal_formee_est_refusee() {
        assert!(analyser_designation("0").is_err(), "un seul index");
        assert!(analyser_designation("0:1:2").is_err(), "trois index");
        assert!(analyser_designation("a:b").is_err(), "pas des entiers");
        assert!(analyser_designation("").is_err(), "vide");
    }

    #[test]
    fn des_dimensions_identiques_donnent_un_facteur_unite() {
        assert_eq!(facteur_echelle((2400, 1080), (2400, 1080)), Some((1.0, 1.0)));
    }

    /// Le piège relevé par la sonde : sortie annoncée 3413×960 par DXGI,
    /// 5120×1440 par WMI — rapport 1,5, la mise à l'échelle DPI à 150 %.
    #[test]
    fn le_piege_dpi_de_la_sonde_donne_un_facteur_de_un_et_demi() {
        let (horizontal, vertical) = facteur_echelle((3413, 960), (5120, 1440)).unwrap();
        assert!((horizontal - 1.5).abs() < 0.001, "horizontal = {horizontal}");
        assert!((vertical - 1.5).abs() < 0.001, "vertical = {vertical}");
    }

    #[test]
    fn une_annonce_degeneree_ne_donne_aucun_facteur() {
        assert_eq!(facteur_echelle((0, 960), (5120, 1440)), None);
        assert_eq!(facteur_echelle((3413, 0), (5120, 1440)), None);
    }

    #[test]
    fn sans_echelle_ni_decalage_la_region_ne_bouge_pas() {
        let sortie = Rect { x: 0, y: 0, width: 2400, height: 1080 };
        let region = Rect { x: 100, y: 200, width: 300, height: 400 };
        assert_eq!(vers_texture(region, sortie, (1.0, 1.0)), region);
    }

    #[test]
    fn une_sortie_decalee_ramene_la_region_a_l_origine_de_sa_texture() {
        let sortie = Rect { x: 2400, y: 0, width: 3413, height: 960 };
        let region = Rect { x: 2500, y: 100, width: 200, height: 200 };
        assert_eq!(
            vers_texture(region, sortie, (1.0, 1.0)),
            Rect { x: 100, y: 100, width: 200, height: 200 }
        );
    }

    /// Le cas qui compte : recadrer sur le rectangle annoncé alors que la
    /// texture est aux dimensions physiques décalerait tout d'un facteur 1,5.
    #[test]
    fn le_facteur_dpi_agrandit_la_region_et_son_origine() {
        let sortie = Rect { x: 0, y: 0, width: 3413, height: 960 };
        let region = Rect { x: 100, y: 100, width: 200, height: 200 };
        assert_eq!(
            vers_texture(region, sortie, (1.5, 1.5)),
            Rect { x: 150, y: 150, width: 300, height: 300 }
        );
    }
}
```

- [ ] **Step 2: Déclarer le module et vérifier que la compilation échoue**

Ajouter dans `agent/src/main.rs`, à sa place alphabétique parmi les `mod` :

```rust
mod moniteurs_virtuels;
```

Run: `cargo test -p agent 2>&1 | tail -20`
Expected: FAIL à la compilation — `cannot find type Sorties`, `cannot find function analyser_designation`, etc.

- [ ] **Step 3: Écrire l'implémentation**

Insérer dans `agent/src/moniteurs_virtuels.rs`, **entre** les `use` et `mod tests` :

```rust
/// Identifiant d'une sortie virtuelle, tel que le pilote le rend.
pub type IdSortie = u32;

/// Ce que ce bloc attend d'un pilote d'affichage virtuel, quel qu'il soit.
///
/// L'indirection existe pour deux raisons. La spec §6.4 acte un repli —
/// changer de pilote si celui de la VM résiste — et ce repli ne doit faire
/// réécrire ni la montée en N ni la garde. Et la garde ci-dessous doit
/// pouvoir être éprouvée sans Windows.
pub trait PiloteAffichageVirtuel {
    fn creer(&self, largeur: u32, hauteur: u32, hertz: u32) -> Result<IdSortie>;
    fn detruire(&self, id: IdSortie) -> Result<()>;
}

/// Détruit les sorties créées quoi qu'il arrive, y compris si le fil panique.
///
/// Sans elle, une sonde qui plante à la cinquième création laisse cinq
/// moniteurs derrière elle, et l'état survit au processus.
pub struct Sorties<'p> {
    pilote: &'p dyn PiloteAffichageVirtuel,
    creees: Vec<IdSortie>,
}

impl<'p> Sorties<'p> {
    pub fn nouvelles(pilote: &'p dyn PiloteAffichageVirtuel) -> Self {
        Self { pilote, creees: Vec::new() }
    }

    /// Un refus du pilote ressort tel quel et ne compte pas comme création :
    /// détruire un identifiant que le pilote n'a jamais rendu ferait au mieux
    /// une erreur de plus au journal, au pire détruirait la sortie d'autrui.
    pub fn creer(&mut self, largeur: u32, hauteur: u32, hertz: u32) -> Result<IdSortie> {
        let id = self.pilote.creer(largeur, hauteur, hertz)?;
        self.creees.push(id);
        Ok(id)
    }

    pub fn nombre(&self) -> usize {
        self.creees.len()
    }
}

impl Drop for Sorties<'_> {
    fn drop(&mut self) {
        // En ordre inverse de création : si le pilote a un état d'ordre, le
        // défaire dans l'ordre où il a été construit est le seul choix sûr.
        // `Drop` court aussi pendant le déroulement d'une panique — c'est
        // précisément le cas que la garde existe pour couvrir.
        for id in self.creees.drain(..).rev() {
            if let Err(erreur) = self.pilote.detruire(id) {
                tracing::error!(
                    id,
                    %erreur,
                    "sortie virtuelle NON détruite — purge manuelle requise"
                );
            }
        }
    }
}

/// Analyse une désignation de sortie « index_adaptateur:index_sortie », telle
/// que la portent les variables d'environnement du banc.
///
/// Les deux index sont ceux de `capture::enumerer_sorties`, et ce sont ceux
/// qu'attend `DesktopCapture::sur_sortie`.
pub fn analyser_designation(texte: &str) -> Result<(u32, u32)> {
    let (adaptateur, sortie) = texte
        .split_once(':')
        .with_context(|| format!("désignation « {texte} » : forme attendue « adaptateur:sortie »"))?;
    let adaptateur: u32 = adaptateur
        .parse()
        .with_context(|| format!("index d'adaptateur « {adaptateur} » n'est pas un entier"))?;
    let sortie: u32 = sortie
        .parse()
        .with_context(|| format!("index de sortie « {sortie} » n'est pas un entier"))?;
    Ok((adaptateur, sortie))
}

/// Rapport entre les dimensions ANNONCÉES par la sortie
/// (`DXGI_OUTPUT_DESC::DesktopCoordinates`) et celles de la texture
/// RÉELLEMENT rendue par l'acquisition.
///
/// La sonde a relevé une sortie virtuelle annoncée 3413×960 par DXGI quand
/// WMI la disait 5120×1440 — rapport 1,5006, la mise à l'échelle DPI à 150 %.
/// Si un recadrage est calculé sur le rectangle annoncé alors que la texture
/// est aux dimensions physiques, il est décalé d'autant. Rend `None` si
/// l'annonce est dégénérée : un rapport n'y aurait aucun sens.
pub fn facteur_echelle(annonce: (u32, u32), texture: (u32, u32)) -> Option<(f64, f64)> {
    if annonce.0 == 0 || annonce.1 == 0 {
        return None;
    }
    Some((
        texture.0 as f64 / annonce.0 as f64,
        texture.1 as f64 / annonce.1 as f64,
    ))
}

/// Convertit un rectangle exprimé en coordonnées du bureau virtuel — celles
/// où vivent les fenêtres — vers les coordonnées de la texture rendue par
/// l'acquisition de `sortie`.
///
/// Deux corrections en une : le décalage de l'origine de la sortie dans le
/// bureau virtuel, et le facteur d'échelle de `facteur_echelle`.
pub fn vers_texture(region: Rect, sortie: Rect, facteur: (f64, f64)) -> Rect {
    let x = (region.x - sortie.x) as f64 * facteur.0;
    let y = (region.y - sortie.y) as f64 * facteur.1;
    Rect {
        x: x.round() as i32,
        y: y.round() as i32,
        width: (region.width as f64 * facteur.0).round() as u32,
        height: (region.height as f64 * facteur.1).round() as u32,
    }
}
```

- [ ] **Step 4: Vérifier que les tests passent**

Run: `cargo test -p agent moniteurs_virtuels 2>&1 | tail -20`
Expected: PASS — 11 tests.

Puis la suite entière, pour s'assurer que rien n'a régressé :

Run: `cargo test -p agent 2>&1 | tail -3`
Expected: `185 passed; 0 failed`

- [ ] **Step 5: Commit**

```bash
git add agent/src/moniteurs_virtuels.rs agent/src/main.rs
git commit -m "feat(moniteurs): trait du pilote, garde de destruction, conversions

Une sortie virtuelle survit au processus : une sonde qui plante à la
cinquième création laisserait cinq moniteurs derrière elle. La garde couvre
ce cas, panique comprise, et c'est testé.

Module NON gardé par cfg(windows), délibérément : sous le module diagnostics
ces tests ne tourneraient sur aucune machine de développement.

Le facteur d'échelle reprend le piège relevé par la sonde — sortie annoncée
3413x960 quand la texture fait 5120x1440.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 5: Le pilote concret

**Pourquoi séparé de la tâche 6.** C'est le moment de vérité de la spec §6.1 : un reviewer doit pouvoir accepter ou rejeter « notre code fait apparaître une sortie DXGI » indépendamment de la montée en N.

**Files:**
- Create: `agent/src/diagnostics/multifenetre/moniteurs.rs`
- Modify: `agent/src/diagnostics/multifenetre.rs` (déclaration du sous-module)
- Modify: `agent/Cargo.toml` (features `windows` supplémentaires, **hypothèse B seulement**)

**Interfaces:**
- Consumes: le canal consigné par la tâche 3 (`canal-de-controle.md`) ; `crate::moniteurs_virtuels::{IdSortie, PiloteAffichageVirtuel}`.
- Produces: `pub(super) fn ouvrir_pilote() -> anyhow::Result<PiloteParDll>` (hypothèse A) ou `-> anyhow::Result<PiloteParIoctl>` (hypothèse B) — le type concret, pas un `impl Trait` : les tâches 6 et 7 en prennent une référence, que Rust coerce vers `&dyn PiloteAffichageVirtuel`.

- [ ] **Step 1: Relire le verdict de la tâche 3**

```bash
cat docs/superpowers/plans/journaux-mesures-prealables/canal-de-controle.md
```

Selon la forme du verdict, implémenter **A** ou **B** ci-dessous, et supprimer l'autre. Si le verdict est **C**, arrêter cette tâche, installer un pilote d'affichage virtuel à interface publique sur la VM, et reprendre la tâche 3 sur ce nouveau pilote — c'est le repli acté par la spec §6.4.

- [ ] **Step 2A: Implémenter le pilote par DLL (hypothèse A)**

```rust
//! Mesure ① : le pilote d'affichage virtuel, commandé depuis notre code.
//!
//! La spec §2 tranche : on ne relance pas Apollo pour obtenir cette mesure.
//! D'abord parce qu'elle dépendrait d'un second appareil client apparié que
//! le propriétaire du poste n'a pas — c'est ce qui a bloqué la sonde. Ensuite
//! parce que le produit devra de toute façon se passer d'Apollo.
//!
//! Canal de contrôle relevé par la tâche 3 :
//! `docs/superpowers/plans/journaux-mesures-prealables/canal-de-controle.md`.

use anyhow::{anyhow, Context, Result};
use windows::core::{s, PCSTR};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};

use crate::moniteurs_virtuels::{IdSortie, PiloteAffichageVirtuel};

/// Signatures relevées à la tâche 3. **À ajuster à ce que dit
/// `canal-de-controle.md`** — c'est le seul point de ce plan qui dépend d'un
/// fait que seule la reconnaissance pouvait établir.
type FnCreer = unsafe extern "system" fn(u32, u32, u32) -> i32;
type FnDetruire = unsafe extern "system" fn(i32) -> i32;

pub(super) struct PiloteParDll {
    /// Gardé vivant : décharger la DLL invaliderait les pointeurs ci-dessous.
    _module: HMODULE,
    creer: FnCreer,
    detruire: FnDetruire,
}

pub(super) fn ouvrir_pilote() -> Result<PiloteParDll> {
    // Chemin relevé à la tâche 3.
    let module = unsafe { LoadLibraryA(s!("SudoVDA.dll")) }
        .context("chargement de la DLL de contrôle du pilote d'affichage virtuel")?;
    let creer = unsafe { GetProcAddress(module, s!("AddVirtualDisplay")) }
        .ok_or_else(|| anyhow!("AddVirtualDisplay absente de la DLL de contrôle"))?;
    let detruire = unsafe { GetProcAddress(module, s!("RemoveVirtualDisplay")) }
        .ok_or_else(|| anyhow!("RemoveVirtualDisplay absente de la DLL de contrôle"))?;
    Ok(PiloteParDll {
        _module: module,
        creer: unsafe { std::mem::transmute::<_, FnCreer>(creer) },
        detruire: unsafe { std::mem::transmute::<_, FnDetruire>(detruire) },
    })
}

impl PiloteAffichageVirtuel for PiloteParDll {
    fn creer(&self, largeur: u32, hauteur: u32, hertz: u32) -> Result<IdSortie> {
        let rendu = unsafe { (self.creer)(largeur, hauteur, hertz) };
        // Un identifiant négatif est le code d'erreur du pilote : la
        // convention exacte est celle relevée à la tâche 3.
        if rendu < 0 {
            anyhow::bail!("le pilote refuse une sortie {largeur}x{hauteur}@{hertz} (code {rendu})");
        }
        Ok(rendu as IdSortie)
    }

    fn detruire(&self, id: IdSortie) -> Result<()> {
        let rendu = unsafe { (self.detruire)(id as i32) };
        anyhow::ensure!(rendu >= 0, "le pilote refuse de détruire la sortie {id} (code {rendu})");
        Ok(())
    }
}
```

Le `PCSTR` importé sert si `canal-de-controle.md` impose de charger la DLL par chemin absolu plutôt que par nom : remplacer alors `s!("SudoVDA.dll")` par `PCSTR(chemin_nul_termine.as_ptr())`.

- [ ] **Step 2B: Implémenter le pilote par IOCTL (hypothèse B)**

Ajouter d'abord les features à `agent/Cargo.toml`, dans la liste `windows = { version = "0.62", features = [...] }`, en respectant l'ordre alphabétique existant :

```toml
    "Win32_Devices_DeviceAndDriverInstallation",
    "Win32_Storage_FileSystem",
    "Win32_System_IO",
```

Puis :

```rust
//! Mesure ① : le pilote d'affichage virtuel, commandé depuis notre code.
//!
//! (Même commentaire de tête que l'hypothèse A — le motif du choix ne dépend
//! pas du canal retenu.)

use anyhow::{anyhow, Context, Result};
use windows::core::GUID;
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
    SetupDiGetDeviceInterfaceDetailW, DIGCF_DEVICEINTERFACE, DIGCF_PRESENT,
    SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W,
};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_READ,
    FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::IO::DeviceIoControl;

use crate::moniteurs_virtuels::{IdSortie, PiloteAffichageVirtuel};

/// GUID d'interface et codes IOCTL relevés à la tâche 3.
const INTERFACE_PILOTE: GUID = GUID::from_u128(0x0000_0000_0000_0000_0000_0000_0000_0000);
const IOCTL_CREER: u32 = 0;
const IOCTL_DETRUIRE: u32 = 0;

/// Tampon d'entrée de `IOCTL_CREER`, disposition relevée à la tâche 3.
#[repr(C)]
struct DemandeSortie {
    largeur: u32,
    hauteur: u32,
    hertz: u32,
}

pub(super) struct PiloteParIoctl {
    handle: HANDLE,
}

pub(super) fn ouvrir_pilote() -> Result<PiloteParIoctl> {
    let chemin = chemin_du_peripherique()?;
    let handle = unsafe {
        CreateFileW(
            windows::core::PCWSTR(chemin.as_ptr()),
            (FILE_GENERIC_READ | FILE_GENERIC_WRITE).0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
    }
    .context("ouverture du périphérique du pilote d'affichage virtuel")?;
    Ok(PiloteParIoctl { handle })
}

/// Résout le chemin `\\?\...` du périphérique qui expose `INTERFACE_PILOTE`.
fn chemin_du_peripherique() -> Result<Vec<u16>> {
    // `SetupDiGetClassDevsW` puis `SetupDiEnumDeviceInterfaces(…, 0, …)` :
    // le pilote n'expose qu'une instance de cette interface. Le détail se lit
    // en deux appels à `SetupDiGetDeviceInterfaceDetailW`, le premier pour la
    // taille, le second pour le contenu — patron imposé par SetupAPI.
    // Implémentation à écrire ici selon `canal-de-controle.md` ; libérer la
    // liste par `SetupDiDestroyDeviceInfoList` sur tous les chemins.
    todo!("résolution du chemin — voir canal-de-controle.md")
}

impl PiloteAffichageVirtuel for PiloteParIoctl {
    fn creer(&self, largeur: u32, hauteur: u32, hertz: u32) -> Result<IdSortie> {
        let demande = DemandeSortie { largeur, hauteur, hertz };
        let mut id: IdSortie = 0;
        let mut rendus = 0u32;
        unsafe {
            DeviceIoControl(
                self.handle,
                IOCTL_CREER,
                Some(&demande as *const _ as *const _),
                std::mem::size_of::<DemandeSortie>() as u32,
                Some(&mut id as *mut _ as *mut _),
                std::mem::size_of::<IdSortie>() as u32,
                Some(&mut rendus),
                None,
            )
        }
        .with_context(|| format!("création d'une sortie {largeur}x{hauteur}@{hertz}"))?;
        Ok(id)
    }

    fn detruire(&self, id: IdSortie) -> Result<()> {
        let mut rendus = 0u32;
        unsafe {
            DeviceIoControl(
                self.handle,
                IOCTL_DETRUIRE,
                Some(&id as *const _ as *const _),
                std::mem::size_of::<IdSortie>() as u32,
                None,
                0,
                Some(&mut rendus),
                None,
            )
        }
        .with_context(|| format!("destruction de la sortie {id}"))
    }
}

impl Drop for PiloteParIoctl {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.handle) };
    }
}
```

**Le `todo!()` ci-dessus n'est pas un reste de plan** : il marque le seul endroit dont le contenu dépend d'un GUID que seule la tâche 3 pouvait établir. Il doit être remplacé par du code avant la fin de cette tâche, et `INTERFACE_PILOTE`, `IOCTL_CREER` et `IOCTL_DETRUIRE` doivent porter les valeurs relevées.

- [ ] **Step 3: Déclarer le sous-module**

Dans `agent/src/diagnostics/multifenetre.rs`, ajouter à la liste des `pub(super) mod` (ordre alphabétique) :

```rust
pub(super) mod moniteurs;
```

- [ ] **Step 4: Vérifier qu'aucun `todo!()` ne subsiste**

Run: `grep -n 'todo!\|GUID::from_u128(0x0000_0000' agent/src/diagnostics/multifenetre/moniteurs.rs`
Expected: aucune correspondance. Une correspondance signifie que le fait relevé à la tâche 3 n'a pas été reporté dans le code, et la tâche n'est pas finie.

- [ ] **Step 5: Compiler sur la VM**

Run: `scripts/build-agent.sh`
Expected: compilation réussie, aucun avertissement sur `moniteurs.rs`.

- [ ] **Step 6: Commit**

```bash
git add agent/src/diagnostics/multifenetre/moniteurs.rs agent/src/diagnostics/multifenetre.rs agent/Cargo.toml
git commit -m "feat(moniteurs): commander le pilote d'affichage virtuel

Le canal de contrôle relevé à la tâche précédente, derrière le trait
PiloteAffichageVirtuel — de sorte que le repli acté par la spec (changer de
pilote) ne fasse réécrire ni la montée en N ni la garde.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 6: Mesure ① — la montée en N

**Files:**
- Modify: `agent/src/diagnostics/multifenetre/moniteurs.rs` (sonde `monter_en_n`)
- Modify: `agent/src/diagnostics/multifenetre.rs` (aiguillage + `causes` partagé)
- Modify: `agent/src/diagnostics/multifenetre/nvenc.rs` (retirer le `causes` local)
- Modify: `scripts/run-agent.sh` (passage de `MULTIFENETRE_VDD`)
- Create: `docs/superpowers/plans/journaux-mesures-prealables/moniteurs-montee-en-n.log`

**Interfaces:**
- Consumes: `ouvrir_pilote()` (tâche 5), `crate::moniteurs_virtuels::Sorties`, `crate::capture::enumerer_sorties() -> Result<Vec<SortieDxgi>>` où `SortieDxgi { index_adaptateur: u32, index_sortie: u32, adaptateur: String, nom_sortie: String, attachee_au_bureau: bool, rect: Rect }`.
- Produces: `pub(super) fn causes(erreur: impl Into<anyhow::Error>) -> String` remonté dans `multifenetre.rs`, utilisé par `nvenc.rs` (tâche 9) et `moniteurs.rs`.

- [ ] **Step 1: Remonter `causes` au module parent**

Déplacer la fonction `causes` de `nvenc.rs:48-55` vers `agent/src/diagnostics/multifenetre.rs`, en la rendant `pub(super)` et en gardant son commentaire :

```rust
/// Chaîne complète des causes d'une erreur, du contexte le plus englobant au
/// HRESULT sous-jacent — sans cela, une erreur contextualisée par
/// `H264Encoder::new` (ex. `.context("partage du périphérique D3D avec
/// l'encodeur")`) n'afficherait que ce contexte et perdrait le code d'erreur
/// natif. Voir le défaut équivalent corrigé à la tâche 6 de la sonde.
pub(super) fn causes(erreur: impl Into<anyhow::Error>) -> String {
    erreur
        .into()
        .chain()
        .map(|cause| cause.to_string())
        .collect::<Vec<_>>()
        .join(" : ")
}
```

Dans `nvenc.rs`, supprimer la fonction locale et remplacer `causes(erreur)` par `super::causes(erreur)`.

- [ ] **Step 2: Écrire la sonde de montée en N**

Ajouter dans `agent/src/diagnostics/multifenetre/moniteurs.rs` :

```rust
/// Au-delà, on cesse de chercher : le chantier D vise 8 fenêtres, et la
/// sonde d'encodeurs emploie déjà ce même plafond de recherche.
const PLAFOND_RECHERCHE: usize = 16;

/// Résolution demandée à chaque sortie : celle que le chantier D vise par
/// fenêtre, pas celle du bureau.
const RESOLUTION: (u32, u32, u32) = (1280, 720, 60);

/// Un pilote d'affichage indirect ne publie pas sa sortie dans l'instant :
/// Windows reconfigure sa topologie d'affichage. Interroger DXGI trop tôt
/// ferait conclure à un refus là où il n'y a qu'un délai.
const DELAI_TOPOLOGIE: std::time::Duration = std::time::Duration::from_secs(3);

pub(super) fn monter_en_n() -> Result<()> {
    // Relevé AVANT toute création : sans lui, une restauration manuelle après
    // plantage se ferait à l'aveugle (spec §6.3).
    let avant = crate::capture::enumerer_sorties()?;
    let attachees_avant = avant.iter().filter(|s| s.attachee_au_bureau).count();
    tracing::info!(
        nombre = avant.len(),
        attachees = attachees_avant,
        "topologie AVANT toute création"
    );
    for sortie in &avant {
        tracing::info!(
            nom = %sortie.nom_sortie,
            adaptateur = %sortie.adaptateur,
            index_adaptateur = sortie.index_adaptateur,
            index_sortie = sortie.index_sortie,
            attachee = sortie.attachee_au_bureau,
            largeur = sortie.rect.width,
            hauteur = sortie.rect.height,
            "sortie initiale"
        );
    }

    let pilote = ouvrir_pilote()?;
    let mut sorties = crate::moniteurs_virtuels::Sorties::nouvelles(&pilote);
    let (largeur, hauteur, hertz) = RESOLUTION;

    for rang in 1..=PLAFOND_RECHERCHE {
        match sorties.creer(largeur, hauteur, hertz) {
            Err(erreur) => {
                tracing::info!(
                    plafond = rang - 1,
                    causes = %super::causes(erreur),
                    "plafond de sorties virtuelles atteint — le pilote refuse la suivante"
                );
                return Ok(());
            }
            Ok(id) => {
                std::thread::sleep(DELAI_TOPOLOGIE);
                let apres = crate::capture::enumerer_sorties()?;
                let attachees = apres.iter().filter(|s| s.attachee_au_bureau).count();
                tracing::info!(
                    rang,
                    id,
                    sorties_dxgi = apres.len(),
                    attachees,
                    "sortie virtuelle créée"
                );
                for sortie in apres.iter().filter(|s| s.attachee_au_bureau) {
                    tracing::info!(
                        rang,
                        nom = %sortie.nom_sortie,
                        index_adaptateur = sortie.index_adaptateur,
                        index_sortie = sortie.index_sortie,
                        largeur = sortie.rect.width,
                        hauteur = sortie.rect.height,
                        "sortie attachée après création"
                    );
                }
                // Le cas qui ferait basculer tout l'arbitrage du chantier D :
                // le pilote accepte la demande mais REMPLACE au lieu
                // d'ajouter. C'est ce qu'a fait Apollo pendant la sonde, par
                // son réglage `ensure_only_display` — on vérifie ici que le
                // pilote nu ne le fait pas.
                if attachees < attachees_avant + rang {
                    tracing::error!(
                        rang,
                        attachees,
                        attendu = attachees_avant + rang,
                        "le pilote a accepté la demande mais le compte de sorties ne suit pas \
                         — remplacement, pas addition"
                    );
                    return Ok(());
                }
            }
        }
    }

    tracing::info!(
        plafond_recherche = PLAFOND_RECHERCHE,
        "aucun plafond atteint sous {PLAFOND_RECHERCHE} sorties virtuelles"
    );
    Ok(())
}
```

- [ ] **Step 3: Aiguiller la sonde**

Dans `agent/src/diagnostics/multifenetre.rs`, avant le `Ok(false)` final :

```rust
    // Mesure ① de la spec : combien de sorties virtuelles simultanées ce
    // pilote accepte. Sans ce chiffre, la voie « un moniteur virtuel par
    // fenêtre » n'est pas spécifiable.
    if std::env::var("MULTIFENETRE_VDD").is_ok() {
        moniteurs::monter_en_n()?;
        return Ok(true);
    }
```

Dans `scripts/run-agent.sh`, ajouter à la liste des variables transmises, après `MULTIFENETRE_NVENC` :

```
${MULTIFENETRE_VDD:+\$env:MULTIFENETRE_VDD = '$MULTIFENETRE_VDD'}
```

- [ ] **Step 4: Vérifier que rien n'a régressé côté Linux**

Run: `cargo test -p agent 2>&1 | tail -3`
Expected: `185 passed; 0 failed`

- [ ] **Step 5: Mesurer sur la VM**

```bash
scripts/build-agent.sh
MULTIFENETRE_VDD=1 scripts/run-agent.sh
sleep 90        # jusqu'à 16 créations × 3 s de délai de topologie
cp /media/vm/dev/agent.log docs/superpowers/plans/journaux-mesures-prealables/moniteurs-montee-en-n.log
grep -E "topologie AVANT|sortie virtuelle créée|plafond|remplacement" \
  docs/superpowers/plans/journaux-mesures-prealables/moniteurs-montee-en-n.log
```

Attendu, l'un des trois : une ligne `plafond de sorties virtuelles atteint` avec sa valeur ; une ligne `remplacement, pas addition` ; ou `aucun plafond atteint sous 16 sorties virtuelles`.

- [ ] **Step 6: Vérifier que la VM est revenue à son état initial**

```bash
MULTIFENETRE_DXGI=1 scripts/run-agent.sh
sleep 20
grep "sorties DXGI relevées" /media/vm/dev/agent.log
```

Attendu : le même nombre de sorties que la ligne `topologie AVANT` du journal précédent. Si ce n'est pas le cas, la garde a échoué — passer à la tâche 7 avant toute autre mesure, et le consigner comme défaut.

- [ ] **Step 7: Commit**

```bash
git add agent/src/diagnostics/multifenetre/moniteurs.rs agent/src/diagnostics/multifenetre.rs \
        agent/src/diagnostics/multifenetre/nvenc.rs scripts/run-agent.sh \
        docs/superpowers/plans/journaux-mesures-prealables/moniteurs-montee-en-n.log
git commit -m "mesure(moniteurs): plafond de sorties virtuelles simultanées

Première des deux mesures que la sonde qualifiait de bloquantes. Le compte
de sorties attachées est relevé après CHAQUE création : un pilote qui
remplace au lieu d'ajouter, comme le faisait Apollo par ensure_only_display,
se voit tout de suite.

causes() remonte au module parent, nvenc.rs et moniteurs.rs le partagent.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 7: Purge autonome

**Pourquoi séparé.** La garde de la tâche 4 couvre le processus qui panique ; elle ne couvre pas le processus tué net ni un plantage dans le pilote lui-même. La spec §6.3 exige un rattrapage sans redémarrage de la VM.

**Files:**
- Modify: `agent/src/diagnostics/multifenetre/moniteurs.rs`
- Modify: `agent/src/diagnostics/multifenetre.rs`
- Modify: `scripts/run-agent.sh`

**Interfaces:**
- Consumes: `ouvrir_pilote()`, `PLAFOND_RECHERCHE`, `crate::capture::enumerer_sorties`.
- Produces: la sonde `MULTIFENETRE_VDD_PURGE`, utilisable seule à tout moment.

- [ ] **Step 1: Écrire la purge**

Ajouter dans `moniteurs.rs` :

```rust
/// Détruit toute sortie virtuelle laissée par une exécution précédente.
///
/// La garde de `moniteurs_virtuels::Sorties` couvre la panique ; elle ne
/// couvre ni un processus tué net, ni un plantage à l'intérieur du pilote.
/// Sans cette purge, le rattrapage exigerait de redémarrer la VM.
///
/// On tente les identifiants de 1 à `PLAFOND_RECHERCHE` sans rien présumer :
/// le pilote n'offre pas nécessairement d'énumérer ce qu'il a créé, et un
/// refus sur un identifiant libre est ici l'issue NORMALE, pas une erreur.
pub(super) fn purger() -> Result<()> {
    let avant = crate::capture::enumerer_sorties()?;
    tracing::info!(nombre = avant.len(), "topologie avant purge");

    let pilote = ouvrir_pilote()?;
    let mut retirees = 0usize;
    for id in 1..=PLAFOND_RECHERCHE as crate::moniteurs_virtuels::IdSortie {
        if pilote.detruire(id).is_ok() {
            retirees += 1;
            tracing::info!(id, "sortie virtuelle retirée");
        }
    }

    std::thread::sleep(DELAI_TOPOLOGIE);
    let apres = crate::capture::enumerer_sorties()?;
    tracing::info!(
        retirees,
        avant = avant.len(),
        apres = apres.len(),
        "purge terminée"
    );
    Ok(())
}
```

- [ ] **Step 2: Aiguiller la purge AVANT la montée en N**

Dans `agent/src/diagnostics/multifenetre.rs`, placer ce bloc **au-dessus** de celui de `MULTIFENETRE_VDD` — une purge demandée doit l'emporter sur une mesure, jamais l'inverse :

```rust
    // Rattrapage : détruit les sorties virtuelles laissées par une exécution
    // tuée net, que la garde de `moniteurs_virtuels::Sorties` ne peut pas
    // couvrir. Placée avant la montée en N : si les deux variables sont
    // posées, on purge.
    if std::env::var("MULTIFENETRE_VDD_PURGE").is_ok() {
        moniteurs::purger()?;
        return Ok(true);
    }
```

Dans `scripts/run-agent.sh`, à côté de `MULTIFENETRE_VDD` :

```
${MULTIFENETRE_VDD_PURGE:+\$env:MULTIFENETRE_VDD_PURGE = '$MULTIFENETRE_VDD_PURGE'}
```

- [ ] **Step 3: Éprouver la purge sur un état réellement sale**

```bash
scripts/build-agent.sh
# Salir : créer des sorties puis tuer le processus avant que la garde ne court.
MULTIFENETRE_VDD=1 scripts/run-agent.sh
sleep 15
node scripts/winrm.js "Stop-Process -Name agent -Force"
sleep 5
MULTIFENETRE_DXGI=1 scripts/run-agent.sh && sleep 20
grep "sorties DXGI relevées" /media/vm/dev/agent.log     # doit montrer des sorties EN TROP
```

- [ ] **Step 4: Purger et vérifier le retour à l'état initial**

```bash
MULTIFENETRE_VDD_PURGE=1 scripts/run-agent.sh
sleep 30
grep -E "topologie avant purge|sortie virtuelle retirée|purge terminée" /media/vm/dev/agent.log
MULTIFENETRE_DXGI=1 scripts/run-agent.sh && sleep 20
grep "sorties DXGI relevées" /media/vm/dev/agent.log
```

Attendu : le nombre de sorties revient à celui de la ligne `topologie AVANT` de la tâche 6.

Si l'étape 3 n'a produit aucune sortie en trop — parce que la garde a couru malgré `Stop-Process` — le consigner : c'est un résultat favorable, et la purge reste due comme filet.

- [ ] **Step 5: Commit**

```bash
git add agent/src/diagnostics/multifenetre/moniteurs.rs agent/src/diagnostics/multifenetre.rs scripts/run-agent.sh
git commit -m "feat(moniteurs): purge autonome des sorties virtuelles

La garde RAII couvre la panique, pas le processus tué net. Sans cette purge,
le rattrapage exigerait de redémarrer la VM.

Un refus sur un identifiant libre est l'issue normale, pas une erreur : le
pilote n'offre pas nécessairement d'énumérer ce qu'il a créé.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 8: Mesure ③ — capture réelle sur la sortie virtuelle

**Pourquoi un seul processus.** La garde détruit les sorties à la fin du processus : un banc lancé séparément ne trouverait plus rien. Cette sonde crée donc **une** sortie, attend la topologie, mesure la concordance d'échelle, puis fait tourner le banc dessus — le tout sans rendre la main.

**Files:**
- Modify: `agent/src/diagnostics/multifenetre/voies.rs` (`VoieDuplication::partagee_sur`)
- Modify: `agent/src/diagnostics/multifenetre/banc.rs` (`executer` prend une sortie et un bureau)
- Modify: `agent/src/diagnostics/multifenetre/moniteurs.rs` (sonde `capturer_sur_virtuelle`)
- Modify: `agent/src/diagnostics/multifenetre.rs`, `scripts/run-agent.sh`
- Create: `docs/superpowers/plans/journaux-mesures-prealables/moniteurs-capture.log`

**Interfaces:**
- Consumes: `crate::moniteurs_virtuels::{facteur_echelle, vers_texture, analyser_designation}`, `crate::capture::DesktopCapture::sur_sortie(index_adaptateur: u32, index_sortie: u32) -> Result<DesktopCapture>` (existe déjà, `capture.rs:76`), `crate::disposition::tuiles(bureau: Rect, n: u32) -> Option<Vec<Rect>>`.
- Produces: `banc::executer(nom_voie: &str, nombre: u8, sortie: Option<(u32, u32)>) -> Result<()>`.

- [ ] **Step 1: Permettre au banc de dupliquer une sortie désignée**

Dans `voies.rs`, à côté de `VoieDuplication::partagee` (ligne 167), ajouter :

```rust
    /// Comme `partagee`, mais sur une sortie DXGI désignée par ses index.
    ///
    /// Le bureau retourné par `desktop_size()` est celui de CETTE sortie,
    /// dans ses dimensions de mode — c'est-à-dire potentiellement les
    /// dimensions physiques, là où `DXGI_OUTPUT_DESC::DesktopCoordinates`
    /// donne les dimensions mises à l'échelle. Voir
    /// `moniteurs_virtuels::facteur_echelle`.
    pub(super) fn partagee_sur(
        sortie: Option<(u32, u32)>,
    ) -> Result<Rc<RefCell<SourceDuplication>>> {
        let capture = match sortie {
            Some((adaptateur, index)) => DesktopCapture::sur_sortie(adaptateur, index)?,
            None => DesktopCapture::new()?,
        };
        let (largeur, hauteur) = capture.desktop_size();
        let contexte = unsafe { capture.device().GetImmediateContext() }
            .context("contexte immédiat pour les sous-recadrages partagés")?;
        Ok(Rc::new(RefCell::new(SourceDuplication {
            capture,
            bureau: Rect { x: 0, y: 0, width: largeur, height: hauteur },
            contexte,
            dernier_tour: None,
            dernier_bureau: None,
        })))
    }
```

Puis réécrire `partagee` pour qu'elle délègue, afin qu'il n'existe qu'un seul chemin de construction :

```rust
    /// Crée la duplication partagée, une fois pour tout le banc.
    pub(super) fn partagee() -> Result<Rc<RefCell<SourceDuplication>>> {
        Self::partagee_sur(None)
    }
```

- [ ] **Step 2: Faire passer la désignation jusqu'au banc, et séparer les deux systèmes de coordonnées**

C'est ici que `vers_texture` sert : **les fenêtres vivent dans le bureau virtuel, la texture rendue par l'acquisition est aux dimensions du mode**. Sur la sortie relevée par la sonde, les deux diffèrent d'un facteur 1,5. Recadrer une région de fenêtre dans la texture sans conversion décalerait tout, et le verdict de correction ne voudrait plus rien dire — la mesure ③ mesurerait son propre défaut.

Dans `banc.rs`, changer la signature et la tête de `executer` :

```rust
pub(super) fn executer(nom_voie: &str, nombre: u8, sortie: Option<(u32, u32)>) -> Result<()> {
    anyhow::ensure!(
        nombre >= 1 && nombre <= mire::MIRES_MAX,
        "MULTIFENETRE_N doit valoir 1 à {}",
        mire::MIRES_MAX
    );

    let capture = match sortie {
        Some((adaptateur, index)) => crate::capture::DesktopCapture::sur_sortie(adaptateur, index)?,
        None => crate::capture::DesktopCapture::new()?,
    };
    let (texture_largeur, texture_hauteur) = capture.desktop_size();

    // Le rectangle où vivent les FENÊTRES : les coordonnées du bureau
    // virtuel, telles que `DXGI_OUTPUT_DESC::DesktopCoordinates` les donne.
    // Sans sortie désignée, c'est le bureau à l'origine — le comportement
    // d'avant, inchangé.
    let bureau = match sortie {
        Some((adaptateur, index)) => crate::capture::enumerer_sorties()?
            .into_iter()
            .find(|s| s.index_adaptateur == adaptateur && s.index_sortie == index)
            .map(|s| s.rect)
            .with_context(|| format!("sortie {adaptateur}:{index} absente de l'énumération"))?,
        None => Rect { x: 0, y: 0, width: texture_largeur, height: texture_hauteur },
    };
    let facteur = crate::moniteurs_virtuels::facteur_echelle(
        (bureau.width, bureau.height),
        (texture_largeur, texture_hauteur),
    )
    .unwrap_or((1.0, 1.0));
    tracing::info!(
        bureau_x = bureau.x,
        bureau_y = bureau.y,
        bureau_largeur = bureau.width,
        bureau_hauteur = bureau.height,
        texture_largeur,
        texture_hauteur,
        facteur_horizontal = facteur.0,
        facteur_vertical = facteur.1,
        "banc : coordonnées de fenêtre et de texture"
    );

    let places = disposition::tuiles(bureau, nombre as u32).with_context(|| {
        format!("{nombre} places sur un bureau {}x{}", bureau.width, bureau.height)
    })?;
    let places_texture: Vec<Rect> = places
        .iter()
        .map(|place| crate::moniteurs_virtuels::vers_texture(*place, bureau, facteur))
        .collect();
    tracing::info!(
        voie = nom_voie,
        nombre,
        ?places,
        ?places_texture,
        "banc : disposition retenue"
    );

    let mut mires = Mires::ouvrir(capture.device(), &places)?;
```

Le reste de `executer` est inchangé, sauf l'appel à `ouvrir_voies`, qui reçoit maintenant les deux dispositions :

```rust
    let mut voies = ouvrir_voies(nom_voie, nombre, &mires, &places, &places_texture, sortie)?;
```

Puis, dans `ouvrir_voies`, prendre les deux tranches et **choisir selon la voie** :

```rust
/// Deux dispositions, et elles ne sont pas interchangeables :
/// `places_fenetres` est en coordonnées du bureau virtuel — c'est là que sont
/// les fenêtres, et `PrintWindow` travaille sur la fenêtre elle-même ;
/// `places_texture` est en coordonnées de la texture dupliquée — c'est là que
/// recadre `CopySubresourceRegion`. Elles ne coïncident que si la sortie n'est
/// pas mise à l'échelle, ce qui est le cas du bureau physique mais pas
/// nécessairement d'une sortie virtuelle (facteur 1,5 relevé par la sonde).
fn ouvrir_voies(
    nom_voie: &str,
    nombre: u8,
    mires: &Mires,
    places_fenetres: &[Rect],
    places_texture: &[Rect],
    sortie: Option<(u32, u32)>,
) -> Result<Vec<Box<dyn VoieDeCapture>>> {
```

et, dans le corps :

- branche `"duplication"` : `VoieDuplication::partagee()?` devient `VoieDuplication::partagee_sur(sortie)?`, et `places[id as usize]` devient `places_texture[id as usize]` ;
- branche `"printwindow"` : `places[id as usize]` devient `places_fenetres[id as usize]`.

Dans `multifenetre.rs`, l'appel existant devient :

```rust
        let sortie = match std::env::var("MULTIFENETRE_SORTIE") {
            Ok(designation) => Some(crate::moniteurs_virtuels::analyser_designation(&designation)?),
            Err(_) => None,
        };
        banc::executer(&voie, nombre, sortie)?;
```

Et dans `scripts/run-agent.sh` :

```
${MULTIFENETRE_SORTIE:+\$env:MULTIFENETRE_SORTIE = '$MULTIFENETRE_SORTIE'}
```

- [ ] **Step 3: Écrire la sonde de capture sur sortie virtuelle**

Ajouter dans `moniteurs.rs` :

```rust
/// Mesure ③ : une fenêtre posée sur un moniteur virtuel est-elle capturée
/// correctement, et le rectangle annoncé s'accorde-t-il à la texture ?
///
/// Un seul processus, contrairement aux autres sondes de ce module : la
/// garde détruit la sortie à la sortie du processus, donc un banc lancé
/// séparément ne trouverait plus rien à capturer.
///
/// Que Windows compose réellement des fenêtres sur un moniteur virtuel sans
/// écran attaché est l'hypothèse FONDATRICE de la voie 2, et elle n'a jamais
/// été vérifiée. Une image noire ici est un résultat, pas une panne.
pub(super) fn capturer_sur_virtuelle(nombre: u8) -> Result<()> {
    let avant: std::collections::HashSet<String> = crate::capture::enumerer_sorties()?
        .into_iter()
        .map(|sortie| sortie.nom_sortie)
        .collect();

    let pilote = ouvrir_pilote()?;
    let mut sorties = crate::moniteurs_virtuels::Sorties::nouvelles(&pilote);
    let (largeur, hauteur, hertz) = RESOLUTION;
    let id = sorties.creer(largeur, hauteur, hertz)?;
    std::thread::sleep(DELAI_TOPOLOGIE);

    // La sortie neuve est celle dont le nom n'était pas là avant : identifier
    // par index serait fragile, DXGI renumérotant à chaque reconfiguration.
    let apres = crate::capture::enumerer_sorties()?;
    let virtuelle = apres
        .iter()
        .find(|sortie| !avant.contains(&sortie.nom_sortie))
        .ok_or_else(|| anyhow!("aucune sortie DXGI neuve après création de la sortie {id}"))?;
    tracing::info!(
        id,
        nom = %virtuelle.nom_sortie,
        index_adaptateur = virtuelle.index_adaptateur,
        index_sortie = virtuelle.index_sortie,
        x = virtuelle.rect.x,
        y = virtuelle.rect.y,
        largeur_annoncee = virtuelle.rect.width,
        hauteur_annoncee = virtuelle.rect.height,
        "sortie virtuelle retenue pour la capture"
    );

    // Le piège d'échelle est relevé et corrigé par le banc lui-même
    // (`banc : coordonnées de fenêtre et de texture`) : le mesurer une
    // seconde fois ici obligerait à ouvrir une duplication sur cette sortie,
    // or DXGI n'en autorise qu'UNE — celle du banc échouerait alors en
    // 0x80070057, comme au temps 2 de la sonde.

    super::banc::executer(
        "duplication",
        nombre,
        Some((virtuelle.index_adaptateur, virtuelle.index_sortie)),
    )
}
```

- [ ] **Step 4: Aiguiller la sonde**

Dans `multifenetre.rs`, après le bloc `MULTIFENETRE_VDD` :

```rust
    // Mesure ③ : la correction d'image sur la sortie virtuelle, jamais
    // mesurée par la sonde — la voie 2 n'y était garantie que « par
    // construction ».
    if let Ok(texte) = std::env::var("MULTIFENETRE_VDD_CAPTURE") {
        let nombre: u8 = texte.parse().context("MULTIFENETRE_VDD_CAPTURE doit être un entier")?;
        moniteurs::capturer_sur_virtuelle(nombre)?;
        return Ok(true);
    }
```

Et dans `scripts/run-agent.sh` :

```
${MULTIFENETRE_VDD_CAPTURE:+\$env:MULTIFENETRE_VDD_CAPTURE = '$MULTIFENETRE_VDD_CAPTURE'}
```

- [ ] **Step 5: Vérifier la non-régression sur Linux**

Run: `cargo test -p agent 2>&1 | tail -3`
Expected: `185 passed; 0 failed`

- [ ] **Step 6: Mesurer**

```bash
scripts/build-agent.sh
MULTIFENETRE_VDD_CAPTURE=2 scripts/run-agent.sh
sleep 90
cp /media/vm/dev/agent.log docs/superpowers/plans/journaux-mesures-prealables/moniteurs-capture.log
grep -E "sortie virtuelle retenue|coordonnées de fenêtre et de texture|disposition retenue|verdicts_faux|verdict" \
  docs/superpowers/plans/journaux-mesures-prealables/moniteurs-capture.log
```

Trois issues, toutes trois des résultats :
- `verdicts_faux = 0` → la voie 2 franchit enfin la porte de correction ;
- `Verdict::Noire` → Windows ne compose pas sur un moniteur sans écran attaché, et l'hypothèse fondatrice de la voie 2 tombe ;
- `facteur_horizontal` ≠ 1 dans la ligne « coordonnées de fenêtre et de texture » → le piège d'échelle de la sonde est réel, et le chantier D devra convertir ses recadrages comme le fait `vers_texture`.

- [ ] **Step 7: Vérifier que la VM est propre**

```bash
MULTIFENETRE_DXGI=1 scripts/run-agent.sh && sleep 20
grep "sorties DXGI relevées" /media/vm/dev/agent.log
```

Attendu : le compte initial. Sinon, `MULTIFENETRE_VDD_PURGE=1 scripts/run-agent.sh`.

- [ ] **Step 8: Commit**

```bash
git add agent/src/diagnostics/multifenetre/ agent/src/diagnostics/multifenetre.rs \
        scripts/run-agent.sh docs/superpowers/plans/journaux-mesures-prealables/moniteurs-capture.log
git commit -m "mesure(moniteurs): correction d'image sur la sortie virtuelle

La sonde ne garantissait la voie 2 que « par construction », sans jamais
capturer dessus. Cette mesure la met à l'épreuve, et tranche au passage le
piège d'échelle 1,5 signalé mais jamais rencontré.

Un seul processus : la garde détruit la sortie en sortant, un banc lancé
séparément ne trouverait plus rien.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 9: Mesure ② — plafond d'encodage sur périphériques D3D11 séparés

**Files:**
- Modify: `agent/src/diagnostics/multifenetre/nvenc.rs`
- Modify: `scripts/run-agent.sh` (rien à ajouter : `MULTIFENETRE_NVENC` passe déjà)
- Create: `docs/superpowers/plans/journaux-mesures-prealables/nvenc-separe.log`

**Interfaces:**
- Consumes: `crate::encode::H264Encoder::new(device: &ID3D11Device, capture: (u32, u32), encode: (u32, u32), fps: u32, bitrate: u32) -> Result<H264Encoder>` ; `super::causes` (tâche 6).
- Produces: la mesure ② et le nom du composant qui refuse.

- [ ] **Step 1: Ajouter le périphérique autonome**

Dans `nvenc.rs`, ajouter les `use` nécessaires et la fabrique :

```rust
use anyhow::{anyhow, Context};
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_0};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Multithread,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION,
};

/// Un périphérique D3D11 neuf, sans lien avec la capture.
///
/// L'adaptateur est laissé au choix du système (`D3D_DRIVER_TYPE_HARDWARE`) :
/// sur cette VM il n'y a qu'un GPU réel, et c'est celui qui porte NVENC.
fn peripherique_autonome() -> Result<(ID3D11Device, ID3D11DeviceContext)> {
    let mut device: Option<ID3D11Device> = None;
    let mut contexte: Option<ID3D11DeviceContext> = None;
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            Default::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&[D3D_FEATURE_LEVEL_11_0]),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut contexte),
        )
        .context("création d'un périphérique D3D11 autonome")?;
    }
    let device = device.ok_or_else(|| anyhow!("périphérique D3D11 autonome absent"))?;
    let contexte = contexte.ok_or_else(|| anyhow!("contexte D3D11 autonome absent"))?;

    // Même protection que `DesktopCapture::ouvrir` : ce périphérique est
    // confié à Media Foundation, dont le convertisseur de couleur et
    // l'encodeur y entrent depuis leurs propres fils de travail. Sans elle,
    // deux fils entrent ensemble dans le pilote et l'un peut ne pas
    // ressortir — le blocage mesuré au jalon 1.
    let multithread: ID3D11Multithread = contexte
        .cast()
        .context("obtention de ID3D11Multithread sur le périphérique autonome")?;
    unsafe { multithread.SetMultithreadProtected(true) };

    Ok((device, contexte))
}
```

- [ ] **Step 2: Ouvrir le mode « séparé »**

Réécrire `plafond()` pour qu'elle prenne le mode :

```rust
/// `partage` reproduit la mesure de la sonde : UN périphérique D3D11 pour
/// tous les encodeurs. `separe` répond à la question qu'elle a laissée
/// ouverte : le refus de la 9ᵉ instance venait-il de l'encodeur matériel, ou
/// du partage du périphérique ?
pub(super) fn plafond(mode: &str) -> Result<()> {
    let partage = match mode {
        "partage" => Some(crate::capture::DesktopCapture::new()?),
        "separe" => None,
        autre => anyhow::bail!("MULTIFENETRE_NVENC vaut « partage » ou « separe », pas « {autre} »"),
    };
    tracing::info!(mode, "plafond d'encodeurs : mode retenu");

    // Les périphériques autonomes DOIVENT rester vivants aussi longtemps que
    // les encodeurs qui s'y appuient : les relâcher au tour suivant ferait
    // mesurer autre chose que ce qu'on croit.
    let mut peripheriques = Vec::new();
    let mut encodeurs = Vec::new();

    for rang in 1..=PLAFOND_RECHERCHE {
        let device = match &partage {
            Some(capture) => capture.device().clone(),
            None => {
                let (device, contexte) = peripherique_autonome()?;
                peripheriques.push((device.clone(), contexte));
                device
            }
        };
        match crate::encode::H264Encoder::new(&device, (1280, 720), (1280, 720), 60, 8_000_000) {
            Ok(encodeur) => {
                encodeurs.push(encodeur);
                tracing::info!(rang, mode, "encodeur créé");
            }
            Err(erreur) => {
                tracing::info!(
                    plafond = rang - 1,
                    mode,
                    causes = %super::causes(erreur),
                    "plafond d'encodeurs atteint — création du suivant refusée"
                );
                return Ok(());
            }
        }
    }
    tracing::info!(
        plafond_recherche = PLAFOND_RECHERCHE,
        mode,
        "aucun plafond atteint sous {PLAFOND_RECHERCHE} encodeurs"
    );
    Ok(())
}
```

Dans `multifenetre.rs`, l'aiguillage devient :

```rust
    // Mesure ② : le plafond d'encodeurs, sur périphérique partagé (la mesure
    // de la sonde) ou sur périphériques séparés (la question qu'elle laisse).
    if let Ok(mode) = std::env::var("MULTIFENETRE_NVENC") {
        nvenc::plafond(&mode)?;
        return Ok(true);
    }
```

- [ ] **Step 3: Vérifier la non-régression sur Linux**

Run: `cargo test -p agent 2>&1 | tail -3`
Expected: `185 passed; 0 failed`

- [ ] **Step 4: Reproduire la mesure de la sonde, en témoin**

```bash
scripts/build-agent.sh
MULTIFENETRE_NVENC=partage scripts/run-agent.sh
sleep 45
grep -E "mode retenu|encodeur créé|plafond" /media/vm/dev/agent.log
```

Attendu : `plafond = 8`, comme la sonde. Un chiffre différent invaliderait la comparaison de l'étape suivante et devrait être élucidé d'abord.

- [ ] **Step 5: Mesurer sur périphériques séparés**

```bash
MULTIFENETRE_NVENC=separe scripts/run-agent.sh
sleep 45
cp /media/vm/dev/agent.log docs/superpowers/plans/journaux-mesures-prealables/nvenc-separe.log
grep -E "mode retenu|encodeur créé|plafond|causes" \
  docs/superpowers/plans/journaux-mesures-prealables/nvenc-separe.log
```

La ligne `causes` nomme le composant fautif : `configuration du type d'entrée de l'encodeur H.264 (transform matériel)` ou `configuration du type d'entrée du convertisseur de couleur (Video Processor MFT)`. C'est ce que la sonde n'avait pas su dire.

- [ ] **Step 6: Commit**

```bash
git add agent/src/diagnostics/multifenetre/nvenc.rs agent/src/diagnostics/multifenetre.rs \
        docs/superpowers/plans/journaux-mesures-prealables/nvenc-separe.log
git commit -m "mesure(nvenc): plafond d'encodeurs sur périphériques D3D11 séparés

Seconde des deux mesures bloquantes. La sonde n'avait mesuré que le
périphérique unique et partagé, et ne pouvait donc pas dire si le refus de
la 9e instance venait de l'encodeur matériel ou du partage lui-même.

Le mode « partage » reste disponible comme témoin, pour que la mesure de la
sonde reste reproductible.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 10: Document de résultats et mise à jour des documents qui concluaient

**Files:**
- Create: `docs/superpowers/plans/2026-07-31-mesures-prealables-chantier-d-resultats.md`
- Modify: `docs/superpowers/specs/2026-07-28-support-jeux-design.md` (§5, chantier D)
- Modify: `CLAUDE.md`

**Interfaces:**
- Consumes: les cinq journaux de `journaux-mesures-prealables/` et `canal-de-controle.md`.
- Produces: le chantier D spécifiable.

- [ ] **Step 1: Rédiger le document de résultats**

Structure, calquée sur `2026-07-30-sonde-capture-multifenetre-resultats.md` :

1. **Ce qui est acquis** — les quatre chiffres, chacun avec son fichier de journal nommé.
2. **Ce qui reste ouvert** — tout ce qui a été borné ou non atteint.
3. **Mesure par mesure** — le relevé, puis ce qu'il autorise à conclure, **et rien de plus**. La sonde a passé onze rondes de revue à corriger des rapports qui affirmaient au-delà de leur relevé : c'est le coût principal d'un chantier de mesure.
4. **Recommandation pour le chantier D** — quelle voie de capture, avec son prix.
5. **Pièges rencontrés.**

Règle de rédaction : **aucun chiffre sans son fichier de journal**. C'est la seule faiblesse laissée par la sonde, et elle portait justement sur la voie recommandée.

- [ ] **Step 2: Retirer la mention « deux mesures bloquantes »**

Dans `docs/superpowers/specs/2026-07-28-support-jeux-design.md`, la puce **Capture** du chantier D (~ligne 408) se termine par :

```
  **Deux mesures sont bloquantes avant de spécifier ce chantier** : le plafond
  de sorties virtuelles simultanées (deux appareils clients appariés suffiraient)
  et le plafond d'encodage sur périphériques D3D11 séparés.
```

La remplacer par le renvoi aux résultats obtenus, et mettre à jour la puce **Budget encodeurs** avec le chiffre de la mesure ②.

- [ ] **Step 3: Mettre à jour `CLAUDE.md`**

Deux ajouts, un retrait :

- Une section « Mesures préalables au chantier D » à la suite de « Sonde de capture multi-fenêtres », avec les quatre chiffres et les pièges neufs.
- La liste des variables d'environnement du banc, complétée de `MULTIFENETRE_VDD`, `MULTIFENETRE_VDD_PURGE`, `MULTIFENETRE_VDD_CAPTURE`, `MULTIFENETRE_SORTIE`, et du fait que `MULTIFENETRE_NVENC` prend désormais `partage` ou `separe`.
- Dans la section de la sonde, la phrase sur les encodages mixtes des journaux gagne une précision : les journaux de **ce** bloc sont en UTF-8, la conversion `iconv` ne concerne que ceux de la sonde.

- [ ] **Step 4: Vérifier la règle des 500 lignes**

```bash
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
```

Attendu : uniquement les trois fichiers de dette déjà connus (`encode.rs`, `windows_source.rs`, `wasapi.rs`). Si `voies.rs`, `banc.rs` ou `moniteurs.rs` y figurent, extraire avant de committer.

- [ ] **Step 5: Vérification finale**

```bash
cargo test -p agent 2>&1 | tail -3
ls docs/superpowers/plans/journaux-mesures-prealables/
file docs/superpowers/plans/journaux-mesures-prealables/*.log
```

Attendu : tous les tests passent ; cinq journaux présents ; tous en `UTF-8 Unicode text`.

- [ ] **Step 6: Commit**

```bash
git add docs/ CLAUDE.md
git commit -m "docs(mesures): résultats du bloc, le chantier D devient spécifiable

Les quatre inconnues laissées par la sonde de capture sont levées, chacune
avec son journal versé — la faiblesse que la sonde avait laissée sur son
propre chiffre fondateur.

La spec du support jeux perd sa mention « deux mesures bloquantes ».

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```
