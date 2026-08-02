# Sous-bloc D2 — rendre l'arrangement multi-fenêtres dynamique

> Conception. Chantier D (multi-fenêtres) du plan de support des jeux,
> `2026-07-28-support-jeux-design.md` §5 D.
>
> Antécédent direct : le sous-bloc D1, exécuté le 1ᵉʳ août 2026, **n'est pas
> reçu** — `plans/2026-08-01-multifenetres-tranche-verticale-resultats.md`.
> D2 lève ce qui l'en empêche, et rien d'autre.

> ⚠️ **Verdict d'exécution, 1ᵉʳ août 2026 — il est DOUBLE.**
> `plans/2026-08-01-multifenetres-arrangement-dynamique-resultats.md`.
> **① Le défaut central du §3 est réparé, et démontré réparé en conditions de
> produit** : 44 pertes d'accès `0x887A0026` encaissées, **aucune session
> perdue**, montées 1→2, 2→3, 3→4 propres, une seule clôture de session et elle
> était sollicitée. La cause n'était pas celle que ce document soupçonnait —
> `rouvrir()` demandait une seconde duplication de la même sortie **sans avoir
> relâché la première** —, et il a fallu **trois mesures dont deux réfutations**
> pour l'établir ; le §3.4 a été réécrit en cours de route, après la première.
> **② Le critère de réception du §2.2 n'est PAS atteint** : il exige **cinq**
> fenêtres simultanées, on en atteint **quatre**. La 5ᵉ duplication DXGI, dans
> un 5ᵉ processus, est refusée en `0x887A0022`, par une limite de concurrence
> qui **résiste à trois secondes de patience explicite** ; **la couche qui
> l'impose n'est pas identifiée**, et **rien n'établit que 4 soit une borne du
> système**. **D2 n'est donc pas reçu au sens de son critère, et répare pourtant
> ce pour quoi il existait.** La voie de repli du §8 (sérialisation
> superviseur→enfants) n'a **jamais été implémentée** et reste disponible.

## 1. Ce que D1 a laissé, et ce qui reste vraiment ouvert

D1 a démontré, en session réelle et sur de vraies applications, une fenêtre
navigateur par fenêtre Windows, chacune sur sa sortie virtuelle, chacune
capturée et encodée par son propre processus — **jusqu'à quatre simultanées,
mais seulement pour des fenêtres qui préexistaient au démarrage du
superviseur**. Le cas produit, un utilisateur qui ouvre une application, a été
tenté deux fois et a échoué deux fois.

Le §9 de son rapport liste sept points à lever. Trois ont depuis été refermés
par le correctif final de branche (`e9691eb`), ce que la table ci-dessous relève
**par lecture du code au 1ᵉʳ août 2026**, et non par lecture du rapport :

| §9 | Objet | État réel |
| --- | --- | --- |
| 1 | Créer une sortie tue les captures en cours | **ouvert** — `windows_source.rs:572-579` classe toute erreur d'acquisition en définitive |
| 2 | Désigner la sortie par son nom, pas son index | **ouvert** — `SORTIE_DXGI` porte `adaptateur:sortie` (`lanceur.rs:140`, `main.rs:116`) |
| 3 | `resize` doit connaître le mode `sur_sortie` | **fermé** — `ModeCapture::SortieEntiere` et garde en tête de `resize` |
| 4 | Viewport pair, appariement tolérant | **à moitié** — `region_de_sortie` aligne en pair, mais `sortie_par_dimensions` apparie par **égalité exacte** (`boucle.rs:275`) et rien n'arrondit le viewport en amont |
| 5 | Premier plan avant injection clavier | **ouvert** |
| 6 | Ne pas inonder le journal sur le chemin d'erreur | **fermé de fait** — la garde de `resize` supprime la source du flot |
| 7 | Ne pas perdre une fenêtre vivante quand sa session meurt | **ouvert** — `enfant_mort` retire l'entrée, rien ne la rappelle |

