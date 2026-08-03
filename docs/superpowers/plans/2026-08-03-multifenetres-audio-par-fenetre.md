# Sous-bloc D7 — l'audio par fenêtre : plan d'implémentation

> **Pour les agents d'exécution :** SOUS-COMPÉTENCE REQUISE — employer
> `superpowers:subagent-driven-development` (recommandé) ou
> `superpowers:executing-plans` pour exécuter ce plan tâche par tâche. Les
> étapes sont en cases à cocher (`- [ ]`).

**But** : chaque fenêtre porte le son de **son** application, et de rien
d'autre.

**Architecture** : la capture vit dans l'**enfant** (`ActivateAudioInterfaceAsync`
avec `AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK` sur le PID propriétaire de
son `FENETRE_HWND`) ; l'arbitrage vit dans le **capteur**, où le focus est déjà
tenu ; un seul message neuf, `DepuisCapteur::Audio { actif }`, circule sur le
canal de commandes existant. **Aucun octet audio ne traverse le tube.**

**Pile** : Rust 2021, crate `windows` 0.62, `str0m`, `opus`. Client TypeScript +
Vite. Compilation distante sur la VM Windows par `scripts/build-agent.sh`.

**Spécification** :
`docs/superpowers/specs/2026-08-03-multifenetres-audio-par-fenetre-design.md`

## Contraintes globales

- **Un fichier de code source ne dépasse pas 500 lignes.** Toute addition
  substantielle à un fichier déjà proche du plafond s'accompagne d'une
  **extraction**, jamais d'une compression. Vérifier par la commande de
  `CLAUDE.md`, **jamais** en recopiant un nombre d'un document.
- **`cargo check --target x86_64-pc-windows-gnu` depuis `agent/` AVANT toute
  compilation distante.** Il couvre types, emprunts, visibilités et durées de
  vie ; **pas** l'édition de liens.
- **`cargo test` depuis `agent/` doit rester vert** à chaque commit.
- **Sourcer `.env` avant `scripts/build-agent.sh`** : sans cela il s'arrête **en
  silence** après « sources synchronisées », et l'on mesure le binaire
  précédent.
- **`git add` nominatif, jamais `git add -A`** : l'arbre est partagé.
- **Toute variable d'environnement neuve est ajoutée à `scripts/run-agent.sh`**
  dans le même commit que le code qui la lit. Piège payé en D1 et D2.
- **Ne jamais tracer par paquet ni par image** dans une boucle chaude.
- Messages de commit en français, sans accents dans le sujet (convention du
  dépôt), terminés par `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`.

---

## Structure de fichiers

| Fichier | Responsabilité | Tâche |
| --- | --- | --- |
| **Créer** `agent/src/wasapi/process_loopback.rs` | Toute la machinerie COM du *process loopback* : activation asynchrone, gestionnaire de complétion, ouverture d'une capture par PID | 1, 2 |
| **Créer** `agent/src/capteur/audio.rs` | **La règle pure** : qui porte le son. Aucun `cfg`, aucun COM, entièrement testé sur l'hôte | 4 |
| **Créer** `agent/src/capteur/sommeil/porteurs.rs` | **La branche** de la règle sur le registre : calcul, filtre d'écrasement, envoi | 5 |
| **Modifier** `agent/src/wasapi.rs` | Perd ~180 lignes au profit de son enfant ; garde `LoopbackCapture` et `open()` | 1 |
| **Modifier** `agent/src/capteur/sommeil.rs` | Le registre gagne `pids`, `arrivees`, `derniers_focus`, `derniers_audio`, et `Message::Audio` | 5 |
| **Modifier** `agent/src/capteur/fenetre.rs` | Dérive le PID du `hwnd` et le passe à `inscrire` ; pose le span `tracing` | 5, 10 |
| **Modifier** `agent/src/capteur/fenetre/transitions.rs` | Relaie `Message::Audio` en `DepuisCapteur::Audio` | 6 |
| **Modifier** `agent/src/capteur/protocole.rs` | `DepuisCapteur::Audio { actif: bool }` | 6 |
| **Modifier** `agent/src/capteur/distante.rs` | `Recu::Audio`, champ `audio`, `audio_a_appliquer()` | 6 |
| **Modifier** `agent/src/source.rs` | Méthode de trait `audio_a_appliquer()`, sans effet par défaut | 6 |
| **Modifier** `agent/src/transport/tick.rs` | Branche `a1quinquies` qui consomme l'ordre audio | 6 |
| **Modifier** `agent/src/transport/piste_audio.rs` | `appliquer_audio()` : la source se tait ET le budget audio tombe à zéro | 6, 7 |
| **Modifier** `agent/src/audio.rs` | Méthode de trait `AudioSource::set_actif()`, sans effet par défaut | 7 |
| **Modifier** `agent/src/windows_audio.rs` | `pour_processus(pid, origin)`, `AtomicBool` d'émission, trace `pid=`/`actif=` | 7 |
| **Modifier** `agent/src/demarrage.rs` | Choisit `pour_processus` ou `open()` selon `FENETRE_HWND` | 7 |
| **Modifier** `agent/src/congestion/controleur.rs` | `changer_audio_bps()` | 8 |
| **Modifier** `agent/src/superviseur/table.rs`, `enfants.rs`, `lanceur.rs`, `boucle.rs` | Suppression de `audio` / `audio_libre` | 9 |
| **Modifier** `agent/src/main.rs` | `AUDIO` redevient un interrupteur global | 9 |
| **Modifier** `agent/src/diagnostics/audio.rs`, `agent/src/diagnostics.rs` | Sonde `PROCESS_LOOPBACK_CAPTURE` | 1, 2 |
| **Modifier** `CLAUDE.md` | Dette, marges et section D7, sur des chiffres relevés | 12 |
| **Créer** `docs/superpowers/plans/journaux-multifenetres-d7/` | Journaux versés, instrument, résultats | 3, 11, 13 |

---

## Tâche 1 : extraire la machinerie *process loopback* de `wasapi.rs`

Refactor **pur**, aucun changement de comportement. Il paie la règle des
500 lignes **avant** d'ajouter quoi que ce soit : `wasapi.rs` est à **543 lignes,
dette gelée**.

**Fichiers**
- Créer : `agent/src/wasapi/process_loopback.rs`
- Modifier : `agent/src/wasapi.rs`

**Interfaces**
- Produit : `crate::wasapi::process_loopback::probe_process_loopback(pid: u32) -> anyhow::Result<String>` — même signature qu'aujourd'hui, nouveau chemin de module.

- [ ] **Étape 1 : relever la taille réelle, par la commande et non par un document**

```bash
cd /home/mallanic/Projects/Guacamole && { git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>400'
```

Noter les chiffres obtenus. Ils serviront à la tâche 12 (mise à jour de
`CLAUDE.md`), et **aucun autre chiffre ne doit être recopié d'un document**.

- [ ] **Étape 2 : créer le module enfant avec le contenu déplacé**

