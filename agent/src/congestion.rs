//! Contrôleur de congestion : décide du débit et de la résolution d'encodage
//! à partir de ce que le pair rapporte.
//!
//! Aucune dépendance à Windows, à str0m ni au socket — c'est ce qui rend
//! toute la politique testable sur Linux, sans VM et sans réseau. Même
//! raison d'être que `geometry.rs`, `rebuild.rs` et `clock.rs`.

/// Diviseurs successifs appliqués à la taille source pour former l'échelle.
///
/// Quatre barreaux, choisis pour que chaque descente soit visible sans être
/// brutale : de 1080p on passe à 864p, puis 720p, puis 540p.
const DIVISEURS: [f32; 4] = [1.0, 1.25, 1.5, 2.0];

/// Débit minimal, en bits par pixel et par image, en dessous duquel un barreau
/// devient laid.
///
/// **C'est LE réglage du contrôleur.** La valeur de départ est choisie pour
/// donner une échelle cohérente sous le plafond de 12 Mb/s en 1080p60 (6,2 →
/// 4,0 → 2,8 → 1,6 Mb/s), pas mesurée.
///
/// **Issue réelle (tâche 12, recette netem) :** RECONDUITE, faute de preuve
/// du contraire — pas confirmée par une inspection visuelle positive. Sous
/// `adsl` (8 Mb/s), le seuil du barreau plein pour la source captée valait
/// ≈1,11 Mb/s, largement sous le débit du lien : les descentes observées
/// venaient de l'instabilité de l'estimation BWE (voir `DELAI_REMONTEE`), pas
/// d'un seuil mal calibré. Mais le critère qui aurait permis de VALIDER cette
/// valeur (« l'image en pleine résolution était visiblement acceptable ou
/// dégradée ») suppose un jugement visuel qui n'a jamais été fait — aucune
/// capture d'écran n'a été comparée à l'œil. Voir
/// `docs/superpowers/plans/2026-07-29-reseau-adaptatif-resultats.md`, §5.
const BPP_MIN: f32 = 0.05;

/// Un barreau de l'échelle : une taille d'encodage et le débit en dessous
/// duquel elle cesse d'être regardable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Barreau {
    pub taille: (u32, u32),
    pub min_bps: u32,
}

/// Échelle de résolutions dérivée d'une taille source et d'une cadence.
///
/// L'échelle contient **au plus quatre barreaux**, strictement décroissants en
/// largeur. La troncature au pixel pair (H.264) peut faire converger plusieurs
/// diviseurs vers la même taille pour les sources minuscules — dans ce cas,
/// l'échelle les fusionne, tout en garantissant au moins un barreau (le
/// plancher), ce qui évite les préconditions inutiles en amont (qui ne
/// garantissent pas une taille source minimale).
///
/// Le premier barreau est toujours la taille source (diviseur 1.0).
#[derive(Debug, Clone)]
pub struct Echelle {
    barreaux: Vec<Barreau>,
}

impl Echelle {
    pub fn depuis(source: (u32, u32), fps: u32) -> Self {
        let (sw, sh) = source;
        let tous_barreaux: Vec<Barreau> = DIVISEURS
            .iter()
            .map(|d| {
                // `& !1` : H.264 exige des dimensions paires. La même
                // contrainte est déjà appliquée par `WindowsSource::resize`.
                // `.max(2)` empêche une source minuscule de produire une
                // dimension nulle, que Media Foundation refuserait.
                let w = (((sw as f32) / d) as u32 & !1).max(2);
                let h = (((sh as f32) / d) as u32 & !1).max(2);
                let pixels = w as u64 * h as u64;
                let min_bps = (pixels * fps as u64) as f32 * BPP_MIN;
                Barreau { taille: (w, h), min_bps: min_bps as u32 }
            })
            .collect();

        // Éliminer les barreaux dont la taille est identique au précédent.
        // Cela peut arriver pour les sources minuscules, en raison de la
        // troncature au pixel pair et du plancher `.max(2)`.
        let mut barreaux: Vec<Barreau> = Vec::new();
        for barreau in tous_barreaux {
            if barreaux.is_empty() || barreau.taille != barreaux.last().unwrap().taille {
                barreaux.push(barreau);
            }
        }

        Self { barreaux }
    }

    pub fn barreaux(&self) -> &[Barreau] {
        &self.barreaux
    }

