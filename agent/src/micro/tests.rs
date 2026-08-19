//! Les tests de `micro.rs`, sortis dans leur propre fichier au titre de la
//! règle des 500 lignes : le module a franchi le plafond en gagnant la
//! correction de dérive (chantier E, bloc E1, tâche 5), et la doctrine du
//! dépôt impose d'EXTRAIRE, jamais de compresser un commentaire pour repasser
//! sous la ligne.
//!
//! Déclaré chez le parent par `#[path]` — l'usage explicitement HORS de la
//! « Convention de module enfant » de `CLAUDE.md`, qui ne vise que les modules
//! qu'on sort d'un parent `#[cfg(windows)]` pour les compiler sur l'hôte. Ici
//! le parent est déjà pur ; le seul motif est la taille, et le précédent est
//! `superviseur/table.rs`.

use super::*;
use std::time::Duration;

/// Construit une trame de `ms` millisecondes commençant à `rtp_48k`.
fn trame(rtp_48k: u64, ms: u64) -> TrameMicro {
    let echantillons = (48_000 * ms / 1000) as usize;
    TrameMicro {
        // Le contenu importe peu ici : ce module ne décode rien, il ordonne.
        // Un octet dérivé de l'horodatage suffit à identifier la trame.
        opus: vec![(rtp_48k % 251) as u8, 0x11],
        rtp_48k,
        echantillons,
    }
}

fn tampon() -> TamponGigue {
    TamponGigue::new(CIBLE, PLAFOND)
}

/// Spec §11 : « une arrivée désordonnée est restituée dans l'ordre ».
///
/// ⚠️ str0m réordonne DÉJÀ (`packet/buffer_rx.rs`), et la décision 2 de ce
/// plan ramène sa profondeur à 2 : sa garantie est donc VOLONTAIREMENT
/// affaiblie, et c'est ici que l'ordre se rattrape. Ce test n'est pas
/// redondant avec str0m, il est le filet de ce que la décision 2 lui
/// retire.
#[test]
fn une_arrivee_desordonnee_est_restituee_dans_l_ordre() {
    let mut t = tampon();
    t.deposer(trame(1920, 20));
    t.deposer(trame(0, 20));
    t.deposer(trame(960, 20));

    let mut vus = Vec::new();
    for _ in 0..3 {
        match t.retirer() {
            Retrait::Trame(tr) => vus.push(tr.rtp_48k),
            autre => panic!("retrait inattendu : {autre:?}"),
        }
    }
    assert_eq!(vus, vec![0, 960, 1920]);
    assert_eq!(t.compteurs().hors_ordre, 2, "les deux arrivées tardives");
}

#[test]
fn un_doublon_est_compte_et_jete() {
    let mut t = tampon();
    t.deposer(trame(960, 20));
    t.deposer(trame(960, 20));

    assert_eq!(t.compteurs().deposees, 2);
    assert_eq!(t.compteurs().doublons, 1);
    assert!(matches!(t.retirer(), Retrait::Trame(tr) if tr.rtp_48k == 960));
    // …et il n'en reste pas une seconde copie.
    assert!(matches!(t.retirer(), Retrait::Manquante));
}

/// Spec §11 : « à saturation, c'est la trame la plus ANCIENNE qui part ».
/// C'est l'inverse de l'émission (le chantier A jette le vieux pour garder
/// le frais) — ici on subit une ligne de temps distante.
#[test]
fn a_saturation_c_est_la_plus_ancienne_qui_part() {
    let mut t = tampon();
    // PLAFOND = 200 ms, soit 10 trames de 20 ms. En déposer 12 en ordre.
    for i in 0..12u64 {
        t.deposer(trame(i * 960, 20));
    }
    assert!(
        t.occupation() <= PLAFOND,
        "occupation {:?} au-dessus du plafond {PLAFOND:?}",
        t.occupation()
    );
    assert!(t.compteurs().jetees_saturation >= 2);

    // La PREMIÈRE trame rendue n'est plus la 0 : ce sont les plus
    // anciennes qui sont parties, pas les plus récentes.
    let premiere = match t.retirer() {
        Retrait::Trame(tr) => tr.rtp_48k,
        autre => panic!("retrait inattendu : {autre:?}"),
    };
    assert!(
        premiere > 0,
        "la trame la plus ancienne (rtp 0) est encore là : c'est la plus RÉCENTE \
         qui a été jetée"
    );

    // …et la plus récente, elle, a survécu.
    let mut derniere = premiere;
    while let Retrait::Trame(tr) = t.retirer() {
        derniere = tr.rtp_48k;
    }
    assert_eq!(derniere, 11 * 960, "la trame la plus récente a été jetée");
}

