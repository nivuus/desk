# Sous-projet ① « Divers » — presse-papier, sous-bloc **P1** : la VM copie, le navigateur colle — plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> ⚠️ **AVERTISSEMENT DE NOM, à lire avant toute autre chose.** Ce dépôt porte
> **deux** sous-blocs nommés « P1 », et ce ne sont pas les mêmes :
>
> | Nom court | Ce que c'est | État |
> | --- | --- | --- |
> | **P1 (plateforme)** — sous-projet ⑤ | « le service naît, absorbe le signaling, et persiste » | **livré et clos**, `CLAUDE.md:7110` |
> | **P1 (presse-papier)** — sous-projet ① | ce document | **à faire** |
>
> **Dans tout ce document, « P1 » désigne le presse-papier**, jamais la
> plateforme. Les autres sous-blocs du sous-projet ① sont **P2** (le navigateur
> colle dans la VM), **P3** (les N fenêtres) et **A1** (la couleur d'accent) —
> et P2/P3 du presse-papier ne sont pas non plus les P2/P3 de la plateforme.
> Les messages de commit de ce sous-bloc portent le préfixe
> `plan(presse-papier)` / `presse-papier(p1)`, jamais `p1` nu.

**Goal:** faire arriver dans le presse-papier de la machine locale ce que
l'utilisateur copie dans une application de la VM, par un chemin qui ne demande
**aucune permission** au navigateur et ne touche **pas** au clavier de la
fenêtre de session.

**Architecture:** un propriétaire unique, un compteur sondé, un message d'état.

1. **Le capteur, et lui seul, observe le presse-papier Windows** (spec D1). Il
   sonde `GetClipboardSequenceNumber()` sur son tour de roue, n'ouvre le
   presse-papier que lorsque le compteur a bougé, et pousse le texte à chaque
   fenêtre par le canal `mpsc` déjà existant.
2. **Le trajet est celui, éprouvé, de `Sommeil` / `Part` / `Audio` /
   `PleinEcran`** : `Message` → `DepuisCapteur::PressePapier` → connexion
   média → `pont_media.rs` → `Recu::PressePapier` → `SourceDistante` →
   branche `a1…` de `tick.rs` → `AgentControl::Clipboard`.
3. **Le navigateur appelle `navigator.clipboard.writeText`**, qui ne demande
   aucune permission (relevé R1 de la spec, §3.1). Toute la décision vit dans
   un module **pur, sans DOM**.

**Tech Stack:** inchangée. **P1 N'AJOUTE AUCUNE DÉPENDANCE**, ni Rust ni
TypeScript : `GetClipboardSequenceNumber`, `OpenClipboard`, `GetClipboardData`
et `GlobalLock` vivent tous dans la caisse `windows` déjà présente, et le
client n'emploie que `navigator.clipboard`, natif.

**Spec :** `docs/superpowers/specs/2026-08-19-presse-papier-design.md`
(commit `a6b1b64`, 1063 lignes), §6 « P1 » et tout ce dont il dépend — §2
(points de passage), §3 (relevés R1/R2), D1, D2, D4, D5 (garde n°2 seul), D7,
D8, §7 (pureté et plafond de lignes), §8, §10, §11.

**Ce plan ne couvre QUE P1 (presse-papier).** Explicitement hors périmètre, et
rien de ce qui suit n'est écrit par ce sous-bloc :

- **P2** — l'exception étroite de `client/src/input.ts`, l'écouteur `paste`,
  `ClientControl::Clipboard`, l'écriture du presse-papier Windows, l'injection
  de `Ctrl+V` (D6), et **les trois gardes anti-écho de D5**. P1 n'écrit
  **jamais** le presse-papier Windows : la boucle qu'ils ferment ne peut pas
  exister ici. Seul le **garde n°2** (égalité de contenu) est livré, parce
  qu'il sert à autre chose — absorber le faux positif du compteur (D2).
- **P3** — la règle « toutes reçoivent, seule la focalisée écrit localement »
  mesurée à N fenêtres, et le critère qui **réfuterait** la contrainte supposée
  du §3.3 (`writeText` depuis une fenêtre non focalisée).
- **A1** — l'icône, la couleur dominante, `AgentControl::Accent`,
  `--accent-fenetre`, `client/src/accent.ts`, `agent/src/accent.rs`. **Aucun
  fichier d'A1 n'est créé par P1**, pas même vide.
- **Le propriétaire mono-fenêtre.** Voir E6 : P1 livre le propriétaire
  **capteur** seul, et le déclare.

---

## Contraintes globales

### Ce qui a été relevé PAR LA COMMANDE avant d'écrire une ligne

Toutes les valeurs ci-dessous ont été obtenues le **19 août 2026**, sur cette
machine, **en lançant réellement la commande**. Aucune n'est recopiée d'un
document antérieur. Tout ce qui n'a pas été mesuré est marqué comme tel
partout ailleurs dans ce plan.

| # | Commande | Résultat relevé |
| --- | --- | --- |
| 1 | `cargo test -p agent` | **550 passed, 0 failed** |
| 2 | `cargo test -p proto` | **52 passed, 0 failed** |
| 3 | `cd client && npx vitest run` | **187 passed**, 21 fichiers |
| 4 | `cd proto && npx vitest run` | **70 passed**, 3 fichiers — dont `ts/control.test.ts` : **17** |
| 5 | `cd agent && cargo check --target x86_64-pc-windows-gnu` | **sortie 0**, **11 avertissements** (tous `dead_code`) |
| 6 | la commande des 500 lignes de `CLAUDE.md` | voir le tableau ci-dessous |

**Ces cinq comptes sont les comptes de DÉPART.** Chaque tâche qui touche du
code annonce le compte qu'elle attend **avant** de le lire — c'est ce qui a
permis à D10 d'attraper un test supprimé par un `Write` maladroit.

### La règle des 500 lignes : UNE extraction est requise, et elle précède son addition

Relevé par la commande, le 19 août 2026 :

```
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>400'
```

| Fichier touché par P1 | Lignes | Marge | Ce que P1 y met |
| --- | --- | --- | --- |
| `proto/src/control.rs` | **470** | **30** | 🔴 variante + constructeur + tests ⟹ **franchira 500**. **Extraction en tâche 3, AVANT l'addition de la tâche 6** |
| `agent/src/capteur/distante/tests.rs` | **474** | **26** | ⚠️ **marge que la spec §7.2 NE LISTE PAS** (E1). Le test de `Recu::PressePapier` s'y ajoute — **porte de mesure obligatoire** en tâche 10 |
| `client/src/main.ts` | **451** | **49** | +1 branche `onControl` + câblage. **Porte de mesure** en tâche 15 |
| `agent/src/capteur/protocole.rs` | **419** | 81 | +1 variante `DepuisCapteur` + doc + tests |
| `agent/src/capteur/distante.rs` | **409** | 91 | `Recu`, 1 champ, 1 méthode |
| `agent/src/transport/tick.rs` | **403** | 97 | +1 branche `a1septies` |
| `agent/src/source.rs` | **393** | 107 | +1 méthode de trait, à défaut inerte |
| `agent/src/capteur/sommeil/registre.rs` | **346** | 154 | le sondage sur le tour de roue |
| `agent/src/capteur/sommeil.rs` | **328** | 172 | +1 variante de `Message` |
| `agent/src/capteur/pont_media.rs` | **263** | 237 | +1 bras, +1 test |
| `agent/src/transport/controle.rs` | **225** | 275 | +1 bras du `match` de journal |
| `agent/src/capteur/fenetre/transitions.rs` | **193** | 307 | +1 bras de relais |
| `proto/ts/control.ts` | **139** | 361 | +1 interface, l'union, `TYPES_AGENT` **dérivé** |
| `scripts/run-agent.sh` | **130** | 370 | 2 lignes |
| `agent/src/diagnostics.rs` | **128** | 372 | +1 bras d'aiguillage |
| `agent/src/capteur.rs` | **36** | 464 | — (P1 n'y touche pas : voir E7) |

**Fichiers neufs** : `agent/src/presse_papier.rs`,
`agent/src/presse_papier/win32.rs`, `agent/src/diagnostics/presse_papier.rs`,
`agent/src/capteur/sommeil/presse_papier.rs`, `client/src/presse-papier.ts`,
`client/src/presse-papier.test.ts`, `proto/src/control/tests.rs`. **Aucun ne
doit naître au-dessus de 500 lignes**, et aucun n'en approche.

⚠️ **Aucun fichier de la dette gelée n'est touché** : ni `encode.rs` (1536),
ni `windows_source.rs` (630), ni `encode/arret.rs` (500, marge 0), ni
`capture.rs` (492). Ni `agent/src/transport.rs` (**495**, marge 5) — **et il
faut le dire, parce que c'est la marge la plus serrée d'`agent/src` après
`arret.rs`**, et que P1 passe par `transport/tick.rs` et
`transport/controle.rs`, ses voisins. **Si une addition dérivait vers
`transport.rs`, elle exigerait une extraction préalable.**

⚠️ **Deux chiffres de `CLAUDE.md` ont DÉRIVÉ, et ce plan ne les recopie pas** —
il ne les corrige pas non plus, `CLAUDE.md` étant sous périmètre concurrent
(voir plus bas) :

- `agent/src/transport.rs` : `CLAUDE.md:723` publie **491** (relevé de clôture
  du chantier E) ; la commande rend **495**.
- `client/verify-webrtc.mjs` : `CLAUDE.md:598` publie **497** ; la commande
  rend **494**. La spec de ce chantier l'avait déjà relevé (§7.2).

**La tâche 18 (mise à jour de `CLAUDE.md`) remesure tout par la commande** et
corrige ces deux nombres à leur place, en énumérant les places par
`grep -n` **avant** d'écrire — c'est le geste que ce dépôt a payé **neuf** fois.

### Les règles de méthode, héritées et non négociables

1. **Un contrôle doit pouvoir échouer.** Chaque test de ce plan porte sa
   colonne « ce qui le rend ROUGE », et la rouge est **jouée**, pas seulement
   écrite. Ce dépôt a payé F1 (D7) trois fois, et D10 a attrapé **quatre**
   contrôles vacueux dont **trois étaient écrits par le plan lui-même**.
2. **Ne jamais fabriquer une sortie de commande.** D10 a trouvé deux pièces
   assemblées à la main et présentées comme des relevés. La conclusion était
   vraie ; la preuve ne l'était pas, donc invérifiable.
3. **Une extraction précède son addition, et n'est jamais une compression.**
   D9 a payé deux compressions ; D10 a joué trois extractions préalables et
   n'en a payé aucune.
4. **Une extraction est VERBATIM**, comparée mot pour mot, et **ne change
   aucun compte de tests**.
5. **`git add` nominatif, jamais `git add -A`.** Un chantier concurrent vit
   dans cet arbre.
6. **Jamais de `git commit --amend`.**
7. **Toute variable d'environnement neuve est ajoutée à
   `scripts/run-agent.sh` dans la tâche qui l'introduit.** Payé en D1
   (`SUPERVISEUR`), D2 (`MULTIFENETRE_REPRISE`) et D7 (`AUDIO`) : l'agent
   démarre sans elle **et ne le dit pas**.
8. **Le compte de tests attendu est annoncé AVANT d'être lu.**
9. **La preuve d'une affirmation ne vit jamais dans un rapport gitignoré.**
   L'espace de travail de D9 a disparu avec six constats de revue. Tout ce qui
   compte va dans le document de résultats (tâche 19) et dans
   `docs/superpowers/plans/journaux-presse-papier-p1/`, **versés dans git**.

### Périmètre concurrent — à lire avant de toucher quoi que ce soit

Un agent concurrent clôt le sous-bloc **P3 de la plateforme** et édite
`CLAUDE.md`, `proto/`, `client/`, `agent/`, `scripts/`. **Relancer
`git log --oneline -5 -- <paquet>` avant de démarrer toute famille qui touche
`proto/` ou `client/`.** Les tâches 3, 6, 7 et 15 sont les plus exposées.

⚠️ **La tâche 3 (extraction de `proto/src/control.rs`) et la tâche 21 du plan
P3 de la plateforme se croisent** : `docs/superpowers/plans/2026-08-19-plateforme-p3.md:650`
(« E14 — `proto/src/control.rs` est à 470 lignes, marge 30, et P3 n'y touche
pas ») dit que P3 **n'y touche pas**. Vérifié à l'écriture de ce plan ;
**à revérifier avant la tâche 3**, parce qu'un énoncé daté vieillit.

---

## Décisions tranchées — ce que la spec laissait ouvert et que ce plan ferme

### D-P1-1 — La forme du REFUS au-delà de `PRESSE_PAPIER_MAX` : un `Option<String>` et un compte d'octets

**La spec ne tranche pas la forme du refus.** Elle tranche le comportement
(§D4) — « au-delà de la borne, on REFUSE — on ne tronque pas », « le refus est
**dit** : un bandeau de statut persistant côté client, et un `warn!` côté
agent » — mais elle ne dit **pas** par quel message le client apprend qu'un
refus a eu lieu. Sans message, le bandeau est impossible.

**Décision :**

```rust
/// Le presse-papier de la VM a changé.
///
/// `text` vaut `None` quand le contenu dépasse `PRESSE_PAPIER_MAX` : il est
/// REFUSÉ, jamais tronqué — un collage silencieusement amputé est le pire
/// résultat possible, et il est pire que pas de collage du tout. `bytes`
/// porte alors la taille refusée, en octets d'UTF-8, pour que le bandeau
/// puisse la dire ; il porte la taille émise dans le cas normal.
Clipboard {
    #[serde(rename = "v", deserialize_with = "verifie_version")]
    version: u8,
    text: Option<String>,
    bytes: u32,
},
```

Côté TypeScript : `{ v, type: 'clipboard', text: string | null, bytes: number }`.

**Pourquoi cette forme et pas deux variantes** : le précédent du dépôt est
`Asleep { asleep, reason }` (`proto/src/control.rs:161-167` — un état plus sa
raison) et `Link { … quality, adaptation }` (`:200-208`) ; le dépôt n'a aucun
précédent de **deux variantes pour un seul état**. `bytes` n'est pas redondant
quand `text` est `Some` : il est la taille exacte du texte **émis**, ce qui
rend le message auto-descriptif au journal.

⚠️ **`bytes` est la taille APRÈS normalisation des fins de ligne**, jamais
avant — voir D-P1-2.

### D-P1-2 — L'ordre est : lire UTF-16 → convertir en UTF-8 → normaliser `\r\n` → borner. Et pas un autre.

**La spec dit qu'il faut normaliser et borner ; elle ne dit pas dans quel
ordre**, et l'ordre change le résultat : un texte Windows de 65 000 lignes de
`\r\n` perd 65 000 octets à la normalisation. Borner d'abord refuserait un
texte qui, une fois normalisé, tiendrait.

**Décision : normaliser d'abord, borner ensuite.** La borne porte sur ce qu'on
**émet**, jamais sur ce qu'on a lu. C'est aussi ce qui rend `bytes` honnête.

⚠️ **La conversion UTF-16 → UTF-8 emploie `String::from_utf16_lossy`.** Un
substitut isolé dans le presse-papier (possible : `CF_UNICODETEXT` n'est pas
validé par Windows) ne doit **pas** faire échouer la lecture ni tuer un fil.

### D-P1-3 — Le sondage lit HORS du verrou du registre, et distribue DANS

C'est **un défaut que le plan corrige avant qu'il n'existe** — voir E3. La
spec place le sondage « sur le tour de roue du registre »
(`agent/src/capteur/sommeil/registre.rs`), et le tour de roue prend le verrou
**global** du capteur en tête de tour (`registre.rs:157`,
`let mut garde = etat();`) puis appelle ses quatre distributeurs sous ce
verrou. Une E/S Win32 sur le presse-papier placée là bloquerait `inscrire`,
`retirer`, `signaler` et `echec_de_reveil` de **toutes** les fenêtres.

**Décision, avec sa forme exacte :**

```rust
fn demarrer_le_tour_de_roue() {
    std::thread::spawn(|| {
        let mut sondeur = crate::presse_papier::Sondeur::nouveau();
        loop {
            std::thread::sleep(PERIODE_REARBITRAGE);
            // HORS VERROU : lecture Win32, bornée par PERIODE_PRESSE_PAPIER.
            let annonce = sondeur.tour();
            let mut garde = etat();
            …  // les quatre distributeurs existants, inchangés
            if let Some(annonce) = annonce {
                presse_papier::distribuer(&mut garde, annonce);
            }
        }
    });
}
```

### D-P1-4 — L'état de référence est pris au PREMIER sondage, et rien n'est émis pour lui

Patron §2.3 point 4 de la spec, celui de `SuiviBordure`
(`agent/src/capteur/plein_ecran.rs:82-84`) : **l'état lu à l'attache fait
référence**. Le premier tour mémorise le numéro de séquence courant et **n'émet
rien**. Une fenêtre qui s'attache ne reçoit donc pas le contenu déjà présent
dans le presse-papier ; elle reçoit la première copie **qui suit**.

**Conséquence à ne pas dissimuler** : le critère ① de la recette doit copier
**après** que la session est établie, sans quoi il mesurerait zéro sur un
produit correct. Voir aussi E5, qui est le même fait vu de P3.

### D-P1-5 — Un échec d'ouverture du presse-papier N'AVANCE PAS la référence

Le cas R7 de la spec — `OpenClipboard` échoue parce qu'une autre application
le tient — est **normal sous Windows**, pas une panne. Mais si le sondeur
mémorisait le nouveau numéro de séquence **avant** d'avoir réussi à lire, le
contenu correspondant serait **perdu à jamais** : le tour suivant verrait un
compteur « inchangé » et ne retenterait rien.

**Décision : la référence n'avance qu'après une lecture RÉUSSIE.** Un échec
laisse la référence en place, ce qui fait retenter au tour suivant, sans
boucle d'attente et sans compteur d'attente supplémentaire. Le nombre
d'échecs consécutifs est journalisé une fois par palier, jamais à chaque tour.

### D-P1-6 — `Capabilities.clipboard` est REPORTÉ à P2, et ce n'est pas un oubli

Voir E6. `Capabilities` est émis par l'**enfant**
(`agent/src/demarrage.rs:291`, `:363`, `:383`), qui ne lit pas
`PRESSE_PAPIER` — la spec place cette lecture dans le **propriétaire**, donc
dans le capteur (D8). L'enfant ne peut pas annoncer honnêtement la capacité au
moment où il émet ce message. Et P1 n'en a **aucun usage** : le client se
contente de recevoir un `Clipboard` et de l'écrire. **C'est P2 qui en a
besoin** — c'est là que le client doit décider s'il intercepte `Ctrl+V`.

### D-P1-7 — La sonde d'ouverture écrit elle-même le presse-papier, et cela est déclaré

Voir tâche 1. La sonde répond à quatre questions, dont deux exigent qu'elle
**écrive** le presse-papier de la VM (Q3 et Q4). Cela détruit le contenu
courant du presse-papier de la VM, et cela mesure gratuitement, dès P1, deux
faits dont **P2** dépend entièrement (garde n°1 de D5).

⚠️ **Le PRODUIT, lui, n'écrit jamais le presse-papier en P1.** L'écriture est
un geste de sonde, pas de produit. Ne pas confondre les deux.

⚠️ **La sonde ne journalise JAMAIS le texte du presse-papier** — une empreinte
tronquée et une longueur, jamais le contenu. C'est une ressource privée, et un
journal versé dans git est public au dépôt.

---

## Divergences entre la spec et le code réel, relevées à l'écriture de ce plan

Toutes ont été vérifiées **dans l'arbre courant**, `git log -1` = `e77fc0e`.
Chaque citation a été relue après avoir été écrite.

### E1 — 🔴 `agent/src/capteur/distante/tests.rs` est à 474 lignes, marge 26, et la spec §7.2 NE LE LISTE PAS

Le §7.2 de la spec budgète douze fichiers ; celui-ci n'y est pas. Or c'est là
que vit le test de `Recu::PleinEcran`
(`agent/src/capteur/distante/tests.rs:470` — **le dernier test du fichier**),
donc le voisin naturel du test de `Recu::PressePapier`.

**Ce que ce plan prescrit** : la tâche 10 mesure **avant** et **après**. Si le
compte franchit 460, l'extraction est jouée **dans la même tâche**, et son
point de chute est nommé : `agent/src/capteur/distante/tests_etats.rs`,
déclaré par `#[path]` `#[cfg(test)]` depuis `distante.rs` — le mécanisme est
déjà employé par `agent/src/superviseur/table.rs`, et la « Convention de module
enfant » de `CLAUDE.md` range explicitement cet usage **hors de sa portée**.

### E2 — La sonde d'ouverture du §8 n'a AUCUNE forme dans la spec, et ce plan l'invente

Le §8 dit « **Le premier geste de P1 est de le mesurer** », et le critère ② dit
quoi faire si le compteur ne se comporte pas comme annoncé. Mais rien, dans la
spec, ne dit **comment** mesurer, ni quelles questions, ni ce qui vaut verdict
défavorable. La tâche 1 de ce plan écrit cette forme, la tâche 2 la joue, et
le §« Verdicts » de la tâche 2 énumère **cinq** issues dont **trois rendent le
sous-bloc non livrable en l'état**.

### E3 — 🔴 Le tour de roue tient le verrou GLOBAL du registre, et la spec y place une E/S Win32

`agent/src/capteur/sommeil/registre.rs:154-164` :

```rust
fn demarrer_le_tour_de_roue() {
    std::thread::spawn(|| loop {
        std::thread::sleep(PERIODE_REARBITRAGE);
        let mut garde = etat();
        …
    });
}
```

`etat()` rend un `MutexGuard<'static, Etat>` sur l'unique `OnceLock<Mutex<Etat>>`
du capteur (`registre.rs:121-144`), que prennent aussi `inscrire`
(`:278-329`), `retirer` (`:330…`), `signaler` (`:190…`) et `echec_de_reveil`.
Un `OpenClipboard` / `GetClipboardData` posé sous ce verrou bloquerait
l'attache et le retrait de **toutes** les fenêtres pendant tout le temps de la
contention Win32. **La spec ne le mentionne pas.** Remède : D-P1-3.

### E4 — Le tour de roue ne démarre qu'à la PREMIÈRE inscription

`demarrer_le_tour_de_roue` est lancé **depuis la fermeture d'initialisation de
`ETAT.get_or_init`** (`registre.rs:146-153`, le commentaire le dit
explicitement). Il n'existe donc aucun sondage du presse-papier tant qu'aucune
fenêtre n'est attachée au capteur.

**Ce n'est pas un défaut** — il n'y a personne à qui envoyer —, mais c'est ce
qui rend D-P1-4 nécessaire et c'est un fait de recette : la sonde de la tâche 2
tourne **hors** de ce chemin, dans un processus dédié, et ne mesure donc pas la
même chose que le produit.

### E5 — Une fenêtre attachée APRÈS une copie ne reçoit jamais ce contenu

Le garde d'égalité de contenu (garde n°2 de D5) vit dans le **propriétaire**,
donc il est **global au capteur**. Une fenêtre qui s'attache plus tard ne
recevra le presse-papier courant qu'à la **prochaine** copie.

**Sans conséquence en P1** (une seule fenêtre, attachée avant la copie du
critère ①). **Réel en P3**, et à porter dans le plan de P3 plutôt qu'à y être
redécouvert : le remède naturel est d'émettre l'état courant à l'inscription,
comme `parts::distribuer_les_parts` le fait déjà depuis `inscrire`
(`registre.rs:325`).

### E6 — `Capabilities.clipboard` (D7) n'est pas livrable par P1

Voir D-P1-6. La spec §D7 écrit « une annonce de capacité l'accompagne, calquée
sur `mic` », sans l'attribuer à un sous-bloc ; le §6 « P1 » ne la liste pas
dans son chemin. **Le plan la reporte à P2, avec sa raison technique.**

### E7 — L'argument du §7.1 sur l'emplacement du module est INEXACT ; sa conclusion est retenue

La spec justifie `agent/src/presse_papier.rs` à la racine nue par la
« Convention de module enfant » de `CLAUDE.md`. **Cette convention déclare
elle-même sa portée**, et elle ne couvre pas ce cas : « elle ne s'applique
QU'aux modules qu'on **extrait** d'un fichier `#[cfg(windows)]` (ou autrement
non portable) […] et qui doivent de ce fait devenir des **frères de premier
niveau** de ce parent ». `presse_papier` n'est extrait de rien.

**Le précédent exact du dépôt est ailleurs** : `agent/src/capteur/plein_ecran.rs`
(**235** lignes) est un module **pur**, vivant **sous `capteur/`** (déclaré
`pub mod plein_ecran;` dans `agent/src/capteur.rs:15`, sans `cfg`, parce que
`capteur.rs` n'est pas gaté), avec un `#[cfg(windows)] mod win { … }`
**inline** (`plein_ecran.rs:140-158`) et un `#[cfg(windows)] pub use
win::lire_style;` (`:160-161`). C'est exactement la forme dont P1 a besoin.

**Décision : on retient malgré tout la racine nue, et pour une AUTRE raison
que celle de la spec.** D1 pose que le propriétaire est « le capteur quand il
existe, l'enfant sinon ». Un module rangé sous `capteur/` porterait un nom
faux le jour où le propriétaire mono-fenêtre arrivera — et ce jour est nommé
(leg n°1 de P1, plus bas). `agent/src/presse_papier.rs`, racine nue,
`mod presse_papier;` ordinaire, avec `#[cfg(windows)] mod win32;` **à
l'intérieur** : l'enfant n'a jamais besoin de sortir de l'arbre de son parent,
donc la règle du `#[path]` ne le concerne pas.

⚠️ **Une différence avec `plein_ecran.rs` qu'il faut traiter, pas découvrir** :
`plein_ecran::lire_style` n'a **aucun** repli non-Windows, parce que son unique
appelant (`capteur/fenetre.rs`) est lui-même `#[cfg(windows)]`. Ici l'appelant
est `capteur/sommeil/registre.rs`, qui **n'est pas gaté** et doit compiler sur
l'hôte. `presse_papier` doit donc porter un **stub `#[cfg(not(windows))]`**.

### E8 — Deux variantes s'appelleront `Clipboard`, et il faut le savoir avant P2

`AgentControl::Clipboard` (P1) et `ClientControl::Clipboard` (P2) porteront
tous deux le tag `"clipboard"` en kebab-case. **Aucune collision réelle** :
`parseAgentControl` (`proto/ts/control.ts:130-139`) n'analyse que
`AgentControl`, et un message client ne passe jamais par là. Mais côté
TypeScript, les deux interfaces ne peuvent pas s'appeler `ClipboardMessage`.

**Décision, prise ici pour que P2 n'ait pas à renommer du code livré** :
l'interface de P1 s'appelle **`ClipboardAgentMessage`** ; celle de P2
s'appellera `ClipboardClientMessage`. Le tag reste `'clipboard'` des deux
côtés — c'est le nom **de la variante Rust** qui est traduit, pas le nom de
l'interface TS.

### E9 — Le canal `Message` du registre est NON BORNÉ

`registre.rs:279` : `let (emetteur, receveur) = channel::<Message>();` —
`std::sync::mpsc::channel`, donc **illimité**. `send` ne bloque jamais (ce qui
est bien : le tour de roue tient le verrou pendant la distribution), mais un
fil de fenêtre bloqué accumulerait des `Message::PressePapier` portant jusqu'à
64 KiB chacun.

**Borné en pratique** par le fait qu'on n'émet **qu'au changement** et que le
garde n°2 supprime les répétitions. **Nommé, non corrigé**, et à surveiller si
P3 mesure une fenêtre lente.

### E10 — 🔴 Une variable de sonde héritée fait tourner la SONDE à la place du CAPTEUR

`agent/src/main.rs:375` appelle `diagnostics::aiguiller()?` et retourne si une
sonde a tourné ; le bras `CAPTEUR` n'arrive qu'en `:383`. Or
`agent/src/superviseur/lanceur.rs:157-170` lance le capteur en héritant de
l'environnement, ne retirant que `SUPERVISEUR`, `TEST_FILE` et `WINDOW_TITLE`.

**Donc : un `PRESSE_PAPIER_SONDE` resté posé ferait exécuter la sonde par le
processus CAPTEUR, qui s'arrêterait aussitôt — et le superviseur le
relancerait en boucle.** C'est le piège de D3 (`MULTIFENETRE_VDD_PURGE` hérité
par les sondes) rejoué sur une autre paire.

**Ce plan ne modifie pas `lanceur.rs`** — ajouter un `env_remove` pour une
variable de banc y poserait une convention que les huit variables
`MULTIFENETRE_*` existantes ne suivent pas. Il prescrit à la place, et
l'inscrit **dans le commentaire de la sonde et dans `run-agent.sh`** : **la
sonde se lance SEULE**, sans `SUPERVISEUR`, et le tableau des variables de
`CLAUDE.md` (tâche 18) le dit.

### E11 — 🔴 Le montage de recette peut être incapable de lire ce que `writeText` a écrit

Le critère ① de la spec exige de juger **sur le contenu collé localement**,
« pas sur une ligne de journal ». Or la recette de ce dépôt pilote un Chromium
**sans interface** (`--headless`), et **rien n'établit qu'un `writeText` y
atteigne le presse-papier X11 de l'hôte**. Ce n'est ni mesuré ni exclu : la
sonde du §3 de la spec a tourné dans une page Playwright et n'a jamais relu le
presse-papier du système hôte.

**Ce plan prescrit un TÉMOIN DE MESURABILITÉ, joué AVANT le critère ①** —
tâche 16, étape 0. Une page appelle `writeText('temoin-<nonce>')` ; l'hôte
lit `xclip -o -selection clipboard` (ou `wl-paste`). Si le nonce revient, le
critère ① est mesurable **au niveau 2** (le vrai). Sinon, il est **NON
MESURABLE au niveau 2 sur ce montage**, et le relevé doit l'écrire — ce n'est
pas un échec du produit, c'est une mesure non prise.

⚠️ **Le niveau 1 ne remplace pas le niveau 2, et le plan interdit de le
présenter comme tel.** Le niveau 1 relève le texte réellement passé à
`writeText` et le fait que la promesse ait **résolu** — c'est un pas de plus
que « la trace est apparue », et un pas de moins que « l'utilisateur peut
coller ».

### E12 — Le bandeau garde son TEXTE une fois masqué : lire `#status.textContent` ne prouve rien

`client/src/status.ts:79-80` écrit `element.textContent = message` puis
`element.dataset.hidden = 'false'` ; `masquer()` (`:65-68`) ne pose que
`dataset.hidden = 'true'` et **n'efface jamais `textContent`**. C'est le piège
relevé en D5 et jamais corrigé.

**Le critère ③ lit les DEUX** : `textContent` **et** `dataset.hidden === 'false'`.

### E13 — La ROUGE du critère ① se joue sur l'HÔTE, pas sur la VM, et c'est déclaré

La spec veut que la rouge de `pont_media.rs` soit jouée « avant le vert ». Ce
plan la joue en **test d'hôte** (tâche 9, étape 1), sur le patron des trois
tests existants (`pont_media.rs:156`, `:184`, `:215`), qui existent exactement
pour cela — le fichier est hors `#[cfg(windows)]` **à dessein**, et son
commentaire de module le dit (`pont_media.rs:4-10`).

**La rouge n'est PAS rejouée sur la VM**, et c'est déclaré : elle coûterait une
compilation distante et une exécution complète pour observer le même défaut,
au même hop, avec moins de précision. Le test d'hôte est vu **rouge** avant
d'être vu vert, et son `cargo test` rouge est **versé** dans le document de
résultats.

### E14 — Le critère ② dépend d'une mesure qui n'a pas encore eu lieu

La spec l'écrit elle-même : « **Si aucun message ne part même sans le garde, le
critère est NON MESURABLE** et doit le dire ». Le plan rend cette
conditionnalité opérationnelle : la question **Q2** de la sonde (tâche 2)
tranche, et la tâche 16 lit la réponse **avant** de jouer le critère.

---

## Structure des fichiers

```
agent/src/
  presse_papier.rs                     ← NEUF, PUR, racine nue (E7)
    · PRESSE_PAPIER_MAX, PERIODE_PRESSE_PAPIER
    · actif()                          — `PRESSE_PAPIER=0` désarme (OnceLock)
    · normaliser(&str) -> String       — `\r\n` → `\n`
    · Annonce { Texte(String), Refus { octets: u32 } }
    · Sondeur { reference: Option<u32>, dernier_emis: Option<String>, dernier: Instant }
        · observer(seq, lire) -> Option<Annonce>   ← LE CŒUR, PUR, INJECTÉ
        · tour(&mut self) -> Option<Annonce>       ← appelle win32, hors verrou
  presse_papier/
    win32.rs                           ← NEUF, #[cfg(windows)], AUCUNE décision
        · numero_de_sequence() -> u32
        · lire_texte() -> Result<Option<String>>
  diagnostics/
    presse_papier.rs                   ← NEUF, #[cfg(windows)], la sonde P0
  capteur/
    protocole.rs                       ← DepuisCapteur::PressePapier { texte, octets }
    pont_media.rs                      ← 🔴 le bras + son test
    distante.rs                        ← Recu::PressePapier, champ, méthode
    distante/tests.rs                  ← ⚠️ marge 26 (E1)
    sommeil.rs                         ← Message::PressePapier
    sommeil/registre.rs                ← le tour de roue, sondage HORS VERROU
    sommeil/presse_papier.rs           ← NEUF — la distribution, sur le patron de parts.rs
    fenetre/transitions.rs             ← le bras de relais
  transport/
    tick.rs                            ← branche a1septies
    controle.rs                        ← le nom au journal (gardé par le compilateur)
  source.rs                            ← presse_papier_a_annoncer(), à défaut inerte
  main.rs                              ← mod presse_papier;

proto/
  src/control.rs                       ← AgentControl::Clipboard + constructeur
  src/control/tests.rs                 ← NEUF — 🔴 EXTRACTION PRÉALABLE (tâche 3)
  ts/control.ts                        ← ClipboardAgentMessage, union, TYPES_AGENT DÉRIVÉ
  ts/control.test.ts                   ← +tests, dont la rouge du Record

client/src/
  presse-papier.ts                     ← NEUF, PUR, SANS DOM
  presse-papier.test.ts                ← NEUF
  main.ts                              ← +1 branche onControl, +câblage (marge 49)

scripts/run-agent.sh                   ← PRESSE_PAPIER_SONDE (tâche 1), PRESSE_PAPIER (tâche 14)

docs/superpowers/plans/
  2026-08-19-presse-papier-p1-resultats.md      ← tâche 19
  journaux-presse-papier-p1/                    ← versés dans git
```

---

## Interfaces partagées

**Rust, `agent/src/presse_papier.rs`** — le cœur pur, injecté :

```rust
pub enum Annonce {
    /// Le texte à pousser, déjà normalisé et sous la borne.
    Texte(String),
    /// Un contenu de `octets` octets d'UTF-8 (APRÈS normalisation) a été
    /// REFUSÉ, jamais tronqué.
    Refus { octets: u32 },
}

impl Sondeur {
    /// Le cœur, et **il ne touche pas Windows** : `seq` est le numéro de
    /// séquence lu par l'appelant, `lire` n'est appelée QUE si ce numéro a
    /// bougé. C'est ce qui rend l'ensemble éprouvable sur l'hôte, y compris
    /// la propriété « on n'ouvre pas le presse-papier pour rien ».
    pub fn observer(
        &mut self,
        seq: u32,
        lire: impl FnOnce() -> Option<String>,
    ) -> Option<Annonce>;
}
```

**Rust, `agent/src/presse_papier/win32.rs`** — aucune décision :

```rust
#[cfg(windows)] pub fn numero_de_sequence() -> u32;
#[cfg(windows)] pub fn lire_texte() -> anyhow::Result<Option<String>>;
```

**Rust, `agent/src/capteur/protocole.rs`** :

```rust
/// Le presse-papier de la VM a changé. Poussé non sollicité, **au changement
/// seulement**. `texte` est `None` sur un refus de taille (voir D4).
PressePapier { texte: Option<String>, octets: u32 },
```

**TypeScript, `client/src/presse-papier.ts`** — pur, sans DOM :

```ts
export interface Recu { texte: string | null; octets: number }

export class PressePapierLocal {
    /** Un message est arrivé de l'agent. Toujours mémorisé. */
    recevoir(recu: Recu): void;
    /** Ce qu'il faut écrire MAINTENANT, ou `undefined`. */
    aEcrire(focalise: boolean): string | undefined;
    /** L'écriture a réussi. */
    confirmer(texte: string): void;
    /** L'écriture a échoué. Rend le message à afficher au DEUXIÈME échec. */
    echouer(): string | undefined;
    /** Le refus à dire, ou `undefined`. */
    refusADire(): string | undefined;
}
```

---

## Ordre et parallélisme

| Famille | Tâches | Dépend de | Parallélisable | Paquets touchés |
| --- | --- | --- | --- | --- |
| 0 — la mesure | 1, 2 | 1 ← rien ; 2 ← 1 | non | 🔴 `agent/`, `scripts/` |
| 1 — extraction préalable | 3 | rien | avec tout | 🔴 `proto/` |
| 2 — les purs | 4, 5 | rien (⚠️ 4 ← **verdict de 2**) | entre elles, et avec 3 | `agent/`, `client/` |
| 3 — le protocole | 6, 7, 8 | 6 ← **3** ; 7 ← rien ; 8 ← rien | 7 et 8 entre elles | 🔴 `proto/`, `agent/` |
| 4 — le trajet agent | 9, 10, 11, 12, 13, 14 | 9 ← 8 ; 10 ← 8 ; 11 ← 6, 10 ; 12 ← 4 ; 13 ← 4, 8, 12 ; 14 ← 4 | 9 et 10 entre elles ; 12 avec 9/10 | 🔴 `agent/`, `scripts/` |
| 5 — le navigateur | 15 | 5, 7 | avec la famille 4 | 🔴 `client/` |
| 6 — recette et clôture | 16, 17, 18, 19 | 16 ← tout ; 17, 18, 19 ← 16 | 17 et 18 entre elles | — |

**Chemin critique** : 1 → 2 → 4 → 12 → 13 → 16.

🔴 **La tâche 2 est une PORTE ÉLIMINATOIRE, et elle ne garde qu'une partie du
plan.** Son verdict décide du mécanisme de **détection** (D2), donc des tâches
**4, 12 et 13**. Il ne décide **rien** du trajet du message : les tâches 3, 5,
6, 7, 8, 9, 10, 11, 14 et 15 sont valides quel que soit le mécanisme retenu,
parce que tous les replis envisagés par la spec (`AddClipboardFormatListener`,
ou l'ouverture du presse-papier à chaque tour) poussent **le même**
`DepuisCapteur::PressePapier` sur **le même** chemin. **Elles peuvent donc
démarrer en parallèle de la tâche 2.**

🔴 **La tâche 3 (extraction) précède la tâche 6 sans exception**, et n'ajoute
**aucune** ligne de comportement.

**Quatre tâches purement isolées peuvent démarrer au premier tour** : 1, 3, 5,
7 — et deux d'entre elles (3, 7) touchent `proto/`, donc relancer
`git log --oneline -5 -- proto/` avant de les lancer.

---

# Famille 0 — la mesure qui gouverne

### Task 1 : la sonde `PRESSE_PAPIER_SONDE`, et sa ligne de `run-agent.sh`

**Objet :** écrire l'instrument qui répond aux quatre questions que la spec §8
déclare **non mesurées**. Aucun code de produit.

**Files:**
- Create: `agent/src/diagnostics/presse_papier.rs`
- Modify: `agent/src/diagnostics.rs`, `scripts/run-agent.sh`

**Interfaces:** produit `PRESSE_PAPIER_SONDE=<secondes>`.

**Forme exacte de la sonde.** Elle se lance **SEULE** — sans `SUPERVISEUR`, et
sans aucune autre variable `MULTIFENETRE_*` ni `PRESSE_PAPIER_SONDE` résiduelle
(E10, et le piège de D8 : *l'aiguillage retourne après la première sonde
reconnue*). Elle journalise chaque ligne avec le préfixe **`P0`**, et **ne
journalise jamais le texte du presse-papier** — une empreinte SHA-256 tronquée
à 12 caractères hexadécimaux, et une longueur.

Séquence, dans cet ordre :

| Phase | Ce qu'elle fait | La question |
| --- | --- | --- |
| **A** | `GetClipboardSequenceNumber()` trois fois à 250 ms, sans rien toucher | **Q1** — le compteur existe-t-il, et est-il STABLE au repos ? |
| **B** | boucle de `<secondes>` à 4 Hz : à chaque tour, relit le compteur ; s'il a bougé, `OpenClipboard`/`GetClipboardData(CF_UNICODETEXT)`/`CloseClipboard`, et journalise `seq`, `octets_utf16`, `octets_utf8`, `empreinte`, `echecs_open` | **Q1bis** — une copie faite à la main dans le Bloc-notes fait-elle bouger le compteur, et le texte est-il lisible ? |
| **C** | écrit elle-même `sonde-presse-papier-<nonce>` par `SetClipboardData`, relit le compteur | **Q3** — notre PROPRE écriture fait-elle bouger le compteur ? (fonde le garde n°1 de D5, donc P2) |
| **D** | réécrit **le MÊME** texte, relit le compteur | **Q2** — une réécriture IDENTIQUE fait-elle bouger le compteur ? (décide du critère ② de P1) |
| **E** | `P0 BILAN q1=… q1bis=… q2=… q3=… echecs_open=<n>/<k> seq_debut=… seq_fin=…` | — |

- [ ] **Step 1 : écrire la sonde de sorte qu'elle PUISSE rendre un verdict
      défavorable.** 🔴 **Aucune phase ne juge sur un code de retour** : la
      phase C juge sur le **mouvement du compteur relu**, jamais sur le
      succès de `SetClipboardData` ; la phase A rend `P0 NON MESURABLE` si les
      trois relevés valent **0** (la documentation de
      `GetClipboardSequenceNumber` prévoit `0` quand le processus n'a pas
      l'accès `WINSTA_ACCESSCLIPBOARD`). **Un verdict positif exige que la
      chose mesurée existe** — piège de D9 (`survit=true` rendu par une sortie
      disparue).
- [ ] **Step 2 : l'écriture des phases C et D est DÉCLARÉE dans le commentaire
      de module** — elle détruit le presse-papier de la VM, et c'est la seule
      écriture de tout le sous-bloc (D-P1-7).
- [ ] **Step 3 : le bras d'aiguillage.** Dans `agent/src/diagnostics.rs`, sur
      le patron des voisines (`if std::env::var("…").is_ok() { …; return
      Ok(true) }`), **avant** le bloc `multifenetre::aiguiller()` qui est
      délibérément en dernier (`diagnostics.rs:118-125`). Le commentaire de ce
      bras **nomme E10** : *cette variable ne doit jamais coexister avec
      `SUPERVISEUR`, sinon le processus capteur exécute la sonde à la place de
      son service.*
- [ ] **Step 4 : la ligne de `scripts/run-agent.sh`**, sur le patron des
      quarante existantes :
      `${PRESSE_PAPIER_SONDE:+\$env:PRESSE_PAPIER_SONDE = '$PRESSE_PAPIER_SONDE'}`.
      **C'est la règle n°7, et elle se joue dans la tâche qui introduit la
      variable, jamais après.**
- [ ] **Step 5 :** `cd agent && cargo check --target x86_64-pc-windows-gnu` →
      **sortie 0**, et le compte d'avertissements **annoncé avant d'être lu**
      (attendu : **11**, le compte de départ, la sonde étant appelée).
- [ ] **Step 6 :** `cargo test -p agent` → **550**, inchangé (aucun test neuf :
      la sonde est `#[cfg(windows)]` et sans logique pure).

---

### Task 2 : 🔴 jouer la sonde sur la VM — PORTE ÉLIMINATOIRE

**Objet :** prendre la mesure que la spec §8 déclare due, et écrire le verdict.
**Aucune ligne de code.**

**Files:**
- Create: `docs/superpowers/plans/journaux-presse-papier-p1/p0-sonde-{1,2}.log`, et
  leurs jumeaux `-plat.log`
- Modify: aucun

- [ ] **Step 1 : la VM.** `virsh list --all` ; la démarrer si besoin ; attendre
      **le montage réel** (`until ls /media/vm/dev >/dev/null 2>&1`), pas le
      port 5985 — `mountpoint -q` ne suffit pas.
      `set -a && source .env && set +a` **avant** `scripts/build-agent.sh`,
      sans quoi il s'arrête EN SILENCE après « sources synchronisées ».
- [ ] **Step 2 : `Get-Process agent`** avant la première tentative **et après
      chaque tentative, y compris échouée** — un superviseur resté vivant
      empêche l'ouverture d'`agent.log` et fait relire le journal PÉRIMÉ. Payé
      trois fois sur trois en D8.
- [ ] **Step 3 : lancer la sonde SEULE**, `SUPERVISEUR` absent :
      `PRESSE_PAPIER_SONDE=40 scripts/run-agent.sh`. Pendant la phase B,
      copier à la main, dans le Bloc-notes de la VM, **trois** textes
      distincts, puis **recopier le troisième à l'identique**.
- [ ] **Step 4 : DEUX exécutions.** Verser les journaux **bruts** et leurs
      `-plat` (`sed 's/\x1b\[[0-9;]*m//g'`).
- [ ] **Step 5 : écrire le verdict** dans le document de résultats (tâche 19),
      selon cette grille — **elle est écrite AVANT la mesure, pas après** :

| Verdict | Relevé | Ce qui suit |
| --- | --- | --- |
| **V0 — favorable** | Q1 stable non nul, Q1bis bouge à chaque copie, texte lisible | le plan continue tel quel |
| **V1 — 🔴 le compteur rend 0** | phase A rend `0, 0, 0` | **D2 est mort.** Les tâches 4, 12, 13 se rouvrent : repli sur `AddClipboardFormatListener` (fenêtre message-only **dans le capteur**, que D2 écarte avec ses raisons) ou sur l'ouverture du presse-papier à **chaque** tour (coût réel, contention R7). **Le sous-bloc n'est pas livrable en l'état** ; le trajet (tâches 3, 5–11, 14, 15) reste valide |
| **V2 — 🔴 le compteur ne bouge PAS sur une copie** | Q1bis = 0 mouvement sur 3 copies | même conclusion que V1 |
| **V3 — ⚠️ le compteur bouge SANS copie** | phase A instable au repos | D2 perd son argument principal : le garde n°2 devient l'unique garde, et le presse-papier doit être **ouvert à chaque tour**. Livrable, à un autre coût — **à écrire, pas à taire** |
| **V4 — ⚠️ le compteur ne bouge PAS sur une réécriture IDENTIQUE** | Q2 = pas de mouvement | **le produit va bien ; c'est le CRITÈRE ② qui devient NON MESURABLE** par la rouge prescrite (désarmer le garde n°2 ne ferait alors partir aucun message). La tâche 16 doit l'écrire, exactement comme la spec l'ordonne |
| **V5 — ⚠️ `OpenClipboard` échoue souvent** | `echecs_open` > 10 % | R7 est plus mordant qu'annoncé ; D-P1-5 devient essentiel, et le taux est **relevé**, pas supposé |

⚠️ **Q3 (notre propre écriture fait-elle bouger le compteur ?) ne conditionne
RIEN dans P1** — l'agent n'écrit jamais. Elle est mesurée ici parce qu'elle est
gratuite et que **P2 en dépend entièrement**.

---

# Famille 1 — l'extraction préalable

### Task 3 : 🔴 EXTRACTION de `proto/src/control.rs`, AVANT toute addition

**Objet :** rendre au fichier la marge que la tâche 6 va consommer. **Aucune
addition de comportement, aucune ligne de presse-papier.**

**Files:**
- Create: `proto/src/control/tests.rs`
- Modify: `proto/src/control.rs`

**Relevé, par la commande, le 19 août 2026 : 470 lignes, marge 30.** Le bloc de
tests est `#[cfg(test)] mod tests { … }` de **`:272` à `:470`**, soit **199
lignes** — vérifié par `grep -n '#\[cfg(test)\]' proto/src/control.rs` et
`wc -l`. L'extraction rend **271**, et clôt la question pour longtemps.

- [ ] **Step 0 : `git log --oneline -5 -- proto/`** — périmètre concurrent.
- [ ] **Step 1 : relever le compte de départ.** `cargo test -p proto` →
      **52 passed** (annoncé ici, à confirmer).
- [ ] **Step 2 : extraire VERBATIM** vers `proto/src/control/tests.rs`,
      déclaré dans `control.rs` par
      `#[cfg(test)] #[path = "control/tests.rs"] mod tests;`.
      ⚠️ **C'est le mécanisme que `agent/src/superviseur/table.rs` emploie
      déjà** pour ses deux modules de tests, et la « Convention de module
      enfant » de `CLAUDE.md` range explicitement cet usage **hors de sa
      portée** : ce n'est pas la convention du `#[path]` chez le parent, c'est
      la règle des 500 lignes.
      ⚠️ **Extraction, pas compression.** Aucun commentaire n'est resserré.
      Comparer mot pour mot (`git show HEAD:proto/src/control.rs | sed -n
      '272,470p' | diff - proto/src/control/tests.rs` doit ne rendre que le
      `use super::*;` réécrit, s'il l'est).
- [ ] **Step 3 : relever le compte APRÈS** — **identique, 52**. *Une extraction
      qui change un compte de tests n'est pas une extraction.*
- [ ] **Step 4 : `wc -l proto/src/control.rs proto/src/control/tests.rs`**, et
      écrire les deux nombres et la marge obtenue **dans le message de commit**.

---

# Famille 2 — les modules purs

### Task 4 : `agent/src/presse_papier.rs` — le `Sondeur`, PUR et injecté

**Objet :** toute la décision du sens VM → navigateur, **sans une ligne de
Win32**, éprouvable sur l'hôte Linux.

⚠️ **Dépend du verdict de la tâche 2** : sous V1 ou V2, le `Sondeur` change de
forme (la détection ne passe plus par un numéro de séquence) et cette tâche se
réécrit.

**Files:**
- Create: `agent/src/presse_papier.rs`
- Modify: `agent/src/main.rs` (une ligne `mod presse_papier;` + son commentaire)

**Interfaces:** produit `PRESSE_PAPIER_MAX`, `PERIODE_PRESSE_PAPIER`, `actif()`,
`normaliser`, `Annonce`, `Sondeur`.

- [ ] **Step 1 : écrire les tests d'abord, et les voir ROUGES.**

| Test | Ce qui le rend ROUGE |
| --- | --- | 
| `normaliser("a\r\nb")` rend `"a\nb"` | 🔴 **la première chose qu'un test doit voir rouge** (spec §7.1) : ne pas normaliser double les lignes à l'aller-retour |
| `normaliser("a\rb")` — un `\r` **seul** — rend `"a\nb"` | ne traiter que `\r\n` laisse passer les fins de ligne Mac classiques |
| `normaliser("a\nb")` est **idempotent** | remplacer `\n` par `\r\n` puis re-normaliser : la fonction n'est plus idempotente et l'aller-retour de P2 divergera |
| 🔴 `observer(seq_inchangé, spy)` **n'appelle PAS** `spy` et rend `None` | c'est la propriété « on n'ouvre pas le presse-papier pour rien », **et elle est vérifiable sans Windows** : le spy est un `Cell<bool>`. La muter en appelant toujours `lire` fait tomber ce test |
| `observer(seq_neuf, ‖ Some("bonjour"))` rend `Annonce::Texte("bonjour")` | — |
| 🔴 `observer` deux fois avec un seq **différent** mais le **même texte** rend `Some` puis **`None`** | c'est le **garde n°2 de D5**, et c'est la rouge du critère ② : retirer la comparaison de contenu fait rendre `Some` deux fois |
| un texte de `PRESSE_PAPIER_MAX + 1` octets d'UTF-8 rend `Annonce::Refus { octets }` et **jamais** un `Texte` tronqué | 🔴 tronquer produit un collage silencieusement faux — le pire résultat possible (D4) |
| un texte de **exactement** `PRESSE_PAPIER_MAX` octets passe | une borne écrite `>=` au lieu de `>` |
| 🔴 un texte de `PRESSE_PAPIER_MAX + 8` octets **dont 12 sont des `\r`** passe **après** normalisation | c'est **D-P1-2** : borner avant de normaliser fait tomber ce test |
| le bornage compte des **octets d'UTF-8**, pas des `char` | borner sur `.chars().count()` : un texte d'emoji de 64 000 caractères pèse 256 000 octets et passerait |
| 🔴 `observer(seq_neuf, ‖ None)` — la lecture a ÉCHOUÉ — rend `None` **et n'avance pas la référence** ; l'appel suivant au **même** seq rappelle `lire` | c'est **D-P1-5** : mémoriser le seq avant la lecture perd le contenu à jamais, et ce test le voit |
| un refus **répété** à l'identique n'est annoncé **qu'une fois** | sinon un fichier de 100 Mio dans le presse-papier ferait clignoter le bandeau à chaque copie voisine |

- [ ] **Step 2 : implémenter.**
      - `PRESSE_PAPIER_MAX: usize = 64 * 1024`, **non calibrée**, et le
        doc-comment le dit — elle rejoint `BPP_MIN`, `FACTEUR_FOCUS`,
        `PART_DORMANTE_BPS`, `HYSTERESIS`, `TAILLE_MAX_SORTIE`.
      - `PERIODE_PRESSE_PAPIER: Duration = Duration::from_millis(250)`, avec le
        doc-comment de `plein_ecran::PERIODE_STYLE` (`plein_ecran.rs:130-138`)
        transposé : ⚠️ **ce n'est PAS `PERIODE_REARBITRAGE`**, la valeur est du
        même ordre **délibérément**, et les faire suivre l'une l'autre
        coupleraient deux mécanismes que rien ne lie.
      - `actif()` : `OnceLock<bool>`, `std::env::var("PRESSE_PAPIER").as_deref()
        != Ok("0")`, `tracing::warn!` **une fois** si désarmé. Copier mot pour
        mot la structure de `plein_ecran::actif()` (`plein_ecran.rs:118-128`),
        **y compris son avertissement** : `=0` DÉSACTIVE, une simple présence
        n'active pas — tester `is_ok()` armerait le mécanisme en écrivant
        `PRESSE_PAPIER=0` pour le couper.
      - `Sondeur::tour()` : `#[cfg(windows)]` appelle `win32::numero_de_sequence()`
        puis `observer(seq, win32::lire_texte)` ; `#[cfg(not(windows))]` rend
        `None` — **le stub est obligatoire** (E7), l'appelant
        `capteur/sommeil/registre.rs` n'étant pas gaté.
      - `tour()` porte **son propre minuteur** `PERIODE_PRESSE_PAPIER` et rend
        `None` sans rien lire tant qu'il n'est pas échu, **même si le tour de
        roue l'appelle plus souvent**.
- [ ] **Step 3 : `mod presse_papier;`** dans `agent/src/main.rs`, **racine
      nue, sans `#[cfg(windows)]`**, avec le commentaire qui dit pourquoi (E7) :
      pur, testable sur l'hôte, `win32` gaté à l'intérieur. Le placer entre
      `mod pointer_settings;` et `mod plateforme;`, où l'ordre alphabétique le
      met.
- [ ] **Step 4 : voir vert.** Compte de tests Rust attendu : **550 + 12 = 562**,
      **annoncé avant d'être lu**.
- [ ] **Step 5 :** `cargo check --target x86_64-pc-windows-gnu` → sortie 0.

---

### Task 5 : `client/src/presse-papier.ts` — pur, sans DOM

**Objet :** la machine à états « reçu → focalisé → écrit », le dépôt différé, et
les deux messages que D8 exige. **Aucun `document`, aucun `navigator`.**

**Files:**
- Create: `client/src/presse-papier.ts`, `client/src/presse-papier.test.ts`
- Modify: aucun

**Injection de dépendances**, patron de `client/src/status.ts:16-17` et de
`client/src/resize.ts` — c'est ce qui rend le module éprouvable.

- [ ] **Step 0 : `git log --oneline -5 -- client/`** — périmètre concurrent.
- [ ] **Step 1 : écrire les tests d'abord, et les voir ROUGES.**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `recevoir` puis `aEcrire(true)` rend le texte | — |
| 🔴 `recevoir` puis `aEcrire(**false**)` rend `undefined`, et `aEcrire(true)` **ensuite** rend le texte | c'est le **dépôt différé** de D3 : écrire sans focus ferait rendre le texte au premier appel et ce test tombe |
| 🔴 **deux** réceptions sans focus, puis `aEcrire(true)` : c'est le **DERNIER** texte qui sort, jamais une file | « une écriture obsolète est impossible » (D3). Empiler dans un tableau fait sortir le premier, et ce test le voit |
| `confirmer` puis `aEcrire(true)` rend `undefined` | ne pas mémoriser l'écrit fait réécrire à chaque appel |
| 🔴 `echouer()` rend `undefined` au **premier** échec, un message au **second** | D8 : « bandeau persistant après **deux** échecs consécutifs, pour ne pas crier sur un simple défaut de focus ». Crier au premier fait tomber ce test |
| un `confirmer` **remet à zéro** le compteur d'échecs | sans cela, un échec au démarrage et un échec une heure plus tard crieraient |
| 🔴 le message d'échec dit **COMMENT** rétablir, pas seulement que quelque chose manque | règle déjà appliquée au micro (`client/src/main.ts:280-283`). Assertion sur une sous-chaîne d'instruction, pas sur la présence d'un message |
| `recevoir({ texte: null, octets: 102400 })` : `aEcrire(true)` rend `undefined` **et** `refusADire()` rend un message **nommant la taille** | 🔴 c'est le critère ③ : écrire quand même, ou taire le refus, fait tomber ce test |
| un refus **n'écrase pas** le dernier texte mémorisé | sinon un refus effacerait un contenu valide encore non écrit |
| `refusADire()` **se consomme** : deux appels ne rendent qu'un message | un bandeau réaffiché à chaque tour |

- [ ] **Step 2 : implémenter.** Aucune importation de `proto/ts/control.ts`
      dans ce fichier : il prend une `Recu` locale — c'est ce qui le garde pur
      et le découple du protocole.
- [ ] **Step 3 : voir vert.** Compte de tests client attendu :
      **187 + 10 = 197**, **annoncé avant d'être lu**.

---

# Famille 3 — le protocole partagé

### Task 6 : `proto/src/control.rs` — `AgentControl::Clipboard`

**Objet :** la variante, son constructeur, ses tests. **Dépend de la tâche 3.**

**Files:**
- Modify: `proto/src/control.rs`, `proto/src/control/tests.rs`

- [ ] **Step 0 :** vérifier que la tâche 3 est passée — `wc -l proto/src/control.rs`
      doit rendre ≈ **271**, pas 470. 🔴 **Si le fichier est encore à 470,
      s'arrêter : l'extraction précède l'addition, jamais l'inverse.**
- [ ] **Step 1 : la variante et le constructeur**, exactement comme D-P1-1 :
      `Clipboard { version, text: Option<String>, bytes: u32 }`, plus
      `pub fn clipboard(text: Option<String>, bytes: u32) -> Self`, sur le
      patron de `AgentControl::fullscreen` (`control.rs:243-245`).
- [ ] **Step 2 : `CONTROL_VERSION` NE MONTE PAS, et le doc le dit.** Reprendre
      la preuve du §D7 de la spec, vérifiée dans le code à l'écriture de ce
      plan : les deux vérifications de `v` sont des **égalités strictes**
      (`proto/src/control.rs:46`, `proto/ts/control.ts:132`), donc monter
      ferait rejeter **tous** les messages, `Ready` et `SessionEnd` compris —
      une incompatibilité **totale** remplacerait une dégradation **par
      message**. Le précédent est déjà écrit à trois lignes de là : le champ
      `mic` (`control.rs:117-138`).
- [ ] **Step 3 : les tests**, dans `control/tests.rs` :

| Test | Ce qui le rend ROUGE |
| --- | --- |
| un `clipboard` avec `text` rend `{"type":"clipboard","v":3,"text":"…","bytes":…}` | — |
| un `clipboard` de **refus** sérialise `"text":null` | employer `#[serde(skip_serializing_if)]` : le champ disparaît, et le client ne peut plus distinguer refus et absence |
| 🔴 un `clipboard` **avec `v:2`** est REJETÉ | c'est ce qui prouve que `verifie_version` est bien branché sur la variante neuve — l'oublier est une erreur silencieuse, `version` n'étant vérifié que par son attribut |
| un `clipboard` portant un champ **inconnu** est rejeté | retirer `deny_unknown_fields` — il est sur l'enum (`control.rs:110`), le test le fige |

- [ ] **Step 4 : voir vert.** `cargo test -p proto` → **52 + 4 = 56**,
      **annoncé avant d'être lu**. `wc -l proto/src/control.rs` → écrire le
      nombre dans le commit.

---

### Task 7 : `proto/ts/control.ts` — et 🔴 `TYPES_AGENT` DÉRIVÉ de l'union

**Objet :** l'interface, l'union, et **fermer structurellement** le risque R3 de
la spec : `TYPES_AGENT` est aujourd'hui un littéral écrit à la main
(`proto/ts/control.ts:106-108`) que **rien** ne confronte à l'union
(`:101-104`). L'oublier ne casse **ni la compilation TypeScript ni aucun test** :
`parseAgentControl` lève (`:135-137`), `client/src/webrtc.ts:272-276`
intercepte, et le message est perdu contre un `console.warn`.

**Ce plan REPREND le remède structurel** du plan de la gestion d'apps
(`docs/superpowers/plans/2026-08-19-gestion-apps-g1.md:784-812`, « E11 »), et
le dit : c'est la même maladie, sur le fichier d'origine.

**Files:**
- Modify: `proto/ts/control.ts`, `proto/ts/control.test.ts`

- [ ] **Step 0 : `git log --oneline -5 -- proto/`.**
- [ ] **Step 1 : l'interface**, nommée `ClipboardAgentMessage` (E8) :
      `{ v: number; type: 'clipboard'; text: string | null; bytes: number }`,
      et son ajout à l'union `AgentControl` (`:101-104`).
      ⚠️ **`text` n'est PAS optionnel ici** — c'est `string | null`. L'agent
      l'émet toujours ; `?` ferait passer un message tronqué pour un refus.
- [ ] **Step 2 : 🔴 dériver `TYPES_AGENT` de l'union**, et **voir la rouge** :

```ts
// Écrit comme un enregistrement EXHAUSTIF typé par l'union, jamais comme un
// littéral : ajouter une variante à `AgentControl` sans ajouter sa clé ici
// fait échouer `npm run typecheck`. Avant ce remède, l'oubli ne cassait NI la
// compilation NI aucun test — le message était simplement perdu contre un
// `console.warn` de `client/src/webrtc.ts:272-276`.
const TOUS_AGENT: Record<AgentControl['type'], true> = {
    ready: true, 'session-end': true, pointer: true, rumble: true,
    capabilities: true, link: true, asleep: true, fullscreen: true,
    clipboard: true,
};
const TYPES_AGENT = Object.keys(TOUS_AGENT) as AgentControl['type'][];
```

      **La ROUGE est ATTEIGNABLE et doit être JOUÉE** : retirer la clé
      `clipboard` de `TOUS_AGENT` en gardant `ClipboardAgentMessage` dans
      l'union fait échouer `cd proto && npm run typecheck` — un `Record<K, true>`
      dont une clé manque est une erreur `tsc`. **Verser la sortie `tsc`
      rouge** dans le document de résultats.
- [ ] **Step 3 : les tests** dans `ts/control.test.ts` :

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `parseAgentControl` accepte un `clipboard` avec texte | retirer `'clipboard'` de la liste — **et c'est précisément ce que le Step 2 rend impossible à oublier** |
| `parseAgentControl` accepte un `clipboard` à `text: null` | — |
| 🔴 `parseAgentControl` **rejette** un `clipboard` en `v: 2` | la vérification de `v` (`:132`) précède celle du type ; ce test la fige pour la variante neuve |
| 🔴 `TYPES_AGENT` contient **exactement** les `type` de l'union | `expect(TYPES_AGENT.slice().sort()).toEqual([…].sort())` sur les **neuf** valeurs, écrites à la main **dans le test** : c'est le témoin indépendant de la dérivation. Le rouge : ajouter une clé de trop à `TOUS_AGENT` |

      ⚠️ **`TYPES_AGENT` doit être exporté** pour ce dernier test. C'est un
      élargissement de surface, assumé et déclaré : sans lui la propriété n'a
      aucun témoin d'exécution, seulement un témoin de compilation.
- [ ] **Step 4 : voir vert.** `cd proto && npx vitest run` → **70 + 4 = 74**,
      **annoncé avant d'être lu**. Et `npm run typecheck` → sortie 0.

---

### Task 8 : `agent/src/capteur/protocole.rs` — `DepuisCapteur::PressePapier`

**Objet :** la variante du canal capteur ↔ enfant.

**Files:**
- Modify: `agent/src/capteur/protocole.rs`

**Relevé : 419 lignes, marge 81.**

- [ ] **Step 1 : la variante**, après `PleinEcran` (`protocole.rs:179`), avec
      son doc-comment sur le patron des quatre voisines — **poussé non
      sollicité, au changement seulement**, et **distinct d'`Etat` pour la même
      raison** qu'elles : `Etat` alimente un cache lu à chaque tour de la
      boucle de transport, et y mêler une annonce ponctuelle passerait par un
      chemin conçu pour un état permanent.
- [ ] **Step 2 : le doc-comment nomme la borne.** `TAILLE_MAX` du canal vaut
      **8 Mio** (`protocole.rs:23`) ; `PRESSE_PAPIER_MAX` vaut 64 KiB. **Ce
      n'est donc pas ce canal qui contraint**, et l'écrire ici évite qu'un
      successeur croie l'inverse.
- [ ] **Step 3 :** `cargo test -p agent` → **562**, inchangé (la variante seule
      n'ajoute pas de test ici ; ses tests vivent en tâches 9 et 10).
      `wc -l` et le nombre dans le commit.

---

# Famille 4 — le trajet dans l'agent

### Task 9 : 🔴 `agent/src/capteur/pont_media.rs` — le bras, et sa ROUGE jouée d'ABORD

**Objet :** le point de passage **non gardé par le compilateur** que ce dépôt a
payé **QUATRE** fois. **Ce serait la cinquième.**

**Files:**
- Modify: `agent/src/capteur/pont_media.rs`

Le bras catch-all vit en `pont_media.rs:75-78` :

```rust
Ok(autre) => {
    tracing::warn!(?autre, "trame inattendue sur la connexion média, abandonnée");
    return;
}
```

Ce `return` **tue le fil `lire_le_media`**, donc affame `SourceDistante`, donc
fait tomber la session dans sa fenêtre de reprise — **sans aucune panne
apparente**. Les quatre précédents sont inscrits dans le fichier lui-même :
`Sommeil` (D5, `:38-52`), `Part` (D6, `:53-61`), `Audio` (D7, `:62-67`),
`PleinEcran` (D8, `:68-74`).

- [ ] **Step 1 : 🔴 ÉCRIRE LE TEST D'ABORD ET LE VOIR ROUGE.** Sur le patron
      exact des trois existants — `lire_le_media_survit_a_une_part_et_la_transmet`
      (`:156`), `…_a_un_audio_…` (`:184`), `…_a_un_plein_ecran_…` (`:215`) : un
      flux **réel**, sérialisé par `ecrire_json`/`ecrire_image` de
      `protocole.rs` et non des octets à la main, portant un `Etat`, une image,
      puis un `PressePapier` — le fil doit **survivre** et transmettre
      `Recu::PressePapier`. **Sans le bras, ce test échoue** : c'est la rouge
      du critère ① de la spec, jouée sur l'hôte (E13). **Verser la sortie
      `cargo test` ROUGE** dans le document de résultats.
- [ ] **Step 2 : écrire le bras**, avec son commentaire nommant **explicitement
      que c'est la cinquième fois**, comme les quatre précédents nomment leur
      rang.
- [ ] **Step 3 : vérifier que la sévérité d'`Ok(autre)` n'a pas été
      affaiblie** — le test `une_trame_de_commande_egaree_sur_le_media_abandonne_le_fil`
      (`:244`) doit rester **vert**. C'est le test que D5 avait posé
      exactement pour cela.
- [ ] **Step 4 : voir vert.** `cargo test -p agent` → **562 + 1 = 563**,
      **annoncé avant d'être lu**.

⚠️ **Cette tâche dépend de la tâche 10 pour compiler** (`Recu::PressePapier`
doit exister). **Les faire dans le même tour ou dans cet ordre : 10 puis 9.**
Le tableau d'ordre les donne parallèles parce qu'elles se relisent bien
ensemble ; **si elles sont dispatchées séparément, 10 précède 9.**

---

### Task 10 : `distante.rs` et `source.rs` — `Recu`, le champ, la méthode de trait

**Objet :** porter l'annonce depuis le pont jusqu'à la boucle de transport.

**Files:**
- Modify: `agent/src/capteur/distante.rs`, `agent/src/capteur/distante/tests.rs`,
  `agent/src/source.rs`

**Relevés : `distante.rs` 409 (marge 91), `distante/tests.rs` **474 (marge 26)**,
`source.rs` 393 (marge 107).**

- [ ] **Step 1 : `Recu::PressePapier { texte: Option<String>, octets: u32 }`**
      (`distante.rs:37-54`) et son bras dans `next_frame` (`:167-200`).
      🔵 **Ce `match` n'a AUCUN catch-all** — vérifié : la variante neuve
      **casse la compilation** tant que son bras n'est pas écrit. C'est le
      contraire de `pont_media.rs`, et il faut savoir lequel est lequel.
- [ ] **Step 2 : le champ et la méthode.** `presse_papier: Option<(Option<String>, u32)>`
      sur `SourceDistante`, initialisé `None` (`:125-148`), et
      `presse_papier_a_annoncer(&mut self) -> Option<(Option<String>, u32)>`
      qui **CONSOMME** (`self.presse_papier.take()`), sur le patron exact de
      `plein_ecran_a_annoncer` (`:377-379`).
      ⚠️ **État courant, pas un historique** : deux copies arrivées entre deux
      lectures s'écrasent — c'est correct, le presse-papier **est** un état.
- [ ] **Step 3 : la méthode de trait dans `source.rs`**, à défaut **inerte**
      (`None`), sur le patron de `plein_ecran_a_annoncer` (`source.rs:141-153`),
      avec son doc-comment qui dit pourquoi le défaut est inerte : une source
      fichier n'a pas de presse-papier, et une `WindowsSource` tenue en direct
      par son propre processus n'a pas de capteur pour la lui pousser.
- [ ] **Step 4 : 🔴 PORTE DE MESURE sur `distante/tests.rs` (E1).**
      `wc -l agent/src/capteur/distante/tests.rs` **AVANT** (attendu **474**)
      et **APRÈS**. **Si l'après dépasse 460, extraire dans cette même tâche**
      vers `agent/src/capteur/distante/tests_etats.rs`, déclaré par
      `#[cfg(test)] #[path = "distante/tests_etats.rs"] mod tests_etats;`.
      **Extraction verbatim, jamais une compression.**
- [ ] **Step 5 : le test**, à côté de celui de `PleinEcran`
      (`distante/tests.rs:470`) : un `Recu::PressePapier` déposé se relit **une
      fois** par `presse_papier_a_annoncer`, puis rend `None`.
      **ROUGE** : ne pas consommer — la boucle de transport, qui interroge à
      ~100 Hz, inonderait le canal de contrôle.
- [ ] **Step 6 : voir vert.** `cargo test -p agent` → **563 + 1 = 564**,
      **annoncé avant d'être lu**. `cargo check --target x86_64-pc-windows-gnu`
      → sortie 0.

---

### Task 11 : `tick.rs` — la branche `a1septies` ; `controle.rs` — le nom au journal

**Objet :** transformer l'annonce en `AgentControl::Clipboard`.

**Files:**
- Modify: `agent/src/transport/tick.rs`, `agent/src/transport/controle.rs`

**Relevés : `tick.rs` 403 (marge 97), `controle.rs` 225 (marge 275).**

- [ ] **Step 1 : la branche**, après `a1sexies` et avec son commentaire dans la
      liste de priorités en tête de fonction (`tick.rs:55-60`, à compléter) :

```rust
// a1septies) Le presse-papier de la VM a changé. Même régime qu'a1ter-bis :
//            `presse_papier_a_annoncer` CONSOMME, donc aucun message n'est
//            jamais réémis et cette branche ne peut pas inonder le canal de
//            contrôle même à ~100 Hz.
if let Some((texte, octets)) = self.source.presse_papier_a_annoncer() {
    self.queue_control(AgentControl::clipboard(texte, octets));
    return Ok(Tick::Continue);
}
```

      ⚠️ **Le commentaire d'audit de `tick.rs:67-68` doit être mis à jour** : il
      énumère les branches qui « ne touchent même pas `self.rtc` ». `a1septies`
      en fait partie — elle ne touche que `self.source` et la file. **Ce dépôt
      a payé DEUX rondes pour avoir laissé cet énoncé faux** (D6, §« trois faits
      de conception », point 3).
- [ ] **Step 2 : `controle.rs`** — le bras `AgentControl::Clipboard { .. } =>
      "clipboard"` dans le `match` **exhaustif** de `:98-107`.
      🔵 **Gardé par le compilateur** : sans lui, `cargo check` échoue. C'est
      le point de passage n°6 de la spec §11, et il se signale tout seul.
- [ ] **Step 3 :** `cargo test -p agent` → **564**, inchangé.
      `cargo check --target x86_64-pc-windows-gnu` → sortie 0.

---

### Task 12 : `agent/src/presse_papier/win32.rs` — la lecture Win32, AUCUNE décision

**Objet :** deux fonctions, zéro logique. ⚠️ **Dépend du verdict de la tâche 2.**

**Files:**
- Create: `agent/src/presse_papier/win32.rs`
- Modify: `agent/src/presse_papier.rs` (la déclaration `#[cfg(windows)] mod win32;`)

- [ ] **Step 1 : `numero_de_sequence() -> u32`** — `GetClipboardSequenceNumber()`,
      sans ouvrir le presse-papier ni posséder de fenêtre. **Rendre `0` tel
      quel** : c'est au `Sondeur` (pur) de décider que `0` signifie « pas de
      compteur », et cette décision est déjà testée sur l'hôte.
- [ ] **Step 2 : `lire_texte() -> anyhow::Result<Option<String>>`** —
      `OpenClipboard(None)` → `GetClipboardData(CF_UNICODETEXT)` → `GlobalLock`
      → copie → `GlobalUnlock` → `CloseClipboard`.
      🔴 **Un garde RAII ferme le presse-papier sur TOUS les chemins**, y
      compris un `?` et une panique. Un presse-papier laissé ouvert bloque
      **toute la window station**, pas seulement l'agent.
      🔴 **La donnée est copiée AVANT `CloseClipboard`** : le handle appartient
      au presse-papier et n'est plus valide après.
      - `Ok(None)` quand le format `CF_UNICODETEXT` est absent (une image, un
        fichier) — **ce n'est pas une erreur**, et le sondeur ne doit rien
        émettre.
      - `Err` quand `OpenClipboard` échoue — cas **normal** (R7), traité par
        D-P1-5 : la référence n'avance pas, on retente au tour suivant.
      - Conversion par `String::from_utf16_lossy` sur la tranche **jusqu'au
        premier `0u16`** ; un substitut isolé ne doit jamais faire échouer la
        lecture (D-P1-2).
- [ ] **Step 3 : la normalisation et le bornage ne sont PAS ici.** Ce fichier
      rend le texte **brut**. C'est `Sondeur::observer` qui normalise et borne,
      parce que c'est là que c'est testable. **Un seul appel de décision dans
      ce fichier ferait sortir la logique de la zone éprouvable.**
- [ ] **Step 4 :** `cargo check --target x86_64-pc-windows-gnu` → **sortie 0**,
      et le compte d'avertissements **annoncé avant d'être lu**.

---

### Task 13 : le capteur — `Message::PressePapier`, le sondage HORS VERROU, le relais

**Objet :** brancher le sondeur sur le tour de roue et faire partir le message.
⚠️ **Dépend du verdict de la tâche 2.**

**Files:**
- Create: `agent/src/capteur/sommeil/presse_papier.rs`
- Modify: `agent/src/capteur/sommeil.rs`, `agent/src/capteur/sommeil/registre.rs`,
  `agent/src/capteur/fenetre/transitions.rs`

**Relevés : `sommeil.rs` 328, `registre.rs` 346, `transitions.rs` 193.**

- [ ] **Step 1 : `Message::PressePapier { texte: Option<String>, octets: u32 }`**
      (`sommeil.rs:172-188`).
      🔵 **Gardé par le compilateur** : le `match` de
      `transitions.rs::appliquer_les_ordres` (`:121-190`) n'a **aucun**
      catch-all — vérifié —, donc `cargo check` échoue tant que le bras manque.
- [ ] **Step 2 : le distributeur**, `agent/src/capteur/sommeil/presse_papier.rs`,
      sur le patron **exact** de `porteurs::distribuer_l_audio`
      (`sommeil/porteurs.rs:28`) et `parts::distribuer_les_parts`
      (`sommeil/parts.rs:104`) : signature
      `pub(super) fn distribuer(garde: &mut MutexGuard<'static, Etat>, annonce: Annonce)`,
      envoi sur `garde.canaux`, et **un canal rompu passe par
      `registre::oublier`** — le point de passage unique, jamais un `remove`
      direct (leçon de M1/D6, où `focalisee` avait été oublié par deux chemins).
- [ ] **Step 3 : 🔴 le sondage HORS VERROU** (D-P1-3, E3), dans
      `registre::demarrer_le_tour_de_roue` (`registre.rs:154-164`) : `sondeur.tour()`
      est appelé **avant** `let mut garde = etat();`, et seul le résultat entre
      sous le verrou. **Le commentaire de cette ligne dit pourquoi**, en nommant
      les quatre fonctions qu'un blocage priverait de service (`inscrire`,
      `retirer`, `signaler`, `echec_de_reveil`).
- [ ] **Step 4 : le garde d'armement.** `presse_papier::actif()` est testé
      **AVANT** le sondage, sur le patron de `fenetre.rs:375`
      (`if plein_ecran::actif() && dernier_style.elapsed() >= …`) : `PRESSE_PAPIER=0`
      doit empêcher jusqu'à la **lecture** du compteur, pas seulement l'envoi.
- [ ] **Step 5 : le bras de relais** dans `transitions.rs`, calqué mot pour mot
      sur celui de `Message::Audio` (`:162-170`) :
      `deposer(AEcrire::Etat(DepuisCapteur::PressePapier { texte, octets }), …)`.
      🔴 **`deposer`, jamais un `send` bloquant** — c'est le cinquième
      ingrédient du patron §2.3, et attendre sur une file pleine sans servir
      les commandes recréerait l'interblocage à six maillons de la tâche 10 de
      D4. Le commentaire de `transitions.rs:126-129` le dit déjà pour `Sommeil` ;
      **le répéter ici ou y renvoyer**.
- [ ] **Step 6 : une trace, une seule.** `tracing::info!(octets, refus = texte.is_none(),
      "presse-papier de la VM")`, **au changement seulement**. ⚠️ **Ne jamais
      journaliser le texte** (D-P1-7). ⚠️ **Une seule trace** : deux traces au
      même instant se comptent comme deux événements — piège maison de D6.
- [ ] **Step 7 :** `cargo test -p agent` → **564**, inchangé.
      `cargo check --target x86_64-pc-windows-gnu` → **sortie 0**.

---

### Task 14 : `scripts/run-agent.sh` — `PRESSE_PAPIER` — TÂCHE DÉDIÉE

**Objet :** une ligne. **Elle a sa propre tâche parce que ce dépôt l'a oubliée
trois fois** — D1 (`SUPERVISEUR`), D2 (`MULTIFENETRE_REPRISE`), D7 (`AUDIO`) —
et que chaque fois l'agent a démarré sans la variable **sans rien signaler**.
D6 avait consacré sa tâche 9 à cette seule ligne pour `BUDGET_BPS`, et le
piège a été évité.

**Files:**
- Modify: `scripts/run-agent.sh`

**Relevé : 130 lignes.**

- [ ] **Step 1 :** ajouter `${PRESSE_PAPIER:+\$env:PRESSE_PAPIER = '$PRESSE_PAPIER'}`
      dans le bloc d'amorçage, à côté de `${PLEIN_ECRAN:+…}` (`run-agent.sh:35`).
- [ ] **Step 2 : vérifier que la ligne de la tâche 1 (`PRESSE_PAPIER_SONDE`) y
      est toujours** — deux variables au même préfixe, et une seule d'entre
      elles doit jamais accompagner `SUPERVISEUR` (E10).
- [ ] **Step 3 :** `grep -n 'PRESSE_PAPIER' scripts/run-agent.sh` → **deux**
      lignes, et **relire la sortie**. Le contrôle qui vaut n'est pas que la
      ligne soit écrite mais que la variable **atteigne le processus** : c'est
      le critère ④ de la recette qui le mesure, pas cette tâche.

---

# Famille 5 — le navigateur

### Task 15 : `client/src/main.ts` — la branche `onControl` et le câblage

**Objet :** brancher le module pur sur `navigator.clipboard`. **Aucune logique
dans `main.ts`.**

**Files:**
- Modify: `client/src/main.ts`

**Relevé : 451 lignes, marge 49. 🔴 PORTE DE MESURE obligatoire au Step 4.**

- [ ] **Step 0 : `git log --oneline -5 -- client/`** — périmètre concurrent.
- [ ] **Step 1 : la branche**, dans la chaîne `else if` d'`onControl`
      (`main.ts:131-224`), après `capabilities` :

```ts
} else if (message.type === 'clipboard') {
    pressePapier.recevoir({ texte: message.text, octets: message.bytes });
    ecrirePressePapierSiPossible();
}
```

- [ ] **Step 2 : le câblage**, dans le `.then((session) => …)` (`main.ts:227…`),
      à côté de `armerLeSon` :
      - `ecrirePressePapierSiPossible()` demande `pressePapier.aEcrire(document.hasFocus())` ;
        si un texte revient, `navigator.clipboard.writeText(texte)` →
        `.then(() => pressePapier.confirmer(texte))` →
        `.catch(() => { const m = pressePapier.echouer(); if (m) statut.afficher(m, { persistant: true }); })`.
      - un écouteur `focus` sur `window` rappelle `ecrirePressePapierSiPossible` —
        **c'est le dépôt différé de D3**, et sa seule ligne de DOM.
      - `pressePapier.refusADire()` → `statut.afficher(…, { persistant: true })`,
        sur le patron **exact** du micro (`main.ts:283`).
      - le détacheur est ajouté à la liste de nettoyage existante, comme
        `detacherPleinEcran`.
- [ ] **Step 3 : 🔴 ne JAMAIS appeler `readText()`.** Ni au focus, ni au clic,
      ni jamais — c'est le geste de l'ancien produit (`web/index.js:349-364`),
      il exige la permission (R1), et il lit une ressource privée **en dehors
      de toute intention de collage**. **Le nouveau produit ne demande aucune
      permission de presse-papier**, et c'est le meilleur résultat de ce
      chantier.
- [ ] **Step 4 : 🔴 PORTE DE MESURE.** `wc -l client/src/main.ts` **avant**
      (attendu **451**) et **après**. **Si l'après dépasse 480, extraire** —
      point de chute nommé : `client/src/presse-papier-dom.ts`, sur le patron
      d'`attachFullscreenAuDOM` (`client/src/fullscreen.ts`). **Extraction,
      jamais compression** : ce dépôt interdit la seconde nommément.
- [ ] **Step 5 : voir vert.** `cd client && npx vitest run` → **197**,
      inchangé (aucun test de `main.ts` : il n'a pas de couverture, et ce plan
      n'en crée pas — **déclaré**). `npx tsc --noEmit` → sortie 0.

---

# Famille 6 — recette et clôture

### Task 16 : la recette sur la VM — quatre critères, DEUX exécutions chacun

**Objet :** mesurer. **Aucune ligne de code de produit.**

**Files:**
- Create: `docs/superpowers/plans/journaux-presse-papier-p1/` — les journaux
  d'agent bruts **et** leurs `-plat`, les JSON de pilote, et l'instrument
  lui-même dans `journaux-presse-papier-p1/instrument/`
- Modify: aucun

- [ ] **Step 0 : 🔴 LE TÉMOIN DE MESURABILITÉ, joué AVANT tout critère** (E11).
      Une page pilotée appelle `navigator.clipboard.writeText('temoin-<nonce>')` ;
      l'hôte lit `xclip -o -selection clipboard` (ou `wl-paste`).
      - le nonce revient ⟹ le critère ① est mesurable **au niveau 2**, le vrai ;
      - le nonce ne revient pas ⟹ **le niveau 2 est NON MESURABLE sur ce
        montage**, et le relevé l'écrit. **Ce n'est pas un échec du produit,
        c'est une mesure non prise**, et le critère ① tombe alors au niveau 1
        avec sa réserve écrite en toutes lettres.
      🔴 **Ce témoin peut échouer, et c'est ce qui en fait un témoin.**
- [ ] **Step 1 : l'environnement.** VM démarrée et `/media/vm` **réellement
      accessible** ; `.env` sourcé ; `Get-Process agent` vide ;
      `MULTIFENETRE_VDD_PURGE=1` **dans un lancement séparé** (l'aiguillage
      retourne après la première sonde reconnue) ; le navigateur pilote tourne
      sur l'**HÔTE**, jamais sur la VM (sur la VM la page-shell est elle-même
      capturée, et cela boucle en cascade d'ouvertures).
- [ ] **Step 2 : `grep 'sortie créée mais introuvable'`** avant de conclure à
      quoi que ce soit sur le nombre de fenêtres — le blocage par pollution du
      registre a plafonné D9 à trois fenêtres. **P1 n'a besoin que d'UNE**,
      mais un plafond mal attribué ferait conclure de travers.
- [ ] **Step 3 : les quatre critères. DEUX exécutions chacun. Aucun taux n'est
      revendiqué.**

| # | Critère | Comment il est jugé | 🔴 Ce qui le rend ROUGE |
| --- | --- | --- | --- |
| ① | Copier du texte dans le Bloc-notes de la VM le rend collable dans une application **LOCALE** | **niveau 2** : `xclip -o -selection clipboard` sur l'hôte rend le nonce copié dans la VM. **niveau 1** (repli déclaré, si le Step 0 a échoué) : le texte réellement passé à `writeText` **et** la résolution de sa promesse, relevés par l'instrument. ⚠️ **Jamais sur une ligne de journal** : elle apparaîtrait aussi sur un mécanisme qui ne lit rien | le bras de `pont_media.rs` retiré — **rouge DÉJÀ JOUÉE en tâche 9, sur l'hôte, et versée** (E13). Elle **n'est pas rejouée ici**, et c'est déclaré |
| ② | Un texte **inchangé** recopié ne produit **aucun** message | compter les `AgentControl::Clipboard` reçus par la page entre deux copies identiques : **un**, pas deux | désarmer le garde n°2. ⚠️ **CONDITIONNÉ PAR LA TÂCHE 2** : si Q2 dit que le compteur **ne bouge pas** sur une réécriture identique, **aucun message ne partirait même sans le garde**, et le critère est **NON MESURABLE** — **il faut alors l'écrire**, comme la spec l'ordonne. La rouge d'hôte équivalente (tâche 4) reste, elle, **inconditionnelle** |
| ③ | Un texte au-dessus de `PRESSE_PAPIER_MAX` est **refusé et dit**, jamais tronqué | copier **100 KiB** dans la VM ; lire **`#status.textContent` ET `#status.dataset.hidden === 'false'`** (E12) ; et vérifier que le presse-papier local est **inchangé** | un presse-papier local qui contient 64 KiB du texte. ⚠️ **`#status` est le bandeau des pages d'APPLICATION ; `#statut` est celui de la page-shell** — une exécution entière de D5 a lu le mauvais et n'a rien vu |
| ④ | `PRESSE_PAPIER=0` désarme | **A/B, deux exécutions par bras** : sans la variable, **≥ 1** `AgentControl::Clipboard` ; avec `PRESSE_PAPIER=0`, **exactement 0**, et la trace `presse-papier DESARME` présente | ⚠️ **Le contrôle qui vaut est le ZÉRO de messages, pas la trace** : la trace prouve que la variable est arrivée, elle ne prouve pas que le mécanisme est coupé. Et le bras **sans** la variable est ce qui rend le zéro discriminant — un zéro seul serait rendu par un produit entièrement en panne |

- [ ] **Step 4 : ordre de la copie.** Copier **APRÈS** que la session est
      établie — D-P1-4 : l'état de référence est pris au premier sondage, et
      une copie antérieure n'est **jamais** annoncée. Un protocole qui
      copierait avant mesurerait zéro sur un produit correct.
- [ ] **Step 5 : appel BLOQUANT au premier plan** pour toute exécution longue.
      Une commande backgroundée par le harnais **ne survit pas** à la fin du
      tour de l'agent qui l'a lancée, et **deux recettes de D10 y ont perdu une
      exécution chacune** — le symptôme est un journal **tronqué** copié depuis
      une VM où l'agent continue de tourner.
- [ ] **Step 6 : copier `agent.log` APRÈS la fin réelle**, pas à la fin du
      pilote : les enfants meurent quand le navigateur se ferme, donc **après**
      la copie.
- [ ] **Step 7 :** `grep -a` sur tout journal de pilote (octets de contrôle
      isolés du PowerShell de `run-agent.sh`), et `grep -a` aussi sur les
      journaux d'agent — une queue d'octets NUL fait classer le fichier
      « binaire » et **rendre une sortie VIDE, pas zéro** (piège de D10).
- [ ] **Step 8 : verser tous les journaux dans git**, bruts **et** `-plat`, et
      l'instrument dans son **état final**.

---

### Task 17 : la revue transverse de fin de branche — OBLIGATOIRE

**Objet :** chercher les défauts qui **franchissent une frontière de tâche**, et
que treize revues par tâche ne peuvent structurellement pas voir.

**Barème du dépôt, à titre d'ordre de grandeur, jamais de quota :** D7 **5**,
D8 **3**, D9 **6**, D10 **douze**, D11 **sept**, P1 (plateforme) **huit**,
P2 (plateforme) **dix**, S1 **cinq**, E **neuf**.

**Files:**
- Modify: ceux que la revue désigne, et eux seuls

- [ ] **Step 1 : la cible propre de cette revue — les affirmations de code
      devenues fausses dans leur propre branche.** C'est le mode de
      défaillance dominant de ce dépôt : D10 en a trouvé **sept** de suite, et
      D9 **trois**. Balayer par le **SENS**, pas par la formule.
- [ ] **Step 2 : les questions à poser explicitement**, chacune ayant déjà
      coûté une ronde ailleurs :
      1. `tick.rs:67-68` énumère les branches qui « ne touchent même pas
         `self.rtc` » — `a1septies` y est-elle ? *(D6 a payé DEUX rondes pour
         avoir laissé cet énoncé faux, aux DEUX endroits du même fichier.)*
      2. Le doc-comment de `AgentControl::Clipboard` promet-il quelque chose
         que le refus de taille contredit ?
      3. Le commentaire d'`Ok(autre)` de `pont_media.rs` dit-il toujours le bon
         nombre de précédents ? **Il en faut cinq, pas quatre.**
      4. La spec §6 « P1 » écrit « Aucun garde anti-écho n'est nécessaire » —
         le code livre pourtant le **garde n°2**. Le commentaire du `Sondeur`
         dit-il **pourquoi** (absorber le faux positif de D2), et non « pour
         fermer la boucle » qui serait faux ?
      5. Le tableau des variables de `CLAUDE.md` (tâche 18) porte-t-il **les
         deux** variables, avec la convention **inverse** de l'une par rapport
         à l'autre (`PRESSE_PAPIER=0` désarme ; `PRESSE_PAPIER_SONDE` absente
         = désarmée) ?
      6. Un commentaire dit-il « la recette qui l'exercerait n'a pas encore
         tourné » alors qu'elle a tourné ? *(C'est le septième défaut de D10,
         « le plus pur ».)*
- [ ] **Step 3 : chercher les CONTRÔLES INCAPABLES D'ÉCHOUER**, dans le code
      **et dans ce plan**. D10 en a attrapé quatre, dont **trois écrits par le
      plan lui-même**. Candidats désignés d'avance ici : le témoin de
      mesurabilité du Step 0 de la tâche 16, le critère ④, et le test
      `TYPES_AGENT` de la tâche 7.
- [ ] **Step 4 : chercher les PIÈCES FABRIQUÉES.** D10 en a trouvé deux, et
      l'implémenteur a nommé le mécanisme : **réutiliser la sortie d'une
      commande antérieure pour répondre à la question d'une AUTRE, sans la
      relancer.** Relancer au moins un `wc -l` et un `cargo test` de la branche
      et comparer aux nombres publiés.
- [ ] **Step 5 : écrire les constats dans le document de RÉSULTATS**, jamais
      dans un rapport gitignoré. L'espace de travail de D9 a disparu avec
      **six** constats qui sont **définitivement perdus**.

---

### Task 18 : `CLAUDE.md` gagne sa section, et ses tailles sont RELEVÉES

**Objet :** l'index de connaissances. 🔴 **Toutes les tailles sont relevées par
la commande, APRÈS les dernières éditions de la branche, revue transverse
comprise** — une table relevée en début de ronde serait fausse à la fin de la
même ronde, erreur que D8 a commise en croyant bien faire.

**Files:**
- Modify: `CLAUDE.md`

⚠️ **`CLAUDE.md` est sous périmètre concurrent.** Vérifier
`git log --oneline -5 -- CLAUDE.md` **juste avant**, et relire le diff.

- [ ] **Step 1 : relancer la commande des 500 lignes** et écrire le tableau des
      fichiers que la branche a fait bouger — **tous mesurés**, aucun recopié.
- [ ] **Step 2 : corriger les DEUX chiffres dérivés relevés par ce plan** —
      `agent/src/transport.rs` **491 → 495** (`CLAUDE.md:723`) et
      `client/verify-webrtc.mjs` **497 → 494** (`CLAUDE.md:598`, et **quatre
      autres places**, l. 529, 531, 613, 666).
      🔴 **« Corrigé à sa place » est une affirmation de COMPLÉTUDE, et elle se
      vérifie en énumérant les places AVANT d'écrire** : `grep -n '497' CLAUDE.md`,
      `grep -n '491' CLAUDE.md`. Le naufrage du « 487 » s'est rejoué **neuf**
      fois dans ce fichier.
      🔴 **Puis RELIRE place par place APRÈS l'édition** — « la leçon n'est pas
      `mesurer`, qui avait été fait : c'est que `grep -n` doit être relu place
      par place APRÈS l'édition, pas seulement lancé avant ».
      ⚠️ **Ne barrer que ce qui est FAUX** : un énoncé **daté** (« relevé le 19
      août 2026 ») reste vrai comme histoire, et le barrer le rendrait faux.
- [ ] **Step 3 : les deux variables au tableau du banc et du produit**, avec
      leurs conventions **inverses**, écrites côte à côte :
      - `PRESSE_PAPIER=0` — **variable de PRODUIT**. `=0` DÉSACTIVE, une simple
        présence n'active pas. Lue dans le **capteur** (le propriétaire, D1),
        par `OnceLock`. Transmise par `scripts/run-agent.sh`. Trace :
        `presse-papier DESARME (PRESSE_PAPIER=0)`.
      - `PRESSE_PAPIER_SONDE=<secondes>` — **variable de BANC, jamais une
        configuration livrée**. **ABSENTE = DÉSARMÉE.** 🔴 **Ne jamais
        coexister avec `SUPERVISEUR`** : l'aiguillage de `diagnostics` précède
        le bras `CAPTEUR` dans `main.rs`, et le capteur hérite de
        l'environnement — le processus capteur exécuterait la sonde à la place
        de son service (E10). ⚠️ **Elle ÉCRIT le presse-papier de la VM** en
        phases C et D.
- [ ] **Step 4 : la section du sous-bloc**, placée entre « Chantier E —
      Microphone, bloc E1 » (`CLAUDE.md:8368`) et « 🚀 Commandes de
      Développement Essentielles » (`:8765`), et **titrée sans ambiguïté** :
      *« 📋 Sous-projet ① Divers — presse-papier, sous-bloc P1 : la VM copie, le
      navigateur colle »*. **Elle dit d'emblée que ce P1 n'est pas celui de la
      plateforme.**
- [ ] **Step 5 : elle porte, dans cet ordre** : le verdict de la sonde
      d'ouverture (tâche 2) avec **son nombre d'exécutions** ; les quatre
      critères avec leur verdict et leur nombre d'exécutions ; **ce que P1
      N'ÉTABLIT PAS** ; les pièges neufs ; et les legs.

---

### Task 19 : clore le document de résultats

**Objet :** le document permanent, versé dans git, qui porte l'analyse **et sa
preuve**. La preuve d'une affirmation de ce dépôt ne vit **jamais** dans un
rapport gitignoré (leçon de D9, tâche 18 de D10).

**Files:**
- Create: `docs/superpowers/plans/2026-08-19-presse-papier-p1-resultats.md`

- [ ] **Step 1 : la note de lecture des journaux** — familles d'encodage, fins
      de ligne, séquences ANSI, et si `grep -a` est requis. **Relevée par
      `file` et par un `grep` d'essai**, jamais supposée.
- [ ] **Step 2 : chaque énoncé porte son NOMBRE D'EXÉCUTIONS.** Aucun taux
      n'est revendiqué nulle part.
- [ ] **Step 3 : la section « Ce que P1 n'établit PAS »**, qui doit au minimum
      contenir :
      - **aucun taux**, deux exécutions par critère au mieux ;
      - **rien d'un navigateur autre que Chromium**, rien d'un Chromium **avec
        interface** ; aucune fenêtre de dialogue de permission n'a jamais été
        montrée à un humain ;
      - **rien au-delà d'UNE fenêtre** — c'est P3 ;
      - **le sens navigateur → VM n'est pas touché** — c'est P2, et son
        préalable éliminatoire (`paste` sur un `<video>` focalisé) **n'est
        toujours pas mesuré** ;
      - **`writeText` depuis une fenêtre NON focalisée n'est pas mesuré** — la
        règle du dépôt différé est livrée sur une contrainte **supposée**, que
        le critère ④ de P3 peut **réfuter** ;
      - **`PRESSE_PAPIER_MAX` (64 KiB) et `PERIODE_PRESSE_PAPIER` (250 ms) ne
        sont pas calibrées** — elles rejoignent `BPP_MIN`, `FACTEUR_FOCUS`,
        `PART_DORMANTE_BPS`, `HYSTERESIS`, `REPIT_APRES_ECHEC`,
        `TAILLE_MAX_SORTIE`, `REPIT_REARMEMENT_AUDIO`, `REARMEMENTS_MAX` ;
      - **le propriétaire mono-fenêtre n'existe pas** (E6, leg n°1) ;
      - **rien de la latence de bout en bout**, qu'aucun sous-bloc du chantier
        D n'a jamais mesurée ;
      - **le niveau réellement atteint par le critère ①** — niveau 2, ou
        niveau 1 avec sa réserve (E11).
- [ ] **Step 4 : les legs**, repris tels quels de la section « Ce que P1 lègue »
      ci-dessous.
- [ ] **Step 5 : vérifier que TOUTES les pièces citées sont versées** —
      `git status --porcelain docs/superpowers/plans/journaux-presse-papier-p1/`
      doit être **vide** après le commit.

---

## Ce que ce plan NE prescrit PAS, et pourquoi

- **Aucun garde anti-écho n°1 ni n°3.** P1 n'écrit jamais le presse-papier
  Windows : la boucle qu'ils ferment ne peut pas exister. Les livrer ici serait
  livrer du code jamais couru — et le dépôt sait ce que cela coûte
  (`borner_a_la_taille_max`, restée sans appelant un sous-bloc entier).
  **P2 les livre, et il les livre dès sa naissance**, parce qu'un sous-bloc ne
  livre pas un défaut qu'il crée.
- **Aucune modification de `client/src/input.ts`.** Le `event.preventDefault()`
  inconditionnel de `:94-95`, sur `window` (`:111`), **empêche aujourd'hui tout
  événement `paste` dans la fenêtre de session** — vérifié à l'écriture de ce
  plan. **C'est le sujet de P2, et P1 n'y touche pas.**
- **Aucune modification de `agent/src/superviseur/lanceur.rs`.** Voir E10 : la
  parade est un protocole de lancement, pas un `env_remove` qui poserait une
  convention que les huit variables `MULTIFENETRE_*` ne suivent pas.
- **Aucun test de `client/src/main.ts`.** Il n'en a aucun aujourd'hui, et ce
  plan n'en crée pas — **déclaré, pas dissimulé**. C'est ce qui justifie que
  toute la logique aille dans `presse-papier.ts`.
- **Aucune correction de `CLAUDE.md` hors tâche 18**, périmètre concurrent.
- **Aucun jugement visuel ni d'écoute.** P1 n'a rien à regarder.

---

## Ce que P1 lègue

1. ⛔ **Le propriétaire MONO-FENÊTRE n'existe pas.** D1 pose « le capteur quand
   il existe, l'enfant sinon » ; P1 ne livre que le capteur. Un agent
   mono-fenêtre (ni `SUPERVISEUR` ni `CAPTEUR`, `agent/src/main.rs:434`) n'a
   donc **aucun** presse-papier. **Ce n'est pas une régression** — il n'en avait
   pas non plus —, mais c'est exactement la forme du legs n°4 de D10 (« le
   remède est INERTE en mono-fenêtre »), et il faut le nommer avant qu'un
   commentaire n'affirme le contraire. Point de chute : `agent/src/demarrage.rs`
   (**491 lignes, marge 9** — 🔴 **une extraction préalable y sera requise**).
2. ⛔ **`Capabilities.clipboard`** (D7, E6, D-P1-6) — reporté à P2, où il sert.
3. ⛔ **Une fenêtre attachée après une copie ne reçoit jamais ce contenu** (E5).
   Sans conséquence à une fenêtre, **réel en P3**. Remède nommé : émettre l'état
   courant à l'inscription, comme `parts::distribuer_les_parts` le fait déjà
   depuis `inscrire` (`registre.rs:325`).
4. ⛔ **Le canal `Message` du registre est non borné** (E9), et P1 y fait
   circuler jusqu'à 64 KiB par fenêtre et par changement. Nommé, non corrigé.
5. ⛔ **Le préalable éliminatoire de P2 n'est toujours pas mesuré** : l'événement
   `paste` parvient-il quand le focus est sur le `<video>` ? R2 a été mesuré
   avec le focus sur un `body`. **Si la réponse est non, P2 est bloqué** et la
   conception se replie sur `readText()` avec permission — c'est-à-dire sur
   l'ancien produit.
6. ⛔ **Le niveau 2 du critère ① peut n'avoir jamais été atteint** (E11). Si le
   témoin du Step 0 a échoué, **personne n'a jamais vérifié qu'un humain peut
   coller**, et la voie qui le permettrait est nommée : `Xvfb` + `xdotool`, dont
   le consentement d'installation a été **donné en D8 et jamais suivi d'effet**.
   ⚠️ Les mesures qui en sortiraient **ne se compareraient à aucune campagne
   antérieure**.
7. ⛔ **`PRESSE_PAPIER_MAX` et `PERIODE_PRESSE_PAPIER` ne sont pas calibrées.**
8. ⛔ **Le presse-papier reste PARTAGÉ entre les fenêtres d'une même session**
   (D3), et **entre deux utilisateurs d'une même VM** (R11 de la spec) — la
   seconde est une question de **confidentialité**, elle est **réelle**, elle
   est hors périmètre v1, et elle appartient à ⑤.

---

## Risques qui rendraient ce sous-bloc NON LIVRABLE

| # | Risque | Gravité | Ce qui le lève, ou le borne |
| --- | --- | --- | --- |
| **RP1** | 🔴 **`GetClipboardSequenceNumber` ne se comporte pas comme la documentation l'annonce** — il rend 0, ou ne bouge pas | **éliminatoire pour D2** | **Tâche 2, jouée en premier**, avec ses cinq verdicts écrits d'avance. Sous V1/V2 le trajet reste valide mais la détection se rouvre |
| **RP2** | 🔴 **Le bras de `pont_media.rs` est oublié** | tue la session au premier message, **sans panne apparente** | Tâche 9, **rouge jouée avant le vert**, sur l'hôte, versée. **Cinquième rappel d'un défaut payé quatre fois** |
| **RP3** | 🔴 **Le type est oublié dans `TYPES_AGENT`** | message perdu contre un `console.warn`, **aucun test ne le voit aujourd'hui** | Tâche 7 : remède **structurel**, `Record<AgentControl['type'], true>`, dont la rouge est un échec `tsc` **atteignable et vérifié** |
| **RP4** | 🔴 **Le montage de recette ne peut pas lire ce que `writeText` a écrit** | le critère ① tombe au niveau 1 | E11 : **témoin de mesurabilité joué en premier**, et non-mesurabilité **écrite** si le témoin échoue. **Ce n'est pas un échec du produit** |
| **RP5** | 🔴 **Le sondage bloque le verrou global du capteur** | fige l'attache et le retrait de **toutes** les fenêtres | D-P1-3/E3 : lecture **hors verrou**, distribution **sous verrou**. Prescrit, pas découvert |
| **RP6** | ⚠️ **`OpenClipboard` échoue et la référence avance quand même** | le contenu copié est **perdu à jamais**, en silence | D-P1-5, et son test d'hôte avec spy |
| **RP7** | ⚠️ **`PRESSE_PAPIER_SONDE` hérité par le processus capteur** | le capteur exécute la sonde au lieu de servir, et le superviseur le relance en boucle | E10 : lancement séparé, commentaire dans le code, ligne dans `CLAUDE.md` |
| **RP8** | ⚠️ **Le presse-papier est laissé OUVERT** sur un chemin d'erreur | bloque **toute la window station**, pas seulement l'agent | Tâche 12 : garde RAII, fermeture sur **tous** les chemins |
| **RP9** | ⚠️ **`proto/src/control.rs` franchit 500** | dette neuve sur un fichier partagé | Tâche 3, extraction **avant** l'addition, et la tâche 6 s'arrête si elle constate 470 |
| **RP10** | ⚠️ **`distante/tests.rs` (474) ou `main.ts` (451) franchissent leur plafond** | dette neuve, **et la spec ne signalait pas la première** | E1 : portes de mesure obligatoires, points de chute nommés |
| **RP11** | ⚠️ **Un texte du presse-papier atterrit dans un journal versé dans git** | **confidentialité** | D-P1-7 : empreinte et longueur, jamais le contenu. À vérifier en revue transverse |
| **RP12** | ⚠️ **La VM s'hiberne en pleine mesure** | exécution perdue, agents survivants | Vérifier la survie de la VM **après chaque rang**, et `Get-Process agent` après **chaque** tentative, y compris échouée |
