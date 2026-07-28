# Recette du jalon 1 — tranche verticale

**Date** : 28 juillet 2026
**Statut** : recette conduite, résultats consignés tels que mesurés

Ce document consigne, pour chacun des cinq critères d'acceptation du §1 de
la spec (`docs/superpowers/specs/2026-07-27-jalon1-tranche-verticale-design.md`),
la valeur mesurée, le verdict, et — pour tout critère non tenu — l'étage
responsable identifié à l'aide du budget de latence du §8 de la spec :
capture ≤ 5 ms, encodage ≤ 10 ms, réseau LAN ≤ 5 ms, tampon de gigue +
décodage ≤ 20 ms (total 40 ms, marge de 10 ms sur la cible de 50 ms).

L'esprit de cette recette n'est pas de faire réussir le jalon : trois des
cinq critères ne sont **pas** tenus, et c'est écrit tel quel ci-dessous.

## Livrable : l'overlay de mesure

`client/src/stats.ts` (nouveau), branché dans `client/src/main.ts` sur
`session.pc` juste après connexion, affiché dans un `<div id="stats">`
(`client/index.html`, `client/src/style.css`). Toutes les valeurs
proviennent de `RTCPeerConnection.getStats()` — ce sont celles calculées par
le navigateur lui-même, pas une estimation du code du produit.

Capture d'écran vérifiée en direct (chaîne complète, agent réel sur la VM,
Firefox sur `anim.html`) :

```
30.0 i/s  ·  784×592  ·  1.34 Mb/s  ·  RTT 4.0 ms  ·  tampon 39.5 ms  ·  ≈ 41.5 ms  ·  perdues 0
```

L'image (fenêtre Firefox complète avec son chrome, bandeau de notification
Firefox inclus, overlay lisible en bas à gauche) confirme au passage le
critère 1 sans ambiguïté.

## Tableau des cinq critères

| # | Critère | Mesure | Cible | Verdict | Étage responsable si non atteint |
|---|---|---|---|---|---|
| 1 | Firefox capturé par fenêtre | 784×632 (tâche 11) et 784×592 / 764×484 (recette, selon la taille de fenêtre du navigateur) — jamais 2400×1080 (bureau) | dimensions fenêtre, pas bureau | **Atteint** | — |
| 2 | Défilement à 60 i/s | ~29,5–30 i/s (moyenne de 6 mesures indépendantes convergentes, voir détail) | ≥ 55 i/s | **Non atteint** | Encodage |
| 3 | Latence bout en bout | approximation overlay : 5–57 ms · mesure directe (12 essais) : min 54,1 ms, médiane 275,95 ms, max 1516,8 ms | < 50 ms | **Non atteint** | Encodage (hypothèse plausible, non confirmée pour la traîne — voir texte) + tampon de gigue (aggravant, confirmé) |
| 4 | Souris, clavier, molette | clic → navigation, Origine+Suppr → bon caractère supprimé, F5 → rechargement, molette → défilement bidirectionnel (tâche 12) ; clavier reconfirmé en direct aujourd'hui | tous réussis | **Atteint** (avec une réserve, voir texte) | — |
| 5 | Redimensionnement | 32 mesures connues : min 365 ms, max 534 ms, 27/32 (84 %) sous 500 ms | < 500 ms | **Non atteint de façon fiable** | Encodage (redémarrage à froid) |

Détail de chaque critère ci-dessous.

---

## Critère 1 — Firefox capturé par fenêtre : ATTEINT

Repris de la tâche 11, vérifié par échantillonnage aujourd'hui (screenshot
ci-dessus, 784×592 pour une fenêtre client 800×600, et 764×484 pour une
fenêtre 780×493 demandée par redimensionnement automatique). Le recadrage
suit systématiquement la taille de la **fenêtre**, jamais celle du bureau
(2400×1080) : le pipeline Desktop Duplication + `CopySubresourceRegion`
(tâche 9) traverse correctement toute la chaîne jusqu'au navigateur.

## Critère 2 — Défilement à 60 i/s : NON ATTEINT

**Valeur retenue : ~29,5–30 i/s**, à peine la moitié de la cible (≥ 55 i/s).

Ce chiffre n'est pas une mesure isolée mais la convergence de six mesures
indépendantes, prises à des tâches différentes, avec des contenus
différents (animation CSS, molette réelle, défilement clavier) :

