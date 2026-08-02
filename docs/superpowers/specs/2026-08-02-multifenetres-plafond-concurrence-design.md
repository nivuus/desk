# Sous-bloc D3 — retenir les sorties, et caractériser le plafond de concurrence

**Date** : 2 août 2026
**Chantier** : D (multi-fenêtres), sous-bloc 3
**Amont** : `plans/2026-08-01-multifenetres-arrangement-dynamique-resultats.md` §7
**Spec de chantier** : `specs/2026-07-28-support-jeux-design.md` §5 D

---

## 0. Ce que ce sous-bloc existe pour faire

D2 a réparé le défaut central de D1 — créer une sortie virtuelle ne tue plus les
captures en cours — mais il n'a pas été reçu au sens de son critère : la montée
s'arrête à **quatre** fenêtres simultanées, la 5ᵉ duplication DXGI étant refusée
en `0x887A0022` dans un 5ᵉ processus, par une limite de concurrence **dont la
couche n'est pas identifiée**.

Le §7 de ses résultats nomme précisément ce qui reste. D3 en prend trois points
et en écarte explicitement le reste :

| § de D2 | Objet | D3 |
| --- | --- | --- |
| 7.1 | Ne plus détruire puis recréer la sortie virtuelle à chaque relance d'enfant | **pris** |
| 7.2 | Le plafond de quatre — identifier la couche | **pris**, sous la forme d'une campagne discriminante |
| 7.2 bis | `CAPACITE = 8` face à un plafond mesuré de 4 | **pris** |
| 7.3 | Fuite de capacité d'une fenêtre neuve dont la page-shell ne répond jamais | **pris** |
| 7.4 | Points mineurs différés | **écarté** — sans lien avec les deux volets |
| 7.5 | Plafond d'encodeurs multi-processus, latence/cadence/durée, extinction propre, `SendInput` global | **écarté** |

---

## 1. Périmètre

Deux volets **indépendants** : ils ne partagent ni code, ni instrument, ni
terrain d'exécution.

| Volet | Objet | Terrain |
| --- | --- | --- |
| **1 — correctifs produit** | §7.1, §7.3, §7.2 bis | `agent/src/superviseur/` — parties **pures**, éprouvables sur l'hôte Linux |
| **2 — campagne discriminante** | Trancher sur quoi porte le plafond de quatre, puis écrire une décision d'arrangement | banc neuf + VM Windows |

Le volet 1 est du code testable **sans VM**. Le volet 2 exige la VM, et la VM
n'accepte qu'une tâche à la fois. Les deux peuvent donc être menés en parallèle,
à condition qu'une seule tâche à la fois touche la VM.

### Hors périmètre, explicitement

- **La capture mutualisée** (repli de la spec §8, où le superviseur tiendrait les
  N duplications dans son propre processus). D3 peut la **désigner** comme
  décision d'arrangement ; il ne l'implémente pas. C'est une refonte
  structurante, et elle déplacerait le risque vers le plafond d'encodeurs en
  multi-processus, jamais relevé.
- Le **plafond d'encodeurs en multi-processus**.
- **Latence, cadence, tenue dans la durée** — toujours jamais mesurées.
- Le **chemin d'extinction propre** du superviseur.
- **`SendInput` global à la session Windows**.

---

## 2. Volet 1 — les correctifs

### 2.1 §7.1 — retenir la sortie entre la mort d'un enfant et sa relance

#### Le défaut

Cycle actuel, dans `agent/src/superviseur/table.rs` :

1. `enfant_mort` bascule l'entrée en `Etat::SansSession`, fait
   `entree.sortie_pilote.take()` et émet `Effet::DetruireSortie` ;
2. `relancer_les_orphelines` retire l'entrée et la réinsère sous une session
   neuve avec `sortie_pilote: None` et `nom_sortie: None` ;
3. la relance repasse donc par `viewport_recu` → `Effet::CreerSortie` →
   `creer_sortie` (dans `boucle.rs`), **qui crée une sortie virtuelle**.

**Et c'est cette recréation qui abandonne les mutex des duplications voisines.**
Relevé de D2 : à l'étape 4 du passage D, une seule fenêtre condamnée fait passer
le compteur de réouvertures de **6 à 38**. Le réessai à l'ouverture posé en
tâche 11 bis n'en a supprimé **aucune** — aucun de ces échecs n'était un
transitoire, et aucun réessai ne peut rien contre cette cause-là.

#### Le changement

