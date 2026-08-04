# Sous-bloc D8 — le plein écran et Keyboard Lock — plan d'implémentation

> **Pour les agents d'exécution** : COMPÉTENCE REQUISE — employer
> `superpowers:subagent-driven-development` (recommandé) ou
> `superpowers:executing-plans` pour exécuter ce plan tâche par tâche. Les
> étapes emploient la syntaxe à case à cocher (`- [ ]`) pour le suivi.

**But** : quand une application Windows passe en plein écran, sa fenêtre
navigateur suit, à la bonne résolution, et Échap atteint le jeu au lieu de
casser le plein écran.

**Architecture** : le fil de fenêtre du capteur lit le style de la fenêtre
Windows et pousse un `PleinEcran { actif }` à l'enfant, qui le relaie en
`AgentControl::Fullscreen` sur le canal de données ; le client **arme** l'entrée
en plein écran au prochain geste utilisateur, `requestFullscreen()` exigeant une
activation transitoire. La résolution suit par le chemin `Resize` **déjà
existant**, dont le no-op en mode `SortieEntiere` devient un
`ChangeDisplaySettingsExW` sur la sortie virtuelle — qui garde son nom, donc son
attache et sa place dans la table.

**Pile technique** : Rust (agent, `proto`), TypeScript (client, `proto/ts`),
Win32 (`GetWindowLongPtrW`, `ChangeDisplaySettingsExW`, `EnumDisplaySettingsExW`),
CDP pour l'instrumentation de recette.

**Spec** : `docs/superpowers/specs/2026-08-04-multifenetres-plein-ecran-design.md`

## Contraintes globales

Ces exigences valent pour **toutes** les tâches, sans être répétées dans
chacune.

- **Plafond de 500 lignes par fichier source** (`CLAUDE.md`). Aucun fichier neuf
  ne naît au-dessus ; aucun fichier déjà au-dessus ne grossit. **Relever la
  taille par la commande, jamais par recopie d'un tableau** :
  ```bash
  { git ls-files; git ls-files --others --exclude-standard; } \
    | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
    | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
  ```
- **`agent/src/windows_source.rs` (638 lignes) ne doit PAS être touché.** Une
  condition explicite pèse sur lui : toute addition exige une extraction. Ce
  plan est conçu pour ne jamais l'atteindre — `resize` vit dans
  `agent/src/windows_source/redimensionnement.rs` (232 lignes).
- **`agent/src/capteur/fenetre.rs` est à 437 lignes** (marge 63). Y tenir : tout
  le code neuf substantiel va dans `agent/src/capteur/plein_ecran.rs`, seuls les
  appels restent dans `fenetre.rs`.
- **Vérification du code `#[cfg(windows)]` sur l'hôte** : depuis `agent/`,
  `cargo check --target x86_64-pc-windows-gnu`. Couvre types, emprunts,
  visibilités et durées de vie ; **pas l'édition de liens**. À passer avant toute
  compilation distante.
- **Commandes de test** : `cargo test` depuis `agent/` (workspace `proto` +
  `agent`) ; `npm test` depuis `client/` et depuis `proto/` ; `npm run typecheck`
  dans les deux.
- **`git add` nominatif, jamais `git add -A`** : l'arbre est partagé, un `-A` a
  déjà emporté le travail concurrent d'une autre tâche dans un commit qui ne
  compilait pas.
- **Toute variable d'environnement neuve doit être ajoutée à
  `scripts/run-agent.sh` DANS LA MÊME TÂCHE** que le code qui la lit. Le piège a
  été payé en D1 (`SUPERVISEUR`), D2 (`MULTIFENETRE_REPRISE`), D4 et D7 (`AUDIO`).
- **La VM n'est jamais supposée allumée** : `virsh list --all`, puis
  `virsh start Windows`, puis attendre que `ls /media/vm/dev` réponde — le
  montage CIFS survit à l'extinction, `mountpoint -q` ne prouve rien.
- **`set -a && source .env && set +a` avant tout `scripts/build-agent.sh`** :
  sans cela le script s'arrête **en silence** après « sources synchronisées », et
  le symptôme se lit comme une compilation réussie.

---

## Structure des fichiers

| Fichier | Responsabilité | Tâche |
| --- | --- | --- |
| `docs/superpowers/plans/journaux-multifenetres-d8/instrument/sonde-instrument.mjs` | **Créé** — P2 : l'instrument sait-il entrer en plein écran ? | 1 |
| `agent/src/diagnostics/multifenetre/mode_sortie.rs` | **Créé** — P1 : quels modes une sortie virtuelle accepte-t-elle vraiment | 3 |
| `agent/src/diagnostics/multifenetre.rs` | **Modifié** — un bras d'aiguillage pour P1 | 3 |
| `agent/src/capteur/plein_ecran.rs` | **Créé** — le prédicat pur, le suivi d'état de référence, et la seule lecture Win32 | 4, 6 |
| `agent/src/capteur/protocole.rs` | **Modifié** — `DepuisCapteur::PleinEcran` | 5 |
| `agent/src/capteur/distante.rs` | **Modifié** — `Recu::PleinEcran`, champ et accesseur | 5 |
| `agent/src/source.rs` | **Modifié** — `VideoSource::plein_ecran_a_annoncer` | 5 |
| `agent/src/capteur/fenetre.rs` | **Modifié** — l'appel bridé, et l'émission au changement | 6 |
| `proto/src/control.rs`, `proto/ts/control.ts` | **Modifiés** — `AgentControl::Fullscreen` | 7 |
| `agent/src/transport/tick.rs` | **Modifié** — la branche d'émission | 7 |
| `client/src/fullscreen.ts` | **Modifié** — `armerPleinEcran` | 8 |
| `client/src/main.ts` | **Modifié** — câblage du message | 8 |
| `agent/src/windows_source/redimensionnement.rs` | **Modifié** — le changement de mode, et `TAILLE_MAX_SORTIE` | 9 |
| `agent/src/windows_source/sortie.rs` | **Modifié** — le commentaire réfuté par D8 | 9 |
| `scripts/run-agent.sh` | **Modifié** — `MULTIFENETRE_MODE_SORTIE` et `PLEIN_ECRAN` | 3, 9 |
| `docs/superpowers/plans/journaux-multifenetres-d8/instrument/pilote-recette-d8.mjs` | **Créé** — le pilote de recette | 10 |
| `docs/superpowers/plans/2026-08-04-multifenetres-plein-ecran-resultats.md` | **Créé** — les résultats | 11 |
| `CLAUDE.md` | **Modifié** — l'index durable | 12 |

---

## Tâche 1 : P2 — la sonde d'instrument (hôte seul, aucune VM)

**Elle passe en premier parce qu'elle ne coûte rien et qu'elle décide du
protocole de recette entier.** Si l'instrument ne sait pas entrer en plein écran,
tout le §8 de la spec change de forme.

**Fichiers :**
- Créer : `docs/superpowers/plans/journaux-multifenetres-d8/instrument/sonde-instrument.mjs`
- Créer : `docs/superpowers/plans/journaux-multifenetres-d8/p2-instrument.log`

**Interfaces :**
- Consomme : rien.
- Produit : un verdict écrit, lu par la tâche 10 pour choisir le montage.

- [ ] **Étape 1 : écrire la sonde**

```javascript
#!/usr/bin/env node
// P2 du sous-bloc D8 : l'instrument de recette sait-il entrer en plein écran ?
//
// Deux questions, et une seule les deux : `requestFullscreen()` exige une
// activation utilisateur TRANSITOIRE, et D1 a relevé qu'un clic CDP n'en
// fournissait pas une pour `requestPointerLock` — sans que la cause soit
// isolée. Si celle-ci échoue, tout le protocole de recette de D8 change.
//
// Usage : node sonde-instrument.mjs
import { spawn } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const chromeBin = process.env.CHROME_BIN ?? 'google-chrome';

class Cdp {
    constructor(wsUrl) {
        this.ws = new WebSocket(wsUrl);
        this.nextId = 1;
        this.pending = new Map();
        this.ready = new Promise((resolve) =>
            this.ws.addEventListener('open', () => resolve(), { once: true }),
        );
        this.ws.addEventListener('message', (event) => {
            const message = JSON.parse(String(event.data));
            if (message.id !== undefined && this.pending.has(message.id)) {
                const { resolve, reject } = this.pending.get(message.id);
                this.pending.delete(message.id);
                if (message.error) reject(new Error(JSON.stringify(message.error)));
                else resolve(message.result);
            }
        });
    }
    async send(method, params = {}) {
        await this.ready;
        const id = this.nextId++;
        return new Promise((resolve, reject) => {
            this.pending.set(id, { resolve, reject });
            this.ws.send(JSON.stringify({ id, method, params }));
        });
    }
    async eval(expression, awaitPromise = false) {
        const r = await this.send('Runtime.evaluate', {
            expression,
            awaitPromise,
            returnByValue: true,
        });
        if (r.exceptionDetails) throw new Error(r.exceptionDetails.text);
        return r.result.value;
    }
}

const PAGE = `data:text/html,<body style="margin:0"><div id="c" style="width:100vw;height:100vh;background:#333"></div>`;

async function main() {
    const profil = await mkdtemp(join(tmpdir(), 'd8-p2-'));
    const chrome = spawn(chromeBin, [
        '--headless=new',
        '--remote-debugging-port=9333',
        `--user-data-dir=${profil}`,
        '--no-first-run',
        '--window-size=1280,720',
        PAGE,
    ]);
    // Attendre que le port réponde : un port qui répond ne prouve pas que
    // c'est le bon navigateur (D1), d'où le profil jetable ci-dessus.
    let cible;
    for (let i = 0; i < 50; i++) {
        try {
            const liste = await fetch('http://127.0.0.1:9333/json/list').then((r) => r.json());
            cible = liste.find((t) => t.type === 'page');
            if (cible) break;
        } catch {}
        await new Promise((r) => setTimeout(r, 200));
    }
    if (!cible) throw new Error('aucune page CDP');

    const cdp = new Cdp(cible.webSocketDebuggerUrl);
    await cdp.send('Runtime.enable');

    // Q2 d'abord : elle ne dépend d'aucun geste.
    const clavier = await cdp.eval(
        `({ present: typeof navigator.keyboard === 'object' && navigator.keyboard !== null,
            lock: typeof navigator.keyboard?.lock === 'function' })`,
    );

    // Q1 : armer la demande, puis fournir un clic CDP, puis relire l'état.
    await cdp.eval(`
        window.__verdict = { appele: false, rejet: null };
        document.addEventListener('pointerdown', () => {
            window.__verdict.appele = true;
            document.getElementById('c').requestFullscreen()
                .catch((e) => { window.__verdict.rejet = String(e); });
        }, { once: true });
    `);
    for (const type of ['mousePressed', 'mouseReleased']) {
        await cdp.send('Input.dispatchMouseEvent', {
            type, x: 40, y: 40, button: 'left', clickCount: 1,
        });
    }
    await new Promise((r) => setTimeout(r, 800));
    const pleinEcran = await cdp.eval(
        `({ ...window.__verdict, actif: document.fullscreenElement !== null })`,
    );

    console.log(JSON.stringify({ clavier, pleinEcran }, null, 2));
    const recu = pleinEcran.appele && pleinEcran.actif && clavier.lock;
    console.log(recu
        ? 'P2 RECU : l instrument sait entrer en plein ecran et verrouiller le clavier'
        : 'P2 REFUSE : voir le detail ci-dessus, et le repli du §4 de la spec');

    chrome.kill('SIGKILL');
    await rm(profil, { recursive: true, force: true });
    process.exit(recu ? 0 : 1);
}

main().catch((e) => { console.error(e); process.exit(2); });
```

