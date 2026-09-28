//! Pure bridge between the frames of the sensor's media connection and the `Recu`
//! consumed by `SourceDistante`.
//!
//! **No `#[cfg(windows)]`, extracted from `tube.rs` on purpose**: this
//! function knows neither Windows nor the named pipe carrying it — it takes
//! any `R: std::io::Read` — and it is precisely decision
//! logic (translating a byte stream into `Recu`, deciding when
//! to abandon the thread) that the module's doctrine (`capteur.rs`) reserves for
//! code outside `cfg`, testable on the host. `tube.rs` stays gated: it alone
//! opens the real pipe and launches the thread that calls `lire_le_media`.

use std::io::Read;
use std::sync::mpsc::SyncSender;

use crate::capteur::distante::Recu;
use crate::capteur::protocole::{lire_trame, DepuisCapteur, Trame};

/// Reads the media connection — and **nothing else**: frames and states. A
/// command reply does not go through it, it is read by `commander` on the
/// command connection.
///
/// `pub(crate)`, not `pub`: only `tube.rs` calls it, from the same crate.
pub(crate) fn lire_le_media<R: Read>(mut lecteur: R, images: SyncSender<Recu>) {
    loop {
        let trame = match lire_trame(&mut lecteur) {
            Ok(trame) => trame,
            // End of pipe: the sensor has gone. Dropping the sender
            // makes `SourceDistante` return `Disconnected`, which OPENS ITS
            // RESUMPTION WINDOW instead of closing the session.
            Err(_) => return,
        };
        let envoi = match trame {
            Trame::Image(unite) => images.send(Recu::Image(unite)).is_ok(),
            Trame::Json(octets) => match serde_json::from_slice::<DepuisCapteur>(&octets) {
                Ok(DepuisCapteur::Etat {
                    vivante,
                    epuisee,
                    largeur,
                    hauteur,
                }) => images
                    .send(Recu::Etat {
                        vivante,
                        epuisee,
                        largeur,
                        hauteur,
                    })
                    .is_ok(),
                // MANDATORY passage point for every `DepuisCapteur` variant
                // pushed on the media connection: forgetting it here is NOT
                // reported by a compile error, but by a thread that dies
                // silently (`Ok(autre)` branch below) at the first message
                // of that type received — and this thread is the one feeding
                // `SourceDistante`, so the session falls into its resumption
                // window without any real failure. It is exactly the omission
                // that escaped task 7 of sub-block D5: `Sommeil` was
                // wired end to end on the sensor side and on the `SourceDistante` side,
                // but never connected here — and it is this lack of test
                // coverage that let it through, hence the extraction of this
                // file out of `#[cfg(windows)]`.
                Ok(DepuisCapteur::Sommeil { endormie, raison }) => {
                    images.send(Recu::Sommeil { endormie, raison }).is_ok()
                }
                // Connected by the fix of task 5/D6: `DepuisCapteur::Part`
                // was already wired on the sensor side (protocol + window thread)
                // but never connected HERE, exactly the omission that the
                // comment above already reported for `Sommeil` in D5.
                // Without this arm, the first share — sent by
                // `sommeil::inscrire` at attach time, before any frame —
                // fell into `Ok(autre)` and killed this thread at the very first
                // message received, under product conditions and on every session.
                Ok(DepuisCapteur::Part { bps }) => images.send(Recu::Part { bps }).is_ok(),
                // Task 6, sub-block D7: same mandatory passage point as
                // `Sommeil` and `Part` just above — forgetting it here would kill
                // this thread silently at the first audio order received.
                Ok(DepuisCapteur::Audio { actif }) => images.send(Recu::Audio { actif }).is_ok(),
                // Task 6, sub-block D8: same mandatory passage point as
                // `Sommeil`, `Part` and `Audio` just above — forgetting it here
                // would kill this thread silently at the first fullscreen
                // change received.
                Ok(DepuisCapteur::PleinEcran { actif }) => {
                    images.send(Recu::PleinEcran { actif }).is_ok()
                }
                // Task 9, sub-block P1 (clipboard): **the FIFTH time
                // this passage point has to be connected**, after `Sommeil`
                // (D5), `Part` (D6), `Audio` (D7) and `PleinEcran` (D8). Each
                // of the four previous ones carries its warning just above
                // — ⚠️ but NONE names its rank, contrary to what
                // this sentence first claimed (cross-cutting review, 20 August
                // 2026): it is this occurrence that starts the count.
                // Each was paid for the same way: the missing arm is reported
                // by NO compile error — it makes the
                // message fall into `Ok(autre)` below, which kills this thread
                // silently, starves `SourceDistante` and throws the session into its
                // resumption window without any failure showing.
                // The corresponding RED was played before this arm (E13):
                // `lire_le_media_survit_a_un_presse_papier_et_le_transmet`
                // returned `RecvError` on the very first announcement.
                Ok(DepuisCapteur::PressePapier { texte, octets }) => {
                    images.send(Recu::PressePapier { texte, octets }).is_ok()
                }
                // Sub-block A1: **the SIXTH time** this passage point
                // has to be connected, after `Sommeil` (D5), `Part` (D6), `Audio`
                // (D7), `PleinEcran` (D8) and `PressePapier` (P1). The RED was
                // played BEFORE this arm:
                // `lire_le_media_survit_a_un_accent_et_le_transmet` returned
                // `RecvError` on the very first announcement — recorded verbatim
                // in `journaux-accent-a1/04-rouge-pont-media.log`.
                Ok(DepuisCapteur::Accent { couleur }) => {
                    images.send(Recu::Accent { couleur }).is_ok()
                }
                Ok(autre) => {
                    tracing::warn!(
                        ?autre,
                        "trame inattendue sur la connexion média, abandonnée"
                    );
                    return;
                }
                Err(error) => {
                    tracing::warn!(%error, "trame illisible du capteur, canal abandonné");
                    return;
                }
            },
        };
        if !envoi {
            return; // the source has gone
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capteur::protocole::{write_image, write_json};
    use crate::h264::AccessUnit;
    use std::sync::mpsc::sync_channel;

    /// The test that would have caught task 7's omission: a real stream
    /// (serialised by the write functions of `protocole.rs`, not
    /// hand-made bytes) carrying an `Etat`, a frame, then a `Sommeil` must
    /// come out as three `Recu`, IN ORDER, without the thread having
    /// given up before the end of the buffer.
    #[test]
    fn read_media_relays_state_frame_and_sleep_in_order() {
        let mut tampon = Vec::new();
        write_json(
            &mut tampon,
            &DepuisCapteur::Etat {
                vivante: true,
                epuisee: false,
                largeur: 1280,
                hauteur: 720,
            },
        )
        .unwrap();
        write_image(
            &mut tampon,
            &AccessUnit {
                data: vec![1, 2, 3],
                is_keyframe: true,
                pts_90k: 42,
            },
        )
        .unwrap();
        write_json(
            &mut tampon,
            &DepuisCapteur::Sommeil {
                endormie: true,
                raison: "masquee".into(),
            },
        )
        .unwrap();

        let (tx, rx) = sync_channel(8);
        // Called directly (not in a thread): the in-memory buffer is
        // exhausted after the three frames, `lire_trame` then returns a read
        // error, and the function returns by itself — no risk of
        // blocking to test here.
        lire_le_media(std::io::Cursor::new(tampon), tx);

        assert_eq!(
            rx.recv().unwrap(),
            Recu::Etat {
                vivante: true,
                epuisee: false,
                largeur: 1280,
                hauteur: 720
            }
        );
        match rx.recv().unwrap() {
            Recu::Image(unite) => {
                assert_eq!(unite.pts_90k, 42);
                assert!(unite.is_keyframe);
            }
            autre => panic!("attendu une image, reçu {autre:?}"),
        }
        assert_eq!(
            rx.recv().unwrap(),
            Recu::Sommeil {
                endormie: true,
                raison: "masquee".to_string()
            }
        );
        assert!(
            rx.try_recv().is_err(),
            "aucune trame de plus après la fin du tampon : le fil n'a rien perdu ni rien inventé"
        );
    }

    /// The test that would have caught the critical defect found in review of
    /// sub-block D6: `DepuisCapteur::Part` is the very first frame
    /// a session receives under product conditions (`sommeil::inscrire`
    /// sends it at attach time, before any frame). Before this fix,
    /// it fell into the `Ok(autre)` arm and abandoned the thread — every
    /// session would have died at the first frame received, without any test of
    /// tasks 4 or 5 being able to see it since neither pushes a
    /// frame as far as this thread.
    #[test]
    fn lire_le_media_survit_a_une_part_et_la_transmet() {
        let mut tampon = Vec::new();
        write_json(&mut tampon, &DepuisCapteur::Part { bps: 4_000_000 }).unwrap();
        // A frame AFTER the share: if the thread had given up on the share,
        // this frame would never be relayed either.
        write_image(
            &mut tampon,
            &AccessUnit {
                data: vec![9, 9, 9],
                is_keyframe: true,
                pts_90k: 7,
            },
        )
        .unwrap();

        let (tx, rx) = sync_channel(8);
        lire_le_media(std::io::Cursor::new(tampon), tx);

        assert_eq!(rx.recv().unwrap(), Recu::Part { bps: 4_000_000 });
        match rx.recv().unwrap() {
            Recu::Image(unite) => assert_eq!(unite.pts_90k, 7),
            autre => panic!("attendu une image après la part, reçu {autre:?}"),
        }
        assert!(
            rx.try_recv().is_err(),
            "aucune trame de plus après la fin du tampon : le fil n'a rien perdu ni rien inventé"
        );
    }

    /// The same defect, on the same hop, for the same reason — this time for
    /// `DepuisCapteur::Audio` (task 6, sub-block D7): wired on the sensor side
    /// (protocol + window thread) but, without this arm, it would fall into
    /// `Ok(autre)` and kill this thread at the very first audio order received, under
    /// product conditions and on every session.
    #[test]
    fn lire_le_media_survit_a_un_audio_et_le_transmet() {
        let mut tampon = Vec::new();
        write_json(&mut tampon, &DepuisCapteur::Audio { actif: true }).unwrap();
        // A frame AFTER the order: if the thread had given up on it, this
        // frame would never be relayed either.
        write_image(
            &mut tampon,
            &AccessUnit {
                data: vec![4, 4, 4],
                is_keyframe: true,
                pts_90k: 11,
            },
        )
        .unwrap();

        let (tx, rx) = sync_channel(8);
        lire_le_media(std::io::Cursor::new(tampon), tx);

        assert_eq!(rx.recv().unwrap(), Recu::Audio { actif: true });
        match rx.recv().unwrap() {
            Recu::Image(unite) => assert_eq!(unite.pts_90k, 11),
            autre => panic!("attendu une image après l'ordre audio, reçu {autre:?}"),
        }
        assert!(
            rx.try_recv().is_err(),
            "aucune trame de plus après la fin du tampon : le fil n'a rien perdu ni rien inventé"
        );
    }

    /// The same defect, on the same hop, for the same reason — this time for
    /// `DepuisCapteur::PleinEcran` (task 6, sub-block D8): wired on the sensor side
    /// (protocol + window thread) but, without this arm, it would fall into
    /// `Ok(autre)` and kill this thread at the very first fullscreen change
    /// received, under product conditions and on every session.
    #[test]
    fn lire_le_media_survit_a_un_plein_ecran_et_le_transmet() {
        let mut tampon = Vec::new();
        write_json(&mut tampon, &DepuisCapteur::PleinEcran { actif: true }).unwrap();
        // A frame AFTER the order: if the thread had given up on it, this
        // frame would never be relayed either.
        write_image(
            &mut tampon,
            &AccessUnit {
                data: vec![5, 5, 5],
                is_keyframe: true,
                pts_90k: 13,
            },
        )
        .unwrap();

        let (tx, rx) = sync_channel(8);
        lire_le_media(std::io::Cursor::new(tampon), tx);

        assert_eq!(rx.recv().unwrap(), Recu::PleinEcran { actif: true });
        match rx.recv().unwrap() {
            Recu::Image(unite) => assert_eq!(unite.pts_90k, 13),
            autre => panic!("attendu une image après l'ordre plein écran, reçu {autre:?}"),
        }
        assert!(
            rx.try_recv().is_err(),
            "aucune trame de plus après la fin du tampon : le fil n'a rien perdu ni rien inventé"
        );
    }

    /// `DepuisCapteur::PressePapier` (task 9, sub-block P1): **the FIFTH
    /// time** this passage point has to be connected, after `Sommeil` (D5),
    /// `Part` (D6), `Audio` (D7) and `PleinEcran` (D8). Without the arm, this test
    /// fails — and it is the RED of the specification's criterion ①, played on
    /// the host (E13) rather than on the VM, because the defect shows there at the
    /// same hop with more precision and without remote compilation.
    #[test]
    fn lire_le_media_survit_a_un_presse_papier_et_le_transmet() {
        let mut tampon = Vec::new();
        write_json(
            &mut tampon,
            &DepuisCapteur::PressePapier {
                texte: Some("bonjour".into()),
                octets: 7,
            },
        )
        .unwrap();
        // A size REFUSAL travels through the same variant, `texte` as `None`:
        // it must get through too, otherwise the browser's banner would
        // never know that a copy was refused.
        write_json(
            &mut tampon,
            &DepuisCapteur::PressePapier {
                texte: None,
                octets: 100_000,
            },
        )
        .unwrap();
        // A frame AFTER the two announcements: if the thread had given up
        // on them, this frame would never be relayed either.
        write_image(
            &mut tampon,
            &AccessUnit {
                data: vec![7, 7, 7],
                is_keyframe: true,
                pts_90k: 21,
            },
        )
        .unwrap();

        let (tx, rx) = sync_channel(8);
        lire_le_media(std::io::Cursor::new(tampon), tx);

        assert_eq!(
            rx.recv().unwrap(),
            Recu::PressePapier {
                texte: Some("bonjour".into()),
                octets: 7
            }
        );
        assert_eq!(
            rx.recv().unwrap(),
            Recu::PressePapier {
                texte: None,
                octets: 100_000
            }
        );
        match rx.recv().unwrap() {
            Recu::Image(unite) => assert_eq!(unite.pts_90k, 21),
            autre => panic!("attendu une image après le presse-papier, reçu {autre:?}"),
        }
        assert!(
            rx.try_recv().is_err(),
            "aucune trame de plus après la fin du tampon : le fil n'a rien perdu ni rien inventé"
        );
    }

    /// A frame of unknown type on the media connection must kill the thread —
    /// not make it drift silently. Checks that the severity of
    /// `Ok(autre)` was not weakened by the addition of the `Sommeil` arm.
    #[test]
    fn une_trame_de_commande_egaree_sur_le_media_abandonne_le_fil() {
        let mut tampon = Vec::new();
        // `Attachee` is NEVER supposed to go through the media connection:
        // it is a command reply. Receiving it here must abort.
        write_json(
            &mut tampon,
            &DepuisCapteur::Attachee {
                largeur: 1280,
                hauteur: 720,
            },
        )
        .unwrap();
        write_json(
            &mut tampon,
            &DepuisCapteur::Etat {
                vivante: true,
                epuisee: false,
                largeur: 1280,
                hauteur: 720,
            },
        )
        .unwrap();

        let (tx, rx) = sync_channel(8);
        lire_le_media(std::io::Cursor::new(tampon), tx);

        assert!(
            rx.try_recv().is_err(),
            "le fil doit abandonner à la première trame inattendue, sans lire la suite"
        );
    }
    /// `DepuisCapteur::Accent` (task 8, sub-block A1): **the SIXTH time**
    /// this passage point has to be connected, after `Sommeil` (D5), `Part` (D6),
    /// `Audio` (D7), `PleinEcran` (D8) and `PressePapier` (P1).
    ///
    /// 🔴 **THIS TEST WAS WRITTEN AND SEEN RED BEFORE THE ARM EXISTED.** Without
    /// it, the announcement falls into the `Ok(autre)` catch-all, the thread `return`s, and
    /// the first `rx.recv()` returns `RecvError` — no compile error,
    /// no visible failure, and the session falls into its resumption window.
    #[test]
    fn lire_le_media_survit_a_un_accent_et_le_transmet() {
        let mut tampon = Vec::new();
        write_json(
            &mut tampon,
            &DepuisCapteur::Accent {
                couleur: "#7aa2f7".into(),
            },
        )
        .unwrap();
        // A SECOND accent: the sensor only announces on change, but nothing
        // in this thread knows it — it must relay both.
        write_json(
            &mut tampon,
            &DepuisCapteur::Accent {
                couleur: "#fa8c16".into(),
            },
        )
        .unwrap();
        // A frame AFTER the two announcements: if the thread had given up
        // on them, this frame would never be relayed either. It is this
        // third assertion that distinguishes "the arm is missing" from "the message
        // was not written".
        write_image(
            &mut tampon,
            &AccessUnit {
                data: vec![9, 9, 9],
                is_keyframe: true,
                pts_90k: 42,
            },
        )
        .unwrap();

        let (tx, rx) = sync_channel(8);
        lire_le_media(std::io::Cursor::new(tampon), tx);

        assert_eq!(
            rx.recv().unwrap(),
            Recu::Accent {
                couleur: "#7aa2f7".into()
            }
        );
        assert_eq!(
            rx.recv().unwrap(),
            Recu::Accent {
                couleur: "#fa8c16".into()
            }
        );
        match rx.recv().unwrap() {
            Recu::Image(unite) => assert_eq!(unite.pts_90k, 42),
            autre => panic!("attendu une image après l'accent, reçu {autre:?}"),
        }
        assert!(
            rx.try_recv().is_err(),
            "aucune trame de plus après la fin du tampon : le fil n'a rien perdu ni rien inventé"
        );
    }
}
