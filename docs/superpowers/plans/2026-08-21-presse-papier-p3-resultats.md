# Sous-projet ① « Divers » — presse-papier, sous-bloc **P3** : les N fenêtres — résultats

> ⚠️ **TROIS « P3 » PEUVENT SE CONFONDRE DANS CE DÉPÔT.** Celui-ci est le
> **presse-papier**. Le sous-projet ⑤ (plateforme) a un « P3 » clos et sans
> rapport ; le chantier D a des sous-blocs `D1`…`D11`. Quand ce document écrit
> « P1 » ou « P2 », il désigne **toujours** le presse-papier.

Plan : `2026-08-21-presse-papier-p3.md`.
Conception : `../specs/2026-08-19-presse-papier-design.md` — **annotée par ce
sous-bloc**, à son §3.3, son §4.2 et son §6.3.
Journaux : `journaux-presse-papier-p3/`.

---

## 0. 🔴 LE VERDICT D'ENSEMBLE : LA RECETTE N'A PAS EU LIEU

**Onze tâches sur treize sont faites. La tâche 10 — la recette sur la VM
Windows — n'a PAS été jouée**, et avec elle rien des quatre critères ①②③④ en
conditions de produit.

**La raison est nommée d'avance par le plan (RP3-4) et elle est externe** : la
VM était tenue par le chantier **F3** (pont fichiers), qui y mesurait et portait
du travail **non commité** dans `agent/`. `scripts/build-agent.sh` rsynchronise
l'arbre entier : le lancer aurait poussé sur la VM du travail à demi fait, puis
écrasé le binaire que F3 était en train de mesurer. Le plan classe explicitement
la tâche 10 comme un **préalable EXTERNE, jamais une dépendance de tâche**.

**Ce qui est donc établi** : du code, des tests d'hôte, douze rouges jouées, et
**DEUX SONDES MESURÉES HORS VM, deux exécutions chacune**. **Rien du produit en
marche à N fenêtres.**

> 🔵 **LE BLOCAGE A ÉTÉ LEVÉ À LA TOUTE FIN DE LA RONDE, ET IL FAUT LE DIRE POUR
> QUE CE DOCUMENT NE MENTE PAS PAR VIEILLISSEMENT.** F3 a clos son sous-bloc
> (commits `30bc42e` puis `6f0aaa5`), `git status --porcelain` est redevenu
> **vide**, et `Get-Process agent` sur la VM rend **0** — la VM est libre.
> **Cela s'est produit APRÈS que tout le travail hôte de P3 était fait et
> commité**, et la recette n'a donc pas été jouée pour autant.
>
> **Ce qu'il reste à faire est donc entièrement débloqué**, et son montage est
> nommé : un service de plateforme au bon numéro de version, un compte
> (`npm run admin:utilisateur`), un agent enrôlé (`npm run admin:agent`), une VM
> attribuée (`npm run admin:attribuer`), un serveur `vite` pour le client, un
> fichier d'identité **hors dépôt** portant `AGENT_VM`, `AGENT_SECRET`,
> `PREFIXE_VM`, `RECETTE_EMAIL` et `RECETTE_MOTDEPASSE`, puis
> `cargo clean --release -p proto -p agent` et `scripts/build-agent.sh` —
> **jamais** avant d'avoir sourcé `.env`, faute de quoi il s'arrête EN SILENCE
> après « sources synchronisées ».
>
> ⚠️ **Et le pilote n'a jamais tourné** : sa première exécution sera aussi son
> premier débogage.

### Note de lecture des journaux — RELEVÉE, pas supposée

Mesurée par `file`, `grep -lP '\x1b\['` et un balayage `tr -dc '\000'`, **après
la dernière écriture** :

| Famille | Fichiers | Ce qu'il faut faire |
| --- | --- | --- |
| **tous** | les 12 journaux et JSON, plus les 5 fichiers d'`instrument/` | **rien** — UTF-8 partout, **aucune séquence ANSI**, **aucun `\r`**, **aucun octet NUL** : ils se `grep`ent à plat |

