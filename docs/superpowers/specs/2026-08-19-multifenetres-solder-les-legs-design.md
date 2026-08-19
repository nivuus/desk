# Sous-bloc D11 — solder les legs : rendre le son au cas majoritaire, et prouver ce qui ne l'a jamais été

**Date** : 19 août 2026
**Prédécesseur** : sous-bloc D10 (`2026-08-07-multifenetres-solder-la-branche-design.md`),
fusionné en `df03fc6`, qui laisse **huit legs** — ou sept, selon la source
qu'on lit (§1.1).
**Objet** : en traiter six sur huit, plus **deux legs de D9 tombés du
registre** (§1.2) — et en écarter deux avec leur raison écrite.

---

## 1. Objet, et l'état des lieux vérifié dans le code

D10 a réglé **huit** des **douze** legs de D9 et en a ouvert **cinq**. Ce qui
en reste tient en un tableau — **et chaque ligne « état vérifié » ci-dessous a
été relue dans le code aujourd'hui, jamais recopiée d'un document** :

| # | Leg | État vérifié, avec sa pièce |
| --- | --- | --- |
| 1 | L'A/B sur `set_desired_bitrate` | joué deux fois (D9, D10), **n'établit rien** : 4 paires, signes `− − + +` |
| 2 | Le maillon fautif du `Resize` | **instrumenté** (`client/src/main.ts:346-374`, grille à trois issues), **jamais relu sur la VM** |
| 3 | Les six constats parqués de D9 | **perdus** — l'espace de travail était gitignoré |
| 4 | 🔴 La reconstruction audio est **inerte en mono-fenêtre** | `transport/piste_audio.rs:318` applique `set_actif(self.audio_porteuse)` ; `transport.rs:386` naît `false` ; le seul écrivain est `appliquer_audio` (`piste_audio.rs:138, 183`) ; son seul appelant est `tick.rs:259`, gardé par `source.audio_a_appliquer()`, **que seul `capteur/distante.rs:372` surcharge** — `source.rs:137-139` rend `None` par défaut. **Un agent mono-fenêtre n'a pas de `SourceDistante` : `audio_porteuse` y reste `false` à jamais.** |
| 5 | `AUDIO_FAUTE_RECONSTRUCTION` | n'existe pas — `grep -rn "AUDIO_FAUTE" agent/src` ne rend que `AUDIO_FAUTE_LECTURE` (`windows_audio/fil.rs:109, 216`) |
| 6 | La cause du refus de reconstruction | **la cause EXISTE et est jetée à l'écriture** : `windows_audio.rs:180` pose un `with_context`, et `piste_audio.rs:325` journalise `%erreur` — le `Display` simple d'`anyhow`, qui ne rend que le contexte le plus externe |
| 7 | Le coût de la duplication d'une sortie surdimensionnée | jamais mesuré |
| 8 | La séparation des flux entre fenêtres | jamais prouvée — le contrôle de D10 **ne peut pas échouer** (§5.2) |

### 1.1 ⚠️ Les deux sources ne se numérotent pas pareil, et l'écart porte sur le leg le plus lourd

`CLAUDE.md` (§ « Ce que D10 lègue ») compte **huit** legs. Le §11 du document
de résultats de D10 en compte **sept**. L'écart n'est pas une coquille de
numérotation : **le §11 omet purement et simplement le leg n°4** — celui que
`CLAUDE.md` marque 🔴, et le seul des huit qui ait une **conséquence de
comportement** en production.

Vérifié : `grep -n "mono-fenêtre"` sur le document de résultats rend **quatre**
occurrences, toutes au §8bis (la revue transverse), **aucune au §11**. Les six
legs suivants sont donc décalés d'un rang d'une source à l'autre — le
`AUDIO_FAUTE_RECONSTRUCTION` est n°5 dans `CLAUDE.md` et n°4 dans le document
de résultats.

**Cette conception adopte la numérotation de `CLAUDE.md`**, qui est l'index que
l'on lit, et **le premier geste de D11 est de réparer le §11 du document de
résultats de D10** pour qu'il porte les huit. C'est exactement le patron que ce
dépôt a payé sept fois sous le nom de « naufrage du 487 » : une affirmation
corrigée à un endroit et pas à l'autre, l'endroit non corrigé étant celui que
le successeur lit.

*(La correction est un geste de document, pas de code : elle ne change aucun
comportement et ne demande aucune mesure.)*

### 1.2 ⚠️ Et DEUX legs de D9 sont tombés du registre sans que personne le dise

Le compte ne tombe pas juste, et c'est la seconde divergence. D9 laissait
**douze** legs numérotés. D10 en déclare huit réglés (1, 2, 4, 5, 6, 8, 9, 10)
et trois encore dus (3, 7, 10-partiel). **Il en manque deux : les n°11 et 12 de
D9**, ajoutés par la vague de correction de sa revue finale. Ni le §11 du
document de résultats de D10, ni la liste de `CLAUDE.md` ne les mentionnent —
`grep -n "leg 11\|leg 12\|test faible\|invariant non écrit"` sur le document
de résultats de D10 rend **zéro**.

**Les deux tiennent toujours, vérifiés dans le code aujourd'hui** :

