# Lot 2 — les huit legs fermables sans VM ni humain

> **22 août 2026.** Deuxième lot de la finalisation. Cadrage d'ensemble :
> [`2026-08-21-finalisation-cadrage.md`](2026-08-21-finalisation-cadrage.md).
> Le lot 1 est clos ([sa spec](2026-08-21-page-derriere-pomerium-design.md)).

## 1. Ce que ce lot est, et ce qu'il n'est pas

**Il ferme les legs du § « Legs ouverts » de `CLAUDE.md` qui ne demandent ni la
VM Windows, ni un œil, ni une oreille, ni un arbitrage du propriétaire.**

🔴 **CE N'EST PAS UN LOT HOMOGÈNE, ET IL NE FAUT PAS LE TRAITER COMME TEL.** Huit
items, quatre sous-projets, deux langages. Ils n'ont en commun **que** leur
condition de vérifiabilité : `verify-all.sh` et les deux passes Vitest suffisent
à les juger. Ce qui les lie est une **propriété du montage de recette**, pas une
parenté de conception — et un plan qui les traiterait comme une famille
produirait des tâches artificiellement couplées.

**Il ne livre pas** — et c'est nommé ici pour qu'aucun successeur ne le lise
comme un oubli :

- 🔴 **Le WCO** (Window Controls Overlay), pourtant listé au cadrage. **Retiré
  par décision** : il est **inatteignable tant que le hub n'est pas
  installable**, et l'installabilité exige d'ouvrir une route aujourd'hui
  authentifiée — une **décision de sécurité qui appartient au propriétaire**
  (legs ④). Livrer le WCO avant elle serait livrer une fonction que rien ne peut
  exercer.
- Aucune **calibration**. Toutes les constantes que ce lot introduit sont **non
  calibrées**, comme toutes celles de ce dépôt, et chacune le dit sur place.
- Aucun **jugement d'usage**, visuel ou d'écoute.

## 2. L'ordre, et les deux contraintes qui le fixent

| # | Item | Sous-projet |
| --- | --- | --- |
| **A** | Le canal `Message` du capteur, **non borné** | ① |
| **B** | Les freins manquants : `GET /vm`, `POST /session`, le relais `/signal` | ⑤ |
| **C** | Le contrat de `SIGNALING_URL`, **figé par aucun test** | auth-pomerium |
| **D** | L'éviction des magasins | ④ ③ |
| **E** | `tokens.css` à **300/300** — l'extraction | ⑥ |
| **F** | `--accent-fenetre`, déclaré nulle part et peint par rien | ⑥ |
| **G** | Les **douze** pilotes de recette visant une racine fermée | auth-pomerium |
| **H** | Les galeries, sans aucun test | ⑥ |

🔴 **DEUX CONTRAINTES D'ORDRE, ET ELLES NE SONT PAS DES PRÉFÉRENCES :**

1. **E PRÉCÈDE F.** `client/src/design/tokens.css` est à **300 lignes sur 300**
   (relevé du 22 août 2026 — **le relancer**). Déclarer `--accent-fenetre` sans
   extraire d'abord franchirait le plafond, et `CLAUDE.md` **interdit nommément**
   de rattraper par une compression. C'est le patron « extraire, jamais
   comprimer », et « la marge regagnée par une extraction se reperd si on la
   traite comme acquise » — payé six fois.
   🔴 **LE NOMBRE DE LECTEURS EST À MESURER, PAS À RECOPIER.** `CLAUDE.md`
   annonce « onze lecteurs qui le nomment par son chemin » ; une commande large
   en rend **33** (relevé du 22 août 2026). Les deux peuvent être vrais de
   choses différentes — et c'est précisément le problème : **personne n'a
   énoncé la règle de sélection.** *Un sous-ensemble sans règle énoncée est un
   sous-ensemble choisi, même quand on ne l'a pas choisi.* L'extraction doit
   **énoncer sa règle, puis compter**, et corriger le chiffre de `CLAUDE.md`
   dans le même mouvement.
2. **A EST LE PLUS GROS, ET IL EST EN TÊTE.** Ce n'est pas la ligne d'une seule
   correction : c'est un changement de structure de canal, avec un invariant
   d'ordre à préserver. Le mettre en dernier ferait porter son risque par la fin
   du lot.

