# Lot 34 — la session de contrôle se REPREND, et le relais dit à l'agent qui arrive qu'on l'attendait

**31 août 2026.** Branche `package-nivuus`. Commits `865ebf1` (agent),
`2f42e81` (plateforme).

> ⚠️ **Document VERSIONNÉ à dessein.** Le rapport de tâche vit sous
> `.superpowers/`, qui est **gitignoré** : ce dépôt a déjà perdu six constats
> de revue de cette façon, et deux legs déclarés « ouverts » alors que le
> produit les avait résolus.

✅ **DÉPLOYÉ EN PRODUCTION LE 31 AOÛT 2026** — plateforme à 11:43:03Z,
agent à 11:47:35Z (**un seul redémarrage d'agent**, groupé avec le correctif
du lot 33 : voir le § 6).

---

## 1. Les deux défauts, et pourquoi ils sont UN

Ils vivent dans le même trajet — superviseur ↔ plateforme ↔ page-shell — et
le second ne peut pas être réparé tant que le premier tient.

**① La session de contrôle du superviseur ne se reconnectait JAMAIS.**
`agent/src/superviseur/signalisation.rs` ouvrait son socket une fois, au
démarrage. Ses deux tâches de fond sortaient de leur boucle à la première
chute, journalisaient `connexion de contrôle au signaling perdue`, et **plus
rien** : `rx_shell` fermé, `envoyer` écrivant dans un canal sans consommateur
(`let _ = …`), et `boucle::tourner` continuant de tourner en croyant parler à
quelqu'un. Legs n°1 du lot 17.

**② Une fenêtre ouverte plus de 30 s avant la connexion du navigateur était
perdue.** 🔴 **CE LEGS ÉTAIT PÉRIMÉ AU MOMENT OÙ ON ME L'A CONFIÉ, ET C'EST
LA PREMIÈRE CHOSE QUE CE LOT A ÉTABLIE** — par les dates, pas par une
impression :

| Commit | Date | Ce qu'il fait |
| --- | --- | --- |
| `5d01f1e` | 30 août, **00:43:11** | écrit le legs dans `CLAUDE.md` |
| `6a7c98e` | 30 août, **02:49:53** | **le corrige** (lot 17, `pair-present`) |
| `f4e25f4` | 30 août, 03:14:53 | mesure la correction : 0 / 4 / 5 fenêtres |

`git merge-base --is-ancestor 5d01f1e 6a7c98e` : le legs a été écrit **deux
heures six avant** son propre correctif, et le lot 17 ne l'a pas mis à jour.
**C'est le patron du legs `403/404`, payé une seconde fois** : un § « Legs
ouverts » consolidé à la main vieillit comme n'importe quel relevé daté.

**Ce qui restait réellement dû de ② n'est donc pas la borne de 30 s** — elle
est intacte, elle court depuis l'arrivée de la shell, c'est le bon
mécanisme — **mais TROIS résidus** : (a) tout le mécanisme du lot 17 est
inopérant quand le socket de ① est mort ; (b) si la shell revient **avant**
l'agent, personne ne prévient l'agent ; (c) une fenêtre `Vivante` n'est jamais
redite. Ce lot ferme (a) et (b). **(c) est laissé au propriétaire, dossier au
§ 7.**

---

## 2. Le bras ROUGE, mesuré sur la VM de production

Ancien binaire (`B6B984E8…`, MSVC/mingw d'avant ce lot), `desk-plateforme`
redémarré à 11:43:03Z. Segment d'`agent.log` ouvert à 11:42:49Z (octet
27 620 015), fermé à 11:47:2xZ (octet 27 645 917) — **segmenté par
HORODATAGE, jamais par un marqueur écrit dans un journal que l'agent tient**.

```
lignes du segment ................................. 111
JUGE   « session de contr(ôle) » .................... 0
       connexion de contrôle au signaling perdue .... 1   (11:43:02.751451Z)
TÉMOIN reprise du canal /agent ...................... 1
TÉMOIN pont fichiers lancé .......................... 3
TÉMOIN émission vers la shell échouée ............... 1
```

**La seule ligne du segment entier qui porte `contr` est la ligne de perte.**
Relevé de nouveau à 11:46:29Z, soit **3 min 27 s après la coupure** : les
mêmes chiffres.

🔵 **POURQUOI CE ZÉRO EST INTERPRÉTABLE.** Les trois témoins sont pris dans le
**même segment** : le processus est vivant, et ses deux autres mécanismes
supervisés — le canal `/agent` et le pont fichiers — **se rétablissent tous
les deux**. Le zéro n'est donc pas celui d'un produit en panne : c'est celui
du seul socket qui ne sait pas revenir.