    /// Indice du barreau le plus haut que `disponible_bps` finance.
    ///
    /// Rend le dernier barreau quand rien ne le finance : l'échelle n'a pas
    /// de barreau en dessous. Constater l'insuffisance est le rôle du
    /// contrôleur, qui seul sait qu'on est au plancher.
    pub fn barreau_finance(&self, disponible_bps: u32) -> usize {
        self.barreaux
            .iter()
            .position(|b| disponible_bps >= b.min_bps)
            .unwrap_or(self.barreaux.len() - 1)
    }
}

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
const DELAI_AMORCAGE: Duration = Duration::from_secs(5);

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

/// Part de l'estimation qu'on s'autorise à consommer.
///
/// Les 10 % restants laissent la place aux retransmissions RTX et aux paquets
/// de sondage que le sous-système BWE émet pour tester à la hausse. Viser
/// 100 % de l'estimation, c'est garantir de la dépasser.
const MARGE: f32 = 0.9;

/// Écart relatif en dessous duquel on ne reconfigure pas le débit. Sans lui,
/// une estimation qui frémit ferait écrire l'encodeur à chaque seconde.
const ECART_MINIMAL_DEBIT: f32 = 0.10;

/// Plafond du pourcentage de perte déclaré à Opus. Au-delà, la redondance
/// LBRR coûte plus de débit qu'elle n'en sauve.
const PERTE_MAX_OPUS: i32 = 25;

/// Réglages figés d'une session.
#[derive(Debug, Clone, Copy)]
pub struct Config {
    /// Plafond de débit vidéo, en bits par seconde (variable `BITRATE`).
    /// Sert aussi de valeur de repli quand aucune estimation n'arrive.
    pub plafond_bps: u32,
    /// Budget réservé à la piste audio, retiré de l'estimation.
    pub audio_bps: u32,
    /// Taille de la source capturée, sommet de l'échelle.
    pub source: (u32, u32),
    pub fps: u32,
}

/// Ce que le transport observe, une fois par seconde.
#[derive(Debug, Clone, Copy)]
pub struct Observation {
    /// Estimation de bande passante sortante. `None` tant qu'aucune n'est
    /// arrivée — cas normal au démarrage, cas permanent si TWCC n'est pas
    /// négocié.
    pub estimate_bps: Option<u32>,
    /// Non consommé par la décision : journalisé par `transport.rs` pour que
    /// la recette dispose du RTT vu par l'agent, à confronter à celui que le
    /// navigateur rapporte.
    pub rtt: Option<Duration>,
    /// Fraction de paquets perdus, entre 0 et 1.
    pub loss: Option<f32>,
    pub at: Instant,
}

/// État du lien tel qu'on l'annonce à l'utilisateur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Qualite {
    /// Barreau le plus haut.
    Bonne,
    /// Résolution réduite : l'utilisateur doit savoir pourquoi l'image a molli.
    Degradee,
    /// Plancher atteint. On ne dégrade plus — on le dit.
    Insuffisante,
}

/// Le contrôleur reçoit-il de quoi s'asservir ?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Adaptation {
    Active,
    /// Aucune estimation n'est jamais arrivée. Le débit reste au plafond, et
    /// ce fait doit être annoncé — un silence ressemblerait à « tout va bien ».
    Indisponible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decision {
    pub video_bitrate_bps: u32,
    pub encode_size: (u32, u32),
    pub opus_loss_perc: i32,
    pub qualite: Qualite,
    pub adaptation: Adaptation,
}

pub struct Controleur {
    config: Config,
    echelle: Echelle,
    hysteresis: Hysteresis,
    courant: Decision,
    /// Instant de la toute première estimation reçue, `None` tant qu'aucune
    /// n'est arrivée.
    ///
    /// **Repurposé en revue finale de branche (I3+I2).** Ce champ existait
    /// déjà comme simple booléen (`estimation_vue`), écrit à la première
    /// estimation mais jamais relu — fossile d'une intention perdue, signalé
    /// par la revue. Il devient utile en portant l'INSTANT de cette première
    /// estimation plutôt qu'un simple drapeau : c'est ce qui permet de borner
    /// `DELAI_AMORCAGE` (voir sa doc), la fenêtre pendant laquelle la rampe du
    /// BWE ne doit pas être prise pour une dégradation.
    premiere_estimation_a: Option<Instant>,
}

