# N duplications DXGI de front sur N sorties virtuelles — plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Mesurer si N sorties virtuelles portant chacune une fenêtre, une duplication DXGI et un encodeur H.264 tiennent 60 i/s par fenêtre jusqu'à N=8 — l'arrangement que la voie recommandée du chantier D propose réellement et que rien n'a exercé.

**Architecture:** Le défaut de libération des encodeurs est diagnostiqué et corrigé d'abord (c'est un défaut du chemin de production du chantier D, pas du banc). La métrologie commune est ensuite extraite de `banc.rs` vers `compteurs.rs`, ce qui permet à un pilote neuf `paralleles.rs` de porter le protocole multi-sorties sans dupliquer la boucle de passes ni faire cohabiter deux protocoles dans le même fichier. La logique pure et testable sans Windows (rotation du contrôle, conversion des places par sortie) vit hors `#[cfg(windows)]`.

**Tech Stack:** Rust, `windows` crate (DXGI Desktop Duplication, Direct3D 11, Media Foundation), pilote SudoVDA piloté par IOCTL, `tracing` pour les journaux, `cargo test` sur l'hôte Linux pour la logique pure, compilation et exécution sur la VM Windows via `scripts/build-agent.sh` et `scripts/run-agent.sh`.

## Global Constraints

