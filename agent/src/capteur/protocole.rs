//! Messages et cadrage du canal entre le capteur et un enfant.
//!
//! **Pas de `#[cfg(windows)]`** : c'est de la sérialisation pure, et c'est
//! justement le genre de contrat qui doit être éprouvé sur l'hôte — un nom de
//! champ qui dérive ne se verrait autrement qu'en session réelle sur la VM.
//! Même motif et même montage que `superviseur/protocole.rs`.

use std::io::{self, Read, Write};

use serde::{Deserialize, Serialize};

use crate::h264::AccessUnit;

/// Nom du tube nommé sur lequel le capteur accepte ses enfants.
pub const NOM_TUBE: &str = r"\\.\pipe\agent-capteur";

/// Borne de taille d'une trame, éprouvée AVANT toute allocation.
///
/// Une unité d'accès à 8 Mb/s pèse quelques dizaines de kilooctets ; une image
/// clé de démarrage à haute résolution reste très en deçà du mégaoctet. 8 Mio
/// laissent trois ordres de grandeur de marge tout en rendant impossible
/// qu'une longueur corrompue fasse réserver des gigaoctets.
pub const TAILLE_MAX: usize = 8 * 1024 * 1024;

pub const ETIQUETTE_JSON: u8 = 1;
pub const ETIQUETTE_IMAGE: u8 = 2;