Restent **quatre points et demi**, dont **un seul** est bloquant : le n°1.

## 2. Périmètre et critère de réception

### 2.1 Ce que D2 fait

Les quatre points et demi ci-dessus, et rien de plus.

### 2.2 Critère de réception, posé d'avance

Partant de *k* fenêtres capturées (*k* de 1 à 4), **l'ouverture d'une
application supplémentaire ne tue aucune session préexistante**, répété jusqu'à
cinq fenêtres simultanées.

Mesuré de deux façons concordantes : sur les journaux d'agent, le nombre de
`clôture de session amorcée` non sollicitées vaut **zéro** ; à l'écran, les
fenêtres navigateur préexistantes affichent toujours leur application.

### 2.3 Ce que D2 ne fait pas, explicitement

- **Aucune mesure neuve.** Ni le plafond d'encodeurs en multi-processus, que D1
  devait relever et n'a pas approché, ni la latence, ni la cadence. Elles
  restent dues à un sous-bloc ultérieur.
- **Le redimensionnement d'une fenêtre déjà ouverte** reste hors périmètre : le
  pilote SudoVDA n'expose aucun `SET_MODE`, la sortie ne peut donc pas suivre.
  Tranché en D1, non rouvert ici.
- **La destruction d'une sortie sous duplication ouverte** n'est pas exigée. Le
  cas n'a jamais été exercé — les dix-sept destructions des journaux de D1
  tombent toutes hors de toute duplication ouverte. Si elle produit la même
  erreur que la création, la reprise du §3 la couvrira ; ce serait un bénéfice
  observé, pas une exigence tenue.
- **Le caractère global de `SendInput`.** Voir §5.2.
- **Chercher un plafond de fenêtres.** Cinq est ce que la recette exercera, pas
  une limite trouvée.

## 3. Le défaut central — la perte d'accès devient récupérable

### 3.1 Ce qu'on sait, et ce qu'on n'a fait qu'inférer

**Relevé par D1** : sur les dix-sept créations de sortie des trois journaux
versés, les quatre qui surviennent alors que des duplications sont ouvertes
produisent neuf erreurs `0x887A0026`, soit exactement une par enfant qui
capture ; les treize autres n'en produisent aucune. La correspondance est
observée, **le mécanisme ne l'est pas** : rien dans D1 ne dit *pourquoi* DXGI
abandonne le mutex.

**Inféré, à confirmer par la mesure** : `0x887A0026` est
`DXGI_ERROR_ACCESS_LOST`, que la documentation Desktop Duplication décrit comme
récupérable — la conduite prescrite est de relâcher l'`IDXGIOutputDuplication`
et d'en créer une nouvelle. Notre code, lui, la traite comme définitive.

C'est cette inférence que l'étape 1 de la recette (§6.1) éprouve **avant**
d'engager le reste du sous-bloc.

### 3.2 Classer l'échec là où le `HRESULT` est encore lisible

`capture.rs:211-216` ne connaît aujourd'hui que deux issues : *rien de neuf*
(`DXGI_ERROR_WAIT_TIMEOUT`) et *erreur*, cette dernière remontant en
`anyhow::Error` opaque que `windows_source.rs` déclare fatale. Il en faut trois,
et la troisième doit se décider là où le code d'erreur est encore typé —
**jamais sur le texte du message** : `CLAUDE.md` porte déjà le précédent d'un
libellé Windows qui a fait attribuer un refus au mauvais appel pendant tout un
chantier.

```rust
/// Pourquoi une acquisition d'image a échoué.
pub enum EchecAcquisition {
    /// DXGI a révoqué notre accès à la duplication (DXGI_ERROR_ACCESS_LOST,
    /// 0x887A0026). Rendu notamment quand la topologie d'affichage change —
    /// et la création d'une sortie virtuelle en est un cas, relevé par D1.
    /// Conduite prescrite : recréer la duplication sur la même sortie.
    AccesPerdu,
    /// Toute autre panne, DXGI_ERROR_DEVICE_REMOVED compris : définitive.
    Panne(anyhow::Error),
}
```