- **Spec de référence** : `docs/superpowers/specs/2026-07-31-duplications-paralleles-design.md`. Aucune décision n'est prise contre elle sans la modifier d'abord.
- **500 lignes maximum par fichier de code source.** Aucun nouveau fichier ne naît au-dessus ; aucun fichier déjà au-dessus ne grossit.
- **Un processus par rang de mesure.** Ces API échouent par plantage du processus, pas par code d'erreur.
- **Aucune trace par trame.** Compteurs agrégés, journalisés à la seconde. Une trace par paquet a détruit une session au chantier NAT.
- **Comparer des ensembles de noms, jamais des nombres** pour les sorties DXGI : Apollo agit sur le même vivier, et une addition externe compense exactement un retrait.
- **Ne jamais activer une sonde sur la seule présence de sa variable** : `sonde_demandee` compare à `"0"` (`diagnostics/multifenetre.rs:43`).
- **Ne pas se fier au texte d'un HRESULT pour désigner un appel.** Chaque appel du chemin de construction porte son propre `.context()`.
- **Résolution des sorties virtuelles : 1280×720@60** — la constante `montee::RESOLUTION`, déjà en place.
- **Rangs mesurés : N = 1, 2, 4, 8.** Critère de réception : à N=8, ≥ 60 i/s par fenêtre en capture+encodage et zéro verdict faux.
- **Journaux en UTF-8 par les deux réglages PowerShell** (`StreamWriter` pour l'écriture, `[Console]::OutputEncoding` pour la lecture) — déjà posés dans `scripts/run-agent.sh`, ne pas les défaire.
- **La VM Windows n'est pas démarrée automatiquement.** Avant toute tâche qui compile ou mesure : `virsh list --all`, puis `virsh start Windows`, puis attendre WinRM **et** l'accès réel à `/media/vm` (voir `CLAUDE.md`, « Cycle de vie de la VM Windows »).
- **Charger l'environnement** avant tout script : `set -a && source .env && set +a`.

---

## File Structure

| Fichier | Responsabilité |
| --- | --- |
| `agent/src/mire.rs` (modifié) | Ajoute `voie_controlee(tour, nombre)` — quelle voie est contrôlée à un tour donné. Logique pure, hors `#[cfg(windows)]`, testée sur l'hôte. |
| `agent/src/moniteurs_virtuels.rs` (modifié) | Ajoute `places_texture_par_sortie(sorties, textures)` — la place plein cadre de chaque sortie, convertie dans le repère de SA texture avec SON facteur d'échelle. Logique pure, testée sur l'hôte. |
| `agent/src/diagnostics/multifenetre/compteurs.rs` (créé) | La métrologie extraite de `banc.rs` : `Verdicts`, `Compteurs`, `passe_temoin`, `journaliser`, `lire_verdict`. Ce qui ne dépend pas du protocole. |
| `agent/src/diagnostics/multifenetre/banc.rs` (modifié) | Conserve le seul protocole mono-sortie à recouvrement. Consomme `compteurs.rs`. Redescend sous 300 lignes. |
| `agent/src/diagnostics/multifenetre/paralleles.rs` (créé) | Le pilote multi-sorties : N sorties virtuelles, N mires, N duplications, N encodeurs, rotation du contrôle, ping du chien de garde. |
| `agent/src/diagnostics/multifenetre/voies.rs` (modifié) | Expose `creer_device()` pour que `paralleles.rs` donne un périphérique D3D11 aux mires sans passer par une `DesktopCapture` provisoire (qui consommerait une duplication). |
| `agent/src/diagnostics/multifenetre.rs` (modifié) | Aiguillage de `MULTIFENETRE_VDD_PARALLELE`. |
| `agent/src/encode.rs` (modifié, tâche 2) | Correctif de la libération, selon ce que la tâche 1 désigne. |
| `scripts/run-agent.sh` (modifié) | Transmet `MULTIFENETRE_VDD_PARALLELE` à la VM. |
| `docs/superpowers/plans/2026-07-31-duplications-paralleles-resultats.md` (créé) | Document de résultats. |
| `docs/superpowers/plans/journaux-duplications-paralleles/` (créé) | Journaux bruts, un par rang. |

---

### Task 1: Instrumenter la libération et désigner le coupable

**Files:**
- Modify: `agent/src/diagnostics/multifenetre/banc.rs:255-369` (fin de `passe_capture`), `agent/src/diagnostics/multifenetre/banc.rs:154-172` (`executer`)
- Test: aucun test automatisé possible — le défaut est `#[cfg(windows)]` et se manifeste par mort du processus. La vérification est le journal de la VM.

**Interfaces:**
- Consumes: rien.
- Produces: un journal désignant nommément le relâchement qui tue le processus. La tâche 2 en dépend entièrement.

**Contexte.** Sur la voie `duplication`, la passe capture+encodage tue le processus **à la sortie de la boucle** — les deux exécutions du 31 juillet écrivent leur dixième ligne périodique à `debut + 10,00 s` et n'atteignent jamais `journaliser`. Ce qui court entre les deux est la destruction du `Vec<H264Encoder>` local à `passe_capture`. La voie `printwindow` survit au même traitement. Le défaut est donc dans le couple « voie duplication + encodeur H.264 ».

**Ne pas déduire, désigner.** La leçon du chantier précédent est explicite : instrumenter la sortie autant que l'entrée, deux traces encadrant chaque relâchement.

- [ ] **Step 1: Démarrer la VM et charger l'environnement**

```bash
virsh list --all
virsh start Windows 2>/dev/null || true
until timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null; do sleep 5; done
until ls /media/vm/dev >/dev/null 2>&1; do sleep 5; done
set -a && source .env && set +a
```

Attendu : `/media/vm/dev` listable. `mountpoint -q` ne suffit pas — l'entrée CIFS persiste VM éteinte.

- [ ] **Step 2: Rendre la libération des encodeurs explicite et tracée**

Dans `agent/src/diagnostics/multifenetre/banc.rs`, à la fin de `passe_capture`, juste avant `Ok(compteurs)` :

```rust
    // Libération EXPLICITE et tracée, une par une. Le défaut hérité tue le
    // processus ici — au relâchement, pas à la soumission — et un `Vec`
    // détruit implicitement ne dirait pas lequel de ses éléments a tué.
    // Ces traces sont rares par construction (une par encodeur, une fois par
    // passe) : elles ne violent pas la règle « aucune trace par trame ».
    if !encodeurs.is_empty() {
        tracing::info!(nombre = encodeurs.len(), "libération des encodeurs : début");
        for (id, encodeur) in encodeurs.drain(..).enumerate() {
            tracing::info!(id, "libération d'un encodeur : avant");
            drop(encodeur);
            tracing::info!(id, "libération d'un encodeur : après");
        }
        tracing::info!("libération des encodeurs : terminée");
    }
    Ok(compteurs)
```

- [ ] **Step 3: Tracer aussi le relâchement des voies, qui suit**

Dans `executer`, remplacer la fin (à partir de l'appel de la seconde `passe_capture`) par :

```rust
    let compteurs = passe_capture(&mut mires, &mut voies, &regions, true)?;
    journaliser("capture+encodage", nom_voie, nombre, &compteurs);
    // Le second suspect, après les encodeurs : la source de duplication que
    // toutes les voies partagent. Tracé séparément pour que le journal
    // distingue « mort aux encodeurs » de « mort à la duplication ».
    tracing::info!("libération des voies de capture : avant");
    drop(voies);
    tracing::info!("libération des voies de capture : après");
    Ok(())
```

- [ ] **Step 4: Compiler sur la VM**

```bash
scripts/build-agent.sh
```

Attendu : compilation réussie. En cas de `STATUS_STACK_BUFFER_OVERRUN (0xc0000409)`, c'est le quota WinRM — le script le relève lui-même, relancer.

- [ ] **Step 5: Reproduire sur le cas minimal**

Le cas minimal n'exige **aucune** sortie virtuelle : le défaut se reproduit sur le bureau physique.

```bash
MULTIFENETRE_BANC=duplication MULTIFENETRE_N=1 RUST_LOG=info scripts/run-agent.sh
sleep 60
cp /media/vm/dev/agent.log /tmp/user/0/claude-0/-home-mallanic-Projects-Guacamole/*/scratchpad/defaut-liberation.log
grep -n "libération\|passe terminée" /media/vm/dev/agent.log
```

Attendu : la dernière ligne `libération d'un encodeur : avant` **sans** son `après` correspondant désigne l'encodeur fautif ; ou bien tous les encodeurs passent et c'est `libération des voies de capture : avant` qui reste orpheline.

- [ ] **Step 6: Consigner le point désigné**

Écrire dans `/tmp/user/0/claude-0/-home-mallanic-Projects-Guacamole/*/scratchpad/defaut-liberation-constat.md` : la dernière ligne écrite, la première absente, et laquelle des trois hypothèses de la spec §5 elle accrédite. **Ne rien conclure au-delà** : le journal dit *où*, pas *pourquoi*.

- [ ] **Step 7: Commit**

```bash
git add agent/src/diagnostics/multifenetre/banc.rs
git commit -m "diag(banc): tracer chaque relâchement d'encodeur séparément

Le défaut hérité tue le processus à la libération, et un Vec détruit
implicitement ne dit pas lequel de ses éléments a tué.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 2: Corriger la libération

**Files:**
- Modify: `agent/src/encode.rs:1000-1019` (`Drop for H264Encoder`) et/ou `agent/src/encode.rs:251-313` (ordre des champs), selon ce que la tâche 1 désigne
- Test: la vérification est le journal de la VM (le chemin est `#[cfg(windows)]`, sans filet automatisé — dette assumée du projet)

**Interfaces:**
- Consumes: le constat de la tâche 1.
- Produces: un `H264Encoder` dont la destruction ne tue pas le processus quand il partage son périphérique D3D11 avec une duplication DXGI vivante. Toutes les tâches suivantes en dépendent : sans lui, aucune mesure d'encodage n'a de bilan.

**Les trois hypothèses, et le correctif de chacune.** Appliquer celui que la tâche 1 désigne, pas les trois.

**(a) L'encodeur survit au périphérique D3D11 que sa voie possède.** `H264Encoder` conserve `device: ID3D11Device` (`encode.rs:266`) et un `IMFDXGIDeviceManager` (`:254`) construits sur le périphérique de la voie. Si la voie — donc la `SourceDuplication`, donc `DesktopCapture` — est relâchée avant, les MFT tiennent des références sur un périphérique dont le propriétaire est parti. Correctif : dans `passe_capture`, libérer les encodeurs **avant** que quoi que ce soit d'autre ne bouge (déjà le cas depuis la tâche 1), et dans `executer` s'assurer que `voies` survit à `encodeurs` — ce que l'ordre actuel garantit déjà. Si le journal accrédite malgré tout cette piste, faire porter à `H264Encoder` un clone COM explicite de la duplication n'est **pas** la réponse : documenter l'ordre requis et le verrouiller par un commentaire de champ, comme `_media_foundation` l'est déjà (`encode.rs:306-312`).

**(b) Le pipeline Media Foundation n'est pas drainé avant relâchement.** `Drop for H264Encoder` envoie `MFT_MESSAGE_NOTIFY_END_OF_STREAM` puis `NOTIFY_END_STREAMING` aux deux MFT, mais **ne récupère jamais les sorties restantes**. Un MFT matériel asynchrone à qui l'on retire ses tampons alors qu'il détient des échantillons peut fauter dans le pilote. Correctif :

```rust
impl Drop for H264Encoder {
    fn drop(&mut self) {
        if self.skipped_busy > 0 {
            tracing::debug!(
                skipped_busy = self.skipped_busy,
                "images renoncées faute de confirmation du convertisseur (diagnostic)"
            );
        }
        // Drainer AVANT de relâcher : un MFT matériel asynchrone à qui l'on
        // retire ses tampons alors qu'il détient encore des échantillons
        // faute dans le pilote, et la faute emporte le processus — elle ne
        // ressort pas en HRESULT. Les échantillons drainés sont jetés : on
        // ferme, on ne transporte plus.
        unsafe {
            let _ = self.converter.ProcessMessage(MFT_MESSAGE_COMMAND_DRAIN, 0);
            let _ = self.transform.ProcessMessage(MFT_MESSAGE_COMMAND_DRAIN, 0);
        }
        self.pending_nv12.clear();
        self.pending_conversion_timestamps.clear();
        unsafe {
            let _ = self.converter.ProcessMessage(MFT_MESSAGE_NOTIFY_END_OF_STREAM, 0);
            let _ = self.converter.ProcessMessage(MFT_MESSAGE_NOTIFY_END_STREAMING, 0);
            let _ = self.transform.ProcessMessage(MFT_MESSAGE_NOTIFY_END_OF_STREAM, 0);
            let _ = self.transform.ProcessMessage(MFT_MESSAGE_NOTIFY_END_STREAMING, 0);
        }
        let _ = &self.device_manager;
    }
}
```

Ajouter `MFT_MESSAGE_COMMAND_DRAIN` aux imports `windows::Win32::Media::MediaFoundation` en tête de `encode.rs`.

**(c) `MFShutdown` court trop tôt parce qu'il court N fois.** `MediaFoundationSession` est un champ **par encodeur** (`encode.rs:312`) : N encodeurs appellent `MFStartup` N fois et `MFShutdown` N fois. Le compte est apparié, mais la destruction du premier encodeur décrémente pendant que N-1 encodeurs vivent encore. Correctif : ne pas toucher au comptage — le rendre **explicite et unique** au niveau du banc en construisant les encodeurs et en les détruisant tous ensemble, ce que la tâche 1 a déjà rendu vrai. Si le journal accrédite cette piste, le vrai correctif est produit et sort du périmètre de ce chantier : le noter dans le document de résultats comme **dette désignée**, et poser le contournement minimal (une `MediaFoundationSession` unique hissée hors des encodeurs) en le documentant comme tel.

- [ ] **Step 1: Appliquer le correctif désigné par la tâche 1**

Un seul des trois ci-dessus. Le commentaire du correctif cite la ligne de journal qui l'a désigné.

- [ ] **Step 2: Compiler**

```bash
scripts/build-agent.sh
```

- [ ] **Step 3: Vérifier sur le cas minimal**

```bash
MULTIFENETRE_BANC=duplication MULTIFENETRE_N=1 RUST_LOG=info scripts/run-agent.sh
sleep 60
grep -n "passe terminée\|libération" /media/vm/dev/agent.log
```

Attendu, et c'est le critère : la ligne `passe terminée passe="capture+encodage"` est présente, précédée de `libération des encodeurs : terminée` et suivie de `libération des voies de capture : après`. Le processus rend la main.

- [ ] **Step 4: Vérifier à N=2, où le défaut coûtait le plus**

```bash
MULTIFENETRE_BANC=duplication MULTIFENETRE_N=2 RUST_LOG=info scripts/run-agent.sh
sleep 60
grep -n "passe terminée" /media/vm/dev/agent.log
```

Attendu : la passe `capture` journalise son bilan. La passe `capture+encodage` est **sautée** — la porte éliminatoire coupe sous recouvrement sur cette voie (`verdicts_faux > 0`), c'est le comportement connu et attendu, pas une régression.

- [ ] **Step 5: Verser le journal**

```bash
mkdir -p docs/superpowers/plans/journaux-duplications-paralleles
cp /media/vm/dev/agent.log docs/superpowers/plans/journaux-duplications-paralleles/defaut-liberation-corrige.log
file docs/superpowers/plans/journaux-duplications-paralleles/*.log
```

Attendu : `UTF-8 Unicode text`, sans BOM. Si le fichier ressort en `ISO-8859` ou avec du mojibake sur les accents, les réglages d'encodage de `run-agent.sh` ont été défaits — les rétablir avant de continuer.

- [ ] **Step 6: Commit**

```bash
git add agent/src/encode.rs docs/superpowers/plans/journaux-duplications-paralleles/
git commit -m "fix(encode): <ce que le journal a désigné>

<Une phrase sur le mécanisme, sans dépasser ce que le journal montre.>

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 3: La logique pure — rotation du contrôle et places par sortie

**Files:**
- Modify: `agent/src/mire.rs` (ajout de `voie_controlee` et de ses tests)
- Modify: `agent/src/moniteurs_virtuels.rs` (ajout de `places_texture_par_sortie` et de ses tests)
- Test: les modules `#[cfg(test)]` de ces deux fichiers, exécutés sur l'hôte Linux

**Interfaces:**
- Consumes: `mire::MIRES_MAX`, `moniteurs_virtuels::{facteur_echelle, vers_texture}`, `geometry::Rect`.
- Produces:
  - `mire::voie_controlee(tour: u64, nombre: usize) -> Option<usize>`
  - `moniteurs_virtuels::places_texture_par_sortie(sorties: &[Rect], textures: &[(u32, u32)]) -> anyhow::Result<Vec<Rect>>`

  `paralleles.rs` (tâche 5) appelle exactement ces deux signatures.

**Pourquoi ces deux-là et pas ailleurs.** `diagnostics::multifenetre` est tout entier `#[cfg(windows)]` (`diagnostics.rs:20-21`) : rien de ce qui y vit ne peut être testé sur l'hôte. `mire.rs` et `moniteurs_virtuels.rs` sont hors `#[cfg(windows)]`, délibérément et pour cette raison exacte (voir le commentaire de tête de `moniteurs_virtuels.rs`).

- [ ] **Step 1: Écrire les tests de la rotation, qui échouent**

Dans `agent/src/mire.rs`, à la fin du module `#[cfg(test)] mod tests` (le créer s'il n'existe pas, avec `use super::*;`) :

```rust
    /// La propriété qui compte : sur k·N tours, chaque voie est contrôlée
    /// exactement k fois. Un contrôle qui favoriserait une voie laisserait
    /// les autres non couvertes, et c'est précisément l'appariement croisé
    /// entre sorties que ce montage doit détecter.
    #[test]
    fn la_rotation_controle_chaque_voie_le_meme_nombre_de_fois() {
        for nombre in 1..=8usize {
            let mut comptes = vec![0usize; nombre];
            for tour in 0..(nombre as u64 * 7) {
                let voie = voie_controlee(tour, nombre).expect("nombre non nul");
                comptes[voie] += 1;
            }
            assert!(
                comptes.iter().all(|compte| *compte == 7),
                "nombre = {nombre}, comptes = {comptes:?}"
            );
        }
    }

    #[test]
    fn la_rotation_ne_designe_jamais_une_voie_inexistante() {
        for nombre in 1..=8usize {
            for tour in 0..100u64 {
                let voie = voie_controlee(tour, nombre).expect("nombre non nul");
                assert!(voie < nombre, "voie {voie} hors des {nombre} voies");
            }
        }
    }

    /// Zéro voie n'est pas une erreur d'appelant à signaler par panique : le
    /// banc doit pouvoir demander sans savoir, et ne rien contrôler.
    #[test]
    fn sans_voie_il_n_y_a_rien_a_controler() {
        assert_eq!(voie_controlee(0, 0), None);
        assert_eq!(voie_controlee(42, 0), None);
    }
```

- [ ] **Step 2: Lancer les tests pour les voir échouer**

```bash
cd agent && cargo test --lib mire:: 2>&1 | tail -20
```

Attendu : ÉCHEC de compilation, `cannot find function 'voie_controlee' in this scope`.

- [ ] **Step 3: Écrire l'implémentation minimale**

Dans `agent/src/mire.rs`, après `verdict` :

```rust
/// Quelle voie est contrôlée au tour `tour`, parmi `nombre` voies.
///
/// Le montage multi-sorties supprime le recouvrement — une fenêtre par
/// sortie, rien ne peut en cacher une autre — donc la porte éliminatoire du
/// banc mono-sortie n'a plus d'objet. Le risque devient l'appariement : que la
/// voie *i* capture en réalité la sortie *j*, ou du noir.
///
/// Contrôler les N voies à chaque tour le détecterait, mais ferait croître le
/// coût CPU du contrôle avec N : la cadence relevée à N=8 intégrerait huit
/// fois ce coût et ne serait comparable à rien — l'erreur déjà payée au
/// chantier précédent, où la portée de la lecture de pixel a changé en cours
/// de route. La rotation couvre toutes les voies pour **une** lecture par
/// tour, quel que soit N.
pub fn voie_controlee(tour: u64, nombre: usize) -> Option<usize> {
    if nombre == 0 {
        return None;
    }
    Some((tour % nombre as u64) as usize)
}
```

- [ ] **Step 4: Lancer les tests pour les voir passer**

```bash
cd agent && cargo test --lib mire:: 2>&1 | tail -20
```

Attendu : `test result: ok`, trois tests neufs passants.

- [ ] **Step 5: Écrire les tests des places par sortie, qui échouent**

Dans le `mod tests` de `agent/src/moniteurs_virtuels.rs` :

```rust
    /// Le piège que cette fonction existe pour éviter : appliquer à toutes les
    /// sorties le facteur d'échelle de la première. Deux sorties virtuelles
    /// peuvent porter deux DPI différents, et un recadrage calculé au mauvais
    /// facteur est décalé sans que rien ne le signale.
    #[test]
    fn chaque_sortie_est_convertie_avec_son_propre_facteur() {
        let sorties = vec![
            Rect { x: 0, y: 0, width: 1280, height: 720 },
            Rect { x: 1280, y: 0, width: 853, height: 480 },
        ];
        let textures = vec![(1280, 720), (1280, 720)];
        let places = places_texture_par_sortie(&sorties, &textures).unwrap();
        assert_eq!(places[0], Rect { x: 0, y: 0, width: 1280, height: 720 });
        // Facteur 1280/853 ≈ 1,5 : la seconde sortie couvre TOUTE sa texture.
        // C'est le test qui compte : avec le facteur de la sortie 0 (l'unité),
        // on obtiendrait 853×480 dans un coin d'une texture 1280×720.
        assert_eq!(places[1], Rect { x: 0, y: 0, width: 1280, height: 720 });
    }

    /// Chaque place est ramenée à l'origine de SA texture : c'est ce qui
    /// distingue N sorties de N tuiles sur une sortie.
    #[test]
    fn une_sortie_decalee_dans_le_bureau_virtuel_part_de_l_origine_de_sa_texture() {
        let sorties = vec![Rect { x: 3840, y: 200, width: 1280, height: 720 }];
        let places = places_texture_par_sortie(&sorties, &[(1280, 720)]).unwrap();
        assert_eq!(places[0], Rect { x: 0, y: 0, width: 1280, height: 720 });
    }

    #[test]
    fn un_desaccord_de_longueur_est_refuse() {
        let sorties = vec![Rect { x: 0, y: 0, width: 1280, height: 720 }];
        assert!(places_texture_par_sortie(&sorties, &[]).is_err());
        assert!(places_texture_par_sortie(&[], &[(1280, 720)]).is_err());
    }

    /// Une annonce dégénérée ne donne aucun facteur (`facteur_echelle` rend
    /// `None`) : le refus doit ressortir, pas un facteur unité silencieux qui
    /// décalerait tous les recadrages de cette sortie.
    #[test]
    fn une_sortie_degeneree_est_refusee_plutot_que_supposee_a_l_unite() {
        let sorties = vec![Rect { x: 0, y: 0, width: 0, height: 720 }];
        assert!(places_texture_par_sortie(&sorties, &[(1280, 720)]).is_err());
    }
```

- [ ] **Step 6: Lancer les tests pour les voir échouer**

```bash
cd agent && cargo test --lib moniteurs_virtuels:: 2>&1 | tail -20
```

Attendu : ÉCHEC de compilation, `cannot find function 'places_texture_par_sortie'`.

- [ ] **Step 7: Écrire l'implémentation**

Dans `agent/src/moniteurs_virtuels.rs`, après `vers_texture` :

```rust
/// La place plein cadre de chaque sortie, exprimée dans le repère de SA
/// texture.
///
/// Le montage « une fenêtre par sortie » du chantier D pose une fenêtre qui
/// couvre toute sa sortie ; la région à recadrer est donc toute la texture.
/// Le calcul n'en est pas trivial pour autant : chaque sortie porte son propre
/// facteur d'échelle DPI, et appliquer à toutes celui de la première décalerait
/// silencieusement les recadrages des autres. Le banc mono-sortie n'avait qu'un
/// facteur à connaître ; celui-ci en a N.
///
/// `sorties` porte les rectangles annoncés par DXGI
/// (`DXGI_OUTPUT_DESC::DesktopCoordinates`), `textures` les dimensions
/// réellement rendues par l'acquisition de chacune, dans le même ordre.
pub fn places_texture_par_sortie(
    sorties: &[Rect],
    textures: &[(u32, u32)],
) -> Result<Vec<Rect>> {
    anyhow::ensure!(
        sorties.len() == textures.len(),
        "{} sorties pour {} textures : l'appariement serait arbitraire",
        sorties.len(),
        textures.len()
    );
    sorties
        .iter()
        .zip(textures)
        .enumerate()
        .map(|(index, (sortie, texture))| {
            let facteur = facteur_echelle((sortie.width, sortie.height), *texture)
                .with_context(|| {
                    format!(
                        "sortie {index} annoncée {}x{} : dimension nulle, aucun facteur \
                         d'échelle n'a de sens",
                        sortie.width, sortie.height
                    )
                })?;
            Ok(vers_texture(*sortie, *sortie, facteur))
        })
        .collect()
}
```

- [ ] **Step 8: Lancer toute la suite**

```bash
cd agent && cargo test --lib 2>&1 | tail -20
```

Attendu : `test result: ok`, aucun test antérieur cassé.

- [ ] **Step 9: Commit**

```bash
git add agent/src/mire.rs agent/src/moniteurs_virtuels.rs
git commit -m "feat(mesure): rotation du contrôle et places de texture par sortie

La logique pure du montage multi-sorties, hors cfg(windows) pour être
testée : le contrôle tourne d'une voie par tour (coût constant, couverture
complète), et chaque sortie est convertie avec SON facteur d'échelle.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 4: Extraire la métrologie vers `compteurs.rs`

**Files:**
- Create: `agent/src/diagnostics/multifenetre/compteurs.rs`
- Modify: `agent/src/diagnostics/multifenetre/banc.rs` (retrait de ce qui part, adaptation des appels)
- Modify: `agent/src/diagnostics/multifenetre.rs` (déclaration du module)
- Modify: `agent/src/diagnostics/multifenetre/voies.rs` (expose `creer_device`)

**Interfaces:**
- Consumes: `mire::{verdict, Verdict}`, `crate::diagnostics::pixels::read_pixel`, `super::mires::Mires`, `super::voies::VoieDeCapture`.
- Produces, tous en `pub(super)` :
  - `struct Verdicts { justes: u64, voisines: u64, noires: u64, inconnues: u64 }` avec `compter(&mut self, mire::Verdict)` et `faux(&self) -> u64`
  - `struct Compteurs { images: Vec<u64>, unites: Vec<u64>, avant_recouvrement: Verdicts, apres_recouvrement: Verdicts }` avec `nouveaux(nombre: usize) -> Self`
  - `fn passe_temoin(mires: &mut Mires) -> Result<()>`
  - `fn journaliser(passe: &str, voie: &str, nombre: u8, compteurs: &Compteurs)`
  - `fn lire_verdict(voie: &mut dyn VoieDeCapture, image: &CapturedFrame, attendu: u8) -> Result<mire::Verdict>`
  - `const DUREE_PASSE: Duration`, `const PERIODE_JOURNAL: Duration`
  - `voies::creer_device() -> Result<(ID3D11Device, ID3D11DeviceContext)>`

  `paralleles.rs` (tâche 5) consomme exactement ces noms.

**Remaniement pur.** Aucun comportement ne change. Le contrôle est que le banc mono-sortie rend les mêmes chiffres qu'avant.

- [ ] **Step 1: Créer `compteurs.rs` avec ce qui migre**

Créer `agent/src/diagnostics/multifenetre/compteurs.rs` avec en tête :

```rust
//! La métrologie du banc, partagée par ses deux protocoles.
//!
//! Extraite de `banc.rs` au moment où un second protocole est apparu (le
//! montage multi-sorties de `paralleles.rs`). Le partage n'est pas un confort
//! d'écriture : c'est ce qui rend les chiffres des deux bancs comparables. Un
//! second banc qui aurait sa propre boucle de comptage divergerait, et l'on ne
//! saurait plus si un écart de cadence vient de la voie mesurée ou du banc qui
//! la mesure.
//!
//! Ce qui n'est PAS ici : la mise en scène du recouvrement et la porte
//! éliminatoire (propres au protocole mono-sortie, restées dans `banc.rs`), et
//! la rotation du contrôle (propre au protocole multi-sorties, dans
//! `paralleles.rs`).
```

Y déplacer, **inchangés**, depuis `banc.rs` : `DUREE_PASSE`, `PERIODE_JOURNAL`, `Verdicts` et son `impl`, `Compteurs`, `passe_temoin`, `journaliser`. Passer `Verdicts`, `Compteurs` et leurs champs en `pub(super)`. Ajouter à `Compteurs` :

```rust
impl Compteurs {
    pub(super) fn nouveaux(nombre: usize) -> Self {
        Self {
            images: vec![0; nombre],
            unites: vec![0; nombre],
            avant_recouvrement: Verdicts::default(),
            apres_recouvrement: Verdicts::default(),
        }
    }
}
```

`passe_temoin` et `journaliser` lisent `DUREE_PASSE` directement, comme aujourd'hui : les deux protocoles mesurent sur la même durée, et un paramètre que les deux appelants rempliraient toujours avec la même constante serait un réglage qui n'existe pas.

- [ ] **Step 2: Y déplacer aussi la lecture de verdict**

La lecture de pixel est identique dans les deux protocoles ; seul diffère *quelle* voie est lue. Ajouter à `compteurs.rs` :

```rust
/// Lit le centre de l'image et rend le verdict correspondant.
///
/// `attendu` est l'identité de la mire qui DOIT s'y trouver. La lecture se
/// fait sur le périphérique de la voie : une texture ne se lit pas depuis un
/// autre périphérique que le sien.
pub(super) fn lire_verdict(
    voie: &mut dyn VoieDeCapture,
    image: &CapturedFrame,
    attendu: u8,
) -> Result<mire::Verdict> {
    let appareil = voie.device();
    let (r, g, b, _a) = crate::diagnostics::pixels::read_pixel(
        &appareil,
        &image.texture,
        image.width,
        image.height,
        image.width / 2,
        image.height / 2,
    )?;
    Ok(mire::verdict(attendu, (r, g, b)))
}
```

- [ ] **Step 3: Déclarer le module et adapter `banc.rs`**

Dans `agent/src/diagnostics/multifenetre.rs`, ajouter `pub(super) mod compteurs;` en respectant l'ordre alphabétique (après `mod capture_virtuelle;`).

Dans `banc.rs` : retirer ce qui a migré, ajouter `use super::compteurs::{self, Compteurs, DUREE_PASSE, PERIODE_JOURNAL};`, remplacer la construction manuelle de `Compteurs` par `Compteurs::nouveaux(nombre)`, remplacer le bloc de lecture de pixel de `passe_capture` par un appel à `compteurs::lire_verdict(voie.as_mut(), &image, 0)`, et passer `DUREE_PASSE` aux deux fonctions déplacées.

- [ ] **Step 4: Exposer un constructeur de périphérique dans `voies.rs`**

`paralleles.rs` doit donner un périphérique D3D11 à `Mires::ouvrir` sans ouvrir de `DesktopCapture` provisoire — DXGI n'autorisant qu'une duplication par sortie, une capture provisoire ferait échouer la vraie en `0x80070057`. Extraire de `VoiePrintWindow::partagee` la création du périphérique :

```rust
/// Crée un périphérique D3D11 matériel avec le support BGRA.
///
/// `pub(super)` parce que `paralleles.rs` en a besoin pour ses mires : il ne
/// peut pas emprunter celui d'une `DesktopCapture` provisoire, DXGI
/// n'autorisant qu'UNE duplication par sortie — la provisoire ferait échouer
/// la vraie en 0x80070057.
pub(super) fn creer_device() -> Result<(ID3D11Device, ID3D11DeviceContext)> {
    // corps repris tel quel de `VoiePrintWindow::partagee`
}
```

Faire appeler `creer_device()` par `VoiePrintWindow::partagee`.

- [ ] **Step 5: Compiler et vérifier les tailles**

```bash
cd agent && cargo check 2>&1 | tail -20
wc -l src/diagnostics/multifenetre/banc.rs src/diagnostics/multifenetre/compteurs.rs
```

Attendu : compilation propre ; `banc.rs` sous 300 lignes, `compteurs.rs` sous 200.

- [ ] **Step 6: Vérifier que le banc rend les mêmes chiffres qu'avant**

```bash
scripts/build-agent.sh
MULTIFENETRE_BANC=duplication MULTIFENETRE_N=2 RUST_LOG=info scripts/run-agent.sh
sleep 60
grep -n "passe terminée\|verdict :" /media/vm/dev/agent.log
```

Attendu : la passe `capture` journalise des cadences du même ordre que celles connues (≈99 i/s par fenêtre à N=2 sur le bureau physique) et la porte éliminatoire coupe toujours sous recouvrement. Un remaniement qui changerait ces chiffres n'en serait pas un.

- [ ] **Step 7: Commit**

```bash
git add agent/src/diagnostics/multifenetre/
git commit -m "refactor(banc): extraire la métrologie commune aux deux protocoles

Ce qui ne dépend pas du protocole part dans compteurs.rs : c'est ce qui
rendra les chiffres du banc multi-sorties comparables à ceux du banc
mono-sortie. Aucun comportement ne change.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 5: Le pilote multi-sorties

**Files:**
- Create: `agent/src/diagnostics/multifenetre/paralleles.rs`
- Modify: `agent/src/diagnostics/multifenetre.rs` (déclaration et aiguillage)
- Modify: `scripts/run-agent.sh` (transmission de la variable)

**Interfaces:**
- Consumes: `compteurs::{Compteurs, lire_verdict, passe_temoin, journaliser, DUREE_PASSE, PERIODE_JOURNAL}`, `mire::voie_controlee`, `moniteurs_virtuels::{places_texture_par_sortie, Sorties}`, `montee::{relever_topologie, noms_attaches, attendre_en_pinguant, RESOLUTION, DELAI_TOPOLOGIE}`, `moniteurs::ouvrir_pilote`, `purge::rejouer_purge_due`, `voies::{creer_device, VoieDuplication, VoieDeCapture}`, `mires::Mires`, `capture::{DesktopCapture, enumerer_sorties}`, `encode::H264Encoder`.
- Produces: `pub(super) fn mesurer(nombre: u8) -> Result<()>`.

**Le squelette, dans l'ordre imposé par la spec §3.**

- [ ] **Step 1: Écrire l'en-tête et la séquence de création**

Créer `agent/src/diagnostics/multifenetre/paralleles.rs` :

```rust
//! N sorties virtuelles, une fenêtre et une duplication DXGI chacune.
//!
//! **C'est l'arrangement que la voie recommandée du chantier D propose
//! réellement**, et que rien n'avait exercé : le banc de la sonde et la mesure
//! ③ ont tous deux posé N fenêtres sur UNE sortie. DXGI n'autorisant qu'une
//! duplication par sortie, N duplications de front est une question ouverte,
//! pas un détail d'implémentation.
//!
//! Second écart avec tout ce qui précède : chaque sortie fait 1280×720, donc
//! **l'aire totale croît avec N**. Les cadences de la sonde étaient prises à
//! aire totale fixe (`disposition::tuiles` découpe un bureau), où le débit de
//! pixels est quasi constant par construction et où le nombre de fenêtres
//! n'est pas prouvé neutre en soi.
//!
//! Pas de recouvrement ici — une fenêtre par sortie, rien ne peut en cacher
//! une autre — donc pas de porte éliminatoire. Le risque est l'appariement :
//! que la voie *i* capture la sortie *j*, ou du noir. C'est ce que la rotation
//! du contrôle (`mire::voie_controlee`) détecte, pour une lecture par tour.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use super::compteurs::{self, Compteurs, DUREE_PASSE, PERIODE_JOURNAL};
use super::montee::{
    attendre_en_pinguant, noms_attaches, relever_topologie, DELAI_TOPOLOGIE, RESOLUTION,
};
use super::voies::{creer_device, VoieDeCapture, VoieDuplication};
use crate::capture::SortieDxgi;
use crate::geometry::Rect;
use crate::mire;

/// Cadence de ping du chien de garde du pilote pendant les passes.
///
/// Le banc mono-sortie ne pingue pas : il tenait trente secondes sur UNE
/// sortie, et `capture_virtuelle.rs` signale ce point comme un risque assumé,
/// rattrapé après coup par un contrôle de survie. Ici huit sorties sont
/// exposées, sous un chien de garde dont **l'unité reste inconnue — aucune
/// n'est exclue, pas même la seconde**. Une sortie retirée sous la mesure
/// ferait imputer à Windows un défaut du protocole.
const CADENCE_PING: Duration = Duration::from_secs(1);

pub(super) fn mesurer(nombre: u8) -> Result<()> {
    anyhow::ensure!(
        (1..=mire::MIRES_MAX).contains(&nombre),
        "MULTIFENETRE_VDD_PARALLELE doit valoir 1 à {}",
        mire::MIRES_MAX
    );

    let avant = relever_topologie("avant création")?;
    let noms_avant = noms_attaches(&avant);
    let connues: HashSet<String> =
        avant.iter().map(|sortie| sortie.nom_sortie.clone()).collect();

    let pilote = super::moniteurs::ouvrir_pilote()?;
    let (largeur, hauteur, hertz) = RESOLUTION;

    let issue = {
        let mut sorties = crate::moniteurs_virtuels::Sorties::nouvelles(&pilote);
        for rang in 1..=nombre {
            let id = sorties
                .creer(largeur, hauteur, hertz)
                .with_context(|| format!("création de la sortie virtuelle n°{rang}"))?;
            tracing::info!(rang, id, "sortie virtuelle créée");
        }
        attendre_en_pinguant(&pilote, DELAI_TOPOLOGIE)?;

        let apres = relever_topologie("après création")?;
        let virtuelles = designer_sorties_neuves(&apres, &connues, nombre)?;
        pilote.pinguer()?;
        let issue = executer_passes(&pilote, &virtuelles);
        constater_survie(&virtuelles);
        issue
    };

    let rejoues = super::purge::rejouer_purge_due(&pilote);
    if rejoues > 0 {
        tracing::info!(rejoues, "retraits dus rejoués avec succès après la garde");
    }

    std::thread::sleep(DELAI_TOPOLOGIE);
    let final_ = relever_topologie("après destruction")?;
    let noms_final = noms_attaches(&final_);
    if noms_final == noms_avant {
        tracing::info!(noms = ?noms_final, "état initial restauré — mêmes sorties, nommément");
    } else {
        tracing::error!(
            noms_avant = ?noms_avant,
            noms_apres = ?noms_final,
            "la topologie n'est PAS revenue à son état initial — purge requise"
        );
    }

    issue
}
```

- [ ] **Step 2: Écrire la désignation des sorties neuves**

```rust
/// Retrouve les `nombre` sorties que cette sonde vient de créer, par
/// DIFFÉRENCE D'ENSEMBLES DE NOMS.
///
/// Pas par index : DXGI renumérote ses sorties à chaque reconfiguration de
/// topologie. Pas par cardinal : Apollo pilote la configuration d'affichage de
/// cette VM et peut ajouter une sortie à tout instant — une addition externe
/// compenserait exactement un retrait, et un contrôle par nombre passerait
/// alors qu'une sortie a disparu. Le cardinal n'est éprouvé qu'APRÈS la
/// différence de noms, jamais à sa place.
fn designer_sorties_neuves(
    apres: &[SortieDxgi],
    connues: &HashSet<String>,
    nombre: u8,
) -> Result<Vec<SortieDxgi>> {
    let neuves: Vec<SortieDxgi> = apres
        .iter()
        .filter(|sortie| !connues.contains(&sortie.nom_sortie))
        .cloned()
        .collect();
    let noms: Vec<&str> = neuves.iter().map(|s| s.nom_sortie.as_str()).collect();
    anyhow::ensure!(
        neuves.len() == nombre as usize,
        "{} sorties DXGI neuves après création de {nombre} ({noms:?}) — \
         une addition ou un retrait externe rend la mesure inimputable",
        neuves.len()
    );
    for sortie in &neuves {
        tracing::info!(
            nom = %sortie.nom_sortie,
            adaptateur = %sortie.adaptateur,
            index_adaptateur = sortie.index_adaptateur,
            index_sortie = sortie.index_sortie,
            attachee = sortie.attachee_au_bureau,
            x = sortie.rect.x,
            y = sortie.rect.y,
            largeur_annoncee = sortie.rect.width,
            hauteur_annoncee = sortie.rect.height,
            "sortie virtuelle retenue"
        );
    }
    Ok(neuves)
}
```

- [ ] **Step 3: Écrire l'ouverture des N duplications — le cœur de la question posée**

```rust
/// Ouvre une duplication DXGI par sortie.
///
/// **Un échec ici est LE RÉSULTAT de ce chantier, pas une panne.** Si la Kᵉ
/// `DuplicateOutput` est refusée, le rang, le HRESULT nu et la sortie visée
/// sont journalisés, et l'erreur ressort telle quelle : c'est la réponse à la
/// question posée. Ne jamais l'avaler ni la retenter.
fn ouvrir_duplications(
    virtuelles: &[SortieDxgi],
    mires: &super::mires::Mires,
) -> Result<(Vec<Box<dyn VoieDeCapture>>, Vec<Rect>)> {
    let mut sources = Vec::new();
    let mut textures = Vec::new();
    for (rang, sortie) in virtuelles.iter().enumerate() {
        let designation = (sortie.index_adaptateur, sortie.index_sortie);
        match VoieDuplication::partagee_sur(Some(designation)) {
            Ok(source) => {
                let dimensions = source.borrow().dimensions_bureau();
                tracing::info!(
                    rang = rang + 1,
                    nom = %sortie.nom_sortie,
                    texture_largeur = dimensions.0,
                    texture_hauteur = dimensions.1,
                    "duplication ouverte"
                );
                textures.push(dimensions);
                sources.push(source);
            }
            Err(erreur) => {
                tracing::error!(
                    rang = rang + 1,
                    nom = %sortie.nom_sortie,
                    causes = %super::causes(erreur),
                    "duplication REFUSÉE — c'est le résultat de la mesure, pas une panne"
                );
                anyhow::bail!(
                    "{} duplications DXGI ouvertes de front, la {}ᵉ refusée",
                    rang,
                    rang + 1
                );
            }
        }
    }

    let rects: Vec<Rect> = virtuelles.iter().map(|sortie| sortie.rect).collect();
    let places = crate::moniteurs_virtuels::places_texture_par_sortie(&rects, &textures)?;

    let mut voies: Vec<Box<dyn VoieDeCapture>> = Vec::new();
    for (id, source) in sources.into_iter().enumerate() {
        let mut voie: Box<dyn VoieDeCapture> = Box::new(VoieDuplication::nouvelle(source));
        voie.ouvrir(mires.hwnd(id as u8)?, places[id])?;
        voies.push(voie);
    }
    Ok((voies, places))
}
```

`SortieDxgi` dérive déjà `Clone` (`capture.rs:329`), le `.cloned()` de `designer_sorties_neuves` compile donc tel quel. Ajouter en revanche à `SourceDuplication` (`voies.rs`) l'accès à ses dimensions, qui n'existe pas :

```rust
    /// Dimensions de la TEXTURE que rend l'acquisition de cette sortie — pas
    /// celles annoncées par DXGI, dont elles peuvent différer d'un facteur DPI.
    pub(super) fn dimensions_bureau(&self) -> (u32, u32) {
        (self.bureau.width, self.bureau.height)
    }
```

- [ ] **Step 4: Écrire les passes, avec ping et rotation**

```rust
fn passe_capture(
    pilote: &super::moniteurs::PiloteParIoctl,
    mires: &mut super::mires::Mires,
    voies: &mut [Box<dyn VoieDeCapture>],
    avec_encodage: bool,
) -> Result<Compteurs> {
    let nombre = voies.len();
    let mut compteurs = Compteurs::nouveaux(nombre);
    let mut encodeurs: Vec<crate::encode::H264Encoder> = Vec::new();
    if avec_encodage {
        for id in 0..nombre {
            let place = mires.place(id as u8)?;
            let appareil = voies[id].device();
            encodeurs.push(
                crate::encode::H264Encoder::new(
                    &appareil,
                    (place.width, place.height),
                    (place.width, place.height),
                    60,
                    8_000_000,
                )
                .with_context(|| format!("encodeur n°{}", id + 1))?,
            );
        }
    }

    let debut = Instant::now();
    let mut prochain_journal = debut + PERIODE_JOURNAL;
    let mut prochain_ping = debut + CADENCE_PING;
    let mut pts = vec![0u64; nombre];

    while debut.elapsed() < DUREE_PASSE {
        mires.peindre()?;
        mires.pomper();
        let tour = mires.trame();
        // La voie contrôlée à ce tour, et elle seule : une lecture par tour
        // quel que soit N (voir `mire::voie_controlee`).
        let controlee = mire::voie_controlee(tour, nombre);

        for (id, voie) in voies.iter_mut().enumerate() {
            let Some(image) = voie.prochaine_image(tour)? else {
                continue;
            };
            compteurs.images[id] += 1;

            if controlee == Some(id) {
                // L'identité ATTENDUE est celle de la mire posée sur CETTE
                // sortie : un verdict `Voisine(j)` dit que la voie i a capturé
                // la sortie j, l'appariement croisé que ce montage doit
                // détecter.
                let verdict = compteurs::lire_verdict(voie.as_mut(), &image, id as u8)?;
                compteurs.apres_recouvrement.compter(verdict);
            }

            if avec_encodage {
                encodeurs[id].submit(&image, pts[id])?;
                pts[id] += 90_000 / 60;
                while let Some(_unite) = encodeurs[id].poll_output()? {
                    compteurs.unites[id] += 1;
                }
            }
        }

        if Instant::now() >= prochain_ping {
            pilote.pinguer()?;
            prochain_ping += CADENCE_PING;
        }
        if Instant::now() >= prochain_journal {
            tracing::info!(
                images = ?compteurs.images,
                unites = ?compteurs.unites,
                verdicts = ?compteurs.apres_recouvrement,
                "banc parallèle en cours"
            );
            prochain_journal += PERIODE_JOURNAL;
        }
    }

    if !encodeurs.is_empty() {
        tracing::info!(nombre = encodeurs.len(), "libération des encodeurs : début");
        for (id, encodeur) in encodeurs.drain(..).enumerate() {
            tracing::info!(id, "libération d'un encodeur : avant");
            drop(encodeur);
            tracing::info!(id, "libération d'un encodeur : après");
        }
        tracing::info!("libération des encodeurs : terminée");
    }
    Ok(compteurs)
}
```

`Compteurs` porte deux jeux de verdicts (avant/après recouvrement) hérités du protocole mono-sortie ; ici seul `apres_recouvrement` est alimenté, et `journaliser` l'affiche sous son nom. Si cette asymétrie gêne à la lecture du journal, renommer les deux champs en `avant`/`pendant` **dans la même tâche**, en adaptant `banc.rs` — pas d'alias, pas de champ mort.

- [ ] **Step 5: Écrire `executer_passes` et `constater_survie`**

```rust
/// Les trois passes, dans l'ordre.
///
/// **Pas de porte éliminatoire entre les deux dernières**, contrairement au
/// banc mono-sortie : sans recouvrement, un verdict faux n'invalide pas la
/// mesure de cadence, il la qualifie. Les deux passes tournent toujours, et le
/// rapport lit les verdicts.
fn executer_passes(
    pilote: &super::moniteurs::PiloteParIoctl,
    virtuelles: &[SortieDxgi],
) -> Result<()> {
    // Le périphérique des mires ne vient PAS d'une `DesktopCapture`
    // provisoire : DXGI n'autorise qu'une duplication par sortie, et la
    // provisoire ferait échouer la vraie en 0x80070057.
    let (device, _contexte) = creer_device()?;
    // Les mires vivent en coordonnées du BUREAU VIRTUEL — les rectangles
    // annoncés par DXGI. Les voies recadrent en coordonnées de TEXTURE, que
    // `ouvrir_duplications` calcule. Les confondre décalerait tout d'un
    // facteur DPI.
    let places_bureau: Vec<Rect> = virtuelles.iter().map(|sortie| sortie.rect).collect();
    let mut mires = super::mires::Mires::ouvrir(&device, &places_bureau)?;

    compteurs::passe_temoin(&mut mires)?;

    let (mut voies, places_texture) = ouvrir_duplications(virtuelles, &mires)?;
    tracing::info!(
        nombre = voies.len(),
        ?places_bureau,
        ?places_texture,
        "les N duplications sont ouvertes de front"
    );

    let nombre = voies.len() as u8;
    let releve = passe_capture(pilote, &mut mires, &mut voies, false)?;
    compteurs::journaliser("capture", "duplication-parallele", nombre, &releve);

    let releve = passe_capture(pilote, &mut mires, &mut voies, true)?;
    compteurs::journaliser("capture+encodage", "duplication-parallele", nombre, &releve);

    // Second suspect du défaut hérité, après les encodeurs : les duplications.
    tracing::info!("libération des voies de capture : avant");
    drop(voies);
    tracing::info!("libération des voies de capture : après");
    Ok(())
}

/// Dit si les N sorties virtuelles sont encore là après le passage du banc.
///
/// N'échoue pas : la mesure est faite, la nier maintenant ne la rendrait pas
/// meilleure. Ce relevé sert à INTERPRÉTER les verdicts, pas à les remplacer —
/// une sortie retirée par le chien de garde en cours de route rendrait du noir,
/// et l'on imputerait à Windows un défaut du protocole de mesure.
fn constater_survie(virtuelles: &[SortieDxgi]) {
    let vivantes = match crate::capture::enumerer_sorties() {
        Ok(sorties) => sorties,
        Err(erreur) => {
            tracing::error!(
                causes = %super::causes(erreur),
                "topologie illisible après le banc — survie des sorties inconnue"
            );
            return;
        }
    };
    let presentes: HashSet<&str> =
        vivantes.iter().map(|sortie| sortie.nom_sortie.as_str()).collect();
    let disparues: Vec<&str> = virtuelles
        .iter()
        .map(|sortie| sortie.nom_sortie.as_str())
        .filter(|nom| !presentes.contains(nom))
        .collect();
    if disparues.is_empty() {
        tracing::info!(
            nombre = virtuelles.len(),
            "les N sorties virtuelles ont survécu au banc — les verdicts portent bien sur elles"
        );
    } else {
        tracing::error!(
            ?disparues,
            "des sorties virtuelles ont DISPARU pendant le banc — leurs verdicts ne sont pas \
             imputables à Windows, elles n'existaient plus"
        );
    }
}
```

- [ ] **Step 6: Aiguiller la variable**

Dans `agent/src/diagnostics/multifenetre.rs`, **après** le bloc `MULTIFENETRE_VDD_PURGE` (cette sonde crée des sorties, une purge demandée ne doit jamais être supplantée) et près de `MULTIFENETRE_VDD_CAPTURE` :

```rust
    // La mesure de ce chantier : N sorties virtuelles, une fenêtre et une
    // duplication DXGI chacune — l'arrangement que la voie recommandée
    // propose réellement. Elle crée des sorties, donc elle passe après
    // `MULTIFENETRE_VDD_PURGE`.
    if let Ok(texte) = std::env::var("MULTIFENETRE_VDD_PARALLELE") {
        let nombre: u8 = texte
            .parse()
            .context("MULTIFENETRE_VDD_PARALLELE doit être un entier (nombre de sorties)")?;
        paralleles::mesurer(nombre)?;
        return Ok(true);
    }
```

Et `pub(super) mod paralleles;` dans la liste des modules.

- [ ] **Step 7: Transmettre la variable à la VM**

Dans `scripts/run-agent.sh`, après la ligne `MULTIFENETRE_VDD_CAPTURE` :

```bash
${MULTIFENETRE_VDD_PARALLELE:+\$env:MULTIFENETRE_VDD_PARALLELE = '$MULTIFENETRE_VDD_PARALLELE'}
```

- [ ] **Step 8: Compiler et vérifier la taille**

```bash
cd agent && cargo check 2>&1 | tail -20
cargo clippy --all-targets 2>&1 | grep -E "^(warning|error)" | head -20
wc -l src/diagnostics/multifenetre/paralleles.rs
```

Attendu : compilation propre, pas d'avertissement neuf, fichier sous 500 lignes. S'il approche, séparer le pilote des sorties (création, désignation, survie) de la boucle de passes, comme la spec §6 le prévoit.

- [ ] **Step 9: Épreuve à N=1 avant toute mesure**

```bash
scripts/build-agent.sh
MULTIFENETRE_VDD_PARALLELE=1 RUST_LOG=info scripts/run-agent.sh
sleep 90
grep -n "sortie virtuelle\|duplication ouverte\|passe terminée\|état initial" /media/vm/dev/agent.log
```

Attendu : une sortie créée, une duplication ouverte, trois passes journalisées, `état initial restauré`. À N=1 ce montage doit retrouver l'ordre de grandeur de la mesure ③ (90,0 i/s par fenêtre) : un écart franc désignerait le banc, pas la voie.

- [ ] **Step 10: Contrôler depuis un processus neuf**

```bash
MULTIFENETRE_DXGI=1 RUST_LOG=info scripts/run-agent.sh
sleep 30
grep -n "sortie" /media/vm/dev/agent.log
```

Attendu : aucune sortie virtuelle résiduelle. Le processus mesureur est juge et partie ; ce relevé-ci est le contrôle qui vaut.

- [ ] **Step 11: Commit**

```bash
git add agent/src/diagnostics/multifenetre/ scripts/run-agent.sh
git commit -m "feat(mesure): banc N sorties virtuelles × 1 duplication DXGI

L'arrangement que la voie recommandée du chantier D propose réellement.
Un refus de DuplicateOutput y est le résultat, pas une panne.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 6: Prendre les quatre rangs

**Files:**
- Create: `docs/superpowers/plans/journaux-duplications-paralleles/paralleles-n{1,2,4,8}.log`

**Interfaces:**
- Consumes: `paralleles::mesurer` de la tâche 5.
- Produces: quatre journaux UTF-8, un par rang, versés au dépôt.

**Un processus par rang**, avec purge et contrôle neuf entre chacun.

- [ ] **Step 1: Vérifier l'état de départ, depuis un processus neuf**

```bash
set -a && source .env && set +a
MULTIFENETRE_VDD_PURGE=1 RUST_LOG=info scripts/run-agent.sh && sleep 30
MULTIFENETRE_DXGI=1 RUST_LOG=info scripts/run-agent.sh && sleep 30
grep -c "sortie" /media/vm/dev/agent.log
```

Attendu : aucune sortie virtuelle avant de commencer. Noter les **noms** des sorties présentes — c'est la référence de tous les contrôles qui suivent, et Apollo peut la changer sous nos pieds.

- [ ] **Step 2: Prendre les quatre rangs**

Pour chaque `N` dans 1, 2, 4, 8, **dans cet ordre** :

```bash
for N in 1 2 4 8; do
  MULTIFENETRE_VDD_PARALLELE=$N RUST_LOG=info scripts/run-agent.sh
  sleep 120
  cp /media/vm/dev/agent.log \
     docs/superpowers/plans/journaux-duplications-paralleles/paralleles-n$N.log
  MULTIFENETRE_VDD_PURGE=1 RUST_LOG=info scripts/run-agent.sh
  sleep 30
done
```

Attendu par rang : `N sorties virtuelles créées`, `N duplications ouvertes`, trois lignes `passe terminée`, `état initial restauré`. **Si une duplication est refusée**, le rang s'arrête là et c'est le résultat : ne pas retenter, ne pas contourner, verser le journal et passer à la rédaction.

- [ ] **Step 3: Contrôler les encodages, dont le plafond connu est 8**

À N=8, la passe capture+encodage construit 8 encodeurs — exactement le plafond mesuré au chantier précédent, refus au 9ᵉ sur `SetOutputType`. Si un encodeur est refusé ici alors que 8 passaient à 720p sur un périphérique unique, c'est un **fait neuf** : le relever tel quel, avec le rang et le contexte de l'appel, sans l'expliquer.

- [ ] **Step 4: Vérifier l'encodage des journaux**

```bash
file docs/superpowers/plans/journaux-duplications-paralleles/*.log
grep -c "libération" docs/superpowers/plans/journaux-duplications-paralleles/paralleles-n8.log
```

Attendu : `UTF-8 Unicode text` pour les quatre, accents intacts, `grep` sur un mot accentué non vide.

- [ ] **Step 5: Contrôle final depuis un processus neuf**

```bash
MULTIFENETRE_DXGI=1 RUST_LOG=info scripts/run-agent.sh && sleep 30
grep -n "sortie" /media/vm/dev/agent.log
```

Attendu : l'ensemble des noms est identique à celui du Step 1.

- [ ] **Step 6: Commit**

```bash
git add docs/superpowers/plans/journaux-duplications-paralleles/
git commit -m "mesure(paralleles): quatre rangs de N duplications DXGI de front

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 7: Document de résultats et mise à jour des documents qui concluaient

**Files:**
- Create: `docs/superpowers/plans/2026-07-31-duplications-paralleles-resultats.md`
- Modify: `CLAUDE.md` (section « Mesures préalables au chantier D », ajout d'une section pour ce bloc)
- Modify: `docs/superpowers/specs/2026-07-28-support-jeux-design.md` §5 D (la puce « Capture », qui déclare cette mesure due)

**Interfaces:**
- Consumes: les quatre journaux de la tâche 6.
- Produces: le verdict de la voie recommandée, sur lequel le cadrage du chantier D s'appuiera.

**La discipline d'énoncé est le livrable.** Sur les deux chantiers de mesure précédents, la quasi-totalité des rondes de correction ont porté sur des **rapports qui affirmaient au-delà de leur relevé**, jamais sur des bugs. Chaque chiffre du rapport cite le journal et la ligne d'où il vient.

- [ ] **Step 1: Écrire le tableau des cadences**

Un rang par ligne : N, cadence par fenêtre en capture, en capture+encodage, débit de pixels total (N × 1280 × 720 × cadence), unités encodées, verdicts par nature. Recoller la colonne « débit de pixels » aux quatre rangs de `duplication` de la sonde (208–258 MP/s à aire fixe) **en signalant que les deux séries ne mesurent pas la même chose** : celle-ci fait croître l'aire avec N, celle-là non.

- [ ] **Step 2: Prononcer le critère de réception**

Reçue si, à N=8 : ≥ 60 i/s par fenêtre en capture+encodage **et** zéro verdict faux. Écrire le verdict en une phrase, puis les chiffres qui le fondent. Si le critère n'est pas tenu, dire **de combien** et sur quel rang il décroche — un refus chiffré vaut mieux qu'un refus qualitatif.

- [ ] **Step 3: Écrire la section « Ce que cette mesure n'établit pas »**

Reprendre la spec §4 : rien au-delà de 8 sorties, rien d'autres résolutions ou débits, rien de la latence, rien du comportement quand Apollo consomme le même vivier, rien de la couche qui imposerait un plafond, rien de la mise en sommeil des fenêtres masquées. Y ajouter toute réserve née pendant l'exécution.

- [ ] **Step 4: Consigner le sort du défaut hérité**

Ce qui a été désigné par la tâche 1, ce qui a été corrigé par la tâche 2, et ce qui reste — notamment si l'hypothèse (c) s'est vérifiée, auquel cas la dette `MediaFoundationSession` par encodeur est à nommer explicitement comme portée au chantier D.

- [ ] **Step 5: Mettre à jour `CLAUDE.md`**

Ajouter une section datée après « Mesures préalables au chantier D », sur le modèle des précédentes : les chiffres, les pièges neufs, ce que la mesure ne dit pas, et la variable d'environnement `MULTIFENETRE_VDD_PARALLELE` dans le tableau existant. Corriger la phrase de la section « Mesures préalables » qui déclare cette mesure due.

- [ ] **Step 6: Mettre à jour la spec du chantier D**

Dans `docs/superpowers/specs/2026-07-28-support-jeux-design.md` §5 D, remplacer le paragraphe « Une mesure reste due avant de dimensionner la voie » par le résultat obtenu. Si la voie est reçue, le dire sans en élargir la portée ; si elle tombe, dire ce qui la remplace n'est **pas** tranché par cette mesure.

- [ ] **Step 7: Commit**

```bash
git add docs/ CLAUDE.md
git commit -m "docs(paralleles): résultats du bloc — <verdict en cinq mots>

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Ce que ce plan ne couvre pas

- **Le chantier D lui-même** — détection `SetWinEventHook`, filtrage Alt-Tab, une `RTCPeerConnection` par fenêtre, cycle de vie des fenêtres. Ce bloc mesure ; il ne construit pas.
- **La latence** — seule la cadence est relevée. Le pipeline de bout en bout n'est pas exercé.
- **La mise en sommeil des fenêtres masquées** — que détruire un encodeur libère la place reste une conjecture non éprouvée, et la séquence « créer 8 → en détruire 1 → tenter un 9ᵉ » n'est pas jouée ici.
- **Le repli `PrintWindow`** — mesuré et reculé au chantier précédent (8,8 i/s par fenêtre à N=8), il n'est pas rejoué.
