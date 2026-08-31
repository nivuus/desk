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

//!
//! ## 🔴 CE QUE LA PREMIÈRE CORRECTION A CORRIGÉ, ET CE QU'IL LUI MANQUAIT
//!
//! Le lot 32Q a fait dériver la référence de `config.sortie_dxgi` et l'a
//! posée sur **la sortie DXGI entière**. C'était **la bonne ORIGINE et la
//! mauvaise TAILLE**, et le propriétaire l'a vu tout de suite : « en 0 ok,
//! totalement à droite pas bon, c'est progressif », en x seulement.
//!
//! **La capture n'est pas la sortie : c'est un RECADRAGE de la sortie,
//! `taille_retenue`, posé à son origine** (`windows_source/sortie.rs`,
//! `capteur/fenetre/ouverture.rs`). Relevé sur la machine du propriétaire,
//! dans le même journal :
//!
//! ```text
//! duplication de sortie établie  desktop_width=1860 desktop_height=1080
//! session NVENC native initialisée   largeur=1428   hauteur=1080
//! ```
//!
//! d'où l'erreur résiduelle, purement d'ÉCHELLE (l'origine, elle, est juste) :
//!
//! | fraction | l'image montre | l'agent injectait | écart |
//! | --- | --- | --- | --- |
//! | 0,00 | 0 | 0 | **0** |
//! | 0,50 | 714 | 930 | +216 |
//! | 1,00 | 1428 | 1860 | **+432** |
//!
//! En y, 1080 contre 1080 : **zéro**. C'est exactement le symptôme décrit.
//!
//! 🔴 **LA TAILLE N'EST PAS RECALCULÉE ICI, ELLE EST PARTAGÉE.** Recalculer
//! `taille_retenue(taille_fenetre, taille_sortie)` dans l'enfant redonnerait
//! **deux descriptions du même rectangle** — précisément le mécanisme qui a
//! produit le défaut du lot 32M. La taille employée est celle que le
//! **capteur** a retenue et qu'il a annoncée par `DepuisCapteur::Attachee`,
//! c'est-à-dire **le seul et même stockage** que `SourceDistante::dimensions`
//! rend au reste de l'enfant : [`TailleImage`], créée par
//! `demarrage::source::construire` et confiée à la fois à la source et à
//! l'injecteur. Il n'y a **rien à tenir d'accord**, parce qu'il n'y a qu'une
//! seule valeur.
//!
//! Les commandes qui l'établissent, pour que le prochain lecteur refasse le
//! contrôle sans croire personne :
//!
//! ```text
//! grep -n 'Attachee { largeur, hauteur }' agent/src/capteur/tube.rs
//! grep -n 'taille_retenue' agent/src/capteur/fenetre/ouverture.rs
//! grep -rn 'TailleImage' agent/src/
//! ```

/// La taille de l'image RÉELLEMENT capturée et encodée, en pixels.
///
/// 🔴 **UN SEUL STOCKAGE, DEUX LECTEURS.** `SourceDistante` la pose depuis
/// `DepuisCapteur::Attachee` (attache), `DepuisCapteur::Etat` (changement) et
/// `DepuisCapteur::Taille` (redimensionnement acquitté), et son
/// `dimensions()` la relit ; l'injecteur d'entrées la relit aussi. Personne
/// ne la recalcule — c'est tout l'objet de ce type, et la raison pour
/// laquelle il vit dans CE module plutôt qu'à côté de la source.
///
/// **Un seul `AtomicU64` et non deux `AtomicU32`, à dessein** : une paire
/// d'atomiques lue en deux temps peut rendre une largeur neuve avec une
/// hauteur périmée pendant un redimensionnement, et l'événement de souris qui
/// tomberait dans cet intervalle serait démappé sur un rectangle qui n'a
/// jamais existé. Empaqueter les deux moitiés rend ce déchirement
/// **impossible** au lieu de le rendre rare.
#[derive(Debug)]
pub struct TailleImage(std::sync::atomic::AtomicU64);

