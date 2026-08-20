//! Capture du son que joue la machine, par WASAPI en mode loopback.
//!
//! Le périphérique visé est **celui que désigne `AUDIO_PERIPHERIQUE`**, ou le
//! rendu par défaut de la session à défaut : on capte ce qui sortirait de ce
//! périphérique, quelle que soit l'application qui le produit.
//!
//! ⚠️ **Ce fichier a longtemps dit « le rendu par défaut » sans condition, et
//! c'était une dépendance implicite qui s'est retournée** : l'installation de
//! VB-Cable sur la VM (19 août 2026, préparation du chantier E) a fait
//! basculer ce défaut sur un câble virtuel que rien n'alimente, et le produit
//! s'est mis à capter du silence sans qu'aucune ligne ne le dise. La
//! résolution vit désormais dans `wasapi/rendu.rs`, et la règle qui élit —
//! pure, éprouvée sur l'hôte — dans `wasapi/peripherique.rs`.
//!
//! **Sondage, pas événement.** `AUDCLNT_STREAMFLAGS_EVENTCALLBACK` n'est pas
//! supporté en combinaison avec `AUDCLNT_STREAMFLAGS_LOOPBACK` : Microsoft
//! documente la capture loopback comme devant être pilotée par minuterie. Un
//! flux de rendu inactif ne signalerait d'ailleurs aucun événement, ce qui est
//! le cas fréquent ici — rien ne joue la plupart du temps. Le sondage est donc
//! la seule forme correcte, et il sert directement le complément de silence de
//! `frames.rs`.

#![cfg(windows)]

/// L'ÉCRITURE d'échantillons sur un point de terminaison de rendu : la moitié
/// Windows du microphone (bloc E2). Miroir de `LoopbackCapture` ci-dessous —
/// celui-ci lit ce que la machine joue, celui-là fait jouer à la machine ce
/// que le navigateur envoie.
pub mod ecriture;
pub mod process_loopback;
/// Résolution d'un point de terminaison audio de **rendu** — celui que capte
/// le loopback (correction « A-bis »), **et** celui du câble sur lequel le
/// micro écrit (bloc E2). Deux consommateurs, deux politiques de repli
/// opposées — le premier se replie, le second refuse : voir l'en-tête du
/// module, qui porte la table et la raison.
pub mod rendu;

use anyhow::{bail, Context, Result};
use windows::Win32::Media::Audio::{
    IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator,
    AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK,
    WAVEFORMATEX, WAVEFORMATEXTENSIBLE,
};
use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::Media::Multimedia::KSDATAFORMAT_SUBTYPE_IEEE_FLOAT;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED,
};

use crate::opus::{CHANNELS, SAMPLE_RATE_HZ};

/// Durée du tampon demandé à WASAPI, en unités de 100 ns. 200 ms : large
/// marge pour absorber un tour de boucle en retard sans jamais perdre
/// d'échantillon.
const BUFFER_DURATION_100NS: i64 = 2_000_000;

/// Étiquette `WAVE_FORMAT_EXTENSIBLE` du champ `wFormatTag`.
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;
/// Étiquette `WAVE_FORMAT_IEEE_FLOAT`.
const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;

/// Garde RAII pour le pointeur rendu par `IAudioClient::GetMixFormat`.
///
/// Ce pointeur est alloué par COM via `CoTaskMemAlloc` (documentation de
/// `GetMixFormat`) : l'appelant doit le libérer par `CoTaskMemFree`, ce que
/// ce garde fait dans son `Drop`. `IAudioClient::Initialize` copie le format
/// en interne, donc libérer *après* son appel est toujours correct — y
/// compris à la sortie normale de `open()`, où ce garde n'est libéré qu'en
/// toute fin de fonction.
///
/// L'intérêt d'un garde plutôt qu'une libération explicite : entre
/// `GetMixFormat` et `Initialize`, `open()` peut sortir en erreur par deux
/// `bail!` (fréquence ou largeur de format refusée) — exactement les
/// chemins qu'on emprunte le jour où la VM change de configuration audio.
/// Une libération posée seulement en fin de fonction heureuse les
/// manquerait ; un garde RAII les couvre par construction, quel que soit le
/// chemin de sortie (`return`, `?`, `bail!`).
struct FormatMixage(*mut WAVEFORMATEX);

impl std::ops::Deref for FormatMixage {
    type Target = WAVEFORMATEX;
    fn deref(&self) -> &WAVEFORMATEX {
        // SAFETY : le pointeur vient d'un `GetMixFormat` réussi et n'est
        // libéré que dans `Drop`, donc valide pour toute la durée de vie du
        // garde.
        unsafe { &*self.0 }
    }
}