1. **`enfant_mort` conserve** `sortie_pilote` et `nom_sortie` au lieu de les
   `take()`, et n'émet plus que `Effet::AnnoncerFermeture`.
2. **`relancer_les_orphelines` reporte** ces champs dans l'entrée réinsérée,
   comme il reporte déjà `audio` et `relances`.
3. L'entrée mémorise en outre la **taille** de la sortie retenue — champ neuf
   `taille_sortie: Option<(u32, u32)>`, posé par `sortie_creee` en même temps que
   `sortie_pilote` et `nom_sortie`, et effacé avec eux. Sans lui, on ne saurait
   pas si la sortie retenue convient au viewport que la page-shell annoncera à la
   relance.
4. **`creer_sortie` (dans `boucle.rs`, là où vit le pilote) consulte d'abord la
   table** :
   - sortie retenue **à la taille demandée** → on **saute** l'appel au pilote et
     l'attente de rattachement DXGI ; on repose la fenêtre sur la sortie
     (`placement`), on réaffirme l'état par `table.sortie_creee(...)`, et on
     enchaîne `Effet::LancerEnfant` ;
   - sortie retenue **à une autre taille**, ou pas de sortie retenue → on détruit
     la retenue s'il y en a une, puis on crée, comme aujourd'hui.

La branche vit dans `boucle.rs` et non dans `table.rs` : le pilote y est déjà, et
`table.rs` reste une machine à états pure, éprouvable sur l'hôte.

**Pourquoi reposer la fenêtre même quand on réutilise la sortie** : entre la mort
de l'enfant et sa relance, l'application a pu déplacer ou retailler sa fenêtre.
Le contrôle périodique `controler_le_placement` le détecterait avec jusqu'à une
seconde de retard ; reposer coûte un appel et supprime la fenêtre d'incertitude.

#### La contrepartie, et c'est le vrai travail : trois chemins de destruction

Après ce changement, une entrée peut porter une sortie **dans l'état
`SansSession` et dans l'état `AttendLeViewport`**, ce qui n'était jamais le cas
auparavant. Trois chemins retirent une entrée de la table et devront donc rendre
sa sortie :

| Chemin | Aujourd'hui | Après |
| --- | --- | --- |
| `fenetre_disparue` | émet déjà `DetruireSortie` si `sortie_pilote` est `Some` | inchangé — et c'est désormais le cas nominal |
| abandon après `RELANCES_MAX` (`relancer_les_orphelines`) | n'émet rien : la sortie est toujours `None` à ce point | **doit émettre `DetruireSortie`** |
| abandon d'une entrée figée en `AttendLeViewport` | idem | **doit émettre `DetruireSortie`** |

**Une sortie oubliée sur l'un de ces chemins consommerait le vivier de dix
jusqu'à l'arrêt du superviseur.** C'est le risque principal du volet 1, et c'est
ce que les tests doivent couvrir en premier.

### 2.2 §7.3 — la fuite de capacité

Une fenêtre **neuve** dont la page-shell ne répond **jamais** reste en
`Etat::AttendLeViewport` sans être ni relancée ni abandonnée : ni `SansSession`
(elle ne l'est plus), ni `Vivante` (elle ne l'atteindra jamais). Elle consomme sa
place indéfiniment. Le garde-fou posé en D2 (`DELAI_ATTENTE_VIEWPORT_MAX = 30 s`)
n'a été branché que sur les entrées **relancées**, celles dont `attente_depuis`
est tamponné.

**Remède, celui que la revue de D2 avait nommé** : tamponner `attente_depuis`
**paresseusement**, au premier passage de `relancer_les_orphelines` sur une
entrée `AttendLeViewport` dont `attente_depuis` est `None`. `Table` reste pure —
elle ne lit toujours aucune horloge, `maintenant` lui étant passé en argument —
et aucun appelant ne bouge.

#### ⚠️ Un effet de bord à assumer, que la revue ne mentionnait pas

Ce tampon s'appliquera aussi aux entrées **issues de l'énumération initiale**
(`hook::enumerer_existantes`), aujourd'hui délibérément exemptées. Conséquence :
**si la page-shell se connecte plus de 30 s après le superviseur, les fenêtres
préexistantes seront abandonnées** — et une entrée abandonnée n'est jamais
reproposée, le hook ne réémettant pas d'événement pour une fenêtre déjà ouverte.

