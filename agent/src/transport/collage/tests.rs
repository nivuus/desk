use std::sync::{Arc, Mutex};
use std::time::Instant;

use proto::input::InputMessage;

use crate::h264::AccessUnit;
use crate::source::VideoSource;
use crate::transport::Session;

/// ORDERED trace shared between the fake source and the input callback:
/// it is what makes D6's order observable, and not only its two
/// effects taken separately.
type Trace = Arc<Mutex<Vec<String>>>;

/// Fake source that records each clipboard write and returns the
/// prepared answer.
///
/// ⚠️ **It OVERRIDES `write_clipboard`, and that is intended here**: this
/// file exercises the use `collage` makes of the method, not its default.
/// The trait's default — which must return `Err` and not `Ok(())`, precisely
/// so that D10's trap does not replay — is exercised separately, in
/// `capteur/distante/tests_etats.rs`.
struct WritingSource {
    inner: crate::source::FileSource,
    trace: Trace,
    accepte: bool,
}

impl VideoSource for WritingSource {
    fn next_frame(&mut self) -> Option<AccessUnit> {
        self.inner.next_frame()
    }
    fn dimensions(&self) -> (u32, u32) {
        self.inner.dimensions()
    }
    fn write_clipboard(&mut self, texte: &str) -> anyhow::Result<()> {
        self.trace.lock().unwrap().push(format!("ecrire:{texte}"));
        if self.accepte {
            Ok(())
        } else {
            anyhow::bail!("the sensor refused: OpenClipboard")
        }
    }
}

fn writing_session(accepte: bool) -> (Session, Trace) {
    let trace: Trace = Arc::new(Mutex::new(Vec::new()));
    let source_path =
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/testsrc.264"));
    let inner = crate::source::FileSource::from_path(source_path, 1280, 720, 60)
        .expect("loading the test stream");
    let source = Box::new(WritingSource {
        inner,
        trace: trace.clone(),
        accepte,
    });
    let session = Session::new(
        source,
        crate::transport::fixtures::local_ip(),
        Instant::now(),
        12_000_000,
    )
    .expect("session");
    (session, trace)
}

/// Empties the flag through the product's path, recording the keys in
/// the SAME trace as the writes.
fn injecter(session: &mut Session, trace: &Trace) {
    let t = trace.clone();
    let mut on_input = move |message: InputMessage| {
        t.lock().unwrap().push(format!("{message:?}"));
    };
    session.injecter_le_collage(&mut on_input);
}

/// 🔴 **D6's ORDER, OBSERVED ON A SINGLE TRACE.** The write precedes the
/// four keys — and the clipboard receives the **DENORMALISED** text,
/// since it is Windows that will read it.
///
/// RED if injection preceded the write, or if the text went out without
/// denormalisation.
#[test]
fn the_write_precedes_the_injection_and_the_text_goes_out_denormalised() {
    let (mut session, trace) = writing_session(true);
    session.traiter_le_collage("une\ndeux");
    injecter(&mut session, &trace);

    let vue = trace.lock().unwrap().clone();
    assert_eq!(vue.len(), 5, "one write then four keys: {vue:?}");
    assert_eq!(
        vue[0], "ecrire:une\r\ndeux",
        "the text must go out as \\r\\n"
    );
    assert!(
        vue[1].starts_with("Key"),
        "the keys follow the write: {vue:?}"
    );
}

/// 🔴 **EXACTLY FOUR KEYS, IN THE ORDER Ctrl↓ V↓ V↑ Ctrl↑.**
///
/// RED if the `Ctrl`↑ is omitted: the application would stay with a
/// modifier PRESSED, and **every following keystroke would become a
/// shortcut**. It is the most insidious defect of this path.
#[test]
fn a_paste_injects_exactly_the_four_keys_in_order() {
    let (mut session, trace) = writing_session(true);
    session.traiter_le_collage("x");
    trace.lock().unwrap().clear();
    injecter(&mut session, &trace);

    let vue = trace.lock().unwrap().clone();
    assert_eq!(
        vue,
        vec![
            format!(
                "{:?}",
                InputMessage::Key {
                    scancode: 0x1d,
                    pressed: true,
                    extended: false
                }
            ),
            format!(
                "{:?}",
                InputMessage::Key {
                    scancode: 0x2f,
                    pressed: true,
                    extended: false
                }
            ),
            format!(
                "{:?}",
                InputMessage::Key {
                    scancode: 0x2f,
                    pressed: false,
                    extended: false
                }
            ),
            format!(
                "{:?}",
                InputMessage::Key {
                    scancode: 0x1d,
                    pressed: false,
                    extended: false
                }
            ),
        ]
    );
}

