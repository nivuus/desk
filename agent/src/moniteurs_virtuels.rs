//! Mesure ① de la spec : combien de sorties virtuelles simultanées un pilote
//! d'affichage indirect accepte-t-il, et une fenêtre posée dessus est-elle
//! capturée correctement.
//!
//! Ce module ne contient QUE de la logique pure : le trait que doit remplir
//! un pilote, la garde qui détruit ce qui a été créé, et les conversions de
//! coordonnées. La glue Windows vit dans les sous-modules `pilote`,
//! `sudovda`, `peripherique` et `purge`, promus depuis
//! `diagnostics/multifenetre/` au sous-bloc D1.
//!
//! Il n'est PAS sous `#[cfg(windows)]`, délibérément : une sortie virtuelle
//! survit au processus, donc la garde ci-dessous est le seul rempart contre
//! une VM laissée avec des moniteurs fantômes — c'est exactement le genre de
//! code qui doit avoir des tests, et ils ne tourneraient pas sous
//! `#[cfg(windows)]`.

// Glue Windows du pilote SudoVDA, promue depuis `diagnostics/multifenetre/`
// au sous-bloc D1 : ce n'est plus de l'outillage de mesure, c'est le chemin
// par lequel le produit fait paraître ses sorties. Le module parent reste
// hors `#[cfg(windows)]` — c'est ce qui permet à sa garde `Sorties` d'avoir
// des tests, et cette raison n'a pas changé.
#[cfg(windows)]
pub mod guid;
#[cfg(windows)]
pub mod peripherique;
#[cfg(windows)]
pub mod pilote;
#[cfg(windows)]
pub mod purge;
#[cfg(windows)]
pub mod sudovda;

// Hors `#[cfg(windows)]`, comme le module parent et pour la même raison :
// l'attribution des numéros de GUID décide si une sortie virtuelle orpheline
// reste récupérable, et ce genre de code doit avoir des tests. Voir son
// commentaire de tête (correctif I1).
pub mod numeros;

// Hors `#[cfg(windows)]` pour la même raison encore : la RÈGLE qui échange un
// identifiant de cible contre un nom GDI décide de l'appariement de TOUTE
// fenêtre, et elle doit avoir des tests. Sa moitié Win32 est dans son `mod
// win` interne — patron de `superviseur/placement.rs`.
pub mod config_affichage;

// Hors `#[cfg(windows)]` pour la même raison : le VERDICT d'une purge est une
// règle pure, et un `ERROR` qui crie à tort à chaque démarrage est un `ERROR`
// que plus personne ne lit. Voir son commentaire de tête.
pub mod verdict_purge;

use anyhow::{Context, Result};

use crate::geometry::Rect;

/// Identifiant d'une sortie virtuelle, tel que le pilote le rend.
pub type IdSortie = u32;

/// L'adaptateur sur lequel le pilote a créé une sortie : un `LUID` Win32,
/// écrit en deux moitiés — exactement comme `sudovda::SortieAjoutee` l'écrit
/// déjà, et pour la même raison (la disposition supposée doit être lisible là
/// où elle est en jeu).
///
/// 🔴 **Le pilote rend TROIS nombres, et le produit n'en gardait qu'UN.**
/// `SortieAjoutee` porte `(adaptateur_bas, adaptateur_haut, identifiant_cible)`
/// ; jusqu'au lot 32 seul le troisième survivait à `creer`, les deux autres
/// n'étant que journalisés. Or c'est le COUPLE qui désigne une cible
/// d'affichage sans ambiguïté : un identifiant de cible n'est unique que PAR
/// adaptateur, et cette VM en a plus d'un (SudoVDA, plus le VGA de QEMU quand
/// il est présent).
pub type Adaptateur = (u32, i32);

/// Ce que ce bloc attend d'un pilote d'affichage virtuel, quel qu'il soit.
///
/// L'indirection existe pour deux raisons. La spec §6.4 acte un repli —
/// changer de pilote si celui de la VM résiste — et ce repli ne doit faire
/// réécrire ni la montée en N ni la garde. Et la garde ci-dessous doit
/// pouvoir être éprouvée sans Windows.
pub trait PiloteAffichageVirtuel {
    fn creer(&self, largeur: u32, hauteur: u32, hertz: u32) -> Result<IdSortie>;
    fn detruire(&self, id: IdSortie) -> Result<()>;
}

/// Détruit les sorties créées quoi qu'il arrive, y compris si le fil panique.
///
/// Sans elle, une sonde qui plante à la cinquième création laisse cinq
/// moniteurs derrière elle, et l'état survit au processus.
pub struct Sorties<'p> {
    pilote: &'p dyn PiloteAffichageVirtuel,
    creees: Vec<IdSortie>,
}

impl<'p> Sorties<'p> {
    pub fn nouvelles(pilote: &'p dyn PiloteAffichageVirtuel) -> Self {
        Self {
            pilote,
            creees: Vec::new(),
        }
    }

    /// Un refus du pilote ressort tel quel et ne compte pas comme création :
    /// détruire un identifiant que le pilote n'a jamais rendu ferait au mieux
    /// une erreur de plus au journal, au pire détruirait la sortie d'autrui.
    pub fn creer(&mut self, largeur: u32, hauteur: u32, hertz: u32) -> Result<IdSortie> {
        let id = self.pilote.creer(largeur, hauteur, hertz)?;
        self.creees.push(id);
        Ok(id)
    }