- **D9 n°11 — deux tests qui ne peuvent pas rendre l'autre valeur.**
  `windows_source/telemetrie.rs:69`, `une_telemetrie_neuve_est_a_zero`,
  n'éprouve que `#[derive(Default)]` (`Telemetrie::default().lire() ==
  (0, 0, 0)`), jamais `tick`/`capturee`/`produite`. Et
  `client/src/resize.test.ts:18` annonce « REJOUE la dernière taille **quand le
  canal était fermé** » alors que `RejeuResize` n'a **aucune notion de canal ni
  de `readyState`** — vérifié : `grep -n "readyState\|channel\|canal"
  client/src/resize.ts` rend **une seule** ligne, et c'est un commentaire de
  documentation. Le test se contente d'omettre `confirmer()`, ce qui rendrait le
  même verdict pour n'importe quelle autre raison de non-confirmation.
- **D9 n°12 — un invariant porteur, ni écrit ni testé.** Le rejeu du `Resize`
  ne tient que parce que le `.then()` qui pose
  `session.controlChannel.addEventListener('open', emettreSiPossible)` s'exécute
  **intégralement de façon synchrone**, sans `await` intercalé. Un `await`
  glissé là romprait le rejeu **en silence**. La ligne existe toujours —
  `client/src/main.ts:386` — ⚠️ **et son numéro a bougé** : D9 la citait à
  `:345`, le fichier ayant grossi de 352 à 392 lignes depuis. *Un numéro de
  ligne recopié survit à la réalité qu'il décrivait : ce dépôt l'écrit depuis
  D6, et c'est vrai des legs comme des comptes.*

**D11 les reprend** (§7.2). Ils sont froids et bon marché, et les laisser
tomber une seconde fois les perdrait comme les six constats du leg 3 — à ceci
près que ceux-là sont encore récupérables **parce que leur preuve est dans le
code, pas dans un rapport gitignoré**. C'est très exactement l'argument du §9.4.

---

## 2. Arbitrage — ce que D11 prend, et ce qu'il écarte

**Six legs de D10 retenus, deux écartés — plus les deux de D9 que le registre
avait perdus.** Un sous-bloc de ce dépôt fait dix à quinze tâches ; prendre les
huit sans distinction reviendrait à rejouer deux fois une mesure qui a déjà
échoué à trancher.

### 2.1 Retenus

| Leg | Famille | Pourquoi maintenant |
| --- | --- | --- |
| **4** 🔴 | ① audio | seule conséquence de comportement encore ouverte ; le correctif est nommé, borné, et déjà gardé rouge par un test existant |
| **6** | ① audio | **une ligne**, et elle rend décidable le leg 5 de D10 qui est aujourd'hui indécidable faute de journal |
| **5** | ② repli | seule voie nommée pour exercer le critère ④ que D10 laisse non démontré — et sa formulation de D10 est **incomplète** (§4.2) |
| **8** | ③ preuve | une propriété de **correction** du produit multi-fenêtres, jamais prouvée depuis D1, et dont le contrôle actuel est vacueux |
| **7** | ④ coût | le prix assumé de la voie « tolérer et recadrer » de D10 ; s'il est lourd, la voie se rediscute |
| **2** | ⑤ client | l'instrumentation est **déjà en place** : il ne manque qu'une lecture. Le leg est ouvert depuis D8, soit trois sous-blocs |
| **D9 n°11 et n°12** | ⑥ froid | tombés du registre (§1.2), **récupérables parce que leur preuve est dans le code** ; le n°12 vit dans le fichier même que la famille ⑤ relit |

**Et un point qui n'est numéroté nulle part mais que D10 marque 🔴 dans son
§9** : *« LES DEUX FAMILLES N'ONT JAMAIS TOURNÉ ENSEMBLE »* — la recette ① de
D10 a tourné à **dix** fenêtres sans aucune faute audio, la recette ② à **une
seule**. C'est, mot pour mot, « la lacune de couverture la plus lourde de
D10 ». D11 le traite comme un leg à part entière (§5.1) : il ne coûte presque
rien une fois la famille ① faite, et il apporte la **seule discrimination** que
D10 n'a pas pu obtenir (§5.1.1).

### 2.2 Écartés, avec la raison

**Leg 1 — l'A/B sur `set_desired_bitrate`. ÉCARTÉ.**

Trois raisons, et la troisième est décisive :

1. **Il a été joué deux fois et n'a rien tranché.** D9 : écart entre bras
   +23,2 % contre **+83,1 %** de variance intra-bras. D10 : appariement dos à
   dos à huit fenêtres, **2 paires positives contre 2 négatives**, moyenne des
   différences **−0,100 Mb/s** — dans le sens *inverse* de l'effet attendu —
   pour un écart-type de **0,367 Mb/s**, soit **3,7 fois** la moyenne.
2. **Le montage ne fait pas ce qu'on lui demande.** D10 relève lui-même que
   l'appariement n'est « dos à dos » que **marginalement** : ~150,8 s
   intra-paire contre ~158,6 s inter-paires, **~5 %**. L'annulation de la
   dérive de charge de l'hôte, qui est la raison d'être de l'appariement,
   **reste une hypothèse**. Ajouter des paires à un montage dont la propriété
   centrale n'est pas acquise n'est pas un plan, c'est un pari.
3. **La prémisse qui en faisait « le point ouvert le plus important » a été
   réfutée par D6 lui-même.** Le sondage cumulé ne saturait rien : le pont
   porte ≥ 1,44 Gb/s et `packetsLost` vaut **0** — pas « négligeable », zéro —
   aux huit exécutions de D10 comme aux sept de la recette D6. **La question
   que cet A/B pose n'a plus l'importance qu'elle avait quand elle a été
   posée.**

⚠️ **Ce que cet écartement n'affirme PAS** : il ne dit pas que l'appel est sans
effet. D10 l'écrit déjà — « n'établit rien » n'est pas « l'appel n'a pas
d'effet ». **Condition de réouverture, écrite ici pour ne pas se rejouer à
l'aveugle** : un montage dont la charge d'hôte est *contrôlée* (et non
seulement appariée), sur au moins huit paires. C'est un chantier de banc à part
entière, pas une tâche de solde.

**Leg 3 — les six constats parqués de D9. ÉCARTÉ, irrécupérable.**

La tâche 18 de D10 a établi **par la commande** que l'espace de travail de D9
(`.superpowers/sdd/`) est gitignoré, n'a jamais été commité, et n'existe ni sur
le disque ni dans git. Trois des neuf constats survivent parce qu'ils avaient
été extraits vers un document permanent avant la disparition ; **les six autres
n'existent plus.** **Inventer une liste serait pire que d'admettre qu'elle est
perdue** — et c'est exactement ce que D10 écrit.

**Rien à faire, et c'est le verdict.** La contre-mesure, elle, est déjà
appliquée depuis D10 : un document de résultats permanent porte l'analyse, les
pièces brutes sont versées sous `journaux-*`, et aucune affirmation du dépôt ne
dépend plus d'un rapport de tâche. **D11 tient cette discipline** (§9.4).

---

## 3. Famille ① — rendre le son au cas majoritaire (legs 4 et 6)

### 3.1 Le fait à réparer, et la chaîne complète

**En mono-fenêtre, le remède de reconstruction que D10 a livré éteint le son
qu'il vient de rallumer.** La chaîne, entièrement relue dans le code :

1. `demarrage/audio.rs:40` — sans `FENETRE_HWND`, la source est
   `WindowsAudioSource::new`, qui **s'auto-émet** (`windows_audio.rs:168-169`,
   « Aucun capteur n'enverra jamais d'ordre à cet agent : il émet d'emblée »).
2. `demarrage/audio.rs:56-67` — le reconstructeur est posé dans les **deux**
   modes ; sa branche `None` (l. 64) rappelle `new()`, qui s'auto-émet aussi.
3. `transport/piste_audio.rs:318` — mais `reconstruire_ou_signaler` applique
   **ensuite** `source.set_actif(self.audio_porteuse)`, **sans condition**.
4. `transport.rs:386` — `audio_porteuse` naît `false`.
5. `piste_audio.rs:138, 183` — son **unique** écrivain est `appliquer_audio`.
6. `tick.rs:259` — l'unique appelant d'`appliquer_audio` est gardé par
   `self.source.audio_a_appliquer()`.
7. `source.rs:137-139` — cette méthode de trait rend `None` par défaut, et
   **`capteur/distante.rs:372` est sa seule surcharge du dépôt**
   (`grep -n "fn audio_a_appliquer" src/**/*.rs` : deux occurrences, la
   définition et cette surcharge).

**Un agent mono-fenêtre n'a pas de `SourceDistante`.** `audio_porteuse` y vaut
donc `false` pour toujours, et la ligne 318 remet au silence chaque capture
reconstruite. **Ce n'est pas une régression** — avant D10 rien n'était
reconstruit, et le son mourait de la même façon — mais le remède ne sauve pas
le cas qu'il vise.

### 3.2 Le remède, et pourquoi pas l'autre

`CLAUDE.md` nomme le correctif : *« `demarrage/audio.rs::brancher` connaît déjà
`config.fenetre_hwnd`, et poser `audio_porteuse = true` dans la seule branche
`None` »*. **Vérifié, et exact quant au fond** — avec une précision que la
formule ne porte pas : `audio_porteuse` est un champ **privé** de `Session`
(`transport.rs:293`, sans `pub`), et `demarrage/audio.rs` vit hors du module
`transport`. Le correctif passe donc par un **accesseur public** sur `Session`,
posé auprès de ses jumeaux `set_audio_source` (`piste_audio.rs:26`) et
`set_audio_reconstructeur` (`piste_audio.rs:227`), tous deux `pub fn`.

L'appel se fait **dans la seule branche `None`**, c'est-à-dire au seul endroit
qui sait qu'il n'y aura jamais de capteur pour l'écrire.

⚠️ **Ne pas « corriger » en forçant `true` dans `reconstruire_ou_signaler`.**
Le code le dit déjà (`piste_audio.rs:265-269`) et un test l'y garde rouge :
`une_session_non_porteuse_reconstruite_reste_muette`
(`transport/tick/tests/audio.rs:383-403`) prouve qu'une session non porteuse
reconstruite doit **rester muette**. Forcer `true` sans arbitrage ferait fuir
le son vers une fenêtre qui doit se taire — défaut **pire** que celui qu'on
répare. Le correctif retenu laisse ce test vert **sans y toucher** : il n'écrit
`audio_porteuse` qu'au branchement mono-fenêtre, jamais dans le chemin de
reconstruction.

### 3.3 Le leg 6 — une ligne, et un journal qui cesse de mentir par omission

`piste_audio.rs:325` journalise le refus de reconstruction avec `%erreur`.
`anyhow` ne rend alors que le contexte **le plus externe** — et
`windows_audio.rs:180` en pose justement un
(`ouverture du process loopback du PID {pid}`). Le HRESULT, la seule donnée qui
répondrait au leg, reste dans les causes.

**C'est visible dans les pièces de D10** :

```
WARN agent::transport::piste_audio: reconstruction de la capture audio refusée
  erreur=ouverture du process loopback du PID 27544 restantes=2
