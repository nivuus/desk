# Sous-bloc D8 — la recette du plein écran (5 août 2026)

Cahier des charges : `.superpowers/sdd/2026-08-04-multifenetres-plein-ecran/task-11-brief.md`.
Instrument, écrit par la tâche 10 : `docs/superpowers/plans/journaux-multifenetres-d8/instrument/pilote-recette-d8.mjs`
— **modifié par la correction de revue** (voir « Ce que la correction a changé »
en fin de document) ; il ne l'a **pas** été pour l'exécution initiale.

⚠️ **Ce document a été corrigé sur DEUX rondes de revue.** La première
rédaction portait deux Critiques et quatre Importants — pour l'essentiel des
affirmations qui dépassaient leur relevé, dans les deux sens (trop dites ou
pas assez dites). Une seconde exécution (`REJOUER_2_ET_5=1`) a rejoué ② et ⑤
avec un instrument corrigé sur trois défauts. **Une seconde ronde de revue,
sur le document corrigé lui-même, a trouvé deux nouvelles casses** (une
latence de réveil qui mêlait une mesure agent et un artefact d'échantillonnage
du pilote ; une pièce d'appui — le bandeau du témoin — citée à l'envers) et
une lacune de provenance (une pièce de préparation régénérée après coup,
classée sans le dire) — aucune n'exigeant de nouvelle mesure. **Ce qui suit
est le document deux fois corrigé** ; rien n'est laissé sous une forme
fautive connue.

Deux exécutions produisent les pièces de ce document :

| Étiquette | Rôle | Journal pilote | Journal agent | JSON |
| --- | --- | --- | --- | --- |
| `recette` | exécution complète (témoin, ①+②, ④+⑤) | `critere-recette.log` | `agent-recette.log` | `recette-recette.json` |
| `rejeu-2-5` | rejeu de ② et ⑤ avec l'instrument corrigé | `critere-rejeu-2-5.log` | `agent-rejeu-2-5.log` | `recette-rejeu-2-5.json` |

Binaire agent mesuré (inchangé entre les deux exécutions — seul le pilote a été
corrigé) : commit `7032b01`, `agent.exe` 9 248 256 octets, horodaté 4 août
17:59, 4 minutes après le commit, `git status --porcelain` vide sur
`agent/ scripts/ proto/ client/ signaling/` au moment des deux mesures.
**Aucune recompilation n'a eu lieu** : le binaire déjà présent sur la VM était
à jour pour les deux exécutions.

## Le verdict, en une phrase

**Deux exécutions.** ④ est TENU sur ses deux moitiés (sommeil et réveil). ①
est confirmé sur son mécanisme (détection exclusive, à la bonne session, dans
les deux sens) — l'identité de la session qui portait la fenêtre du pilote
était mal résolue par le pilote, pas par l'agent. ⑤, rejoué avec une
identité correctement résolue, reçoit désormais la tonalité qui lui est
réellement assignée : la mesure initiale portait sur la mauvaise fenêtre. ②
reste **NON EXERCÉ** après les deux exécutions — mais la seconde, instrumentée,
établit que ce n'est **pas** faute du viewport CDP de prendre effet
(`window.innerWidth`/`innerHeight` atteignent la cible exactement) : c'est en
aval, entre ce fait et l'émission d'un message `Resize`, que la chaîne
s'arrête. **Les trois inconnues du brief restent donc entièrement ouvertes.**

| # | Critère | Verdict | Exécutions |
| --- | --- | --- | --- |
| témoin | Non-régression (aucun plein écran) | **TENU** | 1 |
| ① | Plein écran Windows détecté et annoncé, à la bonne fenêtre seule | **CONFIRMÉ** — détection exclusive, symétrique (activation/restauration), sur la session dont l'identité a été vérifiée. ⚠️ **La « corroboration » est PARTIELLEMENT CIRCULAIRE** : `resoudreIdentite()` identifie la session **par la bascule de bordure**, donc **par ① lui-même**. Ce que le rejeu corrobore est la cohérence de deux emplois du même mécanisme, pas ① par un signal indépendant | 1 mesure + 1 corroboration (voir la réserve ci-contre) |
| ② | Le flux suit le viewport plein écran, la sortie garde son nom | **NON EXERCÉ** — zéro tentative de changement de mode, dans les deux exécutions, malgré un viewport qui atteint bien sa cible côté page | 2 |
| ③ | Échap et Keyboard Lock | **NON MESURÉ** — décision actée en tête de tâche 10 | 0 |
| ④ | Les voisines s'endorment par le chemin existant | **TENU** — latences AGENT (ordre→transition) : sommeil 37 ms / 28 ms, réveil 471 ms / 119 ms selon l'exécution ; 2,2 s / 10,2 s ne sont PAS des latences produit, voir le corps du texte | 2 |
| ⑤ | L'audio d'une endormie survit | **TENU** — dominante à la fréquence assignée, une fois la bonne session ciblée | 1 mesure corrigée (1 mesure initiale invalidée par une mauvaise identité) |

## ⛔ La décision de conception : le changement de mode est DÉSARMÉ PAR DÉFAUT

**Prise par le propriétaire du dépôt à la revue finale de branche (5 août
2026), après ce document et sur ses pièces.** La moitié « changement de mode
de sortie » **n'a jamais été sollicitée par cette recette** — critère ② NON
EXERCÉ, `mode_sortie_demande=0` aux deux exécutions, **zéro tentative** de
`ChangeDisplaySettingsExW` en conditions de produit — et la revue finale y a
trouvé **deux Critiques**. On livre donc **la détection et l'annonce**, qui
sont mesurées et confirmées ; le changement de mode reste dans le code, sous
un garde qui l'arme.

**C'est exactement le repli que le §4 de la conception a écrit d'avance** :
« ①, ③, ④ et ⑤ tiennent sans ② ».

| Ce qui reste ACTIF, livré | Ce qui est DÉSARMÉ |
| --- | --- |
| relecture du style de fenêtre (`capteur/fenetre.rs`, `PERIODE_STYLE`) | `changer_mode_de_sortie` (`windows_source/redimensionnement/mode_sortie.rs`) |
| `DepuisCapteur::PleinEcran` → `AgentControl::Fullscreen` | tout appel à `ChangeDisplaySettingsExW` par le produit |
| armement client (`client/src/fullscreen.ts`) et Keyboard Lock | — |

**Le mécanisme** : une variable d'environnement dédiée,
**`PLEIN_ECRAN_MODE_SORTIE=1`**, qui **ARME** ce chemin. Absente, il ne
s'exécute pas. Elle est **distincte de `PLEIN_ECRAN=0`**, qui désarme le
mécanisme **entier**, et sa convention est **inverse à dessein** : on désarme
sur `=0` ce qui est livré, on arme sur `=1` ce qui ne l'est pas. Le garde vit
dans `agent/src/capteur/plein_ecran.rs::changement_de_mode_arme`, avec ses
raisons. Trace de contrôle, à chaque `resize` : `redimensionnement ignoré :
changement de mode de sortie DÉSARMÉ (poser PLEIN_ECRAN_MODE_SORTIE=1 pour
l'armer)`.

### ⚠️ Les deux Critiques ne sont PAS corrigées — et c'est délibéré

Le désarmement les rend **inatteignables**. Elles sont inscrites auprès du
garde, dans le code, **comme le premier travail de la recette qui armera ce
chemin** :

- **C1 — le produit bloque ses propres ouvertures de fenêtre ultérieures.**
  `changer_mode_de_sortie` écrit `CDS_UPDATEREGISTRY` à **chaque** plein écran
  réussi, et une sortie virtuelle **naît à la dernière taille laissée au
  registre** (mesuré, tâche 3bis). C'est littéralement le blocage que la
  préparation de cette recette a dû lever à la main (« Étape 0 » ci-dessous) —
  **le produit se l'infligerait à lui-même, en marche normale.**
  ⚠️ **Et sa portée est INCONNUE, par l'alternative des cinq GUID** : voir
  l'encadré C1 de l'étape 0. Les deux branches aggravent le défaut.
