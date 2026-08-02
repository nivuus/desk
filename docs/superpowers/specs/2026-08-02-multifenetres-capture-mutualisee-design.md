# Chantier D, sous-bloc D4 — capture mutualisée

**Date** : 2 août 2026
**Chantier parent** : D (multi-fenêtres), `2026-07-28-support-jeux-design.md` §4 et §5 D
**Sous-bloc précédent** : D3, `2026-08-02-multifenetres-plafond-concurrence-design.md`
**Statut** : conception validée, plan à écrire

---

## 1. Pourquoi ce sous-bloc, et d'où vient son numéro

### 1.1 Ce que D3 a désigné

D3 a établi, par une campagne de 15 exécutions, que le plafond de quatre fenêtres
porte sur le **nombre de processus** concurrents tenant une duplication DXGI, et
qu'il vaut **exactement 4** — huit duplications passent dès lors qu'elles sont
réparties sur au plus quatre processus, et le refus tombe au cinquième processus
en `0x887A0022`. Le fait décisif est une **exclusion positive** : le rang qui
échoue n'a que 4 duplications ouvertes quand des rangs qui réussissent en ont 8.

En conséquence, et selon une règle de décision **écrite avant la mesure**
(conception D3 §3.5, ligne H1), la **capture mutualisée** — un seul processus
tenant les N duplications et distribuant le média — a été **désignée** pour le
sous-bloc suivant et **non implémentée** par D3, qui a par ailleurs ramené
`CAPACITE` de 8 à 4.

**C'est le présent document.** Tant que la capture n'est pas mutualisée, la cible
de huit fenêtres du chantier D reste hors de portée, et le superviseur refuse
proprement la cinquième fenêtre.

### 1.2 La numérotation a dérivé, et il faut le dire

Le découpage annoncé par la conception de D1 (§1) n'a pas été suivi à la lettre :

| Slot d'origine | Objet annoncé | Ce qui a réellement été exécuté |
| --- | --- | --- |
| D1 | tranche verticale | ✅ conforme |
| D2 | cycle de vie complet | ✅ exécuté sous le titre « arrangement dynamique » |
| D3 | partage de la capacité réseau entre N flux, et audio par fenêtre | ❌ **non exécuté** — le D3 réel fut « retenir les sorties, caractériser le plafond » |
| D4 | plein écran + Keyboard Lock | ⬜ non exécuté |
| D5 | mise en sommeil des fenêtres masquées | ⬜ non exécuté |

Le présent sous-bloc porte le numéro **D4** parce que c'est ainsi que les
résultats de D3 le nomment (§7.1), et non parce qu'il occupe le slot D4
d'origine. **Les trois objets non exécutés ci-dessus restent dus** et se
renumérotent à la suite :

- **D5** — mise en sommeil des fenêtres masquées (voir §7.1 : ce sous-bloc la
  rend nécessaire pour dépasser huit) ;
- **D6** — partage de la capacité réseau entre N flux, et audio par fenêtre ;
- **D7** — plein écran + Keyboard Lock.

Cet ordre n'est pas neutre : D4 déplace le plafond sur les encodeurs, ce qui fait
de D5 la condition de viabilité au-delà de huit fenêtres, tandis que D6 et D7
sont des fonctionnalités que le plafond ne bloque pas.

---

## 2. État de départ — ce que le code fait aujourd'hui

**Un processus enfant par fenêtre, capture et encodage compris.** Le superviseur
(`agent/src/superviseur/`) tient le hook `SetWinEventHook`, le canal IOCTL vers
le pilote SudoVDA, la `Table` d'attribution et le placement des fenêtres. Pour
chaque fenêtre retenue il crée une sortie virtuelle, y pose la fenêtre, puis
lance un processus enfant en lui passant `SESSION_ID`, `FENETRE_HWND`,
`SORTIE_DXGI` et `AUDIO` **par variables d'environnement**
(`superviseur/lanceur.rs:134-140`).

**Il n'existe aucun canal entre le superviseur et ses enfants.** La conception de
D1 (§3.1) le notait comme un « coût assumé » ; il n'a jamais été payé. La seule
communication est l'environnement au lancement, et la mort du processus.

**Côté enfant**, `demarrage/source.rs` construit un `WindowsSource::sur_sortie`
qui réunit une `DesktopCapture` (duplication DXGI + recadrage GPU) et un
`H264Encoder` (Media Foundation, NVENC) derrière le trait `VideoSource`
(`agent/src/source.rs`). La boucle de transport str0m consomme ce trait et rien
d'autre du média vidéo.