🔴 **ET LE PREMIER JUGE QUE J'AI ÉCRIT ÉTAIT VACUEUX — CONSIGNÉ PLUTÔT QUE
CORRIGÉ EN SILENCE.** Je comptais `declaration du superviseur`, **sans
accent**, contre un produit qui écrit `déclaration`. Ce motif ne pouvait
**structurellement pas** rendre autre chose que zéro : « un zéro rendu par une
chaîne que le produit n'émet nulle part n'est pas une mesure », et je l'ai
rejoué. Le juge ci-dessus (`session de contr`) l'a remplacé, et **il porte son
propre témoin positif** : le MÊME motif rend **5** dans le segment vert (§ 3).

---

## 3. Le bras VERT, même machine, même geste

Binaire neuf (`CDEB2197…`), `desk-plateforme` redémarré à 11:47:57Z.

```
2026-08-31T11:47:57.108507Z WARN  lecture de la session de contrôle en erreur
2026-08-31T11:47:57.108556Z WARN  session de contrôle PERDUE : reconnexion programmée
                                  vecu_ms=31672 repli_rearme=false
                                  prochaine_tentative_dans_ms=500
2026-08-31T11:47:59.150162Z INFO  déclaration du superviseur émise sur la session de contrôle
2026-08-31T11:47:59.150197Z WARN  session de contrôle RÉTABLIE … tentative=1
                                  delai_ms=500 messages_jetes=0
```

**2,04 s entre la perte et le rétablissement.** Le même motif `session de
contr` rend **5** ici contre **0** au bras rouge.

🔵 **Trois choses que ce relevé établit en plus du rétablissement :**
- `repli_rearme=false` avec `vecu_ms=31672` : le seuil de réarmement
  (`SEUIL_CONNEXION_UTILE_MS`, 35 s) est **réellement consulté**, et la
  connexion, née 31,6 s plus tôt, tombe du bon côté. Un seuil non branché
  aurait rendu `true`.
- `lecture de la session de contrôle en erreur` : le bras `Err` que ce lot
  ajoute est **sur le chemin vivant**, pas décoratif. Avant ce lot, ce cas
  partait dans un `continue`.
- `messages_jetes=0` : le compte est émis **inconditionnellement**, zéro
  compris — c'est son propre témoin négatif.

### Deux coupures de plus, et les DEUX côtés du seuil

`desk-plateforme` a été redémarré trois fois au total sur le binaire neuf.
Les trois coupures ont été suivies d'un rétablissement, et elles se
complètent :

| Coupure | `vecu_ms` | `repli_rearme` | Rétabli en |
| --- | --- | --- | --- |
| 11:47:57Z | 31 672 | **false** | 2,04 s |
| 11:55:02Z | 423 791 | **true** | 2,55 s |
| 11:58:15Z | — | — | 2,54 s |

🔵 **Les deux côtés de `SEUIL_CONNEXION_UTILE_MS` (35 s) sont donc exercés sur
le produit réel**, et pas seulement dans les tests d'hôte : une connexion de
31,6 s ne réarme pas le repli, une de 7 min 3 s le réarme.

⚠️ **Les trois rétablissements ont abouti à `tentative=1`** : la branche
d'échec de reconnexion (`reconnexion de la session de contrôle ÉCHOUÉE`)
**n'a jamais tiré en production** — `aboutit` = 0 dans le journal. Le repli
n'a donc été exercé qu'à sa première marche. Voir le § 7.

---

## 4. Ce que les tests d'hôte tiennent, et les rouges vues rouges

**`agent/src/superviseur/reprise_controle.rs`** — la décision PURE, 7 tests.
Quatre mutations du produit, chacune restaurée depuis une **copie nommée**
(jamais `git checkout`, qui restaure HEAD et non l'état d'avant) :

| Mutation | Rougit |
| --- | --- |
| `SEUIL_CONNEXION_UTILE_MS` retombe à 500 ms (le défaut du round 3 de `relance_pont`) | l'invariant **et** le refus en boucle |
| `<` devient `<=` à la frontière | la frontière **et** le réarmement |
| `tentative_lancee` ne compte plus | **5 tests sur 7** |
| `connexion_terminee` réarme toujours | la frontière **et** le refus en boucle |

