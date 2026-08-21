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
//!
//! ⚙️ DEUX VOIES, UNE SEULE IMPLÉMENTATION (sous-bloc G3) : `Condensateur`
//! absorbe le message par morceaux — un installeur de plusieurs centaines de
//! mégaoctets s'empreint PENDANT qu'on l'écrit sur le disque, sans jamais le
//! tenir en mémoire —, et `condenser` n'est plus que l'appel de commodité qui
//! absorbe tout d'un coup. Le bourrage n'est donc écrit qu'UNE fois, dans
//! `terminer`, et les vecteurs de réponse connue traversent les deux voies :
//! deux implémentations qui divergeraient en silence sont exactement ce que
//! cette économie interdit.

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

/// L'état d'une condensation EN COURS, pour empreindre un message qu'on ne
/// tient pas — et qu'on ne peut pas tenir — en mémoire.
///
/// 🔴 C'EST CETTE VOIE QUI PORTE L'ALGORITHME ; `condenser` n'en est qu'un
/// appel de commodité. Deux invariants la gouvernent, et les rater rendrait
/// une empreinte fausse qu'aucun compilateur n'attraperait :
/// - le résidu porte TOUJOURS moins de 64 octets — dès qu'il en atteint 64 il
///   est comprimé et vidé —, et les octets qui arrivent se recopient à la
///   SUITE de ceux qui l'occupent déjà, jamais à son début ;
/// - le compte de bits est celui du message ENTIER, pas du dernier morceau :
///   c'est lui que le bourrage inscrit.
pub struct Condensateur {
    etat: [u32; 8],
    /// Les octets reçus qui n'ont pas encore rempli un bloc. Seuls les
    /// `en_residu` premiers sont valides ; le reste est du remplissage mort.
    residu: [u8; 64],
    en_residu: usize,
    /// La longueur de tout ce qui a été absorbé, en BITS.
    bits: u64,
}

impl Condensateur {
    /// Un condensateur vierge, sur l'état initial de la norme.
    pub fn neuf() -> Self {
        Self {
            etat: ETAT_INITIAL,
            residu: [0u8; 64],
            en_residu: 0,
            bits: 0,
        }
    }

    /// Absorbe un morceau du message. La DÉCOUPE ne doit rien changer au
    /// résultat, quelle que soit la taille des morceaux — c'est précisément ce
    /// que le test des tailles irrégulières exerce.
    pub fn absorber(&mut self, mut bloc: &[u8]) {
        self.bits = self.bits.wrapping_add((bloc.len() as u64) * 8);

        // D'abord compléter le résidu, s'il y en a un. S'il ne suffit pas à
        // remplir un bloc, `bloc` est épuisé PAR CONSTRUCTION et il faut
        // sortir ici : la suite écraserait sinon `en_residu` avec le reste
        // vide d'un `chunks_exact` sur une tranche vide, et le résidu déjà
        // reçu serait perdu sans un mot.
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

        let mut entiers = bloc.chunks_exact(64);
        for entier in entiers.by_ref() {
            comprimer(&mut self.etat, entier.try_into().expect("64 octets"));
        }
        let reste = entiers.remainder();
        self.residu[..reste.len()].copy_from_slice(reste);
        self.en_residu = reste.len();
    }

