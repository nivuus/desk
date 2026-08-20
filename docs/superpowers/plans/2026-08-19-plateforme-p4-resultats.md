# Sous-bloc P4 — l'orchestration et l'attribution d'une VM : résultats

**Plan** : `docs/superpowers/plans/2026-08-19-plateforme-p4.md`.
**Spec** : `docs/superpowers/specs/2026-08-19-plateforme-design.md`, §3.6 et §4 « P4 ».
**Journaux** : `docs/superpowers/plans/journaux-plateforme-p4/`.

**Aucun taux n'est revendiqué dans ce document.** Chaque énoncé porte son
nombre d'exécutions. La convention de P2 et P3 est reconduite :
**exécution 1 = `sqlite`, exécution 2 = `postgres`**, déclaré dans l'en-tête de
chaque journal.

---

## 0. Comment lire les journaux — une seule famille, et une exception

| Famille | État | Ce qu'il faut faire |
| --- | --- | --- |
| tout le répertoire, sauf la ligne ci-dessous | UTF-8, aucune séquence ANSI | rien : les journaux se `grep`ent à plat |
| `vm-1-agent.log`, `vm-2-agent.log` | UTF-8, **séquences ANSI de `tracing`**, CRLF | `sed 's/\x1b\[[0-9;]*m//g'` — ou lire le jumeau `-plat`, versé pour chacun |

C'est la famille de lecture la plus simple de tous les sous-blocs de ce dépôt,
et pour une raison structurelle : **P4 ne fait presque rien tourner sur la VM
Windows**. Les deux seuls journaux qui en viennent sont ceux de la tâche 16.

---

## 1. Le fait n°1 : `vm.utilisateur_id` a désormais son écrivain ET son lecteur filtrant

C'était le point le plus lourd de toute la plateforme, et il était nommé comme
tel dans le plan : « **aucun code de production ne lit ni n'écrit
`vm.utilisateur_id`** ». La colonne existait depuis P1, avec son index unique
partiel, et personne ne s'en servait — c'est-à-dire qu'**il n'y avait aucune
isolation entre utilisateurs** : n'importe quel compte authentifié aurait vu
n'importe quelle VM, s'il avait existé une route pour les lister.

Les deux bouts existent maintenant, et ils sont éprouvés de bout en bout :

- **l'écrivain** — `npm run admin:attribuer -- --email <courriel> --vm <nom|id>`,
  plus `--detacher` pour rendre la VM au vivier. Il passe par
  `orchestration/inventaire-statique.ts::attribuer`, jamais par un `UPDATE`
  écrit à part : dupliquer l'ordre « lire, écrire sous clause, traduire
  l'exception » ferait diverger les deux chemins le jour où l'un changerait, et
  la commande d'administration est précisément celle qu'on relit le moins ;
- **le lecteur filtrant** — `orchestration/selection.ts::vmsDe`, **pur**, sur
  lequel `GET /vm` et `POST /session` s'appuient tous deux.

🔴 **`attribuer` n'est PAS exposée sur HTTP, et c'est une décision, pas un
oubli.** Il n'existe aucun rôle d'administration dans ce service — relevé :
`identite/jeton.ts` ne connaît que `'utilisateur' | 'agent'`, et `config.ts`
n'a aucune variable d'administrateur. Une route qui attribuerait une VM aurait
donc été, au mieux, ouverte à tout utilisateur authentifié : une escalade de
privilège offerte. Un test assère nommément qu'`attribuer` **ne figure pas**
dans la liste blanche des opérations HTTP — sans lui, l'y ajouter un jour de
fatigue ouvrirait l'attribution à tout le monde sans qu'aucun test ne bouge.

---

## 2. Le verdict des quatre critères, avec leur nombre d'exécutions

**Les quatre sont TENUS, deux exécutions chacun.** Aucune assertion n'est
tombée sur aucun des deux moteurs.

| # | Critère | Verdict | Exéc. | Journaux |
| --- | --- | --- | --- | --- |
| ① | `instantane` refuse explicitement, en **501**, et le **journalise** | **TENU** | **2** | `critere-1-{1,2}.log` |
| ② | Deux utilisateurs ne partagent pas une VM, un utilisateur n'en a pas deux, **et jamais un 500** | **TENU** | **2** | `critere-2-{1,2}.log` |
| ③ | Un utilisateur sans VM reçoit un refus **immédiat** | **TENU** | **2** | `critere-3-{1,2}.log` |
| ④ | Une VM dont l'agent n'a pas été vu est **annoncée injoignable**, et l'API **avoue** ne pas savoir la redémarrer | **TENU** | **2** | `critere-4-{1,2}.log` |

### ① — le refus typé, et la ligne qu'il écrit

Le corps rendu, **verbatim** (`critere-1-1.log`) :

```
501 {"motif":"non-supporte","operation":"instantane","backend":"inventaire-statique"}
```

et la ligne de journal, **exactement une**, qui nomme l'opération **et** le
backend :

```
opération refusée : instantane n'est pas supportée par le backend
inventaire-statique, qui ne pilote aucun hyperviseur — il inventorie ce qu'un
administrateur a enrôlé, et n'écrit que vm.utilisateur_id.
```

Le code `501` n'est pas écrit à la main dans la route : il est lu dans
`CODE_HTTP`, un `Record<Motif, number>` dont la clé est l'union dérivée de
`MOTIFS`. **Ajouter un motif sans lui donner son code HTTP est une erreur de
compilation**, que `npm run typecheck` — étape du script de vérification —
attrape. Le sens de la dérivation compte : le tableau `as const` produit le
type, si bien que la liste d'exécution et la liste de types sont **le même
objet**, et non deux objets qu'on espère égaux.

### ② — la propriété que le critère énonçait n'était éprouvée par rien, et c'était mesurable