/// 🔴 **A FAILING WRITE DOES NOT ARM THE INJECTION, AND IT IS THE TEST OF
/// THE ORDER.** The flag is only set in the `Ok` arm: a mutation that
/// armed it unconditionally — or BEFORE the `match` — turns this
/// test red and this one alone.
///
/// Without it, `Ctrl+V` would go out on an unchanged clipboard and paste the
/// **PREVIOUS** content: the silent failure mode D6 exists
/// entirely to avoid, and the only one that gives the user a WRONG result
/// rather than a missing one.
#[test]
fn a_refused_write_injects_no_key() {
    let (mut session, trace) = writing_session(false);
    session.traiter_le_collage("x");
    assert_eq!(
        trace.lock().unwrap().len(),
        1,
        "the write was indeed attempted"
    );
    trace.lock().unwrap().clear();

    injecter(&mut session, &trace);
    assert!(
        trace.lock().unwrap().is_empty(),
        "the V key is LOST, not deferred"
    );
}

/// The flag is **consumed**: a second loop round injects nothing.
///
/// RED if the boolean were never reset to false — `Ctrl+V` would go out at
/// every round, that is, at the video cadence.
#[test]
fn the_flag_is_consumed() {
    let (mut session, trace) = writing_session(true);
    session.traiter_le_collage("x");
    injecter(&mut session, &trace);
    trace.lock().unwrap().clear();

    injecter(&mut session, &trace);
    assert!(
        trace.lock().unwrap().is_empty(),
        "a second round injects nothing"
    );
}

/// Without a paste, no loop round injects anything at all.
///
/// RED if `injecter_le_collage` typed without looking at the flag.
#[test]
fn without_paste_no_key_is_injected() {
    let (mut session, trace) = writing_session(true);
    injecter(&mut session, &trace);
    assert!(trace.lock().unwrap().is_empty());
}

/// 🔴 **ABOVE THE BOUND: NOTHING IS WRITTEN, AND NOTHING IS INJECTED.**
///
/// RED if the bound were not applied on this path: 64 KiB + 1 would go out
/// to the sensor↔child pipe, and the VM would paste a text the client had
/// nevertheless refused to send.
#[test]
fn a_text_above_the_bound_is_neither_written_nor_injected() {
    let (mut session, trace) = writing_session(true);
    let trop = "a".repeat(crate::presse_papier::PRESSE_PAPIER_MAX + 1);
    session.traiter_le_collage(&trop);

    assert!(
        trace.lock().unwrap().is_empty(),
        "no write must be attempted"
    );
    injecter(&mut session, &trace);
    assert!(trace.lock().unwrap().is_empty(), "no key either");
}

/// 🔴 **THE BOUND APPLIES TO THE NORMALISED TEXT, NEVER TO THE DENORMALISED ONE**, and
/// this test measures it where the order of the three operations might seem
/// indifferent.
///
/// A text of `PRESSE_PAPIER_MAX` bytes made entirely of line breaks
/// DOUBLES in size at denormalisation. Bounding after it would refuse it —
/// whereas it is exactly what the VM could have sent in the other
/// direction, where the bound also applies to the normalised form (D-P1-2). A
/// round trip would become impossible.
///
/// ROUGE si l'ordre est `denormaliser` puis `borner_entrant`.
#[test]
fn the_bound_applies_to_the_normalised_form_so_the_round_trip_holds() {
    let (mut session, trace) = writing_session(true);
    let sauts = "\n".repeat(crate::presse_papier::PRESSE_PAPIER_MAX);
    session.traiter_le_collage(&sauts);

    let vue = trace.lock().unwrap().clone();
    assert_eq!(
        vue.len(),
        1,
        "the text should have been written: {}",
        vue.len()
    );
    assert_eq!(
        vue[0].len() - "ecrire:".len(),
        crate::presse_papier::PRESSE_PAPIER_MAX * 2,
        "denormalisation does double the size, and is NOT bounded"
    );
}
