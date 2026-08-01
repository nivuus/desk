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
pub const RELANCES_MAX: u32 = 3;

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
    audio: bool,
    /// Nombre de fois où cette fenêtre a déjà été relancée après la mort de
    /// son enfant. Le garde-fou de `relancer_les_orphelines` (`RELANCES_MAX`)
    /// s'appuie dessus pour abandonner plutôt que de relancer sans fin.
    relances: u32,
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
                audio,
                relances: 0,
            },
        );
        vec![Effet::AnnoncerOuverture { session, titre }]
    }

    pub fn viewport_recu(&mut self, session: &IdSession, largeur: u32, hauteur: u32) -> Vec<Effet> {
        // Un message du navigateur est une source externe : tardif, rejoué ou
        // inventé, il ne doit jamais faire avancer la machine deux fois.
        let Some(entree) = self.entrees.get_mut(session) else {
            return Vec::new();
        };
        if entree.etat != Etat::AttendLeViewport {
            return Vec::new();
        }
        entree.etat = Etat::AttendLaSortie;
        vec![Effet::CreerSortie {
            session: session.clone(),
            titre: entree.titre.clone(),
            largeur,
            hauteur,
        }]
    }

    /// `sortie_pilote` est ce que le pilote a rendu à la création (il ne sait
    /// détruire que par là) ; `nom_sortie` est le nom DXGI de la même sortie
    /// (l'enfant ne sait capturer que par là). Aucune relation calculable
    /// entre les deux : les deux sont retenus.
    pub fn sortie_creee(
        &mut self,
        session: &IdSession,
        sortie_pilote: u32,
        nom_sortie: String,
    ) -> Vec<Effet> {
        let Some(entree) = self.entrees.get_mut(session) else {
            return Vec::new();
        };
        if entree.etat != Etat::AttendLaSortie {
            return Vec::new();
        }
        entree.etat = Etat::Vivante;
        entree.sortie_pilote = Some(sortie_pilote);
        entree.nom_sortie = Some(nom_sortie.clone());
        vec![Effet::LancerEnfant {
            session: session.clone(),
            fenetre: entree.fenetre,
            nom_sortie,
            audio: entree.audio,
        }]
    }

    /// Nom de la sortie d'une session, pour le contrôle périodique de
    /// placement.
    pub fn nom_sortie_de(&self, session: &IdSession) -> Option<&str> {
        self.entrees.get(session).and_then(|e| e.nom_sortie.as_deref())
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
        let mut effets = Vec::new();
        // Effacés pour qu'une seconde mort ne redemande pas deux fois la
        // même destruction — la sortie ne sera rendue qu'ici, une fois.
        if let Some(sortie_pilote) = entree.sortie_pilote.take() {
            effets.push(Effet::DetruireSortie {
                sortie_pilote,
                // `nom_sortie` est toujours renseigné quand `sortie_pilote`
                // l'est : `sortie_creee` pose les deux ensemble, jamais l'un
                // sans l'autre. Le repli n'est donc pas atteignable.
                nom_sortie: entree.nom_sortie.take().unwrap_or_default(),
            });
        }
        effets.push(Effet::AnnoncerFermeture { session: session.clone() });
        effets
    }

    /// Repropose les fenêtres dont la session est morte mais qui existent
    /// toujours côté Windows. Appelée par le contrôle périodique du
    /// superviseur.
    ///
    /// L'entrée change d'identifiant de session à chaque relance — un
    /// identifiant réutilisé apparierait un message tardif du navigateur à
    /// la mauvaise fenêtre — donc chaque orpheline est retirée puis
    /// réinsérée sous une session neuve, `relances` et `audio` reportés.
    /// `self.compteur` continue de croître sans jamais reculer.
    pub fn relancer_les_orphelines(&mut self) -> Vec<Effet> {
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
                    sortie_pilote: None,
                    nom_sortie: None,
                    audio: entree.audio,
                    relances: entree.relances + 1,
                },
            );
            effets.push(Effet::AnnoncerOuverture { session, titre: entree.titre });
        }
        effets
    }
}

// Module de tests extrait dans un fichier voisin : la production seule
// approche déjà le plafond de 500 lignes du projet, et les tests en
// ajoutent régulièrement (le correctif d'idempotence ci-dessus en a ajouté
// deux). Extraire plutôt que compresser — la compression s'est déjà jouée
// ailleurs dans ce dépôt (`agent/src/encode/arret.rs`) et elle ne se joue
// qu'une fois.
#[cfg(test)]
#[path = "table/tests.rs"]
mod tests;
