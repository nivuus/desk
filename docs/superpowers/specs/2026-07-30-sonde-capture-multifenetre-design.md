# Sonde de capture multi-fenêtres — conception

> Sonde préalable au **chantier D (multi-fenêtres)**, décrit dans
> `2026-07-28-support-jeux-design.md` §4 et §5. Elle ne construit rien du
> chantier : elle tranche par la mesure la décision de capture laissée
> ouverte, dont dépend tout le reste.

## 1. Question posée

Le modèle multi-fenêtres suppose une capture **par fenêtre**, correcte même
quand une autre fenêtre la recouvre. La capture actuelle est du **DXGI Desktop
Duplication recadré** : elle duplique le bureau entier puis découpe la région
de la fenêtre. Deux fenêtres qui se chevauchent produisent donc un recadrage
pollué par ce qui est au-dessus.

`Windows.Graphics.Capture`, la seule API qui donne la capture hors-écran
gratuitement, a été **abandonnée au jalon 1** (commit `4493b24`) : sur cette VM
Windows Server 2022 (build 20348), `captureservice.dll` plantait en
`0xc0000005` de façon déterministe et `IsSupported()` levait `E_OUTOFMEMORY`
avec 13 Go libres.

Le risque n°1 du chantier D (popup blocker) a été levé le 28/07/2026. Celui-ci
ne l'a jamais été, et il conditionne davantage : sans capture par fenêtre
non polluée, le modèle multi-fenêtres ne tient pas.

## 2. Décisions de cadrage

| Décision | Retenu |
| --- | --- |
| Entrée dans le chantier D | Sonde de capture **avant** toute spécification de D |
| Voies mises au banc | Les **quatre** (WGC, un moniteur par fenêtre, tuilage disjoint, replis par fenêtre) |
| Cible de fenêtres simultanées | **8** — assumé : la mesure touchera aussi le plafond NVENC |
| Profondeur | **Capture + encodage**, pas de transport ni de topologie WebRTC |
| Organisation | **Deux temps** : viabilité des quatre voies, puis banc sur les seules survivantes |

Le choix de 8 fenêtres est délibérément au-delà des cas d'usage visés (Steam +
jeu, un IDE et ses fenêtres) : l'objet est de trouver les bornes réelles, pas
de confirmer un cas facile.

## 3. Relevé préalable — un écart documentaire à lever

Relevé sur la VM le 30/07/2026 (`Win32_VideoController`) : le bureau est piloté
par la **RTX 4070 à 2400×1080** ; le « SudoMaker Virtual Display Adapter » est
présent mais **n'annonce aucune résolution courante**.

Or deux documents du dépôt se contredisent :

- `plans/fix-debit-socket-report.md:163` — la capture passerait par l'écran
  virtuel SudoMaker ;
- commit `4493b24` — le bureau est piloté par la RTX 4070, en position 0 de
  l'énumération DXGI.

Les deux ne peuvent pas être vrais. **Le premier acte de la sonde est un relevé
DXGI complet** (adaptateurs, sorties, laquelle porte le bureau, à quelle
résolution) : tout le dimensionnement des voies 2 et 3 en dépend.

## 4. Les quatre questions de viabilité

1. **WGC.** `GraphicsCaptureSession::IsSupported()` répond quoi aujourd'hui ?
   `GraphicsCaptureItem::CreateForWindow` aboutit-il ? Des trames arrivent-elles ?
   `captureservice.dll` plante-t-il encore ? Si l'échec s'explique par un
   composant Windows absent (App Compatibility de Server Core / Desktop
   Experience), la sonde le **constate et s'arrête** — installer une
   fonctionnalité serveur implique un redémarrage de la VM, qui relève d'une
   décision humaine, pas de la sonde.

2. **Un moniteur virtuel par fenêtre.** Combien de sorties le SudoMaker
   sait-il instancier, par quel mécanisme, à quelles résolutions ? Un unique
   device D3D11 peut-il tenir N `IDXGIOutputDuplication` simultanées ?

3. **Tuilage disjoint sur un bureau unique.** Le bureau peut-il dépasser
   largement 2400×1080 ? À la résolution actuelle, huit tuiles font 600×270
   chacune : la voie n'a de sens que si le bureau s'agrandit beaucoup. C'est
   donc une question de **disponibilité** avant d'être une question de
   performance.

4. **Replis par fenêtre.** `PrintWindow(PW_RENDERFULLCONTENT)` et
   `DwmGetDXSharedSurface` rendent-ils autre chose que du noir sur une fenêtre
   **D3D** ?

## 5. Critères de jugement

