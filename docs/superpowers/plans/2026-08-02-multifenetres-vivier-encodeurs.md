# Sous-bloc D5 — le vivier d'encodeurs : plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Faire du vivier d'encodeurs une ressource gérée — dix fenêtres ouvertes, au plus huit éveillées, les masquées relâchant leur encodeur et leur duplication — et tuer par là le défaut ouvert de D4 où `set_encode_size` est refusé à huit fenêtres.

**Architecture:** Le navigateur annonce visibilité et focus sur le data channel ; l'enfant relaie au capteur ; un vivier LRU **pur** vivant dans le capteur décide qui dort et qui veille ; le fil de fenêtre relâche ou reconstruit son `WindowsSource`. La sortie virtuelle n'est jamais touchée — c'est ce qui rend le sommeil sans effet sur les fenêtres voisines.

**Tech Stack:** Rust (windows-rs, Media Foundation, DXGI), TypeScript/Vite côté client, `serde_json` pour les deux protocoles, `vitest` côté client, tests unitaires Rust natifs côté agent.

**Spec de référence:** `docs/superpowers/specs/2026-08-02-multifenetres-vivier-encodeurs-design.md`

## Global Constraints

- **Aucun fichier source ne dépasse 500 lignes.** Un fichier déjà au-dessus ne grossit pas. Vérifier avec la commande du §8 de la spec après chaque tâche qui touche un fichier proche du plafond.
- **`agent/src/capteur/serveur.rs` est à 490 lignes (marge 10).** Ce plan est construit pour **ne pas le toucher** — le point d'accroche du sommeil est `capteur/fenetre.rs` (329 lignes). Si une tâche croit devoir modifier `serveur.rs`, elle doit s'arrêter et le signaler.
- **`agent/src/windows_source.rs` est à 648 lignes, dette gelée.** Seule la tâche 10 y touche, pour quelques lignes.
- **Le code `#[cfg(windows)]` se vérifie sur l'hôte** : `cd agent && cargo check --target x86_64-pc-windows-gnu`. Obligatoire avant toute compilation distante. Ne couvre pas l'édition de liens.
- **La logique décisionnelle ne vit jamais sous `#[cfg(windows)]`.** Patron établi par D4 (`capteur/protocole.rs`, `capteur/distante.rs`) : ce qui décide se teste sur l'hôte.
- **`CONTROL_VERSION` vaut 3** (`proto/src/control.rs:14`, `proto/ts/control.ts:8`). Ajouter des variantes ne le change pas ; les deux côtés doivent rester d'accord.
- **Jamais de trace par image ni par paquet.** Le dépôt a déjà perdu une session entière à cela.
- **Nommer les fichiers dans `git add`, jamais `git add -A`** : l'arbre est partagé avec d'autres tâches.
- **Commits en français, sans accent dans le sujet** (convention observable dans `git log`), terminés par `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`.

---

## Structure des fichiers

**Créés :**

| Fichier | Responsabilité |
| --- | --- |
| `agent/src/capteur/vivier.rs` | Le LRU pur : qui dort, qui veille, l'hystérésis. Aucun `cfg`, aucun objet COM, entièrement testé |
| `agent/src/capteur/sommeil.rs` | Le registre global : un `Vivier` partagé, un `Sender<Ordre>` par session, le fil de ré-arbitrage périodique |
| `agent/src/diagnostics/multifenetre/recyclage.rs` | La mesure pivot : créer 8, en détruire 1, en créer 1, en boucle |
| `client/src/visibilite.ts` | Écoute `visibilitychange`/`focus`/`blur`, émet sur le data channel |
| `client/src/visibilite.test.ts` | Ses tests |

**Modifiés :**

| Fichier | Nature de la modification |
| --- | --- |
| `proto/src/control.rs` | `ClientControl::Visibility`, `AgentControl::Asleep` |
| `proto/ts/control.ts` | Les mêmes, côté TypeScript, plus `encodeVisibility` |
| `agent/src/capteur/protocole.rs` | `VersCapteur::Visibilite`, `DepuisCapteur::Sommeil` |
| `agent/src/capteur.rs` | Déclaration des deux modules neufs |
| `agent/src/capteur/fenetre.rs` | `Option<WindowsSource>`, sondage des ordres, dormir/réveiller |
| `agent/src/capteur/distante.rs` | `Recu::Sommeil`, `set_awake`, `sommeil_a_annoncer` |
| `agent/src/capteur/distante/tests.rs` | Tests des deux ci-dessus |
| `agent/src/source.rs` | `VideoSource::set_awake`, par défaut inerte |
| `agent/src/transport/evenements.rs` | `pending_visibility` |
| `agent/src/transport/tick.rs` | Branche `a0quater` : appliquer la visibilité, annoncer le sommeil |
| `agent/src/superviseur/boucle.rs` | `CAPACITE` : 8 → 10 |
| `agent/src/windows_source.rs` | `set_encode_size` détruit avant de construire |
| `agent/src/diagnostics/multifenetre.rs` | Aiguillage du mode `recyclage` |
| `client/src/main.ts` | Câblage de `visibilite.ts` et du message `asleep` |
| `scripts/run-agent.sh` | Transmission de `MULTIFENETRE_NVENC_CYCLES` |
| `CLAUDE.md` | Correction du chiffre de `distante.rs`, section D5 |

---

## Tâche 1 : le mode `recyclage` du banc

**Files:**
- Create: `agent/src/diagnostics/multifenetre/recyclage.rs`
- Modify: `agent/src/diagnostics/multifenetre/nvenc.rs` (rendre `peripherique_autonome` visible au module frère)
- Modify: `agent/src/diagnostics/multifenetre.rs:216` (aiguillage)
- Modify: `scripts/run-agent.sh:61` (transmission de la variable de cycles)

**Interfaces:**
- Consumes: `crate::encode::H264Encoder::new(&ID3D11Device, (u32,u32), (u32,u32), u32, u32) -> Result<H264Encoder>`, `super::causes(anyhow::Error) -> String`
- Produces: `pub(super) fn recyclage::mesurer(cycles: usize) -> anyhow::Result<()>`

Ce banc **n'a aucun test** : il est `#[cfg(windows)]` et ne s'exécute que sur la VM. Sa vérification est `cargo check --target x86_64-pc-windows-gnu`, puis son exécution réelle en tâche 2.

- [ ] **Step 1: Rendre `peripherique_autonome` accessible au module frère**

Dans `agent/src/diagnostics/multifenetre/nvenc.rs`, changer la signature :

```rust
// AVANT
fn peripherique_autonome() -> Result<(ID3D11Device, ID3D11DeviceContext)> {

// APRÈS
pub(super) fn peripherique_autonome() -> Result<(ID3D11Device, ID3D11DeviceContext)> {
```

Aucun autre changement dans ce fichier.

- [ ] **Step 2: Écrire le banc**

Créer `agent/src/diagnostics/multifenetre/recyclage.rs` :

```rust
//! La mesure pivot du sous-bloc D5 : **détruire un encodeur libère-t-il la
//! place ?**
//!
//! La question est ouverte depuis le 31 juillet 2026 et conditionne tout le
//! sous-bloc — le remède au défaut de D4 comme la mise en sommeil elle-même.
//! La séquence « créer 8 → en détruire 1 → tenter un 9ᵉ » n'avait jamais été
//! jouée.
//!
//! **Le montage reproduit l'arrangement de PRODUCTION** : un processus, un
//! périphérique D3D11 par encodeur (chaque `DesktopCapture` crée le sien, voir
//! `capture/ouverture.rs`). Ce n'est ni le mode `partage` ni le mode `separe`
//! de `nvenc.rs`, et c'est le seul montage dont la réponse engage le produit.
//!
//! **Le cycle répété est le cœur de la mesure, pas un supplément.** Un seul
//! recyclage ne distingue pas un plafond de CONCURRENCE (8 vivants à la fois)
//! d'un plafond de CRÉATIONS CUMULÉES avec du mou : le premier cycle passerait
//! dans les deux cas. Le vivier de D5 recycle des encodeurs par construction —
//! c'est précisément lui qui déclencherait la seconde panne.

#![cfg(windows)]

use anyhow::Result;

use super::nvenc::peripherique_autonome;
use crate::encode::H264Encoder;

/// Les paramètres exacts de la seconde recette de D4, pour que le chiffre soit
/// opposable au sien.
const LARGEUR: u32 = 1280;
const HAUTEUR: u32 = 720;
const FPS: u32 = 60;
const DEBIT: u32 = 8_000_000;

/// On cesse de chercher au-delà : le plafond attendu est 8.
const PLAFOND_RECHERCHE: usize = 16;

/// Un encodeur et le périphérique qui le porte. Le périphérique DOIT vivre
/// aussi longtemps que l'encodeur ; les relâcher séparément ferait mesurer
/// autre chose que ce qu'on croit.
struct Instance {
    _peripherique: windows::Win32::Graphics::Direct3D11::ID3D11Device,
    _contexte: windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext,
    encodeur: H264Encoder,
}

/// Construit une instance complète, ou rend l'erreur du refus.
fn construire() -> Result<Instance> {
    let (peripherique, contexte) = peripherique_autonome()?;
    let encodeur = H264Encoder::new(
        &peripherique,
        (LARGEUR, HAUTEUR),
        (LARGEUR, HAUTEUR),
        FPS,
        DEBIT,
    )?;
    Ok(Instance { _peripherique: peripherique, _contexte: contexte, encodeur })
}

pub(super) fn mesurer(cycles: usize) -> Result<()> {
    tracing::info!(cycles, largeur = LARGEUR, hauteur = HAUTEUR, fps = FPS, debit = DEBIT,
        "mesure pivot D5 : recyclage d'encodeurs, un périphérique D3D11 par encodeur");

    // ── Phase 1 : monter jusqu'au refus, et le NOMMER.
    //
    // Sans ce témoin, rien de ce qui suit ne prouve quoi que ce soit : un 9ᵉ
    // qui réussit après une destruction ne dit rien si le 9ᵉ réussissait déjà
    // avant.
    let mut vivants: Vec<Instance> = Vec::new();
    let mut plafond = 0usize;
    for rang in 1..=PLAFOND_RECHERCHE {
        match construire() {
            Ok(instance) => {
                vivants.push(instance);
                tracing::info!(rang, vivants = vivants.len(), "phase 1 : encodeur créé");
            }
            Err(erreur) => {
                plafond = rang - 1;
                tracing::info!(
                    phase = 1,
                    plafond,
                    rang_refuse = rang,
                    causes = %super::causes(erreur),
                    "phase 1 : plafond atteint — création refusée"
                );
                break;
            }
        }
    }
    if plafond == 0 {
        tracing::warn!(
            plafond_recherche = PLAFOND_RECHERCHE,
            "phase 1 : aucun refus sous le plafond de recherche — la mesure pivot est SANS OBJET"
        );
        drop(vivants);
        return Ok(());
    }

    // ── Phase 2 : le cycle. Détruire un, en construire un, k fois.
    //
    // `vivants` contient exactement `plafond` instances. À chaque tour on en
    // retire une (destruction réelle : `drop` explicite, tracé de part et
    // d'autre pour qu'un gel de `Drop for H264Encoder` se lise comme tel) puis
    // on tente d'en construire une neuve.
    let mut reussis = 0usize;
    let mut premier_echec: Option<usize> = None;
    for cycle in 1..=cycles {
        let retiree = vivants.pop().expect("le vivier ne peut pas être vide ici");
        tracing::info!(cycle, restants = vivants.len(), "cycle : relâchement d'un encodeur");
        drop(retiree);
        tracing::info!(cycle, restants = vivants.len(), "cycle : relâchement terminé");

        match construire() {
            Ok(instance) => {
                vivants.push(instance);
                reussis += 1;
                tracing::info!(cycle, vivants = vivants.len(), "cycle : reconstruction RÉUSSIE");
            }
            Err(erreur) => {
                premier_echec = Some(cycle);
                tracing::info!(
                    cycle,
                    vivants = vivants.len(),
                    causes = %super::causes(erreur),
                    "cycle : reconstruction REFUSÉE"
                );
                break;
            }
        }
    }

    // ── Verdict, en une ligne lisible sans le reste du journal.
    match premier_echec {
        None => tracing::info!(
            plafond,
            cycles_demandes = cycles,
            cycles_reussis = reussis,
            verdict = "concurrence",
            "VERDICT : détruire un encodeur libère la place, et les cycles tiennent"
        ),
        Some(0) | Some(1) => tracing::info!(
            plafond,
            cycles_reussis = reussis,
            verdict = "aucune-liberation",
            "VERDICT : détruire un encodeur ne libère PAS la place"
        ),
        Some(k) => tracing::info!(
            plafond,
            cycles_reussis = reussis,
            premier_echec = k,
            verdict = "cumule",
            "VERDICT : plafond de créations CUMULÉES — le recyclage lâche au cycle {k}"
        ),
    }

    tracing::info!(vivants = vivants.len(), "relâchement final");
    drop(vivants);
    tracing::info!("relâchement final terminé — le processus a survécu");
    Ok(())
}
```

