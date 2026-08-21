# Sous-projet ① « Divers » — **A1** : la couleur d'accent — RÉSULTATS

> 🔴 **CE DOCUMENT EST PERMANENT ET VERSÉ DANS GIT, ET C'EST DÉLIBÉRÉ.** Le
> sous-bloc D9 a perdu **six** constats de revue avec son espace de travail
> `.superpowers/sdd/`, gitignoré et jamais commité, en laissant quatre phrases
> qui invitaient à « relire » un rapport qui n'existait plus. **La preuve d'une
> affirmation du dépôt ne doit jamais vivre dans un rapport gitignoré.**
> Les pièces brutes vivent sous `journaux-accent-a1/`.

## Ce que A1 livre, mesure, et ne mesure pas

Plan : `docs/superpowers/plans/2026-08-21-accent-a1.md` (commit `375b894`).
Conception : `docs/superpowers/specs/2026-08-19-presse-papier-design.md`, §6.4
et décisions **D9** et **D10** — **annotées par A1** : *deux de leurs clauses
sont mesurées inapplicables* (voir ① et ② ci-dessous).
Résultats : `docs/superpowers/plans/2026-08-21-accent-a1-resultats.md`.
Journaux : `docs/superpowers/plans/journaux-accent-a1/` — **UNE SEULE FAMILLE
DE LECTURE, la plus simple du dépôt, et c'est MESURÉ** (`file`,
`grep -lP '\x1b\['`, `grep -lU $'\r'`, balayage `tr -dc '\000'`) : **UTF-8
partout, aucune séquence ANSI, aucun `\r`, aucun octet NUL**. Ils se `grep`ent
à plat, sans `sed`, sans `grep -a`. La raison est structurelle : ce sont des
sorties `npm`/`cargo`/`node` sur l'**hôte**, jamais du PowerShell distant — le
défaut à deux réglages ne peut pas les atteindre.

⛔ **A1 N'A PAS TOUCHÉ LA VM**, et ce n'est pas un choix : elle a été tenue par
le sous-bloc **G4** de la gestion d'apps pendant toute la durée du sous-bloc.
**Douze tâches sur dix-sept n'en dépendaient pas**, et elles sont faites ; les
tâches **13 et 14 — l'instrument et la recette — NE SONT PAS FAITES.**

### ① 🔴 Deux clauses de la spécification sont MESURÉES INAPPLICABLES

**D9 prescrit de relire l'icône « sur le même tour de roue que le presse-papier ».
Le tour de roue n'a pas le `hwnd`.** Lecture de code, pas opinion : aucun des
quinze champs d'`Etat` (`capteur/sommeil/registre.rs`) ne le porte, et
`inscrire(session, pid)` ne le prend pas. Le presse-papier y vit parce qu'il est
**global à la window station** ; **l'accent est PAR FENÊTRE** — la propriété même
que D9 revendique. A1 l'a donc porté sur le **fil de fenêtre**
(`agent/src/capteur/fenetre/accent.rs`), où vit déjà le plein écran de D8 dont il
reprend le patron : **trois maillons disparaissent**, `registre.rs` n'est pas
touché, et le rejeu à l'inscription — qui a coûté **deux tâches à P3** — devient
**sans objet**, un rattachement recréant le fil de fenêtre donc un `SuiviAccent`
neuf donc une première annonce.
🔵 **Effet de bord favorable** : R4 de la spec annonce qu'un `WM_GETICON` bloqué
« gèle le tour de roue, donc TOUTES les fenêtres ». Sur le fil de fenêtre il ne
gèle **que la sienne**. ⚠️ **La gravité baisse, elle ne disparaît pas** — ce fil
produit les images de cette fenêtre, et `SMTO_ABORTIFHUNG` reste obligatoire.

**D10 point 1 fait déclarer `--accent-fenetre` dans les trois blocs de thème. Les
DEUX voies qu'elle déclare « acceptables » sont mesurées impraticables.** C'est
la sonde **H1**, jouée hors VM sur un arbre jetable, cinq cellules, **une
exécution chacune** (`journaux-accent-a1/01-sonde-h1.log`) :

