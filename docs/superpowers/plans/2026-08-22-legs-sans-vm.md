# Lot 2 — les huit legs sans VM : plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fermer les huit legs du dépôt qui ne demandent ni la VM Windows, ni un œil, ni une oreille, ni un arbitrage du propriétaire.

**Architecture:** Huit items indépendants, quatre sous-projets, deux langages. Ils n'ont en commun **que** leur condition de vérifiabilité. Deux contraintes d'ordre seulement : l'extraction de `tokens.css` précède la déclaration du token, et la file bornée du capteur passe en tête parce qu'elle est la plus lourde.

**Tech Stack:** Rust (agent), TypeScript/Node/Vitest (plateforme, client), `tsc --noEmit`.

**Spec:** [`docs/superpowers/specs/2026-08-22-legs-sans-vm-design.md`](../specs/2026-08-22-legs-sans-vm-design.md)
**Cadrage :** [`docs/superpowers/specs/2026-08-21-finalisation-cadrage.md`](../specs/2026-08-21-finalisation-cadrage.md)

## Global Constraints

- **Français partout** : symboles, commentaires, messages d'erreur, messages de commit. Les identifiants techniques gardent leur forme.
- **500 lignes maximum** par fichier de code source. 🔴 **Relever par `wc -l`, JAMAIS recopier.** Relevé du 22 août 2026, **à relancer** : `agent/src/capteur/sommeil/registre.rs` **456**, `agent/src/capteur/sommeil.rs` **375**, `client/src/design/tokens.css` **300/300**.
- **Extraire, jamais comprimer**, et dans une tâche **dédiée, AVANT** celle qui ajoute.
- **Les commandes de test, et il en faut plusieurs** :
  `cargo test --workspace` (agent + proto) ·
  `cargo check --target x86_64-pc-windows-gnu` (le code `#[cfg(windows)]`, **sur l'hôte**) ·
  `cd plateforme && npm run test:sqlite` **et** `npm run test:postgres` **et** `npm run typecheck` ·
  `cd client && npx vitest run` **et** `npx vitest run --dir ../proto` — 🔴 **DEUX commandes, jamais une** : la racine Vitest est `client/`, `proto/ts/` n'est pas couvert sans la seconde.
- 🔴 **Avant toute compilation qui touche `proto/`** : `cargo clean --release -p proto -p agent` — **les DEUX crates**.
- 🔴 **Un contrôle qu'on n'a jamais vu rouge n'est pas un contrôle.** Chaque garde se mute et se voit rouge. **Copie nommée** (`cp`) avant de muter, restauration **depuis elle** — jamais `git checkout --`, qui restaure HEAD et non l'état d'avant.
- 🔴 **Jamais `git add -A`.** Nommer les fichiers. ⚠️ Un pathspec de **répertoire** ne prend pas le module **homonyme**.
- Commencer chaque commande par `unset -f chpwd 2>/dev/null;` — le shell hôte injecte un `ls` dans toute sortie dès qu'un `cd` court dans un sous-shell.
- Message de commit long **par un fichier** (`git commit -F`), jamais `-m` avec des accents graves ou des backticks.

---

### Task 1: La file bornée à coalescence — la RÈGLE, pure

**Files:**
- Create: `agent/src/capteur/sommeil/file.rs`
- Test: dans le même fichier (`#[cfg(test)] mod tests`), convention du dépôt
- Modify: `agent/src/capteur/sommeil.rs` (déclarer `mod file;`)

**Interfaces:**
- Produces: `pub(crate) const PROFONDEUR_MAX: usize`, `pub(crate) enum Depot { Empilee, Coalescee, Refusee }`, `pub(crate) fn deposer(file: &mut VecDeque<Message>, message: Message) -> Depot`, `pub(crate) fn coalescable(m: &Message) -> bool`

⚠️ **Ce module est PUR** : `VecDeque` et `Message`, rien d'autre. Aucun verrou, aucun `cfg`, aucune API Windows. **C'est ce qui le rend éprouvable sur l'hôte par `cargo test --workspace`.** Si tu crois avoir besoin d'un `Mutex` ici, c'est la tâche 2.

- [ ] **Step 1: Relever l'état, et ne rien supposer**

```bash
unset -f chpwd 2>/dev/null; cd /home/mallanic/Projects/Guacamole
wc -l agent/src/capteur/sommeil.rs agent/src/capteur/sommeil/registre.rs
sed -n '180,215p' agent/src/capteur/sommeil.rs      # l'enum Message et ses variantes
```

Consigner les variantes **réellement** présentes. Le relevé du 22 août en donne **quatre** — `Sommeil(Ordre)`, `Part { bps }`, `Audio { actif }`, `PressePapier { texte, octets }` — **mais un cinquième sous-bloc a pu en ajouter une**. La politique doit couvrir **toutes** celles qui existent.

- [ ] **Step 2: Écrire le test qui échoue**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::capteur::sommeil::{Message, Ordre};
    use std::collections::VecDeque;

    /// 🔴 LE CŒUR DE LA DÉCISION : deux `Part` sans lecture n'en laissent
    /// qu'UNE, et c'est la DERNIÈRE valeur qui survit.
    #[test]
    fn deux_parts_se_coalescent_en_une_seule() {
        let mut f = VecDeque::new();
        assert!(matches!(deposer(&mut f, Message::Part { bps: 1 }), Depot::Empilee));
        assert!(matches!(deposer(&mut f, Message::Part { bps: 2 }), Depot::Coalescee));
        assert_eq!(f.len(), 1);
        assert!(matches!(f[0], Message::Part { bps: 2 }));
    }

    /// 🔴 LE CRITÈRE QUI DISTINGUE LA COALESCENCE EN PLACE DE CELLE EN QUEUE,
    /// et c'est l'invariant que `sommeil.rs` écrit : au sein d'une session, le
    /// canal garantit l'ORDRE DE LIVRAISON entre les variantes. Coalescer en
    /// queue ferait franchir à la part un ordre de dormir déposé entre-temps.
    #[test]
    fn la_coalescence_conserve_la_position() {
        let mut f = VecDeque::new();
        deposer(&mut f, Message::Part { bps: 1 });
        deposer(&mut f, Message::Sommeil(Ordre::Reveiller));
        deposer(&mut f, Message::Part { bps: 2 });
        assert_eq!(f.len(), 2);
        assert!(matches!(f[0], Message::Part { bps: 2 }), "la part garde sa PLACE");
        assert!(matches!(f[1], Message::Sommeil(Ordre::Reveiller)));
    }

    /// Un ordre perdu laisse une fenêtre endormie ou éveillée à tort.
    #[test]
    fn un_ordre_de_sommeil_n_est_jamais_coalesce() {
        let mut f = VecDeque::new();
        deposer(&mut f, Message::Sommeil(Ordre::Reveiller));
        assert!(matches!(deposer(&mut f, Message::Sommeil(Ordre::Reveiller)), Depot::Empilee));
        assert_eq!(f.len(), 2);
    }

    /// Un presse-papier perdu, c'est la donnée de l'utilisateur.
    #[test]
    fn un_presse_papier_n_est_jamais_coalesce() {
        let mut f = VecDeque::new();
        deposer(&mut f, Message::PressePapier { texte: Some("a".into()), octets: 1 });
        let d = deposer(&mut f, Message::PressePapier { texte: Some("b".into()), octets: 1 });
        assert!(matches!(d, Depot::Empilee));
        assert_eq!(f.len(), 2);
    }

    /// La borne dure REFUSE, elle ne tronque pas en silence.
    #[test]
    fn au_dela_de_la_borne_le_depot_est_refuse() {
        let mut f = VecDeque::new();
        for _ in 0..PROFONDEUR_MAX {
            deposer(&mut f, Message::Sommeil(Ordre::Reveiller));
        }
        assert!(matches!(deposer(&mut f, Message::Sommeil(Ordre::Reveiller)), Depot::Refusee));
        assert_eq!(f.len(), PROFONDEUR_MAX, "la file n'a pas grossi");
    }

    /// 🔴 LE TÉMOIN NÉGATIF : une variante coalescable ne bute JAMAIS sur la
    /// borne, quelle que soit la cadence. Sans lui, « refusée » au-dessus ne
    /// dirait pas que la coalescence borne réellement.
    #[test]
    fn une_variante_coalescable_ne_bute_jamais_sur_la_borne() {
        let mut f = VecDeque::new();
        for i in 0..(PROFONDEUR_MAX * 10) {
            let d = deposer(&mut f, Message::Part { bps: i as u32 });
            assert!(!matches!(d, Depot::Refusee));
        }
        assert_eq!(f.len(), 1);
    }
}
```

- [ ] **Step 3: Exécuter et vérifier l'échec**

```bash
unset -f chpwd 2>/dev/null; cargo test --workspace 2>&1 | tail -30
```

Attendu : FAIL à la compilation — `deposer` n'existe pas. **Lire le message.**

- [ ] **Step 4: Écrire l'implémentation**

```rust
//! La file des messages d'une session, BORNÉE PAR COALESCENCE.
//!
//! 🔴 POURQUOI CE MODULE EXISTE. Le canal du registre était un
//! `std::sync::mpsc::channel()` NON BORNÉ, et quatre sous-blocs y ont ajouté
//! chacun une variante. Trois d'entre elles sont poussées « au changement
//! seulement » et n'ont de valeur que dans leur DERNIÈRE occurrence : sous
//! coalescence, elles cessent de faire croître la file en régime permanent,
//! quelle que soit la cadence d'arrivée.
//!
//! 🔴 LA COALESCENCE CONSERVE LA POSITION, ET C'EST L'INVARIANT DU CANAL.
//! `sommeil.rs` écrit qu'au sein d'une session, un canal unique garantit
//! l'ORDRE DE LIVRAISON entre les variantes. Remplacer en place préserve cet
//! ordre ; déplacer en queue ferait franchir à une part de débit un ordre de
//! dormir déposé entre-temps, et livrerait la part APRÈS l'ordre qui aurait dû
//! la rendre caduque.
//!
//! ⚠️ SÉPARER LES VARIANTES EN CANAUX DISTINCTS DÉTRUIRAIT CET INVARIANT.
//! C'est la solution qui vient d'abord à l'esprit, et elle est fausse.
//!
//! ⚠️ CE MODULE EST PUR : il ne connaît ni verrou, ni fil, ni Windows. Le
//! verrou et le réveil vivent chez son appelant.

