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

use super::montee::{
    attendre_en_pinguant, noms_attaches, relever_topologie, DELAI_TOPOLOGIE, RESOLUTION,
};

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

/// Le porteur : K = P×D sorties virtuelles, P sondes lancées en ESCALIER.
///
/// **L'échelonnement n'est pas un confort.** P sondes concurrentes rendraient
/// un refus sans rang identifiable, et la matrice ne trancherait rien : c'est
/// le rang du premier refus qui distingue « plafond sur les processus » de
/// « plafond sur les duplications ».
pub(super) fn mesurer(processus: u8, duplications: u8) -> Result<()> {
    let total = u16::from(processus) * u16::from(duplications);
    tracing::info!(processus, duplications, total, "campagne du plafond — début");

    // Aucun résidu d'un tirage précédent : un verdict périmé ferait lire un
    // succès là où la sonde n'a jamais démarré.
    let _ = std::fs::remove_file(sonde::chemin_arret());
    for rang in 0..processus {
        let _ = std::fs::remove_file(sonde::chemin_verdict(rang));
    }

    let avant = relever_topologie("avant création")?;
    let noms_avant = noms_attaches(&avant);

    let pilote = crate::moniteurs_virtuels::pilote::ouvrir_pilote()?;
    let (largeur, hauteur, hertz) = RESOLUTION;

    // Portée explicite de la garde : les sorties doivent être détruites AVANT
    // le relevé final, sans quoi celui-ci décrirait un état transitoire.
    let issue = {
        let mut sorties = crate::moniteurs_virtuels::Sorties::nouvelles(&pilote);
        for rang in 1..=total {
            sorties
                .creer(largeur, hauteur, hertz)
                .with_context(|| format!("création de la sortie virtuelle n°{rang}/{total}"))?;
        }

        // Attendre que les K sorties soient RATTACHÉES, en battant le chien de
        // garde : le pilote retire les sorties d'un client qui cesse de
        // pinguer, y compris celles qu'on vient de créer.
        //
        // Écart au brief (Step 1) : `attendre_en_pinguant` ne prend PAS
        // `(&pilote, &noms_avant, attendues)` et ne rend pas les noms
        // apparus — sa signature réelle, relevée dans `montee.rs:176`, est
        // `(&PiloteParIoctl, Duration) -> Result<()>` : elle attend `duree` en
        // pinguant, un point c'est tout. Même patron que `paralleles.rs`,
        // montage le plus proche de celui-ci : c'est un relevé de topologie
        // APRÈS l'attente qui rend les noms, pas la fonction d'attente
        // elle-même.
        attendre_en_pinguant(&pilote, DELAI_TOPOLOGIE)?;

        let apres_creation = relever_topologie("après création")?;
        let noms_apres_creation = noms_attaches(&apres_creation);
        let apparues: Vec<String> = noms_apres_creation
            .iter()
            .filter(|nom| !noms_avant.contains(nom))
            .cloned()
            .collect();
        // Ajout au-delà du brief : sans ce contrôle, moins de `total` noms
        // apparus ferait paniquer `conduire_les_sondes` sur un découpage en
        // tranches hors bornes plutôt que de rendre une erreur lisible — le
        // même risque que `designer_sorties_neuves` (`paralleles.rs`) couvre
        // pour sa propre mesure.
        anyhow::ensure!(
            apparues.len() == usize::from(total),
            "{} sortie(s) DXGI neuve(s) après création de {total} ({apparues:?}) — une \
             addition ou un retrait externe rend la mesure inimputable",
            apparues.len()
        );
        tracing::info!(attachees = apparues.len(), noms = ?apparues, "sorties rattachées");

        conduire_les_sondes(processus, duplications, &apparues)
    };

    // Signal d'arrêt retiré : le prochain tirage repart propre.
    let _ = std::fs::remove_file(sonde::chemin_arret());

    let apres = relever_topologie("après destruction")?;
    let noms_apres = noms_attaches(&apres);
    // Comparer des ENSEMBLES DE NOMS, jamais des cardinaux : Apollo peut
    // ajouter une sortie à tout instant, et une addition externe compenserait
    // exactement un retrait.
    if noms_apres != noms_avant {
        tracing::error!(
            avant = ?noms_avant, apres = ?noms_apres,
            "topologie NON restaurée — contrôler depuis un processus neuf (MULTIFENETRE_DXGI=1)"
        );
    } else {
        tracing::info!("topologie restaurée nom pour nom");
    }
    issue
}

