//! Sérialisation des requêtes TURN et dérivation des clés d'authentification.
//!
//! `is::stun` sait LIRE les réponses TURN mais ne sait pas ÉMETTRE une requête
//! Allocate valide : l'attribut `REQUESTED-TRANSPORT` (0x0019) est absent de sa
//! table. On écrit donc les quatre requêtes ici, et on lui délègue la lecture.

use std::net::SocketAddr;

use hmac::{Hmac, Mac};

// Constantes de protocole visibles de tout le module `turn` : la machine à
// états (`allocation`) en a besoin pour fabriquer, dans ses tests, les
// réponses qu'un serveur produirait. `pub(super)` et non `pub` — elles ne
// sortent pas de `turn`.

/// Cookie magique STUN (RFC 5389 §6).
pub(super) const MAGIC: [u8; 4] = [0x21, 0x12, 0xA4, 0x42];

// Méthodes TURN. Classe « requête » valant 0b00, le type sur le fil est la
// méthode elle-même.
pub(super) const METHODE_ALLOCATE: u16 = 0x0003;
pub(super) const METHODE_REFRESH: u16 = 0x0004;
const METHODE_CREATE_PERMISSION: u16 = 0x0008;
const METHODE_CHANNEL_BIND: u16 = 0x0009;

// Attributs employés. `REQUESTED_TRANSPORT` est celui qui manque à
// `is::stun` et qui motive tout ce sérialiseur.
const ATTR_USERNAME: u16 = 0x0006;
const ATTR_MESSAGE_INTEGRITY: u16 = 0x0008;
/// Lu par `is::stun`, jamais écrit par nous : sert aux réponses simulées de
/// `allocation`, d'où le `#[cfg(test)]` — sans lui la constante serait du code
/// mort dans le binaire.
#[cfg(test)]
pub(super) const ATTR_ERROR_CODE: u16 = 0x0009;
const ATTR_CHANNEL_NUMBER: u16 = 0x000C;
pub(super) const ATTR_LIFETIME: u16 = 0x000D;
const ATTR_XOR_PEER_ADDRESS: u16 = 0x0012;
pub(super) const ATTR_REALM: u16 = 0x0014;
pub(super) const ATTR_NONCE: u16 = 0x0015;
/// Adresse relayée que le serveur accorde, et adresse réflexive qu'il observe.
/// Comme `ATTR_ERROR_CODE` : lues par `is::stun`, écrites seulement en test.
#[cfg(test)]
pub(super) const ATTR_XOR_RELAYED_ADDRESS: u16 = 0x0016;
const ATTR_REQUESTED_TRANSPORT: u16 = 0x0019;
#[cfg(test)]
pub(super) const ATTR_XOR_MAPPED_ADDRESS: u16 = 0x0020;

/// Numéro de protocole d'IANA pour UDP, valeur du champ `REQUESTED-TRANSPORT`.
const TRANSPORT_UDP: u8 = 17;

/// Durée de bail demandée à l'allocation, en secondes. Le serveur peut en
/// accorder une autre — c'est celle qu'il annonce qui fait foi, et le
/// rafraîchissement se cale dessus (voir `allocation`).
pub const BAIL_DEMANDE_S: u32 = 600;

/// Identifiants longue durée, tels que le serveur les impose dans sa
/// réponse 401.
///
/// Le mot de passe n'y figure pas, contrairement à ce que prévoyait le plan :
/// il ne va JAMAIS sur le fil, il ne sert qu'à dériver la clé d'intégrité
/// (`cle_longue_duree`), que `encoder_requete` reçoit séparément. Un champ que
/// rien ne relit, et qui porte un secret, n'a pas sa place ici.
#[derive(Debug, Clone)]
pub struct Identifiants {
    pub username: String,
    pub realm: String,
    pub nonce: String,
}

/// Requête à émettre. Chaque variante porte exactement ce qui la distingue.
pub enum Requete {
    /// Première tentative, sans identifiants : elle SERT à provoquer le 401
    /// qui révèle le realm et le nonce. Ce n'est pas un échec, c'est l'étape
    /// normale du protocole.
    AllocateNu,
    AllocateSigne,
    Refresh { duree_s: u32 },
    CreatePermission { pair: SocketAddr },
    ChannelBind { canal: u16, pair: SocketAddr },
}

