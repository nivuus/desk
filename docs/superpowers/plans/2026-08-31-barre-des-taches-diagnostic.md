# La barre des tâches dans le flux — diagnostic, et les remèdes instruits

**31 août 2026.** Tâche de MESURE seule : **aucun fichier du produit n'a été
modifié, rien n'a été compilé, rien n'a été déployé.** Le propriétaire était
connecté et en train de tester pendant toute la mesure ; il n'a pas été
dérangé, aucune fenêtre ne lui a été ouverte ni fermée, l'agent n'a pas été
redémarré.

Symptôme rapporté : **« j'ai toujours la taskbar qui s'affiche »**.

---

## 1. Ce qui a été MESURÉ

Toutes les mesures qui dépendent de la session ont été prises **en session 1**,
par une tâche planifiée `/it` dont la sonde imprime sa propre session
(`== session = 1`). Les deux tâches (`NivuusTbDiag`, `NivuusTbDiag2`) et le
répertoire `C:\nivuus\tb-diag` ont été **supprimés** ; contrôle joué :
`taches residuelles : 0`, `tb-diag present ? False`, `agent processes : 5`
avant comme après.

### 1.1 La topologie, en session 1

```
hmon=0x6700C5 device=\\.\DISPLAY1 mon=(0,0)-(1280,800)     1280x800  work=1280x752  flags=1 PRIMARY
hmon=0x3E0351 device=\\.\DISPLAY8 mon=(1280,0)-(2708,1080) 1428x1080 work=1428x1032 flags=0
hmon=0x680359 device=\\.\DISPLAY9 mon=(2708,0)-(4136,1080) 1428x1080 work=1428x1032 flags=0
```

et, par `MULTIFENETRE_DXGI=1` lancé **en session 1** :

```
sortie adaptateur=Microsoft Basic Render Driver index_adaptateur=0 nom=\\.\DISPLAY1 x=0    y=0 largeur=1280 hauteur=800
sortie adaptateur=NVIDIA GeForce RTX 4070      index_adaptateur=1 nom=\\.\DISPLAY8 x=1280 y=0 largeur=1428 hauteur=1080
sortie adaptateur=NVIDIA GeForce RTX 4070      index_adaptateur=1 nom=\\.\DISPLAY9 x=2708 y=0 largeur=1428 hauteur=1080
```

🔴 **LA PRIMAIRE N'EST PAS UNE SORTIE SERVIE, ET LE VGA QEMU N'A PAS ÉTÉ
RETIRÉ.** `\\.\DISPLAY1` (1280×800) est porté par `Microsoft Basic Display
Adapter`, `PCI\VEN_1234&DEV_1111` — le VGA standard de QEMU, `Status OK` au
relevé `Get-PnpDevice -Class Display`. C'est **le contraire** de la présomption
du brief, et cela change l'arbitrage du remède 2.

### 1.2 Les barres des tâches

```
hwnd=0x100F0 cls=Shell_TrayWnd          rect=(0,752)-(1280,800)     1280x48 mon=\\.\DISPLAY1 ex=0x00000080
hwnd=0x201DC cls=Shell_SecondaryTrayWnd rect=(1280,1032)-(2708,1080) 1428x48 mon=\\.\DISPLAY8 ex=0x00000088
hwnd=0x20310 cls=Shell_SecondaryTrayWnd rect=(2708,1032)-(4136,1080) 1428x48 mon=\\.\DISPLAY9 ex=0x00000088
ABM_GETSTATE=0x0 autohide=False alwaystop=False | ABM_GETTASKBARPOS edge=3 rc=(0,752)-(1280,800)
```

- **Il y a une barre des tâches SECONDAIRE sur CHACUNE des deux sorties
  servies**, `1428×48`, collée en bas.
- `ex=0x00000088` = `WS_EX_TOOLWINDOW | WS_EX_TOPMOST`. ⚠️ **Nuance mesurée :
  la barre PRIMAIRE, elle, ne porte PAS `WS_EX_TOPMOST` (`ex=0x80`)** ; ce sont
  les **secondaires** — les seules qui recouvrent une fenêtre servie — qui le
  portent.
- **Le masquage automatique est DÉSARMÉ** : `ABM_GETSTATE=0x0`, et
  `StuckRects3\Settings[8] = 0x02` (bit `0x01` = auto-hide, clair).

### 1.3 Le réglage multi-écrans

