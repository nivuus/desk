# Sous-bloc D8 — la recette du plein écran (5 août 2026)

Cahier des charges : `.superpowers/sdd/2026-08-04-multifenetres-plein-ecran/task-11-brief.md`.
Instrument, écrit par la tâche 10, non modifié par cette tâche :
`docs/superpowers/plans/journaux-multifenetres-d8/instrument/pilote-recette-d8.mjs`.
Journal complet de l'unique exécution retenue :
`docs/superpowers/plans/journaux-multifenetres-d8/critere-recette.log` (pilote,
mis à plat, sans séquences ANSI) et son pendant côté VM,
`docs/superpowers/plans/journaux-multifenetres-d8/agent-recette.log` (copié
après la fermeture du navigateur, comme l'exige l'étape 4 du brief). Relevé
structuré : `docs/superpowers/plans/journaux-multifenetres-d8/recette-recette.json`.

Binaire mesuré : commit `7032b01`, `agent.exe` 9 248 256 octets, horodaté
4 août 17:59 — 4 minutes après le commit (17:55:37), `git status --porcelain`
vide sur `agent/ scripts/ proto/ client/ signaling/` au moment de la mesure.
**Aucune recompilation n'a eu lieu pour cette tâche** : le binaire déjà présent
sur la VM était à jour.

## Le verdict, en une phrase

**Une exécution complète a été menée à son terme, après la levée d'un blocage
environnemental qui empêchait toute fenêtre de s'attacher.** Le témoin de
non-régression est TENU. Le critère ④ (sommeil de la voisine) est TENU sur son
mécanisme central. Les critères ①, ② et ⑤ ne sont **pas confirmés**, chacun pour
une raison distincte et bien identifiée — aucune des trois n'est un « refus »
franc, mais aucune n'établit non plus ce que le critère demandait. **Une seule
exécution complète a été jouée : tout ce qui suit porte ce nombre, jamais un
taux.**

| # | Critère | Verdict | Exécutions |
| --- | --- | --- | --- |
| témoin | Non-régression (aucun plein écran) | **TENU** | 1 |
| ① | Plein écran Windows détecté et annoncé, et à la bonne fenêtre seule | **NON CONFIRMÉ** — aucune détection sur la cible, une fuite vers une voisine | 1 |
| ② | Le flux suit le viewport plein écran, la sortie garde son nom | **NON EXERCÉ** — zéro tentative de changement de mode enregistrée | 1 |
| ③ | Échap et Keyboard Lock | **NON MESURÉ** — décision actée en tête de tâche 10 (voir plus bas) | 0 |
| ④ | Les voisines s'endorment par le chemin existant | **TENU** — endormissement confirmé en 2,2 s | 1 |
| ⑤ | L'audio d'une endormie survit | **MESURÉ, avec réserve** — le seuil brut passe, le contenu fréquentiel est douteux | 1 |

## Étape 0 : un blocage environnemental a d'abord empêché toute mesure

**Avant toute chose, la préparation de la VM (§1 du brief) a révélé un défaut
sévère, non prévu par les « trois inconnues » du brief, mais qui en confirme un
fait déjà établi à un niveau plus grave que documenté.**

Après purge des sorties orphelines (0 orpheline trouvée, topologie propre —
`\\.\DISPLAY1` seule, physique) et lancement du superviseur, **aucune des trois
fenêtres n'a pu s'attacher** : le superviseur a créé et détruit en boucle des
sorties virtuelles à 1280×720 pour une dizaine de noms de session différents en
moins de 6 secondes, chacune refusée par
`sortie créée mais introuvable dans la topologie DXGI` — la sortie apparaissait
bien dans la topologie, mais à **2560×1440**, jamais à 1280×720 demandé.

C'est exactement le défaut F1 déjà documenté en tête de
`agent/src/diagnostics/multifenetre/mode_sortie.rs` (« persistance probable au
registre d'un `CDS_UPDATEREGISTRY` d'une exécution antérieure ») — mais ici il
ne biaisait pas une sonde, **il bloquait le produit entier** : les mesures P1/P1bis
de la tâche 3/3bis (`mode-sortie-1728x1080.log`, `p1bis-mode-sortie.log`) avaient
laissé le registre SudoVDA à 2560×1440 pour le GUID de sortie virtuelle unique
(`9C4A1F6E-2B73-4D51-9E08-677541430001`), et **le code de production**
(`superviseur/boucle.rs`) exige une correspondance exacte avec la taille
demandée — sans le repli dynamique que P1 s'était donné pour se prémunir de ce
même défaut.

