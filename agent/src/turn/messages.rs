//! TURN request serialisation and authentication key derivation.
//!
//! `is::stun` can READ TURN responses but cannot EMIT a valid Allocate
//! request: the `REQUESTED-TRANSPORT` attribute (0x0019) is missing from its
//! table. So we write the four requests here, and delegate reading to it.

use std::net::SocketAddr;

use hmac::{Hmac, Mac};

// Protocol constants visible to the whole `turn` module: the state
// machine (`allocation`) needs them to fabricate, in its tests, the
// responses a server would produce. `pub(super)` and not `pub` — they do not
// leave `turn`.

/// Cookie magique STUN (RFC 5389 §6).
pub(super) const MAGIC: [u8; 4] = [0x21, 0x12, 0xA4, 0x42];

// TURN methods. The "request" class being 0b00, the type on the wire is the
// method itself.
pub(super) const METHODE_ALLOCATE: u16 = 0x0003;
pub(super) const METHODE_REFRESH: u16 = 0x0004;
const METHODE_CREATE_PERMISSION: u16 = 0x0008;
pub(super) const METHODE_CHANNEL_BIND: u16 = 0x0009;

// Attributes used. `REQUESTED_TRANSPORT` is the one `is::stun` lacks
// and which motivates this whole serialiser.
const ATTR_USERNAME: u16 = 0x0006;
const ATTR_MESSAGE_INTEGRITY: u16 = 0x0008;
/// Read by `is::stun`, never written by us: used for the simulated responses of
/// `allocation`, hence the `#[cfg(test)]` — without it the constant would be dead
/// code in the binary.
#[cfg(test)]
pub(super) const ATTR_ERROR_CODE: u16 = 0x0009;
const ATTR_CHANNEL_NUMBER: u16 = 0x000C;
pub(super) const ATTR_LIFETIME: u16 = 0x000D;
const ATTR_XOR_PEER_ADDRESS: u16 = 0x0012;
pub(super) const ATTR_REALM: u16 = 0x0014;
pub(super) const ATTR_NONCE: u16 = 0x0015;
/// Relayed address the server grants, and reflexive address it observes.
/// Like `ATTR_ERROR_CODE`: read by `is::stun`, written only in tests.
#[cfg(test)]
pub(super) const ATTR_XOR_RELAYED_ADDRESS: u16 = 0x0016;
const ATTR_REQUESTED_TRANSPORT: u16 = 0x0019;
#[cfg(test)]
pub(super) const ATTR_XOR_MAPPED_ADDRESS: u16 = 0x0020;

/// IANA protocol number for UDP, value of the `REQUESTED-TRANSPORT` field.
const TRANSPORT_UDP: u8 = 17;

/// Lease duration requested at allocation, in seconds. The server may
/// grant another — it is the one it announces that counts, and the
/// refresh aligns with it (see `allocation`).
pub const BAIL_DEMANDE_S: u32 = 600;

/// Long-term credentials, as the server imposes them in its
/// 401 response.
///
/// The password is not included, contrary to what the plan provided:
/// it NEVER goes on the wire, it only serves to derive the integrity key
/// (`cle_longue_duree`), which `encoder_requete` receives separately. A field
/// nothing reads back, and which carries a secret, has no place here.
#[derive(Debug, Clone)]
pub struct Identifiants {
    pub username: String,
    pub realm: String,
    pub nonce: String,
}

/// Request to emit. Each variant carries exactly what distinguishes it.
pub enum Requete {
    /// First attempt, without credentials: it SERVES to provoke the 401
    /// that reveals the realm and the nonce. It is not a failure, it is the normal
    /// step of the protocol.
    AllocateNu,
    AllocateSigne,
    Refresh {
        duree_s: u32,
    },
    CreatePermission {
        pair: SocketAddr,
    },
    ChannelBind {
        canal: u16,
        pair: SocketAddr,
    },
}