| Cellule | Ce qui est posé | Relevé |
| --- | --- | --- |
| A | `--accent-fenetre: var(--accent);` à `:root` seul | §7.4 **VERT** ; §7.6 **ROUGE** — « NOUVEL ORPHELIN — déclaré et appelé par personne » |
| B | idem + une ligne d'attente | **7/7 VERT**… puis **ROUGE** dès que `'A1'` entre dans `SOUS_BLOCS_CLOS` : « SOUS-BLOC CLOS — nommait A1, qui est clos » |
| C | une **vraie couleur** à la racine seule | §7.4 **ROUGE** — « couleur de la racine sans contrepartie claire » |
| **D** | **aucune déclaration** + un `setProperty` dans un `.ts` | 🔴 **LES NEUF VERTS** |
| E | idem D + une couleur **littérale** en dur | §7.2 **ROUGE** |

🔴 **La voie « ligne d'attente » n'est tenable QUE SI A1 REFUSE DE SE DÉCLARER
CLOS**, ce que la convention exige — c'est pire que le mal qu'elle soigne. Et
`client/src/design/tokens.css` est à **300/300, marge NULLE**, sa scission ayant
été **délibérément écartée** par ⑥ (sept lecteurs le nomment par son chemin).
**A1 a donc pris une TROISIÈME voie que la spec n'envisageait pas : ne pas
déclarer le token.** Le client le pose sur `document.documentElement` à
l'exécution, et **le repli est LU** (`getComputedStyle` sur `--accent`) plutôt
qu'écrit — une couleur en dur dans un `.ts` ferait rougir §7.2, **mesuré**.
⚠️ **Ce n'est pas un contournement** : §7.6 a juridiction sur les tokens
**déclarés dans `tokens.css`**. Le jour où G5 écrira `var(--accent-fenetre)`
dans une feuille, sa **première** inclusion rougira et le forcera à déclarer —
**au moment où il aura un appelant à donner.**
⚠️ **Ce que cette voie coûte** : avant le premier message,
`var(--accent-fenetre)` est **indéfini**. Aucune feuille ne le référence
(mesuré) ; **toute référence future doit porter un repli ou déclarer le token**.

### ② 🔴 A1 EST LIVRÉ SANS AUCUN EFFET VISIBLE, et cela ne se maquille pas

**Aucun rendu n'existe avant qu'un manifeste PWA soit posé — travail de G5, qui
n'a pas de plan.** Les critères portent sur la **valeur** et sa **garantie**,
jamais sur un rendu. **Personne n'a vu une couleur d'accent à l'écran**, et A1
n'invente aucun critère qui aurait l'air de le couvrir. C'est le même statut que
la partie **WCO** de S4.

> ⚠️ **LE TABLEAU DE DETTE N'A PAS BOUGÉ SOUS A1, ET UNE AFFIRMATION DE SON
> JOURNAL D'ENTRÉE ÉTAIT NON MESURÉE.** La commande du § « Conventions de code »,
> relancée **après la dernière édition de la ronde**, rend **DEUX** lignes et
> deux seulement — `agent/src/encode.rs` **1536** et
> `agent/src/windows_source.rs` **630** —, et **A1 n'a touché ni l'un ni
> l'autre**.
>
> ⚠️ **Le plan de A1 déclarait « le tableau publie QUATRE lignes, le dépôt en a
> DEUX », et son journal d'entrée l'a RECOPIÉ SANS LE VÉRIFIER.** C'était vrai
> le 20 août ; le **sous-bloc G2** a résorbé les deux lignes de `proto/` le
> 21 août, dans ses tâches 1 et 2. **Il n'y avait donc rien à corriger**, et
> l'écart n'est pas au débit de A1 — mais *la seule façon de le savoir était de
> lire la table, ce que le journal d'entrée n'a pas fait.*
>
> ⚠️ **Marges les plus serrées du dépôt à cette date, relevées par la commande** :
> `plateforme/src/http/routes-installation.ts` **500** et
> `agent/src/encode/arret.rs` **500** (marge 0, ex æquo),
> `client/verify-webrtc.mjs` **494**, `agent/src/capture.rs` **492**,
> `agent/src/superviseur/lanceur.rs` **488**. **Fichiers de A1 les plus
> gros** : `agent/src/capteur/fenetre.rs` **484** (marge 16),
> `client/src/main.ts` **483** (17), `agent/src/transport/tick.rs` **467** (33),
> `agent/src/source.rs` **460** (40), `proto/src/control.rs` **444** (56).
> **Aucun fichier créé par A1 ne dépasse 214 lignes.**

