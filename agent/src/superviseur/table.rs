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
}

/// Ce que la table demande au monde extérieur de faire. Le superviseur les
/// exécute dans l'ordre rendu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effet {
    AnnoncerOuverture { session: IdSession, titre: String },
    CreerSortie { session: IdSession, largeur: u32, hauteur: u32 },
    LancerEnfant {
        session: IdSession,
        fenetre: IdFenetre,
        index_adaptateur: u32,
        index_sortie: u32,
        audio: bool,
    },
    TuerEnfant { session: IdSession },
    /// `sortie_pilote` est **l'identifiant du PILOTE**, pas l'index DXGI : le
    /// pilote ne sait retirer une sortie que par ce qu'il a lui-même rendu à
    /// la création ; lui présenter un index DXGI ne détruirait rien, ou
    /// détruirait la sortie d'autrui. Les deux identifiants désignent la même
    /// sortie et n'ont aucune relation calculable — d'où les deux champs.
    ///
    /// `dxgi` accompagne la destruction parce que l'entrée a déjà quitté la
    /// table quand cet effet est rendu : sans lui, l'appelant ne pourrait
    /// plus savoir quelle place DXGI redevient libre.
    DetruireSortie { sortie_pilote: u32, dxgi: (u32, u32) },
    AnnoncerFermeture { session: IdSession },
    AnnoncerRefus { titre: String, motif: String },
}

