//! Le sens MONTANT du micro, côté agent : ce qui arrive du navigateur en
//! Opus, remis dans l'ordre, débarrassé de ses doublons, borné en latence, et
//! rendu prêt à décoder.
//!
//! **Ce module est PUR : il ne référence jamais le crate `windows`, n'ouvre
//! aucun périphérique et ne décode rien.** C'est la ligne de partage de la
//! spec §6, et elle est ce qui rend éprouvable sous Linux tout ce qui peut
//! mal tourner — l'ordre, la gigue, la dérive, le silence. Le bloc E2 n'aura
//! qu'à réveiller un fil WASAPI et appeler `LecteurMicro::remplir`.
//!
//! **L'invariant du module** : `deposer` ne rend rien et ne peut donc jamais
//! faire attendre la boucle de transport. C'est le miroir exact de la règle du
//! chantier A — la boucle dépose, un fil dédié travaille.

use std::collections::VecDeque;
use std::time::Duration;

use crate::opus::SAMPLE_RATE_HZ;

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
    /// Rien à jouer : PLC si une trame a déjà été décodée, silence sinon.
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
    #[allow(dead_code)] // consommée par la correction de dérive, tâche 5
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
        let Some(tete) = self.file.front().map(|t| t.rtp_48k) else {
            self.compteurs.famines += 1;
            return Retrait::Manquante;
        };

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

#[cfg(test)]
mod tests {
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
}