C'est cohérent avec le piège documenté en D1 (« lancer le navigateur AVANT le
superviseur »), mais c'est un **changement de comportement au démarrage**, et il
doit être écrit ici plutôt que découvert à la recette. Le délai court à partir du
premier passage de `relancer_les_orphelines`, cadencé par
`PERIODE_PLACEMENT = 1 s`, et non à partir du démarrage du superviseur.

**Qu'une entrée abandonnée ne soit jamais reproposée est un défaut préexistant**,
qui vaut déjà pour les entrées relancées. D3 ne le corrige pas ; il le nomme.

### 2.3 §7.2 bis — `CAPACITE` face au plafond mesuré

`CAPACITE = 8` (`agent/src/superviseur/boucle.rs:45`) est connu **supérieur au
plafond de concurrence mesuré, qui vaut 4**. Le superviseur accepte donc quatre
fenêtres (la 5ᵉ à la 8ᵉ) dont aucune ne peut aboutir ; chacune échoue au bout de
`RELANCES_MAX = 3` relances, soit quatre tentatives, et **chaque tentative
détruit puis recrée une sortie virtuelle** — exactement ce que le §2.1 supprime.

**Décision** : porter `CAPACITE` à la valeur du plafond **mesuré**, de sorte
qu'une fenêtre au-delà soit refusée immédiatement et proprement, au lieu de
brûler quatre tentatives vouées à l'échec.

Deux précisions qui font partie de la décision :

- la constante sera documentée pour ce qu'elle est — **un plafond mesuré sur
  cette VM, non prouvé être une borne du système** ;
- **sa valeur se fixe au retour du volet 2**, pas avant. Si la campagne montrait
  que le plafond porte sur autre chose que le nombre de fenêtres simultanées, une
  autre valeur, ou un autre mécanisme de refus, s'imposerait. D'ici là `CAPACITE`
  reste à 8.

C'est le seul point du volet 1 qui dépende du volet 2, et il en dépend
uniquement pour une valeur numérique.

---

## 3. Volet 2 — la campagne discriminante

### 3.1 Ce qu'on sait déjà, et ce qui ne s'en déduit pas

**Su, relevé** :

- 8 duplications DXGI de front **dans un seul processus** tiennent, à 1280×720
  sur 8 sorties virtuelles (31 juillet 2026,
  `plans/2026-07-31-duplications-paralleles-resultats.md`) — dans un montage où
  **le même processus créait les sorties et les dupliquait** ;
- la **5ᵉ** duplication, **dans un 5ᵉ processus**, est refusée en `0x887A0022`
  (D2), par une limite **durable** qui résiste à 3 s de patience explicite, et
  que libérer une place rend franchissable immédiatement.

**Non su** : quelle couche l'impose, si le plafond tient à d'autres résolutions
ou configurations, et **si 4 est une borne du système** — ce n'est que le point
d'arrêt observé.

**À ne pas confondre avec un acquis** : rapprocher ces deux relevés pour conclure
« le plafond porte sur les processus » est une **inférence**. Rien ne rapproche
formellement les deux montages, qui diffèrent d'au moins deux variables (nombre
de processus, et identité du créateur des sorties). La campagne existe pour
supprimer cette inférence.

### 3.2 L'instrument

Un mode neuf du banc : **`MULTIFENETRE_PLAFOND=<P>x<D>`**, exécuté par un
processus **porteur** qui :

1. relève la topologie depuis son propre démarrage, purge les sorties orphelines,
   puis crée **K = P×D** sorties virtuelles à 1280×720 — le vivier étant de 10,
   K ≤ 8 tient avec deux de marge ;
2. **bat le chien de garde du pilote** pendant toute la mesure, et **ne duplique
   rien lui-même**. C'est la position exacte du superviseur en D2, et c'est ce
   qui rend la mesure comparable au symptôme de produit ;
3. lance **P processus sondes** par `std::process::Command` avec `stdout` hérité,
   sur le modèle de `superviseur/lanceur.rs` — machinerie déjà éprouvée en D1 et
   D2, aucun outillage hôte neuf à écrire ;
4. **échelonne** les lancements : la sonde *i+1* n'est lancée qu'une fois la
   sonde *i* déclarée prête ou en échec, par fichier témoin déposé dans `%TEMP%`.
   Sans cet échelonnement, P sondes concurrentes rendraient un refus **sans rang
   identifiable**, et la matrice ne trancherait rien ;
5. à la fin, détruit ses sorties par la garde `moniteurs_virtuels::Sorties`,
   relève la topologie et la compare **par ensemble de noms** — jamais par
   cardinal, un tiers pouvant ajouter une sortie et compenser exactement un
   retrait.