**UNE SEULE FAMILLE DE LECTURE**, et c'est structurel : ce sont des sorties
`node`, `cargo` et `npx` sur l'**hôte**, jamais du PowerShell distant. Le défaut
à deux réglages ne peut pas les atteindre. ⚠️ **Les journaux de la recette,
quand elle aura lieu, ne seront PAS de cette famille** : ceux de P1 portaient 33
à 36 octets NUL et exigeaient `grep -a`, faute de quoi `grep` rend une sortie
**vide** — pas un zéro.

---

## 1. 🔴 Le fait qui gouverne : ④ n'est pas mesurable, et la cause n'est pas celle qu'on croirait

**Sonde S1**, hors VM, hors agent, hors session WebRTC. **Deux exécutions, aux
relevés identiques** (`p3-focus-{1,2}.json`, `.log`).

Trois fenêtres ouvertes par `window.open` — **le geste du produit**,
`client/src/shell-page.ts` — rapportent **TOUTES** `document.hasFocus() === true`,
aux trois basculements, aux deux exécutions. **RP3-2 est réalisé**, et le témoin
a échoué sur sa **deuxième issue**, écrite d'avance par le plan.

🔵 **MAIS UNE PREMIÈRE RÉDACTION DE CETTE SONDE AURAIT ATTRIBUÉ L'ÉCHEC AU
`--headless`, ET C'EÛT ÉTÉ FAUX.** La sonde S2, qui ouvre ses pages par
`Target.createTarget`, a relevé **l'inverse dans la même heure** : `bringToFront`
y retire parfaitement le focus. Les deux ne pouvaient pas décrire la même cause.
D'où une **SECONDE ARME** ajoutée à S1, dans la **même exécution** et avec le
**même code de mesure** :

| arme | issue |
| --- | --- |
| `window.open` — **le geste du PRODUIT** | `true` PARTOUT ⟹ **NON MESURABLE** |
| `Target.createTarget` — le geste de l'instrument | **un seul `true`, et c'est la fenêtre amenée au premier plan** |

**L'attribution est au MODE D'OUVERTURE, pas au `--headless`.** Le verdict qui
commande la recette est celui de l'arme du produit.

`Xvfb` et `xdotool` sont relevés **ABSENTS de l'hôte** ce jour-là (consentement
donné en D8, **jamais suivi d'effet**), et ⚠️ **les mesures qui en sortiraient
ne se compareraient à AUCUNE campagne antérieure**.

🔴 **COROLLAIRE INCONFORTABLE, ET IL N'EST PAS TU** : si toutes les fenêtres du
produit rapportent le focus, alors **toutes écrivent leur presse-papier local** —
c'est-à-dire le régime de **N ÉCRIVAINS CONCURRENTS** que D-P3-4 nomme et que
**rien ne mesure**. La recette le rencontrera **par accident**, et le pilote le
relève explicitement (`n_ecrivains_concurrents`) plutôt que de le taire.

⚠️ **Ce que S1 n'établit pas** : rien de Firefox, rien de Safari, rien d'un
Chromium **avec interface**, rien d'un humain. Et le rejeu **en conditions de
produit** (Step 2 de la tâche 10) **peut la contredire** — les fenêtres du
produit portent un flux WebRTC, celles de S1 n'en portaient pas.

---

## 2. 🔵 Le §3.3 est mesuré, et le verdict est plus fin que « vrai » ou « faux »

**Sonde S2**, un **2×2** `{hasFocus} × {userActivation.isActive}`. **Deux
exécutions, relevés identiques** (`p3-writetext-{1,2}.json`, `.log`).