**Porte éliminatoire — correction sous recouvrement.** Une fenêtre dont la mire
reste juste **alors qu'une autre fenêtre la recouvre**. Vérification par
comparaison de pixels dans le code, contre un motif attendu ; **aucun jugement
visuel**. Le projet porte déjà un réglage non calibré (`BPP_MIN`) faute d'un
critère qui supposait un œil humain — on ne recommence pas. Une voie qui échoue
ici est éliminée **sans** mesure de cadence : chiffrer la vitesse d'une image
fausse n'apprend rien.

**Porte éliminatoire — chemin GPU.** La texture reste sur le GPU jusqu'à NVENC.
Une voie qui rapatrie les pixels en mémoire centrale est écartée quelle que
soit la qualité de son image : le pipeline actuel livre une texture D3D11 que
l'encodeur consomme sans passage par le CPU, et 8 fenêtres à 60 Hz ne
survivraient pas à une copie descendante.

**Mesures, sur les voies qui franchissent les deux portes :**

- **Cadence** — non pas « tient-elle 60 fps à 8 fenêtres », mais **à quel rang
  N la cadence décroche**, relevée par fenêtre.
- **Images perdues** par fenêtre.
- **Plafond NVENC** — nombre d'encodeurs simultanés créés avant échec, sur
  cette RTX 4070 précisément (l'ordre de grandeur admis, 8 sur Ada, n'est pas
  une mesure).

## 6. Architecture de la sonde

Sous `agent/src/diagnostics/multifenetre/`, selon le patron déjà en place :
activation par variable d'environnement, `aiguiller()` rend `true` et `main`
s'arrête là — aucune session WebRTC, rien de construit.

| Module | Rôle |
| --- | --- |
| `disponibilite.rs` | Temps 1 : relevé DXGI et verdict par voie |
| `banc.rs` | Temps 2 : passes témoin / capture / capture+encodage |
| `mires.rs` | Création et rendu des fenêtres de test |
| `voies.rs` | Trait `VoieDeCapture` ; une implémentation par fichier voisin dès qu'elle dépasse quelques dizaines de lignes |
| `regions.rs` | Attribution des régions et tuiles — **portable, testé** |
| `verification.rs` | Motif attendu contre pixels lus — **portable, testé** |

Le découpage respecte la règle des 500 lignes : aucun de ces fichiers ne naît
au-dessus.

### 6.1 `VoieDeCapture` — écrire le banc une fois

Un trait commun : ouvrir un flux sur un `HWND`, rendre la texture suivante,
fermer. Sans lui, le banc serait écrit une fois par voie et ne comparerait plus
les mêmes choses. Bénéfice secondaire assumé : c'est la couture dont le
chantier D aura besoin de toute façon pour rendre la capture substituable.

### 6.2 Les mires — peintes en D3D11, pas en GDI

Huit fenêtres créées par l'agent lui-même plutôt que huit navigateurs : contenu
déterministe, animation maîtrisée, et chaque fenêtre peint un motif encodant
**son identité et son numéro de trame** — c'est ce qui rend la vérification
automatisable.

Le rendu passe par une **swapchain D3D11**. Une mire peinte en GDI donnerait un
faux positif sur `PrintWindow`, qui échoue précisément sur le contenu D3D : on
validerait une voie qui s'effondrerait devant un vrai jeu.

L'animation n'est pas décorative : **Desktop Duplication n'émet une image que
lorsque le bureau change**. Piège déjà payé au jalon 1, où le premier
`next_frame` réussissait et tous les suivants expiraient.

### 6.3 Le témoin — ne pas imputer à la capture le coût des mires

Huit swapchains qui peignent à 60 Hz consomment du GPU, et cette charge
entrerait dans la mesure du plafond NVENC. Le banc commence donc par une passe
**mires seules, sans capture ni encodage**, exactement comme le profil `lan`
sert de témoin au banc `netem`. Sans elle, un décrochage à six fenêtres serait
indiscernable d'un décrochage de la mire elle-même.

### 6.4 Instrumentation

Compteurs agrégés, journalisés à intervalle — **jamais une trace par trame**.
Au chantier NAT, une trace par paquet écrite sur le partage CIFS a détruit la
session qu'elle mesurait.

## 7. Protocole

### Temps 1 — disponibilité, une voie par exécution du binaire

Ce n'est pas un détail d'organisation : un plantage en `0xc0000005` emporte le
processus. Éprouver les quatre voies dans une même exécution ferait perdre les
trois autres avec la première. Une variable d'environnement par voie, et un
script hôte qui les enchaîne et récolte les journaux.