`next_frame` rend `Result<Option<CapturedFrame>, EchecAcquisition>`.

**La reprise est tentée par `next_frame` elle-même**, et non par son appelant.
Ce n'est pas indifférent : le banc de diagnostic multi-fenêtres capture par
`DesktopCapture::next_frame` (`diagnostics/multifenetre/voies.rs:146`), sans
passer par `WindowsSource`. Une reprise logée chez l'appelant laisserait donc le
banc en dehors du chemin de production — et l'étape 1 de la recette (§6.1), qui
est un point d'arrêt, ne prouverait rien de ce qui tournera réellement.
`EchecAcquisition::AccesPerdu` n'atteint l'appelant qu'une fois la fenêtre de
reprise expirée (§3.4), et vaut alors « je n'ai pas pu revenir ».

### 3.3 La reconstruction est étroite — c'est le point de conception qui compte

`DesktopCapture::rouvrir` ne refait **que** l'`IDXGIOutputDuplication` : elle
conserve le périphérique D3D11, son contexte, et la protection multifil déjà
posée dessus.

Ce n'est pas une économie, c'est une nécessité. L'encodeur H.264 est lié à
`capture.device()` par l'`IMFDXGIDeviceManager` (`encode::share_device`) :
recréer le périphérique obligerait à détruire l'encodeur, donc à emprunter
`Drop for H264Encoder`, dont `CLAUDE.md` borne le pire cas à **8 s** et où un
gel non attribué a déjà été observé. Une reprise qui doit passer inaperçue ne
peut pas payer ce prix.

Si c'est le périphérique lui-même qui est perdu, DXGI rend un code **distinct**
(`DXGI_ERROR_DEVICE_REMOVED`), qui reste classé `Panne` et donc fatal. La
distinction est faite par le code, pas par nous.

### 3.4 Une fenêtre de reprise bornée en DURÉE, non bloquante

> ⚠️ **Cette section a été réécrite le 1ᵉʳ août 2026, après mesure.** Sa
> première rédaction posait « un nombre maximal de reprises **consécutives** »,
> et le plan en avait tiré `REPRISES_MAX = 3` sans délai. **La mesure du §6.1 a
> réfuté ce calibrage**, sans réfuter la voie : voir l'encadré de relevé
> ci-dessous. Le défaut était dans cette spec, pas dans son implémentation.

**Ce que la mesure a relevé** (banc `MULTIFENETRE_REPRISE`, rangs 1, 2 et 4,
une exécution chacun — donc aucun taux ; journaux
`plans/journaux-multifenetres-d2/reprise-n{1,2,4}.log`) :

- créer une sortie rend `ACCESS_LOST` sur **toutes** les duplications ouvertes,
  ce qui reproduit le défaut central de D1 ;
- **`rouvrir()` réussit** — la duplication est bien reconstruite ;
- **mais elle rend aussitôt `ACCESS_LOST` à son tour**, et ce tant que Windows
  n'a pas fini de reconfigurer sa topologie ;
- les trois tentatives sans délai sont donc brûlées en **14 à 21 ms**, et les
  voies meurent 66 à 232 ms après la perturbation, aux trois rangs, 0 survivante
  sur 7 ;
- **7 sondes post-mortem sur 7** rapportent que la même sortie se rouvre et rend
  une image une fois la topologie stabilisée.

**Le budget comptait des tentatives là où le phénomène a une durée.** C'est la
seule chose que la mesure corrige.

**La forme retenue.** Une **fenêtre de reprise**, portée par `DesktopCapture`,
ouverte à la première perte d'accès et fermée par le premier succès :

