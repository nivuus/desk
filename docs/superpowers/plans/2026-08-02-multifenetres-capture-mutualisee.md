# Chantier D, sous-bloc D4 — capture mutualisée : plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Sortir la capture et l'encodage des processus enfants vers un **capteur** unique, pour que le plafond de quatre processus tenant une duplication DXGI cesse de borner le nombre de fenêtres.

**Architecture:** Trois étages — superviseur (hook, pilote SudoVDA, supervision ; ne duplique rien) → capteur (N fils, chacun un `WindowsSource` complet) → N enfants (str0m/WebRTC, entrée, audio). Un tube nommé par fenêtre porte les unités d'accès H.264 **en push** et les commandes du trait `VideoSource` en requête/réponse. Côté enfant, une `SourceDistante` implémente `VideoSource` et remplace `WindowsSource` ; rien d'autre du transport ne bouge.

**Tech Stack:** Rust 2021, `windows-rs` 0.62, `serde` / `serde_json`, `tokio` (déjà présents). **Aucune dépendance neuve.** Tubes nommés Windows via `windows::Win32::System::Pipes`.

**Conception :** `docs/superpowers/specs/2026-08-02-multifenetres-capture-mutualisee-design.md`

---

## Global Constraints

- **Aucun fichier source ne dépasse 500 lignes**, et aucun fichier neuf ne naît au-dessus. Marges à ne pas frôler : `agent/src/encode/arret.rs` = **500 exactement**, `agent/src/capture.rs` = **496**, `agent/src/superviseur/table.rs` = **489**, `agent/src/superviseur/boucle.rs` = **485**.
- **`capture.rs`, `capture/*`, `encode.rs` et `windows_source.rs` ne sont ni déplacés ni modifiés.** Le capteur instancie `WindowsSource::sur_sortie` tel quel. Toute tâche qui croit devoir les toucher a mal compris le plan : s'arrêter et le signaler.
- **La logique pure vit hors de tout `#[cfg(windows)]`** et se teste sur l'hôte Linux, sur le modèle de `superviseur/protocole.rs` et `capture/reprise.rs`. Aucun type `windows-rs` ne franchit cette frontière.
- **Ne jamais tracer par image ni par message** dans le canal. Compter, et journaliser périodiquement. Le projet a déjà perdu une session entière à une trace par paquet (18 619 lignes en quelques secondes sur un partage CIFS).
- **Ne pas utiliser `git add -A`** : l'arbre est partagé entre tâches concurrentes. **Nommer les fichiers** dans chaque `git add`.
- **Avant toute compilation distante** : `cd agent && cargo check --target x86_64-pc-windows-gnu`. Il couvre types, emprunts, visibilités et durées de vie du code `#[cfg(windows)]` ; il **ne couvre pas l'édition de liens**.
- **Compilation sur la VM** : `set -a && source .env && set +a` **d'abord**, puis `scripts/build-agent.sh`. Sans `.env` sourcé, le script s'arrête **en silence** et le symptôme se lit comme une compilation réussie.
- **Toute variable d'environnement neuve doit être ajoutée explicitement à `scripts/run-agent.sh`**, dans la même tâche que le code qui la lit. Piège payé trois fois (`SUPERVISEUR` en D1, `MULTIFENETRE_REPRISE` en D2).
- Tests hôte : `cd agent && cargo test`. Tout doit passer avant chaque commit.

---

## Structure des fichiers

| Fichier | Responsabilité | `cfg` |
| --- | --- | --- |
| `agent/src/capteur.rs` | Racine du module : déclarations et point d'entrée `executer`. Mince, comme `superviseur.rs` | non gaté |
| `agent/src/capteur/protocole.rs` | Messages et cadrage du canal. Sérialisation pure | **non gaté** |
| `agent/src/capteur/distante.rs` | `SourceDistante` : implémente `VideoSource` sur un canal injecté | **non gaté** |
| `agent/src/capteur/reprise.rs` | Fenêtre de reprise du canal (rupture du tube) | **non gaté** |
| `agent/src/capteur/serveur.rs` | Serveur de tube nommé : accepte, lit l'attache, ouvre un fil | `#[cfg(windows)]` |
| `agent/src/capteur/fenetre.rs` | Le fil d'une fenêtre : tient un `WindowsSource`, pousse, sert les commandes | `#[cfg(windows)]` |
| `agent/src/capteur/tube.rs` | Le client de tube côté enfant : connexion, et l'impl de `Commandes` | `#[cfg(windows)]` |
| `agent/src/capteur/horloge.rs` | `QueryPerformanceCounter` : lecture et rebasage d'origine | `#[cfg(windows)]` |
| `agent/src/demarrage/source.rs` | *Modifié* : arbitre source locale / distante | `#[cfg(windows)]` |
| `agent/src/superviseur/lanceur.rs` | *Modifié* : lance le capteur, le rattache au job | `#[cfg(windows)]` |
| `agent/src/superviseur/boucle.rs` | *Modifié* : surveille et relance le capteur | `#[cfg(windows)]` |
| `agent/src/main.rs` | *Modifié* : déclare `capteur`, aiguille le mode | mixte |

---

### Task 1 : Le protocole du canal

**Files:**
- Create: `agent/src/capteur.rs`
- Create: `agent/src/capteur/protocole.rs`
- Modify: `agent/src/main.rs` (déclaration du module)
- Test: dans `agent/src/capteur/protocole.rs`, module `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `crate::h264::AccessUnit` (champs `data: Vec<u8>`, `is_keyframe: bool`, `pts_90k: u64`).
- Produces: `VersCapteur`, `DepuisCapteur`, `Trame`, `NOM_TUBE`, `TAILLE_MAX`, `ecrire_json`, `ecrire_image`, `lire_trame`.

- [ ] **Step 1 : Écrire les tests d'abord**

Créer `agent/src/capteur/protocole.rs` avec **uniquement** le module de tests ci-dessous en tête de fichier ; le corps viendra à l'étape 3.

```rust
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
```

- [ ] **Step 2 : Lancer les tests pour vérifier qu'ils échouent**

Run: `cd agent && cargo test capteur::protocole`
Expected: **échec de compilation** — `VersCapteur`, `ecrire_json`, `lire_trame`… n'existent pas.

- [ ] **Step 3 : Écrire l'implémentation**

En tête de `agent/src/capteur/protocole.rs`, **avant** le module de tests :

```rust
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
        /// `QueryPerformanceCounter` lu par l'enfant au moment même où il crée
        /// son `clock_origin`. Un `Instant` n'a aucun sens dans un autre
        /// processus ; QPC, lui, est commun à toute la machine. Sans ce
        /// rebasage, la vidéo de l'enfant porteur du son serait décalée de
        /// l'écart entre les deux origines.
        origine_qpc: i64,
    },
    Redimensionner { largeur: u32, hauteur: u32 },
    TailleEncodage { largeur: u32, hauteur: u32 },
    Debit { bps: u32 },
    ImageCle,
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
```

Créer `agent/src/capteur.rs` :

```rust
//! Le capteur : un seul processus qui tient les N duplications DXGI et les N
//! encodeurs, et distribue le média aux enfants par tube nommé.
//!
//! Ce fichier reste mince à dessein — il assemble, il ne décide pas. Même
//! découpage que `superviseur.rs` : la logique pure (protocole, source
//! distante, reprise) est hors `cfg` et se teste sur l'hôte ; ce qui touche
//! DXGI et les tubes est gaté.

pub mod protocole;
```

Dans `agent/src/main.rs`, ajouter la déclaration **à côté de celle de `superviseur`**, avec son commentaire de motif :

```rust
// Pas de `#[cfg(windows)]` ici : le protocole du canal média et la
// `SourceDistante` sont de la logique pure, et doivent se compiler et se
// tester sur Linux. Les sous-modules qui touchent DXGI et les tubes sont
// gatés à l'intérieur de `capteur.rs`.
mod capteur;
```

- [ ] **Step 4 : Lancer les tests pour vérifier qu'ils passent**

Run: `cd agent && cargo test capteur::protocole`
Expected: **8 tests passent**.

- [ ] **Step 5 : Vérifier la compilation croisée Windows**

Run: `cd agent && cargo check --target x86_64-pc-windows-gnu`
Expected: sortie 0.

- [ ] **Step 6 : Commit**

```bash
git add agent/src/capteur.rs agent/src/capteur/protocole.rs agent/src/main.rs
git commit -m "feat(d4): le protocole du canal media, cadre et teste sur l'hote"
```

---

### Task 2 : `SourceDistante` implémente `VideoSource`

**Files:**
- Create: `agent/src/capteur/distante.rs`
- Modify: `agent/src/capteur.rs` (déclaration)
- Test: dans `agent/src/capteur/distante.rs`

**Interfaces:**
- Consumes: `capteur::protocole::{VersCapteur, DepuisCapteur}` ; `crate::source::VideoSource` ; `crate::h264::AccessUnit`.
- Produces: `trait Commandes { fn commander(&mut self, message: VersCapteur) -> anyhow::Result<DepuisCapteur>; }` ; `enum Recu { Image(AccessUnit), Etat { vivante: bool, epuisee: bool, largeur: u32, hauteur: u32 } }` ; `struct SourceDistante` avec `SourceDistante::nouvelle(commandes: Box<dyn Commandes + Send>, images: std::sync::mpsc::Receiver<Recu>, largeur: u32, hauteur: u32) -> Self`.

**Pourquoi un trait injecté** : le tube réel est `#[cfg(windows)]`. Sans cette injection, `SourceDistante` ne serait testable que sur la VM — or c'est la pièce qui décide de clore ou non une session, donc la plus coûteuse à se tromper.

- [ ] **Step 1 : Écrire les tests d'abord**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::sync_channel;

    /// Canal factice : rend des réponses préparées et retient ce qui a été
    /// demandé, pour que les tests vérifient le message ÉMIS et pas seulement
    /// l'effet.
    struct CanalFactice {
        reponses: Vec<anyhow::Result<DepuisCapteur>>,
        recus: std::sync::Arc<std::sync::Mutex<Vec<VersCapteur>>>,
    }

    impl Commandes for CanalFactice {
        fn commander(&mut self, message: VersCapteur) -> anyhow::Result<DepuisCapteur> {
            self.recus.lock().unwrap().push(message);
            if self.reponses.is_empty() {
                Ok(DepuisCapteur::Fait)
            } else {
                self.reponses.remove(0)
            }
        }
    }

    fn source_avec(
        capacite: usize,
    ) -> (
        SourceDistante,
        std::sync::mpsc::SyncSender<Recu>,
        std::sync::Arc<std::sync::Mutex<Vec<VersCapteur>>>,
    ) {
        let (tx, rx) = sync_channel(capacite);
        let recus = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let canal = CanalFactice { reponses: Vec::new(), recus: recus.clone() };
        (SourceDistante::nouvelle(Box::new(canal), rx, 1280, 720), tx, recus)
    }

    #[test]
    fn une_image_poussee_est_rendue_par_next_frame() {
        let (mut source, tx, _) = source_avec(4);
        tx.send(Recu::Image(AccessUnit { data: vec![1, 2], is_keyframe: true, pts_90k: 42 }))
            .unwrap();
        let unite = source.next_frame().expect("une image était en file");
        assert_eq!(unite.pts_90k, 42);
        assert!(unite.is_keyframe);
    }

    /// Le cas COURANT : rien de neuf. Il doit être gratuit et ne surtout pas
    /// passer pour un épuisement — la boucle de transport interroge à 100 Hz.
    #[test]
    fn une_file_vide_rend_none_sans_epuiser_la_source() {
        let (mut source, _tx, _) = source_avec(4);
        assert!(source.next_frame().is_none());
        assert!(!source.is_exhausted());
        assert!(source.is_alive());
    }

    #[test]
    fn les_images_sortent_dans_l_ordre_d_arrivee() {
        let (mut source, tx, _) = source_avec(4);
        for pts in [1, 2, 3] {
            tx.send(Recu::Image(AccessUnit { data: vec![], is_keyframe: false, pts_90k: pts }))
                .unwrap();
        }
        let rendus: Vec<u64> =
            (0..3).map(|_| source.next_frame().unwrap().pts_90k).collect();
        assert_eq!(rendus, vec![1, 2, 3]);
    }

    /// `Etat` n'est pas une image : il met à jour le cache et la lecture
    /// continue, sans consommer le tour.
    #[test]
    fn un_etat_intercale_met_a_jour_le_cache_sans_masquer_l_image_suivante() {
        let (mut source, tx, _) = source_avec(4);
        tx.send(Recu::Etat { vivante: true, epuisee: false, largeur: 800, hauteur: 600 })
            .unwrap();
        tx.send(Recu::Image(AccessUnit { data: vec![], is_keyframe: false, pts_90k: 5 }))
            .unwrap();
        assert_eq!(source.next_frame().unwrap().pts_90k, 5);
        assert_eq!(source.dimensions(), (800, 600));
    }

    #[test]
    fn une_fenetre_disparue_rend_la_source_non_vivante_et_epuisee() {
        let (mut source, tx, _) = source_avec(4);
        tx.send(Recu::Etat { vivante: false, epuisee: true, largeur: 1280, hauteur: 720 })
            .unwrap();
        assert!(source.next_frame().is_none());
        assert!(!source.is_alive());
        assert!(source.is_exhausted());
    }

    #[test]
    fn les_commandes_partent_sous_la_forme_attendue() {
        let (mut source, _tx, recus) = source_avec(4);
        source.set_bitrate(3_000_000).unwrap();
        source.set_encode_size(640, 360).unwrap();
        source.request_keyframe().unwrap();
        let recus = recus.lock().unwrap();
        assert_eq!(
            *recus,
            vec![
                VersCapteur::Debit { bps: 3_000_000 },
                VersCapteur::TailleEncodage { largeur: 640, hauteur: 360 },
                VersCapteur::ImageCle,
            ]
        );
    }

    /// `resize` doit retenir la taille RÉELLEMENT obtenue, pas celle demandée
    /// — même règle qu'en mono-fenêtre (`transport/redimensionnement.rs`).
    ///
    /// ⚠️ **Les dimensions initiales (640×480) DOIVENT différer de celles que
    /// le capteur factice répond.** Une première rédaction de ce test
    /// construisait la source en 1280×720, soit exactement la réponse : elle
    /// passait aussi bien avec un `resize` correct qu'avec un `resize`
    /// devenu no-op, et ne gardait donc pas la règle qu'elle existe pour
    /// garder — celle-là même sur laquelle D1 s'était trompé.
    #[test]
    fn un_redimensionnement_retient_la_taille_obtenue() {
        let (tx_img, rx) = sync_channel(4);
        let recus = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let canal = CanalFactice {
            reponses: vec![Ok(DepuisCapteur::Taille { largeur: 1280, hauteur: 720 })],
            recus: recus.clone(),
        };
        let mut source = SourceDistante::nouvelle(Box::new(canal), rx, 640, 480);
        drop(tx_img);
        source.resize(1281, 713).unwrap();
        assert_eq!(source.dimensions(), (1280, 720));
    }

    #[test]
    fn une_erreur_du_capteur_remonte_en_erreur() {
        let (_tx, rx) = sync_channel(4);
        let canal = CanalFactice {
            reponses: vec![Ok(DepuisCapteur::Erreur { motif: "encodeur perdu".into() })],
            recus: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        };
        let mut source = SourceDistante::nouvelle(Box::new(canal), rx, 1280, 720);
        let erreur = source.set_bitrate(1).unwrap_err().to_string();
        assert!(erreur.contains("encodeur perdu"), "message inattendu : {erreur}");
    }
}
```

- [ ] **Step 2 : Lancer les tests pour vérifier qu'ils échouent**

Run: `cd agent && cargo test capteur::distante`
Expected: échec de compilation — `SourceDistante` n'existe pas.

- [ ] **Step 3 : Écrire l'implémentation**

```rust
//! `SourceDistante` — la source vidéo d'un enfant, alimentée par le capteur.
//!
//! **Pas de `#[cfg(windows)]`** : le tube réel est gaté (`capteur/tube.rs`),
//! mais la décision de clore ou non une session vit ici, et c'est la pièce la
//! plus coûteuse à se tromper. Elle est donc écrite contre un `Commandes`
//! injecté et un `Receiver`, tous deux triviaux à simuler sur l'hôte.