| cellule | état RELEVÉ | `writeText`, message **verbatim** |
| --- | --- | --- |
| focus ✔, activation ✘ | `{hasFocus:true, isActive:false}` | `NotAllowedError: Failed to execute 'writeText' on 'Clipboard': Write permission denied.` |
| focus ✔, activation ✔ | `{hasFocus:true, isActive:true}` | **OK** |
| focus ✘, activation ✘ | `{hasFocus:false, isActive:false}` | `NotAllowedError: Failed to execute 'writeText' on 'Clipboard': Document is not focused.` |
| **focus ✘, activation ✔** | — | 🔴 **INATTEIGNABLE** |

🔴 **LA CELLULE QUI TRANCHE EST INATTEIGNABLE**, et ce n'est pas faute d'avoir
essayé : le geste de confiance **REND le focus** à la fenêtre qui le reçoit, et
`Page.bringToFront` ne le lui reprend plus. **Deux moyens** ont été joués — le
geste ordinaire, puis un geste **qui ne focalise aucun élément** —, et l'attente
porte sur le **FAIT** (vingt relectures de `hasFocus`), jamais sur une durée.

**Le §3.3 reste donc SUPPOSÉ au sens strict**, et le dire est un verdict
recevable ; **en fabriquer un autre ne le serait pas** (RP3-5).

✅ **MAIS IL EST CORROBORÉ PAR UNE PIÈCE, et cela vaut mieux qu'un « non
tranché »** : les deux refus portent le **MÊME NOM** (`NotAllowedError`) et des
**MESSAGES DIFFÉRENTS**. **Un chemin de refus PROPRE AU FOCUS existe, et il se
nomme lui-même.** Ce qui reste non mesuré est **s'il survit à une activation**.

🔴 **POURQUOI UN 2×2 ET NON UNE SIMPLE OBSERVATION** — c'est D-P3-5, et il était
juste. P2 avait **déjà** mesuré un `NotAllowedError` sur `writeText`, et ce
n'était **pas** le focus : son annexe versée relève `hasFocus: true` **des deux
côtés**, et ce qu'elle mesurait était l'**ACTIVATION**. Les deux mécanismes
lèvent la même exception, et **le nom seul ne les distingue pas** — c'est le
message verbatim qui le fait.

🔴 **ET LA PREMIÈRE RÉDACTION DE S2 A RENDU LE VERDICT INVERSE — « le §3.3 est
RÉFUTÉ » — ALORS QUE SON PROPRE RELEVÉ LE RÉFUTAIT.** Elle jouait le geste **en
dernier**, obtenait `{hasFocus:true, isActive:true}` — c'est-à-dire la cellule
**précédente** sous une autre étiquette — et en tirait sa conclusion. **C'est
l'erreur d'attribution que D-P3-5 existe pour empêcher, commise par l'instrument
écrit pour l'empêcher.**

Deux remèdes, tous deux dans le fichier :
1. l'ordre **geste → retrait du focus → écriture** ;
2. 🔴 **le verdict calculé sur l'ÉTAT OBSERVÉ, jamais sur l'étiquette voulue** —
   une cellule dont l'état ne correspond pas à ce qu'elle prétend mesurer est
   **requalifiée INATTEIGNABLE**, et le relevé le dit.

### Ce que le verdict commande, et qui était écrit d'avance (D-P3-4)

C'est la branche « `writeText` échoue sans focus » qui est corroborée. **La
règle du dépôt différé RESTE**, sa justification d'origine est confirmée sans
être établie, et **la seconde justification vaut indépendamment** :

> **À N fenêtres, le test de focus n'est pas seulement une parade à un refus :
> c'est l'ARBITRAGE qui élit l'unique écrivain local.** Le capteur pousse le
> contenu à **toutes** les fenêtres (D3), chacune a son propre
> `PressePapierLocal`, et si toutes écrivaient, **N appels concurrents à
> `writeText` partiraient pour une seule copie**, le dernier gagnant
> arbitrairement.

⚠️ **« Du coût pour rien » (spec §6.3) suppose UNE fenêtre.** Retirer la règle
signifierait « toutes les fenêtres écrivent », **régime que rien ne mesure** :
son retrait est donc une **décision du propriétaire du dépôt**, jamais une
conséquence mécanique d'un verdict de sonde.