/// En-tête binaire d'une image : 8 octets de `pts_90k`, 1 octet d'image clé.
const EN_TETE_IMAGE: usize = 9;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum VersCapteur {
    /// Premier message d'un enfant : il se décrit lui-même. Le capteur n'a
    /// besoin d'aucune information venue du superviseur.
    Attache {
        session: String,
        hwnd: u64,
        sortie: String,
        fps: u32,
        debit: u32,
        /// La taille RETENUE que le superviseur a posée sur cette fenêtre
        /// (`TAILLE_FENETRE`), pas la taille de la sortie — qui peut être
        /// bien plus grande sur un registre pollué. `(u32::MAX, u32::MAX)`
        /// quand l'enfant ne la connaît pas (chemin mono-fenêtre, sans
        /// superviseur) : `taille_retenue` la ramène alors à la taille de la
        /// sortie, ce qui reproduit le comportement d'avant ce sous-bloc.
        /// Non consommé avant la tâche 8 — voir `Fenetre::ouvrir`.
        taille: (u32, u32),
        /// `QueryPerformanceCounter` lu par l'enfant au moment même où il crée
        /// son `clock_origin`. Un `Instant` n'a aucun sens dans un autre
        /// processus ; QPC, lui, est commun à toute la machine. Sans ce
        /// rebasage, la vidéo de l'enfant porteur du son serait décalée de
        /// l'écart entre les deux origines.
        origine_qpc: i64,
    },
    /// Première et **unique** trame de la connexion média : elle apparie ce
    /// second tube à la session déjà attachée sur la connexion de commandes.
    /// Après elle, l'enfant n'écrit plus jamais sur cette connexion — c'est
    /// ce qui garantit qu'aucune lecture et écriture n'y sont concurrentes.
    Identite { session: String },
    Redimensionner { largeur: u32, hauteur: u32 },
    TailleEncodage { largeur: u32, hauteur: u32 },
    Debit { bps: u32 },
    ImageCle,
    /// Visibilité annoncée par le client, relayée par l'enfant.
    ///
    /// **Ne se répond pas par `Fait`** : le capteur arbitre globalement, et la
    /// décision peut concerner une AUTRE fenêtre que celle qui a signalé.
    /// L'effet revient par `DepuisCapteur::Sommeil`, poussé sur la connexion
    /// média de chaque fenêtre concernée.
    Visibilite { visible: bool, focalisee: bool },
    /// La capture audio de cette fenêtre est morte définitivement, après
    /// `crate::audio::LECTURES_ECHOUEES_MAX` erreurs de lecture consécutives.
    ///
    /// **Aucune charge utile** : la session est celle du canal, comme pour
    /// toutes les commandes — `capteur/fenetre/commandes.rs` la tire de son
    /// contexte.
    ///
    /// **Ne se répond pas par `Fait` au sens de l'effet** : le capteur
    /// ré-arbitre globalement, et la décision peut concerner une AUTRE fenêtre
    /// du même groupe de PID. L'effet revient par `DepuisCapteur::Audio`,
    /// poussé sur la connexion média. Même patron exactement que `Visibilite`.
    AudioMort,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DepuisCapteur {
    Attachee { largeur: u32, hauteur: u32 },
    Refus { motif: String },
    Taille { largeur: u32, hauteur: u32 },
    Fait,
    Erreur { motif: String },
    /// Émis **au changement seulement**, jamais périodiquement : il alimente
    /// le cache que lisent `is_alive`, `is_exhausted` et `dimensions`, qui
    /// sont interrogées à chaque tour de la boucle de transport.
    Etat { vivante: bool, epuisee: bool, largeur: u32, hauteur: u32 },
    /// Poussé, non sollicité, quand une fenêtre change d'état de sommeil.
    ///
    /// Distinct d'`Etat` à dessein : `Etat` alimente un cache lu à chaque tour
    /// de la boucle de transport (`is_alive`, `is_exhausted`, `dimensions`),
    /// et y mêler le sommeil ferait passer une annonce ponctuelle par un
    /// chemin conçu pour un état permanent.
    Sommeil { endormie: bool, raison: String },
    /// Part du budget de débit de session accordée à cette fenêtre, poussée
    /// non sollicitée quand elle CHANGE.
    ///
    /// Distincte d'`Etat` pour la même raison que `Sommeil` : `Etat` alimente
    /// un cache lu à chaque tour de la boucle de transport, et y mêler une
    /// annonce ponctuelle passerait par un chemin conçu pour un état permanent.
    ///
    /// L'enfant l'applique en DEUX endroits (`transport/part.rs`), et c'est le
    /// PREMIER qui agit : `Controleur::changer_plafond` borne ce que l'encodeur
    /// produit, donc fait descendre l'échelle d'un barreau, donc réduit la
    /// RÉSOLUTION — le seul levier que la recette de D6 ait mesuré efficace.
    /// `rtc.bwe().set_desired_bitrate` arrête en plus le sondage à la hausse,
    /// excessif en principe à N fenêtres.
    ///
    /// ⚠️ **Le second n'est pas « la vraie cause de la congestion à N
    /// fenêtres », et la prémisse qui le disait a été RÉFUTÉE par la branche
    /// elle-même** : le pont porte ≥ 1,44 Gb/s, `packetsLost` vaut 0 aux onze
    /// exécutions, il n'y a jamais eu de congestion de lien. Ce qui sature est
    /// le décodeur du navigateur.
    ///
    /// **Une part d'ENDORMIE ne va qu'au second** — voir
    /// `capteur::repartiteur::PART_DORMANTE_BPS` et `Session::appliquer_part`.
    Part { bps: u32 },
    /// Ordre de porter le son, ou de se taire. Poussé non sollicité, **au
    /// changement seulement**.
    ///
    /// Distinct d'`Etat` pour la même raison que `Sommeil` et `Part` : `Etat`
    /// alimente un cache lu à chaque tour de la boucle de transport, et y mêler
    /// une annonce ponctuelle passerait par un chemin conçu pour un état
    /// permanent.
    ///
    /// **Le capteur ne capte AUCUN son.** Il arbitre seulement : il sait quelles
    /// fenêtres partagent un processus (il a leurs `hwnd`) et qui a le focus,
    /// ce que l'enfant ignore. La capture, elle, vit dans l'enfant — le *process
    /// loopback* n'a aucune des propriétés qui avaient forcé la mutualisation de
    /// la vidéo en D4.
    Audio { actif: bool },
    /// La fenêtre Windows est passée en plein écran, ou en est sortie. Poussé
    /// non sollicité, **au changement seulement**.
    ///
    /// Distinct d'`Etat` pour la même raison que `Sommeil`, `Part` et `Audio` :
    /// `Etat` alimente un cache lu à chaque tour de la boucle de transport, et
    /// y mêler une annonce ponctuelle passerait par un chemin conçu pour un
    /// état permanent.
    PleinEcran { actif: bool },
}

#[derive(Debug)]
pub enum Trame {
    Json(Vec<u8>),
    Image(AccessUnit),
}

fn ecrire_trame<W: Write>(sortie: &mut W, etiquette: u8, corps: &[u8]) -> io::Result<()> {
    let longueur = corps.len() + 1;
    if longueur > TAILLE_MAX {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("trame de {longueur} octets au-dessus de la borne {TAILLE_MAX}"),
        ));
    }
    sortie.write_all(&(longueur as u32).to_le_bytes())?;
    sortie.write_all(&[etiquette])?;
    sortie.write_all(corps)
}

pub fn ecrire_json<W: Write, T: Serialize>(sortie: &mut W, message: &T) -> io::Result<()> {
    let corps = serde_json::to_vec(message).map_err(io::Error::other)?;
    ecrire_trame(sortie, ETIQUETTE_JSON, &corps)
}