| Source | Contenu | Résultat |
|---|---|---|
| Tâche 11 | animation CSS | ~23–25 i/s |
| Tâche 12 | molette réelle, rafale dense | 22,9 i/s |
| Tâche 12 | molette réelle, rafale espacée | 9,7 i/s |
| `remesure-debit.md` | animation CSS, 3 essais (10/15/20 s) | 29,5 / 29,67 / 29,75 i/s |
| `diagnostic-plafond-debit.md` | molette réelle | 29,70 i/s |
| Recette (aujourd'hui) | animation CSS, overlay en direct | **30,0 i/s** |

Les deux mesures à 9,7 et 22,9 i/s (tâche 12) ne sont **pas** incluses dans
la valeur retenue (~29,5–30 i/s) : ce sont des essais à cadence de
stimulus délibérément différente (rafale de molette « espacée » contre
« dense »), cités ici pour montrer que le plafond apparaît quelle que soit
l'intensité de la sollicitation, pas pour être moyennés avec les quatre
autres mesures, qui elles isolent le plafond propre du pipeline (contenu
changeant en continu, sans sous-échantillonnage volontaire du stimulus). Le
lecteur pressé qui ne retiendrait que la ligne du haut du tableau pourrait
sinon y voir une sélection favorable — ce n'en est pas une : les six chiffres
sont rapportés tels quels, seuls les quatre derniers mesurent la même chose.

Le débit **n'est pas** limité par la capture (~90 i/s isolée, `CAPTURE_TEST`),
ni par le réseau (`packetsLost=0` dans tous les essais), ni par la boucle de
transport (`Session::run` tourne à 60,0 Hz exact, mesuré). Il est limité par
l'**encodeur matériel** : `H264Encoder::submit`, sollicité à la cadence fixe
de 16,7 ms de `Session::run`, ne reçoit de nouvelles demandes d'entrée
(`METransformNeedInput`) qu'à ~30 Hz — alors que le même encodeur, sur la
même machine, soutient ~80 i/s en boucle serrée (`ENCODE_TEST`). Ce n'est
donc **pas** une limite du GPU/pilote NVIDIA, mais une interaction non
résolue entre le rythme de soumission fixe et le rythme propre du MFT
matériel (voir `diagnostic-plafond-debit.md` et `remesure-debit.md` pour le
détail des essais qui écartent successivement les hypothèses concurrentes —
`SUBMIT_POLL_BUDGET`, composition GPU/duplication du bureau).

**Étage responsable (budget de latence)** : encodage. Deux correctifs
tentés (réessai de capture borné, drainage plus agressif de `poll_output`)
n'ont produit aucun gain et un a provoqué une régression ; ils ont été
intégralement annulés. La cause exacte côté pilote NVENC/Media Foundation
reste à isoler — piste retenue non explorée : comparer le rythme de
`METransformNeedInput` sous soumission à cadence fixe (16,7 ms, régulière)
contre soumission en rafale (comme `ENCODE_TEST`), à débit moyen identique,
pour savoir si c'est la régularité de l'espacement elle-même qui bride le
MFT.

## Critère 3 — Latence bout en bout : NON ATTEINT

C'est le critère qui n'avait jamais été mesuré avant cette tâche. Deux
méthodes ont été utilisées, avec des portées différentes — voir la section
dédiée « Méthodologie de la latence » plus bas pour le détail et les limites
de chacune.

**Approximation de l'overlay (RTT/2 + tampon de gigue)** : observée entre
5 ms et 57 ms selon les essais — semble tenir la cible, mais c'est trompeur
(voir plus bas pourquoi).

**Mesure directe touche→photon** (12 essais indépendants, chaîne complète,
horloge unique) :

```
valeurs (ms) : 54,1 · 60,5 · 70,2 · 102,4 · 218,4 · 260,0 · 291,9 · 294,4 · 580,9 · 829,2 · 1358,6 · 1516,8
min = 54,1 ms · max = 1516,8 ms · moyenne = 469,8 ms · médiane = 275,95 ms
0/12 sous la cible de 50 ms (le MEILLEUR essai, 54,1 ms, la dépasse déjà légèrement)
```

(Médiane corrigée d'une erreur de calcul relevée en revue : sur douze
valeurs triées, la médiane est la moyenne des deux valeurs centrales
— (260,0 + 291,9) / 2 = 275,95 — pas l'élément d'indice `longueur/2`, qui
donne le 7ᵉ élément et non le milieu. L'écart avec le chiffre initialement
publié, 291,9, est de 15,95 ms, soit 5,8 % de la valeur corrigée ; il ne
change aucun verdict.)

**Verdict : non atteint, et de loin** — y compris sur le meilleur des 12
essais. La variance est elle-même un résultat important : un facteur 28
entre le meilleur et le pire essai n'est pas du bruit de mesure ordinaire.

**Essai complémentaire, avec télémétrie de gel corrélée** : un second relevé
de 12 essais a été conduit avec le harnais versionné
(`client/recette/harness.mjs`), en échantillonnant `freezeCount` et
`totalFreezesDuration` (`getStats()`, disponibles sur la piste vidéo)
immédiatement avant et après chaque essai, pour vérifier plutôt que
supposer l'origine de la traîne :

```
valeurs (ms) : 83,0 · 89,1 · 134,6 · 241,1 · 285,3 · 321,7 · 340,6 · 351,0 · 641,8 · 877,6 · 1164,0 · 1449,9
min = 83,0 ms · max = 1449,9 ms · moyenne = 498,3 ms · médiane = 331,15 ms
freezeCountDelta = 0 et totalFreezesDurationDelta = 0,0 ms sur les 12 essais,
  y compris les quatre plus lents (641,8 à 1449,9 ms)
```

**Étage responsable — reformulé en hypothèse, pas en fait établi** :
l'explication initialement avancée dans une version antérieure de ce
document (« c'est la signature du goulot d'encodeur du critère 2 ») allait
au-delà de ce que les données établissaient. Le goulot d'encodeur mesuré au
critère 2 (~30 Hz au lieu de 60 Hz) explique mécaniquement une attente
d'environ un cycle manqué, soit ~33 ms — pas un facteur allant jusqu'à 28.
Le second relevé, ci-dessus, **infirme partiellement** l'hypothèse d'un arrêt
net de capture/encodage comme cause des essais les plus lents : si la
capture ou l'encodage s'étaient réellement arrêtés pendant 600 ms à 1,45 s,
le détecteur de gel du navigateur (qui compare l'écart entre deux images
consécutives à l'écart attendu) se serait presque certainement déclenché —
il ne s'est déclenché sur **aucun** des douze essais, y compris les plus
lents. Le document liste par ailleurs deux autres causes actives pendant
cette recette et non exclues (instabilité de la VM, dérive du focus —
voir « Ce qui a été appris ») : cet essai ne permet pas non plus de les
écarter.

Piste alternative, cohérente avec un motif observé dans le second relevé
sans être démontrée : les essais 3 à 6 forment une **suite quasi monotone
croissante** (641,8 → 877,6 → 1164,0 → 1449,9 ms) suivie d'une chute nette à
83,0 ms à l'essai suivant — une signature plus proche d'un **tampon de
gigue adaptatif qui dérive puis se resynchronise** que d'un événement isolé
répété. Cohérent avec l'observation indépendante que le tampon de gigue
affiché par l'overlay (35–55 ms en régime établi) est déjà proche de son
propre sous-budget de 20 ms : un tampon dont la cible dérive occasionnellement
vers le haut expliquerait les deux symptômes à la fois. Cette hypothèse
n'est pas tranchée par les données de cette recette — elle demanderait une
mesure dédiée de `jitterBufferDelay`/`jitterBufferTarget` échantillonnée en
continu, pas seulement avant/après chaque essai.

**Facteur aggravant confirmé indépendamment** : le **tampon de gigue**
lui-même tourne autour de 35–55 ms dans les essais à 30 i/s (colonne
« tampon » de l'overlay) — à lui seul, il dépasse déjà le sous-budget de
20 ms alloué à « tampon de gigue + décodage » combinés. Cause probable :
`playoutDelayHint`/`jitterBufferTarget` (mentionnés au §3.3 de la spec comme
optimisation prévue) ne sont **jamais positionnés** côté client — vérifié
par recherche dans `client/src/*.ts` : aucune occurrence. C'est un levier
concret et non exploité pour la suite — **mais avec une mise en garde** : le
rythme d'encodage documenté au critère 2 est lui-même irrégulier (~30 Hz,
pas un métronome parfait). Réduire agressivement `jitterBufferTarget` sans
d'abord fiabiliser ce rythme risque de convertir de la latence en gels
visibles (le tampon absorbe aujourd'hui une partie de cette irrégularité) —
un compromis, pas un gain net garanti. Le prochain chantier qui touche ce
réglage devrait mesurer `freezeCount`/`totalFreezesDuration` en fonction de
plusieurs valeurs de `jitterBufferTarget`, plutôt que de minimiser la cible
à l'aveugle.

## Critère 4 — Souris, clavier, molette : ATTEINT (avec une réserve)

Repris de la tâche 12, dont la preuve reste la plus solide dont on dispose :
effet réel observé sur le flux vidéo décodé d'un vrai navigateur (Chrome
piloté par CDP), pas une auto-évaluation du code testé — clic déclenchant
une navigation, `Origine`+`Suppr` effaçant le bon caractère (distinguant la
touche étendue du pavé numérique), `F5` rechargeant la page, molette faisant
défiler dans les deux sens (`SCROLL:0 → 3264 → 1632`), captures d'écran à
l'appui (`task-12-assets/proof-*.png`).

**Reconfirmé en direct aujourd'hui, pour le clavier** : 12/12 essais de la
mesure de latence (critère 3) reposent sur un vrai aller-retour
touche→SendInput→Firefox→capture→réseau→décodage, exécuté à l'instant de
cette recette — preuve fraîche, pas seulement héritée.

**Réserve, à consigner honnêtement** : la molette, elle, n'a **pas** pu être
reproduite aujourd'hui avec le harnais de recette (`WheelEvent` synthétique
dispatché sur l'élément vidéo, curseur repositionné au préalable) — zéro
image nouvelle sur 8 à 10 s d'envoi continu (134 à 166 messages envoyés,
confirmés par instrumentation de `RTCDataChannel.send`, aucune erreur de
décodage côté agent), alors que le clavier fonctionnait de façon fiable
dans les mêmes conditions de session. Voir « Ce qui a été appris » pour le
détail de l'investigation et pourquoi elle ne remet pas en cause la preuve
de la tâche 12 (harnais différent, captures d'écran directes plutôt que
compteurs `getStats()`). Le critère reste jugé **atteint** sur la base de la
preuve la plus solide (tâche 12), mais la molette mériterait d'être
reproduite avec le harnais de cette recette avant de s'appuyer dessus pour
un chantier ultérieur.

## Critère 5 — Redimensionnement < 500 ms : NON ATTEINT DE FAÇON FIABLE

Repris tel quel de la tâche 13, chiffres non refaits (conformément à la
consigne) :

```
32 mesures connues (22 de l'implémenteur + 10 du relecteur, deux séries de
régimes différents : 22/22 puis 5/10) : min = 365 ms, max = 534 ms,
moyenne combinée cohérente autour de 430-450 ms
27/32 (84 %) sous 500 ms
```

**Étage responsable** : encodage — le redimensionnement reconstruit toute
la chaîne d'encodage (nouvelle capture DXGI, nouveau convertisseur, nouvel
encodeur H.264), et c'est ce redémarrage à froid qui domine le délai. Fait
directement lié, à ne pas laisser de côté : **environ un redimensionnement
sur trois à cinq produit un gel visible de 0,3 à 0,5 s** (mesuré via
`freezeCount`/`totalFreezesDuration`, invisible si l'on ne regarde que
`framesDecoded`, qui reste croissant). Cause probable : le temps que met le
nouvel encodeur à produire sa première image après reconstruction — pas une
anomalie d'horodatage.

