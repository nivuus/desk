# Sous-bloc D9 — solder la dette du chantier D — plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fermer les douze legs ouverts au sortir du sous-bloc D8 — six propres à D8, deux de D7 et trois de D6 jamais traités, plus trois défauts d'instrument.

**Architecture :** Une **phase P** de sonde de banc tranche l'inconnue éliminatoire (un changement de mode de sortie est-il accepté sur une sortie dont la duplication DXGI est ouverte ?) avant qu'une ligne de la famille ① ne s'écrive. Les trois familles suivent : ① armer le plein écran, ② l'audio mort et l'identité de session, ③ la dette de mesure de D6. Les familles ② et ③ ne croisent pas le verdict de P et s'écrivent en parallèle.

**Tech Stack :** Rust 2021 (`agent/`), TypeScript + Vite (`client/`), Node.js pour les pilotes de recette, PowerShell distant via WinRM pour la VM Windows.

Spécification : `docs/superpowers/specs/2026-08-06-multifenetres-solder-la-dette-design.md`

## Global Constraints

- **Plafond de 500 lignes** par fichier de code source écrit à la main. Aucun **nouveau** fichier ne naît au-dessus ; un fichier déjà au-dessus ne grossit pas sans extraction. Vérifier par la commande, jamais par recopie d'un tableau :
  ```bash
  { git ls-files; git ls-files --others --exclude-standard; } \
    | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
    | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
  ```