- [ ] **Step 3: Déclarer le module et l'aiguiller**

Dans `agent/src/diagnostics/multifenetre.rs`, ajouter la déclaration de module auprès des autres (`mod nvenc;` etc.) :

```rust
mod recyclage;
```

Puis, dans la fonction d'aiguillage, **juste avant** le bloc `MULTIFENETRE_NVENC` existant (ligne 216) :

```rust
    // Mesure pivot du sous-bloc D5 : détruire un encodeur libère-t-il la
    // place ? Placée AVANT `MULTIFENETRE_NVENC` : les deux variables ont un
    // préfixe commun, et l'ordre rend l'intention non ambiguë si les deux
    // sont posées par mégarde.
    if let Ok(texte) = std::env::var("MULTIFENETRE_NVENC_CYCLES") {
        let cycles: usize = texte
            .parse()
            .context("MULTIFENETRE_NVENC_CYCLES doit être un entier (nombre de recyclages)")?;
        recyclage::mesurer(cycles)?;
        return Ok(true);
    }
```

- [ ] **Step 4: Transmettre la variable depuis l'hôte**

Dans `scripts/run-agent.sh`, après la ligne 61 (`MULTIFENETRE_NVENC`) :

```sh
${MULTIFENETRE_NVENC_CYCLES:+\$env:MULTIFENETRE_NVENC_CYCLES = '$MULTIFENETRE_NVENC_CYCLES'}
```

**Pourquoi cette ligne compte** : `run-agent.sh` ne transmet pas les variables neuves, et le piège a été payé en D1 (`SUPERVISEUR`) puis en D2 (`MULTIFENETRE_REPRISE`). L'agent démarrerait sans la variable et **sans rien signaler**.

- [ ] **Step 5: Vérifier la compilation croisée**

Run: `cd agent && cargo check --target x86_64-pc-windows-gnu 2>&1 | tail -20`
Expected: aucune erreur. Des avertissements `dead_code` préexistants sont normaux ; **aucun ne doit viser `recyclage.rs`**.

- [ ] **Step 6: Commit**

```bash
git add agent/src/diagnostics/multifenetre/recyclage.rs \
        agent/src/diagnostics/multifenetre/nvenc.rs \
        agent/src/diagnostics/multifenetre.rs \
        scripts/run-agent.sh
git commit -m "mesure(d5): banc de recyclage d'encodeurs, avec son cycle repete

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 2 : exécuter la mesure pivot, et trancher

**Files:**
- Create: `docs/superpowers/plans/journaux-multifenetres-d5/recyclage-1.log` (et `-2`, `-3`)
- Create: `docs/superpowers/plans/2026-08-02-multifenetres-vivier-encodeurs-resultats.md` (section « mesure pivot » seulement)

**Interfaces:**
- Consumes: le banc de la tâche 1
- Produces: **le verdict qui gouverne les tâches 10 et 12** — `concurrence`, `cumule`, ou `aucune-liberation`

**Cette tâche est une porte.** Aucune tâche ultérieure ne dépend d'elle pour compiler, mais les tâches 10 et 12 dépendent de son verdict pour savoir quoi faire.

- [ ] **Step 1: S'assurer que la VM tourne**

```bash
virsh list --all
virsh start Windows   # si « fermé »
until timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null; do sleep 5; done
until ls /media/vm/dev >/dev/null 2>&1; do sleep 5; done
```

`mountpoint -q` ne suffit pas : `/media/vm` est un montage CIFS dont l'entrée persiste VM éteinte. Éprouver un **accès réel**.

- [ ] **Step 2: Compiler sur la VM**

```bash
set -a && source .env && set +a
scripts/build-agent.sh 2>&1 | tee /tmp/build-d5-t2.log
```

**Sourcer `.env` d'abord** : sans lui le script s'arrête EN SILENCE après « sources synchronisées », et le symptôme se lit comme une compilation réussie et muette.

- [ ] **Step 3: Relever la taille du binaire**

```bash
ls -l /media/vm/agent/target/release/agent.exe
```

**Une compilation de 0,13 s est un aveu** : après tout aller-retour de sources, `cargo` peut ne rien bâtir et le dire comme un succès. En cas de doute : `cargo clean --release -p agent` sur la VM, puis recompiler.

- [ ] **Step 4: Exécuter trois fois**

```bash
for i in 1 2 3; do
  MULTIFENETRE_NVENC_CYCLES=10 scripts/run-agent.sh
  # attendre la ligne « relâchement final terminé » puis copier le journal
  cp /media/vm/agent/agent.log docs/superpowers/plans/journaux-multifenetres-d5/recyclage-$i.log
done
```

**Trois exécutions au minimum, et le nombre écrit dans le rapport.** Un défaut intermittent qu'on croit déterministe se déclare corrigé à la première exécution qui passe.

Vérifier avant chaque exécution qu'aucun agent ne survit d'une précédente :

```bash
node scripts/winrm.js 'Get-Process agent -ErrorAction SilentlyContinue | Select-Object Id,StartTime | Format-List | Out-String'
```

- [ ] **Step 5: Extraire les trois verdicts**

```bash
grep -h "VERDICT" docs/superpowers/plans/journaux-multifenetres-d5/recyclage-*.log
grep -h "phase 1 : plafond atteint" docs/superpowers/plans/journaux-multifenetres-d5/recyclage-*.log
```

- [ ] **Step 6: Écrire la section « mesure pivot » du rapport**

Créer `docs/superpowers/plans/2026-08-02-multifenetres-vivier-encodeurs-resultats.md` avec, au minimum : le plafond relevé en phase 1 et le `HRESULT` du refus, le verdict de chacune des trois exécutions, **le nombre d'exécutions**, et la ligne de la table de décision du §3.4 de la spec qui s'applique.

**Ne rien affirmer au-delà du relevé.** C'est le mode de défaillance dominant de ce dépôt sur les chantiers de mesure — pas les bugs, les phrases.

- [ ] **Step 7: Commit**

```bash
git add docs/superpowers/plans/journaux-multifenetres-d5/ \
        docs/superpowers/plans/2026-08-02-multifenetres-vivier-encodeurs-resultats.md
git commit -m "mesure(d5): verdict de la mesure pivot sur le recyclage d'encodeurs

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 3 : le vivier pur

**Files:**
- Create: `agent/src/capteur/vivier.rs`
- Modify: `agent/src/capteur.rs` (déclaration du module)

**Interfaces:**
- Consumes: rien du projet — `std` seulement
- Produces:
  - `pub enum Raison { Masquee, Evincee }`
  - `pub enum Ordre { Dormir(Raison), Reveiller }`
  - `pub const PLAFOND_EVEIL: usize = 8;`
  - `pub const HYSTERESIS: Duration = Duration::from_secs(2);`
  - `Vivier::nouveau(plafond: usize, hysteresis: Duration) -> Vivier`
  - `Vivier::inscrire(&mut self, session: &str, maintenant: Instant) -> Vec<(String, Ordre)>`
  - `Vivier::retirer(&mut self, session: &str, maintenant: Instant) -> Vec<(String, Ordre)>`
  - `Vivier::signaler(&mut self, session: &str, visible: bool, focalisee: bool, maintenant: Instant) -> Vec<(String, Ordre)>`
  - `Vivier::rearbitrer(&mut self, maintenant: Instant) -> Vec<(String, Ordre)>`
  - `Vivier::eveillee(&self, session: &str) -> Option<bool>`

**C'est la pièce la plus coûteuse à se tromper, et c'est la seule entièrement testée.** Aucun `cfg`, aucun objet COM.

- [ ] **Step 1: Écrire les tests, d'abord**

