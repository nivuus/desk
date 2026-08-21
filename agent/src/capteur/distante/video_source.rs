//! `impl VideoSource for SourceDistante` — EXTRAIT VERBATIM de `distante.rs`.
//!
//! **Pourquoi ce fichier existe** : `distante.rs` était à **472 lignes** pour
//! un plafond de projet à 500, et le sous-bloc A1 (la couleur d'accent) doit y
//! greffer le patron `PressePapier` — une variante de `Recu`, un champ de
//! rétention, un bras et un accesseur, soit une trentaine de lignes. **La marge
//! de 28 ne suffisait pas.** L'extraction est donc jouée **AVANT** l'addition
//! qui la rend nécessaire, dans son propre commit et sans aucune autre
//! modification : c'est la doctrine du dépôt, et elle a cinq précédents (D9
//! tâche 6 ; D10 tâches 1 à 3 ; P3 tâches 3 et 4 ; G1 tâche 1). **Jamais une
//! compression** — D9 l'a payé deux fois.
//!
//! ⚠️ **Le `#[path]` qui déclare ce module est HORS de la convention de
//! `CLAUDE.md`** (§ « Convention de module enfant ») : celle-ci ne vise que les
//! modules extraits d'un parent `#[cfg(windows)]` pour compiler sur l'hôte.
//! C'est ici le **second** usage du même mécanisme Rust — scinder un fichier
//! trop long —, celui que `distante/tests.rs` et `distante/tests_etats.rs`
//! emploient déjà dans ce même fichier, et il ne suit pas la règle du préfixe.
//!
//! ⚠️ **Un `impl` de trait dans un module enfant est global : aucun site
//! d'appel n'a bougé, et aucune ré-exportation n'est nécessaire.**

use anyhow::{bail, Result};

use super::{Rattachee, Recu, SourceDistante};
use crate::capteur::protocole::{DepuisCapteur, VersCapteur};
use crate::h264::AccessUnit;
use crate::source::VideoSource;
use std::sync::mpsc::TryRecvError;
use std::time::Instant;