### ③ Ce qui est mesuré, et en combien d'exécutions

**Aucun taux n'est revendiqué nulle part.** Les tests d'hôte sont
**déterministes** : une exécution y établit un fait, jamais une fréquence.

| Objet | Verdict | Exéc. |
| --- | --- | --- |
| Sonde **H1** (5 cellules) | **D-A1-2 CONFIRMÉE** — la première des quatre issues écrites d'avance, et **trois** la réfutaient | 1 par cellule |
| Critère **③** — une couleur illisible est refusée | 🔴 **VU ROUGE**, sur son assertion propre : `expected '#1e2229' to be '#7aa2f7'` | 1 rouge + vert |
| Le trou de `pont_media.rs` | 🔴 **ROUGE JOUÉE AVANT LE BRAS** : `RecvError` sur la toute première annonce, 6 passed / 1 failed | 1 |
| `TOUS_AGENT` (seul garde `tsc`) | 🔴 **VU ÉCHOUER** : « Property 'accent' is missing » | 1 |
| Branche `a1nonies` | 🔴 rouge T1 : les DEUX tests tombent | 1 |
| Critères **①, ②, ④** | ⛔ **NON MESURÉS** — ils exigent la VM | **0** |
| Critère **⑤** (les NEUF contrôles) | **TENU** — 7/7 scripts + §7.5 et §7.10 | 2 |

**Comptes de clôture, tous ANNONCÉS avant d'être mesurés** : `cargo test -p agent`
944 → **958** (+14), `cargo test -p proto` **109** (inchangé), `client` 466 →
**479** (42 fichiers), `proto` 296 → **298** (9 fichiers), `design:verifier`
**7/7**, `verify-all.sh` depuis un shell propre → **« Les 10 étapes sont
passées. »**

### ④ 🔴 DEUX DÉFAUTS QUE A1 A INTRODUITS, ET QUI ONT COÛTÉ UNE MESURE À UN VOISIN

**Signalés par un chantier voisin, ÉTABLIS PAR PIÈCE, et les deux sont de A1.**

**a) `design:verifier` §7.2 a été ROUGE du commit `46e3aa8` au commit `135d632`.**
Bissection par `git archive` + `couleurs-litterales.mjs --racine` : `4d950be` →
**0**, `46e3aa8` → **21**, `077af01` → **21**. Cause : les fichiers de tests de
A1 portaient des couleurs en **littéral**, et §7.2 balaie les `.ts` de
`client/src/` — son exclusion ne couvre que le socle
(`client/src/design/*.test.ts`). **Faute propre : `design:verifier` n'avait pas
été lancé avant de committer.** Le plan ne le prescrivait qu'à la recette
(critère ⑤) : *c'est une porte de SORTIE à laquelle il manquait une porte de
PASSAGE.*
⚠️ **Le remède retenu n'est ni d'élargir l'exclusion ni de ranger les fixtures
dans un `.json` que §7.2 ne balaie pas** — la première viole D-A1-14, la seconde
serait **satisfaire un contrôle en le vidant**. **Zéro littéral, tout LU dans
`tokens.css`** : `--bord` pour l'illisible (1,447 / 1,336 / 1,215), `--succes`
pour le lisible (8,867 / 8,186 / 7,446), le troisième fond **égal à la
candidate** pour le « deux sur trois » (1,000), et les formes non hexadécimales
**dérivées** d'un token. 🔵 **Et cela a rendu un test MEILLEUR** : `--succes`
vaut 2,194 / 2,047 / 1,889 sur les fonds **clairs**, donc **la même couleur est
acceptée en sombre et refusée en clair** — le fixture exact du test de bascule
de thème, que les littéraux n'exprimaient qu'approximativement.

