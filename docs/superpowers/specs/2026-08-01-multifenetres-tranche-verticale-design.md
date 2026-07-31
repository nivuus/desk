# Chantier D, sous-bloc D1 — tranche verticale multi-fenêtres

**Date** : 1ᵉʳ août 2026
**Chantier parent** : D (multi-fenêtres), `2026-07-28-support-jeux-design.md` §4 et §5 D
**Statut** : conception validée, plan à écrire

---

## 1. Pourquoi ce document, et pourquoi il ne couvre pas le chantier D entier

Le chantier D est décrit par son cadrage comme « le plus structurant et le plus
risqué ». Trois chantiers de mesure successifs l'ont rendu spécifiable — la voie
« un moniteur virtuel par fenêtre » est **reçue** à N=8, avec 90,1 i/s par
fenêtre en capture+encodage et zéro verdict faux
(`plans/2026-07-31-duplications-paralleles-resultats.md`). Reste à le construire.

Il touche au moins sept sous-systèmes largement indépendants : détection des
fenêtres, topologie d'affichage, N pipelines de capture et d'encodage, N
connexions WebRTC et N fenêtres navigateur, partage de la capacité réseau, cycle
de vie, plein écran. Les spécifier ensemble produirait un document que personne
ne peut exécuter d'un tenant.

**Découpage retenu** :

| Sous-bloc | Objet |
| --- | --- |
| **D1** | **tranche verticale : une 2ᵉ fenêtre Windows ouvre une 2ᵉ fenêtre navigateur, avec image, entrée et son** |
| D2 | cycle de vie complet (fermeture dans les deux sens, destruction d'un encodeur à chaud, purge) |
| D3 | partage de la capacité réseau entre N flux, et audio par fenêtre |
| D4 | plein écran (§4.1 du cadrage) + Keyboard Lock |
| D5 | mise en sommeil des fenêtres masquées (condition de viabilité au-delà de 8) |

**Le présent document ne spécifie que D1.** La méthode est celle qui a validé le
jalon 1 du projet : faire tomber le risque d'intégration en premier, en
traversant tous les étages sur le plus petit cas utile, puis élargir avec un
produit qui marche derrière soi.

---

## 2. État de départ — ce que le code fait aujourd'hui

**Un processus agent = une session = une fenêtre.** `demarrage.rs` cherche *une*
fenêtre par `WINDOW_TITLE` (`window::find_window_by_title`), construit *une*
`VideoSource` qui duplique le bureau et recadre la fenêtre, *une* `Session`
str0m, *un* injecteur d'entrée, *une* source audio. Le client ouvre *une* page
avec *un* `<video>`, paramétrée par `?session=`. Le signaling apparie deux pairs
(`agent`, `client`) par `session_id` et ne relaie que `offer` et `answer`.

**Ce qui existe déjà mais vit dans `diagnostics/`** — écrit pour mesurer, pas
pour tourner :

- le canal IOCTL vers le pilote SudoVDA, la création et la destruction de
  sorties virtuelles, la purge des orphelines
  (`diagnostics/multifenetre/moniteurs.rs`) ;
- la capture d'une sortie DXGI entière (`SourceDuplication`) ;
- la garde de destruction et le trait `PiloteAffichageVirtuel`
  (`moniteurs_virtuels.rs`) — celui-ci est déjà en logique pure et testé, il est
  réutilisable tel quel.

D1 doit **promouvoir** ce code en modules de production. Ce n'est pas du
rangement : le code de mesure n'a ni reprise sur erreur ni cycle de vie long, et
il est appelé depuis un arbre de modules (`diagnostics/`) dont rien en
exploitation ne doit dépendre.

---

## 3. Décisions actées

### 3.1 Superviseur + un processus enfant par fenêtre

Un processus par fenêtre, sous un superviseur qui détient le hook de détection,
le pilote de sorties virtuelles et le lancement des enfants.

**Motif** : le projet a documenté et payé trois fois le fait que ces API
échouent par **plantage du processus**, pas par code d'erreur — c'est la raison
pour laquelle les bancs de sonde tournaient un processus par voie. Le défaut de
libération des encodeurs est corrigé, mais son propre document énonce que
« 0 récidive sur 20 exécutions » **n'est pas une preuve d'absence**. Et le gel
de `IMFShutdown::Shutdown` dans un `Drop` reste un risque **ouvert et observé**
(1 fois sur 6 à N=4, cause non attribuée).

En mono-processus, l'un ou l'autre emporte toutes les fenêtres. Ici, une fenêtre
qui meurt ne tue qu'elle-même.

Bénéfices annexes : `demarrage.rs` reste presque inchangé, et l'appariement
client ↔ flux passe par le `session_id` du signaling existant, sans multiplexage
à inventer.

**Coût assumé** : un canal superviseur ↔ enfants à créer, et le plafond de 8
encodeurs a été mesuré **dans un seul processus** — rien ne dit s'il est par
processus ou global (voir §7).

### 3.2 L'audio : une seule fenêtre porteuse, désignée par le superviseur

Le loopback WASAPI capte le son de toute la session Windows, pas d'une fenêtre.
Deux enfants l'ouvrant chacun feraient entendre le même son deux fois, dans deux
fenêtres navigateur, désynchronisées.

Le superviseur passe donc `--audio` à **un seul** enfant : celui de la première
fenêtre. Les autres n'ouvrent jamais le loopback. Zéro coordination côté client,
et à N=1 le comportement est **exactement celui d'aujourd'hui** — aucune
régression possible sur l'existant.

**Limite assumée, reportée à D2** : si l'utilisateur ferme la fenêtre porteuse,
le son disparaît jusqu'à ce que le cycle de vie sache le redésigner.

**Évolution prévue en D3 — une bande son par fenêtre.** Windows sait faire du
loopback **par processus** depuis la build 19041 :
`ActivateAudioInterfaceAsync` sur `VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK`, avec
`AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS` en mode `INCLUDE_TARGET_PROCESS_TREE` ou
`EXCLUDE_TARGET_PROCESS_TREE` — le mécanisme derrière l'« Application Audio
Capture » d'OBS. Le changement porterait sur l'activation seule : format de
mixage, encodage Opus et horodatage restent identiques.

Trois nuances qui **changent la règle de désignation sans la supprimer** :

1. **C'est par arbre de processus, pas par fenêtre.** Une application à
   plusieurs fenêtres (navigateur, Steam et son overlay, IDE) rend le même son
   pour toutes. Il faudra désigner une porteuse **par arbre de processus** — le
   mécanisme de D1 change de granularité, il ne disparaît pas.
2. **Le son orphelin.** Ce qui ne vient d'aucune fenêtre capturée (sons système,
   application sans fenêtre Alt-Tab-able) serait perdu ; le rattraper demande un
   flux supplémentaire en mode `EXCLUDE` sur l'union des processus déjà captés.