use std::sync::mpsc::{Receiver, TryRecvError};

use anyhow::{bail, Result};

use crate::capteur::protocole::{DepuisCapteur, VersCapteur};
use crate::h264::AccessUnit;
use crate::source::VideoSource;

/// Ce que l'enfant peut demander au capteur, en requête/réponse.
pub trait Commandes {
    fn commander(&mut self, message: VersCapteur) -> Result<DepuisCapteur>;
}

/// Ce que le capteur pousse, non sollicité.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recu {
    Image(AccessUnit),
    Etat { vivante: bool, epuisee: bool, largeur: u32, hauteur: u32 },
}

pub struct SourceDistante {
    commandes: Box<dyn Commandes + Send>,
    images: Receiver<Recu>,
    largeur: u32,
    hauteur: u32,
    vivante: bool,
    epuisee: bool,
}

impl SourceDistante {
    pub fn nouvelle(
        commandes: Box<dyn Commandes + Send>,
        images: Receiver<Recu>,
        largeur: u32,
        hauteur: u32,
    ) -> Self {
        Self { commandes, images, largeur, hauteur, vivante: true, epuisee: false }
    }

    /// Émet une commande et n'accepte que `Fait` comme succès.
    fn commander_simple(&mut self, message: VersCapteur) -> Result<()> {
        match self.commandes.commander(message)? {
            DepuisCapteur::Fait => Ok(()),
            DepuisCapteur::Erreur { motif } => bail!("le capteur a refusé : {motif}"),
            autre => bail!("réponse inattendue du capteur : {autre:?}"),
        }
    }
}

impl VideoSource for SourceDistante {
    fn next_frame(&mut self) -> Option<AccessUnit> {
        loop {
            match self.images.try_recv() {
                Ok(Recu::Image(unite)) => return Some(unite),
                Ok(Recu::Etat { vivante, epuisee, largeur, hauteur }) => {
                    self.vivante = vivante;
                    self.epuisee = epuisee;
                    self.largeur = largeur;
                    self.hauteur = hauteur;
                }
                // Le cas COURANT et normal : rien de neuf ce tour-ci. La
                // boucle de transport interroge à 100 Hz une source qui
                // produit à ~90 i/s.
                Err(TryRecvError::Empty) => return None,
                // Le canal est rompu. Ce n'est PAS traité ici comme un
                // épuisement : la tâche 3 y branche la fenêtre de reprise.
                Err(TryRecvError::Disconnected) => return None,
            }
        }
    }

    fn dimensions(&self) -> (u32, u32) {
        (self.largeur, self.hauteur)
    }

    fn is_exhausted(&self) -> bool {
        self.epuisee
    }

    fn is_alive(&self) -> bool {
        self.vivante
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        match self
            .commandes
            .commander(VersCapteur::Redimensionner { largeur: width, hauteur: height })?
        {
            // La taille RETENUE est celle obtenue, jamais celle demandée : le
            // pilote quantifie, et une fenêtre Windows impose des dimensions
            // paires. Même règle qu'en mono-fenêtre.
            DepuisCapteur::Taille { largeur, hauteur } => {
                self.largeur = largeur;
                self.hauteur = hauteur;
                Ok(())
            }
            DepuisCapteur::Erreur { motif } => bail!("le capteur a refusé : {motif}"),
            autre => bail!("réponse inattendue du capteur : {autre:?}"),
        }
    }

    fn set_bitrate(&mut self, bitrate: u32) -> Result<()> {
        self.commander_simple(VersCapteur::Debit { bps: bitrate })
    }

    fn set_encode_size(&mut self, width: u32, height: u32) -> Result<()> {
        self.commander_simple(VersCapteur::TailleEncodage { largeur: width, hauteur: height })
    }

    fn request_keyframe(&mut self) -> Result<()> {
        self.commander_simple(VersCapteur::ImageCle)
    }
}
```

Dans `agent/src/capteur.rs`, ajouter `pub mod distante;` après `pub mod protocole;`.

- [ ] **Step 4 : Lancer les tests pour vérifier qu'ils passent**

Run: `cd agent && cargo test capteur::`
Expected: les 8 tests du protocole **et** les 8 de `distante` passent.

- [ ] **Step 5 : Commit**

```bash
git add agent/src/capteur/distante.rs agent/src/capteur.rs
git commit -m "feat(d4): SourceDistante implemente VideoSource sur un canal injecte"
```

---

### Task 3 : La fenêtre de reprise du canal

**Files:**
- Create: `agent/src/capteur/reprise.rs`
- Modify: `agent/src/capteur/distante.rs` (branchement)
- Modify: `agent/src/capteur.rs` (déclaration)
- Test: dans les deux fichiers

**Interfaces:**
- Produces: `DUREE_FENETRE_CANAL: Duration` ; `struct FenetreCanal` avec `nouvelle() -> Self`, `rupture(&mut self, maintenant: Instant) -> bool` (vrai si la fenêtre est **expirée**, donc l'épuisement acquis), `succes(&mut self)`.

**Pourquoi cette tâche existe** : c'est le point exact où le critère 2 se gagne ou se perd. Si `is_exhausted` passe à vrai sur une simple rupture de tube, `brancher_video` appelle `begin_ending("source vidéo épuisée")` (`transport/piste_video.rs:130`) et l'on perd précisément les sessions que la relance du capteur devait sauver.

- [ ] **Step 1 : Écrire les tests d'abord**

Dans `agent/src/capteur/reprise.rs` :

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn une_rupture_n_epuise_pas_immediatement() {
        let mut fenetre = FenetreCanal::nouvelle();
        let t0 = Instant::now();
        assert!(!fenetre.rupture(t0), "la première rupture ouvre la fenêtre, elle ne conclut pas");
        assert!(!fenetre.rupture(t0 + Duration::from_secs(1)));
    }

    #[test]
    fn une_rupture_ininterrompue_au_dela_de_la_fenetre_epuise() {
        let mut fenetre = FenetreCanal::nouvelle();
        let t0 = Instant::now();
        assert!(!fenetre.rupture(t0));
        assert!(fenetre.rupture(t0 + DUREE_FENETRE_CANAL + Duration::from_millis(1)));
    }

    /// Le raccrochage referme la fenêtre : une SECONDE relance du capteur,
    /// plus tard, doit retrouver son budget entier.
    #[test]
    fn un_succes_referme_la_fenetre_et_rend_le_budget_entier() {
        let mut fenetre = FenetreCanal::nouvelle();
        let t0 = Instant::now();
        assert!(!fenetre.rupture(t0));
        fenetre.succes();
        let t1 = t0 + DUREE_FENETRE_CANAL * 3;
        assert!(!fenetre.rupture(t1), "la fenêtre doit repartir de zéro");
        assert!(!fenetre.rupture(t1 + DUREE_FENETRE_CANAL / 2));
        assert!(fenetre.rupture(t1 + DUREE_FENETRE_CANAL + Duration::from_millis(1)));
    }
}
```

Dans `agent/src/capteur/distante.rs`, **ajouter** au module de tests existant :

```rust
    /// Le cœur du critère 2 : tuer le capteur ferme le tube, donc rompt le
    /// canal — et cela ne doit PAS clore la session, sans quoi
    /// `brancher_video` appelle `begin_ending("source vidéo épuisée")`.
    #[test]
    fn un_canal_rompu_n_epuise_pas_la_source_dans_la_fenetre() {
        let (mut source, tx, _) = source_avec(4);
        drop(tx);
        assert!(source.next_frame().is_none());
        assert!(!source.is_exhausted(), "une rupture de canal n'est pas un épuisement");
    }

    /// Mais une rupture qui dure l'est : sans cela, une session morte
    /// resterait ouverte indéfiniment sur une image figée.
    #[test]
    fn un_canal_rompu_au_dela_de_la_fenetre_epuise_la_source() {
        let (mut source, tx, _) = source_avec(4);
        drop(tx);
        assert!(source.next_frame().is_none());
        source.vieillir_pour_test(DUREE_FENETRE_CANAL + std::time::Duration::from_millis(1));
        assert!(source.next_frame().is_none());
        assert!(source.is_exhausted());
    }
```

- [ ] **Step 2 : Lancer les tests pour vérifier qu'ils échouent**

Run: `cd agent && cargo test capteur::`
Expected: échec de compilation — `FenetreCanal` et `vieillir_pour_test` n'existent pas.

- [ ] **Step 3 : Écrire l'implémentation**

`agent/src/capteur/reprise.rs` :

```rust
//! Borner la reprise après une rupture du canal média.
//!
//! **Pur à dessein**, sur le modèle exact de `capture/reprise.rs` : c'est la
//! pièce qui décide si une session meurt, et elle doit se tester sur l'hôte.

use std::time::{Duration, Instant};

/// Durée pendant laquelle une rupture du canal est tolérée avant d'être
/// déclarée définitive.
///
/// **Majorante et non calibrée, et il faut le dire.** Elle doit couvrir la
/// détection de la mort du capteur par le superviseur (un tour de boucle), le
/// relancement du processus, l'ouverture de son serveur de tube, et la
/// reconnexion de l'enfant. Aucun de ces quatre délais n'est mesuré à ce jour ;
/// le critère 2 de la recette en donnera un premier ordre de grandeur.
///
/// Ce qui borne le coût d'une valeur trop grande : pendant la fenêtre, la
/// session reste ouverte sur une image figée. Trop petite, elle tue les
/// sessions que la relance devait sauver — le risque est franchement
/// asymétrique, d'où le choix d'une valeur large.
pub const DUREE_FENETRE_CANAL: Duration = Duration::from_secs(15);

/// Fenêtre ouverte à la première rupture et **refermée par le premier
/// succès**. La durée court donc depuis la DERNIÈRE rupture constatée après un
/// succès, jamais depuis la première de la session.
#[derive(Debug, Default)]
pub struct FenetreCanal {
    ouverte_depuis: Option<Instant>,
}

impl FenetreCanal {
    pub fn nouvelle() -> Self {
        Self::default()
    }

    /// À appeler à chaque constat de rupture. Rend **vrai** quand la fenêtre
    /// est expirée, c'est-à-dire quand l'épuisement est acquis.
    pub fn rupture(&mut self, maintenant: Instant) -> bool {
        match self.ouverte_depuis {
            None => {
                self.ouverte_depuis = Some(maintenant);
                false
            }
            Some(debut) => maintenant.duration_since(debut) > DUREE_FENETRE_CANAL,
        }
    }

    /// À appeler dès qu'une lecture aboutit : la fenêtre se referme et le
    /// budget repart entier pour une rupture ultérieure.
    pub fn succes(&mut self) {
        self.ouverte_depuis = None;
    }
}
```