/// HMAC-SHA1, dans la forme que réclament `is::stun::verify` et `to_bytes`.
pub fn sha1_hmac(cle: &[u8], morceaux: &[&[u8]]) -> [u8; 20] {
    let mut mac = Hmac::<sha1::Sha1>::new_from_slice(cle).expect("HMAC accepte toute longueur");
    for morceau in morceaux {
        mac.update(morceau);
    }
    mac.finalize().into_bytes().into()
}

/// Clé d'intégrité longue durée : `MD5(username:realm:password)` (RFC 5766
/// §4, qui reprend RFC 5389 §15.4).
///
/// MD5 est ici une dérivation de clé normative, pas un choix : le serveur
/// calcule la même, et toute autre fonction produirait un 401 systématique.
pub fn cle_longue_duree(username: &str, realm: &str, password: &str) -> Vec<u8> {
    use md5::Digest;
    md5::Md5::digest(format!("{username}:{realm}:{password}").as_bytes()).to_vec()
}

/// Sérialise une requête TURN complète, prête à être envoyée.
///
/// `identifiants` absent produit une requête nue (sans USERNAME/REALM/NONCE ni
/// MESSAGE-INTEGRITY) : c'est la forme de la première tentative d'allocation.
///
/// FINGERPRINT n'est pas émis : il est facultatif en TURN, et l'omettre évite
/// d'avoir à l'inclure dans le calcul d'intégrité.
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

    // L'ordre suit celui de la RFC : les attributs propres à la méthode, puis
    // les attributs d'authentification, puis MESSAGE-INTEGRITY en dernier.
    match requete {
        Requete::AllocateNu | Requete::AllocateSigne => {
            ecrire_attribut(
                &mut attributs,
                ATTR_REQUESTED_TRANSPORT,
                &[TRANSPORT_UDP, 0, 0, 0],
            );
            if matches!(requete, Requete::AllocateSigne) {
                ecrire_attribut(&mut attributs, ATTR_LIFETIME, &BAIL_DEMANDE_S.to_be_bytes());
            }
        }
        Requete::Refresh { duree_s } => {
            ecrire_attribut(&mut attributs, ATTR_LIFETIME, &duree_s.to_be_bytes());
        }
        Requete::CreatePermission { pair } => {
            ecrire_attribut(
                &mut attributs,
                ATTR_XOR_PEER_ADDRESS,
                &xor_adresse(*pair, &trans_id),
            );
        }
        Requete::ChannelBind { canal, pair } => {
            ecrire_attribut(
                &mut attributs,
                ATTR_CHANNEL_NUMBER,
                &[(canal >> 8) as u8, *canal as u8, 0, 0],
            );
            ecrire_attribut(
                &mut attributs,
                ATTR_XOR_PEER_ADDRESS,
                &xor_adresse(*pair, &trans_id),
            );
        }
    }

    if let Some((ids, _)) = identifiants {
        ecrire_attribut(&mut attributs, ATTR_USERNAME, ids.username.as_bytes());
        ecrire_attribut(&mut attributs, ATTR_REALM, ids.realm.as_bytes());
        ecrire_attribut(&mut attributs, ATTR_NONCE, ids.nonce.as_bytes());
    }

    let mut paquet = Vec::with_capacity(20 + attributs.len() + 24);
    paquet.extend_from_slice(&methode.to_be_bytes());
    // Longueur : renseignée après, une fois connue. Les 24 octets de
    // MESSAGE-INTEGRITY doivent être COMPTÉS dans la longueur au moment où
    // l'empreinte est calculée — c'est la subtilité qui fait échouer la
    // plupart des implémentations naïves.
    paquet.extend_from_slice(&[0, 0]);
    paquet.extend_from_slice(&MAGIC);
    paquet.extend_from_slice(&trans_id);
    paquet.extend_from_slice(&attributs);

    let Some((_, cle)) = identifiants else {
        let longueur = (paquet.len() - 20) as u16;
        paquet[2..4].copy_from_slice(&longueur.to_be_bytes());
        return paquet;
    };

    // Longueur annoncée AVANT le calcul : elle inclut déjà l'attribut
    // MESSAGE-INTEGRITY qui n'est pas encore écrit (4 octets d'en-tête + 20
    // d'empreinte).
    let longueur_avec_integrite = (paquet.len() - 20 + 24) as u16;
    paquet[2..4].copy_from_slice(&longueur_avec_integrite.to_be_bytes());

    let empreinte = sha1_hmac(cle, &[&paquet]);
    ecrire_attribut(&mut paquet, ATTR_MESSAGE_INTEGRITY, &empreinte);
    paquet
}

