//! Campagne discriminante du sous-bloc D3 : sur quoi porte le plafond de
//! quatre duplications DXGI simultanées ?
//!
//! Ce qu'on sait : **8 duplications de front dans UN SEUL processus tiennent**
//! (31 juillet 2026), et la **5ᵉ, dans un 5ᵉ processus, est refusée** en
//! `0x887A0022` (sous-bloc D2), par une limite durable qui résiste à trois
//! secondes de patience explicite. Rapprocher ces deux relevés pour conclure
//! « le plafond porte sur les processus » est une **inférence** : les deux
//! montages diffèrent d'au moins deux variables. Ce module existe pour
//! supprimer cette inférence.
//!
//! # Le montage
//!
//! Un processus **porteur** crée K = P×D sorties virtuelles, bat le chien de
//! garde du pilote et **ne duplique rien lui-même** — c'est la position exacte
//! du superviseur en D2, et c'est ce qui rend la mesure comparable au symptôme
//! de produit. Il lance ensuite P processus **sondes minimales**, qui ouvrent
//! chacune D duplications et les tiennent.
//!
//! **Pourquoi le porteur ne duplique pas** : une sonde peut mourir en
//! `0xc0000005`, comme toutes ces API. Le porteur, qui ne touche qu'au pilote,
//! survit, et sa garde détruit les K sorties. Un porteur qui dupliquerait
//! risquerait de les emporter avec lui.
//!
//! Spec : `docs/superpowers/specs/2026-08-02-multifenetres-plafond-concurrence-design.md`

mod sonde;

pub(super) use sonde::sonder;

use anyhow::{Context, Result};

/// Nombre maximal de sorties que la campagne s'autorise à créer. Le vivier du
/// pilote vaut **10** (mesuré le 31 juillet 2026, refus à la 11ᵉ création en
/// `ERROR_TOO_MANY_NAMES`), et Apollo puise au même : huit laisse deux de
/// marge.
const SORTIES_MAX: u16 = 8;

/// Analyse `"<P>x<D>"` : P processus sondes, D duplications chacune.
pub(super) fn analyser(valeur: &str) -> Result<(u8, u8)> {
    let (p, d) = valeur
        .split_once('x')
        .with_context(|| format!("MULTIFENETRE_PLAFOND attend « <P>x<D> », reçu « {valeur} »"))?;
    let processus: u8 = p
        .trim()
        .parse()
        .with_context(|| format!("nombre de processus illisible dans « {valeur} »"))?;
    let duplications: u8 = d
        .trim()
        .parse()
        .with_context(|| format!("nombre de duplications illisible dans « {valeur} »"))?;
    anyhow::ensure!(processus >= 1, "au moins un processus sonde est nécessaire");
    anyhow::ensure!(duplications >= 1, "au moins une duplication par sonde est nécessaire");
    let total = u16::from(processus) * u16::from(duplications);
    anyhow::ensure!(
        total <= SORTIES_MAX,
        "{processus}x{duplications} demande {total} sorties, le maximum est {SORTIES_MAX} \
         (vivier du pilote de 10, dont deux de marge pour Apollo)"
    );
    Ok((processus, duplications))
}

/// Le porteur. Corps écrit en Task 10 du plan D3.
pub(super) fn mesurer(_processus: u8, _duplications: u8) -> Result<()> {
    anyhow::bail!("porteur non implémenté — voir la Task 10 du plan D3")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_cinq_rangs_de_la_matrice_sont_acceptes() {
        for (valeur, attendu) in
            [("1x8", (1, 8)), ("2x4", (2, 4)), ("4x2", (4, 2)), ("8x1", (8, 1)), ("4x1", (4, 1))]
        {
            assert_eq!(analyser(valeur).unwrap(), attendu, "rang {valeur}");
        }
    }

    #[test]
    fn un_produit_au_dela_du_vivier_est_refuse() {
        let erreur = analyser("4x4").unwrap_err().to_string();
        assert!(erreur.contains("16 sorties"), "reçu « {erreur} »");
    }

    #[test]
    fn un_zero_est_refuse() {
        assert!(analyser("0x4").is_err());
        assert!(analyser("4x0").is_err());
    }

    #[test]
    fn une_valeur_malformee_est_refusee() {
        assert!(analyser("4").is_err());
        assert!(analyser("quatre x deux").is_err());
    }
}