Dans `agent/src/capteur/distante.rs` : ajouter le champ, l'import, et brancher les deux branches de `try_recv`.

```rust
use crate::capteur::reprise::FenetreCanal;
use std::time::Instant;
```

Champ à ajouter à `SourceDistante` :

```rust
    /// Rupture du canal en cours. Une rupture n'épuise pas la source tant que
    /// cette fenêtre n'a pas expiré : c'est ce qui fait survivre les sessions
    /// à une relance du capteur.
    fenetre: FenetreCanal,
```

Initialisation dans `nouvelle` : `fenetre: FenetreCanal::nouvelle(),`.

Remplacer les deux bras terminaux de `next_frame` :

```rust
                Ok(Recu::Image(unite)) => {
                    self.fenetre.succes();
                    return Some(unite);
                }
                // …
                Err(TryRecvError::Empty) => {
                    self.fenetre.succes();
                    return None;
                }
                Err(TryRecvError::Disconnected) => {
                    if self.fenetre.rupture(Instant::now()) {
                        self.epuisee = true;
                    }
                    return None;
                }
```

> `Empty` referme la fenêtre : un canal vivant mais silencieux est le cas
> nominal (bureau immobile), et il ne doit jamais consommer le budget de
> reprise. Seul `Disconnected` le consomme.

Ajouter enfin la trappe de test, **gatée** :

```rust
    /// Fait vieillir la fenêtre de reprise, pour les seuls tests : sans elle,
    /// éprouver l'expiration exigerait d'attendre réellement 15 secondes.
    #[cfg(test)]
    pub fn vieillir_pour_test(&mut self, ecart: std::time::Duration) {
        self.fenetre.vieillir_pour_test(ecart);
    }
```

et dans `FenetreCanal` :

```rust
    #[cfg(test)]
    pub fn vieillir_pour_test(&mut self, ecart: Duration) {
        if let Some(debut) = self.ouverte_depuis {
            self.ouverte_depuis = Some(debut - ecart);
        }
    }
```

Dans `agent/src/capteur.rs`, ajouter `pub mod reprise;`.

- [ ] **Step 4 : Lancer les tests pour vérifier qu'ils passent**

Run: `cd agent && cargo test capteur::`
Expected: 3 tests de `reprise` + 10 de `distante` + 8 de `protocole` passent.

- [ ] **Step 5 : Vérifier la compilation croisée Windows**

Run: `cd agent && cargo check --target x86_64-pc-windows-gnu`
Expected: sortie 0.

- [ ] **Step 6 : Commit**

```bash
git add agent/src/capteur/reprise.rs agent/src/capteur/distante.rs agent/src/capteur.rs
git commit -m "feat(d4): une rupture du canal ne tue pas la session avant expiration"
```

---

### Task 3bis : Rattachement du canal après une rupture

**Files:**
- Modify: `agent/src/capteur/distante.rs`
- Modify: `agent/src/capteur/reprise.rs`
- Test: dans les deux fichiers

**Interfaces:**
- Produces: `trait Canal` (**renommage** de `Commandes`) avec `fn commander(&mut self, message: VersCapteur) -> Result<DepuisCapteur>` **et** `fn rattacher(&mut self) -> Result<Rattachee>` ; `struct Rattachee { pub images: Receiver<Recu>, pub largeur: u32, pub hauteur: u32 }` ; `FenetreCanal::peut_reessayer(&mut self, maintenant: Instant) -> bool` ; `PAS_RATTACHEMENT: Duration`.

**Pourquoi cette tâche existe — un trou de conception, pas une extension.** La
tâche 3 a livré une fenêtre qui **retarde** l'épuisement de 15 s. Elle ne
**rattache rien** : `connecter()` (tâche 6) n'est appelée qu'une fois, au
démarrage de l'enfant ; à la mort du capteur le fil lecteur sort, le `Receiver`
se déconnecte définitivement, et rien n'ouvre jamais de tube vers le capteur
relancé. Les sessions survivraient 15 s puis mourraient toutes — **le critère de
réception n°2 échouerait**, et le recul d'isolation assumé au §3.6 de la
conception ne serait compensé par rien.

**Pourquoi le rattachement est injecté et non écrit dans le tube.** La décision
de rattacher, d'attendre ou d'abandonner est exactement la logique qui décide
si une session WebRTC meurt. Elle doit rester hors `#[cfg(windows)]` et
testable sur l'hôte, comme le reste de `SourceDistante`. La tâche 6 fournira
l'implémentation réelle du trait ; celle-ci n'écrit que la politique.

- [ ] **Step 1 : Écrire les tests d'abord**

Dans `agent/src/capteur/reprise.rs`, **ajouter** au module de tests existant :

```rust
    /// Le pas d'espacement existe parce que `next_frame` est appelée ~100
    /// fois par seconde : sans lui, une rupture déclencherait 100 tentatives
    /// de reconnexion par seconde et par fenêtre.
    #[test]
    fn les_essais_de_rattachement_sont_espaces() {
        let mut fenetre = FenetreCanal::nouvelle();
        let t0 = Instant::now();
        assert!(!fenetre.rupture(t0));
        assert!(fenetre.peut_reessayer(t0), "le premier essai est immédiat");
        assert!(!fenetre.peut_reessayer(t0), "deux essais dans le même instant");
        assert!(!fenetre.peut_reessayer(t0 + PAS_RATTACHEMENT / 2));
        assert!(fenetre.peut_reessayer(t0 + PAS_RATTACHEMENT + Duration::from_millis(1)));
    }

    /// Un succès doit rendre le budget d'essais entier, pas seulement celui
    /// d'expiration : une seconde rupture, plus tard, doit pouvoir réessayer
    /// tout de suite.
    #[test]
    fn un_succes_rend_aussi_le_droit_de_reessayer_immediatement() {
        let mut fenetre = FenetreCanal::nouvelle();
        let t0 = Instant::now();
        assert!(!fenetre.rupture(t0));
        assert!(fenetre.peut_reessayer(t0));
        fenetre.succes();
        let t1 = t0 + Duration::from_millis(1);
        assert!(!fenetre.rupture(t1));
        assert!(fenetre.peut_reessayer(t1), "après un succès, le premier essai est immédiat");
    }
```

Dans `agent/src/capteur/distante.rs`, **ajouter** au module de tests existant.
Le canal factice gagne d'abord de quoi simuler un rattachement :

```rust
    /// Ce que le canal factice rendra au prochain `rattacher`. `None` = échec.
    /// Une file, pour que les tests enchaînent échecs puis succès.
    type ProchainsRattachements = std::sync::Arc<std::sync::Mutex<Vec<Option<u32>>>>;
```