---

## 3. 🔴 Une course trouvée par LECTURE, puis MESURÉE, puis fermée

D-P3-6. Le plan la rapportait comme une **lecture, jamais une mesure**, et
armait explicitement le cas où elle serait fausse (RP3-9 : « s'arrêter, l'écrire,
et ne PAS écrire le correctif »).

**Elle est vraie.** Le test a été **vu ROUGE sur l'arbre intact, sans aucune
mutation** — la forme la plus forte de rouge de ce dépôt : elle ne mute rien,
donc elle ne peut ni rougir pour la mauvaise raison, ni être satisfaite par un
commentaire du fichier qu'elle analyse. Pièce :
`journaux-presse-papier-p3/rouge-t5-d-p3-6-arbre-intact.log`.

```
assertion `left == right` failed
  left: Some(Texte("textB"))
 right: None
```

**L'entrelacement**, à deux fenêtres : A colle → le tour de roue **prend** le
couple et arme `reference = seqA`, `dernier_emis = textA` → **B colle** → `tour()`
lit `seqB ≠ seqA` (le garde n°1 ne mord pas) puis `textB ≠ textA` (le n°2 non
plus) → **`Annonce::Texte(textB)` part vers les N fenêtres** → au tour suivant,
`armer_les_gardes` prend `(seqB, textB)` : **trop tard**.

**Le remède, en deux étages**, comme la doctrine du dépôt le veut : la **RÈGLE**
pure et injectée (`Sondeur::ecarter` / `ecarter_notre_ecriture`) et sa
**BRANCHE** sur le registre (`filtrer_nos_ecritures_tardives`), appelée depuis
`registre.rs` **entre `tour()` et `distribuer`**.

⚠️ **LE RÉSIDU EST ÉCRIT DANS LE CODE.** `ecrire_avec` écrit le presse-papier
**PUIS** pose `notre_ecriture` — le verrou y est délibérément pris **après**
l'E/S Win32, parce que le tenir autour d'`OpenClipboard` bloquerait l'attache et
le retrait de **toutes** les fenêtres. Si `tour()` lit dans cet intervalle, la
seconde prise ne trouve rien. **Le remède RÉTRÉCIT la fenêtre, il ne la ferme
pas** — du même genre que le résidu qu'`apres_notre_ecriture` déclare déjà
accepté.

⚠️ **Ce n'est PAS un défaut créé par P3** : à une fenêtre, deux collages en
moins de `PERIODE_PRESSE_PAPIER` (250 ms) le produisent aussi. P2 ne l'a pas
rencontré, ses quatre collages étant espacés de plusieurs secondes.

🔵 **`PRESSE_PAPIER_GARDE=0` désarme AUSSI cette prise**, et il le faut : ce
bras de banc existe pour rendre atteignable la rouge du critère ④ de P2, qui
compte les messages revenant vers la fenêtre après un collage. Une seconde prise
qui mordrait quand même le viderait de son sens. Un test le tient.

### 🔴 Un défaut du plan, signalé et corrigé plutôt que recopié

Son Step 1 prescrit un test de **deux lignes** — `armer(true, seqA, textA)` puis
`observer(seqB, || Some(textB))` — « vu ROUGE sur l'arbre intact ». **Il l'a
été.**

⚠️ **Mais ces deux lignes seules ne peuvent JAMAIS devenir vertes, et le plan ne
l'avait pas vu** : le remède qu'il tranche lui-même est un **POST-FILTRE**, qui
court **après** `tour()` sur son résultat. `observer` ne peut pas connaître une
écriture arrivée après lui ; exiger qu'il rende `None` serait exiger qu'il
devine.

**La lettre du test est conservée** — ses deux premières lignes sont celles qui
ont rougi — **et une ligne lui est ajoutée** : la seconde prise, appliquée au
résultat.

