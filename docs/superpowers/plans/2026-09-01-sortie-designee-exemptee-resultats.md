# La sortie DÉSIGNÉE cesse d'être refusée sur sa taille — 1ᵉʳ septembre 2026

**Le symptôme rapporté par le propriétaire** : « J'ai lancé steam depuis
hub.html, mais rien ne s'affiche. De plus quand je lance une session moonlight,
steam ne s'affiche pas. »

🔴 **Ce n'était pas Steam. AUCUNE application ne pouvait s'afficher**, et les
deux moitiés de la phrase avaient deux causes différentes, sans rapport l'une
avec l'autre.

---

## 1. La cause première, mesurée sur l'agent de PRODUCTION

`creer_sortie` demande au pilote SudoVDA une sortie virtuelle **à la taille du
viewport** (1614×1080 après `borner_a_la_taille_max`), puis exige, par
`placement::sortie_pour_viewport` → `sortie_assez_grande`, que la sortie née
soit **au moins aussi grande**. Le pilote n'honore pas la taille demandée.

Relevé dans `C:\nivuus\agent.log`, **huit fois, en boucle** :

```text
ERROR aucune sortie candidate ne peut servir ce viewport — elle est rendue au pilote
  demande="1614x1080" designee="\\.\DISPLAY6" candidates=["\\.\DISPLAY6 1428x1080"]
```

Le champ `designee` est **NON VIDE** : la sortie refusée était **la nôtre**,
nommée par l'API CCD (chemin ① du lot 32). Elle était refusée sur sa **seule**
taille. Créer → refuser → détruire, en boucle, et pas une fenêtre servie.

🔴 **LA MÊME DEMANDE, LA MÊME EXÉCUTION DE L'AGENT, DES TAILLES DIFFÉRENTES
SELON L'HEURE** — c'est ce contraste qui a fait la preuve, et non une lecture
de code :

| Heure (UTC) | Demandé | Sortie née | Issue |
| --- | --- | --- | --- |
| 12:14–12:19 | 1614×1080 | **1860**×1080 | servie (w-2…w-8) |
| 20:46 | 1614×1080 | **1428**×1080 | **8 refus** |

Sonde en **session 1** (tâche planifiée `/it`, `== session = 1` en première
ligne) au moment du blocage : **aucune** sortie virtuelle ne subsistait, seul
`\\.\DISPLAY1 1280×800`.

### Ce que le lot 33 a changé sans le savoir

L'inconnue est ouverte depuis **D8** (« une sortie ne naît PAS à la taille
demandée »). Elle était jusqu'ici sans conséquence parce que la demande ne
suivait pas le navigateur. Le lot 33 a fait suivre la taille demandée au
**viewport** : une fenêtre de navigateur large demande alors plus que ce que le
pilote rend, et l'inconnue devient une **panne totale**.

---

## 2. Le remède : une CONTRAINTE, pas un contrat

**Quand la désignation a POSITIVEMENT nommé notre sortie, sa taille n'est plus
un critère de REFUS.** `windows_source_sortie::taille_pour_viewport` sait déjà
y ajuster la fenêtre **à rapport d'aspect préservé** (lot 33) : refuser, c'était
refuser la seule sortie qu'on pouvait servir.

`sortie_pour_viewport` reçoit désormais `designee: Option<&str>`.

⚠️ **CE QUI N'EST PAS DESSERRÉ** — et sans quoi ce serait une régression :

- `attachee_au_bureau` : une sortie non rattachée n'a rien à dupliquer ;
- `deja_prises` : **c'est LUI, et non la taille**, qui empêche deux fenêtres de
  montrer la même image ;
- l'exemption est **NOMINATIVE** — elle ne vaut que pour la sortie que la
  désignation a nommée, jamais pour ses voisines ;
- `None` reste **le produit d'hier, ligne pour ligne**, donc le repli par
  différence d'ensembles continue de refuser un écran PHYSIQUE trop petit.

🔴 **UNE AFFIRMATION DU DÉPÔT EST DEVENUE FAUSSE, ET ELLE EST CORRIGÉE PLUTÔT
QUE CONTOURNÉE.** L'en-tête de `superviseur::designation` écrivait : « La
désignation **resserre** l'ensemble des candidates, elle ne desserre aucun
garde — c'est toute la différence avec un relâchement de la règle
d'appariement. » Elle desserre désormais **un** garde, nommément. Les **trois**
places ont été cherchées par `grep` et corrigées ensemble (doc du module, doc
du test `la_designation_ne_court_circuite_pas_le_filtre_des_prises`, et la
liste des causes de l'`ERROR` dans `creation_sortie`, dont le commentaire
exige lui-même d'être **recompté** à toute addition — la famille `designee`
non vide perd sa cause n°1 et en garde deux).