---

## Méthodologie de la latence — ce que chaque méthode mesure et omet

**Méthode 1 — approximation de l'overlay** (`RTT/2 + tampon de gigue`,
implémentée dans `stats.ts` conformément au brief) :

- Ce qu'elle mesure réellement : la moitié de l'aller-retour ICE (donc un
  ordre de grandeur du trajet réseau aller simple, en supposant un chemin
  symétrique) plus le temps que le navigateur retient une image dans son
  tampon de gigue avant de la présenter.
- Ce qu'elle omet entièrement : le temps de capture côté agent, le temps de
  conversion BGRA→NV12 et d'encodage matériel, le temps d'attente avant
  qu'une soumission soit acceptée par l'encodeur (précisément le goulot des
  critères 2 et 3), et le temps de décodage matériel côté navigateur (le
  tampon de gigue et le décodage sont fusionnés en une seule valeur par
  `getStats()`, mais la part décodage elle-même n'est pas isolée).
- Verdict sur la méthode : **optimiste par construction**, puisqu'elle
  ignore justement l'étage qui s'avère être le goulot. C'est un signal utile
  au jour le jour (variations relatives, dégradation réseau), pas une mesure
  de la latence glass-to-glass réelle.

**Méthode 2 — touche→photon, horloge unique** (nouvelle, écrite pour cette
recette, harnais versionné dans `client/recette/` — voir « Instrument versionné »
ci-dessous) : un script piloté par CDP (Chrome sans interface) dispatche un
`KeyboardEvent` synthétique (`code: 'Space'`) sur `window` — exactement
l'événement que `client/src/input.ts` écoute réellement — et chronomètre,
dans la **même horloge JavaScript** (`performance.now()`), le moment où un
motif visuel plein écran (bascule noir/blanc sur `client/recette/latency-test.html`,
servie localement puis ouverte dans Firefox côté VM) change de couleur dans
l'élément `<video>` décodé. La détection utilise
`video.requestVideoFrameCallback`, qui fournit `metadata.expectedDisplayTime`
— le vsync par lequel le navigateur *s'attend* à ce que l'image soit
affichée, c'est-à-dire l'estimation la plus proche du moment d'affichage
réel. Point corrigé en revue : `metadata.presentationTime`, utilisé par
erreur dans une version antérieure de ce document et du harnais, mesure
autre chose — le moment où le navigateur a **soumis** l'image au
compositeur, un cycle d'affichage plus tôt. L'écart entre les deux est de
l'ordre d'un cycle d'affichage (quelques millisecondes), négligeable face
aux latences mesurées ici (54 ms à 1,5 s), et s'il joue, c'est dans le sens
où la latence réelle est très légèrement supérieure à ce qu'aurait rapporté
`presentationTime` — aucun verdict ne change. Le harnais utilise désormais
`expectedDisplayTime` (avec repli sur `presentationTime` si absent d'une
implémentation donnée) ; les chiffres du relevé complémentaire ci-dessus
(avec télémétrie de gel) ont été mesurés avec la version corrigée.

- Ce qu'elle mesure : l'aller-retour complet réellement vécu par
  l'utilisateur pour un stimulus donné — envoi sur le canal de données →
  `SendInput` côté Windows → traitement par Firefox → capture → conversion →
  encodage → réseau → tampon de gigue → décodage → rendu. Aucun problème de
  synchronisation d'horloges entre deux machines : la mesure démarre et se
  termine dans le même processus Chrome.
- Ce qu'elle ajoute par rapport à une latence glass-to-glass « pure » (qui
  ne mesurerait que capture→affichage, sans l'aller-retour d'entrée) : le
  temps d'acheminement du message sur le canal `input` (non fiable, mais en
  LAN typiquement inférieur à 1 ms), le temps de traitement `SendInput` côté
  Windows (sub-milliseconde), et le temps que met Firefox à réagir à
  l'événement clavier et à repeindre (généralement inférieur à une image).
  Ce surcoût est réel mais faible comparé aux écarts mesurés (54 ms à
  1,5 s) — l'essentiel de la mesure reflète bien la chaîne vidéo.
