//! Construction et branchement de la source audio d'une session.
//!
//! Extrait de `demarrage.rs` à la tâche 7 du sous-bloc D7 : l'ajout du mode
//! par-processus (deux embranchements, plus l'aide `pid_de_fenetre`) portait
//! le fichier parent au-dessus du plafond de 500 lignes du dépôt — même
//! règle, même remède que `demarrage/source.rs` (voir son commentaire de
//! tête) : l'addition s'accompagne de son extraction plutôt que d'une
//! compression du commentaire qu'elle porte.

#![cfg(windows)]

use std::time::Instant;

use crate::transport::Session;
use crate::{windows_audio, Config};

/// Ouvre la source audio adaptée au mode de l'agent, et la branche sur
/// `session` si l'ouverture réussit.
///
/// Son absence ne compromet jamais la session vidéo : sur une source de test
/// (`TEST_FILE`), il n'y a rien à capter, et si la capture refuse de
/// s'ouvrir, on journalise et la session continue, muette.
///
/// **Deux modes, et le repli n'est JAMAIS le mix global.** Avec
/// `FENETRE_HWND`, l'enfant capte le son du seul processus propriétaire de sa
/// fenêtre, et le capteur arbitre entre les fenêtres qui en partagent un.
/// Sans, il capte le mix de la session : c'est le mode mono-fenêtre d'avant
/// le sous-bloc D7, et il ne doit pas régresser.
///
/// Retomber sur le mix global quand le process loopback échoue ferait
/// entendre à une fenêtre le son de TOUTES les autres, sous couvert
/// d'isolation — c'est le repli explicitement écarté au cadrage.
pub(super) fn brancher(config: &Config, session: &mut Session, clock_origin: Instant) {
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
                tracing::info!(
                    format = source_audio.description(),
                    pid = source_audio.pid(),
                    "audio activé"
                );
                session.set_audio_source(Box::new(source_audio));

                // Mode MONO-FENÊTRE : aucun capteur n'arbitrera jamais cette
                // session, donc `appliquer_audio` — l'unique écrivain de
                // `audio_porteuse` côté `transport` — n'y sera jamais appelée
                // (son garde est `VideoSource::audio_a_appliquer`, qui rend
                // `None` par défaut et n'a `capteur/distante.rs` pour seule
                // surcharge). Sans cette ligne, toute capture reconstruite est
                // remise au silence par le réarmement
                // `set_actif(self.audio_porteuse)` de
                // `reconstruire_ou_signaler`, et le remède de D10 est INERTE
                // dans le cas MAJORITAIRE — une application, une fenêtre.
                // Leg 4 de D10.
                //
                // ⚠️ `is_none()` et non un `match` recopié : le `match` du
                // choix de source ci-dessus a déjà consommé la valeur, et
                // refaire ici un troisième embranchement sur le mode
                // dupliquerait ce que ce fichier a déjà payé deux fois.
                if config.fenetre_hwnd.is_none() {
                    session.set_audio_porteuse(true);
                }

                // Le MÊME choix de mode que ci-dessus, refait à l'identique.
                // Le repli n'est JAMAIS le mix global : une fenêtre qui
                // entendrait toutes les autres sous couvert d'isolation est
                // l'arbitrage explicitement écarté au cadrage de D7.
                let hwnd = config.fenetre_hwnd;
                session.set_audio_reconstructeur(Box::new(move || {
                    let source = match hwnd {
                        Some(hwnd) => {
                            windows_audio::WindowsAudioSource::pour_processus(
                                pid_de_fenetre(hwnd)?,
                                clock_origin,
                            )?
                        }
                        None => windows_audio::WindowsAudioSource::new(clock_origin)?,
                    };
                    Ok(Box::new(source) as Box<dyn crate::audio::AudioSource + Send>)
                }));
            }
            Err(e) => {
                tracing::warn!(erreur = %e, "audio indisponible, la session continue sans son");
            }
        }
    }
    if !config.audio {
        tracing::info!("son désactivé sur cet agent par AUDIO=0");
    }
}

/// Le PID du processus propriétaire d'une fenêtre.
///
/// Dérivé du `hwnd`, exactement comme le capteur le dérive de son côté
/// (`capteur/fenetre.rs`). Le PID ne circule sur aucun protocole : deux
/// dérivations indépendantes du même identifiant stable valent mieux qu'un
/// champ de plus à tenir cohérent.
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