**La couture est donc déjà là, et elle est complète.** Les huit méthodes de
`VideoSource` sont exactement ce que la boucle appelle :

| Méthode | Appelée depuis |
| --- | --- |
| `next_frame` | `transport/piste_video.rs:103` |
| `is_exhausted` | `transport/piste_video.rs:130` |
| `is_alive` | `transport/tick.rs:140` |
| `dimensions` | `transport.rs:268`, `transport/redimensionnement.rs:32` |
| `resize` | `transport/redimensionnement.rs:26` |
| `set_bitrate` | `transport/adaptation.rs:89` |
| `set_encode_size` | `transport/adaptation.rs:102` |
| `request_keyframe` | `transport/evenements.rs:78` |

Aucun autre point du transport ne touche à la capture ni à l'encodage. C'est
cette propriété qui rend le présent sous-bloc réalisable sans refonte du
transport.

---

## 3. Décisions actées

### 3.1 Trois étages : superviseur, capteur, enfants

**Superviseur** — rôle inchangé : hook, pilote SudoVDA, `Table`, placement,
session de contrôle avec la page-shell. **Il ne duplique rien et n'encode rien**,
exactement comme le porteur de D3, qui ne dupliquait pas *précisément pour
survivre et détruire ses sorties*. Il gagne une seule responsabilité : lancer le
capteur et le maintenir en vie.

**Capteur** — processus neuf, **un seul**, portant **N fils**, chacun tenant une
duplication DXGI, un encodeur H.264 et le canal vers son enfant.

**Enfants** — N processus au rôle réduit : str0m/WebRTC, injection d'entrée
(`SetForegroundWindow` puis `SendInput`), audio pour celui qui porte `AUDIO=1`,
et l'adaptation réseau. Ils ne touchent plus ni DXGI ni Media Foundation.

**Ce qui fonde cet arrangement, et ce qui ne le fonde pas.** Deux mesures
existantes le soutiennent, et il faut être exact sur leur portée :

- le banc du 31 juillet 2026 a tenu **8 duplications et 8 encodeurs de front dans
  un seul processus**, à 90,1 i/s par fenêtre et zéro verdict faux — mais sur des
  **mires D3D11**, une exécution par rang, sans redimensionnement ni
  recouvrement, et **sur huit périphériques D3D11 distincts** ;
- le rang témoin `1x8` de D3 a un processus qui **crée** les sorties et un
  **autre** qui les duplique, et il passe 3/3 — ce qui vaut pour l'asymétrie
  superviseur/capteur retenue ici, et pour rien d'autre : ces sondes n'encodent
  rien et ne capturent aucune image.

**Aucune des deux ne mesure l'arrangement de ce sous-bloc.** Elles rendent
seulement improbables les deux façons dont il pourrait échouer d'emblée.

**Le capteur ne réécrit aucun code de capture ni d'encodage.** Son fil de fenêtre
instancie `WindowsSource::sur_sortie(hwnd, nom_sortie, fps, debit, clock_origin)`
**tel quel** — ce type est déjà exactement le couple `DesktopCapture` +
`H264Encoder` derrière le trait `VideoSource` (`agent/src/windows_source.rs`), et
le capteur ne fait qu'appeler ce trait depuis un autre processus. `capture.rs`,
`capture/*`, `encode.rs` et `windows_source.rs` **ne sont ni déplacés ni
modifiés**.

C'est la simplification centrale du sous-bloc : elle retire son risque le plus
lourd — aucune régression possible sur les chemins de capture, d'encodage, de
reprise après perte de mutex ou de libération d'encodeur, puisque pas une ligne
n'y change. Ce qui est neuf se réduit au canal, à son protocole, et à la
`SourceDistante` qui le consomme.

### 3.2 Un seul canal, et l'enfant s'y décrit lui-même

Le capteur est un **serveur de tube nommé** sur un nom bien connu. Chaque enfant
s'y connecte et se décrit dans son premier message ; le capteur ouvre alors un
fil, une duplication et un encodeur pour lui.

**Il n'y a donc pas de canal superviseur → capteur.** L'enfant possède déjà tout
ce qui décrit sa fenêtre (`SESSION_ID`, `FENETRE_HWND`, `SORTIE_DXGI` lui sont
passés au lancement) ; le faire transiter par le superviseur n'ajouterait qu'un
état partagé entre trois processus et un séquencement à orchestrer.

