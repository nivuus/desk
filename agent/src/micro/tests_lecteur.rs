//! Les tests de `LecteurMicro` — le résidu, l'instrument de fréquence, le
//! bout en bout pur, et le plafond de dissimulation.
//!
//! **Sortis de `micro/tests.rs` au titre de la règle des 500 lignes**, et
//! sortis AVANT l'addition qu'ils devaient accueillir (le plafond de
//! dissimulation), pas après avoir franchi le plafond : c'est la doctrine du
//! dépôt, qui impose d'extraire et interdit de comprimer un commentaire pour
//! repasser sous la ligne. La coupure suit la bannière qui séparait déjà les
//! deux moitiés du fichier — les tests du `TamponGigue` restent chez le
//! voisin, les helpers `trames_d_un_ton` et `gauche` viennent ici avec les
//! seuls tests qui les emploient.
//!
//! Déclaré chez le parent par `#[path]`, comme son voisin, et pour la même
//! raison : l'usage est HORS de la « Convention de module enfant » de
//! `CLAUDE.md`, qui ne vise que les modules qu'on sort d'un parent
//! `#[cfg(windows)]`. Ici le parent est pur ; le seul motif est la taille, et
//! le précédent est `superviseur/table.rs`.

use super::*;

/// Encode `n` trames de 10 ms d'un ton pur à `f` hertz, en stéréo, telles que
/// le chantier A les produit — c'est-à-dire par le MÊME encodeur que celui du
/// produit, pas par un encodeur ad hoc plus complaisant.
fn trames_d_un_ton(f: f32, n: usize) -> Vec<TrameMicro> {
    use crate::opus::{OpusEncoder, FRAME_SAMPLES};
    let mut enc = OpusEncoder::new().expect("encodeur");
    let mut out = Vec::new();
    let mut phase = 0u64;
    for i in 0..n {
        let pcm: Vec<i16> = (0..FRAME_SAMPLES)
            .flat_map(|k| {
                let t = (phase + k as u64) as f32 / 48_000.0;
                let e = ((2.0 * std::f32::consts::PI * f * t).sin() * 12_000.0) as i16;
                [e, e]
            })
            .collect();
        phase += FRAME_SAMPLES as u64;
        out.push(TrameMicro {
            opus: enc.encode(&pcm).expect("encodage"),
            rtp_48k: (i * FRAME_SAMPLES) as u64,
            echantillons: FRAME_SAMPLES,
        });
    }
    out
}

/// Le canal gauche d'un tampon stéréo entrelacé.
fn gauche(entrelace: &[f32]) -> Vec<f32> {
    entrelace.iter().step_by(2).copied().collect()
}

