# Sous-bloc D3 — retenir les sorties, et caractériser le plafond de concurrence : résultats

**Date** : 2 août 2026. **Tâche 13** (dernière) du plan
`docs/superpowers/plans/2026-08-02-multifenetres-plafond-concurrence.md`.
**Branche** : `chantier-multifenetres-d3`, base `935cbda`.
**Conception** : `docs/superpowers/specs/2026-08-02-multifenetres-plafond-concurrence-design.md`.
**Antécédent** : le sous-bloc D2, non reçu au sens de son critère
(`plans/2026-08-01-multifenetres-arrangement-dynamique-resultats.md` §7).

Pièces versées, toutes dans `docs/superpowers/plans/journaux-multifenetres-d3/`
— **37 fichiers suivis par git** (24 au premier niveau, plus 13 dans
`instrument/`). Les journaux d'agent portent les séquences ANSI
de `tracing` (`sed 's/\x1b\[[0-9;]*m//g'` pour les lire à plat) ; l'exception est
`agent-critere1.txt`, déjà mis à plat.

| Pièce | Ce qu'elle porte |
| --- | --- |
| `build-agent.log` | Compilation de l'agent sur la VM (cible MSVC) |
| `essai-fumee-1x1.log` | Essai de fumée de l'instrument (1 processus, 1 duplication) |
| `etat-initial.log`, `etat-final.log` | Topologie **depuis un processus neuf**, avant et après la campagne |
| `plafond-{1x8,2x4,4x2,8x1,4x1}-{1,2,3}.log` | **Les quinze exécutions de la campagne** |
| `agent-critere1.log` / `.txt` | Le journal d'agent de la recette du critère 1 (260 lignes) |
| `demonstration-critere1.log` | Sortie complète de l'instrument sur l'exécution retenue |
| `demonstration-critere1-execution-invalide.log` | La première exécution, invalide côté instrument, versée plutôt que rapportée |
| `etat-final-apres-critere1.log` | Contrôle de topologie **différé**, depuis un processus neuf |
| `instrument/` | `pilote-recette-d3.mjs` dans son état d'exécution, et les `.ps1` |

---

## 0. Le verdict

**① Critère 1 — TENU.** Sur une séquence où une fenêtre est condamnée à
répétition pendant que trois autres capturent, dans la fenêtre bornée
`09:06:44.639142Z` → `09:06:48.784516Z` : **0** réouverture de duplication
imputable à une relance d'enfant, **0** session saine perdue. Un **seul**
`sortie virtuelle créée id=257` et un **seul** `sortie virtuelle détruite id=257`
encadrent **quatre** `enfant lancé … sortie=\\.\DISPLAY8` : la sortie virtuelle
est bien **retenue** d'une relance à l'autre au lieu d'être détruite puis
recréée.

**② Critère 2 — TENU, et l'hypothèse H1 est confirmée.** Quinze exécutions,
cinq rangs × trois essais. `1x8`, `2x4`, `4x2` et `4x1` passent **3/3** chacun ;
`8x1` est refusé **3/3**, toujours à la sonde 4 (le **5ᵉ** processus), toujours
en `0x887a0022` (`DXGI_ERROR_NOT_CURRENTLY_AVAILABLE`). **Le témoin `1x8`
passe, donc H2 est réfutée.** Le plafond porte sur le **nombre de processus
concurrents tenant une duplication DXGI ouverte**, et il vaut **exactement 4**
sur cette VM.

**③ La décision d'arrangement, écrite avant la mesure, s'applique sans
arbitrage.** Ligne H1 de la règle du §3.5 de la conception : la contrainte
étant le nombre de processus, la **capture mutualisée** — le repli où le
superviseur tiendrait lui-même les N duplications dans son propre processus —
est **désignée pour D4** et **non implémentée** par D3 ; **`CAPACITE` passe de
8 à 4**.

**Ce que le verdict ② ne dit pas, et ne peut pas dire : quelle couche impose ce
plafond.** Windows, DXGI, le pilote NVIDIA, SudoVDA ou la virtualisation — la
campagne mesure sur quoi porte le plafond, pas ce qui le produit.

---

## 1. Ce qui a été exécuté, et ce qui a été écarté

### Exécuté

**Volet 1 — correctifs produit** (tâches 1 à 7, commits `935cbda..de428cf`),
parties **pures**, éprouvées sur l'hôte Linux :

- `enfant_mort` **conserve** `sortie_pilote` et `nom_sortie` au lieu de les
  `take()` ; `relancer_les_orphelines` les reporte dans l'entrée réinsérée ;
  un champ neuf `taille_sortie` mémorise la taille de la sortie retenue, pour
  savoir si elle convient au viewport que la page-shell annoncera à la relance ;
- `creer_sortie` (dans `boucle.rs`, là où vit le pilote) consulte d'abord la
  table : sortie retenue **à la taille demandée** (à 4 px près) → l'appel au
  pilote et l'attente de rattachement DXGI sont **sautés**, la fenêtre est
  reposée sur sa sortie, et l'enfant relancé ;