- tant que la fenêtre court, `next_frame` retente `rouvrir` **au plus une fois
  par pas d'attente**, puis rend `Ok(None)` — « rien de neuf », le cas le plus
  banal de cette API ;
- **elle ne bloque jamais.** La reprise s'étale sur plusieurs appels au lieu de
  dormir dans un seul. C'est ce qui distingue cette forme d'un simple « plus de
  tentatives, avec délai » : `next_frame` est appelée depuis la boucle de
  `Session::run`, et l'y bloquer trois secondes suspendrait du même coup les
  demandes de keyframe, les changements de barreau de l'adaptation réseau et le
  traitement des redimensionnements ;
- à l'expiration, et alors seulement, `EchecAcquisition::AccesPerdu` remonte à
  l'appelant, qui déclare la source épuisée comme aujourd'hui.

**Deux valeurs, et ce qui les fonde** : la fenêtre doit couvrir largement les
**3 s** que ce dépôt admet déjà pour qu'une topologie se stabilise
(`DELAI_TOPOLOGIE`), et le pas d'attente doit être petit devant elle pour ne pas
retarder la reprise réelle. ⚠️ **Le seuil réel n'est pas mesuré** : on sait que
~20 ms ne suffisent pas et que 3 s suffisent, rien entre les deux. Les valeurs
retenues sont donc **majorantes et non calibrées**, et il faut le dire partout
où elles apparaissent.

**La fenêtre se ferme sur un succès d'acquisition, `Ok(None)` compris** — c'est-
à-dire dès que `AcquireNextFrame` cesse de refuser, même sans image neuve. La
première rédaction disait « dès qu'une image passe », ce qui était trop étroit :
un bureau immobile ne produit aucune image pendant de longues périodes, et une
fenêtre qui ne se refermerait que sur une image livrée transformerait des pertes
d'accès rares et sans rapport entre elles en une usure.

Reste inchangé : si la sortie n'existe plus au moment de rouvrir, ce n'est pas
une panne mais le cas légitime de la fenêtre qu'on vient de fermer.

Une trace `info!` par tentative de réouverture — rare par construction, et c'est
par elle que le diagnostic se fera. **Aucune trace par image ni par tentative
d'acquisition** : la leçon du chantier TURN vaut ici aussi, et le pas d'attente
est précisément ce qui borne le débit de cette trace.

### 3.5 Où ce code vit

`capture.rs` est à 463 lignes et `windows_source.rs` à 648 (dette gelée,
`CLAUDE.md`). La reprise vit donc dans un module enfant `capture/reprise.rs`.