**C'est la trouvaille de conception la plus lourde du sous-bloc**, et elle a
été faite **avant dispatch**, par une sonde, sur les deux moteurs.

La spec et `0001-socle.sql` disaient tous deux que l'index unique partiel
`vm_un_utilisateur` établissait « deux utilisateurs ne peuvent pas recevoir la
même VM », et prescrivaient comme ROUGE « retirer l'index partiel ». **C'est
faux.** L'index rend `utilisateur_id` unique **à travers les lignes** : il
interdit qu'**un utilisateur ait deux VMs**. Il n'interdit rien à
`UPDATE vm SET utilisateur_id='bob' WHERE id='v1'` quand `v1` est à alice —
écraser un `utilisateur_id` ne viole aucune unicité.

Le critère se scinde donc en **trois** propriétés, à **trois** gardes, avec
**trois** rouges distinctes :

| | Propriété | Garde | Ce qui la rougit |
| --- | --- | --- | --- |
| ②a | une VM n'est attribuée qu'une fois | la clause `AND utilisateur_id IS NULL` de l'`UPDATE` | retirer la clause |
| ②b | un utilisateur ne reçoit qu'une VM | l'index partiel, qui **lève** | retirer l'index de `0001-socle.sql` |
| ②c | la violation est traduite en refus **typé**, jamais un 500 | la relecture après exception | laisser l'exception remonter |

**Les deux moteurs ne lèvent pas le même texte** — `UNIQUE constraint failed:
vm.utilisateur_id` contre `duplicate key value violates unique constraint
"vm_un_utilisateur"`, les deux relevés dans les journaux. **Le code ne compare
donc JAMAIS le message de l'exception** : il **relit** l'état et ne traduit que
ce que la relecture explique ; si elle n'explique rien, **il relance**. Ce
point est éprouvé à part par un `Pilote` factice dont `executer` lève une
erreur étrangère : elle doit REMONTER, et la rouge est de retirer le `throw`
final. Sans cela, un `catch` avalerait une base injoignable et la présenterait
comme un refus métier — la panne muette exacte que la spec §6 interdit.

⚠️ **Un contrôle atteste que la relecture a bien eu lieu, et pas seulement que
le motif est juste** : `lectures faites par la course : 2` (`critere-2-2.log`).
Un code qui devinerait le motif sans relire rendrait **1**, et la rouge ②a
l'établit — elle fait tomber cette assertion précise avec `obtenu 1, attendu 2`.

### ③ — la borne est MESURÉE avant d'être fixée, et elle n'est pas un quantile

`mesure-borne-3.log` : **100 refus consécutifs par moteur**, contre le service
réel.

| | sqlite | postgres |
| --- | --- | --- |
| premier appel, **à froid** | 78,738 ms | 39,866 ms |
| maximum des **99 suivants** | **7,479 ms** | **12,934 ms** |
| médiane | 1,335 ms | 5,023 ms |

**Borne retenue : 250 ms.** Elle n'est **pas** le p99 du relevé, et c'est
délibéré : calée sur lui, elle rougirait au premier ralentissement de la
machine et transformerait le critère en détecteur de charge d'hôte.

🔵 **Une auto-correction du journal mérite d'être conservée, parce qu'elle est
exactement le mode de défaillance que ce dépôt surveille.** Une première
rédaction annonçait « deux ordres de grandeur sous la borne » — **et son propre
relevé la réfutait** : le pire cas brut est de 70,7 ms, soit 3,5 fois
seulement. La correction ne consiste pas à changer le chiffre mais à **séparer
le démarrage à froid**, que le critère ne mesure jamais ; le pire cas
comparable est alors de **12,9 ms**, soit **~19×** sous la borne. Le rapport
juste est plus petit que celui qu'on avait écrit, et il est écrit.

Relevé aux deux exécutions du critère : pire cas de 20 appels **8,29 ms**
(sqlite) et **14,30 ms** (postgres), tous deux sous la borne.

### ④ — la transition est assiégée des deux côtés, à la milliseconde

`SEUIL_INJOIGNABLE_MS = 90 000`. Le test ne se contente pas de constater un
état : il **voit la transition**, et il l'assiège :

```
t = vu_a + SEUIL (la borne EXACTE)   : prete
t = vu_a + SEUIL + 1 ms              : injoignable
```

Une VM **jamais vue** (`vu_a` nul) est `injoignable`, jamais « peut-être ». Et
le corps du 503 porte l'**aveu** :

```
{"vm":"…","nom":"w-dave","prefixe":"cZIhGilp8lrenslvWWvRdg","etat":"injoignable",
 "motif":"agent-injoignable",
 "redemarrage":{"possible":false,"motif":"non-supporte","backend":"inventaire-statique"}}
```

🔴 **`redemarrage` est l'aveu, pas la fonction.** Le cadrage promet « VM
injoignable → le hub l'indique, **propose redémarrage** » ; avec le backend v1
le hub **indique** et **dit qu'il ne sait pas redémarrer**. Le champ porte le
même motif et le même backend que le refus typé de `orchestrateur.demarrer`,
dont il est la projection HTTP — et il est rendu plutôt que laissé au
navigateur à deviner, parce qu'**une absence de champ se lit comme un oubli**.

⚠️ **Le préfixe est rendu QUAND MÊME sur le 503** : il est connu et juste, et
le navigateur en a besoin pour ne pas rejoindre l'espace de noms partagé en
attendant que la VM revienne.

---

## 3. Les DIX rouges, toutes jouées, avec leur message verbatim

⚠️ **Le plan en annonce « dix » dans sa §« règles de méthode » et dans son
tableau, et « neuf » dans l'étape 1 de sa tâche 19.** C'est une incohérence du
plan ; **il y en a dix**, une par assertion des quatre critères, et les dix
sont versées.