`HKCU\…\Explorer\Advanced` : **`MMTaskbarEnabled` est ABSENTE** (ainsi que
`MMTaskbarMode`, `TaskbarAl`, `TaskbarSi`, `TaskbarSd`) — donc **le défaut
Windows s'applique : barre sur TOUS les écrans**. `MMStuckRects3` porte onze
blobs, un par moniteur : `SMKD1CE…UID256` à `UID265` (les dix sorties SudoVDA)
plus `Default_Monitor…UID0`. Explorer a donc bien fabriqué une barre par
sortie virtuelle.

### 1.4 Les fenêtres servies

```
hwnd=0x60242 cls=Notepad rect=(1280,0)-(2708,1080) 1428x1080 mon=\\.\DISPLAY8 pid=1452  style=0x14CF0000 ex=0x00000110
hwnd=0x6004E cls=Notepad rect=(2708,0)-(4136,1080) 1428x1080 mon=\\.\DISPLAY9 pid=11800 style=0x14CF0000 ex=0x00000110
```

- Le rectangle de la fenêtre servie **est exactement le rectangle du moniteur**
  (`1428×1080`), **pas** sa zone de travail (`1428×1032`).
- `style=0x14CF0000` porte `WS_CAPTION` **et** `WS_THICKFRAME` : fenêtre
  ordinaire, **jamais** « sans bordure » au sens de `capteur::plein_ecran`.
- `ex=0x110` = `WS_EX_WINDOWEDGE | WS_EX_ACCEPTFILES` : **pas de
  `WS_EX_TOPMOST`**.
- Le premier plan, à l'instant du relevé, était une console PowerShell
  (`pid=11492`) — donc **aucune** des deux fenêtres servies.

Et côté agent, dans `C:\nivuus\agent.log` (vivant, dernière écriture à l'heure
du relevé) :

```
attaché au capteur session=…:w-6 sortie=\\.\DISPLAY9 largeur=1428 hauteur=1080
duplication de sortie établie desktop_width=1860 desktop_height=1080
session NVENC native initialisée largeur=1428 hauteur=1080
```

⚠️ **`GetDesc().DesktopCoordinates` et `DXGI_OUTDUPL_DESC` NE DISENT PAS LA
MÊME CHOSE, et les deux relevés sont vrais de choses différentes** : la sonde
d'énumération rend `1428×1080`, la duplication rend `1860×1080`. C'est
exactement l'écart de 432 px en largeur du lot 32T. **En HAUTEUR ils
coïncident : 1080 des deux côtés** — c'est ce qui rend la conclusion ci-dessous
insensible à cet écart.

---

## 2. Ce qui en est DÉDUIT

### 2.1 L'hypothèse du propriétaire est JUSTE dans son mécanisme, et incomplète

`superviseur/placement.rs::win::poser` pose bien la fenêtre par
`SetWindowPos(HWND_TOP, …, SWP_NOACTIVATE)`, sans maximisation, et le
commentaire dit pourquoi. Les barres secondaires sont bien `WS_EX_TOPMOST`
(mesuré), la fenêtre servie ne l'est pas (mesuré), et elle n'est pas au premier
plan (mesuré). Une fenêtre non-topmost, même à `HWND_TOP`, ne passe donc jamais
devant une fenêtre topmost.

**Ce que l'hypothèse ne dit pas, et qui est la moitié qui compte :**

🔴 **La barre est dans le flux parce que la RÉGION CAPTURÉE couvre la
HAUTEUR ENTIÈRE de la sortie, pas seulement parce que la fenêtre passe
dessous.** Le z-order explique que la barre soit *dessinée* par-dessus
l'application ; il n'explique pas qu'elle soit *transmise*. C'est le cadrage
qui l'explique :

- `creation_sortie.rs` pose la fenêtre à
  `Rect { x: cible.rect.x, y: cible.rect.y, width: retenue.0, height: retenue.1 }`,
  avec `retenue = taille_retenue((1428,1080), (sortie))` = **1428×1080** ;
- `windows_source/sortie.rs::region_de_sortie` recadre
  `Rect { x: 0, y: 0, width: 1428, height: 1080 }` **à l'origine de la
  sortie** ;
- la sortie mesure 1080 de haut. **Les lignes 1032 à 1080 — les 48 rangées de
  la barre secondaire — sont donc À L'INTÉRIEUR du recadrage**, soit
  **4,4 % de la hauteur de l'image**.

**Rien, dans tout le dépôt, ne consulte la zone de travail** : `grep -rni
'rcWork|SPI_GETWORKAREA|work_area'` sur `agent/`, `client/`, `plateforme/`,
`proto/` ne rend **aucune** occurrence. La distinction moniteur / zone de
travail n'existe pas dans ce produit.