use std::collections::VecDeque;

use super::Message;

/// La profondeur au-delà de laquelle un dépôt est REFUSÉ.
///
/// ⚠️ **NON CALIBRÉE.** Aucune constante de ce dépôt ne l'est. Elle est choisie
/// assez grande pour qu'un régime normal ne l'atteigne jamais — les variantes
/// coalescables n'y contribuent pas — et assez petite pour que la mémoire reste
/// bornée si un enfant cesse de lire.
pub(crate) const PROFONDEUR_MAX: usize = 64;

/// Ce qu'un dépôt a fait.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Depot {
    /// Ajouté en queue.
    Empilee,
    /// A remplacé, EN PLACE, un message de la même variante déjà en attente.
    Coalescee,
    /// La file était pleine. **Rien n'a été ajouté, rien n'a été retiré.**
    Refusee,
}

/// Cette variante peut-elle remplacer une occurrence en attente d'elle-même ?
///
/// 🔴 LA RÈGLE EST « SA PERTE COÛTE-T-ELLE QUELQUE CHOSE ? », PAS « EST-ELLE
/// FRÉQUENTE ? ». `Part` et `Audio` sont poussées au changement seulement et
/// n'ont de valeur que dans leur dernière occurrence. `Sommeil` porte un ORDRE,
/// `PressePapier` porte la DONNÉE DE L'UTILISATEUR : ni l'un ni l'autre ne se
/// remplace.
///
/// ⚠️ TOUTE VARIANTE NEUVE DOIT PASSER ICI, et le `match` est EXHAUSTIF pour
/// que le compilateur l'exige — jamais un `_ => false`, qui la classerait
/// « à conserver » en silence et laisserait la file recroître.
pub(crate) fn coalescable(m: &Message) -> bool {
    match m {
        Message::Part { .. } | Message::Audio { .. } => true,
        Message::Sommeil(_) | Message::PressePapier { .. } => false,
    }
}