Créer `agent/src/capteur/vivier.rs` avec **seulement** le module de tests et les déclarations minimales pour qu'il compile — non, plus simple et plus fidèle au TDD : écrire le fichier complet de tests ci-dessous, laisser l'implémentation vide, constater l'échec.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn t0() -> Instant {
        Instant::now()
    }

    /// Un vivier de 2 places, sans hystérésis, pour que les tests d'éviction
    /// n'aient pas à faire vieillir le temps.
    fn petit() -> Vivier {
        Vivier::nouveau(2, Duration::ZERO)
    }

    #[test]
    fn une_fenetre_inscrite_est_endormie_tant_qu_elle_n_est_pas_visible() {
        let mut v = petit();
        let ordres = v.inscrire("a", t0());
        assert!(ordres.is_empty(), "une inscription seule n'ordonne rien : {ordres:?}");
        assert_eq!(v.eveillee("a"), Some(false));
    }

    #[test]
    fn une_fenetre_visible_est_reveillee_quand_il_reste_de_la_place() {
        let mut v = petit();
        let t = t0();
        v.inscrire("a", t);
        let ordres = v.signaler("a", true, true, t);
        assert_eq!(ordres, vec![("a".to_string(), Ordre::Reveiller)]);
        assert_eq!(v.eveillee("a"), Some(true));
    }

    #[test]
    fn une_fenetre_masquee_s_endort_avec_la_raison_masquee() {
        let mut v = petit();
        let t = t0();
        v.inscrire("a", t);
        v.signaler("a", true, true, t);
        let ordres = v.signaler("a", false, false, t + Duration::from_secs(1));
        assert_eq!(ordres, vec![("a".to_string(), Ordre::Dormir(Raison::Masquee))]);
        assert_eq!(v.eveillee("a"), Some(false));
    }

    #[test]
    fn au_dela_du_plafond_la_moins_recemment_vue_est_evincee() {
        let mut v = petit();
        let t = t0();
        for (i, nom) in ["a", "b", "c"].iter().enumerate() {
            let quand = t + Duration::from_millis(i as u64 * 100);
            v.inscrire(nom, quand);
            v.signaler(nom, true, true, quand);
        }
        // « a » a été vue en premier, donc la plus anciennement vue des trois.
        assert_eq!(v.eveillee("a"), Some(false), "a devait être évincée");
        assert_eq!(v.eveillee("b"), Some(true));
        assert_eq!(v.eveillee("c"), Some(true));
    }

    #[test]
    fn l_eviction_porte_la_raison_evincee_et_non_masquee() {
        let mut v = petit();
        let t = t0();
        v.inscrire("a", t);
        v.signaler("a", true, true, t);
        v.inscrire("b", t + Duration::from_millis(100));
        v.signaler("b", true, true, t + Duration::from_millis(100));
        v.inscrire("c", t + Duration::from_millis(200));
        let ordres = v.signaler("c", true, true, t + Duration::from_millis(200));
        assert!(
            ordres.contains(&("a".to_string(), Ordre::Dormir(Raison::Evincee))),
            "l'éviction doit être motivée, pas confondue avec une mise en veille voulue : {ordres:?}"
        );
        assert!(ordres.contains(&("c".to_string(), Ordre::Reveiller)));
    }

    #[test]
    fn un_focus_rafraichit_la_recence_et_protege_de_l_eviction() {
        let mut v = petit();
        let t = t0();
        for (i, nom) in ["a", "b"].iter().enumerate() {
            let quand = t + Duration::from_millis(i as u64 * 100);
            v.inscrire(nom, quand);
            v.signaler(nom, true, true, quand);
        }
        // « a » reprend le focus : elle redevient la plus récemment vue.
        v.signaler("a", true, true, t + Duration::from_millis(500));
        v.inscrire("c", t + Duration::from_millis(600));
        v.signaler("c", true, true, t + Duration::from_millis(600));
        assert_eq!(v.eveillee("a"), Some(true), "a venait d'être focalisée");
        assert_eq!(v.eveillee("b"), Some(false), "b est la plus anciennement vue");
    }

    #[test]
    fn l_hysteresis_empeche_d_evincer_une_fenetre_tout_juste_reveillee() {
        let mut v = Vivier::nouveau(1, Duration::from_secs(2));
        let t = t0();
        v.inscrire("a", t);
        v.signaler("a", true, true, t);
        assert_eq!(v.eveillee("a"), Some(true));

        // « b » demande à veiller 500 ms plus tard : « a » est protégée.
        v.inscrire("b", t + Duration::from_millis(500));
        let ordres = v.signaler("b", true, true, t + Duration::from_millis(500));
        assert!(ordres.is_empty(), "rien ne doit bouger sous l'hystérésis : {ordres:?}");
        assert_eq!(v.eveillee("a"), Some(true));
        assert_eq!(v.eveillee("b"), Some(false));
    }

    #[test]
    fn passee_l_hysteresis_la_rearbitration_periodique_debloque_le_reveil() {
        let mut v = Vivier::nouveau(1, Duration::from_secs(2));
        let t = t0();
        v.inscrire("a", t);
        v.signaler("a", true, true, t);
        v.inscrire("b", t + Duration::from_millis(500));
        v.signaler("b", true, true, t + Duration::from_millis(500));

        // Sans ce ré-arbitrage, « b » attendrait un signal qui ne viendra
        // jamais : elle est déjà visible et déjà focalisée.
        let ordres = v.rearbitrer(t + Duration::from_secs(3));
        assert!(ordres.contains(&("a".to_string(), Ordre::Dormir(Raison::Evincee))));
        assert!(ordres.contains(&("b".to_string(), Ordre::Reveiller)));
    }

    #[test]
    fn une_fenetre_masquee_s_endort_meme_sous_l_hysteresis() {
        // Se masquer est un geste EXPLICITE de l'utilisateur : l'hystérésis
        // protège contre le battement d'éviction, jamais contre une volonté.
        let mut v = Vivier::nouveau(2, Duration::from_secs(10));
        let t = t0();
        v.inscrire("a", t);
        v.signaler("a", true, true, t);
        let ordres = v.signaler("a", false, false, t + Duration::from_millis(10));
        assert_eq!(ordres, vec![("a".to_string(), Ordre::Dormir(Raison::Masquee))]);
    }

    #[test]
    fn retirer_une_fenetre_eveillee_rend_sa_place_a_une_endormie() {
        let mut v = petit();
        let t = t0();
        for (i, nom) in ["a", "b", "c"].iter().enumerate() {
            let quand = t + Duration::from_millis(i as u64 * 100);
            v.inscrire(nom, quand);
            v.signaler(nom, true, true, quand);
        }
        assert_eq!(v.eveillee("a"), Some(false));
        let ordres = v.retirer("c", t + Duration::from_secs(1));
        assert_eq!(ordres, vec![("a".to_string(), Ordre::Reveiller)]);
        assert_eq!(v.eveillee("c"), None, "une session retirée n'a plus d'état");
    }

    #[test]
    fn un_signal_pour_une_session_inconnue_est_ignore_sans_paniquer() {
        let mut v = petit();
        let ordres = v.signaler("fantome", true, true, t0());
        assert!(ordres.is_empty());
        assert_eq!(v.eveillee("fantome"), None);
    }

    #[test]
    fn un_ordre_n_est_jamais_emis_deux_fois_pour_le_meme_etat() {
        let mut v = petit();
        let t = t0();
        v.inscrire("a", t);
        v.signaler("a", true, true, t);
        // Second signal identique : la fenêtre est déjà éveillée.
        let ordres = v.signaler("a", true, true, t + Duration::from_millis(50));
        assert!(ordres.is_empty(), "un état inchangé n'ordonne rien : {ordres:?}");
    }
}
```

- [ ] **Step 2: Vérifier que ça échoue**

Ajouter en tête du fichier les imports et un corps vide :

```rust
use std::collections::HashMap;
use std::time::{Duration, Instant};
```

Run: `cd agent && cargo test vivier 2>&1 | tail -20`
Expected: FAIL — `cannot find type Vivier in this scope`, `cannot find enum Ordre`, etc.

- [ ] **Step 3: Écrire l'implémentation**

Insérer **avant** le `#[cfg(test)] mod tests`, sous les `use` :

```rust
//! Le vivier d'encodeurs : qui dort, qui veille.
//!
//! **Pas de `#[cfg(windows)]`, aucun objet COM, aucun canal.** Ce module ne
//! fait que décider ; l'application des décisions vit dans `capteur/sommeil.rs`
//! et `capteur/fenetre.rs`. C'est le patron établi par D4 pour
//! `capteur/protocole.rs` et `capteur/distante.rs` : ce qui décide se teste sur
//! l'hôte, et c'est ici la pièce la plus coûteuse à se tromper.
//!
//! **Pourquoi un LRU et pas un « premier arrivé, premier servi ».** La fenêtre
//! au premier plan doit toujours gagner : c'est la main de l'utilisateur qui
//! arbitre, sans qu'il ait rien à régler.

/// Nombre d'encodeurs simultanément vivants que le capteur s'autorise.
///
/// **Relevé sur cette VM, pas une borne du système** : mesuré les 30 et
/// 31 juillet 2026 (la 9ᵉ création refusée au `SetOutputType` de la MFT NVIDIA,
/// `MF_E_UNSUPPORTED_D3D_TYPE`), inchangé que les encodeurs partagent un
/// périphérique D3D11 ou qu'ils en aient chacun un neuf. **La couche qui
/// l'impose n'est pas identifiée.**
pub const PLAFOND_EVEIL: usize = 8;

/// Temps minimal d'éveil avant qu'une fenêtre puisse être ÉVINCÉE.
///
/// ⚠️ **Valeur NON CALIBRÉE.** Elle borne le battement — dix fenêtres visibles
/// et un utilisateur qui passe de l'une à l'autre reconstruiraient sinon une
/// duplication DXGI et un encodeur par changement de focus. Le nombre
/// d'endormissements relevé à la recette est ce qui la jugera, pas une
/// intuition.
///
/// Elle ne protège PAS contre une mise en veille voulue : se masquer est un
/// geste explicite de l'utilisateur.
pub const HYSTERESIS: Duration = Duration::from_secs(2);

/// Pourquoi une fenêtre s'endort. Les deux cas ne se valent pas pour
/// l'utilisateur, et le client les affiche différemment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Raison {
    /// Il l'a voulu : la fenêtre est minimisée ou son onglet est caché.
    Masquee,
    /// Le vivier la lui a prise alors qu'il la regardait.
    Evincee,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ordre {
    Dormir(Raison),
    Reveiller,
}

struct Entree {
    visible: bool,
    /// Instant du dernier focus ou de la dernière remise en visibilité.
    ///
    /// **La visibilité seule ne suffirait pas à ordonner un LRU** : dix
    /// fenêtres toutes visibles ont exactement la même visibilité, et
    /// l'éviction serait alors arbitraire.
    dernier_vu: Instant,
    eveillee: bool,
    eveillee_depuis: Instant,
}

pub struct Vivier {
    plafond: usize,
    hysteresis: Duration,
    entrees: HashMap<String, Entree>,
}

impl Vivier {
    pub fn nouveau(plafond: usize, hysteresis: Duration) -> Vivier {
        Vivier { plafond, hysteresis, entrees: HashMap::new() }
    }

    pub fn inscrire(&mut self, session: &str, maintenant: Instant) -> Vec<(String, Ordre)> {
        // Une fenêtre naît ENDORMIE : le client annoncera sa visibilité, et
        // c'est elle qui la réveillera. Naître éveillée ferait dépasser le
        // plafond entre l'attache et le premier signal.
        self.entrees.insert(
            session.to_string(),
            Entree {
                visible: false,
                dernier_vu: maintenant,
                eveillee: false,
                eveillee_depuis: maintenant,
            },
        );
        self.arbitrer(maintenant)
    }

    pub fn retirer(&mut self, session: &str, maintenant: Instant) -> Vec<(String, Ordre)> {
        self.entrees.remove(session);
        self.arbitrer(maintenant)
    }

    pub fn signaler(
        &mut self,
        session: &str,
        visible: bool,
        focalisee: bool,
        maintenant: Instant,
    ) -> Vec<(String, Ordre)> {
        let Some(entree) = self.entrees.get_mut(session) else {
            // Un signal peut arriver d'un enfant dont la fenêtre vient d'être
            // retirée. Ignorer, jamais paniquer.
            return Vec::new();
        };
        // La récence se rafraîchit au focus ET au retour de visibilité : ce
        // sont les deux façons dont l'utilisateur dit « je regarde celle-ci ».
        if focalisee || (visible && !entree.visible) {
            entree.dernier_vu = maintenant;
        }
        entree.visible = visible;
        self.arbitrer(maintenant)
    }

    /// Ré-arbitrage périodique, appelé par le fil de `sommeil.rs`.
    ///
    /// **Indispensable, et pas un luxe** : sous hystérésis, une fenêtre qui
    /// demande à veiller peut être refusée. Elle est alors déjà visible et
    /// déjà focalisée — aucun signal ne viendra plus la débloquer, et elle
    /// dormirait pour toujours sans ce tour de roue.
    pub fn rearbitrer(&mut self, maintenant: Instant) -> Vec<(String, Ordre)> {
        self.arbitrer(maintenant)
    }

    pub fn eveillee(&self, session: &str) -> Option<bool> {
        self.entrees.get(session).map(|e| e.eveillee)
    }

