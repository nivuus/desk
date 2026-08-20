# Sous-projet ⑥ — Design system

**Date** : 19 août 2026
**Cadrage parent** : `2026-07-27-refonte-produit-design.md`, §5 ⑥ (« Design
system »), §3 (« Direction visuelle : sobre "pro" » ; « Client web : Vite +
TypeScript »), §5 ② (les **deux surfaces** : hub et fenêtre de session), §9.5
(« tokens posés dès le début, appliqué en continu »), §11 (l'ancien code reste
fonctionnel jusqu'à son remplacement).
**Statut** : conception, plan à écrire
**Portée** : la direction visuelle du nouveau produit, et les tokens partagés
entre toutes ses surfaces

---

## 0. Ce document ne modifie aucun code

Il n'a été écrit qu'avec des lectures et des commandes de mesure. Chaque
affirmation sur l'existant porte son `fichier:ligne`, relu après avoir été
écrit. Les sorties de commande citées ont été obtenues en les lançant.

---

## 1. Objet, et pourquoi un document pour six puces

Le cadrage décrit ⑥ en trois puces (§5 ⑥) : direction sobre, tokens partagés,
application continue. Prises à la lettre, elles n'engagent rien de
vérifiable — « sobre », « soignée », « discrets » sont des adjectifs, et ce
dépôt traîne déjà plusieurs constantes qu'il déclare **« jamais calibrées par
un jugement visuel »** (`BPP_MIN`, `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`,
`TAILLE_MAX_SORTIE`). Un design system écrit sans précaution ajouterait une
douzaine de constantes de la même famille, et il n'y aurait aucun moyen de
savoir si elles tiennent.

Ce document fait donc deux choses que les trois puces ne font pas :

1. il **sépare** ce qui est mesurable par une commande de ce qui est un
   **jugement humain assumé**, et nomme le second comme tel plutôt que de le
   déguiser en mesure ;
2. il pose le **socle** (§6, S1) avant les surfaces qui en dépendront, parce
   que l'ordre inverse produit deux jeux de valeurs qui divergent — ce dépôt a
   payé ce mode de défaillance sur une constante recopiée à la main
   (`agent/src/superviseur/protocole.rs:17` recopié en
   `client/src/shell-page.ts:10`, relevé par le sous-projet ⑤, §2.3 ①).

---

## 2. État des lieux, relevé et non supposé

### 2.1 Tout le style du nouveau produit tient en 85 lignes

`client/src/style.css` est le **seul** fichier CSS du paquet client. Mesuré le
19 août 2026 :

```
$ find client -name '*.css' -not -path './node_modules/*' -not -path './dist/*'
client/src/style.css
$ wc -l client/src/style.css client/index.html client/shell.html
   85 client/src/style.css
   16 client/index.html
   14 client/shell.html
```

Ce que ces 85 lignes contiennent :

- **deux « tokens » de fait**, `--surface: #0b0d10` (`style.css:3`) et
  `--text: #e6e8eb` (`style.css:4`) — les seules propriétés personnalisées du
  dépôt ;
- **`color-scheme: dark` en dur** (`style.css:2`) : le produit n'a **pas** de
  thème clair, et n'a aucun mécanisme pour en avoir un ;
- une police système en une ligne, base **14 px**, interligne **1,5**
  (`style.css:18`) ;
- le style des **quatre** éléments de la fenêtre de session : `#remote`
  (`:21`), `#status` (`:29`), `#stats` (`:45`), `#fullscreen` (`:57`).

**Aucune occurrence de `prefers-color-scheme`, `windowControlsOverlay`,
`titlebar` ni `manifest`** dans `client/src` ni dans les deux HTML — vérifié
par `grep -rn`, qui ne rend qu'un faux positif sur le mot
« manifesterait » (`client/src/viewport.test.ts:22`).

### 2.2 La page-shell n'a AUCUNE feuille de style, et le build le prouve

`client/shell.html` ne porte aucun `<link rel="stylesheet">` — les 14 lignes du
fichier se lisent d'un coup d'œil. La conséquence se mesure sur le produit
bâti (`npm run build`, vite 6.4.3, exécuté le 19 août 2026) :

```
$ for f in dist/*.html; do grep -q 'rel="stylesheet"' "$f" \
    && echo "OK   $f" || echo "SANS $f"; done
OK   dist/index.html
SANS dist/shell.html
```

**C'est la divergence que le cadrage demande d'empêcher, déjà présente**, et
sous sa forme la plus extrême : une surface n'a pas *d'autres* valeurs, elle
n'en a aucune. Le §7.3 en fait un critère, et ce relevé est sa preuve
d'atteignabilité — le contrôle est **ROUGE aujourd'hui**, sans rien casser
exprès.

### 2.3 Le poids actuel du client, mesuré — il gouverne la décision typographique

```
$ npm run build && find dist -type f -printf '%s\t%p\n' | sort -rn && du -sb dist
20744   dist/assets/main-cq0ocINa.js
2018    dist/assets/shell-B8QdpCJy.js
1055    dist/assets/main-1syDQ0iu.css
749     dist/index.html
711     dist/assets/modulepreload-polyfill-B5Qt9EMX.js
460     dist/shell.html
25737   dist
```

**Le produit entier pèse 25 737 octets, dont 1 055 de CSS.** C'est le chiffre
qui tranche le §4.3.

### 2.4 Cinq couleurs littérales vivent hors de toute déclaration de token

```
$ grep -nE '#[0-9a-fA-F]{3,8}\b' client/src/style.css
3:    --surface: #0b0d10;
4:    --text: #e6e8eb;
26:    background: #000;
$ grep -nE 'rgb\(|hsl\(' client/src/style.css
35:    background: rgb(0 0 0 / 0.72);
51:    background: rgb(0 0 0 / 0.72);
68:    background: rgb(0 0 0 / 0.55);
73:    background: rgb(0 0 0 / 0.75);
```

Deux des trois hexadécimaux sont les déclarations de token elles-mêmes ; il
reste **cinq** valeurs littérales employées comme couleurs (l. 26, 35, 51, 68,
73). C'est la ligne de base du critère §7.2, et elle le rend **ROUGE
aujourd'hui**.

### 2.5 🔴 L'écran de connexion de P2 n'existe pas encore côté navigateur

Le cahier des charges de ce document annonçait que « le sous-bloc P2 vient
d'ajouter un écran de connexion côté client ». **Ce n'est pas ce que le dépôt
montre**, et le corriger change l'ordonnancement de ⑥ (§6.1) :

| Ce qui existe | Preuve |
| --- | --- |
| les routes d'authentification, côté serveur | `plateforme/src/http/routes-auth.ts`, `plateforme/src/identite/{garde,jeton,mot-de-passe}.ts` |
| la création de compte en ligne de commande | `plateforme/src/admin/creer-utilisateur.ts` (commit `5add07b`) |
| **aucun écran de connexion** | `client/vite.config.ts:10-12` déclare exactement deux entrées, `index.html` et `shell.html` ; aucun fichier de `client/src` ne mentionne `/auth/` ni `jeton` |
| **aucun jeton dans la poignée de main** | `client/src/shell-page.ts:45` envoie encore `{ role: 'client', session }` |

**Conséquence** : la moitié navigateur de P2 est **à venir**, et c'est elle qui
posera la **première surface d'interface réelle du nouveau produit** — la
fenêtre de session n'affiche qu'un `<video>` et trois bandeaux. Le socle de
tokens (S1) doit donc être posé **avant ou avec** cet écran, pas après. Le
cadrage le disait déjà (§9.5, « tokens posés dès le début ») ; le relevé lui
donne une date.

### 2.6 Le legacy, et ce qu'il faut en retenir

`assets/index.html` et `assets/app.tpl.html` (3 013 et 4 239 octets, `ls -la`)
portent l'interface du Guacamole historique, avec son style **en ligne dans le
`<head>`** : `background: linear-gradient(135deg, #667eea 0%, #764ba2 100%)`
(`assets/index.html:36`), cartes translucides, `backdrop-filter`. C'est le
« glassmorphism » que `CLAUDE.md` §« 4. Design Obsolète de la Page d'Accueil »
décrit comme la modernisation d'octobre 2025.

**C'est exactement la direction que ⑥ ne prend pas** : dégradé saturé,
transparence décorative, aucune hiérarchie typographique. Il est lu ici pour
savoir ce qu'on remplace, **jamais comme modèle** — le cadrage §3 nomme
Linear/Vercel, pas ceci.

---

## 3. La direction visuelle, énoncée en contraintes plutôt qu'en adjectifs

« Sobre pro » n'est pas vérifiable. Ce qui suit l'est, et c'est ce qui engage :

1. **Un seul accent chromatique**, et il ne sert qu'à trois choses : l'action
   principale, l'état de focus, et le lien. Toute autre couleur du produit est
   un gris, ou une couleur **sémantique** (succès, alerte, danger) qui ne peut
   apparaître que pour dire un état.
2. **Aucun dégradé décoratif, aucune ombre portée colorée, aucun
   `backdrop-filter` de décor.** L'élévation se dit par la valeur du fond et
   par une bordure, pas par un flou. *(Exception unique et nommée : les
   bandeaux flottants de la fenêtre de session, posés au-dessus d'une vidéo
   dont on ne contrôle pas le contenu — §5.2.)*
3. **La hiérarchie se fait par la taille et la valeur, jamais par la
   couleur** : trois niveaux d'encre (`--texte-fort`, `--texte`,
   `--texte-faible`), sept crans typographiques (§4.4).
