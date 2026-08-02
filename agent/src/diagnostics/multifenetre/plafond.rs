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

use super::compteurs;
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
    // Verdict de secours d'un rang illisible : sans effet sur la mesure (le
    // porteur ne le lit jamais), mais trompeur pour qui fouille `%TEMP%`
    // après coup si un tirage précédent l'a laissé derrière lui.
    let _ = std::fs::remove_file(sonde::chemin_verdict_rang_invalide());

    let avant = relever_topologie("avant création")?;
    let noms_avant = noms_attaches(&avant);

    let pilote = crate::moniteurs_virtuels::pilote::ouvrir_pilote()?;
    let (largeur, hauteur, hertz) = RESOLUTION;

    // Portée explicite de la garde : les sorties doivent être détruites AVANT
    // le relevé final, sans quoi celui-ci décrirait un état transitoire.
    //
    // Enveloppée dans une fermeture immédiatement appelée, et non un simple
    // bloc `{ }` : un `?` DANS un bloc sort de toute la fonction `mesurer`,
    // pas seulement du bloc — il sauterait alors le rejeu des retraits dus,
    // le retrait du signal d'arrêt et le relevé final de restauration de
    // topologie plus bas. La fermeture capture un échec dans `issue` comme
    // une valeur normale : l'exécution continue après elle sur TOUS les
    // chemins, y compris ceux où la création, l'attente ou le contrôle des
    // noms apparus échouent — pas seulement ceux internes à
    // `conduire_les_sondes`. `sorties` reste locale à cette fermeture, donc
    // toujours détruite avant que `issue` ne soit produit : l'invariant de
    // portée ne change pas de nature avec la fermeture.
    let issue = (|| -> Result<()> {
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
        // pour sa propre mesure. `noms_attaches` filtre déjà sur
        // `attachee_au_bureau` : une sortie créée mais non composée par
        // Windows ne compterait donc pas comme apparue, d'où le message
        // ci-dessous qui NOMME ce cas plutôt que de ne parler que
        // d'« addition ou retrait externe » (le seul diagnostic exact serait
        // de reprendre la relève brute pour distinguer les deux causes, comme
        // `designer_sorties_neuves` le fait — hors de portée d'une correction
        // d'une ligne, donc seul le message est corrigé ici).
        anyhow::ensure!(
            apparues.len() == usize::from(total),
            "{} sortie(s) DXGI attachée(s) neuve(s) après création de {total} ({apparues:?}) \
             — addition externe, retrait par le chien de garde, ou sortie créée mais jamais \
             composée par Windows (non attachée au bureau)",
            apparues.len()
        );
        tracing::info!(attachees = apparues.len(), noms = ?apparues, "sorties rattachées");

        // La `Garde` continue de battre le chien de garde PENDANT toute la
        // conduite des sondes, pas seulement pendant la création : sans elle,
        // plus un seul ping ne partirait entre ce point et la fin de
        // `conduire_les_sondes`, soit jusqu'à P × (D × 3 s + marge) de
        // silence — bien au-delà du trou de 11,1 s que `Garde` a été créée
        // pour fermer (`compteurs.rs`), sur un chien de garde dont l'unité de
        // `delai` n'est toujours pas établie. Construite ici, juste après le
        // dernier ping d'`attendre_en_pinguant` : la couture entre les deux
        // est négligeable (66 µs mesurés ailleurs sur le même patron), pas
        // comptée dans `intervalle_max`.
        let mut garde = compteurs::Garde::nouvelle(&pilote);
        let issue_conduite = conduire_les_sondes(processus, duplications, &apparues, &mut garde);
        tracing::info!(
            intervalle_ping_max_ms = garde.intervalle_max().as_millis() as u64,
            "chien de garde : plus grand écart entre deux battements sur toute la conduite"
        );
        issue_conduite
    })();

    // Second essai des retraits que la garde n'a pas obtenus : dernière
    // chance de CE processus de les rejouer, au-delà seule la purge
    // inter-processus (`MULTIFENETRE_VDD_PURGE`) les atteindra. Même geste
    // que `monter_en_n` et `paralleles::mesurer` — sans lui, sur un vivier de
    // 10 et une matrice de cinq rangs joués à la suite, une sortie orpheline
    // d'un rang empoisonnerait les rangs suivants.
    let rejoues = crate::moniteurs_virtuels::purge::rejouer_purge_due(&pilote);
    if rejoues > 0 {
        tracing::info!(rejoues, "retraits dus rejoués avec succès après la garde");
    }

    // Signal d'arrêt retiré : le prochain tirage repart propre.
    let _ = std::fs::remove_file(sonde::chemin_arret());

    // Un pilote d'affichage indirect ne défait pas sa topologie dans
    // l'instant : Windows reconfigure. Sans ce délai de grâce (même valeur et
    // même raison que `monter_en_n` et `paralleles::mesurer`), le relevé
    // ci-dessous risquerait de voir encore les K sorties en cours de retrait
    // et d'écrire à tort « topologie NON restaurée ».
    std::thread::sleep(DELAI_TOPOLOGIE);
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