    /// Le cœur : calcule l'ensemble cible des éveillées, et en déduit les
    /// transitions. **Idempotent** — appelé deux fois de suite sans changement
    /// d'état ni de temps, il ne rend rien la seconde fois.
    fn arbitrer(&mut self, maintenant: Instant) -> Vec<(String, Ordre)> {
        let mut ordres = Vec::new();

        // 1. Toute éveillée devenue invisible s'endort. Sans hystérésis : le
        //    masquage est explicite.
        let masquees: Vec<String> = self
            .entrees
            .iter()
            .filter(|(_, e)| e.eveillee && !e.visible)
            .map(|(nom, _)| nom.clone())
            .collect();
        for nom in masquees {
            if let Some(e) = self.entrees.get_mut(&nom) {
                e.eveillee = false;
            }
            ordres.push((nom, Ordre::Dormir(Raison::Masquee)));
        }

        // 2. Les épinglées : éveillées, encore visibles, et réveillées depuis
        //    moins que l'hystérésis. Elles occupent leur place quoi qu'il
        //    arrive.
        let epinglees: Vec<String> = self
            .entrees
            .iter()
            .filter(|(_, e)| {
                e.eveillee
                    && e.visible
                    && maintenant.saturating_duration_since(e.eveillee_depuis) < self.hysteresis
            })
            .map(|(nom, _)| nom.clone())
            .collect();

        // 3. Les candidates : toutes les visibles, de la plus récemment vue à
        //    la plus ancienne. Un ordre total est nécessaire pour que le
        //    résultat ne dépende pas du parcours d'une table de hachage : à
        //    récence égale, le nom départage.
        let mut candidates: Vec<(String, Instant)> = self
            .entrees
            .iter()
            .filter(|(_, e)| e.visible)
            .map(|(nom, e)| (nom.clone(), e.dernier_vu))
            .collect();
        candidates.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        // 4. L'ensemble cible : les épinglées d'abord, puis les candidates les
        //    plus récentes jusqu'à remplir le plafond.
        let mut cible: Vec<String> = epinglees.clone();
        for (nom, _) in candidates {
            if cible.len() >= self.plafond {
                break;
            }
            if !cible.contains(&nom) {
                cible.push(nom);
            }
        }

        // 5. Les transitions.
        let noms: Vec<String> = self.entrees.keys().cloned().collect();
        for nom in noms {
            let doit_veiller = cible.contains(&nom);
            let Some(e) = self.entrees.get_mut(&nom) else { continue };
            if doit_veiller && !e.eveillee {
                e.eveillee = true;
                e.eveillee_depuis = maintenant;
                ordres.push((nom, Ordre::Reveiller));
            } else if !doit_veiller && e.eveillee {
                e.eveillee = false;
                ordres.push((nom, Ordre::Dormir(Raison::Evincee)));
            }
        }

        ordres
    }
}
```

- [ ] **Step 4: Déclarer le module**

Dans `agent/src/capteur.rs`, auprès des autres modules **non gatés** :

```rust
pub mod vivier;
```

- [ ] **Step 5: Vérifier que tout passe**

Run: `cd agent && cargo test vivier 2>&1 | tail -20`
Expected: PASS — 12 tests.

- [ ] **Step 6: Commit**

```bash
git add agent/src/capteur/vivier.rs agent/src/capteur.rs
git commit -m "feat(d5): le vivier d'encodeurs, LRU pur et entierement teste

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 4 : les deux protocoles

**Files:**
- Modify: `proto/src/control.rs` (après la variante `Resize`, et dans `AgentControl`)
- Modify: `proto/ts/control.ts`
- Modify: `agent/src/capteur/protocole.rs`

**Interfaces:**
- Consumes: `CONTROL_VERSION: u8 = 3`, `verifie_version`
- Produces:
  - Rust : `ClientControl::Visibility { version, visible: bool, focused: bool }`, `AgentControl::Asleep { version, asleep: bool, reason: String }`, `AgentControl::asleep(asleep: bool, reason: &str) -> AgentControl`
  - TS : `VisibilityMessage`, `AsleepMessage`, `encodeVisibility(visible: boolean, focused: boolean): string`
  - Capteur : `VersCapteur::Visibilite { visible: bool, focalisee: bool }`, `DepuisCapteur::Sommeil { endormie: bool, raison: String }`

- [ ] **Step 1: Écrire les tests Rust**

Dans `proto/src/control.rs`, module `tests`, ajouter :

```rust
    #[test]
    fn une_visibilite_se_relit_telle_qu_ecrite() {
        let json = r#"{"type":"visibility","v":3,"visible":false,"focused":false}"#;
        let message: ClientControl = serde_json::from_str(json).expect("visibilité valide");
        assert_eq!(
            message,
            ClientControl::Visibility { version: 3, visible: false, focused: false }
        );
    }

    #[test]
    fn une_visibilite_de_mauvaise_version_est_rejetee() {
        let json = r#"{"type":"visibility","v":1,"visible":true,"focused":true}"#;
        assert!(serde_json::from_str::<ClientControl>(json).is_err());
    }

    #[test]
    fn un_sommeil_s_ecrit_avec_son_type_en_tete_et_sa_raison() {
        let json = serde_json::to_string(&AgentControl::asleep(true, "evincee"))
            .expect("sérialisation");
        assert!(json.starts_with(r#"{"type":"asleep""#), "obtenu : {json}");
        assert!(json.contains(r#""asleep":true"#), "obtenu : {json}");
        assert!(json.contains(r#""reason":"evincee""#), "obtenu : {json}");
    }
```

Dans `agent/src/capteur/protocole.rs`, module `tests`, ajouter :

```rust
    #[test]
    fn une_visibilite_traverse_le_canal_du_capteur() {
        let message = VersCapteur::Visibilite { visible: true, focalisee: false };
        let json = serde_json::to_string(&message).expect("sérialisation");
        let relu: VersCapteur = serde_json::from_str(&json).expect("désérialisation");
        assert_eq!(relu, message);
    }

    #[test]
    fn un_sommeil_traverse_le_canal_du_capteur() {
        let message = DepuisCapteur::Sommeil { endormie: true, raison: "masquee".into() };
        let json = serde_json::to_string(&message).expect("sérialisation");
        let relu: DepuisCapteur = serde_json::from_str(&json).expect("désérialisation");
        assert_eq!(relu, message);
    }
```

- [ ] **Step 2: Vérifier que ça échoue**

Run: `cd proto && cargo test control 2>&1 | tail -10 && cd ../agent && cargo test protocole 2>&1 | tail -10`
Expected: FAIL — variantes inconnues.

- [ ] **Step 3: Ajouter les variantes Rust**

Dans `proto/src/control.rs`, dans `enum ClientControl`, après `Resize` :

```rust
    /// Visibilité de la fenêtre navigateur, et si elle a le focus.
    ///
    /// **Deux signaux dans un seul message, et le second n'est pas
    /// décoratif** : la visibilité seule ne suffirait pas à ordonner le vivier
    /// du capteur quand plusieurs fenêtres sont visibles en même temps — elles
    /// ont alors exactement la même visibilité.
    Visibility {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        visible: bool,
        focused: bool,
    },
```

Dans `enum AgentControl`, après `Pointer` :

```rust
    /// La fenêtre dort — son encodeur et sa duplication ont été relâchés.
    ///
    /// `reason` vaut `"masquee"` (l'utilisateur l'a voulu) ou `"evincee"` (le
    /// vivier lui a pris sa place alors qu'il la regardait). Les deux ne se
    /// valent pas pour lui : la seconde mérite d'être dite.
    Asleep {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        asleep: bool,
        reason: String,
    },
```

Et le constructeur, auprès de `AgentControl::pointer` :

```rust
    pub fn asleep(asleep: bool, reason: &str) -> AgentControl {
        AgentControl::Asleep {
            version: CONTROL_VERSION,
            asleep,
            reason: reason.to_string(),
        }
    }
```

⚠️ **`evenements.rs:195` déstructure `ClientControl` directement** (`let ClientControl::Resize { .. } = &message;`) parce que l'enum n'avait qu'une variante. **Cette ligne cesse de compiler ici.** La tâche 8 la remplace ; d'ici là, la remplacer par un `match` exhaustif provisoire :

```rust
                    match &message {
                        ClientControl::Resize { width, height, .. } => {
                            self.pending_resize = Some((*width, *height));
                        }
                        ClientControl::Visibility { .. } => {}
                    }
```

- [ ] **Step 4: Ajouter les variantes du canal capteur**

Dans `agent/src/capteur/protocole.rs`, dans `enum VersCapteur`, après `ImageCle` :

```rust
    /// Visibilité annoncée par le client, relayée par l'enfant.
    ///
    /// **Ne se répond pas par `Fait`** : le capteur arbitre globalement, et la
    /// décision peut concerner une AUTRE fenêtre que celle qui a signalé.
    /// L'effet revient par `DepuisCapteur::Sommeil`, poussé sur la connexion
    /// média de chaque fenêtre concernée.
    Visibilite { visible: bool, focalisee: bool },
```

Dans `enum DepuisCapteur`, après `Etat` :

```rust
    /// Poussé, non sollicité, quand une fenêtre change d'état de sommeil.
    ///
    /// Distinct d'`Etat` à dessein : `Etat` alimente un cache lu à chaque tour
    /// de la boucle de transport (`is_alive`, `is_exhausted`, `dimensions`),
    /// et y mêler le sommeil ferait passer une annonce ponctuelle par un
    /// chemin conçu pour un état permanent.
    Sommeil { endormie: bool, raison: String },
```

- [ ] **Step 5: Ajouter les variantes TypeScript**

Dans `proto/ts/control.ts` :

```typescript
export interface VisibilityMessage {
    v: number;
    type: 'visibility';
    visible: boolean;
    focused: boolean;
}

export type ClientControl = ResizeMessage | VisibilityMessage;

export interface AsleepMessage {
    v: number;
    type: 'asleep';
    asleep: boolean;
    reason: string;
}
```

Étendre l'union et la liste de types :

```typescript
export type AgentControl =
    | ReadyMessage | SessionEndMessage
    | PointerMessage | RumbleMessage | CapabilitiesMessage | LinkMessage
    | AsleepMessage;

const TYPES_AGENT = [
    'ready', 'session-end', 'pointer', 'rumble', 'capabilities', 'link', 'asleep',
] as const;
```

Et l'encodeur, auprès d'`encodeResize` :

```typescript
export function encodeVisibility(visible: boolean, focused: boolean): string {
    const message: VisibilityMessage = {
        v: CONTROL_VERSION,
        type: 'visibility',
        visible,
        focused,
    };
    return JSON.stringify(message);
}
```

- [ ] **Step 6: Test TypeScript**

Dans `proto/ts/control.test.ts` :

```typescript
it('encode une visibilité à la version courante', () => {
    const json = JSON.parse(encodeVisibility(false, true));
    expect(json).toEqual({ v: CONTROL_VERSION, type: 'visibility', visible: false, focused: true });
});

it('accepte un message asleep venant de l’agent', () => {
    const raw = JSON.stringify({ v: CONTROL_VERSION, type: 'asleep', asleep: true, reason: 'evincee' });
    expect(parseAgentControl(raw)).toEqual({
        v: CONTROL_VERSION, type: 'asleep', asleep: true, reason: 'evincee',
    });
});
```

Ajouter `encodeVisibility` aux imports du fichier de test.

- [ ] **Step 7: Vérifier**

Run: `cd proto && cargo test 2>&1 | tail -5 && npm test 2>&1 | tail -10 && cd ../agent && cargo test protocole 2>&1 | tail -5`
Expected: PASS partout.

- [ ] **Step 8: Commit**

```bash
git add proto/src/control.rs proto/ts/control.ts proto/ts/control.test.ts \
        agent/src/capteur/protocole.rs agent/src/transport/evenements.rs
git commit -m "feat(d5): visibilite et sommeil dans les deux protocoles

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 5 : le registre de sommeil

**Files:**
- Create: `agent/src/capteur/sommeil.rs`
- Modify: `agent/src/capteur.rs`

**Interfaces:**
- Consumes: `capteur::vivier::{Vivier, Ordre, Raison, PLAFOND_EVEIL, HYSTERESIS}`
- Produces:
  - `pub fn inscrire(session: &str) -> std::sync::mpsc::Receiver<Ordre>`
  - `pub fn retirer(session: &str)`
  - `pub fn signaler(session: &str, visible: bool, focalisee: bool)`
  - `pub fn raison_en_texte(raison: Raison) -> &'static str`

Ce module tient les canaux et le fil de ré-arbitrage. **Il ne décide rien** — le `Vivier` décide. Pas de `#[cfg(windows)]` : ce ne sont que des canaux et un mutex, et il doit se compiler sur l'hôte pour que `vivier.rs` reste utilisable par les tests.

