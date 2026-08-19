//! Le contrôle périodique des entrées qui n'avancent plus : celles dont
//! l'enfant est mort mais dont la fenêtre Windows vit toujours, et celles
//! que la page-shell a laissées en attente de leur viewport.
//!
//! Extrait de `table.rs` (tâche 17 du sous-bloc P3) pour rester sous le
//! plafond de 500 lignes du projet — pas pour une raison de conception,
//! exactement comme `attribution.rs` : cette méthode reste une méthode de
//! `Table` comme les autres, dans le même module logique, juste dans un
//! fichier voisin. **Extraction VERBATIM** : aucune ligne n'a été
//! reformulée, et surtout aucun commentaire n'a été comprimé — la
//! compression pour repasser sous la ligne s'est déjà jouée ailleurs dans ce
//! dépôt (`agent/src/encode/arret.rs`) et elle ne se joue qu'une fois.

use super::*;

impl Table {
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
    ///   est retirée puis réinsérée sous une session neuve, `relances`
    ///   reporté ; `self.compteur` continue de croître sans jamais reculer) ;
    /// - `DELAI_ATTENTE_VIEWPORT_MAX` borne le temps passé en `AttendLeViewport`,
    ///   relancée ou non depuis le sous-bloc D3, si la PAGE-SHELL, elle, ne
    ///   répond jamais.
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
                effets.extend(rendre_la_sortie_de(&entree));
                effets.push(Effet::AnnoncerRefus {
                    titre: entree.titre,
                    motif: format!("la session n'a pas tenu après {RELANCES_MAX} tentatives"),
                });
                continue;
            }
            let session = self.prochaine_session();
            self.entrees.insert(
                session.clone(),
                Entree {
                    fenetre: entree.fenetre,
                    titre: entree.titre.clone(),
                    etat: Etat::AttendLeViewport,
                    // Reporté, comme `relances` : la sortie virtuelle survit
                    // à la relance (§7.1).
                    sortie_pilote: entree.sortie_pilote,
                    nom_sortie: entree.nom_sortie.clone(),
                    taille_sortie: entree.taille_sortie,
                    relances: entree.relances + 1,
                    attente_depuis: Some(maintenant),
                },
            );
            effets.push(Effet::AnnoncerOuverture { session, titre: entree.titre });
        }

        // Tampon PARESSEUX (§7.3 du sous-bloc D2, corrigé en D3). Une entrée
        // issue de `fenetre_apparue` n'était pas tamponnée : cette fonction
        // est le seul endroit qui reçoive un instant, et `fenetre_apparue`
        // doit rester pure. On la tamponne donc ici, au premier passage.
        //
        // ⚠️ **Changement de comportement au démarrage** : les fenêtres de
        // l'énumération initiale cessent d'être exemptées. Si la page-shell se
        // connecte plus de `DELAI_ATTENTE_VIEWPORT_MAX` après le superviseur,
        // elles seront abandonnées — et une entrée abandonnée n'est JAMAIS
        // reproposée, le hook ne réémettant rien pour une fenêtre déjà
        // ouverte. Cohérent avec le piège de la recette D1 (« lancer le
        // navigateur AVANT le superviseur »), mais à connaître.
        //
        // Le délai court à partir de ce premier passage, cadencé par
        // `PERIODE_PLACEMENT` (1 s), et non depuis le démarrage.
        for entree in self.entrees.values_mut() {
            if entree.etat == Etat::AttendLeViewport && entree.attente_depuis.is_none() {
                entree.attente_depuis = Some(maintenant);
            }
        }

        // Second garde-fou : une entrée en `AttendLeViewport` dont la
        // page-shell ne répond jamais — ni `SansSession` (elle ne l'est
        // plus, ou ne l'a jamais été), ni `Vivante` (elle ne l'atteindra
        // jamais) — et ne serait donc JAMAIS relevée par le filtre
        // ci-dessus. Toute entrée en attente porte désormais un
        // `attente_depuis` depuis la boucle qui précède.
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
            effets.extend(rendre_la_sortie_de(&entree));
            effets.push(Effet::AnnoncerRefus {
                titre: entree.titre,
                motif: "la page-shell n'a jamais répondu après la relance".into(),
            });
        }

        effets
    }
}