impl Drop for FormatMixage {
    fn drop(&mut self) {
        unsafe { CoTaskMemFree(Some(self.0 as *const _)) };
    }
}

pub struct LoopbackCapture {
    client: IAudioClient,
    capture: IAudioCaptureClient,
    canaux: usize,
    flottant: bool,
    description: String,
}

// SÉCURITÉ : `LoopbackCapture` enveloppe des interfaces COM (`IAudioClient`,
// `IAudioCaptureClient`) que `windows-core` ne marque pas `Send` par défaut —
// un objet COM générique peut être lié à un appartement mono-thread (STA), et
// le déplacer vers un autre fil serait alors un comportement indéfini. Ce
// n'est pas le cas ici : `open()` (ci-dessous) *vérifie*, plutôt que de
// supposer, que le fil appelant rejoint l'appartement multi-thread (MTA) via
// `CoInitializeEx(None, COINIT_MULTITHREADED)`, et refuse d'ouvrir si ce fil
// appartient déjà à un autre appartement (`RPC_E_CHANGED_MODE`).
// `WindowsAudioSource::new` (agent/src/windows_audio.rs) déplace ensuite cet
// objet, par `move`, vers un fil de capture dédié qui rejoint à son tour
// cette même MTA avant tout appel COM (voir son commentaire). Un objet créé
// dans une MTA est par construction appelable depuis n'importe quel fil qui
// en est membre, sans marshaling — c'est cette propriété, garantie par la
// vérification d'`open()`, qui rend le transfert sûr.
//
// Cette promesse porte sur le **struct entier, champs futurs compris** : si
// un futur champ ajoute un `HANDLE` d'événement, un pointeur brut, ou tout
// autre état lié à un fil précis plutôt qu'à l'appartement, cet `unsafe impl`
// cesserait d'être valide sans que rien ne le signale. Quiconque ajoute un
// champ à `LoopbackCapture` doit vérifier qu'il reste utilisable depuis
// n'importe quel fil membre de la MTA avant de le faire — sans quoi ce
// `Send` doit être retiré ou restreint.
//
// Alternative écartée : `windows_core::AgileReference<T>`, le mécanisme
// officiellement prévu par `windows-core` 0.62 pour transporter un objet COM
// entre fils sans supposer son modèle de threading. Non retenu ici : il exige
// une résolution (`resolve()`, un `QueryInterface` interne) à chaque
// récupération, un coût et une complexité inutiles alors que ce processus n'a,
// sous ce plan, aucune STA — la vérification d'`open()` suffit et reste bon
// marché.
unsafe impl Send for LoopbackCapture {}

