//! Où en est chaque fenêtre : détectée, en attente de son viewport, en attente
//! de sa sortie, vivante.
//!
//! Logique pure et sans effet de bord : la table ne crée rien, ne tue rien,
//! ne parle à personne. Elle rend une liste d'`Effet` que `superviseur.rs`
//! exécute. C'est ce qui la rend éprouvable sans Windows, sans pilote et sans
//! navigateur — et c'est là que vivent les règles qui, mal écrites, feraient
//! fuir une sortie virtuelle ou dédoubler le son.

use std::collections::HashMap;

/// Identifiant opaque d'une fenêtre Windows. C'est un `HWND` côté Windows,
/// mais ce module n'en sait rien et n'a pas à en savoir plus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IdFenetre(pub u64);

/// Identifiant de session, tel que le signaling et l'URL du navigateur le
/// portent. Opaque à dessein : ni le `HWND` ni le titre, qui changent tous
/// deux au cours de la vie d'une fenêtre.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdSession(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Etat {
    /// Annoncée à la page-shell ; on attend qu'elle dise la taille de sa
    /// fenêtre navigateur.
    AttendLeViewport,
    /// Le viewport est connu, la sortie virtuelle est demandée.
    AttendLaSortie,
    /// L'enfant tourne.
    Vivante,
    /// L'enfant est mort et la sortie a été rendue, mais **la fenêtre Windows
    /// est toujours là**. Le contrôle périodique la reproposera.
    ///
    /// Sans cet état, `enfant_mort` retirait purement l'entrée : plus rien ne
    /// rappelait la fenêtre sauf un `SHOW` fortuit de Windows, et la shell
    /// restait vide devant des applications bien vivantes (recette D1 §3.3).
    SansSession,
}

/// Relances tolérées pour une même fenêtre avant abandon.
///
/// Le garde-fou de l'emballement relevé en recette D1 : une fenêtre dont
/// l'enfant meurt systématiquement produirait sinon `w-5, w-6, w-7, w-8…`
/// jusqu'à épuiser le vivier de sorties du pilote.
///
/// **Ce compteur ne redescend jamais à zéro**, y compris quand une relance
/// atteint `Vivante` : il mesure les morts cumulées sur toute la vie de la
/// fenêtre, pas les échecs consécutifs. Une fenêtre qui vit dix minutes puis
/// meurt trois fois de suite plus tard est abandonnée à la troisième — pas
/// « trois échecs d'affilée » au sens strict. Choix conservateur, hérité tel
/// quel du brief de la tâche 10.
///
/// **Corollaire non corrigé, à documenter seulement** : un cycle `HIDE`/`SHOW`
/// (fenêtre réduite puis restaurée) fait quitter puis rejoindre la table par
/// `fenetre_disparue`/`fenetre_apparue`, donc repart à `relances = 0`. Le
/// garde-fou reste borné à chaque cycle pris isolément (jamais plus de 3
/// relances par cycle), donc aucune fuite — seulement une remise à zéro dont
/// il faut avoir conscience si l'on cherchait à borner le nombre total de
/// morts d'une fenêtre sur sa vie entière.
pub const RELANCES_MAX: u32 = 3;

/// Temps toléré, en attente d'un viewport, avant qu'une fenêtre relancée ne
/// soit abandonnée.
///
/// **Régression que ce délai corrige** : avant la tâche 10, un enfant mort
/// libérait sa place dans `entrees` immédiatement (`enfant_mort` retirait
/// l'entrée). Depuis, une fenêtre relancée reste `AttendLeViewport` — et si
/// la page-shell ne répond jamais (pop-up bloqué, shell déconnectée :
/// `CLAUDE.md` documente nommément ce cas pour la recette du sous-bloc D1),
/// plus rien ne fait progresser cette entrée : ni `SansSession` (elle ne l'est
/// plus), ni `Vivante` (elle ne l'atteindra jamais). Sans ce délai, sa place
/// serait perdue pour la durée de vie du superviseur.
///
/// **Majorante et non calibrée** — même aveu que `DUREE_FENETRE_REPRISE`
/// (`agent/src/capture/reprise.rs`) : aucune mesure n'a établi combien de
/// temps une page-shell peut légitimement mettre à répondre. Trente secondes
/// couvrent largement un rechargement de page ou une reconnexion réseau, sans
/// bloquer indéfiniment une place sur un vivier de huit.
///
/// **Portée volontairement limitée aux entrées RELANCÉES** (voir
/// `Entree::attente_depuis`) : une entrée issue de la détection initiale
/// (`fenetre_apparue`) porte le même risque en théorie, mais cette fonction
/// reste pure et ne reçoit aucun instant — l'étendre à elle changerait sa
/// signature et tous ses appelants, hors du périmètre de ce correctif.
pub const DELAI_ATTENTE_VIEWPORT_MAX: std::time::Duration = std::time::Duration::from_secs(30);