⚠️ **UNE ROUGE EST RESTÉE VERTE, ET ELLE A ÉTÉ DIAGNOSTIQUÉE, PAS CLASSÉE.**
`une_connexion_refusee_ne_rearme_jamais_le_repli` était d'abord écrit avec une
vie de **3 ms** — l'ordre de grandeur mesuré d'un refus réel — et restait vert
sous la première mutation. Il éprouve désormais `REPLI_MAX_MS`, **une durée
tirée du PRODUIT** (le plafond du repli, le pire cas qu'un épisode de refus
puisse occuper) et jamais d'un calcul sur ce qu'il juge.

**`plateforme/src/signaling/pair-present.test.ts`** — 9 tests (4 neufs), et
quatre mutations :

| Mutation | Rougit |
| --- | --- |
| `prevenirLArrivant` rend toujours `false` (le remède retiré) | 3 tests, dont le bout-en-bout sur socket réel |
| `prevenirLArrivant` rend toujours `true` | la règle pure **et** l'exclusivité |
| le CÂBLAGE disparaît de `relais.ts`, la règle restant juste | le bout-en-bout seul |
| la garde `if (pairEnFace)` disparaît | « ne se laisse pas FABRIQUER » (2 messages au lieu d'1) |

⚠️ **LA QUATRIÈME A CORRIGÉ UNE AFFIRMATION FAUSSE DE MON PROPRE
COMMENTAIRE.** J'avais écrit que le témoin négatif tenait la garde
d'appariement ; il ne la tient pas (`send(undefined, …)` est déjà un no-op).
Le commentaire dit désormais ce qu'il n'établit pas, et nomme le test voisin
qui le tient.

⚠️ **`plateforme/src/signaling/relais.ts` EST À 468 LIGNES APRÈS CE LOT**
(460 avant : +8). Il ne franchit pas le plafond, mais il n'en est plus qu'à
32 lignes : **la prochaine addition substantielle doit s'accompagner d'une
extraction, jouée AVANT elle et dans sa propre tâche**. Relevé daté du
31 août 2026 — la source de vérité reste la commande de `CLAUDE.md`,
relancée.

Suites complètes, toutes vertes : `cargo test --workspace` (1143 + 114),
`cargo check --target x86_64-pc-windows-gnu`, `cd client && npx vitest run`
(599) **et** `npx vitest run --dir ../proto` (308) — deux commandes,
`tsc --noEmit` des deux côtés, `npm run test:sqlite` (749),
`env -u TURN_URL -u TURN_SECRET ./scripts/verify-all.sh` (10 étapes).

---

## 5. Le piège qui aurait rendu le remède PLACEBO

🔴 **Le jeton est relu à CHAQUE tentative depuis la veille d'identité, jamais
`config.jeton`.** `main.rs` porte déjà l'argument, en toutes lettres, pour le
LANCEUR : « un superviseur vit des heures ; le jeton d'agent, lui, dure dix
minutes et se renouvelle à chaque battement ». `superviseur::executer`, lui,
passait l'instantané du démarrage à `signalisation::connecter` — sans
conséquence tant que la connexion n'était faite qu'une fois.

**Une reconnexion présentant ce jeton aurait été refusée à chaque tentative,
pour toujours.** Le remède aurait fonctionné sur une coupure d'une minute et
plus jamais après dix — la pire forme d'un correctif, celle qui se mesure
verte.

### 🔵 Et ce n'est PAS resté une lecture de code : c'est mesuré

Trois faits, pris dans la même exécution :

| Fait | Valeur |
| --- | --- |
| `expire_a` du jeton de DÉMARRAGE (`1788177446379` ms, journal de l'agent) | **11:57:26Z** |
| `desk-plateforme` redémarré | 11:58:16Z |
| `session de contrôle RÉTABLIE`, **acceptée** | **11:58:18.006Z** |
| `agent enrôlé auprès de la plateforme` (canal `/agent`) | 11:58:18.153Z |

**La reconnexion a été acceptée 52 s après l'expiration du jeton de
démarrage, et 147 ms AVANT que le canal `/agent` ne se réenrôle.** Le jeton
présenté n'est donc ni l'instantané du démarrage (mort), ni celui d'un
enrôlement neuf (pas encore obtenu) : c'est un jeton **rafraîchi**, tenu par
la veille d'identité. `journalctl -u desk-plateforme --since 11:58:00` ne
porte **aucun** refus de poignée de main.

🔵 **LE DISCRIMINANT EST VÉRIFIÉ, PAS SUPPOSÉ** : la garde rejette bien un
jeton expiré (`plateforme/src/identite/garde.ts`, motif `jeton-expire`), et
elle l'a fait **17 fois aujourd'hui** sur des sessions réelles
(`poignée de main refusée : jeton refusé (expire)`). Un succès à 11:58:18
n'est donc pas celui d'un service permissif.

---

## 6. Le déploiement, et ce qu'il a coûté

**Un seul redémarrage d'agent**, groupé avec le correctif du lot 33 (la
bordure d'un pixel), qui était déjà commité dans la branche : `f4ce400` est
ancêtre du commit bâti, vérifié par `git merge-base --is-ancestor`.

**Attestation À DESTINATION**, jamais à la source, avec témoin négatif **pris
sur le binaire d'hier au même endroit** :

| Chaîne | Ancien binaire en place | Neuf, après copie |
| --- | --- | --- |
| `reconnexion programm` (posée par moi) | **0** | **1** |
| `messages_jetes` (posée par moi) | **0** | **1** |
| `jamais rejou` (posée par moi) | — | **1** |
| `pont fichiers lanc` (préexistante) | **2** | **2** |
| `reconnexion programmXYZ` (témoin négatif) | **0** | **0** |

sha256 identique à la source, au partage et à destination
(`CDEB2197EEC32FCB4F00018D4503B488EBE0D797B0F61B5BC41ACACB530407F6`,
20 564 743 octets). Sauvegarde en place :
`C:\nivuus\agent\agent.exe.sauvegarde-lot34` (20 538 719 octets). Processus
arrêtés **par PID relevé**, jamais par motif. Session 1 attestée
(`C:\nivuus\state\agent-session.txt` = `1`).

⚠️ **CE QUE LE PROPRIÉTAIRE A PAYÉ** : un redémarrage d'agent, donc **toutes
ses fenêtres ouvertes orphelinées** (legs du lot 32I : le job d'appartenance
ne survit pas au processus qui le crée). Il doit relancer ses applications
depuis le hub. Et **quatre redémarrages de `desk-plateforme`**, donc autant de
rechargements de sa page — le premier était le bras ROUGE lui-même, les trois
suivants les bras VERTS (dont un provoqué exprès après l'expiration du jeton,
§ 5). Depuis le second, **l'agent se rétablit seul en ~2,5 s** : ces
rechargements sont ceux de la page, plus ceux de l'agent.

---

## 7. 🔴 Ce que ce lot N'ÉTABLIT PAS

1. **LA JONCTION N'EST PAS MESURÉE EN PRODUCTION.** La chaîne complète est
   « l'agent se reconnecte → la plateforme lui dit qu'un client attend →
   l'agent réannonce ». Le premier maillon est mesuré sur la VM (§ 3), le
   troisième l'a été par le lot 17 sur cette même VM (4 et 5 fenêtres
   réannoncées), **le deuxième ne l'est que par test contre le code réel du
   relais** (socket `ws` véritable, 4 mutations rouges). Il ne peut pas
   l'être en production : l'exercer demande un pair `client` sur la session de
   contrôle, or **le rôle `client` est exclusif** et le prendre déconnecterait
   le propriétaire. `pair-present` = **0** et `viewport recu` = **0** dans le
   segment vert : sa page n'était pas connectée.
2. ✅ ~~**LE CHEMIN DU JETON RAFRAÎCHI N'EST PAS MESURÉ**~~ — **il l'est,
   voir le § 5.** *(Cette réserve était juste de la PREMIÈRE coupure, à 32 s
   du démarrage ; une troisième coupure a été provoquée exprès après
   l'expiration du jeton initial pour la lever. Barrée plutôt qu'effacée.)*
3. **LE CRITÈRE PIXEL DU LOT 33 EST NON JOUÉ, ET SON RELEVÉ EST NON
   CONCLUANT.** La sonde a bien tourné en **session 1** — mais sur une fenêtre
   `Notepad` que `desk` **ne sert pas** : après le redémarrage, la règle
   d'appartenance n'adopte plus aucune fenêtre préexistante et aucune shell
   n'était connectée pour en ouvrir de neuves (`fenetres placees` = 0,
   `sorties creees` = 0, 3 processus agent = superviseur + capteur + pont).
   Le rectangle lu est `1264x743+8+1`, sans rapport avec le `1548x1032+1280+0`
   d'une fenêtre servie, et la capture est **occultée** par la console
   PowerShell de la tâche planifiée (`#012456` sur les trois rangées basses).
   🔴 **`colonne 0` y ressort sombre pendant que `colonne 1` est blanche —
   c'est-à-dire EXACTEMENT la forme du symptôme, pour la mauvaise raison.**
   Le relevé est conservé hors du dépôt et **ne doit pas être lu comme un
   verdict**. Le critère se rejoue en deux gestes : le propriétaire ouvre une
   application depuis le hub, puis on relance la sonde.
