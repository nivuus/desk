//! Le sens MONTANT du micro, côté agent : ce qui arrive du navigateur en
//! Opus, remis dans l'ordre, débarrassé de ses doublons, borné en latence,
//! décodé, et rendu prêt à jouer.
//!
//! **Ce module est PUR : il ne référence jamais le crate `windows` et n'ouvre
//! aucun périphérique.** C'est la ligne de partage de la spec §6, et elle est
//! ce qui rend éprouvable sous Linux tout ce qui peut mal tourner — l'ordre,
//! la gigue, la dérive, le décodage, le silence. Le bloc E2 n'aura qu'à
//! réveiller un fil WASAPI et appeler `LecteurMicro::remplir`.
//!
//! ⚠️ **Il DÉCODE, en revanche, et l'en-tête a dit le contraire jusqu'à la
//! clôture du chantier E** : `LecteurMicro` possède l'`OpusDecoder` (l. 278).
//! Ce n'est pas une entorse à la pureté — libopus ne connaît ni Windows ni
//! périphérique —, et c'est même ce qui rend le décodage, le PLC et le FEC
//! éprouvables sous Linux. La phrase fausse datait de la tâche 4, qui n'avait
//! que le tampon ; la tâche 6 a ajouté le décodeur sans la relire.
//!
//! **L'invariant du module** : `deposer` ne rend rien et ne peut donc jamais
//! faire attendre la boucle de transport. C'est le miroir exact de la règle du
//! chantier A — la boucle dépose, un fil dédié travaille.

use std::collections::VecDeque;
use std::time::Duration;

use crate::opus::{OpusDecoder, CHANNELS, SAMPLE_RATE_HZ};

/// Occupation visée du tampon : le compromis latence / résistance à la gigue.
///
/// ⚠️ **NON CALIBRÉE.** Elle rejoint `BPP_MIN`, `FACTEUR_FOCUS`,
/// `PART_DORMANTE_BPS` et les autres : aucun jugement d'écoute n'a jamais été
/// porté sur une constante de ce dépôt.
pub const CIBLE: Duration = Duration::from_millis(40);

/// Occupation au-delà de laquelle on jette plutôt que d'accumuler. Sans elle,
/// un navigateur qui émet plus vite que le câble ne consomme ferait croître la
/// latence sans borne — le tampon deviendrait un retard permanent.
pub const PLAFOND: Duration = Duration::from_millis(200);

/// Au-dessus, on saute une trame pour rattraper la dérive (tâche 5).
pub const SEUIL_SAUT: Duration = Duration::from_millis(120);

/// En dessous, on en insère une (tâche 5).
pub const SEUIL_INSERTION: Duration = Duration::from_millis(20);

/// Une trame Opus telle qu'elle arrive du navigateur.
///
/// `echantillons` est le nombre d'échantillons PAR CANAL que porte le paquet,
/// **lu de son en-tête** par `OpusDecoder::echantillons_de` et jamais supposé
/// (spec §7) : Chrome émet du 20 ms, le chantier A du 10 ms, et rien n'oblige
/// un pair à s'y tenir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrameMicro {
    pub opus: Vec<u8>,
    pub rtp_48k: u64,
    pub echantillons: usize,
}

/// Tout ce que le tampon a rencontré. **Aucun de ces événements n'est
/// silencieux** (spec §8) : chacun a son compteur, et c'est ce qui rend une
/// recette lisible sans instrumenter le code à chaud.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompteursMicro {
    pub deposees: u64,
    pub hors_ordre: u64,
    pub doublons: u64,
    pub jetees_saturation: u64,
    pub jetees_perimees: u64,
    pub sauts: u64,
    pub insertions: u64,
    pub famines: u64,
    pub plc: u64,
    /// Dissimulations REFUSÉES parce que `PLAFOND_DISSIMULATION` était
    /// atteint : autant de trames rendues en SILENCE au lieu d'être
    /// extrapolées.
    ///
    /// ⚠️ **Il est disjoint de `plc`, et c'est tout son intérêt** : sans lui,
    /// une recette ne pourrait pas distinguer « la dissimulation travaille »
    /// de « le plafond a mordu et le puits se tait », et la correction ne
    /// serait pas falsifiable. `plc` compte ce qui a été extrapolé,
    /// `plc_plafonnees` ce qui ne l'a délibérément pas été.
    pub plc_plafonnees: u64,
    pub fec: u64,
}