/// Ce que la table demande au monde extérieur de faire. Le superviseur les
/// exécute dans l'ordre rendu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effet {
    AnnoncerOuverture { session: IdSession, titre: String },
    /// `titre` accompagne la demande parce que le refus qui peut en découler
    /// s'affiche à un humain. Sans lui, l'appelant n'a que l'identifiant de
    /// session sous la main et la page-shell annonce « *« w-3 » n'a pas pu
    /// s'ouvrir* » — un message qui ne désigne rien pour l'utilisateur.
    CreerSortie { session: IdSession, titre: String, largeur: u32, hauteur: u32 },
    LancerEnfant {
        session: IdSession,
        fenetre: IdFenetre,
        /// Nom DXGI de la sortie (`\\.\DISPLAYn`), **et non un couple
        /// d'index** : ceux-ci sont positionnels, l'enfant les résout à son
        /// démarrage — donc plus tard — et une sortie apparue ou disparue
        /// entre-temps le fait capturer autre chose, ou échouer.
        nom_sortie: String,
        audio: bool,
    },
    TuerEnfant { session: IdSession },
    /// `sortie_pilote` est **l'identifiant du PILOTE**, pas le nom DXGI : le
    /// pilote ne sait retirer une sortie que par ce qu'il a lui-même rendu à
    /// la création ; lui présenter un nom DXGI ne détruirait rien, ou
    /// détruirait la sortie d'autrui. Les deux identifiants désignent la même
    /// sortie et n'ont aucune relation calculable — d'où les deux champs.
    ///
    /// `nom_sortie` accompagne la destruction parce que l'entrée a déjà
    /// quitté la table quand cet effet est rendu : sans lui, l'appelant ne
    /// pourrait plus savoir quelle place DXGI redevient libre.
    DetruireSortie { sortie_pilote: u32, nom_sortie: String },
    AnnoncerFermeture { session: IdSession },
    AnnoncerRefus { titre: String, motif: String },
}

#[derive(Debug)]
struct Entree {
    fenetre: IdFenetre,
    /// Retenu pour la seule raison qu'un refus de sortie doit se dire à un
    /// humain (voir `Effet::CreerSortie`). Il n'est PAS un identifiant : le
    /// titre d'une fenêtre change au cours de sa vie, c'est `IdSession` qui
    /// désigne.
    titre: String,
    etat: Etat,
    /// Identifiant rendu par le pilote à la création, pour la destruction.
    sortie_pilote: Option<u32>,
    /// Nom DXGI (`\\.\DISPLAYn`) de la même sortie, pour la capture et le
    /// placement. Stable, contrairement à une position d'énumération.
    nom_sortie: Option<String>,
    /// Dimensions RÉELLEMENT rendues par DXGI pour cette sortie — et non
    /// celles demandées. Le pilote quantifie (1280×632 demandé rend une sortie
    /// 1280×720, mesuré au sous-bloc D2) : comparer un viewport ultérieur à la
    /// demande jugerait réutilisable une sortie qui ne l'est pas.
    ///
    /// Posé et effacé en même temps que `sortie_pilote` et `nom_sortie` : les
    /// trois désignent la même sortie et ne se séparent jamais.
    taille_sortie: Option<(u32, u32)>,
    audio: bool,
    /// Nombre de fois où cette fenêtre a déjà été relancée après la mort de
    /// son enfant. Le garde-fou de `relancer_les_orphelines` (`RELANCES_MAX`)
    /// s'appuie dessus pour abandonner plutôt que de relancer sans fin.
    relances: u32,
    /// Instant où cette entrée est entrée en `AttendLeViewport` À LA SUITE
    /// D'UNE RELANCE — `None` pour une entrée issue de `fenetre_apparue`, qui
    /// reste une fonction pure sans horloge. C'est le garde-fou du second
    /// risque de capacité de la tâche 10 : sans lui, une fenêtre relancée
    /// dont la page-shell ne répond plus jamais resterait `AttendLeViewport`
    /// pour toujours, ni `SansSession` ni `Vivante`, place perdue jusqu'à
    /// l'arrêt du superviseur. Effacé dès que le viewport arrive
    /// (`viewport_recu`) : passé ce point, l'entrée n'attend plus le
    /// navigateur.
    attente_depuis: Option<std::time::Instant>,
}