```
*(`journaux-multifenetres-d10/agent-critere-2-1-plat.log:97`)*

Aucune cause. Le leg « la cause du refus n'est pas identifiée » n'est donc pas
une inconnue du système : **c'est un choix de format**.

**Le remède est `{erreur:#}`**, et **la doctrine est déjà écrite dans ce dépôt**
— `diagnostics/multifenetre/plafond/sonde.rs:110-119` l'explique mot pour mot,
au sujet d'un autre HRESULT perdu de la même façon, et note que le précédent
n'avait survécu aux journaux de D3 que par une **dépendance fortuite** à la
trace d'un autre module. **Six** sites du dépôt emploient déjà ce format —
`capteur/serveur.rs:304`, `capteur/fenetre/commandes.rs:211` et `:232`,
`capture/types.rs:38`, `diagnostics/multifenetre/paralleles/passes.rs:74` et
`diagnostics/multifenetre/plafond/sonde.rs:120` (compte relevé par
`grep -rn '{erreur:#}\|{e:#}' agent/src`, hors commentaires).

⚠️ **Ce que le leg 6 devient après ce changement** : il **cesse d'être un leg
de code** et devient une **observation à faire** — la cause s'affichera à la
prochaine occurrence. D11 ne promet pas de l'expliquer, seulement de la rendre
lisible. Si la recette ② en produit une, la cause est versée ; sinon, le leg
reste ouvert **et devient enfin observable**, ce qu'il n'était pas.

---

## 4. Famille ② — exercer le repli, et la correction que D10 n'a pas vue (leg 5)

### 4.1 Ce que D10 a laissé

Critère ④ de D10 — « une capture irrécupérable retombe sur la promotion » —
**non démontrable par le protocole prescrit** : tuer l'arbre de processus cible
tue **indissociablement la fenêtre**, `pid_de_fenetre(hwnd)` liant toujours la
cible audio au PID propriétaire du HWND. La mort côté vidéo est détectée en
≈ 1,3 s quand le budget de reconstruction met ≥ 6 s à s'épuiser
(`RECONSTRUCTIONS_MAX = 3` × `REPIT_RECONSTRUCTION = 2 s`, `audio.rs:83, 93`).

La voie nommée par D10 : *« un `AUDIO_FAUTE_RECONSTRUCTION` calqué sur
`AUDIO_FAUTE_LECTURE` »*.

### 4.2 ⚠️ Cette formulation est INCOMPLÈTE, et l'arithmétique le montre

**Un `AUDIO_FAUTE_RECONSTRUCTION` seul ne suffit pas à exercer la promotion**,
et la raison se calcule à partir de constantes relevées dans le code :

- `superviseur/lanceur.rs:229-281` — un enfant **hérite l'environnement** du
  superviseur ; seules `SUPERVISEUR`, `TEST_FILE`, `WINDOW_TITLE` et `CAPTEUR`
  en sont retirées. Toute variable d'injection posée au lancement atteint donc
  **tous** les enfants.
- `windows_audio/fil.rs:107-118` — le budget de `AUDIO_FAUTE_LECTURE` est un
  `static` **par processus**, et chaque fenêtre est son propre processus :
  chaque enfant reçoit donc un budget **neuf**.
- `windows_audio.rs:39` — `POLL_INTERVAL = 5 ms` ; `audio.rs:118` —
  `LECTURES_ECHOUEES_MAX = 10`.

Or seule la fenêtre **porteuse** consomme des fautes : le garde `if !emettait { … continue; }` (`fil.rs:196-203`) fait qu'une source muette n'appelle jamais
`read()` — c'est le témoin d'armement que D10 a trouvé. La voisine reste donc
intacte tant qu'elle se tait… **et meurt en ≈ 50 ms dès qu'elle est promue**
(10 fautes × 5 ms), sur son propre budget resté plein.

**Conséquence chiffrée** : la voisine promue disparaît avant d'avoir pu produire
la moindre preuve d'écoute. `compteurs audio` est périodique à
`REPORT_INTERVAL = 30 s` (`windows_audio.rs:42`) — soit **600 fois** la durée
de vie de la promue. Le critère ④ resterait non démontrable.

### 4.3 Le remède : deux boutons, et pourquoi deux

| Variable | Rôle | Défaut, et pourquoi |
| --- | --- | --- |
| `AUDIO_FAUTE_RECONSTRUCTION=<n>` | fait échouer les *n* prochaines reconstructions | absente = désarmée. Budget **global au processus** (`OnceLock` + `AtomicU32`), **jamais par appel** — c'est la leçon que D10 a payée sur `AUDIO_FAUTE_LECTURE` (§10 de ses résultats) |
| `AUDIO_FAUTE_LECTURE_MS=<ms>` | **borne dans le temps** l'armement de l'injection de lecture existante | absente = **illimité**, donc le comportement de D10 est strictement préservé et ses recettes restent reproductibles |

La seconde est ce qui rend le critère ④ atteignable **sans nommer de session** :
la porteuse consomme son budget dans les premières millisecondes, la fenêtre
d'armement se referme, et la voisine — promue au plus tôt 6 s plus tard, une
fois le budget de reconstruction de la porteuse épuisé — lit pour de vrai.
**Aucune reconnaissance préalable, aucun nom de session à deviner, aucun appel
Win32 de plus.**

**Point de chute** :

- `AUDIO_FAUTE_RECONSTRUCTION` dans **`transport/piste_audio.rs`** (403 lignes,
  marge 97) — et **non** dans `demarrage/audio.rs`, qui est `#[cfg(windows)]`.
  `transport/` est portable : l'injection y est **éprouvable sur l'hôte Linux**,
  donc son rouge s'obtient sans VM. Précédent de lecture d'environnement dans
  `transport/` : `PART_SONDAGE` (`transport/part.rs:40-49`).
- `AUDIO_FAUTE_LECTURE_MS` dans **`windows_audio/fil.rs`** (339 lignes, marge
  161), auprès du budget qu'elle borne.

⚠️ **Convention, et le piège payé trois fois** : ce sont des **variables de
BANC, jamais une configuration livrée** — même statut que `PART_SONDAGE`, avec
une trace `warn!` émise **seulement si armée**. Et **toutes deux doivent être
ajoutées explicitement à `scripts/run-agent.sh`** (124 lignes ; l'ajout se pose
auprès de `AUDIO_FAUTE_LECTURE`, l. 42). Le piège a été payé en D1
(`SUPERVISEUR`), D2 (`MULTIFENETRE_REPRISE`) et D6 (`BUDGET_BPS`) : sans cette
ligne, l'agent démarre sans la variable **et sans rien signaler**.

---

## 5. Famille ③ — les deux familles ensemble, et la séparation des flux (leg 8)

### 5.1 Le 🔴 du §9 de D10 : ① à dix fenêtres, ② à une seule

D10 le nomme lui-même « la lacune de couverture la plus lourde ». Deux
conséquences, qu'il énonce et que D11 reprend telles quelles :

- le couplage documenté par sa tâche 12 — **la réélection annule le répit
  `REPIT_RECONSTRUCTION`, donc une ouverture WASAPI bloquante peut tomber sur
  le fil de drainage** (`piste_audio.rs:158-180`) — n'a été exercé qu'à **une**
  fenêtre, alors qu'il n'est borné par `PERIODE_REARBITRAGE` (250 ms) que
  **parce que plusieurs fenêtres peuvent le déclencher** ;
- et le point suivant, qui est le plus utile.

#### 5.1.1 La discrimination que seule une recette multi-fenêtres peut donner

**À une seule fenêtre, `audio_porteuse` vaut toujours `true`.** La recette verte
de D10 **ne peut donc pas distinguer** le correctif livré
(`set_actif(self.audio_porteuse)`) de la version que le code déclare **pire**
(`set_actif(true)` inconditionnel) : les deux rendraient 441 Hz. D10 l'écrit —
« cette discrimination n'existe que dans les tests d'hôte ».

**À deux fenêtres d'un même groupe de PID, elle existe sur la VM** : la
non-porteuse reconstruite doit rester **muette**. Un `set_actif(true)`
inconditionnel la ferait parler, et les deux fenêtres joueraient le même mix
désynchronisé — l'**écho audible** que F2 de D7 décrit déjà.

C'est donc, en une seule recette, la couverture manquante **et** le contrôle
discriminant. Montage : **deux fenêtres Chrome `--app` partageant un
`--user-data-dir`** — un seul `chrome.exe`, montage déjà validé par D8, dont
`CLAUDE.md` rappelle qu'il faut **relever les PID avant de conclure**, jamais
les supposer d'un nom d'exécutable.

### 5.2 ⚠️ Le contrôle de séparation des flux de D10 ne peut pas échouer (leg 8)

L'empreinte échantillonnée par `pilote-critere1-d10.mjs:191-207` est un
sous-échantillonnage 8×8 de l'élément `<video>` **vivant**, relevé page par page
par des allers-retours CDP indépendants (étalement mesuré : 187 et 245 ms), sur
une source dont le fond **dérive à chaque trame** (`anim-d4.html`, `phase =
(t / 20) % 360`). **Deux pages décodant le MÊME flux rendraient donc des
empreintes différentes elles aussi.** Le contrôle garde son pouvoir sur les
seules pages **figées**, où il ne trouve aucune collision — et c'est tout ce que
D10 revendique.

**Le remède est dans l'instrument, pas dans le produit** : la mire reçoit un
**marqueur d'identité invariant dans le temps** — un aplat de couleur, à une
position fixe, dérivé du paramètre `n` que `anim-d4.html` reçoit déjà
(`instrument/anim-d4.html`, `const n = new URLSearchParams(...).get('n')`) —, et
le contrôle n'échantillonne **que ce patch**. Le fond continue de dériver, donc
Desktop Duplication continue d'émettre : le piège de la mire immobile n'est pas
réintroduit.

**Deux pages sur le même flux rendent alors le même marqueur, par
construction.**

⚠️ **L'instrument vit sous `docs/superpowers/plans/journaux-multifenetres-d11/instrument/`**
— répertoire exempté de la règle des 500 lignes, et **aucun fichier de code
n'est touché par ce leg**.

---

## 6. Famille ④ — le coût de la duplication surdimensionnée (leg 7)

### 6.1 Ce qu'on mesure, et contre quoi

D10 accepte une sortie née trop grande et **recadre** dans sa duplication. Sur
cette VM, les sorties naissent à **3840×2160** pour un rectangle utile de
1280×720 : la duplication porte **9 fois** l'aire encodée. **Rien ne mesure ce
que cela coûte**, et c'est le prix assumé de la voie retenue.

**Un A/B à une seule variable** : même nombre de fenêtres, même taille retenue,
même taille d'encodage — **seule la taille de naissance des sorties change**.

| Bras | Registre | Comment il s'obtient | Comment il se **vérifie** |
| --- | --- | --- | --- |
| SALE | 3840×2160 | l'état courant de la VM | `duplication de sortie établie desktop_width=3840` au journal |
| PROPRE | 1280×720 | `MULTIFENETRE_MODE_SORTIE=1280x720`, **dans un lancement à elle seule** | `desktop_width=1280` |

Grandeur relevée : la **cadence par fenêtre** du capteur
(`cadence du capteur … images=… cadence="…"`), sur un palier de 60 s, à huit
fenêtres, **deux exécutions par bras**.

### 6.2 ⚠️ Le bras PROPRE peut ne pas être obtenable, et c'est déclaré d'avance

La portée de la pollution de registre — **par GUID ou globale** — n'est pas
tranchée, et D10 l'a rendue *sans objet*, pas *résolue* : `agent-recette.log` de
D8 porte **cinq GUID SudoVDA distincts**. Si la sonde ne déplace que le sien, les
sorties suivantes peuvent naître grandes malgré elle.

**La règle est écrite avant la mesure** : le bras PROPRE n'est réputé obtenu que
si le journal porte `desktop_width=1280` pour **toutes** les sorties du rang. À
défaut, **la mesure est déclarée non prise** — et non pas approchée, ni
interprétée. C'est l'issue acceptable de ce leg : mieux vaut « non pris, et
voici pourquoi » qu'un chiffre bâti sur un bras non établi.

⚠️ **Une variable de plus qu'il faut nommer** : `MULTIFENETRE_MODE_SORTIE`
**n'enchaîne pas deux sondes** — l'aiguillage de
`diagnostics/multifenetre.rs` retourne après la première reconnue. Un
`MULTIFENETRE_VDD_PURGE=1` dans le même lancement serait ignoré **en silence**.

---

## 7. Famille ⑤ — relire l'instrumentation du `Resize` (leg 2), et les legs froids

### 7.1 Le leg 2 — il ne manque qu'une lecture

**Aucun code à écrire côté produit.** `client/src/main.ts:346-374` porte déjà la
grille de lecture à trois issues, posée par la tâche 18 de D10, et
`main.ts:319-327` trace le report quand le canal n'est pas ouvert (`console.warn`).
Ce qui manque est **une exécution qui lise ces logs sur la VM**.

Ce que la grille tranche, dans son ordre de lecture :

1. **aucun** log `declenchement ResizeObserver` pour une session qui n'émet
   jamais de `Resize` ⟹ le maillon est **en amont de la mise en page** ;
2. log présent et `clientWidth` **suit** `innerWidth` ⟹ ni l'observateur ni la
   mise en page ne sont en cause ;
3. log présent et `clientWidth` **ne suit pas** ⟹ la mise en page CSS de
   l'élément `<video>` est en cause.

**Ce qui manque à l'instrument** : les trois pilotes de D10 appellent
`Runtime.enable` (`pilote-critere1-d10.mjs:278` et ses jumeaux) mais **aucun ne
s'abonne à `Runtime.consoleAPICalled`** — les `console.debug` de la page ne sont
donc collectés nulle part. Le pilote de D11 s'y abonne, page par page, et verse
le flux de console avec les journaux.

⚠️ **Ce que cette recette ne promettra PAS** : elle rend le maillon **décidable**,
pas identifié d'avance. Le canal de contrôle **reste une hypothèse à part
entière** — la correction C3 de D8 a établi qu'aucune pièce ne le disculpe, et le
`console.warn` de la ligne 326 est précisément là pour l'attraper. **L'issue « on
ne reproduit pas le silence » est possible** : D9 a déjà relevé que le réseau
local est trop rapide pour provoquer la course, et que la forcer par latence a
cassé la reconnexion. Elle serait alors déclarée telle quelle.

### 7.2 Famille ⑥ — les deux legs de D9 tombés du registre

**Aucune mesure, aucune VM.** Trois gestes, tous éprouvables sur l'hôte :

- `une_telemetrie_neuve_est_a_zero` (`windows_source/telemetrie.rs:69`) soit
  **exerce la logique propre** de `Telemetrie` (`tick`/`capturee`/`produite`),
  soit **disparaît** — le dépôt n'a pas de doctrine sur le code orphelin (D10 le
  relève lui-même), mais il en a une sur les tests : *un test qui ne peut pas
  rendre l'autre valeur n'est pas un test*. La couverture réelle vit déjà dans
  `deux_telemetries_ne_se_melangent_pas` ;