/// Le paquet WASAPI ne fait presque jamais la taille d'une trame Opus : une
/// trame de 10 ms rend 480 échantillons par canal, et le tampon réclamé peut
/// en vouloir 441, 480 ou 1024. Le résidu est la pièce qui évite de jeter la
/// queue de chaque trame.
///
/// **Le critère est une ÉGALITÉ ÉCHANTILLON PAR ÉCHANTILLON**, entre un
/// lecteur rempli par petites tranches et un lecteur identique rempli d'un
/// seul coup. C'est exact, et ça ne se confond avec rien.
///
/// ⚠️ **La première rédaction jugeait à la FRÉQUENCE sur 83 ms, et elle
/// mesurait autre chose** : elle rendait 402 Hz pour un ton de 440 sur un
/// régime pourtant parfaitement nominal (`sauts`, `insertions`, `plc`,
/// `famines` tous nuls, relevé). L'écart venait de la mise en régime du
/// décodeur Opus, dont les premières trames pèsent un quart d'une fenêtre
/// aussi courte — le même test sur 1 s rend 437 Hz. **Un instrument juste
/// appliqué à la mauvaise fenêtre reste un mauvais instrument.**
#[test]
fn le_residu_survit_d_un_remplissage_a_l_autre() {
    let trames = trames_d_un_ton(440.0, 12);

    // Deux lecteurs nourris à l'identique, lus différemment.
    let mut par_tranches = LecteurMicro::new().unwrap();
    let mut d_un_coup = LecteurMicro::new().unwrap();
    for t in &trames {
        par_tranches.deposer(t.clone());
        d_un_coup.deposer(t.clone());
    }

    // 100 échantillons ENTRELACÉS par tranche, soit 50 par canal : jamais un
    // diviseur des 480 d'une trame, donc chaque tranche coupe une trame en
    // plein milieu. Sans résidu, la queue serait perdue à chaque fois.
    const TRANCHE: usize = 100;
    const TOURS: usize = 80;
    let mut recolte = Vec::new();
    for _ in 0..TOURS {
        let mut tranche = vec![0.0f32; TRANCHE];
        par_tranches.remplir(&mut tranche);
        recolte.extend_from_slice(&tranche);
    }

    let mut reference = vec![0.0f32; TRANCHE * TOURS];
    d_un_coup.remplir(&mut reference);

    // Le test ne doit pas comparer deux silences : sans cette garde, un
    // lecteur qui ne décoderait RIEN passerait l'égalité ci-dessous.
    assert!(
        reference.iter().any(|&e| e.abs() > 0.01),
        "la référence est muette : il n'y a rien à comparer"
    );
    assert_eq!(
        recolte.len(),
        reference.len(),
        "les deux découpages ne rendent pas le même nombre d'échantillons"
    );
    if let Some(i) = recolte.iter().zip(&reference).position(|(a, b)| a != b) {
        panic!(
            "divergence à l'échantillon {i} ({} contre {}) : le résidu perd la queue \
             des trames que le découpage coupe en deux",
            recolte[i], reference[i]
        );
    }

    // Et le régime est bien nominal : aucune correction n'a maquillé l'égalité.
    let c = par_tranches.compteurs();
    assert_eq!((c.sauts, c.insertions, c.plc, c.famines), (0, 0, 0, 0), "{c:?}");
}

/// Spec §8 « Silence » : le câble doit être alimenté EN CONTINU. Une
/// application qui écoute un tampon vide ne perçoit pas du silence, elle voit
/// un flux qui s'interrompt.
#[test]
fn un_lecteur_vide_rend_du_silence_et_jamais_une_erreur() {
    let mut l = LecteurMicro::new().unwrap();
    let mut sortie = vec![42.0f32; 480 * 2];
    l.remplir(&mut sortie);
    assert!(
        sortie.iter().all(|&e| e == 0.0),
        "le silence n'a pas été écrit"
    );
}

/// L'instrument de la recette (décision 8), éprouvé sur un signal SYNTHÉTIQUE
/// avant de servir à juger quoi que ce soit.
#[test]
fn la_frequence_d_un_ton_pur_est_retrouvee_a_un_pour_cent() {
    for cible in [220.0f32, 440.0, 1000.0] {
        let pcm: Vec<f32> = (0..48_000)
            .map(|n| (2.0 * std::f32::consts::PI * cible * n as f32 / 48_000.0).sin())
            .collect();
        let f = frequence_par_passages_a_zero(&pcm, 48_000).expect("ton mesurable");
        assert!(
            (f - cible).abs() / cible < 0.01,
            "cible {cible}, mesuré {f}"
        );
    }
}

/// ⚠️ LE test qui rend l'instrument crédible : il doit REFUSER ce qui n'est
/// pas un ton. Sans lui, « la fréquence vaut 440 » ne prouverait rien de plus
/// qu'un compte d'octets — et c'est exactement la doctrine que ce dépôt a
/// payée en D7.
#[test]
fn le_silence_et_le_bruit_ne_rendent_pas_une_frequence_credible() {
    assert!(
        frequence_par_passages_a_zero(&vec![0.0; 48_000], 48_000).is_none(),
        "le silence a rendu une fréquence"
    );
    // Bruit déterministe (générateur congruentiel, aucune dépendance neuve).
    let mut x = 12_345u32;
    let bruit: Vec<f32> = (0..48_000)
        .map(|_| {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            (x >> 16) as f32 / 32_768.0 - 1.0
        })
        .collect();
    let f = frequence_par_passages_a_zero(&bruit, 48_000);
    assert!(
        f.is_none_or(|f| (f - 440.0).abs() > 100.0),
        "du bruit a été pris pour un 440 Hz : {f:?}"
    );
}