Le dépôt le SAIT déjà, en toutes lettres, depuis le lot 32M —
`agent/src/entrees.rs` : « l'image est **la sortie entière**, fond d'écran et
barre des tâches compris » ; et `agent/src/input.rs:264`. Le symptôme n'est
donc pas une découverte, c'est une **conséquence connue et jamais traitée**.

### 2.2 Deux conséquences que le symptôme rapporté ne dit pas

- 🔴 **L'application perd 48 px de contenu, elle ne les partage pas.** La
  fenêtre est posée à `1428×1080`, donc ses 48 dernières rangées existent et
  sont **recouvertes** par la barre. Ce n'est pas une bande ajoutée sous
  l'application : c'est du contenu applicatif masqué.
- 🔴 **Un clic dans les 4,4 % inférieurs de la vidéo atteint la BARRE DES
  TÂCHES, pas l'application.** `input.rs::move_mouse` démappe `0..65535` sur
  `rectangle_de_reference()` = origine de la sortie + `TailleImage`
  (1428×1080) ; `y = 65535` tombe donc à `y = 1080` de la sortie, dans la
  barre. C'est de l'arithmétique sur du code lu, **non exercé par une
  session** (le rôle `client` est exclusif, le propriétaire était connecté).

---

## 3. Les remèdes, et ce que chacun coûte

**Aucun n'est appliqué, aucun n'est choisi.**

### Remède 1 — masquage automatique de la barre (réglage Windows)

**Ce qu'il donne** : la barre disparaît de **toutes** les sorties, servies
comme primaire. Aucune ligne de code produit.