et `CanalFactice` gagne deux champs — `rattachements: ProchainsRattachements`
et `essais: std::sync::Arc<std::sync::Mutex<u32>>` — plus cette implémentation
(le `u32` rendu est la largeur, pour que le test distingue la file neuve de
l'ancienne) :

```rust
        fn rattacher(&mut self) -> anyhow::Result<Rattachee> {
            *self.essais.lock().unwrap() += 1;
            let prochain = {
                let mut file = self.rattachements.lock().unwrap();
                if file.is_empty() { None } else { file.remove(0) }
            };
            match prochain {
                Some(largeur) => {
                    let (tx, rx) = sync_channel(4);
                    // Une image dans la file neuve : c'est elle qui prouvera
                    // que la source lit bien le NOUVEAU canal.
                    tx.send(Recu::Image(AccessUnit {
                        data: vec![7],
                        is_keyframe: true,
                        pts_90k: 700,
                    }))
                    .unwrap();
                    Ok(Rattachee { images: rx, largeur, hauteur: 480 })
                }
                None => anyhow::bail!("aucun capteur"),
            }
        }
```

Les tests :

```rust
    /// Le cœur du critère 2 : le capteur meurt, il est relancé, et la session
    /// reprend — même file neuve, mêmes dimensions annoncées par le capteur.
    #[test]
    fn un_rattachement_reussi_fait_revivre_la_source_et_reprend_ses_dimensions() {
        let (mut source, tx, _, rattachements, essais) = source_rattachable(vec![Some(1600)]);
        drop(tx);
        // Premier tour : rupture constatée, rattachement tenté et réussi.
        assert!(source.next_frame().is_none(), "le tour de la rupture ne rend pas d'image");
        assert_eq!(*essais.lock().unwrap(), 1);
        assert!(rattachements.lock().unwrap().is_empty());
        // Tour suivant : l'image vient de la file NEUVE.
        let unite = source.next_frame().expect("la file neuve porte une image");
        assert_eq!(unite.pts_90k, 700);
        assert_eq!(source.dimensions(), (1600, 480), "les dimensions du capteur relancé");
        assert!(!source.is_exhausted());
        assert!(source.is_alive());
    }

    /// Un rattachement qui échoue ne conclut rien : la fenêtre court encore.
    #[test]
    fn un_rattachement_qui_echoue_laisse_la_source_en_attente_sans_l_epuiser() {
        let (mut source, tx, _, _, essais) = source_rattachable(vec![None]);
        drop(tx);
        assert!(source.next_frame().is_none());
        assert_eq!(*essais.lock().unwrap(), 1);
        assert!(!source.is_exhausted(), "un échec de rattachement n'épuise pas");
    }

    /// Mais un échec qui dure au-delà de la fenêtre, si : sans cela une
    /// session morte resterait ouverte indéfiniment sur une image figée.
    #[test]
    fn un_rattachement_qui_echoue_jusqu_a_expiration_epuise_la_source() {
        let (mut source, tx, _, _, _) = source_rattachable(vec![None]);
        drop(tx);
        assert!(source.next_frame().is_none());
        source.vieillir_pour_test(DUREE_FENETRE_CANAL + std::time::Duration::from_millis(1));
        assert!(source.next_frame().is_none());
        assert!(source.is_exhausted());
    }

    /// Sans espacement, une rupture provoquerait ~100 tentatives par seconde.
    #[test]
    fn une_rafale_d_interrogations_ne_produit_qu_un_seul_essai() {
        let (mut source, tx, _, _, essais) = source_rattachable(vec![None, None, None, None]);
        drop(tx);
        for _ in 0..10 {
            assert!(source.next_frame().is_none());
        }
        assert_eq!(*essais.lock().unwrap(), 1, "un seul essai dans la rafale");
    }
```

avec l'aide de construction, à placer près de `source_avec` :

```rust
    #[allow(clippy::type_complexity)]
    fn source_rattachable(
        rattachements: Vec<Option<u32>>,
    ) -> (
        SourceDistante,
        std::sync::mpsc::SyncSender<Recu>,
        std::sync::Arc<std::sync::Mutex<Vec<VersCapteur>>>,
        ProchainsRattachements,
        std::sync::Arc<std::sync::Mutex<u32>>,
    ) {
        let (tx, rx) = sync_channel(4);
        let recus = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let file = std::sync::Arc::new(std::sync::Mutex::new(rattachements));
        let essais = std::sync::Arc::new(std::sync::Mutex::new(0));
        let canal = CanalFactice {
            reponses: Vec::new(),
            recus: recus.clone(),
            rattachements: file.clone(),
            essais: essais.clone(),
        };
        (
            SourceDistante::nouvelle(Box::new(canal), rx, 1280, 720),
            tx,
            recus,
            file,
            essais,
        )
    }
```

> Les constructions existantes de `CanalFactice` (dans `source_avec` et dans
> les deux tests qui l'instancient à la main) gagnent les deux champs neufs.
> **Ne pas dupliquer le corps de `source_avec`** : le faire déléguer à
> `source_rattachable(Vec::new())` est acceptable et préférable.

- [ ] **Step 2 : Lancer les tests pour vérifier qu'ils échouent**

Run: `cd agent && cargo test capteur::`
Expected: échec de compilation — `rattacher`, `Rattachee`, `peut_reessayer`
et `PAS_RATTACHEMENT` n'existent pas.

- [ ] **Step 3 : Écrire l'implémentation**

Dans `agent/src/capteur/reprise.rs` :

```rust
/// Intervalle minimal entre deux tentatives de rattachement.
///
/// `next_frame` est appelée ~100 fois par seconde (`FRAME_INTERVAL` vaut
/// 10 ms) : sans ce pas, une rupture provoquerait une centaine de tentatives
/// d'ouverture de tube par seconde et par fenêtre. 250 ms laissent au
/// superviseur le temps de relancer le capteur sans que la reprise traîne —
/// au pire 250 ms de retard sur un rattachement possible, contre 15 s de
/// budget total.
pub const PAS_RATTACHEMENT: Duration = Duration::from_millis(250);
```

`FenetreCanal` gagne un champ et une méthode :

```rust
    /// Dernier essai de rattachement. `None` = aucun depuis le dernier
    /// succès, donc le prochain est immédiat.
    dernier_essai: Option<Instant>,
```

```rust
    /// Vrai si un essai de rattachement est dû. À n'appeler qu'après une
    /// `rupture` non expirée.
    pub fn peut_reessayer(&mut self, maintenant: Instant) -> bool {
        let du = match self.dernier_essai {
            None => true,
            Some(precedent) => maintenant.duration_since(precedent) > PAS_RATTACHEMENT,
        };
        if du {
            self.dernier_essai = Some(maintenant);
        }
        du
    }
```

et `succes()` remet **les deux** champs à zéro :

```rust
    pub fn succes(&mut self) {
        self.ouverte_depuis = None;
        self.dernier_essai = None;
    }
```

> ⚠️ `vieillir_pour_test` doit faire vieillir `dernier_essai` **aussi**.
>
> **Correction d'une note fausse de la première rédaction**, relevée à la revue :
> cette exigence n'est **pas** éprouvée par le test d'expiration, contrairement à
> ce qui était écrit ici. Dans le bras `Disconnected`, `rupture` est évaluée
> **avant** `peut_reessayer` et sort par un `return None` anticipé : après un
> vieillissement au-delà de `DUREE_FENETRE_CANAL`, `peut_reessayer` n'est jamais
> atteinte, et le test passerait à l'identique si le vieillissement de
> `dernier_essai` était retiré. Le seul test qui l'éprouve est
> `un_vieillissement_du_pas_seul_relance_un_essai`, qui vieillit **dans** la
> fenêtre — au-delà de `PAS_RATTACHEMENT` seulement — et attend un second essai.

Dans `agent/src/capteur/distante.rs` : `Commandes` est **renommé `Canal`** et
gagne `rattacher`. Le nom `Commandes` deviendrait faux — un trait qui rouvre
une connexion ne porte pas que des commandes.

```rust
/// Ce que l'enfant peut demander au capteur, et le moyen de s'y rattacher
/// quand le canal se rompt.
pub trait Canal {
    fn commander(&mut self, message: VersCapteur) -> Result<DepuisCapteur>;
    /// Rouvre un canal vers le capteur et s'y réattache. L'implémentation
    /// remplace son propre état interne d'écriture ; elle rend la file
    /// d'images neuve et les dimensions annoncées à l'attache.
    fn rattacher(&mut self) -> Result<Rattachee>;
}

/// Le fruit d'un rattachement réussi.
pub struct Rattachee {
    pub images: Receiver<Recu>,
    pub largeur: u32,
    pub hauteur: u32,
}
```

Le bras `Disconnected` de `next_frame` devient :

```rust
                Err(TryRecvError::Disconnected) => {
                    let maintenant = Instant::now();
                    if self.fenetre.rupture(maintenant) {
                        // La fenêtre est expirée : l'épuisement est acquis.
                        self.epuisee = true;
                        return None;
                    }
                    if self.fenetre.peut_reessayer(maintenant) {
                        match self.canal.rattacher() {
                            Ok(Rattachee { images, largeur, hauteur }) => {
                                tracing::info!(largeur, hauteur, "canal rattaché au capteur");
                                self.images = images;
                                self.largeur = largeur;
                                self.hauteur = hauteur;
                                self.vivante = true;
                                self.epuisee = false;
                                self.fenetre.succes();
                            }
                            // Journalisé en `debug!` et non `info!` : au pas
                            // de 250 ms sur une fenêtre de 15 s, un capteur
                            // durablement absent produirait 60 lignes par
                            // fenêtre et par session.
                            Err(erreur) => tracing::debug!(%erreur, "rattachement refusé"),
                        }
                    }
                    return None;
                }
```

> ⚠️ **Le tour du rattachement ne rend pas d'image**, même réussi : `return
> None` est délibéré et les tests l'encodent. Le tour suivant lira la file
> neuve, 10 ms plus tard.

Renommer enfin le champ `commandes` en `canal` et son type en
`Box<dyn Canal + Send>`, dans la structure, dans `nouvelle` et dans
`commander_simple`.

- [ ] **Step 4 : Lancer les tests pour vérifier qu'ils passent**

Run: `cd agent && cargo test capteur::`
Expected: 8 (`protocole`) + 14 (`distante`) + 5 (`reprise`) = **27 tests**.

- [ ] **Step 5 : Vérifier la compilation croisée Windows**

Run: `cd agent && cargo check --target x86_64-pc-windows-gnu`
Expected: sortie 0.

- [ ] **Step 6 : Commit**

```bash
git add agent/src/capteur/distante.rs agent/src/capteur/reprise.rs
git commit -m "feat(d4): rattacher le canal au capteur relance, sans quoi la reprise ne reprend rien"
```

---

### Task 4 : L'horloge commune aux deux processus

**Files:**
- Create: `agent/src/capteur/horloge.rs`
- Modify: `agent/src/capteur.rs`
- Test: dans `agent/src/capteur/horloge.rs` (partie pure)

**Interfaces:**
- Produces: `pub fn lire_qpc() -> anyhow::Result<i64>` et `pub fn frequence_qpc() -> anyhow::Result<i64>` (`#[cfg(windows)]`) ; `pub fn origine_depuis_qpc(origine_qpc: i64, qpc_maintenant: i64, frequence: i64, maintenant: Instant) -> Instant` (**pur, hors `cfg`**).

- [ ] **Step 1 : Écrire le test d'abord**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn une_origine_anterieure_se_reconstruit_en_arriere() {
        let maintenant = Instant::now();
        // 10 MHz, et 25 millions de tics écoulés = 2,5 s.
        let reconstruite = origine_depuis_qpc(1_000_000, 26_000_000, 10_000_000, maintenant);
        let ecart = maintenant.duration_since(reconstruite);
        assert!(
            ecart.abs_diff(Duration::from_millis(2500)) < Duration::from_millis(1),
            "écart reconstruit : {ecart:?}"
        );
    }

    #[test]
    fn une_origine_egale_a_maintenant_ne_recule_pas() {
        let maintenant = Instant::now();
        assert_eq!(origine_depuis_qpc(42, 42, 10_000_000, maintenant), maintenant);
    }

    /// Une origine POSTÉRIEURE ne peut pas exister, mais une horloge lue de
    /// travers la produirait : on retombe alors sur `maintenant` plutôt que de
    /// paniquer en soustrayant au-delà de l'origine de l'`Instant`.
    #[test]
    fn une_origine_posterieure_retombe_sur_maintenant() {
        let maintenant = Instant::now();
        assert_eq!(origine_depuis_qpc(100, 50, 10_000_000, maintenant), maintenant);
    }

    #[test]
    fn une_frequence_nulle_retombe_sur_maintenant() {
        let maintenant = Instant::now();
        assert_eq!(origine_depuis_qpc(1, 2, 0, maintenant), maintenant);
    }
}
```

- [ ] **Step 2 : Lancer le test pour vérifier qu'il échoue**

Run: `cd agent && cargo test capteur::horloge`
Expected: échec de compilation — `origine_depuis_qpc` n'existe pas.

- [ ] **Step 3 : Écrire l'implémentation**

```rust
//! L'horloge commune au capteur et à ses enfants.
//!
//! `clock_origin` est un `std::time::Instant`, partagé côté enfant entre la
//! vidéo et l'audio : c'est cette origine commune qui rend les deux lignes de
//! temps comparables, donc la synchro A/V exacte. Un `Instant` n'a aucun sens
//! dans un autre processus — mais `QueryPerformanceCounter` est monotone et
//! **commun à toute la machine**. L'enfant envoie donc son origine en tics QPC,
//! et le capteur reconstruit l'`Instant` équivalent chez lui.
//!
//! La conversion est PURE et hors `cfg` : c'est elle qui peut être fausse, pas
//! l'appel système.

use std::time::{Duration, Instant};

/// Reconstruit l'origine d'horloge de l'enfant dans le référentiel `Instant`
/// du capteur.
///
/// Retombe sur `maintenant` dans les deux cas dégénérés — origine postérieure
/// à la lecture courante, ou fréquence nulle — plutôt que de paniquer : une
/// origine fausse décale la synchro A/V, une panique tue la fenêtre.
pub fn origine_depuis_qpc(
    origine_qpc: i64,
    qpc_maintenant: i64,
    frequence: i64,
    maintenant: Instant,
) -> Instant {
    if frequence <= 0 || qpc_maintenant <= origine_qpc {
        return maintenant;
    }
    let tics = (qpc_maintenant - origine_qpc) as u128;
    let nanos = tics * 1_000_000_000u128 / frequence as u128;
    maintenant
        .checked_sub(Duration::from_nanos(nanos.min(u64::MAX as u128) as u64))
        .unwrap_or(maintenant)
}

#[cfg(windows)]
mod systeme {
    use anyhow::{Context, Result};
    use windows::Win32::System::Performance::{
        QueryPerformanceCounter, QueryPerformanceFrequency,
    };

    pub fn lire_qpc() -> Result<i64> {
        let mut valeur = 0i64;
        unsafe { QueryPerformanceCounter(&mut valeur) }.context("QueryPerformanceCounter")?;
        Ok(valeur)
    }

    pub fn frequence_qpc() -> Result<i64> {
        let mut valeur = 0i64;
        unsafe { QueryPerformanceFrequency(&mut valeur) }.context("QueryPerformanceFrequency")?;
        Ok(valeur)
    }
}

#[cfg(windows)]
pub use systeme::{frequence_qpc, lire_qpc};
```

Ajouter `pub mod horloge;` à `agent/src/capteur.rs`, et `"Win32_System_Performance"` aux `features` du crate `windows` dans `agent/Cargo.toml` si elle n'y est pas déjà.

- [ ] **Step 4 : Lancer les tests pour vérifier qu'ils passent**

Run: `cd agent && cargo test capteur::horloge`
Expected: 4 tests passent.

- [ ] **Step 5 : Vérifier la compilation croisée Windows**

Run: `cd agent && cargo check --target x86_64-pc-windows-gnu`
Expected: sortie 0. Si `QueryPerformanceCounter` est introuvable, la feature `Win32_System_Performance` manque dans `Cargo.toml`.

- [ ] **Step 6 : Commit**

```bash
git add agent/src/capteur/horloge.rs agent/src/capteur.rs agent/Cargo.toml
git commit -m "feat(d4): rebaser l'origine d'horloge par QPC entre les deux processus"
```

---

### Task 5 : Le capteur — serveur de tube et fil de fenêtre

**Files:**
- Create: `agent/src/capteur/serveur.rs`
- Create: `agent/src/capteur/fenetre.rs`
- Modify: `agent/src/capteur.rs` (déclarations + `executer`)
- Modify: `agent/src/main.rs` (aiguillage du mode `CAPTEUR`)

**Interfaces:**
- Consumes: `capteur::protocole::*`, `capteur::horloge::{lire_qpc, frequence_qpc, origine_depuis_qpc}`, `crate::windows_source::WindowsSource`, `crate::source::VideoSource`.
- Produces: `capteur::executer() -> anyhow::Result<()>` ; `serveur::servir() -> anyhow::Result<()>` ; `fenetre::servir_une_fenetre<E: Write>(commandes: Receiver<VersCapteur>, ecrivain: E, attache: VersCapteur) -> anyhow::Result<()>`.

**Deux fils par fenêtre, et c'est délibéré.** Le fil **lecteur** lit le tube en
bloquant et dépose les `VersCapteur` dans un `std::sync::mpsc::channel` ; le fil
**de service** (`servir_une_fenetre`) ne fait que `try_recv` dessus. Sans cette
séparation, la boucle s'arrêterait à attendre une commande qui n'arrive presque
jamais. C'est aussi pourquoi `servir_une_fenetre` ne prend pas de lecteur mais un
`Receiver`.

**Contrainte non négociable** : `fenetre.rs` instancie `WindowsSource::sur_sortie(hwnd, &sortie, fps, debit, clock_origin)` **tel quel**. Aucune modification de `windows_source.rs`, `capture.rs` ni `encode.rs`.

**Pas de test hôte pour cette tâche** : elle est `#[cfg(windows)]` de bout en bout, comme `capture.rs` et `superviseur/hook.rs` avant elle. Sa vérification est `cargo check --target x86_64-pc-windows-gnu` puis la recette de la tâche 8.

- [ ] **Step 1 : Écrire `agent/src/capteur/fenetre.rs`**

