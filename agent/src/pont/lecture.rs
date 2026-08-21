//! **La fenêtre de lecture** : combien de morceaux on demande d'avance, et
//! l'invariant d'ordre qui rend cette avance sûre. **PUR** — aucun `cfg`,
//! aucune E/S, aucune horloge.
//!
//! # 🔴 POURQUOI CE MODULE EXISTE, ET POURQUOI IL NE POUVAIT PAS ÊTRE LIVRÉ
//! SEUL
//!
//! La spec §7.3 pose : « le pont ne demande pas le morceau *n+1* tant que le
//! canal a plus de `SEUIL_TAMPON` octets en attente ». **Relevé dans le code de
//! F1 et de F2 : il n'y a qu'UN morceau en vol à la fois** — le suivant n'est
//! demandé qu'à réception du précédent, et trois commentaires du dépôt
//! annonçaient la fenêtre comme un livrable de F3.
//!
//! **Avec un seul morceau en vol, la règle de la spec ne peut JAMAIS mordre :
//! le pont n'est jamais en avance.** Livrer `SEUIL_TAMPON` sans la fenêtre
//! serait livrer un mécanisme incapable de se déclencher — c'est-à-dire un
//! contrôle qu'on ne verra jamais rouge, appliqué cette fois à un mécanisme de
//! PRODUIT. F3 livre donc **les deux, ou aucun**.
//!
//! ⚠️ **La contre-pression, elle, est CÔTÉ NAVIGATEUR** (`client/src/fichiers/
//! flux.ts`), parce que c'est lui qui émet les gros messages et que
//! `bufferedAmount` est une propriété de SON canal. Le pont ne la voit pas et
//! ne peut pas la voir. Les deux moitiés sont indissociables : la fenêtre sans
//! la contre-pression remplirait la file SCTP, la contre-pression sans la
//! fenêtre n'aurait rien à retenir.
//!
//! # L'INVARIANT QUI REND LA FENÊTRE SÛRE, ET IL EST VÉRIFIÉ PLUTÔT QUE CRU
//!
//! Le canal est `ordered` (`client/src/fichiers/canal.ts`), les morceaux sont
//! demandés dans l'ordre croissant des positions, donc les réponses arrivent
//! dans cet ordre, donc `PrjWriteFileData` est appelé dans cet ordre.
//!
//! 🔴 **Ce module ne fait PAS confiance à SCTP pour autant.** Il vérifie que la
//! réponse reçue est bien celle attendue, et **dénonce** sinon au lieu de
//! l'appliquer. Écrire une plage au mauvais endroit produirait un fichier dont
//! **seul un condensat SHA-256 dirait qu'il est faux** — et le condensat de
//! bout en bout est précisément ce que F1 n'a JAMAIS établi (son legs n°6).
//!
//! ⚠️ **F3 NE REVENDIQUE AUCUN GAIN DE DÉBIT.** La seule mesure de débit du
//! dépôt est incohérente d'un facteur ~120 (6,5 Mio/s contre 52–55 Kio/s, F1
//! §11), sans explication. C'est F4 qui jugera ; F3 livre le mécanisme et le
//! rend observable par `en_vol_max`.

use std::collections::VecDeque;

use crate::pont::decoupe::Morceau;

/// Combien de morceaux au plus sont demandés d'avance.
///
/// ⚠️ **NON CALIBRÉE.** Elle rejoint `SEUIL_TAMPON`, `DELAI_MUTATION`,
/// `PERIODE_RECENSEMENT`, les quatre de F1, celles de F2 et les huit du
/// chantier D dans la liste des constantes qu'aucune mesure n'a jugées.
///
/// 🔴 **UNE VALEUR DE 1 RENDRAIT LA FENÊTRE INERTE**, c'est-à-dire livrerait un
/// contrôle de flux incapable de mordre. C'est ce que
/// `la_fenetre_atteint_reellement_MORCEAUX_EN_VOL_sur_une_lecture_longue`
/// dénonce, et c'est **le rouge du livrable lui-même**.
pub const MORCEAUX_EN_VOL: usize = 4;

/// Une réponse qui n'est pas celle qu'on attendait.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HorsOrdre {
    /// La position dont on a reçu la réponse.
    pub recue: u64,
    /// Celle qu'on attendait, s'il y en avait une.
    pub attendue: Option<u64>,
}

/// Les morceaux d'une lecture : ce qui reste à demander, ce qui est en vol.
#[derive(Debug)]
pub struct Fenetre {
    /// Ce qui n'a pas encore été demandé.
    restants: VecDeque<Morceau>,
    /// Les positions demandées et pas encore reçues, **dans l'ordre
    /// d'émission**.
    en_vol: VecDeque<u64>,
    /// Le maximum de `en_vol.len()` atteint. **Relevé au recensement** : c'est
    /// lui qui dit si la fenêtre a servi à quelque chose.
    en_vol_max: usize,
}

impl Fenetre {
    pub fn nouvelle(restants: VecDeque<Morceau>) -> Self {
        Self { restants, en_vol: VecDeque::new(), en_vol_max: 0 }
    }

    /// Les morceaux à demander MAINTENANT — **au plus [`MORCEAUX_EN_VOL`] en
    /// vol au total**, jamais par appel.
    ///
    /// ⚠️ **La borne porte sur le TOTAL en vol, pas sur le lot rendu.** Borner
    /// le lot laisserait la file croître sans terme : quatre par appel, appelé
    /// quatre fois, feraient seize en vol.
    pub fn a_demander(&mut self) -> Vec<Morceau> {
        let mut lot = Vec::new();
        while self.en_vol.len() < MORCEAUX_EN_VOL {
            let Some(morceau) = self.restants.pop_front() else { break };
            self.en_vol.push_back(morceau.position);
            lot.push(morceau);
        }
        self.en_vol_max = self.en_vol_max.max(self.en_vol.len());
        lot
    }

    /// Une réponse est arrivée pour `position`.
    ///
    /// 🔴 **ELLE DOIT ÊTRE LA PLUS ANCIENNE EN VOL**, et le reste est dénoncé.
    /// Appliquer une réponse hors d'ordre écrirait une plage au mauvais rang du
    /// fichier, et **seul un condensat SHA-256 l'attraperait**.
    ///
    /// ⚠️ **Une position INCONNUE est dénoncée aussi** — pas seulement une
    /// position en vol arrivée trop tôt. Une réponse tardive à une corrélation
    /// déjà résolue est jetée en amont par `pont::table` ; en recevoir une ici
    /// signifierait que les deux bouts ont divergé, et deviner ferait écrire
    /// n'importe quoi dans le tampon de ProjFS.
    pub fn recu(&mut self, position: u64) -> Result<(), HorsOrdre> {
        let attendue = self.en_vol.front().copied();
        if attendue != Some(position) {
            return Err(HorsOrdre { recue: position, attendue });
        }
        self.en_vol.pop_front();
        Ok(())
    }

    /// Le maximum de morceaux en vol atteint depuis le début de la lecture.
    pub fn en_vol_max(&self) -> usize {
        self.en_vol_max
    }

    /// Combien sont en vol à cet instant.
    pub fn en_vol(&self) -> usize {
        self.en_vol.len()
    }

    /// Plus rien à demander, plus rien en vol.
    pub fn terminee(&self) -> bool {
        self.restants.is_empty() && self.en_vol.is_empty()
    }
}

#[cfg(test)]
mod tests;
