//! Les trois IOCTL du pilote qui n'ajoutent ni ne retirent rien : version de
//! protocole, ping du chien de garde, lecture de sa veille.
//!
//! **Module ENFANT de `pilote`**, et non frère : c'est ce qui lui donne accès à
//! `PiloteParIoctl::commander`, restée privée. Extrait de `pilote.rs` à la revue
//! finale de branche parce que le correctif I1 y ajoutait le recyclage des
//! numéros et portait le fichier à 507 lignes, au-dessus du plafond de 500 du
//! projet (voir `CLAUDE.md`) : l'addition s'accompagne de son extraction.
//!
//! Ces trois-là forment un bloc naturel — aucune ne touche à l'état des
//! sorties, aucune n'a d'effet de bord sur la topologie, et toutes trois ne
//! servent qu'à interroger ou entretenir le pilote. Rien n'a changé de valeur
//! au déplacement.

use anyhow::Result;

use super::PiloteParIoctl;
use crate::moniteurs_virtuels::sudovda::{
    Veille, VersionProtocole, IOCTL_LIRE_VEILLE, IOCTL_LIRE_VERSION_PROTOCOLE, IOCTL_PINGUER,
};

impl PiloteParIoctl {
    /// Version de protocole annoncée par le pilote installé. Sans effet de
    /// bord — le tampon le plus simple des six.
    pub(crate) fn version_protocole(&self) -> Result<(VersionProtocole, u32)> {
        let mut version = VersionProtocole::default();
        let rendus = self.commander(
            IOCTL_LIRE_VERSION_PROTOCOLE,
            None,
            Some((
                &mut version as *mut _ as *mut _,
                std::mem::size_of::<VersionProtocole>() as u32,
            )),
            "lecture de la version de protocole du pilote",
        )?;
        Ok((version, rendus))
    }

    /// Réarme le chien de garde du pilote pour CE handle.
    ///
    /// Ni entrée ni sortie : c'est le seul des six IOCTL dont les deux tampons
    /// soient vides, donc le seul dont aucune disposition supposée ne puisse
    /// être fausse.
    ///
    /// **Pourquoi ce battement n'est pas lancé ici, dans un fil interne.** Le
    /// pilote associe vraisemblablement son chien de garde au *file object*
    /// ouvert par `CreateFile` — c'est ce que fait le client amont, qui pingue
    /// sur le handle même dont il s'est servi pour ajouter ses sorties. Pinguer
    /// depuis un second handle ne sauverait donc rien. Un fil interne devrait
    /// alors partager CE handle, ce qui obligerait à le rendre `Send` ; or les
    /// deux seuls appelants de ce module sont séquentiels par construction et
    /// n'ont besoin que de ponctuer leurs attentes. On expose le battement,
    /// l'appelant tient la cadence.
    pub(crate) fn pinguer(&self) -> Result<()> {
        self.commander(IOCTL_PINGUER, None, None, "ping du chien de garde du pilote")?;
        Ok(())
    }

    /// Délai et décompte du chien de garde du pilote. Sans effet de bord.
    pub(crate) fn veille(&self) -> Result<(Veille, u32)> {
        let mut veille = Veille::default();
        let rendus = self.commander(
            IOCTL_LIRE_VEILLE,
            None,
            Some((&mut veille as *mut _ as *mut _, std::mem::size_of::<Veille>() as u32)),
            "lecture du watchdog du pilote",
        )?;
        Ok((veille, rendus))
    }}
