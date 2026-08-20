//! Le PUITS DE MESURE lui-même : le consommateur, sa fenêtre d'observation
//! d'une seconde, et ce qu'il rend au journal.
//!
//! ⚠️ **EXTRAIT VERBATIM de `demarrage/micro.rs` (tâche 6 du plan E2), AVANT
//! l'addition qui l'aurait fait franchir le plafond — pas après.** Le fichier
//! parent valait **452** lignes et la tâche 10 doit y ajouter l'aiguillage
//! « câble ou mesure ou rien ». D9 a payé deux fois pour l'ordre inverse
//! (`sommeil.rs` ramené à 499 **par compression**, geste que `CLAUDE.md`
//! interdit nommément, puis extrait sur exigence de revue ; `capteur/fenetre.rs`,
//! 508 → 496 → 485) ; D10 a inversé l'ordre trois fois et n'a compressé aucune
//! fois. **Aucun comportement ne change**, et le contrôle de transposition est
//! le compte de tests, annoncé AVANT d'être mesuré.
//!
//! Ce qui RESTE chez le parent est l'aiguillage seul : `arme` et `brancher`.
//!
//! ⚠️ **INSTRUMENT DE BANC, JAMAIS UNE CONFIGURATION LIVRÉE** — voir l'en-tête
//! de `demarrage/micro.rs` pour la convention `MICRO_MESURE=1`.
//!
//! **PUR : aucun `cfg`, aucun objet COM, aucun périphérique.**

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::micro::{frequence_par_passages_a_zero, CompteursMicro, LecteurMicro, PuitsMicro, TrameMicro};
use crate::opus::SAMPLE_RATE_HZ;

/// Période de réveil du consommateur. 10 ms est la granularité usuelle d'un
/// tampon WASAPI partagé : le puits imite donc ce que le bloc E2 fera.
const PERIODE: Duration = Duration::from_millis(10);

/// Trames par canal consommées à chaque réveil. 480 à 48 kHz = 10 ms.
const TRAMES_PAR_REVEIL: usize = 480;

/// Le puits, tel que `transport/piste_micro.rs` le voit.
///
/// ⚠️ `pub(super)` : c'est, avec celle de `consommer`, la SEULE chose que
/// l'extraction change au code — `brancher` est resté chez le parent et doit
/// pouvoir les nommer. Aucun comportement ne bouge.
///
/// ⚠️ **`deposer` prend un verrou, et l'invariant du chantier est qu'il ne
/// bloque PAS.** Le verrou n'est tenu que le temps de `TamponGigue::deposer`
/// (une insertion ordonnée dans une file bornée) d'un côté, et de
/// `LecteurMicro::remplir` (le décodage d'au plus une poignée de trames Opus)
/// de l'autre : des fenêtres de l'ordre de la dizaine de microsecondes, contre
/// une boucle de transport qui tourne au rythme de la vidéo. C'est acceptable
/// **pour un banc**.
///
/// ⚠️ **La suite de cette phrase annonçait que « le bloc E2, lui, aura un vrai
/// fil WASAPI à échéance dure et devra trancher autrement — une file sans
/// verrou, ou un double tampon ». C'était une PRÉDICTION, et le bloc E2 a
/// tranché dans l'autre sens** : `windows_micro.rs` garde le `Mutex`, parce que
/// le fil de rendu ne le tient que le temps de `remplir` — de l'ordre de la
/// dizaine de microsecondes — contre une échéance WASAPI de l'ordre de 10 ms,
/// trois ordres de grandeur au-dessus. Une file sans verrou serait du travail
/// écrit avant d'avoir constaté le besoin. **Et le besoin est rendu
/// OBSERVABLE** : le fil de rendu compte ses retards d'échéance (`retards` de
/// sa trace périodique), ce qui rend la question décidable au lieu de
/// conjecturale.
pub(super) struct PuitsDeMesure {
    pub(super) lecteur: Arc<Mutex<LecteurMicro>>,
}

