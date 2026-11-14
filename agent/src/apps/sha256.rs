//! SHA-256, through the maintained RustCrypto `sha2` crate.
//!
//! The agent's single entry point for SHA-256: `Condensateur` absorbs a
//! message in pieces (an installer of several hundred megabytes is
//! fingerprinted WHILE it is written to disk, never held in memory), `hex`
//! fingerprints a message held in memory, and `hex_de` renders a digest. Its
//! callers identify catalogue entries and icons, and — since sub-block G3 —
//! check the integrity of a downloaded installer before it is run.
//!
//! This module used to carry a hand-written FIPS 180-4 implementation, whose
//! own header said it must give way to an audited one the day something
//! depended on it for integrity. Installer verification is that day: the
//! algorithm now comes from `sha2`, and only the agent's small API stays here.
//!
//! The FIPS 180-4 known-answer vectors stay too, through both paths: they
//! pin the API (lowercase hexadecimal, piecewise absorption) whatever
//! implements it.

use sha2::{Digest, Sha256};

/// A SHA-256 computation fed piece by piece. The SPLITTING changes nothing
/// in the result, whatever the size of the pieces.
pub struct Condensateur(Sha256);

impl Condensateur {
    /// A blank digester.
    pub fn neuf() -> Self {
        Self(Sha256::new())
    }

    /// Absorbs a piece of the message.
    pub fn absorber(&mut self, bloc: &[u8]) {
        self.0.update(bloc);
    }

    /// Closes the message and returns its fingerprint, in 32 bytes.
    pub fn terminer(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

/// The 32 bytes of a digest, as 64 lowercase hexadecimal characters.
///
/// ⚠️ IT EXISTS BECAUSE `hex` TAKES A MESSAGE, NOT A DIGEST: a
/// caller that absorbed its file in pieces no longer has the message. Without
/// it, `installation::telechargement` had its own copy — and two
/// hexadecimal formattings would diverge the day one of them changed
/// case, which would make a fingerprint comparison fail without anything
/// saying why.
pub fn hex_de(condensat: [u8; 32]) -> String {
    let mut sortie = String::with_capacity(64);
    for octet in condensat {
        use std::fmt::Write;
        let _ = write!(sortie, "{octet:02x}");
    }
    sortie
}

/// The fingerprint of a message held in memory, as 64 lowercase
/// hexadecimal characters.
pub fn hex(message: &[u8]) -> String {
    hex_de(Sha256::digest(message).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The FIPS 180-4 known-answer vectors, copied from the standard.
    ///
    /// 🔴 THEY ARE WHAT MAKES THIS MODULE SOMETHING OTHER THAN A PROMISE. A
    /// wrong implementation — a badly copied constant, a shift in the
    /// wrong direction, padding that forgets its second block, a residue copied at the
    /// wrong offset — misses them.
    ///
    /// They are laid out as a TABLE rather than as assertions, because BOTH
    /// paths must go through them: the convenience one and the incremental one.
    /// Duplicating them would let one path live tested on weaker vectors
    /// than the other.
    const VECTEURS: [(&[u8], &str); 3] = [
        // §D.1 : le message vide.
        (
            b"",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
        // §B.1 : "abc", un seul bloc.
        (
            b"abc",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
        // §B.2: 448 bits — 56 bytes, so the length does NOT fit in the
        // padding block and a second one is needed. It is the only vector that
        // exercises this branch.
        (
            b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
        ),
    ];

    #[test]
    fn les_vecteurs_de_reponse_connue_de_fips_180_4() {
        for (message, attendu) in VECTEURS {
            assert_eq!(hex(message), attendu, "message de {} octets", message.len());
        }
    }

    /// The SAME vectors, absorbed in pieces, in IRREGULAR sizes.
    ///
    /// 🔴 ABSORBING IN BLOCKS OF 64 WOULD PROVE ALMOST NOTHING: the residue would
    /// never be partial and a copy mistake at the offset would not show.
    /// It is 1, 63 and 65 that keep the residue alive across
    /// several calls and make it cross a block boundary in the middle
    /// of a piece; 1000, longer than the longest vector,
    /// checks that a piece overflowing the message changes nothing.
    #[test]
    fn the_hasher_absorbs_irregular_sizes_without_changing_the_digest() {
        for (message, attendu) in VECTEURS {
            for size in [1usize, 63, 64, 65, 1000] {
                let mut condensateur = Condensateur::neuf();
                for morceau in message.chunks(size) {
                    condensateur.absorber(morceau);
                }
                assert_eq!(
                    hex_de(condensateur.terminer()),
                    attendu,
                    "vector of {} bytes, chunks of {size}",
                    message.len()
                );
            }
        }
    }

    #[test]
    fn a_message_length_right_on_the_padding_boundary() {
        // 55 bytes: the last one where the length still fits in the block.
        // 56: the first that requires a second. 64: a full block, whose
        // padding takes a whole extra block.
        for size in [55usize, 56, 63, 64, 65, 119, 120] {
            let message = vec![b'a'; size];
            // We do not check the value — it is not in the standard —
            // but that nothing panics and that the fingerprint changes with the
            // size, which broken padding would not guarantee.
            assert_eq!(hex(&message).len(), 64, "size {size}");

            // And the INCREMENTAL path, one byte at a time, must return the
            // same thing: these lengths are the ones that move the residue
            // on either side of the padding boundary.
            let mut condensateur = Condensateur::neuf();
            for octet in &message {
                condensateur.absorber(std::slice::from_ref(octet));
            }
            assert_eq!(
                hex_de(condensateur.terminer()),
                hex(&message),
                "size {size}, one byte at a time"
            );
        }
        let all: std::collections::HashSet<String> =
            (0..130).map(|n| hex(&vec![b'a'; n])).collect();
        assert_eq!(all.len(), 130, "130 lengths, 130 distinct hashes");
    }
}