pub struct Table {
    /// Nombre de fenêtres simultanées que la table s'autorise. Le vivier de
    /// sorties du pilote vaut 10 (mesuré), mais Apollo puise au même : la
    /// capacité est un paramètre, pas une constante.
    capacite: usize,
    entrees: HashMap<IdSession, Entree>,
    /// Compteur des sessions attribuées. Croît sans jamais reculer : un
    /// identifiant réutilisé apparierait un message tardif du navigateur à la
    /// mauvaise fenêtre.
    compteur: u64,
    /// Vrai tant qu'aucune fenêtre ne porte le son.
    ///
    /// **Défaut préexistant, hors périmètre de la tâche 10, à documenter
    /// seulement** : ce champ ne redevient jamais vrai une fois une porteuse
    /// désignée (`le_son_repasse_a_personne_tant_que_d2_ne_le_redesigne_pas`).
    /// Ce n'est pas une régression — l'ancien `enfant_mort` perdait déjà le
    /// son dès la première mort de la fenêtre porteuse — mais le chemin
    /// d'abandon de `relancer_les_orphelines` (au-delà de `RELANCES_MAX`) en
    /// est une occasion de plus, silencieuse comme les autres.
    audio_libre: bool,
}

impl Table {
    pub fn nouvelle(capacite: usize) -> Self {
        Self {
            capacite,
            entrees: HashMap::new(),
            compteur: 0,
            audio_libre: true,
        }
    }

    pub fn etat(&self, session: &IdSession) -> Option<&Etat> {
        self.entrees.get(session).map(|e| &e.etat)
    }

    /// Fenêtre Windows associée à une session.
    ///
    /// Le superviseur en a besoin pour le contrôle périodique de placement :
    /// il connaît la sortie par session, mais c'est la fenêtre qu'il faut
    /// replacer.
    pub fn fenetre_de(&self, session: &IdSession) -> Option<IdFenetre> {
        self.entrees.get(session).map(|e| e.fenetre)
    }

    pub fn fenetre_apparue(&mut self, fenetre: IdFenetre, titre: String) -> Vec<Effet> {
        // Idempotence par fenêtre, et cette garde passe AVANT le contrôle de
        // capacité (voir plus bas) : Windows peut annoncer deux fois le même
        // HWND — l'énumération de démarrage du superviseur et le hook
        // `EVENT_OBJECT_SHOW` se recouvrent sur une fenêtre qui apparaît
        // pendant l'énumération. Sans cette garde, une seconde entrée serait
        // créée pour la même fenêtre : deux places consommées, et si la
        // seconde atteint `sortie_creee`, deux sorties réelles ouvertes chez
        // le pilote. Or `fenetre_disparue` ne retrouve qu'UNE entrée par
        // recherche linéaire sur `fenetre`, et Windows n'émet qu'UN
        // événement de fermeture par HWND : la seconde entrée deviendrait
        // inatteignable, sa sortie ne serait jamais détruite, et le vivier
        // de sorties du pilote se viderait en silence — jusqu'à ce que plus
        // aucune fenêtre ne puisse s'ouvrir, des dizaines de minutes plus
        // tard, sans rapport visible avec la cause.
        //
        // Si la garde venait après le contrôle de capacité, une réannonce
        // sur une table pleine produirait à tort un `AnnoncerRefus` pour une
        // fenêtre... déjà ouverte.
        if self.entrees.values().any(|e| e.fenetre == fenetre) {
            return Vec::new();
        }
        if self.entrees.len() >= self.capacite {
            return vec![Effet::AnnoncerRefus {
                titre,
                motif: "plus aucune sortie virtuelle disponible".into(),
            }];
        }
        self.compteur += 1;
        let session = IdSession(format!("w-{}", self.compteur));
        // Le son est réservé ici, à la détection, et non au lancement : deux
        // fenêtres détectées coup sur coup ne doivent pas se le voir attribuer
        // toutes les deux parce que aucune n'a encore été lancée.
        let audio = self.audio_libre;
        self.audio_libre = false;
        self.entrees.insert(
            session.clone(),
            Entree {
                fenetre,
                titre: titre.clone(),
                etat: Etat::AttendLeViewport,
                sortie_pilote: None,
                nom_sortie: None,
                taille_sortie: None,
                audio,
                relances: 0,
                attente_depuis: None,
            },
        );
        vec![Effet::AnnoncerOuverture { session, titre }]
    }

    /// Nom de la sortie d'une session, pour le contrôle périodique de
    /// placement.
    pub fn nom_sortie_de(&self, session: &IdSession) -> Option<&str> {
        self.entrees.get(session).and_then(|e| e.nom_sortie.as_deref())
    }