/// ⚠️ **LE test qui exerce la BANDE MORTE**, et il a fallu une mutation pour
/// découvrir qu'aucun autre ne le faisait : retirer la bande morte laisse vert
/// `le_silence_et_le_bruit_ne_rendent_pas_une_frequence_credible` (du bruit pur
/// rend ~12 000 Hz avec ou sans elle, donc toujours loin de 440).
///
/// Ce que la bande morte évite vraiment, c'est le cas MIXTE : un vrai ton, avec
/// du bruit de faible amplitude qui traverse zéro entre deux passages
/// légitimes. Chaque traversée parasite y ajoute deux changements de signe, et
/// la fréquence mesurée explose alors que le signal, lui, est bien un 440 Hz.
#[test]
fn un_ton_bruite_reste_mesure_a_sa_frequence() {
    let mut x = 987_654u32;
    let pcm: Vec<f32> = (0..48_000)
        .map(|n| {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let bruit = ((x >> 16) as f32 / 32_768.0 - 1.0) * 0.05;
            (2.0 * std::f32::consts::PI * 440.0 * n as f32 / 48_000.0).sin() + bruit
        })
        .collect();
    let f = frequence_par_passages_a_zero(&pcm, 48_000).expect("ton mesurable");
    assert!(
        (f - 440.0).abs() / 440.0 < 0.01,
        "un ton de 440 Hz légèrement bruité est mesuré {f} Hz : les traversées \
         parasites autour de zéro sont comptées comme des passages"
    );
}

/// Le bout en bout PUR : encoder un 440 Hz, le passer par le tampon de gigue,
/// le décoder, et le retrouver à sa fréquence. **C'est le critère de la
/// recette de E1, joué sans VM et sans navigateur** — ce qui restera à la
/// recette, c'est le trajet WebRTC, pas la chaîne de traitement.
#[test]
fn un_ton_encode_traverse_le_tampon_et_ressort_a_sa_frequence() {
    let trames = trames_d_un_ton(440.0, 106);
    let mut l = LecteurMicro::new().unwrap();

    // Amorcer à 6 trames (60 ms) : franchement DANS la bande morte de la
    // correction de dérive. À une seule trame en réserve on serait sous
    // SEUIL_INSERTION et le lecteur insérerait du silence à chaque tour ; à
    // plus de douze on serait au-dessus de SEUIL_SAUT et il en sauterait.
    let mut it = trames.into_iter();
    for _ in 0..6 {
        l.deposer(it.next().unwrap());
    }

    let mut recolte = Vec::new();
    for t in it {
        l.deposer(t);
        let mut tranche = vec![0.0f32; 480 * 2];
        l.remplir(&mut tranche);
        recolte.extend_from_slice(&tranche);
    }

    let c = l.compteurs();
    assert_eq!(
        (c.sauts, c.insertions, c.plc, c.famines),
        (0, 0, 0, 0),
        "le régime n'est pas nominal : la mesure de fréquence porterait sur \
         un signal corrigé, pas sur le signal transmis — {c:?}"
    );

    let g = gauche(&recolte);
    let f = frequence_par_passages_a_zero(&g, 48_000).expect("ton mesurable");
    eprintln!("bout en bout pur : {} échantillons, {f} Hz mesurés", g.len());
    assert!(
        (f - 440.0).abs() / 440.0 < 0.02,
        "le ton n'a pas traversé : mesuré {f} Hz au lieu de 440"
    );
}

// ----------------------------------------------------------------------
// Le plafond de dissimulation (correctif du chantier E)
// ----------------------------------------------------------------------