/// Deux messages sont-ils de la même variante ?
fn meme_variante(a: &Message, b: &Message) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

/// Dépose un message, en appliquant la politique de sa variante.
pub(crate) fn deposer(file: &mut VecDeque<Message>, message: Message) -> Depot {
    if coalescable(&message) {
        if let Some(place) = file.iter().position(|en_attente| meme_variante(en_attente, &message)) {
            file[place] = message;
            return Depot::Coalescee;
        }
    }
    if file.len() >= PROFONDEUR_MAX {
        return Depot::Refusee;
    }
    file.push_back(message);
    Depot::Empilee
}
```

⚠️ **`coalescable` doit être adapté aux variantes RÉELLEMENT relevées à l'étape 1.** Si une cinquième existe, elle doit y figurer explicitement.

- [ ] **Step 5: Déclarer le module**

Dans `agent/src/capteur/sommeil.rs`, à côté des autres `mod` enfants : `mod file;`.
⚠️ **Relever `wc -l` de `sommeil.rs` après** : il était à 375.

- [ ] **Step 6: Exécuter et vérifier le passage**

```bash
unset -f chpwd 2>/dev/null; cd /home/mallanic/Projects/Guacamole && cargo test --workspace 2>&1 | tail -20
```

Attendu : PASS, **6 tests neufs**.

- [ ] **Step 7: Jouer DEUX rouges, et lire laquelle rougit**

1. Remplacer `file[place] = message;` par `file.push_back(message);` (coalescence en **queue**). Attendu : **`la_coalescence_conserve_la_position` rougit, et elle SEULE** — c'est ce qui prouve que ce test distingue les deux conceptions.
2. Faire rendre `true` à `coalescable` pour `Message::Sommeil(_)`. Attendu : `un_ordre_de_sommeil_n_est_jamais_coalesce` rougit.

⚠️ **Copie nommée avant chaque mutation, restauration depuis elle.** Consigner **quel** test a rougi et **avec quel message**.

- [ ] **Step 8: Commit**

```bash
unset -f chpwd 2>/dev/null; cd /home/mallanic/Projects/Guacamole
git add agent/src/capteur/sommeil/file.rs agent/src/capteur/sommeil.rs
git commit -m 'capteur(file) : la regle de coalescence, PURE — et la position conservee est l invariant du canal'
```

---

### Task 2: Brancher la file dans le registre

**Files:**
- Modify: `agent/src/capteur/sommeil/file.rs` (y ajouter le couple émetteur/receveur)
- Modify: `agent/src/capteur/sommeil/registre.rs` (`HashMap<String, Sender<Message>>` → le type neuf ; `inscrire`)
- Modify: les appelants relevés à l'étape 1

**Interfaces:**
- Consumes: `deposer`, `Depot`, `PROFONDEUR_MAX` (tâche 1)
- Produces: `pub(crate) struct EmetteurSession` avec `fn envoyer(&self, m: Message) -> Result<Depot, ()>` ; `pub(crate) struct ReceveurSession` avec `fn essayer_recevoir(&self) -> Result<Message, VideOuFerme>` et `fn recevoir(&self) -> Result<Message, VideOuFerme>` ; `inscrire(session, pid) -> (ReceveurSession, u64)` — **même forme qu'aujourd'hui**

- [ ] **Step 1: Relever la surface RÉELLEMENT employée, avant d'écrire**

```bash
unset -f chpwd 2>/dev/null; cd /home/mallanic/Projects/Guacamole
grep -rn "Receiver<Message>\|Sender<Message>" agent/src --include='*.rs'
grep -rn "inscrire(" agent/src --include='*.rs'
grep -rn "ordres\.\|\.try_recv()\|\.recv()" agent/src/capteur/fenetre.rs agent/src/capteur/fenetre/transitions.rs
```

**Relevé du 22 août 2026, à confirmer** : la production n'emploie que **`try_recv()`**, dans une boucle de `transitions.rs::appliquer_les_ordres` ; les tests emploient aussi `recv()`. 🔴 **Aucun `recv_timeout` en production — donc aucun blocage n'est requis, et la structure s'en trouve beaucoup réduite.** Si le relevé dit autre chose, **c'est lui qui fait foi**.

- [ ] **Step 2: Écrire les tests qui échouent**

Dans `file.rs`, en plus des six de la tâche 1 :

```rust
    /// Le couple se comporte comme le canal qu'il remplace : ce qu'on dépose
    /// se reçoit, dans l'ordre.
    #[test]
    fn ce_qui_est_depose_se_recoit_dans_l_ordre() {
        let (e, r) = canal_de_session();
        e.envoyer(Message::Sommeil(Ordre::Reveiller)).unwrap();
        e.envoyer(Message::PressePapier { texte: Some("a".into()), octets: 1 }).unwrap();
        assert!(matches!(r.essayer_recevoir(), Ok(Message::Sommeil(_))));
        assert!(matches!(r.essayer_recevoir(), Ok(Message::PressePapier { .. })));
        assert!(r.essayer_recevoir().is_err());
    }

    /// 🔴 LE REFUS EST COMPTÉ. Un refus qui ne se compte pas est un refus
    /// qu'aucune exploitation ne verra jamais.
    #[test]
    fn les_refus_se_comptent() {
        let (e, _r) = canal_de_session();
        for _ in 0..PROFONDEUR_MAX {
            e.envoyer(Message::Sommeil(Ordre::Reveiller)).unwrap();
        }
        assert_eq!(e.refuses(), 0, "aucun refus tant que la borne n'est pas atteinte");
        e.envoyer(Message::Sommeil(Ordre::Reveiller)).unwrap();
        assert_eq!(e.refuses(), 1);
    }

    /// L'émetteur sait que plus personne ne lit.
    #[test]
    fn un_receveur_tombe_ferme_l_emetteur() {
        let (e, r) = canal_de_session();
        drop(r);
        assert!(e.envoyer(Message::Sommeil(Ordre::Reveiller)).is_err());
    }