/// « en famine, le puits rend du silence sans jamais bloquer » (spec §11).
#[test]
fn en_famine_le_retrait_rend_manquante_sans_bloquer() {
    let mut t = tampon();
    for _ in 0..5 {
        assert!(matches!(t.retirer(), Retrait::Manquante));
    }
    assert_eq!(t.compteurs().famines, 5);
    assert_eq!(t.occupation(), Duration::ZERO);
}

/// Le FEC ne sert QUE si la suivante est déjà là (spec §8) : la
/// reconstruction se fait depuis la trame qui SUIT celle qui manque.
#[test]
fn une_trame_absente_dont_la_suivante_est_la_donne_reconstruire() {
    let mut t = tampon();
    t.deposer(trame(0, 20));
    t.deposer(trame(1920, 20)); // la trame 960 manque

    assert!(matches!(t.retirer(), Retrait::Trame(tr) if tr.rtp_48k == 0));
    match t.retirer() {
        Retrait::Reconstruire { suivante } => {
            assert_eq!(suivante, trame(1920, 20).opus, "ce n'est pas la SUIVANTE");
        }
        autre => panic!("attendu Reconstruire, reçu {autre:?}"),
    }
    assert_eq!(t.compteurs().fec, 1);
    // La suivante n'a pas été consommée : elle se joue au tour d'après.
    assert!(matches!(t.retirer(), Retrait::Trame(tr) if tr.rtp_48k == 1920));
}

#[test]
fn une_trame_absente_sans_suivante_donne_manquante() {
    let mut t = tampon();
    t.deposer(trame(0, 20));
    assert!(matches!(t.retirer(), Retrait::Trame(tr) if tr.rtp_48k == 0));
    assert!(matches!(t.retirer(), Retrait::Manquante));
    assert_eq!(t.compteurs().fec, 0, "aucune suivante : rien à reconstruire");
    assert_eq!(t.compteurs().famines, 1);
}

/// Une trame qui arrive APRÈS que sa place est passée n'est pas jouée hors
/// de son tour : elle est périmée, comptée, et jetée.
#[test]
fn une_trame_perimee_est_comptee_et_jetee() {
    let mut t = tampon();
    t.deposer(trame(960, 20));
    assert!(matches!(t.retirer(), Retrait::Trame(_)));
    t.deposer(trame(0, 20)); // arrive après son tour
    assert_eq!(t.compteurs().jetees_perimees, 1);
    assert!(matches!(t.retirer(), Retrait::Manquante));
}

/// L'invariant du module, et il est STRUCTUREL : `deposer` ne rend rien et
/// ne peut donc pas faire attendre la boucle de transport.
///
/// ⚠️ **Aucune mutation ne peut faire échouer ce test**, et c'est noté
/// plutôt qu'habillé d'un contrôle de façade : la propriété est portée par
/// la SIGNATURE (`fn deposer(&mut self, trame: TrameMicro)`, sans valeur de
/// retour et sans `Result`), et le compilateur la tient. Ce test vérifie
/// seulement que 10 000 dépôts d'affilée ne divergent pas et laissent le
/// tampon borné.
#[test]
fn deposer_ne_bloque_jamais_meme_a_saturation() {
    let mut t = tampon();
    for i in 0..10_000u64 {
        t.deposer(trame(i * 960, 20));
    }
    assert_eq!(t.compteurs().deposees, 10_000);
    assert!(
        t.occupation() <= PLAFOND,
        "le tampon a enflé sans borne : {:?}",
        t.occupation()
    );
}

