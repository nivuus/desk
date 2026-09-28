use super::*;
use proto::files::MAX_FRAME_SIZE;

#[test]
fn a_range_of_exactly_max_frame_size_makes_a_single_piece() {
    // At the EXACT threshold: one chunk, not two with one empty. It is the
    // classic error of a loop that tests `>=` where `>` is needed.
    let m = decouper(0, MAX_FRAME_SIZE as u64, MAX_FRAME_SIZE);
    assert_eq!(m.len(), 1);
    assert_eq!(
        m[0],
        Morceau {
            position: 0,
            length: MAX_FRAME_SIZE as u32
        }
    );

    // …and one more byte makes exactly two, the second being one byte.
    let m = decouper(0, MAX_FRAME_SIZE as u64 + 1, MAX_FRAME_SIZE);
    assert_eq!(m.len(), 2);
    assert_eq!(
        m[1],
        Morceau {
            position: MAX_FRAME_SIZE as u64,
            length: 1
        }
    );
}

#[test]
fn a_zero_size_range_makes_no_piece() {
    // A chunk of length 0 would cause an empty response frame, which
    // nothing would distinguish from an end of file.
    assert!(decouper(0, 0, MAX_FRAME_SIZE).is_empty());
    assert!(decouper(4096, 0, MAX_FRAME_SIZE).is_empty());
}

#[test]
fn the_sum_of_lengths_equals_the_requested_length() {
    // Over a hundred sizes from 1 to 3·max: it is the property that makes the
    // SHA-256 digest of criterion (2) fail if it is wrong by a single byte.
    let max = 1000usize;
    for n in 1..=100u64 {
        let length = n * 3 * max as u64 / 100 + 1;
        let somme: u64 = decouper(7, length, max)
            .iter()
            .map(|m| u64::from(m.length))
            .sum();
        assert_eq!(somme, length, "requested length {length}");
    }
}

#[test]
fn chunks_are_contiguous_and_increasing() {
    // Neither hole (the file would be truncated in the middle), nor overlap (some
    // bytes would be written twice).
    let morceaux = decouper(1_000, 10_000, 3_000);
    assert_eq!(morceaux.len(), 4);
    let mut attendu = 1_000u64;
    for m in &morceaux {
        assert_eq!(m.position, attendu, "gap or overlap before {m:?}");
        attendu += u64::from(m.length);
    }
    assert_eq!(attendu, 11_000);
    // And no empty chunk in the middle.
    assert!(morceaux.iter().all(|m| m.length > 0));
}

#[test]
fn the_first_chunk_starts_at_the_requested_position() {
    // The position is ignored by any implementation that would restart from 0:
    // ProjFS commonly requests a range IN THE MIDDLE of a file.
    assert_eq!(decouper(1_234_567, 10, 4)[0].position, 1_234_567);
    assert_eq!(decouper(0, 10, 4)[0].position, 0);
}

#[test]
fn a_length_above_u32_max_is_split_anyway() {
    // `PRJ_GET_FILE_DATA_CB` receives `byteoffset: u64`: the file can
    // exceed 4 GiB. An implementation that converted the length to `u32`
    // before splitting would lose everything beyond the first round.
    let length = u64::from(u32::MAX) + 5;
    let morceaux = decouper(u64::from(u32::MAX), length, MAX_FRAME_SIZE);
    let somme: u64 = morceaux.iter().map(|m| u64::from(m.length)).sum();
    assert_eq!(somme, length);
    assert_eq!(morceaux[0].position, u64::from(u32::MAX));
    // The last chunk does end well beyond 4 GiB + the starting position.
    let last = morceaux.last().unwrap();
    assert_eq!(
        last.position + u64::from(last.length),
        u64::from(u32::MAX) + length
    );
}

#[test]
fn a_max_larger_than_u32_max_does_not_overflow_on_conversion() {
    // On a 64-bit target, `usize` is wider than `u32`: a `max` received
    // beyond `u32::MAX` would make the length conversion overflow. The
    // module bounds it BEFORE converting, and this test is the only thing that
    // establishes it — no F1 caller passes such a value.
    let morceaux = decouper(0, u64::from(u32::MAX) + 2, usize::MAX);
    let somme: u64 = morceaux.iter().map(|m| u64::from(m.length)).sum();
    assert_eq!(somme, u64::from(u32::MAX) + 2);
    assert!(morceaux.iter().all(|m| m.length > 0));
}