4. **Densité** : base 14 px et échelle d'espacement à pas de 4 px (§4.4), à
   opposer aux 16 px et aux marges de 32 px du legacy.
5. **Rien ne bouge sans raison** : une transition n'existe que pour un
   changement d'état déclenché par l'utilisateur ou par le produit, et dure au
   plus `--duree-2` (§4.4). Aucune animation d'entrée, aucun défilement
   parallaxe.

⚠️ **Les cinq points ci-dessus sont des DÉCISIONS, pas des mesures.** Les
points 1, 2 et 5 sont vérifiables par une commande (§7.2 pour les couleurs
littérales ; un `grep` de `backdrop-filter`/`linear-gradient` pour le point 2).
Les points 3 et 4 ne le sont **pas** : qu'une échelle 1,2 soit la bonne, ou que
14 px soit assez dense sans être trop petit, relève du **jugement humain**, et
§8 le déclare comme tel.

---

## 4. Les décisions, avec leur justification et leur coût

### 4.1 Où vivent les tokens : une source unique, en CSS

**Décision : `client/src/design/tokens.css` est la source unique. Il n'y a pas
de miroir TypeScript des valeurs.**

*Justification.* Un miroir TypeScript serait une seconde copie, et ce dépôt a
déjà payé une copie manuelle de constante entre deux langages
(`protocole.rs:17` → `shell-page.ts:10`, §2.5 du sous-projet ⑤). Ce dont le
TypeScript a besoin n'est pas la **valeur** d'un token mais son **nom**, et
seulement pour basculer un attribut — ce qui ne demande aucune couleur.

*La règle, quand du code a réellement besoin d'une valeur* (par exemple pour
un `<meta name="theme-color">` d'une PWA, ou un `canvas`) : il la lit au
moment où il en a besoin, par
`getComputedStyle(document.documentElement).getPropertyValue('--…')`. Un seul
mécanisme, zéro duplication.

*Coût, assumé.* La lecture par `getComputedStyle` est un accès au style
calculé, donc synchrone et potentiellement coûteuse si elle est faite en
boucle. Le contrat est qu'elle n'est faite **qu'à un changement de thème**,
jamais par image. **Aucun appelant n'existe aujourd'hui** — le nouveau client
n'a ni manifest ni `canvas` (§2.1) ; la règle est écrite pour le premier qui
en aura besoin, pas pour un besoin constaté.

*Ce qui l'accompagne quand même en TypeScript* : `client/src/design/theme.ts`,
qui ne connaît **que trois chaînes** — `'systeme' | 'clair' | 'sombre'` — et
aucune couleur. C'est le seul module TypeScript du sous-projet, et il est
testable sans DOM, comme `status.ts` et `audio.ts` le sont déjà
(`client/src/status.ts:16-17` explique cette pratique).

**Le contrôle qui garantit la non-divergence entre surfaces** est en §7.3 : ce
n'est pas une convention, c'est une commande, et elle est ROUGE aujourd'hui
(§2.2).

### 4.2 Le thème clair/sombre : trois états, une clé, et l'attribut avant la première peinture

**Décision : trois états — `systeme` (défaut), `clair`, `sombre`.** L'état
vit dans `localStorage`, sous la clé `theme`. Il est appliqué par un attribut
`data-theme` sur `<html>` ; **son absence signifie `systeme`**.

*La structure CSS, sombre d'abord* :

```css
:root { /* palette SOMBRE, sans condition */ }
@media (prefers-color-scheme: light) {
  :root:not([data-theme="sombre"]) { /* palette CLAIRE */ }
}
:root[data-theme="clair"] { /* palette CLAIRE */ }
```

*Pourquoi sombre d'abord.* La fenêtre de session est dominée par une vidéo sur
fond noir (`style.css:26`). Un défaut clair y produirait, au pire moment — le
premier paint, avant que la vidéo n'arrive — un cadre blanc autour d'un trou
noir. Le sens inverse est bénin.