**Les six autres sont indépendants** et peuvent être menés dans n'importe quel
ordre.

---

## 3. A — Le canal `Message`, borné par coalescence

### 3.1 L'état, relevé et non recopié

`agent/src/capteur/sommeil/registre.rs` crée le canal par
`std::sync::mpsc::channel::<Message>()` — **non borné**. Un `Sender<Message>`
par session vit dans `registre.canaux`.

**Quatre variantes l'empruntent, ajoutées par quatre sous-blocs distincts**
(`agent/src/capteur/sommeil.rs`, `enum Message`) :

| Variante | Poussée | Ce que sa perte coûte |
| --- | --- | --- |
| `Sommeil(Ordre)` | à l'arbitrage | 🔴 **un ordre perdu** — la fenêtre reste endormie ou éveillée à tort |
| `Part { bps }` | **au changement seulement** | rien : seule la dernière valeur compte |
| `Audio { actif }` | **au changement seulement** | rien : idem |
| `PressePapier { texte, octets }` | au changement | 🔴 **la donnée de l'utilisateur** |

🔴 **L'INVARIANT QUE TOUTE CORRECTION DOIT PRÉSERVER, ET IL EST ÉCRIT SUR PLACE :**
« *au sein du canal d'UNE session, un canal unique garantit l'ordre de LIVRAISON
entre `Sommeil`, `Part` et `Audio`* ». **Séparer les variantes en canaux
distincts détruirait cette garantie** — c'est la solution qui vient d'abord à
l'esprit, et elle est fausse.

⚠️ Le même commentaire dit ce que l'invariant **ne** couvre **pas** : il ne
s'étend pas entre deux sessions, l'ancienne porteuse et la nouvelle ayant chacune
son canal.

### 3.2 La décision

**Une file par session, bornée, avec une politique PAR VARIANTE au dépôt.**

| Variante | Politique |
| --- | --- |
| `Part`, `Audio` (et toute variante idempotente future) | **coalescence EN PLACE** : si un message de la même variante est déjà en attente, sa charge est **remplacée** et sa **position conservée** |
| `Sommeil`, `PressePapier` | **jamais coalescés, jamais silencieusement perdus** |
| au-delà de la borne dure | **refus COMPTÉ et JOURNALISÉ**, jamais silencieux |

🔴 **POURQUOI LA COALESCENCE CONSERVE LA POSITION.** Remplacer en place préserve
l'ordre relatif avec `Sommeil` — qui est exactement ce que l'invariant du § 3.1
garantit. Déplacer le message coalescé en queue le ferait franchir un ordre de
sommeil déposé entre-temps, et livrerait une part de débit *après* un ordre de
dormir qui aurait dû la rendre caduque.

🔴 **POURQUOI CELA BORNE RÉELLEMENT.** Les trois variantes qui ont « aggravé » le
canal sont précisément les **idempotentes** : sous coalescence, elles cessent de
faire croître la file en régime permanent, quelle que soit la cadence d'arrivée.
Ce qui reste est le trafic d'ordres, rare et déjà auto-limité.

⚠️ **`std::sync::mpsc::Sender` NE PERMET PAS D'INSPECTER SA FILE.** La
coalescence exige donc une structure propre — une file sous verrou plus un
réveil — et **ce n'est pas un détail d'implémentation** : c'est le coût de la
décision, et il doit être assumé plutôt que contourné.

### 3.3 Ce que la recette doit établir

| # | Critère | Comment il rougit |
| --- | --- | --- |
| ① | deux `Part` successifs sans lecture ⇒ **une seule** en file, la **dernière** | compter la file, pas seulement lire la valeur |
| ② | `Part`, `Sommeil`, `Part` ⇒ **l'ordre relatif est conservé**, et la première `Part` porte la seconde valeur | 🔴 **le critère qui distingue la coalescence en place de la coalescence en queue** |
| ③ | `Sommeil` et `PressePapier` ne sont **jamais** coalescés | déposer deux fois chacun, en compter deux |
| ④ | au-delà de la borne, le refus est **compté et journalisé** | lire le compteur ET la trace |
| ⑤ | **témoin négatif** : sans coalescence, la file croît | sans lui, un « une seule en file » ne prouverait rien |

