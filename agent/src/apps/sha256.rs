//! SHA-256 (FIPS 180-4 §6.2), written here rather than borrowed.
//!
//! 🔴 WHY THIS MODULE EXISTS WHEN `sha2` IS ONE `cargo add` AWAY: sub-block
//! G1 declares it adds NO production dependency, neither in
//! TypeScript nor in Rust — its only addition to `agent/Cargo.toml` is a
//! `feature` of a crate already present. Checked before writing a line:
//! `sha2` appears NOWHERE in `Cargo.lock`, neither directly nor
//! transitively; `sha1`, `md-5` and `hmac` are there, but none yields SHA-256, and
//! the spec names SHA-256. The choice is therefore between breaking the sub-block's
//! invariant and writing eighty lines of an entirely
//! specified algorithm. It is the second, and it is declared.
//!
//! ⚠️ IT IS NOT A SECURITY PRIMITIVE HERE: the fingerprint serves
//! as a stable identity for a triple, under a unique index `(vm_id, cle)` that
//! bounds any collision to a single VM. The day something
//! authenticating depended on it, this module must give way to an
//! audited implementation — and this sentence is here so that people know it.
//!
//! The proof rests on the FIPS 180-4 known-answer vectors: the empty
//! string, `abc`, and the 448-bit message that forces a second block. A
//! wrong digest algorithm misses all three.
//!
//! ⚙️ TWO PATHS, A SINGLE IMPLEMENTATION (sub-block G3): `Condensateur`
//! absorbs the message in pieces — an installer of several hundred
//! megabytes is fingerprinted WHILE it is being written to disk, without ever
//! holding it in memory —, and `condenser` is now only the convenience call that
//! absorbs everything at once. The padding is therefore written only ONCE, in
//! `terminer`, and the known-answer vectors go through both paths:
//! two implementations that diverged silently are exactly what
//! this economy forbids.

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

const ETAT_INITIAL: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

fn comprimer(etat: &mut [u32; 8], bloc: &[u8; 64]) {
    let mut w = [0u32; 64];
    for (i, mot) in w.iter_mut().enumerate().take(16) {
        let d = i * 4;
        *mot = u32::from_be_bytes([bloc[d], bloc[d + 1], bloc[d + 2], bloc[d + 3]]);
    }
    for i in 16..64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16]
            .wrapping_add(s0)
            .wrapping_add(w[i - 7])
            .wrapping_add(s1);
    }
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *etat;
    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ ((!e) & g);
        let t1 = h
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(K[i])
            .wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = s0.wrapping_add(maj);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }
    for (mot, ajout) in etat.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *mot = mot.wrapping_add(ajout);
    }
}

/// The state of a digest IN PROGRESS, to fingerprint a message we do not
/// hold — and cannot hold — in memory.
///
/// 🔴 IT IS THIS PATH THAT CARRIES THE ALGORITHM; `condenser` is only a
/// convenience call to it. Two invariants govern it, and missing them would yield
/// a wrong fingerprint no compiler would catch:
/// - the residue ALWAYS holds fewer than 64 bytes — as soon as it reaches 64 it
///   is compressed and emptied —, and incoming bytes are copied AFTER
///   those already occupying it, never at its start;
/// - the bit count is that of the WHOLE message, not of the last piece:
///   it is what the padding writes.
pub struct Condensateur {
    etat: [u32; 8],
    /// The received bytes that have not yet filled a block. Only the first
    /// `en_residu` are valid; the rest is dead filler.
    residu: [u8; 64],
    en_residu: usize,
    /// The length of everything absorbed so far, in BITS.
    bits: u64,
}

impl Condensateur {
    /// A blank digester, on the standard's initial state.
    pub fn neuf() -> Self {
        Self {
            etat: ETAT_INITIAL,
            residu: [0u8; 64],
            en_residu: 0,
            bits: 0,
        }
    }