- **C2 — la reprise sur perte d'accès de D2 est court-circuitée.** Après un
  changement de mode réussi, `reconstruire_sur_la_sortie` rend une `Err` sur
  un échec de réouverture de la duplication — **y compris transitoire**, ce
  qui est la classe d'échec exacte que la fenêtre de reprise de D2 existe pour
  encaisser (44 pertes `0x887A0026` absorbées sans tuer une seule session,
  1ᵉʳ août 2026). Ici, la session meurt.

**Une troisième raison, qui n'est pas un défaut mais un manque** : ce chemin
**n'a jamais tourné en conditions de produit**. L'écart banc/produit est nommé
et non mesuré — la sonde P1 n'ouvre **jamais** de `DuplicateOutput`, là où la
production retaille une sortie dont la duplication est ouverte et détenue
pendant l'attente (jusqu'à 3,1 s). **C'est la première des trois inconnues du
brief, et elle commande les deux autres.**

## Étape 0 : un blocage environnemental a d'abord empêché toute mesure

**Avant toute chose, la préparation de la VM (§1 du brief) a révélé un défaut
sévère, non prévu par les « trois inconnues » du brief, mais qui en confirme un
fait déjà établi à un niveau plus grave que documenté.**

Après purge des sorties orphelines (0 orpheline trouvée, topologie propre —
`\\.\DISPLAY1` seule, physique) et lancement du superviseur, **aucune des trois
fenêtres n'a pu s'attacher** lors de la préparation de la première exécution :
le superviseur a créé et détruit en boucle des sorties virtuelles à 1280×720
pour une dizaine de noms de session différents en moins de 6 secondes, chacune
refusée par `sortie créée mais introuvable dans la topologie DXGI` — la sortie
apparaissait bien dans la topologie, mais à **2560×1440**, jamais à 1280×720
demandé (pièce : sortie de la commande `MULTIFENETRE_VDD_PURGE=1
scripts/run-agent.sh` suivie du lancement du superviseur, capturées dans le
terminal de préparation — **non versées en fichier séparé**).
⚠️ **CORRIGÉ (revue finale de branche) : la rédaction précédente écrivait ici
« voir le manque signalé en revue et CORRIGÉ ci-dessous ». Il ne l'est pas.**
Les deux pièces versées plus bas (`mode-sortie-1280x720-preparation.log`,
`dxgi-controle-preparation.log`) documentent le **remède** et le **contrôle
d'état** — pas ce relevé-ci : la sortie de terminal qui montre le superviseur
créant et détruisant ses sorties en boucle n'a **jamais** été capturée en
fichier, et ne peut plus l'être. **Le manque est DÉCLARÉ et NON CORRIGÉ** ; la
phrase disait l'inverse.

C'est exactement le défaut F1 déjà documenté en tête de
`agent/src/diagnostics/multifenetre/mode_sortie.rs` (« persistance probable au
registre d'un `CDS_UPDATEREGISTRY` d'une exécution antérieure ») — mais ici il
ne biaisait pas une sonde, **il bloquait le produit entier** : les mesures
antérieures avaient laissé le registre SudoVDA à 2560×1440, et **le code de
production** (`superviseur/boucle.rs`) exige une correspondance exacte avec la
taille demandée — sans le repli dynamique que P1 s'était donné pour se
prémunir de ce même défaut.

❌ **CORRIGÉ (C1, revue finale de branche) : « le GUID de sortie virtuelle
UNIQUE (`9C4A1F6E-2B73-4D51-9E08-677541430001`) » est FAUX.**
`agent-recette.log` porte **CINQ GUID SudoVDA distincts**, un par sortie
virtuelle créée — `…677541430001`, `…430002`, `…430003`, `…430004`,
`…430005` (lignes `sortie virtuelle créée`, `grep -aoE '[0-9A-F-]{36}'`). Il
n'y a donc pas « un » GUID, et l'affirmation d'unicité **ouvre une alternative
que ce document n'a pas les moyens de trancher** :

- **ou bien le mode enregistré est PAR GUID** — et alors la sonde P1, qui n'a
  agi que sur **une** sortie, ne pouvait pas débloquer les quatre autres :
  **la chaîne causale « remède appliqué → blocage levé » n'est pas fermée par
  ses pièces**, et ce qui a réellement débloqué la recette reste à établir ;
- **ou bien il ne l'est pas** — et alors **une seule écriture
  `CDS_UPDATEREGISTRY` empoisonne TOUTES les sorties futures**, ce qui est
  strictement pire que ce que le paragraphe décrivait.

⚠️ **Les deux branches AGGRAVENT le défaut, et aucune mesure ne les
départage.** C'est la Critique C1 de la revue finale de branche, et c'est
l'une des deux raisons pour lesquelles **le changement de mode de sortie est
désormais DÉSARMÉ PAR DÉFAUT** — voir « La décision de conception » en tête de
ce document.

**Remède appliqué : la sonde P1 elle-même**
(`MULTIFENETRE_MODE_SORTIE=1280x720`, invocation séparée — elle a son propre
aiguillage et sort après une seule mesure), qui crée une sortie, lit la taille
héritée du registre (2560×1440, confirmée), et la fait passer à 1280×720 par
`CDS_UPDATEREGISTRY` seul. **Pièce versée cette fois** :
`docs/superpowers/plans/journaux-multifenetres-d8/mode-sortie-1280x720-preparation.log`
(copie de `agent.log` juste après cette invocation) — verdict "P1 RECU",
`largeur_avant_tentative=2560 hauteur_avant_tentative=1440`,
`largeur_relue=1280 hauteur_relue=720`, `cible_exacte_atteinte=true`. Ce n'est
**pas** un changement de code — c'est l'exécution de l'outillage diagnostic
déjà présent, au même titre que la purge des sorties orphelines déjà prescrite
par le brief. Un contrôle de topologie (`MULTIFENETRE_DXGI=1`, même commande
que celle employée au moment des faits) confirme l'état propre — pièce :
`docs/superpowers/plans/journaux-multifenetres-d8/dxgi-controle-preparation.log`.
⚠️ **Provenance de cette pièce, signalée en re-revue** : elle est horodatée
**21:48:25**, c'est-à-dire produite en FIN de la ronde de correction de
revue — après la recette ET après le rejeu, pas au moment de la préparation
historique. Elle documente la MÊME vérification (topologie propre,
`\\.\DISPLAY1` seule), pas l'exécution historique exacte du tout premier
contrôle, qui n'a pas été capturée en fichier séparé sur le moment.

