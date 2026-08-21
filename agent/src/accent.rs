//! La couleur d'accent d'une fenêtre — sous-bloc **A1**.
//!
//! **PUR, aucun `cfg`** : tout ce fichier compile et se teste sur l'hôte Linux.
//! La moitié Windows — lire l'icône d'un `hwnd` — vit dans `accent/win32.rs`,
//! qui ne prend **aucune décision** : il rend des octets, c'est ici qu'on
//! décide.
//!
//! **Convention de module** (`CLAUDE.md`, § « Convention de module enfant ») :
//! `accent` ne préfixe aucun module de premier niveau existant, il vit donc à
//! la **racine nue** — `mod accent;` ordinaire dans `main.rs`. Son enfant
//! `win32` se déclare par un `mod` ordinaire **à l'intérieur** de lui : il n'a
//! jamais besoin de sortir de l'arbre de son parent, donc la règle du `#[path]`
//! est **hors de portée**. C'est ce que `presse_papier` fait déjà.
//!
//! ⚠️ **AUCUNE des cinq constantes de `dominante` n'est CALIBRÉE**, non plus que
//! `PERIODE_ACCENT`. Elles rejoignent la liste que ce dépôt tient depuis
//! `BPP_MIN` : aucun jugement visuel n'a été porté sur aucune, et le sous-bloc
//! A1 n'en porte pas davantage.

use std::sync::OnceLock;
use std::time::Duration;

/// La lecture Win32 de l'icône — **aucune décision n'y vit**.
#[cfg(windows)]
pub mod win32;

#[cfg(test)]
mod tests;

/// Le pas de relecture de l'icône, sur le **fil de fenêtre** du capteur.
///
/// ⚠️ **Ce n'est PAS le tour de roue.** La spec D9 prescrivait de relire
/// l'icône sur le même tour que le presse-papier ; **le tour de roue n'a pas le
/// `hwnd`** (`capteur/sommeil/registre.rs` : aucun de ses quinze champs ne le
/// porte, et `inscrire(session, pid)` ne le prend pas). Le presse-papier y vit
/// parce qu'il est **global à la window station** — une ressource, un sondeur ;
/// l'accent est **par fenêtre**, et c'est justement la propriété que D9
/// revendique. Voir D-A1-1 du plan.
///
/// ⚠️ **NON CALIBRÉE.**
pub const PERIODE_ACCENT: Duration = Duration::from_secs(5);

/// En deçà de cette opacité, un pixel ne compte pas.
///
/// Une icône est **majoritairement transparente** : compter ses pixels vides
/// noierait toute teinte. ⚠️ **NON CALIBRÉE.**
const ALPHA_MIN: u8 = 128;

/// En deçà de cet écart `max − min` par pixel, la couleur est **achromatique**
/// et ne compte pas : c'est ce qui empêche un aplat gris de gagner.
/// ⚠️ **NON CALIBRÉE.**
const SATURATION_MIN: u8 = 32;

/// Hors de cette bande de luminance, un pixel ne compte pas : c'est ce qui
/// empêche un **contour sombre majoritaire** de l'emporter sur la teinte de
/// l'icône. ⚠️ **NON CALIBRÉES.**
const LUMA_MIN: u8 = 32;
/// Voir [`LUMA_MIN`].
const LUMA_MAX: u8 = 224;

/// Le pas de quantification, par canal : les pixels retenus sont rangés dans
/// des seaux de `PAS³`, et c'est le seau le plus peuplé qui décide.
/// ⚠️ **NON CALIBRÉE.**
const PAS: u16 = 32;

