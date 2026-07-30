//! Échafaudages partagés des tests de `turn` : l'horloge de départ, l'adresse
//! du serveur, la fabrique de réponses qui remplace coturn, et le client déjà
//! amené jusqu'à l'état alloué.
//!
//! `#[cfg(test)]` : rien de ceci n'est compilé en `release`.
//!
//! N'y figurent que les éléments réellement partagés par `allocation` et
//! `canaux` — même critère que `transport/fixtures.rs`.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use super::allocation::TurnClient;
use super::messages::{
    ecrire_attribut, xor_adresse, ATTR_ERROR_CODE, ATTR_LIFETIME, ATTR_NONCE, ATTR_REALM,
    ATTR_XOR_MAPPED_ADDRESS, ATTR_XOR_RELAYED_ADDRESS, MAGIC, METHODE_ALLOCATE,
};

/// Instant de référence des tests, dans le passé : `avancer` peut ainsi porter
/// l'horloge de plusieurs minutes sans jamais dépasser `Instant::now()`.
pub(super) fn t0() -> Instant {
    Instant::now() - Duration::from_secs(3600)
}

pub(super) fn serveur() -> SocketAddr {
    "192.0.2.1:3478".parse().unwrap()
}

/// Fabrique la réponse qu'un serveur produirait, pour piloter le client
/// sans réseau. C'est ce qui remplace coturn dans ces tests.
pub(super) fn reponse(
    methode: u16,
    classe_succes: bool,
    trans_id: [u8; 12],
    attributs: &[(u16, Vec<u8>)],
) -> Vec<u8> {
    let mut corps = Vec::new();
    for (type_, valeur) in attributs {
        ecrire_attribut(&mut corps, *type_, valeur);
    }
    // Classe succès = 0b10 → bits 0x0100 ; classe erreur = 0b11 → 0x0110.
    let type_fil = methode | if classe_succes { 0x0100 } else { 0x0110 };
    let mut paquet = Vec::new();
    paquet.extend_from_slice(&type_fil.to_be_bytes());
    paquet.extend_from_slice(&((corps.len()) as u16).to_be_bytes());
    paquet.extend_from_slice(&MAGIC);
    paquet.extend_from_slice(&trans_id);
    paquet.extend_from_slice(&corps);
    paquet
}

pub(super) fn trans_id_de(paquet: &[u8]) -> [u8; 12] {
    paquet[8..20].try_into().unwrap()
}

/// Amène un client jusqu'à l'état alloué, pour les tests qui partent de là.
pub(super) fn allouee() -> TurnClient {
    let mut c = TurnClient::new(serveur(), "u".into(), "p".into(), t0());
    let nue = c.poll_transmit().unwrap();
    let refus = reponse(
        METHODE_ALLOCATE,
        false,
        trans_id_de(&nue),
        &[
            (ATTR_ERROR_CODE, vec![0, 0, 4, 1, b'x']),
            (ATTR_REALM, b"r".to_vec()),
            (ATTR_NONCE, b"n1".to_vec()),
        ],
    );
    c.handle_packet(&refus).unwrap();
    let signee = c.poll_transmit().unwrap();
    let succes = reponse(
        METHODE_ALLOCATE,
        true,
        trans_id_de(&signee),
        &[
            (
                ATTR_XOR_RELAYED_ADDRESS,
                xor_adresse("192.0.2.15:50000".parse().unwrap(), &trans_id_de(&signee)),
            ),
            (
                ATTR_XOR_MAPPED_ADDRESS,
                xor_adresse("203.0.113.4:41234".parse().unwrap(), &trans_id_de(&signee)),
            ),
            (ATTR_LIFETIME, 600u32.to_be_bytes().to_vec()),
        ],
    );
    c.handle_packet(&succes).unwrap();
    c
}