- les **trois** chemins qui retirent une entrée de la table rendent désormais sa
  sortie — `fenetre_disparue` (inchangé), l'abandon après `RELANCES_MAX`, et
  l'abandon d'une entrée figée en `AttendLeViewport`. C'était le risque
  principal du volet : une sortie oubliée sur l'un d'eux consommerait le vivier
  de dix jusqu'à l'arrêt du superviseur ;
- la **fuite de capacité** du §7.3 de D2 est fermée : `attente_depuis` est
  tamponné **paresseusement**, au premier passage de `relancer_les_orphelines`
  sur une entrée `AttendLeViewport` non tamponnée. `Table` reste pure — elle ne
  lit aucune horloge, `maintenant` lui étant passé en argument — et aucun
  appelant n'a bougé.

**Volet 2 — l'instrument** (tâches 8 à 10, commits `de428cf..cb6915a`) : le mode
`MULTIFENETRE_PLAFOND=<P>x<D>`, un processus **porteur** qui crée K = P×D sorties
virtuelles, bat le chien de garde du pilote, **ne duplique rien lui-même**, et
lance P processus **sondes** en escalier — la sonde *i+1* n'est lancée qu'une
fois la *i* déclarée prête ou en échec, par fichier témoin. Chaque sonde ouvre D
duplications, les **tient** jusqu'au signal d'arrêt, et journalise le succès ou
le `HRESULT` exact.

**Volet 2 — les mesures** (tâches 11 et 12, commits `cb6915a..2d836b6`) : la
recette du critère 1 sur la VM, puis la campagne de quinze exécutions.

### Écarté, explicitement

- **La capture mutualisée elle-même.** D3 la **désigne** ; il ne l'implémente
  pas. C'est une refonte structurante, et elle déplacerait le risque vers le
  plafond d'encodeurs en multi-processus, jamais relevé.
- Le **plafond d'encodeurs en multi-processus** — toujours non approché.
- **Latence, cadence, tenue dans la durée.**
- Le **chemin d'extinction propre** du superviseur.
- **`SendInput` global à la session Windows.**
- Les **points mineurs différés** du §7.4 de D2, sans lien avec les deux volets.

---

## 2. Le déroulé, avec son issue réellement observée

### 2.1 Le critère 1 — une fenêtre condamnée n'inflige plus de réouvertures

Rapport détaillé : tâche 11. Journal : `agent-critere1.log` / `.txt`,
instrument : `demonstration-critere1.log`.

**La séquence jouée.** Chrome sans interface lancé **avant** le superviseur
(contrainte devenue obligatoire, voir §2.3), page-shell connectée, VM vidée de
toute fenêtre éligible. Superviseur lancé sur un bureau **vide** — donc **le cas
produit, pas l'énumération initiale**. Trois Bloc-notes ouverts un par un,
chacun attendu avant le suivant : sessions `w-2`, `w-4`, `w-6`. Les PID du
superviseur et des trois enfants sains relevés et **protégés**. Puis un tueur
PowerShell lancé sur la VM, bouclant à 150 ms, qui tue immédiatement tout
processus `agent.exe` hors de cette liste ; puis la quatrième fenêtre ouverte.

**Ce qui s'est passé, relevé.** Le tueur a tué exactement quatre PID. Quatre
cycles lancement/mort se sont enchaînés — `w-8` (204 ms de vie), `w-9` (100 ms),
`w-10` (201 ms), `w-11` (101 ms) —, soit l'original plus `RELANCES_MAX = 3`
relances exactement, puis l'abandon.

**Le relevé du critère, borné par une fenêtre temporelle explicite** — jamais un
total de fichier, la leçon que D2 avait payée (44 dans la fenêtre, 50 en fin de
fichier) :

| Borne | Ligne | Horodatage |
| --- | --- | --- |
| Première — lancement du 4ᵉ enfant | 139 | `2026-08-02T09:06:44.639142Z` |
| Dernière — son abandon, sortie rendue au pilote | 179 | `2026-08-02T09:06:48.784516Z` |

Dans `sed -n '139,179p' agent-critere1.txt` : `accès à la duplication perdu,
réouverture` → **0** ; `sortie virtuelle créée` → **0**. Sur le fichier entier
(260 lignes) : `clôture de session amorcée` → **0**, et **4 sorties créées pour
4 détruites** — aucune fuite du vivier.

**La preuve de la rétention, et ce qui n'en est PAS une.** Chaque enfant relancé
affiche `sortie retenue pour la duplication … nom_sortie=\.\DISPLAY8`, mais cette
ligne est émise par l'**enfant** (`agent/src/capture/ouverture.rs`) et dit
seulement quelle sortie il a choisie — identique à la première ouverture comme à
chaque réouverture. La preuve est côté **superviseur**
(`agent/src/moniteurs_virtuels/pilote.rs`) : sur les 260 lignes il n'existe
qu'**une seule** `sortie virtuelle créée id=257` (l. 132) et qu'**une seule**
`sortie virtuelle détruite id=257` (l. 178), et elles **encadrent les quatre
lancements**. Ce point a été **revérifié indépendamment** à la rédaction du
présent document.