/// Nourrit un lecteur en régime nominal, puis coupe l'émetteur et rend ce
/// que le puits a entendu pendant `reveils` réveils de 10 ms de famine.
///
/// **Le montage imite le consommateur réel** (`demarrage/micro.rs`) : un
/// réveil toutes les 10 ms, 480 trames par canal. Une famine mesurée d'un
/// seul `remplir` géant ne serait pas le même chemin.
fn famine_apres_un_ton(reveils: usize) -> (Vec<f32>, LecteurMicro) {
    let mut it = trames_d_un_ton(440.0, 30).into_iter();
    let mut l = LecteurMicro::new().unwrap();
    // Amorcer à 6 trames (60 ms) : franchement DANS la bande morte de la
    // correction de dérive, comme le bout en bout pur ci-dessus.
    for _ in 0..6 {
        l.deposer(it.next().unwrap());
    }
    for t in it {
        l.deposer(t);
        let mut tranche = vec![0.0f32; 480 * 2];
        l.remplir(&mut tranche);
    }

    // L'émetteur se tait. Plus une seule trame ne sera déposée.
    let mut recolte = Vec::with_capacity(reveils * 480 * 2);
    for _ in 0..reveils {
        let mut tranche = vec![0.0f32; 480 * 2];
        l.remplir(&mut tranche);
        recolte.extend_from_slice(&tranche);
    }
    (recolte, l)
}

/// ⚠️ **LE test du défaut mesuré par la recette E1.** Pendant 60 s de silence
/// du navigateur — DTX nominal, `packetsSent` figé —, le lecteur appelait
/// `dissimuler()` à chaque trame manquante sans aucune borne, et le journal
/// relevait `plc = 50/s`, `crete` entre 0,53 et 0,67 et une `frequence_hz`
/// errant entre 308 et 393 Hz. **La dissimulation fabriquait un bourdon
/// continu**, qui au bloc E2 sortirait sur le câble virtuel, donc dans
/// l'application Windows.
///
/// La cause est dans libopus et n'est pas un défaut : sa dissimulation
/// CELT bascule sur du bruit dès la 6ᵉ perte consécutive, puis fait décroître
/// l'énergie **jusqu'au plancher de bruit de fond et l'y maintient**
/// (`celt/celt_decoder.c:537` et `:562`, `MAX16(backgroundLogE, ...)`). Elle
/// converge donc vers du bruit de confort et ne s'arrête JAMAIS d'elle-même :
/// borner la durée dissimulée est à NOUS.
#[test]
fn une_famine_prolongee_cesse_de_dissimuler_et_rend_du_silence() {
    const REVEILS: usize = 100; // 1 s de famine
    const QUEUE_DEPUIS: usize = 50; // on juge la seconde moitié
    let (recolte, lecteur) = famine_apres_un_ton(REVEILS);

    // Garde anti-vacuité : sans elle, un lecteur qui ne dissimulerait JAMAIS
    // rien passerait ce test sans avoir exercé quoi que ce soit.
    let c = lecteur.compteurs();
    assert!(
        c.plc > 0,
        "aucune dissimulation n'a eu lieu : le test ne mesure rien — {c:?}"
    );

    let queue = &recolte[QUEUE_DEPUIS * 480 * 2..];
    let crete = queue.iter().fold(0.0f32, |m, e| m.max(e.abs()));
    let f = frequence_par_passages_a_zero(&gauche(queue), 48_000);
    assert_eq!(
        crete, 0.0,
        "après 500 ms de famine le puits rend encore du signal (crête {crete:.3}, \
         fréquence {f:?}) : la dissimulation Opus n'est bornée par rien et \
         fabrique un bourdon continu"
    );
    assert_eq!(
        f, None,
        "après 500 ms de famine le puits rend encore une fréquence dominante"
    );
}