impl VideoSource for SourceDistante {
    fn next_frame(&mut self) -> Option<AccessUnit> {
        loop {
            match self.images.try_recv() {
                Ok(Recu::Image(unite)) => {
                    self.fenetre.succes();
                    return Some(unite);
                }
                Ok(Recu::Etat { vivante, epuisee, largeur, hauteur }) => {
                    self.vivante = vivante;
                    self.epuisee = epuisee;
                    self.largeur = largeur;
                    self.hauteur = hauteur;
                }
                Ok(Recu::Sommeil { endormie, raison }) => {
                    // Deux écritures, deux durées de vie : l'état courant, qui
                    // survit à sa lecture, et l'annonce, qui ne s'y survit pas.
                    self.endormie = endormie;
                    self.sommeil = Some((endormie, raison));
                }
                Ok(Recu::Part { bps }) => {
                    self.part = Some(bps);
                }
                Ok(Recu::Audio { actif }) => {
                    self.audio = Some(actif);
                }
                Ok(Recu::PleinEcran { actif }) => {
                    self.plein_ecran = Some(actif);
                }
                Ok(Recu::PressePapier { texte, octets }) => {
                    self.presse_papier = Some((texte, octets));
                }
                Ok(Recu::Accent { couleur }) => {
                    self.accent = Some(couleur);
                }
                // Le cas COURANT et normal : rien de neuf ce tour-ci. La
                // boucle de transport interroge à 100 Hz une source qui
                // produit à ~90 i/s.
                Err(TryRecvError::Empty) => {
                    self.fenetre.succes();
                    return None;
                }
                // Le canal est rompu. La fenêtre de reprise borne combien de
                // temps la source reste vivante en attendant un rattachement.
                Err(TryRecvError::Disconnected) => {
                    // **Un épuisement AUTORITAIRE ne se retente pas** (I2,
                    // revue finale de branche du sous-bloc D4). À chaque
                    // fermeture NORMALE d'une fenêtre, le capteur pousse un
                    // `Etat { epuisee: true }` puis ferme le tube : l'enfant
                    // consomme cet état, reboucle, et voit `Disconnected` dans
                    // le même tour. Sans cette garde il rattacherait
                    // aussitôt — un tube de commandes neuf, un tube média
                    // neuf, et côté capteur une duplication DXGI et un
                    // `H264Encoder` neufs sur une sortie que le superviseur
                    // est justement en train de détruire. Pire, un
                    // rattachement qui aboutirait remettrait `epuisee` à faux
                    // (plus bas) : la session qui devait se clore ne se
                    // clorait pas, et le résultat dépendrait d'une course.
                    if self.epuisee {
                        return None;
                    }
                    let maintenant = Instant::now();
                    if self.fenetre.rupture(maintenant) {
                        // La fenêtre est expirée : l'épuisement est acquis.
                        self.epuisee = true;
                        return None;
                    }
                    if self.fenetre.peut_reessayer(maintenant) {
                        match self.canal.rattacher() {
                            Ok(Rattachee { images, largeur, hauteur }) => {
                                tracing::info!(largeur, hauteur, "canal rattaché au capteur");
                                self.images = images;
                                self.largeur = largeur;
                                self.hauteur = hauteur;
                                self.vivante = true;
                                self.epuisee = false;
                                // Un rattachement passe par `VersCapteur::Attache`,
                                // donc par une `Fenetre` NEUVE côté capteur — et
                                // une fenêtre naît endormie. Garder ici l'état
                                // d'avant la rupture ferait appliquer la part
                                // plancher de cette renaissance comme un plafond
                                // d'encodage, pour toute la durée qui sépare
                                // l'attache du premier `Ordre::Reveiller`.
                                self.endormie = true;
                                // ⚠️ **`Some(false)`, PAS `None` : au
                                // rattachement, l'enfant REDEVIENT MUET**
                                // (conception §4.4 ; F2, revue finale de
                                // branche du sous-bloc D7). `None` ne veut
                                // pas dire « muet », il veut dire « rien à
                                // changer » : le drapeau `emet` de la
                                // `WindowsAudioSource` gardait alors sa valeur
                                // d'AVANT la rupture, et une fenêtre qui
                                // portait le son continuait de le porter.
                                //
                                // Le cas qui mord : deux fenêtres A et B d'un
                                // même PID, A porteuse, le capteur redémarre.
                                // B se rattache la première, son groupe est
                                // vide côté capteur — donc elle est élue et
                                // démarre. A se rattache quelques dizaines à
                                // quelques centaines de millisecondes plus
                                // tard (D4 a mesuré 538 à 689 ms pour le seul
                                // rattachement, et rien ne synchronise les deux
                                // enfants) en émettant TOUJOURS : les deux
                                // jouent le même mix du même PID, désynchronisé
                                // — un écho audible — jusqu'à ce que le
                                // `Audio { actif: false }` destiné à A arrive.
                                //
                                // Le défaut inverse n'existe pas : un
                                // rattachement passe par `VersCapteur::Attache`,
                                // donc par une `Fenetre` NEUVE côté capteur,
                                // dont `sommeil::inscrire` purge
                                // `derniers_audio` — un ordre neuf arrive donc
                                // TOUJOURS, et sans la borne du tour de roue.
                                // `inscrire` appelle `distribuer_l_audio`
                                // SYNCHRONEMENT, avant même que `boucler` ne
                                // démarre sa boucle (`fenetre.rs`, `servir`),
                                // laquelle sonde `ordres` en tête de chaque
                                // tour, sans délai. Ce n'est PAS le résidu de
                                // `sommeil/porteurs.rs` (un ordre différé au
                                // TOUR DE ROUE SUIVANT, borné
                                // `PERIODE_REARBITRAGE` = 250 ms) : ce
                                // résidu-là ne joue que quand le canal d'une
                                // fenêtre VOISINE casse pendant la MÊME passe
                                // d'arbitrage. Le silence d'une porteuse qui se
                                // rattache est donc borné par l'acheminement du
                                // message sur le fil, et c'est l'arbitrage que
                                // la conception a choisi : un blanc bref plutôt
                                // qu'un écho.
                                self.audio = Some(false);
                                // Consommé par `rattachement_survenu`, pour
                                // remettre à zéro `Session::audio_mort_signale`
                                // : un `AudioMort` déjà signalé avant la
                                // rupture n'est pas garanti connu du capteur de
                                // l'autre côté de CE rattachement (le cas visé
                                // est le capteur relancé, dont le registre
                                // d'inaptitudes repart vide en mémoire).
                                self.rattache = true;
                                self.fenetre.succes();
                            }
                            // Journalisé en `debug!` et non `info!` : au pas
                            // de 250 ms sur une fenêtre de 15 s, un capteur
                            // durablement absent produirait 60 lignes par
                            // fenêtre et par session.
                            Err(erreur) => tracing::debug!(%erreur, "rattachement refusé"),
                        }
                    }
                    return None;
                }
            }
        }
    }

    fn dimensions(&self) -> (u32, u32) {
        (self.largeur, self.hauteur)
    }

    fn is_exhausted(&self) -> bool {
        self.epuisee
    }

    fn is_alive(&self) -> bool {
        self.vivante
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        match self
            .canal
            .commander(VersCapteur::Redimensionner { largeur: width, hauteur: height })?
        {
            // La taille RETENUE est celle obtenue, jamais celle demandée : le
            // pilote quantifie, et une fenêtre Windows impose des dimensions
            // paires. Même règle qu'en mono-fenêtre.
            DepuisCapteur::Taille { largeur, hauteur } => {
                self.largeur = largeur;
                self.hauteur = hauteur;
                Ok(())
            }
            DepuisCapteur::Erreur { motif } => bail!("le capteur a refusé : {motif}"),
            autre => bail!("réponse inattendue du capteur : {autre:?}"),
        }
    }