- [ ] **Step 1: Écrire le test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_session_inscrite_recoit_l_ordre_de_se_reveiller_quand_elle_devient_visible() {
        // Noms uniques : le registre est un état GLOBAL de processus, et les
        // tests Rust tournent en parallèle dans le même processus.
        let ordres = inscrire("t5-a");
        signaler("t5-a", true, true);
        assert_eq!(ordres.try_recv(), Ok(Ordre::Reveiller));
        retirer("t5-a");
    }

    #[test]
    fn une_session_retiree_ne_recoit_plus_rien() {
        let ordres = inscrire("t5-b");
        retirer("t5-b");
        signaler("t5-b", true, true);
        assert!(ordres.try_recv().is_err());
    }

    #[test]
    fn les_deux_raisons_ont_un_texte_stable_pour_le_client() {
        assert_eq!(raison_en_texte(Raison::Masquee), "masquee");
        assert_eq!(raison_en_texte(Raison::Evincee), "evincee");
    }
}
```

- [ ] **Step 2: Vérifier que ça échoue**

Run: `cd agent && cargo test sommeil 2>&1 | tail -10`
Expected: FAIL — module absent.

- [ ] **Step 3: Écrire l'implémentation**

```rust
//! Le registre global du sommeil : un `Vivier` partagé, un canal d'ordres par
//! fenêtre, et le tour de roue qui débloque l'hystérésis.
//!
//! **Il ne décide rien.** Toute la logique est dans `vivier.rs`, qui est pur et
//! testé ; ce module ne fait que la brancher sur des canaux.
//!
//! **Pourquoi un état global de processus plutôt qu'un objet passé de main en
//! main.** Chaque fenêtre du capteur vit sur son propre fil, créé par
//! `serveur::ouvrir_les_commandes`, et l'arbitrage est par nature transverse :
//! le signal d'une fenêtre peut endormir sa voisine. Le registre des attentes
//! de connexion média (`serveur.rs`) emploie déjà exactement ce patron, pour
//! la même raison. C'est aussi ce qui permet à ce sous-bloc de **ne pas
//! toucher `serveur.rs`**, dont la marge de taille est de 10 lignes.

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use crate::capteur::vivier::{Ordre, Raison, Vivier, HYSTERESIS, PLAFOND_EVEIL};

/// Période du tour de roue. Ni une cadence de rendu ni une horloge : c'est le
/// seul moyen pour une fenêtre bloquée sous hystérésis d'être réexaminée, et
/// 250 ms est très en deçà des 2 s d'hystérésis tout en restant négligeable.
const PERIODE_REARBITRAGE: Duration = Duration::from_millis(250);

struct Etat {
    vivier: Vivier,
    canaux: HashMap<String, Sender<Ordre>>,
}

static ETAT: OnceLock<Mutex<Etat>> = OnceLock::new();

fn etat() -> MutexGuard<'static, Etat> {
    let mutex = ETAT.get_or_init(|| {
        demarrer_le_tour_de_roue();
        Mutex::new(Etat {
            vivier: Vivier::nouveau(PLAFOND_EVEIL, HYSTERESIS),
            canaux: HashMap::new(),
        })
    });
    // Un empoisonnement ne doit pas tuer le capteur : l'état du vivier reste
    // cohérent (un `Vec` d'ordres perdu au pire), et refuser de servir serait
    // pire que de continuer.
    mutex.lock().unwrap_or_else(|empoisonne| empoisonne.into_inner())
}

/// **Un seul fil pour tout le processus**, démarré à la première inscription.
fn demarrer_le_tour_de_roue() {
    std::thread::spawn(|| loop {
        std::thread::sleep(PERIODE_REARBITRAGE);
        let mut garde = etat();
        let maintenant = Instant::now();
        let ordres = garde.vivier.rearbitrer(maintenant);
        distribuer(&mut garde, ordres);
    });
}

/// Envoie chaque ordre à la fenêtre concernée. Un canal rompu signale une
/// fenêtre déjà morte : on retire son entrée plutôt que de la journaliser à
/// chaque tour de roue.
fn distribuer(garde: &mut MutexGuard<'static, Etat>, ordres: Vec<(String, Ordre)>) {
    for (session, ordre) in ordres {
        let rompu = match garde.canaux.get(&session) {
            Some(canal) => canal.send(ordre).is_err(),
            None => false,
        };
        if rompu {
            garde.canaux.remove(&session);
            let maintenant = Instant::now();
            garde.vivier.retirer(&session, maintenant);
        }
    }
}

pub fn inscrire(session: &str) -> Receiver<Ordre> {
    let (emetteur, receveur) = channel::<Ordre>();
    let mut garde = etat();
    if garde.canaux.insert(session.to_string(), emetteur).is_some() {
        tracing::warn!(%session, "canal d'ordres remplacé pour cette session");
    }
    let ordres = garde.vivier.inscrire(session, Instant::now());
    distribuer(&mut garde, ordres);
    receveur
}

pub fn retirer(session: &str) {
    let mut garde = etat();
    garde.canaux.remove(session);
    let ordres = garde.vivier.retirer(session, Instant::now());
    distribuer(&mut garde, ordres);
}

pub fn signaler(session: &str, visible: bool, focalisee: bool) {
    let mut garde = etat();
    let ordres = garde.vivier.signaler(session, visible, focalisee, Instant::now());
    distribuer(&mut garde, ordres);
}

/// Le texte que le client recevra. **Stable** : il traverse deux protocoles et
/// s'affiche à l'utilisateur.
pub fn raison_en_texte(raison: Raison) -> &'static str {
    match raison {
        Raison::Masquee => "masquee",
        Raison::Evincee => "evincee",
    }
}
```

- [ ] **Step 4: Déclarer le module**

Dans `agent/src/capteur.rs`, auprès des modules non gatés :

```rust
pub mod sommeil;
```

- [ ] **Step 5: Vérifier**

Run: `cd agent && cargo test sommeil 2>&1 | tail -10`
Expected: PASS — 3 tests.

- [ ] **Step 6: Commit**

```bash
git add agent/src/capteur/sommeil.rs agent/src/capteur.rs
git commit -m "feat(d5): registre du sommeil, avec son tour de roue

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 6 : le fil de fenêtre dort et se réveille

**Files:**
- Modify: `agent/src/capteur/fenetre.rs`

**Interfaces:**
- Consumes: `capteur::sommeil::{inscrire, retirer, signaler, raison_en_texte}`, `capteur::vivier::Ordre`, `WindowsSource::sur_sortie(HWND, &str, u32, u32, Instant) -> Result<WindowsSource>`
- Produces: rien de public de neuf — le comportement interne du fil

`#[cfg(windows)]`, donc **aucun test unitaire possible**. Vérification : `cargo check --target x86_64-pc-windows-gnu`, puis la recette.

- [ ] **Step 1: Retenir de quoi reconstruire**

`Fenetre::ouvrir` consomme aujourd'hui l'attache. Ajouter une structure de paramètres et la conserver — **`clock_origin` en particulier : le reconstruire au réveil décalerait tous les horodatages de la piste vidéo.**

Remplacer la définition de `Fenetre` et son `ouvrir` :

```rust
/// De quoi reconstruire la source à l'identique après un sommeil.
///
/// **`clock_origin` est retenue, jamais recalculée** : elle est l'origine des
/// horodatages de la piste vidéo, et la refaire au réveil décalerait le flux de
/// l'écart entre les deux origines — le même piège que l'attache résout par
/// `origine_qpc`.
struct Parametres {
    hwnd: HWND,
    sortie: String,
    fps: u32,
    debit: u32,
    clock_origin: Instant,
}

pub struct Fenetre {
    /// `None` quand la fenêtre dort : l'encodeur et la duplication DXGI sont
    /// alors relâchés, et c'est tout l'objet du sous-bloc. La sortie virtuelle,
    /// elle, n'est jamais touchée — c'est ce qui évite d'infliger un abandon de
    /// mutex aux fenêtres voisines à chaque endormissement.
    source: Option<WindowsSource>,
    parametres: Parametres,
    session: String,
    sortie: String,
    largeur: u32,
    hauteur: u32,
}
```

Dans `ouvrir`, après la construction de `source`, remplacer la ligne de retour :

```rust
        let hwnd = HWND(hwnd as *mut core::ffi::c_void);
        let source = WindowsSource::sur_sortie(hwnd, &sortie, fps, debit, clock_origin)
            .with_context(|| format!("attache de la session {session}"))?;
        let (largeur, hauteur) = source.dimensions();
        let parametres = Parametres { hwnd, sortie: sortie.clone(), fps, debit, clock_origin };
        Ok(Fenetre { source: Some(source), parametres, session, sortie, largeur, hauteur })
```

- [ ] **Step 2: Les deux transitions**

Ajouter à `impl Fenetre` :

```rust
    /// Relâche l'encodeur et la duplication. **Sur CE fil**, jamais ailleurs :
    /// `Drop for H264Encoder` peut geler (risque observé, non attribué), et
    /// ici il ne gèlerait que cette fenêtre.
    fn dormir(&mut self) {
        if self.source.take().is_some() {
            tracing::info!(session = %self.session, "fenêtre endormie, encodeur et duplication relâchés");
        }
    }

    /// Reconstruit la source à l'identique, puis force une image clé.
    ///
    /// Sans l'image clé, le décodeur du navigateur n'aurait aucun point
    /// d'entrée dans le flux neuf et rendrait un écran gris jusqu'à la
    /// prochaine — le groupe d'images de l'encodeur matériel est ouvert.
    fn reveiller(&mut self) -> Result<()> {
        if self.source.is_some() {
            return Ok(());
        }
        let p = &self.parametres;
        let mut source = WindowsSource::sur_sortie(p.hwnd, &p.sortie, p.fps, p.debit, p.clock_origin)
            .with_context(|| format!("réveil de la session {}", self.session))?;
        source.request_keyframe().context("image clé au réveil")?;
        let (largeur, hauteur) = source.dimensions();
        self.largeur = largeur;
        self.hauteur = hauteur;
        self.source = Some(source);
        tracing::info!(session = %self.session, largeur, hauteur, "fenêtre réveillée");
        Ok(())
    }
```

- [ ] **Step 3: Sonder les ordres dans la boucle**

Dans `servir`, après le lancement du fil écrivain, inscrire la fenêtre :

```rust
        // Inscription au vivier. Une fenêtre naît ENDORMIE côté vivier ; le
        // premier signal de visibilité du client la réveillera. Elle est
        // pourtant construite éveillée ici — c'est voulu : la première image
        // doit pouvoir partir avant que le client ait eu le temps de parler,
        // sans quoi la page resterait grise le temps d'un aller-retour.
        let ordres_vivier = crate::capteur::sommeil::inscrire(&session);
```

Remplacer `let source = &mut self.source;` par un emprunt qui tolère l'absence. La boucle devient — **remplacer les points 1 et 2 de la boucle existante** :

```rust
        loop {
            // 0. Les ordres du vivier. Avant tout le reste : dormir libère des
            //    ressources, et il n'y a aucune raison d'encoder une image de
            //    plus quand l'ordre est déjà là.
            loop {
                match ordres_vivier.try_recv() {
                    Ok(Ordre::Dormir(raison)) => {
                        self.dormir();
                        let texte = crate::capteur::sommeil::raison_en_texte(raison);
                        if deposer_etat(
                            DepuisCapteur::Sommeil { endormie: true, raison: texte.into() },
                            &ecritures,
                        )
                        .is_err()
                        {
                            tracing::info!(%session, images, "fin de la fenêtre côté capteur");
                            crate::capteur::sommeil::retirer(&session);
                            return Ok(());
                        }
                    }
                    Ok(Ordre::Reveiller) => {
                        if let Err(erreur) = self.reveiller() {
                            // Un réveil qui échoue n'est pas fatal : la
                            // fenêtre reste endormie et le tour de roue la
                            // représentera. Elle garde sa sortie virtuelle.
                            tracing::warn!(%session, %erreur, "réveil refusé, la fenêtre reste endormie");
                        } else {
                            let _ = deposer_etat(
                                DepuisCapteur::Sommeil { endormie: false, raison: String::new() },
                                &ecritures,
                            );
                        }
                    }
                    Err(TryRecvError::Empty) => break,
                    // Le registre a laissé tomber notre émetteur : la fenêtre
                    // n'est plus arbitrée. On continue de servir plutôt que de
                    // clore — perdre l'arbitrage n'est pas perdre la session.
                    Err(TryRecvError::Disconnected) => break,
                }
            }

            // 1. Les commandes en attente. Servies MÊME ENDORMIE : refuser
            //    tout pendant le sommeil ferait échouer l'adaptation réseau et
            //    ferait clore la session par un chemin qui n'a rien à voir.
            let fin = match self.source.as_mut() {
                Some(source) => servir_les_commandes(source, &commandes, &reponses),
                None => servir_les_commandes_endormie(&commandes, &reponses),
            };
            if let Fin::Terminer(motif) = fin {
                tracing::info!(%session, images, motif, "fin de la fenêtre côté capteur");
                crate::capteur::sommeil::retirer(&session);
                return Ok(());
            }

            // 2. Une image, s'il y en a une — et il n'y en a jamais quand la
            //    fenêtre dort.
            let Some(source) = self.source.as_mut() else {
                std::thread::sleep(PAS_A_VIDE);
                continue;
            };
            match source.next_frame() {
                // … le corps existant, inchangé …
```