**b) 🔴 L'ARBRE N'A PAS COMPILÉ ENTRE `9e05f1c` ET `022dde2`.** Reconstruit dans
un arbre jetable : `error[E0004]: non-exhaustive patterns:
&AgentControl::Accent { .. } not covered --> agent/src/transport/controle.rs:98`.
**Cause : l'ordre du plan n'est compilable dans AUCUN sens.** Son §7 ordonne
8 → 9 → 10, mais la tâche 9 appelle `AgentControl::accent` que la tâche 10 crée ;
et l'ordre inverse échoue aussi, `transport/controle.rs` portant un `match`
**exhaustif** dont le bras est prescrit à la tâche 9.
🔴 **CE QUE CELA A COÛTÉ** : un voisin compilait sur la VM depuis l'arbre partagé
(`build-agent.sh` rsynchronise tout). Son contrôle d'auto-identification a rendu
**0**, et il a **abandonné son diagnostic plutôt que de le jouer faux** — rien de
faux n'a été publié, **une mesure est perdue**.

> 🔴 **LA RÈGLE QUI EN SORT, ET QU'AUCUN PLAN DE CE DÉPÔT NE PORTAIT** : *un
> `match` exhaustif dans un crate **AVAL** rend toute addition de variante
> **AMONT** cassante. Le bras part dans le MÊME commit que la variante, ou les
> deux tâches n'en font qu'une.* Et **vérifier `git status proto/ agent/` avant
> le PREMIER build ne suffit pas** : il faut s'assurer que **chacun de ses
> propres commits compile**, parce qu'un voisin bat depuis le même arbre.

### ⑤ Le trou payé cinq fois l'a été une SIXIÈME, et la rouge a précédé le bras

`agent/src/capteur/pont_media.rs` porte un `Ok(autre) => { warn!; return; }` qui
**tue le fil `lire_le_media` en silence**. `Sommeil` (D5), `Part` (D6), `Audio`
(D7), `PleinEcran` (D8), `PressePapier` (P1) — **`Accent` est la sixième.** La
rouge a été jouée **avant** le bras : `called Result::unwrap() on an Err value:
RecvError`, 6 passed / 1 failed. Le contrôle prescrit
(`grep -cn 'DepuisCapteur::Accent' pont_media.rs`) rend **quatre**.

🔴 **ET UN ONZIÈME MAILLON QUE LE PLAN N'AVAIT PAS INVENTORIÉ** (son §6.1 en
compte dix) : **`proto/src/control/redaction.rs`**, les deux `impl Debug`
**écrites à la main** qui ont remplacé le `#[derive(Debug)]` retiré par P2 après
la fuite du presse-papier **en clair dans `agent.log`**. Il est **gardé par le
compilateur**, et il a fait exactement ce que son en-tête promet : *forcer une
décision*. **Décision prise et écrite** : montrer la couleur, parce que ce
message ne porte **rien de privé** — ni `hwnd`, ni PID, ni titre de fenêtre, ni
chemin d'exécutable.

### ⑥ Une ROUGE est restée VERTE, et elle a été DIAGNOSTIQUÉE

La rouge T2 mutait `SourceDistante::accent_a_annoncer` en `.clone()` et attendait
que `l_accent_annonce_est_consomme_…` tombe. **Il est resté vert : 12 passed, 0
failed.** Cause, trouvée en **relisant** le test : il emploie une source
**factice** dont la consommation lui est propre — il éprouve le **câblage** de la
branche, jamais `SourceDistante`. **La mutation visait un chemin que le test
n'emprunte pas.**
🔴 **CE QUE CELA A RÉVÉLÉ : la consommation du VRAI `SourceDistante` n'était
couverte par RIEN.** Deux tests neufs dans `capteur/distante/tests_etats.rs`, et
T2 rejouée : **1 failed sur 14, sur l'assertion visée.** *La classer « le test ne
peut pas échouer » aurait publié l'inverse de la vérité, et laissé le trou.*

### ⑦ Le plafond franchi, et rattrapé par une EXTRACTION