---

## 4. 🔴 Trois harnais pris en défaut — le résultat de méthode du sous-bloc

**a) `git checkout --` restaure à HEAD, pas à l'état d'avant la mutation.**
La première rouge a donc **EFFACÉ le correctif non commité** que les rouges
suivantes devaient éprouver ; celles-ci se sont arrêtées sur « ancre
introuvable », c'est-à-dire sur **le seul symptôme visible d'un travail perdu**.
Le contrôle de `sha256` a bien crié « DIVERGENT » — **mais après la perte**.
**Une rouge se restaure depuis une COPIE NOMMÉE.**

**b) L'étape « la mutation a-t-elle changé quelque chose ? » était VACUEUSE.**
Elle exigeait un `git diff --numstat` **non vide** — mais il compare à **HEAD**,
donc il reste non vide tant qu'un correctif non commité vit dans le fichier,
**quelle que soit la mutation, et même s'il n'y en a aucune**. Le contrôle censé
refuser une rouge qui ne mute rien **ne pouvait pas échouer**. Il compare
désormais à la copie nommée, et une **ROUGE 0** — une mutation qui ne mute
rien — est jouée **pour le voir refuser** : il refuse.

> C'est « un contrôle qu'on n'a jamais vu rouge n'est pas un contrôle »,
> appliqué **au contrôle lui-même**.

**c) 🔴 Une rouge est restée VERTE, et elle a été DIAGNOSTIQUÉE plutôt que
classée.** Elle remplaçait `take()` par `clone()` dans
`filtrer_nos_ecritures_tardives` : le diff n'était pas vide, et le test restait
vert. Cause, trouvée en **rejouant la mutation à la main** (`sed`, qui remplace
toutes les occurrences) : **cette ligne existe DEUX FOIS dans le fichier** —
`armer_les_gardes` la porte aussi, et elle vient **en premier** —, et le
`replace(…, 1)` a frappé la mauvaise fonction. Rejouée **ancrée sur la
SIGNATURE**, elle rougit.

C'est le §2.2 règle 3 du plan payé **sur une ligne qui n'avait rien d'un
commentaire** : la seule **duplication** suffit.

🔴 **LA LEÇON EST NEUVE : UNE ROUGE QUI RESTE VERTE SE DIAGNOSTIQUE, ELLE NE SE
CLASSE PAS.** Sans le rejeu manuel, ce relevé se serait lu comme « le test ne
peut pas échouer » — **l'inverse exact de la vérité**.

**d) Un piège mineur, relevé pendant la revue transverse** : `grep -c` avec des
alternatives `\|` compte des **lignes**, pas des motifs. Il a rendu « 3 » pour
**six** éditions bel et bien présentes. Les places ont donc été relues **une par
une, chacune avec son propre motif**.

---

## 5. Les douze rouges, et ce qu'elles tiennent

Chacune nomme **l'assertion qui a rougi et ce qu'elle a rendu**, jamais le seul
code de sortie. Journaux : `rouge-t5-…`, `rouges-t5.log`, `rouges-t6.log`,
`rouges-t7.log`.

| # | Ce qu'elle mute | Ce qui tombe |
| --- | --- | --- |
| — | **rien : l'arbre intact** | la course de D-P3-6 elle-même |
| 0 | **rien** (contrôle du harnais) | le harnais REFUSE, comme il doit |
| A | le filtre porte sur le `seq` et non le texte | une copie **TIERCE** cesserait d'être annoncée |
| B | le filtre s'applique aussi à `Annonce::Refus` | l'utilisateur perdrait le bandeau |
| C | le garde `if !armes` est retiré | `PRESSE_PAPIER_GARDE=0` cesserait de désarmer |
| D | la branche **lit** le couple sans le **consommer** | il serait rejoué au tour suivant |
| E | l'émission à l'inscription n'existe pas | **c'est l'arbre d'avant, donc le legs n°3 lui-même** |
| F | l'émission fait un **fan-out** | un aller-retour par attache |
| G | on ne mémorise que les `Texte` | un refus ne serait jamais rejoué |
| H | on émet un message **VIDE** quand la mémoire est `None` | le client écrirait une chaîne vide à chaque attache |
| I | on **purge** la mémoire à la ré-inscription | 🔵 **D-P3-3** — le rattachement ne rejouerait rien |
| J | le paramètre `initial` n'existe pas | **3 tests** tombent : c'est l'arbre d'avant P3 |
| K | le montage écrit **directement**, hors du dépôt différé | **2** tombent, dont le refus sans bandeau |
| L | un `Recu` vide est rejoué quand `initial` est absent | le comportement d'avant P3 ne serait plus préservé |