/// Garde qui dépose le signal d'arrêt et attend les sondes encore vivantes,
/// **quoi qu'il arrive** — y compris sur un chemin d'erreur (un `?` de
/// `spawn`, ou désormais un ping en échec relayé par `Garde::battre_si_du`
/// depuis `attendre_le_verdict`) ou pendant le déroulement d'une panique.
/// Même patron que `moniteurs_virtuels::Sorties`.
///
/// Sans elle, un `?` qui sort après quelques sondes déjà lancées les laisse
/// boucler indéfiniment à 10 Hz sur `chemin_arret()`, tenant leurs
/// duplications DXGI et faussant le rang suivant de la matrice — un plafond
/// mesuré au rang N+1 refléterait alors des duplications du rang N encore
/// vivantes, pas seulement celles de N+1.
struct SondesEnCours {
    enfants: Vec<(u8, std::process::Child)>,
}

impl SondesEnCours {
    fn nouvelle() -> Self {
        Self { enfants: Vec::new() }
    }

    fn ajouter(&mut self, rang: u8, enfant: std::process::Child) {
        self.enfants.push((rang, enfant));
    }
}

impl Drop for SondesEnCours {
    fn drop(&mut self) {
        if self.enfants.is_empty() {
            return;
        }
        // Best-effort : sur ce chemin, quelque chose a déjà mal tourné avant
        // d'atteindre ce `drop` — c'est le signal qui compte, une écriture
        // ratée ici n'aggraverait rien que l'échec déjà journalisé en amont.
        if let Err(erreur) = std::fs::write(sonde::chemin_arret(), b"1") {
            tracing::error!(
                %erreur,
                "dépôt du signal d'arrêt échoué — sondes potentiellement orphelines"
            );
        }
        for (rang, enfant) in &mut self.enfants {
            match enfant.wait() {
                Ok(statut) => tracing::info!(sonde = *rang, ?statut, "sonde terminée"),
                Err(erreur) => {
                    tracing::error!(sonde = *rang, %erreur, "attente de la sonde échouée")
                }
            }
        }
    }
}

/// Marge ajoutée au pire cas de réessai d'ouverture (`D × 3 s`) pour borner
/// `attendre_le_verdict` : démarrage du processus enfant, jusqu'à D créations
/// de périphérique D3D11, et la latence d'écriture du fichier de verdict.
/// **Jugement, pas une mesure** — aucun relevé de ces chantiers ne chiffre ce
/// coût combiné.
const MARGE_ATTENTE_VERDICT: std::time::Duration = std::time::Duration::from_secs(15);

