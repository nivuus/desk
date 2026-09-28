//! The path assigning a virtual output to a session: from the
//! viewport request to the output actually created.
//!
//! Extracted from `table.rs` (task 4 of sub-block D3) to stay under the
//! project's 500-line ceiling — not for a design reason: these
//! two methods stay methods of `Table` like the others, in the
//! same logical module, just in a neighbouring file.

use super::*;

impl Table {
    /// Decides, on receiving the viewport announced by the browser, whether to
    /// create an output or reuse the one the window kept from its
    /// previous life.
    pub fn viewport_recu(&mut self, session: &IdSession, largeur: u32, hauteur: u32) -> Vec<Effet> {
        // A browser message is an external source: late, replayed or
        // invented, it must never advance the machine twice.
        let Some(entree) = self.entrees.get_mut(session) else {
            return Vec::new();
        };
        // 🔴 **AN ALREADY LIVE SESSION IS NO LONGER IGNORED, AND THAT IS BATCH
        // 33.** This `return Vec::new()` held for ANY state other
        // than `AttendLeViewport`, hence also for `Vivante` — so that a
        // viewport announced after opening did **nothing**, and the
        // window stayed served forever at the size of its opening
        // day. The browser, for its part, re-announced none (the
        // `postMessage` of `client/src/main.ts` only went out at load time):
        // the two halves of the defect covered each other, and neither
        // was visible from the other.
        //
        // The bounding below has not run yet: we redo it here rather
        // than move the line, so that both paths stay readable
        // separately.
        if entree.etat == Etat::Vivante && entree.nom_sortie.is_some() {
            let (largeur, hauteur) =
                crate::windows_source_sortie::clamp_to_max_size((largeur, hauteur));
            return vec![Effet::SuivreLeViewport {
                session: session.clone(),
                largeur,
                hauteur,
            }];
        }
        if entree.etat != Etat::AttendLeViewport {
            return Vec::new();
        }
        // Past this point, the entry no longer waits for the browser: the
        // staleness safeguard of `relancer_les_orphelines` no longer concerns it.
        entree.attente_depuis = None;

        // Same bounding as at creation (`creation_sortie::create_output`), and
        // for the same reason: the viewport arrives in device pixels
        // since D9, and without this bounding a retained output would be judged large
        // enough — or too small — against a request exceeding the ceiling
        // creation imposes on itself. Setting it here rather than at the caller
        // (`boucle.rs`) keeps both paths symmetric.
        let (largeur, hauteur) =
            crate::windows_source_sortie::clamp_to_max_size((largeur, hauteur));

        // **Reuse path (§7.1 of sub-block D3).** A restarted window
        // kept its output; if the announced viewport matches it,
        // there is NOTHING to create — and it is precisely creation that makes
        // the neighbouring DXGI duplications abandon the mutex.
        if let (Some(nom), Some(size)) = (entree.nom_sortie.clone(), entree.output_size) {
            // Same predicate as pairing at creation
            // (`placement::sortie_pour_viewport`), and that is the point: two
            // distinct rules would make the restart destroy an output
            // creation had just accepted. The retained output is reused
            // as soon as it is LARGE ENOUGH; it is only handed back if it is
            // too small, the only case where recreating it can bring pixels.
            if crate::superviseur::placement::sortie_assez_grande(size, (largeur, hauteur)) {
                entree.etat = Etat::Vivante;
                let retenue =
                    crate::superviseur::placement::retained_size((largeur, hauteur), size);
                entree.output_size = Some(retenue);
                return vec![Effet::LancerEnfant {
                    session: session.clone(),
                    fenetre: entree.fenetre,
                    nom_sortie: nom,
                    size: retenue,
                }];
            }
        }

        entree.etat = Etat::AttendLaSortie;
        let mut effets = Vec::new();
        // The retained output no longer fits (the browser resized its
        // window meanwhile). Hand it back BEFORE requesting another:
        // left in place, it would stay captive from the pool of ten, and
        // the entry would no longer keep its identifier.
        // The three fields are set together in `sortie_creee` and cleared
        // together here: it is the invariant on which the whole reuse path
        // above rests, which requires `nom_sortie` AND `output_size`
        // to recognise a retained output. The `unwrap_or_default` is therefore
        // not reachable; if it were, it would produce a `nom_sortie: ""`,
        // that is, a destruction targeting a nameless output. Same fallback and
        // same invariant as `rendre_la_sortie_de` (`table.rs`), which notes it.
        if let Some(sortie_pilote) = entree.sortie_pilote.take() {
            effets.push(Effet::DetruireSortie {
                sortie_pilote,
                nom_sortie: entree.nom_sortie.take().unwrap_or_default(),
            });
            entree.output_size = None;
        }
        effets.push(Effet::CreateOutput {
            session: session.clone(),
            titre: entree.titre.clone(),
            largeur,
            hauteur,
        });
        effets
    }

    /// `sortie_pilote` is what the driver returned at creation (it only knows how to
    /// destroy through that); `nom_sortie` is the DXGI name of the same output
    /// (the child only knows how to capture through that). No computable relation
    /// between the two: both are kept.
    pub fn sortie_creee(
        &mut self,
        session: &IdSession,
        sortie_pilote: u32,
        nom_sortie: String,
        size: (u32, u32),
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
        entree.output_size = Some(size);
        vec![Effet::LancerEnfant {
            session: session.clone(),
            fenetre: entree.fenetre,
            nom_sortie,
            size,
        }]
    }
}