/// Ce qu'un tour de lecture réclame au décodeur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Retrait {
    /// La trame due est là : décodage normal.
    Trame(TrameMicro),
    /// La trame due manque mais la SUIVANTE est là : décoder avec le FEC.
    ///
    /// Le sens est contre-intuitif et vaut d'être redit : la redondance LBRR
    /// d'un paquet reconstruit la trame qui le PRÉCÈDE. D'où la règle — le FEC
    /// ne sert que si la suivante est déjà arrivée, et jamais autrement.
    Reconstruire { suivante: Vec<u8> },
    /// Rien à jouer. **TROIS issues, pas deux** : dissimulation si une trame a
    /// déjà été décodée ET que le budget de `micro/dissimulation.rs` n'est pas
    /// épuisé ; silence dans les deux autres cas — rien n'a jamais été décodé,
    /// ou `PLAFOND_DISSIMULATION` est atteint (compteur `plc_plafonnees`).
    /// ⚠️ La troisième est NEUVE : sans elle, une famine prolongée dissimulait
    /// sans fin et fabriquait un bourdon, mesuré sur 60 s par la recette E1.
    Manquante,
}

/// Convertit un nombre d'échantillons par canal en durée à 48 kHz.
fn duree_de(echantillons: usize) -> Duration {
    Duration::from_nanos(echantillons as u64 * 1_000_000_000 / SAMPLE_RATE_HZ as u64)
}

/// Le tampon de gigue : il ordonne, dédoublonne, borne, et rend ce qui est dû.
pub struct TamponGigue {
    file: VecDeque<TrameMicro>,
    /// Horodatage RTP de la trame attendue au prochain retrait. `None` tant
    /// qu'aucune trame n'a été jouée : la première qui se présente fait
    /// référence, plutôt qu'un zéro arbitraire qu'un navigateur n'a aucune
    /// raison d'employer (Chrome tire son horodatage RTP initial au hasard).
    prochain_du: Option<u64>,
    /// Occupation VISÉE. Retenue pour la lecture d'un futur asservissement
    /// fin ; la correction de dérive d'aujourd'hui n'emploie que les deux
    /// seuils, qui bornent la bande morte autour d'elle.
    #[allow(dead_code)]
    cible: Duration,
    plafond: Duration,
    compteurs: CompteursMicro,
}

impl TamponGigue {
    pub fn new(cible: Duration, plafond: Duration) -> Self {
        Self {
            file: VecDeque::new(),
            prochain_du: None,
            cible,
            plafond,
            compteurs: CompteursMicro::default(),
        }
    }

    /// **NON BLOQUANT, et c'est l'invariant de ce module.** Aucune valeur de
    /// retour, aucun `Result` : la boucle de transport ne peut structurellement
    /// pas attendre ici, ni avoir à décider quoi que ce soit.
    pub fn deposer(&mut self, trame: TrameMicro) {
        self.compteurs.deposees += 1;

        // Périmée : sa place est déjà passée. La jouer hors de son tour
        // désordonnerait la ligne de temps qu'on est justement là pour tenir.
        if let Some(du) = self.prochain_du {
            if trame.rtp_48k < du {
                self.compteurs.jetees_perimees += 1;
                return;
            }
        }

        // Doublon : le même horodatage RTP est déjà en file. Une
        // retransmission ou un rejeu ne doit pas être joué deux fois.
        if self.file.iter().any(|t| t.rtp_48k == trame.rtp_48k) {
            self.compteurs.doublons += 1;
            return;
        }

        // Insertion ORDONNÉE. Un simple `push_back` suffirait au cas nominal —
        // et c'est exactement ce que la décision 2 du plan nous interdit de
        // supposer : en ramenant `reordering_size_audio` de 15 à 2, on retire
        // à str0m la garantie d'ordre qu'il offrait, et c'est ici qu'elle se
        // rattrape.
        let place = self
            .file
            .iter()
            .position(|t| t.rtp_48k > trame.rtp_48k)
            .unwrap_or(self.file.len());
        if place != self.file.len() {
            self.compteurs.hors_ordre += 1;
        }
        self.file.insert(place, trame);

        // Saturation : c'est la plus ANCIENNE qui part, pas la plus récente.
        // L'inverse de l'émission du chantier A, et pour une raison exacte —
        // là-bas on choisit quoi envoyer, ici on subit une ligne de temps
        // distante qu'il faut suivre en avançant, jamais en reculant.
        while self.occupation() > self.plafond {
            let Some(partie) = self.file.pop_front() else {
                break;
            };
            self.compteurs.jetees_saturation += 1;
            // On a sauté par-dessus : le prochain dû devient ce qui reste, sans
            // quoi le tour suivant réclamerait par FEC la trame qu'on vient
            // délibérément de jeter.
            if self.prochain_du.is_none_or(|du| du <= partie.rtp_48k) {
                self.prochain_du = self.file.front().map(|t| t.rtp_48k);
            }
        }
    }