- `client/src/resize.test.ts:18` soit **exerce réellement l'état de canal
  qu'il annonce**, soit **change de titre** pour dire ce qu'il éprouve. La
  seconde branche est légitime : `RejeuResize` est délibérément pur et sans DOM,
  et lui donner une notion de canal serait un couplage neuf pour un test ;
- l'invariant de `client/src/main.ts:386` est **écrit dans un commentaire** —
  le *pourquoi* de l'ordre, pas seulement le *quoi* — et, si le coût est nul, un
  test le garde. ⚠️ **Le vérifier avant d'écrire** : la ligne a déjà bougé de
  `:345` à `:386`, et un commentaire qui cite un numéro de ligne le reperdra.

---

## 8. Recettes, critères, et le ROUGE de chacun

### 8.1 Les critères

**Deux exécutions par critère** (règle héritée de D9), et **aucun taux ne sera
revendiqué nulle part**.

| # | Critère | Verdict jugé sur | Exéc. |
| --- | --- | --- | --- |
| ① | **En mono-fenêtre, une capture audio reconstruite redevient audible** | fréquence **dominante** reçue = celle assignée à la source, plancher de bruit à l'appui — jamais un compte d'octets (D7 : `bytesReceived` a crû sur un spectre à −1000 dB) | 2 |
| ② | **À deux fenêtres d'un même PID, la porteuse reconstruite parle et la voisine se tait** | dominante attendue sur la porteuse **et** absence de dominante sur la voisine, dans la **même** mesure | 2 |
| ③ | **Une capture irrécupérable retombe sur la promotion** | épuisement du budget → `AudioMort` → élection de la voisine → la voisine émet réellement | 2 |
| ④ | **Deux fenêtres montrent deux flux distincts** | marqueurs d'identité invariants, tous distincts deux à deux | 2 |
| ⑤ | **Le coût de la duplication surdimensionnée** | cadence par fenêtre, bras SALE contre bras PROPRE — ou **« non pris »** si le bras PROPRE n'est pas établi | 2 par bras |
| ⑥ | **Le maillon du `Resize`** | laquelle des trois issues de la grille se produit — ou **« non reproduit »** | 2 |