pub fn ecrire_image<W: Write>(sortie: &mut W, unite: &AccessUnit) -> io::Result<()> {
    let mut corps = Vec::with_capacity(EN_TETE_IMAGE + unite.data.len());
    corps.extend_from_slice(&unite.pts_90k.to_le_bytes());
    corps.push(u8::from(unite.is_keyframe));
    corps.extend_from_slice(&unite.data);
    ecrire_trame(sortie, ETIQUETTE_IMAGE, &corps)
}

pub fn lire_trame<R: Read>(entree: &mut R) -> io::Result<Trame> {
    let mut longueur = [0u8; 4];
    entree.read_exact(&mut longueur)?;
    let longueur = u32::from_le_bytes(longueur) as usize;
    if longueur == 0 || longueur > TAILLE_MAX {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("longueur de trame aberrante : {longueur}"),
        ));
    }
    let mut corps = vec![0u8; longueur];
    entree.read_exact(&mut corps)?;
    let etiquette = corps[0];
    let corps = &corps[1..];
    match etiquette {
        ETIQUETTE_JSON => Ok(Trame::Json(corps.to_vec())),
        ETIQUETTE_IMAGE => {
            if corps.len() < EN_TETE_IMAGE {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "trame image sans en-tête complet",
                ));
            }
            let pts_90k = u64::from_le_bytes(corps[..8].try_into().expect("8 octets"));
            Ok(Trame::Image(AccessUnit {
                pts_90k,
                is_keyframe: corps[8] != 0,
                data: corps[EN_TETE_IMAGE..].to_vec(),
            }))
        }
        // REFUSÉE et non ignorée : un flux mal aligné doit tuer le canal
        // plutôt que de faire dériver la lecture sur des octets arbitraires.
        autre => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("étiquette de trame inconnue : {autre}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::h264::AccessUnit;
    use std::io::Cursor;

    #[test]
    fn une_attache_fait_l_aller_retour() {
        let message = VersCapteur::Attache {
            session: "w-1".into(),
            hwnd: 0x1a2b,
            sortie: r"\\.\DISPLAY8".into(),
            fps: 90,
            debit: 8_000_000,
            taille: (1280, 720),
            origine_qpc: 123_456_789,
        };
        let mut tampon = Vec::new();
        ecrire_json(&mut tampon, &message).unwrap();
        let mut lecteur = Cursor::new(tampon);
        match lire_trame(&mut lecteur).unwrap() {
            Trame::Json(octets) => {
                assert_eq!(serde_json::from_slice::<VersCapteur>(&octets).unwrap(), message)
            }
            autre => panic!("attendu du JSON, reçu {autre:?}"),
        }
    }

    /// L'identité est la trame qui apparie la connexion média à la session
    /// déjà attachée sur la connexion de commandes : un nom de champ qui
    /// dériverait ferait échouer l'appariement en session réelle seulement.
    #[test]
    fn une_identite_fait_l_aller_retour() {
        let message = VersCapteur::Identite { session: "w-1".into() };
        let mut tampon = Vec::new();
        ecrire_json(&mut tampon, &message).unwrap();
        let mut lecteur = Cursor::new(tampon);
        match lire_trame(&mut lecteur).unwrap() {
            Trame::Json(octets) => {
                assert_eq!(serde_json::from_slice::<VersCapteur>(&octets).unwrap(), message)
            }
            autre => panic!("attendu du JSON, reçu {autre:?}"),
        }
    }

    #[test]
    fn chaque_reponse_fait_l_aller_retour() {
        for message in [
            DepuisCapteur::Attachee { largeur: 1280, hauteur: 720 },
            DepuisCapteur::Refus { motif: "sortie inconnue".into() },
            DepuisCapteur::Taille { largeur: 1280, hauteur: 720 },
            DepuisCapteur::Fait,
            DepuisCapteur::Erreur { motif: "encodeur perdu".into() },
            DepuisCapteur::Etat { vivante: true, epuisee: false, largeur: 1280, hauteur: 720 },
        ] {
            let mut tampon = Vec::new();
            ecrire_json(&mut tampon, &message).unwrap();
            let mut lecteur = Cursor::new(tampon);
            let Trame::Json(octets) = lire_trame(&mut lecteur).unwrap() else {
                panic!("attendu du JSON")
            };
            assert_eq!(serde_json::from_slice::<DepuisCapteur>(&octets).unwrap(), message);
        }
    }

    /// L'unité d'accès voyage en BINAIRE BRUT, jamais en base64 : c'est le
    /// seul message dont le volume compte (8 Mb/s par fenêtre).
    #[test]
    fn une_unite_d_acces_fait_l_aller_retour_sans_reencodage() {
        let unite = AccessUnit {
            data: vec![0, 0, 0, 1, 0x67, 0xff, 0x00, 0x01],
            is_keyframe: true,
            pts_90k: 90_000,
        };
        let mut tampon = Vec::new();
        ecrire_image(&mut tampon, &unite).unwrap();
        // 4 (longueur) + 1 (étiquette) + 8 (pts) + 1 (clé) + 8 (données)
        assert_eq!(tampon.len(), 22, "cadrage inattendu : {tampon:?}");
        let mut lecteur = Cursor::new(tampon);
        match lire_trame(&mut lecteur).unwrap() {
            Trame::Image(rendue) => assert_eq!(rendue, unite),
            autre => panic!("attendu une image, reçu {autre:?}"),
        }
    }

    #[test]
    fn deux_trames_a_la_suite_se_lisent_dans_l_ordre() {
        let mut tampon = Vec::new();
        ecrire_json(&mut tampon, &DepuisCapteur::Fait).unwrap();
        ecrire_image(
            &mut tampon,
            &AccessUnit { data: vec![9, 9], is_keyframe: false, pts_90k: 7 },
        )
        .unwrap();
        let mut lecteur = Cursor::new(tampon);
        assert!(matches!(lire_trame(&mut lecteur).unwrap(), Trame::Json(_)));
        assert!(matches!(lire_trame(&mut lecteur).unwrap(), Trame::Image(_)));
    }

    /// Une étiquette inconnue est REFUSÉE, jamais ignorée : un flux mal
    /// aligné doit tuer le canal plutôt que de faire dériver la lecture.
    #[test]
    fn une_etiquette_inconnue_est_refusee() {
        let mut tampon = Vec::new();
        tampon.extend_from_slice(&2u32.to_le_bytes());
        tampon.push(99);
        tampon.push(0);
        assert!(lire_trame(&mut Cursor::new(tampon)).is_err());
    }

    #[test]
    fn une_trame_tronquee_est_refusee() {
        let mut tampon = Vec::new();
        ecrire_json(&mut tampon, &DepuisCapteur::Fait).unwrap();
        tampon.truncate(tampon.len() - 1);
        assert!(lire_trame(&mut Cursor::new(tampon)).is_err());
    }

    /// Sans cette borne, une longueur corrompue ferait réserver des gigaoctets
    /// avant même de lire un octet de corps.
    #[test]
    fn une_longueur_aberrante_est_refusee_avant_toute_allocation() {
        let mut tampon = Vec::new();
        tampon.extend_from_slice(&(TAILLE_MAX as u32 + 1).to_le_bytes());
        tampon.push(1);
        assert!(lire_trame(&mut Cursor::new(tampon)).is_err());
    }

    #[test]
    fn une_visibilite_traverse_le_canal_du_capteur() {
        let message = VersCapteur::Visibilite { visible: true, focalisee: false };
        let json = serde_json::to_string(&message).expect("sérialisation");
        let relu: VersCapteur = serde_json::from_str(&json).expect("désérialisation");
        assert_eq!(relu, message);
    }

    #[test]
    fn un_sommeil_traverse_le_canal_du_capteur() {
        let message = DepuisCapteur::Sommeil { endormie: true, raison: "masquee".into() };
        let json = serde_json::to_string(&message).expect("sérialisation");
        let relu: DepuisCapteur = serde_json::from_str(&json).expect("désérialisation");
        assert_eq!(relu, message);
    }

    #[test]
    fn une_part_traverse_l_encodage_json() {
        let mut tampon = Vec::new();
        ecrire_json(&mut tampon, &DepuisCapteur::Part { bps: 4_000_000 }).unwrap();
        let mut lecture = &tampon[..];
        let Trame::Json(corps) = lire_trame(&mut lecture).unwrap() else {
            panic!("une trame JSON était attendue");
        };
        let message: DepuisCapteur = serde_json::from_slice(&corps).unwrap();
        assert_eq!(message, DepuisCapteur::Part { bps: 4_000_000 });
    }

    /// Une image de zéro octet n'existe pas : elle signalerait un cadrage
    /// perdu, pas une image vide.
    #[test]
    fn une_image_sans_en_tete_complet_est_refusee() {
        let mut tampon = Vec::new();
        tampon.extend_from_slice(&3u32.to_le_bytes());
        tampon.push(ETIQUETTE_IMAGE);
        tampon.extend_from_slice(&[0, 0]);
        assert!(lire_trame(&mut Cursor::new(tampon)).is_err());
    }
}