    /// Ce qui est dû maintenant. Ne bloque jamais et rend toujours quelque
    /// chose : à défaut de trame, une consigne de dissimulation.
    pub fn retirer(&mut self) -> Retrait {
        // `_tete` : le `let … else` n'existe que pour son bras `else` — la
        // valeur est relue plus bas, après la correction de dérive. Le tiret
        // bas ferme le legs n°4 de E1, seul avertissement du crate qui ne fût
        // pas un `dead_code`.
        let Some(_tete) = self.file.front().map(|t| t.rtp_48k) else {
            self.compteurs.famines += 1;
            return Retrait::Manquante;
        };

        // --- correction de dérive (spec §8) ---------------------------------
        //
        // Deux horloges libres se croisent : celle du navigateur qui encode et
        // celle du câble qui consomme. Rien ne les asservit l'une à l'autre, et
        // l'écart, si petit soit-il, s'accumule sans terme.
        //
        // Le remède est GROSSIER et assumé : on saute une trame quand on a trop
        // de retard, on en insère une quand on a trop d'avance. « Audible une
        // fois par plusieurs minutes » — un rééchantillonnage adaptatif serait
        // du travail écrit avant d'avoir constaté le besoin.
        //
        // ⚠️ **Les deux seuils forment une HYSTÉRÉSIS, et son absence ferait
        // osciller le tampon à chaque trame** : sans bande morte entre eux, la
        // correction qui rattrape un retard créerait aussitôt l'avance que
        // l'autre correction viendrait défaire.
        let occupation = self.occupation();
        if occupation > SEUIL_SAUT {
            let saute = self.file.pop_front().expect("tête relue");
            self.compteurs.sauts += 1;
            // Même raison qu'à la saturation : sans avancer le dû, le tour
            // suivant réclamerait par FEC la trame qu'on vient délibérément de
            // sauter, et le saut serait un no-op déguisé.
            if self.prochain_du.is_none_or(|du| du <= saute.rtp_48k) {
                self.prochain_du = self.file.front().map(|t| t.rtp_48k);
            }
            let Some(_) = self.file.front() else {
                self.compteurs.famines += 1;
                return Retrait::Manquante;
            };
        } else if occupation < SEUIL_INSERTION {
            // ⚠️ **Une insertion ne CONSOMME PAS de trame.** Elle rend
            // `Manquante` sans dépiler, ce qui laisse l'occupation croître
            // jusqu'à la bande morte. Un `pop` accompagné d'une insertion
            // serait un no-op déguisé — l'occupation ne bougerait pas d'un
            // échantillon, et le test de l'hystérésis ne le verrait même pas.
            self.compteurs.insertions += 1;
            return Retrait::Manquante;
        }

        let tete = self.file.front().expect("tête relue").rtp_48k;

        let du = *self.prochain_du.get_or_insert(tete);

        if tete > du {
            // La trame due manque, mais la suivante est là : c'est exactement
            // la condition — et la seule — où le FEC in-band peut travailler.
            self.compteurs.fec += 1;
            let suivante = self.file.front().expect("tête relue").opus.clone();
            // On avance jusqu'à la suivante SANS la consommer : elle se joue à
            // son tour. Le trou est comblé par sa redondance, pas par elle.
            self.prochain_du = Some(tete);
            return Retrait::Reconstruire { suivante };
        }

        let trame = self.file.pop_front().expect("tête relue");
        self.prochain_du = Some(trame.rtp_48k + trame.echantillons as u64);
        Retrait::Trame(trame)
    }

    /// Durée d'audio actuellement en attente.
    pub fn occupation(&self) -> Duration {
        duree_de(self.file.iter().map(|t| t.echantillons).sum())
    }

    pub fn compteurs(&self) -> CompteursMicro {
        self.compteurs
    }
}

/// Puits d'un flux montant.
///
/// `deposer` rend `false` quand le puits REFUSE — exclusivité non acquise
/// (bloc E2, un mutex nommé à l'échelle de la machine). L'appelant journalise
/// une fois et n'insiste pas. **La couche pure ne connaît aucun mutex** : ce
/// trait est la couture, et rien de plus.
pub trait PuitsMicro {
    fn deposer(&mut self, trame: TrameMicro) -> bool;
}