⚠️ Le reste de la boucle emprunte `source` : l'implémenteur doit s'assurer que l'emprunt du point 2 couvre bien tout le corps qui suit, et que le `dernier_etat` / les compteurs restent inchangés.

- [ ] **Step 4: Les deux fonctions d'appoint**

Ajouter, auprès de `servir_les_commandes` :

```rust
/// Sert les commandes d'une fenêtre ENDORMIE.
///
/// Toutes réussissent sans rien faire, sauf ce qui exige la source. Le
/// raisonnement : une fenêtre endormie n'a pas d'encodeur à régler, et rendre
/// une erreur ferait remonter un échec jusqu'à l'adaptation réseau de l'enfant,
/// qui le journaliserait comme un refus — un bruit sans objet.
fn servir_les_commandes_endormie(
    commandes: &Receiver<VersCapteur>,
    reponses: &Sender<DepuisCapteur>,
) -> Fin {
    loop {
        match commandes.try_recv() {
            Ok(VersCapteur::Visibilite { visible, focalisee }) => {
                // Sans session ici : le fil appelant l'a déjà, et cette
                // fonction est appelée depuis `servir` qui la connaît. Voir
                // l'appel — la session est passée par le module `sommeil`.
                let _ = (visible, focalisee);
                if reponses.send(DepuisCapteur::Fait).is_err() {
                    return Fin::Terminer("fil de commandes parti");
                }
            }
            Ok(_) => {
                if reponses.send(DepuisCapteur::Fait).is_err() {
                    return Fin::Terminer("fil de commandes parti");
                }
            }
            Err(TryRecvError::Empty) => return Fin::Continuer,
            Err(TryRecvError::Disconnected) => return Fin::Terminer("fil de commandes parti"),
        }
    }
}

/// Dépose un état sur la connexion média. Rend `Err` si le fil écrivain est
/// parti.
fn deposer_etat(etat: DepuisCapteur, ecritures: &SyncSender<AEcrire>) -> Result<(), ()> {
    ecritures.send(AEcrire::Etat(etat)).map_err(|_| ())
}
```

⚠️ **Défaut du code ci-dessus, à corriger par l'implémenteur, pas à recopier** : `servir_les_commandes_endormie` reçoit `Visibilite` sans pouvoir appeler `sommeil::signaler`, faute de connaître la session. **Passer `session: &str` en paramètre** et appeler `crate::capteur::sommeil::signaler(session, visible, focalisee)`. La même correction s'applique à `executer_commande`. *(Le plan expose ce défaut plutôt que de le taire : la clause « signaler un défaut du plan » vaut aussi pour un bug dans le code fourni.)*

- [ ] **Step 5: Router `Visibilite` dans `executer_commande`**

Changer la signature en `fn executer_commande(source: &mut WindowsSource, session: &str, message: VersCapteur) -> DepuisCapteur` et ajouter le bras :

```rust
        VersCapteur::Visibilite { visible, focalisee } => {
            // L'effet ne revient PAS par cette réponse : l'arbitrage est
            // global et peut concerner une autre fenêtre. Il revient par
            // `DepuisCapteur::Sommeil`, poussé sur la connexion média.
            crate::capteur::sommeil::signaler(session, visible, focalisee);
            return DepuisCapteur::Fait;
        }
```

Répercuter le paramètre `session` sur `servir_les_commandes` et `deposer`, qui appellent `executer_commande`.

- [ ] **Step 6: Vérifier la compilation croisée**

Run: `cd agent && cargo check --target x86_64-pc-windows-gnu 2>&1 | tail -20`
Expected: aucune erreur.

- [ ] **Step 7: Vérifier la taille du fichier**

Run: `wc -l agent/src/capteur/fenetre.rs`
Expected: < 500. Si le compte approche 480, extraire `dormir`/`reveiller` vers `agent/src/capteur/fenetre/sommeil.rs`.

- [ ] **Step 8: Commit**

```bash
git add agent/src/capteur/fenetre.rs
git commit -m "feat(d5): le fil de fenetre relache et reconstruit sa source

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 7 : l'enfant relaie la visibilité et annonce le sommeil

**Files:**
- Modify: `agent/src/source.rs`
- Modify: `agent/src/capteur/distante.rs`
- Modify: `agent/src/capteur/distante/tests.rs`

**Interfaces:**
- Consumes: `VersCapteur::Visibilite`, `DepuisCapteur::Sommeil`
- Produces:
  - `VideoSource::set_awake(&mut self, _visible: bool, _focalisee: bool) -> anyhow::Result<()>` (défaut inerte)
  - `Recu::Sommeil { endormie: bool, raison: String }`
  - `SourceDistante::sommeil_a_annoncer(&mut self) -> Option<(bool, String)>`

- [ ] **Step 1: Écrire les tests**

Dans `agent/src/capteur/distante/tests.rs` :

```rust
    #[test]
    fn set_awake_transmet_la_visibilite_au_capteur() {
        let (mut source, journal) = source_avec_canal_espion();
        source.set_awake(false, false).expect("le capteur accepte");
        assert_eq!(
            journal.lock().unwrap().as_slice(),
            &[VersCapteur::Visibilite { visible: false, focalisee: false }]
        );
    }

    #[test]
    fn un_sommeil_pousse_par_le_capteur_est_retenu_pour_le_client() {
        let (mut source, emetteur) = source_avec_file();
        emetteur
            .send(Recu::Sommeil { endormie: true, raison: "evincee".into() })
            .expect("dépôt");
        // Le sommeil est consommé par le tour de boucle qui cherche une image.
        assert!(source.next_frame().is_none());
        assert_eq!(source.sommeil_a_annoncer(), Some((true, "evincee".to_string())));
        assert_eq!(source.sommeil_a_annoncer(), None, "une annonce ne se répète pas");
    }
```

⚠️ Les deux fonctions d'appoint (`source_avec_canal_espion`, `source_avec_file`) **existent déjà** dans `tests.rs` sous des noms propres au fichier : l'implémenteur doit **lire `tests.rs` et réutiliser ses fabriques existantes** plutôt que d'en créer de nouvelles.

- [ ] **Step 2: Vérifier que ça échoue**

Run: `cd agent && cargo test distante 2>&1 | tail -15`
Expected: FAIL — `set_awake` et `sommeil_a_annoncer` inconnus.

- [ ] **Step 3: La méthode de trait**

Dans `agent/src/source.rs`, à la suite de `set_encode_size` :

```rust
    /// Annonce au producteur si la fenêtre est visible pour l'utilisateur, et
    /// si elle a le focus.
    ///
    /// Par défaut sans effet : une source fichier n'a personne à qui plaire.
    /// La source distante la relaie au capteur, qui arbitre GLOBALEMENT — la
    /// décision qui s'ensuit peut donc concerner une autre fenêtre que
    /// celle-ci, et ne revient jamais par la valeur de retour.
    fn set_awake(&mut self, _visible: bool, _focalisee: bool) -> anyhow::Result<()> {
        Ok(())
    }
```

- [ ] **Step 4: Côté `SourceDistante`**

Dans `agent/src/capteur/distante.rs`, ajouter la variante :

```rust
pub enum Recu {
    Image(AccessUnit),
    Etat { vivante: bool, epuisee: bool, largeur: u32, hauteur: u32 },
    Sommeil { endormie: bool, raison: String },
}
```

Ajouter le champ à `SourceDistante` :

```rust
    /// Dernier changement de sommeil reçu du capteur, en attente d'être
    /// annoncé au navigateur. Consommé par `sommeil_a_annoncer`.
    sommeil: Option<(bool, String)>,
```

L'initialiser à `None` dans `nouvelle`. Dans `next_frame`, ajouter le bras :

```rust
                Ok(Recu::Sommeil { endormie, raison }) => {
                    self.sommeil = Some((endormie, raison));
                }
```

Ajouter la méthode d'inspection, hors du bloc `impl VideoSource` :

```rust
    /// Rend le changement de sommeil en attente, et le consomme.
    ///
    /// **Une annonce ne se répète pas** : la boucle de transport l'interroge à
    /// chaque tour, et réémettre le même message inonderait le canal de
    /// contrôle.
    pub fn sommeil_a_annoncer(&mut self) -> Option<(bool, String)> {
        self.sommeil.take()
    }
```

Et l'implémentation du trait, dans `impl VideoSource for SourceDistante` :

```rust
    fn set_awake(&mut self, visible: bool, focalisee: bool) -> Result<()> {
        self.commander_simple(VersCapteur::Visibilite { visible, focalisee })
    }
```

- [ ] **Step 5: Vérifier**

Run: `cd agent && cargo test distante 2>&1 | tail -10`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add agent/src/source.rs agent/src/capteur/distante.rs agent/src/capteur/distante/tests.rs
git commit -m "feat(d5): l'enfant relaie la visibilite et retient le sommeil

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 8 : le transport applique et annonce

**Files:**
- Modify: `agent/src/transport/evenements.rs`
- Modify: `agent/src/transport/tick.rs`
- Modify: la structure de session (là où `pending_resize` est déclaré — le chercher par `grep -rn "pending_resize:" agent/src/transport/`)

**Interfaces:**
- Consumes: `ClientControl::Visibility`, `VideoSource::set_awake`, `AgentControl::asleep`, `queue_control(AgentControl)`
- Produces: le champ `pending_visibility: Option<(bool, bool)>`

- [ ] **Step 1: Écrire le test**

Dans le module `tests` de `agent/src/transport/evenements.rs` :

```rust
    #[test]
    fn un_message_de_visibilite_est_memorise_et_non_applique_sur_le_champ() {
        // Même raison que pour `Resize` : ce code court pendant le drainage de
        // `poll_output`, et relâcher un encodeur y romprait l'invariant d'une
        // seule mutation de `Rtc` par appel.
        let mut session = session_de_test();
        let json = r#"{"type":"visibility","v":3,"visible":false,"focused":false}"#;
        session.dispatch_controle_de_test(json);
        assert_eq!(session.pending_visibility, Some((false, false)));
    }
```

⚠️ `session_de_test` et `dispatch_controle_de_test` sont des noms **à faire correspondre** aux fabriques déjà présentes dans ce module de tests — l'implémenteur les lit et les réutilise.

- [ ] **Step 2: Vérifier que ça échoue**

Run: `cd agent && cargo test evenements 2>&1 | tail -10`
Expected: FAIL — champ inconnu.

- [ ] **Step 3: Déclarer le champ**

Auprès de `pending_resize`, dans la structure de session :

```rust
    /// Dernière visibilité annoncée par le navigateur, en attente
    /// d'application. Même raison de différer que `pending_resize`.
    pending_visibility: Option<(bool, bool)>,
```

L'initialiser à `None` partout où la session se construit.

- [ ] **Step 4: Mémoriser à la réception**

Dans `evenements.rs`, remplacer le `match` provisoire posé en tâche 4 :

```rust
                    match &message {
                        ClientControl::Resize { width, height, .. } => {
                            self.pending_resize = Some((*width, *height));
                        }
                        ClientControl::Visibility { visible, focused, .. } => {
                            self.pending_visibility = Some((*visible, *focused));
                        }
                    }
                    on_control(message);