Le classificateur (d'un code d'erreur vers `EchecAcquisition`) et la fenêtre de
reprise (ouverture, pas d'attente, expiration, fermeture sur succès) y sont des
**fonctions et types purs** — la fenêtre raisonne sur des instants qu'on lui
passe, elle ne lit aucune horloge elle-même, sans quoi elle ne serait pas
testable —,
au-dessus du `#[cfg(windows)]`, donc testables sur l'hôte Linux — c'est le
motif déjà employé par `windows_source/sortie.rs`, et la seule part de ce
chantier qu'un test automatisé peut couvrir.

## 4. Désigner une sortie par son nom

`(index_adaptateur, index_sortie)` est **positionnel** : il change dès qu'une
sortie apparaît ou disparaît. C'est déjà un défaut au démarrage de l'enfant —
les `aucune sortie DXGI à l'index adaptateur 0, sortie 5` de D1 — et la reprise
du §3 le rend doublement gênant, puisqu'elle doit retrouver *sa* sortie après
un remaniement de la topologie.

`\\.\DISPLAYn` est stable, et `enumerer_sorties` le rend déjà (`nom_sortie`).
Trois retouches :

1. `DesktopCapture::sur_sortie(nom: &str)` résout par le nom, à l'ouverture
   **et** à chaque réouverture.
2. La variable d'environnement `SORTIE_DXGI` porte le nom au lieu du couple
   d'index. `lanceur.rs:140` a `cible.nom_sortie` sous la main ; `main.rs:116`
   cesse d'analyser un couple d'entiers. Elle reste un contrat interne entre le
   superviseur et ses enfants : aucune compatibilité ascendante à tenir.
3. La table du superviseur retient le nom. `prises`, qui garde aujourd'hui des
   couples d'index pour éviter d'apparier deux sessions à la même sortie,
   devient un ensemble de noms — ce qui la rend juste au lieu
   d'approximativement juste. `Effet::LancerEnfant` et `Effet::DetruireSortie`
   suivent : la seconde continue de porter **les deux** identifiants (celui du
   pilote pour détruire, celui de DXGI pour libérer la place), sans relation
   calculable entre eux, comme sa documentation actuelle l'explique.

`MULTIFENETRE_SORTIE`, la variable du banc de diagnostic, garde sa forme
actuelle : le banc ne survit pas à un remaniement de topologie et n'a rien à
gagner au changement.

> ⚠️ **Décision DÉPASSÉE à l'exécution — la variable a changé de forme.** Elle ne
> pouvait pas ne pas changer : le point 1 ci-dessus fait de
> `DesktopCapture::sur_sortie` une fonction qui prend un **nom** DXGI et ne
> prend plus d'index, et le banc passe la valeur de `MULTIFENETRE_SORTIE`
> directement à cette fonction. « Garder sa forme actuelle » aurait exigé de
> conserver en plus, pour le seul banc, la résolution par couple d'index que le
> point 1 supprime. `MULTIFENETRE_SORTIE` attend donc désormais un nom
> `\\.\DISPLAYn` (`diagnostics/multifenetre.rs`, `diagnostics/multifenetre/banc.rs`),
> comme `SORTIE_DXGI`. Le raisonnement de la décision reste juste — le banc n'y
> gagne rien — il ne tenait simplement pas compte du coût du *maintien*.

## 5. Les trois défauts d'accompagnement

### 5.1 Apparier une sortie fraîchement créée

Trois corrections de natures différentes.

**a. La shell arrondit à des dimensions paires le viewport qu'elle annonce.**
`1280×713` est le cas banal d'un pop-up de navigateur, et une sortie créée à
hauteur impaire ne pourra jamais égaler la source, que `region_de_sortie`
aligne en pair pour l'encodeur NV12. L'arrondi se fait **à la source**, dans
`client/src/main.ts` (l. 41-44), là où `window.innerWidth`/`innerHeight` sont
lus et déjà passés par `Math.round`.

**b. L'appariement devient tolérant.** `placement::sortie_par_dimensions` cesse
d'exiger l'égalité exacte et emploie la même tolérance que le replacement
(`TOLERANCE_PX`, 4 px). Le journal d'échec continue d'énumérer les **candidats**
et pas seulement la demande : c'est ce qui a permis de départager, en D1, une
course de rattachement d'un facteur DPI de 1,5.

**c. `DELAI_RATTACHEMENT` devient une attente sur condition observable.**
1500 ms plats se sont révélés insuffisants au moins une fois dans les journaux
de D1. À la place : scruter la topologie jusqu'à ce qu'une sortie neuve
apparaisse, **bornée dans le temps**, en continuant de pinguer le chien de garde
à chaque tour — la contrainte que `attendre_en_pinguant` porte déjà, et qui ne
doit pas se perdre dans la réécriture. On attend un fait, pas une durée : c'est
la leçon de la sentinelle d'`encode/arret.rs`.

### 5.2 Le clavier : donner le premier plan, et déclarer la limite

`SetForegroundWindow` sur la fenêtre de la session avant d'injecter.

**Nécessaire, et probablement pas suffisant** : `SendInput` reste global à la
session Windows. Deux fenêtres qui reçoivent des frappes simultanées se
disputeront le premier plan, et rien dans cette conception ne l'empêche. D2 pose
le premier plan, observe ce que ça donne, et **déclare la limite dans son
rapport**. La réponse structurelle — injection ciblée par messages de fenêtre,
ou un pilote — est hors périmètre.

`SetForegroundWindow` **échoue silencieusement** quand le processus appelant n'a
pas le droit de voler le focus (règles de `AllowSetForegroundWindow`). Son
retour est donc vérifié et journalisé : sans cela, cette mitigation serait
muette, et on ne saurait pas distinguer « le premier plan n'a pas suffi » de
« le premier plan n'a jamais été donné ».

Rappel du second suspect, non départagé par D1 et non traité ici : côté
navigateur, le premier clic est consommé par `requestPointerLock`, qui exige une
activation utilisateur qu'un clic CDP ne fournit pas sans interface.

### 5.3 Une fenêtre vivante ne doit pas être oubliée

`enfant_mort` efface aujourd'hui l'entrée de la table ; plus rien ne rappelle la
fenêtre, sauf un `SHOW` fortuit de Windows. Une fenêtre bien vivante peut donc
disparaître de la shell pour toujours — c'est ce qui, en D1, laissait la
page-shell vide alors que les quatre applications tournaient encore.

L'entrée est désormais **marquée sans session** plutôt que supprimée, et rendue
éligible au contrôle périodique du superviseur, qui énumère déjà les fenêtres.

Avec le garde-fou que l'emballement de D1 rend obligatoire : un **compteur
d'échecs par fenêtre**, et au-delà d'un nombre fixé de relances, abandon
**annoncé à la shell** (`Effet::AnnoncerRefus`) plutôt que relance en boucle. Sans lui, une
fenêtre dont l'enfant meurt systématiquement produirait exactement la boucle
`w-5, w-6, w-7, w-8…` observée en D1.

La table reste ce qu'elle est : logique pure, sans effet de bord, rendant des
`Effet` que le superviseur exécute. C'est ce qui rend cette règle-ci
éprouvable sans Windows.

## 6. Recette

### 6.1 Étape 1 — le banc, qui isole la cause, et qui est un point d'arrêt

Une sonde `MULTIFENETRE_REPRISE=<k>` dans `diagnostics/multifenetre/`, **sans
navigateur ni signaling** : elle crée *k* sorties virtuelles, y ouvre *k*
duplications, capture quelques secondes, puis **crée une sortie de plus** et
vérifie que les *k* duplications reprennent et rendent à nouveau des images
justes.

Relevé attendu, chiffré : images avant et après la création, nombre de reprises
effectives par voie, verdicts de justesse en rotation — le protocole de contrôle
d'image du chantier des duplications parallèles, réemployé tel quel.

**C'est un point d'arrêt.** Si les duplications ne reprennent pas, l'approche
est réfutée et il faut basculer sur la sérialisation (§8) — sans avoir engagé
le reste. C'est tout l'intérêt de faire cette étape en premier.

### 6.2 Étape 2 — la démonstration bout en bout

Superviseur, page-shell, navigateur, applications Windows réelles. *k* fenêtres
préexistantes, puis ouverture d'applications une à une par WinRM jusqu'à cinq.

Les pièges relevés par D1 sont ici des **contraintes de protocole**, pas des
recommandations :

- lancer le navigateur **avant** le superviseur — le signaling ne mémorise que
  les offres SDP, et les annonces `fenetre-ouverte` émises trop tôt sont perdues
  sans trace ;
- `--disable-popup-blocking`, sans quoi la démonstration est vide et muette ;
- **aucune capture d'écran CDP pendant la mesure** : en D1 c'est elle qui
  déclenchait l'effondrement, en provoquant un `Resize`. Captures en fin de
  séquence seulement ;
- vérifier `Get-Process agent` **avant** de démarrer : un agent survit à
  l'hibernation de la VM et `run-agent.sh` ne le tue pas ;
- vérifier **après** la séquence que la VM n'a pas hiberné (`virsh list --all`,
  journal libvirt) — deux mesures de D1 ont été perdues ainsi ;
