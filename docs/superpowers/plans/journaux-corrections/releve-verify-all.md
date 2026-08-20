# `scripts/verify-all.sh` — le rouge inexpliqué, et ce qu'il mesurait (20 août 2026)

## Les six tests, nommés

Tous dans `plateforme/src/signaling/server.test.ts`, `describe('serveur de signaling')` :

1. `relaie une réponse de l'agent vers le client`
2. `signale la disparition du pair`
3. `relaie les messages de la session de contrôle entre superviseur et shell`
4. `délivre à l'agent l'offre arrivée avant lui`
5. `ne délivre que la dernière offre, pas toutes celles reçues`
6. `oublie l'offre mémorisée quand la session se vide`

## Les conditions, mesurées

| Condition | Résultat | Exécutions |
| --- | --- | --- |
| `plateforme : npm run test:sqlite`, shell nu | **340/340** | 3 |
| idem, avec `TURN_URL` **et** `TURN_SECRET` posées | **6 failed \| 334 passed (340)**, les six ci-dessus | 2 |
| `verify-all.sh`, `env -i` (shell nu) | **sortie 0**, 18 en-têtes, plateforme 340/340 sur les deux moteurs | 1 |
| `verify-all.sh`, après `set -a && source .env && set +a` | **sortie 1**, échec à l'étape `test:sqlite`, les six | 1 |

Journaux : `verify-all-vert-shell-nu.log`, `verify-all-rouge-source-env.log`.

## La cause

`relais.ts` lit `process.env` à **chaque** déclaration de pair
(`configurationIce(process.env, …)`) et, si `TURN_URL` et `TURN_SECRET` sont
toutes deux posées, envoie un `ice-config` au pair **avant tout autre message**.
Les douze tests de `server.test.ts` lisent « le message suivant » sans jamais
poser cet environnement : ils recevaient l'`ice-config` à la place de ce qu'ils
attendaient.

**Le service était sain. Les tests mesuraient l'environnement de celui qui les
lançait.** `CLAUDE.md` prescrit `set -a && source .env && set +a` pour tout le
reste du dépôt (VM, WinRM, coturn) : suivre la consigne du dépôt suffisait à
faire rougir sa barrière. C'est ce qui réconcilie les deux relevés contradictoires
du 19 août — aucun commit ne les sépare, seul l'environnement du shell les sépare.
La valeur de `TURN_SECRET` n'entre pas en jeu : seule sa **présence** compte
(`configurationIce` teste `if (!urls || !secret) return undefined`).

## Le défaut latent trouvé au passage

`garde-fil.test.ts` restaurait `process.env.TURN_URL = turnAvant.url`, où
`turnAvant.url` vaut `undefined` sur une machine sans TURN. `process.env` coerce
en chaîne : la variable ressortait à `"undefined"` — **truthy**. Mesuré :

```
apres restauration  TURN_URL = "undefined"
configurationIce rend : {"iceServers":[{"urls":"undefined", …}]}
```

**Jamais observé mordant** : l'ordre par taille de fichier place aujourd'hui
`server.test.ts` avant `garde-fil.test.ts`, et il aurait suffi que l'un des deux
change de taille pour l'inverser.

## La réparation, et sa rouge reproductible

- `plateforme/src/signaling/turn-harnais.ts` (neuf) : `poserTurnAmbiant()` pose
  l'état TURN voulu et rend la fonction qui rétablit l'état d'avant, `delete`
  compris pour les clés absentes.
- `server.test.ts` : neutralise TURN pour ses douze tests, **et** ajoute un
  `describe('avec un serveur TURN configuré')` qui mesure la configuration de
  PRODUCTION — l'`ice-config` arrive, puis le relais relaie. Sans lui,
  neutraliser aurait retiré ce cas de la couverture au lieu de le nommer.
- `garde-fil.test.ts` : passe par le harnais.

**Aucune ligne de produit n'est modifiée** (`git diff --quiet relais.ts` : vrai).

Deux rouges reproductibles à volonté, mesurées :

| Rouge | Geste | Relevé |
| --- | --- | --- |
| 1 | remplacer `poserTurnAmbiant()` par un no-op dans `server.test.ts`, TURN posé | **6 failed \| 335 passed (341)**, les six mêmes |
| 2 | muter `relais.ts` en `if (false && ice)` | le test neuf seul échoue : **1 failed \| 12 passed (13)** |

Après réparation : **341/341**, shell nu comme TURN posé, **2 exécutions chacun**.
`verify-all.sh` sort à **0** dans les deux environnements
(`verify-all-vert-source-env-corrige.log`).

## La seconde perte : l'arrêt au premier échec

Le script s'arrêtait à la première étape rouge. `test:postgres` et `typecheck`
n'étaient donc **jamais atteintes** pendant toute la durée du défaut — personne
ne savait si elles étaient vertes ; elles n'étaient pas mesurées.

Changé : les dix étapes sont jouées jusqu'au bout, la sortie reste 1 dès qu'une
seule échoue, et un récapitulatif nomme toutes les fautives. Les dix étapes sont
indépendantes (chacune est un `(cd X && …)` autonome), et la chaîne entière
tourne en **~30 s** tout en cache (mesuré : 08:48:32 → 08:49:02).

**Ce que le changement a immédiatement révélé** — rouge n°1 réinjectée, chaîne
complète, `verify-all-rouge-injecte-recapitulatif.log` :

```
══ 2 étape(s) sur 10 en ÉCHEC ══
  - plateforme : npm run test:sqlite
  - plateforme : npm run test:postgres
```

**Le défaut touchait DEUX étapes sur dix, pas une.** L'ancien script ne pouvait
structurellement pas le dire : il sortait avant de mesurer la seconde.

## Comptes du script

**10 appels d'étape** (`grep -c '^etape "'`), **18 en-têtes `==>`** au journal :
les **8** de plus viennent de l'intérieur d'un seul appel,
`client : npm run design:verifier` (son propre `npm run build` plus les sept
contrôles §7).

## Ce que ce relevé n'établit pas

- **Aucun taux** : 1 à 3 exécutions par condition, nommées ci-dessus.
- La rouge d'origine du 19 août n'a pas été rejouée sur son binaire d'alors :
  elle portait **338** tests là où HEAD en collecte 340. L'écart de 2 n'est pas
  expliqué — il vient d'un état de l'arbre que je n'ai pas reconstitué.
- `resilience.test.ts` transmet `...process.env` à son processus enfant et hérite
  donc de TURN, lui aussi. Il passe dans les deux environnements (mesuré), mais
  **il n'a pas été rendu hermétique** : sa sensibilité reste ambiante.