- Limite reconnue : un seul point de mesure par essai (le centre de
  l'image), et le seuil de détection de changement de couleur (somme des
  écarts RVB > 150) introduit un arrondi à la granularité d'une image
  décodée (~33 ms à 30 i/s) — négligeable face à la variance observée.

**Pourquoi la méthode 2 est retenue comme la mesure de référence** : elle ne
suppose rien sur la symétrie du réseau, elle traverse réellement tous les
étages (y compris ceux qu'un budget théorique doit couvrir), et elle a été
vérifiée reproductible (deux relevés de 12 essais indépendants chacun, même
chaîne, même Firefox/agent, à quelques heures d'écart). Le brief demandait
explicitement une mesure plus honnête si elle était à portée : celle-ci
l'est, et elle est nettement moins flatteuse que l'approximation.

### Instrument versionné

Le harnais est commité (une version antérieure ne vivait que dans un
répertoire temporaire de session — corrigé en revue, avant que le fichier
ne disparaisse et que seule la prose ne subsiste) :

- `client/recette/harness.mjs` — les deux modes (`stats`, `latency`),
  usage documenté en tête de fichier.
- `client/recette/latency-test.html` — la page de test pilotée par le mode
  `latency` (bascule noir/blanc sur Espace).
- `client/recette/scroll-test.html` — la page de test pilotée par le mode
  `stats` (bandes de couleur, défilement).

Reproduction : chaîne complète montée (signaling, client Vite, agent via
`scripts/run-agent.sh`), la page de test correspondante servie depuis cette
machine (`python3 -m http.server 8099` dans `client/recette/`, ou pointer
directement sur le fichier) et ouverte dans Firefox côté VM, puis
`node client/recette/harness.mjs latency http://127.0.0.1:5173/?session=<id> 12`.
Voir « Ce qui a été appris » ci-dessous pour le piège du focus à respecter
avant de lancer une mesure.

---

## Ce qui a été appris

Cette section consigne les écarts d'API et les pièges de mesure qui ne
seraient pas ailleurs autrement — le brief demande explicitement de ne pas
les laisser disparaître.

### Écarts d'API rencontrés aux tâches 7, 9 et 10

**Tâche 7 (str0m 0.21, transport WebRTC)** : quasiment aucun écart réel.
Sept points d'incertitude signalés par le brief ont été vérifiés directement
dans les sources de la crate ; six correspondaient exactement, un seul écart
mineur (`Rtc::add_local_candidate` retourne `Option<&Candidate>`, pas un
`Result` — seule la construction du `Candidate` via `Candidate::host(...)`
peut échouer). Leçon : une crate récente et activement maintenue peut très
bien coller à la documentation/l'intuition ; ne pas supposer un écart qui
n'existe pas.

**Tâche 9 (capture, windows-rs 0.62)** : trois écarts réels, tous liés au
passage d'une ancienne convention COM à l'ergonomie windows-rs actuelle.
(1) `ClientToScreen` vit dans `Win32::Graphics::Gdi`, pas
`Win32::UI::WindowsAndMessaging` comme `GetClientRect` — deux fonctions de
la « même famille » historique peuvent atterrir dans des modules windows-rs
différents. (2) `ClientToScreen` renvoie un `BOOL` brut avec une méthode
`.ok()`, pas un `Result<()>` comme `GetClientRect` (annotée
succès/échec dans win32metadata, contrairement à `ClientToScreen`).
(3) `IDXGIOutputDuplication::GetDesc`, `IDXGIOutput1::GetDesc` et
`IDXGIAdapter1::GetDesc1` renvoient désormais la structure directement (par
valeur ou par `Result<T>`), plus de paramètre de sortie façon C++. Fait plus
important en amont de ces détails : **`Windows.Graphics.Capture` (WGC) est
inutilisable sur Windows Server 2022** — `captureservice.dll` plante en
`0xc0000005` sur `CreateForWindow`, `GraphicsCaptureSession::IsSupported()`
lève `E_OUTOFMEMORY` avec 13 Go libres, aucun correctif documenté trouvé
malgré plusieurs pistes (fenêtre ciblée, apartment STA/MTA, RoInitialize,
redémarrage complet). Bascule décidée vers DXGI Desktop Duplication, qui
fonctionne. **À vérifier avant tout futur travail de capture sur Windows
Server** : ne pas repartir de WGC sans confirmer d'abord qu'un correctif
existe sur la version cible.

**Tâche 10 (encodeur H.264 matériel, Media Foundation)** : quatre écarts
d'API (features Cargo `Win32_System_Variant`/`Win32_System_Ole` non activées
par défaut ; `VARIANT::from(...)` inexistant, construction manuelle par
champs d'union nécessaire ; `VARIANT_TRUE`/`VARIANT_FALSE` dans
`Win32::Foundation` et non `Win32::System::Variant` ; `GetStringAlloc`
renommée `GetAllocatedString`) — mineurs comparés au piège hors API qui a
coûté plusieurs rondes de correction : **`MFT_OUTPUT_DATA_BUFFER::pSample`
est un `ManuallyDrop<Option<IMFSample>>`, et le lire par `.as_ref().cloned()`
ajoute une référence COM sans jamais relâcher celle déposée par le MFT.**
Chaque image faisait fuir une référence, vidant progressivement le pool de
l'`IMFVideoSampleAllocator` — expliquant en cascade trois symptômes qui
semblaient sans rapport (`MF_E_NOTACCEPTING` après 4 soumissions,
`MF_E_SAMPLEALLOCATOR_EMPTY` insensible à `MF_SA_MINIMUM_OUTPUT_SAMPLE_COUNT`,
plafond à très exactement ~1 image/s = le délai d'attente de l'allocateur
avant abandon). Correctif : `ManuallyDrop::take` au lieu de `.cloned()`,
pour transférer réellement la propriété. Leçon générale : un type
`ManuallyDrop` dans une API Win32/COM signale presque toujours un protocole
de propriété à respecter à la lettre — le lire sans le « prendre »
correctement est le genre de bogue qui se cache derrière son propre
symptôme (débit trop faible pour jamais épuiser le pool assez vite pour
qu'on soupçonne une fuite plutôt qu'une lenteur).

### Pièges de mesure

**Firefox bride son animation hors focus.** Documenté dès la tâche 11 :
Firefox réduit la fréquence d'une animation `requestAnimationFrame` quand sa
fenêtre n'a pas le focus, faussant toute mesure de débit vers le bas sans
qu'aucun message d'erreur ne le signale. Toute mesure de débit ou de latence
sur ce projet **doit** s'assurer que Firefox a le focus juste avant de
mesurer, pas seulement au lancement.

**Un harnais isolé n'est pas comparable au bout en bout.** Le chiffre
« plafond capture/encodage ~47-51 im/s », cité comme référence à plusieurs
reprises entre les tâches 10 et 12, provenait du harnais isolé
`ENCODER_THROUGHPUT_TEST`, qui ne passe **pas** par `Session::run` (donc pas
par la boucle de transport réelle, pas par le rythme de soumission à
16,7 ms). Il n'était tout simplement pas comparable au débit de bout en
bout observé (~23-30 i/s), et l'écart entre les deux a été à tort interprété
comme un signal à expliquer, retardant le vrai diagnostic. Leçon : toute
mesure de performance destinée à être comparée au chemin réel doit passer
par le chemin réel, ou être explicitement étiquetée comme non comparable dès
sa première mention.

**Un chiffre de référence périmé a égaré trois agents.** Le chiffre
« capture ~47 im/s », qui semblait cohérent avec le plafond de bout en bout
observé (~23-30 i/s après division par ~2), s'est révélé périmé : remesuré à
neuf, `CAPTURE_TEST` donne ~90 im/s. Un correctif antérieur (fil d'agitation
de fenêtre à 30 ms, animation par `requestAnimationFrame` au lieu d'un
intervalle fixe à 40 ms) avait déjà levé ce plafond sans que personne ne
remesure la chaîne complète — trois rondes d'agents successives ont raisonné
sur un chiffre obsolète avant que l'un d'eux ne pense à le vérifier à
nouveau. Leçon : un chiffre de référence cité de mémoire (« on sait que la
capture plafonne à X ») doit être revérifié dès qu'il sert de pivot à une
conclusion importante, surtout si du code a changé entre-temps — ne jamais
présumer qu'une mesure ancienne reste valide après une modification, même
sans rapport apparent.

**Le redimensionnement gèle visiblement une fois sur trois à cinq.**
`framesDecoded` seul ne suffit pas à juger de la fluidité : il reste
strictement croissant même pendant un gel de 0,3 à 0,5 s (pas de rejet
d'horodatage, juste un ralentissement de la production). Seuls
`freezeCount`/`totalFreezesDuration` (également disponibles via
`getStats()`, non exposés par l'overlay de cette tâche faute d'être demandés
par le brief) révèlent le phénomène. À ajouter à l'overlay si un futur
chantier a besoin de surveiller la fluidité perçue, pas seulement le débit
moyen.

### Découvertes faites pendant cette recette

**Un redémarrage d'agent (`schtasks /it`) vole le focus à Firefox, même
masqué.** Le mécanisme d'exécution interactive de Task Scheduler
(`schtasks /it`, utilisé pour contourner la frontière session 0/session 1)
active la nouvelle fenêtre de console qu'il crée, **y compris quand elle est
lancée avec `-WindowStyle Hidden`** — confirmé par `GetForegroundWindow()`
juste après un redémarrage de l'agent : la fenêtre au premier plan est la
console PowerShell de la tâche planifiée, pas Firefox, alors même que
Firefox avait été explicitement remis au premier plan quelques secondes
plus tôt. Conséquence pratique découverte par élimination : toute chaîne de
mesure qui redémarre l'agent (obligatoire, l'agent étant mono-session) doit
**re-forcer le focus sur Firefox après le dernier redémarrage de l'agent**,
pas seulement au moment de lancer Firefox — sans quoi le clavier et la
molette n'ont strictement aucun effet, silencieusement (aucune erreur nulle
part dans la chaîne : le message part bien du navigateur, `RTCDataChannel
.send` réussit, l'agent le décode sans erreur, `SendInput` réussit — c'est
Windows qui livre l'événement à la mauvaise fenêtre). Ce projet n'avait
jamais eu besoin de redémarrer l'agent entre le focus et une mesure
d'entrée avant cette tâche (les tâches précédentes lançaient Firefox et
l'agent une seule fois chacun, dans le bon ordre, sans jamais revenir en
arrière) — ce piège était donc resté invisible jusqu'ici.