    /// Clôt le message et rend son empreinte, en 32 octets.
    pub fn terminer(self) -> [u8; 32] {
        let mut etat = self.etat;

        // Bourrage FIPS 180-4 §5.1.1 : l'octet 0x80, des zéros, puis la
        // longueur en BITS sur 64 bits gros-boutiens. Si le reste dépasse
        // 55 octets, la longueur ne tient plus dans ce bloc et il en faut un
        // second — c'est le cas que le troisième vecteur de réponse connue
        // exerce. ⚠️ La longueur inscrite est celle du message ENTIER, que
        // `self.bits` accumule depuis le premier `absorber`, et jamais celle
        // du résidu qu'on borde ici.
        let reste = &self.residu[..self.en_residu];
        let mut queue = [0u8; 128];
        queue[..reste.len()].copy_from_slice(reste);
        queue[reste.len()] = 0x80;
        let taille = if reste.len() < 56 { 64 } else { 128 };
        queue[taille - 8..taille].copy_from_slice(&self.bits.to_be_bytes());
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
}

/// L'empreinte SHA-256 d'un message tenu en mémoire, en 32 octets.
///
/// Pure commodité : un `Condensateur` absorbé d'un seul coup. Rien d'autre ne
/// vit ici, pour que les deux voies ne PUISSENT pas diverger.
pub fn condenser(message: &[u8]) -> [u8; 32] {
    let mut condensateur = Condensateur::neuf();
    condensateur.absorber(message);
    condensateur.terminer()
}

/// L'empreinte, en 64 caractères hexadécimaux minuscules.
pub fn hex(message: &[u8]) -> String {
    hexa(condenser(message))
}

/// Trente-deux octets rendus en 64 caractères hexadécimaux minuscules.
///
/// Séparé de `hex` pour que la voie INCRÉMENTALE, qui rend déjà une empreinte,
/// se compare aux vecteurs sans qu'on recopie ce formatage une seconde fois.
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

    /// Les vecteurs de réponse connue de FIPS 180-4, recopiés de la norme.
    ///
    /// 🔴 CE SONT EUX QUI FONT DE CE MODULE AUTRE CHOSE QU'UNE PROMESSE. Une
    /// implémentation fausse — une constante mal recopiée, un décalage à
    /// l'envers, un bourrage qui oublie son second bloc, un résidu recopié au
    /// mauvais offset — les rate.
    ///
    /// Ils sont posés en TABLE plutôt qu'en assertions, parce que les DEUX
    /// voies doivent les traverser : la commodité et l'incrémental. Les
    /// dupliquer laisserait vivre une voie éprouvée sur des vecteurs plus
    /// faibles que l'autre.
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
        // §B.2 : 448 bits — 56 octets, donc la longueur ne tient PAS dans le
        // bloc de bourrage et il en faut un second. C'est le seul vecteur qui
        // exerce cette branche.
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

    /// Les MÊMES vecteurs, absorbés par morceaux, en tailles IRRÉGULIÈRES.
    ///
    /// 🔴 ABSORBER PAR BLOCS DE 64 NE PROUVERAIT PRESQUE RIEN : le résidu ne
    /// serait jamais partiel et une faute de recopie à l'offset ne se verrait
    /// pas. Ce sont le 1, le 63 et le 65 qui font vivre le résidu à travers
    /// plusieurs appels et le font franchir la frontière d'un bloc au milieu
    /// d'un morceau ; le 1000, plus long que le plus long des vecteurs,
    /// vérifie qu'un morceau qui déborde le message ne change rien.
    #[test]
    fn le_condensateur_absorbe_en_tailles_irregulieres_sans_changer_l_empreinte() {
        for (message, attendu) in VECTEURS {
            for taille in [1usize, 63, 64, 65, 1000] {
                let mut condensateur = Condensateur::neuf();
                for morceau in message.chunks(taille) {
                    condensateur.absorber(morceau);
                }
                assert_eq!(
                    hexa(condensateur.terminer()),
                    attendu,
                    "vecteur de {} octets, morceaux de {taille}",
                    message.len()
                );
            }
        }
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

            // Et la voie INCRÉMENTALE, un octet à la fois, doit rendre la
            // même chose : ce sont ces longueurs-là qui promènent le résidu
            // de part et d'autre de la frontière du bourrage.
            let mut condensateur = Condensateur::neuf();
            for octet in &message {
                condensateur.absorber(std::slice::from_ref(octet));
            }
            assert_eq!(
                hexa(condensateur.terminer()),
                hex(&message),
                "taille {taille}, un octet à la fois"
            );
        }
        let toutes: std::collections::HashSet<String> = (0..130)
            .map(|n| hex(&vec![b'a'; n]))
            .collect();
        assert_eq!(toutes.len(), 130, "130 longueurs, 130 empreintes distinctes");
    }
}