*Coût, assumé.* La palette claire est déclarée **deux fois** (dans la requête
média et dans le sélecteur d'attribut). C'est le prix de la symétrie : sans le
second bloc, un utilisateur sous préférence système sombre ne pourrait pas
choisir le clair. Le contrôle §7.4 vérifie que les **trois** blocs déclarent
exactement le même ensemble de noms — c'est lui qui empêche cette duplication
de dériver.

*Avant la première peinture.* Lire `localStorage` depuis un module ES chargé
en fin de `<head>` laisserait un éclair de mauvais thème. Il faut un script
**en ligne**, synchrone, qui pose `data-theme` avant tout. Le recopier dans
chaque HTML d'entrée serait exactement la duplication que §4.1 refuse :
**décision — un greffon Vite (`transformIndexHtml`) l'injecte dans toutes les
entrées depuis une source unique**, `client/src/design/amorce-theme.js`. Coût :
une dizaine de lignes dans `client/vite.config.ts`, un fichier de plus, et un
script en ligne (donc une contrainte si une CSP stricte arrive un jour — nommé
ici, non traité).

#### Ce qui se passe quand l'utilisateur change de thème dans UNE des N fenêtres

C'est la question que pose le multi-fenêtres, et elle a une réponse précise.
Le produit ouvre une fenêtre navigateur par fenêtre Windows, plus la
page-shell — et **toutes sont de même origine** :
`client/src/shell-page.ts:19` ouvre `/?session=…`, un chemin relatif. Le
partage de `localStorage` est donc acquis.

**Le mécanisme retenu est l'événement `storage`**, et pas `BroadcastChannel` :
la persistance est de toute façon nécessaire, `storage` en découle sans ajouter
d'API, et une fenêtre ouverte plus tard lit la même clé au démarrage.

⚠️ **Le piège, qui doit être écrit et testé : l'événement `storage` ne se
déclenche PAS dans le document qui a écrit.** La fenêtre qui change le thème
doit donc l'appliquer **elle-même**, en plus d'écrire. Un module qui ne
ferait qu'écrire en comptant sur l'événement laisserait la fenêtre d'origine
inchangée — et c'est précisément la seule que l'utilisateur regarde. Le
critère §7.5 exerce les deux moitiés séparément.

*Ce que le changement de thème NE fait pas* : il ne touche pas le `<video>`,
donc n'interrompt ni ne renégocie aucun flux. C'est une propriété du choix
« attribut + variables CSS » : aucune reconstruction de DOM.

### 4.3 La typographie : polices système, coût nul, et le budget qui l'a tranché

**Décision : pile de polices système. Aucun fichier de police n'est embarqué,
aucun CDN n'est appelé.**

*Justification, chiffrée.* Le produit entier pèse **25 737 octets** (§2.3).
Une police variable moderne, sous-ensemble latin et format WOFF2, se compte en
dizaines de kilo-octets : l'embarquer **doublerait au moins le poids total du
client** pour un gain purement esthétique, sur un chemin où la première image
doit arriver vite (le cadrage vise « < 3 s si VM chaude », §6). Le CDN est
exclu séparément et catégoriquement : la cible est une VM sur un réseau
potentiellement restreint, et une police qui ne charge pas est un décalage de
mise en page à chaque ouverture de fenêtre.

*Le coût, assumé et non mesuré.* Le rendu **diffère d'un système à l'autre** :
Segoe UI Variable sur Windows 11, San Francisco sur macOS, une police
d'interface variable selon la distribution sous Linux. Deux captures d'écran
prises sur deux postes ne se superposent pas. **C'est le prix de zéro octet, et
il est accepté** — il interdit en revanche toute recette par comparaison de
pixels entre machines (§7.7).

*Les deux piles* :

```css
--police-ui: system-ui, -apple-system, "Segoe UI Variable Text", "Segoe UI",
             Roboto, "Helvetica Neue", Arial, sans-serif;
--police-mono: ui-monospace, "Cascadia Mono", "Segoe UI Mono", "SF Mono",
               "JetBrains Mono", Menlo, Consolas, monospace;
```

La pile d'interface étend celle qui existe déjà (`style.css:18`) plutôt que de
la remplacer. La pile monospace est neuve, et elle a un usage précis : le
bandeau `#stats`, qui affiche des nombres qui changent (`client/src/stats.ts:105`)
et qui porte déjà `font-variant-numeric: tabular-nums` (`style.css:52`) pour
la même raison.

*Réserve de portée* : `--police-mono` est déclarée en S1 et n'a **qu'un seul
appelant prévu**. Le contrôle §7.6 (« tout token déclaré est employé ») le
vérifiera ; si aucun appelant n'apparaît, le token sort — un token orphelin est
du code mort, et ce dépôt en a déjà deux qu'il conserve en le déclarant
(`TAILLE_MAX_SORTIE`, `borner_a_la_taille_max`).

> ✅ **RÉSERVE LEVÉE : `--police-mono` EST CÂBLÉ** (sous-bloc S4, tâche 6), sur
> `#stats` — l'appelant unique que ce paragraphe désigne. Trois sous-blocs se
> sont passé la décision « le câbler ou le retirer », faute d'avoir le droit de
> changer l'apparence de la fenêtre de session ; S4 l'a, et il câble. **La liste
> d'attente de §7.6 est désormais VIDE**, 52 tokens déclarés pour 52 employés,
> et elle **ne disparaît pas pour autant** : c'est l'ÉGALITÉ qui vaut, pas la
> liste, et celle-ci doit rester pour attraper un orphelin futur.
> ⚠️ **Que la pile monospace se lise mieux que le crénage qu'elle remplace est
> un JUGEMENT HUMAIN** (§8) : aucune commande ne le dira.

### 4.4 Les échelles, chiffrées

**Décision : toutes les longueurs typographiques et d'espacement sont en `rem`,
et la racine n'est JAMAIS forcée.** Pas de `html { font-size: 62.5% }`, pas de
`font-size` en pixels sur `<html>`.

*Justification.* Le point 4 de §3 demande de la densité, donc une base de
14 px — soit **moins** que le défaut du navigateur. Exprimer l'échelle en
pixels figerait cette réduction et annulerait le réglage de taille de police du
navigateur, ce qui est un défaut d'accessibilité réel. En `rem` sur une racine
laissée libre, la base vaut 14 px chez qui n'a rien réglé, et suit chez qui a
réglé. *Coût* : les valeurs en `rem` sont moins lisibles qu'en pixels ; la
table ci-dessous porte donc les deux, l'équivalent en pixels valant pour une
racine à 16 px.

**Échelle typographique** — sept crans, ratio ≈ 1,2, arrondi au pixel entier
(aucun demi-pixel, qui rend inégalement selon le moteur) :

| Token | `rem` | px à racine 16 | Emploi |
| --- | --- | --- | --- |
| `--t-xs` | 0.6875 | 11 | mentions légales, étiquettes de graphique |
| `--t-s` | 0.75 | 12 | `#stats`, métadonnées |
| `--t-m` | 0.875 | **14** | **corps — le défaut, celui de `style.css:18`** |
| `--t-l` | 1 | 16 | intertitres, libellés de champ |
| `--t-xl` | 1.25 | 20 | titre de section |
| `--t-2xl` | 1.5 | 24 | titre de page |
| `--t-3xl` | 2 | 32 | titre d'écran de connexion |

**Plancher dur : rien sous `--t-xs`.** Toute taille inférieure est un défaut,
pas un choix de densité.

**Interlignes** : `--lh-serre: 1.25` (titres, à partir de `--t-xl`),
`--lh-normal: 1.5` (corps — la valeur de `style.css:18`), `--lh-large: 1.7`
(paragraphes longs).

**Échelle d'espacement** — pas de 4 px, huit crans :

| Token | `rem` | px | Token | `rem` | px |
| --- | --- | --- | --- | --- | --- |
| `--e-1` | 0.25 | 4 | `--e-5` | 1.5 | 24 |
| `--e-2` | 0.5 | 8 | `--e-6` | 2 | 32 |
| `--e-3` | 0.75 | **12** | `--e-7` | 3 | 48 |
| `--e-4` | 1 | 16 | `--e-8` | 4 | 64 |

`--e-3` vaut exactement les 12 px des décalages actuels des bandeaux
(`style.css:31` et suivantes) : la reprise est à l'identique, pas à
l'approximation.

**Rayons** : `--r-1: 4px`, `--r-2: 6px` (la valeur d'aujourd'hui,
`style.css:34`), `--r-3: 10px`, `--r-plein: 999px`. En **pixels** et non en
`rem`, délibérément : un rayon qui grandit avec le réglage de police déforme
les petits éléments.

**Durées** : `--duree-1: 120ms` (survol, focus), `--duree-2: 300ms`
(apparition/disparition d'un bandeau — la valeur d'aujourd'hui,
`style.css:37`). Rien au-delà.

**Épaisseurs de trait** : `--trait: 1px`, `--trait-focus: 2px`.

### 4.5 La palette, et ses contrastes — mesurés, pas estimés

**Décision : treize tokens de couleur par thème.** Les valeurs ci-dessous ont
été choisies puis **vérifiées par calcul WCAG 2.1**, pas l'inverse.

| Rôle | Sombre | Clair |
| --- | --- | --- |
| `--fond-0` (toile) | `#0b0d10` | `#ffffff` |
| `--fond-1` (surface) | `#14171c` | `#f6f7f9` |
| `--fond-2` (surface élevée) | `#1c2027` | `#eceef2` |
| `--bord` (séparateur décoratif) | `#2a2f38` | `#d5dae2` |
| `--bord-fort` (contour de contrôle) | `#6a7484` | `#7d8695` |
| `--texte-fort` | `#e6e8eb` | `#10131a` |
| `--texte` | `#c3c8d0` | `#3d4450` |
| `--texte-faible` | `#8b94a3` | `#5c6675` |
| `--accent` | `#7aa2f7` | `#2f5fd0` |
| `--sur-accent` | `#0b0d10` | `#ffffff` |
| `--succes` | `#6ec08a` | `#1d7a4a` |
| `--alerte` | `#e0b55c` | `#8a5a00` |
| `--danger` | `#f07a7a` | `#c02b2b` |

`--fond-0` sombre et `--texte-fort` sombre sont **exactement** les deux valeurs
déjà en place (`style.css:3-4`) : le thème sombre du produit ne change pas de
teinte, il gagne des voisins.

**Deux tokens hors thème**, déclarés une seule fois et jamais redéfinis :

- `--video-letterbox: #000` — les bandes que laisse `object-fit: contain`
  (`style.css:25`). **Volontairement noir dans les deux thèmes** : un
  encadrement clair autour d'une image vidéo est perçu comme un défaut
  d'affichage, et le contenu de la vidéo ne suit aucun thème. C'est le
  `background: #000` de `style.css:26`, promu en token pour qu'il cesse d'être
  une valeur littérale (§7.2).
- `--voile-flottant: rgb(0 0 0 / 0.72)` — le fond des bandeaux posés sur la
  vidéo (§5.2). Même raison, et **exception nommée** au point 2 de §3.

**Le relevé de contraste** (script de calcul WCAG 2.1 pur, sans dépendance,
lancé le 19 août 2026) :

```
paires vérifiées : 50
échecs : 0
minimum global : 3.16 (clair bord-fort/fond-2)
```

Les 50 paires sont **déclarées**, pas le produit cartésien : sept encres × trois
fonds au seuil **4,5** (texte, WCAG 1.4.3 AA), plus `--bord-fort` sur les trois
fonds au seuil **3** (composant d'interface, WCAG 1.4.11), plus
`--sur-accent` sur `--accent` au seuil 4,5 — le tout ×2 thèmes. Quelques
valeurs, pour situer : `--texte` sur `--fond-0` rend **11,58** en sombre et
**9,81** en clair ; le pire cas *de texte* est `--succes` sur `--fond-2` en
clair, à **4,59**.

⚠️ **`--bord` n'est PAS dans les paires, et c'est une décision** : il rend
**1,45** (sombre) et **1,40** (clair) sur `--fond-0`. Il est réservé aux
séparateurs **purement décoratifs**, que WCAG 1.4.11 exempte explicitement. Dès
qu'une bordure porte une information — le contour d'un champ, l'état d'un
contrôle — c'est `--bord-fort` qui s'applique, et lui est mesuré. **Le coût de
cette décision est qu'aucune commande ne peut vérifier qu'on n'a pas employé
`--bord` là où il fallait `--bord-fort`** : c'est une règle de revue, nommée
comme telle en §8.

### 4.6 Le legacy : ⑥ n'y touche pas, et les contrôles ne le balaient pas

**Décision : `assets/`, `web/`, `src/` et `index.js` sont hors périmètre, et
les contrôles automatisés de §7 sont bornés à `client/src/`.**

*Justification.* Cadrage §11 : « L'ancien code n'est pas modifié ; il reste
fonctionnel jusqu'au remplacement. » Le sous-projet ⑤ prend la même décision
dans les mêmes termes (§9 de sa spec).

*Le coût, qui est réel et qu'il faut dire.* Pendant toute la durée de la
transition, **deux directions visuelles coexistent** dans le même dépôt et
peuvent être servies au même utilisateur : le dégradé violet du legacy
(`assets/index.html:36`) et la palette neutre de ⑥. Personne ne peut confondre
les deux, mais personne ne peut non plus prétendre que le produit a une seule
identité tant que le legacy vit.

*Et pourquoi les contrôles s'arrêtent à `client/src/`.* Un contrôle « aucune
couleur littérale » qui balaierait `assets/` serait **rouge pour toujours**,
pour une raison que ⑥ n'a pas le droit de corriger. Un contrôle qui ne peut
pas devenir vert n'est pas un contrôle : c'est un avertissement permanent que
l'on finit par ignorer.

---

## 5. Le périmètre exact de la fenêtre de session

C'est la surface la plus contrainte du produit, et la seule où la question
« qu'est-ce qui est stylable ? » a une réponse non triviale.

### 5.1 Ce qui n'est PAS stylable, et ne le sera jamais

**Les pixels à l'intérieur de `#remote`.** Ce sont ceux de la fenêtre Windows,
encodés en H.264 par l'agent et décodés par le navigateur. Aucun token, aucune
feuille de style, aucun thème n'a prise dessus. Une bascule de thème du produit
laisse une application Windows claire, claire.

*Corollaire à écrire une fois pour toutes* : le thème du produit et
l'apparence de l'application distante sont **deux choses indépendantes**, et
⑥ ne prétend ni les accorder ni les proposer.

### 5.2 Ce qui est stylable — quatre éléments aujourd'hui, trois familles demain

**Aujourd'hui**, la fenêtre de session contient exactement quatre éléments
(`client/index.html`, lus l. 10-13) :

| Élément | Rôle | Écrit par |
| --- | --- | --- |
| `#remote` | le `<video>` | — ; seules ses **bandes** (`object-fit: contain`) sont stylables, par `--video-letterbox` |
| `#status` | bandeau d'état, haut-gauche | `client/src/status.ts` uniquement — point d'écriture unique |
| `#stats` | compteurs, bas-gauche | `client/src/stats.ts:105` |
| `#fullscreen` | bouton, bas-droite | `client/src/fullscreen.ts` |

**Contrainte propre à ces trois bandeaux** : ils flottent au-dessus d'une image
dont le produit ne contrôle pas le contenu. Aucun token de fond ne peut donc
garantir leur lisibilité — d'où `--voile-flottant` (§4.5), qui est l'exception
nommée au point 2 de §3.

⚠️ **Une contrainte de comportement à ne pas casser, écrite dans le code
d'aujourd'hui** : `#fullscreen[data-actif="true"]` porte
`pointer-events: none`, et le commentaire qui l'accompagne explique qu'une zone
d'environ 40×36 px en bas à droite avalait sinon les clics destinés au jeu.
**Toute reprise du bouton en S4 doit conserver cette propriété** ; changer sa
taille change la zone qu'il occupe, et la justification est écrite en fonction
d'elle.

> ✅ **LA REPRISE A EU LIEU, ET LA PHRASE EST DEVENUE UNE COMMANDE** (S4,
> tâches 7 et 5). `client/src/style.test.ts` exige la déclaration
> `pointer-events: none` sur la règle `#fullscreen[data-actif="true"]`, **ancrée
> sur la règle entière et lue après blanchiment** — un garde qui aurait cherché
> la sous-chaîne aurait été satisfait par le commentaire du fichier qu'il
> analyse. Il a été **écrit et vu ROUGE AVANT** le changement de taille, et
> l'ordre est portant : un garde écrit après ne prouve pas qu'il aurait attrapé
> la régression.
> ⚠️ **Le glyphe est passé de 18 px à `var(--t-xl)` (20 px), et le « 40×36 »
> ci-dessus N'A PAS ÉTÉ RECALCULÉ** : la hauteur se CALCULE (8 + 8 + 20 = 36 px
> à racine 16), la LARGEUR non — elle dépend de l'avance du glyphe `⛶`, que
> personne n'a mesurée. Le commentaire du code cesse donc de la chiffrer plutôt
> que de remplacer une estimation par une autre. **Que 20 px soit la bonne
> taille, et que la zone occupée soit la bonne, sont deux JUGEMENTS HUMAINS**
> (§8), et **aucun n'a été porté**.
> ⚠️ **Les deux numéros de ligne que ce paragraphe portait (`style.css:84` et
> `:79-83`) ont dérivé et sont retirés** : ce dépôt a déjà écrit qu'un numéro de
> ligne recopié survit à la réalité qu'il décrivait.

**Demain**, trois familles s'ajoutent, et aucune n'existe encore :

1. **Les écrans d'erreur et de reconnexion.** Le cadrage les promet (§7 :
   « la PWA affiche "reconnexion…" puis se rattache »). Aujourd'hui **il n'y a
   pas d'écran** : tout passe par le bandeau `#status`, y compris les états
   terminaux — les **deux** appels de `client/src/main.ts` qui passent
   `terminal: true` (`session terminée`, puis `échec : …`). Un état terminal
   occupe donc six lignes de texte dans un bandeau de 12 px de marge. **S4
   livre un écran plein cadre pour les seuls états terminaux**, et laisse les
   messages transitoires au bandeau.
   ✅ **LIVRÉ (S4, tâche 9)** : `client/src/ecran-terminal.ts` (pur,
   dépendances injectées), `client/src/session/etat-terminal.css`, et le
   balisage `#fin` d'`index.html`. **L'écran se branche DANS `creerStatut`**,
   par une seconde cible **optionnelle**, et non par un appelant de plus : ce
   module existe pour qu'« aucun appelant ne puisse oublier la garde », et deux
   écritures côte à côte seraient deux choses que rien n'oblige à rester
   d'accord. **Aucune frontière n'est déplacée** — les onze autres appels de
   `main.ts` restent au bandeau. **Aucune action** n'est posée sur cet écran :
   une action est un comportement de ②, et l'inventer ici la ferait naître sans
   recette. ⚠️ **Que l'écran plein cadre soit la bonne forme, et qu'il doive
   rester sans action, sont deux JUGEMENTS HUMAINS** (§8), non portés.
   ⚠️ **Les deux numéros de ligne (`main.ts:136`, `:405-406`) ont dérivé** et
   sont remplacés par le fait qui, lui, ne dérive pas : il y a EXACTEMENT deux
   appels terminaux. *Ce n'est pas qu'une décision d'esthétique* :
   `status.ts` distingue déjà `terminal` de `persistant` et d'ordinaire, donc la
   frontière existe dans le modèle avant d'exister à l'écran.
