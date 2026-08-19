//! Le PUITS DE MESURE du micro (`MICRO_MESURE=1`) : un consommateur qui joue le
//! rôle qu'un vrai câble audio tiendra au bloc E2, et qui rend au journal ce
//! qu'il a entendu.
//!
//! ⚠️ **INSTRUMENT DE BANC, JAMAIS UNE CONFIGURATION LIVRÉE.** D'où la
//! convention `MICRO_MESURE=1` qui **ARME** — et non `=0` qui désarmerait,
//! comme le font `AUDIO`, `PLEIN_ECRAN`, `SUPERVISEUR` et `CAPTEUR`. Une simple
//! présence ne suffit pas non plus : il faut la valeur `1`. La règle générale du
//! dépôt reste « on désarme sur `=0` ce qui est livré, on arme sur `=1` ce qui
//! ne l'est pas ».
//!
//! **PUR : aucun `cfg`, aucun objet COM, aucun périphérique.** Ce fichier
//! compile et se teste sous Linux, contrairement à son voisin
//! `demarrage/audio.rs`. C'est ce qui permet à la partie qui peut réellement se
//! tromper — le désentrelacement, la fenêtre d'une seconde, le sens de la crête
//! — d'être éprouvée sans VM.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::micro::{frequence_par_passages_a_zero, CompteursMicro, LecteurMicro, PuitsMicro, TrameMicro};
use crate::opus::SAMPLE_RATE_HZ;
use crate::transport::Session;
use crate::Config;

/// Période de réveil du consommateur. 10 ms est la granularité usuelle d'un
/// tampon WASAPI partagé : le puits imite donc ce que le bloc E2 fera.
const PERIODE: Duration = Duration::from_millis(10);

/// Trames par canal consommées à chaque réveil. 480 à 48 kHz = 10 ms.
const TRAMES_PAR_REVEIL: usize = 480;

/// Le puits, tel que `transport/piste_micro.rs` le voit.
///
/// ⚠️ **`deposer` prend un verrou, et l'invariant du chantier est qu'il ne
/// bloque PAS.** Le verrou n'est tenu que le temps de `TamponGigue::deposer`
/// (une insertion ordonnée dans une file bornée) d'un côté, et de
/// `LecteurMicro::remplir` (le décodage d'au plus une poignée de trames Opus)
/// de l'autre : des fenêtres de l'ordre de la dizaine de microsecondes, contre
/// une boucle de transport qui tourne au rythme de la vidéo. C'est acceptable
/// **pour un banc**. Le bloc E2, lui, aura un vrai fil WASAPI à échéance dure
/// et devra trancher autrement — une file sans verrou, ou un double tampon.
struct PuitsDeMesure {
    lecteur: Arc<Mutex<LecteurMicro>>,
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

/// `MICRO_MESURE` arme-t-elle le puits ? **`1`, et rien d'autre.**
///
/// ⚠️ **Ce prédicat existe pour être TESTÉ**, et le test existe pour empêcher
/// une « simplification » future en `is_ok()`. La convention est l'inverse de
/// celle d'`AUDIO`/`SUPERVISEUR`/`PLEIN_ECRAN`/`CAPTEUR`, qui désarment sur
/// `=0` : ici on arme sur `=1`, parce qu'un instrument de banc ne doit pas
/// s'allumer par la simple présence d'une variable — quelqu'un qui écrirait
/// `MICRO_MESURE=0` pour être sûr de le couper l'allumerait.
pub(crate) fn arme(valeur: Option<&str>) -> bool {
    valeur == Some("1")
}

/// Installe le puits de mesure sur `session`, si `MICRO_MESURE=1`.
///
/// Son absence ne compromet jamais la session : sans lui, `micro_disponible()`
/// reste faux, `ready` porte `mic: false`, et le bouton du navigateur ne paraît
/// pas — exactement le comportement voulu tant que le vrai câble (bloc E2)
/// n'existe pas.
pub(super) fn brancher(config: &Config, session: &mut Session) {
    if !config.micro_mesure {
        return;
    }

    let lecteur = match LecteurMicro::new() {
        Ok(l) => Arc::new(Mutex::new(l)),
        Err(e) => {
            tracing::warn!(erreur = %e, "puits de mesure du micro indisponible, la session continue sans");
            return;
        }
    };

    session.set_puits_micro(Box::new(PuitsDeMesure { lecteur: Arc::clone(&lecteur) }));

    // ⚠️ ÉMISE AU BRANCHEMENT, PAS AU PREMIER PAQUET, et c'est délibéré. D6 a
    // écrit un contrôle « la variable est-elle arrivée ? » qui rendait vide aux
    // sept exécutions parce qu'il courait avant l'initialisation qu'il
    // observait : il aurait masqué une variable réellement manquante. Ici, la
    // ligne sort dès que le puits est posé — donc avant toute session WebRTC —
    // et son ABSENCE prouve que `MICRO_MESURE` n'a pas atteint le processus.
    tracing::info!(
        session = %config.session_id,
        "micro de mesure ARME (MICRO_MESURE=1) : instrument de banc, jamais une configuration livree"
    );

    let session_id = config.session_id.clone();
    std::thread::spawn(move || consommer(lecteur, session_id));
}

/// Le fil consommateur : il tient l'échéance, remplit, et rend compte.
fn consommer(lecteur: Arc<Mutex<LecteurMicro>>, session_id: String) {
    let mut tampon = vec![0.0f32; TRAMES_PAR_REVEIL * 2];
    let mut fenetre = Fenetre::new(SAMPLE_RATE_HZ);
    let mut precedents = CompteursMicro::default();
    let mut echeance = Instant::now();

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

        let compteurs = {
            let Ok(mut lecteur) = lecteur.lock() else {
                tracing::warn!("verrou du puits de mesure empoisonne, fil de mesure arrete");
                return;
            };
            lecteur.remplir(&mut tampon);
            lecteur.compteurs()
        };

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
            fec = d(compteurs.fec, precedents.fec),
            famines = d(compteurs.famines, precedents.famines),
            hors_ordre = d(compteurs.hors_ordre, precedents.hors_ordre),
            doublons = d(compteurs.doublons, precedents.doublons),
            jetees_saturation = d(compteurs.jetees_saturation, precedents.jetees_saturation),
            jetees_perimees = d(compteurs.jetees_perimees, precedents.jetees_perimees),
            deposees_total = compteurs.deposees,
            "micro mesuré"
        );
        precedents = compteurs;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
