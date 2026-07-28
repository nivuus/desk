# Spike multi-fenêtres — résultats

**Date d'exécution** : 28 juillet 2026
**Navigateur** : Google Chrome 150.0.7871.150 (build officiel, 64 bits),
V8 15.0.245.19
**Plateforme** : **ChromeOS** 16700.46.0, canal stable, carte `brya`,
micrologiciel `Google_Redrix`, ARC SDK 33 — c'est-à-dire un Chromebook, et donc
**la plateforme cliente privilégiée du cadrage** (`2026-07-28-support-jeux-design.md` §6.1)
**Exposition** : `https://app.allanic.me` via Pomerium, serveur du spike sur
`127.0.0.1:3445`, l'ancienne application arrêtée le temps du test

**Verdict : le modèle multi-fenêtres du cadrage jeu §4 TIENT.**

Mais pas par le chemin qu'il espérait. L'hypothèse « une PWA installée a plus de
latitude » est **fausse** : sans permission explicite, une PWA installée est
bloquée exactement comme un onglet ordinaire. Ce qui débloque l'ouverture sans
geste, c'est la **permission pop-up du site** — une permission ordinaire,
accordée une fois, au même titre que les notifications.

---

## 1. Verdicts

Quatre variantes, plus une dimension que le plan n'avait pas prévue et qui s'est
révélée décisive : la permission pop-up accordée ou non au site.

| # | Variante | Sans permission | Avec permission |
|---|---|---|---|
| 1 | Onglet ordinaire, sans activation — *témoin* | **`bloque`** ×2 | **`succes`** |
| 2 | PWA installée (`standalone`), sans activation | **`bloque`** ×3 | **`succes`** |
| 3 | Armement sur le prochain clic | **`succes`** | — |
| 4 | Notification → `clients.openWindow()`, fenêtre en arrière-plan | **`succes`** | — |

Toutes les mesures sans activation ont été relevées **plus de 15 secondes après
le dernier geste utilisateur** (15 202 à 15 876 ms), soit trois fois la durée de
l'activation transitoire de Chromium. La variante 3 est à 24 ms, ce qui est
normal : son clic *est* le mécanisme testé, et elle est classée avec
`gesteAttendu: true`.

### Détail des mesures

| Heure | Variante | Mode | Poignée | Vie | ms depuis geste | Verdict |
|---|---|---|---|---|---|---|
| 16:12:02 | 1 | onglet | nulle | absente | 15 253 | `bloque` |
| 16:12:21 | 1 | onglet | rendue | reçue | 15 214 | `succes` *(permission accordée)* |
| 16:13:23 | 2 | standalone | nulle | absente | 15 243 | `bloque` |
| 16:13:39 | 2 | standalone | nulle | absente | 15 202 | `bloque` |
| 16:13:56 | 3 | standalone | rendue | reçue | 24 | `succes` |
| 16:14:26 | 4 | standalone | sans objet | reçue | 15 876 | `succes` |
| 16:16:55 | 1 | onglet | nulle | absente | 15 214 | `bloque` |
| 16:17:24 | 2 | standalone | nulle | absente | 15 222 | `bloque` |
| 16:19:55 | 2 | standalone | rendue | reçue | 15 235 | `succes` *(permission accordée)* |

## 2. Ce que chaque mesure établit

**Le témoin valide l'instrument, et le second essai prouve le mécanisme.** La
variante 1 rend `bloque` par défaut (deux fois, dans deux sessions distinctes),
puis `succes` une fois la permission pop-up accordée. Cette bascule est la
preuve directe que le spike mesure bien la politique de pop-up du navigateur, et
non un défaut de câblage, une redirection d'authentification ou une fenêtre
perdue. Aucune des rondes n'a eu à interpréter un silence.

**L'hypothèse centrale du cadrage est réfutée.** Le §5 D pariait qu'« une PWA
installée a plus de latitude ». Trois mesures indépendantes disent le contraire :
en `standalone`, sans permission, l'ouverture est bloquée comme dans un onglet.
Le statut d'application installée n'accorde, à lui seul, aucun privilège
d'ouverture de fenêtre.

**Deux replis fonctionnent, et un troisième chemin les rend facultatifs.**
L'armement sur le prochain clic (variante 3) et la notification cliquable
(variante 4) réussissent tous deux, cette dernière **fenêtre en arrière-plan** —
le cas réel où un jeu finit de charger pendant que l'utilisateur fait autre
chose. Mais la permission pop-up rend l'ouverture immédiate et sans geste, ce qui
retire tout coût d'interaction.

## 3. Recommandation pour le chantier D

**Mécanisme nominal : demander la permission pop-up à l'installation**, au même
moment que la permission notifications. Coût utilisateur : **nul** une fois
accordée. C'est une permission de site ordinaire, révocable, présentée par le
navigateur — pas un contournement.