2. **Le chrome de fenêtre PWA (Window Controls Overlay).** Stylable par
   `env(titlebar-area-x/y/width/height)` et l'API `navigator.windowControlsOverlay`.
   ⚠️ **Rien de tout cela n'est actif sans un manifest déclarant
   `display_override: ["window-controls-overlay"]`, et le nouveau client n'a
   AUCUN manifest** (§2.1). **⑥ ne livre pas le manifest** — il appartient à ②
   (« PWA par application », cadrage §5 ②). Ce que ⑥ livre est la **règle de
   mise en page** qui s'adapte quand il arrivera : les variables `env(…)`
   valent 0 quand WCO est inactif, donc la feuille est sûre à livrer d'avance.
   *Coût* : cette partie de S4 ne peut **pas** être exercée tant que ② n'a pas
   posé le manifest, et le §9 le déclare comme non établi.
3. **Le sélecteur de thème lui-même.** Il n'a pas sa place dans la fenêtre de
   session (une barre d'outils sur un jeu en plein écran est une régression) :
   il vit sur la page-shell et sur le futur hub. La fenêtre de session **suit**,
   elle ne choisit pas.

---

## 6. Découpage en sous-blocs

| # | Objet | Dépend de |
| --- | --- | --- |
| **S1** | **Le socle : tokens, thèmes, et les contrôles qui les gardent** | — |
| **S2** | Les primitives d'interface | S1 |
| **S3** | Les deux surfaces existantes habillées : page-shell et écran de connexion | S2, et la moitié navigateur de P2 |
| **S4** | La fenêtre de session : état terminal plein cadre, et Window Controls Overlay | S1 ; WCO dépend en outre du manifest de ② |

**Le hub ne figure PAS dans ce découpage.** Il n'existe pas (§2.5, et sous-projet
⑤ §2.4 : « aucun hub ») et son contenu dépend de ④ (catalogue, upload). ⑥ lui
prépare tout — tokens et primitives — et ne le livre pas. §9 le redit.

### 6.1 Ce qui est un socle à poser maintenant, et ce qui s'applique au fil de l'eau

Le cadrage dit « appliqué en continu au fil des sous-projets » (§5 ⑥). Cette
phrase a deux moitiés, et elles n'ont pas le même statut :

- **Le socle (S1) se pose une fois, et maintenant.** Il est immédiatement
  utile — il fait passer la page-shell de « aucune feuille » (§2.2) à
  « la même feuille que la fenêtre de session » — et il est urgent : le
  premier écran de connexion doit naître avec, faute de quoi il naîtra avec
  ses propres couleurs qu'il faudra reprendre. **C'est le seul sous-bloc dont
  l'ordonnancement soit contraint par autre chose que ⑥.**
- **L'application continue est une RÈGLE, pas un sous-bloc**, et une règle
  n'est tenue que si une commande la vérifie. C'est le rôle des contrôles §7.2
  et §7.6, qui tournent dans `scripts/verify-all.sh` : à partir de S1, toute
  surface neuve qui introduirait une couleur littérale casse la vérification
  du dépôt. **Sans ce contrôle, « appliqué en continu » est un vœu.**