`agent/src/capteur/fenetre.rs` serait tombé à **500 EXACTEMENT** — marge nulle —
parce que **le plan chiffrait l'addition à « ~18 lignes » et qu'elle en pèse 55**.
Rattrapé par `capteur/fenetre/accent.rs` (**87**), jamais par une compression ;
`fenetre.rs` finit à **484**. Ce fichier avait déjà franchi 500 **deux fois**
(508 en D9, 505 en D10).
⚠️ **Et le piège s'est rejoué DANS L'ACTE MÊME DE DOCUMENTER L'EXTRACTION** : le
commentaire écrit pour expliquer le `#[path]` a porté le fichier de 477 à **490**
— *treize des vingt-trois lignes que l'extraction venait de rendre.* « Une
addition de commentaire peut annuler une extraction » (P2, S2 deux fois, S3).
Corrigé **en ne gardant que le contenu unique**, la raison de l'extraction vivant
dans l'en-tête du module extrait.
✅ **Une seconde extraction, celle-là PRÉALABLE et jouée dans sa propre tâche** :
`agent/src/capteur/distante.rs` **472 → 203**, l'`impl VideoSource` partant
**verbatim** (`diff` vide) vers `distante/video_source.rs`.
⚠️ **Une extraction rigoureusement verbatim NE COMPILE PAS** : le module enfant
a besoin de son propre bloc `use`, le parent perd trois imports, **et les deux
fichiers de tests ont besoin de `use crate::source::VideoSource` en propre** — un
trait doit être en portée pour que ses méthodes soient appelables.

### ⑧ Ce que A1 N'ÉTABLIT PAS

- 🔴 **RIEN DE LA VM.** Les critères ①, ② et ④ ne sont **pas mesurés**, l'agent
  n'a **pas été rebâti**, **aucune icône réelle n'a jamais été lue**, et **aucune
  dominante n'a été calculée sur autre chose que des pixels de test**.
- 🔴 **`agent/src/accent/win32.rs` (200 lignes) n'est couvert par AUCUN test** :
  il est `#[cfg(windows)]`, et `cargo check --target x86_64-pc-windows-gnu`
  vérifie types, emprunts, visibilités et durées de vie — **jamais le
  comportement**. Sa seule preuve de fonctionnement serait la recette.
- 🔴 **La conversion BGRA → RGBA n'est éprouvée par rien** (RA1-6) : elle vit
  derrière le `cfg`, aucun test d'hôte ne la voit, et **son seul contrôle réel
  est le critère ①**, non joué. Se tromper de sens échangerait le rouge et le
  bleu, **silencieusement**.
- **`SMTO_ABORTIFHUNG` est POSÉ, JAMAIS EXERCÉ** ; le repli `GetClassLongPtrW`
  n'a **jamais couru** — code livré, chemin probablement jamais emprunté, comme
  `borner_a_la_taille_max` l'a été un sous-bloc entier.
- **Aucun jugement visuel n'est porté sur aucune couleur**, ni sur la dominante
  calculée ni sur son accord avec l'icône. Le **contraste** est mesuré ; la
  **justesse** de la teinte ne l'est pas.
- 🔴 **Une couleur reçue du serveur est STRUCTURELLEMENT hors de tout contrôle de
  contraste automatique de ce dépôt** : §7.2 est syntaxique **et le déclare**,
  §7.1 compare une liste de paires **écrite à la main**. Le seul rempart est un
  test d'hôte.
- **Les deux lignes de `client/src/main.ts` ne sont couvertes par aucun test**,
  et leur seul contrôle est le critère ② — non joué.
- **SEPT constantes non calibrées** : `PERIODE_ACCENT`, `DELAI_MS`, `ALPHA_MIN`,
  `SATURATION_MIN`, `LUMA_MIN`, `LUMA_MAX`, `PAS`. Elles rejoignent la liste que
  ce dépôt tient depuis `BPP_MIN`.
- **L'icône ne suit pas le thème du document** — la régression que D9 assume sur
  un axe. A1 ne la mesure pas et ne la corrige pas.
- **Rien d'une icône qui CHANGE en cours de session**, rien de la latence, rien
  au-delà d'une fenêtre.

### ⑨ Ce que A1 lègue

1. ⛔ **G5 — `--accent-fenetre` n'est peint par rien, ET SA DÉCLARATION N'EXISTE
   PAS.** Les deux sont **indissociables** : le jour où G5 écrira
   `var(--accent-fenetre)` dans une feuille, §7.6 rougira sur sa **première**
   inclusion et le forcera à déclarer le token dans les trois blocs — **et il
   paiera alors la scission de `tokens.css`**, à 300/300, dont le point de chute
   est écrit dans le fichier même. ⚠️ **G5 n'a pas de plan**, et rien ne garantit
   sa date.
2. 🔴 **Les critères ①, ② et ④ restent à jouer sur la VM**, avec l'instrument
   (tâche 13) et la recette (tâche 14) **non faits**. ⚠️ **Le critère ① est NON
   MESURABLE si les deux icônes ont la même dominante** : la sonde qui le dirait
   n'a pas été jouée.
