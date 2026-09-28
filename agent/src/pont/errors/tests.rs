use super::*;

#[test]
fn two_distinct_causes_never_share_a_code() {
    // ⚠️ The test spec §4.4 names, and the counter-example is the old
    // bridge: `cb(-1)` (EPERM) at NINE distinct sites of `src/file.js`.
    //
    // It is written by SWEEPING `ALL`, never by listing by hand:
    // otherwise a variant added tomorrow would escape the check, and the table
    // would become decorative again with nothing saying so.
    for (i, a) in Error::ALL.iter().enumerate() {
        for b in &Error::ALL[i + 1..] {
            assert_ne!(
                hresult(*a),
                hresult(*b),
                "{a:?} and {b:?} share code {:#010x}",
                hresult(*a)
            );
        }
    }
}

#[test]
fn the_sweep_really_covers_every_variant() {
    // The guard's guard: without it, `ALL` could forget a variant and
    // the sweep above would pass while exercising nothing — the exact pattern
    // this repository caught four times in D10.
    assert_eq!(Error::ALL.len(), COUNT);
    let mut vus = [false; COUNT];
    for e in Error::ALL {
        assert!(!vus[index(e)], "{e:?} appears twice in ALL");
        vus[index(e)] = true;
    }
    assert!(vus.iter().all(|v| *v), "a variant is missing from ALL");
}

#[test]
fn no_returned_code_is_a_success() {
    // The severity bit (0x8000_0000) must be set on all twelve: a
    // success `HRESULT` returned to ProjFS would make it believe the operation
    // succeeded, and the Windows application would read an empty file instead of an
    // error — the worst possible failure mode for this module.
    for e in Error::ALL {
        let h = hresult(e);
        assert!(h < 0, "{e:?} returns {h:#010x}, which is a success");
        assert_eq!(
            (h as u32) & 0xFFFF_0000,
            FACILITE_WIN32,
            "{e:?} is not in FACILITY_WIN32"
        );
    }
}

#[test]
fn the_closed_channel_does_return_error_io_device() {
    // "The standard I/O error" of framing §7: it is what Explorer
    // displays as "the device is not accessible", and not as
    // "file not found", when the browser tab closes.
    assert_eq!(hresult(Error::CanalFerme), 0x8007_045Du32 as i32);
}

#[test]
fn write_protection_returns_0x80070013() {
    // ❌ *This comment said "every write, every CREATION, every
    // deletion ends up there". F1's acceptance run refuted creation: a
    // file created from scratch in the root SUCCEEDS (2 runs
    // recorded out of 2 that reach this phase). `NEW_FILE_CREATED` is a
    // POST notification, hence unrefusable — see `pont::notifications`.*
    assert_eq!(hresult(Error::ProtegeEnEcriture), 0x8007_0013u32 as i32);
}

#[test]
fn the_twelve_codes_are_pinned_one_by_one() {
    // Pins the whole table: the distinction test above would stay
    // green if two variants SWAPPED their codes, which would return
    // "disk full" for an absent file.
    let attendu: [(Error, u32); COUNT] = [
        (Error::Introuvable, 0x8007_0002),
        (Error::CheminIntrouvable, 0x8007_0003),
        (Error::AccesRefuse, 0x8007_0005),
        (Error::CanalFerme, 0x8007_045D),
        (Error::DelaiDepasse, 0x8007_0079),
        (Error::Abandonnee, 0x8007_03E3),
        (Error::DisquePlein, 0x8007_0070),
        (Error::NonSupporte, 0x8007_0032),
        (Error::RepertoireNonVide, 0x8007_0091),
        (Error::DejaPresent, 0x8007_0050),
        (Error::ProtegeEnEcriture, 0x8007_0013),
        (Error::Inattendue, 0x8007_001F),
    ];
    for (e, code) in attendu {
        assert_eq!(hresult(e), code as i32, "{e:?}");
    }
}

/// `HRESULT_FROM_WIN32(ERROR_IO_PENDING)` is `0x800703E5`, and it is the value
/// EVERY asynchronous callback returns. A transcription error would return
/// a failure code where ProjFS expects "in progress": the application would receive
/// a failed I/O on every read, and the bridge would wait indefinitely for a
/// completion ProjFS would no longer accept.
#[test]
fn in_progress_equals_the_hresult_of_error_io_pending() {
    assert_eq!(EN_COURS, 0x8007_03E5u32 as i32);
}

/// `EN_COURS` is the `hresult` of NO `Error` variant. If it were,
/// a real failure would be indistinguishable from an operation in progress, and ProjFS
/// would wait for a completion that would never come.
#[test]
fn in_progress_collides_with_no_failure_cause() {
    for cause in Error::ALL {
        assert_ne!(hresult(cause), EN_COURS, "{cause:?}");
    }
}