🔵 **LA ROUGE I EST CELLE QUI ENSEIGNE LE PLUS** : elle établit que D-P3-3
n'était pas un scrupule théorique. Copier la purge de `dernieres_parts` **par
symétrie de forme** fait tomber le rattachement — **le cas où le rejeu est le
plus utile**. La symétrie est trompeuse parce que ces deux-là se purgent pour
une raison que le presse-papier n'a pas : leur distribution **FILTRE** sur eux,
la sienne est **inconditionnelle**.

---

## 6. Ce que le code livre

| Étage | Fichier | Nature |
| --- | --- | --- |
| la règle de la seconde prise | `agent/src/presse_papier/sondeur.rs` | **pure, aucun `cfg`**, état du garde **injecté** — c'est ce qui rend le bras désarmé éprouvable sur l'hôte |
| sa branche | `agent/src/capteur/sommeil/presse_papier.rs` | verrou pris et rendu, **ne couvre aucune E/S** |
| la mémoire de l'état courant | `agent/src/capteur/sommeil/registre.rs` | `Etat::dernier_presse_papier`, **aucune purge** (D-P3-3) |
| son émission | `emettre_l_etat_courant` | **le seul canal neuf**, jamais un fan-out |
| l'attribution | `agent/src/capteur/fenetre.rs` | **une ligne** : `pid` dans `fenêtre attachée au capteur` |
| la règle du rejeu au montage | `client/src/presse-papier-dom.ts` | paramètre **facultatif** `initial?: Recu`, rejoué **par le chemin qui existe déjà** |
| le câblage | `client/src/main.ts` | **deux lignes**, patron `micAnnonce` — 🔴 **non testées, et déclarées telles** |

**Deux extractions préalables**, jouées **AVANT** les additions qui les rendent
nécessaires : `presse_papier.rs` **441 → 278** et
`capteur/sommeil/presse_papier.rs` **379 → 241**. Transpositions **vérifiées
caractère pour caractère**, et la désindentation de quatre espaces vérifiée
**réversible**.

🔴 **UN PLAFOND A ÉTÉ FRANCHI QUAND MÊME**, et c'est mon erreur :
`agent/src/capteur/sommeil/tests.rs` est monté de **417 à 562**. E13 du plan
l'annonçait au-delà de 480, et **la mesure n'a pas été prise d'avance**.
**L'extraction est jouée, jamais une compression**, et le fichier retombe à
**417** — sa taille exacte d'avant. Le point de chute n'est pas de commodité :
ces tests exercent `emettre_l_etat_courant`, ils vont donc auprès d'elle.

Comptes, **annoncés avant d'être lus** : `cargo test -p agent` **904 → 916**,
`cargo test -p proto` **109** (inchangé), `client` **462 → 466** / 40 fichiers,
`proto/ts` **296** (inchangé). ⚠️ **`cd client && npx vitest run` NE COUVRE PAS
`proto/ts/`.** `./scripts/verify-all.sh` depuis un shell **propre** :
**sortie 0**, **dix** étapes du script, **dix-huit** en-têtes `==>` à l'écran.

