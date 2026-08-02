# Sous-bloc D3 — retenir les sorties, et caractériser le plafond de concurrence — Plan d'implémentation

> **Pour les agents d'exécution :** SOUS-SKILL REQUISE — utiliser
> `superpowers:subagent-driven-development` (recommandé) ou
> `superpowers:executing-plans` pour exécuter ce plan tâche par tâche. Les
> étapes sont en cases à cocher (`- [ ]`).

**Goal :** supprimer la recréation de sortie virtuelle qui inflige des abandons
de mutex aux sessions saines, fermer la fuite de capacité qui l'accompagne, puis
mener une campagne discriminante qui tranche sur quoi porte le plafond de quatre
duplications DXGI simultanées — et écrire la décision d'arrangement qui en
découle.

**Architecture :** deux volets indépendants. Le **volet 1** modifie la machine à
états pure `agent/src/superviseur/table.rs` (éprouvable sur l'hôte Linux) et son
exécutant `boucle.rs` (Windows). Le **volet 2** ajoute un mode de banc
`MULTIFENETRE_PLAFOND=<P>x<D>` : un processus **porteur** crée K = P×D sorties
virtuelles et bat le chien de garde du pilote, puis lance P processus **sondes
minimales** qui ouvrent chacune D duplications DXGI et les tiennent.

**Tech Stack :** Rust (agent Windows, `windows` crate, DXGI Desktop Duplication,
pilote SudoVDA par IOCTL), `tracing` pour les journaux, `cargo test` sur l'hôte
Linux pour les parties pures, `scripts/build-agent.sh` + `scripts/run-agent.sh`
pour l'exécution sur la VM.

**Spec :** `docs/superpowers/specs/2026-08-02-multifenetres-plafond-concurrence-design.md`

## Contraintes globales

- **Aucun fichier de code source ne dépasse 500 lignes.** `table.rs` est à
  **471** lignes, ses tests déjà extraits en deux fichiers voisins
  (`table/tests.rs`, `table/tests_relance.rs`). Toute addition substantielle
  s'accompagne d'une extraction, **jamais d'une compression** — la compression
  s'est déjà jouée sur ce terrain. Vérification :
  `{ git ls-files; git ls-files --others --exclude-standard; } | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'`
- **`Table` ne lit aucune horloge.** `maintenant: std::time::Instant` lui est
  passé en argument. C'est ce qui la rend éprouvable sans Windows. Ne pas
  introduire d'appel à `Instant::now()` dans `table.rs`.
- **Ne jamais tracer par duplication, par image ou par paquet.** Compter, et
  journaliser au rang. Le dépôt a payé deux fois pour une trace émise à la
  cadence d'une boucle.
- **Ne pas utiliser `git add -A`** : l'arbre est partagé entre tâches
  concurrentes. Nommer les fichiers dans chaque `git add`.
- **Toute variable d'environnement neuve du banc doit être ajoutée à
  `scripts/run-agent.sh` dans la MÊME tâche que le mode qu'elle active.** Sans
  cette ligne, l'agent démarre sans elle **et sans rien signaler** — piège payé
  deux fois (`SUPERVISEUR` en D1, `MULTIFENETRE_REPRISE` en D2).
- **Sourcer `.env` avant tout `scripts/build-agent.sh`** :
  `set -a && source .env && set +a`. Sans cela le script s'arrête **en silence**,
  et le symptôme se lit exactement comme une compilation réussie et muette.
- **Vérification locale** : `cargo test --workspace` et `cargo clippy --workspace`
  depuis `agent/`, ou `scripts/verify-all.sh`. Le code `#[cfg(windows)]` ne se
  compile que sur la VM.

---

## Carte des fichiers

| Fichier | Responsabilité | Volet |
| --- | --- | --- |
| `agent/src/superviseur/placement.rs` | Appariement et placement. **Modifié** : `TOLERANCE_PX` exposée par un prédicat public `taille_compatible` | 1 |
| `agent/src/superviseur/table.rs` | Machine à états pure. **Modifié** : rétention de la sortie, réutilisation, destruction sur les chemins d'abandon, tampon paresseux | 1 |
| `agent/src/superviseur/table/tests.rs` | Tests du cycle nominal. **Modifié** : appelants de `sortie_creee` | 1 |
| `agent/src/superviseur/table/tests_retention.rs` | **Créé** — tests de la rétention et de la réutilisation. `tests_relance.rs` ne peut pas les accueillir sans franchir le plafond | 1 |
| `agent/src/superviseur/table/tests_relance.rs` | Tests des garde-fous. **Modifié** : tampon paresseux | 1 |
| `agent/src/superviseur/boucle.rs` | Exécutant des effets. **Modifié** : replacement de la fenêtre sur le chemin `LancerEnfant` | 1 |
| `agent/src/diagnostics/multifenetre/plafond.rs` | **Créé** — le porteur : parsing, création des K sorties, lancement échelonné, restauration, verdict | 2 |
| `agent/src/diagnostics/multifenetre/plafond/sonde.rs` | **Créé** — la sonde minimale : D duplications, tenue, témoin de fichier | 2 |
| `agent/src/diagnostics/multifenetre.rs` | Aiguillage des sondes. **Modifié** : deux modes de plus | 2 |
| `scripts/run-agent.sh` | **Modifié** : transmission des variables neuves | 2 |
| `docs/superpowers/plans/2026-08-02-multifenetres-plafond-concurrence-resultats.md` | **Créé** — résultats et décision | 2 |

---

# VOLET 1 — les correctifs

## Task 1 : un prédicat de compatibilité de taille, partagé

**Files:**
- Modify: `agent/src/superviseur/placement.rs:36-67`
- Test: `agent/src/superviseur/placement.rs` (module `#[cfg(test)]` existant en
  fin de fichier ; s'il n'existe pas, le créer)

**Interfaces:**
- Consomme : rien.
- Produit : `pub fn taille_compatible(a: (u32, u32), b: (u32, u32)) -> bool`,
  utilisé par `table::viewport_recu` (Task 4) et par `sortie_par_dimensions`.

**Pourquoi cette tâche existe :** `TOLERANCE_PX` est privée et la logique
« proche à quatre pixels près » est écrite en dur dans `sortie_par_dimensions`.
La réutilisation d'une sortie retenue (Task 4) doit appliquer **exactement la
même** tolérance, sinon une sortie appariée à la création serait jugée
incompatible à la relance, et le correctif ne servirait à rien.

- [ ] **Step 1 : écrire le test qui échoue**

Dans `agent/src/superviseur/placement.rs`, à la fin du fichier :

```rust
#[cfg(test)]
mod tests_taille {
    use super::*;

    #[test]
    fn une_taille_identique_est_compatible() {
        assert!(taille_compatible((1280, 720), (1280, 720)));
    }

    /// La course de rattachement de la recette D1 : la sortie est créée à
    /// 1280×713 et DXGI la rend à 1280×720 un essai sur deux. Quatre pixels
    /// de tolérance ne couvrent PAS cet écart de sept — c'est
    /// `viewport_recu` qui doit alors détruire et recréer, pas apparier à
    /// tort.
    #[test]
    fn un_ecart_de_sept_pixels_n_est_pas_compatible() {
        assert!(!taille_compatible((1280, 713), (1280, 720)));
    }

    #[test]
    fn un_ecart_de_quatre_pixels_est_compatible() {
        assert!(taille_compatible((1276, 716), (1280, 720)));
    }

    /// Le facteur DPI de 1,5 que `CLAUDE.md` documente sur une sortie
    /// virtuelle doit rester refusé : l'accepter ferait poser la fenêtre sur
    /// une texture aux mauvaises dimensions.
    #[test]
    fn le_facteur_dpi_reste_refuse() {
        assert!(!taille_compatible((1280, 720), (1920, 1080)));
    }
}
```

- [ ] **Step 2 : exécuter le test pour vérifier qu'il échoue**

```bash
cd agent && cargo test --workspace tests_taille 2>&1 | tail -20
```

Attendu : ÉCHEC de compilation, `cannot find function 'taille_compatible' in this scope`.

- [ ] **Step 3 : écrire l'implémentation minimale**

Dans `agent/src/superviseur/placement.rs`, juste après la déclaration de
`TOLERANCE_PX` (l. 36) :

```rust
/// Vrai si deux tailles se correspondent à `TOLERANCE_PX` près.
///
/// **Le même prédicat que `sortie_par_dimensions`, et c'est le point.** Une
/// sortie appariée à la création doit être jugée réutilisable à la relance
/// (`table::viewport_recu`) : deux tolérances distinctes feraient détruire
/// puis recréer une sortie parfaitement bonne — exactement la recréation que
/// le sous-bloc D3 existe pour supprimer.
pub fn taille_compatible(a: (u32, u32), b: (u32, u32)) -> bool {
    let proche = |x: u32, y: u32| (x as i64 - y as i64).abs() <= TOLERANCE_PX;
    proche(a.0, b.0) && proche(a.1, b.1)
}
```

Puis remplacer le corps de `sortie_par_dimensions` (l. 57-66) pour qu'il
l'emploie, au lieu de sa fermeture locale `proche` :

```rust
    sorties
        .iter()
        .find(|s| {
            s.attachee_au_bureau
                && taille_compatible((s.rect.width, s.rect.height), (largeur, hauteur))
                && !deja_prises.contains(&s.nom_sortie)
        })
        .cloned()
```

- [ ] **Step 4 : exécuter les tests pour vérifier qu'ils passent**

```bash
cd agent && cargo test --workspace 2>&1 | tail -20
```

Attendu : SUCCÈS, y compris les tests existants de `sortie_par_dimensions` —
la refonte ne doit rien changer à son comportement.

- [ ] **Step 5 : commit**

```bash
git add agent/src/superviseur/placement.rs
git commit -m "refactor(d3): exposer la tolerance d'appariement en un predicat partage

La reutilisation d'une sortie retenue doit appliquer EXACTEMENT la meme
tolerance que l'appariement a la creation, sinon une sortie appariee a la
creation serait jugee incompatible a la relance.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Task 2 : la table retient la taille réelle de la sortie

**Files:**
- Modify: `agent/src/superviseur/table.rs` (struct `Entree`, `fenetre_apparue`,
  `sortie_creee`, `relancer_les_orphelines`)
- Modify: `agent/src/superviseur/table/tests.rs` (appelants de `sortie_creee`)
- Modify: `agent/src/superviseur/table/tests_relance.rs` (appelants)
- Modify: `agent/src/superviseur/boucle.rs:319` (appelant de production)
- Test: `agent/src/superviseur/table/tests_retention.rs` (**créé**)

**Interfaces:**
- Consomme : rien.
- Produit :
  - `Entree::taille_sortie: Option<(u32, u32)>` (champ privé) ;
  - signature modifiée
    `pub fn sortie_creee(&mut self, session: &IdSession, sortie_pilote: u32, nom_sortie: String, taille: (u32, u32)) -> Vec<Effet>` ;
  - `pub fn taille_sortie_de(&self, session: &IdSession) -> Option<(u32, u32)>`
    — lecteur employé par les tests des tâches suivantes.

**Pourquoi la taille RÉELLE et non la taille demandée :** ce qu'un viewport
ultérieur devra égaler, c'est ce que DXGI rend, pas ce que le navigateur a
demandé. La recette D1 a mesuré 1280×713 demandé rendu 1280×720 ; comparer à la
demande ferait juger réutilisable une sortie qui ne l'est pas, et
réciproquement.

- [ ] **Step 1 : écrire le test qui échoue**

Créer `agent/src/superviseur/table/tests_retention.rs` :

```rust
//! Tests du sous-bloc D3 : la sortie virtuelle est RETENUE entre la mort d'un
//! enfant et sa relance, au lieu d'être détruite puis recréée.
//!
//! Fichier distinct de `tests_relance.rs` : celui-ci est à 211 lignes et le
//! plafond du projet est à 500, mais la vraie raison est de lisibilité — ces
//! tests portent sur la rétention, ceux-là sur les garde-fous de capacité.

use super::*;

/// Ouvre une fenêtre et la mène jusqu'à `Vivante`, en rendant la session.
fn session_vivante(t: &mut Table, fenetre: u64, titre: &str, sortie: u32, nom: &str) -> IdSession {
    let effets = t.fenetre_apparue(IdFenetre(fenetre), titre.into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("ouverture attendue, reçu {effets:?}");
    };
    let session = session.clone();
    t.viewport_recu(&session, 1280, 720);
    t.sortie_creee(&session, sortie, nom.into(), (1280, 720));
    session
}

#[test]
fn la_table_retient_la_taille_reelle_de_la_sortie() {
    let mut t = Table::nouvelle(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("ouverture attendue, reçu {effets:?}");
    };
    let session = session.clone();
    t.viewport_recu(&session, 1280, 713);
    // Le pilote quantifie : demandé 1280×713, rendu 1280×720. C'est la taille
    // RENDUE qu'un viewport ultérieur devra égaler.
    t.sortie_creee(&session, 42, "\\\\.\\DISPLAY7".into(), (1280, 720));

    assert_eq!(t.taille_sortie_de(&session), Some((1280, 720)));
}

#[test]
fn une_session_sans_sortie_n_a_pas_de_taille() {
    let mut t = Table::nouvelle(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("ouverture attendue, reçu {effets:?}");
    };
    assert_eq!(t.taille_sortie_de(session), None);
}

#[test]
fn la_taille_survit_a_la_mort_de_l_enfant() {
    let mut t = Table::nouvelle(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");
    t.enfant_mort(&session);
    assert_eq!(
        t.taille_sortie_de(&session),
        Some((1280, 720)),
        "la taille accompagne la sortie retenue"
    );
}
```

Déclarer le module en fin de `agent/src/superviseur/table.rs`, à la suite des
deux déclarations existantes :

```rust
#[cfg(test)]
mod tests_retention;
```

- [ ] **Step 2 : exécuter le test pour vérifier qu'il échoue**

```bash
cd agent && cargo test --workspace tests_retention 2>&1 | tail -30
```

Attendu : ÉCHEC de compilation — `sortie_creee` prend 3 arguments et non 4,
`taille_sortie_de` n'existe pas.

- [ ] **Step 3 : écrire l'implémentation minimale**

Dans `agent/src/superviseur/table.rs`, ajouter le champ à `Entree`, après
`nom_sortie` :

```rust
    /// Dimensions RÉELLEMENT rendues par DXGI pour cette sortie — et non
    /// celles demandées. Le pilote quantifie (1280×632 demandé rend une sortie
    /// 1280×720, mesuré au sous-bloc D2) : comparer un viewport ultérieur à la
    /// demande jugerait réutilisable une sortie qui ne l'est pas.
    ///
    /// Posé et effacé en même temps que `sortie_pilote` et `nom_sortie` : les
    /// trois désignent la même sortie et ne se séparent jamais.
    taille_sortie: Option<(u32, u32)>,
```

L'initialiser à `None` dans les deux constructions d'`Entree`
(`fenetre_apparue` et `relancer_les_orphelines`).

Modifier `sortie_creee` :

```rust
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
            audio: entree.audio,
        }]
    }