/// Luminance perçue, en entier, sur les coefficients ITU-R BT.601.
fn luma(r: u8, g: u8, b: u8) -> u8 {
    ((r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000) as u8
}

/// La couleur dominante d'une tranche **RGBA**, ou `None`.
///
/// ⚠️ **RGBA, et non BGRA.** `GetDIBits` rend du **BGRA** : la conversion
/// appartient à `accent/win32.rs`, jamais ici. Se tromper de sens échangerait
/// le rouge et le bleu — un défaut **plausible et silencieux**, qu'aucun test
/// d'hôte ne verrait puisqu'il vivrait derrière le `#[cfg(windows)]`.
///
/// Les cinq clauses, et chacune est un test :
/// 1. les pixels **trop transparents** ne comptent pas (`ALPHA_MIN`) ;
/// 2. les pixels **non chromatiques** ne comptent pas — trop peu saturés
///    (`SATURATION_MIN`), trop sombres ou trop clairs (`LUMA_MIN`/`LUMA_MAX`) ;
/// 3. les survivants sont **quantifiés** par seaux de `PAS` ;
/// 4. on rend la **MOYENNE des pixels du seau le plus peuplé** — jamais le
///    centre du seau : la moyenne rend une teinte **réelle de l'image**, le
///    centre rend une teinte **de la grille** ;
/// 5. `None` si aucun pixel ne survit. **`None` n'est PAS une erreur** : c'est
///    « pas d'accent », et aucune annonce ne part. Il doit rester
///    **atteignable**, sans quoi la clause 2 serait un ornement.
///
/// En cas d'égalité de population, le seau de plus petite clé l'emporte : le
/// résultat est **déterministe**, ce qu'un test exige.
pub fn dominante(rgba: &[u8], largeur: u32, hauteur: u32) -> Option<[u8; 3]> {
    let attendu = (largeur as usize).checked_mul(hauteur as usize)?.checked_mul(4)?;
    if attendu == 0 || rgba.len() != attendu {
        return None;
    }

    // clé de seau -> (somme_r, somme_g, somme_b, compte)
    let mut seaux: std::collections::BTreeMap<(u16, u16, u16), (u64, u64, u64, u64)> =
        std::collections::BTreeMap::new();

    for pixel in rgba.chunks_exact(4) {
        let (r, g, b, a) = (pixel[0], pixel[1], pixel[2], pixel[3]);
        if a < ALPHA_MIN {
            continue;
        }
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        if max - min < SATURATION_MIN {
            continue;
        }
        let l = luma(r, g, b);
        if l < LUMA_MIN || l > LUMA_MAX {
            continue;
        }
        let cle = (r as u16 / PAS, g as u16 / PAS, b as u16 / PAS);
        let seau = seaux.entry(cle).or_insert((0, 0, 0, 0));
        seau.0 += r as u64;
        seau.1 += g as u64;
        seau.2 += b as u64;
        seau.3 += 1;
    }

    let (_, (sr, sg, sb, n)) = seaux.iter().max_by_key(|(cle, (_, _, _, n))| {
        // `max_by_key` rend le DERNIER maximum : on inverse la clé pour que le
        // seau de plus petite clé gagne les égalités.
        (*n, std::cmp::Reverse(**cle))
    })?;
    if *n == 0 {
        return None;
    }
    Some([(sr / n) as u8, (sg / n) as u8, (sb / n) as u8])
}

/// `#rrggbb`, **six chiffres hexadécimaux minuscules**, et rien d'autre.
///
/// 🔴 **Le format est une contrainte du DESIGN SYSTEM, pas du protocole**, et un
/// successeur qui l'ignorerait l'élargirait sans le savoir :
/// `client/src/design/contraste.ts::luminanceRelative` n'accepte que `#rgb`,
/// `#rgba`, `#rrggbb` et `#rrggbbaa`, et **LÈVE** sur tout le reste. Le client
/// se défend (`client/src/accent.ts` refuse toute autre forme **avant** d'appeler
/// `rapportDeContraste`), mais l'agent n'a aucune raison de lui envoyer une
/// forme qu'il devra jeter.
pub fn en_hexa(rgb: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])
}

/// Suit la couleur d'accent d'une fenêtre et n'annonce que les CHANGEMENTS —
/// **sa première lecture comprise**.
///
/// 🔴 **C'est l'INVERSE de `plein_ecran::SuiviBordure`, et c'est délibéré.**
/// Celui-là est construit à partir de **l'état lu à l'ouverture**, précisément
/// pour ne **rien** annoncer au premier tour : D8 voulait qu'« une application
/// née sans bordure n'annonce rien ». L'accent a le besoin **inverse** — le
/// navigateur doit recevoir la couleur initiale, sinon `--accent-fenetre` n'est
/// jamais posé de la session.
///
/// ⚠️ **C'est ce qui rend le critère ④ jugeable** : « aucun message tant que
/// l'icône ne change pas » se compte **APRÈS** la première annonce, et le relevé
/// doit dire *exactement une* annonce en régime établi. **Un critère qui
/// exigerait zéro message serait tenu par un mécanisme entièrement mort.**
///
/// ⚠️ **Et c'est ce qui rend le rejeu à l'inscription inutile** : un
/// rattachement recrée le fil de fenêtre côté capteur, donc un `SuiviAccent`
/// neuf, donc une première annonce. Le legs n°3 de P1 — l'état courant à
/// l'attache, qui a coûté deux tâches à P3 — **n'a pas d'équivalent ici**.
#[derive(Default)]
pub struct SuiviAccent {
    derniere: Option<String>,
}

impl SuiviAccent {
    /// Un suivi neuf n'a **rien** vu : sa première lecture réussie s'annonce.
    pub fn neuf() -> Self {
        Self::default()
    }

    /// Rend `Some(couleur)` au changement — **première lecture comprise** —,
    /// `None` sinon.
    pub fn observer(&mut self, couleur: &str) -> Option<String> {
        if self.derniere.as_deref() == Some(couleur) {
            return None;
        }
        self.derniere = Some(couleur.to_string());
        Some(couleur.to_string())
    }
}

/// `ACCENT=0` désarme le mécanisme **ENTIER**.
///
/// ⚠️ **`=0` DÉSACTIVE ; une simple PRÉSENCE n'active pas** — convention de
/// `PLEIN_ECRAN`, `AUDIO`, `SUPERVISEUR`, `CAPTEUR`, `PART_SONDAGE`,
/// `PRESSE_PAPIER` et `APPS`, **et pour la même raison** : tester `is_ok()`
/// armerait le mécanisme en écrivant `ACCENT=0` pour le couper.
///
/// 🔴 **Elle se teste AVANT toute lecture Win32**, comme `Sondeur::tour` le fait
/// pour le presse-papier : `ACCENT=0` doit empêcher jusqu'au
/// `SendMessageTimeout`, pas seulement l'envoi.
///
/// `OnceLock` et non une lecture par appel : la relecture court à 0,2 Hz, et
/// l'environnement ne change pas en cours de processus.
pub fn actif() -> bool {
    static ACTIF: OnceLock<bool> = OnceLock::new();
    *ACTIF.get_or_init(|| {
        let actif = std::env::var("ACCENT").as_deref() != Ok("0");
        if !actif {
            tracing::warn!(
                "accent de fenetre DESARME (ACCENT=0) : la couleur de l'icone \
                 n'est plus poussee au navigateur"
            );
        }
        actif
    })
}