---

## 3. La méthode, et la rouge

**Extraction DÉDIÉE et PRÉALABLE** : `placement/tests.rs` était à **485
lignes**, donc à quinze du plafond. `mod lisere` et `mod bordure_peinte` sont
sortis **verbatim** vers `placement/tests_lisere.rs` — la forme choisie garde
les deux modules NESTED d'un cran, si bien que leur `use super::super::*;`
désigne toujours `placement` et qu'**aucune ligne d'import n'a bougé**.
485 → **284**.

**La rouge, et elle a rougi pour la bonne raison** : des cinq tests ajoutés,
**un seul** a échoué — celui du cas de production. Les quatre autres sont les
**gardes** (déjà prise, non attachée, sans désignation, exemption nominative)
et ils étaient verts avant comme après : ils ne pouvaient donc pas maquiller
l'effet du correctif.

**Vert** : 1148 + 114 tests, `cargo check --target x86_64-pc-windows-gnu`
propre, plafond de 500 lignes respecté (seul `windows_source.rs`, dette
préexistante, le dépasse).

---

## 4. La trace, et ce qu'elle a établi EN PRODUCTION

Une trace a été posée **avant** le court-circuit et elle **nomme la branche
prise** (`exemptee=true|false`) : c'est la leçon que le lot 33 venait de payer,
où un `0` au journal ne distinguait plus « le message n'arrive jamais » de « il
arrive et ne change rien ». Sans ce champ, une fenêtre servie ne dirait pas
lequel des deux chemins l'a servie, et l'exemption serait **invérifiable en
production**.

Elle sert aussi d'**attestation du binaire** — une taille ne prouve rien :
chaîne neuve présente (1), chaîne préexistante présente (1), témoin **négatif**
absent (0), empreinte `27ff81bf54018c7d` identique sur l'hôte et sur la VM.

**Après déploiement, le propriétaire a relancé Steam depuis le hub. 16 fenêtres
servies, 0 refus** (contre 8 avant) :

| Sessions | Demandé | Sortie née | `exemptee` |
| --- | --- | --- | --- |
| w-1…w-3, w-5…w-12, w-14…w-16 | 1614×1080 | 1860×1080 | `false` |
| w-13 | 1614×1080 | 1614×1080 | `false` |
| **w-4** | 1614×1080 | **780×492** | **`true`** |

🔵 **L'exemption a tiré UNE fois, et sur un vrai cas** : une sortie née à
**780×492** pour une demande de 1614×1080. Sous le code d'hier cette fenêtre
était refusée. Les quinze autres passent par le chemin normal — **le correctif
n'est donc pas un relâchement global, et la trace le discrimine.**

🔴 **TROISIÈME POINT DE MESURE POUR L'INCONNUE DE D8** : la même demande de
1614×1080 fait naître **1860×1080**, **1614×1080** ou **780×492** selon
l'instant. La relation entre taille demandée et taille née reste **incomprise**,
et c'est précisément pourquoi la traiter comme une contrainte est le bon
arbitrage : la question devient sans objet, pas résolue.

---

## 5. Le SECOND symptôme — Steam absent de Moonlight

**Steam allait très bien.** Sonde session 1 : `steamwebhelper pid=3560
rect=0,0 1280x800` sur `\\.\DISPLAY1`, visible, non minimisé — Big Picture
était affiché sur l'écran **réel**.

Apollo était configuré `dd_configuration_option = ensure_active`, assoupli
depuis `ensure_only_display` au **lot 32C** pour que `desk` puisse coexister :
Apollo active donc sa sortie virtuelle **sans désactiver** `DISPLAY1`, et
Moonlight diffuse un écran sur lequel Steam n'est pas.

Sur décision du propriétaire, la conf est passée à **`ensure_primary`** (copie
nommée `sunshine.conf.copie-nommee-avant-ensure-primary`, service redémarré).

⚠️ **LA CONF PORTAIT DÉJÀ UNE RÉFUTATION MESURÉE DE CETTE OPTION**, et elle est
dite plutôt que passée sous silence : « With `ensure_primary` the dummy plug
stayed primary at (0,0) and fullscreen games opened on IT » (2026-07-23). Ce
montage-là avait un **dummy plug HDMI, aujourd'hui retiré** : la condition a
changé, ce n'est pas une garantie. **Non rejoué sous Moonlight à ce jour.**