**Piège opérationnel neuf, à consigner** : la première tentative de combiner
`MULTIFENETRE_VDD_PURGE=1` et `MULTIFENETRE_MODE_SORTIE=1280x720` dans le même
lancement n'a rien fait pour la seconde variable — l'aiguillage de
`agent/src/diagnostics/multifenetre.rs` retourne après la **première** sonde
reconnue (chaque branche fait `return Ok(true)`), il ne les enchaîne pas.
**Chaque sonde doit être son propre lancement.** Second piège, plus coûteux,
rencontré **trois fois sur les trois occasions** où une exécution a suivi une
tentative précédente sur la même VM (préparation de la première exécution,
puis avant et après le rejeu) : un pilote ou une sonde qui laisse un
superviseur vivant fait échouer **silencieusement** le lancement suivant — le
nouveau `StreamWriter` ne peut pas ouvrir `agent.log` déjà tenu par l'ancien
processus, et la copie relue est celle, périmée, de la tentative précédente.
**`Get-Process agent` doit être revérifié après CHAQUE tentative, précédente ou
échouée — pas seulement avant la toute première.** C'est ce constat répété qui
a motivé le remède B2 ci-dessous (purge automatique en tête de pilote, plutôt
que confiée à l'opérateur).

## Le témoin de non-régression : TENU

Palier de 60 s, 3 fenêtres réellement ouvertes menant à **5 sessions
détectées** lors de la première exécution (voir « Fenêtres préexistantes »
ci-dessous pour ce que ce nombre signifie réellement — ce n'est **pas** ce que
la première rédaction en disait). Une session focalisée (`w-1`, une des deux
préexistantes) à 1280×720, quatre non focalisées réduites à 1024×576.

**Correction (Important 7, PUIS re-corrigée en re-revue — la pièce d'appui
était inversée)** : la réduction n'est **pas** causée par le « réseau » au
sens propre — le bandeau « Image réduite par le réseau » est un **libellé
d'interface**, pas un diagnostic de cause. Le journal porte **cinq**
`WARN … aucune estimation de bande passante reçue : l'adaptation reste
indisponible`. **Correction de la pièce (elle était donnée à l'envers)** :
c'est la session FOCALISÉE (`w:w-1`) qui porte le bandeau
« 1280×720, 4.0 Mb/s — adaptation indisponible » — c'est elle qui sonde le
lien et ne reçoit aucune estimation. Les **quatre** non focalisées portent
toutes « Image réduite par le réseau — 1024×576, 2.0 Mb/s » (`recette-recette.json`,
`phases.temoin.deltas`, champ `bandeau` de chaque session). La réduction
observée sur les quatre non focalisées (1280×720 → 1024×576) reste celle de
la **part de budget** attribuée par l'arbitrage D6
(`agent/src/capteur/sommeil/parts.rs`), pas une dégradation décidée par le
contrôleur de congestion réseau — c'est seulement la session focalisée dont
l'`Adaptation` reste `Indisponible`, faute d'estimation, parce que c'est elle
seule qui sonde le lien à pleine résolution. Les deux mécanismes (budget de
part, contrôleur de congestion) coexistent sans se confondre ; le fond de la
correction (« Image réduite par le réseau » est un libellé, pas une cause)
tient toujours, seule l'attribution par session était fausse.

Chiffres, focalisée : 84,71 i/s, 0,04 % jetées, 3,75 Mb/s. Non focalisées :
81,9 à 84,5 i/s, 0,04 à 4,21 % jetées. **« Comportement conforme au régime
documenté pour D6/D7 » est RETIRÉ** (Important 7) : aucun point de comparaison
n'a été nommé (D6 publie des chiffres à N=8, D7 n'a pas de rang comparable à
N=3/5) — un « TENU » ne doit pas s'appuyer sur une référence non citée. Le
verdict TENU repose seulement sur l'absence de régression observable :
aucune session n'est morte, aucune n'a décroché, aucune erreur. VM vivante
après la phase. **1 exécution, non rejouée** (le témoin ne dépend pas de
l'identité résolue par fenêtre — n'importe quelle session convient pour juger
« une focalisée / les autres réduites »).

## Fenêtres préexistantes : ce que « 5 sessions pour 3 fenêtres » signifiait réellement

**CORRIGÉ EN TOTALITÉ (Critique 2 de la revue).** La première rédaction
attribuait ce nombre au phénomène de « fenêtre fantôme » (Paint, Bloc-notes) —
**c'est l'explication inverse de ce que les pièces disent, et le mauvais
précédent était invoqué.**

Les faits, relevés directement :

| Fait | Pièce |
| --- | --- |
| Deux sorties virtuelles créées et deux enfants lancés (`w-2`, `w-1`) à 21:01:54,62 / 21:01:54,88 puis 21:01:55,03 | `agent-recette.log:19` et `:25` (les deux créations), `:26-27` (les deux enfants). ⚠️ *La plage `24-27` publiée jusqu'ici ne contient qu'UNE des deux créations : l'autre est à la ligne 19.* |
| Première fenêtre OUVERTE PAR LE PILOTE à 21:01:57,20 | `critere-recette.log:16` |
| Ouverture 1 (21:01:57,20) → enfant `w-4` lancé à 21:01:58,55 | `agent-recette.log:98` |
| Ouverture 2 (21:01:59,83) → enfant `w-6` lancé à 21:02:01,36 | `agent-recette.log:139` |
| Ouverture 3 (21:02:02,60) → enfant `w-8` lancé à 21:02:04,50 | `agent-recette.log:222` |

**Chaque fenêtre ouverte PAR le pilote a produit EXACTEMENT une session
neuve, dans l'ordre.** `w-1` et `w-2` existaient déjà **2,2 secondes avant** la
première ouverture du pilote : ce ne sont pas des doublons produits par une
fenêtre qui se multiplie, ce sont **deux fenêtres éligibles préexistantes** —
très vraisemblablement les rescapées, jamais fermées, d'une tentative
antérieure (le pilote ne ferme que son Chrome hôte dans son `finally`, jamais
les fenêtres qu'il ouvre côté VM). Le journal de la première exécution
annonçait « ÉTAPE 0 : VM sans fenêtre éligible » **sans jamais le vérifier.**

**Conséquence directe sur ① et ⑤** : le pilote assigne `cible = noms[0]` (donc
`w-1`) et `voisine = noms[1]` (donc `w-2`) en supposant que ce sont les deux
premières fenêtres QU'IL a ouvertes. C'est faux dans les deux cas : `w-1` et
`w-2` sont les rescapées, et la vraie fenêtre 1 du pilote (marqueur
`chrome-d8-recette-1`, hz 410) est en réalité la session **`w-4`** — voir ①
ci-dessous, qui l'établit par quatre faits indépendants, puis le rejeu qui le
confirme par résolution directe.

**Remède implémenté (B2)** : `purgerFenetresVMRescapees()` — tue tout
`chrome.exe` sur la VM et relit le compte (jamais ne le suppose) avant de
lancer le superviseur. Sur le rejeu, elle rapporte
`{"agents_restants":0,"chrome_restants":0}` avant lancement, et le superviseur
détecte alors **exactement 3 sessions pour 3 fenêtres** (`w-2, w-4, w-6`, une
par ouverture, dans l'ordre) — la correspondance qu'on aurait naïvement
supposée la première fois se vérifie effectivement une fois la VM
réellement propre. **Remède complémentaire (B3), volontairement redondant**
avec B2 : rien ne prouve qu'un doublon ne puisse survenir malgré un nettoyage
préalable — voir « Résolution d'identité » plus bas.

## Critère ① : CONFIRMÉ (le mécanisme, une fois la bonne session ciblée)

**CORRIGÉ (Critique 3 de la revue) — le mot « fuite » est retiré : il impute au
produit un défaut qu'aucune pièce ne montre.** Sur tout le run initial, il y a
**exactement deux** lignes `plein ecran de la fenetre Windows` — toutes deux
`session=w-4`, une `actif=true`, une `actif=false`. **Annonce unique, exclusive
et symétrique.** Aucune autre session n'a jamais rien reçu.

**La bascule de style Windows a réellement eu lieu, confirmée par relecture
directe** (`GetWindowLongPtrW` avant/après, jamais le seul code de retour) :
`avant=382664704` (bordure présente), `après=369819648` (bordure retirée), sur
la fenêtre marquée `chrome-d8-recette-1`.

**Que `w-4` — et non `w-1` — soit la session qui porte cette fenêtre repose sur
QUATRE faits indépendants, pas sur une coïncidence unique :**

1. `w-1` et `w-2` ne PEUVENT PAS être des fenêtres du pilote — c'est un fait
   d'horodatage (section précédente), pas une lecture.
2. Chaque ouverture du pilote ajoute EXACTEMENT une session, dans l'ordre —
   fait relevé, pas une inférence.
3. La bascule réelle (déduite de l'envoi de la tâche planifiée `sansBordure`,
   ~21:03:41,4) et la détection agent (21:03:41,971) sont séparées d'environ
   0,5 s.
4. La restauration (tâche planifiée `restaurer`, ~21:05:24,1) et `actif=false`
   (21:05:24,604) sont séparées de la MÊME latence, ~0,5 s, dans l'autre sens.

**Le verdict que ces pièces portent** : *le mécanisme agent de ① est démontré
— un vrai changement de style est détecté et annoncé à une session et une
seule, deux fois, à latence constante. Ce qui n'était pas établi par la
première exécution seule était l'identité physique de la session annoncée ;
l'hypothèse `cible = noms[0]` du pilote était réfutée.*

**Corroboration par le rejeu (`rejeu-2-5`, session résolue = `w-2` cette
fois — la VM ayant été repartie propre, la numérotation change, ce qui est
sans conséquence puisque l'identité est désormais résolue et non supposée) :**
`detection_agent.session = "w-2"`, **exactement** la session que
`resoudreIdentite` avait résolue pour ce marqueur AVANT que la mesure ①
officielle ne commence. `messages_navigateur_cible` porte 3 entrées ; les deux
premières proviennent de la balise d'identité elle-même (toggle diagnostique,
avant le début de la fenêtre de mesure officielle à 21:40:42,013Z), la
troisième (21:40:47,825Z, DANS la fenêtre officielle) est la bascule
mesurée par ①. `messages_navigateur_voisines.w:w-6` (la session non touchée)
est vide — zéro message, aucune exception.

⚠️ **Défaut d'instrument découvert par ce rejeu, à consigner** :
`window.__pleinEcran` (le tampon côté page) **n'est jamais vidé ni borné dans
le temps** — il persiste pour toute la durée de vie de la page et n'est
purgé que par dépassement de capacité (50 entrées). Le rejeu montre donc
`messages_navigateur_voisines.w:w-4` avec DEUX entrées historiques —
celles de la balise d'identité de LA FENÊTRE 2 elle-même (légitime bascule
de `w-4`, réalisée par `resoudreIdentite` avant le début de la phase ①+②,
21:40:29-34), lues alors comme si elles s'étaient produites PENDANT la
mesure. `fuite_vers_voisine: ["w:w-4"]` réapparaît donc dans le JSON du
rejeu, mais c'est un **FAUX POSITIF de l'instrument** : les deux horodatages
concernés (1785966029848, 1785966034388) précèdent le `debut` de la phase
(`"debut": "2026-08-05T21:40:42.013Z"`, soit ~1785966042013 en ms) de
12 à 8 secondes. **Aucun message n'a été reçu par une voisine PENDANT la
fenêtre de mesure du rejeu** — seul `w:w-6`, dont le tampon n'a jamais reçu
d'entrée d'aucune sorte, le prouve sans ambiguïté. Non corrigé dans
l'instrument (hors budget de cette ronde) : à borner par horodatage au
prochain travail sur ce fichier.

**1 exécution qui établit le mécanisme + 1 rejeu qui corrobore l'identité et
révèle un artefact d'instrument sans le contredire.**

## Critère ② : NON EXERCÉ (dans les deux exécutions), et mieux caractérisé

**Toujours zéro tentative de changement de mode de sortie**, dans les deux
exécutions (`mode_sortie_demande=0` aux deux runs). `stats_apres_redimensionnement.l/h`
et `stats_apres_surdimensionne.l/h` restent à 1280×720 dans les deux cas — le
flux vidéo n'a jamais bougé.

**CORRIGÉ (Important 5) — l'argument d'exclusion de la première rédaction
était circulaire.** Elle écartait l'explication « `PLEIN_ECRAN=0` désarme le
chemin » au motif qu'aucune trace `redimensionnement ignoré : PLEIN_ECRAN=0`
n'apparaît. Or le run tourne avec `PLEIN_ECRAN=1` : cette trace **ne pouvait
structurellement pas être émise**, quelle que soit la cause réelle — son
absence ne prouve donc rien sur cette hypothèse ni sur aucune autre.

❌ **La « pièce corroborante » que la ronde précédente ajoutait ici est
RÉFUTÉE PAR SES PROPRES JOURNAUX (C3, revue finale de branche) — et c'était
le seul argument qui situait la rupture côté client.** Elle écrivait : « sur
le run initial, l'agent reçoit **22 messages de contrôle** — 20 `Visibility`,
2 `Resize` (tous deux à la connexion initiale, 1280×720) ; le canal de
contrôle vit et délivre pour les cinq sessions **pendant toute la phase ②** ;
le défaut est donc bien côté ÉMISSION du client, pas côté transport ni côté
agent. »

**Le compte de 22 est exact — mais il porte sur TOUT LE RUN, pas sur la
phase ②, et la phase ② est précisément l'intervalle où le canal est MUET :**

| Fait | Relevé |
| --- | --- |
| Fenêtre de la phase ② | `21:03:35.810Z` → `21:05:23.233Z` (`recette-recette.json`, `phases.critere1et2`) |
| Lignes `contrôle reçu` dans cette fenêtre | **ZÉRO** |
| Dernière avant | `21:03:31.785` |
| Première après | `21:05:52.524` |
| Silence | **≈ 141 s**, soit plus que la phase entière |
| Idem au rejeu | phase ② `21:40:42.013Z` → `21:41:27.840Z` ; dernière avant `21:40:37.982`, première après `21:41:36.534`, **zéro dans la fenêtre** |

**Et « pour les cinq sessions » n'est étayé par rien non plus** : les lignes
`contrôle reçu` sortent d'`agent::demarrage` **sans champ `session` ni
span** — elles ne s'attribuent à aucune session (c'est le même défaut
d'attribution que D6 a payé, et le legs n°2 de D7 y touche).

**Conséquence, qu'il faut porter en entier** : *le transport n'est disculpé
par AUCUNE pièce de cette recette.* La rupture de ② peut se situer côté
émission client, mais **rien ici ne l'y situe** — l'argument qui le faisait
était faux. Le verdict de ② ne change pas pour autant : **« NON EXERCÉ, cause
inconnue » reste vrai, et l'est DAVANTAGE.** Le legs n°8 (instrumenter
`video.clientWidth`) reste utile, mais il vise désormais **une hypothèse parmi
d'autres**, et non le maillon désigné.

❌ **Fait annexe également RÉFUTÉ comme énoncé (I8)** : la rédaction précédente
écrivait « sur les 5 sessions du run initial, **3 n'ont jamais émis même leur
`Resize` initial de connexion** ». **Ce « 3 » n'est pas dérivable.** Ce qui est
RELEVÉ est : **2 lignes `Resize` pour 5 sessions**. Ces deux lignes ne portant
**aucun champ `session`**, rien n'établit qu'elles viennent de deux sessions
**distinctes** — elles pourraient venir de la même. Le « donc 3 » reposait sur
une hypothèse tacite d'unicité, publiée comme un fait et **inscrite en dette**
(legs n°10). **L'énoncé juste est : 2 `Resize` pour 5 sessions, l'attribution
par session étant impossible en l'état de la trace.** Ce qui reste ouvert est
donc *pourquoi si peu de `Resize`*, pas *lesquelles n'en ont pas émis*.

**L'hypothèse `--ozone-override-screen-size` est RÉFUTÉE par le rejeu, avec
mesure directe.** La première rédaction proposait, sans le vérifier, que
l'écran émulé du Chrome hôte du pilote (1600×1000) plafonnerait
silencieusement `Emulation.setDeviceMetricsOverride` demandé plus grand
(1920×1080, puis 3840×2160). Le rejeu instrumente désormais
`window.innerWidth`/`innerHeight` immédiatement avant et après chaque appel
CDP (remède B4) :

```
window.innerWidth/Height — plein écran : 1280x720 → 1920x1080 (cible 1920x1080)
                            surdimensionné : 1920x1080 → 3840x2160 (cible 3840x2160)
```

**Les deux appels atteignent leur cible EXACTEMENT**, y compris au-delà de
l'écran émulé. L'hypothèse du plafonnement est donc réfutée par une mesure
directe, pas seulement écartée par défaut d'observation. **Ce que ceci établit
avec précision, pour la première fois** : le viewport CDP prend bien effet
côté page (`window.innerWidth`/`innerHeight` corrects), et pourtant **aucun**
message `Resize` n'est émis vers l'agent (`mode_sortie_demande=0` au rejeu
aussi). **Le point de rupture est donc situé entre `window.innerWidth` et
l'émission du message de contrôle** — très probablement le `ResizeObserver`
sur l'élément `<video>` (`client/src/main.ts`), dont `clientWidth`/
`clientHeight` ne suivent `window.innerWidth`/`innerHeight` que si la mise en
page CSS le permet effectivement. **Ceci n'est PAS vérifié plus loin** : aucune
instrumentation de `video.clientWidth` elle-même n'a été ajoutée, et le
diagnostic s'arrête ici, dans le respect du mandat de mesure (ne pas corriger,
ne pas creuser le code client).

