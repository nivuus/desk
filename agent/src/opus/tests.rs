//! Les tests d'`opus.rs`, sortis dans leur propre fichier au titre de la
//! règle des 500 lignes : `opus.rs` a franchi le plafond en gagnant son
//! DÉCODEUR (chantier E, bloc E1, tâche 3), et la doctrine du dépôt impose
//! d'EXTRAIRE, jamais de compresser un commentaire pour repasser sous la
//! ligne.
//!
//! Déclaré chez le parent par `#[path]` — c'est l'usage explicitement HORS de
//! la « Convention de module enfant » de `CLAUDE.md`, qui ne vise que les
//! modules qu'on sort d'un parent `#[cfg(windows)]` pour les compiler sur
//! l'hôte. Ici le parent est déjà portable ; le seul motif est la taille, et
//! le précédent est `superviseur/table.rs`.

use super::*;

/// Énergie du signal à `freq` hertz sur le canal gauche, par l'algorithme
/// de Goertzel.
///
/// Insensible au retard : le codec introduit une latence algorithmique
/// (312 échantillons en `Application::Audio`, voir le docstring de
/// `OpusEncoder`), donc une comparaison échantillon par échantillon avec
/// l'entrée échouerait pour une raison qui n'a rien à voir avec la
/// fidélité.
fn energie_a(pcm: &[i16], freq: f64) -> f64 {
    let n = pcm.len() / CHANNELS;
    let w = 2.0 * std::f64::consts::PI * freq / SAMPLE_RATE_HZ as f64;
    let coeff = 2.0 * w.cos();
    let (mut s1, mut s2) = (0.0f64, 0.0f64);
    for i in 0..n {
        let s0 = pcm[i * CHANNELS] as f64 + coeff * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    (s1 * s1 + s2 * s2 - coeff * s1 * s2).sqrt() / n as f64
}

/// `n` échantillons entrelacés stéréo d'une sinusoïde à `freq` hertz,
/// démarrant à l'échantillon `depuis` pour rester continue d'une trame à
/// la suivante.
fn ton(freq: f64, depuis: usize, n: usize) -> Vec<i16> {
    (0..n)
        .flat_map(|i| {
            let phase = (depuis + i) as f64 * 2.0 * std::f64::consts::PI * freq
                / SAMPLE_RATE_HZ as f64;
            let v = (phase.sin() * 12_000.0) as i16;
            [v, v]
        })
        .collect()
}

#[test]
fn une_trame_de_10_ms_vaut_480_echantillons_par_canal() {
    assert_eq!(FRAME_SAMPLES, 480);
    assert_eq!(FRAME_INTERLEAVED, 960);
}

#[test]
fn refuse_une_trame_de_mauvaise_taille() {
    let mut encodeur = OpusEncoder::new().unwrap();
    let err = encodeur.encode(&vec![0i16; 1000]).unwrap_err();
    assert!(
        err.to_string().contains("960"),
        "le message doit nommer la taille attendue, obtenu : {err}"
    );
}

#[test]
fn un_ton_encode_puis_decode_reste_le_meme_ton() {
    // Vérifier que l'encodeur rend des octets ne prouverait rien : du
    // bruit en rendrait tout autant. On décode en retour et on vérifie
    // que l'énergie reste concentrée sur la fréquence d'origine.
    let mut encodeur = OpusEncoder::new().unwrap();
    // Contrairement aux `use`, les expressions résolvent le chemin `opus::` en parcourant
    // d'abord l'arbre des modules du crate courant. N'y trouvant rien nommé `opus`,
    // elles remontent au prélude (crates externes), d'où le crate `opus`. Pas de `::` requis.
    let mut decodeur =
        opus::Decoder::new(SAMPLE_RATE_HZ, opus::Channels::Stereo).unwrap();

    let mut sortie: Vec<i16> = Vec::new();
    for t in 0..20 {
        let paquet = encodeur
            .encode(&ton(440.0, t * FRAME_SAMPLES, FRAME_SAMPLES))
            .unwrap();
        let mut trame = vec![0i16; FRAME_INTERLEAVED];
        decodeur.decode(&paquet, &mut trame, false).unwrap();
        sortie.extend_from_slice(&trame);
    }

    let a_440 = energie_a(&sortie, 440.0);
    let a_1500 = energie_a(&sortie, 1500.0);
    assert!(
        a_440 > 100.0 * a_1500,
        "l'énergie doit rester concentrée sur 440 Hz : 440 Hz = {a_440:.1}, 1500 Hz = {a_1500:.1}"
    );

    let entree = energie_a(&ton(440.0, 0, 20 * FRAME_SAMPLES), 440.0);
    let rapport = a_440 / entree;
    assert!(
        (0.8..=1.2).contains(&rapport),
        "l'amplitude restituée doit rester proche de l'originale, rapport = {rapport:.3}"
    );
}

#[test]
fn le_silence_prolonge_retombe_a_quelques_octets_par_trame() {
    // DTX met plusieurs trames à converger : les cinq premières valent
    // encore 217 puis 161 octets. Mesurer trop tôt conclurait à tort que
    // DTX ne fonctionne pas. On regarde donc la QUEUE, pas le début.
    let mut encodeur = OpusEncoder::new().unwrap();
    let silence = vec![0i16; FRAME_INTERLEAVED];
    let tailles: Vec<usize> = (0..40)
        .map(|_| encodeur.encode(&silence).unwrap().len())
        .collect();

    let queue = &tailles[35..];
    assert!(
        queue.iter().all(|&t| t <= 8),
        "en régime établi, une trame de silence doit tenir en quelques octets, obtenu : {queue:?}"
    );
}

#[test]
fn le_pourcentage_de_perte_est_borne() {
    let mut enc = OpusEncoder::new().expect("encodeur");

    enc.set_packet_loss_perc(0).expect("0 accepté");
    enc.set_packet_loss_perc(25).expect("25 accepté");

    // Hors bornes : borné plutôt que refusé. Le contrôleur borne déjà,
    // mais cette fonction est publique et ne doit pas laisser passer une
    // valeur que libopus rejetterait avec une erreur opaque.
    enc.set_packet_loss_perc(-5).expect("valeur négative bornée");
    enc.set_packet_loss_perc(300).expect("valeur excessive bornée");
}

#[test]
fn une_perte_declaree_change_reellement_l_encodage() {
    // Preuve que le FEC in-band n'est plus inerte.
    //
    // ATTENTION à la direction : ce test ne mesure PAS une augmentation de
    // taille. Sous un débit cible fixe, LBRR ne s'ajoute pas aux octets,
    // il les redistribue — `compute_silk_rate_for_hybrid`
    // (opus_encoder.c:751) emploie des tables de débit différentes selon
    // que le FEC est codé ou non. Les paquets peuvent donc RÉTRÉCIR.
    //
    // Ce qui fait preuve, c'est que la sortie DIFFÈRE : en mode CELT seul
    // (`Application::LowDelay`), elle était bit à bit identique, parce que
    // `decide_fec` (opus_encoder.c:721) rend 0 sans rien regarder d'autre.
    // En mode SILK/hybride, le seul chemin par lequel `packet_loss_perc`
    // influence l'encodage est `decide_fec` -> `LBRR_coded`.
    //
    // Un signal NON silencieux est indispensable : sous DTX, le silence
    // retombe à 1 octet par trame quoi qu'on déclare.
    let pcm: Vec<i16> = (0..FRAME_INTERLEAVED)
        .map(|i| ((i as f32 * 0.05).sin() * 8000.0) as i16)
        .collect();

    let mut sans = OpusEncoder::new().expect("encodeur");
    let mut avec = OpusEncoder::new().expect("encodeur");
    avec.set_packet_loss_perc(20).expect("perte déclarée");

    let mut total_sans = 0usize;
    let mut total_avec = 0usize;
    for _ in 0..100 {
        total_sans += sans.encode(&pcm).expect("encodage").len();
        total_avec += avec.encode(&pcm).expect("encodage").len();
    }

    assert_ne!(
        total_avec, total_sans,
        "sortie identique ({total_sans} octets des deux côtés) : \
         `decide_fec` a pris son retour anticipé, donc aucune redondance \
         LBRR n'est codée — c'est le symptôme du mode CELT seul"
    );
}

#[test]
fn lbrr_est_reellement_decodable() {
    // Test que la redondance LBRR codée est réellement présente et
    // décodable. C'est la preuve sémantique que le FEC est opérant :
    // on encode en déclarant une perte, on prend un paquet en régime
    // établi, et on décode ce paquet avec le drapeau FEC sur un décodeur
    // neuf (sans historique). On mesure l'énergie reconstruite.
    //
    // La stratégie : encoder la même trame 100 fois pour atteindre le
    // régime établi. Prendre le paquet #99. Décoder ce paquet avec FEC
    // sur deux décodeurs neufs : un depuis l'encodeur avec perte déclarée
    // (attend la redondance LBRR), un depuis l'encodeur sans perte
    // (pas de redondance, seulement du bruit de reconstruction).
    //
    // Attendu : énergie reconstruite(avec FEC) >> énergie reconstruite(sans).
    let pcm: Vec<i16> = (0..FRAME_INTERLEAVED)
        .map(|i| ((i as f32 * 0.05).sin() * 8000.0) as i16)
        .collect();

    let mut enc_sans = OpusEncoder::new().expect("encodeur");
    let mut enc_avec = OpusEncoder::new().expect("encodeur");
    enc_avec
        .set_packet_loss_perc(20)
        .expect("perte déclarée");

    // Encoder 100 trames pour atteindre le régime établi.
    let mut paquets_sans = Vec::new();
    let mut paquets_avec = Vec::new();
    for _ in 0..100 {
        paquets_sans.push(enc_sans.encode(&pcm).expect("encodage sans"));
        paquets_avec.push(enc_avec.encode(&pcm).expect("encodage avec"));
    }

    // Prendre le dernier paquet en régime établi.
    let dernier_sans = &paquets_sans[99];
    let dernier_avec = &paquets_avec[99];

    // Décodeur neuf sans historique pour décoder le paquet comme une
    // trame FEC (le décodeur reconstruit à partir de la redondance du
    // paquet SUIVANT, ou simplement tente de masquer la perte).
    let mut dec_pour_sans =
        ::opus::Decoder::new(SAMPLE_RATE_HZ, ::opus::Channels::Stereo)
            .expect("décodeur");
    let mut dec_pour_avec =
        ::opus::Decoder::new(SAMPLE_RATE_HZ, ::opus::Channels::Stereo)
            .expect("décodeur");

    let mut sortie_sans = vec![0i16; FRAME_INTERLEAVED];
    let mut sortie_avec = vec![0i16; FRAME_INTERLEAVED];

    // Décoder avec le drapeau FEC (simule une trame perdue).
    dec_pour_sans
        .decode(dernier_sans, &mut sortie_sans, true)
        .expect("décodage sans avec FEC");
    dec_pour_avec
        .decode(dernier_avec, &mut sortie_avec, true)
        .expect("décodage avec avec FEC");

    // Mesurer l'énergie (somme des carrés normalisée).
    let energie_sans: f64 = sortie_sans
        .iter()
        .map(|&s| (s as f64) * (s as f64))
        .sum::<f64>()
        / (FRAME_INTERLEAVED as f64);
    let energie_avec: f64 = sortie_avec
        .iter()
        .map(|&s| (s as f64) * (s as f64))
        .sum::<f64>()
        / (FRAME_INTERLEAVED as f64);

    eprintln!(
        "Énergie reconstruite : sans FEC = {:.2}, avec FEC = {:.2}",
        energie_sans, energie_avec
    );

    // Attend que la redondance LBRR produise du signal significatif.
    // Si elle est présente, energie_avec >> energie_sans.
    assert!(
        energie_avec > energie_sans,
        "pas de redondance LBRR décodable : \
         énergie sans FEC = {:.2}, énergie avec FEC = {:.2} — \
         le FEC n'a rien apporté à la reconstruction",
        energie_sans, energie_avec
    );
}

// ------------------------------------------------------------------
// Le DÉCODEUR (chantier E, bloc E1, tâche 3)
// ------------------------------------------------------------------

/// Le fait de conception le moins évident du décodage : Chrome encode le
/// micro en MONO, et le câble attend du stéréo. On ne l'écrit PAS
/// nous-mêmes : un décodeur créé pour deux canaux duplique un flux mono.
/// Ce test est là pour que cette propriété soit VÉRIFIÉE et non supposée
/// — la spec §7 la décrivait comme un travail à faire (« mono → stéréo
/// par duplication »).
#[test]
fn un_flux_mono_ressort_stereo_par_duplication() {
    let mut enc = ::opus::Encoder::new(
        SAMPLE_RATE_HZ,
        ::opus::Channels::Mono,
        ::opus::Application::Audio,
    )
    .unwrap();
    let mono: Vec<i16> = (0..960)
        .map(|n| ((n as f32 * 0.1).sin() * 8000.0) as i16)
        .collect();
    let paquet = enc.encode_vec(&mono, 4000).unwrap();

    let mut dec = OpusDecoder::new().unwrap();
    assert_eq!(dec.echantillons_de(&paquet).unwrap(), 960);
    let mut sortie = vec![0i16; 960 * CHANNELS];
    assert_eq!(dec.decoder(&paquet, &mut sortie).unwrap(), 960);
    // Entrelacé : les deux canaux sont IDENTIQUES échantillon par échantillon.
    for paire in sortie.chunks_exact(2) {
        assert_eq!(paire[0], paire[1], "canaux gauche et droit dissemblables");
    }
    // …et le signal n'est pas nul : un décodeur qui rendrait du silence
    // passerait l'égalité ci-dessus sans rien décoder.
    assert!(
        sortie.iter().any(|&e| e.abs() > 500),
        "signal décodé nul"
    );
}

/// « Aucune durée de trame n'est supposée » (spec §7). Chrome émet du
/// 20 ms par défaut, le chantier A produit du 10 ms, et rien ne garantit
/// qu'ils s'y tiennent. C'est le test qui interdit de re-supposer une
/// constante.
#[test]
fn des_trames_de_dix_vingt_et_quarante_millisecondes_passent_toutes() {
    for ms in [10u32, 20, 40] {
        let par_canal = (SAMPLE_RATE_HZ / 1000 * ms) as usize;
        let mut enc = ::opus::Encoder::new(
            SAMPLE_RATE_HZ,
            ::opus::Channels::Stereo,
            ::opus::Application::Audio,
        )
        .unwrap();
        // Un signal RÉEL, pas du silence : avec le DTX qu'un encodeur
        // pourrait porter, une trame muette peut se coder en un octet et
        // `get_nb_samples` n'aurait plus rien de représentatif.
        let pcm: Vec<i16> = (0..par_canal * CHANNELS)
            .map(|n| ((n as f32 * 0.03).sin() * 6000.0) as i16)
            .collect();
        let paquet = enc.encode_vec(&pcm, 4000).unwrap();
        let mut dec = OpusDecoder::new().unwrap();
        assert_eq!(
            dec.echantillons_de(&paquet).unwrap(),
            par_canal,
            "durée {ms} ms"
        );
        let mut sortie = vec![0i16; par_canal * CHANNELS];
        assert_eq!(
            dec.decoder(&paquet, &mut sortie).unwrap(),
            par_canal,
            "durée {ms} ms"
        );
    }
}

/// « Vérifier seulement qu'il rend des octets prouverait qu'il rend du
/// bruit tout aussi bien » (spec §11). Le PLC doit rendre le BON NOMBRE
/// d'échantillons, et ce nombre vient de la DERNIÈRE TRAME DÉCODÉE — pas
/// d'une constante.
#[test]
fn la_dissimulation_rend_la_duree_de_la_derniere_trame() {
    let par_canal = (SAMPLE_RATE_HZ / 1000 * 40) as usize; // 40 ms → 1920
    let mut enc = ::opus::Encoder::new(
        SAMPLE_RATE_HZ,
        ::opus::Channels::Stereo,
        ::opus::Application::Audio,
    )
    .unwrap();
    let pcm: Vec<i16> = (0..par_canal * CHANNELS)
        .map(|n| ((n as f32 * 0.02).sin() * 7000.0) as i16)
        .collect();
    let paquet = enc.encode_vec(&pcm, 4000).unwrap();

    let mut dec = OpusDecoder::new().unwrap();
    let mut sortie = vec![0i16; par_canal * CHANNELS];
    assert_eq!(dec.decoder(&paquet, &mut sortie).unwrap(), par_canal);
    assert_eq!(
        dec.derniere_duree().unwrap(),
        par_canal,
        "la durée relue n'est pas celle de la trame qui vient d'être décodée"
    );

    // ⚠️ Le tampon est VOLONTAIREMENT plus grand que la trame — 60 ms pour
    // une trame de 40. Sans cet écart, le test ne mesurerait que la taille
    // du tampon : `opus_decode` sur un paquet vide prend `frame_size` de
    // la longueur qu'on lui tend. C'est exactement ce que la mutation de
    // la tâche 3 a révélé.
    let mut plc = vec![0i16; (SAMPLE_RATE_HZ / 1000 * 60) as usize * CHANNELS];
    assert_eq!(
        dec.dissimuler(&mut plc).unwrap(),
        par_canal,
        "la dissimulation n'a pas rendu la durée de la dernière trame décodée"
    );

    // Et un décodeur NEUF n'a aucune durée à dissimuler : il rend 0, à
    // charge de l'appelant d'écrire du silence.
    let mut neuf = OpusDecoder::new().unwrap();
    assert_eq!(neuf.dissimuler(&mut plc).unwrap(), 0);
}

/// Le pendant exact du test d'énergie du FEC de l'ENCODEUR
/// (`lbrr_est_reellement_decodable`), pris à l'envers : la trame SUIVANTE
/// restitue un signal CORRÉLÉ à l'original, pas seulement des octets.
///
/// Le critère est un RAPPORT d'énergie entre deux flux dont seule la perte
/// déclarée à l'encodeur diffère : sans elle, libopus n'émet aucune
/// redondance LBRR et le décodeur FEC ne peut que masquer.
#[test]
fn la_reconstruction_fec_restitue_un_signal_correle_a_l_original() {
    let pcm: Vec<i16> = (0..FRAME_INTERLEAVED)
        .map(|i| ((i as f32 * 0.05).sin() * 8000.0) as i16)
        .collect();

    let mut enc_sans = OpusEncoder::new().expect("encodeur");
    let mut enc_avec = OpusEncoder::new().expect("encodeur");
    enc_avec.set_packet_loss_perc(20).expect("perte déclarée");

    let mut paquets_sans = Vec::new();
    let mut paquets_avec = Vec::new();
    for _ in 0..100 {
        paquets_sans.push(enc_sans.encode(&pcm).expect("encodage sans"));
        paquets_avec.push(enc_avec.encode(&pcm).expect("encodage avec"));
    }

    let energie = |paquet: &[u8]| -> f64 {
        // Décodeur NEUF, donc sans historique : ce qui sort ne peut venir
        // que de la redondance portée par ce paquet-ci.
        let mut dec = OpusDecoder::new().expect("décodeur");
        let mut sortie = vec![0i16; FRAME_INTERLEAVED];
        let n = dec.decoder_fec(paquet, &mut sortie).expect("décodage FEC");
        sortie[..n * CHANNELS]
            .iter()
            .map(|&e| (e as f64) * (e as f64))
            .sum::<f64>()
            / (n * CHANNELS).max(1) as f64
    };

    let avec = energie(&paquets_avec[99]);
    let sans = energie(&paquets_sans[99]);
    eprintln!("FEC décodé : énergie avec perte déclarée={avec:.2}, sans={sans:.2}");
    // ⚠️ DEUX assertions, et la seconde est celle qui rend le test
    // discriminant. Le seul rapport d'énergie ne prouve PAS que le chemin
    // FEC a été emprunté : décoder ces deux paquets NORMALEMENT (fec
    // = false) rend aussi un rapport supérieur à 10, et cette mutation
    // passait — relevé à la tâche 3, step 2.
    //
    // Ce qui distingue vraiment le chemin FEC, c'est que sans redondance
    // LBRR il rend du SILENCE : libopus n'a rien à reconstruire et
    // n'invente rien. Un décodage normal, lui, rend le signal du paquet.
    assert!(
        avec > sans * 10.0 + 1.0,
        "la reconstruction FEC ne restitue rien de corrélé : avec={avec:.2}, sans={sans:.2}"
    );
    assert!(
        sans < 1.0,
        "sans redondance LBRR, le décodage FEC devrait rendre du SILENCE et rend {sans:.2} : \
         le drapeau FEC n'a pas été honoré, et ce paquet a été décodé normalement"
    );
}