**Le chiffre large de l'instrument, réconcilié plutôt que tu.** L'instrument
publie `delta_reouverture: 6` sur une fenêtre plus large que celle du critère.
Les six se décomposent exactement : **3** réouvertures causées par la création
**initiale** de la sortie de la fenêtre 4 (l. 133-135, **avant** la borne
d'ouverture), et **3** causées par sa **destruction** au moment de l'abandon
(l. 180-182, **après** la borne de fermeture). Aucune des six ne tombe dans
[139, 179]. Le fenêtrage plus étroit est ce que le brief imposait ; le silence
sur le 6 aurait été une faute d'énoncé, et il a été corrigé en revue.

### 2.2 La campagne — la matrice, et ce qu'elle tranche

Rapport détaillé : tâche 12. Journaux : les quinze `plafond-*.log`.

**État de départ, depuis un processus neuf** (`etat-initial.log`) : ensemble des
noms de sorties attachées = `{\\.\DISPLAY1}`, la seule sortie physique. Aucune
orpheline.

| Rang | P × D | Duplications ouvertes au total | Essais | Issue |
| --- | --- | --- | --- | --- |
| **témoin** | 1 × 8 | 8 | 3 | **OK 3/3** |
| A | 2 × 4 | 8 | 3 | **OK 3/3** |
| B | 4 × 2 | 8 | 3 | **OK 3/3** |
| **contrôle** | 4 × 1 | 4 | 3 | **OK 3/3** |
| C | 8 × 1 | **4 au moment du refus** | 3 | **KO 3/3**, sonde 4 (5ᵉ processus), `0x887a0022` |

`topologie restaurée nom pour nom` aux quinze exécutions, y compris les trois qui
refusent.

**Le refus est un état atteint, pas une extrapolation.** Dans les trois essais de
`8x1`, la ligne `verdict reçu sonde=4` (le refus) **précède** toutes les lignes
`arrêt demandé, relâchement des duplications` des sondes 0 à 3 — vérifié
indépendamment à la rédaction : dans `plafond-8x1-1.log`, refus à
`08:31:51.374836Z`, premier relâchement à `08:31:51.452385Z`. Au moment où la 5ᵉ
sonde échoue, les quatre premières **tiennent encore** leur duplication. L'état
« 5 processus concurrents, dont 4 déjà en succès » a donc été **atteint et
refusé**, 3 fois sur 3.

**Le fait le plus tranchant du sous-bloc, et il ne doit pas se perdre.** Le rang
qui échoue (`8x1`) n'a que **4** duplications ouvertes au moment du refus, quand
des rangs qui **réussissent** (`1x8`, `2x4`, `4x2`) en ont **8**. Le rang qui
échoue en a donc **moins** que les rangs qui passent. Ce n'est pas une absence de
corrélation : c'est une **exclusion positive** du nombre total de duplications
comme cause, puisque la cause présumée est absente à l'instant précis de
l'échec. Et `4x2` et `8x1` créent en outre le **même** nombre de sorties
virtuelles — `nombre=9 attachees=9` dans les deux, soit 8 virtuelles plus la
physique : entre ces deux rangs, **seul le nombre de processus diffère**.

**H2 est réfutée par le témoin.** H2 prédisait un refus à la 5ᵉ duplication du
rang `1x8`, au motif que le plafond frapperait dès que le **créateur** des
sorties est un autre processus que le duplicateur. Dans `1x8`, le porteur crée
les huit sorties et une sonde **distincte** ouvre les huit duplications : c'est
exactement la configuration que H2 déclarait fatale, et elle passe 3/3.

**H3 n'a pas eu à être éprouvée, et l'escalade prévue n'a pas été jouée** — le
rang C refuse bien sur la sonde **minimale**, donc le symptôme est reproduit sans
encodeur ni fenêtre ni WebRTC. ⚠️ **Précision à ne pas omettre** : la sonde dite
« minimale » porte **inévitablement** un `ID3D11Device` avec
`SetMultithreadProtected(true)`, `DuplicateOutput` l'exigeant. Le **premier
étage** de l'escalade prévue si H3 s'était vérifiée — « ajouter un périphérique
D3D11 » — est donc **déjà franchi par construction**. Un lecteur qui lirait
« sonde minimale » sans cette phrase croirait le périphérique D3D11 exclu du
montage : il ne l'est pas. Ce qui est exclu, c'est l'**encodeur**, la fenêtre, la
session WebRTC et le reste de l'empreinte de l'enfant réel.

### 2.3 Un changement de comportement au démarrage, assumé et à connaître

Le tampon paresseux d'`attente_depuis` (§7.3 de D2, fermé ici) s'applique aussi
aux entrées issues de l'**énumération initiale**, jusque-là délibérément
exemptées. Conséquence : **si la page-shell se connecte plus de
`DELAI_ATTENTE_VIEWPORT_MAX = 30 s` après le superviseur, les fenêtres
préexistantes sont abandonnées** — et une entrée abandonnée n'est jamais
reproposée, le hook ne réémettant pas d'événement pour une fenêtre déjà ouverte.

C'est cohérent avec le piège documenté depuis D1 (« lancer le navigateur AVANT le
superviseur »), c'était écrit dans la conception plutôt que découvert à la
recette, et **c'est ce qui a imposé le `preparer-vide.ps1` de la tâche 11** : la
recette démarre sur un bureau vide et ouvre ses fenêtres ensuite. Que l'entrée
abandonnée ne soit jamais reproposée est un **défaut préexistant** ; D3 ne le
corrige pas, il le nomme.

