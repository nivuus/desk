# Chantier D, sous-bloc D5 — le vivier d'encodeurs, et la mise en sommeil des fenêtres masquées

**Date** : 2 août 2026
**Chantier parent** : D (multi-fenêtres), `2026-07-28-support-jeux-design.md` §4 et §5 D
**Sous-bloc précédent** : D4, `2026-08-02-multifenetres-capture-mutualisee-design.md`
**Statut** : conception validée, plan à écrire

---

## 1. Pourquoi ce sous-bloc, et ce qu'il réunit

### 1.1 Ce que D4 laisse

D4 est fusionné (`f591845`) et ses trois critères sont tenus : huit fenêtres
diffusent simultanément depuis un capteur unique, tuer le capteur ne tue aucune
session, et la cadence est relevée des deux côtés du canal.

Il laisse **un seul défaut ouvert** (§13.2 et §16 de ses résultats) : à huit
fenêtres, `set_encode_size` est refusé **18 fois sur 18** au `SetOutputType` de
la MFT NVIDIA (`0xC00D6D76`, `MF_E_UNSUPPORTED_D3D_TYPE`), là où la même
opération réussit 3 fois sur 3 à deux fenêtres. **À la capacité maximale, la
seule réponse à la congestion qui subsiste est le débit, la résolution étant
gelée** — la moitié du dispositif du chantier C volet 1 disparaît, signalée par
un seul `WARN`.

Et il laisse le sous-bloc que la renumérotation de sa propre conception (§1.2)
désigne comme suivant : **D5 — mise en sommeil des fenêtres masquées**, promue
d'optimisation souhaitable à condition de viabilité au-delà de huit fenêtres.

### 1.2 Ces deux objets n'en font qu'un

Le défaut ouvert et la mise en sommeil reposent sur **la même conjecture, jamais
éprouvée depuis le 31 juillet 2026** : *détruire un encodeur libère-t-il la
place ?* La séquence « créer 8 → en détruire 1 → tenter un 9ᵉ » n'a jamais été
jouée par aucun chantier.

- Si elle est vraie, le défaut se remédie en détruisant avant de construire, et
  la mise en sommeil rend réellement des places.
- Si elle est fausse, ni l'un ni l'autre ne tient, et il faut le savoir avant
  d'écrire une ligne de vivier.

**D5 traite donc les deux comme un seul sujet — la gestion du vivier
d'encodeurs — et commence par la mesure qui les conditionne.**

### 1.3 La suite, inchangée

La renumérotation de D4 §1.2 reste valable : **D6** — partage de la capacité
réseau entre N flux, et audio par fenêtre ; **D7** — plein écran et Keyboard
Lock. Ni l'un ni l'autre n'est bloqué par le plafond, et ni l'un ni l'autre
n'entre dans D5.

---

## 2. État de départ — ce que le code fait aujourd'hui

**Le capteur tient tout, les enfants ne tiennent rien.** Un processus capteur
unique (`agent/src/capteur/`) sert N enfants par deux connexions au tube nommé
`\\.\pipe\agent-capteur`, une par sens. Chaque fenêtre y vit sur son propre fil
(`capteur/fenetre.rs`), qui possède un `WindowsSource` — le couple
`DesktopCapture` + `H264Encoder` — et n'en sort jamais, ces objets COM n'étant
pas `Sync`.

**Chaque `DesktopCapture` crée son propre périphérique D3D11**
(`agent/src/capture/ouverture.rs:89`). Dans le capteur de D4, les huit encodeurs
vivent donc sur **huit périphériques distincts, dans un seul processus**. Ce
n'est ni le montage `partage` ni le montage `separe` du banc du 31 juillet, et
c'est pourtant l'arrangement de production : toute mesure qui prétend l'éclairer
doit le reproduire.