### S1 — Le socle : tokens, thèmes, et les contrôles qui les gardent

**Livre** :

- `client/src/design/tokens.css` — les treize couleurs × deux thèmes, les deux
  tokens hors thème, les sept crans typographiques, les huit d'espacement, les
  rayons, durées et traits (§4.4, §4.5) ;
- `client/src/design/base.css` — la remise à zéro et les défauts d'élément
  (`html`, `body`, titres, `:focus-visible`), reprenant `style.css:7-19` en
  l'exprimant en tokens ;
- `client/src/design/theme.ts` + `theme.test.ts` — les trois états, la lecture
  et l'écriture de `localStorage`, l'application locale **et** la réaction à
  l'événement `storage` (§4.2), dépendances injectées, testable sans DOM ;
- `client/src/design/amorce-theme.js` — le script en ligne, source unique ;
- le greffon `transformIndexHtml` dans `client/vite.config.ts`, qui l'injecte
  dans **toutes** les entrées ;
- `client/shell.html` reçoit sa feuille de style — la première fois de sa vie
  (§2.2) ;
- `client/src/style.css` cesse de déclarer des couleurs : il importe la couche
  design et ne garde que la mise en page de la fenêtre de session ;
- `client/design.html` — la **galerie**, quatrième entrée Vite : une page
  statique qui rend chaque token et, plus tard, chaque primitive, dans les deux
  thèmes. C'est l'instrument du jugement humain de §8, et il est explicitement
  **exclu** du contrôle §7.6 ;
- les scripts de contrôle sous `client/outils/` (§7), et leur branchement dans
  `client/package.json` puis dans `scripts/verify-all.sh`.

**Ce que S1 ne fait PAS** : aucune primitive, aucun changement d'apparence des
quatre éléments de la fenêtre de session au-delà de leur passage aux tokens.
**S1 doit être visuellement quasi neutre sur `index.html`** — c'est ce qui rend
sa recette lisible : si l'apparence de la session change en S1, c'est qu'une
valeur a été perdue au passage.

### S2 — Les primitives d'interface

**Livre** quatre familles, et rien d'autre : **bouton** (principal, secondaire,
discret ; états survol/actif/désactivé/focus), **champ** (texte, mot de passe,
avec étiquette, aide et erreur), **surface** (carte, séparateur), **message**
(les quatre tons : neutre, succès, alerte, danger).

Ce sont exactement les pièces dont un écran de connexion a besoin — c'est le
critère qui borne la liste, et non un catalogue *a priori*.

**Découpage prévu d'avance** (§10) : `primitives.css` naît d'un seul tenant ;
dès qu'il atteint **300 lignes**, il se scinde en `primitives/bouton.css`,
`primitives/champ.css`, etc. Le seuil est à 300 et non à 500 pour que la
scission soit décidée **avant** l'addition qui la rendrait obligatoire — c'est
la leçon que ce dépôt a payée trois fois en D9 et D10 (« extraire n'est pas
compresser »).

### S3 — Les deux surfaces existantes habillées

**Livre** : la page-shell (`shell.html` + `shell-page.ts`) portée sur les
primitives — sa liste de fenêtres devient une liste de cartes avec leur état et
leur action ; et **l'écran de connexion de P2**, habillé.

⚠️ **S3 dépend d'un travail qui n'est pas le sien** : la moitié navigateur de
P2 n'existe pas (§2.5). Deux ordonnancements sont possibles, et le choix est
énoncé plutôt que subi :

- **retenu** — S1 et S2 précèdent l'écran de connexion, qui naît habillé. Coût :
  P2 attend S2.
- **écarté** — l'écran de connexion naît nu et S3 le reprend. Coût : deux
  passes sur le même fichier, et une fenêtre pendant laquelle le seul écran du
  produit contredit sa direction visuelle.

Si l'ordonnancement retenu s'avère impossible (P2 pressé par ailleurs), S3 se
scinde : la page-shell d'abord, l'écran de connexion quand il existe. **Ce qui
ne doit pas arriver, c'est que l'écran de connexion introduise ses propres
couleurs** — et c'est le contrôle §7.2 qui l'empêche, pas la discipline.

### S4 — La fenêtre de session

**Livre** : l'écran plein cadre des états **terminaux** (§5.2, famille 1), en
réemployant la distinction que `status.ts` porte déjà ; la reprise des trois
bandeaux sur les primitives « message » ; et la mise en page conditionnelle au
Window Controls Overlay.

⚠️ **La partie WCO est livrée sans pouvoir être exercée** tant que ② n'a pas
posé de manifest. Elle est sûre (les `env(…)` valent 0 sans WCO) et **non
mesurée** — §9.

> ✅ **S4 EST LIVRÉ (20 août 2026)**, et trois précisions que cette page ne
> pouvait pas porter :
>
> - **la reprise des bandeaux a demandé une VARIANTE, pas seulement une
>   classe.** `.message` pose `background: var(--fond-1)`, un fond **opaque qui
>   suit le thème** : posée nue sur `#status`, elle mettrait en thème clair un
>   panneau clair et opaque au-dessus de la vidéo — exactement ce que le §4.5
>   interdit en une phrase, quand le §5.2 exige `--voile-flottant` sur ces
>   mêmes bandeaux. **Les deux exigences de cette spécification se
>   contredisaient**, et `.message--flottant` les tient toutes les deux.
> - **un défaut d'encre a été trouvé et réparé, qu'aucun document de ⑥ ne
>   portait** : sous le thème clair, les éléments de la fenêtre de session
>   écrivaient du quasi-noir sur un voile quasi-noir, effet de bord de S1 qui
>   avait donné un thème clair à une surface qui n'en avait pas. D'où
>   `--sur-voile`, **septième** hors-thème, et la **53ᵉ** paire de contraste.
> - 🔴 **le WCO n'a AUCUN critère de recette, et c'est délibéré.** Il n'existe
>   aucun manifeste dans ce dépôt, donc **aucun état atteignable** où la règle
>   agisse : un critère qui prétendrait l'exercer serait vacueux **par
>   construction**, et un critère vacueux est pire qu'un critère absent — il se
>   lit comme une preuve. Ce qui est livré à la place est un **garde de forme**
>   (`client/src/style.test.ts`) à trois assertions — repli `0px` obligatoire,
>   **aucune** `@media (display-mode: window-controls-overlay)`, et
>   atteignabilité — **dont la rouge a une conséquence RÉELLE aujourd'hui** :
>   un repli non nul descend le bandeau maintenant. **Il prouve l'inertie,
>   jamais le comportement.** Destinataire nommé du legs : la **recette de ④
>   G5**, celle qui pose le manifeste.

---

## 7. Les contrôles automatisés — et la preuve que chacun peut échouer

Sept contrôles. Pour chacun : ce qu'il vérifie, la commande, et **l'état qui le
rend ROUGE**. Trois sont **rouges aujourd'hui sans rien casser** — ce n'est pas
une hypothèse, c'est le relevé du §2.

> ✅ **ILS SONT NEUF DEPUIS LE SOUS-BLOC S4 (20 août 2026), et les deux derniers
> ne sont pas de cette spécification : ce sont des ADDITIONS DE PLAN, déclarées
> comme telles.** Voir le **§7.9** (S3) et le **§7.10** (S4) ci-dessous. La
> phrase « sept contrôles » reste vraie comme HISTOIRE — c'est ce que ⑥
> prévoyait — et elle est fausse au présent.
> ⚠️ **Le nombre à ne pas confondre** : `npm run design:verifier` en lance
> **SEPT**, parce que **deux** des neuf sont des tests unitaires qui tournent
> dans `npm test` — le §7.5 et le §7.10. **Sept scripts, neuf contrôles**, et
> ni l'un ni l'autre ne se suffit sans dire lequel on compte.
> ❌ « ILS SONT HUIT DEPUIS LE SOUS-BLOC S3 » : vrai à sa date, réfuté par S4.

### 7.1 Les contrastes tiennent les seuils WCAG

**Comment.** `client/outils/contraste.mjs` **parse `tokens.css`** — il ne
recopie aucune valeur — extrait les couleurs de chaque bloc de thème, et
évalue les **50 paires déclarées** (§4.5). Sortie : le nombre de paires, le
nombre d'échecs, le minimum global. Sortie non nulle si un échec.

Que le contrôle lise la source de vérité plutôt qu'une table jumelle est **le**
point de conception : une table jumelle est exactement ce que §4.1 refuse
ailleurs, et un contrôle qui a sa propre copie des valeurs valide sa copie.

**Relevé du 19 août 2026** sur la palette de §4.5 :
`paires vérifiées : 50 / échecs : 0 / minimum global : 3.16`.

**ROUGE — exercé, pas supposé.** En remplaçant le seul `--texte-faible` clair
(`#5c6675`) par `#7c8697`, le même script rend :