**Remède appliqué : la sonde P1 elle-même**
(`MULTIFENETRE_MODE_SORTIE=1280x720`, invocation séparée — elle a son propre
aiguillage et sort après un seul mode), qui crée une sortie, lit la taille
héritée du registre (2560×1440, confirmée), et la fait passer à 1280×720 par
`CDS_UPDATEREGISTRY` (verdict "P1 RECU", `code_brut_dernier_essai=0`,
mouvement observé et confirmé par relecture DXGI). Ce n'est **pas** un
changement de code — c'est l'exécution de l'outillage diagnostic déjà présent,
au même titre que la purge des sorties orphelines déjà prescrite par le brief.
Un contrôle de topologie neuf (`MULTIFENETRE_DXGI=1`) a confirmé l'état propre
après coup.

**Piège opérationnel neuf, à consigner** : la première tentative de combiner
`MULTIFENETRE_VDD_PURGE=1` et `MULTIFENETRE_MODE_SORTIE=1280x720` dans le même
lancement n'a rien fait pour la seconde variable — l'aiguillage de
`agent/src/diagnostics/multifenetre.rs` retourne après la **première** sonde
reconnue (chaque branche fait `return Ok(true)`), il ne les enchaîne pas.
**Chaque sonde doit être son propre lancement.** Second piège, plus coûteux :
un premier essai de correction a été silencieusement sans effet parce que
**deux processus `agent` de la tentative de recette précédente (échouée) étaient
encore vivants** — `Get-Process agent` en montrait déjà zéro juste avant le
lancement de la recette elle-même, mais celle-ci avait relancé un superviseur
qui n'a jamais été arrêté après l'échec (`ERREUR FATALE`, aucun nettoyage côté
VM dans le pilote). Résultat observé : le fichier `agent.log` restait identique
octet pour octet après un second `run-agent.sh`, parce que le nouveau
`StreamWriter` ne pouvait pas ouvrir le fichier déjà tenu par l'ancien
processus. **`Get-Process agent` doit être revérifié après CHAQUE tentative
échouée, pas seulement avant la première.**

## Le témoin de non-régression : TENU

Palier de 60 s, 3 fenêtres réellement ouvertes menant à **5 sessions
détectées** (voir plus bas), une focalisée (w-1) à 1280×720, quatre non
focalisées réduites à 1024×576 par l'adaptation réseau (« Image réduite par le
réseau »). Comportement conforme au régime déjà documenté pour D6/D7 : la
focalisée tient son plein débit (84,71 i/s, 0,04 % jetées, 3,75 Mb/s), les
non-focalisées sont cadencées de façon comparable (81,9 à 84,5 i/s, 0,04 à
4,21 % jetées) sans qu'aucune ne meure ni ne décroche. VM vivante après la
phase (`virsh="en cours d'exécution"`, `acces_partage=OUI`). **1 exécution.**

## Une inconnue neuve, non prévue par le brief : 5 sessions pour 3 fenêtres ouvertes

**Fait à consigner avant les critères eux-mêmes, parce qu'il conditionne la
lecture de ①, ② et ⑤.** Le pilote a ouvert exactement 3 fenêtres
(`ouvrirFenetre(1)`, `(2)`, `(3)`, marqueurs `chrome-d8-recette-{1,2,3}`,
fréquences 410/520/630 Hz). Le superviseur a détecté et attaché **5 sessions
distinctes** : `w-1, w-2, w-4, w-6, w-8` (`enfant_lancé=5`,
`attachee_capteur=5`). Ce nombre est un **fait relevé directement dans le
journal** (`enfant lancé session=… pid=… sortie=\\.\DISPLAYn` × 5), pas une
extrapolation.

