# Sous-bloc P3 — l'identité des agents, et le canal plateforme ↔ agent : résultats

**Plan** : `docs/superpowers/plans/2026-08-19-plateforme-p3.md` (commit `d280745`).
**Conception** : `docs/superpowers/specs/2026-08-19-plateforme-design.md`
(commit `217a765`), §3.3, §3.4 et §4 « P3 ».
**Journaux** : `docs/superpowers/plans/journaux-plateforme-p3/`.

**Vingt-cinq tâches, numérotées 1 à 26 sans le 5** (le plan déclare l'absence
plutôt que de renuméroter). Toutes exécutées.

---

## 0. Comment lire les journaux — DEUX familles, et une seule demande un `sed`

**47 fichiers suivis par git**, relevé par
`git ls-files docs/superpowers/plans/journaux-plateforme-p3 | wc -l`. **Tous
UTF-8**, vérifié fichier par fichier par `iconv -f UTF-8 -t UTF-8` : **aucun
non-UTF-8**.

| Famille | Fichiers | Ce qu'il faut faire |
| --- | --- | --- |
| tout le reste — sondes, rouges, témoins, `instrument/`, les `-plat` | 43 | rien. LF, aucune séquence ANSI, `grep`-ables à plat |
| les **quatre** journaux d'agent bruts — `vm-{1,2}-agent-{avec,sans}-identite.log` | 4 | **CRLF et séquences ANSI de `tracing` PRÉSENTES** : `sed 's/\x1b\[[0-9;]*m//g'`, **ou** lire le `-plat` jumeau, versé pour chacun des quatre |

⚠️ **Neuf fichiers portent des CRLF** (les quatre ci-dessus, leurs quatre
jumeaux `-plat`, et `vm-2-pilote.log`) : ils viennent de la VM Windows. Cela ne
gêne aucun `grep` ; c'est dit pour qu'un `diff` avec un journal de l'hôte ne
surprenne personne.

⚠️ **Contrairement à P1 et P2, ce sous-bloc a employé la VM Windows** — la
tâche 23, et elle seule, hors critère (spec §4).

---

## 1. Le fait n°1 : E2 est fermée, et c'est mesuré sur le fil

La divergence que P1 avait nommée et que P2 avait laissée ouverte à dessein —
un pair `{"role":"agent"}` reçoit des identifiants TURN valables 86 400 s sans
présenter la moindre identité — **est fermée**.

**La même sonde** que celle de P2, à lire ligne à ligne contre elle
(`journaux-plateforme-p2/e2-role-agent-toujours-anonyme.log` contre
`journaux-plateforme-p3/e2-ferme-{1,2}.log`), **deux exécutions**
(exéc. 1 = `sqlite`, exéc. 2 = `postgres`) :

```
messages reçus                    : ["{\"type\":\"error\",\"reason\":\"authentification requise\",\"motif\":\"jeton-absent\"}"]
a reçu ice-config                 : false
a été refusé                      : true
```

En P2, le même relevé donnait `a reçu ice-config = true`, `a été refusé =
false`, et les identifiants TURN présents.

**Ce n'est PAS une assertion unique** : le refus et l'absence d'`ice-config`
sont **deux** assertions distinctes, exactement pour la raison que P2 a payée
(sa rouge ①A n'en faisait tomber qu'une, `expect` interrompant le test à la
première, et il avait fallu écrire une ①A-bis). Les deux vivent aussi comme
tests d'hôte : `garde-fil.test.ts:208` et `:219`.

---

## 2. Le verdict des quatre critères, avec leur nombre d'exécutions

**Deux exécutions par critère** — exéc. 1 = `sqlite`, exéc. 2 = `postgres`,
déclaré dans l'en-tête de chaque journal. **Aucun taux n'est revendiqué.**

| # | Critère | Verdict | Exéc. | Journaux |
| --- | --- | --- | --- | --- |
| ① | Le préfixe cloisonne les sessions, et un agent refusé n'obtient **aucune** `ice-config` | **TENU** | **2** | `critere-1-{1,2}.log` |
| ② | Les deux refus d'enrôlement sont **indistinguables** | **TENU** | **2** | `critere-2-{1,2}.log` |
| ③ | Une version de protocole divergente est refusée **des deux côtés du canal** | **TENU** | **2** | `critere-3-{1,2}.log`, `critere-3-rust-vert.log` |
| ④ | Un agent muet est vu comme tel, et la **transition** est vue | **TENU** | **2** | `critere-4-{1,2}.log` |

### ① — le cloisonnement, et l'intrus qui ne reçoit rien

Deux VMs, deux préfixes de **22 caractères** (128 bits en `base64url`),
distincts, et deux jetons distincts. Les quatre sessions légitimes sont
acceptées (`<P>:bureau`, `<P>:w-1`, `<Q>:bureau`, `<Q>:w-1`), **aucun refus
« déjà connecté »**, et un agent accepté reçoit bien son `ice-config` — le
témoin qui empêche le critère d'être vert par panne.

Puis l'intrus : l'agent P demande `<Q>:w-9`, une session **vierge**.

```
agent P -> <Q>:w-9 (session VIERGE): REFUSÉ (accès refusé à la session demandée)
trames reçues par l'intrus        : ["{\"type\":\"error\",\"reason\":\"accès refusé à la session demandée\",\"motif\":\"session-refusee\"}"]
l'intrus n'a reçu AUCUNE ice-config: TENU
le refus ne dit ni à qui ni si elle existe: TENU
```

Et sur une session **occupée** (`<Q>:bureau`), le refus est le **même** —
`accès refusé à la session demandée`, et non « un agent est déjà connecté » :
**la garde tranche AVANT l'appariement**, si bien que la réponse ne révèle pas
si la session existe.

⚠️ **Le journal du service, lui, nomme tout** — et c'est délibéré :
`poignée de main refusée : session <Q>:w-9 refusée à l'agent <P> : elle ne
porte pas son préfixe`. Ce qui part sur le fil ne porte ni l'un ni l'autre.

### ② — deux refus identiques caractère pour caractère

```
A — VM inconnue, refus BRUT       : {"type":"refus","v":1,"motif":"enrolement"}
B — secret faux,  refus BRUT      : {"type":"refus","v":1,"motif":"enrolement"}
longueurs (A, B)                  : [43,43]
premier caractère divergent       : aucun
A — fermeture                     : {"code":1008,"raison":"enrolement"}
B — fermeture                     : {"code":1008,"raison":"enrolement"}
```

**Les fermetures aussi sont comparées**, et pas seulement les refus : un refus
identique suivi d'une fermeture différente serait le même oracle
d'énumération sous une autre forme. Le bon secret est accepté dans la même
exécution (`type de réponse : enrole`) — sans ce témoin, le critère passerait
sur un service qui refuse tout.

⚠️ **Ce que ce critère N'ÉTABLIT PAS** : il compare des **messages**, jamais des
**durées**. Une attaque temporelle sur la différence entre « lire une ligne
absente » et « vérifier un `scrypt` » reste possible et **n'est mesurée par
rien** (spec §8, choix raisonné non éprouvé).

### ③ — la divergence de version, des deux côtés

`PLATEFORME_VERSION = 1`, vecteur éprouvé en version 2.

```
TS parseVersLaPlateforme          : {"ok":false,"motif":"version"}
TS parseDepuisLaPlateforme        : Error: version de plateforme non supportée : 2
service — réponse BRUTE           : {"type":"refus","v":1,"motif":"version"}
service — fermeture               : {"code":1008,"raison":"version"}
```

Côté Rust : `cargo test -p proto plateforme`, **cinq** tests
`rejette_la_version_suivante_sur_*`, un par variante, **plus** la vérification
de la clé `version` du fichier de vecteurs — que **seul** le TypeScript
contrôlait avant P3 (divergence E8). `critere-3-rust-vert.log`.

### ④ — la fraîcheur, et la borne assiégée des deux côtés

```
vu_a après l'enrôlement           : 1787170156673
réponse au battement              : battement-recu
vu_a après un battement           : 1787170156778
le battement rend un jeton FRAIS  : TENU

typeof vu_a rendu par le dépôt    : number
t = vu_a + seuil (borne)          : prete
t = vu_a + seuil + 1 ms           : injoignable
une VM jamais vue (vu_a = null)   : injoignable
```

**La transition est VUE**, pas déduite : la borne exacte est encore `prete`, une
milliseconde de plus bascule. C'est ce que l'horloge en paramètre rend possible,
et c'est la raison pour laquelle `agents/fraicheur.ts` est **pur**.

⚠️ **`typeof vu_a` = `number` : cette ligne du journal était VRAIE et
INSUFFISANTE** — voir le §5, défaut n°1.

---

## 3. Les sept rouges, avec leur message verbatim

**Sept rouges jouées** là où le tableau du plan n'en nommait que **six** : la
rouge ③ a été jouée **en trois** — une par bout du canal (Rust, puis les deux
sens du parseur TypeScript) —, pour qu'aucun sens ne reste non éprouvé. Toutes
les mutations de source portent leur **`sha256` avant et après restauration**,
et les deux empreintes sont égales dans chaque journal.

| Rouge | Ce qui a été muté | Message verbatim | Journal |
| --- | --- | --- | --- |
| ①A — **gratuite** | rien : jouée sur le **service de P2** (`f0b2fca`) | `un agent est déjà connecté à la session bureau` | `rouge-1A-second-agent-sur-bureau.log` |
| ①B | `garde.ts` : `if (!session.startsWith(verdict.sujet + SEPARATEUR))` → `if (false)` | `agent P -> <Q>:w-9 (session VIERGE): ACCEPTÉ` puis `trames reçues par l'intrus : ["{\"type\":\"ice-config\",…\"username\":\"1787256892:7V-QG48cOOj2rvXlatEbPw:w-9\"…}"]` — **4 assertions NON TENUES** | `rouge-1B-prefixe-non-compare.log` |
| ② | `enrolement.ts` : le refus « VM inconnue » rend `motif: 'forme'` | `A : {"type":"refus","v":1,"motif":"forme"}` contre `B : …"motif":"enrolement"}` ; `longueurs (A, B) : [38,43]` ; `premier caractère divergent : 31` — **2 assertions NON TENUES** | `rouge-2-refus-qui-enumere.log` |
| ③-rust | `proto/src/plateforme.rs` : `verifie_version` omise **sur la seule variante `Refus`** | `test plateforme::tests::rejette_la_version_suivante_sur_refus ... FAILED`, et le bilan : `test result: FAILED. 17 passed; 1 failed` sur **18** | `rouge-3-rust-verifie-version-omise.log` |
| ③-ts-vers | `proto/ts/plateforme.ts` : la vérification de `v` dans `parseVersLaPlateforme` | `TS (vers) refuse pour motif « version » : 🔴 NON TENU` — **2 assertions NON TENUES** | `rouge-3-ts-vers-la-plateforme.log` |
| ③-ts-depuis | idem, `parseDepuisLaPlateforme` | `TS parseDepuisLaPlateforme : 🔴 N'A PAS LEVÉ` — **1 assertion NON TENUE** | `rouge-3-ts-depuis-la-plateforme.log` |
| ④ | `fraicheur.ts` : l'horloge figée | `t = vu_a + 10 × seuil : prete` — **2 assertions NON TENUES** | `rouge-4-horloge-figee.log` |

**La rouge ③-rust est la plus instructive du lot** : elle n'omet la
vérification que sur **UNE** variante, et **un seul test tombe sur DIX-HUIT**
(`17 passed; 1 failed`, relevé verbatim). ❌ *Une première rédaction écrivait
« sur neuf » — le nombre de lignes `test …` que la première page du journal
affiche, et non le compte du bilan. Un chiffre lu sur une page tronquée n'est
pas un chiffre mesuré.*
C'est ce qui établit que la vérification est branchée **variante par variante**
et non une fois pour toutes — la propriété que `control.rs` documente et que le
plan (D4) exigeait de reproduire.

⚠️ **La rouge ①A ne mute rien**, et c'est ce qui fait sa valeur : elle est jouée
sur le binaire du sous-bloc **précédent**, où le défaut est réel et non
simulé.

### La huitième rouge, non prévue par le plan

Voir le §5 : la correction du défaut légué n°1 a exigé sa propre rouge, jouée
et versée (`rouge-bigint-en-chaine-sur-postgres.log`).

---

## 4. Hors critère — deux hypothèses non mesurées, tranchées

### Les enrôlements concurrents pour la MÊME VM

Le sous-bloc l'avait **déduite d'une lecture de `canal.ts`** sans la mesurer :
le superviseur **et chacun de ses enfants** ouvrent leur propre canal `/agent`
avec le même `AGENT_VM` et le même `AGENT_SECRET`.

- **Au banc, sur un service isolé** : **4 canaux de front**, `2 exécutions`.
  `["enrole","enrole","enrole","enrole"]`, **un seul** préfixe distinct,
  **aucun socket fermé**, et les quatre jetons servent quatre sessions de la
  même VM (`<P>:bureau`, `<P>:w-1`, `<P>:w-2`, `<P>:w-3`).
  `enrolements-concurrents-{1,2}.log`.
- **Sur le produit réel** : **3 enrôlements**, `2 exécutions`, même préfixe,
  aucun refus. Voir le §6.

⚠️ **AUCUN PLAFOND N'A ÉTÉ CHERCHÉ.** Quatre au banc, trois sur le produit ;
rien n'est établi au-delà, ni sur ce qui arriverait à N grand.

Le jeton rendu au battement porte bien son *claim* de type — relevé verbatim :
`{"sub":"B_RcUG4pTwJI63W_0S7bFw","exp":1787170713801,"sty":"agent"}`.

### L'agent sans secret échoue BRUYAMMENT

C'est la rouge de la tâche 20, celle des deux lignes de `scripts/run-agent.sh`.
**2 exécutions**, sur la VM :

```
WARN agent: AGENT_VM ou AGENT_SECRET absent : aucun enrôlement, donc aucun
jeton d'agent. La plateforme REFUSERA la poignée de main et aucune session
ne s'établira (sous-bloc P3, sans interrupteur permissif).
```

côté service : `poignée de main refusée : poignée de main sans jeton sur la
session bureau`, et **5,1 ms** (exéc. 1) puis **8,2 ms** (exéc. 2) plus tard,
côté agent : `connexion de contrôle au signaling perdue`. **Aucune session ne
s'établit, et aucun enfant n'est lancé.**

---

## 5. Les DEUX défauts que la recette a trouvés, et ce qu'ils sont devenus

La recette les a relevés **sans les corriger**, en les léguant à la clôture. Ils
le sont désormais.

### Défaut n°1 — `pg` rend les `BIGINT` en CHAÎNE : un défaut de CLASSE

`LigneAgent.vu_a` est déclaré `number | null`. Relevé par `typeof` : `number`
sous `node:sqlite`, **`string` sous `pg`**, qui rend tout `int8` en texte —
`plateforme/src/base/pilote-postgres.ts` ne posait aucun `setTypeParser`.
`interroger<T>` faisant un `as T[]`, **aucun typage ne pouvait l'attraper**.

🔴 **Ce n'était pas une coquille de type.** `agents/fraicheur.ts::etatDe`
survivait **par accident** — sa soustraction convertit l'opérande —, et
`depot/jeton.ts` s'en tirait par un `Number(...)` local avec un type
`number | string`. Rien ne rougissait, et pourtant tout `+`, tout `===` et tout
`>` aurait divergé selon le moteur : `'1787136773742' + 90000` vaut une
concaténation.

**Il est de classe, pas d'instance.** Sept colonnes `BIGINT` sont relues par le
service — `vm.vue_a`, `session.ouverte_a`, `session.fermee_a`,
`utilisateur.cree_a`, `agent_enrole.vu_a`, `jeton_rafraichissement.expire_a`,
`schema_migration.applique_a`. Corrigé **au pilote**, une fois.

**La rouge, vue, une exécution par moteur** (`rouge-bigint-en-chaine-sur-postgres.log`) :

```
AssertionError: expected [ 'vm.vue_a', 'string' ] to deeply equal [ 'vm.vue_a', 'number' ]
AssertionError: promise resolved "[ { vue_a: '9007199254740993' } ]" instead of rejecting
```

⚠️ **Le premier test rougit sous `test:postgres` et reste VERT sous
`test:sqlite`** : c'est exactement la divergence que la double passe existe pour
trouver, et que le test d'époque de P1 **ne pouvait pas** voir — il enveloppe
chaque lecture dans `Number(...)`, ce qui **convertit** la divergence au lieu
de la mesurer. `agent.test.ts` faisait de même ; son assertion est désormais
nue.

La conversion **LÈVE** au-delà de `Number.MAX_SAFE_INTEGER` plutôt que
d'arrondir en silence (`Number('9007199254740993')` rend `9007199254740992`
sans le dire). ⚠️ **Ce chemin n'est pas atteignable par le service**, qui
n'écrit que des `Date.now()` (~1,8e12, quatre ordres de grandeur sous la
borne) : il est gardé quand même, et éprouvé.

