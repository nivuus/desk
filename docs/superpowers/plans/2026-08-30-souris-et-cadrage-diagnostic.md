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
| **S1** | **La souris est démappée sur la FENÊTRE alors que l'image est la SORTIE.** | Mesurer `GetCursorPos` après injection, hors du flux, **deux bras** (§ 2.4). |
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
