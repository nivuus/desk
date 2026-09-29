# Legs ouverts — ce que le chantier package-nivuus laisse dû

> Extrait de [`CLAUDE.md`](../../CLAUDE.md) (lignes 1263-1574 de l'original), déplacé tel quel. Liens relatifs à ce dossier.

### Ce que le chantier `package-nivuus` laisse dû (30 août 2026)

🔴 **INSCRIT ICI ET NON DANS LE SEUL DOCUMENT DE RÉSULTATS, POUR LA MÊME
RAISON QUE LE CHANTIER `auth-pomerium` — et parce que ce chantier vient de
payer DEUX FOIS le patron « une preuve ne doit jamais vivre dans un rapport
gitignoré » : la revue de sa tâche 9 l'a fait corriger, et quatre lots plus
tard il était rouvert en plus grand.**

- 🔴 **UNE FENÊTRE OUVERTE PLUS DE 30 SECONDES AVANT LA CONNEXION DU
  NAVIGATEUR EST PERDUE, DÉFINITIVEMENT, ET RIEN NE LA REPROPOSE.** C'est un
  défaut du **PRODUIT**, pas du package, établi par capture réseau et
  messages du protocole décodés (lot 10D). `agent/src/superviseur/table/
  orphelines.rs::relancer_les_orphelines`, second garde-fou : toute entrée en
  `AttendLeViewport` depuis plus de `DELAI_ATTENTE_VIEWPORT_MAX`
  (`agent/src/superviseur/table.rs`, **30 s**) est retirée et refusée
  (« la page-shell n'a jamais répondu après la relance »). Le commentaire
  d'`orphelines.rs` le dit lui-même : **« une entrée abandonnée n'est JAMAIS
  reproposée, le hook ne réémettant rien pour une fenêtre déjà ouverte »** —
  la seule façon de la revoir est de fermer puis rouvrir la fenêtre Windows.
  Et **rien ne bufferise côté plateforme** : `plateforme/src/signaling/
  appariement.ts` ne conserve aucun état pour `fenetre-ouverte`/`refus`
  (seule l'offre SDP l'est), un message dont le pair n'est pas connecté est
  simplement **non relayé, sans mise en file**. 🔴 **C'est exactement le mode
  d'usage réel derrière Pomerium** — l'OAuth prend du temps — et c'est le
  symptôme que le propriétaire a rapporté (« aucune fenêtre disponible »).
  ✅ **CE LEGS ÉTAIT PÉRIMÉ QUAND ON L'A RELU, ET C'EST LE PATRON DU LEGS
  `403/404` PAYÉ UNE SECONDE FOIS** (établi le 31 août 2026, lot 34, **par
  les dates et non par une impression**) : il a été écrit par `5d01f1e` le
  30 août à **00:43:11**, et **corrigé par `6a7c98e` le même jour à
  02:49:53** — le lot 17, qui a donné au relais le message `pair-present` et
  fait redire ses fenêtres au superviseur, sans jamais revenir mettre à jour
  ce paragraphe. `git merge-base --is-ancestor 5d01f1e 6a7c98e` le confirme,
  et `f4e25f4` l'a mesuré (0 / 4 / 5 fenêtres). **La borne de 30 s est
  intacte et reste le bon mécanisme** : elle court depuis l'arrivée de la
  page-shell, l'instant que sa propre documentation prétend mesurer.
  🔴 **CE QUI RESTAIT RÉELLEMENT DÛ, ET QUE LE LOT 34 A FERMÉ, EST AILLEURS,
  EN DEUX POINTS** : ① tout le mécanisme du lot 17 est **inopérant** tant que
  la session de contrôle est morte, et elle ne se rouvrait jamais (voir
  « Ce qu'aucun chantier n'a jamais mesuré », legs fermé le 31 août) ; ② si
  la page-shell revient **avant** l'agent — le cas ORDINAIRE, le navigateur
  se rechargeant à la main en quelques secondes quand l'agent respecte un
  repli qui atteint trente secondes —, **personne ne prévenait l'agent** :
  `pair-present` ne partait qu'au pair DÉJÀ EN PLACE. Le relais sait
  désormais le dire aussi à l'agent qui **ARRIVE**
  (`pair-present.ts::prevenirLArrivant`). ⚠️ **UN TROISIÈME RÉSIDU RESTE, ET
  IL EST DEVENU LE SYMPTÔME DOMINANT** : une fenêtre `Vivante` n'est jamais
  redite à une shell rechargée — **décision du propriétaire**, dossier au § 8
  des résultats du lot 34.
- 🔴 **UNE FENÊTRE `Vivante` N'EST JAMAIS REDITE À UNE PAGE-SHELL QUI ARRIVE,
  ET C'EST DEVENU LE SYMPTÔME DOMINANT** (lot 34, 31 août 2026 ; nommé par le
  lot 17, jamais inscrit ici). `reannoncer_les_attentes` ne redit que les
  entrées en `AttendLeViewport` ; `recenser_les_fenetres_existantes` ne
  rattrape que les fenêtres ABANDONNÉES, `fenetre_apparue` étant idempotente
  par `HWND`. Une fenêtre dont l'enfant tourne reste donc **invisible à toute
  shell rechargée**, pendant que sa pop-up continue de diffuser. **Ce n'est
  pas un oubli, c'est la borne de la conception** : l'enfant consomme **une**
  offre et ne renégocie jamais (`agent/src/demarrage.rs`), donc la redire
  ferait ouvrir une page dont personne ne prendrait l'offre. 🔵 **DÉCISION DU
  PROPRIÉTAIRE, dossier à trois voies chiffrées au § 8 des résultats du lot
  34** — dont une (`window.open('', 'guac-<session>')`, qui rend la fenêtre
  existante **sans la naviguer**) est côté client seul, donc déployable
  **sans redémarrer l'agent**.
- ⚠️ **AUCUN BATTEMENT APPLICATIF SUR `/signal`, DONC UNE RECONNEXION PEUT
  ÊTRE REFUSÉE UN TEMPS INDÉTERMINÉ** (lot 34). Si un pair perd son socket
  sans que la plateforme voie le FIN, son rôle reste occupé dans
  l'appariement et la reconnexion est refusée en « un agent est déjà
  connecté » jusqu'au délai TCP de l'hôte. Relevé par `grep` sur
  `relais.ts` et `http/serveur.ts` : **aucun** `ping`/`pong`/`isAlive`. Le
  repli tient la cadence et le refus est **tracé** depuis le lot 34
  (`session de contrôle REFUSÉE par la plateforme`), mais la fenêtre
  d'indisponibilité n'est ni bornée ni mesurée.
- ⚠️ **LE PRÉFIXE DE SESSION N'EST PAS RELU À LA RECONNEXION** (lot 34) :
  seul le jeton l'est. Un réenrôlement qui délivrerait un préfixe différent
  ferait rouvrir l'ANCIENNE session de contrôle. Non observé (le préfixe
  dérive de la VM), non gardé, écrit ici plutôt que découvert.