**Ce qu'il coûte** :
- il faut **redémarrer `explorer.exe`** dans la session du propriétaire pour
  que le réglage prenne (ou passer par l'IHM Paramètres) — une interruption
  visible pour lui ;
- une barre en masquage automatique laisse **une lisière de quelques pixels**
  au bord et **ressort dès que le pointeur atteint le bas de l'écran**. Or
  chaque fenêtre servie occupe la hauteur ENTIÈRE de sa sortie : le bas de
  « son application » **est** le bord de déclenchement. Le symptôme deviendrait
  intermittent au lieu de permanent — ce qui est parfois pire à diagnostiquer ;
- le réglage est **global au bureau du propriétaire**, pas propre à `desk` ;
- ⚠️ le blob par moniteur `MMStuckRects3` porte lui aussi un octet de drapeaux
  (mesuré à `0x00` sur les dix sorties), mais **sa sémantique n'est pas
  documentée** : ne pas parier sur un masquage automatique par écran.

**Déjà réfuté par ce dépôt ?** Non — jamais essayé, jamais mesuré.

### Remède 2 — `MMTaskbarEnabled=0`

**Ce qu'il donne** : Explorer ne fabrique plus de `Shell_SecondaryTrayWnd`.
**Sur cette machine, aujourd'hui, cela retire la barre des DEUX sorties servies
et la laisse sur `\\.\DISPLAY1` seule** — la primaire, le VGA QEMU, que
**aucune session ne sert jamais**.

🔵 **La réserve du brief (« n'aide que si la primaire n'est pas une sortie
servie ») est LEVÉE PAR LA MESURE** : la primaire est le VGA QEMU, présent et
`OK`, et une sortie préexistante ne peut pas être appariée — `creer_sortie` ne
cherche que parmi les sorties **APPARUES** (`attendre_notre_sortie`, puis
`sortie_pour_viewport(&candidates, …)`), et le chemin nominal la **DÉSIGNE**
par l'identifiant de cible du pilote (`superviseur::designation`).
`\\.\DISPLAY1` ne peut donc jamais devenir une sortie servie.

**Ce qu'il coûte** :
- un redémarrage d'`explorer.exe` (même coût que le remède 1) ;
- 🔴 **la garantie repose entièrement sur le fait que la primaire n'est pas
  servie, et Windows exige TOUJOURS une primaire.** Si le VGA QEMU était
  retiré — ce que le brief croyait déjà fait —, la primaire deviendrait une
  **sortie SudoVDA**, et **une fenêtre servie sur dix retrouverait la barre**,
  laquelle changerait de sortie au gré des créations/destructions. Ce remède
  crée donc une **dépendance non déclarée** entre `desk` et la présence du VGA
  QEMU dans l'appliance ;
- il ne change rien à la perte de 48 px de contenu applicatif **sur la sortie
  primaire** si celle-ci venait à être servie ;
- réglage global au bureau du propriétaire, pas propre à `desk`.

**Déjà réfuté par ce dépôt ?** Non.

### Remède 3 — fenêtre sans bordure et « vraiment plein écran » sur sa sortie

🔵 **RÉPONSE NETTE À LA QUESTION POSÉE : NON, CE REMÈDE NE ROUVRE NI C1 NI C2.**
Les deux Critiques laissées ouvertes par D8/D9 sont attachées à
`ChangeDisplaySettingsExW` — **C1** est la pollution du registre par
`CDS_UPDATEREGISTRY` (une sortie créée ensuite naît à la taille polluée),
**C2** est la reprise D2 court-circuitée par une `Err` sur un échec transitoire
de **réouverture après changement de mode**. Changer le **style** ou le
**rectangle** d'une fenêtre n'appelle pas cette API, n'écrit rien au registre
et ne relâche aucune duplication. **Changer le mode d'affichage et changer le
style d'une fenêtre ne sont pas la même chose**, et le constat de tête de
`agent/src/capteur/plein_ecran.rs` ne porte que sur le premier.

**Mais le remède ne marche pas, et pour une autre raison :**

🔴 **Windows ne masque la barre que pour une fenêtre plein écran qui est AU
PREMIER PLAN, et il n'y a qu'UN premier plan par session — pour N fenêtres.**
La détection « rudely behaved fullscreen window » du shell exige que la fenêtre
soit *foreground*. Or l'architecture est **N SESSIONS, N fenêtres, N sorties**
(fait d'architecture de `CLAUDE.md`) : au mieux **une** des N sorties verrait sa
barre masquée, et laquelle dépendrait de la fenêtre que le propriétaire vient
de toucher. Et `poser` porte `SWP_NOACTIVATE` **délibérément** (« poser une
fenêtre ne doit pas voler le premier plan ») : le lui retirer ferait, à chaque
tour du contrôle périodique de placement, voler le focus — la panne que
`TOLERANCE_PX` et son test `un_ecart_d_un_pixel_ne_declenche_pas_de_replacement`
existent pour éviter.

**Autres coûts, si l'on essayait quand même :**
- retirer `WS_CAPTION | WS_THICKFRAME` d'une fenêtre applicative est intrusif
  (beaucoup d'applications recalculent mal leur zone client) et **entrerait en
  collision frontale avec `capteur::plein_ecran::SuiviBordure`**, qui se sert
  précisément de l'absence de ces deux bits pour **annoncer** `PleinEcran` au
  navigateur : toutes les fenêtres servies annonceraient le plein écran en
  permanence ;
- ⚠️ **variante non demandée mais voisine — poser `WS_EX_TOPMOST` sur la
  fenêtre servie** : elle, n'exige pas le premier plan et marcherait pour les N
  fenêtres à la fois. Elle est **fragile** : la barre est topmost elle aussi, et
  Explorer réaffirme son z-order sur plusieurs événements (recréation de la
  barre, `WM_SETTINGCHANGE`, notifications `ABN_*`) — c'est une course, pas un
  invariant. Elle changerait en outre le bureau que le propriétaire voit
  localement.

### Remède 4 — recadrer sur la ZONE DE TRAVAIL (voie que je propose d'instruire)

**Ce qu'il donne** : la barre cesse d'être **transmise**, sans rien changer au
bureau du propriétaire ni à Windows. Concrètement : lire `rcWork` du moniteur
(`GetMonitorInfo`), poser la fenêtre dans la zone de travail, et recadrer le
même rectangle. Aujourd'hui `region_de_sortie` est câblée à
`Rect { x: 0, y: 0, … }` et la cible de `poser` est câblée à `cible.rect.x/y` +
`retenue` : ce sont les deux seuls endroits à changer, plus `entrees::TailleImage`
qui est **déjà partagée** et suivrait donc toute seule.

**Ce qu'il coûte** :
- l'image servie devient `1428×1032` pour un viewport demandé de `1428×1080` :
  le client met à l'échelle, ce qu'il fait **déjà** dans le cas « sortie née
  plus petite » (`la_taille_retenue_se_borne_a_la_sortie_quand_celle_ci_est_
  plus_petite`) — mais le rapport d'aspect ne colle plus exactement au
  viewport, donc une fine bande de letterbox ;
- il faut lire `rcWork` **par sortie et le relire**, la barre pouvant changer de
  bord, de taille ou disparaître — un `rcWork` mémorisé serait un « 487 » de
  plus ;
- ⚠️ **la variante « créer la sortie 48 px plus haute pour que sa zone de
  travail égale le viewport » est SÉDUISANTE ET NON FIABLE** : D8 (tâche 3bis)
  a établi qu'**une sortie virtuelle ne naît PAS à la taille demandée** mais à
  la dernière taille laissée au registre — c'est la raison d'être de
  `sortie_assez_grande`. La taille de création n'est pas un levier sûr ;
- 🔵 **ne rouvre ni C1 ni C2** : aucun appel à `ChangeDisplaySettingsExW`,
  aucune écriture au registre ;
- 🔵 **corrige du même geste le clic qui atteint la barre** (§2.2), puisque la
  référence des entrées dérive de la taille d'image partagée.

**Déjà réfuté par ce dépôt ?** Non. ⚠️ **Mais il contredit frontalement le
commentaire de `placement.rs::poser`**, qui refuse `SW_MAXIMIZE` au motif que
« le bas du flux serait une bande de bureau vide ». **Cet argument ne vaut que
si le RECADRAGE reste à la taille de la sortie** : maximiser SANS toucher au
recadrage donne effectivement une bande vide. Recadrer sur la zone de travail
**en même temps** ne la produit pas. Le commentaire est juste sur ce qu'il
décrit et ne couvre pas ce cas-là ; il faudra le corriger en même temps, pas le
laisser mentir au présent une troisième fois.

### Remède 5 — recadrer côté CLIENT

**Ce qu'il donne** : zéro changement agent, zéro changement VM, applicable
immédiatement (une règle CSS sur `#remote`).

**Ce qu'il coûte** :
- 🔴 **il recrée exactement la classe de défaut que les lots 32M à 32T viennent
  de fermer** : deux endroits décriraient à nouveau le même rectangle, et le
  démappage des entrées (`entrees::TailleImage`, **un seul `AtomicU64`**, seule
  source de la taille) ne saurait rien du rognage — les clics dériveraient
  proportionnellement, en y cette fois ;
- les 48 rangées sont **encodées et transmises** de toute façon : le coût
  réseau reste payé ;
- la hauteur de la barre est une donnée Windows que le client n'a aucun moyen
  de connaître.

**Déjà réfuté par ce dépôt ?** Pas nommément — mais la doctrine qui l'interdit
est écrite en toutes lettres en tête d'`agent/src/entrees.rs` : « **UNE SEULE
SOURCE, ET C'EST TOUT L'OBJET DE CE MODULE** ».

### Remède 6 — masquer les `Shell_SecondaryTrayWnd` par `ShowWindow(SW_HIDE)`

**Ce qu'il donne** : effet immédiat, sans redémarrer Explorer.

**Ce qu'il coûte** : Explorer les recrée et les réaffiche sur de nombreux
événements ; l'agent devrait donc les rechasser en boucle. Le bureau local du
propriétaire perdrait ses barres. **Non recommandé** — c'est une course, pas un
invariant.

### Remède 7 — un bureau Windows séparé (`CreateDesktop`) sans Explorer

🔴 **RÉFUTÉ PAR L'ARCHITECTURE, sans mesure nécessaire** : DXGI Desktop
Duplication ne duplique que le bureau **affiché**. Un bureau non visible n'est
pas composité, donc pas capturable.

---

## 4. Ce que je n'ai PAS pu établir

- 🔴 **Personne n'a REGARDÉ l'image.** Le rôle `client` est exclusif par
  session et le propriétaire était connecté : aucune recette navigateur n'a été
  jouée. Que la barre soit dans le flux est établi par la **géométrie** et par
  le code lu, **pas** par un pixel observé.
- 🔴 **Le clic qui atteint la barre (§2.2) est de l'arithmétique, pas une
  mesure.** Il faudrait un créneau où le propriétaire est déconnecté — le même
  créneau que celui qu'attend déjà la mesure du curseur du lot 32T.
- ⚠️ **L'écart `GetDesc` 1428 / `OUTDUPL_DESC` 1860 n'est pas expliqué ici** :
  je l'ai seulement constaté et vérifié qu'il **ne porte que sur la largeur**,
  donc qu'il ne change rien à la conclusion sur la barre. C'est le terrain du
  lot 32T, pas celui-ci.
- ⚠️ **Aucun remède n'a été essayé**, donc aucun n'a de rouge ni de verte. Les
  coûts énoncés au §3 sont des lectures de code et de documentation Windows,
  pas des mesures.
- ⚠️ **La sémantique de l'octet de drapeaux des blobs `MMStuckRects3`** (un par
  moniteur) n'est pas documentée et n'a pas été éprouvée : je ne sais pas si un
  masquage automatique **par écran** est atteignable.