impl LoopbackCapture {
    /// Ouvre le loopback sur le périphérique qu'élit `rendu::resoudre` — celui
    /// que désigne `AUDIO_PERIPHERIQUE`, ou le rendu par défaut de Windows à
    /// défaut — et démarre la capture.
    ///
    /// ⚠️ Cette ligne a dit « le périphérique de rendu par défaut » jusqu'à la
    /// clôture du chantier E, alors que l'en-tête de ce module, douze lignes
    /// plus haut, disait déjà l'inverse depuis la correction « A-bis ».
    pub fn open() -> Result<Self> {
        unsafe {
            // `CoInitializeEx` doit être **vérifié**, pas ignoré : c'est la
            // précondition dont dépend `unsafe impl Send for LoopbackCapture`
            // ci-dessus (lire son commentaire d'abord si ce n'est pas fait).
            // `S_OK` (ce fil vient de rejoindre la MTA) et `S_FALSE` (il en
            // était déjà membre) sont tous deux acceptables : dans les deux
            // cas, ce fil est membre de l'appartement multi-thread — le même
            // que rejoindra le fil de capture de `windows_audio.rs`. Seul
            // `RPC_E_CHANGED_MODE` — ce fil appartient déjà à un autre
            // appartement, typiquement une STA liée par un appel antérieur à
            // `CoInitializeEx(..., COINIT_APARTMENTTHREADED)` sur ce même fil
            // — doit faire échouer l'ouverture : sans ce refus,
            // `LoopbackCapture` migrerait d'une STA vers la MTA du fil de
            // capture sans marshaling, un comportement indéfini qu'aucun test
            // ne révèle puisque l'appel par vtable directe « marche » la
            // plupart du temps même quand c'est interdit.
            let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
            if hr == RPC_E_CHANGED_MODE {
                bail!(
                    "ouverture du loopback audio refusée : le fil appelant appartient déjà \
                     à un appartement à thread unique (STA), pas à l'appartement \
                     multi-thread (MTA) qu'exige `LoopbackCapture`. `WindowsAudioSource::new` \
                     (agent/src/windows_audio.rs) déplace cet objet, par `move`, vers un fil \
                     de capture dédié qui rejoint la MTA : migrer un objet COM d'une STA vers \
                     un autre appartement sans marshaling est un comportement indéfini, pas \
                     seulement une erreur de type. Vérifiez qu'aucun \
                     `CoInitializeEx(..., COINIT_APARTMENTTHREADED)` (ni aucune autre \
                     initialisation qui lie ce fil à une STA, par exemple une init WinRT \
                     implicite) n'a précédé cet appel sur ce même fil."
                );
            }

            // Pas de `CoUninitialize` en regard, et c'est délibéré : ce fil
            // n'est pas forcément celui qui utilisera ni celui qui libérera
            // l'objet rendu. La tâche 6 (`windows_audio.rs`) appelle `open()`
            // sur le fil appelant de `WindowsAudioSource::new()`, puis
            // déplace le `LoopbackCapture` obtenu par `move` vers un fil de
            // capture dédié — c'est CE fil-là qui appelle `read()` en boucle
            // et qui exécute `Drop` en sortant. Appeler `CoUninitialize` dans
            // `Drop` s'exécuterait donc sur un fil différent de celui qui a
            // appelé `CoInitializeEx`, ce que COM interdit explicitement.
            // Conséquence acceptée : si le fil appelant de `open()` est
            // recyclé entre sessions (fil d'un pool, par ex. les workers
            // bloquants de tokio), son compte de références COM croît d'une
            // unité par session — jamais celui du fil de capture, qui lui
            // n'est jamais recyclé (créé et détruit une fois par session). Un
            // futur rééquilibrage devra se faire là où l'appel est
            // réellement possédé : autour du fil de `WindowsAudioSource::new`,
            // pas ici.

            let enumerateur: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                    .context("création de l'énumérateur de périphériques audio")?;
            // Correction « A-bis » : plus de `GetDefaultAudioEndpoint` en dur
            // ici. `rendu::resoudre` honore `AUDIO_PERIPHERIQUE` quand elle
            // est posée, retombe sur le défaut de Windows sinon (comportement
            // d'avant, inchangé), et TRACE dans tous les cas le périphérique
            // réellement retenu — y compris quand il s'agit d'un repli.
            let peripherique = rendu::resoudre(&enumerateur)?;
            let client: IAudioClient = peripherique
                .Activate(CLSCTX_ALL, None)
                .context("activation du client audio")?;

            let mix = FormatMixage(
                client
                    .GetMixFormat()
                    .context("lecture du format de mixage")?,
            );
            let canaux = mix.nChannels as usize;
            let frequence = mix.nSamplesPerSec;
            let bits = mix.wBitsPerSample;

            let flottant = if mix.wFormatTag == WAVE_FORMAT_EXTENSIBLE {
                // WAVEFORMATEXTENSIBLE est repr(packed) : prendre une
                // référence sur SubFormat — ce que fait `==` sur un GUID —
                // est un accès non aligné, donc un comportement indéfini.
                let ext = mix.0 as *const WAVEFORMATEXTENSIBLE;
                let sous_format = std::ptr::addr_of!((*ext).SubFormat).read_unaligned();
                sous_format == KSDATAFORMAT_SUBTYPE_IEEE_FLOAT
            } else {
                mix.wFormatTag == WAVE_FORMAT_IEEE_FLOAT
            };

            let description = format!(
                "{frequence} Hz, {canaux} canaux, {bits} bits, {}",
                if flottant { "flottant" } else { "entier" }
            );

            if frequence != SAMPLE_RATE_HZ {
                bail!(
                    "format de mixage à {frequence} Hz : seul {SAMPLE_RATE_HZ} Hz est supporté \
                     (aucun rééchantillonneur n'est embarqué)"
                );
            }
            if !flottant && bits != 16 {
                bail!("format de mixage entier {bits} bits non supporté ({description})");
            }
            if flottant && bits != 32 {
                bail!("format de mixage flottant {bits} bits non supporté ({description})");
            }

            client
                .Initialize(
                    AUDCLNT_SHAREMODE_SHARED,
                    AUDCLNT_STREAMFLAGS_LOOPBACK,
                    BUFFER_DURATION_100NS,
                    0,
                    mix.0,
                    None,
                )
                .context("initialisation du client audio en loopback")?;
            // `mix` (le garde `FormatMixage`) sort de portée en fin de bloc
            // `unsafe` et libère alors le format par `CoTaskMemFree` — après
            // `Initialize`, qui en a fait sa propre copie interne, comme
            // l'exige la documentation de `GetMixFormat`.

            let capture: IAudioCaptureClient = client
                .GetService()
                .context("obtention du service de capture")?;
            client.Start().context("démarrage de la capture")?;

            Ok(Self {
                client,
                capture,
                canaux,
                flottant,
                description,
            })
        }
    }