Commit `373e331`. Témoin vert : `vert-bigint-en-number-deux-moteurs.log`.

### Défaut n°2 — deux traces annonçaient une session que la plateforme refusait

`agent/src/superviseur/signalisation.rs` écrivait

```
INFO agent::superviseur::signalisation: superviseur enregistré sur la session de contrôle session="bureau"
```

à l'**ÉMISSION** de la poignée de main, donc **avant** d'apprendre qu'elle est
refusée. Aux deux exécutions sans secret, cette ligne sort alors qu'**aucune
session ne s'établit**, et la connexion tombe 5 ms plus tard
(`vm-{1,2}-agent-sans-identite-plat.log`). **Qui la cherche au `grep` pour
savoir si une session tient conclut l'inverse de la vérité.**

⚠️ **`agent/src/signaling.rs:70` portait le même mensonge** (« agent enregistré
auprès du signaling »), que le legs ne nommait pas : **le défaut était de
forme, pas d'instance.** Les deux traces disent désormais ce qu'elles savent —
la déclaration est partie, l'acceptation n'est pas encore connue.

**Et le refus devient observable** : `superviseur/signalisation.rs` classait le
`{"type":"error","reason":…,"motif":…}` du relais dans son bras `Err(_) =>
debug!`, avec `ice-config` et `peer-gone` — donc **invisible** sous le
`RUST_LOG=info` de l'exploitation. Le seul signe restant était « connexion de
contrôle au signaling perdue », qui se lit comme une panne réseau. Un bras
`error` explicite le porte maintenant en `warn!`, avec son motif et sa raison.