**Ce que cette forme achète** : la fermeture du tube **est** le signal de fin de
vie. La mort d'un enfant démonte sa duplication et son encodeur sans que personne
n'ait à l'annoncer, et le capteur n'a aucune table à tenir à jour.

**Course à traiter** : un enfant peut démarrer avant que le tube n'existe (au tout
premier lancement, ou pendant une relance du capteur). La connexion est donc
retentée dans une **fenêtre bornée**, sur le patron de `capture/reprise.rs`.

### 3.3 Le protocole est le trait `VideoSource`, sérialisé

| Message enfant → capteur | Réponse | Méthode couverte |
| --- | --- | --- |
| `Attache { session, hwnd, sortie, fps, debit, origine_qpc }` | `Attachee { largeur, hauteur }` \| `Refus { motif }` | construction |
| `Redimensionner { largeur, hauteur }` | `Taille { largeur, hauteur }` \| `Erreur { motif }` | `resize` |
| `TailleEncodage { largeur, hauteur }` | `Fait` \| `Erreur { motif }` | `set_encode_size` |
| `Debit { bps }` | `Fait` \| `Erreur { motif }` | `set_bitrate` |
| `ImageCle` | `Fait` \| `Erreur { motif }` | `request_keyframe` |

| Message capteur → enfant, non sollicité | Effet côté enfant |
| --- | --- |
| `Image { pts_90k, cle, octets }` | mise en file ; `next_frame` la dépile |
| `Etat { vivante, epuisee, largeur, hauteur }` | met à jour le cache lu par `is_alive`, `is_exhausted` et `dimensions` |

**`is_alive`, `is_exhausted` et `dimensions` ne sont pas des requêtes.** Elles
lisent un cache que `Etat` met à jour, et `Etat` n'est émis **qu'au changement** :
`is_alive` est interrogée à chaque tour de la boucle de transport et `dimensions`
doit rester synchrone et gratuite. Une requête par prédicat serait un coût par
tour pour une information qui ne bouge presque jamais.

**Cadrage** : `u32` de longueur (petit-boutiste) + `u8` d'étiquette + corps. Les
messages de contrôle en JSON — `serde_json` est déjà une dépendance ; l'unité
d'accès en **binaire brut** derrière un en-tête fixe (8 octets de `pts_90k`,
1 octet d'image clé, puis les octets Annex-B), **jamais en base64**.

**Le module de protocole vit hors de tout `#[cfg(windows)]`**, comme
`superviseur/protocole.rs` et pour la raison qu'il énonce : c'est de la
sérialisation pure, donc le genre de contrat qui doit être éprouvé sur l'hôte
plutôt qu'en session réelle sur la VM.

**Débit** : 8 Mb/s par fenêtre × 8 ≈ 8 Mo/s sur des tubes locaux. Sans objet.

**`origine_qpc` — l'horloge ne traverse pas un processus.** `clock_origin` est un
`std::time::Instant`, imposé par `demarrage.rs` et **partagé avec la source
audio** : c'est cette origine commune qui rend les deux lignes de temps
comparables, donc la synchro A/V exacte (commentaire du champ dans
`windows_source.rs`). Un `Instant` n'a aucun sens dans un autre processus. Si le
capteur horodatait sur sa propre origine, l'enfant porteur du son verrait sa
vidéo décalée de l'écart entre les deux origines — l'intervalle entre le
démarrage de l'enfant et son attache, soit potentiellement des centaines de
millisecondes.

`Attache` porte donc `origine_qpc`, la valeur de `QueryPerformanceCounter` lue
par l'enfant **au moment même** où il crée son `clock_origin`. QPC est monotone
et **commun à tous les processus** de la machine : le capteur reconstruit
l'`Instant` équivalent chez lui (`Instant::now()` moins l'écart converti par
`QueryPerformanceFrequency`) et le passe à `WindowsSource::sur_sortie`, dont
l'horodatage reste inchangé.

### 3.4 Les images vont en push, les commandes en requête/réponse

Le fil du capteur pousse chaque unité d'accès dès qu'elle est produite ; côté
enfant, un fil lecteur la met en file et `next_frame` ne fait que **dépiler sans
bloquer**. Les commandes — `resize`, `set_bitrate`, `set_encode_size`,
`request_keyframe` — restent en requête/réponse : elles sont rares.