**Le défaut de C2 se lit dans le code, pas seulement au journal.**
`WindowsSource::set_encode_size` (`agent/src/windows_source.rs:272-286`)
construit le nouvel encodeur **pendant que l'ancien vit encore** — l'affectation
`self.encoder = encoder;` ne relâche l'ancien qu'après. À huit encodeurs
vivants, ce neuvième transitoire est exactement le rang que le 31 juillet avait
relevé comme refusé.

**Le chemin de contrôle client → agent existe et est court.** Le navigateur émet
un `ClientControl` sur le data channel ; `transport/evenements.rs:195` le
mémorise ; `transport/tick.rs:128` agit. Une seule variante existe aujourd'hui,
`Resize`.

**Le chemin enfant → capteur existe aussi.** Le trait `Canal`
(`capteur/distante.rs`) porte `commander(VersCapteur) -> DepuisCapteur`, et le
capteur repousse un `DepuisCapteur::Etat { vivante, epuisee, largeur, hauteur }`
**au changement seulement**, jamais périodiquement.

**Rien, nulle part, ne connaît la visibilité d'une fenêtre.** Aucun message,
aucun champ, aucun signal côté client.

---

## 3. La mesure pivot, et la règle de décision écrite avant elle

### 3.1 Ce qu'elle éprouve

*Détruire un encodeur libère-t-il la place ?* — et, question que les trois
documents qui posent la première n'ont jamais posée, *le plafond de 8 porte-t-il
sur la concurrence ou sur les créations cumulées ?*

### 3.2 Montage

Un mode neuf du banc existant, `agent/src/diagnostics/multifenetre/nvenc.rs`,
dans **l'arrangement de production** : un processus, un périphérique D3D11 par
encodeur, 1280×720 à 60 Hz et 8 Mb/s — les paramètres exacts de la seconde
recette de D4.

### 3.3 Séquence

1. Construire 8 encodeurs. Tenter le 9ᵉ et **confirmer qu'il est refusé**, avec
   son `HRESULT` et l'appel qui refuse. Sans ce témoin, rien de ce qui suit ne
   prouve quoi que ce soit.
2. En détruire **un**, attendre son relâchement effectif, tenter un 9ᵉ.
3. **Répéter le cycle « détruire un, en construire un » au moins dix fois de
   suite**, en journalisant le rang de chaque tentative et son issue.

**Le point 3 est celui qui compte.** Un seul cycle ne distingue pas un plafond de
**concurrence** (8 vivants à la fois) d'un plafond de **créations cumulées** avec
du mou : le premier cycle passerait dans les deux cas, et seul le k-ième
révélerait la différence. Sans lui, D5 se bâtirait sur un chiffre qui lâche en
production après quelques minutes d'usage — et le vivier, qui recycle des
encodeurs par construction, est précisément ce qui déclencherait la panne.

**Au moins trois exécutions, et le nombre écrit dans le rapport.** La leçon que
ce dépôt a déjà payée en pleine campagne : *un défaut intermittent qu'on croit
déterministe se déclare corrigé à la première exécution qui passe.*

### 3.4 La règle de décision, posée AVANT la mesure

| Résultat | Conséquence |
| --- | --- |
| Le 9ᵉ réussit, et les dix cycles tiennent | **Cible « dépasser 8 » maintenue.** C2 se remédie en détruisant avant de construire |
| Le 9ᵉ réussit, mais les cycles lâchent au k-ième | **Plafond cumulé.** C1 est abandonné, repli sur « rendre 8 viable ». Le k relevé est reporté comme un défaut neuf, et il expliquerait rétroactivement le refus de D4 |
| Le 9ᵉ est refusé | **C1 est abandonné et rapporté non tenu**, sans être maquillé. `CAPACITE` reste à 8, le sommeil garde sa valeur (GPU et réseau économisés), et C2 se remédie par reconfiguration en place |

Cette table est le contrat du sous-bloc. Elle est écrite ici pour que le repli
soit un choix acté, et non un arbitrage improvisé sous le résultat — comme la
conception de D3 §3.5 l'avait fait avant sa propre campagne.