⚠️ **CE BRAS N'A PAS TOURNÉ SUR LA VM** : les recettes étaient jouées, et ce
chemin est `#[cfg(windows)]`, donc hors de portée de tout test d'hôte. Vérifié
par `cargo check --target x86_64-pc-windows-gnu` (sortie 0) et `cargo test
--workspace` (550 + 52, inchangé). **Déclaré plutôt que dissimulé.**

Commit `c053fa4`.

---

## 6. La corroboration sur VM réelle — HORS CRITÈRE, 2 exécutions

Relevé complet : `vm-corroboration-releve.log`. Binaire rebâti :
**9 620 480 octets**, contre **9 401 856** pour celui que P3 rend périmé.

| | exéc. 1 | exéc. 2 |
| --- | --- | --- |
| lignes `agent enrôlé auprès de la plateforme` | **3** | **3** |
| préfixes distincts délivrés | **1** | **1** |
| sessions d'agent acceptées | **3** | **3** |
| lignes `ERROR` ou `WARN`, phase « avec identité » | **0** | **0** |

Les **trois** lignes de la table `session` portent leur `vm_id`, résolu depuis
le **préfixe** du nom de session (`signaling/trace.ts`) — la tâche 16 vue sur le
produit. ⚠️ À l'exécution 1, la première ligne porte `utilisateur_id: null` : la
session de contrôle y a été appariée par l'agent avant que la shell scriptée
n'ait revendiqué ; `vm_id` y est rempli tout de même, sur les trois lignes.