### 2.4 Le chien de garde du pilote — une donnée à reporter au chantier D

`intervalle_ping_max_ms`, plus grand écart entre deux battements du chien de
garde sur toute la conduite, **relevé** aux quinze exécutions :

| Rang | Essai 1 | Essai 2 | Essai 3 |
| --- | --- | --- | --- |
| 1x8 | 372 | 372 | 370 |
| 2x4 | 486 | 486 | 483 |
| 4x1 | 512 | 513 | 511 |
| 4x2 | 935 | 935 | 934 |
| 8x1 | 1012 | 1012 | 1012 |

Maximum observé : **1012 ms**. **L'écart croît avec le rang** (×2,7 de `1x8` à
`8x1`, monotone), et ce 1012 ms a été obtenu sur un escalier **arrêté à 5 sondes
sur 8** : un escalier qui irait au bout serait vraisemblablement plus haut
encore — *inférence, pas relevé*. Rien de tout cela n'est alarmant en soi, mais
**l'unité du `delai = 3` du pilote reste inconnue et aucune n'est exclue, pas
même la seconde** : c'est à ce titre que la tendance mérite d'être reportée.

---

## 3. Ce qui a échoué

**Rien, au sens du critère de réception** : les deux critères sont tenus, et la
campagne a tranché entre H1, H2 et H3. Ce qui suit sont des échecs d'exécution,
tous corrigés avant tout relevé retenu, et versés plutôt que rapportés.

- **Première exécution de la recette du critère 1, invalide côté instrument.**
  `nodejs-winrm` enveloppe systématiquement la commande dans
  `powershell -Command "& { … }"` ; les guillemets doubles du tueur inline
  entraient en collision avec ceux de cette enveloppe. Dix erreurs de parsing en
  cascade, **zéro kill exécuté**, donc aucun abandon possible — un échec
  d'**instrument**, pas une observation sur le critère. Corrigé en écrivant le
  script sur le partage et en l'invoquant par `-File`, validé isolément, puis la
  séquence complète rejouée. La sortie de cette exécution invalide est versée
  (`demonstration-critere1-execution-invalide.log`).
- **Le serveur de signaling tournait depuis ~23 h sans `TURN_URL`** — le piège
  documenté dans `CLAUDE.md`. Tué par PID relevé (jamais `pkill -f`) et relancé
  avec `.env` sourcé, vérifié sur le processus qui **écoute réellement**.
  ⚠️ **Et le correctif n'a pas produit son effet** : `TURN_URL` est bien parvenu
  aux enfants, mais **aucune allocation TURN n'a abouti** pendant la recette —
  trois `WARN allocation TURN impossible` dans `agent-critere1.txt`. Non bloquant
  ici (agent et navigateur sur le même pont, les candidats `host` suffisent, et
  les quatre sessions se sont bien établies), **cause non investiguée** :
  possiblement `coturn` non démarré, ou un délai de 2 s trop court pour cette VM.
- **La VM a hiberné** entre la recette et son contrôle de topologie différé —
  nouvelle occurrence de la veille prolongée que `CLAUDE.md` documente sans
  l'expliquer. Conséquence sur la valeur de ce contrôle : voir §4.
- **Aucun échec pendant la campagne du plafond** : quinze exécutions strictement
  reproductibles par rang, aucun journal rejoué, aucune récupération manuelle,
  VM contrôlée après chaque rang.

---

## 4. Ce que cette campagne N'établit PAS

**Sur le plafond lui-même :**

- **La couche qui l'impose n'est pas identifiée** — Windows, DXGI, pilote
  NVIDIA, SudoVDA, virtualisation. Cette campagne mesure **sur quoi** porte le
  plafond, pas **ce qui** le produit.
- **Rien n'établit que 4 soit une borne du système.** C'est le point d'arrêt
  observé sur **cette** VM, dans **cette** configuration.
- **La VM est mono-GPU** : rien n'est su d'un éventuel plafond **par
  adaptateur**.
- **Il n'y a qu'une sortie physique** : aucune série virtuel/physique n'est
  possible ; au mieux un point isolé, et il n'a pas été pris.
- **Rien d'autres résolutions, d'autres fréquences, ni d'autres débits.** Toutes
  les sorties de la campagne sont à 1280×720 / 60 Hz.