**Conséquence inchangée pour le brief : les trois inconnues restent
entièrement ouvertes** — voir la section dédiée, elle-même jugée la meilleure
partie du document initial et volontairement peu retouchée.

## Critère ③ : NON MESURÉ, décision actée en amont

Inchangé. Comme convenu en tête de la tâche 10 : la sonde P2 a établi que
Chrome `--headless=new` n'entre pas réellement en plein écran
(`document.fullscreenElement` reste `null` 800 ms après un
`requestFullscreen()` par ailleurs invoqué) et n'expose pas
`navigator.keyboard`. **Décision actée : ③ n'est pas instrumenté.** Confirmé
pour cette recette : `Xvfb` n'est **pas** installé sur cet hôte
(`which Xvfb` : introuvable). 0 exécution.

❌ **CORRIGÉ (I7, revue finale de branche) : la formule « sans installation de
`Xvfb` » laissait croire que ③ était HORS D'ATTEINTE. Il ne l'était pas.** Le
registre de la branche enregistre, **postérieurement à cette décision**, que
le **consentement d'installer `Xvfb` + `xdotool` a été DONNÉ** par le
propriétaire de la machine (`progress.md`, « DÉCISIONS DU PARTENAIRE HUMAIN
(4 août 2026) », point 3). **L'obstacle avait donc été levé, et l'installation
n'a simplement pas eu lieu.** L'énoncé juste est : *③ n'est pas mesuré parce
que l'installation consentie n'a pas été faite*, et non parce qu'elle était
refusée ou impossible. `CLAUDE.md` porte déjà cette version ; ce document ne
la portait pas, et les deux se contredisaient.