```

- [ ] **Step 5: Appliquer et annoncer dans `tick`**

Dans `agent/src/transport/tick.rs`, **juste après** la branche `a1` du redimensionnement :

```rust
        // a1bis) Visibilité en attente. Après le redimensionnement et avant la
        //        vidéo, pour la même raison que lui : la décision peut
        //        relâcher un encodeur côté capteur, ce qui est long, et ne
        //        mute jamais `Rtc`.
        if let Some((visible, focalisee)) = self.pending_visibility.take() {
            if let Err(erreur) = self.source.set_awake(visible, focalisee) {
                // Non fatal : perdre l'arbitrage n'est pas perdre la session.
                tracing::warn!(%erreur, visible, focalisee, "visibilité refusée par le capteur");
            }
            return Ok(Tick::Continue);
        }

        // a1ter) Un changement de sommeil à annoncer au navigateur. Interrogé
        //        à chaque tour, mais `sommeil_a_annoncer` consomme : aucun
        //        message n'est jamais réémis.
        if let Some((endormie, raison)) = self.source.sommeil_a_annoncer() {
            self.queue_control(AgentControl::asleep(endormie, &raison));
            return Ok(Tick::Continue);
        }
```

⚠️ **`sommeil_a_annoncer` n'est pas sur le trait `VideoSource`** : `self.source` est typée par le trait. Deux voies, à trancher par l'implémenteur en regardant le type réel de `self.source` :

- **si `self.source` est un `Box<dyn VideoSource>`** : ajouter `sommeil_a_annoncer` **au trait**, avec un défaut `None` (c'est le patron déjà employé par `set_encode_size` et `set_awake`, et c'est la voie recommandée) ;
- si la session est générique sur `S: VideoSource`, la même addition au trait convient tout autant.

Dans les deux cas, la méthode inhérente ajoutée en tâche 7 sur `SourceDistante` devient l'implémentation du trait.

- [ ] **Step 6: Vérifier**

Run: `cd agent && cargo test 2>&1 | tail -15`
Expected: PASS — toute la suite.

Run: `cd agent && cargo check --target x86_64-pc-windows-gnu 2>&1 | tail -10`
Expected: aucune erreur.

- [ ] **Step 7: Commit**

```bash
git add agent/src/transport/evenements.rs agent/src/transport/tick.rs agent/src/source.rs
git commit -m "feat(d5): le transport applique la visibilite et annonce le sommeil

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 9 : le client annonce et affiche

**Files:**
- Create: `client/src/visibilite.ts`
- Create: `client/src/visibilite.test.ts`
- Modify: `client/src/main.ts`

**Interfaces:**
- Consumes: `encodeVisibility(visible: boolean, focused: boolean): string`, `RTCDataChannel`
- Produces: `attachVisibilite(cible: CibleVisibilite, envoyer: (charge: string) => void): () => void`

- [ ] **Step 1: Écrire les tests**

`client/src/visibilite.test.ts` :

```typescript
import { describe, it, expect, vi } from 'vitest';
import { attachVisibilite, type CibleVisibilite } from './visibilite';
import { CONTROL_VERSION } from '../../proto/ts/control';

function cibleFactice(): CibleVisibilite & { declencher: (nom: string) => void } {
    const ecouteurs = new Map<string, () => void>();
    return {
        hidden: false,
        focalisee: true,
        addEventListener(nom: string, rappel: () => void) {
            ecouteurs.set(nom, rappel);
        },
        removeEventListener(nom: string) {
            ecouteurs.delete(nom);
        },
        declencher(nom: string) {
            ecouteurs.get(nom)?.();
        },
    };
}

describe('attachVisibilite', () => {
    it('annonce l’état courant dès l’attache', () => {
        const envoyer = vi.fn();
        attachVisibilite(cibleFactice(), envoyer);
        expect(envoyer).toHaveBeenCalledWith(
            JSON.stringify({ v: CONTROL_VERSION, type: 'visibility', visible: true, focused: true }),
        );
    });

    it('annonce la disparition quand la page est cachée', () => {
        const cible = cibleFactice();
        const envoyer = vi.fn();
        attachVisibilite(cible, envoyer);
        envoyer.mockClear();
        cible.hidden = true;
        cible.focalisee = false;
        cible.declencher('visibilitychange');
        expect(envoyer).toHaveBeenCalledWith(
            JSON.stringify({ v: CONTROL_VERSION, type: 'visibility', visible: false, focused: false }),
        );
    });

    it('n’annonce pas deux fois le même état', () => {
        // Le canal de contrôle est fiable et ordonné : réémettre un état
        // inchangé n'apporterait rien et se paierait à chaque blur/focus
        // parasite.
        const cible = cibleFactice();
        const envoyer = vi.fn();
        attachVisibilite(cible, envoyer);
        envoyer.mockClear();
        cible.declencher('focus');
        expect(envoyer).not.toHaveBeenCalled();
    });

    it('annonce la perte de focus sans perte de visibilité', () => {
        const cible = cibleFactice();
        const envoyer = vi.fn();
        attachVisibilite(cible, envoyer);
        envoyer.mockClear();
        cible.focalisee = false;
        cible.declencher('blur');
        expect(envoyer).toHaveBeenCalledWith(
            JSON.stringify({ v: CONTROL_VERSION, type: 'visibility', visible: true, focused: false }),
        );
    });

    it('détache ses trois écouteurs', () => {
        const cible = cibleFactice();
        const detacher = attachVisibilite(cible, vi.fn());
        detacher();
        const envoyer = vi.fn();
        cible.declencher('visibilitychange');
        expect(envoyer).not.toHaveBeenCalled();
    });
});
```

- [ ] **Step 2: Vérifier que ça échoue**

Run: `cd client && npx vitest run src/visibilite.test.ts 2>&1 | tail -10`
Expected: FAIL — module introuvable.

- [ ] **Step 3: Écrire le module**

`client/src/visibilite.ts` :

```typescript
import { encodeVisibility } from '../../proto/ts/control';

/**
 * Ce dont ce module a besoin du document. Réduit à sa plus simple expression
 * pour être simulable : `document` réel en production, objet nu en test.
 */
export interface CibleVisibilite {
    readonly hidden: boolean;
    readonly focalisee: boolean;
    addEventListener(nom: string, rappel: () => void): void;
    removeEventListener(nom: string, rappel: () => void): void;
}

/**
 * Annonce visibilité et focus à l'agent, et rend de quoi se détacher.
 *
 * **Pourquoi le focus en plus de la visibilité.** Le vivier d'encodeurs de
 * l'agent arbitre par récence ; entre dix fenêtres toutes visibles, la
 * visibilité seule ne les ordonnerait pas et l'éviction serait arbitraire.
 *
 * **Ce que `visibilityState` ne rapporte PAS** : sur Chrome/Linux, une fenêtre
 * entièrement recouverte par une autre reste `visible`. Le sommeil s'y déclenche
 * donc à la minimisation et à l'onglet caché, pas au recouvrement.
 */
export function attachVisibilite(
    cible: CibleVisibilite,
    envoyer: (charge: string) => void,
): () => void {
    let dernier: string | undefined;

    const annoncer = () => {
        const charge = encodeVisibility(!cible.hidden, cible.focalisee);
        // Le canal de contrôle est fiable et ordonné : réémettre un état
        // inchangé n'apporte rien, et un navigateur émet volontiers plusieurs
        // événements pour un seul geste.
        if (charge === dernier) return;
        dernier = charge;
        envoyer(charge);
    };

    cible.addEventListener('visibilitychange', annoncer);
    cible.addEventListener('focus', annoncer);
    cible.addEventListener('blur', annoncer);
    annoncer();

    return () => {
        cible.removeEventListener('visibilitychange', annoncer);
        cible.removeEventListener('focus', annoncer);
        cible.removeEventListener('blur', annoncer);
    };
}
```

- [ ] **Step 4: Vérifier**

Run: `cd client && npx vitest run src/visibilite.test.ts 2>&1 | tail -10`
Expected: PASS — 5 tests.

- [ ] **Step 5: Câbler dans `main.ts`**

Import, auprès des autres :

```typescript
import { attachVisibilite } from './visibilite';
```

Une variable de détachement auprès de `detacherPleinEcran` :

```typescript
let detacherVisibilite: ReturnType<typeof attachVisibilite> | undefined;
```

Dans le `.then()` de `connectSession`, auprès du `ResizeObserver` :

```typescript
        // `document` ne porte pas `focus`/`blur` : ils vont sur `window`. La
        // cible réunit les deux sources sous l'interface que le module attend.
        detacherVisibilite = attachVisibilite(
            {
                get hidden() {
                    return document.hidden;
                },
                get focalisee() {
                    return document.hasFocus();
                },
                addEventListener(nom, rappel) {
                    if (nom === 'visibilitychange') document.addEventListener(nom, rappel);
                    else window.addEventListener(nom, rappel);
                },
                removeEventListener(nom, rappel) {
                    if (nom === 'visibilitychange') document.removeEventListener(nom, rappel);
                    else window.removeEventListener(nom, rappel);
                },
            },
            (charge) => {
                if (session.controlChannel.readyState !== 'open') return;
                session.controlChannel.send(charge);
            },
        );
```

Dans `onControl`, auprès des autres types, **avant** la branche `link` :

```typescript
        } else if (message.type === 'asleep') {
            if (message.asleep) {
                const texte =
                    message.reason === 'evincee'
                        ? 'image figée : trop de fenêtres actives'
                        : 'image figée : fenêtre masquée';
                // `persistant` : l'état dure tant que la fenêtre dort, il ne
                // doit pas être effacé par la minuterie d'un bandeau voisin.
                statut.afficher(texte, { persistant: true });
            } else {
                statut.masquer();
            }
```

Et dans la branche de fin de session, auprès des autres détachements :

```typescript
            detacherVisibilite?.();
```

- [ ] **Step 6: Vérifier l'ensemble**

Run: `cd client && npm test 2>&1 | tail -10 && npx tsc --noEmit 2>&1 | tail -10`
Expected: PASS, et aucune erreur de type.

- [ ] **Step 7: Commit**

```bash
git add client/src/visibilite.ts client/src/visibilite.test.ts client/src/main.ts
git commit -m "feat(d5): le client annonce sa visibilite et affiche le sommeil

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 10 : `CAPACITE`, et le remède au défaut de D4

**Files:**
- Modify: `agent/src/superviseur/boucle.rs:58`
- Modify: `agent/src/windows_source.rs:272-286`

**Interfaces:**
- Consumes: **le verdict de la tâche 2**
- Produces: `CAPACITE = 10` (ou 8), et un `set_encode_size` qui ne construit plus un encodeur de trop

⚠️ **Cette tâche est conditionnelle.** Appliquer la ligne de la table de décision (spec §3.4) que la tâche 2 a retenue.

- [ ] **Step 1: Lire le verdict**

Run: `grep -h "VERDICT" docs/superpowers/plans/journaux-multifenetres-d5/recyclage-*.log`

- [ ] **Step 2 (verdict `concurrence`) : détruire avant de construire**

Dans `agent/src/windows_source.rs`, remplacer le bloc de construction :

```rust
        let device = self.capture_mut().device().clone();
        // **Détruire AVANT de construire.** Reconstruire en gardant l'ancien
        // vivant demandait un encodeur de plus le temps de la bascule : à
        // `PLAFOND_EVEIL` encodeurs vivants, ce transitoire est refusé au
        // `SetOutputType` de la MFT NVIDIA (`MF_E_UNSUPPORTED_D3D_TYPE`) —
        // 18 refus sur 18 relevés à la seconde recette de D4, contre 3 succès
        // sur 3 à deux fenêtres.
        //
        // ⚠️ **Le prix est réel et assumé** : si la construction du neuf
        // échoue, l'ancien n'est plus là. La session perd alors sa vidéo au
        // lieu de garder son barreau — d'où le `self.fatal`, qui fait clore la
        // session par son chemin normal plutôt que de laisser une source
        // muette. Une panique, elle, traverserait `spawn_blocking` et
        // emporterait tout le processus.
        let ancien = std::mem::replace(&mut self.encoder, H264Encoder::inerte());
        drop(ancien);
        let mut encoder = match H264Encoder::new(
            &device,
            (self.width, self.height),
            (width, height),
            self.fps,
            self.bitrate,
        ) {
            Ok(encoder) => encoder,
            Err(erreur) => {
                self.fatal = true;
                return Err(erreur).context(
                    "encodeur neuf refusé après destruction de l'ancien : source épuisée",
                );
            }
        };