**Ce que la mesure pivot ne fera pas** : identifier la **couche** qui impose le
plafond de 8 (NVENC, pilote NVIDIA, Media Foundation, ou virtualisation).
Question ouverte depuis le 30 juillet 2026, elle le reste — D5 la contourne
comme D4 contournait le plafond de quatre processus.

---

## 4. Décisions actées

### 4.1 Le capteur arbitre

Le capteur **possède** la ressource arbitrée : il est le seul processus à voir
les huit encodeurs à la fois. L'arbitrage y vit donc, et aucun canal neuf n'est
créé.

Les deux alternatives sont écartées, et pourquoi :

- **le superviseur arbitre** — il tient la `Table` et la capacité, mais il n'a
  aucun canal vers ses enfants ni vers le capteur (D4 §2). Il faudrait en créer
  un pour arbitrer une ressource qu'il ne détient pas ;
- **chaque enfant décide seul** — sans vue d'ensemble, aucune éviction n'est
  possible : le neuvième qui demande à veiller échouerait et resterait figé.

### 4.2 Ce qu'une fenêtre endormie libère

**Son encodeur et sa duplication DXGI. Elle garde sa sortie virtuelle et la
géométrie de sa fenêtre Windows.**

Conséquence directe, et c'est ce qui rend le sommeil bon marché : **rien n'est
créé ni détruit côté pilote, donc aucune session tierce ne perd son mutex**
quand une fenêtre s'endort ou se réveille. Les 28 pertes d'accès `0x887a0026`
relevées sur la montée à huit de D4 venaient toutes de créations de sortie ; le
sommeil n'en ajoute aucune.

Conséquence acceptée : **le plafond de fenêtres ouvertes est celui du vivier de
sorties virtuelles, soit 10** (mesure ① du 31 juillet, refus à la 11ᵉ en
`ERROR_TOO_MANY_NAMES`). Dépasser 10 exigerait de rendre les sorties au pilote,
donc d'infliger un abandon de mutex à toutes les voisines à chaque
endormissement. **Ce n'est pas le marché de D5.**

### 4.3 L'arbitrage est un LRU, et il a besoin de deux signaux

**La fenêtre la moins récemment vue s'endort**, même si elle est encore à
l'écran. La main de l'utilisateur décide implicitement, sans qu'il ait rien à
régler.

La visibilité seule ne suffit pas à ordonner ce LRU : le cas de C1 est *dix
fenêtres toutes visibles*, qui ont alors exactement la même visibilité. Le
message porte donc **deux** signaux — `visible` et `focused` — et `dernier_vu`
est le dernier instant où la fenêtre a été focalisée **ou** est redevenue
visible.

**Une hystérésis borne le battement** : une fenêtre réveillée ne peut être
évincée avant un temps minimal d'éveil. Valeur de départ **2 s**, **explicitement
non calibrée** ; la recette relève le nombre d'endormissements sur C1 et c'est ce
chiffre, pas une intuition, qui la jugera.

### 4.4 Les pièces

```
navigateur          enfant                    capteur
─────────           ──────                    ───────
visibilitychange
focus / blur
  └─ ClientControl::Visibility ─► evenements.rs (mémorise)
                                    └─ tick.rs ─► VideoSource::set_awake
                                                    └─ SourceDistante::commander
                                                        └─ VersCapteur::Visibilite ─►
                                                                                serveur.rs
                                                                                  └─ Vivier (LRU)
                                                                                      └─ fil de fenêtre
                                                                                          Dormir → drop(WindowsSource)
                                                                                          Réveil → sur_sortie + image clé
                                                              ◄── DepuisCapteur::Etat { endormie }
  ◄── AgentControl::Asleep { asleep, raison } ──┘
```

- **`client/src/visibilite.ts`** — écoute `visibilitychange`, `focus` et `blur`,
  émet sur le data channel.
