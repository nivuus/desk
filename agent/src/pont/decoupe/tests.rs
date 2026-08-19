use super::*;
use proto::fichiers::TAILLE_TRAME_MAX;

#[test]
fn une_plage_de_taille_exactement_taille_trame_max_fait_un_seul_morceau() {
    // Au seuil EXACT : un morceau, pas deux dont un vide. C'est l'erreur
    // classique d'une boucle qui teste `>=` là où il faut `>`.
    let m = decouper(0, TAILLE_TRAME_MAX as u64, TAILLE_TRAME_MAX);
    assert_eq!(m.len(), 1);
    assert_eq!(m[0], Morceau { position: 0, longueur: TAILLE_TRAME_MAX as u32 });

    // …et un octet de plus en fait exactement deux, dont le second d'un octet.
    let m = decouper(0, TAILLE_TRAME_MAX as u64 + 1, TAILLE_TRAME_MAX);
    assert_eq!(m.len(), 2);
    assert_eq!(m[1], Morceau { position: TAILLE_TRAME_MAX as u64, longueur: 1 });
}

#[test]
fn une_plage_de_taille_nulle_ne_fait_aucun_morceau() {
    // Un morceau de longueur 0 provoquerait une trame de réponse vide, que
    // rien ne distinguerait d'une fin de fichier.
    assert!(decouper(0, 0, TAILLE_TRAME_MAX).is_empty());
    assert!(decouper(4096, 0, TAILLE_TRAME_MAX).is_empty());
}

#[test]
fn la_somme_des_longueurs_egale_la_longueur_demandee() {
    // Sur cent tailles de 1 à 3·max : c'est la propriété qui fait échouer le
    // condensat SHA-256 du critère (2) si elle est fausse d'un seul octet.
    let max = 1000usize;
    for n in 1..=100u64 {
        let longueur = n * 3 * max as u64 / 100 + 1;
        let somme: u64 = decouper(7, longueur, max).iter().map(|m| u64::from(m.longueur)).sum();
        assert_eq!(somme, longueur, "longueur demandée {longueur}");
    }
}

#[test]
fn les_morceaux_sont_contigus_et_croissants() {
    // Ni trou (le fichier serait tronqué au milieu), ni recouvrement (des
    // octets seraient écrits deux fois).
    let morceaux = decouper(1_000, 10_000, 3_000);
    assert_eq!(morceaux.len(), 4);
    let mut attendu = 1_000u64;
    for m in &morceaux {
        assert_eq!(m.position, attendu, "trou ou recouvrement avant {m:?}");
        attendu += u64::from(m.longueur);
    }
    assert_eq!(attendu, 11_000);
    // Et aucun morceau vide au milieu.
    assert!(morceaux.iter().all(|m| m.longueur > 0));
}

#[test]
fn le_premier_morceau_part_de_la_position_demandee() {
    // La position est ignorée par toute implémentation qui repartirait de 0 :
    // ProjFS demande couramment une plage AU MILIEU d'un fichier.
    assert_eq!(decouper(1_234_567, 10, 4)[0].position, 1_234_567);
    assert_eq!(decouper(0, 10, 4)[0].position, 0);
}

#[test]
fn une_longueur_superieure_a_u32_max_se_decoupe_quand_meme() {
    // `PRJ_GET_FILE_DATA_CB` reçoit `byteoffset: u64` : le fichier peut
    // dépasser 4 Gio. Une implémentation qui convertirait la longueur en `u32`
    // avant de découper perdrait tout au-delà du premier tour.
    let longueur = u64::from(u32::MAX) + 5;
    let morceaux = decouper(u64::from(u32::MAX), longueur, TAILLE_TRAME_MAX);
    let somme: u64 = morceaux.iter().map(|m| u64::from(m.longueur)).sum();
    assert_eq!(somme, longueur);
    assert_eq!(morceaux[0].position, u64::from(u32::MAX));
    // Le dernier morceau finit bien au-delà de 4 Gio + la position de départ.
    let dernier = morceaux.last().unwrap();
    assert_eq!(
        dernier.position + u64::from(dernier.longueur),
        u64::from(u32::MAX) + longueur
    );
}

#[test]
fn un_max_plus_grand_que_u32_max_ne_deborde_pas_a_la_conversion() {
    // Sur une cible 64 bits, `usize` est plus large que `u32` : un `max` reçu
    // au-delà de `u32::MAX` ferait déborder la conversion de longueur. Le
    // module le borne AVANT de convertir, et ce test est la seule chose qui
    // l'établisse — aucun appelant de F1 ne passe une telle valeur.
    let morceaux = decouper(0, u64::from(u32::MAX) + 2, usize::MAX);
    let somme: u64 = morceaux.iter().map(|m| u64::from(m.longueur)).sum();
    assert_eq!(somme, u64::from(u32::MAX) + 2);
    assert!(morceaux.iter().all(|m| m.longueur > 0));
}
