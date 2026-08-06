//! Création et acceptation des instances du tube nommé du capteur.
//!
//! **Extrait de `serveur.rs` le 6 août 2026**, qui était à 490 lignes pour un
//! plafond de 500 — `CLAUDE.md` exige pour ce fichier « une extraction, jamais
//! une compression du commentaire de `TAMPON` », et c'est bien le commentaire
//! de `TAMPON` qui part ici AVEC sa constante, auprès de laquelle il doit
//! rester. **Aucune valeur, aucun ordre d'opération n'a changé.**

use std::time::Duration;

use anyhow::{bail, Context, Result};
use windows::core::HRESULT;
use windows::Win32::Foundation::{ERROR_PIPE_CONNECTED, HANDLE};
use windows::Win32::Storage::FileSystem::PIPE_ACCESS_DUPLEX;
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE,
    PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};

use crate::capteur::protocole::NOM_TUBE;

/// Tampon de tube, dans les deux sens. **Dimensionné en unités d'accès, pas
/// en mégaoctets ronds** (correctif I4 de la revue finale de branche).
///
/// C'est ce tampon qui fixe la profondeur RÉELLE de la file d'images, et non
/// `CAPACITE_ECRITURES` ni `CAPACITE_FILE` : ces deux-là valent 8 et raisonnent
/// sur ≈90 ms de vidéo, mais un tampon OS plus grand les rend sans effet — il
/// se remplit derrière elles. À 1 MiB, une unité d'accès pesant 10 à 30 Ko, le
/// tube retenait 30 à 100 images, soit **0,3 à 1,1 s de vidéo en file**. Or
/// chaque image porte son instant de capture d'origine : un à-coup ne se
/// rattrape pas, il se rejoue en rafale d'images anciennes.
///
/// 128 Kio ramène cela à ≈4 à 12 images, du même ordre que les deux capacités
/// ci-dessus, sans descendre au point qu'un à-coup normal bloque le fil de
/// capture.
///
/// ⚠️ **La latence n'a jamais été mesurée sur ce chemin** : la recette du
/// sous-bloc D4 a relevé des cadences, jamais un délai de bout en bout. Ce
/// dimensionnement est un raisonnement sur des tailles d'unités d'accès
/// observées, pas un réglage calibré.
const TAMPON: u32 = 128 * 1024;

/// Souffle entre deux tentatives de création d'instance de tube après un échec
/// (correctif I5). Assez court pour qu'un échec transitoire ne coûte rien de
/// perceptible à l'enfant qui attend, assez long pour qu'un échec persistant
/// ne devienne pas une boucle serrée. Majorant assumé, non calibré.
pub(super) const SOUFFLE_CREATION_INSTANCE: Duration = Duration::from_millis(200);

/// Bloque jusqu'à ce qu'un enfant se connecte à `tube`.
///
/// **`ERROR_PIPE_CONNECTED` est un SUCCÈS déguisé en erreur.** Il signale
/// qu'un enfant s'est connecté dans l'intervalle entre `CreateNamedPipeW` et
/// cet appel — une course banale, attendue sous `PIPE_UNLIMITED_INSTANCES` —
/// et non un échec. Le confondre avec un échec réel tuerait le processus
/// capteur entier (donc les N fenêtres avec lui) à la première course.
pub(super) fn connecter(tube: HANDLE) -> Result<()> {
    match unsafe { ConnectNamedPipe(tube, None) } {
        Ok(()) => Ok(()),
        Err(erreur) if erreur.code() == HRESULT::from_win32(ERROR_PIPE_CONNECTED.0) => Ok(()),
        Err(erreur) => Err(erreur).context("attente d'un enfant"),
    }
}

pub(super) fn creer_instance() -> Result<HANDLE> {
    let nom: Vec<u16> = NOM_TUBE.encode_utf16().chain(std::iter::once(0)).collect();
    let tube = unsafe {
        CreateNamedPipeW(
            windows::core::PCWSTR(nom.as_ptr()),
            PIPE_ACCESS_DUPLEX,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
            PIPE_UNLIMITED_INSTANCES,
            TAMPON,
            TAMPON,
            0,
            None,
        )
    };
    if tube.is_invalid() {
        bail!("CreateNamedPipeW a rendu un handle invalide");
    }
    Ok(tube)
}