Déplacer **textuellement**, depuis `agent/src/wasapi.rs`, tout le bloc allant du
commentaire `// ---` « Sonde n°4 de la spec du chantier A (§11) » jusqu'à la fin
de `probe_process_loopback` (aujourd'hui l. 313 à 493) vers
`agent/src/wasapi/process_loopback.rs`, précédé de cet en-tête et des `use`
nécessaires :

```rust
//! Le *process loopback* : capter l'audio d'un seul processus, et de son arbre.
//!
//! **Extrait de `wasapi.rs` et non ajouté dedans.** Ce fichier-là est à
//! 543 lignes, `#[cfg(windows)]`, sans aucun test : c'est de la dette gelée au
//! sens de `CLAUDE.md`, et la règle du dépôt veut qu'une addition
//! substantielle s'y accompagne d'une extraction. Toute la machinerie COM
//! asynchrone d'`ActivateAudioInterfaceAsync` vit donc ici, où le code du
//! sous-bloc D7 a sa place.
//!
//! **Ce que cette API a de particulier** : elle n'est pas synchrone. Le
//! résultat n'arrive pas en retour d'appel mais par
//! `IActivateAudioInterfaceCompletionHandler::ActivateCompleted`, invoqué
//! depuis un fil du pool COM — d'où l'état partagé et la `Condvar` ci-dessous.

#![cfg(windows)]

use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use windows::core::{implement, Interface, Ref};
use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::Media::Audio::{
    ActivateAudioInterfaceAsync, IActivateAudioInterfaceAsyncOperation,
    IActivateAudioInterfaceCompletionHandler, IActivateAudioInterfaceCompletionHandler_Impl,
    IAudioClient, AUDIOCLIENT_ACTIVATION_PARAMS, AUDIOCLIENT_ACTIVATION_PARAMS_0,
    AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK, AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS,
    PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE, VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
};
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Com::{CoInitializeEx, BLOB, COINIT_MULTITHREADED};
use windows::Win32::System::Variant::VT_BLOB;
```

**Ne rien réécrire du corps déplacé.** En particulier, conserver mot pour mot le
commentaire `CORRECTIF (revue)` sur le `ManuallyDrop<PROPVARIANT>` : il
documente une corruption mémoire réelle (libération d'une adresse de **pile** par
`PropVariantClear`), et sa disparition rouvrirait le défaut.

- [ ] **Étape 3 : déclarer le module et retirer les `use` devenus inutiles**

Dans `agent/src/wasapi.rs`, juste après le `#![cfg(windows)]` de tête :

```rust
pub mod process_loopback;
```

Puis retirer de son bloc `use` les symboles qui ne servent plus qu'à l'enfant :
`ActivateAudioInterfaceAsync`, `IActivateAudioInterfaceAsyncOperation`,
`IActivateAudioInterfaceCompletionHandler`,
`IActivateAudioInterfaceCompletionHandler_Impl`, `AUDIOCLIENT_ACTIVATION_PARAMS`,
`AUDIOCLIENT_ACTIVATION_PARAMS_0`, `AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK`,
`AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS`,
`PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE`,
`VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK`, `PROPVARIANT`, `BLOB`, `VT_BLOB`,
`implement`, `Ref`, `Condvar`, `Arc`, `Mutex` — **au cas par cas**, en se fiant
aux avertissements du compilateur et non à cette liste.

- [ ] **Étape 4 : corriger l'appelant**

Dans `agent/src/diagnostics/audio.rs`, l. 49 :

```rust
    match wasapi::process_loopback::probe_process_loopback(pid) {
```

- [ ] **Étape 5 : vérifier que rien n'a bougé**

```bash
cd agent && cargo check --target x86_64-pc-windows-gnu 2>&1 | tail -20 && cargo test 2>&1 | tail -10
```

Attendu : `Finished` sans erreur sur la cible Windows, et les tests d'hôte tous
verts. **Les avertissements `dead_code` dus au `#[cfg(windows)]` sont normaux et
leur nombre dérive d'une exécution à l'autre — vérifier leur nature, jamais leur
nombre.**

- [ ] **Étape 6 : relever la taille obtenue et commettre**

```bash
wc -l agent/src/wasapi.rs agent/src/wasapi/process_loopback.rs
git add agent/src/wasapi.rs agent/src/wasapi/process_loopback.rs agent/src/diagnostics/audio.rs
git commit -F - <<'EOF'
refactor(d7): la machinerie process loopback sort de wasapi.rs

wasapi.rs etait a 543 lignes, cfg(windows), sans aucun test : dette gelee au
sens de CLAUDE.md. La regle du depot veut qu'une addition substantielle s'y
accompagne d'une extraction, et D7 va y ajouter la capture par processus.

Deplacement textuel, aucun changement de comportement. Le commentaire du
ManuallyDrop<PROPVARIANT> est conserve mot pour mot : il documente une
corruption memoire reelle, et sa disparition rouvrirait le defaut.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
```

---

## Tâche 2 : la capture par processus, et la sonde qui la mesurera

**Fichiers**
- Modifier : `agent/src/wasapi/process_loopback.rs`
- Modifier : `agent/src/diagnostics/audio.rs`, `agent/src/diagnostics.rs`
- Modifier : `scripts/run-agent.sh`

**Interfaces**
- Consomme : `crate::opus::{CHANNELS, SAMPLE_RATE_HZ}`, `crate::wasapi::LoopbackCapture`.
- Produit :
  - `process_loopback::CaptureProcessus` avec
    `ouvrir(pid: u32) -> Result<Self>`,
    `read(&mut self) -> Result<Option<Vec<i16>>>`,
    `description(&self) -> &str`,
    `demarrer(&mut self) -> Result<()>`,
    `arreter(&mut self) -> Result<()>`.
  - `unsafe impl Send for CaptureProcessus {}`

⚠️ **Deux divergences connues avec `LoopbackCapture::open`, à traiter comme des
inconnues et non comme des acquis** — la tâche 3 les tranchera sur `HRESULT` :

1. **`GetMixFormat` peut refuser** sur un client de *process loopback* : il
   n'est lié à aucun point de terminaison de périphérique. Le code pose donc
   **explicitement** le format qu'`opus.rs` veut déjà, plutôt que de le
   demander.
2. **`AUDCLNT_STREAMFLAGS_EVENTCALLBACK`** : l'échantillon officiel de Microsoft
   l'emploie pour cette API, là où notre doctrine de loopback est le **sondage**
   (voir l'en-tête de `wasapi.rs`). On garde le sondage ; si `Initialize` refuse
   sans ce drapeau, **c'est un relevé de la tâche 3**, pas une supposition à
   corriger d'avance.

- [ ] **Étape 1 : écrire la capture par processus**

Ajouter à la fin de `agent/src/wasapi/process_loopback.rs` :

```rust
/// Durée du tampon demandé, en unités de 100 ns. 200 ms, comme
/// `LoopbackCapture::open` : large marge pour absorber un tour de boucle en
/// retard sans perdre d'échantillon.
const DUREE_TAMPON_100NS: i64 = 2_000_000;

/// Capture loopback d'un seul processus et de son arbre.
///
/// **Le format n'est pas demandé, il est IMPOSÉ.** Un client de *process
/// loopback* n'est lié à aucun point de terminaison : `GetMixFormat` n'y a pas
/// de sens évident, et l'échantillon officiel de Microsoft pose lui aussi un
/// format explicite. On pose donc exactement celui qu'`opus.rs` attend
/// (48 kHz, 2 canaux, 16 bits entiers), ce qui supprime du même coup toute
/// conversion : `read` rend des `i16` entrelacés directement exploitables par
/// `FrameAssembler`.
///
/// ⚠️ **Que Windows accepte ce format n'est pas établi avant la mesure de la
/// tâche 3.** S'il refuse, le `HRESULT` exact est le relevé qui compte — ne pas
/// deviner un repli.
pub struct CaptureProcessus {
    client: IAudioClient,
    capture: IAudioCaptureClient,
    description: String,
    /// Vrai quand `Start()` a été appelé sans `Stop()` depuis. **Nécessaire** :
    /// `IAudioClient::Start` sur un flux déjà démarré rend
    /// `AUDCLNT_E_NOT_STOPPED`, et l'arbitrage peut réémettre un ordre
    /// identique après un rattachement de canal.
    demarre: bool,
}

// SÉCURITÉ : même raisonnement que `unsafe impl Send for LoopbackCapture`
// (`wasapi.rs`, dont il faut lire le commentaire d'abord). `ouvrir` vérifie que
// le fil appelant est membre de la MTA et refuse `RPC_E_CHANGED_MODE` ; le fil
// de capture de `windows_audio.rs` rejoint cette même MTA avant tout appel COM.
// **La promesse porte sur le struct entier, champs futurs compris.**
unsafe impl Send for CaptureProcessus {}

impl CaptureProcessus {
    pub fn ouvrir(pid: u32) -> Result<Self> {
        let client = activer_pour_processus(pid)?;
        unsafe {
            let mut format = WAVEFORMATEX {
                wFormatTag: WAVE_FORMAT_PCM as u16,
                nChannels: CHANNELS as u16,
                nSamplesPerSec: SAMPLE_RATE_HZ,
                wBitsPerSample: 16,
                nBlockAlign: (CHANNELS as u16) * 2,
                nAvgBytesPerSec: SAMPLE_RATE_HZ * (CHANNELS as u32) * 2,
                cbSize: 0,
            };
            format.nBlockAlign = format.nChannels * format.wBitsPerSample / 8;
            format.nAvgBytesPerSec = format.nSamplesPerSec * format.nBlockAlign as u32;

            client
                .Initialize(
                    AUDCLNT_SHAREMODE_SHARED,
                    AUDCLNT_STREAMFLAGS_LOOPBACK,
                    DUREE_TAMPON_100NS,
                    0,
                    &format,
                    None,
                )
                .context(
                    "Initialize du client de process loopback (format impose : 48 kHz, \
                     2 canaux, 16 bits)",
                )?;

            let capture: IAudioCaptureClient = client
                .GetService()
                .context("GetService(IAudioCaptureClient) sur le client de process loopback")?;

            let description = format!(
                "process loopback pid={pid} — {} Hz, {} canaux, 16 bits entiers",
                format.nSamplesPerSec, format.nChannels
            );

            Ok(Self { client, capture, description, demarre: false })
        }
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    /// Démarre le flux. Idempotent : un second appel ne fait rien.
    pub fn demarrer(&mut self) -> Result<()> {
        if self.demarre {
            return Ok(());
        }
        unsafe { self.client.Start() }.context("Start du client de process loopback")?;
        self.demarre = true;
        Ok(())
    }

    /// Arrête le flux. Idempotent.
    ///
    /// **Ce n'est pas une destruction** : le client reste activé, donc aucune
    /// réactivation COM — la seule étape qui puisse refuser — n'a lieu à la
    /// bascule suivante. C'est tout l'intérêt de l'approche retenue au §4.4 de
    /// la spec.
    pub fn arreter(&mut self) -> Result<()> {
        if !self.demarre {
            return Ok(());
        }
        unsafe { self.client.Stop() }.context("Stop du client de process loopback")?;
        self.demarre = false;
        Ok(())
    }

    /// Lit un paquet, ou `None` s'il n'y en a aucun de prêt.
    ///
    /// Rend des `i16` entrelacés, sans conversion : le format est imposé à
    /// l'ouverture.
    pub fn read(&mut self) -> Result<Option<Vec<i16>>> {
        unsafe {
            let disponibles = self
                .capture
                .GetNextPacketSize()
                .context("GetNextPacketSize sur le process loopback")?;
            if disponibles == 0 {
                return Ok(None);
            }

            let mut donnees = std::ptr::null_mut();
            let mut images = 0u32;
            let mut drapeaux = 0u32;
            self.capture
                .GetBuffer(&mut donnees, &mut images, &mut drapeaux, None, None)
                .context("GetBuffer sur le process loopback")?;

            let echantillons = images as usize * CHANNELS;
            let sortie = if drapeaux & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 {
                // Le drapeau SILENT autorise le pilote à ne pas remplir le
                // tampon : lire ses octets rendrait n'importe quoi.
                vec![0i16; echantillons]
            } else {
                std::slice::from_raw_parts(donnees as *const i16, echantillons).to_vec()
            };

            self.capture
                .ReleaseBuffer(images)
                .context("ReleaseBuffer sur le process loopback")?;
            Ok(Some(sortie))
        }
    }
}

impl Drop for CaptureProcessus {
    fn drop(&mut self) {
        unsafe {
            let _ = self.client.Stop();
        }
    }
}

/// Active un `IAudioClient` de *process loopback* et l'attend.
///
/// Extrait de `probe_process_loopback`, qui l'appelle désormais : la sonde et
/// la capture réelle doivent activer **exactement de la même façon**, sans quoi
/// la sonde ne mesurerait pas ce que le produit fait.
fn activer_pour_processus(pid: u32) -> Result<IAudioClient> {
    // ... (corps repris de `probe_process_loopback`, qui rend `IAudioClient`
    //      au lieu d'une `String` — voir l'étape 2)
    unimplemented!()
}
```

⚠️ **Le `unimplemented!()` ci-dessus n'est pas une lacune de ce plan : c'est le
sujet de l'étape 2, qui suit immédiatement.** Il ne doit jamais être commis.

Compléter le bloc `use` du fichier :

```rust
use windows::Win32::Media::Audio::{
    IAudioCaptureClient, AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED,
    AUDCLNT_STREAMFLAGS_LOOPBACK, WAVEFORMATEX,
};
use windows::Win32::Media::Multimedia::WAVE_FORMAT_PCM;

use crate::opus::{CHANNELS, SAMPLE_RATE_HZ};
```

- [ ] **Étape 2 : factoriser l'activation, pour que la sonde et le produit activent pareil**

Découper l'actuelle `probe_process_loopback` en deux : `activer_pour_processus`
qui rend l'`IAudioClient`, et `probe_process_loopback` qui l'appelle et rend sa
description. Le corps de `activer_pour_processus` est **exactement** celui de
l'actuelle sonde, jusqu'au `match verrou.take()`, dont les trois bras
deviennent :

```rust
        match verrou.take() {
            Some(ResultatActivation(Ok(client))) => Ok(client),
            Some(ResultatActivation(Err(e))) => Err(e).context("activation refusée"),
            None => bail!(
                "aucun rappel d'activation reçu en {DELAI_RAPPEL_ACTIVATION:?} pour le PID {pid}"
            ),
        }
```

et `probe_process_loopback` devient :

```rust
/// Sonde d'ACTIVATION seule — conservée telle quelle pour que le relevé du
/// chantier A (28 juillet 2026) reste reproductible à l'identique. Elle
/// n'initialise rien et ne lit aucun octet : c'est `CaptureProcessus::ouvrir`
/// qui va plus loin, et c'est `PROCESS_LOOPBACK_CAPTURE` qui le mesure.
pub fn probe_process_loopback(pid: u32) -> Result<String> {
    let _client = activer_pour_processus(pid)?;
    Ok(format!(
        "activation réussie : IAudioClient obtenu pour le PID {pid} \
         (VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK, INCLUDE_TARGET_PROCESS_TREE)"
    ))
}
```

- [ ] **Étape 3 : écrire la sonde de capture**

Ajouter à `agent/src/diagnostics/audio.rs` :

```rust
/// `PROCESS_LOOPBACK_CAPTURE=<pid>` — la mesure pivot du sous-bloc D7.
///
/// Va jusqu'où `probe_process_loopback` s'arrête : `Initialize`,
/// `GetService`, `Start`, et une lecture réelle. **Le relevé qui compte n'est
/// pas la crête non nulle** — une capture qui rendrait en réalité le mix global
/// la produirait aussi — **mais la crête NULLE pendant qu'un autre processus
/// joue.** Les deux moitiés se jouent par deux exécutions successives, et le
/// protocole est au §3 de la conception.
pub(super) fn executer_capture_process_loopback(pid_texte: &str) -> Result<()> {
    let pid: u32 = pid_texte
        .parse()
        .context("PROCESS_LOOPBACK_CAPTURE doit être un identifiant de processus")?;
    let secondes: u64 = std::env::var("PROCESS_LOOPBACK_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(15);

    let mut capture = wasapi::process_loopback::CaptureProcessus::ouvrir(pid)?;
    tracing::info!(pid, format = %capture.description(), "process loopback ouvert");
    capture.demarrer()?;

    let debut = std::time::Instant::now();
    let mut echantillons = 0u64;
    let mut crete = 0i16;
    let mut lectures_vides = 0u64;
    while debut.elapsed() < std::time::Duration::from_secs(secondes) {
        match capture.read()? {
            Some(bloc) => {
                echantillons += bloc.len() as u64;
                for v in bloc {
                    crete = crete.max(v.saturating_abs());
                }
            }
            None => lectures_vides += 1,
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    tracing::info!(
        pid,
        echantillons,
        lectures_vides,
        crete,
        silencieux = crete == 0,
        "sonde de capture process loopback : premiere moitie"
    );

    // Le cycle Stop/Start, dont dépend l'approche retenue au §4.4 de la spec.
    // Un refus ici fait replier sur l'approche B — c'est un relevé, pas un
    // incident.
    capture.arreter()?;
    std::thread::sleep(std::time::Duration::from_millis(500));
    match capture.demarrer() {
        Ok(()) => tracing::info!(pid, "cycle Stop puis Start accepte"),
        Err(e) => tracing::warn!(pid, erreur = %e, "cycle Stop puis Start REFUSE"),
    }

    let debut = std::time::Instant::now();
    let mut crete_apres = 0i16;
    while debut.elapsed() < std::time::Duration::from_secs(5) {
        if let Some(bloc) = capture.read()? {
            for v in bloc {
                crete_apres = crete_apres.max(v.saturating_abs());
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    tracing::info!(pid, crete_apres, "sonde de capture process loopback terminee");
    Ok(())
}
```

- [ ] **Étape 4 : brancher la variable d'aiguillage**

Dans `agent/src/diagnostics.rs`, ajouter le bras — **avant** celui de
`PROCESS_LOOPBACK_PROBE`, les deux variables partageant un préfixe (le piège
exact de `MULTIFENETRE_NVENC_CYCLES` en D5) :

```rust
    if let Ok(pid) = std::env::var("PROCESS_LOOPBACK_CAPTURE") {
        audio::executer_capture_process_loopback(&pid)?;
        return Ok(true);
    }
```

Reproduire la forme exacte du bras voisin (valeur de retour, `return`) : la
lire dans le fichier, ne pas l'inventer.

- [ ] **Étape 5 : transmettre les deux variables neuves**

Dans `scripts/run-agent.sh`, à côté des autres `$env:` déjà posées, ajouter
`PROCESS_LOOPBACK_CAPTURE` et `PROCESS_LOOPBACK_SECS` **selon la forme
conditionnelle employée par les variables voisines** (les lire dans le fichier).

**C'est le piège payé en D1 (`SUPERVISEUR`) et D2 (`MULTIFENETRE_REPRISE`)** :
sans cette ligne, l'agent démarre sans la variable et **sans rien signaler**.

- [ ] **Étape 6 : vérifier**

```bash
cd agent && cargo check --target x86_64-pc-windows-gnu 2>&1 | tail -20 && cargo test 2>&1 | tail -5
```

Attendu : compilation Windows sans erreur, tests d'hôte verts.

- [ ] **Étape 7 : commettre**

```bash
git add agent/src/wasapi/process_loopback.rs agent/src/diagnostics/audio.rs agent/src/diagnostics.rs scripts/run-agent.sh
git commit -F - <<'EOF'
feat(d7): la capture par processus, et la sonde qui la mesurera

CaptureProcessus va ou probe_process_loopback s'arretait : Initialize,
GetService, Start, lecture. Le format n'est pas demande mais IMPOSE (48 kHz,
2 canaux, 16 bits) : un client de process loopback n'est lie a aucun point de
terminaison, et poser le format qu'opus.rs attend supprime toute conversion.

L'activation est factorisee : la sonde et le produit activent exactement de la
meme facon, sans quoi la sonde ne mesurerait pas ce que le produit fait.

Que Windows accepte ce format n'est pas etabli : c'est la tache 3 qui le dira,
sur HRESULT et non sur conjecture.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
```

---

## Tâche 3 : LA MESURE — le *process loopback* rend-il vraiment des octets, et seulement les siens ?

**C'est la tâche qui gouverne le sous-bloc.** Aucune tâche ultérieure ne
commence avant que son relevé ② ne soit versé.

**Fichiers**
- Créer : `docs/superpowers/plans/journaux-multifenetres-d7/` (répertoire)
- Créer : `docs/superpowers/plans/journaux-multifenetres-d7/capture-*.log`
- Créer : `docs/superpowers/plans/2026-08-03-multifenetres-audio-par-fenetre-resultats.md` (§1 seulement)

- [ ] **Étape 1 : la VM**

```bash
virsh list --all
virsh start Windows 2>/dev/null
until timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null; do sleep 5; done
until ls /media/vm/dev >/dev/null 2>&1; do sleep 5; done
set -a && source .env && set +a
```

**Ne jamais supposer la VM allumée**, et **`/media/vm` se monte APRÈS que le
port 5985 répond** — `mountpoint -q` ne suffit pas, l'entrée CIFS survit à une
VM éteinte.

- [ ] **Étape 2 : compiler sur la VM et vérifier la taille du binaire**

```bash
scripts/build-agent.sh 2>&1 | tail -20
ls -l /media/vm/dev/agent/target/release/agent.exe
```

⚠️ **Une compilation de 0,13 s est un aveu, pas un succès** : après un
aller-retour de sources, cargo garde le binaire précédent et le dit comme un
succès. Seul `cargo clean --release -p agent` débloque. **Vérifier la TAILLE**,
et la noter — elle ira dans le document de résultats.

- [ ] **Étape 3 : relevé ① — la crête non nulle sur le processus cible**

Lancer sur la VM une application qui joue un son en boucle, relever son PID, puis :

```bash
node scripts/winrm.js 'Get-Process chrome | Select-Object Id,MainWindowTitle | Format-Table | Out-String'
PROCESS_LOOPBACK_CAPTURE=<pid> PROCESS_LOOPBACK_SECS=15 scripts/run-agent.sh
```

**Employer `Media.SoundPlayer` ou une vraie application, JAMAIS
`[Console]::Beep`** : celui-ci passe par `kernel32!Beep`, ne traverse pas le
périphérique de rendu, et a produit un **faux négatif documenté** au chantier A.

Attendu : `silencieux=false`, `crete` très supérieure à 0.

- [ ] **Étape 4 : relevé ② — LA MESURE DÉCISIVE : la crête NULLE sur le voisin**

Sans rien changer d'autre : **le processus A continue de jouer**, et l'on sonde
le PID d'un processus B **silencieux**.

```bash
PROCESS_LOOPBACK_CAPTURE=<pid_de_B> PROCESS_LOOPBACK_SECS=15 scripts/run-agent.sh
```

**Attendu : `silencieux=true`, `crete=0`.** Si la crête est non nulle, la
capture rend le mix global et **l'isolation stricte n'existe pas par cette
API** : arrêter ici, écrire le §1 des résultats comme une **réfutation**, et
revenir au cadrage. C'est le cas prévu, et ce n'est pas un échec du sous-bloc —
c'est ce pour quoi cette tâche est la première.

- [ ] **Étape 5 : relevé ③ — le format et le cycle**

Lire dans les journaux des étapes 3 et 4 :

- la ligne `process loopback ouvert` : `Initialize` a-t-il accepté le format
  imposé ? Sinon, **le `HRESULT` exact est le relevé** — le reporter tel quel,
  ne pas deviner un repli ;
- la ligne `cycle Stop puis Start accepte` ou `REFUSE`, et `crete_apres`. Un
  refus fait replier la tâche 8 sur l'approche B (source vivante, paquets
  jetés).

- [ ] **Étape 6 : copier les journaux APRÈS la fin réelle de l'exécution**

```bash
mkdir -p docs/superpowers/plans/journaux-multifenetres-d7
cp /media/vm/dev/agent/agent.log docs/superpowers/plans/journaux-multifenetres-d7/capture-releve1.log
```

Un journal d'agent **s'écrase facilement** : copier avant tout relevé qui écrit
au même endroit. Une pièce a été perdue ainsi en D2.

- [ ] **Étape 7 : contrôler que la VM a survécu**

```bash
grep -E "terminating on signal|shutting down" /var/log/libvirt/qemu/Windows.log | tail -4
```

Elle s'hiberne d'elle-même, déclencheur non identifié.

- [ ] **Étape 8 : écrire le §1 des résultats et commettre**

Créer
`docs/superpowers/plans/2026-08-03-multifenetres-audio-par-fenetre-resultats.md`
avec un §1 qui porte **le nombre d'exécutions dans chaque énoncé** et distingue
ce qui est **relevé** de ce qui est **calculé** ou **inféré**.

```bash
git add docs/superpowers/plans/journaux-multifenetres-d7/ docs/superpowers/plans/2026-08-03-multifenetres-audio-par-fenetre-resultats.md
git commit -F - <<'EOF'
mesure(d7): le process loopback rend des octets, et seulement les siens

Le releve qui gouverne n'est pas la crete non nulle sur le processus cible :
une capture qui rendrait le mix global la produirait aussi. C'est la crete NULLE
pendant qu'un AUTRE processus joue.

Journaux verses, nombre d'executions dans chaque enonce.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
```

---

## Tâche 4 : `capteur/audio.rs` — la règle pure

**Fichiers**
- Créer : `agent/src/capteur/audio.rs`
- Modifier : `agent/src/capteur.rs` (déclaration du module)

**Interfaces**
- Produit :
  - `pub struct FenetreAudio { pub session: String, pub pid: u32, pub arrivee: u64, pub dernier_focus: u64 }`
  - `pub fn arbitrer(fenetres: &[FenetreAudio]) -> Vec<(String, bool)>`

- [ ] **Étape 1 : écrire les tests qui échouent**

Créer `agent/src/capteur/audio.rs` avec **seulement** le bloc de tests et les
déclarations minimales :

```rust
//! La règle : qui porte le son.
//!
//! **Pur, sans aucun `cfg`, sans COM, sans fenêtre** — comme `vivier.rs` et
//! `repartiteur.rs` avant lui. Il ne connaît ni `IAudioClient` ni `HWND` : il
//! reçoit des PID et rend des booléens.
//!
//! **Le sommeil n'entre PAS dans la règle**, et son absence de ce fichier est
//! le meilleur endroit pour le dire : `FenetreAudio` ne porte aucun champ
//! `eveillee`. Une fenêtre endormie (sous-bloc D5) a relâché son encodeur
//! vidéo ; son application peut parfaitement continuer à jouer de la musique,
//! et c'est précisément le cas où l'on veut du son sans image.

use std::cmp::Ordering;
use std::collections::HashMap;

/// Ce que le registre sait d'une fenêtre, du point de vue du son.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FenetreAudio {
    pub session: String,
    /// PID du processus propriétaire de la fenêtre Windows.
    pub pid: u32,
    /// Rang d'arrivée, strictement croissant. Départage deux fenêtres d'un
    /// même processus dont **aucune** n'a jamais été focalisée.
    pub arrivee: u64,
    /// Rang du dernier focus reçu, `0` si cette session n'a jamais été
    /// focalisée. **Un rang, pas un horodatage** : un `Instant` n'est pas
    /// comparable entre processus et n'apporterait rien ici.
    pub dernier_focus: u64,
}

pub fn arbitrer(_fenetres: &[FenetreAudio]) -> Vec<(String, bool)> {
    todo!("étape 3")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fenetre(session: &str, pid: u32, arrivee: u64, dernier_focus: u64) -> FenetreAudio {
        FenetreAudio { session: session.into(), pid, arrivee, dernier_focus }
    }

    fn porteurs(fenetres: &[FenetreAudio]) -> Vec<String> {
        let mut noms: Vec<String> = arbitrer(fenetres)
            .into_iter()
            .filter(|(_, actif)| *actif)
            .map(|(session, _)| session)
            .collect();
        noms.sort();
        noms
    }

    #[test]
    fn une_fenetre_seule_de_son_processus_porte_le_son() {
        let f = vec![fenetre("a", 100, 1, 0)];
        assert_eq!(porteurs(&f), vec!["a".to_string()]);
    }

    #[test]
    fn deux_processus_distincts_portent_chacun_le_leur() {
        // Le cas nominal du produit : une application par fenêtre.
        let f = vec![fenetre("a", 100, 1, 0), fenetre("b", 200, 2, 0)];
        assert_eq!(porteurs(&f), vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn deux_fenetres_d_un_meme_processus_jamais_focalisees_la_premiere_arrivee_porte() {
        let f = vec![fenetre("a", 100, 1, 0), fenetre("b", 100, 2, 0)];
        assert_eq!(porteurs(&f), vec!["a".to_string()]);
    }

    #[test]
    fn le_focus_prend_le_son_a_sa_voisine_du_meme_processus() {
        // "b" arrive après "a" et prend le focus : le son bascule.
        let f = vec![fenetre("a", 100, 1, 0), fenetre("b", 100, 2, 7)];
        assert_eq!(porteurs(&f), vec!["b".to_string()]);
    }

    #[test]
    fn un_groupe_qui_perd_tout_focus_garde_son_son_sur_la_derniere_focalisee() {
        // C'est la règle 3 de la spec, et elle n'est pas cosmétique :
        // `focalisee` est GLOBAL — au plus une fenêtre focalisée sur toute la
        // session. Cliquer sur une fenêtre d'un AUTRE processus fait perdre le
        // focus à tout ce groupe, et sans cette règle son son se couperait.
        let f = vec![
            fenetre("a", 100, 1, 3),
            fenetre("b", 100, 2, 7),
            fenetre("etranger", 200, 3, 9),
        ];
        assert_eq!(porteurs(&f), vec!["b".to_string(), "etranger".to_string()]);
    }

    #[test]
    fn l_oubli_du_porteur_fait_passer_le_son_a_la_suivante_du_groupe() {
        // Le registre retire "b" (canal rompu, fermeture) et rappelle
        // `arbitrer` sur ce qui reste : "a" doit reprendre le son, sans quoi
        // le groupe deviendrait définitivement muet.
        let f = vec![fenetre("a", 100, 1, 3)];
        assert_eq!(porteurs(&f), vec!["a".to_string()]);
    }

    #[test]
    fn la_decision_est_rendue_pour_chaque_session_meme_muette() {
        // `arbitrer` rend une entrée par fenêtre, pas seulement pour les
        // porteuses : le registre a besoin du `false` pour envoyer l'ordre de
        // se taire à celle qui portait le son juste avant.
        let f = vec![fenetre("a", 100, 1, 0), fenetre("b", 100, 2, 0)];
        let decisions = arbitrer(&f);
        assert_eq!(decisions.len(), 2);
        assert!(decisions.contains(&("a".to_string(), true)));
        assert!(decisions.contains(&("b".to_string(), false)));
    }

    #[test]
    fn aucune_fenetre_rend_aucune_decision() {
        assert!(arbitrer(&[]).is_empty());
    }
}
```

Déclarer le module dans `agent/src/capteur.rs`, à sa place alphabétique parmi
les `mod` existants :

```rust
pub mod audio;
```

- [ ] **Étape 2 : lancer les tests pour vérifier qu'ils échouent**

```bash
cd agent && cargo test capteur::audio 2>&1 | tail -20
```

Attendu : **échec** sur `not yet implemented` (le `todo!`). Un test qui passe
ici signalerait qu'il ne teste rien.

- [ ] **Étape 3 : écrire la règle**

Remplacer le `todo!` :

```rust
/// Rend, pour chaque fenêtre, si elle porte le son.
///
/// **Une entrée par fenêtre, y compris les muettes** : le registre a besoin du
/// `false` pour ordonner de se taire à celle qui portait le son l'instant
/// d'avant.
pub fn arbitrer(fenetres: &[FenetreAudio]) -> Vec<(String, bool)> {
    let mut porteur: HashMap<u32, &FenetreAudio> = HashMap::new();
    for f in fenetres {
        match porteur.get(&f.pid) {
            Some(actuel) if !l_emporte(f, actuel) => {}
            _ => {
                porteur.insert(f.pid, f);
            }
        }
    }

    fenetres
        .iter()
        .map(|f| {
            let actif = porteur.get(&f.pid).is_some_and(|p| p.session == f.session);
            (f.session.clone(), actif)
        })
        .collect()
}

/// `candidat` l'emporte-t-il sur `actuel` au sein de leur groupe de PID ?
///
/// Le focus le plus RÉCENT prime ; à égalité — deux fenêtres jamais focalisées,
/// donc `dernier_focus == 0` toutes les deux — la PREMIÈRE arrivée. Le sommeil
/// n'entre pas dans la comparaison, et aucun champ ne le porte.
fn l_emporte(candidat: &FenetreAudio, actuel: &FenetreAudio) -> bool {
    match candidat.dernier_focus.cmp(&actuel.dernier_focus) {
        Ordering::Greater => true,
        Ordering::Less => false,
        Ordering::Equal => candidat.arrivee < actuel.arrivee,
    }
}
```

- [ ] **Étape 4 : lancer les tests pour vérifier qu'ils passent**

```bash
cd agent && cargo test capteur::audio 2>&1 | tail -10
```

Attendu : `test result: ok. 8 passed`.

- [ ] **Étape 5 : commettre**

```bash
git add agent/src/capteur/audio.rs agent/src/capteur.rs
git commit -F - <<'EOF'
feat(d7): la regle pure de l'audio, sans aucun cfg

Qui porte le son : une fenetre par PID, la focalisee si l'une du groupe l'a,
sinon la derniere a l'avoir eu, sinon la premiere arrivee.

Le sommeil n'entre pas dans la regle, et son ABSENCE de ce fichier est le
meilleur endroit pour le dire : FenetreAudio ne porte aucun champ eveillee. Une
endormie a relache son encodeur video ; son application peut parfaitement
continuer a jouer de la musique.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
```

---

## Tâche 5 : brancher la règle sur le registre du capteur

**Fichiers**
- Créer : `agent/src/capteur/sommeil/porteurs.rs`
- Modifier : `agent/src/capteur/sommeil.rs`
- Modifier : `agent/src/capteur/fenetre.rs`

**Interfaces**
- Consomme : `capteur::audio::{arbitrer, FenetreAudio}`.
- Produit :
  - `capteur::sommeil::Message::Audio { actif: bool }`
  - `capteur::sommeil::inscrire(session: &str, pid: u32) -> Receiver<Message>` (**signature changée**)
  - `capteur::sommeil::porteurs::distribuer_l_audio(garde: &mut MutexGuard<'static, Etat>)`

> **Nommage** : la règle est `capteur/audio.rs`, la branche
> `capteur/sommeil/porteurs.rs`. Les deux ne portent **pas** le même nom, pour
> la raison exacte que `repartiteur.rs` (la règle) et `sommeil/parts.rs` (la
> branche) ne le portent pas non plus — voir l'en-tête de `parts.rs`.

- [ ] **Étape 1 : écrire le test qui échoue**

Ajouter à la fin de `agent/src/capteur/sommeil.rs`, dans son `mod tests`
existant :

```rust
    /// Dernier ordre audio reçu sur un canal, en vidant ce qui s'y trouve.
    fn dernier_audio(canal: &std::sync::mpsc::Receiver<Message>) -> Option<bool> {
        canal
            .try_iter()
            .filter_map(|m| match m {
                Message::Audio { actif } => Some(actif),
                _ => None,
            })
            .last()
    }

    #[test]
    fn deux_fenetres_d_un_meme_pid_se_disputent_le_son_et_le_focus_tranche() {
        let _verrou = verrouiller_pour_le_test();
        let a = inscrire("t9-a", 4242);
        let b = inscrire("t9-b", 4242);

        // Aucune focalisée : la première arrivée porte le son.
        assert_eq!(dernier_audio(&a), Some(true), "la premiere arrivee porte le son");
        assert_eq!(dernier_audio(&b), Some(false), "la seconde se tait");

        // "b" prend le focus : le son bascule, et "a" reçoit l'ordre de se
        // taire — sans quoi les deux seraient audibles en même temps.
        signaler("t9-b", true, true);
        assert_eq!(dernier_audio(&b), Some(true), "la focalisee prend le son");
        assert_eq!(dernier_audio(&a), Some(false), "la precedente porteuse se tait");

        // "b" disparaît : "a" doit reprendre le son, sinon le groupe devient
        // definitivement muet.
        retirer("t9-b");
        assert_eq!(dernier_audio(&a), Some(true), "le son revient a la survivante");

        retirer("t9-a");
    }

    #[test]
    fn deux_pid_distincts_portent_chacun_leur_son() {
        let _verrou = verrouiller_pour_le_test();
        let a = inscrire("t9-c", 111);
        let b = inscrire("t9-d", 222);
        assert_eq!(dernier_audio(&a), Some(true));
        assert_eq!(dernier_audio(&b), Some(true));
        retirer("t9-c");
        retirer("t9-d");
    }

    #[test]
    fn un_ordre_audio_inchange_n_est_pas_reemis() {
        // Sans le filtre d'écrasement, le tour de roue (250 ms) enverrait
        // quatre ordres par seconde et par fenêtre, à vie. Même rempart que
        // `dernieres_parts`.
        let _verrou = verrouiller_pour_le_test();
        let a = inscrire("t9-e", 333);
        let _ = a.try_iter().count();
        signaler("t9-e", true, true);
        let ordres: Vec<Message> = a
            .try_iter()
            .filter(|m| matches!(m, Message::Audio { .. }))
            .collect();
        assert!(ordres.is_empty(), "ordre audio inchange reemis : {ordres:?}");
        retirer("t9-e");
    }
```

Corriger dans le même mouvement **tous** les appels existants à `inscrire` des
tests de `sommeil.rs` et de `sommeil/parts.rs` : ils prennent désormais un PID.
Employer un PID **distinct par session** dans les tests préexistants (par
exemple l'index de boucle), pour ne pas leur faire subir l'arbitrage par groupe.

- [ ] **Étape 2 : lancer les tests pour vérifier qu'ils échouent**

```bash
cd agent && cargo test capteur::sommeil 2>&1 | tail -20
```

Attendu : **échec de compilation** (`Message::Audio` n'existe pas, `inscrire`
prend un argument de trop). C'est la forme d'échec attendue ici.

- [ ] **Étape 3 : étendre le registre**

Dans `agent/src/capteur/sommeil.rs` :

```rust
mod parts;
mod porteurs;
```

Ajouter le bras au `Message` :

```rust
pub enum Message {
    Sommeil(Ordre),
    Part { bps: u32 },
    /// Ordre de porter le son, ou de se taire. Poussé **au changement
    /// seulement**, comme `Part`.
    ///
    /// Sur le même canal que les deux autres, et pour la même raison : un
    /// canal unique garantit l'ordre de LIVRAISON. Un ordre de se taire doit
    /// atteindre l'ancienne porteuse **avant** que la nouvelle ne commence,
    /// sans quoi les deux fenêtres d'un même processus seraient audibles
    /// ensemble.
    Audio { actif: bool },
}
```

Étendre `Etat` :

```rust
struct Etat {
    vivier: Vivier,
    canaux: HashMap<String, Sender<Message>>,
    focalisee: Option<String>,
    dernieres_parts: HashMap<String, u32>,
    /// PID du processus propriétaire de chaque fenêtre. **Ici et pas dans un
    /// second registre** : le capteur n'a qu'une vérité à tenir, et deux
    /// tables à synchroniser en feraient deux.
    pids: HashMap<String, u32>,
    /// Rang d'arrivée de chaque session, et rang du dernier focus reçu. Deux
    /// compteurs tirés du même `horloge`, strictement croissante.
    arrivees: HashMap<String, u64>,
    derniers_focus: HashMap<String, u64>,
    /// Compteur monotone qui sert de rang aux deux tables ci-dessus. Un
    /// `Instant` ne conviendrait pas : il faut un ordre total, stable et
    /// comparable, pas une durée.
    horloge: u64,
    /// Dernier ordre audio envoyé à chaque session. **Le rempart contre
    /// l'inondation**, exactement comme `dernieres_parts` : le tour de roue
    /// ré-arbitre toutes les 250 ms.
    derniers_audio: HashMap<String, bool>,
}
```

et son initialisation dans `etat()` :

```rust
            pids: HashMap::new(),
            arrivees: HashMap::new(),
            derniers_focus: HashMap::new(),
            horloge: 0,
            derniers_audio: HashMap::new(),
```

- [ ] **Étape 4 : le point de passage unique doit tout oublier**

Dans `oublier` — **c'est le défaut M1 de D6 rejoué si on l'omet** :

```rust
fn oublier(garde: &mut MutexGuard<'static, Etat>, session: &str) -> Vec<(String, Ordre)> {
    garde.canaux.remove(session);
    garde.dernieres_parts.remove(session);
    // Les quatre tables de D7 s'oublient ICI et nulle part ailleurs. Le
    // registre a trois chemins de retrait (fermeture normale, canal rompu
    // détecté par les ordres, canal rompu détecté par les parts) : un champ
    // oublié par deux d'entre eux est exactement le défaut M1 de la revue
    // finale de branche du sous-bloc D6.
    //
    // `derniers_audio` en particulier : un rattachement réinscrit la MÊME
    // session (voir `inscrire`), et un `false` resté en mémoire ferait juger
    // l'ordre déjà livré — sur un canal disparu avec la rupture. La fenêtre
    // resterait muette sans terme.
    garde.pids.remove(session);
    garde.arrivees.remove(session);
    garde.derniers_focus.remove(session);
    garde.derniers_audio.remove(session);
    if garde.focalisee.as_deref() == Some(session) {
        garde.focalisee = None;
    }
    garde.vivier.retirer(session, Instant::now())
}
```

- [ ] **Étape 5 : `inscrire` prend le PID, `signaler` avance l'horloge**

```rust
pub fn inscrire(session: &str, pid: u32) -> Receiver<Message> {
    let (emetteur, receveur) = channel::<Message>();
    let mut garde = etat();
    if garde.canaux.insert(session.to_string(), emetteur).is_some() {
        tracing::warn!(%session, "canal d'ordres remplacé pour cette session");
        garde.dernieres_parts.remove(session);
        // Même motif que la ligne ci-dessus : l'ordre audio mémorisé l'a été
        // sur l'ANCIEN canal, disparu avec la rupture.
        garde.derniers_audio.remove(session);
    }
    garde.pids.insert(session.to_string(), pid);
    // Le rang d'arrivée n'est posé qu'à la PREMIÈRE inscription : un
    // rattachement ne doit pas faire perdre à la fenêtre son ancienneté au
    // sein de son groupe de PID.
    if !garde.arrivees.contains_key(session) {
        garde.horloge += 1;
        let rang = garde.horloge;
        garde.arrivees.insert(session.to_string(), rang);
    }
    let ordres = garde.vivier.inscrire(session, Instant::now());
    distribuer(&mut garde, ordres);
    parts::distribuer_les_parts(&mut garde);
    porteurs::distribuer_l_audio(&mut garde);
    receveur
}
```

```rust
pub fn signaler(session: &str, visible: bool, focalisee: bool) {
    let mut garde = etat();
    if focalisee {
        garde.focalisee = Some(session.to_string());
        // Le rang du focus, et non un booléen : c'est lui qui fait tenir la
        // règle 3 de l'arbitrage — un groupe qui perd tout focus garde son son
        // sur la DERNIÈRE à l'avoir eu.
        garde.horloge += 1;
        let rang = garde.horloge;
        garde.derniers_focus.insert(session.to_string(), rang);
    } else if garde.focalisee.as_deref() == Some(session) {
        garde.focalisee = None;
    }
    let ordres = garde.vivier.signaler(session, visible, focalisee, Instant::now());
    distribuer(&mut garde, ordres);
    parts::distribuer_les_parts(&mut garde);
    porteurs::distribuer_l_audio(&mut garde);
}
```

Ajouter le même appel `porteurs::distribuer_l_audio(&mut garde);` en dernière
position de `retirer`, `echec_de_reveil`, et du corps du tour de roue
(`demarrer_le_tour_de_roue`).

- [ ] **Étape 6 : écrire la branche**

Créer `agent/src/capteur/sommeil/porteurs.rs` :

```rust
//! Distribution des ordres audio — la branche de `capteur::audio::arbitrer`
//! sur le registre de `capteur::sommeil`.
//!
//! **Extrait de `sommeil.rs` et non ajouté dedans**, exactement comme
//! `parts.rs` : ce fichier-là est proche de son plafond, et la règle du dépôt
//! veut qu'une addition substantielle s'accompagne d'une extraction.
//!
//! Il ne s'appelle pas `audio` : ce nom est déjà pris par le module qui porte
//! la RÈGLE pure. Celui-ci ne porte que sa BRANCHE sur ce registre — même
//! distinction que `repartiteur` / `parts`.

use std::sync::MutexGuard;

use crate::capteur::audio::{arbitrer, FenetreAudio};

use super::{oublier, Etat, Message};

/// Recalcule qui porte le son et n'envoie que ce qui a changé.
///
/// **Appelée APRÈS `distribuer_les_parts`**, en dernière position de tous les
/// chemins d'entrée du registre. L'ordre importe peu vis-à-vis des parts — le
/// son et le débit sont orthogonaux — mais un ordre unique et documenté vaut
/// mieux qu'un ordre qui dépend de l'appelant.
///
/// Un canal rompu ici est retiré par `oublier`, exactement comme dans
/// `distribuer` et `distribuer_les_parts` : c'est le point de passage unique du
/// registre, et le contourner laisserait des entrées fantômes au vivier.
pub(super) fn distribuer_l_audio(garde: &mut MutexGuard<'static, Etat>) {
    let fenetres: Vec<FenetreAudio> = garde
        .canaux
        .keys()
        .filter_map(|session| {
            // Une session sans PID connu n'existe pas : `inscrire` pose les
            // deux ensemble. Le `filter_map` est un filet, pas un cas nominal.
            let pid = *garde.pids.get(session)?;
            Some(FenetreAudio {
                session: session.clone(),
                pid,
                arrivee: garde.arrivees.get(session).copied().unwrap_or(0),
                dernier_focus: garde.derniers_focus.get(session).copied().unwrap_or(0),
            })
        })
        .collect();

    let decisions = arbitrer(&fenetres);

    let vivantes: std::collections::HashSet<&String> =
        decisions.iter().map(|(session, _)| session).collect();
    garde.derniers_audio.retain(|session, _| vivantes.contains(session));

    // Les ordres de SE TAIRE partent d'abord, les ordres de PORTER ensuite.
    // Sans cet ordre, une bascule de focus au sein d'un groupe rendrait les
    // deux fenêtres audibles pendant le temps qui sépare les deux messages —
    // court, mais parfaitement audible sur de la musique.
    let (a_porter, a_taire): (Vec<_>, Vec<_>) =
        decisions.into_iter().partition(|(_, actif)| *actif);

    let mut rompus = Vec::new();
    for (session, actif) in a_taire.into_iter().chain(a_porter) {
        if garde.derniers_audio.get(&session) == Some(&actif) {
            continue;
        }
        let envoye = match garde.canaux.get(&session) {
            Some(canal) => canal.send(Message::Audio { actif }).is_ok(),
            None => false,
        };
        if envoye {
            garde.derniers_audio.insert(session, actif);
        } else {
            rompus.push(session);
        }
    }

    let mut ordres_du_retrait = Vec::new();
    for session in rompus {
        ordres_du_retrait.extend(oublier(garde, &session));
    }
    if !ordres_du_retrait.is_empty() {
        super::distribuer(garde, ordres_du_retrait);
    }
}
```

- [ ] **Étape 7 : le fil de fenêtre dérive le PID et le passe**

Dans `agent/src/capteur/fenetre.rs`, dans `Fenetre::ouvrir`, juste après la
construction du `hwnd` (l. 174) :

```rust
        // Le PID ne circule pas sur le protocole : il se dérive du `hwnd` que
        // l'enfant a déjà envoyé. L'enfant fait de même de son côté, depuis son
        // `FENETRE_HWND`. Deux dérivations indépendantes du même identifiant
        // stable valent mieux qu'un champ de protocole à tenir cohérent.
        let mut pid = 0u32;
        // SAFETY : `hwnd` vient d'un enfant vivant ; un handle invalide fait
        // rendre 0 à la fonction, ce que le `ensure` ci-dessous attrape.
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        anyhow::ensure!(
            pid != 0,
            "impossible de dériver le PID de la fenêtre {hwnd:?} de la session {session}"
        );
```

Ajouter `pid: u32` au struct `Fenetre` et le poser dans le `Ok(Fenetre { ... })`
final d'`ouvrir`. Ajouter l'import :

```rust
use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;
```

Puis, dans `servir` (l. 236) :

```rust
        let ordres = crate::capteur::sommeil::inscrire(&session, self.pid);
```

- [ ] **Étape 8 : lancer les tests**

```bash
cd agent && cargo test capteur 2>&1 | tail -20 && cargo check --target x86_64-pc-windows-gnu 2>&1 | tail -20
```

Attendu : tous les tests de `capteur::` verts, compilation Windows sans erreur.

- [ ] **Étape 9 : relever les tailles et commettre**

```bash
wc -l agent/src/capteur/sommeil.rs agent/src/capteur/sommeil/porteurs.rs agent/src/capteur/audio.rs agent/src/capteur/fenetre.rs
git add agent/src/capteur/sommeil.rs agent/src/capteur/sommeil/porteurs.rs agent/src/capteur/fenetre.rs
git commit -F - <<'EOF'
feat(d7): le capteur decide qui porte le son

Le registre gagne quatre tables (pids, arrivees, derniers_focus, derniers_audio)
et elles s'oublient TOUTES dans `oublier`, le point de passage unique. Un champ
oublie par deux des trois chemins de retrait est exactement le defaut M1 que la
revue finale de D6 a paye.

Les ordres de SE TAIRE partent avant les ordres de PORTER : sans cet ordre, une
bascule de focus rendrait les deux fenetres d'un meme processus audibles
ensemble le temps qui separe les deux messages.

Le PID ne circule pas sur le protocole : capteur et enfant le derivent chacun du
hwnd qu'ils ont deja.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
```

---

## Tâche 6 : le protocole et le chemin jusqu'à l'enfant

**Fichiers**
- Modifier : `agent/src/capteur/protocole.rs`
- Modifier : `agent/src/capteur/fenetre/transitions.rs`
- Modifier : `agent/src/capteur/distante.rs`
- Modifier : `agent/src/source.rs`
- Modifier : `agent/src/transport/tick.rs`

**Interfaces**
- Consomme : `capteur::sommeil::Message::Audio { actif: bool }` (tâche 5).
- Produit :
  - `capteur::protocole::DepuisCapteur::Audio { actif: bool }`
  - `capteur::distante::Recu::Audio { actif: bool }`
  - `source::VideoSource::audio_a_appliquer(&mut self) -> Option<bool>` (défaut `None`)

- [ ] **Étape 1 : le message de protocole**

Dans `agent/src/capteur/protocole.rs`, ajouter au bout de `DepuisCapteur` :

```rust
    /// Ordre de porter le son, ou de se taire. Poussé non sollicité, **au
    /// changement seulement**.
    ///
    /// Distinct d'`Etat` pour la même raison que `Sommeil` et `Part` : `Etat`
    /// alimente un cache lu à chaque tour de la boucle de transport, et y mêler
    /// une annonce ponctuelle passerait par un chemin conçu pour un état
    /// permanent.
    ///
    /// **Le capteur ne capte AUCUN son.** Il arbitre seulement : il sait quelles
    /// fenêtres partagent un processus (il a leurs `hwnd`) et qui a le focus,
    /// ce que l'enfant ignore. La capture, elle, vit dans l'enfant — le *process
    /// loopback* n'a aucune des propriétés qui avaient forcé la mutualisation de
    /// la vidéo en D4.
    Audio { actif: bool },
```

- [ ] **Étape 2 : le relais dans le fil de fenêtre**

Dans `agent/src/capteur/fenetre/transitions.rs`, ajouter un bras à
`appliquer_les_ordres`, après celui de `Message::Part` (l. 152-164) :

```rust
                Ok(Message::Audio { actif }) => {
                    // Rien à faire localement : le capteur ne capte pas de son.
                    // Il n'est ici que le facteur, comme pour les parts.
                    let message = DepuisCapteur::Audio { actif };
                    if let Fin::Terminer(motif) =
                        deposer(AEcrire::Etat(message), ecritures, self.source.as_mut(), ctx)
                    {
                        return Fin::Terminer(motif);
                    }
                }
```

- [ ] **Étape 3 : la réception côté enfant**

Dans `agent/src/capteur/distante.rs` :

```rust
    /// Ordre audio poussé par le capteur, non sollicité. Retenu par
    /// `SourceDistante::audio` jusqu'à ce que `audio_a_appliquer` le consomme.
    Audio { actif: bool },
```

au bout de `Recu`, puis le champ dans `SourceDistante` :

```rust
    /// Dernier ordre audio reçu du capteur, en attente d'application. Consommé
    /// par `audio_a_appliquer`. Même régime d'écrasement que `part`.
    ///
    /// ⚠️ **`None` à la naissance, et ce n'est pas « pas d'ordre » mais « rien
    /// à changer »** : l'enfant naît MUET (voir `demarrage.rs`), et le capteur
    /// lui envoie son premier ordre dès l'attache. Partir d'un `Some(true)`
    /// implicite ferait porter le son aux deux fenêtres d'un même processus
    /// pendant les millisecondes qui précèdent le premier arbitrage.
    audio: Option<bool>,
```

posé à `None` dans `nouvelle`, **et remis à `None` dans la branche de
rattachement** (l. 203, à côté de `self.endormie = true;`) avec ce
commentaire :

```rust
                                // Un rattachement passe par une `Fenetre` NEUVE
                                // côté capteur, dont `sommeil::inscrire` purge
                                // `derniers_audio` : un ordre neuf arrive donc
                                // toujours. Ne rien retenir d'avant la rupture.
                                self.audio = None;
```

Le bras de réception dans `next_frame`, après celui de `Recu::Part` :

```rust
                Ok(Recu::Audio { actif }) => {
                    self.audio = Some(actif);
                }
```

Et la redéfinition de trait, à côté de `part_a_appliquer` (l. 275) :

```rust
    fn audio_a_appliquer(&mut self) -> Option<bool> {
        self.audio.take()
    }
```

Enfin, dans le décodage des trames JSON reçues du capteur, ajouter le bras
`DepuisCapteur::Audio { actif } => Recu::Audio { actif }` **à côté de celui de
`DepuisCapteur::Part`** — le lire dans le fichier pour en reproduire la forme
exacte.

- [ ] **Étape 4 : la méthode de trait**

Dans `agent/src/source.rs`, après `part_a_appliquer` (l. 125) :

```rust
    /// Rend l'ordre audio en attente d'application, et le consomme.
    ///
    /// **État courant, pas un historique** : deux ordres arrivés entre deux
    /// lectures s'écrasent — même régime que `part_a_appliquer` juste
    /// au-dessus. Comme elle consomme, la branche de transport qui l'interroge
    /// à ~100 Hz ne peut pas rejouer `Start()`/`Stop()` en boucle.
    ///
    /// Par défaut sans effet : une source fichier n'a pas de son, et une
    /// `WindowsSource` tenue en direct par son propre processus est
    /// mono-fenêtre, donc jamais arbitrée. Seule `SourceDistante` la redéfinit.
    fn audio_a_appliquer(&mut self) -> Option<bool> {
        None
    }
```

- [ ] **Étape 5 : la branche de transport**

Dans `agent/src/transport/tick.rs`, juste après la branche `a1quater`
(l. 221-224) :

```rust
        // a1quinquies) Un ordre audio décidé par le capteur. Après a1quater,
        //              pour la même raison de lisibilité : on respecte l'ordre
        //              d'arrivée plutôt que de l'inverser sans raison.
        //
        //              Ne met AUCUN paquet en file : `appliquer_audio` ne
        //              touche que la source audio et le budget du contrôleur —
        //              l'invariant de drainage de cette fonction est préservé.
        //
        //              `audio_a_appliquer` CONSOMME : un `Start()`/`Stop()` par
        //              tour à ~100 Hz est exactement ce que cette consommation
        //              empêche.
        if let Some(actif) = self.source.audio_a_appliquer() {
            self.appliquer_audio(actif);
            return Ok(Tick::Continue);
        }
```

`appliquer_audio` est écrite à la tâche 8. Pour que ce commit compile seul,
l'ajouter dès maintenant à `agent/src/transport/piste_audio.rs` sous sa forme
minimale, que la tâche 8 complètera :

```rust
    /// Applique l'arbitrage audio du capteur : porter le son, ou se taire.
    pub(super) fn appliquer_audio(&mut self, actif: bool) {
        tracing::info!(session = %self.session_id, actif, "ordre audio applique");
    }
```

- [ ] **Étape 6 : vérifier et commettre**

```bash
cd agent && cargo test 2>&1 | tail -10 && cargo check --target x86_64-pc-windows-gnu 2>&1 | tail -20
```

```bash
git add agent/src/capteur/protocole.rs agent/src/capteur/fenetre/transitions.rs agent/src/capteur/distante.rs agent/src/source.rs agent/src/transport/tick.rs agent/src/transport/piste_audio.rs
git commit -F - <<'EOF'
feat(d7): l'ordre audio circule du capteur jusqu'a l'enfant

Un seul message neuf, sur le canal de commandes qui porte deja Sommeil et Part.
Aucun octet audio ne traverse le tube : le capteur arbitre, il ne capte pas.

SourceDistante::audio part de None et y retourne a chaque rattachement, pour la
meme raison qu'endormie repart a true : un ordre neuf arrive toujours, et
retenir celui d'avant la rupture ferait porter le son a deux fenetres d'un meme
processus.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
```

---

## Tâche 7 : l'enfant capte le son de SON processus

**Fichiers**
- Modifier : `agent/src/windows_audio.rs`
- Modifier : `agent/src/demarrage.rs`
- Modifier : `agent/src/main.rs` (documentation de `AUDIO`)

**Interfaces**
- Consomme : `wasapi::process_loopback::CaptureProcessus` (tâche 2).
- Produit : `windows_audio::WindowsAudioSource::pour_processus(pid: u32, origin: Instant) -> Result<Self>`, `WindowsAudioSource::emettre(&self, actif: bool)`.

> ⚠️ **Si la tâche 3 a relevé un refus du cycle `Stop()`/`Start()`**, remplacer
> partout ci-dessous les appels `capture.demarrer()` / `capture.arreter()` par
> un simple saut de l'écriture dans l'anneau (approche B de la spec §4.4) : la
> capture reste vivante, seuls les paquets cessent d'être déposés. Le reste du
> plan est inchangé.

- [ ] **Étape 1 : généraliser la source audio**

Dans `agent/src/windows_audio.rs`, introduire une énumération de capture pour
que le fil de production serve les deux chemins sans être dupliqué :

```rust
/// Ce que le fil de capture lit, selon le mode.
///
/// **Deux chemins, un seul fil.** Le mode global (`Session`) est celui d'avant
/// D7 et sert le cas mono-fenêtre — un agent lancé à la main, sans
/// `FENETRE_HWND`. Le mode `Processus` est celui du multi-fenêtres.
enum Capture {
    Session(LoopbackCapture),
    Processus(crate::wasapi::process_loopback::CaptureProcessus),
}

impl Capture {
    fn read(&mut self) -> Result<Option<Vec<i16>>> {
        match self {
            Capture::Session(c) => c.read(),
            Capture::Processus(c) => c.read(),
        }
    }

    fn description(&self) -> String {
        match self {
            Capture::Session(c) => c.description().to_string(),
            Capture::Processus(c) => c.description().to_string(),
        }
    }

    /// Démarre ou arrête le flux. **Sans effet en mode session** : le loopback
    /// global n'est jamais arbitré — un agent mono-fenêtre porte toujours son
    /// son.
    fn emettre(&mut self, actif: bool) -> Result<()> {
        match self {
            Capture::Session(_) => Ok(()),
            Capture::Processus(c) => {
                if actif {
                    c.demarrer()
                } else {
                    c.arreter()
                }
            }
        }
    }
}
```

- [ ] **Étape 2 : l'ordre d'émission, et son observabilité**

Ajouter au struct `WindowsAudioSource` :

```rust
    /// Ordre d'émission voulu par l'arbitrage du capteur, lu par le fil de
    /// capture avant chaque tour. Même patron d'indirection que
    /// `perte_desiree` : la capture vit sur le fil, pas ici.
    ///
    /// **Faux à la naissance.** L'enfant naît MUET et n'émet que sur ordre du
    /// capteur — même doctrine que `SourceDistante::endormie`, qui naît à
    /// `true`. C'est ce qui évite que deux fenêtres d'un même processus soient
    /// toutes deux audibles pendant les millisecondes qui précèdent le premier
    /// arbitrage.
    emet: Arc<AtomicBool>,
    /// PID capté, pour la trace périodique. `None` en mode session.
    pid: Option<u32>,
```

et la méthode publique :

```rust
    /// Porte le son, ou se tait. Appelée depuis la boucle de transport, qui
    /// consomme l'ordre du capteur.
    pub fn emettre(&self, actif: bool) {
        self.emet.store(actif, Ordering::Relaxed);
    }
```

Dans le corps du fil, en tête de boucle :

```rust
                let mut emettait = false;
                while !arret_fil.load(Ordering::Relaxed) {
                    let veut_emettre = emet_fil.load(Ordering::Relaxed);
                    if veut_emettre != emettait {
                        match capture.emettre(veut_emettre) {
                            Ok(()) => emettait = veut_emettre,
                            Err(e) => {
                                // Un refus ne tue pas la session : on
                                // journalise et on retentera au prochain
                                // changement d'ordre plutôt qu'à chaque tour.
                                emettait = veut_emettre;
                                tracing::warn!(
                                    erreur = %e,
                                    actif = veut_emettre,
                                    "bascule d'emission audio refusee"
                                );
                            }
                        }
                    }
                    if !emettait {
                        // Muette : ne rien lire, ne rien encoder, ne rien
                        // déposer. Une trame de silence encodée coûterait
                        // quelques octets grâce au DTX, mais elle arriverait
                        // au navigateur — et deux fenêtres d'un même processus
                        // s'entendraient toutes les deux.
                        std::thread::sleep(POLL_INTERVAL);
                        continue;
                    }
                    // ... (le corps existant, inchangé)
```

Et dans la trace périodique `compteurs audio`, **ajouter les deux champs qui
rendent observable le mode de défaillance silencieux** :

```rust
                        tracing::info!(
                            pid = pid_fil,
                            actif = emettait,
                            rejetes = ring_fil.rejetes(),
                            complements = assembleur.complements(),
                            echantillons_jetes = assembleur.echantillons_jetes(),
                            "compteurs audio"
                        );
```

⚠️ **`actif` et `pid` sont le seul moyen de voir un arbitrage figé.** Si aucune
fenêtre ne portait jamais le son, le symptôme serait le silence total sans un
`WARN`, sans une erreur. C'est le `grep` d'entrée de D8 (spec §6).

- [ ] **Étape 3 : le constructeur par processus**

Extraire le corps commun de `new` dans une fonction privée
`demarrer(capture: Capture, origin: Instant, pid: Option<u32>) -> Result<Self>`,
puis :

```rust
    /// Ouvre le loopback GLOBAL de la session et démarre le fil de production.
    ///
    /// Mode mono-fenêtre : un agent lancé à la main, sans `FENETRE_HWND`. Le
    /// son est porté sans arbitrage — il n'y a personne avec qui le partager.
    pub fn new(origin: Instant) -> Result<Self> {
        let capture = LoopbackCapture::open().context("ouverture du loopback audio")?;
        let mut source = Self::demarrer(Capture::Session(capture), origin, None)?;
        // Aucun capteur n'enverra jamais d'ordre à cet agent : il émet d'emblée.
        source.emettre(true);
        Ok(source)
    }

    /// Ouvre le loopback du PROCESSUS `pid` et de son arbre, et démarre le fil
    /// de production.
    ///
    /// **La source naît MUETTE** : c'est le capteur qui décide qui porte le
    /// son, et son premier ordre arrive dès l'attache. Voir le champ `emet`.
    pub fn pour_processus(pid: u32, origin: Instant) -> Result<Self> {
        let capture = crate::wasapi::process_loopback::CaptureProcessus::ouvrir(pid)
            .with_context(|| format!("ouverture du process loopback du PID {pid}"))?;
        Self::demarrer(Capture::Processus(capture), origin, Some(pid))
    }
```

- [ ] **Étape 4 : le câblage dans `demarrage.rs`**

Remplacer le bloc l. 98-122 de `agent/src/demarrage.rs` :

```rust
    // Source audio : son absence ne compromet jamais la session vidéo. Sur une
    // source de test (TEST_FILE), il n'y a rien à capter. Hors Windows, il n'y
    // a pas de WASAPI. Et si la capture refuse de s'ouvrir, on journalise et la
    // session continue, muette.
    //
    // **Deux modes, et le repli n'est JAMAIS le mix global.** Avec
    // `FENETRE_HWND`, l'enfant capte le son du seul processus propriétaire de
    // sa fenêtre, et le capteur arbitre entre les fenêtres qui en partagent un.
    // Sans, il capte le mix de la session : c'est le mode mono-fenêtre d'avant
    // le sous-bloc D7, et il ne doit pas régresser.
    //
    // Retomber sur le mix global quand le process loopback échoue ferait
    // entendre à une fenêtre le son de TOUTES les autres, sous couvert
    // d'isolation — c'est le repli explicitement écarté au cadrage.
    #[cfg(windows)]
    if config.test_file.is_none() && config.audio {
        let ouverture = match config.fenetre_hwnd {
            Some(hwnd) => match pid_de_fenetre(hwnd) {
                Ok(pid) => windows_audio::WindowsAudioSource::pour_processus(pid, clock_origin),
                Err(e) => Err(e),
            },
            None => windows_audio::WindowsAudioSource::new(clock_origin),
        };
        match ouverture {
            Ok(source_audio) => {
                tracing::info!(format = source_audio.description(), "audio activé");
                session.set_audio_source(Box::new(source_audio));
            }
            Err(e) => {
                tracing::warn!(erreur = %e, "audio indisponible, la session continue sans son");
            }
        }
    }
    #[cfg(windows)]
    if !config.audio {
        tracing::info!("son désactivé sur cet agent par AUDIO=0");
    }
```

et ajouter la fonction, à côté des autres aides du fichier :

```rust
/// Le PID du processus propriétaire d'une fenêtre.
///
/// Dérivé du `hwnd`, exactement comme le capteur le dérive de son côté
/// (`capteur/fenetre.rs`). Le PID ne circule sur aucun protocole : deux
/// dérivations indépendantes du même identifiant stable valent mieux qu'un
/// champ de plus à tenir cohérent.
#[cfg(windows)]
fn pid_de_fenetre(hwnd: u64) -> anyhow::Result<u32> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

    let handle = HWND(hwnd as *mut core::ffi::c_void);
    let mut pid = 0u32;
    // SAFETY : un handle invalide fait rendre 0, ce que le `ensure` attrape.
    unsafe { GetWindowThreadProcessId(handle, Some(&mut pid)) };
    anyhow::ensure!(pid != 0, "aucun PID pour la fenêtre {hwnd:#x}");
    Ok(pid)
}
```

- [ ] **Étape 5 : compléter `appliquer_audio`**

Dans `agent/src/transport/piste_audio.rs`, remplacer la forme minimale de la
tâche 6 :

```rust
    /// Applique l'arbitrage audio du capteur : porter le son, ou se taire.
    ///
    /// **Deux effets, et le second est facile à oublier** : la source cesse
    /// d'émettre, ET le budget audio retenu par le contrôleur de congestion
    /// tombe à zéro. Sans le second, une fenêtre muette continuerait d'amputer
    /// son budget vidéo de 128 kb/s pour une piste qui n'émet rien — c'est le
    /// défaut préexistant que D7 corrige (spec §5).
    pub(super) fn appliquer_audio(&mut self, actif: bool) {
        if let Some(source) = self.audio_source.as_mut() {
            source.set_actif(actif);
        }
        self.congestion.changer_audio_bps(if actif {
            crate::opus::BITRATE_BPS as u32
        } else {
            0
        });
        // `session` : sans ce champ la trace n'est PAS attribuable — tous les
        // enfants héritent le même `agent.log` depuis D4. Même motif et même
        // champ que « part de budget appliquee ».
        tracing::info!(session = %self.session_id, actif, "ordre audio applique");
    }
```

Cela exige une méthode sur le trait `AudioSource` (`agent/src/audio.rs`), le
`Box<dyn AudioSource>` ne connaissant pas `WindowsAudioSource` :

```rust
    /// Porte le son, ou se tait, sur ordre de l'arbitrage du capteur.
    ///
    /// Sans effet par défaut : une source qui n'est pas arbitrée émet
    /// toujours.
    fn set_actif(&mut self, _actif: bool) {}
```

et sa redéfinition dans `windows_audio.rs` :

```rust
    fn set_actif(&mut self, actif: bool) {
        self.emettre(actif);
    }
```

- [ ] **Étape 6 : vérifier et commettre**

```bash
cd agent && cargo test 2>&1 | tail -10 && cargo check --target x86_64-pc-windows-gnu 2>&1 | tail -20
```

```bash
git add agent/src/windows_audio.rs agent/src/demarrage.rs agent/src/audio.rs agent/src/transport/piste_audio.rs agent/src/main.rs
git commit -F - <<'EOF'
feat(d7): l'enfant capte le son de SON processus, et se tait sur ordre

Deux modes : avec FENETRE_HWND, le process loopback du processus proprietaire ;
sans, le mix de session — le mode mono-fenetre d'avant D7, qui ne doit pas
regresser. Le repli n'est JAMAIS le mix global : il ferait entendre a une
fenetre le son de toutes les autres, sous couvert d'isolation.

La trace periodique porte desormais pid= et actif=. C'est le seul moyen de voir
un arbitrage fige : sans eux, le symptome serait le silence total sans un WARN.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
```

---

## Tâche 8 : `audio_bps` suit l'arbitrage

**Fichiers**
- Modifier : `agent/src/congestion/controleur.rs`
- Modifier : `agent/src/congestion/reconfiguration.rs` (tests existants)

**Interfaces**
- Produit : `congestion::Controleur::changer_audio_bps(&mut self, bps: u32)`.

- [ ] **Étape 1 : écrire le test qui échoue**

Dans le `mod tests` de `agent/src/congestion/controleur.rs` (ou de
`reconfiguration.rs`, selon où vivent les tests du contrôleur — le lire d'abord),
ajouter :

```rust
    #[test]
    fn une_session_muette_ne_retranche_plus_le_budget_audio() {
        // Le défaut préexistant que D7 corrige : `Controleur::new` posait
        // `audio_bps` inconditionnellement, si bien que sept fenêtres sur huit
        // amputaient leur budget vidéo de 128 kb/s pour une piste qu'elles
        // n'avaient pas — environ 8,5 % d'une part de 1,5 Mb/s.
        let debut = Instant::now();
        let mut avec = Controleur::new(config_de_test(), debut);
        let mut sans = Controleur::new(config_de_test(), debut);
        sans.changer_audio_bps(0);

        let o = observation_a(debut + DELAI_AMORCAGE * 2, 2_000_000);
        avec.observer(o);
        sans.observer(o);

        assert!(
            sans.debit_video_bps() > avec.debit_video_bps(),
            "sans piste audio, le budget video doit etre plus grand : {} vs {}",
            sans.debit_video_bps(),
            avec.debit_video_bps()
        );
        assert_eq!(
            sans.debit_video_bps() - avec.debit_video_bps(),
            crate::opus::BITRATE_BPS as u32,
            "l'ecart doit valoir exactement le budget audio"
        );
    }
```

Les aides `config_de_test`, `observation_a` et l'accesseur du débit vidéo
existent déjà sous un nom ou un autre dans les tests du contrôleur : **les lire
et employer les vrais noms**, ne pas en inventer.

- [ ] **Étape 2 : lancer le test pour vérifier qu'il échoue**

```bash
cd agent && cargo test une_session_muette 2>&1 | tail -15
```

Attendu : échec de compilation, `changer_audio_bps` n'existant pas.

- [ ] **Étape 3 : écrire la méthode**

Dans `agent/src/congestion/controleur.rs` :

```rust
    /// Change le budget réservé à la piste audio.
    ///
    /// **Zéro quand la session ne porte pas le son** (sous-bloc D7). Avant lui,
    /// `Config::audio_bps` valait inconditionnellement `opus::BITRATE_BPS`, et
    /// une fenêtre sans aucune piste audio amputait quand même son budget vidéo
    /// de 128 kb/s.
    ///
    /// **Ne recalcule rien de lui-même**, et c'est délibéré : la valeur ne mord
    /// qu'au prochain `observer`, qui est le seul endroit où le budget vidéo se
    /// dérive de l'estimation. Recalculer ici demanderait une estimation qui
    /// peut n'avoir jamais existé.
    pub fn changer_audio_bps(&mut self, bps: u32) {
        self.config.audio_bps = bps;
    }
```

- [ ] **Étape 4 : lancer les tests**

```bash
cd agent && cargo test congestion 2>&1 | tail -10
```

Attendu : tous verts.

- [ ] **Étape 5 : commettre**

```bash
git add agent/src/congestion/controleur.rs agent/src/congestion/reconfiguration.rs
git commit -F - <<'EOF'
fix(d7): une fenetre muette ne retranche plus 128 kb/s pour une piste absente

Controleur::new posait audio_bps inconditionnellement. A huit fenetres, sept
amputaient leur budget video d'un budget audio qu'elles n'employaient pas :
environ 8,5 % d'une part de 1,5 Mb/s. Defaut preexistant, corrige ici parce que
D7 est l'endroit ou la session sait enfin si elle porte le son.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
```

---

## Tâche 9 : le superviseur cesse de décider du son

**Fichiers**
- Modifier : `agent/src/superviseur/table.rs`, `enfants.rs`, `lanceur.rs`, `boucle.rs`
- Modifier : `agent/src/main.rs` (documentation de `AUDIO`)

- [ ] **Étape 1 : retirer le champ et la réservation**

Dans `agent/src/superviseur/table.rs` :
- supprimer le champ `audio_libre` de `Table` et son initialisation dans
  `nouvelle` ;
- supprimer le champ `audio` de `Entree` et sa reconduction dans la relance
  (l. 405) ;
- supprimer les lignes `let audio = self.audio_libre; self.audio_libre = false;`
  et le champ `audio` de l'`Entree` construite ;
- supprimer le paramètre `audio: bool` de la variante `Effet` concernée (l. 108)
  et de tous ses points de construction ;
- **supprimer le test `le_son_repasse_a_personne_tant_que_d2_ne_le_redesigne_pas`**
  et le long commentaire de dette qui l'accompagne : ils décrivent un mécanisme
  qui n'existe plus.

Dans `enfants.rs` : supprimer `pub audio: bool` de `Consigne`, le champ de la
trace l. 68, et l'aide de test `consigne(session, audio)` devient
`consigne(session)` — corriger ses appelants et l'assertion l. 181.

Dans `lanceur.rs` : supprimer la ligne `.env("AUDIO", ...)` (l. 234).

Dans `boucle.rs` : supprimer `audio` du motif de déstructuration (l. 136) et de
la construction de `Consigne` (l. 146).

- [ ] **Étape 2 : `AUDIO` reste un interrupteur global**

Dans `agent/src/main.rs`, remplacer le commentaire du champ `audio` de `Config`
(l. 96-98) et celui de sa lecture (l. 135-138) :

```rust
    /// Faux quand `AUDIO=0` coupe le son de cet agent.
    ///
    /// **Interrupteur GLOBAL, plus une consigne par fenêtre.** Jusqu'au
    /// sous-bloc D7, le superviseur posait `AUDIO=0` sur tous les enfants sauf
    /// un, parce qu'un unique loopback de session aurait été capté huit fois.
    /// Chaque enfant capte désormais le son de son PROPRE processus, et c'est
    /// le capteur qui arbitre entre les fenêtres d'un même processus : il n'y a
    /// plus rien à réserver.
    #[cfg_attr(not(windows), allow(dead_code))]
    audio: bool,
```

- [ ] **Étape 3 : vérifier**

```bash
cd agent && cargo test superviseur 2>&1 | tail -15 && cargo check --target x86_64-pc-windows-gnu 2>&1 | tail -20
wc -l agent/src/superviseur/table.rs
```

Attendu : tests verts, et `table.rs` **plus court** qu'avant (il était à 489
lignes, marge 11).

- [ ] **Étape 4 : commettre**

```bash
git add agent/src/superviseur/table.rs agent/src/superviseur/enfants.rs agent/src/superviseur/lanceur.rs agent/src/superviseur/boucle.rs agent/src/main.rs
git commit -F - <<'EOF'
refactor(d7): le superviseur ne decide plus du son

audio_libre disparait, et avec lui le defaut que son propre commentaire
documentait : « ne redevient jamais vrai une fois une porteuse designee ».
Chaque enfant capte le son de son processus, le capteur arbitre entre les
fenetres qui en partagent un — il n'y a plus rien a reserver.

AUDIO=0 reste un interrupteur GLOBAL : un agent lance a la main doit pouvoir
couper le son.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
```

---

## Tâche 10 : le span `tracing` porteur de `session` (consignation n°2 de D6)

**Fichiers**
- Modifier : `agent/src/capteur/fenetre.rs`

Sans lui, les traces du capteur sont anonymes dans un `agent.log` que tous les
enfants se partagent depuis D4, et la recette de la tâche 13 devrait imputer à
la main — ce que D6 a dû faire **en pleine mesure**.

- [ ] **Étape 1 : poser le span**

Dans `Fenetre::servir` (`agent/src/capteur/fenetre.rs`), avant le premier
`tracing::info!` :

```rust
        // Consignation n°2 du sous-bloc D6, portée ici : tous les enfants et le
        // capteur écrivent dans le MÊME `agent.log` (stdout hérité depuis D4).
        // Une trace sans `session` y est un nombre dans un multiensemble
        // anonyme, et D6 a dû ajouter ce champ à deux traces EN PLEINE RECETTE.
        // Un span posé une fois sur le fil de fenêtre le donne à tout ce qui
        // s'émet en dessous, y compris aux `warn!` des modules appelés.
        let _span = tracing::info_span!("fenetre", session = %session).entered();
```

⚠️ **`entered()` et non `enter()`** : le garde doit vivre jusqu'à la fin de
`servir`, et un `enter()` dont le résultat est ignoré serait relâché
immédiatement — le span ne couvrirait rien, sans qu'aucun test ne le dise.

- [ ] **Étape 2 : vérifier que le span est effectivement porté**

```bash
cd agent && cargo check --target x86_64-pc-windows-gnu 2>&1 | tail -10
```

La vérification réelle a lieu à la tâche 13, sur le journal : les lignes du
capteur doivent porter `fenetre{session=w-N}`. **C'est un contrôle qui peut
échouer** — vérifier qu'il n'est pas joué avant qu'une fenêtre existe, le piège
du contrôle prématuré de D6.

- [ ] **Étape 3 : commettre**

```bash
git add agent/src/capteur/fenetre.rs
git commit -F - <<'EOF'
feat(d7): un span tracing porteur de session sur le fil de fenetre du capteur

Consignation n°2 du sous-bloc D6. Tous les enfants et le capteur ecrivent dans
le meme agent.log depuis D4 : une trace sans session y est un nombre dans un
multiensemble anonyme, et D6 a du ajouter ce champ a deux traces en pleine
recette.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
```

---

## Tâche 11 : l'instrument de recette — la fréquence dominante

**Fichiers**
- Créer : `docs/superpowers/plans/journaux-multifenetres-d7/instrument/ton.html`
- Créer : `docs/superpowers/plans/journaux-multifenetres-d7/instrument/pilote-recette-d7.mjs`

Prouver « chaque fenêtre entend son application et elle seule » par
`bytesReceived` ne prouve **rien** : du son arrive, on ignore lequel.

- [ ] **Étape 1 : la source sonore, à fréquence connue et AFFICHÉE**

Créer `instrument/ton.html` : une page qui joue une onde sinusoïdale continue
dont la fréquence vient de la chaîne de requête, **et qui affiche cette
fréquence ainsi que la cadence de son `requestAnimationFrame`** — sans ce
chiffre, une capture muette et une source muette se lisent pareil.

```html
<!doctype html>
<meta charset="utf-8">
<title>ton</title>
<body style="font:48px monospace;background:#111;color:#0f0">
<div id="info">…</div>
<script>
const hz = Number(new URLSearchParams(location.search).get('hz') || 440);
const ctx = new AudioContext();
const osc = ctx.createOscillator();
const gain = ctx.createGain();
osc.frequency.value = hz;
gain.gain.value = 0.25;
osc.connect(gain).connect(ctx.destination);
osc.start();
// L'animation donne aussi de quoi capturer une image : Desktop Duplication
// n'emet une trame qu'au changement du bureau.
let n = 0, t0 = performance.now();
function boucle(t) {
  n++;
  document.getElementById('info').textContent =
    `${hz} Hz · rAF ${(n / ((t - t0) / 1000)).toFixed(1)} /s · ${ctx.state}`;
  document.body.style.background = `hsl(${(n * 3) % 360} 40% 12%)`;
  requestAnimationFrame(boucle);
}
requestAnimationFrame(boucle);
addEventListener('click', () => ctx.resume());
</script>
```

⚠️ **`AudioContext` démarre `suspended` sans activation utilisateur.** Le pilote
doit appeler `ctx.resume()` par `Runtime.evaluate`, et **vérifier que
`ctx.state` vaut `running`** avant de commencer toute mesure — sinon on mesure
un silence et l'on conclut à tort à une panne d'isolation.

⚠️ **Un `--user-data-dir` par fenêtre est obligatoire**, sans quoi Chrome
rejoint son instance existante et l'on compte des lancements au lieu de
fenêtres. Leçon de la seconde recette de D4.

- [ ] **Étape 2 : le relevé de fréquence dominante**

Dans `pilote-recette-d7.mjs`, la fonction évaluée sur chaque page
d'application :

```js
// Branche un AnalyserNode sur la piste audio reçue et rend la fréquence du
// bin le plus énergique. C'est ce qui rend le critère ① mesurable au lieu de
// déclaratif : `bytesReceived` dit que du son arrive, jamais lequel.
const RELEVE_FREQUENCE = `(async () => {
  const pc = window.__pc;               // exposée par client/src/webrtc.ts
  if (!pc) return { erreur: 'aucune PeerConnection exposee' };
  const piste = pc.getReceivers()
    .map(r => r.track).find(t => t && t.kind === 'audio');
  if (!piste) return { erreur: 'aucune piste audio' };
  const ctx = new AudioContext();
  const analyseur = ctx.createAnalyser();
  analyseur.fftSize = 8192;
  ctx.createMediaStreamSource(new MediaStream([piste])).connect(analyseur);
  await new Promise(r => setTimeout(r, 1500));
  const bins = new Float32Array(analyseur.frequencyBinCount);
  analyseur.getFloatFrequencyData(bins);
  let meilleur = 0;
  for (let i = 1; i < bins.length; i++) if (bins[i] > bins[meilleur]) meilleur = i;
  return {
    hz: Math.round(meilleur * ctx.sampleRate / analyseur.fftSize),
    db: Math.round(bins[meilleur]),
    plancher_db: Math.round(bins.reduce((a, b) => a + b, 0) / bins.length),
  };
})()`;
```

⚠️ **Toute évaluation CDP sur une page portant un flux WebRTC actif doit être
BORNÉE** — elle peut ne **jamais** rendre. Envelopper chaque `Runtime.evaluate`
dans un `Promise.race` avec un délai, comme le fait déjà le pilote de D6.

⚠️ **Aucune capture d'écran CDP pendant une mesure.**

- [ ] **Étape 3 : vérifier que l'instrument peut échouer**

Le lancer contre **une seule** fenêtre, muette : il doit rendre une erreur ou un
plancher, **pas** un `hz` plausible. Un instrument qui ne peut pas échouer ne
contrôle rien — le piège du contrôle prématuré de D6.

- [ ] **Étape 4 : commettre**

```bash
git add docs/superpowers/plans/journaux-multifenetres-d7/instrument/
git commit -F - <<'EOF'
test(d7): l'instrument de recette releve la frequence dominante

bytesReceived dit que du son arrive, jamais lequel. Un AnalyserNode branche sur
la piste recue rend le critere d'isolation mesurable au lieu de declaratif.

La source affiche sa propre frequence et sa cadence de rAF : sans ce chiffre,
une capture muette et une source muette se lisent pareil.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
```

---

## Tâche 12 : mettre `CLAUDE.md` à jour, sur des chiffres RELEVÉS

**Fichiers**
- Modifier : `CLAUDE.md`

- [ ] **Étape 1 : relever, ne jamais recopier**

```bash
cd /home/mallanic/Projects/Guacamole && { git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>400'
```

- [ ] **Étape 2 : corriger le tableau de dette et les encadrés**

`wasapi.rs` **sort de la dette gelée** (tâche 1) : retirer sa ligne du tableau
et le dire. Reporter les tailles réellement relevées pour `table.rs`,
`sommeil.rs`, `fenetre.rs`, et les fichiers neufs.

⚠️ **Corriger une affirmation exige de la CHERCHER, pas de la corriger là où on
nous l'a montrée.** Ce fichier a payé quatre fois le naufrage du « 487 » : le
sommaire et le chapitre de détail doivent bouger **ensemble**. Balayer sur le
**sens** (« un nombre de lignes de `wasapi.rs` »), pas sur la formule.

- [ ] **Étape 3 : ajouter la section D7**

Après la section D6, une section « Sous-bloc D7 » qui porte : le verdict, les
chiffres **avec leur nombre d'exécutions**, ce que D7 **n'établit PAS** (spec
§10), les pièges neufs, et la recette d'entrée de D8 (le `grep` sur `actif=`).

- [ ] **Étape 4 : commettre**

```bash
git add CLAUDE.md
git commit -F - <<'EOF'
docs(d7): l'index durable, sur des chiffres releves par la commande

wasapi.rs sort de la dette gelee : l'extraction de la tache 1 le fait passer
sous les 500 lignes.

Sommaire et chapitre de detail corriges ENSEMBLE : ce fichier a paye quatre fois
le naufrage d'un nombre corrige a un seul endroit.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
```

---

## Tâche 13 : la recette

**Fichiers**
- Créer : `docs/superpowers/plans/journaux-multifenetres-d7/critere-*.log`
- Modifier : `docs/superpowers/plans/2026-08-03-multifenetres-audio-par-fenetre-resultats.md`

- [ ] **Étape 1 : préparer la VM et le binaire**

Reprendre les étapes 1 et 2 de la tâche 3, **y compris le contrôle de la taille
du binaire**. Purger les sorties virtuelles :

```bash
MULTIFENETRE_VDD_PURGE=1 scripts/run-agent.sh
```

Elles survivent à un `Stop-Process -Force` : sans purge, l'exécution démarre
avec un vivier déjà entamé.

- [ ] **Étape 2 : critère ① — isolation**

Deux fenêtres Chrome `--app`, deux `--user-data-dir` distincts, l'une sur
`ton.html?hz=440`, l'autre sur `ton.html?hz=880`. Relever la fréquence dominante
sur les deux pages navigateur.

**Attendu** : page A ≈ 440 Hz et **pas** 880 ; page B ≈ 880 et **pas** 440.
Reporter les deux valeurs et le `plancher_db`.

- [ ] **Étape 3 : critère ② — arbitrage par PID**

Deux fenêtres du **même** processus (même `--user-data-dir`). Relever quelle
piste croît, déplacer le focus, relever l'inversion.

⚠️ **La visibilité et le focus sont IMPOSÉS page par page** : un Chrome sans
interface rapporte `document.hidden = true` pour toute fenêtre d'arrière-plan.
**Faire passer la cible par `blur` puis `focus`** — la déduplication de
`client/src/visibilite.ts` peut sinon faire *disparaître* le focus.

⚠️ **Dimensionner le palier APRÈS lecture des constantes** : `PERIODE_REARBITRAGE`
vaut 250 ms, mais l'arbitrage traverse le canal, le tube et la boucle de
transport. **Attendre le FAIT** (l'inversion relevée), jamais une durée.

- [ ] **Étape 4 : critère ③ — l'audio survit au sommeil**

Ouvrir plus de `vivier::PLAFOND_EVEIL` (8) fenêtres, chacune jouant un ton.
Relever, sur la **même** session endormie : `framesDecoded` **figé** et
`bytesReceived` audio **en croissance**.

- [ ] **Étape 5 : critère ④ — le budget rendu**

```bash
grep 'ordre audio applique' agent.log | grep -c 'actif=false'
grep 'ordre audio applique' agent.log | grep -c 'actif=true'
```

- [ ] **Étape 6 : critère ⑤ — aucune régression mono-fenêtre**

Un agent lancé **sans** `SUPERVISEUR` ni `FENETRE_HWND`. Vérifier la ligne
`audio activé` avec un format de **session**, et une piste audio au navigateur.

⚠️ **Vérifier `Get-Process agent` avant chaque exécution** : `run-agent.sh` ne
tue pas l'agent existant, et un agent survit à l'hibernation de la VM. Sans ce
contrôle on mesure le processus précédent.

- [ ] **Étape 7 : contrôler la survie de la VM et copier les journaux**

Après **chaque** rang, et **après la fin réelle** de l'exécution (les enfants
meurent quand le navigateur se ferme, donc **après** la copie naïve).

- [ ] **Étape 8 : le contrôle qui doit pouvoir échouer**

```bash
grep -c 'compteurs audio' agent.log
grep 'compteurs audio' agent.log | grep -c 'actif=true'
```

Si le second vaut zéro alors que le premier ne le vaut pas, l'arbitrage est
figé. **Vérifier que ce contrôle est joué après qu'une fenêtre existe** — la
trace est périodique (30 s).

- [ ] **Étape 9 : écrire les résultats**

Compléter
`docs/superpowers/plans/2026-08-03-multifenetres-audio-par-fenetre-resultats.md`.

**Règles d'énoncé, non négociables :**
- **le nombre d'exécutions figure DANS chaque énoncé** ; aucun taux n'est
  revendiqué qui n'ait été mesuré ;
- distinguer **relevé** / **calculé** / **inféré**, à chaque phrase ;
- **énoncer la règle de sélection AVANT de compter** si un sous-ensemble est
  retenu — un sous-ensemble sans règle énoncée est un sous-ensemble **choisi** ;
- reproduire la section « ce que D7 n'établit PAS » de la spec §10, **augmentée
  de ce que la recette a effectivement laissé ouvert**.

- [ ] **Étape 10 : commettre**

```bash
git add docs/superpowers/plans/journaux-multifenetres-d7/ docs/superpowers/plans/2026-08-03-multifenetres-audio-par-fenetre-resultats.md
git commit -F - <<'EOF'
recette(d7): l'isolation audio par fenetre, mesuree a la frequence

Cinq criteres, chacun portant son nombre d'executions dans son enonce. Journaux
verses.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
```

---

## Auto-revue du plan

**Couverture de la spec** — chaque section a sa ou ses tâches :

| Spec | Tâches |
| --- | --- |
| §3 tâche 1, la mesure qui gouverne | 2, 3 |
| §4.2 le PID ne circule pas | 5 (capteur), 7 (enfant) |
| §4.3 l'arbitrage, règle pure | 4 ; branche : 5 |
| §4.4 cycle de vie, `Start`/`Stop` | 2 (capture), 7 (application) |
| §4.5 le protocole | 6 |
| §5 le budget | 8 |
| §6 repli silence, défaut muet observable | 7 (trace `actif=`/`pid=`), 13 (étape 8) |
| §7 dettes soldées : `audio_libre`, extraction `wasapi.rs` | 9, 1 |
| §7 consignation n°2 de D6 (span) | 10 |
| §8 tests hôte | 4, 5, 8 |
| §9 recette, instrument, critères | 11, 13 |
| §10 ce que D7 n'établit pas | 13 étape 9, 12 étape 3 |

**Cohérence des noms**, vérifiée d'un bout à l'autre : `FenetreAudio` /
`arbitrer` (t4) → `distribuer_l_audio` (t5) → `Message::Audio { actif }` (t5) →
`DepuisCapteur::Audio { actif }` (t6) → `Recu::Audio { actif }` (t6) →
`audio_a_appliquer()` (t6) → `appliquer_audio()` (t6 minimal, t7 complet) →
`AudioSource::set_actif()` (t7) → `WindowsAudioSource::emettre()` (t7) →
`CaptureProcessus::{demarrer, arreter}` (t2). `inscrire(session, pid)` change de
signature en t5, et t5 corrige **tous** ses appelants — y compris les tests de
`sommeil/parts.rs`.

**Dépendance dure** : les tâches 4 à 13 ne commencent qu'après le relevé ② de la
tâche 3. Les tâches 1 et 2 le précèdent nécessairement.