```

- [ ] **Step 3: Exécuter et vérifier l'échec**

```bash
unset -f chpwd 2>/dev/null; cd /home/mallanic/Projects/Guacamole && cargo test --workspace 2>&1 | tail -20
```

- [ ] **Step 4: Implémenter le couple**

Dans `file.rs`. La forme, à respecter :

```rust
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// L'état partagé d'une session : sa file, et le compte de ses refus.
struct Partage {
    file: Mutex<VecDeque<Message>>,
    refuses: AtomicU64,
}

pub(crate) struct EmetteurSession { partage: Arc<Partage> }
pub(crate) struct ReceveurSession { partage: Arc<Partage> }

/// La file est VIDE, ou plus personne n'écrit.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum VideOuFerme { Vide, Ferme }

pub(crate) fn canal_de_session() -> (EmetteurSession, ReceveurSession) {
    let partage = Arc::new(Partage {
        file: Mutex::new(VecDeque::new()),
        refuses: AtomicU64::new(0),
    });
    (
        EmetteurSession { partage: Arc::clone(&partage) },
        ReceveurSession { partage },
    )
}
```

⚠️ **Comment `envoyer` sait que le receveur est tombé** : `Arc::strong_count`
sur le partage vaut 1 quand le `ReceveurSession` a été laissé choir. **C'est le
seul signal disponible** — il n'y a plus de `mpsc` pour le donner. Le nommer
dans le commentaire, avec la raison.

🔴 **`envoyer` rend `Err` quand le receveur est tombé** — c'est ce que `registre.rs` teste aujourd'hui par `canal.send(...).is_err()` pour retirer une session morte. **Ce comportement doit être préservé à l'identique**, sinon les sessions mortes cessent d'être purgées, en silence.
🔴 **Le compte des refus est porté par l'ÉMETTEUR**, et il est **par session** : c'est ce qui permet de dire *laquelle* déborde.
⚠️ **Aucun `unwrap()` sur le verrou dans le chemin de production** : un verrou empoisonné y tuerait le capteur. Décider et **écrire** ce qu'il advient d'un verrou empoisonné.

- [ ] **Step 5: Journaliser le refus, et le faire une seule fois par palier**

Le refus doit produire une trace `warn!`. ⚠️ **Ne pas tracer à chaque refus** : « ne jamais tracer par paquet dans la boucle de transport » — 18 619 lignes en quelques secondes ont déjà empêché une session de s'établir. **Tracer au franchissement**, ou échantillonner, et **dire lequel dans le commentaire**.

- [ ] **Step 6: Substituer le type dans `registre.rs` et chez les appelants**

⚠️ **Ne pas changer la forme de `inscrire`** : elle rend toujours un couple `(receveur, generation)`.
⚠️ **Muter par un motif ancré sur la SYNTAXE, jamais par une sous-chaîne** : dans un dépôt qui commente ses invariants, la substitution frappe le commentaire avant le code.

- [ ] **Step 7: Toute la suite, plus la vérification Windows**

```bash
unset -f chpwd 2>/dev/null; cd /home/mallanic/Projects/Guacamole
cargo test --workspace 2>&1 | tail -20
cargo check --target x86_64-pc-windows-gnu 2>&1 | tail -20
wc -l agent/src/capteur/sommeil/file.rs agent/src/capteur/sommeil/registre.rs agent/src/capteur/sommeil.rs
```

🔴 **`cargo check --target x86_64-pc-windows-gnu` n'est PAS facultatif** : la moitié du capteur est `#[cfg(windows)]` et ne compile pas autrement. ⚠️ **Vérifier la NATURE des avertissements, jamais leur nombre** — il dérive avec la fraîcheur du build. ⚠️ **Ne jamais compter par `grep -c '^warning'`** : cela compte aussi la ligne de résumé.