La **sonde** est délibérément **minimale** : elle ouvre D duplications DXGI sur
les sorties nommées qu'on lui passe, journalise pour chacune le succès ou le
`HRESULT` exact, les **tient ouvertes** jusqu'au signal d'arrêt du porteur, puis
sort. Ni périphérique D3D11 d'encodage, ni encodeur, ni fenêtre, ni WebRTC.

Chaque ligne d'une sonde porte un champ `sonde=<i>` : `agent.log` mêle le porteur
et toutes ses sondes par héritage de `stdout`, et rien d'autre ne distinguerait
l'émetteur — piège relevé en D1.

**Pourquoi le porteur ne duplique pas** : une sonde peut mourir en `0xc0000005`,
comme toutes ces API. Le porteur, qui ne touche qu'au pilote, survit, et sa garde
détruit les K sorties. Un porteur qui dupliquerait risquerait d'emporter les
sorties avec lui.

### 3.3 La matrice, à K = 8

| Rang | P | D | Ce qu'il éprouve |
| --- | --- | --- | --- |
| **témoin** | 1 | 8 | Reproduit le point connu **OK** du 31 juillet sous le nouvel instrument — mais avec les sorties créées par un **autre** processus, ce que la mesure d'origine ne faisait pas |
| A | 2 | 4 | 8 duplications réparties sur 2 processus — rang intermédiaire entre le témoin et B |
| B | 4 | 2 | 8 duplications réparties sur 4 processus, soit le nombre de processus du plafond observé, mais deux fois plus de duplications |
| C | 8 | 1 | Doit refuser au **5ᵉ processus** si le plafond porte sur les processus |
| **contrôle** | 4 | 1 | Doit passer entièrement — borne inférieure du plafond, et témoin que l'instrument n'échoue pas de lui-même |

**Trois exécutions par rang, et non une.** Chaque rang dure quelques secondes ;
le reproche que ce projet se fait à chaque chantier — « une exécution par rang,
donc aucun taux » — se paie ici pour presque rien. Un rang qui passerait deux
fois sur trois est une information que la matrice doit pouvoir rendre, et qu'une
exécution unique effacerait.

### 3.4 Les hypothèses, et ce qui les tranche

- **H1 — le plafond porte sur le nombre de *processus* tenant au moins une
  duplication.** Prédit : témoin OK, A OK, B OK, C refusé au 5ᵉ processus.
- **H2 — il porte sur les duplications ouvertes par des processus autres que
  celui qui a créé les sorties.** Prédit : **témoin refusé à la 5ᵉ duplication**.
  C'est précisément ce que le témoin existe pour éprouver ; sans lui, H1 serait
  tenue pour acquise par le seul rapprochement avec le 31 juillet, qui est une
  inférence.