4. **RIEN N'EST ÉTABLI SUR UNE COUPURE LONGUE, NI SUR LA BRANCHE D'ÉCHEC.**
   Les **trois** rétablissements ont abouti à `tentative=1`, et la trace
   `reconnexion de la session de contrôle ÉCHOUÉE` n'a **jamais** été émise
   (`aboutit` = 0). Le repli n'a donc été exercé qu'à sa **première** marche
   (500 ms) ; le plateau de 30 s, le comportement sur une plateforme morte une
   heure, et le volume de trace que cela écrirait (~120 lignes/heure, **calculé
   et non mesuré**) restent hors mesure. Seuls les tests d'hôte couvrent la
   rampe.
5. **AUCUN JUGEMENT D'USAGE.** Personne n'a regardé une page se remplir de
   nouveau après une coupure.

---

## 8. 🔴 Legs que ce lot laisse

1. **UNE FENÊTRE `Vivante` N'EST TOUJOURS PAS REDITE À UNE SHELL QUI ARRIVE —
   ET C'EST DÉSORMAIS LE SYMPTÔME DOMINANT.** `reannoncer_les_attentes` ne
   redit que les entrées en `AttendLeViewport`, et `recenser_les_fenetres_
   existantes` ne rattrape que les fenêtres ABANDONNÉES (`fenetre_apparue`
   est idempotente par `HWND`). Une fenêtre dont l'enfant tourne reste donc
   invisible à toute shell rechargée. **Ce n'est pas un oubli, c'est la borne
   de la conception** : l'enfant consomme **une** offre et ne renégocie jamais
   (`agent/src/demarrage.rs`), donc redire la fenêtre ferait ouvrir une page
   qui enverrait une offre que personne ne prendrait. **DÉCISION DU
   PROPRIÉTAIRE**, dossier ci-dessous.
