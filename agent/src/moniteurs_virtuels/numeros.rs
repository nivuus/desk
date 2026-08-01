//! Attribution des numéros qui distinguent nos GUID de sortie virtuelle.
//!
//! **Correctif I1 de la revue finale de branche.** Le numéro était un simple
//! compteur strictement monotone, jamais recyclé : `pilote::creer`
//! l'incrémentait à chaque création et rien ne le faisait redescendre à la
//! destruction. Or le superviseur crée une sortie par **ouverture** de fenêtre,
//! sans borne — là où le banc qui a précédé n'en créait que dix dans toute sa
//! vie. À la 17ᵉ ouverture, le GUID attribué sortait de la plage que la purge
//! inter-processus balaye (`purge::purger`, `1..=PLAFOND_NUMEROS`) : un arrêt
//! brutal laissait alors un moniteur virtuel **irrécupérable sans redémarrer la
//! VM**, puisque le pilote ne retire que par GUID et qu'aucune table ne survit
//! au processus.
//!
//! **Recycler plutôt que relever le plafond**, et c'est un arbitrage, pas un
//! détail : relever le plafond ne borne rien — le compteur restait croissant
//! sans limite, et repousser le mur à 64 ou à 1024 n'aurait fait que retarder
//! le même défaut tout en allongeant d'autant le balayage de la purge. Recycler
//! borne l'ensemble des numéros en vol par ce qui est réellement dû
//! (sorties vivantes + retraits en échec), quantité que le pilote plafonne
//! lui-même à dix. Le plafond subsiste comme **garde-fou dur** : au-delà, la
//! création est refusée bruyamment plutôt que d'attribuer un numéro que la
//! purge ne retrouverait jamais.
//!
//! Module hors `#[cfg(windows)]` à dessein : c'est de la logique pure, et
//! c'est la pièce dont dépend la récupérabilité d'un état global du système.

use anyhow::Result;

/// Numéro le plus élevé qu'une sortie puisse porter — donc l'étendue exacte
/// que `purge::purger` doit balayer pour retrouver les GUID d'un processus
/// tué net.
///
/// Seize et non dix : le pilote refuse la 11ᵉ sortie simultanée (mesuré à la
/// tâche 6), et les six numéros de marge couvrent les retraits en échec, qui
/// restent dus sans être recyclés.
///
/// **C'est ce plafond-ci qui fait autorité, pas
/// `diagnostics::multifenetre::montee::PLAFOND_RECHERCHE`** — lequel dit
/// jusqu'où une MESURE grimpe, une question sans rapport qui partageait
/// seulement la même valeur.
pub const PLAFOND_NUMEROS: u16 = 16;

/// Distributeur de numéros, avec reprise de ceux qui ont été rendus.
#[derive(Default)]
pub struct Numeros {
    /// Plus haut numéro jamais attribué. Ne redescend pas : ce qui redescend,
    /// c'est le contenu de `libres`.
    plus_haut: u16,
    /// Numéros rendus par une sortie RÉELLEMENT retirée, donc réattribuables.
    libres: Vec<u16>,
}

impl Numeros {
    /// Attribue un numéro, en reprenant d'abord ceux qui ont été rendus.
    ///
    /// Échoue plutôt que de déborder le plafond : un numéro hors plage serait
    /// un moniteur que plus aucune purge ne pourrait retirer.
    pub fn attribuer(&mut self) -> Result<u16> {
        if let Some(recycle) = self.libres.pop() {
            return Ok(recycle);
        }
        anyhow::ensure!(
            self.plus_haut < PLAFOND_NUMEROS,
            "plus aucun numéro de sortie virtuelle disponible : {PLAFOND_NUMEROS} sont \
             attribués et non rendus. Créer au-delà donnerait un GUID que la purge \
             inter-processus ne balaye pas, donc un moniteur irrécupérable sans \
             redémarrage de la VM"
        );
        self.plus_haut += 1;
        Ok(self.plus_haut)
    }