Chaque rouge porte, dans son journal : le diff de la mutation, le contrôle
joué **sur l'arbre muté**, et le `sha256` **avant mutation et après
restauration**, les deux comparés. **Les dix restaurations concordent à
l'octet près.** La restauration ne passe ni par `git checkout` ni par
`git stash` — le contenu d'origine est copié dans un fichier temporaire avant
la mutation et réécrit après, et le script ne connaît **qu'un seul fichier**,
ce qui le rend incapable d'emporter le travail d'un voisin.

| Rouge | Mutation | Message VERBATIM de l'assertion tombée |
| --- | --- | --- |
| ①a | `instantane` rend `{ok:true}` | `①a le code est 501, jamais 500 : 🔴 NON TENU — obtenu 200, attendu 501` |
| ①b | retirer le `console.warn` de `refuser` | `①b le refus a produit EXACTEMENT une ligne de journal : 🔴 NON TENU — obtenu 0, attendu 1` |
| ②a | retirer `AND utilisateur_id IS NULL` | `②a l'UPDATE de vol ne touche AUCUNE ligne : 🔴 NON TENU — obtenu 1, attendu 0` |
| ②b | retirer `CREATE UNIQUE INDEX vm_un_utilisateur` de `0001-socle.sql` | `②b l'index partiel LÈVE plutôt que d'attribuer : 🔴 NON TENU — obtenu false, attendu true` |
| ②c | laisser l'exception d'unicité remonter | `②c AUCUNE exception ne s'échappe — c'est elle qui ferait le 500 : 🔴 NON TENU — obtenu "Error: UNIQUE constraint failed: vm.utilisateur_id", attendu undefined` |
| ③a | la route rend 200 sur un utilisateur sans VM | `③a le corps ne porte pas de champ 'prefixe' — un préfixe vide au coffre serait la panne muette : 🔴 NON TENU — obtenu true, attendu false` |
| ③b | 2 000 ms d'attente dans la route | `③b le pire des 20 refus reste sous la borne : 🔴 NON TENU — 2016.17 ≥ 250` |
| ④a | remplacer le corps par un « réessayez » générique | `④a l'état annoncé est 'injoignable' : 🔴 NON TENU — obtenu undefined, attendu "injoignable"` |
| ④b | retirer le champ `redemarrage` | `④b le champ 'redemarrage' EXISTE : 🔴 NON TENU — obtenu false, attendu true` |
| ④c | figer l'horloge injectée de l'orchestrateur | `④a l'état annoncé est 'injoignable' : 🔴 NON TENU — obtenu "prete", attendu "injoignable"` |

🔴 **②b mute une migration livrée par P1**, et c'est la seule rouge de ce
sous-bloc qui touche un fichier de schéma. Elle est jouée comme P3 jouait les
siennes, et son `sha256` concorde.

🔴 **UNE ONZIÈME MUTATION EST RESTÉE VERTE, ET C'EST UNE PIÈCE, PAS UN
INCIDENT.** Elle est nommée au §4 ci-dessous.

---

## 4. 🔴 Trois contrôles vacueux attrapés en chemin — la vraie récolte du sous-bloc

Ce dépôt tient une doctrine : *un contrôle qu'on n'a jamais vu rouge n'est pas
un contrôle*. P4 en a attrapé **trois** qui la violaient, chacun d'une espèce
différente, et **les trois ont été trouvés par l'exécution, jamais par la
relecture**.

**(a) Un `toThrow()` nu, VERT alors que la fonction n'existait pas.** Un test
écrit avant l'implémentation appelait une fonction absente ; `expect(() =>
…).toThrow()` attrapait le `ReferenceError` et se déclarait satisfait. Le test
« passait au rouge » pour la mauvaise raison, puis « passait au vert » sans
que rien n'ait changé de sens. **Remède employé** : un `toThrow()` de ce dépôt
nomme désormais ce qu'il attend, jamais rien.

**(b) Une rouge restée VERTE parce que la chaîne à muter apparaissait D'ABORD
DANS LE COMMENTAIRE QUI LA JUSTIFIE.** La mutation de ②a devait retirer
`AND utilisateur_id IS NULL` du SQL ; la substitution a frappé la **première**
occurrence, qui était dans la phrase française expliquant pourquoi la clause
est là. **Le code est resté intact, et le contrôle est resté vert.** Ce qui l'a
attrapé n'est pas une relecture mais un **garde**, ajouté à l'instrument :
*une rouge doit produire une sortie non vide*, et un diff vide est un échec de
la rouge, pas un succès du produit.

> ⚠️ **La leçon générale, et elle est neuve dans ce dépôt** : dans un dépôt qui
> commente abondamment ses invariants, **une mutation par substitution de
> chaîne frappe le commentaire avant le code**. Plus un invariant est bien
> documenté, plus sa rouge est fragile. Muter par **numéro de ligne** ou par un
> motif ancré sur la syntaxe, jamais par la seule sous-chaîne.

**(c) Une mutation restée verte a révélé qu'une clause du critère n'était
éprouvée par RIEN.** La clause « violation d'index traduite en refus typé,
jamais un 500 » passait par un chemin où la **lecture préalable** refuse avant
toute écriture : l'`UPDATE` n'était jamais atteint, donc l'exception jamais
levée, donc la traduction jamais exercée. Le contrôle mesurait un chemin, la
clause en décrivait un autre. **Remède** : le contrôle compte désormais les
lectures (`lectures faites par la course : 2`) et force le cas où l'écriture
est réellement atteinte.

---

## 5. 🔴 Deux défauts CORS rendaient les DEUX routes inatteignables depuis un navigateur

Et **aucun test de Node ne pouvait les voir** — c'est ce qui en fait une classe,
pas deux accidents.

1. **`Access-Control-Allow-Headers` ne permettait pas `Authorization`.** Les
   deux routes de P4 exigent `Authorization: Bearer`. Un `fetch` de Node envoie
   l'en-tête sans rien demander à personne ; un navigateur, lui, ne l'envoie
   que si la réponse préalable le permet.
2. **La requête préalable `OPTIONS` n'était pas traitée.** L'en-tête
   `Authorization` rend la requête **non simple** : le navigateur envoie
   d'abord un `OPTIONS`, et abandonne sans jamais envoyer la vraie requête si
   la réponse ne lui convient pas.

**Ils ont été trouvés par une corroboration navigateur**
(`corroboration-navigateur.log`), montée pour la tâche 14 : service réel sur un
port, client servi par `vite` sur un autre, donc **origine croisée** — c'est ce
qui met la politique du navigateur dans le chemin. Relevé après correction, par
`curl`, avant toute manipulation de page :

```
OPTIONS /session, Origin: http://127.0.0.1:5199, Request-Headers: authorization
  HTTP/1.1 204 No Content
  Access-Control-Allow-Methods: GET, POST, OPTIONS
  Access-Control-Allow-Headers: content-type, authorization