⚠️ **Conséquence à porter avec le consentement, et que la décision d'origine
énonçait déjà** : si l'installation se fait un jour, **les mesures qui en
sortiront ne se compareront à AUCUNE campagne antérieure** — les sept
précédentes ont toutes tourné sous Chrome sans interface. La conception le dit
sans y attacher de compte, et aucun n'est ajouté ici.

## Critère ④ : TENU (les deux moitiés)

**CORRIGÉ (Critique 1 de la revue) — le réveil ÉTAIT confirmé, et le pilote
avait un défaut qui l'empêchait de le voir.**

**Second correctif, sur cette même section (re-revue)** : la première
correction publiait « sommeil en 2,2 s, réveil en 471 ms à 10,2 s selon
l'exécution » comme si c'était un intervalle de latence produit. **Ce n'en
était pas un** — 2,2 s et 10,2 s sont le temps mis par le PILOTE à confirmer
le fait (`attendreLeFait` interroge le journal toutes les 2 s ; pour le
réveil, le test lit la prochaine ligne `cadence du capteur`, dont la
**période est de 10 s** — 10,2 s est un artefact d'échantillonnage, pas une
latence, exactement le piège maison « un compteur de journal peut compter des
LIGNES et non des ÉVÉNEMENTS »). Les latences RÉELLES sont celles de
l'agent, ordre reçu → transition, et elles sont dans les pièces déjà
versées :

| | Run initial (`agent-recette.log`, session `w-2`) | Rejeu (`agent-rejeu-2-5.log`, session `w-4`) |
| --- | --- | --- |
| Sommeil (ordre → `fenêtre endormie`) | **37 ms** (21:05:52.524 → 21:05:52.561, l. 614→621) | **28 ms** (21:41:36.534 → 21:41:36.562, l. 235→240) |
| Réveil (ordre → `fenêtre réveillée`) | **471 ms** (21:05:56.244 → 21:05:56.716, l. 631→642, `duree_ms=454`) | **119 ms** (21:41:40.178 → 21:41:40.297, l. 249→258, `duree_ms=97`) |

Masquage de la voisine par `document.hidden=true` : sommeil confirmé aux deux
exécutions (37 ms et 28 ms d'après le journal agent — 2,2 s est le délai de
confirmation du pilote, pas la latence). Part de budget appliquée :
256 000 bps (`PART_DORMANTE_BPS`), `endormie=true`. Comportement conforme au
contrat documenté depuis D5/D6.

**Le réveil a bien eu lieu, 471 ms après l'ordre sur le run initial (pas
10,2 s — voir le tableau), et trois lignes le confirment dans la fenêtre de
30 s du run initial** — la première rédaction disait le contraire :

```
21:05:56.244  contrôle reçu Visibility { visible: true }
21:05:56.715  fenêtre réveillée session=w-2 … duree_ms=454
21:05:56.719  part de budget appliquee session=w-2 part_bps=2000000 endormie=false
21:06:05.198 / 21:06:15.207 / 21:06:25.214  cadence du capteur session=w-2 … endormie=false
```

