//! Découpe d'une plage de lecture en trames bornées. **PUR** : aucun `cfg`,
//! testé isolément sur l'hôte.
//!
//! ⚠️ **C'est le module où vivent les erreurs d'unité, et c'est pour cela
//! qu'il est pur et testé à part** (spec §7.3). Le critère (2) de la recette
//! F1 — le condensat SHA-256 du fichier lu à travers le lecteur — est
//! exactement ce que ce module peut faire échouer : un fichier tronqué d'une
//! trame, des plages dans le désordre, un recouvrement qui duplique des
//! octets. Aucun de ces trois défauts ne se voit à l'œil sur un fichier
//! texte ; tous les trois cassent le condensat.
//!
//! ⚠️ **Le piège d'unité que le type impose** : `PRJ_GET_FILE_DATA_CB` reçoit
//! un `byteoffset: u64` et une `length: u32`. La position est donc sur 64 bits
//! — un fichier peut dépasser 4 Gio — mais chaque longueur de morceau tient
//! sur 32 bits. La signature de [`decouper`] prend une longueur en `u64` et
//! rend des longueurs en `u32` : c'est la conversion qui déborderait si `max`
//! n'était pas lui-même borné, et le test la couvre.

/// Une plage contiguë à demander au navigateur en une seule trame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Morceau {
    pub position: u64,
    pub longueur: u32,
}

/// Découpe `[position, position + longueur)` en morceaux d'au plus `max`
/// octets, contigus et croissants.
///
/// Une longueur nulle ne produit **aucun** morceau : un morceau vide
/// provoquerait une trame de réponse vide que rien ne distinguerait d'une fin
/// de fichier.
///
/// # Panique
///
/// Si `max` vaut 0 — une découpe en morceaux de zéro octet ne se termine pas.
/// C'est une erreur de programmation de l'appelant, pas un cas d'exécution :
/// `max` est une constante du pont, jamais une valeur reçue du réseau.
pub fn decouper(position: u64, longueur: u64, max: usize) -> Vec<Morceau> {
    assert!(max > 0, "une découpe en morceaux de zéro octet ne se termine pas");
    // `max` est borné à `u32::MAX` avant toute conversion : c'est ici que le
    // débordement se produirait sur une cible 64 bits, où `usize` est plus
    // large que `u32`.
    let max = max.min(u32::MAX as usize) as u64;
    let mut morceaux = Vec::new();
    let mut reste = longueur;
    let mut curseur = position;
    while reste > 0 {
        let prise = reste.min(max);
        morceaux.push(Morceau {
            position: curseur,
            // `prise <= max <= u32::MAX` : la conversion ne peut pas déborder,
            // et c'est le `min` ci-dessus qui le garantit, pas un espoir.
            longueur: prise as u32,
        });
        curseur += prise;
        reste -= prise;
    }
    morceaux
}

#[cfg(test)]
mod tests;