```

Ajouter le lecteur, juste après `nom_sortie_de` :

```rust
    /// Dimensions de la sortie retenue par une session, s'il y en a une.
    pub fn taille_sortie_de(&self, session: &IdSession) -> Option<(u32, u32)> {
        self.entrees.get(session).and_then(|e| e.taille_sortie)
    }
```

Mettre à jour l'appelant de production, `agent/src/superviseur/boucle.rs:319` :

```rust
    let suite = table.sortie_creee(&session, id_pilote, nom, (cible.rect.width, cible.rect.height));
```

Mettre à jour tous les appels à `sortie_creee` dans `table/tests.rs` et
`table/tests_relance.rs` en ajoutant `(1280, 720)` comme quatrième argument.

```bash
grep -rn "sortie_creee(" agent/src/
```

- [ ] **Step 4 : exécuter les tests pour vérifier qu'ils passent**

```bash
cd agent && cargo test --workspace 2>&1 | tail -20
```

Attendu : SUCCÈS. `la_taille_survit_a_la_mort_de_l_enfant` **échouera encore**
si `enfant_mort` efface toujours les champs — c'est attendu, la Task 3 le
corrige. Si c'est le cas, marquer temporairement ce seul test `#[ignore]` avec
le commentaire `// levé par la Task 3 : enfant_mort retient la sortie`, et
l'activer en Task 3.

- [ ] **Step 5 : commit**