    /// Absorbs a piece of the message. The SPLITTING must change nothing in the
    /// result, whatever the size of the pieces — that is precisely what
    /// the irregular sizes test exercises.
    pub fn absorber(&mut self, mut bloc: &[u8]) {
        self.bits = self.bits.wrapping_add((bloc.len() as u64) * 8);

        // First top up the residue, if there is one. If that does not fill a
        // block, `bloc` is exhausted BY CONSTRUCTION and we must return here:
        // otherwise the code below would overwrite `en_residu` with the empty
        // remainder of `as_chunks` on an empty slice, and the residue already
        // received would be lost without a word.
        if self.en_residu > 0 {
            let manque = (64 - self.en_residu).min(bloc.len());
            self.residu[self.en_residu..self.en_residu + manque].copy_from_slice(&bloc[..manque]);
            self.en_residu += manque;
            bloc = &bloc[manque..];
            if self.en_residu < 64 {
                return;
            }
            comprimer(&mut self.etat, &self.residu);
            self.en_residu = 0;
        }

        let (entiers, reste) = bloc.as_chunks::<64>();
        for entier in entiers {
            comprimer(&mut self.etat, entier);
        }
        self.residu[..reste.len()].copy_from_slice(reste);
        self.en_residu = reste.len();
    }

    /// Closes the message and returns its fingerprint, in 32 bytes.
    pub fn terminer(self) -> [u8; 32] {
        let mut etat = self.etat;

        // FIPS 180-4 §5.1.1 padding: the byte 0x80, zeros, then the
        // length in BITS on 64 big-endian bits. If the remainder exceeds
        // 55 bytes, the length no longer fits in this block and a
        // second is needed — that is the case the third known-answer vector
        // exercises. ⚠️ The length written is that of the WHOLE message, which
        // `self.bits` accumulates since the first `absorber`, and never that
        // of the residue being padded here.
        let reste = &self.residu[..self.en_residu];
        let mut queue = [0u8; 128];
        queue[..reste.len()].copy_from_slice(reste);
        queue[reste.len()] = 0x80;
        let size = if reste.len() < 56 { 64 } else { 128 };
        queue[size - 8..size].copy_from_slice(&self.bits.to_be_bytes());
        for debut in (0..size).step_by(64) {
            comprimer(
                &mut etat,
                queue[debut..debut + 64].try_into().expect("64 octets"),
            );
        }

        let mut sortie = [0u8; 32];
        for (i, mot) in etat.iter().enumerate() {
            sortie[i * 4..i * 4 + 4].copy_from_slice(&mot.to_be_bytes());
        }
        sortie
    }
}

/// The SHA-256 fingerprint of a message held in memory, in 32 bytes.
///
/// Pure convenience: a `Condensateur` absorbed in one go. Nothing else
/// lives here, so that the two paths CANNOT diverge.
pub fn condenser(message: &[u8]) -> [u8; 32] {
    let mut condensateur = Condensateur::neuf();
    condensateur.absorber(message);
    condensateur.terminer()
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

/// The fingerprint, as 64 lowercase hexadecimal characters.
pub fn hex(message: &[u8]) -> String {
    hexa(condenser(message))
}

/// Thirty-two bytes rendered as 64 lowercase hexadecimal characters.
///
/// Separated from `hex` so that the INCREMENTAL path, which already returns a fingerprint,
/// is compared to the vectors without copying this formatting a second time.
fn hexa(empreinte: [u8; 32]) -> String {
    let mut sortie = String::with_capacity(64);
    for octet in empreinte {
        use std::fmt::Write;
        let _ = write!(sortie, "{octet:02x}");
    }
    sortie
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
                    hexa(condensateur.terminer()),
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
                hexa(condensateur.terminer()),
                hex(&message),
                "size {size}, one byte at a time"
            );
        }
        let all: std::collections::HashSet<String> =
            (0..130).map(|n| hex(&vec![b'a'; n])).collect();
        assert_eq!(all.len(), 130, "130 lengths, 130 distinct hashes");
    }
}