/// Écrit un attribut TLV, complété à un multiple de 4 octets.
///
/// `pub(super)` : `allocation` s'en sert pour fabriquer ses réponses de test.
pub(super) fn ecrire_attribut(sortie: &mut Vec<u8>, type_: u16, valeur: &[u8]) {
    sortie.extend_from_slice(&type_.to_be_bytes());
    sortie.extend_from_slice(&(valeur.len() as u16).to_be_bytes());
    sortie.extend_from_slice(valeur);
    // Le remplissage n'est PAS compté dans la longueur annoncée de
    // l'attribut, mais il doit être présent sur le fil.
    let reste = valeur.len() % 4;
    if reste != 0 {
        sortie.extend_from_slice(&[0u8; 4][..4 - reste]);
    }
}

/// Encode une adresse au format XOR-MAPPED-ADDRESS (RFC 5389 §15.2).
///
/// Le port est masqué par les 16 bits de poids fort du cookie magique ;
/// l'adresse par le cookie entier en IPv4, ou par cookie ‖ identifiant de
/// transaction en IPv6.
///
/// `pub(super)` : `allocation` s'en sert pour fabriquer ses réponses de test.
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
    fn la_cle_longue_duree_est_le_md5_des_trois_champs() {
        use md5::Digest;

        // Forme imposée par la RFC 5766 §4 (qui reprend RFC 5389 §15.4) :
        // la clé d'intégrité vaut MD5("username:realm:password").
        let cle = cle_longue_duree("user", "example.org", "pass");
        let attendu = md5::Md5::digest(b"user:example.org:pass");
        assert_eq!(cle.as_slice(), attendu.as_slice());
        assert_eq!(cle.len(), 16, "un condensé MD5 fait 16 octets");
    }

    #[test]
    fn une_allocation_nue_porte_requested_transport_et_pas_d_integrite() {
        let trans_id = [7u8; 12];
        let paquet = encoder_requete(&Requete::AllocateNu, trans_id, None);

        // En-tête : type 0x0003 (Allocate, classe requête), cookie magique.
        assert_eq!(&paquet[0..2], &[0x00, 0x03], "méthode Allocate attendue");
        assert_eq!(&paquet[4..8], &[0x21, 0x12, 0xA4, 0x42], "cookie magique");
        assert_eq!(&paquet[8..20], &trans_id);

        // La longueur annoncée doit correspondre à ce qui suit l'en-tête.
        let longueur = u16::from_be_bytes([paquet[2], paquet[3]]) as usize;
        assert_eq!(longueur, paquet.len() - 20, "longueur d'en-tête incohérente");

        // REQUESTED-TRANSPORT = UDP (17), l'attribut que is::stun ne sait pas
        // écrire et sans lequel coturn répond 400.
        assert_eq!(
            &paquet[20..28],
            &[0x00, 0x19, 0x00, 0x04, 17, 0x00, 0x00, 0x00],
            "REQUESTED-TRANSPORT=UDP attendu en premier attribut"
        );

        // Aucune intégrité sur la requête nue : c'est elle qui provoque le
        // 401 porteur du realm et du nonce.
        assert_eq!(paquet.len(), 28, "aucun autre attribut attendu");
    }

    #[test]
    fn une_allocation_signee_est_relue_et_verifiee_par_is_stun() {
        // Le test le plus important du module : notre sérialiseur doit
        // produire un message que le parseur de référence accepte ET dont il
        // valide l'intégrité. C'est ce qui remplace un aller-retour avec un
        // vrai serveur.
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
    fn une_permission_porte_l_adresse_du_pair_en_xor() {
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
        // Le XOR de l'adresse est fait par nous et défait par le parseur :
        // si les deux ne s'accordent pas, cette égalité échoue.
        assert_eq!(message.xor_peer_address(), Some(pair));
        assert!(message.verify(&cle, sha1_hmac));
    }

    #[test]
    fn un_channel_bind_porte_le_numero_et_l_adresse() {
        let ids = Identifiants {
            username: "u".into(),
            realm: "r".into(),
            nonce: "n".into(),
        };
        let cle = cle_longue_duree(&ids.username, &ids.realm, "p");
        let pair: std::net::SocketAddr = "198.51.100.9:1234".parse().unwrap();
        let paquet = encoder_requete(
            &Requete::ChannelBind { canal: 0x4000, pair },
            [2u8; 12],
            Some((&ids, &cle)),
        );

        let message = is::stun::StunMessage::parse(&paquet).expect("relu par is::stun");
        assert_eq!(message.channel_number(), Some(0x4000));
        assert_eq!(message.xor_peer_address(), Some(pair));
    }
}