impl Controleur {
    pub fn new(config: Config, now: Instant) -> Self {
        let echelle = Echelle::depuis(config.source, config.fps);
        let courant = Decision {
            video_bitrate_bps: config.plafond_bps,
            encode_size: echelle.barreaux()[0].taille,
            opus_loss_perc: 0,
            qualite: Qualite::Bonne,
            adaptation: Adaptation::Indisponible,
        };
        Self {
            config,
            echelle,
            hysteresis: Hysteresis::new(0, now),
            courant,
            premiere_estimation_a: None,
        }
    }

    /// Décision actuellement appliquée. Sert au démarrage, avant toute
    /// observation, et à alimenter le message d'état du lien.
    pub fn courant(&self) -> Decision {
        self.courant
    }

    /// Rebâtit l'échelle pour une nouvelle taille de source, en conservant le
    /// barreau courant.
    ///
    /// Appelée quand l'utilisateur redimensionne sa fenêtre : la source change,
    /// donc les seuils de l'échelle aussi. Sans cela, l'échelle resterait
    /// calibrée pour une source qui n'existe plus — et pourrait demander une
    /// taille d'encodage supérieure à la capture.
    ///
    /// Le barreau est conservé et non la taille absolue : c'est le NIVEAU de
    /// réduction qui a du sens, pas le nombre de pixels. Il est borné à la
    /// longueur de la nouvelle échelle, qui peut être plus courte (voir
    /// l'invariant d'`Echelle`).
    pub fn changer_source(&mut self, source: (u32, u32), now: Instant) -> Decision {
        // Indice du barreau actuellement appliqué, sur l'ANCIENNE échelle —
        // c'est la taille encodée en place qui porte cette information, il
        // n'existe pas de champ dédié. `unwrap_or(0)` : si `encode_size` ne
        // correspond à aucun barreau (ne devrait pas arriver), repartir du
        // sommet est le choix le plus sûr, jamais celui qui manquerait de
        // débit.
        let indice_avant = self
            .echelle
            .barreaux()
            .iter()
            .position(|b| b.taille == self.courant.encode_size)
            .unwrap_or(0);

        self.echelle = Echelle::depuis(source, self.config.fps);
        self.config.source = source;

        // Bornage : la nouvelle échelle peut compter moins de barreaux que
        // l'ancienne (source minuscule après un rétrécissement extrême, voir
        // l'invariant d'`Echelle`).
        let indice = indice_avant.min(self.echelle.barreaux().len() - 1);
        self.hysteresis = Hysteresis::new(indice, now);
        self.courant.encode_size = self.echelle.barreaux()[indice].taille;

        self.courant
    }