C'est le même phénomène de « fenêtre fantôme » déjà documenté (Paint ouvre deux
fenêtres éligibles en D2, un Bloc-notes fait avancer le compteur de deux en D4)
— **une seule fenêtre navigateur `--app` peut donc produire plus d'une entrée
détectable côté superviseur.** Ce qui n'était PAS établi jusqu'ici : la
correspondance entre l'ORDRE de lancement du script (`n=1,2,3`, donc les
fréquences 410/520/630 assignées par index de boucle) et l'ordre réel
d'attachement (`w-1, w-2, w-4, w-6, w-8`) **n'a aucune raison d'être fidèle**
dès qu'un doublon fantôme s'intercale. Le pilote assigne `cible = noms[0]`
(donc `w-1`) et `voisine = noms[1]` (donc `w-2`) en supposant cette fidélité —
supposition **non vérifiée par ce pilote**, et les deux anomalies ci-dessous
(①, ⑤) sont cohérentes avec sa mise en défaut, **sans que cela soit prouvé**
par la seule exécution menée.

## Critère ① : NON CONFIRMÉ

**La bascule de style Windows a réellement eu lieu, confirmée par relecture
directe.** `togglerStyleFenetre` relit le style par `GetWindowLongPtrW` avant
et après (jamais le seul code de retour de l'appel, doctrine déjà établie) :
`avant=382664704` (`WS_CAPTION`/`WS_THICKFRAME` présents),
`après=369819648` (les deux bits retirés) — **la fenêtre Windows ciblée
(marqueur `chrome-d8-recette-1`) est passée sans bordure pour de vrai.**

**Mais l'agent n'a jamais annoncé cette bascule pour la session cible.**
`détection_agent` = `null` : aucune ligne `plein ecran de la fenetre Windows
session=w-1` en 30 s. Le message de contrôle attendu côté navigateur n'est donc
jamais arrivé non plus (`messages_navigateur_cible: null`).

**Et une VOISINE a reçu le message que la cible attendait.** `w:w-4` a bien
reçu `{"type":"fullscreen","active":true}` sur son canal de contrôle pendant
la fenêtre de mesure (`fuite_vers_voisine: ["w:w-4"]`). Relevé direct dans le
journal agent, les DEUX seules lignes `plein ecran de la fenetre Windows` de
**tout le run** portent `session=w-4` (une `actif=true` à 21:03:41.971Z, une
`actif=false` à 21:05:24.604Z) — jamais `session=w-1`.

**Lecture, sous réserve de la section précédente** : l'explication la plus
cohérente avec les faits est que la session que le superviseur nomme `w-4` est
en réalité celle qui porte le HWND de `chrome-d8-recette-1` — pas `w-1`, comme
le pilote le suppose par construction (`cible = noms[0]`). Sous cette lecture,
① n'est pas réfuté sur le fond (le mécanisme détecte bien un vrai changement de
style et l'annonce bien, exclusivement, à une seule session) — c'est
l'**attribution** cible/session du pilote qui serait fausse. **Cette lecture
n'est PAS prouvée** : elle repose sur une seule coïncidence temporelle
(l'annonce tombe pendant la fenêtre où la bascule a eu lieu) et sur le fait,
distinct, que le sous-bloc précédent établit que la correspondance
ordre-de-lancement / ordre-d'attachement n'est pas fiable. **Verdict tel que le
critère l'exige (« et elle seule », sur la session PRÉSUMÉE cible) : NON
CONFIRMÉ, mesurable uniquement en partie
(`verdict_partie_mesurable: false`).** 1 exécution.

## Critère ② : NON EXERCÉ

**Zéro tentative de changement de mode de sortie enregistrée sur toute la
fenêtre de mesure** (`mode_sortie_demande=0`,
`mode_sortie_complet: {demandees: [], reussies: [], refusees: [], illisibles: [], impossibles: []}`).
Les deux forçages de viewport CDP (`Emulation.setDeviceMetricsOverride`, 1920×1080
puis 3840×2160, sur la page présumée cible) n'ont produit **aucun** message
`Resize` visible côté agent : sur tout le run, seules **deux** lignes
`contrôle reçu Resize` existent, toutes deux à la connexion initiale
(21:01:58 et 21:02:09, 1280×720 — la taille déjà en place), **avant** que la
phase critère ①+② ne commence (21:03:31). `stats_apres_redimensionnement.l/h`
et `stats_apres_surdimensionne.l/h` restent tous deux à `1280×720` : le flux
vidéo n'a jamais bougé.

**Ni l'explication « `PLEIN_ECRAN=0` désarme le chemin » ni l'explication
« le mode `SortieEntiere` n'était pas actif » ne sont établies** : aucune trace
`redimensionnement ignoré : PLEIN_ECRAN=0` n'apparaît non plus dans tout le
journal — si c'était la cause, elle se serait journalisée. **Le fait le plus
probable, non vérifié au-delà de cette inférence, est que le message `Resize`
lui-même n'a jamais été ÉMIS côté client** : le pilote lance son Chrome hôte
avec `--ozone-override-screen-size=1600,1000`, plus petit que la cible
1920×1080 demandée par `Emulation.setDeviceMetricsOverride` — si l'écran
émulé borne l'override, `video.clientWidth`/`clientHeight` ne changeraient
jamais, et le `ResizeObserver` du client n'aurait rien à observer. **Cette
explication n'a pas été vérifiée indépendamment** (aucune instrumentation
côté page pour confirmer que `window.innerWidth` a ou non changé après
l'appel CDP) — elle est proposée comme piste, pas comme fait établi.

**Conséquence directe pour le brief : les trois inconnues qu'il posait comme
risque n°1 ne sont TRANCHÉES PAR AUCUNE DONNÉE de cette exécution** — voir la
section dédiée ci-dessous. Ce n'est pas un refus du pilote SudoVDA : c'est
l'absence totale de sollicitation.

## Critère ③ : NON MESURÉ, décision actée en amont

Comme convenu en tête de la tâche 10 (voir le commentaire de tête du pilote) :
la sonde P2 a établi que Chrome `--headless=new` n'entre pas réellement en
plein écran (`document.fullscreenElement` reste `null` 800 ms après un
`requestFullscreen()` par ailleurs invoqué) et n'expose pas `navigator.keyboard`.
**Décision actée : ③ n'est pas instrumenté, sans installation de `Xvfb`.**
Confirmé pour cette recette : `Xvfb` n'est **pas** installé sur cet hôte
(`which Xvfb` : introuvable). 0 exécution.

## Critère ④ : TENU

Masquage de la voisine (`w:w-2`, présumée hz=520) par `document.hidden=true` :
**sommeil confirmé en 2,2 s** (`FAIT ATTEINT … après 2.2 s`). La part de
budget appliquée à `w-2` tombe à **256 000 bps** (`PART_DORMANTE_BPS`) avec
`endormie=true`, pendant que les quatre autres sessions restent à
2 348 800 ou 4 697 600 bps, `endormie=false` — exactement le contrat documenté
depuis D5/D6, exercé ici sur une troisième fenêtre dans un binaire portant
le plein écran. **Le chemin existant fonctionne encore.**

**Réserve** : le réveil (masquage levé) n'a **pas** été confirmé dans la
fenêtre d'attente de 30 s du pilote (`reveil_confirme: null`,
`!! FAIT NON ATTEINT … après 30 s`). Ce n'est pas une réfutation du critère tel
que le brief le formule (« les voisines s'endorment », côté endormissement
seul) — mais c'est une donnée manquante que le pilote lui-même cherchait à
établir en plus. **1 exécution, le sommeil est confirmé, le réveil ne l'est
pas dans le délai imparti.**

## Critère ⑤ : MESURÉ, avec réserve sérieuse

Le seuil brut programmé dans le pilote passe : `audio_survit: true`
(le niveau à la fréquence assignée à `w-2`, 520 Hz, dépasse le plancher de
bruit de 43 dB, seuil fixé à 20 dB). **Mais le contenu fréquentiel dominant
reçu sur cette session est 409 Hz** (`hz: 409, db: -36`) — la tonalité assignée
à `w-1` (410 Hz), pas celle assignée à `w-2` (520 Hz, mesurée nettement plus
bas, `-115 dB`, tout de même 43 dB au-dessus du plancher).

**Deux lectures possibles, non départagées par cette seule exécution :**

1. **Confirmation du fait de conception déjà documenté par D7** : une fenêtre
   qui partage le groupe de PID d'une autre application entend le mélange
   entier de ce groupe (`PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE`
   capture l'arbre de processus, pas la fenêtre). Si `w-2` est en réalité une
   fenêtre fantôme du MÊME processus Chrome que `w-1` (cohérent avec la section
   « 5 sessions pour 3 fenêtres » ci-dessus), elle entendrait légitimement le
   ton de `w-1` — pas un défaut, le comportement déjà connu et accepté.
2. **Un défaut d'isolation propre à cette mesure**, sans rapport avec le fait
   ci-dessus.

**Aucune des deux n'est établie ici.** Ce que l'exécution établit sans
ambiguïté : de l'audio arrive bel et bien sur la session masquée (le critère
tel qu'énoncé — « l'audio d'une endormie survit » — passe au sens strict, de
l'énergie franchit le plancher de bruit pendant le sommeil), mais **la preuve
que c'est SPÉCIFIQUEMENT le ton de la fenêtre censée être `w-2` qui survit
n'est pas apportée** — le signal dominant reçu est celui d'une autre fenêtre.
**1 exécution.**

## Les trois inconnues du brief : AUCUNE N'EST TRANCHÉE

Le brief posait trois inconnues comme risque n°1 de ce sous-bloc. Cette
recette **ne les tranche pas**, faute d'avoir sollicité le mécanisme qu'elles
interrogent — voir critère ② ci-dessus.

1. **Le pilote SudoVDA accepte-t-il un changement de mode sur une sortie dont
   la duplication est ouverte ?** NON TRANCHÉE. Zéro tentative de changement de
   mode a eu lieu pendant que des duplications étaient ouvertes (0 tentative,
   point final) — la question posée par le brief reste **exactement aussi
   ouverte qu'avant cette recette**.
2. **Combien de pertes d'accès `0x887a0026` un changement de mode inflige-t-il
   aux voisines ?** SANS OBJET. `pertes_acces_voisines: []` reflète l'absence
   de toute tentative, **pas** l'absence de pertes lors d'un changement réel.
   Les 6 pertes de mutex relevées sur tout le run (`mutex_abandonne=6`,
   marqueurs finaux) sont toutes les réouvertures de routine à l'ouverture des
   captures (documentées depuis D2), sans rapport avec un changement de mode.
3. **La sortie garde-t-elle son nom `\\.\DISPLAYn` à travers le changement ?**
   SANS OBJET pour la même raison. `garde_fou_7_nom_sortie` relève
   `avant: "\\.\DISPLAY6"`, `apres: null`, `conserve: false` — le `null` vient
   de l'absence totale de ligne « demandées » à comparer, **pas** d'un
   changement de nom observé. Ne pas lire `conserve: false` comme un refus.

**Ces trois inconnues restent donc le premier travail d'une prochaine
recette**, avec un correctif de méthode nommé : vérifier, par une trace côté
page (`window.innerWidth`/`innerHeight` avant/après l'appel CDP), que le
viewport forcé prend réellement effet dans l'environnement du pilote avant de
compter sur le `ResizeObserver` du client pour le relayer.

## Ce que D8 n'établit pas

- **Aucun taux nulle part** : une seule exécution complète, pour les six
  lignes du tableau de synthèse comme pour chaque phase.
- **③ n'est pas mesuré**, et sa raison est mesurée elle-même (P2, sonde
  d'instrument) : Chrome `--headless=new` n'entre pas réellement en plein
  écran et n'expose pas `navigator.keyboard`.
- **Le cas HiDPI reste structurellement invisible à ce montage** : le pilote
  tourne avec `deviceScaleFactor: 1` partout (garde-fou de mesure hérité), et
  le défaut déjà documenté (le garde-fou anti-changement-de-mode-parasite ne
  tient qu'à `devicePixelRatio == 1`) n'a donc pu ni se manifester ni être
  réfuté ici.
- **Rien de la latence de bout en bout**, qu'aucun sous-bloc du chantier D n'a
  mesurée à ce jour.
- **Les trois couches inconnues du chantier D restent inconnues** : le plafond
  de 8 encodeurs, celui de 4 processus, et le mécanisme de l'abandon du mutex
  DXGI. Cette recette n'en a rencontré qu'une conséquence déjà documentée
  (6 pertes `0x887a0026` de routine, toutes encaissées).
- **L'attribution session ↔ fenêtre physique n'est vérifiée par aucune preuve
  indépendante** dans ce pilote (pas de titre de fenêtre journalisé côté
  agent, pas de HWND journalisé au moment de l'attachement) — c'est la lacune
  d'instrumentation qui rend ① et ⑤ non conclusifs plutôt que réfutés.
- **Un seul rang (N=3 fenêtres)** a été joué ; rien au-delà, rien sur le
  recouvrement, le déplacement ou le redimensionnement manuel d'une fenêtre.
- **`Xvfb` n'a pas été installé** pour cette recette — confirmé
  explicitement (§ critère ③) : aucune mesure de ce document ne s'appuie sur
  lui, et rien ici ne se compare à une éventuelle future campagne qui
  l'installerait.

## Réserves héritées, reprises sans les remesurer

- **La mesure F1 (P0) porte une seule exécution et une bascule de porteur en
  cours de mesure** — non rejouée ici, reprise telle quelle depuis la tâche 2.
- **Le repli `NORESET`→`RESET` de la sonde P1 n'a jamais fait bouger une
  sortie** — `CDS_UPDATEREGISTRY` seul a suffi aux deux occasions où P1 a
  tourné (tâche 3 et l'exécution de préparation de cette tâche) ; les deux
  combinaisons de repli restent non exercées.
- **Le défaut HiDPI reste ouvert côté client**, non remesuré (voir ci-dessus).

## Pièges neufs à connaître avant de retoucher ce terrain

- **Combiner deux variables `MULTIFENETRE_*` dans le même lancement n'enchaîne
  PAS deux sondes** : l'aiguillage retourne après la première reconnue. Une
  variable de préparation (`MULTIFENETRE_VDD_PURGE`) et une variable de mesure
  (`MULTIFENETRE_MODE_SORTIE`) doivent être deux lancements séparés, même si
  leur ordre relatif de priorité est documenté dans le code.
- **Un pilote qui se termine en erreur (`ERREUR FATALE`) ne nettoie PAS le
  superviseur côté VM.** Le bloc `finally` du pilote ne tue que le Chrome
  hôte ; les processus `agent` (superviseur + capteur) lancés par
  `run-agent.sh` survivent, tiennent le fichier `agent.log` en écriture
  exclusive, et font échouer silencieusement tout relancement ultérieur tant
  qu'ils n'ont pas été tués nommément (`Get-Process agent` puis
  `Stop-Process -Id … -Force`).
- **Une pollution de registre laissée par une sonde de mesure peut bloquer le
  produit, pas seulement fausser une sonde future.** Le défaut F1 documenté
  pour P1 (« persistance probable au registre ») s'est manifesté ici comme un
  blocage total de l'attachement de fenêtre — aucune des trois fenêtres ne
  pouvait s'ouvrir tant que le registre n'a pas été remis à 1280×720 par la
  sonde P1 elle-même, en préparation.
- **Le nombre de sessions détectées par le superviseur n'est pas fiable comme
  proxy du nombre de fenêtres réellement ouvertes par un pilote**, et ce
  n'était pas déjà écrit noir sur blanc pour ce cas précis (3 fenêtres, 5
  sessions) — seuls les précédents Paint/Bloc-notes l'étaient. Un pilote qui
  assigne une identité (fréquence, marqueur) par INDEX DE BOUCLE plutôt que
  par un identifiant relu côté agent (titre de fenêtre, HWND) s'expose à une
  attribution fausse dès qu'une fenêtre fantôme s'intercale dans l'ordre
  d'attachement.
- **`--ozone-override-screen-size` du Chrome hôte peut plafonner
  silencieusement un `Emulation.setDeviceMetricsOverride` demandé plus
  grand** — hypothèse posée par cette recette pour expliquer le silence total
  de ②, non vérifiée indépendamment.

## Fichiers

- Pilote (non modifié) :
  `docs/superpowers/plans/journaux-multifenetres-d8/instrument/pilote-recette-d8.mjs`
- Journal du pilote (hôte, mis à plat) :
  `docs/superpowers/plans/journaux-multifenetres-d8/critere-recette.log`
- Journal de l'agent (VM, copié après fermeture du navigateur) :
  `docs/superpowers/plans/journaux-multifenetres-d8/agent-recette.log`
- Relevé structuré : `docs/superpowers/plans/journaux-multifenetres-d8/recette-recette.json`
