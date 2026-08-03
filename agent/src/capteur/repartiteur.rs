//! La règle de part : qui reçoit quelle fraction du budget de session.
//!
//! **Pas de `#[cfg(windows)]`, aucun objet COM, aucun canal.** Ce module ne
//! fait que décider ; l'application vit dans `capteur/sommeil.rs` et
//! `capteur/fenetre.rs`. C'est le patron posé par D4 pour `capteur/protocole.rs`
//! et par D5 pour `capteur/vivier.rs` : ce qui décide se teste sur l'hôte.
//!
//! **Le défaut que ce module existe pour corriger.** Depuis le sous-bloc D1,
//! chaque fenêtre est un processus portant sa propre `PeerConnection`, donc son
//! propre BWE, et chacune hérite `BITRATE` tel quel. À huit fenêtres, huit
//! `set_desired_bitrate` visent 96 Mb/s cumulés sur un lien unique, et le
//! sondage à la hausse de chacune est lu par les autres comme de la congestion.

/// Majoration accordée à la fenêtre que l'utilisateur regarde.
///
/// ⚠️ **NON CALIBRÉE.** Le raisonnement qui la fonde n'est pas une mesure :
/// deux barreaux voisins de l'échelle sont dans un rapport de pixels de
/// 1,25² ≈ 1,56 (`DIVISEURS` de `congestion/echelle.rs`), donc un facteur 2
/// garantit plus d'un barreau d'écart en faveur de la fenêtre regardée. C'est
/// le critère ② de la recette qui la jugera, pas cette intuition.
pub const FACTEUR_FOCUS: u32 = 2;

/// Part laissée à une fenêtre endormie.
///
/// **Jamais zéro** : elle n'encode plus rien (D5 a relâché son encodeur) mais
/// sa `PeerConnection` vit, et `set_desired_bitrate(0)` n'est pas un réglage
/// que str0m est censé recevoir.
///
/// ⚠️ **NON CALIBRÉE**, et l'hypothèse qui la motive n'est pas vérifiée : on
/// ignore si str0m émet réellement du bourrage de sondage quand aucun média ne
/// part. Si oui, ce plancher empêche des fenêtres endormies de manger le lien
/// pour rien ; si non, il ne coûte que sa ligne. L'ordre de grandeur couvre
/// l'audio (`opus::BITRATE_BPS`, 128 kb/s) et laisse de la marge.
pub const PART_DORMANTE_BPS: u32 = 256_000;

/// L'état d'une fenêtre, tel que le répartiteur a besoin de le connaître.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fenetre {
    pub session: String,
    pub eveillee: bool,
    pub focalisee: bool,
}

/// Découpe `budget_bps` entre les fenêtres.
///
/// Chaque endormie reçoit `PART_DORMANTE_BPS` ; le reste se divise entre les
/// éveillées, la focalisée recevant `FACTEUR_FOCUS` parts au lieu d'une.
///
/// **Garanties inviolables** : aucune panique, aucune part nulle, fonction
/// totale. La somme des parts peut dépasser le budget dans une bande étroite
/// qui a pour cause : quand le budget reste après paiement des planchers ne
/// suffit pas pour servir 1 bps à chaque éveillée, le `.max(1)` appliqué à
/// chaque part finale produit un dépassement borné par le nombre d'éveillées,
/// en bits par seconde.
///
/// **Cas normal** (`reste ≥ eveillees`) : la somme ne dépasse jamais le
/// budget, et la majoration de focus est appliquée.
///
/// **Cas dégénéré** (`reste < eveillees`) : chaque éveillée reçoit 1 bps
/// (dû au `.max(1)`), chaque endormie son plancher, et la somme dépasse le
/// budget d'au plus `eveillees` bps. La majoration de focus disparaît car
/// `part_base` vaut 0. Ce comportement est assumé : un `set_desired_bitrate(0)`
/// serait pire qu'un microdepassement sur un lien que rien ne peut satisfaire.
///
/// **Aucun travail conservateur** : une fenêtre qui n'use pas sa part ne la
/// rend pas aux autres. Ce serait une seconde boucle de rétroaction dont la
/// stabilité devrait être éprouvée — hors périmètre de D6.
pub fn repartir(budget_bps: u32, fenetres: &[Fenetre]) -> Vec<(String, u32)> {
    let endormies = fenetres.iter().filter(|f| !f.eveillee).count() as u32;
    let eveillees = fenetres.iter().filter(|f| f.eveillee).count() as u32;

    // `saturating_sub` : un budget inférieur au total des planchers rend un
    // reste nul, jamais un débordement. Les endormies gardent alors leur
    // plancher et les éveillées reçoivent le minimum d'une part, ce qui fait
    // franchir le budget — cas dégénéré assumé, mais il ne panique pas.
    let reste = budget_bps.saturating_sub(endormies.saturating_mul(PART_DORMANTE_BPS));

    // Une seule majoration, à la PREMIÈRE focalisée éveillée rencontrée :
    // plusieurs focalisées ne durent pas (le client émet `blur`), mais en
    // accorder deux ferait sauter l'invariant de budget.
    let indice_focalisee =
        fenetres.iter().position(|f| f.eveillee && f.focalisee);

    let diviseur = match indice_focalisee {
        Some(_) => eveillees.saturating_sub(1) + FACTEUR_FOCUS,
        None => eveillees,
    };
    // `max(1)` : sans éveillée, le diviseur vaut 0 et la division paniquerait.
    // La valeur ne sert alors à personne — aucune fenêtre n'est éveillée.
    let part_base = reste / diviseur.max(1);

    fenetres
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let bps = if !f.eveillee {
                PART_DORMANTE_BPS
            } else if Some(i) == indice_focalisee {
                part_base.saturating_mul(FACTEUR_FOCUS)
            } else {
                part_base
            };
            // Aucune part nulle : un budget dérisoire ne doit pas produire un
            // `set_desired_bitrate(0)`.
            (f.session.clone(), bps.max(1))
        })
        .collect()
}

#[cfg(test)]
mod tests;