    pub fn observer(&mut self, o: Observation) -> Option<Decision> {
        let Some(estimate) = o.estimate_bps else {
            // Sans estimation, rien à asservir sur le débit ni la résolution.
            // L'INDISPONIBILITÉ, elle, doit être reflétée immédiatement :
            // sans cette ligne, `self.courant.adaptation` resterait figé à
            // `Active` après une première estimation suivie d'un silence
            // prolongé (TWCC qui se tarit, voir I4 côté transport), et
            // `courant()` mentirait sur l'état réel du lien à quiconque
            // l'interroge pendant ce silence — précisément le trou que la
            // revue finale de branche a nommé (I2). Seul ce champ bouge ici :
            // qualité, débit et taille restent ceux de la dernière décision
            // réelle, il n'y a rien de neuf à en tirer sans estimation.
            self.courant.adaptation = Adaptation::Indisponible;
            return None;
        };
        if self.premiere_estimation_a.is_none() {
            self.premiere_estimation_a = Some(o.at);
        }
        // Fenêtre d'amorçage : le sous-système BWE part bas et sonde à la
        // hausse (voir `DELAI_AMORCAGE`) — pendant cette rampe, un débit
        // disponible bas ne signifie pas un lien dégradé.
        let en_amorcage = o.at.duration_since(self.premiere_estimation_a.expect(
            "vient d'être posé si absent",
        )) < DELAI_AMORCAGE;

        // Part vidéo : marge de sécurité, moins le budget audio, borné au
        // plafond. `saturating_sub` : une estimation plus basse que le seul
        // budget audio ne doit pas déborder.
        let disponible = ((estimate as f32 * MARGE) as u32)
            .saturating_sub(self.config.audio_bps)
            .min(self.config.plafond_bps);

        let vise = self.echelle.barreau_finance(disponible);
        let taille_avant = self.courant.encode_size;
        let barreau_courant = self
            .echelle
            .barreaux()
            .iter()
            .position(|b| b.taille == taille_avant)
            .unwrap_or(0);
        // Pendant l'amorçage, ne jamais VISER un barreau pire que celui déjà
        // en place : on nourrit l'hystérésis avec le barreau courant plutôt
        // qu'avec la cible calculée, pour qu'aucune descente ne s'accumule
        // sur la rampe du BWE (voir `DELAI_AMORCAGE`). Une cible MEILLEURE
        // (remontée) reste autorisée sans restriction.
        let vise = if en_amorcage && vise > barreau_courant { barreau_courant } else { vise };
        if let Some(nouveau) = self.hysteresis.observer(vise, o.at) {
            self.courant.encode_size = self.echelle.barreaux()[nouveau].taille;
        }
        // Signal correct d'un changement de résolution, capturé avant/après
        // l'appel à l'hystérésis : depuis que `qualite` peut basculer dès que
        // le débit s'effondre (avant même que l'hystérésis n'ait bougé la
        // résolution), le changement de résolution qui arrive *plus tard* ne
        // fait souvent plus varier ni `qualite` ni `video_bitrate_bps` — sans
        // ce signal, ce changement de résolution ne remonterait jamais à
        // l'appelant. Ne pas confondre avec la clause plus bas comparant
        // `encode_size` au barreau appliqué : elle est toujours fausse par
        // construction (point relevé en revue, laissé pour la revue finale
        // de branche) — ce nouveau signal la complète sans la remplacer.
        let resolution_changee = self.courant.encode_size != taille_avant;

        let dernier = self.echelle.barreaux().len() - 1;
        let barreau_applique = self
            .echelle
            .barreaux()
            .iter()
            .position(|b| b.taille == self.courant.encode_size)
            .unwrap_or(0);

        // `video_bitrate_bps` bascule immédiatement (pas d'hystérésis dessus),
        // alors que `encode_size` ne bouge qu'après le délai de descente.
        // Comparer seulement au minimum du barreau *appliqué* laisserait donc
        // afficher `Bonne` pendant cette fenêtre, alors que le débit qui
        // arrive déjà ne finance plus la résolution encore en place. On
        // compare aussi `disponible` au minimum du barreau appliqué, pas
        // seulement à son indice.
        //
        // Pendant l'amorçage (`en_amorcage`), cette dernière comparaison est
        // désactivée : c'est elle qui, sur une source ≥1080p, faisait
        // afficher « Image réduite par le réseau » en bandeau persistant dès
        // la première observation de la rampe du BWE (I3, revue finale de
        // branche) — le débit disponible y est bas par construction, sans
        // que le lien le soit. Ce qui reste actif pendant l'amorçage : le
        // plancher (`Insuffisante`, ci-dessus) si le débit tombe RÉELLEMENT
        // sous le dernier barreau, et la dégradation par barreau déjà
        // appliqué (`barreau_applique > 0`) si une réduction a réellement eu
        // lieu avant l'amorçage.
        let qualite = if disponible < self.echelle.barreaux()[dernier].min_bps {
            Qualite::Insuffisante
        } else if barreau_applique > 0
            || (!en_amorcage && disponible < self.echelle.barreaux()[barreau_applique].min_bps)
        {
            Qualite::Degradee
        } else {
            Qualite::Bonne
        };

        let perte = o
            .loss
            .map(|l| ((l * 100.0).round() as i32).clamp(0, PERTE_MAX_OPUS))
            .unwrap_or(self.courant.opus_loss_perc);

        let debit_change = ecart_relatif(self.courant.video_bitrate_bps, disponible)
            >= ECART_MINIMAL_DEBIT;
        let change = debit_change
            || resolution_changee
            || qualite != self.courant.qualite
            || perte != self.courant.opus_loss_perc
            || self.courant.adaptation != Adaptation::Active
            || self.courant.encode_size != self.echelle.barreaux()[barreau_applique].taille;

        if debit_change {
            self.courant.video_bitrate_bps = disponible;
        }
        self.courant.qualite = qualite;
        self.courant.opus_loss_perc = perte;
        self.courant.adaptation = Adaptation::Active;

        change.then_some(self.courant)
    }
}