/// HMAC-SHA1, in the form `is::stun::verify` and `to_bytes` require.
pub fn sha1_hmac(cle: &[u8], morceaux: &[&[u8]]) -> [u8; 20] {
    let mut mac = Hmac::<sha1::Sha1>::new_from_slice(cle).expect("HMAC accepts any length");
    for morceau in morceaux {
        mac.update(morceau);
    }
    mac.finalize().into_bytes().into()
}

/// Long-term integrity key: `MD5(username:realm:password)` (RFC 5766
/// §4, which takes up RFC 5389 §15.4).
///
/// MD5 is here a normative key derivation, not a choice: the server
/// computes the same, and any other function would produce a systematic 401.
pub fn cle_longue_duree(username: &str, realm: &str, password: &str) -> Vec<u8> {
    use md5::Digest;
    md5::Md5::digest(format!("{username}:{realm}:{password}").as_bytes()).to_vec()
}

/// Serialises a complete TURN request, ready to be sent.
///
/// Absent `identifiants` produces a bare request (without USERNAME/REALM/NONCE or
/// MESSAGE-INTEGRITY): it is the form of the first allocation attempt.
///
/// FINGERPRINT is not emitted: it is optional in TURN, and omitting it avoids
/// having to include it in the integrity computation.
pub fn encoder_requete(
    requete: &Requete,
    trans_id: [u8; 12],
    identifiants: Option<(&Identifiants, &[u8])>,
) -> Vec<u8> {
    let methode = match requete {
        Requete::AllocateNu | Requete::AllocateSigne => METHODE_ALLOCATE,
        Requete::Refresh { .. } => METHODE_REFRESH,
        Requete::CreatePermission { .. } => METHODE_CREATE_PERMISSION,
        Requete::ChannelBind { .. } => METHODE_CHANNEL_BIND,
    };

    let mut attributs: Vec<u8> = Vec::new();

    // The order follows the RFC's: the method's own attributes, then
    // the authentication attributes, then MESSAGE-INTEGRITY last.
    match requete {
        Requete::AllocateNu | Requete::AllocateSigne => {
            write_attribute(
                &mut attributs,
                ATTR_REQUESTED_TRANSPORT,
                &[TRANSPORT_UDP, 0, 0, 0],
            );
            if matches!(requete, Requete::AllocateSigne) {
                write_attribute(&mut attributs, ATTR_LIFETIME, &BAIL_DEMANDE_S.to_be_bytes());
            }
        }
        Requete::Refresh { duree_s } => {
            write_attribute(&mut attributs, ATTR_LIFETIME, &duree_s.to_be_bytes());
        }
        Requete::CreatePermission { pair } => {
            write_attribute(
                &mut attributs,
                ATTR_XOR_PEER_ADDRESS,
                &xor_adresse(*pair, &trans_id),
            );
        }
        Requete::ChannelBind { canal, pair } => {
            write_attribute(
                &mut attributs,
                ATTR_CHANNEL_NUMBER,
                &[(canal >> 8) as u8, *canal as u8, 0, 0],
            );
            write_attribute(
                &mut attributs,
                ATTR_XOR_PEER_ADDRESS,
                &xor_adresse(*pair, &trans_id),
            );
        }
    }

    if let Some((ids, _)) = identifiants {
        write_attribute(&mut attributs, ATTR_USERNAME, ids.username.as_bytes());
        write_attribute(&mut attributs, ATTR_REALM, ids.realm.as_bytes());
        write_attribute(&mut attributs, ATTR_NONCE, ids.nonce.as_bytes());
    }

    let mut paquet = Vec::with_capacity(20 + attributs.len() + 24);
    paquet.extend_from_slice(&methode.to_be_bytes());
    // Length: filled in afterwards, once known. The 24 bytes of
    // MESSAGE-INTEGRITY must be COUNTED in the length at the moment
    // the digest is computed — it is the subtlety that makes most
    // naive implementations fail.
    paquet.extend_from_slice(&[0, 0]);
    paquet.extend_from_slice(&MAGIC);
    paquet.extend_from_slice(&trans_id);
    paquet.extend_from_slice(&attributs);

    let Some((_, cle)) = identifiants else {
        let length = (paquet.len() - 20) as u16;
        paquet[2..4].copy_from_slice(&length.to_be_bytes());
        return paquet;
    };

    // Length announced BEFORE the computation: it already includes the
    // MESSAGE-INTEGRITY attribute not yet written (4 header bytes + 20
    // digest bytes).
    let length_with_integrity = (paquet.len() - 20 + 24) as u16;
    paquet[2..4].copy_from_slice(&length_with_integrity.to_be_bytes());

    let empreinte = sha1_hmac(cle, &[&paquet]);
    write_attribute(&mut paquet, ATTR_MESSAGE_INTEGRITY, &empreinte);
    paquet
}