- [ ] **Step 8: Jouer la rouge de la purge des sessions mortes**

Faire rendre `Ok` à `envoyer` quand le receveur est tombé, et vérifier que le test de purge de `registre.rs` rougit. **S'il ne rougit pas, c'est un trou de couverture** : le dire, et ajouter le test.

- [ ] **Step 9: Commit**

```bash
unset -f chpwd 2>/dev/null; cd /home/mallanic/Projects/Guacamole
git add agent/src/capteur/sommeil/file.rs agent/src/capteur/sommeil/registre.rs agent/src/capteur/sommeil.rs agent/src/capteur/fenetre.rs agent/src/capteur/fenetre/transitions.rs
git commit -F <fichier de message>
```

⚠️ **Ajouter aussi les autres appelants relevés à l'étape 1** — la liste ci-dessus est celle du 22 août, pas une vérité.

---

### Task 3: Les freins manquants

**Files:**
- Modify: `plateforme/src/securite/frein.ts` (un budget « toute requête »)
- Modify: `plateforme/src/http/routes-vm.ts`, `plateforme/src/http/routes-session.ts`
- Modify: `plateforme/src/signaling/relais.ts` (le relais `/signal`)
- Test: les `*.test.ts` correspondants

**Interfaces:**
- Consumes: `Frein`, `cleAdresse`, `Budget` (existants)
- Produces: `BUDGET_REQUETES: Budget`, `cleRequetes(adresse: string): string`

- [ ] **Step 1: Relever ce qui existe, avant de concevoir**

```bash
unset -f chpwd 2>/dev/null; cd /home/mallanic/Projects/Guacamole/plateforme
sed -n '60,135p' src/securite/frein.ts
grep -rln 'deps\.frein' src/http/routes-*.ts
```

🔴 **`Frein` COMPTE DES ÉCHECS** (`echec`, `succes`). Les trois routes à freiner n'ont **pas de notion d'échec** : leur abus est un volume de requêtes **réussies**. On réutilise la **structure** — fenêtre glissante, plafond d'entrées, éviction — avec un budget distinct.

- [ ] **Step 2: Écrire les tests qui échouent**

Un test par route, plus celui qui compte le plus :

```ts
// 🔴 LES DEUX COMPTES NE PARTAGENT PAS DE CLÉ. Sinon un utilisateur actif
// épuiserait son propre budget d'ÉCHECS d'authentification par de simples
// requêtes réussies — ou l'inverse, et un attaquant s'offrirait des essais de
// mot de passe en faisant du trafic légitime.
it("le budget de requêtes n'entame PAS le budget d'échecs de la même adresse", () => {
    const frein = new Frein();
    for (let i = 0; i < 10; i++) frein.echec([[cleRequetes('10.0.0.1'), BUDGET_REQUETES]], 0);
    expect(frein.consulter([[cleAdresse('10.0.0.1'), BUDGET_ADRESSE]], 0).freine).toBe(false);
});
```

- [ ] **Step 3: Exécuter, vérifier l'échec, implémenter, vérifier le passage**

```bash
unset -f chpwd 2>/dev/null; cd /home/mallanic/Projects/Guacamole/plateforme
npx vitest run src/securite/ src/http/routes-vm.test.ts src/http/routes-session.test.ts
npm run typecheck
```