    /// Dimensions de la sortie retenue par une session, s'il y en a une.
    pub fn taille_sortie_de(&self, session: &IdSession) -> Option<(u32, u32)> {
        self.entrees.get(session).and_then(|e| e.taille_sortie)
    }

    /// Sessions dont l'enfant tourne, pour le contrôle périodique de
    /// placement. Rendues par valeur : l'appelant mute la table pendant
    /// qu'il les parcourt.
    pub fn sessions_vivantes(&self) -> Vec<IdSession> {
        self.entrees
            .iter()
            .filter(|(_, e)| e.etat == Etat::Vivante)
            .map(|(s, _)| s.clone())
            .collect()
    }

    pub fn fenetre_disparue(&mut self, fenetre: IdFenetre) -> Vec<Effet> {
        let Some(session) = self
            .entrees
            .iter()
            .find(|(_, e)| e.fenetre == fenetre)
            .map(|(s, _)| s.clone())
        else {
            return Vec::new();
        };
        let entree = self.entrees.remove(&session).expect("trouvée à l'instant");
        let mut effets = vec![Effet::TuerEnfant { session: session.clone() }];
        // Rien à détruire si la fenêtre s'est fermée avant que sa sortie
        // n'existe : demander au pilote de retirer une sortie qu'il n'a
        // jamais créée ne ferait qu'une erreur de plus au journal.
        if let Some(sortie_pilote) = entree.sortie_pilote {
            effets.push(Effet::DetruireSortie {
                sortie_pilote,
                // `nom_sortie` est toujours renseigné quand `sortie_pilote`
                // l'est : `sortie_creee` pose les deux ensemble, jamais l'un
                // sans l'autre. Le repli n'est donc pas atteignable.
                nom_sortie: entree.nom_sortie.clone().unwrap_or_default(),
            });
        }
        effets.push(Effet::AnnoncerFermeture { session });
        effets
    }

    pub fn enfant_mort(&mut self, session: &IdSession) -> Vec<Effet> {
        // Pas de `TuerEnfant` : il est déjà mort. Mais sa sortie, elle, ne
        // s'est pas détruite toute seule — une sortie virtuelle survit au
        // processus qui l'a créée.
        //
        // L'entrée n'est PAS retirée : la fenêtre Windows, elle, est toujours
        // là (sauf coïncidence avec sa fermeture, traitée ailleurs par
        // `fenetre_disparue`). Elle bascule en `SansSession` pour que le
        // contrôle périodique la retrouve et la reproposera via
        // `relancer_les_orphelines` — sans quoi rien ne rappelait plus la
        // fenêtre, sauf un `SHOW` fortuit de Windows (recette D1 §3.3).
        let Some(entree) = self.entrees.get_mut(session) else {
            return Vec::new();
        };
        entree.etat = Etat::SansSession;
        // **La sortie est RETENUE, et c'est le correctif §7.1 du sous-bloc
        // D3.** Elle était jusqu'ici rendue au pilote ici même, et la relance
        // en recréait une — or c'est la CRÉATION d'une sortie qui fait
        // abandonner le mutex de toutes les duplications DXGI déjà ouvertes
        // (`0x887A0026`). Une seule fenêtre condamnée faisait ainsi passer le
        // compteur de réouvertures de 6 à 38 sur des sessions parfaitement
        // saines (recette D2, étape 4 du passage D).
        //
        // La contrepartie est que trois chemins, et non plus un, doivent
        // rendre la sortie : `fenetre_disparue`, et les deux abandons de
        // `relancer_les_orphelines`. Une sortie oubliée sur l'un d'eux
        // consommerait le vivier de dix jusqu'à l'arrêt du superviseur.
        vec![Effet::AnnoncerFermeture { session: session.clone() }]
    }

