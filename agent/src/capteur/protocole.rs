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
    /// Premier message d'un enfant : il se décrit lui-même. Il n'existe
    /// **aucun canal direct superviseur→capteur** ; ce qui vient du
    /// superviseur (par exemple `taille`, ci-dessous) transite par l'enfant,
    /// qui le lui redit ici.
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
    /// La capture audio de cette fenêtre a cessé de produire du son, et
    /// l'enfant a **épuisé ses moyens de la rétablir**.
    ///
    /// ❌ **Ce champ disait « morte DÉFINITIVEMENT, après
    /// `LECTURES_ECHOUEES_MAX` erreurs de lecture consécutives », et les DEUX
    /// moitiés sont fausses depuis le sous-bloc D10** — relevé par la revue
    /// transverse, la tâche qui a ajouté `AudioVivant` juste en dessous
    /// n'ayant pas relu la variante du dessus.
    ///
    /// - **Pas définitivement** : `Session::reconstruire_ou_signaler`
    ///   (`transport/piste_audio.rs`) refabrique la capture, et
    ///   `VersCapteur::AudioVivant` existe précisément pour prouver la
    ///   reprise par un paquet réel.
    /// - **Pas au bout de `LECTURES_ECHOUEES_MAX`** : ces dix erreurs posent
    ///   `capture_morte`, rien de plus. Ce message-ci ne part **qu'en
    ///   REPLI** — quand `crate::audio::RECONSTRUCTIONS_MAX` tentatives de
    ///   reconstruction ont été épuisées, ou qu'il n'existe aucun
    ///   reconstructeur.
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
    /// La capture audio de cette fenêtre vient d'apporter la PREUVE qu'elle
    /// est repartie : un paquet réel a été produit, pas seulement une
    /// reconstruction qui a rendu `Ok` (sous-bloc D10, ferme le leg 6 de D9).
    ///
    /// **Aucune charge utile**, comme `AudioMort`. **Ne se répond pas par
    /// `Fait` au sens de l'effet non plus** : elle ne fait que remettre à
    /// zéro le compteur de réarmements de CETTE session
    /// (`capteur::sommeil::signaler_audio_vivant`). À la différence
    /// d'`AudioMort`, elle ne ré-arbitre rien : la preuve ne concerne que la
    /// session qui l'apporte, jamais une AUTRE fenêtre du même groupe de PID.
    AudioVivant,
    /// L'utilisateur a collé dans SA fenêtre : écris ce texte dans le
    /// presse-papier de la VM (sous-bloc P2 du chantier presse-papier).
    ///
    /// 🔴 **Elle SE RÉPOND par `Fait`, et c'est le seul point où cette famille
    /// de commandes le fait — la différence n'est pas stylistique.**
    /// `Visibilite`, `AudioMort` et `AudioVivant` ne se répondent pas parce
    /// que leur effet est un ARBITRAGE global, qui peut concerner une autre
    /// fenêtre et revient par la connexion média. Ici l'appelant a besoin de
    /// savoir que l'écriture a **réellement eu lieu AVANT** d'injecter
    /// `Ctrl+V` : c'est tout l'ordre de D6, et rien d'autre ne le porte. Sur
    /// `DepuisCapteur::Erreur`, l'enfant n'injecte pas — la touche `V` est
    /// **perdue, pas reportée**, parce qu'un `Ctrl+V` sur un presse-papier
    /// inchangé collerait le contenu PRÉCÉDENT, sans que rien ne le dise.
    ///
    /// Le texte est déjà **normalisé, borné et dénormalisé** (`\r\n`) par
    /// l'enfant quand il arrive ici : le propriétaire ne décide rien de son
    /// contenu, il l'écrit. La borne de ce tube (`TAILLE_MAX`, 8 Mio) n'est
    /// donc **pas** le facteur contraignant — `PRESSE_PAPIER_MAX` (64 Kio)
    /// mord cent-vingt-huit fois plus tôt —, et un test le vérifie plutôt que
    /// de le supposer.
    PressePapierEcrire { texte: String },
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
    /// Le presse-papier de la VM a changé. Poussé non sollicité, **au
    /// changement seulement**.
    ///
    /// Distinct d'`Etat` pour la même raison que `Sommeil`, `Part`, `Audio` et
    /// `PleinEcran` : `Etat` alimente un cache lu à chaque tour de la boucle de
    /// transport, et y mêler une annonce ponctuelle passerait par un chemin
    /// conçu pour un état permanent.
    ///
    /// **Le capteur est propriétaire du presse-papier, et lui seul.** La raison
    /// n'est pas l'écriture — P1 n'écrit rien — mais l'ÉCOUTE et le garde
    /// anti-écho : N enfants observant une ressource GLOBALE à la session
    /// Windows auraient N gardes qui ne se voient pas, et l'oscillation serait
    /// **inter-processus, donc irréparable localement**.
    ///
    /// `texte` est `None` sur un refus de taille : le contenu dépassait
    /// `presse_papier::PRESSE_PAPIER_MAX` et il est **refusé, jamais tronqué**.
    /// `octets` porte alors la taille refusée, après normalisation des fins de
    /// ligne.
    ///
    /// ⚠️ **Ce n'est PAS ce canal qui contraint la taille**, et l'écrire ici
    /// évite qu'un successeur croie l'inverse : `TAILLE_MAX` vaut **8 Mio**
    /// (voir en tête de fichier) quand `PRESSE_PAPIER_MAX` vaut **64 Kio** —
    /// deux ordres de grandeur d'écart. La borne est une décision de produit
    /// (D4), pas une limite de transport.
    ///
    /// ✅ **CETTE VARIANTE EST RELIÉE DANS `capteur/pont_media.rs` depuis la
    /// tâche 9 du sous-bloc P1** (`pont_media.rs:87`), avec son test, et la
    /// ROUGE a été jouée avant le bras : sans lui, la toute première annonce
    /// rendait `RecvError` au bout de la file. L'avertissement qui vivait ici
    /// disait « pas encore reliée » et « le contrôle doit rendre UNE ligne » :
    /// les deux sont devenus faux, et les laisser aurait été précisément le
    /// défaut d'énoncé périmé que la revue transverse de ce dépôt traque.
    ///
    /// Ce que le contrôle rend AUJOURD'HUI, relevé par la commande :
    /// `grep -n 'DepuisCapteur::PressePapier' agent/src/capteur/pont_media.rs`
    /// rend **quatre** lignes — une pour le bras, trois pour le test qui le
    /// garde. **Ce qui compte est qu'il ne rende pas ZÉRO** : le bras manquant
    /// ne se signale par aucune erreur de compilation, il fait tomber le
    /// message dans le catch-all `Ok(autre)`, qui **tue le fil `lire_le_media`
    /// sans aucune panne apparente** — la session tombe dans sa fenêtre de
    /// reprise, et rien ne dit pourquoi. Le dépôt a payé ce défaut **quatre
    /// fois** avant celle-ci — `Sommeil` (D5), `Part` (D6), `Audio` (D7),
    /// `PleinEcran` (D8) —, et toute variante NEUVE de cette énumération
    /// poussée sur la connexion média devra refaire le même chemin.
    PressePapier { texte: Option<String>, octets: u32 },
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

// Les tests de ce module vivent à part depuis le sous-bloc P2 du chantier
// presse-papier : le fichier était à 464 lignes pour un plafond de 500, et la
// documentation d'une variante de `VersCapteur` y est copieuse — celle
// d'`AudioMort` fait vingt-sept lignes à elle seule. L'extraction précède
// l'addition de `PressePapierEcrire`, comme la règle du dépôt l'exige.
//
// ⚠️ Cet emploi de `#[path]` est HORS de la portée de la « Convention de module
// enfant » de `CLAUDE.md` : même mécanisme Rust, autre raison — la règle des
// 500 lignes —, exactement comme `superviseur/table.rs`. Pas de hissage.
#[cfg(test)]
#[path = "protocole/tests.rs"]
mod tests;
