# Spike — ouverture d'une fenêtre navigateur sans geste utilisateur

**Date** : 28 juillet 2026
**Statut** : Spécification validée, prête à planifier
**Portée** : Lever le risque n°1 du chantier D avant d'engager les chantiers A et B

---

## 1. Question posée

`2026-07-28-support-jeux-design.md` §5 D désigne un risque capable d'invalider le
modèle produit :

> **Risque n°1 — popup blocker** : `window.open()` déclenché sans geste
> utilisateur est bloqué par défaut dans tous les navigateurs. Une PWA installée
> a plus de latitude mais rien n'est garanti. **À valider en tout premier** : ce
> point peut invalider le modèle.

Le modèle en question est celui du §4 : **une fenêtre principale Windows = une
fenêtre navigateur**. C'est lui qui rend Steam gratuit, et avec lui Epic, GOG et
tout logiciel multi-fenêtres. S'il ne tient pas, les chantiers A et B — bâtis sur
l'hypothèse d'une fenêtre par session — devraient être partiellement défaits.

**La question n'est donc pas « `window.open()` passe-t-il ? » mais « le modèle
multi-fenêtres est-il viable, et par quel mécanisme ? ».** Un verdict négatif sans
repli validé ne tranche rien.

## 2. Décisions actées

| Décision | Choix | Justification |
|---|---|---|
| Position dans la roadmap | Avant les chantiers A et B | Recommandation explicite du cadrage jeu §8 : « à lever par un test isolé dès maintenant » |
| Plateforme testée | Chromium desktop, poste de l'utilisateur | Disponible immédiatement ; un succès y vaut a fortiori sur ChromeOS, plateforme cliente privilégiée |
| Exposition | `app.allanic.me` via Pomerium | Exigence utilisateur ; plus représentatif que localhost — le produit final sera derrière ce proxy |
| Occupation du port 3445 | Arrêt temporaire de l'ancien serveur Guacamole | Choix utilisateur. Aucune modification de `/etc/pomerium/config.yaml`, aucune modification de l'ancien code |
| Périmètre technique | Ni agent Rust, ni VM Windows, ni WebRTC | Un message WebSocket reproduit exactement la condition testée : l'absence d'activation utilisateur transitoire |
| Portée | 4 variantes, pas un test binaire | Le coût marginal est faible et l'on doit sortir avec un mécanisme utilisable, pas un verdict |

## 3. Environnement

### Ce qui est en place

- Pomerium route `https://app.allanic.me` vers `http://127.0.0.1:3445`, avec
  `allow_websockets: true` **déjà actif** (`/etc/pomerium/config.yaml`).
- Sa politique autorise l'email de l'utilisateur, et laisse passer sans
  authentification les chemins finissant par `manifest.json`, `.ico` et `.png`.
  Ces exceptions attestent qu'installer une PWA derrière Pomerium a déjà exigé un
  contournement — fait à retenir pour le chantier D.
- Le port 3445 est occupé par le conteneur `guacamole-web-1`
  (`network_mode: host`, `restart: always`), qui sert l'ancienne application.

### Ce que cela impose