⚠️ **CE QUE CETTE CORROBORATION N'EST PAS.** La page-shell est **scriptée** :
elle signe elle-même son jeton d'utilisateur au lieu de passer par la route
d'authentification, et **elle ne négocie AUCUNE session WebRTC**. Aucune image
n'est décodée, aucune latence mesurée, aucun `coturn` ne tournait. Ce qui est
établi est l'**enrôlement**, le **préfixe** et l'acceptation des **poignées de
main** — l'objet exact de la tâche, que la spec §4 range hors critère.

⚠️ **Une troisième tentative, antérieure, a AVORTÉ et n'est pas versée** : la VM
s'était éteinte d'elle-même entre la compilation et le lancement (piège
documenté depuis D1), et le port 8080 était tenu par un service de plateforme
d'une recette antérieure. Rien n'en a été mesuré. **Dit plutôt que tu.**

---

## 7. Le sort des seize divergences E1…E16, tranchées avant d'écrire

| # | Ce qu'elle tranchait | Sort, vérifié à la clôture |
| --- | --- | --- |
| **E1** | P3 casse la compatibilité de l'agent déployé, et la spec §10.3 ne le dit pas | **TENUE.** Coupure franche, sans interrupteur permissif ; `agent/src/main.rs` le journalise en `warn!` à défaut de variables. **La spec §10.3 est annotée** par la revue transverse (tâche 24) |
| **E2** | les numéros de ligne de la spec §3.4 et §10.2 ont dérivé | **TENUE, et REJOUÉE** : `webrtc.ts:193` vaut **229**, `shell-page.ts:45` vaut **70** (45 → 62 sous P2, 62 → 70 sous P3). Annoté dans la spec |
| **E3** | `shell-page.ts` ne peut pas « recevoir le préfixe de la plateforme » en P3 | **TENUE.** `client/src/prefixe.ts` (**72** lignes, pur) lit le coffre puis la requête, **et rend la chaîne vide à défaut** — ce qui restitue `bureau` |
| **E4** | la garde de P2 est pure et synchrone : l'enrôlement ne peut pas y vivre | **TENUE.** Le canal vit dans `http/serveur.ts`, sur son propre `WebSocketServer` ; `verifier` rend toujours un `Verdict` sans `Promise` |
| **E5** | jeton d'agent et jeton humain, mêmes secret et charge : un *claim* de type | **TENUE.** `sty` relevé sur le fil : `{"sub":"…","exp":…,"sty":"agent"}`. **DEUX rouges distinctes**, une par sens de confusion, comme la leçon ①A-bis de P2 l'exigeait |
| **E6** | le séparateur `:` croise le format d'identifiant TURN | **TENUE et FIGÉE PAR UN TEST** : `expect(username).toBe('4600:AAAAAAAAAAAAAAAAAAAAAA:bureau')`, plus `username.split(':')[0] === '4600'`. ⚠️ **Aucun coturn vivant n'a été sollicité** : « coturn coupe sur le premier `:` » reste une lecture de la convention `use-auth-secret` |
| **E7** | `plateforme/` n'est câblé sur `proto/` par rien | **TENUE.** `plateforme/tsconfig.json` : `"include": ["src/**/*.ts", "../proto/ts/**/*.ts"]`, sur le modèle exact de `client/` |
| **E8** | `proto/vectors.json` est structuré pour `input` seul | **TENUE.** `proto/plateforme-vectors.json` (**73** lignes), `"version": 1`, **vérifiée des deux côtés** — `include_str!` côté Rust, `import` côté TS. La lacune de `vectors.json` est corrigée **pour le fichier neuf**, sans réécrire un test qui n'appartient pas à P3 |
| **E9** | aucun client WebSocket de l'agent n'a de reprise | **TENUE.** `agent/src/plateforme/repli.rs` (**89** lignes, pur) porte le calcul du délai ; le socket, non. ⚠️ **La reprise n'a JAMAIS été exercée** — aucune coupure n'a été provoquée, ni au banc ni sur la VM |
| **E10** | `session.vm_id` : le relais n'apprend rien de la base | **TENUE.** La résolution vit dans `signaling/trace.ts` ; `apparie` n'a pas changé de signature, et le relais ne connaît toujours pas la base |
| **E11** | la rouge gratuite du critère ① ne passe pas par le message de refus | **TENUE**, jouée sur le service de P2. ❌ **Et son propre relevé a été rendu FAUX par cette branche** : elle déclarait `agent/src/signaling.rs:130` « relu et juste », et le commit `5fbc89b` l'a poussé à **139**. Voir le §8 |
| **E12** | un test livré par P2 affirme l'inverse de ce que P3 livre | **TENUE.** `garde-fil.test.ts:208` et `:219` — le test est réécrit **à dessein**, et en **deux** assertions séparées, pas une |
| **E13** | `application` naît en P3 avec sa contrainte | **TENUE.** `0003-agents.sql` (**58** lignes) fait naître `agent_enrole` **et** `application`, cette dernière **vide** mais **avec sa clé étrangère vers `vm(id)`** — leg n°2 de P1 appliqué |
| **E14** | `proto/src/control.rs` est à 470 lignes, et P3 n'y touche pas | **TENUE, et VÉRIFIÉE PAR LA COMMANDE à la clôture** : `wc -l` rend toujours **470**. C'est ce qui a fait vivre le canal dans un fichier neuf |
| **E15** | le déni de service en une trame reste ouvert : PRÉCISER, pas corriger | **TENUE**, et exécutée par la revue transverse : la phrase principale de `resilience.test.ts` reste vraie **par sa première raison seule** |
| **E16** | la création d'un enrôlement n'a ni point d'entrée ni convention | **TENUE.** `npm run admin:agent` (**162** lignes), secret tiré au sort et écrit **une seule fois** sur stdout ; un `--secret` en argument est refusé **avec son motif**, parce que `ps` expose la ligne de commande de tout processus |