⚠️ **Les budgets sont NON CALIBRÉS**, et chacun doit le dire sur place.

- [ ] **Step 4: Jouer la rouge de chaque frein**

Neutraliser le frein d'**une** route à la fois, et vérifier que **son** test rougit — et lui seul. Trois mutations, trois relevés.

- [ ] **Step 5: La double passe, puis commit**

```bash
unset -f chpwd 2>/dev/null; cd /home/mallanic/Projects/Guacamole/plateforme
npm run test:sqlite && npm run test:postgres
```

⚠️ **Une rouge de la double passe peut venir du MOTEUR et non du produit** : une base Postgres de test en `recovery mode` a déjà fait échouer un essai. Vérifier `pg_isready` avant de conclure.

---

### Task 4: Le contrat de `SIGNALING_URL`, figé par un test

**Files:**
- Modify: `agent/src/signaling.rs` (le module de test)

- [ ] **Step 1: Lire le test existant, et comprendre pourquoi il ne ferme rien**

```bash
unset -f chpwd 2>/dev/null; cd /home/mallanic/Projects/Guacamole
grep -n "le_canal_agent_n_est_pas_affecte" -A 20 agent/src/signaling.rs
grep -n "url_du_relais\|url_du_canal" agent/src/signaling.rs
```

`le_canal_agent_n_est_pas_affecte` passe une base **PROPRE**, donc n'éprouve pas le cas dangereux — alors que son commentaire prétendait le fermer.

- [ ] **Step 2: Écrire le test qui échoue**

```rust
/// 🔴 `SIGNALING_URL` EST LA BASE DU SERVICE, JAMAIS L'URL DU RELAIS.
/// `url_du_relais` y ajoute `/signal`, `url_du_canal` y ajoute `/agent`.
/// Y écrire `/signal` casserait l'enrôlement — `ws://h:8080/signal/agent` — et
/// AUCUN test ne le disait : celui d'à côté passe une base PROPRE, donc
/// n'éprouve pas ce cas, alors que son commentaire prétendait le fermer.
#[test]
fn une_base_portant_deja_signal_casse_le_canal_agent() {
    // La base JUSTE : `url_du_canal` y ajoute `/agent`.
    assert_eq!(crate::plateforme::url_du_canal("ws://h:8080"), "ws://h:8080/agent");
    // 🔴 LA BASE FAUSSE, celle qu'aucun test n'éprouvait : elle porte déjà le
    // suffixe du relais, et l'enrôlement part alors vers un chemin qui
    // n'existe pas. C'est CE cas que le contrat doit interdire.
    assert_eq!(
        crate::plateforme::url_du_canal("ws://h:8080/signal"),
        "ws://h:8080/signal/agent",
        "si cette égalité tient, le produit accepte une base fausse EN SILENCE"
    );
}
```

**Relevé du 22 août 2026, à confirmer** : le test voisin tient en une ligne —
`assert_eq!(crate::plateforme::url_du_canal("ws://h:8080"), "ws://h:8080/agent");`
(`agent/src/signaling.rs:322-324`) —, `url_du_relais` vit dans `signaling.rs`
et `url_du_canal` dans `plateforme.rs`. **Relancer `grep -n` : ces deux
emplacements sont le genre de fait qui dérive.**

- [ ] **Step 3: Vérifier que le test ROUGIT sur le produit d'aujourd'hui, ou non**

🔴 **C'est l'étape qui décide de la suite, et elle a deux issues légitimes :**
- **s'il rougit** : le produit a un défaut, et le corriger fait partie de la tâche ;
- **s'il passe** : le produit était déjà juste, et ce test **fige** un contrat que rien ne tenait.

**Dire laquelle**, et ne pas la supposer.

- [ ] **Step 4: `cargo test --workspace`, puis commit**

---

### Task 5: L'éviction par âge, avec plancher

**Files:**
- Modify: le magasin d'icônes et le magasin de tranches (`plateforme/src/apps/`)
- Test: les `*.test.ts` correspondants

- [ ] **Step 1: Relever les magasins et ce qui les référence**

```bash
unset -f chpwd 2>/dev/null; cd /home/mallanic/Projects/Guacamole/plateforme
ls src/apps/
grep -rn "ouvrirMagasin\|ouvrirMagasinTranches" src --include='*.ts' | grep -v '\.test\.'
```

- [ ] **Step 2: Écrire les TROIS tests qui échouent**

🔴 **Les trois, et pas seulement le premier** — une éviction qui emporte **tout** passerait le premier :

```ts
// Le temps est INJECTÉ, jamais lu de l'horloge : un test qui attendrait
// réellement l'âge d'éviction serait un test qu'on désactive au premier
// ralentissement de la machine.
const JOUR_MS = 24 * 60 * 60_000;

it('évince un objet vieux et NON référencé', async () => {
    const magasin = magasinJetable();
    await magasin.deposer('orphelin', OCTETS, 0);
    await magasin.evincer({ maintenant: 400 * JOUR_MS, referencees: new Set() });
    expect(await magasin.existe('orphelin')).toBe(false);
});