**Le service worker de l'ancienne application est probablement déjà enregistré**
sur cet origin (`web/index.js:418` l'enregistre), et une PWA de l'ancien système
peut y être installée. Un service worker actif intercepterait les requêtes du
spike et rendrait tous les verdicts ininterprétables. Le protocole commence donc
par le neutraliser — il ne suppose pas son absence.

## 4. Protocole

### Étape 0 — hygiène de l'origin

Page `/reset` qui :

1. énumère les service workers enregistrés sur l'origin et les désenregistre ;
2. vide les caches (`caches.keys()` → `caches.delete()`) ;
3. **affiche ce qu'elle a trouvé avant de le supprimer.**

Le point 3 n'est pas décoratif : découvrir un service worker de l'ancienne
application est en soi un résultat, celui qui explique pourquoi le spike aurait
été faux sans cette étape.

L'utilisateur désinstalle également l'ancienne PWA si elle est présente.

Le spike occupant seul l'origin pendant le test, son manifest prend `scope: "/"`.
C'est la configuration la plus proche du produit final, et elle est sans risque
de collision une fois l'étape 0 passée : l'ancien serveur est arrêté et son
service worker désenregistré.

### Les quatre variantes

Chaque variante est déclenchée par un message WebSocket émis par le serveur, et
**jamais** par le clic sur son propre bouton — c'est toute la condition testée.

| # | Variante | Attendu | Ce qu'elle établit |
|---|---|---|---|
| 1 | Onglet normal, sans activation | bloqué | Témoin. S'il passe, c'est le test qui est faux, pas le navigateur qui est permissif |
| 2 | PWA installée (`display: standalone`), sans activation | **inconnu** | Le cas nominal espéré : la fenêtre s'ouvre seule quand le jeu est prêt |
| 3 | Armement sur le prochain `pointerdown` | **inconnu** | Le plus prometteur. Même technique que celle déjà retenue pour le plein écran (cadrage jeu §4.1) : si elle vaut ici aussi, un seul mécanisme couvre les deux besoins |
| 4 | `showNotification()` → `notificationclick` → `clients.openWindow()`, fenêtre PWA **en arrière-plan** | marche en principe | Le repli documenté. Ce qui est réellement vérifié est le cas réel : hors focus, pendant que l'utilisateur fait autre chose et qu'un jeu finit de charger |

### Ce qui reste manuel

Non automatisable de façon honnête, donc exécuté par l'utilisateur :

- l'installation de la PWA (variante 2) ;
- l'octroi de la permission notifications (variante 4) ;
- le clic que la variante 3 attend.

Chrome n'expose pas d'API CDP fiable pour installer une PWA. `--app=URL` produit
une fenêtre app-like qui **n'est pas** une PWA installée ; s'en contenter
fausserait précisément la variante 2, celle qui porte la question.

Tout le reste — serveur, déclenchement, journalisation, horodatage — est
automatique.

## 5. Interprétation : trois issues, pas deux

C'est le point où le spike peut se tromper lui-même. Derrière Pomerium, une
session expirée renvoie une redirection d'authentification, et une fenêtre qui
part sur l'IdP ressemble beaucoup à une fenêtre qui n'a pas pu s'ouvrir.

| Observation | Conclusion |
|---|---|
| `window.open()` renvoie `null` | Bloqué par le navigateur — le vrai négatif |
| Renvoie une référence, mais aucun signal de vie dans le délai imparti | Ouverte puis partie ailleurs : redirection d'authentification, ou page vide. **Pas un blocage** |
| Renvoie une référence et le message de vie arrive | Succès franc |

Le délai imparti dépend de ce qu'il mesure. Pour les variantes 1 à 3, il borne
un chargement de page : **3 s**. Pour la variante 4, il borne un temps de
réaction humain — le signal de vie ne peut arriver qu'après un clic sur la
notification : **60 s**. Même unité, phénomènes sans rapport ; un seuil unique à
3 s produirait un faux négatif systématique sur la variante 4.

La page ouverte signale sa vie dès son chargement. Sans
cette distinction, une session Pomerium expirée produirait un « bloqué » faux et
très convaincant — exactement le genre de conclusion hâtive que le ledger du
projet reproche aux rondes de diagnostic précédentes.

## 6. Livrable

`docs/superpowers/plans/2026-07-28-spike-multifenetres.md`, versionné, contenant :

- le tableau des quatre verdicts ;
- la version exacte de Chromium testée ;
- l'état de l'origin constaté à l'étape 0 (service workers et caches trouvés) ;
- une **recommandation tranchée** : soit « le modèle §4 tient, mécanisme retenu =
  X, coût UX = Y », soit « il ne tient pas, voici ce qui change dans le modèle
  produit ».

Le document est commité. Trois documents de mesure du jalon 1 n'existaient dans
aucun commit et allaient disparaître ; la revue finale a dû les rattraper.

## 7. Restauration

`docker compose start web` remet l'ancien serveur en service sur 3445. C'est une
étape explicite du plan, pas une intention : l'ancienne application reste le
système utilisable jusqu'à son remplacement
(`2026-07-27-refonte-produit-design.md` §11).

## 8. Critère d'arrêt

Dès qu'une variante ≥ 2 réussit franchement — fenêtre ouverte, coût utilisateur
inférieur ou égal à un clic — le modèle multi-fenêtres est validé et la roadmap
reprend son cours vers les chantiers A et B.

Les variantes restantes sont **exécutées quand même**. Savoir laquelle est la
moins coûteuse en UX sert directement l'implémentation du chantier D, et le coût
marginal est de quelques minutes.

## 9. Hors périmètre

- Le chantier D lui-même : détection des fenêtres par `SetWinEventHook`,
  filtrage « Alt-Tab-able », topologie WebRTC à N connexions, budget d'encodeurs
  NVENC. Le spike ne tranche que la faisabilité de l'ouverture de fenêtre.
- Le choix entre `Windows.Graphics.Capture` et Desktop Duplication
  (cadrage jeu §4, décision ouverte) — indépendant de ce test.
- La liaison réelle avec l'agent : aucune ligne de l'agent Rust n'est touchée.
- Les autres plateformes clientes (ChromeOS, Windows, macOS). Un succès sur
  Chromium desktop suffit à lever le risque ; un échec rouvrirait la question de
  ChromeOS, où le support des PWA multi-fenêtres est meilleur.
- Firefox et Safari, où le multi-fenêtres est déjà documenté comme non supporté
  (cadrage jeu §6.1).