3. ⛔ **La ligne `ACCENT` de `scripts/run-agent.sh` n'a jamais été lue dans le
   `run-agent.ps1` GÉNÉRÉ** — le seul contrôle qui vaille. Dette de vérification.
4. ⛔ **Le point de réunion avec ④ reste PROPOSÉ** : `agent/src/accent.rs::dominante`
   est le candidat, et G5 aura un **PNG** là où A1 a un **DIB** — besoin
   différent en amont, identique en aval. ⚠️ Relevé avant d'écrire :
   `agent/src/apps/icone.rs` fait 53 lignes et **ne contient aucune fonction** ;
   `HICON`, `GetIconInfo`, `WM_GETICON` et `GCLP_HICON` avaient **zéro
   occurrence** dans le dépôt. **A1 a tout écrit.**
5. ⛔ **Aucune mémoire d'accent nulle part.** Si un rejeu devient nécessaire, le
   point de chute est `SourceDistante`, pas le registre.
6. ⚠️ **`client/src/design/galerie.ts` affirme être « LE PREMIER APPELANT de
   `getComputedStyle` ».** A1 en pose un second, et **le premier de PRODUIT**.
   **Consigné, non corrigé** : `client/src/design/` est hors périmètre.
7. ⚠️ **`proto/src/control.rs` dit « `pub fn ready` est TRENTE-DEUX lignes plus
   haut ».** Mesuré : **36**, avant comme après A1 — *elle était déjà fausse*, et
   la phrase qui la porte est celle qui énonce « nommer la chose, jamais compter
   les lignes qui l'en séparent ». **Consigné.**
8. ⚠️ **`agent/src/plateforme/{session,tests_installation}.rs` disent « la
   CINQUIÈME fois que ce dépôt paie la leçon du bras manquant ».** A1 en fait la
   **sixième** sur `pont_media.rs`. Ces places comptent peut-être une **autre**
   liste : **consigné pour leur propriétaire, non corrigé.**

### ⑩ Pièges neufs — à connaître avant de toucher à ce terrain

- 🔴 **ZSH NE DÉCOUPE PAS LES VARIABLES EN MOTS**, et le symptôme n'est pas celui
  qu'on attend : une commande de contrôle passée par `$VAR` à un harnais arrive
  **entière comme nom de fichier**, « Aucun fichier ou dossier de ce nom »,
  **code 127** — c'est-à-dire **UN CONTRÔLE QUI N'A PAS TOURNÉ**, indiscernable
  d'un échec si l'on ne lit que le verdict. *C'est le harnais qui l'a dit, en
  imprimant le code de sortie du contrôle.* **Écrire les commandes littéralement.**
- 🔴 **LE REPORTER PAR DÉFAUT DE VITEST DÉDUPLIQUE LES ÉCHECS** et n'en montre
  **qu'un**. Une première passe de rouges ne montrait qu'un échec sur trois. Un
  relevé qui doit dire **quelle assertion** a rougi ne peut pas s'en contenter :
  `--reporter=verbose`.
- 🔴 **DES BACKTICKS DANS UN `echo` DE JOURNAL EXÉCUTENT UNE COMMANDE.** Écrire
  « un `if let` oublié compile » a fait rendre « parse error near `let` » et **a
  tué le script au milieu**. Piège documenté par S4, payé ici. Quotes simples.
- 🔴 **UNE ANCRE DE MUTATION PEUT EXISTER CINQ FOIS.** L'assertion « aucune trame
  de plus après la fin du tampon » est partagée par **cinq** tests de
  `pont_media.rs` : un `replace(…, 1)` aurait frappé un **autre** test.
  **C'est `assert t.count(ancre) == 1` qui est le garde, jamais la relecture.**
- ⚠️ **`git status --porcelain` sur un fichier NEUF rend `??` et non le vide** :
  un harnais de rouge qui exige « vide » à l'étape 7 échoue sur tout fichier pas
  encore commité. **La preuve de restauration qui vaut y est le `sha256`.**