```
ÉCHEC clair texte-faible/fond-0 = 3.68 < 4.5
ÉCHEC clair texte-faible/fond-1 = 3.43 < 4.5
ÉCHEC clair texte-faible/fond-2 = 3.16 < 4.5
paires vérifiées : 50
échecs : 3
```

Une perturbation d'un token suffit. Le contrôle discrimine.

⚠️ **Ce qu'il ne vérifie PAS** : que le bon token a été employé au bon endroit
(§4.5, dernier paragraphe), ni les couleurs composées à l'exécution, ni ce qui
est posé au-dessus de la vidéo — dont le fond n'est pas connaissable.

### 7.2 Aucune couleur littérale hors de `tokens.css`

**Comment.** Balayage de `client/src/**/*.{css,ts}` **moins** `tokens.css`,
cherchant `#rgb`/`#rrggbb`/`#rrggbbaa`, `rgb(`, `rgba(`, `hsl(`, `hsla(` et les
mots-clés `black`/`white`/`red`. Autorisés : `currentColor`, `transparent`,
`inherit`, `none`, et `var(--…)`.

**ROUGE aujourd'hui, mesuré (§2.4)** : cinq occurrences —
`style.css:26,35,51,68,73`. Le contrôle passe au vert quand S1 les a promues en
`--video-letterbox` et `--voile-flottant`.

### 7.3 Toute surface bâtie porte les tokens

C'est **le** contrôle que le cadrage demande quand il exige des tokens
« partagés entre hub et fenêtre de session ». Une convention ne garantit rien ;
une commande, si.

**Comment.** Après `npm run build` : chaque `dist/*.html` doit référencer une
feuille de style, et l'ensemble des CSS émis doit contenir la déclaration
`--fond-0`. Deux assertions distinctes — la présence du lien **et** la présence
du token —, parce qu'une feuille vide passerait la première.

**ROUGE aujourd'hui, mesuré (§2.2)** : `dist/shell.html` ne porte aucune
feuille.

*Portée honnête* : le contrôle vérifie que la surface **charge** les tokens,
pas qu'elle les **emploie**. Une page qui chargerait la feuille et écrirait ses
propres couleurs passerait §7.3 — et tomberait sur §7.2. Les deux se
complètent ; aucun ne suffit.

### 7.4 Les trois blocs de thème déclarent le même ensemble de noms

**Comment.** Extraction des noms `--*` de chacun des trois blocs (`:root`, la
requête média, `[data-theme="clair"]`) et **égalité d'ensembles**, dans les
deux sens.

**ROUGE.** Retirer un token d'un seul bloc, ou en ajouter un à un seul.
*C'est le mode de défaillance réel du choix de §4.2* : la palette claire y est
écrite deux fois, et rien d'autre que ce contrôle n'empêche les deux copies de
diverger.

> ⚠️ **CE QUI EST ÉCRIT CI-DESSUS N'EST PLUS L'ÉNONCÉ DU CONTRÔLE, et le titre
> de ce §7.4 non plus** — S3 (tâche 1) a ÉLARGI sa portée, sur un trou que S2
> avait mesuré et laissé ouvert. « Égalité d'ensembles dans les deux sens » n'a
> jamais été ce qu'il faisait : il comparait **clair ⇄ clair** et
> **clair ⊆ racine**, jamais **racine ⊆ clair** — si bien qu'un token de couleur
> retiré des DEUX blocs clairs et laissé à la racine passait, 48 contre 13, zéro
> écart (mesuré par S2). **L'énoncé exact est désormais** : ① clair ≡ clair,
> ② clair ⊆ racine, ③ **les COULEURS de racine ⊆ clair**, sauf six tokens hors
> thème nommés. ⚠️ **La clause ③ est restreinte aux COULEURS à dessein** : les
> échelles (espacement, typographie, rayons, durées) ne vivent que dans la
> racine et n'ont pas de contrepartie claire. La rouge de la clause ③ est versée
> — `journaux-design-s3/rouge-1-7-4-couleur-hors-clair.log`.

### 7.5 La bascule de thème atteint les N fenêtres, y compris celle qui l'a demandée

**Comment.** Tests unitaires sur `theme.ts`, dépendances injectées, **deux
assertions distinctes** :

1. **la fenêtre écrivante** — appeler `choisir('clair')` écrit la clé **et**
   pose `data-theme` sur la cible locale ;
2. **une fenêtre voisine** — recevoir un événement `storage` portant la clé
   `theme` pose `data-theme`, sans écrire quoi que ce soit en retour.

Plus deux cas de robustesse : un `storage` sur une **autre** clé ne fait rien ;
une valeur inconnue en `localStorage` retombe sur `systeme` au lieu de poser un
attribut absurde.

**ROUGE.** Supprimer l'application locale de (1) — le défaut naturel de ce
mécanisme, puisque `storage` ne se déclenche pas chez l'écrivain (§4.2) — laisse
(2) vert et fait tomber (1). **Les deux assertions sont séparées précisément
pour que ce défaut-là ne puisse pas se cacher derrière l'autre.**

⚠️ **Ce que ce contrôle ne prouve PAS** : que le navigateur déclenche bien
`storage` entre deux fenêtres réelles. Il éprouve **notre** gestionnaire, pas
la plateforme. Une confirmation manuelle sur deux fenêtres ouvertes est prévue
**hors critère**, en corroboration — même statut que les confirmations sur VM
réelle du sous-projet ⑤ (§4).

### 7.6 Aucun token orphelin, aucun `var(--…)` non déclaré

**Comment.** Deux inclusions, dans les deux sens, entre l'ensemble des tokens
déclarés dans `tokens.css` et l'ensemble des `var(--…)` référencés dans les
**feuilles de production**.

⚠️ **La galerie `client/design.html` est EXCLUE de la moitié « employé »**, et
c'est essentiel : la galerie rend tous les tokens par construction, donc
l'inclure rendrait ce contrôle **incapable d'échouer** — il validerait la
galerie et rien d'autre. C'est la classe de défaut dont ce dépôt a attrapé
quatre exemplaires sur le seul sous-bloc D10.

**ROUGE.** Une faute de frappe dans un `var(--fond-O)` tombe sur la seconde
inclusion ; un token déclaré et jamais employé tombe sur la première.
❌ « **le cas prévu de `--police-mono`, §4.3** » : ce n'est plus un cas prévu —
il est **câblé** depuis le sous-bloc S4 (tâche 6), et la liste d'attente est
vide. La rouge que le plan de S4 prescrivait pour ce sens — retirer
`font-family: var(--police-mono)` de `#stats`, attendu `NOUVEL ORPHELIN
--police-mono` — n'a **aucun journal versé** : ⚠️ **les rouges des tâches 1 à 8
de S4 ne sont rapportées que par leurs messages de commit**, contrairement à
celles des tâches 9 et 10 (`journaux-design-s4/rouges-t9.log`, `rouges-t10.log`).

### 7.7 Le poids ne dérive pas

**Comment.** Après `npm run build`, la somme des `dist/assets/*.css` doit
rester sous un plafond déclaré. **Ligne de base mesurée : 1 055 octets**
(§2.3). **Plafond posé à 12 288 octets (12 Kio)** — un peu plus de onze fois la
base, ce qui laisse la place à quatre familles de primitives sans laisser
passer une police embarquée par inadvertance (§4.3).

**ROUGE.** Ajouter un `@font-face` avec une police en `data:` URI, ou importer
une bibliothèque CSS tierce.

⚠️ **Ce plafond est ARBITRAIRE, et il est déclaré tel.** Il n'est adossé à
aucune mesure de performance : c'est un garde-fou contre une addition massive,
pas une cible de budget. Il rejoint la liste du §8.

### 7.9 Une primitive atteint réellement une surface du produit — ADDITION DE S3

> 🔴 **CE CONTRÔLE N'ÉTAIT PAS PRÉVU PAR CETTE SPÉCIFICATION.** Il est ajouté
> par le sous-bloc S3 (tâche 2), et il est inscrit ici parce qu'un contrôle qui
> ne vit que dans un plan de sous-bloc se perd. **Numéroté 7.9 et non 7.8** :
> le §7.8 existe déjà et dit ce qui n'est PAS un contrôle.

**Comment.** `client/outils/classes-employees.mjs`. Trois assertions :
① toute classe employée par une surface ou un module est **déclarée** par une
feuille ou un `<style>` en ligne ; ② **au moins une famille de primitives est
employée par une surface du PRODUIT** (`index.html`, `shell.html`,
`connexion.html`) ; ③ **chaque famille est rendue par la galerie**
`primitives.html`. Les familles sont **dérivées** des fichiers de
`src/design/primitives/`, jamais énumérées : une cinquième famille entre dans
② et ③ sans qu'une ligne du script ne change.

**Pourquoi.** Aucun des sept autres ne peut voir l'écart que S2 a lui-même
nommé — *« les dix-huit tokens sortis de la liste ont un appelant ÉCRIT, pas un
pixel RENDU »*. §7.6 compte des `var(--…)` dans des **fichiers**, §7.3 ne
vérifie qu'un **chargement**, §7.2 ne voit que des couleurs.