2. **UN SOCKET À MOITIÉ OUVERT PEUT FAIRE REFUSER LA RECONNEXION UN TEMPS.**
   Si l'agent perd son socket sans que la plateforme le voie (coupure réseau
   sans FIN), l'entrée `agent` reste dans l'appariement et la reconnexion est
   refusée en « un agent est déjà connecté ». **Il n'y a AUCUN battement
   applicatif sur `/signal`** (`grep` sur `relais.ts` et `http/serveur.ts` :
   aucun `ping`/`pong`/`isAlive`) : seul le délai TCP de l'hôte tranche. Le
   repli tient la cadence et le refus est désormais **tracé**
   (`session de contrôle REFUSÉE par la plateforme`), mais la fenêtre
   d'indisponibilité n'est ni bornée ni mesurée.
3. **LE PRÉFIXE DE SESSION N'EST PAS RELU À LA RECONNEXION.** Seul le jeton
   l'est. Si un réenrôlement délivrait un préfixe différent, l'agent
   rouvrirait l'ancienne session de contrôle. Non observé (le préfixe dérive
   de la VM), non gardé, écrit ici plutôt que découvert.
4. **LA PAGE-SHELL, ELLE, NE SE RECONNECTE PAS** — `shell.ts::
   canalDeControlePerdu` affiche « Rechargez la page pour vous reconnecter ».
   **Délibérément non touché** : ce n'est pas une panne muette (le message est
   exact et actionnable), et une reconnexion automatique rappellerait
   `ouvrir()` pour chaque fenêtre déjà connue, donc **rechargerait les pop-ups
   vivantes** (`window.open(url, "guac-<session>")` vise une fenêtre NOMMÉE).
   Le remède juste suppose de résoudre le legs n°1 d'abord.

### Le dossier du legs n°1, pour le propriétaire

Trois voies, et ce que chacune coûte :

| Voie | Ce qu'elle exige | Ce qu'elle coûte |
| --- | --- | --- |
| **Rendre l'enfant renégociable** | une seconde offre acceptée par `demarrage.rs`, donc un changement du cycle de vie de la `PeerConnection` | le plus cher, le plus complet : une shell rechargée retrouve tout |
| **Réacquérir la pop-up sans la naviguer** | `window.open('', 'guac-<session>')` rend la fenêtre existante **sans la recharger** — mais ouvre une fenêtre blanche si elle n'existe plus | changement côté client seul, déployable sans toucher l'agent ; le cas « la pop-up a été fermée » reste à traiter |
| **Ne rien faire** | — | ce qui se passe aujourd'hui : après une coupure, le hub est vide pendant que les pop-ups continuent de diffuser |

⚠️ **Aucune n'est prise ici.** Elles ne sont pas équivalentes en risque : la
deuxième est réversible et bon marché, la première touche le cœur du
transport.