/// Écart relatif entre deux débits, rapporté au plus grand des deux pour
/// rester symétrique — sinon une division par un `avant` nul exploserait, et
/// une hausse de 1 à 2 ne pèserait pas comme une baisse de 2 à 1.
fn ecart_relatif(avant: u32, apres: u32) -> f32 {
    let max = avant.max(apres);
    if max == 0 {
        return 0.0;
    }
    (avant as f32 - apres as f32).abs() / max as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_echelle_a_quatre_barreaux_decroissants_et_pairs() {
        let echelle = Echelle::depuis((1920, 1080), 60);
        let tailles: Vec<(u32, u32)> = echelle.barreaux().iter().map(|b| b.taille).collect();

        // Pour 1920×1080, on attend exactement 4 barreaux (cas nominal).
        assert_eq!(tailles.len(), 4, "quatre barreaux attendus pour 1920×1080");
        assert_eq!(tailles[0], (1920, 1080), "le premier barreau est la taille source");
        for (i, (w, h)) in tailles.iter().enumerate() {
            assert_eq!(w % 2, 0, "barreau {i} : largeur impaire, refusée par H.264");
            assert_eq!(h % 2, 0, "barreau {i} : hauteur impaire, refusée par H.264");
        }
        for i in 1..tailles.len() {
            assert!(
                tailles[i].0 < tailles[i - 1].0,
                "barreau {i} pas plus petit que le précédent : {:?}",
                tailles
            );
        }
    }

    #[test]
    fn echelle_minuscule_sans_doublons() {
        // Sources où la troncature au pixel pair peut produire des doublons,
        // sans cette correction. Vérifie que l'échelle élimine les doublons et
        // reste strictement décroissante.

        // Source 8×8 : les diviseurs 1.5 et 2.0 retomberaient sur (4, 4).
        let echelle_8x8 = Echelle::depuis((8, 8), 60);
        let tailles_8x8: Vec<(u32, u32)> =
            echelle_8x8.barreaux().iter().map(|b| b.taille).collect();

        assert!(tailles_8x8.len() <= 4, "au plus 4 barreaux pour source 8×8");
        assert_eq!(
            tailles_8x8[0],
            (8, 8),
            "le premier barreau est la taille source (8×8)"
        );
        for (i, (w, h)) in tailles_8x8.iter().enumerate() {
            assert_eq!(w % 2, 0, "barreau {i} : largeur impaire");
            assert_eq!(h % 2, 0, "barreau {i} : hauteur impaire");
        }
        for i in 1..tailles_8x8.len() {
            assert!(
                tailles_8x8[i].0 < tailles_8x8[i - 1].0,
                "barreau {i} pas plus petit que le précédent : {:?}",
                tailles_8x8
            );
        }

        // Source 2×2 : les quatre diviseurs retomberaient tous sur (2, 2).
        // L'échelle ne doit avoir qu'un seul barreau, le plancher, pas quatre
        // doublons.
        let echelle_2x2 = Echelle::depuis((2, 2), 60);
        let tailles_2x2: Vec<(u32, u32)> =
            echelle_2x2.barreaux().iter().map(|b| b.taille).collect();

        assert_eq!(tailles_2x2.len(), 1, "source 2×2 : un seul barreau (plancher)");
        assert_eq!(tailles_2x2[0], (2, 2), "le barreau est le plancher (2, 2)");
    }

    #[test]
    fn le_barreau_finance_est_le_plus_haut_que_le_debit_paie() {
        let echelle = Echelle::depuis((1920, 1080), 60);
        let barreaux = echelle.barreaux();

        // Très large : le barreau 0.
        assert_eq!(echelle.barreau_finance(50_000_000), 0);

        // Juste au minimum du barreau 0 : encore le barreau 0.
        assert_eq!(echelle.barreau_finance(barreaux[0].min_bps), 0);

        // Un bit sous le minimum du barreau 0 : on descend d'un cran.
        assert_eq!(echelle.barreau_finance(barreaux[0].min_bps - 1), 1);

        // Sous le minimum du dernier barreau : on reste au dernier, c'est le
        // plancher. Déclarer l'insuffisance est le rôle du contrôleur
        // (tâche 5), pas celui de l'échelle.
        assert_eq!(echelle.barreau_finance(0), barreaux.len() - 1);
    }

    use std::time::{Duration, Instant};

    /// Instant de référence des tests. Placé loin dans le passé pour que
    /// toute soustraction de durée reste valide.
    fn t0() -> Instant {
        Instant::now() - Duration::from_secs(3600)
    }

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

    fn config() -> Config {
        Config {
            plafond_bps: 12_000_000,
            audio_bps: 128_000,
            source: (1920, 1080),
            fps: 60,
        }
    }

    fn obs(estimate_bps: Option<u32>, loss: Option<f32>, at: Instant) -> Observation {
        Observation { estimate_bps, rtt: None, loss, at }
    }

    #[test]
    fn sans_estimation_le_debit_reste_au_plafond_et_l_adaptation_est_indisponible() {
        let base = t0();
        let mut c = Controleur::new(config(), base);
        let d = c.courant();

        assert_eq!(d.video_bitrate_bps, 12_000_000);
        assert_eq!(d.encode_size, (1920, 1080));
        assert_eq!(d.adaptation, Adaptation::Indisponible);
        assert_eq!(d.qualite, Qualite::Bonne);

        // Cent observations sans estimation ne changent rien et ne
        // produisent aucune décision.
        for i in 0..100 {
            let at = base + Duration::from_millis(i * 100);
            assert_eq!(c.observer(obs(None, None, at)), None);
        }
        assert_eq!(c.courant().adaptation, Adaptation::Indisponible);
    }

    #[test]
    fn la_premiere_estimation_rend_l_adaptation_active() {
        let base = t0();
        let mut c = Controleur::new(config(), base);
        let d = c
            .observer(obs(Some(9_000_000), None, base + Duration::from_secs(1)))
            .expect("la première estimation doit produire une décision");

        assert_eq!(d.adaptation, Adaptation::Active);
        // 9 Mb/s × 0,9 − 128 kb/s d'audio = 7,972 Mb/s.
        assert_eq!(d.video_bitrate_bps, 7_972_000);
        // 7,97 Mb/s finance encore le barreau 0 (minimum 6,2 Mb/s).
        assert_eq!(d.encode_size, (1920, 1080));
        assert_eq!(d.qualite, Qualite::Bonne);
    }

    #[test]
    fn le_debit_ne_bouge_pas_pour_moins_de_dix_pour_cent_d_ecart() {
        let base = t0();
        let mut c = Controleur::new(config(), base);
        c.observer(obs(Some(9_000_000), None, base + Duration::from_secs(1)))
            .expect("première décision");

        // +5 % : sous le seuil, aucune décision.
        assert_eq!(
            c.observer(obs(Some(9_450_000), None, base + Duration::from_secs(3))),
            None
        );
        // +20 % : au-delà du seuil, décision produite.
        assert!(c
            .observer(obs(Some(10_800_000), None, base + Duration::from_secs(5)))
            .is_some());
    }

    #[test]
    fn le_debit_ne_depasse_jamais_le_plafond() {
        let base = t0();
        let mut c = Controleur::new(config(), base);
        let d = c
            .observer(obs(Some(80_000_000), None, base + Duration::from_secs(1)))
            .expect("décision");
        assert_eq!(d.video_bitrate_bps, 12_000_000, "le plafond BITRATE doit borner");
    }

    #[test]
    fn une_contrainte_durable_fait_descendre_un_barreau_et_marque_la_degradation() {
        let base = t0();
        let mut c = Controleur::new(config(), base);
        c.observer(obs(Some(9_000_000), None, base + Duration::from_secs(1)));

        // 5 Mb/s : sous le minimum du barreau 0 (6,2 Mb/s). Il faut 2 s — mais
        // la toute première estimation date de 1 s ci-dessus, donc les 5 s
        // suivantes (jusqu'à 6 s) tombent dans `DELAI_AMORCAGE` (I3) : le
        // contrôleur n'y vise jamais un barreau pire que le courant, la
        // descente ne peut donc commencer à s'accumuler qu'à partir de 6 s,
        // pour aboutir à 8 s. La borne du balayage est repoussée en
        // conséquence (40 -> 80, soit 15,8 s) pour laisser cette marge, sans
        // quoi ce test daterait d'avant l'amorçage et échouerait à tort.
        let mut derniere = None;
        for i in 10..80 {
            let at = base + Duration::from_millis(i * 200);
            if let Some(d) = c.observer(obs(Some(5_000_000), None, at)) {
                derniere = Some(d);
            }
        }
        let d = derniere.expect("une décision devait tomber");
        assert_ne!(d.encode_size, (1920, 1080), "la résolution devait descendre");
        assert_eq!(d.qualite, Qualite::Degradee);
    }

    #[test]
    fn sous_le_plancher_la_qualite_est_declaree_insuffisante() {
        let base = t0();
        let mut c = Controleur::new(config(), base);
        c.observer(obs(Some(9_000_000), None, base + Duration::from_secs(1)));

        let mut derniere = None;
        for i in 10..200 {
            let at = base + Duration::from_millis(i * 200);
            if let Some(d) = c.observer(obs(Some(300_000), None, at)) {
                derniere = Some(d);
            }
        }
        let d = derniere.expect("une décision devait tomber");
        assert_eq!(d.qualite, Qualite::Insuffisante);
        // On est descendu au dernier barreau, pas plus bas : la cadence n'est
        // jamais sacrifiée automatiquement.
        let echelle = Echelle::depuis((1920, 1080), 60);
        assert_eq!(d.encode_size, echelle.barreaux().last().unwrap().taille);
    }

    #[test]
    fn la_perte_est_convertie_en_pourcentage_et_plafonnee_a_vingt_cinq() {
        let base = t0();
        let mut c = Controleur::new(config(), base);

        let d = c
            .observer(obs(Some(9_000_000), Some(0.03), base + Duration::from_secs(1)))
            .expect("décision");
        assert_eq!(d.opus_loss_perc, 3);

        // 60 % de perte : plafonné à 25, au-delà duquel la redondance coûte
        // plus qu'elle ne sauve.
        let d = c
            .observer(obs(Some(9_000_000), Some(0.60), base + Duration::from_secs(3)))
            .expect("décision");
        assert_eq!(d.opus_loss_perc, 25);
    }

    #[test]
    fn la_qualite_ne_ment_pas_pendant_la_fenetre_d_hysteresis() {
        let base = t0();
        let mut c = Controleur::new(config(), base);
        // Amorce le contrôleur puis laisse s'écouler `DELAI_AMORCAGE` (5 s,
        // voir I3) avant les deux observations qui font l'objet de ce test :
        // il vérifie un mensonge possible en RÉGIME ÉTABLI, pas pendant la
        // rampe de démarrage du BWE, que l'amorçage protège maintenant
        // délibérément (autre test dédié à ce cas : voir
        // `l_amorcage_ne_declenche_pas_de_fausse_alerte`).
        c.observer(obs(Some(9_000_000), None, base + Duration::from_secs(1)));
        c.observer(obs(Some(9_000_000), None, base + Duration::from_secs(7)));

        // Effondrement à 5 Mb/s, observé une seule fois, moins de 2 s après
        // l'observation précédente : l'hystérésis de descente n'a pas eu le
        // temps de faire descendre la résolution, qui reste donc la taille
        // source. Le débit vidéo, lui, bascule immédiatement (aucune
        // hystérésis ne le protège).
        let d = c
            .observer(obs(Some(5_000_000), None, base + Duration::from_millis(7500)))
            .expect("le débit a assez bougé pour produire une décision");

        assert_eq!(
            d.encode_size,
            (1920, 1080),
            "l'hystérésis n'a pas eu le temps de faire descendre la résolution"
        );
        assert_ne!(
            d.qualite,
            Qualite::Bonne,
            "le débit ne finance plus la résolution encore appliquée : la qualité ne doit pas mentir"
        );
    }

    #[test]
    fn l_amorcage_ne_declenche_pas_de_fausse_alerte() {
        // I3 (revue finale de branche) : sur une source 1920×1080, le barreau
        // 0 exige 6,22 Mb/s, mais le BWE part volontairement bas
        // (`ESTIMATION_INITIALE_BPS` = 2,5 Mb/s côté transport) et sonde à la
        // hausse. Sans fenêtre d'amorçage, la toute première observation
        // annoncerait « Image réduite par le réseau » sur un lien par ailleurs
        // parfait.
        let base = t0();
        let mut c = Controleur::new(config(), base);

        // Première estimation, basse comme au vrai démarrage du BWE : 2,12 Mb/s
        // disponibles (2,5 M × 0,9 − 128 k), bien sous le minimum du barreau 0.
        let d = c
            .observer(obs(Some(2_500_000), None, base + Duration::from_secs(1)))
            .expect("le débit a assez bougé depuis le plafond pour produire une décision");

        assert_eq!(
            d.qualite,
            Qualite::Bonne,
            "la rampe de démarrage du BWE ne doit pas être prise pour une dégradation"
        );
        assert_eq!(
            d.encode_size,
            (1920, 1080),
            "aucune descente ne doit s'engager pendant l'amorçage"
        );

        // Une seconde estimation tout aussi basse, encore pendant la fenêtre
        // d'amorçage (moins de 5 s après la première) : toujours aucune
        // descente engagée, la qualité reste bonne.
        let d = c.observer(obs(Some(2_500_000), None, base + Duration::from_secs(3)));
        if let Some(d) = d {
            assert_eq!(d.qualite, Qualite::Bonne);
            assert_eq!(d.encode_size, (1920, 1080));
        }

        // Après la fenêtre d'amorçage (>= 5 s après la première estimation,
        // donc >= 6 s depuis `base`), une estimation toujours basse doit,
        // elle, produire la dégradation normale — l'amorçage ne doit protéger
        // que la rampe de démarrage, pas masquer un lien réellement mauvais.
        let mut derniere = None;
        for i in 30..80 {
            let at = base + Duration::from_millis(i * 200);
            if let Some(d) = c.observer(obs(Some(2_500_000), None, at)) {
                derniere = Some(d);
            }
        }
        let d = derniere.expect("une décision devait tomber une fois l'amorçage terminé");
        assert_eq!(
            d.qualite,
            Qualite::Degradee,
            "un débit durablement insuffisant hors amorçage doit dégrader normalement"
        );
        assert_ne!(d.encode_size, (1920, 1080), "la résolution devait finir par descendre");
    }

    #[test]
    fn changer_source_conserve_le_barreau_courant_sur_un_agrandissement() {
        let base = t0();
        let mut c = Controleur::new(config(), base);
        // Simule un barreau 2 déjà appliqué (comme après une dégradation
        // réseau), sans passer par le délai réel de l'hystérésis : ce test
        // porte sur `changer_source`, pas sur la façon d'atteindre ce
        // barreau.
        c.hysteresis = Hysteresis::new(2, base);
        c.courant.encode_size = c.echelle.barreaux()[2].taille;

        // Agrandissement de la source (1920×1080 -> 2560×1440).
        let nouvelle_source = (2560, 1440);
        let decision = c.changer_source(nouvelle_source, base + Duration::from_secs(1));

        let nouvelle_echelle = Echelle::depuis(nouvelle_source, config().fps);
        assert_eq!(
            decision.encode_size,
            nouvelle_echelle.barreaux()[2].taille,
            "le barreau 2 doit être conservé, à la taille de la NOUVELLE échelle"
        );
        assert_eq!(c.config.source, nouvelle_source, "la source mémorisée doit suivre");
    }

    #[test]
    fn changer_source_borne_le_barreau_quand_la_nouvelle_echelle_est_plus_courte() {
        let base = t0();
        let mut c = Controleur::new(config(), base);
        // Barreau 3 (le plus bas de l'échelle nominale 1920×1080) déjà
        // appliqué.
        c.hysteresis = Hysteresis::new(3, base);
        c.courant.encode_size = c.echelle.barreaux()[3].taille;
        assert_eq!(c.echelle.barreaux().len(), 4, "précondition : 4 barreaux sur la source nominale");

        // Rétrécissement vers une source minuscule dont l'échelle ne compte
        // qu'un seul barreau (voir `echelle_minuscule_sans_doublons`) :
        // l'indice 3 n'existe plus, il doit être borné à 0, le seul barreau
        // disponible — pas paniquer sur un accès hors bornes.
        let nouvelle_source = (2, 2);
        let decision = c.changer_source(nouvelle_source, base + Duration::from_secs(1));

        let nouvelle_echelle = Echelle::depuis(nouvelle_source, config().fps);
        assert_eq!(nouvelle_echelle.barreaux().len(), 1);
        assert_eq!(decision.encode_size, nouvelle_echelle.barreaux()[0].taille);
        assert_eq!(decision.encode_size, (2, 2));
    }

    #[test]
    fn changer_source_recalcule_les_seuils_min_bps_pour_la_nouvelle_taille() {
        let base = t0();
        let mut c = Controleur::new(config(), base);

        let nouvelle_source = (1280, 720);
        c.changer_source(nouvelle_source, base + Duration::from_secs(1));

        let echelle_attendue = Echelle::depuis(nouvelle_source, config().fps);
        let echelle_1080p = Echelle::depuis((1920, 1080), config().fps);
        // Précondition : les deux échelles ont bien des seuils différents,
        // sans quoi ce test ne prouverait rien.
        assert_ne!(echelle_attendue.barreaux()[0].min_bps, echelle_1080p.barreaux()[0].min_bps);

        assert_eq!(
            c.echelle.barreaux()[0].min_bps,
            echelle_attendue.barreaux()[0].min_bps,
            "les seuils min_bps doivent suivre la nouvelle taille de source, pas rester ceux de 1920×1080"
        );
    }
}