```rust
//! Le fil d'une fenêtre : il tient un `WindowsSource` complet et le sert à
//! l'enfant par le tube.
//!
//! **Il ne réécrit AUCUN code de capture ni d'encodage.** `WindowsSource` est
//! déjà exactement le couple `DesktopCapture` + `H264Encoder` derrière le
//! trait `VideoSource` : ce module ne fait qu'appeler ce trait et transporter
//! ses résultats. C'est la simplification centrale du sous-bloc D4.

#![cfg(windows)]

use std::io::{BufReader, BufWriter, Write};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use windows::Win32::Foundation::HWND;

use crate::capteur::horloge::{frequence_qpc, lire_qpc, origine_depuis_qpc};
use crate::capteur::protocole::{
    ecrire_image, ecrire_json, lire_trame, DepuisCapteur, Trame, VersCapteur,
};
use crate::source::VideoSource;
use crate::windows_source::WindowsSource;

/// Pas de sommeil quand la source n'a rien rendu.
///
/// 10 ms, la valeur exacte de `FRAME_INTERVAL` côté transport : cette boucle
/// prend la place de l'interrogation que faisait l'enfant, et il n'y a aucune
/// raison de changer la cadence de sondage en même temps que le reste. Le
/// commentaire de `transport/piste_video.rs` explique pourquoi 10 ms et non
/// 16 : interroger plus souvent que la source ne produit lève une borne sans
/// rien coûter quand il n'y a rien à prendre.
const PAS_A_VIDE: Duration = Duration::from_millis(10);

/// Période des lignes de compteurs. **Jamais de trace par image** : le projet
/// a déjà perdu une session entière à une trace par paquet.
const PERIODE_COMPTEURS: Duration = Duration::from_secs(10);

pub fn servir_une_fenetre<E: Write>(
    commandes: std::sync::mpsc::Receiver<VersCapteur>,
    mut ecrivain: E,
    attache: VersCapteur,
) -> Result<()> {
    let VersCapteur::Attache { session, hwnd, sortie, fps, debit, origine_qpc } = attache else {
        bail!("le premier message d'un enfant doit être une attache");
    };

    let clock_origin = origine_depuis_qpc(
        origine_qpc,
        lire_qpc().context("lecture de QPC à l'attache")?,
        frequence_qpc().context("fréquence de QPC")?,
        Instant::now(),
    );

    let hwnd = HWND(hwnd as *mut core::ffi::c_void);
    let mut source = match WindowsSource::sur_sortie(hwnd, &sortie, fps, debit, clock_origin) {
        Ok(source) => source,
        Err(erreur) => {
            // Le refus est ANNONCÉ à l'enfant, jamais silencieux : sans ce
            // message il attendrait une image qui ne viendra pas.
            let _ = ecrire_json(
                &mut ecrivain,
                &DepuisCapteur::Refus { motif: format!("{erreur:#}") },
            );
            return Err(erreur).with_context(|| format!("attache de la session {session}"));
        }
    };

    let (largeur, hauteur) = source.dimensions();
    ecrire_json(&mut ecrivain, &DepuisCapteur::Attachee { largeur, hauteur })?;
    tracing::info!(%session, %sortie, largeur, hauteur, "fenêtre attachée au capteur");

    let mut dernier_etat = (true, false, largeur, hauteur);
    let mut images = 0u64;
    let mut dernier_compte = Instant::now();

    loop {
        // 1. Les commandes en attente, s'il y en a. Elles sont rares.
        loop {
            match commandes.try_recv() {
                Ok(message) => {
                    let reponse = executer_commande(&mut source, message);
                    ecrire_json(&mut ecrivain, &reponse)?;
                    ecrivain.flush()?;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                // L'enfant a fermé le tube. On sort par le haut, ce qui
                // relâche `source` — donc la duplication et l'encodeur — SUR
                // CE FIL-CI, jamais sur la boucle d'acceptation. `Drop for
                // H264Encoder` peut geler (risque observé, non attribué) ;
                // ici il ne gèlerait que cette fenêtre.
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    tracing::info!(%session, images, "l'enfant a fermé le canal");
                    return Ok(());
                }
            }
        }

        // 2. Une image, s'il y en a une.
        match source.next_frame() {
            Some(unite) => {
                images += 1;
                // Une écriture bloquante EST la contre-pression : si l'enfant
                // ne lit plus, ce fil attend — et il n'attend que pour SA
                // fenêtre. Une unité d'accès ne peut pas être jetée sans
                // corrompre le flux (les images P référencent les
                // précédentes).
                ecrire_image(&mut ecrivain, &unite)?;
                ecrivain.flush()?;
            }
            None => std::thread::sleep(PAS_A_VIDE),
        }

        // 3. L'état, au CHANGEMENT seulement.
        let (largeur, hauteur) = source.dimensions();
        let etat = (source.is_alive(), source.is_exhausted(), largeur, hauteur);
        if etat != dernier_etat {
            dernier_etat = etat;
            ecrire_json(
                &mut ecrivain,
                &DepuisCapteur::Etat {
                    vivante: etat.0,
                    epuisee: etat.1,
                    largeur: etat.2,
                    hauteur: etat.3,
                },
            )?;
            ecrivain.flush()?;
            if !etat.0 || etat.1 {
                tracing::info!(%session, vivante = etat.0, epuisee = etat.1, images,
                    "fin de la fenêtre côté capteur");
                return Ok(());
            }
        }

        if dernier_compte.elapsed() >= PERIODE_COMPTEURS {
            let ecoule = dernier_compte.elapsed().as_secs_f64();
            tracing::info!(
                %session,
                images,
                cadence = format!("{:.1}", images as f64 / ecoule),
                "cadence du capteur"
            );
            images = 0;
            dernier_compte = Instant::now();
        }
    }
}

fn executer_commande(source: &mut WindowsSource, message: VersCapteur) -> DepuisCapteur {
    let resultat = match message {
        VersCapteur::Redimensionner { largeur, hauteur } => {
            return match source.resize(largeur, hauteur) {
                Ok(()) => {
                    let (largeur, hauteur) = source.dimensions();
                    DepuisCapteur::Taille { largeur, hauteur }
                }
                Err(erreur) => DepuisCapteur::Erreur { motif: format!("{erreur:#}") },
            }
        }
        VersCapteur::TailleEncodage { largeur, hauteur } => source.set_encode_size(largeur, hauteur),
        VersCapteur::Debit { bps } => source.set_bitrate(bps),
        VersCapteur::ImageCle => source.request_keyframe(),
        VersCapteur::Attache { .. } => {
            return DepuisCapteur::Erreur { motif: "seconde attache sur un canal déjà attaché".into() }
        }
    };
    match resultat {
        Ok(()) => DepuisCapteur::Fait,
        Err(erreur) => DepuisCapteur::Erreur { motif: format!("{erreur:#}") },
    }
}
```

- [ ] **Step 2 : Écrire `agent/src/capteur/serveur.rs`**

```rust
//! Le serveur de tube nommé du capteur.
//!
//! Chaque enfant s'y connecte et se décrit lui-même dans sa première trame :
//! il n'y a donc AUCUN canal superviseur → capteur, et aucune table d'état
//! partagée entre trois processus. La fermeture du tube EST le signal de fin
//! de vie d'une fenêtre.

#![cfg(windows)]

use std::io::{BufReader, BufWriter};
use std::sync::mpsc::{channel, Sender};

use anyhow::{bail, Context, Result};
use windows::Win32::Foundation::{CloseHandle, ERROR_PIPE_CONNECTED, HANDLE};
// ⚠️ `PIPE_ACCESS_DUPLEX` vit dans `Storage::FileSystem` et NON dans
// `System::Pipes` en windows-rs 0.62 — vérifié dans les sources du crate.
// La feature `Win32_Storage_FileSystem` est déjà activée, employée par
// `moniteurs_virtuels/pilote.rs`.
use windows::Win32::Storage::FileSystem::PIPE_ACCESS_DUPLEX;
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE,
    PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};

use crate::capteur::fenetre::servir_une_fenetre;
use crate::capteur::protocole::{lire_trame, Trame, VersCapteur, NOM_TUBE};

/// Tampon de tube, dans les deux sens. Généreux à dessein : c'est lui qui
/// absorbe les à-coups avant que la contre-pression ne remonte jusqu'au fil
/// de capture.
const TAMPON: u32 = 1024 * 1024;

pub fn servir() -> Result<()> {
    loop {
        // Une instance NEUVE par client. `PIPE_UNLIMITED_INSTANCES` autorise
        // autant d'instances simultanées que de fenêtres — et c'est vrai
        // parce que `accueillir` est déportée sur son propre fil ci-dessous :
        // cette boucle revient écouter tout de suite.
        let tube = creer_instance().context("création d'une instance de tube")?;

        // Bloque jusqu'à ce qu'un enfant se connecte.
        //
        // ⚠️ `ERROR_PIPE_CONNECTED` est un SUCCÈS, pas une erreur : l'enfant
        // s'est connecté dans l'intervalle entre `CreateNamedPipeW` et cet
        // appel, et le tube est bel et bien connecté. Le passer par `?`
        // ferait mourir le processus capteur — donc TOUTES les fenêtres —
        // sur une course banale. (Ronde de correction de la tâche 5.)
        if let Err(erreur) = unsafe { ConnectNamedPipe(tube, None) } {
            if erreur.code() != ERROR_PIPE_CONNECTED.to_hresult() {
                // Un échec RÉEL ne fait pas non plus tomber le serveur : on
                // referme l'instance — sinon le handle fuit — et on reboucle.
                tracing::warn!(%erreur, "connexion d'un enfant échouée");
                let _ = unsafe { CloseHandle(tube) };
                continue;
            }
        }

        // Déportée sur son propre fil, DÉTACHÉ : `accueillir` lit la trame
        // d'attache en bloquant, et un enfant qui se connecterait sans jamais
        // l'envoyer bloquerait sinon l'accueil de toutes les autres fenêtres.
        std::thread::spawn(move || {
            if let Err(erreur) = accueillir(tube) {
                // Une attache ratée ne fait PAS tomber le serveur : les autres
                // fenêtres continuent. C'est tout l'intérêt d'avoir un capteur
                // qui survit à ses fenêtres.
                tracing::warn!(%erreur, "attache d'un enfant refusée");
            }
        });
    }
}

fn creer_instance() -> Result<HANDLE> {
    let nom: Vec<u16> = NOM_TUBE.encode_utf16().chain(std::iter::once(0)).collect();
    let tube = unsafe {
        CreateNamedPipeW(
            windows::core::PCWSTR(nom.as_ptr()),
            PIPE_ACCESS_DUPLEX,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
            PIPE_UNLIMITED_INSTANCES,
            TAMPON,
            TAMPON,
            0,
            None,
        )
    };
    if tube.is_invalid() {
        bail!("CreateNamedPipeW a rendu un handle invalide");
    }
    Ok(tube)
}

/// Lit la première trame — qui DOIT être une attache — puis détache les deux
/// fils de cette fenêtre.
fn accueillir(tube: HANDLE) -> Result<()> {
    // `std::fs::File` depuis le handle : il donne `Read`/`Write` sans écrire
    // d'enveloppe, et sa fermeture ferme le tube.
    use std::os::windows::io::FromRawHandle;
    let fichier = unsafe { std::fs::File::from_raw_handle(tube.0 as *mut _) };
    let mut lecteur = BufReader::new(fichier.try_clone().context("clone du tube en lecture")?);
    let ecrivain = BufWriter::new(fichier);

    let attache = match lire_trame(&mut lecteur).context("première trame de l'enfant")? {
        Trame::Json(octets) => serde_json::from_slice::<VersCapteur>(&octets)
            .context("première trame illisible")?,
        Trame::Image(_) => bail!("le premier message d'un enfant ne peut pas être une image"),
    };
    if !matches!(attache, VersCapteur::Attache { .. }) {
        bail!("le premier message d'un enfant doit être une attache, reçu {attache:?}");
    }

    let (tx, rx) = channel::<VersCapteur>();

    // Fil LECTEUR : lit le tube en bloquant, dépose les commandes.
    std::thread::spawn(move || lire_les_commandes(lecteur, tx));

    // Fil de SERVICE : tient le `WindowsSource`. DÉTACHÉ, jamais joint — la
    // boucle d'acceptation ne doit pas pouvoir être bloquée par un démontage
    // d'encodeur (`Drop for H264Encoder` peut geler).
    std::thread::spawn(move || {
        if let Err(erreur) = servir_une_fenetre(rx, ecrivain, attache) {
            tracing::warn!(%erreur, "fil de fenêtre terminé sur erreur");
        }
    });
    Ok(())
}

fn lire_les_commandes<R: std::io::Read>(mut lecteur: R, tx: Sender<VersCapteur>) {
    loop {
        match lire_trame(&mut lecteur) {
            Ok(Trame::Json(octets)) => match serde_json::from_slice::<VersCapteur>(&octets) {
                Ok(message) => {
                    if tx.send(message).is_err() {
                        return; // le fil de service est parti
                    }
                }
                Err(erreur) => {
                    tracing::warn!(%erreur, "commande illisible, canal abandonné");
                    return;
                }
            },
            Ok(Trame::Image(_)) => {
                tracing::warn!("un enfant a envoyé une image, canal abandonné");
                return;
            }
            // Fin de tube : l'enfant est parti. Laisser tomber `tx` signale
            // `Disconnected` au fil de service, qui démonte sa source.
            Err(_) => return,
        }
    }
}
```

> **`PIPE_WAIT` et non `PIPE_NOWAIT`** : la lecture bloquante est justement ce
> que le fil lecteur dédié rend acceptable, et l'écriture bloquante **est** la
> contre-pression décrite au §3.4 de la conception.

- [ ] **Step 3 : Câbler le point d'entrée**

Dans `agent/src/capteur.rs` :

```rust
#[cfg(windows)]
pub mod fenetre;
#[cfg(windows)]
pub mod serveur;

