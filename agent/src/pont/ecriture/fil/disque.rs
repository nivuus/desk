//! Ce que le fil d'écriture fait du DISQUE : résoudre un chemin, lire un
//! morceau, ajouter une ligne au journal, le compacter.
//!
//! **Extrait de [`super`] à 494 lignes pour une porte de 500, marge 6.**
//!
//! ⚠️ **Le plafond n'a PAS été franchi, et cette extraction n'est donc pas un
//! rattrapage** — mais une marge de six est intenable : le sous-bloc **F3**
//! doit toucher ce fichier (ses deux règles d'entrelacement — drainer avant un
//! renommage, oublier sur une suppression), et ce dépôt a payé quatre fois la
//! leçon « la marge regagnée par une extraction se reperd à la ronde suivante
//! si on la traite comme acquise ». **Jamais une compression**, que
//! `CLAUDE.md` interdit nommément et que D9 a dû défaire deux fois.
//!
//! **Le découpage est par NATURE, pas par taille** : la machine à états reste
//! chez [`super`], tout ce qui touche un système de fichiers vient ici. C'est
//! le même geste que `service/verbes.rs`, qui porte les `unsafe` là où
//! `service.rs` porte la boucle.
//!
//! 🔵 **Tout ce fichier est PUR** : un `std::fs` ordinaire, portable, éprouvé
//! sur l'hôte par les tests de [`super`]. Après
//! `FILE_HANDLE_CLOSED_FILE_MODIFIED`, le fichier est **complet** dans la
//! racine — ProjFS n'appelle `GetFileData` que sur un **substitut**.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::pont::decoupe::Morceau;
use crate::pont::journal::Journal;

/// Le chemin local d'un chemin logique du protocole.
///
/// ⚠️ **Composant par composant, jamais par `join` d'une chaîne entière** : un
/// chemin logique porte des `/`, que `Path::join` interpréterait comme un
/// chemin ABSOLU sur un `/foo`. La normalisation de `pont::chemins` a déjà
/// refusé les `..` et les `:` en amont — c'est elle la barrière, pas ceci.
pub(super) fn local(racine: &Path, chemin: &str) -> PathBuf {
    let mut local = racine.to_path_buf();
    for composant in chemin.split('/').filter(|c| !c.is_empty()) {
        local.push(composant);
    }
    local
}

/// La taille du fichier local. **Zéro pour un répertoire**, qui n'a aucun
/// octet à pousser, et zéro pour un chemin absent — le fil s'en apercevra à la
/// lecture, où l'erreur est nommable.
pub(super) fn taille_de(racine: &Path, chemin: &str) -> u64 {
    std::fs::metadata(local(racine, chemin))
        .map(|m| if m.is_dir() { 0 } else { m.len() })
        .unwrap_or(0)
}

/// Lit un morceau du fichier local.
pub(super) fn lire(racine: &Path, chemin: &str, morceau: Morceau) -> std::io::Result<Vec<u8>> {
    use std::io::{Read, Seek, SeekFrom};
    if morceau.longueur == 0 {
        // Un fichier vide : rien à lire, et le flux se ferme sur un morceau
        // sans octet. `File::open` échouerait tout de même si le fichier avait
        // disparu entre-temps, ce qu'on veut savoir.
        std::fs::File::open(local(racine, chemin))?;
        return Ok(Vec::new());
    }
    let mut fichier = std::fs::File::open(local(racine, chemin))?;
    fichier.seek(SeekFrom::Start(morceau.position))?;
    let mut tampon = vec![0u8; morceau.longueur as usize];
    let lus = fichier.read(&mut tampon)?;
    // ⚠️ **La longueur ANNONCÉE doit être celle RÉELLEMENT lue.** Le fichier a
    // pu rétrécir entre le `metadata` et la lecture ; annoncer la demande
    // ferait diverger l'en-tête de la charge, et le navigateur écrirait des
    // zéros de remplissage.
    tampon.truncate(lus);
    Ok(tampon)
}

/// Ajoute une ligne au journal, **et la vide sur le disque**.
///
/// ⚠️ **`sync_all` et non un simple `write`** : une ligne restée dans le cache
/// du système ne survit pas à un arrêt brutal, et c'est exactement le cas que
/// ce journal existe pour couvrir.
pub(super) fn ajouter(chemin_journal: &Path, ligne: &str) -> std::io::Result<()> {
    let mut fichier =
        std::fs::OpenOptions::new().create(true).append(true).open(chemin_journal)?;
    fichier.write_all(ligne.as_bytes())?;
    fichier.sync_all()
}

/// Tronque le journal **s'il est vide** et qu'il a grossi.
///
/// 🔴 **`compactable()` teste `est_vide()` ET la taille, jamais la taille
/// seule.** Tronquer un fichier qui porte encore une due perdrait la donnée
/// **exactement quand elle sert** : un journal gros est un journal où beaucoup
/// d'écritures ont échoué.
pub(super) fn compacter_si_possible(chemin_journal: &Path, journal: &Journal) {
    let Ok(meta) = std::fs::metadata(chemin_journal) else { return };
    if !journal.compactable(meta.len()) {
        return;
    }
    if let Err(erreur) = std::fs::write(chemin_journal, b"") {
        tracing::warn!(%erreur, "compactage du journal des ecritures echoue");
    } else {
        tracing::info!(octets = meta.len(), "journal des ecritures compacte (aucune due)");
    }
}