    /// Repropose les fenêtres dont la session est morte mais qui existent
    /// toujours côté Windows, et abandonne les entrées figées trop longtemps
    /// en attente d'un viewport. Appelée par le contrôle périodique du
    /// superviseur.
    ///
    /// **Ne lit aucune horloge** : `maintenant` est reçu en argument, sur le
    /// modèle de `FenetreDeReprise` (`agent/src/capture/reprise.rs`) — c'est
    /// ce qui garde cette table éprouvable sur l'hôte Linux.
    ///
    /// Deux garde-fous indépendants, tous deux nécessaires :
    /// - `RELANCES_MAX` borne le nombre de fois où une fenêtre dont l'ENFANT
    ///   meurt est relancée (l'entrée change d'identifiant de session à
    ///   chaque relance — un identifiant réutilisé apparierait un message
    ///   tardif du navigateur à la mauvaise fenêtre — donc chaque orpheline
    ///   est retirée puis réinsérée sous une session neuve, `relances` et
    ///   `audio` reportés ; `self.compteur` continue de croître sans jamais
    ///   reculer) ;
    /// - `DELAI_ATTENTE_VIEWPORT_MAX` borne le temps passé en `AttendLeViewport`
    ///   après une relance, si la PAGE-SHELL, elle, ne répond jamais.
    pub fn relancer_les_orphelines(&mut self, maintenant: std::time::Instant) -> Vec<Effet> {
        let orphelines: Vec<IdSession> = self
            .entrees
            .iter()
            .filter(|(_, e)| e.etat == Etat::SansSession)
            .map(|(s, _)| s.clone())
            .collect();
        let mut effets = Vec::new();
        for ancienne in orphelines {
            let entree = self.entrees.remove(&ancienne).expect("relevée à l'instant");
            if entree.relances >= RELANCES_MAX {
                effets.push(Effet::AnnoncerRefus {
                    titre: entree.titre,
                    motif: format!("la session n'a pas tenu après {RELANCES_MAX} tentatives"),
                });
                continue;
            }
            self.compteur += 1;
            let session = IdSession(format!("w-{}", self.compteur));
            self.entrees.insert(
                session.clone(),
                Entree {
                    fenetre: entree.fenetre,
                    titre: entree.titre.clone(),
                    etat: Etat::AttendLeViewport,
                    // Reportés, comme `audio` et `relances` : la sortie
                    // virtuelle survit à la relance (§7.1).
                    sortie_pilote: entree.sortie_pilote,
                    nom_sortie: entree.nom_sortie.clone(),
                    taille_sortie: entree.taille_sortie,
                    audio: entree.audio,
                    relances: entree.relances + 1,
                    attente_depuis: Some(maintenant),
                },
            );
            effets.push(Effet::AnnoncerOuverture { session, titre: entree.titre });
        }

        // Second garde-fou : une entrée relancée dont la page-shell ne
        // répond jamais reste `AttendLeViewport` — ni `SansSession` (elle ne
        // l'est plus), ni `Vivante` (elle ne l'atteindra jamais) — et ne
        // serait donc JAMAIS relevée par le filtre ci-dessus. `attente_depuis`
        // est `None` pour une entrée issue de `fenetre_apparue` : elle n'est
        // délibérément pas concernée (voir la doc de `DELAI_ATTENTE_VIEWPORT_MAX`).
        let figees: Vec<IdSession> = self
            .entrees
            .iter()
            .filter(|(_, e)| {
                e.etat == Etat::AttendLeViewport
                    && e.attente_depuis
                        .is_some_and(|depuis| maintenant.duration_since(depuis) > DELAI_ATTENTE_VIEWPORT_MAX)
            })
            .map(|(s, _)| s.clone())
            .collect();
        for figee in figees {
            let entree = self.entrees.remove(&figee).expect("relevée à l'instant");
            effets.push(Effet::AnnoncerRefus {
                titre: entree.titre,
                motif: "la page-shell n'a jamais répondu après la relance".into(),
            });
        }

        effets
    }
}

// Chemin d'attribution d'une sortie à une session (`viewport_recu`,
// `sortie_creee`) : extrait côté PRODUCTION, et non seulement les tests. Ce
// fichier frôlait déjà le plafond de 500 lignes du projet avant l'ajout du
// chemin de réutilisation de sortie du sous-bloc D3 (tâche 4) — l'ajouter ici
// l'aurait franchi. Extraire plutôt que compresser, même raison que les
// modules de tests ci-dessous.
mod attribution;

// Module de tests extrait dans un fichier voisin : la production seule
// approche déjà le plafond de 500 lignes du projet, et les tests en
// ajoutent régulièrement (le correctif d'idempotence ci-dessus en a ajouté
// deux). Extraire plutôt que compresser — la compression s'est déjà jouée
// ailleurs dans ce dépôt (`agent/src/encode/arret.rs`) et elle ne se joue
// qu'une fois.
#[cfg(test)]
#[path = "table/tests.rs"]
mod tests;

// Tests de la tâche 10 (relance après mort d'enfant), extraits dans un
// second fichier voisin : leur ajout dans `tests.rs` en aurait fait franchir
// le plafond de 500 lignes du projet. Voir la doc en tête de ce fichier.
#[cfg(test)]
#[path = "table/tests_relance.rs"]
mod tests_relance;

#[cfg(test)]
mod tests_retention;