/// Point d'entrée du mode capteur.
#[cfg(windows)]
pub fn executer() -> anyhow::Result<()> {
    tracing::info!(tube = protocole::NOM_TUBE, "capteur démarré");
    serveur::servir()
}

#[cfg(not(windows))]
pub fn executer() -> anyhow::Result<()> {
    anyhow::bail!("le mode capteur n'existe que sur Windows")
}
```

Dans `agent/src/main.rs`, aiguiller **avant** la branche superviseur et après
`diagnostics::aiguiller()`, sur le modèle exact de la branche `superviseur` :

```rust
    // `CAPTEUR=0` DÉSACTIVE le mode, comme `SUPERVISEUR=0` et `AUDIO=0` —
    // même piège d'exploitation, même parade.
    if matches!(std::env::var("CAPTEUR").as_deref(), Ok(v) if v != "0") {
        return capteur::executer();
    }
```

- [ ] **Step 4 : Vérifier la compilation croisée Windows**

Run: `cd agent && cargo check --target x86_64-pc-windows-gnu`
Expected: sortie 0, et **aucun avertissement dans les fichiers neufs**.

- [ ] **Step 5 : Vérifier que les tests hôte passent toujours**

Run: `cd agent && cargo test`
Expected: tout passe.

- [ ] **Step 6 : Contrôler la dette de taille**

Run:
```bash
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
```
Expected: **les trois fichiers de dette gelée, et aucun de plus.** Si
`fenetre.rs` ou `serveur.rs` dépasse 500, extraire avant de committer.

- [ ] **Step 7 : Commit**

```bash
git add agent/src/capteur/fenetre.rs agent/src/capteur/serveur.rs agent/src/capteur.rs agent/src/main.rs
git commit -m "feat(d4): le capteur — serveur de tube, et un fil par fenetre"
```

---

### Task 6 : L'enfant se branche sur le capteur

**Files:**
- Create: `agent/src/capteur/tube.rs`
- Modify: `agent/src/demarrage/source.rs`
- Modify: `agent/src/capteur.rs`

**Interfaces:**
- Consumes: `capteur::distante::{Commandes, Recu, SourceDistante}`, `capteur::protocole::*`, `capteur::horloge::lire_qpc`.
- Produces: `tube::connecter(session: &str, hwnd: u64, sortie: &str, fps: u32, debit: u32, clock_origin: Instant) -> Result<SourceDistante>`.

- [ ] **Step 1 : Écrire `agent/src/capteur/tube.rs`**

```rust
//! Le client de tube côté enfant : il se connecte au capteur, s'attache, et
//! rend une `SourceDistante` prête à servir la boucle de transport.

#![cfg(windows)]

use std::io::{BufReader, BufWriter, Write};
use std::sync::mpsc::{channel, sync_channel, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};

use crate::capteur::distante::{Commandes, Recu, SourceDistante};
use crate::capteur::horloge::lire_qpc;
use crate::capteur::protocole::{
    ecrire_json, lire_trame, DepuisCapteur, Trame, VersCapteur, NOM_TUBE,
};
use crate::capteur::reprise::DUREE_FENETRE_CANAL;

/// Profondeur de la file d'images entre le fil lecteur et `next_frame`.
///
/// **C'est elle qui exerce la contre-pression sur toute la chaîne** : file
/// pleine → le fil lecteur bloque → le tampon du tube se remplit → l'écriture
/// du capteur bloque, et son fil de fenêtre attend. Une unité d'accès ne peut
/// pas être jetée sans corrompre le flux, donc bloquer est la seule issue
/// correcte. 8 unités ≈ 90 ms de vidéo à 90 i/s : assez pour absorber un
/// à-coup d'ordonnancement, trop peu pour laisser une session dériver en
/// silence.
const CAPACITE_FILE: usize = 8;

/// Attente maximale d'une réponse à une commande.
///
/// **Majorant assumé** : `resize` peut reconstruire une chaîne d'encodage
/// complète, et `Drop for H264Encoder` porte une partie bornée de 8 s au pire
/// cas. Trop court, on déclarerait morte une fenêtre qui travaille.
const DELAI_COMMANDE: Duration = Duration::from_secs(10);

/// Pas entre deux tentatives de connexion. Même valeur que le pas de reprise
/// de `capture/reprise.rs`, et pour la même raison : assez petit pour ne pas
/// retarder la reprise réelle, assez grand pour que la trace reste rare.
const PAS_CONNEXION: Duration = Duration::from_millis(150);

pub fn connecter(
    session: &str,
    hwnd: u64,
    sortie: &str,
    fps: u32,
    debit: u32,
    clock_origin: Instant,
) -> Result<SourceDistante> {
    let signalement = Signalement {
        session: session.to_string(),
        hwnd,
        sortie: sortie.to_string(),
        fps,
        debit,
        clock_origin,
    };
    // La PREMIÈRE ouverture est patiente : l'enfant peut démarrer avant que le
    // capteur n'ait ouvert son tube. Les réouvertures de `rattacher`, elles,
    // ne le sont pas — elles courent depuis la boucle de transport.
    let attachee = attacher_sur(ouvrir_avec_patience()?, &signalement)?;
    let (largeur, hauteur) = (attachee.largeur, attachee.hauteur);
    Ok(SourceDistante::nouvelle(
        Box::new(CanalTube {
            ecrivain: Mutex::new(attachee.ecrivain),
            reponses: attachee.reponses,
            signalement,
        }),
        attachee.images,
        largeur,
        hauteur,
    ))
}

/// Envoie l'attache sur un tube déjà ouvert, lit la réponse, et démarre le fil
/// répartiteur. **Partagée par `connecter` et `rattacher`** : les deux ne
/// doivent pas porter deux copies de cette séquence.
fn attacher_sur(fichier: std::fs::File, signalement: &Signalement) -> Result<Attachee> {
    let mut ecrivain = BufWriter::new(fichier.try_clone().context("clone du tube en écriture")?);
    let mut lecteur = BufReader::new(fichier);

    // `clock_origin` a été créée par `demarrage.rs` AVANT cet appel — la
    // connexion peut avoir attendu le capteur plusieurs secondes, et un
    // rattachement survient bien plus tard encore. Lire QPC maintenant et
    // l'envoyer tel quel décalerait la vidéo de tout cet écart par rapport à
    // l'audio, qui partage `clock_origin`. On CORRIGE donc de l'écoulé, ce
    // qui rend l'origine exacte à chaque attache.
    let frequence = crate::capteur::horloge::frequence_qpc()?;
    let ecoule_tics = (signalement.clock_origin.elapsed().as_nanos() * frequence as u128
        / 1_000_000_000)
        .min(i64::MAX as u128) as i64;
    let origine_qpc = lire_qpc().context("lecture de QPC avant l'attache")? - ecoule_tics;

    ecrire_json(
        &mut ecrivain,
        &VersCapteur::Attache {
            session: signalement.session.clone(),
            hwnd: signalement.hwnd,
            sortie: signalement.sortie.clone(),
            fps: signalement.fps,
            debit: signalement.debit,
            origine_qpc,
        },
    )?;
    ecrivain.flush()?;

    let (largeur, hauteur) = match lire_trame(&mut lecteur).context("réponse à l'attache")? {
        Trame::Json(octets) => match serde_json::from_slice::<DepuisCapteur>(&octets)? {
            DepuisCapteur::Attachee { largeur, hauteur } => (largeur, hauteur),
            // Un refus fait échouer l'attache BRUYAMMENT : sans cela l'enfant
            // attendrait une image qui ne viendra jamais.
            DepuisCapteur::Refus { motif } => bail!("le capteur a refusé l'attache : {motif}"),
            autre => bail!("réponse inattendue à l'attache : {autre:?}"),
        },
        Trame::Image(_) => bail!("le capteur a répondu une image à l'attache"),
    };
    tracing::info!(
        session = %signalement.session,
        sortie = %signalement.sortie,
        largeur, hauteur,
        "attaché au capteur"
    );

    let (tx_images, rx_images) = sync_channel::<Recu>(CAPACITE_FILE);
    let (tx_reponses, rx_reponses) = channel::<DepuisCapteur>();
    std::thread::spawn(move || repartir_les_trames(lecteur, tx_images, tx_reponses));

    Ok(Attachee {
        ecrivain,
        reponses: rx_reponses,
        images: rx_images,
        largeur,
        hauteur,
    })
}

/// Réessaie la connexion dans une fenêtre bornée : l'enfant peut démarrer
/// avant que le capteur n'ait ouvert son tube — au tout premier lancement, ou
/// pendant une relance du capteur.
fn ouvrir_avec_patience() -> Result<std::fs::File> {
    let debut = Instant::now();
    let mut derniere = None;
    while debut.elapsed() <= DUREE_FENETRE_CANAL {
        match std::fs::OpenOptions::new().read(true).write(true).open(NOM_TUBE) {
            Ok(fichier) => return Ok(fichier),
            Err(erreur) => {
                derniere = Some(erreur);
                std::thread::sleep(PAS_CONNEXION);
            }
        }
    }
    Err(anyhow::Error::from(derniere.expect("au moins une tentative"))
        .context(format!("aucun capteur sur {NOM_TUBE} après {DUREE_FENETRE_CANAL:?}")))
}

/// Aiguille chaque trame reçue : les images vers la file bornée, les réponses
/// de commande vers leur propre canal. Les deux ne doivent PAS partager une
/// file — une réponse coincée derrière huit images bloquerait `commander`.
fn repartir_les_trames<R: std::io::Read>(
    mut lecteur: R,
    images: SyncSender<Recu>,
    reponses: Sender<DepuisCapteur>,
) {
    loop {
        let trame = match lire_trame(&mut lecteur) {
            Ok(trame) => trame,
            // Fin de tube : le capteur est parti. Laisser tomber les deux
            // émetteurs fait rendre `Disconnected` à `SourceDistante`, qui
            // OUVRE SA FENÊTRE DE REPRISE au lieu de clore la session.
            Err(_) => return,
        };
        let envoi = match trame {
            Trame::Image(unite) => images.send(Recu::Image(unite)).is_ok(),
            Trame::Json(octets) => match serde_json::from_slice::<DepuisCapteur>(&octets) {
                Ok(DepuisCapteur::Etat { vivante, epuisee, largeur, hauteur }) => images
                    .send(Recu::Etat { vivante, epuisee, largeur, hauteur })
                    .is_ok(),
                Ok(reponse) => reponses.send(reponse).is_ok(),
                Err(erreur) => {
                    tracing::warn!(%erreur, "trame illisible du capteur, canal abandonné");
                    return;
                }
            },
        };
        if !envoi {
            return; // la source est partie
        }
    }
}

struct CanalTube {
    /// `Mutex` et non `&mut` : `Canal::commander` prend `&mut self`, mais
    /// l'écrivain est aussi le seul point d'écriture du tube et rien ne promet
    /// qu'il restera consulté depuis un seul fil.
    ecrivain: Mutex<BufWriter<std::fs::File>>,
    reponses: Receiver<DepuisCapteur>,
    /// De quoi se réattacher à un capteur relancé. Retenu à la connexion :
    /// au moment de la rupture, plus rien d'autre ne porte ces valeurs.
    signalement: Signalement,
}

/// Ce qu'il faut redire au capteur pour se réattacher.
///
/// `clock_origin` est retenue et NON figée en tics QPC : chaque attache
/// recalcule `origine_qpc` à partir d'elle, de sorte que l'origine reste
/// exacte quel que soit le temps écoulé depuis le démarrage de l'enfant.
struct Signalement {
    session: String,
    hwnd: u64,
    sortie: String,
    fps: u32,
    debit: u32,
    clock_origin: Instant,
}

/// Le fruit d'une attache réussie, côté enfant.
struct Attachee {
    ecrivain: BufWriter<std::fs::File>,
    reponses: Receiver<DepuisCapteur>,
    images: Receiver<Recu>,
    largeur: u32,
    hauteur: u32,
}