Ordre : relevé DXGI → WGC → sorties virtuelles → replis par fenêtre. Chaque
voie ressort avec un verdict : **viable**, **éliminée**, ou **conditionnelle**
(viable moyennant une action à décider, telle qu'une installation de composant).

### Temps 2 — banc, sur les seules survivantes

Pour chaque voie retenue, et pour N de 1 à 8 fenêtres :

1. passe **témoin** — mires seules ;
2. passe **capture seule** ;
3. passe **capture + encodage**.

À mi-parcours des passes 2 et 3 — la passe témoin ne capture rien, il n'y a
donc rien à y vérifier — une mire est déplacée pour en recouvrir une autre : la
fenêtre recouverte doit continuer de rendre sa mire juste (§5).

Puis, séparément : création d'encodeurs jusqu'à l'échec, pour établir le
plafond NVENC.

### Lancement

Par `scripts/run-agent.sh`, en **session interactive** : ni la capture ni
`SendInput` ne franchissent la session 0 où tourne WinRM. Les nouvelles
variables s'ajoutent au heredoc du script, comme `CAPTURE_TEST` et
`AUDIO_PROBE` avant elles.

## 8. Gestion des erreurs

- **Isolation par processus** au temps 1 (voir §7) — une voie qui tue le
  processus n'emporte pas les autres.
- **Échec distingué de l'absence** : une voie qui ne produit aucune image doit
  dire *pourquoi* (HRESULT, code de plantage, expiration), jamais rendre un
  simple « rien ». C'est la différence entre « éliminée » et « non mesurée ».
- **Nettoyage des mires** : les fenêtres créées sont détruites en fin de sonde,
  y compris sur chemin d'erreur. Une mire orpheline fausserait la mesure
  suivante — et le banc est lancé plusieurs fois de suite.
- **Sorties virtuelles** : toute modification de configuration d'affichage est
  réversible et remise en état en fin de sonde. La VM sert à d'autres travaux.

## 9. Stratégie de test

La sonde tourne sur la VM et échappe aux tests automatisés, comme tout
`#[cfg(windows)]` de ce dépôt. Deux morceaux n'ont pourtant rien de Windows et
sont **testés sur l'hôte Linux**, sur le modèle de `geometry.rs` :

- `regions.rs` — attribution des régions et calcul des tuiles disjointes :
  huit tuiles sur un bureau donné ne se recouvrent pas, tiennent dans les
  bornes, et le calcul refuse une configuration qui ne rentre pas.
- `verification.rs` — un motif attendu confronté à des pixels lus : accepte la
  mire juste, **rejette** une mire décalée d'une fenêtre voisine, rejette une
  image noire.

Sans ces tests, un banc qui « ne trouve rien » serait indiscernable d'un
vérificateur cassé.

## 10. Livrables

1. `docs/superpowers/plans/2026-07-30-sonde-capture-multifenetre-resultats.md` —
   relevés bruts, verdict par voie, recommandation pour le chantier D.
2. Un amendement au §4 de `2026-07-28-support-jeux-design.md` **tranchant** sa
   décision ouverte (WGC contre Desktop Duplication), avec la mesure en appui.
   La présente spec pose la méthode ; c'est le document de résultats qui porte
   la décision.
3. `CLAUDE.md` gagne sa section : ce qui a été mesuré, et les pièges rencontrés.

## 11. Hors périmètre

Explicitement **pas** dans cette sonde, tout cela relevant du chantier D
lui-même :

- `SetWinEventHook` et la détection dynamique des fenêtres ;
- le filtrage « Alt-Tab-able » ;
- la topologie N `RTCPeerConnection` et la répartition du débit entre flux ;
- l'audio par processus (sonde d'activation déjà faite au chantier A) ;
- le cycle de vie fenêtre Windows ↔ fenêtre navigateur ;
- le plein écran Windows → navigateur.

La sonde ne touche pas au chemin de production, à une exception additive près :
rendre explicite le choix de la sortie DXGI dans `capture.rs`, aujourd'hui figé
sur la première sortie attachée au bureau (`capture.rs:313`). La voie « un
moniteur par fenêtre » ne peut pas être éprouvée sans cela.

## 12. Risques et inconnues

| Inconnue | Conséquence si défavorable |
| --- | --- |
| Le mécanisme de création de sorties du SudoMaker n'est pas documenté | La voie 2 peut être inéprouvable sans un autre pilote d'affichage virtuel |
| Les quatre voies échouent | Le modèle multi-fenêtres du §4 est à rouvrir — c'est précisément ce que la sonde existe pour découvrir tôt |
| Le plafond NVENC tombe très bas | Le chantier D devra suspendre l'encodage des fenêtres masquées dès la v1, et non « en option souhaitable » |
| La charge des mires domine la mesure | Le témoin (§6.3) le rend visible ; le banc redescendrait alors à une cadence de mire plus basse, au prix d'une mesure moins représentative |
