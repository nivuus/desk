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
//! `set_desired_bitrate` visent 96 Mb/s cumulés sur un lien unique : personne
//! n'arbitre, et chaque fenêtre encode comme si elle était seule.
//!
//! ⚠️ **Ce n'est PAS une congestion de lien, et la prémisse d'origine du
//! sous-bloc disait le contraire — la recette l'a réfutée.** Le pont porte
//! **≥ 1,44 Gb/s** et `packetsLost` vaut **0 aux onze exécutions** : 96 Mb/s
//! cumulés sont entre 15 et 27 fois moins que ce que le chemin porte, et
//! aucune fenêtre n'a jamais lu le sondage d'une autre comme de la congestion.
//! **Le goulot mesuré est le DÉCODEUR du navigateur**, et ce qui le soulage
//! est le nombre de pixels : réduire les bits sans franchir de seuil de
//! barreau ne sauve rien (23,08 % d'images jetées à surface constante), quand
//! passer de 1280×720 à 852×480 fait tomber le taux de 18,03 % à 3,94 %.
//! Le partage reste donc le bon mécanisme, mais **il agit par la RÉSOLUTION** :
//! une part plus petite fait descendre l'échelle d'`congestion/echelle.rs`
//! d'un barreau, et c'est ce barreau qui soulage le décodeur. Voir
//! `docs/superpowers/plans/2026-08-03-multifenetres-partage-capacite-resultats.md`,
//! §1 et §3.6.

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
/// part. Si oui, ce plancher évite à des fenêtres endormies d'émettre du
/// trafic pour rien ; si non, il ne coûte que sa ligne. **Ce n'était de toute
/// façon jamais une question de saturation** : le lien porte ≥ 1,44 Gb/s et
/// n'a jamais perdu un paquet (voir la doc de tête). L'ordre de grandeur
/// couvre l'audio (`opus::BITRATE_BPS`, 128 kb/s) et laisse de la marge.
///
/// ⚠️ **Cette valeur ne doit JAMAIS atteindre `Controleur::changer_plafond`.**
/// Elle est très en dessous du barreau le plus bas de l'échelle (691 200 bps à
/// 1280×720/60 avec `BPP_MIN`) : appliquée comme plafond d'encodage, elle
/// pose `video_bitrate_bps = 256_000` par le `min` de `changer_plafond`, et
/// **rien ne le remonte au réveil** — une endormie n'émet rien, donc str0m
/// n'émet aucun `MediaEgressStats` pour elle, donc `Controleur::observer`,
/// seule réparation possible, n'est jamais appelé. C'est le défaut I1 de la
/// revue finale de branche ; le remède vit dans `Session::appliquer_part`, qui
/// n'applique une part d'endormie qu'au sondage. **Une fenêtre endormie a
/// relâché son encodeur (D5) : il n'y a rien à borner côté encodage.**
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
/// totale.
///
/// **Trois régimes** selon `budget` et `diviseur` (nombre d'éveillées,
/// majoré de `FACTEUR_FOCUS - 1` s'il y a une focalisée éveillée) :
///
/// **Régime 1** : `budget ≥ endormies × PART_DORMANTE_BPS` ET `reste ≥ diviseur`.
/// La somme ne dépasse jamais le budget, et la majoration de focus est appliquée.
/// C'est la seule garantie de non-dépassement.
///
/// **Régime 2** : `budget ≥ endormies × PART_DORMANTE_BPS` MAIS `reste < diviseur`.
/// Le `.max(1)` appliqué à chaque part finale rend `part_base = 0`. Chaque
/// éveillée reçoit 1 bps, la majoration de focus disparaît, et la somme dépasse
/// le budget d'au plus `diviseur − reste` bps. Les planchers des endormies sont
/// payés en intégralité.
///
/// **Régime 3** : `budget < endormies × PART_DORMANTE_BPS`. Les planchers des
/// endormies sont quand même payés (aucune part nulle), et la somme dépasse le
/// budget de `(endormies × PART_DORMANTE_BPS − budget) + eveillees` bps, non borné
/// par le seul nombre d'éveillées. Ce comportement est assumé : un
/// `set_desired_bitrate(0)` serait pire qu'un dépassement sur un lien que rien
/// ne peut satisfaire de toute façon. Ce cas est décrit en détail au commentaire
/// du calcul de `reste` dans le corps.
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