    /// Format réellement obtenu, pour le journal et la sonde.
    pub fn description(&self) -> String {
        self.description.clone()
    }

    /// Lit le paquet disponible suivant, converti en entiers 16 bits
    /// entrelacés stéréo. Rend `None` quand rien n'est disponible — le cas
    /// courant quand aucune application ne joue.
    pub fn read(&mut self) -> Result<Option<Vec<i16>>> {
        unsafe {
            let dispo = self
                .capture
                .GetNextPacketSize()
                .context("interrogation du paquet suivant")?;
            if dispo == 0 {
                return Ok(None);
            }

            let mut donnees: *mut u8 = std::ptr::null_mut();
            let mut images: u32 = 0;
            let mut drapeaux: u32 = 0;
            self.capture
                .GetBuffer(&mut donnees, &mut images, &mut drapeaux, None, None)
                .context("lecture du tampon de capture")?;

            let muet = drapeaux & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0;
            let sortie = if muet {
                vec![0i16; images as usize * CHANNELS]
            } else {
                let brut = images as usize * self.canaux;
                if self.flottant {
                    let source = std::slice::from_raw_parts(donnees as *const f32, brut);
                    convertir_flottant(source, self.canaux)
                } else {
                    let source = std::slice::from_raw_parts(donnees as *const i16, brut);
                    convertir_entier(source, self.canaux)
                }
            };

            self.capture
                .ReleaseBuffer(images)
                .context("libération du tampon de capture")?;
            Ok(Some(sortie))
        }
    }
}

impl Drop for LoopbackCapture {
    fn drop(&mut self) {
        unsafe {
            let _ = self.client.Stop();
        }
    }
}

/// Convertit des échantillons flottants en entiers 16 bits stéréo entrelacés.
///
/// Mono : le canal est dupliqué. Plus de deux canaux : **troncature**, pas
/// sous-mixage — seuls les deux premiers canaux (gauche et droite d'un flux
/// multicanal, par convention WAVE_FORMAT_EXTENSIBLE) sont conservés tels
/// quels ; l'énergie des canaux surround n'est mélangée dans aucun des deux,
/// elle est simplement ignorée. Sans conséquence en pratique : le format
/// réel de cette VM est déjà stéréo.
fn convertir_flottant(source: &[f32], canaux: usize) -> Vec<i16> {
    let images = source.len() / canaux.max(1);
    let mut sortie = Vec::with_capacity(images * CHANNELS);
    for i in 0..images {
        let base = i * canaux;
        let g = source[base];
        let d = if canaux >= 2 { source[base + 1] } else { g };
        sortie.push(vers_i16(g));
        sortie.push(vers_i16(d));
    }
    sortie
}

/// Même conversion, pour une source déjà en entiers 16 bits.
fn convertir_entier(source: &[i16], canaux: usize) -> Vec<i16> {
    let images = source.len() / canaux.max(1);
    let mut sortie = Vec::with_capacity(images * CHANNELS);
    for i in 0..images {
        let base = i * canaux;
        let g = source[base];
        let d = if canaux >= 2 { source[base + 1] } else { g };
        sortie.push(g);
        sortie.push(d);
    }
    sortie
}

/// Flottant normalisé → entier 16 bits, avec écrêtage explicite.
///
/// WASAPI ne garantit pas que les échantillons restent dans [-1, 1] : un
/// mixage de plusieurs flux peut dépasser. Le cast `as` sature déjà
/// nativement depuis Rust 1.45 (une valeur hors bornes est ramenée à
/// `i16::MIN`/`i16::MAX`, jamais enroulée) : le `clamp` explicite ne sert
/// donc pas à éviter un dépassement silencieux, mais à fixer la borne haute
/// exactement sur `i16::MAX` — le cast seul, sans clamp, saturerait vers le
/// bas jusqu'à `i16::MIN`, un LSB plus loin que `-i16::MAX` — et à rendre
/// l'intention explicite plutôt que de reposer sur ce détail de sémantique
/// de `as`.
fn vers_i16(v: f32) -> i16 {
    (v.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
}