/// Le tampon de gigue, le décodeur, et le résidu : tout ce qu'un fil WASAPI
/// aura besoin d'appeler, et rien de plus.
///
/// **Pur, alors qu'il sert un fil WASAPI** — c'est la ligne de partage de la
/// spec §6, et elle paie ici : le bloc E2 n'aura qu'à appeler
/// `remplir(&mut [f32])` depuis le fil que WASAPI réveille, et **tout ce qui
/// peut mal tourner — l'ordre, la gigue, la dérive, le décodage, le résidu, le
/// silence — est éprouvé sous Linux.**
pub struct LecteurMicro {
    tampon: TamponGigue,
    decodeur: OpusDecoder,
    /// PCM décodé pas encore remis à l'appelant, stéréo entrelacé.
    ///
    /// **Sans lui, la queue de chaque trame serait jetée.** Le paquet que
    /// réclame WASAPI ne fait presque jamais la taille d'une trame Opus : une
    /// trame de 20 ms rend 960 échantillons par canal, et le tampon réclamé
    /// peut en vouloir 441, 480 ou 1024. Le résidu est la pièce qui recolle
    /// deux découpages sans rapport.
    residu: VecDeque<f32>,
    /// Ce qui reste à dissimuler avant qu'on ne se taise. Voir
    /// `micro/dissimulation.rs` pour le défaut mesuré qui l'a rendu
    /// nécessaire, et pour ce que libopus fait — et ne fait pas — de son côté.
    budget: BudgetDissimulation,
}

