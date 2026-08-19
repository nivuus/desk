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
        // Le contenu importe peu ici : CES TESTS-CI ne décodent rien, ils
        // éprouvent l'ordre. (Le module, lui, décode — voir `tests_lecteur`.)
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