// 🔴 LE SEUL TEST QUI DISTINGUE UNE ÉVICTION D'UNE CORRUPTION. Sans lui, une
// éviction qui emporte TOUT passerait le test précédent.
it("NE PEUT PAS évincer un objet vieux mais RÉFÉRENCÉ par une entrée vivante", async () => {
    const magasin = magasinJetable();
    await magasin.deposer('en-service', OCTETS, 0);
    await magasin.evincer({ maintenant: 400 * JOUR_MS, referencees: new Set(['en-service']) });
    expect(await magasin.existe('en-service')).toBe(true);
});

it("n'évince pas un objet jeune", async () => {
    const magasin = magasinJetable();
    await magasin.deposer('recent', OCTETS, 0);
    await magasin.evincer({ maintenant: 1 * JOUR_MS, referencees: new Set() });
    expect(await magasin.existe('recent')).toBe(true);
});
```

⚠️ **Les noms `deposer`, `existe`, `evincer` sont ceux que cette tâche doit
POSER — pas ceux qui existent.** L'étape 1 relève la surface réelle du magasin ;
si elle diffère, **c'est elle qui fait foi**, et le test s'y conforme.

- [ ] **Step 3: Implémenter, avec la constante et son avertissement**

```ts
/// ⚠️ **NON CALIBRÉE.** Aucune constante de ce dépôt ne l'est.
///
/// 🔴 LE PLANCHER DE RÉFÉRENCE EST CE QUI DISTINGUE UNE ÉVICTION D'UNE
/// CORRUPTION : évincer une icône encore nommée par une application ferait
/// disparaître son image sans que rien ne le dise.
///
/// ⚠️ CE QUE CETTE RÈGLE NE FAIT PAS : elle ne borne PAS le disque. Un
/// catalogue qui grossit sans cesse grossit sans cesse. Le plafond de taille a
/// été ÉCARTÉ par décision, parce qu'il peut évincer un objet encore référencé
/// — c'est-à-dire échanger une croissance visible contre une panne silencieuse.
```

- [ ] **Step 4: Jouer la rouge du PLANCHER**

Retirer le test de référence, et vérifier que **le deuxième test rougit** — c'est le seul qui distingue une éviction d'une corruption.

- [ ] **Step 5: La double passe, puis commit**

---

### Task 6: L'extraction de `tokens.css` — AVANT la tâche 7

**Files:**
- Modify: `client/src/design/tokens.css`
- Create: le ou les fichiers extraits
- Modify: les lecteurs que l'étape 1 aura **énumérés**

- [ ] **Step 1: ÉNONCER LA RÈGLE DE SÉLECTION, PUIS COMPTER**

🔴 **`CLAUDE.md` annonce « onze lecteurs qui le nomment par son chemin ». Une commande large en rend 33.** Les deux peuvent être vrais de choses différentes — et c'est le problème : **personne n'a énoncé la règle.** *Un sous-ensemble sans règle énoncée est un sous-ensemble choisi, même quand on ne l'a pas choisi.*

**Écrire la règle** (un `@import` CSS en est un ; un import de `tokens.ts` n'en est pas un ; une mention en commentaire non plus), **puis compter avec la commande qui l'applique**, **puis corriger le chiffre de `CLAUDE.md`**.

```bash
unset -f chpwd 2>/dev/null; cd /home/mallanic/Projects/Guacamole
wc -l client/src/design/tokens.css
```

- [ ] **Step 2: Extraire**

⚠️ **Le découpage doit avoir un SENS** — un axe (les couleurs, les longueurs, les durées), pas une coupure à la ligne 150.
⚠️ **Une extraction n'est jamais rigoureusement verbatim** : elle casse les déictiques (« plus bas dans ce fichier »), et **déplace ce qu'un garde d'absence surveille**.

- [ ] **Step 3: Vérifier que les gardes de design voient toujours ce qu'ils voyaient**

```bash
unset -f chpwd 2>/dev/null; cd /home/mallanic/Projects/Guacamole/client
npm run design:verifier
npx vitest run
npx tsc --noEmit
```

🔴 **Puis, depuis la racine, l'autre passe** : `cd client && npx vitest run --dir ../proto`.

- [ ] **Step 4: Relever les tailles APRÈS, et commiter**

---

### Task 7: `--accent-fenetre` — le token, et le garde rendu capable de rougir

**Files:**
- Modify: `client/src/design/tokens.css` (ou le fichier extrait qui le porte)
- Modify: la feuille qui le peint
- Modify: `client/outils/tokens-orphelins.mjs`
- Test: le contrôle du vérificateur

- [ ] **Step 1: Relever l'état**

```bash
unset -f chpwd 2>/dev/null; cd /home/mallanic/Projects/Guacamole
grep -rn "TOKEN_ACCENT\|--accent-fenetre" client/src client/outils --include='*.ts' --include='*.css' --include='*.mjs' | grep -v '\.test\.'
```

`accent-dom.ts` **pose** le token ; **aucune feuille ne le lit** ; il n'est déclaré nulle part.

- [ ] **Step 2: Le déclarer et le faire PEINDRE**

Avec un repli : `var(--accent-fenetre, var(--accent))` — sans quoi il est **indéfini** avant le premier message, et rien n'est peint.

- [ ] **Step 3: 🔴 RENDRE LE GARDE CAPABLE DE ROUGIR — c'est le cœur de la tâche**

Le contrôle §7.6 (`client/outils/tokens-orphelins.mjs`, « aucun token orphelin, aucun `var()` non déclaré ») signale un token non déclaré **employé par un `var()` dans du CSS**. **Un token posé par le JS et employé par aucun CSS lui est structurellement invisible.**

**Étendre le contrôle** pour qu'il voie aussi les tokens **posés par le JS** (`poserToken(...)`, la constante `TOKEN_ACCENT`) et déclarés nulle part.

- [ ] **Step 4: JOUER SA ROUGE**

Retirer la déclaration du token, et vérifier que le contrôle **étendu** rougit — **et qu'il le fait pour la bonne raison**. 🔴 **Sans cette rouge, on aura remplacé un contrôle aveugle par un autre**, ce qui est exactement le défaut qu'on répare.

- [ ] **Step 5: `design:verifier`, les deux passes Vitest, `tsc`, puis commit**

---

### Task 8: Les douze pilotes de recette

**Files:**
- Modify: les pilotes que l'étape 1 aura **énumérés**

- [ ] **Step 1: Énumérer, ne pas croire le chiffre**

```bash
unset -f chpwd 2>/dev/null; cd /home/mallanic/Projects/Guacamole
grep -rln "signaling=" --include='*.mjs' --include='*.js' docs/superpowers/ | sort | cat -n
```

Relevé du 21 août : **douze**. **Relancer.**

- [ ] **Step 2: Réparer, en distinguant les deux cas**

⚠️ **`?signaling=` reste EXPLICITE et ne reçoit PAS le suffixe `/signal`.** Une substitution qui ne distinguerait pas les deux casserait les pilotes autrement. **Lire chaque pilote**, ne pas substituer en masse.

- [ ] **Step 3: Dire ce que cette tâche N'ÉTABLIT PAS**

🔴 **Aucun de ces pilotes n'est rejoué** : ils exigent la VM Windows. Le lot les **répare** ; le lot 3 les rejouera. **Le document de résultats doit le dire.**

- [ ] **Step 4: Commit**

---

### Task 9: Les tests des galeries

**Files:**
- Create: `client/src/design/galerie.test.ts`, `client/src/design/galerie-primitives.test.ts`

- [ ] **Step 1: Lire les deux galeries, et décider ce qu'un test peut établir**

⚠️ **Ce qu'un test de galerie PEUT faire** : figer la liste des cas, leur nommage, l'absence de doublon, la présence de chaque primitive. **Ce qu'il NE PEUT PAS faire** : porter un jugement visuel. Celui-là attend un œil, et c'est le lot 4. **L'écrire dans le fichier de test.**

- [ ] **Step 2: Écrire les tests, jouer leur rouge, commiter**

🔴 **La rouge** : retirer une primitive de la galerie et vérifier que le test la réclame. Un test de galerie qui ne rougirait pas sur une primitive manquante ne servirait à rien.

---

### Task 10: Recette et revue transverse

- [ ] **Step 1: La suite entière, depuis un shell propre**

```bash
unset -f chpwd 2>/dev/null; cd /home/mallanic/Projects/Guacamole
env -u TURN_URL -u TURN_SECRET ./scripts/verify-all.sh
cargo test --workspace
cargo check --target x86_64-pc-windows-gnu
cd client && npx vitest run && npx vitest run --dir ../proto
```

⚠️ Le script compte **dix** étapes et affiche **dix-huit** en-têtes `==>` : **dire lequel on annonce.**

- [ ] **Step 2: Relever les tailles APRÈS la dernière édition**

```bash
unset -f chpwd 2>/dev/null; cd /home/mallanic/Projects/Guacamole
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
```

**Et corriger le tableau de dette de `CLAUDE.md` dans le même mouvement** s'il a changé.

- [ ] **Step 3: Relire les journaux CONTRE les messages de commit**

🔴 *Un message de commit est une pièce du dépôt, et personne ne le relit* — sept écarts sur neuf n'existaient que là. **Un lot antérieur a eu un message qui disait l'inverse de son diff.**

- [ ] **Step 4: Chercher les affirmations devenues fausses, PAR LE SENS**

⚠️ **Le lot précédent en a produit SEPT**, dont une dans le commentaire qui explique comment ne pas en produire. Chercher sur **plusieurs lignes** : une phrase coupée par un retour à la ligne est invisible à tout `grep` d'une ligne, et c'est ainsi qu'un site a survécu à trois recherches.

- [ ] **Step 5: Écrire le document de résultats, ajouter UNE ligne à l'index de `CLAUDE.md`, commiter**

Le document doit porter un § « Ce que ce lot n'établit PAS » disant au minimum : **aucun des douze pilotes n'est rejoué**, **aucun jugement visuel n'est porté**, **aucune constante n'est calibrée**, et **le comportement en charge réelle de la file du capteur relève du lot 3**.