- s'assurer qu'on parle au bon navigateur : un port de débogage qui répond ne
  prouve pas que l'instance est la nôtre.

## 7. Tests

Ce qui est pur, donc ce qui doit être couvert par des tests tournant sur l'hôte
Linux :

- la classification d'un code d'erreur en `EchecAcquisition` — dont le cas
  `DXGI_ERROR_ACCESS_LOST`, le cas `DXGI_ERROR_DEVICE_REMOVED` qui doit rester
  fatal, et le cas `WAIT_TIMEOUT` qui ne doit pas devenir un échec ;
- la fenêtre de reprise : ouverture à la première perte, respect du pas
  d'attente, expiration au bout de la durée, fermeture par un succès — dont le
  cas `Ok(None)`, qui doit la fermer aussi ;
- l'appariement tolérant : une sortie à ±4 px apparie, au-delà non ;
- l'arrondi pair du viewport, côté client (`client/src/main.ts`, suite `vitest`
  existante) ;
- la table du superviseur : une fenêtre dont la session meurt reste connue, elle
  est reproposée, et le compteur d'abandon la retire après le nombre de
  relances fixé, en annonçant un refus.

Le reste — COM, DXGI, pilote SudoVDA, Media Foundation — n'est éprouvé que par
le banc et la démonstration, comme tout le code `#[cfg(windows)]` de ce dépôt.