✅ **`proto/` n'a pas bougé d'une ligne** (D-P3-13, vérifié par
`git diff --stat … -- proto/`, sortie **VIDE**), et **aucune variable
d'environnement n'est introduite** — `scripts/run-agent.sh` n'est pas modifié.

---

## 7. La revue transverse — huit affirmations, neuf places

Barème du dépôt : 5 en D7, 3 en D8, 6 en D9, douze en D10, sept en D11, huit en
P1 de la plateforme, dix en P2, cinq en S1, neuf au chantier E, douze en P3 de
la plateforme, douze en S2, onze en F1, huit en P4, treize en S3, huit en G1.

Les places ont été **énumérées par `grep -rniE` avant toute édition, et relues
place par place après**. Le détail vit dans le message du commit `9ab9b8f` ; les
trois qui enseignent :

- `agent/src/presse_papier/sondeur.rs` et `…/tests.rs` — « une fenêtre qui
  s'attache ne reçoit donc pas le contenu déjà présent », **réfutée par la
  tâche 6**, à **deux** places. ⚠️ Ce qui reste vrai est la propriété **du
  champ** : le `Sondeur` n'annonce toujours rien à son premier tour ; c'est le
  **registre** qui rejoue ;
- `apres_notre_ecriture` — sa réserve était **incomplète** : elle ne traite que
  la copie **tierce**, qu'elle déclare voulue, et le cas de **notre propre
  seconde écriture** n'était déclaré nulle part ;
- le plan de P2 §8 — « c'est le legs n°3 de P1, et il appartient à P3 » : **fait**,
  et il n'en nommait **qu'une moitié**.

Les documents datés (plans de P1 et P2, résultats de P2) sont **annotés, jamais
réécrits** : un relevé daté reste vrai comme histoire, et ce sont les
**pronostics** qu'on reprend.

⚠️ **La revue transverse est elle-même une source de croissance, et elle l'a
été** : `agent/src/presse_papier/tests.rs` passe à **477**, marge **23**.
Déclaré ; aucune porte n'est franchie, et rien n'est comprimé.

---

## 8. Ce que P3 n'établit PAS

- 🔴 **RIEN DU PRODUIT EN MARCHE À N FENÊTRES.** Les quatre critères ①②③④ n'ont
  pas été joués.
- 🔴 **Les deux lignes de câblage de `client/src/main.ts` ne sont couvertes par
  AUCUN test**, et leur seul contrôle de bout en bout est le critère ① dans sa
  forme « une fenêtre attachée APRÈS la copie ». **Il n'a pas été joué**, donc
  elles ne sont éprouvées par rien — **RP3-17, réalisé**.
- 🔴 **Le site d'appel de `registre.rs`** n'est couvert par aucun test d'hôte :
  il vit dans le fil du tour de roue.
- 🔴 **④ n'est pas mesurable** en conditions de produit, et la cause est le mode
  d'ouverture du produit lui-même.
- **Le §3.3 reste SUPPOSÉ au sens strict** — corroboré, non établi.
- **Le régime de N écrivains concurrents n'est mesuré par rien.**
- **La borne de 12 s de `commander` n'a toujours pas couru** (legs n°5 de P2) :
  P3 est le premier à pouvoir la mettre sous contention réelle, **et il ne l'a
  pas fait**.
- **Le pilote multi-fenêtres N'A JAMAIS ÉTÉ EXÉCUTÉ**, et le fichier le dit de
  lui-même. Ce qui est vérifié : il parse, ses imports résolvent, et un import
  réel s'arrête sur son garde d'identité — la première chose qui doit l'arrêter.
- **Aucun taux, nulle part.** Deux exécutions par sonde, une par rouge. **Deux
  exécutions établissent la reproductibilité d'un mécanisme déterministe, jamais
  une fréquence.**
- **Rien d'un navigateur autre que Chromium**, rien avec interface, **rien d'un
  humain**, rien du HiDPI, rien de la latence.