impl LecteurMicro {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            tampon: TamponGigue::new(CIBLE, PLAFOND),
            decodeur: OpusDecoder::new()?,
            residu: VecDeque::new(),
            budget: BudgetDissimulation::new(PLAFOND_DISSIMULATION),
        })
    }

    pub fn deposer(&mut self, trame: TrameMicro) {
        self.tampon.deposer(trame);
    }

    pub fn compteurs(&self) -> CompteursMicro {
        self.tampon.compteurs()
    }

    /// Durée d'audio déposée et pas encore rendue — c'est-à-dire la latence que
    /// le tampon de gigue AJOUTE, à elle seule.
    ///
    /// ⚠️ **Ce n'est PAS la latence de bout en bout**, que ce chantier ne mesure
    /// pas plus que les précédents : le trajet réseau, l'encodage du navigateur
    /// et — au bloc E2 — l'écriture sur le câble en sont absents. C'est la borne
    /// « dépôt → retrait » de la recette E1, et rien de plus. Bornée par
    /// construction à `PLAFOND` (voir `TamponGigue::deposer`).
    pub fn occupation(&self) -> Duration {
        self.tampon.occupation()
    }

    /// Remplit `sortie` (stéréo entrelacé, `f32`) avec ce qui est dû, complète
    /// au silence, et **ne bloque JAMAIS**.
    ///
    /// Spec §8 « Silence » : le câble doit être alimenté EN CONTINU. Une
    /// application qui écoute un tampon vide ne perçoit pas du silence — elle
    /// voit un flux qui s'interrompt, ce qui n'est pas la même chose et
    /// s'entend.
    pub fn remplir(&mut self, sortie: &mut [f32]) {
        let mut ecrit = 0;
        while ecrit < sortie.len() {
            // Le résidu d'abord : c'est lui qui recolle les découpages.
            while ecrit < sortie.len() {
                let Some(e) = self.residu.pop_front() else {
                    break;
                };
                sortie[ecrit] = e;
                ecrit += 1;
            }
            if ecrit == sortie.len() {
                return;
            }

            // Rien en réserve : réclamer au tampon de quoi continuer.
            if !self.produire_une_trame() {
                // Plus rien à produire, et pas même une dissimulation. On
                // complète au silence et on rend la main — **la boucle DOIT
                // s'arrêter ici** : sans cette sortie, un lecteur qui n'a
                // jamais rien décodé tournerait sans fin, `dissimuler` rendant
                // zéro échantillon à chaque tour.
                sortie[ecrit..].fill(0.0);
                return;
            }
        }
    }

    /// Décode ce qui est dû dans le résidu. Rend `false` quand rien n'a pu
    /// être produit — à l'appelant de compléter au silence.
    fn produire_une_trame(&mut self) -> bool {
        let (paquet, echantillons, fec) = match self.tampon.retirer() {
            Retrait::Trame(t) => (t.opus, t.echantillons, false),
            Retrait::Reconstruire { suivante } => {
                // La durée reconstruite est celle de la trame MANQUANTE, qu'on
                // ne connaît pas. Celle de la suivante en est le meilleur
                // témoin disponible, et elle est LUE du paquet, jamais supposée.
                let n = self.decodeur.echantillons_de(&suivante).unwrap_or(0);
                if n == 0 {
                    return false;
                }
                (suivante, n, true)
            }
            Retrait::Manquante => {
                // Dissimulation : la durée vient de la dernière trame décodée,
                // et vaut zéro tant que rien n'a été décodé — auquel cas il n'y
                // a rien à dissimuler, et le silence est la bonne réponse.
                let n = self.decodeur.derniere_duree().unwrap_or(0);
                if n == 0 {
                    return false;
                }
                // ⚠️ **LE PLAFOND.** Au-delà de `PLAFOND_DISSIMULATION`
                // dissimulée d'affilée, on rend du SILENCE : libopus ne
                // s'arrête jamais de lui-même et converge vers du bruit de
                // confort qu'il maintient sans terme — mesuré comme un bourdon
                // continu sur 60 s de silence du navigateur. `false` fait
                // compléter au silence par `remplir`, qui sait déjà le faire.
                if !self.budget.consommer(duree_de(n)) {
                    self.tampon.compteurs.plc_plafonnees += 1;
                    return false;
                }
                let mut pcm = vec![0i16; n * CHANNELS];
                let Ok(rendus) = self.decodeur.dissimuler(&mut pcm) else {
                    return false;
                };
                self.pousser(&pcm[..rendus * CHANNELS]);
                self.tampon.compteurs.plc += 1;
                return rendus > 0;
            }
        };

        let mut pcm = vec![0i16; echantillons * CHANNELS];
        let rendus = if fec {
            self.decodeur.decoder_fec(&paquet, &mut pcm)
        } else {
            self.decodeur.decoder(&paquet, &mut pcm)
        };
        let Ok(rendus) = rendus else {
            return false;
        };
        // Du vrai audio est revenu — reconstruit par le FEC ou décodé tel
        // quel : le budget de dissimulation repart entier. Sans cette ligne,
        // un plafond atteint une fois condamnerait la session au silence
        // définitif.
        if rendus > 0 {
            self.budget.trame_reelle();
        }
        self.pousser(&pcm[..rendus * CHANNELS]);
        rendus > 0
    }

    fn pousser(&mut self, pcm: &[i16]) {
        self.residu
            .extend(pcm.iter().map(|&e| e as f32 / 32_768.0));
    }
}

/// La fréquence dominante d'un signal périodique, par passages par zéro.
///
/// ⚠️ **Extraite vers `micro/frequence.rs`** au titre de la règle des 500
/// lignes, et ré-exportée ici pour qu'aucun site d'appel ne bouge : elle reste
/// `crate::micro::frequence_par_passages_a_zero` pour `demarrage/micro.rs`
/// comme pour les tests.
pub use frequence::frequence_par_passages_a_zero;

mod frequence;

/// La politique d'exclusivité du câble : un seul écrivain, la tentative
/// refaite à chaque dépôt, et un journal qui ne se répète pas.
///
/// ⚠️ **`pub mod`, et NON le `pub use` + `mod` que le plan E2 prescrit
/// (tâche 3, « Files »), et c'est une divergence assumée.** Ré-exporter trois
/// noms que personne ne consomme encore — les tâches 7 à 10 les câbleront —
/// lève un `unused_imports`, c'est-à-dire un avertissement d'une **catégorie
/// neuve**, alors que le contrôle de cette même tâche exige que tous les
/// avertissements restants soient des `dead_code`. Le plan se contredit sur ce
/// point ; on garde son contrôle et l'on suit le patron de `wasapi.rs`
/// (`pub mod rendu;`, `pub mod process_loopback;`). Les sites d'appel écriront
/// `crate::micro::exclusivite::Exclusivite`.
pub mod exclusivite;

/// Le plafond de dissimulation, et la règle pure qui le tient.
pub use dissimulation::{BudgetDissimulation, PLAFOND_DISSIMULATION};

mod dissimulation;

#[cfg(test)]
#[path = "micro/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "micro/tests_lecteur.rs"]
mod tests_lecteur;