**ROUGE.** Retirer les classes de primitive des **deux** surfaces habillées
(assertion ②) ; employer une classe que rien ne déclare (①) ; retirer une
famille de la galerie (③). ⚠️ **L'assertion ② A a été ROUGE SUR L'ARBRE INTACT**
entre les tâches 2 et 4 de S3 — c'est sa preuve d'atteignabilité, et non un
raisonnement. Les trois rouges sont versées sous
`docs/superpowers/plans/journaux-design-s3/`.

**Ce qu'il NE fait PAS.** Il n'a pas le sens inverse — *toute classe déclarée
est employée* —, qui exigerait une seconde liste d'attente. Il relève
l'écart (52 déclarées / 51 employées à la fin de S3) **sans le juger**.

### 7.10 Aucune longueur hors token dans une feuille de surface — ADDITION DE S4

🔴 **CE CONTRÔLE N'EST PAS DE CETTE SPÉCIFICATION : c'est une ADDITION DU PLAN
DE S4**, inscrite ici par sa revue transverse, « parce qu'un contrôle qui ne vit
que dans un plan de sous-bloc se perd ». Même statut que le §7.9 ci-dessus.

**Pourquoi.** La clause « aucune longueur hors échelle » du §8 était une **dette
d'énoncé** : elle était fausse depuis S1, nommée comme telle, et **invérifiable**
— « aucun des huit contrôles ne mesure une longueur » était écrit à cinq endroits
du dépôt sous cette formule exacte, et davantage sous d'autres tournures. §7.10
est **le premier contrôle de ⑥ qui en mesure une**.

**Comment.** `client/src/design/longueurs.test.ts`, **test unitaire** (comme le
§7.5), lancé par `npm test` **depuis `client/`**. Il lit les feuilles de
**surface** — portée **DÉRIVÉE, jamais énumérée** : toutes les `*.css` de
`client/src/` **hors** `client/src/design/`. Une feuille de surface neuve y entre
sans qu'une ligne du contrôle ne change. Il blanchit les commentaires, découpe en
déclarations, et refuse toute valeur dimensionnée qui ne passe pas par un
`var(--…)`. **Trois exceptions closes, chacune avec sa raison** : les
remplissages de fenêtre (`100vw`, `100vh`, `100dvh`), le **zéro** (aucune échelle
n'a de cran nul, et le repli des `env(titlebar-area-*)` du WCO en porte un par
construction), et `tokens.css` — dont les littéraux **sont** l'échelle, et qui
est de toute façon hors de la portée dérivée.

**Sortie, toujours imprimée, succès compris** : le nombre de feuilles, de
déclarations lues, celles qui passent par un token, puis **deux nombres** — les
**occurrences** et les **valeurs distinctes**. Les deux, jamais un seul : ils sont
vrais de choses différentes, un contrôle ne pouvant pas dédupliquer sans décider
que deux `6px` écrits à deux endroits sont le même.

**ROUGE.** Il est **né rouge sur l'arbre intact** — **8 occurrences pour 6
valeurs** —, et c'est sa preuve d'atteignabilité. Il porte en outre son
**assertion d'atteignabilité** : un contrôle d'absence est vert sur des fichiers
vides. ⚠️ **Sa rouge d'atteignabilité doit vider LES TROIS feuilles de surface**,
pas une seule : la portée étant dérivée, vider `style.css` seule laisse les deux
autres porter leurs déclarations, et l'assertion reste verte à juste titre.

**Ce qu'il NE dit PAS** — et c'est **exactement la limite de G4** : *que le BON
token a été choisi.* `padding: var(--e-8)` sur un bandeau serait vert et absurde.
Le bon emploi reste une **règle de revue**. Il ne voit pas non plus une longueur
calculée à l'exécution (`el.style.padding = …`), même angle mort que §7.9 pour
les classes, et même parade : la convention est d'écrire les longueurs dans le
CSS.

### 7.8 Ce qui n'est PAS un contrôle de ⑥, et pourquoi

**La comparaison d'images de référence est ÉCARTÉE pour S1 à S4.** Trois
raisons, dont la première est éliminatoire :

1. **Les polices système rendent différemment d'une machine à l'autre** (§4.3,
   coût assumé). Une référence prise sur un poste échouerait sur le suivant,
   pour une raison qui n'est pas un défaut. Il faudrait embarquer une police
   **pour rendre la recette possible** — c'est-à-dire payer le coût que §4.3
   refuse, au bénéfice de l'instrument et non du produit.
2. `client/package.json:14-17` ne déclare aucun pilote de navigateur ;
   l'ajouter est une dépendance lourde pour ce seul usage.
3. La fenêtre de session porte un `<video>` **animé** : toute capture y est
   non déterministe. Ce dépôt a déjà relevé qu'un contrôle de distinction
   d'images « ne peut pas échouer sur une page vivante » (D10, critère ②).

**Ce qui remplace** : la galerie (§6, S1), regardée par un humain. C'est un
**jugement**, il est déclaré comme tel en §8, et il n'est pas déguisé en
mesure.

---

## 8. Ce qui relève du JUGEMENT HUMAIN, et qui n'est vérifié par aucune commande

Ce dépôt nomme ses constantes non calibrées plutôt que de les laisser passer
pour des mesures. Les suivantes sont dans cet état, **par construction et non
par négligence** :

> 🔴 **ILS SONT VINGT-CINQ À LA FIN DU SOUS-PROJET, ET AUCUN N'A ÉTÉ PORTÉ.**
> La table ci-dessous en porte **huit** ; S2 en a ajouté **trois**, S3 **quatre**
> (quinze à sa fin, relevé par son propre document de résultats), et S4 **dix** —
> ÉNUMÉRÉS, pas recopiés, chacun avec le fichier qui le déclare :
> `--police-mono` plus lisible que le crénage (`style.css`) ; 20 px la bonne
> taille des boutons de coin (`style.css`) ; `#e6e8eb` la bonne encre sur un
> voile (`design/contraste.ts`) ; les trois mesures de contenant méritent des
> tokens (`design/tokens.css`) ; le rayon 6 → 4 px acceptable (`style.css`) ;
> **la zone occupée** par le bouton, distincte de sa taille (`style.test.ts`) ;
> `--e-8` reste la bonne marge du micro après ce changement (`style.css`) ;
> l'écran plein cadre est la bonne forme, et il doit rester **sans action**
> (`ecran-terminal.ts`, deux jugements) ; et les deux **libellés de titre** de
> cet écran (`ecran-terminal.ts`).
> ⚠️ **Les trois derniers de S4 n'étaient PAS prévus par son plan** — qui en
> annonçait sept — et **le taire les aurait déguisés en mesures**.
>
> 🔴 **ET LE JUGEMENT VISUEL N'A JAMAIS ÉTÉ PORTÉ SUR ⑥, D'UN BOUT À L'AUTRE :
> aucune page du sous-projet n'a été ouverte dans un navigateur, ni en S1, ni en
> S2, ni en S3, ni en S4.** La galerie existe pour cela, et personne ne l'a
> regardée. S4 était la dernière occasion, et **il la laisse passer en le
> déclarant** — un agent qui prendrait une capture d'écran ne porterait pas un
> jugement, il produirait une image que personne n'a regardée.

| Ce qui n'est pas mesuré | Ce qui est mesuré à la place |
| --- | --- |
| que la direction soit « sobre » et « pro » (cadrage §3) | rien. C'est l'objet même du sous-projet, et c'est un goût |
| le **choix des teintes** — que `#7aa2f7` soit le bon bleu | leur **contraste** (§7.1). Une centaine d'autres bleus passeraient les mêmes seuils |
| que le ratio **1,2** de l'échelle typographique soit le bon | que l'échelle ait sept crans nommés et un plancher dur à 11 px |
| que **14 px** soit assez dense sans être trop petit | qu'il soit exprimé en `rem` sur une racine libre, donc réglable (§4.4) |
| que le pas de **4 px** d'espacement soit le bon | qu'il soit unique — et, **depuis le sous-bloc S4, que la clause « aucune longueur hors échelle » soit TENUE ET MESURÉE** par le contrôle §7.10 : 0 occurrence, 0 valeur. ~~cette clause est FAUSSE de SIX valeurs~~ **voir l'encadré ci-dessous** |
| que `--bord` ait été employé là où il fallait, et non `--bord-fort` | que `--bord-fort` tienne 3:1 (§7.1). **Le bon emploi est une règle de revue** |
| que le **plafond de 12 Kio** (§7.7) soit au bon endroit | la ligne de base, 1 055 octets, elle mesurée |
| que la **galerie** montre ce qu'il faut regarder | qu'elle existe et qu'elle soit exclue de §7.6 |