impl PuitsMicro for PuitsDeMesure {
    fn deposer(&mut self, trame: TrameMicro) -> bool {
        // `false` signifierait « exclusivité non acquise » (bloc E2). Ce puits
        // n'en revendique aucune : il accepte toujours.
        match self.lecteur.lock() {
            Ok(mut lecteur) => {
                lecteur.deposer(trame);
                true
            }
            // Le verrou est empoisonné : le fil consommateur a paniqué. On
            // refuse plutôt que de propager la panique dans la boucle de
            // transport — un défaut du micro ne tue jamais une session vidéo.
            Err(_) => false,
        }
    }
}

/// Ce qu'une fenêtre d'observation d'une seconde rend au journal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Releve {
    /// Échantillons PAR CANAL réellement consommés dans la fenêtre.
    pub echantillons: usize,
    /// Crête absolue, tous canaux confondus.
    pub crete: f32,
    /// Fréquence dominante, ou `None` si le signal est trop faible pour qu'un
    /// passage par zéro ait un sens.
    pub frequence_hz: Option<f32>,
}

/// L'accumulateur d'une seconde de PCM consommé.
///
/// ⚠️ **Le tampon rendu par `LecteurMicro::remplir` est STÉRÉO ENTRELACÉ**, et
/// `frequence_par_passages_a_zero` attend un signal MONO. C'est ici, et nulle
/// part ailleurs, que le désentrelacement doit avoir lieu.
///
/// ⚠️ **L'oublier DIVISE la fréquence par deux — il ne la double pas**, contre
/// ce que la doc de `micro::frequence_par_passages_a_zero` a longtemps affirmé.
/// **Mesuré** en retirant le `step_by(2)` ci-dessous et en relançant
/// `la_frequence_est_celle_du_signal_et_non_son_double` : **219,5 Hz rendus pour
/// une tonalité de 440 Hz**, canaux identiques. Le mécanisme : la fonction
/// dérive la durée de `pcm.len() / hz`, or un tampon entrelacé porte deux fois
/// plus de valeurs que de trames — la durée calculée double, tandis que le
/// nombre de passages par zéro, lui, ne bouge pas (dupliquer chaque échantillon
/// n'ajoute aucun changement de signe).
pub(super) struct Fenetre {
    /// Échantillons par seconde et par canal : la taille de la fenêtre.
    hz: u32,
    /// Le canal GAUCHE seul, accumulé.
    mono: Vec<f32>,
    /// Crête absolue, tous canaux confondus — un déséquilibre entre canaux ne
    /// doit pas passer inaperçu sous prétexte qu'on n'analyse qu'un canal.
    crete: f32,
}

impl Fenetre {
    pub(super) fn new(hz: u32) -> Self {
        Self { hz, mono: Vec::with_capacity(hz as usize), crete: 0.0 }
    }

    /// Absorbe un tampon stéréo entrelacé. Rend un relevé — et repart à zéro —
    /// dès que la fenêtre est pleine.
    pub(super) fn absorber(&mut self, stereo: &[f32]) -> Option<Releve> {
        for &e in stereo {
            self.crete = self.crete.max(e.abs());
        }
        // Le canal GAUCHE seul : un échantillon sur deux. Voir la doc du type
        // pour ce que coûte l'oubli de ce `step_by`.
        for &g in stereo.iter().step_by(2) {
            self.mono.push(g);
        }

        if self.mono.len() < self.hz as usize {
            return None;
        }
        let releve = Releve {
            echantillons: self.mono.len(),
            crete: self.crete,
            frequence_hz: frequence_par_passages_a_zero(&self.mono, self.hz),
        };
        self.mono.clear();
        self.crete = 0.0;
        Some(releve)
    }
}