    /// Rend un numéro dont la sortie a été **réellement** retirée.
    ///
    /// Un numéro jamais attribué, ou déjà rendu, est ignoré sans bruit : ce
    /// n'est pas une erreur de l'appelant, c'est le cas d'un GUID régénéré par
    /// la purge, ou d'un `oublier` rejoué.
    ///
    /// Ne JAMAIS appeler sur un retrait qui a échoué : le moniteur existe
    /// encore, son GUID est la seule prise qu'on ait dessus, et réattribuer ce
    /// numéro ferait fabriquer un second GUID identique.
    pub fn rendre(&mut self, numero: u16) {
        if numero == 0 || numero > self.plus_haut || self.libres.contains(&numero) {
            return;
        }
        self.libres.push(numero);
    }

    /// Numéros attribués et pas encore rendus. Sert au journal.
    pub fn en_vol(&self) -> usize {
        self.plus_haut as usize - self.libres.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_numeros_se_suivent_tant_que_rien_n_est_rendu() {
        let mut numeros = Numeros::default();
        assert_eq!(numeros.attribuer().unwrap(), 1);
        assert_eq!(numeros.attribuer().unwrap(), 2);
        assert_eq!(numeros.attribuer().unwrap(), 3);
        assert_eq!(numeros.en_vol(), 3);
    }

    /// Le défaut I1 lui-même : un superviseur ouvre et ferme des fenêtres sans
    /// borne. Avec un compteur monotone, la 17ᵉ ouverture attribuait un numéro
    /// hors de la plage balayée par la purge.
    #[test]
    fn cent_ouvertures_fermetures_ne_font_pas_sortir_du_plafond() {
        let mut numeros = Numeros::default();
        for _ in 0..100 {
            let numero = numeros.attribuer().expect("le recyclage doit rendre la place");
            assert!(numero <= PLAFOND_NUMEROS, "numéro {numero} hors de portée de la purge");
            numeros.rendre(numero);
        }
        assert_eq!(numeros.en_vol(), 0);
    }

    /// Huit fenêtres ouvertes en même temps — la cible du chantier — puis
    /// renouvelées indéfiniment : les numéros restent dans la plage.
    #[test]
    fn huit_sorties_simultanees_renouvelees_restent_dans_la_plage() {
        let mut numeros = Numeros::default();
        let mut vivantes: Vec<u16> = (0..8).map(|_| numeros.attribuer().unwrap()).collect();
        for _ in 0..50 {
            let rendue = vivantes.remove(0);
            numeros.rendre(rendue);
            let neuve = numeros.attribuer().unwrap();
            assert!(neuve <= PLAFOND_NUMEROS, "numéro {neuve} hors de portée de la purge");
            vivantes.push(neuve);
        }
        assert_eq!(numeros.en_vol(), 8);
    }

    /// Le garde-fou dur : un état où rien n'est rendu (retraits tous en échec)
    /// doit faire refuser la création, pas attribuer un numéro invisible de la
    /// purge.
    #[test]
    fn au_dela_du_plafond_la_creation_est_refusee() {
        let mut numeros = Numeros::default();
        for _ in 0..PLAFOND_NUMEROS {
            numeros.attribuer().unwrap();
        }
        assert!(numeros.attribuer().is_err(), "le plafond aurait dû refuser");
        // Un seul retrait réussi rouvre exactement une place.
        numeros.rendre(4);
        assert_eq!(numeros.attribuer().unwrap(), 4);
        assert!(numeros.attribuer().is_err());
    }

    #[test]
    fn rendre_un_numero_inconnu_ou_deja_rendu_ne_cree_pas_de_doublon() {
        let mut numeros = Numeros::default();
        let a = numeros.attribuer().unwrap();
        numeros.rendre(a);
        // Deux fois le même, plus un jamais attribué, plus zéro.
        numeros.rendre(a);
        numeros.rendre(999);
        numeros.rendre(0);
        assert_eq!(numeros.attribuer().unwrap(), a);
        assert_eq!(numeros.attribuer().unwrap(), a + 1, "aucun doublon n'a été mis en réserve");
    }
}
