//! Le fil de trace du chemin réel de l'enfant (`SOURCE_TRACE=1`), extrait de
//! `demarrage.rs` par le sous-bloc P2 du chantier presse-papier : ce fichier
//! était à 491 lignes pour un plafond de 500, et P2 y ajoute la lecture de
//! `PRESSE_PAPIER` et un champ aux trois `capabilities`. L'extraction précède
//! l'addition, comme la règle du dépôt l'exige.
//!
//! **Le jumeau CAPTEUR de ce fichier existe déjà** : `capteur/fenetre/trace.rs`,
//! né du sous-bloc D9, lit `Telemetrie` là où la source vit réellement. Les deux
//! côtés de `SOURCE_TRACE` ont donc désormais chacun leur module.
//!
//! ⚠️ Ce module est un enfant `#[cfg(windows)]` ordinaire, déclaré par un `mod`
//! simple **à l'intérieur** de son parent : la « Convention de module enfant »
//! de `CLAUDE.md` ne le concerne pas — il n'a jamais besoin de sortir de l'arbre
//! de son parent pour se compiler sur l'hôte.

use std::time::Duration;

use crate::{capture, encode, windows_source};

// Surveillance du chemin réel (`SOURCE_TRACE=1`) : ce qui reste ici après
// la tâche 11 de D9 est la partie ENCODEUR (tentatives/accumulation de
// capture, entrées/sorties du convertisseur et de l'encodeur). La cadence
// de `next_frame`, les captures neuves et les unités produites — les trois
// compteurs qui vivaient dans `windows_source::TICKS`/`CAPTURED`/
// `PRODUCED` — sont partis avec eux : ce fil-ci est dans l'ENFANT, qui n'a
// plus de `WindowsSource` depuis D4, donc plus rien à en lire. Ils sont
// désormais un champ PAR SESSION sur `WindowsSource` lui-même
// (`telemetrie`, voir `windows_source/telemetrie.rs`), tracés côté
// CAPTEUR par `capteur/fenetre.rs`, où la source vit réellement.
//
// ⚠️ Les QUINZE compteurs que ce fil lit encore (`capture::ATTEMPTS`/
// `HITS`/`ACCUMULATED`, `encode::NEED_INPUT_EVENTS`/`ENCODER_INPUTS`/
// `DROPPED_STALE`/`CONVERTER_*`/`*_NS`, `windows_source::CAPTURE_NS`/
// `SUBMIT_NS`/`DRAIN_NS`) sont morts pour la MÊME raison que les trois
// ci-dessus : ce sont des statiques du crate, écrites uniquement par
// `WindowsSource` (`capture.rs:311,328`, `encode.rs:522,850,868`), et
// `demarrage/source.rs` dit en toutes lettres que « l'enfant ne touche
// plus ni DXGI ni Media Foundation » en mode multi-fenêtres. Ce fil ne
// peut donc rien y lire non plus, en mode multi-fenêtres.
//
// Second effet, réciproque : en mode MONO-fenêtre, ce fil touche bien
// DXGI/MF en process (via `WindowsSource`) et ces quinze compteurs y sont
// vivants — mais `Telemetrie` (le remplaçant par session de `TICKS`/
// `CAPTURED`/`PRODUCED`) n'a qu'un seul lecteur, `capteur/fenetre/
// trace.rs`, qui ne tourne que dans le CAPTEUR. Le chemin mono-fenêtre a
// donc perdu ces trois compteurs-là, sans que rien ne les y remplace.
pub(super) fn brancher() -> Option<std::thread::JoinHandle<()>> {
    std::env::var("SOURCE_TRACE").is_ok().then(|| {
        std::thread::spawn(|| {
            use std::sync::atomic::Ordering::Relaxed;
            let (mut a0, mut h0, mut ac0) = (0u64, 0u64, 0u64);
            let (mut ni0, mut ei0, mut ds0) = (0u64, 0u64, 0u64);
            let (mut cn0, mut sn0, mut dn0) = (0u64, 0u64, 0u64);
            let (mut cv0, mut in0, mut out0) = (0u64, 0u64, 0u64);
            let (mut ci0, mut co0, mut cs0) = (0u64, 0u64, 0u64);
            loop {
                std::thread::sleep(Duration::from_secs(2));
                let (a, h, ac) = (
                    capture::ATTEMPTS.load(Relaxed),
                    capture::HITS.load(Relaxed),
                    capture::ACCUMULATED.load(Relaxed),
                );
                let (ni, ei, ds) = (
                    encode::NEED_INPUT_EVENTS.load(Relaxed),
                    encode::ENCODER_INPUTS.load(Relaxed),
                    encode::DROPPED_STALE.load(Relaxed),
                );
                let (cap_ns, sub_ns, dr_ns) = (
                    windows_source::CAPTURE_NS.load(Relaxed),
                    windows_source::SUBMIT_NS.load(Relaxed),
                    windows_source::DRAIN_NS.load(Relaxed),
                );
                let (ci, co, cs) = (
                    encode::CONVERTER_INPUTS.load(Relaxed),
                    encode::CONVERTER_OUTPUTS.load(Relaxed),
                    encode::CONVERTER_SKIPPED.load(Relaxed),
                );
                let (cv_ns, in_ns, out_ns) = (
                    encode::CONVERT_NS.load(Relaxed),
                    encode::ENC_IN_NS.load(Relaxed),
                    encode::ENC_OUT_NS.load(Relaxed),
                );
                // Part de la fenêtre d'observation (2 s = 2e9 ns) réellement
                // passée dans chaque appel : c'est ce qui distingue un étage
                // qui sature d'un étage qui attend.
                let pct = |now: u64, prev: u64| (now - prev) as f64 / 2e9 * 100.0;
                tracing::info!(
                    acquire_hz = (a - a0) as f64 / 2.0,
                    hits_hz = (h - h0) as f64 / 2.0,
                    // Mises à jour du bureau réellement survenues, y compris
                    // celles que DXGI a fusionnées : c'est ce chiffre qui dit
                    // si la fenêtre produit plus que ce qu'on en récupère.
                    desktop_updates_hz = (ac - ac0) as f64 / 2.0,
                    need_input_hz = (ni - ni0) as f64 / 2.0,
                    encoder_inputs_hz = (ei - ei0) as f64 / 2.0,
                    dropped_stale_hz = (ds - ds0) as f64 / 2.0,
                    conv_in_hz = (ci - ci0) as f64 / 2.0,
                    conv_out_hz = (co - co0) as f64 / 2.0,
                    conv_skipped_hz = (cs - cs0) as f64 / 2.0,
                    capture_pct = pct(cap_ns, cn0),
                    submit_pct = pct(sub_ns, sn0),
                    drain_pct = pct(dr_ns, dn0),
                    convert_pct = pct(cv_ns, cv0),
                    enc_in_pct = pct(in_ns, in0),
                    enc_out_pct = pct(out_ns, out0),
                    "cadence de la source (chemin réel)"
                );
                (a0, h0, ac0) = (a, h, ac);
                (ni0, ei0, ds0) = (ni, ei, ds);
                (cn0, sn0, dn0) = (cap_ns, sub_ns, dr_ns);
                (cv0, in0, out0) = (cv_ns, in_ns, out_ns);
                (ci0, co0, cs0) = (ci, co, cs);
            }
        })
    })
}
