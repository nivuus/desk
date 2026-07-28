//! Capture du son que joue la machine, par WASAPI en mode loopback.
//!
//! Le périphérique visé est le **rendu par défaut** de la session : on capte ce
//! qui sortirait des haut-parleurs, quelle que soit l'application qui le
//! produit.
//!
//! **Sondage, pas événement.** `AUDCLNT_STREAMFLAGS_EVENTCALLBACK` n'est pas
//! supporté en combinaison avec `AUDCLNT_STREAMFLAGS_LOOPBACK` : Microsoft
//! documente la capture loopback comme devant être pilotée par minuterie. Un
//! flux de rendu inactif ne signalerait d'ailleurs aucun événement, ce qui est
//! le cas fréquent ici — rien ne joue la plupart du temps. Le sondage est donc
//! la seule forme correcte, et il sert directement le complément de silence de
//! `frames.rs`.

#![cfg(windows)]

use anyhow::{bail, Context, Result};
use windows::Win32::Media::Audio::{
    eConsole, eRender, IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator,
    AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK,
    WAVEFORMATEX, WAVEFORMATEXTENSIBLE,
};
use windows::Win32::Media::Multimedia::KSDATAFORMAT_SUBTYPE_IEEE_FLOAT;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED,
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

pub struct LoopbackCapture {
    client: IAudioClient,
    capture: IAudioCaptureClient,
    canaux: usize,
    flottant: bool,
    description: String,
}

impl LoopbackCapture {
    /// Ouvre le loopback sur le périphérique de rendu par défaut et démarre la
    /// capture.
    pub fn open() -> Result<Self> {
        unsafe {
            // Résultat volontairement ignoré : `RPC_E_CHANGED_MODE` signifie
            // que COM est déjà initialisé sur ce fil, ce qui n'est pas une
            // erreur ici.
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

            let enumerateur: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                    .context("création de l'énumérateur de périphériques audio")?;
            let peripherique = enumerateur
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .context("aucun périphérique de rendu audio par défaut")?;
            let client: IAudioClient = peripherique
                .Activate(CLSCTX_ALL, None)
                .context("activation du client audio")?;

            let mix = client.GetMixFormat().context("lecture du format de mixage")?;
            let format: &WAVEFORMATEX = &*mix;
            let canaux = format.nChannels as usize;
            let frequence = format.nSamplesPerSec;
            let bits = format.wBitsPerSample;

            let flottant = if format.wFormatTag == WAVE_FORMAT_EXTENSIBLE {
                // WAVEFORMATEXTENSIBLE est repr(packed) : prendre une
                // référence sur SubFormat — ce que fait `==` sur un GUID —
                // est un accès non aligné, donc un comportement indéfini.
                let ext = mix as *const WAVEFORMATEXTENSIBLE;
                let sous_format = std::ptr::addr_of!((*ext).SubFormat).read_unaligned();
                sous_format == KSDATAFORMAT_SUBTYPE_IEEE_FLOAT
            } else {
                format.wFormatTag == WAVE_FORMAT_IEEE_FLOAT
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

            client
                .Initialize(
                    AUDCLNT_SHAREMODE_SHARED,
                    AUDCLNT_STREAMFLAGS_LOOPBACK,
                    BUFFER_DURATION_100NS,
                    0,
                    mix,
                    None,
                )
                .context("initialisation du client audio en loopback")?;

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
/// Mono : le canal est dupliqué. Plus de deux canaux : seuls les deux premiers
/// sont conservés (gauche et droite d'un flux multicanal, par convention
/// WAVE_FORMAT_EXTENSIBLE).
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

/// Flottant normalisé → entier 16 bits, avec écrêtage.
///
/// WASAPI ne garantit pas que les échantillons restent dans [-1, 1] : un
/// mixage de plusieurs flux peut dépasser. Sans écrêtage, la conversion
/// enroulerait et produirait un craquement au lieu d'une saturation.
fn vers_i16(v: f32) -> i16 {
    (v.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
}