- **H3 — ce n'est pas la duplication, mais ce qui l'accompagne dans l'enfant**
  (périphérique D3D11 d'encodage, encodeur NVENC, empreinte du processus).
  Prédit : la sonde **minimale** ne reproduit pas le refus au rang C.

**Escalade bornée si H3** : épaissir la sonde d'un élément à la fois — d'abord le
périphérique D3D11, puis l'encodeur — **au plus deux fois**. Si le symptôme ne
revient pas, la campagne conclut « non reproduit par sonde minimale ni épaissie
deux fois », et le nomme comme tel. **C'est un résultat, pas un échec.**

### 3.5 La règle de décision, écrite avant la mesure

| Verdict | Décision d'arrangement que D3 écrit |
| --- | --- |
| **H1 confirmée** | La contrainte est le nombre de processus → la voie est la **capture mutualisée** (repli de la spec §8), **désignée** par D3 et **implémentée en D4**. `CAPACITE` passe à 4. |
| **Témoin refusé (H2)** | Le plafond est plus étroit qu'on ne croyait et frappe aussi le processus unique dès que le créateur des sorties est distinct → la mutualisation ne sauve rien, et le chantier D doit rouvrir sa cible de huit fenêtres. `CAPACITE` passe au rang du dernier succès du témoin. |
| **Rang C réussit** | Le plafond de D2 avait une **autre** cause que la concurrence de duplications — ordonnancement, charge, ou un état de la VM. À rouvrir avec les journaux de D2 en main, et `CAPACITE` **reste à 8** : rien n'autoriserait alors à la baisser. |
| **H3** | Le plafond est du côté périphérique ou encodeur, ce qui le raccorde au **plafond d'encodeurs en multi-processus**, jamais relevé. Il devient le sujet de D4. `CAPACITE` passe à 4, valeur du plafond observé en conditions de produit — sans que la campagne en ait identifié la couche. |

Dans les quatre cas, D3 rend une décision écrite. **Aucun de ces verdicts ne fait
échouer le sous-bloc** — c'est le sens du critère de réception retenu au §4.

### 3.6 Ce que la campagne ne pourra pas dire

- La VM est **mono-GPU** : rien ne sera su d'un éventuel plafond par adaptateur.
- Il n'y a **qu'une sortie physique** : la comparaison virtuel/physique ne peut
  pas être une série, au mieux un point isolé.
- **Apollo consomme le même vivier de dix.** L'état de départ doit être relevé
  **depuis un processus neuf** avant chaque rang, faute de quoi un refus de
  création se lirait comme un plafond de duplications.
- Rien ne sera su d'autres résolutions, d'autres fréquences, ni de la tenue dans
  la durée : les sondes tiennent leurs duplications quelques secondes.

---

## 4. Critère de réception

Deux critères **indépendants**, et **aucun n'est un nombre de fenêtres**. D2 a
échoué au sien parce qu'il portait sur un nombre gouverné par une couche que
personne n'avait identifiée ; D3 ne reproduit pas cette erreur.

**Critère 1 — volet 1.** Sur une séquence où une fenêtre est condamnée pendant
que d'autres capturent : **zéro réouverture de duplication imputable à une
relance d'enfant**, et **aucune session saine perdue**.

> Le compteur de réouvertures est déjà journalisé. Il se relève **fenêtré par
> bornes temporelles explicites**, jamais comme un total de fichier — le journal
> continue de courir après la séquence de critère, et D2 a payé cette confusion
> (44 dans la fenêtre, 50 en fin de fichier).

**Critère 2 — volet 2.** La matrice rend un verdict qui **tranche entre H1, H2 et
H3**, et la décision d'arrangement correspondante du §3.5 est écrite dans le
document de résultats.

**Un plafond qui résisterait à la mesure serait un résultat**, à condition que le
document dise précisément ce qui a été éprouvé et ce qui ne l'a pas été.

---

## 5. Risques et pièges connus

- **La VM se met en veille prolongée toute seule** — D1, cause non identifiée,
  intervalles observés de 70, 60 puis 50 minutes. Contrôler sa survie **après
  chaque rang**, pas à la fin de la campagne.
- **`scripts/run-agent.sh` ne transmet pas les variables neuves.** Piège payé
  deux fois : `SUPERVISEUR` en D1, `MULTIFENETRE_REPRISE` en D2. Dans les deux
  cas l'agent démarre sans la variable **et sans rien signaler**.
  `MULTIFENETRE_PLAFOND` doit y être ajoutée **dans la même tâche** que le mode.
- **`scripts/build-agent.sh` lancé sans avoir sourcé `.env` s'arrête en
  silence**, et le symptôme se lit exactement comme une compilation réussie et
  muette. Sourcer `.env` d'abord.
- **Un plantage de sonde laisse jusqu'à 8 sorties orphelines.** Le porteur ne
  duplique pas, précisément pour survivre et détruire. Purge de contrôle avant
  chaque rang, depuis un processus neuf.
- **`table.rs` est à 471 lignes**, plafond du projet à 500, tests déjà extraits
  dans un fichier voisin. Le volet 1 y ajoute un champ, une branche et de la
  documentation : **toute addition substantielle appelle une extraction**, pas
  une compression — la compression s'est déjà jouée sur ce terrain.
- **Ne pas tracer par duplication ni par image** dans les sondes. Compter, et
  journaliser au rang.
- **Ne pas utiliser `git add -A`** : l'arbre est partagé entre tâches
  concurrentes. Nommer les fichiers.

---

## 6. Ce que ce sous-bloc laissera ouvert

Reporté tel quel du §7.5 de D2, augmenté de ce que D3 écarte :

- le **plafond d'encodeurs en multi-processus** — toujours non approché ;
- **latence, cadence, durée** — rien n'aura été mesuré ;
- le **chemin d'extinction propre** du superviseur — jamais exercé ;
- **`SendInput` global à la session Windows** ;
- la **capture mutualisée** (repli spec §8) — désignée par D3 si la campagne le
  commande, implémentée en D4 ;
- **qu'une entrée abandonnée ne soit jamais reproposée** (§2.2) — défaut
  préexistant, nommé et non corrigé.