## 8. Ce qui a été écarté, et pourquoi

**Préallouer un vivier de sorties au démarrage.** Créer les huit sorties avant
toute duplication supprimerait le déclencheur. Mais le pilote SudoVDA n'expose
aucun `SET_MODE` parmi ses six IOCTL : la taille d'une sortie est figée à sa
création. On renoncerait donc à « une sortie à la taille du viewport annoncé par
le navigateur », qui est précisément un acquis de D1. Écarté.

**Sérialiser : faire relâcher, créer, faire rouvrir.** Le superviseur ordonne
aux enfants de lâcher leur duplication, crée la sortie, leur dit de rouvrir.
Correct, mais coûteux : il faut un canal superviseur→enfants qui n'existe pas
aujourd'hui, et toutes les fenêtres gèlent le temps du rattachement. **Gardé en
réserve** : c'est la voie de repli si l'étape 1 de la recette réfute la reprise.

## 9. Risques assumés

- **Une reprise peut échouer à son tour** si la topologie bouge encore pendant
  la reconstruction — deux fenêtres ouvertes coup sur coup. La fenêtre du §3.4
  se referme sur le premier succès et se rouvre à la perte suivante : deux
  remaniements espacés sont donc couverts. Ce qui ne l'est pas, c'est un
  remaniement **continu** plus long que la fenêtre elle-même. C'est une limite,
  et elle sera dite dans le rapport plutôt que découverte par un utilisateur.
- **Le seuil réel de stabilisation de la topologie n'est PAS mesuré** : la
  mesure du §6.1 établit que ~20 ms ne suffisent pas et que 3 s suffisent, rien
  entre les deux. La durée de fenêtre retenue est **majorante et non calibrée**,
  au même titre que `BPP_MIN` l'est pour le contrôleur de congestion de ce
  dépôt.
- **Le premier plan peut ne pas suffire au clavier** (§5.2), et D2 ne s'engage
  pas à le faire fonctionner — seulement à le tenter et à rendre compte.
- **Cinq fenêtres est ce que la recette exercera**, pas un plafond mesuré.
  Aucune conclusion sur le nombre maximal de fenêtres ne sortira de ce
  sous-bloc.
- **Le mécanisme de l'abandon du mutex reste inconnu**, y compris si la reprise
  fonctionne. On saura la traiter, pas l'expliquer.
