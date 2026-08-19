//! Le chemin d'attribution d'une sortie virtuelle à une session : de la
//! demande de viewport à la sortie effectivement créée.
//!
//! Extrait de `table.rs` (tâche 4 du sous-bloc D3) pour rester sous le
//! plafond de 500 lignes du projet — pas pour une raison de conception : ces
//! deux méthodes restent des méthodes de `Table` comme les autres, dans le
//! même module logique, juste dans un fichier voisin.

use super::*;

impl Table {
    /// Décide, à réception du viewport annoncé par le navigateur, s'il faut
    /// créer une sortie ou réutiliser celle que la fenêtre a gardée de sa vie
    /// précédente.
    pub fn viewport_recu(&mut self, session: &IdSession, largeur: u32, hauteur: u32) -> Vec<Effet> {
        // Un message du navigateur est une source externe : tardif, rejoué ou
        // inventé, il ne doit jamais faire avancer la machine deux fois.
        let Some(entree) = self.entrees.get_mut(session) else {
            return Vec::new();
        };
        if entree.etat != Etat::AttendLeViewport {
            return Vec::new();
        }
        // Passé ce point, l'entrée n'attend plus le navigateur : le garde-fou
        // de staleness de `relancer_les_orphelines` ne la concerne plus.
        entree.attente_depuis = None;

        // Même bornage qu'à la création (`creation_sortie::creer_sortie`), et
        // pour la même raison : le viewport arrive en pixels périphériques
        // depuis D9, et sans ce bornage une sortie retenue serait jugée assez
        // grande — ou trop petite — contre une demande qui dépasse le plafond
        // que la création s'impose. Le poser ici plutôt que chez l'appelant
        // (`boucle.rs`) garde les deux chemins symétriques.
        let (largeur, hauteur) =
            crate::windows_source_sortie::borner_a_la_taille_max((largeur, hauteur));

        // **Chemin de réutilisation (§7.1 du sous-bloc D3).** Une fenêtre
        // relancée a gardé sa sortie ; si le viewport annoncé lui correspond,
        // il n'y a RIEN à créer — et c'est précisément la création qui fait
        // abandonner le mutex des duplications DXGI voisines.
        if let (Some(nom), Some(taille)) = (entree.nom_sortie.clone(), entree.taille_sortie) {
            // Même prédicat que l'appariement à la création
            // (`placement::sortie_pour_viewport`), et c'est le point : deux
            // règles distinctes feraient détruire à la relance une sortie que
            // la création venait d'accepter. La sortie retenue est réutilisée
            // dès qu'elle est ASSEZ GRANDE ; elle n'est rendue que si elle est
            // trop petite, seul cas où la recréer peut apporter des pixels.
            if crate::superviseur::placement::sortie_assez_grande(taille, (largeur, hauteur)) {
                entree.etat = Etat::Vivante;
                let retenue =
                    crate::superviseur::placement::taille_retenue((largeur, hauteur), taille);
                entree.taille_sortie = Some(retenue);
                return vec![Effet::LancerEnfant {
                    session: session.clone(),
                    fenetre: entree.fenetre,
                    nom_sortie: nom,
                    taille: retenue,
                }];
            }
        }

        entree.etat = Etat::AttendLaSortie;
        let mut effets = Vec::new();
        // La sortie retenue ne convient plus (le navigateur a retaillé sa
        // fenêtre entre-temps). La rendre AVANT d'en demander une autre :
        // laissée en place, elle resterait captive du vivier de dix, et
        // l'entrée n'en garderait plus l'identifiant.
        // Les trois champs se posent ensemble dans `sortie_creee` et se vident
        // ensemble ici : c'est l'invariant sur lequel repose tout le chemin de
        // réutilisation ci-dessus, qui exige `nom_sortie` ET `taille_sortie`
        // pour reconnaître une sortie retenue. Le `unwrap_or_default` n'est donc
        // pas atteignable ; s'il l'était, il produirait un `nom_sortie: ""`,
        // c'est-à-dire une destruction visant une sortie sans nom. Même repli et
        // même invariant que `rendre_la_sortie_de` (`table.rs`), qui le note.
        if let Some(sortie_pilote) = entree.sortie_pilote.take() {
            effets.push(Effet::DetruireSortie {
                sortie_pilote,
                nom_sortie: entree.nom_sortie.take().unwrap_or_default(),
            });
            entree.taille_sortie = None;
        }
        effets.push(Effet::CreerSortie {
            session: session.clone(),
            titre: entree.titre.clone(),
            largeur,
            hauteur,
        });
        effets
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
        taille: (u32, u32),
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
        entree.taille_sortie = Some(taille);
        vec![Effet::LancerEnfant {
            session: session.clone(),
            fenetre: entree.fenetre,
            nom_sortie,
            taille,
        }]
    }
}