- **Rien de la tenue dans la durée** : les sondes tiennent leurs duplications
  quelques secondes. La durée propre du journal d'agent va de **6,42 s** (`1x8`)
  à **9,69-9,70 s** (`8x1`) — relevée sur les quinze fichiers, et laissée à sa
  dispersion réelle plutôt qu'arrondie (9,686 / 9,697 / 9,699 s aux trois
  essais de `8x1`).
- **Un rang `5x1` autonome n'a pas été joué**, ni `6x1` ni `7x1`. L'état qu'un
  `5x1` aurait mesuré a bien été **atteint et refusé 3/3** comme sous-produit de
  `8x1` (voir §2.2) ; il manque néanmoins une mesure **dédiée** à ce rang.
- **Le comportement entre 6, 7 et 8 processus reste inconnu** : l'escalier
  s'arrête au premier refus, par construction de `conduire_les_sondes`.
- **Ni la destruction intercalée d'un processus, ni un ordre de montée
  différent** (démarrer par le 5ᵉ, par exemple) n'ont été exercés.
- **Le sens de « processus » n'est pas creusé** : la matrice oppose 1/2/4/8
  processus, jamais d'autres découpages — 8 **fils** dans un seul processus, par
  exemple, n'a pas été mesuré.
- **Aucune image n'est capturée ni encodée par les sondes** : elles ouvrent des
  duplications nues et les tiennent. **Aucun encodeur n'est construit** — donc
  rien n'est su ici du plafond d'encodeurs en multi-processus, qui reste
  entièrement ouvert.

**Nombre d'exécutions, précisément :**

- **Campagne** : **trois** par rang, cinq rangs, **quinze au total**. 3/3 est net
  et sans variation, **mais c'est trois essais, pas trente**.
- **Recette du critère 1** : **une seule** exécution retenue (plus une invalide,
  versée). **Aucun taux.** Le critère est tenu sur cette exécution ; rien ne dit
  qu'il le soit systématiquement.

**Sur la recette du critère 1 :**

- **Aucune image n'a été comptée.** « Les sessions saines tournent sans
  interruption » est correct sur ce que le journal établit — processus vivants,
  aucune `clôture de session amorcée`, ICE établi — mais le journal n'établit ni
  le flux de trames des trois sessions saines ni l'absence d'interruption de leur
  **affichage**. Leurs sorties ont bien perdu leur mutex à répétition sous
  l'effet des créations/destructions de la fenêtre 4 : `DISPLAY5` **4** fois,
  `DISPLAY6` **3**, `DISPLAY7` **2** — chacune rouverte du premier coup
  (`tentative=1` sur les neuf lignes). Formulation exacte : *processus vivants,
  ICE établi, neuf réouvertures réussies du premier coup ; aucune trame comptée.*
- **La condamnation ne frappe qu'un enfant tout juste lancé.** Aux quatre tours,
  la mort survient **100 à 204 ms** après le lancement, et `w-9` n'atteint même
  jamais la ligne `duplication de sortie établie` avant de mourir. **La mort d'un
  enfant qui capture depuis longtemps, en pleine diffusion, n'est pas exercée.**
- **Le contrôle de topologie final est affaibli par une hibernation
  intercalée.** Entre la recette (09:06-09:08) et le contrôle (09:34), la VM
  s'est éteinte et a redémarré — un cycle qui réinitialise le pilote et
  effacerait de toute façon une fuite. Ce contrôle prouve donc qu'**aucun
  artefact ne survit à long terme**, pas que le **chemin de libération** de
  l'agent a fonctionné. Cette preuve-là est ailleurs, et elle est antérieure au
  redémarrage : `sortie virtuelle détruite id=257` (l. 178) puis `sortie
  virtuelle rendue au pilote sortie_pilote=257` (l. 179).
- **Trois allocations TURN ont échoué** (§3) : la recette s'est jouée sans relais.
- **Le texte de l'abandon n'est pas tracé.** `la session n'a pas tenu après 3
  tentatives` n'existe **que** dans le message envoyé à la page-shell —
  `Effet::AnnoncerRefus` n'appelle aucun `tracing::`. Toute recette future qui
  voudrait le détecter par `grep` sur `agent.log` échouera ; lire le statut de la
  page-shell par CDP, ou s'appuyer sur `sortie virtuelle rendue au pilote`.
- **Aucun redimensionnement, aucun recouvrement, aucun déplacement de fenêtre**,
  et les fenêtres affichent du Bloc-notes statique — pas une application qui
  redessine, donc **aucune charge d'encodage réelle**.
- **Rien de la latence ni de la cadence**, ici non plus.

**Sur le code du volet 1 :**

- Il est éprouvé par **289 tests** qui passent sur l'hôte Linux, et par la
  recette en conditions de produit — mais les tests portent sur les parties
  **pures** de la table. Le chemin `creer_sortie` de `boucle.rs`, `#[cfg(windows)]`,
  n'est éprouvé que par la recette et par la compilation croisée (§5).