**Pourquoi pas en pull, alors que le trait est en pull.** Parce que le pull sur
IPC détruirait la propriété sur laquelle repose le réglage actuel.
`FRAME_INTERVAL` vaut **10 ms, soit 100 Hz** (`transport/piste_video.rs:40`), et
c'est délibéré : l'enfant sur-interroge sa source, qui produit à ~90 i/s, parce
qu'« interroger plus souvent que la source ne produit lève cette borne **sans
rien coûter** quand il n'y a rien à prendre — `AcquireNextFrame` est appelée avec
un délai NUL, donc un tour à vide se résume à un aller-retour DXGI immédiat »
(commentaire de ce même fichier, l. 28-31).

En pull sur tube, **un tour à vide cesse d'être gratuit** : il devient un
aller-retour qui bloque la boucle de transport, laquelle porte aussi ICE et RTP.
À 100 Hz par fenêtre et 8 fenêtres, c'est 800 allers-retours par seconde dont la
grande majorité ne rapporte rien, et surtout c'est de la gigue injectée dans la
boucle qui gère la connectivité. En push, un tour à vide redevient une lecture de
file — exactement le coût d'aujourd'hui.

**Bénéfice annexe** : le fil du capteur peut appeler `AcquireNextFrame` avec un
délai **non nul**, donc dormir jusqu'au prochain changement du bureau, au lieu du
tour de garde actif qu'un producteur libre en pull imposerait.

**La file est bornée et exerce une contre-pression** : **une unité d'accès ne peut
pas être jetée** sans corrompre le flux jusqu'à la prochaine image clé — les
images P référencent les précédentes. File pleine (enfant bloqué dans un `resize`,
par exemple) : le fil du capteur attend, et il n'attend que pour **sa** fenêtre.

Le profil de latence attendu est donc celui d'aujourd'hui, augmenté du trajet
d'une unité d'accès sur un tube local — **attendu, non mesuré** : voir le §4,
critère 3.

### 3.5 `CAPACITE` est rouverte, sans être décrétée

`CAPACITE = 4` (`superviseur/boucle.rs:64`) encode exactement le plafond que ce
sous-bloc lève. Elle **ne remonte pas à 8 par décret** : elle prend la valeur du
rang que la recette atteint réellement, documentée pour ce qu'elle est — une
valeur mesurée sur cette VM, non prouvée être une borne du système.

### 3.6 Ce que l'isolation devient

Le motif du §3.1 de D1 — ces API échouent par **plantage du processus**, payé
trois fois par ce projet — reste valide. Il se reporte du couple « une fenêtre =
un processus » vers « le média d'un côté, le transport de l'autre ».

**C'est un recul d'isolation réel, et c'est le prix de la voie retenue** : un
plantage du capteur emporte les N images. Il n'emporte en revanche ni les N
sessions WebRTC, ni le superviseur, ni les sorties virtuelles : le capteur est
relançable et les enfants se raccrochent. **Cette mitigation se prouve** — c'est
le critère 2 du §4 — elle ne s'affirme pas.

---

## 4. Critère de réception

Trois critères **indépendants**. Aucun n'est un nombre de fenêtres : D2 a échoué
au sien parce qu'il portait sur un nombre gouverné par une couche que personne
n'avait identifiée, et D3 a explicitement refusé de reproduire cette erreur.

**Critère 1 — le plafond de quatre ne s'applique plus, et le suivant est nommé.**
En conditions de produit, sur de vraies applications, le nombre de fenêtres
simultanées **dépasse 4** ; et le rang auquel la montée s'arrête est **identifié
et nommé** : l'appel exact et son `HRESULT`, relevés dans le journal — pas « ça a
échoué ». Un plafond qui tomberait à 5 serait un résultat recevable **à condition
d'être expliqué**.

> Rappel de méthode, payé au chantier des mesures préalables : **ne jamais se
> fier au texte d'un HRESULT pour désigner un appel.** `MF_E_UNSUPPORTED_D3D_TYPE`
> parle du type d'entrée alors que l'appel refusé règle la sortie ; il a fallu dix
> annotations de contexte pour trancher. Tout appel du chemin de construction
> ajouté par ce sous-bloc porte son propre contexte.