**Cause du faux négatif, dans l'instrument, corrigée (B1)** :
`cadencesAgent` (parseur de journal du pilote) extrayait la session par
`champ(l, 'session') ?? sessionDeLigne(l)`. Sur une ligne portant le span
`fenetre{session=w-2}:`, `champ()` matche la **première** occurrence de
`session=` — celle DANS le span — et son motif `\S+` capture `w-2}:`
(accolade et deux-points compris, non whitespace). Cette valeur fausse est
**truthy**, donc `?? sessionDeLigne(l)` — qui aurait rendu la valeur correcte —
ne s'évalue jamais. `eveillees` contenait alors des entrées `"w-2}:@…"`, et le
prédicat `e.startsWith('w-2@')` en aval ne pouvait **jamais** être vrai. Le
sommeil, lui, passait grâce à un repli sur `partsAgent` (dont les lignes
n'ont pas de span) — masquant le défaut plutôt que le révéler. **C'est le
piège « vérifier qu'un contrôle peut réussir » rejoué une troisième fois, sur
la fonction même dont le commentaire se félicitait d'avoir déjà corrigé ce
prédicat.** Remède : `sessionDeLigne(l)` seul, comme dans tous les autres
parseurs du fichier.

**Corroboré par le rejeu**, avec l'instrument corrigé : réveil confirmé par
le pilote en 10,2 s (délai de confirmation, voir ci-dessus — la latence
AGENT réelle est de 119 ms, tableau ci-dessus), les trois sessions présentes
portant chacune une ligne `endormie=false` postérieure à l'ordre de réveil.

**2 exécutions, les deux moitiés (sommeil et réveil) tenues aux deux, sur les
latences AGENT — jamais les délais de confirmation du pilote.**

## Critère ⑤ : TENU (une fois la bonne session ciblée)

**CORRIGÉ EN TOTALITÉ (Important 4 de la revue).** La première rédaction
écrivait « 409 Hz — la tonalité assignée à `w-1` » : **c'est une erreur, il n'y
avait aucun référent.** `hzDe(n)` assigne les fréquences aux fenêtres 1, 2, 3
ouvertes PAR le pilote — c'est-à-dire, sur le run initial, aux sessions
`w-4, w-6, w-8` (voir « Fenêtres préexistantes » ci-dessus). `w-1` et `w-2`
n'ont **jamais** reçu d'assignation de fréquence : ⑤ avait été mesuré sur une
fenêtre dont le contenu réel était inconnu.

**Deux pièces, versées dans le run initial mais non exploitées, le
confirmaient déjà :**

- **`kbps_audio` du témoin** (`recette-recette.json:36-92`) : `w-1` 128,8 ·
  `w-2` 128,8 · `w-4` **0** · `w-6` **0** · `w-8` 128,8. **Cette pièce ne
  soutient PAS proprement l'attribution « groupe de PID » que la première
  correction en tirait** : `w-8` est ELLE AUSSI une fenêtre ouverte par le
  pilote (fenêtre 3, hz=630) et porte pourtant 128,8 kbps d'audio, exactement
  comme les deux préexistantes — la ligne nette « fenêtres du pilote sans
  audio / préexistantes avec audio » ne survit pas à ce contre-exemple.
  L'attribution à l'arbitrage par groupe de PID de D7 reste **plausible pour
  `w-4`/`w-6` spécifiquement**, mais elle n'est plus présentée ici comme
  acquise : `w-8` n'a pas d'explication versée dans ce document.
- **Le seuil `audio_survit` du pilote ne peut quasiment pas échouer** : il
  compare le niveau à la fréquence assignée au **plancher de bruit**
  (`pilote-recette-d8.mjs:1205-1206`), pas à la dominante — voir « Pièges
  neufs » plus bas, où ce défaut d'instrument est maintenant consigné à la
  même place que les autres. Sur le run initial, le niveau à 520 Hz
  (`-115 dB`) était 79 dB **sous** la dominante mesurée (409 Hz, `-36 dB`) —
  indiscernable d'une fuite spectrale au seuil employé. Le pilote donnait les
  deux nombres sans en tirer la conclusion qui s'imposait.

**Le rejeu, ciblant la session correctement résolue (`w-4`, la vraie fenêtre 2,
hz=520), tranche sans ambiguïté :**

```
"hz": 522, "db": -40, "niveaux": [{"f": 520, "db": -40}]
```

**La fréquence DOMINANTE reçue (522 Hz) est désormais celle assignée (520 Hz,
à la résolution du bin FFT près)** — le pic dominant EST le pic à la
fréquence attendue, pas un signal 79 dB en dessous de lui. **Ceci confirme
que l'anomalie du run initial était une mauvaise identification de session,
pas une fuite d'isolation audio ni un défaut du produit.** Le fait de
conception D7 (une fenêtre qui partage le groupe de PID d'une application en
entend le mélange entier) reste vrai en soi, mais il n'est **plus** la
lecture qui explique cette mesure — il ne s'applique qu'aux DEUX fenêtres
préexistantes entre elles, hors du périmètre de ⑤.

**1 exécution corrigée, tient sans réserve : une session endormie continue de
recevoir l'audio de SA propre fenêtre.** (La mesure initiale n'est pas
comptée comme une exécution valide de ⑤ : elle portait sur la mauvaise
session.)

## Les trois inconnues du brief : AUCUNE N'EST TRANCHÉE

*Section jugée la meilleure du document initial — conservée presque à
l'identique, seule la clause finale (méthode) est mise à jour pour refléter
ce que le rejeu a effectivement vérifié.*

Le brief posait trois inconnues comme risque n°1 de ce sous-bloc. **Les deux
exécutions ne les tranchent pas**, faute d'avoir sollicité le mécanisme
qu'elles interrogent — voir critère ② ci-dessus.

1. **Le pilote SudoVDA accepte-t-il un changement de mode sur une sortie dont
   la duplication est ouverte ?** NON TRANCHÉE. Zéro tentative de changement de
   mode a eu lieu pendant que des duplications étaient ouvertes, aux deux
   exécutions — la question posée par le brief reste **exactement aussi
   ouverte qu'avant cette recette**.
2. **Combien de pertes d'accès `0x887a0026` un changement de mode inflige-t-il
   aux voisines ?** SANS OBJET. `pertes_acces_voisines: []` aux deux
   exécutions reflète l'absence de toute tentative, **pas** l'absence de
   pertes lors d'un changement réel. Les pertes de mutex relevées (6 au run
   initial, 1 au rejeu) sont toutes des réouvertures de routine à l'ouverture
   des captures, sans rapport avec un changement de mode.
3. **La sortie garde-t-elle son nom `\\.\DISPLAYn` à travers le changement ?**
   SANS OBJET pour la même raison, aux deux exécutions. Ne pas lire
   `conserve: false` comme un refus : c'est l'absence de mesure qui le rend
   `null`/`false`, pas un changement de nom observé.

**Ces trois inconnues restent le premier travail d'une prochaine recette.**
Le correctif de méthode proposé initialement (« vérifier par une trace côté
page que le viewport pris effet ») **a été appliqué par le rejeu, et il a
rempli son rôle** : il a permis d'écarter une fausse piste (le plafond
d'écran émulé) et de localiser la rupture plus précisément (entre
`window.innerWidth` et l'émission du `Resize`). **Il reste à instrumenter
`video.clientWidth`/`clientHeight` directement**, pour savoir si c'est là ou
plus en aval (le `ResizeObserver` lui-même, son verrou de 200 ms, ou le canal
de contrôle) que la chaîne casse.

⚠️ **Et le CANAL DE CONTRÔLE reste dans cette liste de suspects, contrairement
à ce que la ronde précédente laissait croire** (C3, revue finale de branche) :
la pièce qui prétendait le disculper est réfutée par les journaux — **zéro
ligne `contrôle reçu` pendant les 141 s de la phase ②**, aux deux exécutions.
Le classer en dernier « pour mémoire » serait reprendre une conclusion sans
sa prémisse. **Aucun maillon n'est disculpé par cette recette.**

## Ce que D8 n'établit pas

- **RIEN N'EST DISCULPÉ dans la chaîne qui casse à ②** — ni le client, ni le
  canal de contrôle, ni l'agent. La pièce qui prétendait disculper le transport
  est réfutée par les journaux (C3, revue finale de branche) : **zéro ligne
  `contrôle reçu` pendant les 141 s de la phase ②**, aux deux exécutions.
- **Le changement de mode de sortie n'a JAMAIS tourné en conditions de
  produit**, et il est **désarmé par défaut** depuis la revue finale de branche
  (voir la section en tête). Son verdict n'est donc ni « il marche » ni « il ne
  marche pas » : **il n'a pas été essayé.**
- **Aucun taux nulle part** : une exécution complète (témoin, ①, ④) et une
  seconde ciblée (②, ⑤, plus corroboration de ① et ④). Aucune ligne de ce
  document ne porte de fréquence de succès.
- **③ n'est pas mesuré**, et sa raison est mesurée elle-même (P2, sonde
  d'instrument) : Chrome `--headless=new` n'entre pas réellement en plein
  écran et n'expose pas `navigator.keyboard`.
- **Le cas HiDPI reste structurellement invisible à ce montage** : le pilote
  tourne avec `deviceScaleFactor: 1` partout, et le défaut déjà documenté (le
  garde-fou anti-changement-de-mode-parasite ne tient qu'à
  `devicePixelRatio == 1`) n'a donc pu ni se manifester ni être réfuté ici.
- **La visibilité ET le focus sont IMPOSÉS par le pilote, page par page**
  (garde-fou 2, hérité de D5/D6) — un Chrome sans interface rapporte
  `document.hidden = true` pour toute fenêtre d'arrière-plan. C'est la limite
  la plus lourde du montage : aucune minimisation de vraie fenêtre, aucun
  focus par clic réel. **Absente de la première rédaction, ajoutée ici.**
- **Rien de la latence de bout en bout**, qu'aucun sous-bloc du chantier D n'a
  mesurée à ce jour.