/// Lance les sondes en escalier et journalise la table de verdict.
fn conduire_les_sondes(
    processus: u8,
    duplications: u8,
    noms: &[String],
    garde: &mut compteurs::Garde<'_>,
) -> Result<()> {
    let executable = std::env::current_exe().context("chemin de l'exécutable courant")?;
    let mut sondes = SondesEnCours::nouvelle();
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
            // premier (remonté en tête, voir `multifenetre.rs::aiguiller`),
            // mais retirer la variable rend l'invariant explicite.
            .env_remove("MULTIFENETRE_PLAFOND")
            .spawn()
            .with_context(|| format!("lancement de la sonde {rang}"))?;
        sondes.ajouter(rang, enfant);

        let verdict = attendre_le_verdict(rang, duplications, garde)?;
        tracing::info!(sonde = rang, %verdict, "verdict reçu");
        // `MORTE` arrête l'escalier au même titre qu'un `KO` : la laisser
        // passer lancerait la sonde suivante alors que celle-ci ouvre peut-
        // être encore ses duplications (jusqu'à `D × 3 s` de seuls réessais),
        // rompant l'escalier EN SILENCE — exactement ce que le montage existe
        // pour éviter (voir le commentaire de tête du fichier).
        let arret = verdict.starts_with("KO") || verdict.starts_with("MORTE");
        verdicts.push((rang, verdict));
        if arret {
            // On s'arrête au premier refus (ou à la première sonde muette) :
            // c'est SON rang qui est la mesure. Continuer ne rendrait que des
            // refus dérivés.
            break;
        }
    }

    // Signal d'arrêt et attente des enfants, explicites ici plutôt que
    // laissés à la destruction de `sondes` en fin de fonction : le bilan
    // ci-dessous doit se journaliser une fois toutes les sondes relâchées, et
    // seul un `drop` placé à cet endroit précis le garantit. `SondesEnCours`
    // ferait le même geste de toute façon si cette ligne était sautée par un
    // chemin d'erreur — c'est tout l'intérêt de la garde.
    drop(sondes);

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
/// **Borné** sur `duplications × DUREE_FENETRE_OUVERTURE + MARGE_ATTENTE_VERDICT`
/// et non sur une constante fixe : `DesktopCapture::sur_sortie` retente
/// pendant `DUREE_FENETRE_OUVERTURE` (3 s) PAR duplication refusée, donc une
/// sonde à D=8 peut légitimement passer 24 s en seuls réessais avant
/// d'écrire son verdict. Une borne fixe à 30 s déclarerait `MORTE` une sonde
/// encore vivante à ce rang, et le porteur lancerait la suivante en
/// parallèle — l'escalier serait rompu sans qu'aucun message ne le dise.
///
/// Continue de battre le chien de garde pendant l'attente
/// (`Garde::battre_si_du`, sans trace par tour) : sans cela, plus un seul
/// ping ne partirait tant qu'une sonde n'a pas rendu son verdict.
fn attendre_le_verdict(
    rang: u8,
    duplications: u8,
    garde: &mut compteurs::Garde<'_>,
) -> Result<String> {
    let limite = crate::capture_reprise::DUREE_FENETRE_OUVERTURE
        .saturating_mul(u32::from(duplications))
        + MARGE_ATTENTE_VERDICT;
    let echeance = std::time::Instant::now() + limite;
    let chemin = sonde::chemin_verdict(rang);
    loop {
        // Course de lecture avec `sonde.rs`, qui écrit par `fs::write`
        // (troncature puis écriture, non atomique) : une lecture tombant
        // entre les deux rendrait un fichier lisible mais VIDE. Un contenu
        // vide n'est pas un verdict — le traiter comme tel ferait lire
        // `"".starts_with("KO")` = faux, donc un refus se lirait comme un
        // succès et l'escalier continuerait à tort.
        if let Ok(contenu) = std::fs::read_to_string(&chemin) {
            let contenu = contenu.trim();
            if !contenu.is_empty() {
                return Ok(contenu.to_string());
            }
        }
        if std::time::Instant::now() >= echeance {
            return Ok(format!("MORTE (aucun verdict en {} s)", limite.as_secs()));
        }
        garde.battre_si_du()?;
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