### 8.2 Le ROUGE de chacun, et sa vérification d'atteignabilité

**Doctrine du dépôt, payée sept fois** (F1 de D7 ; la sonde P1 de D8 ; le
confondeur de D9 ; **quatre** contrôles vacueux en D10, dont **trois écrits par
le plan lui-même**) : *un contrôle qu'on n'a jamais vu rouge n'est pas un
contrôle*. Pour chacun, l'état à provoquer **et** la preuve qu'il est
atteignable :

| # | ROUGE | Pourquoi il est atteignable |
| --- | --- | --- |
| ① | le binaire de `main` (`df03fc6`), sous la même injection | le défaut est **structurel** et démontré par lecture de code (§3.1) : `audio_porteuse` y vaut `false` pour toujours en mono-fenêtre. **Le rouge n'est pas un pari** |
| ② | un binaire portant `set_actif(true)` inconditionnel, **bâti pour la seule mesure et jamais fusionné** | c'est la version que `piste_audio.rs:265-269` déclare *pire* ; elle ferait parler la voisine. **C'est la discrimination que D10 n'a pas pu faire** (§5.1.1) |
| ③ | `AUDIO_FAUTE_RECONSTRUCTION` **désarmée**, tout le reste identique | la reconstruction réussit alors, `AudioMort` n'est jamais émis, aucune promotion n'a lieu. Le contrôle a bien **deux** issues |
| ④ | **deux fenêtres lancées avec le MÊME `n`** | le contrôle doit alors signaler une collision. Déterministe, sans dépendre du produit |
| ⑤ | *sans objet* — c'est une mesure, pas un contrôle | mais le **contrôle du bras** l'est, et il peut échouer : `desktop_width` vaut 3840 ou 1280, jamais les deux |
| ⑥ | *sans objet* — c'est un diagnostic | l'atteignabilité porte sur la **collecte** : vérifier qu'un `console.debug` posé à la main dans la page ressort bien du pilote, **avant** de conclure de son absence |