⚠️ **Steam déjà ouvert ne bouge pas** : il faut le relancer après avoir démarré
la session Moonlight.

---

## 6. Le SECOND blocage de Steam, qui n'a rien à voir avec le premier

Après déploiement, le premier lancement depuis le hub n'a **rien** produit :
`raccourci lancé` à 21:15:23Z, puis **aucun** événement de fenêtre — ni
adoption, **ni même un refus**.

**Steam était toujours l'instance de 11:03:53**, antérieure à l'agent. Le
`steam.exe` lancé par desk a passé la main à cette instance et s'est terminé :
aucun processus neuf, aucune fenêtre neuve, donc **aucun événement de hook**.

C'est le legs d'appartenance du **lot 32I**, et l'**instance unique** de Steam
le rend **systématique** : tant qu'un Steam précède l'agent, aucun lancement
depuis le hub ne peut produire une fenêtre adoptable. Corollaire non écrit
jusqu'ici : **tout redémarrage de l'agent condamne Steam jusqu'à sa fermeture
complète.** Steam fermé, le lancement suivant a fonctionné.

⚠️ **Une erreur de méthode, consignée** : le `-shutdown` de Steam a été émis
par WinRM, donc **en session 0**, ce qui y a relancé un Steam invisible. C'est
le piège que `CLAUDE.md` documente (« UN RELEVÉ WinRM EST CELUI DE LA
SESSION 0 ») — payé une fois de plus, sur une commande d'ACTION et non de
relevé. Corrigé dans la foulée.

---

## 7. Ce que ce lot n'établit PAS

- 🔴 **PERSONNE N'A REGARDÉ L'IMAGE.** `framesDecoded` n'a pas été relevé, et
  « 16 fenêtres servies » ne dit pas qu'une image est juste. Le jugement du
  propriétaire (« celui de Steam est bien ouvert et bien affiché ») est le seul
  élément visuel, et il porte sur **une** fenêtre.
- 🔴 **AUCUNE RECETTE NAVIGATEUR N'A ÉTÉ JOUÉE** : le rôle `client` est exclusif
  par session, le propriétaire était connecté. Tout ce qui suit vient de son
  geste, pas d'un pilote.
- 🔴 **LA BOUCLE DE REPLACEMENT À 1 Hz N'EST NI EXPLIQUÉE NI CORRIGÉE** —
  **424** replacements en ~4 minutes, la même ligne chaque seconde :
  `de="1544x1034+8139+-1" vers="1542x1032+3140+0"`. La fenêtre ne reste jamais
  où desk la pose. Hypothèse **non mesurée** : la création/destruction de
  sorties virtuelles décale l'origine des sorties existantes, périmant la cible
  mémorisée. Voir le legs ci-dessous.
- ⚠️ **Le `ensure_primary` d'Apollo n'a pas été éprouvé sous Moonlight.**
- ⚠️ **La dérive entre paquets n'est pas résolue** : le template
  `installer/console/guest/templates/sunshine.conf.j2` dit toujours
  `ensure_only_display`, et un test l'assure. La VM disait `ensure_active`,
  édité à la main au lot 32C et **jamais reporté**. **Un reprovisionnement
  écraserait le changement.** Cela touche un AUTRE paquet : décision du
  propriétaire.
- ⚠️ **Le hook ne journalise que les REFUS** : une fenêtre adoptée n'a ni titre
  ni classe au journal, ce qui a empêché d'identifier les 16 fenêtres de Steam
  sans une sonde séparée.

## 8. Legs ouverts par ce lot

- 🔴 **UNE BOUCLE DE REPLACEMENT À 1 Hz**, ci-dessus. Elle coûte du CPU en
  continu et fait bouger les fenêtres à l'écran.
- 🔴 **UNE APPLICATION À FENÊTRES MULTIPLES OUVRE AUTANT D'ONGLETS QU'ELLE A DE
  FENÊTRES** : Steam en présente ~16. Ce n'est **pas** une régression — c'est la
  conception « une fenêtre = une session » rencontrant une application qui en
  ouvre beaucoup, et elle était masquée jusqu'ici par la règle d'appartenance
  qui écartait tout Steam.
- ⚠️ **Des enfants meurent avec `la fenêtre … imposée par le superviseur
  n'existe plus`** : Steam ferme ses fenêtres transitoires plus vite que la
  session ne se monte.
