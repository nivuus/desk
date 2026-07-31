//! Description des mires de la sonde multi-fenêtres : la couleur qu'une
//! fenêtre doit peindre, et le verdict rendu sur un pixel lu dans une image
//! capturée.
//!
//! Portable à dessein, comme `geometry.rs` : c'est le juge de la porte
//! éliminatoire du banc (« la fenêtre recouverte rend-elle toujours sa mire »).
//! Un juge cassé et un banc qui ne trouve rien produisent le même silence —
//! d'où les tests, exécutés sur l'hôte Linux.

/// Nombre maximal de mires simultanées — la cible du banc.
pub const MIRES_MAX: u8 = 8;

/// Rouge de la mire n°0. Non nul : un rouge à 0 se confondrait avec du noir
/// sur une lecture bruitée.
const BASE_IDENTITE: u8 = 16;
/// Écart de rouge entre deux mires voisines. Très au-delà de la tolérance :
/// confondre deux mires ferait passer la porte éliminatoire à une voie qui
/// capture la mauvaise fenêtre, exactement le défaut recherché.
const PAS_IDENTITE: u8 = 24;
/// Bleu commun à toutes les mires : signe qu'on lit bien une mire.
const BLEU_MIRE: u8 = 96;
/// Vert des trames paires et impaires. L'alternance rend l'animation
/// détectable, et Desktop Duplication n'émet une image que si le bureau change.
const VERT_PAIR: u8 = 32;
const VERT_IMPAIR: u8 = 224;
/// Écart toléré par canal sur un pixel lu.
const TOLERANCE: i16 = 4;
/// En deçà, sur les trois canaux, l'image est tenue pour noire.
const SEUIL_NOIR: u8 = 12;

/// Ce qu'un pixel lu dit de la fenêtre attendue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// La mire attendue, à la tolérance près.
    Juste,
    /// La mire d'une AUTRE fenêtre : la voie capture le mauvais contenu.
    Voisine(u8),
    /// Image noire : la voie ne rend rien du contenu.
    Noire,
    /// Ni une mire, ni du noir.
    Inconnue,
}

/// Couleur que la mire `id` doit peindre à la trame `trame`, en `(r, g, b)`.
pub fn couleur_mire(id: u8, trame: u64) -> (u8, u8, u8) {
    debug_assert!(id < MIRES_MAX);
    let rouge = BASE_IDENTITE + id * PAS_IDENTITE;
    let vert = if trame % 2 == 0 { VERT_PAIR } else { VERT_IMPAIR };
    (rouge, vert, BLEU_MIRE)
}

fn proche(valeur: u8, attendu: u8) -> bool {
    (valeur as i16 - attendu as i16).abs() <= TOLERANCE
}

/// Identifie la fenêtre dont ce pixel porte la mire, s'il en porte une.
pub fn identifier(pixel: (u8, u8, u8)) -> Option<u8> {
    let (rouge, vert, bleu) = pixel;
    if !proche(bleu, BLEU_MIRE) {
        return None;
    }
    if !proche(vert, VERT_PAIR) && !proche(vert, VERT_IMPAIR) {
        return None;
    }
    let ecart = rouge as i16 - BASE_IDENTITE as i16;
    if ecart < 0 {
        return None;
    }
    let id = ecart / PAS_IDENTITE as i16;
    // Le reste doit tomber sur un multiple exact du pas, à la tolérance près :
    // sans cette vérification, toute nuance de rouge serait attribuée à une
    // mire par simple division.
    if (ecart - id * PAS_IDENTITE as i16).abs() > TOLERANCE || id >= MIRES_MAX as i16 {
        return None;
    }
    Some(id as u8)
}

/// Juge un pixel lu contre la mire attendue.
pub fn verdict(attendu: u8, pixel: (u8, u8, u8)) -> Verdict {
    let (rouge, vert, bleu) = pixel;
    if rouge < SEUIL_NOIR && vert < SEUIL_NOIR && bleu < SEUIL_NOIR {
        return Verdict::Noire;
    }
    match identifier(pixel) {
        Some(id) if id == attendu => Verdict::Juste,
        Some(id) => Verdict::Voisine(id),
        None => Verdict::Inconnue,
    }
}