---

## 5. Pièges rencontrés

**Un acquis d'outillage qui dépasse ce sous-bloc — la compilation croisée
Windows.** Le dépôt dispose désormais d'une vérification **réelle** du code
`#[cfg(windows)]` sur l'hôte : mingw-w64 installé, et depuis `agent/`

```bash
cargo check --target x86_64-pc-windows-gnu
```

compile l'intégralité du code Windows. **Vérifié à la rédaction de ce
document** : sortie 0, **9 avertissements `dead_code` préexistants**, aucun dans
les fichiers neufs de D3. ⚠️ **Portée exacte** : cela couvre types, emprunts,
visibilités et durées de vie ; cela **ne couvre pas l'édition de liens**, la
cible réelle du projet étant `msvc` sur la VM. C'est néanmoins la levée, en
grande partie, de la réserve « non vérifiable sur l'hôte » qui pesait sur tout
le code `#[cfg(windows)]` depuis le début du projet.

**Les autres pièges, dans l'ordre où ils ont coûté :**

- **`nodejs-winrm` enveloppe toujours la commande** dans
  `powershell -Command "& { … }"` (`usePowershell=true` dans `scripts/winrm.js`).
  Un script inline portant des guillemets doubles entre en collision avec cette
  enveloppe, et **le symptôme est un script qui ne tourne jamais** — pas une
  erreur claire. Écrire le script sur le partage et l'invoquer par `-File`.
- **`scripts/run-agent.sh` ne transmet pas les variables neuves.** Piège payé
  trois fois maintenant : `SUPERVISEUR` en D1, `MULTIFENETRE_REPRISE` en D2,
  et il aurait été payé une troisième fois si `MULTIFENETRE_PLAFOND` n'avait pas
  été ajoutée **dans la même tâche** que le mode. Dans les trois cas l'agent
  démarre sans la variable **et sans rien signaler**.
- **Le code fourni par un brief peut ne pas compiler, et ce n'est pas au
  relecteur de le découvrir** : la tâche 8 a rencontré un `E0364` (réexport
  `pub(super)` excédant la visibilité de l'item) dans le code du brief, corrigé
  par l'implémenteur et confirmé en caisse jetable par la revue. La tâche 10 a
  rencontré une **signature fausse** dans son brief. **Signaler, corriger, ne pas
  recopier.**
- **Ne jamais interpoler une valeur d'environnement brute dans un composant de
  chemin.** Une correction de la tâche 9 avait introduit un nom de fichier de
  secours construit sur `MULTIFENETRE_PLAFOND_RANG` : `..` est significatif sous
  Windows, et l'écriture pouvait sortir de `%TEMP%`. Rattrapé à la ronde
  suivante — **une ronde de correction peut introduire une casse neuve**.
- **Une sonde qui ne dépose pas son verdict rend la mesure muette.** Le `?` posé
  avant le `fs::write` du verdict faisait qu'un rang malformé n'écrivait rien :
  exactement le cas que la trace existait pour révéler.
- **Le porteur hérite ses variables aux sondes**, et `MULTIFENETRE_VDD_PURGE` y
  aurait détruit les sorties du porteur **en pleine mesure**. L'environnement des
  enfants doit être **nettoyé explicitement**, pas hérité.
- **Ne pas se fier au compte absolu d'avertissements `clippy`.** Il dérive d'une
  exécution à l'autre selon la fraîcheur du build (99 puis 100 relevés par deux
  relecteurs différents) ; ce sont tous des `dead_code` dus au `#[cfg(windows)]`.
  **Vérifier la nature, jamais le nombre.**
- **Le `sleep` d'attente est un artefact de minutage.** Le brief prévoyait
  `sleep 45` par rang ; remplacé par un **polling sur deux lignes de fait**
  (`campagne du plafond — bilan` **et** `topologie (restaurée|NON restaurée)`),
  la durée réelle s'est révélée être de 6,4 à 9,7 s. Attendre le **fait**, jamais
  une durée.
- **`build-agent.log` reste mutilé** par le défaut d'encodage à deux réglages
  déjà documenté : `build-agent.sh` ne pose pas `[Console]::OutputEncoding`.
  Inoffensif ici (`Compiling agent` et `Finished release profile` restent
  lisibles), signalé et **non corrigé** — le script est partagé.
- **La fraîcheur du binaire n'est adossée à aucun horodatage de fichier versé.**
  Le `ls -l --time-style=full-iso` d'`agent.exe` demandé par le brief n'a pas été
  capturé. Elle se déduit **indirectement** : la trace `intervalle_ping_max_ms`,
  introduite par le commit `cb6915a`, figure dans les quinze journaux, et un
  binaire antérieur n'aurait pas pu l'émettre. **Cohérent, non prouvé par un
  horodatage direct.**

---

## 6. État de la VM à la fin

**Relevé à la rédaction du présent document :**