    fn set_bitrate(&mut self, bitrate: u32) -> Result<()> {
        self.commander_simple(VersCapteur::Debit { bps: bitrate })
    }

    fn set_encode_size(&mut self, width: u32, height: u32) -> Result<()> {
        self.commander_simple(VersCapteur::TailleEncodage { largeur: width, hauteur: height })
    }

    fn request_keyframe(&mut self) -> Result<()> {
        self.commander_simple(VersCapteur::ImageCle)
    }

    fn set_awake(&mut self, visible: bool, focalisee: bool) -> Result<()> {
        self.commander_simple(VersCapteur::Visibilite { visible, focalisee })
    }

    /// Fait écrire le presse-papier de la VM par le CAPTEUR, qui en est le seul
    /// propriétaire (D1), et **attend sa réponse**.
    ///
    /// 🔴 **Synchrone à dessein, et c'est tout l'ordre de D6** :
    /// `commander_simple` bloque jusqu'au `Fait` du capteur, si bien que
    /// l'appelant (`transport/tick.rs`, branche `a1octies`) ne peut armer
    /// l'injection de `Ctrl+V` qu'après une écriture RÉELLEMENT survenue.
    /// Aucun ordonnancement de canal n'entre là-dedans.
    ///
    /// ⚠️ **Le coût est réel et il est nommé** : cet appel bloque la boucle de
    /// transport le temps d'un aller-retour de tube. Le précédent existe et il
    /// est exercé — `set_awake` juste au-dessus commande le capteur depuis
    /// cette même boucle, et D4 a mesuré une attache en 34 µs. Mais la borne du
    /// canal est de **12 s**, et un capteur mort ferait attendre la boucle
    /// jusque-là. **Ce chemin-là n'a jamais couru** (la borne de 12 s est
    /// déclarée « code jamais couru » depuis D4) : c'est un legs de P2, pas un
    /// remède.
    ///
    /// `commander_simple` — et non `commander` nu — parce que lui seul traduit
    /// `DepuisCapteur::Erreur` en `Err` : un refus d'ouverture du presse-papier
    /// par une autre application (cas NORMAL sous Windows) doit empêcher
    /// l'injection, pas la laisser passer.
    fn ecrire_le_presse_papier(&mut self, texte: &str) -> Result<()> {
        self.commander_simple(VersCapteur::PressePapierEcrire { texte: texte.to_owned() })
    }

    /// Rend le changement de sommeil en attente, et le consomme.
    ///
    /// **Une annonce ne se répète pas** : la boucle de transport l'interroge à
    /// chaque tour, et réémettre le même message inonderait le canal de
    /// contrôle.
    fn sommeil_a_annoncer(&mut self) -> Option<(bool, String)> {
        self.sommeil.take()
    }

    /// Rend la part en attente, et la consomme.
    fn part_a_appliquer(&mut self) -> Option<u32> {
        self.part.take()
    }

    /// Rend l'ordre audio en attente, et le consomme.
    fn audio_a_appliquer(&mut self) -> Option<bool> {
        self.audio.take()
    }

    /// Rend le changement de plein écran en attente, et le consomme.
    fn plein_ecran_a_annoncer(&mut self) -> Option<bool> {
        self.plein_ecran.take()
    }

    /// Rend le presse-papier en attente d'annonce, et le consomme.
    fn presse_papier_a_annoncer(&mut self) -> Option<(Option<String>, u32)> {
        self.presse_papier.take()
    }

    /// Rend la couleur d'accent en attente d'annonce, et la consomme.
    fn accent_a_annoncer(&mut self) -> Option<String> {
        self.accent.take()
    }

    /// Rend l'état de sommeil courant, sans le consommer.
    fn est_endormie(&self) -> bool {
        self.endormie
    }

    fn signaler_audio_mort(&mut self) {
        if let Err(erreur) = self.commander_simple(VersCapteur::AudioMort) {
            tracing::warn!(%erreur, "signalement de capture audio morte non délivré");
        }
    }

    /// Voir le trait : prévient le capteur qu'un paquet réel a prouvé la
    /// reprise de la capture audio — la PREUVE, pas la seule décision de
    /// reconstruction (sous-bloc D10).
    fn signaler_audio_vivant(&mut self) {
        if let Err(erreur) = self.commander_simple(VersCapteur::AudioVivant) {
            tracing::warn!(%erreur, "signalement de capture audio vivante non délivré");
        }
    }

    fn rattachement_survenu(&mut self) -> bool {
        std::mem::take(&mut self.rattache)
    }
}