#[derive(Debug)]
struct Entree {
    fenetre: IdFenetre,
    etat: Etat,
    /// Identifiant rendu par le pilote à la création, pour la destruction.
    sortie_pilote: Option<u32>,
    /// Position de la même sortie dans l'énumération DXGI, pour la capture
    /// et le placement.
    dxgi: Option<(u32, u32)>,
    audio: bool,
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
                etat: Etat::AttendLeViewport,
                sortie_pilote: None,
                dxgi: None,
                audio,
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
        vec![Effet::CreerSortie { session: session.clone(), largeur, hauteur }]
    }

    /// `sortie_pilote` est ce que le pilote a rendu à la création (il ne sait
    /// détruire que par là) ; `dxgi` est la position de la même sortie dans
    /// l'énumération DXGI (l'enfant ne sait capturer que par là). Aucune
    /// relation calculable entre les deux : les deux sont retenus.
    pub fn sortie_creee(
        &mut self,
        session: &IdSession,
        sortie_pilote: u32,
        dxgi: (u32, u32),
    ) -> Vec<Effet> {
        let Some(entree) = self.entrees.get_mut(session) else {
            return Vec::new();
        };
        if entree.etat != Etat::AttendLaSortie {
            return Vec::new();
        }
        entree.etat = Etat::Vivante;
        entree.sortie_pilote = Some(sortie_pilote);
        entree.dxgi = Some(dxgi);
        vec![Effet::LancerEnfant {
            session: session.clone(),
            fenetre: entree.fenetre,
            index_adaptateur: dxgi.0,
            index_sortie: dxgi.1,
            audio: entree.audio,
        }]
    }

    /// Position DXGI de la sortie d'une session, pour le contrôle périodique
    /// de placement.
    pub fn sortie_dxgi_de(&self, session: &IdSession) -> Option<(u32, u32)> {
        self.entrees.get(session).and_then(|e| e.dxgi)
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
                // `dxgi` est toujours renseigne quand `sortie_pilote` l'est :
                // `sortie_creee` pose les deux ensemble, jamais l'un sans
                // l'autre. Le repli n'est donc pas atteignable.
                dxgi: entree.dxgi.unwrap_or((0, 0)),
            });
        }
        effets.push(Effet::AnnoncerFermeture { session });
        effets
    }

    pub fn enfant_mort(&mut self, session: &IdSession) -> Vec<Effet> {
        // Pas de `TuerEnfant` : il est déjà mort. Mais sa sortie, elle, ne
        // s'est pas détruite toute seule — une sortie virtuelle survit au
        // processus qui l'a créée.
        let Some(entree) = self.entrees.remove(session) else {
            return Vec::new();
        };
        let mut effets = Vec::new();
        if let Some(sortie_pilote) = entree.sortie_pilote {
            effets.push(Effet::DetruireSortie {
                sortie_pilote,
                // `dxgi` est toujours renseigne quand `sortie_pilote` l'est :
                // `sortie_creee` pose les deux ensemble, jamais l'un sans
                // l'autre. Le repli n'est donc pas atteignable.
                dxgi: entree.dxgi.unwrap_or((0, 0)),
            });
        }
        effets.push(Effet::AnnoncerFermeture { session: session.clone() });
        effets
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Table {
        Table::nouvelle(10)
    }

    /// Récupère l'identifiant de session attribué à la fenêtre, en lisant
    /// l'effet d'annonce — c'est la seule sortie publique qui le porte.
    fn session_annoncee(effets: &[Effet]) -> IdSession {
        effets
            .iter()
            .find_map(|e| match e {
                Effet::AnnoncerOuverture { session, .. } => Some(session.clone()),
                _ => None,
            })
            .expect("une ouverture doit être annoncée")
    }

    #[test]
    fn une_fenetre_qui_apparait_est_annoncee_et_rien_de_plus() {
        // Rien ne peut être créé avant de connaître le viewport : c'est lui
        // qui donne la taille de la sortie.
        let mut t = table();
        let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
        assert_eq!(effets.len(), 1);
        let session = session_annoncee(&effets);
        assert_eq!(t.etat(&session), Some(&Etat::AttendLeViewport));
    }

    #[test]
    fn le_viewport_declenche_la_creation_de_la_sortie() {
        let mut t = table();
        let session = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into()));
        let effets = t.viewport_recu(&session, 1600, 900);
        assert_eq!(
            effets,
            vec![Effet::CreerSortie { session: session.clone(), largeur: 1600, hauteur: 900 }]
        );
        assert_eq!(t.etat(&session), Some(&Etat::AttendLaSortie));
    }

    #[test]
    fn la_sortie_creee_declenche_le_lancement_de_l_enfant() {
        let mut t = table();
        let session = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into()));
        t.viewport_recu(&session, 1600, 900);
        // Deux identifiants distincts, et c'est le fond du sujet : `7` est
        // l'identifiant que le PILOTE a rendu, `(0, 4)` la position de la
        // même sortie dans l'énumération DXGI. Le pilote ne détruit que par
        // le premier ; l'enfant ne sait capturer que par le second.
        let effets = t.sortie_creee(&session, 7, (0, 4));
        assert_eq!(
            effets,
            vec![Effet::LancerEnfant {
                session: session.clone(),
                fenetre: IdFenetre(1),
                index_adaptateur: 0,
                index_sortie: 4,
                // La première fenêtre porte le son : le loopback WASAPI capte
                // toute la session Windows, deux porteurs feraient entendre
                // deux fois le même son.
                audio: true,
            }]
        );
        assert_eq!(t.etat(&session), Some(&Etat::Vivante));
        assert_eq!(t.sortie_dxgi_de(&session), Some((0, 4)));
    }

    #[test]
    fn seule_la_premiere_fenetre_porte_le_son() {
        let mut t = table();
        let a = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "A".into()));
        t.viewport_recu(&a, 1600, 900);
        t.sortie_creee(&a, 7, (0, 4));

        let b = session_annoncee(&t.fenetre_apparue(IdFenetre(2), "B".into()));
        t.viewport_recu(&b, 1280, 720);
        let effets = t.sortie_creee(&b, 8, (0, 5));
        assert_eq!(
            effets,
            vec![Effet::LancerEnfant {
                session: b,
                fenetre: IdFenetre(2),
                index_adaptateur: 0,
                index_sortie: 5,
                audio: false,
            }]
        );
    }

    #[test]
    fn une_fenetre_qui_disparait_tue_l_enfant_detruit_la_sortie_et_l_annonce() {
        let mut t = table();
        let session = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into()));
        t.viewport_recu(&session, 1600, 900);
        t.sortie_creee(&session, 7, (0, 4));

        let effets = t.fenetre_disparue(IdFenetre(1));
        assert_eq!(
            effets,
            vec![
                Effet::TuerEnfant { session: session.clone() },
                // L'identifiant du PILOTE, seul avec lequel il sait retirer.
                Effet::DetruireSortie { sortie_pilote: 7, dxgi: (0, 4) },
                Effet::AnnoncerFermeture { session: session.clone() },
            ]
        );
        assert_eq!(t.etat(&session), None, "la fenêtre doit avoir quitté la table");
    }

    #[test]
    fn un_enfant_qui_meurt_seul_libere_la_sortie_et_l_annonce_sans_le_tuer() {
        // C'est le bénéfice pour lequel le multi-processus a été choisi : la
        // mort d'un enfant ne doit rien emporter d'autre, mais elle ne doit
        // pas non plus laisser fuir sa sortie.
        let mut t = table();
        let session = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into()));
        t.viewport_recu(&session, 1600, 900);
        t.sortie_creee(&session, 7, (0, 4));

        let effets = t.enfant_mort(&session);
        assert_eq!(
            effets,
            vec![
                Effet::DetruireSortie { sortie_pilote: 7, dxgi: (0, 4) },
                Effet::AnnoncerFermeture { session: session.clone() },
            ]
        );
        assert_eq!(t.etat(&session), None);
    }

    #[test]
    fn une_fenetre_qui_disparait_avant_sa_sortie_ne_demande_aucune_destruction() {
        // Fermée pendant qu'on attendait son viewport : aucune sortie
        // n'existe, et demander d'en détruire une ferait échouer le pilote.
        let mut t = table();
        let session = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "A".into()));
        let effets = t.fenetre_disparue(IdFenetre(1));
        assert_eq!(
            effets,
            vec![
                Effet::TuerEnfant { session: session.clone() },
                Effet::AnnoncerFermeture { session },
            ]
        );
    }

    #[test]
    fn le_vivier_plein_refuse_la_fenetre_suivante_sans_rien_casser() {
        let mut t = Table::nouvelle(2);
        for n in 1..=2u64 {
            let s = session_annoncee(&t.fenetre_apparue(IdFenetre(n), format!("F{n}")));
            t.viewport_recu(&s, 1280, 720);
            t.sortie_creee(&s, n as u32, (0, n as u32));
        }
        let effets = t.fenetre_apparue(IdFenetre(3), "F3".into());
        assert_eq!(
            effets,
            vec![Effet::AnnoncerRefus {
                titre: "F3".into(),
                motif: "plus aucune sortie virtuelle disponible".into()
            }]
        );
    }

    #[test]
    fn une_sortie_liberee_rouvre_la_place() {
        let mut t = Table::nouvelle(1);
        let a = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "A".into()));
        t.viewport_recu(&a, 1280, 720);
        t.sortie_creee(&a, 7, (0, 4));
        assert!(matches!(
            t.fenetre_apparue(IdFenetre(2), "B".into()).as_slice(),
            [Effet::AnnoncerRefus { .. }]
        ));

        t.fenetre_disparue(IdFenetre(1));
        let effets = t.fenetre_apparue(IdFenetre(3), "C".into());
        assert!(matches!(effets.as_slice(), [Effet::AnnoncerOuverture { .. }]));
    }

    #[test]
    fn le_son_repasse_a_personne_tant_que_d2_ne_le_redesigne_pas() {
        // Limite assumée de D1, écrite en test pour qu'elle soit un choix
        // visible plutôt qu'un oubli : fermer la porteuse ne redésigne rien.
        let mut t = table();
        let a = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "A".into()));
        t.viewport_recu(&a, 1280, 720);
        t.sortie_creee(&a, 7, (0, 4));
        let b = session_annoncee(&t.fenetre_apparue(IdFenetre(2), "B".into()));
        t.viewport_recu(&b, 1280, 720);
        t.sortie_creee(&b, 8, (0, 5));

        t.fenetre_disparue(IdFenetre(1));
        let c = session_annoncee(&t.fenetre_apparue(IdFenetre(3), "C".into()));
        t.viewport_recu(&c, 1280, 720);
        let effets = t.sortie_creee(&c, 9, (0, 6));
        assert_eq!(
            effets,
            vec![Effet::LancerEnfant {
                session: c,
                fenetre: IdFenetre(3),
                index_adaptateur: 0,
                index_sortie: 6,
                audio: false,
            }],
            "aucune redésignation du son en D1"
        );
    }

    #[test]
    fn un_viewport_pour_une_session_inconnue_est_ignore() {
        // Le navigateur est une source externe : un message tardif ou rejoué
        // ne doit produire aucun effet.
        let mut t = table();
        let effets = t.viewport_recu(&IdSession("w-inconnue".into()), 800, 600);
        assert!(effets.is_empty());
    }

    #[test]
    fn un_second_viewport_pour_la_meme_session_est_ignore() {
        let mut t = table();
        let session = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "A".into()));
        t.viewport_recu(&session, 1600, 900);
        let effets = t.viewport_recu(&session, 800, 600);
        assert!(effets.is_empty(), "la sortie est déjà demandée à la première taille");
    }

    #[test]
    fn les_identifiants_de_session_sont_uniques() {
        let mut t = table();
        let a = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "A".into()));
        let b = session_annoncee(&t.fenetre_apparue(IdFenetre(2), "B".into()));
        assert_ne!(a, b);
    }

    #[test]
    fn la_fenetre_d_une_session_est_retrouvable() {
        // Le contrôle périodique de placement connaît la sortie par session
        // et doit remonter à la fenêtre pour la replacer.
        let mut t = table();
        let session = session_annoncee(&t.fenetre_apparue(IdFenetre(42), "A".into()));
        assert_eq!(t.fenetre_de(&session), Some(IdFenetre(42)));
        assert_eq!(t.fenetre_de(&IdSession("w-inconnue".into())), None);
    }
}