- `virsh list --all` → `61  Windows  en cours d'exécution` ;
- `/media/vm` réellement accessible (`ls /media/vm/dev`, pas `mountpoint`) ;
- **aucun processus `agent` survivant** (`(Get-Process agent …).Count` → `0`).

**Topologie.** Le dernier relevé **depuis un processus neuf** est
`etat-final-apres-critere1.log` (09:34:05Z) : ensemble des noms attachés
= `{\\.\DISPLAY1}`, **identique** à `etat-initial.log` (08:25:30Z). Le contrôle
de fin de campagne (`etat-final.log`) donnait déjà le même ensemble, vérifié par
`diff` d'ensembles triés — jamais par cardinal, un tiers pouvant ajouter une
sortie et compenser exactement un retrait. ⚠️ Voir §4 pour ce que le contrôle
différé **ne** prouve **pas**, une hibernation s'étant intercalée.

**La VM a hiberné une fois pendant le sous-bloc**, entre la recette du critère 1
et son contrôle différé. Elle a été redémarrée et a survécu à toutes les
séquences exploitées. La cause de ces veilles prolongées reste **non
identifiée** ; la règle prudente demeure : **vérifier que la VM a survécu après
toute séquence longue**, plutôt que se fier à une fenêtre horaire.

---

## 7. Ce qu'il reste à régler

### 7.1 La décision d'arrangement — la capture mutualisée, pour D4

**C'est la sortie de ce sous-bloc.** H1 étant confirmée, la contrainte est le
nombre de **processus** tenant une duplication. Un arrangement qui garde une
duplication par processus enfant est donc plafonné à **4 fenêtres**, quoi qu'on
fasse par ailleurs. La voie est de **mutualiser la capture** : un seul processus
tient les N duplications et distribue les textures.

**Ce que la campagne apporte à cette voie, et ce qu'elle ne lui apporte pas.**
Le rang témoin `1x8` est précisément la forme que prendrait la mutualisation —
**un** processus tenant 8 duplications, les sorties étant créées par un **autre**
processus (le superviseur) — et il passe **3/3**. C'est encourageant, et c'est
tout : ces sondes n'encodent rien, ne capturent aucune image, et ne construisent
aucun encodeur. **La mutualisation déplace le risque vers le plafond d'encodeurs
en multi-processus, qui n'a toujours jamais été approché** — et vers le partage
de textures entre le périphérique de capture et ceux des encodeurs, qui reste à
concevoir.

⚠️ **Une précision de vocabulaire, pour que D4 ne se trompe pas de repli.** Ce
que la conception de D3 nomme « le repli de la spec §8 » est la **capture
mutualisée**. Le §8 de la conception de **D2**, lui, décrit sous « gardé en
réserve » un mécanisme **différent** : *sérialiser* — le superviseur ordonne aux
enfants de relâcher, crée la sortie, leur dit de rouvrir. Les deux occupent la
même case « voie de repli » mais ne sont pas la même chose. **C'est la capture
mutualisée que D3 désigne**, et la sérialisation n'est plus nécessaire de toute
façon : la reprise de D2 encaisse déjà l'abandon de mutex.

### 7.2 `CAPACITE` est passée à 4 — et ce que cela ne règle pas

`agent/src/superviseur/boucle.rs` porte désormais `CAPACITE = 4`, documentée pour
ce qu'elle est : **une valeur mesurée sur cette VM, non prouvée être une borne du
système**. Le superviseur refuse donc immédiatement et proprement la 5ᵉ fenêtre,
au lieu de lui laisser brûler quatre tentatives vouées à l'échec.

**Ce que cela ne règle pas** : un refus reste un refus. L'utilisateur qui ouvre
une 5ᵉ fenêtre obtient un message, pas une fenêtre. **La cible de huit fenêtres
du chantier D reste hors de portée** tant que la capture n'est pas mutualisée.

### 7.3 Identifier la couche du plafond — toujours dû

D3 a répondu à *sur quoi porte* le plafond. **Il n'a pas répondu à *qui
l'impose*.** Aucun des quatre candidats — Windows, DXGI, pilote NVIDIA, SudoVDA
ou virtualisation — n'a été mis à l'épreuve. Si D4 mutualise la capture, la
question perd son caractère bloquant sans cesser d'exister : un plafond dont la
couche est inconnue peut se déplacer sous une autre charge.

### 7.4 Reporté tel quel des sous-blocs précédents

- Le **plafond d'encodeurs en multi-processus** — toujours non approché, et
  **il devient le risque n°1 de D4**.
- **Latence, cadence, durée** — rien n'a jamais été mesuré.
- Le **chemin d'extinction propre** du superviseur — jamais exercé.
- **`SendInput` global à la session Windows** — la réponse structurelle
  (injection ciblée par messages de fenêtre, ou un pilote) reste hors périmètre.
- **Qu'une entrée abandonnée ne soit jamais reproposée** (§2.3) — défaut
  préexistant, nommé et non corrigé.
- Les **points mineurs différés** du §7.4 de D2, écartés par D3.

### 7.5 Points mineurs différés par D3, à traiter à l'occasion