⚠️ **Tout cela est éprouvable SUR L'HÔTE** : la structure est pure, aucun `cfg`,
aucune API Windows. **Si l'implémentation exige la VM, c'est que la frontière a
été mal placée.**

---

## 4. B — Les freins manquants

`securite/frein.ts` n'est consulté que par `servirAuth`. `GET /vm`, `POST
/session` et le relais `/signal` ne sont **freinés par rien**.

### 4.1 La décision, et l'écart de sémantique qu'elle traverse

🔴 **`Frein` COMPTE DES ÉCHECS** (`echec`, `succes`, `BUDGET_COMPTE`,
`BUDGET_ADRESSE`) — c'est ce qu'il faut contre une attaque par mot de passe.
**Les trois routes à freiner n'ont pas de notion d'échec** : leur abus est un
volume de requêtes **réussies**.

**Décision : réutiliser la STRUCTURE de `Frein` — sa fenêtre glissante, son
plafond d'entrées, son éviction — avec un budget « toute requête » par adresse**,
plutôt que d'écrire un second mécanisme.

⚠️ **Ce que cela oblige à ne pas confondre** : une requête freinée par volume
n'est **pas** un échec d'authentification, et les deux comptes ne doivent pas
partager une clé — sinon un utilisateur actif épuiserait son propre budget
d'échecs, ou l'inverse.

⚠️ Les budgets sont **NON CALIBRÉS**, et chacun le dit sur place.

⚠️ **`PLATEFORME_PROXY_DE_CONFIANCE` mal posée dégénère le frein par adresse en
frein GLOBAL** — c'est déjà écrit dans `CLAUDE.md`, et cela vaut pour ces trois
routes comme pour `/auth/*`. L'annonce de démarrage posée par le lot 1 rend
l'ensemble retenu visible ; **c'est elle qui rend ce piège diagnosticable.**

---

## 5. C — Le contrat de `SIGNALING_URL`

**La variable est la BASE du service, jamais l'URL du relais** : `url_du_relais`
y ajoute `/signal`, `url_du_canal` y ajoute `/agent`. **Y écrire `/signal`
casserait l'enrôlement** (`ws://h:8080/signal/agent`) **sans qu'aucun test ne
bronche**.

⚠️ Le test existant `le_canal_agent_n_est_pas_affecte` d'`agent/src/signaling.rs`
passe une base **propre**, donc n'éprouve pas ce cas — alors que son commentaire
prétendait le fermer. Le commentaire a été corrigé ; **le test, lui, reste dû.**

**Décision** : un test qui passe une base portant **déjà** `/signal` et vérifie
que l'enrôlement casse — c'est-à-dire qui **fige le contrat par sa rouge**.

---

## 6. D — L'éviction des magasins

Le magasin d'applications (`PLATEFORME_ICONES`, `PLATEFORME_TELEVERSEMENTS`) et
le magasin du pont ne sont **jamais nettoyés** : le disque grossit.

### 6.1 La décision

**Éviction PAR ÂGE, avec un PLANCHER de référence.**

- Un objet **non touché depuis N** est évincé.
- 🔴 **SAUF s'il est référencé par une entrée VIVANTE du catalogue.** Le plancher
  est ce qui distingue une éviction d'une corruption : évincer une icône encore
  nommée par une application ferait disparaître son image sans que rien ne le
  dise.
- **N est NON CALIBRÉ.**

⚠️ **Ce que cette règle NE fait PAS, et qu'il faut dire** : elle ne borne **pas**
le disque. Un catalogue qui grossit sans cesse grossit sans cesse. Le plafond de
taille a été **écarté par décision** parce qu'il peut évincer un objet encore
référencé — c'est-à-dire échanger une croissance visible contre une panne
silencieuse.

### 6.2 Ce que la recette doit établir

Un objet vieux **et non référencé** part ; un objet vieux **et référencé** reste ;
un objet jeune reste. 🔴 **Les trois, sinon le premier ne prouve rien** — une
éviction qui emporte tout passerait le premier critère.

---

## 7. E puis F — `tokens.css`, puis le token que rien ne peint

### 7.1 E — l'extraction

`client/src/design/tokens.css` est à **300/300**. **Extraction en tâche dédiée,
AVANT celle qui ajoute.**

🔴 **COMBIEN DE LECTEURS ? LE CHIFFRE DE `CLAUDE.md` NE TIENT PAS.** Il annonce
**onze** « lecteurs qui le nomment par son chemin » ; une commande large en rend
**33** (relevé du 22 août 2026). **Ni l'un ni l'autre n'est utilisable tant que
la règle de sélection n'est pas écrite** — un import de `tokens.ts` n'est pas un
lecteur de `tokens.css`, une mention en commentaire non plus, et un `@import`
CSS en est un. **Énoncer la règle, PUIS compter, PUIS corriger `CLAUDE.md`.**

⚠️ Le vérificateur de design en fait partie, et pas par un seul chemin : le
contrôle §7.6 vit dans `client/outils/tokens-orphelins.mjs`, appelé par
`client/outils/verifier-design.mjs`. **Ses contrôles doivent continuer de voir
ce qu'ils voyaient** — c'est le garde de l'extraction.

### 7.2 F — `--accent-fenetre`

**L'état, relevé** : `client/src/accent-dom.ts` **pose** le token
(`TOKEN_ACCENT = '--accent-fenetre'`), et **aucune feuille ne le lit**. Il n'est
déclaré nulle part.

🔴 **ET LE GARDE QUI DEVRAIT LE DIRE NE PEUT PAS ROUGIR.** Le contrôle §7.6
(`client/outils/tokens-orphelins.mjs`, « aucun token orphelin, aucun `var()` non
déclaré ») signale un token non déclaré **employé par un `var()` dans du CSS** —
or aucun CSS ne l'emploie. **Le contrôle est structurellement incapable de dénoncer ce
cas-ci**, et son propre commentaire dans `accent-dom.ts` l'avait vu sans en tirer
la conséquence. C'est le patron que ce dépôt place au sommet.

**Décision, en deux temps qu'il ne faut pas confondre :**
1. **Déclarer le token** et **le faire peindre par une feuille**, avec un repli
   (`var(--accent-fenetre, var(--accent))`) — sans quoi il reste indéfini avant
   le premier message.
2. 🔴 **Rendre le garde capable de rougir** : le vérificateur doit dénoncer un
   token **posé par le JS** et déclaré nulle part, pas seulement un token employé
   par du CSS. **Et cette rouge doit être JOUÉE** — sinon on aura remplacé un
   contrôle aveugle par un autre.

---

## 8. G — Les douze pilotes de recette

Le relais a quitté la racine `/` pour `/signal`, et **douze** pilotes posent une
URL **sans chemin** (relevé du 21 août 2026 : `accent-a1`, `micro-e2`,
`micro-e3` — le pilote **et** son `injection-e3.js` —, `pont-fichiers` f1 à f5,
`presse-papier` p1 à p3. **Relancer le relevé** : `grep -rln "signaling=" --include='*.mjs' --include='*.js' docs/superpowers/`).

⚠️ **Ce n'est pas une substitution aveugle** : `?signaling=` reste **explicite**
et ne reçoit pas le suffixe. Une substitution qui ne distinguerait pas les deux
casserait les pilotes autrement.

⚠️ **Ces pilotes ne peuvent pas être exécutés par ce lot** (ils exigent la VM).
Le lot les **répare** ; il ne les **rejoue** pas, et le document de résultats
doit le dire.

---

## 9. H — Les galeries

`client/src/design/galerie.ts` et `galerie-primitives.ts` n'ont **aucun test**.

⚠️ **Ce qu'un test de galerie peut établir, et ce qu'il ne peut pas** : il peut
figer ce que la galerie **rend** — la liste des cas, leur nommage, l'absence de
doublon, la présence de chaque primitive. **Il ne peut porter aucun jugement
visuel** : celui-là attend un œil, et c'est le lot 4.

---

## 10. Ce que ce lot ne peut pas établir

- 🔴 **Aucun des vingt-cinq jugements visuels de ⑥.** Ce lot ajoute des tests de
  structure ; il ne regarde rien.
- 🔴 **Aucun des douze pilotes n'est rejoué.**
- 🔴 **Aucune constante introduite n'est calibrée.**
- Le canal `Message` est éprouvé **sur l'hôte** : son comportement **en charge
  réelle**, sur la VM, relève du lot 3.