---

## 8. La revue transverse de fin de branche — DOUZE défauts, QUINZE places

Barème : **5** défauts en D7, **3** en D8, **6** en D9, **douze** en D10,
**sept** en D11, **huit** en P1, **dix** en P2 (sur vingt-trois places).
**Douze ici, sur quinze places** — toutes énumérées par `grep -n` **avant**
l'édition et relues place par place **après**. Commit `e77fc0e`.

| # | Place | Ce qui est devenu faux | Sort |
| --- | --- | --- | --- |
| 1 | `plateforme/src/signaling/appariement.ts:50` **et** `appariement.test.ts:9-11` | `agent/src/signaling.rs:130` — **poussé à 139 par le commit `5fbc89b` DE CETTE BRANCHE** ; et `client/src/webrtc.ts:109` — la ligne est **vide**, la lecture vit aux 130-131 (dérive de P2) | **CORRIGÉ, deux places** |
| 2 | `plateforme/src/base/migrations/0001-socle.sql:77` | « `vm_id`, lui, reste entièrement vide : c'est P3 » — **P3 la remplit** | **CORRIGÉ** |
| 3 | `plateforme/src/signaling/resilience.test.ts:10` | « le rôle `agent` reste de toute façon anonyme jusqu'à P3 » | **PRÉCISÉ, pas corrigé** — le sort qu'E15 prescrivait d'avance |
| 4 | `plateforme/src/signaling/appariement.ts:19` | « P3 et P4 restent à venir » | **ANNOTÉ** — et la propriété tient une seconde fois : ce module n'a pas gagné une ligne |
| 5 | `plateforme/src/signaling/trace.ts:13` **et** son jumeau de `CLAUDE.md` | « elle se rouvrira […] ce qui est le sujet de P3 (`vu_a`) » — **P3 a eu lieu SANS la rouvrir** | **ANNOTÉ, deux places** |
| 6 | `plateforme/src/signaling/propriete.ts:19` **et** son jumeau de `CLAUDE.md` | « la vraie réponse est le PRÉFIXE OPAQUE de P3 » — il existe, **et il ne suffit pas** | **ANNOTÉ, deux places** |
| 7 | spec §2.3 ③ | l'annotation de P2 : « un pair de rôle `agent` est **toujours** servi sans aucune identité, jusqu'à P3 » | **RÉ-ANNOTÉ** |
| 8 | spec §7.2, « fenêtre d'exposition déclarée » | « entre P2 et P3, le pair `agent` reste non authentifié » | **ANNOTÉ** — elle aura duré le temps d'un sous-bloc |
| 9 | spec §10.3 point 3 **et** son ⚠️ | « sa modification est d'une ligne par site » ; « le mode d'essai local reste donc possible » | **ANNOTÉ, deux places** (E1) |
| 10 | spec §10.2 point 2 | `webrtc.ts:193` et `shell-page.ts:45` | **ANNOTÉ** |
| 11 | `CLAUDE.md` P1 ⑫ legs 1, 3, 4, 5 | dont **deux pronostics faux DE LIEU** : `vm.vue_a` (P3 a créé `agent_enrole.vu_a`), et « `relais.ts` accueillera le canal `/agent` » (il vit dans `http/serveur.ts`) | **CORRIGÉ** |
| 12 | `CLAUDE.md` P2 ⑪ legs 1, 2, 3 | E2, le préfixe, `session.vm_id` | **CORRIGÉ**, dont le n°2 **REFORMULÉ et non coché** |