```

> ⚠️ **La classe reste ouverte** : « ce qu'un navigateur exige et qu'un test
> serveur ne voit pas » n'a aucun garde automatique dans ce dépôt. La seule
> parade employée ici est une corroboration navigateur, et elle est manuelle.

La corroboration a par ailleurs établi les trois issues de l'écran de connexion
sur un vrai Chrome, **avec un résidu délibéré dans le coffre** pour que
l'assertion d'effacement puisse échouer :

| Situation | Coffre | Redirection |
| --- | --- | --- |
| VM attribuée et prête | le préfixe **de sa VM** est écrit | vers la suite |
| **aucune VM** | le préfixe périmé est **effacé** | **non** |
| agent muet | le préfixe est écrit **quand même** | **non**, avec l'état **et** l'aveu |

---

## 6. La corroboration sur VM réelle — PARTIELLE, et sa cause est mesurée

**Deux exécutions**, journaux `vm-1-*`, `vm-2-*`, synthèse dans
`vm-corroboration-releve.log`. **Dix assertions tenues aux deux, quatre non
tenues aux deux**, toutes de la même cause :

```
WARN agent::plateforme: message de la plateforme illisible (version divergente ?)
erreur=version de plateforme non supportée : 2
texte="{\"type\":\"refus\",\"v\":2,\"motif\":\"version\"}"
```

L'agent présent sur la VM parle `PLATEFORME_VERSION = 1` ; le service, bâti
depuis l'arbre partagé, parle la **2**. **C'est exactement la collision que la
décision D6 du plan avait nommée avant tout dispatch** — et elle est survenue
**pendant cette tâche**, le sous-bloc G1 ayant fusionné sa montée de version
dans le même arbre entre la clôture de P4 et sa recette de corroboration.

🔵 **Et la collision est BRUYANTE, pas muette.** Le service refuse, l'agent
journalise chacun de ses essais, `vu_a` reste `null`, et la plateforme annonce
donc correctement `injoignable`. **Elle n'a rien fait de faux** : elle a fait
de cette VM exactement ce que le critère ④ lui demande de faire d'une VM dont
l'agent ne bat pas.

**L'agent n'a PAS été rebâti, et c'est une décision** : P4 ne touche ni
`agent/` ni `proto/` ; le rebâtissage appartient à G1, actif dans le même arbre ;
et à la première tentative `proto/` y était **modifié et non commité**, si bien
qu'une compilation depuis l'hôte aurait poussé du travail à demi fait sur la VM.

**Ce qui est établi malgré tout, contre le VRAI service HTTP** (avec `curl`,
sans une ligne de code à nous entre la surface et le relevé), **avec un VRAI
jeton obtenu par `POST /auth/connexion`, sur une base neuve** :

- `POST /session` sans attribution → **409 `{"motif":"aucune-vm"}`**, et
  **aucun préfixe délivré** ;
- attribuée, agent muet → **503**, `etat: "injoignable"`,
  `redemarrage.possible: false`, **et le préfixe rendu quand même** ;
- le préfixe rendu par la route **est celui de la VM enrôlée** ;
- la page-shell compose `<préfixe>:bureau` **à partir du préfixe rendu par la
  route**, sa poignée de main est **acceptée**, et le relais lui délivre son
  `ice-config` pour cette session-là ;
- `GET /vm` porte `sessions_ouvertes`.

Et les **trois** commandes d'administration ont tourné à la suite sur une base
neuve : `admin:agent`, `admin:utilisateur`, `admin:attribuer`. **C'est la
première fois que le chemin d'attribution complet tourne hors des tests.**

### ✅ Un legs de P3 exercé par accident, et seulement à moitié

E9 — « la reprise du canal `/agent` n'a jamais été exercée » — était encore due.
Les deux journaux d'agent portent **sept** reprises, avec leur échelle de
temporisation doublante relevée verbatim : `delai_ms=500`, `1000`, `2000`,
`4000`, `8000`, `16000`, `30000`.

⚠️ **Ce que cela n'établit pas, et c'est l'essentiel** : l'échec est ici
**permanent** (une divergence de version ne se répare pas d'elle-même). Ce qui
est exercé est **l'échelle de réessai et son plafonnement à 30 s**, jamais une
reprise **réussie**. **Le legs reste dû dans sa moitié utile** — un canal qui
se rompt puis se rétablit —, mais il est désormais dû avec une pièce.

---

## 7. Le sort des douze divergences E1…E12

| # | Divergence | Sort |
| --- | --- | --- |
| E1 | la spec §3.6 annonce un **fichier de configuration** | **Écarté.** `InventaireStatique` lit la base : les trois champs du fichier y sont déjà, et deux sources de vérité divergent en silence. « Statique » = « ne pilote aucun hyperviseur ». Annoté dans la spec |
| E2 | `EtatVm` a **quatre** états | **Réduit à deux.** `arretee` et `demarrage` supposent un hyperviseur hors périmètre ; les écrire ferait du code mort **dans un type**. Annoté dans la spec |
| E3 | la ROUGE que la spec prescrit pour ② ne rougit pas ce que ② énonce | **RÉFUTÉ PAR MESURE**, sur les deux moteurs. Trois propriétés, trois gardes, trois rouges. Annoté dans la spec **et** dans `0001-socle.sql` |
| E4 | la route rendrait la **configuration ICE** | **Écarté.** L'identifiant TURN porte le **nom de session** (`ice.ts`), et une VM en ouvre N : la route n'en connaîtrait qu'une. Elle rend `{ vm, nom, prefixe, etat }`, et **aucun code client n'est privé de quoi que ce soit** |
| E5 | `depot/vm.ts` n'existait pas | **Créé** |
| E6 | aucune extraction de jeton HTTP ; la garde du relais n'est pas réutilisable | **`http/porteur.ts`**, pur et testé, qui exige `type === 'utilisateur'`. **Deux rouges distinctes**, une par sens de confusion |
| E7 | `depot/utilisateur.ts` n'a pas de `lireParId` | **Aucun n'est ajouté** : la commande prend `--email`, les routes emploient le **sujet du jeton**, qui *est* l'`utilisateur.id`. ⚠️ Un jeton reste valide même si le compte disparaissait ; aucun chemin de suppression n'existe, donc le cas n'est pas atteignable — **déclaré, non corrigé** |
| E8 | `changes = 0` confond **trois** causes | **La lecture préalable nomme le motif**, et la scission administration / utilisateur fait que distinguer les trois est **un oracle** sur une route et **nécessaire** sur une commande |
| E9 | la reprise du canal `/agent` jamais exercée | **Exercée à moitié, par accident** (§6). L'échelle de réessai est vue ; une reprise réussie, non. **Reste dû** |
| E10 | `entetesCors` n'annonçait pas `GET` | **Corrigé** — et **deux défauts de plus** ont été trouvés au passage (§5), qu'aucun test de Node ne pouvait voir |
| E11 | `TYPES_RELAYES` ne doit pas changer | **VÉRIFIÉ, pas supposé** : `grep -rn 'TYPES_RELAYES'` rend deux places, toutes deux dans `signaling/relais.ts`, et P4 ne fait transiter **aucun** message par le relais |
| E12 | le rapport de la tâche 14 de D9 n'existe plus | **Hors périmètre**, mais sa règle est appliquée sans exception : **aucune affirmation de ce document ne s'adosse à un rapport de tâche.** Tout s'adosse à un fichier de `journaux-plateforme-p4/`, à un commit, ou à une commande relancée à la clôture |

---

## 8. La revue transverse de fin de branche — sept places, dont une attribution FAUSSE

Le barème du plan est explicite : 5 défauts en D7, 3 en D8, 6 en D9, douze en
D10, sept en D11, huit en P1, dix en P2, cinq en S1, neuf en E, douze en P3,
douze en S2. **Une revue qui n'en trouve aucun n'a pas cherché.**

Les places ont été **énumérées par `grep -n` avant d'écrire** et **relues place
par place après**.

| # | Place | Affirmation, et son sort |
| --- | --- | --- |
| 1 | `plateforme/src/base/migrations/0001-socle.sql:53-56` | 🔴 « une seconde attribution est refusée […] c'est de lui que dépendra le critère 2 de P4 » — **attribution FAUSSE**, réfutée par une rouge jouée sur les deux moteurs. **Annotée**, avec le chemin du journal qui la réfute |
| 2 | spec §3.2 | la même attribution, mot pour mot. **Annotée** |
| 3 | spec §4 « P4 », colonne ROUGE du critère ② | la même. **Barrée** et renvoyée à l'encadré |
| 4 | `plateforme/src/agents/fraicheur.ts:11-20` | « IL N'A AUCUN APPELANT DE PRODUCTION […] ses deux lecteurs à ce jour sont son propre test et la recette » — **P4 lui en donne un**. Annotée |
| 5 | `plateforme/src/signaling/appariement.ts:28` | « P4 reste à venir » — **faux dès la fusion de P4**. Annotée, et la propriété du fichier (« il n'a pas gagné une ligne ») **tient une troisième fois** |
| 6 | `plateforme/src/signaling/propriete.ts:36-39` | « c'est ce dont P4 **aura** besoin » — il la lit. Annotée, **avec la réserve que le nom du champ porte** (`sessions_ouvertes`, pas `sessions_actives`) |
| 7 | `plateforme/src/depot/session.ts:56` | la même formule au futur. Annotée |
| 8 | `client/src/prefixe.ts:11-17` | « LA SOURCE DÉFINITIVE DU PRÉFIXE […] N'EXISTE PAS ENCORE » — **déjà corrigée par la tâche 13**, vérifiée à la revue |

### Deux constats NEUFS, documentés et NON corrigés

🔴 **(a) Le littéral `aucune-vm` du client est une copie qu'aucun type ne
confronte à sa source.** `MOTIFS` (`orchestration/refus.ts`) est un tableau
`as const` dont le type dérive, précisément pour qu'ajouter un motif sans son
code HTTP soit une erreur de compilation — **et cette propriété s'arrête à la
frontière du paquet**. `client/` ne peut pas importer de `plateforme/`, et le
seul paquet partagé est `proto/`, que P4 s'interdit de toucher. **Conséquence à
connaître : renommer `aucune-vm` côté service laisserait le test du client
toujours faux, donc le préfixe périmé au coffre — une panne MUETTE que ni
`typecheck` ni aucun test de ce dépôt ne verrait.** Le remède est de faire
descendre `MOTIFS` dans `proto/ts` ; il est **légué**.

⚠️ **(b) La tension du plan sur `connexion.ts` est ARBITRÉE, pas contournée.**
Sa tâche 14 interdit toute condition dans ce fichier, **puis en prescrit les
branches** ; l'implémenteur l'a signalée sans la trancher. L'arbitrage est
écrit **dans le fichier** plutôt que dans un rapport : *ce que la clause
interdit est qu'une RÈGLE vive dans un fichier non testé, pas qu'un `if` y
apparaisse*. Le critère qui départage est reproductible — **une condition est
une règle si la changer change ce que le produit décide ; elle est du câblage
si elle ne fait que router une décision déjà prise ailleurs, et testée
là-bas.** La clause est donc **resserrée, pas assouplie**.

### ⛔ Une divergence de sécurité, DÉCLARÉE et NON TRANCHÉE — elle appartient au propriétaire du dépôt

Sur une VM qui appartient à **quelqu'un d'autre** :

- **P4** rend un **404 `vm-inconnue`**, *indistinguable* du cas où la VM
  n'existe pas. Distinguer les deux ferait un **oracle d'énumération** : un
  utilisateur apprendrait quelles VMs existent en lisant le code de retour.
  C'est la règle du critère ② de P3 (`agents/enrolement.ts`, deux refus
  identiques caractère pour caractère) et celle de `routes-auth.ts`, appliquées
  ici pour la troisième fois.
- **G1** retient, pour ses propres routes, **403 `vm-etrangere`** — c'est-à-dire
  **un oracle**, distinct du 404 d'une VM inconnue.

**Les deux chantiers ne peuvent pas avoir raison en même temps.** P4 ne
l'aligne pas : unifier est une décision de sécurité qui appartient au
propriétaire du dépôt, pas à la seconde branche arrivée. L'écart est inscrit
dans le code (`plateforme/src/http/routes-vm.ts`), dans ce document, et dans
`CLAUDE.md`.

### Trois affirmations du plan VÉRIFIÉES plutôt que supposées

| Affirmation | Relevé du 20 août 2026 |
| --- | --- |
| `TYPES_RELAYES` n'a pas changé | **VRAI** — `grep -rn` rend `signaling/relais.ts:58` (définition) et `:282` (unique lecture). P4 ne relaie rien |
| `PLATEFORME_VERSION` vaut toujours 1 des deux côtés | ❌ **FAUX AU MOMENT DU RELEVÉ, et pas du fait de P4** : `proto/src/plateforme.rs:44` et `proto/ts/plateforme.ts:16` valent **2**, montés par le sous-bloc G1 (`e4671e6`). **P4 n'a touché aucun fichier de `proto/`** — vérifié : `git diff --stat b4b9adb..HEAD -- proto/` ne porte aucun commit de P4 |
| `plateforme/src/base/pilote.test.ts:49` vaut `expect(deps).toEqual(['pg', 'ws'])` | **VRAI**, inchangé. **P4 n'ajoute aucune dépendance de production** |

---

## 9. Le relevé de tailles, PAR LA COMMANDE, APRÈS la dernière édition

Commande de `CLAUDE.md`, relancée après la dernière édition de la ronde, revue
transverse comprise : **deux** fichiers au-dessus de 500 lignes, et ce sont les
**deux entrées connues du tableau de dette** — `agent/src/encode.rs` **1536**,
`agent/src/windows_source.rs` **630**. **Aucun fichier de `plateforme/` ni de
`client/src/` ne dépasse 500**, ni même 450.

**Le plus gros fichier de production que P4 crée ou touche** :

| Fichier | Lignes | Marge |
| --- | --- | --- |
| `plateforme/src/admin/attribuer-vm.ts` | **224** | 276 |
| `plateforme/src/http/routes-vm.ts` | **212** | 288 |
| `plateforme/src/http/serveur.ts` | **207** | 293 |
| `plateforme/src/orchestration/inventaire-statique.ts` | **189** | 311 |
| `client/src/connexion.ts` | **171** | 329 |
| `plateforme/src/depot/session.ts` | **158** | 342 |
| `plateforme/src/http/routes-session.ts` | **157** | 343 |
| `plateforme/src/depot/vm.ts` | **143** | 357 |
| `plateforme/src/orchestration/interface.ts` | **127** | 373 |
| `client/src/prefixe.ts` | **123** | 377 |
| `plateforme/src/orchestration/refus.ts` | **98** | 402 |
| `plateforme/src/http/porteur.ts` | **90** | 410 |
| `plateforme/src/agents/fraicheur.ts` | **86** | 414 |
| `plateforme/src/http/cors.ts` | **50** | 450 |
| `plateforme/src/orchestration/selection.ts` | **48** | 452 |

🔴 **AUCUNE EXTRACTION N'A ÉTÉ REQUISE PAR P4, et ce n'est pas une omission :
c'est un relevé**, annoncé par le plan avant dispatch et confirmé à la clôture.
Le dépôt n'a jamais eu de sous-bloc où la question ne se posait pas ; celui-ci
en est un, et le dire est plus utile que d'inventer une extraction pour se
conformer à une habitude.

---

## 10. Ce que P4 n'établit PAS

- **Aucun taux, nulle part.** Deux exécutions par critère, deux pour la
  corroboration VM, une pour la corroboration navigateur, une pour la mesure de
  la borne du critère ③ (100 appels par moteur, mais **une** exécution).
- **Aucun backend d'hyperviseur.** `demarrer`, `arreter` et `instantane` ne
  sont **jamais exécutés** ; seule leur voie de refus l'est. Le hub **indique**
  et **avoue** ; il ne redémarre rien.
- **La corroboration sur VM est PARTIELLE** (§6) : l'état `prete` n'a jamais
  été obtenu d'un battement RÉEL. Il l'est en test, sur les deux moteurs, avec
  sa transition assiégée — jamais sur la VM.
- **La concurrence n'est mesurée qu'à DEUX transactions**, une exécution, sur
  Postgres seul, et le test de la suite la reproduit **séquentiellement** : il
  éprouve la traduction du refus, **pas** la sérialisation par le moteur. Rien
  n'est établi à N concurrents, ni sous une autre isolation.
- **`compterOuvertesDe` compte des LIGNES ouvertes, pas des sessions média
  vivantes.** Le média survit à un redémarrage du service alors que la ligne
  est close par le balayage, et une ligne ouverte peut correspondre à un pair
  parti sans que la déconnexion ait été vue. **Le nom du champ porte la
  réserve.**
- **Le legs n°4 de P3 n'est pas soldé** : le préfixe reste **par VM**, pas par
  session, et `signaling/propriete.ts` reste **en mémoire**. Après un
  redémarrage du service, deux clients humains de la même VM retrouvent le même
  préfixe et `<préfixe>:w-1` redevient revendicable. **P4 branche la source du
  préfixe ; il ne change pas sa portée.**
- **`vm.vue_a` reste ORPHELINE** — vérifié à la clôture : aucun code de
  production ne l'écrit ni ne la lit, et le `SELECT` de `depot/vm.ts` énumère
  ses colonnes explicitement pour l'**exclure**, avec son commentaire. P4 lit
  `agent_enrole.vu_a`, jamais `vm.vue_a`.
- **`depot/session.ts::lireParNom` reste sans appelant de production** — seuls
  ses tests et ceux de la trace l'emploient. ⚠️ Ne pas le confondre avec
  `depot/vm.ts::lireParNom`, homonyme, qui en a un (`admin/attribuer-vm.ts`).
- **La reprise RÉUSSIE du canal `/agent` n'est toujours pas exercée** (§6).
- **Aucune revérification d'une session en cours** : la garde ne couvre que la
  poignée de main. Legs n°9 de P2, reconduit par P3, reconduit ici.
- **Aucun durcissement de production** : ni TLS, ni cookies, ni en-têtes de
  sécurité, ni frein sur `/auth/connexion`, `/agent`, `/vm` ou `/session`, ni
  `coturn` restreint, ni `/sante`. **Les deux routes neuves sont ouvertes à la
  force brute exactement comme `/auth/connexion` l'est**, et c'est P5.
- **Aucune constante n'est calibrée**, et aucune ne l'a jamais été depuis
  `BPP_MIN` : `SEUIL_INJOIGNABLE_MS`, `PERIODE_BATTEMENT` (qui se recalibrent
  **ensemble** et vivent dans **deux dépôts distincts**),
  `DUREE_JETON_ACCES_MS`, `OCTETS_PREFIXE`, `DUREE_SECONDES`, et la borne de
  250 ms du critère ③ — celle-là au moins est **mesurée avant d'être fixée**,
  ce qui n'est pas la même chose qu'être calibrée.
- **Aucun audit par un tiers** : CSRF, fixation de session, attaques
  temporelles — choix raisonnés, non éprouvés.
- **La table `application` reste vide** : son chemin d'écriture est le
  sous-projet ④.

---

## 11. Contrôle explicite qu'aucune preuve ne vit hors de git

D10 a établi **par la commande** que l'espace de travail de D9
(`.superpowers/sdd/`, gitignoré, jamais commité) **a disparu**, emportant six
constats de revue définitivement perdus — et que `CLAUDE.md` invitait encore à
« relire » un rapport qui n'existait plus.

**La règle est appliquée ici sans exception, et elle est vérifiée :**

- **aucune affirmation de ce document ne cite un rapport de tâche.** Chacune
  s'adosse à un fichier de `journaux-plateforme-p4/`, à un commit nommé, ou à
  une commande relancée à la clôture ;
- l'**instrument** est versé avec les journaux
  (`journaux-plateforme-p4/instrument/`), dans son état final ;
- le répertoire `.playwright-mcp/`, où l'outillage de navigateur écrivait ses
  captures, a été **ignoré par `.gitignore`** plutôt que versé ou supprimé :
  ce sont des sorties d'outil, jamais des preuves — les preuves de la
  corroboration navigateur sont dans `corroboration-navigateur.log`. Le
  supprimer aurait par ailleurs emporté des traces de chantiers voisins.

**Nombre de fichiers versés sous `docs/superpowers/plans/journaux-plateforme-p4/`,
relevé par `git ls-files docs/superpowers/plans/journaux-plateforme-p4 | wc -l`
à la clôture : 43** — dont **9** d'instrument (les sondes, les pilotes de
rouge, et le script de corroboration VM avec son juge).

---

## 12. Le témoin de clôture — et pourquoi il sort en 1

⚠️ **Deux témoins existent, et il faut dire ce que chacun mesure.**

**① `temoin-verify-all-cloture.log`, joué au commit `b4b9adb` — sortie 0.**
C'est le témoin de P4, et **c'est celui qui fait foi pour lui**.

**② `temoin-verify-all-cloture-finale.log`, relancé après la dernière édition
de cette ronde — sortie 1.** 🔴 **L'échec n'est pas celui de P4**, et c'est
établi par trois relevés plutôt que par une conviction :

1. l'étape qui échoue est la **dernière**, `plateforme : npm run typecheck` :
   `src/agents/canal.ts(147,21): error TS2339: Property 'vm' does not exist on
   type 'EnrolerMessage | CatalogueMessage | LanceeMessage'` ;
2. `git log -S 'CatalogueMessage' --oneline -- proto/ts/plateforme.ts` rend
   **`3bb7487 apps(g1): le miroir TypeScript des trois variantes …`** — un
   commit du sous-bloc **G1**, qui a élargi l'union du protocole sans que
   `agents/canal.ts` ne suive ;
3. `git diff --stat b4b9adb..HEAD -- plateforme/src/agents/canal.ts` rend
   **VIDE** : le fichier qui ne compile pas n'a pas bougé depuis la clôture de
   P4. Et tout ce que P4 a touché depuis est du **commentaire** — sept
   fichiers, 165 insertions, 4 suppressions, **aucune ligne exécutable**.

⚠️ **Lequel des deux comptes est rapporté**, puisque les deux sont vrais de
choses différentes et que P3 a payé une correction pour ne pas l'avoir dit :

- appels de la fonction `etape` **dans le script** : **10**
  (`grep -cE '^etape ' scripts/verify-all.sh`) ;
- en-têtes `==>` **à l'écran de cette exécution** : **17**.

Les sept en-têtes de plus viennent de l'intérieur de l'étape
`client : npm run design:verifier`. **Les deux chiffres sont relevés ce jour**,
aucun n'est recopié.

**Les comptes de tests relevés dans ce témoin** : `plateforme` **284 tests /
37 fichiers** sur les deux moteurs, `client` **223 / 24**, `proto` **130 / 5**.
⚠️ **Le compte `proto` a monté de 111 à 130 depuis le témoin d'entrée, et cette
montée est ENTIÈREMENT celle de G1** : P4 ne touche aucun fichier de `proto/`.
⚠️ **`cargo test --workspace` figure dans le journal ; son compte n'est PAS
attribuable à P4** — deux chantiers voisins travaillent dans `agent/`. Il est
**rapporté, jamais revendiqué**.

---

## 13. Ce que P4 lègue à P5

### Les onze legs de P3, un par un

| # | Legs de P3 | Ce que P4 en a fait |
| --- | --- | --- |
| 1 | la route qui délivre le préfixe au navigateur | ✅ **FERMÉ** — `POST /session`, et `connexion.ts` écrit au coffre par `poserPrefixe` |
| 2 | `agents/fraicheur.ts` orphelin | ✅ **FERMÉ** — `InventaireStatique::etat` est son appelant de production |
| 3 | `session.utilisateur_id` sans lecteur | ✅ **FERMÉ** — `compterOuvertesDe`, rendu par `GET /vm` en `sessions_ouvertes`. ⚠️ **La réserve tient** : il compte des lignes, pas des sessions média |
| 4 | le préfixe est par VM, `propriete.ts` en mémoire | ⛔ **NON SOLDÉ, et non réduit non plus.** P4 branche la **source**, pas la **portée** |
| 5 | aucune isolation entre utilisateurs | ✅ **FERMÉ** — c'est le fait n°1 : `vm.utilisateur_id` a son écrivain et son lecteur filtrant, éprouvés de bout en bout |
| 6 | le secret d'enrôlement en clair dans `C:\dev\run-agent.ps1` | ⛔ **INTOUCHÉ** — P4 n'écrit pas dans `scripts/` |
| 7 | `vm.vue_a` orpheline | ⛔ **TOUJOURS ORPHELINE**, vérifié à la clôture. Le `SELECT` de `depot/vm.ts` l'exclut explicitement, avec son commentaire : c'est la seule garde bon marché contre un successeur qui la croirait renseignée |
| 8 | la reprise du canal `/agent` jamais exercée | ⚠️ **À MOITIÉ**, et par accident (§6). L'échelle de réessai est vue sur sept reprises ; une reprise **réussie**, non. **Reste dû** |
| 9 | aucune revérification d'une session en cours | ⛔ **RECONDUIT** |
| 10 | aucune constante calibrée | ⛔ **RECONDUIT**, et P4 en ajoute une (la borne de 250 ms du critère ③), **mesurée avant d'être fixée** mais pas calibrée |
| 11 | `docker-compose.plateforme.yml` incomplet, aucun durcissement | ⛔ **RECONDUIT** — c'est P5 |

### Les legs PROPRES à P4

1. ⛔ **`depot/session.ts::lireParNom` reste sans appelant de production.**
   ⚠️ Ne pas le confondre avec son homonyme de `depot/vm.ts`, qui en a un.
2. 🔴 **Faire descendre `MOTIFS` dans `proto/ts`** — sans quoi le littéral
   `aucune-vm` du client reste une copie qu'aucun type ne confronte à sa
   source, et son renommage côté service serait une **panne muette** (§8a).
3. ⛔ **La divergence de refus avec G1** — `404 vm-inconnue` contre
   `403 vm-etrangere` — est **déclarée et NON TRANCHÉE**. C'est une décision de
   sécurité (un oracle d'énumération contre son absence) qui appartient au
   **propriétaire du dépôt**.
4. ⛔ **Rejouer la corroboration VM sur un arbre où `PLATEFORME_VERSION` est
   stable.** L'instrument est versé et prêt
   (`journaux-plateforme-p4/instrument/vm-corroboration.sh`) ; les quatre
   assertions qui tombent sont écrites, et **le script n'a pas une ligne à
   changer**.
5. ⚠️ **La classe « ce qu'un navigateur exige et qu'un test serveur ne voit
   pas » n'a aucun garde automatique** (§5). Deux défauts CORS y sont passés ;
   la seule parade employée est une corroboration navigateur **manuelle**.
6. ⚠️ **Le chemin de « ré-attribution au même utilisateur » n'est pas
   distingué** sur la route : `changes = 0` confond trois causes, et la lecture
   préalable nomme le motif **sans que la route ne l'expose** (c'est délibéré —
   l'exposer serait un oracle). L'administrateur, lui, le voit.
7. ⚠️ **Un jeton reste valide jusqu'à son expiration même si le compte
   disparaissait.** Aucun chemin de suppression d'utilisateur n'existe, donc le
   cas n'est pas atteignable — **déclaré, non corrigé** (E7).