- ⚠️ **LA PAGE-SHELL, ELLE, NE SE RECONNECTE PAS** — `shell.ts::
  canalDeControlePerdu` affiche « Rechargez la page pour vous reconnecter ».
  **Délibérément non touché par le lot 34** : ce n'est pas une panne muette
  (le message est exact et actionnable), et une reconnexion automatique
  rappellerait `ouvrir()` pour chaque fenêtre connue, donc **rechargerait les
  pop-ups vivantes** (`window.open(url, "guac-<session>")` vise une fenêtre
  NOMMÉE). Le remède juste suppose de trancher le legs précédent d'abord.
- 🔴 **UNE APPLICATION DU WINDOWS STORE NE SERAIT PAS ADOPTÉE** (lot 32I,
  30 août 2026). Depuis la règle d'appartenance, `desk` n'adopte que les
  fenêtres des processus qu'il a lancés, reconnus par un **job object sans
  aucune limite** (`crate::appartenance`). Or une application du Store paraît
  sous `ApplicationFrameHost.exe`, **qui ne descend pas de nous** — mesuré :
  `ApplicationFrameHost.exe ← svchost ← services ← wininit`.
  🔵 **Ce N'EST PAS une panne muette, et c'est ce qui rend ce legs
  acceptable** : `superviseur::hook::refusee_pour_appartenance` journalise
  chaque refus en nommant **la fenêtre, son processus et la raison**, et dit
  comment désarmer (`APPARTENANCE=0`).
  ⚠️ ~~**Aucune application du catalogue n'est dans ce cas aujourd'hui** : ses
  41 entrées sont des raccourcis Win32, et `Calculator` est servi par
  `win32calc.exe`, la version héritée, enfant direct de l'agent. **Le risque
  est repoussé, pas supprimé.**~~ 🔴 **FAUX DEPUIS LE 5 SEPTEMBRE 2026, ET LE
  LEGS EST PLUS LARGE QUE CE QU'IL DIT : `Microsoft Edge` — un raccourci Win32
  du catalogue, PAS une application du Store — N'EST JAMAIS ADOPTÉ.** Mesuré,
  deux lignes à 60 ms d'intervalle : `agent::apps::lancement: raccourci lancé
  chemin="…\Microsoft Edge.lnk"` puis `agent::superviseur::hook: fenêtre
  ÉCARTÉE : desk ne l'a pas lancée (règle d'appartenance)
  processus=msedge.exe`. **Aucune page de session ne s'ouvre**, 90 s d'attente.
  Le processus qui porte la fenêtre n'est pas celui que desk a créé — un
  navigateur se relance à travers un courtier. `Notepad`, lui, est adopté et
  sert une session en 4 s. Voir
  [les résultats partiels du lot 3](../superpowers/plans/2026-09-05-lot3-campagne-vm-resultats-partiels.md).
  ⚠️ **La contrepartie, assumée et non une régression** : une fenêtre **déjà
  ouverte avant `desk`** n'est plus reprise — `cmd.exe` et `Forza Horizon 6`
  quittent le hub. Décision du propriétaire, prise en connaissance de cause.
- 🔴 **`AGENT_VM`/`AGENT_SECRET` NE SONT POUSSÉS PAR RIEN DANS LA VM.**
  ⚠️ **REQUALIFIÉ le 30 août 2026 (lot 32C), PAS FERMÉ.** Sur la machine de
  production, le couple **atteint bien l'agent** — mesuré sur le processus
  vivant, trace d'enrôlement présente et avertissement d'absence à zéro : il y
  a été posé **à la main** dans `C:\nivuus\agent\run-agent.ps1`. **L'asset
  livré par `console`** (`console/guest/provision/assets/run-agent.ps1`) **ne
  les pose toujours pas** : une installation NEUVE retombe dans le legs. Ce
  qu'il faudrait y écrire : `$env:AGENT_VM` et `$env:AGENT_SECRET`, alimentés
  par ce que `desk activate` écrit déjà dans `desk.env`, **AVANT** la ligne
  `& 'C:\nivuus\agent\agent.exe'` — l'ordre est ce qui compte.
  ⚠️ **Et `main.rs` ne REFUSE pas** : `SourceIdentite::Aucune` émet un `warn!`
  et continue sans canal. L'agent démarre quand même ; seul le journal le dit.
  `agent/src/configuration.rs` les lit, `main.rs` refuse explicitement sans
  eux, et `run-agent.ps1` de `console` ne pose que `SIGNALING_URL`,
  `LOCAL_IP` et `RUST_LOG`. Le hook `activate` les écrit dans `desk.env`
  **côté hôte**. Sans ce couple, **aucune session ne peut s'établir**, quelle
  que soit la qualité de l'installation. Vérifié encore le 30 août 2026.
- ⚠️ **UN SERVICE `desk` TOURNE EN PRODUCTION SUR CETTE MACHINE**
  (`192.168.3.1:3445`, derrière `https://app.allanic.me`, mode `pomerium`,
  copie déployée sous `/opt/nivuus/desk`) — **ce n'est pas le dépôt qui le
  sert**. Tout redéploiement par `rsync -a` doit refaire un
  `chmod -R a+rX` : `DynamicUser=yes` fait tourner le service sous un UID
  éphémère, et des droits trop restrictifs lui rendent la page illisible
  (incident réel, page blanche d'une minute).
- ✅ ~~⚠️ **LA RACINE `/` SERT LA PAGE DE SESSION**, qui se rabat sur la
  session `demo` sans jeton : un utilisateur qui tape l'adresse du service
  tombe mécaniquement sur la seule page qui ne peut pas marcher. **Décision
  non prise** (servir le hub, ou rediriger) — elle appartient au
  propriétaire.~~ **TRANCHÉ, ET CE LEGS ÉTAIT DÉJÀ PÉRIMÉ QUAND ON L'A RELU —
  LE PATRON DU LEGS `403/404`, PAYÉ UNE TROISIÈME FOIS** (constaté le 31 août
  2026, chantier `navigation-hub-unique`, tâche 11). La décision a été prise
  par le **lot 14**, qui a posé `const PAGE = 'hub.html'` dans
  `plateforme/src/http/page/resolution.ts` : la racine sert le **hub**, pas la
  page de session, et ce paragraphe est resté « non tranché » pendant que le
  produit l'avait tranché. Le chantier du 31 août est allé plus loin — le hub
  est désormais la **SEULE** surface, et `shell.html` une redirection
  permanente vers `/`. 🔴 **La leçon est celle que ce fichier écrit déjà deux
  fois et n'a pas empêchée : un § « Legs ouverts » consolidé À LA MAIN
  vieillit comme n'importe quel relevé daté.** Voir
  [les résultats du chantier](../superpowers/plans/2026-08-31-navigation-hub-unique-resultats.md).
- 🔴 **`metadata.captureTime` EST INATTEIGNABLE TANT QUE L'AGENT NE NÉGOCIE
  PAS L'EXTENSION D'EN-TÊTE RTP `abs-capture-time`** (lot 3, item 7, 5 septembre
  2026). **Ce n'est PAS que l'agent se taise** : il émet bien des sender reports
  RTCP — **204** relevés sur le fil en 110 s (tous `192.168.3.2 > 192.168.3.1`),
  et Chrome les lit (`remote-outbound-rtp` vidéo, `reportsSent=63`,
  `remoteTimestamp` peuplé). Mais un sender report alimente `remoteTimestamp`,
  **jamais `captureTime`** : ce dernier, comme
  `RTCRtpContributingSource.captureTimestamp` et `senderCaptureTimeOffset`
  (relevés **absents**), vient de l'extension `abs-capture-time`.
  **Relevé : `grep -rn "abs-capture-time\|absolute-capture-time\|extmap"
  agent/src/ --include='*.rs'` rend `0`** — aucune extension d'en-tête n'est
  déclarée nulle part. **Où cela se poserait** : la négociation SDP passe par
  `agent/src/transport/boucle.rs::accept_offer` (`sdp_api().accept_offer`), et
  l'horodatage de capture est déjà calculé côté émission —
  `agent/src/transport/piste_video.rs::capture_instant`, tenu par le test
  `write_frame_annonce_l_instant_de_capture_au_pair_via_le_sender_report_rtcp`.
  🔴 **CONSÉQUENCE À DIRE À CHAQUE FOIS : toute mesure de latence reste un
  SUBSTITUT tant que cette extension n'est pas livrée.** Le substitut mesuré
  (`RTT/2 + totalProcessingDelay`) rend **9,18 et 9,04 ms** au nominal contre
  **42,27 et 39,72 ms** sous `netem adsl` — il **borne par le bas** et ne
  contient ni la capture, ni l'encodage, ni l'affichage après
  `presentationTime`. **Décision du propriétaire du dépôt** : ce lot mesure et
  ne répare pas ; une campagne qui modifie ce qu'elle mesure ne mesure plus rien.
- ⚠️ **LE RETRAIT RÉEL DES SCRIPTS DE LA VOIE MORTE EST UNE DETTE NOMMÉE**
  (5 septembre 2026). `scripts/winrm.js`, `scripts/build-agent.sh`,
  `scripts/sync-agent.sh`, `scripts/run-agent.sh`, `scripts/stop-agent.sh`,
  `scripts/check-session.sh` et `scripts/sonde-multifenetre.sh` **échouent
  désormais vite en nommant leur successeur** (`scripts/voie-morte.sh`, code de
  sortie **78**) — sur décision du propriétaire du dépôt, qui a écarté aussi
  bien leur suppression (elle perdrait la trace de ce qu'ils faisaient) que le
  statu quo (des scripts **morts qui ont l'air vivants**). **Ils ne sont PAS
  supprimés**, leur corps reste lisible sous le garde. 🔴 **CE QUI EMPÊCHE LE
  RETRAIT, ET QUI EST MESURÉ** : ils sont encore cités par **48** appelants
  exécutables pour `scripts/winrm.js`, **57** pour `/media/vm` et **2** pour
  `scripts/build-agent.sh` dans l'arbre suivi par git — dont **23 sont les
  instruments mêmes que le lot 3 doit rejouer** (`jouer-f1.sh` à `jouer-f5.sh`,
  `pilote-pp-p1/p2/p3.mjs`, `pilote-accent-a1.mjs`, `jouer-e3.sh`…).
  ⚠️ **Relancer la commande, ne pas recopier ces trois nombres** :
  `git ls-files | grep -E '\.(sh|mjs|js|py)$' | xargs grep -l -- <motif>`.
  Le retrait se joue le jour où plus rien ne les cite.
- ⚠️ **`coturn` EST POSÉ, JAMAIS ARMÉ** : `/etc/turnserver.conf` est écrit,
  `/etc/default/coturn` ne l'est pas, et **rien n'écoute sur le port 3478**
  alors que le service annonce `TURN_URL=turn:90.87.35.18:3478` à ses
  clients.
- 🔴 **UN REDÉMARRAGE DE L'AGENT ORPHELINE TOUTES LES FENÊTRES OUVERTES, ET
  RIEN NE LES RÉCUPÈRE.** Conséquence d'exploitation de la règle
  d'appartenance du lot 32I, mesurée deux fois le 31 août 2026 : après toute
  relance, chaque fenêtre préexistante est refusée (`fenêtre ÉCARTÉE : desk ne
  l'a pas lancée`), le hub est vide, et le propriétaire ne retrouve rien tant
  qu'il n'a pas **relancé** ses applications depuis le hub. Le job
  d'appartenance ne survit pas au processus qui le crée, et **rien ne persiste
  l'ensemble des PID adoptés**. ⚠️ **NON CORRIGÉ, à dessein** : persister les
  PID, ré-adopter au démarrage ou adopter par ascendance sont un **changement
  de conception**, qui appartient au propriétaire. Se cumule avec le legs
  « une fenêtre ouverte plus de 30 secondes avant la connexion du navigateur
  est perdue », ci-dessus.
- 🔴 **LA MESURE DU CURSEUR DU LOT 32T RESTE DUE, ROUGE COMME VERTE.** Le
  remède E1 est écrit, éprouvé sur l'hôte, **vu rouge** (432 px à l'assertion
  attendue) et **déployé** (`12040BEAAE905B3D…`, session 1 attestée) — mais
  **aucune session ne l'a encore exercé** : sa trace n'apparaît **0** fois dans
  `agent.log`, `InputInjector::new` n'étant atteint qu'à l'établissement d'une
  session WebRTC réelle. Le pilote a été refusé par le produit lui-même
  (« Bureau refusé : un client est déjà connecté »), le rôle `client` étant
  exclusif par session. **Il faut un créneau où le propriétaire est
  déconnecté** — ou son propre jugement, la correction étant en place.
- 🔴 **AUCUNE INSTALLATION N'A JAMAIS ÉTÉ JOUÉE PAR LE MOTEUR RÉEL.** La
  Critique de la revue finale — un `resolve` qui refusait toujours — a
  survécu à huit suites vertes et neuf revues **pour cette seule raison**.
  Tant que `run.py` n'a pas appelé ces hooks pour de vrai, la même classe de
  défaut reste possible.
- 🔴 **`SORTIE_DESIGNEE` N'ATTEINT PAS L'AGENT DE L'APPLIANCE** (lot 32,
  30 août 2026). La variable est posée par `scripts/run-agent.sh`, ce
  qu'exige la règle du dépôt — mais ce script vise la VM de développement
  **qui n'existe plus sous cette forme**. L'agent réel est lancé par la tâche
  planifiée `guacamole-agent`, qui exécute **`C:\nivuus\agent\run-agent.ps1`**,
  un fichier du package **`console`** ne posant que `SIGNALING_URL`,
  `LOCAL_IP`, `RUST_LOG`, `AGENT_VM`, `AGENT_SECRET` et `SUPERVISEUR`.
  🔴 **CE QUE CELA EMPÊCHE : le bras ROUGE du remède du lot 32 n'est PAS
  rejouable sur l'agent réel de l'appliance** — seulement sur celui que
  `scripts/run-agent.sh` lance, ou par une injection à la main dans le `.ps1`
  de `console` (ce que la recette du lot 32 a dû faire). ⚠️ **Corriger cela
  touche un AUTRE package** : hors périmètre du lot 32, la décision
  appartient au propriétaire. **Consigné ici et non dans le seul relevé daté,
  parce que ce dépôt a constaté deux fois qu'un legs qui ne vit que là est un
  legs perdu.**

- 🔴 **CRÉER LA SORTIE GÉNÉREUSEMENT — LA QUESTION A CHANGÉ DE NATURE, ET
  C'EST CE QUI LA REND DÉCIDABLE** (lot 33). ❌ **La première rédaction de ce
  legs disait « un viewport plus large que la sortie garde ses bandes noires
  (mesuré : `5118x1438` demandé, `1428x538` servi) ». C'EST FAUX depuis le fit
  à aspect préservé** : ce même viewport est désormais servi **à son rapport
  exact**, et il n'y a plus de bandes, quelle que soit la borne. Ce qu'une
  sortie plus grande achète n'est donc plus l'absence de bandes mais **la
  NETTETÉ** — servir `1723x1303` nativement au lieu de `1364x1032` remonté par
  le navigateur. ⚠️ **Ce qui fragilise l'option reste entier** : D8 a établi
  qu'une sortie ne naît PAS à la taille demandée, et la relation entre taille
  demandée, `DesktopCoordinates` et `DXGI_OUTDUPL_DESC` **n'est toujours pas
  comprise** — sur cette machine et le même jour, une sortie créée à 1428 a
  été relevée à 1428 puis à 1860 selon la session. 🔴 **Et le plafond
  d'encodeurs à N fenêtres n'a JAMAIS été mesuré au-delà de 720p** (protocole
  écrit et non joué au lot 31) : NVENC borne en macroblocs par seconde, et
  huit fenêtres à `TAILLE_MAX_SORTIE` sont **2,25×** les macroblocs de huit
  fenêtres à 720p. **Décision du propriétaire**, et le dossier complet est au
  § 11.3 du document de résultats.
- ✅ ~~**UNE MARGE DE 1 À 2 PX SUBSISTE SUR LES QUATRE CÔTÉS**~~ **RETIRÉE LE
  31 AOÛT 2026, SUR DÉCISION DU PROPRIÉTAIRE** (lot 33). C'était le **liseré
  d'accent** — `#remote { border: var(--trait) … }`, 1 px CSS peint **hors de
  la boîte de contenu** (`box-sizing: border-box`), sur les quatre côtés, soit
  1 à 2 px écran selon `devicePixelRatio`. Établi par l'arithmétique : le
  recadrage et le cadre visible coïncidaient à **0x0 près, origine comprise**,
  et le résidu d'`object-fit: contain` valait **0,40 px sur DEUX côtés
  seulement** — `contain` centre l'image et ne laisse **jamais** quatre bords.
  **Seule la déclaration `border` a été retirée** ; voir la régression qu'elle
  rouvre au § ⑥ du tableau par sous-projet.
- ✅ ~~**LA MARGE SUR LES QUATRE CÔTÉS N'EST PAS EXPLIQUÉE**~~ **LES ~7 PX SUR
  TROIS CÔTÉS : EXPLIQUÉS ET CORRIGÉS le 31 août 2026** — c'était **le cadre INVISIBLE de DWM**, mesuré en
  session 1 sur la session vivante du propriétaire : `GetWindowRect` rend
  `1732x1032+1280+0` là où `DWMWA_EXTENDED_FRAME_BOUNDS` rend
  `1718x1025+1287+0`, soit **7 px à gauche, à droite et en bas, 0 en haut**.
  Depuis Windows 10 les bordures de redimensionnement sont **transparentes** et
  `GetWindowRect` les inclut : le dépôt posait, relisait et comparait de bout en
  bout dans un espace **qui n'est pas celui qu'on voit**, et le recadrage
  suivant la taille POSÉE, l'image contenait du bureau sur trois côtés.
  🔵 **C'est le discriminant « la marge varie-t-elle avec la forme ? », écrit
  AVANT la mesure, qui l'a départagé d'une erreur d'aspect résiduelle** — le
  propriétaire a répondu « la marge ne varie pas », et l'aspect était de toute
  façon tombé à 0,176 % au pire (~3 px sur 1700). Le quatrième côté est le
  liseré d'accent d'un pixel (`#remote { border: var(--trait) … }`), **une
  fonctionnalité voulue qui n'est pas retirée**. Remède :
  `placement::{Lisere, rect_a_poser, taille_a_poser}` — et `rectangle_de` rend
  désormais le cadre VISIBLE, **les deux moitiés allant ensemble sous peine
  d'un replacement à 1 Hz**.
- 🔴 **`client/src/main.ts` A FRANCHI SON PLAFOND POUR LA DEUXIÈME FOIS** par
  une addition d'une vingtaine de lignes (lot 33) ; il était à **500
  EXACTEMENT** au commit précédent. Troisième extraction (`viewport-dom.ts`,
  après `resize-dom.ts` et `presse-papier-dom.ts`). **Le prochain qui y ajoute
  quoi que ce soit doit extraire d'abord** — de même que
  `superviseur/placement.rs`, porté à 500 pile par une simple correction de
  commentaire et extrait dans la foulée.
- ⚠️ **UN REDIMENSIONNEMENT DEMANDÉ PENDANT LE SOMMEIL D'UNE FENÊTRE EST À
  NOUVEAU PERDU** (lot 33) — il ne l'était plus tant que `resize` était un
  `no-op`. Le rejeu du client (`RejeuResize`) devrait le rattraper au `Resize`
  suivant : **non mesuré**, et dit comme tel dans `capteur/fenetre/commandes.rs`.

- ⚠️ **UN PIXEL DE CONTENU EST ROGNÉ SUR UNE FENÊTRE SANS BORDURE PEINTE**
  (lot 33, contrepartie assumée). Le recadrage se rétracte de
  `GetSystemMetrics(SM_CXBORDER/SM_CYBORDER)` sur chaque bord pour exclure la
  bordure que Windows peint — mais cette métrique **ne distingue pas** une
  fenêtre qui n'en a pas (plein écran sans cadre, `WS_POPUP` nue) : on y
  mangera 1 px de contenu réel. **Arbitrage pris par le propriétaire** : un
  pixel perdu sur le cas RARE vaut mieux qu'une ligne sombre permanente sur le
  cas COURANT. **Où regarder** : rejouer l'instrument de
  `docs/superpowers/plans/journaux-lot33/pixels/` sur cette application et
  comparer `rangee 0` à `rangee 1` ; le désarmement tient en une ligne
  (`placement.rs::enveloppe()` ignorant `bordure`), et un test le fige.
- 🔴 **UN CORRECTIF DU LOT 33 EST BÂTI, ATTESTÉ, ET NON DÉPLOYÉ**
  (`a4b8ba1604b9279a`) : la rétraction d'un pixel ci-dessus. Il est **dans la
  branche** et voyagera avec le prochain envoi côté agent — déployer seul
  aurait coûté un quatrième redémarrage, donc un quatrième hub vidé, au
  propriétaire. ⚠️ **Il n'ajoute aucune chaîne** et n'est donc pas attestable
  par `strings` : son témoin est la relecture des pixels (`rangee 0` doit
  devenir claire).