**Les trois plus instructives**, parce qu'aucune revue par tâche ne pouvait les
voir :

1. **`appariement.ts:50` a été rendue fausse par P3 lui-même.** La citation
   était **exacte quand le plan a été écrit** — E11 le dit en toutes lettres,
   « relu et juste » —, et la tâche 19 l'a déplacée. **Leçon neuve, que ce
   dépôt n'avait pas formulée : relire une citation `fichier:ligne` avant
   d'écrire ne suffit pas quand le plan prescrit par ailleurs de déplacer la
   ligne citée. Il faut la relire APRÈS avoir exécuté ce qui la déplace.**
2. **Le naufrage du « 487 », à l'intérieur du commit qui le dénonçait.** Le
   commit `254fdd5` a réécrit les **six** lignes qui précèdent
   « `vm_id`, lui, reste entièrement vide » — sans balayer les **deux**
   suivantes. Son propre message de commit revendique pourtant d'avoir
   « énuméré les places par `grep` AVANT d'écrire ».
3. **`verify-all.sh` : « dix » et « dix-sept » sont vrais de deux choses
   différentes.** Le tableau S1 de `CLAUDE.md` annonce « neuf → dix étapes », le
   plan de P3 « ses neuf étapes ». **Mesuré** sur une exécution complète
   (sortie **0**) : **DIX** appels `etape` dans le script, et **DIX-SEPT**
   en-têtes `==>` à l'écran — les **sept** derniers venant de l'intérieur de
   l'étape `client : npm run design:verifier` (un `npm run build` plus les
   **six** contrôles du socle ; le septième contrôle, §7.5, est un test
   unitaire et tourne dans `client : npm test`). Les deux comptes sont
   corrigés ou précisés ; **le plan disait « neuf », ce qui est faux des deux
   façons de compter.**

**Une mesure de doctrine, que ce dépôt n'avait pas** : l'en-tête de
`plateforme/src/signaling/relais.ts` a été corrigé **TROIS FOIS, une par
sous-bloc** — P1, P2, P3 —, et chaque correction a laissé derrière elle une
« moitié qui reste vraie » que la suivante a dû reprendre. **La durée de vie
d'un « reste VRAI » dans ce dépôt est d'UN sous-bloc.**

### Relevé, non corrigé — hors du périmètre de P3

- `docs/superpowers/specs/2026-08-19-design-system-design.md:145` cite
  `client/src/shell-page.ts:45` pour affirmer « aucun jeton dans la poignée de
  main ». **La ligne vaut 70, et l'affirmation a été rendue fausse par P2**, pas
  par P3 : le fichier envoie `{ role: 'client', session, jeton }`. C'est une
  affirmation du sous-projet ⑥ ; **signalée, laissée à son propriétaire.**

---

## 9. Le relevé de tailles, PAR LA COMMANDE, APRÈS la dernière édition

Relevé au commit de clôture, **après** les corrections des deux défauts légués
**et** celles de la revue transverse — une table mesurée en début de ronde
serait fausse à la fin de la même ronde (erreur de D8).

**Le tableau de dette est INCHANGÉ, et il a toujours DEUX lignes** :
`agent/src/encode.rs` **1536**, `agent/src/windows_source.rs` **630**. **Aucun
autre fichier de code source ne dépasse 500 lignes.**

**59 fichiers touchés par les commits `(p3)`**, relevé par
`git log --format='%H' --grep='(p3)' | while read h; do git show --name-only --format='' "$h"; done | sort -u`.
Les plus gros, et ceux dont la marge s'est resserrée :