```bash
git add agent/src/superviseur/table.rs agent/src/superviseur/table/tests_retention.rs \
        agent/src/superviseur/table/tests.rs agent/src/superviseur/table/tests_relance.rs \
        agent/src/superviseur/boucle.rs
git commit -m "feat(d3): retenir la taille reellement rendue par DXGI

Ce qu'un viewport ulterieur devra egaler est ce que DXGI rend, pas ce que le
navigateur a demande : le pilote quantifie, et D2 a mesure 1280x632 demande
rendu 1280x720.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Task 3 : la sortie survit à la mort de l'enfant et à la relance

**Files:**
- Modify: `agent/src/superviseur/table.rs` (`enfant_mort`,
  `relancer_les_orphelines`)
- Test: `agent/src/superviseur/table/tests_retention.rs`
- Modify: `agent/src/superviseur/table/tests_relance.rs` (le test
  `une_fenetre_dont_l_enfant_meurt_est_reproposee` assertait l'inverse)

**Interfaces:**
- Consomme : `Entree::taille_sortie` (Task 2), `taille_sortie_de` (Task 2).
- Produit : `enfant_mort` n'émet plus `DetruireSortie` ; l'entrée réinsérée par
  `relancer_les_orphelines` porte `sortie_pilote`, `nom_sortie` et
  `taille_sortie` de l'entrée d'origine.

**C'est le cœur du correctif §7.1.** À l'étape 4 du passage D de la recette D2,
une seule fenêtre condamnée fait passer le compteur de réouvertures de mutex de
**6 à 38** — parce que chacune de ses quatre tentatives détruit puis recrée une
sortie virtuelle, et que c'est **la création** qui abandonne les mutex des
duplications voisines.

- [ ] **Step 1 : écrire le test qui échoue**

Ajouter à `agent/src/superviseur/table/tests_retention.rs` :

```rust
/// Le correctif §7.1 de D3. Avant lui, `enfant_mort` rendait la sortie au
/// pilote et la relance en recréait une — et c'est cette RECRÉATION qui
/// abandonne le mutex de toutes les duplications voisines (D2, 44 pertes
/// d'accès encaissées ; une seule fenêtre condamnée faisait passer le
/// compteur de réouvertures de 6 à 38).
#[test]
fn la_mort_de_l_enfant_ne_rend_plus_la_sortie_au_pilote() {
    let mut t = Table::nouvelle(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");

    let effets = t.enfant_mort(&session);

    assert!(
        !effets
            .iter()
            .any(|e| matches!(e, Effet::DetruireSortie { .. })),
        "la sortie est retenue pour la relance, reçu {effets:?}"
    );
    assert!(effets.contains(&Effet::AnnoncerFermeture { session: session.clone() }));
}

#[test]
fn l_entree_relancee_porte_encore_sa_sortie() {
    let mut t = Table::nouvelle(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");
    t.enfant_mort(&session);

    let effets = t.relancer_les_orphelines(std::time::Instant::now());
    let Some(Effet::AnnoncerOuverture { session: neuve, .. }) = effets.first() else {
        panic!("réouverture attendue, reçu {effets:?}");
    };
    let neuve = neuve.clone();

    assert_ne!(neuve, session, "un identifiant réutilisé apparierait un message tardif");
    assert_eq!(
        t.nom_sortie_de(&neuve),
        Some("\\\\.\\DISPLAY7"),
        "la sortie suit la fenêtre dans sa nouvelle session"
    );
    assert_eq!(t.taille_sortie_de(&neuve), Some((1280, 720)));
}
```

Retirer l'`#[ignore]` temporaire posé en Task 2 sur
`la_taille_survit_a_la_mort_de_l_enfant`, s'il y en avait un.

- [ ] **Step 2 : exécuter les tests pour vérifier qu'ils échouent**

```bash
cd agent && cargo test --workspace tests_retention 2>&1 | tail -30
```

Attendu : ÉCHEC — `la_mort_de_l_enfant_ne_rend_plus_la_sortie_au_pilote` trouve
un `DetruireSortie`, `l_entree_relancee_porte_encore_sa_sortie` lit `None`.

- [ ] **Step 3 : écrire l'implémentation minimale**

Dans `enfant_mort`, remplacer **tout ce qui suit l'affectation
`entree.etat = Etat::SansSession;`** — c'est-à-dire le `let mut effets`, le bloc
`if let Some(sortie_pilote) = entree.sortie_pilote.take()`, le `push` final et
le `effets` de retour (actuellement l. 358 à 371) — par :

```rust
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
```

Le `let Some(entree) = … else { return Vec::new(); }` et
`entree.etat = Etat::SansSession;` restent inchangés au-dessus.

Dans `relancer_les_orphelines`, reporter les trois champs dans l'entrée
réinsérée :

```rust
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
```

Corriger l'assertion devenue fausse dans
`table/tests_relance.rs::une_fenetre_dont_l_enfant_meurt_est_reproposee` :
remplacer le bloc `assert!(effets.contains(&Effet::DetruireSortie { .. }))` par

```rust
    assert!(
        !effets.iter().any(|e| matches!(e, Effet::DetruireSortie { .. })),
        "depuis D3 §7.1 la sortie est retenue pour la relance, reçu {effets:?}"
    );
```

- [ ] **Step 4 : exécuter les tests pour vérifier qu'ils passent**

```bash
cd agent && cargo test --workspace 2>&1 | tail -20
```

Attendu : SUCCÈS sur l'ensemble du workspace.

- [ ] **Step 5 : commit**

```bash
git add agent/src/superviseur/table.rs agent/src/superviseur/table/tests_retention.rs \
        agent/src/superviseur/table/tests_relance.rs
git commit -m "fix(d3): retenir la sortie virtuelle entre la mort d'un enfant et sa relance

C'est la CREATION d'une sortie qui fait abandonner le mutex des duplications
DXGI voisines. La detruire a chaque mort pour la recreer a chaque relance
infligeait ces abandons a des sessions saines : une seule fenetre condamnee
faisait passer le compteur de reouvertures de 6 a 38 (recette D2).

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Task 4 : réutiliser la sortie retenue, ou la remplacer

**Files:**
- Modify: `agent/src/superviseur/table.rs` (`viewport_recu`)
- Test: `agent/src/superviseur/table/tests_retention.rs`

**Interfaces:**
- Consomme : `placement::taille_compatible` (Task 1), `Entree::taille_sortie`
  (Task 2), champs reportés (Task 3).
- Produit : `viewport_recu` rend `[LancerEnfant]` sur le chemin de réutilisation
  (l'entrée passe directement à `Etat::Vivante`), ou
  `[DetruireSortie, CreerSortie]` quand la sortie retenue ne convient pas, ou
  `[CreerSortie]` quand il n'y en a aucune.

**Décision de conception, à ne pas déplacer :** le choix vit dans la table pure
et non dans `boucle.rs`. `boucle.rs` est `#[cfg(windows)]` et n'est éprouvé par
aucun test ; y mettre la branche rendrait le cœur du correctif invérifiable sur
l'hôte. La table sait déjà tout ce qu'il faut pour trancher.

- [ ] **Step 1 : écrire les tests qui échouent**

Ajouter à `agent/src/superviseur/table/tests_retention.rs` :

```rust
#[test]
fn sans_sortie_retenue_le_viewport_en_demande_une() {
    let mut t = Table::nouvelle(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("ouverture attendue, reçu {effets:?}");
    };
    let session = session.clone();

    let effets = t.viewport_recu(&session, 1280, 720);

    assert_eq!(
        effets,
        vec![Effet::CreerSortie {
            session: session.clone(),
            titre: "Bloc-notes".into(),
            largeur: 1280,
            hauteur: 720
        }]
    );
}

/// **Le chemin qui supprime les réouvertures parasites.** La fenêtre garde sa
/// sortie, le viewport annoncé lui correspond : plus rien à créer, donc plus
/// aucun mutex abandonné chez les voisines.
#[test]
fn une_sortie_retenue_compatible_est_reutilisee_sans_rien_creer() {
    let mut t = Table::nouvelle(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");
    t.enfant_mort(&session);
    let effets = t.relancer_les_orphelines(std::time::Instant::now());
    let Some(Effet::AnnoncerOuverture { session: neuve, .. }) = effets.first() else {
        panic!("réouverture attendue, reçu {effets:?}");
    };
    let neuve = neuve.clone();

    let effets = t.viewport_recu(&neuve, 1280, 720);

    assert_eq!(
        effets,
        vec![Effet::LancerEnfant {
            session: neuve.clone(),
            fenetre: IdFenetre(1),
            nom_sortie: "\\\\.\\DISPLAY7".into(),
            audio: true
        }],
        "ni DetruireSortie ni CreerSortie : c'est tout l'objet du correctif"
    );
    assert_eq!(t.etat(&neuve), Some(&Etat::Vivante));
}

/// La tolérance est celle de l'appariement — quatre pixels — et pas davantage.
#[test]
fn une_sortie_retenue_a_quatre_pixels_pres_est_reutilisee() {
    let mut t = Table::nouvelle(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");
    t.enfant_mort(&session);
    let effets = t.relancer_les_orphelines(std::time::Instant::now());
    let Some(Effet::AnnoncerOuverture { session: neuve, .. }) = effets.first() else {
        panic!("réouverture attendue, reçu {effets:?}");
    };
    let neuve = neuve.clone();

    let effets = t.viewport_recu(&neuve, 1278, 718);

    assert!(matches!(effets.first(), Some(Effet::LancerEnfant { .. })), "reçu {effets:?}");
}

/// Le navigateur a redimensionné sa fenêtre entre-temps : la sortie retenue
/// ne convient plus, il faut la rendre AVANT d'en demander une autre — sans
/// quoi elle resterait captive du vivier de dix.
#[test]
fn une_sortie_retenue_incompatible_est_rendue_puis_remplacee() {
    let mut t = Table::nouvelle(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");
    t.enfant_mort(&session);
    let effets = t.relancer_les_orphelines(std::time::Instant::now());
    let Some(Effet::AnnoncerOuverture { session: neuve, .. }) = effets.first() else {
        panic!("réouverture attendue, reçu {effets:?}");
    };
    let neuve = neuve.clone();

    let effets = t.viewport_recu(&neuve, 1920, 1080);

    assert_eq!(
        effets,
        vec![
            Effet::DetruireSortie {
                sortie_pilote: 42,
                nom_sortie: "\\\\.\\DISPLAY7".into()
            },
            Effet::CreerSortie {
                session: neuve.clone(),
                titre: "Bloc-notes".into(),
                largeur: 1920,
                hauteur: 1080
            },
        ],
        "la destruction précède la demande, et dans cet ordre"
    );
    assert_eq!(t.nom_sortie_de(&neuve), None, "l'entrée ne retient plus rien");
    assert_eq!(t.etat(&neuve), Some(&Etat::AttendLaSortie));
}

/// Un message du navigateur est une source externe : rejoué, il ne doit pas
/// relancer un second enfant sur la même sortie.
#[test]
fn un_viewport_rejoue_apres_reutilisation_ne_fait_rien() {
    let mut t = Table::nouvelle(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");
    t.enfant_mort(&session);
    let effets = t.relancer_les_orphelines(std::time::Instant::now());
    let Some(Effet::AnnoncerOuverture { session: neuve, .. }) = effets.first() else {
        panic!("réouverture attendue, reçu {effets:?}");
    };
    let neuve = neuve.clone();
    t.viewport_recu(&neuve, 1280, 720);

    assert!(t.viewport_recu(&neuve, 1280, 720).is_empty());
}
```

- [ ] **Step 2 : exécuter les tests pour vérifier qu'ils échouent**

```bash
cd agent && cargo test --workspace tests_retention 2>&1 | tail -40
```

Attendu : ÉCHEC — `viewport_recu` rend toujours un `CreerSortie` unique.

- [ ] **Step 3 : écrire l'implémentation minimale**

Remplacer le corps de `viewport_recu` dans `agent/src/superviseur/table.rs` :

```rust
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

        // **Chemin de réutilisation (§7.1 du sous-bloc D3).** Une fenêtre
        // relancée a gardé sa sortie ; si le viewport annoncé lui correspond,
        // il n'y a RIEN à créer — et c'est précisément la création qui fait
        // abandonner le mutex des duplications DXGI voisines.
        if let (Some(nom), Some(taille)) = (entree.nom_sortie.clone(), entree.taille_sortie) {
            if crate::superviseur::placement::taille_compatible(taille, (largeur, hauteur)) {
                entree.etat = Etat::Vivante;
                return vec![Effet::LancerEnfant {
                    session: session.clone(),
                    fenetre: entree.fenetre,
                    nom_sortie: nom,
                    audio: entree.audio,
                }];
            }
        }

        entree.etat = Etat::AttendLaSortie;
        let mut effets = Vec::new();
        // La sortie retenue ne convient plus (le navigateur a retaillé sa
        // fenêtre entre-temps). La rendre AVANT d'en demander une autre :
        // laissée en place, elle resterait captive du vivier de dix, et
        // l'entrée n'en garderait plus l'identifiant.
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
```

- [ ] **Step 4 : exécuter les tests pour vérifier qu'ils passent**

```bash
cd agent && cargo test --workspace 2>&1 | tail -20 && cargo clippy --workspace 2>&1 | tail -10
```

Attendu : SUCCÈS, et clippy sans avertissement.

- [ ] **Step 5 : contrôler la taille du fichier**

```bash
wc -l agent/src/superviseur/table.rs
```

Si le fichier dépasse 500 lignes, **extraire** — ne pas compresser. La coupe
naturelle : `viewport_recu` et `sortie_creee` forment le chemin d'attribution
d'une sortie et peuvent partir dans `table/attribution.rs`.

- [ ] **Step 6 : commit**

```bash
git add agent/src/superviseur/table.rs agent/src/superviseur/table/tests_retention.rs
git commit -m "feat(d3): reutiliser la sortie retenue quand le viewport lui correspond

Le chemin qui supprime les reouvertures parasites : une fenetre relancee dont
le viewport n'a pas change ne fait plus rien creer, donc n'inflige plus aucun
abandon de mutex a ses voisines. Sortie incompatible : rendue AVANT d'en
demander une autre, sinon elle resterait captive du vivier de dix.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Task 5 : les deux chemins d'abandon rendent la sortie

**Files:**
- Modify: `agent/src/superviseur/table.rs` (`relancer_les_orphelines`, les deux
  branches d'abandon)
- Test: `agent/src/superviseur/table/tests_retention.rs`

**Interfaces:**
- Consomme : les champs reportés (Task 3).
- Produit : chaque branche d'abandon émet `DetruireSortie` **avant**
  `AnnoncerRefus` quand l'entrée retirée porte une sortie.

**C'est la contrepartie du §7.1, et le risque principal du volet 1.** Avant D3,
une entrée abandonnée ne portait jamais de sortie — `enfant_mort` l'avait
toujours rendue. Ce n'est plus vrai. Une sortie oubliée ici consommerait le
vivier de dix jusqu'à l'arrêt du superviseur, sans qu'aucune trace ne le dise.

- [ ] **Step 1 : écrire les tests qui échouent**

Ajouter à `agent/src/superviseur/table/tests_retention.rs` :

```rust
/// Contrepartie du §7.1 : une entrée abandonnée porte désormais une sortie,
/// ce qui n'arrivait jamais avant D3. L'oublier viderait le vivier de dix du
/// pilote, silencieusement, jusqu'à l'arrêt du superviseur.
#[test]
fn l_abandon_apres_relances_max_rend_la_sortie() {
    let base = std::time::Instant::now();
    let mut t = Table::nouvelle(4);
    let mut session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");

    // RELANCES_MAX relances, puis l'abandon au tour suivant.
    for tour in 0..=RELANCES_MAX {
        t.enfant_mort(&session);
        let effets = t.relancer_les_orphelines(base + std::time::Duration::from_secs(tour as u64));
        if let Some(Effet::AnnoncerOuverture { session: neuve, .. }) = effets.first() {
            session = neuve.clone();
            // La sortie suit ; on ne la recrée pas.
            t.viewport_recu(&session, 1280, 720);
            continue;
        }
        // Tour d'abandon.
        assert!(
            effets.contains(&Effet::DetruireSortie {
                sortie_pilote: 42,
                nom_sortie: "\\\\.\\DISPLAY7".into()
            }),
            "la sortie retenue doit être rendue à l'abandon, reçu {effets:?}"
        );
        assert!(
            effets
                .iter()
                .any(|e| matches!(e, Effet::AnnoncerRefus { .. })),
            "reçu {effets:?}"
        );
        return;
    }
    panic!("l'abandon n'est jamais survenu");
}

/// Second chemin d'abandon : la page-shell ne répond jamais après la relance.
/// L'entrée porte encore sa sortie retenue — même exigence.
#[test]
fn l_abandon_d_une_entree_figee_rend_la_sortie() {
    let base = std::time::Instant::now();
    let mut t = Table::nouvelle(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");
    t.enfant_mort(&session);
    // Relance : l'entrée repasse en AttendLeViewport, tamponnée à `base`.
    t.relancer_les_orphelines(base);

    // La page-shell ne répond jamais : au-delà du délai, abandon.
    let effets = t.relancer_les_orphelines(base + DELAI_ATTENTE_VIEWPORT_MAX + std::time::Duration::from_secs(1));

    assert!(
        effets.contains(&Effet::DetruireSortie {
            sortie_pilote: 42,
            nom_sortie: "\\\\.\\DISPLAY7".into()
        }),
        "la sortie retenue doit être rendue, reçu {effets:?}"
    );
}
```

- [ ] **Step 2 : exécuter les tests pour vérifier qu'ils échouent**

```bash
cd agent && cargo test --workspace tests_retention 2>&1 | tail -30
```

Attendu : ÉCHEC — aucun `DetruireSortie` n'est émis sur ces deux chemins.

- [ ] **Step 3 : écrire l'implémentation minimale**

Dans `relancer_les_orphelines`, factoriser le rendu de sortie et l'appliquer aux
deux branches. Ajouter, juste avant la fonction :

```rust
/// Effet de destruction d'une sortie retenue par une entrée qu'on retire.
///
/// **Trois chemins retirent une entrée de la table, et depuis le §7.1 du
/// sous-bloc D3 les trois peuvent en porter une** : `fenetre_disparue`, et les
/// deux abandons de `relancer_les_orphelines`. Avant D3, `enfant_mort` avait
/// toujours rendu la sortie et le cas n'existait pas. Une sortie oubliée ici
/// consommerait le vivier de dix du pilote jusqu'à l'arrêt du superviseur,
/// sans qu'aucune trace ne le dise.
fn rendre_la_sortie_de(entree: &Entree) -> Option<Effet> {
    entree.sortie_pilote.map(|sortie_pilote| Effet::DetruireSortie {
        sortie_pilote,
        nom_sortie: entree.nom_sortie.clone().unwrap_or_default(),
    })
}
```

Dans la branche `if entree.relances >= RELANCES_MAX` :

```rust
            if entree.relances >= RELANCES_MAX {
                effets.extend(rendre_la_sortie_de(&entree));
                effets.push(Effet::AnnoncerRefus {
                    titre: entree.titre,
                    motif: format!("la session n'a pas tenu après {RELANCES_MAX} tentatives"),
                });
                continue;
            }
```

Dans la boucle des entrées figées :

```rust
        for figee in figees {
            let entree = self.entrees.remove(&figee).expect("relevée à l'instant");
            effets.extend(rendre_la_sortie_de(&entree));
            effets.push(Effet::AnnoncerRefus {
                titre: entree.titre,
                motif: "la page-shell n'a jamais répondu après la relance".into(),
            });
        }
```

- [ ] **Step 4 : exécuter les tests pour vérifier qu'ils passent**

```bash
cd agent && cargo test --workspace 2>&1 | tail -20
```

Attendu : SUCCÈS.

- [ ] **Step 5 : commit**

```bash
git add agent/src/superviseur/table.rs agent/src/superviseur/table/tests_retention.rs
git commit -m "fix(d3): rendre la sortie retenue sur les deux chemins d'abandon

Contrepartie de la retention : une entree abandonnee porte desormais une
sortie, ce qui n'arrivait jamais avant. L'oublier viderait le vivier de dix
du pilote en silence, jusqu'a l'arret du superviseur.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Task 6 : fermer la fuite de capacité (§7.3)

**Files:**
- Modify: `agent/src/superviseur/table.rs` (`relancer_les_orphelines`,
  documentation de `DELAI_ATTENTE_VIEWPORT_MAX` et d'`Entree::attente_depuis`)
- Test: `agent/src/superviseur/table/tests_relance.rs`

**Interfaces:**
- Consomme : `Entree::attente_depuis`.
- Produit : toute entrée en `AttendLeViewport` non tamponnée est tamponnée au
  premier passage de `relancer_les_orphelines`, y compris celles issues de
  `fenetre_apparue`.

**Effet de bord à assumer, et à documenter dans le code :** les fenêtres issues
de l'énumération initiale cessent d'être exemptées. **Si la page-shell se
connecte plus de 30 s après le superviseur, elles seront abandonnées** — et une
entrée abandonnée n'est jamais reproposée, le hook ne réémettant rien pour une
fenêtre déjà ouverte. C'est cohérent avec le piège documenté en D1 (« lancer le
navigateur AVANT le superviseur »), mais c'est un changement de comportement au
démarrage.

- [ ] **Step 1 : écrire les tests qui échouent**

Ajouter à `agent/src/superviseur/table/tests_relance.rs` :

```rust
/// §7.3 du sous-bloc D2, corrigé en D3. Une fenêtre NEUVE dont la page-shell
/// ne répond jamais restait `AttendLeViewport` sans être ni relancée ni
/// abandonnée : ni `SansSession`, ni `Vivante`. Sa place était perdue jusqu'à
/// l'arrêt du superviseur.
#[test]
fn une_fenetre_neuve_dont_la_shell_ne_repond_jamais_finit_par_etre_abandonnee() {
    let base = std::time::Instant::now();
    let mut t = Table::nouvelle(4);
    t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());

    // Premier passage : le tampon est posé, rien n'est abandonné.
    assert!(t.relancer_les_orphelines(base).is_empty());

    // Le délai court à partir du premier passage, pas du démarrage.
    let effets = t.relancer_les_orphelines(instant(base, 30_001));

    assert!(
        effets.iter().any(|e| matches!(e, Effet::AnnoncerRefus { .. })),
        "la place doit être libérée, reçu {effets:?}"
    );
    assert_eq!(t.fenetre_apparue(IdFenetre(2), "Autre".into()).len(), 1);
}

/// Le tampon ne doit pas abandonner une fenêtre qui répond dans le délai.
#[test]
fn une_fenetre_neuve_qui_repond_dans_le_delai_n_est_pas_abandonnee() {
    let base = std::time::Instant::now();
    let mut t = Table::nouvelle(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("ouverture attendue, reçu {effets:?}");
    };
    let session = session.clone();

    t.relancer_les_orphelines(base);
    t.viewport_recu(&session, 1280, 720);

    let effets = t.relancer_les_orphelines(instant(base, 30_001));
    assert!(effets.is_empty(), "reçu {effets:?}");
}
```

- [ ] **Step 2 : exécuter les tests pour vérifier qu'ils échouent**

```bash
cd agent && cargo test --workspace tests_relance 2>&1 | tail -30
```

Attendu : ÉCHEC du premier test — l'entrée issue de `fenetre_apparue` a
`attente_depuis: None` et n'est jamais relevée.

- [ ] **Step 3 : écrire l'implémentation minimale**

Dans `relancer_les_orphelines`, **avant** le filtre `figees`, ajouter le tampon
paresseux :

```rust
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
```

Mettre à jour la documentation de `DELAI_ATTENTE_VIEWPORT_MAX` (l. 81-85) : le
paragraphe « Portée volontairement limitée aux entrées RELANCÉES » n'est plus
vrai. Le remplacer par :

```rust
/// **Portée : TOUTES les entrées en attente de viewport**, depuis le sous-bloc
/// D3. Elle était limitée aux entrées relancées, parce que `fenetre_apparue`
/// est pure et ne reçoit aucun instant ; le tampon est désormais posé
/// paresseusement par `relancer_les_orphelines`, qui en reçoit un. Conséquence
/// à connaître : une fenêtre préexistante au démarrage est abandonnée si la
/// page-shell ne s'est pas connectée dans ce délai.
```

Et celle d'`Entree::attente_depuis` : remplacer « `None` pour une entrée issue
de `fenetre_apparue` » par « posé paresseusement par `relancer_les_orphelines`
pour une entrée issue de `fenetre_apparue`, qui reste pure et sans horloge ».

- [ ] **Step 4 : exécuter les tests pour vérifier qu'ils passent**

```bash
cd agent && cargo test --workspace 2>&1 | tail -20 && cargo clippy --workspace 2>&1 | tail -10
```

Attendu : SUCCÈS. Vérifier que le test existant de D2 sur la portée limitée aux
entrées relancées, s'il existe, a été mis à jour et non supprimé :

```bash
grep -rn "attente_depuis\|DELAI_ATTENTE_VIEWPORT" agent/src/superviseur/table/
```

- [ ] **Step 5 : commit**

```bash
git add agent/src/superviseur/table.rs agent/src/superviseur/table/tests_relance.rs
git commit -m "fix(d3): fermer la fuite de capacite d'une fenetre neuve sans reponse

Tampon paresseux au premier passage de relancer_les_orphelines : Table reste
pure et aucun appelant ne bouge. Effet de bord assume et documente — les
fenetres preexistantes au demarrage cessent d'etre exemptees du delai.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Task 7 : replacer la fenêtre sur le chemin de réutilisation

**Files:**
- Modify: `agent/src/superviseur/boucle.rs` (bras `Effet::LancerEnfant`,
  extraction depuis `controler_le_placement`)

**Interfaces:**
- Consomme : `Effet::LancerEnfant` (inchangé), `table.nom_sortie_de`,
  `table.fenetre_de`.
- Produit : `fn replacer_si_besoin(table: &Table, session: &IdSession, toutes: &[SortieDxgi])`,
  appelée par `controler_le_placement` **et** par le bras `LancerEnfant`.

**Pourquoi :** sur le chemin de réutilisation (Task 4), `creer_sortie` n'est pas
appelée — donc `placement::poser` non plus. Entre la mort de l'enfant et sa
relance, l'application a pu déplacer ou retailler sa fenêtre. Le contrôle
périodique la rattraperait avec jusqu'à une seconde de retard ; l'enfant
capturerait d'ici là une fenêtre mal posée.

**Code Windows uniquement : aucun test automatisé.** La vérification est la
compilation sur la VM (Tasks 11-12) et la relecture.

- [ ] **Step 1 : extraire le corps de `controler_le_placement`**

Dans `agent/src/superviseur/boucle.rs`, remplacer `controler_le_placement`
(l. 456-483) par :

```rust
/// Remet sur sa sortie toute fenêtre qui en est partie.
fn controler_le_placement(table: &Table) {
    let toutes = enumerer_sorties_silencieux().unwrap_or_default();
    for session in table.sessions_vivantes() {
        replacer_si_besoin(table, &session, &toutes);
    }
}

/// Remet une fenêtre sur sa sortie si elle en est partie.
///
/// Appelée par le contrôle périodique, **et par le bras `LancerEnfant`** : sur
/// le chemin de réutilisation d'une sortie retenue (§7.1 du sous-bloc D3),
/// `creer_sortie` n'est pas appelée, donc `placement::poser` non plus. Entre
/// la mort de l'enfant et sa relance, l'application a pu déplacer ou retailler
/// sa fenêtre ; sans cet appel, l'enfant capturerait une fenêtre mal posée
/// jusqu'au prochain contrôle périodique — jusqu'à `PERIODE_PLACEMENT` plus
/// tard.
///
/// Idempotente : `doit_etre_replacee` garde l'appel, donc le chemin de
/// création — où la fenêtre vient d'être posée — n'émet aucun second
/// `SetWindowPos`.
fn replacer_si_besoin(table: &Table, session: &IdSession, toutes: &[SortieDxgi]) {
    let Some(nom) = table.nom_sortie_de(session) else {
        return;
    };
    let Some(cible) = toutes.iter().find(|s| s.nom_sortie == nom) else {
        return;
    };
    let Some(fenetre) = table.fenetre_de(session) else { return };
    let hwnd = windows::Win32::Foundation::HWND(fenetre.0 as *mut core::ffi::c_void);
    let Ok(actuel) = placement::rectangle_de(hwnd) else { return };
    if placement::doit_etre_replacee(&actuel, &cible.rect) {
        tracing::info!(
            session = %session.0,
            de = format!("{}x{}+{}+{}", actuel.width, actuel.height, actuel.x, actuel.y),
            vers = format!(
                "{}x{}+{}+{}",
                cible.rect.width, cible.rect.height, cible.rect.x, cible.rect.y
            ),
            "fenêtre sortie de sa sortie, replacement"
        );
        if let Err(erreur) = placement::poser(hwnd, &cible.rect) {
            tracing::warn!(session = %session.0, %erreur, "replacement échoué");
        }
    }
}
```

- [ ] **Step 2 : appeler depuis le bras `LancerEnfant`**

Dans la boucle, remplacer le début du bras `Effet::LancerEnfant` (l. 118) par :

```rust
                Effet::LancerEnfant { session, fenetre, nom_sortie, audio } => {
                    // Le chemin de réutilisation d'une sortie retenue ne passe
                    // pas par `creer_sortie`, donc la fenêtre n'a pas été
                    // reposée. Une seule énumération, sur ce seul bras.
                    let toutes = enumerer_sorties_silencieux().unwrap_or_default();
                    replacer_si_besoin(&table, &session, &toutes);
                    if let Err(erreur) = enfants.lancer(Consigne {
```

⚠️ **Le reste du bras est inchangé.** Ne pas toucher au traitement de l'erreur
de lancement — c'est le seul chemin qui rende la sortie quand `Lanceur` refuse.

- [ ] **Step 3 : vérifier que le fichier compile sur l'hôte, dans la mesure du possible**

`boucle.rs` est `#[cfg(windows)]` : `cargo check` sur Linux ne le compile pas.
Contrôle minimal disponible :

```bash
cd agent && cargo clippy --workspace 2>&1 | tail -10
```

Attendu : aucun avertissement neuf. La compilation réelle a lieu sur la VM (Tasks 11-12).

- [ ] **Step 4 : contrôler la taille du fichier**

```bash
wc -l agent/src/superviseur/boucle.rs
```

Si le fichier dépasse 500 lignes, extraire — le couple
`controler_le_placement` / `replacer_si_besoin` forme une unité qui peut partir
dans `superviseur/boucle/placement_periodique.rs`.

- [ ] **Step 5 : commit**

```bash
git add agent/src/superviseur/boucle.rs
git commit -m "fix(d3): reposer la fenetre avant de lancer l'enfant

Le chemin de reutilisation d'une sortie retenue ne passe pas par creer_sortie,
donc la fenetre n'est pas reposee. L'application a pu la deplacer entre la mort
de l'enfant et sa relance ; sans cet appel l'enfant capturerait une fenetre mal
posee jusqu'au prochain controle periodique.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

# VOLET 2 — la campagne discriminante

## Task 8 : le mode `MULTIFENETRE_PLAFOND` — parsing et aiguillage

**Files:**
- Create: `agent/src/diagnostics/multifenetre/plafond.rs`
- Modify: `agent/src/diagnostics/multifenetre.rs` (déclaration de module +
  aiguillage)
- Modify: `scripts/run-agent.sh`

**Interfaces:**
- Produit :
  - `pub(super) fn analyser(valeur: &str) -> anyhow::Result<(u8, u8)>` — parse
    `"<P>x<D>"` ;
  - `pub(super) fn mesurer(processus: u8, duplications: u8) -> anyhow::Result<()>`
    — corps du porteur, écrit en Task 10 ;
  - `pub(super) fn sonder(sorties: &[String]) -> anyhow::Result<()>` —
    réexportée depuis `plafond::sonde`, écrite en Task 9.

**Contrainte non négociable :** `MULTIFENETRE_PLAFOND` **et**
`MULTIFENETRE_PLAFOND_SONDE` doivent être ajoutées à `scripts/run-agent.sh`
**dans cette tâche**. Le dépôt a payé deux fois pour l'oubli.

- [ ] **Step 1 : écrire le test qui échoue**

Créer `agent/src/diagnostics/multifenetre/plafond.rs` avec, pour l'instant, le
seul parsing et son module de tests :

```rust
//! Campagne discriminante du sous-bloc D3 : sur quoi porte le plafond de
//! quatre duplications DXGI simultanées ?
//!
//! Ce qu'on sait : **8 duplications de front dans UN SEUL processus tiennent**
//! (31 juillet 2026), et la **5ᵉ, dans un 5ᵉ processus, est refusée** en
//! `0x887A0022` (sous-bloc D2), par une limite durable qui résiste à trois
//! secondes de patience explicite. Rapprocher ces deux relevés pour conclure
//! « le plafond porte sur les processus » est une **inférence** : les deux
//! montages diffèrent d'au moins deux variables. Ce module existe pour
//! supprimer cette inférence.
//!
//! # Le montage
//!
//! Un processus **porteur** crée K = P×D sorties virtuelles, bat le chien de
//! garde du pilote et **ne duplique rien lui-même** — c'est la position exacte
//! du superviseur en D2, et c'est ce qui rend la mesure comparable au symptôme
//! de produit. Il lance ensuite P processus **sondes minimales**, qui ouvrent
//! chacune D duplications et les tiennent.
//!
//! **Pourquoi le porteur ne duplique pas** : une sonde peut mourir en
//! `0xc0000005`, comme toutes ces API. Le porteur, qui ne touche qu'au pilote,
//! survit, et sa garde détruit les K sorties. Un porteur qui dupliquerait
//! risquerait de les emporter avec lui.
//!
//! Spec : `docs/superpowers/specs/2026-08-02-multifenetres-plafond-concurrence-design.md`

mod sonde;

pub(super) use sonde::sonder;

use anyhow::{Context, Result};

/// Nombre maximal de sorties que la campagne s'autorise à créer. Le vivier du
/// pilote vaut **10** (mesuré le 31 juillet 2026, refus à la 11ᵉ création en
/// `ERROR_TOO_MANY_NAMES`), et Apollo puise au même : huit laisse deux de
/// marge.
const SORTIES_MAX: u16 = 8;

/// Analyse `"<P>x<D>"` : P processus sondes, D duplications chacune.
pub(super) fn analyser(valeur: &str) -> Result<(u8, u8)> {
    let (p, d) = valeur
        .split_once('x')
        .with_context(|| format!("MULTIFENETRE_PLAFOND attend « <P>x<D> », reçu « {valeur} »"))?;
    let processus: u8 = p
        .trim()
        .parse()
        .with_context(|| format!("nombre de processus illisible dans « {valeur} »"))?;
    let duplications: u8 = d
        .trim()
        .parse()
        .with_context(|| format!("nombre de duplications illisible dans « {valeur} »"))?;
    anyhow::ensure!(processus >= 1, "au moins un processus sonde est nécessaire");
    anyhow::ensure!(duplications >= 1, "au moins une duplication par sonde est nécessaire");
    let total = u16::from(processus) * u16::from(duplications);
    anyhow::ensure!(
        total <= SORTIES_MAX,
        "{processus}x{duplications} demande {total} sorties, le maximum est {SORTIES_MAX} \
         (vivier du pilote de 10, dont deux de marge pour Apollo)"
    );
    Ok((processus, duplications))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_cinq_rangs_de_la_matrice_sont_acceptes() {
        for (valeur, attendu) in
            [("1x8", (1, 8)), ("2x4", (2, 4)), ("4x2", (4, 2)), ("8x1", (8, 1)), ("4x1", (4, 1))]
        {
            assert_eq!(analyser(valeur).unwrap(), attendu, "rang {valeur}");
        }
    }

    #[test]
    fn un_produit_au_dela_du_vivier_est_refuse() {
        let erreur = analyser("4x4").unwrap_err().to_string();
        assert!(erreur.contains("16 sorties"), "reçu « {erreur} »");
    }

    #[test]
    fn un_zero_est_refuse() {
        assert!(analyser("0x4").is_err());
        assert!(analyser("4x0").is_err());
    }

    #[test]
    fn une_valeur_malformee_est_refusee() {
        assert!(analyser("4").is_err());
        assert!(analyser("quatre x deux").is_err());
    }
}
```

- [ ] **Step 2 : exécuter le test pour vérifier qu'il échoue**

```bash
cd agent && cargo test --workspace plafond 2>&1 | tail -20
```

Attendu : ÉCHEC — le module n'est pas déclaré, et `sonde` n'existe pas.

- [ ] **Step 3 : déclarer le module et créer le squelette de la sonde**

Dans `agent/src/diagnostics/multifenetre.rs`, ajouter la déclaration de module
en respectant l'ordre alphabétique existant, entre `nvenc` et `pointeur_virtuel` :

```rust
pub(super) mod plafond;
```

Créer `agent/src/diagnostics/multifenetre/plafond/sonde.rs` avec un corps
provisoire (la Task 9 l'écrit) :

```rust
//! La sonde minimale : D duplications DXGI, tenues, et rien d'autre.

use anyhow::Result;

pub(super) fn sonder(_sorties: &[String]) -> Result<()> {
    anyhow::bail!("sonde non implémentée — voir la Task 9 du plan D3")
}
```

Ajouter les deux ajouts d'aiguillage dans `aiguiller()`, **après**
`MULTIFENETRE_VDD_PURGE` (le mode crée des sorties, une purge demandée doit
toujours l'emporter) :

```rust
    // Sous-bloc D3 — la sonde minimale, lancée par le porteur ci-dessous. Elle
    // est aiguillée AVANT le porteur : un processus qui porte les deux
    // variables (elles sont héritées) doit se comporter en sonde, sinon il
    // créerait à son tour K sorties.
    if let Ok(liste) = std::env::var("MULTIFENETRE_PLAFOND_SONDE") {
        let sorties: Vec<String> = liste.split(',').map(|s| s.trim().to_string()).collect();
        plafond::sonder(&sorties)?;
        return Ok(true);
    }
    // Sous-bloc D3 — le porteur : K = P×D sorties virtuelles, P sondes.
    if let Ok(valeur) = std::env::var("MULTIFENETRE_PLAFOND") {
        let (processus, duplications) = plafond::analyser(&valeur)?;
        plafond::mesurer(processus, duplications)?;
        return Ok(true);
    }
```

Ajouter un `mesurer` provisoire dans `plafond.rs` :

```rust
/// Le porteur. Corps écrit en Task 10 du plan D3.
pub(super) fn mesurer(_processus: u8, _duplications: u8) -> Result<()> {
    anyhow::bail!("porteur non implémenté — voir la Task 10 du plan D3")
}
```

- [ ] **Step 4 : transmettre les variables dans `scripts/run-agent.sh`**

Ajouter les deux lignes à la suite de `MULTIFENETRE_REPRISE` (l. 68 environ) :

```bash
${MULTIFENETRE_PLAFOND:+\$env:MULTIFENETRE_PLAFOND = '$MULTIFENETRE_PLAFOND'}
${MULTIFENETRE_PLAFOND_SONDE:+\$env:MULTIFENETRE_PLAFOND_SONDE = '$MULTIFENETRE_PLAFOND_SONDE'}
```

Vérifier :

```bash
grep -n "MULTIFENETRE_PLAFOND" scripts/run-agent.sh
```

Attendu : deux lignes.

- [ ] **Step 5 : exécuter les tests pour vérifier qu'ils passent**

```bash
cd agent && cargo test --workspace plafond 2>&1 | tail -20
```

Attendu : SUCCÈS des quatre tests de parsing.

- [ ] **Step 6 : commit**

```bash
git add agent/src/diagnostics/multifenetre/plafond.rs \
        agent/src/diagnostics/multifenetre/plafond/sonde.rs \
        agent/src/diagnostics/multifenetre.rs scripts/run-agent.sh
git commit -m "feat(d3): aiguiller le mode MULTIFENETRE_PLAFOND, et le transmettre

Parsing de <P>x<D> borne par le vivier du pilote. Les deux variables sont
ajoutees a run-agent.sh DANS CETTE TACHE : le depot a paye deux fois pour
l'oubli (SUPERVISEUR en D1, MULTIFENETRE_REPRISE en D2).

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Task 9 : la sonde minimale

**Files:**
- Modify: `agent/src/diagnostics/multifenetre/plafond/sonde.rs`

**Interfaces:**
- Consomme : `crate::capture::DesktopCapture::sur_sortie(nom: &str)` — résout
  une sortie **par nom DXGI** (`\\.\DISPLAYn`), les index étant positionnels.
- Produit : `pub(super) fn sonder(sorties: &[String]) -> anyhow::Result<()>`,
  et le protocole de fichier témoin décrit ci-dessous.

**Protocole de fichier témoin.** La sonde écrit dans `%TEMP%` un fichier
`plafond-sonde-<rang>.verdict` dont le contenu est `OK` ou
`KO <hresult> <sortie>`, puis attend l'apparition de
`%TEMP%\plafond-arret` pour sortir. Le porteur lit le verdict avant de lancer la
sonde suivante. Choix assumé : c'est fruste, mais le dépôt n'a aucune IPC
superviseur→enfant, et en inventer une pour une campagne de mesure serait du
périmètre en trop.

**La sonde n'ouvre RIEN d'autre que des duplications.** Ni périphérique D3D11
d'encodage, ni encodeur, ni fenêtre, ni WebRTC. C'est ce qui discrimine H3.

- [ ] **Step 1 : relever la signature réelle d'ouverture d'une duplication**

```bash
grep -n "pub fn sur_sortie\|pub fn nouvelle\|pub fn new" agent/src/capture.rs agent/src/capture/*.rs | head
```

Noter la signature exacte : elle est le seul point de contact de la sonde avec
le reste de l'agent, et le code ci-dessous doit s'y conformer.

- [ ] **Step 2 : écrire la sonde**

Remplacer le contenu de `agent/src/diagnostics/multifenetre/plafond/sonde.rs` :

```rust
//! La sonde minimale : D duplications DXGI, tenues, et rien d'autre.
//!
//! **Rien d'autre est le point.** Ni périphérique D3D11 d'encodage, ni
//! encodeur, ni fenêtre, ni WebRTC. Si le refus de la 5ᵉ duplication observé
//! au sous-bloc D2 ne se reproduit pas ici, c'est que le plafond ne porte pas
//! sur la duplication mais sur ce qui l'accompagnait dans l'enfant (hypothèse
//! H3 de la spec) — et c'est un résultat, pas une panne de la sonde.
//!
//! Le rang de la sonde vient de `MULTIFENETRE_PLAFOND_RANG` ; chaque ligne le
//! porte, car `agent.log` mêle le porteur et toutes ses sondes par héritage de
//! `stdout` et rien d'autre ne distinguerait l'émetteur (piège relevé en D1).

use anyhow::{Context, Result};

/// Fichier témoin du porteur : sa présence ordonne la sortie.
pub(super) fn chemin_arret() -> std::path::PathBuf {
    std::env::temp_dir().join("plafond-arret")
}

/// Fichier de verdict d'une sonde.
pub(super) fn chemin_verdict(rang: u8) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("plafond-sonde-{rang}.verdict"))
}

pub(super) fn sonder(sorties: &[String]) -> Result<()> {
    let rang: u8 = std::env::var("MULTIFENETRE_PLAFOND_RANG")
        .unwrap_or_else(|_| "0".to_string())
        .parse()
        .context("MULTIFENETRE_PLAFOND_RANG doit être un entier")?;

    tracing::info!(sonde = rang, sorties = ?sorties, "sonde démarrée");

    // Les duplications sont TENUES dans ce vecteur : les relâcher libérerait
    // la place et la mesure ne mesurerait plus rien.
    let mut tenues = Vec::new();
    let mut verdict = String::from("OK");
    for (rang_local, nom) in sorties.iter().enumerate() {
        match crate::capture::DesktopCapture::sur_sortie(nom) {
            Ok(duplication) => {
                tracing::info!(
                    sonde = rang,
                    duplication = rang_local + 1,
                    %nom,
                    "duplication ouverte"
                );
                tenues.push(duplication);
            }
            Err(erreur) => {
                // Le HRESULT EXACT, et le rang : c'est la seule donnée que la
                // matrice exploite.
                tracing::error!(
                    sonde = rang,
                    duplication = rang_local + 1,
                    %nom,
                    %erreur,
                    "duplication REFUSÉE"
                );
                verdict = format!("KO {erreur} {nom}");
                break;
            }
        }
    }

    std::fs::write(chemin_verdict(rang), &verdict)
        .with_context(|| format!("écriture du verdict de la sonde {rang}"))?;
    tracing::info!(sonde = rang, ouvertes = tenues.len(), %verdict, "verdict déposé");

    // Tenir jusqu'au signal du porteur. Les duplications restent ouvertes tant
    // que `tenues` est vivant.
    while !chemin_arret().exists() {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    tracing::info!(sonde = rang, "arrêt demandé, relâchement des duplications");
    Ok(())
}
```

⚠️ **Adapter `crate::capture::DesktopCapture::sur_sortie` à la signature
relevée au Step 1.** Au 2 août 2026 elle vaut
`pub fn sur_sortie(nom: &str) -> Result<Self>` et convient telle quelle.
**Ne pas ajouter d'encodeur ni de périphérique d'encodage** : ce serait
exactement la variable que la sonde existe pour exclure.

**Fait à connaître, et à redire dans les résultats :** `sur_sortie` retente une
ouverture refusée pour indisponibilité passagère pendant
`capture_reprise::DUREE_FENETRE_OUVERTURE`. Un refus rapporté par la sonde est
donc **déjà un refus durable**, pas un transitoire — c'est cohérent avec le
relevé de D2 (« résiste à trois secondes de patience explicite ») et c'est ce
qui donne son poids au verdict. Relever la valeur de cette constante et la citer
dans les résultats.

- [ ] **Step 3 : vérifier que le workspace compile encore sur l'hôte**

```bash
cd agent && cargo clippy --workspace 2>&1 | tail -10
```

Attendu : aucun avertissement neuf. `sonde.rs` est sous
`diagnostics/multifenetre`, qui est `#[cfg(windows)]` — la compilation réelle a
lieu sur la VM (Tasks 11-12).

- [ ] **Step 4 : commit**

```bash
git add agent/src/diagnostics/multifenetre/plafond/sonde.rs
git commit -m "feat(d3): la sonde minimale — D duplications tenues, et rien d'autre

Ni peripherique D3D11 d'encodage, ni encodeur, ni fenetre : c'est ce qui
discrimine l'hypothese H3. Le HRESULT exact et le rang de la duplication
refusee sont la seule donnee que la matrice exploite.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Task 10 : le porteur

**Files:**
- Modify: `agent/src/diagnostics/multifenetre/plafond.rs`

**Interfaces:**
- Consomme : `crate::moniteurs_virtuels::pilote::ouvrir_pilote()`,
  `crate::moniteurs_virtuels::Sorties::{nouvelles, creer, detruire}`,
  `super::montee::{relever_topologie, noms_attaches, attendre_en_pinguant, RESOLUTION}`,
  `sonde::{chemin_arret, chemin_verdict}`.
- Produit : `pub(super) fn mesurer(processus: u8, duplications: u8) -> Result<()>`
  et la table de verdict journalisée en fin de passe.

- [ ] **Step 1 : relever les signatures réelles des dépendances**

```bash
grep -n "pub fn attendre_en_pinguant\|pub const RESOLUTION\|pub fn relever_topologie\|pub fn noms_attaches" agent/src/diagnostics/multifenetre/montee.rs
sed -n '60,110p' agent/src/moniteurs_virtuels.rs
grep -n "pub fn pinguer" agent/src/moniteurs_virtuels/pilote.rs
```

Le code ci-dessous doit s'y conformer exactement.

- [ ] **Step 2 : écrire le porteur**

Ajouter à `agent/src/diagnostics/multifenetre/plafond.rs`, en remplaçant le
`mesurer` provisoire :

```rust
/// Le porteur : K = P×D sorties virtuelles, P sondes lancées en ESCALIER.
///
/// **L'échelonnement n'est pas un confort.** P sondes concurrentes rendraient
/// un refus sans rang identifiable, et la matrice ne trancherait rien : c'est
/// le rang du premier refus qui distingue « plafond sur les processus » de
/// « plafond sur les duplications ».
pub(super) fn mesurer(processus: u8, duplications: u8) -> Result<()> {
    let total = u16::from(processus) * u16::from(duplications);
    tracing::info!(processus, duplications, total, "campagne du plafond — début");

    // Aucun résidu d'un tirage précédent : un verdict périmé ferait lire un
    // succès là où la sonde n'a jamais démarré.
    let _ = std::fs::remove_file(sonde::chemin_arret());
    for rang in 0..processus {
        let _ = std::fs::remove_file(sonde::chemin_verdict(rang));
    }

    let avant = relever_topologie("avant création")?;
    let noms_avant = noms_attaches(&avant);

    let pilote = crate::moniteurs_virtuels::pilote::ouvrir_pilote()?;
    let (largeur, hauteur, hertz) = RESOLUTION;

    // Portée explicite de la garde : les sorties doivent être détruites AVANT
    // le relevé final, sans quoi celui-ci décrirait un état transitoire.
    let issue = {
        let mut sorties = crate::moniteurs_virtuels::Sorties::nouvelles(&pilote);
        for rang in 0..total {
            sorties
                .creer(largeur, hauteur, hertz)
                .with_context(|| format!("création de la sortie {}/{total}", rang + 1))?;
        }
        // Attendre que les K sorties soient RATTACHÉES, en battant le chien de
        // garde : le pilote retire les sorties d'un client qui cesse de
        // pinguer, y compris celles qu'on vient de créer.
        let apparues = attendre_en_pinguant(&pilote, &noms_avant, usize::from(total))?;
        tracing::info!(attachees = apparues.len(), noms = ?apparues, "sorties rattachées");

        conduire_les_sondes(processus, duplications, &apparues)
    };

    // Signal d'arrêt retiré : le prochain tirage repart propre.
    let _ = std::fs::remove_file(sonde::chemin_arret());

    let apres = relever_topologie("après destruction")?;
    let noms_apres = noms_attaches(&apres);
    // Comparer des ENSEMBLES DE NOMS, jamais des cardinaux : Apollo peut
    // ajouter une sortie à tout instant, et une addition externe compenserait
    // exactement un retrait.
    if noms_apres != noms_avant {
        tracing::error!(
            avant = ?noms_avant, apres = ?noms_apres,
            "topologie NON restaurée — contrôler depuis un processus neuf (MULTIFENETRE_DXGI=1)"
        );
    } else {
        tracing::info!("topologie restaurée nom pour nom");
    }
    issue
}

/// Lance les sondes une à une et journalise la table de verdict.
fn conduire_les_sondes(processus: u8, duplications: u8, noms: &[String]) -> Result<()> {
    let executable = std::env::current_exe().context("chemin de l'exécutable courant")?;
    let mut enfants = Vec::new();
    let mut verdicts: Vec<(u8, String)> = Vec::new();

    for rang in 0..processus {
        let debut = usize::from(rang) * usize::from(duplications);
        let lot: Vec<&str> =
            noms[debut..debut + usize::from(duplications)].iter().map(|s| s.as_str()).collect();
        tracing::info!(sonde = rang, sorties = ?lot, "lancement de la sonde");

        let enfant = std::process::Command::new(&executable)
            .env("MULTIFENETRE_PLAFOND_SONDE", lot.join(","))
            .env("MULTIFENETRE_PLAFOND_RANG", rang.to_string())
            // La variable du porteur ne doit PAS être héritée : l'enfant
            // recréerait K sorties. L'aiguillage traite déjà la sonde en
            // premier, mais retirer la variable rend l'invariant explicite.
            .env_remove("MULTIFENETRE_PLAFOND")
            .spawn()
            .with_context(|| format!("lancement de la sonde {rang}"))?;
        enfants.push(enfant);

        let verdict = attendre_le_verdict(rang)?;
        tracing::info!(sonde = rang, %verdict, "verdict reçu");
        let refuse = verdict.starts_with("KO");
        verdicts.push((rang, verdict));
        if refuse {
            // On s'arrête au premier refus : c'est SON rang qui est la mesure.
            // Continuer ne rendrait que des refus dérivés.
            break;
        }
    }

    // Signal d'arrêt : toutes les sondes relâchent et sortent.
    std::fs::write(sonde::chemin_arret(), b"1").context("dépôt du signal d'arrêt")?;
    for (rang, mut enfant) in enfants.into_iter().enumerate() {
        match enfant.wait() {
            Ok(statut) => tracing::info!(sonde = rang, ?statut, "sonde terminée"),
            Err(erreur) => tracing::error!(sonde = rang, %erreur, "attente de la sonde échouée"),
        }
    }

    tracing::info!(
        processus,
        duplications,
        lancees = verdicts.len(),
        verdicts = ?verdicts,
        "campagne du plafond — bilan"
    );
    Ok(())
}

/// Attend le verdict d'une sonde, ou conclut qu'elle est morte sans en rendre.
///
/// **Borné.** Une sonde peut mourir en `0xc0000005` sans jamais écrire son
/// fichier ; sans cette borne, le porteur attendrait indéfiniment en tenant K
/// sorties virtuelles.
fn attendre_le_verdict(rang: u8) -> Result<String> {
    const LIMITE: std::time::Duration = std::time::Duration::from_secs(30);
    let echeance = std::time::Instant::now() + LIMITE;
    let chemin = sonde::chemin_verdict(rang);
    loop {
        if let Ok(contenu) = std::fs::read_to_string(&chemin) {
            return Ok(contenu.trim().to_string());
        }
        if std::time::Instant::now() >= echeance {
            return Ok(format!("MORTE (aucun verdict en {} s)", LIMITE.as_secs()));
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
```

⚠️ **`attendre_en_pinguant` a peut-être une autre signature** que celle
supposée ici (`(&pilote, &noms_avant, attendues) -> Result<Vec<String>>`).
L'adapter au relevé du Step 1, en conservant l'invariant : **battre le chien de
garde pendant l'attente**, et rendre les **noms** des sorties apparues.

- [ ] **Step 3 : vérifier la compilation sur l'hôte, dans la mesure du possible**

```bash
cd agent && cargo test --workspace 2>&1 | tail -10 && cargo clippy --workspace 2>&1 | tail -10
```

Attendu : les tests de parsing passent toujours, clippy sans avertissement neuf.

- [ ] **Step 4 : contrôler la taille des fichiers**

```bash
wc -l agent/src/diagnostics/multifenetre/plafond.rs agent/src/diagnostics/multifenetre/plafond/sonde.rs
```

Si `plafond.rs` dépasse 500 lignes, extraire `conduire_les_sondes` et
`attendre_le_verdict` dans `plafond/conduite.rs`.

- [ ] **Step 5 : commit**

```bash
git add agent/src/diagnostics/multifenetre/plafond.rs
git commit -m "feat(d3): le porteur — K sorties, P sondes lancees en escalier

L'echelonnement n'est pas un confort : P sondes concurrentes rendraient un
refus sans rang identifiable, et c'est le rang du premier refus qui distingue
un plafond sur les processus d'un plafond sur les duplications.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Task 11 : éprouver le critère 1 en conditions de produit

**Files:**
- Create: `docs/superpowers/plans/journaux-multifenetres-d3/` (les journaux)
- Copy: `docs/superpowers/plans/journaux-multifenetres-d2/instrument/pilote-recette-d2.mjs`
  → `docs/superpowers/plans/journaux-multifenetres-d3/instrument/pilote-recette-d3.mjs`

**Aucun code de production.** C'est la recette du volet 1, et **elle est le
critère 1 de réception** : sur une séquence où une fenêtre est condamnée pendant
que d'autres capturent, **zéro réouverture de duplication imputable à une
relance d'enfant**, et **aucune session saine perdue**.

Sans cette tâche, le volet 1 ne serait éprouvé que par des tests de logique
pure : ils démontrent que la table n'émet plus `DetruireSortie`, pas que les
sessions voisines cessent d'en souffrir.

**Contrainte de protocole, héritée de D1 et D2 :**

- **Aucune capture d'écran CDP pendant la mesure.** Elle provoque un `Resize`,
  donc un `SHOW`, donc une session et une sortie de plus. Depuis D2 le dernier
  maillon ne tue plus rien, mais la chaîne demeure entière et fausserait le
  compte.
- **Toute évaluation CDP sur une page portant un flux WebRTC actif doit être
  BORNÉE** : `Page.captureScreenshot` peut ne **jamais** rendre.
- **Lancer le navigateur AVANT le superviseur** : le signaling ne mémorise que
  les offres SDP, les annonces `fenetre-ouverte` émises avant la connexion de la
  page-shell sont perdues sans trace. ⚠️ **Et depuis la Task 6, une fenêtre
  préexistante dont la page-shell tarde de plus de 30 s est ABANDONNÉE** — cette
  contrainte est passée de recommandation à obligation.
- **Chrome avec `--disable-popup-blocking`**, sans quoi la démonstration est
  vide et muette.
- **Vérifier `Get-Process agent` avant l'exécution** : un agent survit à
  l'hibernation de la VM et `run-agent.sh` ne le tue pas.
- **`pkill -f <motif>` depuis un shell dont la ligne de commande contient le
  motif tue le shell lui-même.** Tuer par PID relevé.

- [ ] **Step 1 : préparer la VM, compiler, contrôler l'état de départ**

Exécuter les Steps 1 à 3 de la Task 12 (démarrage de la VM, compilation avec
`.env` sourcé, purge et relevé de topologie depuis un processus neuf). Ils sont
communs aux deux tâches ; ne pas les rejouer si la Task 12 vient de les faire.

- [ ] **Step 2 : dériver l'instrument de recette**

```bash
mkdir -p docs/superpowers/plans/journaux-multifenetres-d3/instrument
cp docs/superpowers/plans/journaux-multifenetres-d2/instrument/pilote-recette-d2.mjs \
   docs/superpowers/plans/journaux-multifenetres-d3/instrument/pilote-recette-d3.mjs
cp docs/superpowers/plans/journaux-multifenetres-d2/instrument/{preparer,listefen,nettoyer,ouvrir-2,ouvrir-3,ouvrir-4,fermer-2}.ps1 \
   docs/superpowers/plans/journaux-multifenetres-d3/instrument/
```

Lire `pilote-recette-d3.mjs` et l'adapter à la séquence ci-dessous. **Verser
l'instrument dans son état final**, celui de la dernière exécution — c'est ce
que D1 et D2 ont fait, et c'est ce qui rend la mesure relisible.

- [ ] **Step 3 : jouer la séquence de critère**

La séquence, dans cet ordre exact :

1. lancer Chrome puis la page-shell, attendre sa connexion au signaling ;
2. lancer le superviseur (`SUPERVISEUR=1 scripts/run-agent.sh`) ;
3. ouvrir **trois** Bloc-notes, un par un, en attendant que chacun ait sa
   fenêtre navigateur avant d'ouvrir le suivant — ce sont les **sessions
   saines** ;
4. **condamner une quatrième fenêtre** : ouvrir une fenêtre dont l'enfant
   mourra à coup sûr. Le moyen le plus simple et le plus reproductible est de
   tuer son processus enfant dès qu'il apparaît, par PID relevé côté Windows —
   ce qui fait passer l'entrée en `SansSession` puis déclenche la relance. Le
   répéter jusqu'à l'abandon après `RELANCES_MAX` ;
5. **relever le compteur de réouvertures de duplication, borné par une fenêtre
   temporelle explicite** : le premier horodatage est celui du lancement du
   quatrième enfant, le dernier celui de son abandon.

⚠️ **Ne jamais opposer un total de fichier à un compte fenêtré.** Le journal
continue de courir après la séquence ; D2 a payé cette confusion (44 dans la
fenêtre, 50 en fin de fichier).

- [ ] **Step 4 : relever le critère**

```bash
cd docs/superpowers/plans/journaux-multifenetres-d3
sed 's/\x1b\[[0-9;]*m//g' agent-critere1.log > agent-critere1.txt
# Les deux grandeurs du critère, dans la fenêtre temporelle relevée au Step 3 :
grep -nE "accès perdu|0x887A0026|duplication de sortie établie" agent-critere1.txt
grep -nE "clôture de session amorcée" agent-critere1.txt
# Et la preuve que la réutilisation a bien eu lieu, plutôt qu'une recréation :
grep -nE "sortie virtuelle rendue au pilote|création de sortie" agent-critere1.txt
```

**Le critère est tenu si**, dans la fenêtre :

- aucune ligne de création de sortie n'est imputable à une **relance** de la
  fenêtre condamnée (celles de son ouverture initiale et de son abandon final ne
  le sont pas) ;
- aucune ligne `clôture de session amorcée` ne concerne l'une des trois sessions
  saines.

**Si le critère n'est pas tenu, le dire et l'analyser — ne pas rejouer jusqu'à
ce qu'il passe.** Un défaut intermittent qu'on croit déterministe se déclare
corrigé à la première exécution qui passe.

- [ ] **Step 5 : copier les journaux et committer**

```bash
cp /media/vm/dev/agent.log \
   docs/superpowers/plans/journaux-multifenetres-d3/agent-critere1.log
git add docs/superpowers/plans/journaux-multifenetres-d3/
git commit -m "recette(d3): critere 1 — une fenetre condamnee n'inflige plus de reouvertures

Sequence : trois sessions saines, une quatrieme fenetre condamnee jusqu'a son
abandon. Compte des reouvertures borne par une fenetre temporelle explicite,
jamais un total de fichier.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Task 12 : exécuter la campagne sur la VM

**Files:**
- Create: `docs/superpowers/plans/journaux-multifenetres-d3/` (les journaux)

**Aucun code.** C'est la tâche de mesure du volet 2. Elle exige la VM, et **une
seule tâche à la fois y accède**.

- [ ] **Step 1 : démarrer la VM et attendre l'accès réel au partage**

```bash
cd /home/mallanic/Projects/Guacamole
set -a && source .env && set +a
virsh list --all
virsh start Windows
until timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null; do sleep 5; done
# /media/vm se monte APRÈS que WinRM réponde. `mountpoint -q` ne suffit pas :
# l'entrée CIFS persiste VM éteinte. Éprouver un ACCÈS réel.
until ls /media/vm/dev >/dev/null 2>&1; do sleep 5; done
echo "VM prête"
```

- [ ] **Step 2 : compiler, et vérifier que la compilation a réellement eu lieu**

```bash
set -a && source .env && set +a   # sans quoi build-agent.sh s'arrête EN SILENCE
mkdir -p docs/superpowers/plans/journaux-multifenetres-d3
scripts/build-agent.sh 2>&1 \
  | tee docs/superpowers/plans/journaux-multifenetres-d3/build-agent.log | tail -20
ls -l --time-style=full-iso /media/vm/dev/target/release/agent.exe
```

**La sortie du build est CAPTURÉE dans un fichier, et versée.** Le chantier des
duplications parallèles a dû reconnaître que la fraîcheur de son binaire
n'était adossée à aucune pièce, faute d'avoir fait exactement cela.

Attendu : une ligne de succès de `cargo build`, et un horodatage du binaire
postérieur au lancement. **Un build muet est le symptôme d'un `.env` non
sourcé** — ne pas le prendre pour un succès.

- [ ] **Step 3 : contrôler l'état de départ depuis un processus neuf**

```bash
MULTIFENETRE_VDD_PURGE=1 scripts/run-agent.sh
sleep 10
MULTIFENETRE_DXGI=1 scripts/run-agent.sh
sleep 10
cp /media/vm/dev/agent.log docs/superpowers/plans/journaux-multifenetres-d3/etat-initial.log
grep -c "DISPLAY" docs/superpowers/plans/journaux-multifenetres-d3/etat-initial.log
```

Noter l'ensemble des **noms** de sorties présentes. C'est la référence de toutes
les comparaisons de la campagne.

- [ ] **Step 4 : exécuter les cinq rangs, trois fois chacun**

Pour chaque rang de `1x8 2x4 4x2 8x1 4x1` et chaque exécution `1 2 3` :

```bash
mkdir -p docs/superpowers/plans/journaux-multifenetres-d3
for rang in 1x8 2x4 4x2 8x1 4x1; do
  for essai in 1 2 3; do
    # Un agent survivant mesurerait le processus précédent.
    node scripts/winrm.js 'Get-Process agent -ErrorAction SilentlyContinue | Stop-Process -Force'
    MULTIFENETRE_VDD_PURGE=1 scripts/run-agent.sh; sleep 8
    MULTIFENETRE_PLAFOND="$rang" scripts/run-agent.sh
    sleep 45
    # COPIER AVANT le tirage suivant : un journal d'agent s'écrase facilement,
    # et deux pièces ont été perdues ainsi en D2.
    cp /media/vm/dev/agent.log \
       "docs/superpowers/plans/journaux-multifenetres-d3/plafond-${rang}-${essai}.log"
    # La VM se met en veille prolongée toute seule (cause non identifiée) :
    # contrôler sa survie APRÈS CHAQUE RANG, pas à la fin.
    virsh list --all | grep -q "en cours d'exécution" || { echo "VM ÉTEINTE après $rang/$essai"; break 2; }
  done
done
```

⚠️ **`sleep 45` est un majorant à ajuster au premier tirage** : la campagne
n'attend pas une durée, elle attend le bilan. Si `plafond-1x8-1.log` ne porte
pas la ligne `campagne du plafond — bilan`, allonger et rejouer ce rang.

- [ ] **Step 5 : relever la matrice**

```bash
cd docs/superpowers/plans/journaux-multifenetres-d3
for f in plafond-*.log; do
  echo "=== $f"
  sed 's/\x1b\[[0-9;]*m//g' "$f" | grep -E "campagne du plafond — bilan|duplication REFUSÉE|topologie (restaurée|NON restaurée)"
done
```

Reporter dans un tableau, pour chaque rang et chaque exécution : le rang de la
sonde qui refuse (ou « aucun »), le `HRESULT` exact, et l'état de restauration
de la topologie.

- [ ] **Step 6 : contrôle final depuis un processus neuf**

```bash
MULTIFENETRE_DXGI=1 scripts/run-agent.sh
sleep 10
cp /media/vm/dev/agent.log docs/superpowers/plans/journaux-multifenetres-d3/etat-final.log
```

Comparer l'**ensemble des noms** à celui du Step 3. S'ils diffèrent, purger et
le dire dans les résultats — une sortie orpheline invaliderait tout rang
ultérieur.

- [ ] **Step 7 : commit des journaux**

```bash
git add docs/superpowers/plans/journaux-multifenetres-d3/
git commit -m "mesure(d3): journaux de la campagne du plafond de concurrence

Cinq rangs (1x8, 2x4, 4x2, 8x1, 4x1), trois executions chacun, etat de la
topologie releve avant et apres depuis un processus neuf.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Task 13 : la décision, `CAPACITE`, et le document de résultats

**Files:**
- Create: `docs/superpowers/plans/2026-08-02-multifenetres-plafond-concurrence-resultats.md`
- Modify: `agent/src/superviseur/boucle.rs:45` (`CAPACITE`)
- Modify: `CLAUDE.md` (section D3)
- Modify: `docs/superpowers/specs/2026-07-28-support-jeux-design.md` (§5 D et §8)

**Interfaces:**
- Consomme : le relevé de critère de la Task 11 et la matrice de la Task 12.
- Produit : la décision d'arrangement, et la valeur de `CAPACITE`.

- [ ] **Step 1 : appliquer la règle de décision, telle qu'écrite avant la mesure**

La spec §3.5 fixe les quatre issues. **Ne pas la réécrire après coup :**

| Verdict observé | Décision |
| --- | --- |
| Témoin OK, A OK, B OK, C refusé au 5ᵉ processus (**H1**) | La contrainte est le nombre de processus → la capture mutualisée (repli spec §8) est **désignée** pour D4, non implémentée. `CAPACITE = 4`. |
| Témoin refusé à la 5ᵉ duplication (**H2**) | Le plafond frappe aussi le processus unique dès que le créateur est distinct → la mutualisation ne sauve rien, la cible de huit fenêtres est rouverte. `CAPACITE` = rang du dernier succès du témoin. |
| Rang C réussit | Le plafond de D2 avait une **autre** cause. À rouvrir avec les journaux de D2 en main. `CAPACITE` **reste à 8**. |
| Le refus ne se reproduit pas sur sonde minimale (**H3**) | Le plafond est du côté périphérique ou encodeur → sujet de D4. `CAPACITE = 4`, valeur du plafond observé en conditions de produit, **sans que sa couche soit identifiée**. |

- [ ] **Step 2 : si H3 — escalade bornée, au plus deux fois**

Épaissir la sonde d'un élément à la fois : d'abord un périphérique D3D11, puis
un encodeur H.264. Rejouer le rang C après chaque épaississement. **Au plus deux
épaississements.** Si le symptôme ne revient pas, écrire « non reproduit par
sonde minimale ni épaissie deux fois » — c'est un résultat.

- [ ] **Step 3 : fixer `CAPACITE`**

Dans `agent/src/superviseur/boucle.rs:45`, poser la valeur décidée avec sa
justification :

```rust
/// Fenêtres simultanées que le superviseur s'autorise.
///
/// **Valeur MESURÉE, non prouvée être une borne du système** (campagne du
/// sous-bloc D3, `plans/2026-08-02-multifenetres-plafond-concurrence-resultats.md`).
/// Elle valait 8 — le plafond d'encodeurs en processus unique — alors que le
/// plafond réellement rencontré en conditions de produit est celui des
/// duplications DXGI simultanées. Le superviseur acceptait donc des fenêtres
/// dont aucune ne pouvait aboutir : chacune brûlait quatre tentatives, et
/// chaque tentative recréait une sortie virtuelle.
const CAPACITE: usize = 4;
```

*(Ajuster la valeur et le texte au verdict réellement obtenu.)*

- [ ] **Step 4 : écrire le document de résultats**

Créer
`docs/superpowers/plans/2026-08-02-multifenetres-plafond-concurrence-resultats.md`
avec la structure des chantiers précédents :

- **§0 Le verdict**, en une phrase par volet ;
- **§1 Ce qui a été exécuté, et ce qui a été écarté** ;
- **§2 Le déroulé, avec son issue réellement observée** ;
- **§3 Ce qui a échoué** ;
- **§4 Ce que cette campagne N'établit PAS** — au minimum : une VM mono-GPU
  donc rien d'un plafond par adaptateur ; une seule sortie physique donc aucune
  série virtuel/physique ; rien d'autres résolutions ni fréquences ; rien de la
  tenue dans la durée ; et le nombre exact d'exécutions par rang ;
- **§5 Pièges rencontrés** ;
- **§6 État de la VM à la fin** ;
- **§7 Ce qu'il reste à régler** ;
- **§8 Contrôle de la dette de taille de fichier**.

**Règle d'énoncé, et c'est le mode de défaillance dominant de ce projet :
n'affirmer que ce que le relevé porte.** Distinguer systématiquement *relevé* de
*calculé* et de *inféré*. Un chiffre sans journal joint se dit tel quel.

- [ ] **Step 5 : accorder les documents amont**

- `CLAUDE.md` : ajouter une section « Sous-bloc D3 » après celle de D2, et
  **annoter les affirmations de D2 que D3 réfute ou complète — les chercher par
  le SENS, pas par la formule**. Une négation se dit de plusieurs façons, et
  c'est celle qu'on n'a pas listée qui survit : balayer sur la *chose niée*
  (« le plafond de quatre », « la couche non identifiée », « CAPACITE = 8 »,
  « les 32 réouvertures parasites ») en énumérant les tournures. **Annoter
  l'affirmation elle-même, pas sa voisine**, et **traiter le sommaire autant que
  le chapitre de détail**.
- `docs/superpowers/specs/2026-07-28-support-jeux-design.md` : encadré D3 en
  tête du §5 D, et mise à jour du point 3 du §8.
- `docs/superpowers/plans/2026-08-01-multifenetres-arrangement-dynamique-resultats.md` :
  annoter les §7.1, §7.2, §7.2 bis et §7.3 comme traités, en renvoyant aux
  résultats de D3.

- [ ] **Step 6 : contrôler la dette de taille de fichier**

```bash
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
```

Attendu : uniquement les trois fichiers de dette assumée (`encode.rs`,
`windows_source.rs`, `wasapi.rs`). Tout fichier neuf au-dessus de 500 lignes
doit être découpé avant le commit.

- [ ] **Step 7 : vérification finale**

```bash
cd agent && cargo test --workspace && cargo clippy --workspace
```

Attendu : SUCCÈS, aucun avertissement. **Ne pas déclarer le sous-bloc terminé
avant d'avoir vu cette sortie.**

- [ ] **Step 8 : commit**

```bash
git add docs/superpowers/plans/2026-08-02-multifenetres-plafond-concurrence-resultats.md \
        agent/src/superviseur/boucle.rs CLAUDE.md \
        docs/superpowers/specs/2026-07-28-support-jeux-design.md \
        docs/superpowers/plans/2026-08-01-multifenetres-arrangement-dynamique-resultats.md
git commit -m "docs(d3): resultats de la campagne, decision d'arrangement, et CAPACITE

La regle de decision etait ecrite AVANT la mesure : elle est appliquee telle
quelle. CAPACITE cesse d'etre le plafond d'encodeurs en processus unique pour
devenir le plafond reellement rencontre en conditions de produit.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Ordre d'exécution et parallélisation

Les tâches **1 à 7** (volet 1) sont séquentielles : chacune s'appuie sur la
précédente. Les tâches **8 à 10** (volet 2) sont séquentielles entre elles, mais
**indépendantes du volet 1** — elles ne touchent aucun fichier commun et peuvent
être menées en parallèle.

Les tâches **11 et 12 exigent la VM, et la VM n'accepte qu'une tâche à la
fois** : elles sont donc strictement sérialisées entre elles, jamais lancées en
parallèle. Elles compilent toutes deux l'agent entier, donc elles viennent
**après les tâches 7 et 10**, quelles qu'aient été leurs parallélisations. Leur
ordre relatif est libre ; enchaîner 11 puis 12 évite une seconde compilation.

La tâche **13 clôt le sous-bloc** et dépend de tout.

**Récapitulatif des dépendances :**

```
1 → 2 → 3 → 4 → 5 → 6 → 7 ─┐
                            ├→ 11 → 12 → 13
8 → 9 → 10 ────────────────┘
```
