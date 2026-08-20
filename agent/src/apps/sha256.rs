//! SHA-256 (FIPS 180-4 §6.2), écrit ici plutôt qu'emprunté.
//!
//! 🔴 POURQUOI CE MODULE EXISTE ALORS QUE `sha2` EST À UN `cargo add` : le
//! sous-bloc G1 déclare n'ajouter AUCUNE dépendance de production, ni en
//! TypeScript ni en Rust — sa seule addition à `agent/Cargo.toml` est une
//! `feature` d'un crate déjà présent. Relevé avant d'écrire une ligne :
//! `sha2` n'apparaît NULLE PART dans `Cargo.lock`, ni en direct ni en
//! transitif ; `sha1`, `md-5` et `hmac` y sont, mais aucun ne rend SHA-256, et
//! la spec nomme SHA-256. Le choix est donc entre rompre l'invariant du
//! sous-bloc et écrire quatre-vingts lignes d'un algorithme entièrement
//! spécifié. C'est le second, et il se déclare.
//!
//! ⚠️ CE N'EST PAS UNE PRIMITIVE DE SÉCURITÉ ICI : l'empreinte sert
//! d'identité stable pour un triplet, sous un index unique `(vm_id, cle)` qui
//! borne toute collision à une seule VM. Le jour où quelque chose
//! d'authentifiant en dépendrait, ce module doit céder la place à une
//! implémentation auditée — et cette phrase est là pour qu'on le sache.
//!
//! La preuve tient aux vecteurs de réponse connue de FIPS 180-4 : la chaîne
//! vide, `abc`, et le message de 448 bits qui force un second bloc. Un
//! algorithme de condensation faux les rate tous les trois.

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

/// L'empreinte SHA-256 d'un message, en 32 octets.
pub fn condenser(message: &[u8]) -> [u8; 32] {
    let mut etat = ETAT_INITIAL;
    let mut blocs = message.chunks_exact(64);
    for bloc in blocs.by_ref() {
        comprimer(&mut etat, bloc.try_into().expect("64 octets"));
    }

    // Bourrage FIPS 180-4 §5.1.1 : l'octet 0x80, des zéros, puis la longueur
    // en BITS sur 64 bits gros-boutiens. Si le reste dépasse 55 octets, la
    // longueur ne tient plus dans ce bloc et il en faut un second — c'est le
    // cas que le troisième vecteur de réponse connue exerce.
    let reste = blocs.remainder();
    let mut queue = [0u8; 128];
    queue[..reste.len()].copy_from_slice(reste);
    queue[reste.len()] = 0x80;
    let taille = if reste.len() < 56 { 64 } else { 128 };
    let bits = (message.len() as u64) * 8;
    queue[taille - 8..taille].copy_from_slice(&bits.to_be_bytes());
    for debut in (0..taille).step_by(64) {
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

/// L'empreinte, en 64 caractères hexadécimaux minuscules.
pub fn hex(message: &[u8]) -> String {
    let mut sortie = String::with_capacity(64);
    for octet in condenser(message) {
        use std::fmt::Write;
        let _ = write!(sortie, "{octet:02x}");
    }
    sortie
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les vecteurs de réponse connue de FIPS 180-4, recopiés de la norme.
    ///
    /// 🔴 CE SONT EUX QUI FONT DE CE MODULE AUTRE CHOSE QU'UNE PROMESSE. Une
    /// implémentation fausse — une constante mal recopiée, un décalage à
    /// l'envers, un bourrage qui oublie son second bloc — les rate.
    #[test]
    fn les_vecteurs_de_reponse_connue_de_fips_180_4() {
        // §D.1 : le message vide.
        assert_eq!(
            hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        // §B.1 : "abc", un seul bloc.
        assert_eq!(
            hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        // §B.2 : 448 bits — 56 octets, donc la longueur ne tient PAS dans le
        // bloc de bourrage et il en faut un second. C'est le seul vecteur qui
        // exerce cette branche.
        assert_eq!(
            hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn une_longueur_de_message_pile_sur_la_frontiere_du_bourrage() {
        // 55 octets : le dernier où la longueur tient encore dans le bloc.
        // 56 : le premier qui en exige un second. 64 : un bloc plein, dont le
        // bourrage occupe tout un bloc de plus.
        for taille in [55usize, 56, 63, 64, 65, 119, 120] {
            let message = vec![b'a'; taille];
            // On ne vérifie pas la valeur — elle n'est pas dans la norme —
            // mais que rien ne panique et que l'empreinte change avec la
            // taille, ce qu'un bourrage cassé ne garantirait pas.
            assert_eq!(hex(&message).len(), 64, "taille {taille}");
        }
        let toutes: std::collections::HashSet<String> = (0..130)
            .map(|n| hex(&vec![b'a'; n]))
            .collect();
        assert_eq!(toutes.len(), 130, "130 longueurs, 130 empreintes distinctes");
    }
}