- **Les trois couches inconnues du chantier D restent inconnues** : le plafond
  de 8 encodeurs, celui de 4 processus, et le mécanisme de l'abandon du mutex
  DXGI. Cette recette n'en a rencontré qu'une conséquence déjà documentée
  (pertes `0x887a0026` de routine, toutes encaissées, aux deux exécutions).
- **`video.clientWidth`/`clientHeight` n'a pas été instrumenté** (voir ②) :
  la localisation de la rupture entre le viewport CDP et l'émission du
  `Resize` s'arrête à `window.innerWidth`.
- **Un seul rang (N=3 fenêtres)** a été joué aux deux exécutions ; rien
  au-delà, rien sur le recouvrement, le déplacement ou le redimensionnement
  manuel d'une fenêtre.
- **`Xvfb` n'a pas été installé** pour cette recette, aux deux exécutions —
  confirmé explicitement (§ critère ③) : aucune mesure de ce document ne
  s'appuie sur lui.
- **Aucun journal séparé par critère** : le pilote produit un journal combiné
  par exécution (`critere-recette.log`, `critere-rejeu-2-5.log`), pas les
  `critere-{1..5}-*.log` que le cahier des charges nommait. Ce n'est pas une
  omission silencieuse — elle est déclarée ici : le pilote (tâche 10) est
  monolithique par construction (une seule fonction `main()`, phases
  enchaînées), et le scinder pour produire cinq fichiers aurait dépassé le
  périmètre d'une recette qui ne doit pas réécrire l'instrument sans raison.

## Réserves héritées, reprises sans les remesurer — corrigées sur trois points

- **La mesure F1 (P0) porte une seule exécution et une bascule de porteur en
  cours de mesure** — non rejouée ici, reprise telle quelle depuis la tâche 2.
- **CORRIGÉ (Important 6)** : la première rédaction citait « les deux
  occasions où P1 a tourné (tâche 3 et l'exécution de préparation) » comme
  preuve que `CDS_UPDATEREGISTRY` seul suffit. **Le verdict P1 de la tâche 3
  est INVALIDE** — c'est l'objet même du commit `7032b01`
  (« P1 rendait RECU sans valeur, le critere ne pouvait pas echouer ») : son
  journal (`p1-mode-sortie.log`) ne porte pas le champ
  `largeur_avant_tentative`, signature de la version d'AVANT le correctif.
  **Les occasions valides sont** : `p1bis-mode-sortie.log` (tâche 3bis,
  cible 3840×2160 → « snap » à 2560×1440, `cible_exacte_atteinte=false`,
  mais `CDS_UPDATEREGISTRY` seul a suffi à produire un mouvement) et
  l'exécution de préparation de cette tâche (`mode-sortie-1280x720-preparation.log`,
  cible 1280×720, atteinte exactement). **Une troisième pièce, invoquée nulle
  part par la première rédaction, porte le refus net d'un mode non annoncé**
  (`mode-sortie-1728x1080.log` — **CORRIGÉ (re-revue) : attribué à tort à la
  tâche 3bis, c'est le commit `d5ce288` de la TÂCHE 9**
  (`git log --diff-filter=A`, et `task-9-report.md` §B) : cible 1728×1080,
  hors des neuf modes annoncés, `verdict="P1 REFUSE"`, `code=-2`
  (`DISP_CHANGE_BADMODE`) — c'est la preuve directe de la conséquence sur les
  rapports d'aspect non-16:9 que le brief demande de porter comme fait
  établi, absente de la première rédaction. **Les neuf modes annoncés**
  (`p1-mode-sortie.log`, `p1bis-mode-sortie.log`, `mode-sortie-1728x1080.log`,
  identiques aux trois occasions) : 640×360, 800×600, 960×540, 1280×720,
  1366×768, 1600×900, 1920×1080, 2560×1440, 3840×2160.
  ❌ **CORRIGÉ (I6, revue finale de branche) : « huit en 16:9 exactement » est
  FAUX — il y en a SEPT.** Le décompte juste est **7 en 16:9 exact**
  (640×360, 960×540, 1280×720, 1600×900, 1920×1080, 2560×1440, 3840×2160),
  **1 en 4:3** (800×600), et **1 qui n'est NI l'un NI l'autre** : **1366×768**
  (1366 × 9 = 12 294 ≠ 768 × 16 = 12 288). Le mot « exactement » interdisait
  de le lire comme une approximation, et c'est bien la seule lecture qui
  sauverait la phrase. **La conclusion aval survit intacte** : un client dont
  le viewport de plein écran a un rapport d'aspect ni 16:9 ni 4:3 (16:10,
  3:2 — les formats d'ordinateurs portables les plus courants) n'a AUCUN mode
  dans cette liste qui le serve, et le refus net serait systématique.
- ❌ **CORRIGÉ (I5, revue finale de branche) : « les deux combinaisons de repli
  restent non exercées » est FAUX — elles LE SONT, dans un journal que ce
  document cite lui-même.** `mode-sortie-1728x1080.log` porte les trois
  combinaisons, jouées :

  | Combinaison | Code | `api_annonce_succes` | Sortie relue | `conforme` |
  | --- | --- | --- | --- | --- |
  | `CDS_UPDATEREGISTRY` seul | **−2** | `false` | 1920×1080 (inchangée) | `false` |
  | `CDS_UPDATEREGISTRY \| CDS_RESET` | **−2** | `false` | 1920×1080 (inchangée) | `false` |
  | `CDS_UPDATEREGISTRY\|CDS_NORESET` puis `CDS_RESET` seul | `code_premier_appel=-2`, **`code_second_appel=0`** | **`true`** | 1920×1080 (inchangée) | `false` |

  **L'énoncé juste** : les deux replis **ont été exercés, et aucun n'a jamais
  fait bouger une sortie** — sur les occasions valides ci-dessus,
  `CDS_UPDATEREGISTRY` seul a toujours suffi quand quelque chose bougeait.
  ⚠️ **Et la troisième ligne est le fait le plus important du tableau : c'est
  un REFUS DÉGUISÉ EN SUCCÈS.** L'API annonce `0` sur une sortie qui n'a pas
  bougé d'un pixel — exactement ce contre quoi ce dépôt a bâti sa doctrine
  « juger sur la relecture DXGI, jamais sur le code de retour ». **Le produit
  n'est pas affecté** (il n'emploie que `CDS_UPDATEREGISTRY` seul, et juge sur
  la relecture), mais un successeur qui adopterait l'idiome multi-écran sur la
  foi de la phrase corrigée ici **croirait avoir réussi**.
- **CORRIGÉ (Important 8, point 4)** : la première rédaction disait la
  naissance d'une sortie à la dernière taille du registre « probable ».
  **Elle est CONFIRMÉE ET REPRODUITE**, pas supposée : la tâche 3bis
  établit la chaîne `avant(N) = après(N-1)` sur trois transitions
  consécutives (`task-3bis-report.md`, § « Persistance au registre ») — pas
  une lecture du commentaire de code, une mesure.
  ⚠️ **Réserve de la tâche 3bis, NON TRANSPORTÉE jusqu'ici et rétablie
  (revue finale de branche)** : ces trois transitions reposent sur **UN SEUL
  journal brut versé**, donc **une exécution**. `task-3bis-report.md` porte sa
  propre réserve en toutes lettres — trois exécutions au total, dont l'une
  « non versée en brut (nettoyée par erreur avant copie) », son contenu étant
  reproduit à plat dans le rapport. **La chaîne est cohérente et reproduite,
  elle n'est pas rejouable sur pièces.** Aucun taux, ici comme partout.
- **Le défaut HiDPI reste ouvert côté client**, non remesuré (voir
  ci-dessus).

## Pièges neufs à connaître avant de retoucher ce terrain