⚠️ **La ligne ⑥ est celle qui mérite le plus d'attention.** L'issue n°1 de la
grille se lit sur une **absence de log** — et une absence de log est exactement
ce que produit aussi un pilote qui ne collecte pas la console. **Les deux se
lisent pareil.** Le contrôle d'atteignabilité ci-dessus n'est donc pas une
formalité : sans lui, l'issue n°1 serait indiscernable d'une panne
d'instrument, et D11 rejouerait le naufrage de F1.

### 8.3 Montages, tous hérités

Fenêtres **Chrome `--app` animées** à cadence connue, **un `--user-data-dir` par
fenêtre** — sauf pour ② et ③, qui exigent au contraire un `--user-data-dir`
**partagé** pour obtenir un PID unique, les PID étant **relevés** et non
supposés. Navigateur pilote sur l'**hôte**, jamais sur la VM. `Get-Process agent`
revérifié **après chaque tentative, y compris échouée**. Appels à la VM
**bloquants au premier plan**, jamais backgroundés — D10 y a perdu deux
exécutions. `grep -a` sur tout journal copié pendant que Windows écrit encore.

---

## 9. Contraintes de méthode qui gouvernent ce sous-bloc

### 9.1 Le plafond de 500 lignes, budgété d'avance

**Relevé par la commande le 19 août 2026**, et non recopié :

| Fichier | Lignes | Marge | Rôle dans D11 |
| --- | --- | --- | --- |
| `agent/src/transport/piste_audio.rs` | **403** | **97** | `AUDIO_FAUTE_RECONSTRUCTION`, `{erreur:#}`, l'accesseur |
| `agent/src/transport/tick/tests/audio.rs` | **403** | **97** | les tests neufs des deux injections |
| `agent/src/windows_audio/fil.rs` | **339** | **161** | `AUDIO_FAUTE_LECTURE_MS` |
| `agent/src/demarrage/audio.rs` | **95** | **405** | l'appel mono-fenêtre |
| `agent/src/transport.rs` | **468** | **32** | ⚠️ **la doc du champ `audio_porteuse` vit ici** — toute addition y appelle une **extraction**, pas une compression |
| `scripts/run-agent.sh` | **124** | — | hors portée de la règle |