/// Lance les sondes une à une et journalise la table de verdict.
fn conduire_les_sondes(processus: u8, duplications: u8, noms: &[String]) -> Result<()> {
    let executable = std::env::current_exe().context("chemin de l'exécutable courant")?;
    let mut enfants = Vec::new();
    let mut verdicts: Vec<(u8, String)> = Vec::new();

    for rang in 0..processus {
        let debut = usize::from(rang) * usize::from(duplications);
        let lot: Vec<&str> =
            noms[debut..debut + usize::from(duplications)].iter().map(|s| s.as_str()).collect();
        tracing::info!(sonde = rang, sorties = ?lot, "lancement de la sonde");

        let enfant = std::process::Command::new(&executable)
            .env("MULTIFENETRE_PLAFOND_SONDE", lot.join(","))
            .env("MULTIFENETRE_PLAFOND_RANG", rang.to_string())
            // La variable du porteur ne doit PAS être héritée : l'enfant
            // recréerait K sorties. L'aiguillage traite déjà la sonde en
            // premier, mais retirer la variable rend l'invariant explicite.
            .env_remove("MULTIFENETRE_PLAFOND")
            .spawn()
            .with_context(|| format!("lancement de la sonde {rang}"))?;
        enfants.push(enfant);

        let verdict = attendre_le_verdict(rang)?;
        tracing::info!(sonde = rang, %verdict, "verdict reçu");
        let refuse = verdict.starts_with("KO");
        verdicts.push((rang, verdict));
        if refuse {
            // On s'arrête au premier refus : c'est SON rang qui est la mesure.
            // Continuer ne rendrait que des refus dérivés.
            break;
        }
    }

    // Signal d'arrêt : toutes les sondes relâchent et sortent.
    std::fs::write(sonde::chemin_arret(), b"1").context("dépôt du signal d'arrêt")?;
    for (rang, mut enfant) in enfants.into_iter().enumerate() {
        match enfant.wait() {
            Ok(statut) => tracing::info!(sonde = rang, ?statut, "sonde terminée"),
            Err(erreur) => tracing::error!(sonde = rang, %erreur, "attente de la sonde échouée"),
        }
    }

    tracing::info!(
        processus,
        duplications,
        lancees = verdicts.len(),
        verdicts = ?verdicts,
        "campagne du plafond — bilan"
    );
    Ok(())
}

/// Attend le verdict d'une sonde, ou conclut qu'elle est morte sans en rendre.
///
/// **Borné.** Une sonde peut mourir en `0xc0000005` sans jamais écrire son
/// fichier ; sans cette borne, le porteur attendrait indéfiniment en tenant K
/// sorties virtuelles.
fn attendre_le_verdict(rang: u8) -> Result<String> {
    const LIMITE: std::time::Duration = std::time::Duration::from_secs(30);
    let echeance = std::time::Instant::now() + LIMITE;
    let chemin = sonde::chemin_verdict(rang);
    loop {
        if let Ok(contenu) = std::fs::read_to_string(&chemin) {
            return Ok(contenu.trim().to_string());
        }
        if std::time::Instant::now() >= echeance {
            return Ok(format!("MORTE (aucun verdict en {} s)", LIMITE.as_secs()));
        }
        // Pas de trace ici : cette boucle scrute à 10 Hz, et une trace par
        // tour a déjà détruit une mesure de ce projet (18 619 lignes en
        // quelques secondes sur un partage réseau, chantier TURN).
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
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