- **Combiner deux variables `MULTIFENETRE_*` dans le même lancement n'enchaîne
  PAS deux sondes** : l'aiguillage retourne après la première reconnue. Une
  variable de préparation (`MULTIFENETRE_VDD_PURGE`) et une variable de mesure
  (`MULTIFENETRE_MODE_SORTIE`) doivent être deux lancements séparés, même si
  leur ordre relatif de priorité est documenté dans le code.
- **Un pilote qui se termine en erreur, OU qui laisse un superviseur vivant en
  fin d'exécution normale, bloque silencieusement la tentative suivante.**
  Rencontré trois fois sur trois lors de ce sous-bloc. Le `finally` du pilote
  ne tue que le Chrome hôte, jamais le superviseur/capteur côté VM. Remède
  appliqué dans l'instrument (B2) : `purgerFenetresVMRescapees()` en tête de
  `main()`, qui tue tout `chrome.exe` **et vérifie** (`agents_restants`,
  `chrome_restants`) plutôt que suppose — mais elle ne tue PAS les processus
  `agent` eux-mêmes, seulement les fenêtres Chrome : `Get-Process agent`
  reste à vérifier manuellement entre deux invocations du pilote, comme
  rencontré à trois reprises pendant cette même tâche.
- **Une pollution de registre laissée par une sonde de mesure peut bloquer le
  produit, pas seulement fausser une sonde future.** Le défaut F1 documenté
  pour P1 (« persistance probable au registre ») s'est manifesté ici comme un
  blocage total de l'attachement de fenêtre — remède : la sonde P1 elle-même,
  en préparation.
- **Le nombre de sessions détectées par le superviseur n'est PAS la
  multiplication d'une seule fenêtre (contrairement à ce qu'une première
  lecture, calquée sur le précédent Paint/Bloc-notes, suggérait) — c'est le
  signe de fenêtres PRÉEXISTANTES non nettoyées.** Ces deux causes produisent
  la même observation de surface (plus de sessions que de fenêtres ouvertes
  par le pilote) et ne se distinguent QUE par l'horodatage relatif entre la
  création des sessions en trop et la première ouverture du pilote. **Ne
  jamais conclure sans comparer ces deux horodatages.**
- **Résoudre cible/voisine par rang de nom (`noms[0]`, `noms[1]`) n'est pas
  fiable, MÊME sur une VM nettoyée au préalable** — rien ne prouve qu'un
  doublon ne puisse survenir pour une autre raison. Remède appliqué (B3) :
  `resoudreIdentite()`, qui réutilise le mécanisme même de ① (bascule de
  bordure + observation de la session qui l'annonce) comme balise
  d'identité, avant toute mesure qui en dépend.
- **`window.__pleinEcran` (tampon client du pilote) n'est jamais borné dans le
  temps** : toute bascule de bordure antérieure (y compris une balise
  d'identité) laisse une trace que `messagesVoisines` relit sans filtrer par
  horodatage, produisant un faux positif de « fuite » si une autre bascule a
  eu lieu plus tôt sur la page lue. Découvert par le rejeu (①), non corrigé
  dans l'instrument.
- **Le seuil `audio_survit` du pilote ne peut quasiment pas échouer**
  (`pilote-recette-d8.mjs:1205-1206` :
  `(niveaux[0].db ?? -1000) - (plancher_db ?? 0) >= 20`, avec
  `plancher_db = -158` sur les mesures de ce sous-bloc) — il compare le
  niveau à la fréquence assignée au PLANCHER de bruit, jamais à la
  dominante. Le verdict ⑤ de ce document ne tient PAS grâce à ce seuil : il a
  fallu le déplacer sur la dominante (`hz`/`db` du pic, comparée à la
  fréquence assignée) pour que la mesure soit discriminante. Le seuil du
  pilote reste sans valeur de détection et n'a pas été corrigé — ouvert.
- **`verdict_partie_mesurable` (champ du JSON de ①, présent dans les DEUX
  exécutions — `recette-recette.json` et `recette-rejeu-2-5.json`) n'est
  mentionné NULLE PART dans ce document corrigé.** Il vaut `false` aux deux
  runs (calculé sur `detection_agent && messages_navigateur_cible &&
  fuite_vers_voisine.length === 0` — et `fuite_vers_voisine` porte le faux
  positif du point précédent au rejeu). Un lecteur qui ouvrirait le JSON
  seul, sans ce document, y lirait `false` là où le texte dit ① CONFIRMÉ :
  le champ n'a pas été mis à jour pour refléter la lecture corrigée de la
  fuite, et ce document ne le signale pas. Ouvert, à corriger au prochain
  travail sur cet instrument (soit en bornant `fuite_vers_voisine` par
  horodatage, soit en annotant le champ lui-même).
- **`--ozone-override-screen-size` du Chrome hôte NE plafonne PAS
  silencieusement `Emulation.setDeviceMetricsOverride`** — hypothèse posée
  par la première rédaction sans preuve, RÉFUTÉE par mesure directe
  (`window.innerWidth`/`innerHeight`) sur le rejeu.

## Ce que la correction a changé dans l'instrument (parties B de la revue)

Quatre changements dans `pilote-recette-d8.mjs`, tous conservés pour tout
travail futur sur ce terrain :

1. **`cadencesAgent`** : l'extraction de session utilise désormais
   `sessionDeLigne(l)` seul (au lieu de `champ(l, 'session') ?? sessionDeLigne(l)`,
   dont le premier terme masquait systématiquement le second sur les lignes à
   span). Corrige le faux négatif de réveil de ④.
2. **`purgerFenetresVMRescapees()`** : tue tout `chrome.exe` sur la VM et
   RELIT le compte de processus (`chrome_restants`, `agents_restants`) avant
   de lancer le superviseur — l'« ÉTAPE 0 » vérifie désormais ce qu'elle
   annonçait sans le faire.
3. **`resoudreIdentite(marqueur, etiquette)`** : bascule la bordure de la
   fenêtre portant `marqueur`, observe QUELLE session l'annonce, restaure, et
   rend cette session — jamais un rang de nom. Employée pour `cible` et
   `voisine` avant ①+②/④+⑤.
4. **Instrumentation `window.innerWidth`/`innerHeight`** autour des deux
   appels `forcerViewport` de ②, versée dans le JSON
   (`inner_avant`/`inner_apres`, `inner_a_atteint_la_cible`).

Un cinquième changement, `REJOUER_2_ET_5=1` : shortcut de `main()` qui saute
le témoin (déjà TENU, non redépendant de l'identité résolue) mais laisse ①+②
et ④+⑤ intacts (non scindés, par prudence — scinder ①+② pour ne rejouer QUE ②
aurait risqué d'introduire un bug dans un code par ailleurs correct pour un
gain marginal). ① et ④ sont donc réexercés comme sous-produit du rejeu ; leurs
verdicts ne sont **pas** re-émis à partir de cette seconde exécution seule —
ils corroborent ceux déjà établis par la première.

## Fichiers

- Pilote, corrigé : `docs/superpowers/plans/journaux-multifenetres-d8/instrument/pilote-recette-d8.mjs`
- Exécution initiale : `critere-recette.log`, `agent-recette.log`, `recette-recette.json`
- Rejeu ②+⑤ (avec corroboration ①/④) : `critere-rejeu-2-5.log`, `agent-rejeu-2-5.log`, `recette-rejeu-2-5.json`
- Préparation (levée du blocage registre) : `mode-sortie-1280x720-preparation.log`, `dxgi-controle-preparation.log`
- Réserves héritées, réexaminées : `p1-mode-sortie.log` (invalide, tâche 3),
  `p1bis-mode-sortie.log` (valide, snap), `mode-sortie-1728x1080.log` (valide,
  refus net), `.superpowers/sdd/2026-08-04-multifenetres-plein-ecran/task-3bis-report.md`
  (confirmation de la persistance au registre)

Tous les fichiers ci-dessus sous
`docs/superpowers/plans/journaux-multifenetres-d8/`, sauf le dernier.