| Fichier | Avant P3 (`d280745`) | Après | Remarque |
| --- | --- | --- | --- |
| ⚠️ `agent/src/demarrage.rs` | 481 | **491** (marge **9**) | ❌ **CE +10 N'EST PAS DE P3.** Une première rédaction de ce tableau le lui imputait, sur le seul écart avant/après ; `git log --numstat` dit autre chose : **+10/−0 par le chantier E1** (`f2f8e05`), et **+1/−1 par P3** (`5fbc89b`), soit **net zéro**. Le plan de P3 annonçait « rien si le câblage passe par `superviseur.rs` et `main.rs` » : **il avait raison**. La marge de 9 est déjà signalée par la section du chantier E de `CLAUDE.md`. **Toute addition future appelle une extraction** |
| ✅ `agent/src/superviseur/table.rs` | **492** (marge 8) | **433** (marge 67) | **L'EXTRACTION A ÉTÉ FAITE AVANT L'ADDITION** (tâche 17, `aa220ba`), et elle a tenu : `table/orphelines.rs` (**121**) est né, puis la tâche 19 y a écrit son préfixe **sans franchir quoi que ce soit**. C'est le geste que **D9 (tâche 6) a inventé** et que **D10 a joué trois fois** (ses tâches 1 à 3) ; P3 le reconduit |
| `agent/src/main.rs` | 329 | **435** | dont **+77 net par P3** (`AGENT_VM`, `AGENT_SECRET`, le câblage de l'enrôlement) et **+29 par les chantiers A-bis et E1**. Attribution relevée par `git log --numstat`, **pas déduite de l'écart avant/après** |
| `proto/src/plateforme.rs` | — | **410** | neuf — `PLATEFORME_VERSION`, les messages, `verifie_version` variante par variante |
| `plateforme/src/agents/canal.test.ts` | — | **361** | neuf |
| `plateforme/src/signaling/relais.ts` | 310 | **327** | +17, presque entièrement du commentaire |
| `agent/src/plateforme.rs` | — | **277** | neuf — le client du canal, avec sa reprise |
| `proto/ts/plateforme.test.ts` | — | **267** | neuf |
| `proto/ts/plateforme.ts` | — | **207** | neuf |
| `plateforme/src/agents/canal.ts` | — | **198** | neuf |
| `agent/src/plateforme/tests.rs` | — | **196** | neuf |
| `plateforme/src/admin/enroler-agent.ts` | — | **162** | neuf |
| `agent/src/superviseur/table/orphelines.rs` | — | **121** | neuf — l'extraction de la tâche 17 |
| `plateforme/src/base/pilote-postgres.ts` | 76 | **118** | +42 : le `setTypeParser` et sa justification |
| `plateforme/src/base/pilotes.test.ts` | 141 | **251** | +110 : les deux tests du défaut `BIGINT`, et leur raison |
| `plateforme/src/signaling/appariement.ts` | 97 | **120** | +23, entièrement du commentaire de revue transverse |
| `agent/src/plateforme/repli.rs` | — | **89** | neuf — **pur** |
| `plateforme/src/agents/fraicheur.ts` | — | **76** | neuf — **pur** |
| `proto/plateforme-vectors.json` | — | **73** | neuf |
| `client/src/prefixe.ts` | — | **72** | neuf — **pur, sans DOM** |
| `plateforme/src/agents/prefixe.ts` | — | **68** | neuf — **pur** |
| `plateforme/src/agents/enrolement.ts` | — | **65** | neuf |
| `plateforme/src/base/migrations/0003-agents.sql` | — | **58** | neuf — `agent_enrole` **et** `application` |

⚠️ **`agent/src/transport.rs` est à 495 lignes, marge 5** — la **deuxième** plus
serrée du dépôt après `encode/arret.rs` (500, marge 0), devant
`client/verify-webrtc.mjs` (494, marge 6). Le plan de P3 le relevait à **491**.
❌ **Une première rédaction ajoutait « et qu'aucun document ne signalait » :
c'est FAUX** — la section du chantier **E1** de `CLAUDE.md` le déclare en toutes
lettres, avec son attribution et son injonction d'extraction. **P3 n'y a pas
touché** ; la ligne reste ici pour qu'un lecteur de P3 connaisse la marge dont
il dispose, non parce qu'elle serait neuve.

---

## 10. Ce que P3 n'établit PAS

- **Aucun taux, nulle part.** Deux exécutions par critère, deux pour la
  corroboration VM, une par moteur pour la rouge du `BIGINT`. Jamais une
  campagne.
- **La reprise du canal `/agent` (E9) n'a JAMAIS été exercée** : aucune coupure
  n'a été provoquée, ni au banc ni sur la VM. Son calcul de délai est pur et
  testé ; le comportement du socket, non.
- **Le bras `error` neuf de `superviseur/signalisation.rs` n'a pas tourné sur la
  VM** — `#[cfg(windows)]`, hors de portée des tests d'hôte, et les recettes
  étaient jouées quand il a été écrit.
- **Aucun plafond d'enrôlements concurrents n'a été cherché** : quatre au banc,
  trois sur le produit.
- **Aucun `coturn` vivant n'a été sollicité** (E6) : « coturn coupe sur le
  premier `:` » reste une lecture de la convention `use-auth-secret`.
- **Aucune session WebRTC n'a été négociée** sur la VM : la page-shell est
  scriptée, aucune image décodée, aucune latence mesurée.
- **Aucune constante n'est calibrée** : `SEUIL_INJOIGNABLE_MS`, `REPLI_MIN_MS`,
  `REPLI_MAX_MS`, `OCTETS_PREFIXE`, et celles de P1/P2 que P3 ne recalibre pas
  (`DUREE_SECONDES = 86 400`, `DUREE_JETON_ACCES_MS`, `N`/`r`/`p` de `scrypt`).
  **Aucun jugement d'usage n'a été porté sur aucune d'elles** — la lacune que ce
  dépôt traîne depuis `BPP_MIN`.
- **Le déni de service en une trame reste OUVERT** (E15) : le contrôle de forme
  court avant toute garde, et aucune authentification ne peut fermer ce
  chemin-là. Le frein est P5 ③.
- **Aucune protection contre le vol du secret d'enrôlement sur la VM** : il vit
  en clair dans `C:\dev\run-agent.ps1`, comme les 57 autres variables. **À
  rouvrir en P5.**
- **Aucune revérification d'une session en cours** : la garde ne couvre que la
  poignée de main — legs n°9 de P2, reconduit.
- **Aucune attaque temporelle n'est mesurée** (§2 ②) : le critère compare des
  messages, jamais des durées.
- **Rien du comportement d'un agent qui perd son canal pendant une session
  établie.**
- **Aucun audit par un tiers** : CSRF, fixation de session — choix raisonnés,
  non éprouvés (spec §8).
- **`vm.vue_a` reste une colonne orpheline** : P3 a créé `agent_enrole.vu_a` et
  n'écrit jamais dans `vm.vue_a` (relevé par `grep -rn "vue_a" plateforme/src` :
  seuls des tests l'écrivent).
- **`application` reste VIDE** : son chemin d'écriture est le sous-projet ④.

---

## 11. Contrôle explicite qu'aucune preuve ne vit hors de git

```
$ git ls-files docs/superpowers/plans/journaux-plateforme-p3 | wc -l
47
```

**Aucun rapport de tâche n'est cité comme preuve dans ce document.** D10 a
établi par la commande que l'espace de travail de D9 (`.superpowers/sdd/`,
gitignoré) **a disparu**, emportant six constats de revue définitivement perdus.
Tout ce que ce document affirme est adossé à un fichier de
`journaux-plateforme-p3/`, à un commit, ou à une commande relancée à la
clôture — et le dit.

L'instrument lui-même est versé : `journaux-plateforme-p3/instrument/`
(`socle.ts`, `sonde.ts`, `rouge-1a.ts`, `shell-scripte.ts`, `jouer.sh`,
`rouge.sh`, `vm-corroboration.sh`, `package.json`).

---

## 12. Le témoin de clôture

