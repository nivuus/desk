//! Hystérésis : combien de temps une contrainte doit durer avant qu'on
//! descende d'un barreau, et combien avant qu'on remonte. Asymétrique à
//! dessein — voir `DELAI_REMONTEE`.

use std::time::{Duration, Instant};

/// Durée pendant laquelle la condition doit tenir avant de DESCENDRE.
const DELAI_DESCENTE: Duration = Duration::from_secs(2);
/// Durée pendant laquelle la condition doit tenir avant de REMONTER.
///
/// Dix fois plus long que la descente, et c'est délibéré : une estimation
/// qui oscille autour d'un seuil ferait sinon battre l'encodeur, et chaque
/// battement coûte une reconstruction du type de sortie et une image clé.
/// On dégrade vite pour rester fluide, on restaure lentement pour rester
/// stable.
///
/// **Ajusté de 10 s à 20 s à la tâche 12** (recette), après mesure sous le
/// profil `adsl` (8 Mb/s, 30 ms ±5 ms, sans perte) : 4 changements de barreau
/// observés en 49 s, alors que la propriété attendue est « au plus deux en
/// 60 s ». Preuve tracée dans le journal de l'agent : une remontée au
/// barreau plein (764×242 → 764×484, `taille d'encodage changée` à
/// 16:24:09.545) est suivie, une seconde plus tard, d'un effondrement de
/// l'estimation BWE de ×10 en une seule observation
/// (`estimation=Some(6639480)` à 16:24:10 puis `estimation=Some(619982)` à
/// 16:24:11) — cohérent avec l'image clé que le changement de résolution
/// déclenche lui-même, interprétée par l'estimateur comme une surcharge. La
/// remontée suivante retombe alors immédiatement (5,0 s plus tard, pile le
/// plancher `SEJOUR_MINIMAL`). Doubler `DELAI_REMONTEE` exige deux fois plus
/// de temps de confiance avant de reprendre la pleine résolution, ce qui
/// laisse au réseau (et à l'effet de la propre image clé du contrôleur) le
/// temps de se stabiliser avant la prochaine tentative. Remesuré après ce
/// changement (voir le document de résultats, §5) : plus aucune régression
/// observée sur ce point, mais l'échantillon reste court (une seule
/// fenêtre) — voir les réserves du document de résultats.
const DELAI_REMONTEE: Duration = Duration::from_secs(20);
/// Durée minimale entre deux changements de barreau, quelle que soit la
/// condition. Filet contre un aller-retour rapide autour d'un seuil.
const SEJOUR_MINIMAL: Duration = Duration::from_secs(5);

/// Durée après la PREMIÈRE estimation pendant laquelle la rampe du BWE ne doit
/// pas être prise pour une dégradation.
///
/// Le sous-système d'estimation part volontairement bas et sonde à la hausse
/// (voir `ESTIMATION_INITIALE_BPS` côté transport) : pendant cette montée, le
/// débit disponible est bas sans que le lien le soit. Sans cette fenêtre, toute
/// session sur une source 1080p annoncerait « Image réduite par le réseau » sur
/// un lien parfait, en bandeau persistant — mesuré : la rampe atteint 8,7 à
/// 17,7 Mb/s en 1 à 3 s sur gigabit.
///
/// `pub(super)` : lue par `controleur::observer`, qui est un module frère —
/// voir la doc de tête de `congestion.rs`.
pub(super) const DELAI_AMORCAGE: Duration = Duration::from_secs(5);

/// Filtre temporel asymétrique sur un indice de barreau.
///
/// Rend `Some(nouvel_indice)` à l'instant précis où un changement est retenu,
/// et `None` sinon. L'appelant n'a rien à mémoriser.
///
/// **À ne pas confondre avec `cursor::Hysteresis`**, qui compte des
/// observations booléennes consécutives : ici le filtre est temporel,
/// asymétrique, et porte sur une échelle ordonnée.
pub struct Hysteresis {
    courant: usize,
    /// Barreau visé de façon continue depuis `vise_depuis`, s'il diffère du
    /// courant.
    vise: Option<(usize, Instant)>,
    /// Instant du dernier changement retenu.
    dernier_changement: Instant,
}

impl Hysteresis {
    pub fn new(barreau_initial: usize, now: Instant) -> Self {
        Self {
            courant: barreau_initial,
            vise: None,
            // Placé de façon à ce que le temps de séjour soit déjà écoulé au
            // démarrage : la toute première adaptation ne doit pas attendre
            // 5 s de plus que sa propre condition.
            dernier_changement: now - SEJOUR_MINIMAL,
        }
    }

    pub fn observer(&mut self, vise: usize, now: Instant) -> Option<usize> {
        if vise == self.courant {
            // Retour au barreau courant : toute intention de changement en
            // cours est annulée.
            self.vise = None;
            return None;
        }

        // Un barreau visé DIFFÉRENT de celui déjà en cours d'observation
        // redémarre le décompte : la condition n'a pas « tenu », elle a
        // changé de cible.
        let depuis = match self.vise {
            Some((precedent, depuis)) if precedent == vise => depuis,
            _ => {
                self.vise = Some((vise, now));
                now
            }
        };

        // Indices croissants = résolutions décroissantes : viser plus grand
        // que le courant, c'est descendre.
        let delai = if vise > self.courant { DELAI_DESCENTE } else { DELAI_REMONTEE };
        if now.duration_since(depuis) < delai {
            return None;
        }
        if now.duration_since(self.dernier_changement) < SEJOUR_MINIMAL {
            return None;
        }

        self.courant = vise;
        self.vise = None;
        self.dernier_changement = now;
        Some(vise)
    }
}