/// Method of a STUN message, extracted from its type on the wire.
///
/// The type mixes method and class: the class bits (0x0100 and 0x0010) are
/// interleaved in the method bits (RFC 5389 §6). Removing them gives back the
/// method, the only way to know WHICH request a response answers — and hence
/// whether it carries a lease (`Allocate`, `Refresh`) or not (`CreatePermission`,
/// `ChannelBind`).
pub(super) fn methode_de(paquet: &[u8]) -> Option<u16> {
    if paquet.len() < 2 {
        return None;
    }
    let type_fil = u16::from_be_bytes([paquet[0], paquet[1]]);
    Some((type_fil & 0x000F) | ((type_fil & 0x00E0) >> 1) | ((type_fil & 0x3E00) >> 2))
}

/// Writes a TLV attribute, padded to a multiple of 4 bytes.
///
/// `pub(super)`: `allocation` uses it to fabricate its test responses.
pub(super) fn write_attribute(sortie: &mut Vec<u8>, type_: u16, value: &[u8]) {
    sortie.extend_from_slice(&type_.to_be_bytes());
    sortie.extend_from_slice(&(value.len() as u16).to_be_bytes());
    sortie.extend_from_slice(value);
    // Padding is NOT counted in the attribute's announced length,
    // but it must be present on the wire.
    let reste = value.len() % 4;
    if reste != 0 {
        sortie.extend_from_slice(&[0u8; 4][..4 - reste]);
    }
}