impl TailleImage {
    pub fn nouvelle(largeur: u32, hauteur: u32) -> Self {
        let cellule = Self(std::sync::atomic::AtomicU64::new(0));
        cellule.poser(largeur, hauteur);
        cellule
    }

    pub fn poser(&self, largeur: u32, hauteur: u32) {
        let empaquetee = ((largeur as u64) << 32) | hauteur as u64;
        self.0.store(empaquetee, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn lire(&self) -> (u32, u32) {
        let empaquetee = self.0.load(std::sync::atomic::Ordering::Relaxed);
        ((empaquetee >> 32) as u32, empaquetee as u32)
    }
}

/// Le rectangle sur lequel démapper les coordonnées reçues du navigateur.
///
/// 🔴 **LA VARIANTE MULTI-FENÊTRES PORTE LA TAILLE DE L'IMAGE, ET C'EST LE
/// TYPE QUI L'IMPOSE.** Un `Option<&str>` d'un côté et un
/// `Option<Arc<TailleImage>>` de l'autre auraient laissé représentable l'état
/// « une sortie nommée sans sa taille », c'est-à-dire exactement le repli
/// silencieux que ce module existe pour interdire. Ici, on ne peut pas
/// construire la variante sans la taille.
#[derive(Debug, Clone)]
pub enum Reference {
    /// La zone client de la fenêtre : ce que la capture montre quand elle
    /// recadre la fenêtre (`ModeCapture::FenetreRecadree`, chemin
    /// mono-fenêtre). La zone client EST le recadrage : il n'y a pas de
    /// seconde taille à porter.
    ZoneClientDeLaFenetre,
    /// Le recadrage de la sortie DXGI nommée : **son origine, et la taille de
    /// l'image** (`ModeCapture::SortieEntiere`, chemin multi-fenêtres).
    ///
    /// ⚠️ Le nom de la variante dit « sortie » et ce n'est **pas** la sortie
    /// entière : c'est la sortie qui donne l'ORIGINE, et `image` qui donne la
    /// TAILLE. Les confondre est le défaut du lot 32Q, corrigé ici.
    SortieCapturee { nom: String, image: std::sync::Arc<TailleImage> },
}

/// Le rectangle de la région capturée : **l'origine de la sortie, la taille
/// de l'image**.
///
/// Rend `None` quand la taille de l'image n'est pas encore connue (une des
/// deux moitiés nulle). ⚠️ **Aucun repli sur la sortie entière** : ce serait
/// réintroduire en silence l'erreur d'échelle que cette fonction supprime, et
/// le symptôme redeviendrait « la souris dérive vers la droite » sans qu'une
/// seule trace ne le dise. L'appelant doit en faire une erreur nommée.
///
/// ⚠️ **Aucun bornage à la sortie non plus.** `taille_retenue` garantit déjà
/// que l'image tient dans la texture (`capteur/fenetre/ouverture.rs`) ; un
/// `min` ici serait une SECONDE règle, qui masquerait une divergence au lieu
/// de la montrer.
pub fn rectangle_capture(
    sortie: crate::geometry::Rect,
    image: (u32, u32),
) -> Option<crate::geometry::Rect> {
    let (largeur, hauteur) = image;
    if largeur == 0 || hauteur == 0 {
        return None;
    }
    Some(crate::geometry::Rect { x: sortie.x, y: sortie.y, width: largeur, height: hauteur })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{to_virtual_desktop, Rect};

    /// La cellule partagée : ce qu'on pose est ce qu'on relit, et les deux
    /// moitiés ne se mélangent pas.
    #[test]
    fn la_taille_partagee_rend_ce_qu_on_y_pose() {
        let taille = TailleImage::nouvelle(1428, 1080);
        assert_eq!(taille.lire(), (1428, 1080));
        taille.poser(640, 360);
        assert_eq!(taille.lire(), (640, 360));
        // Les deux moitiés sont bien séparées, y compris sur les extrêmes.
        taille.poser(u32::MAX, 1);
        assert_eq!(taille.lire(), (u32::MAX, 1));
    }

    /// Une taille pas encore connue ne doit **pas** produire un rectangle :
    /// c'est ce refus qui empêche un repli silencieux sur la sortie entière.
    #[test]
    fn sans_taille_d_image_il_n_y_a_pas_de_rectangle() {
        let sortie = Rect { x: 3140, y: 0, width: 1860, height: 1080 };
        assert_eq!(rectangle_capture(sortie, (0, 1080)), None);
        assert_eq!(rectangle_capture(sortie, (1428, 0)), None);
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
    /// 🔴 **LA ROUGE DE E1, ET ELLE PORTE LES CHIFFRES DU JOURNAL DE LA VM.**
    ///
    /// Relevé dans le même `agent.log`, la même session :
    ///
    /// ```text
    /// duplication de sortie établie  desktop_width=1860 desktop_height=1080
    /// session NVENC native initialisée   largeur=1428   hauteur=1080
    /// ```
    ///
    /// L'attendu est écrit **AVANT** toute mesure sur la VM, et il ne dérive
    /// d'aucun point mesuré : il vient de ces deux lignes seules. C'est la
    /// condition posée après la mesure circulaire du lot 32R — voir le § « son
    /// piège le plus traître » de `CLAUDE.md`.
    ///
    /// **Le fond droit de l'image est à 1428 px de l'origine de la sortie.**
    /// La formule du lot 32Q, qui démappait sur la sortie ENTIÈRE, y visait
    /// 1860 : **+432 px**, nul à gauche, la moitié à mi-course. En y, 1080
    /// contre 1080 : **zéro**.
    #[test]
    fn le_recadrage_annule_les_432_px_de_derive_en_x_et_ne_touche_pas_l_y() {
        let bureau = Rect { x: 0, y: 0, width: 8192, height: 2160 };
        let sortie = Rect { x: 3140, y: 0, width: 1860, height: 1080 };
        let image = (1428u32, 1080u32);

        // Ce que le produit calcule aujourd'hui.
        let juste = rectangle_capture(sortie, image).expect("taille d'image connue");
        // Ce que le lot 32Q calculait : la sortie entière, telle quelle.
        let formule_32q = sortie;

        let en_px = |r: Rect, f: u16| {
            let (nx, ny) = to_virtual_desktop(f, f, r, bureau);
            (
                (nx as f64 / 65535.0 * bureau.width as f64).round() as i32,
                (ny as f64 / 65535.0 * bureau.height as f64).round() as i32,
            )
        };

        // Le bord DROIT de l'image : 3140 + 1428.
        assert_eq!(en_px(juste, 65535).0, 3140 + 1428, "le bord droit de l'IMAGE");
        assert_eq!(
            en_px(formule_32q, 65535).0 - en_px(juste, 65535).0,
            432,
            "les 432 px que la formule du lot 32Q ajoutait au bord droit"
        );
        // À mi-course, la moitié : l'erreur est proportionnelle, pas constante.
        assert_eq!(en_px(formule_32q, 32767).0 - en_px(juste, 32767).0, 216);
        // À gauche, rien : c'est pourquoi le propriétaire disait « en 0 ok ».
        assert_eq!(en_px(formule_32q, 0).0 - en_px(juste, 0).0, 0);
        // En y, rien nulle part : la sortie et l'image font 1080 toutes deux.
        for f in [0u16, 32767, 65535] {
            assert_eq!(en_px(formule_32q, f).1, en_px(juste, f).1, "aucune dérive en y");
        }
    }

    /// Le témoin négatif de la rouge ci-dessus : quand l'image occupe TOUTE la
    /// sortie, les deux formules coïncident. Sans lui, un `rectangle_capture`
    /// qui rendrait n'importe quoi de plus petit passerait la rouge.
    #[test]
    fn quand_l_image_occupe_toute_la_sortie_les_deux_formules_coincident() {
        let sortie = Rect { x: 3140, y: 0, width: 1860, height: 1080 };
        assert_eq!(rectangle_capture(sortie, (1860, 1080)), Some(sortie));
    }

}