/// ⚠️ **L'OBSERVABILITÉ, sans laquelle la correction ne serait pas
/// falsifiable.** Une recette doit pouvoir distinguer « la dissimulation
/// travaille » de « le plafond a mordu et le puits se tait » : les deux
/// rendent des famines, et sans deux compteurs DISJOINTS elles se lisent
/// identiquement au journal.
///
/// Le test vérifie les trois propriétés qui rendent la trace lisible :
/// `plc` s'arrête, `plc_plafonnees` prend le relais, et la somme des deux
/// couvre bien toutes les trames manquantes rendues.
#[test]
fn le_plafond_est_compte_a_part_de_la_dissimulation() {
    const REVEILS: usize = 100;
    let (_, lecteur) = famine_apres_un_ton(REVEILS);
    let c = lecteur.compteurs();

    assert!(
        c.plc_plafonnees > 0,
        "le plafond a mordu (le puits se tait) mais rien ne le compte : une \
         recette ne peut pas distinguer ce silence-là d'une dissimulation qui \
         travaille — {c:?}"
    );
    // La dissimulation a bien eu lieu AVANT le plafond, et elle est bornée par
    // lui : 200 ms de trames de 10 ms font au plus 20 dissimulations.
    assert!(
        (1..=20).contains(&c.plc),
        "les dissimulations réellement produites devraient tenir dans les \
         200 ms du plafond (au plus 20 trames de 10 ms) — {c:?}"
    );
    // Et les deux compteurs se partagent EXACTEMENT les trames manquantes :
    // une trame due est soit dissimulée, soit rendue en silence, jamais les
    // deux ni ni l'une ni l'autre.
    assert_eq!(
        c.plc + c.plc_plafonnees,
        c.famines + c.insertions,
        "des trames manquantes ne sont comptées ni comme dissimulées ni comme \
         plafonnées — {c:?}"
    );
}

/// ⚠️ **LE test qui interdit au plafond de condamner la session.** Une fois le
/// plafond atteint, le puits se tait — mais le retour de la parole doit rendre
/// le budget entier, faute de quoi la PROCHAINE perte réseau, si courte
/// soit-elle, ne serait plus jamais dissimulée. C'est le seul test de ce
/// fichier qui exerce la remise à zéro sur le chemin de PRODUCTION : les tests
/// unitaires de `micro/dissimulation.rs` couvrent la règle, jamais son câblage.
#[test]
fn apres_le_plafond_la_parole_qui_revient_rend_le_budget_entier() {
    let toutes = trames_d_un_ton(440.0, 70);
    let mut it = toutes.into_iter();
    let mut l = LecteurMicro::new().unwrap();
    let mut reveil = |l: &mut LecteurMicro| {
        let mut tranche = vec![0.0f32; 480 * 2];
        l.remplir(&mut tranche);
        tranche
    };

    for _ in 0..6 {
        l.deposer(it.next().unwrap());
    }
    for _ in 0..24 {
        l.deposer(it.next().unwrap());
        reveil(&mut l);
    }

    // Première famine, assez longue pour épuiser le plafond.
    for _ in 0..60 {
        reveil(&mut l);
    }
    let apres_famine_1 = l.compteurs();
    assert!(
        apres_famine_1.plc_plafonnees > 0,
        "le plafond n'a pas mordu : la suite du test ne prouverait rien — {apres_famine_1:?}"
    );

    // La parole revient : trente trames, une par réveil.
    let mut entendu = Vec::new();
    for _ in 0..30 {
        l.deposer(it.next().unwrap());
        entendu.extend_from_slice(&reveil(&mut l));
    }

    // Le ton est bien revenu — sans quoi « le budget est rendu » se lirait sur
    // un puits qui ne rend plus rien du tout.
    let crete = entendu.iter().fold(0.0f32, |m, e| m.max(e.abs()));
    assert!(
        crete > 0.05,
        "le ton n'est pas revenu après le plafond (crête {crete:.3}) : le puits \
         reste muet alors que des trames arrivent"
    );

    // Seconde famine, COURTE : trois réveils, l'ordre de grandeur d'une perte
    // réseau. Elle doit être dissimulée comme avant, donc faire croître `plc`.
    for _ in 0..3 {
        reveil(&mut l);
    }
    let apres_famine_2 = l.compteurs();
    assert!(
        apres_famine_2.plc > apres_famine_1.plc,
        "une perte courte survenue APRÈS que le plafond a mordu n'est plus \
         dissimulée : le budget n'est pas rendu au retour de la parole — \
         {apres_famine_1:?} puis {apres_famine_2:?}"
    );
}