/// Quelle voie est contrôlée au tour `tour`, parmi `nombre` voies.
///
/// Le montage multi-sorties supprime le recouvrement — une fenêtre par
/// sortie, rien ne peut en cacher une autre — donc la porte éliminatoire du
/// banc mono-sortie n'a plus d'objet. Le risque devient l'appariement : que la
/// voie *i* capture en réalité la sortie *j*, ou du noir.
///
/// Contrôler les N voies à chaque tour le détecterait, mais ferait croître le
/// coût CPU du contrôle avec N : la cadence relevée à N=8 intégrerait huit
/// fois ce coût et ne serait comparable à rien — l'erreur déjà payée au
/// chantier précédent, où la portée de la lecture de pixel a changé en cours
/// de route. La rotation couvre toutes les voies pour **une** lecture par
/// tour, quel que soit N.
pub fn voie_controlee(tour: u64, nombre: usize) -> Option<usize> {
    if nombre == 0 {
        return None;
    }
    Some((tour % nombre as u64) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_couleur_d_une_mire_identifie_sa_fenetre() {
        for id in 0..MIRES_MAX {
            assert_eq!(identifier(couleur_mire(id, 0)), Some(id));
            assert_eq!(identifier(couleur_mire(id, 1)), Some(id));
        }
    }

    #[test]
    fn l_alternance_de_trame_change_le_vert_sans_toucher_a_l_identite() {
        let paire = couleur_mire(3, 10);
        let impaire = couleur_mire(3, 11);
        assert_ne!(paire.1, impaire.1, "sans alternance visible, Desktop Duplication n'émet rien");
        assert_eq!(paire.0, impaire.0);
        assert_eq!(identifier(impaire), Some(3));
    }

    #[test]
    fn la_mire_d_une_fenetre_voisine_est_rejetee() {
        // Le cas exact que la porte éliminatoire doit attraper : la capture
        // d'une fenêtre recouverte rend le contenu de celle du dessus.
        assert_eq!(verdict(3, couleur_mire(4, 0)), Verdict::Voisine(4));
    }

    #[test]
    fn une_image_noire_est_rejetee() {
        // PrintWindow sur une fenêtre D3D rend typiquement du noir : c'est un
        // échec de voie, pas une mire inconnue.
        assert_eq!(verdict(0, (0, 0, 0)), Verdict::Noire);
    }

    #[test]
    fn un_ecart_de_lecture_dans_la_tolerance_reste_juste() {
        let (r, g, b) = couleur_mire(5, 0);
        assert_eq!(verdict(5, (r + 2, g + 2, b + 2)), Verdict::Juste);
    }

    #[test]
    fn une_couleur_etrangere_est_inconnue() {
        // Le fond du bureau, une console PowerShell : ni une mire, ni du noir.
        assert_eq!(verdict(0, (255, 255, 255)), Verdict::Inconnue);
        assert_eq!(identifier((1, 36, 86)), None);
    }

    /// La propriété qui compte : sur k·N tours, chaque voie est contrôlée
    /// exactement k fois. Un contrôle qui favoriserait une voie laisserait
    /// les autres non couvertes, et c'est précisément l'appariement croisé
    /// entre sorties que ce montage doit détecter.
    #[test]
    fn la_rotation_controle_chaque_voie_le_meme_nombre_de_fois() {
        for nombre in 1..=8usize {
            let mut comptes = vec![0usize; nombre];
            for tour in 0..(nombre as u64 * 7) {
                let voie = voie_controlee(tour, nombre).expect("nombre non nul");
                comptes[voie] += 1;
            }
            assert!(
                comptes.iter().all(|compte| *compte == 7),
                "nombre = {nombre}, comptes = {comptes:?}"
            );
        }
    }

    #[test]
    fn la_rotation_ne_designe_jamais_une_voie_inexistante() {
        for nombre in 1..=8usize {
            for tour in 0..100u64 {
                let voie = voie_controlee(tour, nombre).expect("nombre non nul");
                assert!(voie < nombre, "voie {voie} hors des {nombre} voies");
            }
        }
    }

    /// Zéro voie n'est pas une erreur d'appelant à signaler par panique : le
    /// banc doit pouvoir demander sans savoir, et ne rien contrôler.
    #[test]
    fn sans_voie_il_n_y_a_rien_a_controler() {
        assert_eq!(voie_controlee(0, 0), None);
        assert_eq!(voie_controlee(42, 0), None);
    }
}