- [ ] **Étape 2 : l'exécuter et verser le journal**

```bash
mkdir -p docs/superpowers/plans/journaux-multifenetres-d8/instrument
node docs/superpowers/plans/journaux-multifenetres-d8/instrument/sonde-instrument.mjs \
  2>&1 | tee docs/superpowers/plans/journaux-multifenetres-d8/p2-instrument.log
```

Attendu : un JSON puis une ligne `P2 RECU` ou `P2 REFUSE`. **Les deux issues sont
des résultats** — ne pas rejouer jusqu'à obtenir celle qu'on préfère.

- [ ] **Étape 3 : consigner le verdict et le repli retenu**

Écrire au-dessus du JSON, dans le journal, deux lignes :
- le verdict (`RECU` / `REFUSE`) ;
- **si `REFUSE`** : lequel des deux replis du §4 de la spec est retenu —
  installer `Xvfb` + `xdotool`, ou déclarer le critère ③ non éprouvé. ⚠️ Si
  `Xvfb` est retenu, écrire aussi que **les mesures ne se comparent alors à
  aucune campagne antérieure**.

- [ ] **Étape 4 : commit**

```bash
git add docs/superpowers/plans/journaux-multifenetres-d8/instrument/sonde-instrument.mjs \
        docs/superpowers/plans/journaux-multifenetres-d8/p2-instrument.log
git commit -m "mesure(d8): P2, ce que l instrument sait faire du plein ecran"
```

---

## Tâche 2 : P0 — le `grep` de D7, première mesure de F1

**Aucun code.** C'est le geste d'ouverture que D7 prescrit, et la **seule mesure
jamais prise** du correctif F1 — lequel repose aujourd'hui sur un argument de
flot de contrôle, pas sur une observation.

**Fichiers :**
- Créer : `docs/superpowers/plans/journaux-multifenetres-d8/p0-f1.log`

**Interfaces :**
- Consomme : rien.
- Produit : un verdict sur F1. S'il est réfuté, **D8 s'arrête et F1 se traite
  d'abord**.

- [ ] **Étape 1 : démarrer la VM et compiler le binaire de HEAD**

```bash
virsh list --all
virsh start Windows   # si « fermé »
until timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null; do sleep 5; done
until ls /media/vm/dev >/dev/null 2>&1; do sleep 5; done
set -a && source .env && set +a
scripts/build-agent.sh
```

⚠️ **Vérifier la TAILLE du binaire produit.** Une compilation annoncée en 0,13 s
est un aveu : après un aller-retour de sources, seul
`cargo clean --release -p agent` débloque (D4).

- [ ] **Étape 2 : lancer une session à DEUX fenêtres d'un même processus, et la laisser vivre plus de 30 s**

`compteurs audio` est **périodique** (`REPORT_INTERVAL = 30 s`,
`agent/src/windows_audio.rs`). Sous cette durée, un `A = 0` ne dit **rien** — et
c'est exactement le piège qui a masqué F1 pendant toute la recette D7.

```bash
node scripts/winrm.js "Get-Process agent"   # tuer tout agent survivant AVANT
scripts/run-agent.sh
# ouvrir deux fenêtres Bloc-notes (même exécutable, donc même groupe de PID)
# laisser vivre AU MOINS 90 s, pour obtenir au moins deux périodes
```

- [ ] **Étape 3 : relever, sur un journal MIS À PLAT**

```bash
cp /media/vm/…/agent.log docs/superpowers/plans/journaux-multifenetres-d8/p0-agent.log
sed 's/\x1b\[[0-9;]*m//g' \
  docs/superpowers/plans/journaux-multifenetres-d8/p0-agent.log > /tmp/agent-plat.log
grep -c 'compteurs audio' /tmp/agent-plat.log                      # A
grep 'compteurs audio' /tmp/agent-plat.log | grep -c 'actif=true'  # B
```

Le `sed` est **obligatoire** : les séquences ANSI de `tracing` séparent le nom du
champ de sa valeur, et `grep 'actif=true'` ne matche jamais littéralement.

- [ ] **Étape 4 : lire le relevé selon la grille**

| Relevé | Verdict | Suite |
| --- | --- | --- |
| `A = 0` | Mesure non prise | La session n'a pas vécu 30 s — recommencer, ne pas conclure |
| `A > 0`, `B = 0` | **Défaut** : des fenêtres vivent, aucune ne porte le son | Diagnostiquer avant D8 |
| `A > 0`, `B < A` | **F1 CONFIRMÉ** | Continuer le plan |
| `B == A` avec deux fenêtres d'un même PID | **F1 RÉFUTÉ** | **Arrêter D8**, traiter F1 d'abord |

- [ ] **Étape 5 : verser le journal et le verdict, puis commit**

```bash
git add docs/superpowers/plans/journaux-multifenetres-d8/p0-f1.log \
        docs/superpowers/plans/journaux-multifenetres-d8/p0-agent.log
git commit -m "mesure(d8): P0, la premiere et seule mesure de F1"
```

---

## Tâche 3 : P1 — quels modes une sortie virtuelle accepte-t-elle vraiment

**Fichiers :**
- Créer : `agent/src/diagnostics/multifenetre/mode_sortie.rs`
- Modifier : `agent/src/diagnostics/multifenetre.rs` (déclaration + aiguillage)
- Modifier : `scripts/run-agent.sh` (transmission de `MULTIFENETRE_MODE_SORTIE`)
- Créer : `docs/superpowers/plans/journaux-multifenetres-d8/p1-mode-sortie.log`