**Critère 2 — tuer le capteur ne tue aucune session WebRTC.** Le capteur est tué
net pendant que N fenêtres diffusent ; le superviseur le relance ; les enfants se
raccrochent ; l'image revient. Relevé attendu : **zéro** `clôture de session
amorcée` non sollicitée sur la fenêtre temporelle de l'épreuve, bornée par des
horodatages explicites.

> Le journal se relève **fenêtré par bornes temporelles**, jamais comme un total
> de fichier : D2 a payé cette confusion (44 dans la fenêtre, 50 en fin de
> fichier).

**Critère 3 — la cadence est relevée, avant et après.** Images par seconde et par
fenêtre, sur la même VM, dans le même arrangement, avec et sans mutualisation.
Ce relevé n'a **pas de seuil de réception** : il existe pour que le document de
résultats puisse dire ce que le trajet IPC coûte, au lieu de l'affirmer.

> Motif : le projet n'a **jamais** mesuré cadence ni latence en conditions de
> produit — D1, D2 et D3 le déclarent tous les trois. Écrire « profil de latence
> inchangé » sans relevé serait exactement le mode de défaillance dominant des
> rapports de ce projet : les onze rondes de correction de la sonde de capture
> multi-fenêtres ont « quasiment toutes porté sur des rapports qui affirmaient
> au-delà de leur relevé, jamais sur des bugs » (`CLAUDE.md`).

---

## 5. Risques et pièges connus

**La réouverture simultanée intra-processus n'a jamais été éprouvée.** Créer une
sortie virtuelle abandonne toujours le mutex des duplications ouvertes
(`0x887A0026`) — ni évité ni expliqué depuis D1. La reprise de D2 déménage telle
quelle dans le capteur, par fil ; mais les N réouvertures deviennent
**intra-processus et simultanées**, ce que rien n'a mesuré : le rang `1x8` de D3
tenait bien 8 duplications, **aucune sortie n'a été créée pendant qu'il les
tenait**.

**Le démontage doit se faire sur le fil de la fenêtre, jamais sur la boucle
d'acceptation.** `Drop for H264Encoder` porte un risque **observé** :
`IMFShutdown::Shutdown` non borné, un gel vu 1 fois sur 6 à N=4, cause non
attribuée, borne du pire cas de **8 s** pour la partie bornée et **rien ne borne
le total**. Aujourd'hui il figerait une session ; centralisé, il figerait les
huit. Corollaire : **ne pas redescendre en `debug!` les deux traces `info!` qui
encadrent cet appel** — l'exploitation tourne en `RUST_LOG=info` et une
mitigation muette n'en est pas une.

**`is_exhausted` doit rester faux pendant une reprise de canal.** Sinon
`brancher_video` appelle `begin_ending("source vidéo épuisée")`
(`transport/piste_video.rs:130`) et l'on perd les sessions que la relance du
capteur devait sauver. C'est le point exact où le critère 2 se gagne ou se perd.

**`scripts/run-agent.sh` ne transmet pas les variables neuves.** Piège payé trois
fois : `SUPERVISEUR` en D1, `MULTIFENETRE_REPRISE` en D2, évité en D3 en ajoutant
la variable dans la même tâche que le mode. Toute variable neuve de ce sous-bloc
y est ajoutée explicitement, sinon l'agent démarre sans elle **et sans rien
signaler**.

**`scripts/build-agent.sh` lancé sans avoir sourcé `.env` s'arrête en silence**,
et le symptôme se lit exactement comme une compilation réussie et muette.

**La VM se met en veille prolongée toute seule**, cause non identifiée,
intervalles observés de 70, 60 puis 50 minutes. Contrôler sa survie après chaque
séquence longue.

**`cargo check --target x86_64-pc-windows-gnu` avant toute compilation
distante.** Acquis de D3 : il couvre types, emprunts, visibilités et durées de
vie du code `#[cfg(windows)]` sur l'hôte Linux. Il **ne couvre pas l'édition de
liens** et ne remplace pas `scripts/build-agent.sh`.

**Ne pas utiliser `git add -A`** : l'arbre est partagé entre tâches concurrentes,
et un `git add -A agent/src` a déjà emporté le travail d'une autre tâche dans un
commit qui ne compilait pas. Nommer les fichiers.

**Ne jamais tracer par image ni par message** dans le canal. Le chantier TURN
a perdu une session entière à une trace par paquet — 18 619 lignes en quelques
secondes sur un partage CIFS : la mesure détruisait ce qu'elle mesurait. Compter,
et journaliser périodiquement.

### Dette de taille de fichier

Relevé le 2 août 2026 par la commande de `CLAUDE.md`.

