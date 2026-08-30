//! Sur QUEL rectangle les coordonnées du navigateur se démappent.
//!
//! 🔴 **LE DÉFAUT QUE CE MODULE EXISTE POUR FERMER (mesuré le 30 août 2026).**
//! Le client normalise ses coordonnées sur **l'image qu'il reçoit**
//! (`client/src/input.ts` : « normalisées sur 0..65535 par rapport à la zone
//! d'image », fraction du `<video>`). L'agent, lui, les appliquait à la **zone
//! client de la FENÊTRE**. Les deux ne parlaient plus du même rectangle depuis
//! que la capture est passée en `ModeCapture::SortieEntiere` (sous-bloc D10) :
//! l'image est **la sortie entière**, fond d'écran et barre des tâches
//! compris.
//!
//! **L'erreur se calcule**, et elle a deux termes — c'est pourquoi elle ne se
//! corrige pas par un décalage :
//!
//! ```text
//! erreur(f) = (Wx − Ox) + f·(Ww − Ow)
//!             └ ORIGINE ┘   └ ÉCHELLE ┘
//! ```
//!
//! Relevé sur la machine du propriétaire : la fenêtre à `+4428+51` pour une
//! sortie visée à `+3140+0`, soit **+1288 en x et +51 en y** de terme
//! d'origine — et un terme d'échelle non nul, puisque la barre des tâches est
//! visible dans l'image (`Oh > Wh`) et que le redimensionnement est
//! **délibérément ignoré** en `SortieEntiere`.
//!
//! 🔵 **Le clavier n'était pas touché**, et ce n'est pas une coïncidence : il
//! ne porte aucune coordonnée. Le partage clavier/souris est **prédit** par
//! cette explication.
//!
//! ## 🔴 UNE SEULE SOURCE, ET C'EST TOUT L'OBJET DE CE MODULE
//!
//! Ce défaut est né parce que **deux endroits décrivaient le même rectangle**
//! et qu'un seul a suivi D10. Une correction qui laisserait subsister deux
//! descriptions indépendantes se redéferait au prochain changement de mode.
//!
//! **`config.sortie_dxgi` est déjà l'unique discriminant du mode de capture**
//! (`demarrage/source.rs` : `Some(nom)` ⇒ source distante servie par le
//! capteur, sortie entière ; `None` ⇒ capture locale de la fenêtre recadrée).
//! **La référence des entrées en dérive désormais, de la même valeur** — il
//! n'y a plus rien à tenir d'accord.

/// Le rectangle sur lequel démapper les coordonnées reçues du navigateur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reference {
    /// La zone client de la fenêtre : ce que la capture montre quand elle
    /// recadre la fenêtre (`ModeCapture::FenetreRecadree`, chemin
    /// mono-fenêtre).
    ZoneClientDeLaFenetre,
    /// La sortie DXGI **entière**, désignée par son nom : ce que la capture
    /// montre en multi-fenêtres (`ModeCapture::SortieEntiere`).
    SortieCapturee(String),
}