    #[cfg(test)]
    pub fn nombre(&self) -> usize {
        self.creees.len()
    }

    /// Rend une sortie au pilote **pendant** l'exécution, et cesse de la
    /// tenir.
    ///
    /// Sans cette méthode, une sortie n'est rendue qu'à la destruction de la
    /// garde, c'est-à-dire à l'arrêt du superviseur : le vivier du pilote
    /// (dix sorties, mesuré) se consommerait alors à chaque OUVERTURE de
    /// fenêtre et non par fenêtre simultanée, et une dizaine
    /// d'ouvertures-fermetures suffirait à bloquer toute nouvelle fenêtre.
    ///
    /// Sur refus du pilote, la sortie **reste tenue** : elle est encore due,
    /// et la garde la retentera à la destruction. L'oublier ici la rendrait
    /// irrécupérable — le pilote ne retire que par un GUID dont lui seul et
    /// `PiloteParIoctl` gardent la trace.
    pub fn detruire(&mut self, id: IdSortie) -> Result<()> {
        let rang = self
            .creees
            .iter()
            .position(|connu| *connu == id)
            .with_context(|| format!("sortie {id} non tenue par cette garde — rien à rendre"))?;
        self.pilote.detruire(id)?;
        self.creees.remove(rang);
        Ok(())
    }
}

impl Drop for Sorties<'_> {
    fn drop(&mut self) {
        // En ordre inverse de création : si le pilote a un état d'ordre, le
        // défaire dans l'ordre où il a été construit est le seul choix sûr.
        // `Drop` court aussi pendant le déroulement d'une panique — c'est
        // précisément le cas que la garde existe pour couvrir.
        for id in self.creees.drain(..).rev() {
            if let Err(erreur) = self.pilote.detruire(id) {
                tracing::error!(
                    id,
                    %erreur,
                    "sortie virtuelle NON détruite — purge manuelle requise"
                );
            }
        }
    }
}

/// Rapport entre les dimensions ANNONCÉES par la sortie
/// (`DXGI_OUTPUT_DESC::DesktopCoordinates`) et celles de la texture
/// RÉELLEMENT rendue par l'acquisition.
///
/// La sonde a relevé une sortie virtuelle annoncée 3413×960 par DXGI quand
/// WMI la disait 5120×1440 — rapport 1,5006, la mise à l'échelle DPI à 150 %.
/// Si un recadrage est calculé sur le rectangle annoncé alors que la texture
/// est aux dimensions physiques, il est décalé d'autant. Rend `None` si
/// l'annonce est dégénérée : un rapport n'y aurait aucun sens.
pub fn facteur_echelle(annonce: (u32, u32), texture: (u32, u32)) -> Option<(f64, f64)> {
    if annonce.0 == 0 || annonce.1 == 0 {
        return None;
    }
    Some((
        texture.0 as f64 / annonce.0 as f64,
        texture.1 as f64 / annonce.1 as f64,
    ))
}

/// Convertit un rectangle exprimé en coordonnées du bureau virtuel — celles
/// où vivent les fenêtres — vers les coordonnées de la texture rendue par
/// l'acquisition de `sortie`.
///
/// Deux corrections en une : le décalage de l'origine de la sortie dans le
/// bureau virtuel, et le facteur d'échelle de `facteur_echelle`.
pub fn vers_texture(region: Rect, sortie: Rect, facteur: (f64, f64)) -> Rect {
    let x = (region.x - sortie.x) as f64 * facteur.0;
    let y = (region.y - sortie.y) as f64 * facteur.1;
    Rect {
        x: x.round() as i32,
        y: y.round() as i32,
        width: (region.width as f64 * facteur.0).round() as u32,
        height: (region.height as f64 * facteur.1).round() as u32,
    }
}

/// La place plein cadre de chaque sortie, exprimée dans le repère de SA
/// texture.
///
/// Le montage « une fenêtre par sortie » du chantier D pose une fenêtre qui
/// couvre toute sa sortie ; la région à recadrer est donc toute la texture.
/// Le calcul n'en est pas trivial pour autant : chaque sortie porte son propre
/// facteur d'échelle DPI, et appliquer à toutes celui de la première décalerait
/// silencieusement les recadrages des autres. Le banc mono-sortie n'avait qu'un
/// facteur à connaître ; celui-ci en a N.
///
/// `sorties` porte les rectangles annoncés par DXGI
/// (`DXGI_OUTPUT_DESC::DesktopCoordinates`), `textures` les dimensions
/// réellement rendues par l'acquisition de chacune, dans le même ordre.
pub fn places_texture_par_sortie(sorties: &[Rect], textures: &[(u32, u32)]) -> Result<Vec<Rect>> {
    anyhow::ensure!(
        sorties.len() == textures.len(),
        "{} sorties pour {} textures : l'appariement serait arbitraire",
        sorties.len(),
        textures.len()
    );
    sorties
        .iter()
        .zip(textures)
        .enumerate()
        .map(|(index, (sortie, texture))| {
            let facteur =
                facteur_echelle((sortie.width, sortie.height), *texture).with_context(|| {
                    format!(
                        "sortie {index} annoncée {}x{} : dimension nulle, aucun facteur \
                         d'échelle n'a de sens",
                        sortie.width, sortie.height
                    )
                })?;
            Ok(vers_texture(*sortie, *sortie, facteur))
        })
        .collect()
}

#[cfg(test)]
mod tests;