/// Encode une adresse au format XOR-MAPPED-ADDRESS (RFC 5389 §15.2).
///
/// The port is masked by the 16 most significant bits of the magic cookie;
/// the address by the whole cookie in IPv4, or by cookie ‖ transaction
/// identifier in IPv6.
///
/// `pub(super)`: `allocation` uses it to fabricate its test responses.
pub(super) fn xor_adresse(addr: SocketAddr, trans_id: &[u8; 12]) -> Vec<u8> {
    let mut sortie = vec![0u8, 0];
    let port = addr.port() ^ u16::from_be_bytes([MAGIC[0], MAGIC[1]]);
    match addr {
        SocketAddr::V4(v4) => {
            sortie[1] = 0x01;
            sortie.extend_from_slice(&port.to_be_bytes());
            for (i, octet) in v4.ip().octets().iter().enumerate() {
                sortie.push(octet ^ MAGIC[i]);
            }
        }
        SocketAddr::V6(v6) => {
            sortie[1] = 0x02;
            sortie.extend_from_slice(&port.to_be_bytes());
            let mut masque = [0u8; 16];
            masque[..4].copy_from_slice(&MAGIC);
            masque[4..].copy_from_slice(trans_id);
            for (i, octet) in v6.ip().octets().iter().enumerate() {
                sortie.push(octet ^ masque[i]);
            }
        }
    }
    sortie
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_long_term_key_is_the_md5_of_the_three_fields() {
        use md5::Digest;

        // Form imposed by RFC 5766 §4 (which takes up RFC 5389 §15.4):
        // the integrity key is MD5("username:realm:password").
        let cle = cle_longue_duree("user", "example.org", "pass");
        let attendu = md5::Md5::digest(b"user:example.org:pass");
        assert_eq!(cle.as_slice(), attendu.as_slice());
        assert_eq!(cle.len(), 16, "an MD5 digest is 16 bytes");
    }

    #[test]
    fn a_bare_allocation_carries_requested_transport_and_no_integrity() {
        let trans_id = [7u8; 12];
        let paquet = encoder_requete(&Requete::AllocateNu, trans_id, None);

        // Header: type 0x0003 (Allocate, request class), magic cookie.
        assert_eq!(&paquet[0..2], &[0x00, 0x03], "Allocate method expected");
        assert_eq!(&paquet[4..8], &[0x21, 0x12, 0xA4, 0x42], "cookie magique");
        assert_eq!(&paquet[8..20], &trans_id);

        // The announced length must match what follows the header.
        let length = u16::from_be_bytes([paquet[2], paquet[3]]) as usize;
        assert_eq!(length, paquet.len() - 20, "inconsistent header length");

        // REQUESTED-TRANSPORT = UDP (17), the attribute is::stun cannot
        // write and without which coturn answers 400.
        assert_eq!(
            &paquet[20..28],
            &[0x00, 0x19, 0x00, 0x04, 17, 0x00, 0x00, 0x00],
            "REQUESTED-TRANSPORT=UDP expected as the first attribute"
        );

        // No integrity on the bare request: it is what provokes the
        // 401 carrying the realm and the nonce.
        assert_eq!(paquet.len(), 28, "no other attribute expected");
    }

    #[test]
    fn a_signed_allocation_is_read_back_and_checked_by_is_stun() {
        // The module's most important test: our serialiser must
        // produce a message the reference parser accepts AND whose
        // integrity it validates. It is what replaces a round trip with a
        // real server.
        let ids = Identifiants {
            username: "user".into(),
            realm: "example.org".into(),
            nonce: "abcdef".into(),
        };
        let cle = cle_longue_duree(&ids.username, &ids.realm, "pass");
        let paquet = encoder_requete(&Requete::AllocateSigne, [3u8; 12], Some((&ids, &cle)));

        let message = is::stun::StunMessage::parse(&paquet).expect("relu par is::stun");
        assert_eq!(message.username(), Some("user"));
        assert_eq!(message.realm(), Some("example.org"));
        assert_eq!(message.nonce(), Some("abcdef"));
        assert!(
            message.verify(&cle, sha1_hmac),
            "MESSAGE-INTEGRITY invalide : le serveur refuserait en 401"
        );
    }

    #[test]
    fn a_permission_carries_the_peer_address_in_xor() {
        let ids = Identifiants {
            username: "u".into(),
            realm: "r".into(),
            nonce: "n".into(),
        };
        let cle = cle_longue_duree(&ids.username, &ids.realm, "p");
        let pair: std::net::SocketAddr = "203.0.113.7:5000".parse().unwrap();
        let paquet = encoder_requete(
            &Requete::CreatePermission { pair },
            [1u8; 12],
            Some((&ids, &cle)),
        );

        let message = is::stun::StunMessage::parse(&paquet).expect("relu par is::stun");
        // The address XOR is done by us and undone by the parser:
        // if the two do not agree, this equality fails.
        assert_eq!(message.xor_peer_address(), Some(pair));
        assert!(message.verify(&cle, sha1_hmac));
    }

    #[test]
    fn a_channel_bind_carries_the_number_and_the_address() {
        let ids = Identifiants {
            username: "u".into(),
            realm: "r".into(),
            nonce: "n".into(),
        };
        let cle = cle_longue_duree(&ids.username, &ids.realm, "p");
        let pair: std::net::SocketAddr = "198.51.100.9:1234".parse().unwrap();
        let paquet = encoder_requete(
            &Requete::ChannelBind {
                canal: 0x4000,
                pair,
            },
            [2u8; 12],
            Some((&ids, &cle)),
        );

        let message = is::stun::StunMessage::parse(&paquet).expect("relu par is::stun");
        assert_eq!(message.channel_number(), Some(0x4000));
        assert_eq!(message.xor_peer_address(), Some(pair));
    }
}