**Aucune extraction n'est requise a priori** : les quatre fichiers du chemin
principal ont de la marge. **Une seule vigilance, et elle est nommée** :
`transport.rs` (marge 32) porte la documentation du champ que le leg 4 corrige,
et sa mise à jour est **inévitable** — le commentaire actuel décrit un invariant
qui va cesser d'être vrai. Si l'addition dépasse la marge, la règle du dépôt
s'applique **sans exception** : extraction d'abord, addition ensuite. Point de
chute : les champs et leur documentation ont vocation à rejoindre
`transport/piste_audio.rs`, où vivent déjà leurs seuls écrivains.

⚠️ Pour mémoire : le plafond a été franchi **trois fois** en D10, chaque fois
rattrapé par une extraction et **jamais par une compression**. D9 avait joué la
compression deux fois, et la revue l'a fait défaire les deux fois.

### 9.2 La revue transverse de fin de branche — obligatoire

Cinq défauts en D7, trois Critiques en D8, six en D9, **douze en D10** — et
**tous franchissent une frontière de tâche** : chacun est correct des deux côtés
pris séparément. Une revue par tâche ne peut structurellement pas les voir.

**Sa cible propre en D11**, nommée d'avance : **les commentaires qui décrivent le
mono-fenêtre**. Le leg 4 en a déjà réfuté **six** en D10 — la revue transverse en
avait corrigé trois en affirmant qu'il n'y en avait que trois, la revue finale en
a trouvé deux de plus, et le balayage exigé par elle un sixième. **Le correctif
du leg 4 les réfute tous une seconde fois**, puisqu'il change ce qu'ils décrivent.
Balayage prescrit : `grep -rn "mono-fenêtre\|mono-fenetre" agent/src client/src`,
place par place, **relu après édition** et non seulement avant.

### 9.3 Le mode de défaillance n°1, et il n'est pas technique

D10 a relevé **deux pièces FABRIQUÉES** présentées comme des relevés — une
transcription `cargo` assemblée à la main, et une sortie de commande inventée
inscrite dans `CLAUDE.md` **à l'intérieur d'une correction qui dénonçait une
affirmation non étayée**. Les deux fois, le fait rapporté était vrai ; les deux
fois, la preuve ne l'était pas. Le mécanisme a été nommé par son auteur :
**réutiliser la sortie d'une commande antérieure pour répondre à la question
d'une autre, sans la relancer.**

**Règle de ce sous-bloc** : toute affirmation « vérifié par la commande » est
accompagnée de la commande **et** de sa sortie, relancée pour cette
affirmation-là. Un nombre de lignes ne se recopie jamais.

### 9.4 Aucune preuve ne vit dans un rapport gitignoré

Le leg 3 est perdu parce que l'analyse de D9 vivait dans `.superpowers/sdd/`.
D11 tient la contre-mesure de D10 : un **document de résultats permanent** porte
l'analyse, et les pièces brutes sont versées sous
`docs/superpowers/plans/journaux-multifenetres-d11/`. **Aucune affirmation de
`CLAUDE.md` ne doit dépendre d'un rapport de tâche.**

---

## 10. Gestion des erreurs

- **Une reconstruction refusée** n'est pas fatale : la session continue muette,
  et `AudioMort` part au capteur une fois le budget épuisé. **Le repli n'est
  JAMAIS le mix global** — arbitrage écrit au cadrage de D7, qui ne se rouvre pas
  ici.
- **L'accesseur du leg 4 ne touche que le mode mono-fenêtre.** Il ne doit exister
  aucun chemin par lequel il s'applique à une session servie par un capteur : la
  fuite de son vers une fenêtre qui doit se taire est le défaut *pire*, et le
  test qui le garde rouge ne doit pas changer.
- **Les deux injections sont inertes par défaut.** Absentes, le binaire se
  comporte exactement comme celui de D10 — et ses recettes restent reproductibles.
- **La branche a1sexies ne met aucun paquet en file** : c'est l'invariant de
  drainage de `transport/tick.rs`, que D6 a payé deux rondes pour énoncer
  correctement. Aucune des additions de D11 n'y touche.
- **Un bras de mesure non établi produit « non pris »**, jamais un chiffre
  approché (§6.2).

---

## 11. Stratégie de test

**Tout ce qui peut être pur l'est**, et se teste sur l'hôte Linux :

- `AUDIO_FAUTE_RECONSTRUCTION` vit dans `transport/`, qui ne connaît qu'un objet
  de trait et une fermeture : la chaîne « injection → refus → épuisement →
  `AudioMort` » est **entièrement éprouvable sans VM**, avec un reconstructeur
  factice, comme le sont déjà les tests de
  `transport/tick/tests/audio.rs:351-403` ;
- le correctif du leg 4 se couvre par un test **symétrique** de
  `une_session_non_porteuse_reconstruite_reste_muette`, et ce dernier reste vert
  **sans modification** — c'est la vérification que le correctif ne déborde pas ;
- ⚠️ **et une leçon de D10 s'applique en plein ici** : *une source factice qui
  implémente un effet de bord en NO-OP rend une famille entière de défauts
  invisible aux tests d'hôte*. `set_actif` en no-op a laissé passer un état
  absorbant complet sur 456 tests verts. **Toute source factice de D11 doit
  OBSERVER `set_actif`**, jamais l'ignorer — c'est ce que fait déjà
  `SourceVivante::observant_actif`.

**Références mesurées aujourd'hui, à opposer aux comptes de fin de branche** :

```
$ cargo test -p agent
test result: ok. 458 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ npx vitest run   (depuis client/)
 Test Files  12 passed (12)
      Tests  107 passed (107)
```