/// Le fil consommateur : il tient l'échéance, remplit, et rend compte.
pub(super) fn consommer(lecteur: Arc<Mutex<LecteurMicro>>, session_id: String) {
    let mut tampon = vec![0.0f32; TRAMES_PAR_REVEIL * 2];
    let mut fenetre = Fenetre::new(SAMPLE_RATE_HZ);
    let mut precedents = CompteursMicro::default();
    let mut echeance = Instant::now();
    // Le maximum d'occupation sur la fenêtre d'observation. ⚠️ Un maximum, pas
    // une moyenne : le critère ④ de la recette E1 est une BORNE, et une moyenne
    // noierait la seule pointe qui la franchirait.
    let mut occupation_max = Duration::ZERO;

    loop {
        // ⚠️ ÉCHÉANCE, PAS `sleep(PERIODE)`. Un `sleep` de période fixe dérive
        // du temps de travail à chaque tour : le puits consommerait plus lentement
        // que 48 kHz, le tampon de gigue saturerait, et l'on mesurerait la dérive
        // de l'instrument en croyant mesurer celle du réseau.
        echeance += PERIODE;
        let maintenant = Instant::now();
        if echeance > maintenant {
            std::thread::sleep(echeance - maintenant);
        } else {
            // Retard franc (machine chargée) : on repart de maintenant plutôt
            // que de rattraper en rafale, ce qui viderait le tampon d'un coup
            // et compterait des famines qui n'en sont pas.
            echeance = maintenant;
        }

        let (compteurs, occupation) = {
            let Ok(mut lecteur) = lecteur.lock() else {
                tracing::warn!("verrou du puits de mesure empoisonne, fil de mesure arrete");
                return;
            };
            // ⚠️ RELEVÉE AVANT `remplir`, jamais après : c'est ce que le tampon
            // faisait attendre au moment où le consommateur s'est présenté.
            // Après, la file vient d'être vidée et l'on relèverait toujours à
            // peu près zéro — un contrôle incapable de franchir sa borne.
            let occupation = lecteur.occupation();
            lecteur.remplir(&mut tampon);
            (lecteur.compteurs(), occupation)
        };
        occupation_max = occupation_max.max(occupation);

        let Some(releve) = fenetre.absorber(&tampon) else {
            continue;
        };

        // ⚠️ `crete` et `frequence_hz` sont CÔTE À CÔTE, et c'est le fond de
        // cette trace : une fréquence rendue sur un signal quasi nul ne veut
        // rien dire. `frequence_par_passages_a_zero` rend alors `None`, qui
        // s'écrit « aucune » — un lecteur du journal ne peut pas lire l'un sans
        // l'autre, ni prendre un nombre pour une preuve de son.
        let frequence = match releve.frequence_hz {
            Some(f) => format!("{f:.1}"),
            None => "aucune".to_string(),
        };
        // Les compteurs sont des DELTAS de la seconde écoulée, pas des cumuls.
        // `CompteursMicro` est cumulatif ; un cumul ferait traîner un unique
        // incident pour le restant de la session, et l'exemple du plan
        // (`deposees=50`, soit une seconde de trames de 20 ms) est un delta.
        // `deposees_total` est joint pour que le cumul reste lisible.
        let d = |maintenant: u64, avant: u64| maintenant.saturating_sub(avant);
        tracing::info!(
            // ⚠️ OBLIGATOIRE : `agent.log` mêle le superviseur et tous ses
            // enfants depuis D4. Une trace sans `session` est un nombre dans un
            // multiensemble anonyme, et D6 a dû ré-imputer deux traces en pleine
            // recette faute de ce champ.
            session = %session_id,
            echantillons = releve.echantillons,
            crete = format!("{:.3}", releve.crete),
            frequence_hz = %frequence,
            deposees = d(compteurs.deposees, precedents.deposees),
            sauts = d(compteurs.sauts, precedents.sauts),
            insertions = d(compteurs.insertions, precedents.insertions),
            plc = d(compteurs.plc, precedents.plc),
            // ⚠️ **`plc` et `plc_plafonnees` sont CÔTE À CÔTE, et c'est le fond
            // de cette paire** : les deux naissent d'une trame manquante, et
            // sans le second on ne peut pas distinguer « la dissimulation
            // travaille » de « le plafond a mordu et le puits se tait ». C'est
            // ce qui rend le correctif du plafond FALSIFIABLE — la recette E1
            // relevait `plc = 50/s` pendant soixante secondes de silence, et
            // ce qu'on doit y lire désormais est `plc = 0` avec
            // `plc_plafonnees = 50/s`, `crete = 0.000` et `frequence_hz =
            // aucune`. Voir `micro/dissimulation.rs`.
            plc_plafonnees = d(compteurs.plc_plafonnees, precedents.plc_plafonnees),
            fec = d(compteurs.fec, precedents.fec),
            famines = d(compteurs.famines, precedents.famines),
            hors_ordre = d(compteurs.hors_ordre, precedents.hors_ordre),
            doublons = d(compteurs.doublons, precedents.doublons),
            jetees_saturation = d(compteurs.jetees_saturation, precedents.jetees_saturation),
            jetees_perimees = d(compteurs.jetees_perimees, precedents.jetees_perimees),
            deposees_total = compteurs.deposees,
            // Le critère ④ de la recette E1 (spec §13) : la latence que le
            // tampon de gigue ajoute à lui seul, bornée par `micro::PLAFOND`.
            // ⚠️ Ce n'est PAS la latence de bout en bout — voir la doc de
            // `LecteurMicro::occupation`.
            occupation_ms = occupation.as_millis() as u64,
            occupation_max_ms = occupation_max.as_millis() as u64,
            "micro mesuré"
        );
        precedents = compteurs;
        occupation_max = Duration::ZERO;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // `arme` est resté chez le parent (l'aiguillage) ; ses tests, eux,
    // partent avec le module. Seule ligne ajoutée par l'extraction.
    use crate::demarrage::micro::arme;

    /// Une tonalité CONTINUE, stéréo entrelacée, les deux canaux identiques.
    ///
    /// ⚠️ **La continuité de la phase est essentielle, et son absence a d'abord
    /// fait échouer le test pour une mauvaise raison.** Une première rédaction
    /// fabriquait UN bloc de 10 ms et le répétait cent fois : la phase repartait
    /// de zéro à chaque bloc, 440 Hz ne tenant pas un nombre entier de cycles en
    /// 480 échantillons, et la discontinuité de raccord faisait relever 400 Hz.
    /// L'implémentation était juste ; c'était l'instrument qui mentait.
    fn tonalite_continue(hz_signal: f32, trames: usize) -> Vec<f32> {
        let mut v = Vec::with_capacity(trames * 2);
        for n in 0..trames {
            let e = (2.0 * std::f32::consts::PI * hz_signal * n as f32 / SAMPLE_RATE_HZ as f32)
                .sin()
                * 0.5;
            v.push(e);
            v.push(e);
        }
        v
    }

    /// Découpe un tampon entrelacé en réveils de `TRAMES_PAR_REVEIL` trames.
    fn reveils(entrelace: &[f32]) -> impl Iterator<Item = &[f32]> {
        entrelace.chunks(TRAMES_PAR_REVEIL * 2)
    }

    #[test]
    fn seule_la_valeur_1_arme_le_puits() {
        assert!(arme(Some("1")));

        // ⚠️ Les quatre cas qui suivent sont le fond du test. Une lecture par
        // `is_ok()` — la forme qu'ont les variables voisines — rendrait VRAI sur
        // `Some("0")`, `Some("")` et `Some("true")` : un opérateur qui écrit
        // `MICRO_MESURE=0` pour être certain de couper l'instrument
        // l'allumerait.
        assert!(!arme(None), "absente : désarmé");
        assert!(!arme(Some("0")), "« 0 » : désarmé, comme l'absence");
        assert!(!arme(Some("")), "vide : désarmé");
        assert!(!arme(Some("true")), "« true » n'est pas « 1 »");
    }

    #[test]
    fn une_fenetre_ne_se_cloture_qu_a_la_seconde_pleine() {
        let mut fenetre = Fenetre::new(SAMPLE_RATE_HZ);
        let seconde = tonalite_continue(440.0, SAMPLE_RATE_HZ as usize);
        let mut blocs = reveils(&seconde);

        // 99 réveils de 10 ms : 990 ms, la fenêtre n'est pas pleine.
        for _ in 0..99 {
            assert_eq!(fenetre.absorber(blocs.next().unwrap()), None);
        }
        let releve = fenetre
            .absorber(blocs.next().unwrap())
            .expect("la centième clôture la seconde");
        assert_eq!(releve.echantillons, SAMPLE_RATE_HZ as usize);
    }

    #[test]
    fn la_frequence_est_celle_du_signal_et_non_son_double() {
        // ⚠️ LE TEST QUI COMPTE. Le tampon est STÉRÉO ENTRELACÉ : analysé tel
        // quel, il compte les passages par zéro des DEUX canaux et rend 880 Hz
        // pour une tonalité de 440. C'est ce qu'a fait la première rédaction de
        // `Fenetre::absorber`, et c'est ce que ce test a attrapé.
        let mut fenetre = Fenetre::new(SAMPLE_RATE_HZ);
        let seconde = tonalite_continue(440.0, SAMPLE_RATE_HZ as usize);
        let mut dernier = None;
        for bloc in reveils(&seconde) {
            if let Some(r) = fenetre.absorber(bloc) {
                dernier = Some(r);
            }
        }
        let f = dernier
            .expect("une seconde absorbée")
            .frequence_hz
            .expect("un signal fort a une fréquence");
        assert!(
            (f - 440.0).abs() < 5.0,
            "fréquence relevée {f:.1} Hz, attendue ≈ 440 Hz (220 signalerait un tampon non désentrelacé)"
        );
    }

    #[test]
    fn le_silence_ne_rend_aucune_frequence_mais_rend_sa_crete() {
        let mut fenetre = Fenetre::new(SAMPLE_RATE_HZ);
        let silence = vec![0.0f32; TRAMES_PAR_REVEIL * 2];
        let mut dernier = None;
        for _ in 0..100 {
            if let Some(r) = fenetre.absorber(&silence) {
                dernier = Some(r);
            }
        }
        let releve = dernier.expect("une seconde absorbée");
        // Un signal trop faible n'a PAS de fréquence : rendre un nombre pour le
        // silence ferait de cet instrument le compteur d'octets qu'il existe
        // pour remplacer (doctrine payée en D7).
        assert_eq!(releve.frequence_hz, None);
        assert_eq!(releve.crete, 0.0);
    }

    #[test]
    fn la_crete_voit_les_deux_canaux_pas_seulement_celui_qu_on_analyse() {
        // Canal gauche muet, canal droit à 0,8 : la crête doit le voir, alors
        // même que la fréquence s'analyse sur le gauche. Un déséquilibre entre
        // canaux ne doit pas passer inaperçu.
        let mut fenetre = Fenetre::new(SAMPLE_RATE_HZ);
        let mut bloc = Vec::with_capacity(TRAMES_PAR_REVEIL * 2);
        for _ in 0..TRAMES_PAR_REVEIL {
            bloc.push(0.0);
            bloc.push(0.8);
        }
        let mut dernier = None;
        for _ in 0..100 {
            if let Some(r) = fenetre.absorber(&bloc) {
                dernier = Some(r);
            }
        }
        assert_eq!(dernier.expect("une seconde absorbée").crete, 0.8);
    }

    #[test]
    fn une_fenetre_repart_a_zero_apres_sa_cloture() {
        let mut fenetre = Fenetre::new(SAMPLE_RATE_HZ);
        let tonalite = tonalite_continue(440.0, SAMPLE_RATE_HZ as usize);
        let silence = vec![0.0f32; TRAMES_PAR_REVEIL * 2];

        for bloc in reveils(&tonalite) {
            fenetre.absorber(bloc);
        }
        // Seconde fenêtre, silencieuse : sans remise à zéro, la crête et la
        // fréquence de la première fuiraient dans la seconde.
        let mut dernier = None;
        for _ in 0..100 {
            if let Some(r) = fenetre.absorber(&silence) {
                dernier = Some(r);
            }
        }
        let releve = dernier.expect("une seconde absorbée");
        assert_eq!(releve.crete, 0.0);
        assert_eq!(releve.frequence_hz, None);
    }

    /// Le critère ④ de la recette E1 se lit sur `occupation_ms`, et rien
    /// d'autre ne le porte. Ce test garde l'instrument lui-même : une
    /// délégation oubliée — `Duration::ZERO` rendu sans regarder la file —
    /// ferait relever une latence de tampon nulle sur un tampon plein, et le
    /// critère passerait sans avoir rien mesuré.
    #[test]
    fn l_occupation_du_lecteur_est_celle_des_trames_en_attente() {
        let mut lecteur = LecteurMicro::new().expect("décodeur Opus");
        assert_eq!(lecteur.occupation(), Duration::ZERO, "à vide");

        // Trois trames de 20 ms : 960 échantillons par canal chacune.
        for i in 0..3u64 {
            lecteur.deposer(TrameMicro {
                opus: vec![0u8; 8],
                rtp_48k: i * 960,
                echantillons: 960,
            });
        }
        assert_eq!(
            lecteur.occupation(),
            Duration::from_millis(60),
            "trois trames de 20 ms font 60 ms d'attente"
        );
    }
}