- **`agent/src/capteur/serveur.rs` vaut 490 lignes, marge 10** (relevé par la commande le 6 août 2026). La tâche 6 porte son extraction, **avant** que la tâche 9 n'y ajoute quoi que ce soit.
- **`agent/src/windows_source.rs` vaut 638 lignes, dette gelée** sous condition « la prochaine addition exige une extraction ». La tâche 11 l'**allège** ; aucune autre tâche n'y ajoute de ligne.
- **Compilation croisée avant toute compilation distante** : `cd agent && cargo check --target x86_64-pc-windows-gnu`. Couvre types, emprunts, visibilités et durées de vie ; **ne couvre PAS l'édition de liens** (la cible réelle est `msvc` sur la VM).
- **Toute variable d'environnement neuve va dans `scripts/run-agent.sh` dans la tâche même qui la crée.** Piège payé en D1 (`SUPERVISEUR`), D2 (`MULTIFENETRE_REPRISE`) et D6 (`BUDGET_BPS`) : sans cette ligne l'agent démarre sans la variable et ne le signale pas.
- **Aucune constante non calibrée n'est présentée comme calibrée.** Le commentaire dit ce qui a été mesuré et ce qui ne l'a pas été.
- **Un contrôle qu'on n'a jamais vu ROUGE n'est pas un contrôle.** Chaque critère de recette se joue d'abord sans son remède, et la pièce du rouge est versée.
- **Journaux d'agent** : `sed 's/\x1b\[[0-9;]*m//g' agent.log > agent-plat.log` avant tout `grep` — les séquences ANSI de `tracing` séparent le nom du champ de sa valeur.
- **La VM n'est pas allumée par défaut.** `virsh list --all`, puis `virsh start Windows`, puis attendre le montage réel (`until ls /media/vm/dev >/dev/null 2>&1; do sleep 5; done`), et **sourcer `.env`** (`set -a && source .env && set +a`) avant tout `scripts/build-agent.sh`, qui s'arrête sinon **en silence**.
- **Vérifier `Get-Process agent` avant CHAQUE tentative de recette, y compris après une tentative échouée** : un superviseur survivant tient `agent.log` et l'on relit alors le journal du run précédent en croyant lire le sien.

---

## Structure des fichiers

**Phase P**

| Fichier | Responsabilité |
| --- | --- |
| `agent/src/diagnostics/multifenetre/mode_sortie.rs` (421) | sonde de banc : quatrième combinaison de drapeaux, duplication ouverte, témoin sans duplication |

**Famille ① — armer le plein écran**

| Fichier | Responsabilité |
| --- | --- |
| `agent/src/windows_source/redimensionnement/mode_sortie.rs` (458) | drapeaux retenus, encadrement relâcher/changer/rouvrir |
| `agent/src/windows_source/redimensionnement.rs` (345) | C2 — la réouverture retentable emprunte la reprise de D2 |
| `client/src/resize.ts` **(créer)** | règle **pure** du rejeu de `Resize` — testable sans DOM |
| `client/src/resize.test.ts` **(créer)** | ses tests |
| `client/src/main.ts` (295) | unité du viewport (dpr), câblage du rejeu |
| `agent/src/demarrage.rs` (457) | champ `session` sur la trace `contrôle reçu` |

**Famille ② — l'audio mort et l'identité de session**

| Fichier | Responsabilité |
| --- | --- |
| `agent/src/capteur/serveur/instances.rs` **(créer)** | création et acceptation des instances de tube, extraites de `serveur.rs` |
| `agent/src/capteur/serveur.rs` (490 → allégé) | boucle de service, dispatch des commandes |
| `agent/src/capteur/audio.rs` (152) | règle **pure** : une fenêtre inapte ne porte jamais le son |
| `agent/src/capteur/sommeil.rs` (327) | registre : inaptitudes, réarmements, générations |
| `agent/src/capteur/sommeil/porteurs.rs` (184) | remplit `inapte` depuis le registre, expire le répit |
| `agent/src/capteur/protocole.rs` (369) | `VersCapteur::AudioMort`, `generation` sur `Attache` |
| `agent/src/capteur/fenetre/commandes.rs` (192) | bras `AudioMort` du fil de fenêtre |
| `agent/src/source.rs` (362) | `VideoSource::signaler_audio_mort`, défaut inerte |
| `agent/src/capteur/distante.rs` (375) | l'implémente en émettant `AudioMort` |
| `agent/src/transport/tick.rs` (308) | détecte la transition et signale une seule fois |
| `agent/src/superviseur/boucle.rs` (492) | frappe la génération au lancement |

**Famille ③ — la dette de mesure de D6, et l'instrument**

| Fichier | Responsabilité |
| --- | --- |
| `agent/src/windows_source/telemetrie.rs` **(créer)** | compteurs de capture **par session**, purs et testables |
| `agent/src/windows_source.rs` (638 → allégé) | perd ses trois statiques |
| `agent/src/capteur/fenetre.rs` (470) | lit et journalise la télémétrie sous son span `session` |
| `agent/src/transport/part.rs` (274) | neutralisation de `set_desired_bitrate` sous variable de banc |
| `scripts/run-agent.sh` (122) | transmet la variable neuve |
| `docs/superpowers/plans/journaux-multifenetres-d8/instrument/pilote-recette-d8.mjs` | trois correctifs d'instrument |

---

## Phase P — trancher l'éliminatoire

### Task 1 : la sonde éliminatoire, avec sa contre-épreuve

**Files:**
- Modify: `agent/src/diagnostics/multifenetre/mode_sortie.rs` (421 lignes, marge 79)

**Interfaces:**
- Consumes: `crate::capture::DesktopCapture::sur_sortie(nom: &str)`, `crate::moniteurs_virtuels` pour créer/détruire une sortie
- Produces: rien pour les tâches suivantes — la sonde rend un **verdict au journal**, lu par la tâche 2

**Contexte.** Le module éprouve aujourd'hui **trois** combinaisons de drapeaux (`mode_sortie.rs:256-259`), qui portent **toutes** `CDS_UPDATEREGISTRY`, et il **n'ouvre jamais de duplication DXGI**. Or le produit retaille une sortie dont la duplication est ouverte et détenue jusqu'à 3,1 s. C'est l'écart banc/produit que D8 a laissé béant.

- [ ] **Step 1 : ajouter la quatrième combinaison de drapeaux**

Dans la liste des combinaisons (`mode_sortie.rs:256-259`), en **première** position — la moins insistante d'abord, comme le veut l'ordre croissant déjà en place :

```rust
Combo::Simple("aucun drapeau (dynamique, non persisté)", CDS_TYPE(0)),
Combo::Simple("CDS_UPDATEREGISTRY seul", CDS_UPDATEREGISTRY),
Combo::Simple("CDS_UPDATEREGISTRY | CDS_RESET", CDS_UPDATEREGISTRY | CDS_RESET),
Combo::Deux(
    "CDS_UPDATEREGISTRY|CDS_NORESET puis CDS_RESET seul (idiome multi-ecran)",
    // ... inchangé
),
```

Documenter en tête de la liste **pourquoi** ce bras existe :

```rust
// `CDS_TYPE(0)` — changement DYNAMIQUE, non écrit au registre. C'est le
// remède candidat de C1 : le produit écrit aujourd'hui `CDS_UPDATEREGISTRY`
// à chaque plein écran réussi, et une sortie NAÎT à la dernière taille
// laissée au registre (chaîne `avant(N) = après(N-1)`, tâche 3bis de D8) —
// si bien qu'il bloquerait ses propres ouvertures de fenêtre ultérieures.
// Les trois combinaisons éprouvées par D8 portaient TOUTES
// `CDS_UPDATEREGISTRY` : ce bras-ci n'a jamais été tenté.
```

- [ ] **Step 2 : ouvrir une duplication DXGI et la tenir pendant la tentative**

Après la création de la sortie et **avant** la première tentative de changement de mode, ouvrir une duplication sur cette sortie et la garder vivante jusqu'à la fin de tous les bras :

```rust
// L'ÉCART BANC/PRODUIT que D8 a laissé béant, et l'objet même de cette
// sonde : la production retaille une sortie DONT LA DUPLICATION EST
// OUVERTE et détenue jusqu'à 3,1 s. P1 n'en ouvrait jamais.
let duplication = DesktopCapture::sur_sortie(&nom_sortie)
    .context("ouverture de la duplication sur la sortie virtuelle neuve")?;
tracing::info!(sortie = %nom_sortie, "duplication ouverte et TENUE pendant les tentatives");
```

- [ ] **Step 3 : ajouter le témoin sans duplication**

Après le tour complet avec duplication, **relâcher** la duplication et rejouer **le même bras gagnant** (ou, si aucun n'a gagné, le premier bras) sur une seconde sortie neuve, sans duplication ouverte :

```rust
// TÉMOIN. Sans lui, un refus s'imputerait à la duplication alors qu'il
// pourrait venir du mode choisi. Le témoin rejoue le MÊME geste sur une
// sortie neuve, duplication fermée.
drop(duplication);
tracing::info!("duplication relâchée — début du témoin sans duplication");
```

- [ ] **Step 4 : verrouiller la contre-épreuve du critère**

Conserver l'acquis de la tâche 3bis de D8 — la cible **exclut structurellement** la taille courante, et la sonde rend `P1 NON MESURABLE` si aucun mode annoncé n'en diffère — et **juger sur le mouvement relu par DXGI, jamais sur le code de retour**. Ajouter en tête du module :

```rust
//! ⚠️ **Le critère juge sur la RELECTURE DXGI, jamais sur le code de retour.**
//! `mode-sortie-1728x1080.log` montre l'idiome `CDS_UPDATEREGISTRY|CDS_NORESET`
//! puis `CDS_RESET` annonçant `0` sur une sortie qui n'a pas bougé d'un pixel :
//! un refus déguisé en succès. Et le premier verdict P1 de D8 était `REÇU`
//! rendu par un critère qui ne pouvait pas rendre l'autre valeur — la sonde
//! demandait à la sortie la taille qu'elle avait déjà.
```

- [ ] **Step 5 : journaliser les deux inconnues annexes**

Dans la même exécution, relever :

```rust
tracing::info!(
    pertes_acces_voisines,
    nom_avant = %nom_sortie,
    nom_apres = %nom_relu,
    nom_conserve = nom_avant == nom_apres,
    "inconnues annexes relevées au même moment que l'éliminatoire"
);
```

`pertes_acces_voisines` se compte en ouvrant **deux** sorties supplémentaires avec leur duplication avant le tour, et en comptant les `0x887a0026` qu'elles rendent pendant les tentatives.

- [ ] **Step 6 : vérifier la compilation croisée**

Run: `cd agent && cargo check --target x86_64-pc-windows-gnu`
Expected: sortie 0. Aucun avertissement neuf dans `diagnostics/`.

- [ ] **Step 7 : vérifier le plafond de lignes**

Run: `wc -l agent/src/diagnostics/multifenetre/mode_sortie.rs`
Expected: < 500. Si dépassé, extraire les bras de combinaison vers `agent/src/diagnostics/multifenetre/mode_sortie/combinaisons.rs`.

- [ ] **Step 8 : commit**

```bash
git add agent/src/diagnostics/multifenetre/mode_sortie.rs
git commit -m "sonde(d9): l'eliminatoire, duplication ouverte et quatrieme combinaison"
```

---

### Task 2 : exécuter la phase P et rendre le verdict

**Files:**
- Create: `docs/superpowers/plans/journaux-multifenetres-d9/p-eliminatoire-{1,2}.log`
- Create: `.superpowers/sdd/2026-08-06-multifenetres-solder-la-dette/task-2-report.md`

**Interfaces:**
- Consumes: la sonde de la tâche 1
- Produces: **le verdict qui commande les tâches 3 à 5** — `ACCEPTE` ou `REFUSE`, et la combinaison de drapeaux retenue

- [ ] **Step 1 : préparer la VM**

```bash
virsh list --all
virsh start Windows 2>/dev/null || true
until timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null; do sleep 5; done
until ls /media/vm/dev >/dev/null 2>&1; do sleep 5; done
set -a && source .env && set +a
```

- [ ] **Step 2 : purger les sorties orphelines, en lancement SÉPARÉ**

⚠️ **Deux variables `MULTIFENETRE_*` dans le même lancement n'enchaînent PAS deux sondes** : l'aiguillage retourne après la première reconnue.

```bash
MULTIFENETRE_VDD_PURGE=1 scripts/run-agent.sh
```

- [ ] **Step 3 : compiler et vérifier que le binaire a bougé**

```bash
scripts/build-agent.sh
ls -l /media/vm/dev/agent/target/release/agent.exe
```
Expected: taille et horodatage changés. ⚠️ Une compilation de 0,13 s est un aveu : après un aller-retour de sources, `cargo clean --release -p agent` est le seul déblocage.

- [ ] **Step 4 : exécuter la sonde, deux fois**

```bash
MULTIFENETRE_MODE_SORTIE=1280x720 scripts/run-agent.sh
cp /media/vm/dev/agent/agent.log docs/superpowers/plans/journaux-multifenetres-d9/p-eliminatoire-1.log
# contrôle de survie de la VM, puis seconde exécution
virsh list --all
MULTIFENETRE_MODE_SORTIE=1280x720 scripts/run-agent.sh
cp /media/vm/dev/agent/agent.log docs/superpowers/plans/journaux-multifenetres-d9/p-eliminatoire-2.log
```

⚠️ **Copier `agent.log` APRÈS la fin réelle de l'exécution**, pas à la fin du pilote.

- [ ] **Step 5 : lire le verdict**

```bash
for f in docs/superpowers/plans/journaux-multifenetres-d9/p-eliminatoire-*.log; do
  sed 's/\x1b\[[0-9;]*m//g' "$f" > "${f%.log}-plat.log"
  grep -E 'combinaison de drapeaux tentee|mouvement_observe|P1 (RECU|NON MESURABLE)|temoin|inconnues annexes' "${f%.log}-plat.log"
done
```

- [ ] **Step 6 : écrire le rapport de verdict**

Le rapport DOIT trancher, en nommant son nombre d'exécutions :

1. **L'éliminatoire** : `ACCEPTE` (mouvement DXGI observé, duplication ouverte) ou `REFUSE`.
2. **C1** : la combinaison `CDS_TYPE(0)` fait-elle bouger la sortie ? Si oui, **C1 disparaît** ; sinon, le repli de la tâche 3 s'applique.
3. **Le témoin** : le même bras sans duplication se comporte-t-il autrement ? Sans écart, l'attribution à la duplication est invalide.
4. **Inconnues annexes** : pertes d'accès voisines, conservation du nom.

⚠️ Si la sonde rend `NON MESURABLE`, **le dire et ne rien conclure** — ce n'est pas un refus, c'est une mesure non prise.

- [ ] **Step 7 : commit**

```bash
git add docs/superpowers/plans/journaux-multifenetres-d9/ .superpowers/
git commit -m "mesure(d9): verdict de l'eliminatoire, deux executions"
```

---

### Task 2bis : la persistance sous `CDS_UPDATEREGISTRY`

**Inscrite le 6 août 2026, APRÈS le verdict de la tâche 2, sur décision de
l'utilisateur.** Elle n'était pas au plan initial : la règle de décision du §3.6
couvrait « le pilote refuse », et la mesure a rendu « il accepte, puis défait ».

**Files:**
- Modify: `agent/src/diagnostics/multifenetre/mode_sortie.rs` (496 lignes, **marge 4**)
- Create: `docs/superpowers/plans/journaux-multifenetres-d9/p-persistance-{1,2}.log`

**Interfaces:**
- Consumes: la sonde des tâches 1 et 2
- Produces: **le verdict qui décide du sort de la famille ①** — `SURVIT` ou `NE SURVIT PAS`

**Ce qui motive cette tâche.** `CDS_TYPE(0)` a réussi **au premier essai** aux
deux exécutions : les trois autres combinaisons **n'ont jamais été sollicitées**.
On ignore donc si le changement écrit sous `CDS_UPDATEREGISTRY` — celui qui
persiste par construction — survivrait, lui, à la création d'une sortie.
Abandonner la famille ① sur la foi d'un bras sur quatre serait trancher sur une
preuve incomplète.

- [ ] **Step 1 : ajouter le sélecteur de combinaison**

Variable de banc `MULTIFENETRE_MODE_SORTIE_DRAPEAUX=<étiquette>` qui restreint le
tour à **une seule** combinaison, désignée par son étiquette exacte. Sans elle,
le comportement actuel est inchangé.

```rust
/// Restreint le tour à UNE combinaison, désignée par son étiquette exacte.
///
/// **Pourquoi** : `CDS_TYPE(0)` réussit au premier essai, ce qui laisse les
/// trois combinaisons suivantes non sollicitées — dont `CDS_UPDATEREGISTRY`,
/// la seule qui persiste par construction. Sans ce sélecteur, la question
/// « le changement PERSISTANT survit-il, lui, à la création d'une sortie ? »
/// n'est pas atteignable.
fn combinaison_imposee() -> Option<String> {
    std::env::var("MULTIFENETRE_MODE_SORTIE_DRAPEAUX").ok()
}
```

⚠️ **La transmettre dans `scripts/run-agent.sh` dans CETTE tâche** — piège payé
en D1, D2 et D6.

- [ ] **Step 2 : journaliser le contrôle de survie comme un VERDICT nommé**

Le retour à la taille d'origine existe déjà dans les journaux de la tâche 2, mais
il n'y est lisible qu'en recoupant deux blocs de topologie à dix lignes d'écart —
c'est ce qui l'a fait manquer au premier rapport. Le rendre explicite :

```rust
tracing::info!(
    combinaison = %etiquette,
    avant_creation_l = taille_apres_tour.0,
    avant_creation_h = taille_apres_tour.1,
    apres_creation_l = taille_apres_sortie_neuve.0,
    apres_creation_h = taille_apres_sortie_neuve.1,
    survit = taille_apres_tour == taille_apres_sortie_neuve,
    "PERSISTANCE : le changement de mode survit-il à la création d'une sortie ?"
);
```

⚠️ **Ce contrôle doit pouvoir rendre les DEUX valeurs**, et la tâche 2 établit
qu'il le peut : `survit=false` y est le relevé réel sous `CDS_TYPE(0)`. Une
exécution sous `CDS_TYPE(0)` sert donc de **témoin rouge** et doit être jouée.

- [ ] **Step 3 : vérifier le plafond de lignes**

Run: `wc -l agent/src/diagnostics/multifenetre/mode_sortie.rs`
Expected: < 500. ⚠️ **Le fichier est à 496 : la marge est de 4.** L'addition
appelle donc une extraction — `combinaisons.rs` (129), `temoin.rs` (164) et
`voisines.rs` (240) sont les points de chute.

- [ ] **Step 4 : compiler et mesurer, deux exécutions par bras**

```bash
set -a && source .env && set +a
scripts/build-agent.sh
MULTIFENETRE_MODE_SORTIE_DRAPEAUX='CDS_UPDATEREGISTRY seul' \
  MULTIFENETRE_MODE_SORTIE=1280x720 scripts/run-agent.sh
```

Deux exécutions sous `CDS_UPDATEREGISTRY seul`, plus **une** sous
`aucun drapeau (dynamique, non persisté)` comme témoin rouge.

- [ ] **Step 5 : rendre le verdict**

**SURVIT** — l'arbitrage redevient « persistance contre pollution du registre »,
qui a un remède connu (restaurer le registre après coup, §4.1 du plan).
**NE SURVIT PAS** — l'abandon de la famille ① devient **établi et non supposé**.

⚠️ Relever aussi si `CDS_UPDATEREGISTRY` **pollue effectivement** : une sortie
créée après le changement naît-elle à la nouvelle taille ? Sous `CDS_TYPE(0)`, la
tâche 2 relève que `DISPLAY8` naît à 1280×720 et non 2560×1440 — le contrôle
existe donc déjà et se lit au même endroit.

- [ ] **Step 6 : commit**

```bash
git add agent/src/diagnostics/multifenetre/mode_sortie.rs scripts/run-agent.sh \
  docs/superpowers/plans/journaux-multifenetres-d9/
git commit -m "mesure(d9): la persistance sous CDS_UPDATEREGISTRY, deux executions"
```

---

## Famille ① — le plein écran : retirer ce qui ne peut pas fonctionner

> ⚠️ **CETTE FAMILLE A CHANGÉ D'OBJET LE 6 AOÛT 2026, après le verdict consolidé
> de la phase P (tâches 2 et 2bis), sur décision de l'utilisateur.** Elle
> s'appelait « armer le plein écran ». La mesure a établi que le changement de
> mode **ne survit pas** — `n = 4` exécutions propres à cibles distinctes de la
> taille de création, sous `CDS_UPDATEREGISTRY` comme sous `flags = 0` — et que
> `CDS_UPDATEREGISTRY` **pollue le registre**, confirmé et attribuable par GUID.
> **L'abandon est donc établi, pas supposé.** L'ancienne tâche 4 (C2) est
> supprimée : `reconstruire_sur_la_sortie` n'a qu'un seul appelant, le
> changement de mode lui-même, et disparaît avec lui.

### Task 3 : retirer le changement de mode de sortie

**Files:**
- Delete: `agent/src/windows_source/redimensionnement/mode_sortie.rs` (458 lignes)
- Modify: `agent/src/windows_source/redimensionnement.rs` (345) — l'appel unique, l. 185
- Modify: `agent/src/capteur/plein_ecran.rs` (263) — le garde `changement_de_mode_arme` et ses raisons
- Modify: `scripts/run-agent.sh` — la ligne `PLEIN_ECRAN_MODE_SORTIE`
- Modify: `agent/src/superviseur/table.rs:279`, `agent/src/superviseur/boucle/placement_periodique.rs:19`, `agent/src/superviseur/table/tests_retention.rs:283`, `agent/src/windows_source/sortie.rs:61` — les quatre commentaires renvoyants

**Interfaces:**
- Consumes: le verdict consolidé de la phase P
- Produces: rien — c'est une suppression

**Ce qui reste livré, et c'est le repli que la conception de D8 avait écrit
d'avance** (« ①, ③, ④ et ⑤ tiennent sans ② ») : la détection du style de
fenêtre (`capteur/fenetre.rs`, `PERIODE_STYLE`), l'annonce
`DepuisCapteur::PleinEcran` → `AgentControl::Fullscreen`, et l'armement client
(`client/src/fullscreen.ts`) avec Keyboard Lock. **La fenêtre navigateur passe
en plein écran ; le flux garde la résolution de création de sa sortie.**

- [ ] **Step 1 : vérifier qu'il n'y a bien qu'un appelant**

Run: `grep -rn "changer_mode_de_sortie\|reconstruire_sur_la_sortie\|changement_de_mode_arme\|PLEIN_ECRAN_MODE_SORTIE" agent/src client/src proto scripts`
Expected: le seul site d'appel de `changer_mode_de_sortie` est `redimensionnement.rs:185` ; le seul appelant de `reconstruire_sur_la_sortie` est `changer_mode_de_sortie` lui-même. **Si le relevé contredit cela, s'arrêter et le signaler** — le périmètre de la suppression en dépend.

- [ ] **Step 2 : supprimer le module et son appel**

Supprimer le fichier, sa déclaration `mod` dans `redimensionnement.rs`, et le
bras qui l'appelle (l. 185). **Ce que `resize` doit faire à la place est ce
qu'il faisait avant D8** : en mode « une sortie par fenêtre », il n'y a rien à
redimensionner — la garde `sur_sortie` posée par D2 reste, et c'est elle qui
porte ce comportement.

- [ ] **Step 3 : supprimer le garde, en inscrivant POURQUOI**

Retirer `changement_de_mode_arme` de `capteur/plein_ecran.rs`, et remplacer ses
deux Critiques par le constat de mesure, auprès de la détection qui reste :

```rust
//! **Le sens « la résolution suit » n'existe pas, et c'est une décision de
//! mesure, pas un oubli.** Le sous-bloc D8 avait écrit un changement de mode de
//! la sortie virtuelle, livré désarmé faute d'avoir jamais tourné. Le sous-bloc
//! D9 l'a mesuré et l'a retiré :
//!
//! - le changement **ne survit pas** — la sortie revient à sa taille de création
//!   dès qu'une sortie virtuelle de plus est créée, c'est-à-dire à chaque
//!   ouverture de fenêtre. `n = 4` exécutions propres, cibles toutes distinctes
//!   de la taille de création, sous `CDS_UPDATEREGISTRY` comme sous `flags = 0` ;
//! - `CDS_UPDATEREGISTRY` **pollue le registre**, confirmé et attribuable par
//!   GUID sur 3 transitions probantes : une sortie créée ensuite naît à la
//!   taille polluée, ce qui bloque le produit — la préparation de la recette D8
//!   avait dû lever ce blocage à la main.
//!
//! Journaux : `docs/superpowers/plans/journaux-multifenetres-d9/p-persistance-*`
//! et `p2-*`. **Le mécanisme n'est PAS expliqué** : il est séparé en deux
//! régimes observables, et un confondeur covarie avec leur frontière.
```

- [ ] **Step 4 : reprendre les quatre commentaires renvoyants**

Les quatre nommés ci-dessus décrivent un mécanisme qui n'existe plus. **Les
corriger à leur place, pas ailleurs** — et vérifier par `grep -n` qu'aucune
autre occurrence ne survit.

- [ ] **Step 5 : vérifier**

Run: `cd agent && cargo check --target x86_64-pc-windows-gnu && cargo test -p agent`
Expected: sortie 0 ; **425 tests passent** (le compte de début de tâche), aucun avertissement neuf.

Run: `grep -rn "PLEIN_ECRAN_MODE_SORTIE" agent client proto scripts docs/superpowers/specs CLAUDE.md`
Expected: aucune occurrence hors documents de résultats et `CLAUDE.md` (que la tâche 17 reprendra).

- [ ] **Step 6 : commit**

```bash
git add agent/src/windows_source/redimensionnement.rs agent/src/capteur/plein_ecran.rs \
  agent/src/superviseur/table.rs agent/src/superviseur/boucle/placement_periodique.rs \
  agent/src/superviseur/table/tests_retention.rs agent/src/windows_source/sortie.rs \
  scripts/run-agent.sh
git rm agent/src/windows_source/redimensionnement/mode_sortie.rs
git commit -m "feat(d9): retirer le changement de mode de sortie, mesure a l'appui"
```

---

### Task 5 : le rejeu du `Resize`, l'unité du viewport, et la session au journal

**Files:**
- Create: `client/src/resize.ts`
- Create: `client/src/resize.test.ts`
- Modify: `client/src/main.ts:31-45, 275-289`
- Modify: `agent/src/demarrage.rs:381`

**Interfaces:**
- Produces: `RejeuResize` — `observer(taille)`, `aEmettre(): Taille | undefined`, `confirmer(taille)`

**Contexte, vérifié dans le code.** `client/src/main.ts:283` abandonne **en silence** si `session.controlChannel.readyState !== 'open'` au moment où la temporisation de 200 ms expire, et **ne réémet jamais** — l'observateur ne se redéclenche que si l'élément change encore de taille. Et `main.ts:41` annonce le viewport en `window.innerWidth` **sans** dpr, quand `main.ts:284` envoie `video.clientWidth × window.devicePixelRatio` : à `devicePixelRatio > 1`, le court-circuit anti-`Resize`-de-routine ne retient plus rien.

- [ ] **Step 1 : écrire le test qui échoue**

`client/src/resize.test.ts` :

```ts
import { describe, expect, it } from 'vitest';
import { RejeuResize } from './resize';

describe('RejeuResize', () => {
    it("rend la taille observée quand rien n'a encore été émis", () => {
        const r = new RejeuResize();
        r.observer({ largeur: 1280, hauteur: 720 });
        expect(r.aEmettre()).toEqual({ largeur: 1280, hauteur: 720 });
    });

    it('ne rend rien quand la taille émise est déjà la bonne', () => {
        const r = new RejeuResize();
        r.observer({ largeur: 1280, hauteur: 720 });
        r.confirmer({ largeur: 1280, hauteur: 720 });
        expect(r.aEmettre()).toBeUndefined();
    });

    it('REJOUE la dernière taille quand le canal était fermé au moment du geste', () => {
        // Le cas du leg 10 : le ResizeObserver a vu la taille, mais
        // `readyState !== 'open'` a fait abandonner l'envoi. À l'ouverture du
        // canal, la taille doit repartir — sans quoi elle est perdue à jamais,
        // l'observateur ne se redéclenchant que sur un NOUVEAU changement.
        const r = new RejeuResize();
        r.observer({ largeur: 1920, hauteur: 1080 });
        // aucun `confirmer` : l'envoi n'a pas eu lieu
        expect(r.aEmettre()).toEqual({ largeur: 1920, hauteur: 1080 });
    });

    it('rend la DERNIÈRE taille observée, pas la première', () => {
        const r = new RejeuResize();
        r.observer({ largeur: 1280, hauteur: 720 });
        r.observer({ largeur: 1920, hauteur: 1080 });
        expect(r.aEmettre()).toEqual({ largeur: 1920, hauteur: 1080 });
    });

    it("ne rend rien tant que rien n'a été observé", () => {
        expect(new RejeuResize().aEmettre()).toBeUndefined();
    });
});
```

- [ ] **Step 2 : exécuter le test pour vérifier qu'il échoue**

Run: `cd client && npx vitest run src/resize.test.ts`
Expected: FAIL — `Cannot find module './resize'`.

- [ ] **Step 3 : écrire l'implémentation minimale**

`client/src/resize.ts` :

```ts
/**
 * Retient la dernière taille observée et dit s'il faut l'émettre.
 *
 * **Pourquoi cet objet existe** : le `ResizeObserver` de `main.ts` abandonnait
 * en silence quand le canal de contrôle n'était pas ouvert au moment où sa
 * temporisation expirait, et ne réémettait JAMAIS — l'observateur ne se
 * redéclenche que si l'élément change encore de taille. Une taille perdue
 * l'était donc à jamais (leg 10 du sous-bloc D8 : deux `Resize` relevés pour
 * cinq sessions).
 *
 * **Pur, sans DOM** : c'est ce qui le rend éprouvable.
 */
export interface Taille {
    largeur: number;
    hauteur: number;
}

export class RejeuResize {
    private derniere: Taille | undefined;
    private emise: Taille | undefined;

    /** Le `ResizeObserver` a vu une taille. */
    observer(taille: Taille): void {
        this.derniere = taille;
    }

    /** La taille à émettre, ou `undefined` s'il n'y a rien de neuf. */
    aEmettre(): Taille | undefined {
        const derniere = this.derniere;
        if (!derniere) return undefined;
        if (
            this.emise &&
            this.emise.largeur === derniere.largeur &&
            this.emise.hauteur === derniere.hauteur
        ) {
            return undefined;
        }
        return derniere;
    }

    /** L'émission a réellement eu lieu. */
    confirmer(taille: Taille): void {
        this.emise = taille;
    }
}
```

- [ ] **Step 4 : exécuter le test pour vérifier qu'il passe**

Run: `cd client && npx vitest run src/resize.test.ts`
Expected: PASS, 5 tests.

- [ ] **Step 5 : câbler le rejeu et corriger l'unité dans `main.ts`**

Remplacer le corps de l'observateur (`main.ts:280-289`) :

```ts
const rejeu = new RejeuResize();
const emettreSiPossible = () => {
    const taille = rejeu.aEmettre();
    if (!taille) return;
    if (session.controlChannel.readyState !== 'open') {
        // Tracé, et non plus muet : c'est ce `return` silencieux qui perdait
        // les `Resize` sans laisser la moindre trace (leg 10).
        console.warn('Resize différé : canal de contrôle non ouvert');
        return;
    }
    session.controlChannel.send(encodeResize(taille.largeur, taille.hauteur));
    rejeu.confirmer(taille);
};

let resizeTimer: number | undefined;
const observer = new ResizeObserver(() => {
    window.clearTimeout(resizeTimer);
    resizeTimer = window.setTimeout(() => {
        rejeu.observer({
            largeur: Math.round(video.clientWidth * window.devicePixelRatio),
            hauteur: Math.round(video.clientHeight * window.devicePixelRatio),
        });
        emettreSiPossible();
    }, 200);
});
observer.observe(video);
// Le rejeu : à l'ouverture du canal, la taille retenue repart.
session.controlChannel.addEventListener('open', emettreSiPossible);
```

- [ ] **Step 6 : corriger l'unité de l'annonce de viewport**

`main.ts:41` — multiplier **puis** arrondir en pair :

```ts
// MÊME UNITÉ que le `Resize` émis plus bas (`clientWidth × devicePixelRatio`).
// Sans ce facteur, à `devicePixelRatio > 1` la sortie virtuelle naît sur une
// grandeur que le `Resize` de routine ne peut pas égaler, et le court-circuit
// « taille inchangée » de `windows_source/redimensionnement.rs` ne retient
// plus rien : CHAQUE connexion de CHAQUE fenêtre déclencherait un changement
// de mode, avec 25 à 100 % d'écart (leg 7 du sous-bloc D8).
//
// Il n'y a qu'un `devicePixelRatio` en jeu : c'est CETTE page qui annonce, et
// c'est son propre `ResizeObserver` qui émettra le `Resize`.
//
// Multiplier PUIS arrondir en pair — `viewportPair` a un plancher à 2, et
// l'ordre inverse laisserait passer une hauteur impaire à dpr impair.
const dpr = window.devicePixelRatio;
const { largeur, hauteur } = viewportPair(
    Math.round(window.innerWidth * dpr),
    Math.round(window.innerHeight * dpr),
);
```

- [ ] **Step 7 : donner sa session à la trace `contrôle reçu`**

`agent/src/demarrage.rs:381` :

```rust
// `session` : sans ce champ la trace n'est PAS attribuable — tous les enfants
// héritent le même `agent.log` depuis D4. C'est exactement ce qui a rendu
// indécidable « 2 `Resize` pour 5 sessions » (leg 10 de D8, correction I8) :
// les deux lignes ne portaient aucune session, donc rien n'établissait
// qu'elles vinssent de deux sessions distinctes.
let mut on_control = |message| tracing::info!(session = %session_id, ?message, "contrôle reçu");
```

- [ ] **Step 8 : vérifier l'ensemble**

Run: `cd client && npx vitest run && npx tsc --noEmit`
Expected: PASS, aucune erreur de type.

Run: `cd agent && cargo check --target x86_64-pc-windows-gnu`
Expected: sortie 0.

- [ ] **Step 9 : commit**

```bash
git add client/src/resize.ts client/src/resize.test.ts client/src/main.ts agent/src/demarrage.rs
git commit -m "fix(d9): le Resize se rejoue, le viewport passe en pixels peripheriques"
```

---

## Famille ② — l'audio mort et l'identité de session

### Task 6 : extraire les instances de tube hors de `serveur.rs`

**Files:**
- Create: `agent/src/capteur/serveur/instances.rs`
- Modify: `agent/src/capteur/serveur.rs` (490 lignes, **marge 10**)

**Interfaces:**
- Produces: `pub(super) fn creer_instance() -> Result<HANDLE>`, `pub(super) fn connecter(tube: HANDLE) -> Result<()>`

**Contexte.** `CLAUDE.md` exige pour ce fichier « une extraction, jamais une compression du commentaire de `TAMPON` ». La tâche 9 y ajoute un bras : l'extraction se fait **maintenant**, séparément, pour qu'elle soit une pure mécanique qu'un relecteur puisse approuver sans juger de comportement.

- [ ] **Step 1 : déplacer, sans rien changer**

Déplacer vers `agent/src/capteur/serveur/instances.rs` : `TAMPON` **avec son commentaire entier**, `SOUFFLE_CREATION_INSTANCE`, `creer_instance` (l. 221) et `connecter` (l. 213). Déclarer `mod instances;` dans `serveur.rs`.

En tête du fichier neuf :

```rust
//! Création et acceptation des instances du tube nommé du capteur.
//!
//! **Extrait de `serveur.rs` le 6 août 2026**, qui était à 490 lignes pour un
//! plafond de 500 — `CLAUDE.md` exige pour ce fichier « une extraction, jamais
//! une compression du commentaire de `TAMPON` », et c'est bien le commentaire
//! de `TAMPON` qui part ici AVEC sa constante, auprès de laquelle il doit
//! rester. **Aucune valeur, aucun ordre d'opération n'a changé.**
```

- [ ] **Step 2 : vérifier qu'aucun comportement n'a bougé**

Run: `cd agent && cargo check --target x86_64-pc-windows-gnu && cargo test -p agent`
Expected: sortie 0 ; tous les tests d'hôte passent, au même nombre qu'avant.

Run: `git diff --stat HEAD -- agent/src/capteur/`
Expected: les lignes retirées de `serveur.rs` égalent (au `mod` et aux `use` près) celles ajoutées dans `instances.rs`.

- [ ] **Step 3 : vérifier la marge regagnée**

Run: `wc -l agent/src/capteur/serveur.rs agent/src/capteur/serveur/instances.rs`
Expected: `serveur.rs` nettement sous 490. ⚠️ **La marge regagnée par une extraction se reperd à la ronde suivante si on la traite comme acquise** — ce dépôt a payé cette leçon trois fois.

- [ ] **Step 4 : commit**

```bash
git add agent/src/capteur/serveur.rs agent/src/capteur/serveur/instances.rs
git commit -m "refactor(d9): les instances de tube sortent de serveur.rs, marge 10 rendue"
```

---

### Task 7 : la règle d'inaptitude, pure et éprouvée sur l'hôte

**Files:**
- Modify: `agent/src/capteur/audio.rs` (152 lignes)

**Interfaces:**
- Consumes: rien
- Produces: `FenetreAudio { session: String, pid: u32, arrivee: u64, dernier_focus: u64, inapte: bool }` et `arbitrer(&[FenetreAudio]) -> Vec<(String, bool)>` — **signature inchangée**

**Contexte.** `capteur/audio.rs` est pur, sans `cfg`, sans COM, sans fenêtre. La règle neuve y reste : **une fenêtre inapte ne peut jamais devenir porteuse**, mais elle reçoit toujours son `(session, false)` — le registre a besoin de ce `false` pour ordonner de se taire.

⚠️ **Le champ est un `bool`, jamais un `Instant`.** L'expiration du répit vit dans le registre, qui a l'horloge ; `audio.rs` garde sa doctrine — « un rang, pas un horodatage ».

- [ ] **Step 1 : écrire les tests qui échouent**

Dans le module `tests` de `agent/src/capteur/audio.rs` :

```rust
#[test]
fn une_fenetre_inapte_ne_porte_jamais_le_son() {
    // Le cas du leg 1 : sa capture WASAPI est morte après dix échecs
    // consécutifs. Elle ne doit plus être élue, sans quoi le groupe entier
    // reste muet — c'est l'état d'avant D9.
    let fenetres = vec![
        FenetreAudio { session: "w-1".into(), pid: 42, arrivee: 1, dernier_focus: 9, inapte: true },
        FenetreAudio { session: "w-2".into(), pid: 42, arrivee: 2, dernier_focus: 0, inapte: false },
    ];
    let verdict = arbitrer(&fenetres);
    assert_eq!(verdict, vec![("w-1".into(), false), ("w-2".into(), true)]);
}

#[test]
fn une_inapte_recoit_quand_meme_son_verdict_false() {
    // Le registre a besoin de ce `false` pour ordonner de se taire à celle
    // qui portait le son l'instant d'avant.
    let fenetres = vec![FenetreAudio {
        session: "w-1".into(), pid: 42, arrivee: 1, dernier_focus: 1, inapte: true,
    }];
    assert_eq!(arbitrer(&fenetres), vec![("w-1".into(), false)]);
}

#[test]
fn un_groupe_entierement_inapte_reste_muet() {
    // Aucune voisine à promouvoir : c'est le cas MAJORITAIRE — une
    // application, une fenêtre. Le réarmement après répit est le seul remède,
    // et il vit dans le registre, pas ici.
    let fenetres = vec![
        FenetreAudio { session: "w-1".into(), pid: 42, arrivee: 1, dernier_focus: 0, inapte: true },
        FenetreAudio { session: "w-2".into(), pid: 42, arrivee: 2, dernier_focus: 0, inapte: true },
    ];
    assert_eq!(arbitrer(&fenetres), vec![("w-1".into(), false), ("w-2".into(), false)]);
}

#[test]
fn l_inaptitude_d_un_groupe_ne_touche_pas_un_autre_pid() {
    let fenetres = vec![
        FenetreAudio { session: "w-1".into(), pid: 42, arrivee: 1, dernier_focus: 0, inapte: true },
        FenetreAudio { session: "w-2".into(), pid: 77, arrivee: 2, dernier_focus: 0, inapte: false },
    ];
    assert_eq!(arbitrer(&fenetres), vec![("w-1".into(), false), ("w-2".into(), true)]);
}
```

- [ ] **Step 2 : exécuter les tests pour vérifier qu'ils échouent**

Run: `cd agent && cargo test -p agent capteur::audio`
Expected: FAIL — `struct FenetreAudio has no field named inapte`.

- [ ] **Step 3 : écrire l'implémentation minimale**

Ajouter le champ, documenté :

```rust
    /// Cette fenêtre ne peut pas porter le son en ce moment.
    ///
    /// Vrai quand sa capture WASAPI est morte — dix erreurs de lecture
    /// consécutives (`crate::audio::LECTURES_ECHOUEES_MAX`) — et qu'elle
    /// observe son répit de réarmement.
    ///
    /// ⚠️ **Un `bool`, jamais un `Instant`.** L'expiration du répit vit dans le
    /// registre, qui a l'horloge ; ce module garde sa doctrine — « un rang, pas
    /// un horodatage » — et reste éprouvable sans horloge.
    pub inapte: bool,
```

Et dans `arbitrer`, une seule ligne dans la boucle de candidature :

```rust
    for f in fenetres {
        // Une inapte n'est jamais CANDIDATE. Elle reçoit quand même son
        // verdict plus bas, qui vaudra `false` : c'est ce `false` qui ordonne
        // de se taire à celle qui portait le son l'instant d'avant.
        if f.inapte {
            continue;
        }
        match porteur.get(&f.pid) {
```

- [ ] **Step 4 : exécuter les tests pour vérifier qu'ils passent**

Run: `cd agent && cargo test -p agent capteur::audio`
Expected: PASS. Les tests préexistants de ce module passent aussi — les corriger en ajoutant `inapte: false`, jamais en changeant leur assertion.

- [ ] **Step 5 : commit**

```bash
git add agent/src/capteur/audio.rs
git commit -m "feat(d9): une fenetre dont la capture audio est morte ne porte plus le son"
```

---

### Task 8 : le registre — inaptitudes, réarmements, expiration

**Files:**
- Modify: `agent/src/capteur/sommeil.rs` (327 lignes, marge 173)
- Modify: `agent/src/capteur/sommeil/porteurs.rs` (184 lignes)
- Modify: `agent/src/capteur/sommeil/tests.rs` (203 lignes)

**Interfaces:**
- Consumes: `capteur::audio::{FenetreAudio, arbitrer}` (tâche 7)
- Produces: `pub fn audio_mort(session: &str)`, `pub const REPIT_REARMEMENT_AUDIO: Duration`, `pub const REARMEMENTS_MAX: u32`

- [ ] **Step 1 : ajouter les constantes, en disant qu'elles ne sont pas calibrées**

Dans `sommeil.rs` :

```rust
/// Répit avant qu'une fenêtre dont la capture audio est morte ne redevienne
/// éligible au portage.
///
/// **Il finance le cas MAJORITAIRE** — une application, une fenêtre, donc
/// aucune voisine à promouvoir. Sans lui, le remède ne couvrirait que les
/// applications multi-fenêtres et le groupe resterait muet sans retour,
/// exactement comme avant D9. Réélire la même session construit une activation
/// *process loopback* NEUVE, ce qui est une chance réelle : les causes connues
/// d'un refus de lecture WASAPI — changement de périphérique, redémarrage du
/// service audio, changement de format — sont transitoires.
///
/// ⚠️ **NON CALIBRÉE.** Aucune mesure ne la fonde : elle rejoint `BPP_MIN`,
/// `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`, `HYSTERESIS`, `REPIT_APRES_ECHEC` et
/// `TAILLE_MAX_SORTIE`.
pub const REPIT_REARMEMENT_AUDIO: Duration = Duration::from_secs(5);

/// Nombre de réarmements consécutifs avant abandon définitif.
///
/// Sans borne, un périphérique audio définitivement mort ferait tourner le
/// cycle « inapte → répit → réélue → morte » sans fin, et chaque tour coûte
/// une activation COM.
///
/// ⚠️ **NON CALIBRÉE**, comme la précédente. L'abandon définitif est
/// **journalisé**, jamais muet — c'est la contrainte que F3 de D7 a posée.
pub const REARMEMENTS_MAX: u32 = 5;
```

- [ ] **Step 2 : ajouter les tables à `Etat`**

```rust
    /// Instant après lequel une session dont la capture audio est morte
    /// redevient éligible au portage. Absente = apte.
    ///
    /// **Ici et pas dans `capteur::audio`** : ce module a l'horloge, l'autre
    /// est pur et le reste.
    inaptes: HashMap<String, Instant>,
    /// Nombre de réarmements consécutifs déjà accordés à chaque session.
    /// Remis à zéro dès qu'elle porte le son sans mourir.
    rearmements: HashMap<String, u32>,
```

- [ ] **Step 3 : écrire `audio_mort`**

```rust
/// Une session signale que sa capture audio est morte.
///
/// **Ne répond rien, et c'est voulu** : l'arbitrage est global et la décision
/// peut concerner une AUTRE fenêtre. L'effet revient par
/// `DepuisCapteur::Audio`, poussé sur la connexion média de chaque fenêtre
/// concernée — exactement le patron de `signaler`.
pub fn audio_mort(session: &str) {
    let mut garde = etat();
    let tours = garde.rearmements.entry(session.to_string()).or_insert(0);
    *tours += 1;
    if *tours > REARMEMENTS_MAX {
        // Abandon définitif, JOURNALISÉ. Un silence muet est précisément le
        // défaut que F3 de D7 a corrigé ; ne pas le réintroduire ici.
        tracing::warn!(
            %session,
            rearmements = *tours - 1,
            "capture audio morte et abandon définitif : le groupe de PID restera muet"
        );
        garde.inaptes.insert(session.to_string(), Instant::now() + Duration::from_secs(86_400));
    } else {
        tracing::info!(
            %session,
            rearmement = *tours,
            repit = ?REPIT_REARMEMENT_AUDIO,
            "capture audio morte, réarmement programmé"
        );
        garde.inaptes.insert(session.to_string(), Instant::now() + REPIT_REARMEMENT_AUDIO);
    }
    porteurs::distribuer_l_audio(&mut garde);
}
```

- [ ] **Step 4 : expirer le répit au tour de roue, par une fonction NOMMÉE**

⚠️ **La purge est une fonction du produit, pas un `retain` en ligne** — sans quoi le test du step 7 n'éprouverait que `HashMap::retain`, c'est-à-dire rien.

```rust
/// Retire du registre les inaptitudes dont le répit a expiré.
///
/// **Nommée et séparée pour être ÉPROUVABLE** : le tour de roue (250 ms) est
/// ce qui rend le répit effectif, et sans cette purge une inapte le resterait
/// jusqu'au prochain événement, qui peut ne jamais venir. Un `retain` en ligne
/// dans le tour de roue ne serait couvert par aucun test.
pub(super) fn purger_les_inaptitudes(
    inaptes: &mut HashMap<String, Instant>,
    maintenant: Instant,
) {
    inaptes.retain(|_, echeance| *echeance > maintenant);
}
```

Et dans le tour de roue (`sommeil.rs:146`), **avant** `porteurs::distribuer_l_audio` :

```rust
        purger_les_inaptitudes(&mut garde.inaptes, Instant::now());
```

- [ ] **Step 5 : remplir `inapte` dans `porteurs.rs`**

Là où `distribuer_l_audio` construit son `Vec<FenetreAudio>` :

```rust
            inapte: garde.inaptes.contains_key(session),
```

- [ ] **Step 6 : remettre le compteur à zéro quand le son tient**

Dans `distribuer_l_audio`, après qu'une session a été élue **et** qu'aucun `AudioMort` ne l'a suivie au tour d'après — le plus simple et le plus sûr : remettre à zéro dès qu'une session est élue porteuse **alors qu'elle n'est pas inapte** :

```rust
            // Elle porte le son et n'est pas inapte : le cycle de réarmement
            // est refermé. Sans cette remise à zéro, `REARMEMENTS_MAX`
            // s'épuiserait sur toute la vie de la session au lieu de compter
            // des échecs CONSÉCUTIFS.
            if actif {
                garde.rearmements.remove(session);
            }
```

- [ ] **Step 7 : écrire le test d'hôte**

Dans `agent/src/capteur/sommeil/tests.rs` — les tables et l'expiration se testent sur la structure `Etat` extraite ; si `Etat` n'est pas accessible aux tests, tester la **règle d'expiration** comme fonction pure :

```rust
#[test]
fn le_repit_expire_et_rend_la_fenetre_apte() {
    // Éprouve `purger_les_inaptitudes`, la fonction du PRODUIT — pas
    // `HashMap::retain`. L'horloge est injectée (`maintenant`), ce qui rend
    // le test déterministe sans aucune attente réelle.
    let mut inaptes: HashMap<String, Instant> = HashMap::new();
    let t0 = Instant::now();
    inaptes.insert("w-1".into(), t0 + Duration::from_millis(10));
    inaptes.insert("w-2".into(), t0 + Duration::from_secs(60));

    purger_les_inaptitudes(&mut inaptes, t0 + Duration::from_millis(20));

    assert!(!inaptes.contains_key("w-1"), "le répit de w-1 a expiré");
    assert!(inaptes.contains_key("w-2"), "celui de w-2 court encore");
}

#[test]
fn une_purge_sur_un_registre_vide_ne_panique_pas() {
    let mut inaptes: HashMap<String, Instant> = HashMap::new();
    purger_les_inaptitudes(&mut inaptes, Instant::now());
    assert!(inaptes.is_empty());
}
```

- [ ] **Step 8 : vérifier**

Run: `cd agent && cargo test -p agent capteur:: && cargo check --target x86_64-pc-windows-gnu`
Expected: PASS ; sortie 0.

Run: `wc -l agent/src/capteur/sommeil.rs agent/src/capteur/sommeil/porteurs.rs`
Expected: les deux < 500.

- [ ] **Step 9 : commit**

```bash
git add agent/src/capteur/sommeil.rs agent/src/capteur/sommeil/porteurs.rs agent/src/capteur/sommeil/tests.rs
git commit -m "feat(d9): le registre rearme puis promeut quand une capture audio meurt"
```

---

### Task 9 : le message `AudioMort`, de l'enfant au registre

**Files:**
- Modify: `agent/src/capteur/protocole.rs` (369 lignes, marge 131)
- Modify: `agent/src/capteur/fenetre/commandes.rs` (192 lignes)
- Modify: `agent/src/source.rs` (362 lignes)
- Modify: `agent/src/capteur/distante.rs` (375 lignes)
- Modify: `agent/src/transport/tick.rs` (308 lignes)
- Modify: `agent/src/capteur/pont_media.rs` (263 lignes) — **vérification seulement**

**Interfaces:**
- Consumes: `capteur::sommeil::audio_mort` (tâche 8)
- Produces: `VersCapteur::AudioMort`, `VideoSource::signaler_audio_mort(&mut self)`

**Contexte, vérifié dans le code.** `capture_morte` est posé par `windows_audio.rs:342,406` et lu **uniquement** par `transport/piste_audio.rs:133`, pour une ligne de journal. Le capteur ne le voit jamais. Et `appliquer_audio` ne court qu'à l'arrivée d'un ordre, **jamais périodiquement** : la détection doit vivre dans le tick.

- [ ] **Step 1 : ajouter la variante, SANS charge utile**

Dans `protocole.rs`, après `Visibilite` :

```rust
    /// La capture audio de cette fenêtre est morte définitivement, après
    /// `crate::audio::LECTURES_ECHOUEES_MAX` erreurs de lecture consécutives.
    ///
    /// **Aucune charge utile** : la session est celle du canal, comme pour
    /// toutes les commandes — `capteur/fenetre/commandes.rs` la tire de son
    /// contexte.
    ///
    /// **Ne se répond pas par `Fait` au sens de l'effet** : le capteur
    /// ré-arbitre globalement, et la décision peut concerner une AUTRE fenêtre
    /// du même groupe de PID. L'effet revient par `DepuisCapteur::Audio`,
    /// poussé sur la connexion média. Même patron exactement que `Visibilite`.
    AudioMort,
```

⚠️ **Rien à ajouter dans `DepuisCapteur`** : le réarmement réemploie la variante `Audio { emet }` existante.

- [ ] **Step 2 : vérifier le bras catch-all de `pont_media.rs`**

Run: `grep -n 'Ok(autre)' -B 20 agent/src/capteur/pont_media.rs`

`AudioMort` allant dans `VersCapteur` et non dans `DepuisCapteur`, ce bras **ne devrait pas** être sur le chemin. Le vérifier quand même : il tue le fil `lire_le_media` **en silence** et ce dépôt l'a payé **quatre fois** (D5 `Sommeil`, D6 `Part`, D7 `Audio`, D8 `PleinEcran`). Écrire dans le rapport de tâche ce que la vérification a montré.

- [ ] **Step 3 : traiter le message côté fil de fenêtre**

Dans `capteur/fenetre/commandes.rs`, juste après le bras `Visibilite` (l. 95-101) :

```rust
        VersCapteur::AudioMort => {
            // L'effet ne revient PAS par cette réponse : l'arbitrage est global
            // et peut concerner une AUTRE fenêtre du même groupe de PID. Il
            // revient par `DepuisCapteur::Audio`, poussé sur la connexion
            // média. Même patron que `Visibilite` juste au-dessus.
            crate::capteur::sommeil::audio_mort(ctx.session);
            return DepuisCapteur::Fait;
        }
```

Et l'ajouter au `match` exhaustif de la l. 182 (« commande déjà traitée hors de la source »).

- [ ] **Step 4 : ajouter la méthode au trait, défaut inerte**

Dans `agent/src/source.rs` :

```rust
    /// Signale au capteur que la capture audio de cette fenêtre est morte.
    ///
    /// **Défaut inerte**, comme `est_endormie` : une source qui n'a pas de
    /// capteur en face n'a personne à prévenir. Seule `SourceDistante`
    /// l'implémente réellement.
    fn signaler_audio_mort(&mut self) {}
```

- [ ] **Step 5 : l'implémenter dans `SourceDistante`**

Dans `agent/src/capteur/distante.rs`, sur le patron de `signaler_visibilite` (l. 339) :

```rust
    fn signaler_audio_mort(&mut self) {
        if let Err(erreur) = self.commander_simple(VersCapteur::AudioMort) {
            tracing::warn!(%erreur, "signalement de capture audio morte non délivré");
        }
    }
```

- [ ] **Step 6 : détecter la transition dans le tick, et une seule fois**

Dans `agent/src/transport/tick.rs`, sur le fil de la session :

```rust
        // `appliquer_audio` ne court qu'à l'ARRIVÉE d'un ordre, jamais
        // périodiquement : sans ce contrôle au tick, une capture qui meurt
        // entre deux ordres ne serait jamais signalée. Un `load` atomique par
        // tour est bon marché.
        //
        // Le verrou `audio_mort_signale` est ce qui empêche d'inonder le
        // capteur : `capture_morte` reste vrai à jamais une fois posé.
        if !self.audio_mort_signale && self.piste_audio.capture_morte() {
            self.audio_mort_signale = true;
            self.source.signaler_audio_mort();
        }
```

Ajouter le champ `audio_mort_signale: bool` à la structure de session, initialisé à `false`, et **remis à `false` au rattachement** — un capteur relancé n'a plus l'information.

- [ ] **Step 7 : vérifier**

Run: `cd agent && cargo check --target x86_64-pc-windows-gnu && cargo test -p agent`
Expected: sortie 0 ; tous les tests passent.

Run: `wc -l agent/src/capteur/serveur.rs agent/src/capteur/protocole.rs agent/src/capteur/fenetre/commandes.rs agent/src/source.rs agent/src/capteur/distante.rs agent/src/transport/tick.rs`
Expected: tous < 500.

- [ ] **Step 8 : commit**

```bash
git add agent/src/capteur/protocole.rs agent/src/capteur/fenetre/commandes.rs agent/src/source.rs agent/src/capteur/distante.rs agent/src/transport/tick.rs
git commit -m "feat(d9): l'enfant dit au capteur que sa capture audio est morte"
```

---

### Task 10 : l'identité d'une session par génération monotone

**Files:**
- Modify: `agent/src/capteur/protocole.rs`
- Modify: `agent/src/capteur/sommeil.rs`
- Modify: `agent/src/superviseur/boucle.rs` (492 lignes, **marge 8**)
- Modify: `agent/src/capteur/sommeil/tests.rs`

**Interfaces:**
- Produces: `VersCapteur::Attache { .., generation: u64 }`, `sommeil::inscrire(session, pid, generation)`, `sommeil::retirer(session, generation)`

**Contexte.** F5 de D7, **préexistant** : l'identité d'une session par son seul nom porte une course au `retirer`. Un rattachement réinscrit le même nom, que le `retirer` de l'instance précédente peut alors emporter.

⚠️ **`superviseur/boucle.rs` est à 492 lignes, marge 8.** Si l'addition la dépasse, extraire vers `agent/src/superviseur/boucle/generations.rs`.

- [ ] **Step 1 : écrire le test qui échoue**

⚠️ **Le test éprouve un PRÉDICAT NOMMÉ du produit, pas l'opérateur `>`.** Écrire d'abord ce prédicat dans `sommeil.rs`, puis le tester :

```rust
/// Ce `retirer` est-il périmé, c'est-à-dire adressé à une instance déjà
/// remplacée par un rattachement ?
///
/// **Nommé et séparé pour être ÉPROUVABLE** : la logique en ligne dans
/// `retirer` ne serait couverte par aucun test, `retirer` touchant un état
/// global (`OnceLock<Mutex<Etat>>`) qu'un test d'hôte ne peut pas isoler.
pub(super) fn retirer_est_perime(
    generations: &HashMap<String, u64>,
    session: &str,
    generation: u64,
) -> bool {
    generations
        .get(session)
        .is_some_and(|courante| *courante > generation)
}
```

```rust
#[test]
fn un_retirer_perime_n_emporte_pas_l_inscription_neuve() {
    // F5 (D7, préexistant). Une session meurt, se rattache sous le même nom
    // avec une génération neuve, et le `retirer` de l'instance PRÉCÉDENTE
    // arrive après. Sans la génération, il emporterait la session vivante.
    let mut generations: HashMap<String, u64> = HashMap::new();
    generations.insert("w-1".into(), 7); // l'inscription neuve, après rattachement

    assert!(
        retirer_est_perime(&generations, "w-1", 6),
        "le retirer de la génération 6 est en retard : il ne doit rien retirer"
    );
    assert!(
        !retirer_est_perime(&generations, "w-1", 7),
        "celui de la génération courante retire bien"
    );
}

#[test]
fn un_retirer_sur_une_session_inconnue_n_est_pas_perime() {
    // Aucune inscription : `retirer` doit suivre son chemin normal, qui est
    // déjà tolérant à l'absence. Rendre `true` ici le rendrait inerte pour
    // toute session que le registre ne connaît pas encore.
    let generations: HashMap<String, u64> = HashMap::new();
    assert!(!retirer_est_perime(&generations, "w-1", 3));
}

#[test]
fn un_retirer_d_une_generation_posterieure_n_est_pas_perime() {
    // Le cas d'un `retirer` qui arrive APRÈS l'inscription qu'il vise : il
    // porte une génération plus récente que celle enregistrée, donc il agit.
    let mut generations: HashMap<String, u64> = HashMap::new();
    generations.insert("w-1".into(), 7);
    assert!(!retirer_est_perime(&generations, "w-1", 8));
}
```

- [ ] **Step 2 : exécuter le test pour vérifier qu'il échoue**

Run: `cd agent && cargo test -p agent capteur::sommeil::tests::un_retirer`
Expected: FAIL — `cannot find function retirer_est_perime`.

- [ ] **Step 3 : ajouter le champ au protocole**

```rust
        /// Génération de cette session, strictement croissante, frappée par le
        /// superviseur au lancement de l'enfant.
        ///
        /// **Le nom seul ne suffit pas à identifier une session** (F5 du
        /// sous-bloc D7, défaut préexistant) : un rattachement réinscrit le
        /// même nom, et le `retirer` de l'instance précédente pouvait alors
        /// emporter la session vivante. La génération rend ce `retirer`
        /// inoffensif.
        generation: u64,
```

- [ ] **Step 4 : la frapper dans le superviseur**

Dans `superviseur/boucle.rs`, un compteur monotone incrémenté à chaque `enfant lancé`, passé à l'enfant et relayé dans son `Attache`.

- [ ] **Step 5 : la faire respecter par le registre**

`Etat` gagne `generations: HashMap<String, u64>` ; `inscrire` la retient ; `retirer` devient :

```rust
pub fn retirer(session: &str, generation: u64) {
    let mut garde = etat();
    // Sans effet si l'inscription enregistrée est PLUS RÉCENTE : ce `retirer`
    // est celui d'une instance déjà remplacée par un rattachement (F5).
    if retirer_est_perime(&garde.generations, session, generation) {
        tracing::info!(%session, generation, "retirer périmé ignoré");
        return;
    }
    // ... corps existant inchangé, plus :
    garde.generations.remove(session);
}
```

- [ ] **Step 6 : vérifier**

Run: `cd agent && cargo test -p agent && cargo check --target x86_64-pc-windows-gnu`
Expected: PASS ; sortie 0.

Run: `wc -l agent/src/superviseur/boucle.rs`
Expected: < 500. Sinon, extraire comme prévu.

- [ ] **Step 7 : commit**

```bash
git add agent/src/capteur/protocole.rs agent/src/capteur/sommeil.rs agent/src/superviseur/boucle.rs agent/src/capteur/sommeil/tests.rs
git commit -m "fix(d9): une session s'identifie par sa generation, plus par son seul nom"
```

---

## Famille ③ — la dette de mesure de D6, et l'instrument

### Task 11 : la télémétrie par session, extraite

**Files:**
- Create: `agent/src/windows_source/telemetrie.rs`
- Modify: `agent/src/windows_source.rs` (638 lignes, **dette gelée**) — lignes 313, 453-455, 518, 544
- Modify: `agent/src/demarrage.rs:127-129` — le lecteur mort
- Modify: `agent/src/capteur/fenetre.rs` (470 lignes, marge 30) — le lecteur neuf

**Interfaces:**
- Produces: `Telemetrie::{tick, capturee, produite, lire}` — `lire() -> (u64, u64, u64)`

**Contexte, vérifié.** Les trois statiques (`windows_source.rs:453-455`) sont écrites aux lignes 313, 518 et 544 — donc dans le **capteur** depuis D4 — et lues au seul `demarrage.rs:127-129`, donc dans l'**enfant**. `SOURCE_TRACE=1` **est** transmis par `scripts/run-agent.sh:38` : la variable arrive, mais n'affiche que des zéros, et les compteurs du capteur ne sont lus par personne.

- [ ] **Step 1 : écrire le test qui échoue**

`agent/src/windows_source/telemetrie.rs`, module `tests` :

```rust
#[test]
fn deux_telemetries_ne_se_melangent_pas() {
    // C'est TOUT l'objet du leg : trois statiques de processus mélangeaient
    // les N fenêtres du capteur depuis D4, et `SOURCE_TRACE=1` n'affichait
    // que des zéros dans l'enfant, qui n'écrit rien.
    let a = Telemetrie::default();
    let b = Telemetrie::default();
    a.tick();
    a.tick();
    a.capturee();
    b.produite();
    assert_eq!(a.lire(), (2, 1, 0));
    assert_eq!(b.lire(), (0, 0, 1));
}

#[test]
fn une_telemetrie_neuve_est_a_zero() {
    assert_eq!(Telemetrie::default().lire(), (0, 0, 0));
}
```

- [ ] **Step 2 : exécuter pour vérifier l'échec**

Run: `cd agent && cargo test -p agent windows_source::telemetrie`
Expected: FAIL — `cannot find type Telemetrie`.

- [ ] **Step 3 : écrire l'implémentation**

```rust
//! Compteurs de capture, **par session**.
//!
//! **Pourquoi ce fichier existe** : `windows_source.rs` portait trois statiques
//! de processus (`TICKS`, `CAPTURED`, `PRODUCED`). Depuis le sous-bloc D4, la
//! capture est mutualisée dans un processus unique qui tient N fenêtres : les
//! trois compteurs mélangeaient donc N sessions. Pire, leur unique lecteur
//! (`demarrage.rs`) vit dans l'ENFANT, qui n'écrit rien — `SOURCE_TRACE=1`
//! n'affichait que des zéros, et les compteurs du capteur n'étaient lus par
//! personne. Consignation n°1 du sous-bloc D6, due depuis le 3 août 2026.
//!
//! **Pur, aucun `cfg`** : c'est ce qui le rend éprouvable sur l'hôte.

use std::sync::atomic::{AtomicU64, Ordering};

/// Compteurs de capture d'UNE fenêtre.
#[derive(Debug, Default)]
pub struct Telemetrie {
    ticks: AtomicU64,
    captured: AtomicU64,
    produced: AtomicU64,
}

impl Telemetrie {
    /// Un tour de boucle de capture.
    pub fn tick(&self) {
        self.ticks.fetch_add(1, Ordering::Relaxed);
    }

    /// Une image acquise auprès de la duplication.
    pub fn capturee(&self) {
        self.captured.fetch_add(1, Ordering::Relaxed);
    }

    /// Une image délivrée en aval.
    pub fn produite(&self) {
        self.produced.fetch_add(1, Ordering::Relaxed);
    }

    /// `(ticks, capturées, produites)`.
    pub fn lire(&self) -> (u64, u64, u64) {
        (
            self.ticks.load(Ordering::Relaxed),
            self.captured.load(Ordering::Relaxed),
            self.produced.load(Ordering::Relaxed),
        )
    }
}
```

- [ ] **Step 4 : exécuter pour vérifier le succès**

Run: `cd agent && cargo test -p agent windows_source::telemetrie`
Expected: PASS, 2 tests.

- [ ] **Step 5 : retirer les statiques et brancher le champ**

- Supprimer `windows_source.rs:453-455` ;
- ajouter `pub(crate) telemetrie: Telemetrie` à `WindowsSource` ;
- remplacer `PRODUCED.fetch_add(...)` (l. 313), `TICKS.fetch_add(...)` (l. 518) et `CAPTURED.fetch_add(...)` (l. 544) par `self.telemetrie.produite()` / `.tick()` / `.capturee()` ;
- déclarer `mod telemetrie;` dans `windows_source.rs`.

- [ ] **Step 6 : supprimer le lecteur mort, poser le lecteur vivant**

Supprimer le bloc `demarrage.rs:127-129` et sa lecture de `SOURCE_TRACE`. Le poser dans `capteur/fenetre.rs`, sur le fil de fenêtre — **qui porte déjà son span `session` depuis D7** :

```rust
    // Sous le span `fenetre{session=…}` posé par D7 : la trace est donc
    // attribuable sans champ supplémentaire, ce qui était impossible tant que
    // les compteurs vivaient dans trois statiques de processus.
    if trace_source_active() {
        let (ticks, capturees, produites) = source.telemetrie.lire();
        tracing::info!(ticks, capturees, produites, "compteurs de capture");
    }
```

- [ ] **Step 7 : mettre `CLAUDE.md` à jour sur la dette gelée**

`windows_source.rs` a maigri : relever sa taille **par la commande** et corriger le tableau de dette **à toutes ses places** (voir la contrainte globale et la tâche 17).

- [ ] **Step 8 : vérifier**

Run: `cd agent && cargo test -p agent && cargo check --target x86_64-pc-windows-gnu`
Expected: PASS ; sortie 0 ; **aucun avertissement `dead_code` neuf**.

Run: `wc -l agent/src/windows_source.rs agent/src/windows_source/telemetrie.rs agent/src/capteur/fenetre.rs`
Expected: `windows_source.rs` **strictement inférieur à 638** ; les deux autres < 500.

- [ ] **Step 9 : commit**

```bash
git add agent/src/windows_source.rs agent/src/windows_source/telemetrie.rs agent/src/demarrage.rs agent/src/capteur/fenetre.rs
git commit -m "fix(d9): les compteurs de capture deviennent per-session et sortent du fichier gele"
```

---

### Task 12 : la neutralisation de `set_desired_bitrate`, pour l'A/B

**Files:**
- Modify: `agent/src/transport/part.rs:71` (274 lignes)
- Modify: `scripts/run-agent.sh` (122 lignes)

**Interfaces:**
- Produces: variable de banc `PART_SONDAGE=0`

**Contexte.** L'A/B différentiel n'a **jamais été joué** — « le point ouvert le plus important de D6 ». La méthode que D6 prescrit à elle-même : neutraliser l'appel, opposer les deux trafics cumulés.

- [ ] **Step 1 : ajouter le garde**

```rust
/// L'objectif de sondage est-il armé ?
///
/// **Variable de BANC, pas de produit** : elle n'existe que pour l'A/B
/// différentiel de la consignation n°4 du sous-bloc D6, jamais joué à ce jour.
/// `PART_SONDAGE=0` neutralise `set_desired_bitrate` ; toute autre valeur, et
/// l'absence de variable, l'arment. Même convention que `AUDIO` et
/// `PLEIN_ECRAN` : on désarme sur `=0` ce qui est livré.
///
/// ⚠️ **Ce que l'A/B établira, et rien de plus** : que l'appel a un effet
/// observable sur le trafic émis. Il n'établira PAS qu'il est nécessaire — la
/// prémisse qui le disait « le plus important » a été réfutée par D6 elle-même,
/// le pont portant ≥ 1,44 Gb/s pour `packetsLost = 0`.
fn sondage_arme() -> bool {
    static ARME: OnceLock<bool> = OnceLock::new();
    *ARME.get_or_init(|| {
        let arme = std::env::var("PART_SONDAGE").as_deref() != Ok("0");
        if !arme {
            tracing::warn!("objectif de sondage DESARME (PART_SONDAGE=0) : bras A/B, jamais une configuration livrée");
        }
        arme
    })
}
```

Et au point d'appel (l. 71) :

```rust
        if sondage_arme() {
            self.rtc.bwe().set_desired_bitrate(Bitrate::bps(bps as u64));
        }
```

- [ ] **Step 2 : la transmettre**

Dans `scripts/run-agent.sh`, auprès des autres :

```bash
${PART_SONDAGE:+\$env:PART_SONDAGE = '$PART_SONDAGE'}
```

⚠️ **Cette ligne est le cœur de la tâche**, pas un détail : le piège a été payé en D1, D2 et D6, et sans elle l'agent démarre sans la variable **sans rien signaler**.

- [ ] **Step 3 : vérifier**

Run: `cd agent && cargo check --target x86_64-pc-windows-gnu`
Expected: sortie 0.

Run: `grep -c 'PART_SONDAGE' scripts/run-agent.sh`
Expected: `1`.

- [ ] **Step 4 : commit**

```bash
git add agent/src/transport/part.rs scripts/run-agent.sh
git commit -m "feat(d9): PART_SONDAGE=0 neutralise l'objectif de sondage, pour l'A/B de D6"
```

---

### Task 13 : les trois correctifs d'instrument

**Files:**
- Modify: `docs/superpowers/plans/journaux-multifenetres-d8/instrument/pilote-recette-d8.mjs`

**Interfaces:**
- Produces: l'instrument que les recettes des tâches 14 à 16 réemploient

- [ ] **Step 1 : le seuil `audio_survit` se compare à la dominante**

Aux lignes 1205-1206, remplacer la comparaison au plancher de bruit :

```js
// AVANT : `(niveaux[0].db ?? -1000) - (plancher_db ?? 0) >= 20`, avec
// `plancher_db = -158`. Ce seuil ne pouvait quasiment pas échouer : sur le run
// initial de D8, le niveau à 520 Hz valait -115 dB, soit 79 dB SOUS la
// dominante mesurée (409 Hz, -36 dB) — indiscernable d'une fuite spectrale —
// et il passait quand même. Le verdict ⑤ n'a tenu que parce qu'on l'a déplacé
// à la main sur la dominante.
const dominante = niveaux.reduce((a, b) => (b.db > a.db ? b : a));
const audio_survit = Math.abs(dominante.hz - hz_assignee) <= tolerance_bin_hz;
```

- [ ] **Step 2 : `verdict_partie_mesurable` cesse de contredire le verdict**

`fuite_vers_voisine` doit être **bornée par horodatage** (step 3) avant d'entrer dans ce calcul, faute de quoi le champ vaut `false` là où le document conclut CONFIRMÉ — un lecteur du seul JSON y lirait l'inverse.

- [ ] **Step 3 : `window.__pleinEcran` se borne par horodatage**

```js
// Le tampon côté page n'était jamais vidé ni borné dans le temps : il persiste
// pour toute la vie de la page et n'est purgé qu'à 50 entrées. Toute bascule
// antérieure — y compris la balise d'identité de `resoudreIdentite()` — y
// laissait une trace que `messagesVoisines` relisait SANS filtrer, produisant
// un faux positif de « fuite ». Découvert par le rejeu de D8, non corrigé.
const messages = (await lire('window.__pleinEcran')).filter((m) => m.t >= debutPhase);
```

Poser `m.t` (horodatage) à l'écriture côté page si le champ n'existe pas.

- [ ] **Step 4 : vérifier que l'instrument tourne encore**

Run: `node --check docs/superpowers/plans/journaux-multifenetres-d8/instrument/pilote-recette-d8.mjs`
Expected: aucune sortie (syntaxe valide).

- [ ] **Step 5 : commit**

```bash
git add docs/superpowers/plans/journaux-multifenetres-d8/instrument/pilote-recette-d8.mjs
git commit -m "fix(d9): trois defauts d'instrument, dont un seuil qui ne pouvait pas echouer"
```

---

## Recettes

### Task 14 : recette ① — la chaîne `Resize`, sans changement de mode

> ⚠️ **RÉDUITE le 6 août 2026.** Elle s'appelait « le plein écran armé » et
> devait exercer le critère ② de D8. Le changement de mode ayant été **retiré**
> (tâche 3, sur mesure), il n'y a plus rien à armer : ce qui reste à éprouver est
> la chaîne `Resize` de la tâche 5 et la non-régression de la détection.

**Files:**
- Create: `docs/superpowers/plans/journaux-multifenetres-d9/critere-1-{1,2}.log`, `agent-critere-1-{1,2}.log`

**Interfaces:**
- Consumes: les tâches 3, 5, 13

- [ ] **Step 1 : jouer le contrôle ROUGE d'abord**

Sur le binaire **d'avant la tâche 5**, à `deviceScaleFactor: 2` : l'annonce de
viewport et le `Resize` doivent être dans **deux unités différentes** — c'est le
leg 7. **Sans cette pièce, le critère HiDPI n'est pas un critère**, et le
montage de D8 ne pouvait structurellement pas le voir (dpr = 1 partout).

- [ ] **Step 2 : exécuter la recette, deux fois**

```bash
scripts/run-agent.sh
node docs/superpowers/plans/journaux-multifenetres-d8/instrument/pilote-recette-d8.mjs
```

Le pilote pose `Emulation.setDeviceMetricsOverride` avec `deviceScaleFactor: 2`
sur une des fenêtres et `1` sur les autres, puis **change le viewport** d'une
fenêtre en cours de session.

⚠️ Vérifier `Get-Process agent` **après chaque tentative, y compris échouée** ;
contrôler la survie de la VM après chaque rang.

- [ ] **Step 3 : relever**

```bash
sed 's/\x1b\[[0-9;]*m//g' agent-critere-1-1.log > agent-critere-1-1-plat.log
grep 'contrôle reçu' agent-critere-1-1-plat.log | grep -c 'Resize'
grep 'contrôle reçu' agent-critere-1-1-plat.log | grep 'Resize'   # doivent porter leur session
grep -c 'plein ecran de la fenetre Windows' agent-critere-1-1-plat.log
```

- [ ] **Step 4 : rendre le verdict**

**Tenu** si : (a) un `Resize` émis alors que le canal n'était pas ouvert est
**rejoué** à son ouverture ; (b) chaque `contrôle reçu Resize` porte son champ
`session`, ce qui rend enfin décidable la question du leg 10 — *pourquoi si peu
de `Resize` ?* ; (c) à `deviceScaleFactor = 2`, l'annonce de viewport et le
`Resize` sont dans la **même** unité ; (d) la détection du plein écran annonce
toujours à la bonne fenêtre **et à elle seule**, non-régression de D8.

⚠️ **Le transport n'est disculpé par aucune pièce** (C3 de D8) : si le nombre de
`Resize` reste anormalement bas, le canal de contrôle reste suspect au même titre
que le client — ne pas désigner un maillon sans pièce.

- [ ] **Step 5 : commit**

```bash
git add docs/superpowers/plans/journaux-multifenetres-d9/
git commit -m "recette(d9): la chaine Resize et le HiDPI, deux executions"
```

---


### Task 15 : recette ② — l'audio mort, deux montages

**Files:**
- Create: `docs/superpowers/plans/journaux-multifenetres-d9/critere-2-{a,b}-{1,2}.log`

**Interfaces:**
- Consumes: les tâches 7 à 10

- [ ] **Step 1 : monter les deux configurations**

- **Montage A — réarmement** : une seule fenêtre Chrome `--app`, donc aucune voisine à promouvoir. C'est le cas **majoritaire**.
- **Montage B — promotion** : deux fenêtres Chrome `--app` partageant un `--user-data-dir`, donc **un seul `chrome.exe`**. ⚠️ **Deux `notepad.exe` sont deux PID DISTINCTS** — le piège a été trouvé avant dispatch en D8, ne pas le rejouer. Relever les PID **avant** de conclure, jamais les supposer d'un nom d'exécutable.

⚠️ **Une session WebRTC vivante est requise** : `Session::run()` est la seule boucle qui consomme l'ordre audio du capteur. Un répondeur de viewport nu donne un **faux négatif indiscernable du défaut**. Et le navigateur pilote tourne sur l'**HÔTE**, jamais sur la VM.

- [ ] **Step 2 : jouer le contrôle ROUGE**

Sur le binaire **d'avant les tâches 7 à 9**, provoquer la mort et vérifier que le groupe reste muet définitivement. **Sans cette pièce, le critère ne vaut rien** — c'est la leçon payée trois fois par ce dépôt (F1 de D7, le premier P1 de D8, le parseur de session du pilote de D8).

- [ ] **Step 3 : provoquer la mort de capture**

```bash
node scripts/winrm.js 'Restart-Service Audiosrv -Force'
```

`agent/src/audio.rs:69-70` nomme le redémarrage du service audio parmi les causes transitoires qui justifient les dix réessais : c'est donc le déclencheur légitime, pas un artefact.

- [ ] **Step 4 : relever, à la fréquence dominante**

Le verdict se lit à la **dominante** reçue (`AnalyserNode`), **jamais à un compte d'octets** : D7 a mesuré `bytesReceived` croissant sur un spectre à −1000 dB.

```bash
sed 's/\x1b\[[0-9;]*m//g' agent-critere-2-a-1.log > plat.log
grep -E 'capture audio morte|réarmement programmé|abandon définitif|ordre audio applique' plat.log
```

- [ ] **Step 5 : rendre le verdict**

**Tenu** si la dominante revient dans les **deux** montages, et si le contrôle a été vu **ROUGE** sans le remède. Deux exécutions par montage.

- [ ] **Step 6 : la non-régression du rattachement**

Le montage du critère 2 de D4 : tuer le capteur par PID relevé, vérifier **0** enfant terminé et **0** clôture de session.

⚠️ **La course du leg 2 n'est PAS exercée** — elle ne se provoque pas à la main de façon fiable. Le dire dans le rapport ; le correctif est prouvé par tests d'hôte, pas par la VM.

- [ ] **Step 7 : commit**

```bash
git add docs/superpowers/plans/journaux-multifenetres-d9/
git commit -m "recette(d9): l'audio mort rearme et promeut, deux montages deux executions"
```

---

### Task 16 : recette ③ — le focus à palier long, et l'A/B

**Files:**
- Create: `docs/superpowers/plans/journaux-multifenetres-d9/critere-3-focus-{1,2}.log`, `critere-3-ab-{arme,desarme}-{1,2}.log`

**Interfaces:**
- Consumes: la tâche 12, l'instrument de D6

- [ ] **Step 1 : rejouer le critère ④ de D6 à palier de 60 s**

Réemployer `docs/superpowers/plans/journaux-multifenetres-d6/instrument/pilote-recette-d6.mjs`, en portant le palier de 25 s à **60 s**.

**Pourquoi 60** : `DELAI_REMONTEE` vaut 20 s (`agent/src/congestion/hysteresis.rs`). D6 mesurait à 25 s, soit **25 % de marge**, dans laquelle devaient encore tenir l'annonce de focus, la redistribution des parts et la montée de l'estimation — **trois promotions sur quatorze sont arrivées après la fin du palier**, et une quatrième a été préemptée.

⚠️ **Faire passer la cible par `blur` puis `focus`** : la déduplication d'annonce de `client/src/visibilite.ts` peut sinon faire **disparaître** le focus.

- [ ] **Step 2 : relever le taux de promotion**

```bash
sed 's/\x1b\[[0-9;]*m//g' agent-critere-3-focus-1.log > plat.log
grep -c 'part de budget appliquee' plat.log
```

⚠️ **Énoncer la règle de sélection AVANT de compter.** « 4 déplacements sur 4 » en cachait 14 en D6, dont l'exécution écartée était **précisément celle qui échoue**.

⚠️ **Un changement de barreau produit DEUX lignes** au même horodatage (côté capteur et côté enfant) : tout compteur brut vaut le double.

- [ ] **Step 3 : jouer l'A/B**

Huit fenêtres **toutes ÉVEILLÉES** (`vivier::PLAFOND_EVEIL = 8`), deux bras :

```bash
scripts/run-agent.sh                    # bras ARMÉ
PART_SONDAGE=0 scripts/run-agent.sh     # bras DÉSARMÉ
```

⚠️ **Le témoin DOIT porter sur des éveillées.** C'est ce qui a empêché la recette de D6 de servir : une endormie émet **0,000 Mb/s sur 30 s**, si bien que « sondage borné » et « pas de sondage du tout » s'y lisent à l'identique.

- [ ] **Step 4 : opposer les deux trafics cumulés**

Relever `bytesSent` cumulé sur les huit sessions, aux deux bras, deux exécutions chacun.

- [ ] **Step 5 : rendre le verdict, sans le surinterpréter**

L'A/B établit que l'appel a **un effet observable sur le trafic émis**. Il n'établit **pas** qu'il est nécessaire : la prémisse qui le disait « le plus important » est réfutée par D6 elle-même.

- [ ] **Step 6 : commit**

```bash
git add docs/superpowers/plans/journaux-multifenetres-d9/
git commit -m "recette(d9): le focus a palier long, et l'A/B jamais joue de D6"
```

---

### Task 17 : revue transverse, et remise à jour de `CLAUDE.md`

**Files:**
- Create: `docs/superpowers/plans/2026-08-06-multifenetres-solder-la-dette-resultats.md`
- Modify: `CLAUDE.md`

- [ ] **Step 1 : la revue transverse**

Elle a trouvé **cinq** défauts en D7 et **trois** Critiques en D8, tous franchissant une **frontière de tâche** : chacun était correct des deux côtés pris séparément. **Une revue par tâche ne peut structurellement pas les voir.** Relire l'ensemble de la branche d'un seul tenant, en cherchant les contrats qui traversent deux tâches.

- [ ] **Step 2 : relever les tailles PAR LA COMMANDE**

```bash
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | head -25
```

- [ ] **Step 3 : corriger `CLAUDE.md` À TOUTES LES PLACES**

⚠️ **« Corrigé à sa place » est une affirmation de COMPLÉTUDE, et une affirmation de complétude se vérifie en énumérant les places AVANT de l'écrire.** Ce dépôt a payé **cinq** naufrages successifs sur ce geste. Pour chaque nombre modifié :

```bash
grep -n '<le nombre>' CLAUDE.md
```

Et traiter **toutes** les occurrences, **y compris le récapitulatif de tête** — dont ce fichier écrit trois fois qu'il est le seul qu'on lise.

⚠️ **Relever les tailles APRÈS les dernières éditions de la ronde**, jamais avant : une table corrigée sur une mesure de début de ronde est fausse à la fin de la même ronde.

- [ ] **Step 4 : écrire le document de résultats**

Chaque énoncé porte **son nombre d'exécutions**. Chaque leg est déclaré **fermé sur pièces** ou **non exercé**, jamais entre les deux. La liste « ce que D9 n'établit PAS » reprend au minimum le §11 de la spec.

- [ ] **Step 5 : commit et fusion**

```bash
git add CLAUDE.md docs/superpowers/plans/
git commit -m "docs(d9): resultats du sous-bloc, et les chiffres releves par la commande"
```

---

## Auto-revue du plan

**Couverture de la spec** — chaque section a sa tâche :

| Spec | Tâche(s) |
| --- | --- |
| §3 Phase P (éliminatoire, 4ᵉ combinaison, contre-épreuve, inconnues annexes, règle de décision) | 1, 2 |
| §4.1 C1 | 1 et 2bis (mesure) ; **3 (retrait — C1 cesse d'exister)** |
| §4.2 C2 | **sans objet depuis le 6 août 2026** : `reconstruire_sur_la_sortie` n'a qu'un appelant, le changement de mode, supprimé en tâche 3 |
| §4.3 HiDPI | 5 (step 6), 14 (mesure à `deviceScaleFactor: 2`). ⚠️ Le défaut perd sa conséquence produit avec le retrait, mais l'asymétrie d'unité reste et se corrige |
| §4.4 legs 8 et 10 | 5 (steps 1-5, 7), 14 (step 3) |
| §5.1 leg 1 | 7 (règle pure), 8 (registre), 9 (message), 15 (recette) |
| §5.2 leg 2 | 10, 15 (step 6) |
| §5.3 plafond de `serveur.rs` | 6 |
| §6.1 leg 3 | 11 |
| §6.2 leg 4 | 16 (steps 1-2) |
| §6.3 leg 5 | 12, 16 (steps 3-5) |
| §6.4 leg 9 | 13 |
| §7.1 « vu rouge » | 14 (step 1), 15 (step 2) |
| §7.2 deux exécutions | 2, 14, 15, 16 |
| §7.4 revue transverse | 17 |
| §8 plafond de 500 lignes | contraintes globales, 6, 11, 17 |

**Cohérence des types** — vérifiée : `FenetreAudio.inapte: bool` (tâche 7) est rempli par `porteurs.rs` (tâche 8, step 5) depuis `Etat.inaptes: HashMap<String, Instant>` (tâche 8, step 2) ; `VersCapteur::AudioMort` sans charge utile (tâche 9, step 1) est consommé par `commandes.rs` via `ctx.session` (step 3) et produit par `signaler_audio_mort` (steps 4-5) ; `Telemetrie::lire() -> (u64, u64, u64)` (tâche 11, step 3) est consommé au step 6 dans cet ordre.

**Ordre imposé** : la tâche 2, complétée par la tâche 2bis, a commandé le sort des tâches 3 à 5 — la famille ① est devenue un retrait ; la tâche 6 précède obligatoirement la tâche 9 (marge de 10 lignes sur `serveur.rs`) ; la tâche 7 précède la 8 ; la 12 précède la 16. Les familles ② et ③ ne dépendent d'aucune tâche de la famille ①.