- ⚠️ **UN COMPTE DE TESTS N'EST ATTRIBUABLE QU'ASSORTI DE SON HEURE**, et A1 l'a
  payé **trois fois en une journée** : 944/0 à l'entrée, 942/2 une heure plus
  tard (un `MORCEAUX_EN_VOL = 1  // 🔴 DIAG F4, jamais fusionné` **non commité**
  du voisin), 939/15 puis 954/0 **à la relance sans aucune modification**.

---

---

## ⑪ La famille de lecture des journaux, MESURÉE et non supposée

| Famille | Fichiers | Ce qu'il faut faire |
| --- | --- | --- |
| **tous** — les neuf `*.log` et `instrument/harnais-rouge.sh` | **10** | **rien** : ils se `grep`ent à plat |

Relevé le 21 août 2026, **après la dernière écriture** :
`file` rend « Unicode text, UTF-8 text » partout ; `grep -lP '\x1b\['` rend la
**liste vide** ; `grep -lU $'\r'` rend la **liste vide** ; un balayage
`tr -dc '\000'` rend **zéro octet NUL** sur les dix.

⚠️ **Ce n'est pas un mérite, c'est une propriété du chemin** : ce sont des
sorties `npm`, `cargo` et `node` sur l'**hôte**, jamais du PowerShell distant.
Le défaut à deux réglages de `build-agent.sh`/`run-agent.sh` — **toujours non
corrigé** — ne peut pas les atteindre. Les journaux d'une recette VM, eux, en
auraient trois familles ; **A1 n'en a produit aucun.**

## ⑫ L'inventaire des pièces

| Pièce | Ce qu'elle établit |
| --- | --- |
| `00-controle-entree.log` | la ligne de base, six comptes **relancés**, la commande de tailles, les quatre `clippy` hors `dead_code` **préexistants** |
| `01-sonde-h1.log` | **les cinq cellules de H1** — la mesure qui fonde la troisième voie de D-A1-2, avec les messages verbatim |
| `02-rouges-module-pur.log` | **ROUGE 0 refusée** + huit rouges du module pur, chacune sur son assertion |
| `03-rouges-conformer.log` | **ROUGE 0 refusée** + six rouges, dont 🔴 **celle du critère ③** |
| `04-rouge-pont-media.log` | 🔴 **la rouge jouée AVANT le bras** : `RecvError` |
| `05-rouge-tous-agent.log` | 🔴 **`tsc` VU échouer** sur `TOUS_AGENT`, et le onzième maillon |
| `06-rouges-a1nonies.log` | **ROUGE 0 refusée**, T1, et 🔴 **T2 restée verte, DIAGNOSTIQUÉE puis rejouée** |
| `07-defaut-72-et-arbre-casse.log` | 🔴 **les deux défauts de A1**, bissectés par `git archive` |
| `08-revue-transverse.log` | 2 corrigées, 7 consignées, le coût mesuré (+7 lignes), le contrôle de périmètre |
| `instrument/harnais-rouge.sh` | le harnais à huit étapes, **qui REFUSE une rouge qui ne mute rien** |

## ⑬ Le harnais de rouge, et pourquoi il refuse `git diff`

Le §6.4 du plan l'exige, et sa clause décisive est mesurée par P3 : **le contrôle
« la mutation a-t-elle changé quelque chose ? » ne peut PAS être `git diff
--numstat`**, qui compare à **HEAD** et reste donc non vide tant qu'un correctif
non commité vit dans le fichier — **même quand la mutation est nulle**. Le
harnais compare à une **COPIE NOMMÉE**, restaure **depuis elle** (jamais
`git checkout --`, qui restaure à HEAD et a effacé du travail non commité deux
fois dans ce dépôt), et vérifie l'égalité des `sha256`.

🔴 **Une « ROUGE 0 » — une mutation qui ne mute rien — a été jouée EN PREMIER
devant chaque famille de rouges, et le harnais l'a REFUSÉE les trois fois** :
« DIFF VIDE — LE HARNAIS REFUSE CETTE ROUGE ». C'est *« un contrôle qu'on n'a
jamais vu rouge n'est pas un contrôle »*, appliqué **au harnais lui-même**.

⚠️ **Une correction du harnais a dû être faite en cours de route** : son étape 7
exigeait un `git status --porcelain` **vide**, ce qu'un fichier **neuf** ne rend
jamais (il rend `??`). Corrigé pour le dire, plutôt que pour l'ignorer — la
preuve de restauration qui vaut y est le `sha256` de l'étape 6.