impl Canal for CanalTube {
    /// Rouvre un tube vers le capteur (relancé par le superviseur) et
    /// réémet l'attache. Remplace l'écrivain et le canal de réponses de
    /// CE `CanalTube`, et rend la file d'images neuve.
    ///
    /// **Sans cette méthode, la fenêtre de reprise de `SourceDistante` ne
    /// ferait que retarder la mort des sessions de 15 s** : rien d'autre
    /// n'ouvre jamais un second tube. Voir la tâche 3bis.
    ///
    /// Une seule tentative, sans patience interne : c'est `SourceDistante`
    /// qui tient le budget et l'espacement (`PAS_RATTACHEMENT`). Ouvrir le
    /// tube directement par `OpenOptions`, PAS par `ouvrir_avec_patience`,
    /// qui bloquerait la boucle de transport jusqu'à 15 s.
    fn rattacher(&mut self) -> Result<Rattachee> {
        let fichier = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(NOM_TUBE)
            .context("réouverture du tube du capteur")?;
        let attachee = attacher_sur(fichier, &self.signalement)?;
        // Remplacer l'état d'écriture de CE canal : l'ancien pointe sur un
        // tube mort, et `commander` l'emploierait encore.
        self.ecrivain = Mutex::new(attachee.ecrivain);
        self.reponses = attachee.reponses;
        Ok(Rattachee {
            images: attachee.images,
            largeur: attachee.largeur,
            hauteur: attachee.hauteur,
        })
    }

    fn commander(&mut self, message: VersCapteur) -> Result<DepuisCapteur> {
        {
            let mut ecrivain = self
                .ecrivain
                .lock()
                .unwrap_or_else(|empoisonne| empoisonne.into_inner());
            ecrire_json(&mut *ecrivain, &message)?;
            ecrivain.flush()?;
        }
        self.reponses
            .recv_timeout(DELAI_COMMANDE)
            .with_context(|| format!("aucune réponse du capteur à {message:?}"))
    }
}
```

> ⚠️ **`Arc` est importé mais peut ne pas servir** selon la forme finale : ne pas
> laisser d'import inutilisé, `cargo check` le signalerait.

- [ ] **Step 2 : Modifier `agent/src/demarrage/source.rs`**

Remplacer la branche `Some(nom_sortie)` :

```rust
    let source: Box<dyn VideoSource + Send> = match &config.sortie_dxgi {
        Some(nom_sortie) => {
            // Mode multi-fenêtres : la capture et l'encodage vivent dans le
            // CAPTEUR, un seul processus pour toutes les fenêtres. C'est ce
            // qui lève le plafond de quatre processus tenant une duplication
            // DXGI (sous-bloc D3). L'enfant ne touche plus ni DXGI ni Media
            // Foundation.
            tracing::info!(
                nom_sortie = %nom_sortie,
                bitrate,
                fps,
                "source distante servie par le capteur (mode multi-fenêtres)"
            );
            Box::new(crate::capteur::tube::connecter(
                &config.session_id,
                hwnd.0 as u64,
                nom_sortie,
                fps,
                bitrate,
                clock_origin,
            )?)
        }
        None => {
            // Mode mono-fenêtre, inchangé : agent lancé à la main, aucun
            // capteur. Ce chemin ne doit RIEN perdre au passage.
            tracing::info!(bitrate, fps, "capture de la fenêtre Windows (recadrage)");
            Box::new(windows_source::WindowsSource::new(hwnd, fps, bitrate, clock_origin)?)
        }
    };
```

> ⚠️ `WindowsSource::sur_sortie` n'est plus appelée ici — elle l'est désormais
> par `capteur/fenetre.rs`. **Ne pas la supprimer** ni la marquer `dead_code`.

Dans `agent/src/capteur.rs`, ajouter `#[cfg(windows)] pub mod tube;`.

- [ ] **Step 3 : Vérifier la compilation croisée Windows**

Run: `cd agent && cargo check --target x86_64-pc-windows-gnu`
Expected: sortie 0.

- [ ] **Step 4 : Vérifier que les tests hôte passent toujours**

Run: `cd agent && cargo test`
Expected: tout passe.

- [ ] **Step 5 : Commit**

```bash
git add agent/src/capteur/tube.rs agent/src/capteur.rs agent/src/demarrage/source.rs
git commit -m "feat(d4): l'enfant consomme sa video par le canal du capteur"
```

---

### Task 7 : Le superviseur lance et surveille le capteur

**Files:**
- Modify: `agent/src/superviseur/lanceur.rs`
- Modify: `agent/src/superviseur/boucle.rs`
- Modify: `scripts/run-agent.sh`

**Interfaces:**
- Produces: `LanceurDeProcessus::lancer_capteur(&self) -> Result<u32>` ; `LanceurDeProcessus::capteur_vivant(&self) -> bool`.

- [ ] **Step 1 : Ajouter le lancement du capteur dans `lanceur.rs`**

Le capteur se lance avec le **même exécutable**, `CAPTEUR=1`, et **doit être
rattaché au même job object** (`KILL_ON_JOB_CLOSE`) que les enfants : sans cela
il survivrait au superviseur en tenant N duplications et N sorties.