- **Le niveau 2 du sens VM → navigateur reste NON MESURABLE** (legs n°11 de P1) :
  `xclip` et `wl-paste` sont **absents** de l'hôte, `xsel` refuse en « Can't
  open display ».
- **`PRESSE_PAPIER_MAX` et `PERIODE_PRESSE_PAPIER` ne sont toujours pas
  calibrées**, et elles rejoignent la liste que ce dépôt tient depuis `BPP_MIN`.
- **Aucun texte non-ASCII ni multi-ligne n'a traversé la chaîne réelle** — legs
  n°10 de P1, reconduit.

---

## 9. Ce que P3 lègue

**Legs de P1 réglé** : n°3 (l'état courant à l'attache — **les DEUX moitiés**),
🔴 **fermé SUR PIÈCES et jamais mesuré de bout en bout**.

**Ce qui reste dû :**

1. 🔴 **LA RECETTE ENTIÈRE.** Les quatre critères, le témoin de focus en
   conditions de produit, l'attribution par `pid`, et le nombre de fenêtres que
   la VM rend ce jour-là (**il se RELÈVE, il ne s'exige pas** — D-P3-8, et à une
   seule fenêtre P3 n'est pas livrable, RP3-1). L'instrument est versé, prêt, et
   **n'a jamais tourné**.
2. 🔴 **Le régime de N ÉCRIVAINS CONCURRENTS n'est mesuré par rien**, et le
   relevé de S1 dit que la recette le rencontrera **par accident**.
3. 🔴 **Le résidu de D-P3-6** : `ecrire_avec` pose `notre_ecriture` **après**
   l'E/S Win32, et la seconde prise ne ferme pas cet intervalle. Le fermer
   demanderait de tenir le verrou autour de l'E/S — ce que
   `capteur/sommeil/presse_papier.rs` **interdit nommément**, et pour une bonne
   raison. **Écrit, non fermé.**
4. ⛔ **Le retrait de la règle du dépôt différé**, si le §3.3 devait être réfuté
   un jour : **décision du propriétaire du dépôt** (D-P3-4), avec son coût
   nommé — un régime que rien ne mesure.
5. ⛔ **Le canal `Message` reste NON BORNÉ**, et P3 l'aggrave **d'un message par
   attache**. Le borner est un **changement de conception** : bloquer serait le
   pire, le seul écrivain étant le tour de roue, **sous le verrou global**
   (D-P3-11).
6. ⛔ **Le propriétaire MONO-FENÊTRE n'existe toujours pas** (legs n°1 de P1).
   Point de chute : `agent/src/demarrage.rs`, **426 lignes, marge 74**.
7. ⛔ **La borne de 12 s de `commander` n'a toujours pas couru** (legs n°5 de P2).
8. ⛔ **La rouge du critère ③ de P2 n'est toujours pas jouée** — elle exige un
   second binaire, et **P3 ne la joue pas non plus** (legs n°1 de P2). La
   justification de D6 reste donc **non éprouvée**.
9. ⛔ **Le critère ⑤ de P2 reste NON MESURÉ**, et **le niveau 2 du sens
   VM → navigateur avec lui** : `Xvfb` + `xdotool`, consentement donné en D8,
   jamais suivi d'effet.
10. ⚠️ **`agent/src/presse_papier/tests.rs` est à 477, marge 23** : toute
    addition y appelle une **EXTRACTION**, jamais une compression.

**Et pour A1 — la couleur d'accent — P3 ne lègue RIEN**, parce qu'il n'y a
touché à rien : A1 est hors périmètre entier (spec §5, §6.4), il n'a pas de
plan, et **aucun fichier de P3 ne l'approche**. ⚠️ Ce qui le concerne
indirectement est le §2 ci-dessus : `Capabilities` et le canal de contrôle sont
les chemins qu'A1 empruntera, et **P3 n'y a ajouté aucune variante de
protocole** — `CONTROL_VERSION` ne bouge pas, `proto/` n'a pas bougé d'une
ligne.