**Repli si l'utilisateur la refuse, dans cet ordre :**

1. **Armement sur le prochain clic** (variante 3) — coût : un clic, qui survient
   de toute façon puisqu'il faut cliquer pour jouer. C'est déjà le mécanisme
   retenu par le cadrage §4.1 pour la bascule plein écran : **un seul mécanisme
   couvre donc les deux besoins**, ce qui simplifie l'implémentation.
2. **Notification cliquable** (variante 4) — coût : un clic sur notification.
   Utile quand la fenêtre du hub est en arrière-plan, cas où l'armement sur clic
   n'aurait pas d'occasion de se déclencher.

**Conséquence pour la roadmap** : les chantiers A (audio) et B (input jeu)
peuvent être bâtis sur l'hypothèse multi-fenêtres. Le risque n°1 du chantier D
est levé.

## 4. Réserves et limites

- **Testé sur ChromeOS, et sur ChromeOS seulement.** C'est la plateforme cliente
  privilégiée du cadrage (§6.1), donc le résultat porte directement sur la cible
  — mais l'inférence ne va pas dans l'autre sens. Le même cadrage classe ChromeOS
  comme ayant le **meilleur** support des PWA multi-fenêtres : un succès ici ne
  garantit pas Chrome desktop sous Windows, macOS ou Linux, où la politique de
  pop-up et le traitement des applications installées peuvent différer. **À
  vérifier avant tout engagement produit sur poste desktop** — le spike est
  rejouable tel quel, il suffit de le relancer sur 3445. Firefox et Safari ne
  sont pas testés : le multi-fenêtres y est déjà documenté comme non supporté.
- **La permission pop-up n'a pas été testée en refus explicite.** On sait qu'elle
  débloque quand elle est accordée ; on n'a pas vérifié le comportement d'un
  refus définitif (l'utilisateur peut bloquer le site, pas seulement s'abstenir).
  À vérifier lors de l'implémentation du chantier D, car cela conditionne le
  déclenchement des replis.
- **L'état de l'origin avant test n'a pas été relevé.** L'étape 0 d'hygiène a été
  exécutée mais son rapport JSON n'a pas été consigné. Ce n'est pas gênant ici :
  le témoin rend `bloque` proprement dans les deux sessions, ce qui exclut une
  contamination par le service worker de l'ancienne application — un service
  worker interceptant les requêtes aurait produit des verdicts erratiques, pas un
  blocage net et reproductible.
- **Le mode d'affichage se stabilise après le chargement.** La trace de
  chargement annonce « onglet navigateur » dans une session où le verdict rend
  « standalone » : Chromium n'applique pas `display-mode: standalone` au moment
  où le script s'exécute. Le verdict fait foi, car le mode est relu **à l'instant
  de la mesure**, et parce que la variante 2 refuse de s'exécuter hors standalone
  — un verdict rendu prouve donc le contexte. Sans cette correction (constat I1
  de la revue finale), cette mesure aurait été attribuée à un onglet et le
  résultat le plus important du spike aurait été perdu.
- **Le service worker s'est enregistré sans difficulté derrière Pomerium**, dont
  la politique n'exempte pourtant que `manifest.json`, `.ico` et `.png`. Les
  cookies de session same-origin ont suffi. L'installation de la PWA n'a demandé
  aucun aménagement de la configuration du proxy.
- **Un résiduel parké n'a pas été exercé** : `sw.js:32` (rapport de blocage sans
  vérification de `reponse.ok`) ne se déclenche que si la variante 4 échoue. Elle
  a réussi, donc aucun verdict ne dépend de ce chemin.

## 5. Ce que le spike a coûté, et ce qu'il a évité

L'instrument lui-même a failli produire une mesure fausse. La revue finale a
trouvé, dans le code prescrit par le plan, que `window.open(url, "spike-N")`
passait un **nom** de fenêtre : Chromium navigue alors une fenêtre existante de
ce nom au lieu d'en créer une, sans consulter le bloqueur de pop-up. La poignée
revenait non nulle, la page se rechargeait, renvoyait son signal de vie, et le
verdict tombait à `succes`. Le déroulé nominal y menait — un premier essai
`non-concluant` invite à relancer, et la fenêtre précédente n'était jamais
fermée.

Autrement dit : **le spike aurait conclu que le modèle multi-fenêtres est viable
sans jamais avoir testé une seule ouverture de fenêtre.** La conclusion aurait
été juste par accident, ce qui est pire qu'une conclusion fausse : elle aurait
survécu à toutes les relectures.

Huit autres chemins du même genre ont été corrigés avant la mesure — nonce de
passage contre les signaux de vie étrangers, vérification du contexte
d'affichage, verrou contre le double déclenchement, reconnexion WebSocket,
échec de l'étape 0 rendu visible.