/// Spec §8 : au-dessus de 120 ms d'occupation on saute une trame, en
/// dessous de 20 ms on en insère une. « Grossier, audible une fois par
/// plusieurs minutes » — et assumé.
#[test]
fn au_dela_du_seuil_haut_une_trame_est_sautee_et_comptee() {
    let mut t = tampon();
    // 7 trames de 20 ms = 140 ms, au-dessus de SEUIL_SAUT (120 ms) et sous
    // le PLAFOND (200 ms) : c'est la DÉRIVE qu'on exerce, pas la saturation.
    for i in 0..7u64 {
        t.deposer(trame(i * 960, 20));
    }
    assert_eq!(t.compteurs().jetees_saturation, 0, "c'est la dérive, pas la saturation");

    match t.retirer() {
        // La trame 0 a été sautée : c'est la 960 qui sort.
        Retrait::Trame(tr) => assert_eq!(tr.rtp_48k, 960, "aucune trame n'a été sautée"),
        autre => panic!("retrait inattendu : {autre:?}"),
    }
    assert_eq!(t.compteurs().sauts, 1);
    assert_eq!(t.compteurs().insertions, 0);
    // Et le saut n'est pas rattrapé par une reconstruction FEC du trou
    // qu'il vient de creuser — ce serait un no-op déguisé.
    assert_eq!(t.compteurs().fec, 0);
}

#[test]
fn en_dessous_du_seuil_bas_une_trame_est_inseree_et_comptee() {
    let mut t = tampon();
    // Une seule trame de 10 ms : 10 ms d'occupation, sous SEUIL_INSERTION.
    t.deposer(trame(0, 10));

    assert!(matches!(t.retirer(), Retrait::Manquante));
    assert_eq!(t.compteurs().insertions, 1);
    assert_eq!(t.compteurs().sauts, 0);
    // ⚠️ L'insertion ne CONSOMME PAS : l'occupation n'a pas bougé, et c'est
    // ce qui lui permet de croître jusqu'à la bande morte.
    assert_eq!(t.occupation(), Duration::from_millis(10));
    // …et ce n'est pas une famine : la trame est là, c'est nous qui
    // attendons.
    assert_eq!(t.compteurs().famines, 0);
}

/// Entre les deux seuils, RIEN ne bouge : c'est l'hystérésis, et son
/// absence ferait osciller le tampon à chaque trame.
#[test]
fn entre_les_deux_seuils_aucune_correction_n_est_appliquee() {
    let mut t = tampon();
    // 4 trames de 20 ms = 80 ms, franchement entre 20 et 120.
    for i in 0..4u64 {
        t.deposer(trame(i * 960, 20));
    }
    assert!(matches!(t.retirer(), Retrait::Trame(tr) if tr.rtp_48k == 0));
    assert_eq!(t.compteurs().sauts, 0, "un saut dans la bande morte");
    assert_eq!(t.compteurs().insertions, 0, "une insertion dans la bande morte");
}

/// « Aucun n'est silencieux » (spec §8). Chaque correction incrémente son
/// compteur, et ce test le vérifie sur les DEUX à la fois, dans une même
/// vie de tampon — un compteur partagé par les deux passerait les deux
/// tests précédents pris séparément.
#[test]
fn chaque_correction_a_son_compteur() {
    let mut t = tampon();
    for i in 0..7u64 {
        t.deposer(trame(i * 960, 20));
    }
    // Trop de retard : on saute.
    assert!(matches!(t.retirer(), Retrait::Trame(_)));
    assert_eq!((t.compteurs().sauts, t.compteurs().insertions), (1, 0));

    // On vide jusqu'à passer sous le seuil bas.
    while t.occupation() >= SEUIL_INSERTION {
        t.retirer();
    }
    let sauts_avant = t.compteurs().sauts;
    // Il reste de quoi ne pas être en famine, mais pas assez pour jouer.
    t.deposer(trame(100_000, 10));
    assert!(t.occupation() < SEUIL_INSERTION);
    assert!(matches!(t.retirer(), Retrait::Manquante));

    assert_eq!(t.compteurs().insertions, 1, "l'insertion n'a pas son compteur");
    assert_eq!(
        t.compteurs().sauts,
        sauts_avant,
        "l'insertion a incrémenté le compteur des SAUTS : les deux corrections \
         partagent un compteur"
    );
}

// ----------------------------------------------------------------------
// LecteurMicro, le résidu, et l'instrument de fréquence (tâche 6)
// ----------------------------------------------------------------------

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