`./scripts/verify-all.sh`, **relancé APRÈS la dernière édition de la ronde** —
les deux correctifs des défauts légués (`373e331`, `c053fa4`) **et** la revue
transverse (`e77fc0e`) —, **sortie 0**. Journal :
`temoin-verify-all-cloture-finale.log`, versé, arbre propre sur les cinq
paquets. ⚠️ **Précision, parce que « après la dernière édition » est une
affirmation qu'il faut tenir** : ce témoin est joué au commit `e77fc0e`, qui
est le **dernier commit de CODE** de la branche. Les commits qui le suivent
(`add387e`, et la correction du « sur neuf ») ne touchent que `CLAUDE.md` et
ce document — que `verify-all.sh` ne lit pas. *(Le témoin de la recette, `temoin-verify-all-cloture.log`, reste versé
à côté : il porte `plateforme` à 194, l'état d'avant les deux tests neufs.)*

| Étape | Relevé |
| --- | --- |
| `cargo test --workspace` | **550** + **52** + 0 |
| `cargo clippy --workspace` | vert (210 lignes `warning`, toutes `dead_code` préexistantes) |
| `client : npm test` | **187** |
| `client : npm run typecheck` | sortie 0 |
| `client : npm run design:verifier` | **6/6 contrôle(s) vert(s)** |
| `proto : npm test` | **70** |
| `proto : npm run typecheck` | sortie 0 |
| `plateforme : npm run test:sqlite` | **196** |
| `plateforme : npm run test:postgres` | **196** |
| `plateforme : npm run typecheck` | sortie 0 |

⚠️ **`verify-all.sh` compte DIX étapes et en affiche DIX-SEPT.** Le tableau
ci-dessus a **dix** lignes — les dix appels `etape` du script —, quand
`grep -c '^==>'` sur le journal rend **17** : l'étape
`client : npm run design:verifier` en imprime **sept** de son propre chef (un
`npm run build`, puis les six contrôles du socle). **Relevé par la commande sur
ce journal-ci**, et non recopié.

⚠️ **`plateforme` passe de 194 à 196**, et les **+2** sont annoncés avant d'être
lus : ce sont les deux tests neufs du défaut `BIGINT`. C'est le geste que D10 a
inventé pour rattraper un test supprimé par un `Write` d'écrasement.

---

## 13. Ce que P3 lègue à P4 et à P5

**Legs de P1 réglés** : n°1 (l'authentification, **entièrement** cette fois),
n°3 (`session.vm_id`), n°4 (observer les agents — **mais pas sur la colonne que
le pronostic nommait**), n°5 (le canal `/agent` — **mais pas au lieu que le
pronostic nommait**).
**Legs de P2 réglés** : n°1 (E2), n°3 (`session.vm_id`). Le n°2 (le préfixe) est
**réduit, pas soldé** — voir ci-dessous.

**Ce qui reste dû :**

1. ⛔ **P4 — la route qui rend un préfixe à un navigateur.** `client/src/prefixe.ts`
   lit le coffre puis la chaîne de requête, et **rend la chaîne vide à défaut**
   (E3). C'est un **paramètre sans source** : P4 doit brancher la sienne, avec
   l'attribution d'une VM à un utilisateur. ⚠️ **Le coût est celui que la spec
   §10 nomme elle-même** : une plateforme mal configurée retombe silencieusement
   dans un espace de noms partagé.
2. ⛔ **P4 — `agents/fraicheur.ts` n'a AUCUN appelant de production.** Ce qu'il
   décide — `prete` / `injoignable` — n'est lu par personne tant qu'aucune vue ne
   liste les VMs. Il est pur, testé, et **déclaré orphelin** plutôt que doté d'un
   appelant inventé qui préempterait P4.
3. ⛔ **P4 — `session.utilisateur_id` attend toujours son lecteur**, et
   `application` attend son écrivain (sous-projet ④, qui empruntera le canal
   `/agent`).
4. 🔴 **P4 ou P5 — le préfixe ne ferme PAS la revendication au sein d'une VM.**
   Il est **par VM**, et le registre d'appartenance de `signaling/propriete.ts`
   est **en mémoire** : après un redémarrage du service, deux clients humains de
   la même VM retrouvent le même préfixe et `<préfixe>:w-1` redevient
   revendicable. Le legs n°2 de P2 est **réduit, pas soldé**.
5. ⛔ **P5 — le frein sur les routes d'authentification ET sur `/agent`.** Les
   tentatives de secret d'enrôlement ne sont bridées par **rien** ; c'est la même
   fragilité que `/auth/connexion`, sur un chemin neuf.
6. ⛔ **P5 — le secret d'enrôlement en clair sur la VM**, dans
   `C:\dev\run-agent.ps1` sur un partage CIFS lisible depuis l'hôte. **Nommé
   d'avance par la décision D1 du plan, et explicitement renvoyé à P5.**
7. ⛔ **P5 — TLS, cookies, en-têtes de sécurité, `/sante`, `coturn` restreint**,
   et le déni de service en une trame (E15).
8. ⛔ **Tous — la reprise du canal `/agent` n'a jamais été exercée** (E9). Sans
   elle, une coupure réseau rendrait une VM `injoignable` définitivement — c'est
   la raison même pour laquelle elle existe, et rien ne l'a éprouvée.
9. ⛔ **Tous — la garde ne couvre que la POIGNÉE DE MAIN.** Legs n°9 de P2,
   reconduit sans changement : revérifier une session en cours est un changement
   de conception, pas un correctif.
10. ⚠️ **Hors P3 — `agent/src/transport.rs` est à 495 lignes, marge 5**, portée
    là par le chantier E1, **qui la signale déjà lui-même**. Reprise ici pour
    mémoire, non corrigée : hors périmètre.
11. ⚠️ **Hors P3 — `docs/…/2026-08-19-design-system-design.md:145`** affirme
    « aucun jeton dans la poignée de main » sur `shell-page.ts:45`. **Faux depuis
    P2**, et le numéro vaut 70. Laissé à son propriétaire.