- **`proto/`** — `ClientControl::Visibility { v, visible, focused }` et
  `AgentControl::Asleep { v, asleep, raison }`. Deux raisons distinctes, parce
  qu'elles ne se valent pas pour l'utilisateur : `masquee` (il l'a voulu) et
  `evincee` (le vivier le lui a pris alors qu'il regardait).
- **`VideoSource::set_awake`** — méthode de trait avec implémentation par défaut
  inerte, exactement comme `set_encode_size`. `LoopSource` et les tests
  existants ne bougent pas.
- **`agent/src/capteur/vivier.rs`** *(neuf, et c'est le cœur)* — tient par
  session `{ visible, focused, dernier_vu, eveillee, eveillee_depuis }` et une
  capacité ; rend la liste des transitions à appliquer. **Aucun `#[cfg(windows)]`,
  aucun objet COM** : c'est du LRU pur, testable sur l'hôte, comme
  `capteur/protocole.rs` et `capteur/distante.rs` l'ont été en D4. **La pièce la
  plus coûteuse à se tromper est ainsi la seule qui soit entièrement couverte par
  des tests.**
- **`agent/src/capteur/fenetre.rs`** — `source: WindowsSource` devient
  `Option<WindowsSource>`. Endormir, c'est relâcher ; réveiller, c'est
  reconstruire par le même `sur_sortie` que l'ouverture, suivi d'une image clé.
- **`agent/src/windows_source.rs`** — `set_encode_size` détruit avant de
  construire, **si et seulement si** la mesure pivot l'autorise (§3.4).

### 4.5 Ce que le sommeil ne touche pas

Trois choses, dites ici pour qu'on ne les découvre pas à la recette :

1. **L'audio.** Une seule fenêtre le porte (relevé de D1) ; l'endormir couperait
   le son de toute la session. **Le sommeil est vidéo, et rien d'autre.**
2. **L'entrée.** Une fenêtre endormie reste injectable — sans effet en pratique,
   et le garder évite un cas de bord au réveil.
3. **La sortie virtuelle et la géométrie de la fenêtre Windows** (§4.2).

### 4.6 Ce que voit l'utilisateur

Une fenêtre endormie **conserve sa dernière image** : l'élément `<video>` retient
la dernière trame décodée, sans qu'aucun code ne l'y aide. Le bandeau de statut
(`client/src/status.ts`) annonce l'état et sa raison. Un retour de focus la
réveille.

### 4.7 `CAPACITE` passe de 8 à 10

`agent/src/superviseur/boucle.rs:58`. Un `PLAFOND_EVEIL` de 8 le complète, côté
capteur. **Sous réserve de la règle du §3.4** : si la mesure pivot réfute la
conjecture, `CAPACITE` reste à 8 et cette section tombe avec C1. **Les deux
valeurs sont des relevés de cette VM, pas des bornes du système** — la première vient de la mesure ① du 31 juillet, la seconde des
mesures des 30 et 31 juillet, toutes deux confirmées par aucune identification
de la couche qui les impose.

---

## 5. Critère de réception

Trois critères, aucun ne recouvrant les autres, chacun réfutable.

### C1 — dépasser huit fenêtres

**10 fenêtres ouvertes simultanément, toutes visibles.** Relevé attendu : 8 avec
`framesDecoded` croissant, 2 stables. Puis l'utilisateur focalise une endormie :
elle diffuse, et la moins récemment vue des huit se fige.

**Le refus au rang 11 doit venir du pilote** (`ERROR_TOO_MANY_NAMES`), pas d'une
constante du produit. C'est ce que D4 n'avait pas pu montrer : sa montée
s'arrêtait sur `CAPACITE = 8`, une constante du produit, sans approcher aucun
plafond du système.

**Les deux raisons de sommeil doivent être exercées, pas une seule.** La
séquence ci-dessus exerce `evincee`. Une minimisation d'une fenêtre éveillée
exerce `masquee` — et doit réveiller une endormie, le vivier ayant regagné une
place. Sans ce second geste, la moitié du §4.4 resterait du code jamais couru.

### C2 — le défaut de D4 est mort

**À 8 fenêtres éveillées, `set_encode_size` réussit.** D4 relevait 18 refus sur
18 ; le relevé opposable est le même journal, le même `WARN`, sur la même montée.

### C3 — le réveil est borné

**Délai entre le retour de focus et la première image décodée par le
navigateur**, relevé des deux côtés — l'agent et `framesDecoded` — sur les deux
réveils de C1 au moins. **Mesuré et rapporté, pas conjecturé.** Aucun seuil n'est
posé : rien dans le dépôt ne permettrait de le calibrer, et un seuil non calibré
ferait échouer un sous-bloc sur un nombre arbitraire.

### Protocole de recette — quatre contraintes héritées

1. **La source doit bouger.** Desktop Duplication n'émet une trame qu'au
   changement du bureau ; une fenêtre immobile ne produit aucune image. La
   seconde recette de D4 a établi le montage : une fenêtre Chrome `--app` sur
   une page `canvas` animée, **un `--user-data-dir` par fenêtre**, faute de quoi
   Chrome rejoint son instance existante et l'on compte des lancements au lieu
   de fenêtres.
2. **Le sommeil se pilote par la minimisation.** Sur Chrome/Linux,
   `visibilityState` ne rapporte **pas** l'occultation par une autre fenêtre,
   seulement la minimisation et l'onglet caché. C'est un geste réel de
   l'utilisateur, et le seul que le navigateur rapporte de façon portable.
   L'éviction, elle, s'exerce par le focus entre dix fenêtres toutes visibles.
3. **Aucune capture d'écran CDP pendant une mesure**, et **toute évaluation CDP
   bornée** : une page portant un flux WebRTC actif peut ne jamais rendre.
4. **Contrôler la survie de la VM après chaque rang**, et copier `agent.log`
   après la fin réelle de l'exécution, pas à la fin du pilote.

---

## 6. Risques et pièges connus

1. **La conjecture du pivot.** Couverte par la règle de décision du §3.4, et par
   rien d'autre. C'est le risque n°1 du sous-bloc.
2. **Le battement.** Dix fenêtres visibles et un utilisateur qui passe de l'une à
   l'autre déclencheraient des endormissements et réveils en rafale, chacun
   reconstruisant une duplication DXGI et un encodeur. Traité par l'hystérésis du
   §4.3 — **risque neuf, propre à D5**, et qui se traite dans la partie pure,
   donc testable sur l'hôte.
3. **Réveil et reprise se croisent.** Réveiller rouvre une duplication ; si le
   superviseur crée une sortie au même instant, le mutex est abandonné. **Le
   réveil doit emprunter le chemin de reprise de D2** (`capteur/reprise.rs`,
   48 à 144 ms relevés), pas un chemin parallèle qui aurait à réapprendre la
   même leçon.
4. **`agent/src/capteur/serveur.rs` est à 490 lignes, marge 10**, et il est sur
   le chemin. **Extraction, jamais compression** — le dépôt a payé cette leçon
   deux fois, et `encode/arret.rs` est à 500 pour l'avoir jouée une fois de trop.
   Le vivier naît dans son propre fichier, ce que la règle imposerait de toute
   façon.
5. **`agent/src/windows_source.rs` est à 648 lignes, dette gelée**, et le remède
   de C2 y atterrit. Quelques lignes seulement, mais le plan doit recompter et
   extraire si l'addition devient substantielle.
6. **Détruire avant de construire n'est pas gratuit.** Si la construction du
   nouvel encodeur échoue après la destruction de l'ancien, la session perd sa
   vidéo au lieu de garder son barreau. Le remède de C2 doit donc rendre une
   erreur qui **épuise proprement** la source, et non paniquer : une panique
   traverserait `spawn_blocking` et emporterait le processus — le commentaire de
   `set_encode_size` le dit déjà pour un autre cas.
7. **La VM s'hiberne toute seule**, déclencheur non identifié. Vérifier
   `virsh list --all` après toute séquence longue.
8. **`scripts/run-agent.sh` ne transmet pas les variables neuves.** Piège payé en
   D1 (`SUPERVISEUR`), en D2 (`MULTIFENETRE_REPRISE`) et évité en D3. Toute
   variable neuve du banc pivot doit y être ajoutée **dans la même tâche** que le
   mode.
9. **`cargo check --target x86_64-pc-windows-gnu`** vérifie types, emprunts,
   visibilités et durées de vie du code `#[cfg(windows)]` sur l'hôte. **À lancer
   avant toute compilation distante**, et ce n'est pas un substitut à
   `scripts/build-agent.sh` : l'édition de liens n'est pas couverte.
10. **Un `rsync -a` qui remonte le temps fait qu'un `cargo build` ne bâtit rien
    et le dit comme un succès.** Vérifier la **taille** du binaire après tout
    aller-retour de sources ; une compilation de 0,13 s est un aveu.

---

## 7. Ce que ce sous-bloc laissera ouvert

À écrire dans les résultats, pour qu'on ne le croie pas fait :

- **La couche qui impose le plafond de 8** — inconnue depuis le 30 juillet 2026.
  D5 la contourne, il ne l'explique pas.
- **Le mécanisme de l'abandon du mutex DXGI** — inexpliqué depuis D1. On sait le
  traiter, pas le dire.
- **La couche qui impose le plafond de 4 processus** — inconnue depuis D3.
- **La latence de bout en bout** — jamais mesurée par aucun sous-bloc du
  chantier D. C3 mesure un délai de réveil, ce qui n'est pas la même chose.
- **L'occultation par recouvrement** — non détectée sur Chrome/Linux, donc jamais
  exercée comme déclencheur de sommeil.
- **D6** : partage de la capacité réseau entre N flux, et audio par fenêtre. En
  particulier, une fenêtre endormie continue de porter son propre contrôleur de
  congestion, que D5 ne touche pas.
- **D7** : plein écran et Keyboard Lock.
- **Le chemin d'extinction propre du superviseur** — toujours jamais exercé.
- **La mort d'un enfant pendant que les autres diffusent**, et la fermeture d'une
  fenêtre en cours de diffusion — toujours jamais exercées.

---

## 8. Dette de taille de fichier

Relevé **par la commande**, le 2 août 2026, et non recopié :

```bash
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
```

Trois fichiers dépassent 500 lignes, et ce sont les trois du tableau de dette
gelée : `agent/src/encode.rs` (1536), `agent/src/windows_source.rs` (648),
`agent/src/wasapi.rs` (543). **Aucun autre.**

⚠️ **Une correction que ce sous-bloc doit porter à `CLAUDE.md`** : le fichier
annonce `agent/src/capteur/distante.rs` **à 487 lignes, marge 13**, et en fait
une « marge étroite neuve à surveiller ». Le relevé autoritaire dit **206**.
C'est exactement la dérive contre laquelle l'encadré de `CLAUDE.md` prévient, et
la consigne du même encadré s'applique : **corriger le tableau dans le même
mouvement**. À faire dans ce sous-bloc.

Marges étroites réellement sur le chemin de D5 :

| Fichier | Lignes | Marge | Sur le chemin ? |
| --- | --- | --- | --- |
| `agent/src/capteur/serveur.rs` | 490 | 10 | **oui** — câblage du vivier |
| `agent/src/superviseur/boucle.rs` | 491 | 9 | oui, mais une constante seulement |
| `agent/src/windows_source.rs` | 648 | gelée | **oui** — remède de C2 |
| `agent/src/capteur/fenetre.rs` | 329 | 171 | oui, confortable |
| `agent/src/transport/tick.rs` | 391 | 109 | oui, confortable |
| `proto/src/control.rs` | 330 | 170 | oui, confortable |

Les marges hors chemin (`encode/arret.rs` à 500, `capture.rs` à 496,
`superviseur/table.rs` à 489) ne sont pas touchées par ce sous-bloc et gardent
leurs encadrés existants.