Vérifications de fin de branche, comme en D9 et D10 : `cargo test -p agent`,
`cargo check --target x86_64-pc-windows-gnu` (types, emprunts, visibilités et
durées de vie ; **pas l'édition de liens**), et `npx vitest run`.

⚠️ **Annoncer le compte de tests attendu AVANT de le mesurer.** D10 a récupéré
un test écrasé par un `Write` uniquement parce que le compte est sorti à 452 au
lieu des 453 annoncés d'avance.

---

## 12. Ce que D11 n'établira PAS

- **Aucun taux, nulle part.** Deux exécutions par critère au mieux.
- **L'existence d'une cause NATURELLE de mort de capture audio reste inconnue.**
  Les quatre déclencheurs de D9 n'en produisent aucune ; le cinquième tue la
  fenêtre avant l'audio ; tout ce que D11 mesure l'est **sous injection de
  faute**. **L'injection établit que le remède fonctionne, jamais qu'une cause
  existe.**
- **Le leg 6 devient observable, pas expliqué** : si aucun refus ne survient
  pendant les recettes, la cause reste inconnue — mais elle cessera d'être
  invisible.
- **Le maillon du `Resize` peut rester non identifié** : « on ne reproduit pas le
  silence » est une issue déclarée d'avance.
- **Le coût de la duplication surdimensionnée peut n'être pas pris**, si le bras
  PROPRE n'est pas établi (§6.2).
- **La portée du blocage registre** — par GUID ou globale — reste **non
  tranchée**, et **rien ne nettoie le registre**.
- **L'A/B sur `set_desired_bitrate` reste dû**, écarté par décision (§2.2).
- **Les six constats parqués de D9 restent perdus.**
- **Le plafond de 8 encodeurs au-delà de 720p** reste inconnu.
- **Les trois couches inconnues du chantier D** le restent : le plafond de 8
  encodeurs, celui de 4 processus, et le mécanisme de l'abandon du mutex DXGI.
- **La latence de bout en bout**, qu'aucun sous-bloc du chantier D n'a jamais
  mesurée — et D11 ne la mesurera pas davantage.
- **Aucune constante n'est calibrée** par un jugement visuel ou d'écoute :
  `BPP_MIN`, `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`, `HYSTERESIS`,
  `REPIT_APRES_ECHEC`, `TAILLE_MAX_SORTIE`, `REPIT_REARMEMENT_AUDIO`,
  `REARMEMENTS_MAX`, `RECONSTRUCTIONS_MAX`, `REPIT_RECONSTRUCTION`.
- **La visibilité et le focus restent imposés par le pilote de recette**, page
  par page — limite héritée de D5, qu'aucun sous-bloc n'a levée.
- **Aucun client réel, aucun HiDPI réel** : `deviceScaleFactor = 1` partout, donc
  le legs HiDPI reste **inexercé**.
- **Le chemin d'extinction propre du superviseur** n'aura toujours jamais été
  exercé, depuis D1.
- **Le couplage réélection / répit** (§5.1) est **exercé**, pas **borné** : la
  recette ② le fait tourner à plusieurs fenêtres, elle ne mesure pas le retard
  qu'il inflige au fil de drainage.

---

## 13. Hors périmètre

- **Rejouer l'A/B sur `set_desired_bitrate`** — écarté avec sa condition de
  réouverture (§2.2).
- **Reconstituer les six constats perdus** — irrécupérables ; les inventer serait
  pire.
- **Nettoyer le registre**, sous quelque forme que ce soit : la voie de D10 rend
  le produit indifférent à son état, et y revenir réintroduirait l'écriture que
  D9 en a retirée.
- **Expliquer** la naissance d'une sortie à la taille du registre, ni la
  non-persistance du changement de mode. Deux faits mesurés, non expliqués.
- **Identifier** la couche du plafond de 8 encodeurs ou de 4 processus.
- **Réactiver le redimensionnement par fenêtre** — comportement neuf, pas un leg.
- **Passer la reconstruction audio sur un fil** : `piste_audio.rs:271-275` nomme
  déjà la condition (« si la mesure montre qu'elle retarde le drainage »), et
  cette mesure n'est pas au programme.
- La latence de bout en bout, le recouvrement, le déplacement de fenêtre, le
  clavier concurrent, les applications UWP.

---

## 14. Découpage indicatif — quinze tâches

L'ordre porte **une seule dépendance forte** : la famille ① doit être livrée
avant les recettes ② et ③, qui en éprouvent le correctif.

| # | Tâche | Nature |
| --- | --- | --- |
| 1 | Réparer le §11 du document de résultats de D10 : il porte **huit** legs et non sept (§1.1), et **les n°11 et 12 de D9 n'en sont jamais sortis** (§1.2) | document |
| 2 | Leg 6 — `%erreur` → `{erreur:#}` (`piste_audio.rs:325`) | code, 1 ligne + sa raison |
| 3 | Leg 4 — l'accesseur sur `Session`, son appel dans la branche `None` de `brancher`, et la documentation de `transport.rs` mise en accord | code + tests d'hôte |
| 4 | Leg 5 — `AUDIO_FAUTE_RECONSTRUCTION` dans `piste_audio.rs`, budget **global au processus** | code + tests d'hôte |
| 5 | `AUDIO_FAUTE_LECTURE_MS` dans `windows_audio/fil.rs`, et **les deux** variables dans `scripts/run-agent.sh` | code + script |
| 6 | **Famille ⑥** — les deux legs de D9 tombés du registre (§7.2) : les deux tests faibles, et l'invariant de `main.ts` | code + tests d'hôte |
| 7 | Leg 8 — le marqueur d'identité invariant dans la mire, et le prédicat de distinction du pilote | instrument |
| 8 | Le pilote de recette audio (mono- et multi-fenêtres), et le pilote de rejeu `Resize` **avec abonnement à `Runtime.consoleAPICalled`** | instrument |
| 9 | Recette ① — mono-fenêtre, **avec son ROUGE sur `df03fc6`**, 2 exécutions | mesure VM |
| 10 | Recette ② — deux fenêtres d'un même PID, **avec son ROUGE sur un binaire `set_actif(true)` jamais fusionné**, 2 exécutions | mesure VM |
| 11 | Recette ③ — le repli sur la promotion, **avec son ROUGE injection désarmée**, 2 exécutions | mesure VM |
| 12 | Recette ④ — la séparation des flux, **avec son ROUGE à deux fenêtres de même `n`**, 2 exécutions | mesure VM |
| 13 | Recette ⑤ — le coût de la duplication, 2 exécutions par bras, **ou « non pris »** | mesure VM |
| 14 | Recette ⑥ — le rejeu du `Resize`, **précédée du contrôle d'atteignabilité de la collecte console** (§8.2) | mesure VM |
| 15 | Document de résultats permanent, **revue transverse de fin de branche** (§9.2), et mise à jour de `CLAUDE.md` | document |

⚠️ **La tâche 10 construit un binaire délibérément défectueux.** Il est bâti pour
la seule mesure, versé avec sa provenance (hachage et taille, comme D10 l'a fait
pour ses deux binaires), et **jamais fusionné**. Sans lui, le critère ② ne se
distingue pas de la version *pire*, et D11 rejouerait à l'identique la lacune
que D10 s'est reprochée.