/// La référence, **dérivée du même discriminant que le mode de capture**.
///
/// ⚠️ **Ne jamais rétablir un second critère ici.** Si un jour le mode de
/// capture cesse de se lire dans `config.sortie_dxgi`, c'est CETTE fonction
/// qu'il faut suivre — pas une seconde condition écrite ailleurs.
pub fn reference(sortie_dxgi: Option<&str>) -> Reference {
    match sortie_dxgi {
        Some(nom) => Reference::SortieCapturee(nom.to_string()),
        None => Reference::ZoneClientDeLaFenetre,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{to_virtual_desktop, Rect};

    #[test]
    fn sans_sortie_nommee_la_reference_est_la_fenetre() {
        assert_eq!(reference(None), Reference::ZoneClientDeLaFenetre);
    }

    #[test]
    fn avec_une_sortie_nommee_la_reference_est_la_sortie() {
        assert_eq!(
            reference(Some("\\\\.\\DISPLAY7")),
            Reference::SortieCapturee("\\\\.\\DISPLAY7".into())
        );
    }

    /// 🔴 **LA ROUGE, ET ELLE PORTE LES CHIFFRES DU RELEVÉ.**
    ///
    /// Le montage est celui de la machine du propriétaire : la fenêtre est à
    /// `+4428+51` (relevé de `placement_periodique`), la sortie capturée à
    /// `+3140+0`. Un clic au coin **haut-gauche** de l'image — donc à
    /// l'origine de la SORTIE — doit atterrir sur `(3140, 0)`.
    ///
    /// La formule d'AVANT, qui démappe sur la fenêtre, rend `(4428, 51)` :
    /// **+1288 en x et +51 en y**, exactement le décalage dérivé du journal.
    #[test]
    fn la_formule_d_avant_rend_le_decalage_releve_de_1288_et_51() {
        let bureau = Rect { x: 0, y: 0, width: 8192, height: 2160 };
        let fenetre = Rect { x: 4428, y: 51, width: 1428, height: 1080 };
        let sortie = Rect { x: 3140, y: 0, width: 1920, height: 1200 };

        let vise = |r: Rect| {
            let (nx, ny) = to_virtual_desktop(0, 0, r, bureau);
            (
                (nx as f64 / 65535.0 * bureau.width as f64).round() as i32,
                (ny as f64 / 65535.0 * bureau.height as f64).round() as i32,
            )
        };
        let (juste_x, juste_y) = vise(sortie);
        let (faux_x, faux_y) = vise(fenetre);
        assert_eq!((juste_x, juste_y), (3140, 0), "la référence JUSTE vise l'origine de la sortie");
        assert_eq!(faux_x - juste_x, 1288, "le terme d'ORIGINE en x, relevé sur la VM");
        assert_eq!(faux_y - juste_y, 51, "le terme d'ORIGINE en y, relevé sur la VM");
    }

    /// 🔴 **Le second terme, celui que le décalage constant ne corrigerait
    /// pas.** Au coin BAS-DROITE, l'écart n'est plus le même qu'au coin
    /// haut-gauche : c'est le terme d'ÉCHELLE, et c'est pourquoi ce défaut ne
    /// se répare pas en soustrayant 1288.
    #[test]
    fn l_erreur_n_est_pas_un_simple_decalage_elle_croit_avec_la_distance() {
        let bureau = Rect { x: 0, y: 0, width: 8192, height: 2160 };
        let fenetre = Rect { x: 4428, y: 51, width: 1428, height: 1080 };
        let sortie = Rect { x: 3140, y: 0, width: 1920, height: 1200 };
        let ecart = |f: u16| {
            let en_px = |r: Rect| {
                let (nx, _) = to_virtual_desktop(f, 0, r, bureau);
                (nx as f64 / 65535.0 * bureau.width as f64).round() as i32
            };
            en_px(fenetre) - en_px(sortie)
        };
        let au_coin = ecart(0);
        let au_bout = ecart(65535);
        assert_eq!(au_coin, 1288);
        assert_ne!(au_bout, au_coin, "l'écart CHANGE : ce n'est pas un décalage constant");
        // Ow − Ww = 1920 − 1428 = 492 : l'écart se réduit d'autant au bout.
        assert_eq!(au_coin - au_bout, 492);
    }

    /// Le cas où les deux références COÏNCIDENT : la fenêtre occupe exactement
    /// sa sortie. C'est le témoin — il montre que la correction ne change
    /// rien quand il n'y avait rien à changer, et donc que la rouge ci-dessus
    /// vient bien de l'écart des rectangles.
    #[test]
    fn quand_la_fenetre_occupe_sa_sortie_les_deux_references_coincident() {
        let bureau = Rect { x: 0, y: 0, width: 8192, height: 2160 };
        let meme = Rect { x: 3140, y: 0, width: 1920, height: 1200 };
        for f in [0u16, 12345, 65535] {
            assert_eq!(to_virtual_desktop(f, f, meme, bureau), to_virtual_desktop(f, f, meme, bureau));
        }
        let (nx, ny) = to_virtual_desktop(0, 0, meme, bureau);
        assert_eq!(
            (
                (nx as f64 / 65535.0 * bureau.width as f64).round() as i32,
                (ny as f64 / 65535.0 * bureau.height as f64).round() as i32
            ),
            (3140, 0)
        );
    }
}
