# Lot 32M — la souris ne pointe pas là où l'image le montre, et le cadrage est une décision

Date : 30 août 2026. Branche `package-nivuus`.

🔴 **AUCUN REMÈDE, AUCUN REDÉPLOIEMENT, ET AUCUNE RELANCE.** Cette manche est
**entièrement en lecture seule** — journal, code, et rien d'autre. Le
propriétaire se servait du système pendant tout le diagnostic : **l'agent n'a
pas été redémarré une seule fois.**

---

## 0. 🔵 LE PREMIER JUGEMENT VISUEL DE CE DÉPÔT

**Une image de la VM est arrivée dans un navigateur et un humain l'a jugée
FLUIDE** — le propriétaire, sur `app.allanic.me`, en production, le 30 août
2026.

C'est le premier jugement d'usage porté dans ce dépôt, **tous chantiers
confondus**. Le legs « personne n'a jamais regardé une image » se referme
**en partie**.

⚠️ **Ce qu'il n'établit PAS** : ni la **latence de bout en bout**, ni la
justesse des couleurs, ni aucune des vingt-cinq attentes de jugement du
sous-projet ⑥. **Fluide** est ce qui a été jugé, et rien d'autre.

---

## 1. Les quatre symptômes du propriétaire

1. Notepad lancé depuis le hub : il voit **le fond d'écran et la barre des
   tâches**, pas Notepad.
2. Un **clic** ne produit rien — y compris quand Notepad est visible.
3. Après un aller-retour **plein écran du navigateur**, Notepad **paraît**.
4. **Redimensionner la fenêtre PWA ne redimensionne pas Notepad.**
5. 🔴 **Le CLAVIER fonctionne. Seule la souris ne fait rien.**

---

## 2. ② LA SOURIS — hypothèse principale, et elle explique le partage clavier/souris

### 2.1 Ce que le transport fait : il MARCHE

| Relevé (journal, lecture seule) | Valeur |
| --- | --- |
| `canal de données ouvert label=input` | présent |
| `premier plan obtenu avant injection clavier hwnd=HWND(0x180360)` | **1** |
| `SendInput a refusé l'entrée` | **0** |
| session des processus `agent.exe` | **1** (tous) |

🔵 **Le chemin d'entrée est VIVANT** : les messages arrivent, `SendInput` est
appelé, il ne refuse jamais, et personne n'est en session 0. **Toute la
famille « l'injection est morte » est éliminée.**

### 2.2 🔴 L'hypothèse du coordinateur est RÉFUTÉE PAR LE CODE

L'hypothèse donnée était : une entrée souris absolue serait normalisée sur
l'écran **principal** faute de déclarer le bureau virtuel. **Faux ici** —
`agent/src/input.rs` :

```rust
dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
```

et l'origine vient bien de `SM_XVIRTUALSCREEN` / `SM_CXVIRTUALSCREEN`. **Le
drapeau est là, la normalisation est celle du bureau virtuel.**

### 2.3 🔴 CE QU'IL Y A À LA PLACE — une PRÉMISSE devenue fausse

`move_mouse` mappe les coordonnées normalisées du navigateur sur
**`client_rect_on_screen(self.hwnd)`**, c'est-à-dire la **zone client de la
FENÊTRE**. Et le commentaire dit pourquoi :

> Sur la région RÉELLEMENT capturée, pas sur la zone client entière : le
> navigateur normalise ses coordonnées sur l'image qu'il reçoit, **et cette
> image est l'intersection de la fenêtre avec l'écran**.

🔴 **Cette dernière affirmation n'est plus vraie du produit d'aujourd'hui.**
La capture est en **`ModeCapture::SortieEntiere`** — mesuré, § 3 — donc
**l'image reçue par le navigateur est la SORTIE ENTIÈRE**, fond d'écran et
barre des tâches compris. C'est d'ailleurs exactement ce que le propriétaire
décrit au symptôme ①.

**Le navigateur normalise donc sur l'image de la SORTIE, et l'agent démappe
sur la FENÊTRE. Les deux ne parlent pas du même rectangle.**

🔵 **Et c'est précisément ce qui explique le partage clavier/souris** : le
clavier ne porte **aucune coordonnée**, il ne peut pas être victime d'un
mauvais rectangle. La souris, si. **Aucune autre hypothèse examinée n'explique
ce partage aussi simplement.**