- `cargo fmt --check` signale 4 lignes dans `plafond.rs`, toutes dans du code
  repris du brief. **Le dépôt n'est pas `rustfmt`-clean par ailleurs.**
- `enfant.wait()` dans `SondesEnCours::drop` est **non borné** : une sonde morte
  mais dont le processus survivrait y bloquerait le porteur. **Forme
  préexistante**, reprise du modèle de `superviseur/lanceur.rs`.
- Le dépôt d'un verdict de sonde par `fs::write` laisse une **course résiduelle**
  (un lecteur pourrait voir un préfixe non vide) ; seul un *write-then-rename* la
  fermerait complètement.
- Deux sondes à rang illisible **collisionneraient** sur le fichier de verdict de
  secours, qui porte un nom fixe. Ne pas s'appuyer sur ce fichier pour attribuer
  une erreur de rang : la trace `tracing::error!` porte chaque rang brut et ne
  collisionne pas.
- `rendre_la_sortie_de` est une fonction **libre** placée avant l'`impl` ; une
  fonction associée près de son appelant serait plus lisible. *(La justification
  d'origine — « contrainte du langage » — était inexacte.)*
- `agent/src/superviseur/table/attribution.rs` : le vidage de
  `nom_sortie`/`taille_sortie` est **imbriqué** dans `if let Some(sortie_pilote)`,
  donc il dépend de l'invariant des trois champs **sans le dire**
  (`fenetre_disparue` porte, lui, la note). Le littéral `Effet::LancerEnfant` y
  est en outre dupliqué deux fois.
- `agent/src/superviseur/boucle/placement_periodique.rs` : la documentation dit
  que le chemin de création « n'émet aucun second `SetWindowPos` ». Si Windows
  **clampe** la taille, `doit_etre_replacee` sera vrai et un second sera émis.
  Écrire « n'émet **normalement** aucun ». **Formulation héritée du plan.**
- Le commentaire du tampon d'`attente_depuis` se corrige au paragraphe suivant :
  l'écart réel est borné à `PERIODE_PLACEMENT = 1 s`, donc rien n'est faux, mais
  **un lecteur qui s'arrête au premier paragraphe repart avec la lecture que la
  conception demandait d'exclure**.
- `agent/src/mire.rs:48` — un avertissement `clippy::manual_is_multiple_of`
  (`trame % 2 == 0` → `trame.is_multiple_of(2)`). **Préexistant** : le fichier
  n'est pas touché par cette branche.

---

## 8. Contrôle de la dette de taille de fichier

Commande de `CLAUDE.md`, lancée à la fin du sous-bloc :

```
1536 agent/src/encode.rs
 648 agent/src/windows_source.rs
 543 agent/src/wasapi.rs
```

**Les trois fichiers de dette gelée, et aucun de plus.** Aucun fichier neuf de D3
n'est au-dessus de 500 lignes : `plafond.rs` est à **412**, `sonde.rs` à **138**,
`table/attribution.rs` à **94**, `table/tests_retention.rs` à **282**,
`boucle/placement_periodique.rs` à **57**.

⚠️ **Deux marges à connaître avant de retoucher ce terrain :**

- **`agent/src/superviseur/table.rs` est à 490 lignes — marge de 10.** La
  conception l'annonçait à 471 ; le volet 1 y a ajouté un champ, une branche et
  sa documentation, soit +19. Les extractions ont bien eu lieu
  (`table/attribution.rs`, `table/tests_retention.rs`), et c'est **grâce à
  elles** que le fichier tient. **Toute addition future appelle une extraction de
  plus, pas une compression.**
- **`agent/src/superviseur/boucle.rs` est à 485 lignes — marge de 15.** La
  documentation de `CAPACITE` y a été condensée à la rédaction précisément pour
  ne pas serrer cette marge ; le détail vit ici, pas dans le code.
- **`agent/src/capture.rs` est à 496 lignes — marge de 4.** Il était à 485 à la
  fin de D2. **La leçon déjà écrite se vérifie une fois de plus : la marge
  regagnée par une extraction se reperd à la ronde suivante si on la traite comme
  acquise.**

Rappel des marges nulles déjà connues : `agent/src/encode/arret.rs` est à **500
lignes exactement**.

---

## 9. Vérification finale

Lancée à la rédaction de ce document, depuis `agent/` :

| Commande | Résultat |
| --- | --- |
| `cargo test --workspace` | **exit 0** — 289 tests (agent) + 27 (proto), 0 échec |
| `cargo clippy --workspace` | **exit 0** — 101 avertissements, dont **100 `dead_code`** dus au `#[cfg(windows)]` et **1 préexistant** (`mire.rs:48`, hors de cette branche) |
| `cargo check --target x86_64-pc-windows-gnu` | **exit 0** — 9 avertissements `dead_code` préexistants, **aucun dans les fichiers neufs** |

**Ne pas chasser le compte absolu d'avertissements `clippy`** : il dérive avec la
fraîcheur du build. Vérifier la **nature**.