```

⚠️ **`H264Encoder::inerte()` n'existe pas**, et c'est un défaut connu de ce plan : Rust interdit de laisser un champ vide entre deux affectations. **Deux voies, à trancher par l'implémenteur** :

- **recommandée** — faire de `WindowsSource::encoder` un `Option<H264Encoder>` : `self.encoder.take()` puis `drop`, puis `self.encoder = Some(neuf)`. Coûte de dérouler l'`Option` aux ~6 points d'usage, ce que le compilateur énumère ;
- alternative — `H264Encoder::inerte()`, un constructeur qui ne touche pas Media Foundation. Ajoute une variante d'état à un type qui n'en avait pas, pour un usage transitoire : **moins bon**.

- [ ] **Step 2bis (verdict `cumule` ou `aucune-liberation`) : ne pas toucher `set_encode_size`**

Laisser le code en l'état, et **reporter dans le rapport que C2 n'est pas remédiable par cette voie**. Le remède devient « reconfigurer en place », hors portée de ce sous-bloc — le nommer comme suite à donner.

- [ ] **Step 3: `CAPACITE`**

Verdict `concurrence` → dans `agent/src/superviseur/boucle.rs:58` :

```rust
/// Nombre de fenêtres que le superviseur accepte simultanément.
///
/// **10, soit le vivier de sorties virtuelles du pilote** (mesure ① du
/// 31 juillet 2026 : refus à la 11ᵉ création en `ERROR_TOO_MANY_NAMES`). Ce
/// n'est plus le plafond d'encodeurs : depuis le sous-bloc D5, au plus
/// `vivier::PLAFOND_EVEIL` fenêtres sont éveillées à la fois, les autres
/// dormant sans rendre leur sortie.
///
/// **Relevé sur cette VM, pas une borne du système.**
const CAPACITE: usize = 10;
```

Autres verdicts → laisser `CAPACITE = 8` et le dire dans le rapport.

- [ ] **Step 4: Vérifier**

Run: `cd agent && cargo test 2>&1 | tail -10 && cargo check --target x86_64-pc-windows-gnu 2>&1 | tail -10`
Expected: PASS, aucune erreur.

Run: `wc -l agent/src/windows_source.rs`
Expected: le compte doit rester **proche de 648**. C'est de la dette gelée : si l'addition dépasse une vingtaine de lignes, extraire `set_encode_size` vers `agent/src/windows_source/encodage.rs`.

- [ ] **Step 5: Commit**

```bash
git add agent/src/superviseur/boucle.rs agent/src/windows_source.rs
git commit -m "fix(d5): detruire l'encodeur avant d'en construire un neuf, et dix fenetres

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 11 : la recette

**Files:**
- Modify: `docs/superpowers/plans/2026-08-02-multifenetres-vivier-encodeurs-resultats.md`
- Create: `docs/superpowers/plans/journaux-multifenetres-d5/critere{1,2,3}-*.log`

**Interfaces:**
- Consumes: tout ce qui précède
- Produces: le verdict des trois critères de réception (spec §5)

- [ ] **Step 1: Préparer le protocole**

Quatre contraintes héritées, **toutes obligatoires** (spec §5) :

1. **Source animée** : une fenêtre Chrome `--app` sur une page `canvas` animée par `requestAnimationFrame`, **un `--user-data-dir` par fenêtre**. Réutiliser `docs/superpowers/plans/journaux-multifenetres-d4/instrument/anim-d4.html` et `preparer-d4b.ps1`. Une fenêtre immobile ne produit **aucune** image.
2. **Aucune capture d'écran CDP pendant une mesure**, et toute évaluation CDP **bornée** : une page portant un flux WebRTC actif peut ne jamais rendre.
3. **Contrôler la survie de la VM après chaque rang** (`virsh list --all`).
4. **Copier `agent.log` après la fin réelle**, pas à la fin du pilote : les enfants meurent quand le navigateur se ferme.

- [ ] **Step 2: C1 — dépasser huit fenêtres**

Ouvrir 10 fenêtres, toutes visibles et tuilées. Relever :

```bash
# Côté agent
grep -c "fenêtre attachée au capteur" agent-critere1.log
grep -c "fenêtre endormie" agent-critere1.log
grep -c "fenêtre réveillée" agent-critere1.log
grep "sortie virtuelle" agent-critere1.log | tail -5
# Le rang 11 doit être refusé PAR LE PILOTE
grep -i "TOO_MANY_NAMES\|création de sortie refusée" agent-critere1.log
```

Côté navigateur, relever `framesDecoded` par fenêtre à deux instants espacés : **8 doivent croître, 2 rester stables**.

Puis focaliser une endormie, et vérifier qu'elle diffuse et que la moins récemment vue se fige.

**Puis exercer la seconde raison** : minimiser une fenêtre éveillée, vérifier qu'une endormie se réveille. Sans ce geste, `Raison::Masquee` reste du code jamais couru.

- [ ] **Step 3: C2 — le défaut de D4 est mort**

Sur cette même montée à 8 fenêtres éveillées :

```bash
grep -c "changement de taille d'encodage refusé" agent-critere1.log   # attendu : 0
grep -c "taille d'encodage changée" agent-critere1.log                 # attendu : > 0
```

Si aucun changement n'est demandé spontanément, forcer une descente de barreau avec `scripts/netem.sh adsl` sur `internalBridge` — **et reposer `off` en fin de mesure**.

- [ ] **Step 4: C3 — le réveil est borné**

Pour chacun des deux réveils de C1, relever l'écart entre la ligne `fenêtre réveillée` de l'agent et la première croissance de `framesDecoded` côté navigateur. **Rapporter les deux chiffres, sans seuil** — rien dans le dépôt ne permettrait d'en calibrer un.

- [ ] **Step 5: Relever le battement**

`grep -c "fenêtre endormie"` sur toute la séquence. C'est ce chiffre, et lui seul, qui juge l'`HYSTERESIS` de 2 s posée non calibrée.

- [ ] **Step 6: Contrôler la topologie depuis un processus NEUF**

```bash
MULTIFENETRE_DXGI=1 scripts/run-agent.sh
```

Comparer **par ensemble de noms**, jamais par cardinal : un tiers peut ajouter une sortie et compenser exactement un retrait.

- [ ] **Step 7: Écrire les résultats**

Compléter `2026-08-02-multifenetres-vivier-encodeurs-resultats.md` : verdict par critère, **le nombre d'exécutions à chaque fois**, et une section « ce que cette recette N'établit PAS » reprenant le §7 de la spec.

**Ne rien affirmer au-delà du relevé.**

- [ ] **Step 8: Commit**

```bash
git add docs/superpowers/plans/journaux-multifenetres-d5/ \
        docs/superpowers/plans/2026-08-02-multifenetres-vivier-encodeurs-resultats.md
git commit -m "recette(d5): les trois criteres du vivier d'encodeurs

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Tâche 12 : la documentation, et une correction due

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Corriger le chiffre faux**

`CLAUDE.md` annonce `agent/src/capteur/distante.rs` **à 487 lignes, marge 13**, et en fait une « marge étroite neuve à surveiller ». Le relevé autoritaire dit **206** — le fichier a été scindé (`distante/tests.rs`) sans que le chiffre suive. Corriger **les deux occurrences** (section D4 « Marge étroite neuve à surveiller », et l'encadré des marges de la section « Conventions de code »).

Balayer aussi par le **sens** et non par la formule : chercher « 487 », « marge de 13 », « distante.rs ».

- [ ] **Step 2: Relever les tailles à jour**

```bash
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | head -15
```

Reporter les marges étroites réelles, **et corriger le tableau dans le même mouvement**.

- [ ] **Step 3: Écrire la section D5**

Ajouter une section « 🛏️ Sous-bloc D5 — le vivier d'encodeurs » après celle de D4, portant :

- le verdict de la mesure pivot, et **ce qu'elle n'établit pas** (la couche du plafond de 8 reste inconnue) ;
- le verdict des trois critères, avec le nombre d'exécutions ;
- `PLAFOND_EVEIL = 8` et `HYSTERESIS = 2 s`, **la seconde explicitement non calibrée**, avec le chiffre de battement relevé ;
- le piège neuf : `visibilityState` ne rapporte pas le recouvrement sur Chrome/Linux ;
- la variable d'environnement `MULTIFENETRE_NVENC_CYCLES`, **dans le tableau des variables du banc** de la section « Mesures préalables au chantier D » — un seul tableau, pour qu'il n'y ait qu'un endroit à consulter ;
- annoter les sections D3 et D4 là où D5 les réfute ou les complète, **en cherchant les affirmations par le sens** (« conjecture », « jamais éprouvée », « n'a jamais été jouée ») et **en annotant l'affirmation elle-même, pas sa voisine**.

- [ ] **Step 4: Commit**

```bash
git add CLAUDE.md
git commit -m "docs(d5): le vivier d'encodeurs, et un chiffre de dette faux depuis D4

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Auto-revue du plan

**Couverture de la spec** — chaque section a sa tâche :

| Spec | Tâche |
| --- | --- |
| §3 mesure pivot + règle de décision | 1, 2 (et 10 pour l'application) |
| §4.1 le capteur arbitre | 3, 5 |
| §4.2 ce que libère une endormie | 6 |
| §4.3 LRU à deux signaux + hystérésis | 3 (logique), 4 et 9 (les deux signaux) |
| §4.4 les cinq pièces | 3, 4, 5, 6, 7, 9 |
| §4.5 ce que le sommeil ne touche pas | 6 (audio et entrée intouchés par construction — aucun code ne les vise) |
| §4.6 ce que voit l'utilisateur | 9 |
| §4.7 `CAPACITE` 8 → 10 | 10 |
| §5 C1, C2, C3 + protocole de recette | 11 |
| §6 risques | 1 (piège 8), 6 (piège 4 : `serveur.rs` non touché), 10 (risque 6), 11 (pièges 7, 9, 10) |
| §7 ce qui reste ouvert | 11 step 7, 12 step 3 |
| §8 dette de taille + correction `CLAUDE.md` | 6 step 7, 10 step 4, 12 |

**Deux défauts assumés du plan, exposés plutôt que tus** — tous deux signalés à l'endroit où ils mordent :

1. tâche 6, step 4 : `servir_les_commandes_endormie` ne peut pas appeler `sommeil::signaler` faute de connaître la session. Correction prescrite dans le même step.
2. tâche 10, step 2 : `H264Encoder::inerte()` n'existe pas ; deux voies proposées, la recommandée étant `Option<H264Encoder>`.

**Cohérence des types** — `Ordre`, `Raison`, `PLAFOND_EVEIL`, `HYSTERESIS` définis en tâche 3 et consommés sous ces noms exacts en 5 et 6 ; `VersCapteur::Visibilite { visible, focalisee }` défini en 4 et consommé en 6 et 7 ; `DepuisCapteur::Sommeil { endormie, raison }` défini en 4 et consommé en 6 et 7 ; `encodeVisibility` défini en 4 et consommé en 9 ; `set_awake(visible, focalisee)` défini en 7 et appelé en 8 ; `sommeil_a_annoncer` défini en 7 et appelé en 8, avec la remontée au trait prescrite dans le même step.