**Conséquences attendues, toutes conformes aux symptômes** :
- un clic sur la **barre des tâches** (bas de l'image) est démappé **dans la
  zone client de Notepad** → l'application reçoit un clic, la barre des tâches
  n'en reçoit aucun → **rien ne se passe** (symptôme 2, premier cas) ;
- un clic **dans Notepad visible** atterrit à une position **comprimée**,
  ailleurs dans le document → le caret bouge dans un document vide, ce qui se
  lit comme « rien » (symptôme 2, second cas).

### 2.4 Ce qui CONFIRMERAIT cette hypothèse

🔴 **Le contrôle qui vaut : mesurer où le curseur ATTERRIT, hors du flux
vidéo.** Depuis une tâche `/it` qui imprime sa session, relever `GetCursorPos`
après une injection, et le comparer au point visé.

**Avec ses deux bras**, et c'est ce qui le rend lisible :
- **bras attendu FAUX** : viser un point de l'image de la sortie **hors** de la
  fenêtre (la barre des tâches) → le curseur doit tomber **dans** la fenêtre ;
- **bras attendu JUSTE** : viser un point **dans** la fenêtre, quand la fenêtre
  occupe toute la sortie → le curseur doit tomber au bon endroit.

⚠️ **Non joué** : bouger le curseur pendant que le propriétaire travaille est
désagréable. **Je le demande plutôt que de le prendre.**

### 2.5 L'hypothèse concurrente, gardée ouverte

⚠️ **`self.hwnd` pourrait désigner une AUTRE fenêtre** que celle capturée.
L'injecteur est construit une fois (`demarrage.rs:266`) avec le `hwnd` de la
session ; si ce `hwnd` n'est pas celui que la capture montre, le démappage est
faux **pour une autre raison**. **Ce qui la distinguerait** : comparer le
`hwnd` de l'injecteur à celui de la fenêtre de la session — les deux
hypothèses prédisent le même symptôme, et **seule cette comparaison les
sépare**.

---

## 3. ① et ③ — LE CADRAGE EST UNE DÉCISION, pas une panne

🔵 **Confirmé par une trace du produit lui-même, pendant la session du
propriétaire** — et non supposé :

```
20:19:23  INFO fenetre{session=…:w-1}: agent::windows_source::redimensionnement:
  redimensionnement ignoré : la source capture une sortie DXGI entière
  (voir le constat de mesure de capteur::plein_ecran, sous-bloc D9)
  width=1580 height=978 mode=SortieEntiere
20:20:46  … w-10 … width=1723 height=1303 mode=SortieEntiere
```

**Onze occurrences** dans le journal. La trace **sort**, elle nomme le mode, et
elle renvoie au sous-bloc qui l'a décidée. **Le diagnostic de ① et ③ est donc
clos par le produit lui-même** : le redimensionnement demandé par le navigateur
est **délibérément ignoré** en `SortieEntiere`.

**Ce que cela explique** : la fenêtre ne suit pas le viewport, donc elle
n'occupe pas toute la sortie, donc **la barre des tâches reste visible**
(symptôme ③) et **redimensionner la PWA ne change rien** (symptôme ④).

### 3.1 L'état des lieux du mécanisme, sans le juger

- `ModeCapture::SortieEntiere::redimensionne_la_fenetre()` rend **faux**
  (`windows_source/sortie.rs`), et un test le fige.
- Le mode est **choisi délibérément** sur le chemin multi-fenêtres :
  « le discriminant qui manquait : sans lui, `resize` retaillerait cette
  fenêtre-ci et lui substituerait le bureau physique ».
- `PLEIN_ECRAN_MODE_SORTIE` — la variable qui armait le changement de mode de
  la sortie — **n'existe plus** : retirée au sous-bloc D9.
- 🔴 **Les deux Critiques de D8/D9 sont toujours OUVERTES, à dessein** :
  **C1**, la pollution du registre qui bloque les ouvertures ultérieures
  (portée inconnue, cinq GUID SudoVDA au journal de recette) ; **C2**, la
  reprise de D2 court-circuitée par une `Err` sur un échec **transitoire**.
  Elles sont inatteignables **parce que** le mécanisme est désarmé.

⚠️ **Rétablir l'accord fenêtre/sortie rouvrirait ces deux Critiques.** C'est
la raison pour laquelle ce dépôt les a fermées, et elle n'a pas changé.

### 3.2 ③ Pourquoi Notepad paraît après un aller-retour plein écran du navigateur

**Je ne le sais pas, et je ne l'invente pas.** Ce geste ne redimensionne
**aucune** fenêtre Windows — la trace ci-dessus montre que le `resize` est
ignoré. Deux pistes, **non départagées** :

- **(a)** le geste change le **viewport annoncé**, donc `taille_retenue`, donc
  la cible du **contrôle périodique de placement** — qui replace alors la
  fenêtre ;
- **(b)** le geste provoque une **renégociation** ou une réouverture de
  session, et c'est le chemin de création (`creer_sortie` → `placement::poser`)
  qui repose la fenêtre.

**Ce qui les départagerait** : lire, dans le journal et à l'horodatage du
geste, s'il y a un `enfant lancé` (b) ou seulement des
`fenêtre sortie de sa sortie, replacement` (a). **Non fait** : je n'ai pas
l'horodatage du geste du propriétaire.

### 3.3 🔵 Un fait mesuré qui éclaire le placement initial

**587 `fenêtre sortie de sa sortie, replacement`** dans le journal, et les
rectangles disent quelque chose de précis :

```
w-2 : de="1428x1080+4428+51"  vers="1428x1080+3140+0"
w-3 : de="1428x1080+6288+51"  vers="1428x1080+5000+0"
w-1 : de="1428x1039+1280+0"   vers="1428x1080+1280+0"
```

⚠️ **L'écart de `w-2` et `w-3` est IDENTIQUE : +1288 en x, +51 en y.** Un
**décalage constant**, donc une **translation**, jamais une mise à l'échelle —
ce qui écarte d'emblée une hypothèse de DPI. Et le replacement se répète
**chaque seconde** avec le **même** `de=`, ce qui veut dire que le
`SetWindowPos` **ne déplace pas la fenêtre** — alors que
`placement de la fenêtre échoué` est à **0**.

🔴 **C'est le patron que ce dépôt nomme : « JUGER SUR LA RELECTURE, JAMAIS SUR
LE CODE DE RETOUR ».** `SetWindowPos` rend un succès sur une fenêtre qui n'a
pas bougé d'un pixel. **Non expliqué**, et c'est probablement le cœur du
symptôme ①.

⚠️ `w-1` est un cas **différent** : bonne position, hauteur **1039 au lieu de
1080** — 41 px de moins, l'ordre de grandeur d'une **zone de travail** amputée
de la barre des tâches. **Deux symptômes distincts sous une même trace**, et
les confondre serait une erreur.

---

## 4. Les hypothèses, avec leur critère de confirmation

| # | Hypothèse | Ce qui la confirmerait |
| --- | --- | --- |
| **S1** | **La souris est démappée sur la FENÊTRE alors que l'image est la SORTIE.** ✅ **Les deux bouts sont LUS (§ 6) et l'erreur est DÉRIVÉE : origine + échelle.** | Mesurer `GetCursorPos` après injection, hors du flux, **deux bras** (§ 2.4, § 6.5). |
| **S2** | Le `hwnd` de l'injecteur n'est pas celui de la fenêtre capturée. | Comparer les deux `hwnd`. **Sépare S1 de S2** — elles prédisent le même symptôme. |
| **P1** | `SetWindowPos` réussit sans déplacer (fenêtre maximisée, contrainte de moniteur, ou style). | Relever le rectangle **immédiatement après** l'appel, dans le même tour. Un succès qui ne bouge rien est la preuve. |
| **P2** | La cible du placement est **périmée** : la disposition du bureau virtuel a changé depuis la création. | Comparer le `sortie.rect` **courant** de la sortie attribuée au `vers=` que le contrôle vise. Un écart de +1288 les apparie. |
| **C1** | Le cadrage est la **décision `SortieEntiere` de D9**, pas un défaut. | ✅ **DÉJÀ CONFIRMÉ** par la trace du produit, onze fois (§ 3). |

🔴 **S1 et P1 ne sont pas concurrentes** : la première explique le clic, la
seconde le cadrage. **Les confondre ferait corriger l'une en croyant l'autre
réglée.**

---

## 5. Ce que cette manche N'établit PAS

- **rien n'est corrigé, rien n'est déployé, rien n'a été relancé** ;
- **S1 n'est pas mesurée** : elle est cohérente avec les cinq symptômes, dont
  le partage clavier/souris, mais **la mesure du curseur n'a pas été jouée** —
  elle dérangerait le propriétaire, et je la **demande** ;
- **le geste plein écran n'est pas expliqué** (§ 3.2) ;
- **le décalage de +1288 n'est pas expliqué** (§ 3.3) ;
- ⚠️ **la latence de bout en bout reste non mesurée**, et le jugement visuel
  porté ne couvre que la **fluidité**.

---

## 6. Les deux bouts, lus — et l'erreur DÉRIVÉE, pas devinée

**Correction du propriétaire** : la souris **fonctionne**, elle ne clique
simplement **pas au bon endroit**. Cela réfute au passage l'hypothèse « les
clics partent sur l'écran principal » : le curseur **paraît** dans le flux,
donc il est bien sur la bonne sortie. **Il est décalé.**

### 6.1 Ce que le CLIENT envoie

`client/src/input.ts` :

```js
// Les coordonnées sont normalisées sur 0..65535 par rapport à la zone d'image
const rect = video.getBoundingClientRect();
const x = ((clientX - rect.x) / Math.max(1, rect.width)) * 65535;
```

🔵 **Une FRACTION de l'image reçue** — la zone du `<video>` —, jamais des
pixels de la VM. `proto::input::MouseMove { x: u16, y: u16 }`.

### 6.2 Ce que l'AGENT en fait

`agent/src/input.rs::move_mouse` applique cette fraction à
**`client_rect_on_screen(self.hwnd)`** : la **zone client de la FENÊTRE**.

### 6.3 🔴 Les deux ne parlent pas du même rectangle — et l'erreur se calcule

Soit **`O`** le rectangle de la **sortie** (ce que l'image montre, en
`SortieEntiere`) et **`W`** celui de la **zone client de la fenêtre** :

```
point VRAI    = ( Ox + fx·Ow ,  Oy + fy·Oh )
point INJECTÉ = ( Wx + fx·Ww ,  Wy + fy·Wh )

erreur(fx) = (Wx − Ox) + fx·(Ww − Ow)
             └── ORIGINE ──┘   └── ÉCHELLE ──┘
```

🔵 **Réponse à la question posée : c'est LES DEUX.** Un terme **constant**
égal à la position de la fenêtre dans sa sortie, **plus** un terme
**proportionnel** à l'écart de taille. Et les deux sont non nuls dans l'état
observé :

- **terme d'origine** — les relevés de replacement donnent la fenêtre à
  `+4428+51` (`w-2`) et `+6288+51` (`w-3`) pour des sorties dont l'origine
  visée est `3140` et `5000` : **+1288 en x, +51 en y** ;
- **terme d'échelle** — la barre des tâches est **visible dans l'image**, donc
  `Oh > Wh` par construction, et le redimensionnement est **ignoré**
  (§ 3), donc rien ne les réaccorde.

⚠️ **Nul au coin haut-gauche seulement si la fenêtre est à l'origine de sa
sortie** — ce qui n'est pas le cas ici.

### 6.4 🔵 Ce que cela réconcilie

**Une seule racine pour les trois symptômes** : au sous-bloc D10, **la sortie a
cessé d'être la fenêtre** (`ModeCapture::SortieEntiere`), et le chemin des
entrées est resté sur l'**ancienne référence**. Le commentaire de `move_mouse`
le dit encore aujourd'hui, au présent :

> cette image est l'intersection de la fenêtre avec l'écran

**C'était vrai avant D10. Ce ne l'est plus.** Et c'est le patron
`placement.rs:4-7` — une affirmation exacte à l'écriture, devenue fausse sous
elle, jamais relue.

⚠️ **Le clavier n'est pas concerné** : il ne porte aucune coordonnée. Le
partage clavier/souris est **prédit** par cette hypothèse, il n'est pas une
coïncidence à expliquer en plus.

### 6.5 Le contrôle qui trancherait — **toujours demandé, toujours pas joué**

Viser deux ou trois points connus, relever `GetCursorPos` depuis une tâche
`/it` qui imprime sa session, comparer :

| Ce qu'on observerait | Ce que cela dirait |
| --- | --- |
| décalage **constant** | terme d'origine seul |
| décalage **croissant avec la distance** | terme d'échelle seul |
| **nul en haut-gauche et croissant** | les deux — **ce que la dérivation prédit** |

⚠️ **Non joué** : cela bougerait le curseur du propriétaire pendant qu'il
travaille. **Je le demande.**

### 6.6 Un fait neuf, relevé au passage et NON expliqué

```
w-10 : de="160x28+-32000+-32000"  vers="1428x1080+1280+0"
```

**`-32000,-32000` est le rectangle d'une fenêtre MINIMISÉE.** Le contrôle
périodique tente de la replacer **chaque seconde**, sans effet, alors que
`placement::poser` appelle pourtant `ShowWindow(SW_SHOWNORMAL)` avant
`SetWindowPos`. **Troisième cas distinct sous la même trace** — après le
décalage constant (`w-2`, `w-3`) et l'écart de hauteur (`w-1`). **Non
expliqué, et à ne pas confondre avec les deux autres.**

---

## 7. LE REMÈDE DES ENTRÉES — livré, testé, **non déployé**

### 7.1 Le principe : une seule source, pas deux descriptions à tenir d'accord

🔴 **Ce défaut est né parce que deux endroits décrivaient le même rectangle et
qu'un seul a suivi D10.** Une correction qui laisserait subsister deux
descriptions se redéferait au prochain changement de mode.

🔵 **La source unique existait déjà** : `config.sortie_dxgi` est **le
discriminant du mode de capture** (`demarrage/source.rs` : `Some(nom)` ⇒ source
distante servie par le capteur, sortie entière ; `None` ⇒ capture locale de la
fenêtre recadrée). **La référence des entrées en dérive désormais, de la même
valeur** — `crate::entrees::reference(config.sortie_dxgi)`. Il n'y a plus rien
à tenir d'accord.

| | Avant | Après |
| --- | --- | --- |
| Le client normalise sur | l'image reçue | l'image reçue |
| L'agent démappe sur | **la zone client de la FENÊTRE** | **ce qui a été capturé** |

### 7.2 Les cinq tests d'hôte, et la rouge porte les chiffres du relevé

`agent/src/entrees.rs`, module **pur**. La rouge demandée :

```rust
la_formule_d_avant_rend_le_decalage_releve_de_1288_et_51
    assert_eq!(faux_x - juste_x, 1288);   // le terme d'ORIGINE en x
    assert_eq!(faux_y - juste_y,   51);   // le terme d'ORIGINE en y
```

🔵 **Et un second test montre que ce n'est PAS un simple décalage** :
`l_erreur_n_est_pas_un_simple_decalage_elle_croit_avec_la_distance` — l'écart
vaut 1288 au coin haut-gauche et **1288 − 492** au bout, `492 = Ow − Ww`. **Une
correction par soustraction d'une constante aurait été fausse partout ailleurs
qu'au coin.**

🔵 **Le témoin** : `quand_la_fenetre_occupe_sa_sortie_les_deux_references_coincident`
— la correction ne change rien quand il n'y avait rien à changer, ce qui
prouve que la rouge vient bien de l'écart des rectangles.

🔴 **Vue rouge par mutation** (référence forcée à la fenêtre, comme avant) :
`left: ZoneClientDeLaFenetre, right: SortieCapturee("\\.\DISPLAY7")`.
Restauration depuis une **copie nommée**.

### 7.3 Deux décisions à connaître

- ⚠️ **Le rectangle de la sortie est mis en CACHE** (`PEREMPTION_SORTIE = 1 s`,
  **non calibrée et déclarée telle**) : `move_mouse` court à la cadence de la
  souris, et énumérer DXGI à chaque événement coûterait des dizaines d'appels
  COM par seconde. Mais **pas figé** : la disposition du bureau change quand
  une sortie naît ou meurt, et une origine périmée redonnerait le défaut.
- 🔴 **Aucun repli sur la fenêtre si la sortie est introuvable.** Ce serait
  réintroduire **en silence** le décalage qu'on supprime. Le chemin rend une
  **erreur nommée** — bruyant plutôt que faux.

### 7.4 Le commentaire menteur, corrigé là où il vit

`move_mouse` affirmait **au présent** : « cette image est l'intersection de la
fenêtre avec l'écran ». **Vrai avant D10, faux depuis.** Le commentaire dit
désormais ce qui est vrai, ce qui l'était, **et porte les trois commandes qui
l'établissent**. ⚠️ **Second commentaire de ce dépôt à mentir au présent** après
`superviseur/placement.rs`, et pour la même raison : exact à l'écriture, jamais
relu après le changement qui l'a défait.

---

## 8. Les deux faits neufs — DIAGNOSTIC, et ils sont expliqués par LECTURE

### 8.1 ① Pourquoi la boucle recommence, et pourquoi elle ne voit pas son échec

`boucle.rs:74` : `PERIODE_PLACEMENT = 1 s`. `replacer_si_besoin` :

```rust
let Ok(actuel) = placement::rectangle_de(hwnd) else { return };
if placement::doit_etre_replacee(&actuel, &cible) {
    tracing::info!(… "fenêtre sortie de sa sortie, replacement");
    if let Err(erreur) = placement::poser(hwnd, &cible) { … }
}
```

🔴 **Elle recommence parce que `doit_etre_replacee` est réévaluée à chaque
seconde sur le rectangle COURANT.** Si `poser` ne déplace pas, le tour suivant
retrouve le même écart, journalise, retente — **indéfiniment**. D'où les
**587** lignes.

🔴 **Et elle ne voit pas son échec parce qu'elle ne juge `poser` QUE sur son
`Result`.** `poser` rend `Ok` dès que `SetWindowPos` rend un succès ; **il ne
relit jamais le rectangle après l'appel**. Un `SetWindowPos` qui réussit sans
rien déplacer est donc **indiscernable** d'un qui a marché.

⚠️ **C'est très exactement la règle que ce dépôt s'est écrite** — *« JUGER SUR
LA RELECTURE, JAMAIS SUR LE CODE DE RETOUR. Une API peut rendre 0 sur une
sortie qui n'a pas bougé d'un pixel »* — **violée ici**. Le remède évident
serait de relire après `poser` et de ne réessayer qu'un nombre borné de fois,
**mais je ne l'écris pas** : ce n'est pas ce qui m'est autorisé.

### 8.2 ② La fenêtre minimisée — et c'est probablement LE symptôme d'origine

`w-10` à `160x28+-32000+-32000` : le rectangle canonique d'une fenêtre
**minimisée** sous Windows.

🔵 **Cela expliquerait le premier symptôme du propriétaire, entièrement** :
*« je vois le fond d'écran et la barre des tâches, pas Notepad »*. **Une
fenêtre minimisée n'est sur aucun écran** — la sortie capturée ne montre donc
que le bureau. Et l'aller-retour en plein écran du navigateur l'aurait
**restaurée**, ce qui est le symptôme ③.

⚠️ **Ce qui rend le fait troublant** : `placement::poser` appelle pourtant
`ShowWindow(hwnd, SW_SHOWNORMAL)` **avant** `SetWindowPos` — donc le produit
demande la restauration **une fois par seconde**, et la fenêtre reste
minimisée. ⚠️ **Le résultat de `ShowWindow` est ignoré** (`let _ = …`) : s'il
échoue, rien ne le dit.

**Trois hypothèses, et ce qui les départagerait** :

| # | Hypothèse | Ce qui la confirmerait |
| --- | --- | --- |
| **M1** | `ShowWindow` **échoue** et le `let _` l'avale. | Journaliser sa valeur de retour, ou la relever depuis une sonde `/it`. |
| **M2** | `ShowWindow` **réussit**, et quelque chose **re-minimise** la fenêtre entre deux tours. | Échantillonner le rectangle à ~15 ms : on verrait l'aller-retour, invisible à 1 Hz. |
| **M3** | La fenêtre n'est pas minimisée mais **détruite/remplacée**, et le `hwnd` de la table est **périmé**. | Comparer le `hwnd` de la table à celui d'une énumération courante. |

🔴 **M3 se teste sans rien perturber** et devrait être fait en premier : un
`hwnd` périmé expliquerait AUSSI que `SetWindowPos` « réussisse » sans effet
visible (§ 8.1) — **une seule cause pour les deux faits**.

---

## 9. Ce que cette manche N'établit PAS

- **rien n'est déployé** : le binaire en place a été jugé bon par un humain,
  et **un remède non mesuré ne le remplace pas** ;
- **la correction des entrées n'est pas MESURÉE sur la VM** : elle est dérivée,
  testée sur l'hôte, et sa rouge porte les chiffres du relevé — **ce n'est pas
  la même chose qu'un curseur qui atterrit au bon endroit** ;
- **la mesure du curseur reste à jouer**, et **l'AVANT disparaîtra dès le
  déploiement** : elle est préparée (§ 2.4, § 6.5), et j'attends le créneau ;
- **les deux faits neufs ne sont pas corrigés** — expliqués par lecture pour
  l'un, trois hypothèses départageables pour l'autre ;
- ⚠️ **le cadrage n'est pas touché** : décision de D9, deux Critiques ouvertes
  à dessein, et rétablir l'accord fenêtre/sortie les rouvrirait. **Cette
  décision appartient au propriétaire.**

---

## 10. M3 éprouvé — **NON DÉCIDÉ**, et je dis pourquoi plutôt que de conclure

M3 était : *le `hwnd` que la table croit sien est périmé*, ce qui expliquerait
**à la fois** le `SetWindowPos` qui « réussit » sans effet et la lecture d'un
rectangle de fenêtre minimisée. Une seule cause pour deux faits — c'est
pourquoi elle méritait d'être éprouvée en premier.

### 10.1 Ce que la sonde a rendu (session 1, lecture seule)

| Relevé | Valeur |
| --- | --- |
| `IsWindow(0x180360)` — un `hwnd` du journal de 20:17 | **false** |
| `IsWindow(0x1A064A)` — l'autre | **false** |
| fenêtres de premier niveau, session 1 | **126** |
| fenêtres à `-32000,-32000` | **0** |
| dernier `fenêtre sortie de sa sortie` | **20:20:51** |

### 10.2 🔴 Pourquoi cela ne décide RIEN

**Mes observations sont séparées du phénomène par une vingtaine de minutes.**

- Les deux `hwnd` `IsWindow=false` **ne prouvent pas** qu'ils étaient périmés
  quand l'agent s'en servait : ils datent de 20:17, et ces fenêtres ont
  simplement **fermé depuis**. Conclure de leur invalidité *aujourd'hui* à
  leur invalidité *alors* serait exactement le défaut que ce chantier a
  dénoncé trois fois — **un relevé daté n'est pas une mesure du présent, et
  l'inverse n'est pas plus vrai.**
- **Zéro fenêtre à `-32000`** aujourd'hui est **cohérent** avec le fait que le
  phénomène a cessé — ce n'est pas une réfutation de M2 ni de M1.

**Ce qui déciderait** : une corrélation **simultanée** — pendant qu'une ligne
`de="…-32000…"` s'écrit, interroger `IsWindow` sur le `hwnd` de CETTE session.
⚠️ Le `hwnd` de la table **n'est journalisé nulle part** ; il faudrait soit
l'ajouter à la trace de replacement (une ligne), soit lire `FENETRE_HWND` dans
l'environnement de l'enfant. **Ni l'un ni l'autre n'est fait** : le premier est
une modification du produit, que je n'ai pas à écrire ici.

### 10.3 🔵 Un fait neuf qui CORRIGE ma propre affirmation

J'ai écrit au § 8.1 que la boucle retentait **« indéfiniment »**. **C'est
faux, et la mesure le montre** : la dernière trace de replacement est à
**20:20:51**, identique avant et après une sonde de seize secondes lancée
vingt minutes plus tard. **La rafale des 587 lignes était BORNÉE** — elle a
cessé d'elle-même, très probablement avec la fin de la session `w-10`.

⚠️ **Ce que cela change au diagnostic** : ce n'est pas « une boucle qui tourne
à vide en permanence sur une machine dont quelqu'un se sert », c'est **une
rafale d'une à trois minutes liée à une session**. Moins grave, et il faut le
dire — j'avais forcé le trait.

⚠️ **Ce que cela ne change pas** : le mécanisme reste celui du § 8.1 —
`poser` n'est jugé que sur son `Result`, jamais sur une relecture. La rafale
s'arrête parce que la **session** s'arrête, pas parce que le produit constate
quoi que ce soit.

### 10.4 L'état des trois hypothèses

| # | État |
| --- | --- |
| **M1** `ShowWindow` échoue et le `let _` l'avale | **non éprouvée** — demande de journaliser son retour |
| **M2** quelque chose re-minimise entre deux tours | **non éprouvée** — demande un échantillonnage à ~15 ms *pendant* le phénomène |
| **M3** le `hwnd` est périmé | 🔴 **NON DÉCIDÉE** — le phénomène a cessé avant que je puisse corréler |

**Aucune n'est écartée. Aucune n'est confirmée.** Le phénomène est
**intermittent et lié à une session**, donc reproductible seulement en en
ouvrant une — ce que je n'ai pas fait, le propriétaire se servant de la
machine.

---

## 11. Le bras « AVANT » de la mesure du curseur existe déjà, sous deux formes

⚠️ **Il serait injuste de présenter la mesure sur la VM comme la seule
preuve.** Le décalage est déjà établi par **deux pièces indépendantes** :

1. 🔵 **Le test d'hôte** `la_formule_d_avant_rend_le_decalage_releve_de_1288_et_51`,
   qui **reproduit** le décalage à partir des rectangles relevés dans le
   journal de production ;
2. 🔵 **Le témoignage du propriétaire**, qui a vu ses clics tomber à côté —
   c'est lui qui a rapporté le symptôme.

**La mesure du curseur sur la VM serait une TROISIÈME pièce** : précieuse,
parce qu'elle mesurerait l'effet réel plutôt que la formule, mais **non
décisive à elle seule** et **non nécessaire** pour établir le défaut. Ce
qu'elle apporterait de propre : le bras **AVANT**, qui **disparaît au
déploiement** et n'existera plus jamais.

---

## 12. La trace qui rend M3 décidable (lot 32P)

`fenêtre sortie de sa sortie, replacement` porte désormais **`hwnd`** et
**`fenetre_vivante`** — `IsWindow` **à l'instant même** du replacement.

🔵 **C'est ce qui manquait, et rien d'autre n'a été touché** — ni la boucle, ni
la relecture après `poser`. `fenetre_vivante = false` expliquerait **d'un
coup** les deux faits ouverts ; `true` les laisserait tous deux entiers.

---

## 13. La fenêtre de déploiement — **prête, et chiffrée**

`journaux-lot32q/instrument/fenetre-de-deploiement.sh`, plus la sonde
`curseur.ps1` (session 1, `GetCursorPos` à 100 ms) et le pilote
`pilote-curseur.mjs`.

### 13.1 🔴 Ce qui interrompt le propriétaire commence AVANT le redémarrage

⚠️ **Le rôle `client` est EXCLUSIF par session** (établi au lot 22) : le pilote
lui **prend sa place dès qu'il se connecte**, donc **dès la mesure d'avant**.
La fenêtre est **continue**, de la première mesure à la dernière — ce n'est pas
« deux minutes de redémarrage », c'est **tout le créneau**.

### 13.2 Ce qui en est SORTI, et pourquoi

**La fabrication croisée et le dépôt par le hook ne touchent pas la VM.** Ils
se font **avant**, hors créneau — les y mettre coûterait au propriétaire deux
minutes pour rien.

### 13.3 Le chiffrage, à partir des durées mesurées ce jour

| Étape | Durée | Dans le créneau ? |
| --- | --- | --- |
| Fabrication croisée + dépôt par le hook | ~80 s | **non** |
| Mesure du curseur **avant** (sonde 40 s + pilote) | ~90 s | oui |
| Arrêt, copie nommée, dépôt VM, relance, session 1 | ~60 s | oui |
| Mesure du curseur **après**, mêmes points | ~90 s | oui |
| Marge pour les à-coups WinRM (trois observés ce jour) | ~60 s | oui |

🔵 **Créneau à annoncer : SIX MINUTES**, dont quatre de travail utile et deux de
marge. **Le propriétaire ne peut pas se servir du système pendant tout ce
temps**, et devra **rouvrir sa session** ensuite.

### 13.4 Les trois points, et pourquoi trois

Le test d'hôte a montré que l'erreur a **un terme d'origine et un terme
d'échelle**, et qu'ils **ne se distinguent qu'à distance**. Donc :

| Point | Fraction de l'image | Ce qu'il isole |
| --- | --- | --- |
| **A** | 0,02 / 0,02 | le terme d'**ORIGINE** presque seul |
| **B** | 0,50 / 0,50 | départage — une erreur affine passe par B |
| **C** | 0,98 / 0,98 | la **somme** des deux termes |

🔴 **Deux points auraient pu tomber juste par hasard.** A seul ne verrait pas
l'échelle ; C seul confondrait les deux termes.

⚠️ **Le pilote vise dans `contentRect(video)`** — l'image **bandes noires
exclues**, calculée depuis `videoWidth`/`videoHeight` —, c'est-à-dire
**exactement le rectangle sur lequel le client normalise**. Viser le rectangle
brut de l'élément ferait mesurer au pilote **sa propre erreur de cadrage**.

### 13.5 Comment se lira le bras d'APRÈS

| Ce qu'on observerait | Ce que cela dirait |
| --- | --- |
| A, B, C **tous justes** | la référence est corrigée |
| A juste, C faux | il reste un terme d'**échelle** |
| A et C faux du même nombre | il reste un terme d'**origine** |
| A juste et C juste, B faux | **impossible pour une erreur affine** — donc autre chose, et il faudrait chercher |

⚠️ **Le bras d'AVANT n'existe qu'une fois** : il disparaît au redémarrage. Ne
pas inverser l'ordre, ne pas le sauter — c'est écrit en tête du script.