⚠️ **Les trois gros fichiers de dette ne sont PAS touchés par ce sous-bloc**, et
c'est une conséquence directe de la décision du §3.1 : le capteur instancie
`WindowsSource::sur_sortie` tel quel, donc `capture.rs`, `encode.rs` et
`windows_source.rs` ne sont ni déplacés ni modifiés. **Ce sous-bloc n'est donc
pas l'occasion de payer leur dette**, et il ne doit pas l'aggraver.

Les fichiers réellement modifiés, avec leur marge :

| Fichier | Lignes | Marge |
| --- | --- | --- |
| `agent/src/superviseur/boucle.rs` | 485 | **15** — gagne le lancement et la surveillance du capteur |
| `agent/src/demarrage.rs` | 468 | 32 |
| `agent/src/superviseur/lanceur.rs` | 255 | large |
| `agent/src/demarrage/source.rs` | 100 | large — arbitre entre source locale et distante |
| `agent/src/main.rs` | — | déclaration du module `capteur` et aiguillage du mode |

Rappel des marges nulles ou quasi nulles à ne pas frôler par accident :
`agent/src/encode/arret.rs` est à **500 lignes exactement**,
`agent/src/capture.rs` à **496** (marge 4), `agent/src/superviseur/table.rs` à
**489** (marge 11). Aucun fichier neuf du sous-bloc ne naît au-dessus de 500.

---

## 6. Ce que ce sous-bloc laissera ouvert

- **La couche qui impose le plafond de 4 processus reste inconnue** — Windows,
  DXGI, pilote NVIDIA, SudoVDA, virtualisation. D4 la contourne, il ne l'identifie
  pas. Un plafond dont la couche est inconnue peut se déplacer sous une autre
  charge.
- **La latence bout en bout reste non mesurée.** Le critère 3 relève la cadence,
  pas la latence capture → affichage navigateur, qui exige un instrument à
  construire.
- **Le mécanisme de l'abandon du mutex DXGI reste inexpliqué** : on sait le
  traiter depuis D2, pas le comprendre.
- **La mise en sommeil des fenêtres masquées reste une conjecture** : « créer 8 →
  en détruire 1 → tenter un 9ᵉ » n'a toujours pas été jouée, et que détruire un
  encodeur libère la place n'est établi par rien.
- **Le partage de la capacité réseau entre N flux** (D6) : chaque enfant garde sa
  propre `RTCPeerConnection` donc sa propre estimation de bande passante d'un lien
  qu'ils **partagent** — à N flux ils sur-souscrivent le chemin. Le chantier C
  volet 1 avait nommé cette dette ; elle reste entière.
- **L'audio reste porté par une seule fenêtre**, et `SendInput` reste global à la
  session Windows.
- **Le chemin d'extinction propre du superviseur n'a jamais été exercé** ; il
  gagne ici un processus de plus à éteindre.
- **Qu'une entrée abandonnée ne soit jamais reproposée** — défaut préexistant,
  nommé et non corrigé depuis D3.

---

## 7. Le plafond qui prend le relais

Ce sous-bloc ne supprime pas le plafond : il le **déplace sur une couche connue**,
et c'est le progrès qu'il faut lire.

Le capteur devient le seul processus tenant des duplications, donc le plafond de
4 processus cesse de mordre — le superviseur crée des sorties sans en dupliquer,
et D3 a **réfuté H2**, ce qui rend cette asymétrie mesurée bonne.

**Le plafond qui prend le relais est celui des encodeurs : 8 dans un processus,
la 9ᵉ refusée au `SetOutputType` de la MFT NVIDIA
(`MF_E_UNSUPPORTED_D3D_TYPE`, `0xC00D6D76`).** Mesuré deux fois — 30 puis
31 juillet 2026 — et **inchangé que les encodeurs partagent un périphérique D3D11
ou qu'ils en aient chacun un neuf**.

⚠️ **Portée de ce 8, à ne pas élargir** : la couche qui l'impose n'est pas
identifiée (NVENC, pilote, Media Foundation, ou virtualisation) ; rien ne dit
qu'il tienne à d'autres résolutions ou débits ; et la comparaison partagé/séparé
porte sur **deux variables confondues**, le mode séparé n'ouvrant aucune
duplication DXGI.

**Conséquence directe** : au-delà de huit fenêtres, la mise en sommeil des
fenêtres masquées passe d'optimisation souhaitable à **condition de viabilité**,
comme le cadrage l'annonçait. C'est le sous-bloc D5, et c'est pourquoi il vient
avant D6 et D7 dans la renumérotation du §1.2.