**Interfaces :**
- Consomme : `crate::moniteurs_virtuels` (création d'une sortie), et
  `crate::capture::enumerer_sorties` ou son équivalent employé par
  `placement_periodique` pour relire `GetDesc`/`DesktopCoordinates`.
- Produit : le verdict qui conditionne la tâche 9. Si négatif, **la tâche 9 se
  réduit à documenter le repli**, et le critère ② de la recette tombe.

- [ ] **Étape 1 : écrire la sonde**

Créer `agent/src/diagnostics/multifenetre/mode_sortie.rs` :

```rust
//! P1 du sous-bloc D8 : une sortie virtuelle SudoVDA accepte-t-elle un autre
//! mode que celui de sa création ?
//!
//! **L'énumération ne suffit pas, et c'est tout le sujet.** Un pilote peut
//! annoncer un mode et le refuser, comme il peut accepter un mode qu'il
//! n'énumère pas. Cette sonde fait donc les deux : elle énumère, PUIS elle
//! change réellement, PUIS elle relit par `GetDesc`/`DesktopCoordinates` —
//! jamais WMI, dont le champ a été vu périmé de 68 s sur ce terrain.
//!
//! `MULTIFENETRE_MODE_SORTIE=<L>x<H>` : crée une sortie à 1280×720, tente de
//! la passer à L×H, relève, puis la rend au pilote.

use anyhow::{Context, Result};
use windows::core::PCWSTR;
use windows::Win32::Graphics::Gdi::{
    ChangeDisplaySettingsExW, EnumDisplaySettingsExW, CDS_UPDATEREGISTRY, DEVMODEW,
    DISP_CHANGE_SUCCESSFUL, DM_PELSHEIGHT, DM_PELSWIDTH, ENUM_CURRENT_SETTINGS,
};

/// Modes annoncés par une sortie, relevés par énumération.
fn modes_annonces(nom: &str) -> Result<Vec<(u32, u32)>> {
    let large: Vec<u16> = nom.encode_utf16().chain(std::iter::once(0)).collect();
    let mut modes = Vec::new();
    let mut index = 0u32;
    loop {
        let mut dm = DEVMODEW { dmSize: std::mem::size_of::<DEVMODEW>() as u16, ..Default::default() };
        let ok = unsafe {
            EnumDisplaySettingsExW(PCWSTR(large.as_ptr()), windows::Win32::Graphics::Gdi::ENUM_DISPLAY_SETTINGS_MODE(index), &mut dm, 0)
        };
        if !ok.as_bool() {
            break;
        }
        modes.push((dm.dmPelsWidth, dm.dmPelsHeight));
        index += 1;
    }
    modes.sort_unstable();
    modes.dedup();
    Ok(modes)
}

/// Tente le changement de mode. Rend le code brut de `ChangeDisplaySettingsExW`.
fn changer_mode(nom: &str, largeur: u32, hauteur: u32) -> i32 {
    let large: Vec<u16> = nom.encode_utf16().chain(std::iter::once(0)).collect();
    let mut dm = DEVMODEW {
        dmSize: std::mem::size_of::<DEVMODEW>() as u16,
        dmFields: DM_PELSWIDTH | DM_PELSHEIGHT,
        dmPelsWidth: largeur,
        dmPelsHeight: hauteur,
        ..Default::default()
    };
    unsafe {
        ChangeDisplaySettingsExW(PCWSTR(large.as_ptr()), Some(&mut dm), None, CDS_UPDATEREGISTRY, None).0
    }
}

pub fn executer(consigne: &str) -> Result<()> {
    let (l, h) = consigne
        .split_once('x')
        .context("MULTIFENETRE_MODE_SORTIE attend <largeur>x<hauteur>, par exemple 1920x1080")?;
    let (cible_l, cible_h): (u32, u32) = (l.trim().parse()?, h.trim().parse()?);

    // Créer une sortie à 1280×720 par le chemin de production, battre le chien
    // de garde, et relever son nom DXGI.
    let sortie = crate::moniteurs_virtuels::creer_pour_sonde(1280, 720)
        .context("création de la sortie virtuelle de sonde")?;
    tracing::info!(nom = %sortie.nom_dxgi, "sortie de sonde créée");

    let avant = modes_annonces(&sortie.nom_affichage)?;
    tracing::info!(
        nombre = avant.len(),
        contient_cible = avant.contains(&(cible_l, cible_h)),
        modes = ?avant,
        "modes annonces avant changement"
    );

    let code = changer_mode(&sortie.nom_affichage, cible_l, cible_h);
    let accepte = code == DISP_CHANGE_SUCCESSFUL.0;
    tracing::info!(code, accepte, cible_l, cible_h, "changement de mode tente");

    // Relire par DXGI, source de vérité — jamais WMI.
    let reel = crate::moniteurs_virtuels::taille_dxgi_de(&sortie.nom_dxgi)?;
    tracing::info!(
        largeur = reel.0,
        hauteur = reel.1,
        conforme = (reel == (cible_l, cible_h)),
        "taille relue par GetDesc/DesktopCoordinates"
    );

    tracing::info!(
        verdict = if reel == (cible_l, cible_h) { "P1 RECU" } else { "P1 REFUSE" },
        "verdict de la sonde"
    );

    crate::moniteurs_virtuels::rendre_pour_sonde(sortie)?;
    Ok(())
}
```

⚠️ **Les trois fonctions `creer_pour_sonde`, `taille_dxgi_de` et
`rendre_pour_sonde` sont des noms de commodité de ce plan.** Employer les
fonctions réellement exposées par `agent/src/moniteurs_virtuels/` et
`agent/src/sortie_dxgi.rs` — les mêmes que `diagnostics/multifenetre/montee.rs`
et `capture_virtuelle.rs` emploient déjà. **Si aucune n'est publique, les rendre
`pub(crate)` plutôt que d'en dupliquer la logique.**

- [ ] **Étape 2 : brancher l'aiguillage**

Dans `agent/src/diagnostics/multifenetre.rs`, déclarer le module et ajouter le
bras. Le placer **avant** tout bras dont la variable partage un préfixe (leçon
de `MULTIFENETRE_NVENC_CYCLES` en D5 — il n'y a pas de collision ici, mais la
règle se respecte à l'ajout, pas après) :

```rust
mod mode_sortie;

// … dans aiguiller() :
    // P1 du sous-bloc D8 : une sortie virtuelle accepte-t-elle un autre mode
    // que celui de sa création ? L'énumération ne suffit pas — un pilote peut
    // annoncer un mode et le refuser.
    if let Ok(consigne) = std::env::var("MULTIFENETRE_MODE_SORTIE") {
        mode_sortie::executer(&consigne)?;
        return Ok(true);
    }
```

- [ ] **Étape 3 : transmettre la variable dans `scripts/run-agent.sh`**

**Dans cette tâche, pas dans une autre.** Ajouter `MULTIFENETRE_MODE_SORTIE` à
la liste des variables transmises, à côté de `MULTIFENETRE_PLAFOND`.

- [ ] **Étape 4 : vérifier la compilation croisée**

```bash
cd agent && cargo check --target x86_64-pc-windows-gnu
```
Attendu : sortie 0. Les avertissements `dead_code` préexistants sont attendus ;
**vérifier leur nature, jamais leur nombre** (il dérive selon la fraîcheur du
build).

- [ ] **Étape 5 : compiler sur la VM et exécuter**

```bash
set -a && source .env && set +a
scripts/build-agent.sh
node scripts/winrm.js "Get-Process agent"          # aucun survivant
MULTIFENETRE_VDD_PURGE=1 scripts/run-agent.sh      # purger les orphelines
MULTIFENETRE_MODE_SORTIE=1920x1080 scripts/run-agent.sh
```

- [ ] **Étape 6 : relever le verdict, et le contrôler depuis un processus NEUF**

Le processus mesureur est juge et partie, et une sortie virtuelle lui survit :

```bash
MULTIFENETRE_DXGI=1 scripts/run-agent.sh   # topologie depuis un processus neuf
```

Comparer les **ensembles de noms** avant/après, **jamais des cardinaux** : un
tiers peut ajouter une sortie et compenser exactement un retrait.

- [ ] **Étape 7 : verser le journal et committer**

```bash
git add agent/src/diagnostics/multifenetre/mode_sortie.rs \
        agent/src/diagnostics/multifenetre.rs scripts/run-agent.sh \
        docs/superpowers/plans/journaux-multifenetres-d8/p1-mode-sortie.log
git commit -m "mesure(d8): P1, les modes qu une sortie virtuelle accepte reellement"
```

⚠️ **Si P1 est refusé** : la tâche 9 se réduit à ses étapes de documentation, le
critère ② de la tâche 11 tombe, et le repli est **acté** — on accepte l'upscale
et on le documente. Les tâches 4 à 8 sont **inchangées**.

---

## Tâche 4 : le prédicat pur du plein écran

**Fichiers :**
- Créer : `agent/src/capteur/plein_ecran.rs`
- Modifier : `agent/src/capteur.rs` (déclaration du module)
- Test : dans le même fichier, `#[cfg(test)] mod tests` — convention de
  `capteur/audio.rs` et `capteur/repartiteur.rs`

**Interfaces :**
- Consomme : rien.
- Produit :
  - `pub const WS_CAPTION_BIT: u32 = 0x00C0_0000;`
  - `pub const WS_THICKFRAME_BIT: u32 = 0x0004_0000;`
  - `pub fn est_sans_bordure(style: u32) -> bool`
  - `pub struct SuiviBordure` avec
    `pub fn nouveau(style_initial: u32) -> Self` et
    `pub fn observer(&mut self, style: u32) -> Option<bool>`

- [ ] **Étape 1 : écrire les tests qui échouent**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Style d'une fenêtre applicative ordinaire : titre + cadre.
    const ORDINAIRE: u32 = WS_CAPTION_BIT | WS_THICKFRAME_BIT | 0x1000_0000;
    /// Style d'une fenêtre « borderless fullscreen » : ni l'un ni l'autre.
    const SANS_BORDURE: u32 = 0x1000_0000;

    #[test]
    fn une_fenetre_a_titre_et_cadre_a_une_bordure() {
        assert!(!est_sans_bordure(ORDINAIRE));
    }

    #[test]
    fn une_fenetre_sans_titre_ni_cadre_n_a_pas_de_bordure() {
        assert!(est_sans_bordure(SANS_BORDURE));
    }

    #[test]
    fn le_titre_seul_suffit_a_faire_une_bordure() {
        // Une fenêtre non redimensionnable garde sa barre de titre : elle
        // n'est pas en plein écran.
        assert!(!est_sans_bordure(WS_CAPTION_BIT));
    }

    #[test]
    fn le_cadre_seul_suffit_a_faire_une_bordure() {
        assert!(!est_sans_bordure(WS_THICKFRAME_BIT));
    }

    #[test]
    fn le_premier_observer_sur_l_etat_initial_n_annonce_rien() {
        // La garde du §5.2 : l'état lu à l'attache fait référence, et l'on
        // n'annonce que les CHANGEMENTS.
        let mut suivi = SuiviBordure::nouveau(ORDINAIRE);
        assert_eq!(suivi.observer(ORDINAIRE), None);
    }

    #[test]
    fn une_fenetre_nee_sans_bordure_n_annonce_rien() {
        // Sans cette garde, une application déjà sans bordure au démarrage
        // ferait entrer sa fenêtre navigateur en plein écran sans raison.
        let mut suivi = SuiviBordure::nouveau(SANS_BORDURE);
        assert_eq!(suivi.observer(SANS_BORDURE), None);
    }

    #[test]
    fn la_perte_de_la_bordure_annonce_le_plein_ecran_une_seule_fois() {
        let mut suivi = SuiviBordure::nouveau(ORDINAIRE);
        assert_eq!(suivi.observer(SANS_BORDURE), Some(true));
        // Deuxième lecture identique : plus rien à annoncer.
        assert_eq!(suivi.observer(SANS_BORDURE), None);
    }

    #[test]
    fn le_retour_de_la_bordure_annonce_la_sortie_du_plein_ecran() {
        let mut suivi = SuiviBordure::nouveau(ORDINAIRE);
        assert_eq!(suivi.observer(SANS_BORDURE), Some(true));
        assert_eq!(suivi.observer(ORDINAIRE), Some(false));
        assert_eq!(suivi.observer(ORDINAIRE), None);
    }

    #[test]
    fn un_aller_retour_complet_annonce_deux_fois_et_pas_davantage() {
        let mut suivi = SuiviBordure::nouveau(ORDINAIRE);
        let annonces: Vec<Option<bool>> = [SANS_BORDURE, SANS_BORDURE, ORDINAIRE, ORDINAIRE]
            .into_iter()
            .map(|s| suivi.observer(s))
            .collect();
        assert_eq!(annonces, vec![Some(true), None, Some(false), None]);
    }
}
```

- [ ] **Étape 2 : lancer les tests pour vérifier qu'ils échouent**

```bash
cd agent && cargo test plein_ecran
```
Attendu : ÉCHEC de compilation — `est_sans_bordure` et `SuiviBordure` n'existent
pas.

- [ ] **Étape 3 : écrire l'implémentation minimale**

```rust
//! Détecter qu'une application est passée en plein écran, par son style.
//!
//! **Pur, sans aucun `cfg`** — comme `capteur/audio.rs` et
//! `capteur/repartiteur.rs` avant lui : il reçoit un `u32` de style et rend des
//! booléens. La seule lecture Win32 vit dans le module `win` en bas de fichier,
//! derrière un `#[cfg(windows)]`.
//!
//! ❌ **Le critère du cadrage jeux §4.1 — comparer le rect de la fenêtre à
//! celui du moniteur — est MORT dans cette architecture, et il ne faut pas y
//! revenir.** Depuis le sous-bloc D1, `superviseur::placement::poser` donne à
//! la fenêtre exactement la taille de sa sortie virtuelle, et
//! `controler_le_placement` la lui réimpose périodiquement : « rect fenêtre ==
//! rect moniteur » est l'état NOMINAL. Ce critère n'est pas inopérant, il est
//! TOUJOURS VRAI — une implémentation fidèle annoncerait le plein écran en
//! permanence, pour toutes les fenêtres.
//!
//! `SHQueryUserNotificationState` a été écarté pour une autre raison : il est
//! global à la session interactive, donc à N fenêtres il ne dit pas LAQUELLE,
//! et il ne voit pas le « borderless fullscreen » que les jeux emploient.

/// `WS_CAPTION` — la fenêtre a une barre de titre.
pub const WS_CAPTION_BIT: u32 = 0x00C0_0000;
/// `WS_THICKFRAME` — la fenêtre a un cadre redimensionnable.
pub const WS_THICKFRAME_BIT: u32 = 0x0004_0000;

/// Vrai si le style ne porte ni barre de titre ni cadre redimensionnable.
pub fn est_sans_bordure(style: u32) -> bool {
    style & (WS_CAPTION_BIT | WS_THICKFRAME_BIT) == 0
}

/// Suit l'état de bordure d'une fenêtre et n'annonce que les CHANGEMENTS.
///
/// **L'état lu à l'attache fait référence** (§5.2 de la spec) : une application
/// née sans bordure n'annonce rien, et ne fait donc pas entrer sa fenêtre
/// navigateur en plein écran sans raison.
pub struct SuiviBordure {
    sans_bordure: bool,
}

impl SuiviBordure {
    pub fn nouveau(style_initial: u32) -> Self {
        Self { sans_bordure: est_sans_bordure(style_initial) }
    }

    /// Rend `Some(actif)` au changement, `None` sinon.
    pub fn observer(&mut self, style: u32) -> Option<bool> {
        let courant = est_sans_bordure(style);
        if courant == self.sans_bordure {
            return None;
        }
        self.sans_bordure = courant;
        Some(courant)
    }
}
```

Déclarer le module dans `agent/src/capteur.rs` :

```rust
pub mod plein_ecran;
```

- [ ] **Étape 4 : lancer les tests pour vérifier qu'ils passent**

```bash
cd agent && cargo test plein_ecran
```
Attendu : 8 tests passés.

- [ ] **Étape 5 : commit**

```bash
git add agent/src/capteur/plein_ecran.rs agent/src/capteur.rs
git commit -m "feat(d8): le predicat de plein ecran, pur et eprouve sur l hote"
```

---

## Tâche 5 : le message capteur → enfant

**Fichiers :**
- Modifier : `agent/src/capteur/protocole.rs` (361 lignes, marge 139)
- Modifier : `agent/src/capteur/distante.rs` (354 lignes, marge 146)
- Modifier : `agent/src/source.rs` (348 lignes, marge 152)
- Test : `agent/src/capteur/distante/tests.rs` (462 lignes, **marge 38** — y
  ajouter un seul test, court)

**Interfaces :**
- Consomme : rien des tâches précédentes.
- Produit :
  - `DepuisCapteur::PleinEcran { actif: bool }`
  - `Recu::PleinEcran { actif: bool }`
  - `VideoSource::plein_ecran_a_annoncer(&mut self) -> Option<bool>`, défaut
    `None`, **consommante** — même régime que `sommeil_a_annoncer`.

- [ ] **Étape 1 : écrire le test qui échoue**

Dans `agent/src/capteur/distante/tests.rs` :

```rust
#[test]
fn un_plein_ecran_pousse_est_annonce_une_seule_fois() {
    // Même régime que `sommeil_a_annoncer` : l'annonce est un CHANGEMENT, elle
    // se consomme. Sans quoi la branche de transport qui l'interroge à ~100 Hz
    // inonderait le canal de contrôle.
    let (envoi, reception) = std::sync::mpsc::channel();
    let mut source = SourceDistante::nouvelle(
        Box::new(CanalFactice::nouveau()),
        reception,
        1280,
        720,
    );
    envoi.send(Recu::PleinEcran { actif: true }).unwrap();
    assert_eq!(source.next_frame(), None);
    assert_eq!(source.plein_ecran_a_annoncer(), Some(true));
    assert_eq!(source.plein_ecran_a_annoncer(), None);
}
```

⚠️ `CanalFactice` est le double déjà présent dans ce fichier — employer son nom
réel, et le constructeur de `SourceDistante` tel qu'il y est déjà appelé, plutôt
que la forme ci-dessus si elle diffère.

- [ ] **Étape 2 : lancer le test pour vérifier qu'il échoue**

```bash
cd agent && cargo test un_plein_ecran_pousse_est_annonce_une_seule_fois
```
Attendu : ÉCHEC — `Recu::PleinEcran` et `plein_ecran_a_annoncer` n'existent pas.

- [ ] **Étape 3 : écrire l'implémentation minimale**

Dans `agent/src/capteur/protocole.rs`, à côté de `Audio` :

```rust
    /// La fenêtre Windows est passée en plein écran, ou en est sortie. Poussé
    /// non sollicité, **au changement seulement**.
    ///
    /// Distinct d'`Etat` pour la même raison que `Sommeil`, `Part` et `Audio` :
    /// `Etat` alimente un cache lu à chaque tour de la boucle de transport, et
    /// y mêler une annonce ponctuelle passerait par un chemin conçu pour un
    /// état permanent.
    PleinEcran { actif: bool },
```

Dans `agent/src/capteur/distante.rs`, dans `enum Recu` :

```rust
    /// Changement de plein écran poussé par le capteur, non sollicité. Retenu
    /// par `SourceDistante::plein_ecran` jusqu'à ce que
    /// `plein_ecran_a_annoncer` le consomme.
    PleinEcran { actif: bool },
```

Dans `struct SourceDistante` :

```rust
    /// Dernier changement de plein écran reçu du capteur, en attente d'être
    /// annoncé au navigateur. Consommé par `plein_ecran_a_annoncer`.
    ///
    /// **Même régime d'écrasement que `sommeil`** : deux changements arrivés
    /// entre deux lectures s'écrasent, seul le dernier survit. Il n'y a pas
    /// d'état courant jumeau ici, contrairement à `sommeil`/`endormie` : rien
    /// dans l'enfant n'a besoin de relire le plein écran hors de l'annonce.
    plein_ecran: Option<bool>,
```

Initialiser à `None` dans `nouvelle`, dans la boucle `next_frame` :

```rust
                Ok(Recu::PleinEcran { actif }) => {
                    self.plein_ecran = Some(actif);
                }
```

et l'implémentation du trait :

```rust
    fn plein_ecran_a_annoncer(&mut self) -> Option<bool> {
        self.plein_ecran.take()
    }
```

Dans `agent/src/source.rs`, à côté d'`audio_a_appliquer` :

```rust
    /// Rend le changement de plein écran en attente d'annonce, et le consomme.
    ///
    /// **État courant, pas un historique** : deux changements arrivés entre
    /// deux lectures s'écrasent — même régime que `sommeil_a_annoncer`. Comme
    /// elle consomme, la branche de transport qui l'interroge à ~100 Hz ne peut
    /// pas inonder le canal de contrôle.
    ///
    /// Par défaut sans effet : une source fichier n'a pas de fenêtre Windows,
    /// et une `WindowsSource` tenue en direct par son propre processus n'a
    /// personne pour la lui pousser.
    fn plein_ecran_a_annoncer(&mut self) -> Option<bool> {
        None
    }
```

- [ ] **Étape 4 : lancer les tests pour vérifier qu'ils passent**

```bash
cd agent && cargo test
cargo check --target x86_64-pc-windows-gnu
```
Attendu : tous verts, `cargo check` en sortie 0.

- [ ] **Étape 5 : relever la taille de `distante/tests.rs`**

```bash
wc -l agent/src/capteur/distante/tests.rs
```
Sa marge était de **38 lignes**. Si l'addition la franchit, **extraire** — ne
pas compresser.

- [ ] **Étape 6 : commit**

```bash
git add agent/src/capteur/protocole.rs agent/src/capteur/distante.rs \
        agent/src/capteur/distante/tests.rs agent/src/source.rs
git commit -m "feat(d8): le plein ecran voyage du capteur vers l enfant"
```

---

## Tâche 6 : la lecture du style, et l'émission au changement

**Fichiers :**
- Modifier : `agent/src/capteur/plein_ecran.rs` (ajout du module `win`)
- Modifier : `agent/src/capteur/fenetre.rs` (**437 lignes, marge 63** — n'y
  mettre que l'appel)

**Interfaces :**
- Consomme : `SuiviBordure`, `est_sans_bordure` (tâche 4) ;
  `DepuisCapteur::PleinEcran` (tâche 5).
- Produit :
  - `pub const PERIODE_STYLE: Duration = Duration::from_millis(250);`
  - `#[cfg(windows)] pub fn lire_style(hwnd: HWND) -> Option<u32>`

- [ ] **Étape 1 : ajouter la lecture Win32 et sa période**

Dans `agent/src/capteur/plein_ecran.rs`, sous le code pur :

```rust
use std::time::Duration;

/// Période de relecture du style de la fenêtre.
///
/// ⚠️ **Ce n'est PAS le tour de roue de `PERIODE_REARBITRAGE`**, qui vit sur le
/// fil de sommeil (`capteur/sommeil.rs`) et n'a pas les `HWND`. La valeur est du
/// même ordre, délibérément, mais la constante est propre à ce module : les
/// faire suivre l'une l'autre coupleraient deux mécanismes que rien ne lie.
///
/// **Jamais à l'image** : un `GetWindowLongPtrW` est bon marché, pas gratuit à
/// N × 90 i/s.
pub const PERIODE_STYLE: Duration = Duration::from_millis(250);

#[cfg(windows)]
mod win {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowLongPtrW, GWL_STYLE};

    /// Style de la fenêtre, ou `None` si la fenêtre a disparu.
    ///
    /// `GetWindowLongPtrW` rend `0` sur erreur, ce qui est aussi un style
    /// valide en théorie — mais une fenêtre applicative réelle en a toujours au
    /// moins un bit. On traite donc `0` comme une disparition : le seul risque
    /// est de ne rien annoncer, jamais d'annoncer à tort.
    pub fn lire_style(hwnd: HWND) -> Option<u32> {
        let brut = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) };
        if brut == 0 {
            return None;
        }
        Some(brut as u32)
    }
}

#[cfg(windows)]
pub use win::lire_style;
```

- [ ] **Étape 2 : brancher l'appel dans le fil de fenêtre**

Dans `agent/src/capteur/fenetre.rs`, dans la boucle du fil de fenêtre, à côté
des autres traitements périodiques :

```rust
// D8 : le style de la fenêtre dit si l'application est passée en plein écran.
// Bridé par son propre minuteur — voir `plein_ecran::PERIODE_STYLE`.
if dernier_style.elapsed() >= plein_ecran::PERIODE_STYLE {
    dernier_style = std::time::Instant::now();
    if let Some(style) = plein_ecran::lire_style(hwnd) {
        if let Some(actif) = suivi_bordure.observer(style) {
            tracing::info!(actif, "plein ecran de la fenetre Windows");
            pousser(DepuisCapteur::PleinEcran { actif });
        }
    }
}
```

`suivi_bordure` est construit à l'ouverture de la fenêtre, sur le style lu à ce
moment-là :

```rust
let mut suivi_bordure = plein_ecran::SuiviBordure::nouveau(
    plein_ecran::lire_style(hwnd).unwrap_or(0),
);
let mut dernier_style = std::time::Instant::now();
```

⚠️ **`pousser` est un nom de commodité** : employer le mécanisme réellement
utilisé dans ce fichier pour émettre `Sommeil`, `Part` et `Audio` vers l'enfant.

- [ ] **Étape 3 : vérifier la compilation croisée**

```bash
cd agent && cargo check --target x86_64-pc-windows-gnu && cargo test
```
Attendu : sortie 0, tous les tests verts.

- [ ] **Étape 4 : relever la taille de `fenetre.rs`**

```bash
wc -l agent/src/capteur/fenetre.rs
```
Sa marge était de **63 lignes**. Si l'addition la franchit, **extraire** vers
`agent/src/capteur/fenetre/plein_ecran.rs` — ne pas compresser.

- [ ] **Étape 5 : commit**

```bash
git add agent/src/capteur/plein_ecran.rs agent/src/capteur/fenetre.rs
git commit -m "feat(d8): le fil de fenetre lit le style et annonce le plein ecran"
```

---

## Tâche 7 : `AgentControl::Fullscreen`, des deux côtés du fil

**Fichiers :**
- Modifier : `proto/src/control.rs` (386 lignes, marge 114)
- Modifier : `proto/ts/control.ts` (123 lignes)
- Modifier : `agent/src/transport/tick.rs` (297 lignes, marge 203)
- Test : `proto/src/control.rs` (`mod tests`, l. 234) et `proto/ts/control.test.ts`

**Interfaces :**
- Consomme : `VideoSource::plein_ecran_a_annoncer` (tâche 5).
- Produit :
  - Rust : `AgentControl::Fullscreen { version: u8, active: bool }` et
    `AgentControl::fullscreen(active: bool) -> AgentControl`
  - TS : `FullscreenMessage { v: number; type: 'fullscreen'; active: boolean }`,
    ajouté à l'union `AgentControl` **et** au tableau `TYPES_AGENT`.

- [ ] **Étape 1 : écrire les deux tests qui échouent**

Dans `proto/src/control.rs`, `mod tests` :

```rust
    #[test]
    fn le_plein_ecran_se_serialise_en_kebab_case_avec_sa_version() {
        let json = serde_json::to_string(&AgentControl::fullscreen(true)).unwrap();
        assert!(json.contains(r#""type":"fullscreen""#), "{json}");
        assert!(json.contains(r#""active":true"#), "{json}");
        assert!(json.contains(r#""v":"#), "{json}");
    }

    #[test]
    fn le_plein_ecran_fait_l_aller_retour() {
        let origine = AgentControl::fullscreen(false);
        let json = serde_json::to_string(&origine).unwrap();
        let relu: AgentControl = serde_json::from_str(&json).unwrap();
        assert_eq!(relu, origine);
    }
```

Dans `proto/ts/control.test.ts` :

```typescript
it('analyse un message de plein écran', () => {
    const message = parseAgentControl(
        JSON.stringify({ v: CONTROL_VERSION, type: 'fullscreen', active: true }),
    );
    expect(message).toEqual({ v: CONTROL_VERSION, type: 'fullscreen', active: true });
});
```

⚠️ Employer l'import de `CONTROL_VERSION` et la forme d'assertion déjà présents
dans ce fichier plutôt que celle ci-dessus si elles diffèrent.

- [ ] **Étape 2 : lancer les tests pour vérifier qu'ils échouent**

```bash
cd agent && cargo test le_plein_ecran
cd ../proto && npm test
```
Attendu : ÉCHEC des deux côtés.

- [ ] **Étape 3 : écrire l'implémentation minimale**

Dans `proto/src/control.rs`, dans `enum AgentControl` :

```rust
    /// L'application Windows est passée en plein écran, ou en est sortie.
    ///
    /// **Le client ARME, il n'agit pas** : `requestFullscreen()` exige une
    /// activation utilisateur transitoire qu'un message de canal de données ne
    /// fournit pas. Voir `client/src/fullscreen.ts`.
    ///
    /// Le sens est UNIQUE — le navigateur ne force jamais l'état de la fenêtre
    /// Windows —, et c'est ce qui rend toute oscillation impossible.
    Fullscreen {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        active: bool,
    },
```

et son constructeur, à côté d'`asleep` :

```rust
    pub fn fullscreen(active: bool) -> AgentControl {
        AgentControl::Fullscreen { version: CONTROL_VERSION, active }
    }
```

Dans `proto/ts/control.ts` :

```typescript
export interface FullscreenMessage {
    v: number;
    type: 'fullscreen';
    active: boolean;
}
```

Ajouter `| FullscreenMessage` à l'union `AgentControl`, **et** `'fullscreen'` au
tableau `TYPES_AGENT` — les deux, sans quoi le message est rejeté à l'analyse.

Dans `agent/src/transport/tick.rs`, juste après la branche `a1ter`
(`sommeil_a_annoncer`) :

```rust
        // a1ter-bis) Un changement de plein écran à annoncer au navigateur.
        //            Même régime que a1ter juste au-dessus :
        //            `plein_ecran_a_annoncer` CONSOMME, donc aucun message
        //            n'est jamais réémis et cette branche ne peut pas inonder
        //            le canal de contrôle même à ~100 Hz.
        if let Some(actif) = self.source.plein_ecran_a_annoncer() {
            self.queue_control(AgentControl::fullscreen(actif));
            return Ok(Tick::Continue);
        }
```

- [ ] **Étape 4 : lancer les tests pour vérifier qu'ils passent**

```bash
cd agent && cargo test && cargo check --target x86_64-pc-windows-gnu
cd ../proto && npm test && npm run typecheck
cd ../client && npm test && npm run typecheck
```
Attendu : tout vert. Le `typecheck` du client compte : il consomme l'union
`AgentControl` élargie.

- [ ] **Étape 5 : commit**

```bash
git add proto/src/control.rs proto/ts/control.ts proto/ts/control.test.ts \
        agent/src/transport/tick.rs
git commit -m "feat(d8): AgentControl::Fullscreen, des deux cotes du fil"
```

---

## Tâche 8 : le client arme, puis entre au prochain geste

**Fichiers :**
- Modifier : `client/src/fullscreen.ts` (114 lignes)
- Modifier : `client/src/main.ts` (279 lignes)
- Test : `client/src/fullscreen.test.ts`

**Interfaces :**
- Consomme : `FullscreenMessage` (tâche 7).
- Produit :
  - `export function armerPleinEcran(options: ArmementOptions): () => void`
  - `export interface ArmementOptions { cible: CibleEcran; doc: DocumentPleinEcran; ecouteurs: CibleEvenement }`
  - `export interface CibleEvenement { addEventListener(type, e, o?): void; removeEventListener(type, e): void }`
  - `export function armerPleinEcranAuDOM(cible: CibleEcran): () => void`

- [ ] **Étape 1 : écrire les tests qui échouent**

Dans `client/src/fullscreen.test.ts` :

```typescript
/** Double de test pour une cible d'événements. */
function faireEcouteurs() {
    const ecouteurs = new Map<string, EventListener[]>();
    return {
        addEventListener(type: string, e: EventListener) {
            const l = ecouteurs.get(type) ?? [];
            l.push(e);
            ecouteurs.set(type, l);
        },
        removeEventListener(type: string, e: EventListener) {
            ecouteurs.set(type, (ecouteurs.get(type) ?? []).filter((x) => x !== e));
        },
        declencher(type: string) {
            for (const e of ecouteurs.get(type) ?? []) e(new Event(type));
        },
        compte(type: string) {
            return (ecouteurs.get(type) ?? []).length;
        },
    };
}

describe('armement du plein écran', () => {
    it("n'entre pas en plein écran avant un geste utilisateur", () => {
        // `requestFullscreen()` exige une activation transitoire : appeler
        // depuis le message serait rejeté par le navigateur.
        const requestFullscreen = vi.fn().mockResolvedValue(undefined);
        const cible = { requestFullscreen };
        const doc = { fullscreenElement: null, exitFullscreen: vi.fn(), addEventListener: vi.fn(), removeEventListener: vi.fn() };
        const ecouteurs = faireEcouteurs();
        armerPleinEcran({ cible, doc, ecouteurs });
        expect(requestFullscreen).not.toHaveBeenCalled();
    });

    it('entre en plein écran au premier pointerdown', () => {
        const requestFullscreen = vi.fn().mockResolvedValue(undefined);
        const cible = { requestFullscreen };
        const doc = { fullscreenElement: null, exitFullscreen: vi.fn(), addEventListener: vi.fn(), removeEventListener: vi.fn() };
        const ecouteurs = faireEcouteurs();
        armerPleinEcran({ cible, doc, ecouteurs });
        ecouteurs.declencher('pointerdown');
        expect(requestFullscreen).toHaveBeenCalledOnce();
    });

    it('entre en plein écran au premier keydown, sans clic', () => {
        // Un joueur à la manette ou au clavier n'a aucune raison de cliquer.
        const requestFullscreen = vi.fn().mockResolvedValue(undefined);
        const cible = { requestFullscreen };
        const doc = { fullscreenElement: null, exitFullscreen: vi.fn(), addEventListener: vi.fn(), removeEventListener: vi.fn() };
        const ecouteurs = faireEcouteurs();
        armerPleinEcran({ cible, doc, ecouteurs });
        ecouteurs.declencher('keydown');
        expect(requestFullscreen).toHaveBeenCalledOnce();
    });

    it("n'entre qu'une fois, et retire ses écouteurs après", () => {
        const requestFullscreen = vi.fn().mockResolvedValue(undefined);
        const cible = { requestFullscreen };
        const doc = { fullscreenElement: null, exitFullscreen: vi.fn(), addEventListener: vi.fn(), removeEventListener: vi.fn() };
        const ecouteurs = faireEcouteurs();
        armerPleinEcran({ cible, doc, ecouteurs });
        ecouteurs.declencher('pointerdown');
        ecouteurs.declencher('pointerdown');
        expect(requestFullscreen).toHaveBeenCalledOnce();
        expect(ecouteurs.compte('pointerdown')).toBe(0);
        expect(ecouteurs.compte('keydown')).toBe(0);
    });

    it("n'arme rien si la page est déjà en plein écran", () => {
        const cible = { requestFullscreen: vi.fn().mockResolvedValue(undefined) };
        const doc = { fullscreenElement: cible, exitFullscreen: vi.fn(), addEventListener: vi.fn(), removeEventListener: vi.fn() };
        const ecouteurs = faireEcouteurs();
        armerPleinEcran({ cible, doc, ecouteurs });
        expect(ecouteurs.compte('pointerdown')).toBe(0);
    });

    it('la fonction de détachement retire les écouteurs sans avoir armé', () => {
        // Fin de session avant tout geste : rien ne doit survivre.
        const cible = { requestFullscreen: vi.fn().mockResolvedValue(undefined) };
        const doc = { fullscreenElement: null, exitFullscreen: vi.fn(), addEventListener: vi.fn(), removeEventListener: vi.fn() };
        const ecouteurs = faireEcouteurs();
        const detacher = armerPleinEcran({ cible, doc, ecouteurs });
        detacher();
        expect(ecouteurs.compte('pointerdown')).toBe(0);
        ecouteurs.declencher('pointerdown');
        expect(cible.requestFullscreen).not.toHaveBeenCalled();
    });
});
```

- [ ] **Étape 2 : lancer les tests pour vérifier qu'ils échouent**

```bash
cd client && npm test -- fullscreen
```
Attendu : ÉCHEC — `armerPleinEcran` n'est pas exporté.

- [ ] **Étape 3 : écrire l'implémentation minimale**

Dans `client/src/fullscreen.ts` :

```typescript
/** Ce dont l'armement a besoin d'une cible d'événements (le document). */
export interface CibleEvenement {
    addEventListener(type: string, ecouteur: EventListener): void;
    removeEventListener(type: string, ecouteur: EventListener): void;
}

export interface ArmementOptions {
    cible: CibleEcran;
    doc: DocumentPleinEcran;
    ecouteurs: CibleEvenement;
}

/** Les gestes qui portent une activation utilisateur transitoire. */
const GESTES = ['pointerdown', 'keydown'] as const;

/**
 * Arme l'entrée en plein écran sur le prochain geste utilisateur.
 *
 * **On arme, on n'agit pas.** `requestFullscreen()` exige une activation
 * utilisateur transitoire ; un message reçu sur canal de données n'en est pas
 * une, et l'appel serait rejeté. C'est le mécanisme retenu au §4.1 du cadrage
 * jeux, et le même que le spike multi-fenêtres a validé pour `window.open()` :
 * un seul mécanisme pour les deux besoins.
 *
 * Le clavier compte autant que le pointeur : un joueur à la manette ou au
 * clavier n'a aucune raison de cliquer.
 *
 * **Keyboard Lock n'est pas à demander ici** : `attachFullscreen` verrouille
 * déjà sur `fullscreenchange`, quelle que soit l'origine de l'entrée.
 *
 * Renvoie une fonction de détachement, à appeler en fin de session.
 */
export function armerPleinEcran({ cible, doc, ecouteurs }: ArmementOptions): () => void {
    if (doc.fullscreenElement) return () => {};

    const detacher = (): void => {
        for (const geste of GESTES) ecouteurs.removeEventListener(geste, surGeste);
    };
    const surGeste = (): void => {
        detacher();
        void cible.requestFullscreen();
    };
    for (const geste of GESTES) ecouteurs.addEventListener(geste, surGeste);
    return detacher;
}

/** Valeurs par défaut pour utilisation dans le navigateur réel. */
export function armerPleinEcranAuDOM(cible: CibleEcran): () => void {
    return armerPleinEcran({
        cible,
        doc: document as unknown as DocumentPleinEcran,
        ecouteurs: document as unknown as CibleEvenement,
    });
}
```

- [ ] **Étape 4 : lancer les tests pour vérifier qu'ils passent**

```bash
cd client && npm test -- fullscreen && npm run typecheck
```
Attendu : les 6 tests neufs passent, plus les anciens.

- [ ] **Étape 5 : câbler le message dans `main.ts`**

Ajouter, à côté de la branche `'asleep'` :

```typescript
        } else if (message.type === 'fullscreen') {
            // Sens UNIQUE : l'application Windows décide, le navigateur suit.
            // Sortir n'exige aucune activation utilisateur ; entrer, si — d'où
            // l'armement.
            detacherArmement?.();
            detacherArmement = undefined;
            if (message.active) {
                detacherArmement = armerPleinEcranAuDOM(document.documentElement);
            } else {
                void document.exitFullscreen().catch(() => {
                    // Sortir d'un plein écran qu'on n'a pas est sans
                    // conséquence : l'utilisateur a pu en sortir lui-même.
                });
            }
```

Déclarer la variable à côté de `detacherPleinEcran` :

```typescript
let detacherArmement: (() => void) | undefined;
```

et la détacher dans la branche `'session-end'`, à côté des quatre autres :

```typescript
            detacherArmement?.();
```

Importer `armerPleinEcranAuDOM` à la ligne 8, à côté d'`attachFullscreenAuDOM`.

- [ ] **Étape 6 : vérifier**

```bash
cd client && npm test && npm run typecheck
```
Attendu : tout vert.

- [ ] **Étape 7 : relever la taille de `main.ts`**

```bash
wc -l client/src/main.ts
```
Il était à 279. Loin du plafond ; relever tout de même, la règle vaut par la
commande.

- [ ] **Étape 8 : commit**

```bash
git add client/src/fullscreen.ts client/src/fullscreen.test.ts client/src/main.ts
git commit -m "feat(d8): le client arme le plein ecran, et entre au prochain geste"
```

---

## Tâche 9 : la résolution suit — le changement de mode

⚠️ **Conditionnée à P1 (tâche 3).** Si P1 a été refusé, **sauter les étapes 1 à
5** et n'exécuter que les étapes 6 et 7 (documentation du repli).

**Fichiers :**
- Modifier : `agent/src/windows_source/redimensionnement.rs` (232 lignes)
- Modifier : `agent/src/windows_source/sortie.rs` (188 lignes) — le commentaire
  que D8 réfute
- Modifier : `scripts/run-agent.sh` (`PLEIN_ECRAN`)
- Test : `agent/src/windows_source/sortie.rs` (`mod tests` déjà présent)

**Interfaces :**
- Consomme : rien des tâches précédentes.
- Produit :
  - `pub const TAILLE_MAX_SORTIE: (u32, u32) = (1920, 1080);`
  - `pub fn borner_a_la_taille_max(demande: (u32, u32)) -> (u32, u32)`

- [ ] **Étape 1 : écrire les tests qui échouent**

Dans `agent/src/windows_source/sortie.rs`, `mod tests` :

```rust
    #[test]
    fn une_taille_sous_le_plafond_passe_telle_quelle() {
        assert_eq!(borner_a_la_taille_max((1280, 720)), (1280, 720));
    }

    #[test]
    fn une_taille_4k_est_ramenee_au_plafond() {
        // D6 a mesuré le décodeur du navigateur saturé dès huit fenêtres de
        // 720p : 9× les pixels d'une seule est exactement ce qu'il encaisse le
        // plus mal.
        assert_eq!(borner_a_la_taille_max((3840, 2160)), TAILLE_MAX_SORTIE);
    }

    #[test]
    fn le_bornage_preserve_le_rapport_d_aspect() {
        // Un 21:9 borné indépendamment sur chaque axe déformerait l'image.
        let (l, h) = borner_a_la_taille_max((3440, 1440));
        assert!(l <= TAILLE_MAX_SORTIE.0 && h <= TAILLE_MAX_SORTIE.1, "{l}x{h}");
        let ecart = (l as f64 / h as f64) - (3440.0 / 1440.0);
        assert!(ecart.abs() < 0.01, "rapport {l}/{h} contre 3440/1440");
    }

    #[test]
    fn le_bornage_rend_des_dimensions_paires() {
        // Une fenêtre Windows impose des dimensions paires, et un encodeur
        // NV12 aussi.
        let (l, h) = borner_a_la_taille_max((3441, 1441));
        assert_eq!(l % 2, 0, "largeur {l}");
        assert_eq!(h % 2, 0, "hauteur {h}");
    }
```

- [ ] **Étape 2 : lancer les tests pour vérifier qu'ils échouent**

```bash
cd agent && cargo test borner_a_la_taille_max
```
Attendu : ÉCHEC de compilation.

- [ ] **Étape 3 : écrire l'implémentation**

Dans `agent/src/windows_source/sortie.rs` :

```rust
/// Taille maximale qu'une sortie virtuelle prendra sur demande de viewport.
///
/// ⚠️ **NON CALIBRÉE.** C'est un garde-fou posé par prudence, sans qu'aucun
/// jugement visuel ne l'ait jugée — exactement la lacune que `BPP_MIN` traîne
/// depuis le chantier C volet 1. Sa raison est mesurée, elle : D6 a relevé le
/// décodeur du navigateur saturé dès huit fenêtres de 1280×720 (18,03 %
/// d'images jetées au barreau plein, une exécution), et un écran 4K
/// demanderait 9× les pixels d'une seule de ces fenêtres.
pub const TAILLE_MAX_SORTIE: (u32, u32) = (1920, 1080);

/// Ramène une taille demandée sous `TAILLE_MAX_SORTIE`, à rapport d'aspect
/// préservé et en dimensions paires.
pub fn borner_a_la_taille_max((l, h): (u32, u32)) -> (u32, u32) {
    let (max_l, max_h) = TAILLE_MAX_SORTIE;
    if l <= max_l && h <= max_h {
        return (l & !1, h & !1);
    }
    // Le facteur le plus contraignant des deux axes : borner chaque axe
    // séparément déformerait l'image.
    let facteur = f64::min(max_l as f64 / l as f64, max_h as f64 / h as f64);
    let borne = |x: u32| (((x as f64 * facteur).round() as u32).max(2)) & !1;
    (borne(l), borne(h))
}
```

Dans `agent/src/windows_source/redimensionnement.rs`, remplacer le garde
`if !self.mode.redimensionne_la_fenetre()` par le changement de mode :

```rust
        if !self.mode.redimensionne_la_fenetre() {
            // D8 : la sortie SUIT désormais le viewport, ce que D1 déclarait
            // hors de portée. Ce n'était pas une erreur de D1 : le pilote
            // SudoVDA n'expose effectivement aucun `SET_MODE` parmi ses six
            // IOCTL. Ce qui a changé est le chemin employé — l'API d'affichage
            // de WINDOWS, pas le canal du pilote (mesure P1 du sous-bloc D8).
            //
            // La sortie GARDE SON NOM : son attache capteur→enfant, sa
            // duplication et sa place dans la table survivent. Seule la
            // duplication perd son accès, et la reprise de D2 la rouvre.
            let (largeur, hauteur) = super::sortie::borner_a_la_taille_max((width, height));
            return self.changer_mode_de_sortie(largeur, hauteur);
        }
```

et écrire `changer_mode_de_sortie` dans le même fichier :

```rust
    /// Change le mode de la sortie virtuelle, puis y repose la fenêtre.
    ///
    /// Rend `Ok(())` même quand le pilote refuse : rien n'a échoué du point de
    /// vue de la session, l'image reste simplement mise à l'échelle. Un `Err`
    /// ferait journaliser un incident à chaque connexion (le `ResizeObserver`
    /// émet une fois à l'observation initiale).
    fn changer_mode_de_sortie(&mut self, largeur: u32, hauteur: u32) -> Result<()> {
        // … `ChangeDisplaySettingsExW` sur `self.nom_sortie`, comme la sonde P1 ;
        // puis `superviseur::placement::poser(self.hwnd, &rect_relu)` ;
        // puis relecture par `GetDesc`/`DesktopCoordinates` et mise à jour des
        // dimensions retenues — la taille OBTENUE, jamais la demandée.
        //
        // ✅ Le superviseur ne combattra pas ce placement, et c'est vérifié :
        // `replacer_si_besoin` réénumère les sorties DXGI à chaque contrôle et
        // les résout par NOM, puis pose sur le `rect` fraîchement lu. Il n'y a
        // aucun rectangle mémorisé qui deviendrait périmé.
        todo!("reprendre le corps éprouvé par la sonde P1 (tâche 3)")
    }
```

⚠️ **Le `todo!` ci-dessus est à remplacer par le corps réel.** Il est écrit ainsi
parce que la forme exacte des appels dépend de ce que la sonde P1 aura établi —
notamment si `CDS_UPDATEREGISTRY` suffit ou s'il faut `CDS_RESET`. **Reprendre
littéralement le code qui a marché dans `mode_sortie.rs`**, et ne rien inventer
au-delà.

- [ ] **Étape 4 : ajouter l'interrupteur global et le transmettre**

`PLEIN_ECRAN=0` désarme le mécanisme entier — lecture de style comprise. Même
forme qu'`AUDIO=0` en D7, et **avec sa ligne dans `scripts/run-agent.sh` dans
cette tâche**, sans quoi un agent lancé à la main ne pourra pas l'employer :
c'est exactement le défaut n°1 trouvé par la recette D7.

- [ ] **Étape 5 : vérifier**

```bash
cd agent && cargo test && cargo check --target x86_64-pc-windows-gnu
```

- [ ] **Étape 6 : corriger le commentaire que D8 réfute**

`agent/src/windows_source/redimensionnement.rs` porte aujourd'hui :

> « la spec §3.3 acte que le redimensionnement d'une fenêtre déjà ouverte est
> hors périmètre de D1 (le pilote SudoVDA n'expose aucun `SET_MODE`, la sortie
> ne peut donc pas suivre) »

La prémisse reste **vraie** (le pilote n'a effectivement aucun `SET_MODE`), la
conclusion ne l'est plus. **Ne pas supprimer la phrase : y inscrire la
réfutation**, avec la mesure qui l'établit. C'est la leçon de D6 — « corriger une
affirmation fausse peut en produire une autre » —, et le remède qui a tenu est
d'inscrire dans le commentaire ce qui permet au prochain lecteur de refaire le
contrôle sans croire personne.

- [ ] **Étape 7 : commit**

```bash
git add agent/src/windows_source/redimensionnement.rs \
        agent/src/windows_source/sortie.rs scripts/run-agent.sh
git commit -m "feat(d8): la sortie virtuelle suit le viewport, par l API d affichage"
```

---

## Tâche 10 : le pilote de recette

**Fichiers :**
- Créer : `docs/superpowers/plans/journaux-multifenetres-d8/instrument/pilote-recette-d8.mjs`

**Interfaces :**
- Consomme : le verdict de P2 (tâche 1), qui décide du montage ; les critères du
  §8 de la spec.
- Produit : un pilote qui joue les cinq critères et écrit un journal par phase.

- [ ] **Étape 1 : partir du pilote de D6**

`docs/superpowers/plans/journaux-multifenetres-d6/instrument/pilote-recette-d6.mjs`
est le point de départ : il sait ouvrir N fenêtres, imposer la visibilité et le
focus page par page, et relever `getStats()`. **Le copier, ne pas le modifier en
place** — les journaux de D6 y renvoient.

- [ ] **Étape 2 : reprendre les six garde-fous que les recettes précédentes ont payés**

1. **Un `--user-data-dir` par fenêtre**, sans quoi Chrome rejoint son instance
   existante et l'on compte des lancements au lieu de fenêtres (D4).
2. **Imposer la visibilité page par page** : un Chrome sans interface rapporte
   `document.hidden = true` pour toute fenêtre d'arrière-plan (D5).
   `Page.addScriptToEvaluateOnNewDocument` **ne court pas** sur une page ouverte
   par `window.open` — poser l'override explicitement.
3. **Faire passer une cible de focus par `blur` PUIS `focus`** : la
   déduplication de `client/src/visibilite.ts` peut sinon faire disparaître le
   focus entièrement (D6).
4. **Aucune capture d'écran CDP pendant une mesure** : elle provoque un
   `Resize`, donc un `SHOW`, donc une session et une sortie de plus (D1). Et
   toute évaluation CDP sur une page portant un flux WebRTC actif doit être
   **bornée** — `Page.captureScreenshot` peut ne jamais rendre (D2).
5. **Un palier plusieurs fois plus long que la temporisation observée** :
   `DELAI_REMONTEE` vaut 20 s (`agent/src/congestion/hysteresis.rs`). **Aucun
   palier de D8 ne descend sous 60 s** — c'est la consignation n°3 de D6, et
   trois mesures y ont été perdues.
6. **Attendre le FAIT, jamais une durée** : douze secondes sans changement de
   barreau avant de mesurer, plutôt qu'un `sleep` arbitraire.

- [ ] **Étape 3 : lire le bandeau au bon endroit**

Le bandeau des pages d'application est `#status` ; `#statut` est celui de la
page-shell. Une exécution entière de D5 a lu le mauvais et n'a rien vu. Et
`#status` **garde son texte une fois masqué** : le lire prouve qu'un message est
arrivé, **pas qu'il était affiché**.

- [ ] **Étape 4 : instrumenter le critère ③ par le titre Windows**

La page `--app` réécrit son titre sur `keydown` ; le pilote relit le titre de la
fenêtre Windows par `WM_GETTEXT` via `scripts/winrm.js`. Réécrire un titre est
sans effet sur le produit depuis D7 — l'identité d'une fenêtre vit dans son nom
de fichier (`92ea675`).

⚠️ **`nodejs-winrm` enveloppe TOUJOURS la commande** dans
`powershell -Command "& { … }"`. Un script inline portant des guillemets doubles
entre en collision avec cette enveloppe, **et le symptôme est un script qui ne
tourne jamais**, pas une erreur claire. Écrire le script sur le partage et
l'invoquer par `-File`.

- [ ] **Étape 5 : instrumenter le critère ⑤ par la fréquence dominante**

Reprendre l'`AnalyserNode` du pilote de D7 — **jamais un compte d'octets** : D7 a
relevé un `bytesReceived` qui croît sur un spectre à −1000 dB.

- [ ] **Étape 6 : commit**

```bash
git add docs/superpowers/plans/journaux-multifenetres-d8/instrument/pilote-recette-d8.mjs
git commit -m "test(d8): le pilote de recette, et les six garde-fous deja payes"
```

---

## Tâche 11 : la recette

**Fichiers :**
- Créer : `docs/superpowers/plans/journaux-multifenetres-d8/critere-{1..5}-*.log`
- Créer : `docs/superpowers/plans/journaux-multifenetres-d8/temoin-sans-plein-ecran.log`
- Créer : `docs/superpowers/plans/2026-08-04-multifenetres-plein-ecran-resultats.md`

**Interfaces :**
- Consomme : tout ce qui précède.
- Produit : les cinq verdicts, et leurs pièces.

- [ ] **Étape 1 : préparer la VM et compiler**

```bash
virsh list --all && virsh start Windows
until timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null; do sleep 5; done
until ls /media/vm/dev >/dev/null 2>&1; do sleep 5; done
set -a && source .env && set +a
scripts/build-agent.sh
ls -l /media/vm/…/agent.exe    # VÉRIFIER LA TAILLE
node scripts/winrm.js "Get-Process agent"           # aucun survivant
MULTIFENETRE_VDD_PURGE=1 scripts/run-agent.sh       # purger les orphelines
```

- [ ] **Étape 2 : jouer les cinq critères, un journal par phase**

| # | Critère | Ce qui le juge |
| --- | --- | --- |
| ① | Une application passe en plein écran, sa fenêtre navigateur y entre au geste suivant, **et elle seule** | `document.fullscreenElement` non nul sur la bonne page, nul sur les autres |
| ② | Le flux passe à la taille du viewport plein écran, bornée par `TAILLE_MAX_SORTIE`, **et la sortie garde son nom** | `frameWidth`/`frameHeight` de `getStats()` croisés avec `GetDesc`. ⚠️ Conditionné à **P1** |
| ③ | Échap ne casse pas le plein écran et atteint Windows | `fullscreenElement` non nul **et** le titre relu par `WM_GETTEXT`. ⚠️ Conditionné à **P2** |
| ④ | Les voisines s'endorment par le chemin existant | Les traces `Sommeil` du capteur, et `endormie=true` sur leurs parts |
| ⑤ | L'audio d'une endormie survit | La **fréquence dominante** par `AnalyserNode` |

Plus le **témoin de non-régression** : une phase à N fenêtres sans aucun plein
écran, qui doit se comporter comme D7.

- [ ] **Étape 3 : après CHAQUE rang, contrôler la survie de la VM**

```bash
grep -E "terminating on signal|shutting down" /var/log/libvirt/qemu/Windows.log | tail -4
```
La VM s'hiberne d'elle-même, déclencheur non identifié. Une mesure prise sur une
VM morte se lit comme une mesure prise.

- [ ] **Étape 4 : copier `agent.log` APRÈS la fin réelle de l'exécution**

Pas à la fin du pilote : les enfants meurent quand le navigateur se ferme, donc
**après** la copie, et leurs lignes de libération partiraient avec le journal
suivant. Une pièce a été perdue ainsi en D4.

- [ ] **Étape 5 : écrire le document de résultats**

`docs/superpowers/plans/2026-08-04-multifenetres-plein-ecran-resultats.md`.
**Chaque énoncé porte son nombre d'exécutions** — le mode de défaillance dominant
de ce dépôt est l'énoncé qui affirme au-delà de son relevé, pas le code. Un
sous-ensemble sans règle de sélection énoncée est un sous-ensemble **choisi**,
même quand on ne l'a pas choisi.

Y inscrire aussi, explicitement :
- **si ④ est réfuté** : le relever et le documenter — **ne pas** coder une
  seconde autorité sur le sommeil ;
- **si `Xvfb` a été installé** : que les mesures ne se comparent à **aucune**
  campagne antérieure ;
- le compte des pertes de mutex `0x887a0026` sur les **voisines** pendant un
  changement de mode — le §6 de la spec dit que rien n'est su de ce point, et
  cette recette est ce qui le saura.

- [ ] **Étape 6 : commit**

```bash
git add docs/superpowers/plans/journaux-multifenetres-d8/ \
        docs/superpowers/plans/2026-08-04-multifenetres-plein-ecran-resultats.md
git commit -m "recette(d8): le plein ecran, mesure sur cinq criteres"
```

---

## Tâche 12 : l'index durable

**Fichiers :**
- Modifier : `CLAUDE.md`

- [ ] **Étape 1 : relever les tailles PAR LA COMMANDE**

```bash
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | head -25
```

**Ne recopier aucun nombre d'un tableau existant.** Ce fichier a payé quatre fois
cette leçon — le « 487 » de `capteur/distante.rs`, le 119/138 de
`repartiteur.rs`/`part.rs`, les quatre chiffres du tableau D6, et les deux deltas
de la vague de correction de D7.

- [ ] **Étape 2 : écrire la section D8**

Sur le modèle des sections D5, D6 et D7 : le verdict, ce que le code livre, ce
que la recette n'établit pas, les pièges neufs, et ce que D8 lègue.

**Ce qui doit y figurer sans faute** :
- **le critère de détection du §4.1 est mort**, et pourquoi — c'est le fait de
  conception le plus réutilisable du sous-bloc, et il concerne quiconque relira
  le cadrage jeux ;
- le verdict de P0, qui est **la seule mesure de F1** jamais prise ;
- **corriger l'annotation de `SOURCE_TRACE`** si le numéro de ligne de
  `demarrage.rs` a bougé — elle porte déjà la mention « valeur non revérifiée » ;
- si P1 a été refusé, **corriger la phrase de ce plan et de la spec** qui suppose
  le contraire, à l'endroit où elle dort et pas seulement là où on l'a vue.

- [ ] **Étape 3 : mettre à jour la liste des legs**

D8 ne traite ni F3 (signal enfant→capteur), ni F5 (génération monotone), ni les
trois consignations de D6 restantes. **Les reporter, ne pas les laisser
disparaître** : le §11 de la spec les nomme.

- [ ] **Étape 4 : commit**

```bash
git add CLAUDE.md
git commit -m "docs(d8): l index durable, sur des chiffres releves par la commande"
```

---

## Auto-revue du plan

**Couverture de la spec** — chaque section a sa tâche :

| Spec | Tâche |
| --- | --- |
| §4 P0 | 2 |
| §4 P1 | 3, et le repli en 9 |
| §4 P2 | 1 |
| §5.1 prédicat pur | 4 |
| §5.2 état de référence | 4 (deux tests dédiés) |
| §5.3 cadence, pas d'hystérésis | 6 |
| §5.4 les deux messages | 5 et 7 |
| §6 changement de mode, `TAILLE_MAX_SORTIE` | 9 |
| §7.1 armement | 8 |
| §7.2 Keyboard Lock : rien à écrire | aucune tâche — **et c'est juste**, `attachFullscreen` verrouille déjà sur `fullscreenchange` |
| §7.3 deux entrées, un seul sens | 8 (le bouton reste, aucune tâche ne le retire) |
| §8 recette | 10 et 11 |
| §9 risques | portés dans les tâches conditionnées (3→9, 1→11) |
| §10 réserves | 11 étape 5 |
| §11 legs | 12 étape 3 |

**Un point de la spec sans tâche, et c'est délibéré** : le §7.2 ne demande aucun
code. Le noter plutôt que d'inventer une tâche vide.

**Cohérence des types** — vérifiée d'un bout à l'autre :
`DepuisCapteur::PleinEcran { actif: bool }` (T5) →
`Recu::PleinEcran { actif: bool }` (T5) →
`plein_ecran_a_annoncer() -> Option<bool>` (T5) →
`AgentControl::fullscreen(active: bool)` (T7) →
`FullscreenMessage.active: boolean` (T7) →
`message.type === 'fullscreen'` (T8). Le champ Rust interne s'appelle `actif`,
le champ **du fil** s'appelle `active` — c'est la convention déjà en place
(`Asleep { asleep }`, `Pointer { visible }`), pas une incohérence.
`est_sans_bordure`/`SuiviBordure` (T4) sont consommés sous ces noms exacts en T6 ;
`borner_a_la_taille_max`/`TAILLE_MAX_SORTIE` (T9) ne sont consommés que dans T9.

**Deux `todo!` assumés, et signalés comme tels** : le corps de
`changer_mode_de_sortie` (T9 étape 3) et les trois noms de fonctions de
`moniteurs_virtuels` (T3 étape 1). Les deux dépendent de ce que la mesure P1
établira ou de ce que le code existant expose réellement — les inventer ici
produirait un plan qui a l'air complet et ne compile pas. **Chacun porte
l'instruction de ce qu'il faut aller lire.**