> ✅ **CETTE CLAUSE EST TENUE DEPUIS LE SOUS-BLOC S4, ET ELLE EST DEVENUE UNE
> COMMANDE** (20 août 2026). Le contrôle **§7.10**
> (`client/src/design/longueurs.test.ts`, addition de plan) mesure les longueurs
> des feuilles de SURFACE. Il est **né ROUGE de HUIT OCCURRENCES pour SIX
> VALEURS** sur l'arbre intact — les deux nombres sont vrais de choses
> différentes, un contrôle comptant des occurrences là où S3 comptait des
> valeurs — et les tâches 5 à 8 de S4 les ont **toutes** fait tomber : le relevé
> de recette rend **0 occurrence, 0 valeur** (`journaux-design-s4/recette-1.log`
> et `recette-2.log`, deux exécutions).
> ⚠️ **CE QU'IL NE DIT TOUJOURS PAS : que le BON token a été choisi.**
> `padding: var(--e-8)` sur un bandeau serait vert et absurde. C'est la limite
> exacte de G4, et elle ne bouge pas — **le bon emploi reste une règle de
> revue**, comme celui de `--bord` contre `--bord-fort`.
> ⚠️ **Et sa portée exclut `client/src/design/`** : `tokens.css` est son
> exception ③ nommée (ses littéraux SONT l'échelle), et les quatre familles de
> primitives restent gardées par G4.
>
> ❌ **CE QUI SUIT ÉTAIT LE RELEVÉ DE S3, ET IL EST RÉFUTÉ PAR S4 — conservé
> parce qu'il reste vrai à sa date, et parce qu'il nomme les six.**
>
> ❌ **LA CLAUSE « aucune longueur hors échelle » EST FAUSSE, ET DE SIX
> VALEURS** (relevé par la revue transverse de S3, 20 août 2026 ; journal
> `journaux-design-s3/longueurs-hors-echelle.log`). **Elle l'était déjà de
> trois à la fin de S1**, ce que l'en-tête de `client/src/style.css` déclare
> depuis, et **S3 en ajoute trois** en habillant deux surfaces :
>
> | Feuille | Valeurs hors échelle |
> | --- | --- |
> | `client/src/style.css` (fenêtre de session, S1) | `6px`, `18px`, `0.02em` |
> | `client/src/shell.css` (S3) | `72rem`, `18rem` |
> | `client/src/connexion.css` (S3) | `26rem` |
>
> ⚠️ **LA RÈGLE DE COMPTE EST ÉNONCÉE AVANT LE COMPTE, et c'est celle que
> `style.css` applique depuis S1** : un **remplissage de fenêtre** n'est pas une
> valeur hors échelle — `style.css` ne compte ni son `100vw` ni son `100vh`, et
> `100dvh` de `connexion.css` est exclu de la même façon. **Si on les comptait,
> ce serait NEUF.** ⚠️ **Un énoncé intermédiaire de S3 a publié CINQ** (en-tête
> de `shell.css`, tâche 4) : il était juste à sa date, `connexion.css`
> n'existant pas encore, et la tâche 5 ne l'a pas repris. Corrigé à sa place.
>
> ⚠️ **AUCUN CONTRÔLE NE MESURE UNE LONGUEUR** — le §7 n'en a aucun qui le
> puisse. Le seul garde est **G4** de `client/src/design/primitives.test.ts`,
> et il ne porte que sur `primitives.css` : il vérifie qu'une longueur passe par
> un token, **jamais que le bon token a été choisi**. C'est une dette d'ÉNONCÉ,
> nommée ; elle se solde au sous-bloc qui aura le droit de toucher ces
> éléments — **S4** pour les trois de `style.css`.

**Aucune de ces lignes ne se transformera en mesure au fil des sous-blocs.**
Les déclarer ici est la seule chose honnête à en faire.

---

## 9. Ce que ⑥ n'établira PAS

- **Aucun taux, nulle part.** Ce sous-projet n'a pas de recette sur VM ; ses
  contrôles sont déterministes et se rejouent à volonté. La question « combien
  de fois sur combien » ne se pose pas — et **ne doit pas être empruntée** à
  une campagne qui, elle, l'aurait posée.
- **Le hub visuel.** Il n'existe pas (§2.5), son contenu dépend de ④, et ⑥ ne
  le livre pas. Il en prépare la matière ; c'est tout.
- **Le manifest PWA, `theme-color`, les icônes.** Ils appartiennent à ②. Sans
  eux, **la partie Window Controls Overlay de S4 est livrée non exercée** — elle
  est sûre, pas mesurée.
- **La bascule de thème entre deux fenêtres réelles** : éprouvée sur notre
  gestionnaire (§7.5), pas sur le navigateur.
- **Le rendu sur autre chose qu'un Chromium de bureau.** Le produit vise
  Chromium (cadrage §5 ④ : « ni Firefox ni Safari » pour les file handlers) ;
  ⑥ n'éprouve rien ailleurs, et ne prétend rien de Firefox, Safari ou mobile.
- **L'accessibilité au-delà du contraste** : navigation clavier complète,
  lecteurs d'écran, `prefers-reduced-motion`, cibles tactiles minimales. Le
  contraste est mesuré ; le reste ne l'est pas. *`prefers-reduced-motion` est le
  moins cher des quatre et le premier à prendre* — nommé, non pris.
- **Le mode compact / la densité réglable.** L'échelle est fixe.
- **Toute internationalisation** : le produit est en français, sans mécanisme
  de traduction, et ⑥ n'en introduit pas. Aucune vérification que la mise en
  page survit à une langue plus longue.
- **Le rendu HiDPI**, qui traîne comme legs non exercé depuis D9/D10 : ⑥ pose
  des longueurs relatives, il ne mesure rien à `devicePixelRatio > 1`.
- **Toute modification du legacy** (`assets/`, `web/`, `src/`, `index.js`) —
  cadrage §11, §4.6 ci-dessus.
- **Ce qui est à l'intérieur du `<video>`** (§5.1), définitivement.

---

## 10. Taille des fichiers, prévue à la conception

La règle des 500 lignes de `CLAUDE.md` couvre `client/src/`, et sa commande de
vérification ne filtre pas par extension : **une feuille de style y est
soumise comme un module TypeScript**. Relevé du 19 août 2026, portée `client/` :
le plus gros fichier est `client/verify-webrtc.mjs` à **497** lignes (marge
**3** — la marge la plus serrée du dépôt après `agent/src/encode/arret.rs`,
déjà signalée par `CLAUDE.md` et **intouchée par ⑥**), puis
`client/src/main.ts` à **408**. `client/src/style.css` en fait **85**.

Tailles prévues, et le point de chute de chacune si elle est dépassée :

| Fichier | Prévu | S'il grossit |
| --- | --- | --- |
| `design/tokens.css` | ~180 | scinder en `tokens/couleurs.css` et `tokens/echelles.css` |
| `design/base.css` | ~90 | — |
| `design/primitives.css` (S2) | ~250 | **seuil d'action à 300** : un fichier par famille (§6, S2) |
| `design/theme.ts` | ~90 | — |
| `style.css` (fenêtre de session) | ~120 après S1 | ✅ **FAIT (S4, tâche 9)** : `client/src/session/etat-terminal.css` existe, et l'extraction a été faite **AVANT** l'addition, pas après |
| `design.html` (galerie) | ~150 | scinder par famille, au même rythme que les primitives |

**Aucun fichier neuf ne naît au-dessus de 500 lignes**, et le seuil d'action
des primitives est posé **à 300** délibérément : ce dépôt a franchi le plafond
trois fois en D10 et deux fois en D9, et l'a chaque fois rattrapé par une
extraction faite **après** — dont deux par une compression que `CLAUDE.md`
interdit nommément. Décider la scission à 300 la rend possible **avant**
l'addition qui la rendrait urgente.

---

## 11. Risques

| Risque | Mitigation, ou déclaration |
| --- | --- |
| **La palette est jolie sur la galerie et illisible sur une vraie interface** | La galerie est un instrument, pas une preuve. S3 est le premier test réel, et c'est pourquoi il vient tôt |
| **`--voile-flottant` ne suffit pas sur une vidéo très claire** | Non mesuré. Aucun fond ne peut le garantir (§5.2). Un contour ou une ombre de texte serait le remède ; il n'est pas pris ici |
| **S3 attend P2, qui attend autre chose** | §6, S3 : le repli est écrit — la page-shell d'abord, l'écran de connexion quand il existe |
| **Le greffon Vite d'injection casse une entrée** | Le contrôle §7.3 balaie **toutes** les entrées bâties, pas une |
| **Le contrôle §7.2 devient pénible et on l'assouplit** | C'est le mode de défaillance réel d'un contrôle de style. Il est borné à `client/src/` (§4.6) précisément pour rester atteignable, et il est vert dès la fin de S1 |
| **Une police système absente change la mise en page** | Assumé (§4.3), et c'est la raison pour laquelle §7.8 écarte la comparaison d'images |
| **Le thème sombre par défaut surprend** | Décision motivée (§4.2), réversible par un utilisateur en un clic dès S3 |
| **Le legacy et le nouveau produit coexistent visuellement** | Assumé et daté (§4.6) : cadrage §11, jusqu'au remplacement |
| **Un token est ajouté sans être ajouté aux trois blocs de thème** | §7.4, égalité d'ensembles dans les deux sens |