**La VM peut s'arrêter spontanément en cours de session de mesure** (déjà
documenté aux tâches 7 et 12, reproduit une troisième fois ici) : `virsh
list` passe à « fermé » sans avertissement, `/media/vm` se démonte, toute
commande WinRM échoue. `virsh start Windows` suffit à la relancer (session 1
retrouvée déjà connectée en quelques secondes, sans ré-authentification
manuelle) — mais toute commande shell qui écrit vers `/media/vm` en plein
milieu de cette fenêtre échoue silencieusement si elle n'est pas sous
`set -euo pipefail` (observé : `run-agent.sh` a échoué à l'écriture de son
script d'amorçage puis, comme le script n'utilisait pas de garde explicite
à cet endroit précis, aurait pu laisser croire à un agent relancé alors
qu'il ne l'était pas — heureusement intercepté ici car le script s'arrête
bien sur l'échec de `cat >`, mais le symptôme côté harnais de mesure, un
`getStats()` qui ne bouge jamais, était initialement indiscernable d'un vrai
défaut d'entrée). Leçon : après tout redémarrage de VM en cours de mesure,
revérifier explicitement `agent.exe` et la session interactive avant de
recommencer à mesurer, plutôt que de supposer que le redémarrage a tout
remis dans l'état précédent.

**La molette (`WheelEvent` synthétique) n'a pas reproduit d'effet visible
dans le harnais de cette recette**, alors que le clavier (`KeyboardEvent`
synthétique) fonctionnait de façon fiable dans les mêmes conditions de
session (focus revérifié, canaux confirmés ouverts, envois confirmés par
instrumentation de `RTCDataChannel.send`, zéro erreur de décodage côté
agent). Cause non isolée dans le budget de cette tâche — hypothèses non
tranchées : positionnement du curseur insuffisant avant l'émission du
`WheelEvent` (le gestionnaire `onWheel` de `client/src/input.ts` n'envoie
que le delta, jamais une position, en s'appuyant sur un déplacement de
curseur préalable), paramétrage `deltaMode`/magnitude ne correspondant pas
à ce qu'un vrai périphérique produit, ou différence de routage Windows
molette-sous-curseur / molette-vers-fenêtre-active spécifique à ce serveur.
Ne remet **pas** en cause la preuve de la tâche 12 (molette prouvée
fonctionnelle par capture d'écran directe d'un défilement réel, harnais
différent — `client/verify-webrtc.mjs`-like avec pilotage CDP direct plutôt
que dispatch d'événements synthétiques dans la page). À reproduire avec le
harnais de cette recette avant de s'appuyer dessus pour un chantier
ultérieur sensible à la molette (le jeu vidéo, notamment).

---

## Conclusion

Deux critères sur cinq sont tenus sans réserve (capture par fenêtre,
entrées — avec la réserve molette ci-dessus). Les trois autres — débit,
latence, redimensionnement fiable — sont chacun affectés par le même
étage à des degrés de certitude différents, à ne pas aplatir en une seule
affirmation : l'encodage matériel H.264, dont le rythme de soumission
accepté (~30 Hz) ne suit pas la cadence de la boucle de transport (60 Hz),
explique **solidement** le débit (mesure directe du rythme
`METransformNeedInput`) et **solidement** la reconstruction à froid qui
domine le délai de redimensionnement. Pour la latence, seul le **plancher**
(meilleurs essais, 54 à 133 ms, cohérent avec une attente d'environ un
cycle manqué) se rattache à ce même mécanisme de façon plausible ; la
**traîne** (jusqu'à 1,5 s) reste, à l'issue de cette recette, une question
ouverte — l'hypothèse la plus cohérente avec les données récoltées est une
dérive du tampon de gigue adaptatif plutôt qu'un arrêt de capture/encodage
(voir critère 3). C'est néanmoins le prérequis de travail le plus clair
pour la suite : tant que le rythme d'encodage à 30 Hz n'est pas expliqué et
corrigé côté pilote NVENC/Media Foundation, le débit ne pourra pas
atteindre sa cible, et la latence ne pourra pas descendre sous son plancher
actuel, quelle que soit l'optimisation apportée ailleurs dans la chaîne
(réseau et capture sont déjà largement dans leur budget). Le tampon de
gigue non réglé (`playoutDelayHint`/`jitterBufferTarget` jamais positionnés)
est un second levier à traiter en parallèle — mais pas à l'aveugle : voir la
mise en garde du critère 3 sur le risque de convertir de la latence en gels
si ce réglage est durci avant que le rythme d'encodage ne soit lui-même
régularisé.