3. **Non éprouvé sur cette VM.** Build relevée le 1ᵉʳ août 2026 : `20348`
   (Windows Server 2022), donc **supérieure au minimum de 19041**. C'est un
   numéro compatible, **pas une preuve que l'API répond** — Server peut différer
   sur les composants audio, et rien n'a été appelé.

Le point d'extension est le superviseur : passer d'« un enfant reçoit
`--audio` » à « chaque enfant reçoit `--audio-pid <arbre>` » ne touche que lui
et l'activation WASAPI.

### 3.3 La sortie virtuelle est créée à la taille du viewport annoncé

Pas de mise à l'échelle : image au pixel près, et cohérence avec ce que le §4.1
du cadrage exigera pour le plein écran (« le pilote doit annoncer une résolution
cohérente avec le viewport client, faute de quoi l'image subit deux mises à
l'échelle successives »).

**Conséquence sur la séquence de démarrage** : la fenêtre navigateur s'ouvre
d'abord, mesure son viewport, et **seulement ensuite** la sortie est créée et
l'enfant lancé. C'est ce qui impose le protocole du §5.

**Conséquence sur le redimensionnement** : hors périmètre de D1. Le pilote
SudoVDA n'expose aucun changement de mode documenté dans notre reconnaissance
(`plans/journaux-mesures-prealables/canal-de-controle.md` §5.2 : les six IOCTL
sont `ADD`, `REMOVE`, `SET_RENDER_ADAPTER`, `GET_WATCHDOG`, `DRIVER_PING`,
`GET_PROTOCOL_VERSION` — aucun `SET_MODE`). Retailler une fenêtre navigateur
déjà ouverte ne recréera donc pas sa sortie. L'adaptation réseau, qui change la
taille d'**encodage** et non celle de la source, continue de fonctionner.

### 3.4 Fermer une page ne ferme pas l'application Windows

Le cadrage laissait la question ouverte (« fenêtre navigateur fermée → `WM_CLOSE`
sur la fenêtre Windows, à confirmer comme comportement souhaité »). **Décision :
non.**

Un `Ctrl+W` mal placé ne doit pas détruire un travail non enregistré, et aucune
confirmation n'est possible depuis le navigateur. La page-shell garde la fenêtre
dans sa table et peut la rouvrir.

**Conséquence assumée** : une fenêtre Windows peut exister sans fenêtre
navigateur. La shell doit donc lister les fenêtres et permettre de les rouvrir —
ce qui tranche aussi son statut : **la shell est un bureau, pas un onglet
technique**.

### 3.5 Réception de D1

Ce qui est testable sans Windows l'est réellement (§6), plus une **démonstration
bout en bout** scrupuleusement décrite et son journal versé, comme les recettes
précédentes.

Pas de banc chiffré : D1 est un chantier de **produit**, pas de mesure. Les
chiffres de cadence sont pris (90,1 i/s par fenêtre à N=8) ; les reprendre sur
des applications réelles ne répondrait à aucune question ouverte de ce
sous-bloc. Ce qui reste à prouver, c'est que l'ensemble s'assemble.

---

## 4. Architecture

```
┌─ superviseur (agent.exe --superviseur) ────────────────┐
│  hook SetWinEventHook + pompe de messages (fil dédié)  │
│  filtrage Alt-Tab                                      │
│  pilote SudoVDA : créer / détruire / purger            │
│  placement des fenêtres sur leur sortie                │
│  lance et surveille les enfants                        │
└───────┬──────────────────────────────┬─────────────────┘
        │ signaling (session « bureau »)│ lance
        │                               │
┌───────▼─────────┐          ┌──────────▼──────────────────────────┐
│  page-shell     │          │ agent.exe --fenetre H --sortie 3    │
│  ouvre/ferme    │          │           --session w-B [--audio]   │
│  les fenêtres   │          │  = l'agent d'aujourd'hui, sauf que  │
│  annonce les    │          │    la source capture une SORTIE     │
│  viewports      │          │    entière au lieu de recadrer      │
└───────┬─────────┘          └──────────┬──────────────────────────┘
        │ window.open                    │ signaling (session w-B)
        └────────────► page d'application ◄┘
```

### 4.1 Le signaling ne gagne aucun rôle neuf

La session de contrôle réutilise les rôles existants sur un `session_id`
réservé (`bureau`) : le superviseur est l'`agent`, la page-shell est le
`client`. Le seul changement au serveur est d'**élargir la liste des types
relayés** — aujourd'hui `offer` et `answer` passent, tout le reste est rejeté
(`signaling/src/server.ts:133`). On y ajoute `fenetre-ouverte`,
`fenetre-fermee`, `viewport`.

C'est beaucoup moins de plomberie qu'un rôle supplémentaire, et le serveur reste
un relais sans logique métier.

Deux précisions que le plan ne doit pas avoir à inventer :

- **L'identifiant de session d'une fenêtre** est opaque, généré par le
  superviseur, unique par fenêtre et sans signification (ni le `HWND`, ni le
  titre, qui changent tous deux au cours de la vie d'une fenêtre).
- **Une seule page-shell à la fois.** Le serveur refuse déjà un second `client`
  sur une session occupée (`server.ts:93`) : une deuxième shell reçoit donc une
  erreur explicite plutôt que de dédoubler silencieusement les ouvertures. C'est
  le comportement voulu ; le multi-utilisateur est hors périmètre du produit.

### 4.2 Découpage des fichiers

La règle du projet est qu'aucun **nouveau** fichier ne naît au-dessus de 500
lignes. Le superviseur y arriverait d'un bloc : les frontières sont donc posées
en conception, pas après.

```
agent/src/superviseur.rs               orchestration, mince
agent/src/superviseur/fenetres.rs      filtrage Alt-Tab (pur, testé)
agent/src/superviseur/hook.rs          SetWinEventHook + pompe (glue Windows)
agent/src/superviseur/table.rs         machine à états fenêtre→sortie→enfant (pur, testé)
agent/src/superviseur/enfants.rs       lancement, surveillance, mise à mort bornée
agent/src/superviseur/protocole.rs     messages de la session « bureau »
agent/src/moniteurs_virtuels/pilote.rs glue IOCTL, PROMUE depuis diagnostics/
agent/src/source/sortie.rs             VideoSource sur une sortie DXGI entière
```

---

## 5. Flux

### 5.1 Ouverture d'une fenêtre

```
1. le superviseur démarre : purge les sorties orphelines, pose le hook,
   se connecte au signaling (session « bureau », rôle agent)
2. l'utilisateur ouvre la page-shell (rôle client, même session)
3. le superviseur énumère l'existant et annonce chaque fenêtre :
      → fenetre-ouverte { session: "w-B", titre: "Bloc-notes" }
4. la shell : window.open('/?session=w-B')
5. la page B mesure son viewport et le renvoie par window.opener.postMessage
      (même origine, référence directe — pas de BroadcastChannel nécessaire)
6. la shell → superviseur :  viewport { session: "w-B", 1600, 900 }
7. le superviseur crée la sortie à 1600×900, y place la fenêtre, la maximise,
   puis lance :  agent.exe --fenetre 0x5678 --sortie 4 --session w-B
8. l'enfant rejoint le signaling sur w-B, répond à l'offre, le média coule
```

**Une course à supprimer, et elle existe déjà aujourd'hui.** À l'étape 4, la
page B envoie son offre SDP dès qu'elle est prête, donc avant l'étape 8 où son
agent existe. Le signaling ne mémorise rien : `send(peer, …)` avec un pair
absent perd le message en silence (`signaling/src/server.ts:134`).

**Correctif retenu** : le serveur mémorise la **dernière offre** d'une session et
la délivre à l'agent dès qu'il se déclare. Quelques lignes, la course disparaît
par construction plutôt que par temporisation, et un défaut latent du signaling
est corrigé au passage — aujourd'hui il est masqué parce que l'agent démarre
toujours avant que le navigateur n'ouvre la page.

### 5.2 Fermeture

| Événement | Réaction |
| --- | --- |
| Fenêtre Windows détruite (`EVENT_OBJECT_DESTROY`) | le superviseur tue l'enfant, détruit la sortie, annonce `fenetre-fermee` ; la shell ferme la fenêtre navigateur |
| L'enfant meurt seul (plantage) | le superviseur constate la mort du processus, détruit la sortie, annonce `fenetre-fermee` — **c'est le bénéfice pour lequel le multi-processus a été choisi : il doit être réellement câblé, pas seulement possible** |
| Page d'application fermée par l'utilisateur | l'enfant voit sa session mourir et se termine ; **la fenêtre Windows reste ouverte** (§3.4) |

---

## 6. Gestion d'erreurs

| Situation | Comportement | Statut de la connaissance |
| --- | --- | --- |
| Vivier de sorties épuisé | refus propre de la Nᵉ fenêtre, annoncé à la shell, rien ne plante | plafond **mesuré à 10** (refus du pilote à la 11ᵉ, `ERROR_TOO_MANY_NAMES`, preuve par identité). Apollo puise au même vivier et n'a **jamais** été mesuré en concurrence : le refus peut tomber plus tôt |
| Plafond d'encodeurs atteint | l'enfant échoue à construire son encodeur, se termine avec un message net ; le superviseur libère la sortie et l'annonce | plafond **mesuré à 8 dans UN processus**, refus sur `SetOutputType` de la MFT NVIDIA (`MF_E_UNSUPPORTED_D3D_TYPE`). Ici les encodeurs sont dans des processus distincts : **on ignore si le plafond est par processus ou global** (§7) |
| Enfant gelé à l'arrêt | le superviseur le **tue** après un délai borné | traite un risque **ouvert et observé** : `IMFShutdown::Shutdown` non borné dans un `Drop`, gel constaté 1 fois sur 6 à N=4, cause non attribuée. En mono-processus il figerait la session entière ; ici il ne fige que sa propre fenêtre |
| Enfant qui plante | mort du processus constatée, sortie détruite, `fenetre-fermee` annoncée | mode de défaillance dominant du terrain, d'après les trois chantiers de mesure |
| Pilote VDD absent ou muet | refus explicite et journalisé, **aucune dégradation silencieuse** | pas de repli « capture bureau » en D1 : deux chemins de capture à maintenir dès le premier sous-bloc, pour un cas qui ne se produit pas sur la VM cible |
| Fenêtre qui quitte sa sortie | replacement sur `EVENT_OBJECT_LOCATIONCHANGE` | le hook est posé de toute façon pour la détection |

---

## 7. Stratégie de test

**Testable sans Windows, et testé** :

- **Filtrage Alt-Tab** — sur des descriptions de fenêtres factices : top-level et
  `WS_VISIBLE`, propriétaire nul, `WS_EX_TOOLWINDOW` sauf `WS_EX_APPWINDOW`, non
  masquée par DWM (`DWMWA_CLOAKED`). Logique pure, aucune API appelée.
- **Table du superviseur** — la machine à états complète : fenêtre qui apparaît,
  qui disparaît, enfant qui meurt, vivier plein, viewport reçu avant ou après le
  lancement. Pilote et lanceur factices ; le trait `PiloteAffichageVirtuel`
  existe déjà et sert exactement à ça.
- **Vivier de sorties** — allocation, libération, refus, purge des orphelines.
- **Signaling** — extension de `signaling/src/server.test.ts` : types relayés, et
  mémorisation de l'offre qui supprime la course du §5.1.
- **Page-shell** — table des fenêtres ouvertes, réouverture d'une fenêtre dont la
  page a été fermée.

**Démonstration bout en bout, journal versé** : deux applications, deux fenêtres
navigateur, image juste dans chacune, clavier et souris arrivant dans la bonne,
son dans la porteuse.

**Point à vérifier dès la première tâche du plan — l'injection d'entrée.**
`SendInput` en absolu normalise sur l'écran **primaire** sauf à passer
`MOUSEEVENTF_VIRTUALDESK`. Une fenêtre posée sur un moniteur virtuel est hors de
cet espace. Si le drapeau ne suffit pas, l'input relatif — déjà implémenté pour
le jeu au chantier B — est le repli.

---

## 8. Hors périmètre de D1

- **Plein écran** (D4), y compris Keyboard Lock.
- **Mise en sommeil des fenêtres masquées** (D5), condition de viabilité au-delà
  de huit fenêtres.
- **Partage de la capacité réseau entre flux** (D3). Chaque enfant garde le
  contrôleur de congestion actuel et estime sa propre part sans se coordonner :
  à plusieurs fenêtres actives, **le lien sera sur-souscrit**. C'est une limite
  connue de D1, à écrire dans son rapport, pas un défaut à découvrir.
- **Audio par fenêtre** (D3), voie documentée en §3.2.
- **Redimensionnement d'une fenêtre déjà ouverte** (§3.3).
- **Redésignation de la porteuse audio** quand sa fenêtre se ferme (D2).
- **Le repli `PrintWindow`** pour les fenêtres auxquelles la voie principale ne
  s'appliquerait pas : mesuré à 8,8 i/s par fenêtre à N=8, il ne tient pas la
  cible et reste un repli pour deux à quatre fenêtres — hors de cette tranche.

---

## 9. Ce que cette conception ne sait pas

À lire avant d'écrire le plan, et à ne pas confondre avec des risques traités.

- **Le plafond d'encodeurs est-il par processus ou global ?** Mesuré à 8 dans un
  processus unique, jamais en multi-processus. L'architecture retenue rend la
  question observable pour la première fois — D1 la relèvera, il ne la résout
  pas d'avance.
- **Le vivier de 10 sorties est-il partagé avec Apollo ?** Non mesuré en
  concurrence. Le refus peut donc tomber avant la 11ᵉ.
- **L'injection d'entrée sur un moniteur virtuel n'est pas éprouvée** (§7).
- **Le placement d'une fenêtre sur une sortie virtuelle n'a jamais été fait par
  notre code.** Les mesures posaient les fenêtres par le banc, dans des
  conditions qu'il maîtrisait ; une application réelle peut se replacer, se
  redimensionner, refuser d'être maximisée.
- **Le loopback par processus n'est pas éprouvé** (§3.2) — seul le numéro de
  build a été relevé.
- **Les mires ne sont pas des applications.** Toutes les cadences connues
  viennent de mires D3D11 plein cadre, sans occlusion, sans interaction, sans
  redimensionnement. D1 est le premier montage à exercer de vraies applications.
- **Rien de la latence en multi-fenêtres** : aucune mesure, à aucun rang.