/// Instant de référence des tests. Placé loin dans le passé pour que toute
/// soustraction de durée reste valide.
///
/// `pub(super)` : les tests de `controleur` et de `reconfiguration`, modules
/// frères, en ont aussi besoin — voir la doc de tête de `congestion.rs`.
#[cfg(test)]
pub(super) fn t0() -> Instant {
    Instant::now() - Duration::from_secs(3600)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descendre_exige_deux_secondes_sous_le_barreau() {
        // Base liée UNE SEULE FOIS : `t0()` rend un instant neuf à chaque
        // appel, et des assertions posées sur des bornes exactes (2,000 s)
        // deviendraient instables à quelques microsecondes près.
        let base = t0();
        let mut h = Hysteresis::new(0, base);

        // Première observation du barreau 1 : le décompte DÉMARRE ici, il ne
        // s'est encore rien écoulé.
        assert_eq!(h.observer(1, base + Duration::from_millis(1900)), None);
        // 1,999 s après le début du décompte : pas encore.
        assert_eq!(h.observer(1, base + Duration::from_millis(3899)), None);
        // 2,000 s pile : on descend.
        assert_eq!(h.observer(1, base + Duration::from_millis(3900)), Some(1));
    }

    #[test]
    fn un_repit_remet_le_compteur_de_descente_a_zero() {
        let base = t0();
        let mut h = Hysteresis::new(0, base);

        assert_eq!(h.observer(1, base + Duration::from_millis(1900)), None);
        // Une seule observation revenue au barreau courant annule le décompte.
        assert_eq!(h.observer(0, base + Duration::from_millis(1950)), None);
        // Le décompte repart de zéro à 3000 ms.
        assert_eq!(h.observer(1, base + Duration::from_millis(3000)), None);
        // 1,9 s après ce nouveau départ : toujours pas.
        assert_eq!(h.observer(1, base + Duration::from_millis(4900)), None);
        // 2,0 s après : cette fois oui.
        assert_eq!(h.observer(1, base + Duration::from_millis(5000)), Some(1));
    }

    #[test]
    fn remonter_exige_vingt_secondes_et_non_deux() {
        let base = t0();
        // Départ au barreau 1 : `new` place le dernier changement dans le
        // passé, donc le temps de séjour n'entrave pas ce test.
        let mut h = Hysteresis::new(1, base);

        assert_eq!(h.observer(0, base + Duration::from_millis(2000)), None);
        // 2,0 s pile après le début du décompte : une DESCENTE aurait basculé
        // ici, le seuil étant atteint. Une remontée, non — c'est tout l'objet
        // de ce test.
        assert_eq!(h.observer(0, base + Duration::from_millis(4000)), None);
        // 19,999 s : toujours pas (DELAI_REMONTEE = 20 s depuis la tâche 12).
        assert_eq!(h.observer(0, base + Duration::from_millis(21_999)), None);
        // 20,000 s pile : on remonte.
        assert_eq!(h.observer(0, base + Duration::from_millis(22_000)), Some(0));
    }

    #[test]
    fn le_temps_de_sejour_bloque_un_second_changement_trop_proche() {
        let base = t0();
        let mut h = Hysteresis::new(0, base);

        // Première descente : décompte démarré à 0, retenu à 2,0 s.
        assert_eq!(h.observer(1, base), None);
        assert_eq!(h.observer(1, base + Duration::from_millis(2000)), Some(1));

        // La condition de descente vers 2 est remplie 2 s plus tard, mais le
        // temps de séjour de 5 s depuis le dernier changement l'interdit.
        assert_eq!(h.observer(2, base + Duration::from_millis(2001)), None);
        assert_eq!(h.observer(2, base + Duration::from_millis(4001)), None);
        // À 7,000 s : 5,0 s de séjour écoulées ET la condition tient depuis
        // 4,999 s. Les deux verrous sont levés.
        assert_eq!(h.observer(2, base + Duration::from_millis(7000)), Some(2));
    }

    #[test]
    fn cibler_un_second_barreau_sans_repasser_par_le_courant_redemarre_le_decompte() {
        // Trouvaille triviale de la revue finale : ce chemin (branche
        // `_ => { self.vise = Some((vise, now)); now }` d'`observer`) n'était
        // exercé par aucun test. On vise d'abord 1, puis on change de cible
        // vers 2 SANS jamais repasser par le barreau courant (0) entre les
        // deux — le décompte doit repartir de zéro pour la nouvelle cible, ne
        // pas se poursuivre depuis la première.
        let base = t0();
        let mut h = Hysteresis::new(0, base);

        // Vise 1 : décompte démarré à 0 ms.
        assert_eq!(h.observer(1, base + Duration::from_millis(500)), None);
        // Change de cible vers 2 à 1000 ms, sans repasser par 0 : le
        // décompte pour 2 doit repartir de 1000 ms, pas de 0 ms.
        assert_eq!(h.observer(2, base + Duration::from_millis(1000)), None);
        // 1,999 s après ce redémarrage (2999 ms) : si le décompte avait
        // continué depuis le tout premier `observer` (0 ms), il serait déjà
        // à 2,999 s et aurait basculé — la preuve que ce n'est pas le cas.
        assert_eq!(h.observer(2, base + Duration::from_millis(2999)), None);
        // 2,000 s pile après le redémarrage à 1000 ms : bascule vers 2, la
        // cible la plus récente — jamais vers 1.
        assert_eq!(h.observer(2, base + Duration::from_millis(3000)), Some(2));
    }
}