Variables à **retirer** de son environnement, pour la raison exacte que
documente déjà `lancer` : `SUPERVISEUR` (il se prendrait pour un superviseur),
`TEST_FILE` et `WINDOW_TITLE` (elles changent le sens d'une source). Variables
à **conserver** : `BITRATE`, `ENCODER_FPS`, `SOURCE_TRACE`, `RUST_LOG` — ce sont
des réglages, pas des changements de mode.

```rust
    fn lancer_capteur(&self) -> Result<u32> {
        let mut capteur = std::process::Command::new(&self.executable)
            .env("CAPTEUR", "1")
            .env_remove("SUPERVISEUR")
            .env_remove("TEST_FILE")
            .env_remove("WINDOW_TITLE")
            .spawn()
            .context("lancement du capteur")?;
        let pid = capteur.id();
        let handle = HANDLE(capteur.as_raw_handle() as *mut core::ffi::c_void);
        if let Err(erreur) = unsafe { AssignProcessToJobObject(self.job, handle) } {
            // Même contrat atomique que `lancer` : un capteur non rattaché au
            // job survivrait au superviseur EN TENANT N duplications.
            if let Err(mise_a_mort) = capteur.kill() {
                tracing::error!(pid, %mise_a_mort,
                    "capteur NON rattaché au job ET NON tué — il survivra au superviseur");
            }
            let _ = capteur.wait();
            return Err(anyhow::Error::new(erreur)
                .context(format!("rattachement du capteur {pid} au job object")));
        }
        tracing::info!(pid, "capteur lancé");
        Ok(pid)
    }
```

- [ ] **Step 2 : Lancer et surveiller le capteur dans `boucle.rs`**

Le capteur est lancé **avant** la première fenêtre, et relancé s'il meurt. La
surveillance se fait au même tour de boucle que `Enfants::morts()`.

⚠️ **`boucle.rs` est à 485 lignes, marge 15.** Si l'ajout la dépasse, extraire la
surveillance dans `agent/src/superviseur/boucle/capteur.rs` — le répertoire
existe déjà (`boucle/placement_periodique.rs`) et c'est le bon endroit.

Points imposés :

- une relance est journalisée en `warn!` avec le PID mort et le PID neuf ;
- **aucune fenêtre n'est fermée** quand le capteur meurt : les enfants tiennent
  sur leur fenêtre de reprise et se raccrochent. C'est le critère 2.

- [ ] **Step 3 : Transmettre `CAPTEUR` dans `scripts/run-agent.sh`**

Piège payé trois fois (`SUPERVISEUR` en D1, `MULTIFENETRE_REPRISE` en D2). Ajouter
`CAPTEUR` **dans la même tâche que le mode**, sur le modèle exact de
`SUPERVISEUR`.

- [ ] **Step 4 : Vérifier la compilation croisée Windows**

Run: `cd agent && cargo check --target x86_64-pc-windows-gnu`
Expected: sortie 0.

- [ ] **Step 5 : Contrôler la dette de taille**

Run: la commande de dette du Global Constraints.
Expected: les trois fichiers gelés, et aucun de plus. **Vérifier explicitement
`boucle.rs`** : `wc -l agent/src/superviseur/boucle.rs`.

- [ ] **Step 6 : Commit**

```bash
git add agent/src/superviseur/lanceur.rs agent/src/superviseur/boucle.rs scripts/run-agent.sh
git commit -m "feat(d4): le superviseur lance le capteur, le rattache au job et le relance"
```

---

### Task 8 : Rouvrir `CAPACITE` et compter la cadence côté enfant

**Files:**
- Modify: `agent/src/superviseur/boucle.rs`
- Modify: `agent/src/transport/piste_video.rs` (compteur)

- [ ] **Step 1 : Poser `CAPACITE` à la valeur de mesure**

`CAPACITE = 4` encode exactement le plafond que ce sous-bloc lève. La porter à
**8**, avec une documentation qui dit ce qu'elle est :

```rust
/// Nombre maximal de fenêtres servies simultanément.
///
/// **8, et voici exactement ce que ce chiffre est.** D3 avait ramené cette
/// valeur à 4, le plafond de processus concurrents tenant une duplication DXGI.
/// Le capteur mutualise désormais toutes les duplications dans un seul
/// processus : ce plafond-là ne mord plus. Le plafond qui prend le relais est
/// celui des **encodeurs** — 8 dans un processus, la 9ᵉ refusée au
/// `SetOutputType` de la MFT NVIDIA (`MF_E_UNSUPPORTED_D3D_TYPE`), mesuré deux
/// fois, les 30 et 31 juillet 2026, et inchangé que les encodeurs partagent un
/// périphérique D3D11 ou qu'ils en aient chacun un neuf.
///
/// ⚠️ **Valeur mesurée sur cette VM, à 1280×720 / 60 Hz / 8 Mb/s, non prouvée
/// être une borne du système.** La couche qui l'impose n'est pas identifiée
/// (NVENC, pilote, Media Foundation, ou virtualisation). À corriger au rang que
/// la recette du sous-bloc atteint réellement, s'il diffère.
const CAPACITE: usize = 8;
```

- [ ] **Step 2 : Compter la cadence côté enfant**

Le capteur compte déjà ses images (tâche 5). Il faut le **pendant côté enfant**,
pour que le critère 3 oppose deux chiffres et non un seul : dans
`transport/piste_video.rs`, un compteur d'unités écrites, journalisé toutes les
10 s en `info!` avec la session.

⚠️ **`piste_video.rs` est à 466 lignes, marge 34.** Si l'ajout la dépasse,
extraire le compteur dans un module voisin.

⚠️ **Jamais de trace par image.** Compter, et journaliser au pas de 10 s.

- [ ] **Step 3 : Vérifier compilation et tests**

Run: `cd agent && cargo test && cargo check --target x86_64-pc-windows-gnu`
Expected: tout passe, sortie 0.

- [ ] **Step 4 : Commit**

```bash
git add agent/src/superviseur/boucle.rs agent/src/transport/piste_video.rs
git commit -m "feat(d4): CAPACITE au plafond d'encodeurs, et un compteur de cadence par cote"
```

---

### Task 9 : Recette sur la VM

**Files:**
- Create: `docs/superpowers/plans/journaux-multifenetres-d4/` (journaux versés)
- Create: `docs/superpowers/plans/2026-08-02-multifenetres-capture-mutualisee-resultats.md`

**Préalables, dans cet ordre :**

```bash
virsh list --all                       # « fermé » = éteinte
virsh start Windows
until timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null; do sleep 5; done
until ls /media/vm/dev >/dev/null 2>&1; do sleep 5; done   # le partage se monte APRÈS WinRM
set -a && source .env && set +a        # SANS QUOI build-agent.sh s'arrête EN SILENCE
cd agent && cargo check --target x86_64-pc-windows-gnu && cd ..
scripts/build-agent.sh 2>&1 | tee /tmp/build-d4.log        # capturer la sortie, pas la recopier
```

⚠️ **Vérifier `Get-Process agent` avant chaque exécution** : `run-agent.sh` ne
tue pas l'agent existant, et un agent survit à l'hibernation de la VM. Sans ce
contrôle on mesure le processus précédent.

⚠️ **Aucune capture d'écran CDP pendant une mesure** : elle provoque un `Resize`,
donc un `SHOW`, donc une session et une sortie de plus. Et toute évaluation CDP
sur une page portant un flux WebRTC actif doit être **bornée** — elle peut ne
jamais rendre.

⚠️ **Lancer le navigateur AVANT le superviseur** : depuis D3, le garde-fou
`DELAI_ATTENTE_VIEWPORT_MAX = 30 s` s'applique à **toutes** les entrées, et une
entrée abandonnée n'est jamais reproposée.

- [ ] **Step 1 : Critère 1 — dépasser quatre fenêtres, et nommer le plafond suivant**

Ouvrir des fenêtres une à une (applications à **fenêtre unique** — pas Paint, qui
en ouvre deux éligibles), jusqu'au refus. Relever :

- le nombre de fenêtres simultanément **diffusant** (ICE établi, images comptées) ;
- au rang qui échoue : **l'appel exact et son `HRESULT`**, depuis le journal.
  Ne pas se fier au texte du HRESULT pour désigner l'appel — les contextes de
  `encode.rs` sont là pour cela.

Verser `agent.log` sous `journaux-multifenetres-d4/critere1-*.log`. **Copier le
journal avant tout relevé qui écrit au même endroit** — deux pièces ont été
perdues ainsi en D2.

- [ ] **Step 2 : Critère 2 — tuer le capteur ne tue aucune session**

Avec N fenêtres diffusant, tuer le capteur par PID (**jamais `pkill -f capteur`
depuis un shell dont la ligne de commande contient le motif** : il tue le shell,
exit 144). Relever, sur une **fenêtre bornée par horodatages explicites** :

- le délai relance du capteur → première image reçue, par session ;
- le nombre de `clôture de session amorcée` **non sollicitées** : attendu **0** ;
- que les N sessions rendent à nouveau des images.

- [ ] **Step 3 : Critère 3 — la cadence, avant et après**

Deux exécutions à N fenêtres identiques : l'une sur le commit précédant la
tâche 6 (source locale), l'autre sur HEAD (source distante). Relever les
compteurs des deux côtés. **Aucun seuil de réception** : le relevé existe pour
que le document puisse dire ce que le trajet IPC coûte.

- [ ] **Step 4 : Contrôle de topologie depuis un processus neuf**

```bash
MULTIFENETRE_DXGI=1 scripts/run-agent.sh
MULTIFENETRE_VDD_PURGE=1 scripts/run-agent.sh   # si des sorties ont fuité
```

**Comparer des ensembles de NOMS, jamais des nombres** : Apollo peut ajouter une
sortie et compenser exactement un retrait.

- [ ] **Step 5 : Écrire le document de résultats**

Structure imposée par les sous-blocs précédents : ce qui a été exécuté et ce qui
a été écarté ; le relevé de chaque critère avec ses lignes de journal citées ;
**ce que la mesure NE dit pas** ; les pièges neufs ; le contrôle de dette de
taille ; la vérification finale.

⚠️ **Le mode de défaillance dominant de ce projet est l'énoncé, pas le code.**
N'affirmer que ce que le relevé porte. Une exécution unique ne donne **aucun
taux** — le dire.

- [ ] **Step 6 : Mettre `CLAUDE.md` à jour**

Une section « Sous-bloc D4 » sur le modèle de D1/D2/D3, **et** l'annotation des
affirmations que D4 réfute ou complète dans les sections antérieures.
**Chercher par le SENS, pas par la formule** : balayer sur la chose niée
(un plafond, une mesure, une explication) en énumérant les tournures —
« pas / non / jamais / reste ouvert / n'est établi par rien ». **Annoter
l'affirmation elle-même, pas sa voisine**, et traiter le **sommaire** autant que
le chapitre de détail.

Mettre aussi à jour le tableau de dette de taille de `CLAUDE.md` **avec les
chiffres relevés**, pas recopiés.

- [ ] **Step 7 : Commit**

```bash
git add docs/superpowers/plans/journaux-multifenetres-d4 \
        docs/superpowers/plans/2026-08-02-multifenetres-capture-mutualisee-resultats.md \
        CLAUDE.md
git commit -m "recette(d4): resultats des trois criteres, et mise a jour de CLAUDE.md"
```

---

### Task 10 : Une connexion par sens

**Files:**
- Modify: `agent/src/capteur/protocole.rs`, `agent/src/capteur/serveur.rs`, `agent/src/capteur/fenetre.rs`, `agent/src/capteur/tube.rs`

**Pourquoi cette tâche existe.** La recette (tâche 9) a établi, par expérience
différentielle sur le même binaire à une variable près, que **l'écriture du
capteur sur le tube n'aboutit pas tant que son fil lecteur de commandes a une
lecture bloquante pendante sur la même instance de tube**. Empêcher ce fil
d'entrer en lecture fait aboutir l'attache des deux côtés en **34 µs**, passer
l'ICE à `connected`, et arriver de vraies images H.264 1280×720. Aucun des trois
critères de réception n'était atteignable sans cela.

⚠️ **Le mécanisme reste inconnu** — l'hypothèse de la sérialisation des E/S sur
un objet fichier synchrone Windows est *contrariée* par une observation côté
enfant, et la recette a eu raison de ne pas conclure. **Cette tâche ne prétend
pas l'expliquer : elle rend la situation impossible.**

**Le principe : aucun objet fichier ne porte jamais une lecture et une écriture
concurrentes.** Deux connexions par fenêtre, et une discipline de fil à chaque
bout.

| Connexion | Capteur | Enfant | Concurrence |
| --- | --- | --- | --- |
| **A — média** | écrit seulement (fil de fenêtre) | lit seulement (fil répartiteur) | aucune : un seul sens par bout |
| **B — commandes** | lit puis écrit, **sur un seul fil** | écrit puis lit, **sur le fil appelant** | aucune : stricte alternance |

Sur B, il n'y a **plus de fil lecteur dédié à aucun bout**. Côté enfant,
`Canal::commander` écrit puis lit sa réponse sur le fil qui l'appelle. Côté
capteur, un fil unique boucle : lire une commande → la faire exécuter → écrire
la réponse.

**Le `WindowsSource` reste strictement mono-fil.** Il porte des objets COM et
n'est pas `Sync` : le fil B ne le touche jamais. Il transmet la commande au fil
de fenêtre par un `mpsc::channel`, et attend la réponse sur un second — c'est
le seul point de synchronisation, et il est déjà la forme employée aujourd'hui.

- [ ] **Step 1 : Le protocole gagne un message d'identité**

Dans `agent/src/capteur/protocole.rs`, ajouter à `VersCapteur` :

```rust
    /// Première et **unique** trame de la connexion média : elle apparie ce
    /// second tube à la session déjà attachée sur la connexion de commandes.
    /// Après elle, l'enfant n'écrit plus jamais sur cette connexion — c'est
    /// ce qui garantit qu'aucune lecture et écriture n'y sont concurrentes.
    Identite { session: String },
```

et son test d'aller-retour, sur le modèle exact de `une_attache_fait_l_aller_retour`.

- [ ] **Step 2 : Le capteur apparie les deux connexions**

`agent/src/capteur/serveur.rs` gagne un registre des sessions en attente de leur
connexion média :

```rust
/// Sessions attachées sur leur connexion de commandes et attendant leur
/// connexion média. Clé : l'identifiant de session.
///
/// `Mutex` et non `RefCell` : la boucle d'acceptation et les fils de commandes
/// y touchent tous deux.
static EN_ATTENTE_DE_MEDIA: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<String, std::sync::mpsc::Sender<std::fs::File>>>,
> = std::sync::OnceLock::new();
```

`accueillir` lit la première trame et **aiguille sur son type** :

- `VersCapteur::Attache { .. }` → c'est la connexion **B**. Le capteur inscrit la
  session au registre avec l'extrémité émettrice d'un `channel::<File>()`, répond
  `Attachee`, puis lance le fil de commandes. Le fil de fenêtre attend la
  connexion média sur l'extrémité réceptrice, **avec un délai borné** :
  `DELAI_CONNEXION_MEDIA = 15 s`, la même valeur que `DUREE_FENETRE_CANAL`
  côté enfant, pour qu'un enfant qui abandonne et un capteur qui renonce se
  découvrent au même moment. Le délai expiré, la session est retirée du registre
  et le fil se termine sur un `warn!`.
- `VersCapteur::Identite { session }` → c'est la connexion **A**. Le capteur
  retire l'entrée du registre et lui envoie le `File`. **Session inconnue** :
  `warn!` nommant la session, et la connexion est abandonnée — jamais un panic,
  et jamais un silence.
- toute autre trame → `warn!` et abandon, comme aujourd'hui.

> ⚠️ **Le retrait du registre doit être fait par le receveur comme par
> l'expéditeur.** Un enfant qui meurt entre ses deux connexions laisserait sinon
> une entrée éternelle. Le fil de fenêtre retire sa propre entrée quand son
> attente expire.

- [ ] **Step 3 : Le fil de fenêtre n'écrit plus que des images**

`agent/src/capteur/fenetre.rs` : `servir_une_fenetre` reçoit désormais **deux**
écrivains distincts — celui de A pour les images et l'état, et un canal de
réponses vers le fil B pour les commandes. Sa boucle ne change pas de forme :
commandes en attente (`try_recv`), puis une image, puis l'état au changement.

**Ce qui change** : la réponse à une commande ne part plus par une écriture
directe, mais par un `Sender<DepuisCapteur>` vers le fil B, qui l'écrit sur sa
propre connexion. Le fil de fenêtre n'écrit **que** sur A.

- [ ] **Step 4 : L'enfant ouvre deux tubes, et `commander` ne passe plus par un canal**

`agent/src/capteur/tube.rs` : `attacher_sur` devient une séquence à deux temps.

1. Ouvrir B, écrire `Attache`, lire `Attachee` — **sur le fil appelant**, sans
   aucun fil lecteur.
2. Ouvrir A, écrire `Identite { session }`, et **ne plus jamais y écrire**.
   Lancer sur A le fil répartiteur, qui ne fait que lire.

`CanalTube` porte donc deux objets : `commandes: Mutex<std::fs::File>` (B, lu et
écrit par le seul `commander`) et rien de plus pour A — le répartiteur en est
propriétaire.

`Canal::commander` devient une écriture suivie d'une lecture sur B, sur le fil
appelant :

```rust
    fn commander(&mut self, message: VersCapteur) -> Result<DepuisCapteur> {
        let mut commandes = self
            .commandes
            .lock()
            .unwrap_or_else(|empoisonne| empoisonne.into_inner());
        // Écriture PUIS lecture sur le même fil : c'est la discipline qui
        // rend le blocage impossible. Ne jamais introduire de fil lecteur
        // sur cette connexion — voir le §Task 10 du plan.
        ecrire_json(&mut *commandes, &message)?;
        commandes.flush()?;
        match lire_trame(&mut *commandes).context("réponse du capteur")? {
            Trame::Json(octets) => Ok(serde_json::from_slice(&octets)?),
            Trame::Image(_) => bail!("le capteur a répondu une image à une commande"),
        }
    }
```

> ⚠️ **`recv_timeout` disparaît, et avec lui la borne de `DELAI_COMMANDE`.** Une
> lecture bloquante sur B n'a plus de délai : un capteur mort pendant une
> commande figerait la boucle de transport de cet enfant. **Poser
> `SetNamedPipeHandleState` avec un délai, ou `SO_RCVTIMEO`-équivalent, n'existe
> pas pour les tubes** — la parade retenue est que le capteur ferme ses tubes en
> mourant (le job object garantit sa mort, et la fermeture des handles avec),
> ce qui fait rendre une erreur à la lecture plutôt que de la suspendre.
> **À vérifier explicitement à la recette** : tuer le capteur pendant que des
> commandes circulent, et constater que `commander` rend une erreur.

`rattacher` refait la séquence complète des deux connexions et remplace
`self.commandes`.

- [ ] **Step 5 : Vérifier et committer**

Run: `cd agent && cargo test` puis `cd agent && cargo check --target x86_64-pc-windows-gnu`
Expected: tous les tests passent (les tests neufs du protocole compris), sortie 0, aucun avertissement dans les fichiers touchés.

Contrôle de dette : `agent/src/capteur/distante.rs` est à **487 lignes, marge 13** — cette tâche ne doit pas y toucher. `tube.rs` était à 270.

```bash
git add agent/src/capteur/protocole.rs agent/src/capteur/serveur.rs \
        agent/src/capteur/fenetre.rs agent/src/capteur/tube.rs
git commit -m "fix(d4): une connexion par sens, pour qu'aucun objet fichier ne porte lecture et ecriture"
```

---

### Task 11 : Rejouer la recette

Identique à la tâche 9, sur le binaire de la tâche 10, **avec quatre corrections
de protocole que la recette précédente a elle-même identifiées** :

1. **Une source qui bouge.** La recette 9 a employé le Bloc-notes, immobile :
   Desktop Duplication n'émet une trame qu'au changement du bureau. Employer une
   application qui redessine (une horloge, une vidéo, une fenêtre dont le contenu
   change), ou animer la fenêtre.
2. **Vérifier que la mise à mort du capteur pendant une commande** fait rendre
   une erreur à `commander` plutôt que de le suspendre (voir le §Step 4).
3. **Contrôler la survie de la VM après chaque rang**, pas seulement à la fin :
   la recette 9 en a perdu une exécution.
4. **Le « avant » du critère 3** se joue sur `7d7e254`, dernier commit où
   l'enfant capture lui-même. S'il n'est pas joué, l'écrire.

Le document de résultats de la tâche 9 est **amendé**, pas réécrit : il porte
déjà le diagnostic du défaut, qui reste vrai et qui est le fait le plus utile de
ce sous-bloc.

---

## Ce que ce plan ne couvre pas, et pourquoi

- **Le partage de la capacité réseau entre N flux** (D6) : chaque enfant garde sa
  propre `RTCPeerConnection` donc sa propre estimation d'un lien partagé. Dette
  nommée par le chantier C volet 1, hors périmètre ici.
- **L'audio par fenêtre** (D6) : une seule fenêtre porte le son, inchangé.
- **La mise en sommeil des fenêtres masquées** (D5) : c'est ce que le plafond
  d'encodeurs rendra nécessaire au-delà de huit.
- **La latence bout en bout** : le critère 3 relève la cadence, pas la latence —
  celle-ci exige un instrument à construire.
- **L'identification de la couche du plafond de 4 processus** : D4 le contourne,
  il ne l'explique pas.
